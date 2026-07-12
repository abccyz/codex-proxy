use eframe::egui;
use egui::{
    Align, Color32, Context, Frame, Layout, Margin, RichText, ScrollArea,
    Sense, Stroke, vec2, CornerRadius,
};
use egui_plot::{Line, Plot, PlotPoints};
use std::collections::HashMap;
use std::sync::Arc;
use once_cell::sync::Lazy;

use crate::config::{ConfigManager, SavedConfig, SecureConfigStore};
use crate::connectivity::{self, ConnectivityResult};
use crate::model::{get_provider_presets, ProviderPreset};

/// Proxy provider name and base URL written to config.toml
pub const PROXY_PROVIDER: &str = "Model_Studio";
pub const PROXY_BASE_URL: &str = "http://127.0.0.1:8000/v1";
use crate::metrics::{Metrics, SharedMetrics, Snapshot};
use crate::proxy::AppState;

// ═══════════════════════════════════════════════════════════════
// i18n
// ═══════════════════════════════════════════════════════════════

#[derive(Clone, Copy, PartialEq)]
pub enum Lang { Zh, En }

impl Lang {
    fn toggle(self) -> Self { match self { Self::Zh => Self::En, Self::En => Self::Zh } }
    fn label(self) -> &'static str { match self { Self::Zh => "中文", Self::En => "EN" } }
}

static I18N_ZH: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    HashMap::from([
        ("title","Codex 代理监控"),("tab_dashboard","仪表盘"),("tab_config","配置中心"),
        ("stat_total","总请求数"),("stat_success","成功"),("stat_failed","失败"),
        ("stat_latency","平均延迟"),("stat_streams","活跃流"),("stat_tokens","总 Token"),
        ("chart_throughput","吞吐量 (请求/分钟)"),("chart_latency","延迟趋势"),
        ("chart_models","模型使用量"),("chart_tokens","Token 用量"),
        ("token_input","输入"),("token_output","输出"),
        ("table_history","请求历史"),("th_time","时间"),("th_model","模型"),
        ("th_type","类型"),("th_status","状态"),("th_latency","延迟"),
        ("th_tokens","Token (入/出)"),("th_input","输入"),("th_output","输出"),
        ("detail_title","内容详情"),("detail_instructions","系统指令"),
        ("detail_messages","消息"),("detail_tools","工具定义"),("detail_output","模型输出"),
        ("detail_model","模型"),("detail_time","时间"),("detail_tokens","Token"),
        ("detail_latency","延迟"),("detail_status","状态"),
        ("config_quick","快速配置"),("config_name","配置名称"),
        ("config_provider","Provider"),("config_base_url","Base URL"),("config_api_key","API Key"),("config_model","Model"),
        ("config_name_ph","例: my-key"),("config_saved","已保存配置 (加密)"),("badge_active","当前使用"),
        ("toast_apply_ok","配置已应用"),("toast_save_apply_ok","配置已保存并应用"),
        ("config_status","代理状态"),
        ("config_preset","供应商"),("config_preset_custom","自定义"),
        ("btn_test","测试连接"),("btn_testing","测试中..."),
        ("btn_edit","编辑"),("btn_cancel","取消"),("btn_confirm_delete","确认删除"),
        ("test_success","连接成功"),("test_failed","连接失败"),
        ("test_latency","延迟"),("test_models","可用模型"),
        ("history_search","搜索"),("history_no_results","无匹配结果"),
        ("delete_confirm_title","确认删除"),("delete_confirm_msg","确定要删除配置 '{}' 吗？"),
        ("btn_apply","应用"),("btn_reload","重新加载"),
        ("btn_back","返回"),("btn_page","页"),("btn_delete","删除"),
        ("empty_data","暂无数据"),("empty_history","暂无请求，请在 Codex 中开始对话！"),
        ("empty_config","暂无保存的配置"),
        ("uptime","运行时间"),("rpm","请求/分钟"),("current","当前"),("via","通过"),
        ("proxy_status","代理服务"),("running","运行中"),("stopped","已停止"),
        ("theme_light","亮色"),("theme_dark","暗色"),
        ("status_ok","OK"),("status_err","ERR"),
        ("type_stream","stream"),("type_sync","sync"),("no_tools","无工具定义"),
    ])
});

static I18N_EN: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    HashMap::from([
        ("title","Codex Proxy Monitor"),("tab_dashboard","Dashboard"),("tab_config","Configuration"),
        ("stat_total","Total Requests"),("stat_success","Success"),("stat_failed","Failed"),
        ("stat_latency","Avg Latency"),("stat_streams","Active Streams"),("stat_tokens","Total Tokens"),
        ("chart_throughput","Throughput (req/min)"),("chart_latency","Latency Trend"),
        ("chart_models","Model Usage"),("chart_tokens","Token Usage"),
        ("token_input","Input"),("token_output","Output"),
        ("table_history","Request History"),("th_time","Time"),("th_model","Model"),
        ("th_type","Type"),("th_status","Status"),("th_latency","Latency"),
        ("th_tokens","Tokens"),("th_input","Input"),("th_output","Output"),
        ("detail_title","Content Detail"),("detail_instructions","Instructions"),
        ("detail_messages","Messages"),("detail_tools","Tools"),("detail_output","Model Output"),
        ("detail_model","Model"),("detail_time","Time"),("detail_tokens","Tokens"),
        ("detail_latency","Latency"),("detail_status","Status"),
        ("config_quick","Quick Config"),("config_name","Config Name"),
        ("config_provider","Provider"),("config_base_url","Base URL"),("config_api_key","API Key"),("config_model","Model"),
        ("config_name_ph","e.g. my-key"),("config_saved","Saved Configs (Encrypted)"),("badge_active","Active"),
        ("toast_apply_ok","Config applied"),("toast_save_apply_ok","Config saved & applied"),
        ("config_status","Proxy Status"),
        ("config_preset","Provider"),("config_preset_custom","Custom"),
        ("btn_test","Test Connection"),("btn_testing","Testing..."),
        ("btn_edit","Edit"),("btn_cancel","Cancel"),("btn_confirm_delete","Confirm Delete"),
        ("test_success","Connection OK"),("test_failed","Connection Failed"),
        ("test_latency","Latency"),("test_models","Available Models"),
        ("history_search","Search"),("history_no_results","No matching results"),
        ("delete_confirm_title","Confirm Delete"),("delete_confirm_msg","Are you sure you want to delete config '{}'?"),
        ("btn_apply","Apply"),("btn_reload","Reload"),
        ("btn_back","Back"),("btn_page","Page"),("btn_delete","Delete"),
        ("empty_data","No data yet"),("empty_history","No requests yet."),
        ("empty_config","No saved configs yet."),
        ("uptime","Uptime"),("rpm","req/min"),("current","Current"),("via","via"),
        ("proxy_status","Proxy"),("running","Running"),("stopped","Stopped"),
        ("theme_light","Light"),("theme_dark","Dark"),
        ("status_ok","OK"),("status_err","ERR"),
        ("type_stream","stream"),("type_sync","sync"),("no_tools","No tools defined"),
    ])
});

