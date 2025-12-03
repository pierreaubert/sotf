// Spectrum Analyzer Plugin
// Wraps the existing spectrum analyzer as a plugin

import { BasePlugin } from "./plugin-base";
import { PluginMenubar } from "./plugin-menubar";
import {
  SpectrumAnalyzerComponent,
  type SpectrumDisplayConfig,
} from "../audio-player/spectrum-analyzer";
import type { PluginMetadata } from "./plugin-types";

/**
 * Spectrum Analyzer Plugin
 * FFT-based frequency spectrum visualization
 */
export class SpectrumPlugin extends BasePlugin {
  public readonly metadata: PluginMetadata = {
    id: "spectrum-plugin",
    name: "SotF: Spectrum",
    category: "analyzer",
    version: "1.0.0",
  };

  // UI components
  private menubar: PluginMenubar | null = null;
  private spectrumAnalyzer: SpectrumAnalyzerComponent | null = null;

  // UI elements
  private spectrumCanvas: HTMLCanvasElement | null = null;

  // Configuration
  private minFreq: number = 20;
  private maxFreq: number = 20000;
  private dbRange: number = 60;
  private pollInterval: number = 100;
  private colorScheme: "light" | "dark" = "dark";

  // Parameter metadata for keyboard control
  protected parameterOrder = ["minFreq", "maxFreq", "dbRange", "pollInterval"];
  protected parameterLabels = {
    minFreq: "Min Frequency",
    maxFreq: "Max Frequency",
    dbRange: "dB Range",
    pollInterval: "Update Rate",
  };
  private parameterRanges = {
    minFreq: { min: 10, max: 200, step: 10 },
    maxFreq: { min: 5000, max: 24000, step: 1000 },
    dbRange: { min: 30, max: 120, step: 10 },
    pollInterval: { min: 50, max: 500, step: 50 },
  };

  /**
   * Render a single parameter slider with labels
   */
  private renderParameter(
    paramName: string,
    index: number,
    unit: string,
  ): string {
    const value = (this as any)[paramName];
    const range =
      this.parameterRanges[paramName as keyof typeof this.parameterRanges];

    // Get formatted label with keyboard shortcut
    const formattedLabel = this.getFormattedLabel(paramName);

    // Format value display
    let displayValue = value.toString();
    displayValue = `${displayValue} ${unit}`;

    // Generate 6 legend values from max to min
    const legendValues = [];
    for (let i = 0; i < 6; i++) {
      const legendValue = range.max - (i * (range.max - range.min)) / 5;
      const formatted = legendValue.toString();
      legendValues.push(formatted);
    }

    return `
      <div class="field parameter-field" data-param="${paramName}" data-index="${index}">
        <div class="is-flex is-flex-direction-column is-align-items-center">
          <div class="has-text-centered has-text-weight-semibold mb-2 has-text-light is-size-7" style="min-height: 2em; display: flex; align-items: center; justify-content: center;">${formattedLabel}</div>
          <span class="tag is-success is-small mb-2 param-value" data-param="${paramName}">${displayValue}</span>
          <div class="is-flex is-align-items-center">
            <!-- Legend on the left -->
            <div class="is-flex is-flex-direction-column is-justify-content-space-between mr-2 has-text-grey-light is-size-7" style="height: 400px; text-align: right;">
              ${legendValues.map((v) => `<span>${v}</span>`).join("")}
            </div>
            <!-- Slider -->
            <input type="range" class="param-slider" data-param="${paramName}"
                   min="${range.min}" max="${range.max}" step="${range.step}" value="${value}"
                   style="writing-mode: vertical-lr; direction: rtl; width: 16px; height: 400px;" />
          </div>
        </div>
      </div>
    `;
  }

