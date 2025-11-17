// Capture Recording Panel (Step 2)
// Run test signals per channel, display state, and plot results

console.log("[MODULE] capture-recording-panel.ts loading...");

import {
  recordChannel,
  recordAllChannels,
  saveRecordings,
  loadRecordings,
  type RecordingResult,
} from "./capture-tauri";
import type { CaptureConfig } from "./capture-config-panel";
import { plotFrequencyResponse, updatePlot, clearPlot } from "./capture-plot";
import { save, open } from "@tauri-apps/plugin-dialog";

console.log("[MODULE] capture-recording-panel.ts imports complete");

export type SignalType = "sweep" | "white" | "pink";

export interface SignalProperties {
  type: SignalType;
  duration?: number; // in seconds, for sweep
  amplitude?: number; // 0-1
}

export type RecordingState = "empty" | "recording" | "done" | "error";

export interface ChannelRecording {
  channelIndex: number;
  channelName: string;
  state: RecordingState;
  data?: Float32Array[];
  timestamp?: number;
}

export class CaptureRecordingPanel extends HTMLElement {
  private hasRendered = false;
  private recordings: Map<number, ChannelRecording> = new Map();
  private recordingResults: Map<number, RecordingResult> = new Map();
  private signalType: SignalType = "sweep";
  private signalProperties: SignalProperties = {
    type: "sweep",
    duration: 10,
    amplitude: 0.8,
  };
  private channelCount = 0;
  private channelNames: string[] = [];
  private captureConfig: CaptureConfig | null = null;
  private isRecording = false;

  constructor() {
    super();
  }

  connectedCallback() {
    console.log("[CaptureRecordingPanel] connectedCallback called");
    if (!this.hasRendered) {
      console.log("[CaptureRecordingPanel] Rendering for first time");
      this.render();
      this.hasRendered = true;
      this.attachEventListeners();
      console.log("[CaptureRecordingPanel] Initialization complete");
    }
  }

  private render(): void {
    this.innerHTML = `
      <div class="capture-recording-panel">
        <div class="capture-recording-header">
          <h3>Signal Recording</h3>
          <p class="capture-recording-description">
            Test each channel individually. Signals will play sequentially with a 1-second pause between channels.
          </p>
        </div>

        <!-- Signal Selection -->
        <div class="capture-signal-section">
          <div class="capture-signal-controls">
            <div class="capture-signal-row">
              <label for="signal_type_select">Signal Type:</label>
              <select id="signal_type_select" class="capture-signal-select">
                <option value="sweep" selected>Frequency Sweep</option>
                <option value="white">White Noise</option>
                <option value="pink">Pink Noise</option>
              </select>

              <div id="signal_properties" class="capture-signal-properties">
                <!-- Signal-specific properties will be shown here -->
                <label for="sweep_duration">Duration:</label>
                <select id="sweep_duration" class="capture-duration-select">
                  <option value="5">5 seconds</option>
                  <option value="10" selected>10 seconds</option>
                  <option value="15">15 seconds</option>
                  <option value="20">20 seconds</option>
                </select>
              </div>
            </div>
          </div>
        </div>

        <!-- Channel Recording Status -->
        <div class="capture-channels-section">
          <div class="capture-section-header">
            <h4>CHANNEL STATUS</h4>
            <div class="capture-recording-actions">
              <button type="button" id="record_all_btn" class="btn btn-primary">
                Record All Channels
              </button>
              <button type="button" id="stop_recording_btn" class="btn btn-danger" style="display: none">
                Stop Recording
              </button>
            </div>
          </div>

          <div id="channels_list" class="capture-channels-list">
            <!-- Channel rows will be dynamically generated -->
            <div class="capture-no-channels">
              <p>No channels configured. Please go back and configure your devices.</p>
            </div>
          </div>
        </div>

        <!-- Frequency Response Plot -->
        <div class="capture-plot-section">
          <div class="capture-section-header">
            <h4>FREQUENCY RESPONSE (20 Hz - 20 kHz)</h4>
          </div>

          <div class="capture-plot-container">
            <div id="recording_plot_container" class="capture-plot-chart"></div>
            <div id="recording_plot_placeholder" class="capture-plot-placeholder">
              <div class="capture-placeholder-content">
                <div class="capture-placeholder-icon">📊</div>
                <h4>Frequency Response Plot</h4>
                <p>Start recording to see frequency and phase response for all channels</p>
              </div>
            </div>
          </div>
        </div>

        <!-- Action Buttons -->
        <div class="capture-recording-actions-bottom">
          <div class="capture-recording-actions-left">
            <button type="button" id="recording_prev_btn" class="btn btn-secondary">
              ← Previous
            </button>
            <div class="capture-recording-actions-spacer"></div>
            <button type="button" id="recording_load_btn" class="btn btn-outline">
              Load
            </button>
            <button type="button" id="recording_save_btn" class="btn btn-outline">
              Save
            </button>
            <button type="button" id="recording_redo_btn" class="btn btn-outline">
              Redo
            </button>
          </div>
          <div class="capture-recording-actions-right">
            <button type="button" id="recording_next_btn" class="btn btn-primary">
              Next →
            </button>
          </div>
        </div>
      </div>
    `;
  }

