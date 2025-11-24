// Audio module test suite
import { describe, test, expect } from "vitest";

// Import all audio module exports to ensure they're properly exported
import {
  AudioPlayer,
  type AudioPlayerConfig,
  type AudioPlayerCallbacks,
  type FilterParam,
} from "@audio-player/audio-player";

describe("Audio Module Exports", () => {
  test("should export AudioPlayer class", () => {
    expect(AudioPlayer).toBeDefined();
    expect(typeof AudioPlayer).toBe("function");
  });

  test("should export TypeScript interfaces", () => {
    // These are compile-time checks, but we can verify the types exist
    const config: AudioPlayerConfig = {
      enableEQ: true,
      enableSpectrum: true,
    };

    const callbacks: AudioPlayerCallbacks = {
      onPlay: () => {},
      onStop: () => {},
    };

    const filterParam: FilterParam = {
      frequency: 1000,
      q: 1,
      gain: 0,
      enabled: true,
    };
  });

  test("should maintain backward compatibility with module exports", () => {
    // Verify that the audio module can be imported from the main index
    expect(() => {
      // This would be caught at compile time if exports are broken
      const player = new AudioPlayer(document.createElement("div"));
      // Note: AudioProcessor is tested separately

      expect(player).toBeInstanceOf(AudioPlayer);

      // Cleanup
      player.destroy();
    }).not.toThrow();
  });
});
