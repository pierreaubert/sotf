// Refactored main application - step-based workflow

// Import web components first so they're registered
import "./modules/audio-capture/capture-config-panel";
import "./modules/audio-capture/capture-recording-panel";

import {
  UIManager,
  PlotComposer,
  OptimizationManager,
  APIManager,
  AudioPlayer,
  AudioPlayerFilterParam as FilterParam,
  PlotManager,
  StepNavigator,
  StepContainer,
  UseCaseSelector,
  DataAcquisitionStep,
  generateDataAcquisition,
  generateEQDesign,
  generateOptimizationFineTuning,
  generatePlotsPanel,
  generateBottomRow,
  generateOptimizationModal,
} from "./modules";
import { OptimizationResult } from "./types";
import { AutoEQPlotAPI, PlotFiltersParams, PlotSpinParams } from "./types";

class AutoEQApplication {
  private uiManager: UIManager;
  private plotComposer: PlotComposer;
  private optimizationManager: OptimizationManager;
  private apiManager: APIManager;
  private audioPlayer: AudioPlayer | undefined;
  private plotManager: PlotManager;
  private stepNavigator!: StepNavigator;
  private stepContainer!: StepContainer;
  private useCaseSelector!: UseCaseSelector;
  private dataAcquisitionStep!: DataAcquisitionStep;
  private optimizedFilters: FilterParam[] | null = null;

  constructor() {
    // Generate and inject HTML content FIRST, before initializing managers
    this.injectStepWorkflowHTML();

    // Initialize step navigation
    this.initializeStepNavigation();

    // Initialize managers
    this.uiManager = new UIManager();
    this.apiManager = new APIManager();

    // Initialize AudioPlayer early so CamillaDSP is running and ready for EQ updates
    // This is needed because optimization in Step 3 needs to upload EQ parameters
    // Run async initialization in background (don't await to avoid blocking constructor)
    this.initializeAudioPlayer().catch((error) => {
      console.error("[Main] Failed to initialize AudioPlayer:", error);
    });

    // Initialize plot manager with DOM elements
    const progressGraphElement = document.getElementById(
      "progress_graph",
    ) as HTMLElement;
    const tonalPlotElement = document.getElementById(
      "tonal_plot",
    ) as HTMLElement;
    const filterPlotElement = document.getElementById(
      "filter_plot",
    ) as HTMLElement;
    const spinPlotElement = document.getElementById("spin_plot") as HTMLElement;
    const detailsPlotElement = document.getElementById(
      "details_plot",
    ) as HTMLElement;

    this.plotComposer = new PlotComposer(
      filterPlotElement,
      detailsPlotElement,
      spinPlotElement,
      progressGraphElement,
      tonalPlotElement,
    );

    this.optimizationManager = new OptimizationManager();
    this.plotManager = new PlotManager();
    this.setupManagerConnections();
    this.setupEventHandlers();
    this.initialize();
  }

  private injectStepWorkflowHTML(): void {
    const appElement = document.getElementById("app");
    if (!appElement) {
      throw new Error("Application container element not found");
    }

    // Generate step-based HTML structure
    appElement.innerHTML = `
      <div class="app">
        <div id="nav-container"></div>
        <div id="content-container" class="main-content">
          <!-- Step 1: Use Case Selector -->
          <div data-step="1" id="step1-container"></div>

          <!-- Step 2: Data Acquisition -->
          <div data-step="2" id="step2-container">
            <!-- DataAcquisitionStep component will be rendered here -->
          </div>

          <!-- Step 3: EQ Design & Optimization -->
          <div data-step="3" id="step3-container">
            <div class="step-content-wrapper">
              <div class="step-header-section">
                <h2 class="step-title">EQ Design & Optimization</h2>
                <p class="step-description">
                  Configure your EQ parameters and optimization algorithm settings.
                </p>
              </div>
              <div class="step3-two-column-layout">
                <form id="autoeq_form" class="parameter-form">
                  <div class="step3-column">
                    ${generateEQDesign()}
                  </div>
                  <div class="step3-column">
                    ${generateOptimizationFineTuning()}
                  </div>
                </form>
              </div>
              <div class="step-actions step3-actions">
                <button type="submit" id="optimize_btn" class="btn btn-primary btn-large">
                  Run Optimization
                </button>
                <button type="button" id="reset_btn" class="btn btn-outline">
                  Reset to Defaults
                </button>
              </div>

              <!-- Results Section - Shown after optimization -->
              <div id="step3-results" class="step3-results" style="display: none;">
                <div class="step3-results-header">
                  <h3>Optimization Results</h3>
                  <div class="step3-score-row">
                    <div class="step3-score-item">
                      <label>Before:</label>
                      <span id="step3_score_before">-</span>
                    </div>
                    <div class="step3-score-item">
                      <label>After:</label>
                      <span id="step3_score_after">-</span>
                    </div>
                    <div class="step3-score-item improvement">
                      <label>Improvement:</label>
                      <span id="step3_score_improvement">-</span>
                    </div>
                  </div>
                </div>

                ${generatePlotsPanel()}

                <div class="step3-results-actions">
                  <button type="button" id="step3_continue_btn" class="btn btn-primary btn-large">
                    Continue to Listening & Testing
                  </button>
                  <button type="button" id="step3_reoptimize_btn" class="btn btn-outline">
                    Tweak & Re-optimize
                  </button>
                </div>
              </div>
            </div>
          </div>

          <!-- Step 4: Listening & Testing -->
          <div data-step="4" id="step4-container">
            <h2 class="step-title">Listening & Testing</h2>
            <p class="step-description">
              Test your optimized EQ with the audio player. Toggle EQ on/off to hear the difference.
            </p>
            <!-- Audio Player for Step 4 -->
            <div id="step4-audio-controls"></div>
            <div class="step-actions" style="margin-top: 24px;">
              <button type="button" id="continue_to_save_btn" class="btn btn-primary btn-large">
                Continue to Save & Export
              </button>
            </div>
          </div>

          <!-- Step 5: Save & Export -->
          <div data-step="5" id="step5-container">
            <div class="step-content-wrapper">
              <div class="step-header-section">
                <h2 class="step-title">Save & Export</h2>
                <p class="step-description">
                  Export your optimized EQ settings to use with your favorite audio software or hardware.
                </p>
              </div>
              <div class="export-section">
                <div class="export-format-section">
                  <label for="export_format_select">Export Format:</label>
                  <select id="export_format_select" class="export-format-select">
                    <option value="apo">APO - Equalizer APO (Windows)</option>
                    <option value="aupreset">AUpreset - macOS Audio Units</option>
                    <option value="rme">RME Channel - RME TotalMix FX</option>
                    <option value="rme-room">RME Room - RME Room Correction</option>
                  </select>
                  <button type="button" id="download_apo_btn" class="btn btn-primary btn-large" disabled>
                    💾 Download EQ File
                  </button>
                </div>
              </div>
              <div class="step-actions">
                <button type="button" id="start_new_btn" class="btn btn-secondary btn-large">
                  Start New Optimization
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Modals -->
      ${generateOptimizationModal()}
    `;
  }

