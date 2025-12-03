/**
 * Capture Controller - Manages audio capture logic
 * Separates audio capture functionality from UI concerns
 */

import { getAudioDevices as tauriGetAudioDevices } from "@audio-player/audio-interface";

export interface CaptureResult {
  success: boolean;
  frequencies: number[];
  magnitudes: number[];
  phases?: number[];
  error?: string;
}

export interface CaptureParameters {
  inputDevice: string;
  outputDevice: string;
  outputChannel: "left" | "right" | "both" | "default";
  signalType: "sweep" | "white" | "pink";
  duration: number;
  sampleRate: number;
  inputVolume: number; // 0-100
  outputVolume: number; // 0-100
}

export interface DeviceInfo {
  value: string;
  label: string;
  info?: string;
}

/**
 * Controller for managing audio capture operations
 */
export class CaptureController {
  private isCapturing: boolean = false;

  constructor() {}

  /**
   * Get list of available audio devices
   */
  async getAudioDevices(): Promise<{
    input: DeviceInfo[];
    output: DeviceInfo[];
  }> {
    const devices = await tauriGetAudioDevices();
    const toInfo = (name: string) => ({ value: name, label: name });
    return {
      input: [
        { value: "default", label: "System Default" },
        ...devices.input.map((d) => toInfo(d.name)),
      ],
      output: [
        { value: "default", label: "System Default" },
        ...devices.output.map((d) => toInfo(d.name)),
      ],
    };
  }

  /**
   * Start audio capture with the given parameters
   */
  async startCapture(_params: CaptureParameters): Promise<CaptureResult> {
    if (this.isCapturing) {
      throw new Error("Capture already in progress");
    }
    this.isCapturing = false;
    throw new Error(
      "Audio capture via WebRTC has been removed. Use backend recording commands.",
    );
  }

  /**
   * Stop the current capture
   */
  stopCapture(): void {
    console.log("[CaptureController] Stopping capture");

    this.cleanup();
  }

  /**
   * Clean up resources
   */
  private cleanup(): void {
    this.isCapturing = false;
  }

  /**
   * Get the device manager instance
   */
  getDeviceManager(): never {
    throw new Error(
      "DeviceManager removed. Use getAudioDevices() returning Tauri devices.",
    );
  }

  /**
   * Check if capture is currently in progress
   */
  isCaptureInProgress(): boolean {
    return this.isCapturing;
  }

  /**
   * Destroy the controller and clean up resources
   */
  destroy(): void {
    this.stopCapture();
    this.cleanup();
  }
}
