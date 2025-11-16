// Upmixer Plugin
// Stereo to 5.0 surround upmixer with level metering

import { BasePlugin } from "./plugin-base";
import { PluginMenubar } from "./plugin-menubar";
import { LevelMeter } from "./level-meter";
import type { PluginMetadata, LevelMeterData } from "./plugin-types";

/**
 * Channel groups for mute/solo control
 */
interface ChannelGroup {
  name: string;
  channels: number[]; // Channel indices
  muted: boolean;
  solo: boolean;
}

/**
 * Upmixer Plugin
 * Converts stereo (2ch) to 5.0 surround (L, R, C, LFE, SL, SR)
 */
export class UpmixerPlugin extends BasePlugin {
  public readonly metadata: PluginMetadata = {
    id: "upmixer-plugin",
    name: "SotF: Upmixer",
    category: "spatial",
    version: "1.0.0",
    hasBuiltInLevelMeters: true,
  };

  // UI components
  private menubar: PluginMenubar | null = null;
  private inputMeter: LevelMeter | null = null;
  private outputMeter: LevelMeter | null = null;

  // UI elements
  private parametersContainer: HTMLElement | null = null;
  private muteButtons: Map<string, HTMLButtonElement> = new Map();
  private soloButtons: Map<string, HTMLButtonElement> = new Map();

  // Channel groups (for mute/solo)
  private channelGroups: ChannelGroup[] = [
    { name: "L+R", channels: [0, 1], muted: false, solo: false },
    { name: "C", channels: [2], muted: false, solo: false },
    { name: "LFE", channels: [3], muted: false, solo: false },
    { name: "SL+SR", channels: [4, 5], muted: false, solo: false },
  ];

  // Parameters
  private centerLevel: number = -3.0; // Center channel level (dB)
  private surroundLevel: number = -3.0; // Surround level (dB)
  private lfeLevel: number = 0.0; // LFE level (dB)
  private crossfeedAmount: number = 0.5; // Surround crossfeed (0-1)

  // Parameter metadata for keyboard control
  protected parameterOrder = [
    "centerLevel",
    "surroundLevel",
    "lfeLevel",
    "crossfeedAmount",
  ];
  protected parameterLabels = {
    centerLevel: "Center",
    surroundLevel: "Surround",
    lfeLevel: "LFE",
    crossfeedAmount: "Crossfeed",
  };
  private sliders: HTMLInputElement[] = [];

  /**
   * Render the plugin UI
   */
  render(standalone: boolean): void {
    if (!this.container) return;

    this.container.innerHTML = `
      <div class="upmixer-plugin ${standalone ? "standalone" : "embedded"} has-background-dark p-4" style="max-height: 650px;">
        ${standalone ? '<div class="upmixer-menubar-container"></div>' : ""}
        <div class="columns is-mobile">
          <!-- Input Meters Column -->
          <div class="column is-narrow">
            <div class="box has-background-dark">
              <div class="has-text-centered has-text-weight-semibold mb-2 has-text-light is-size-7">Input</div>
              <canvas class="upmixer-input-meters" width="50" height="250"></canvas>
              <div class="meter-labels is-flex is-justify-content-space-around mt-2">
                <span class="tag is-small is-dark">L</span>
                <span class="tag is-small is-dark">R</span>
              </div>
            </div>
          </div>

          <!-- Parameters Column -->
          <div class="column">
            <div class="box has-background-dark">
              <div class="upmixer-parameters"></div>
            </div>
          </div>

          <!-- Output Meters Column -->
          <div class="column is-narrow">
            <div class="box has-background-dark">
              <div class="has-text-centered has-text-weight-semibold mb-2 has-text-light is-size-7">Output</div>
              <canvas class="upmixer-output-meters" width="120" height="250"></canvas>
              <div class="meter-labels-output mt-2"></div>
              <!-- Mute/Solo Controls -->
              <div class="upmixer-controls">
                ${this.renderMuteSoloControls()}
              </div>
            </div>
          </div>
        </div>
      </div>
    `;

    // Initialize menubar if standalone
    if (standalone) {
      const menubarContainer = this.container.querySelector(
        ".upmixer-menubar-container",
      ) as HTMLElement;
      if (menubarContainer) {
        this.menubar = new PluginMenubar(menubarContainer, this.metadata.name);
      }
    }

    // Cache elements
    this.parametersContainer = this.container.querySelector(
      ".upmixer-parameters",
    ) as HTMLElement;

    // Initialize meters
    const inputCanvas = this.container.querySelector(
      ".upmixer-input-meters",
    ) as HTMLCanvasElement;
    if (inputCanvas) {
      this.inputMeter = new LevelMeter({
        canvas: inputCanvas,
        channels: 2,
        channelLabels: ["L", "R"],
      });
    }

    const outputCanvas = this.container.querySelector(
      ".upmixer-output-meters",
    ) as HTMLCanvasElement;
    if (outputCanvas) {
      this.outputMeter = new LevelMeter({
        canvas: outputCanvas,
        channels: 6,
        channelLabels: ["L", "R", "C", "LFE", "SL", "SR"],
      });
    }

    // Render parameters
    this.renderParameters();
    this.attachEventListeners();

    // Setup UI enhancements after render
    setTimeout(() => this.postRender(), 100);
  }

