// Tauri backend integration for audio capture
// Provides TypeScript interfaces for Tauri audio commands

import { invoke } from "@tauri-apps/api/core";

export interface AudioConfig {
  sample_rate: number;
  channels: number;
  buffer_size?: number;
  sample_format: string; // "f32", "i16", "u16"
}

export interface AudioDevice {
  name: string;
  is_input: boolean;
  is_default: boolean;
  supported_configs: AudioConfig[];
  default_config?: AudioConfig;
}

export interface AudioDevices {
  input: AudioDevice[];
  output: AudioDevice[];
}

export interface RecordingProgress {
  channel: number;
  state: "playing" | "recording" | "analyzing" | "done";
  progress: number; // 0.0 to 1.0
}

export interface RecordingResult {
  channel: number;
  wav_path: string;
  csv_path: string;
  frequencies: number[];
  magnitude_db: number[];
  phase_deg: number[];
}

/**
 * Get all available audio devices (input and output)
 */
export async function getAudioDevices(): Promise<AudioDevices> {
  const devices = await invoke<{ [key: string]: AudioDevice[] }>(
    "get_audio_devices",
  );
  return {
    input: devices.input || [],
    output: devices.output || [],
  };
}

/**
 * Get properties for a specific device
 */
export async function getDeviceProperties(
  deviceName: string,
  isInput: boolean,
): Promise<AudioConfig | null> {
  try {
    const props = await invoke<any>("get_device_properties", {
      deviceName,
      isInput,
    });
    return props;
  } catch (error) {
    console.error("Failed to get device properties:", error);
    return null;
  }
}

/**
 * Set audio device configuration
 */
export async function setAudioDevice(
  deviceName: string,
  isInput: boolean,
  config: AudioConfig,
): Promise<string> {
  return await invoke<string>("set_audio_device", {
    deviceName,
    isInput,
    config,
  });
}

/**
 * Record a single channel with a test signal
 *
 * This will:
 * 1. Generate a test signal (sweep, pink noise, white noise)
 * 2. Play it back on the specified output channel
 * 3. Record from the specified input channel
 * 4. Analyze the recording (frequency response, phase)
 * 5. Return the results
 */
export async function recordChannel(
  outputDevice: string,
  inputDevice: string,
  outputChannel: number,
  inputChannel: number,
  signalType: "sweep" | "white" | "pink",
  duration: number, // in seconds
  sampleRate: number,
  outputPath: string, // base path for output files
  onProgress?: (progress: RecordingProgress) => void,
): Promise<RecordingResult> {
  // Note: This command needs to be implemented in Tauri backend
  return await invoke<RecordingResult>("record_channel", {
    outputDevice,
    inputDevice,
    outputChannel,
    inputChannel,
    signalType,
    duration,
    sampleRate,
    outputPath,
  });
}

/**
 * Record all channels sequentially
 */
export async function recordAllChannels(
  outputDevice: string,
  inputDevice: string,
  channelCount: number,
  signalType: "sweep" | "white" | "pink",
  duration: number,
  sampleRate: number,
  outputPath: string,
  onProgress?: (channelIndex: number, progress: RecordingProgress) => void,
): Promise<RecordingResult[]> {
  const results: RecordingResult[] = [];

  for (let i = 0; i < channelCount; i++) {
    try {
      const result = await recordChannel(
        outputDevice,
        inputDevice,
        i,
        i,
        signalType,
        duration,
        sampleRate,
        `${outputPath}_ch${i}`,
        (progress) => {
          if (onProgress) {
            onProgress(i, progress);
          }
        },
      );
      results.push(result);

      // Wait 1 second between channels
      if (i < channelCount - 1) {
        await new Promise((resolve) => setTimeout(resolve, 1000));
      }
    } catch (error) {
      console.error(`Failed to record channel ${i}:`, error);
      throw error;
    }
  }

  return results;
}

/**
 * Generate a test signal and save to file
 */
export async function generateTestSignal(
  signalType: "sweep" | "white" | "pink",
  duration: number,
  sampleRate: number,
  amplitude: number,
  outputPath: string,
): Promise<string> {
  return await invoke<string>("generate_test_signal", {
    signalType,
    duration,
    sampleRate,
    amplitude,
    outputPath,
  });
}

/**
 * Analyze a recorded WAV file
 */
export async function analyzeRecording(
  wavPath: string,
  csvPath: string,
): Promise<{
  frequencies: number[];
  magnitude_db: number[];
  phase_deg: number[];
}> {
  return await invoke("analyze_recording", {
    wavPath,
    csvPath,
  });
}

/**
 * Save capture configuration to JSON file
 */
export async function saveCaptureConfig(
  config: any,
  filePath: string,
): Promise<void> {
  await invoke("save_capture_config", {
    config: JSON.stringify(config),
    filePath,
  });
}

/**
 * Load capture configuration from JSON file
 */
export async function loadCaptureConfig(filePath: string): Promise<any> {
  const configJson = await invoke<string>("load_capture_config", {
    filePath,
  });
  return JSON.parse(configJson);
}

/**
 * Save recordings to a ZIP file containing WAV and CSV files
 */
export async function saveRecordings(
  recordings: RecordingResult[],
  outputPath: string,
): Promise<string> {
  return await invoke<string>("save_recordings_zip", {
    recordings,
    outputPath,
  });
}

/**
 * Load recordings from a ZIP file
 */
export async function loadRecordings(
  zipPath: string,
): Promise<RecordingResult[]> {
  return await invoke<RecordingResult[]>("load_recordings_zip", {
    zipPath,
  });
}
