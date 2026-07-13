import { useState, useMemo, useCallback, useEffect } from 'react';
import { Search, ChevronLeft, ChevronRight, X, FileText, Activity, CheckCircle, Clock, Coins, CalendarDays, Layers, Zap } from 'lucide-react';
import * as Tabs from '@radix-ui/react-tabs';
import { BarChart, Bar, LineChart, Line, AreaChart, Area, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, PieChart, Pie, Cell } from 'recharts';
import { useApp } from '@/contexts/AppContext';
import { useMetrics } from '@/contexts/MetricsContext';
import { t } from '@/lib/i18n';
import { cn, formatLatency, formatNumber } from '@/lib/utils';
import { invoke } from '@tauri-apps/api/core';
import StatCard from '@/components/StatCard';
import type { InputDetail, AggregatedStat, StatsDimension, GlobalSummary } from '@/lib/types';

const PIE_COLORS = ['var(--accent)', 'var(--blue)', 'var(--purple)', 'var(--yellow)', 'var(--green)', 'var(--red)', 'var(--text-3)'];

// ── History List Tab (existing functionality, preserved) ──

function HistoryListTab() {
  const { lang } = useApp();
  const { snapshot } = useMetrics();
  const [search, setSearch] = useState('');
  const [page, setPage] = useState(0);
  const [selectedIdx, setSelectedIdx] = useState<number | null>(null);
  const [selectedDetail, setSelectedDetail] = useState<InputDetail | null>(null);
  const pageSize = 15;

  const filtered = useMemo(() => {
    if (!snapshot) return [];
    const q = search.toLowerCase();
    return [...snapshot.history].reverse().filter(r => !q || r.model.toLowerCase().includes(q) || r.preview.toLowerCase().includes(q));
  }, [snapshot, search]);

  const paged = useMemo(() => filtered.slice(page * pageSize, (page + 1) * pageSize), [filtered, page]);
  const totalPages = Math.max(1, Math.ceil(filtered.length / pageSize));

  const openDetail = useCallback(async (idx: number) => {
    setSelectedIdx(idx);
    try {
      const detail = await invoke<InputDetail>('get_history_detail', { idx });
      setSelectedDetail(detail);
    } catch { setSelectedDetail(null); }
  }, []);

  return (
    <div className="h-full flex flex-col overflow-hidden">
      {/* Search */}
      <div className="flex items-center gap-2 mb-3 flex-shrink-0">
        <div className="relative flex-1 max-w-xs">
          <Search className="w-3.5 h-3.5 absolute left-2.5 top-1/2 -translate-y-1/2 text-text-3" />
          <input
            value={search}
            onChange={e => { setSearch(e.target.value); setPage(0); }}
            placeholder={t(lang, 'history_search')}
            className="w-full pl-7 pr-3 py-1.5 bg-bg-input border border-border rounded-md text-xs text-text-1 placeholder:text-text-3 focus:outline-none focus:border-accent transition-colors"
          />
        </div>
        <span className="text-[10px] text-text-3">{filtered.length} records</span>
      </div>

      {/* Table */}
      <div className="flex-1 overflow-auto rounded-lg border border-border">
        <table className="w-full text-xs">
          <thead className="sticky top-0 bg-bg-elev z-10">
            <tr className="text-text-3 uppercase tracking-wider text-[10px]">
              <th className="text-left px-3 py-2 font-semibold">{t(lang, 'th_time')}</th>
              <th className="text-left px-3 py-2 font-semibold">{t(lang, 'th_model')}</th>
              <th className="text-left px-3 py-2 font-semibold">{t(lang, 'th_status')}</th>
              <th className="text-left px-3 py-2 font-semibold">{t(lang, 'th_latency')}</th>
              <th className="text-left px-3 py-2 font-semibold">{t(lang, 'th_tokens')}</th>
              <th className="text-left px-3 py-2 font-semibold">Preview</th>
            </tr>
          </thead>
          <tbody>
            {paged.map((r, i) => (
              <tr
                key={`${r.time}-${i}`}
                onClick={() => openDetail(page * pageSize + i)}
                className={cn(
                  "border-t border-border hover:bg-blue-bg/20 cursor-pointer transition-colors",
                  selectedIdx === page * pageSize + i && "bg-blue-bg/40"
                )}
              >
                <td className="px-3 py-1.5 font-mono text-text-2 whitespace-nowrap">{r.time}</td>
                <td className="px-3 py-1.5 font-mono text-text-1 max-w-[120px] truncate">{r.model}</td>
                <td className="px-3 py-1.5">
                  <span className={cn(
                    "px-1.5 py-0.5 rounded text-[10px] font-semibold",
                    r.status === 'success' ? "bg-green-bg text-green" : "bg-red-bg text-red"
                  )}>
                    {r.status === 'success' ? 'OK' : 'ERR'}
                  </span>
                </td>
                <td className="px-3 py-1.5 font-mono text-text-2">{formatLatency(r.latency * 1000)}</td>
                <td className="px-3 py-1.5 font-mono text-text-2">
                  {r.input_tokens}/{r.output_tokens}
                </td>
                <td className="px-3 py-1.5 text-text-2 max-w-[250px] truncate">{r.preview}</td>
              </tr>
            ))}
            {filtered.length === 0 && (
              <tr>
                <td colSpan={6} className="px-3 py-10 text-center text-text-3">
                  {search ? t(lang, 'history_no_results') : t(lang, 'history_empty')}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {/* Pagination */}
      <div className="flex items-center justify-between pt-2 flex-shrink-0">
        <span className="text-[10px] text-text-3">
          {page * pageSize + 1}–{Math.min((page + 1) * pageSize, filtered.length)} / {filtered.length}
        </span>
        <div className="flex items-center gap-1">
          <button onClick={() => setPage(p => Math.max(0, p - 1))} disabled={page === 0}
            className="p-1 rounded text-text-3 hover:text-text-1 disabled:opacity-30">
            <ChevronLeft className="w-3.5 h-3.5" />
          </button>
          <span className="text-[10px] text-text-3 font-mono">{page + 1}/{totalPages}</span>
          <button onClick={() => setPage(p => Math.min(totalPages - 1, p + 1))} disabled={page >= totalPages - 1}
            className="p-1 rounded text-text-3 hover:text-text-1 disabled:opacity-30">
            <ChevronRight className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Detail panel */}
      {selectedDetail && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40" onClick={() => { setSelectedIdx(null); setSelectedDetail(null); }}>
          <div className="bg-bg-card border border-border rounded-lg w-[700px] max-h-[80vh] flex flex-col shadow-2xl" onClick={e => e.stopPropagation()}>
            <div className="flex items-center justify-between px-4 py-3 border-b border-border flex-shrink-0">
              <div className="flex items-center gap-2">
                <FileText className="w-4 h-4 text-accent" />
                <span className="text-sm font-semibold">{t(lang, 'detail_title')}</span>
              </div>
              <button onClick={() => { setSelectedIdx(null); setSelectedDetail(null); }}
                className="p-1 rounded hover:bg-bg-elev text-text-2 hover:text-text-1">
                <X className="w-4 h-4" />
              </button>
            </div>
            <div className="flex-1 overflow-auto p-4 space-y-3">
              {selectedDetail.instructions && (
                <div>
                  <div className="text-[10px] text-text-3 uppercase mb-1 font-semibold">{t(lang, 'detail_instructions')}</div>
                  <pre className="text-xs font-mono text-text-2 bg-bg-input p-2 rounded whitespace-pre-wrap max-h-40 overflow-auto">{selectedDetail.instructions}</pre>
                </div>
              )}
              {selectedDetail.messages.length > 0 && (
                <div>
                  <div className="text-[10px] text-text-3 uppercase mb-1 font-semibold">{t(lang, 'detail_messages')} ({selectedDetail.messages.length})</div>
                  <div className="space-y-1.5 max-h-60 overflow-auto">
                    {selectedDetail.messages.map((m, i) => (
                      <div key={i} className="bg-bg-input p-2 rounded text-xs">
                        <span className="text-accent font-semibold">{m.role}</span>
                        <pre className="text-text-2 mt-0.5 whitespace-pre-wrap">{m.content || '(empty)'}</pre>
                      </div>
                    ))}
                  </div>
                </div>
              )}
              {selectedDetail.tools && selectedDetail.tools !== '[]' && (
                <div>
                  <div className="text-[10px] text-text-3 uppercase mb-1 font-semibold">{t(lang, 'detail_tools')}</div>
                  <pre className="text-xs font-mono text-text-2 bg-bg-input p-2 rounded whitespace-pre-wrap max-h-40 overflow-auto">{selectedDetail.tools}</pre>
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// ── Usage Stats Tab (new) ──

function GlobalCards({ summary, lang: lang_ }: { summary: GlobalSummary | null; lang: string }) {
  if (!summary || summary.total_requests === 0) return null;
  return (
    <div className="grid grid-cols-4 gap-2">
      <StatCard
        icon={Activity}
        label="总请求数"
        value={formatNumber(summary.total_requests)}
        sub={`${summary.total_success} OK / ${summary.total_failed} ERR`}
        accentClass="text-blue"
      />
      <StatCard
        icon={CheckCircle}
        label="成功率"
        value={`${summary.success_rate}%`}
        accentClass="text-accent"
      />
      <StatCard
        icon={Coins}
        label="总 Token"
        value={formatNumber(summary.total_tokens)}
        sub={`入 ${formatNumber(summary.total_input_tokens)} / 出 ${formatNumber(summary.total_output_tokens)}`}
        accentClass="text-purple"
      />
      <StatCard
        icon={Zap}
        label="使用概况"
        value={`${summary.active_days}d · ${summary.unique_models}m`}
        sub={`均延 ${formatLatency(summary.avg_latency_ms)}`}
        accentClass="text-yellow"
      />
    </div>
  );
}

const DIMENSIONS: { id: StatsDimension; labelKey: string }[] = [
  { id: 'hour', labelKey: 'stats_dimension_hour' },
  { id: 'day', labelKey: 'stats_dimension_day' },
  { id: 'week', labelKey: 'stats_dimension_week' },
  { id: 'month', labelKey: 'stats_dimension_month' },
  { id: 'year', labelKey: 'stats_dimension_year' },
];

function UsageStatsTab() {
  const { lang } = useApp();
  const [dimension, setDimension] = useState<StatsDimension>('day');
  const [stats, setStats] = useState<AggregatedStat[]>([]);
  const [globalSummary, setGlobalSummary] = useState<GlobalSummary | null>(null);
  const [loading, setLoading] = useState(false);

  // Fetch global summary once on mount
  useEffect(() => {
    invoke<GlobalSummary>('get_global_summary')
      .then(s => setGlobalSummary(s))
      .catch(() => {});
  }, []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    invoke<AggregatedStat[]>('get_history_stats', { dimension })
      .then(data => { if (!cancelled) setStats(data); })
      .catch(() => { if (!cancelled) setStats([]); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [dimension]);

  const summary = useMemo(() => {
    if (stats.length === 0) return null;
    const total = stats.reduce((s, r) => s + r.requests, 0);
    const success = stats.reduce((s, r) => s + r.success, 0);
    const inputTokens = stats.reduce((s, r) => s + r.input_tokens, 0);
    const outputTokens = stats.reduce((s, r) => s + r.output_tokens, 0);
    const avgLat = total > 0
      ? stats.reduce((s, r) => s + r.avg_latency_ms * r.requests, 0) / total
      : 0;
    return { total, success, failed: total - success, inputTokens, outputTokens, avgLat };
  }, [stats]);

  const trendData = useMemo(() => {
    return stats.map(s => ({
      period: formatPeriodLabel(s.period, dimension),
      requests: s.requests,
      success: s.success,
      failed: s.failed,
      input_tokens: s.input_tokens,
      output_tokens: s.output_tokens,
      avg_latency_ms: +(s.avg_latency_ms.toFixed(0)),
    }));
  }, [stats, dimension]);

  const modelData = useMemo(() => {
    const map = new Map<string, number>();
    for (const s of stats) {
      for (const [name, count] of s.models) {
        map.set(name, (map.get(name) ?? 0) + count);
      }
    }
    return Array.from(map.entries())
      .map(([name, count]) => ({ name, count }))
      .sort((a, b) => b.count - a.count)
      .slice(0, 10);
  }, [stats]);

  if (loading) {
    return (
      <div className="h-full overflow-auto space-y-3">
        <GlobalCards summary={globalSummary} lang={lang} />
        <div className="flex items-center justify-center h-32 text-text-3 text-xs">Loading...</div>
      </div>
    );
  }

  if (stats.length === 0) {
    return (
      <div className="h-full overflow-auto space-y-3">
        <GlobalCards summary={globalSummary} lang={lang} />
        <div className="flex items-center justify-center h-32 text-text-3 text-xs">{t(lang, 'stats_no_data')}</div>
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto space-y-3">
      {/* Global summary cards - all-time totals */}
      <GlobalCards summary={globalSummary} lang={lang} />

      {/* Dimension selector */}
      <div className="flex items-center gap-1 bg-bg-elev rounded-lg p-0.5 w-fit">
        {DIMENSIONS.map(d => (
          <button
            key={d.id}
            onClick={() => setDimension(d.id)}
            className={cn(
              "px-2.5 py-1 rounded-md text-[11px] font-medium transition-all duration-150",
              dimension === d.id
                ? "bg-bg-card text-text-1 shadow-sm"
                : "text-text-2 hover:text-text-1"
            )}
          >
            {t(lang, d.labelKey)}
          </button>
        ))}
      </div>

      {/* Summary cards */}
      {summary && (
        <div className="grid grid-cols-4 gap-2">
          <StatCard icon={Activity} label={t(lang, 'stats_total_requests')} value={formatNumber(summary.total)} accentClass="text-blue" />
          <StatCard icon={CheckCircle} label={t(lang, 'stats_success_rate')} value={`${summary.total > 0 ? Math.round((summary.success / summary.total) * 100) : 0}%`} sub={`${summary.success}/${summary.total}`} accentClass="text-accent" />
          <StatCard icon={Clock} label={t(lang, 'stats_avg_latency')} value={formatLatency(summary.avgLat)} accentClass="text-yellow" />
          <StatCard icon={Coins} label={t(lang, 'stats_total_tokens')} value={formatNumber(summary.inputTokens + summary.outputTokens)} sub={`${t(lang, 'token_input')}: ${formatNumber(summary.inputTokens)} / ${t(lang, 'token_output')}: ${formatNumber(summary.outputTokens)}`} accentClass="text-accent" />
        </div>
      )}

      {/* Charts */}
      <div className="grid grid-cols-2 gap-2">
        <ChartCard title={t(lang, 'stats_request_trend')}>
          <ResponsiveContainer width="100%" height={160}>
            <BarChart data={trendData}>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
              <XAxis dataKey="period" tick={{ fontSize: 9, fill: 'var(--text-3)' }} interval="preserveStartEnd" />
              <YAxis tick={{ fontSize: 10, fill: 'var(--text-3)' }} width={30} />
              <Tooltip contentStyle={{ background: 'var(--bg-elev)', border: '1px solid var(--border)', borderRadius: 6, fontSize: 11 }} />
              <Bar dataKey="success" stackId="a" fill="var(--accent)" radius={[0, 0, 0, 0]} />
              <Bar dataKey="failed" stackId="a" fill="var(--red)" radius={[3, 3, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title={t(lang, 'stats_token_trend')}>
          <ResponsiveContainer width="100%" height={160}>
            <AreaChart data={trendData}>
              <defs>
                <linearGradient id="gradInput" x1="0" y1="0" x2="0" y2="1"><stop offset="5%" stopColor="var(--blue)" stopOpacity={0.3} /><stop offset="95%" stopColor="var(--blue)" stopOpacity={0} /></linearGradient>
                <linearGradient id="gradOutput" x1="0" y1="0" x2="0" y2="1"><stop offset="5%" stopColor="var(--accent)" stopOpacity={0.3} /><stop offset="95%" stopColor="var(--accent)" stopOpacity={0} /></linearGradient>
              </defs>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
              <XAxis dataKey="period" tick={{ fontSize: 9, fill: 'var(--text-3)' }} interval="preserveStartEnd" />
              <YAxis tick={{ fontSize: 10, fill: 'var(--text-3)' }} width={40} />
              <Tooltip contentStyle={{ background: 'var(--bg-elev)', border: '1px solid var(--border)', borderRadius: 6, fontSize: 11 }} />
              <Area type="monotone" dataKey="input_tokens" stroke="var(--blue)" fill="url(#gradInput)" strokeWidth={1.5} name={t(lang, 'token_input')} />
              <Area type="monotone" dataKey="output_tokens" stroke="var(--accent)" fill="url(#gradOutput)" strokeWidth={1.5} name={t(lang, 'token_output')} />
            </AreaChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title={t(lang, 'stats_latency_trend') + ' (ms)'}>
          <ResponsiveContainer width="100%" height={160}>
            <LineChart data={trendData}>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
              <XAxis dataKey="period" tick={{ fontSize: 9, fill: 'var(--text-3)' }} interval="preserveStartEnd" />
              <YAxis tick={{ fontSize: 10, fill: 'var(--text-3)' }} width={40} />
              <Tooltip contentStyle={{ background: 'var(--bg-elev)', border: '1px solid var(--border)', borderRadius: 6, fontSize: 11 }} />
              <Line type="monotone" dataKey="avg_latency_ms" stroke="var(--yellow)" strokeWidth={2} dot={{ r: 2 }} />
            </LineChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title={t(lang, 'stats_model_distribution')}>
          <ResponsiveContainer width="100%" height={160}>
            {modelData.length <= 5 ? (
              <PieChart>
                <Pie data={modelData} dataKey="count" nameKey="name" cx="50%" cy="50%" outerRadius={55} label={({ name, percent }) => `${name} ${(percent * 100).toFixed(0)}%`} labelLine={false}>
                  {modelData.map((_, i) => (<Cell key={i} fill={PIE_COLORS[i % PIE_COLORS.length]} />))}
                </Pie>
                <Tooltip contentStyle={{ background: 'var(--bg-elev)', border: '1px solid var(--border)', borderRadius: 6, fontSize: 11 }} />
              </PieChart>
            ) : (
              <BarChart data={modelData} layout="vertical">
                <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
                <XAxis type="number" tick={{ fontSize: 10, fill: 'var(--text-3)' }} />
                <YAxis type="category" dataKey="name" tick={{ fontSize: 9, fill: 'var(--text-3)' }} width={80} />
                <Tooltip contentStyle={{ background: 'var(--bg-elev)', border: '1px solid var(--border)', borderRadius: 6, fontSize: 11 }} />
                <Bar dataKey="count" fill="var(--purple)" radius={[0, 3, 3, 0]} />
              </BarChart>
            )}
          </ResponsiveContainer>
        </ChartCard>
      </div>
    </div>
  );
}

// ── Shared components ──

function ChartCard({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="bg-bg-card border border-border rounded-lg p-2.5">
      <div className="text-[10px] text-text-3 uppercase tracking-wider font-semibold mb-1">{title}</div>
      {children}
    </div>
  );
}

function formatPeriodLabel(period: string, dimension: StatsDimension): string {
  switch (dimension) {
    case 'hour': return period.slice(11) + ':00';   // "14:00" from "2026-07-13-14"
    case 'day': return period.slice(5);              // "07-13"
    case 'week': return period.slice(6);              // "W28" from "2026-W28"
    case 'month': return period.slice(5);             // "07" from "2026-07"
    case 'year': return period;                       // "2026"
    default: return period;
  }
}

// ── Main History Page ──

export default function HistoryPage() {
  const { lang } = useApp();
  const [activeTab, setActiveTab] = useState('list');

  return (
    <Tabs.Root value={activeTab} onValueChange={setActiveTab} className="h-full flex flex-col p-4 overflow-hidden">
      <Tabs.List className="flex items-center gap-1 bg-bg-elev rounded-lg p-0.5 w-fit mb-3 flex-shrink-0">
        <Tabs.Trigger
          value="list"
          className={cn(
            "px-3 py-1.5 rounded-md text-xs font-medium transition-all duration-150 inline-flex items-center gap-1.5",
            activeTab === 'list' ? "bg-bg-card text-text-1 shadow-sm" : "text-text-2 hover:text-text-1"
          )}
        >
          <Search className="w-3 h-3" />
          {t(lang, 'history_tab_list')}
        </Tabs.Trigger>
        <Tabs.Trigger
          value="stats"
          className={cn(
            "px-3 py-1.5 rounded-md text-xs font-medium transition-all duration-150 inline-flex items-center gap-1.5",
            activeTab === 'stats' ? "bg-bg-card text-text-1 shadow-sm" : "text-text-2 hover:text-text-1"
          )}
        >
          <Activity className="w-3 h-3" />
          {t(lang, 'history_tab_stats')}
        </Tabs.Trigger>
      </Tabs.List>

      <Tabs.Content value="list" className="flex-1 overflow-hidden outline-none">
        <HistoryListTab />
      </Tabs.Content>

      <Tabs.Content value="stats" className="flex-1 overflow-hidden outline-none">
        <UsageStatsTab />
      </Tabs.Content>
    </Tabs.Root>
  );
}
