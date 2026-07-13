import { useMemo, useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Activity, CheckCircle, XCircle, Clock, Zap, Coins, Wifi, WifiOff, Globe, ArrowRight, Radio, LogOut, Play } from 'lucide-react';
import { LineChart, Line, BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, AreaChart, Area } from 'recharts';
import ReactMarkdown from 'react-markdown';
import { useApp } from '@/contexts/AppContext';
import { useMetrics } from '@/contexts/MetricsContext';
import { t } from '@/lib/i18n';
import { cn, formatNumber, formatLatency, formatUptime } from '@/lib/utils';
import StatCard from '@/components/StatCard';
import type { CurrentConfig, TaskItem } from '@/lib/types';

export default function Dashboard() {
  const { lang, proxyRunning, setProxyRunning } = useApp();
  const { snapshot } = useMetrics();

  const [currentConfig, setCurrentConfig] = useState<CurrentConfig | null>(null);
  const [upstreamUrl, setUpstreamUrl] = useState('');
  const [upstreamModel, setUpstreamModel] = useState('');

  useEffect(() => {
    const fetchConfig = async () => {
      try {
        const [cc, ui] = await Promise.all([
          invoke<CurrentConfig>('get_current_config'),
          invoke<{ url: string; model: string }>('get_upstream_info'),
        ]);
        setCurrentConfig(cc);
        setUpstreamUrl(ui.url);
        setUpstreamModel(ui.model);
      } catch {}
    };
    fetchConfig();
  }, []);

  const doBypassProxy = async () => {
    try {
      await invoke('bypass_proxy');
      setProxyRunning(false);
      const [cc, ui] = await Promise.all([
        invoke<CurrentConfig>('get_current_config'),
        invoke<{ url: string; model: string }>('get_upstream_info'),
      ]);
      setCurrentConfig(cc);
      setUpstreamUrl(ui.url);
      setUpstreamModel(ui.model);
    } catch {}
  };

  const doStartProxy = async () => {
    try {
      await invoke('start_proxy');
      setProxyRunning(true);
      const [cc, ui] = await Promise.all([
        invoke<CurrentConfig>('get_current_config'),
        invoke<{ url: string; model: string }>('get_upstream_info'),
      ]);
      setCurrentConfig(cc);
      setUpstreamUrl(ui.url);
      setUpstreamModel(ui.model);
    } catch {}
  };

  const stats = useMemo(() => {
    if (!snapshot) return null;
    const s = snapshot;
    return {
      total: formatNumber(s.total),
      success: formatNumber(s.success),
      failed: formatNumber(s.failed),
      avgLatency: formatLatency(s.avg_latency * 1000),
      activeStreams: String(s.active_streams),
      totalTokens: formatNumber(s.total_tokens),
      uptime: formatUptime(s.uptime),
      rpm: s.rpm.toFixed(1),
    };
  }, [snapshot]);

  const throughputData = useMemo(() => {
    if (!snapshot) return [];
    return snapshot.throughput.slice(-30).map((p, i) => ({
      i,
      v: p.c,
      label: new Date(p.t * 60 * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
    }));
  }, [snapshot]);

  const latencyData = useMemo(() => {
    if (!snapshot) return [];
    return snapshot.latency_history.slice(-50).map((p, i) => ({
      i,
      v: +(p.v * 1000).toFixed(0),
      label: new Date(p.t * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
    }));
  }, [snapshot]);

  const modelData = useMemo(() => {
    if (!snapshot) return [];
    return Object.entries(snapshot.model_stats).map(([name, count]) => ({ name, count }));
  }, [snapshot]);

  const tokenData = useMemo(() => {
    if (!snapshot) return [];
    return [
      { name: t(lang, 'token_input'), tokens: snapshot.total_input_tokens },
      { name: t(lang, 'token_output'), tokens: snapshot.total_output_tokens },
    ];
  }, [snapshot, lang]);

  return (
    <div className="h-full overflow-auto p-3 space-y-2.5">
      {/* Stats Row */}
      <div className="grid grid-cols-6 gap-2">
        <StatCard icon={Activity} label={t(lang, 'stat_total')} value={stats?.total ?? '—'} colorClass="" accentClass="text-blue" />
        <StatCard icon={CheckCircle} label={t(lang, 'stat_success')} value={stats?.success ?? '—'} colorClass="" accentClass="text-accent" />
        <StatCard icon={XCircle} label={t(lang, 'stat_failed')} value={stats?.failed ?? '—'} colorClass="" accentClass="text-red" />
        <StatCard icon={Clock} label={t(lang, 'stat_latency')} value={stats?.avgLatency ?? '—'} colorClass="" accentClass="text-yellow" />
        <StatCard icon={Zap} label={t(lang, 'stat_streams')} value={stats?.activeStreams ?? '—'} colorClass="" accentClass="text-purple" />
        <StatCard icon={Coins} label={t(lang, 'stat_tokens')} value={stats?.totalTokens ?? '—'} colorClass="" accentClass="text-accent" />
      </div>

      {/* Info bar */}
      {snapshot && (
        <div className="flex items-center gap-4 text-[11px] text-text-2 font-mono">
          <span>{t(lang, 'uptime')}: {formatUptime(snapshot.uptime)}</span>
          <span>{t(lang, 'rpm')}: {snapshot.rpm.toFixed(1)}</span>
          <span className="flex items-center gap-1">
            {proxyRunning ? <Wifi className="w-3 h-3 text-accent" /> : <WifiOff className="w-3 h-3 text-red" />}
            {proxyRunning ? t(lang, 'running') : t(lang, 'stopped')}
          </span>
        </div>
      )}

      {/* Proxy Status + Live Stream */}
      <div className="grid grid-cols-2 gap-2">
        {/* Proxy Status - half width */}
        <div className="bg-bg-card border border-border rounded-lg p-3">
          <div className="flex items-center justify-between mb-2">
            <h2 className="text-xs font-semibold text-text-1 flex items-center gap-2">
              <Globe className="w-3.5 h-3.5 text-blue" />
              {t(lang, 'config_status')}
            </h2>
            {proxyRunning ? (
              <button
                onClick={doBypassProxy}
                className="px-2 py-1 rounded-lg border text-[11px] font-medium inline-flex items-center gap-1 transition-all bg-bg-elev border-border hover:border-text-3 text-text-2 hover:text-text-1"
                title={t(lang, 'config_bypass_desc')}
              >
                <LogOut className="w-3 h-3" />
                {t(lang, 'config_bypass')}
              </button>
            ) : (
              <button
                onClick={doStartProxy}
                className="px-2 py-1 rounded-lg text-[11px] font-medium inline-flex items-center gap-1 transition-all bg-accent/10 text-accent hover:bg-accent hover:text-white"
                title={t(lang, 'config_start_proxy')}
              >
                <Play className="w-3 h-3" />
                {t(lang, 'config_start_proxy')}
              </button>
            )}
          </div>
          {currentConfig?.model && (
            <div className="space-y-1 text-xs">
              <div className="flex gap-2">
                <span className="text-text-3 w-20">{t(lang, 'config_model')}:</span>
                <span className="text-text-1 font-semibold font-mono">{currentConfig.model}</span>
              </div>
              <div className="flex gap-2">
                <span className="text-text-3 w-20">{t(lang, 'config_preset')}:</span>
                <span className="text-text-1 font-mono">{currentConfig.provider}</span>
              </div>
              <div className="flex gap-2">
                <span className="text-text-3 w-20">{t(lang, 'config_base_url')}:</span>
                <span className="text-text-1 font-mono flex items-center gap-1">
                  {currentConfig.base_url}
                  {currentConfig.base_url.includes('127.0.0.1') ? (
                    <span className="text-text-3">({t(lang, 'config_via')})</span>
                  ) : (
                    <span className="text-green">({t(lang, 'config_bypassed')})</span>
                  )}
                </span>
              </div>
              <div className="border-t border-border pt-1.5 mt-1">
                <div className="flex items-center gap-1.5 text-text-3 text-[10px] mb-1 font-semibold">
                  <ArrowRight className="w-3 h-3 text-accent" /> {t(lang, 'upstream_title')}
                </div>
                <div className="flex gap-2">
                  <span className="text-text-3 w-20">{t(lang, 'upstream_url')}:</span>
                  <span className="text-text-1 font-mono text-[11px]">{upstreamUrl}</span>
                </div>
                <div className="flex gap-2">
                  <span className="text-text-3 w-20">{t(lang, 'upstream_model')}:</span>
                  <span className="text-accent font-semibold font-mono">{upstreamModel}</span>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Session - half width */}
        <div className="bg-bg-card border border-border rounded-lg p-3">
          <div className="flex items-center gap-2 mb-2">
            {snapshot?.live_stream?.finished ? (
              <CheckCircle className="w-3.5 h-3.5 text-text-3" />
            ) : (
              <Radio className={cn("w-3.5 h-3.5", snapshot?.live_stream ? "text-accent animate-pulse" : "text-text-3")} />
            )}
            <span className="text-xs font-semibold text-text-1">{t(lang, 'live_stream')}</span>
            {snapshot?.live_stream && (
              <span className="text-[10px] text-text-3 font-mono ml-auto">
                {snapshot.live_stream.model}
              </span>
            )}
            {snapshot?.live_stream?.accumulated && (
              <button
                onClick={() => invoke('clear_session')}
                className="text-[10px] text-text-3 hover:text-accent transition-colors ml-1"
                title={t(lang, 'clear_session')}
              >
                {t(lang, 'clear_session')}
              </button>
            )}
          </div>
          {snapshot?.live_stream?.accumulated ? (
            <>
              {snapshot.live_stream.tasks.length > 0 && (
                <TaskProgress tasks={snapshot.live_stream.tasks} />
              )}
              <LiveStreamMarkdown content={snapshot.live_stream.accumulated} />
            </>
          ) : (
            <div className="text-xs text-text-3 py-6 text-center italic">Waiting for session...</div>
          )}

        </div>
      </div>

      {/* Runtime Charts */}
      <div>
        <div className="text-[10px] text-text-3 uppercase tracking-wider font-semibold mb-1.5">运行时</div>
        <div className="grid grid-cols-2 gap-2">
          <div className="bg-bg-card border border-border rounded-lg p-2.5">
            <div className="text-[10px] text-text-3 uppercase tracking-wider font-semibold mb-1">
              {t(lang, 'chart_throughput')}
            </div>
          <ResponsiveContainer width="100%" height={110}>
            <AreaChart data={throughputData}>
              <defs>
                <linearGradient id="gradThroughput" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="var(--blue)" stopOpacity={0.3} />
                  <stop offset="95%" stopColor="var(--blue)" stopOpacity={0} />
                </linearGradient>
              </defs>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
              <XAxis dataKey="label" tick={{ fontSize: 9, fill: 'var(--text-3)' }} interval="preserveStartEnd" />
              <YAxis tick={{ fontSize: 10, fill: 'var(--text-3)' }} width={30} />
              <Tooltip contentStyle={{ background: 'var(--bg-elev)', border: '1px solid var(--border)', borderRadius: 6, fontSize: 11 }} labelFormatter={(l) => l} formatter={(v) => [`${v} req/min`, t(lang, 'chart_throughput')]} />
              <Area type="monotone" dataKey="v" stroke="var(--blue)" fill="url(#gradThroughput)" strokeWidth={1.5} dot={false} />
            </AreaChart>
          </ResponsiveContainer>
        </div>

        <div className="bg-bg-card border border-border rounded-lg p-2.5">
          <div className="text-[10px] text-text-3 uppercase tracking-wider font-semibold mb-1">
            {t(lang, 'chart_latency')} (ms)
          </div>
          <ResponsiveContainer width="100%" height={110}>
            <LineChart data={latencyData}>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
              <XAxis dataKey="label" tick={{ fontSize: 9, fill: 'var(--text-3)' }} interval="preserveStartEnd" />
              <YAxis tick={{ fontSize: 10, fill: 'var(--text-3)' }} width={40} />
              <Tooltip contentStyle={{ background: 'var(--bg-elev)', border: '1px solid var(--border)', borderRadius: 6, fontSize: 11 }} labelFormatter={(l) => l} formatter={(v) => [`${v} ms`, t(lang, 'chart_latency')]} />
              <Line type="monotone" dataKey="v" stroke="var(--yellow)" strokeWidth={1.5} dot={false} />
            </LineChart>
          </ResponsiveContainer>
        </div>
      </div>
      </div>

      {/* Charts Row 2 */}
      <div className="grid grid-cols-2 gap-2">
        <div className="bg-bg-card border border-border rounded-lg p-2.5">
          <div className="text-[10px] text-text-3 uppercase tracking-wider font-semibold mb-1">
            {t(lang, 'chart_models')}
          </div>
          <ResponsiveContainer width="100%" height={110}>
            <BarChart data={modelData}>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
              <XAxis dataKey="name" tick={{ fontSize: 9, fill: 'var(--text-3)' }} angle={-20} textAnchor="end" height={40} />
              <YAxis tick={{ fontSize: 10, fill: 'var(--text-3)' }} width={30} />
              <Tooltip contentStyle={{ background: 'var(--bg-elev)', border: '1px solid var(--border)', borderRadius: 6, fontSize: 11 }} />
              <Bar dataKey="count" fill="var(--purple)" radius={[3, 3, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </div>

        <div className="bg-bg-card border border-border rounded-lg p-2.5">
          <div className="text-[10px] text-text-3 uppercase tracking-wider font-semibold mb-1">
            {t(lang, 'chart_tokens')}
          </div>
          <ResponsiveContainer width="100%" height={110}>
            <BarChart data={tokenData} layout="vertical">
              <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
              <XAxis type="number" tick={{ fontSize: 10, fill: 'var(--text-3)' }} />
              <YAxis type="category" dataKey="name" tick={{ fontSize: 10, fill: 'var(--text-2)' }} width={50} />
              <Tooltip contentStyle={{ background: 'var(--bg-elev)', border: '1px solid var(--border)', borderRadius: 6, fontSize: 11 }} />
              <Bar dataKey="tokens" fill="var(--accent)" radius={[0, 3, 3, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </div>
      </div>
    </div>
  );
}

/**
 * Task progress bar for session panel.
 * Displays progress when tasks are detected in the content.
 */
function TaskProgress({ tasks }: { tasks: TaskItem[] }) {
  const done = tasks.filter(t => t.done).length;
  const total = tasks.length;
  const percent = total > 0 ? Math.round((done / total) * 100) : 0;

  return (
    <div className="mb-2 p-2 bg-bg-elev/50 border border-border rounded">
      <div className="flex items-center justify-between mb-1">
        <span className="text-[10px] text-text-3 font-semibold uppercase tracking-wider">Task Progress</span>
        <span className="text-[10px] font-mono text-text-2">{done}/{total} ({percent}%)</span>
      </div>
      <div className="h-1.5 bg-bg-card rounded-full overflow-hidden">
        <div
          className="h-full bg-accent transition-all duration-300"
          style={{ width: `${percent}%` }}
        />
      </div>
      <div className="mt-1.5 space-y-0.5 max-h-20 overflow-auto">
        {tasks.map((task, i) => (
          <div key={i} className="flex items-start gap-1.5 text-[10px]">
            <span className={task.done ? 'text-accent' : 'text-text-3'}>
              {task.done ? '✓' : '○'}
            </span>
            <span className={task.done ? 'text-text-3 line-through' : 'text-text-2'}>
              {task.text}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

/**
 * Throttled Markdown renderer for live stream.
 * Updates at most every 100ms to avoid excessive re-renders during streaming.
 */
const MAX_DISPLAY_CHARS = 4000; // Limit displayed content for Markdown rendering performance

function LiveStreamMarkdown({ content }: { content: string }) {
  const [displayed, setDisplayed] = useState(content);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastUpdateRef = useRef(0);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const now = Date.now();
    const elapsed = now - lastUpdateRef.current;
    const throttleMs = 100;

    const truncated = content.length > MAX_DISPLAY_CHARS
      ? '...\n' + content.slice(content.length - MAX_DISPLAY_CHARS)
      : content;

    if (elapsed >= throttleMs) {
      setDisplayed(truncated);
      lastUpdateRef.current = now;
    } else {
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => {
        setDisplayed(truncated);
        lastUpdateRef.current = Date.now();
      }, throttleMs - elapsed);
    }
  }, [content]);

  // Auto-scroll to bottom when content updates
  useEffect(() => {
    const el = scrollRef.current;
    if (el) {
      el.scrollTop = el.scrollHeight;
    }
  }, [displayed]);

  return (
    <div ref={scrollRef} className="max-h-48 overflow-auto text-xs text-text-2 leading-relaxed">
      <div className="markdown-body">
        <ReactMarkdown
          components={{
            pre: ({ children }) => (
              <pre className="whitespace-pre-wrap break-all">{children}</pre>
            ),
            blockquote: ({ children }) => (
              <blockquote className="border-l-2 border-accent/60 pl-2 text-text-3 text-[11px] my-1">{children}</blockquote>
            ),
            hr: () => (
              <hr className="border-0 my-2"
                style={{
                  background: 'repeating-linear-gradient(90deg, var(--border) 0, var(--border) 6px, transparent 6px, transparent 10px)',
                  height: '1px',
                  maskImage: 'linear-gradient(to right, transparent 0%, black 30%, black 70%, transparent 100%)',
                  WebkitMaskImage: 'linear-gradient(to right, transparent 0%, black 30%, black 70%, transparent 100%)',
                }} />
            ),
          }}
        >
          {displayed || 'Waiting for content...'}
        </ReactMarkdown>
      </div>
    </div>
  );
}
