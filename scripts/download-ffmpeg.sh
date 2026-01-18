#!/bin/bash
# Download ffmpeg, ffprobe, and yt-dlp static binaries for bundling

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BINARIES_DIR="$PROJECT_DIR/src-tauri/binaries"

mkdir -p "$BINARIES_DIR"

# Detect architecture
ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
    ARCH_SUFFIX="aarch64-apple-darwin"
    YTDLP_SUFFIX="macos"
else
    ARCH_SUFFIX="x86_64-apple-darwin"
    YTDLP_SUFFIX="macos"
fi

cd "$BINARIES_DIR"

echo "=== Downloading ffmpeg for $ARCH ==="

# Download from evermeet.cx (macOS static builds)
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

# Rename with Tauri sidecar naming convention
mv ffmpeg "ffmpeg-$ARCH_SUFFIX" 2>/dev/null || true
mv ffprobe "ffprobe-$ARCH_SUFFIX" 2>/dev/null || true

chmod +x "ffmpeg-$ARCH_SUFFIX"
chmod +x "ffprobe-$ARCH_SUFFIX"

echo "=== Downloading yt-dlp ==="

# yt-dlp provides standalone macOS binaries
# https://github.com/yt-dlp/yt-dlp/releases
# Use the direct latest release URL which always points to the newest version
YTDLP_URL="https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos"
echo "Downloading from: $YTDLP_URL"

curl -L "$YTDLP_URL" -o "yt-dlp-$ARCH_SUFFIX"
chmod +x "yt-dlp-$ARCH_SUFFIX"

# Verify it downloaded correctly
YTDLP_SIZE=$(stat -f%z "yt-dlp-$ARCH_SUFFIX" 2>/dev/null || stat -c%s "yt-dlp-$ARCH_SUFFIX" 2>/dev/null)
if [ "$YTDLP_SIZE" -lt 1000000 ]; then
    echo "ERROR: yt-dlp download failed (file too small: $YTDLP_SIZE bytes)"
    exit 1
fi
echo "yt-dlp downloaded successfully ($YTDLP_SIZE bytes)"

echo ""
echo "=== Done! Binaries installed to $BINARIES_DIR ==="
ls -la "$BINARIES_DIR"
