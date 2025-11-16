// Capture Configuration Panel (Step 1)
// Device selection, channel routing, and microphone calibration

console.log("[MODULE] capture-config-panel.ts loading...");

import {
  getAudioDevices,
  getDeviceProperties,
  saveCaptureConfig,
  loadCaptureConfig,
  type AudioDevice,
  type AudioConfig,
} from "./capture-tauri";
import { save, open } from "@tauri-apps/plugin-dialog";
import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";

console.log("[MODULE] capture-config-panel.ts imports complete");

export interface ChannelGroup {
  id: string;
  name: string;
  channels: number[];
}

export interface DeviceConfig {
  deviceId: string;
  deviceName: string;
  channels: number;
  sampleRate: number;
  bitDepth: number;
}

export interface PlaybackConfig extends DeviceConfig {
  channelGroups: ChannelGroup[];
}

export interface RecordingConfig extends DeviceConfig {
  channelMapping: number[];
}

export interface CaptureConfig {
  playback: PlaybackConfig;
  recording: RecordingConfig;
  microphoneCalibration?: string; // CSV file path
}

const DEFAULT_CHANNEL_GROUPS = [
  { id: "L", name: "Left (L)", channels: [0] },
  { id: "R", name: "Right (R)", channels: [1] },
  { id: "C", name: "Center (C)", channels: [2] },
  { id: "LFE", name: "Subwoofer (LFE)", channels: [3] },
  { id: "SL", name: "Surround Left (SL)", channels: [4] },
  { id: "SR", name: "Surround Right (SR)", channels: [5] },
];

export class CaptureConfigPanel extends HTMLElement {
  private hasRendered = false;
  private config: CaptureConfig | null = null;
  private playbackDevices: AudioDevice[] = [];
  private recordingDevices: AudioDevice[] = [];

  constructor() {
    super();
  }

  async connectedCallback() {
    console.log("[CaptureConfigPanel] connectedCallback called");
    if (!this.hasRendered) {
      console.log("[CaptureConfigPanel] Rendering for first time");
      this.render();
      this.hasRendered = true;
      this.attachEventListeners();
      await this.loadDevices();
      console.log("[CaptureConfigPanel] Initialization complete");
    }
  }

  private async loadDevices(): Promise<void> {
    try {
      const devices = await getAudioDevices();
      this.playbackDevices = devices.output;
      this.recordingDevices = devices.input;

      this.populateDeviceSelect("playback_device", this.playbackDevices);
      this.populateDeviceSelect("recording_device", this.recordingDevices);

      console.log(
        `Loaded ${this.playbackDevices.length} playback devices and ${this.recordingDevices.length} recording devices`,
      );
    } catch (error) {
      console.error("Failed to load audio devices:", error);
    }
  }

  private populateDeviceSelect(selectId: string, devices: AudioDevice[]): void {
    const select = this.querySelector(`#${selectId}`) as HTMLSelectElement;
    if (!select) return;

    // Clear existing options
    select.innerHTML = "";

    if (devices.length === 0) {
      const option = document.createElement("option");
      option.value = "";
      option.textContent = "No devices found";
      select.appendChild(option);
      return;
    }

    // Add devices
    devices.forEach((device) => {
      const option = document.createElement("option");
      option.value = device.name;
      option.textContent = device.name;
      if (device.is_default) {
        option.selected = true;
      }
      select.appendChild(option);
    });

    // Trigger change event to update device info
    select.dispatchEvent(new Event("change"));
  }

