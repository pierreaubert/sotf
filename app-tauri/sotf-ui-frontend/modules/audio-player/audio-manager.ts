// StreamingDSP Audio Manager
// Wrapper for Tauri backend audio commands with StreamingDSP integration

import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

export interface AudioSpec {
  sample_rate: number;
  channels: number;
  bits_per_sample: number;
  total_frames?: number;
}

export interface AudioFileInfo {
  path: string;
  format: string;
  sample_rate: number;
  channels: number;
  bits_per_sample: number;
  duration_seconds?: number;
}

export interface FilterParam {
  frequency: number;
  q: number;
  gain: number;
}

export interface AudioStreamState {
  state: string; // "idle" | "loading" | "ready" | "playing" | "paused" | "seeking" | "error"
  position_seconds?: number;
  duration_seconds?: number;
  file?: string;
  output_device?: string;
}

export interface LoudnessInfo {
  momentary_lufs: number;
  shortterm_lufs: number;
  peak: number;
}

export interface AudioManagerCallbacks {
  onStateChange?: (state: string) => void;
  onPositionUpdate?: (position: number, duration?: number) => void;
  onError?: (error: string) => void;
  onFileLoaded?: (info: AudioFileInfo) => void;
}

/**
 * StreamingDSP Audio Manager
 * Manages audio playback through Tauri backend with StreamingDSP processing
 */
export class StreamingManager {
  private callbacks: AudioManagerCallbacks;
  private pollingInterval: number | null = null;
  private loudnessPollingInterval: number | null = null;
  private currentFileInfo: AudioFileInfo | null = null;
  private currentFilePath: string | null = null;
  private eventUnlisteners: UnlistenFn[] = [];
  private isTauriAvailable: boolean = false;
  private currentPosition: number = 0; // Track current position in seconds
  private loudnessCallback: ((info: LoudnessInfo | null) => void) | null = null;

  constructor(callbacks: AudioManagerCallbacks = {}) {
    this.callbacks = callbacks;
    this.checkTauriAvailability();
    if (this.isTauriAvailable) {
      this.setupEventListeners();
    } else {
      console.warn(
        "[StreamingManager] Tauri not available. Audio playback disabled. " +
          "Run the app via 'npm run tauri dev' or 'npm run tauri build'.",
      );
    }
  }

  /**
   * Check if Tauri is available (running in Tauri app vs browser)
   */
  private checkTauriAvailability(): void {
    try {
      // Check if __TAURI__ global exists
      this.isTauriAvailable =
        typeof (window as { __TAURI__?: unknown }).__TAURI__ !== "undefined";
    } catch {
      this.isTauriAvailable = false;
    }
  }

  /**
   * Set up Tauri event listeners for backend events
   */
  private async setupEventListeners(): Promise<void> {
    try {
      // Listen for state changes
      const stateUnlisten = await listen<{ state?: string }>(
        "stream:state-changed",
        (event) => {
          console.log("[StreamingManager] State changed:", event.payload);
          if (event.payload.state) {
            this.callbacks.onStateChange?.(event.payload.state);
          }
        },
      );
      this.eventUnlisteners.push(stateUnlisten);

      // Listen for position updates
      const positionUnlisten = await listen<{ position_seconds?: number }>(
        "stream:position-changed",
        (event) => {
          if (event.payload.position_seconds !== undefined) {
            this.callbacks.onPositionUpdate?.(
              event.payload.position_seconds,
              this.currentFileInfo?.duration_seconds,
            );
          }
        },
      );
      this.eventUnlisteners.push(positionUnlisten);

      // Listen for errors
      const errorUnlisten = await listen<{ error?: string }>(
        "stream:error",
        (event) => {
          console.error("[StreamingManager] Error:", event.payload);
          this.callbacks.onError?.(event.payload.error || "Unknown error");
        },
      );
      this.eventUnlisteners.push(errorUnlisten);

      // Listen for file loaded events
      const fileLoadedUnlisten = await listen<AudioFileInfo>(
        "stream:file-loaded",
        (event) => {
          console.log("[StreamingManager] File loaded:", event.payload);
          this.currentFileInfo = event.payload;
          this.callbacks.onFileLoaded?.(event.payload);
        },
      );
      this.eventUnlisteners.push(fileLoadedUnlisten);

      // Listen for filter update events
      const filtersUpdatedUnlisten = await listen<{
        ok: boolean;
        error?: string;
      }>("stream:filters-updated", (event) => {
        if (event.payload.ok) {
          console.log("[StreamingManager] Filters updated successfully");
        } else {
          console.error(
            "[StreamingManager] Filter update failed:",
            event.payload.error,
          );
          this.callbacks.onError?.(
            event.payload.error || "Failed to update filters",
          );
        }
      });
      this.eventUnlisteners.push(filtersUpdatedUnlisten);
    } catch (error) {
      console.error(
        "[StreamingManager] Failed to setup event listeners:",
        error,
      );
    }
  }

