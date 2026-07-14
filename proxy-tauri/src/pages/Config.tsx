import { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Check, X, Server, ArrowRight, Globe, Key, Cpu, RefreshCw, Zap, Plus, ChevronLeft, LogOut, Search, Wrench, Brain, Paperclip, DollarSign, Calendar, Box, Play, Trash2, Power, Square, Eye, EyeOff } from 'lucide-react';
import { useApp } from '@/contexts/AppContext';
import { t } from '@/lib/i18n';
import { cn } from '@/lib/utils';
import type { SavedConfig, CurrentConfig, ConnectivityResult, ModelCatalog, CatalogProvider, CatalogModel } from '@/lib/types';

interface FullConfig {
  model: string;
  provider: string;
  base_url: string;
  api_key: string;
}

const LOCAL_PROVIDERS = [
  { id: 'ollama', name: 'Ollama (Local)', url: 'http://localhost:11434/v1', model: 'llama3' },
  { id: 'lmstudio', name: 'LM Studio (Local)', url: 'http://localhost:1234/v1', model: 'local-model' },
];

const formatContext = (n: number | null): string => {
  if (!n) return '';
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(n % 1_000_000 === 0 ? 0 : 1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(0)}K`;
  return `${n}`;
};

const formatCost = (n: number | null): string => {
  if (n === null || n === undefined) return '';
  if (n === 0) return 'Free';
  if (n < 0.01) return `$${n.toFixed(3)}`;
  return `$${n.toFixed(2)}`;
};

export default function Config() {
  const { lang, configVersion, proxyRunning, setProxyRunning, widgetVisible, setWidgetVisible } = useApp();

  // Data
  const [currentConfig, setCurrentConfig] = useState<CurrentConfig | null>(null);
  const [savedConfigs, setSavedConfigs] = useState<SavedConfig[]>([]);
  const [upstreamUrl, setUpstreamUrl] = useState('');
  const [upstreamModel, setUpstreamModel] = useState('');

  // Catalog
  const [catalog, setCatalog] = useState<ModelCatalog | null>(null);
  const [catalogLoading, setCatalogLoading] = useState(false);

  // Modal
  const [showModal, setShowModal] = useState(false);
  const [modalView, setModalView] = useState<'main' | 'add'>('main');

  // Add form
  const [addProvider, setAddProvider] = useState<CatalogProvider | null>(null);
  const [addPresetId, setAddPresetId] = useState('');
  const [addBaseUrl, setAddBaseUrl] = useState('');
  const [addApiKey, setAddApiKey] = useState('');
  const [addModels, setAddModels] = useState<string[]>([]);

  // Search
  const [searchQuery, setSearchQuery] = useState('');

  // Test (for local providers)
  const [testResult, setTestResult] = useState<ConnectivityResult | null>(null);
  const [testing, setTesting] = useState(false);
  const testTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Disconnect confirm
  const [disconnecting, setDisconnecting] = useState<string | null>(null);

  // Model list pagination
  const [modelPage, setModelPage] = useState(0);
  const MODELS_PER_PAGE = 20;

  // Toast
  const [toast, setToast] = useState('');
  const showToast = useCallback((msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(''), 2000);
  }, []);

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

  const loadCatalog = useCallback(async () => {
    setCatalogLoading(true);
    try {
      let cat = await invoke<ModelCatalog>('get_model_catalog');
      if (!cat?.providers || cat.providers.length === 0) {
        cat = await invoke<ModelCatalog>('refresh_model_catalog');
      }
      if (cat?.providers?.length > 0) {
        setCatalog(cat);
      }
    } catch {
      try {
        const cat = await invoke<ModelCatalog>('refresh_model_catalog');
        if (cat?.providers?.length > 0) setCatalog(cat);
      } catch {}
    }
    setCatalogLoading(false);
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  // Refresh when config changes via header switcher
  useEffect(() => { if (configVersion > 0) refresh(); }, [configVersion]);



  const openModal = async (view: 'main' | 'add' = 'main') => {
    refresh();
    setModalView(view);
    setAddProvider(null);
    setAddPresetId('');
    setAddBaseUrl('');
    setAddApiKey('');
    setAddModels([]);
    setTestResult(null);
    setTesting(false);
    setDisconnecting(null);
    setSearchQuery('');
    setShowModal(true);
    // Load catalog when opening modal
    await loadCatalog();
  };

  const selectAddProvider = (provider: CatalogProvider) => {
    setAddProvider(provider);
    setAddPresetId(provider.id);
    setAddBaseUrl(provider.api ?? '');
    setAddModels([]);
    setTestResult(null);
  };

  const selectLocalProvider = (id: string) => {
    const lp = LOCAL_PROVIDERS.find(x => x.id === id);
    if (!lp) return;
    setAddProvider(null);
    setAddPresetId(id);
    setAddBaseUrl(lp.url);
    setAddModels([]);
    setTestResult(null);
  };

  const selectCustom = () => {
    setAddProvider(null);
    setAddPresetId('custom');
    setAddBaseUrl('');
    setAddModels([]);
    setTestResult(null);
  };

  const runTest = useCallback(async (url: string, key: string) => {
    if (!url || !key) return;
    setTesting(true);
    setTestResult(null);
    try {
      const res = await invoke<ConnectivityResult>('test_connectivity', { baseUrl: url, apiKey: key });
      setTestResult(res);
    } catch {
      setTestResult({ success: false, models: [], error_message: String('Connection error'), latency_ms: 0 });
    }
    setTesting(false);
  }, []);

  const queueLocalTest = (url: string, key: string) => {
    if (testTimerRef.current) clearTimeout(testTimerRef.current);
    if (!url || !key) return;
    testTimerRef.current = setTimeout(() => runTest(url, key), 600);
  };

  const doConnectSave = async () => {
    if (!addPresetId || !addApiKey || addModels.length === 0) return;
    const baseName = addPresetId === 'custom' ? 'custom-config' : addPresetId;
    // Save each selected model as a separate config
    for (let i = 0; i < addModels.length; i++) {
      const modelId = addModels[i];
      const name = addModels.length === 1 ? baseName : `${baseName}-${modelId.split('/').pop()}`;
      await invoke('save_config', { name, model: modelId, provider: addPresetId, baseUrl: addBaseUrl, apiKey: addApiKey });
    }
    // Apply the first model
    const firstName = addModels.length === 1 ? baseName : `${baseName}-${addModels[0].split('/').pop()}`;
    await invoke('apply_config', { name: firstName });
    showToast(t(lang, 'toast_saved'));
    setAddProvider(null);
    setAddPresetId('');
    setAddBaseUrl('');
    setAddApiKey('');
    setAddModels([]);
    setTestResult(null);
    await refresh();
    setShowModal(false);
  };

  const doSwitchModel = async (cfgName: string, model: string) => {
    try {
      const full = await invoke<FullConfig>('get_config_full', { name: cfgName });
      await invoke('save_config', { name: cfgName, model, provider: full.provider, baseUrl: full.base_url, apiKey: full.api_key });
      await invoke('apply_config', { name: cfgName });
      showToast(t(lang, 'toast_applied'));
      await refresh();
    } catch {}
  };

  const doDisconnect = async (name: string) => {
    await invoke('delete_config', { name });
    showToast(t(lang, 'toast_deleted'));
    setDisconnecting(null);
    await refresh();
  };

  const doBypassProxy = async () => {
    try {
      await invoke('bypass_proxy');
      setProxyRunning(false);
      showToast(t(lang, 'toast_bypassed'));
      await refresh();
    } catch (e: any) {
      showToast(String(e));
    }
  };

  const doStartProxy = async () => {
    try {
      await invoke('start_proxy');
      setProxyRunning(true);
      await refresh();
      showToast(t(lang, 'toast_applied'));
    } catch (e: any) {
      showToast(String(e));
    }
  };

  const toggleAddModel = (modelId: string) => {
    setAddModels(prev => 
      prev.includes(modelId) 
        ? prev.filter(m => m !== modelId)
        : [...prev, modelId]
    );
  };

  // Derived
  const isLocalProvider = addPresetId === 'ollama' || addPresetId === 'lmstudio';
  const addModelsList = addProvider?.models ?? [];
  const hasCatalogModels = addModelsList.length > 0;

  // Reset pagination when provider changes
  useEffect(() => { setModelPage(0); }, [addProvider]);

  // Paginated models
  const paginatedModels = addModelsList.slice(
    modelPage * MODELS_PER_PAGE,
    (modelPage + 1) * MODELS_PER_PAGE
  );
  const totalPages = Math.ceil(addModelsList.length / MODELS_PER_PAGE);

  // Filter catalog providers (memoized for responsiveness)
  const filteredCatalog = useMemo(() => {
    const q = searchQuery.toLowerCase().trim();
    if (!q) return catalog?.providers ?? [];
    // Score: name/id match = 2, model match = 1, then sort by score desc
    return (catalog?.providers ?? [])
      .map(p => {
        let score = 0;
        if (p.name.toLowerCase().includes(q)) score = Math.max(score, 2);
        if (p.id.toLowerCase().includes(q)) score = Math.max(score, 2);
        if (p.models.some(m => m.name.toLowerCase().includes(q) || m.id.toLowerCase().includes(q) || (m.description?.toLowerCase().includes(q) ?? false))) {
          score = Math.max(score, 1);
        }
        return { provider: p, score };
      })
      .filter(({ score }) => score > 0)
      .sort((a, b) => b.score - a.score)
      .map(({ provider }) => provider);
  }, [catalog, searchQuery]);

  // Saved config provider IDs for highlighting
  const savedProviderIds = new Set(savedConfigs.map(c => c.provider));

  return (
    <div className="h-full overflow-auto p-4 space-y-4 relative">
      {/* ===== 代理状态 ===== */}
      <div className="bg-bg-card border border-border rounded-lg p-4">
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-xs font-semibold text-text-1 flex items-center gap-2">
            <Globe className="w-3.5 h-3.5 text-blue" />
            {t(lang, 'config_status')}
          </h2>
          {currentConfig?.model && (proxyRunning ? (
            <button
              onClick={doBypassProxy}
              className="px-2.5 py-1 rounded-lg border text-xs font-semibold inline-flex items-center gap-1.5 transition-all bg-bg-elev border-border hover:border-text-3 text-text-2 hover:text-text-1"
              title={t(lang, 'config_bypass_desc')}
            >
              <LogOut className="w-3 h-3" />
              {t(lang, 'config_bypass')}
            </button>
          ) : (
            <button
              onClick={doStartProxy}
              className="px-2.5 py-1 rounded-lg text-xs font-semibold inline-flex items-center gap-1.5 transition-all bg-accent/10 text-accent hover:bg-accent hover:text-white"
              title={t(lang, 'config_start_proxy')}
            >
              <Play className="w-3 h-3" />
              {t(lang, 'config_start_proxy')}
            </button>
          ))}
        </div>
        {currentConfig?.model && (
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
              <span className="text-text-1 font-mono flex items-center gap-1">
                {currentConfig.base_url}
                {currentConfig.base_url.includes('127.0.0.1') ? (
                  <span className="text-text-3">({t(lang, 'config_via')})</span>
                ) : (
                  <span className="text-green">({t(lang, 'config_bypassed')})</span>
                )}
              </span>
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

      {/* ===== 悬浮窗设置 ===== */}
      <div className="bg-bg-card border border-border rounded-lg p-4">
        <div className="flex items-center justify-between">
          <div className="flex flex-col">
            <h2 className="text-xs font-semibold text-text-1 flex items-center gap-2 mb-0.5">
              <Eye className="w-3.5 h-3.5 text-purple" />
              {t(lang, 'widget_toggle_label')}
            </h2>
            <span className="text-[10px] text-text-3">{t(lang, 'widget_toggle_desc')}</span>
          </div>
          <button
            onClick={() => setWidgetVisible(!widgetVisible)}
            className={cn(
              'relative w-9 h-5 rounded-full transition-colors duration-200 flex-shrink-0',
              widgetVisible ? 'bg-accent' : 'bg-bg-elev border border-border'
            )}
          >
            <span
              className={cn(
                'absolute top-0.5 w-4 h-4 rounded-full bg-white shadow transition-transform duration-200',
                widgetVisible ? 'left-[calc(100%-1.125rem)]' : 'left-0.5'
              )}
            />
          </button>
        </div>
      </div>

      {/* ===== 已保存配置（表格） ===== */}
      <div className="bg-bg-card border border-border rounded-lg p-4">
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-xs font-semibold text-text-1 flex items-center gap-2">
            <Key className="w-3.5 h-3.5 text-yellow" />
            {t(lang, 'config_saved')} ({savedConfigs.length})
          </h2>
          <div className="flex items-center gap-2">
            <button
              onClick={() => openModal('main')}
              className="px-3 py-1.5 rounded-lg bg-accent/10 text-accent hover:bg-accent hover:text-white transition-all text-xs font-semibold inline-flex items-center gap-1.5"
            >
              <Server className="w-3.5 h-3.5" />
              {t(lang, 'config_manage_models')}
            </button>
          </div>
        </div>
        {savedConfigs.length === 0 ? (
          <div className="text-xs text-text-3 py-6 text-center">{t(lang, 'history_empty')}</div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-xs">
              <thead>
                <tr className="text-text-3 border-b border-border">
                  <th className="text-left font-medium py-1.5 pr-3">{t(lang, 'config_name')}</th>
                  <th className="text-left font-medium py-1.5 pr-3">{t(lang, 'config_provider')}</th>
                  <th className="text-left font-medium py-1.5 pr-3">{t(lang, 'config_model')}</th>
                  <th className="text-left font-medium py-1.5 pr-3">{t(lang, 'config_base_url')}</th>
                  <th className="text-left font-medium py-1.5 pr-3">{t(lang, 'config_api_key')}</th>
                  <th className="text-left font-medium py-1.5 w-16">{t(lang, 'config_actions')}</th>
                </tr>
              </thead>
              <tbody>
                {savedConfigs.map(cfg => (
                  <tr key={cfg.id} className="hover:bg-bg-elev/50 transition-colors">
                    <td className="py-1.5 pr-3">
                      <div className="flex items-center gap-1.5">
                        <span className="font-semibold text-text-1 truncate max-w-[100px]">{cfg.name}</span>
                        {cfg.model === currentConfig?.model && (
                          <span className="px-1 py-0.5 bg-accent/10 text-accent text-[9px] rounded font-semibold flex-shrink-0">ACTIVE</span>
                        )}
                      </div>
                    </td>
                    <td className="py-1.5 pr-3 text-text-2 font-mono truncate max-w-[80px]">{cfg.provider}</td>
                    <td className="py-1.5 pr-3 text-text-2 font-mono truncate max-w-[140px]">{cfg.model.includes('/') ? cfg.model.split('/').slice(1).join('/') : cfg.model}</td>
                    <td className="py-1.5 pr-3 text-text-3 font-mono truncate max-w-[180px]">{cfg.base_url}</td>
                    <td className="py-1.5 pr-3 text-text-3 font-mono">{cfg.api_key_masked}</td>
                    <td className="py-1.5">
                      <div className="flex items-center gap-1">
                        <button
                          onClick={async () => {
                            if (cfg.model === currentConfig?.model && proxyRunning) {
                              // Current model is active, stop proxy
                              await invoke('bypass_proxy');
                              setProxyRunning(false);
                              showToast(t(lang, 'toast_bypassed'));
                            } else {
                              // Apply this config
                              await invoke('apply_config', { name: cfg.name });
                              showToast(t(lang, 'toast_applied'));
                            }
                            await refresh();
                          }}
                          className={`p-1 rounded transition-colors ${
                            cfg.model === currentConfig?.model && proxyRunning
                              ? 'hover:bg-orange/10 text-orange hover:text-orange'
                              : 'hover:bg-green/10 text-text-3 hover:text-green'
                          }`}
                          title={cfg.model === currentConfig?.model && proxyRunning ? t(lang, 'btn_deactivate') : t(lang, 'btn_apply')}
                        >
                          {cfg.model === currentConfig?.model && proxyRunning ? (
                            <Square className="w-3.5 h-3.5" />
                          ) : (
                            <Play className="w-3.5 h-3.5" />
                          )}
                        </button>
                        <button
                          onClick={async () => {
                            try {
                              const res = await invoke<ConnectivityResult>('test_saved_config', { name: cfg.name });
                              showToast(res.success ? t(lang, 'test_success') : `${t(lang, 'test_failed')}: ${res.error_message || ''}`);
                            } catch {
                              showToast(t(lang, 'test_failed'));
                            }
                          }}
                          className="p-1 rounded hover:bg-green/10 text-text-3 hover:text-green transition-colors"
                          title={t(lang, 'btn_test')}
                        >
                          <Zap className="w-3.5 h-3.5" />
                        </button>
                        <button
                          onClick={() => doDisconnect(cfg.name)}
                          className="p-1 rounded hover:bg-red/10 text-text-3 hover:text-red transition-colors"
                          title={t(lang, 'btn_delete')}
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* ===== 模型管理弹窗 ===== */}
      {showModal && (
        <div className="fixed inset-0 z-50 flex items-start justify-center pt-[8vh] bg-black/40">
          <div className="bg-bg-card border border-border rounded-xl shadow-2xl w-[640px] max-h-[84vh] flex flex-col" onClick={e => e.stopPropagation()}>

            {/* Header */}
            <div className="flex items-center justify-between px-5 py-3 border-b border-border flex-shrink-0">
              {modalView === 'add' ? (
                <button
                  onClick={() => { setModalView('main'); setAddProvider(null); setAddPresetId(''); setAddBaseUrl(''); setAddApiKey(''); setAddModels([]); setTestResult(null); setSearchQuery(''); }}
                  className="text-text-2 hover:text-text-1 transition-colors inline-flex items-center gap-1 text-xs font-medium"
                >
                  <ChevronLeft className="w-3.5 h-3.5" />
                  {t(lang, 'btn_cancel')}
                </button>
              ) : (
                <h3 className="text-sm font-semibold text-text-1">{t(lang, 'config_manage_models')}</h3>
              )}
              <button onClick={() => setShowModal(false)} className="p-1 rounded text-text-3 hover:text-text-1 hover:bg-bg-elev transition-colors">
                <X className="w-4 h-4" />
              </button>
            </div>

            {/* Body */}
            <div className="flex-1 overflow-auto p-5">
              {catalogLoading ? (
                /* Loading state */
                <div className="flex flex-col items-center justify-center py-12">
                  <RefreshCw className="w-6 h-6 text-accent animate-spin mb-3" />
                  <p className="text-xs text-text-3">{t(lang, 'config_loading_catalog')}</p>
                </div>
              ) : modalView === 'main' ? (
                /* ===== 主视图：厂商列表 ===== */
                <div className="space-y-4">
                  {/* Search */}
                  <div className="relative">
                    <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-text-4" />
                    <input
                      value={searchQuery}
                      onChange={e => setSearchQuery(e.target.value)}
                      placeholder={t(lang, 'config_search_providers')}
                      className="w-full pl-7 pr-3 py-2 bg-bg-input border border-border rounded-lg text-xs text-text-1 focus:outline-none focus:border-accent placeholder:text-text-4"
                    />
                  </div>

                  {/* Catalog stats */}
                  {catalog && (
                    <p className="text-[10px] text-text-4 text-center">
                      {t(lang, 'config_catalog_info').replace('{n}', String(catalog.providers.length)).replace('{m}', String(catalog.providers.reduce((s, p) => s + p.models.length, 0)))}
                    </p>
                  )}

                  {/* Provider grid - rich cards */}
                  {searchQuery && filteredCatalog.length === 0 ? (
                    <p className="text-xs text-text-4 text-center py-4">{t(lang, 'history_no_results')}</p>
                  ) : (
                    filteredCatalog.map(p => (
                    <button
                      key={p.id}
                      onClick={() => { selectAddProvider(p); setModalView('add'); }}
                      className={cn(
                        "w-full px-3 py-2.5 rounded-lg border text-left transition-all group",
                        savedProviderIds.has(p.id)
                          ? "bg-accent/5 border-accent/30 hover:bg-accent/10"
                          : "bg-bg-elev border-border hover:border-text-3 hover:bg-bg-elev/80"
                      )}
                    >
                      <div className="flex items-start justify-between gap-2">
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2">
                            <span className="text-xs font-semibold text-text-1">{p.name}</span>
                            {savedProviderIds.has(p.id) && (
                              <span className="px-1 py-0.5 bg-accent/10 text-accent text-[8px] rounded font-semibold">CONNECTED</span>
                            )}
                          </div>
                          {/* Model preview - first 3 models */}
                          <div className="flex items-center gap-1 mt-1 flex-wrap">
                            {p.models.slice(0, 3).map(m => (
                              <span key={m.id} className="text-[9px] font-mono text-text-3 bg-bg-card px-1.5 py-0.5 rounded">
                                {m.name}
                              </span>
                            ))}
                            {p.model_count > 3 && (
                              <span className="text-[9px] text-text-4">+{p.model_count - 3} more</span>
                            )}
                          </div>
                        </div>
                        <div className="flex flex-col items-end gap-1 flex-shrink-0">
                          <span className="text-[10px] font-semibold text-accent bg-accent/10 px-1.5 py-0.5 rounded">
                            {p.model_count} models
                          </span>
                          {p.npm && (
                            <span className="text-[8px] text-text-4 font-mono">{p.npm.replace('@ai-sdk/', '')}</span>
                          )}
                        </div>
                      </div>
                    </button>
                  )))}

                  {/* Local & Custom providers */}
                  <div className="border-t border-border pt-3">
                    <h4 className="text-[10px] text-text-3 uppercase mb-2 font-semibold">
                      {t(lang, 'config_custom')}
                    </h4>
                    <div className="grid grid-cols-3 gap-2">
                      {LOCAL_PROVIDERS.map(lp => (
                        <button
                          key={lp.id}
                          onClick={() => { selectLocalProvider(lp.id); setModalView('add'); }}
                          className="px-2 py-2 rounded-lg border text-left transition-all bg-bg-elev border-border hover:border-text-3"
                        >
                          <div className="text-[11px] font-semibold text-text-1 truncate">{lp.name}</div>
                          <div className="text-[9px] text-text-3 mt-0.5">{t(lang, 'config_connection_testing')}</div>
                        </button>
                      ))}
                      <button
                        onClick={() => { selectCustom(); setModalView('add'); }}
                        className="px-2 py-2 rounded-lg border text-left transition-all bg-bg-elev border-border hover:border-text-3"
                      >
                        <div className="text-[11px] font-semibold text-text-1">{t(lang, 'config_custom')}</div>
                        <div className="text-[9px] text-text-3 mt-0.5">{t(lang, 'config_preset_custom')}</div>
                      </button>
                    </div>
                  </div>
                </div>
              ) : (
                /* ===== 添加视图 ===== */
                <div>
                  {!addPresetId ? (
                    /* Step 1: Select provider */
                    <>
                      <h4 className="text-xs font-semibold text-text-1 mb-3">{t(lang, 'config_preset')}</h4>

                      {/* Search */}
                      <div className="relative mb-3">
                        <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-text-4" />
                        <input
                          value={searchQuery}
                          onChange={e => setSearchQuery(e.target.value)}
                          placeholder={t(lang, 'config_search_providers')}
                          className="w-full pl-7 pr-3 py-1.5 bg-bg-input border border-border rounded text-xs text-text-1 focus:outline-none focus:border-accent placeholder:text-text-4"
                        />
                      </div>

                      {/* Catalog providers */}
                      {filteredCatalog.length > 0 && (
                        <div className="grid grid-cols-3 gap-2 mb-4">
                          {filteredCatalog.map(p => (
                            <button
                              key={p.id}
                              onClick={() => selectAddProvider(p)}
                              className={cn(
                                "px-2.5 py-2.5 rounded-lg border text-left transition-all",
                                savedProviderIds.has(p.id)
                                  ? "bg-accent/5 border-accent/30"
                                  : "bg-bg-elev border-border hover:border-text-3"
                              )}
                            >
                              <div className="text-xs font-semibold text-text-1">{p.name}</div>
                              <div className="text-[9px] text-text-3 mt-0.5">{p.model_count} models</div>
                              {p.api && (() => { try { return <div className="text-[8px] text-text-4 font-mono mt-1 truncate">{new URL(p.api!).hostname}</div>; } catch { return null; } })()}
                            </button>
                          ))}
                        </div>
                      )}

                      {/* Local + Custom */}
                      <div className="grid grid-cols-3 gap-2 mb-4">
                        {LOCAL_PROVIDERS.map(lp => (
                          <button
                            key={lp.id}
                            onClick={() => selectLocalProvider(lp.id)}
                            className="px-2.5 py-2.5 rounded-lg border text-left transition-all bg-bg-elev border-border hover:border-text-3"
                          >
                            <div className="text-xs font-semibold text-text-1">{lp.name}</div>
                            <div className="text-[9px] text-text-3 mt-0.5">{t(lang, 'config_connection_testing')}</div>
                          </button>
                        ))}
                        <button
                          onClick={selectCustom}
                          className="px-2.5 py-2.5 rounded-lg border text-left transition-all bg-bg-elev border-border hover:border-text-3"
                        >
                          <div className="text-xs font-semibold text-text-1">{t(lang, 'config_custom')}</div>
                          <div className="text-[9px] text-text-3 mt-0.5">{t(lang, 'config_preset_custom')}</div>
                        </button>
                      </div>
                    </>
                  ) : (
                    /* Step 2: Enter key + pick model */
                    <div className="flex flex-col h-full">
                      {/* Provider info */}
                      <div className="flex items-center gap-2 p-2.5 bg-bg-elev rounded-lg border border-border">
                        <Cpu className="w-4 h-4 text-accent" />
                        <div className="flex-1 min-w-0">
                          <div className="text-xs font-semibold text-text-1">
                            {addProvider?.name ?? LOCAL_PROVIDERS.find(x => x.id === addPresetId)?.name ?? t(lang, 'config_custom')}
                          </div>
                          {addBaseUrl && (
                            <div className="text-[9px] font-mono text-text-3 truncate">{addBaseUrl}</div>
                          )}
                        </div>
                        {addProvider && (
                          <span className="text-[10px] font-semibold text-accent bg-accent/10 px-1.5 py-0.5 rounded flex-shrink-0">
                            {addProvider.model_count} models
                          </span>
                        )}
                      </div>

                      {/* Base URL (custom only) */}
                      {addPresetId === 'custom' && (
                        <div>
                          <label className="text-[10px] text-text-3 uppercase mb-1 block font-semibold">
                            {t(lang, 'config_base_url')}
                          </label>
                          <input
                            value={addBaseUrl}
                            onChange={e => setAddBaseUrl(e.target.value)}
                            placeholder="https://api.example.com/v1"
                            className="w-full px-2.5 py-1.5 bg-bg-input border border-border rounded text-xs font-mono text-text-1 focus:outline-none focus:border-accent placeholder:text-text-4"
                          />
                        </div>
                      )}

                      {/* API Key */}
                      <div>
                        <label className="text-[10px] text-text-3 uppercase mb-1 block font-semibold">
                          {t(lang, 'config_api_key')}
                        </label>
                        <input
                          value={addApiKey}
                          onChange={e => {
                            setAddApiKey(e.target.value);
                            if (isLocalProvider) queueLocalTest(addBaseUrl, e.target.value);
                          }}
                          type="password"
                          placeholder="sk-..."
                          className="w-full px-2.5 py-1.5 bg-bg-input border border-border rounded text-xs font-mono text-text-1 focus:outline-none focus:border-accent placeholder:text-text-4"
                        />
                      </div>

                      {/* Model list from catalog */}
                      {hasCatalogModels && (
                        <div className="flex-1 flex flex-col min-h-0 mt-3">
                          <div className="flex items-center justify-between mb-1.5">
                            <label className="text-[10px] text-text-3 uppercase block font-semibold">
                              {t(lang, 'test_models')} ({addModelsList.length})
                            </label>
                            {totalPages > 1 && (
                              <div className="flex items-center gap-1 text-[10px] text-text-3">
                                <button
                                  onClick={() => setModelPage(p => Math.max(0, p - 1))}
                                  disabled={modelPage === 0}
                                  className="px-1.5 py-0.5 rounded bg-bg-elev hover:bg-bg-elev/80 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
                                >
                                  ‹
                                </button>
                                <span className="font-mono">{modelPage + 1}/{totalPages}</span>
                                <button
                                  onClick={() => setModelPage(p => Math.min(totalPages - 1, p + 1))}
                                  disabled={modelPage >= totalPages - 1}
                                  className="px-1.5 py-0.5 rounded bg-bg-elev hover:bg-bg-elev/80 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
                                >
                                  ›
                                </button>
                              </div>
                            )}
                          </div>
                          <div className="flex-1 overflow-auto space-y-1">
                            {paginatedModels.map(m => (
                              <ModelCard
                                key={m.id}
                                model={m}
                                selected={addModels.includes(m.id)}
                                onSelect={() => toggleAddModel(m.id)}
                                lang={lang}
                              />
                            ))}
                          </div>
                        </div>
                      )}

                      {/* Local: API discovery */}
                      {isLocalProvider && (
                        <div>
                          <label className="text-[10px] text-text-3 uppercase mb-1.5 block font-semibold">
                            {t(lang, 'test_models')}
                          </label>
                          {!addApiKey ? (
                            <p className="text-[10px] text-text-4 italic">{t(lang, 'config_connection_testing')}</p>
                          ) : testing ? (
                            <div className="flex items-center gap-2 text-[11px] text-text-3">
                              <RefreshCw className="w-3 h-3 animate-spin" />
                              {t(lang, 'config_connection_testing')}
                            </div>
                          ) : testResult?.success && testResult.models.length > 0 ? (
                            <div className="space-y-0.5 max-h-48 overflow-auto">
                              {testResult.models.map(m => (
                                <button
                                  key={m}
                                  onClick={() => toggleAddModel(m)}
                                  className={cn(
                                    "w-full px-2.5 py-1.5 rounded text-left text-[11px] font-mono transition-all flex items-center gap-2",
                                    addModels.includes(m)
                                      ? "bg-accent/15 text-accent font-semibold"
                                      : "text-text-2 hover:text-text-1 hover:bg-bg-elev"
                                  )}
                                >
                                  <span className={cn(
                                    "w-3.5 h-3.5 rounded border flex items-center justify-center flex-shrink-0",
                                    addModels.includes(m) ? "border-accent bg-accent" : "border-text-3"
                                  )}>
                                    {addModels.includes(m) && <Check className="w-2.5 h-2.5 text-white" />}
                                  </span>
                                  {m}
                                </button>
                              ))}
                            </div>
                          ) : testResult && !testResult.success ? (
                            <div className="flex items-center gap-2">
                              <p className="text-[10px] text-red">{testResult.error_message || t(lang, 'test_failed')}</p>
                              <button
                                onClick={() => runTest(addBaseUrl, addApiKey)}
                                className="text-[10px] text-accent hover:underline"
                              >
                                {t(lang, 'config_refresh_models')}
                              </button>
                            </div>
                          ) : (
                            <div className="flex items-center gap-2">
                              <p className="text-[10px] text-text-4">{t(lang, 'config_connection_testing')}</p>
                              <button
                                onClick={() => runTest(addBaseUrl, addApiKey)}
                                disabled={!addApiKey}
                                className="text-[10px] text-accent hover:underline disabled:opacity-50 inline-flex items-center gap-1"
                              >
                                <RefreshCw className="w-2.5 h-2.5" />
                                {t(lang, 'config_refresh_models')}
                              </button>
                            </div>
                          )}
                        </div>
                      )}

                      {/* Custom: manual model input */}
                      {addPresetId === 'custom' && (
                        <div>
                          <label className="text-[10px] text-text-3 uppercase mb-1 block font-semibold">
                            {t(lang, 'config_model')}
                          </label>
                          <input
                            value={addModels.join(', ')}
                            onChange={e => setAddModels(e.target.value.split(',').map(s => s.trim()).filter(Boolean))}
                            placeholder="gpt-4o-mini, claude-3-5-sonnet"
                            className="w-full px-2.5 py-1.5 bg-bg-input border border-border rounded text-xs font-mono text-text-1 focus:outline-none focus:border-accent placeholder:text-text-4"
                          />
                          <p className="text-[9px] text-text-4 mt-1">{t(lang, 'config_multi_model_hint')}</p>
                        </div>
                      )}

                      {/* Connect & Save */}
                      {addModels.length > 0 && (
                        <button
                          onClick={doConnectSave}
                          className="w-full py-2 rounded-lg text-xs font-semibold bg-accent text-white hover:bg-accent/90 transition-all inline-flex items-center justify-center gap-1.5 mt-3 flex-shrink-0"
                        >
                          <Zap className="w-3.5 h-3.5" />
                          {t(lang, 'config_connect_save')} ({addModels.length})
                        </button>
                      )}
                    </div>
                  )}
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {/* ===== Toast ===== */}
      {toast && (
        <div className="fixed bottom-4 left-1/2 -translate-x-1/2 bg-green-bg border border-green text-green px-4 py-2 rounded-lg text-xs font-semibold shadow-lg z-[60]">
          {toast}
        </div>
      )}
    </div>
  );
}

/* ===== Model Card Component ===== */
function ModelCard({ model, selected, onSelect, lang }: { model: CatalogModel; selected: boolean; onSelect: () => void; lang: string }) {
  return (
    <button
      onClick={onSelect}
      className={cn(
        "w-full px-3 py-2 rounded-lg text-left transition-all border",
        selected
          ? "bg-accent/10 border-accent/40 text-accent"
          : "bg-bg-elev/50 border-transparent hover:border-border hover:bg-bg-elev"
      )}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0 flex-1">
          {/* Name + badges */}
          <div className="flex items-center gap-1.5 flex-wrap">
            <span className={cn("text-[11px] font-semibold font-mono truncate", selected ? "text-accent" : "text-text-1")}>
              {model.name}
            </span>
            {model.tool_call && (
              <span className="inline-flex items-center gap-0.5 px-1 py-0.5 bg-blue/10 text-blue text-[8px] rounded" title="Tool Call">
                <Wrench className="w-2 h-2" />
              </span>
            )}
            {model.reasoning && (
              <span className="inline-flex items-center gap-0.5 px-1 py-0.5 bg-purple/10 text-purple text-[8px] rounded" title="Reasoning">
                <Brain className="w-2 h-2" />
              </span>
            )}
            {model.attachment && (
              <span className="inline-flex items-center gap-0.5 px-1 py-0.5 bg-green/10 text-green text-[8px] rounded" title="Multimodal">
                <Paperclip className="w-2 h-2" />
              </span>
            )}
          </div>
          {/* Description */}
          {model.description && (
            <p className="text-[9px] text-text-3 mt-0.5 line-clamp-1">{model.description}</p>
          )}
          {/* Meta row */}
          <div className="flex items-center gap-2 mt-1 flex-wrap">
            {model.context && (
              <span className="inline-flex items-center gap-0.5 text-[8px] text-text-4">
                <Box className="w-2 h-2" />
                {formatContext(model.context)}
              </span>
            )}
            {model.cost_input !== null && model.cost_input !== undefined && (
              <span className="inline-flex items-center gap-0.5 text-[8px] text-text-4">
                <DollarSign className="w-2 h-2" />
                {formatCost(model.cost_input)}/{formatCost(model.cost_output)}
              </span>
            )}
            {model.release_date && (
              <span className="inline-flex items-center gap-0.5 text-[8px] text-text-4">
                <Calendar className="w-2 h-2" />
                {model.release_date}
              </span>
            )}
          </div>
        </div>
        {/* Checkbox indicator */}
        <span className={cn(
          "w-3.5 h-3.5 rounded border-2 flex items-center justify-center flex-shrink-0 mt-0.5",
          selected ? "border-accent bg-accent" : "border-text-3"
        )}>
          {selected && <Check className="w-2.5 h-2.5 text-white" />}
        </span>
      </div>
    </button>
  );
}
