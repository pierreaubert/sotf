/**
 * Device Info UI Component
 * Shows enhanced device capabilities from cpal
 * TODO: Fix device manager import - currently disabled
 */

// import { AudioDeviceManager } from "./device-manager";

// Define UnifiedAudioDevice interface
export interface UnifiedAudioDevice {
  deviceId: string;
  name: string;
  type: "input" | "output";
  isDefault: boolean;
  isWebAudio: boolean;
  channels: number | null;
  sampleRates: number[];
  defaultSampleRate?: number;
  formats: string[];
}

export class DeviceInfoUI {
  private container: HTMLElement;
  // private deviceManager: AudioDeviceManager;
  private currentDevice: UnifiedAudioDevice | null = null;

  constructor(container: HTMLElement) {
    this.container = container;
    // this.deviceManager = new AudioDeviceManager(true);
  }

  /**
   * Initialize and show device info panel
   */
  async initialize(): Promise<void> {
    // Clear container
    this.container.innerHTML = "";

    // Add loading indicator
    this.showLoading();

    try {
      // TODO: Re-enable once device manager is fixed
      // const devices = await this.deviceManager.enumerateDevices();
      this.showError("Device enumeration temporarily disabled");
      // this.buildUI(
      //   devices as {
      //     input: UnifiedAudioDevice[];
      //     output: UnifiedAudioDevice[];
      //   },
      // );
    } catch (error) {
      this.showError("Failed to enumerate audio devices: " + error);
    }
  }

  /**
   * Show loading indicator
   */
  private showLoading(): void {
    this.container.innerHTML = `
      <div class="device-info-loading">
        <div class="spinner"></div>
        <p>Detecting audio devices...</p>
      </div>
    `;
  }

  /**
   * Show error message
   */
  private showError(message: string): void {
    this.container.innerHTML = `
      <div class="device-info-error">
        <span class="error-icon">⚠️</span>
        <p>${message}</p>
      </div>
    `;
  }

  /**
   * Build the device info UI
   */
  private buildUI(devices: {
    input: UnifiedAudioDevice[];
    output: UnifiedAudioDevice[];
  }): void {
    // Clear container
    this.container.innerHTML = "";

    // Create main wrapper
    const wrapper = document.createElement("div");
    wrapper.className = "device-info-wrapper";

    // Add title
    const title = document.createElement("h3");
    title.className = "device-info-title";
    title.textContent = "Audio Device Information";
    wrapper.appendChild(title);

    // Add device type tabs
    const tabs = document.createElement("div");
    tabs.className = "device-info-tabs";

    const inputTab = document.createElement("button");
    inputTab.className = "device-tab active";
    inputTab.textContent = `Input Devices (${devices.input.length})`;
    inputTab.onclick = () =>
      this.showDeviceList("input", devices.input, inputTab, outputTab);
    tabs.appendChild(inputTab);

    const outputTab = document.createElement("button");
    outputTab.className = "device-tab";
    outputTab.textContent = `Output Devices (${devices.output.length})`;
    outputTab.onclick = () =>
      this.showDeviceList("output", devices.output, outputTab, inputTab);
    tabs.appendChild(outputTab);

    wrapper.appendChild(tabs);

    // Device list container
    const listContainer = document.createElement("div");
    listContainer.className = "device-list-container";
    listContainer.id = "device-list-container";
    wrapper.appendChild(listContainer);

    // Selected device details container
    const detailsContainer = document.createElement("div");
    detailsContainer.className = "device-details-container";
    detailsContainer.id = "device-details-container";
    detailsContainer.style.display = "none";
    wrapper.appendChild(detailsContainer);

    this.container.appendChild(wrapper);

    // Show input devices by default
    this.showDeviceList("input", devices.input, inputTab, outputTab);

    // Add styles if not already added
    this.addStyles();
  }

