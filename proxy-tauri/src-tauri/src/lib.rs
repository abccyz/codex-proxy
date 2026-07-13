mod config;
mod connectivity;
mod convert;
mod error;
mod history_store;
mod metrics;
mod model;
mod model_catalog;
mod proxy;
mod sse;

use config::{ConfigManager, SecureConfigStore, SavedConfig};
use connectivity::ConnectivityResult;
use history_store::HistoryStore;
use metrics::{Metrics, SharedMetrics};
use model_catalog::ModelCatalog;
use proxy::AppState;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};
use std::sync::{Arc, RwLock};
use std::path::PathBuf;

static PROXY_PORT: u16 = 8000;

#[derive(Serialize, Deserialize, Default)]
struct PersistedState {
    proxy_running: bool,
}

fn load_persisted_state(path: &PathBuf) -> PersistedState {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_persisted_state(path: &PathBuf, state: &PersistedState) {
    if let Ok(json) = serde_json::to_string(state) {
        let _ = std::fs::write(path, json);
    }
}

/// Strip provider prefix from model name: "deepseek/deepseek-v4-flash" -> "deepseek-v4-flash"
fn strip_model_prefix(model: &str) -> String {
    model.split('/').last().unwrap_or(model).to_string()
}

// ── State wrapper for Tauri managed state ──
pub struct TauriAppState {
    pub proxy_state: Arc<AppState>,
    pub metrics: SharedMetrics,
    pub config_manager: Arc<ConfigManager>,
    pub secure_store: Arc<SecureConfigStore>,
    pub proxy_running: Arc<AtomicBool>,
    pub proxy_handle: RwLock<Option<tokio::task::JoinHandle<()>>>,
    pub metrics_handle: RwLock<Option<tokio::task::JoinHandle<()>>>,
    pub state_file_path: PathBuf,
    pub history_store: Arc<HistoryStore>,
}

// ── Tauri Commands ──

#[tauri::command]
fn get_metrics(state: tauri::State<TauriAppState>) -> Result<metrics::Snapshot, String> {
    Ok(state.metrics.snapshot())
}

#[tauri::command]
fn get_history_detail(state: tauri::State<TauriAppState>, idx: usize) -> Result<metrics::InputDetail, String> {
    state.metrics.get_input_detail(idx).ok_or_else(|| "Record not found".to_string())
}

#[tauri::command]
fn get_current_config(state: tauri::State<TauriAppState>) -> config::CurrentConfig {
    state.config_manager.get_current_model()
}

#[tauri::command]
fn get_saved_configs(state: tauri::State<TauriAppState>) -> Vec<SavedConfig> {
    state.secure_store.list_configs()
}

#[tauri::command]
fn save_config(state: tauri::State<TauriAppState>, name: String, model: String, provider: String, base_url: String, api_key: String) -> bool {
    state.secure_store.save_config(&name, &model, &provider, &base_url, &api_key)
}

#[tauri::command]
fn get_config_full(state: tauri::State<TauriAppState>, name: String) -> Result<config::SavedConfigFull, String> {
    state.secure_store.get_config_full(&name)
        .ok_or_else(|| "Config not found".to_string())
}

#[tauri::command]
fn delete_config(state: tauri::State<TauriAppState>, name: String) -> bool {
    let ok = state.secure_store.delete_config(&name);
    // If no configs remain, clear proxy state
    if ok && state.secure_store.list_configs().is_empty() {
        state.proxy_state.set_upstream(String::new(), String::new());
        state.proxy_state.set_upstream_model(String::new());
        // Also clear the config manager's current model
        state.config_manager.apply_model("", "", "", "");
        tracing::info!("All configs deleted, proxy state cleared");
    }
    ok
}

#[tauri::command]
fn apply_config(state: tauri::State<TauriAppState>, name: String) -> Result<(), String> {
    let cfg = state.secure_store.get_config_full(&name)
        .ok_or_else(|| "Config not found".to_string())?;
    let upstream = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let proxy_base_url = format!("http://127.0.0.1:{}/v1", PROXY_PORT);
    // Backup original config before proxy modifies it
    state.config_manager.backup_config();
    state.config_manager.apply_model(&cfg.model, &cfg.provider, &proxy_base_url, &cfg.api_key);
    state.proxy_state.set_upstream(upstream, cfg.api_key);
    let model_clone = strip_model_prefix(&cfg.model);
    state.proxy_state.set_upstream_model(model_clone);
    tracing::info!("Applied config: {} -> {} (model: {})", name, cfg.base_url, cfg.model);
    Ok(())
}

#[tauri::command]
async fn test_saved_config(
    state: tauri::State<'_, TauriAppState>,
    name: String,
) -> Result<ConnectivityResult, String> {
    let cfg = state.secure_store.get_config_full(&name)
        .ok_or_else(|| "Config not found".to_string())?;
    let client = &state.proxy_state.http_client;
    let result = connectivity::test_connectivity(client, &cfg.base_url, &cfg.api_key).await;
    Ok(result)
}

#[tauri::command]
async fn test_connectivity(
    state: tauri::State<'_, TauriAppState>,
    base_url: String,
    api_key: String,
) -> Result<ConnectivityResult, String> {
    let client = &state.proxy_state.http_client;
    let result = connectivity::test_connectivity(client, &base_url, &api_key).await;
    Ok(result)
}

#[tauri::command]
fn get_proxy_status(state: tauri::State<TauriAppState>) -> bool {
    state.proxy_running.load(Ordering::SeqCst)
}

#[tauri::command]
fn bypass_proxy(state: tauri::State<TauriAppState>) -> Result<(), String> {
    // Stop the proxy server
    if let Some(handle) = state.proxy_handle.write().unwrap().take() {
        handle.abort();
    }
    // Stop the metrics emission loop
    if let Some(handle) = state.metrics_handle.write().unwrap().take() {
        handle.abort();
    }
    state.proxy_state.set_upstream(String::new(), String::new());
    state.proxy_state.set_upstream_model(String::new());
    state.proxy_running.store(false, Ordering::SeqCst);

    // Persist state
    save_persisted_state(&state.state_file_path, &PersistedState { proxy_running: false });

    // Restore original config file from backup
    state.config_manager.restore_config();

    tracing::info!("Proxy stopped and config restored");
    Ok(())
}

#[tauri::command]
async fn start_proxy(app_handle: tauri::AppHandle, state: tauri::State<'_, TauriAppState>) -> Result<(), String> {
    if state.proxy_running.load(Ordering::SeqCst) {
        return Err("Proxy is already running".to_string());
    }

    let saved_configs = state.secure_store.list_configs();
    if saved_configs.is_empty() {
        return Err("No saved configs found".to_string());
    }

    // Find the matching config
    let current = state.config_manager.get_current_model();
    let matching = saved_configs.iter()
        .find(|c| c.model == current.model)
        .or_else(|| saved_configs.first());
    let cfg_name = matching.map(|c| c.name.clone()).ok_or_else(|| "No config found".to_string())?;
    let cfg = state.secure_store.get_config_full(&cfg_name)
        .ok_or_else(|| "Config data not found".to_string())?;

    let upstream = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let proxy_base_url = format!("http://127.0.0.1:{}/v1", PROXY_PORT);

    // Set up upstream state
    state.proxy_state.set_upstream(upstream, cfg.api_key.clone());
    state.proxy_state.set_upstream_model(strip_model_prefix(&cfg.model));

    // Backup original config before proxy modifies it
    state.config_manager.backup_config();
    // Update config file to point to proxy
    state.config_manager.apply_model(&cfg.model, &cfg.provider, &proxy_base_url, &cfg.api_key);

    // Start the proxy server
    let px = state.proxy_state.clone();
    let metrics = state.metrics.clone();
    let h = proxy::run_server(px.clone(), PROXY_PORT).await
        .ok_or_else(|| "Failed to start proxy server".to_string())?;

    // Store the handle
    *state.proxy_handle.write().unwrap() = Some(h);

    // Start metrics emission loop
    let mh = tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let snap = metrics.snapshot();
            let _ = app_handle.emit("metrics", snap);
        }
    });
    *state.metrics_handle.write().unwrap() = Some(mh);

    // Persist state
    save_persisted_state(&state.state_file_path, &PersistedState { proxy_running: true });

    tracing::info!("Proxy started: {} -> {} (model: {})", cfg_name, cfg.base_url, cfg.model);
    Ok(())
}

