use chrono::Local;
use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

struct LiveStream { model: String, content: String, start_time: Instant }

const MAX_HISTORY: usize = 100;
const MAX_FULL_DETAIL: usize = 30;

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars { s.to_string() } else { format!("{}...", s.chars().take(max_chars).collect::<String>()) }
}

#[derive(Debug, Clone, Serialize)]
pub struct LatencyPoint { pub t: f64, pub v: f64 }
#[derive(Debug, Clone, Serialize)]
pub struct ThroughputPoint { pub t: i64, pub c: u64 }

#[derive(Debug, Clone, Serialize)]
pub struct InputDetail { pub instructions: String, pub messages: Vec<MessageItem>, pub tools: String, pub params: serde_json::Value }

#[derive(Debug, Clone, Serialize)]
pub struct MessageItem { pub role: String, pub content: String, #[serde(skip_serializing_if = "Option::is_none")] pub tool_calls: Option<serde_json::Value>, #[serde(skip_serializing_if = "Option::is_none")] pub tool_call_id: Option<String> }

#[derive(Debug, Clone, Serialize)]
pub struct HistoryMeta { pub req_id: Option<String>, pub time: String, pub timestamp: f64, pub model: String, pub stream: bool, pub status: String, pub latency: f64, pub input_tokens: u64, pub output_tokens: u64, pub error: String, pub input_summary: String, pub input_preview: String, pub preview: String }

#[derive(Debug, Clone)]
pub struct HistoryRecord { pub req_id: Option<String>, pub time: String, pub timestamp: f64, pub model: String, pub stream: bool, pub status: String, pub latency: f64, pub input_tokens: u64, pub output_tokens: u64, pub error: String, pub input_summary: String, pub input_preview: String, pub input_detail: InputDetail, pub preview: String, pub content: String }

impl HistoryRecord {
    fn to_meta(&self) -> HistoryMeta {
        HistoryMeta { req_id: self.req_id.clone(), time: self.time.clone(), timestamp: self.timestamp, model: self.model.clone(), stream: self.stream, status: self.status.clone(), latency: self.latency, input_tokens: self.input_tokens, output_tokens: self.output_tokens, error: self.error.clone(), input_summary: self.input_summary.clone(), input_preview: self.input_preview.clone(), preview: self.preview.clone() }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveStreamSnapshot { pub model: String, pub accumulated: String, pub elapsed_secs: f64 }

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub uptime: u64, pub total: u64, pub success: u64, pub failed: u64, pub active_streams: u64,
    pub avg_latency: f64, pub rpm: f64, pub total_input_tokens: u64, pub total_output_tokens: u64,
    pub total_tokens: u64, pub history: Vec<HistoryMeta>, pub throughput: Vec<ThroughputPoint>,
    pub latency_history: Vec<LatencyPoint>, pub model_stats: HashMap<String, u64>,
    pub live_stream: Option<LiveStreamSnapshot>,
}

pub struct Metrics {
    total: AtomicU64, success: AtomicU64, failed: AtomicU64,
    total_input_tokens: AtomicU64, total_output_tokens: AtomicU64,
    total_latency_ms: AtomicU64, active_streams: AtomicU64,
    start_time: Instant, generation: AtomicU64,
    history: RwLock<MetricsHistory>,
    live_stream: Mutex<Option<LiveStream>>,
    cached_snapshot: Mutex<Option<(u64, Snapshot)>>,
}

struct MetricsHistory {
    records: VecDeque<HistoryRecord>,
    throughput: Vec<ThroughputPoint>,
    latency_history: Vec<LatencyPoint>,
    model_stats: HashMap<String, u64>,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            total: AtomicU64::new(0), success: AtomicU64::new(0), failed: AtomicU64::new(0),
            total_input_tokens: AtomicU64::new(0), total_output_tokens: AtomicU64::new(0),
            total_latency_ms: AtomicU64::new(0), active_streams: AtomicU64::new(0),
            start_time: Instant::now(), generation: AtomicU64::new(0),
            history: RwLock::new(MetricsHistory { records: VecDeque::new(), throughput: Vec::new(), latency_history: Vec::new(), model_stats: HashMap::new() }),
            live_stream: Mutex::new(None), cached_snapshot: Mutex::new(None),
        }
    }

    pub fn stream_begin(&self, model: String) {
        self.active_streams.fetch_add(1, Ordering::AcqRel);
        *self.live_stream.lock() = Some(LiveStream { model, content: String::new(), start_time: Instant::now() });
        self.generation.fetch_add(1, Ordering::Release);
    }

    pub fn stream_chunk(&self, chunk: &str) {
        if let Some(ref mut live) = *self.live_stream.lock() { live.content.push_str(chunk); }
        self.generation.fetch_add(1, Ordering::Release);
    }

    pub fn stream_end(&self) {
        *self.live_stream.lock() = None;
        let prev = self.active_streams.fetch_sub(1, Ordering::AcqRel);
        if prev == 0 { self.active_streams.store(0, Ordering::Release); }
        self.generation.fetch_add(1, Ordering::Release);
    }

    pub fn generation(&self) -> u64 { self.generation.load(Ordering::Acquire) }