  /**
   * Show device list for a specific type
   */
  private showDeviceList(
    type: "input" | "output",
    devices: UnifiedAudioDevice[],
    activeTab: HTMLElement,
    inactiveTab: HTMLElement,
  ): void {
    // Update tab states
    activeTab.classList.add("active");
    inactiveTab.classList.remove("active");

    // Get container
    const container = document.getElementById("device-list-container");
    if (!container) return;

    // Clear container
    container.innerHTML = "";

    if (devices.length === 0) {
      container.innerHTML = `
        <div class="no-devices">
          <p>No ${type} devices found</p>
        </div>
      `;
      return;
    }

    // Create device cards
    devices.forEach((device) => {
      const card = this.createDeviceCard(device);
      container.appendChild(card);
    });
  }

  /**
   * Create a device card element
   */
  private createDeviceCard(device: UnifiedAudioDevice): HTMLElement {
    const card = document.createElement("div");
    card.className = "device-card";
    if (device.isDefault) {
      card.classList.add("default");
    }

    // Device header
    const header = document.createElement("div");
    header.className = "device-card-header";

    // Device name
    const name = document.createElement("div");
    name.className = "device-name";
    name.textContent = device.name;
    header.appendChild(name);

    // Device badges
    const badges = document.createElement("div");
    badges.className = "device-badges";

    // Source badge (cpal or WebAudio)
    const sourceBadge = document.createElement("span");
    sourceBadge.className = `badge badge-${device.isWebAudio ? "web" : "cpal"}`;
    sourceBadge.textContent = device.isWebAudio ? "WebAudio" : "CPAL";
    sourceBadge.title = device.isWebAudio
      ? "Standard browser audio API"
      : "Enhanced native audio (via Tauri)";
    badges.appendChild(sourceBadge);

    // Default badge
    if (device.isDefault) {
      const defaultBadge = document.createElement("span");
      defaultBadge.className = "badge badge-default";
      defaultBadge.textContent = "Default";
      badges.appendChild(defaultBadge);
    }

    // Channel count badge
    const channelsBadge = document.createElement("span");
    channelsBadge.className = "badge badge-channels";
    channelsBadge.textContent = `${device.channels}ch`;
    badges.appendChild(channelsBadge);

    header.appendChild(badges);
    card.appendChild(header);

    // Device info
    const info = document.createElement("div");
    info.className = "device-card-info";

    // Sample rates
    if (device.sampleRates.length > 0) {
      const ratesDiv = document.createElement("div");
      ratesDiv.className = "device-info-row";

      const ratesLabel = document.createElement("span");
      ratesLabel.className = "info-label";
      ratesLabel.textContent = "Sample Rates:";
      ratesDiv.appendChild(ratesLabel);

      const ratesValue = document.createElement("span");
      ratesValue.className = "info-value";
      const ratesList = device.sampleRates
        .map((r: number) => (r >= 1000 ? `${r / 1000}kHz` : `${r}Hz`))
        .join(", ");
      ratesValue.textContent = ratesList;
      ratesDiv.appendChild(ratesValue);

      info.appendChild(ratesDiv);
    }

    // Formats
    if (device.formats.length > 0) {
      const formatsDiv = document.createElement("div");
      formatsDiv.className = "device-info-row";

      const formatsLabel = document.createElement("span");
      formatsLabel.className = "info-label";
      formatsLabel.textContent = "Formats:";
      formatsDiv.appendChild(formatsLabel);

      const formatsValue = document.createElement("span");
      formatsValue.className = "info-value";
      formatsValue.textContent = device.formats.join(", ");
      formatsDiv.appendChild(formatsValue);

      info.appendChild(formatsDiv);
    }

    card.appendChild(info);

    // Actions
    const actions = document.createElement("div");
    actions.className = "device-card-actions";

    // Select button
    const selectBtn = document.createElement("button");
    selectBtn.className = "btn-select-device";
    selectBtn.textContent = "Select Device";
    selectBtn.onclick = () => this.selectDevice(device);
    actions.appendChild(selectBtn);

    // Details button
    if (!device.isWebAudio) {
      const detailsBtn = document.createElement("button");
      detailsBtn.className = "btn-device-details";
      detailsBtn.textContent = "Details";
      detailsBtn.onclick = () => this.showDeviceDetails(device);
      actions.appendChild(detailsBtn);
    }

    card.appendChild(actions);

    return card;
  }

