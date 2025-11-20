// RoomEQ Wizard UI Component
// Multi-step wizard for configuring and running room EQ optimization

import type {
  RoomConfigType,
  ChannelConfig,
  RoomEQWorkflowState,
  MeasurementSource,
  DriverConfig,
  CrossoverConfig,
} from "../types/roomeq";
import type { OptimizationParams } from "../types/optimization";
import { ROOMEQ_PRESETS, CROSSOVER_TYPES } from "../types/roomeq";
import { RoomEQManager } from "./roomeq-manager";
import { OPTIMIZATION_DEFAULTS } from "./optimization-constants";

export interface RoomEQWizardConfig {
  container: HTMLElement;
  onComplete?: () => void;
  onCancel?: () => void;
}

export class RoomEQWizard {
  private container: HTMLElement;
  private manager: RoomEQManager;
  private config: RoomEQWizardConfig;
  private currentState: RoomEQWorkflowState | null = null;

  constructor(config: RoomEQWizardConfig) {
    this.container = config.container;
    this.config = config;
    this.manager = new RoomEQManager();

    // Setup manager callbacks
    this.manager.setCallbacks({
      onWorkflowStateChange: (state) => this.handleStateChange(state),
      onChannelProgress: (index, name, stage, progress) =>
        this.handleChannelProgress(index, name, stage, progress),
      onOptimizationComplete: (result) => this.handleOptimizationComplete(result),
      onError: (error) => this.handleError(error),
    });

    this.render();
  }

  /**
   * Render the wizard
   */
  private render(): void {
    const state = this.manager.getState();
    this.currentState = state;

    this.container.innerHTML = this.generateHTML(state);
    this.attachEventListeners();
  }

  /**
   * Generate HTML for current step
   */
  private generateHTML(state: RoomEQWorkflowState): string {
    switch (state.step) {
      case 0:
        return this.renderStep0_ConfigType();
      case 1:
        return this.renderStep1_ChannelSetup(state);
      case 2:
        return this.renderStep2_OptimizerSettings(state);
      case 3:
        return this.renderStep3_Progress(state);
      case 4:
        return this.renderStep4_Results(state);
      default:
        return "<div>Unknown step</div>";
    }
  }

  /**
   * Step 0: Configuration Type Selection
   */
  private renderStep0_ConfigType(): string {
    return `
      <div class="roomeq-wizard">
        <div class="wizard-header">
          <h2>Room EQ Configuration</h2>
          <p>What would you like to optimize?</p>
        </div>

        <div class="wizard-content">
          <div class="preset-grid">
            ${ROOMEQ_PRESETS.map(
              (preset, index) => `
              <div class="preset-card" data-preset-index="${index}">
                <div class="preset-icon">${preset.icon || "🔊"}</div>
                <h3>${preset.name}</h3>
                <p>${preset.description}</p>
              </div>
            `,
            ).join("")}
          </div>
        </div>

        <div class="wizard-footer">
          <button type="button" class="btn-secondary" data-action="cancel">Cancel</button>
        </div>
      </div>
    `;
  }

  /**
   * Step 1: Channel Setup
   */
  private renderStep1_ChannelSetup(state: RoomEQWorkflowState): string {
    const channels = state.channels || [];
    const isMultiWay =
      state.config_type?.type === "multi_way" || false;

    return `
      <div class="roomeq-wizard">
        <div class="wizard-header">
          <h2>Channel Setup</h2>
          <p>Configure measurement sources for each channel</p>
          <div class="wizard-progress">Step 2 of 5</div>
        </div>

        <div class="wizard-content">
          ${channels
            .map(
              (channel, index) => `
            <div class="channel-config-card" data-channel-index="${index}">
              <h3>${channel.config.channel_name}</h3>

              ${
                isMultiWay
                  ? this.renderMultiWayChannelSetup(channel.config, index)
                  : this.renderSimpleChannelSetup(channel.config, index)
              }
            </div>
          `,
            )
            .join("")}

          ${
            state.config_type?.type === "multi_channel"
              ? `
            <button type="button" class="btn-secondary" data-action="add-channel">
              + Add Channel
            </button>
          `
              : ""
          }
        </div>

        <div class="wizard-footer">
          <button type="button" class="btn-secondary" data-action="back">Back</button>
          <button type="button" class="btn-primary" data-action="next">Next</button>
        </div>
      </div>
    `;
  }