    pub fn record_request(&self, model: String, success: bool, status: String, latency: f64, input_tokens: u64, output_tokens: u64, error: String, content: String, req_id: Option<String>, input_detail: InputDetail) {
        self.total.fetch_add(1, Ordering::Release);
        if success { self.success.fetch_add(1, Ordering::Release); } else { self.failed.fetch_add(1, Ordering::Release); }
        self.total_input_tokens.fetch_add(input_tokens, Ordering::Release);
        self.total_output_tokens.fetch_add(output_tokens, Ordering::Release);
        self.total_latency_ms.fetch_add((latency * 1000.0) as u64, Ordering::Release);
        let now = Local::now();
        let time_str = now.format("%H:%M:%S").to_string();
        let timestamp = now.timestamp() as f64;
        let minute = now.timestamp() / 60;
        let summary = if input_detail.instructions.is_empty() {
            input_detail.messages.iter().map(|m| m.content.clone()).collect::<Vec<_>>().join(" | ")
        } else { input_detail.instructions.clone() };
        let input_preview = truncate_str(&summary, 80);
        let preview = truncate_str(&content, 120);
        let rec = HistoryRecord {
            req_id, time: time_str, timestamp, model: model.clone(), stream: true, status, latency,
            input_tokens, output_tokens, error, input_summary: summary, input_preview, input_detail, preview, content,
        };
        let mut hist = self.history.write();
        hist.records.push_back(rec);
        while hist.records.len() > MAX_HISTORY { hist.records.pop_front(); }
        let full_detail_count = hist.records.iter().filter(|r| r.content.len() > 500 || r.input_detail.messages.len() > 3).count();
        if full_detail_count > MAX_FULL_DETAIL {
            for r in hist.records.iter_mut().rev().skip(MAX_FULL_DETAIL) { r.content.truncate(0); }
        }
        let mut tps = hist.throughput.iter().enumerate().filter_map(|(i, tp)| if i > 0 { Some(tp.c) } else { None }).collect::<Vec<_>>();
        tps.push(hist.records.len() as u64);
        hist.throughput.push(ThroughputPoint { t: minute, c: 1 });
        if hist.throughput.len() > 60 { hist.throughput.remove(0); }
        hist.latency_history.push(LatencyPoint { t: timestamp, v: latency });
        if hist.latency_history.len() > 200 { hist.latency_history.remove(0); }
        *hist.model_stats.entry(model).or_insert(0) += 1;
        self.generation.fetch_add(1, Ordering::Release);
    }

    pub fn snapshot(&self) -> Snapshot {
        let gen = self.generation();
        {
            let cache = self.cached_snapshot.lock();
            if let Some((cached_gen, ref cs)) = *cache {
                if cached_gen == gen {
                    let live = self.live_stream.lock();
                    if let Some(ref li) = *live {
                        let ls = Some(LiveStreamSnapshot { model: li.model.clone(), accumulated: li.content.clone(), elapsed_secs: li.start_time.elapsed().as_secs_f64() });
                        drop(live);
                        let mut snap = cs.clone();
                        snap.live_stream = ls;
                        return snap;
                    }
                    return cs.clone();
                }
            }
        }
        let uptime = self.start_time.elapsed().as_secs();
        let total = self.total.load(Ordering::Acquire);
        let success = self.success.load(Ordering::Acquire);
        let failed = self.failed.load(Ordering::Acquire);
        let active_streams = self.active_streams.load(Ordering::Acquire);
        let total_input = self.total_input_tokens.load(Ordering::Acquire);
        let total_output = self.total_output_tokens.load(Ordering::Acquire);
        let total_latency_ms = self.total_latency_ms.load(Ordering::Acquire);
        let avg_latency = if success > 0 { total_latency_ms as f64 / 1000.0 / success as f64 } else { 0.0 };
        let hist = self.history.read();
        let now = chrono::Utc::now().timestamp();
        let recent_count: u64 = hist.throughput.iter().filter(|tp| now - tp.t * 60 < 300).map(|tp| tp.c).sum();
        let recent_minutes = hist.throughput.iter().filter(|tp| now - tp.t * 60 < 300).count().max(1) as f64;
        let rpm = recent_count as f64 / recent_minutes.min(5.0).max(1.0);
        let live_stream = self.live_stream.lock().as_ref().map(|live| LiveStreamSnapshot { model: live.model.clone(), accumulated: live.content.clone(), elapsed_secs: live.start_time.elapsed().as_secs_f64() });
        let snap = Snapshot {
            uptime, total, success, failed, active_streams,
            avg_latency: (avg_latency * 100.0).round() / 100.0,
            rpm: (rpm * 10.0).round() / 10.0,
            total_input_tokens: total_input, total_output_tokens: total_output,
            total_tokens: total_input + total_output,
            history: hist.records.iter().map(|r| r.to_meta()).collect(),
            throughput: hist.throughput.iter().cloned().collect(),
            latency_history: hist.latency_history.iter().cloned().collect(),
            model_stats: hist.model_stats.clone(),
            live_stream,
        };
        *self.cached_snapshot.lock() = Some((gen, snap.clone()));
        snap
    }

    pub fn get_full_content(&self, idx: usize) -> Option<String> {
        self.history.read().records.get(idx).map(|r| r.content.clone())
    }

    pub fn get_input_detail(&self, idx: usize) -> Option<InputDetail> {
        self.history.read().records.get(idx).map(|r| r.input_detail.clone())
    }
}

impl Default for Metrics {
    fn default() -> Self { Self::new() }
}

pub type SharedMetrics = Arc<Metrics>;
