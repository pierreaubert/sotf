import { invoke } from "@tauri-apps/api/core";

/**
 * Spectrum information from the Rust backend
 */
export interface SpectrumInfo {
  /** Frequency bin centers in Hz */
  frequencies: number[];
  /** Magnitude values in dB (relative to full scale) */
  magnitudes: number[];
  /** Peak magnitude across all bins */
  peak_magnitude: number;
}

/**
 * Configuration for spectrum display
 */
export interface SpectrumDisplayConfig {
  /** Canvas element to render to */
  canvas: HTMLCanvasElement;
  /** Polling interval in milliseconds (default: 100ms) */
  pollInterval?: number;
  /** Minimum frequency to display (default: 20 Hz) */
  minFreq?: number;
  /** Maximum frequency to display (default: 20000 Hz) */
  maxFreq?: number;
  /** dB range for display (default: 60 dB) */
  dbRange?: number;
  /** Color scheme: 'light' or 'dark' (default: 'dark') */
  colorScheme?: "light" | "dark";
  /** Show frequency labels (default: true) */
  showLabels?: boolean;
  /** Show grid (default: true) */
  showGrid?: boolean;
}

/**
 * Real-time spectrum analyzer component
 * Displays frequency spectrum from Rust backend
 */
export class SpectrumAnalyzerComponent {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D | null;
  private config: Required<SpectrumDisplayConfig>;
  private pollInterval: number | null = null;
  private isMonitoring = false;
  private currentSpectrum: SpectrumInfo | null = null;
  private animationFrameId: number | null = null;

  constructor(config: SpectrumDisplayConfig) {
    this.canvas = config.canvas;
    this.ctx = this.canvas.getContext("2d");

    // Initialize config regardless of context availability
    this.config = {
      canvas: config.canvas,
      pollInterval: config.pollInterval ?? 100,
      minFreq: config.minFreq ?? 20,
      maxFreq: config.maxFreq ?? 20000,
      dbRange: config.dbRange ?? 60,
      colorScheme: config.colorScheme ?? "dark",
      showLabels: config.showLabels ?? true,
      showGrid: config.showGrid ?? true,
    };

    if (!this.ctx) {
      console.warn("[Spectrum] Failed to get 2D context for canvas");
      return;
    }

    this.setupCanvas();

    // Do initial render to show proper "no data" state
    // Note: This is just a single render, not starting the animation loop
    requestAnimationFrame(() => this.render());
  }

  /**
   * Setup canvas size and DPI scaling
   */
  private setupCanvas(): void {
    const dpr = window.devicePixelRatio || 1;
    const rect = this.canvas.getBoundingClientRect();

    // Use getBoundingClientRect if available, otherwise fall back to canvas attributes
    const width = rect.width > 0 ? rect.width : this.canvas.width;
    const height = rect.height > 0 ? rect.height : this.canvas.height;

    console.log("[Spectrum] Setting up canvas:", {
      rectWidth: rect.width,
      rectHeight: rect.height,
      attrWidth: this.canvas.width,
      attrHeight: this.canvas.height,
      finalWidth: width,
      finalHeight: height,
      dpr,
    });

    this.canvas.width = width * dpr;
    this.canvas.height = height * dpr;

    // Reset context after changing canvas size
    this.ctx = this.canvas.getContext("2d");
    if (this.ctx) {
      this.ctx.scale(dpr, dpr);
    }
  }

  /**
   * Start monitoring spectrum
   */
  async start(): Promise<void> {
    if (this.isMonitoring) {
      return;
    }

    try {
      await invoke("stream_enable_spectrum_monitoring");
      this.isMonitoring = true;
      this.startPolling();
      this.startRendering();
      // Rendering will start when first data is received
    } catch (error) {
      console.error("[Spectrum] Failed to start spectrum monitoring:", error);
      throw error;
    }
  }

  /**
   * Stop monitoring spectrum
   */
  async stop(): Promise<void> {
    if (!this.isMonitoring) return;

    this.isMonitoring = false;
    this.stopPolling();
    this.stopRendering();

    try {
      await invoke("stream_disable_spectrum_monitoring");
    } catch (error) {
      console.error("Failed to stop spectrum monitoring:", error);
    }
  }

