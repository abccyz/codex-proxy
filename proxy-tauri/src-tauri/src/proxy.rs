use axum::{
    extract::{DefaultBodyLimit, State},
    http::HeaderMap,
    http::header::{CONTENT_TYPE, USER_AGENT},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use tower_http::cors::{CorsLayer, Any};
use serde_json::Value;
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tokio::net::TcpListener;

use crate::convert::*;
use crate::sse::{make_item_id, make_req_id, make_response_id, truncate_utf8, StreamCleanupGuard, ToolCallAcc, FunctionCall};
use crate::metrics::{InputDetail, SharedMetrics};
use crate::model::ModelVendor;

fn build_text_output_item(item_id: &str, text: &str, reasoning_content: &str, model: &str) -> Value {
    let vendor = ModelVendor::from_model_name(model);
    let should_include = vendor.build_reasoning_output(reasoning_content);
    let mut content_parts: Vec<Value> = Vec::new();
    if let Some(reasoning) = should_include {
        content_parts.push(serde_json::json!({ "type": "reasoning", "text": reasoning }));
    }
    content_parts.push(serde_json::json!({ "type": "output_text", "text": text, "annotations": [] }));
    serde_json::json!({ "type": "message", "id": item_id, "role": "assistant", "status": "completed", "content": content_parts })
}

fn kill_port_occupier(port: u16) {
    use std::process::Command;
    if let Ok(output) = Command::new("lsof").args(["-ti", &format!(":{}", port)]).output() {
        for pid_str in String::from_utf8_lossy(&output.stdout).lines() {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                tracing::info!("Killing process {} occupying port {}", pid, port);
                let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
            }
        }
    }
}

pub struct AppState {
    pub metrics: SharedMetrics,
    pub http_client: reqwest::Client,
    pub upstream_url: RwLock<String>,
    pub api_key: RwLock<String>,
    pub upstream_model: RwLock<String>,
    pub config_manager: Arc<crate::config::ConfigManager>,
    pub proxy_running: Arc<AtomicBool>,
    pub connectivity_result: RwLock<Option<crate::connectivity::ConnectivityResult>>,
}

impl AppState {
    pub fn set_upstream(&self, url: String, key: String) {
        *self.upstream_url.write().unwrap() = url;
        *self.api_key.write().unwrap() = key;
    }
    pub fn set_upstream_model(&self, model: String) {
        *self.upstream_model.write().unwrap() = model;
    }
    pub fn get_upstream_url(&self) -> String { self.upstream_url.read().unwrap().clone() }
    pub fn get_api_key(&self) -> String { self.api_key.read().unwrap().clone() }
    pub fn get_upstream_model(&self) -> String { self.upstream_model.read().unwrap().clone() }
}

pub async fn run_server(state: Arc<AppState>, port: u16) -> Option<tokio::task::JoinHandle<()>> {
    eprintln!("[proxy-tauri] Attempting to start proxy server on port {}...", port);
    kill_port_occupier(port);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let app = Router::new()
        .route("/v1/responses", post(handle_responses))
        .route("/health", get(handle_health))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
        .layer(CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any))
        .with_state(state.clone());

    let addr = format!("127.0.0.1:{}", port);
    match TcpListener::bind(&addr).await {
        Ok(listener) => {
            eprintln!("[proxy-tauri] ✅ Proxy server listening on http://{}", addr);
            tracing::info!("Proxy server listening on http://{}", addr);
            let proxy_flag = state.proxy_running.clone();
            proxy_flag.store(true, Ordering::SeqCst);
            Some(tokio::spawn(async move {
                axum::serve(listener, app).await.ok();
                proxy_flag.store(false, Ordering::SeqCst);
            }))
        }
        Err(e) => {
            eprintln!("[proxy-tauri] ❌ Failed to bind port {}: {}", port, e);
            tracing::error!("Failed to bind port {}: {}", port, e);
            None
        }
    }
}

