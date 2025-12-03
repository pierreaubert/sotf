// Integration tests for optimization with different data sources
// Tests all 5 input cases to prevent regression

import {
  describe,
  test,
  expect,
  beforeEach,
  vi,
  beforeAll,
  afterAll,
} from "vitest";
import { OptimizationManager } from "../modules/optimization-manager";
import { OptimizationParams, OptimizationResult } from "../types/optimization";

/**
 * Mock Tauri API for testing
 * In real tests, we would use actual backend calls or mock more comprehensively
 */
const mockTauriInvoke = vi.fn();
(globalThis as any).window = {
  __TAURI_INTERNALS__: {
    invoke: mockTauriInvoke,
  },
  __TAURI__: {
    core: {
      invoke: mockTauriInvoke,
    },
    event: {
      listen: vi.fn().mockResolvedValue(() => {}),
    },
  },
};

/**
 * Helper function to create a mock optimization result
 */
function createMockOptimizationResult(
  hasSpinData: boolean = false,
): OptimizationResult {
  const frequencies = Array.from(
    { length: 200 },
    (_, i) => 20 * Math.pow(20000 / 20, i / 199),
  );
  const mockCurve = frequencies.map(() => Math.random() * 2 - 1);

  const result: OptimizationResult = {
    success: true,
    filter_params: [2.5, 1.0, 3.0, 3.0, 0.7, -2.0, 3.5, 1.2, 1.5], // 3 filters
    objective_value: 0.5,
    preference_score_before: hasSpinData ? 5.5 : undefined,
    preference_score_after: hasSpinData ? 7.2 : undefined,
    filter_response: {
      frequencies,
      curves: {
        Target: mockCurve.map((v) => v * 0.8),
        "EQ Response": mockCurve,
      },
      metadata: {},
    },
    filter_plots: {
      frequencies,
      curves: {
        "PK 1 at 316Hz": mockCurve.map((v) => v * 0.3),
        "PK 2 at 1000Hz": mockCurve.map((v) => v * 0.4),
        "PK 3 at 3162Hz": mockCurve.map((v) => v * 0.3),
        Sum: mockCurve,
      },
      metadata: {},
    },
    input_curve: {
      frequencies,
      curves: {
        Input: frequencies.map(() => Math.random() * 2 - 1),
      },
      metadata: {},
    },
    deviation_curve: {
      frequencies,
      curves: {
        Deviation: frequencies.map(() => Math.random() * 4 - 2),
      },
      metadata: {},
    },
  };

  // Add CEA2034/spinorama data if this is a speaker-based optimization
  if (hasSpinData) {
    result.spin_details = {
      frequencies,
      curves: {
        "Listening Window": mockCurve.map((v) => v * 0.9),
        "On Axis": mockCurve,
        "Early Reflections": mockCurve.map((v) => v * 0.85),
        "Sound Power": mockCurve.map((v) => v * 0.8),
      },
      metadata: {},
    };
  }

  return result;
}