  /**
   * Render mute/solo controls (initial simple version, enhanced in postRender)
   */
  private renderMuteSoloControls(): string {
    return this.channelGroups
      .map(
        (group, idx) => `
      <div class="control-group" data-group-index="${idx}">
        <button class="control-btn mute-btn ${group.muted ? "active" : ""}" data-group-index="${idx}" title="Mute">M</button>
        <button class="control-btn solo-btn ${group.solo ? "active" : ""}" data-group-index="${idx}" title="Solo">S</button>
      </div>
    `,
      )
      .join("");
  }

  /**
   * Post-render setup for Bulma tags and layout
   */
  private postRender(): void {
    const meterCanvas = this.container?.querySelector(
      ".upmixer-output-meters",
    ) as HTMLCanvasElement;
    if (!meterCanvas) return;

    const canvasWidth = meterCanvas.getBoundingClientRect().width;
    const numChannels = 6;
    const meterWidth = canvasWidth / numChannels;

    // Replace output meter labels with Bulma tags
    const meterLabelsOutput = this.container?.querySelector(
      ".meter-labels-output",
    );
    if (meterLabelsOutput) {
      meterLabelsOutput.innerHTML = "";
      meterLabelsOutput.className =
        "meter-labels-output is-flex is-justify-content-flex-start";

      // Channel groups: [L+R] [C] [LFE] [SL+SR]
      const labelGroups = [
        { label: "L+R", channels: 2, color: "is-info" },
        { label: "C", channels: 1, color: "is-success" },
        { label: "LFE", channels: 1, color: "is-warning" },
        { label: "SL+SR", channels: 2, color: "is-danger" },
      ];

      labelGroups.forEach((group) => {
        const tag = document.createElement("span");
        tag.className = `tag is-small ${group.color} upmixer-channel-tag`;
        tag.textContent = group.label;
        tag.style.width = meterWidth * group.channels + "px";
        meterLabelsOutput.appendChild(tag);
      });
    }

    // Restructure mute/solo controls with Bulma tags
    const controlsContainer =
      this.container?.querySelector(".upmixer-controls");
    if (controlsContainer) {
      const controlGroups = Array.from(
        controlsContainer.querySelectorAll(".control-group"),
      );

      controlsContainer.innerHTML = "";
      controlsContainer.className = "is-flex is-flex-direction-column mt-2";

      // Channel groups matching labels
      const channelGroups = [
        { channels: 2, color: "is-info", indices: [0] }, // L+R
        { channels: 1, color: "is-success", indices: [1] }, // C
        { channels: 1, color: "is-warning", indices: [2] }, // LFE
        { channels: 2, color: "is-danger", indices: [3] }, // SL+SR
      ];

      // Create mute row
      const muteRow = document.createElement("div");
      muteRow.className = "is-flex is-justify-content-flex-start mt-1";

      channelGroups.forEach((group) => {
        const container = document.createElement("div");
        container.className =
          "is-flex is-justify-content-center upmixer-button-container";
        container.style.width = meterWidth * group.channels + "px";

        group.indices.forEach((idx) => {
          if (controlGroups[idx]) {
            const muteBtn = controlGroups[idx]
              .querySelector(".mute-btn")
              ?.cloneNode(true) as HTMLButtonElement;
            if (muteBtn) {
              muteBtn.className = `tag is-small ${group.color} mute-btn is-clickable has-text-white`;
              muteBtn.textContent = "M";
              muteBtn.dataset.groupIndex = idx.toString();
              container.appendChild(muteBtn);
            }
          }
        });

        muteRow.appendChild(container);
      });

      // Create solo row
      const soloRow = document.createElement("div");
      soloRow.className = "is-flex is-justify-content-flex-start mt-1";

      channelGroups.forEach((group) => {
        const container = document.createElement("div");
        container.className =
          "is-flex is-justify-content-center upmixer-button-container";
        container.style.width = meterWidth * group.channels + "px";

        group.indices.forEach((idx) => {
          if (controlGroups[idx]) {
            const soloBtn = controlGroups[idx]
              .querySelector(".solo-btn")
              ?.cloneNode(true) as HTMLButtonElement;
            if (soloBtn) {
              soloBtn.className = `tag is-small ${group.color} solo-btn is-clickable has-text-white`;
              soloBtn.textContent = "S";
              soloBtn.dataset.groupIndex = idx.toString();
              container.appendChild(soloBtn);
            }
          }
        });

        soloRow.appendChild(container);
      });

      controlsContainer.appendChild(muteRow);
      controlsContainer.appendChild(soloRow);

      // Re-attach event listeners after restructuring
      this.attachEventListeners();
    }
  }

