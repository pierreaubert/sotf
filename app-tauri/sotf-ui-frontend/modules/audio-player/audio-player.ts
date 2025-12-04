// Standalone Audio Player Module

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { StreamingManager, type AudioFileInfo } from "./audio-manager";
import { SpectrumAnalyzerComponent } from "./spectrum-analyzer";
import {
  VisualEQConfig,
  type ExtendedFilterParam,
  type FilterParam,
  FILTER_TYPES,
} from "./visual-eq-config";

export interface ReplayGainInfo {
  gain: number; // dB
  peak: number; // 0.0 to 1.0+
}

export interface AudioPlayerConfig {
  // Demo audio tracks configuration
  demoTracks?: { [key: string]: string };

  // EQ configuration
  enableEQ?: boolean;
  maxFilters?: number;

  // Spectrum analyzer configuration
  enableSpectrum?: boolean;
  fftSize?: number;
  smoothingTimeConstant?: number;

  // UI configuration
  showProgress?: boolean;
  showFrequencyLabels?: boolean;
  compactMode?: boolean;
}

// Re-export EQ types from visual-eq-config for backward compatibility
export type { FilterParam, ExtendedFilterParam } from "./visual-eq-config";
export { FILTER_TYPES } from "./visual-eq-config";

export interface AudioPlayerCallbacks {
  onPlay?: () => void;
  onStop?: () => void;
  onEQToggle?: (enabled: boolean) => void;
  onTrackChange?: (trackName: string) => void;
  onError?: (error: string) => void;
}

export class AudioPlayer {
  private audioContext: AudioContext | null = null;
  private audioBuffer: AudioBuffer | null = null;
  private audioSource: AudioBufferSourceNode | null = null;
  private gainNode: GainNode | null = null;
  private isAudioPlaying: boolean = false;
  private isAudioPaused: boolean = false;
  private audioStartTime: number = 0;
  private audioPauseTime: number = 0;
  private audioAnimationFrame: number | null = null;
  private currentAudioPath: string | null = null; // Track current audio file path for Rust backend

  // Visual EQ Configuration
  private visualEQConfig: VisualEQConfig | null = null;
  private eqEnabled: boolean = true; // Backward compatibility property
  private eqFilters: any[] = []; // Backward compatibility property
  private outputDeviceId: string = "default"; // Selected output device ID
  private audioElement: HTMLAudioElement | null = null; // For device routing

  // Playback configuration
  private loudnessCompensation: boolean = false;
  private splAmplitude: number = -20; // dB range: -30 to 0
  private autoGain: boolean = true; // Auto-gain enabled by default

  // Frequency analyzer
  private analyserNode: AnalyserNode | null = null;
  private spectrumCanvas: HTMLCanvasElement | null = null;
  private spectrumCtx: CanvasRenderingContext2D | null = null;
  private spectrumAnimationFrame: number | null = null;

  // Loudness monitoring
  private loudnessDisplayMomentary: HTMLElement | null = null;
  private loudnessDisplayShortterm: HTMLElement | null = null;
  private loudnessPollingActive: boolean = false;
  private replayGainDisplay: HTMLElement | null = null;
  private peakDisplay: HTMLElement | null = null;
  private currentReplayGain: ReplayGainInfo | null = null;

  // Host info display
  private hostInfoFormat: HTMLElement | null = null;
  private hostInfoSampleRate: HTMLElement | null = null;
  private hostInfoChannels: HTMLElement | null = null;
  private hostInfoBits: HTMLElement | null = null;

  // 30-bin spectrum analyzer constants
  private readonly SPECTRUM_BINS = 30;
  private readonly SPECTRUM_MIN_FREQ = 20;
  private readonly SPECTRUM_MAX_FREQ = 20000;
  private spectrumBinEdges: number[] = [];
  private spectrumBinCenters: number[] = [];
  private spectrumBinValues: number[] = []; // Smoothed values for display

  // UI Elements
  private container: HTMLElement;
  private demoSelect: HTMLSelectElement | null = null;
  private playBtn: HTMLButtonElement | null = null;
  private pauseBtn: HTMLButtonElement | null = null;
  private stopBtn: HTMLButtonElement | null = null;
  private eqOnBtn: HTMLButtonElement | null = null;
  private eqOffBtn: HTMLButtonElement | null = null;
  private eqConfigBtn: HTMLButtonElement | null = null;
  private eqMiniCanvas: HTMLCanvasElement | null = null;
  private statusText: HTMLElement | null = null;
  private positionText: HTMLElement | null = null;
  private durationText: HTMLElement | null = null;
  private progressFill: HTMLElement | null = null;

  // ReplayGain
  private replayGainInfo: ReplayGainInfo | null = null;
  private replayGainContainer: HTMLElement | null = null;

  // Configuration
  private config: AudioPlayerConfig;
  private callbacks: AudioPlayerCallbacks;
  private instanceId: string;

  // Pause double-click tracking
  private pauseClickCount: number = 0;
  private pauseClickTimer: number | null = null;

