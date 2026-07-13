import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { LayoutDashboard, History, Settings, Sun, Moon, Languages, Activity, Wifi, WifiOff, ChevronDown, Check, Info } from 'lucide-react';
import { useApp } from '@/contexts/AppContext';
import { useMetrics } from '@/contexts/MetricsContext';
import { t } from '@/lib/i18n';
import { cn } from '@/lib/utils';
import Dashboard from '@/pages/Dashboard';
import HistoryPage from '@/pages/History';
import Config from '@/pages/Config';
import About from '@/pages/About';
import type { SavedConfig } from '@/lib/types';

type Tab = 'dashboard' | 'history' | 'config' | 'about';

const tabs: { id: Tab; icon: typeof LayoutDashboard; labelKey: string }[] = [
  { id: 'dashboard', icon: LayoutDashboard, labelKey: 'tab_dashboard' },
  { id: 'history', icon: History, labelKey: 'tab_history' },
  { id: 'config', icon: Settings, labelKey: 'tab_config' },
  { id: 'about', icon: Info, labelKey: 'tab_about' },
];

const PRESET_NAMES: Record<string, string> = {
  openai: 'OpenAI',
  deepseek: 'DeepSeek',
  qwen: 'Qwen',
  ollama: 'Ollama',
  lmstudio: 'LM Studio',
};

