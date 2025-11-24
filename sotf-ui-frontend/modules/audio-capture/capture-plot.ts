// Capture Plot Module
// Handles frequency response plotting for capture recordings

import Plotly from "plotly.js-basic-dist-min";
import type { RecordingResult } from "./capture-tauri";

export interface PlotConfig {
  showPhase: boolean;
  minFreq: number;
  maxFreq: number;
  minDb: number;
  maxDb: number;
  minPhase: number;
  maxPhase: number;
}

const DEFAULT_CONFIG: PlotConfig = {
  showPhase: true,
  minFreq: 20,
  maxFreq: 20000,
  minDb: -40,
  maxDb: 10,
  minPhase: -180,
  maxPhase: 180,
};

/**
 * Plot frequency response for multiple channels
 */
export function plotFrequencyResponse(
  container: HTMLElement,
  results: RecordingResult[],
  config: Partial<PlotConfig> = {},
): void {
  const cfg = { ...DEFAULT_CONFIG, ...config };

  // Create magnitude traces
  const magnitudeTraces = results.map((result, idx) => ({
    x: result.frequencies,
    y: result.magnitude_db,
    name: `Channel ${result.channel + 1} (Mag)`,
    type: "scatter" as const,
    mode: "lines" as const,
    line: {
      width: 2,
    },
    yaxis: "y1",
  }));

  // Create phase traces if enabled
  const phaseTraces = cfg.showPhase
    ? results.map((result, idx) => ({
        x: result.frequencies,
        y: result.phase_deg,
        name: `Channel ${result.channel + 1} (Phase)`,
        type: "scatter" as const,
        mode: "lines" as const,
        line: {
          width: 1,
          dash: "dot" as const,
        },
        yaxis: "y2",
      }))
    : [];

  const data = [...magnitudeTraces, ...phaseTraces];

  const layout: Partial<Plotly.Layout> = {
    title: {
      text: "Frequency Response (20 Hz - 20 kHz)",
    },
    showlegend: true,
    legend: {
      x: 1.05,
      y: 1,
      xanchor: "left",
    },
    xaxis: {
      title: {
        text: "Frequency (Hz)",
      },
      type: "log",
      range: [Math.log10(cfg.minFreq), Math.log10(cfg.maxFreq)],
      tickvals: [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000],
      ticktext: [
        "20",
        "50",
        "100",
        "200",
        "500",
        "1k",
        "2k",
        "5k",
        "10k",
        "20k",
      ],
      gridcolor: "#e0e0e0",
    },
    yaxis: {
      title: {
        text: "Magnitude (dB)",
      },
      range: [cfg.minDb, cfg.maxDb],
      gridcolor: "#e0e0e0",
    },
    ...(cfg.showPhase && {
      yaxis2: {
        title: {
          text: "Phase (degrees)",
        },
        overlaying: "y",
        side: "right",
        range: [cfg.minPhase, cfg.maxPhase],
        gridcolor: "#f0f0f0",
      },
    }),
    margin: {
      l: 60,
      r: cfg.showPhase ? 60 : 20,
      t: 40,
      b: 60,
    },
    plot_bgcolor: "#fafafa",
    paper_bgcolor: "#ffffff",
    hovermode: "x unified",
  };

  const plotConfig: Partial<Plotly.Config> = {
    responsive: true,
    displayModeBar: true,
    modeBarButtonsToRemove: ["lasso2d", "select2d"],
    displaylogo: false,
  };

  Plotly.newPlot(container, data, layout, plotConfig);
}

/**
 * Update existing plot with new data
 */
export function updatePlot(
  container: HTMLElement,
  results: RecordingResult[],
  config: Partial<PlotConfig> = {},
): void {
  const cfg = { ...DEFAULT_CONFIG, ...config };

  // Create magnitude traces
  const magnitudeTraces = results.map((result) => ({
    x: result.frequencies,
    y: result.magnitude_db,
  }));

  // Create phase traces if enabled
  const phaseTraces = cfg.showPhase
    ? results.map((result) => ({
        x: result.frequencies,
        y: result.phase_deg,
      }))
    : [];

  const allTraces = [...magnitudeTraces, ...phaseTraces];

  // Update each trace
  allTraces.forEach((trace, idx) => {
    Plotly.restyle(container, { x: [trace.x], y: [trace.y] }, idx);
  });
}

/**
 * Clear plot
 */
export function clearPlot(container: HTMLElement): void {
  Plotly.purge(container);
}

/**
 * Resize plot to fit container
 */
export function resizePlot(container: HTMLElement): void {
  Plotly.Plots.resize(container);
}
