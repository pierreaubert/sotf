// Enhanced graph renderer for the capture modal with smoothing capabilities

export interface GraphData {
  frequencies: number[];
  rawMagnitudes: number[];
  smoothedMagnitudes?: number[];
  rawPhase?: number[]; // Phase in degrees
  smoothedPhase?: number[]; // Smoothed phase in degrees
  // Channel-specific data (if available)
  channelData?: {
    left?: {
      rawMagnitudes: number[];
      smoothedMagnitudes?: number[];
      rawPhase?: number[];
      smoothedPhase?: number[];
    };
    right?: {
      rawMagnitudes: number[];
      smoothedMagnitudes?: number[];
      rawPhase?: number[];
      smoothedPhase?: number[];
    };
    average?: {
      rawMagnitudes: number[];
      smoothedMagnitudes?: number[];
      rawPhase?: number[];
      smoothedPhase?: number[];
    };
  };
  // Channel metadata
  outputChannel?: "left" | "right" | "both" | "default";
}

export class CaptureGraphRenderer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private width: number = 0;
  private height: number = 0;
  private padding = { top: 60, right: 120, bottom: 200, left: 80 }; // Increased bottom padding for legend at bottom
  private gridColor = "#e0e0e0";
  private rawMagnitudeColor = "#007bff";
  private smoothedMagnitudeColor = "#28a745";
  private rawPhaseColor = "#ff6b6b";
  private smoothedPhaseColor = "#fd7e14";
  // Channel-specific colors
  private leftChannelColor = "#e74c3c"; // Red
  private rightChannelColor = "#3498db"; // Blue
  private averageChannelColor = "#9b59b6"; // Purple
  private backgroundColor = "#ffffff";
  private textColor = "#333333";
  private gridTextColor = "#666666";
  private showPhase = true; // Control phase visibility
  private calibrationData: {
    frequencies: number[];
    magnitudes: number[];
  } | null = null;

  // Channel display controls
  private showLeft = true;
  private showRight = true;
  private showAverage = true;
  private showCombined = true; // Show the main combined curve

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    const ctx = canvas.getContext("2d");
    if (!ctx) {
      throw new Error("Could not get 2D context from canvas");
    }
    this.ctx = ctx;

    // Set up responsive canvas sizing
    this.setupCanvas();
    this.bindEvents();
  }

  private setupCanvas(): void {
    const rect = this.canvas.getBoundingClientRect();
    const devicePixelRatio = window.devicePixelRatio || 1;

    this.width = rect.width * devicePixelRatio;
    this.height = rect.height * devicePixelRatio;

    this.canvas.width = this.width;
    this.canvas.height = this.height;

    // Reset the context transform before scaling to avoid accumulation
    this.ctx.setTransform(1, 0, 0, 1, 0, 0);

    // Scale the context to ensure correct drawing on high DPI displays
    this.ctx.scale(devicePixelRatio, devicePixelRatio);
    this.ctx.imageSmoothingEnabled = true;
  }

  private bindEvents(): void {
    // Handle window resize with debouncing
    let resizeTimeout: number;
    window.addEventListener("resize", () => {
      clearTimeout(resizeTimeout);
      resizeTimeout = setTimeout(() => {
        this.setupCanvas();
        // Re-render if we have data
        this.renderPlaceholder();
      }, 100);
    });
  }

  public renderGraph(data: GraphData): void {
    console.log("renderGraph called with data:", {
      frequencies: data.frequencies.length,
      rawMagnitudes: data.rawMagnitudes.length,
      smoothedMagnitudes: data.smoothedMagnitudes?.length || 0,
      rawPhase: data.rawPhase?.length || 0,
      smoothedPhase: data.smoothedPhase?.length || 0,
    });

    // Note: Graph uses -40 to +10 dB Y-axis range and normalizes magnitudes
    // by removing the mean over 100Hz-10kHz to improve visibility

    this.clear();
    this.drawBackground();
    this.drawGrid();
    this.drawAxes();
    this.drawLabels();

    if (data.frequencies.length > 0) {
      console.log("Drawing magnitude curves...");

      // Apply calibration correction first, then normalize magnitudes
      const calibratedRawMagnitudes =
        data.rawMagnitudes.length > 0
          ? this.applyCalibration(data.frequencies, data.rawMagnitudes)
          : [];

      const calibratedSmoothedMagnitudes =
        data.smoothedMagnitudes && data.smoothedMagnitudes.length > 0
          ? this.applyCalibration(data.frequencies, data.smoothedMagnitudes)
          : [];

      const normalizedRawMagnitudes =
        calibratedRawMagnitudes.length > 0
          ? CaptureGraphRenderer.normalizeMagnitudes(
              data.frequencies,
              calibratedRawMagnitudes,
            )
          : [];

      const normalizedSmoothedMagnitudes =
        calibratedSmoothedMagnitudes.length > 0
          ? CaptureGraphRenderer.normalizeMagnitudes(
              data.frequencies,
              calibratedSmoothedMagnitudes,
            )
          : null;

      // Draw main combined magnitude curves (if enabled)
      if (this.showCombined) {
        if (normalizedRawMagnitudes.length > 0) {
          console.log("Drawing raw magnitude curve (normalized)");
          this.drawMagnitudeCurve(
            data.frequencies,
            normalizedRawMagnitudes,
            this.rawMagnitudeColor,
            1,
            "raw",
            "Combined",
          );
        }

        if (
          normalizedSmoothedMagnitudes &&
          normalizedSmoothedMagnitudes.length > 0
        ) {
          console.log("Drawing smoothed magnitude curve (normalized)");
          this.drawMagnitudeCurve(
            data.frequencies,
            normalizedSmoothedMagnitudes,
            this.smoothedMagnitudeColor,
            3,
            "smoothed",
            "Combined",
          );
        }
      }

      // Draw channel-specific curves (if available and enabled)
      if (data.channelData) {
        // Left channel
        if (this.showLeft && data.channelData.left) {
          const leftCalibratedRaw = this.applyCalibration(
            data.frequencies,
            data.channelData.left.rawMagnitudes,
          );
          const leftNormalizedRaw = CaptureGraphRenderer.normalizeMagnitudes(
            data.frequencies,
            leftCalibratedRaw,
          );

          this.drawMagnitudeCurve(
            data.frequencies,
            leftNormalizedRaw,
            this.leftChannelColor,
            1,
            "raw",
            "Left",
          );

          if (data.channelData.left.smoothedMagnitudes) {
            const leftCalibratedSmoothed = this.applyCalibration(
              data.frequencies,
              data.channelData.left.smoothedMagnitudes,
            );
            const leftNormalizedSmoothed =
              CaptureGraphRenderer.normalizeMagnitudes(
                data.frequencies,
                leftCalibratedSmoothed,
              );
            this.drawMagnitudeCurve(
              data.frequencies,
              leftNormalizedSmoothed,
              this.leftChannelColor,
              2,
              "smoothed",
              "Left",
            );
          }
        }

        // Right channel
        if (this.showRight && data.channelData.right) {
          const rightCalibratedRaw = this.applyCalibration(
            data.frequencies,
            data.channelData.right.rawMagnitudes,
          );
          const rightNormalizedRaw = CaptureGraphRenderer.normalizeMagnitudes(
            data.frequencies,
            rightCalibratedRaw,
          );

          this.drawMagnitudeCurve(
            data.frequencies,
            rightNormalizedRaw,
            this.rightChannelColor,
            1,
            "raw",
            "Right",
          );

          if (data.channelData.right.smoothedMagnitudes) {
            const rightCalibratedSmoothed = this.applyCalibration(
              data.frequencies,
              data.channelData.right.smoothedMagnitudes,
            );
            const rightNormalizedSmoothed =
              CaptureGraphRenderer.normalizeMagnitudes(
                data.frequencies,
                rightCalibratedSmoothed,
              );
            this.drawMagnitudeCurve(
              data.frequencies,
              rightNormalizedSmoothed,
              this.rightChannelColor,
              2,
              "smoothed",
              "Right",
            );
          }
        }

        // Average channel
        if (this.showAverage && data.channelData.average) {
          const avgCalibratedRaw = this.applyCalibration(
            data.frequencies,
            data.channelData.average.rawMagnitudes,
          );
          const avgNormalizedRaw = CaptureGraphRenderer.normalizeMagnitudes(
            data.frequencies,
            avgCalibratedRaw,
          );

          this.drawMagnitudeCurve(
            data.frequencies,
            avgNormalizedRaw,
            this.averageChannelColor,
            1,
            "raw",
            "Average",
          );

          if (data.channelData.average.smoothedMagnitudes) {
            const avgCalibratedSmoothed = this.applyCalibration(
              data.frequencies,
              data.channelData.average.smoothedMagnitudes,
            );
            const avgNormalizedSmoothed =
              CaptureGraphRenderer.normalizeMagnitudes(
                data.frequencies,
                avgCalibratedSmoothed,
              );
            this.drawMagnitudeCurve(
              data.frequencies,
              avgNormalizedSmoothed,
              this.averageChannelColor,
              2,
              "smoothed",
              "Average",
            );
          }
        }
      }

      // Draw phase curves (right axis) if phase data is available and enabled
      if (this.showPhase) {
        if (data.rawPhase && data.rawPhase.length > 0) {
          this.drawPhaseCurve(
            data.frequencies,
            data.rawPhase,
            this.rawPhaseColor,
            1,
            "raw",
          );
        }

        if (data.smoothedPhase && data.smoothedPhase.length > 0) {
          this.drawPhaseCurve(
            data.frequencies,
            data.smoothedPhase,
            this.smoothedPhaseColor,
            3,
            "smoothed",
          );
        }
      }

      this.drawLegend(data);
    } else {
      console.log("No frequency data to draw");
    }
  }

  public setPhaseVisibility(visible: boolean): void {
    this.showPhase = visible;
  }

  public setCalibrationData(frequencies: number[], magnitudes: number[]): void {
    console.log("Setting calibration data:", {
      frequencies: frequencies.length,
      magnitudes: magnitudes.length,
    });
    this.calibrationData = { frequencies, magnitudes };
  }

  public clearCalibrationData(): void {
    console.log("Clearing calibration data");
    this.calibrationData = null;
  }

  public hasCalibration(): boolean {
    return this.calibrationData !== null;
  }

  public setChannelVisibility(
    channel: "left" | "right" | "average" | "combined",
    visible: boolean,
  ): void {
    switch (channel) {
      case "left":
        this.showLeft = visible;
        break;
      case "right":
        this.showRight = visible;
        break;
      case "average":
        this.showAverage = visible;
        break;
      case "combined":
        this.showCombined = visible;
        break;
    }
    console.log(`Channel visibility: ${channel} = ${visible}`);
  }

  public getChannelVisibility(
    channel: "left" | "right" | "average" | "combined",
  ): boolean {
    switch (channel) {
      case "left":
        return this.showLeft;
      case "right":
        return this.showRight;
      case "average":
        return this.showAverage;
      case "combined":
        return this.showCombined;
      default:
        return false;
    }
  }

  public renderPlaceholder(): void {
    this.clear();
    this.drawBackground();
    this.drawGrid();
    this.drawAxes();
    this.drawLabels();

    // Draw placeholder text
    this.ctx.fillStyle = this.gridTextColor;
    this.ctx.font = "16px sans-serif";
    this.ctx.textAlign = "center";
    const centerX = this.width / window.devicePixelRatio / 2;
    const centerY = this.height / window.devicePixelRatio / 2;
    this.ctx.fillText(
      'Click "Start Capture" to begin measurement',
      centerX,
      centerY,
    );
  }

  public updateProgress(progress: number, currentFreq?: number): void {
    // Update progress indicator on the graph
    const rect = this.getPlotArea();
    const progressX = rect.left + rect.width * progress;

    // Draw progress indicator
    this.ctx.save();
    this.ctx.strokeStyle = "#ff6b6b";
    this.ctx.lineWidth = 2;
    this.ctx.setLineDash([5, 5]);
    this.ctx.beginPath();
    this.ctx.moveTo(progressX, rect.top);
    this.ctx.lineTo(progressX, rect.bottom);
    this.ctx.stroke();
    this.ctx.restore();

    // Show current frequency if provided
    if (currentFreq) {
      this.ctx.fillStyle = "#ff6b6b";
      this.ctx.font = "bold 14px sans-serif";
      this.ctx.textAlign = "center";
      this.ctx.fillText(
        `${Math.round(currentFreq)} Hz`,
        progressX,
        rect.top - 10,
      );
    }
  }

  private clear(): void {
    this.ctx.clearRect(
      0,
      0,
      this.width / window.devicePixelRatio,
      this.height / window.devicePixelRatio,
    );
  }

  private drawBackground(): void {
    this.ctx.fillStyle = this.backgroundColor;
    this.ctx.fillRect(
      0,
      0,
      this.width / window.devicePixelRatio,
      this.height / window.devicePixelRatio,
    );
  }

  private getPlotArea(): {
    left: number;
    top: number;
    width: number;
    height: number;
    right: number;
    bottom: number;
  } {
    const scaledWidth = this.width / window.devicePixelRatio;
    const scaledHeight = this.height / window.devicePixelRatio;

    return {
      left: this.padding.left,
      top: this.padding.top,
      width: scaledWidth - this.padding.left - this.padding.right,
      height: scaledHeight - this.padding.top - this.padding.bottom,
      right: scaledWidth - this.padding.right,
      bottom: scaledHeight - this.padding.bottom,
    };
  }

  private drawGrid(): void {
    const rect = this.getPlotArea();
    this.ctx.strokeStyle = this.gridColor;
    this.ctx.lineWidth = 0.5;

    // Vertical grid lines (logarithmic frequencies)
    const freqPoints = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
    freqPoints.forEach((freq) => {
      const x = this.freqToX(freq, rect);
      this.ctx.beginPath();
      this.ctx.moveTo(x, rect.top);
      this.ctx.lineTo(x, rect.bottom);
      this.ctx.stroke();
    });

    // Horizontal grid lines for magnitude (dB scale - left axis)
    this.ctx.strokeStyle = this.gridColor;
    for (let db = -40; db <= 10; db += 10) {
      const y = this.dbToY(db, rect);
      this.ctx.beginPath();
      this.ctx.moveTo(rect.left, y);
      this.ctx.lineTo(rect.right, y);
      this.ctx.stroke();
    }

    // Horizontal grid lines for phase (degree scale - right axis) - lighter
    if (this.showPhase) {
      this.ctx.strokeStyle = "#f0f0f0"; // Lighter color for phase grid
      for (let deg = -180; deg <= 180; deg += 45) {
        const y = this.phaseToY(deg, rect);
        this.ctx.beginPath();
        this.ctx.moveTo(rect.left, y);
        this.ctx.lineTo(rect.right, y);
        this.ctx.stroke();
      }
    }
  }

  private drawAxes(): void {
    const rect = this.getPlotArea();
    this.ctx.strokeStyle = this.textColor;
    this.ctx.lineWidth = 2;

    // X-axis
    this.ctx.beginPath();
    this.ctx.moveTo(rect.left, rect.bottom);
    this.ctx.lineTo(rect.right, rect.bottom);
    this.ctx.stroke();

    // Y-axis
    this.ctx.beginPath();
    this.ctx.moveTo(rect.left, rect.top);
    this.ctx.lineTo(rect.left, rect.bottom);
    this.ctx.stroke();
  }

  private drawLabels(): void {
    const rect = this.getPlotArea();
    this.ctx.fillStyle = this.gridTextColor;
    this.ctx.font = "12px sans-serif";

    // X-axis labels (frequency)
    this.ctx.textAlign = "center";
    const freqPoints = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
    freqPoints.forEach((freq) => {
      const x = this.freqToX(freq, rect);
      const label = freq >= 1000 ? `${freq / 1000}k` : `${freq}`;
      this.ctx.fillText(label, x, rect.bottom + 20);
    });

    // Left Y-axis labels (Magnitude in dB)
    this.ctx.textAlign = "right";
    for (let db = -40; db <= 10; db += 10) {
      const y = this.dbToY(db, rect);
      this.ctx.fillText(`${db} dB`, rect.left - 10, y + 4);
    }

    // Right Y-axis labels (Phase in degrees) - if phase is enabled
    if (this.showPhase) {
      this.ctx.fillStyle = this.rawPhaseColor;
      this.ctx.textAlign = "left";
      for (let deg = -180; deg <= 180; deg += 90) {
        const y = this.phaseToY(deg, rect);
        this.ctx.fillText(`${deg}°`, rect.right + 10, y + 4);
      }
    }

    // Axis titles
    this.ctx.fillStyle = this.textColor;
    this.ctx.font = "bold 14px sans-serif";
    this.ctx.textAlign = "center";

    // X-axis title
    this.ctx.fillText(
      "Frequency (Hz)",
      rect.left + rect.width / 2,
      rect.bottom + 50,
    );

    // Left Y-axis title (Magnitude)
    this.ctx.save();
    this.ctx.translate(25, rect.top + rect.height / 2);
    this.ctx.rotate(-Math.PI / 2);
    this.ctx.fillText("Magnitude (dB)", 0, 0);
    this.ctx.restore();

    // Right Y-axis title (Phase) - if phase is enabled
    if (this.showPhase) {
      this.ctx.save();
      this.ctx.translate(rect.right + 80, rect.top + rect.height / 2);
      this.ctx.rotate(Math.PI / 2);
      this.ctx.fillText("Phase (degrees)", 0, 0);
      this.ctx.restore();
    }

    // Graph title
    this.ctx.font = "bold 16px sans-serif";
    const title = this.showPhase
      ? "Frequency & Phase Response"
      : "Frequency Response";
    this.ctx.fillText(title, rect.left + rect.width / 2, 30);
  }

  private drawMagnitudeCurve(
    frequencies: number[],
    magnitudes: number[],
    color: string,
    lineWidth: number,
    type: "raw" | "smoothed",
    _channelLabel?: string,
  ): void {
    const rect = this.getPlotArea();

    // Debug logging
    console.log(`Drawing ${type} magnitude curve:`);
    console.log("  Rect:", rect);
    console.log("  Color:", color);
    console.log("  Line width:", lineWidth);
    console.log("  Frequencies length:", frequencies.length);
    console.log("  Magnitudes length:", magnitudes.length);

    if (frequencies.length > 0 && magnitudes.length > 0) {
      console.log("  First 5 frequencies:", frequencies.slice(0, 5));
      console.log("  First 5 magnitudes:", magnitudes.slice(0, 5));
    }

    this.ctx.strokeStyle = color;
    this.ctx.lineWidth = lineWidth;
    this.ctx.lineCap = "round";
    this.ctx.lineJoin = "round";

    // Set line style for raw data
    if (type === "raw") {
      this.ctx.globalAlpha = 0.6;
      this.ctx.setLineDash([2, 2]);
    } else {
      this.ctx.globalAlpha = 1.0;
      this.ctx.setLineDash([]);
    }

    this.ctx.beginPath();
    let started = false;
    let pointsDrawn = 0;
    let validPoints = 0;
    let outOfRangePoints = 0;

    for (let i = 0; i < frequencies.length; i++) {
      const freq = frequencies[i];
      const mag = magnitudes[i];

      // Check if values are valid
      const freqValid = freq >= 20 && freq <= 20000;
      const magValid = !isNaN(mag) && isFinite(mag);

      if (!freqValid || !magValid) {
        outOfRangePoints++;
        if (pointsDrawn < 5) {
          console.log(
            `  Skipped point ${i}: freq=${freq} (valid=${freqValid}), mag=${mag} (valid=${magValid})`,
          );
        }
        continue;
      }

      const x = this.freqToX(freq, rect);
      const y = this.dbToY(mag, rect);

      // Check if coordinates are within reasonable bounds
      const xInBounds = x >= rect.left && x <= rect.left + rect.width;
      const yInBounds = y >= rect.top && y <= rect.top + rect.height;

      // Debug first few points with more detail
      if (pointsDrawn < 5) {
        console.log(
          `  Point ${pointsDrawn}: freq=${freq}, mag=${mag}, x=${x} (inBounds=${xInBounds}), y=${y} (inBounds=${yInBounds})`,
        );
        console.log(
          `    Rect bounds: left=${rect.left}, right=${rect.left + rect.width}, top=${rect.top}, bottom=${rect.top + rect.height}`,
        );
      }

      if (!started) {
        this.ctx.moveTo(x, y);
        started = true;
        console.log(`  Started path at (${x}, ${y})`);
      } else {
        this.ctx.lineTo(x, y);
      }

      pointsDrawn++;
      validPoints++;
    }

    console.log(
      `  Valid points: ${validPoints}, Out of range: ${outOfRangePoints}, Total processed: ${frequencies.length}`,
    );
    console.log(`  Path started: ${started}`);

    // Debug the current path state
    console.log(`  Current stroke style: ${this.ctx.strokeStyle}`);
    console.log(`  Current line width: ${this.ctx.lineWidth}`);
    console.log(`  Current global alpha: ${this.ctx.globalAlpha}`);

    console.log(`  Points drawn: ${pointsDrawn}`);

    this.ctx.stroke();

    // Draw debug circles at first few points to verify coordinates
    if (type === "raw" && validPoints > 0) {
      this.ctx.save();
      this.ctx.fillStyle = "blue";
      this.ctx.globalAlpha = 1.0;

      let circlesDrawn = 0;
      for (let i = 0; i < frequencies.length && circlesDrawn < 5; i++) {
        const freq = frequencies[i];
        const mag = magnitudes[i];

        if (freq >= 20 && freq <= 20000 && !isNaN(mag) && isFinite(mag)) {
          const x = this.freqToX(freq, rect);
          const y = this.dbToY(mag, rect);

          this.ctx.beginPath();
          this.ctx.arc(x, y, 3, 0, 2 * Math.PI);
          this.ctx.fill();
          circlesDrawn++;

          console.log(
            `Drew debug circle at (${x}, ${y}) for freq=${freq}, mag=${mag}`,
          );
        }
      }

      this.ctx.restore();
    }

    // Red test line removed

    this.ctx.globalAlpha = 1.0;
    this.ctx.setLineDash([]);

    // Try a completely simple drawing approach as fallback
    if (type === "raw" && validPoints > 0) {
      console.log("Drawing simple fallback curve...");
      this.ctx.save();
      this.ctx.strokeStyle = "green";
      this.ctx.lineWidth = 2;
      this.ctx.globalAlpha = 1.0;
      this.ctx.setLineDash([]);

      this.ctx.beginPath();
      let simpleStarted = false;

      for (let i = 0; i < Math.min(frequencies.length, 10); i++) {
        const freq = frequencies[i];
        const mag = magnitudes[i];

        if (freq >= 20 && freq <= 20000 && !isNaN(mag) && isFinite(mag)) {
          const x = this.freqToX(freq, rect);
          const y = this.dbToY(mag, rect);

          if (!simpleStarted) {
            this.ctx.moveTo(x, y);
            simpleStarted = true;
            console.log(`Simple curve started at (${x}, ${y})`);
          } else {
            this.ctx.lineTo(x, y);
            console.log(`Simple curve line to (${x}, ${y})`);
          }
        }
      }

      if (simpleStarted) {
        this.ctx.stroke();
        console.log("Simple fallback curve drawn");
      }

      this.ctx.restore();
    }
  }

  private drawPhaseCurve(
    frequencies: number[],
    phases: number[],
    color: string,
    lineWidth: number,
    type: "raw" | "smoothed",
  ): void {
    const rect = this.getPlotArea();
    this.ctx.strokeStyle = color;
    this.ctx.lineWidth = lineWidth;
    this.ctx.lineCap = "round";
    this.ctx.lineJoin = "round";

    // Set line style for raw data
    if (type === "raw") {
      this.ctx.globalAlpha = 0.5;
      this.ctx.setLineDash([3, 3]);
    } else {
      this.ctx.globalAlpha = 0.8;
      this.ctx.setLineDash([]);
    }

    this.ctx.beginPath();
    let started = false;

    for (let i = 0; i < frequencies.length; i++) {
      const freq = frequencies[i];
      const phase = phases[i];

      if (freq >= 20 && freq <= 20000 && !isNaN(phase) && isFinite(phase)) {
        const x = this.freqToX(freq, rect);
        const y = this.phaseToY(phase, rect);

        if (!started) {
          this.ctx.moveTo(x, y);
          started = true;
        } else {
          this.ctx.lineTo(x, y);
        }
      }
    }

    this.ctx.stroke();
    this.ctx.globalAlpha = 1.0;
    this.ctx.setLineDash([]);
  }

  private drawLegend(data: GraphData): void {
    const rect = this.getPlotArea();

    // Position legend at bottom center in horizontal layout
    const legendWidth = 600; // Wider for horizontal layout
    const legendX = rect.left + (rect.width - legendWidth) / 2;

    // Calculate legend height based on available data
    let rowCount = 0;

    // Count rows for combined curves
    if (this.showCombined) {
      rowCount++; // Raw magnitude
      if (data.smoothedMagnitudes) rowCount++; // Smoothed magnitude
    }

    // Count rows for channel-specific curves
    if (data.channelData) {
      if (this.showLeft && data.channelData.left) {
        rowCount++; // Left raw
        if (data.channelData.left.smoothedMagnitudes) rowCount++; // Left smoothed
      }
      if (this.showRight && data.channelData.right) {
        rowCount++; // Right raw
        if (data.channelData.right.smoothedMagnitudes) rowCount++; // Right smoothed
      }
      if (this.showAverage && data.channelData.average) {
        rowCount++; // Average raw
        if (data.channelData.average.smoothedMagnitudes) rowCount++; // Average smoothed
      }
    }

    // Count phase rows
    if (this.showPhase) {
      if (data.rawPhase) rowCount++;
      if (data.smoothedPhase) rowCount++;
    }
    void rowCount; // rowCount is used for documentation purposes

    // Position at bottom of graph with proper spacing below axis title
    const legendHeight = 50; // Fixed height for horizontal layout
    const legendY = rect.bottom + 70; // Increased to be below X-axis title (was +10)

    // Legend background
    this.ctx.fillStyle = "rgba(255, 255, 255, 0.95)";
    this.ctx.fillRect(
      legendX - 10,
      legendY - 10,
      legendWidth + 20,
      legendHeight,
    );
    this.ctx.strokeStyle = this.gridColor;
    this.ctx.lineWidth = 1;
    this.ctx.strokeRect(
      legendX - 10,
      legendY - 10,
      legendWidth + 20,
      legendHeight,
    );

    this.ctx.font = "10px sans-serif";
    this.ctx.textAlign = "left";
    let currentX = legendX;
    const currentY = legendY + 5;

    // Helper function to get channel suffix for legend
    const getChannelSuffix = (outputChannel?: string) => {
      if (
        !outputChannel ||
        outputChannel === "both" ||
        outputChannel === "default"
      ) {
        return "";
      }
      return ` (${outputChannel.charAt(0).toUpperCase() + outputChannel.slice(1)})`;
    };

    const itemSpacing = 100; // Horizontal spacing between legend items

    // Combined magnitude curves
    if (this.showCombined) {
      const channelSuffix = getChannelSuffix(data.outputChannel);

      // Raw magnitude
      this.ctx.strokeStyle = this.rawMagnitudeColor;
      this.ctx.lineWidth = 1;
      this.ctx.setLineDash([2, 2]);
      this.ctx.beginPath();
      this.ctx.moveTo(currentX, currentY);
      this.ctx.lineTo(currentX + 20, currentY);
      this.ctx.stroke();
      this.ctx.setLineDash([]);

      this.ctx.fillStyle = this.textColor;
      this.ctx.fillText(`Raw${channelSuffix}`, currentX + 25, currentY + 4);
      currentX += itemSpacing;

      // Smoothed magnitude (if available)
      if (data.smoothedMagnitudes) {
        this.ctx.strokeStyle = this.smoothedMagnitudeColor;
        this.ctx.lineWidth = 3;
        this.ctx.beginPath();
        this.ctx.moveTo(currentX, currentY);
        this.ctx.lineTo(currentX + 20, currentY);
        this.ctx.stroke();

        this.ctx.fillStyle = this.textColor;
        this.ctx.fillText(
          `Smoothed${channelSuffix}`,
          currentX + 25,
          currentY + 4,
        );
        currentX += itemSpacing;
      }
    }

    // Channel-specific curves
    if (data.channelData) {
      // Left channel
      if (this.showLeft && data.channelData.left) {
        this.ctx.strokeStyle = this.leftChannelColor;
        this.ctx.lineWidth = 1;
        this.ctx.setLineDash([2, 2]);
        this.ctx.beginPath();
        this.ctx.moveTo(currentX, currentY);
        this.ctx.lineTo(currentX + 20, currentY);
        this.ctx.stroke();
        this.ctx.setLineDash([]);

        this.ctx.fillStyle = this.textColor;
        this.ctx.fillText("Left Raw", currentX + 25, currentY + 4);
        currentX += itemSpacing;

        if (data.channelData.left.smoothedMagnitudes) {
          this.ctx.strokeStyle = this.leftChannelColor;
          this.ctx.lineWidth = 2;
          this.ctx.beginPath();
          this.ctx.moveTo(currentX, currentY);
          this.ctx.lineTo(currentX + 20, currentY);
          this.ctx.stroke();

          this.ctx.fillStyle = this.textColor;
          this.ctx.fillText("Left Smooth", currentX + 25, currentY + 4);
          currentX += itemSpacing;
        }
      }

      // Right channel
      if (this.showRight && data.channelData.right) {
        this.ctx.strokeStyle = this.rightChannelColor;
        this.ctx.lineWidth = 1;
        this.ctx.setLineDash([2, 2]);
        this.ctx.beginPath();
        this.ctx.moveTo(currentX, currentY);
        this.ctx.lineTo(currentX + 20, currentY);
        this.ctx.stroke();
        this.ctx.setLineDash([]);

        this.ctx.fillStyle = this.textColor;
        this.ctx.fillText("Right Raw", currentX + 25, currentY + 4);
        currentX += itemSpacing;

        if (data.channelData.right.smoothedMagnitudes) {
          this.ctx.strokeStyle = this.rightChannelColor;
          this.ctx.lineWidth = 2;
          this.ctx.beginPath();
          this.ctx.moveTo(currentX, currentY);
          this.ctx.lineTo(currentX + 20, currentY);
          this.ctx.stroke();

          this.ctx.fillStyle = this.textColor;
          this.ctx.fillText("Right Smooth", currentX + 25, currentY + 4);
          currentX += itemSpacing;
        }
      }

      // Average channel
      if (this.showAverage && data.channelData.average) {
        this.ctx.strokeStyle = this.averageChannelColor;
        this.ctx.lineWidth = 1;
        this.ctx.setLineDash([2, 2]);
        this.ctx.beginPath();
        this.ctx.moveTo(currentX, currentY);
        this.ctx.lineTo(currentX + 20, currentY);
        this.ctx.stroke();
        this.ctx.setLineDash([]);

        this.ctx.fillStyle = this.textColor;
        this.ctx.fillText("Average Raw", currentX + 25, currentY + 4);
        currentX += itemSpacing;

        if (data.channelData.average.smoothedMagnitudes) {
          this.ctx.strokeStyle = this.averageChannelColor;
          this.ctx.lineWidth = 2;
          this.ctx.beginPath();
          this.ctx.moveTo(currentX, currentY);
          this.ctx.lineTo(currentX + 20, currentY);
          this.ctx.stroke();

          this.ctx.fillStyle = this.textColor;
          this.ctx.fillText("Average Smooth", currentX + 25, currentY + 4);
          currentX += itemSpacing;
        }
      }
    }

    // Phase legends (if enabled and data available)
    if (this.showPhase) {
      // Raw phase
      if (data.rawPhase) {
        this.ctx.strokeStyle = this.rawPhaseColor;
        this.ctx.lineWidth = 1;
        this.ctx.setLineDash([3, 3]);
        this.ctx.beginPath();
        this.ctx.moveTo(currentX, currentY);
        this.ctx.lineTo(currentX + 20, currentY);
        this.ctx.stroke();
        this.ctx.setLineDash([]);

        this.ctx.fillStyle = this.textColor;
        this.ctx.fillText("Raw Phase", currentX + 25, currentY + 4);
        currentX += itemSpacing;
      }

      // Smoothed phase (if available)
      if (data.smoothedPhase) {
        this.ctx.strokeStyle = this.smoothedPhaseColor;
        this.ctx.lineWidth = 3;
        this.ctx.beginPath();
        this.ctx.moveTo(currentX, currentY);
        this.ctx.lineTo(currentX + 20, currentY);
        this.ctx.stroke();

        this.ctx.fillStyle = this.textColor;
        this.ctx.fillText("Smoothed Phase", currentX + 25, currentY + 4);
      }
    }
  }

  private freqToX(freq: number, rect: { left: number; width: number }): number {
    const logMin = Math.log10(20);
    const logMax = Math.log10(20000);
    const logFreq = Math.log10(freq);
    const ratio = (logFreq - logMin) / (logMax - logMin);
    const x = rect.left + ratio * rect.width;

    // Debug logging for coordinate conversion (reduced frequency)
    if (Math.random() < 0.001) {
      // Very occasional logging
      console.log(`freqToX: ${freq}Hz -> x=${x}`);
    }

    return x;
  }

  private dbToY(
    db: number,
    rect: { top: number; height: number; bottom: number },
  ): number {
    const minDb = -40;
    const maxDb = 10;
    const ratio = (db - minDb) / (maxDb - minDb);
    const y = rect.bottom - ratio * rect.height;

    // Debug logging for coordinate conversion (reduced frequency)
    if (Math.random() < 0.001) {
      // Very occasional logging
      console.log(`dbToY: ${db}dB -> y=${y}`);
    }

    return y;
  }

  private phaseToY(
    phase: number,
    rect: { top: number; height: number; bottom: number },
  ): number {
    const minPhase = -180;
    const maxPhase = 180;
    const ratio = (phase - minPhase) / (maxPhase - minPhase);
    return rect.bottom - ratio * rect.height;
  }

  // Apply calibration correction to magnitudes
  private applyCalibration(
    frequencies: number[],
    magnitudes: number[],
  ): number[] {
    if (!this.calibrationData) {
      return magnitudes.slice(); // Return copy if no calibration
    }

    console.log("Applying calibration correction...");

    const corrected = magnitudes.map((mag, i) => {
      const freq = frequencies[i];

      // Find calibration correction for this frequency using interpolation
      const correction = this.interpolateCalibration(freq);

      // Subtract calibration (remove microphone response)
      return mag - correction;
    });

    console.log("Calibration applied to", corrected.length, "points");
    return corrected;
  }

  // Interpolate calibration data for a given frequency
  private interpolateCalibration(targetFreq: number): number {
    if (!this.calibrationData) return 0;

    const { frequencies, magnitudes } = this.calibrationData;

    // Find surrounding points
    let lowerIndex = -1;
    for (let i = 0; i < frequencies.length - 1; i++) {
      if (frequencies[i] <= targetFreq && frequencies[i + 1] > targetFreq) {
        lowerIndex = i;
        break;
      }
    }

    // Handle edge cases
    if (lowerIndex === -1) {
      if (targetFreq <= frequencies[0]) {
        return magnitudes[0];
      } else {
        return magnitudes[magnitudes.length - 1];
      }
    }

    // Linear interpolation in log-frequency space
    const f1 = frequencies[lowerIndex];
    const f2 = frequencies[lowerIndex + 1];
    const m1 = magnitudes[lowerIndex];
    const m2 = magnitudes[lowerIndex + 1];

    const logF1 = Math.log10(f1);
    const logF2 = Math.log10(f2);
    const logTarget = Math.log10(targetFreq);

    const ratio = (logTarget - logF1) / (logF2 - logF1);
    return m1 + ratio * (m2 - m1);
  }

  // Magnitude normalization - removes the mean over 100Hz-10kHz
  public static normalizeMagnitudes(
    frequencies: number[],
    magnitudes: number[],
  ): number[] {
    console.log("Normalizing magnitudes...");

    // Find indices within the normalization range (100Hz - 10kHz)
    let sum = 0;
    let count = 0;

    for (let i = 0; i < frequencies.length; i++) {
      const freq = frequencies[i];
      if (freq >= 100 && freq <= 10000) {
        sum += magnitudes[i];
        count++;
      }
    }

    if (count === 0) {
      console.warn(
        "No frequencies found in 100Hz-10kHz range for normalization",
      );
      return magnitudes.slice(); // Return copy of original
    }

    const mean = sum / count;
    console.log(
      `Normalization: mean over 100Hz-10kHz = ${mean.toFixed(2)} dB (${count} points)`,
    );

    // Subtract the mean from all magnitudes
    const normalized = magnitudes.map((mag) => mag - mean);

    console.log(
      `Magnitude range before normalization: ${Math.min(...magnitudes).toFixed(1)} to ${Math.max(...magnitudes).toFixed(1)} dB`,
    );
    console.log(
      `Magnitude range after normalization: ${Math.min(...normalized).toFixed(1)} to ${Math.max(...normalized).toFixed(1)} dB`,
    );

    return normalized;
  }

  // Smoothing algorithm - 1/3 octave smoothing for magnitude
  public static applySmoothing(
    frequencies: number[],
    magnitudes: number[],
    octaveFraction: number = 3,
  ): number[] {
    const smoothed: number[] = new Array(magnitudes.length);

    for (let i = 0; i < frequencies.length; i++) {
      const centerFreq = frequencies[i];
      const octaveWidth = 1.0 / octaveFraction;
      const lowerBound = centerFreq * Math.pow(2, -octaveWidth / 2);
      const upperBound = centerFreq * Math.pow(2, octaveWidth / 2);

      // Find indices within the smoothing window
      let sum = 0;
      let count = 0;

      for (let j = 0; j < frequencies.length; j++) {
        if (frequencies[j] >= lowerBound && frequencies[j] <= upperBound) {
          // Convert from dB to linear for averaging
          const linear = Math.pow(10, magnitudes[j] / 20);
          sum += linear;
          count++;
        }
      }

      if (count > 0) {
        // Convert average back to dB
        const avgLinear = sum / count;
        smoothed[i] = 20 * Math.log10(avgLinear);
      } else {
        // No data in range, use original value
        smoothed[i] = magnitudes[i];
      }
    }

    return smoothed;
  }

  // Phase smoothing algorithm - handles phase wrapping properly
  public static applyPhaseSmoothing(
    frequencies: number[],
    phases: number[],
    octaveFraction: number = 3,
  ): number[] {
    const smoothed: number[] = new Array(phases.length);

    for (let i = 0; i < frequencies.length; i++) {
      const centerFreq = frequencies[i];
      const octaveWidth = 1.0 / octaveFraction;
      const lowerBound = centerFreq * Math.pow(2, -octaveWidth / 2);
      const upperBound = centerFreq * Math.pow(2, octaveWidth / 2);

      // Collect phase values within the smoothing window
      const phasesInWindow: number[] = [];

      for (let j = 0; j < frequencies.length; j++) {
        if (frequencies[j] >= lowerBound && frequencies[j] <= upperBound) {
          phasesInWindow.push(phases[j]);
        }
      }

      if (phasesInWindow.length > 0) {
        // Use circular mean for phase averaging
        smoothed[i] = this.circularMean(phasesInWindow);
      } else {
        // No data in range, use original value
        smoothed[i] = phases[i];
      }
    }

    return smoothed;
  }

  // Test method for debugging - call from browser console
  public testRender(): void {
    console.log("Running test render...");

    // Generate test data
    const frequencies: number[] = [];
    const magnitudes: number[] = [];

    // Create logarithmically spaced frequencies from 20Hz to 20kHz
    for (let i = 0; i < 100; i++) {
      const freq = 20 * Math.pow(1000, i / 99); // 20 Hz to 20 kHz
      const mag = -10 + 20 * Math.sin(i * 0.1); // Oscillating between -10 and +10 dB (fits new range)

      frequencies.push(freq);
      magnitudes.push(mag);
    }

    console.log("Test data generated:", {
      frequencies: frequencies.length,
      magnitudes: magnitudes.length,
    });
    console.log("First 5 frequencies:", frequencies.slice(0, 5));
    console.log("First 5 magnitudes:", magnitudes.slice(0, 5));

    // Render the test data
    this.renderGraph({
      frequencies,
      rawMagnitudes: magnitudes,
      smoothedMagnitudes: magnitudes.map((m) => m * 0.8), // Slightly attenuated for smoothed
      rawPhase: frequencies.map(() => Math.random() * 360 - 180), // Random phase data
      smoothedPhase: frequencies.map(() => Math.random() * 180 - 90), // Random smoothed phase
    });

    console.log("Test render complete");
  }

  // Circular mean calculation for phase data (handles wrapping)
  private static circularMean(phases: number[]): number {
    let sumSin = 0;
    let sumCos = 0;

    for (const phase of phases) {
      const radians = (phase * Math.PI) / 180; // Convert to radians
      sumSin += Math.sin(radians);
      sumCos += Math.cos(radians);
    }

    const meanRadians = Math.atan2(
      sumSin / phases.length,
      sumCos / phases.length,
    );
    return (meanRadians * 180) / Math.PI; // Convert back to degrees
  }

  public destroy(): void {
    // Clean up event listeners
    window.removeEventListener("resize", this.setupCanvas);
  }
}