async fn handle_health() -> impl IntoResponse {
    serde_json::json!({ "status": "ok" }).to_string()
}

async fn handle_responses(
    State(state): State<Arc<AppState>>,
    _headers: HeaderMap,
    body: String,
) -> Response {
    let input: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return (StatusCode::BAD_REQUEST, serde_json::json!({ "error": { "message": e.to_string(), "type": "invalid_request" } }).to_string()).into_response(),
    };

    let model = input.get("model").and_then(|v| v.as_str()).unwrap_or("qwen-plus").to_string();
    let stream = input.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);
    let instructions = input.get("instructions").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let tools = input.get("tools").cloned().unwrap_or(Value::Array(vec![]));

    // Determine actual upstream model BEFORE converting messages
    let upstream_model = {
        let mapped = state.get_upstream_model();
        if mapped.is_empty() { model.clone() } else { mapped }
    };

    // Convert input to messages using the ACTUAL upstream model
    let default_input = Value::String(String::new());
    let input_val = input.get("input").unwrap_or(&default_input);
    let mut messages = convert_input_to_messages(input_val, &upstream_model);

    // Prepend system message with instructions
    if !instructions.is_empty() {
        messages.insert(0, serde_json::json!({"role": "system", "content": instructions}));
    }

    let has_tools = !tools.as_array().map(|a| a.is_empty()).unwrap_or(true);

    if upstream_model != model {
        tracing::info!("[DEBUG] Model mapping: {} -> {}", model, upstream_model);
    }

    let mut upstream_payload = serde_json::json!({
        "model": &upstream_model,
        "messages": messages,
        "stream": false,
    });

    // Map parameters (full mapping like proxy-rs)
    let param_mapping = [
        ("temperature", "temperature"),
        ("max_tokens", "max_tokens"),
        ("max_output_tokens", "max_tokens"),
        ("top_p", "top_p"),
        ("frequency_penalty", "frequency_penalty"),
        ("presence_penalty", "presence_penalty"),
        ("stop", "stop"),
        ("seed", "seed"),
        ("logprobs", "logprobs"),
        ("top_logprobs", "top_logprobs"),
    ];
    for (resp_key, chat_key) in param_mapping.iter() {
        if let Some(val) = input.get(resp_key) {
            upstream_payload[chat_key] = val.clone();
        }
    }

    // Convert tools
    if has_tools {
        let converted_tools = convert_tools_to_chat_format(&tools);
        upstream_payload["tools"] = converted_tools;
    }

    // tool_choice
    if let Some(tc) = input.get("tool_choice") {
        upstream_payload["tool_choice"] = tc.clone();
    }

    // Auto-set max_tokens if tools present but no explicit limit
    if has_tools && upstream_payload.get("max_tokens").is_none() {
        upstream_payload["max_tokens"] = serde_json::json!(32768);
    }

    // Force non-streaming when tools present (DashScope bug)
    let upstream_stream = if has_tools { false } else { stream };
    upstream_payload["stream"] = upstream_stream.into();

    // Build input_detail for history
    let params: Value = {
        let mut p = serde_json::Map::new();
        if let Some(obj) = input.as_object() {
            for (k, v) in obj {
                if !["input", "instructions", "tools", "model", "stream"].contains(&k.as_str()) {
                    p.insert(k.clone(), v.clone());
                }
            }
        }
        Value::Object(p)
    };
    let input_detail = build_input_detail(&instructions, &messages, &tools, &params);

    tracing::info!(
        "\n═══════════════ REQUEST ═══════════════\n\
         Model: {} | Stream: {} | Messages: {} | Tools: {}\n\
         ═══════════════════════════════════════",
        model, stream, messages.len(),
        tools.as_array().map(|a| a.len()).unwrap_or(0),
    );

    let t0 = Instant::now();
    let upstream_url = state.get_upstream_url();
    let api_key = state.get_api_key();

    if upstream_url.is_empty() {
        tracing::error!("No upstream URL configured.");
        return (StatusCode::BAD_GATEWAY,
            [(CONTENT_TYPE, "application/json")],
            serde_json::json!({"error": {"message": "No upstream URL configured", "type": "config_error"}}).to_string()
        ).into_response();
    }

    match state.http_client
        .post(&upstream_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .header(USER_AGENT, "codex-proxy/1.0")
        .json(&upstream_payload)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            let latency = t0.elapsed().as_secs_f64();

            if !status.is_success() {
                let err_body = resp.text().await.unwrap_or_default();
                tracing::error!(
                    "\n═══════════════ UPSTREAM ERROR ═══════════════\n\
                     Status: {} | URL: {} | Latency: {:.2}s\n\
                     Response: {}\n\
                     ══════════════════════════════════════════",
                    status, upstream_url, latency,
                    truncate_utf8(&err_body, 2000),
                );
                state.metrics.record_request(
                    upstream_model.clone(), false, "error".into(), latency, 0, 0,
                    format!("HTTP {}", status.as_u16()),
                    String::new(), None,
                    Some(input_detail),
                );
                return (
                    StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
                    [(CONTENT_TYPE, "application/json")],
                    serde_json::json!({"error": {"message": err_body, "type": "upstream_error"}}).to_string(),
                ).into_response();
            }

            if upstream_stream {
                handle_streaming_response(state, resp, upstream_model.clone(), t0, input_detail).await
            } else if stream {
                // Codex wants streaming but upstream returned non-streaming
                handle_nonstreaming_to_sse(state, resp, upstream_model.clone(), t0, input_detail).await
            } else {
                handle_normal_response(state, resp, upstream_model.clone(), t0, input_detail).await
            }
        }
        Err(e) => {
            let latency = t0.elapsed().as_secs_f64();
            state.metrics.record_request(
                upstream_model, false, "error".into(), latency, 0, 0,
                e.to_string(), String::new(), None,
                Some(input_detail),
            );
            (StatusCode::BAD_GATEWAY,
                [(CONTENT_TYPE, "application/json")],
                serde_json::json!({"error": {"message": e.to_string(), "type": "proxy_error"}}).to_string()
            ).into_response()
        }
    }
}

