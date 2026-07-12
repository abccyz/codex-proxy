---
name: proxy-optional-fields
description: Handle model-specific optional fields (e.g. reasoning_content) when proxying between API formats — model-aware conditional output and lenient input extraction
source: auto-skill
extracted_at: '2026-07-11T08:15:15.931Z'
---

# Handling Model-Specific Optional Fields in API Proxies

## Problem

When proxying between API formats (e.g., Responses API → Chat Completions API), some models return optional fields (like `reasoning_content` for thinking mode) that **must be passed back** in subsequent turns. Different models have **contradictory requirements** for the same field.

### Concrete Example: `reasoning_content` Conflict

| Model | Requirement |
|-------|-------------|
| **DeepSeek** | Rejects empty `reasoning_content` — only include when non-empty |
| **Qwen** | Requires `reasoning_content` to always be present in assistant messages (even if empty `""`) |

Both produce the same error when their requirement is violated:
```json
{"error":{"message":"The `reasoning_content` in the thinking mode must be passed back to the API.","type":"invalid_request_error"}}
```

## Solution

### Rule 1: Model-Aware Output — Detect Model, Apply Correct Policy

Create a central helper that builds output items with model-specific field handling:

```rust
fn is_deepseek_model(model: &str) -> bool {
    model.to_lowercase().contains("deepseek")
}

fn build_text_output_item(item_id: &str, text: &str, reasoning_content: &str, model: &str) -> Value {
    let mut item = serde_json::json!({
        "type": "message", "id": item_id, "role": "assistant",
        "status": "completed",
        "content": [{"type": "output_text", "text": text, "annotations": []}],
    });
    let should_include = if is_deepseek_model(model) {
        !reasoning_content.is_empty()  // DeepSeek: only when non-empty
    } else {
        true  // Qwen and others: always include (even if empty)
    };
    if should_include {
        item.as_object_mut().unwrap().insert(
            "reasoning_content".to_string(),
            Value::String(reasoning_content.to_string()),
        );
    }
    item
}
```

**Use this helper in ALL response paths**: normal response, streaming SSE, non-streaming-to-SSE, and fallback items.

### Rule 2: Model-Aware Input Extraction — Preserve Empty for Non-DeepSeek

When extracting from incoming requests, pass the model type to the extractor:

```rust
fn extract_reasoning_content(obj: &serde_json::Map<String, Value>, is_deepseek: bool) -> Option<String> {
    // Check top-level field
    if let Some(r) = obj.get("reasoning_content").and_then(|v| v.as_str()) {
        if !r.is_empty() { return Some(r.to_string()); }
        if !is_deepseek { return Some(String::new()); } // Qwen: preserve empty
    }
    // Check content array items (type="reasoning" or nested reasoning_content)
    if let Some(Value::Array(items)) = obj.get("content") {
        let mut parts = Vec::new();
        for item in items {
            if let Some(item_obj) = item.as_object() {
                if let Some(r) = item_obj.get("reasoning_content").and_then(|v| v.as_str()) {
                    if !r.is_empty() { parts.push(r.to_string()); }
                } else if item_obj.get("type").and_then(|v| v.as_str()) == Some("reasoning") {
                    if let Some(t) = item_obj.get("text").and_then(|v| v.as_str()) {
                        if !t.is_empty() { parts.push(t.to_string()); }
                    }
                }
            }
        }
        if !parts.is_empty() { return Some(parts.join("\n")); }
    }
    None
}
```

### Rule 3: Pass Model Through the Conversion Pipeline

`convert_input_to_messages` must accept a `model` parameter so it can determine field policies:

```rust
pub fn convert_input_to_messages(input: &Value, model: &str) -> Vec<Value> {
    let is_deepseek = model.to_lowercase().contains("deepseek");
    // ... use is_deepseek when calling extract_reasoning_content
}
```

## Checklist for Adding Model-Specific Field Support

1. **Identify the field** and its per-model requirements (some models need it always present, others reject empty)
2. **Create a model detection helper** (`is_deepseek_model`, `is_qwen_model`, etc.)
3. **Create a central output builder** that applies the correct policy per model
4. **Use the builder in ALL response paths** — normal, streaming, non-streaming-to-SSE, fallback
5. **Make input extraction model-aware** — pass model type to extractors
6. **Preserve through merging** — merge consecutive assistant messages' fields
7. **Pass model through the conversion pipeline** — `convert_input_to_messages(input, model)`

## Key Lesson

**Never assume a single policy for optional fields across all models.** The same field can have opposite requirements:
- Model A may reject empty values → conditionally include
- Model B may require the field to always exist → always include

The solution is always model-aware branching, not a one-size-fits-all approach.
