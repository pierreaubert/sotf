import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import {
  SpectrumAnalyzerComponent,
  type SpectrumInfo,
} from "../modules/audio-player/spectrum-analyzer";
import { invoke } from "@tauri-apps/api/core";

// Mock Tauri invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("SpectrumAnalyzerComponent", () => {
  let canvas: HTMLCanvasElement;
  let component: SpectrumAnalyzerComponent;

  beforeEach(() => {
    // Clear all mocks before each test
    vi.clearAllMocks();

    // Create a canvas element
    canvas = document.createElement("canvas");
    canvas.width = 800;
    canvas.height = 400;
    document.body.appendChild(canvas);

    // Create component
    component = new SpectrumAnalyzerComponent({
      canvas,
      pollInterval: 100,
      minFreq: 20,
      maxFreq: 20000,
      dbRange: 60,
      colorScheme: "dark",
      showLabels: true,
      showGrid: true,
    });
  });

  afterEach(() => {
    component.destroy();
    document.body.removeChild(canvas);
  });

  it("should create component with default config", () => {
    const defaultComponent = new SpectrumAnalyzerComponent({
      canvas,
    });

    expect(defaultComponent).toBeDefined();
    expect(defaultComponent.isActive()).toBe(false);
    expect(defaultComponent.getSpectrum()).toBeNull();
  });

  it("should initialize canvas context", () => {
    expect(component).toBeDefined();
    expect(canvas.width).toBeGreaterThan(0);
    expect(canvas.height).toBeGreaterThan(0);
  });

  it("should start monitoring", async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValue(undefined);

    await component.start();

    expect(mockInvoke).toHaveBeenCalledWith(
      "stream_enable_spectrum_monitoring",
    );
    expect(component.isActive()).toBe(true);
  });

  it("should stop monitoring", async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValue(undefined);

    await component.start();
    expect(component.isActive()).toBe(true);

    await component.stop();

    expect(mockInvoke).toHaveBeenCalledWith(
      "stream_disable_spectrum_monitoring",
    );
    expect(component.isActive()).toBe(false);
  });

  it("should handle start errors gracefully", async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockRejectedValue(new Error("Test error"));

    await expect(component.start()).rejects.toThrow("Test error");
    expect(component.isActive()).toBe(false);
  });

  it("should not start if already monitoring", async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValue(undefined);

    await component.start();
    const callCount = mockInvoke.mock.calls.length;

    await component.start();

    expect(mockInvoke.mock.calls.length).toBe(callCount);
  });

  it("should not stop if not monitoring", async () => {
    const mockInvoke = vi.mocked(invoke);

    await component.stop();

    expect(mockInvoke).not.toHaveBeenCalledWith(
      "stream_disable_spectrum_monitoring",
    );
  });

  it("should handle spectrum data", async () => {
    const mockSpectrum: SpectrumInfo = {
      frequencies: [20, 100, 1000, 10000, 20000],
      magnitudes: [-40, -30, -20, -10, -5],
      peak_magnitude: -5,
    };

    // Simulate receiving spectrum data
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValue(mockSpectrum);

    await component.start();

    // Wait for polling to get data
    await new Promise((resolve) => setTimeout(resolve, 150));

    const spectrum = component.getSpectrum();
    expect(spectrum).toBeDefined();
    expect(spectrum?.frequencies.length).toBe(5);
    expect(spectrum?.peak_magnitude).toBe(-5);
  });

  it("should handle null spectrum data", async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValue(null);

    await component.start();

    // Wait for polling
    await new Promise((resolve) => setTimeout(resolve, 150));

    const spectrum = component.getSpectrum();
    expect(spectrum).toBeNull();
  });

  it("should resize canvas", () => {
    const originalWidth = canvas.width;
    canvas.style.width = "600px";

    component.resize();

    // Canvas should be resized (actual size depends on DPR)
    expect(canvas.width).toBeGreaterThan(0);
  });

  it("should cleanup on destroy", async () => {
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValue(undefined);

    await component.start();
    expect(component.isActive()).toBe(true);

    component.destroy();

    expect(component.isActive()).toBe(false);
  });

  it("should handle different color schemes", () => {
    const darkComponent = new SpectrumAnalyzerComponent({
      canvas,
      colorScheme: "dark",
    });

    const lightComponent = new SpectrumAnalyzerComponent({
      canvas,
      colorScheme: "light",
    });

    expect(darkComponent).toBeDefined();
    expect(lightComponent).toBeDefined();
  });

  it("should render without data", () => {
    // Should not throw
    expect(() => {
      component["render"]();
    }).not.toThrow();
  });

  it("should handle infinite magnitude values", async () => {
    const mockSpectrum: SpectrumInfo = {
      frequencies: [20, 100, 1000],
      magnitudes: [-Infinity, -30, -Infinity],
      peak_magnitude: -30,
    };

    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValue(mockSpectrum);

    await component.start();
    await new Promise((resolve) => setTimeout(resolve, 150));

    // Should not throw
    expect(() => {
      component["render"]();
    }).not.toThrow();
  });

  describe("Frequency and Magnitude Range", () => {
    it("should correctly map 20 Hz to left edge", () => {
      const width = 800;
      const x = component["freqToX"](20, width);

      // Left padding is 30px
      expect(x).toBeCloseTo(30, 1);
    });

    it("should correctly map 20 kHz to right edge", () => {
      const width = 800;
      const x = component["freqToX"](20000, width);

      // Right edge is width - 5px padding = 795px
      expect(x).toBeCloseTo(width - 5, 1);
    });

    it("should correctly map frequencies in logarithmic scale", () => {
      const width = 800;

      // 1 kHz should be roughly in the middle (log scale)
      const x1k = component["freqToX"](1000, width);
      const xMid = (30 + (width - 5)) / 2;

      // Should be close to middle (within 15% tolerance due to logarithmic spacing)
      // 1kHz is not exactly at the geometric center due to 20Hz-20kHz range
      expect(Math.abs(x1k - xMid) / xMid).toBeLessThan(0.15);
    });

    it("should correctly map 0 dB to full height", () => {
      const height = 120; // Compact canvas height
      const barHeight = component["dbToHeight"](0, height);

      // 0 dB should use full available height (height - 20px for labels)
      expect(barHeight).toBeCloseTo(height - 20, 1);
    });

    it("should correctly map -60 dB to zero height", () => {
      const height = 120;
      const barHeight = component["dbToHeight"](-60, height);

      // -60 dB should have zero height
      expect(barHeight).toBeCloseTo(0, 1);
    });

    it("should correctly map -30 dB to half height", () => {
      const height = 120;
      const barHeight = component["dbToHeight"](-30, height);

      // -30 dB should be half of available height
      const expectedHeight = (height - 20) / 2;
      expect(barHeight).toBeCloseTo(expectedHeight, 1);
    });

    it("should clamp values above 0 dB", () => {
      const height = 120;
      const barHeight = component["dbToHeight"](10, height);

      // Should clamp to 0 dB (full height)
      expect(barHeight).toBeCloseTo(height - 20, 1);
    });

    it("should clamp values below -60 dB", () => {
      const height = 120;
      const barHeight = component["dbToHeight"](-100, height);

      // Should clamp to -60 dB (zero height)
      expect(barHeight).toBeCloseTo(0, 1);
    });
  });

  describe("Bar Scaling with Canvas Size", () => {
    it("should scale bars proportionally with canvas width", () => {
      const widthSmall = 400;
      const widthLarge = 1200;

      const x1Small = component["freqToX"](1000, widthSmall);
      const x2Small = component["freqToX"](2000, widthSmall);
      const widthRatioSmall = x2Small / x1Small;

      const x1Large = component["freqToX"](1000, widthLarge);
      const x2Large = component["freqToX"](2000, widthLarge);
      const widthRatioLarge = x2Large / x1Large;

      // Ratio should be close regardless of canvas width (within 2%)
      // Small differences due to fixed padding at different canvas sizes
      expect(
        Math.abs(widthRatioSmall - widthRatioLarge) / widthRatioLarge,
      ).toBeLessThan(0.02);
    });

    it("should scale bars proportionally with canvas height", () => {
      const heightSmall = 60;
      const heightLarge = 240;

      const barSmall = component["dbToHeight"](-30, heightSmall);
      const barLarge = component["dbToHeight"](-30, heightLarge);

      // Bar height should scale with canvas height
      // (heightSmall - 20) / (heightLarge - 20) should equal barSmall / barLarge
      const expectedRatio = (heightSmall - 20) / (heightLarge - 20);
      const actualRatio = barSmall / barLarge;

      expect(actualRatio).toBeCloseTo(expectedRatio, 2);
    });

    it("should handle very small canvas sizes", () => {
      const height = 40; // Very small height

      // Should still work and not return negative values
      const barHeight = component["dbToHeight"](-20, height);
      expect(barHeight).toBeGreaterThanOrEqual(0);
      expect(barHeight).toBeLessThanOrEqual(height - 20);
    });

    it("should handle very large canvas sizes", () => {
      const width = 3840; // 4K width
      const height = 2160; // 4K height

      // Should not throw and return valid values
      const x = component["freqToX"](1000, width);
      const barHeight = component["dbToHeight"](-30, height);

      expect(x).toBeGreaterThan(0);
      expect(x).toBeLessThan(width);
      expect(barHeight).toBeGreaterThan(0);
      expect(barHeight).toBeLessThan(height);
    });
  });

  describe("Full Spectrum Data Rendering", () => {
    it("should correctly render spectrum covering full frequency range", async () => {
      // Create mock spectrum data covering 20 Hz to 20 kHz
      const frequencies: number[] = [];
      const magnitudes: number[] = [];

      // Generate logarithmically spaced frequencies
      for (let i = 0; i < 100; i++) {
        const logFreq =
          Math.log10(20) + (Math.log10(20000) - Math.log10(20)) * (i / 99);
        frequencies.push(Math.pow(10, logFreq));
        // Simulate varying magnitudes
        magnitudes.push(-60 + (60 * i) / 99);
      }

      const mockSpectrum: SpectrumInfo = {
        frequencies,
        magnitudes,
        peak_magnitude: 0,
      };

      const mockInvoke = vi.mocked(invoke);
      mockInvoke.mockResolvedValue(mockSpectrum);

      await component.start();
      await new Promise((resolve) => setTimeout(resolve, 150));

      const spectrum = component.getSpectrum();
      expect(spectrum).toBeDefined();
      expect(spectrum?.frequencies[0]).toBeCloseTo(20, 0);
      expect(
        spectrum?.frequencies[spectrum.frequencies.length - 1],
      ).toBeCloseTo(20000, 0);
      expect(spectrum?.magnitudes[0]).toBeCloseTo(-60, 0);
      expect(spectrum?.magnitudes[spectrum.magnitudes.length - 1]).toBeCloseTo(
        0,
        0,
      );

      // Should render without throwing
      expect(() => {
        component["render"]();
      }).not.toThrow();
    });
  });
});
