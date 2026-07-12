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