  // Streaming manager
  private streamingManager: StreamingManager;

  // Spectrum analyzer component
  private spectrumAnalyzer: SpectrumAnalyzerComponent | null = null;
  private spectrumStopTimer: number | null = null;

  // Event handlers
  private resizeHandler: (() => void) | null = null;

  constructor(
    container: HTMLElement,
    config: AudioPlayerConfig = {},
    callbacks: AudioPlayerCallbacks = {},
  ) {
    if (!container) {
      throw new Error(
        "AudioPlayer: container element is required but was null/undefined",
      );
    }
    this.container = container;
    this.instanceId = "audio-player-" + Math.random().toString(36).substr(2, 9);
    this.config = {
      enableEQ: true,
      maxFilters: 10,
      enableSpectrum: true,
      fftSize: 4096,
      smoothingTimeConstant: 0.8,
      showProgress: true,
      showFrequencyLabels: true,
      compactMode: false,
      demoTracks: {
        classical: "/demo-audio/classical.flac",
        country: "/demo-audio/country.flac",
        edm: "/demo-audio/edm.flac",
        female_vocal: "/demo-audio/female_vocal.flac",
        jazz: "/demo-audio/jazz.flac",
        piano: "/demo-audio/piano.flac",
        rock: "/demo-audio/rock.flac",
      },
      ...config,
    };
    this.callbacks = callbacks;

    // Initialize streaming manager
    this.streamingManager = new StreamingManager({
      onStateChange: (state) => this.handleStateChange(state),
      onPositionUpdate: (position, duration) =>
        this.handlePositionUpdate(position, duration ?? 0),
      onError: (error) => this.handleError(error),
      onFileLoaded: (info) => this.handleFileLoaded(info),
    });

    this.init();
  }

  private async init(): Promise<void> {
    try {
      await this.setupAudioContext();
      this.createUI();

      // Initialize Visual EQ Configuration after UI is created
      if (this.config.enableEQ) {
        // Get mini canvas after UI is created
        const eqMiniCanvas = this.container.querySelector(
          ".eq-mini-canvas",
        ) as HTMLCanvasElement | null;

        this.visualEQConfig = new VisualEQConfig(
          this.container,
          this.instanceId,
          this.streamingManager,
          {
            onFilterParamsChange: (filterParams) => {
              // Sync local state without calling updateFilterParams to avoid recursion
              // The VisualEQConfig has already handled the update
            },
            onEQToggle: (enabled) => {
              // Sync local state
              this.eqEnabled = enabled;
              // Notify external callback
              this.callbacks.onEQToggle?.(enabled);
            },
            onAutoGainChange: (enabled) => {
              this.autoGain = enabled;
            },
            onLoudnessCompensationChange: (enabled) => {
              this.loudnessCompensation = enabled;
            },
            onSplAmplitudeChange: (amplitude) => {
              this.splAmplitude = amplitude;
            },
            getAutoGain: () => this.autoGain,
            getLoudnessCompensation: () => this.loudnessCompensation,
            getSplAmplitude: () => this.splAmplitude,
          },
          eqMiniCanvas,
        );
      }
      this.setupEventListeners();
    } catch (error) {
      console.error("Failed to initialize AudioPlayer:", error);
      this.callbacks.onError?.("Failed to initialize audio player: " + error);
    }
  }

  private handleStateChange(state: string): void {
    console.log("[AudioPlayer] Backend state:", state);

    if (state === "playing") {
      this.isAudioPlaying = true;
      this.isAudioPaused = false;

      // Restart spectrum analyzer if it's not already running
      if (this.spectrumAnalyzer && !this.spectrumAnalyzer.isActive()) {
        this.spectrumAnalyzer.start().catch((err) => {
          console.error(
            "[AudioPlayer] Failed to start spectrum analyzer:",
            err,
          );
        });
      }
    } else if (state === "paused") {
      this.isAudioPlaying = false;
      this.isAudioPaused = true;
    } else if (state === "idle" || state === "ready") {
      this.isAudioPlaying = false;
      this.isAudioPaused = false;
      // Reset position display
      if (this.positionText) {
        this.positionText.textContent = "--:--";
      }
      if (this.progressFill) {
        this.progressFill.style.width = "0%";
      }
      this.setStatus("Ready");
    } else if (state === "error") {
      this.isAudioPlaying = false;
      this.isAudioPaused = false;
      this.setStatus("Error");
    }
  }

  private handlePositionUpdate(position: number, duration: number): void {
    if (this.positionText) {
      this.positionText.textContent = this.formatTime(position);
    }
    if (this.durationText) {
      this.durationText.textContent = this.formatTime(duration);
    }
    if (this.progressFill && duration > 0) {
      const progress = (position / duration) * 100;
      this.progressFill.style.width = `${progress}%`;
    }
  }

  private handleError(error: string): void {
    console.error("[AudioPlayer] Backend error:", error);
    this.callbacks.onError?.(error);
    this.setStatus("Error: " + error);
  }

