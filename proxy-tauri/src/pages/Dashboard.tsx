import { useMemo, useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Activity, CheckCircle, XCircle, Clock, Zap, Coins, Wifi, WifiOff, Globe, ArrowRight } from 'lucide-react';
import { LineChart, Line, BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, AreaChart, Area } from 'recharts';
import { useApp } from '@/contexts/AppContext';
import { useMetrics } from '@/contexts/MetricsContext';
import { t } from '@/lib/i18n';
import { cn, formatNumber, formatLatency, formatUptime } from '@/lib/utils';
import type { CurrentConfig } from '@/lib/types';

function StatCard({ icon: Icon, label, value, sub, colorClass, accentClass }: {
  icon: typeof Activity;
  label: string;
  value: string;
  sub?: string;
  colorClass: string;
  accentClass: string;
}) {
  return (
    <div className={cn(
      "bg-bg-card border border-border rounded-lg p-2.5 hover:border-accent/50 transition-all duration-200 hover:-translate-y-0.5",
      colorClass
    )}>
      <div className="flex items-center gap-1 mb-0.5">
        <Icon className="w-2.5 h-2.5 text-text-3" />
        <span className="text-[9px] text-text-3 uppercase tracking-wider font-semibold">{label}</span>
      </div>
      <div className={cn("text-xl font-bold font-mono leading-tight", accentClass)}>{value}</div>
      {sub && <div className="text-[10px] text-text-2 mt-0.5 font-mono">{sub}</div>}
    </div>
  );
}

export default function Dashboard() {
  const { lang, proxyRunning } = useApp();
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

      {/* Proxy Status */}
      <div className="bg-bg-card border border-border rounded-lg p-3">
        <h2 className="text-xs font-semibold text-text-1 mb-2 flex items-center gap-2">
          <Globe className="w-3.5 h-3.5 text-blue" />
          {t(lang, 'config_status')}
        </h2>
        {currentConfig && (
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
              <span className="text-text-1 font-mono">http://127.0.0.1:8000/v1 <span className="text-text-3">({t(lang, 'config_via')})</span></span>
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

      {/* Charts Row 1 */}
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
              <XAxis dataKey="i" hide />
              <YAxis tick={{ fontSize: 10, fill: 'var(--text-3)' }} width={30} />
              <Tooltip contentStyle={{ background: 'var(--bg-elev)', border: '1px solid var(--border)', borderRadius: 6, fontSize: 11 }} />
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
              <XAxis dataKey="i" hide />
              <YAxis tick={{ fontSize: 10, fill: 'var(--text-3)' }} width={40} />
              <Tooltip contentStyle={{ background: 'var(--bg-elev)', border: '1px solid var(--border)', borderRadius: 6, fontSize: 11 }} />
              <Line type="monotone" dataKey="v" stroke="var(--yellow)" strokeWidth={1.5} dot={false} />
            </LineChart>
          </ResponsiveContainer>
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

      {/* Live Stream */}
      {snapshot?.live_stream && (
        <div className="bg-bg-card border border-accent/30 rounded-lg p-3">
          <div className="flex items-center gap-2 mb-2">
            <div className="w-1.5 h-1.5 rounded-full bg-accent animate-pulse-dot" />
            <span className="text-[10px] text-text-3 uppercase tracking-wider font-semibold">
              {t(lang, 'live_stream')}
            </span>
            <span className="text-[10px] text-text-3 font-mono ml-auto">
              {snapshot.live_stream.model} · {snapshot.live_stream.elapsed_secs.toFixed(1)}s
            </span>
          </div>
          <pre className="text-xs font-mono text-text-2 whitespace-pre-wrap max-h-32 overflow-auto leading-relaxed">
            {snapshot.live_stream.accumulated || <span className="text-text-3 italic">Waiting for content...</span>}
          </pre>
        </div>
      )}
    </div>
  );
}