  /**
   * Select a device for use
   */
  private async selectDevice(device: UnifiedAudioDevice): Promise<void> {
    try {
      // DeviceManager doesn't have selectDevice, just store the selection
      this.showNotification(`Selected: ${device.name}`, "success");
      this.currentDevice = device;

      // Update UI to show selected device
      document.querySelectorAll(".device-card").forEach((card) => {
        card.classList.remove("selected");
      });

      // Find and mark selected card
      const cards = document.querySelectorAll(".device-card");
      cards.forEach((card) => {
        const nameEl = card.querySelector(".device-name");
        if (nameEl?.textContent === device.name) {
          card.classList.add("selected");
        }
      });
    } catch (error) {
      this.showNotification(`Error selecting device: ${error}`, "error");
    }
  }

  /**
   * Show detailed device properties
   */
  private async showDeviceDetails(device: UnifiedAudioDevice): Promise<void> {
    const container = document.getElementById("device-details-container");
    if (!container) return;

    // Show loading
    container.innerHTML =
      '<div class="loading">Loading device details...</div>';
    container.style.display = "block";

    try {
      // DeviceManager doesn't have getDeviceDetails, show device info directly
      const details = {
        deviceId: device.deviceId,
        name: device.name,
        type: device.type,
        isDefault: device.isDefault,
        isWebAudio: device.isWebAudio,
        channels: device.channels,
        sampleRates: device.sampleRates,
        defaultSampleRate: device.defaultSampleRate,
        formats: device.formats,
      };

      // Build details view
      container.innerHTML = `
        <div class="device-details">
          <div class="details-header">
            <h4>${device.name}</h4>
            <button class="close-details" onclick="this.parentElement.parentElement.parentElement.style.display='none'">×</button>
          </div>
          <div class="details-content">
            <pre>${JSON.stringify(details, null, 2)}</pre>
          </div>
        </div>
      `;
    } catch (error) {
      container.innerHTML = `
        <div class="error">
          Failed to load device details: ${error}
        </div>
      `;
    }
  }

  /**
   * Show notification
   */
  private showNotification(
    message: string,
    type: "success" | "error" | "info",
  ): void {
    // Create notification element
    const notification = document.createElement("div");
    notification.className = `notification notification-${type}`;
    notification.textContent = message;

    // Add to container
    this.container.appendChild(notification);

    // Auto-remove after 3 seconds
    setTimeout(() => {
      notification.remove();
    }, 3000);
  }