  private handleFileLoaded(info: AudioFileInfo): void {
    console.log("[AudioPlayer] File loaded:", info);
    this.updateAudioInfo();
    this.updateHostInfo(info);
    this.showAudioStatus(true);
    this.setListenButtonEnabled(true);
    this.setStatus("Ready");
  }

  private async setupAudioContext(): Promise<void> {
    this.audioContext = new AudioContext();
    this.gainNode = this.audioContext.createGain();
    this.gainNode.connect(this.audioContext.destination);
  }

  private createUI(): void {
    const selectId = `demo-audio-select-${this.instanceId}`;

    const html = `
<div class="audio-player">

  <!-- Section: Playback Options -->
  ${
    this.config.enableEQ
      ? `
  <div class="playback-options-section-inline">
    <h4>Playback Options</h4>
    <div class="playback-options-container"></div>
  </div>
  `
      : ""
  }

  <!-- Section: Filter Configuration -->
  ${
    this.config.enableEQ
      ? `
  <div class="filter-config-section-inline" style="margin: 16px 0; border: 1px solid var(--border-color, #ddd); border-radius: 8px; overflow: hidden;">
    <h4 class="filter-config-header" style="margin: 0; padding: 12px; background: var(--bg-secondary, #f5f5f5); cursor: pointer; user-select: none; display: flex; align-items: center; justify-content: space-between;">
      <span>Filter Configuration</span>
      <span class="filter-config-toggle" style="font-size: 1.2em; transition: transform 0.2s;">▼</span>
    </h4>
    <div class="eq-table-container" style="display: none; padding: 12px;"></div>
  </div>
  `
      : ""
  }

  <!-- Section: Host Info -->
  <div class="host-info-section-inline" style="margin: 16px 0; padding: 12px; background: var(--bg-secondary, #f5f5f5); border-radius: 8px;">
    <h4 style="margin: 0 0 8px 0;">Audio Host Information</h4>
    <div class="host-info-grid" style="display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 12px; font-size: 0.9em;">
      <div class="host-info-item">
        <span class="host-info-label" style="color: var(--text-secondary, #666); font-weight: 500;">Format:</span>
        <span class="host-info-format" style="font-family: monospace;">--</span>
      </div>
      <div class="host-info-item">
        <span class="host-info-label" style="color: var(--text-secondary, #666); font-weight: 500;">Sample Rate:</span>
        <span class="host-info-sample-rate" style="font-family: monospace;">--</span>
      </div>
      <div class="host-info-item">
        <span class="host-info-label" style="color: var(--text-secondary, #666); font-weight: 500;">Channels:</span>
        <span class="host-info-channels" style="font-family: monospace;">--</span>
      </div>
      <div class="host-info-item">
        <span class="host-info-label" style="color: var(--text-secondary, #666); font-weight: 500;">Bit Depth:</span>
        <span class="host-info-bits" style="font-family: monospace;">--</span>
      </div>
    </div>
  </div>

  <!-- Section: Spectrum Analyzer -->
  <div class="spectrum-section-inline" style="margin: 16px 0; padding: 12px; background: var(--bg-secondary, #f5f5f5); border-radius: 8px;">
    <h4 style="margin: 0 0 8px 0;">Spectrum Analyzer</h4>
    <div class="frequency-analyzer" style="width: 100%; min-height: 120px;">
      <canvas class="spectrum-canvas" width="800" height="120" style="display: block; width: 100%; height: 120px;"></canvas>
    </div>
  </div>

  <!-- Section: Bottom Control Row -->
  <div class="audio-control-row-inline" style="display: flex; gap: 16px; align-items: flex-start;">

    <!-- Block: Track Selection -->
    <div class="demo-track-container" style="display: flex; flex-direction: column; gap: 4px; flex-shrink: 0;">
      <label style="font-weight: 500; font-size: 0.9em;">Load Track</label>
      <div class="demo-track-select-row">
        <select id="${selectId}" class="demo-audio-select">
          <option value="">Pick a track...</option>
          ${Object.keys(this.config.demoTracks || {})
            .map(
              (key) =>
                `<option value="${key}">${this.formatTrackName(key)}</option>`,
            )
            .join("")}
        </select>
        <button type="button" class="file-upload-btn">📁</button>
      </div>
    </div>

    <!-- Block: Playback Controls -->
    <div class="audio-playback-controls" style="display: flex; flex-direction: column; gap: 4px; flex: 1; min-width: 250px;">
      <label style="font-weight: 500; font-size: 0.9em;">Play</label>
      <div class="audio-status-display">
        <div class="status-display-row">
          <div class="audio-status-text">No audio</div>
          <div class="audio-time-display">
            <span class="audio-position">--:--</span> / <span class="audio-duration">--:--</span>
          </div>
        </div>
        <div class="audio-progress-row">
          <div class="audio-progress-bar">
            <div class="audio-progress-fill"></div>
          </div>
        </div>
      </div>
      <div class="playback-controls-row">
        <button type="button" class="listen-button" disabled>▶️</button>
        <button type="button" class="pause-button" disabled>⏸️</button>
        <button type="button" class="stop-button" disabled>⏹️</button>
      </div>
    </div>

    <!-- Block: Mini EQ -->
    ${
      this.config.enableEQ
        ? `
    <div class="audio-eq-controls" style="display: flex; flex-direction: column; gap: 4px; flex-shrink: 0;">
      <label style="font-weight: 500; font-size: 0.9em;">EQ</label>
      <div class="eq-control-section">
        <div class="eq-mini-graph">
          <canvas class="eq-mini-canvas" width="160" height="50"></canvas>
        </div>
        <div class="eq-controls-row">
          <div class="eq-info-text">
            <span class="eq-filter-count">#0</span>
            <span class="eq-gain-compensation">0dB</span>
          </div>
          <div class="eq-toggle-buttons" tabindex="0">
            <button type="button" class="eq-toggle-btn eq-on-btn active">On</button>
            <button type="button" class="eq-toggle-btn eq-off-btn">Off</button>
          </div>
        </div>
      </div>
    </div>
    `
        : ""
    }

    <!-- Block: Audio Metrics -->
    <div class="audio-metrics-block" style="display: flex; flex-direction: column; gap: 4px; flex-shrink: 0;">
      <label style="font-weight: 500; font-size: 0.9em;">Loudness</label>
      <div class="metrics-display">
        <div class="metric-section">
          <div class="metric-label">LUFS M/S</div>
          <div class="metric-row">
            <span class="metric-label-small">M</span>
            <span id="metrics-lufs-m" class="metric-value loudness-momentary">-∞</span>
            <span class="metric-separator">/</span>
            <span class="metric-label-small">S</span>
            <span id="metrics-lufs-s" class="metric-value loudness-shortterm">-∞</span>
          </div>
        </div>
        <div class="metric-section">
          <div class="metric-label">ReplayGain</div>
          <div class="metric-row">
            <span id="metrics-replay-gain" class="metric-value">--</span>
            <span class="metric-separator">/</span>
            <span id="metrics-peak" class="metric-value">--</span>
          </div>
        </div>
      </div>
    </div>

  </div>
</div>

    `;

    this.container.innerHTML = html;
    this.cacheUIElements();
  }