  /**
   * Render simple channel setup (single measurement)
   */
  private renderSimpleChannelSetup(config: ChannelConfig, index: number): string {
    return `
      <div class="measurement-source">
        <label>
          <input type="radio" name="ch${index}_source" value="database" ${!config.measurement || (config.measurement as any).source_type === "database" ? "checked" : ""}>
          <span>From Spinorama Database</span>
        </label>
        <div class="source-config" data-source="database">
          <select name="ch${index}_speaker" class="speaker-select">
            <option value="">-- Select Speaker --</option>
          </select>
          <select name="ch${index}_version" class="version-select" disabled>
            <option value="">-- Select Version --</option>
          </select>
          <select name="ch${index}_measurement" class="measurement-select" disabled>
            <option value="">-- Select Measurement --</option>
          </select>
        </div>

        <label>
          <input type="radio" name="ch${index}_source" value="file">
          <span>From CSV File</span>
        </label>
        <div class="source-config" data-source="file">
          <input type="text" name="ch${index}_file_path" placeholder="/path/to/measurement.csv">
          <button type="button" class="btn-small" data-action="browse-file" data-channel="${index}">Browse</button>
        </div>

        <label>
          <input type="radio" name="ch${index}_source" value="captured">
          <span>From Captured Audio</span>
        </label>
        <div class="source-config" data-source="captured">
          <p class="hint">Use the audio capture feature to record and use live measurements</p>
        </div>
      </div>
    `;
  }

  /**
   * Render multi-way channel setup (multiple drivers + crossover)
   */
  private renderMultiWayChannelSetup(config: ChannelConfig, index: number): string {
    const drivers = config.drivers || [];
    const crossover = config.crossover || {
      crossover_type: "LR24",
      optimize: true,
    };

    return `
      <div class="multiway-setup">
        <h4>Drivers</h4>
        ${drivers
          .map(
            (driver, driverIndex) => `
          <div class="driver-config" data-driver-index="${driverIndex}">
            <label>Driver ${driverIndex + 1} (${driver.name})</label>
            <input type="text" name="ch${index}_driver${driverIndex}_file" placeholder="/path/to/driver.csv" value="">
            <button type="button" class="btn-small" data-action="browse-driver" data-channel="${index}" data-driver="${driverIndex}">Browse</button>
            <button type="button" class="btn-small btn-danger" data-action="remove-driver" data-channel="${index}" data-driver="${driverIndex}">Remove</button>
          </div>
        `,
          )
          .join("")}

        <button type="button" class="btn-secondary btn-small" data-action="add-driver" data-channel="${index}">
          + Add Driver
        </button>

        <h4>Crossover</h4>
        <div class="crossover-config">
          <label>Type</label>
          <select name="ch${index}_crossover_type">
            ${CROSSOVER_TYPES.map(
              (type) => `
              <option value="${type.value}" ${crossover.crossover_type === type.value ? "selected" : ""}>
                ${type.label}
              </option>
            `,
            ).join("")}
          </select>

          <label>
            <input type="checkbox" name="ch${index}_optimize_crossover" ${crossover.optimize ? "checked" : ""}>
            <span>Optimize crossover frequency</span>
          </label>

          <div class="crossover-freq" ${crossover.optimize ? 'style="display:none"' : ""}>
            <label>Fixed Frequency (Hz)</label>
            <input type="number" name="ch${index}_crossover_freq" value="${crossover.frequency || 2500}" step="100">
          </div>
        </div>
      </div>
    `;
  }

