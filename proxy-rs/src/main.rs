// Windows 平台隐藏控制台窗口
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod config;
mod convert;
mod error;
mod metrics;
mod model;
mod proxy;
mod sse;
mod ui;
#[cfg(target_os = "macos")]
mod macos_menu;
mod connectivity;

use crate::config::{ConfigManager, SecureConfigStore};
use crate::metrics::Metrics;
use crate::proxy::AppState;
use crate::ui::{render, UiState, PROXY_BASE_URL, PROXY_PROVIDER};
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

struct App {
    ui_state: UiState,
    metrics: Arc<Metrics>,
    config_manager: Arc<ConfigManager>,
    secure_store: Arc<SecureConfigStore>,
    proxy_running: Arc<AtomicBool>,
    app_state: Arc<AppState>,
    // Performance: track last generation for smart repaint
    last_generation: u64,
}

impl App {
    fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

        // Codex native config path (all platforms):
        //   ~/.codex/config.toml
        //   macOS:   /Users/<user>/.codex/config.toml
        //   Linux:   /home/<user>/.codex/config.toml
        //   Windows: C:\Users\<user>\.codex\config.toml
        let config_path = home.join(".codex").join("config.toml");

        // Proxy's own data files — all platforms under ~/.codex/proxy/
        //   macOS:   /Users/<user>/.codex/proxy/
        //   Linux:   /home/<user>/.codex/proxy/
        //   Windows: C:\Users\<user>\.codex\proxy\
        let data_dir = home.join(".codex").join("proxy");
        std::fs::create_dir_all(&data_dir).ok();

        let db_path = data_dir.join("proxy_config.db");
        let key_file = data_dir.join(".proxy_key");

        let metrics = Arc::new(Metrics::new());
        let config_manager = Arc::new(ConfigManager::new(config_path));
        let secure_store = Arc::new(SecureConfigStore::new(db_path, key_file));
        let proxy_running = Arc::new(AtomicBool::new(false));

        // Default upstream configuration (DashScope Coding Plan)
        const DEFAULT_UPSTREAM_URL: &str = "https://coding.dashscope.aliyuncs.com/v1/chat/completions";
        const DEFAULT_UPSTREAM_MODEL: &str = "qwen3-coder-plus";
        const DEFAULT_API_KEY: &str = "sk-sp-9166e1c03e8b4c75b54fa1740a042ba0";

        // Create shared AppState – upstream_url and api_key are dynamic (RwLock)
        // Initialize with defaults, will be overridden by saved configs if available
        // Create reusable HTTP client with optimized settings
        let http_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .pool_idle_timeout(Some(std::time::Duration::from_secs(90)))
            .pool_max_idle_per_host(32)
            .tcp_keepalive(Some(std::time::Duration::from_secs(60)))
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let app_state = Arc::new(AppState {
            metrics: metrics.clone(),
            http_client,
            upstream_url: RwLock::new(DEFAULT_UPSTREAM_URL.to_string()),
            api_key: RwLock::new(DEFAULT_API_KEY.to_string()),
            upstream_model: RwLock::new(DEFAULT_UPSTREAM_MODEL.to_string()),
            config_manager: config_manager.clone(),
            proxy_running: proxy_running.clone(),
            log_level: tracing::Level::INFO,
            connectivity_result: RwLock::new(None),
        });

        // Read current config from config.toml
        let current = config_manager.get_current_model();

        // Try to load initial upstream config matching current config.toml model (overrides defaults)
        let saved_configs = secure_store.list_configs();
        let matching_cfg = saved_configs.iter().find(|s| s.model == current.model);
        if let Some(cfg) = matching_cfg {
            if let Some(full) = secure_store.get_config_full(&cfg.name) {
                let upstream = format!("{}/chat/completions", full.base_url.trim_end_matches('/'));
                app_state.set_upstream(upstream, full.api_key);
                app_state.set_upstream_model(cfg.model.clone());
                tracing::info!("Initial upstream loaded from saved config '{}': {} (model: {})", cfg.name, full.base_url, cfg.model);
            }
        } else if let Some(first_cfg) = saved_configs.first() {
            // Fallback: no matching config found, use most recently updated
            if let Some(full) = secure_store.get_config_full(&first_cfg.name) {
                let upstream = format!("{}/chat/completions", full.base_url.trim_end_matches('/'));
                app_state.set_upstream(upstream, full.api_key);
                app_state.set_upstream_model(first_cfg.model.clone());
                tracing::info!("Initial upstream loaded from saved config '{}': {} (model: {}) (fallback)", first_cfg.name, first_cfg.base_url, first_cfg.model);
            }
        } else {
            tracing::info!("Using default upstream: {} (model: {})", DEFAULT_UPSTREAM_URL, DEFAULT_UPSTREAM_MODEL);
        }
        // On startup, ensure config.toml always points to the proxy
        if current.provider != PROXY_PROVIDER || current.base_url != PROXY_BASE_URL {
            let model = if current.model.is_empty() {
                secure_store.list_configs().first().map(|s| s.model.clone()).unwrap_or_default()
            } else {
                current.model.clone()
            };
            if !model.is_empty() {
                config_manager.apply_model(&model, PROXY_PROVIDER, PROXY_BASE_URL, "");
                tracing::info!("config.toml auto-corrected: provider={}, base_url={}", PROXY_PROVIDER, PROXY_BASE_URL);
            }
        }