  private cacheUIElements(): void {
    this.demoSelect = this.container.querySelector(".demo-audio-select");
    this.playBtn = this.container.querySelector(".listen-button");
    this.pauseBtn = this.container.querySelector(".pause-button");
    this.stopBtn = this.container.querySelector(".stop-button");
    this.eqOnBtn = this.container.querySelector(".eq-on-btn");
    this.eqOffBtn = this.container.querySelector(".eq-off-btn");
    this.eqMiniCanvas = this.container.querySelector(".eq-mini-canvas");

    this.statusText = this.container.querySelector(".audio-status-text");
    this.positionText = this.container.querySelector(".audio-position");
    this.durationText = this.container.querySelector(".audio-duration");
    this.progressFill = this.container.querySelector(".audio-progress-fill");
    this.spectrumCanvas = this.container.querySelector(".spectrum-canvas");
    this.loudnessDisplayMomentary = this.container.querySelector(
      ".loudness-momentary",
    );
    this.loudnessDisplayShortterm = this.container.querySelector(
      ".loudness-shortterm",
    );
    this.replayGainDisplay = this.container.querySelector(
      "#metrics-replay-gain",
    );
    this.peakDisplay = this.container.querySelector("#metrics-peak");

    // Cache host info elements
    this.hostInfoFormat = this.container.querySelector(".host-info-format");
    this.hostInfoSampleRate = this.container.querySelector(
      ".host-info-sample-rate",
    );
    this.hostInfoChannels = this.container.querySelector(".host-info-channels");
    this.hostInfoBits = this.container.querySelector(".host-info-bits");

    console.log("[AudioPlayer] Cached ReplayGain elements:", {
      replayGainDisplay: !!this.replayGainDisplay,
      peakDisplay: !!this.peakDisplay,
    });

    if (this.spectrumCanvas) {
      this.spectrumCtx = this.spectrumCanvas.getContext("2d");
      // Initialize spectrum analyzer component
      if (this.config.enableSpectrum) {
        // Detect system color scheme
        const prefersDark = window.matchMedia(
          "(prefers-color-scheme: dark)",
        ).matches;
        const colorScheme = prefersDark ? "dark" : "light";

        this.spectrumAnalyzer = new SpectrumAnalyzerComponent({
          canvas: this.spectrumCanvas,
          pollInterval: 100,
          minFreq: 20,
          maxFreq: 20000,
          dbRange: 60,
          colorScheme: colorScheme,
          showLabels: true,
          showGrid: true,
        });
      }
    }

    // Cache ReplayGain elements
    this.replayGainContainer =
      this.container.querySelector(".replay-gain-info");
    console.log("[ReplayGain] Container cached:", !!this.replayGainContainer);
    if (this.replayGainContainer) {
      console.log("[ReplayGain] Container element:", this.replayGainContainer);
      console.log(
        "[ReplayGain] Container initial display:",
        this.replayGainContainer.style.display,
      );
    }
  }

