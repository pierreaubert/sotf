// Capture Modal Management - Handles the capture modal UI and interactions

import {
  CaptureController,
  type CaptureParameters,
} from "./capture-controller";
import { CaptureStorage } from "./capture-storage";
import { CSVExporter } from "./csv-export";
import { CaptureGraphRenderer } from "./capture-graph";
import { RoutingMatrix, type RoutingConfig } from "@audio-player/audio-routing";
import { invoke } from "@tauri-apps/api/core";

export interface CaptureData {
  frequencies: number[];
  rawMagnitudes: number[];
  smoothedMagnitudes: number[];
  rawPhase: number[];
  smoothedPhase: number[];
  metadata: {
    timestamp: Date;
    deviceName: string;
    signalType: "sweep" | "white" | "pink";
    duration: number;
    sampleRate: number;
    outputChannel: "left" | "right" | "both" | "default";
  };
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
  outputChannel?: "left" | "right" | "both" | "default";
}

export class CaptureModalManager {
  // Panel elements (formerly modal elements)
  private capturePanel: HTMLElement | null = null;
  private captureModalGraph: HTMLCanvasElement | null = null;
  private captureModalPlaceholder: HTMLElement | null = null;
  private captureModalProgress: HTMLElement | null = null;
  private captureModalProgressFill: HTMLElement | null = null;
  private captureModalStatus: HTMLElement | null = null;
  private captureModalStart: HTMLButtonElement | null = null;
  private captureModalStop: HTMLButtonElement | null = null;
  private captureModalExport: HTMLButtonElement | null = null;

  // Control elements
  private modalCaptureDevice: HTMLSelectElement | null = null;
  private modalOutputDevice: HTMLSelectElement | null = null;
  private modalCaptureVolume: HTMLInputElement | null = null;
  private modalCaptureVolumeValue: HTMLElement | null = null;
  private inputChannelsInfo: HTMLElement | null = null;
  private modalOutputChannel: HTMLSelectElement | null = null;
  private modalOutputVolume: HTMLInputElement | null = null;
  private modalOutputVolumeValue: HTMLElement | null = null;
  private outputChannelsInfo: HTMLElement | null = null;
  private modalSignalType: HTMLSelectElement | null = null;
  private modalSweepDuration: HTMLSelectElement | null = null;
  private modalCaptureSampleRate: HTMLElement | null = null;
  private modalCaptureBitDepth: HTMLElement | null = null;
  private modalCaptureSPL: HTMLElement | null = null;
  private modalOutputSampleRate: HTMLElement | null = null;
  private modalOutputBitDepth: HTMLElement | null = null;
  private modalSweepDurationContainer: HTMLElement | null = null;
  private capturePhaseToggle: HTMLInputElement | null = null;
  private captureSmoothingSelect: HTMLSelectElement | null = null;
  private captureCalibrationFile: HTMLInputElement | null = null;
  private captureCalibrationBtn: HTMLButtonElement | null = null;
  private captureCalibrationClear: HTMLButtonElement | null = null;
  private captureChannelSelect: HTMLSelectElement | null = null;
  private inputRoutingBtn: HTMLButtonElement | null = null;
  private outputRoutingBtn: HTMLButtonElement | null = null;

  // Records management elements
  private recordsSidebar: HTMLElement | null = null;
  private recordsList: HTMLElement | null = null;
  private recordsToggleBtn: HTMLButtonElement | null = null;
  private recordsSelectAllBtn: HTMLButtonElement | null = null;
  private recordsDeselectAllBtn: HTMLButtonElement | null = null;
  private recordsDeleteSelectedBtn: HTMLButtonElement | null = null;
  private selectedRecordIds: Set<string> = new Set();

  // State
  private captureGraphRenderer: CaptureGraphRenderer | null = null;
  private captureController: CaptureController | null = null;
  private currentCaptureData: CaptureData | null = null;
  private captureVolume: number = 70;
  private outputVolume: number = 50;
  private splPollingInterval: number | null = null;

  // Routing matrices
  private inputRoutingMatrix: RoutingMatrix | null = null;
  private outputRoutingMatrix: RoutingMatrix | null = null;
  private deviceChannelInfo: { input: number | null; output: number | null } = {
    input: null,
    output: null,
  };

  // Callbacks
  private onCaptureComplete?: (
    frequencies: number[],
    magnitudes: number[],
  ) => void;
  private outputDeviceChangeCallback: ((deviceId: string) => void) | null =
    null;
  private deviceStatusCallback?: (
    status: "success" | "error" | "neutral",
  ) => void;

  constructor() {
    this.initializeElements();
    this.setupEventListeners();
  }

  private initializeElements(): void {
    // Panel container (can be either capture_panel or capture_modal for backward compatibility)
    this.capturePanel =
      document.getElementById("capture_panel") ||
      document.getElementById("capture_modal");
    this.captureModalGraph = document.getElementById(
      "capture_modal_graph",
    ) as HTMLCanvasElement;
    this.captureModalPlaceholder = document.getElementById(
      "capture_modal_placeholder",
    );
    this.captureModalProgress = document.getElementById(
      "capture_modal_progress",
    );
    this.captureModalProgressFill = document.getElementById(
      "capture_modal_progress_fill",
    );
    this.captureModalStatus = document.getElementById("capture_modal_status");

    // Buttons
    this.captureModalStart = document.getElementById(
      "capture_modal_start",
    ) as HTMLButtonElement;
    this.captureModalStop = document.getElementById(
      "capture_modal_stop",
    ) as HTMLButtonElement;
    this.captureModalExport = document.getElementById(
      "capture_modal_export",
    ) as HTMLButtonElement;

    // Device controls
    this.modalCaptureDevice = document.getElementById(
      "modal_capture_device",
    ) as HTMLSelectElement;
    this.modalOutputDevice = document.getElementById(
      "modal_output_device",
    ) as HTMLSelectElement;
    this.modalCaptureVolume = document.getElementById(
      "modal_capture_volume",
    ) as HTMLInputElement;
    this.modalCaptureVolumeValue = document.getElementById(
      "modal_capture_volume_value",
    );
    this.inputChannelsInfo = document.getElementById("input_channels_info");
    this.modalOutputChannel = document.getElementById(
      "modal_output_channel",
    ) as HTMLSelectElement;
    this.modalOutputVolume = document.getElementById(
      "modal_output_volume",
    ) as HTMLInputElement;
    this.modalOutputVolumeValue = document.getElementById(
      "modal_output_volume_value",
    );
    this.outputChannelsInfo = document.getElementById("output_channels_info");

    // Signal controls
    this.modalSignalType = document.getElementById(
      "modal_signal_type",
    ) as HTMLSelectElement;
    this.modalSweepDuration = document.getElementById(
      "modal_sweep_duration",
    ) as HTMLSelectElement;
    this.modalCaptureSampleRate = document.getElementById(
      "modal_capture_sample_rate",
    );
    this.modalCaptureBitDepth = document.getElementById(
      "modal_capture_bit_depth",
    );
    this.modalCaptureSPL = document.getElementById("modal_capture_spl");
    this.modalOutputSampleRate = document.getElementById(
      "modal_output_sample_rate",
    );
    this.modalOutputBitDepth = document.getElementById(
      "modal_output_bit_depth",
    );
    this.modalSweepDurationContainer = document.getElementById(
      "modal_sweep_duration_container",
    );

    // Graph controls
    this.capturePhaseToggle = document.getElementById(
      "capture_phase_toggle",
    ) as HTMLInputElement;
    this.captureSmoothingSelect = document.getElementById(
      "capture_smoothing_select",
    ) as HTMLSelectElement;
    this.captureCalibrationFile = document.getElementById(
      "capture_calibration_file",
    ) as HTMLInputElement;
    this.captureCalibrationBtn = document.getElementById(
      "capture_calibration_btn",
    ) as HTMLButtonElement;
    this.captureCalibrationClear = document.getElementById(
      "capture_calibration_clear",
    ) as HTMLButtonElement;
    this.captureChannelSelect = document.getElementById(
      "capture_channel_select",
    ) as HTMLSelectElement;

    // Routing buttons
    this.inputRoutingBtn = document.getElementById(
      "input_routing_btn",
    ) as HTMLButtonElement;
    this.outputRoutingBtn = document.getElementById(
      "output_routing_btn",
    ) as HTMLButtonElement;

    // Records management elements
    this.recordsSidebar = document.getElementById("capture_records_sidebar");
    this.recordsList = document.getElementById("capture_records_list");
    this.recordsToggleBtn = document.getElementById(
      "records_toggle",
    ) as HTMLButtonElement;
    this.recordsSelectAllBtn = document.getElementById(
      "records_select_all",
    ) as HTMLButtonElement;
    this.recordsDeselectAllBtn = document.getElementById(
      "records_deselect_all",
    ) as HTMLButtonElement;
    this.recordsDeleteSelectedBtn = document.getElementById(
      "records_delete_selected",
    ) as HTMLButtonElement;
  }

