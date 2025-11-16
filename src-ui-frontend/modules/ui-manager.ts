// UI management and interaction functionality

import {
  OPTIMIZATION_DEFAULTS,
  LOSS_OPTIONS,
  SPEAKER_LOSS_OPTIONS,
  HEADPHONE_LOSS_OPTIONS,
} from "./optimization-constants";
import { CaptureModalManager } from "@audio-capture/capture-modal-manager";
import { exportEQ, type ExportFormat } from "./apo-export";

export class UIManager {
  private form!: HTMLFormElement;
  private optimizeBtn!: HTMLButtonElement;
  private resetBtn!: HTMLButtonElement;
  private progressElement!: HTMLElement;
  private errorElement!: HTMLElement;

  // Modal elements
  private optimizationModal!: HTMLElement;
  private progressStatus!: HTMLElement;
  private elapsedTimeElement!: HTMLElement;
  private progressTableBody!: HTMLElement;
  private cancelOptimizationBtn!: HTMLButtonElement;
  private doneOptimizationBtn!: HTMLButtonElement;
  private modalCloseBtn!: HTMLButtonElement;
  private progressGraphElement!: HTMLElement;

  // Timing
  private optimizationStartTime: number = 0;

  // Audio testing elements
  private demoAudioSelect!: HTMLSelectElement;
  private eqOnBtn!: HTMLButtonElement;
  private eqOffBtn!: HTMLButtonElement;
  private listenBtn!: HTMLButtonElement;
  private stopBtn!: HTMLButtonElement;
  private audioStatus!: HTMLElement;
  private audioStatusText!: HTMLElement;
  private audioDuration!: HTMLElement;
  private audioPosition!: HTMLElement;
  private audioProgressFill!: HTMLElement;

  // Download APO button and format selector
  private downloadApoBtn!: HTMLButtonElement;
  private exportFormatSelect!: HTMLSelectElement;

  // Capture elements
  private captureBtn: HTMLButtonElement | null = null;
  private captureStatus: HTMLElement | null = null;
  private captureStatusText: HTMLElement | null = null;
  private captureProgressFill: HTMLElement | null = null;
  private captureWaveform: HTMLCanvasElement | null = null;
  private captureWaveformCtx: CanvasRenderingContext2D | null = null;
  private captureResult: HTMLElement | null = null;
  private captureClearBtn: HTMLButtonElement | null = null;
  private capturePlot: HTMLElement | null = null;
  private captureDeviceSelect: HTMLSelectElement | null = null;
  private sweepDurationSelect: HTMLSelectElement | null = null;
  private outputChannelSelect: HTMLSelectElement | null = null;
  private captureSampleRateSelect: HTMLSelectElement | null = null;
  private signalTypeSelect: HTMLSelectElement | null = null;

  // State
  private eqEnabled: boolean = true;
  private isResizing: boolean = false;
  private startX: number = 0;
  private startWidth: number = 0;

  // Capture modal manager
  private captureModalManager: CaptureModalManager | null = null;

  // Callbacks for external interactions
  private onCaptureComplete?: (
    frequencies: number[],
    magnitudes: number[],
  ) => void;
  private outputDeviceChangeCallback?: (deviceId: string) => void;
  private getOptimizationResult?: () => {
    filterParams: number[] | null;
    sampleRate: number | null;
    peqModel: string | null;
    lossType: string | null;
    speakerName: string | null;
  };

  constructor() {
    this.initializeElements();
    this.setupEventListeners();
    this.setupUIInteractions();
    this.setupModalEventListeners();
    this.setupResizer();
    this.initializeAudioDevices();
    this.initializeCaptureModal();
  }

