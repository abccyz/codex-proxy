import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Check, X, Trash2, Save, Play, Server, ArrowRight, Globe, Key, Cpu } from 'lucide-react';
import { useApp } from '@/contexts/AppContext';
import { t } from '@/lib/i18n';
import { cn } from '@/lib/utils';
import type { SavedConfig, CurrentConfig, ConnectivityResult } from '@/lib/types';

const PRESETS = [
  { name: 'openai', display: 'OpenAI', url: 'https://api.openai.com/v1', model: 'gpt-4o' },
  { name: 'deepseek', display: 'DeepSeek', url: 'https://api.deepseek.com/v1', model: 'deepseek-chat' },
  { name: 'qwen', display: 'Qwen (DashScope)', url: 'https://dashscope.aliyuncs.com/compatible-mode/v1', model: 'qwen-plus' },
  { name: 'ollama', display: 'Ollama (Local)', url: 'http://localhost:11434/v1', model: 'llama3' },
  { name: 'lmstudio', display: 'LM Studio (Local)', url: 'http://localhost:1234/v1', model: 'local-model' },
];

export default function Config() {
  const { lang } = useApp();

  const [currentConfig, setCurrentConfig] = useState<CurrentConfig | null>(null);
  const [savedConfigs, setSavedConfigs] = useState<SavedConfig[]>([]);
  const [upstreamUrl, setUpstreamUrl] = useState('');
  const [upstreamModel, setUpstreamModel] = useState('');

  const [preset, setPreset] = useState('qwen');
  const [baseUrl, setBaseUrl] = useState('https://dashscope.aliyuncs.com/compatible-mode/v1');
  const [apiKey, setApiKey] = useState('');
  const [model, setModel] = useState('qwen-plus');
  const [configName, setConfigName] = useState('');

  const [testResult, setTestResult] = useState<ConnectivityResult | null>(null);
  const [testing, setTesting] = useState(false);
  const [toast, setToast] = useState('');

  const [deleteConfirm, setDeleteConfirm] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [cc, sc, ui] = await Promise.all([
        invoke<CurrentConfig>('get_current_config'),
        invoke<SavedConfig[]>('get_saved_configs'),
        invoke<{ url: string; model: string }>('get_upstream_info'),
      ]);
      setCurrentConfig(cc);
      setSavedConfigs(sc);
      setUpstreamUrl(ui.url);
      setUpstreamModel(ui.model);
    } catch {}
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  const applyPreset = (presetName: string) => {
    const p = PRESETS.find(x => x.name === presetName);
    if (p) {
      setPreset(presetName);
      setBaseUrl(p.url);
      setModel(p.model);
      setTestResult(null);
    }
  };

  const doTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const res = await invoke<ConnectivityResult>('test_connectivity', { baseUrl, apiKey });
      setTestResult(res);
    } catch {
      setTestResult({ success: false, models: [], error_message: String('Connection error'), latency_ms: 0 });
    }
    setTesting(false);
  };

  const doSave = async () => {
    if (!configName.trim()) return;
    const name = configName.trim();
    await invoke('save_config', { name, model, provider: preset, baseUrl, apiKey });
    setToast(t(lang, 'toast_saved'));
    setConfigName('');
    refresh();
    setTimeout(() => setToast(''), 2000);
  };

  const doApply = async (name: string) => {
    await invoke('apply_config', { name });
    setToast(t(lang, 'toast_applied'));
    refresh();
    setTimeout(() => setToast(''), 2000);
  };

  const doDelete = async (name: string) => {
    await invoke('delete_config', { name });
    setToast(t(lang, 'toast_deleted'));
    setDeleteConfirm(null);
    refresh();
    setTimeout(() => setToast(''), 2000);
  };

  return (
    <div className="h-full overflow-auto p-4 space-y-4 relative">
      {/* Quick Config */}
      <div className="bg-bg-card border border-border rounded-lg p-4">
        <h2 className="text-xs font-semibold text-text-1 mb-3 flex items-center gap-2">
          <Server className="w-3.5 h-3.5 text-accent" />
          {t(lang, 'config_preset')}
        </h2>

        {/* Presets */}
        <div className="flex flex-wrap gap-1.5 mb-3">
          {PRESETS.map(p => (
            <button
              key={p.name}
              onClick={() => applyPreset(p.name)}
              className={cn(
                "px-2.5 py-1 rounded text-[11px] font-medium transition-all border",
                preset === p.name
                  ? "bg-accent/10 border-accent text-accent"
                  : "bg-bg-elev border-border text-text-2 hover:text-text-1 hover:border-text-3"
              )}
            >
              {p.display}
            </button>
          ))}
        </div>

        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className="text-[10px] text-text-3 uppercase mb-1 block font-semibold">{t(lang, 'config_base_url')}</label>
            <input value={baseUrl} onChange={e => setBaseUrl(e.target.value)}
              className="w-full px-2.5 py-1.5 bg-bg-input border border-border rounded text-xs font-mono text-text-1 focus:outline-none focus:border-accent" />
          </div>
          <div>
            <label className="text-[10px] text-text-3 uppercase mb-1 block font-semibold">{t(lang, 'config_api_key')}</label>
            <input value={apiKey} onChange={e => setApiKey(e.target.value)} type="password"
              className="w-full px-2.5 py-1.5 bg-bg-input border border-border rounded text-xs font-mono text-text-1 focus:outline-none focus:border-accent" />
          </div>
          <div>
            <label className="text-[10px] text-text-3 uppercase mb-1 block font-semibold">{t(lang, 'config_model')}</label>
            <input value={model} onChange={e => setModel(e.target.value)}
              className="w-full px-2.5 py-1.5 bg-bg-input border border-border rounded text-xs font-mono text-text-1 focus:outline-none focus:border-accent" />
          </div>
          <div>
            <label className="text-[10px] text-text-3 uppercase mb-1 block font-semibold">{t(lang, 'config_name')}</label>
            <input value={configName} onChange={e => setConfigName(e.target.value)}
              placeholder="my-config"
              className="w-full px-2.5 py-1.5 bg-bg-input border border-border rounded text-xs font-mono text-text-1 placeholder:text-text-3 focus:outline-none focus:border-accent" />
          </div>
        </div>

        <div className="flex items-center gap-2 mt-3">
          <button onClick={doTest} disabled={testing}
            className="px-3 py-1.5 rounded text-[11px] font-medium bg-bg-elev border border-border text-text-2 hover:text-text-1 hover:bg-bg-input transition-all inline-flex items-center gap-1.5 disabled:opacity-50">
            <Play className="w-3 h-3" />
            {testing ? t(lang, 'btn_testing') : t(lang, 'btn_test')}
          </button>
          <button onClick={doSave}
            className="px-3 py-1.5 rounded text-[11px] font-medium bg-accent/10 border border-accent text-accent hover:bg-accent hover:text-white transition-all inline-flex items-center gap-1.5">
            <Save className="w-3 h-3" />
            {t(lang, 'btn_save')}
          </button>
        </div>

        {/* Test Result */}
        {testResult && (
          <div className={cn("mt-3 p-3 rounded-lg border text-xs", testResult.success ? "bg-green-bg border-green/30" : "bg-red-bg border-red/30")}>
            <div className="flex items-center gap-1.5 mb-1">
              {testResult.success ? <Check className="w-3.5 h-3.5 text-green" /> : <X className="w-3.5 h-3.5 text-red" />}
              <span className={cn("font-semibold", testResult.success ? "text-green" : "text-red")}>
                {testResult.success ? t(lang, 'test_success') : t(lang, 'test_failed')}
              </span>
              <span className="text-text-3">· {testResult.latency_ms}ms</span>
            </div>
            {testResult.error_message && <div className="text-red text-[11px]">{testResult.error_message}</div>}
            {testResult.models.length > 0 && (
              <div className="mt-1">
                <span className="text-text-3 text-[10px]">{t(lang, 'test_models')}:</span>
                <div className="flex flex-wrap gap-1 mt-1">
                  {testResult.models.map(m => <span key={m} className="px-1.5 py-0.5 bg-bg-elev rounded text-[10px] font-mono text-text-2">{m}</span>)}
                </div>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Proxy Status */}
      <div className="bg-bg-card border border-border rounded-lg p-4">
        <h2 className="text-xs font-semibold text-text-1 mb-3 flex items-center gap-2">
          <Globe className="w-3.5 h-3.5 text-blue" />
          {t(lang, 'config_status')}
        </h2>
        {currentConfig && (
          <div className="space-y-1.5 text-xs">
            <div className="flex gap-2">
              <span className="text-text-3 w-24">{t(lang, 'config_model')}:</span>
              <span className="text-text-1 font-semibold font-mono">{currentConfig.model}</span>
            </div>
            <div className="flex gap-2">
              <span className="text-text-3 w-24">{t(lang, 'config_preset')}:</span>
              <span className="text-text-1 font-mono">{currentConfig.provider}</span>
            </div>
            <div className="flex gap-2">
              <span className="text-text-3 w-24">{t(lang, 'config_base_url')}:</span>
              <span className="text-text-1 font-mono">http://127.0.0.1:8000/v1 <span className="text-text-3">({t(lang, 'config_via')})</span></span>
            </div>
            <div className="border-t border-border pt-2 mt-2">
              <div className="flex items-center gap-1.5 text-text-3 text-[10px] mb-1 font-semibold">
                <ArrowRight className="w-3 h-3 text-accent" /> {t(lang, 'upstream_title')}
              </div>
              <div className="flex gap-2">
                <span className="text-text-3 w-24">{t(lang, 'upstream_url')}:</span>
                <span className="text-text-1 font-mono text-[11px]">{upstreamUrl}</span>
              </div>
              <div className="flex gap-2">
                <span className="text-text-3 w-24">{t(lang, 'upstream_model')}:</span>
                <span className="text-accent font-semibold font-mono">{upstreamModel}</span>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Saved Configs */}
      <div className="bg-bg-card border border-border rounded-lg p-4">
        <h2 className="text-xs font-semibold text-text-1 mb-3 flex items-center gap-2">
          <Key className="w-3.5 h-3.5 text-yellow" />
          {t(lang, 'config_saved')} ({savedConfigs.length})
        </h2>
        {savedConfigs.length === 0 ? (
          <div className="text-xs text-text-3 py-6 text-center">{t(lang, 'history_empty')}</div>
        ) : (
          <div className="space-y-2">
            {savedConfigs.map(cfg => (
              <div key={cfg.id} className="flex items-center justify-between bg-bg-elev border border-border rounded-lg px-3 py-2.5">
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <Cpu className="w-3 h-3 text-text-3" />
                    <span className="text-xs font-semibold text-text-1">{cfg.name}</span>
                    {cfg.model === currentConfig?.model && <span className="px-1 py-0.5 bg-accent/10 text-accent text-[9px] rounded font-semibold">ACTIVE</span>}
                  </div>
                  <div className="text-[10px] text-text-3 font-mono mt-1 truncate">
                    {cfg.base_url} · {cfg.model} · {cfg.api_key_masked}
                  </div>
                </div>
                <div className="flex items-center gap-1.5 flex-shrink-0 ml-3">
                  <button onClick={() => doApply(cfg.name)}
                    className="px-2 py-1 text-[10px] rounded bg-accent/10 text-accent hover:bg-accent hover:text-white transition-all font-medium">
                    {t(lang, 'btn_apply')}
                  </button>
                  <button onClick={() => setDeleteConfirm(cfg.name)}
                    className="p-1 rounded text-text-3 hover:text-red hover:bg-red-bg transition-colors">
                    <Trash2 className="w-3 h-3" />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Delete confirm */}
      {deleteConfirm && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40" onClick={() => setDeleteConfirm(null)}>
          <div className="bg-bg-card border border-border rounded-lg p-5 w-80 shadow-2xl" onClick={e => e.stopPropagation()}>
            <h3 className="text-sm font-semibold mb-2">{t(lang, 'delete_confirm_title')}</h3>
            <p className="text-xs text-text-2 mb-4">{t(lang, 'delete_confirm_msg')} <strong>{deleteConfirm}</strong> ?</p>
            <div className="flex justify-end gap-2">
              <button onClick={() => setDeleteConfirm(null)}
                className="px-3 py-1.5 text-[11px] rounded bg-bg-elev text-text-2 hover:text-text-1">{t(lang, 'btn_cancel')}</button>
              <button onClick={() => doDelete(deleteConfirm)}
                className="px-3 py-1.5 text-[11px] rounded bg-red/10 text-red hover:bg-red hover:text-white font-medium">{t(lang, 'btn_delete')}</button>
            </div>
          </div>
        </div>
      )}

      {/* Toast */}
      {toast && (
        <div className="fixed bottom-4 left-1/2 -translate-x-1/2 bg-green-bg border border-green text-green px-4 py-2 rounded-lg text-xs font-semibold shadow-lg z-50">
          {toast}
        </div>
      )}
    </div>
  );
}
