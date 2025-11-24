// Types for multi-channel room EQ optimization

import type { OptimizationParams, OptimizationResult } from "./optimization";

// ============================================================================
// RoomEQ Configuration Types
// ============================================================================

/**
 * Type of room EQ configuration
 */
export type RoomConfigType =
  | { type: "single" }
  | { type: "stereo_pair"; mirror: boolean }
  | { type: "multi_channel"; channel_count: number; parallel: boolean }
  | { type: "multi_way"; driver_count: number; optimize_crossover: boolean };

/**
 * Measurement source for a channel
 */
export type MeasurementSource =
  | {
      source_type: "file";
      path: string;
    }
  | {
      source_type: "database";
      speaker: string;
      version: string;
      measurement: string;
    }
  | {
      source_type: "captured";
      frequencies: number[];
      magnitudes: number[];
    };

/**
 * Multi-way driver configuration
 */
export interface DriverConfig {
  name: string;
  measurement: MeasurementSource;
}

/**
 * Crossover configuration for multi-way speakers
 */
export interface CrossoverConfig {
  crossover_type: string; // "LR24", "LR48", "Butterworth24", etc.
  frequency?: number; // Fixed frequency (if not optimizing)
  optimize: boolean; // Whether to optimize crossover frequency
}

/**
 * Channel configuration
 */
export interface ChannelConfig {
  channel_name: string;
  measurement?: MeasurementSource; // For simple channels
  drivers?: DriverConfig[]; // For multi-way speakers
  crossover?: CrossoverConfig; // For multi-way speakers
  target?: MeasurementSource; // Optional target curve
}

/**
 * Complete room EQ configuration
 */
export interface RoomEQConfig {
  config_type: RoomConfigType;
  channels: ChannelConfig[];
  optimizer_params: OptimizationParams; // Base optimization parameters
}

/**
 * Progress update for multi-channel optimization
 */
export interface RoomEQProgress {
  channel_index: number;
  channel_name: string;
  stage: string; // "crossover", "eq", "complete"
  progress: {
    iteration: number;
    fitness: number;
    params: number[];
    convergence: number;
  };
}

/**
 * Result for a single channel optimization
 */
export interface ChannelResult {
  channel_name: string;
  success: boolean;
  error_message?: string;
  optimization_result?: OptimizationResult;
}

/**
 * Complete room EQ optimization result
 */
export interface RoomEQResult {
  success: boolean;
  error_message?: string;
  channel_results: ChannelResult[];
  dsp_chain_json?: string; // JSON output compatible with roomeq binary
}

// ============================================================================
// UI State Types
// ============================================================================

/**
 * Channel status in the UI
 */
export type ChannelStatus = "pending" | "optimizing" | "complete" | "error";

/**
 * Channel state for UI display
 */
export interface ChannelState {
  config: ChannelConfig;
  status: ChannelStatus;
  progress?: number; // 0-100
  result?: ChannelResult;
  stage?: string; // Current stage (for multi-way)
}

/**
 * Overall room EQ workflow state
 */
export interface RoomEQWorkflowState {
  step: number; // Current workflow step (0-4)
  config_type?: RoomConfigType;
  channels: ChannelState[];
  optimizer_params?: OptimizationParams;
  is_running: boolean;
  current_channel_index?: number;
  overall_result?: RoomEQResult;
}

// ============================================================================
// Helper Types for UI
// ============================================================================

/**
 * Preset configurations for common setups
 */
export interface RoomEQPreset {
  name: string;
  description: string;
  config_type: RoomConfigType;
  channel_names: string[];
  icon?: string;
}

/**
 * Crossover type option for UI
 */
export interface CrossoverTypeOption {
  value: string;
  label: string;
  description: string;
}

// ============================================================================
// Preset Configurations
// ============================================================================