  private initializeElements(): void {
    this.form = document.getElementById("autoeq_form") as HTMLFormElement;
    this.optimizeBtn = document.getElementById(
      "optimize_btn",
    ) as HTMLButtonElement;
    this.resetBtn = document.getElementById("reset_btn") as HTMLButtonElement;
    this.progressElement = document.getElementById(
      "optimization_progress",
    ) as HTMLElement;
    // Scores are now always visible in the bottom row
    this.errorElement = document.getElementById("error_display") as HTMLElement;

    // Initialize modal elements
    this.optimizationModal = document.getElementById(
      "optimization_modal",
    ) as HTMLElement;
    this.progressStatus = document.getElementById(
      "progress_status",
    ) as HTMLElement;
    this.elapsedTimeElement = document.getElementById(
      "elapsed_time",
    ) as HTMLElement;
    this.progressTableBody = document.getElementById(
      "progress_table_body",
    ) as HTMLElement;

    // Debug element initialization
    this.cancelOptimizationBtn = document.getElementById(
      "cancel_optimization",
    ) as HTMLButtonElement;
    this.doneOptimizationBtn = document.getElementById(
      "done_optimization",
    ) as HTMLButtonElement;
    this.modalCloseBtn = document.getElementById(
      "modal_close",
    ) as HTMLButtonElement;
    this.progressGraphElement = document.getElementById(
      "progress_graph",
    ) as HTMLElement;

    // Initialize audio elements
    this.demoAudioSelect = document.getElementById(
      "demo_audio_select",
    ) as HTMLSelectElement;
    this.eqOnBtn = document.getElementById("eq_on_btn") as HTMLButtonElement;
    this.eqOffBtn = document.getElementById("eq_off_btn") as HTMLButtonElement;
    this.listenBtn = document.getElementById("listen_btn") as HTMLButtonElement;

    // Check for duplicate elements
    const allListenButtons = document.querySelectorAll("#listen_btn");
    const allListenButtonsByClass = document.querySelectorAll(".listen-button");
    if (allListenButtons.length > 1) {
      console.warn(
        "Multiple elements found with ID listen_btn!",
        allListenButtons,
      );
    }

    // Add debugging to track what's disabling the button
    if (this.listenBtn) {
      const originalDisabledSetter = Object.getOwnPropertyDescriptor(
        HTMLButtonElement.prototype,
        "disabled",
      )?.set;
      if (originalDisabledSetter) {
        Object.defineProperty(this.listenBtn, "disabled", {
          set: function (value: boolean) {
            console.trace("Stack trace for disabled setter:");
            originalDisabledSetter.call(this, value);
          },
          get: function () {
            return this.hasAttribute("disabled");
          },
          configurable: true,
        });
      }
    }
    this.stopBtn = document.getElementById("stop_btn") as HTMLButtonElement;
    this.audioStatus = document.getElementById("audio_status") as HTMLElement;
    this.audioStatusText = document.getElementById(
      "audio_status_text",
    ) as HTMLElement;
    this.audioDuration = document.getElementById(
      "audio_duration",
    ) as HTMLElement;
    this.audioPosition = document.getElementById(
      "audio_position",
    ) as HTMLElement;
    this.audioProgressFill = document.getElementById(
      "audio_progress_fill",
    ) as HTMLElement;

    // Capture elements
    this.captureBtn = document.getElementById(
      "capture_btn",
    ) as HTMLButtonElement;
    this.captureStatus = document.getElementById(
      "capture_status",
    ) as HTMLElement;
    this.captureStatusText = document.getElementById(
      "capture_status_text",
    ) as HTMLElement;
    this.captureProgressFill = document.getElementById(
      "capture_progress_fill",
    ) as HTMLElement;
    this.captureWaveform = document.getElementById(
      "capture_waveform",
    ) as HTMLCanvasElement;
    this.captureWaveformCtx = this.captureWaveform
      ? this.captureWaveform.getContext("2d")
      : null;
    this.captureResult = document.getElementById(
      "capture_result",
    ) as HTMLElement;
    this.captureClearBtn = document.getElementById(
      "capture_clear",
    ) as HTMLButtonElement;
    this.capturePlot = document.getElementById("capture_plot") as HTMLElement;
    this.captureDeviceSelect = document.getElementById(
      "capture_device",
    ) as HTMLSelectElement;
    this.sweepDurationSelect = document.getElementById(
      "sweep_duration",
    ) as HTMLSelectElement;
    this.outputChannelSelect = document.getElementById(
      "output_channel",
    ) as HTMLSelectElement;
    this.captureSampleRateSelect = document.getElementById(
      "capture_sample_rate",
    ) as HTMLSelectElement;
    this.signalTypeSelect = document.getElementById(
      "signal_type",
    ) as HTMLSelectElement;

    // Initialize download APO button and format selector
    this.downloadApoBtn = document.getElementById(
      "download_apo_btn",
    ) as HTMLButtonElement;
    this.exportFormatSelect = document.getElementById(
      "export_format_select",
    ) as HTMLSelectElement;
  }

  private setupEventListeners(): void {
    // Form submission (may not exist in step-based workflow)
    if (this.form) {
      this.form.addEventListener("submit", (e) => {
        e.preventDefault();
        this.onOptimizeClick();
      });
    }

    // Reset button
    this.resetBtn.addEventListener("click", () => {
      this.resetToDefaults();
    });

    // Capture button is now deprecated - capture panel is inline in data acquisition step
    // Keep this for backward compatibility but it does nothing
    this.captureBtn?.addEventListener("click", () => {
      console.log(
        "[UIManager] Capture button clicked - panel is now inline in data acquisition step",
      );
    });

    // Clear capture button
    this.captureClearBtn?.addEventListener("click", () => {
      this.clearCaptureResults();
    });

    // Sweep duration selector
    this.sweepDurationSelect?.addEventListener("change", () => {});

    // Output channel selector
    this.outputChannelSelect?.addEventListener("change", () => {});

    // Sample rate selector
    this.captureSampleRateSelect?.addEventListener("change", () => {});

    // Signal type selector
    this.signalTypeSelect?.addEventListener("change", () => {
      const signalType = this.signalTypeSelect?.value;

      // Show/hide sweep duration based on signal type
      const durationContainer = document.getElementById(
        "sweep_duration_container",
      );
      if (durationContainer) {
        durationContainer.style.display =
          signalType === "sweep" ? "flex" : "none";
      }
    });

    // Audio control buttons
    this.eqOnBtn?.addEventListener("click", () => this.setEQEnabled(true));
    this.eqOffBtn?.addEventListener("click", () => this.setEQEnabled(false));
    this.listenBtn?.addEventListener("click", () => this.onListenClick());
    this.stopBtn?.addEventListener("click", () => this.onStopClick());

    // Download APO button
    this.downloadApoBtn?.addEventListener("click", () =>
      this.onDownloadApoClick(),
    );
  }

