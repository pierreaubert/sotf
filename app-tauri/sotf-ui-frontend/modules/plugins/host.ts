// Plugin Host Component
// Container for managing multiple plugins with menubar, hosting bar, and display box

import { PluginMenubar, type PluginMenubarCallbacks } from "./plugin-menubar";
import { LevelMeter } from "./level-meter";
import type {
  IPlugin,
  MenubarConfig,
  LevelMeterData,
  LUFSMeterData,
  ShortcutItem,
} from "./plugin-types";

export interface HostConfig {
  name: string;
  allowedPlugins?: string[]; // List of allowed plugin types
  maxPlugins?: number; // Maximum number of plugins (default: unlimited)
  showLevelMeters?: boolean; // Show level meters in right panel
  showLUFS?: boolean; // Show LUFS meter
  showVolumeControl?: boolean; // Show volume control
  showHelpBar?: boolean; // Show help bar with shortcuts
  menubarConfig?: MenubarConfig;
  accentColor?: string; // Accent color for UI (default: 'is-success' - green)
}

export interface HostCallbacks {
  onPluginAdd?: (plugin: IPlugin) => void;
  onPluginRemove?: (plugin: IPlugin) => void;
  onPluginSelect?: (plugin: IPlugin | null) => void;
  onVolumeChange?: (volume: number) => void;
  onPluginReorder?: (plugins: IPlugin[]) => void;
}

/**
 * Plugin Host
 * Manages a collection of plugins with unified UI
 */
export class PluginHost {
  private container: HTMLElement;
  private config: HostConfig;
  private callbacks: HostCallbacks;

  // UI components
  private menubar: PluginMenubar | null = null;
  private levelMeter: LevelMeter | null = null;

  // UI elements
  private hostingBar: HTMLElement | null = null;
  private helpBar: HTMLElement | null = null;
  private displayBox: HTMLElement | null = null;
  private displayLeft: HTMLElement | null = null;
  private displayRight: HTMLElement | null = null;
  private pluginSlotsContainer: HTMLElement | null = null;

  // State
  private plugins: IPlugin[] = [];
  private selectedPlugin: IPlugin | null = null;
  private volume: number = 1.0;
  private muted: boolean = false;
  private monitoringMode: "input" | "output" = "output";
  private helpBarVisible: boolean = true;

  // Drag-and-drop state
  private draggedPlugin: IPlugin | null = null;
  private draggedIndex: number = -1;

  constructor(
    container: HTMLElement,
    config: HostConfig,
    callbacks: HostCallbacks = {},
  ) {
    this.container = container;
    this.config = {
      allowedPlugins: config.allowedPlugins ?? [],
      maxPlugins: config.maxPlugins ?? Infinity,
      showLevelMeters: config.showLevelMeters ?? true,
      showLUFS: config.showLUFS ?? true,
      showVolumeControl: config.showVolumeControl ?? true,
      showHelpBar: config.showHelpBar ?? true,
      menubarConfig: config.menubarConfig ?? {},
      accentColor: config.accentColor ?? "is-success",
      ...config,
    };
    this.callbacks = callbacks;

    this.render();
  }

  /**
   * Render the host UI
   */
  private render(): void {
    this.container.innerHTML = `
      <div class="box has-background-dark p-0">
        <!-- Menubar -->
        <div class="host-menubar"></div>

        <!-- Hosting Bar -->
        <div class="level is-mobile p-3 has-background-grey-dark" style="border-bottom: 1px solid #404040;">
          <div class="level-left">
            <div class="level-item">
              <div class="buttons plugin-slots"></div>
            </div>
          </div>
          <div class="level-right">
            <div class="level-item">
              <button class="button is-small is-ghost has-text-light add-plugin-btn" title="Add Plugin">
                <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M8 2v12M2 8h12" stroke-linecap="round"/>
                </svg>
              </button>
            </div>
          </div>
        </div>

        <!-- Display Box -->
        <div class="is-flex" style="min-height: 400px;">
          <!-- Left: Active Plugin Display -->
          <div class="is-flex-grow-1 display-left has-background-black-ter p-4" style="min-height: 400px;">
            <div class="is-flex is-flex-direction-column is-align-items-center is-justify-content-center has-text-grey" style="height: 100%;">
              <svg width="48" height="48" viewBox="0 0 48 48" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="8" y="8" width="32" height="32" rx="4"/>
                <circle cx="24" cy="20" r="4"/>
                <path d="M16 32h16"/>
              </svg>
              <p class="mt-3">No plugin selected</p>
            </div>
          </div>

          <!-- Right: Meters & Controls -->
          <div class="is-flex is-flex-direction-column display-right p-4 has-background-grey-darker" style="border-left: 1px solid #404040;">
            ${this.renderRightPanel()}
          </div>
        </div>

        <!-- Help Bar -->
        ${this.config.showHelpBar ? this.renderHelpBar() : ""}

      </div>
    `;

    this.cacheElements();
    this.initializeComponents();
    this.attachEventListeners();
  }

  /**
   * Render right panel (meters at top, then vertical controls)
   * Always use vertical layout for controls
   */
  private renderRightPanel(): string {
    const pluginHasMeters =
      this.selectedPlugin?.metadata.hasBuiltInLevelMeters ?? false;

    if (pluginHasMeters) {
      // Vertical layout: LUFS, Input/Output toggle, Volume stacked (no host meters)
      return this.renderCompactControlsVertical();
    } else {
      // Vertical layout: meters at top, then LUFS, Input/Output toggle, Volume stacked
      return `
        ${this.config.showLevelMeters ? this.renderLevelMeters() : ""}
        ${this.config.showLUFS || this.config.showLevelMeters || this.config.showVolumeControl ? this.renderCompactControlsVertical() : ""}
      `;
    }
  }

