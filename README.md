# Rippr

A desktop app for downloading audio samples from YouTube, SoundCloud, Bandcamp, and other sources. Built for beat makers who need quick access to samples with proper metadata.

## Features

- **Multi-source support** - Download from YouTube, SoundCloud, Bandcamp, and 1000+ sites via yt-dlp
- **Smart metadata parsing** - Auto-extracts title and artist from video titles
- **Album art embedding** - Thumbnails automatically embedded as cover art
- **Audio trimming** - Select start/end points before downloading
- **BPM & Key detection** - Analyzes audio to detect tempo and musical key
- **Batch downloads** - Queue multiple URLs and download them all
- **Download history** - Search and re-download past samples
- **Auto-updates** - Keep yt-dlp up to date with one click

## Installation

### Pre-built releases

Download the latest release for your platform from the [Releases](https://github.com/iheanyi/rippr/releases) page.

**Zero dependencies** - Just download and run. Python, yt-dlp, and ffmpeg are all bundled.

### Build from source

**Prerequisites:**
- Node.js 18+
- Rust 1.70+

```bash
# Clone the repo
git clone https://github.com/iheanyi/rippr.git
cd rippr

# Install dependencies
npm install

# Bundle Python with yt-dlp (one-time setup)
./scripts/bundle-python.sh

# Download ffmpeg binaries for bundling
./scripts/download-ffmpeg.sh

# Run in development
npm run tauri dev

# Build for production
npm run tauri build
```

**Note:** Release builds are fully self-contained - Python, yt-dlp, and ffmpeg are all bundled.

## Tech Stack

- **Frontend**: React + TypeScript + Vite
- **Backend**: Rust + Tauri 2.0
- **Audio**: Symphonia (decoding) + mp3lame-encoder (encoding)
- **Downloads**: yt-dlp (bundled Python + subprocess)
- **Database**: SQLite (rusqlite)

## Usage

1. Paste a URL (or drag & drop)
2. Edit the title and artist if needed
3. Optionally enable trim mode to select a portion
4. Click Download

Samples are saved as MP3 files with embedded metadata and album art.

### Keyboard Shortcuts

- `Cmd/Ctrl + V` - Paste URL and auto-fetch
- `Enter` - Start download (when ready)
- `Escape` - Cancel download or reset

## License

MIT
