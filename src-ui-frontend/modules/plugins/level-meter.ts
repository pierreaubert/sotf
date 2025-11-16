// Level Meter Component
// Reusable vertical level meter with peak hold

import type { LevelMeterData } from "./plugin-types";

export interface LevelMeterConfig {
  canvas: HTMLCanvasElement;
  channels: number; // Number of channels to display
  channelLabels?: string[]; // Optional labels for each channel
  minDb?: number; // Minimum dB value (default: -60)
  maxDb?: number; // Maximum dB value (default: 0)
  peakHoldTime?: number; // Peak hold time in ms (default: 1000)
  colorScheme?: "light" | "dark"; // Color scheme (default: 'dark')
}

/**
 * Vertical level meter component
 * Displays RMS and peak levels for multiple channels
 */
export class LevelMeter {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D | null;
  private config: Required<LevelMeterConfig>;

  // State
  private currentLevels: number[] = [];
  private currentPeaks: number[] = [];
  private peakHolds: number[] = [];
  private peakHoldTimers: number[] = [];

  // Animation
  private animationFrameId: number | null = null;

  constructor(config: LevelMeterConfig) {
    this.canvas = config.canvas;
    this.ctx = this.canvas.getContext("2d");

    // Initialize config with defaults
    this.config = {
      canvas: config.canvas,
      channels: config.channels,
      channelLabels:
        config.channelLabels ??
        Array.from({ length: config.channels }, (_, i) => `${i + 1}`),
      minDb: config.minDb ?? -60,
      maxDb: config.maxDb ?? 0,
      peakHoldTime: config.peakHoldTime ?? 1000,
      colorScheme: config.colorScheme ?? "dark",
    };

    // Initialize arrays
    this.currentLevels = new Array(this.config.channels).fill(
      this.config.minDb,
    );
    this.currentPeaks = new Array(this.config.channels).fill(this.config.minDb);
    this.peakHolds = new Array(this.config.channels).fill(this.config.minDb);
    this.peakHoldTimers = new Array(this.config.channels).fill(0);

    if (!this.ctx) {
      console.warn("[LevelMeter] Failed to get 2D context");
      return;
    }

    this.setupCanvas();
    this.render();
  }

