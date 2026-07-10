use axum::{
    extract::{DefaultBodyLimit, State},
    http::HeaderMap,
    http::header::{CONTENT_TYPE, AUTHORIZATION, USER_AGENT},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde_json::Value;
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tokio::net::TcpListener;
use uuid::Uuid;

use crate::convert::*;
use crate::sse::{make_item_id, make_req_id, make_response_id, truncate_utf8, StreamCleanupGuard, ToolCallAcc, FunctionCall};
use crate::metrics::{InputDetail, SharedMetrics};
// use crate::types::*; // Will be integrated in next phase

/// Kill process occupying the specified port
fn kill_port_occupier(port: u16) {
    use std::process::Command;
    
    // Try lsof first (macOS/Linux)
    if let Ok(output) = Command::new("lsof")
        .args(["-ti", &format!(":{}", port)])
        .output()
    {
        let pids = String::from_utf8_lossy(&output.stdout);
        for pid_str in pids.lines() {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                tracing::info!("Killing process {} occupying port {}", pid, port);
                let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
            }
        }
    }
    // Fallback: try fuser on Linux
    else if let Ok(_output) = Command::new("fuser")
        .args(["-k", &format!("{}/tcp", port)])
        .output()
    {
        tracing::info!("Killed process occupying port {} using fuser", port);
    }
}

pub struct AppState {
    pub metrics: SharedMetrics,
    pub http_client: reqwest::Client,
    pub upstream_url: RwLock<String>,
    pub api_key: RwLock<String>,
    /// Model name to use when forwarding to upstream (may differ from what Codex sends)
    pub upstream_model: RwLock<String>,
    pub config_manager: Arc<crate::config::ConfigManager>,
    pub proxy_running: Arc<AtomicBool>,
    #[allow(dead_code)]
    pub log_level: tracing::Level,
}

impl AppState {
    /// Dynamically update the upstream URL, API key, and model mapping
    pub fn set_upstream(&self, url: String, key: String) {
        // Minimize lock hold time by setting both in sequence
        {
            let mut u = self.upstream_url.write().unwrap();
            *u = url;
        }
        {
            let mut k = self.api_key.write().unwrap();
            *k = key;
        }
        tracing::debug!("Upstream config updated dynamically");
    }

    /// Set the model name to use when forwarding to upstream
    pub fn set_upstream_model(&self, model: String) {
        let mut m = self.upstream_model.write().unwrap();
        *m = model;
        tracing::debug!("Upstream model mapping updated");
    }

    pub fn get_upstream_url(&self) -> String {
        // Use read lock with minimal scope
        self.upstream_url.read().unwrap().clone()
    }

    pub fn get_api_key(&self) -> String {
        self.api_key.read().unwrap().clone()
    }

    /// Get the model name to use for upstream requests.
    /// Returns the mapped model if set, otherwise returns the original model.
    pub fn get_upstream_model(&self) -> String {
        self.upstream_model.read().unwrap().clone()
    }

    pub fn should_log_detail(&self) -> bool {
        self.log_level <= tracing::Level::DEBUG
    }
}

pub async fn run_server(state: Arc<AppState>) -> Arc<AppState> {
    // Extract flag before state is moved into Router
    let proxy_flag = state.proxy_running.clone();

    let app = Router::new()
        .route("/v1/responses", post(handle_responses))
        .route("/health", get(handle_health))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024)) // 50MB body limit
        .with_state(state.clone());

    // Try to bind port, kill occupying process if needed
    let listener = match TcpListener::bind("127.0.0.1:8000").await {
        Ok(l) => l,
        Err(_) => {
            tracing::warn!("Port 8000 is occupied, attempting to free it...");
            kill_port_occupier(8000);
            // Wait a moment for port to be released
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            // Retry binding
            match TcpListener::bind("127.0.0.1:8000").await {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!("Failed to bind port 8000 after cleanup: {}", e);
                    return state;
                }
            }
        }
    };
    proxy_flag.store(true, Ordering::SeqCst);
    tracing::info!("Proxy server listening on http://127.0.0.1:8000");

    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!("Proxy server error: {}", e);
        proxy_flag.store(false, Ordering::SeqCst);
    }

    state
}

async fn handle_health() -> &'static str {
    "OK"
}



// ── Proxy Handler ───────────────────────────────────────────