fn t(lang: Lang, key: &'static str) -> &'static str {
    match lang {
        Lang::Zh => I18N_ZH.get(key).copied().unwrap_or(key),
        Lang::En => I18N_EN.get(key).copied().unwrap_or(key),
    }
}

// ═══════════════════════════════════════════════════════════════
// Colors – high contrast, readable
// ═══════════════════════════════════════════════════════════════

struct C {
    bg:          Color32,   // page background
    surface:     Color32,   // content surface – modal body / scroll bg
    card:        Color32,   // card / section background
    elev:        Color32,   // elevated / input / alternate-row
    border:      Color32,   // subtle border
    text:        Color32,   // primary text
    text2:       Color32,   // secondary / label text
    text3:       Color32,   // muted / placeholder
    green:       Color32,
    blue:        Color32,
    red:         Color32,
    yellow:      Color32,
    purple:      Color32,
}

fn colors(dark: bool) -> C {
    if dark {
        C {
            bg:     Color32::from_rgb(12, 14, 22),     // deep blue-black – page only
            surface:Color32::from_rgb(24, 30, 50),     // content surface – clearly lifts from bg
            card:   Color32::from_rgb(34, 42, 66),     // card – distinct from surface
            elev:   Color32::from_rgb(48, 58, 86),     // hover / input bg
            border: Color32::from_rgb(65, 78, 110),    // visible separation
            text:   Color32::from_rgb(238, 243, 252),  // near-white primary text
            text2:  Color32::from_rgb(195, 205, 225),  // secondary – clearly readable
            text3:  Color32::from_rgb(148, 158, 188),  // muted – usable
            green:  Color32::from_rgb(74, 222, 128),
            blue:   Color32::from_rgb(129, 180, 255),
            red:    Color32::from_rgb(252, 129, 129),
            yellow: Color32::from_rgb(253, 224, 71),
            purple: Color32::from_rgb(221, 190, 255),
        }
    } else {
        C {
            bg:     Color32::from_rgb(243, 245, 249),
            surface:Color32::from_rgb(250, 251, 253),
            card:   Color32::WHITE,
            elev:   Color32::from_rgb(231, 235, 242),
            border: Color32::from_rgb(210, 218, 230),
            text:   Color32::from_rgb(15, 23, 42),
            text2:  Color32::from_rgb(71, 85, 105),
            text3:  Color32::from_rgb(130, 145, 165),
            green:  Color32::from_rgb(5, 150, 105),
            blue:   Color32::from_rgb(37, 99, 235),
            red:    Color32::from_rgb(220, 38, 38),
            yellow: Color32::from_rgb(202, 138, 4),
            purple: Color32::from_rgb(147, 51, 234),
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// UI State
// ═══════════════════════════════════════════════════════════════

#[derive(Clone, Copy, PartialEq)]
pub enum ActiveTab { Dashboard, Config }

#[derive(Clone, Copy, PartialEq)]
pub enum DetailMode { Instructions, Messages, Tools, Output }

pub struct UiState {
    pub lang: Lang,
    pub dark_mode: bool,
    last_dark_mode: bool,  // 追踪上次 theme 状态，仅在变化时重建 style
    pub active_tab: ActiveTab,
    pub history_page: usize,
    pub page_size: usize,
    pub detail_idx: Option<usize>,
    pub detail_mode: DetailMode,
    pub config_name: String,
    #[allow(dead_code)]
    pub config_provider: String,
    pub config_base_url: String,
    pub config_api_key: String,
    pub config_model: String,
    pub current_config: crate::config::CurrentConfig,
    pub saved_configs: Vec<SavedConfig>,
    pub config_loaded: bool,
    pub toast_msg: Option<String>,
    pub toast_time: f64,
    // Provider preset selection
    pub selected_preset: String,
    pub presets: Vec<ProviderPreset>,
    // Connectivity test state
    pub connectivity_result: Option<ConnectivityResult>,
    pub connectivity_testing: bool,
    // History search and sort
    pub history_search: String,
    pub history_sort_by: HistorySortBy,
    pub history_sort_asc: bool,
    // Delete confirmation
    pub delete_confirm_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HistorySortBy {
    Time,
    Tokens,
    Latency,
}

impl UiState {
    pub fn new() -> Self {
        Self {
            lang: Lang::Zh, dark_mode: true, last_dark_mode: true, active_tab: ActiveTab::Dashboard,
            history_page: 1, page_size: 20, detail_idx: None,
            detail_mode: DetailMode::Messages,
            config_name: String::new(),
            config_provider: String::new(),
            config_base_url: "".into(),
            config_api_key: String::new(),
            config_model: "".into(),
            current_config: crate::config::CurrentConfig { model: String::new(), provider: String::new(), base_url: String::new() },
            saved_configs: vec![], config_loaded: false,
            toast_msg: None, toast_time: 0.0,
            selected_preset: "qwen".to_string(),
            presets: get_provider_presets(),
            connectivity_result: None,
            connectivity_testing: false,
            history_search: String::new(),
            history_sort_by: HistorySortBy::Time,
            history_sort_asc: false,
            delete_confirm_name: None,
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════

fn fmt_uptime(s: u64) -> String {
    let h = s/3600; let m = (s%3600)/60; let sec = s%60;
    if h > 0 { format!("{}h {}m {}s", h, m, sec) } else if m > 0 { format!("{}m {}s", m, sec) } else { format!("{}s", sec) }
}
fn trunc(s: &str, n: usize) -> String {
    if s.len() > n {
        // Find largest char boundary <= n to avoid UTF-8 panic
        let mut end = n;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    } else {
        s.to_string()
    }
}
fn fmt_num(n: u64) -> String {
    if n >= 1_000_000 { format!("{:.1}M", n as f64 / 1_000_000.0) }
    else if n >= 10_000 { format!("{:.1}k", n as f64 / 1_000.0) }
    else { format!("{}", n) }
}

fn section_label(ui: &mut egui::Ui, c: &C, label: &str) {
    ui.label(RichText::new(label).color(c.text3).size(10.0).strong());
    ui.add_space(6.0);
}

fn card_frame<'a>(c: &C) -> Frame {
    Frame::NONE
        .fill(c.card)
        .stroke(Stroke::new(1.0, c.border))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(12, 10))
}

/// Content card in detail modal – uses elev for max distinctness from surface bg
fn content_card<'a>(c: &C, _border_color: Color32) -> Frame {
    Frame::NONE
        .fill(c.elev)
        .stroke(Stroke::new(1.0, c.border))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(14, 12))
}

// ═══════════════════════════════════════════════════════════════
// Main render entry
// ═══════════════════════════════════════════════════════════════

pub fn render(
    ui: &mut egui::Ui, state: &mut UiState, metrics: &SharedMetrics,
    cm: &ConfigManager, ss: &SecureConfigStore, app_state: &Arc<AppState>, proxy_running: bool,
) {
    let ctx = ui.ctx().clone();
    let c = colors(state.dark_mode);
    let lang = state.lang;

    // 仅在 theme 变化时重建并设置 Style（避免每帧克隆+布局）
    if state.dark_mode != state.last_dark_mode {
        state.last_dark_mode = state.dark_mode;
        let mut style = (*ctx.global_style()).clone();
        let vis = &mut style.visuals;
        vis.window_fill = c.bg;
        vis.panel_fill = c.bg;
        vis.extreme_bg_color = c.bg;
        vis.window_corner_radius = egui::CornerRadius::same(8);
        vis.window_stroke = Stroke::new(1.0, c.border);
        vis.window_shadow = egui::epaint::Shadow::NONE;
        vis.widgets.noninteractive.fg_stroke = Stroke::new(1.0, c.text);
        vis.widgets.noninteractive.bg_fill = Color32::TRANSPARENT;
        vis.widgets.noninteractive.weak_bg_fill = Color32::TRANSPARENT;
        vis.widgets.inactive.fg_stroke = Stroke::new(1.0, c.text);
        vis.widgets.inactive.bg_fill = c.elev;
        vis.widgets.inactive.weak_bg_fill = c.elev;
        vis.widgets.hovered.fg_stroke = Stroke::new(1.0, c.blue);
        vis.widgets.hovered.bg_fill = c.border;
        vis.widgets.hovered.weak_bg_fill = c.elev;
        vis.widgets.active.fg_stroke = Stroke::new(2.0, c.blue);
        vis.widgets.active.bg_fill = c.blue.linear_multiply(0.3);
        vis.widgets.active.weak_bg_fill = c.elev;
        vis.widgets.open.fg_stroke = Stroke::new(1.5, Color32::WHITE);
        vis.widgets.open.bg_fill = c.blue.linear_multiply(0.30);
        vis.widgets.open.weak_bg_fill = c.blue.linear_multiply(0.30);
        vis.selection.bg_fill = c.blue.linear_multiply(0.35);
        vis.selection.stroke = Stroke::new(1.0, c.blue);
        vis.text_cursor = egui::style::TextCursorStyle {
            stroke: Stroke::new(2.0, c.blue),
            ..Default::default()
        };
        vis.hyperlink_color = c.blue;
        ctx.set_global_style(style);
    }

    let snap = metrics.snapshot();
    let viewport_h = ui.available_rect_before_wrap().height();

    // ── Top bar ──
    egui::Panel::top("bar").frame(Frame { fill: c.card, inner_margin: Margin::symmetric(10, 6), ..Default::default() }).show(ui, |ui| {
        ui.horizontal(|ui| {
            // Status dot + proxy server status
            let (dot_color, status_text) = if proxy_running {
                (c.green, t(lang,"running"))
            } else {
                (c.red, t(lang,"stopped"))
            };
            ui.label(RichText::new("●").color(dot_color).size(9.0));
            ui.label(RichText::new(format!("{} {}", t(lang,"proxy_status"), status_text)).color(dot_color).size(11.0).strong());
            ui.separator();
            ui.label(RichText::new(t(lang,"title")).color(c.text).size(14.0).strong());
            let model_name = cm.get_current_model().model;
            if !model_name.is_empty() {
                ui.label(RichText::new(format!(" — {}", model_name)).color(c.green).size(12.0).strong());
            }
            ui.label(RichText::new(format!(" | {}: {}", t(lang,"uptime"), fmt_uptime(snap.uptime))).color(c.text2).size(10.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                if ui.small_button(RichText::new(state.lang.label()).color(c.text2).size(11.0)).clicked() { state.lang = state.lang.toggle(); }
                if ui.small_button(RichText::new(if state.dark_mode { t(lang,"theme_light") } else { t(lang,"theme_dark") }).color(c.text2).size(11.0)).clicked() { state.dark_mode = !state.dark_mode; ctx.request_repaint(); }
            });
        });
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.selectable_value(&mut state.active_tab, ActiveTab::Dashboard, RichText::new(t(lang,"tab_dashboard")).size(12.0));
            ui.selectable_value(&mut state.active_tab, ActiveTab::Config, RichText::new(t(lang,"tab_config")).size(12.0));
    });
    });

    // ── Content ──
    egui::CentralPanel::default_margins().frame(Frame::NONE.fill(c.bg).inner_margin(Margin::symmetric(12, 6))).show(ui, |ui| {
        match state.active_tab {
            ActiveTab::Dashboard => {
                render_dashboard(ui, state, &snap, app_state, lang, &c, viewport_h);
            }
            ActiveTab::Config => render_config(ui, state, cm, ss, app_state, lang, &c, viewport_h),
        }
    });

    // ── Keyboard Shortcuts ──
    ctx.input(|i| {
        let cmd_or_ctrl = i.modifiers.command || i.modifiers.ctrl;
        
        // Cmd/Ctrl+1: Switch to Dashboard
        if cmd_or_ctrl && i.key_pressed(egui::Key::Num1) {
            state.active_tab = ActiveTab::Dashboard;
        }
        // Cmd/Ctrl+2: Switch to Config
        if cmd_or_ctrl && i.key_pressed(egui::Key::Num2) {
            state.active_tab = ActiveTab::Config;
        }
        // Esc: Close detail modal or clear search
        if i.key_pressed(egui::Key::Escape) {
            if state.detail_idx.is_some() {
                state.detail_idx = None;
            } else if !state.history_search.is_empty() {
                state.history_search.clear();
            }
        }
    });

    // ── Detail Modal (overlay) ──
    if state.detail_idx.is_some() {
        render_detail_modal(&ctx, state, metrics, &snap, lang, &c);
    }
}

// ═══════════════════════════════════════════════════════════════
// Dashboard cards
// ═══════════════════════════════════════════════════════════════

fn stat_card(ui: &mut egui::Ui, c: &C, accent: Color32, label: &str, value: &str, sub: &str) {
    // Calculate responsive sizing
    let available_w = ui.available_width();
    let min_width = (available_w * 0.85).max(80.0);  // At least 80px wide
    
    Frame::NONE
        .fill(c.card)
        .stroke(Stroke::new(1.0, c.border))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(8, 8))
        .show(ui, |ui| {
            ui.set_clip_rect(ui.max_rect());  // clip to card bounds
            ui.set_min_width(min_width);
            ui.set_max_width(available_w);
            
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.label(RichText::new(label).color(c.text3).size(9.5));
                ui.add_space(3.0);
                ui.label(RichText::new(value).color(accent).size(20.0).strong());
                if !sub.is_empty() {
                    ui.add_space(1.0);
                    ui.label(RichText::new(sub).color(c.text2).size(8.5));
                }
            });
        });
}