  private setupUIInteractions(): void {
    // Algorithm change handler
    const algoSelect = document.getElementById("algo") as HTMLSelectElement;
    if (algoSelect) {
      algoSelect.addEventListener("change", () => {
        this.updateConditionalParameters();
      });
    }

    // Input source change handler and tab switching
    const inputSourceRadios = document.querySelectorAll(
      'input[name="input_source"]',
    );
    inputSourceRadios.forEach((radio) => {
      radio.addEventListener("change", (e) => {
        const target = e.target as HTMLInputElement;
        const value = target.value;

        // Update conditional parameters
        this.updateConditionalParameters();

        // Handle tab switching
        this.switchTab(value);
      });
    });

    // Tab label click handlers
    const tabLabels = document.querySelectorAll(".tab-label");
    tabLabels.forEach((label) => {
      label.addEventListener("click", (e) => {
        const tabName = (e.currentTarget as HTMLElement).getAttribute(
          "data-tab",
        );
        if (tabName) {
          // Find and check the corresponding radio button
          const radio = document.querySelector(
            `input[name="input_source"][value="${tabName}"]`,
          ) as HTMLInputElement;
          if (radio) {
            radio.checked = true;
            this.switchTab(tabName);
            this.updateConditionalParameters();
          }
        }
      });
    });

    // Grid layout - accordion functionality removed
  }

  private setupModalEventListeners(): void {
    // Modal close handlers
    this.modalCloseBtn?.addEventListener("click", () => {
      this.closeOptimizationModal();
    });

    this.doneOptimizationBtn?.addEventListener("click", () => {
      this.closeOptimizationModal();
    });

    // Cancel optimization
    this.cancelOptimizationBtn?.addEventListener("click", () => {
      this.cancelOptimization();
    });

    // Close modal when clicking outside
    this.optimizationModal?.addEventListener("click", (e) => {
      if (e.target === this.optimizationModal) {
        this.closeOptimizationModal();
      }
    });
  }

  private setupCaptureModalEventListeners(): void {
    // This method is no longer needed - CaptureModalManager handles all event listeners
  }

  private setupResizer(): void {
    // Panel resizing functionality
    const resizer = document.getElementById("resizer");
    if (!resizer) return;

    resizer.addEventListener("mousedown", (e: MouseEvent) => {
      this.isResizing = true;
      const leftPanel = document.getElementById("left_panel");
      if (!leftPanel) return;
      this.startX = e.clientX;
      this.startWidth = parseInt(
        document.defaultView?.getComputedStyle(leftPanel).width || "0",
        10,
      );
      document.addEventListener("mousemove", this.handleMouseMove);
      document.addEventListener("mouseup", this.handleMouseUp);
      e.preventDefault();
    });
  }

  private handleMouseMove = (e: MouseEvent) => {
    if (!this.isResizing) return;

    const leftPanel = document.getElementById("left_panel");
    if (!leftPanel) return;

    const dx = e.clientX - this.startX;
    const newWidth = this.startWidth + dx;
    const minWidth = 300;
    const maxWidth = window.innerWidth * 0.6;

    if (newWidth >= minWidth && newWidth <= maxWidth) {
      leftPanel.style.width = newWidth + "px";
      // Update CSS custom property for bottom-left to match
      document.documentElement.style.setProperty(
        "--left-panel-width",
        newWidth + "px",
      );
    }
  };

  private handleMouseUp = () => {
    this.isResizing = false;
    document.removeEventListener("mousemove", this.handleMouseMove);
    document.removeEventListener("mouseup", this.handleMouseUp);
  };

  showProgress(show: boolean): void {
    if (this.progressElement) {
      this.progressElement.style.display = show ? "block" : "none";
    }
  }

  updateStatus(message: string): void {
    console.log("Status:", message);
  }

  showError(error: string): void {
    const errorMessageElement = document.getElementById(
      "error_message",
    ) as HTMLElement;
    if (errorMessageElement) {
      errorMessageElement.textContent = error;
    }
    if (this.errorElement) {
      this.errorElement.style.display = "block";
    }
  }