async fn handle_responses(
    State(state): State<Arc<AppState>>,
    _headers: HeaderMap,
    body: String,
) -> Response {
    let body: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                serde_json::json!({"error": {"message": format!("Invalid JSON: {}", e), "type": "invalid_request"}}).to_string(),
            )
                .into_response();
        }
    };

    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("qwen-plus")
        .to_string();

    let stream = body
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let instructions = body
        .get("instructions")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let tools = body.get("tools").cloned().unwrap_or(Value::Array(vec![]));

    // Convert input to messages
    let default_input = Value::String(String::new());
    let input = body.get("input").unwrap_or(&default_input);
    let mut messages = convert_input_to_messages(input);

    // Prepend system message with instructions
    if !instructions.is_empty() {
        messages.insert(0, serde_json::json!({"role": "system", "content": instructions}));
    }

    let has_tools = !tools.as_array().map(|a| a.is_empty()).unwrap_or(true);

    // Build upstream payload
    // Use the configured upstream model if available, otherwise use the model from the request.
    // This ensures the user's model selection in the config center takes effect.
    let upstream_model = {
        let mapped = state.get_upstream_model();
        if mapped.is_empty() { 
            model.clone() 
        } else {
            // Log model mapping when it differs from request model
            if mapped != model {
                tracing::info!("[DEBUG] Model mapping: {} -> {}", model, mapped);
            }
            mapped
        }
    };
    let mut upstream_payload = serde_json::json!({
        "model": &upstream_model,
        "messages": messages,
        "stream": false, // will set after logic below
    });

    // Map parameters
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
        if let Some(val) = body.get(resp_key) {
            upstream_payload[chat_key] = val.clone();
        }
    }

    // Convert tools
    if has_tools {
        let converted_tools = convert_tools_to_chat_format(&tools);
        upstream_payload["tools"] = converted_tools;
    }

    // tool_choice
    if let Some(tc) = body.get("tool_choice") {
        upstream_payload["tool_choice"] = tc.clone();
    }

    // Auto-set max_tokens if tools present but no explicit limit
    if has_tools && !upstream_payload.get("max_tokens").is_some() {
        upstream_payload["max_tokens"] = serde_json::json!(32768);
    }

    // Let the model handle thinking mode by default.
    // Qwen3 thinking consumes token budget; Codex is already a reasoning framework.

    // Force non-streaming when tools present (DashScope bug)
    let upstream_stream = if has_tools { false } else { stream };
    upstream_payload["stream"] = upstream_stream.into();

    // Build input_detail for history
    let params_filtered = serde_json::json!({});
    if let Some(_obj) = params_filtered.as_object() {
        // We'll build the actual params in a moment
    }
    let params: Value = {
        let mut p = serde_json::Map::new();
        for (k, v) in body.as_object().unwrap() {
            if !["input", "instructions", "tools", "model", "stream"].contains(&k.as_str()) {
                p.insert(k.clone(), v.clone());
            }
        }
        Value::Object(p)
    };

    let input_detail = build_input_detail(
        &instructions,
        &messages,
        &tools,
        &params,
    );

    tracing::info!(
        "\n═══════════════ REQUEST ═══════════════\n\
         Model: {} | Stream: {} | Messages: {} | Tools: {}\n\
         ── Instructions ──\n{}\n\
         ── Messages ──\n{}\n\
         ═══════════════════════════════════════",
        model,
        stream,
        messages.len(),
        tools.as_array().map(|a| a.len()).unwrap_or(0),
        if instructions.is_empty() { "(none)".to_string() } else { instructions.chars().take(500).collect::<String>() },
        serde_json::to_string_pretty(&messages).unwrap_or_default().chars().take(2000).collect::<String>(),
    );

    let t0 = Instant::now();

    // Read upstream URL and API key dynamically from AppState
    let upstream_url = state.get_upstream_url();
    let api_key = state.get_api_key();

    if upstream_url.is_empty() {
        tracing::error!("No upstream URL configured. Please apply a config first.");
        return (
            StatusCode::BAD_GATEWAY,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            serde_json::json!({"error": {"message": "No upstream configured. Please apply a config in the proxy monitor.", "type": "config_error"}}).to_string(),
        )
            .into_response();
    }

    tracing::debug!("Forwarding to upstream: {} (key: {}...)", upstream_url, &api_key[..api_key.len().min(8)]);

    // Log the exact payload being sent upstream (for debugging)
    let payload_str = serde_json::to_string(&upstream_payload).unwrap_or_default();
    tracing::info!(
        "\n═══════════════ UPSTREAM REQUEST ═══════════════\n\
         URL: {}\n\
         Model (requested): {}\n\
         Model (in payload): {}\n\
         Payload size: {} bytes\n\
         Payload: {}\n\
         ══════════════════════════════════════════",
        upstream_url,
        model,
        upstream_payload.get("model").and_then(|v| v.as_str()).unwrap_or("?"),
        payload_str.len(),
        truncate_utf8(&payload_str, 3000),
    );

    // Build HTTP request to upstream using shared client
    let upstream_req = state.http_client
        .post(&upstream_url)
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .header(USER_AGENT, "codex-proxy/1.0")
        .body(payload_str.clone());

    match upstream_req.send().await {
        Ok(resp) => {
            let status = resp.status();
            if !status.is_success() {
                let err_body = resp.text().await.unwrap_or_default();
                let latency = t0.elapsed().as_secs_f64();
                tracing::error!(
                    "\n═══════════════ UPSTREAM ERROR ═══════════════\n\
                     Status: {}\n\
                     URL: {}\n\
                     Latency: {:.2}s\n\
                     Response body: {}\n\
                     ══════════════════════════════════════════",
                    status, upstream_url, latency,
                    truncate_utf8(&err_body, 2000),
                );
                state.metrics.record_request(
                    model, stream, "error".into(), latency, 0, 0,
                    format!("HTTP {}", status.as_u16()),
                    String::new(),
                    None,
                    Some(input_detail),
                );
                return (
                    StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    serde_json::json!({"error": {"message": err_body, "type": "upstream_error"}}).to_string(),
                )
                    .into_response();
            }

            if upstream_stream {
                handle_streaming_response(state, resp, model, t0, input_detail).await
            } else if stream {
                // Codex wants streaming but upstream returned non-streaming
                handle_nonstreaming_to_sse(state, resp, model, t0, input_detail).await
            } else {
                handle_normal_response(state, resp, model, t0, input_detail).await
            }
        }
        Err(e) => {
            let latency = t0.elapsed().as_secs_f64();
            state.metrics.record_request(
                model, stream, "error".into(), latency, 0, 0,
                e.to_string(),
                String::new(),
                None,
                Some(input_detail),
            );
            (
                StatusCode::BAD_GATEWAY,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                serde_json::json!({"error": {"message": e.to_string(), "type": "proxy_error"}}).to_string(),
            )
                .into_response()
        }
    }
}