  private render(): void {
    console.log(
      "[CaptureConfigPanel] render() called - about to set innerHTML",
    );
    this.innerHTML = `
      <div class="capture-config-panel">
        <div class="capture-config-header">
          <h3>🎤 Audio Device Configuration</h3>
          <p class="capture-config-description">
            Configure your playback and recording devices, set up channel routing, and load microphone calibration.
          </p>
        </div>

        <!-- Playback Device Section -->
        <div class="capture-device-section">
          <div class="capture-section-header">
            <h4>PLAYING</h4>
          </div>

          <div class="capture-device-controls">
            <div class="capture-device-select-row">
              <select id="playback_device" class="capture-device-select">
                <option value="">Select playback device...</option>
              </select>
              <div class="capture-device-info">
                <span id="playback_channels_badge" class="info-badge">? ch</span>
                <span id="playback_samplerate_badge" class="info-badge">? Hz</span>
                <span id="playback_bitdepth_badge" class="info-badge">? bit</span>
              </div>
            </div>

            <div class="capture-channel-count-row">
              <label for="playback_channel_count">Number of channels:</label>
              <input
                type="number"
                id="playback_channel_count"
                class="capture-channel-input"
                min="1"
                max="16"
                value="2"
              />
            </div>

            <div id="playback_channel_mapping" class="capture-channel-mapping">
              <!-- Channel mapping rows will be dynamically generated -->
            </div>
          </div>
        </div>

        <!-- Recording Device Section -->
        <div class="capture-device-section">
          <div class="capture-section-header">
            <h4>RECORDING</h4>
          </div>

          <div class="capture-device-controls">
            <div class="capture-device-select-row">
              <select id="recording_device" class="capture-device-select">
                <option value="">Select recording device...</option>
              </select>
              <div class="capture-device-info">
                <span id="recording_channels_badge" class="info-badge">? ch</span>
                <span id="recording_samplerate_badge" class="info-badge">? Hz</span>
                <span id="recording_bitdepth_badge" class="info-badge">? bit</span>
              </div>
            </div>

            <div class="capture-channel-count-row">
              <label for="recording_channel_count">Number of channels:</label>
              <input
                type="number"
                id="recording_channel_count"
                class="capture-channel-input"
                min="1"
                max="16"
                value="2"
              />
            </div>

            <div id="recording_channel_mapping" class="capture-channel-mapping">
              <!-- Channel mapping rows will be dynamically generated -->
            </div>
          </div>
        </div>

        <!-- Microphone Calibration Section -->
        <div class="capture-calibration-section">
          <div class="capture-section-header">
            <h4>MICROPHONE CALIBRATION</h4>
          </div>

          <div class="capture-calibration-controls">
            <div class="capture-calibration-row">
              <input
                type="text"
                id="calibration_file_path"
                class="capture-calibration-path"
                placeholder="No calibration file loaded"
                readonly
              />
              <input
                type="file"
                id="calibration_file_input"
                accept=".csv,.txt"
                style="display: none"
              />
              <button type="button" id="calibration_browse_btn" class="btn btn-outline">
                Browse...
              </button>
              <button type="button" id="calibration_clear_btn" class="btn btn-outline" style="display: none">
                Clear
              </button>
            </div>
          </div>
        </div>

        <!-- Action Buttons -->
        <div class="capture-config-actions">
          <div class="capture-config-actions-left">
            <button type="button" id="config_load_btn" class="btn btn-outline">
              Load Config
            </button>
            <button type="button" id="config_save_btn" class="btn btn-outline">
              Save Config
            </button>
          </div>
          <div class="capture-config-actions-right">
            <button type="button" id="config_next_btn" class="btn btn-primary">
              Next →
            </button>
          </div>
        </div>
      </div>
    `;
  }