// ── Normal (non-streaming) Response ─────────────────────────

async fn handle_normal_response(
    state: Arc<AppState>,
    resp: reqwest::Response,
    upstream_model: String,
    t0: Instant,
    input_detail: InputDetail,
) -> Response {
    let raw = resp.text().await.unwrap_or_default();
    let chat_resp: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => {
            let latency = t0.elapsed().as_secs_f64();
            state.metrics.record_request(upstream_model, false, "error".into(), latency, 0, 0, "Invalid JSON".into(), String::new(), None, Some(input_detail));
            return (StatusCode::BAD_GATEWAY, [(CONTENT_TYPE, "application/json")],
                serde_json::json!({"error": {"message": "Invalid upstream response", "type": "proxy_error"}}).to_string()
            ).into_response();
        }
    };

    let latency = t0.elapsed().as_secs_f64();
    let choice = chat_resp.get("choices").and_then(|v| v.as_array()).and_then(|arr| arr.first()).cloned().unwrap_or(Value::Null);
    let msg = choice.get("message").unwrap_or(&Value::Null);
    let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let usage = chat_resp.get("usage").unwrap_or(&Value::Null);
    let in_tok = usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let out_tok = usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);

    state.metrics.record_request(upstream_model.clone(), false, "success".into(), latency, in_tok, out_tok, String::new(), content.clone(), None, Some(input_detail));

    // Also push to live session view
    state.metrics.stream_begin(upstream_model.clone());
    if !content.is_empty() {
        state.metrics.stream_chunk(&content);
    }
    state.metrics.stream_end();

    let resp_id = make_response_id();
    let item_id = make_item_id();
    let created = chrono::Utc::now().timestamp();
    let reasoning_content = msg.get("reasoning_content").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let output_item = build_text_output_item(&item_id, &content, &reasoning_content, &upstream_model);

    let result = serde_json::json!({
        "id": resp_id, "object": "response", "created_at": created,
        "model": &upstream_model, "output": [output_item], "status": "completed",
        "usage": {"input_tokens": in_tok, "output_tokens": out_tok, "total_tokens": in_tok + out_tok},
    });

    (StatusCode::OK, [(CONTENT_TYPE, "application/json")], serde_json::to_string(&result).unwrap()).into_response()
}