// ═══════════════════════════════════════════════════════════════
// Dashboard
// ═══════════════════════════════════════════════════════════════

fn render_dashboard(ui: &mut egui::Ui, state: &mut UiState, snap: &Snapshot, app_state: &Arc<AppState>, lang: Lang, c: &C, viewport_h: f32) {
    // Chart heights scale with viewport: 12% of viewport, clamped
    let chart_h = (viewport_h * 0.12).clamp(80.0, 160.0);
    // History table: ~35% of viewport, clamped
    let table_h = (viewport_h * 0.35).clamp(200.0, 500.0);

    ScrollArea::vertical().auto_shrink([true, false]).show(ui, |ui| {
        ui.push_id("dashboard_scroll", |ui| {
        // ── Stat cards (3 columns × 2 rows) ──
        // 6 columns is too narrow for subtitle text — causes cascade overflow.
        // 3 columns gives each card 2× the space, guaranteeing no overflow.
        let col_spacing = 10.0;
        ui.spacing_mut().item_spacing.x = col_spacing;
        
        // Row 1: Total, Success, Failed
        ui.columns(3, |cols| {
            stat_card(&mut cols[0], c, c.blue,   t(lang,"stat_total"),   &fmt_num(snap.total), &format!("{} {}", snap.rpm, t(lang,"rpm")));
            stat_card(&mut cols[1], c, c.green,  t(lang,"stat_success"),  &fmt_num(snap.success), &format!("{}%", if snap.total>0 {snap.success*100/snap.total}else{0}));
            stat_card(&mut cols[2], c, c.red,    t(lang,"stat_failed"),   &fmt_num(snap.failed), "");
        });
        ui.add_space(8.0);
        // Row 2: Latency, Streams, Tokens
        ui.columns(3, |cols| {
            stat_card(&mut cols[0], c, c.yellow, t(lang,"stat_latency"),  &format!("{:.2}s", snap.avg_latency), "");
            stat_card(&mut cols[1], c, c.purple, t(lang,"stat_streams"),  &format!("{}", snap.active_streams), "");
            stat_card(&mut cols[2], c, c.blue,   t(lang,"stat_tokens"),   &fmt_num(snap.total_tokens), &format!("{}/{}", fmt_num(snap.total_input_tokens), fmt_num(snap.total_output_tokens)));
        });
        
        ui.add_space(12.0);

        // ── Throughput bars ──
        card_frame(c).show(ui, |ui| {
            section_label(ui, c, t(lang,"chart_throughput"));
            let max_c = snap.throughput.iter().map(|tp| tp.c).max().unwrap_or(1);
            let h = chart_h;
            let desired = vec2(ui.available_width(), h);
            let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
            if rect.width() > 0.0 {
                let painter = ui.painter_at(rect);
                // Removed transparent rect - unnecessary drawing
                if !snap.throughput.is_empty() {
                    let n = snap.throughput.len();
                    let bar_w = ((rect.width() - 2.0) / n as f32).max(3.0);
                    // Y-axis labels
                    painter.text(
                        egui::pos2(rect.min.x + 2.0, rect.min.y + 8.0),
                        egui::Align2::LEFT_TOP,
                        format!("{}", max_c),
                        egui::FontId::proportional(8.0), c.text3,
                    );
                    painter.text(
                        egui::pos2(rect.min.x + 2.0, rect.max.y - 16.0),
                        egui::Align2::LEFT_BOTTOM,
                        "0",
                        egui::FontId::proportional(8.0), c.text3,
                    );
                    for (i, tp) in snap.throughput.iter().enumerate() {
                        let ratio = tp.c as f32 / max_c as f32;
                        let bar_h = (ratio * (h - 24.0)).max(2.0);
                        let bar_rect = egui::Rect::from_min_size(
                            egui::pos2(rect.min.x + i as f32 * bar_w + 1.0, rect.max.y - 14.0 - bar_h),
                            vec2(bar_w - 1.0, bar_h),
                        );
                        painter.rect_filled(bar_rect, CornerRadius::same(2), c.blue);
                        // tooltip on hover
                        if bar_rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default())) {
                            painter.text(
                                egui::pos2(bar_rect.center().x, bar_rect.min.y - 6.0),
                                egui::Align2::CENTER_BOTTOM,
                                format!("{}", tp.c),
                                egui::FontId::proportional(9.0), c.text,
                            );
                        }
                    }
                } else {
                    painter.text(rect.center(), egui::Align2::CENTER_CENTER, t(lang,"empty_data"), egui::FontId::proportional(11.0), c.text3);
                }
            }
        });

        ui.add_space(8.0);

        // ── Latency chart + Model usage ──
        ui.horizontal(|ui| {
            let half = ((ui.available_width() - 6.0) / 2.0).max(100.0);
            // Latency chart
            card_frame(c).show(ui, |ui| {
                ui.set_min_width(half);
                section_label(ui, c, t(lang,"chart_latency"));
                let pts: PlotPoints = snap.latency_history.iter().enumerate().map(|(i,lp)| [i as f64, lp.v]).collect();
                Plot::new("lat").height(chart_h).show_x(false).show_y(false).show_axes([false,false]).show(ui, |pui| {
                    pui.line(Line::new("latency", pts).color(c.blue).width(1.5));
                });
            });
            ui.add_space(6.0);
            // Model usage
            card_frame(c).show(ui, |ui| {
                ui.set_min_width(half);
                section_label(ui, c, t(lang,"chart_models"));
                let mut entries: Vec<_> = snap.model_stats.iter().collect();
                entries.sort_by(|a,b| b.1.cmp(&a.1));
                if entries.is_empty() {
                    ui.label(RichText::new(t(lang,"empty_data")).color(c.text3));
                } else {
                    let max = entries[0].1.max(&1);
                    let row_h = 14.0;
                    let label_w = 80.0;
                    let num_w   = 30.0;
                    let gutter  = 6.0;
                    let bar_max_w = (half - label_w - num_w - gutter * 2.0).max(2.0);
                    for (name, cnt) in entries {
                        let desired = vec2(half, row_h);
                        let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
                        let p = ui.painter_at(rect);
                        // Label
                        p.text(
                            egui::pos2(rect.min.x, rect.center().y - 5.0),
                            egui::Align2::LEFT_TOP,
                            trunc(name, 12),
                            egui::FontId::monospace(10.0), c.text2,
                        );
                        // Bar
                        let bar_x = rect.min.x + label_w + gutter;
                        let bar_w = (*cnt as f32 / *max as f32 * bar_max_w).max(2.0);
                        let bar_rect = egui::Rect::from_min_size(
                            egui::pos2(bar_x, rect.center().y - 2.5),
                            vec2(bar_w, 5.0),
                        );
                        p.rect_filled(bar_rect, CornerRadius::same(3), c.blue);
                        // Count
                        let cnt_s = format!("{}", cnt);
                        p.text(
                            egui::pos2(bar_x + bar_w + gutter, rect.center().y - 5.0),
                            egui::Align2::LEFT_TOP,
                            cnt_s,
                            egui::FontId::proportional(10.0), c.text2,
                        );
                    }
                }
            });
        });

        ui.add_space(8.0);

        // ── Token usage bars (hand‑painted, zero‑overflow guarantee) ──
        card_frame(c).show(ui, |ui| {
            section_label(ui, c, t(lang,"chart_tokens"));
            let max_tok = snap.total_input_tokens.max(snap.total_output_tokens).max(1);
            let row_h = 18.0;
            let gap    = 6.0;   // vertical gap between input/output rows
            let gutter = 8.0;   // horizontal gap between label/bar/number
            // Layout: [label 60px]  gap  [bar fills rest]  gap  [number 65px]
            let label_w = 60.0;
            let num_w   = 65.0;
            let avail = ui.available_width();
            let bar_max_w = (avail - label_w - num_w - gutter * 2.0).max(2.0);

            // Render a single row: label text, bar, number text — all inside one allocate
            let render_token_row = |ui: &mut egui::Ui, label: &str, count: u64, accent: Color32| {
                let desired = vec2(avail, row_h);
                let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
                let p = ui.painter_at(rect);
                // Label – left-aligned, vertically centred
                let label_pos = egui::pos2(rect.min.x, rect.center().y - 6.0);
                p.text(label_pos, egui::Align2::LEFT_TOP, label, egui::FontId::proportional(11.0), c.text2);
                // Bar – fill space between label and number
                let bar_x = rect.min.x + label_w + gutter;
                let bar_w = (count as f32 / max_tok as f32 * bar_max_w).max(2.0);
                let bar_rect = egui::Rect::from_min_size(
                    egui::pos2(bar_x, rect.center().y - 5.0),
                    vec2(bar_w, 10.0),
                );
                p.rect_filled(bar_rect, CornerRadius::same(3), accent);
                // Number – right side, vertically centred
                let num_val = fmt_num(count);
                let galley = p.layout_no_wrap(num_val.clone(), egui::FontId::proportional(11.0), c.text2);
                let num_x = rect.max.x - galley.size().x;
                let num_pos = egui::pos2(num_x, rect.center().y - 6.0);
                p.text(num_pos, egui::Align2::LEFT_TOP, num_val, egui::FontId::proportional(11.0), c.text2);
            };

            render_token_row(ui, t(lang,"token_input"),  snap.total_input_tokens,  c.blue);
            ui.add_space(gap);
            render_token_row(ui, t(lang,"token_output"), snap.total_output_tokens, c.green);
        });

        ui.add_space(8.0);

        // ── History table ──
        render_history(ui, state, snap, lang, c, table_h);
    });  // end push_id
    });  // end ScrollArea
}