  updateScores(
    before: number | null | undefined,
    after: number | null | undefined,
  ): void {
    const scoreBeforeElement = document.getElementById(
      "score_before",
    ) as HTMLElement;
    const scoreAfterElement = document.getElementById(
      "score_after",
    ) as HTMLElement;
    const scoreImprovementElement = document.getElementById(
      "score_improvement",
    ) as HTMLElement;

    // Handle null/undefined values
    if (scoreBeforeElement) {
      scoreBeforeElement.textContent =
        before !== null && before !== undefined ? before.toFixed(3) : "-";
    }
    if (scoreAfterElement) {
      scoreAfterElement.textContent =
        after !== null && after !== undefined ? after.toFixed(3) : "-";
    }
    if (scoreImprovementElement) {
      if (
        before !== null &&
        before !== undefined &&
        after !== null &&
        after !== undefined
      ) {
        const improvement = after - before;
        scoreImprovementElement.textContent =
          (improvement >= 0 ? "+" : "") + improvement.toFixed(3);
      } else {
        scoreImprovementElement.textContent = "-";
      }
    }

    // Scores are now always visible in the bottom row
  }

  setCaptureCompleteCallback(
    callback: (frequencies: number[], magnitudes: number[]) => void,
  ): void {
    this.onCaptureComplete = callback;
  }

  clearResults(): void {
    // Reset scores to default values instead of hiding
    const scoreBeforeElement = document.getElementById(
      "score_before",
    ) as HTMLElement;
    const scoreAfterElement = document.getElementById(
      "score_after",
    ) as HTMLElement;
    const scoreImprovementElement = document.getElementById(
      "score_improvement",
    ) as HTMLElement;

    if (scoreBeforeElement) {
      scoreBeforeElement.textContent = "-";
    }
    if (scoreAfterElement) {
      scoreAfterElement.textContent = "-";
    }
    if (scoreImprovementElement) {
      scoreImprovementElement.textContent = "-";
    }

    if (this.errorElement) {
      this.errorElement.style.display = "none";
    }
  }

  setOptimizationRunning(running: boolean): void {
    if (this.optimizeBtn) {
      this.optimizeBtn.disabled = running;
      this.optimizeBtn.textContent = running
        ? "Optimizing..."
        : "Run Optimization";
    }

    // Update modal buttons based on optimization state
    if (running) {
      this.showCancelButton();
      // Start the timer
      this.optimizationStartTime = Date.now();
      if (this.elapsedTimeElement) {
        this.elapsedTimeElement.textContent = "00:00";
      }
    } else {
      // Reset timer
      this.optimizationStartTime = 0;
    }
  }

  showCancelButton(): void {
    if (this.cancelOptimizationBtn && this.doneOptimizationBtn) {
      this.cancelOptimizationBtn.style.display = "inline-block";
      this.doneOptimizationBtn.style.display = "none";
    }
  }

  showCloseButton(): void {
    if (this.cancelOptimizationBtn && this.doneOptimizationBtn) {
      this.cancelOptimizationBtn.style.display = "none";
      this.doneOptimizationBtn.style.display = "inline-block";

      // Update button text and styling for close functionality
      this.doneOptimizationBtn.textContent = "Close";
      this.doneOptimizationBtn.className = "btn btn-primary"; // Blue button
    }

    // Update progress status to show completion
    if (this.progressStatus) {
      this.progressStatus.textContent = "Optimization Complete";
    }
  }

  openOptimizationModal(): void {
    if (this.optimizationModal) {
      this.optimizationModal.style.display = "flex";
      document.body.style.overflow = "hidden";
    }
  }

  closeOptimizationModal(): void {
    if (this.optimizationModal) {
      this.optimizationModal.style.display = "none";
      document.body.style.overflow = "auto";
    }
  }

  updateProgress(
    stage: string,
    status: string,
    details: string,
    percentage: number,
  ): void {
    void percentage; // percentage is documented
    if (this.progressStatus) {
      this.progressStatus.textContent = `${stage}: ${status}`;
    } else {
      console.warn("[UI DEBUG] progressStatus element not found!");
    }

    // Update elapsed time
    this.updateElapsedTime();
  }

  private updateElapsedTime(): void {
    if (this.optimizationStartTime > 0 && this.elapsedTimeElement) {
      const elapsedMs = Date.now() - this.optimizationStartTime;
      const elapsedSeconds = Math.floor(elapsedMs / 1000);
      const minutes = Math.floor(elapsedSeconds / 60);
      const seconds = elapsedSeconds % 60;
      const timeString = `${minutes.toString().padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`;

      this.elapsedTimeElement.textContent = timeString;
    }
  }

  // toggleAccordion method removed - using grid layout

  collapseAllAccordion(): void {
    // Grid layout - accordion functionality removed
  }

  showAccordionSection(sectionId: string): void {
    // Grid layout - accordion functionality removed
  }

  setEQEnabled(enabled: boolean): void {
    this.eqEnabled = enabled;

    // Update button states
    if (this.eqOnBtn && this.eqOffBtn) {
      if (enabled) {
        this.eqOnBtn.classList.add("active");
        this.eqOffBtn.classList.remove("active");
      } else {
        this.eqOnBtn.classList.remove("active");
        this.eqOffBtn.classList.add("active");
      }
    }
  }

