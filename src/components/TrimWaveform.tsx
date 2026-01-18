import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./TrimWaveform.css";

interface WaveformPoint {
  min: number;
  max: number;
}

interface TrimWaveformProps {
  filePath?: string;
  duration: number; // Total duration in seconds
  width?: number;
  height?: number;
  onTrimChange?: (startTime: number, endTime: number) => void;
  initialStart?: number;
  initialEnd?: number;
}

export function TrimWaveform({
  filePath,
  duration,
  width = 400,
  height = 80,
  onTrimChange,
  initialStart = 0,
  initialEnd,
}: TrimWaveformProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [waveformData, setWaveformData] = useState<WaveformPoint[] | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [startTime, setStartTime] = useState(initialStart);
  const [endTime, setEndTime] = useState(initialEnd ?? duration);
  const [dragging, setDragging] = useState<"start" | "end" | null>(null);

  // Update end time when duration changes
  useEffect(() => {
    if (initialEnd === undefined) {
      setEndTime(duration);
    }
  }, [duration, initialEnd]);

  // Load waveform data if file path provided
  useEffect(() => {
    async function loadWaveform() {
      if (!filePath) {
        setWaveformData(null);
        return;
      }
      setIsLoading(true);
      try {
        const numPoints = Math.max(200, width);
        const data = await invoke<WaveformPoint[]>("generate_waveform", {
          filePath,
          numPoints,
        });
        setWaveformData(data);
      } catch (e) {
        console.error("Failed to load waveform:", e);
        setWaveformData(null);
      }
      setIsLoading(false);
    }

    loadWaveform();
  }, [filePath, width]);

  // Notify parent of trim changes
  useEffect(() => {
    onTrimChange?.(startTime, endTime);
  }, [startTime, endTime, onTrimChange]);

  // Draw waveform with trim region
  useEffect(() => {
    if (!canvasRef.current) return;

    const canvas = canvasRef.current;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    ctx.scale(dpr, dpr);

    ctx.clearRect(0, 0, width, height);

    // Get colors
    const computedStyle = getComputedStyle(canvas);
    const waveColor = computedStyle.getPropertyValue("--waveform-color").trim() || "#3b82f6";
    const dimColor = computedStyle.getPropertyValue("--waveform-dim").trim() || "#cbd5e1";
    const bgColor = computedStyle.getPropertyValue("--waveform-bg").trim() || "#f5f5f5";

    // Fill background
    ctx.fillStyle = bgColor;
    ctx.fillRect(0, 0, width, height);

    // Calculate trim positions
    const startX = (startTime / duration) * width;
    const endX = (endTime / duration) * width;

    // Draw dimmed regions outside trim
    ctx.fillStyle = "rgba(0, 0, 0, 0.15)";
    ctx.fillRect(0, 0, startX, height);
    ctx.fillRect(endX, 0, width - endX, height);

    // Draw waveform
    const centerY = height / 2;

    if (waveformData && waveformData.length > 0) {
      const barWidth = width / waveformData.length;

      for (let i = 0; i < waveformData.length; i++) {
        const point = waveformData[i];
        const x = i * barWidth;

        // Use full color for selected region, dim for outside
        const isInSelection = x >= startX && x <= endX;
        ctx.fillStyle = isInSelection ? waveColor : dimColor;

        const minY = centerY - point.min * centerY * 0.9;
        const maxY = centerY - point.max * centerY * 0.9;
        const barHeight = Math.max(1, minY - maxY);

        ctx.fillRect(x, maxY, Math.max(1, barWidth - 0.5), barHeight);
      }
    } else {
      // Draw placeholder timeline when no waveform data
      // Draw a simple bar representation
      const numBars = 50;
      const barWidth = width / numBars;
      const maxBarHeight = height * 0.6;

      for (let i = 0; i < numBars; i++) {
        const x = i * barWidth;
        const isInSelection = x >= startX && x <= endX;

        // Create a simple pattern for visual feedback
        const barHeight = maxBarHeight * (0.3 + 0.4 * Math.sin(i * 0.3) + 0.3 * Math.cos(i * 0.5));
        const y = centerY - barHeight / 2;

        ctx.fillStyle = isInSelection ? waveColor : dimColor;
        ctx.fillRect(x + 1, y, barWidth - 2, barHeight);
      }
    }

    // Draw trim handles
    ctx.fillStyle = "#2563eb";
    // Start handle
    ctx.fillRect(startX - 2, 0, 4, height);
    // End handle
    ctx.fillRect(endX - 2, 0, 4, height);

  }, [waveformData, width, height, duration, startTime, endTime]);

  // Handle mouse/touch interactions
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (!containerRef.current) return;
    const rect = containerRef.current.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const time = (x / width) * duration;

    const startX = (startTime / duration) * width;
    const endX = (endTime / duration) * width;

    // Determine if clicking near a handle (within 10px)
    if (Math.abs(x - startX) < 10) {
      setDragging("start");
    } else if (Math.abs(x - endX) < 10) {
      setDragging("end");
    } else {
      // Click in the middle - move the nearest handle
      if (Math.abs(time - startTime) < Math.abs(time - endTime)) {
        setStartTime(Math.max(0, Math.min(time, endTime - 0.5)));
        setDragging("start");
      } else {
        setEndTime(Math.max(startTime + 0.5, Math.min(time, duration)));
        setDragging("end");
      }
    }
  }, [width, duration, startTime, endTime]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (!dragging || !containerRef.current) return;
    const rect = containerRef.current.getBoundingClientRect();
    const x = Math.max(0, Math.min(e.clientX - rect.left, width));
    const time = (x / width) * duration;

    if (dragging === "start") {
      setStartTime(Math.max(0, Math.min(time, endTime - 0.5)));
    } else {
      setEndTime(Math.max(startTime + 0.5, Math.min(time, duration)));
    }
  }, [dragging, width, duration, startTime, endTime]);

  const handleMouseUp = useCallback(() => {
    setDragging(null);
  }, []);

  // Global mouse up listener
  useEffect(() => {
    if (dragging) {
      const handleGlobalMouseUp = () => setDragging(null);
      window.addEventListener("mouseup", handleGlobalMouseUp);
      return () => window.removeEventListener("mouseup", handleGlobalMouseUp);
    }
  }, [dragging]);

  function formatTime(seconds: number): string {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    const ms = Math.floor((seconds % 1) * 10);
    return `${mins}:${secs.toString().padStart(2, "0")}.${ms}`;
  }

  if (isLoading) {
    return (
      <div className="trim-waveform-container" style={{ width }}>
        <div className="trim-waveform-loading">Loading waveform...</div>
      </div>
    );
  }

  const selectedDuration = endTime - startTime;

  return (
    <div className="trim-waveform-container" style={{ width }}>
      <div
        ref={containerRef}
        className="trim-waveform-canvas-container"
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        style={{ cursor: dragging ? "grabbing" : "pointer" }}
      >
        <canvas
          ref={canvasRef}
          className="trim-waveform-canvas"
          style={{ width, height }}
        />
      </div>
      <div className="trim-controls">
        <div className="trim-times">
          <div className="trim-time-input">
            <label>Start</label>
            <input
              type="number"
              min={0}
              max={endTime - 0.5}
              step={0.1}
              value={startTime.toFixed(1)}
              onChange={(e) => {
                const val = parseFloat(e.target.value);
                if (!isNaN(val)) setStartTime(Math.max(0, Math.min(val, endTime - 0.5)));
              }}
            />
            <span className="trim-time-display">{formatTime(startTime)}</span>
          </div>
          <div className="trim-duration">
            {formatTime(selectedDuration)}
          </div>
          <div className="trim-time-input">
            <label>End</label>
            <input
              type="number"
              min={startTime + 0.5}
              max={duration}
              step={0.1}
              value={endTime.toFixed(1)}
              onChange={(e) => {
                const val = parseFloat(e.target.value);
                if (!isNaN(val)) setEndTime(Math.max(startTime + 0.5, Math.min(val, duration)));
              }}
            />
            <span className="trim-time-display">{formatTime(endTime)}</span>
          </div>
        </div>
      </div>
    </div>
  );
}