  /**
   * Render compact controls row (LUFS + monitoring + volume in one row)
   */
  private renderCompactControlsRow(): string {
    return `
      <div class="is-flex is-align-items-stretch" >
        ${this.config.showLUFS ? this.renderLUFSMeter() : ""}
        ${this.config.showLevelMeters ? this.renderMonitoringToggle() : ""}
        ${this.config.showVolumeControl ? this.renderVolumeControl() : ""}
      </div>
    `;
  }

  /**
   * Render vertical controls layout (when plugin has built-in meters)
   * Stack LUFS, monitoring toggle, and volume vertically
   */
  private renderCompactControlsVertical(): string {
    return `
      <div class="is-flex is-flex-direction-column" >
        ${this.config.showLUFS ? this.renderLUFSMeterVertical() : ""}
        ${this.config.showLevelMeters ? this.renderMonitoringToggleVertical() : ""}
        ${this.config.showVolumeControl ? this.renderVolumeControl("vertical") : ""}
      </div>
    `;
  }

  /**
   * Render monitoring toggle (input/output) - compact version using Bulma buttons
   */
  private renderMonitoringToggle(): string {
    const accentColor = this.config.accentColor!;
    return `
      <div class="is-flex is-flex-direction-column is-align-items-center">
        <div class="has-text-centered has-text-weight-semibold is-size-7 mb-1 has-text-light">Monitor</div>
        <div class="buttons has-addons">
          <button class="button is-small ${this.monitoringMode === "input" ? `${accentColor} is-selected` : ""}" data-mode="input" title="Monitor Input">
            In
          </button>
          <button class="button is-small ${this.monitoringMode === "output" ? `${accentColor} is-selected` : ""}" data-mode="output" title="Monitor Output">
            Out
          </button>
        </div>
      </div>
    `;
  }

  /**
   * Render LUFS meter - compact version using Bulma tags
   */
  private renderLUFSMeter(): string {
    // Map button color classes to text color classes
    const textColorClass = this.config.accentColor!.replace("is-", "has-text-");

    return `
      <div class="box p-2 has-background-dark">
        <div class="has-text-centered has-text-weight-semibold is-size-7 mb-1 has-text-light">LUFS</div>
        <div class="is-flex is-flex-direction-column">
          <div class="tags has-addons mb-0">
            <span class="tag is-dark is-small" style="min-width: 1.5em;">M</span>
            <span class="tag is-dark is-small has-text-right ${textColorClass}" style="font-family: monospace; min-width: 3.5em;" data-lufs="momentary">-∞</span>
          </div>
          <div class="tags has-addons mb-0">
            <span class="tag is-dark is-small" style="min-width: 1.5em;">S</span>
            <span class="tag is-dark is-small has-text-right ${textColorClass}" style="font-family: monospace; min-width: 3.5em;" data-lufs="shortTerm">-∞</span>
          </div>
          <div class="tags has-addons mb-0">
            <span class="tag is-dark is-small" style="min-width: 1.5em;">I</span>
            <span class="tag is-dark is-small has-text-right ${textColorClass}" style="font-family: monospace; min-width: 3.5em;" data-lufs="integrated">-∞</span>
          </div>
        </div>
      </div>
    `;
  }

  /**
   * Render level meters
   */
  private renderLevelMeters(): string {
    const { outputChannels } = this.getChannelCounts();
    // Calculate width: each channel gets 20px + 10px spacing
    const channelWidth = 30;
    const spacing = 10;
    const canvasWidth =
      outputChannels * channelWidth + (outputChannels + 1) * spacing;
    const canvasHeight = 240;

    return `
      <div class="mb-3">
        <canvas class="level-meters-canvas" width="${canvasWidth}" height="${canvasHeight}" style="width: ${canvasWidth}px; height: ${canvasHeight}px; display: block;"></canvas>
      </div>
    `;
  }

  /**
   * Render volume control knob
   */
  private renderVolumeControl(
    layout: "compact" | "vertical" = "compact",
  ): string {
    const volumePercent = Math.round(this.volume * 100);
    const knobClass =
      layout === "compact" ? "volume-knob" : "volume-knob-vertical";
    // Map Bulma color classes to hex colors for SVG stroke
    const accentColorMap: Record<string, string> = {
      "is-success": "#48c78e",
      "is-info": "#3273dc",
      "is-primary": "#00d1b2",
      "is-warning": "#ffdd57",
      "is-danger": "#f14668",
    };
    const strokeColor = accentColorMap[this.config.accentColor!] || "#48c78e";

    return `
      <div class="box p-2 has-background-dark">
        <div class="has-text-weight-semibold ${layout === "compact" ? "is-size-7 mb-1" : "mb-2"} has-text-centered has-text-light">Volume</div>
        <div class="is-flex is-justify-content-center">
          <div class="${knobClass}" data-volume="${volumePercent}" style="cursor: pointer; position: relative; width: 80px; height: 80px;">
            <svg class="volume-knob-svg" viewBox="0 0 100 100" style="width: 100%; height: 100%;">
              <circle class="volume-track" cx="50" cy="50" r="40" fill="none" stroke="#404040" stroke-width="8" />
              <circle class="volume-fill" cx="50" cy="50" r="40" fill="none" stroke="${strokeColor}" stroke-width="8"
                stroke-dasharray="${(volumePercent / 100) * 251.2} 251.2"
                transform="rotate(-90 50 50)" />
              <text class="volume-value-svg" x="50" y="50" fill="white" font-size="20" font-weight="600" text-anchor="middle" dominant-baseline="central">${volumePercent}%</text>
            </svg>
          </div>
        </div>
      </div>
    `;
  }

