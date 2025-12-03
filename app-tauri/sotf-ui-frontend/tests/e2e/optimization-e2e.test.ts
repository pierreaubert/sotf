// End-to-End Tests for Optimization with Real Backend
// Tests all 5 input source scenarios to prevent regression

import { describe, test, expect, beforeAll, afterAll, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  OptimizationParams,
  OptimizationResult,
} from "../../types/optimization";
import { AutoEQPlotAPI } from "../../types/plot";
import * as fs from "fs";
import * as path from "path";

// Extended timeout for E2E tests (30 seconds)
const E2E_TIMEOUT = 30000;

// Check if running in Tauri environment or forced by env var
const isTauriAvailable = () => {
  // Allow forcing E2E tests via environment variable
  if (process.env.FORCE_E2E === "true") {
    return true;
  }
  return (
    typeof window !== "undefined" && window.__TAURI_INTERNALS__ !== undefined
  );
};

// Skip E2E tests if Tauri is not available (unless forced)
const describeE2E = isTauriAvailable() ? describe : describe.skip;

// Path to fixtures
const FIXTURES_DIR = path.join(__dirname, "fixtures");

/**
 * Helper to read fixture files
 */
function readFixture(relativePath: string): string {
  return fs.readFileSync(path.join(FIXTURES_DIR, relativePath), "utf-8");
}

/**
 * Helper to validate optimization result structure
 */
function validateOptimizationResult(
  result: OptimizationResult,
  hasSpinData: boolean = false,
) {
  // Basic success validation
  expect(result.success).toBe(true);
  expect(result.error_message).toBeUndefined();

  // Filter params validation
  expect(result.filter_params).toBeDefined();
  expect(result.filter_params!.length).toBeGreaterThan(0);
  expect(result.filter_params!.length % 3).toBe(0); // Should be multiple of 3 (freq, q, gain)

  // Required curves (ALWAYS present)
  expect(result.filter_response).toBeDefined();
  expect(result.input_curve).toBeDefined();
  expect(result.deviation_curve).toBeDefined();
  expect(result.filter_plots).toBeDefined();

  // Validate curve structures
  expect(result.filter_response!.frequencies).toBeDefined();
  expect(result.filter_response!.curves["Target"]).toBeDefined();
  expect(result.filter_response!.curves["EQ Response"]).toBeDefined();

  expect(result.input_curve!.curves["Input"]).toBeDefined();
  expect(result.deviation_curve!.curves["Deviation"]).toBeDefined();

  // Spin data validation (conditional)
  if (hasSpinData) {
    expect(result.spin_details).toBeDefined();
    expect(result.spin_details!.curves).toBeDefined();
    expect(result.preference_score_before).toBeDefined();
    expect(result.preference_score_after).toBeDefined();
  } else {
    expect(result.spin_details).toBeUndefined();
  }

  // Validate array lengths match
  const freqLength = result.filter_response!.frequencies.length;
  expect(result.filter_response!.curves["Target"].length).toBe(freqLength);
  expect(result.filter_response!.curves["EQ Response"].length).toBe(freqLength);
  expect(result.input_curve!.curves["Input"].length).toBe(freqLength);
  expect(result.deviation_curve!.curves["Deviation"].length).toBe(freqLength);
}

/**
 * Helper to validate plot generation
 */
async function validatePlotGeneration(
  result: OptimizationResult,
  hasSpinData: boolean = false,
) {
  // Always validate filter plot can be generated
  const filterPlotParams = {
    input_curve: {
      freq: result.filter_response!.frequencies,
      spl: result.input_curve!.curves["Input"],
    },
    target_curve: {
      freq: result.filter_response!.frequencies,
      spl: result.filter_response!.curves["Target"],
    },
    deviation_curve: {
      freq: result.filter_response!.frequencies,
      spl: result.deviation_curve!.curves["Deviation"],
    },
    optimized_params: result.filter_params!,
    sample_rate: 48000,
    num_filters: result.filter_params!.length / 3,
    iir_hp_pk: false,
  };

  const filterPlot = await AutoEQPlotAPI.generatePlotFilters(filterPlotParams);
  expect(filterPlot).toBeDefined();
  expect(filterPlot.data).toBeDefined();
  expect(filterPlot.layout).toBeDefined();

  // Validate spinorama plots if applicable
  if (hasSpinData && result.spin_details) {
    const spinParams = {
      cea2034_curves: Object.entries(result.spin_details.curves).reduce(
        (acc, [key, values]) => {
          acc[key] = {
            freq: result.spin_details!.frequencies,
            spl: values,
          };
          return acc;
        },
        {} as any,
      ),
      eq_response: result.filter_response!.curves["EQ Response"],
    };

    const spinPlot = await AutoEQPlotAPI.generatePlotSpin(spinParams);
    expect(spinPlot).toBeDefined();
    expect(spinPlot.data).toBeDefined();

    const detailsPlot = await AutoEQPlotAPI.generatePlotSpinDetails(spinParams);
    expect(detailsPlot).toBeDefined();

    const tonalPlot = await AutoEQPlotAPI.generatePlotSpinTonal(spinParams);
    expect(tonalPlot).toBeDefined();
  }
}

