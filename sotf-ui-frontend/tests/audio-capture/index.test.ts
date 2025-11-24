/* removed old WebAudio/WebRTC AudioProcessor export tests */
import { describe, test, expect } from "vitest";

// Import CaptureResult from capture-controller
import { type CaptureResult } from "@audio-capture/capture-controller";

describe.skip("Audio Module Exports", () => {
  test("should export TypeScript interfaces", () => {
    const captureResult: CaptureResult = {
      success: true,
      frequencies: [100, 200, 300],
      magnitudes: [10, 20, 30],
      phases: [0, 0, 0],
    };

    expect(captureResult).toBeDefined();
    expect(captureResult.success).toBe(true);
    expect(captureResult.frequencies).toHaveLength(3);
  });
});