  /**
   * Render LUFS meter - vertical layout using Bulma tags
   */
  private renderLUFSMeterVertical(): string {
    // Map button color classes to text color classes
    const textColorClass = this.config.accentColor!.replace("is-", "has-text-");

    return `
      <div class="box p-3 has-background-dark">
        <div class="has-text-centered has-text-weight-semibold mb-2 has-text-light">LUFS</div>
        <div class="is-flex is-flex-direction-column">
          <div class="tags has-addons mb-0">
            <span class="tag is-dark" style="min-width: 2em;">M</span>
            <span class="tag is-dark has-text-right ${textColorClass}" style="font-family: monospace; min-width: 4em;" data-lufs="momentary">-∞</span>
          </div>
          <div class="tags has-addons mb-0">
            <span class="tag is-dark" style="min-width: 2em;">S</span>
            <span class="tag is-dark has-text-right ${textColorClass}" style="font-family: monospace; min-width: 4em;" data-lufs="shortTerm">-∞</span>
          </div>
          <div class="tags has-addons mb-0">
            <span class="tag is-dark" style="min-width: 2em;">I</span>
            <span class="tag is-dark has-text-right ${textColorClass}" style="font-family: monospace; min-width: 4em;" data-lufs="integrated">-∞</span>
          </div>
        </div>
      </div>
    `;
  }

  /**
   * Render monitoring toggle - vertical layout using Bulma buttons
   */
  private renderMonitoringToggleVertical(): string {
    const accentColor = this.config.accentColor!;
    return `
      <div class="field is-flex is-flex-direction-column is-align-items-center">
        <label class="label is-small has-text-light has-text-centered">Monitor</label>
        <div class="buttons has-addons">
          <button class="button is-small ${this.monitoringMode === "input" ? `${accentColor} is-selected` : ""}" data-mode="input" title="Monitor Input">
            Input
          </button>
          <button class="button is-small ${this.monitoringMode === "output" ? `${accentColor} is-selected` : ""}" data-mode="output" title="Monitor Output">
            Output
          </button>
        </div>
      </div>
    `;
  }

  /**
   * Render help bar with shortcuts using Bulma tags
   */
  private renderHelpBar(): string {
    if (!this.helpBarVisible) return "";

    const shortcuts = this.getShortcuts();
    const shortcutItems = shortcuts
      .map(
        ({ key, description }) => `
      <div class="control">
        <div class="tags has-addons">
          <span class="tag is-info">${this.escapeHtml(key)}</span>
          <span class="tag is-dark">${this.escapeHtml(description)}</span>
        </div>
      </div>
    `,
      )
      .join("");

    return `
      <div class="notification is-dark p-3" style="position: relative; border-top: 1px solid #404040; margin: 0; border-radius: 0;">
        <div class="field is-grouped is-grouped-multiline">
          ${shortcutItems}
        </div>
        <button class="delete is-medium help-close" style="position: absolute; top: 12px; right: 12px;" title="Close help bar" aria-label="Close help bar"></button>
      </div>
    `;
  }

  /**
   * Get shortcuts for current context
   */
  private getShortcuts(): ShortcutItem[] {
    const shortcuts: ShortcutItem[] = [{ key: "?", description: "Help" }];

    // Add plugin-specific shortcuts first if a plugin is selected
    if (
      this.selectedPlugin &&
      typeof this.selectedPlugin.getShortcuts === "function"
    ) {
      const pluginShortcuts = this.selectedPlugin.getShortcuts();
      shortcuts.push(...pluginShortcuts);
    }

    // Add monitoring shortcuts if meters are shown
    if (this.config.showLevelMeters) {
      shortcuts.push(
        { key: "<", description: "Monitor input" },
        { key: ">", description: "Monitor output" },
      );
    }

    // Add volume shortcuts if volume control is shown
    if (this.config.showVolumeControl) {
      shortcuts.push(
        { key: "↑", description: "Volume up" },
        { key: "↓", description: "Volume down" },
      );
    }

    return shortcuts;
  }

  /**
   * Refresh help bar with current shortcuts
   */
  private refreshHelpBar(): void {
    if (!this.config.showHelpBar || !this.helpBar) return;

    const shortcuts = this.getShortcuts();
    const shortcutsContainer = this.helpBar.querySelector(".field.is-grouped");

    if (shortcutsContainer) {
      const shortcutItems = shortcuts
        .map(
          ({ key, description }) => `
        <div class="control">
          <div class="tags has-addons">
            <span class="tag is-dark">${this.escapeHtml(key)}</span>
            <span class="tag is-light">${this.escapeHtml(description)}</span>
          </div>
        </div>
      `,
        )
        .join("");

      shortcutsContainer.innerHTML = shortcutItems;
    }
  }

  /**
   * Escape HTML to prevent XSS
   */
  private escapeHtml(text: string): string {
    const div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
  }

