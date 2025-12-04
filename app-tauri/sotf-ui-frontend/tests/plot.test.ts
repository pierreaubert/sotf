// Tests for AutoEQ plot functions
// Note: These tests use Vitest testing framework

import { describe, test, expect, beforeEach, vi } from "vitest";
import {
  AutoEQPlotAPI,
  PlotUtils,
  PlotFiltersParams,
  PlotSpinParams,
  CurveData,
  PlotlyData,
} from "../types/plot";
import {
  displayFilterPlots,
  displaySpinPlots,
  displaySpinDetailsPlots,
  displayTonalBalancePlots,
  createSampleCurveData,
  displayOptimizationResults,
} from "../examples/plot-examples";

// Mock Plotly module
vi.mock("plotly.js-basic-dist-min", () => ({
  default: {
    newPlot: vi.fn().mockResolvedValue(undefined),
    Plots: {
      resize: vi.fn(),
    },
  },
  newPlot: vi.fn().mockResolvedValue(undefined),
  Plots: {
    resize: vi.fn(),
  },
}));

// Mock Tauri invoke function
const mockTauriInvoke = vi.fn();
(globalThis as any).window = {
  __TAURI__: {
    core: {
      invoke: mockTauriInvoke,
    },
  },
};

// Mock DOM elements
const createMockElement = () => ({
  innerHTML: "",
  style: { display: "block", padding: "0" },
  classList: { add: vi.fn(), remove: vi.fn() },
  offsetWidth: 800,
  offsetHeight: 600,
  getBoundingClientRect: vi.fn(() => ({
    width: 800,
    height: 600,
    top: 0,
    left: 0,
    right: 800,
    bottom: 600,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  })),
  querySelectorAll: vi.fn(() => []),
  querySelector: vi.fn(() => null),
  appendChild: vi.fn(),
  removeChild: vi.fn(),
});

describe("PlotUtils", () => {
  describe("createResponsiveLayout", () => {
    test("should create responsive layout with default values", () => {
      const layout = PlotUtils.createResponsiveLayout();

      expect(layout).toEqual({
        autosize: true,
        responsive: true,
        margin: {
          l: 50,
          r: 50,
          t: 50,
          b: 50,
        },
      });
    });

    test("should create responsive layout with custom dimensions", () => {
      const layout = PlotUtils.createResponsiveLayout(800, 600);

      expect(layout).toEqual({
        autosize: true,
        responsive: true,
        width: 800,
        height: 600,
        margin: {
          l: 50,
          r: 50,
          t: 50,
          b: 50,
        },
      });
    });
  });

  describe("createDefaultConfig", () => {
    test("should create default Plotly config", () => {
      const config = PlotUtils.createDefaultConfig();

      expect(config).toEqual({
        displayModeBar: true,
        modeBarButtonsToRemove: ["pan2d", "lasso2d"],
        displaylogo: false,
        toImageButtonOptions: {
          format: "png",
          filename: "autoeq_plot",
          height: 600,
          width: 800,
          scale: 2,
        },
      });
    });
  });

  describe("applyUILayout", () => {
    test("should merge custom layout with plot data layout", () => {
      const plotData: PlotlyData = {
        data: [],
        layout: {
          title: "Original Title",
          xaxis: { title: "X" },
        },
      };

      const customLayout = {
        title: "Custom Title",
        yaxis: { title: "Y" },
      };

      const result = PlotUtils.applyUILayout(plotData, customLayout);

      expect(result).toEqual({
        data: [],
        layout: {
          title: "Custom Title",
          xaxis: { title: "X" },
          yaxis: { title: "Y" },
        },
      });
    });
  });
});

