// EQ Plugin
// Parametric equalizer with visual frequency response

import "bulma/css/bulma.min.css";
import { BasePlugin } from "./plugin-base";
import { PluginMenubar } from "./plugin-menubar";
import type { PluginMetadata, PluginConfig } from "./plugin-types";
import { generateEQPlot, addPlotClickHandler } from "./eq-plotter";
import { ShortcutsModal, type ShortcutItem } from "./shortcuts-modal";
import Plotly from "plotly.js-basic-dist-min";

export interface FilterParam {
  filter_type: string; // "Peak", "Lowshelf", "Highshelf", etc.
  frequency: number;
  q: number;
  gain: number;
  enabled: boolean;
}

// Filter type definitions
const FILTER_TYPES = {
  Peak: { label: "Peak", shortName: "PK", icon: "○" },
  Lowpass: { label: "Low Pass", shortName: "LP", icon: "╲" },
  Highpass: { label: "High Pass", shortName: "HP", icon: "╱" },
  Bandpass: { label: "Band Pass", shortName: "BP", icon: "∩" },
  Notch: { label: "Notch", shortName: "NO", icon: "V" },
  Lowshelf: { label: "Low Shelf", shortName: "LS", icon: "⎣" },
  Highshelf: { label: "High Shelf", shortName: "HS", icon: "⎤" },
};

/**
 * EQ Plugin
 * Visual parametric equalizer
 */
export class EQPlugin extends BasePlugin {
  public readonly metadata: PluginMetadata = {
    id: "eq-plugin",
    name: "SotF: EQ",
    category: "eq",
    version: "1.0.0",
  };

  // UI components
  private menubar: PluginMenubar | null = null;
  private shortcutsModal: ShortcutsModal | null = null;

  // UI elements
  private plotContainer: HTMLElement | null = null;
  private filterTable: HTMLElement | null = null;
  private filterPopup: HTMLElement | null = null;

  // EQ state
  private filters: FilterParam[] = [
    { filter_type: "Peak", frequency: 100, q: 1.0, gain: 0, enabled: true },
    { filter_type: "Peak", frequency: 1000, q: 1.0, gain: 0, enabled: true },
    { filter_type: "Peak", frequency: 10000, q: 1.0, gain: 0, enabled: true },
  ];
  private selectedFilterIndex: number = -1;

  // Plot update debounce
  private plotUpdateDebounceTimer: number | null = null;

  /**
   * Render the plugin UI
   */
  render(standalone: boolean): void {
    if (!this.container) return;

    this.container.innerHTML = `
      <div class="is-flex is-flex-direction-column eq-plugin ${standalone ? "standalone" : "embedded"}" style="height: 100%; background: #1a1a1a;">
        ${standalone ? '<div class="eq-menubar-container"></div>' : ""}
        <div class="is-flex is-flex-direction-column is-flex-grow-1" style="gap: 0; padding: 0; overflow-y: auto;">
          <!-- Plotly EQ Graph -->
          <div class="eq-plot-container" style="width: 100%; max-width: 900px; flex: 1; min-height: 150px; margin: 0 auto; background: #1a1a1a; border: 1px solid #404040;" id="eq-plot-${this.metadata.id}"></div>

          <!-- Space for text between graph and table -->
          <div class="is-flex is-align-items-center is-justify-content-center" style="width: 100%; max-width: 900px; height: 20px; margin: 0 auto; font-size: 12px; color: #888;"></div>

          <!-- Filter Table -->
          <div class="box" style="width: 100%; max-width: 900px; margin: 0 auto; background: #1a1a1a; border: 1px solid #404040; padding: 0;">
            <table class="table is-bordered is-striped is-narrow is-hoverable is-fullwidth is-dark eq-table"></table>
          </div>
        </div>

        <!-- Filter Popup (hidden by default) -->
        <div class="eq-filter-popup" style="display: none;">
          <div class="popup-header">
            <span class="popup-title"></span>
            <button class="popup-close">×</button>
          </div>
          <div class="popup-body"></div>
        </div>
      </div>
    `;

    // Initialize menubar if standalone
    if (standalone) {
      const menubarContainer = this.container.querySelector(
        ".eq-menubar-container",
      ) as HTMLElement;
      if (menubarContainer) {
        this.menubar = new PluginMenubar(menubarContainer, this.metadata.name);
      }
    }

    // Cache elements
    this.plotContainer = this.container.querySelector(
      `#eq-plot-${this.metadata.id}`,
    ) as HTMLElement;
    this.filterTable = this.container.querySelector(".eq-table") as HTMLElement;
    this.filterPopup = this.container.querySelector(
      ".eq-filter-popup",
    ) as HTMLElement;

    // Initialize shortcuts modal
    this.initializeShortcutsModal();

    // Setup
    this.renderFilterTable();
    this.attachEventListeners();
    this.updatePlot();
  }