  /**
   * Cache DOM elements
   */
  private cacheElements(): void {
    this.hostingBar = this.container.querySelector(".hosting-bar");
    this.helpBar = this.container.querySelector(".notification");
    this.displayBox = this.container.querySelector(".display-box");
    this.displayLeft = this.container.querySelector(".display-left");
    this.displayRight = this.container.querySelector(".display-right");
    this.pluginSlotsContainer = this.container.querySelector(".plugin-slots");
  }

  /**
   * Initialize components
   */
  private initializeComponents(): void {
    // Initialize menubar
    const menubarContainer = this.container.querySelector(
      ".host-menubar",
    ) as HTMLElement;
    if (menubarContainer) {
      const menubarCallbacks: PluginMenubarCallbacks = {
        onMatrix: () => this.showMatrixDialog(),
        onMute: (muted) => this.setMuted(muted),
      };
      this.menubar = new PluginMenubar(
        menubarContainer,
        this.config.name,
        this.config.menubarConfig,
        menubarCallbacks,
      );
    }

    // Initialize level meters
    if (this.config.showLevelMeters) {
      const canvas = this.container.querySelector(
        ".level-meters-canvas",
      ) as HTMLCanvasElement;
      if (canvas) {
        // Make canvas responsive to container size
        // Use setTimeout to ensure container has rendered and has dimensions
        setTimeout(() => {
          const container = canvas.parentElement as HTMLElement;
          if (container) {
            const width = container.clientWidth || 200;
            const height = container.clientHeight || 300;

            // Set canvas resolution (internal size)
            canvas.width = width;
            canvas.height = height;

            // Force level meter to redraw
            if (this.levelMeter) {
              this.levelMeter.resize();
            }
          }
        }, 0);

        // Get initial channel configuration based on plugin chain
        const { inputChannels, outputChannels } = this.getChannelCounts();
        const channels =
          this.monitoringMode === "input" ? inputChannels : outputChannels;
        const labels = this.generateChannelLabels(channels);

        this.levelMeter = new LevelMeter({
          canvas,
          channels,
          channelLabels: labels,
        });
      }
    }
  }

  /**
   * Attach event listeners
   */
  private attachEventListeners(): void {
    // Add plugin button
    const addButton = this.container.querySelector(
      ".add-plugin-btn",
    ) as HTMLButtonElement;
    if (addButton) {
      addButton.addEventListener("click", () => {
        this.showPluginSelector();
      });
    }

    // Help bar close button
    if (this.helpBar) {
      const closeButton = this.helpBar.querySelector(
        ".help-close",
      ) as HTMLButtonElement;
      if (closeButton) {
        closeButton.addEventListener("click", () => this.toggleHelpBar());
      }
    }

    this.attachRightPanelEventListeners();

    // Keyboard shortcuts
    document.addEventListener("keydown", this.handleKeydown);
  }

  /**
   * Attach event listeners for right panel controls
   */
  private attachRightPanelEventListeners(): void {
    // Volume knob - both compact and vertical versions
    const volumeKnobs = this.container.querySelectorAll(
      ".volume-knob, .volume-knob-vertical",
    );
    volumeKnobs.forEach((volumeKnob) => {
      volumeKnob.addEventListener("wheel", (e) => {
        e.preventDefault();
        const delta = -Math.sign((e as WheelEvent).deltaY) * 0.05;
        this.setVolume(Math.max(0, Math.min(1, this.volume + delta)));
      });

      // Click to adjust
      volumeKnob.addEventListener("click", (e) => {
        const rect = (volumeKnob as HTMLElement).getBoundingClientRect();
        const centerY = rect.top + rect.height / 2;
        const clickY = (e as MouseEvent).clientY;
        const delta = (centerY - clickY) / rect.height;
        this.setVolume(Math.max(0, Math.min(1, this.volume + delta * 0.5)));
      });
    });

    // Monitoring toggle buttons - Bulma buttons with data-mode attribute
    const monitoringButtons =
      this.container.querySelectorAll("button[data-mode]");
    monitoringButtons.forEach((button) => {
      button.addEventListener("click", (e) => {
        const mode = (e.target as HTMLElement).dataset.mode as
          | "input"
          | "output";
        this.setMonitoringMode(mode);
      });
    });
  }

  /**
   * Reinitialize right panel components after re-rendering
   */
  private reinitializeRightPanelComponents(): void {
    // Reinitialize level meters if they're shown
    const pluginHasMeters =
      this.selectedPlugin?.metadata.hasBuiltInLevelMeters ?? false;

    if (!pluginHasMeters && this.config.showLevelMeters) {
      const canvas = this.container.querySelector(
        ".level-meters-canvas",
      ) as HTMLCanvasElement;
      if (canvas) {
        // Destroy old level meter
        if (this.levelMeter) {
          this.levelMeter.destroy();
          this.levelMeter = null;
        }

        // Create new level meter
        setTimeout(() => {
          const container = canvas.parentElement as HTMLElement;
          if (container) {
            const width = container.clientWidth || 200;
            const height = container.clientHeight || 300;

            canvas.width = width;
            canvas.height = height;

            // Get channel configuration based on plugin chain
            const { inputChannels, outputChannels } = this.getChannelCounts();
            const channels =
              this.monitoringMode === "input" ? inputChannels : outputChannels;
            const labels = this.generateChannelLabels(channels);

            this.levelMeter = new LevelMeter({
              canvas,
              channels,
              channelLabels: labels,
            });
          }
        }, 0);
      }
    }

    // Reattach event listeners
    this.attachRightPanelEventListeners();
  }