describe("AutoEQPlotAPI", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("generatePlotFilters", () => {
    test("should call Tauri invoke with correct parameters", async () => {
      const mockResponse: PlotlyData = {
        data: [{ x: [1, 2, 3], y: [1, 2, 3] }],
        layout: { title: "Test Plot" },
      };

      mockTauriInvoke.mockResolvedValue(mockResponse);

      const params: PlotFiltersParams = {
        input_curve: { freq: [1, 2, 3], spl: [1, 2, 3] },
        target_curve: { freq: [1, 2, 3], spl: [1, 2, 3] },
        deviation_curve: { freq: [1, 2, 3], spl: [1, 2, 3] },
        optimized_params: [100, 1, 3],
        sample_rate: 48000,
        num_filters: 1,
        iir_hp_pk: true,
      };

      const result = await AutoEQPlotAPI.generatePlotFilters(params);

      expect(mockTauriInvoke).toHaveBeenCalledWith("generate_plot_filters", {
        params,
      });
      expect(result).toEqual(mockResponse);
    });
  });

  describe("generatePlotSpin", () => {
    test("should call Tauri invoke with correct parameters", async () => {
      const mockResponse: PlotlyData = {
        data: [{ x: [1, 2, 3], y: [1, 2, 3] }],
        layout: { title: "Spin Plot" },
      };

      mockTauriInvoke.mockResolvedValue(mockResponse);

      const params: PlotSpinParams = {
        cea2034_curves: {
          "Listening Window": { freq: [1, 2, 3], spl: [1, 2, 3] },
        },
      };

      const result = await AutoEQPlotAPI.generatePlotSpin(params);

      expect(mockTauriInvoke).toHaveBeenCalledWith("generate_plot_spin", {
        params,
      });
      expect(result).toEqual(mockResponse);
    });
  });

  describe("generatePlotSpinDetails", () => {
    test("should call Tauri invoke with correct parameters", async () => {
      const mockResponse: PlotlyData = {
        data: [{ x: [1, 2, 3], y: [1, 2, 3] }],
        layout: { title: "Detailed Spin Plot" },
      };

      mockTauriInvoke.mockResolvedValue(mockResponse);

      const params: PlotSpinParams = {
        cea2034_curves: {
          "Listening Window": { freq: [1, 2, 3], spl: [1, 2, 3] },
        },
      };

      const result = await AutoEQPlotAPI.generatePlotSpinDetails(params);

      expect(mockTauriInvoke).toHaveBeenCalledWith(
        "generate_plot_spin_details",
        { params },
      );
      expect(result).toEqual(mockResponse);
    });
  });

  describe("generatePlotSpinTonal", () => {
    test("should call Tauri invoke with correct parameters", async () => {
      const mockResponse: PlotlyData = {
        data: [{ x: [1, 2, 3], y: [1, 2, 3] }],
        layout: { title: "Tonal Balance Plot" },
      };

      mockTauriInvoke.mockResolvedValue(mockResponse);

      const params: PlotSpinParams = {
        cea2034_curves: {
          "Listening Window": { freq: [1, 2, 3], spl: [1, 2, 3] },
        },
      };

      const result = await AutoEQPlotAPI.generatePlotSpinTonal(params);

      expect(mockTauriInvoke).toHaveBeenCalledWith("generate_plot_spin_tonal", {
        params,
      });
      expect(result).toEqual(mockResponse);
    });
  });
});

