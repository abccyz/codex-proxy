APP_NAME := proxy-rs
BIN := target/release/$(APP_NAME)
PORT := 8000

# Detect OS
UNAME := $(shell uname -s)

# ── Build ───────────────────────────────────────────────────────
.PHONY: build release build-linux build-macos build-windows clean

build:
	cargo build

release:
	cargo build --release

# Cross-compile targets (requires appropriate toolchain installed)
build-linux:
	rustup target add x86_64-unknown-linux-gnu 2>/dev/null || true
	cargo build --release --target x86_64-unknown-linux-gnu

build-macos:
	rustup target add x86_64-apple-darwin 2>/dev/null || true
	cargo build --release --target x86_64-apple-darwin
	rustup target add aarch64-apple-darwin 2>/dev/null || true
	cargo build --release --target aarch64-apple-darwin
	@echo "Binaries:"
	@echo "  target/x86_64-apple-darwin/release/$(APP_NAME)"
	@echo "  target/aarch64-apple-darwin/release/$(APP_NAME)"

build-windows:
	rustup target add x86_64-pc-windows-gnu 2>/dev/null || true
	cargo build --release --target x86_64-pc-windows-gnu
	@echo "Binary:"
	@echo "  target/x86_64-pc-windows-gnu/release/$(APP_NAME).exe"

# ── Run ─────────────────────────────────────────────────────────
.PHONY: run run-release

run:
	cargo run

run-release:
	cargo run --release

# ── Stop ────────────────────────────────────────────────────────
.PHONY: stop

ifeq ($(UNAME),Darwin)
stop:
	@PID=$$(lsof -ti :$(PORT) 2>/dev/null); \
	if [ -n "$$PID" ]; then \
		echo "Stopping $(APP_NAME) (PID: $$PID)..."; \
		kill $$PID; \
		echo "Done."; \
	else \
		echo "No process found on port $(PORT)."; \
	fi
else ifeq ($(UNAME),Linux)
stop:
	@PID=$$(fuser $(PORT)/tcp 2>/dev/null); \
	if [ -n "$$PID" ]; then \
		echo "Stopping $(APP_NAME) (PID: $$PID)..."; \
		kill $$PID; \
		echo "Done."; \
	else \
		echo "No process found on port $(PORT)."; \
	fi
else
# Windows (Git Bash / MSYS)
stop:
	@echo "Finding process on port $(PORT)..."
	@netstat -ano | findstr :$(PORT) | findstr LISTENING > /dev/null 2>&1 && \
		(for /f "tokens=5" %a in ('netstat -ano ^| findstr :$(PORT) ^| findstr LISTENING') do taskkill /PID %a) || \
		echo "No process found on port $(PORT)."
endif

# ── Status ──────────────────────────────────────────────────────
.PHONY: status

status:
	@echo "Checking port $(PORT)..."
	@lsof -ti :$(PORT) 2>/dev/null && echo "Running." || echo "Not running."

# ── Clean ───────────────────────────────────────────────────────
clean:
	cargo clean

# ── Help ────────────────────────────────────────────────────────
.PHONY: help

help:
	@echo "Usage:"
	@echo "  make build          - Debug build"
	@echo "  make release        - Release build"
	@echo "  make build-linux    - Cross-compile for Linux x86_64"
	@echo "  make build-macos    - Cross-compile for macOS x86_64 + arm64"
	@echo "  make build-windows  - Cross-compile for Windows x86_64 (requires mingw-w64)"
	@echo "  make run            - Run (debug)"
	@echo "  make run-release    - Run (release)"
	@echo "  make stop           - Stop running process"
	@echo "  make status         - Check if running"
	@echo "  make clean          - Clean build artifacts"