  /**
   * Initialize the shortcuts modal with plugin and host shortcuts
   */
  private initializeShortcutsModal(): void {
    const pluginShortcuts: ShortcutItem[] = [
      { key: "1-9", description: "Select EQ filter 1-9" },
      { key: "TAB", description: "Cycle through EQ filters" },
      { key: "ESC", description: "Clear selection" },
      { key: "+ or =", description: "Add new EQ filter" },
      { key: "-", description: "Remove selected EQ filter" },
      { key: "?", description: "Show shortcuts list" },
    ];

    const hostShortcuts: ShortcutItem[] = [
      { key: "< or ,", description: "Monitor input" },
      { key: "> or .", description: "Monitor output" },
      { key: "+, =, or ↑", description: "Increase volume (+5%)" },
      { key: "- or ↓", description: "Decrease volume (-5%)" },
    ];

    this.shortcutsModal = new ShortcutsModal({
      pluginShortcuts,
      hostShortcuts,
      pluginName: "EQ Plugin",
    });

    // Append modal to container
    const modalElement = this.shortcutsModal.createModal();
    this.container?.appendChild(modalElement);
  }

  /**
   * Update plot (debounced)
   */
  private requestPlotUpdate(): void {
    if (this.plotUpdateDebounceTimer) {
      clearTimeout(this.plotUpdateDebounceTimer);
    }

    this.plotUpdateDebounceTimer = window.setTimeout(() => {
      this.updatePlot();
      this.plotUpdateDebounceTimer = null;
    }, 150);
  }

  /**
   * Update the Plotly plot using browser-based computation
   */
  private updatePlot(): void {
    if (!this.plotContainer) return;

    try {
      // Generate plot using browser-based biquad filters
      generateEQPlot(this.plotContainer, this.filters, {
        sampleRate: 48000,
        showIndividualFilters: true,
        height: 400,
      });

      // Add click handler for filter selection
      addPlotClickHandler(this.plotContainer, this.filters, (filterIndex) => {
        this.selectedFilterIndex = filterIndex;
        this.renderFilterTable();
        console.log(
          "[EQPlugin] Selected filter:",
          filterIndex,
          this.filters[filterIndex],
        );
      });

      const enabledCount = this.filters.filter((f) => f.enabled).length;
      console.log("[EQPlugin] Plot updated with", enabledCount, "filters");
    } catch (error) {
      console.error("[EQPlugin] Failed to update plot:", error);
    }
  }

