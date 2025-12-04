// Progress plot functionality

import Plotly from "plotly.js-basic-dist-min";

// Progress data interface
interface ProgressData {
  iteration: number;
  fitness: number;
  convergence: number;
  timestamp?: number;
}

export class ProgressPlot {
  private progressGraphElement: HTMLElement | null = null;
  private progressData: ProgressData[] = [];

  constructor(progressGraphElement?: HTMLElement) {
    this.progressGraphElement = progressGraphElement || null;
  }

  clearProgressGraph(): void {
    if (this.progressGraphElement) {
      try {
        Plotly.purge(this.progressGraphElement);
      } catch (_e) {
        // Element may not have been plotted yet
      }
      this.progressGraphElement.innerHTML = "";
      // Add a placeholder to show the element exists
      this.progressGraphElement.innerHTML =
        '<div style="text-align: center; padding: 20px; color: #666;">Waiting for optimization data...</div>';
    }
    this.progressData = [];
  }

  addProgressData(
    iteration: number,
    fitness: number,
    convergence: number,
  ): void {
    this.progressData.push({
      iteration,
      fitness,
      convergence,
      timestamp: Date.now(),
    });
  }

  async updateProgressGraph(): Promise<void> {
    if (!this.progressGraphElement) {
      console.error("[PLOT DEBUG] Progress graph element not found!");
      return;
    }
    if (this.progressData.length === 0) {
      return;
    }

    const iterations = this.progressData.map((d) => d.iteration);
    const fitness = this.progressData.map((d) => d.fitness);
    const convergence = this.progressData.map((d) => d.convergence);

    const fitnessTrace = {
      x: iterations,
      y: fitness,
      type: "scatter" as const,
      mode: "lines+markers" as const,
      name: "Fitness (f)",
      yaxis: "y",
      line: { color: "#007bff", width: 2 },
      marker: { size: 4 },
    };

    const convergenceTrace = {
      x: iterations,
      y: convergence,
      type: "scatter" as const,
      mode: "lines+markers" as const,
      name: "Convergence",
      yaxis: "y2",
      line: { color: "#ff7f0e", width: 2 },
      marker: { size: 4 },
    };

    const layout = {
      title: {
        text: "Optimization Progress",
        font: { size: 14 },
      },
      width: 400,
      height: 400,
      margin: { l: 60, r: 60, t: 40, b: 40 },
      xaxis: {
        title: { text: "Iterations" },
        showgrid: true,
        zeroline: false,
      },
      yaxis: {
        title: {
          text: "Fitness (f)",
          font: { color: "#007bff" },
        },
        side: "left" as const,
        showgrid: true,
        zeroline: false,
        tickfont: { color: "#007bff" },
      },
      yaxis2: {
        title: {
          text: "Convergence",
          font: { color: "#ff7f0e" },
        },
        side: "right" as const,
        overlaying: "y" as const,
        showgrid: false,
        zeroline: false,
        tickfont: { color: "#ff7f0e" },
      },
      paper_bgcolor: "rgba(0,0,0,0)",
      plot_bgcolor: "rgba(0,0,0,0)",
      font: {
        color: getComputedStyle(document.documentElement)
          .getPropertyValue("--text-primary")
          .trim(),
        size: 11,
      },
      showlegend: true,
      legend: {
        x: 0,
        y: 1,
        bgcolor: "rgba(0,0,0,0)",
      },
      hovermode: "x unified" as const,
    };

    const config = {
      responsive: false,
      displayModeBar: false,
      staticPlot: false,
    };

    try {
      // Clear placeholder text
      if (
        this.progressGraphElement.innerHTML.includes(
          "Waiting for optimization data",
        )
      ) {
        this.progressGraphElement.innerHTML = "";
      }

      if (
        this.progressGraphElement.hasChildNodes() &&
        this.progressGraphElement.children.length > 0
      ) {
        // Update existing plot
        await Plotly.react(
          this.progressGraphElement,
          [fitnessTrace, convergenceTrace],
          layout,
          config,
        );
      } else {
        // Create new plot
        await Plotly.newPlot(
          this.progressGraphElement,
          [fitnessTrace, convergenceTrace],
          layout,
          config,
        );
      }
    } catch (error) {
      console.error("[PLOT DEBUG] ❌ Error updating progress graph:", error);
      // Add error message to the element
      this.progressGraphElement.innerHTML = `<div style="text-align: center; padding: 20px; color: #dc3545;">Error creating progress graph: ${error}</div>`;
    }
  }

  getProgressData(): ProgressData[] {
    return [...this.progressData];
  }
}