  /**
   * Setup canvas size and DPI scaling
   */
  private setupCanvas(): void {
    if (!this.ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const rect = this.canvas.getBoundingClientRect();

    const width = rect.width > 0 ? rect.width : this.canvas.width;
    const height = rect.height > 0 ? rect.height : this.canvas.height;

    this.canvas.width = width * dpr;
    this.canvas.height = height * dpr;

    this.ctx = this.canvas.getContext("2d");
    if (this.ctx) {
      this.ctx.scale(dpr, dpr);
    }
  }

  /**
   * Update meter with new data
   */
  update(data: LevelMeterData): void {
    const now = Date.now();

    for (
      let i = 0;
      i < Math.min(data.channels.length, this.config.channels);
      i++
    ) {
      // Update RMS level
      this.currentLevels[i] = data.channels[i];

      // Update peak
      if (data.peaks && data.peaks[i] !== undefined) {
        this.currentPeaks[i] = data.peaks[i];

        // Update peak hold
        if (data.peaks[i] > this.peakHolds[i]) {
          this.peakHolds[i] = data.peaks[i];
          this.peakHoldTimers[i] = now;
        }
      }
    }

    // Decay peak holds
    for (let i = 0; i < this.config.channels; i++) {
      if (now - this.peakHoldTimers[i] > this.config.peakHoldTime) {
        // Slowly decay peak hold
        this.peakHolds[i] = Math.max(
          this.config.minDb,
          this.peakHolds[i] - 0.5,
        );
      }
    }

    // Render on next animation frame
    if (!this.animationFrameId) {
      this.animationFrameId = requestAnimationFrame(() => this.renderFrame());
    }
  }

  /**
   * Render a single frame
   */
  private renderFrame(): void {
    this.render();
    this.animationFrameId = null;
  }

  /**
   * Render the meter
   */
  private render(): void {
    if (!this.ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const width = this.canvas.width / dpr;
    const height = this.canvas.height / dpr;

    // Clear canvas
    const bgColor = this.config.colorScheme === "dark" ? "#1a1a1a" : "#f8f9fa";
    this.ctx.fillStyle = bgColor;
    this.ctx.fillRect(0, 0, width, height);

    // Calculate layout
    const labelHeight = 20;
    const meterHeight = height - labelHeight;
    const meterWidth = Math.max(10, (width - 10) / this.config.channels - 5);
    const spacing =
      (width - meterWidth * this.config.channels) / (this.config.channels + 1);

    // Draw each channel
    for (let i = 0; i < this.config.channels; i++) {
      const x = spacing + i * (meterWidth + spacing);
      this.drawChannel(x, 0, meterWidth, meterHeight, i);
      this.drawLabel(x, meterHeight, meterWidth, labelHeight, i);
    }
  }

  /**
   * Draw a single channel meter
   */
  private drawChannel(
    x: number,
    y: number,
    width: number,
    height: number,
    channelIndex: number,
  ): void {
    if (!this.ctx) return;

    // Draw background
    this.ctx.fillStyle =
      this.config.colorScheme === "dark" ? "#2a2a2a" : "#e0e0e0";
    this.ctx.fillRect(x, y, width, height);

    // Draw scale markers
    this.drawScale(x, y, width, height);

    // Get levels
    const rmsLevel = this.currentLevels[channelIndex];
    const peakHold = this.peakHolds[channelIndex];

    // Calculate heights
    const rmsHeight = this.dbToHeight(rmsLevel, height);
    const peakHeight = this.dbToHeight(peakHold, height);

    // Draw RMS bar (gradient)
    const gradient = this.ctx.createLinearGradient(x, y + height, x, y);
    gradient.addColorStop(0, this.getLevelColor(this.config.minDb));
    gradient.addColorStop(0.6, this.getLevelColor(-20));
    gradient.addColorStop(0.8, this.getLevelColor(-6));
    gradient.addColorStop(1, this.getLevelColor(0));

    this.ctx.fillStyle = gradient;
    this.ctx.fillRect(x, y + height - rmsHeight, width, rmsHeight);

    // Draw peak hold line
    if (peakHold > this.config.minDb) {
      this.ctx.fillStyle = this.getLevelColor(peakHold);
      this.ctx.fillRect(x, y + height - peakHeight - 2, width, 2);
    }
  }

  /**
   * Draw scale markers
   */
  private drawScale(x: number, y: number, width: number, height: number): void {
    if (!this.ctx) return;

    const markers = [0, -6, -12, -18, -24, -30, -40, -50];
    const lineColor =
      this.config.colorScheme === "dark" ? "#404040" : "#b0b0b0";

    this.ctx.strokeStyle = lineColor;
    this.ctx.lineWidth = 1;

    for (const db of markers) {
      if (db >= this.config.minDb && db <= this.config.maxDb) {
        const markerY = y + height - this.dbToHeight(db, height);
        this.ctx.beginPath();
        this.ctx.moveTo(x, markerY);
        this.ctx.lineTo(x + width, markerY);
        this.ctx.stroke();
      }
    }
  }

  /**
   * Draw channel label
   */
  private drawLabel(
    x: number,
    y: number,
    width: number,
    height: number,
    channelIndex: number,
  ): void {
    if (!this.ctx) return;

    const label =
      this.config.channelLabels[channelIndex] || `${channelIndex + 1}`;
    const textColor =
      this.config.colorScheme === "dark" ? "#ffffff" : "#000000";

    this.ctx.fillStyle = textColor;
    this.ctx.font = "11px sans-serif";
    this.ctx.textAlign = "center";
    this.ctx.textBaseline = "middle";
    this.ctx.fillText(label, x + width / 2, y + height / 2);
  }

  /**
   * Convert dB to pixel height
   */
  private dbToHeight(db: number, totalHeight: number): number {
    const clamped = Math.max(
      this.config.minDb,
      Math.min(this.config.maxDb, db),
    );
    const normalized =
      (clamped - this.config.minDb) / (this.config.maxDb - this.config.minDb);
    return normalized * totalHeight;
  }

  /**
   * Get color for dB level
   */
  private getLevelColor(db: number): string {
    if (db > -3) {
      return "#ef4444"; // Red (clipping)
    } else if (db > -6) {
      return "#f59e0b"; // Orange (hot)
    } else if (db > -12) {
      return "#eab308"; // Yellow (warm)
    } else if (db > -24) {
      return "#84cc16"; // Green (good)
    } else {
      return "#22c55e"; // Light green (quiet)
    }
  }

  /**
   * Resize the meter
   */
  resize(): void {
    this.setupCanvas();
    this.render();
  }

  /**
   * Reconfigure channel count and labels
   */
  reconfigure(channels: number, channelLabels?: string[]): void {
    this.config.channels = channels;
    this.config.channelLabels =
      channelLabels ?? Array.from({ length: channels }, (_, i) => `${i + 1}`);

    // Reinitialize arrays
    this.currentLevels = new Array(channels).fill(this.config.minDb);
    this.currentPeaks = new Array(channels).fill(this.config.minDb);
    this.peakHolds = new Array(channels).fill(this.config.minDb);
    this.peakHoldTimers = new Array(channels).fill(0);

    console.log(
      "[LevelMeter] Reconfigured to",
      channels,
      "channels:",
      channelLabels,
    );

    // Trigger re-render
    this.render();
  }

  /**
   * Cleanup
   */
  destroy(): void {
    if (this.animationFrameId !== null) {
      cancelAnimationFrame(this.animationFrameId);
      this.animationFrameId = null;
    }
  }
}
