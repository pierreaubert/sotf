// Base Plugin Implementation
// Provides common functionality for all plugins

import type {
  IPlugin,
  PluginMetadata,
  PluginState,
  PluginConfig,
} from "./plugin-types";

import { PluginEvent } from "./plugin-types";

/**
 * Base plugin class
 * Implements common plugin functionality
 */
export abstract class BasePlugin implements IPlugin {
  // Metadata
  public abstract readonly metadata: PluginMetadata;

  // Container
  protected container: HTMLElement | null = null;
  protected config: PluginConfig = {};

  // State
  protected state: PluginState = {
    enabled: true,
    bypassed: false,
    parameters: {},
  };

  // Event listeners
  private eventListeners: Map<string, Set<(...args: any[]) => void>> =
    new Map();

  // Keyboard control
  protected selectedParameterIndex: number = -1;
  protected parameterOrder: string[] = [];
  protected parameterLabels: Record<string, string> = {}; // Maps param name to display label
  protected parameterKeys: Record<string, string> = {}; // Maps param name to assigned key
  protected keyToParamIndex: Record<string, number> = {}; // Maps key to param index
  protected keyboardHandler: ((e: KeyboardEvent) => void) | null = null;

  /**
   * Initialize the plugin
   */
  initialize(container: HTMLElement, config: PluginConfig = {}): void {
    this.container = container;
    this.config = config;

    // Apply initial state if provided
    if (config.initialState) {
      this.setState(config.initialState);
    }

    // Render the plugin
    this.render(config.standalone ?? true);

    // Setup keyboard controls
    this.setupKeyboardControls();
  }

  /**
   * Destroy the plugin
   */
  destroy(): void {
    // Remove keyboard handler
    if (this.keyboardHandler) {
      document.removeEventListener("keydown", this.keyboardHandler);
      this.keyboardHandler = null;
    }

    if (this.container) {
      this.container.innerHTML = "";
      this.container = null;
    }
    this.eventListeners.clear();
  }

  /**
   * Get current state
   */
  getState(): PluginState {
    return { ...this.state };
  }

  /**
   * Set state (partial update)
   */
  setState(newState: Partial<PluginState>): void {
    const oldState = { ...this.state };
    this.state = { ...this.state, ...newState };

    // Notify config callback
    if (this.config.onStateChange) {
      this.config.onStateChange(this.state);
    }

    // Emit event
    this.emit(PluginEvent.StateChanged as string, this.state, oldState);

    // Handle specific state changes
    if (
      newState.bypassed !== undefined &&
      newState.bypassed !== oldState.bypassed
    ) {
      this.handleBypassChange(newState.bypassed);
    }

    if (newState.parameters !== undefined) {
      this.handleParameterChange(newState.parameters, oldState.parameters);
    }
  }

  /**
   * Render the plugin UI
   */
  abstract render(standalone: boolean): void;

  /**
   * Handle bypass state change
   */
  protected handleBypassChange(bypassed: boolean): void {
    if (this.config.onBypass) {
      this.config.onBypass(bypassed);
    }
    this.emit(PluginEvent.Bypassed as string, bypassed);
  }

  /**
   * Handle parameter change
   */
  protected handleParameterChange(
    newParams: Record<string, any>,
    oldParams: Record<string, any>,
  ): void {
    // Find changed parameters
    const changed: Record<string, { old: any; new: any }> = {};
    for (const key in newParams) {
      if (newParams[key] !== oldParams[key]) {
        changed[key] = { old: oldParams[key], new: newParams[key] };
      }
    }

    if (Object.keys(changed).length > 0) {
      this.emit(PluginEvent.ParameterChanged as string, changed);
    }
  }

  /**
   * Register event listener
   */
  on(event: string, callback: (...args: any[]) => void): void {
    if (!this.eventListeners.has(event)) {
      this.eventListeners.set(event, new Set());
    }
    this.eventListeners.get(event)!.add(callback);
  }

  /**
   * Unregister event listener
   */
  off(event: string, callback: (...args: any[]) => void): void {
    const listeners = this.eventListeners.get(event);
    if (listeners) {
      listeners.delete(callback);
    }
  }

  /**
   * Emit event
   */
  emit(event: string, ...args: any[]): void {
    const listeners = this.eventListeners.get(event);
    if (listeners) {
      listeners.forEach((callback) => {
        try {
          callback(...args);
        } catch (error) {
          console.error(
            `[Plugin ${this.metadata.name}] Error in event listener for ${event}:`,
            error,
          );
        }
      });
    }
  }

  /**
   * Update a single parameter
   */
  protected updateParameter(key: string, value: any): void {
    const newParameters = { ...this.state.parameters, [key]: value };
    this.setState({ parameters: newParameters });
  }

  /**
   * Get a single parameter value
   */
  protected getParameter<T>(key: string, defaultValue?: T): T | undefined {
    return (this.state.parameters[key] as T) ?? defaultValue;
  }

  /**
   * Check if plugin is bypassed
   */
  isBypassed(): boolean {
    return this.state.bypassed;
  }

  /**
   * Check if plugin is enabled
   */
  isEnabled(): boolean {
    return this.state.enabled;
  }

  /**
   * Set bypass state
   */
  setBypass(bypassed: boolean): void {
    this.setState({ bypassed });
  }

