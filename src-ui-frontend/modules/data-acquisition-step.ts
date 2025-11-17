// Data Acquisition Step Component
// Wraps the existing data acquisition UI for use in the step-by-step workflow

import "@audio-capture/capture-config-panel";
import "@audio-capture/capture-recording-panel";

export type DataSource = "file" | "speaker" | "headphone" | "capture";
export type CaptureStep = "config" | "recording";

export interface DataAcquisitionConfig {
  onDataReady?: (source: DataSource) => void;
  onSourceChange?: (source: DataSource) => void;
}

export class DataAcquisitionStep {
  private container: HTMLElement;
  private config: DataAcquisitionConfig;
  private currentSource: DataSource = "file";
  private currentCaptureStep: CaptureStep = "config";

  constructor(container: HTMLElement, config: DataAcquisitionConfig = {}) {
    this.container = container;
    this.config = config;
    this.render();
    this.attachEventListeners();
  }

  /**
   * Render the data acquisition UI
   */
  private render(): void {
    this.container.classList.add("data-acquisition-step");
    this.container.innerHTML = this.generateHTML();
  }

  /**
   * Generate HTML for the data acquisition step
   */
  private generateHTML(): string {
    return `
      <div class="step-content-wrapper">
        <div class="step-header-section">
          <h2 class="step-title">Data Acquisition</h2>
          <p class="step-description">
            Select your data source and provide the necessary measurements or configuration.
          </p>
        </div>

        <div class="data-acquisition-content">
          <!-- Data Source Tabs -->
          <div class="section-group">
            <h3>Select Data Source</h3>
            <div class="input-source-tabs">
              <label class="tab-label active" data-tab="file" title="CSV Files">
                <input type="radio" name="input_source" value="file" checked />
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"></path>
                  <polyline points="13 2 13 9 20 9"></polyline>
                </svg>
                <span class="tab-label-text">CSV Files</span>
              </label>
              <label class="tab-label" data-tab="speaker" title="Speakers">
                <input type="radio" name="input_source" value="speaker" />
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <rect width="16" height="20" x="4" y="2" rx="2"></rect>
                  <path d="M12 6h.01"></path>
                  <circle cx="12" cy="14" r="4"></circle>
                  <path d="M12 14h.01"></path>
                </svg>
                <span class="tab-label-text">Speakers</span>
              </label>
              <label class="tab-label" data-tab="headphone" title="Headphones">
                <input type="radio" name="input_source" value="headphone" />
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M3 18v-6a9 9 0 0 1 18 0v6"></path>
                  <path d="M21 19a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3zM3 19a2 2 0 0 0 2 2h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H3z"></path>
                </svg>
                <span class="tab-label-text">Headphones</span>
              </label>
              <label class="tab-label" data-tab="capture" title="Microphone Capture">
                <input type="radio" name="input_source" value="capture" />
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"></path>
                  <path d="M19 10v2a7 7 0 0 1-14 0v-2"></path>
                  <line x1="12" y1="19" x2="12" y2="23"></line>
                  <line x1="8" y1="23" x2="16" y2="23"></line>
                </svg>
                <span class="tab-label-text">Capture</span>
              </label>
            </div>

            <!-- CSV Files Tab Content -->
            <div id="file_inputs" class="tab-content active">
              <div class="input-group">
                <label for="curve_path">Input CSV File</label>
                <div class="compact-row">
                  <input type="text" id="curve_path" name="curve_path" placeholder="Select input measurement file..." />
                  <button type="button" id="browse_curve" class="browse-btn">Browse</button>
                </div>
                <p class="input-hint">Frequency response measurement data (freq, dB)</p>
              </div>

              <div class="input-group">
                <label for="target_path">Target CSV File (Optional)</label>
                <div class="compact-row">
                  <input type="text" id="target_path" name="target_path" placeholder="Select target curve (optional)..." />
                  <button type="button" id="browse_target" class="browse-btn">Browse</button>
                </div>
                <p class="input-hint">Leave empty to optimize for flat response</p>
              </div>
            </div>

            <!-- Speaker Tab Content -->
            <div id="speaker_inputs" class="tab-content">
              <div class="input-group">
                <label for="speaker">Speaker Name</label>
                <div class="autocomplete-container">
                  <input type="text" id="speaker" name="speaker" placeholder="Start typing speaker name..." autocomplete="off" />
                  <div id="speaker_dropdown" class="autocomplete-dropdown"></div>
                </div>
                <p class="input-hint">Search from thousands of professional measurements</p>
              </div>

              <div class="input-group">
                <label for="version">Version</label>
                <select id="version" name="version" disabled>
                  <option value="">Select Version</option>
                </select>
              </div>

              <div class="input-group">
                <label for="measurement">Measurement</label>
                <select id="measurement" name="measurement" disabled>
                  <option value="">Select Measurement</option>
                </select>
              </div>
            </div>

            <!-- Headphone Tab Content -->
            <div id="headphone_inputs" class="tab-content">
              <div class="input-group">
                <label for="headphone_curve_path">Headphone Measurement CSV</label>
                <div class="compact-row">
                  <input type="text" id="headphone_curve_path" name="headphone_curve_path" placeholder="Select headphone measurement..." />
                  <button type="button" id="browse_headphone_curve" class="browse-btn">Browse</button>
                </div>
              </div>

              <div class="input-group">
                <label for="headphone_target">Target Curve</label>
                <select id="headphone_target" name="headphone_target">
                  <option value="">Select Target...</option>
                  <option value="harman-over-ear-2018">Harman Over-Ear 2018</option>
                  <option value="harman-over-ear-2015">Harman Over-Ear 2015</option>
                  <option value="harman-over-ear-2013">Harman Over-Ear 2013</option>
                  <option value="harman-in-ear-2019">Harman In-Ear 2019</option>
                </select>
                <p class="input-hint">Industry-standard headphone target curves</p>
              </div>
            </div>

            <!-- Capture Tab Content -->
            <div id="capture_inputs" class="tab-content">
              <!-- Two-step capture workflow -->
              <div id="capture_step_container" class="capture-step-container">
                <!-- Step 1: Configuration -->
                <div id="capture_config_step" class="capture-step active" data-step="config">
                  <!-- capture-config-panel will be rendered here -->
                </div>

                <!-- Step 2: Recording -->
                <div id="capture_recording_step" class="capture-step" data-step="recording">
                  <!-- capture-recording-panel will be rendered here -->
                </div>
              </div>
            </div>
          </div>

          <!-- Info Box -->
          <div class="data-source-info">
            <div class="info-card" id="file_info">
              <h4>📄 CSV Files</h4>
              <p>Import measurement data from external tools or previous captures. Files should contain frequency (Hz) and SPL (dB) columns.</p>
            </div>

            <div class="info-card" id="speaker_info" style="display: none">
              <h4>🔊 Speaker Database</h4>
              <p>Access thousands of professional speaker measurements from <strong>spinorama.org</strong> with full CEA2034 data.</p>
            </div>

            <div class="info-card" id="headphone_info" style="display: none">
              <h4>🎧 Headphone Optimization</h4>
              <p>Optimize headphones using industry-standard Harman target curves based on listener preference research.</p>
            </div>

            <div class="info-card" id="capture_info" style="display: none">
              <h4>🎤 Live Capture</h4>
              <p>Measure your device in real-time using a calibrated microphone and test signals (sine sweeps, pink/white noise).</p>
            </div>
          </div>
        </div>

        <!-- Navigation Buttons -->
        <div class="step-actions">
          <button type="button" id="step2_prev_btn" class="btn btn-secondary btn-large">
            Previous
          </button>
          <button type="button" id="step2_next_btn" class="btn btn-primary btn-large" disabled>
            Continue to EQ Design
          </button>
        </div>
      </div>
    `;
  }