export default function App() {
  const { theme, setTheme, lang, setLang, proxyRunning, bumpConfigVersion } = useApp();
  const { snapshot } = useMetrics();
  const [activeTab, setActiveTab] = useState<Tab>('dashboard');

  // Model switcher state
  const [savedConfigs, setSavedConfigs] = useState<SavedConfig[]>([]);
  const [upstreamModel, setUpstreamModel] = useState('');
  const [upstreamProvider, setUpstreamProvider] = useState('');
  const [showSwitcher, setShowSwitcher] = useState(false);
  const switcherRef = useRef<HTMLDivElement>(null);

  const refreshConfigs = useCallback(async () => {
    try {
      const [sc, cc] = await Promise.all([
        invoke<SavedConfig[]>('get_saved_configs'),
        invoke<{ model: string; provider: string }>('get_current_config'),
      ]);
      setSavedConfigs(sc);
      setUpstreamModel(cc.model);
      setUpstreamProvider(cc.provider);
    } catch {}
  }, []);

  useEffect(() => { refreshConfigs(); }, [refreshConfigs]);

  // Close dropdown on click outside
  useEffect(() => {
    if (!showSwitcher) return;
    const handleClick = (e: MouseEvent) => {
      if (switcherRef.current && !switcherRef.current.contains(e.target as Node)) {
        setShowSwitcher(false);
      }
    };
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, [showSwitcher]);

  const doSwitchConfig = async (name: string) => {
    try {
      await invoke('apply_config', { name });
      setShowSwitcher(false);
      refreshConfigs();
      bumpConfigVersion();
    } catch {}
  };

  return (
    <div className="flex flex-col h-screen">
      {/* Header */}
      <header className="flex items-center justify-between px-5 py-2.5 border-b border-border bg-bg-card flex-shrink-0">
        <div className="flex items-center gap-3">
          <div className={cn(
            "w-2 h-2 rounded-full",
            proxyRunning ? "bg-accent shadow-[0_0_6px_var(--accent)] animate-pulse-dot" : "bg-red"
          )} />
          <h1 className="text-sm font-bold tracking-tight text-text-1">
            ProxyTauri
            <span className="text-text-3 font-normal text-[11px] ml-1.5">
              {proxyRunning ? t(lang, 'running') : t(lang, 'stopped')}
            </span>
          </h1>
          {snapshot && (
            <span className="text-text-3 text-[11px] font-mono hidden sm:inline">
              ↑{snapshot.uptime}s · {snapshot.rpm.toFixed(1)} {t(lang, 'rpm')}
            </span>
          )}
        </div>

        <nav className="flex items-center gap-1 bg-bg-elev rounded-lg p-0.5">
          {tabs.map(tab => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={cn(
                "px-3 py-1.5 rounded-md text-xs font-medium transition-all duration-150 inline-flex items-center gap-1.5",
                activeTab === tab.id
                  ? "bg-bg-card text-text-1 shadow-sm"
                  : "text-text-2 hover:text-text-1"
              )}
            >
              <tab.icon className="w-3.5 h-3.5" />
              {t(lang, tab.labelKey)}
            </button>
          ))}
        </nav>

        <div className="flex items-center gap-1">
          {/* Model Switcher */}
          {proxyRunning && upstreamModel && (
            <div ref={switcherRef} className="relative">
              <button
                onClick={() => { refreshConfigs(); setShowSwitcher(v => !v); }}
                className={cn(
                  "px-2 py-1 rounded text-[11px] font-medium transition-all inline-flex items-center gap-1",
                  "bg-bg-elev border border-border text-text-2 hover:text-text-1 hover:border-text-3"
                )}
              >
                <span className="font-mono max-w-[120px] truncate">{upstreamModel.includes('/') ? upstreamModel.split('/').slice(1).join('/') : upstreamModel}</span>
                <ChevronDown className={cn("w-3 h-3 transition-transform", showSwitcher && "rotate-180")} />
              </button>

              {showSwitcher && (
                <div className="absolute right-0 top-full mt-1 w-64 bg-bg-card border border-border rounded-lg shadow-xl z-50 overflow-hidden">
                  <div className="px-3 py-2 text-[10px] text-text-4 uppercase tracking-wider font-semibold border-b border-border">
                    {t(lang, 'config_connected_providers')}
                  </div>
                  <div className="max-h-60 overflow-auto py-1">
                    {savedConfigs.length === 0 ? (
                      <div className="px-3 py-3 text-[11px] text-text-3 text-center">
                        {t(lang, 'config_no_providers')}
                      </div>
                    ) : (
                      savedConfigs.map(cfg => (
                        <button
                          key={cfg.name}
                          onClick={() => doSwitchConfig(cfg.name)}
                          className="w-full px-3 py-2 text-left text-xs transition-colors hover:bg-bg-elev flex items-center gap-2"
                        >
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-1.5">
                              <span className="font-medium text-text-1 truncate">
                                {PRESET_NAMES[cfg.provider] ?? cfg.provider}
                              </span>
                              {cfg.model === upstreamModel && (
                                <Check className="w-3 h-3 text-accent flex-shrink-0" />
                              )}
                            </div>
                            <div className="text-[10px] text-text-3 font-mono truncate">{cfg.model}</div>
                          </div>
                        </button>
                      ))
                    )}
                  </div>
                  <div className="border-t border-border px-2 py-1.5">
                    <button
                      onClick={() => { setShowSwitcher(false); setActiveTab('config'); }}
                      className="w-full px-2 py-1 text-[10px] text-text-3 hover:text-text-1 text-center transition-colors rounded"
                    >
                      {t(lang, 'tab_config')} →
                    </button>
                  </div>
                </div>
              )}
            </div>
          )}

          <button
            onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
            className="p-1.5 rounded-md text-text-2 hover:text-text-1 hover:bg-bg-elev transition-colors"
            title={theme === 'dark' ? t(lang, 'theme_light') : t(lang, 'theme_dark')}
          >
            {theme === 'dark' ? <Sun className="w-3.5 h-3.5" /> : <Moon className="w-3.5 h-3.5" />}
          </button>
          <button
            onClick={() => setLang(lang === 'zh' ? 'en' : 'zh')}
            className="p-1.5 rounded-md text-text-2 hover:text-text-1 hover:bg-bg-elev transition-colors"
            title={lang === 'zh' ? 'English' : '中文'}
          >
            <Languages className="w-3.5 h-3.5" />
          </button>
          {proxyRunning ? (
            <Activity className="w-3.5 h-3.5 text-accent ml-1" />
          ) : (
            <WifiOff className="w-3.5 h-3.5 text-red ml-1" />
          )}
        </div>
      </header>

      {/* Content */}
      <main className="flex-1 overflow-hidden">
        {activeTab === 'dashboard' && <Dashboard />}
        {activeTab === 'history' && <HistoryPage />}
        {activeTab === 'config' && <Config />}
        {activeTab === 'about' && <About />}
      </main>
    </div>
  );
}