  private setupEventListeners(): void {
    // Capture control buttons
    this.captureModalStart?.addEventListener("click", async () => {
      await this.startCapture();
    });

    this.captureModalStop?.addEventListener("click", () => {
      this.stopCapture();
    });

    this.captureModalExport?.addEventListener("click", () => {
      this.exportCSV();
    });

    // Signal type change handler
    this.modalSignalType?.addEventListener("change", () => {
      const signalType = this.modalSignalType?.value;
      if (this.modalSweepDurationContainer) {
        this.modalSweepDurationContainer.style.display =
          signalType === "sweep" ? "flex" : "none";
      }
    });

    // Output channel change handler
    this.modalOutputChannel?.addEventListener("change", () => {
      const outputChannel = this.modalOutputChannel?.value || "both";
      this.updateChannelSelectOptions(outputChannel);
      this.updateSampleRateForDevice();
    });

    // Device change handlers
    this.modalCaptureDevice?.addEventListener("change", async () => {
      this.updateInputDeviceInfo();
      await this.updateSampleRateForDevice();
      await this.updateOutputChannelOptions();
    });

    this.modalOutputDevice?.addEventListener("change", async () => {
      const deviceId = this.modalOutputDevice?.value || "default";
      this.updateOutputDeviceInfo();
      await this.updateOutputChannelOptions();
      if (this.outputDeviceChangeCallback) {
        this.outputDeviceChangeCallback(deviceId);
      }
    });

    // Volume slider handlers
    this.modalCaptureVolume?.addEventListener("input", () => {
      this.onVolumeChange();
    });

    this.modalOutputVolume?.addEventListener("input", () => {
      this.onOutputVolumeChange();
    });

    // Phase toggle handler
    this.capturePhaseToggle?.addEventListener("change", () => {
      this.onPhaseToggleChange();
    });

    // Smoothing selector handler
    this.captureSmoothingSelect?.addEventListener("change", () => {
      this.onSmoothingChange();
    });

    // Calibration file handlers
    this.captureCalibrationBtn?.addEventListener("click", () => {
      this.captureCalibrationFile?.click();
    });

    this.captureCalibrationFile?.addEventListener("change", (e) => {
      this.onCalibrationFileChange(e);
    });

    this.captureCalibrationClear?.addEventListener("click", () => {
      this.clearCalibrationFile();
    });

    // Channel visibility control
    this.captureChannelSelect?.addEventListener("change", () => {
      this.onChannelDisplayChange();
    });

    // Routing button handlers
    this.inputRoutingBtn?.addEventListener("click", () => {
      this.showRoutingMatrix("input");
    });

    this.outputRoutingBtn?.addEventListener("click", () => {
      this.showRoutingMatrix("output");
    });

    // Records panel event handlers
    this.recordsToggleBtn?.addEventListener("click", () => {
      this.toggleRecordsSidebar();
    });

    this.recordsSelectAllBtn?.addEventListener("click", () => {
      this.selectAllRecords();
    });

    this.recordsDeselectAllBtn?.addEventListener("click", () => {
      this.deselectAllRecords();
    });

    this.recordsDeleteSelectedBtn?.addEventListener("click", () => {
      this.deleteSelectedRecords();
    });
  }

  // Public API
  public async initialize(): Promise<void> {
    console.log("Initializing capture panel...");

    if (!this.capturePanel) {
      console.error("Capture panel not found");
      return;
    }

    await this.initializePanel();
  }

  public cleanup(): void {
    console.log("Cleaning up capture panel...");

    if (!this.capturePanel) return;

    this.stopCapture();

    if (this.captureGraphRenderer) {
      this.captureGraphRenderer.destroy();
      this.captureGraphRenderer = null;
    }
  }

  // Backward compatibility - keep these methods but they just call initialize/cleanup
  public async openModal(): Promise<void> {
    await this.initialize();
  }

  public closeModal(): void {
    this.cleanup();
  }

  public setCaptureCompleteCallback(
    callback: (frequencies: number[], magnitudes: number[]) => void,
  ): void {
    this.onCaptureComplete = callback;
  }

  public setOutputDeviceChangeCallback(
    callback: (deviceId: string) => void,
  ): void {
    this.outputDeviceChangeCallback = callback;
  }

  public setDeviceStatusCallback(
    callback: (status: "success" | "error" | "neutral") => void,
  ): void {
    this.deviceStatusCallback = callback;
  }

  // Private methods
  private async initializePanel(): Promise<void> {
    console.log("Initializing capture panel...");

    // Initialize device status to neutral
    if (this.deviceStatusCallback) {
      this.deviceStatusCallback("neutral");
    }

    // Initialize graph renderer
    if (this.captureModalGraph) {
      try {
        this.captureGraphRenderer = new CaptureGraphRenderer(
          this.captureModalGraph,
        );
        this.captureGraphRenderer.renderPlaceholder();

        // Expose renderer for debugging
        (
          window as unknown as {
            debugCaptureGraphRenderer: CaptureGraphRenderer;
          }
        ).debugCaptureGraphRenderer = this.captureGraphRenderer;
      } catch (error) {
        console.error("Error initializing capture graph:", error);
      }
    }

    // Show placeholder, hide graph and progress
    if (this.captureModalPlaceholder) {
      this.captureModalPlaceholder.style.display = "flex";
    }
    if (this.captureModalProgress) {
      this.captureModalProgress.style.display = "none";
    }

    // Reset button states
    this.resetButtons();

    // Populate audio devices
    await this.populateAudioDevices();

    // Update device info badges
    this.updateInputDeviceInfo();
    this.updateOutputDeviceInfo();

    // Update sample rate and bit depth for current device
    await this.updateSampleRateForDevice();

    // Update output channel options
    await this.updateOutputChannelOptions();

    // Initialize volume sliders appearance
    this.onVolumeChange();
    this.onOutputVolumeChange();

    // Render records list
    await this.renderRecordsList();
  }

  private resetButtons(): void {
    if (this.captureModalStart) {
      this.captureModalStart.style.display = "inline-flex";
      this.captureModalStart.disabled = false;
    }
    if (this.captureModalStop) {
      this.captureModalStop.style.display = "none";
    }
    if (this.captureModalExport) {
      this.captureModalExport.style.display = "none";
    }
  }

  private async populateAudioDevices(): Promise<void> {
    if (!this.modalCaptureDevice || !this.modalOutputDevice) return;

    try {
      // Initialize capture controller if not already done
      if (!this.captureController) {
        this.captureController = new CaptureController();
      }

      const devices = await this.captureController.getAudioDevices();

      // Populate input devices
      this.modalCaptureDevice.innerHTML = "";
      devices.input.forEach((device) => {
        console.log("[CaptureModal] Adding input device:", {
          value: device.value,
          label: device.label,
          info: device.info,
          hasInfo: !!device.info,
          fullDevice: device,
        });
        const option = document.createElement("option");
        option.value = device.value;
        // Add channel info to the label if available
        const displayLabel = device.info
          ? `${device.label} (${device.info})`
          : device.label;
        option.textContent = displayLabel;
        console.log("[CaptureModal] Display label for input:", displayLabel);
        if (device.info) {
          option.title = device.info;
        }
        this.modalCaptureDevice?.appendChild(option);
      });

      // Debug: Check what's actually in the dropdown
      if (this.modalCaptureDevice) {
        const options = Array.from(this.modalCaptureDevice.options);
        console.log("[CaptureModal] INPUT dropdown actual options:");
        options.forEach((opt) => {
          console.log(
            `  - value: "${opt.value}", text: "${opt.textContent}", title: "${opt.title}"`,
          );
        });
      }

      console.log(`Populated ${devices.input.length} input devices`);

      // Update input device channel info badge
      this.updateInputDeviceInfo();

      // Populate output devices
      this.modalOutputDevice.innerHTML = "";
      devices.output.forEach((device) => {
        console.log("[CaptureModal] Adding output device:", {
          value: device.value,
          label: device.label,
          info: device.info,
          hasInfo: !!device.info,
          fullDevice: device,
        });
        const option = document.createElement("option");
        option.value = device.value;
        // Add channel info to the label if available
        const displayLabel = device.info
          ? `${device.label} (${device.info})`
          : device.label;
        option.textContent = displayLabel;
        console.log("[CaptureModal] Display label for output:", displayLabel);
        if (device.info) {
          option.title = device.info;
        }
        this.modalOutputDevice?.appendChild(option);
      });

      // Debug: Check what's actually in the dropdown
      if (this.modalOutputDevice) {
        const options = Array.from(this.modalOutputDevice.options);
        console.log("[CaptureModal] OUTPUT dropdown actual options:");
        options.forEach((opt) => {
          console.log(
            `  - value: "${opt.value}", text: "${opt.textContent}", title: "${opt.title}"`,
          );
        });
      }

      console.log(`Populated ${devices.output.length} output devices`);

      // Update output device channel info badge
      this.updateOutputDeviceInfo();
    } catch (error) {
      console.error("Error populating audio devices:", error);
    }
  }