  /**
   * Step 2: Optimizer Settings
   */
  private renderStep2_OptimizerSettings(state: RoomEQWorkflowState): string {
    const params: Partial<OptimizationParams> = state.optimizer_params || {};

    return `
      <div class="roomeq-wizard">
        <div class="wizard-header">
          <h2>Optimization Settings</h2>
          <p>Configure EQ optimization parameters</p>
          <div class="wizard-progress">Step 3 of 5</div>
        </div>

        <div class="wizard-content">
          <div class="param-grid">
            <div class="param-item">
              <label>Number of Filters</label>
              <input type="number" name="num_filters" value="${params.num_filters || OPTIMIZATION_DEFAULTS.num_filters}" min="1" max="20">
            </div>

            <div class="param-item">
              <label>Algorithm</label>
              <select name="algo">
                <option value="nlopt:cobyla" ${params.algo === "nlopt:cobyla" ? "selected" : ""}>COBYLA (Fast)</option>
                <option value="autoeq:de" ${params.algo === "autoeq:de" ? "selected" : ""}>Differential Evolution (Recommended)</option>
                <option value="nlopt:isres" ${params.algo === "nlopt:isres" ? "selected" : ""}>ISRES</option>
              </select>
            </div>

            <div class="param-item">
              <label>Min Frequency (Hz)</label>
              <input type="number" name="min_freq" value="${params.min_freq || OPTIMIZATION_DEFAULTS.min_freq}" step="10">
            </div>

            <div class="param-item">
              <label>Max Frequency (Hz)</label>
              <input type="number" name="max_freq" value="${params.max_freq || OPTIMIZATION_DEFAULTS.max_freq}" step="100">
            </div>

            <div class="param-item">
              <label>Min Q</label>
              <input type="number" name="min_q" value="${params.min_q || OPTIMIZATION_DEFAULTS.min_q}" step="0.1">
            </div>

            <div class="param-item">
              <label>Max Q</label>
              <input type="number" name="max_q" value="${params.max_q || OPTIMIZATION_DEFAULTS.max_q}" step="0.1">
            </div>

            <div class="param-item">
              <label>Min Gain (dB)</label>
              <input type="number" name="min_db" value="${params.min_db || OPTIMIZATION_DEFAULTS.min_db}" step="0.5">
            </div>

            <div class="param-item">
              <label>Max Gain (dB)</label>
              <input type="number" name="max_db" value="${params.max_db || OPTIMIZATION_DEFAULTS.max_db}" step="0.5">
            </div>
          </div>

          <details class="advanced-params">
            <summary>Advanced Parameters</summary>
            <div class="param-grid">
              <div class="param-item">
                <label>Population Size</label>
                <input type="number" name="population" value="${params.population || OPTIMIZATION_DEFAULTS.population}" min="10">
              </div>

              <div class="param-item">
                <label>Max Evaluations</label>
                <input type="number" name="maxeval" value="${params.maxeval || OPTIMIZATION_DEFAULTS.maxeval}" min="100">
              </div>

              <div class="param-item">
                <label>Sample Rate (Hz)</label>
                <input type="number" name="sample_rate" value="${params.sample_rate || OPTIMIZATION_DEFAULTS.sample_rate}" step="1000">
              </div>
            </div>
          </details>
        </div>

        <div class="wizard-footer">
          <button type="button" class="btn-secondary" data-action="back">Back</button>
          <button type="button" class="btn-primary" data-action="start-optimization">Start Optimization</button>
        </div>
      </div>
    `;
  }

  /**
   * Step 3: Progress Display
   */
  private renderStep3_Progress(state: RoomEQWorkflowState): string {
    const channels = state.channels || [];
    const currentIndex = state.current_channel_index ?? -1;

    return `
      <div class="roomeq-wizard">
        <div class="wizard-header">
          <h2>Optimization Progress</h2>
          <p>Optimizing ${channels.length} channel(s)...</p>
          <div class="wizard-progress">Step 4 of 5</div>
        </div>

        <div class="wizard-content">
          <div class="channel-progress-list">
            ${channels
              .map(
                (channel, index) => `
              <div class="channel-progress-item ${index === currentIndex ? "active" : ""}" data-channel-index="${index}">
                <div class="channel-status-icon">
                  ${this.getStatusIcon(channel.status)}
                </div>
                <div class="channel-info">
                  <h4>${channel.config.channel_name}</h4>
                  <p class="stage">${channel.stage || "Waiting..."}</p>
                  ${
                    channel.status === "optimizing"
                      ? `
                    <div class="progress-bar">
                      <div class="progress-fill" style="width: ${channel.progress || 0}%"></div>
                    </div>
                    <p class="progress-text">${Math.round(channel.progress || 0)}%</p>
                  `
                      : ""
                  }
                </div>
              </div>
            `,
              )
              .join("")}
          </div>
        </div>

        <div class="wizard-footer">
          <button type="button" class="btn-danger" data-action="cancel-optimization">Cancel</button>
        </div>
      </div>
    `;
  }