  /**
   * Get SVG icon for filter type
   */
  private getFilterSVGIcon(filterType: string): string {
    const svgBase = `<svg width="48" height="48" viewBox="0 0 48 48" xmlns="http://www.w3.org/2000/svg">`;
    const svgEnd = `</svg>`;

    switch (filterType) {
      case "Peak":
        return `${svgBase}<path d="M 8,36 L 18,36 L 24,12 L 30,36 L 40,36" stroke="#00bfff" stroke-width="2" fill="none"/>${svgEnd}`;
      case "Lowshelf":
        return `${svgBase}<path d="M 8,32 L 20,32 L 26,16 L 40,16" stroke="#00bfff" stroke-width="2" fill="none"/>${svgEnd}`;
      case "Highshelf":
        return `${svgBase}<path d="M 8,16 L 22,16 L 28,32 L 40,32" stroke="#00bfff" stroke-width="2" fill="none"/>${svgEnd}`;
      case "Lowpass":
        return `${svgBase}<path d="M 8,16 L 28,16 L 34,32 L 40,36" stroke="#00bfff" stroke-width="2" fill="none"/>${svgEnd}`;
      case "Highpass":
        return `${svgBase}<path d="M 8,36 L 14,32 L 20,16 L 40,16" stroke="#00bfff" stroke-width="2" fill="none"/>${svgEnd}`;
      case "Bandpass":
        return `${svgBase}<path d="M 8,36 L 14,28 L 24,12 L 34,28 L 40,36" stroke="#00bfff" stroke-width="2" fill="none"/>${svgEnd}`;
      case "Notch":
        return `${svgBase}<path d="M 8,16 L 18,16 L 24,36 L 30,16 L 40,16" stroke="#00bfff" stroke-width="2" fill="none"/>${svgEnd}`;
      default:
        return `${svgBase}<circle cx="24" cy="24" r="8" stroke="#00bfff" stroke-width="2" fill="none"/>${svgEnd}`;
    }
  }