  private setupEventListeners(): void {
    // Handle window resize
    this.resizeHandler = () => {
      // Spectrum analyzer handles its own resizing
    };
    window.addEventListener("resize", this.resizeHandler);

    // Demo track selection
    this.demoSelect?.addEventListener("change", async (e) => {
      const trackName = (e.target as HTMLSelectElement).value;
      if (trackName) {
        await this.loadDemoTrack(trackName);
        this.callbacks.onTrackChange?.(trackName);
      } else {
        this.clearAudio();
      }
    });

    // File upload button
    const uploadBtn = this.container.querySelector(".file-upload-btn");
    uploadBtn?.addEventListener("click", async () => {
      try {
        const selectedPath = await open({
          multiple: false,
          filters: [
            {
              name: "Audio",
              extensions: ["wav", "flac", "mp3", "ogg", "m4a", "aac", "opus"],
            },
          ],
        });

        if (typeof selectedPath === "string") {
          await this.loadAudioFilePath(selectedPath);
        }
      } catch (error) {
        console.error("File selection failed:", error);
        this.callbacks.onError?.("File selection failed: " + error);
      }
    });

    // Playback controls
    this.playBtn?.addEventListener("click", () => {
      // If truly paused, resume; otherwise, play from beginning
      if (this.isAudioPaused) {
        this.resume();
      } else {
        this.play();
      }
    });

    this.pauseBtn?.addEventListener("click", () => {
      this.handlePauseClick();
    });

    this.stopBtn?.addEventListener("click", () => {
      this.stop();
    });

    // EQ controls
    if (this.eqOnBtn && this.visualEQConfig) {
      this.eqOnBtn.addEventListener("click", () => {
        this.visualEQConfig!.setEQEnabled(true);
        this.updateEQButtonStates(true);
        this.callbacks.onEQToggle?.(true);
      });
    }

    if (this.eqOffBtn && this.visualEQConfig) {
      this.eqOffBtn.addEventListener("click", () => {
        this.visualEQConfig!.setEQEnabled(false);
        this.updateEQButtonStates(false);
        this.callbacks.onEQToggle?.(false);
      });
    }

    // Filter configuration collapsible toggle
    const filterConfigHeader = this.container.querySelector(
      ".filter-config-header",
    );
    const filterConfigToggle = this.container.querySelector(
      ".filter-config-toggle",
    );
    const eqTableContainer = this.container.querySelector(
      ".eq-table-container",
    );

    if (filterConfigHeader && filterConfigToggle && eqTableContainer) {
      filterConfigHeader.addEventListener("click", () => {
        const isCollapsed =
          (eqTableContainer as HTMLElement).style.display === "none";

        if (isCollapsed) {
          // Expand
          (eqTableContainer as HTMLElement).style.display = "block";
          (filterConfigToggle as HTMLElement).style.transform =
            "rotate(180deg)";
        } else {
          // Collapse
          (eqTableContainer as HTMLElement).style.display = "none";
          (filterConfigToggle as HTMLElement).style.transform = "rotate(0deg)";
        }
      });
    }
  }

  // ===== AUDIO LOADING METHODS =====

  async loadDemoTrack(trackName: string): Promise<void> {
    const trackPath = this.config.demoTracks?.[trackName];
    if (!trackPath) {
      throw new Error(`Demo track "${trackName}" not found`);
    }

    try {
      // Use backend command to resolve demo track path
      // This works in both dev and production modes
      console.log(`[AudioPlayer] Resolving demo track path: ${trackPath}`);
      const resolvedPath = await invoke<string>("resolve_demo_track_path", {
        relativePath: trackPath,
      });
      console.log(`[AudioPlayer] Resolved demo track path: ${resolvedPath}`);

      await this.loadAudioFilePath(resolvedPath);
    } catch (error) {
      console.error(`Failed to load demo track "${trackName}":`, error);
      throw error; // Re-throw so caller knows it failed
    }
  }

  async loadAudioFilePath(filePath: string): Promise<void> {
    try {
      this.currentAudioPath = filePath;
      await this.streamingManager.loadAudioFilePath(filePath);
      this.setStatus("Loading...");

      // Analyze ReplayGain in the background
      this.analyzeReplayGain(filePath);
    } catch (error) {
      console.error("Failed to load audio file:", error);
      this.callbacks.onError?.(`Failed to load audio file: ${error}`);
    }
  }