  /**
   * Step 4: Results Display
   */
  private renderStep4_Results(state: RoomEQWorkflowState): string {
    const result = state.overall_result;
    if (!result) {
      return "<div>No results available</div>";
    }

    return `
      <div class="roomeq-wizard">
        <div class="wizard-header">
          <h2>Optimization Complete</h2>
          <p>${result.success ? "All channels optimized successfully!" : "Some channels failed"}</p>
          <div class="wizard-progress">Step 5 of 5</div>
        </div>

        <div class="wizard-content">
          <div class="results-summary">
            ${result.channel_results
              .map(
                (channelResult) => `
              <div class="channel-result ${channelResult.success ? "success" : "error"}">
                <h4>${channelResult.channel_name}</h4>
                ${
                  channelResult.success && channelResult.optimization_result
                    ? `
                  <div class="scores">
                    <div class="score-item">
                      <label>Before:</label>
                      <span>${channelResult.optimization_result.preference_score_before?.toFixed(2) || "N/A"}</span>
                    </div>
                    <div class="score-item">
                      <label>After:</label>
                      <span>${channelResult.optimization_result.preference_score_after?.toFixed(2) || "N/A"}</span>
                    </div>
                    ${
                      channelResult.optimization_result.preference_score_before &&
                      channelResult.optimization_result.preference_score_after
                        ? `
                      <div class="score-item improvement">
                        <label>Improvement:</label>
                        <span>+${(
                          channelResult.optimization_result.preference_score_after -
                          channelResult.optimization_result.preference_score_before
                        ).toFixed(2)}</span>
                      </div>
                    `
                        : ""
                    }
                  </div>
                `
                    : `<p class="error">${channelResult.error_message || "Optimization failed"}</p>`
                }
              </div>
            `,
              )
              .join("")}
          </div>

          <div class="export-options">
            <h3>Export Options</h3>
            <button type="button" class="btn-secondary" data-action="export-json">Export JSON</button>
            <button type="button" class="btn-secondary" data-action="export-apo">Export APO Config</button>
            <button type="button" class="btn-secondary" data-action="export-camilla">Export CamillaDSP</button>
          </div>
        </div>

        <div class="wizard-footer">
          <button type="button" class="btn-secondary" data-action="restart">Start New Optimization</button>
          <button type="button" class="btn-primary" data-action="finish">Finish</button>
        </div>
      </div>
    `;
  }

  /**
   * Get status icon for channel
   */
  private getStatusIcon(status: string): string {
    switch (status) {
      case "pending":
        return "⏸";
      case "optimizing":
        return "⏳";
      case "complete":
        return "✅";
      case "error":
        return "❌";
      default:
        return "○";
    }
  }

  /**
   * Attach event listeners
   */
  private attachEventListeners(): void {
    // Preset selection
    this.container.querySelectorAll(".preset-card").forEach((card) => {
      card.addEventListener("click", (e) => {
        const index = parseInt(
          (e.currentTarget as HTMLElement).dataset.presetIndex || "0",
        );
        this.selectPreset(index);
      });
    });

    // Navigation buttons
    this.container.querySelectorAll("[data-action]").forEach((btn) => {
      btn.addEventListener("click", (e) => {
        const action = (e.currentTarget as HTMLElement).dataset.action;
        this.handleAction(action || "");
      });
    });

    // Radio buttons for measurement source
    this.container.querySelectorAll("input[type=radio]").forEach((radio) => {
      radio.addEventListener("change", () => this.updateMeasurementSourceUI());
    });
  }

  /**
   * Select a preset configuration
   */
  private selectPreset(index: number): void {
    const preset = ROOMEQ_PRESETS[index];
    if (preset) {
      this.manager.initializeWorkflow(preset.config_type, preset.channel_names);
    }
  }