  /**
   * Add required styles
   */
  private addStyles(): void {
    if (document.getElementById("device-info-styles")) return;

    const style = document.createElement("style");
    style.id = "device-info-styles";
    style.textContent = `
      .device-info-wrapper {
        padding: 1rem;
      }
      
      .device-info-title {
        margin-bottom: 1rem;
        color: var(--text-primary);
      }
      
      .device-info-tabs {
        display: flex;
        gap: 0.5rem;
        margin-bottom: 1rem;
        border-bottom: 1px solid var(--border-color);
      }
      
      .device-tab {
        padding: 0.5rem 1rem;
        background: transparent;
        border: none;
        border-bottom: 2px solid transparent;
        color: var(--text-secondary);
        cursor: pointer;
        transition: all 0.2s;
      }
      
      .device-tab:hover {
        color: var(--text-primary);
      }
      
      .device-tab.active {
        color: var(--button-primary);
        border-bottom-color: var(--button-primary);
      }
      
      .device-card {
        background: var(--bg-secondary);
        border: 1px solid var(--border-color);
        border-radius: 8px;
        padding: 1rem;
        margin-bottom: 0.5rem;
        transition: all 0.2s;
      }
      
      .device-card:hover {
        border-color: var(--button-primary-hover);
      }
      
      .device-card.default {
        border-color: var(--success-color);
      }
      
      .device-card.selected {
        background: var(--bg-accent);
        border-color: var(--button-primary);
      }
      
      .device-card-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 0.5rem;
      }
      
      .device-name {
        font-weight: 500;
        color: var(--text-primary);
      }
      
      .device-badges {
        display: flex;
        gap: 0.25rem;
      }
      
      .badge {
        padding: 0.125rem 0.5rem;
        border-radius: 12px;
        font-size: 0.75rem;
        font-weight: 500;
      }
      
      .badge-cpal {
        background: var(--button-primary);
        color: white;
      }
      
      .badge-web {
        background: var(--bg-accent);
        color: var(--text-secondary);
      }
      
      .badge-default {
        background: var(--success-color);
        color: white;
      }
      
      .badge-channels {
        background: var(--bg-accent);
        color: var(--text-secondary);
      }
      
      .device-card-info {
        margin-bottom: 0.75rem;
        font-size: 0.875rem;
      }
      
      .device-info-row {
        display: flex;
        margin-bottom: 0.25rem;
      }
      
      .info-label {
        color: var(--text-secondary);
        margin-right: 0.5rem;
        min-width: 100px;
      }
      
      .info-value {
        color: var(--text-primary);
      }
      
      .device-card-actions {
        display: flex;
        gap: 0.5rem;
      }
      
      .btn-select-device,
      .btn-device-details {
        padding: 0.375rem 0.75rem;
        border-radius: 4px;
        border: 1px solid var(--border-color);
        background: var(--bg-primary);
        color: var(--text-primary);
        cursor: pointer;
        font-size: 0.875rem;
        transition: all 0.2s;
      }
      
      .btn-select-device:hover {
        background: var(--button-primary);
        color: white;
        border-color: var(--button-primary);
      }
      
      .btn-device-details:hover {
        background: var(--bg-accent);
      }
      
      .notification {
        position: fixed;
        top: 1rem;
        right: 1rem;
        padding: 0.75rem 1rem;
        border-radius: 4px;
        color: white;
        font-weight: 500;
        z-index: 1000;
        animation: slideIn 0.3s ease-out;
      }
      
      .notification-success {
        background: var(--success-color);
      }
      
      .notification-error {
        background: var(--error-color);
      }
      
      .notification-info {
        background: var(--button-primary);
      }
      
      @keyframes slideIn {
        from {
          transform: translateX(100%);
          opacity: 0;
        }
        to {
          transform: translateX(0);
          opacity: 1;
        }
      }
      
      .device-details {
        background: var(--bg-secondary);
        border: 1px solid var(--border-color);
        border-radius: 8px;
        padding: 1rem;
        margin-top: 1rem;
      }
      
      .details-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 1rem;
      }
      
      .close-details {
        background: transparent;
        border: none;
        font-size: 1.5rem;
        cursor: pointer;
        color: var(--text-secondary);
      }
      
      .details-content pre {
        background: var(--bg-primary);
        padding: 1rem;
        border-radius: 4px;
        overflow-x: auto;
        font-size: 0.875rem;
        color: var(--text-secondary);
      }
      
      .no-devices {
        text-align: center;
        padding: 2rem;
        color: var(--text-secondary);
      }
      
      .device-info-loading {
        text-align: center;
        padding: 2rem;
      }
      
      .spinner {
        display: inline-block;
        width: 32px;
        height: 32px;
        border: 3px solid var(--border-color);
        border-top-color: var(--button-primary);
        border-radius: 50%;
        animation: spin 1s linear infinite;
      }
      
      @keyframes spin {
        to {
          transform: rotate(360deg);
        }
      }
      
      .device-info-error {
        text-align: center;
        padding: 2rem;
        color: var(--error-color);
      }
      
      .error-icon {
        font-size: 2rem;
        display: block;
        margin-bottom: 0.5rem;
      }
    `;
    document.head.appendChild(style);
  }
}

/**
 * Initialize device info UI in a container
 */
export async function initializeDeviceInfoUI(
  containerId: string,
): Promise<DeviceInfoUI | null> {
  const container = document.getElementById(containerId);
  if (!container) {
    console.error(`Container with ID '${containerId}' not found`);
    return null;
  }

  const ui = new DeviceInfoUI(container);
  await ui.initialize();
  return ui;
}