        // Start proxy server on background thread
        let proxy_state = app_state.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                proxy::run_server(proxy_state).await;
            });
        });

        tracing::info!("Codex Proxy Monitor started");

        Self {
            ui_state: UiState::new(),
            metrics,
            config_manager,
            secure_store,
            proxy_running,
            app_state,
            // Initialize performance tracking
            last_generation: 0,
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Schedule next repaint based on metrics state
        let gen = self.metrics.generation();

        if gen != self.last_generation {
            self.last_generation = gen;
            // 新记录到达：立即请求下一帧重绘
            ctx.request_repaint();
        } else if self.metrics.has_active_stream() {
            // 活动流中 generation 不变，但需要持续重绘以更新实时面板（约 60fps）
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        } else {
            // 空闲轮询：确保跨线程新增的记录能及时显示（200ms 检测周期）
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        render(
            ui,
            &mut self.ui_state,
            &self.metrics,
            &self.config_manager,
            &self.secure_store,
            &self.app_state,
            self.proxy_running.load(Ordering::Relaxed),
        );
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct WindowState {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

fn load_window_state() -> Option<WindowState> {
    let home = std::env::var("HOME").ok()?;
    let path = std::path::Path::new(&home).join(".codex/proxy/window_state.json");
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_window_state(state: &WindowState) {
    if let Ok(home) = std::env::var("HOME") {
        let path = std::path::Path::new(&home).join(".codex/proxy/window_state.json");
        if let Ok(parent) = path.parent().ok_or(()) {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(state) {
            let _ = std::fs::write(path, json);
        }
    }
}

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Load saved window state
    let window_state = load_window_state();
    let default_size = [1200.0, 800.0];
    let size = window_state.as_ref().map(|s| [s.width, s.height]).unwrap_or(default_size);
    
    // Fixed window size for consistent UI
    let fixed_size = [1200.0, 800.0];
    
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(fixed_size)
        .with_resizable(false)  // Fix window size
        .with_title("Codex Proxy Monitor")
        .with_decorations(true);  // Keep window decorations (title bar)
    
    // Restore window position if available
    if let Some(state) = &window_state {
        viewport = viewport.with_position([state.x, state.y]);
    }
    
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Codex Proxy Monitor",
        options,
        Box::new(|cc| {
            // Load Chinese font for proper CJK rendering
            load_chinese_font(&cc.egui_ctx);
            
            // Setup custom menu bar on macOS
            #[cfg(target_os = "macos")]
            macos_menu::setup_custom_menu_bar();
            
            Ok(Box::new(App::new()))
        }),
    )
    .unwrap();
}


fn load_chinese_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    if let Some(bytes) = find_chinese_font() {
        fonts.font_data.insert(
            "chinese".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "chinese".to_owned());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("chinese".to_owned());
    } else {
        tracing::warn!("No Chinese font found, CJK may display as \u{25a1}");
    }

    ctx.set_fonts(fonts);
}

fn find_chinese_font() -> Option<Vec<u8>> {
    // Phase 1: platform-specific known paths
    let known: Vec<&str> = if cfg!(target_os = "macos") {
        vec![
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
            "/System/Library/Fonts/STHeiti Medium.ttc",
            "/System/Library/Fonts/Supplemental/Songti.ttc",
            "/Library/Fonts/Arial Unicode.ttf",
        ]
    } else if cfg!(target_os = "windows") {
        vec![
            "C:\\Windows\\Fonts\\msyh.ttc",
            "C:\\Windows\\Fonts\\msyh.ttf",
            "C:\\Windows\\Fonts\\simsun.ttc",
            "C:\\Windows\\Fonts\\simsun.ttf",
        ]
    } else {
        // Linux - extensive known paths
        vec![
            // Noto CJK / Noto Sans SC
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansSC-Regular.otf",
            "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/google-noto-cjk/NotoSansSC-Regular.otf",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansSC-Regular.otf",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansSC-Regular.otf",
            "/usr/share/fonts/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto/NotoSansSC-Regular.otf",
            // WenQuanYi
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
            "/usr/share/fonts/wenquanyi/wqy-microhei/wqy-microhei.ttc",
            "/usr/share/fonts/wenquanyi/wqy-zenhei/wqy-zenhei.ttc",
            "/usr/share/fonts/wenquanyi/microhei/wqy-microhei.ttc",
            "/usr/share/fonts/wqy-microhei/wqy-microhei.ttc",
            "/usr/share/fonts/wqy-zenhei/wqy-zenhei.ttc",
            // Droid Sans Fallback
            "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
            "/usr/share/fonts/truetype/droid/DroidSansFallback.ttf",
            "/usr/share/fonts/droid/DroidSansFallbackFull.ttf",
            // AR PL UMing / UKai
            "/usr/share/fonts/truetype/arphic/uming.ttc",
            "/usr/share/fonts/truetype/arphic/ukai.ttc",
            "/usr/share/fonts/arphic/uming.ttc",
            "/usr/share/fonts/arphic/ukai.ttc",
            // Source Han Sans
            "/usr/share/fonts/adobe-source-han-sans/SourceHanSansSC-Regular.otf",
            "/usr/share/fonts/opentype/source-han-sans/SourceHanSansSC-Regular.otf",
            "/usr/share/fonts/source-han-sans/SourceHanSansSC-Regular.otf",
            // Other
            "/usr/share/fonts/truetype/HanNom.ttf",
            "/usr/local/share/fonts/wqy-microhei.ttc",
            "/usr/local/share/fonts/NotoSansCJK-Regular.ttc",
        ]
    };

    for p in &known {
        if let Ok(bytes) = std::fs::read(p) {
            tracing::info!("Loaded Chinese font: {}", p);
            return Some(bytes);
        }
    }

    // Phase 2 (Linux): recursive scan of font directories
    #[cfg(target_os = "linux")]
    {
        let cjk_patterns = [
            "CJK", "cjk", "Hans", "hans", "SC-", "sc-",
            "wqy", "microhei", "zenhei", "WenQuanYi",
            "NotoSans", "noto",
            "DroidSansFallback", "droid",
            "uming", "ukai", "arphic",
            "SourceHan", "source-han",
            "HanNom",
        ];

        for root in &["/usr/share/fonts", "/usr/local/share/fonts"] {
            if let Some(bytes) = scan_dir_for_cjk(root, &cjk_patterns) {
                return Some(bytes);
            }
        }

        // Phase 3: fc-list as last resort
        if let Ok(output) = std::process::Command::new("fc-list")
            .arg(":lang=zh")
            .arg("file")
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    let path = line.trim().split(':').next().unwrap_or("");
                    if let Ok(bytes) = std::fs::read(path) {
                        tracing::info!("Loaded Chinese font via fc-list: {}", path);
                        return Some(bytes);
                    }
                }
            }
        }
    }

    None
}

/// Recursively scan a font directory for CJK font files.
fn scan_dir_for_cjk(root: &str, patterns: &[&str]) -> Option<Vec<u8>> {
    fn walk(dir: &Path, patterns: &[&str]) -> Option<Vec<u8>> {
        for entry in std::fs::read_dir(dir).ok()?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(bytes) = walk(&path, patterns) {
                    return Some(bytes);
                }
            } else if path.is_file() {
                let ext = path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                if !matches!(ext.to_lowercase().as_str(), "ttf" | "ttc" | "otf") {
                    continue;
                }
                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                for pat in patterns {
                    if name.contains(pat) {
                        if let Ok(bytes) = std::fs::read(&path) {
                            tracing::info!(
                                "Found CJK font by pattern '{}': {}",
                                pat,
                                path.display()
                            );
                            return Some(bytes);
                        }
                    }
                }
            }
        }
        None
    }

    walk(Path::new(root), patterns)
}
