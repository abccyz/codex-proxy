import { useState, useEffect } from 'react';
import { useApp } from '@/contexts/AppContext';
import { t } from '@/lib/i18n';
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { open } from '@tauri-apps/plugin-shell';
import { Globe, Server, Cpu, Shield, Zap, Brain, Github, Package, ExternalLink, RefreshCw, Download, CheckCircle } from 'lucide-react';
import type { VersionInfo } from '@/lib/types';

const features = [
  { icon: Zap, key: 'about_feat_proxy' },
  { icon: Brain, key: 'about_feat_catalog' },
  { icon: Server, key: 'about_feat_dashboard' },
  { icon: Shield, key: 'about_feat_secure' },
];

const techStack = [
  { icon: Package, label: 'Frontend', value: 'React 18 + TypeScript + Tailwind CSS' },
  { icon: Package, label: 'Backend', value: 'Rust + Tauri 2.x + Axum' },
  { icon: Package, label: 'Runtime', value: 'WebView2 (Windows) / WKWebView (macOS)' },
  { icon: Package, label: 'DB', value: 'SQLite (rusqlite) + Fernet 加密' },
];

export default function About() {
  const { lang, hasUpdate, setHasUpdate, latestVersion, setLatestVersion } = useApp();
  const [appVersion, setAppVersion] = useState('');
  const [checking, setChecking] = useState(false);
  const [checkResult, setCheckResult] = useState<'idle' | 'up_to_date' | 'has_update'>('idle');
  const [releaseUrl, setReleaseUrl] = useState('');

  useEffect(() => {
    getVersion().then(setAppVersion).catch(() => setAppVersion('0.0.0'));
  }, []);

  const handleCheckUpdate = async () => {
    setChecking(true);
    setCheckResult('idle');
    try {
      const result = await invoke<VersionInfo | null>('check_for_update');
      if (result && result.has_update) {
        setHasUpdate(true);
        setLatestVersion(result.latest_version);
        setReleaseUrl(result.release_url);
        setCheckResult('has_update');
      } else if (result && !result.has_update) {
        setCheckResult('up_to_date');
      }
      // If result is null (network failure), silently do nothing
    } catch {
      // Silently ignore errors
    } finally {
      setChecking(false);
    }
  };

  const handleDownload = () => {
    if (releaseUrl) {
      open(releaseUrl).catch(() => {});
    }
  };

  return (
    <div className="h-full overflow-auto p-4 space-y-4">
      {/* Header */}
      <div className="bg-bg-card border border-border rounded-lg p-5 text-center">
        <div className="w-12 h-12 rounded-xl bg-accent/10 inline-flex items-center justify-center mb-3">
          <Globe className="w-6 h-6 text-accent" />
        </div>
        <h1 className="text-lg font-bold text-text-1">ProxyTauri</h1>
        <div className="flex items-center justify-center gap-2 mt-1">
          <span className="px-2 py-0.5 bg-accent/10 text-accent text-xs font-semibold rounded">
            v{appVersion}
          </span>
          <span className="text-text-3 text-xs">{t(lang, 'about_subtitle')}</span>
        </div>
      </div>

      {/* Version Check */}
      <div className="bg-bg-card border border-border rounded-lg p-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <RefreshCw className="w-3.5 h-3.5 text-text-3" />
            <span className="text-xs font-semibold text-text-1">{t(lang, 'about_check_update')}</span>
          </div>
          <button
            onClick={handleCheckUpdate}
            disabled={checking}
            className="px-3 py-1 rounded text-xs font-medium bg-accent/10 text-accent hover:bg-accent/20 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {checking ? t(lang, 'about_checking') : t(lang, 'about_check_update')}
          </button>
        </div>

        {/* Check Result */}
        {checkResult === 'has_update' && latestVersion && (
          <div className="mt-3 flex items-center justify-between p-2.5 rounded-lg bg-green/5 border border-green/20">
            <div className="flex items-center gap-2">
              <Download className="w-3.5 h-3.5 text-green" />
              <span className="text-xs text-text-1">
                {t(lang, 'about_update_available')} <span className="font-semibold">v{latestVersion}</span>
              </span>
            </div>
            <button
              onClick={handleDownload}
              className="px-2 py-0.5 rounded text-[11px] font-medium bg-green/10 text-green hover:bg-green/20 transition-colors"
            >
              {t(lang, 'about_download')}
            </button>
          </div>
        )}

        {checkResult === 'up_to_date' && !checking && (
          <div className="mt-3 flex items-center gap-2 p-2.5 rounded-lg bg-bg-elev border border-border">
            <CheckCircle className="w-3.5 h-3.5 text-text-3" />
            <span className="text-xs text-text-3">{t(lang, 'about_up_to_date')}</span>
          </div>
        )}
      </div>

      {/* Features */}
      <div className="bg-bg-card border border-border rounded-lg p-4">
        <h2 className="text-xs font-semibold text-text-1 mb-3 flex items-center gap-2">
          <Zap className="w-3.5 h-3.5 text-yellow" />
          {t(lang, 'about_features')}
        </h2>
        <div className="grid grid-cols-2 gap-3">
          {features.map(({ icon: Icon, key }) => (
            <div key={key} className="p-3 rounded-lg bg-bg-elev border border-border">
              <div className="flex items-center gap-2 mb-1.5">
                <Icon className="w-4 h-4 text-accent" />
                <span className="text-xs font-semibold text-text-1">
                  {t(lang, `${key}_title`)}
                </span>
              </div>
              <p className="text-[10px] text-text-3 leading-relaxed">
                {t(lang, `${key}_desc`)}
              </p>
            </div>
          ))}
        </div>
      </div>

      {/* Tech Stack */}
      <div className="bg-bg-card border border-border rounded-lg p-4">
        <h2 className="text-xs font-semibold text-text-1 mb-3 flex items-center gap-2">
          <Cpu className="w-3.5 h-3.5 text-blue" />
          {t(lang, 'about_tech')}
        </h2>
        <div className="space-y-2">
          {techStack.map(({ icon: Icon, label, value }) => (
            <div key={label} className="flex items-center gap-3 p-2 rounded-lg bg-bg-elev border border-border">
              <div className="w-7 h-7 rounded-md bg-accent/10 flex items-center justify-center flex-shrink-0">
                <Icon className="w-3.5 h-3.5 text-accent" />
              </div>
              <div className="min-w-0">
                <div className="text-[10px] text-text-3 font-medium">{label}</div>
                <div className="text-[11px] text-text-1 font-mono">{value}</div>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Repository */}
      <div className="bg-bg-card border border-border rounded-lg p-4">
        <h2 className="text-xs font-semibold text-text-1 mb-3 flex items-center gap-2">
          <Github className="w-3.5 h-3.5" />
          {t(lang, 'about_repo')}
        </h2>
        <a
          href="https://github.com/abccyz/codex-proxy"
          target="_blank"
          rel="noreferrer"
          className="flex items-center gap-2 p-2.5 rounded-lg bg-bg-elev border border-border hover:border-accent/40 transition-colors group"
        >
          <Github className="w-4 h-4 text-text-3 group-hover:text-accent transition-colors" />
          <span className="text-xs text-text-2 font-mono group-hover:text-text-1 transition-colors">
            github.com/abccyz/codex-proxy
          </span>
          <ExternalLink className="w-3 h-3 text-text-4 ml-auto group-hover:text-accent transition-colors" />
        </a>
      </div>
    </div>
  );
}
