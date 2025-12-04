// Tonal balance plot functionality

import Plotly from "plotly.js-basic-dist-min";

export class TonalPlot {
  private tonalPlotElement: HTMLElement | null = null;

  constructor(tonalPlotElement?: HTMLElement) {
    this.tonalPlotElement = tonalPlotElement || null;
  }

  updateTonalPlot(plotData: {
    data: Plotly.Data[];
    layout: Partial<Plotly.Layout>;
  }): void {
    if (!this.tonalPlotElement) {
      console.warn("Tonal plot element not available");
      return;
    }

    // Show the tonal plot container first
    const tonalVerticalItem = document.getElementById("tonal_vertical_item");
    if (tonalVerticalItem) {
      tonalVerticalItem.style.display = "flex";
    }

    try {
      if (plotData && plotData.data && plotData.layout) {
        // The backend provides subplot configuration in the layout
        const config = {
          responsive: true,
          displayModeBar: false,
          displaylogo: false,
        };

        // Adjust layout for responsive display
        const layout = {
          ...plotData.layout,
          autosize: true,
          height: 650, // Fixed height for consistent display
          width: 800, // Fixed width for consistent display
          grid: {
            ...(plotData.layout.grid || {}),
            rows: 2,
            columns: 4,
            pattern: "independent" as const,
          },
          legend: {
            ...(plotData.layout.legend || {}),
            orientation: "h" as const,
            x: 0.5,
            xanchor: "center" as const,
            y: 1.2,
            yanchor: "top" as const,
          },
        };

        Plotly.newPlot(
          this.tonalPlotElement,
          plotData.data,
          layout,
          config,
        ).then(() => {
          this.tonalPlotElement!.classList.add("has-plot");
          this.showPlotContainer("tonal_plot");
          setTimeout(() => Plotly.Plots.resize(this.tonalPlotElement!), 100);
        });
      } else {
        console.warn("Invalid tonal plot data structure:", plotData);
      }
    } catch (error) {
      console.error("Error creating tonal plot:", error);
    }
  }

  private showPlotContainer(plotId: string): void {
    const verticalItemMap: { [key: string]: string } = {
      filter_plot: "filter_vertical_item",
      spin_plot: "spin_vertical_item",
      details_plot: "details_vertical_item",
      tonal_plot: "tonal_vertical_item",
    };

    const verticalItemId = verticalItemMap[plotId];
    if (verticalItemId) {
      const element = document.getElementById(verticalItemId);
      if (element) {
        element.style.display = "flex";
      }
    }
  }
}
