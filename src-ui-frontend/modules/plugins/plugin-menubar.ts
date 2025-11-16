// Plugin Menubar Component
// Shared menubar for plugins: Name, Presets, Matrix, Mute/Solo

import type {
  MenubarConfig,
  MenubarButton,
  PluginPreset,
} from "./plugin-types";

export interface PluginMenubarCallbacks {
  onPresetLoad?: (preset: PluginPreset) => void;
  onPresetSave?: (name: string) => void;
  onMatrix?: () => void;
  onMute?: (muted: boolean) => void;
  onSolo?: (solo: boolean) => void;
  onBypass?: (bypassed: boolean) => void;
}

/**
 * Plugin Menubar Component
 * Provides consistent menubar across all plugins
 */
export class PluginMenubar {
  private container: HTMLElement;
  private config: MenubarConfig;
  private callbacks: PluginMenubarCallbacks;
  private pluginName: string;

  // State
  private presets: PluginPreset[] = [];
  private currentPresetName: string | null = null;
  private muted: boolean = false;
  private solo: boolean = false;
  private bypassed: boolean = false;

  // UI elements
  private muteButton: HTMLButtonElement | null = null;
  private soloButton: HTMLButtonElement | null = null;
  private bypassButton: HTMLButtonElement | null = null;

  constructor(
    container: HTMLElement,
    pluginName: string,
    config: MenubarConfig = {},
    callbacks: PluginMenubarCallbacks = {},
  ) {
    this.container = container;
    this.pluginName = pluginName;
    this.config = {
      showName: config.showName ?? true,
      showPresets: config.showPresets ?? true,
      showMatrix: config.showMatrix ?? true,
      showMuteSolo: config.showMuteSolo ?? true,
      customButtons: config.customButtons ?? [],
    };
    this.callbacks = callbacks;

    this.render();
  }

  /**
   * Render the menubar
   */
  render(): void {
    this.container.innerHTML = `
      <nav class="level is-mobile p-3 has-background-dark" style="border-bottom: 1px solid #404040;">
        <div class="level-left">
          ${this.config.showName ? `<div class="level-item"><p class="has-text-weight-semibold has-text-light">${this.pluginName}</p></div>` : ""}
        </div>
        <div class="level-right">
          ${this.renderPresets()}
          ${this.renderMatrix()}
          ${this.renderMuteSolo()}
          ${this.renderBypass()}
          ${this.renderCustomButtons()}
        </div>
      </nav>
    `;

    this.attachEventListeners();
  }

  /**
   * Render presets dropdown
   */
  private renderPresets(): string {
    if (!this.config.showPresets) return "";

    return `
      <div class="level-item">
        <div class="dropdown menubar-presets">
          <div class="dropdown-trigger">
            <button class="button is-small is-dark preset-button" aria-haspopup="true" aria-controls="dropdown-menu" title="Presets">
              <span>${this.currentPresetName || "Presets"}</span>
              <span class="icon is-small">
                <svg width="12" height="12" viewBox="0 0 12 12" fill="currentColor">
                  <path d="M6 8L2 4h8z"/>
                </svg>
              </span>
            </button>
          </div>
          <div class="dropdown-menu" id="dropdown-menu" role="menu" style="display: none;">
            <div class="dropdown-content">
              <div class="preset-list"></div>
              <hr class="dropdown-divider">
              <div class="dropdown-item">
                <button class="button is-small is-fullwidth preset-save-btn">Save Preset</button>
              </div>
            </div>
          </div>
        </div>
      </div>
    `;
  }

  /**
   * Render matrix button
   */
  private renderMatrix(): string {
    if (!this.config.showMatrix) return "";

    return `
      <div class="level-item">
        <button class="button is-small is-dark matrix-button" title="Routing Matrix">
          <span class="icon is-small">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
              <rect x="2" y="2" width="12" height="12" rx="1"/>
              <line x1="5" y1="2" x2="5" y2="14"/>
              <line x1="11" y1="2" x2="11" y2="14"/>
              <line x1="2" y1="5" x2="14" y2="5"/>
              <line x1="2" y1="11" x2="14" y2="11"/>
            </svg>
          </span>
        </button>
      </div>
    `;
  }