  /**
   * Render parameter controls
   */
  private renderParameters(): void {
    if (!this.parametersContainer) return;

    const params = [
      {
        name: "centerLevel",
        value: this.centerLevel,
        min: -12,
        max: 0,
        step: 0.1,
        unit: "dB",
      },
      {
        name: "surroundLevel",
        value: this.surroundLevel,
        min: -12,
        max: 0,
        step: 0.1,
        unit: "dB",
      },
      {
        name: "lfeLevel",
        value: this.lfeLevel,
        min: -12,
        max: 0,
        step: 0.1,
        unit: "dB",
      },
      {
        name: "crossfeedAmount",
        value: this.crossfeedAmount,
        min: 0,
        max: 1,
        step: 0.01,
        unit: "%",
      },
    ];

    this.parametersContainer.innerHTML = `
      <div class="has-text-centered has-text-weight-semibold mb-4 has-text-light is-size-4">Spatial Processing</div>
      <div class="columns is-mobile is-variable is-3">
        ${params
          .map((p, idx) => {
            const displayValue =
              p.unit === "%"
                ? `${(p.value * 100).toFixed(0)}${p.unit}`
                : `${p.value.toFixed(1)} ${p.unit}`;

            // Get formatted label with keyboard shortcut
            const formattedLabel = this.getFormattedLabel(p.name);

            // Generate 6 legend values from max to min
            const legendValues = [];
            for (let i = 0; i < 6; i++) {
              const value = p.max - (i * (p.max - p.min)) / 5;
              const formatted =
                p.unit === "%"
                  ? `${(value * 100).toFixed(0)}`
                  : `${value.toFixed(1)}`;
              legendValues.push(formatted);
            }

            return `
            <div class="column parameter-field" data-param="${p.name}" data-index="${idx}">
              <div class="is-flex is-flex-direction-column is-align-items-center">
                <div class="has-text-centered has-text-weight-semibold mb-2 has-text-light is-size-5" style="min-height: 2em; display: flex; align-items: center; justify-content: center;">${formattedLabel}</div>
                <span class="tag is-success is-small mb-2 param-value" data-param="${p.name}">${displayValue}</span>
                <div class="is-flex is-align-items-center">
                  <!-- Legend on the left -->
                  <div class="is-flex is-flex-direction-column is-justify-content-space-between mr-2 has-text-grey-light is-size-7" style="height: 400px; text-align: right;">
                    ${legendValues.map((v) => `<span>${v}</span>`).join("")}
                  </div>
                  <!-- Slider -->
                  <input type="range" class="param-slider" data-param="${p.name}"
                         min="${p.min}" max="${p.max}" step="${p.step}" value="${p.value}"
                         style="writing-mode: vertical-lr; direction: rtl; width: 16px; height: 400px;" />
                </div>
              </div>
            </div>
          `;
          })
          .join("")}
      </div>
    `;

    this.attachParameterListeners();
  }

