# Codex Proxy Monitor

Codex Proxy Monitor 是一个基于 Rust + egui 构建的桌面端代理服务监控工具，用于拦截、转发和监控 Codex CLI 的 API 请求。

## 功能特性

- **代理转发** — 拦截 Codex CLI 请求，转发至用户配置的上游 API（OpenAI、DeepSeek 等）
- **实时监控仪表盘** — 请求总量、成功率、失败数、平均延迟、活跃流、Token 用量等核心指标
- **吞吐量趋势图** — 实时展示每分钟请求量变化
- **请求历史记录** — 完整的请求/响应结构化日志，支持详情查看（Markdown/源码切换）
- **配置中心** — 多模型配置管理，支持动态切换上游 API，API Key 加密存储
- **Codex 配置自动管理** — 自动维护 `~/.codex/config.toml`，确保请求始终经由代理
- **跨平台中文字体** — 自动加载系统字体（macOS PingFang / Linux Noto CJK / Windows 微软雅黑）
- **响应式 UI 布局** — 所有控件自适应窗口尺寸，支持暗色/亮色主题切换
- **国际化** — 支持中文/英文界面切换

## 技术架构

```
┌─────────────────────────────────────────────┐
│              Codex Proxy Monitor            │
├─────────────────────────────────────────────┤
│  UI Layer (egui + eframe)                   │
│  ├── Dashboard   实时指标 + 图表             │
│  ├── Config      配置管理 + 加密存储          │
│  └── Detail      请求详情弹窗                │
├─────────────────────────────────────────────┤
│  Proxy Server (axum + tokio)                │
│  ├── POST /v1/chat/completions  请求拦截转发 │
│  ├── GET  /sse/metrics          指标推送     │
│  └── SSE 实时推送                 自动重连    │
├─────────────────────────────────────────────┤
│  Core                                       │
│  ├── Metrics     指标采集 + 历史记录          │
│  ├── Config      TOML 读写 + SQLite 加密存储  │
│  └── Convert     请求/响应格式转换            │
└─────────────────────────────────────────────┘
```

## 模块说明

| 模块 | 文件 | 职责 |
|------|------|------|
| main | `src/main.rs` | 应用入口、窗口初始化、中文字体加载 |
| proxy | `src/proxy.rs` | HTTP 代理服务、请求转发、SSE 流处理 |
| ui | `src/ui.rs` | egui 界面渲染、响应式布局 |
| metrics | `src/metrics.rs` | 指标采集、历史队列、吞吐量统计 |
| config | `src/config.rs` | Codex 配置管理、SQLite 加密存储 |
| convert | `src/convert.rs` | 请求/响应格式转换、模型名映射 |

## 数据文件

| 路径 | 说明 |
|------|------|
| `~/.codex/config.toml` | Codex 原生配置文件（自动维护） |
| `~/Library/Application Support/codex-proxy/proxy_config.db` | 加密配置数据库 (macOS) |
| `~/.config/codex-proxy/proxy_config.db` | 加密配置数据库 (Linux) |
| `%APPDATA%\codex-proxy\proxy_config.db` | 加密配置数据库 (Windows) |

## 依赖

| 类别 | 库 |
|------|------|
| GUI | eframe 0.29, egui 0.29, egui_plot 0.29 |
| 异步运行时 | tokio |
| HTTP 服务 | axum 0.7, tower, tower-http |
| HTTP 客户端 | reqwest |
| 数据库 | rusqlite (bundled) |
| 加密 | fernet |
| 配置 | toml |
| 日志 | tracing, tracing-subscriber |

## 快速开始

```bash
# 编译运行
make run

# Release 模式
make run-release

# 查看可用命令
make help
```

详细构建命令参见 [BUILD.md](BUILD.md)。

## 许可证

MIT
