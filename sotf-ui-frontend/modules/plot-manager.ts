// Layout management for responsive 4-graph grid

export class PlotManager {
  private plotsGridElement: HTMLElement | null = null;
  private isInitialized: boolean = false;

  constructor() {
    this.initialize();
  }

  private initialize(): void {
    this.plotsGridElement = document.querySelector(".plots-vertical");
    if (!this.plotsGridElement) {
      console.warn("[LAYOUT] Plots vertical element not found");
      return;
    }

    // Add resize listener
    window.addEventListener("resize", this.handleResize.bind(this));

    // Initial calculation
    this.calculateLayout();

    this.isInitialized = true;
  }

  private handleResize = (): void => {
    // Debounce resize events
    clearTimeout((this as { resizeTimeout?: number }).resizeTimeout);
    (this as { resizeTimeout?: number }).resizeTimeout = setTimeout(() => {
      this.calculateLayout();
    }, 100);
  };

  public calculateLayout(): void {
    if (!this.plotsGridElement) return;

    const rightPanel = document.getElementById("right_panel");
    if (!rightPanel) return;

    // Get available dimensions
    const rightPanelRect = rightPanel.getBoundingClientRect();
    void this.plotsGridElement.getBoundingClientRect();

    // Calculate available space for plots (excluding padding, margins, headers)
    const availableWidth = rightPanelRect.width - 40; // Account for panel padding
    const availableHeight = rightPanelRect.height - 120; // Account for scores display, headers and other elements

    // Update CSS custom properties for dynamic sizing
    document.documentElement.style.setProperty(
      "--plots-vertical-width",
      `${availableWidth}px`,
    );
    document.documentElement.style.setProperty(
      "--plots-vertical-height",
      `${availableHeight}px`,
    );

    // Calculate individual plot dimensions for 3 vertically stacked graphs
    const isMobile = window.innerWidth <= 768;
    const isTablet = window.innerWidth <= 1024;

    if (isMobile) {
      // 3 vertically stacked graphs on mobile
      const plotHeight = Math.max(120, (availableHeight - 30) / 3); // 30px for gaps between 3 graphs
      document.documentElement.style.setProperty(
        "--plot-vertical-height",
        `${plotHeight}px`,
      );
    } else if (isTablet) {
      // 3 vertically stacked graphs on tablet
      const plotHeight = Math.max(150, (availableHeight - 30) / 3); // 30px for gaps between 3 graphs
      document.documentElement.style.setProperty(
        "--plot-vertical-height",
        `${plotHeight}px`,
      );
    } else {
      // 3 vertically stacked graphs on desktop
      const plotHeight = Math.max(200, (availableHeight - 30) / 3); // 30px for gaps between 3 graphs
      document.documentElement.style.setProperty(
        "--plot-vertical-height",
        `${plotHeight}px`,
      );
    }
  }

  public resizePlots(): void {
    if (!this.isInitialized) return;

    // Force Plotly plots to resize
    const plotContainers = document.querySelectorAll(
      ".plot-vertical-container.has-plot",
    );
    plotContainers.forEach((container) => {
      const plotlyDiv = container.querySelector(
        ".js-plotly-plot",
      ) as HTMLElement;
      if (
        plotlyDiv &&
        (
          window as {
            Plotly?: { Plots: { resize: (el: HTMLElement) => void } };
          }
        ).Plotly
      ) {
        try {
          (
            window as {
              Plotly: { Plots: { resize: (el: HTMLElement) => void } };
            }
          ).Plotly.Plots.resize(plotlyDiv);
        } catch (_e) {
          // Ignore resize errors for plots that may not be fully initialized
        }
      }
    });
  }

  public forceRecalculate(): void {
    setTimeout(() => {
      this.calculateLayout();
      this.resizePlots();
    }, 50);
  }

  public destroy(): void {
    window.removeEventListener("resize", this.handleResize);
    clearTimeout((this as { resizeTimeout?: number }).resizeTimeout);
    this.isInitialized = false;
  }
}