  /**
   * Handle keyboard shortcuts
   */
  private handleKeydown = (e: KeyboardEvent): void => {
    // Check if target is an input element
    const target = e.target as HTMLElement;
    if (
      target.tagName === "INPUT" ||
      target.tagName === "TEXTAREA" ||
      target.tagName === "SELECT"
    ) {
      return;
    }

    switch (e.key) {
      case "<":
      case "ArrowLeft":
        e.preventDefault();
        this.setMonitoringMode("input");
        break;
      case ">":
      case "ArrowRight":
        e.preventDefault();
        this.setMonitoringMode("output");
        break;

      // Volume controls
      case "+":
      case "ArrowUp":
      case "AudioVolumeUp":
        e.preventDefault();
        this.setVolume(Math.min(1, this.volume + 0.05));
        break;
      case "-":
      case "ArrowDown":
      case "AudioVolumeDown":
        e.preventDefault();
        this.setVolume(Math.max(0, this.volume - 0.05));
        break;
    }
  };

  /**
   * Add a plugin
   */
  addPlugin(plugin: IPlugin): void {
    // Check if we can add more plugins
    if (this.plugins.length >= this.config.maxPlugins!) {
      console.warn("[PluginHost] Maximum number of plugins reached");
      return;
    }

    // Check if plugin type is allowed
    if (
      this.config.allowedPlugins!.length > 0 &&
      !this.config.allowedPlugins!.includes(plugin.metadata.category)
    ) {
      console.warn(
        "[PluginHost] Plugin type not allowed:",
        plugin.metadata.category,
      );
      return;
    }

    this.plugins.push(plugin);
    this.renderPluginSlot(plugin);

    // Update level meters if plugin changes channel count
    this.updateLevelMeterChannels();

    // Callback
    if (this.callbacks.onPluginAdd) {
      this.callbacks.onPluginAdd(plugin);
    }

    // Auto-select if it's the first plugin
    if (this.plugins.length === 1) {
      this.selectPlugin(plugin);
    }
  }

  /**
   * Remove a plugin
   */
  removePlugin(plugin: IPlugin): void {
    const index = this.plugins.indexOf(plugin);
    if (index === -1) return;

    this.plugins.splice(index, 1);

    // If this was the selected plugin, clear selection
    if (this.selectedPlugin === plugin) {
      this.selectPlugin(null);
    }

    // Re-render slots
    this.renderAllPluginSlots();

    // Update level meters if plugin changes channel count
    this.updateLevelMeterChannels();

    // Cleanup plugin
    plugin.destroy();

    // Callback
    if (this.callbacks.onPluginRemove) {
      this.callbacks.onPluginRemove(plugin);
    }
  }

  /**
   * Select a plugin
   */
  selectPlugin(plugin: IPlugin | null): void {
    this.selectedPlugin = plugin;

    // Update slot highlighting
    this.updateSlotSelection();

    // Render plugin in display area
    if (this.displayLeft) {
      if (plugin) {
        this.displayLeft.innerHTML =
          '<div class="active-plugin-container"></div>';
        const pluginContainer = this.displayLeft.querySelector(
          ".active-plugin-container",
        ) as HTMLElement;
        plugin.initialize(pluginContainer, {
          standalone: false,
          showMenubar: false,
        });
      } else {
        this.displayLeft.innerHTML = `
          <div class="is-flex is-flex-direction-column is-align-items-center is-justify-content-center has-text-grey" >
            <svg width="48" height="48" viewBox="0 0 48 48" fill="none" stroke="currentColor" stroke-width="2" >
              <rect x="8" y="8" width="32" height="32" rx="4"/>
              <circle cx="24" cy="20" r="4"/>
              <path d="M16 32h16"/>
            </svg>
            <p>No plugin selected</p>
          </div>
        `;
      }
    }

    // Re-render right panel based on whether plugin has built-in meters
    if (this.displayRight) {
      this.displayRight.innerHTML = this.renderRightPanel();
      this.reinitializeRightPanelComponents();
    }

    // Refresh help bar with new plugin shortcuts
    this.refreshHelpBar();

    // Callback
    if (this.callbacks.onPluginSelect) {
      this.callbacks.onPluginSelect(plugin);
    }
  }