  private attachEventListeners(): void {
    // Signal type change
    const signalTypeSelect = this.querySelector(
      "#signal_type_select",
    ) as HTMLSelectElement;
    signalTypeSelect?.addEventListener("change", () => {
      this.signalType = signalTypeSelect.value as SignalType;
      this.updateSignalProperties();
    });

    // Sweep duration change
    const sweepDuration = this.querySelector(
      "#sweep_duration",
    ) as HTMLSelectElement;
    sweepDuration?.addEventListener("change", () => {
      this.signalProperties.duration = parseInt(sweepDuration.value);
    });

    // Record all button
    const recordAllBtn = this.querySelector("#record_all_btn");
    recordAllBtn?.addEventListener("click", () => {
      this.startRecordingAllChannels();
    });

    // Stop recording button
    const stopRecordingBtn = this.querySelector("#stop_recording_btn");
    stopRecordingBtn?.addEventListener("click", () => {
      this.stopRecording();
    });

    // Navigation buttons
    const prevBtn = this.querySelector("#recording_prev_btn");
    prevBtn?.addEventListener("click", () => {
      this.dispatchEvent(
        new CustomEvent("captureNavigatePrevious", {
          bubbles: true,
          composed: true,
        }),
      );
    });

    // Load/Save/Redo buttons
    const loadBtn = this.querySelector("#recording_load_btn");
    loadBtn?.addEventListener("click", () => {
      this.loadRecordings();
    });

    const saveBtn = this.querySelector("#recording_save_btn");
    saveBtn?.addEventListener("click", () => {
      this.saveRecordings();
    });

    const redoBtn = this.querySelector("#recording_redo_btn");
    redoBtn?.addEventListener("click", () => {
      this.redoRecordings();
    });

    // Next button
    const nextBtn = this.querySelector("#recording_next_btn");
    nextBtn?.addEventListener("click", () => {
      this.onNext();
    });
  }

  private updateSignalProperties(): void {
    const propertiesContainer = this.querySelector("#signal_properties");
    if (!propertiesContainer) return;

    switch (this.signalType) {
      case "sweep":
        propertiesContainer.innerHTML = `
          <label for="sweep_duration">Duration:</label>
          <select id="sweep_duration" class="capture-duration-select">
            <option value="5">5 seconds</option>
            <option value="10" selected>10 seconds</option>
            <option value="15">15 seconds</option>
            <option value="20">20 seconds</option>
          </select>
        `;
        // Re-attach event listener
        const sweepDuration = this.querySelector(
          "#sweep_duration",
        ) as HTMLSelectElement;
        sweepDuration?.addEventListener("change", () => {
          this.signalProperties.duration = parseInt(sweepDuration.value);
        });
        break;

      case "white":
      case "pink":
        propertiesContainer.innerHTML = `
          <label for="noise_duration">Duration:</label>
          <select id="noise_duration" class="capture-duration-select">
            <option value="5">5 seconds</option>
            <option value="10" selected>10 seconds</option>
            <option value="30">30 seconds</option>
            <option value="60">60 seconds</option>
          </select>
        `;
        // Re-attach event listener
        const noiseDuration = this.querySelector(
          "#noise_duration",
        ) as HTMLSelectElement;
        noiseDuration?.addEventListener("change", () => {
          this.signalProperties.duration = parseInt(noiseDuration.value);
        });
        break;
    }
  }

