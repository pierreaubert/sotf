// TypeScript interfaces for AutoEQ plot functions

export interface CurveData {
  freq: number[];
  spl: number[];
}

export interface PlotFiltersParams {
  input_curve: CurveData;
  target_curve: CurveData;
  deviation_curve: CurveData;
  optimized_params: number[];
  sample_rate: number;
  num_filters: number;
  peq_model?: "pk" | "hp-pk" | "hp-pk-lp" | "free-pk-free" | "free";
  iir_hp_pk: boolean; // Deprecated, kept for backward compatibility
}

export interface PlotSpinParams {
  cea2034_curves?: { [key: string]: CurveData };
  eq_response?: number[];
  frequencies?: number[];
}

// Plotly plot data structure (simplified)
import Plotly from "plotly.js-dist-min";

export interface PlotlyData {
  data: Plotly.Data[];
  layout: Partial<Plotly.Layout>;
  config?: Record<string, unknown>;
}

// AutoEQ plot API functions
export class AutoEQPlotAPI {
  private static async invoke<T>(command: string, params: unknown): Promise<T> {
    // @ts-expect-error - Tauri invoke function
    return window.__TAURI__.core.invoke(command, params);
  }

  /**
   * Generate filter response plots
   */
  static async generatePlotFilters(
    params: PlotFiltersParams,
  ): Promise<PlotlyData> {
    return this.invoke<PlotlyData>("generate_plot_filters", { params });
  }

  /**
   * Generate CEA2034 spin plot
   */
  static async generatePlotSpin(params: PlotSpinParams): Promise<PlotlyData> {
    return this.invoke<PlotlyData>("generate_plot_spin", { params });
  }

  /**
   * Generate detailed CEA2034 spin plot
   */
  static async generatePlotSpinDetails(
    params: PlotSpinParams,
  ): Promise<PlotlyData> {
    return this.invoke<PlotlyData>("generate_plot_spin_details", { params });
  }

  /**
   * Generate tonal balance CEA2034 plot
   */
  static async generatePlotSpinTonal(
    params: PlotSpinParams,
  ): Promise<PlotlyData> {
    return this.invoke<PlotlyData>("generate_plot_spin_tonal", { params });
  }
}

// Utility functions for working with plots
export class PlotUtils {
  /**
   * Apply custom layout modifications to a Plotly plot
   */
  static applyUILayout(
    plotData: PlotlyData,
    customLayout: Partial<Record<string, unknown>>,
  ): PlotlyData {
    return {
      ...plotData,
      layout: {
        ...plotData.layout,
        ...customLayout,
      },
    };
  }

  /**
   * Create a responsive layout configuration
   */
  static createResponsiveLayout(
    width?: number,
    height?: number,
  ): Partial<Record<string, unknown>> {
    return {
      autosize: true,
      responsive: true,
      ...(width && { width }),
      ...(height && { height }),
      margin: {
        l: 50,
        r: 50,
        t: 50,
        b: 50,
      },
    };
  }

  /**
   * Create default config for Plotly plots
   */
  static createDefaultConfig(): Record<string, unknown> {
    return {
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
    };
  }
}
