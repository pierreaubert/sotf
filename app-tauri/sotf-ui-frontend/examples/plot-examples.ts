// Example usage of AutoEQ plot functions in the frontend

import {
  AutoEQPlotAPI,
  PlotUtils,
  PlotFiltersParams,
  PlotSpinParams,
  CurveData,
} from "../types/plot";
import * as Plotly from "plotly.js-basic-dist-min";

/**
 * Example: Generate and display filter response plots
 */
export async function displayFilterPlots(
  containerElement: HTMLElement,
  inputCurve: CurveData,
  targetCurve: CurveData,
  deviationCurve: CurveData,
  optimizedParams: number[],
  sampleRate: number = 48000,
  numFilters: number = 5,
  iirHpPk: boolean = true,
) {
  try {
    // Prepare parameters
    const params: PlotFiltersParams = {
      input_curve: inputCurve,
      target_curve: targetCurve,
      deviation_curve: deviationCurve,
      optimized_params: optimizedParams,
      sample_rate: sampleRate,
      num_filters: numFilters,
      iir_hp_pk: iirHpPk,
    };

    // Generate plot data from Rust backend
    const plotData = await AutoEQPlotAPI.generatePlotFilters(params);

    // Apply UI-specific layout modifications
    const responsiveLayout = PlotUtils.createResponsiveLayout(800, 600);
    const customLayout = {
      ...responsiveLayout,
      title: "Filter Response Analysis",
      xaxis: { title: "Frequency (Hz)" },
      yaxis: { title: "Magnitude (dB)" },
    };

    const finalPlotData = PlotUtils.applyUILayout(plotData, customLayout);
    const config = PlotUtils.createDefaultConfig();

    // Render the plot
    await Plotly.newPlot(
      containerElement,
      finalPlotData.data as Plotly.Data[],
      finalPlotData.layout,
      config,
    );

    console.log("Filter plots rendered successfully");
  } catch (error) {
    console.error("Error generating filter plots:", error);
    throw error;
  }
}

/**
 * Example: Generate and display CEA2034 spin plots
 */
export async function displaySpinPlots(
  containerElement: HTMLElement,
  cea2034Curves?: { [key: string]: CurveData },
  eqResponse?: number[],
  frequencies?: number[],
) {
  try {
    // Prepare parameters
    const params: PlotSpinParams = {
      cea2034_curves: cea2034Curves,
      eq_response: eqResponse,
      frequencies: frequencies,
    };

    // Generate plot data from Rust backend
    const plotData = await AutoEQPlotAPI.generatePlotSpin(params);

    // Apply UI-specific layout modifications
    const responsiveLayout = PlotUtils.createResponsiveLayout(1000, 700);
    const customLayout = {
      ...responsiveLayout,
      title: "CEA2034 Spin Analysis",
      xaxis: { title: "Frequency (Hz)", type: "log" },
      yaxis: { title: "SPL (dB)" },
    };

    const finalPlotData = PlotUtils.applyUILayout(plotData, customLayout);
    const config = PlotUtils.createDefaultConfig();

    // Render the plot
    await Plotly.newPlot(
      containerElement,
      finalPlotData.data as Plotly.Data[],
      finalPlotData.layout,
      config,
    );

    console.log("Spin plots rendered successfully");
  } catch (error) {
    console.error("Error generating spin plots:", error);
    throw error;
  }
}

/**
 * Example: Generate and display detailed CEA2034 plots
 */
export async function displaySpinDetailsPlots(
  containerElement: HTMLElement,
  cea2034Curves?: { [key: string]: CurveData },
  eqResponse?: number[],
) {
  try {
    const params: PlotSpinParams = {
      cea2034_curves: cea2034Curves,
      eq_response: eqResponse,
    };

    const plotData = await AutoEQPlotAPI.generatePlotSpinDetails(params);

    const responsiveLayout = PlotUtils.createResponsiveLayout(1200, 800);
    const customLayout = {
      ...responsiveLayout,
      title: "Detailed CEA2034 Analysis",
      showlegend: true,
    };

    const finalPlotData = PlotUtils.applyUILayout(plotData, customLayout);
    const config = PlotUtils.createDefaultConfig();

    await Plotly.newPlot(
      containerElement,
      finalPlotData.data as Plotly.Data[],
      finalPlotData.layout,
      config,
    );

    console.log("Detailed spin plots rendered successfully");
  } catch (error) {
    console.error("Error generating detailed spin plots:", error);
    throw error;
  }
}

/**
 * Example: Generate and display tonal balance plots
 */
export async function displayTonalBalancePlots(
  containerElement: HTMLElement,
  cea2034Curves?: { [key: string]: CurveData },
  eqResponse?: number[],
) {
  try {
    const params: PlotSpinParams = {
      cea2034_curves: cea2034Curves,
      eq_response: eqResponse,
    };

    const plotData = await AutoEQPlotAPI.generatePlotSpinTonal(params);

    const responsiveLayout = PlotUtils.createResponsiveLayout(900, 600);
    const customLayout = {
      ...responsiveLayout,
      title: "Tonal Balance Analysis",
      xaxis: { title: "Frequency (Hz)", type: "log" },
      yaxis: { title: "Level (dB)" },
    };

    const finalPlotData = PlotUtils.applyUILayout(plotData, customLayout);
    const config = PlotUtils.createDefaultConfig();

    await Plotly.newPlot(
      containerElement,
      finalPlotData.data as Plotly.Data[],
      finalPlotData.layout,
      config,
    );

    console.log("Tonal balance plots rendered successfully");
  } catch (error) {
    console.error("Error generating tonal balance plots:", error);
    throw error;
  }
}

/**
 * Utility function to create sample curve data for testing
 */
export function createSampleCurveData(): CurveData {
  const frequencies = [];
  const spl = [];

  // Generate logarithmic frequency range from 20Hz to 20kHz
  for (let i = 0; i <= 100; i++) {
    const freq = 20 * Math.pow(1000, i / 100);
    frequencies.push(freq);

    // Generate a sample frequency response (flat with some variation)
    const variation = Math.sin(i / 10) * 2 + Math.random() * 0.5;
    spl.push(variation);
  }

  return { freq: frequencies, spl: spl };
}

/**
 * Example integration with optimization results
 */
export async function displayOptimizationResults(
  filterContainer: HTMLElement,
  spinContainer: HTMLElement,
  optimizationResult: {
    filter_params?: number[];
    filter_response?: {
      input_curve?: unknown;
      target_curve?: unknown;
      deviation_curve?: unknown;
      eq_response?: number[];
    };
    spin_details?: { curves?: Record<string, unknown> };
  },
) {
  try {
    // Extract data from optimization result
    const { filter_params, filter_response, spin_details } = optimizationResult;

    if (filter_response && filter_params) {
      // Display filter plots
      await displayFilterPlots(
        filterContainer,
        filter_response.input_curve as CurveData,
        filter_response.target_curve as CurveData,
        filter_response.deviation_curve as CurveData,
        filter_params,
        48000, // sample rate
        filter_params.length / 3, // num filters (3 params per filter)
        true, // iir_hp_pk
      );
    }

    if (spin_details) {
      // Display spin plots
      await displaySpinPlots(
        spinContainer,
        spin_details.curves as { [key: string]: CurveData },
        filter_response?.eq_response,
      );
    }

    console.log("All optimization result plots rendered successfully");
  } catch (error) {
    console.error("Error displaying optimization results:", error);
    throw error;
  }
}
