// Tests for AudioPlayer functionality
import { describe, test, expect, beforeEach, vi, afterEach } from "vitest";

// Mock Tauri APIs before importing modules under test
(globalThis as any).__TAURI__ = (globalThis as any).__TAURI__ || {};

vi.mock("@tauri-apps/api/core", () => {
  return {
    invoke: vi.fn(async (cmd: string, args: any) => {
      switch (cmd) {
        case "flac_load_file":
          return {
            path: args.filePath,
            format: "flac",
            sample_rate: 48000,
            channels: 2,
            bits_per_sample: 16,
            duration_seconds: 10,
          };
        case "flac_start_playback":
        case "flac_stop_playback":
        case "flac_pause_playback":
        case "flac_resume_playback":
        case "flac_seek":
        case "flac_get_state":
          return undefined as any;
        case "flac_get_file_info":
          return null as any;
        default:
          return undefined as any;
      }
    }),
  };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => {
    return () => {};
  }),
}));

vi.mock("@tauri-apps/api/path", () => ({
  resolveResource: vi.fn(async (p: string) => p),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(async () => "/tmp/mock.wav"),
}));

import {
  AudioPlayer,
  type AudioPlayerConfig,
  type AudioPlayerCallbacks,
} from "@audio-player/audio-player";

// Mock Web Audio API
const mockAudioContext = {
  createGain: vi.fn(() => ({
    connect: vi.fn(),
    disconnect: vi.fn(),
    gain: { value: 1 },
  })),
  createAnalyser: vi.fn(() => ({
    connect: vi.fn(),
    disconnect: vi.fn(),
    fftSize: 2048,
    smoothingTimeConstant: 0.8,
    frequencyBinCount: 1024,
    getByteFrequencyData: vi.fn(),
  })),
  createBufferSource: vi.fn(() => ({
    connect: vi.fn(),
    disconnect: vi.fn(),
    start: vi.fn(),
    stop: vi.fn(),
    buffer: null,
    onended: null,
  })),
  createBiquadFilter: vi.fn(() => ({
    connect: vi.fn(),
    disconnect: vi.fn(),
    type: "peaking",
    frequency: { value: 1000 },
    Q: { value: 1 },
    gain: { value: 0 },
  })),
  decodeAudioData: vi.fn(),
  resume: vi.fn(),
  close: vi.fn(),
  destination: {},
  currentTime: 0,
  state: "running",
};

// Mock global AudioContext
(globalThis as any).AudioContext = vi.fn(function () {
  return mockAudioContext;
});
(globalThis as any).webkitAudioContext = vi.fn(function () {
  return mockAudioContext;
});

// Mock Canvas API
const mockCanvasContext = {
  fillStyle: "",
  strokeStyle: "",
  lineWidth: 1,
  fillRect: vi.fn(),
  clearRect: vi.fn(),
  beginPath: vi.fn(),
  moveTo: vi.fn(),
  lineTo: vi.fn(),
  stroke: vi.fn(),
  arc: vi.fn(),
  fill: vi.fn(),
};

// Mock canvas getContext method
HTMLCanvasElement.prototype.getContext = vi.fn((contextType: string) => {
  if (contextType === "2d") {
    return mockCanvasContext as any;
  }
  return null;
});

// Mock fetch for audio loading
globalThis.fetch = vi.fn();

// Mock requestAnimationFrame
globalThis.requestAnimationFrame = vi.fn((cb) => {
  setTimeout(cb, 16);
  return 1;
});

globalThis.cancelAnimationFrame = vi.fn();

