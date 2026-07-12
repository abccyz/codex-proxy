import { useState } from 'react';
import { LayoutDashboard, History, Settings, Sun, Moon, Languages, Activity, Wifi, WifiOff } from 'lucide-react';
import { useApp } from '@/contexts/AppContext';
import { useMetrics } from '@/contexts/MetricsContext';
import { t } from '@/lib/i18n';
import { cn } from '@/lib/utils';
import Dashboard from '@/pages/Dashboard';
import HistoryPage from '@/pages/History';
import Config from '@/pages/Config';

type Tab = 'dashboard' | 'history' | 'config';

const tabs: { id: Tab; icon: typeof LayoutDashboard; labelKey: string }[] = [
  { id: 'dashboard', icon: LayoutDashboard, labelKey: 'tab_dashboard' },
  { id: 'history', icon: History, labelKey: 'tab_history' },
  { id: 'config', icon: Settings, labelKey: 'tab_config' },
];

export default function App() {
  const { theme, setTheme, lang, setLang, proxyRunning } = useApp();
  const { snapshot } = useMetrics();
  const [activeTab, setActiveTab] = useState<Tab>('dashboard');

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
      </main>
    </div>
  );
}