describe("Optimization Integration Tests - All Input Sources", () => {
  let optimizationManager: OptimizationManager;

  beforeEach(() => {
    vi.clearAllMocks();
    optimizationManager = new OptimizationManager();
  });

  describe("Case 1: Speaker data WITH CEA2034/Spinorama", () => {
    test("should optimize with speaker API data and return all 4 plots", async () => {
      const mockResult = createMockOptimizationResult(true);
      mockTauriInvoke.mockResolvedValue(mockResult);

      const params: OptimizationParams = {
        num_filters: 3,
        speaker: "KEF LS50 Meta",
        version: "vendor",
        measurement: "asr",
        curve_name: "Listening Window",
        sample_rate: 48000,
        max_db: 6.0,
        min_db: 0.5,
        max_q: 5.0,
        min_q: 0.5,
        min_freq: 60.0,
        max_freq: 16000.0,
        algo: "nlopt:cobyla",
        population: 300,
        maxeval: 2000,
        refine: false,
        local_algo: "cobyla",
        min_spacing_oct: 0.5,
        spacing_weight: 20.0,
        smooth: true,
        smooth_n: 2,
        loss: "speaker-score",
        iir_hp_pk: false,
        tolerance: 1e-3,
        atolerance: 1e-4,
      };

      const result = await optimizationManager.runOptimization(params);

      expect(result.success).toBe(true);
      expect(result.filter_params).toBeDefined();
      expect(result.filter_params?.length).toBe(9); // 3 filters * 3 params

      // Should have all required curve data
      expect(result.filter_response).toBeDefined();
      expect(result.input_curve).toBeDefined();
      expect(result.deviation_curve).toBeDefined();
      expect(result.filter_plots).toBeDefined();

      // Should have spin data for speaker optimization
      expect(result.spin_details).toBeDefined();
      expect(result.spin_details?.curves).toBeDefined();

      // Should have preference scores for CEA2034
      expect(result.preference_score_before).toBeDefined();
      expect(result.preference_score_after).toBeDefined();
      expect(result.preference_score_after).toBeGreaterThan(
        result.preference_score_before!,
      );
    });
  });

  describe("Case 2: Speaker data WITHOUT CEA2034 (measurement missing spinorama)", () => {
    test("should optimize with speaker data but no spinorama plots", async () => {
      const mockResult = createMockOptimizationResult(false);
      mockTauriInvoke.mockResolvedValue(mockResult);

      const params: OptimizationParams = {
        num_filters: 3,
        speaker: "Some Speaker",
        version: "vendor",
        measurement: "on-axis-only", // Measurement without full spinorama
        curve_name: "On Axis",
        sample_rate: 48000,
        max_db: 6.0,
        min_db: 0.5,
        max_q: 5.0,
        min_q: 0.5,
        min_freq: 60.0,
        max_freq: 16000.0,
        algo: "nlopt:cobyla",
        population: 300,
        maxeval: 2000,
        refine: false,
        local_algo: "cobyla",
        min_spacing_oct: 0.5,
        spacing_weight: 20.0,
        smooth: true,
        smooth_n: 2,
        loss: "speaker-flat",
        iir_hp_pk: false,
        tolerance: 1e-3,
        atolerance: 1e-4,
      };

      const result = await optimizationManager.runOptimization(params);

      expect(result.success).toBe(true);
      expect(result.filter_params).toBeDefined();

      // Should have basic curve data
      expect(result.filter_response).toBeDefined();
      expect(result.input_curve).toBeDefined();
      expect(result.deviation_curve).toBeDefined();

      // Should NOT have spin data (no CEA2034)
      expect(result.spin_details).toBeUndefined();

      // Should NOT have preference scores
      expect(result.preference_score_before).toBeUndefined();
      expect(result.preference_score_after).toBeUndefined();
    });
  });

  describe("Case 3: Headphone data with target curve", () => {
    test("should optimize headphone with target curve", async () => {
      const mockResult = createMockOptimizationResult(false);
      mockTauriInvoke.mockResolvedValue(mockResult);

      const params: OptimizationParams = {
        num_filters: 5,
        curve_path: "/path/to/headphone_measurement.csv",
        curve_name: "harman-in-ear-2019",
        sample_rate: 48000,
        max_db: 6.0,
        min_db: 0.5,
        max_q: 5.0,
        min_q: 0.5,
        min_freq: 20.0,
        max_freq: 20000.0,
        algo: "nlopt:cobyla",
        population: 300,
        maxeval: 2000,
        refine: false,
        local_algo: "cobyla",
        min_spacing_oct: 0.5,
        spacing_weight: 20.0,
        smooth: true,
        smooth_n: 2,
        loss: "headphone-flat",
        iir_hp_pk: false,
        tolerance: 1e-3,
        atolerance: 1e-4,
      };

      const result = await optimizationManager.runOptimization(params);

      expect(result.success).toBe(true);
      expect(result.filter_params).toBeDefined();
      expect(result.filter_params?.length).toBe(9); // Mock returns 3 filters * 3 params

      // Should have all required curve data
      expect(result.filter_response).toBeDefined();
      expect(result.input_curve).toBeDefined();
      expect(result.deviation_curve).toBeDefined();

      // Should NOT have spin data (headphone)
      expect(result.spin_details).toBeUndefined();
    });
  });

  describe("Case 4: File input with optional target", () => {
    test("should optimize from file with custom target", async () => {
      const mockResult = createMockOptimizationResult(false);
      mockTauriInvoke.mockResolvedValue(mockResult);

      const params: OptimizationParams = {
        num_filters: 4,
        curve_path: "/path/to/input.csv",
        target_path: "/path/to/target.csv",
        curve_name: "flat",
        sample_rate: 48000,
        max_db: 6.0,
        min_db: 0.5,
        max_q: 5.0,
        min_q: 0.5,
        min_freq: 60.0,
        max_freq: 16000.0,
        algo: "nlopt:cobyla",
        population: 300,
        maxeval: 2000,
        refine: false,
        local_algo: "cobyla",
        min_spacing_oct: 0.5,
        spacing_weight: 20.0,
        smooth: true,
        smooth_n: 2,
        loss: "speaker-flat",
        iir_hp_pk: false,
        tolerance: 1e-3,
        atolerance: 1e-4,
      };

      const result = await optimizationManager.runOptimization(params);

      expect(result.success).toBe(true);
      expect(result.filter_params).toBeDefined();
      expect(result.filter_params?.length).toBe(9); // Mock returns 3 filters * 3 params

      // Should have all required curve data
      expect(result.filter_response).toBeDefined();
      expect(result.input_curve).toBeDefined();
      expect(result.deviation_curve).toBeDefined();

      // Should NOT have spin data (file input)
      expect(result.spin_details).toBeUndefined();
    });

    test("should optimize from file without target (flat target)", async () => {
      const mockResult = createMockOptimizationResult(false);
      mockTauriInvoke.mockResolvedValue(mockResult);

      const params: OptimizationParams = {
        num_filters: 4,
        curve_path: "/path/to/input.csv",
        curve_name: "flat",
        sample_rate: 48000,
        max_db: 6.0,
        min_db: 0.5,
        max_q: 5.0,
        min_q: 0.5,
        min_freq: 60.0,
        max_freq: 16000.0,
        algo: "nlopt:cobyla",
        population: 300,
        maxeval: 2000,
        refine: false,
        local_algo: "cobyla",
        min_spacing_oct: 0.5,
        spacing_weight: 20.0,
        smooth: true,
        smooth_n: 2,
        loss: "speaker-flat",
        iir_hp_pk: false,
        tolerance: 1e-3,
        atolerance: 1e-4,
      };

      const result = await optimizationManager.runOptimization(params);

      expect(result.success).toBe(true);
      expect(result.filter_params).toBeDefined();

      // Should have all required curve data
      expect(result.filter_response).toBeDefined();
      expect(result.input_curve).toBeDefined();
      expect(result.deviation_curve).toBeDefined();
    });
  });

  describe("Case 5: Captured audio data", () => {
    test("should optimize from captured microphone data", async () => {
      const mockResult = createMockOptimizationResult(false);
      mockTauriInvoke.mockResolvedValue(mockResult);

      // Simulate captured data
      const capturedFreqs = Array.from(
        { length: 100 },
        (_, i) => 20 * Math.pow(20000 / 20, i / 99),
      );
      const capturedMags = capturedFreqs.map(() => Math.random() * 20 - 10);

      optimizationManager.setCapturedData(capturedFreqs, capturedMags);

      const params: OptimizationParams = {
        num_filters: 3,
        curve_name: "flat",
        captured_frequencies: capturedFreqs,
        captured_magnitudes: capturedMags,
        sample_rate: 48000,
        max_db: 6.0,
        min_db: 0.5,
        max_q: 5.0,
        min_q: 0.5,
        min_freq: 60.0,
        max_freq: 16000.0,
        algo: "nlopt:cobyla",
        population: 300,
        maxeval: 2000,
        refine: false,
        local_algo: "cobyla",
        min_spacing_oct: 0.5,
        spacing_weight: 20.0,
        smooth: true,
        smooth_n: 2,
        loss: "speaker-flat",
        iir_hp_pk: false,
        tolerance: 1e-3,
        atolerance: 1e-4,
      };

      const result = await optimizationManager.runOptimization(params);

      expect(result.success).toBe(true);
      expect(result.filter_params).toBeDefined();
      expect(result.filter_params?.length).toBe(9); // 3 filters * 3 params

      // Should have all required curve data
      expect(result.filter_response).toBeDefined();
      expect(result.input_curve).toBeDefined();
      expect(result.deviation_curve).toBeDefined();

      // Should NOT have spin data (captured data)
      expect(result.spin_details).toBeUndefined();
    });
  });

  describe("Plot Data Validation", () => {
    test("should always return input_curve and deviation_curve for all cases", async () => {
      const testCases = [
        { name: "speaker with CEA2034", hasSpinData: true },
        { name: "speaker without CEA2034", hasSpinData: false },
        { name: "headphone", hasSpinData: false },
        { name: "file", hasSpinData: false },
        { name: "capture", hasSpinData: false },
      ];

      for (const testCase of testCases) {
        const mockResult = createMockOptimizationResult(testCase.hasSpinData);
        mockTauriInvoke.mockResolvedValue(mockResult);

        const params: OptimizationParams = {
          num_filters: 3,
          curve_path: "/test/path.csv",
          curve_name: "flat",
          sample_rate: 48000,
          max_db: 6.0,
          min_db: 0.5,
          max_q: 5.0,
          min_q: 0.5,
          min_freq: 60.0,
          max_freq: 16000.0,
          algo: "nlopt:cobyla",
          population: 300,
          maxeval: 2000,
          refine: false,
          local_algo: "cobyla",
          min_spacing_oct: 0.5,
          spacing_weight: 20.0,
          smooth: true,
          smooth_n: 2,
          loss: "speaker-flat",
          iir_hp_pk: false,
          tolerance: 1e-3,
          atolerance: 1e-4,
        };

        const result = await optimizationManager.runOptimization(params);

        // These should ALWAYS be present regardless of input type
        expect(
          result.input_curve,
          `${testCase.name}: missing input_curve`,
        ).toBeDefined();
        expect(
          result.deviation_curve,
          `${testCase.name}: missing deviation_curve`,
        ).toBeDefined();
        expect(
          result.filter_response,
          `${testCase.name}: missing filter_response`,
        ).toBeDefined();
        expect(
          result.filter_plots,
          `${testCase.name}: missing filter_plots`,
        ).toBeDefined();

        // Validate curve structure
        expect(result.input_curve?.frequencies).toBeDefined();
        expect(result.input_curve?.curves["Input"]).toBeDefined();
        expect(result.deviation_curve?.frequencies).toBeDefined();
        expect(result.deviation_curve?.curves["Deviation"]).toBeDefined();

        // Check if spin_details matches expected state
        if (testCase.hasSpinData) {
          expect(
            result.spin_details,
            `${testCase.name}: should have spin_details`,
          ).toBeDefined();
        } else {
          expect(
            result.spin_details,
            `${testCase.name}: should not have spin_details`,
          ).toBeUndefined();
        }
      }
    });

    test("should validate filter plot can be generated for all cases", async () => {
      const mockResult = createMockOptimizationResult(false);
      mockTauriInvoke.mockResolvedValue(mockResult);

      const params: OptimizationParams = {
        num_filters: 3,
        curve_path: "/test/path.csv",
        curve_name: "flat",
        sample_rate: 48000,
        max_db: 6.0,
        min_db: 0.5,
        max_q: 5.0,
        min_q: 0.5,
        min_freq: 60.0,
        max_freq: 16000.0,
        algo: "nlopt:cobyla",
        population: 300,
        maxeval: 2000,
        refine: false,
        local_algo: "cobyla",
        min_spacing_oct: 0.5,
        spacing_weight: 20.0,
        smooth: true,
        smooth_n: 2,
        loss: "speaker-flat",
        iir_hp_pk: false,
        tolerance: 1e-3,
        atolerance: 1e-4,
      };

      const result = await optimizationManager.runOptimization(params);

      // Verify we have all data needed for 4-subplot filter plot
      expect(result.filter_response?.curves["Target"]).toBeDefined();
      expect(result.filter_response?.curves["EQ Response"]).toBeDefined();
      expect(result.input_curve?.curves["Input"]).toBeDefined();
      expect(result.deviation_curve?.curves["Deviation"]).toBeDefined();

      // Verify array lengths match
      const freqLength = result.filter_response?.frequencies.length;
      expect(result.filter_response?.curves["Target"].length).toBe(freqLength);
      expect(result.filter_response?.curves["EQ Response"].length).toBe(
        freqLength,
      );
      expect(result.input_curve?.curves["Input"].length).toBe(freqLength);
      expect(result.deviation_curve?.curves["Deviation"].length).toBe(
        freqLength,
      );
    });
  });

  describe("Error Handling", () => {
    test("should handle backend errors gracefully", async () => {
      mockTauriInvoke.mockRejectedValue(
        new Error("Backend optimization failed"),
      );

      const params: OptimizationParams = {
        num_filters: 3,
        curve_path: "/invalid/path.csv",
        curve_name: "flat",
        sample_rate: 48000,
        max_db: 6.0,
        min_db: 0.5,
        max_q: 5.0,
        min_q: 0.5,
        min_freq: 60.0,
        max_freq: 16000.0,
        algo: "nlopt:cobyla",
        population: 300,
        maxeval: 2000,
        refine: false,
        local_algo: "cobyla",
        min_spacing_oct: 0.5,
        spacing_weight: 20.0,
        smooth: true,
        smooth_n: 2,
        loss: "speaker-flat",
        iir_hp_pk: false,
        tolerance: 1e-3,
        atolerance: 1e-4,
      };

      await expect(
        optimizationManager.runOptimization(params),
      ).rejects.toThrow();
    });
  });
});
