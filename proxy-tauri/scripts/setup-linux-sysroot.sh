#!/bin/bash
set -euo pipefail

# Setup Linux sysroot for cross-compiling Tauri from macOS
# Downloads required .deb packages from Ubuntu 24.04

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
SYSROOT="${PROJECT_DIR}/linux-sysroot"
UBUNTU_MIRROR="http://archive.ubuntu.com/ubuntu"
WORKDIR="/tmp/sysroot-$$"
ARCH="amd64"

cleanup() { rm -rf "$WORKDIR"; }
trap cleanup EXIT

mkdir -p "$WORKDIR/deb" "$SYSROOT"

# ── Step 1: Download index ──
echo "=== Downloading Ubuntu package index ==="
for d in noble noble-updates noble-security; do
  for c in main universe; do
    curl -sfL --retry 2 -o "${WORKDIR}/idx-${d}-${c}.gz" \
      "${UBUNTU_MIRROR}/dists/${d}/${c}/binary-${ARCH}/Packages.gz" 2>/dev/null && echo "  ✓ ${d}/${c}" || echo "  ✗ ${d}/${c}"
  done
done

cat "${WORKDIR}/"idx-*.gz 2>/dev/null | gunzip -c > "${WORKDIR}/Packages" 2>/dev/null || true
echo "  Index: $(wc -c < "${WORKDIR}/Packages" | tr -d ' ') bytes"

# ── Step 2: One-pass extract Filename for target packages ──
echo ""
echo "=== Extracting package URLs (single pass) ==="

# All packages needed for Tauri 2 GTK/WebKit build chain
# Source: apt-cache depends output on Ubuntu 24.04 for:
# libwebkit2gtk-4.1-dev libgtk-3-dev libjavascriptcoregtk-4.1-dev 
# libsoup-3.0-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev
TARGETS=(
    # ── Direct Tauri deps ──
    libwebkit2gtk-4.1-dev libwebkit2gtk-4.1-0
    libgtk-3-dev libgtk-3-0 libgtk-3-common
    libjavascriptcoregtk-4.1-dev libjavascriptcoregtk-4.1-0
    libsoup-3.0-dev libsoup-3.0-0 libsoup-3.0-common
    libayatana-appindicator3-dev libayatana-appindicator3-1
    librsvg2-dev librsvg2-2 librsvg2-common
    libssl-dev libssl3 openssl

    # ── GLib stack ──
    libglib2.0-dev libglib2.0-0 libglib2.0-data
    libgobject-2.0-0 libgio-2.0-0 libgio2.0-0
    libgmodule-2.0-0 libgirepository-2.0-0 libgirepository-1.0-1
    libffi-dev libffi8
    libpcre2-dev libpcre2-8-0

    # ── Cairo / rendering ──
    libcairo2-dev libcairo2 libcairo-gobject2 libcairo-script-interpreter2
    libpixman-1-dev libpixman-1-0
    libfreetype-dev libfreetype6
    libfontconfig-dev libfontconfig1
    libpng-dev libpng16-16t64

    # ── Pango (text layout) ──
    libpango1.0-dev libpango-1.0-0 libpangocairo-1.0-0 libpangoft2-1.0-0
    libharfbuzz-dev libharfbuzz0b libharfbuzz-icu0
    libfribidi-dev libfribidi0
    libthai-dev libthai0 libdatrie-dev libdatrie1
    libgraphite2-dev libgraphite2-3

    # ── ATK (accessibility) ──
    libatk1.0-dev libatk1.0-0
    libatk-bridge2.0-dev libatk-bridge2.0-0

    # ── GdkPixbuf ──
    libgdk-pixbuf-2.0-dev libgdk-pixbuf2.0-0 libgdk-pixbuf2.0-common
    libjpeg-turbo8-dev libjpeg-turbo8 libjpeg8-dev libjpeg8
    libtiff-dev libtiff6
    libdeflate-dev libdeflate0
    liblerc-dev liblerc4
    libwebp-dev libwebp7 libwebpdemux2 libwebpmux3
    libsharpyuv-dev libsharpyuv0
    libjbig-dev libjbig0
    liblzma-dev liblzma5
    libzstd-dev libzstd1

    # ── X11 ──
    libx11-dev libx11-6 libx11-data libx11-xcb1
    libxcb1-dev libxcb1 libxcb-render0-dev libxcb-render0
    libxcb-shm0-dev libxcb-shm0
    libxrender-dev libxrender1
    libxext-dev libxext6
    libxfixes-dev libxfixes3
    libxi-dev libxi6
    libxrandr-dev libxrandr2
    libxcursor-dev libxcursor1
    libxinerama-dev libxinerama1
    libxcomposite-dev libxcomposite1
    libxdamage-dev libxdamage1
    libxss-dev libxss1
    libxtst-dev libxtst6
    libxau-dev libxau6 libxdmcp-dev libxdmcp6

    # ── Wayland / EGL / GL ──
    libwayland-dev libwayland-client0 libwayland-cursor0 libwayland-egl1
    libxkbcommon-dev libxkbcommon0
    libepoxy-dev libepoxy0
    libdrm-dev libdrm2 libdrm-amdgpu1 libdrm-intel1 libdrm-nouveau2 libdrm-radeon1
    libgbm-dev libgbm1
    libegl-dev libegl1 libegl-mesa0
    libgl-dev libgl1 libglx-dev libglx0 libgles-dev libgles2 libopengl0
    libglvnd-dev libglvnd0

    # ── DBus ──
    libdbus-1-dev libdbus-1-3
    libdbusmenu-glib-dev libdbusmenu-glib4 libdbusmenu-gtk3-4
    libayatana-ido3-dev libayatana-ido3-0.4-0 libayatana-indicator3-7

    # ── Enchant (spell check for WebKit) ──
    libenchant-2-dev libenchant-2-2

    # ── ICU ──
    libicu-dev libicu74

    # ── libsecret (password storage) ──
    libsecret-1-dev libsecret-1-0

    # ── System libs ──
    zlib1g-dev zlib1g
    libbz2-dev libbz2-1.0
    libexpat1-dev libexpat1
    libxml2-dev libxml2
    libsystemd-dev libsystemd0 libsystemd-shared
    libudev-dev libudev1
    libmount-dev libmount1
    libblkid-dev libblkid1
    libselinux1-dev libselinux1
    libsepol-dev libsepol2
    libgcrypt20-dev libgcrypt20
    libgpg-error-dev libgpg-error0
    libtasn1-6-dev libtasn1-6
    libp11-kit-dev libp11-kit0
    libgnutls30-dev libgnutls30t64
    libnettle8t64
    libhogweed6t64
    libgmp-dev libgmp10
    libidn2-dev libidn2-0
    libunistring-dev libunistring5

    # ── GStreamer ──
    libgstreamer1.0-dev libgstreamer1.0-0
    libgstreamer-plugins-base1.0-dev libgstreamer-plugins-base1.0-0
    liborc-0.4-dev liborc-0.4-0
    libgstreamer-plugins-bad1.0-dev libgstreamer-plugins-bad1.0-0
    libwoff-dev libwoff1

    # ── Hyphen / Woff2 ──
    libhyphen-dev libhyphen0

    # ── libmanette (gamepad) ──
    libmanette-0.2-dev libmanette-0.2-0

    # ── libnotify ──
    libnotify-dev libnotify4

    # ── libavif ──
    libavif-dev libavif16 libyuv-dev libyuv0 libdav1d-dev libdav1d7
    libaom-dev libaom3
    librav1e-dev librav1e0.7

    # ── Brotli ──
    libbrotli-dev libbrotli1

    # ── pkg-config / toolchain ──
    pkgconf pkg-config
    libpkgconf3

    # ── GTK theme engines ──
    libadwaita-1-dev libadwaita-1-0
    gtk-update-icon-cache
    adwaita-icon-theme
    hicolor-icon-theme
    shared-mime-info

    # ── mime / desktop integration ──
    libglib2.0-bin
    desktop-file-utils
)