  /**
   * Render mute/solo buttons
   */
  private renderMuteSolo(): string {
    if (!this.config.showMuteSolo) return "";

    return `
      <div class="level-item">
        <div class="buttons has-addons">
          <button class="button is-small ${this.muted ? "is-danger" : "is-dark"} mute-button" title="Mute">M</button>
          <button class="button is-small ${this.solo ? "is-warning" : "is-dark"} solo-button" title="Solo">S</button>
        </div>
      </div>
    `;
  }

  /**
   * Render bypass button
   */
  private renderBypass(): string {
    return `
      <div class="level-item">
        <button class="button is-small ${this.bypassed ? "is-info" : "is-dark"} bypass-button" title="Bypass">
          <span class="icon is-small">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
              <circle cx="8" cy="8" r="6"/>
              <line x1="4" y1="12" x2="12" y2="4"/>
            </svg>
          </span>
        </button>
      </div>
    `;
  }

  /**
   * Render custom buttons
   */
  private renderCustomButtons(): string {
    return this.config
      .customButtons!.map(
        (btn) => `
        <div class="level-item">
          <button class="button is-small is-dark custom-button" data-button-id="${btn.id}" title="${btn.label}">
            ${btn.icon || btn.label}
          </button>
        </div>
      `,
      )
      .join("");
  }

  /**
   * Attach event listeners
   */
  private attachEventListeners(): void {
    // Preset dropdown (Bulma dropdown)
    const presetDropdownContainer = this.container.querySelector(
      ".menubar-presets",
    ) as HTMLElement;
    const presetButton = this.container.querySelector(
      ".preset-button",
    ) as HTMLButtonElement;
    const presetDropdownMenu = this.container.querySelector(
      ".dropdown-menu",
    ) as HTMLElement;
    if (presetButton && presetDropdownContainer && presetDropdownMenu) {
      presetButton.addEventListener("click", (e) => {
        e.stopPropagation();
        presetDropdownContainer.classList.toggle("is-active");
        const isActive =
          presetDropdownContainer.classList.contains("is-active");
        presetDropdownMenu.style.display = isActive ? "block" : "none";
        if (isActive) {
          this.updatePresetList();
        }
      });
    }

    // Preset save button
    const presetSaveBtn = this.container.querySelector(
      ".preset-save-btn",
    ) as HTMLButtonElement;
    if (presetSaveBtn) {
      presetSaveBtn.addEventListener("click", () => {
        const name = prompt("Enter preset name:");
        if (name && this.callbacks.onPresetSave) {
          this.callbacks.onPresetSave(name);
        }
      });
    }

    // Matrix button
    const matrixButton = this.container.querySelector(
      ".matrix-button",
    ) as HTMLButtonElement;
    if (matrixButton) {
      matrixButton.addEventListener("click", () => {
        if (this.callbacks.onMatrix) {
          this.callbacks.onMatrix();
        }
      });
    }

    // Mute button
    this.muteButton = this.container.querySelector(
      ".mute-button",
    ) as HTMLButtonElement;
    if (this.muteButton) {
      this.muteButton.addEventListener("click", () => {
        this.muted = !this.muted;
        this.muteButton!.classList.remove("is-dark", "is-danger");
        this.muteButton!.classList.add(this.muted ? "is-danger" : "is-dark");
        if (this.callbacks.onMute) {
          this.callbacks.onMute(this.muted);
        }
      });
    }

    // Solo button
    this.soloButton = this.container.querySelector(
      ".solo-button",
    ) as HTMLButtonElement;
    if (this.soloButton) {
      this.soloButton.addEventListener("click", () => {
        this.solo = !this.solo;
        this.soloButton!.classList.remove("is-dark", "is-warning");
        this.soloButton!.classList.add(this.solo ? "is-warning" : "is-dark");
        if (this.callbacks.onSolo) {
          this.callbacks.onSolo(this.solo);
        }
      });
    }

    // Bypass button
    this.bypassButton = this.container.querySelector(
      ".bypass-button",
    ) as HTMLButtonElement;
    if (this.bypassButton) {
      this.bypassButton.addEventListener("click", () => {
        this.bypassed = !this.bypassed;
        this.bypassButton!.classList.remove("is-dark", "is-info");
        this.bypassButton!.classList.add(this.bypassed ? "is-info" : "is-dark");
        if (this.callbacks.onBypass) {
          this.callbacks.onBypass(this.bypassed);
        }
      });
    }

    // Custom buttons
    const customButtons = this.container.querySelectorAll(".custom-button");
    customButtons.forEach((btn) => {
      const buttonId = (btn as HTMLElement).dataset.buttonId!;
      const customButton = this.config.customButtons!.find(
        (b) => b.id === buttonId,
      );
      if (customButton) {
        btn.addEventListener("click", () => customButton.onClick());
      }
    });

    // Close dropdown when clicking outside
    document.addEventListener("click", (e) => {
      if (
        presetDropdownContainer &&
        presetDropdownMenu &&
        !this.container.contains(e.target as Node)
      ) {
        presetDropdownContainer.classList.remove("is-active");
        presetDropdownMenu.style.display = "none";
      }
    });
  }

