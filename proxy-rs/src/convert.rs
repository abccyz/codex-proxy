use crate::model::ModelVendor;
use crate::sse::FunctionCall;
use serde_json::Value;

/// Map Responses API roles to Chat Completions roles
fn map_role(role: &str) -> &str {
    match role {
        "developer" => "system",
        other => other,
    }
}

/// Extract text from Responses API content format.
/// Content can be a string, or an array of content objects.
pub fn extract_text_from_content(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(items) => {
            let texts: Vec<String> = items
                .iter()
                .filter_map(|item| item.as_object())
                .filter(|obj| {
                    // Skip reasoning content parts — they are extracted separately as reasoning_content
                    obj.get("type").and_then(|v| v.as_str()) != Some("reasoning")
                })
                .filter_map(|obj| {
                    if let Some(t) = obj.get("text").and_then(|v| v.as_str()) {
                        return Some(t.to_string());
                    }
                    if let Some(t) = obj
                        .get("input_text")
                        .and_then(|v| v.get("text"))
                        .and_then(|v| v.as_str())
                    {
                        return Some(t.to_string());
                    }
                    if let Some(c) = obj.get("content") {
                        // Match Python str(item["content"]): extract string directly
                        return Some(c.as_str().unwrap_or("").to_string());
                    }
                    None
                })
                .collect();
            texts.join("\n")
        }
        other => other.as_str().unwrap_or("").to_string(),
    }
}

/// Convert a single tool from Responses API format to Chat Completions format.
fn convert_single_tool(tool: &Value) -> Option<Value> {
    let obj = tool.as_object()?;

    // Already in Chat Completions format (has "function" key)
    if obj.contains_key("function") {
        return Some(tool.clone());
    }

    let tool_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");

    // Skip web_search
    if tool_type == "web_search" {
        return None;
    }

    // Handle namespace type - recursively convert nested tools
    if tool_type == "namespace" {
        if let Some(nested) = obj.get("tools").and_then(|v| v.as_array()) {
            let converted: Vec<Value> = nested
                .iter()
                .filter_map(|t| convert_single_tool(t))
                .collect();
            return Some(Value::Array(converted));
        }
        return None;
    }

    // Convert function tool – has both "name" and "parameters"
    if obj.contains_key("name") && obj.contains_key("parameters") {
        let description = obj.get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        return Some(serde_json::json!({
            "type": "function",
            "function": {
                "name": obj.get("name").cloned().unwrap_or(Value::Null),
                "description": description,
                "parameters": obj.get("parameters").cloned().unwrap_or_else(|| serde_json::json!({})),
            }
        }));
    }

    // Convert function tool – has "name" but uses "inputSchema" (MCP / plugin tools)
    if obj.contains_key("name") && !obj.contains_key("function") {
        let has_input_schema = obj.contains_key("inputSchema");
        let has_description = obj.contains_key("description");
        if has_input_schema || has_description {
            let description = obj.get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let params = if has_input_schema {
                obj.get("inputSchema").cloned().unwrap_or_else(|| serde_json::json!({}))
            } else {
                serde_json::json!({})
            };
            return Some(serde_json::json!({
                "type": "function",
                "function": {
                    "name": obj.get("name").cloned().unwrap_or(Value::Null),
                    "description": description,
                    "parameters": params,
                }
            }));
        }
    }

    Some(tool.clone())
}

