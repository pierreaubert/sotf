// Base plot functionality and visibility management

import Plotly from "plotly.js-basic-dist-min";
import { PlotData } from "../../types";

export class PlotBase {
  // Plot data caching
  protected filterPlotsData: PlotData | null = null;
  protected lastSpinDetails: PlotData | null = null;

  showSpinVerticalItems(): void {
    const verticalItems = [
      "spin_vertical_item",
      "details_vertical_item",
      "tonal_vertical_item",
    ];
    verticalItems.forEach((id) => {
      const element = document.getElementById(id);
      if (element) {
        element.style.display = "flex";
      }
    });
  }

  hideSpinVerticalItems(): void {
    const verticalItems = [
      "spin_vertical_item",
      "details_vertical_item",
      "tonal_vertical_item",
    ];
    verticalItems.forEach((id) => {
      const element = document.getElementById(id);
      if (element) {
        element.style.display = "none";
      }
    });
  }

  showPlotContainer(plotId: string): void {
    // For compatibility with existing code, but now we manage at vertical item level
    const verticalItemMap: { [key: string]: string } = {
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

  hidePlotContainer(plotId: string): void {
    // For compatibility with existing code, but now we manage at vertical item level
    const verticalItemMap: { [key: string]: string } = {
      spin_plot: "spin_vertical_item",
      details_plot: "details_vertical_item",
      tonal_plot: "tonal_vertical_item",
    };

    const verticalItemId = verticalItemMap[plotId];
    if (verticalItemId) {
      const element = document.getElementById(verticalItemId);
      if (element) {
        element.style.display = "none";
      }
    }
  }

  configureVerticalVisibility(hasSpinData: boolean): void {
    if (hasSpinData) {
      // Speaker-based: show all 3 graphs (Filter Response + 2 spinorama graphs)
      this.showSpinVerticalItems();
    } else {
      // Curve+target: only show Filter Response graph
      this.hideSpinVerticalItems();
    }
  }

  configureGridVisibility(hasSpinData: boolean): void {
    this.configureVerticalVisibility(hasSpinData);
  }

  // Deprecated method - kept for compatibility
  configureAccordionVisibility(hasSpinData: boolean): void {
    this.configureGridVisibility(hasSpinData);
  }

  // Getters for cached data
  getLastSpinDetails(): PlotData | null {
    return this.lastSpinDetails;
  }

  setLastSpinDetails(data: PlotData | null): void {
    this.lastSpinDetails = data;
  }

  getFilterPlotsData(): PlotData | null {
    return this.filterPlotsData;
  }

  setFilterPlotsData(data: PlotData | null): void {
    this.filterPlotsData = data;
  }

  protected clearPlotElement(element: HTMLElement | null): void {
    if (element) {
      try {
        Plotly.purge(element);
      } catch (_e) {
        // Element may not have been plotted yet
      }
      element.innerHTML =
        '<div class="plot-placeholder">No data to display</div>';
      element.classList.remove("has-plot");
    }
  }
}