  private attachEventListeners(): void {
    // Playback device change
    const playbackDevice = this.querySelector(
      "#playback_device",
    ) as HTMLSelectElement;
    playbackDevice?.addEventListener("change", () => {
      this.updateDeviceInfo("playback", playbackDevice.value);
    });

    // Recording device change
    const recordingDevice = this.querySelector(
      "#recording_device",
    ) as HTMLSelectElement;
    recordingDevice?.addEventListener("change", () => {
      this.updateDeviceInfo("recording", recordingDevice.value);
    });

    // Playback channel count change
    const playbackChannelCount = this.querySelector(
      "#playback_channel_count",
    ) as HTMLInputElement;
    playbackChannelCount?.addEventListener("change", () => {
      this.updatePlaybackChannelMapping(parseInt(playbackChannelCount.value));
    });

    // Recording channel count change
    const recordingChannelCount = this.querySelector(
      "#recording_channel_count",
    ) as HTMLInputElement;
    recordingChannelCount?.addEventListener("change", () => {
      this.updateRecordingChannelMapping(parseInt(recordingChannelCount.value));
    });

    // Calibration file browse
    const calibrationBrowseBtn = this.querySelector("#calibration_browse_btn");
    const calibrationFileInput = this.querySelector(
      "#calibration_file_input",
    ) as HTMLInputElement;
    calibrationBrowseBtn?.addEventListener("click", () => {
      calibrationFileInput?.click();
    });

    calibrationFileInput?.addEventListener("change", () => {
      if (calibrationFileInput.files && calibrationFileInput.files.length > 0) {
        const file = calibrationFileInput.files[0];
        this.setCalibrationFile(file.name);
      }
    });

    // Calibration clear
    const calibrationClearBtn = this.querySelector("#calibration_clear_btn");
    calibrationClearBtn?.addEventListener("click", () => {
      this.clearCalibrationFile();
    });

    // Config actions
    const configLoadBtn = this.querySelector("#config_load_btn");
    configLoadBtn?.addEventListener("click", () => {
      this.loadConfig();
    });

    const configSaveBtn = this.querySelector("#config_save_btn");
    configSaveBtn?.addEventListener("click", () => {
      this.saveConfig();
    });

    const configNextBtn = this.querySelector("#config_next_btn");
    configNextBtn?.addEventListener("click", () => {
      this.onNext();
    });

    // Initialize with default channel counts
    this.updatePlaybackChannelMapping(2);
    this.updateRecordingChannelMapping(2);
  }

  private async updateDeviceInfo(
    type: "playback" | "recording",
    deviceName: string,
  ): Promise<void> {
    const devices =
      type === "playback" ? this.playbackDevices : this.recordingDevices;
    const device = devices.find((d) => d.name === deviceName);

    if (!device || !device.default_config) {
      return;
    }

    const config = device.default_config;
    const prefix = type === "playback" ? "playback" : "recording";

    // Update badges
    const channelsBadge = this.querySelector(`#${prefix}_channels_badge`);
    const sampleRateBadge = this.querySelector(`#${prefix}_samplerate_badge`);
    const bitDepthBadge = this.querySelector(`#${prefix}_bitdepth_badge`);

    if (channelsBadge) {
      channelsBadge.textContent = `${config.channels} ch`;
    }

    if (sampleRateBadge) {
      const kHz = config.sample_rate / 1000;
      sampleRateBadge.textContent = `${kHz} kHz`;
    }

    if (bitDepthBadge) {
      // Extract bit depth from sample format
      let bitDepth = "?";
      if (config.sample_format === "f32") {
        bitDepth = "32";
      } else if (
        config.sample_format === "i16" ||
        config.sample_format === "u16"
      ) {
        bitDepth = "16";
      }
      bitDepthBadge.textContent = `${bitDepth} bit`;
    }
  }

  private updatePlaybackChannelMapping(channelCount: number): void {
    const container = this.querySelector("#playback_channel_mapping");
    if (!container) return;

    container.innerHTML = "";

    for (let i = 0; i < channelCount; i++) {
      const row = document.createElement("div");
      row.className = "capture-channel-row";

      const defaultGroup =
        i < DEFAULT_CHANNEL_GROUPS.length ? DEFAULT_CHANNEL_GROUPS[i] : null;

      row.innerHTML = `
        <span class="capture-channel-label">Channel ${i + 1}:</span>
        <label class="capture-channel-interface">
          <span>Interface</span>
          <input
            type="number"
            class="capture-interface-input"
            data-channel="${i}"
            min="1"
            max="16"
            value="${i + 1}"
          />
        </label>
        <select class="capture-group-select" data-channel="${i}">
          <option value="">No group</option>
          ${DEFAULT_CHANNEL_GROUPS.map(
            (g) => `
            <option value="${g.id}" ${defaultGroup?.id === g.id ? "selected" : ""}>
              ${g.name}
            </option>
          `,
          ).join("")}
        </select>
      `;

      container.appendChild(row);
    }
  }