  /**
   * Handle action button clicks
   */
  private async handleAction(action: string): Promise<void> {
    switch (action) {
      case "cancel":
        if (this.config.onCancel) {
          this.config.onCancel();
        }
        break;

      case "back":
        this.manager.previousStep();
        break;

      case "next":
        this.collectChannelSetupData();
        this.manager.nextStep();
        break;

      case "start-optimization":
        this.collectOptimizerParams();
        this.manager.nextStep(); // Move to progress step
        await this.manager.runOptimization();
        this.manager.nextStep(); // Move to results step
        break;

      case "cancel-optimization":
        await this.manager.cancelOptimization();
        this.manager.goToStep(2); // Back to settings
        break;

      case "restart":
        this.manager.reset();
        break;

      case "finish":
        if (this.config.onComplete) {
          this.config.onComplete();
        }
        break;

      default:
        console.warn("Unknown action:", action);
    }
  }

  /**
   * Collect channel setup data from form
   */
  private collectChannelSetupData(): void {
    // TODO: Implement form data collection
    console.log("[ROOMEQ] Collecting channel setup data...");
  }

  /**
   * Collect optimizer parameters from form
   */
  private collectOptimizerParams(): void {
    const formData: Record<string, any> = {};

    this.container.querySelectorAll("input, select").forEach((el) => {
      const element = el as HTMLInputElement | HTMLSelectElement;
      if (element.name) {
        formData[element.name] =
          element.type === "checkbox"
            ? (element as HTMLInputElement).checked
            : element.value;
      }
    });

    this.manager.setOptimizerParams({
      num_filters: parseInt(formData.num_filters) || OPTIMIZATION_DEFAULTS.num_filters,
      algo: formData.algo || OPTIMIZATION_DEFAULTS.algo,
      min_freq: parseFloat(formData.min_freq) || OPTIMIZATION_DEFAULTS.min_freq,
      max_freq: parseFloat(formData.max_freq) || OPTIMIZATION_DEFAULTS.max_freq,
      min_q: parseFloat(formData.min_q) || OPTIMIZATION_DEFAULTS.min_q,
      max_q: parseFloat(formData.max_q) || OPTIMIZATION_DEFAULTS.max_q,
      min_db: parseFloat(formData.min_db) || OPTIMIZATION_DEFAULTS.min_db,
      max_db: parseFloat(formData.max_db) || OPTIMIZATION_DEFAULTS.max_db,
      population: parseInt(formData.population) || OPTIMIZATION_DEFAULTS.population,
      maxeval: parseInt(formData.maxeval) || OPTIMIZATION_DEFAULTS.maxeval,
      sample_rate: parseFloat(formData.sample_rate) || OPTIMIZATION_DEFAULTS.sample_rate,
    } as any);
  }

  /**
   * Update measurement source UI based on radio selection
   */
  private updateMeasurementSourceUI(): void {
    // Hide/show appropriate source config sections
    this.container.querySelectorAll(".source-config").forEach((config) => {
      const parent = config.closest(".measurement-source");
      const checked = parent?.querySelector("input[type=radio]:checked");
      if (checked) {
        const value = (checked as HTMLInputElement).value;
        const matchingConfig = config.getAttribute("data-source") === value;
        (config as HTMLElement).style.display = matchingConfig ? "block" : "none";
      }
    });
  }

  /**
   * Handle state change from manager
   */
  private handleStateChange(state: RoomEQWorkflowState): void {
    this.render();
  }

  /**
   * Handle channel progress update
   */
  private handleChannelProgress(
    index: number,
    name: string,
    stage: string,
    progress: number,
  ): void {
    console.log(
      `[ROOMEQ] Channel ${index} (${name}) - ${stage}: ${progress.toFixed(1)}%`,
    );
    this.render(); // Re-render to update progress
  }

  /**
   * Handle optimization complete
   */
  private handleOptimizationComplete(result: any): void {
    console.log("[ROOMEQ] Optimization complete:", result);
  }

  /**
   * Handle error
   */
  private handleError(error: string): void {
    console.error("[ROOMEQ] Error:", error);
    alert(`Error: ${error}`);
  }

  /**
   * Destroy the wizard
   */
  destroy(): void {
    this.manager.destroy();
    this.container.innerHTML = "";
  }
}