  /**
   * Render filter table (horizontal layout)
   */
  private renderFilterTable(): void {
    if (!this.filterTable) return;

    // Build header row with EQ numbers, on/off toggle, and remove buttons
    const headerCells = this.filters
      .map((filter, index) => {
        const isSelected = index === this.selectedFilterIndex;
        const isEnabled = filter.enabled;
        return `
        <th class="has-background-grey-dark has-text-light ${isSelected ? "selected" : ""}" data-filter-index="${index}">
          <p class="buttons eq-header-cell">
            <button class="button is-dark is-small" data-index="${index}" aria-label="Select">
    	      <span>#${index + 1}</span>
	    </button>
            <button class="button ${isEnabled ? "is-success" : "is-dark"} is-small filter-toggle" data-index="${index}" aria-label="Toggle On/Off">
	      <span class="icon is-small">${isEnabled ? "✓" : "○"}</span>
	    </button>
            <button class="button is-warning is-small filter-remove" data-index="${index}" aria-label="Remove">
	      <span class="icon is-small">X</span>
	    </button>
          </p>
        </th>
      `;
      })
      .join("");

    // Build type row with icons in select
    const typeRow = this.filters
      .map((filter, index) => {
        const filterTypeConfig =
          FILTER_TYPES[filter.filter_type as keyof typeof FILTER_TYPES];
        const rowBg = "has-background-black-ter";
        return `
        <td class="${rowBg} ${index === this.selectedFilterIndex ? "selected" : ""}" data-filter-index="${index}">
          <div class="eq-type-cell">
            <div class="select is-small is-dark">
              <select class="filter-type-select-compact has-background-dark has-text-light" data-index="${index}" title="${filter.filter_type}">
                ${Object.entries(FILTER_TYPES)
                  .map(
                    ([type, config]) =>
                      `<option value="${type}" ${filter.filter_type === type ? "selected" : ""}>${config.icon} ${config.shortName}</option>`,
                  )
                  .join("")}
              </select>
            </div>
          </div>
        </td>
      `;
      })
      .join("");

    // Build enabled row with Bulma-styled checkboxes
    const enabledRow = this.filters
      .map(
        (filter, index) => `
      <td class="${index === this.selectedFilterIndex ? "selected" : ""}" data-filter-index="${index}">
        <label class="checkbox">
          <input type="checkbox" class="filter-enabled" data-index="${index}" ${filter.enabled ? "checked" : ""} />
        </label>
      </td>
    `,
      )
      .join("");

    // Build frequency row
    const freqRow = this.filters
      .map(
        (filter, index) => `
      <td class="has-background-black-bis ${index === this.selectedFilterIndex ? "selected" : ""}" data-filter-index="${index}">
        <input type="number" class="input is-small has-background-dark has-text-light filter-frequency" data-index="${index}" value="${Math.round(filter.frequency)}" min="20" max="20000" step="1" />
      </td>
    `,
      )
      .join("");

    // Build gain row
    const gainRow = this.filters
      .map(
        (filter, index) => `
      <td class="has-background-black-ter ${index === this.selectedFilterIndex ? "selected" : ""}" data-filter-index="${index}">
        <input type="number" class="input is-small has-background-dark has-text-light filter-gain" data-index="${index}" value="${filter.gain.toFixed(1)}" step="0.1" />
      </td>
    `,
      )
      .join("");

    // Build Q row
    const qRow = this.filters
      .map(
        (filter, index) => `
      <td class="has-background-black-bis ${index === this.selectedFilterIndex ? "selected" : ""}" data-filter-index="${index}">
        <input type="number" class="input is-small has-background-dark has-text-light filter-q" data-index="${index}" value="${filter.q.toFixed(1)}" min="0.1" max="3.0" step="0.1" />
      </td>
    `,
      )
      .join("");

    // Add button column
    const addButtonCell = `<th><button class="button is-primary is-small filter-add-btn-compact" title="Add Filter">+</button></th>`;

    this.filterTable.innerHTML = `
      <thead class="has-background-grey-dark">
        <tr>
          <th class="eq-param-label has-text-right has-text-light">EQ #</th>
          ${headerCells}
          ${addButtonCell}
        </tr>
      </thead>
      <tbody>
        <tr class="has-background-black-ter">
          <td class="eq-param-label has-text-right has-text-weight-semibold has-text-light">Type</td>
          ${typeRow}
          <td class="has-background-black-ter"></td>
        </tr>
        <tr class="has-background-black-bis">
          <td class="eq-param-label has-text-right has-text-weight-semibold has-text-light">Freq</td>
          ${freqRow}
          <td class="has-background-black-bis"></td>
        </tr>
        <tr class="has-background-black-ter">
          <td class="eq-param-label has-text-right has-text-weight-semibold has-text-light">Gain</td>
          ${gainRow}
          <td class="has-background-black-ter"></td>
        </tr>
        <tr class="has-background-black-bis">
          <td class="eq-param-label has-text-right has-text-weight-semibold has-text-light">Q</td>
          ${qRow}
          <td class="has-background-black-bis"></td>
        </tr>
      </tbody>
    `;

    this.attachTableEventListeners();
  }

  /**
   * Attach event listeners
   */
  private attachEventListeners(): void {
    // Popup close
    const closeBtn = this.filterPopup?.querySelector(
      ".popup-close",
    ) as HTMLButtonElement;
    if (closeBtn) {
      closeBtn.addEventListener("click", () => this.hideFilterPopup());
    }

    // Keyboard shortcuts
    document.addEventListener("keydown", this.handleKeydown);
  }