  private initializeStepNavigation(): void {
    // Define steps
    const steps = [
      {
        id: 1,
        label: "Choose Use Case",
        shortLabel: "Use Case",
        enabled: true,
      },
      { id: 2, label: "Data Acquisition", shortLabel: "Data", enabled: false },
      {
        id: 3,
        label: "EQ Design & Optimization",
        shortLabel: "Optimize",
        enabled: false,
      },
      {
        id: 4,
        label: "Results & Listening",
        shortLabel: "Results",
        enabled: false,
      },
      { id: 5, label: "Save & Export", shortLabel: "Export", enabled: false },
    ];

    // Initialize Navigator
    const navContainer = document.getElementById("nav-container")!;
    this.stepNavigator = new StepNavigator(navContainer, {
      steps,
      currentStep: 1,
      onStepChange: (stepId) => {
        console.log("✨ Step changed to:", stepId);
        this.stepContainer.goToStep(stepId);
      },
      onPrevious: () => {
        const currentStep = this.stepNavigator.getCurrentStep();
        if (currentStep > 1) {
          this.stepNavigator.goToStep(currentStep - 1);
        }
      },
      onNext: () => {
        const currentStep = this.stepNavigator.getCurrentStep();
        if (currentStep < 5 && steps[currentStep].enabled) {
          this.stepNavigator.goToStep(currentStep + 1);
        }
      },
    });

    // Initialize Container
    const contentContainer = document.getElementById("content-container")!;
    this.stepContainer = new StepContainer(contentContainer, {
      currentStep: 1,
      animationDuration: 300,
      onBeforeStepChange: (fromStep, toStep) => {
        console.log(`🔄 Transitioning from step ${fromStep} to ${toStep}`);
        return true;
      },
      onAfterStepChange: (stepId) => {
        console.log(`✅ Now on step ${stepId}`);
        this.stepNavigator.goToStep(stepId);

        // Apply stored optimized filters when entering Step 4
        if (stepId === 4 && this.optimizedFilters && this.audioPlayer) {
          console.log(
            "[MAIN] 📊 Applying stored optimized filters to audio player",
          );
          try {
            this.audioPlayer.updateFilterParams(this.optimizedFilters);
            this.audioPlayer.setEQEnabled(true);
            console.log("[MAIN] ✅ Filters applied successfully");
          } catch (error) {
            console.error("[MAIN] ❌ Failed to apply filters:", error);
          }
        }
      },
    });

    // Initialize Use Case Selector (Step 1)
    const step1Container = document.getElementById("step1-container")!;
    this.useCaseSelector = new UseCaseSelector(step1Container, {
      onSelect: (useCase) => {
        console.log("📱 Selected use case:", useCase);

        // Special handling for "Play Music" - skip directly to step 4
        if (useCase === "play-music") {
          // Enable step 4 only
          this.stepNavigator.setStepEnabled(4, true);

          // Jump directly to audio player
          setTimeout(() => {
            this.stepNavigator.goToStep(4);
          }, 500);
          return;
        }

        // Regular flow for other use cases
        // Enable steps 2 and 3 after selection
        this.stepNavigator.setStepEnabled(2, true);
        this.stepNavigator.setStepEnabled(3, true);

        // Configure Step 2 based on use case
        this.configureDataAcquisitionStep(useCase);

        // Auto-advance to step 2
        setTimeout(() => {
          this.stepNavigator.goToStep(2);
        }, 500);
      },
    });

    // Initialize Data Acquisition Step (Step 2)
    const step2Container = document.getElementById("step2-container")!;
    this.dataAcquisitionStep = new DataAcquisitionStep(step2Container, {
      onDataReady: (source) => {
        console.log("📊 Data ready from source:", source);
        // Enable next button when data is ready
        const step2NextBtn = document.getElementById("step2_next_btn");
        if (step2NextBtn) {
          (step2NextBtn as HTMLButtonElement).disabled = false;
        }
      },
      onSourceChange: (source) => {
        console.log("📊 Data source changed to:", source);
      },
    });

    // Setup Step 2 Navigation buttons
    const step2NextBtn = document.getElementById("step2_next_btn");
    if (step2NextBtn) {
      step2NextBtn.addEventListener("click", () => {
        this.stepNavigator.goToStep(3);
      });
    }

    const step2PrevBtn = document.getElementById("step2_prev_btn");
    if (step2PrevBtn) {
      step2PrevBtn.addEventListener("click", () => {
        this.stepNavigator.goToStep(1);
      });
    }

    // Setup Step 3 button handlers
    const optimizeBtn = document.getElementById("optimize_btn");
    if (optimizeBtn) {
      optimizeBtn.addEventListener("click", (e) => {
        e.preventDefault();
        this.runOptimization();
      });
    }

    const resetBtn = document.getElementById("reset_btn");
    if (resetBtn) {
      resetBtn.addEventListener("click", () => {
        this.uiManager.resetToDefaults();
      });
    }

    // Step 3 results buttons
    const step3ContinueBtn = document.getElementById("step3_continue_btn");
    if (step3ContinueBtn) {
      step3ContinueBtn.addEventListener("click", () => {
        this.stepNavigator.setStepEnabled(4, true);
        this.stepNavigator.goToStep(4);
      });
    }

    const step3ReoptimizeBtn = document.getElementById("step3_reoptimize_btn");
    if (step3ReoptimizeBtn) {
      step3ReoptimizeBtn.addEventListener("click", () => {
        // Scroll back to top of step 3 to see the parameters
        const step3Container = document.getElementById("step3-container");
        if (step3Container) {
          step3Container.scrollTo({ top: 0, behavior: "smooth" });
        }
      });
    }

    // Setup Step 4 button handlers
    const continueToSaveBtn = document.getElementById("continue_to_save_btn");
    if (continueToSaveBtn) {
      continueToSaveBtn.addEventListener("click", () => {
        this.stepNavigator.setStepEnabled(5, true);
        this.stepNavigator.goToStep(5);
      });
    }

    // Setup Step 5 button handlers
    const startNewBtn = document.getElementById("start_new_btn");
    if (startNewBtn) {
      startNewBtn.addEventListener("click", () => {
        if (
          confirm(
            "Start a new optimization?\n\nThis will reset the current workflow.",
          )
        ) {
          // Reset all steps except Step 1
          for (let i = 2; i <= 5; i++) {
            this.stepNavigator.setStepEnabled(i, false);
          }

          // Clear selection and navigate to Step 1
          this.useCaseSelector.clearSelection();
          this.stepNavigator.goToStep(1);

          // Reset UI state
          this.uiManager.clearResults();
          this.plotComposer.clearAllPlots();
        }
      });
    }
  }