#[tauri::command]
fn get_upstream_info(state: tauri::State<TauriAppState>) -> serde_json::Value {
    serde_json::json!({
        "url": state.proxy_state.get_upstream_url(),
        "model": state.proxy_state.get_upstream_model(),
    })
}

#[tauri::command]
fn clear_session(state: tauri::State<TauriAppState>) {
    state.proxy_state.metrics.clear_session();
}

#[tauri::command]
fn get_model_catalog() -> ModelCatalog {
    match model_catalog::get_cached_catalog() {
        Some(cat) => {
            tracing::info!("Catalog: {} providers with {} total models",
                cat.providers.len(),
                cat.providers.iter().map(|p| p.models.len()).sum::<usize>());
            cat
        }
        None => {
            tracing::warn!("Catalog: cache miss or parse error");
            ModelCatalog { providers: vec![] }
        }
    }
}

#[tauri::command]
async fn refresh_model_catalog() -> Result<ModelCatalog, String> {
    model_catalog::refresh_catalog().await
}

#[tauri::command]
fn get_history_stats(state: tauri::State<TauriAppState>, dimension: String) -> Vec<history_store::AggregatedStat> {
    state.history_store.get_stats(&dimension)
}

#[tauri::command]
fn get_global_summary(state: tauri::State<TauriAppState>) -> history_store::GlobalSummary {
    state.history_store.global_summary()
}