  private updateInputDeviceInfo(): void {
    if (!this.inputChannelsInfo || !this.modalCaptureDevice) return;

    const selectedDevice = this.modalCaptureDevice.value;
    if (!selectedDevice || selectedDevice === "default") {
      // For default device, we don't know the exact channels
      this.inputChannelsInfo.textContent = "??";
      this.deviceChannelInfo.input = null;
      return;
    }

    // Extract channel info from the selected option's title or text
    const selectedOption =
      this.modalCaptureDevice.options[this.modalCaptureDevice.selectedIndex];
    const deviceText = selectedOption.textContent || "";

    // Parse channel info from text like "Device Name (1ch 44kHz)"
    const channelMatch = deviceText.match(/(\d+)ch/);
    if (channelMatch) {
      const channels = parseInt(channelMatch[1]);
      this.inputChannelsInfo.textContent = channelMatch[1];
      this.deviceChannelInfo.input = channels;

      // Update routing matrix if it exists
      if (this.inputRoutingMatrix) {
        this.inputRoutingMatrix.updateChannelCount(channels);
      }
    } else {
      this.inputChannelsInfo.textContent = "??";
      this.deviceChannelInfo.input = null;
    }
  }

  private async startCapture(): Promise<void> {
    console.log("Starting capture...");

    if (!this.captureModalStart || !this.captureModalStop) return;

    try {
      // Update button states
      this.captureModalStart.style.display = "none";
      this.captureModalStop.style.display = "inline-flex";

      // Show progress
      if (this.captureModalProgress) {
        this.captureModalProgress.style.display = "block";
      }
      if (this.captureModalPlaceholder) {
        this.captureModalPlaceholder.style.display = "none";
      }

      // Update status
      if (this.captureModalStatus) {
        this.captureModalStatus.textContent = "Starting capture...";
      }

      // Get capture parameters
      const sampleRateText =
        this.modalCaptureSampleRate?.textContent || "48kHz";
      const sampleRate = sampleRateText.includes("kHz")
        ? parseFloat(sampleRateText.replace("kHz", "")) * 1000
        : parseInt(sampleRateText.replace("Hz", ""));

      const captureParams: CaptureParameters = {
        inputDevice: this.modalCaptureDevice?.value || "default",
        outputDevice: this.modalOutputDevice?.value || "default",
        outputChannel:
          (this.modalOutputChannel?.value as
            | "left"
            | "right"
            | "both"
            | "default") || "both",
        signalType:
          (this.modalSignalType?.value as "sweep" | "white" | "pink") ||
          "sweep",
        duration: parseInt(this.modalSweepDuration?.value || "10"),
        sampleRate: sampleRate,
        inputVolume: this.captureVolume,
        outputVolume: this.outputVolume,
      };

      console.log("Capture parameters:", captureParams);

      // Update status message
      if (this.captureModalStatus) {
        const signalName =
          captureParams.signalType === "sweep"
            ? "frequency sweep"
            : `${captureParams.signalType} noise`;
        this.captureModalStatus.textContent = `Playing ${signalName} and capturing response...`;
      }

      // Initialize capture controller if needed
      if (!this.captureController) {
        this.captureController = new CaptureController();
      }

      // Start capture
      const result = await this.captureController.startCapture(captureParams);

      if (result.success && result.frequencies.length > 0) {
        await this.handleSuccessfulCapture(result, captureParams);
      } else {
        throw new Error(result.error || "Capture failed");
      }
    } catch (error) {
      console.error("Capture error:", error);
      this.handleCaptureError(error);
    }
  }

  private async handleSuccessfulCapture(
    result: {
      frequencies: number[];
      magnitudes: number[];
      phases?: number[];
      success: boolean;
      error?: string;
    },
    params: CaptureParameters,
  ): Promise<void> {
    console.log("Processing successful capture...");

    const octaveFraction = this.captureSmoothingSelect
      ? parseInt(this.captureSmoothingSelect.value)
      : 3;

    const smoothedMagnitudes = CaptureGraphRenderer.applySmoothing(
      result.frequencies,
      result.magnitudes,
      octaveFraction,
    );

    let smoothedPhase: number[] = [];
    if (result.phases && result.phases.length > 0) {
      smoothedPhase = CaptureGraphRenderer.applyPhaseSmoothing(
        result.frequencies,
        result.phases,
        octaveFraction,
      );
    }

    // Store capture data
    this.currentCaptureData = {
      frequencies: result.frequencies,
      rawMagnitudes: result.magnitudes,
      smoothedMagnitudes,
      rawPhase: result.phases || [],
      smoothedPhase,
      metadata: {
        timestamp: new Date(),
        deviceName:
          params.inputDevice === "default"
            ? "Default Microphone"
            : "Selected Device",
        signalType: params.signalType,
        duration: params.duration,
        sampleRate: params.sampleRate,
        outputChannel: params.outputChannel,
      },
      outputChannel: params.outputChannel,
    };

    // Update graph
    if (this.captureGraphRenderer) {
      this.captureGraphRenderer.renderGraph({
        frequencies: result.frequencies,
        rawMagnitudes: result.magnitudes,
        smoothedMagnitudes,
        rawPhase: result.phases,
        smoothedPhase,
        outputChannel: params.outputChannel,
      });
    }

    // Update status
    if (this.captureModalStatus) {
      this.captureModalStatus.textContent = `✅ Captured ${result.frequencies.length} frequency points`;
    }

    // Update button states
    this.captureModalStart!.style.display = "inline-flex";
    this.captureModalStop!.style.display = "none";
    this.captureModalExport!.style.display = "inline-flex";

    // Progress to 100%
    if (this.captureModalProgressFill) {
      this.captureModalProgressFill.style.width = "100%";
    }

    // Save to storage
    try {
      const captureId = CaptureStorage.saveCapture({
        timestamp: this.currentCaptureData.metadata.timestamp,
        deviceName: this.currentCaptureData.metadata.deviceName,
        signalType: this.currentCaptureData.metadata.signalType,
        duration: this.currentCaptureData.metadata.duration,
        sampleRate: this.currentCaptureData.metadata.sampleRate,
        outputChannel: this.currentCaptureData.metadata.outputChannel,
        frequencies: this.currentCaptureData.frequencies,
        rawMagnitudes: this.currentCaptureData.rawMagnitudes,
        smoothedMagnitudes: this.currentCaptureData.smoothedMagnitudes,
        rawPhase: this.currentCaptureData.rawPhase,
        smoothedPhase: this.currentCaptureData.smoothedPhase,
      });
      console.log("Capture saved with ID:", captureId);

      // Refresh records list
      await this.renderRecordsList();

      // Refresh channel select options to include new sweep
      this.updateChannelSelectOptions(
        this.currentCaptureData.metadata.outputChannel,
      );
    } catch (error) {
      console.error("Failed to save capture:", error);
    }

    // Call completion callback
    if (this.onCaptureComplete) {
      this.onCaptureComplete(result.frequencies, smoothedMagnitudes);
    }

    console.log("Capture completed successfully");
  }