  /**
   * Toggle bypass state
   */
  toggleBypass(): void {
    this.setBypass(!this.state.bypassed);
  }

  /**
   * Assign keyboard keys to parameters intelligently
   * - First tries first letter of each parameter label
   * - On collision, tries next letters until unique key found
   */
  protected assignParameterKeys(): void {
    const usedKeys = new Set<string>();
    this.parameterKeys = {};
    this.keyToParamIndex = {};

    this.parameterOrder.forEach((paramName, index) => {
      const label = this.parameterLabels[paramName] || paramName;
      let assignedKey: string | null = null;

      // Try each letter in the label
      for (let i = 0; i < label.length; i++) {
        const char = label[i].toLowerCase();
        // Only use letters a-z
        if (char >= "a" && char <= "z" && !usedKeys.has(char)) {
          assignedKey = char;
          usedKeys.add(char);
          break;
        }
      }

      if (assignedKey) {
        this.parameterKeys[paramName] = assignedKey;
        this.keyToParamIndex[assignedKey] = index;
      }
    });
  }

  /**
   * Get formatted label with keyboard shortcut indicator
   * e.g., "Ratio" -> "[R]atio", "Release" -> "R[e]lease"
   */
  protected getFormattedLabel(paramName: string): string {
    const label = this.parameterLabels[paramName] || paramName;
    const key = this.parameterKeys[paramName];

    if (!key) return label;

    // Find the position of the key character
    const keyIndex = label.toLowerCase().indexOf(key.toLowerCase());
    if (keyIndex === -1) return label;

    // Insert brackets around the key character
    return (
      label.substring(0, keyIndex) +
      "[" +
      label[keyIndex] +
      "]" +
      label.substring(keyIndex + 1)
    );
  }

  /**
   * Setup unified keyboard controls
   * - 1-9: Select parameter by index
   * - a-z: Select parameter by assigned letter
   * - Tab: Cycle to next parameter
   * - Shift+Tab: Cycle to previous parameter
   * - Esc: Clear selection
   * - Shift+Up: Increase selected parameter
   * - Shift+Down: Decrease selected parameter
   */
  protected setupKeyboardControls(): void {
    // Assign keys to parameters
    this.assignParameterKeys();

    this.keyboardHandler = (e: KeyboardEvent) => {
      // Ignore if typing in input
      const target = e.target as HTMLElement;
      if (
        target.tagName === "INPUT" ||
        target.tagName === "TEXTAREA" ||
        target.tagName === "SELECT"
      ) {
        return;
      }

      // Letter keys - select parameter by assigned letter
      const lowerKey = e.key.toLowerCase();
      if (
        lowerKey.length === 1 &&
        lowerKey >= "a" &&
        lowerKey <= "z" &&
        this.keyToParamIndex[lowerKey] !== undefined
      ) {
        e.preventDefault();
        this.selectParameter(this.keyToParamIndex[lowerKey]);
        return;
      }

      // 1-9 - select parameter by index
      const numKey = parseInt(e.key, 10);
      if (numKey >= 1 && numKey <= 9 && numKey <= this.parameterOrder.length) {
        e.preventDefault();
        this.selectParameter(numKey - 1);
        return;
      }

      // Tab - cycle to next parameter
      if (e.key === "Tab" && !e.shiftKey) {
        e.preventDefault();
        if (this.parameterOrder.length > 0) {
          const nextIndex =
            (this.selectedParameterIndex + 1) % this.parameterOrder.length;
          this.selectParameter(nextIndex);
        }
        return;
      }

      // Shift+Tab - cycle to previous parameter
      if (e.key === "Tab" && e.shiftKey) {
        e.preventDefault();
        if (this.parameterOrder.length > 0) {
          const prevIndex =
            this.selectedParameterIndex <= 0
              ? this.parameterOrder.length - 1
              : this.selectedParameterIndex - 1;
          this.selectParameter(prevIndex);
        }
        return;
      }

      // Esc - clear selection
      if (e.key === "Escape") {
        e.preventDefault();
        this.clearParameterSelection();
        return;
      }

      // Shift+Up - increase selected parameter
      if (
        e.key === "ArrowUp" &&
        e.shiftKey &&
        this.selectedParameterIndex >= 0
      ) {
        e.preventDefault();
        this.adjustSelectedParameter(1);
        return;
      }

      // Shift+Down - decrease selected parameter
      if (
        e.key === "ArrowDown" &&
        e.shiftKey &&
        this.selectedParameterIndex >= 0
      ) {
        e.preventDefault();
        this.adjustSelectedParameter(-1);
        return;
      }
    };

    document.addEventListener("keydown", this.keyboardHandler);
  }

  /**
   * Select parameter by index
   * Override this in child classes to implement visual feedback
   */
  protected selectParameter(index: number): void {
    if (index < 0 || index >= this.parameterOrder.length) {
      this.selectedParameterIndex = -1;
      return;
    }
    this.selectedParameterIndex = index;
  }

  /**
   * Clear parameter selection
   * Override this in child classes to implement visual feedback
   */
  protected clearParameterSelection(): void {
    this.selectedParameterIndex = -1;
  }

  /**
   * Adjust selected parameter value
   * Override this in child classes to implement parameter adjustment
   */
  protected adjustSelectedParameter(delta: number): void {
    // To be implemented by child classes
  }
}
