import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { Waveform } from "./Waveform";
import "./History.css";

interface HistoryEntry {
  id: number;
  url: string;
  title: string;
  artist: string;
  thumbnail: string | null;
  duration: number | null;
  outputPath: string;
  downloadedAt: string;
}

interface AudioAnalysis {
  bpm: number | null;
  bpmConfidence: number | null;
  key: string | null;
  keyConfidence: number | null;
}

interface HistoryProps {
  onClose: () => void;
}

export function History({ onClose }: HistoryProps) {
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [isLoading, setIsLoading] = useState(true);
  const [analysisResults, setAnalysisResults] = useState<Record<number, AudioAnalysis>>({});
  const [analyzingIds, setAnalyzingIds] = useState<Set<number>>(new Set());
  const [expandedWaveforms, setExpandedWaveforms] = useState<Set<number>>(new Set());

  useEffect(() => {
    loadHistory();
  }, []);

  async function loadHistory() {
    setIsLoading(true);
    try {
      const entries = await invoke<HistoryEntry[]>("get_download_history", { limit: 100 });
      setHistory(entries);
    } catch (e) {
      console.error("Failed to load history:", e);
    }
    setIsLoading(false);
  }

  async function handleSearch() {
    if (!searchQuery.trim()) {
      loadHistory();
      return;
    }

    setIsLoading(true);
    try {
      const entries = await invoke<HistoryEntry[]>("search_download_history", {
        query: searchQuery,
        limit: 100,
      });
      setHistory(entries);
    } catch (e) {
      console.error("Failed to search history:", e);
    }
    setIsLoading(false);
  }

  async function handleDelete(id: number) {
    try {
      await invoke("delete_history_entry", { id });
      setHistory(history.filter((h) => h.id !== id));
    } catch (e) {
      console.error("Failed to delete entry:", e);
    }
  }

  async function handleClearAll() {
    if (!window.confirm("Are you sure you want to clear all download history?")) {
      return;
    }

    try {
      await invoke("clear_download_history");
      setHistory([]);
    } catch (e) {
      console.error("Failed to clear history:", e);
    }
  }

  async function handleReveal(path: string) {
    try {
      await revealItemInDir(path);
    } catch (e) {
      console.error("Failed to reveal file:", e);
    }
  }

  async function handleAnalyze(entry: HistoryEntry) {
    if (analyzingIds.has(entry.id)) return;

    setAnalyzingIds((prev) => new Set([...prev, entry.id]));

    try {
      const result = await invoke<AudioAnalysis>("analyze_audio_file", {
        filePath: entry.outputPath,
      });
      setAnalysisResults((prev) => ({ ...prev, [entry.id]: result }));
    } catch (e) {
      console.error("Failed to analyze:", e);
      // Set error state
      setAnalysisResults((prev) => ({
        ...prev,
        [entry.id]: { bpm: null, bpmConfidence: null, key: null, keyConfidence: null },
      }));
    }

    setAnalyzingIds((prev) => {
      const next = new Set(prev);
      next.delete(entry.id);
      return next;
    });
  }

  function toggleWaveform(id: number) {
    setExpandedWaveforms((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }

  function formatDate(dateStr: string): string {
    const date = new Date(dateStr + "Z"); // Treat as UTC
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

    if (diffDays === 0) {
      return "Today";
    } else if (diffDays === 1) {
      return "Yesterday";
    } else if (diffDays < 7) {
      return `${diffDays} days ago`;
    } else {
      return date.toLocaleDateString();
    }
  }

  function formatDuration(seconds: number | null): string {
    if (!seconds) return "";
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, "0")}`;
  }

  return (
    <div className="history-panel">
      <div className="history-header">
        <h2>Download History</h2>
        <button className="close-btn" onClick={onClose}>
          ×
        </button>
      </div>

      <div className="history-search">
        <input
          type="text"
          placeholder="Search by title or artist..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleSearch()}
        />
        <button onClick={handleSearch}>Search</button>
        {searchQuery && (
          <button
            className="clear-search"
            onClick={() => {
              setSearchQuery("");
              loadHistory();
            }}
          >
            Clear
          </button>
        )}
      </div>

      {history.length > 0 && (
        <div className="history-actions">
          <span className="history-count">{history.length} items</span>
          <button className="clear-all-btn" onClick={handleClearAll}>
            Clear All
          </button>
        </div>
      )}

      {isLoading ? (
        <div className="history-loading">Loading...</div>
      ) : history.length > 0 ? (
        <div className="history-list">
          {history.map((entry) => {
            const analysis = analysisResults[entry.id];
            const isAnalyzing = analyzingIds.has(entry.id);
            const showWaveform = expandedWaveforms.has(entry.id);

            return (
              <div key={entry.id} className="history-item">
                {entry.thumbnail && (
                  <img src={entry.thumbnail} alt="" className="history-thumbnail" />
                )}
                <div className="history-item-info">
                  <div className="history-item-title">{entry.title}</div>
                  <div className="history-item-artist">{entry.artist}</div>
                  <div className="history-item-meta">
                    <span className="history-date">{formatDate(entry.downloadedAt)}</span>
                    {entry.duration && (
                      <span className="history-duration">{formatDuration(entry.duration)}</span>
                    )}
                  </div>
                  {analysis && (
                    <div className="history-analysis">
                      {analysis.bpm && (
                        <span className="analysis-bpm" title={`Confidence: ${Math.round(analysis.bpmConfidence || 0)}%`}>
                          {analysis.bpm} BPM
                        </span>
                      )}
                      {analysis.key && (
                        <span className="analysis-key" title={`Confidence: ${Math.round(analysis.keyConfidence || 0)}%`}>
                          {analysis.key}
                        </span>
                      )}
                    </div>
                  )}
                  {showWaveform && (
                    <div className="history-waveform">
                      <Waveform filePath={entry.outputPath} width={280} height={48} />
                    </div>
                  )}
                </div>
                <div className="history-item-actions">
                  <button
                    className={`waveform-btn ${showWaveform ? "active" : ""}`}
                    onClick={() => toggleWaveform(entry.id)}
                    title={showWaveform ? "Hide waveform" : "Show waveform"}
                  >
                    ~
                  </button>
                  {!analysis && (
                    <button
                      className="analyze-btn"
                      onClick={() => handleAnalyze(entry)}
                      disabled={isAnalyzing}
                      title="Analyze BPM & Key"
                    >
                      {isAnalyzing ? "..." : "BPM"}
                    </button>
                  )}
                  <button
                    className="reveal-btn"
                    onClick={() => handleReveal(entry.outputPath)}
                    title="Show in folder"
                  >
                    📂
                  </button>
                  <button
                    className="delete-btn"
                    onClick={() => handleDelete(entry.id)}
                    title="Remove from history"
                  >
                    ×
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      ) : (
        <div className="history-empty">
          <p>No download history</p>
          <p className="hint">Downloaded files will appear here</p>
        </div>
      )}
    </div>
  );
}