  /**
   * Initialize AudioPlayer early so CamillaDSP is running and ready for EQ updates
   */
  private async initializeAudioPlayer(): Promise<void> {
    const audioControlsContainer = document.getElementById(
      "step4-audio-controls",
    ) as HTMLElement;
    if (audioControlsContainer) {
      // Clear the old controls and let the AudioPlayer component build its own UI
      audioControlsContainer.innerHTML = "";

      this.audioPlayer = new AudioPlayer(
        audioControlsContainer,
        {
          enableEQ: true,
          enableSpectrum: true,
          showProgress: true,
          showFrequencyLabels: true,
          maxFilters: 11,
        },
        {
          onEQToggle: (enabled) => this.setEQEnabled(enabled),
          onError: (error) => this.uiManager.showError(error),
        },
      );

      console.log("[Main] AudioPlayer initialized");

      // Load a default demo track to start CamillaDSP
      // This ensures CamillaDSP is running and ready to receive EQ parameters
      try {
        await this.audioPlayer.loadDemoTrack("classical");
        console.log(
          "[Main] Default demo track loaded - CamillaDSP is now running and ready for EQ updates",
        );
      } catch (error) {
        console.warn(
          "[Main] Could not load default demo track (this is OK if no demo files exist):",
          error,
        );
        // This is not critical - CamillaDSP will start when user loads audio in Step 4
      }
    } else {
      console.error(
        "Audio controls container (#step4-audio-controls) not found!",
      );
    }
  }

