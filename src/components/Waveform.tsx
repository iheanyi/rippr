import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./Waveform.css";

interface WaveformPoint {
  min: number;
  max: number;
}

interface WaveformProps {
  filePath: string;
  width?: number;
  height?: number;
  color?: string;
  backgroundColor?: string;
}

export function Waveform({
  filePath,
  width = 400,
  height = 80,
  color,
  backgroundColor,
}: WaveformProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [waveformData, setWaveformData] = useState<WaveformPoint[] | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function loadWaveform() {
      setIsLoading(true);
      setError(null);
      try {
        // Request slightly more points than width for smoother rendering
        const numPoints = Math.max(200, width);
        const data = await invoke<WaveformPoint[]>("generate_waveform", {
          filePath,
          numPoints,
        });
        setWaveformData(data);
      } catch (e) {
        console.error("Failed to load waveform:", e);
        setError(e instanceof Error ? e.message : String(e));
      }
      setIsLoading(false);
    }

    if (filePath) {
      loadWaveform();
    }
  }, [filePath, width]);

  useEffect(() => {
    if (!canvasRef.current || !waveformData) return;

    const canvas = canvasRef.current;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    // Set actual canvas size (for high DPI displays)
    const dpr = window.devicePixelRatio || 1;
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    ctx.scale(dpr, dpr);

    // Clear canvas
    ctx.clearRect(0, 0, width, height);

    // Get colors from CSS variables or use defaults
    const computedStyle = getComputedStyle(canvas);
    const waveColor = color || computedStyle.getPropertyValue("--waveform-color").trim() || "#3b82f6";
    const bgColor = backgroundColor || computedStyle.getPropertyValue("--waveform-bg").trim() || "transparent";

    // Fill background
    if (bgColor !== "transparent") {
      ctx.fillStyle = bgColor;
      ctx.fillRect(0, 0, width, height);
    }

    // Draw waveform
    const barWidth = width / waveformData.length;
    const centerY = height / 2;

    ctx.fillStyle = waveColor;

    for (let i = 0; i < waveformData.length; i++) {
      const point = waveformData[i];
      const x = i * barWidth;

      // Scale the min/max values to fit the canvas height
      // Samples are typically -1 to 1, but we'll normalize based on actual data
      const minY = centerY - point.min * centerY * 0.9;
      const maxY = centerY - point.max * centerY * 0.9;

      const barHeight = Math.max(1, minY - maxY);

      ctx.fillRect(x, maxY, Math.max(1, barWidth - 0.5), barHeight);
    }
  }, [waveformData, width, height, color, backgroundColor]);

  if (isLoading) {
    return (
      <div className="waveform-container" style={{ width, height }}>
        <div className="waveform-loading">Loading waveform...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="waveform-container" style={{ width, height }}>
        <div className="waveform-error">Failed to load waveform</div>
      </div>
    );
  }

  return (
    <div className="waveform-container" style={{ width, height }}>
      <canvas
        ref={canvasRef}
        className="waveform-canvas"
        style={{ width, height }}
      />
    </div>
  );
}