  /**
   * Load an audio file from a File object
   * Converts the File to a temporary path and loads it via backend
   */
  async loadAudioFile(file: File): Promise<AudioFileInfo> {
    try {
      // For Tauri, we need to convert the File to a path
      // In a real implementation, you'd save to temp directory or use dialog
      // For now, we'll assume the file path is provided directly

      // Try to get file path from File object (works in Tauri context)
      const filePath = (file as File & { path?: string }).path || file.name;

      if (!filePath || filePath === file.name) {
        throw new Error(
          "Cannot load file: File path not available. Use file dialog instead.",
        );
      }

      return await this.loadAudioFilePath(filePath);
    } catch (error) {
      const errorMsg = `Failed to load audio file: ${error}`;
      console.error("[StreamingManager]", errorMsg);
      this.callbacks.onError?.(errorMsg);
      throw error;
    }
  }

  /**
   * Load an audio file from a file path
   */
  async loadAudioFilePath(filePath: string): Promise<AudioFileInfo> {
    if (!this.isTauriAvailable) {
      throw new Error("Tauri not available. Run app via 'npm run tauri dev'.");
    }

    try {
      console.log("[StreamingManager] Loading file:", filePath);

      this.currentFilePath = filePath;
      const info = await invoke<AudioFileInfo>("stream_load_file", {
        filePath,
      });

      this.currentFileInfo = info;
      this.callbacks.onFileLoaded?.(info);

      return info;
    } catch (error) {
      const errorMsg = `Failed to load audio file: ${error}`;
      console.error("[StreamingManager]", errorMsg);
      this.callbacks.onError?.(errorMsg);
      throw error;
    }
  }

  /**
   * Start playback with optional filters and output device
   */
  async play(
    filters: FilterParam[] = [],
    outputDevice?: string,
  ): Promise<void> {
    if (!this.isTauriAvailable) {
      throw new Error("Tauri not available. Run app via 'npm run tauri dev'.");
    }

    try {
      if (!this.currentFileInfo) {
        throw new Error("No audio file loaded");
      }

      console.log(
        "[StreamingManager] Starting playback with filters:",
        filters,
      );

      // Convert FilterParam to backend format (FilterParams struct)
      const backendFilters = filters.map((f) => ({
        filter_type: "Peak",
        freq: f.frequency,
        q: f.q,
        db_gain: f.gain,
      }));

      await invoke("stream_start_playback", {
        outputDevice,
        filters: backendFilters,
      });

      // Reset position tracking
      this.currentPosition = 0;
      // Note: onStateChange("playing") will be called via backend event when playback actually starts
      this.startStatePolling();
    } catch (error) {
      const errorMsg = `Failed to start playback: ${error}`;
      console.error("[StreamingManager]", errorMsg);
      this.callbacks.onError?.(errorMsg);
      throw error;
    }
  }