  /**
   * Configure Data Acquisition step to show only relevant fields for the selected use case
   */
  private configureDataAcquisitionStep(
    useCase: "speaker" | "headphone" | "capture" | "file" | "play-music",
  ): void {
    // Hide the tab interface since we're showing the section directly
    const tabsContainer = document.querySelector(
      ".input-source-tabs",
    ) as HTMLElement;
    if (tabsContainer) {
      tabsContainer.style.display = "none";
    }

    // Also hide the "Select Data Source" heading
    const sectionGroup = document.querySelector(
      ".data-acquisition-step .section-group h3",
    ) as HTMLElement;
    if (sectionGroup) {
      sectionGroup.style.display = "none";
    }

    // Get all tab content sections
    const fileInputs = document.getElementById("file_inputs");
    const speakerInputs = document.getElementById("speaker_inputs");
    const headphoneInputs = document.getElementById("headphone_inputs");
    const captureInputs = document.getElementById("capture_inputs");

    // Get all info cards
    const fileInfo = document.getElementById("file_info");
    const speakerInfo = document.getElementById("speaker_info");
    const headphoneInfo = document.getElementById("headphone_info");
    const captureInfo = document.getElementById("capture_info");

    // Hide all sections first
    [fileInputs, speakerInputs, headphoneInputs, captureInputs].forEach(
      (el) => {
        if (el) {
          el.style.display = "none";
          el.classList.remove("active");
        }
      },
    );

    // Hide all info cards first
    [fileInfo, speakerInfo, headphoneInfo, captureInfo].forEach((el) => {
      if (el) {
        (el as HTMLElement).style.display = "none";
      }
    });

    // Update description and show relevant section
    const description = document.querySelector(
      ".data-acquisition-step .step-description",
    ) as HTMLElement;
    const headerSection = document.querySelector(
      ".data-acquisition-step .step-header-section",
    ) as HTMLElement;

    switch (useCase) {
      case "speaker":
        if (headerSection) {
          headerSection.style.display = "block";
        }
        if (speakerInputs) {
          speakerInputs.style.display = "block";
          speakerInputs.classList.add("active");
        }
        if (speakerInfo) {
          (speakerInfo as HTMLElement).style.display = "block";
        }
        if (description) {
          description.textContent =
            "Search for your speaker in our database or select from recent measurements.";
        }
        // Trigger the radio button selection for proper UI state
        const speakerRadio = document.querySelector<HTMLInputElement>(
          'input[name="input_source"][value="speaker"]',
        );
        if (speakerRadio) {
          speakerRadio.checked = true;
          speakerRadio.dispatchEvent(new Event("change", { bubbles: true }));
        }
        // Enable button when speaker and version are selected
        this.setupSpeakerValidation();
        break;

      case "headphone":
        if (headerSection) {
          headerSection.style.display = "block";
        }
        if (headphoneInputs) {
          headphoneInputs.style.display = "block";
          headphoneInputs.classList.add("active");
        }
        if (headphoneInfo) {
          (headphoneInfo as HTMLElement).style.display = "block";
        }
        if (description) {
          description.textContent =
            "Load your headphone measurement file and select the target curve.";
        }
        const headphoneRadio = document.querySelector<HTMLInputElement>(
          'input[name="input_source"][value="headphone"]',
        );
        if (headphoneRadio) {
          headphoneRadio.checked = true;
          headphoneRadio.dispatchEvent(new Event("change", { bubbles: true }));
        }
        // Enable button when headphone curve and target are selected
        this.setupHeadphoneValidation();
        break;

      case "capture":
        // Hide header section for capture to save space
        if (headerSection) {
          headerSection.style.display = "none";
        }
        if (captureInputs) {
          captureInputs.style.display = "block";
          captureInputs.classList.add("active");

          // Render the capture panel web component if not already rendered
          const container = captureInputs.querySelector(
            "#capture_panel_container",
          );
          if (container && !container.querySelector("capture-panel")) {
            const capturePanel = document.createElement("capture-panel");
            container.appendChild(capturePanel);
            console.log("[Main] Capture panel web component rendered");
          }
        }
        if (captureInfo) {
          (captureInfo as HTMLElement).style.display = "block";
        }
        const captureRadio = document.querySelector<HTMLInputElement>(
          'input[name="input_source"][value="capture"]',
        );
        if (captureRadio) {
          captureRadio.checked = true;
          captureRadio.dispatchEvent(new Event("change", { bubbles: true }));
        }
        break;

      case "file":
        if (headerSection) {
          headerSection.style.display = "block";
        }
        if (fileInputs) {
          fileInputs.style.display = "block";
          fileInputs.classList.add("active");
        }
        if (fileInfo) {
          (fileInfo as HTMLElement).style.display = "block";
        }
        if (description) {
          description.textContent =
            "Load CSV files containing your input curve and optionally a target curve.";
        }
        const fileRadio = document.querySelector<HTMLInputElement>(
          'input[name="input_source"][value="file"]',
        );
        if (fileRadio) {
          fileRadio.checked = true;
          fileRadio.dispatchEvent(new Event("change", { bubbles: true }));
        }
        // Enable button when curve file is selected
        this.setupFileValidation();
        break;
    }
  }

