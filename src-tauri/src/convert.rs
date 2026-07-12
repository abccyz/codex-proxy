use crate::model::ModelVendor;
use crate::sse::FunctionCall;
use serde_json::Value;

fn map_role(role: &str) -> &str {
    match role { "developer" => "system", other => other }
}

pub fn extract_text_from_content(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(items) => {
            items.iter().filter_map(|item| item.as_object())
                .filter(|obj| obj.get("type").and_then(|v| v.as_str()) != Some("reasoning"))
                .filter_map(|obj| {
                    if let Some(t) = obj.get("text").and_then(|v| v.as_str()) { return Some(t.to_string()); }
                    if let Some(t) = obj.get("input_text").and_then(|v| v.get("text")).and_then(|v| v.as_str()) { return Some(t.to_string()); }
                    if let Some(c) = obj.get("content") { return Some(c.as_str().unwrap_or("").to_string()); }
                    None
                }).collect::<Vec<_>>().join("\n")
        }
        other => other.as_str().unwrap_or("").to_string(),
    }
}

fn extract_reasoning_from_content(content: &Value) -> Option<String> {
    content.as_array().and_then(|items| {
        items.iter().find(|item| item.get("type").and_then(|v| v.as_str()) == Some("reasoning"))
            .and_then(|r| r.get("text").and_then(|v| v.as_str()).map(|s| s.to_string()))
    })
}

fn convert_single_tool(tool: &Value) -> Option<Value> {
    let obj = tool.as_object()?;
    if obj.contains_key("function") { return Some(tool.clone()); }
    let tool_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if tool_type == "web_search" { return None; }
    if tool_type == "namespace" {
        if let Some(nested) = obj.get("tools").and_then(|v| v.as_array()) {
            return Some(Value::Array(nested.iter().filter_map(|t| convert_single_tool(t)).collect()));
        }
        return None;
    }
    if obj.contains_key("name") && obj.contains_key("parameters") {
        let description = obj.get("description").and_then(|v| v.as_str()).unwrap_or("");
        return Some(serde_json::json!({
            "type": "function", "function": {
                "name": obj.get("name").cloned()?, "description": description,
                "parameters": obj.get("parameters").cloned().unwrap_or(serde_json::json!({}))
            }
        }));
    }
    if obj.contains_key("name") && !obj.contains_key("function") {
        let has_input_schema = obj.contains_key("inputSchema");
        let has_description = obj.contains_key("description");
        if has_input_schema || has_description {
            let description = obj.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let params = if has_input_schema {
                obj.get("inputSchema").cloned().unwrap_or(serde_json::json!({}))
            } else { serde_json::json!({}) };
            return Some(serde_json::json!({
                "type": "function", "function": {
                    "name": obj.get("name").cloned()?, "description": description, "parameters": params
                }
            }));
        }
    }
    Some(tool.clone())
}

fn extract_tool_name(v: &Value) -> Option<String> {
    v.get("function").and_then(|f| f.get("name")).and_then(|v| v.as_str())
        .or_else(|| v.get("name").and_then(|v| v.as_str())).map(|s| s.to_string())
}

pub fn convert_tools_to_chat_format(tools: &Value) -> Value {
    let arr = match tools.as_array() { Some(a) => a, None => return tools.clone() };
    let mut seen = std::collections::HashSet::new();
    let mut deduped: Vec<Value> = Vec::new();
    for tool in arr.iter().filter_map(|t| convert_single_tool(t)) {
        if tool.is_array() {
            for inner in tool.as_array().unwrap() {
                if let Some(name) = extract_tool_name(inner) {
                    if seen.insert(name) { deduped.push(inner.clone()); }
                }
            }
        } else if let Some(name) = extract_tool_name(&tool) {
            if seen.insert(name) { deduped.push(tool); }
        }
    }
    Value::Array(deduped)
}