  resetToDefaults(): void {
    // Reset form to default values
    const form = this.form;
    if (form) {
      form.reset();

      // Set specific default values with null checks
      const setElementValue = (
        id: string,
        value: string | number | boolean,
        optional: boolean = false,
      ) => {
        const element = document.getElementById(id) as
          | HTMLInputElement
          | HTMLSelectElement;
        if (element) {
          if (element.type === "checkbox") {
            (element as HTMLInputElement).checked = Boolean(value);
          } else {
            element.value = String(value);
          }
        } else if (!optional) {
          console.warn(`Element with id '${id}' not found`);
        }
      };

      // Set input source radio button
      const inputSourceRadio = document.querySelector(
        `input[name="input_source"][value="${OPTIMIZATION_DEFAULTS.input_source}"]`,
      ) as HTMLInputElement;
      if (inputSourceRadio) {
        inputSourceRadio.checked = true;
      }

      // Core EQ parameters
      setElementValue("num_filters", OPTIMIZATION_DEFAULTS.num_filters);
      setElementValue("sample_rate", OPTIMIZATION_DEFAULTS.sample_rate);
      setElementValue("min_db", OPTIMIZATION_DEFAULTS.min_db);
      setElementValue("max_db", OPTIMIZATION_DEFAULTS.max_db);
      setElementValue("min_q", OPTIMIZATION_DEFAULTS.min_q);
      setElementValue("max_q", OPTIMIZATION_DEFAULTS.max_q);
      setElementValue("min_freq", OPTIMIZATION_DEFAULTS.min_freq);
      setElementValue("max_freq", OPTIMIZATION_DEFAULTS.max_freq);
      setElementValue("curve_name", OPTIMIZATION_DEFAULTS.curve_name);
      setElementValue("loss", OPTIMIZATION_DEFAULTS.loss);
      setElementValue("peq_model", "pk"); // Default PEQ model

      // Algorithm parameters
      setElementValue("algo", OPTIMIZATION_DEFAULTS.algo);
      setElementValue("population", OPTIMIZATION_DEFAULTS.population);
      setElementValue("maxeval", OPTIMIZATION_DEFAULTS.maxeval);
      setElementValue("strategy", OPTIMIZATION_DEFAULTS.strategy, true);
      setElementValue("de_f", OPTIMIZATION_DEFAULTS.de_f, true);
      setElementValue("de_cr", OPTIMIZATION_DEFAULTS.de_cr, true);
      setElementValue(
        "adaptive_weight_f",
        OPTIMIZATION_DEFAULTS.adaptive_weight_f,
        true,
      );
      setElementValue(
        "adaptive_weight_cr",
        OPTIMIZATION_DEFAULTS.adaptive_weight_cr,
        true,
      );

      // Spacing parameters
      setElementValue("min_spacing_oct", OPTIMIZATION_DEFAULTS.min_spacing_oct);
      setElementValue("spacing_weight", OPTIMIZATION_DEFAULTS.spacing_weight);

      // Tolerance parameters
      setElementValue("tolerance", OPTIMIZATION_DEFAULTS.tolerance);
      setElementValue("abs_tolerance", OPTIMIZATION_DEFAULTS.abs_tolerance);

      // Refinement parameters
      setElementValue("refine", OPTIMIZATION_DEFAULTS.refine);
      setElementValue("local_algo", OPTIMIZATION_DEFAULTS.local_algo, true);

      // Smoothing parameters
      setElementValue("smooth", OPTIMIZATION_DEFAULTS.smooth);
      setElementValue("smooth_n", OPTIMIZATION_DEFAULTS.smooth_n);
    }

    this.updateConditionalParameters();
  }

  updateConditionalParameters(): void {
    const algo = (document.getElementById("algo") as HTMLSelectElement)?.value;
    const inputType = (
      document.querySelector(
        'input[name="input_source"]:checked',
      ) as HTMLInputElement
    )?.value;

    // Show/hide DE-specific parameters
    const deParams = document.getElementById("de-params");
    if (deParams) {
      deParams.style.display = algo === "autoeq_de" ? "block" : "none";
    }

    // Show/hide speaker selection
    const speakerSelection = document.getElementById("speaker-selection");
    if (speakerSelection) {
      speakerSelection.style.display =
        inputType === "speaker" ? "block" : "none";
    }

    // Show/hide file selection
    const fileSelection = document.getElementById("file-selection");
    if (fileSelection) {
      fileSelection.style.display = inputType === "file" ? "block" : "none";
    }

    // Show/hide capture section
    const captureSection = document.getElementById("capture-section");
    if (captureSection) {
      captureSection.style.display = inputType === "capture" ? "block" : "none";
    }

    // Show/hide curve selection based on input type
    const curveNameParam = document
      .getElementById("curve_name")
      ?.closest(".param-item") as HTMLElement;
    if (curveNameParam) {
      // Hide curve selection for headphones (they use targets instead)
      curveNameParam.style.display =
        inputType === "headphone" ? "none" : "block";
    }

    // Update loss function options based on input type
    const lossSelect = document.getElementById("loss") as HTMLSelectElement;
    if (lossSelect) {
      this.updateLossOptions(inputType, lossSelect);
    }
  }

