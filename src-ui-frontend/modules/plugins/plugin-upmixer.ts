// Upmixer Plugin
// Stereo to 5.0 surround upmixer with level metering

import { BasePlugin } from "./plugin-base";
import { PluginMenubar } from "./plugin-menubar";
import { LevelMeter } from "./level-meter";
import type { PluginMetadata, LevelMeterData } from "./plugin-types";
import {
  SPEAKER_CONFIGS,
  getAvailableConfigs,
  getSpeakerConfig,
  type SpeakerConfig,
} from "./speaker-configs";

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
 * Converts stereo (2ch) to multi-channel surround with configurable speaker layouts
 */
export class UpmixerPlugin extends BasePlugin {
  public readonly metadata: PluginMetadata = {
    id: "upmixer-plugin",
    name: "SotF: Upmixer",
    category: "spatial",
    version: "2.0.0",
    hasBuiltInLevelMeters: true,
  };

  // UI components
  private menubar: PluginMenubar | null = null;
  private inputMeter: LevelMeter | null = null;
  private outputMeter: LevelMeter | null = null;

  // UI elements
  private parametersContainer: HTMLElement | null = null;
  private configSelectorContainer: HTMLElement | null = null;
  private muteButtons: Map<string, HTMLButtonElement> = new Map();
  private soloButtons: Map<string, HTMLButtonElement> = new Map();

  // Speaker configuration
  private currentConfig: SpeakerConfig = SPEAKER_CONFIGS["5.1"];
  private channelGroups: ChannelGroup[] = [];

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
   * Initialize channel groups based on current speaker configuration
   */
  private initializeChannelGroups(): void {
    // Group speakers by their position characteristics
    const groups: Map<string, number[]> = new Map();

    for (const speaker of this.currentConfig.speakers) {
      if (speaker.isLFE) {
        groups.set("LFE", [speaker.channel]);
        continue;
      }

      // Group by position
      if (speaker.elevation > 0) {
        // Height channels
        const key = "Height";
        if (!groups.has(key)) groups.set(key, []);
        groups.get(key)!.push(speaker.channel);
      } else if (Math.abs(speaker.azimuth) <= 45) {
        // Front channels
        const key = "Front";
        if (!groups.has(key)) groups.set(key, []);
        groups.get(key)!.push(speaker.channel);
      } else if (Math.abs(speaker.azimuth) >= 135) {
        // Back channels
        const key = "Back";
        if (!groups.has(key)) groups.set(key, []);
        groups.get(key)!.push(speaker.channel);
      } else {
        // Side/Wide channels
        const key = "Side";
        if (!groups.has(key)) groups.set(key, []);
        groups.get(key)!.push(speaker.channel);
      }
    }

    // Convert to channel groups
    this.channelGroups = Array.from(groups.entries()).map(([name, channels]) => ({
      name,
      channels,
      muted: false,
      solo: false,
    }));
  }