// ── App Entry ──

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("proxy_tauri=info".parse().unwrap())
            .add_directive("axum=warn".parse().unwrap())
            .add_directive("tower_http=warn".parse().unwrap()))
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            let config_path = home.join(".codex").join("config.toml");
            let data_dir = home.join(".codex").join("proxy");
            std::fs::create_dir_all(&data_dir).ok();

            let db_path = data_dir.join("proxy_config.db");
            let key_file = data_dir.join(".proxy_key");
            let state_file_path = data_dir.join("proxy_state.json");

            // Load persisted proxy state
            let persisted = load_persisted_state(&state_file_path);
            let should_start_proxy = persisted.proxy_running;

            let metrics = Arc::new(Metrics::new(Arc::new(HistoryStore::new(&data_dir))));
            let config_manager = Arc::new(ConfigManager::new(config_path));
            let secure_store = Arc::new(SecureConfigStore::new(db_path, key_file));
            let proxy_running = Arc::new(AtomicBool::new(false));

            const DEFAULT_UPSTREAM_URL: &str = "https://coding.dashscope.aliyuncs.com/v1/chat/completions";
            const DEFAULT_UPSTREAM_MODEL: &str = "qwen3-coder-plus";
            const DEFAULT_API_KEY: &str = "sk-sp-9166e1c03e8b4c75b54fa1740a042ba0";

            let http_client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .no_proxy()
                .pool_idle_timeout(Some(std::time::Duration::from_secs(90)))
                .pool_max_idle_per_host(32)
                .tcp_keepalive(Some(std::time::Duration::from_secs(60)))
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());

            let proxy_state = Arc::new(AppState {
                metrics: metrics.clone(),
                http_client,
                upstream_url: RwLock::new(DEFAULT_UPSTREAM_URL.to_string()),
                api_key: RwLock::new(DEFAULT_API_KEY.to_string()),
                upstream_model: RwLock::new(DEFAULT_UPSTREAM_MODEL.to_string()),
                config_manager: config_manager.clone(),
                proxy_running: proxy_running.clone(),
                connectivity_result: RwLock::new(None),
            });

            // Load initial upstream config from saved store
            let current = config_manager.get_current_model();
            let saved_configs = secure_store.list_configs();
            let proxy_base_url = format!("http://127.0.0.1:{}/v1", PROXY_PORT);
            if saved_configs.is_empty() {
                // No saved configs: clear proxy state and config file
                proxy_state.set_upstream(String::new(), String::new());
                proxy_state.set_upstream_model(String::new());
                config_manager.backup_config();
                config_manager.apply_model("", "", "", "");
            } else {
                let matching_cfg = saved_configs.iter().find(|s| s.model == current.model);
                if let Some(cfg) = matching_cfg {
                    if let Some(full) = secure_store.get_config_full(&cfg.name) {
                        let upstream = format!("{}/chat/completions", full.base_url.trim_end_matches('/'));
                        config_manager.backup_config();
                        config_manager.apply_model(&cfg.model, &cfg.provider, &proxy_base_url, &full.api_key);
                        proxy_state.set_upstream(upstream, full.api_key);
                        proxy_state.set_upstream_model(strip_model_prefix(&cfg.model));
                    }
                } else if let Some(first_cfg) = saved_configs.first() {
                    if let Some(full) = secure_store.get_config_full(&first_cfg.name) {
                        let upstream = format!("{}/chat/completions", full.base_url.trim_end_matches('/'));
                        config_manager.backup_config();
                        config_manager.apply_model(&first_cfg.model, &first_cfg.provider, &proxy_base_url, &full.api_key);
                        proxy_state.set_upstream(upstream, full.api_key);
                        proxy_state.set_upstream_model(strip_model_prefix(&first_cfg.model));
                    }
                }
            }

            let tauri_state = TauriAppState {
                proxy_state: proxy_state.clone(),
                metrics: metrics.clone(),
                config_manager,
                secure_store,
                proxy_running: proxy_running.clone(),
                proxy_handle: RwLock::new(None),
                metrics_handle: RwLock::new(None),
                state_file_path,
                history_store: metrics.history_store.clone(),
            };

            app.manage(tauri_state);

            // Start proxy server only if it was running before
            if should_start_proxy && !saved_configs.is_empty() {
                let px = proxy_state.clone();
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(h) = proxy::run_server(px.clone(), PROXY_PORT).await {
                        // Store the handle so we can abort it later
                        if let Some(state) = app_handle.try_state::<TauriAppState>() {
                            *state.proxy_handle.write().unwrap() = Some(h);
                        }
                        // Emit metrics periodically
                        let metrics = px.metrics.clone();
                        let app_handle2 = app_handle.clone();
                        let mh = tokio::spawn(async move {
                            loop {
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                                let snap = metrics.snapshot();
                                let _ = app_handle2.emit("metrics", snap);
                            }
                        });
                        if let Some(state) = app_handle.try_state::<TauriAppState>() {
                            *state.metrics_handle.write().unwrap() = Some(mh);
                        }
                    }
                });
            }

            // Start background catalog refresh
            tauri::async_runtime::spawn(async {
                let _ = model_catalog::refresh_catalog().await;
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                #[cfg(any(target_os = "macos", target_os = "windows"))]
                {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_metrics,
            get_history_detail,
            get_history_stats,
            get_global_summary,
            get_current_config,
            get_saved_configs,
            get_config_full,
            save_config,
            delete_config,
            apply_config,
            test_connectivity,
            test_saved_config,
            get_proxy_status,
            get_upstream_info,
            clear_session,
            get_model_catalog,
            refresh_model_catalog,
            bypass_proxy,
            start_proxy,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
