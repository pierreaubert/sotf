// Filter plot functionality

import Plotly from "plotly.js-basic-dist-min";

export class FilterPlot {
  private filterPlotElement: HTMLElement;

  constructor(filterPlotElement: HTMLElement) {
    this.filterPlotElement = filterPlotElement;
  }

  updateFilterPlot(plotData: {
    data: Plotly.Data[];
    layout: Partial<Plotly.Layout>;
  }): void {
    if (!this.filterPlotElement) {
      console.error("[FILTER PLOT] Filter plot element not found!");
      return;
    }

    // Show the filter plot container first
    const filterVerticalItem = document.getElementById("filter_vertical_item");
    if (filterVerticalItem) {
      filterVerticalItem.style.display = "flex";
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
        const layout: Partial<Plotly.Layout> = {
          ...plotData.layout,
          autosize: true,
          height: 650, // Fixed height for consistent display
          width: 800, // Fixed height for consistent display
          legend: {
            ...(plotData.layout.legend || {}),
            orientation: "h" as const,
            x: 0.5,
            xanchor: "center",
            y: 1.3,
            yanchor: "top",
          },
          margin: {
            ...(plotData.layout.margin || {}),
            t: 80,
          },
        };

        Plotly.newPlot(
          this.filterPlotElement,
          plotData.data,
          layout,
          config,
        ).then(() => {
          this.filterPlotElement.classList.add("has-plot");
          this.showPlotContainer("filter_plot");
          setTimeout(() => Plotly.Plots.resize(this.filterPlotElement), 100);
        });
      } else {
        console.warn(
          "[FILTER PLOT] Invalid filter plot data structure:",
          plotData,
        );
      }
    } catch (error) {
      console.error("[FILTER PLOT] Error creating filter plot:", error);
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