  /**
   * Render the plugin UI
   */
  render(standalone: boolean): void {
    if (!this.container) return;

    // Initialize channel groups if not done
    if (this.channelGroups.length === 0) {
      this.initializeChannelGroups();
    }

    this.container.innerHTML = `
      <div class="upmixer-plugin ${standalone ? "standalone" : "embedded"} has-background-dark p-4" style="max-height: 700px;">
        ${standalone ? '<div class="upmixer-menubar-container"></div>' : ""}

        <!-- Configuration Selector -->
        <div class="upmixer-config-selector mb-3"></div>

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
              <div class="has-text-centered has-text-weight-semibold mb-2 has-text-light is-size-7">
                Output (${this.currentConfig.name})
              </div>
              <canvas class="upmixer-output-meters" width="${this.currentConfig.totalChannels * 20}" height="250"></canvas>
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
    this.configSelectorContainer = this.container.querySelector(
      ".upmixer-config-selector",
    ) as HTMLElement;

    // Render config selector
    this.renderConfigSelector();

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
        channels: this.currentConfig.totalChannels,
        channelLabels: this.currentConfig.speakers.map((s) => s.label),
      });
    }

    // Render parameters
    this.renderParameters();
    this.attachEventListeners();

    // Setup UI enhancements after render
    setTimeout(() => this.postRender(), 100);
  }

  /**
   * Render speaker configuration selector
   */
  private renderConfigSelector(): void {
    if (!this.configSelectorContainer) return;

    const availableConfigs = getAvailableConfigs();

    this.configSelectorContainer.innerHTML = `
      <div class="field">
        <label class="label has-text-light is-size-7">Speaker Configuration</label>
        <div class="control">
          <div class="select is-small is-fullwidth is-dark">
            <select class="config-select has-background-dark has-text-light">
              ${availableConfigs
                .map(
                  (id) => `
                <option value="${id}" ${id === this.currentConfig.id ? "selected" : ""}>
                  ${SPEAKER_CONFIGS[id].name} - ${SPEAKER_CONFIGS[id].description}
                </option>
              `,
                )
                .join("")}
            </select>
          </div>
        </div>
      </div>
    `;

    // Attach event listener
    const selectElement = this.configSelectorContainer.querySelector(
      ".config-select",
    ) as HTMLSelectElement;
    if (selectElement) {
      selectElement.addEventListener("change", (e) => {
        const newConfigId = (e.target as HTMLSelectElement).value;
        this.changeConfiguration(newConfigId);
      });
    }
  }

  /**
   * Change speaker configuration
   */
  private changeConfiguration(configId: string): void {
    const newConfig = getSpeakerConfig(configId);
    if (!newConfig) {
      console.error(`Invalid config ID: ${configId}`);
      return;
    }

    this.currentConfig = newConfig;
    this.initializeChannelGroups();

    // Notify backend of configuration change
    this.emit("configurationChange", {
      config: this.currentConfig.id,
      channels: this.currentConfig.totalChannels,
    });

    // Re-render the plugin
    this.render(this.container?.querySelector(".standalone") !== null);
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
    const numChannels = this.currentConfig.totalChannels;
    const meterWidth = canvasWidth / numChannels;

    // Replace output meter labels with Bulma tags
    const meterLabelsOutput = this.container?.querySelector(
      ".meter-labels-output",
    );
    if (meterLabelsOutput) {
      meterLabelsOutput.innerHTML = "";
      meterLabelsOutput.className =
        "meter-labels-output is-flex is-justify-content-flex-start is-flex-wrap-wrap";

      // Create label for each speaker
      const colors = ["is-info", "is-success", "is-warning", "is-danger", "is-primary", "is-link"];

      this.currentConfig.speakers.forEach((speaker, idx) => {
        const tag = document.createElement("span");
        const colorIdx = speaker.isLFE ? 2 : Math.floor(idx / 2) % colors.length;
        tag.className = `tag is-small ${colors[colorIdx]} upmixer-channel-tag`;
        tag.textContent = speaker.label;
        tag.style.width = meterWidth + "px";
        tag.style.fontSize = "0.65rem";
        tag.title = speaker.name;
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

      // Map channel groups to visual groups
      const colors = ["is-info", "is-success", "is-warning", "is-danger", "is-primary", "is-link"];
      const visualGroups = this.channelGroups.map((group, idx) => ({
        channels: group.channels.length,
        color: group.name === "LFE" ? "is-warning" : colors[idx % colors.length],
        indices: [idx],
        name: group.name,
      }));

      // Create mute row
      const muteRow = document.createElement("div");
      muteRow.className = "is-flex is-justify-content-flex-start mt-1 is-flex-wrap-wrap";

      visualGroups.forEach((group) => {
        const container = document.createElement("div");
        container.className =
          "is-flex is-justify-content-center upmixer-button-container";
        container.style.minWidth = "40px";

        group.indices.forEach((idx) => {
          if (controlGroups[idx]) {
            const muteBtn = controlGroups[idx]
              .querySelector(".mute-btn")
              ?.cloneNode(true) as HTMLButtonElement;
            if (muteBtn) {
              muteBtn.className = `tag is-small ${group.color} mute-btn is-clickable has-text-white`;
              muteBtn.textContent = "M";
              muteBtn.title = `Mute ${group.name}`;
              muteBtn.dataset.groupIndex = idx.toString();
              container.appendChild(muteBtn);
            }
          }
        });

        muteRow.appendChild(container);
      });

      // Create solo row
      const soloRow = document.createElement("div");
      soloRow.className = "is-flex is-justify-content-flex-start mt-1 is-flex-wrap-wrap";

      visualGroups.forEach((group) => {
        const container = document.createElement("div");
        container.className =
          "is-flex is-justify-content-center upmixer-button-container";
        container.style.minWidth = "40px";

        group.indices.forEach((idx) => {
          if (controlGroups[idx]) {
            const soloBtn = controlGroups[idx]
              .querySelector(".solo-btn")
              ?.cloneNode(true) as HTMLButtonElement;
            if (soloBtn) {
              soloBtn.className = `tag is-small ${group.color} solo-btn is-clickable has-text-white`;
              soloBtn.textContent = "S";
              soloBtn.title = `Solo ${group.name}`;
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
   * Get current speaker configuration
   */
  getCurrentConfiguration(): SpeakerConfig {
    return this.currentConfig;
  }

  /**
   * Set speaker configuration
   */
  setConfiguration(configId: string): void {
    this.changeConfiguration(configId);
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
