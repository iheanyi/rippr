#!/bin/bash
# Download ffmpeg and ffprobe static binaries for bundling

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BINARIES_DIR="$PROJECT_DIR/src-tauri/binaries"

mkdir -p "$BINARIES_DIR"

# Detect architecture
ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
    ARCH_SUFFIX="aarch64-apple-darwin"
else
    ARCH_SUFFIX="x86_64-apple-darwin"
fi

echo "Downloading ffmpeg for $ARCH..."

# Download from evermeet.cx (macOS static builds)
# These are well-maintained static builds for macOS
FFMPEG_URL="https://evermeet.cx/ffmpeg/getrelease/ffmpeg/zip"
FFPROBE_URL="https://evermeet.cx/ffmpeg/getrelease/ffprobe/zip"

cd "$BINARIES_DIR"

# Download ffmpeg
echo "Downloading ffmpeg..."
curl -L "$FFMPEG_URL" -o ffmpeg.zip
unzip -o ffmpeg.zip
rm ffmpeg.zip

# Download ffprobe
echo "Downloading ffprobe..."
curl -L "$FFPROBE_URL" -o ffprobe.zip
unzip -o ffprobe.zip
rm ffprobe.zip

# Rename with Tauri sidecar naming convention
# Tauri expects: binary-name-target-triple
mv ffmpeg "ffmpeg-$ARCH_SUFFIX"
mv ffprobe "ffprobe-$ARCH_SUFFIX"

# Make executable
chmod +x "ffmpeg-$ARCH_SUFFIX"
chmod +x "ffprobe-$ARCH_SUFFIX"

echo "Done! Binaries installed to $BINARIES_DIR"
ls -la "$BINARIES_DIR"
