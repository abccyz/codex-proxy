import { useApp } from '@/contexts/AppContext';
import { t } from '@/lib/i18n';
import { Globe, Server, Cpu, Shield, Zap, Brain, Github, Package, ExternalLink } from 'lucide-react';

const APP_VERSION = '0.2.0';

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
  const { lang } = useApp();

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
            v{APP_VERSION}
          </span>
          <span className="text-text-3 text-xs">{t(lang, 'about_subtitle')}</span>
        </div>
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