// ── Normal (non-streaming) Response ─────────────────────────

async fn handle_normal_response(
    state: Arc<AppState>,
    resp: reqwest::Response,
    model: String,
    t0: Instant,
    input_detail: InputDetail,
) -> Response {
    let raw = resp.text().await.unwrap_or_default();
    let chat_resp: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => {
            let latency = t0.elapsed().as_secs_f64();
            state.metrics.record_request(
                model, false, "error".into(), latency, 0, 0,
                "Invalid JSON".into(), String::new(), None, Some(input_detail),
            );
            return (
                StatusCode::BAD_GATEWAY,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                serde_json::json!({"error": {"message": "Invalid upstream response", "type": "proxy_error"}}).to_string(),
            )
                .into_response();
        }
    };

    let latency = t0.elapsed().as_secs_f64();
    let choice = chat_resp.get("choices")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .cloned()
        .unwrap_or(Value::Null);

    let msg = choice.get("message").unwrap_or(&Value::Null);
    let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let usage = chat_resp.get("usage").unwrap_or(&Value::Null);
    let in_tok = usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let out_tok = usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);

    tracing::info!(
        "\n═══════════════ RESPONSE ═══════════════\n\
         Model: {} | Latency: {:.2}s | Tokens: {}/{} (in/out)\n\
         ── Content ──\n{}\n\
         ═══════════════════════════════════════",
        model,
        latency,
        in_tok,
        out_tok,
        if content.is_empty() { "(empty)".to_string() } else { content.chars().take(2000).collect::<String>() },
    );

    state.metrics.record_request(
        model.clone(), false, "success".into(), latency,
        in_tok, out_tok, String::new(), content.clone(),
        None,
        Some(input_detail),
    );

    let resp_id = make_response_id();
    let item_id = make_item_id();
    let created = chrono::Utc::now().timestamp();

    let actual_model = {
        let m = state.get_upstream_model();
        if m.is_empty() { model.clone() } else { m }
    };

    let reasoning_content = msg.get("reasoning_content").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let result = serde_json::json!({
        "id": resp_id,
        "object": "response",
        "created_at": created,
        "model": actual_model,
        "output": [{
            "type": "message",
            "id": item_id,
            "role": "assistant",
            "status": "completed",
            "content": [{"type": "output_text", "text": content, "annotations": []}],
            "reasoning_content": reasoning_content,
        }],
        "status": "completed",
        "usage": {
            "input_tokens": in_tok,
            "output_tokens": out_tok,
            "total_tokens": in_tok + out_tok,
        },
    });

    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&result).unwrap(),
    )
        .into_response()
}

