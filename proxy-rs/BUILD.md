# 构建与运行指南

本项目提供 `Makefile` 统一管理编译、运行、停止等操作，支持 macOS、Linux、Windows 跨平台。

## 前置要求

- **Rust** >= 1.70（推荐最新 stable）
- **系统依赖**（egui/eframe 需要）：
  - **macOS**：Xcode Command Line Tools
  - **Linux**：`libgtk-3-dev`, `libxcb-*` 等（参考 [eframe 文档](https://github.com/emilk/egui/tree/master/crates/eframe)）
  - **Windows**：Visual Studio Build Tools

## 命令一览

| 命令 | 说明 | 适用场景 |
|------|------|----------|
| `make build` | Debug 编译 | 开发调试 |
| `make release` | Release 编译（LTO + opt-level 3） | 生产发布 |
| `make build-linux` | 交叉编译 Linux x86_64 | 跨平台发布 |
| `make build-macos` | 交叉编译 macOS x86_64 + arm64 | 跨平台发布 |
| `make build-windows` | 交叉编译 Windows x86_64（需 mingw-w64） | 跨平台发布 |
| `make run` | Debug 模式运行 | 开发调试 |
| `make run-release` | Release 模式运行 | 性能测试 |
| `make stop` | 停止运行中的程序（自动识别 OS） | 停止服务 |
| `make status` | 查看程序运行状态 | 状态检查 |
| `make clean` | 清除所有编译产物 | 清理环境 |
| `make help` | 查看所有可用命令 | 帮助 |

```bash
make help          # 终端快捷查看
```

---

## 编译

### Debug 编译

```bash
make build
```

等效于 `cargo build`，编译产物位于 `target/debug/proxy-rs`。

### Release 编译

```bash
make release
```

等效于 `cargo build --release`，启用 LTO + opt-level 3 优化，编译产物位于 `target/release/proxy-rs`。

---

## 跨平台编译

> 跨平台编译需要先安装对应的 Rust target，Makefile 会自动尝试添加，但需要系统已安装对应的链接器。

### 编译 Linux x86_64

```bash
make build-linux
```

产物：`target/x86_64-unknown-linux-gnu/release/proxy-rs`

> 在 macOS 上交叉编译需要安装 `FiloSottile/musl-cross` 或 `osxcross` 等工具链。

### 编译 macOS（双架构）

```bash
make build-macos
```

同时编译 x86_64 和 arm64 两个版本：
- `target/x86_64-apple-darwin/release/proxy-rs`
- `target/aarch64-apple-darwin/release/proxy-rs`

> 仅在 macOS 上可用。

### 编译 Windows x86_64

**macOS 交叉编译：**

```bash
make build-windows
# 或直接运行
cargo build --target x86_64-pc-windows-gnu --release
```

产物：`target/x86_64-pc-windows-gnu/release/proxy-rs.exe`

前置要求：
```bash
# 安装 mingw-w64 工具链
brew install mingw-w64

# 添加 Rust 目标平台
rustup target add x86_64-pc-windows-gnu
```

**Windows 原生编译：**

```bash
cargo build --release
```

产物：`target/release/proxy-rs.exe`

> 前置要求：安装 Rust 和 Visual Studio Build Tools。

---

## 运行

### Debug 模式运行

```bash
make run
```

### Release 模式运行

```bash
make run-release
```

程序启动后：
- 代理服务监听 `http://127.0.0.1:8000`
- 自动维护 `~/.codex/config.toml` 指向代理
- 打开 egui 桌面窗口显示监控仪表盘

---

## 停止

```bash
make stop
```

自动检测操作系统并终止占用 8000 端口的进程：
- **macOS**：使用 `lsof` 查找并 `kill`
- **Linux**：使用 `fuser` 查找并 `kill`
- **Windows**：使用 `netstat` + `taskkill`

---

## 查看状态

```bash
make status
```

检查 8000 端口是否有进程在监听。

---

## 清理

```bash
make clean
```

等效于 `cargo clean`，清除所有编译产物。

---

## 自定义端口

如需修改代理端口，编辑 `Makefile` 顶部的 `PORT` 变量：

```makefile
PORT := 8000    # 改为你需要的端口
```

同时需要修改 `src/ui.rs` 中的 `PROXY_BASE_URL` 常量保持一致。

---

## 日志级别

通过环境变量 `RUST_LOG` 控制日志输出：

```bash
# 默认 info 级别
make run

# 调试模式，显示详细请求/响应
RUST_LOG=debug make run

# 仅显示警告和错误
RUST_LOG=warn make run
```
