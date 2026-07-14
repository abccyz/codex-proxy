APP_NAME := proxy-tauri
PORT := 8000
CARGO_DIR := src-tauri
UNAME := $(shell uname -s)
RELEASE_DIR := release
WIN_VERSION := $(shell grep version $(CARGO_DIR)/tauri.conf.json | head -1 | grep -o '[0-9]\+\.[0-9]\+\.[0-9]\+')

# ── Build ───────────────────────────────────────────────────────
.PHONY: build release clean

build:
	cd $(CARGO_DIR) && cargo build

release:
	cd $(CARGO_DIR) && cargo build --release

clean:
	cd $(CARGO_DIR) && cargo clean
	rm -rf node_modules dist $(RELEASE_DIR)

release-clean:
	rm -rf $(RELEASE_DIR)
	@echo "Cleaned $(RELEASE_DIR)/"

# ── Dev ─────────────────────────────────────────────────────────
.PHONY: dev dev-fe dev-be

dev: dev-be dev-fe

dev-be:
	cd $(CARGO_DIR) && cargo run

dev-fe:
	npm run dev

tauri-dev:
	npm run tauri dev

tauri-build:
	npm run tauri build

# ── Platform Bundle Build ────────────────────────────────────────
.PHONY: bundle-macos bundle-macos-arm bundle-macos-x64 bundle-windows bundle-linux bundle-linux-deb bundle-all release-clean

# macOS: .dmg + .app (both architectures)
bundle-macos: bundle-macos-arm bundle-macos-x64
	@echo "Both macOS bundles built."

# macOS ARM (Apple Silicon): .dmg + .app
bundle-macos-arm:
	@echo "Building macOS ARM bundle (Apple Silicon)..."
	npx tauri build --target aarch64-apple-darwin --bundles dmg app
	@mkdir -p $(RELEASE_DIR)/macos-arm64
	@cp $(CARGO_DIR)/target/aarch64-apple-darwin/release/bundle/dmg/ProxyTauri_$(WIN_VERSION)_aarch64.dmg $(RELEASE_DIR)/macos-arm64/
	@cp -R $(CARGO_DIR)/target/aarch64-apple-darwin/release/bundle/macos/ProxyTauri.app $(RELEASE_DIR)/macos-arm64/
	@echo "macOS ARM bundle ready: $(RELEASE_DIR)/macos-arm64/"

# macOS Intel (x86_64): .dmg + .app
bundle-macos-x64:
	@echo "Building macOS Intel bundle (x86_64)..."
	npx tauri build --target x86_64-apple-darwin --bundles dmg app
	@mkdir -p $(RELEASE_DIR)/macos-x64
	@cp $(CARGO_DIR)/target/x86_64-apple-darwin/release/bundle/dmg/ProxyTauri_$(WIN_VERSION)_x64.dmg $(RELEASE_DIR)/macos-x64/
	@cp -R $(CARGO_DIR)/target/x86_64-apple-darwin/release/bundle/macos/ProxyTauri.app $(RELEASE_DIR)/macos-x64/
	@echo "macOS Intel bundle ready: $(RELEASE_DIR)/macos-x64/"

# Windows: .exe + WebView2Loader.dll (cross-compile from macOS via tauri build)
# NSIS installer (makensis.exe) is only available on Windows hosts, so on macOS
# we build the exe via `tauri build` then manually zip it with WebView2Loader.dll.
bundle-windows:
ifeq ($(UNAME),Darwin)
	@echo "Cross-compiling Windows exe from macOS..."
	@npx tauri build --target x86_64-pc-windows-gnu
	@echo "Packaging Windows zip..."
	@mkdir -p $(RELEASE_DIR)/windows/_staging
	@cp $(CARGO_DIR)/target/x86_64-pc-windows-gnu/release/proxy-tauri.exe $(RELEASE_DIR)/windows/_staging/
	@cp dist-windows/WebView2Loader.dll $(RELEASE_DIR)/windows/_staging/
	@cd $(RELEASE_DIR)/windows && rm -f proxy-tauri_$(WIN_VERSION)_x64.zip && zip -r proxy-tauri_$(WIN_VERSION)_x64.zip _staging/
	@rm -rf $(RELEASE_DIR)/windows/_staging
	@echo "Windows bundle ready: $(RELEASE_DIR)/windows/proxy-tauri_$(WIN_VERSION)_x64.zip"
