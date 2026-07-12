APP_NAME := proxy-tauri
PORT := 8000
CARGO_DIR := src-tauri
UNAME := $(shell uname -s)

# ── Build ───────────────────────────────────────────────────────
.PHONY: build release clean

build:
	cd $(CARGO_DIR) && cargo build

release:
	cd $(CARGO_DIR) && cargo build --release

clean:
	cd $(CARGO_DIR) && cargo clean
	rm -rf node_modules dist

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
.PHONY: bundle-macos bundle-windows bundle-linux bundle-all

# macOS: .dmg + .app
bundle-macos:
	@echo "Building macOS bundle (.dmg + .app)..."
	npm run tauri build -- --bundles dmg app
	@echo "macOS bundle built: $(CARGO_DIR)/target/release/bundle/"

# Windows: .msi + .nsis installer
bundle-windows:
	@echo "Building Windows bundle (.msi + .nsis)..."
	npm run tauri build -- --bundles msi nsis
	@echo "Windows bundle built: $(CARGO_DIR)/target/release/bundle/"

# Linux: .deb + .rpm + .AppImage
bundle-linux:
	@echo "Building Linux bundle (.deb + .rpm + .AppImage)..."
	npm run tauri build -- --bundles deb rpm appimage
	@echo "Linux bundle built: $(CARGO_DIR)/target/release/bundle/"

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

# ── Run ─────────────────────────────────────────────────────────
.PHONY: run run-release

run:
	@npm run dev &
	@sleep 3
	cd $(CARGO_DIR) && cargo run &
	@sleep 2
	@echo "$(APP_NAME) starting (frontend: 1420, proxy: $(PORT))..."

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
	@VITE_PID=$$(lsof -ti :1420 2>/dev/null); \
	if [ -n "$$VITE_PID" ]; then \
		echo "Killing Vite dev server on port 1420 (PID: $$VITE_PID)..."; \
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
	@VITE_PID=$$(fuser 1420/tcp 2>/dev/null); \
	if [ -n "$$VITE_PID" ]; then \
		echo "Killing Vite dev server on port 1420 (PID: $$VITE_PID)..."; \
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
	@echo "  make bundle-macos    - Build macOS bundle (.dmg + .app)"
	@echo "  make bundle-windows  - Build Windows bundle (.msi + .nsis)"
	@echo "  make bundle-linux    - Build Linux bundle (.deb + .rpm + .AppImage)"
	@echo "  make bundle-all      - Build bundles for current platform"
	@echo "  make clean           - Clean all build artifacts"