describeE2E("E2E: Optimization with All Input Sources", () => {
  describe("Case 1: Speaker with CEA2034/Spinorama", () => {
    test(
      "should optimize speaker from API with full spinorama data",
      async () => {
        const params: OptimizationParams = {
          num_filters: 5,
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
          maxeval: 500, // Reduced for faster tests
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

        const result = await invoke<OptimizationResult>("run_optimization", {
          params,
        });

        validateOptimizationResult(result, true);
        await validatePlotGeneration(result, true);

        console.log(
          `✅ Speaker+CEA2034: Generated ${result.filter_params!.length / 3} filters`,
        );
        console.log(
          `   Preference score: ${result.preference_score_before?.toFixed(2)} → ${result.preference_score_after?.toFixed(2)}`,
        );
      },
      E2E_TIMEOUT,
    );
  });

  describe("Case 2: Speaker without CEA2034", () => {
    test.skip(
      "should optimize speaker with on-axis only data",
      async () => {
        // This would require a speaker without full CEA2034 data
        // Skipped as most speakers in Spinorama have full data
      },
      E2E_TIMEOUT,
    );
  });

  describe("Case 3: Headphone with Target Curve", () => {
    test(
      "should optimize headphone with Harman target",
      async () => {
        const fixturePath = path.join(
          FIXTURES_DIR,
          "headphone/sample_headphone.csv",
        );

        const params: OptimizationParams = {
          num_filters: 5,
          curve_path: fixturePath,
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
          maxeval: 500,
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

        const result = await invoke<OptimizationResult>("run_optimization", {
          params,
        });

        validateOptimizationResult(result, false);
        await validatePlotGeneration(result, false);

        console.log(
          `✅ Headphone: Generated ${result.filter_params!.length / 3} filters`,
        );
      },
      E2E_TIMEOUT,
    );
  });

  describe("Case 4: File Input with Optional Target", () => {
    test(
      "should optimize from file with custom target",
      async () => {
        const inputPath = path.join(FIXTURES_DIR, "file/input.csv");
        const targetPath = path.join(FIXTURES_DIR, "file/target.csv");

        const params: OptimizationParams = {
          num_filters: 4,
          curve_path: inputPath,
          target_path: targetPath,
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
          maxeval: 500,
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

        const result = await invoke<OptimizationResult>("run_optimization", {
          params,
        });

        validateOptimizationResult(result, false);
        await validatePlotGeneration(result, false);

        console.log(
          `✅ File+Target: Generated ${result.filter_params!.length / 3} filters`,
        );
      },
      E2E_TIMEOUT,
    );

    test(
      "should optimize from file without target (flat)",
      async () => {
        const inputPath = path.join(FIXTURES_DIR, "file/input.csv");

        const params: OptimizationParams = {
          num_filters: 3,
          curve_path: inputPath,
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
          maxeval: 500,
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

        const result = await invoke<OptimizationResult>("run_optimization", {
          params,
        });

        validateOptimizationResult(result, false);
        await validatePlotGeneration(result, false);

        console.log(
          `✅ File (flat): Generated ${result.filter_params!.length / 3} filters`,
        );
      },
      E2E_TIMEOUT,
    );
  });

  describe("Case 5: Captured Audio Data", () => {
    test(
      "should optimize from captured microphone sweep",
      async () => {
        const fixtureData = JSON.parse(
          readFixture("capture/sweep_response.json"),
        );

        const params: OptimizationParams = {
          num_filters: 3,
          curve_name: "flat",
          captured_frequencies: fixtureData.frequencies,
          captured_magnitudes: fixtureData.magnitudes,
          sample_rate: 48000,
          max_db: 6.0,
          min_db: 0.5,
          max_q: 5.0,
          min_q: 0.5,
          min_freq: 60.0,
          max_freq: 16000.0,
          algo: "nlopt:cobyla",
          population: 300,
          maxeval: 500,
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

        const result = await invoke<OptimizationResult>("run_optimization", {
          params,
        });

        validateOptimizationResult(result, false);
        await validatePlotGeneration(result, false);

        console.log(
          `✅ Captured: Generated ${result.filter_params!.length / 3} filters`,
        );
      },
      E2E_TIMEOUT,
    );
  });

  describe("Performance Benchmarks", () => {
    test(
      "should complete optimization within reasonable time",
      async () => {
        const startTime = Date.now();

        const fixturePath = path.join(FIXTURES_DIR, "file/input.csv");

        const params: OptimizationParams = {
          num_filters: 3,
          curve_path: fixturePath,
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
          maxeval: 500,
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

        const result = await invoke<OptimizationResult>("run_optimization", {
          params,
        });

        const duration = Date.now() - startTime;

        expect(result.success).toBe(true);
        expect(duration).toBeLessThan(E2E_TIMEOUT);

        console.log(`⏱️  Optimization completed in ${duration}ms`);
      },
      E2E_TIMEOUT,
    );
  });
});
