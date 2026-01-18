#!/bin/bash
# Script to download and bundle a standalone Python with yt-dlp for the Tauri app
# Uses python-build-standalone for portable Python distributions

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
RESOURCES_DIR="$PROJECT_ROOT/src-tauri/resources"
# Must match the system Python version that PyO3 compiles against
PYTHON_VERSION="3.13.1"
RELEASE_DATE="20250115"

# Detect platform - use install_only (not stripped) to keep C extension modules
case "$(uname -s)-$(uname -m)" in
    Darwin-arm64)
        PLATFORM="aarch64-apple-darwin"
        PYTHON_URL="https://github.com/indygreg/python-build-standalone/releases/download/${RELEASE_DATE}/cpython-${PYTHON_VERSION}+${RELEASE_DATE}-${PLATFORM}-install_only.tar.gz"
        ;;
    Darwin-x86_64)
        PLATFORM="x86_64-apple-darwin"
        PYTHON_URL="https://github.com/indygreg/python-build-standalone/releases/download/${RELEASE_DATE}/cpython-${PYTHON_VERSION}+${RELEASE_DATE}-${PLATFORM}-install_only.tar.gz"
        ;;
    Linux-x86_64)
        PLATFORM="x86_64-unknown-linux-gnu"
        PYTHON_URL="https://github.com/indygreg/python-build-standalone/releases/download/${RELEASE_DATE}/cpython-${PYTHON_VERSION}+${RELEASE_DATE}-${PLATFORM}-install_only.tar.gz"
        ;;
    *)
        echo "Unsupported platform: $(uname -s)-$(uname -m)"
        exit 1
        ;;
esac

echo "Platform: $PLATFORM"
echo "Python URL: $PYTHON_URL"

# Create resources directory
mkdir -p "$RESOURCES_DIR"

# Download Python if not already present
PYTHON_DIR="$RESOURCES_DIR/python"
if [ ! -d "$PYTHON_DIR" ]; then
    echo "Downloading standalone Python..."
    TEMP_TAR=$(mktemp)
    curl -L "$PYTHON_URL" -o "$TEMP_TAR"

    echo "Extracting Python..."
    mkdir -p "$PYTHON_DIR"
    tar -xzf "$TEMP_TAR" -C "$RESOURCES_DIR"
    rm "$TEMP_TAR"

    echo "Python extracted to $PYTHON_DIR"
else
    echo "Python already exists at $PYTHON_DIR"
fi

# Find the Python binary
PYTHON_BIN="$PYTHON_DIR/bin/python3"
if [ ! -f "$PYTHON_BIN" ]; then
    echo "Error: Python binary not found at $PYTHON_BIN"
    exit 1
fi

# Install yt-dlp
echo "Installing yt-dlp..."
"$PYTHON_BIN" -m pip install --upgrade pip
"$PYTHON_BIN" -m pip install yt-dlp

# Verify installation
echo "Verifying yt-dlp installation..."
"$PYTHON_BIN" -c "import yt_dlp; print(f'yt-dlp version: {yt_dlp.version.__version__}')"

echo ""
echo "Python bundling complete!"
echo "Bundled Python location: $PYTHON_DIR"
echo ""
echo "To use this in development, set these environment variables:"
echo "  export PYTHONHOME=$PYTHON_DIR"
echo "  export PYTHONPATH=$PYTHON_DIR/lib/python3.13"
