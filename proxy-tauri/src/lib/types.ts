export interface HistoryMeta {
  req_id: string | null;
  time: string;
  timestamp: number;
  model: string;
  stream: boolean;
  status: string;
  latency: number;
  input_tokens: number;
  output_tokens: number;
  error: string;
  input_summary: string;
  input_preview: string;
  preview: string;
}

export interface TaskItem {
  text: string;
  done: boolean;
}

export interface LiveStreamSnapshot {
  model: string;
  accumulated: string;
  elapsed_secs: number;
  finished: boolean;
  tasks: TaskItem[];
}

export interface Snapshot {
  uptime: number;
  total: number;
  success: number;
  failed: number;
  active_streams: number;
  avg_latency: number;
  rpm: number;
  total_input_tokens: number;
  total_output_tokens: number;
  total_tokens: number;
  history: HistoryMeta[];
  throughput: { t: number; c: number }[];
  latency_history: { t: number; v: number }[];
  model_stats: Record<string, number>;
  live_stream: LiveStreamSnapshot | null;
}

export interface CurrentConfig {
  model: string;
  provider: string;
  base_url: string;
}

export interface SavedConfig {
  id: number;
  name: string;
  model: string;
  provider: string;
  base_url: string;
  api_key_masked: string;
  created_at: number;
  updated_at: number;
}

export interface ConnectivityResult {
  success: boolean;
  models: string[];
  error_message: string | null;
  latency_ms: number;
}

export interface InputDetail {
  instructions: string;
  messages: { role: string; content: string; tool_calls?: unknown; tool_call_id?: string }[];
  tools: string;
  params: unknown;
}

export interface CatalogProvider {
  id: string;
  name: string;
  api: string | null;
  model_count: number;
  npm: string | null;
  models: CatalogModel[];
}

export interface CatalogModel {
  id: string;
  name: string;
  description: string | null;
  family: string | null;
  tool_call: boolean;
  reasoning: boolean;
  attachment: boolean;
  context: number | null;
  output: number | null;
  release_date: string | null;
  open_weights: boolean | null;
  cost_input: number | null;
  cost_output: number | null;
}

export interface ModelCatalog {
  providers: CatalogProvider[];
}

export interface AggregatedStat {
  period: string;
  requests: number;
  success: number;
  failed: number;
  input_tokens: number;
  output_tokens: number;
  avg_latency_ms: number;
  models: [string, number][];
}

export interface GlobalSummary {
  total_requests: number;
  total_success: number;
  total_failed: number;
  success_rate: number;
  total_input_tokens: number;
  total_output_tokens: number
  total_tokens: number;
  avg_latency_ms: number;
  active_days: number;
  unique_models: number;
}

export interface VersionInfo {
  current_version: string;
  latest_version: string;
  has_update: boolean;
  release_url: string;
  release_notes: string;
}

export type StatsDimension = 'hour' | 'day' | 'week' | 'month' | 'year';