/// Convert Responses API input to Chat Completions messages.
/// Handles string/array input, reasoning_content, tool calls, and role validation.
pub fn convert_input_to_messages(input: &Value, upstream_model: &str) -> Vec<Value> {
    let vendor = ModelVendor::from_model_name(upstream_model);
    let mut messages: Vec<Value> = Vec::new();

    let items: Vec<Value> = match input {
        Value::String(s) => {
            messages.push(serde_json::json!({"role": "user", "content": s}));
            return messages;
        }
        Value::Array(arr) => arr.clone(),
        _ => return messages,
    };

    // Track which tool_call_ids we've seen to deduplicate
    let mut seen_tool_calls: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    let mut removed_call_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    // First pass: collect all tool call IDs to detect duplicates
    for item in &items {
        let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("");
        if role == "assistant" {
            if let Some(content) = item.get("content").and_then(|v| v.as_array()) {
                for part in content {
                    if part.get("type").and_then(|v| v.as_str()) == Some("function_call") {
                        let name = part.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let args = part.get("arguments").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let key = (name.clone(), args.clone());
                        if !seen_tool_calls.insert(key) {
                            if let Some(id) = part.get("id").or_else(|| part.get("call_id")).and_then(|v| v.as_str()) {
                                removed_call_ids.insert(id.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // Second pass: build messages
    for item in &items {
        let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("");
        let mapped_role = map_role(role);

        // Skip items with invalid/empty roles
        if mapped_role.is_empty() || !["system", "assistant", "user", "tool", "function"].contains(&mapped_role) {
            continue;
        }

        match mapped_role {
            "assistant" => {
                let content = item.get("content").map(|c| extract_text_from_content(c)).unwrap_or_default();
                let reasoning = item.get("content").and_then(|c| extract_reasoning_from_content(c));

                let mut msg = serde_json::json!({"role": "assistant", "content": content});

                // Add reasoning_content if vendor requires it
                if let Some(reasoning_val) = vendor.build_reasoning_input(reasoning.as_deref()) {
                    msg.as_object_mut().unwrap().insert("reasoning_content".to_string(), Value::String(reasoning_val));
                }

                // Extract tool calls from content array
                if let Some(content_arr) = item.get("content").and_then(|v| v.as_array()) {
                    let mut tool_calls_arr: Vec<Value> = Vec::new();
                    for part in content_arr {
                        if part.get("type").and_then(|v| v.as_str()) == Some("function_call") {
                            let name = part.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let args = part.get("arguments").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let id = part.get("id").or_else(|| part.get("call_id")).and_then(|v| v.as_str()).unwrap_or("").to_string();

                            // Skip if this is a duplicate that was removed
                            if removed_call_ids.contains(&id) { continue; }

                            let key = (name.clone(), args.clone());
                            if seen_tool_calls.contains(&key) {
                                tool_calls_arr.push(serde_json::json!({
                                    "id": id, "type": "function",
                                    "function": {"name": name, "arguments": args}
                                }));
                            }
                        }
                    }
                    if !tool_calls_arr.is_empty() {
                        msg.as_object_mut().unwrap().insert("tool_calls".to_string(), Value::Array(tool_calls_arr));
                    }
                }

                messages.push(msg);
            }
            "tool" => {
                let call_id = item.get("call_id").and_then(|v| v.as_str()).unwrap_or("");
                if removed_call_ids.contains(call_id) { continue; }
                let content = item.get("content").map(|c| extract_text_from_content(c)).unwrap_or_default();
                messages.push(serde_json::json!({"role": "tool", "content": content, "tool_call_id": call_id}));
            }
            _ => {
                let content = item.get("content").map(|c| extract_text_from_content(c)).unwrap_or_default();
                messages.push(serde_json::json!({"role": mapped_role, "content": content}));
            }
        }
    }

    messages
}

pub fn convert_chat_message_to_output(msg: &Value) -> (Vec<Value>, bool, String, Vec<FunctionCall>) {
    let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let tool_calls = msg.get("tool_calls").and_then(|v| v.as_array());
    let mut output_items = Vec::new();
    let mut function_calls = Vec::new();
    let has_text = !content.is_empty();
    if has_text {
        output_items.push(serde_json::json!({
            "type": "message", "role": "assistant", "status": "completed",
            "content": [{"type": "output_text", "text": content, "annotations": []}]
        }));
    }
    if let Some(tc_arr) = tool_calls {
        for tc in tc_arr {
            if let Some(func) = tc.get("function").and_then(|v| v.as_object()) {
                let name = func.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let arguments = func.get("arguments").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                function_calls.push(FunctionCall { id: id.clone(), name: name.clone(), arguments: arguments.clone() });
                output_items.push(serde_json::json!({
                    "type": "function_call", "id": id, "call_id": id, "name": name, "arguments": arguments,
                }));
            }
        }
    }
    if output_items.is_empty() {
        output_items.push(serde_json::json!({
            "type": "message", "role": "assistant", "status": "completed",
            "content": [{"type": "output_text", "text": content, "annotations": []}]
        }));
    }
    (output_items, has_text, content, function_calls)
}

pub fn build_input_detail(instructions: &str, messages: &[Value], tools: &Value, params: &Value) -> crate::metrics::InputDetail {
    let msgs: Vec<crate::metrics::MessageItem> = messages.iter().map(|m| {
        let role = m.get("role").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let content = m.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
        crate::metrics::MessageItem {
            role, content,
            tool_calls: m.get("tool_calls").cloned(),
            tool_call_id: m.get("tool_call_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
        }
    }).collect();
    crate::metrics::InputDetail { instructions: instructions.to_string(), messages: msgs, tools: tools.to_string(), params: params.clone() }
}