// ═══════════════════════════════════════════════════════════════
// History table
fn render_history(ui: &mut egui::Ui, state: &mut UiState, snap: &Snapshot, lang: Lang, c: &C, table_h: f32) {
    card_frame(c).show(ui, |ui| {
        ui.set_max_width(ui.available_width());  // Ensure card doesn't exceed available width
        section_label(ui, c, t(lang,"table_history"));
        
        // Search and sort controls
        ui.horizontal(|ui| {
            ui.label(RichText::new(t(lang,"history_search")).color(c.text2).size(10.0));
            ui.add(egui::TextEdit::singleline(&mut state.history_search)
                .hint_text("model/status...")
                .desired_width(150.0)
                .font(egui::FontId::proportional(10.0)));
            
            ui.separator();
            
            // Sort buttons
            let sort_label = match state.history_sort_by {
                HistorySortBy::Time => t(lang,"th_time"),
                HistorySortBy::Tokens => t(lang,"th_tokens"),
                HistorySortBy::Latency => t(lang,"th_latency"),
            };
            let sort_arrow = if state.history_sort_asc { "↑" } else { "↓" };
            
            if ui.small_button(RichText::new(format!("{} {}", sort_label, sort_arrow)).color(c.text2).size(10.0)).clicked() {
                // Cycle through sort options
                state.history_sort_by = match state.history_sort_by {
                    HistorySortBy::Time => HistorySortBy::Tokens,
                    HistorySortBy::Tokens => HistorySortBy::Latency,
                    HistorySortBy::Latency => HistorySortBy::Time,
                };
            }
            if ui.small_button(RichText::new(sort_arrow).color(c.text2).size(10.0)).clicked() {
                state.history_sort_asc = !state.history_sort_asc;
            }
        });
        
        ui.add_space(4.0);
        
        if snap.history.is_empty() {
            ui.label(RichText::new(t(lang,"empty_history")).color(c.text3));
            return;
        }

        // Filter and sort history
        let filtered_indices: Vec<usize> = snap.history.iter().enumerate()
            .filter(|(_, rec)| {
                if state.history_search.is_empty() {
                    return true;
                }
                let search_lower = state.history_search.to_lowercase();
                rec.model.to_lowercase().contains(&search_lower) ||
                rec.status.to_lowercase().contains(&search_lower) ||
                rec.input_preview.to_lowercase().contains(&search_lower) ||
                rec.preview.to_lowercase().contains(&search_lower)
            })
            .map(|(i, _)| i)
            .collect();
        
        // Sort indices
        let mut sorted_indices = filtered_indices;
        sorted_indices.sort_by(|&a, &b| {
            let rec_a = &snap.history[a];
            let rec_b = &snap.history[b];
            let cmp = match state.history_sort_by {
                HistorySortBy::Time => a.cmp(&b), // Index represents time order
                HistorySortBy::Tokens => (rec_a.input_tokens + rec_a.output_tokens)
                    .cmp(&(rec_b.input_tokens + rec_b.output_tokens)),
                HistorySortBy::Latency => rec_a.latency.partial_cmp(&rec_b.latency)
                    .unwrap_or(std::cmp::Ordering::Equal),
            };
            if state.history_sort_asc { cmp } else { cmp.reverse() }
        });
        
        if sorted_indices.is_empty() {
            ui.label(RichText::new(t(lang,"history_no_results")).color(c.text3));
            return;
        }

        let total = sorted_indices.len();
        let total_pages = (total + state.page_size - 1) / state.page_size;
        if state.history_page > total_pages { state.history_page = total_pages.max(1); }
        let start = (state.history_page - 1) * state.page_size;
        let end = (start + state.page_size).min(total);
        let page_indices = &sorted_indices[start..end];

        // Column widths for better display
        let col_widths = [70.0, 100.0, 60.0, 60.0, 60.0, 80.0, 120.0, 120.0];

        // Table with scrolling - match throughput card pattern
        let table_avail = ui.available_width();
        ScrollArea::horizontal().show(ui, |ui| {
            ui.set_max_width(table_avail);
            ScrollArea::vertical().max_height(table_h).show(ui, |ui| {
                egui::Grid::new("history_table")
                    .num_columns(8)
                    .spacing([8.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                    // Header with fixed column widths
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(col_widths[0]);
                        ui.label(RichText::new(t(lang,"th_time")).color(c.text3).size(9.0).strong());
                    });
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(col_widths[1]);
                        ui.label(RichText::new(t(lang,"th_model")).color(c.text3).size(9.0).strong());
                    });
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(col_widths[2]);
                        ui.label(RichText::new(t(lang,"th_type")).color(c.text3).size(9.0).strong());
                    });
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(col_widths[3]);
                        ui.label(RichText::new(t(lang,"th_status")).color(c.text3).size(9.0).strong());
                    });
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(col_widths[4]);
                        ui.label(RichText::new(t(lang,"th_latency")).color(c.text3).size(9.0).strong());
                    });
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(col_widths[5]);
                        ui.label(RichText::new(t(lang,"th_tokens")).color(c.text3).size(9.0).strong());
                    });
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(col_widths[6]);
                        ui.label(RichText::new(t(lang,"th_input")).color(c.text3).size(9.0).strong());
                    });
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(col_widths[7]);
                        ui.label(RichText::new(t(lang,"th_output")).color(c.text3).size(9.0).strong());
                    });
                    ui.end_row();

                    // Data rows (using sorted and filtered indices)
                    for &i in page_indices.iter().rev() {
                        if let Some(rec) = snap.history.get(i) {
                            // Time column
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.set_min_width(col_widths[0]);
                                ui.label(RichText::new(&rec.time).color(c.text2).size(10.0));
                            });
                            // Model column
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.set_min_width(col_widths[1]);
                                ui.label(RichText::new(&trunc(&rec.model,12)).color(c.text2).size(10.0));
                            });
                            // Type column
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.set_min_width(col_widths[2]);
                                let type_str = if rec.stream { t(lang,"type_stream") } else { t(lang,"type_sync") };
                                let type_color = if rec.stream { c.purple } else { c.blue };
                                ui.label(RichText::new(type_str).color(type_color).size(10.0));
                            });
                            // Status column
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.set_min_width(col_widths[3]);
                                let status_str = if rec.status == "success" { t(lang,"status_ok") } else { t(lang,"status_err") };
                                let status_color = if rec.status == "success" { c.green } else { c.red };
                                ui.label(RichText::new(status_str).color(status_color).size(10.0));
                            });
                            // Latency column
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.set_min_width(col_widths[4]);
                                ui.label(RichText::new(format!("{:.1}s", rec.latency)).color(c.text2).size(10.0));
                            });
                            // Tokens column
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.set_min_width(col_widths[5]);
                                ui.label(RichText::new(format!("{}/{}", rec.input_tokens, rec.output_tokens)).color(c.text2).size(10.0));
                            });
                            // Input preview column
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.set_min_width(col_widths[6]);
                                if ui.add(egui::Label::new(RichText::new(&trunc(&rec.input_preview, 15)).color(c.blue).size(10.0)).sense(Sense::click()))
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked() {
                                    state.detail_idx = Some(i); 
                                    state.detail_mode = DetailMode::Messages;
                                }
                            });
                            // Output preview column
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.set_min_width(col_widths[7]);
                                if ui.add(egui::Label::new(RichText::new(&trunc(&rec.preview, 15)).color(c.blue).size(10.0)).sense(Sense::click()))
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked() {
                                    state.detail_idx = Some(i); 
                                    state.detail_mode = DetailMode::Output;
                                }
                            });
                            ui.end_row();
                        }
                    }
                });
            });
        });
        
        // Pagination at bottom (fixed position)
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("psz").width(50.0).selected_text(format!("{}", state.page_size)).show_ui(ui, |ui| {
                for &ps in &[20usize,50,100] { if ui.selectable_value(&mut state.page_size, ps, format!("{}",ps)).clicked() { state.history_page = 1; } }
            });
            ui.label(RichText::new(format!("{}-{} / {}", start+1, end, total)).color(c.text2).size(10.0));
            if ui.small_button("«").clicked() { state.history_page = 1; }
            if ui.small_button("‹").clicked() && state.history_page > 1 { state.history_page -= 1; }
            let max_btns = 5usize;
            let mut sp = state.history_page.saturating_sub(max_btns/2).max(1);
            let mut ep = (sp + max_btns - 1).min(total_pages);
            if ep - sp < max_btns - 1 { sp = ep.saturating_sub(max_btns - 1).max(1); }
            for p in sp..=ep {
                let color = if p == state.history_page { c.blue } else { c.text2 };
                if ui.small_button(RichText::new(format!("{}",p)).color(color).size(10.0)).clicked() { state.history_page = p; }
            }
            if ui.small_button("›").clicked() && state.history_page < total_pages { state.history_page += 1; }
            if ui.small_button("»").clicked() { state.history_page = total_pages; }
        });
    });
}