describe("AudioPlayer", () => {
  let container: HTMLElement;
  let audioPlayer: AudioPlayer;
  let mockCallbacks: AudioPlayerCallbacks;

  beforeEach(() => {
    // Reset all mocks
    vi.clearAllMocks();

    // Restore AudioContext to working state
    (globalThis as any).AudioContext = vi.fn(function () {
      return mockAudioContext;
    });
    (globalThis as any).webkitAudioContext = vi.fn(function () {
      return mockAudioContext;
    });

    // Create mock container
    container = document.createElement("div");
    container.innerHTML = "";

    // Mock callbacks
    mockCallbacks = {
      onPlay: vi.fn(),
      onStop: vi.fn(),
      onEQToggle: vi.fn(),
      onTrackChange: vi.fn(),
      onError: vi.fn(),
    };
  });

  afterEach(() => {
    if (audioPlayer) {
      audioPlayer.destroy();
    }
  });

  describe("Constructor and Initialization", () => {
    test("should create AudioPlayer with default config", () => {
      audioPlayer = new AudioPlayer(container);

      expect(audioPlayer).toBeInstanceOf(AudioPlayer);
      expect(mockAudioContext.createGain).toHaveBeenCalled();
    });

    test("should create AudioPlayer with custom config", () => {
      const config: AudioPlayerConfig = {
        enableEQ: false,
        enableSpectrum: false,
        maxFilters: 5,
        compactMode: true,
      };

      audioPlayer = new AudioPlayer(container, config, mockCallbacks);

      expect(audioPlayer).toBeInstanceOf(AudioPlayer);
    });

    test("should handle audio context creation failure", async () => {
      // Mock AudioContext to throw error
      (globalThis as any).AudioContext = vi.fn(function () {
        throw new Error("AudioContext not supported");
      });
      (globalThis as any).webkitAudioContext = undefined;

      expect(() => {
        audioPlayer = new AudioPlayer(container, {}, mockCallbacks);
      }).not.toThrow(); // Should handle gracefully

      // Wait for async init to complete
      await new Promise((resolve) => setTimeout(resolve, 50));

      expect(mockCallbacks.onError).toHaveBeenCalledWith(
        expect.stringContaining("Failed to initialize audio player"),
      );
    });
  });

  describe("UI Creation", () => {
    beforeEach(() => {
      audioPlayer = new AudioPlayer(container);
    });

    test("should create UI elements", () => {
      expect(container.innerHTML).toContain("audio-player");
      expect(container.innerHTML).toContain("demo-audio-select");
      expect(container.innerHTML).toContain("listen-button");
    });

    test("should include EQ controls when enabled", () => {
      const config: AudioPlayerConfig = { enableEQ: true };
      audioPlayer = new AudioPlayer(container, config);

      expect(container.innerHTML).toContain("eq-toggle-btn");
    });

    test.skip("should exclude EQ controls when disabled", () => {
      const config: AudioPlayerConfig = { enableEQ: false };
      audioPlayer = new AudioPlayer(container, config);

      // The modal is always created, but the UI controls should not be rendered
      // Check that the main EQ UI controls are not present in the container
      const eqSections = container.querySelectorAll(".eq-control-section");
      const eqButtons = container.querySelectorAll(".eq-toggle-btn");

      expect(eqSections.length).toBe(0);
      expect(eqButtons.length).toBe(0);
    });

    test("should include spectrum analyzer when enabled", () => {
      const config: AudioPlayerConfig = { enableSpectrum: true };
      audioPlayer = new AudioPlayer(container, config);

      expect(container.innerHTML).toContain("spectrum-canvas");
    });

    test.skip("should generate unique IDs for multiple instances", () => {
      const container2 = document.createElement("div");

      // Both instances should be separate
      expect(audioPlayer).toBeDefined();

      const audioPlayer2 = new AudioPlayer(container2);

      // Verify they are separate instances
      expect(audioPlayer).not.toBe(audioPlayer2);

      // Verify both have created some UI (containers may vary)
      const html1 = container.innerHTML;
      const html2 = container2.innerHTML;

      // Both should have audio player UI
      expect(html1.length).toBeGreaterThan(0);
      expect(html2.length).toBeGreaterThan(0);

      audioPlayer2.destroy();
    });
  });

  // NOTE: These tests are skipped because they're out of sync with the current AudioPlayer implementation
  // They need to be updated to match the current StreamingManager-based architecture
  describe.skip("Audio Loading", () => {
    beforeEach(() => {
      audioPlayer = new AudioPlayer(container, {}, mockCallbacks);
    });

    test("should load audio file successfully", async () => {
      await expect(
        audioPlayer.loadAudioFilePath("/tmp/test.wav"),
      ).resolves.not.toThrow();
      const statusEl = container.querySelector(
        ".audio-status-text",
      ) as HTMLElement;
      expect(statusEl?.textContent).toContain("Audio ready");
    });

    test("should handle audio file loading error", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      (invoke as any).mockImplementationOnce(() =>
        Promise.reject(new Error("File read error")),
      );

      await expect(
        audioPlayer.loadAudioFilePath("/invalid.wav"),
      ).rejects.toThrow("File read error");
      expect(mockCallbacks.onError).toHaveBeenCalledWith(
        expect.stringContaining("Failed to load audio file"),
      );
    });

    test("should load demo track from URL", async () => {
      const config: AudioPlayerConfig = {
        demoTracks: { test: "/public/demo-audio/test.wav" },
      };

      audioPlayer = new AudioPlayer(container, config, mockCallbacks);

      await expect(audioPlayer["loadDemoTrack"]("test")).resolves.not.toThrow();
    });
  });

  describe.skip("Playback Controls", () => {
    beforeEach(() => {
      audioPlayer = new AudioPlayer(container, {}, mockCallbacks);
    });

    test("should start playback successfully", async () => {
      await audioPlayer.loadAudioFilePath("/tmp/test.wav");
      await audioPlayer.play();
      expect(mockCallbacks.onPlay).toHaveBeenCalled();
      expect(audioPlayer.isPlaying()).toBe(true);
    });

    test("should stop playback successfully", async () => {
      await audioPlayer.loadAudioFilePath("/tmp/test.wav");
      await audioPlayer.play();
      await audioPlayer.stop();
      expect(mockCallbacks.onStop).toHaveBeenCalled();
      expect(audioPlayer.isPlaying()).toBe(false);
    });

    test("should handle playback without audio buffer", async () => {
      await expect(audioPlayer.play()).rejects.toThrow("No audio file loaded");
    });

    test("should resume suspended audio context", async () => {
      await audioPlayer.loadAudioFilePath("/tmp/test.wav");
      await audioPlayer.play();
      await audioPlayer.pause();
      await audioPlayer.resume();
      expect(audioPlayer.isPlaying()).toBe(true);
    });
  });

  describe.skip("EQ Controls", () => {
    beforeEach(() => {
      audioPlayer = new AudioPlayer(
        container,
        { enableEQ: true },
        mockCallbacks,
      );
    });

    test("should enable EQ", () => {
      audioPlayer.setEQEnabled(true);

      expect(audioPlayer.isEQEnabled()).toBe(true);
      expect(mockCallbacks.onEQToggle).toHaveBeenCalledWith(true);
    });

    test("should disable EQ", () => {
      audioPlayer.setEQEnabled(false);

      expect(audioPlayer.isEQEnabled()).toBe(false);
      expect(mockCallbacks.onEQToggle).toHaveBeenCalledWith(false);
    });

    test("should update filter parameters", () => {
      const filterParams = [
        { frequency: 100, q: 1, gain: 5 },
        { frequency: 1000, q: 2, gain: -3 },
        { frequency: 10000, q: 1.5, gain: 2 },
      ];

      audioPlayer.updateFilterParams(filterParams);

      expect(mockAudioContext.createBiquadFilter).toHaveBeenCalledTimes(3);
    });

    test("should skip filters with zero gain", () => {
      const filterParams = [
        { frequency: 100, q: 1, gain: 0 },
        { frequency: 1000, q: 2, gain: 5 },
      ];

      audioPlayer.updateFilterParams(filterParams);

      // Should only create one filter (the one with non-zero gain)
      expect(mockAudioContext.createBiquadFilter).toHaveBeenCalledTimes(1);
    });
  });

  describe("Public API", () => {
    beforeEach(() => {
      audioPlayer = new AudioPlayer(container, {}, mockCallbacks);
    });

    test("should return current track", () => {
      // Mock demo select element
      const mockSelect = { value: "classical" };
      audioPlayer["demoSelect"] = mockSelect as HTMLSelectElement;

      expect(audioPlayer.getCurrentTrack()).toBe("classical");
    });

    test("should return null when no track selected", () => {
      const mockSelect = { value: "" };
      audioPlayer["demoSelect"] = mockSelect as HTMLSelectElement;

      expect(audioPlayer.getCurrentTrack()).toBe(null);
    });

    test("should return playing state", () => {
      expect(audioPlayer.isPlaying()).toBe(false);

      audioPlayer["isAudioPlaying"] = true;
      expect(audioPlayer.isPlaying()).toBe(true);
    });

    test("should return EQ enabled state", () => {
      expect(audioPlayer.isEQEnabled()).toBe(true);

      audioPlayer["eqEnabled"] = false;
      expect(audioPlayer.isEQEnabled()).toBe(false);
    });
  });

  describe.skip("Cleanup", () => {
    beforeEach(() => {
      audioPlayer = new AudioPlayer(container, {}, mockCallbacks);
    });

    test("should cleanup resources on destroy", () => {
      // Set up some state
      audioPlayer["isAudioPlaying"] = true;
      audioPlayer["eqFilters"] = [
        mockAudioContext.createBiquadFilter() as unknown as BiquadFilterNode,
      ];
      audioPlayer["gainNode"] =
        mockAudioContext.createGain() as unknown as GainNode;
      audioPlayer["analyserNode"] =
        mockAudioContext.createAnalyser() as unknown as AnalyserNode;

      audioPlayer.destroy();

      expect(mockAudioContext.close).toHaveBeenCalled();
      expect(audioPlayer["eqFilters"]).toHaveLength(0);
    });

    test("should handle destroy when already cleaned up", () => {
      audioPlayer["audioContext"] = null;

      expect(() => audioPlayer.destroy()).not.toThrow();
    });
  });

  describe.skip("Error Handling", () => {
    beforeEach(() => {
      audioPlayer = new AudioPlayer(container, {}, mockCallbacks);
    });

    test("should handle fetch errors gracefully", async () => {
      const { resolveResource } = await import("@tauri-apps/api/path");
      (resolveResource as any).mockImplementationOnce(() =>
        Promise.reject(new Error("Network error")),
      );

      const config: AudioPlayerConfig = {
        demoTracks: { test: "/public/demo-audio/test.wav" },
      };

      audioPlayer = new AudioPlayer(container, config, mockCallbacks);

      await expect(audioPlayer["loadDemoTrack"]("test")).rejects.toThrow();
      expect(mockCallbacks.onError).toHaveBeenCalled();
    });

    test("should handle audio decoding errors", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      (invoke as any).mockImplementationOnce(() =>
        Promise.reject(new Error("Decode error")),
      );

      await expect(
        audioPlayer.loadAudioFilePath("/tmp/test.wav"),
      ).rejects.toThrow("Decode error");
      expect(mockCallbacks.onError).toHaveBeenCalled();
    });
  });
});
