---
name: tauri-cross-compile
description: Cross-compile Tauri app for Windows from macOS — toolchain setup and build commands
source: auto-skill
extracted_at: '2026-07-13T09:45:26.262Z'
---

# Cross-compile Tauri for Windows from macOS

## Problem

Running `make bundle-windows` (or `tauri build --bundles msi nsis`) on macOS fails:

```
error: invalid value 'msi' for '--bundles [<BUNDLES>...]'
  [possible values: ios, app, dmg]
```

Tauri CLI's `--bundles` flag only lists **native platform** bundle formats. MSI/NSIS require Windows-specific tools (WiX toolset) that don't exist on macOS.

**Key insight:** Rust itself supports cross-compilation, but Tauri's **bundler** step is platform-restricted. The workaround is to skip bundling and only cross-compile the binary.

## Toolchain Setup

```bash
# 1. Add Windows cross-compile target
rustup target add x86_64-pc-windows-gnu

# 2. Install mingw-w64 cross-compiler (large download, may take time)
brew install mingw-w64
```

**Note:** `brew install mingw-w64` downloads from ghcr.io and the bottle is ~180MB+. On slow connections to ghcr.io this can take 10+ minutes. Run it in the background and monitor progress. Also install dependencies: `gmp`, `isl`, `mpfr`, `libmpc`.

## Common Pitfall: Invalid icon.ico

If `icon.ico` is actually a PNG file (check with `file icon.ico` — it will say "PNG image data" instead of "MS Windows icon resource"), the Windows resource compiler (`windres`) will fail:

```
x86_64-w64-mingw32-windres: icon file `.../icons/icon.ico' does not contain icon data
```

**Fix:** Regenerate the ICO from the source PNG:

```bash
npx tauri icon src-tauri/icons/icon.png
```

This overwrites `icon.ico` with a proper multi-size MS Windows icon resource.

## Cargo Config

Add to `src-tauri/.cargo/config.toml`:

```toml
[target.x86_64-pc-windows-gnu]
linker = "x86_64-w64-mingw32-gcc"
```

## Build Command

```bash
# Cross-compile only, skip bundling
npm run tauri build -- --target x86_64-pc-windows-gnu --no-bundle
```

This produces the Windows `.exe` binary at:
`src-tauri/target/x86_64-pc-windows-gnu/release/<app-name>.exe`

To create installers (MSI/NSIS), you still need a Windows environment.