fn hdr_cell(ui: &mut egui::Ui, c: &C, text: &str, w: f32, h: f32) {
    ui.allocate_ui_with_layout(vec2(w, h), Layout::left_to_right(Align::Center), |ui| {
        ui.label(RichText::new(text).color(c.text3).size(9.0).strong());
    });
}

fn row_cell(ui: &mut egui::Ui, c: &C, text: &str, w: f32, h: f32, color: Color32) {
    ui.allocate_ui_with_layout(vec2(w, h), Layout::left_to_right(Align::Center), |ui| {
        ui.label(RichText::new(text).color(color).size(10.0));
    });
}

#[allow(dead_code)]
fn row_click_cell<F: FnOnce()>(ui: &mut egui::Ui, _c: &C, text: &str, w: f32, h: f32, color: Color32, f: F) {
    ui.allocate_ui_with_layout(vec2(w, h), Layout::left_to_right(Align::Center), |ui| {
        if ui.add(egui::Label::new(RichText::new(text).color(color).size(10.0)).sense(Sense::click()))
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked() { f(); }
    });
}

// Detail Modal (popup overlay)
// ═══════════════════════════════════════════════════════════════

fn render_detail_modal(ctx: &Context, state: &mut UiState, metrics: &Metrics, snap: &Snapshot, lang: Lang, c: &C) {
    let meta = state.detail_idx.and_then(|idx| snap.history.get(idx));
    let Some(meta) = meta else { state.detail_idx = None; return; };

    let mut open = true;
    let idx = state.detail_idx.unwrap();
    egui::Window::new(t(lang,"detail_title"))
        .open(&mut open)
        .default_width(800.0)
        .default_height(600.0)
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            // ── Wrap entire content in surface frame for depth ──
            let surface_frame = Frame::NONE
                .fill(c.surface)
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(12, 10));
            surface_frame.show(ui, |ui| {
            // ── Meta info row ── use text for labels, blue for values ──
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}:", t(lang,"detail_model"))).color(c.text2).size(12.0));
                ui.label(RichText::new(&meta.model).color(c.text).size(12.0).strong());
                ui.separator();
                ui.label(RichText::new(format!("{}:", t(lang,"detail_time"))).color(c.text2).size(12.0));
                ui.label(RichText::new(&meta.time).color(c.text).size(12.0));
                ui.separator();
                ui.label(RichText::new(format!("{}:", t(lang,"detail_tokens"))).color(c.text2).size(12.0));
                ui.label(RichText::new(format!("{}/{}", meta.input_tokens, meta.output_tokens)).color(c.text).size(12.0).strong());
                ui.separator();
                ui.label(RichText::new(format!("{}:", t(lang,"detail_latency"))).color(c.text2).size(12.0));
                ui.label(RichText::new(format!("{:.1}s", meta.latency)).color(c.text).size(12.0));
            });
            ui.add_space(8.0);
            ui.separator();

            // ── Mode tabs with visual grouping ──
            let input_detail = metrics.get_input_detail(idx);
            let has_instructions = input_detail.as_ref().map_or(false, |d| !d.instructions.is_empty());
            let has_tools = input_detail.as_ref().map_or(false, |d| !d.tools.is_empty() && d.tools != "[]");
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if has_instructions { ui.selectable_value(&mut state.detail_mode, DetailMode::Instructions, t(lang,"detail_instructions")); }
                ui.selectable_value(&mut state.detail_mode, DetailMode::Messages, t(lang,"detail_messages"));
                ui.selectable_value(&mut state.detail_mode, DetailMode::Output, t(lang,"detail_output"));
                if has_tools { ui.selectable_value(&mut state.detail_mode, DetailMode::Tools, t(lang,"detail_tools")); }
            });
            ui.add_space(8.0);

            // ── Content in scroll area ──
            ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                ui.add_space(2.0);
                match state.detail_mode {
                    DetailMode::Instructions => {
                        let detail = metrics.get_input_detail(idx);
                        if let Some(d) = detail {
                            content_card(c, c.text).show(ui, |ui| {
                                ui.label(RichText::new(&d.instructions).color(c.text).size(13.0).monospace());
                            });
                        } else {
                            ui.label(RichText::new(t(lang,"empty_data")).color(c.text3));
                        }
                    }
                    DetailMode::Messages => {
                        let detail = metrics.get_input_detail(idx);
                        if let Some(d) = detail {
                            let role_colors: &[(&str, Color32)] = &[("user",c.blue), ("assistant",c.green), ("system",c.yellow), ("tool",c.purple)];
                            for (i, msg) in d.messages.iter().enumerate() {
                                let color = role_colors.iter().find(|(r,_)| *r == msg.role.as_str()).map(|&(_,clr)| clr).unwrap_or(c.text2);
                                // Role badge strip at top
                                let role_bg = color.linear_multiply(0.12);
                                Frame::NONE
                                    .fill(c.card)
                                    .stroke(Stroke::new(1.0, color.linear_multiply(0.4)))
                                    .corner_radius(CornerRadius::same(6))
                                    .inner_margin(Margin::symmetric(12, 10))
                                    .show(ui, |ui| {
                                        // Colored role badge + number
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(format!("#{}", i+1)).color(c.text3).size(11.0).monospace());
                                            Frame::NONE
                                                .fill(role_bg)
                                                .corner_radius(CornerRadius::same(3))
                                                .inner_margin(Margin::symmetric(8, 3))
                                                .show(ui, |ui| {
                                                    ui.label(RichText::new(msg.role.to_uppercase()).color(color).size(12.0).strong());
                                                });
                                        });
                                        ui.add_space(6.0);
                                        // Content
                                        ui.label(RichText::new(&msg.content).color(c.text).size(12.5).monospace());
                                    });
                                ui.add_space(6.0);
                            }
                            if d.messages.is_empty() { ui.label(RichText::new(t(lang,"empty_data")).color(c.text3)); }
                        } else {
                            ui.label(RichText::new(t(lang,"empty_data")).color(c.text3));
                        }
                    }
                    DetailMode::Tools => {
                        let detail = metrics.get_input_detail(idx);
                        if let Some(d) = detail {
                            if d.tools.is_empty() || d.tools == "[]" {
                                ui.label(RichText::new(t(lang,"no_tools")).color(c.text3));
                            } else {
                                let tools_str = serde_json::from_str::<serde_json::Value>(&d.tools).map(|v| serde_json::to_string_pretty(&v).unwrap_or_default()).unwrap_or(d.tools);
                                content_card(c, c.text).show(ui, |ui| {
                                    ui.label(RichText::new(&tools_str).color(c.text).size(13.0).monospace());
                                });
                            }
                        } else {
                            ui.label(RichText::new(t(lang,"no_tools")).color(c.text3));
                        }
                    }
                    DetailMode::Output => {
                        let content_str = metrics.get_full_content(idx).unwrap_or_default();
                        if content_str.is_empty() {
                            content_card(c, c.text).show(ui, |ui| {
                                ui.label(RichText::new(&meta.preview).color(c.text).size(13.0));
                            });
                        } else {
                            content_card(c, c.text).show(ui, |ui| {
                                ui.label(RichText::new(&content_str).color(c.text).size(13.0));
                            });
                        }
                    }
                }
            });
        });
    });
    
    // Close modal when window is closed
    if !open {
        state.detail_idx = None;
    }
}

