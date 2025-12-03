/* AudioProcessor has been deprecated - these tests are kept for reference only */
import { describe, test, expect } from "vitest";
import { type CaptureResult } from "@audio-capture/capture-controller";

describe.skip("AudioProcessor - DEPRECATED", () => {
  test("CaptureResult interface should be defined", () => {
    const result: CaptureResult = {
      success: true,
      frequencies: [100, 200, 300],
      magnitudes: [-20, -15, -10],
      phases: [0, 0.5, 1.0],
      error: undefined,
    };

    expect(result.success).toBe(true);
    expect(result.frequencies).toHaveLength(3);
    expect(result.magnitudes).toHaveLength(3);
  });
});

/* Legacy mocks kept for reference */
const _mockAudioContext = {
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
    getFloatFrequencyData: vi.fn((array) => {
      // Fill with mock frequency data
      for (let i = 0; i < array.length; i++) {
        array[i] = Math.random() * -60; // dB values
      }
    }),
  })),
  createBuffer: vi.fn(
    (channels: number, length: number, sampleRate: number) => ({
      duration: length / sampleRate,
      length: length,
      numberOfChannels: channels,
      sampleRate: sampleRate,
      getChannelData: vi.fn(() => new Float32Array(length)),
    }),
  ),
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
  createOscillator: vi.fn(() => ({
    type: "sine",
    frequency: {
      value: 1000,
      setValueAtTime: vi.fn(),
      exponentialRampToValueAtTime: vi.fn(),
    },
    connect: vi.fn(),
    disconnect: vi.fn(),
    start: vi.fn(),
    stop: vi.fn(),
  })),
  createChannelMerger: vi.fn(() => ({
    connect: vi.fn(),
    disconnect: vi.fn(),
  })),
  createChannelSplitter: vi.fn(() => ({
    connect: vi.fn(),
    disconnect: vi.fn(),
  })),
  createScriptProcessor: vi.fn(() => ({
    onaudioprocess: null,
    connect: vi.fn(),
    disconnect: vi.fn(),
  })),
  createMediaStreamSource: vi.fn(() => ({
    connect: vi.fn(),
    disconnect: vi.fn(),
  })),
  decodeAudioData: vi.fn(),
  resume: vi.fn(),
  close: vi.fn(),
  destination: {},
  currentTime: 0,
  state: "running",
  sampleRate: 44100,
};