  /**
   * Render a single plugin slot
   */
  private renderPluginSlot(plugin: IPlugin): void {
    if (!this.pluginSlotsContainer) {
      console.error("[PluginHost] pluginSlotsContainer is null!");
      return;
    }

    const slot = document.createElement("button");
    slot.className = "button is-small";
    slot.dataset.pluginId = plugin.metadata.id;
    slot.draggable = true;
    slot.title = plugin.metadata.name;

    slot.innerHTML = `
      <span>${plugin.metadata.name}</span>
      <span class="icon is-small plugin-slot-remove" title="Remove">
        <svg width="12" height="12" viewBox="0 0 12 12" fill="currentColor">
          <path d="M2 2l8 8M10 2l-8 8" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
        </svg>
      </span>
    `;

    // Click to select
    slot.addEventListener("click", (e) => {
      if (!(e.target as HTMLElement).closest(".plugin-slot-remove")) {
        this.selectPlugin(plugin);
      }
    });

    // Remove button
    const removeBtn = slot.querySelector(".plugin-slot-remove") as HTMLElement;
    removeBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      this.removePlugin(plugin);
    });

    // Drag-and-drop event listeners
    slot.addEventListener("dragstart", (e) => this.handleDragStart(e, plugin));
    slot.addEventListener("dragend", (e) => this.handleDragEnd(e));
    slot.addEventListener("dragover", (e) => this.handleDragOver(e));
    slot.addEventListener("drop", (e) => this.handleDrop(e, plugin));
    slot.addEventListener("dragenter", (e) => this.handleDragEnter(e));
    slot.addEventListener("dragleave", (e) => this.handleDragLeave(e));

    this.pluginSlotsContainer.appendChild(slot);
  }

  /**
   * Render all plugin slots
   */
  private renderAllPluginSlots(): void {
    if (!this.pluginSlotsContainer) return;

    this.pluginSlotsContainer.innerHTML = "";
    this.plugins.forEach((plugin) => this.renderPluginSlot(plugin));
    this.updateSlotSelection();
  }

  /**
   * Update slot selection highlighting
   */
  private updateSlotSelection(): void {
    const slots = this.container.querySelectorAll(".buttons > button");
    const accentColor = this.config.accentColor!;

    slots.forEach((slot) => {
      const pluginId = (slot as HTMLElement).dataset.pluginId;
      const isSelected = !!(
        this.selectedPlugin && this.selectedPlugin.metadata.id === pluginId
      );

      // Remove all possible color classes
      slot.classList.remove(
        "is-info",
        "is-success",
        "is-primary",
        "is-warning",
        "is-danger",
      );

      if (isSelected) {
        slot.classList.add(accentColor);
      }
      slot.classList.toggle("is-selected", isSelected);
    });
  }

  /**
   * Show plugin selector dialog
   */
  private showPluginSelector(): void {
    // Available plugins
    const availablePlugins = [
      {
        id: "eq",
        name: "EQ",
        category: "eq",
        description: "Parametric Equalizer",
        icon: "🎚️",
      },
      {
        id: "compressor",
        name: "Compressor",
        category: "dynamics",
        description: "Dynamic Range Compressor",
        icon: "🔊",
      },
      {
        id: "limiter",
        name: "Limiter",
        category: "dynamics",
        description: "Peak Limiter",
        icon: "🛡️",
      },
      {
        id: "upmixer",
        name: "Upmixer",
        category: "spatial",
        description: "Stereo to 5.1 Upmixer",
        icon: "🔉",
      },
      {
        id: "spectrum",
        name: "Spectrum",
        category: "analyzer",
        description: "Frequency Spectrum Analyzer",
        icon: "📊",
      },
    ];

    // Filter by allowed plugins if specified
    const plugins =
      this.config.allowedPlugins!.length > 0
        ? availablePlugins.filter((p) =>
            this.config.allowedPlugins!.includes(p.category),
          )
        : availablePlugins;

    // Create Bulma modal
    const dialog = document.createElement("div");
    dialog.className = "modal is-active";
    dialog.style.zIndex = "9999";
    dialog.innerHTML = `
      <div class="modal-background"></div>
      <div class="modal-card">
        <header class="modal-card-head">
          <p class="modal-card-title">Add Plugin</p>
          <button class="delete plugin-selector-close" aria-label="close"></button>
        </header>
        <section class="modal-card-body">
          <div class="columns is-multiline">
            ${plugins
              .map(
                (plugin) => `
              <div class="column is-half">
                <button class="button is-fullwidth is-justify-content-flex-start plugin-selector-item" data-plugin-id="${plugin.id}" style="height: auto; padding: 1rem;">
                  <span class="icon is-large mr-3">${plugin.icon}</span>
                  <div class="has-text-left">
                    <div class="has-text-weight-semibold">${plugin.name}</div>
                    <div class="is-size-7 has-text-grey">${plugin.description}</div>
                  </div>
                </button>
              </div>
            `,
              )
              .join("")}
          </div>
        </section>
      </div>
    `;

    document.body.appendChild(dialog);

    // Close handler
    const closeDialog = () => {
      dialog.remove();
    };

    // Close button
    const closeBtn = dialog.querySelector(
      ".plugin-selector-close",
    ) as HTMLButtonElement;
    closeBtn.addEventListener("click", closeDialog);

    // Modal background click to close
    const modalBg = dialog.querySelector(".modal-background") as HTMLElement;
    modalBg.addEventListener("click", closeDialog);

    // Plugin selection
    const pluginItems = dialog.querySelectorAll(".plugin-selector-item");
    pluginItems.forEach((item) => {
      item.addEventListener("click", () => {
        const pluginId = (item as HTMLElement).dataset.pluginId!;
        this.createPluginById(pluginId);
        closeDialog();
      });
    });
  }

  /**
   * Create plugin by ID
   */
  private createPluginById(pluginId: string): void {
    // Dynamically import and create plugin based on ID
    if (pluginId === "eq") {
      import("./plugin-eq")
        .then(({ EQPlugin }) => {
          const plugin = new EQPlugin();
          this.addPlugin(plugin);
        })
        .catch((err) => {
          console.error("[PluginHost] Failed to import EQ plugin:", err);
        });
    } else if (pluginId === "compressor") {
      import("./plugin-compressor")
        .then(({ CompressorPlugin }) => {
          const plugin = new CompressorPlugin();
          this.addPlugin(plugin);
        })
        .catch((err) => {
          console.error(
            "[PluginHost] Failed to import Compressor plugin:",
            err,
          );
        });
    } else if (pluginId === "limiter") {
      import("./plugin-limiter")
        .then(({ LimiterPlugin }) => {
          const plugin = new LimiterPlugin();
          this.addPlugin(plugin);
        })
        .catch((err) => {
          console.error("[PluginHost] Failed to import Limiter plugin:", err);
        });
    } else if (pluginId === "upmixer") {
      import("./plugin-upmixer")
        .then(({ UpmixerPlugin }) => {
          const plugin = new UpmixerPlugin();
          this.addPlugin(plugin);
        })
        .catch((err) => {
          console.error("[PluginHost] Failed to import Upmixer plugin:", err);
        });
    } else if (pluginId === "spectrum") {
      import("./plugin-spectrum")
        .then(({ SpectrumPlugin }) => {
          const plugin = new SpectrumPlugin();
          this.addPlugin(plugin);
        })
        .catch((err) => {
          console.error("[PluginHost] Failed to import Spectrum plugin:", err);
        });
    } else {
      console.error("[PluginHost] Unknown plugin ID:", pluginId);
    }
  }

  /**
   * Show matrix routing dialog
   */
  private showMatrixDialog(): void {
    // TODO: Implement matrix routing dialog
    console.log("[PluginHost] Show matrix dialog");
  }

  /**
   * Update level meters
   */
  updateLevelMeters(data: LevelMeterData): void {
    if (this.levelMeter) {
      this.levelMeter.update(data);
    }
  }

  /**
   * Update LUFS values
   */
  updateLUFS(data: LUFSMeterData): void {
    const momentaryEl = this.container.querySelector(
      '[data-lufs="momentary"]',
    ) as HTMLElement;
    const shortTermEl = this.container.querySelector(
      '[data-lufs="shortTerm"]',
    ) as HTMLElement;
    const integratedEl = this.container.querySelector(
      '[data-lufs="integrated"]',
    ) as HTMLElement;

    if (momentaryEl) momentaryEl.textContent = data.momentary.toFixed(1);
    if (shortTermEl) shortTermEl.textContent = data.shortTerm.toFixed(1);
    if (integratedEl) integratedEl.textContent = data.integrated.toFixed(1);
  }

  /**
   * Set volume
   */
  setVolume(volume: number): void {
    this.volume = Math.max(0, Math.min(1, volume));

    // Update UI - both compact and vertical versions
    const volumeValueSvg = this.container.querySelectorAll(".volume-value-svg");
    const volumeValueVertical = this.container.querySelector(
      ".volume-value-vertical",
    ) as HTMLElement;
    const volumeKnobCompact = this.container.querySelector(
      ".volume-knob",
    ) as HTMLElement;
    const volumeKnobVertical = this.container.querySelector(
      ".volume-knob-vertical",
    ) as HTMLElement;
    const volumeFills = this.container.querySelectorAll(".volume-fill");

    const volumePercent = Math.round(this.volume * 100);

    volumeValueSvg.forEach((el) => {
      el.textContent = String(volumePercent);
    });
    if (volumeValueVertical)
      volumeValueVertical.textContent = String(volumePercent);
    if (volumeKnobCompact)
      volumeKnobCompact.dataset.volume = String(volumePercent);
    if (volumeKnobVertical)
      volumeKnobVertical.dataset.volume = String(volumePercent);

    volumeFills.forEach((volumeFill) => {
      const circumference = 251.2;
      volumeFill.setAttribute(
        "stroke-dasharray",
        `${(volumePercent / 100) * circumference} ${circumference}`,
      );
    });

    // Callback
    if (this.callbacks.onVolumeChange) {
      this.callbacks.onVolumeChange(this.volume);
    }
  }

  /**
   * Get volume
   */
  getVolume(): number {
    return this.volume;
  }

  /**
   * Set muted state
   */
  setMuted(muted: boolean): void {
    this.muted = muted;
    if (this.menubar) {
      this.menubar.setMuted(muted);
    }
  }

  /**
   * Get all plugins
   */
  getPlugins(): IPlugin[] {
    return [...this.plugins];
  }

  /**
   * Get selected plugin
   */
  getSelectedPlugin(): IPlugin | null {
    return this.selectedPlugin;
  }

  /**
   * Set monitoring mode (input/output)
   */
  setMonitoringMode(mode: "input" | "output"): void {
    this.monitoringMode = mode;

    // Update button states - Bulma buttons with accent color and is-selected classes
    const buttons = this.container.querySelectorAll("button[data-mode]");
    const accentColor = this.config.accentColor!;

    buttons.forEach((button) => {
      const btnMode = (button as HTMLElement).dataset.mode;
      const isActive = btnMode === mode;

      // Remove all possible color classes
      button.classList.remove(
        "is-info",
        "is-success",
        "is-primary",
        "is-warning",
        "is-danger",
      );

      if (isActive) {
        button.classList.add(accentColor);
      }
      button.classList.toggle("is-selected", isActive);
    });

    // Reconfigure level meters with appropriate channel count
    this.updateLevelMeterChannels();

    console.log("[PluginHost] Monitoring mode:", mode);
  }

  /**
   * Update level meter channel configuration based on monitoring mode
   */
  private updateLevelMeterChannels(): void {
    if (!this.levelMeter) return;

    const { inputChannels, outputChannels } = this.getChannelCounts();
    const channels =
      this.monitoringMode === "input" ? inputChannels : outputChannels;
    const labels = this.generateChannelLabels(channels);

    this.levelMeter.reconfigure(channels, labels);
  }

  /**
   * Get input and output channel counts based on plugin chain
   */
  private getChannelCounts(): {
    inputChannels: number;
    outputChannels: number;
  } {
    // Default: start with 2 channels (stereo input)
    let inputChannels = 2;
    let outputChannels = 2;

    // If we have plugins, check if any change channel count
    // For now, check if there's an upmixer plugin
    const hasUpmixer = this.plugins.some(
      (p) => p.metadata.category === "spatial",
    );

    if (hasUpmixer) {
      // Upmixer: 2ch input -> 5ch or 6ch output
      inputChannels = 2;
      outputChannels = 6; // L, R, C, LFE, SL, SR
    } else {
      // No channel-changing plugins: same in/out
      inputChannels = 2;
      outputChannels = 2;
    }

    return { inputChannels, outputChannels };
  }

  /**
   * Generate channel labels based on count
   */
  private generateChannelLabels(count: number): string[] {
    if (count === 2) {
      return ["L", "R"];
    } else if (count === 6) {
      // 5.1
      return ["L", "R", "C", "LFE", "SL", "SR"];
    } else if (count === 8) {
      // 7.1
      return ["L", "R", "C", "LFE", "SL", "SR", "SBL", "SBR"];
    } else if (count === 12) {
      // 5.1.4
      return ["L", "R", "C", "LFE", "SL", "SR", "TFL", "TFR", "TBL", "TBR"];
    } else if (count === 14) {
      // 9.1.4
      return [
        "L",
        "R",
        "C",
        "LFE",
        "SL",
        "SR",
        "SBL",
        "SBR",
        "TFL",
        "TFR",
        "TBL",
        "TBR",
      ];
    } else if (count === 16) {
      // 9.1.6
      return [
        "L",
        "R",
        "C",
        "LFE",
        "SL",
        "SR",
        "SBL",
        "SBR",
        "TFL",
        "TFR",
        "TC",
        "TBL",
        "TBR",
        "TBC",
      ];
    } else {
      return Array.from({ length: count }, (_, i) => `${i + 1}`);
    }
  }

  /**
   * Toggle help bar visibility
   */
  toggleHelpBar(): void {
    this.helpBarVisible = !this.helpBarVisible;

    if (this.helpBar) {
      if (this.helpBarVisible) {
        this.helpBar.style.display = "flex";
      } else {
        this.helpBar.style.display = "none";
      }
    }
  }

  /**
   * Show help bar
   */
  showHelpBar(): void {
    this.helpBarVisible = true;
    if (this.helpBar) {
      this.helpBar.style.display = "flex";
    }
  }

  /**
   * Hide help bar
   */
  hideHelpBar(): void {
    this.helpBarVisible = false;
    if (this.helpBar) {
      this.helpBar.style.display = "none";
    }
  }

  /**
   * Handle drag start
   */
  private handleDragStart(e: DragEvent, plugin: IPlugin): void {
    this.draggedPlugin = plugin;
    this.draggedIndex = this.plugins.indexOf(plugin);

    const target = e.currentTarget as HTMLElement;
    target.classList.add("is-ghost");

    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/html", target.innerHTML);
    }
  }

  /**
   * Handle drag end
   */
  private handleDragEnd(e: DragEvent): void {
    const target = e.currentTarget as HTMLElement;
    target.classList.remove("is-ghost");

    // Remove drag-over classes from all slots
    const slots = this.container.querySelectorAll(".buttons > button");
    slots.forEach((slot) => slot.classList.remove("is-hovered"));

    this.draggedPlugin = null;
    this.draggedIndex = -1;
  }

  /**
   * Handle drag over
   */
  private handleDragOver(e: DragEvent): void {
    e.preventDefault();
    if (e.dataTransfer) {
      e.dataTransfer.dropEffect = "move";
    }
  }

  /**
   * Handle drag enter
   */
  private handleDragEnter(e: DragEvent): void {
    const target = e.currentTarget as HTMLElement;
    target.classList.add("is-hovered");
  }

  /**
   * Handle drag leave
   */
  private handleDragLeave(e: DragEvent): void {
    const target = e.currentTarget as HTMLElement;
    target.classList.remove("is-hovered");
  }

  /**
   * Handle drop
   */
  private handleDrop(e: DragEvent, targetPlugin: IPlugin): void {
    e.preventDefault();
    e.stopPropagation();

    const target = e.currentTarget as HTMLElement;
    target.classList.remove("is-hovered");

    if (!this.draggedPlugin || this.draggedPlugin === targetPlugin) {
      return;
    }

    // Find target index
    const targetIndex = this.plugins.indexOf(targetPlugin);

    if (targetIndex === -1 || this.draggedIndex === -1) {
      return;
    }

    // Reorder plugins array
    const newPlugins = [...this.plugins];
    newPlugins.splice(this.draggedIndex, 1);
    newPlugins.splice(targetIndex, 0, this.draggedPlugin);

    this.plugins = newPlugins;

    // Re-render all slots
    this.renderAllPluginSlots();

    // Trigger callback for config regeneration
    if (this.callbacks.onPluginReorder) {
      this.callbacks.onPluginReorder([...this.plugins]);
    }

    console.log(
      "[PluginHost] Plugin reordered:",
      this.plugins.map((p) => p.metadata.name),
    );
  }

  /**
   * Destroy the host
   */
  destroy(): void {
    // Remove keyboard listener
    document.removeEventListener("keydown", this.handleKeydown);

    // Destroy all plugins
    this.plugins.forEach((plugin) => plugin.destroy());
    this.plugins = [];

    // Destroy components
    if (this.menubar) {
      this.menubar.destroy();
      this.menubar = null;
    }

    if (this.levelMeter) {
      this.levelMeter.destroy();
      this.levelMeter = null;
    }

    // Clear container
    this.container.innerHTML = "";
    this.container.classList.remove("is-flex", "is-flex-direction-column");
  }
}