// ── Streaming (SSE) Response ─────────────────────────────────

async fn handle_streaming_response(
    state: Arc<AppState>,
    resp: reqwest::Response,
    model: String,
    t0: Instant,
    input_detail: InputDetail,
) -> Response {
    use futures::StreamExt;

    let resp_id = make_response_id();
    let item_id = make_item_id();
    let created = chrono::Utc::now().timestamp();

    let actual_model = {
        let m = state.get_upstream_model();
        if m.is_empty() { model.clone() } else { m }
    };

    let req_id = make_req_id();
    state.metrics.stream_start_with_model(&actual_model, req_id.clone());

    let metrics = state.metrics.clone();
    let model_clone = model.clone();
    let req_id_clone = req_id.clone();
    let stream = async_stream::stream! {
        // Drop guard：确保客户端提前断开时也能清理流状态
        let mut _cleanup = StreamCleanupGuard::new(metrics.clone());

        // response.created
        yield Ok::<_, Infallible>(axum::response::sse::Event::default()
            .event("response.created")
            .data(serde_json::to_string(&serde_json::json!({
                "type": "response.created",
                "response": {
                    "id": &resp_id,
                    "object": "response",
                    "created_at": created,
                    "model": &actual_model,
                    "output": [],
                    "status": "in_progress",
                    "usage": {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0},
                }
            })).unwrap()));

        // response.in_progress
        yield Ok(axum::response::sse::Event::default()
            .event("response.in_progress")
            .data(serde_json::to_string(&serde_json::json!({
                "type": "response.in_progress",
                "response": {"id": &resp_id, "object": "response", "status": "in_progress"},
            })).unwrap()));

        let mut body = resp.bytes_stream();
        // Pre-allocate String with reasonable capacity to reduce reallocations
        let mut full_text = String::with_capacity(4096);
        let mut reasoning_content = String::new();
        let mut usage_info: Value = Value::Null;
        let mut tool_calls: std::collections::BTreeMap<usize, ToolCallAcc> = std::collections::BTreeMap::new();
        let mut message_events_sent = false;

        while let Some(chunk_result) = body.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(_) => break,
            };
            let text = String::from_utf8_lossy(&chunk);
            for line in text.lines() {
                let line = line.trim();
                if !line.starts_with("data: ") {
                    continue;
                }
                let data_str = &line[6..];
                if data_str.trim() == "[DONE]" {
                    break;
                }
                let chunk_data: Value = match serde_json::from_str(data_str) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                // Capture non-null usage
                if let Some(u) = chunk_data.get("usage") {
                    if !u.is_null() {
                        usage_info = u.clone();
                    }
                }

                let choices = chunk_data.get("choices")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();

                if choices.is_empty() {
                    continue;
                }

                let delta = choices[0].get("delta").unwrap_or(&Value::Null);
                let text_delta = delta.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let reasoning_delta = delta.get("reasoning_content").and_then(|v| v.as_str()).unwrap_or("");

                if !reasoning_delta.is_empty() {
                    reasoning_content.push_str(reasoning_delta);
                }
                if !text_delta.is_empty() {
                    full_text.push_str(text_delta);
                    metrics.stream_append(text_delta);

                    if !message_events_sent {
                        message_events_sent = true;
                        let event = axum::response::sse::Event::default()
                            .event("response.output_item.added")
                            .data(serde_json::to_string(&serde_json::json!({
                                "type": "response.output_item.added",
                                "output_index": 0,
                                "item": {
                                    "type": "message",
                                    "id": &item_id,
                                    "role": "assistant",
                                    "status": "in_progress",
                                    "content": [],
                                }
                            })).unwrap());
                        yield Ok(event);

                        let event = axum::response::sse::Event::default()
                            .event("response.content_part.added")
                            .data(serde_json::to_string(&serde_json::json!({
                                "type": "response.content_part.added",
                                "item_id": &item_id,
                                "output_index": 0,
                                "content_index": 0,
                                "part": {"type": "output_text", "text": "", "annotations": []},
                            })).unwrap());
                        yield Ok(event);
                    }

                    let event = axum::response::sse::Event::default()
                        .event("response.output_text.delta")
                        .data(serde_json::to_string(&serde_json::json!({
                            "type": "response.output_text.delta",
                            "item_id": &item_id,
                            "output_index": 0,
                            "content_index": 0,
                            "delta": text_delta,
                        })).unwrap());
                    yield Ok(event);
                }

                // Handle tool_calls deltas
                let tc_list = delta.get("tool_calls").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                for tc in &tc_list {
                    let idx = tc.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    let entry = tool_calls.entry(idx).or_insert(ToolCallAcc {
                        id: String::new(),
                        name: String::new(),
                        arguments_parts: Vec::new(),
                    });
                    if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                        if !id.is_empty() { entry.id = id.to_string(); }
                    }
                    if let Some(func) = tc.get("function") {
                        if let Some(name) = func.get("name").and_then(|v| v.as_str()) {
                            if !name.is_empty() { entry.name = name.to_string(); }
                        }
                        if let Some(args) = func.get("arguments").and_then(|v| v.as_str()) {
                            entry.arguments_parts.push(args.to_string());
                        }
                    }
                }
            }
        }

        let latency = t0.elapsed().as_secs_f64();
        let in_tok = usage_info.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        let mut out_tok = usage_info.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);

        // Build function calls
        let function_calls: Vec<FunctionCall> = tool_calls.values()
            .filter(|tc| !tc.name.is_empty())
            .map(|tc| FunctionCall {
                id: if tc.id.is_empty() { format!("call_{}", &Uuid::new_v4().to_string()[..12]) } else { tc.id.clone() },
                name: tc.name.clone(),
                arguments: tc.arguments_parts.join(""),
            })
            .collect();

        // Estimate tokens if missing (using chars count for accuracy)
        if out_tok == 0 && (!full_text.is_empty() || !function_calls.is_empty()) {
            let text_chars = full_text.chars().count();
            let args_chars: usize = function_calls.iter().map(|fc| fc.arguments.chars().count()).sum();
            let total_chars = text_chars + args_chars;
            out_tok = std::cmp::max(1, total_chars as u64 / 3);
        }

        // Build output items
        let mut output_items: Vec<Value> = Vec::new();
        let mut next_output_index = 0u32;

        if !full_text.is_empty() {
            // content_part.done
            yield Ok(axum::response::sse::Event::default()
                .event("response.content_part.done")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.content_part.done",
                    "item_id": &item_id,
                    "output_index": 0,
                    "content_index": 0,
                    "part": {"type": "output_text", "text": &full_text, "annotations": []},
                })).unwrap()));

            // output_item.done (text message)
            yield Ok(axum::response::sse::Event::default()
                .event("response.output_item.done")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.output_item.done",
                    "output_index": 0,
                    "item": {
                        "type": "message",
                        "id": &item_id,
                        "role": "assistant",
                        "status": "completed",
                        "content": [{"type": "output_text", "text": &full_text, "annotations": []}],
                        "reasoning_content": &reasoning_content,
                    },
                })).unwrap()));

            output_items.push(serde_json::json!({
                "type": "message",
                "id": &item_id,
                "role": "assistant",
                "status": "completed",
                "content": [{"type": "output_text", "text": &full_text, "annotations": []}],
                "reasoning_content": &reasoning_content,
            }));
            next_output_index = 1;
        }

        // Emit function_call events
        for (i, fc) in function_calls.iter().enumerate() {
            let oi = next_output_index + i as u32;
            let fc_item = serde_json::json!({
                "type": "function_call",
                "id": fc.id,
                "call_id": fc.id,
                "name": fc.name,
                "arguments": fc.arguments,
                "status": "completed",
            });

            // output_item.added
            yield Ok(axum::response::sse::Event::default()
                .event("response.output_item.added")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.output_item.added",
                    "output_index": oi,
                    "item": {
                        "type": "function_call",
                        "id": fc.id,
                        "call_id": fc.id,
                        "name": fc.name,
                        "arguments": fc.arguments,
                        "status": "in_progress",
                    },
                })).unwrap()));

            // output_item.done
            yield Ok(axum::response::sse::Event::default()
                .event("response.output_item.done")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.output_item.done",
                    "output_index": oi,
                    "item": &fc_item,
                })).unwrap()));

            output_items.push(fc_item);
        }

        // Ensure at least one output item
        if output_items.is_empty() {
            output_items.push(serde_json::json!({
                "type": "message",
                "id": &item_id,
                "role": "assistant",
                "status": "completed",
                "content": [{"type": "output_text", "text": &full_text, "annotations": []}],
                "reasoning_content": &reasoning_content,
            }));
        }

        // response.completed
        yield Ok(axum::response::sse::Event::default()
            .event("response.completed")
            .data(serde_json::to_string(&serde_json::json!({
                "type": "response.completed",
                "response": {
                    "id": &resp_id,
                    "object": "response",
                    "created_at": created,
                    "model": &actual_model,
                    "output": output_items,
                    "status": "completed",
                    "usage": {
                        "input_tokens": in_tok,
                        "output_tokens": out_tok,
                        "total_tokens": in_tok + out_tok,
                    },
                },
            })).unwrap()));

        // [DONE]
        yield Ok(axum::response::sse::Event::default().data("[DONE]"));

        // Record metrics
        _cleanup.disarm();  // ponytail: 正常路径结束时先 disarm，避免 Drop 重复调用 stream_end
        metrics.stream_end();
        metrics.record_request(
            model_clone.clone(), true, "success".into(), latency,
            in_tok, out_tok, String::new(), full_text.clone(),
            Some(req_id_clone.clone()),
            Some(input_detail),
        );

        tracing::info!(
            "\n═══════════════ RESPONSE (stream) ═══════════════\n\
             Model: {} | Latency: {:.2}s | Tokens: {}/{} (in/out)\n\
             ── Content ──\n{}\n\
             ═══════════════════════════════════════════════════",
            model_clone,
            latency,
            in_tok,
            out_tok,
            if full_text.is_empty() { "(empty)".to_string() } else { full_text.chars().take(2000).collect::<String>() },
        );
    };

    axum::response::Sse::new(stream).into_response()
}