else
	@echo "Building Windows bundle (.msi + .nsis)..."
	npm run tauri build -- --bundles msi nsis
	@echo "Windows bundle built: $(CARGO_DIR)/target/release/bundle/"
endif

# Linux: .deb (cross-compile from macOS via cargo-zigbuild)
DEB_NAME := proxy-tauri_$(shell grep version $(CARGO_DIR)/tauri.conf.json | head -1 | grep -o '[0-9]\+\.[0-9]\+\.[0-9]\+')_amd64.deb
bundle-linux: bundle-linux-deb

# Linux .deb (cross-compile from macOS / build natively on Linux)
# On macOS: uses `tauri build --runner cargo-zigbuild` so Tauri's frontend-asset
# embedding (tauri-codegen-assets) still runs, while zig cc handles cross-linking.
# `--allow-shlib-undefined` lets zig ignore missing GLIBC_2.34 symbols in Ubuntu
# 24.04 sysroot .so files (they resolve at runtime on the target system).
# Tauri refuses to bundle deb on macOS host, so we run its `_package-deb` step manually.
bundle-linux-deb:
ifeq ($(UNAME),Darwin)
	@echo "=== Cross-compiling Linux .deb from macOS via tauri + cargo-zigbuild ==="
	@SYSROOT="$(shell pwd)/linux-sysroot"; \
	export PKG_CONFIG_SYSROOT_DIR="$$SYSROOT"; \
	export PKG_CONFIG_PATH="$$SYSROOT/usr/lib/x86_64-linux-gnu/pkgconfig:$$SYSROOT/usr/lib/pkgconfig:$$SYSROOT/usr/share/pkgconfig"; \
	export PKG_CONFIG_ALLOW_CROSS=1; \
	export RUSTFLAGS="-C link-arg=-Wl,--allow-shlib-undefined"; \
	npx tauri build --runner cargo-zigbuild --target x86_64-unknown-linux-gnu
	$(MAKE) _package-deb
	@mkdir -p $(RELEASE_DIR)/linux
	@cp target/$(DEB_NAME) $(RELEASE_DIR)/linux/
	@echo "Linux .deb ready: $(RELEASE_DIR)/linux/$(DEB_NAME)"
else
	@echo "Building Linux .deb natively..."
	npm run tauri build -- --bundles deb
	@echo "Linux .deb built: $(CARGO_DIR)/target/release/bundle/"
endif

# Internal: package the already-built binary into .deb
_package-deb:
	@rm -rf target/deb-build
	@VERSION=$$(grep version $(CARGO_DIR)/tauri.conf.json | head -1 | grep -o '[0-9]\+\.[0-9]\+\.[0-9]\+'); \
	DEB_DIR="target/deb-build/proxy-tauri_$${VERSION}_amd64"; \
	mkdir -p "$${DEB_DIR}/DEBIAN" "$${DEB_DIR}/usr/bin" "$${DEB_DIR}/usr/lib" \
		"$${DEB_DIR}/usr/share/applications" \
		"$${DEB_DIR}/usr/share/icons/hicolor/128x128/apps" \
		"$${DEB_DIR}/usr/share/icons/hicolor/32x32/apps"; \
	cp $(CARGO_DIR)/target/x86_64-unknown-linux-gnu/release/proxy-tauri "$${DEB_DIR}/usr/bin/"; \
	cp $(CARGO_DIR)/target/x86_64-unknown-linux-gnu/release/deps/libproxy_tauri_lib.so "$${DEB_DIR}/usr/lib/"; \
	cp $(CARGO_DIR)/icons/128x128.png "$${DEB_DIR}/usr/share/icons/hicolor/128x128/apps/proxy-tauri.png"; \
	cp $(CARGO_DIR)/icons/32x32.png "$${DEB_DIR}/usr/share/icons/hicolor/32x32/apps/proxy-tauri.png"; \
	printf '[Desktop Entry]\nType=Application\nName=ProxyTauri\nComment=Codex Proxy Monitor\nExec=/usr/bin/proxy-tauri\nIcon=proxy-tauri\nCategories=Development;Network;\nTerminal=false\n' > "$${DEB_DIR}/usr/share/applications/proxy-tauri.desktop"; \
	printf 'Package: proxy-tauri\nVersion: %s\nArchitecture: amd64\nMaintainer: Codex <dev@codex.com>\nDescription: Codex Proxy Monitor\n A Tauri-based desktop application for monitoring and managing Codex CLI proxy connections.\nDepends: libwebkit2gtk-4.1-0, libgtk-3-0, libayatana-appindicator3-1, librsvg2-common, libssl3, libsoup-3.0-0, libjavascriptcoregtk-4.1-0\nSection: devel\nPriority: optional\n' "$${VERSION}" > "$${DEB_DIR}/DEBIAN/control"; \
	dpkg-deb --build "$${DEB_DIR}" "target/$(DEB_NAME)"