  private updateChannelsList(): void {
    const container = this.querySelector("#channels_list");
    if (!container) return;

    if (this.channelCount === 0) {
      container.innerHTML = `
        <div class="capture-no-channels">
          <p>No channels configured. Please go back and configure your devices.</p>
        </div>
      `;
      return;
    }

    container.innerHTML = "";

    for (let i = 0; i < this.channelCount; i++) {
      const channelName = this.channelNames[i] || `Channel ${i + 1}`;
      const recording = this.recordings.get(i);
      const state = recording?.state || "empty";

      const row = document.createElement("div");
      row.className = "capture-channel-status-row";
      row.dataset.channel = i.toString();

      const stateClass = `state-${state}`;
      const stateIcon = this.getStateIcon(state);
      const stateText = this.getStateText(state);

      row.innerHTML = `
        <span class="capture-channel-name">${channelName}:</span>
        <div class="capture-channel-state ${stateClass}">
          <span class="state-icon">${stateIcon}</span>
          <span class="state-text">${stateText}</span>
        </div>
        <button
          type="button"
          class="btn btn-sm btn-outline capture-record-channel-btn"
          data-channel="${i}"
          ${state === "recording" ? "disabled" : ""}
        >
          ${state === "done" ? "Re-record" : "Record"}
        </button>
      `;

      // Attach individual record button listener
      const recordBtn = row.querySelector(".capture-record-channel-btn");
      recordBtn?.addEventListener("click", () => {
        this.startRecordingChannel(i);
      });

      container.appendChild(row);
    }
  }

  private getStateIcon(state: RecordingState): string {
    switch (state) {
      case "empty":
        return "○";
      case "recording":
        return "●";
      case "done":
        return "✓";
      case "error":
        return "✗";
      default:
        return "○";
    }
  }

  private getStateText(state: RecordingState): string {
    switch (state) {
      case "empty":
        return "Not recorded";
      case "recording":
        return "Recording...";
      case "done":
        return "Complete";
      case "error":
        return "Error";
      default:
        return "Unknown";
    }
  }

  private async startRecordingAllChannels(): Promise<void> {
    if (!this.captureConfig) {
      alert("Configuration not set. Please go back and configure devices.");
      return;
    }

    console.log("Starting recording for all channels");
    this.isRecording = true;

    const recordAllBtn = this.querySelector(
      "#record_all_btn",
    ) as HTMLButtonElement;
    const stopBtn = this.querySelector(
      "#stop_recording_btn",
    ) as HTMLButtonElement;

    if (recordAllBtn) recordAllBtn.style.display = "none";
    if (stopBtn) stopBtn.style.display = "inline-block";

    try {
      for (let i = 0; i < this.channelCount; i++) {
        if (!this.isRecording) break; // Check if stopped

        await this.startRecordingChannel(i);

        // Wait 1 second between channels
        if (i < this.channelCount - 1) {
          await new Promise((resolve) => setTimeout(resolve, 1000));
        }
      }
    } catch (error) {
      console.error("Recording error:", error);
      alert(`Recording failed: ${error}`);
    } finally {
      this.isRecording = false;
      if (recordAllBtn) recordAllBtn.style.display = "inline-block";
      if (stopBtn) stopBtn.style.display = "none";
    }
  }

  private async startRecordingChannel(channelIndex: number): Promise<void> {
    if (!this.captureConfig) {
      throw new Error("Configuration not set");
    }

    console.log(`Starting recording for channel ${channelIndex}`);

    // Set state to recording
    const recording: ChannelRecording = {
      channelIndex,
      channelName:
        this.channelNames[channelIndex] || `Channel ${channelIndex + 1}`,
      state: "recording",
      timestamp: Date.now(),
    };

    this.recordings.set(channelIndex, recording);
    this.updateChannelsList();

    try {
      // Use Tauri backend to record
      const outputPath = `/tmp/autoeq_capture_ch${channelIndex}`;
      const sampleRate = this.captureConfig.playback.sampleRate || 48000;

      const result = await recordChannel(
        this.captureConfig.playback.deviceName,
        this.captureConfig.recording.deviceName,
        channelIndex,
        channelIndex,
        this.signalType,
        this.signalProperties.duration || 10,
        sampleRate,
        outputPath,
        (progress) => {
          console.log(`Channel ${channelIndex} progress:`, progress);
        },
      );

      // Store result
      this.recordingResults.set(channelIndex, result);

      // Update state to done
      recording.state = "done";
      this.recordings.set(channelIndex, recording);
      this.updateChannelsList();
      this.updatePlot();
    } catch (error) {
      console.error(`Failed to record channel ${channelIndex}:`, error);
      recording.state = "error";
      this.recordings.set(channelIndex, recording);
      this.updateChannelsList();
      throw error;
    }
  }

  private stopRecording(): void {
    console.log("Stopping recording");
    this.isRecording = false;

    const recordAllBtn = this.querySelector(
      "#record_all_btn",
    ) as HTMLButtonElement;
    const stopBtn = this.querySelector(
      "#stop_recording_btn",
    ) as HTMLButtonElement;

    if (recordAllBtn) recordAllBtn.style.display = "inline-block";
    if (stopBtn) stopBtn.style.display = "none";
  }