  /**
   * Start polling for spectrum data
   */
  private startPolling(): void {
    let pollCount = 0;
    let firstDataReceived = false;
    this.pollInterval = window.setInterval(async () => {
      try {
        const spectrum = await invoke<SpectrumInfo | null>(
          "stream_get_spectrum",
        );
        if (spectrum) {
          this.currentSpectrum = spectrum;

          // Start rendering on first data received
          if (!firstDataReceived) {
            firstDataReceived = true;
            this.startRendering();
          }

          if (pollCount++ % 10 === 0) {
            console.log("[Spectrum] Received data:", {
              frequencies: spectrum.frequencies.length,
              magnitudes: spectrum.magnitudes.length,
              peak: spectrum.peak_magnitude,
            });
          }
        }
      } catch (error) {
        console.error("Failed to get spectrum:", error);
      }
    }, this.config.pollInterval);
  }

  /**
   * Stop polling for spectrum data
   */
  private stopPolling(): void {
    if (this.pollInterval !== null) {
      clearInterval(this.pollInterval);
      this.pollInterval = null;
    }
  }

  /**
   * Start rendering loop
   */
  private startRendering(): void {
    // Prevent duplicate rendering loops
    if (this.animationFrameId !== null) {
      return;
    }

    const render = () => {
      this.render();
      this.animationFrameId = requestAnimationFrame(render);
    };
    this.animationFrameId = requestAnimationFrame(render);
  }

  /**
   * Stop rendering loop
   */
  private stopRendering(): void {
    if (this.animationFrameId !== null) {
      cancelAnimationFrame(this.animationFrameId);
      this.animationFrameId = null;
    }
  }

  /**
   * Render the spectrum to canvas
   */
  private render(): void {
    if (!this.ctx) {
      console.warn("[Spectrum] render() called but no context available");
      return;
    }

    const dpr = window.devicePixelRatio || 1;
    const width = this.canvas.width / dpr;
    const height = this.canvas.height / dpr;

    if (width === 0 || height === 0) {
      console.warn("[Spectrum] Canvas has zero size, skipping render", {
        width,
        height,
        canvasWidth: this.canvas.width,
        canvasHeight: this.canvas.height,
      });
      return;
    }

    // Get background color from CSS variables
    const bgColor = this.getComputedCSSVariable("--bg-secondary");

    // Clear canvas with theme color
    this.ctx.fillStyle = bgColor;
    this.ctx.fillRect(0, 0, width, height);

    if (!this.currentSpectrum || this.currentSpectrum.magnitudes.length === 0) {
      // Show "waiting" message when monitoring but no data yet
      // Show "no data" message when not monitoring
      this.drawNoData(width, height);
      return;
    }

    // Draw grid and labels
    if (this.config.showGrid) {
      this.drawGrid(width, height);
    }

    // Draw spectrum bars
    this.drawSpectrum(width, height);

    // Draw labels
    if (this.config.showLabels) {
      this.drawLabels(width, height);
    }
  }

  /**
   * Draw "no data" message
   */
  private drawNoData(width: number, height: number): void {
    if (!this.ctx) return;
    this.ctx.fillStyle =
      this.config.colorScheme === "dark" ? "#888888" : "#666666";
    this.ctx.font = "14px sans-serif";
    this.ctx.textAlign = "center";
    this.ctx.textBaseline = "middle";
    this.ctx.fillText("Waiting for audio...", width / 2, height / 2);
  }

  /**
   * Draw frequency grid and dB scale
   */
  private drawGrid(width: number, height: number): void {
    if (!this.ctx) return;
    const isDarkMode = this.config.colorScheme === "dark";
    const lineColor = isDarkMode
      ? "rgba(255, 255, 255, 0.8)"
      : "rgba(0, 0, 0, 0.8)";
    const freqMarkers = [
      20, 40, 60, 100, 200, 400, 600, 1000, 2000, 4000, 6000, 10000, 20000,
    ];
    const dashPattern = [1, 2];
    freqMarkers.forEach((freq) => {
      if (!this.ctx) return;
      const x = this.freqToX(freq, width);
      this.ctx.strokeStyle = lineColor;
      this.ctx.lineWidth = 1;
      this.ctx.beginPath();
      this.ctx.setLineDash(dashPattern);
      this.ctx.moveTo(x, 0);
      this.ctx.lineTo(x, height - 20);
      this.ctx.stroke();
    });

    const dbLevels = [0, -10, -20, -30, -40, -50, -60];

    dbLevels.forEach((db) => {
      if (!this.ctx) return;
      const y = this.dbToY(db, height);

      // Set dotted line style - full opacity
      this.ctx.strokeStyle = lineColor;
      this.ctx.lineWidth = 1;
      this.ctx.setLineDash(dashPattern);

      // Draw horizontal line across full width
      this.ctx.beginPath();
      this.ctx.moveTo(20, y);
      this.ctx.lineTo(width - 5, y);
      this.ctx.stroke();
    });

    // Reset line dash to solid
    this.ctx.setLineDash([]);
  }