  private setupManagerConnections(): void {
    // Connect optimization manager callbacks to UI updates
    this.optimizationManager.setCallbacks({
      onProgressUpdate: (stage, status, details, percentage) => {
        this.uiManager.updateProgress(stage, status, details, percentage);
      },
      onOptimizationComplete: (result) => {
        this.handleOptimizationSuccess(result);
      },
      onOptimizationError: (error) => {
        this.handleOptimizationError(error);
      },
      onProgressDataUpdate: (iteration, fitness, convergence) => {
        // Update progress graph with new data
        this.plotComposer.addProgressData(iteration, fitness, convergence);

        // Update elapsed time display
        this.uiManager.updateProgress(
          "Optimization",
          `Iteration ${iteration}`,
          `Fitness: ${fitness.toFixed(4)}`,
          0,
        );

        // Update graph every 5 iterations or for early iterations
        if (iteration <= 5 || iteration % 5 === 0) {
          this.plotComposer.updateProgressGraph().catch((error) => {
            console.error(
              "[MAIN DEBUG] ❌ Error updating progress graph:",
              error,
            );
          });
        }
      },
    });

    // Connect UI manager capture callback to optimization manager
    this.uiManager.setCaptureCompleteCallback((frequencies, magnitudes) => {
      if (frequencies.length > 0 && magnitudes.length > 0) {
        this.optimizationManager.setCapturedData(frequencies, magnitudes);
      } else {
        this.optimizationManager.clearCapturedData();
      }
    });

    // Connect UI manager output device change callback to audio player
    this.uiManager.setOutputDeviceChangeCallback((deviceId) => {
      if (this.audioPlayer) {
        this.audioPlayer.setOutputDevice(deviceId);
      }
    });

    // Connect optimization result getter for APO download
    this.uiManager.setOptimizationResultGetter(() => ({
      filterParams: this.optimizationManager.getFilterParams(),
      sampleRate: this.optimizationManager.getSampleRate(),
      peqModel: this.optimizationManager.getPeqModel(),
      lossType: this.optimizationManager.getLossType(),
      speakerName: this.optimizationManager.getSpeakerName(),
    }));

    // Override UI manager event handlers to connect to application logic
    this.overrideUIEventHandlers();
  }

  private overrideUIEventHandlers(): void {
    // Cancel button handler for optimization modal
    const cancelBtn = this.uiManager.getCancelOptimizationBtn();
    if (cancelBtn) {
      cancelBtn.addEventListener("click", () => this.cancelOptimization());
    }
  }

  private setupEventHandlers(): void {
    // Setup API-related event handlers
    const speakerSelect = document.getElementById(
      "speaker",
    ) as HTMLSelectElement;
    const versionSelect = document.getElementById(
      "version",
    ) as HTMLSelectElement;
    const curveFileBtn = document.getElementById(
      "browse_curve",
    ) as HTMLButtonElement;
    const targetFileBtn = document.getElementById(
      "browse_target",
    ) as HTMLButtonElement;
    const headphoneCurveBtn = document.getElementById(
      "browse_headphone_curve",
    ) as HTMLButtonElement;

    if (speakerSelect) {
      speakerSelect.addEventListener("change", (e) => {
        const speaker = (e.target as HTMLSelectElement).value;
        this.apiManager.handleSpeakerChange(speaker);
      });
    }

    if (versionSelect) {
      versionSelect.addEventListener("change", (e) => {
        const version = (e.target as HTMLSelectElement).value;
        this.apiManager.handleVersionChange(version);
      });
    } else {
      console.warn("Version select element not found");
    }

    if (curveFileBtn) {
      curveFileBtn.addEventListener("click", () => {
        this.apiManager.selectCurveFile();
      });
    }

    if (targetFileBtn) {
      targetFileBtn.addEventListener("click", () => {
        this.apiManager.selectTargetFile();
      });
    }

    if (headphoneCurveBtn) {
      headphoneCurveBtn.addEventListener("click", () => {
        this.apiManager.selectHeadphoneCurveFile();
      });
    }
  }

  private async initialize(): Promise<void> {
    try {
      // Initialize UI state
      this.uiManager.resetToDefaults();
      this.uiManager.setEQEnabled(false);
      this.uiManager.collapseAllAccordion();

      // Clear any existing plots
      this.plotComposer.clearAllPlots();

      // Load initial data
      await this.apiManager.loadDemoAudioList();

      // Setup autocomplete
      this.apiManager.setupAutocomplete();
    } catch (error) {
      console.error("Error during application initialization:", error);
      this.uiManager.showError("Failed to initialize application: " + error);
    }
  }

