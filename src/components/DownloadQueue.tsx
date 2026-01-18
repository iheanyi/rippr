import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import "./DownloadQueue.css";

type QueueStatus = "pending" | "fetching" | "ready" | "downloading" | "complete" | "failed";

interface QueueItem {
  id: string;
  url: string;
  title: string | null;
  artist: string | null;
  thumbnail: string | null;
  duration: number | null;
  status: QueueStatus;
  progress: number;
  error: string | null;
  outputPath: string | null;
}

interface DownloadQueueProps {
  downloadDir: string;
  onClose: () => void;
}

export function DownloadQueue({ downloadDir, onClose }: DownloadQueueProps) {
  const [queue, setQueue] = useState<QueueItem[]>([]);
  const [urls, setUrls] = useState("");
  const [isProcessing, setIsProcessing] = useState(false);

  // Load initial queue
  useEffect(() => {
    loadQueue();
  }, []);

  // Listen for queue updates
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    const setupListener = async () => {
      unlisten = await listen<QueueItem[]>("queue-updated", (event) => {
        setQueue(event.payload);
      });
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  async function loadQueue() {
    try {
      const items = await invoke<QueueItem[]>("get_queue");
      setQueue(items);
    } catch (e) {
      console.error("Failed to load queue:", e);
    }
  }

  async function handleAddUrls() {
    const urlList = urls
      .split("\n")
      .map((u) => u.trim())
      .filter((u) => u.length > 0);

    if (urlList.length === 0) return;

    try {
      await invoke("add_urls_to_queue", { urls: urlList });
      setUrls("");
    } catch (e) {
      console.error("Failed to add URLs:", e);
    }
  }

  async function handleRemove(id: string) {
    try {
      await invoke("remove_from_queue", { id });
    } catch (e) {
      console.error("Failed to remove item:", e);
    }
  }

  async function handleClearCompleted() {
    try {
      await invoke("clear_completed");
    } catch (e) {
      console.error("Failed to clear completed:", e);
    }
  }

  async function handleDownloadAll() {
    setIsProcessing(true);

    // Get pending items
    const pendingItems = queue.filter(
      (item) => item.status === "pending" || item.status === "ready"
    );

    // Process each item sequentially
    for (const item of pendingItems) {
      try {
        await invoke("process_queue_item", {
          id: item.id,
          outputDir: downloadDir,
        });
      } catch (e) {
        console.error(`Failed to download ${item.id}:`, e);
        // Continue with next item
      }
    }

    setIsProcessing(false);
  }

  async function handleDownloadSingle(id: string) {
    try {
      await invoke("process_queue_item", {
        id,
        outputDir: downloadDir,
      });
    } catch (e) {
      console.error(`Failed to download ${id}:`, e);
    }
  }

  function formatDuration(seconds: number | null): string {
    if (!seconds) return "";
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, "0")}`;
  }

  function getStatusIcon(status: QueueStatus): string {
    switch (status) {
      case "pending":
        return "⏳";
      case "fetching":
        return "🔍";
      case "ready":
        return "✓";
      case "downloading":
        return "⬇️";
      case "complete":
        return "✅";
      case "failed":
        return "❌";
      default:
        return "•";
    }
  }

  function getStatusLabel(status: QueueStatus): string {
    switch (status) {
      case "pending":
        return "Pending";
      case "fetching":
        return "Fetching info...";
      case "ready":
        return "Ready";
      case "downloading":
        return "Downloading";
      case "complete":
        return "Complete";
      case "failed":
        return "Failed";
      default:
        return status;
    }
  }

  const pendingCount = queue.filter(
    (i) => i.status === "pending" || i.status === "ready"
  ).length;
  const completedCount = queue.filter((i) => i.status === "complete").length;
  const failedCount = queue.filter((i) => i.status === "failed").length;

  return (
    <div className="queue-panel">
      <div className="queue-header">
        <h2>Download Queue</h2>
        <button className="close-btn" onClick={onClose}>
          ×
        </button>
      </div>

      <div className="queue-add-section">
        <textarea
          placeholder="Paste URLs here (one per line)..."
          value={urls}
          onChange={(e) => setUrls(e.target.value)}
          rows={3}
        />
        <button onClick={handleAddUrls} disabled={!urls.trim()}>
          Add to Queue
        </button>
      </div>

      {queue.length > 0 && (
        <>
          <div className="queue-stats">
            <span>{pendingCount} pending</span>
            <span>{completedCount} complete</span>
            {failedCount > 0 && <span className="failed">{failedCount} failed</span>}
          </div>

          <div className="queue-actions">
            <button
              className="download-all-btn"
              onClick={handleDownloadAll}
              disabled={isProcessing || pendingCount === 0}
            >
              {isProcessing ? "Processing..." : `Download All (${pendingCount})`}
            </button>
            {(completedCount > 0 || failedCount > 0) && (
              <button className="clear-btn" onClick={handleClearCompleted}>
                Clear Completed
              </button>
            )}
          </div>

          <div className="queue-list">
            {queue.map((item) => (
              <div key={item.id} className={`queue-item status-${item.status}`}>
                <div className="queue-item-main">
                  {item.thumbnail && (
                    <img src={item.thumbnail} alt="" className="queue-thumbnail" />
                  )}
                  <div className="queue-item-info">
                    <div className="queue-item-title">
                      {item.title || item.url}
                    </div>
                    {item.artist && (
                      <div className="queue-item-artist">{item.artist}</div>
                    )}
                    <div className="queue-item-status">
                      <span className="status-icon">{getStatusIcon(item.status)}</span>
                      <span className="status-label">{getStatusLabel(item.status)}</span>
                      {item.duration && (
                        <span className="duration">{formatDuration(item.duration)}</span>
                      )}
                    </div>
                    {item.error && <div className="queue-item-error">{item.error}</div>}
                  </div>
                </div>

                {item.status === "downloading" && (
                  <div className="queue-progress-bar">
                    <div
                      className="queue-progress-fill"
                      style={{ width: `${item.progress}%` }}
                    />
                  </div>
                )}

                <div className="queue-item-actions">
                  {(item.status === "pending" || item.status === "ready") && (
                    <button
                      className="download-single-btn"
                      onClick={() => handleDownloadSingle(item.id)}
                      disabled={isProcessing}
                      title="Download"
                    >
                      ⬇️
                    </button>
                  )}
                  {item.status !== "downloading" && (
                    <button
                      className="remove-btn"
                      onClick={() => handleRemove(item.id)}
                      title="Remove"
                    >
                      ×
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
        </>
      )}

      {queue.length === 0 && (
        <div className="queue-empty">
          <p>No items in queue</p>
          <p className="hint">Add URLs above to start batch downloading</p>
        </div>
      )}
    </div>
  );
}