# Build all bundles for current platform
bundle-all:
ifeq ($(UNAME),Darwin)
	$(MAKE) bundle-macos
else ifeq ($(UNAME),Linux)
	$(MAKE) bundle-linux
else
	$(MAKE) bundle-windows
endif
	@echo "All bundles built for $(UNAME)."

# ── Cross-compilation Toolchain Setup ────────────────────────────
.PHONY: setup-cross-linux setup-cross-windows

# Install Linux cross-compilation toolchain (macOS only)
setup-cross-linux:
ifeq ($(UNAME),Darwin)
	@echo "Installing Linux cross-compilation toolchain..."
	brew install zig 2>/dev/null || true
	cargo install cargo-zigbuild 2>/dev/null || true
	rustup target add x86_64-unknown-linux-gnu
	@echo ""
	@echo "Linux cross-compilation toolchain ready."
	@echo "Next: Run scripts/setup-linux-sysroot.sh to download GTK/WebKit libraries"
else
	@echo "Already on Linux, install build deps: sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev ..."
endif

# Install Windows cross-compilation toolchain (macOS only)
setup-cross-windows:
ifeq ($(UNAME),Darwin)
	@echo "Installing Windows cross-compilation toolchain..."
	brew install mingw-w64 2>/dev/null || true
	rustup target add x86_64-pc-windows-gnu
	@echo "Windows cross-compilation toolchain ready."
endif

# ── Run ─────────────────────────────────────────────────────────
.PHONY: run run-release

run:
	@npm run dev &
	@sleep 3
	cd $(CARGO_DIR) && cargo run &
	@sleep 2
	@echo "$(APP_NAME) starting (frontend: 1421, proxy: $(PORT))..."

run-release:
	@npm run build
	cd $(CARGO_DIR) && cargo run --release &
	@sleep 2
	@echo "$(APP_NAME) (release) starting on port $(PORT)..."

# ── Stop ────────────────────────────────────────────────────────
.PHONY: stop

ifeq ($(UNAME),Darwin)
stop:
	@echo "Stopping $(APP_NAME)..."
	@pkill -f "proxy-tauri" 2>/dev/null || true
	@PID=$$(lsof -ti :$(PORT) 2>/dev/null); \
	if [ -n "$$PID" ]; then \
		echo "Killing process on port $(PORT) (PID: $$PID)..."; \
		kill -9 $$PID; \
	fi
	@VITE_PID=$$(lsof -ti :1421 2>/dev/null); \
	if [ -n "$$VITE_PID" ]; then \
		echo "Killing Vite dev server on port 1421 (PID: $$VITE_PID)..."; \
		kill -9 $$VITE_PID; \
	fi
	@sleep 1
	@echo "$(APP_NAME) stopped."