  /**
   * Update preset list
   */
  private updatePresetList(): void {
    const presetList = this.container.querySelector(
      ".preset-list",
    ) as HTMLElement;
    if (!presetList) return;

    if (this.presets.length === 0) {
      presetList.innerHTML =
        '<div class="dropdown-item has-text-grey">No presets available</div>';
      return;
    }

    presetList.innerHTML = this.presets
      .map(
        (preset) => `
        <a class="dropdown-item ${preset.name === this.currentPresetName ? "is-active" : ""}" data-preset-name="${preset.name}">
          ${preset.name}
        </a>
      `,
      )
      .join("");

    // Attach click handlers
    const presetItems = presetList.querySelectorAll(".dropdown-item");
    presetItems.forEach((item) => {
      item.addEventListener("click", () => {
        const presetName = (item as HTMLElement).dataset.presetName!;
        const preset = this.presets.find((p) => p.name === presetName);
        if (preset && this.callbacks.onPresetLoad) {
          this.callbacks.onPresetLoad(preset);
          this.setCurrentPreset(presetName);
        }
      });
    });
  }

  /**
   * Set available presets
   */
  setPresets(presets: PluginPreset[]): void {
    this.presets = presets;
    this.updatePresetList();
  }

  /**
   * Add a preset
   */
  addPreset(preset: PluginPreset): void {
    this.presets.push(preset);
    this.updatePresetList();
  }

  /**
   * Set current preset
   */
  setCurrentPreset(name: string | null): void {
    this.currentPresetName = name;
    const presetButton = this.container.querySelector(
      ".preset-button span",
    ) as HTMLSpanElement;
    if (presetButton) {
      presetButton.textContent = name || "Presets";
    }
    this.updatePresetList();
  }

  /**
   * Set mute state
   */
  setMuted(muted: boolean): void {
    this.muted = muted;
    if (this.muteButton) {
      this.muteButton.classList.toggle("active", muted);
    }
  }

  /**
   * Set solo state
   */
  setSolo(solo: boolean): void {
    this.solo = solo;
    if (this.soloButton) {
      this.soloButton.classList.toggle("active", solo);
    }
  }

  /**
   * Set bypass state
   */
  setBypassed(bypassed: boolean): void {
    this.bypassed = bypassed;
    if (this.bypassButton) {
      this.bypassButton.classList.toggle("active", bypassed);
    }
  }

  /**
   * Destroy the menubar
   */
  destroy(): void {
    this.container.innerHTML = "";
  }
}