// ═══════════════════════════════════════════════════════════════
// Config tab
// ═══════════════════════════════════════════════════════════════

fn render_config(ui: &mut egui::Ui, state: &mut UiState, cm: &ConfigManager, ss: &SecureConfigStore, app_state: &Arc<AppState>, lang: Lang, c: &C, viewport_h: f32) {
    if !state.config_loaded {
        state.current_config = cm.get_current_model();
        state.saved_configs = ss.list_configs();
        state.config_loaded = true;
    }

    // Auto-dismiss toast after 3 seconds
    if state.toast_msg.is_some() {
        let now = ui.ctx().input(|i| i.time);
        if state.toast_time > 0.0 && now - state.toast_time > 3.0 {
            state.toast_msg = None;
        }
    }

    ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
        // ── Quick Config ──
        card_frame(c).show(ui, |ui| {
            section_label(ui, c, t(lang,"config_quick"));
            
            // Row 0: Provider preset selector
            ui.horizontal(|ui| {
                ui.label(RichText::new(t(lang,"config_preset")).color(c.text2).size(11.0));
                egui::ComboBox::from_id_salt("provider_preset")
                    .selected_text(if state.selected_preset == "custom" {
                        t(lang,"config_preset_custom").to_string()
                    } else {
                        state.presets.iter()
                            .find(|p| p.name == state.selected_preset)
                            .map(|p| p.display_name.to_string())
                            .unwrap_or_else(|| t(lang,"config_preset_custom").to_string())
                    })
                    .show_ui(ui, |ui| {
                        for preset in &state.presets {
                            ui.selectable_value(&mut state.selected_preset, preset.name.to_string(), preset.display_name);
                        }
                        ui.selectable_value(&mut state.selected_preset, "custom".to_string(), t(lang,"config_preset_custom"));
                    });
                
                // Auto-fill when preset changes
                if state.selected_preset != "custom" {
                    if let Some(preset) = state.presets.iter().find(|p| p.name == state.selected_preset) {
                        if state.config_base_url.is_empty() || state.config_base_url == preset.base_url {
                            state.config_base_url = preset.base_url.to_string();
                            state.config_model = preset.default_model.to_string();
                        }
                    }
                }
            });
            
            // Row 1: name + model
            ui.horizontal(|ui| {
                ui.label(RichText::new(t(lang,"config_name")).color(c.text2).size(11.0));
                let remaining = ui.available_size_before_wrap();
                ui.add(egui::TextEdit::singleline(&mut state.config_name).hint_text(t(lang,"config_name_ph")).desired_width(remaining.x / 2.0));
                ui.label(RichText::new(t(lang,"config_model")).color(c.text2).size(11.0));
                let remaining2 = ui.available_size_before_wrap();
                ui.add(egui::TextEdit::singleline(&mut state.config_model).hint_text("gpt-4o").desired_width(remaining2.x));
            });
            // Row 2: base_url
            ui.horizontal(|ui| {
                ui.label(RichText::new(t(lang,"config_base_url")).color(c.text2).size(11.0));
                let remaining = ui.available_size_before_wrap();
                ui.add(egui::TextEdit::singleline(&mut state.config_base_url).hint_text("https://api.openai.com/v1").desired_width(remaining.x));
            });
            // Row 3: api_key
            ui.horizontal(|ui| {
                ui.label(RichText::new(t(lang,"config_api_key")).color(c.text2).size(11.0));
                let remaining = ui.available_size_before_wrap();
                ui.add(egui::TextEdit::singleline(&mut state.config_api_key).password(true).hint_text("sk-...").desired_width(remaining.x));
            });
            ui.add_space(4.0);
            
            // Button row: Apply + Reload + Test Connection
            ui.horizontal(|ui| {
                if ui.button(RichText::new(t(lang,"btn_apply")).color(c.green).size(12.0)).clicked() {
                    if !state.config_base_url.is_empty() && !state.config_model.is_empty() {
                        cm.apply_model(&state.config_model, PROXY_PROVIDER, PROXY_BASE_URL, &state.config_api_key);
                        let upstream = format!("{}/chat/completions", state.config_base_url.trim_end_matches('/'));
                        app_state.set_upstream(upstream, state.config_api_key.clone());
                        app_state.set_upstream_model(state.config_model.clone());
                        let toast_key = if !state.config_name.is_empty() && !state.config_api_key.is_empty() {
                            ss.save_config(&state.config_name, &state.config_model, "Custom", &state.config_base_url, &state.config_api_key);
                            t(lang,"toast_save_apply_ok")
                        } else {
                            t(lang,"toast_apply_ok")
                        };
                        state.current_config = cm.get_current_model();
                        state.saved_configs = ss.list_configs();
                        state.toast_msg = Some(toast_key.to_string());
                        state.toast_time = ui.ctx().input(|i| i.time);
                    }
                }
                if ui.button(RichText::new(t(lang,"btn_reload")).color(c.text2).size(12.0)).clicked() {
                    state.current_config = cm.get_current_model();
                    state.saved_configs = ss.list_configs();
                }
                
                // Test Connection button
                ui.separator();
                let test_btn_text = if state.connectivity_testing {
                    t(lang,"btn_testing")
                } else {
                    t(lang,"btn_test")
                };
                let test_btn = ui.add_enabled(
                    !state.connectivity_testing && !state.config_base_url.is_empty() && !state.config_api_key.is_empty(),
                    egui::Button::new(RichText::new(test_btn_text).color(c.blue).size(12.0))
                );
                if test_btn.clicked() {
                    state.connectivity_testing = true;
                    state.connectivity_result = None;
                    // Spawn async task for connectivity test
                    let base_url = state.config_base_url.clone();
                    let api_key = state.config_api_key.clone();
                    let app_state_clone = app_state.clone();
                    let ctx = ui.ctx().clone();
                    std::thread::spawn(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        let result = rt.block_on(connectivity::test_connectivity(&app_state_clone.http_client, &base_url, &api_key));
                        // Store result in AppState
                        *app_state_clone.connectivity_result.write().unwrap() = Some(result);
                        ctx.request_repaint();
                    });
                }
            });
            
            // Check for connectivity test result from AppState
            if let Some(result) = app_state.connectivity_result.read().unwrap().clone() {
                state.connectivity_testing = false;
                state.connectivity_result = Some(result.clone());
                // Clear the result from AppState so we don't re-read it
                *app_state.connectivity_result.write().unwrap() = None;
            }
            
            // Show connectivity test result
            if let Some(ref result) = state.connectivity_result {
                ui.add_space(4.0);
                if result.success {
                    ui.label(RichText::new(format!("✓ {} ({}ms)", t(lang,"test_success"), result.latency_ms)).color(c.green).size(11.0));
                    if !result.models.is_empty() {
                        ui.label(RichText::new(format!("{}: {}", t(lang,"test_models"), result.models.join(", "))).color(c.text2).size(10.0));
                    }
                } else {
                    ui.label(RichText::new(format!("✗ {}: {}", t(lang,"test_failed"), result.error_message.as_deref().unwrap_or("Unknown error"))).color(c.red).size(11.0));
                }
            }
            
            ui.add_space(4.0);
            ui.label(RichText::new(format!("{}: {} {} {} @ {}", t(lang,"current"), state.current_config.model, t(lang,"via"), state.current_config.provider, state.current_config.base_url)).color(c.green).size(10.0));
        });

        ui.add_space(8.0);

        // ── Saved configs ──
        card_frame(c).show(ui, |ui| {
            section_label(ui, c, t(lang,"config_saved"));
            if state.saved_configs.is_empty() {
                ui.label(RichText::new(t(lang,"empty_config")).color(c.text3));
            } else {
                let configs = state.saved_configs.clone();
                for cfg in &configs {
                    let is_active = cfg.provider == state.current_config.provider && cfg.model == state.current_config.model;
                    let (fill, stroke_color, badge) = if is_active {
                        (c.green.linear_multiply(0.18), c.green, t(lang,"badge_active"))
                    } else {
                        (c.elev, c.border, "")
                    };
                    let stroke_w = if is_active { 2.0 } else { 1.0 };
                    Frame::NONE
                        .fill(fill)
                        .stroke(Stroke::new(stroke_w, stroke_color))
                        .corner_radius(CornerRadius::same(4))
                        .inner_margin(Margin::same(6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Left accent bar for active config
                                if is_active {
                                    let bar_rect = ui.available_rect_before_wrap();
                                    let bar = bar_rect.with_max_x(bar_rect.min.x + 3.0);
                                    ui.painter().rect_filled(bar, 2.0, c.green);
                                    ui.add_space(4.0);
                                }
                                ui.label(RichText::new(&cfg.name).color(if is_active { c.green } else { c.text }).size(11.0).strong());
                                ui.label(RichText::new(&cfg.model).color(if is_active { c.green.linear_multiply(0.8) } else { c.text2 }).size(10.0).monospace());
                                ui.label(RichText::new(&cfg.api_key_masked).color(c.text3).size(9.0).monospace());
                                if is_active {
                                    ui.label(RichText::new(badge).color(c.green).size(9.0).strong());
                                }
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui.small_button(RichText::new(t(lang,"btn_apply")).color(if is_active { c.text3 } else { c.green }).size(10.0)).clicked() {
                                        // Load full (decrypted) config and apply
                                        if let Some(full) = ss.get_config_full(&cfg.name) {
                                            cm.apply_model(&full.model, PROXY_PROVIDER, PROXY_BASE_URL, &full.api_key);
                                            // Update proxy upstream dynamically from saved config
                                            let upstream = format!("{}/chat/completions", full.base_url.trim_end_matches('/'));
                                            app_state.set_upstream(upstream, full.api_key);
                                            app_state.set_upstream_model(full.model.clone());
                                            state.current_config = cm.get_current_model();
                                            state.toast_msg = Some(t(lang,"toast_apply_ok").to_string());
                                            state.toast_time = ui.ctx().input(|i| i.time);
                                        }
                                    }
                                    if ui.small_button(RichText::new(t(lang,"btn_edit")).color(c.blue).size(10.0)).clicked() {
                                        // Backfill form with this config's values
                                        if let Some(full) = ss.get_config_full(&cfg.name) {
                                            state.config_name = cfg.name.clone();
                                            state.config_model = full.model.clone();
                                            state.config_base_url = full.base_url.clone();
                                            state.config_api_key = full.api_key.clone();
                                            state.selected_preset = "custom".to_string();
                                        }
                                    }
                                    if ui.small_button(RichText::new(t(lang,"btn_delete")).color(c.red).size(10.0)).clicked() {
                                        state.delete_confirm_name = Some(cfg.name.clone());
                                    }
                                });
                            });
                        });
                    ui.add_space(2.0);
                }
            }
        });

        ui.add_space(8.0);

        // ── Delete Confirmation Dialog ──
        if let Some(ref name) = state.delete_confirm_name {
            let name_clone = name.clone();
            egui::Window::new(t(lang,"delete_confirm_title"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    ui.label(RichText::new(t(lang,"delete_confirm_msg").replace("{}", &name_clone)).size(12.0));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new(t(lang,"btn_confirm_delete")).color(c.red).size(11.0)).clicked() {
                            ss.delete_config(&name_clone);
                            state.saved_configs = ss.list_configs();
                            state.delete_confirm_name = None;
                        }
                        if ui.button(RichText::new(t(lang,"btn_cancel")).color(c.text2).size(11.0)).clicked() {
                            state.delete_confirm_name = None;
                        }
                    });
                });
        }

        // ── Proxy Status ──
        card_frame(c).show(ui, |ui| {
            ui.set_max_width(ui.available_width());
            section_label(ui, c, t(lang,"config_status"));

            let upstream_url = app_state.get_upstream_url();
            let upstream_model = app_state.get_upstream_model();

            // Local proxy endpoint (what Codex connects to)
            ui.add_space(2.0);
            ui.label(RichText::new("← Codex →").color(c.green).size(10.0).strong());
            ui.horizontal(|ui| {
                let tw = ui.available_width();
                let lw = (tw * 0.35).min(140.0);
                ui.set_min_width(lw);
                ui.label(RichText::new(t(lang,"config_model")).color(c.text2).size(11.0));
                ui.label(RichText::new(format!("：{}", state.current_config.model)).color(c.text).size(11.0).strong());
            });
            ui.horizontal(|ui| {
                let tw = ui.available_width();
                let lw = (tw * 0.35).min(140.0);
                ui.set_min_width(lw);
                ui.label(RichText::new(t(lang,"config_provider")).color(c.text2).size(11.0));
                ui.label(RichText::new(format!("：{}", state.current_config.provider)).color(c.text).size(11.0));
            });
            ui.horizontal(|ui| {
                let tw = ui.available_width();
                let lw = (tw * 0.35).min(140.0);
                ui.set_min_width(lw);
                ui.label(RichText::new(t(lang,"config_base_url")).color(c.text2).size(11.0));
                ui.label(RichText::new(format!("：{}", state.current_config.base_url)).color(c.text).size(11.0));
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);

            // Upstream forwarding (proxy → actual API)
            ui.label(RichText::new("→ 上游 →").color(c.blue).size(10.0).strong());
            ui.horizontal(|ui| {
                let tw = ui.available_width();
                let lw = (tw * 0.35).min(140.0);
                ui.set_min_width(lw);
                ui.label(RichText::new("Upstream URL").color(c.text2).size(11.0));
                ui.label(RichText::new(format!("：{}", upstream_url)).color(c.green).size(11.0));
            });
            ui.horizontal(|ui| {
                let tw = ui.available_width();
                let lw = (tw * 0.35).min(140.0);
                ui.set_min_width(lw);
                ui.label(RichText::new("Upstream Model").color(c.text2).size(11.0));
                ui.label(RichText::new(format!("：{}", upstream_model)).color(c.green).size(11.0).strong());
            });

            ui.add_space(6.0);
            ui.label(RichText::new(cm.config_path_display()).color(c.text3).size(9.0).monospace());
        });
    }); // close ScrollArea

    // ── Bottom toast notification (overlay at panel bottom) ──
    if let Some(ref msg) = state.toast_msg {
        let visible = ui.clip_rect();
        let toast_w = 300.0;
        let toast_h = 36.0;
        let toast_pos = egui::pos2(
            visible.center().x - toast_w / 2.0,
            visible.max.y - toast_h - 8.0,
        );
        let toast_rect = egui::Rect::from_min_size(toast_pos, egui::vec2(toast_w, toast_h));
        let _response = ui.allocate_rect(toast_rect, egui::Sense::hover());
        egui::Area::new(egui::Id::new("panel_bottom_toast"))
            .fixed_pos(toast_pos)
            .show(ui.ctx(), |ui| {
                Frame::NONE
                    .fill(c.green.linear_multiply(0.2))
                    .stroke(Stroke::new(1.5, c.green))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_min_width(toast_w - 20.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("[OK]").color(c.green).size(14.0));
                            ui.label(RichText::new(msg).color(c.green).size(12.0).strong());
                        });
                    });
            });
    }
}

