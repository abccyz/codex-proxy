use chrono::{Local, NaiveDate, Datelike};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const MAX_RETENTION_DAYS: i64 = 90;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyRecord {
    pub date: String,          // "2026-07-13-14" (date-hour)
    pub model: String,
    pub requests: u64,
    pub success: u64,
    pub failed: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_latency_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AggregatedStat {
    pub period: String,
    pub requests: u64,
    pub success: u64,
    pub failed: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub avg_latency_ms: f64,
    #[serde(skip)]
    total_latency_ms: f64,  // internal accumulator
    pub models: Vec<(String, u64)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalSummary {
    pub total_requests: u64,
    pub total_success: u64,
    pub total_failed: u64,
    pub success_rate: f64,        // 0.0–100.0
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tokens: u64,
    pub avg_latency_ms: f64,
    pub active_days: u64,
    pub unique_models: u64,
}

pub struct HistoryStore {
    path: PathBuf,
    records: Mutex<Vec<DailyRecord>>,
}

impl HistoryStore {
    pub fn new(data_dir: &PathBuf) -> Self {
        let path = data_dir.join("history.json");
        let records = Self::load_from_file(&path);
        let store = Self {
            path,
            records: Mutex::new(records),
        };
        store.cleanup_old();
        store
    }

    fn load_from_file(path: &PathBuf) -> Vec<DailyRecord> {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_to_file(&self) {
        let records = self.records.lock();
        if let Ok(json) = serde_json::to_string_pretty(&*records) {
            let _ = fs::write(&self.path, json);
        }
    }

    /// Remove records older than MAX_RETENTION_DAYS
    fn cleanup_old(&self) {
        let cutoff = Local::now().naive_local().date() - chrono::Duration::days(MAX_RETENTION_DAYS);
        let cutoff_str = cutoff.format("%Y-%m-%d").to_string();
        let mut records = self.records.lock();
        let before = records.len();
        records.retain(|r| r.date >= cutoff_str);
        if records.len() != before {
            drop(records);
            self.save_to_file();
        }
    }

    /// Append or merge a record for a single request (hourly granularity)
    pub fn record_request(
        &self,
        model: &str,
        status: &str,
        latency_secs: f64,
        input_tokens: u64,
        output_tokens: u64,
    ) {
        let period = Local::now().format("%Y-%m-%d-%H").to_string();
        let latency_ms = (latency_secs * 1000.0) as u64;
        let is_success = status == "success";

        let mut records = self.records.lock();

        // Find existing record for this hour + model
        if let Some(existing) = records.iter_mut().find(|r| r.date == period && r.model == model) {
            existing.requests += 1;
            if is_success {
                existing.success += 1;
            } else {
                existing.failed += 1;
            }
            existing.input_tokens += input_tokens;
            existing.output_tokens += output_tokens;
            existing.total_latency_ms += latency_ms;
        } else {
            records.push(DailyRecord {
                date: period,
                model: model.to_string(),
                requests: 1,
                success: if is_success { 1 } else { 0 },
                failed: if is_success { 0 } else { 1 },
                input_tokens,
                output_tokens,
                total_latency_ms: latency_ms,
            });
        }

        drop(records);
        self.save_to_file();
    }

    /// Get aggregated stats by dimension, filtered by current time range:
    /// - hour: last 12 hours (from current time, counting backwards)
    /// - day: this month
    /// - week: last 4 weeks
    /// - month: this year
    /// - year: all time
    pub fn get_stats(&self, dimension: &str) -> Vec<AggregatedStat> {
        let records = self.records.lock().clone();
        let now = Local::now();

        // Hour dimension: special handling for last 12 hours with empty slot filling
        if dimension == "hour" {
            return self.get_hourly_stats(&records, &now);
        }

        // Filter records by time range based on dimension
        let filtered: Vec<DailyRecord> = match dimension {
            "day" => {
                let this_month = now.format("%Y-%m").to_string();
                records.into_iter().filter(|r| r.date.starts_with(&this_month)).collect()
            }
            "week" => {
                let cutoff = now.naive_local().date() - chrono::Duration::days(28);
                let cutoff_str = cutoff.format("%Y-%m-%d").to_string();
                records.into_iter().filter(|r| {
                    let date_part: String = r.date.chars().take(10).collect();
                    date_part >= cutoff_str
                }).collect()
            }
            "month" => {
                let this_year = now.format("%Y").to_string();
                records.into_iter().filter(|r| r.date.starts_with(&this_year)).collect()
            }
            _ => records, // "year" or unknown: all data
        };

        match dimension {
            "day" => aggregate_by(&filtered, |r| {
                r.date.chars().take(10).collect::<String>()
            }),
            "week" => aggregate_by(&filtered, |r| {
                let date_str: String = r.date.chars().take(10).collect();
                if let Ok(d) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
                    let iso = d.iso_week();
                    format!("{}-W{:02}", iso.year(), iso.week())
                } else {
                    date_str
                }
            }),
            "month" => aggregate_by(&filtered, |r| {
                r.date.chars().take(7).collect::<String>()
            }),
            "year" => aggregate_by(&filtered, |r| {
                r.date.chars().take(4).collect::<String>()
            }),
            _ => aggregate_by(&filtered, |r| r.date.clone()),
        }
    }

    /// Generate last 12 hours of aggregated stats, filling empty slots with zeros.
    fn get_hourly_stats(&self, records: &[DailyRecord], now: &chrono::DateTime<Local>) -> Vec<AggregatedStat> {
        // Generate last 12 hour slots (from 11 hours ago to current hour)
        let slots: Vec<String> = (0..12)
            .map(|i| {
                let dt = *now - chrono::Duration::hours(11 - i);
                dt.format("%Y-%m-%d-%H").to_string()
            })
            .collect();

        // Filter records that fall into these slots
        let filtered: Vec<DailyRecord> = records
            .iter()
            .filter(|r| slots.contains(&r.date))
            .cloned()
            .collect();

        let mut result = aggregate_by(&filtered, |r| r.date.clone());

        // Fill missing slots with zero entries
        for slot in &slots {
            if !result.iter().any(|s| s.period == *slot) {
                result.push(AggregatedStat {
                    period: slot.clone(),
                    requests: 0,
                    success: 0,
                    failed: 0,
                    input_tokens: 0,
                    output_tokens: 0,
                    avg_latency_ms: 0.0,
                    total_latency_ms: 0.0,
                    models: Vec::new(),
                });
            }
        }

        result.sort_by(|a, b| a.period.cmp(&b.period));
        result
    }

    /// Compute global all-time summary across all records
    pub fn global_summary(&self) -> GlobalSummary {
        let records = self.records.lock().clone();
        let total_requests: u64 = records.iter().map(|r| r.requests).sum();
        let total_success: u64 = records.iter().map(|r| r.success).sum();
        let total_failed: u64 = records.iter().map(|r| r.failed).sum();
        let total_input_tokens: u64 = records.iter().map(|r| r.input_tokens).sum();
        let total_output_tokens: u64 = records.iter().map(|r| r.output_tokens).sum();
        let total_latency_ms: u64 = records.iter().map(|r| r.total_latency_ms).sum();
        let active_days = {
            let mut days = std::collections::HashSet::new();
            for r in &records { days.insert(r.date.chars().take(10).collect::<String>()); }
            days.len() as u64
        };
        let mut models = std::collections::HashSet::new();
        for r in &records { models.insert(&r.model); }
        let success_rate = if total_requests > 0 {
            (total_success as f64 / total_requests as f64) * 100.0
        } else { 0.0 };
        let avg_latency_ms = if total_requests > 0 {
            total_latency_ms as f64 / total_requests as f64
        } else { 0.0 };

        GlobalSummary {
            total_requests,
            total_success,
            total_failed,
            success_rate: (success_rate * 10.0).round() / 10.0,
            total_input_tokens,
            total_output_tokens,
            total_tokens: total_input_tokens + total_output_tokens,
            avg_latency_ms: (avg_latency_ms * 10.0).round() / 10.0,
            active_days,
            unique_models: models.len() as u64,
        }
    }
}

fn aggregate_by<F>(records: &[DailyRecord], key_fn: F) -> Vec<AggregatedStat>
where
    F: Fn(&DailyRecord) -> String,
{
    let mut groups: HashMap<String, AggregatedStat> = HashMap::new();

    for r in records {
        let period = key_fn(r);
        let entry = groups.entry(period.clone()).or_insert_with(|| AggregatedStat {
            period,
            requests: 0,
            success: 0,
            failed: 0,
            input_tokens: 0,
            output_tokens: 0,
            avg_latency_ms: 0.0,
            total_latency_ms: 0.0,
            models: Vec::new(),
        });

        entry.requests += r.requests;
        entry.success += r.success;
        entry.failed += r.failed;
        entry.input_tokens += r.input_tokens;
        entry.output_tokens += r.output_tokens;
        entry.total_latency_ms += r.total_latency_ms as f64;

        // Accumulate model counts
        if let Some((_, count)) = entry.models.iter_mut().find(|(m, _)| m == &r.model) {
            *count += r.requests;
        } else {
            entry.models.push((r.model.clone(), r.requests));
        }
    }

    // Compute avg latency and sort
    let mut result: Vec<AggregatedStat> = groups.into_values().collect();
    for s in &mut result {
        if s.requests > 0 {
            s.avg_latency_ms = s.total_latency_ms / s.requests as f64;
        }
        // Sort models by count descending
        s.models.sort_by(|a, b| b.1.cmp(&a.1));
    }

    // Sort by period ascending
    result.sort_by(|a, b| a.period.cmp(&b.period));
    result
}