  /**
   * Pause playback
   */
  async pause(): Promise<void> {
    try {
      await invoke("stream_pause_playback");
      this.callbacks.onStateChange?.("paused");
      this.stopStatePolling();
      this.stopLoudnessPolling();
    } catch (error) {
      console.error("[StreamingManager] Failed to pause:", error);
      throw error;
    }
  }

  /**
   * Resume playback
   */
  async resume(): Promise<void> {
    try {
      await invoke("stream_resume_playback");
      this.callbacks.onStateChange?.("playing");
      this.startStatePolling();
    } catch (error) {
      console.error("[StreamingManager] Failed to resume:", error);
      throw error;
    }
  }

  /**
   * Stop playback
   */
  async stop(): Promise<void> {
    try {
      await invoke("stream_stop_playback");
      this.currentPosition = 0;
      this.callbacks.onStateChange?.("idle");
      this.stopStatePolling();
      this.stopLoudnessPolling();
    } catch (error) {
      console.error("[StreamingManager] Failed to stop:", error);
      throw error;
    }
  }

  /**
   * Seek to a specific position in seconds
   */
  async seek(seconds: number): Promise<void> {
    try {
      await invoke("stream_seek", { seconds });
      this.currentPosition = seconds;
      this.callbacks.onPositionUpdate?.(
        seconds,
        this.currentFileInfo?.duration_seconds,
      );
    } catch (error) {
      console.error("[StreamingManager] Failed to seek:", error);
      throw error;
    }
  }

  /**
   * Update EQ filters in real-time without restarting playback
   * Uses the new stream_update_filters command for gap-free updates
   */
  async updateFilters(filters: FilterParam[]): Promise<void> {
    if (!this.isTauriAvailable) {
      throw new Error("Tauri not available. Run app via 'npm run tauri dev'.");
    }

    try {
      console.log("[StreamingManager] Updating filters (hot-reload):", filters);

      // Convert FilterParam to backend format (FilterParams struct)
      const backendFilters = filters.map((f) => ({
        filter_type: "Peak",
        freq: f.frequency,
        q: f.q,
        db_gain: f.gain,
      }));

      // Call the new stream_update_filters command
      // This updates filters without restarting CamillaDSP
      await invoke("stream_update_filters", {
        filters: backendFilters,
        loudness: null, // TODO: Add loudness support when needed
      });

      console.log("[StreamingManager] Filters updated successfully (no gaps)");
    } catch (error) {
      console.error("[StreamingManager] Failed to update filters:", error);
      throw error;
    }
  }

  /**
   * Get current playback state
   */
  async getPlaybackState(): Promise<string> {
    try {
      return await invoke<string>("stream_get_state");
    } catch (_error) {
      console.error("[StreamingManager] Failed to get state:", _error);
      return "error";
    }
  }

  /**
   * Get current file info
   */
  async getFileInfo(): Promise<AudioFileInfo | null> {
    try {
      return await invoke<AudioFileInfo | null>("stream_get_file_info");
    } catch (error) {
      console.error("[StreamingManager] Failed to get file info:", error);
      return null;
    }
  }

  /**
   * Get loaded file info (cached)
   */
  getCurrentFileInfo(): AudioFileInfo | null {
    return this.currentFileInfo;
  }

  /**
   * Set output device
   */
  setOutputDevice(deviceId: string): void {
    // Note: This will take effect on next playback start
    console.log("[StreamingManager] Output device set to:", deviceId);
  }

  /**
   * Start polling playback state (for progress updates)
   */
  startStatePolling(intervalMs: number = 250): void {
    if (this.pollingInterval !== null) {
      return; // Already polling
    }

    this.pollingInterval = window.setInterval(async () => {
      try {
        // Get current playback state to check if still playing
        const state = await this.getPlaybackState();
        if (state === "playing") {
          // Increment position estimate (will be corrected by backend events)
          this.currentPosition += intervalMs / 1000;

          // Trigger position update callback
          if (this.currentFileInfo?.duration_seconds) {
            this.callbacks.onPositionUpdate?.(
              this.currentPosition,
              this.currentFileInfo.duration_seconds,
            );
          }
        }
      } catch (error) {
        // Ignore polling errors
      }
    }, intervalMs);
  }