  private clearAudio(): void {
    this.stop();
    this.audioBuffer = null;
    this.setListenButtonEnabled(false);
    this.showAudioStatus(false);
    this.setStatus("No audio selected");

    // Clear ReplayGain data
    this.currentReplayGain = null;

    // Reset displays
    if (this.replayGainDisplay) {
      this.replayGainDisplay.textContent = "--";
    }
    if (this.peakDisplay) {
      this.peakDisplay.textContent = "--";
    }

    // Hide ReplayGain display
    if (this.replayGainContainer) {
      this.replayGainContainer.style.display = "none";
    }
  }

  // ===== UI HELPER METHODS =====

  private formatTrackName(key: string): string {
    return key.replace(/_/g, " ").replace(/\b\w/g, (l) => l.toUpperCase());
  }

  private updateAudioInfo(): void {
    if (this.audioBuffer && this.durationText) {
      const duration = this.audioBuffer.duration;
      this.durationText.textContent = this.formatTime(duration);
    }
  }

  private updateHostInfo(info: AudioFileInfo): void {
    if (this.hostInfoFormat) {
      this.hostInfoFormat.textContent = info.format.toUpperCase();
    }
    if (this.hostInfoSampleRate) {
      this.hostInfoSampleRate.textContent = `${(info.sample_rate / 1000).toFixed(1)} kHz`;
    }
    if (this.hostInfoChannels) {
      const channelLabel =
        info.channels === 1
          ? "Mono"
          : info.channels === 2
            ? "Stereo"
            : `${info.channels}ch`;
      this.hostInfoChannels.textContent = channelLabel;
    }
    if (this.hostInfoBits) {
      this.hostInfoBits.textContent = `${info.bits_per_sample}-bit`;
    }
  }