  private updateRecordingChannelMapping(channelCount: number): void {
    const container = this.querySelector("#recording_channel_mapping");
    if (!container) return;

    container.innerHTML = "";

    for (let i = 0; i < channelCount; i++) {
      const row = document.createElement("div");
      row.className = "capture-channel-row";

      row.innerHTML = `
        <span class="capture-channel-label">Channel ${i + 1}:</span>
        <label class="capture-channel-interface">
          <span>Interface</span>
          <input
            type="number"
            class="capture-interface-input"
            data-channel="${i}"
            min="1"
            max="16"
            value="${i + 1}"
          />
        </label>
      `;

      container.appendChild(row);
    }
  }

  private setCalibrationFile(fileName: string): void {
    const pathInput = this.querySelector(
      "#calibration_file_path",
    ) as HTMLInputElement;
    const clearBtn = this.querySelector(
      "#calibration_clear_btn",
    ) as HTMLButtonElement;

    if (pathInput) {
      pathInput.value = fileName;
    }

    if (clearBtn) {
      clearBtn.style.display = "inline-block";
    }
  }

  private clearCalibrationFile(): void {
    const pathInput = this.querySelector(
      "#calibration_file_path",
    ) as HTMLInputElement;
    const fileInput = this.querySelector(
      "#calibration_file_input",
    ) as HTMLInputElement;
    const clearBtn = this.querySelector(
      "#calibration_clear_btn",
    ) as HTMLButtonElement;

    if (pathInput) {
      pathInput.value = "";
      pathInput.placeholder = "No calibration file loaded";
    }

    if (fileInput) {
      fileInput.value = "";
    }

    if (clearBtn) {
      clearBtn.style.display = "none";
    }
  }

  private async loadConfig(): Promise<void> {
    try {
      // Open file dialog
      const filePath = await open({
        title: "Load Capture Configuration",
        filters: [
          {
            name: "JSON",
            extensions: ["json"],
          },
        ],
      });

      if (!filePath) return;

      // Read and parse config
      const configJson = await readTextFile(filePath as string);
      const config = JSON.parse(configJson);

      // Apply config to UI
      this.setConfig(config);

      console.log("Config loaded successfully");
    } catch (error) {
      console.error("Failed to load config:", error);
      alert(`Failed to load configuration: ${error}`);
    }
  }

  private async saveConfig(): Promise<void> {
    try {
      // Get current config
      const config = this.getConfig();

      // Open save dialog
      const filePath = await save({
        title: "Save Capture Configuration",
        filters: [
          {
            name: "JSON",
            extensions: ["json"],
          },
        ],
        defaultPath: "capture-config.json",
      });

      if (!filePath) return;

      // Save config
      await writeTextFile(filePath, JSON.stringify(config, null, 2));

      console.log("Config saved successfully");
    } catch (error) {
      console.error("Failed to save config:", error);
      alert(`Failed to save configuration: ${error}`);
    }
  }

  private onNext(): void {
    // Emit event to move to next step
    const event = new CustomEvent("captureConfigComplete", {
      bubbles: true,
      composed: true,
      detail: { config: this.getConfig() },
    });
    this.dispatchEvent(event);
  }