  private async runOptimization(): Promise<void> {
    if (this.optimizationManager.isRunning()) {
      return;
    }

    try {
      // Clear previous results
      this.uiManager.clearResults();
      this.plotComposer.clearAllPlots();

      // Hide Step 3 results section when starting new optimization
      const step3Results = document.getElementById("step3-results");
      if (step3Results) {
        step3Results.style.display = "none";
      }

      // Get form data
      const formData = new FormData(this.uiManager.getForm());

      // Add Step 2 (Data Acquisition) fields to form data
      const addFieldIfExists = (id: string, name?: string) => {
        const element = document.getElementById(id) as
          | HTMLInputElement
          | HTMLSelectElement;
        if (element && element.value) {
          formData.set(name || id, element.value);
        }
      };

      // Add input source type (this should be set by radio buttons, but let's also check)
      const inputSourceRadio = document.querySelector(
        'input[name="input_source"]:checked',
      ) as HTMLInputElement;
      if (inputSourceRadio) {
        formData.set("input_source", inputSourceRadio.value);
      }

      // Add speaker fields
      addFieldIfExists("speaker");
      addFieldIfExists("version");
      addFieldIfExists("measurement");

      // Add file fields
      addFieldIfExists("curve_path");
      addFieldIfExists("target_path");

      // Add headphone fields
      addFieldIfExists("headphone_curve_path");
      addFieldIfExists("headphone_target");

      // Validate parameters
      const validation = this.apiManager.validateOptimizationParams(formData);
      if (!validation.isValid) {
        this.uiManager.showError(
          "Validation errors:\n" + validation.errors.join("\n"),
        );
        return;
      }

      // Extract optimization parameters
      const params =
        await this.optimizationManager.extractOptimizationParams(formData);

      // Update UI state
      this.uiManager.setOptimizationRunning(true);
      this.uiManager.disableDownloadButton();
      this.uiManager.openOptimizationModal();

      // Clear any existing progress data
      this.plotComposer.clearProgressGraph();

      // Run optimization
      await this.optimizationManager.runOptimization(params);
    } catch (error) {
      console.error("Optimization failed:", error);
      this.handleOptimizationError(
        error instanceof Error ? error.message : "Unknown error",
      );
    } finally {
      this.uiManager.setOptimizationRunning(false);
    }
  }

  private async handleOptimizationSuccess(
    result: OptimizationResult,
  ): Promise<void> {
    console.log("[MAIN] 🎯 handleOptimizationSuccess() called!");
    console.log("[MAIN] Result data:", {
      success: result.success,
      has_filter_params: !!result.filter_params,
      filter_params_length: result.filter_params?.length,
      has_scores: !!(
        result.preference_score_before !== undefined &&
        result.preference_score_after !== undefined &&
        result.preference_score_before !== null &&
        result.preference_score_after !== null
      ),
      score_before: result.preference_score_before,
      score_after: result.preference_score_after,
      has_filter_response: !!result.filter_response,
      has_spin_details: !!result.spin_details,
      has_filter_plots: !!result.filter_plots,
      error_message: result.error_message,
    });

    try {
      // Update scores if available (check for both undefined and null)
      // Note: Headphone optimization doesn't calculate preference scores, so they come back as null
      if (
        result.preference_score_before !== undefined &&
        result.preference_score_after !== undefined &&
        result.preference_score_before !== null &&
        result.preference_score_after !== null
      ) {
        console.log("[MAIN] 📊 Updating scores:", {
          before: result.preference_score_before,
          after: result.preference_score_after,
        });
        // Update scores in Step 3
        this.updateStep3Scores(
          result.preference_score_before,
          result.preference_score_after,
        );

        // Also update scores in the old location (for Step 4)
        this.uiManager.updateScores(
          result.preference_score_before,
          result.preference_score_after,
        );
      } else {
        console.log(
          "[MAIN] ℹ️  Skipping score update (scores not available for this optimization type)",
        );
      }

      // Store optimized filter parameters for later use
      if (result.filter_params) {
        const filterParams: FilterParam[] = [];
        for (let i = 0; i < result.filter_params.length; i += 3) {
          // Convert frequency from log space to linear space
          const logFreq = result.filter_params[i];
          const linearFreq = Math.pow(10, logFreq);
          filterParams.push({
            frequency: linearFreq,
            q: result.filter_params[i + 1],
            gain: result.filter_params[i + 2],
            enabled: true,
          });
        }
        // Store filters for later application
        this.optimizedFilters = filterParams;

        // Try to apply filters now if audio player is ready, but don't fail if it's not
        try {
          this.audioPlayer?.updateFilterParams(filterParams);
          this.audioPlayer?.setEQEnabled(true);
          console.log("[MAIN] ✅ Filters applied to audio player");
        } catch (error) {
          console.log(
            "[MAIN] ℹ️  Audio player not ready yet, filters will be applied when entering Step 4",
          );
        }
      }

      // Generate plots using Tauri backend
      console.log("[MAIN] 📊 Generating optimization plots...");
      await this.generateOptimizationPlots(result);
      console.log("[MAIN] ✅ Plots generated successfully");

      // Show close button instead of cancel button
      this.uiManager.showCloseButton();

      // Enable download button after successful optimization
      this.uiManager.enableDownloadButton();

      // Determine if this is speaker-based or curve+target optimization
      const hasSpinData = !!result.spin_details;

      // Configure plot visibility
      this.plotComposer.configureVerticalVisibility(hasSpinData);

      // Force layout recalculation after plots are updated
      this.plotManager.forceRecalculate();

      // Show results section in Step 3
      const step3Results = document.getElementById("step3-results");
      if (step3Results) {
        step3Results.style.display = "block";

        // Scroll to results section
        setTimeout(() => {
          step3Results.scrollIntoView({ behavior: "smooth", block: "start" });
        }, 300);
      }

      // Enable Step 4 for when user is ready to test with audio
      console.log("[MAIN] 🎵 Enabling Step 4 (Listening & Testing)...");
      this.stepNavigator.setStepEnabled(4, true);
      console.log("[MAIN] ✅ Step 4 enabled successfully");
    } catch (error) {
      console.error("Error processing optimization results:", error);
      this.uiManager.showError("Error processing results: " + error);
    }
  }