  /**
   * Attach event listeners
   */
  private attachEventListeners(): void {
    // Mute buttons
    const muteButtons = this.container?.querySelectorAll(".mute-btn") || [];
    muteButtons.forEach((btn) => {
      const index = parseInt((btn as HTMLElement).dataset.groupIndex!, 10);
      this.muteButtons.set(`group-${index}`, btn as HTMLButtonElement);

      btn.addEventListener("click", () => {
        this.toggleMute(index);
      });
    });

    // Solo buttons
    const soloButtons = this.container?.querySelectorAll(".solo-btn") || [];
    soloButtons.forEach((btn) => {
      const index = parseInt((btn as HTMLElement).dataset.groupIndex!, 10);
      this.soloButtons.set(`group-${index}`, btn as HTMLButtonElement);

      btn.addEventListener("click", () => {
        this.toggleSolo(index);
      });
    });
  }

  /**
   * Attach parameter listeners
   */
  private attachParameterListeners(): void {
    const sliders =
      this.parametersContainer?.querySelectorAll(".param-slider") || [];
    this.sliders = Array.from(sliders) as HTMLInputElement[];

    sliders.forEach((slider) => {
      slider.addEventListener("input", (e) => {
        const param = (e.target as HTMLElement).dataset.param!;
        const value = parseFloat((e.target as HTMLInputElement).value);

        // Update parameter
        (this as any)[param] = value;

        // Update display value tag
        const valueDisplay = this.parametersContainer?.querySelector(
          `.param-value[data-param="${param}"]`,
        ) as HTMLElement;
        if (valueDisplay) {
          if (param === "crossfeedAmount") {
            valueDisplay.textContent = `${(value * 100).toFixed(0)}%`;
          } else {
            valueDisplay.textContent = `${value.toFixed(1)} dB`;
          }
        }

        // Notify parameter change
        this.updateParameter(param, value);
      });
    });

    // Parameter field click to select
    const fields =
      this.parametersContainer?.querySelectorAll(".parameter-field") || [];
    fields.forEach((field) => {
      field.addEventListener("click", (e) => {
        const index = parseInt(
          (field as HTMLElement).dataset.index || "-1",
          10,
        );
        this.selectParameter(index);
      });
    });
  }

  /**
   * Select parameter by index (override base class)
   */
  protected selectParameter(index: number): void {
    super.selectParameter(index);

    // Update visual highlighting
    const fields =
      this.parametersContainer?.querySelectorAll(".parameter-field") || [];
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

    const fields =
      this.parametersContainer?.querySelectorAll(".parameter-field") || [];
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
    const currentValue = (this as any)[paramName];

    // Determine step size based on parameter
    const step = paramName === "crossfeedAmount" ? 0.01 : 0.25;

    // Calculate new value
    let newValue: number;
    if (paramName === "crossfeedAmount") {
      newValue = Math.max(
        0,
        Math.min(1, currentValue + (delta > 0 ? step : -step)),
      );
    } else {
      newValue = Math.max(
        -12,
        Math.min(0, currentValue + (delta > 0 ? step : -step)),
      );
    }

    // Update parameter
    (this as any)[paramName] = newValue;

    // Update display
    const field = this.parametersContainer?.querySelector(
      `.parameter-field[data-param="${paramName}"]`,
    );
    if (field) {
      const valueDisplay = field.querySelector(".param-value");
      if (valueDisplay) {
        if (paramName === "crossfeedAmount") {
          valueDisplay.textContent = `${(newValue * 100).toFixed(0)}%`;
        } else {
          valueDisplay.textContent = `${newValue.toFixed(1)} dB`;
        }
      }

      const slider = field.querySelector(".param-slider") as HTMLInputElement;
      if (slider) {
        slider.value = newValue.toString();
      }
    }

    // Notify parameter change
    this.updateParameter(paramName, newValue);
  }