export const ROOMEQ_PRESETS: RoomEQPreset[] = [
  {
    name: "Single Speaker/Headphone",
    description: "Optimize a single speaker or headphone",
    config_type: { type: "single" },
    channel_names: ["Single"],
    icon: "🔊",
  },
  {
    name: "Stereo Pair",
    description: "Left and Right speakers (optimize separately)",
    config_type: { type: "stereo_pair", mirror: false },
    channel_names: ["Left", "Right"],
    icon: "🔊🔊",
  },
  {
    name: "Stereo Pair (Mirrored)",
    description: "Left and Right speakers (optimize once, mirror)",
    config_type: { type: "stereo_pair", mirror: true },
    channel_names: ["Left", "Right"],
    icon: "🔊↔️🔊",
  },
  {
    name: "5.1 Surround",
    description: "Front L/R, Center, Subwoofer, Surround L/R",
    config_type: { type: "multi_channel", channel_count: 6, parallel: false },
    channel_names: [
      "Front Left",
      "Front Right",
      "Center",
      "Subwoofer",
      "Surround Left",
      "Surround Right",
    ],
    icon: "🔊🔊🔊",
  },
  {
    name: "7.1 Surround",
    description: "Front L/R, Center, Subwoofer, Side L/R, Rear L/R",
    config_type: { type: "multi_channel", channel_count: 8, parallel: false },
    channel_names: [
      "Front Left",
      "Front Right",
      "Center",
      "Subwoofer",
      "Side Left",
      "Side Right",
      "Rear Left",
      "Rear Right",
    ],
    icon: "🔊🔊🔊",
  },
  {
    name: "2-Way Speaker",
    description: "Woofer + Tweeter with crossover",
    config_type: { type: "multi_way", driver_count: 2, optimize_crossover: true },
    channel_names: ["2-Way Speaker"],
    icon: "🔊⚡",
  },
  {
    name: "3-Way Speaker",
    description: "Woofer + Midrange + Tweeter with crossovers",
    config_type: { type: "multi_way", driver_count: 3, optimize_crossover: true },
    channel_names: ["3-Way Speaker"],
    icon: "🔊⚡⚡",
  },
];

export const CROSSOVER_TYPES: CrossoverTypeOption[] = [
  {
    value: "LR24",
    label: "Linkwitz-Riley 24 dB/oct (LR4)",
    description: "Most common, smooth transition, 4th order",
  },
  {
    value: "LR48",
    label: "Linkwitz-Riley 48 dB/oct (LR8)",
    description: "Steeper roll-off, 8th order",
  },
  {
    value: "Butterworth24",
    label: "Butterworth 24 dB/oct (BW4)",
    description: "Maximally flat passband, 4th order",
  },
  {
    value: "Butterworth12",
    label: "Butterworth 12 dB/oct (BW2)",
    description: "Gentler slope, 2nd order",
  },
];

// ============================================================================
// Helper Functions
// ============================================================================

/**
 * Create an empty channel configuration
 */
export function createEmptyChannel(name: string): ChannelConfig {
  return {
    channel_name: name,
  };
}

/**
 * Create channel state from config
 */
export function createChannelState(config: ChannelConfig): ChannelState {
  return {
    config,
    status: "pending",
    progress: 0,
  };
}

/**
 * Get channel count from config type
 */
export function getChannelCount(configType: RoomConfigType): number {
  switch (configType.type) {
    case "single":
      return 1;
    case "stereo_pair":
      return 2;
    case "multi_channel":
      return configType.channel_count;
    case "multi_way":
      return 1; // One speaker with multiple drivers
    default:
      return 1;
  }
}

/**
 * Validate room EQ configuration
 */
export function validateRoomEQConfig(config: RoomEQConfig): string[] {
  const errors: string[] = [];

  // Check channels
  if (config.channels.length === 0) {
    errors.push("At least one channel must be configured");
  }

  // Validate each channel
  for (const [index, channel] of config.channels.entries()) {
    if (!channel.channel_name) {
      errors.push(`Channel ${index + 1} has no name`);
    }

    // Check if channel has measurement or drivers
    if (!channel.measurement && !channel.drivers) {
      errors.push(`Channel "${channel.channel_name}" has no measurement source`);
    }

    // Multi-way validation
    if (channel.drivers) {
      if (channel.drivers.length < 2) {
        errors.push(
          `Multi-way channel "${channel.channel_name}" needs at least 2 drivers`,
        );
      }
      if (!channel.crossover) {
        errors.push(
          `Multi-way channel "${channel.channel_name}" needs crossover configuration`,
        );
      }
    }
  }

  return errors;
}