  /**
   * Update scores displayed in Step 3
   */
  private updateStep3Scores(scoreBefore: number, scoreAfter: number): void {
    const beforeEl = document.getElementById("step3_score_before");
    const afterEl = document.getElementById("step3_score_after");
    const improvementEl = document.getElementById("step3_score_improvement");

    if (beforeEl) {
      beforeEl.textContent = scoreBefore.toFixed(2);
    }

    if (afterEl) {
      afterEl.textContent = scoreAfter.toFixed(2);
    }

    if (improvementEl) {
      const improvement = scoreAfter - scoreBefore;
      const improvementPercent = (improvement / Math.abs(scoreBefore)) * 100;
      const sign = improvement >= 0 ? "+" : "";
      improvementEl.textContent = `${sign}${improvement.toFixed(2)} (${sign}${improvementPercent.toFixed(1)}%)`;
    }
  }

  private setEQEnabled(enabled: boolean): void {
    // This method is called by the AudioPlayer callback
    // It can be used to sync EQ state with other parts of the application if needed
    console.log(`[MAIN] EQ state changed to: ${enabled}`);
    // TODO
  }

  private handleOptimizationError(error: string): void {
    this.uiManager.showError(error);
    this.uiManager.setOptimizationRunning(false);

    // Keep download button disabled on error
    this.uiManager.disableDownloadButton();

    // Show close button instead of cancel button even on error
    this.uiManager.showCloseButton();
  }

  private async generateOptimizationPlots(
    result: OptimizationResult,
  ): Promise<void> {
    try {
      // ALWAYS generate the filter plot - backend always provides this data
      if (result.filter_params && result.filter_params.length > 0) {
        // Verify we have all required curves (backend always provides these)
        if (
          !result.filter_response ||
          !result.input_curve ||
          !result.deviation_curve
        ) {
          console.error("MISSING REQUIRED CURVES FROM BACKEND!");
          console.error("  filter_response:", !!result.filter_response);
          console.error("  input_curve:", !!result.input_curve);
          console.error("  deviation_curve:", !!result.deviation_curve);
          throw new Error("Backend did not return required curve data");
        }

        // Extract data from the optimization result
        const frequencies = result.filter_response.frequencies;
        const targetResponse = result.filter_response.curves["Target"];
        const inputCurve = result.input_curve.curves["Input"];
        const deviationCurve = result.deviation_curve.curves["Deviation"];

        if (!targetResponse || !inputCurve || !deviationCurve) {
          console.error("MISSING CURVE DATA IN RESPONSE!");
          console.error("  targetResponse:", !!targetResponse);
          console.error("  inputCurve:", !!inputCurve);
          console.error("  deviationCurve:", !!deviationCurve);
          throw new Error("Required curves missing from response data");
        }

        // Prepare parameters for backend plot generation
        const plotParams: PlotFiltersParams = {
          input_curve: {
            freq: frequencies,
            spl: inputCurve,
          },
          target_curve: {
            freq: frequencies,
            spl: targetResponse,
          },
          deviation_curve: {
            freq: frequencies,
            spl: deviationCurve,
          },
          optimized_params: result.filter_params,
          sample_rate: 48000, // Use default or get from form
          num_filters: result.filter_params.length / 3,
          peq_model:
            ((document.getElementById("peq_model") as HTMLSelectElement)
              ?.value as
              | "pk"
              | "hp-pk"
              | "hp-pk-lp"
              | "free-pk-free"
              | "free") || "pk",
          iir_hp_pk: false, // Deprecated
        };

        // Call backend to generate the 4-subplot plot
        const filterPlot = await AutoEQPlotAPI.generatePlotFilters(plotParams);

        // Update the filter plot with the Plotly JSON from backend
        this.plotComposer.updateFilterPlot(filterPlot);
      } else {
        console.error("No filter params in optimization result");
      }

      // Generate spinorama plots if we have spin data
      if (result.spin_details) {
        try {
          // Convert curves data to CurveData format
          const cea2034_curves: Record<
            string,
            { freq: number[]; spl: number[] }
          > = {};
          if (result.spin_details.curves) {
            Object.keys(result.spin_details.curves).forEach((key) => {
              const curveArray = result.spin_details!.curves[key];
              if (
                Array.isArray(curveArray) &&
                result.spin_details!.frequencies
              ) {
                cea2034_curves[key] = {
                  freq: result.spin_details!.frequencies,
                  spl: curveArray,
                };
              }
            });
          }

          // Extract EQ response for the spin plots
          const eq_response =
            result.filter_response?.curves["EQ Response"] || [];

          // Generate plots as Plotly JSON from backend
          const spinParams: PlotSpinParams = {
            cea2034_curves,
            eq_response, // Add eq_response to show "with EQ" traces
            frequencies: result.spin_details.frequencies,
          };

          const spinPlot = await AutoEQPlotAPI.generatePlotSpin(spinParams);

          // Update spin plot with Plotly JSON
          this.plotComposer.updateSpinPlot(spinPlot);

          const detailsPlot =
            await AutoEQPlotAPI.generatePlotSpinDetails(spinParams);

          // Update details plot with Plotly JSON
          await this.plotComposer.generateDetailsPlot(detailsPlot);

          const tonalPlot =
            await AutoEQPlotAPI.generatePlotSpinTonal(spinParams);

          // Update tonal plot with Plotly JSON
          this.plotComposer.updateTonalPlot(tonalPlot);
        } catch (plotError) {
          console.error("Error generating spinorama plots:", plotError);
        }
      }
    } catch (error) {
      console.error("Error in generateOptimizationPlots:", error);
      // Don't throw - we want optimization to continue even if plot generation fails
    }
  }