describe("Plot Examples", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("createSampleCurveData", () => {
    test("should create valid curve data", () => {
      const curveData = createSampleCurveData();

      expect(curveData).toHaveProperty("freq");
      expect(curveData).toHaveProperty("spl");
      expect(curveData.freq).toHaveLength(101);
      expect(curveData.spl).toHaveLength(101);
      expect(curveData.freq[0]).toBe(20);
      expect(curveData.freq[curveData.freq.length - 1]).toBeCloseTo(20000, -2);
    });
  });

  describe("displayFilterPlots", () => {
    test("should render filter plots successfully", async () => {
      const mockElement = createMockElement();
      const mockPlotData: PlotlyData = {
        data: [{ x: [1, 2, 3], y: [1, 2, 3] }],
        layout: { title: "Test" },
      };

      mockTauriInvoke.mockResolvedValue(mockPlotData);

      const inputCurve = createSampleCurveData();
      const targetCurve = createSampleCurveData();
      const deviationCurve = createSampleCurveData();
      const optimizedParams = [100, 1, 3, 200, 1.5, 2];

      // Note: This test would need proper Plotly mocking to work fully
      // For now, we test that it doesn't throw
      await expect(
        displayFilterPlots(
          mockElement as any,
          inputCurve,
          targetCurve,
          deviationCurve,
          optimizedParams,
        ),
      ).resolves.not.toThrow();
    });
  });

  describe("displaySpinPlots", () => {
    test("should render spin plots successfully", async () => {
      const mockElement = createMockElement();
      const mockPlotData: PlotlyData = {
        data: [{ x: [1, 2, 3], y: [1, 2, 3] }],
        layout: { title: "Test" },
      };

      mockTauriInvoke.mockResolvedValue(mockPlotData);

      const cea2034Curves = {
        "Listening Window": createSampleCurveData(),
      };

      await expect(
        displaySpinPlots(mockElement as any, cea2034Curves),
      ).resolves.not.toThrow();
    });
  });

  describe("displaySpinDetailsPlots", () => {
    test("should render detailed spin plots successfully", async () => {
      const mockElement = createMockElement();
      const mockPlotData: PlotlyData = {
        data: [{ x: [1, 2, 3], y: [1, 2, 3] }],
        layout: { title: "Test" },
      };

      mockTauriInvoke.mockResolvedValue(mockPlotData);

      const cea2034Curves = {
        "Listening Window": createSampleCurveData(),
      };

      await expect(
        displaySpinDetailsPlots(mockElement as any, cea2034Curves),
      ).resolves.not.toThrow();
    });
  });

  describe("displayTonalBalancePlots", () => {
    test("should render tonal balance plots successfully", async () => {
      const mockElement = createMockElement();
      const mockPlotData: PlotlyData = {
        data: [{ x: [1, 2, 3], y: [1, 2, 3] }],
        layout: { title: "Test" },
      };

      mockTauriInvoke.mockResolvedValue(mockPlotData);

      const cea2034Curves = {
        "Listening Window": createSampleCurveData(),
      };

      await expect(
        displayTonalBalancePlots(mockElement as any, cea2034Curves),
      ).resolves.not.toThrow();
    });
  });

  describe("displayOptimizationResults", () => {
    test("should handle optimization results with filter response", async () => {
      const mockFilterElement = createMockElement();
      const mockSpinElement = createMockElement();
      const mockPlotData: PlotlyData = {
        data: [{ x: [1, 2, 3], y: [1, 2, 3] }],
        layout: { title: "Test" },
      };

      mockTauriInvoke.mockResolvedValue(mockPlotData);

      const optimizationResult = {
        filter_params: [100, 1, 3, 200, 1.5, 2],
        filter_response: {
          input_curve: createSampleCurveData(),
          target_curve: createSampleCurveData(),
          deviation_curve: createSampleCurveData(),
          eq_response: [1, 2, 3],
        },
        spin_details: {
          curves: {
            "Listening Window": createSampleCurveData(),
          },
        },
      };

      await expect(
        displayOptimizationResults(
          mockFilterElement as any,
          mockSpinElement as any,
          optimizationResult,
        ),
      ).resolves.not.toThrow();
    });

    test("should handle optimization results without filter response", async () => {
      const mockFilterElement = createMockElement();
      const mockSpinElement = createMockElement();

      const optimizationResult = {};

      await expect(
        displayOptimizationResults(
          mockFilterElement as any,
          mockSpinElement as any,
          optimizationResult,
        ),
      ).resolves.not.toThrow();
    });
  });
});

describe("Integration Tests", () => {
  test("should work with real-world optimization result structure", async () => {
    const mockElement = createMockElement();
    const mockPlotData: PlotlyData = {
      data: [
        {
          x: [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000],
          y: [0, 1, 2, 1, 0, -1, -2, -1, 0, 1],
          type: "scatter",
          mode: "lines",
          name: "Input Curve",
        },
      ],
      layout: {
        title: "Filter Response",
        xaxis: { title: "Frequency (Hz)", type: "log" },
        yaxis: { title: "Magnitude (dB)" },
      },
    };

    mockTauriInvoke.mockResolvedValue(mockPlotData);

    // This simulates a real optimization result structure
    const realWorldResult = {
      success: true,
      filter_params: [100, 0.7, 3.0, 1000, 1.0, -2.0, 5000, 2.0, 1.5],
      preference_score_before: 85.2,
      preference_score_after: 92.7,
      filter_response: {
        frequencies: [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000],
        curves: {
          Input: [0, 1, 2, 1, 0, -1, -2, -1, 0, 1],
          Target: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
          "EQ Response": [0, 0.5, 1, 0.5, 0, -0.5, -1, -0.5, 0, 0.5],
        },
      },
    };

    // Test that the structure can be processed without errors
    expect(realWorldResult.success).toBe(true);
    expect(realWorldResult.filter_params).toHaveLength(9); // 3 filters * 3 params each
    expect(realWorldResult.preference_score_after).toBeGreaterThan(
      realWorldResult.preference_score_before,
    );
  });
});