  /**
   * Attach event listeners
   */
  private attachEventListeners(): void {
    console.log("[DataAcquisitionStep] attachEventListeners called");

    // Tab switching
    const tabLabels = this.container.querySelectorAll(".tab-label");
    console.log("[DataAcquisitionStep] Found", tabLabels.length, "tab labels");

    tabLabels.forEach((label) => {
      label.addEventListener("click", () => {
        const tab = (label as HTMLElement).dataset.tab as DataSource;
        this.switchTab(tab);
      });
    });

    // Initialize capture panels immediately so they're ready
    // This ensures they exist even if the user navigates to capture tab first
    console.log("[DataAcquisitionStep] About to call renderCapturePanel");
    this.renderCapturePanel();
    console.log("[DataAcquisitionStep] renderCapturePanel returned");
  }

  /**
   * Switch to a different data source tab
   */
  private switchTab(source: DataSource): void {
    this.currentSource = source;

    // Update tab labels
    const tabLabels = this.container.querySelectorAll(".tab-label");
    tabLabels.forEach((label) => {
      if ((label as HTMLElement).dataset.tab === source) {
        label.classList.add("active");
      } else {
        label.classList.remove("active");
      }
    });

    // Update tab content
    const tabContents = this.container.querySelectorAll(".tab-content");
    tabContents.forEach((content) => {
      content.classList.remove("active");
    });
    const activeContent = this.container.querySelector(`#${source}_inputs`);
    if (activeContent) {
      activeContent.classList.add("active");
    }

    // If switching to capture tab, render the capture panel
    if (source === "capture") {
      this.renderCapturePanel();
    }

    // Update info cards
    const infoCards = this.container.querySelectorAll(".info-card");
    infoCards.forEach((card) => {
      (card as HTMLElement).style.display = "none";
    });
    const activeInfo = this.container.querySelector(`#${source}_info`);
    if (activeInfo) {
      (activeInfo as HTMLElement).style.display = "block";
    }

    // Call callback
    if (this.config.onSourceChange) {
      this.config.onSourceChange(source);
    }
  }