// ── Streaming (SSE) Response ─────────────────────────────────

async fn handle_streaming_response(
    state: Arc<AppState>,
    resp: reqwest::Response,
    upstream_model: String,
    t0: Instant,
    input_detail: InputDetail,
) -> Response {
    use futures::StreamExt;

    let resp_id = make_response_id();
    let item_id = make_item_id();
    let created = chrono::Utc::now().timestamp();

    let metrics = state.metrics.clone();
    metrics.stream_begin(upstream_model.clone());

    let upstream_model_clone = upstream_model.clone();
    let stream = async_stream::stream! {
        let mut _cleanup = StreamCleanupGuard::new(metrics.clone());

        yield Ok::<_, Infallible>(axum::response::sse::Event::default()
            .event("response.created")
            .data(serde_json::to_string(&serde_json::json!({
                "type": "response.created", "response": {
                    "id": &resp_id, "object": "response", "created_at": created,
                    "model": &upstream_model, "output": [], "status": "in_progress",
                    "usage": {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0},
                }
            })).unwrap()));

        yield Ok::<_, Infallible>(axum::response::sse::Event::default()
            .event("response.in_progress")
            .data(serde_json::to_string(&serde_json::json!({
                "type": "response.in_progress", "response": {"id": &resp_id, "object": "response", "status": "in_progress"},
            })).unwrap()));

        let mut body = resp.bytes_stream();
        let mut full_text = String::with_capacity(4096);
        let mut reasoning_content = String::new();
        let mut usage_info: Value = Value::Null;
        let mut tool_calls: std::collections::BTreeMap<usize, ToolCallAcc> = std::collections::BTreeMap::new();
        let mut message_events_sent = false;

        while let Some(chunk_result) = body.next().await {
            let chunk = match chunk_result { Ok(c) => c, Err(_) => break };
            let text = String::from_utf8_lossy(&chunk);
            for line in text.lines() {
                let line = line.trim();
                if !line.starts_with("data: ") { continue; }
                let data_str = &line[6..];
                if data_str.trim() == "[DONE]" { break; }
                let chunk_data: Value = match serde_json::from_str(data_str) { Ok(v) => v, Err(_) => continue };

                if let Some(u) = chunk_data.get("usage") { if !u.is_null() { usage_info = u.clone(); } }

                let choices = chunk_data.get("choices").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                if choices.is_empty() { continue; }

                let delta = choices[0].get("delta").unwrap_or(&Value::Null);
                let text_delta = delta.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let reasoning_delta = delta.get("reasoning_content").and_then(|v| v.as_str()).unwrap_or("");

                if !reasoning_delta.is_empty() { reasoning_content.push_str(reasoning_delta); }
                if !text_delta.is_empty() {
                    full_text.push_str(text_delta);
                    metrics.stream_chunk(text_delta);

                    if !message_events_sent {
                        message_events_sent = true;
                        yield Ok::<_, Infallible>(axum::response::sse::Event::default()
                            .event("response.output_item.added")
                            .data(serde_json::to_string(&serde_json::json!({
                                "type": "response.output_item.added", "output_index": 0,
                                "item": {"type": "message", "id": &item_id, "role": "assistant", "status": "in_progress", "content": []},
                            })).unwrap()));
                        yield Ok::<_, Infallible>(axum::response::sse::Event::default()
                            .event("response.content_part.added")
                            .data(serde_json::to_string(&serde_json::json!({
                                "type": "response.content_part.added",
                                "item_id": &item_id, "output_index": 0, "content_index": 0,
                                "part": {"type": "output_text", "text": "", "annotations": []},
                            })).unwrap()));
                    }

                    yield Ok::<_, Infallible>(axum::response::sse::Event::default()
                        .event("response.output_text.delta")
                        .data(serde_json::to_string(&serde_json::json!({
                            "type": "response.output_text.delta",
                            "item_id": &item_id, "output_index": 0, "content_index": 0,
                            "delta": text_delta,
                        })).unwrap()));
                }

                // Accumulate tool calls from stream
                if let Some(tc_delta) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                    for tc in tc_delta {
                        let idx = tc.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                        let acc = tool_calls.entry(idx).or_default();
                        let _name_was_empty = acc.name.is_empty();
                        if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                            if !id.is_empty() { acc.id = id.to_string(); }
                        }
                        if let Some(fn_obj) = tc.get("function") {
                            if let Some(name) = fn_obj.get("name").and_then(|v| v.as_str()) {
                                if !name.is_empty() { acc.name = name.to_string(); }
                            }
                            if let Some(args) = fn_obj.get("arguments").and_then(|v| v.as_str()) {
                                acc.arguments_parts.push(args.to_string());
                            }
                        }
                        // Don't record tool call here - wait until arguments are complete
                    }
                }
            }
        }

        // Finalize text output
        let content_clone = full_text.clone();
        let reasoning_clone = reasoning_content.clone();
        let mut output_items: Vec<Value> = Vec::new();
        let mut next_output_index: u32 = 0;

        if message_events_sent {
            yield Ok::<_, Infallible>(axum::response::sse::Event::default()
                .event("response.content_part.done")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.content_part.done",
                    "item_id": &item_id, "output_index": 0, "content_index": 0,
                    "part": {"type": "output_text", "text": &content_clone, "annotations": []},
                })).unwrap()));

            let ns_text_item = build_text_output_item(&item_id, &content_clone, &reasoning_clone, &upstream_model);
            output_items.push(ns_text_item.clone());

            yield Ok::<_, Infallible>(axum::response::sse::Event::default()
                .event("response.output_item.done")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.output_item.done", "output_index": 0, "item": &ns_text_item,
                })).unwrap()));
            next_output_index = 1;
        }

        // Emit function_call events from accumulated tool calls
        let function_calls: Vec<FunctionCall> = tool_calls.values()
            .filter(|tc| !tc.name.is_empty())
            .map(|acc| FunctionCall {
                id: if acc.id.is_empty() { format!("call_{}", &uuid::Uuid::new_v4().to_string()[..12]) } else { acc.id.clone() },
                name: acc.name.clone(),
                arguments: acc.arguments_parts.join(""),
            })
            .collect();

        // Record tool calls to session with complete arguments
        for fc in &function_calls {
            metrics.stream_tool_call(&fc.name, &fc.arguments);
        }

        for (i, fc) in function_calls.iter().enumerate() {
            let oi = next_output_index + i as u32;
            yield Ok::<_, Infallible>(axum::response::sse::Event::default()
                .event("response.output_item.added")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.output_item.added", "output_index": oi,
                    "item": {"type": "function_call", "id": fc.id, "call_id": fc.id,
                             "name": fc.name, "arguments": fc.arguments, "status": "in_progress"},
                })).unwrap()));

            let fc_item = serde_json::json!({
                "type": "function_call", "id": fc.id, "call_id": fc.id,
                "name": fc.name, "arguments": fc.arguments, "status": "completed",
            });

            yield Ok::<_, Infallible>(axum::response::sse::Event::default()
                .event("response.output_item.done")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.output_item.done", "output_index": oi, "item": &fc_item,
                })).unwrap()));

            output_items.push(fc_item);
        }

        if output_items.is_empty() {
            output_items.push(serde_json::json!({
                "type": "message", "id": &item_id, "role": "assistant", "status": "completed",
                "content": [{"type": "output_text", "text": &content_clone, "annotations": []}],
            }));
        }

        let in_tok = usage_info.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        let out_tok = usage_info.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);

        yield Ok::<_, Infallible>(axum::response::sse::Event::default()
            .event("response.completed")
            .data(serde_json::to_string(&serde_json::json!({
                "type": "response.completed",
                "response": {
                    "id": &resp_id, "object": "response", "created_at": created,
                    "model": &upstream_model, "output": output_items, "status": "completed",
                    "usage": {"input_tokens": in_tok, "output_tokens": out_tok, "total_tokens": in_tok + out_tok},
                },
            })).unwrap()));

        yield Ok::<_, Infallible>(axum::response::sse::Event::default().data("[DONE]"));

        let latency = t0.elapsed().as_secs_f64();
        _cleanup.disarm();
        metrics.stream_end();
        metrics.record_request(
            upstream_model_clone, true, "success".into(), latency,
            in_tok, out_tok, String::new(), full_text.clone(),
            None, Some(input_detail),
        );
    };

    axum::response::Sse::new(stream).into_response()
}