  /**
   * Render the plugin UI
   */
  render(standalone: boolean): void {
    if (!this.container) return;

    this.container.innerHTML = `
      <div class="is-flex is-flex-direction-column spectrum-plugin ${standalone ? "standalone" : "embedded"}" style="height: 100%; min-height: 0; background: #1a1a1a;">
        ${standalone ? '<div class="spectrum-menubar-container"></div>' : ""}
        <div class="is-flex is-flex-direction-column is-flex-grow-1" style="min-height: 0; overflow: hidden; padding: 0; margin: 0;">
          <!-- Bulma Columns -->
          <div class="columns is-gapless" style="height: 100%;">
            <!-- Column 1: Parameters (30%) -->
            <div class="column is-one-quarter">
              <div class="box is-flex is-flex-direction-column" style="background: #2a2a2a; border: none; border-right: 1px solid #404040; border-radius: 0; height: 100%; margin: 0;">
                <h4 class="title is-6 has-text-light">Spectrum Settings</h4>
                <div style="overflow-y: auto;">
                  ${this.renderParameter("minFreq", 0, "Hz")}
                  ${this.renderParameter("maxFreq", 1, "Hz")}
                  ${this.renderParameter("dbRange", 2, "dB")}
                  ${this.renderParameter("pollInterval", 3, "ms")}
                </div>

                <!-- Control Buttons -->
                <div class="mt-auto">
                  <div class="is-flex is-flex-direction-column" style="gap: 8px;">
                    <button class="button is-primary is-fullwidth start-btn">Start</button>
                    <button class="button is-danger is-fullwidth stop-btn" disabled>Stop</button>
                    <button class="button is-light is-fullwidth reset-btn">Reset</button>
                  </div>
                </div>
              </div>
            </div>

            <!-- Column 2: Spectrum Display (70%) -->
            <div class="column is-three-quarters">
              <div class="box is-flex is-flex-direction-column" style="background: #2a2a2a; border: none; border-right: 1px solid #404040; border-radius: 0; height: 100%; margin: 0;">
                <h4 class="title is-6 has-text-light has-text-centered mb-2">Frequency Spectrum</h4>
                <div class="spectrum-display-container">
                  <canvas class="spectrum-canvas" width="800" height="400"></canvas>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    `;

    // Initialize menubar if standalone
    if (standalone) {
      const menubarContainer = this.container.querySelector(
        ".spectrum-menubar-container",
      ) as HTMLElement;
      if (menubarContainer) {
        this.menubar = new PluginMenubar(menubarContainer, this.metadata.name, {
          showPresets: false,
          showMatrix: false,
          showMuteSolo: false,
        });
      }
    }

    // Cache elements
    this.spectrumCanvas = this.container.querySelector(
      ".spectrum-canvas",
    ) as HTMLCanvasElement;

    // Initialize spectrum analyzer
    if (this.spectrumCanvas) {
      const config: SpectrumDisplayConfig = {
        canvas: this.spectrumCanvas,
        pollInterval: this.pollInterval,
        minFreq: this.minFreq,
        maxFreq: this.maxFreq,
        dbRange: this.dbRange,
        colorScheme: this.colorScheme,
        showLabels: true,
        showGrid: true,
      };

      this.spectrumAnalyzer = new SpectrumAnalyzerComponent(config);
    }

    this.attachEventListeners();
  }

  /**
   * Attach event listeners
   */
  private attachEventListeners(): void {
    // Parameter sliders
    const sliders = this.container?.querySelectorAll(".param-slider") || [];
    sliders.forEach((slider) => {
      slider.addEventListener("input", (e) => {
        this.handleSliderChange(e as Event);
      });
    });

    // Parameter field click to select
    const fields = this.container?.querySelectorAll(".parameter-field") || [];
    fields.forEach((field) => {
      field.addEventListener("click", (e) => {
        const index = parseInt(
          (field as HTMLElement).dataset.index || "-1",
          10,
        );
        this.selectParameter(index);
      });
    });

    // Control buttons
    const startBtn = this.container?.querySelector(
      ".start-btn",
    ) as HTMLButtonElement;
    const stopBtn = this.container?.querySelector(
      ".stop-btn",
    ) as HTMLButtonElement;
    const resetBtn = this.container?.querySelector(
      ".reset-btn",
    ) as HTMLButtonElement;

    if (startBtn) {
      startBtn.addEventListener("click", async () => {
        await this.start();
        startBtn.disabled = true;
        if (stopBtn) stopBtn.disabled = false;
      });
    }

    if (stopBtn) {
      stopBtn.addEventListener("click", async () => {
        await this.stop();
        if (startBtn) startBtn.disabled = false;
        stopBtn.disabled = true;
      });
    }

    if (resetBtn) {
      resetBtn.addEventListener("click", () => {
        this.recreateAnalyzer();
      });
    }
  }

  /**
   * Handle slider change
   */
  private handleSliderChange(e: Event): void {
    const param = (e.target as HTMLElement).dataset.param!;
    const value = parseInt((e.target as HTMLInputElement).value, 10);

    // Update parameter
    (this as any)[param] = value;

    // Update display
    this.updateParameterDisplay(param, value);

    // Recreate spectrum analyzer with new settings
    if (param !== "pollInterval") {
      this.recreateAnalyzer();
    }

    // Notify parameter change
    this.updateParameter(param, value);
  }

  /**
   * Update parameter display
   */
  private updateParameterDisplay(param: string, value: number): void {
    const field = this.container?.querySelector(
      `.parameter-field[data-param="${param}"]`,
    );
    if (!field) return;

    const label = field.querySelector(".param-value");
    if (label) {
      let unit = "Hz";
      if (param === "pollInterval") {
        unit = "ms";
      } else if (param === "dbRange") {
        unit = "dB";
      }
      label.textContent = `${value} ${unit}`;
    }

    // Update slider value
    const slider = field.querySelector(".param-slider") as HTMLInputElement;
    if (slider) {
      slider.value = value.toString();
    }
  }

  /**
   * Select parameter by index (override base class)
   */
  protected selectParameter(index: number): void {
    super.selectParameter(index);

    // Update visual highlighting
    const fields = this.container?.querySelectorAll(".parameter-field") || [];
    fields.forEach((field, idx) => {
      const slider = field.querySelector(".param-slider") as HTMLElement;
      if (slider) {
        if (idx === index) {
          slider.style.accentColor = "#22c55e"; // Green
          field.classList.add("is-selected");
        } else {
          slider.style.accentColor = "";
          field.classList.remove("is-selected");
        }
      }
    });
  }