  private switchTab(tabName: string): void {
    // Remove active class from all tab labels
    const tabLabels = document.querySelectorAll(".tab-label");
    tabLabels.forEach((label) => label.classList.remove("active"));

    // Add active class to current tab label
    const activeTabLabel = document.querySelector(
      `.tab-label[data-tab="${tabName}"]`,
    );
    if (activeTabLabel) {
      activeTabLabel.classList.add("active");
    }

    // Hide all tab content
    const tabContents = document.querySelectorAll(".tab-content");
    tabContents.forEach((content) => content.classList.remove("active"));

    // Show current tab content
    const activeTabContent = document.getElementById(`${tabName}_inputs`);
    if (activeTabContent) {
      activeTabContent.classList.add("active");
    } else {
      console.warn(`Tab content for '${tabName}' not found`);
    }

    // Set appropriate loss function based on tab
    const lossSelect = document.getElementById("loss") as HTMLSelectElement;
    if (lossSelect) {
      if (tabName === "speaker") {
        // Only set to speaker-flat if current value is not a speaker option
        if (!lossSelect.value.startsWith("speaker-")) {
          lossSelect.value = "speaker-flat";
        }
      } else if (tabName === "headphone") {
        // Only set to headphone-flat if current value is not a headphone option
        if (!lossSelect.value.startsWith("headphone-")) {
          lossSelect.value = "headphone-flat";
        }
      }
      // For 'file' and 'capture' tabs, keep whatever value is currently selected
    }
  }

  private updateLossOptions(
    inputType: string,
    lossSelect: HTMLSelectElement,
  ): void {
    const currentValue = lossSelect.value;

    // Clear existing options
    lossSelect.innerHTML = "";

    // Determine which options to use
    let options;
    let defaultValue;

    if (inputType === "headphone") {
      // Headphone: only headphone options
      options = HEADPHONE_LOSS_OPTIONS;
      defaultValue = "headphone-flat";
    } else if (inputType === "speaker") {
      // Speaker: only speaker options
      options = SPEAKER_LOSS_OPTIONS;
      defaultValue = "speaker-flat";
    } else {
      // File, Capture, or any other: show all 4 options
      options = LOSS_OPTIONS;
      defaultValue = "speaker-flat";
    }

    // Populate with appropriate options
    Object.entries(options).forEach(([value, label]) => {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = label;
      lossSelect.appendChild(option);
    });

    // Try to keep the current value if it's still valid, otherwise set default
    if (lossSelect.querySelector(`option[value="${currentValue}"]`)) {
      lossSelect.value = currentValue;
    } else {
      lossSelect.value = defaultValue;
    }
  }

  // Event handlers (to be connected to main application logic)
  private onOptimizeClick(): void {
    // This will be connected to the main optimization logic
  }

  private async onCaptureClick(): Promise<void> {
    if (!this.captureBtn) return;

    const isCapturing = this.captureBtn.textContent?.includes("Stop");

    if (isCapturing) {
      // Stop capture
      this.stopCapture();
    } else {
      // Start capture
      await this.startCapture();
    }
  }

  private async startCapture(): Promise<void> {
    // Audio capture is now handled by CaptureModalManager
    // This method is kept for backward compatibility but does nothing
  }

  private stopCapture(): void {
    // Reset UI immediately
    if (this.captureBtn) {
      this.captureBtn.textContent = "🎤 Start Capture";
      this.captureBtn.classList.remove("capturing");
    }

    if (this.captureStatusText) {
      this.captureStatusText.textContent = "Capture stopped";
    }
  }