  /**
   * Render the capture panels into the container
   * Creates both config and recording panels for the two-step workflow
   */
  private renderCapturePanel(): void {
    console.log("[DataAcquisitionStep] renderCapturePanel called");

    // Render config panel (Step 1)
    const configContainer = this.container.querySelector(
      "#capture_config_step",
    );
    console.log(
      "[DataAcquisitionStep] configContainer found:",
      !!configContainer,
    );

    if (configContainer) {
      const existingPanel = configContainer.querySelector(
        "capture-config-panel",
      );
      console.log(
        "[DataAcquisitionStep] existing config panel:",
        !!existingPanel,
      );

      if (!existingPanel) {
        console.log(
          "[DataAcquisitionStep] Creating capture-config-panel element",
        );
        const configPanel = document.createElement("capture-config-panel");
        configContainer.appendChild(configPanel);
        console.log(
          "[DataAcquisitionStep] capture-config-panel appended to DOM",
        );

        // Listen for config completion
        configPanel.addEventListener("captureConfigComplete", ((
          event: CustomEvent,
        ) => {
          console.log("[DataAcquisitionStep] Config complete:", event.detail);
          this.switchCaptureStep("recording", event.detail.config);
        }) as EventListener);
      }
    } else {
      console.error(
        "[DataAcquisitionStep] ERROR: Could not find #capture_config_step container!",
      );
    }

    // Render recording panel (Step 2)
    const recordingContainer = this.container.querySelector(
      "#capture_recording_step",
    );
    console.log(
      "[DataAcquisitionStep] recordingContainer found:",
      !!recordingContainer,
    );

    if (recordingContainer) {
      const existingPanel = recordingContainer.querySelector(
        "capture-recording-panel",
      );
      console.log(
        "[DataAcquisitionStep] existing recording panel:",
        !!existingPanel,
      );

      if (!existingPanel) {
        console.log(
          "[DataAcquisitionStep] Creating capture-recording-panel element",
        );
        const recordingPanel = document.createElement(
          "capture-recording-panel",
        );
        recordingContainer.appendChild(recordingPanel);
        console.log(
          "[DataAcquisitionStep] capture-recording-panel appended to DOM",
        );

        // Listen for recording completion
        recordingPanel.addEventListener("captureRecordingComplete", ((
          event: CustomEvent,
        ) => {
          console.log(
            "[DataAcquisitionStep] Recording complete:",
            event.detail,
          );
          // Notify parent that data is ready
          if (this.config.onDataReady) {
            this.config.onDataReady("capture");
          }
        }) as EventListener);

        // Listen for previous button click - go back to config step
        recordingPanel.addEventListener("captureNavigatePrevious", () => {
          console.log(
            "[DataAcquisitionStep] Navigate to previous capture step",
          );
          this.switchCaptureStep("config");
        });
      }
    } else {
      console.error(
        "[DataAcquisitionStep] ERROR: Could not find #capture_recording_step container!",
      );
    }
  }

  /**
   * Switch between capture steps (config → recording)
   */
  private switchCaptureStep(step: CaptureStep, config?: any): void {
    this.currentCaptureStep = step;

    // Hide all steps
    const steps = this.container.querySelectorAll(".capture-step");
    steps.forEach((s) => s.classList.remove("active"));

    // Show the target step
    const targetStep = this.container.querySelector(`[data-step="${step}"]`);
    if (targetStep) {
      targetStep.classList.add("active");
    }

    // If switching to recording step, pass config to recording panel
    if (step === "recording" && config) {
      const recordingPanel = this.container.querySelector(
        "capture-recording-panel",
      ) as any;
      if (recordingPanel) {
        // Set the full config
        recordingPanel.setConfig(config);

        // Extract channel names from groups
        if (config.playback?.channelGroups) {
          const channelNames = config.playback.channelGroups.map(
            (g: any) => g.name,
          );
          recordingPanel.setChannels(config.playback.channels, channelNames);
        }
      }
    }
  }

  /**
   * Get the current data source
   */
  public getCurrentSource(): DataSource {
    return this.currentSource;
  }

  /**
   * Set the data source programmatically
   */
  public setSource(source: DataSource): void {
    this.switchTab(source);
  }

  /**
   * Update configuration
   */
  public updateConfig(config: Partial<DataAcquisitionConfig>): void {
    this.config = { ...this.config, ...config };
  }

  /**
   * Refresh the component
   */
  public refresh(): void {
    this.render();
    this.attachEventListeners();
  }

  /**
   * Destroy the component
   */
  public destroy(): void {
    this.container.innerHTML = "";
    this.container.classList.remove("data-acquisition-step");
  }
}
