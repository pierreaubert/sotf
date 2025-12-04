// EQ Plot Generation using Plotly and browser-based biquad filters

import Plotly from "plotly.js-basic-dist-min";
import { BiquadFilter, type FilterType } from "./biquad-filter";
import type { FilterParam } from "./plugin-eq";

export interface EQPlotConfig {
  minFreq?: number;
  maxFreq?: number;
  numPoints?: number;
  sampleRate?: number;
  showIndividualFilters?: boolean;
  height?: number;
}

const DEFAULT_CONFIG: Required<EQPlotConfig> = {
  minFreq: 20,
  maxFreq: 20000,
  numPoints: 200,
  sampleRate: 48000,
  showIndividualFilters: true,
  height: 400,
};

/**
 * Generate EQ plot using Plotly
 */
export function generateEQPlot(
  container: HTMLElement,
  filters: FilterParam[],
  config: EQPlotConfig = {},
): void {
  const cfg = { ...DEFAULT_CONFIG, ...config };

  // Generate frequency array
  const frequencies = BiquadFilter.generateLogFrequencies(
    cfg.minFreq,
    cfg.maxFreq,
    cfg.numPoints,
  );

  // Create biquad filters for enabled filters
  const biquadFilters: BiquadFilter[] = [];
  const individualResponses: number[][] = [];

  for (const filter of filters) {
    if (!filter.enabled) continue;

    const biquad = new BiquadFilter(
      filter.filter_type as FilterType,
      filter.frequency,
      cfg.sampleRate,
      filter.q,
      filter.gain,
    );

    biquadFilters.push(biquad);

    // Compute individual response
    const response = biquad.computeFrequencyResponse(frequencies);
    individualResponses.push(response);
  }

  // Compute combined response (sum in dB domain)
  const combinedResponse = new Array(frequencies.length).fill(0);
  for (const response of individualResponses) {
    for (let i = 0; i < frequencies.length; i++) {
      combinedResponse[i] += response[i];
    }
  }

  // Create Plotly traces
  const traces: Plotly.Data[] = [];

  // Add individual filter traces
  if (cfg.showIndividualFilters && biquadFilters.length > 0) {
    biquadFilters.forEach((biquad, idx) => {
      const filterLabel = getFilterLabel(biquad, idx);
      traces.push({
        x: frequencies,
        y: individualResponses[idx],
        type: "scatter",
        mode: "lines",
        name: filterLabel,
        line: {
          width: 2,
        },
        opacity: 0.5,
        hovertemplate:
          `<b>${filterLabel}</b><br>` +
          "Frequency: %{x:.0f} Hz<br>" +
          "Gain: %{y:.2f} dB<br>" +
          "<extra></extra>",
      } as Plotly.Data);
    });
  }

  // Add combined response trace
  if (biquadFilters.length > 0) {
    traces.push({
      x: frequencies,
      y: combinedResponse,
      type: "scatter",
      mode: "lines",
      name: "Combined Response",
      line: {
        width: 3,
        color: "#00bfff",
        dash: "dash",
      },
      hovertemplate:
        "<b>Combined Response</b><br>" +
        "Frequency: %{x:.0f} Hz<br>" +
        "Gain: %{y:.2f} dB<br>" +
        "<extra></extra>",
    } as Plotly.Data);
  }

  // Create layout
  const layout: Partial<Plotly.Layout> = {
    title: {
      text: `Parametric EQ Response (${biquadFilters.length} filters @ ${cfg.sampleRate}Hz)`,
      font: { size: 14 },
    } as any,
    xaxis: {
      title: { text: "Frequency (Hz)" } as any,
      type: "log",
      range: [Math.log10(cfg.minFreq), Math.log10(cfg.maxFreq)],
      gridcolor: "rgba(128, 128, 128, 0.2)",
      showgrid: true,
    },
    yaxis: {
      title: { text: "Magnitude (dB)" } as any,
      gridcolor: "rgba(128, 128, 128, 0.2)",
      showgrid: true,
      zeroline: true,
      zerolinecolor: "rgba(255, 255, 255, 0.3)",
      zerolinewidth: 2,
    },
    height: cfg.height,
    margin: { t: 60, r: 20, b: 60, l: 60 },
    paper_bgcolor: "rgba(26, 26, 26, 1)",
    plot_bgcolor: "rgba(26, 26, 26, 1)",
    font: { color: "#ffffff" },
    showlegend: false,
    hovermode: "closest",
  };

  // Plot config
  const plotConfig: Partial<Plotly.Config> = {
    responsive: true,
    displayModeBar: true,
    displaylogo: false,
    modeBarButtonsToRemove: ["lasso2d", "select2d"],
    modeBarButtonsToAdd: [],
  };

  // Render plot
  Plotly.newPlot(container, traces, layout, plotConfig);
}

/**
 * Get a human-readable label for a filter
 */
function getFilterLabel(biquad: BiquadFilter, index: number): string {
  const typeShort = getFilterTypeShort(biquad.filterType);
  return `${typeShort}${index + 1} @ ${Math.round(biquad.frequency)}Hz`;
}

/**
 * Get short name for filter type
 */
function getFilterTypeShort(type: FilterType): string {
  switch (type) {
    case "Peak":
      return "PK";
    case "Lowshelf":
      return "LS";
    case "Highshelf":
      return "HS";
    case "Lowpass":
      return "LP";
    case "Highpass":
      return "HP";
    case "Bandpass":
      return "BP";
    case "Notch":
      return "NO";
    default:
      return "F";
  }
}

/**
 * Update existing plot without recreating
 */
export function updateEQPlot(
  container: HTMLElement,
  filters: FilterParam[],
  config: EQPlotConfig = {},
): void {
  // For now, just regenerate the plot
  // Can be optimized later with Plotly.react() for performance
  generateEQPlot(container, filters, config);
}

/**
 * Add click handler to plot for filter selection
 */
export function addPlotClickHandler(
  container: HTMLElement,
  filters: FilterParam[],
  onFilterSelect: (filterIndex: number) => void,
): void {
  // Use Plotly's event system
  (container as any).on("plotly_click", (data: any) => {
    if (!data.points || data.points.length === 0) return;

    const point = data.points[0];
    const curveNumber = point.curveNumber;

    // Last curve is combined response, before that are individual filters
    const enabledFilters = filters.filter((f) => f.enabled);
    if (curveNumber < enabledFilters.length) {
      // Map back to original filter index
      let enabledIdx = 0;
      for (let i = 0; i < filters.length; i++) {
        if (filters[i].enabled) {
          if (enabledIdx === curveNumber) {
            onFilterSelect(i);
            return;
          }
          enabledIdx++;
        }
      }
    }
  });
}