  private handleCaptureError(error: unknown): void {
    console.error("Capture failed:", error);

    const errorMessage =
      error instanceof Error ? error.message : "Unknown error";

    if (this.captureModalStatus) {
      let statusHTML = `<div class="capture-error"><strong>❌ Capture Failed</strong><br><span class="error-message">${errorMessage}</span>`;

      if (errorMessage.includes("permission denied")) {
        statusHTML +=
          '<br><br><div class="error-instructions">📝 <strong>To fix this:</strong><br>1. Click the microphone icon in your browser\'s address bar<br>2. Select "Always allow" for microphone access<br>3. Refresh the page and try again</div>';
      } else if (errorMessage.includes("No microphone found")) {
        statusHTML +=
          '<br><br><div class="error-instructions">📝 <strong>To fix this:</strong><br>1. Connect a microphone to your device<br>2. Check your system audio settings<br>3. Try a different input device</div>';
      } else {
        statusHTML +=
          '<br><br><div class="error-instructions">📝 <strong>Try these steps:</strong><br>1. Refresh the page<br>2. Check your microphone connection<br>3. Try a different browser</div>';
      }

      statusHTML += "</div>";
      this.captureModalStatus.innerHTML = statusHTML;
    }

    this.resetButtons();

    if (this.captureModalStart) {
      this.captureModalStart.disabled = false;
      this.captureModalStart.textContent = "Retry Capture";
    }

    if (this.captureModalProgress) {
      this.captureModalProgress.style.display = "none";
    }
    if (this.captureModalPlaceholder) {
      this.captureModalPlaceholder.style.display = "flex";
    }
  }

  private stopCapture(): void {
    console.log("Stopping capture...");

    if (this.captureController) {
      this.captureController.stopCapture();
    }

    this.resetButtons();
    if (this.captureModalStatus) {
      this.captureModalStatus.textContent = "Capture stopped";
    }
  }

  private exportCSV(): void {
    console.log("Exporting capture to CSV...");

    if (!this.currentCaptureData) {
      console.warn("No capture data available");
      alert("No capture data available to export");
      return;
    }

    const exportData = {
      frequencies: this.currentCaptureData.frequencies,
      rawMagnitudes: this.currentCaptureData.rawMagnitudes,
      smoothedMagnitudes: this.currentCaptureData.smoothedMagnitudes,
      rawPhase: this.currentCaptureData.rawPhase,
      smoothedPhase: this.currentCaptureData.smoothedPhase,
      metadata: this.currentCaptureData.metadata,
    };

    try {
      CSVExporter.exportToCSV(exportData);
    } catch (error) {
      console.error("Failed to export CSV:", error);
      alert(
        "Failed to export CSV file: " +
          (error instanceof Error ? error.message : "Unknown error"),
      );
    }
  }

  private onVolumeChange(): void {
    if (!this.modalCaptureVolume || !this.modalCaptureVolumeValue) return;

    const volume = parseInt(this.modalCaptureVolume.value);
    this.captureVolume = volume;
    this.modalCaptureVolumeValue.textContent = `${volume}%`;

    const percentage = volume;
    this.modalCaptureVolume.style.background = `linear-gradient(to right,
      var(--button-primary) 0%,
      var(--button-primary) ${percentage}%,
      var(--bg-accent) ${percentage}%,
      var(--bg-accent) 100%)`;
  }

  private onOutputVolumeChange(): void {
    if (!this.modalOutputVolume || !this.modalOutputVolumeValue) return;

    const volume = parseInt(this.modalOutputVolume.value);
    this.outputVolume = volume;
    this.modalOutputVolumeValue.textContent = `${volume}%`;

    const percentage = volume;
    this.modalOutputVolume.style.background = `linear-gradient(to right,
      var(--button-primary) 0%,
      var(--button-primary) ${percentage}%,
      var(--bg-accent) ${percentage}%,
      var(--bg-accent) 100%)`;
  }

  private onPhaseToggleChange(): void {
    if (this.captureGraphRenderer && this.capturePhaseToggle) {
      const showPhase = this.capturePhaseToggle.checked;
      this.captureGraphRenderer.setPhaseVisibility(showPhase);

      if (this.currentCaptureData) {
        this.captureGraphRenderer.renderGraph({
          frequencies: this.currentCaptureData.frequencies,
          rawMagnitudes: this.currentCaptureData.rawMagnitudes,
          smoothedMagnitudes: this.currentCaptureData.smoothedMagnitudes,
          rawPhase: this.currentCaptureData.rawPhase,
          smoothedPhase: this.currentCaptureData.smoothedPhase,
        });
      } else {
        this.captureGraphRenderer.renderPlaceholder();
      }
    }
  }

  private onSmoothingChange(): void {
    if (this.currentCaptureData && this.captureSmoothingSelect) {
      const octaveFraction = parseInt(this.captureSmoothingSelect.value);
      console.log("Smoothing changed to 1/" + octaveFraction + " octave");
      this.reprocessCaptureData(octaveFraction);
    }
  }

  private async reprocessCaptureData(octaveFraction: number): Promise<void> {
    if (!this.currentCaptureData) return;

    try {
      const smoothedMagnitudes = CaptureGraphRenderer.applySmoothing(
        this.currentCaptureData.frequencies,
        this.currentCaptureData.rawMagnitudes,
        octaveFraction,
      );

      let smoothedPhase: number[] = [];
      if (this.currentCaptureData.rawPhase.length > 0) {
        smoothedPhase = CaptureGraphRenderer.applyPhaseSmoothing(
          this.currentCaptureData.frequencies,
          this.currentCaptureData.rawPhase,
          octaveFraction,
        );
      }

      this.currentCaptureData.smoothedMagnitudes = smoothedMagnitudes;
      this.currentCaptureData.smoothedPhase = smoothedPhase;

      if (this.captureGraphRenderer) {
        this.captureGraphRenderer.renderGraph({
          frequencies: this.currentCaptureData.frequencies,
          rawMagnitudes: this.currentCaptureData.rawMagnitudes,
          smoothedMagnitudes: this.currentCaptureData.smoothedMagnitudes,
          rawPhase: this.currentCaptureData.rawPhase,
          smoothedPhase: this.currentCaptureData.smoothedPhase,
          channelData: this.currentCaptureData.channelData,
          outputChannel: this.currentCaptureData.outputChannel,
        });
      }
    } catch (error) {
      console.error("Error reprocessing capture data:", error);
    }
  }

  private async onCalibrationFileChange(event: Event): Promise<void> {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];

    if (!file) return;

    console.log("Loading calibration file:", file.name);