# Build a fast lookup: awk one-pass to find all target filenames
# Create a pattern file for grep
PATTERN_FILE="${WORKDIR}/targets.txt"
printf '%s\n' "${TARGETS[@]}" > "$PATTERN_FILE"

# Use awk to find Package blocks matching our targets and extract Filename
echo "  Matching ${#TARGETS[@]} target packages..."
awk -v patfile="$PATTERN_FILE" '
    BEGIN {
        while ((getline line < patfile) > 0) targets[line] = 1
        close(patfile)
    }
    /^Package: / {
        pkg = $2
        in_target = (pkg in targets) ? 1 : 0
    }
    in_target && /^Filename: / {
        print $2
        in_target = 0
    }
' "${WORKDIR}/Packages" > "${WORKDIR}/urls.txt"

FOUND=$(wc -l < "${WORKDIR}/urls.txt" | tr -d ' ')
echo "  Found $FOUND / ${#TARGETS[@]} packages"

# ── Step 3: Download ──
echo ""
echo "=== Downloading and extracting ==="
COUNT=0; OK=0; FAIL=0

while IFS= read -r filename; do
    [ -z "$filename" ] && continue
    ((COUNT++))
    url="${UBUNTU_MIRROR}/${filename}"
    deb="${WORKDIR}/deb/$(basename "$filename")"

    curl -sfL --retry 2 -o "$deb" "$url" 2>/dev/null || true

    sz=$(stat -f%z "$deb" 2>/dev/null || stat -c%s "$deb" 2>/dev/null || echo 0)
    if [ -f "$deb" ] && [ "$sz" -gt 100 ]; then
        if dpkg-deb -x "$deb" "$SYSROOT" 2>/dev/null; then
            echo "  [${COUNT}/${FOUND}] ✓ $(basename "$filename" .deb | sed 's/_.*//')"
            ((OK++))
        else
            echo "  [${COUNT}/${FOUND}] ✗ $(basename "$filename") (extract)"
            ((FAIL++))
        fi
    else
        echo "  [${COUNT}/${FOUND}] ✗ $(basename "$filename") (download)"
        ((FAIL++))
    fi
done < "${WORKDIR}/urls.txt"

echo ""
echo "=== Done: $OK extracted, $FAIL failed ==="
echo "Sysroot: $SYSROOT ($(du -sh "$SYSROOT" 2>/dev/null | cut -f1))"

if [ $OK -gt 0 ]; then
    echo ""
    echo "=== Build command ==="
    echo "export PKG_CONFIG_SYSROOT_DIR=$SYSROOT"
    echo "export PKG_CONFIG_PATH=$SYSROOT/usr/lib/x86_64-linux-gnu/pkgconfig:$SYSROOT/usr/lib/pkgconfig:$SYSROOT/usr/share/pkgconfig"
    echo 'export PKG_CONFIG_ALLOW_CROSS=1'
    echo ""
    echo "# Then build with:"
    echo "cd src-tauri"
    echo "cargo build --release --target x86_64-unknown-linux-gnu"
fi