else ifeq ($(UNAME),Linux)
stop:
	@echo "Stopping $(APP_NAME)..."
	@pkill -f "proxy-tauri" 2>/dev/null || true
	@PID=$$(fuser $(PORT)/tcp 2>/dev/null); \
	if [ -n "$$PID" ]; then \
		echo "Killing process on port $(PORT) (PID: $$PID)..."; \
		kill -9 $$PID; \
	fi
	@VITE_PID=$$(fuser 1421/tcp 2>/dev/null); \
	if [ -n "$$VITE_PID" ]; then \
		echo "Killing Vite dev server on port 1421 (PID: $$VITE_PID)..."; \
		kill -9 $$VITE_PID; \
	fi
	@sleep 1
	@echo "$(APP_NAME) stopped."
else
stop:
	@echo "Stopping $(APP_NAME)..."
	@taskkill /F /IM proxy-tauri.exe 2>nul || true
	@echo "$(APP_NAME) stopped."
endif

# ── Restart ─────────────────────────────────────────────────────
.PHONY: restart

restart: stop run
	@echo "$(APP_NAME) restarted."

# ── Status ──────────────────────────────────────────────────────
.PHONY: status

status:
	@echo "=== $(APP_NAME) Status ==="
	@echo ""
	@echo "Proxy port $(PORT):"
ifeq ($(UNAME),Darwin)
	@lsof -ti :$(PORT) 2>/dev/null && echo "  ✓ Running (PID: $$(lsof -ti :$(PORT) 2>/dev/null))" || echo "  ✗ Not running"
else ifeq ($(UNAME),Linux)
	@fuser $(PORT)/tcp 2>/dev/null && echo "  ✓ Running" || echo "  ✗ Not running"
else
	@netstat -ano | findstr :$(PORT) | findstr LISTENING >nul 2>&1 && echo "  ✓ Running" || echo "  ✗ Not running"
endif
	@echo ""
	@echo "Application process:"
	@pgrep -f "proxy-tauri" >/dev/null 2>&1 && echo "  ✓ Running (PID: $$(pgrep -f 'proxy-tauri'))" || echo "  ✗ Not running"

# ── Logs ────────────────────────────────────────────────────────
.PHONY: logs

logs:
	@echo "Tailing proxy-tauri logs..."
	@tail -f /tmp/proxy-tauri.log 2>/dev/null || echo "No log file found at /tmp/proxy-tauri.log"

# ── Install Dependencies ────────────────────────────────────────
.PHONY: install

install:
	npm install

# ── Help ────────────────────────────────────────────────────────
.PHONY: help

help:
	@echo "Usage:"
	@echo "  make install       - Install npm dependencies"
	@echo "  make build         - Debug build (Rust)"
	@echo "  make release       - Release build (Rust)"
	@echo "  make run           - Run $(APP_NAME) in background"
	@echo "  make run-release   - Run $(APP_NAME) release in background"
	@echo "  make stop          - Stop running $(APP_NAME)"
	@echo "  make restart       - Restart $(APP_NAME)"
	@echo "  make status        - Check if $(APP_NAME) and proxy are running"
	@echo "  make dev           - Dev mode (backend + frontend)"
	@echo "  make tauri-dev     - Run full Tauri dev mode"
	@echo "  make tauri-build     - Build production Tauri bundle"
	@echo "  make bundle-macos              - Build both macOS bundles (ARM + Intel)"
	@echo "  make bundle-macos-arm          - Build macOS ARM bundle (Apple Silicon)"
	@echo "  make bundle-macos-x64          - Build macOS Intel bundle (x86_64)"
	@echo "  make bundle-windows            - Build Windows bundle (.exe + WebView2Loader.dll)"
	@echo "  make bundle-linux              - Build Linux bundle (.deb)"
	@echo "  make bundle-linux-deb          - Build Linux .deb only"
	@echo "  make bundle-all              - Build all platform bundles"
	@echo "  make release-clean           - Clean release/ output only"
	@echo "  make setup-cross-linux       - Install Linux cross-compile toolchain"
	@echo "  make setup-cross-windows     - Install Windows cross-compile toolchain"
	@echo "  make clean                   - Clean all build artifacts"
