// Spin plot functionality

import Plotly from "plotly.js-basic-dist-min";

export class SpinPlot {
  private spinPlotElement: HTMLElement;

  constructor(spinPlotElement: HTMLElement) {
    this.spinPlotElement = spinPlotElement;
  }

  updateSpinPlot(plotData: {
    data: Plotly.Data[];
    layout: Partial<Plotly.Layout>;
  }): void {
    if (!this.spinPlotElement) {
      console.error("Spin plot element not found!");
      return;
    }

    // Show the spin plot container first
    const spinVerticalItem = document.getElementById("spin_vertical_item");
    if (spinVerticalItem) {
      spinVerticalItem.style.display = "flex";
    }

    try {
      if (plotData && plotData.data && plotData.layout) {
        // The backend provides configuration in the layout
        const config = {
          responsive: true,
          displayModeBar: false,
          displaylogo: false,
        };

        // Adjust layout for responsive display
        const layout = {
          ...plotData.layout,
          autosize: true,
          height: 450, // Fixed height for consistent display
          width: 800, // Fixed height for consistent display
          grid: {
            ...(plotData.layout.grid || {}),
            rows: 1,
            columns: 2,
            pattern: "independent" as const,
          },
          legend: {
            ...(plotData.layout.legend || {}),
            orientation: "h" as const,
            x: 0.5,
            xanchor: "center" as const,
            y: 1.3,
            yanchor: "top" as const,
          },
        };

        Plotly.newPlot(
          this.spinPlotElement,
          plotData.data,
          layout,
          config,
        ).then(() => {
          this.spinPlotElement.classList.add("has-plot");
          this.showPlotContainer("spin_plot");
          setTimeout(() => Plotly.Plots.resize(this.spinPlotElement), 100);
        });
      } else {
        console.warn("Invalid spin plot data structure:", plotData);
      }
    } catch (error) {
      console.error("Error creating spin plot:", error);
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