  private plotCapturedData(frequencies: number[], magnitudes: number[]): void {
    if (!this.capturePlot) {
      console.warn("Capture plot element not found");
      return;
    }

    // Clear existing content
    this.capturePlot.innerHTML = "";

    // Create a canvas for the plot
    const canvas = document.createElement("canvas");
    canvas.width = this.capturePlot.offsetWidth || 600;
    canvas.height = 250;
    canvas.style.width = "100%";
    canvas.style.height = "250px";
    canvas.style.backgroundColor = "#f8f9fa";
    canvas.style.border = "1px solid #dee2e6";
    canvas.style.borderRadius = "4px";

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    // Add canvas to plot container
    this.capturePlot.appendChild(canvas);

    // Calculate plot dimensions
    const padding = 50;
    const plotWidth = canvas.width - 2 * padding;
    const plotHeight = canvas.height - 2 * padding;

    // Find min/max values for scaling
    const minMag = Math.min(...magnitudes);
    const maxMag = Math.max(...magnitudes);
    const magRange = maxMag - minMag || 1;

    // Draw background
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(padding, padding, plotWidth, plotHeight);

    // Draw grid lines
    ctx.strokeStyle = "#e0e0e0";
    ctx.lineWidth = 0.5;

    // Horizontal grid lines
    for (let i = 0; i <= 5; i++) {
      const y = padding + (i * plotHeight) / 5;
      ctx.beginPath();
      ctx.moveTo(padding, y);
      ctx.lineTo(padding + plotWidth, y);
      ctx.stroke();
    }

    // Vertical grid lines (logarithmic)
    const freqPoints = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
    freqPoints.forEach((freq) => {
      const x =
        padding + (Math.log10(freq / 20) / Math.log10(1000)) * plotWidth;
      ctx.beginPath();
      ctx.moveTo(x, padding);
      ctx.lineTo(x, padding + plotHeight);
      ctx.stroke();
    });

    // Draw axes
    ctx.strokeStyle = "#333";
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(padding, padding);
    ctx.lineTo(padding, padding + plotHeight);
    ctx.lineTo(padding + plotWidth, padding + plotHeight);
    ctx.stroke();

    // Draw frequency response curve
    ctx.strokeStyle = "#007bff";
    ctx.lineWidth = 2;
    ctx.beginPath();

    for (let i = 0; i < frequencies.length; i++) {
      const x =
        padding +
        (Math.log10(frequencies[i] / 20) / Math.log10(1000)) * plotWidth;
      const y =
        padding +
        plotHeight -
        ((magnitudes[i] - minMag) / magRange) * plotHeight;

      if (i === 0) {
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    }

    ctx.stroke();

    // Draw labels
    ctx.fillStyle = "#333";
    ctx.font = "12px sans-serif";
    ctx.textAlign = "center";

    // X-axis labels
    freqPoints.forEach((freq) => {
      const x =
        padding + (Math.log10(freq / 20) / Math.log10(1000)) * plotWidth;
      ctx.fillText(
        freq >= 1000 ? `${freq / 1000}k` : `${freq}`,
        x,
        canvas.height - 25,
      );
    });

    // Y-axis labels
    ctx.textAlign = "right";
    for (let i = 0; i <= 5; i++) {
      const mag = minMag + (1 - i / 5) * magRange;
      const y = padding + (i * plotHeight) / 5;
      ctx.fillText(`${mag.toFixed(1)} dB`, padding - 5, y + 4);
    }

    // Title
    ctx.textAlign = "center";
    ctx.font = "bold 14px sans-serif";
    ctx.fillText("Captured Frequency Response", canvas.width / 2, 20);

    // Axis labels
    ctx.font = "12px sans-serif";
    ctx.fillText("Frequency (Hz)", canvas.width / 2, canvas.height - 5);

    // Rotate for Y-axis label
    ctx.save();
    ctx.translate(15, canvas.height / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.fillText("Magnitude (dB)", 0, 0);
    ctx.restore();
  }

  private async initializeAudioDevices(): Promise<void> {
    // Audio device enumeration is now handled by CaptureModalManager
  }

  private clearCaptureResults(): void {
    // Clear the UI
    if (this.captureResult) {
      this.captureResult.style.display = "none";
    }
    if (this.capturePlot) {
      this.capturePlot.innerHTML = "";
    }
    if (this.captureStatusText) {
      this.captureStatusText.textContent = "Ready to capture";
    }

    // Clear stored data by notifying with empty arrays
    if (this.onCaptureComplete) {
      this.onCaptureComplete([], []);
    }
  }

  private onListenClick(): void {
    // This will be connected to the audio logic
    console.log("TODO: Listen button clicked");
  }

  private onStopClick(): void {
    // This will be connected to the audio logic
    console.log("TODO: Stop button clicked");
  }

  private cancelOptimization(): void {
    // This will be connected to the optimization logic
    console.log("TODO: Cancel optimization");
  }

  // Capture Modal Management Methods
  private initializeCaptureModal(): void {
    // Create CaptureModalManager instance
    this.captureModalManager = new CaptureModalManager();

    // Set up callbacks
    this.captureModalManager.setCaptureCompleteCallback(
      (frequencies: number[], magnitudes: number[]) => {
        if (this.onCaptureComplete) {
          this.onCaptureComplete(frequencies, magnitudes);
        }
      },
    );

    // Listen for the capture panel being rendered and initialize it
    // Use a flag to ensure we only initialize once
    let isInitialized = false;

    document.addEventListener("capturePanelRendered", async (event) => {
      if (isInitialized) {
        console.log(
          "[UIManager] Capture panel already initialized, skipping...",
        );
        return;
      }

      console.log("[UIManager] Capture panel rendered, initializing...");
      if (this.captureModalManager) {
        await this.captureModalManager.initialize();
        isInitialized = true;
        console.log("[UIManager] Capture panel initialization complete");
      }
    });
  }

  private async openCaptureModal(): Promise<void> {
    // This is now deprecated - the panel is rendered inline
    // Just switch to the capture tab instead
    console.log("[UIManager] openCaptureModal called - panel is now inline");
  }

  private closeCaptureModal(): void {
    // This is now deprecated - the panel stays inline
    console.log("[UIManager] closeCaptureModal called - panel is now inline");
  }

  setOutputDeviceChangeCallback(callback: (deviceId: string) => void): void {
    this.outputDeviceChangeCallback = callback;
  }

  // Getters for accessing UI elements from main application
  getForm(): HTMLFormElement {
    return this.form;
  }
  getOptimizeBtn(): HTMLButtonElement {
    return this.optimizeBtn;
  }
  getResetBtn(): HTMLButtonElement {
    return this.resetBtn;
  }
  getListenBtn(): HTMLButtonElement {
    return this.listenBtn;
  }
  getStopBtn(): HTMLButtonElement {
    return this.stopBtn;
  }
  getEqOnBtn(): HTMLButtonElement {
    return this.eqOnBtn;
  }
  getEqOffBtn(): HTMLButtonElement {
    return this.eqOffBtn;
  }
  getCancelOptimizationBtn(): HTMLButtonElement {
    return this.cancelOptimizationBtn;
  }

  updateOptimizeBtn(btn: HTMLButtonElement): void {
    this.optimizeBtn = btn;
  }
  updateResetBtn(btn: HTMLButtonElement): void {
    this.resetBtn = btn;
  }
  updateListenBtn(btn: HTMLButtonElement): void {
    this.listenBtn = btn;
  }
  updateStopBtn(btn: HTMLButtonElement): void {
    this.stopBtn = btn;
  }
  updateEqOnBtn(btn: HTMLButtonElement): void {
    this.eqOnBtn = btn;
  }
  updateEqOffBtn(btn: HTMLButtonElement): void {
    this.eqOffBtn = btn;
  }
  updateCancelOptimizationBtn(btn: HTMLButtonElement): void {
    this.cancelOptimizationBtn = btn;
  }
  getAudioStatus(): HTMLElement {
    return this.audioStatus;
  }
  getAudioStatusText(): HTMLElement {
    return this.audioStatusText;
  }
  getAudioDuration(): HTMLElement {
    return this.audioDuration;
  }
  getAudioPosition(): HTMLElement {
    return this.audioPosition;
  }
  getAudioProgressFill(): HTMLElement {
    return this.audioProgressFill;
  }

  // Capture elements
  getCaptureBtn(): HTMLButtonElement | null {
    return this.captureBtn;
  }
  getCaptureStatus(): HTMLElement | null {
    return this.captureStatus;
  }
  getCaptureStatusText(): HTMLElement | null {
    return this.captureStatusText;
  }
  getCaptureProgressFill(): HTMLElement | null {
    return this.captureProgressFill;
  }
  getCaptureWaveform(): HTMLCanvasElement | null {
    return this.captureWaveform;
  }
  getCaptureWaveformCtx(): CanvasRenderingContext2D | null {
    return this.captureWaveformCtx;
  }
  getCaptureResult(): HTMLElement | null {
    return this.captureResult;
  }
  getCaptureClearBtn(): HTMLButtonElement | null {
    return this.captureClearBtn;
  }
  getCapturePlot(): HTMLElement | null {
    return this.capturePlot;
  }

  // State getters
  isEQEnabled(): boolean {
    return this.eqEnabled;
  }

  // Audio control methods
  setAudioStatus(status: string): void {
    if (this.audioStatusText) {
      this.audioStatusText.textContent = status;
    } else {
      console.warn("Audio status text element not found!");
    }
  }

  setListenButtonEnabled(enabled: boolean): void {
    if (this.listenBtn) {
      this.listenBtn.disabled = !enabled;
      if (enabled) {
        this.listenBtn.classList.remove("disabled");
      } else {
        this.listenBtn.classList.add("disabled");
      }
    } else {
      console.warn("Listen button not found in UIManager!");
    }
  }

  // Download APO button control
  enableDownloadButton(): void {
    if (this.downloadApoBtn) {
      this.downloadApoBtn.disabled = false;
    }
  }

  disableDownloadButton(): void {
    if (this.downloadApoBtn) {
      this.downloadApoBtn.disabled = true;
    }
  }

  private async onDownloadApoClick(): Promise<void> {
    try {
      if (!this.getOptimizationResult) {
        console.error("[DOWNLOAD] Optimization result getter not set");
        this.showError("Unable to download: Optimization result not available");
        return;
      }

      const result = this.getOptimizationResult();
      if (!result.filterParams || !result.sampleRate || !result.peqModel) {
        console.error("[DOWNLOAD] Missing optimization result data", result);
        this.showError("No optimization result available to download");
        return;
      }

      // Get selected export format
      const format = (this.exportFormatSelect?.value || "apo") as ExportFormat;

      await exportEQ(
        result.filterParams,
        result.sampleRate,
        result.peqModel,
        result.lossType,
        result.speakerName,
        format,
      );
    } catch (error) {
      console.error("[DOWNLOAD] Error exporting APO:", error);
      this.showError(
        `Failed to export APO: ${error instanceof Error ? error.message : "Unknown error"}`,
      );
    }
  }

  setOptimizationResultGetter(
    getter: () => {
      filterParams: number[] | null;
      sampleRate: number | null;
      peqModel: string | null;
      lossType: string | null;
      speakerName: string | null;
    },
  ): void {
    this.getOptimizationResult = getter;
  }
}