  /**
   * Stop polling playback state
   */
  stopStatePolling(): void {
    if (this.pollingInterval !== null) {
      clearInterval(this.pollingInterval);
      this.pollingInterval = null;
    }
  }

  /**
   * Enable loudness monitoring in the backend
   */
  async enableLoudnessMonitoring(): Promise<void> {
    if (!this.isTauriAvailable) {
      return;
    }

    try {
      await invoke("stream_enable_loudness_monitoring");
      console.log("[StreamingManager] Loudness monitoring enabled");
    } catch (error) {
      console.error(
        "[StreamingManager] Failed to enable loudness monitoring:",
        error,
      );
      throw error;
    }
  }

  /**
   * Disable loudness monitoring in the backend
   */
  async disableLoudnessMonitoring(): Promise<void> {
    if (!this.isTauriAvailable) {
      return;
    }

    try {
      await invoke("stream_disable_loudness_monitoring");
      console.log("[StreamingManager] Loudness monitoring disabled");
    } catch (error) {
      console.error(
        "[StreamingManager] Failed to disable loudness monitoring:",
        error,
      );
    }
  }

  /**
   * Get current loudness information from backend
   */
  async getLoudness(): Promise<LoudnessInfo | null> {
    if (!this.isTauriAvailable) {
      return null;
    }

    try {
      const result = await invoke<LoudnessInfo | null>("stream_get_loudness");
      return result;
    } catch (error) {
      console.error("[StreamingManager] Failed to get loudness:", error);
      return null;
    }
  }

  /**
   * Start polling loudness information
   */
  startLoudnessPolling(
    intervalMs: number = 100,
    onUpdate: (info: LoudnessInfo | null) => void,
  ): void {
    if (this.loudnessPollingInterval !== null) {
      console.log("[StreamingManager] Loudness polling already active");
      return; // Already polling
    }

    console.log(
      "[StreamingManager] Starting loudness polling with interval:",
      intervalMs,
    );
    this.loudnessCallback = onUpdate;

    this.loudnessPollingInterval = window.setInterval(async () => {
      try {
        const loudness = await this.getLoudness();
        console.log("[StreamingManager] Got loudness data:", loudness);
        this.loudnessCallback?.(loudness);
      } catch (error) {
        console.error("[StreamingManager] Error polling loudness:", error);
      }
    }, intervalMs);
  }

  /**
   * Stop polling loudness information
   */
  stopLoudnessPolling(): void {
    if (this.loudnessPollingInterval !== null) {
      clearInterval(this.loudnessPollingInterval);
      this.loudnessPollingInterval = null;
      this.loudnessCallback = null;
    }
  }

  /**
   * Check if audio is currently playing
   */
  async isPlaying(): Promise<boolean> {
    const state = await this.getPlaybackState();
    return state === "playing";
  }

  /**
   * Get current playback position (estimated)
   */
  getCurrentTime(): number {
    // Would need backend support for real-time position tracking
    return 0;
  }

  /**
   * Get audio duration
   */
  getDuration(): number {
    return this.currentFileInfo?.duration_seconds || 0;
  }

  /**
   * Cleanup and destroy
   */
  destroy(): void {
    this.stopStatePolling();
    this.stopLoudnessPolling();

    // Unlisten from all events
    this.eventUnlisteners.forEach((unlisten) => {
      try {
        unlisten();
      } catch (error) {
        console.error("[StreamingManager] Failed to unlisten:", error);
      }
    });
    this.eventUnlisteners = [];

    this.currentFileInfo = null;
    this.currentFilePath = null;
    this.currentPosition = 0;
  }
}

export default StreamingManager;