  private formatTime(seconds: number): string {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, "0")}`;
  }

  private setStatus(status: string): void {
    if (this.statusText) {
      this.statusText.textContent = status;
    }
  }

  private setListenButtonEnabled(enabled: boolean): void {
    if (this.playBtn) {
      this.playBtn.disabled = !enabled;
    }
  }

  private showAudioStatus(show: boolean): void {
    // Show/hide time display and progress bar
    const timeDisplay = this.container.querySelector(".audio-time-display");
    const progressBar = this.container.querySelector(".audio-progress-row");

    if (timeDisplay) {
      (timeDisplay as HTMLElement).style.display = show ? "block" : "none";
    }
    if (progressBar) {
      (progressBar as HTMLElement).style.display = show ? "block" : "none";
    }
  }

  // ===== PLAYBACK METHODS =====

  async play(): Promise<void> {
    try {
      if (!this.currentAudioPath) {
        this.callbacks.onError?.("No audio file selected");
        return;
      }

      // Get filter parameters from VisualEQConfig if available
      let filters: Array<{ frequency: number; q: number; gain: number }> = [];
      if (this.visualEQConfig && this.visualEQConfig.isEQEnabled()) {
        const filterParams = this.visualEQConfig.getFilterParams();
        filters = filterParams
          .filter((p) => p.enabled)
          .map((p) => ({
            frequency: p.frequency,
            q: p.q,
            gain: p.gain,
          }));
      }

      // Enable monitoring BEFORE starting playback so the decoder thread has access to monitors
      if (this.spectrumAnalyzer) {
        await this.spectrumAnalyzer.start();
      }
      await this.streamingManager.enableLoudnessMonitoring();

      await this.streamingManager.play(filters);

      this.isAudioPlaying = true;
      this.isAudioPaused = false;
      this.audioStartTime = Date.now();

      // Start loudness polling
      this.streamingManager.startLoudnessPolling(100, (loudnessInfo) => {
        this.updateLoudnessDisplay(loudnessInfo);
      });

      // Update UI
      this.updatePlaybackUI();
      this.setStatus("Playing");

      this.callbacks.onPlay?.();
    } catch (error) {
      console.error("Failed to play audio:", error);
      this.callbacks.onError?.("Failed to play audio: " + error);
    }
  }

  async pause(): Promise<void> {
    try {
      await this.streamingManager.pause();
      this.isAudioPlaying = false;
      this.isAudioPaused = true;
      this.audioPauseTime = Date.now();

      // Stop spectrum analyzer after 3 seconds
      if (this.spectrumAnalyzer) {
        // Clear any existing timer
        if (this.spectrumStopTimer !== null) {
          clearTimeout(this.spectrumStopTimer);
        }
        // Set timer to stop spectrum analyzer after 3 seconds
        this.spectrumStopTimer = window.setTimeout(async () => {
          if (this.spectrumAnalyzer && this.isAudioPaused) {
            await this.spectrumAnalyzer.stop();
          }
          this.spectrumStopTimer = null;
        }, 3000);
      }

      // Stop loudness monitoring
      this.streamingManager.stopLoudnessPolling();

      // Update UI
      this.updatePlaybackUI();
      this.setStatus("Paused");

      this.callbacks.onStop?.();
    } catch (error) {
      console.error("Failed to pause audio:", error);
      this.callbacks.onError?.("Failed to pause audio: " + error);
    }
  }

  async stop(): Promise<void> {
    try {
      await this.streamingManager.stop();
      this.isAudioPlaying = false;
      this.isAudioPaused = false;

      // Clear spectrum stop timer if it exists
      if (this.spectrumStopTimer !== null) {
        clearTimeout(this.spectrumStopTimer);
        this.spectrumStopTimer = null;
      }

      // Stop spectrum analyzer immediately
      if (this.spectrumAnalyzer) {
        await this.spectrumAnalyzer.stop();
      }

      // Stop loudness monitoring
      this.streamingManager.stopLoudnessPolling();
      await this.streamingManager.disableLoudnessMonitoring();
      this.updateLoudnessDisplay(null); // Reset display

      // Reset position display
      if (this.positionText) {
        this.positionText.textContent = "--:--";
      }
      if (this.progressFill) {
        this.progressFill.style.width = "0%";
      }

      // Update UI
      this.updatePlaybackUI();
      this.setStatus("Stopped");

      this.callbacks.onStop?.();
    } catch (error) {
      console.error("Failed to stop audio:", error);
      this.callbacks.onError?.("Failed to stop audio: " + error);
    }
  }

  async resume(): Promise<void> {
    try {
      // Clear spectrum stop timer if it exists (user resumed before 3 seconds elapsed)
      if (this.spectrumStopTimer !== null) {
        clearTimeout(this.spectrumStopTimer);
        this.spectrumStopTimer = null;
      }

      await this.streamingManager.resume();
      this.isAudioPlaying = true;
      this.isAudioPaused = false;

      // Note: Spectrum analyzer will be started in handleStateChange() when backend confirms "playing"

      // Restart loudness monitoring
      await this.streamingManager.enableLoudnessMonitoring();
      this.streamingManager.startLoudnessPolling(100, (loudnessInfo) => {
        this.updateLoudnessDisplay(loudnessInfo);
      });

      // Update UI
      this.updatePlaybackUI();
      this.setStatus("Playing");

      this.callbacks.onPlay?.();
    } catch (error) {
      console.error("Failed to resume audio:", error);
      this.callbacks.onError?.("Failed to resume audio: " + error);
    }
  }

  private handlePauseClick(): void {
    this.pauseClickCount++;

    if (this.pauseClickCount === 1) {
      // First click - pause
      this.pause();

      // Set a timer to reset click count
      this.pauseClickTimer = window.setTimeout(() => {
        this.pauseClickCount = 0;
        this.pauseClickTimer = null;
      }, 300);
    } else if (this.pauseClickCount === 2) {
      // Second click - stop
      this.stop();
      this.pauseClickCount = 0;

      // Clear the timer
      if (this.pauseClickTimer) {
        clearTimeout(this.pauseClickTimer);
        this.pauseClickTimer = null;
      }
    }
  }

  private updatePlaybackUI(): void {
    const isPlaying = this.isAudioPlaying;
    const isPaused = this.isAudioPaused;

    // Update button states based on playback status
    if (this.playBtn) {
      this.playBtn.disabled = isPlaying;
    }

    if (this.pauseBtn) {
      this.pauseBtn.disabled = !isPlaying;
    }

    if (this.stopBtn) {
      this.stopBtn.disabled = !isPlaying && !isPaused;
    }
  }

  private updateEQButtonStates(enabled: boolean): void {
    if (this.eqOnBtn) {
      if (enabled) {
        this.eqOnBtn.classList.add("active");
      } else {
        this.eqOnBtn.classList.remove("active");
      }
    }

    if (this.eqOffBtn) {
      if (enabled) {
        this.eqOffBtn.classList.remove("active");
      } else {
        this.eqOffBtn.classList.add("active");
      }
    }
  }

  // ===== LOUDNESS MONITORING =====

  private updateLoudnessDisplay(
    loudnessInfo: {
      momentary_lufs: number;
      shortterm_lufs: number;
      peak: number;
    } | null,
  ): void {
    console.log(
      "[AudioPlayer] updateLoudnessDisplay called with:",
      loudnessInfo,
    );
    console.log("[AudioPlayer] Elements cached:", {
      momentary: !!this.loudnessDisplayMomentary,
      shortterm: !!this.loudnessDisplayShortterm,
      peak: !!this.peakDisplay,
      replayGain: !!this.replayGainDisplay,
    });

    if (!loudnessInfo) {
      // Reset LUFS displays to -∞ when no data
      if (this.loudnessDisplayMomentary) {
        this.loudnessDisplayMomentary.textContent = "-∞";
      }
      if (this.loudnessDisplayShortterm) {
        this.loudnessDisplayShortterm.textContent = "-∞";
      }
      // Note: Don't reset ReplayGain and Peak - they're set by analyzeReplayGain() and should persist
      return;
    }

    // Update momentary LUFS (M)
    if (this.loudnessDisplayMomentary) {
      const mValue = loudnessInfo.momentary_lufs;
      const text =
        mValue !== null && isFinite(mValue) ? mValue.toFixed(1) : "-∞";
      console.log("[AudioPlayer] Setting momentary LUFS to:", text);
      this.loudnessDisplayMomentary.textContent = text;
    } else {
      console.warn("[AudioPlayer] loudnessDisplayMomentary element not found");
    }

    // Update short-term LUFS (S)
    if (this.loudnessDisplayShortterm) {
      const sValue = loudnessInfo.shortterm_lufs;
      const text =
        sValue !== null && isFinite(sValue) ? sValue.toFixed(1) : "-∞";
      console.log("[AudioPlayer] Setting shortterm LUFS to:", text);
      this.loudnessDisplayShortterm.textContent = text;
    } else {
      console.warn("[AudioPlayer] loudnessDisplayShortterm element not found");
    }

    // Note: Peak display is handled by ReplayGain analysis and should not be overwritten
    // The loudnessInfo.peak is the real-time peak, but we want to show the ReplayGain peak

    // Display stored ReplayGain and Peak if available (keep them displayed during playback)
    if (this.currentReplayGain) {
      if (this.replayGainDisplay) {
        this.replayGainDisplay.textContent = `${this.currentReplayGain.gain >= 0 ? "+" : ""}${this.currentReplayGain.gain.toFixed(2)} dB`;
      }
      if (this.peakDisplay) {
        this.peakDisplay.textContent = this.currentReplayGain.peak.toFixed(2);
      }
    }
  }

  private async analyzeReplayGain(filePath: string): Promise<void> {
    try {
      console.log("[AudioPlayer] Analyzing ReplayGain for:", filePath);
      console.log("[AudioPlayer] ReplayGain display elements:", {
        replayGainDisplay: !!this.replayGainDisplay,
        peakDisplay: !!this.peakDisplay,
      });

      const result = await invoke<ReplayGainInfo>("analyze_replaygain", {
        filePath,
      });

      this.currentReplayGain = result;
      console.log("[AudioPlayer] ReplayGain analysis complete:", result);

      // Update ReplayGain display
      if (this.replayGainDisplay) {
        const gainText = `${result.gain >= 0 ? "+" : ""}${result.gain.toFixed(2)} dB`;
        this.replayGainDisplay.textContent = gainText;
        console.log("[AudioPlayer] Set ReplayGain display to:", gainText);
      } else {
        console.warn("[AudioPlayer] ReplayGain display element not found!");
      }

      // Update Peak display
      if (this.peakDisplay) {
        const peakText = result.peak.toFixed(2);
        this.peakDisplay.textContent = peakText;
        console.log("[AudioPlayer] Set Peak display to:", peakText);
      } else {
        console.warn("[AudioPlayer] Peak display element not found!");
      }
    } catch (error) {
      console.error("[AudioPlayer] Failed to analyze ReplayGain:", error);
      // Don't show error to user, just log it
    }
  }

  // ===== PUBLIC API METHODS =====

  getCurrentTrack(): string | null {
    return this.demoSelect?.value || null;
  }

  isPlaying(): boolean {
    return this.isAudioPlaying;
  }

  // ===== EQ FILTER MANAGEMENT - delegate to VisualEQConfig =====

  updateFilterParams(filterParams: Partial<ExtendedFilterParam>[]): void {
    if (this.visualEQConfig) {
      this.visualEQConfig.updateFilterParams(filterParams);
    }
  }

  // Clear all EQ filters
  clearEQFilters(): void {
    if (this.visualEQConfig) {
      this.visualEQConfig.clearEQFilters();
    }
  }

  setEQEnabled(enabled: boolean): void {
    this.eqEnabled = enabled; // Update local property for backward compatibility
    if (this.visualEQConfig) {
      this.visualEQConfig.setEQEnabled(enabled);
    }
  }

  isEQEnabled(): boolean {
    // For backward compatibility, always return the local property
    // since tests might set it directly
    return this.eqEnabled;
  }

  getFilterParams(): ExtendedFilterParam[] {
    return this.visualEQConfig?.getFilterParams() ?? [];
  }

  // ===== OUTPUT DEVICE MANAGEMENT =====

  setOutputDevice(deviceId: string): void {
    this.outputDeviceId = deviceId;
    // Note: Actual device routing would need to be implemented
    // if using Web Audio API directly. With streaming manager,
    // the backend handles device selection.
  }

  // ===== CLEANUP =====

  destroy(): void {
    // Stop audio
    this.stop();

    // Clear spectrum stop timer if it exists
    if (this.spectrumStopTimer !== null) {
      clearTimeout(this.spectrumStopTimer);
      this.spectrumStopTimer = null;
    }

    // Destroy VisualEQConfig
    if (this.visualEQConfig) {
      this.visualEQConfig.destroy();
      this.visualEQConfig = null;
    }

    // Remove event listeners
    if (this.resizeHandler) {
      window.removeEventListener("resize", this.resizeHandler);
    }

    // Clear audio context
    if (this.audioContext) {
      this.audioContext.close();
      this.audioContext = null;
    }
  }
}
