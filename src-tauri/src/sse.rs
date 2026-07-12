use serde_json::Value;
use uuid::Uuid;
use crate::metrics::SharedMetrics;

pub fn sse_event(event: &str, data: &Value) -> String {
    format!("event: {}\ndata: {}\n\n", event, serde_json::to_string(data).unwrap())
}

pub fn make_response_id() -> String { format!("resp_{}", &Uuid::new_v4().to_string().replace("-", "")[..24]) }
pub fn make_item_id() -> String { format!("item_{}", &Uuid::new_v4().to_string().replace("-", "")[..24]) }
pub fn make_req_id() -> String { format!("req_{}", &Uuid::new_v4().to_string().replace("-", "")[..12]) }

pub fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes { return s; }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) { end -= 1; }
    &s[..end]
}

#[derive(Debug, Clone)]
pub struct FunctionCall {
    pub id: String, pub name: String, pub arguments: String,
}

pub struct StreamCleanupGuard {
    metrics: SharedMetrics, armed: bool,
}
impl StreamCleanupGuard {
    pub fn new(metrics: SharedMetrics) -> Self { Self { metrics, armed: true } }
    pub fn disarm(&mut self) { self.armed = false; }
}
impl Drop for StreamCleanupGuard {
    fn drop(&mut self) { if self.armed { self.metrics.stream_end(); } }
}

#[derive(Default)]
pub struct ToolCallAcc {
    pub id: String, pub name: String, pub arguments_parts: Vec<String>,
}
