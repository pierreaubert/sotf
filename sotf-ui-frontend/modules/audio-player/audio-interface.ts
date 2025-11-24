/**
 * Audio interface types for interacting with cpal through Tauri
 */

/**
 * Represents audio configuration parameters
 */
export interface AudioConfig {
  sample_rate: number;
  channels: number;
  buffer_size?: number;
  sample_format: "f32" | "i16" | "u16" | "unknown";
}

/**
 * Represents information about an audio device
 */
export interface AudioDevice {
  name: string;
  is_input: boolean;
  is_default: boolean;
  supported_configs: AudioConfig[];
  default_config?: AudioConfig;
}

/**
 * State for storing the currently selected audio configuration
 */
export interface AudioState {
  selected_input_device?: string;
  selected_output_device?: string;
  input_config?: AudioConfig;
  output_config?: AudioConfig;
}

/**
 * Represents a map of audio devices by type (input/output)
 */
export interface AudioDevicesMap {
  input: AudioDevice[];
  output: AudioDevice[];
}

/**
 * Detailed properties of a specific audio device
 */
export interface DeviceProperties {
  name: string;
  type: "input" | "output";
  supported_config_ranges?: Array<{
    min_sample_rate: number;
    max_sample_rate: number;
    channels: number;
    sample_format: string;
    buffer_size_range?: unknown;
  }>;
  default_config?: {
    sample_rate: number;
    channels: number;
    sample_format: string;
    buffer_size?: unknown;
  };
}

// Tauri command bindings
import { invoke } from "@tauri-apps/api/core";

/**
 * Get information about all available audio devices
 * @returns A map of input and output devices
 */
export async function getAudioDevices(): Promise<AudioDevicesMap> {
  return await invoke<AudioDevicesMap>("get_audio_devices");
}

/**
 * Set the configuration for an audio device
 * @param deviceName - The name of the device to configure
 * @param isInput - Whether this is an input device
 * @param config - The audio configuration to apply
 * @returns Success message or error
 */
export async function setAudioDevice(
  deviceName: string,
  isInput: boolean,
  config: AudioConfig,
): Promise<string> {
  return await invoke<string>("set_audio_device", {
    device_name: deviceName,
    is_input: isInput,
    config: config,
  });
}

/**
 * Get the current audio configuration
 * @returns The current audio state
 */
export async function getAudioConfig(): Promise<AudioState> {
  return await invoke<AudioState>("get_audio_config");
}

/**
 * Get detailed properties of a specific audio device
 * @param deviceName - The name of the device
 * @param isInput - Whether this is an input device
 * @returns Detailed device properties
 */
export async function getDeviceProperties(
  deviceName: string,
  isInput: boolean,
): Promise<DeviceProperties> {
  return await invoke<DeviceProperties>("get_device_properties", {
    device_name: deviceName,
    is_input: isInput,
  });
}
