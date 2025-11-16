// Plugin System Type Definitions
// Base interfaces and types for the plugin architecture

/**
 * Plugin categories
 */
export type PluginCategory =
  | "eq"
  | "dynamics"
  | "spatial"
  | "analyzer"
  | "utility";

/**
 * Plugin metadata
 */
export interface PluginMetadata {
  id: string;
  name: string;
  category: PluginCategory;
  version: string;
  icon?: string;
  hasBuiltInLevelMeters?: boolean; // True if plugin displays its own level meters
}

/**
 * Plugin state
 */
export interface PluginState {
  enabled: boolean;
  bypassed: boolean;
  parameters: Record<string, any>;
}

/**
 * Plugin preset
 */
export interface PluginPreset {
  name: string;
  state: PluginState;
}

/**
 * Base plugin interface
 * All plugins must implement this interface
 */
export interface IPlugin {
  // Metadata
  readonly metadata: PluginMetadata;

  // Lifecycle
  initialize(container: HTMLElement, config?: PluginConfig): void;
  destroy(): void;

  // State management
  getState(): PluginState;
  setState(state: Partial<PluginState>): void;

  // UI
  render(standalone: boolean): void;
  resize?(): void;

  // Keyboard shortcuts
  getShortcuts?(): ShortcutItem[];

  // Events
  on(event: string, callback: (...args: any[]) => void): void;
  off(event: string, callback: (...args: any[]) => void): void;
  emit(event: string, ...args: any[]): void;
}

/**
 * Plugin configuration
 */
export interface PluginConfig {
  // Display settings
  standalone?: boolean; // Whether plugin has its own menubar
  showMenubar?: boolean; // Show/hide menubar

  // Initial state
  initialState?: Partial<PluginState>;

  // Callbacks
  onStateChange?: (state: PluginState) => void;
  onPresetChange?: (preset: PluginPreset) => void;
  onBypass?: (bypassed: boolean) => void;

  // Host integration
  hostId?: string; // Parent host ID if embedded
}

/**
 * Plugin event types
 */
export enum PluginEvent {
  StateChanged = "stateChanged",
  PresetChanged = "presetChanged",
  Bypassed = "bypassed",
  ParameterChanged = "parameterChanged",
  Resize = "resize",
}

/**
 * Menubar configuration
 */
export interface MenubarConfig {
  showName?: boolean;
  showPresets?: boolean;
  showMatrix?: boolean;
  showMuteSolo?: boolean;
  customButtons?: MenubarButton[];
}

/**
 * Custom menubar button
 */
export interface MenubarButton {
  id: string;
  label: string;
  icon?: string;
  onClick: () => void;
}

/**
 * Level meter data
 */
export interface LevelMeterData {
  channels: number[]; // RMS levels in dB (one per channel)
  peaks: number[]; // Peak levels in dB (one per channel)
  clipping: boolean[]; // Clipping indicator per channel
}

/**
 * LUFS meter data
 */
export interface LUFSMeterData {
  momentary: number; // Momentary loudness (LUFS)
  shortTerm: number; // Short-term loudness (LUFS)
  integrated: number; // Integrated loudness (LUFS)
}

/**
 * Keyboard shortcut item
 */
export interface ShortcutItem {
  key: string;
  description: string;
}