  public getConfig(): CaptureConfig {
    // Gather all configuration data
    const playbackChannelCount = parseInt(
      (this.querySelector("#playback_channel_count") as HTMLInputElement)
        ?.value || "2",
    );
    const recordingChannelCount = parseInt(
      (this.querySelector("#recording_channel_count") as HTMLInputElement)
        ?.value || "2",
    );

    // Gather playback channel groups
    const channelGroups: ChannelGroup[] = [];
    const playbackMapping = this.querySelectorAll(
      "#playback_channel_mapping .capture-channel-row",
    );

    playbackMapping.forEach((row, idx) => {
      const groupSelect = row.querySelector(
        ".capture-group-select",
      ) as HTMLSelectElement;
      const groupId = groupSelect?.value;

      if (groupId) {
        const existingGroup = channelGroups.find((g) => g.id === groupId);
        if (existingGroup) {
          existingGroup.channels.push(idx);
        } else {
          const groupInfo = DEFAULT_CHANNEL_GROUPS.find(
            (g) => g.id === groupId,
          );
          if (groupInfo) {
            channelGroups.push({
              id: groupId,
              name: groupInfo.name,
              channels: [idx],
            });
          }
        }
      }
    });

    // Gather recording channel mapping
    const channelMapping: number[] = [];
    const recordingMappingRows = this.querySelectorAll(
      "#recording_channel_mapping .capture-interface-input",
    );
    recordingMappingRows.forEach((input) => {
      const interfaceChannel = parseInt((input as HTMLInputElement).value);
      channelMapping.push(interfaceChannel - 1); // Convert to 0-indexed
    });

    const calibrationPath =
      (this.querySelector("#calibration_file_path") as HTMLInputElement)
        ?.value || undefined;

    return {
      playback: {
        deviceId:
          (this.querySelector("#playback_device") as HTMLSelectElement)
            ?.value || "",
        deviceName:
          (this.querySelector("#playback_device") as HTMLSelectElement)
            ?.selectedOptions[0]?.text || "",
        channels: playbackChannelCount,
        sampleRate: 48000, // TODO: Get from device info
        bitDepth: 24, // TODO: Get from device info
        channelGroups,
      },
      recording: {
        deviceId:
          (this.querySelector("#recording_device") as HTMLSelectElement)
            ?.value || "",
        deviceName:
          (this.querySelector("#recording_device") as HTMLSelectElement)
            ?.selectedOptions[0]?.text || "",
        channels: recordingChannelCount,
        sampleRate: 48000, // TODO: Get from device info
        bitDepth: 24, // TODO: Get from device info
        channelMapping,
      },
      microphoneCalibration: calibrationPath,
    };
  }

  public setConfig(config: CaptureConfig): void {
    this.config = config;

    // Set playback device
    const playbackSelect = this.querySelector(
      "#playback_device",
    ) as HTMLSelectElement;
    if (playbackSelect && config.playback.deviceName) {
      playbackSelect.value = config.playback.deviceName;
      playbackSelect.dispatchEvent(new Event("change"));
    }

    // Set recording device
    const recordingSelect = this.querySelector(
      "#recording_device",
    ) as HTMLSelectElement;
    if (recordingSelect && config.recording.deviceName) {
      recordingSelect.value = config.recording.deviceName;
      recordingSelect.dispatchEvent(new Event("change"));
    }

    // Set playback channel count
    const playbackChannelCount = this.querySelector(
      "#playback_channel_count",
    ) as HTMLInputElement;
    if (playbackChannelCount) {
      playbackChannelCount.value = config.playback.channels.toString();
      playbackChannelCount.dispatchEvent(new Event("change"));
    }

    // Set recording channel count
    const recordingChannelCount = this.querySelector(
      "#recording_channel_count",
    ) as HTMLInputElement;
    if (recordingChannelCount) {
      recordingChannelCount.value = config.recording.channels.toString();
      recordingChannelCount.dispatchEvent(new Event("change"));
    }

    // Set calibration file
    if (config.microphoneCalibration) {
      this.setCalibrationFile(config.microphoneCalibration);
    }

    // Note: Channel groups and mapping will be set by the change events above
  }
}

// Register the custom element
console.log("[MODULE] Registering capture-config-panel custom element...");
if (!customElements.get("capture-config-panel")) {
  customElements.define("capture-config-panel", CaptureConfigPanel);
  console.log("[MODULE] capture-config-panel registered successfully");
} else {
  console.log("[MODULE] capture-config-panel already registered");
}
