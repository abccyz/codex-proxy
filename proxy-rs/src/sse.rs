//! SSE 事件生成 — 统一的流式响应格式化工具
//!
//! 从 proxy.rs 中提取，消除 handle_streaming_response 和
//! handle_nonstreaming_to_sse 之间的重复逻辑。

use serde_json::Value;
use uuid::Uuid;

use crate::metrics::SharedMetrics;

// ── 工具函数 ────────────────────────────────────────────────

pub fn sse_event(event: &str, data: &Value) -> String {
    format!(
        "event: {}\ndata: {}\n\n",
        event,
        serde_json::to_string(data).unwrap()
    )
}

pub fn make_response_id() -> String {
    format!("resp_{}", Uuid::new_v4().to_string().replace("-", "")[..24].to_string())
}

pub fn make_item_id() -> String {
    format!("item_{}", Uuid::new_v4().to_string().replace("-", "")[..24].to_string())
}

pub fn make_req_id() -> String {
    format!("req_{}", Uuid::new_v4().to_string().replace("-", "")[..12].to_string())
}

/// UTF-8 安全截断
pub fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

// ── 事件格式辅助 ─────────────────────────────────────────────

pub struct SseEventBuilder;

impl SseEventBuilder {
    pub fn response_created(resp_id: &str, model: &str, created: i64) -> Value {
        serde_json::json!({
            "type": "response.created",
            "response": {
                "id": resp_id,
                "object": "response",
                "created_at": created,
                "model": model,
                "output": [],
                "status": "in_progress",
                "usage": {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0},
            }
        })
    }

    pub fn response_in_progress(resp_id: &str) -> Value {
        serde_json::json!({
            "type": "response.in_progress",
            "response": {"id": resp_id, "object": "response", "status": "in_progress"},
        })
    }

    pub fn response_completed(
        resp_id: &str, model: &str, created: i64,
        output_items: &[Value], in_tok: u64, out_tok: u64,
    ) -> Value {
        serde_json::json!({
            "type": "response.completed",
            "response": {
                "id": resp_id,
                "object": "response",
                "created_at": created,
                "model": model,
                "output": output_items,
                "status": "completed",
                "usage": {
                    "input_tokens": in_tok,
                    "output_tokens": out_tok,
                    "total_tokens": in_tok + out_tok,
                },
            },
        })
    }

    pub fn output_item_added_text(item_id: &str, output_index: u32) -> Value {
        serde_json::json!({
            "type": "response.output_item.added",
            "output_index": output_index,
            "item": {
                "type": "message",
                "id": item_id,
                "role": "assistant",
                "status": "in_progress",
                "content": [],
            }
        })
    }

    pub fn content_part_added(item_id: &str, output_index: u32, content_index: u32) -> Value {
        serde_json::json!({
            "type": "response.content_part.added",
            "item_id": item_id,
            "output_index": output_index,
            "content_index": content_index,
            "part": {"type": "output_text", "text": "", "annotations": []},
        })
    }

    pub fn output_text_delta(item_id: &str, output_index: u32, content_index: u32, delta: &str) -> Value {
        serde_json::json!({
            "type": "response.output_text.delta",
            "item_id": item_id,
            "output_index": output_index,
            "content_index": content_index,
            "delta": delta,
        })
    }

    pub fn content_part_done(item_id: &str, output_index: u32, content_index: u32, text: &str) -> Value {
        serde_json::json!({
            "type": "response.content_part.done",
            "item_id": item_id,
            "output_index": output_index,
            "content_index": content_index,
            "part": {"type": "output_text", "text": text, "annotations": []},
        })
    }

    pub fn output_item_done_text(item_id: &str, output_index: u32, text: &str) -> Value {
        serde_json::json!({
            "type": "response.output_item.done",
            "output_index": output_index,
            "item": {
                "type": "message",
                "id": item_id,
                "role": "assistant",
                "status": "completed",
                "content": [{"type": "output_text", "text": text, "annotations": []}],
            },
        })
    }

    pub fn output_item_added_fn_call(fc: &FunctionCall, output_index: u32) -> Value {
        serde_json::json!({
            "type": "response.output_item.added",
            "output_index": output_index,
            "item": {
                "type": "function_call",
                "id": fc.id,
                "call_id": fc.id,
                "name": fc.name,
                "arguments": fc.arguments,
                "status": "in_progress",
            },
        })
    }

    pub fn output_item_done_fn_call(fc: &FunctionCall, output_index: u32) -> Value {
        serde_json::json!({
            "type": "response.output_item.done",
            "output_index": output_index,
            "item": {
                "type": "function_call",
                "id": fc.id,
                "call_id": fc.id,
                "name": fc.name,
                "arguments": fc.arguments,
                "status": "completed",
            },
        })
    }
}

// ── 流清理守卫 ──────────────────────────────────────────────
// 确保客户端提前断开时也能清理流状态，防止计数泄漏

pub struct StreamCleanupGuard {
    metrics: SharedMetrics,
    armed: bool,
}

impl StreamCleanupGuard {
    pub fn new(metrics: SharedMetrics) -> Self {
        Self { metrics, armed: true }
    }

    pub fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for StreamCleanupGuard {
    fn drop(&mut self) {
        if self.armed {
            self.metrics.stream_end();
        }
    }
}

// ── 辅助类型 ─────────────────────────────────────────────────

#[derive(Default)]
pub struct ToolCallAcc {
    pub id: String,
    pub name: String,
    pub arguments_parts: Vec<String>,
}

/// 函数调用信息（也导出供 convert.rs 使用）
#[derive(Debug, Clone)]
pub struct FunctionCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}