  /**
   * Convert dB to Y coordinate (inverted because canvas Y increases downward)
   */
  private dbToY(db: number, height: number): number {
    const normalized = (db + this.config.dbRange) / this.config.dbRange;
    return height - 20 - normalized * (height - 20);
  }

  /**
   * Draw spectrum bars
   */
  private drawSpectrum(width: number, height: number): void {
    if (!this.ctx || !this.currentSpectrum) return;

    const spectrum = this.currentSpectrum;

    for (let i = 0; i < spectrum.frequencies.length; i++) {
      if (!this.ctx) return;
      const freq = spectrum.frequencies[i];
      const magnitude = spectrum.magnitudes[i];

      // Skip if frequency is outside display range
      if (freq < this.config.minFreq || freq > this.config.maxFreq) {
        continue;
      }

      // Calculate bin edges (geometric mean boundaries between bins)
      let leftEdge: number, rightEdge: number;

      if (i === 0) {
        // First bin: left edge at minFreq
        leftEdge = this.config.minFreq;
        rightEdge = Math.sqrt(freq * spectrum.frequencies[i + 1]);
      } else if (i === spectrum.frequencies.length - 1) {
        // Last bin: right edge at maxFreq
        leftEdge = Math.sqrt(spectrum.frequencies[i - 1] * freq);
        rightEdge = this.config.maxFreq;
      } else {
        // Middle bins: edges at geometric mean with neighbors
        leftEdge = Math.sqrt(spectrum.frequencies[i - 1] * freq);
        rightEdge = Math.sqrt(freq * spectrum.frequencies[i + 1]);
      }

      // Convert edges to screen coordinates
      const xLeft = this.freqToX(leftEdge, width);
      const xRight = this.freqToX(rightEdge, width);

      // Bar width is the distance between edges (minus small gap for visual separation)
      const barWidth = Math.max(1, xRight - xLeft - 0.5);

      const barHeight = this.dbToHeight(magnitude, height);

      // Color based on magnitude
      const color = this.getMagnitudeColor(magnitude);
      this.ctx.fillStyle = color;
      // Draw bars from bottom, leaving 20px for labels
      // Position bar at left edge
      this.ctx.fillRect(xLeft, height - 20 - barHeight, barWidth, barHeight);
    }
  }

  /**
   * Draw frequency and dB labels
   */
  private drawLabels(width: number, height: number): void {
    if (!this.ctx) return;
    const labelColor =
      this.config.colorScheme === "dark" ? "#ffffff" : "#000000";
    const bgColor =
      this.config.colorScheme === "dark"
        ? "rgba(26, 26, 26, 0.9)"
        : "rgba(248, 249, 250, 0.9)";

    this.ctx.font = "9px monospace";
    this.ctx.textAlign = "center";
    this.ctx.textBaseline = "top";

    // Frequency labels under each bar - reduced to every other label
    const freqLabels = [
      { freq: 20, label: "20" },
      { freq: 40, label: "40" },
      { freq: 60, label: "60" },
      { freq: 100, label: "100" },
      { freq: 200, label: "200" },
      { freq: 400, label: "400" },
      { freq: 600, label: "600" },
      { freq: 1000, label: "1k" },
      { freq: 2000, label: "2k" },
      { freq: 4000, label: "4k" },
      { freq: 6000, label: "6k" },
      { freq: 10000, label: "10k" },
      { freq: 20000, label: "20k" },
    ];

    for (const { freq, label } of freqLabels) {
      if (!this.ctx) return;
      if (freq >= this.config.minFreq && freq <= this.config.maxFreq) {
        const x = this.freqToX(freq, width);
        const y = height - 8;

        // Draw background for better visibility
        this.ctx.fillStyle = bgColor;
        this.ctx.fillRect(x - 20, y - 6, 40, 10);

        // Draw text
        this.ctx.fillStyle = labelColor;
        this.ctx.fillText(label, x, y - 4);
      }
    }

    // dB labels on vertical axis (left side) - adjust for compact height
    this.ctx.textAlign = "right";
    this.ctx.textBaseline = "middle";
    // Show fewer labels for compact height
    for (let i = 0; i <= 3; i++) {
      if (!this.ctx) return;
      const db = -i * (this.config.dbRange / 3);
      // Adjust Y positioning for 72px height
      const y = 3 + (i * (height - 27)) / 3;

      const label = `${db.toFixed(0)}dB`;

      // Draw background for better visibility
      this.ctx.fillStyle = bgColor;
      this.ctx.fillRect(0, y - 6, 30, 12);

      // Draw text
      this.ctx.fillStyle = labelColor;
      this.ctx.fillText(label, 28, y);
    }
  }