// ── Non-streaming to SSE (Codex wants stream, upstream doesn't) ──

async fn handle_nonstreaming_to_sse(
    state: Arc<AppState>,
    resp: reqwest::Response,
    upstream_model: String,
    t0: Instant,
    input_detail: InputDetail,
) -> Response {
    let raw = resp.text().await.unwrap_or_default();
    let chat_resp: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => {
            let latency = t0.elapsed().as_secs_f64();
            state.metrics.record_request(upstream_model, false, "error".into(), latency, 0, 0, "Invalid JSON".into(), String::new(), None, Some(input_detail));
            return (StatusCode::BAD_GATEWAY, [(CONTENT_TYPE, "application/json")],
                serde_json::json!({"error": {"message": "Invalid upstream response", "type": "proxy_error"}}).to_string()
            ).into_response();
        }
    };

    let latency = t0.elapsed().as_secs_f64();
    let choice = chat_resp.get("choices").and_then(|v| v.as_array()).and_then(|arr| arr.first()).cloned().unwrap_or(Value::Null);
    let msg = choice.get("message").unwrap_or(&Value::Null);
    let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let reasoning_content = msg.get("reasoning_content").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let usage = chat_resp.get("usage").unwrap_or(&Value::Null);
    let in_tok = usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let out_tok = usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);

    let resp_id = make_response_id();
    let item_id = make_item_id();
    let created = chrono::Utc::now().timestamp();

    let raw_tool_calls = msg.get("tool_calls").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let function_calls: Vec<FunctionCall> = raw_tool_calls.iter()
        .filter_map(|tc| {
            let func = tc.get("function")?;
            Some(FunctionCall {
                id: tc.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                name: func.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                arguments: func.get("arguments").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            })
        })
        .collect();

    let metrics = state.metrics.clone();
    metrics.stream_begin(upstream_model.clone());

    let stream = async_stream::stream! {
        let mut _cleanup = StreamCleanupGuard::new(metrics.clone());

        yield Ok::<_, Infallible>(axum::response::sse::Event::default()
            .event("response.created")
            .data(serde_json::to_string(&serde_json::json!({
                "type": "response.created", "response": {
                    "id": &resp_id, "object": "response", "created_at": created,
                    "model": &upstream_model, "output": [], "status": "in_progress",
                    "usage": {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0},
                }
            })).unwrap()));

        yield Ok::<_, Infallible>(axum::response::sse::Event::default()
            .event("response.in_progress")
            .data(serde_json::to_string(&serde_json::json!({
                "type": "response.in_progress", "response": {"id": &resp_id, "object": "response", "status": "in_progress"},
            })).unwrap()));

        let mut output_items: Vec<Value> = Vec::new();
        let mut next_output_index: u32 = 0;

        // Emit text content if present
        let has_function_calls = !function_calls.is_empty();
        let stream_vendor = ModelVendor::from_model_name(&upstream_model);
        if !content.is_empty() || !reasoning_content.is_empty() || (has_function_calls && stream_vendor.requires_reasoning_content()) {
            // Feed content to live stream for dashboard preview
            if !content.is_empty() {
                metrics.stream_chunk(&content);
            }
            yield Ok::<_, Infallible>(axum::response::sse::Event::default()
                .event("response.output_item.added")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.output_item.added", "output_index": 0,
                    "item": {"type": "message", "id": &item_id, "role": "assistant", "status": "in_progress", "content": []},
                })).unwrap()));

            yield Ok::<_, Infallible>(axum::response::sse::Event::default()
                .event("response.content_part.added")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.content_part.added",
                    "item_id": &item_id, "output_index": 0, "content_index": 0,
                    "part": {"type": "output_text", "text": "", "annotations": []},
                })).unwrap()));

            yield Ok::<_, Infallible>(axum::response::sse::Event::default()
                .event("response.output_text.delta")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.output_text.delta",
                    "item_id": &item_id, "output_index": 0, "content_index": 0,
                    "delta": &content,
                })).unwrap()));

            let ns_text_item = build_text_output_item(&item_id, &content, &reasoning_content, &upstream_model);
            output_items.push(ns_text_item.clone());

            yield Ok::<_, Infallible>(axum::response::sse::Event::default()
                .event("response.content_part.done")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.content_part.done",
                    "item_id": &item_id, "output_index": 0, "content_index": 0,
                    "part": {"type": "output_text", "text": &content, "annotations": []},
                })).unwrap()));

            yield Ok::<_, Infallible>(axum::response::sse::Event::default()
                .event("response.output_item.done")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.output_item.done", "output_index": 0, "item": &ns_text_item,
                })).unwrap()));
            next_output_index = 1;
        }

        // Emit function_call events
        for (i, fc) in function_calls.iter().enumerate() {
            let oi = next_output_index + i as u32;
            // Record tool call to session with arguments
            metrics.stream_tool_call(&fc.name, &fc.arguments);
            yield Ok::<_, Infallible>(axum::response::sse::Event::default()
                .event("response.output_item.added")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.output_item.added", "output_index": oi,
                    "item": {"type": "function_call", "id": fc.id, "call_id": fc.id,
                             "name": fc.name, "arguments": fc.arguments, "status": "in_progress"},
                })).unwrap()));

            let fc_item = serde_json::json!({
                "type": "function_call", "id": fc.id, "call_id": fc.id,
                "name": fc.name, "arguments": fc.arguments, "status": "completed",
            });

            yield Ok::<_, Infallible>(axum::response::sse::Event::default()
                .event("response.output_item.done")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.output_item.done", "output_index": oi, "item": &fc_item,
                })).unwrap()));

            output_items.push(fc_item);
        }

        if output_items.is_empty() {
            output_items.push(serde_json::json!({
                "type": "message", "id": &item_id, "role": "assistant", "status": "completed",
                "content": [{"type": "output_text", "text": &content, "annotations": []}],
            }));
        }

        yield Ok::<_, Infallible>(axum::response::sse::Event::default()
            .event("response.completed")
            .data(serde_json::to_string(&serde_json::json!({
                "type": "response.completed",
                "response": {
                    "id": &resp_id, "object": "response", "created_at": created,
                    "model": &upstream_model, "output": output_items, "status": "completed",
                    "usage": {"input_tokens": in_tok, "output_tokens": out_tok, "total_tokens": in_tok + out_tok},
                },
            })).unwrap()));

        yield Ok::<_, Infallible>(axum::response::sse::Event::default().data("[DONE]"));

        _cleanup.disarm();
        metrics.stream_end();
        state.metrics.record_request(
            upstream_model, true, "success".into(), latency,
            in_tok, out_tok, String::new(), content,
            None, Some(input_detail),
        );
    };

    axum::response::Sse::new(stream).into_response()
}