    try {
      const text = await file.text();
      const { frequencies, magnitudes } = this.parseCalibrationFile(text);

      if (frequencies.length === 0 || magnitudes.length === 0) {
        throw new Error("No valid data found in calibration file");
      }

      if (frequencies.length !== magnitudes.length) {
        throw new Error(
          "Frequency and magnitude arrays have different lengths",
        );
      }

      // Set calibration data in graph renderer
      if (this.captureGraphRenderer) {
        this.captureGraphRenderer.setCalibrationData(frequencies, magnitudes);

        // Update button states
        if (this.captureCalibrationClear) {
          this.captureCalibrationClear.style.display = "inline-flex";
        }
        if (this.captureCalibrationBtn) {
          this.captureCalibrationBtn.textContent = "✓ Loaded";
        }

        // Re-render current data if available
        if (this.currentCaptureData) {
          this.captureGraphRenderer.renderGraph({
            frequencies: this.currentCaptureData.frequencies,
            rawMagnitudes: this.currentCaptureData.rawMagnitudes,
            smoothedMagnitudes: this.currentCaptureData.smoothedMagnitudes,
            rawPhase: this.currentCaptureData.rawPhase,
            smoothedPhase: this.currentCaptureData.smoothedPhase,
          });
        }

        console.log(
          `Calibration loaded: ${frequencies.length} points from ${file.name}`,
        );
      }
    } catch (error) {
      console.error("Error loading calibration file:", error);
      alert(
        `Failed to load calibration file: ${error instanceof Error ? error.message : "Unknown error"}`,
      );

      // Reset file input
      if (input) {
        input.value = "";
      }
    }
  }

  private parseCalibrationFile(text: string): {
    frequencies: number[];
    magnitudes: number[];
  } {
    const frequencies: number[] = [];
    const magnitudes: number[] = [];

    const lines = text
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.length > 0);

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];

      // Skip comments and header lines
      if (
        line.startsWith("#") ||
        line.startsWith("//") ||
        (i === 0 &&
          (line.toLowerCase().includes("frequency") ||
            line.toLowerCase().includes("freq")))
      ) {
        continue;
      }

      // Parse data lines
      const parts = line.split(/[,\t\s]+/).filter((part) => part.length > 0);

      if (parts.length >= 2) {
        const freq = parseFloat(parts[0]);
        const mag = parseFloat(parts[1]);

        if (!isNaN(freq) && !isNaN(mag) && freq > 0) {
          frequencies.push(freq);
          magnitudes.push(mag);
        }
      }
    }

    console.log(`Parsed ${frequencies.length} calibration points from file`);
    return { frequencies, magnitudes };
  }

  private clearCalibrationFile(): void {
    console.log("Clearing calibration file");

    // Clear file input
    if (this.captureCalibrationFile) {
      this.captureCalibrationFile.value = "";
    }

    // Clear calibration data in graph renderer
    if (this.captureGraphRenderer) {
      this.captureGraphRenderer.clearCalibrationData();

      // Re-render current data if available
      if (this.currentCaptureData) {
        this.captureGraphRenderer.renderGraph({
          frequencies: this.currentCaptureData.frequencies,
          rawMagnitudes: this.currentCaptureData.rawMagnitudes,
          smoothedMagnitudes: this.currentCaptureData.smoothedMagnitudes,
          rawPhase: this.currentCaptureData.rawPhase,
          smoothedPhase: this.currentCaptureData.smoothedPhase,
          channelData: this.currentCaptureData.channelData,
          outputChannel: this.currentCaptureData.outputChannel,
        });
      }
    }

    // Update button states
    if (this.captureCalibrationClear) {
      this.captureCalibrationClear.style.display = "none";
    }
    if (this.captureCalibrationBtn) {
      this.captureCalibrationBtn.textContent = "📁 Load File";
    }
  }

  private onChannelDisplayChange(): void {
    if (!this.captureChannelSelect || !this.captureGraphRenderer) {
      console.warn("Channel select or graph renderer not available");
      return;
    }

    const selectedDisplay = this.captureChannelSelect.value;
    console.log(`Channel display changed to: ${selectedDisplay}`);

    // Handle special aggregate options
    if (selectedDisplay === "__sum_all__") {
      this.displaySumOfAllSweeps();
      return;
    } else if (selectedDisplay === "__average_all__") {
      this.displayAverageOfAllSweeps();
      return;
    }

    // Handle individual sweep selection
    if (selectedDisplay.startsWith("sweep_")) {
      const captureId = selectedDisplay.substring(6); // Remove 'sweep_' prefix
      this.displaySavedSweep(captureId);
      return;
    }

    // Handle current capture display modes
    if (!this.currentCaptureData) {
      console.warn("No current capture data available");
      this.captureGraphRenderer.renderPlaceholder();
      return;
    }

    // Handle channel visibility based on selection
    if (selectedDisplay === "current") {
      this.captureGraphRenderer.setChannelVisibility("combined", true);
      this.captureGraphRenderer.setChannelVisibility("left", false);
      this.captureGraphRenderer.setChannelVisibility("right", false);
      this.captureGraphRenderer.setChannelVisibility("average", false);
    } else if (selectedDisplay === "average") {
      this.captureGraphRenderer.setChannelVisibility("combined", false);
      this.captureGraphRenderer.setChannelVisibility("left", false);
      this.captureGraphRenderer.setChannelVisibility("right", false);
      this.captureGraphRenderer.setChannelVisibility("average", true);
    } else if (selectedDisplay === "left") {
      this.captureGraphRenderer.setChannelVisibility("combined", false);
      this.captureGraphRenderer.setChannelVisibility("left", true);
      this.captureGraphRenderer.setChannelVisibility("right", false);
      this.captureGraphRenderer.setChannelVisibility("average", false);
    } else if (selectedDisplay === "right") {
      this.captureGraphRenderer.setChannelVisibility("combined", false);
      this.captureGraphRenderer.setChannelVisibility("left", false);
      this.captureGraphRenderer.setChannelVisibility("right", true);
      this.captureGraphRenderer.setChannelVisibility("average", false);
    } else if (selectedDisplay === "all") {
      this.captureGraphRenderer.setChannelVisibility("combined", true);
      this.captureGraphRenderer.setChannelVisibility("left", true);
      this.captureGraphRenderer.setChannelVisibility("right", true);
      this.captureGraphRenderer.setChannelVisibility("average", true);
    }

    // Re-render with updated visibility
    this.captureGraphRenderer.renderGraph({
      frequencies: this.currentCaptureData.frequencies,
      rawMagnitudes: this.currentCaptureData.rawMagnitudes,
      smoothedMagnitudes: this.currentCaptureData.smoothedMagnitudes,
      rawPhase: this.currentCaptureData.rawPhase,
      smoothedPhase: this.currentCaptureData.smoothedPhase,
      channelData: this.currentCaptureData.channelData,
      outputChannel: this.currentCaptureData.outputChannel,
    });
  }

  private updateChannelSelectOptions(outputChannel: string): void {
    if (!this.captureChannelSelect) return;

    // Store reference for use in callbacks
    const channelSelect = this.captureChannelSelect;

    // Clear existing options
    channelSelect.innerHTML = "";

    // Always add the current capture option if we have current data
    if (this.currentCaptureData) {
      const currentOption = document.createElement("option");
      currentOption.value = "current";
      currentOption.textContent = `Current${this.getChannelDisplayName(outputChannel)}`;
      channelSelect.appendChild(currentOption);
    }

    // Add "Average" option for current capture if it's stereo
    if (
      this.currentCaptureData &&
      this.currentCaptureData.channelData?.average
    ) {
      const avgOption = document.createElement("option");
      avgOption.value = "average";
      avgOption.textContent = "Average";
      channelSelect.appendChild(avgOption);
    }

    // Add separator before saved sweeps
    const allSavedCaptures = CaptureStorage.getAllCaptures();
    if (allSavedCaptures.length > 0) {
      const separator = document.createElement("option");
      separator.disabled = true;
      separator.textContent = "──────────";
      channelSelect.appendChild(separator);

      // Add sum/average of all sweeps options
      const sumOption = document.createElement("option");
      sumOption.value = "__sum_all__";
      sumOption.textContent = `Sum of All ${allSavedCaptures.length} Sweeps`;
      channelSelect.appendChild(sumOption);

      const avgAllOption = document.createElement("option");
      avgAllOption.value = "__average_all__";
      avgAllOption.textContent = `Average of All ${allSavedCaptures.length} Sweeps`;
      channelSelect.appendChild(avgAllOption);

      // Add separator before individual sweeps
      const separator2 = document.createElement("option");
      separator2.disabled = true;
      separator2.textContent = "──────────";
      channelSelect.appendChild(separator2);

      // Add each saved sweep
      allSavedCaptures.forEach((capture, index) => {
        const sweepOption = document.createElement("option");
        sweepOption.value = `sweep_${capture.id}`;
        sweepOption.textContent = `${index + 1}. ${capture.name}`;
        channelSelect.appendChild(sweepOption);
      });
    }

    // Set default selection
    channelSelect.value = "current";
    console.log(
      `Updated channel options for output: ${outputChannel}, ${allSavedCaptures.length} saved sweeps`,
    );
  }

  private getChannelDisplayName(outputChannel: string): string {
    switch (outputChannel) {
      case "left":
        return " (Left)";
      case "right":
        return " (Right)";
      case "both":
        return " (Stereo)";
      case "default":
        return "";
      default:
        return ` (${outputChannel.charAt(0).toUpperCase() + outputChannel.slice(1)})`;
    }
  }

  private async updateSampleRateForDevice(): Promise<void> {
    if (!this.modalCaptureSampleRate || !this.modalCaptureDevice) return;

    try {
      // Get the audio context to check the current sample rate
      const audioContext = new (window.AudioContext ||
        (window as typeof window & { webkitAudioContext?: typeof AudioContext })
          .webkitAudioContext ||
        AudioContext)();
      const deviceSampleRate = audioContext.sampleRate;
      audioContext.close(); // Clean up

      console.log(`Input device sample rate: ${deviceSampleRate} Hz`);

      // Format sample rate for badge display
      let sampleRateText = "";
      if (deviceSampleRate >= 1000) {
        const khz = deviceSampleRate / 1000;
        sampleRateText = khz % 1 === 0 ? `${khz}kHz` : `${khz.toFixed(1)}kHz`;
      } else {
        sampleRateText = `${deviceSampleRate}Hz`;
      }

      // Update the sample rate badge
      this.modalCaptureSampleRate.textContent = sampleRateText;

      // Update bit depth badge
      if (this.modalCaptureBitDepth) {
        this.modalCaptureBitDepth.textContent = "24";
      }
    } catch (error) {
      console.warn("Could not determine input device sample rate:", error);
      // Fall back to a common default
      if (this.modalCaptureSampleRate) {
        this.modalCaptureSampleRate.textContent = "48kHz";
      }
      if (this.modalCaptureBitDepth) {
        this.modalCaptureBitDepth.textContent = "24";
      }
    }
  }

  private async updateOutputChannelOptions(): Promise<void> {
    if (!this.modalOutputChannel || !this.modalOutputDevice) return;

    try {
      const deviceId = this.modalOutputDevice.value;

      // Get device details
      let deviceInfo: {
        outputChannels: number | null;
        deviceLabel: string;
      } | null = null;

      if (deviceId === "default") {
        // For default device, assume stereo
        deviceInfo = {
          outputChannels: 2,
          deviceLabel: "Default Output",
        };
      } else {
        // Get specific device details from our device list
        if (this.captureController) {
          try {
            const devices = await this.captureController.getAudioDevices();
            const outputDevice = devices.output.find(
              (d) => d.value === deviceId,
            );
            if (outputDevice && outputDevice.info) {
              // Parse channel count from info string (e.g. "2ch 48kHz")
              const channelMatch = outputDevice.info.match(/(\d+)ch/);
              const channels = channelMatch ? parseInt(channelMatch[1]) : null;
              deviceInfo = {
                outputChannels: channels,
                deviceLabel: outputDevice.label,
              };
            }
          } catch (e) {
            console.warn(
              "[CaptureModal] Could not get device details for output device:",
              deviceId,
              e,
            );
          }
        }
      }

      if (!deviceInfo) {
        if (this.deviceStatusCallback) {
          this.deviceStatusCallback("error");
        }
        return;
      }

      // Update device status to success
      if (this.deviceStatusCallback) {
        this.deviceStatusCallback("success");
      }

      // Update output channel dropdown based on channel count
      if (
        deviceInfo.outputChannels !== null &&
        deviceInfo.outputChannels !== undefined
      ) {
        this.populateOutputChannels(deviceInfo.outputChannels);
      } else {
        console.warn("[CaptureModal] Unknown output channel count for device");
      }
    } catch (error) {
      console.error("Error updating output channel options:", error);
      if (this.deviceStatusCallback) {
        this.deviceStatusCallback("error");
      }
    }
  }

  private populateOutputChannels(channelCount: number | null): void {
    if (channelCount === null || channelCount === undefined) {
      console.warn(
        "[CaptureModal] Cannot populate output channels: count is unknown",
      );
      return;
    }
    if (!this.modalOutputChannel) return;

    // Save current selection
    const currentValue = this.modalOutputChannel.value;

    // Clear existing options
    this.modalOutputChannel.innerHTML = "";

    // Add default combined option
    const defaultOption = document.createElement("option");
    defaultOption.value = "default";
    defaultOption.textContent = "System Default";
    this.modalOutputChannel.appendChild(defaultOption);

    if (channelCount === 1) {
      // Mono device
      const monoOption = document.createElement("option");
      monoOption.value = "both";
      monoOption.textContent = "Mono";
      this.modalOutputChannel.appendChild(monoOption);
    } else if (channelCount === 2) {
      // Stereo device - Left and Right options
      const leftOption = document.createElement("option");
      leftOption.value = "left";
      leftOption.textContent = "Left";
      this.modalOutputChannel.appendChild(leftOption);

      const rightOption = document.createElement("option");
      rightOption.value = "right";
      rightOption.textContent = "Right";
      this.modalOutputChannel.appendChild(rightOption);
    } else {
      // Multi-channel device
      const allOption = document.createElement("option");
      allOption.value = "all";
      allOption.textContent = `All Channels (${channelCount})`;
      this.modalOutputChannel.appendChild(allOption);

      for (let i = 1; i <= channelCount; i++) {
        const option = document.createElement("option");
        option.value = `ch${i}`;
        option.textContent = `Channel ${i}`;
        this.modalOutputChannel.appendChild(option);
      }
    }

    // Try to restore previous selection or set default
    if (
      currentValue &&
      this.modalOutputChannel.querySelector(`option[value="${currentValue}"]`)
    ) {
      this.modalOutputChannel.value = currentValue;
    } else {
      this.modalOutputChannel.value = channelCount === 1 ? "both" : "default";
    }

    // Update channel display options
    const outputChannel = this.modalOutputChannel.value || "both";
    this.updateChannelSelectOptions(outputChannel);

    console.log(`Populated output channels for ${channelCount}-channel device`);
  }

  private async updateOutputDeviceInfo(): Promise<void> {
    // Update the channel info badge
    if (this.outputChannelsInfo && this.modalOutputDevice) {
      const selectedDevice = this.modalOutputDevice.value;
      if (!selectedDevice || selectedDevice === "default") {
        // For default device, assume stereo (2 channels)
        this.outputChannelsInfo.textContent = "2";
        this.deviceChannelInfo.output = 2;
      } else {
        // Extract channel info from the selected option's text
        const selectedOption =
          this.modalOutputDevice.options[this.modalOutputDevice.selectedIndex];
        const deviceText = selectedOption.textContent || "";

        // Parse channel info from text like "Device Name (2ch 44kHz)"
        const channelMatch = deviceText.match(/(\d+)ch/);
        if (channelMatch) {
          const channels = parseInt(channelMatch[1]);
          this.outputChannelsInfo.textContent = channelMatch[1];
          this.deviceChannelInfo.output = channels;

          // Update routing matrix if it exists
          if (this.outputRoutingMatrix) {
            this.outputRoutingMatrix.updateChannelCount(channels);
          }
        } else {
          this.outputChannelsInfo.textContent = "??";
          this.deviceChannelInfo.output = null;
        }
      }
    }

    // Also update the sample rate and bit depth badges if needed
    if (this.modalOutputSampleRate) {
      // Extract sample rate from device text if available
      const selectedOption =
        this.modalOutputDevice?.options[this.modalOutputDevice.selectedIndex];
      const deviceText = selectedOption?.textContent || "";
      const rateMatch = deviceText.match(/(\d+)kHz/);
      if (rateMatch) {
        this.modalOutputSampleRate.textContent = rateMatch[0];
      }
    }

    if (this.modalOutputBitDepth) {
      this.modalOutputBitDepth.textContent = "24";
    }
  }

  private async updateOutputSampleRate(): Promise<void> {
    if (!this.modalOutputSampleRate) return;

    try {
      // Get the audio context to check the current sample rate
      const audioContext = new (window.AudioContext ||
        (window as typeof window & { webkitAudioContext?: typeof AudioContext })
          .webkitAudioContext ||
        AudioContext)();
      const deviceSampleRate = audioContext.sampleRate;
      audioContext.close(); // Clean up

      console.log(`Output device sample rate: ${deviceSampleRate} Hz`);

      // Format sample rate for badge display
      let sampleRateText = "";
      if (deviceSampleRate >= 1000) {
        const khz = deviceSampleRate / 1000;
        sampleRateText = khz % 1 === 0 ? `${khz}kHz` : `${khz.toFixed(1)}kHz`;
      } else {
        sampleRateText = `${deviceSampleRate}Hz`;
      }

      // Update the sample rate badge
      this.modalOutputSampleRate.textContent = sampleRateText;

      // Update bit depth badge
      if (this.modalOutputBitDepth) {
        this.modalOutputBitDepth.textContent = "24";
      }
    } catch (error) {
      console.warn("Could not determine output device sample rate:", error);
      // Fall back to a common default
      if (this.modalOutputSampleRate) {
        this.modalOutputSampleRate.textContent = "48kHz";
      }
      if (this.modalOutputBitDepth) {
        this.modalOutputBitDepth.textContent = "24";
      }
    }
  }

  // Records Management Methods

  private async renderRecordsList(): Promise<void> {
    if (!this.recordsList) return;

    try {
      const captures = CaptureStorage.getAllCaptures();

      if (captures.length === 0) {
        this.recordsList.innerHTML =
          '<div class="no-records">No saved records</div>';
        return;
      }

      // Generate color palette for records
      const colors = [
        "#007bff",
        "#28a745",
        "#dc3545",
        "#ffc107",
        "#17a2b8",
        "#6610f2",
        "#e83e8c",
        "#fd7e14",
        "#20c997",
        "#6f42c1",
      ];

      this.recordsList.innerHTML = "";
      const listElement = this.recordsList;

      captures.forEach((capture, index) => {
        const recordItem = document.createElement("div");
        recordItem.className = "record-item";
        recordItem.dataset.captureId = capture.id;

        if (this.selectedRecordIds.has(capture.id)) {
          recordItem.classList.add("selected");
        }

        const color = colors[index % colors.length];

        recordItem.innerHTML = `
          <div class="record-color-indicator" style="background: ${color};"></div>
          <input type="checkbox" class="record-checkbox" ${this.selectedRecordIds.has(capture.id) ? "checked" : ""}>
          <div class="record-info">
            <div class="record-name" contenteditable="false">${capture.name}</div>
            <div class="record-meta">${this.formatRecordMeta(capture)}</div>
          </div>
          <div class="record-actions">
            <button class="record-action-btn load" title="Load">📂</button>
            <button class="record-action-btn rename" title="Rename">✏️</button>
            <button class="record-action-btn delete" title="Delete">🗑️</button>
          </div>
        `;

        // Add event listeners
        const checkbox = recordItem.querySelector(
          ".record-checkbox",
        ) as HTMLInputElement;
        checkbox?.addEventListener("change", (e) => {
          e.stopPropagation();
          this.toggleRecordSelection(capture.id, checkbox.checked);
        });

        const loadBtn = recordItem.querySelector(".load") as HTMLButtonElement;
        loadBtn?.addEventListener("click", (e) => {
          e.stopPropagation();
          this.loadCapture(capture.id);
        });

        const renameBtn = recordItem.querySelector(
          ".rename",
        ) as HTMLButtonElement;
        renameBtn?.addEventListener("click", (e) => {
          e.stopPropagation();
          this.renameRecord(capture.id, recordItem);
        });

        const deleteBtn = recordItem.querySelector(
          ".delete",
        ) as HTMLButtonElement;
        deleteBtn?.addEventListener("click", (e) => {
          e.stopPropagation();
          this.deleteRecord(capture.id);
        });

        // Click on item to toggle selection
        recordItem.addEventListener("click", () => {
          checkbox.checked = !checkbox.checked;
          this.toggleRecordSelection(capture.id, checkbox.checked);
        });

        listElement.appendChild(recordItem);
      });
    } catch (error) {
      console.error("Error rendering records list:", error);
    }
  }

  private formatRecordMeta(capture: {
    outputChannel: string;
    timestamp: string | Date;
  }): string {
    const channel =
      capture.outputChannel === "both"
        ? "Stereo"
        : capture.outputChannel === "left"
          ? "L"
          : capture.outputChannel === "right"
            ? "R"
            : "Mono";
    const date = new Date(capture.timestamp).toLocaleDateString();
    const time = new Date(capture.timestamp).toLocaleTimeString([], {
      hour: "2-digit",
      minute: "2-digit",
    });
    return `${channel} • ${date} ${time}`;
  }

  private toggleRecordsSidebar(): void {
    if (!this.recordsSidebar || !this.recordsToggleBtn) return;

    const isCollapsed = this.recordsSidebar.classList.toggle("collapsed");
    this.recordsToggleBtn.textContent = isCollapsed ? "▶" : "◀";

    // Note: Graph will auto-resize on window resize event
    console.log(
      "Records sidebar toggled:",
      isCollapsed ? "collapsed" : "expanded",
    );
  }

  private toggleRecordSelection(captureId: string, selected: boolean): void {
    if (selected) {
      this.selectedRecordIds.add(captureId);
    } else {
      this.selectedRecordIds.delete(captureId);
    }

    // Update UI
    const recordItem = this.recordsList?.querySelector(
      `[data-capture-id="${captureId}"]`,
    );
    if (recordItem) {
      recordItem.classList.toggle("selected", selected);
    }

    console.log(`Record ${captureId} ${selected ? "selected" : "deselected"}`);
  }

  private async selectAllRecords(): Promise<void> {
    try {
      const captures = CaptureStorage.getAllCaptures();

      captures.forEach((capture) => {
        this.selectedRecordIds.add(capture.id);
      });

      await this.renderRecordsList();
      console.log("All records selected");
    } catch (error) {
      console.error("Error selecting all records:", error);
    }
  }

  private async deselectAllRecords(): Promise<void> {
    this.selectedRecordIds.clear();
    await this.renderRecordsList();
    console.log("All records deselected");
  }

  private async renameRecord(
    captureId: string,
    recordItem: HTMLElement,
  ): Promise<void> {
    const nameElement = recordItem.querySelector(".record-name") as HTMLElement;
    if (!nameElement) return;

    const currentName = nameElement.textContent || "";
    nameElement.contentEditable = "true";
    nameElement.classList.add("editing");
    nameElement.focus();

    // Select all text
    const range = document.createRange();
    range.selectNodeContents(nameElement);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);

    const finishEdit = async () => {
      nameElement.contentEditable = "false";
      nameElement.classList.remove("editing");

      const newName = nameElement.textContent?.trim() || currentName;

      if (newName !== currentName && newName.length > 0) {
        try {
          const capture = CaptureStorage.getCapture(captureId);

          if (capture) {
            // Update name in storage
            capture.name = newName;
            console.log(`Renamed capture ${captureId} to: ${newName}`);
            await this.renderRecordsList();
            await this.updateChannelSelectOptions(
              this.currentCaptureData?.outputChannel || "both",
            );
          }
        } catch (error) {
          console.error("Error renaming capture:", error);
          nameElement.textContent = currentName;
        }
      } else {
        nameElement.textContent = currentName;
      }
    };

    nameElement.addEventListener("blur", finishEdit, { once: true });
    nameElement.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        nameElement.blur();
      } else if (e.key === "Escape") {
        nameElement.textContent = currentName;
        nameElement.blur();
      }
    });
  }

  private async deleteRecord(captureId: string): Promise<void> {
    if (!confirm("Delete this capture?")) return;

    try {
      CaptureStorage.deleteCapture(captureId);
      this.selectedRecordIds.delete(captureId);

      await this.renderRecordsList();
      await this.updateChannelSelectOptions(
        this.currentCaptureData?.outputChannel || "both",
      );

      console.log(`Deleted capture: ${captureId}`);
    } catch (error) {
      console.error("Error deleting capture:", error);
    }
  }

  private async deleteSelectedRecords(): Promise<void> {
    if (this.selectedRecordIds.size === 0) {
      alert("No records selected");
      return;
    }

    if (!confirm(`Delete ${this.selectedRecordIds.size} selected record(s)?`))
      return;

    try {
      for (const captureId of this.selectedRecordIds) {
        CaptureStorage.deleteCapture(captureId);
      }

      this.selectedRecordIds.clear();
      await this.renderRecordsList();
      await this.updateChannelSelectOptions(
        this.currentCaptureData?.outputChannel || "both",
      );

      console.log("Deleted selected captures");
    } catch (error) {
      console.error("Error deleting selected captures:", error);
    }
  }

  private async loadCapture(captureId: string): Promise<void> {
    try {
      const capture = CaptureStorage.getCapture(captureId);
      if (!capture) {
        console.error(`Capture ${captureId} not found`);
        return;
      }

      console.log(`Loading capture: ${capture.name}`);

      // Set the current capture data
      this.currentCaptureData = {
        frequencies: capture.frequencies,
        rawMagnitudes: capture.rawMagnitudes,
        smoothedMagnitudes: capture.smoothedMagnitudes,
        rawPhase: capture.rawPhase,
        smoothedPhase: capture.smoothedPhase,
        metadata: {
          timestamp: capture.timestamp,
          deviceName: capture.deviceName,
          signalType: capture.signalType,
          duration: capture.duration,
          sampleRate: capture.sampleRate,
          outputChannel: capture.outputChannel as
            | "left"
            | "right"
            | "both"
            | "default",
        },
        outputChannel: capture.outputChannel as
          | "left"
          | "right"
          | "both"
          | "default",
      };

      // Update the graph
      if (this.captureGraphRenderer) {
        this.captureGraphRenderer.renderGraph({
          frequencies: capture.frequencies,
          rawMagnitudes: capture.rawMagnitudes,
          smoothedMagnitudes: capture.smoothedMagnitudes,
          rawPhase: capture.rawPhase,
          smoothedPhase: capture.smoothedPhase,
        });
      }

      // Show export button
      if (this.captureModalExport) {
        this.captureModalExport.style.display = "inline-flex";
      }

      // Hide placeholder if visible
      if (this.captureModalPlaceholder) {
        this.captureModalPlaceholder.style.display = "none";
      }

      // Update status
      if (this.captureModalStatus) {
        this.captureModalStatus.textContent = `Loaded: ${capture.name}`;
      }

      // Call completion callback if set (for loading into optimization)
      if (this.onCaptureComplete) {
        this.onCaptureComplete(capture.frequencies, capture.smoothedMagnitudes);
      }

      console.log(`Successfully loaded capture: ${capture.name}`);
    } catch (error) {
      console.error("Error loading capture:", error);
      alert("Failed to load the selected capture");
    }
  }

  private showRoutingMatrix(type: "input" | "output"): void {
    console.log(`Showing ${type} routing matrix`);

    const channelCount =
      type === "input"
        ? this.deviceChannelInfo.input
        : this.deviceChannelInfo.output;

    if (
      channelCount === null ||
      channelCount === undefined ||
      channelCount <= 0
    ) {
      alert(
        `Cannot show routing matrix: ${type === "input" ? "Input" : "Output"} device channel count is unknown.\n\nPlease ensure a device is selected and detected properly.`,
      );
      return;
    }

    // Get or create the appropriate routing matrix
    if (type === "input") {
      if (!this.inputRoutingMatrix) {
        this.inputRoutingMatrix = new RoutingMatrix(channelCount);
        this.inputRoutingMatrix.setOnRoutingChange((routing) => {
          console.log("Input routing changed:", routing);
          // Store routing configuration for use during capture
        });
      } else {
        // Update channel count if it changed
        this.inputRoutingMatrix.updateChannelCount(channelCount);
      }

      // Show the routing matrix UI
      if (this.inputRoutingBtn) {
        this.inputRoutingMatrix.show(this.inputRoutingBtn);
      }
    } else {
      if (!this.outputRoutingMatrix) {
        this.outputRoutingMatrix = new RoutingMatrix(channelCount);
        this.outputRoutingMatrix.setOnRoutingChange((routing) => {
          console.log("Output routing changed:", routing);
          // Store routing configuration for use during playback
        });
      } else {
        // Update channel count if it changed
        this.outputRoutingMatrix.updateChannelCount(channelCount);
      }

      // Show the routing matrix UI
      if (this.outputRoutingBtn) {
        this.outputRoutingMatrix.show(this.outputRoutingBtn);
      }
    }
  }

  /**
   * Display a saved sweep by ID
   */
  private displaySavedSweep(captureId: string): void {
    const capture = CaptureStorage.getCapture(captureId);
    if (!capture) {
      console.error(`Capture ${captureId} not found`);
      if (this.captureModalStatus) {
        this.captureModalStatus.textContent = `⚠️ Sweep not found`;
      }
      return;
    }

    console.log(`Displaying saved sweep: ${capture.name}`);

    if (this.captureGraphRenderer) {
      this.captureGraphRenderer.renderGraph({
        frequencies: capture.frequencies,
        rawMagnitudes: capture.rawMagnitudes,
        smoothedMagnitudes: capture.smoothedMagnitudes,
        rawPhase: capture.rawPhase,
        smoothedPhase: capture.smoothedPhase,
      });
    }

    // Update status
    if (this.captureModalStatus) {
      this.captureModalStatus.textContent = `Displaying: ${capture.name}`;
    }
  }

  /**
   * Calculate and display the sum of all saved sweeps
   */
  private displaySumOfAllSweeps(): void {
    const allCaptures = CaptureStorage.getAllCaptures();
    if (allCaptures.length === 0) {
      console.warn("No saved captures to sum");
      return;
    }

    console.log(`Calculating sum of ${allCaptures.length} sweeps`);

    // Use the first capture as reference for frequencies
    const referenceFreqs = allCaptures[0].frequencies;
    const sumMagnitudes = new Array(referenceFreqs.length).fill(0);

    // Sum all magnitudes (in dB, this adds the amplitudes)
    allCaptures.forEach((capture) => {
      if (capture.frequencies.length === referenceFreqs.length) {
        for (let i = 0; i < referenceFreqs.length; i++) {
          // Convert dB to linear, add, then convert back
          // dB = 20*log10(amplitude), so amplitude = 10^(dB/20)
          const linear = Math.pow(10, capture.smoothedMagnitudes[i] / 20);
          const currentLinear = Math.pow(10, sumMagnitudes[i] / 20);
          const newLinear = currentLinear + linear;
          sumMagnitudes[i] = 20 * Math.log10(newLinear);
        }
      }
    });

    if (this.captureGraphRenderer) {
      this.captureGraphRenderer.renderGraph({
        frequencies: referenceFreqs,
        rawMagnitudes: sumMagnitudes,
        smoothedMagnitudes: sumMagnitudes,
        rawPhase: [],
        smoothedPhase: [],
      });
    }

    // Update status
    if (this.captureModalStatus) {
      this.captureModalStatus.textContent = `Sum of ${allCaptures.length} sweeps`;
    }
  }

  /**
   * Calculate and display the average of all saved sweeps
   */
  private displayAverageOfAllSweeps(): void {
    const allCaptures = CaptureStorage.getAllCaptures();
    if (allCaptures.length === 0) {
      console.warn("No saved captures to average");
      return;
    }

    console.log(`Calculating average of ${allCaptures.length} sweeps`);

    // Use the first capture as reference for frequencies
    const referenceFreqs = allCaptures[0].frequencies;
    const avgMagnitudes = new Array(referenceFreqs.length).fill(0);

    // Average all magnitudes (in dB)
    allCaptures.forEach((capture) => {
      if (capture.frequencies.length === referenceFreqs.length) {
        for (let i = 0; i < referenceFreqs.length; i++) {
          avgMagnitudes[i] += capture.smoothedMagnitudes[i];
        }
      }
    });

    // Divide by count to get average
    for (let i = 0; i < avgMagnitudes.length; i++) {
      avgMagnitudes[i] /= allCaptures.length;
    }

    if (this.captureGraphRenderer) {
      this.captureGraphRenderer.renderGraph({
        frequencies: referenceFreqs,
        rawMagnitudes: avgMagnitudes,
        smoothedMagnitudes: avgMagnitudes,
        rawPhase: [],
        smoothedPhase: [],
      });
    }

    // Update status
    if (this.captureModalStatus) {
      this.captureModalStatus.textContent = `Average of ${allCaptures.length} sweeps`;
    }
  }

  // ============================================================================
  // SPL Monitoring
  // ============================================================================

  /**
   * Start polling for SPL during recording
   */
  public startSPLMonitoring(): void {
    // Stop any existing polling
    this.stopSPLMonitoring();

    // Show the SPL badge
    if (this.modalCaptureSPL) {
      this.modalCaptureSPL.style.display = "inline-block";
      this.modalCaptureSPL.textContent = "-- dB";
    }

    // Start polling every 100ms
    this.splPollingInterval = window.setInterval(async () => {
      try {
        const spl = await invoke<number>("audio_get_recording_spl");

        if (this.modalCaptureSPL) {
          // Format SPL value with color coding
          let displayText: string;
          let colorClass: string;

          if (spl <= -90) {
            displayText = "-- dB";
            colorClass = "";
          } else {
            displayText = `${spl.toFixed(1)} dB`;

            // Color code based on SPL level
            if (spl < 40) {
              colorClass = "spl-too-low"; // Red - too quiet
            } else if (spl < 60) {
              colorClass = "spl-low"; // Yellow - low
            } else if (spl < 90) {
              colorClass = "spl-good"; // Green - good range
            } else if (spl < 100) {
              colorClass = "spl-high"; // Yellow - getting loud
            } else {
              colorClass = "spl-too-high"; // Red - too loud
            }
          }

          this.modalCaptureSPL.textContent = displayText;

          // Remove all color classes
          this.modalCaptureSPL.classList.remove(
            "spl-too-low",
            "spl-low",
            "spl-good",
            "spl-high",
            "spl-too-high",
          );

          // Add appropriate color class
          if (colorClass) {
            this.modalCaptureSPL.classList.add(colorClass);
          }
        }
      } catch (error) {
        // Silent fail - recording may have stopped
        console.debug(
          "SPL monitoring error (expected if not recording):",
          error,
        );
      }
    }, 100);

    console.log("SPL monitoring started");
  }

  /**
   * Stop polling for SPL
   */
  public stopSPLMonitoring(): void {
    if (this.splPollingInterval !== null) {
      clearInterval(this.splPollingInterval);
      this.splPollingInterval = null;
      console.log("SPL monitoring stopped");
    }

    // Hide the SPL badge
    if (this.modalCaptureSPL) {
      this.modalCaptureSPL.style.display = "none";
      this.modalCaptureSPL.textContent = "-- dB";
      this.modalCaptureSPL.classList.remove(
        "spl-too-low",
        "spl-low",
        "spl-good",
        "spl-high",
        "spl-too-high",
      );
    }
  }
}