  /**
   * Handle keyboard shortcuts
   */
  private handleKeydown = (e: KeyboardEvent): void => {
    const target = e.target as HTMLElement;

    // Check if shortcuts modal is open - if so, let it handle keys
    const isModalOpen = this.shortcutsModal?.isVisible();
    if (isModalOpen) {
      return;
    }

    // Don't handle other shortcuts if user is typing in an input field
    if (
      target.tagName === "INPUT" ||
      target.tagName === "TEXTAREA" ||
      target.tagName === "SELECT"
    ) {
      return;
    }

    // Number keys 1-9 to select EQ filters
    if (e.key >= "1" && e.key <= "9") {
      const filterIndex = parseInt(e.key) - 1;
      if (filterIndex < this.filters.length) {
        e.preventDefault();
        this.selectFilter(filterIndex);
      }
      return;
    }

    // TAB to cycle through EQ filters
    if (e.key === "Tab") {
      e.preventDefault();
      this.cycleFilter();
      return;
    }

    // ESC to clear selection
    if (e.key === "Escape") {
      e.preventDefault();
      this.clearSelection();
      return;
    }

    // + to add new EQ
    if (e.key === "+" || e.key === "=") {
      e.preventDefault();
      this.addDefaultFilter();
      return;
    }

    // - to remove highlighted EQ
    if (e.key === "-" || e.key === "_") {
      if (this.selectedFilterIndex >= 0) {
        e.preventDefault();
        this.removeFilter(this.selectedFilterIndex);
      }
      return;
    }

    // ? to show shortcuts modal
    if (e.key === "?" || (e.shiftKey && e.key === "/")) {
      e.preventDefault();
      this.showShortcutsModal();
      return;
    }
  };

  /**
   * Select a filter (highlight in graph and table)
   */
  private selectFilter(index: number): void {
    this.selectedFilterIndex = index;
    this.renderFilterTable();
    this.highlightFilterInPlot(index);
    console.log("[EQPlugin] Selected filter:", index, this.filters[index]);
  }

  /**
   * Clear filter selection
   */
  private clearSelection(): void {
    this.selectedFilterIndex = -1;
    this.renderFilterTable();
    this.clearPlotHighlight();
    console.log("[EQPlugin] Cleared filter selection");
  }

  /**
   * Cycle to next filter (wraps around)
   */
  private cycleFilter(): void {
    if (this.filters.length === 0) return;

    // If nothing selected, select first filter
    if (this.selectedFilterIndex < 0) {
      this.selectFilter(0);
      return;
    }

    // Move to next filter, wrap around to 0 if at end
    const nextIndex = (this.selectedFilterIndex + 1) % this.filters.length;
    this.selectFilter(nextIndex);
  }

  /**
   * Highlight a specific filter in the plot
   */
  private highlightFilterInPlot(index: number): void {
    if (!this.plotContainer || !this.filters[index].enabled) return;

    try {
      // First, reset all traces to default
      const enabledCount = this.filters.filter((f) => f.enabled).length;
      if (enabledCount === 0) return;

      const resetUpdate: any = {
        "line.width": 2,
        opacity: 0.5,
      };
      const allTraceIndices = Array.from({ length: enabledCount }, (_, i) => i);
      Plotly.restyle(this.plotContainer, resetUpdate, allTraceIndices);

      // Count enabled filters before this one to get the trace index
      let traceIndex = 0;
      for (let i = 0; i < index; i++) {
        if (this.filters[i].enabled) {
          traceIndex++;
        }
      }

      // Update the selected trace to be highlighted
      const highlightUpdate: any = {
        "line.width": 4,
        opacity: 1.0,
      };
      Plotly.restyle(this.plotContainer, highlightUpdate, [traceIndex]);
    } catch (error) {
      console.error("[EQPlugin] Failed to highlight filter in plot:", error);
    }
  }

  /**
   * Clear plot highlight
   */
  private clearPlotHighlight(): void {
    if (!this.plotContainer) return;

    try {
      // Reset all individual filter traces to default style
      const enabledCount = this.filters.filter((f) => f.enabled).length;
      if (enabledCount === 0) return;

      const update: any = {
        "line.width": 2,
        opacity: 0.5,
      };

      // Apply to all individual filter traces (indices 0 to enabledCount-1)
      const traceIndices = Array.from({ length: enabledCount }, (_, i) => i);
      Plotly.restyle(this.plotContainer, update, traceIndices);
    } catch (error) {
      console.error("[EQPlugin] Failed to clear plot highlight:", error);
    }
  }