// ── Non-streaming to SSE conversion ─────────────────────────

async fn handle_nonstreaming_to_sse(
    state: Arc<AppState>,
    resp: reqwest::Response,
    model: String,
    t0: Instant,
    input_detail: InputDetail,
) -> Response {
    let raw = resp.text().await.unwrap_or_default();
    let chat_resp: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => {
            let latency = t0.elapsed().as_secs_f64();
            state.metrics.record_request(
                model, true, "error".into(), latency, 0, 0,
                "Invalid JSON".into(), String::new(), None, Some(input_detail),
            );
            return (
                StatusCode::BAD_GATEWAY,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                serde_json::json!({"error": {"message": "Invalid upstream response", "type": "proxy_error"}}).to_string(),
            )
                .into_response();
        }
    };

    let resp_id = make_response_id();
    let item_id = make_item_id();
    let created = chrono::Utc::now().timestamp();

    let actual_model = {
        let m = state.get_upstream_model();
        if m.is_empty() { model.clone() } else { m }
    };
    let req_id = make_req_id();
    state.metrics.stream_start_with_model(&actual_model, req_id.clone());

    let choice = chat_resp.get("choices")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .cloned()
        .unwrap_or(Value::Null);

    let msg = choice.get("message").unwrap_or(&Value::Null);
    let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let reasoning_content = msg.get("reasoning_content").and_then(|v| v.as_str()).unwrap_or("").to_string();
    state.metrics.stream_append(&content);
    let usage = chat_resp.get("usage").unwrap_or(&Value::Null);
    let in_tok = usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let mut out_tok = usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);

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

    if out_tok == 0 && (!content.is_empty() || !function_calls.is_empty()) {
        let total_chars = content.len() + function_calls.iter().map(|fc| fc.arguments.len()).sum::<usize>();
        out_tok = std::cmp::max(1, total_chars as u64 / 3);
    }

    let model_clone = model.clone();
    let content_clone = content.clone();
    let reasoning_clone = reasoning_content.clone();
    let metrics = state.metrics.clone();
    let stream = async_stream::stream! {
        // ponytail: 客户端提前断开时自动清理流状态
        let mut _cleanup = StreamCleanupGuard::new(metrics.clone());

        // response.created
        yield Ok::<_, Infallible>(axum::response::sse::Event::default()
            .event("response.created")
            .data(serde_json::to_string(&serde_json::json!({
                "type": "response.created",
                "response": {
                    "id": &resp_id, "object": "response", "created_at": created,
                    "model": &actual_model, "output": [], "status": "in_progress",
                    "usage": {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0},
                }
            })).unwrap()));

        // response.in_progress
        yield Ok(axum::response::sse::Event::default()
            .event("response.in_progress")
            .data(serde_json::to_string(&serde_json::json!({
                "type": "response.in_progress",
                "response": {"id": &resp_id, "object": "response", "status": "in_progress"},
            })).unwrap()));

        let mut output_items: Vec<Value> = Vec::new();
        let mut next_output_index = 0u32;

        if !content_clone.is_empty() {
            yield Ok(axum::response::sse::Event::default()
                .event("response.output_item.added")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.output_item.added", "output_index": 0,
                    "item": {"type": "message", "id": &item_id, "role": "assistant",
                             "status": "in_progress", "content": []},
                })).unwrap()));

            yield Ok(axum::response::sse::Event::default()
                .event("response.content_part.added")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.content_part.added",
                    "item_id": &item_id, "output_index": 0, "content_index": 0,
                    "part": {"type": "output_text", "text": "", "annotations": []},
                })).unwrap()));

            yield Ok(axum::response::sse::Event::default()
                .event("response.output_text.delta")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.output_text.delta",
                    "item_id": &item_id, "output_index": 0, "content_index": 0,
                    "delta": &content_clone,
                })).unwrap()));

            output_items.push(serde_json::json!({
                "type": "message", "id": &item_id, "role": "assistant",
                "status": "completed",
                "content": [{"type": "output_text", "text": &content_clone, "annotations": []}],
                "reasoning_content": &reasoning_clone,
            }));

            yield Ok(axum::response::sse::Event::default()
                .event("response.content_part.done")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.content_part.done",
                    "item_id": &item_id, "output_index": 0, "content_index": 0,
                    "part": {"type": "output_text", "text": &content_clone, "annotations": []},
                })).unwrap()));

            yield Ok(axum::response::sse::Event::default()
                .event("response.output_item.done")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.output_item.done", "output_index": 0,
                    "item": {"type": "message", "id": &item_id, "role": "assistant",
                             "status": "completed",
                             "content": [{"type": "output_text", "text": &content_clone, "annotations": []}],
                             "reasoning_content": &reasoning_clone},
                })).unwrap()));
            next_output_index = 1;
        }

        // Emit function_call events
        for (i, fc) in function_calls.iter().enumerate() {
            let oi = next_output_index + i as u32;
            yield Ok(axum::response::sse::Event::default()
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

            yield Ok(axum::response::sse::Event::default()
                .event("response.output_item.done")
                .data(serde_json::to_string(&serde_json::json!({
                    "type": "response.output_item.done", "output_index": oi,
                    "item": &fc_item,
                })).unwrap()));

            output_items.push(fc_item);
        }

        if output_items.is_empty() {
            output_items.push(serde_json::json!({
                "type": "message", "id": &item_id, "role": "assistant",
                "status": "completed",
                "content": [{"type": "output_text", "text": &content_clone, "annotations": []}],
            }));
        }

        yield Ok(axum::response::sse::Event::default()
            .event("response.completed")
            .data(serde_json::to_string(&serde_json::json!({
                "type": "response.completed",
                "response": {
                    "id": &resp_id, "object": "response", "created_at": created,
                    "model": &actual_model, "output": output_items,
                    "status": "completed",
                    "usage": {"input_tokens": in_tok, "output_tokens": out_tok,
                              "total_tokens": in_tok + out_tok},
                },
            })).unwrap()));

        yield Ok(axum::response::sse::Event::default().data("[DONE]"));

        let latency = t0.elapsed().as_secs_f64();
        _cleanup.disarm();  // ponytail: 正常路径结束时先 disarm，避免 Drop 重复调用 stream_end
        metrics.stream_end();
        metrics.record_request(
            model_clone, true, "success".into(), latency,
            in_tok, out_tok, String::new(), content_clone,
            Some(req_id.clone()),
            Some(input_detail),
        );
    };

    axum::response::Sse::new(stream).into_response()
}
