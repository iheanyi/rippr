import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import "./App.css";
import { DownloadQueue } from "./components/DownloadQueue";
import { History } from "./components/History";
import { TrimWaveform } from "./components/TrimWaveform";

interface VideoMetadata {
  videoId: string;
  rawTitle: string;
  title: string;
  artist: string;
  thumbnail: string | null;
  duration: number | null;
  channelName: string | null;
}

interface Settings {
  download_dir: string;
}

interface DownloadResult {
  success: boolean;
  path: string;
}

interface DownloadProgress {
  stage: string;
  percent: number;
  message: string;
}

type AppState = "idle" | "fetching" | "ready" | "downloading" | "complete" | "error";

function App() {
  const [url, setUrl] = useState("");
  const [metadata, setMetadata] = useState<VideoMetadata | null>(null);
  const [title, setTitle] = useState("");
  const [artist, setArtist] = useState("");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [appState, setAppState] = useState<AppState>("idle");
  const [error, setError] = useState("");
  const [downloadedPath, setDownloadedPath] = useState("");
  const [showSettings, setShowSettings] = useState(false);
  const [showQueue, setShowQueue] = useState(false);
  const [showHistory, setShowHistory] = useState(false);
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [isDragOver, setIsDragOver] = useState(false);
  const [trimEnabled, setTrimEnabled] = useState(false);
  const [trimStart, setTrimStart] = useState(0);
  const [trimEnd, setTrimEnd] = useState(0);
  const [ytdlpVersion, setYtdlpVersion] = useState<string | null>(null);
  const [ytdlpUpdateAvailable, setYtdlpUpdateAvailable] = useState<string | null>(null);
  const [isUpdatingYtdlp, setIsUpdatingYtdlp] = useState(false);

  // Load settings on mount
  useEffect(() => {
    loadSettings();
    checkYtdlpVersion();
  }, []);

  // Check for yt-dlp updates when settings panel is opened
  useEffect(() => {
    if (showSettings) {
      checkYtdlpVersion();
    }
  }, [showSettings]);

  async function checkYtdlpVersion() {
    try {
      const version = await invoke<string>("get_ytdlp_version");
      setYtdlpVersion(version);

      // Check for updates in background
      const updateVersion = await invoke<string | null>("check_ytdlp_update");
      setYtdlpUpdateAvailable(updateVersion);
    } catch (e) {
      console.error("Failed to check yt-dlp version:", e);
    }
  }

  async function handleUpdateYtdlp() {
    setIsUpdatingYtdlp(true);
    try {
      const newVersion = await invoke<string>("update_ytdlp");
      setYtdlpVersion(newVersion);
      setYtdlpUpdateAvailable(null);
    } catch (e) {
      console.error("Failed to update yt-dlp:", e);
      alert("Failed to update yt-dlp: " + String(e));
    }
    setIsUpdatingYtdlp(false);
  }

  // Listen for download progress events
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    const setupListener = async () => {
      unlisten = await listen<DownloadProgress>("download-progress", (event) => {
        setProgress(event.payload);
      });
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = async (e: KeyboardEvent) => {
      // Cmd/Ctrl + V: Auto-paste and fetch
      if ((e.metaKey || e.ctrlKey) && e.key === "v" && appState === "idle") {
        try {
          const text = await navigator.clipboard.readText();
          if (text && (text.startsWith("http://") || text.startsWith("https://"))) {
            setUrl(text);
            // Small delay to let state update
            setTimeout(() => {
              const fetchBtn = document.querySelector(".url-section button") as HTMLButtonElement;
              if (fetchBtn && !fetchBtn.disabled) {
                fetchBtn.click();
              }
            }, 100);
          }
        } catch (err) {
          // Clipboard access denied, ignore
        }
      }

      // Escape: Cancel download or reset
      if (e.key === "Escape") {
        if (appState === "downloading") {
          handleCancel();
        } else if (appState === "error" || appState === "complete") {
          handleReset();
        }
      }

      // Enter: Download when ready
      if (e.key === "Enter" && appState === "ready" && !e.target) {
        handleDownload();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [appState]);

  // Reset trim state when metadata changes
  useEffect(() => {
    if (metadata?.duration) {
      setTrimStart(0);
      setTrimEnd(metadata.duration);
      setTrimEnabled(false);
    }
  }, [metadata?.duration]);

  const handleTrimChange = useCallback((start: number, end: number) => {
    setTrimStart(start);
    setTrimEnd(end);
  }, []);

  async function loadSettings() {
    try {
      const s = await invoke<Settings>("get_settings");
      setSettings(s);
    } catch (e) {
      console.error("Failed to load settings:", e);
      const defaultDir = await invoke<string>("get_default_download_dir");
      setSettings({ download_dir: defaultDir });
    }
  }

  async function saveSettings(newSettings: Settings) {
    try {
      await invoke("save_settings", { settings: newSettings });
      setSettings(newSettings);
    } catch (e) {
      console.error("Failed to save settings:", e);
    }
  }

  async function handleFetch() {
    if (!url.trim()) return;

    setAppState("fetching");
    setError("");
    setMetadata(null);

    try {
      const meta = await invoke<VideoMetadata>("fetch_metadata", { url: url.trim() });
      setMetadata(meta);
      setTitle(meta.title);
      setArtist(meta.artist);
      setAppState("ready");
    } catch (e) {
      setError(String(e));
      setAppState("error");
    }
  }

  async function handleDownload() {
    if (!metadata || !settings) return;

    setAppState("downloading");
    setError("");
    setProgress(null);

    try {
      let result: DownloadResult;

      if (trimEnabled && metadata.duration) {
        // Use trimmed download if trim is enabled
        result = await invoke<DownloadResult>("download_audio_trimmed", {
          url: url.trim(),
          title,
          artist,
          outputDir: settings.download_dir,
          thumbnailUrl: metadata?.thumbnail || null,
          startTime: trimStart,
          endTime: trimEnd,
        });
      } else {
        // Full download
        result = await invoke<DownloadResult>("download_audio", {
          url: url.trim(),
          title,
          artist,
          outputDir: settings.download_dir,
          thumbnailUrl: metadata?.thumbnail || null,
        });
      }

      setDownloadedPath(result.path);
      setAppState("complete");
      setProgress(null);
    } catch (e) {
      setError(String(e));
      setAppState("error");
      setProgress(null);
    }
  }

  async function handleCancel() {
    try {
      await invoke("cancel_download");
      setAppState("ready");
      setProgress(null);
    } catch (e) {
      console.error("Failed to cancel:", e);
    }
  }

  async function handleSelectFolder() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        defaultPath: settings?.download_dir,
      });

      if (selected && settings) {
        const newSettings = { ...settings, download_dir: selected as string };
        await saveSettings(newSettings);
      }
    } catch (e) {
      console.error("Failed to select folder:", e);
    }
  }

  async function handleOpenFolder() {
    if (downloadedPath) {
      try {
        await revealItemInDir(downloadedPath);
      } catch (e) {
        console.error("Failed to open folder:", e);
      }
    }
  }

  function handleReset() {
    setUrl("");
    setMetadata(null);
    setTitle("");
    setArtist("");
    setAppState("idle");
    setError("");
    setDownloadedPath("");
  }

  function formatDuration(seconds: number | null): string {
    if (!seconds) return "";
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, "0")}`;
  }

  return (
    <main className="container">
      <header className="header">
        <h1>Rippr</h1>
        <div className="header-actions">
          <button
            className={`queue-btn ${showQueue ? "active" : ""}`}
            onClick={() => {
              setShowQueue(!showQueue);
              if (!showQueue) {
                setShowSettings(false);
                setShowHistory(false);
              }
            }}
          >
            Queue
          </button>
          <button
            className={`history-btn ${showHistory ? "active" : ""}`}
            onClick={() => {
              setShowHistory(!showHistory);
              if (!showHistory) {
                setShowSettings(false);
                setShowQueue(false);
              }
            }}
          >
            History
          </button>
          <button
            className="settings-btn"
            onClick={() => {
              setShowSettings(!showSettings);
              if (!showSettings) {
                setShowQueue(false);
                setShowHistory(false);
              }
            }}
          >
            {showSettings ? "Close" : "Settings"}
          </button>
        </div>
      </header>

      {showSettings && settings && (
        <div className="settings-panel">
          <h2>Settings</h2>
          <div className="setting-row">
            <label>Download Folder:</label>
            <div className="folder-picker">
              <input type="text" value={settings.download_dir} readOnly />
              <button onClick={handleSelectFolder}>Browse</button>
            </div>
          </div>
          <div className="setting-row">
            <label>yt-dlp Version:</label>
            <div className="ytdlp-version">
              <span className="version-number">
                {ytdlpVersion || "Loading..."}
              </span>
              {ytdlpUpdateAvailable && (
                <span className="update-available">
                  Update available: {ytdlpUpdateAvailable}
                </span>
              )}
              <button
                className="update-btn"
                onClick={handleUpdateYtdlp}
                disabled={isUpdatingYtdlp || !ytdlpUpdateAvailable}
              >
                {isUpdatingYtdlp ? "Updating..." : ytdlpUpdateAvailable ? "Update" : "Up to date"}
              </button>
            </div>
          </div>
        </div>
      )}

      {showQueue && settings && (
        <DownloadQueue
          downloadDir={settings.download_dir}
          onClose={() => setShowQueue(false)}
        />
      )}

      {showHistory && (
        <History onClose={() => setShowHistory(false)} />
      )}

      <div
        className={`url-section ${isDragOver ? "drag-over" : ""}`}
        onDrop={(e) => {
          e.preventDefault();
          setIsDragOver(false);
          if (appState === "fetching" || appState === "downloading") return;
          const droppedUrl = e.dataTransfer.getData("text/plain");
          if (droppedUrl && (droppedUrl.startsWith("http://") || droppedUrl.startsWith("https://"))) {
            setUrl(droppedUrl);
            // Auto-fetch after a brief delay to let state update
            setTimeout(() => {
              const fetchBtn = document.querySelector(".url-section button") as HTMLButtonElement;
              if (fetchBtn && !fetchBtn.disabled) {
                fetchBtn.click();
              }
            }, 100);
          }
        }}
        onDragOver={(e) => {
          e.preventDefault();
          if (appState !== "fetching" && appState !== "downloading") {
            setIsDragOver(true);
          }
        }}
        onDragLeave={() => setIsDragOver(false)}
      >
        <input
          type="text"
          placeholder="Paste or drag URL (YouTube, SoundCloud, Bandcamp, etc.)..."
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleFetch()}
          disabled={appState === "fetching" || appState === "downloading"}
        />
        <button
          onClick={handleFetch}
          disabled={!url.trim() || appState === "fetching" || appState === "downloading"}
        >
          {appState === "fetching" ? "Loading..." : "Fetch"}
        </button>
      </div>

      {appState === "fetching" && (
        <div className="status loading">Fetching audio information...</div>
      )}

      {(appState === "ready" || appState === "downloading") && metadata && (
        <div className="metadata-panel">
          {metadata.thumbnail && (
            <img src={metadata.thumbnail} alt="Thumbnail" className="thumbnail" />
          )}
          <div className="metadata-fields">
            <div className="field">
              <label>Title:</label>
              <input
                type="text"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                disabled={appState === "downloading"}
              />
            </div>
            <div className="field">
              <label>Artist:</label>
              <input
                type="text"
                value={artist}
                onChange={(e) => setArtist(e.target.value)}
                disabled={appState === "downloading"}
              />
            </div>
            {metadata.duration && (
              <div className="duration-row">
                <span className="duration">Duration: {formatDuration(metadata.duration)}</span>
                <label className="trim-toggle">
                  <input
                    type="checkbox"
                    checked={trimEnabled}
                    onChange={(e) => setTrimEnabled(e.target.checked)}
                    disabled={appState === "downloading"}
                  />
                  <span>Trim</span>
                </label>
              </div>
            )}
            {trimEnabled && metadata.duration && (
              <div className="trim-section">
                <TrimWaveform
                  duration={metadata.duration}
                  width={340}
                  height={60}
                  onTrimChange={handleTrimChange}
                  initialStart={trimStart}
                  initialEnd={trimEnd}
                />
              </div>
            )}
            {appState === "downloading" && progress ? (
              <div className="progress-section">
                <div className="progress-bar-container">
                  <div
                    className="progress-bar-fill"
                    style={{ width: `${progress.percent}%` }}
                  />
                </div>
                <div className="progress-info">
                  <span className="progress-stage">{progress.stage}</span>
                  <span className="progress-percent">{progress.percent}%</span>
                </div>
                <p className="progress-message">{progress.message}</p>
                <button className="cancel-btn" onClick={handleCancel}>
                  Cancel
                </button>
              </div>
            ) : (
              <button
                className="download-btn"
                onClick={handleDownload}
                disabled={appState === "downloading"}
              >
                {trimEnabled ? "Download Trimmed MP3" : "Download MP3"}
              </button>
            )}
          </div>
        </div>
      )}

      {appState === "complete" && (
        <div className="complete-panel">
          <div className="success-icon">✓</div>
          <p>Download complete!</p>
          <p className="path">{downloadedPath}</p>
          <div className="complete-actions">
            <button onClick={handleOpenFolder}>Open Folder</button>
            <button onClick={handleReset}>Download Another</button>
          </div>
        </div>
      )}

      {appState === "error" && (
        <div className="error-panel">
          <p className="error-message">{error}</p>
          <button onClick={handleReset}>Try Again</button>
        </div>
      )}

      <footer className="footer">
        {settings && <span>Saving to: {settings.download_dir}</span>}
      </footer>
    </main>
  );
}

export default App;