  /**
   * Attach table event listeners
   */
  private attachTableEventListeners(): void {
    if (!this.filterTable) return;

    // Type change (compact selector with icons)
    this.filterTable
      .querySelectorAll(".filter-type-select-compact")
      .forEach((select) => {
        select.addEventListener("change", (e) => {
          const index = parseInt((e.target as HTMLElement).dataset.index!, 10);
          const newType = (e.target as HTMLSelectElement).value;
          this.filters[index].filter_type = newType;
          this.onFilterChange();
        });
      });

    // Filter selection (# button in header)
    this.filterTable.querySelectorAll("th .button.is-info").forEach((btn) => {
      btn.addEventListener("click", (e) => {
        const index = parseInt((e.target as HTMLElement).dataset.index!, 10);
        this.selectedFilterIndex = index;
        this.renderFilterTable();
        this.highlightFilterInPlot(index);
      });
    });

    // Enabled toggle (button in header)
    this.filterTable.querySelectorAll(".filter-toggle").forEach((btn) => {
      btn.addEventListener("click", (e) => {
        const index = parseInt((e.target as HTMLElement).dataset.index!, 10);
        this.filters[index].enabled = !this.filters[index].enabled;
        this.renderFilterTable();
        this.onFilterChange();
      });
    });

    // Parameter inputs
    ["frequency", "gain", "q"].forEach((param) => {
      this.filterTable
        ?.querySelectorAll(`.filter-${param}`)
        .forEach((input) => {
          input.addEventListener("input", (e) => {
            const index = parseInt(
              (e.target as HTMLElement).dataset.index!,
              10,
            );
            const value = parseFloat((e.target as HTMLInputElement).value);
            (this.filters[index] as any)[param] = value;
            this.selectedFilterIndex = index;
            this.onFilterChange();
          });
        });
    });

    // Remove button (in header)
    this.filterTable.querySelectorAll(".filter-remove").forEach((btn) => {
      btn.addEventListener("click", (e) => {
        const index = parseInt((e.target as HTMLElement).dataset.index!, 10);
        this.removeFilter(index);
      });
    });

    // Add button (compact version)
    const addBtn = this.filterTable.querySelector(
      ".filter-add-btn-compact",
    ) as HTMLButtonElement;
    if (addBtn) {
      addBtn.addEventListener("click", () => this.addDefaultFilter());
    }
  }

  /**
   * Add a default filter (internal use for UI button)
   */
  private addDefaultFilter(): void {
    this.filters.push({
      filter_type: "Peak",
      frequency: 1000,
      q: 1.0,
      gain: 0,
      enabled: true,
    });
    this.renderFilterTable();
    this.onFilterChange();
  }

  /**
   * Remove a filter
   */
  private removeFilter(index: number): void {
    if (this.filters.length <= 1) {
      console.warn("[EQPlugin] Cannot remove last filter");
      return;
    }
    this.filters.splice(index, 1);
    if (this.selectedFilterIndex >= this.filters.length) {
      this.selectedFilterIndex = this.filters.length - 1;
    }
    this.renderFilterTable();
    this.onFilterChange();
  }

  /**
   * Handle filter change
   */
  private onFilterChange(): void {
    this.requestPlotUpdate();
    this.updateParameter("filters", this.filters);
  }