  /**
   * Convert frequency to x coordinate
   */
  private freqToX(freq: number, width: number): number {
    const logMin = Math.log10(this.config.minFreq);
    const logMax = Math.log10(this.config.maxFreq);
    const logFreq = Math.log10(freq);

    const normalized = (logFreq - logMin) / (logMax - logMin);
    // Use smaller left padding for compact canvas
    return 30 + normalized * (width - 35);
  }

  /**
   * Convert dB magnitude to height
   * @param magnitude - dB value (typically 0 to -60)
   * @param height - Total canvas height
   * @returns Bar height in pixels (accounting for label space at bottom)
   */
  private dbToHeight(magnitude: number, height: number): number {
    if (!isFinite(magnitude)) {
      return 0.0;
    }
    // Clamp magnitude to dbRange (default -60dB to 0dB)
    const clamped = Math.max(-this.config.dbRange, Math.min(0, magnitude));
    // Normalize to 0-1 range (0dB = 1.0, -60dB = 0.0)
    const normalized = (clamped + this.config.dbRange) / this.config.dbRange;
    // Scale to available height (leaving 20px for labels at bottom)
    const availableHeight = height - 20;
    return normalized * availableHeight;
  }

  /**
   * Get color based on magnitude
   */
  private getMagnitudeColor(magnitude: number): string {
    if (!isFinite(magnitude)) {
      return this.config.colorScheme === "dark" ? "#333333" : "#eeeeee";
    }

    // Color gradient: blue -> green -> yellow -> red
    // Dark mode colors are now lighter for better visibility
    if (magnitude < -40) {
      return this.config.colorScheme === "dark" ? "#4fc3f7" : "#8ab4f8";
    } else if (magnitude < -20) {
      return this.config.colorScheme === "dark" ? "#66bb6a" : "#81c995";
    } else if (magnitude < -10) {
      return this.config.colorScheme === "dark" ? "#ffeb3b" : "#fdd835";
    } else if (magnitude < 0) {
      return this.config.colorScheme === "dark" ? "#ff9800" : "#ff6f00";
    } else {
      return this.config.colorScheme === "dark" ? "#ef5350" : "#d32f2f";
    }
  }

  /**
   * Get current spectrum data
   */
  getSpectrum(): SpectrumInfo | null {
    return this.currentSpectrum;
  }

  /**
   * Check if monitoring is active
   */
  isActive(): boolean {
    return this.isMonitoring;
  }

  /**
   * Resize canvas
   */
  resize(): void {
    this.setupCanvas();
  }

  /**
   * Cleanup
   */
  destroy(): void {
    this.stop();
  }

  /**
   * Get computed CSS variable value
   */
  private getComputedCSSVariable(varName: string): string {
    const value = getComputedStyle(document.documentElement)
      .getPropertyValue(varName)
      .trim();

    // Better fallbacks based on color scheme
    if (!value) {
      if (varName === "--bg-secondary") {
        return this.config.colorScheme === "dark" ? "#2d2d2d" : "#f8f9fa";
      }
      return this.config.colorScheme === "dark" ? "#ffffff" : "#000000";
    }

    return value;
  }
}
