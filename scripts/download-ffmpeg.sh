#!/bin/bash
# Download ffmpeg and ffprobe static binaries for bundling

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BINARIES_DIR="$PROJECT_DIR/src-tauri/binaries"

mkdir -p "$BINARIES_DIR"

OS=$(uname -s)
ARCH=$(uname -m)

case "$OS-$ARCH" in
    Darwin-arm64|Darwin-aarch64)
        TARGET_TRIPLE="aarch64-apple-darwin"
        FFMPEG_VARIANT="evermeet"
        ;;
    Darwin-x86_64)
        TARGET_TRIPLE="x86_64-apple-darwin"
        FFMPEG_VARIANT="evermeet"
        ;;
    Linux-x86_64)
        TARGET_TRIPLE="x86_64-unknown-linux-gnu"
        LINUX_BUILD="linux64"
        FFMPEG_VARIANT="lgpl"
        ;;
    Linux-aarch64|Linux-arm64)
        TARGET_TRIPLE="aarch64-unknown-linux-gnu"
        LINUX_BUILD="linuxarm64"
        FFMPEG_VARIANT="lgpl"
        ;;
    *)
        echo "Unsupported platform: $OS-$ARCH"
        exit 1
        ;;
esac

FFMPEG_DEST="$BINARIES_DIR/ffmpeg-$TARGET_TRIPLE"
FFPROBE_DEST="$BINARIES_DIR/ffprobe-$TARGET_TRIPLE"
VARIANT_MARKER="$BINARIES_DIR/ffmpeg-$TARGET_TRIPLE.variant"
INSTALLED_VARIANT=""

if [ -f "$VARIANT_MARKER" ]; then
    INSTALLED_VARIANT=$(head -n 1 "$VARIANT_MARKER")
fi

if [ "${RIPPR_FORCE_FFMPEG:-}" != "1" ] && [ "$INSTALLED_VARIANT" = "$FFMPEG_VARIANT" ] && [ -f "$FFMPEG_DEST" ] && [ -f "$FFPROBE_DEST" ]; then
    echo "ffmpeg already exists for $TARGET_TRIPLE ($FFMPEG_VARIANT)."
    echo "Set RIPPR_FORCE_FFMPEG=1 to download it again."
    ls -la "$BINARIES_DIR"
    exit 0
fi

cd "$BINARIES_DIR"

echo "=== Downloading ffmpeg for $TARGET_TRIPLE ==="

if [ "$OS" = "Darwin" ]; then
    FFMPEG_URL="https://evermeet.cx/ffmpeg/getrelease/ffmpeg/zip"
    FFPROBE_URL="https://evermeet.cx/ffmpeg/getrelease/ffprobe/zip"

    echo "Downloading ffmpeg..."
    curl -L "$FFMPEG_URL" -o ffmpeg.zip
    unzip -o ffmpeg.zip
    rm ffmpeg.zip

    echo "Downloading ffprobe..."
    curl -L "$FFPROBE_URL" -o ffprobe.zip
    unzip -o ffprobe.zip
    rm ffprobe.zip
else
    FFMPEG_URL="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-${LINUX_BUILD}-${FFMPEG_VARIANT}.tar.xz"
    TEMP_DIR=$(mktemp -d)
    TEMP_TAR="$TEMP_DIR/ffmpeg.tar.xz"

    echo "Downloading from: $FFMPEG_URL"
    curl -L "$FFMPEG_URL" -o "$TEMP_TAR"

    echo "Extracting ffmpeg..."
    tar -xJf "$TEMP_TAR" -C "$TEMP_DIR"
    find "$TEMP_DIR" -type f -name ffmpeg -exec cp {} ffmpeg \;
    find "$TEMP_DIR" -type f -name ffprobe -exec cp {} ffprobe \;
    rm -rf "$TEMP_DIR"
fi

if [ ! -f ffmpeg ] || [ ! -f ffprobe ]; then
    echo "ERROR: ffmpeg or ffprobe was not found after extraction"
    exit 1
fi

mv ffmpeg "ffmpeg-$TARGET_TRIPLE"
mv ffprobe "ffprobe-$TARGET_TRIPLE"
printf '%s\n' "$FFMPEG_VARIANT" > "$VARIANT_MARKER"

chmod +x "ffmpeg-$TARGET_TRIPLE"
chmod +x "ffprobe-$TARGET_TRIPLE"

echo ""
echo "=== Done! Binaries installed to $BINARIES_DIR ==="
ls -la "$BINARIES_DIR"
