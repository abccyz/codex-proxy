use chrono::Local;
use parking_lot::RwLock;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Live streaming state — shared between proxy handler (writer) and UI (reader).
/// Stores the currently in-progress streaming response so the real-time panel
/// can display partial content as it arrives.
struct LiveStream {
    model: String,
    content: String,
    start_time: Instant,
}

const MAX_HISTORY: usize = 100;
const THROUGHPUT_WINDOW: usize = 60;
/// 仅最近 N 条记录保留完整 content 和 input_detail（其余截断以节省内存）
const MAX_FULL_DETAIL: usize = 30;

/// Truncate a string to at most `max_chars` characters, respecting UTF-8 boundaries.
fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{}...", truncated)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LatencyPoint {
    pub t: f64,
    pub v: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThroughputPoint {
    pub t: i64,  // minute timestamp
    pub c: u64,  // count
}

#[derive(Debug, Clone, Serialize)]
pub struct InputDetail {
    pub instructions: String,
    pub messages: Vec<MessageItem>,
    pub tools: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageItem {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryMeta {
    pub req_id: Option<String>,
    pub time: String,
    pub timestamp: f64,
    pub model: String,
    pub stream: bool,
    pub status: String,
    pub latency: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub error: String,
    pub input_summary: String,
    pub input_preview: String,
    pub preview: String,
}

/// 内部完整历史记录（含大字段），存储在 RwLock 中，snapshot 时转为轻量的 HistoryMeta
#[derive(Debug, Clone)]
pub struct HistoryRecord {
    pub req_id: Option<String>,
    pub time: String,
    pub timestamp: f64,
    pub model: String,
    pub stream: bool,
    pub status: String,
    pub latency: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub error: String,
    pub input_summary: String,
    pub input_preview: String,
    pub input_detail: InputDetail,
    pub preview: String,
    pub content: String,
}

impl HistoryRecord {
    fn to_meta(&self) -> HistoryMeta {
        HistoryMeta {
            req_id: self.req_id.clone(),
            time: self.time.clone(),
            timestamp: self.timestamp,
            model: self.model.clone(),
            stream: self.stream,
            status: self.status.clone(),
            latency: self.latency,
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            error: self.error.clone(),
            input_summary: self.input_summary.clone(),
            input_preview: self.input_preview.clone(),
            preview: self.preview.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveStreamSnapshot {
    pub model: String,
    pub accumulated: String,
    pub elapsed_secs: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub uptime: u64,
    pub total: u64,
    pub success: u64,
    pub failed: u64,
    pub active_streams: u64,
    pub avg_latency: f64,
    pub rpm: f64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tokens: u64,
    pub history: Arc<Vec<HistoryMeta>>,
    pub throughput: Arc<Vec<ThroughputPoint>>,
    pub latency_history: Arc<Vec<LatencyPoint>>,
    pub model_stats: Arc<HashMap<String, u64>>,
    /// Current in-progress streaming response (if any), so the real-time panel
    /// can display partial content as it arrives.
    pub live_stream: Option<LiveStreamSnapshot>,
}

pub struct Metrics {
    // 热路径计数器 — 无锁原子操作
    total: AtomicU64,
    success: AtomicU64,
    failed: AtomicU64,
    total_input_tokens: AtomicU64,
    total_output_tokens: AtomicU64,
    // total_latency 以毫秒存储，避免 f64 原子操作
    total_latency_ms: AtomicU64,
    active_streams: AtomicU64,
    start_time: Instant,
    generation: AtomicU64,
    // 历史数据 — 仅在推送/快照时锁定
    history: RwLock<MetricsHistory>,
    // 实时流状态 — proxy 线程写入，UI 线程读取
    live_stream: parking_lot::Mutex<Option<LiveStream>>,
    // snapshot 缓存 — 避免每帧重新克隆所有记录
    cached_snapshot: parking_lot::Mutex<Option<(u64, Snapshot)>>,
}

struct MetricsHistory {
    records: VecDeque<HistoryRecord>,
    throughput: VecDeque<ThroughputPoint>,
    latency_history: VecDeque<LatencyPoint>,
    model_stats: HashMap<String, u64>,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            total: AtomicU64::new(0),
            success: AtomicU64::new(0),
            failed: AtomicU64::new(0),
            total_input_tokens: AtomicU64::new(0),
            total_output_tokens: AtomicU64::new(0),
            total_latency_ms: AtomicU64::new(0),
            active_streams: AtomicU64::new(0),
            start_time: Instant::now(),
            generation: AtomicU64::new(0),
            history: RwLock::new(MetricsHistory {
                records: VecDeque::with_capacity(MAX_HISTORY),
                throughput: VecDeque::with_capacity(THROUGHPUT_WINDOW),
                latency_history: VecDeque::with_capacity(MAX_HISTORY),
                model_stats: HashMap::new(),
            }),
            live_stream: parking_lot::Mutex::new(None),
            cached_snapshot: parking_lot::Mutex::new(None),
        }
    }

    pub fn record_request(
        &self,
        model: String,
        stream: bool,
        status: String,
        latency: f64,
        in_tok: u64,
        out_tok: u64,
        error: String,
        preview: String,
        req_id: Option<String>,
        input_detail: Option<InputDetail>,
    ) {
        // ── 无锁更新计数器 ──
        self.total.fetch_add(1, Ordering::Release);
        if status == "success" {
            self.success.fetch_add(1, Ordering::Release);
            self.total_input_tokens.fetch_add(in_tok, Ordering::Release);
            self.total_output_tokens.fetch_add(out_tok, Ordering::Release);
            self.total_latency_ms.fetch_add((latency * 1000.0) as u64, Ordering::Release);
        } else {
            self.failed.fetch_add(1, Ordering::Release);
        }

        // ── 构造记录（锁外IO/分配） ──
        let now = chrono::Utc::now().timestamp();
        let minute_ts = now / 60;
        let time_str = Local::now().format("%H:%M:%S").to_string();
        let truncated_preview = truncate_str(&preview, 120);

        let detail = input_detail.unwrap_or(InputDetail {
            instructions: String::new(),
            messages: vec![],
            tools: String::new(),
            params: serde_json::Value::Null,
        });

        let mut role_counts: HashMap<String, u64> = HashMap::new();
        for m in &detail.messages {
            *role_counts.entry(m.role.clone()).or_insert(0) += 1;
        }
        let mut summary_parts: Vec<String> = role_counts
            .iter()
            .map(|(r, c)| format!("{}:{}", r, c))
            .collect();
        if !detail.tools.is_empty() && detail.tools != "[]" {
            if let Ok(tools_arr) = serde_json::from_str::<serde_json::Value>(&detail.tools) {
                if let Some(arr) = tools_arr.as_array() {
                    summary_parts.push(format!("tools:{}", arr.len()));
                }
            }
        }
        let input_summary = summary_parts.join(", ");

        let last_user = detail
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| truncate_str(&m.content, 120))
            .unwrap_or_default();

        let rounded_latency = (latency * 100.0).round() / 100.0;

        let record = HistoryRecord {
            req_id: req_id.clone(),
            time: time_str,
            timestamp: now as f64,
            model: model.clone(),
            stream,
            status: status.clone(),
            latency: rounded_latency,
            input_tokens: in_tok,
            output_tokens: out_tok,
            error,
            input_summary,
            input_preview: last_user,
            input_detail: detail,
            preview: truncated_preview,
            content: preview,
        };

        // ── 仅历史数据需要写锁 ──
        let mut hist = self.history.write();
        // Throughput update
        if let Some(last) = hist.throughput.back_mut() {
            if last.t == minute_ts {
                last.c += 1;
            } else {
                hist.throughput.push_back(ThroughputPoint { t: minute_ts, c: 1 });
            }
        } else {
            hist.throughput.push_back(ThroughputPoint { t: minute_ts, c: 1 });
        }
        while hist.throughput.len() > THROUGHPUT_WINDOW {
            hist.throughput.pop_front();
        }

        hist.latency_history.push_back(LatencyPoint {
            t: now as f64,
            v: rounded_latency,
        });
        while hist.latency_history.len() > MAX_HISTORY {
            hist.latency_history.pop_front();
        }

        *hist.model_stats.entry(model).or_insert(0) += 1;
        // 限制 model_stats 条目数，防止无限增长（一次排序替代循环O(n)扫描）
        if hist.model_stats.len() > 200 {
            let mut entries: Vec<_> = hist.model_stats.drain().collect();
            entries.sort_by(|a, b| b.1.cmp(&a.1));  // 按计数降序
            entries.truncate(150);  // 保留前150
            hist.model_stats = entries.into_iter().collect();
        }

        // If req_id provided, try to update the matching streaming placeholder
        if let Some(ref rid) = req_id {
            let mut updated = false;
            for rec in hist.records.iter_mut().rev() {
                if rec.req_id.as_deref() == Some(rid.as_str()) {
                    *rec = record.clone();
                    updated = true;
                    break;
                }
            }
            if !updated {
                hist.records.push_back(record);
                while hist.records.len() > MAX_HISTORY {
                    hist.records.pop_front();
                }
            }
        } else {
            hist.records.push_back(record);
            while hist.records.len() > MAX_HISTORY {
                hist.records.pop_front();
            }
        }
        // ── 裁剪超出 MAX_FULL_DETAIL 的记录的 content 和 input_detail ──
        let len = hist.records.len();
        if len > MAX_FULL_DETAIL {
            let cutoff = len - MAX_FULL_DETAIL;
            // 修剪 content：释放大型字符串内存
            for r in hist.records.iter_mut().take(cutoff) {
                // 只释放 content，preview 和 input_detail.summary 需要保留用于列表展示
                r.content = String::new();
            }
        }
        // 同时裁剪 input_detail，保留前 MAX_FULL_DETAIL 条
        if len > MAX_FULL_DETAIL + 5 {
            // 多留 5 条 buffer，减少频繁清空
            let detail_cutoff = len.saturating_sub(MAX_FULL_DETAIL + 5);
            for r in hist.records.iter_mut().take(detail_cutoff) {
                r.input_detail = InputDetail {
                    instructions: String::new(),
                    messages: vec![],
                    tools: String::new(),
                    params: serde_json::Value::Null,
                };
            }
        }
        drop(hist);

        self.generation.fetch_add(1, Ordering::Release);
    }

    pub fn stream_start_with_model(&self, model: &str, req_id: String) {
        self.active_streams.fetch_add(1, Ordering::Release);
        *self.live_stream.lock() = Some(LiveStream {
            model: model.to_string(),
            content: String::new(),
            start_time: Instant::now(),
        });
        // Add a placeholder history entry so the UI shows the request immediately
        let now = chrono::Utc::now().timestamp();
        let time_str = Local::now().format("%H:%M:%S").to_string();
        let placeholder = HistoryRecord {
            req_id: Some(req_id),
            time: time_str,
            timestamp: now as f64,
            model: model.to_string(),
            stream: true,
            status: "streaming".to_string(),
            latency: 0.0,
            input_tokens: 0,
            output_tokens: 0,
            error: String::new(),
            input_summary: String::new(),
            input_preview: String::new(),
            input_detail: InputDetail {
                instructions: String::new(),
                messages: vec![],
                tools: String::new(),
                params: serde_json::Value::Null,
            },
            preview: String::new(),
            content: String::new(),
        };
        let mut hist = self.history.write();
        hist.records.push_back(placeholder);
        while hist.records.len() > MAX_HISTORY {
            hist.records.pop_front();
        }
        self.generation.fetch_add(1, Ordering::Release);
    }

    pub fn stream_append(&self, delta: &str) {
        if let Some(ref mut live) = *self.live_stream.lock() {
            live.content.push_str(delta);
        }
        // ponytail: 不增加 generation —— 内容追加不应使主快照缓存失效
    }

    pub fn stream_end(&self) {
        // Clear live stream state
        *self.live_stream.lock() = None;
        // clamp to 0
        let prev = self.active_streams.fetch_sub(1, Ordering::AcqRel);
        if prev == 0 {
            self.active_streams.store(0, Ordering::Release);
        }
        self.generation.fetch_add(1, Ordering::Release);
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Returns true if there is an in-progress streaming response.
    /// Used by the UI to decide whether to request frequent repaints.
    pub fn has_active_stream(&self) -> bool {
        self.live_stream.lock().is_some()
    }

    pub fn snapshot(&self) -> Snapshot {
        let gen = self.generation();

        // 先检查缓存 — 无重计算开销
        {
            let mut cache = self.cached_snapshot.lock();
            if let Some((cached_gen, ref cached_snap)) = *cache {
                if cached_gen == gen {
                    // 缓存命中：如果 live_stream 活动，只刷新 live_stream 部分
                    let live = self.live_stream.lock();
                    if let Some(ref live_inner) = *live {
                        let live_snap = Some(LiveStreamSnapshot {
                            model: live_inner.model.clone(),
                            accumulated: live_inner.content.clone(),
                            elapsed_secs: live_inner.start_time.elapsed().as_secs_f64(),
                        });
                        drop(live);
                        let mut snap = cached_snap.clone();
                        snap.live_stream = live_snap;
                        // 更新缓存中的 live_stream
                        *cache = Some((gen, snap.clone()));
                        return snap;
                    }
                    // 没有活动流，直接返回缓存的快照
                    return cached_snap.clone();
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

        let avg_latency = if success > 0 {
            total_latency_ms as f64 / 1000.0 / success as f64
        } else {
            0.0
        };

        let hist = self.history.read();

        let now = chrono::Utc::now().timestamp();
        let recent_count: u64 = hist
            .throughput
            .iter()
            .filter(|tp| now - tp.t * 60 < 300)
            .map(|tp| tp.c)
            .sum();
        let recent_minutes = hist
            .throughput
            .iter()
            .filter(|tp| now - tp.t * 60 < 300)
            .count()
            .max(1) as f64;
        let rpm = recent_count as f64 / recent_minutes.min(5.0).max(1.0);

        // Capture current live stream (if any) — brief lock
        let live_stream = self.live_stream.lock().as_ref().map(|live| LiveStreamSnapshot {
            model: live.model.clone(),
            accumulated: live.content.clone(),
            elapsed_secs: live.start_time.elapsed().as_secs_f64(),
        });

        let snap = Snapshot {
            uptime,
            total,
            success,
            failed,
            active_streams,
            avg_latency: (avg_latency * 100.0).round() / 100.0,
            rpm: (rpm * 10.0).round() / 10.0,
            total_input_tokens: total_input,
            total_output_tokens: total_output,
            total_tokens: total_input + total_output,
            history: Arc::new(hist.records.iter().map(|r| r.to_meta()).collect()),
            throughput: Arc::new(hist.throughput.iter().cloned().collect()),
            latency_history: Arc::new(hist.latency_history.iter().cloned().collect()),
            model_stats: Arc::new(hist.model_stats.clone()),
            live_stream,
        };

        // 更新缓存
        *self.cached_snapshot.lock() = Some((gen, snap.clone()));
        snap
    }

    /// 获取某条历史记录的完整内容（用于详情弹窗），避免在 snapshot 中克隆所有 content
    pub fn get_full_content(&self, idx: usize) -> Option<String> {
        let hist = self.history.read();
        hist.records.get(idx).map(|r| r.content.clone())
    }

    /// 获取某条历史记录的 input_detail（用于详情弹窗）
    pub fn get_input_detail(&self, idx: usize) -> Option<InputDetail> {
        let hist = self.history.read();
        hist.records.get(idx).map(|r| r.input_detail.clone())
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

pub type SharedMetrics = Arc<Metrics>;