  /**
   * Toggle mute for a channel group
   */
  private toggleMute(groupIndex: number): void {
    const group = this.channelGroups[groupIndex];
    if (!group) return;

    group.muted = !group.muted;

    // Update UI
    const btn = this.muteButtons.get(`group-${groupIndex}`);
    if (btn) {
      btn.classList.toggle("active", group.muted);
    }

    // Notify
    this.emit("groupMuteChange", { group: group.name, muted: group.muted });
  }

  /**
   * Toggle solo for a channel group
   */
  private toggleSolo(groupIndex: number): void {
    const group = this.channelGroups[groupIndex];
    if (!group) return;

    group.solo = !group.solo;

    // Update UI
    const btn = this.soloButtons.get(`group-${groupIndex}`);
    if (btn) {
      btn.classList.toggle("active", group.solo);
    }

    // Check if any group is soloed
    const anySoloed = this.channelGroups.some((g) => g.solo);

    // If solo mode is active, mute all non-soloed groups
    this.channelGroups.forEach((g, idx) => {
      if (anySoloed && !g.solo) {
        // Implicitly muted by solo
        const muteBtn = this.muteButtons.get(`group-${idx}`);
        if (muteBtn) {
          muteBtn.classList.add("implicit-mute");
        }
      } else {
        const muteBtn = this.muteButtons.get(`group-${idx}`);
        if (muteBtn) {
          muteBtn.classList.remove("implicit-mute");
        }
      }
    });

    // Notify
    this.emit("groupSoloChange", { group: group.name, solo: group.solo });
  }

  /**
   * Update input meters
   */
  updateInputMeters(data: LevelMeterData): void {
    if (this.inputMeter) {
      this.inputMeter.update(data);
    }
  }

  /**
   * Update output meters
   */
  updateOutputMeters(data: LevelMeterData): void {
    if (this.outputMeter) {
      this.outputMeter.update(data);
    }
  }

  /**
   * Get current parameters
   */
  getParameters() {
    return {
      centerLevel: this.centerLevel,
      surroundLevel: this.surroundLevel,
      lfeLevel: this.lfeLevel,
      crossfeedAmount: this.crossfeedAmount,
    };
  }

  /**
   * Set parameters
   */
  setParameters(
    params: Partial<{
      centerLevel: number;
      surroundLevel: number;
      lfeLevel: number;
      crossfeedAmount: number;
    }>,
  ): void {
    if (params.centerLevel !== undefined) this.centerLevel = params.centerLevel;
    if (params.surroundLevel !== undefined)
      this.surroundLevel = params.surroundLevel;
    if (params.lfeLevel !== undefined) this.lfeLevel = params.lfeLevel;
    if (params.crossfeedAmount !== undefined)
      this.crossfeedAmount = params.crossfeedAmount;

    this.renderParameters();
  }

  /**
   * Get channel groups
   */
  getChannelGroups(): ChannelGroup[] {
    return [...this.channelGroups];
  }

  /**
   * Get keyboard shortcuts for this plugin
   */
  getShortcuts() {
    return [
      { key: "1-4", description: "Select parameter" },
      { key: "Esc", description: "Clear selection" },
      { key: "Shift+←→", description: "Adjust value" },
    ];
  }

  /**
   * Resize handler
   */
  resize(): void {
    if (this.inputMeter) {
      this.inputMeter.resize();
    }
    if (this.outputMeter) {
      this.outputMeter.resize();
    }
  }

  /**
   * Destroy the plugin
   */
  destroy(): void {
    if (this.menubar) {
      this.menubar.destroy();
      this.menubar = null;
    }

    if (this.inputMeter) {
      this.inputMeter.destroy();
      this.inputMeter = null;
    }

    if (this.outputMeter) {
      this.outputMeter.destroy();
      this.outputMeter = null;
    }

    this.muteButtons.clear();
    this.soloButtons.clear();
    this.sliders = [];

    super.destroy();
  }
}