  /**
   * Clear parameter selection (override base class)
   */
  protected clearParameterSelection(): void {
    super.clearParameterSelection();

    const fields = this.container?.querySelectorAll(".parameter-field") || [];
    fields.forEach((field) => {
      const slider = field.querySelector(".param-slider") as HTMLElement;
      if (slider) {
        slider.style.accentColor = "";
        field.classList.remove("is-selected");
      }
    });
  }

  /**
   * Adjust selected parameter (override base class)
   */
  protected adjustSelectedParameter(delta: number): void {
    if (this.selectedParameterIndex < 0) return;

    const paramName = this.parameterOrder[this.selectedParameterIndex];
    const range =
      this.parameterRanges[paramName as keyof typeof this.parameterRanges];
    const currentValue = (this as any)[paramName];

    const step = delta > 0 ? range.step : -range.step;
    const newValue = Math.max(
      range.min,
      Math.min(range.max, currentValue + step),
    );

    // Update parameter
    (this as any)[paramName] = newValue;

    // Update display
    this.updateParameterDisplay(paramName, newValue);

    // Recreate analyzer if needed
    if (paramName !== "pollInterval") {
      this.recreateAnalyzer();
    }

    // Notify parameter change
    this.updateParameter(paramName, newValue);
  }

  /**
   * Recreate analyzer with new settings
   */
  private recreateAnalyzer(): void {
    // Stop existing analyzer
    if (this.spectrumAnalyzer) {
      this.spectrumAnalyzer.destroy();
      this.spectrumAnalyzer = null;
    }

    // Create new analyzer
    if (this.spectrumCanvas) {
      const config: SpectrumDisplayConfig = {
        canvas: this.spectrumCanvas,
        pollInterval: this.pollInterval,
        minFreq: this.minFreq,
        maxFreq: this.maxFreq,
        dbRange: this.dbRange,
        colorScheme: this.colorScheme,
        showLabels: true,
        showGrid: true,
      };

      this.spectrumAnalyzer = new SpectrumAnalyzerComponent(config);

      // Restart if was active
      const stopBtn = this.container?.querySelector(
        ".stop-btn",
      ) as HTMLButtonElement;
      if (stopBtn && !stopBtn.disabled) {
        this.start();
      }
    }
  }

  /**
   * Start spectrum monitoring
   */
  async start(): Promise<void> {
    if (this.spectrumAnalyzer) {
      try {
        await this.spectrumAnalyzer.start();
        this.emit("started", {});
      } catch (error) {
        console.error("[SpectrumPlugin] Failed to start:", error);
        throw error;
      }
    }
  }

  /**
   * Stop spectrum monitoring
   */
  async stop(): Promise<void> {
    if (this.spectrumAnalyzer) {
      try {
        await this.spectrumAnalyzer.stop();
        this.emit("stopped", {});
      } catch (error) {
        console.error("[SpectrumPlugin] Failed to stop:", error);
      }
    }
  }

  /**
   * Check if analyzer is active
   */
  isActive(): boolean {
    return this.spectrumAnalyzer?.isActive() ?? false;
  }

  /**
   * Get current spectrum data
   */
  getSpectrum() {
    return this.spectrumAnalyzer?.getSpectrum() ?? null;
  }

  /**
   * Get parameters
   */
  getParameters() {
    return {
      minFreq: this.minFreq,
      maxFreq: this.maxFreq,
      dbRange: this.dbRange,
      pollInterval: this.pollInterval,
      colorScheme: this.colorScheme,
    };
  }

  /**
   * Set parameters
   */
  setParameters(
    params: Partial<{
      minFreq: number;
      maxFreq: number;
      dbRange: number;
      pollInterval: number;
      colorScheme: "light" | "dark";
    }>,
  ): void {
    if (params.minFreq !== undefined) this.minFreq = params.minFreq;
    if (params.maxFreq !== undefined) this.maxFreq = params.maxFreq;
    if (params.dbRange !== undefined) this.dbRange = params.dbRange;
    if (params.pollInterval !== undefined)
      this.pollInterval = params.pollInterval;
    if (params.colorScheme !== undefined) this.colorScheme = params.colorScheme;

    // Recreate analyzer with new settings
    if (this.spectrumAnalyzer) {
      this.recreateAnalyzer();
    }
  }

  /**
   * Resize handler
   */
  resize(): void {
    if (this.spectrumAnalyzer) {
      this.spectrumAnalyzer.resize();
    }
  }

  /**
   * Get keyboard shortcuts for this plugin
   */
  getShortcuts() {
    return [
      { key: "1-4", description: "Select parameter" },
      { key: "Tab", description: "Next parameter" },
      { key: "Esc", description: "Clear selection" },
      { key: "Shift+↑↓", description: "Adjust value" },
    ];
  }

  /**
   * Destroy the plugin
   */
  destroy(): void {
    if (this.spectrumAnalyzer) {
      this.spectrumAnalyzer.destroy();
      this.spectrumAnalyzer = null;
    }

    if (this.menubar) {
      this.menubar.destroy();
      this.menubar = null;
    }

    super.destroy();
  }
}
