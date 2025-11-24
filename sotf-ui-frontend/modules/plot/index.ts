// Main PlotComposer export - combines all plot modules

import { PlotBase } from "./base";
import { FilterPlot } from "./filter-plot";
import { SpinPlot } from "./spin-plot";
import { DetailsPlot } from "./details-plot";
import { TonalPlot } from "./tonal-plot";
import { ProgressPlot } from "./progress-plot";
import Plotly from "plotly.js-basic-dist-min";

export class PlotComposer extends PlotBase {
  private filterPlot: FilterPlot;
  private spinPlot: SpinPlot;
  private detailsPlot: DetailsPlot;
  private tonalPlot: TonalPlot;
  private progressPlot: ProgressPlot;

  private filterPlotElement: HTMLElement;
  private detailsPlotElement: HTMLElement | null;
  private spinPlotElement: HTMLElement;

  constructor(
    filterPlotElement: HTMLElement,
    detailsPlotElement: HTMLElement | null,
    spinPlotElement: HTMLElement,
    progressGraphElement?: HTMLElement,
    tonalPlotElement?: HTMLElement,
  ) {
    super();

    this.filterPlotElement = filterPlotElement;
    this.detailsPlotElement = detailsPlotElement;
    this.spinPlotElement = spinPlotElement;

    // Initialize plot modules
    this.filterPlot = new FilterPlot(filterPlotElement);
    this.spinPlot = new SpinPlot(spinPlotElement);
    this.detailsPlot = new DetailsPlot(detailsPlotElement);
    this.tonalPlot = new TonalPlot(tonalPlotElement);
    this.progressPlot = new ProgressPlot(progressGraphElement);
  }

  clearAllPlots(): void {
    const allPlotElements = [
      this.filterPlotElement,
      this.detailsPlotElement,
      this.spinPlotElement,
    ].filter(Boolean); // Filter out null elements

    // Also clear progress graph
    this.clearProgressGraph();

    try {
      allPlotElements.forEach((element) => {
        if (element) {
          this.clearPlotElement(element);
        }
      });

      // Hide spinorama vertical items by default
      this.hideSpinVerticalItems();
    } catch (error) {
      console.error("Error clearing plots:", error);
    }
  }

  // Filter plot methods
  updateFilterPlot(plotData: {
    data: Plotly.Data[];
    layout: Partial<Plotly.Layout>;
    config?: Record<string, unknown>;
  }): void {
    this.filterPlot.updateFilterPlot(plotData);
  }

  // Spin plot methods
  updateSpinPlot(plotData: {
    data: Plotly.Data[];
    layout: Partial<Plotly.Layout>;
    config?: Record<string, unknown>;
  }): void {
    this.spinPlot.updateSpinPlot(plotData);
  }

  // Details plot methods
  async generateDetailsPlot(plotData: {
    data: Plotly.Data[];
    layout: Partial<Plotly.Layout>;
    config?: Record<string, unknown>;
  }): Promise<void> {
    await this.detailsPlot.generateDetailsPlot(plotData);
  }

  // Tonal plot methods
  updateTonalPlot(plotData: {
    data: Plotly.Data[];
    layout: Partial<Plotly.Layout>;
    config?: Record<string, unknown>;
  }): void {
    this.tonalPlot.updateTonalPlot(plotData);
  }

  // Progress graph methods
  clearProgressGraph(): void {
    this.progressPlot.clearProgressGraph();
  }

  addProgressData(
    iteration: number,
    fitness: number,
    convergence: number,
  ): void {
    this.progressPlot.addProgressData(iteration, fitness, convergence);
  }

  async updateProgressGraph(): Promise<void> {
    await this.progressPlot.updateProgressGraph();
  }

  getProgressData(): Array<{
    iteration: number;
    fitness: number;
    convergence: number;
  }> {
    return this.progressPlot.getProgressData();
  }
}