  private async cancelOptimization(): Promise<void> {
    try {
      await this.optimizationManager.cancelOptimization();
      this.uiManager.closeOptimizationModal();
      this.uiManager.setOptimizationRunning(false);
    } catch (error) {
      console.error("Error cancelling optimization:", error);
      // Even if cancellation fails, we should still update the UI
      this.uiManager.closeOptimizationModal();
      this.uiManager.setOptimizationRunning(false);
      this.uiManager.showError(
        "Failed to cancel optimization, but stopping locally",
      );
    }
  }

  /**
   * Setup validation for speaker selection
   */
  private setupSpeakerValidation(): void {
    const speakerInput = document.getElementById("speaker") as HTMLInputElement;
    const versionSelect = document.getElementById(
      "version",
    ) as HTMLSelectElement;
    const measurementSelect = document.getElementById(
      "measurement",
    ) as HTMLSelectElement;
    const step2NextBtn = document.getElementById(
      "step2_next_btn",
    ) as HTMLButtonElement;

    const checkValidity = () => {
      if (speakerInput && versionSelect && measurementSelect && step2NextBtn) {
        const isValid =
          speakerInput.value.trim() !== "" &&
          versionSelect.value !== "" &&
          measurementSelect.value !== "";
        step2NextBtn.disabled = !isValid;
      }
    };

    if (speakerInput) speakerInput.addEventListener("input", checkValidity);
    if (versionSelect) versionSelect.addEventListener("change", checkValidity);
    if (measurementSelect)
      measurementSelect.addEventListener("change", checkValidity);
  }

  /**
   * Setup validation for headphone selection
   */
  private setupHeadphoneValidation(): void {
    const headphoneCurveInput = document.getElementById(
      "headphone_curve_path",
    ) as HTMLInputElement;
    const headphoneTargetSelect = document.getElementById(
      "headphone_target",
    ) as HTMLSelectElement;
    const step2NextBtn = document.getElementById(
      "step2_next_btn",
    ) as HTMLButtonElement;

    const checkValidity = () => {
      if (headphoneCurveInput && headphoneTargetSelect && step2NextBtn) {
        const isValid =
          headphoneCurveInput.value.trim() !== "" &&
          headphoneTargetSelect.value !== "";
        step2NextBtn.disabled = !isValid;
      }
    };

    if (headphoneCurveInput)
      headphoneCurveInput.addEventListener("input", checkValidity);
    if (headphoneTargetSelect)
      headphoneTargetSelect.addEventListener("change", checkValidity);
  }

  /**
   * Setup validation for file selection
   */
  private setupFileValidation(): void {
    const curvePathInput = document.getElementById(
      "curve_path",
    ) as HTMLInputElement;
    const step2NextBtn = document.getElementById(
      "step2_next_btn",
    ) as HTMLButtonElement;

    const checkValidity = () => {
      if (curvePathInput && step2NextBtn) {
        const isValid = curvePathInput.value.trim() !== "";
        step2NextBtn.disabled = !isValid;
      }
    };

    if (curvePathInput) curvePathInput.addEventListener("input", checkValidity);
  }

  // Cleanup method
  destroy(): void {
    this.optimizationManager?.destroy();
    this.audioPlayer?.destroy();
    this.plotManager?.destroy();
  }
}

// Initialize the application when DOM is loaded
document.addEventListener("DOMContentLoaded", () => {
  const app = new AutoEQApplication();

  // Store app instance globally for debugging
  (window as unknown as { autoEQApp: AutoEQApplication }).autoEQApp = app;

  // Cleanup on page unload
  window.addEventListener("beforeunload", () => {
    app.destroy();
  });
});

export { AutoEQApplication };