  private updatePlot(): void {
    console.log("Updating plot with recorded data");

    const placeholder = this.querySelector(
      "#recording_plot_placeholder",
    ) as HTMLElement;
    const plotContainer = this.querySelector(
      "#recording_plot_container",
    ) as HTMLElement;

    // Get all completed recording results
    const results = Array.from(this.recordingResults.values());

    if (results.length === 0) {
      // No data to plot, show placeholder
      if (placeholder) placeholder.style.display = "flex";
      return;
    }

    // Hide placeholder
    if (placeholder) placeholder.style.display = "none";

    // Plot the frequency response
    if (plotContainer) {
      plotFrequencyResponse(plotContainer, results, {
        showPhase: true,
        minFreq: 20,
        maxFreq: 20000,
      });
    }
  }

  private async loadRecordings(): Promise<void> {
    try {
      // Open file dialog
      const filePath = await open({
        title: "Load Recordings",
        filters: [
          {
            name: "ZIP Archive",
            extensions: ["zip"],
          },
        ],
      });

      if (!filePath) return;

      // Load recordings from ZIP
      const results = await loadRecordings(filePath as string);

      // Clear existing recordings
      this.recordings.clear();
      this.recordingResults.clear();

      // Process loaded results
      results.forEach((result) => {
        // Store result
        this.recordingResults.set(result.channel, result);

        // Create recording entry
        const recording: ChannelRecording = {
          channelIndex: result.channel,
          channelName:
            this.channelNames[result.channel] ||
            `Channel ${result.channel + 1}`,
          state: "done",
          timestamp: Date.now(),
        };
        this.recordings.set(result.channel, recording);
      });

      // Update UI
      this.updateChannelsList();
      this.updatePlot();

      console.log(`Loaded ${results.length} recordings`);
    } catch (error) {
      console.error("Failed to load recordings:", error);
      alert(`Failed to load recordings: ${error}`);
    }
  }

  private async saveRecordings(): Promise<void> {
    try {
      if (this.recordingResults.size === 0) {
        alert("No recordings to save");
        return;
      }

      // Open save dialog
      const filePath = await save({
        title: "Save Recordings",
        filters: [
          {
            name: "ZIP Archive",
            extensions: ["zip"],
          },
        ],
        defaultPath: "capture-recordings.zip",
      });

      if (!filePath) return;

      // Convert recordings to array
      const results = Array.from(this.recordingResults.values());

      // Save to ZIP
      await saveRecordings(results, filePath);

      console.log(`Saved ${results.length} recordings to ${filePath}`);
    } catch (error) {
      console.error("Failed to save recordings:", error);
      alert(`Failed to save recordings: ${error}`);
    }
  }

  private redoRecordings(): void {
    console.log("Redo recordings");
    this.recordings.clear();
    this.recordingResults.clear();
    this.updateChannelsList();

    const placeholder = this.querySelector(
      "#recording_plot_placeholder",
    ) as HTMLElement;
    const plotContainer = this.querySelector(
      "#recording_plot_container",
    ) as HTMLElement;

    if (placeholder) placeholder.style.display = "flex";
    if (plotContainer) clearPlot(plotContainer);
  }

  private onNext(): void {
    // Check if all channels are recorded
    const allRecorded =
      this.channelCount > 0 &&
      Array.from(this.recordings.values()).every((r) => r.state === "done");

    if (!allRecorded) {
      alert("Please record all channels before proceeding.");
      return;
    }

    // Emit event to move to next step
    const event = new CustomEvent("captureRecordingComplete", {
      bubbles: true,
      composed: true,
      detail: { recordings: Array.from(this.recordings.values()) },
    });
    this.dispatchEvent(event);
  }

  public setChannels(count: number, names: string[]): void {
    this.channelCount = count;
    this.channelNames = names;
    this.recordings.clear();
    this.recordingResults.clear();
    this.updateChannelsList();
  }

  public setConfig(config: CaptureConfig): void {
    this.captureConfig = config;
  }

  public getRecordings(): ChannelRecording[] {
    return Array.from(this.recordings.values());
  }

  public getRecordingResults(): RecordingResult[] {
    return Array.from(this.recordingResults.values());
  }
}

// Register the custom element
console.log("[MODULE] Registering capture-recording-panel custom element...");
if (!customElements.get("capture-recording-panel")) {
  customElements.define("capture-recording-panel", CaptureRecordingPanel);
  console.log("[MODULE] capture-recording-panel registered successfully");
} else {
  console.log("[MODULE] capture-recording-panel already registered");
}
