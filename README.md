# ProxyTauri

一款基于 Tauri 构建的轻量级桌面应用，用于将 Codex CLI 等 AI 客户端的请求代理转发至任意兼容 OpenAI Chat Completions API 的 LLM 服务，同时提供实时监控、会话预览与配置管理功能。

## 功能特性

### 协议转换代理

- **Responses API → Chat Completions API**：将 Codex 使用的 OpenAI Responses API 格式自动转换为上游 LLM 服务支持的 Chat Completions 格式
- **流式响应支持**：完整支持 SSE 流式传输，包括流式与非流式上游的自动适配
- **工具调用透传**：自动转换工具定义格式，支持 function call 的流式累积与转发
- **多厂商适配**：内置模型厂商识别逻辑，针对不同 LLM 提供商（Qwen、DeepSeek 等）自动调整请求参数

### 实时监控仪表盘

- **请求统计卡片**：总请求数、成功数、失败数、平均延迟、活跃流数、Token 总量
- **吞吐量图表**：实时请求吞吐量曲线（Area Chart）
- **延迟历史图表**：请求延迟变化趋势（Line Chart）
- **模型使用统计**：各模型调用次数柱状图
- **Token 分布图**：输入/输出 Token 用量对比

### 会话实时预览

- **Markdown 渲染**：使用 `react-markdown` + `remark-gfm` 实时渲染 LLM 返回内容，支持表格、代码块、列表等
- **工具调用可视化**：在会话面板中显示工具调用名称与图标
- **任务进度跟踪**：自动解析 Codex 输出的任务列表（`- [ ]` / `- [x]`），显示进度条与完成百分比
- **时间戳标记**：每条消息和工具调用前显示 `[HH:MM:SS]` 时间标记
- **内容累积**：跨请求保留会话内容，支持手动清空
- **性能优化**：100ms 节流渲染 + 4000 字符截断，避免长文本渲染卡顿

### 配置管理

- **多配置预设**：保存多组上游服务配置（URL、模型、API Key），快速切换
- **API Key 加密存储**：使用 Fernet 对称加密，API Key 安全存储在 SQLite 数据库中
- **连通性测试**：一键测试上游服务连通性，返回可用模型列表与延迟
- **Codex 自动配置**：自动将本地代理地址写入 `~/.codex/config.toml`，无需手动配置

### 系统托盘

- **后台运行**：关闭窗口后最小化到系统托盘，代理持续运行
- **状态指示**：托盘图标实时反映代理运行状态
- **跨平台支持**：支持 macOS、Windows、Linux

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | Tauri 2.x |
| 前端 | React 19 + TypeScript + Tailwind CSS |
| 图表 | Recharts |
| Markdown | react-markdown + remark-gfm |
| 后端代理 | Rust + axum + reqwest |
| 数据存储 | SQLite (rusqlite) |
| 加密 | Fernet |
| 异步运行时 | Tokio |

## 快速开始

### 环境要求

- Rust 1.70+
- Node.js 18+
- npm 9+

### 开发模式

```bash
cd proxy-tauri
npm install
npm run tauri dev
```

### 构建发布版本

```bash
cd proxy-tauri
npm run tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`。

## 项目结构

```
proxy-tauri/
├── src/                    # React 前端
│   ├── pages/              # 页面组件
│   │   ├── Dashboard.tsx   # 仪表盘（统计、图表、会话预览）
│   │   ├── Config.tsx      # 配置中心
│   │   └── History.tsx     # 请求历史
│   ├── contexts/           # React Context
│   ├── lib/                # 工具函数、类型、国际化
│   └── components/         # 通用组件
├── src-tauri/              # Rust 后端
│   ├── src/
│   │   ├── lib.rs          # Tauri 入口与命令注册
│   │   ├── proxy.rs        # 代理服务器核心逻辑
│   │   ├── metrics.rs      # 指标统计与会话管理
│   │   ├── convert.rs      # 协议转换（Responses → Chat）
│   │   ├── config.rs       # 配置管理（Codex 集成）
│   │   ├── model.rs        # 模型厂商识别与参数适配
│   │   └── sse.rs          # SSE 事件构造
│   └── Cargo.toml
└── package.json
```

## 许可证

MIT