/// Extract the function name from a tool value.
/// Checks `function.name` first, then top-level `name`.
fn extract_tool_name(v: &Value) -> Option<String> {
    v.get("function")
        .and_then(|f| f.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            v.get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
}

/// Convert tools from Responses API format to Chat Completions API format.
/// Deduplicates tools by function name — DashScope requires unique tool names.
pub fn convert_tools_to_chat_format(tools: &Value) -> Value {
    let arr = match tools.as_array() {
        Some(a) => a,
        None => return tools.clone(),
    };

    tracing::debug!(
        "[TOOLS] Received {} tools: {:?}",
        arr.len(),
        arr.iter()
            .filter_map(|t| t.as_object()?.get("name").or(t.as_object()?.get("function")?.get("name")).and_then(|v| v.as_str()))
            .collect::<Vec<_>>()
    );

    let mut converted = Vec::new();
    let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for tool in arr {
        match convert_single_tool(tool) {
            Some(Value::Array(nested)) => {
                for t in nested {
                    let name = extract_tool_name(&t);
                    if let Some(ref n) = name {
                        if seen_names.insert(n.clone()) {
                            converted.push(t);
                        } else {
                            tracing::warn!("[TOOLS] Duplicate namespace-nested tool '{}' – skipped", n);
                        }
                    } else {
                        converted.push(t);
                    }
                }
            }
            Some(v) => {
                let name = extract_tool_name(&v);
                if let Some(ref n) = name {
                    if seen_names.insert(n.clone()) {
                        converted.push(v);
                    } else {
                        tracing::warn!("[TOOLS] Duplicate tool '{}' – skipped", n);
                    }
                } else {
                    // Fallback: tool has no recognizable name – try top-level "name" key
                    let fallback_name = v.get("name").and_then(|v| v.as_str());
                    if let Some(fb_name) = fallback_name {
                        if seen_names.insert(fb_name.to_string()) {
                            converted.push(v);
                        } else {
                            tracing::warn!("[TOOLS] Duplicate fallback-name tool '{}' – skipped", fb_name);
                        }
                    } else {
                        tracing::debug!("[TOOLS] Pushing unnamed tool without dedup");
                        converted.push(v);
                    }
                }
            }
            None => {}
        }
    }

    // Post-conversion validation: warn if duplicates slipped through
    {
        let mut final_names = std::collections::HashSet::new();
        for t in &converted {
            if let Some(n) = extract_tool_name(t) {
                if !final_names.insert(n.clone()) {
                    tracing::error!("[TOOLS] BUG: duplicate '{}' in final output – dedup failed!", n);
                }
            }
        }
    }

    tracing::info!(
        "[TOOLS] Converted {} -> {} tools, names: {:?}",
        arr.len(),
        converted.len(),
        converted.iter().filter_map(|t| extract_tool_name(t)).collect::<Vec<_>>()
    );

    Value::Array(converted)
}

/// Extract reasoning_content from a message object.
/// Checks top-level field first, then content array items (type="reasoning" or reasoning_content field).
/// For thinking mode models (DeepSeek/Qwen): reasoning_content must always be present.
fn extract_reasoning_content(obj: &serde_json::Map<String, Value>, vendor: ModelVendor) -> Option<String> {
    // Check top-level field
    if let Some(r) = obj.get("reasoning_content").and_then(|v| v.as_str()) {
        return Some(r.to_string());
    }
    // Check content array items
    if let Some(Value::Array(items)) = obj.get("content") {
        let mut parts = Vec::new();
        let mut has_reasoning_part = false;
        for item in items {
            if let Some(item_obj) = item.as_object() {
                if let Some(r) = item_obj.get("reasoning_content").and_then(|v| v.as_str()) {
                    has_reasoning_part = true;
                    if !r.is_empty() { parts.push(r.to_string()); }
                } else if item_obj.get("type").and_then(|v| v.as_str()) == Some("reasoning") {
                    has_reasoning_part = true;
                    if let Some(t) = item_obj.get("text").and_then(|v| v.as_str()) {
                        if !t.is_empty() { parts.push(t.to_string()); }
                    }
                }
            }
        }
        if !parts.is_empty() {
            return Some(parts.join("\n"));
        }
        // Return empty string if reasoning content part exists
        // (thinking mode requires reasoning_content on every assistant message)
        if has_reasoning_part && vendor.requires_reasoning_content() {
            return Some(String::new());
        }
    }
    None
}

/// Convert Responses API input to Chat Completions messages format.
pub fn convert_input_to_messages(input: &Value, model: &str) -> Vec<Value> {
    let vendor = ModelVendor::from_model_name(model);
    match input {
        Value::String(s) => {
            return vec![serde_json::json!({"role": "user", "content": s})];
        }
        Value::Array(items) => {
            let mut messages = Vec::new();
            for item in items {
                if let Some(obj) = item.as_object() {
                    let item_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");

                    // Message type
                    if item_type == "message"
                        || (obj.contains_key("role") && obj.contains_key("content"))
                    {
                        let role = map_role(
                            obj.get("role").and_then(|v| v.as_str()).unwrap_or("user"),
                        );
                        let default_content = Value::String(String::new());
                        let content = obj.get("content").unwrap_or(&default_content);
                        let text = extract_text_from_content(content);
                        
                        // For assistant messages, include reasoning_content based on vendor requirements
                        if role == "assistant" {
                            if vendor.requires_reasoning_content() {
                                let reasoning = extract_reasoning_content(obj, vendor)
                                    .unwrap_or_else(|| String::new());
                                messages.push(serde_json::json!({
                                    "role": role,
                                    "content": text,
                                    "reasoning_content": reasoning
                                }));
                            } else {
                                messages.push(serde_json::json!({"role": role, "content": text}));
                            }
                        } else {
                            messages.push(serde_json::json!({"role": role, "content": text}));
                        }
                    }
                    // Assistant function calls
                    else if item_type == "function_call" {
                        let call_id = obj
                            .get("call_id")
                            .or_else(|| obj.get("id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let arguments = obj
                            .get("arguments")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let mut fc_msg = serde_json::json!({
                            "role": "assistant",
                            "content": null,
                            "tool_calls": [{
                                "id": call_id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": arguments
                                }
                            }]
                        });
                        // Models requiring reasoning_content on assistant messages
                        if vendor.requires_reasoning_content() {
                            fc_msg.as_object_mut().unwrap().insert(
                                "reasoning_content".to_string(),
                                Value::String(String::new()),
                            );
                        }
                        messages.push(fc_msg);
                    }
                    // Function call outputs
                    else if item_type == "function_call_output" {
                        let call_id = obj
                            .get("call_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        // Match Python str(output): extract string value directly, not JSON-serialized
                        let output = obj
                            .get("output")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        messages.push(serde_json::json!({
                            "role": "tool",
                            "content": output,
                            "tool_call_id": call_id
                        }));
                    }
                    // Content types (input_text, input_image, etc.)
                    else if ["input_text", "input_image", "input_file"].contains(&item_type) {
                        let text = extract_text_from_content(item);
                        messages.push(serde_json::json!({"role": "user", "content": text}));
                    }
                    // Fallback
                    else {
                        let text = extract_text_from_content(item);
                        if !text.is_empty() {
                            messages.push(serde_json::json!({"role": "user", "content": text}));
                        }
                    }
                } else {
                    // Non-dict items: match Python str(item)
                    let text = item.as_str().unwrap_or("");
                    messages
                        .push(serde_json::json!({"role": "user", "content": text}));
                }
            }

            merge_consecutive_assistant(messages)
        }
        _ => {
            // Non-string, non-array inputs: match Python str(data_input)
            let text = input.as_str().unwrap_or("");
            vec![serde_json::json!({"role": "user", "content": text})]
        }
    }
}

/// Merge consecutive assistant messages.
fn merge_consecutive_assistant(messages: Vec<Value>) -> Vec<Value> {
    let mut merged: Vec<Value> = Vec::new();
    for msg in messages {
        let role = msg
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if role == "assistant"
            && merged
                .last()
                .and_then(|m: &Value| m.get("role"))
                .and_then(|v| v.as_str())
                == Some("assistant")
        {
            let prev = merged.last_mut().unwrap();
            let prev_obj = prev.as_object_mut().unwrap();

            // Merge content
            if let (Some(cur_content), Some(prev_content)) = (
                msg.get("content"),
                prev_obj.get("content"),
            ) {
                if !cur_content.is_null() && !prev_content.is_null() {
                    let combined = format!(
                        "{}\n{}",
                        prev_content.as_str().unwrap_or(""),
                        cur_content.as_str().unwrap_or("")
                    );
                    prev_obj.insert("content".to_string(), Value::String(combined));
                } else if !cur_content.is_null() {
                    prev_obj.insert("content".to_string(), cur_content.clone());
                }
            }

            // Merge tool_calls
            if let (Some(cur_tc), Some(prev_tc)) = (
                msg.get("tool_calls"),
                prev_obj.get_mut("tool_calls"),
            ) {
                if let (Some(cur_arr), Some(prev_arr)) =
                    (cur_tc.as_array(), prev_tc.as_array_mut())
                {
                    prev_arr.extend(cur_arr.clone());
                }
            } else if let Some(cur_tc) = msg.get("tool_calls") {
                prev_obj.insert("tool_calls".to_string(), cur_tc.clone());
            }

            // Merge reasoning_content (required for Qwen thinking mode)
            if let (Some(cur_reasoning), Some(prev_reasoning)) = (
                msg.get("reasoning_content"),
                prev_obj.get("reasoning_content"),
            ) {
                if !cur_reasoning.is_null() && !prev_reasoning.is_null() {
                    let combined = format!(
                        "{}\n{}",
                        prev_reasoning.as_str().unwrap_or(""),
                        cur_reasoning.as_str().unwrap_or("")
                    );
                    prev_obj.insert("reasoning_content".to_string(), Value::String(combined));
                } else if !cur_reasoning.is_null() {
                    prev_obj.insert("reasoning_content".to_string(), cur_reasoning.clone());
                }
            } else if let Some(cur_reasoning) = msg.get("reasoning_content") {
                prev_obj.insert("reasoning_content".to_string(), cur_reasoning.clone());
            }
        } else {
            merged.push(msg);
        }
    }
    deduplicate_tool_calls(merged)
}

/// Remove duplicate tool calls from conversation history.
/// DashScope rejects requests with identical tool calls (same name + arguments).
fn deduplicate_tool_calls(messages: Vec<Value>) -> Vec<Value> {
    use std::collections::HashSet;
    
    let mut seen_tool_calls: HashSet<String> = HashSet::new();
    let mut removed_call_ids: HashSet<String> = HashSet::new();
    let mut result: Vec<Value> = Vec::new();
    
    for msg in messages {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
        
        if role == "assistant" {
            if let Some(tool_calls) = msg.get("tool_calls").and_then(|v| v.as_array()) {
                // Filter out duplicate tool calls
                let mut unique_calls: Vec<Value> = Vec::new();
                for tc in tool_calls {
                    let func = tc.get("function");
                    let name = func.and_then(|f| f.get("name")).and_then(|v| v.as_str()).unwrap_or("");
                    let args = func.and_then(|f| f.get("arguments")).and_then(|v| v.as_str()).unwrap_or("");
                    let call_id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let key = format!("{}:{}", name, args);
                    
                    if seen_tool_calls.contains(&key) {
                        // This is a duplicate, mark its ID for removal
                        removed_call_ids.insert(call_id.to_string());
                    } else {
                        // Keep this tool call
                        seen_tool_calls.insert(key);
                        unique_calls.push(tc.clone());
                    }
                }
                
                if unique_calls.is_empty() {
                    // Skip this message if all tool calls are duplicates
                    continue;
                }
                
                let mut new_msg = msg.clone();
                new_msg.as_object_mut().unwrap().insert(
                    "tool_calls".to_string(),
                    Value::Array(unique_calls)
                );
                result.push(new_msg);
            } else {
                result.push(msg);
            }
        } else if role == "tool" {
            // Check if this tool response corresponds to a removed tool call
            let call_id = msg.get("tool_call_id").and_then(|v| v.as_str()).unwrap_or("");
            
            if removed_call_ids.contains(call_id) {
                // Skip this tool response since its tool call was removed
                continue;
            }
            result.push(msg);
        } else {
            result.push(msg);
        }
    }
    
    result
}

/// Convert Chat Completions message to Responses API output items.
/// Returns (output_items, has_text, final_text, function_calls).
pub fn convert_chat_message_to_output(
    msg: &Value,
) -> (Vec<Value>, bool, String, Vec<FunctionCall>) {
    let content = msg
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let tool_calls = msg.get("tool_calls").and_then(|v| v.as_array());

    let mut output_items = Vec::new();
    let mut function_calls = Vec::new();
    let has_text = !content.is_empty();

    if has_text {
        output_items.push(serde_json::json!({
            "type": "message",
            "role": "assistant",
            "status": "completed",
            "content": [{"type": "output_text", "text": content, "annotations": []}]
        }));
    }

    if let Some(tc_arr) = tool_calls {
        for tc in tc_arr {
            let func = tc.get("function").and_then(|v| v.as_object());
            if let Some(func) = func {
                let name = func
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let arguments = func
                    .get("arguments")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let id = tc
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                function_calls.push(FunctionCall {
                    id: id.clone(),
                    name: name.clone(),
                    arguments: arguments.clone(),
                });

                output_items.push(serde_json::json!({
                    "type": "function_call",
                    "id": id,
                    "call_id": id,
                    "name": name,
                    "arguments": arguments,
                }));
            }
        }
    }

    // Ensure at least one output item
    if output_items.is_empty() {
        output_items.push(serde_json::json!({
            "type": "message",
            "role": "assistant",
            "status": "completed",
            "content": [{"type": "output_text", "text": content, "annotations": []}]
        }));
    }

    (output_items, has_text, content, function_calls)
}

/// Estimate token count from text length. Rough: CJK ~1.5, ASCII ~4.
/// Using conservative ~3 chars/token.
#[allow(dead_code)]
pub fn estimate_tokens(text: &str) -> u64 {
    let total_chars = text.chars().count() as u64;
    if total_chars == 0 {
        1
    } else {
        total_chars / 3
    }
}

/// Build input detail for history recording
pub fn build_input_detail(
    instructions: &str,
    messages: &[Value],
    tools: &Value,
    params: &Value,
) -> crate::metrics::InputDetail {
    let msgs: Vec<crate::metrics::MessageItem> = messages
        .iter()
        .map(|m| {
            let role = m.get("role").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let content = m
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let tool_calls = m.get("tool_calls").cloned();
            let tool_call_id = m
                .get("tool_call_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            crate::metrics::MessageItem {
                role,
                content,
                tool_calls,
                tool_call_id,
            }
        })
        .collect();

    crate::metrics::InputDetail {
        instructions: instructions.to_string(),
        messages: msgs,
        tools: tools.to_string(),
        params: params.clone(),
    }
}