  /**
   * Show filter popup
   */
  private showFilterPopup(index: number): void {
    if (!this.filterPopup) return;

    const filter = this.filters[index];
    const filterType =
      FILTER_TYPES[filter.filter_type as keyof typeof FILTER_TYPES];

    const title = this.filterPopup.querySelector(".popup-title") as HTMLElement;
    const body = this.filterPopup.querySelector(".popup-body") as HTMLElement;

    title.textContent = `${filterType.shortName} | ${filterType.icon}`;

    body.innerHTML = `
      <div class="popup-field">
        <label>Frequency:</label>
        <input type="number" id="popup-freq" value="${filter.frequency}" min="20" max="20000" step="1" />
      </div>
      <div class="popup-field">
        <label>Gain:</label>
        <input type="number" id="popup-gain" value="${filter.gain}" step="0.1" />
      </div>
      <div class="popup-field">
        <label>Q:</label>
        <input type="number" id="popup-q" value="${filter.q}" min="0.1" max="3.0" step="0.1" />
      </div>
    `;

    this.filterPopup.style.display = "block";

    // Attach input listeners
    ["freq", "gain", "q"].forEach((param) => {
      const input = body.querySelector(`#popup-${param}`) as HTMLInputElement;
      if (input) {
        input.addEventListener("input", (e) => {
          const value = parseFloat((e.target as HTMLInputElement).value);
          const key = param === "freq" ? "frequency" : param;
          (filter as any)[key] = value;
          this.onFilterChange();
          this.renderFilterTable();
        });
      }
    });
  }

  /**
   * Hide filter popup
   */
  private hideFilterPopup(): void {
    if (this.filterPopup) {
      this.filterPopup.style.display = "none";
    }
  }

  /**
   * Show shortcuts modal
   */
  private showShortcutsModal(): void {
    this.shortcutsModal?.show();
  }

  /**
   * Resize handler
   */
  resize(): void {
    if (this.plotContainer) {
      // Regenerate plot on resize for better responsiveness
      this.updatePlot();
    }
  }

  /**
   * Get keyboard shortcuts for this plugin
   */
  getShortcuts() {
    return [
      { key: "1-9", description: "Select filter" },
      { key: "Tab", description: "Cycle filters" },
      { key: "+", description: "Add filter" },
      { key: "-", description: "Remove filter" },
      { key: "Esc", description: "Clear selection" },
    ];
  }

  /**
   * Destroy the plugin
   */
  destroy(): void {
    // Remove keyboard listener
    document.removeEventListener("keydown", this.handleKeydown);

    if (this.plotUpdateDebounceTimer) {
      clearTimeout(this.plotUpdateDebounceTimer);
    }

    if (this.plotContainer) {
      // Remove Plotly plot
      try {
        Plotly.purge(this.plotContainer);
      } catch (e) {
        // Ignore if Plotly not loaded
      }
    }

    if (this.menubar) {
      this.menubar.destroy();
      this.menubar = null;
    }

    if (this.shortcutsModal) {
      this.shortcutsModal.destroy();
      this.shortcutsModal = null;
    }

    super.destroy();
  }

  /**
   * Get filters
   */
  getFilters(): FilterParam[] {
    return [...this.filters];
  }

  /**
   * Set filters
   */
  setFilters(filters: FilterParam[]): void {
    this.filters = filters;
    this.renderFilterTable();
    this.onFilterChange();
  }

  /**
   * Add a filter programmatically (public API)
   */
  addFilter(params?: {
    type?: string;
    frequency?: number;
    q?: number;
    gain?: number;
    enabled?: boolean;
  }): void {
    const filter: FilterParam = {
      filter_type: params?.type
        ? this.normalizeFilterType(params.type)
        : "Peak",
      frequency: params?.frequency ?? 1000,
      q: params?.q ?? 1.0,
      gain: params?.gain ?? 0,
      enabled: params?.enabled ?? true,
    };

    this.filters.push(filter);
    this.renderFilterTable();
    this.onFilterChange();
  }

  /**
   * Normalize filter type string (e.g., "peak" -> "Peak")
   */
  private normalizeFilterType(type: string): string {
    const normalized =
      type.charAt(0).toUpperCase() + type.slice(1).toLowerCase();
    // Verify it's a valid type
    if (normalized in FILTER_TYPES) {
      return normalized;
    }
    console.warn(
      `[EQPlugin] Unknown filter type "${type}", defaulting to Peak`,
    );
    return "Peak";
  }
}
