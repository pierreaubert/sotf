// Visual EQ Configuration Module
// Extracted from audio-player.ts to handle all EQ table and graph functionality

import { invoke } from "@tauri-apps/api/core";
import { StreamingManager } from "./audio-manager";

export interface FilterParam {
  frequency: number;
  q: number;
  gain: number;
  enabled: boolean;
}

export interface ExtendedFilterParam extends FilterParam {
  filter_type: string; // "Peak", "Lowpass", "Highpass", "Bandpass", "Notch", "Lowshelf", "Highshelf"
}

// Filter type options
export const FILTER_TYPES = {
  Peak: { label: "Peak", shortName: "PK", icon: "○" },
  Lowpass: { label: "Low Pass", shortName: "LP", icon: "╲" },
  Highpass: { label: "High Pass", shortName: "HP", icon: "╱" },
  Bandpass: { label: "Band Pass", shortName: "BP", icon: "∩" },
  Notch: { label: "Notch", shortName: "NO", icon: "V" },
  Lowshelf: { label: "Low Shelf", shortName: "LS", icon: "⎣" },
  Highshelf: { label: "High Shelf", shortName: "HS", icon: "⎤" },
};

export interface VisualEQConfigCallbacks {
  onFilterParamsChange?: (filterParams: ExtendedFilterParam[]) => void;
  onEQToggle?: (enabled: boolean) => void;
  onAutoGainChange?: (enabled: boolean) => void;
  onLoudnessCompensationChange?: (enabled: boolean) => void;
  onSplAmplitudeChange?: (amplitude: number) => void;
  getAutoGain?: () => boolean;
  getLoudnessCompensation?: () => boolean;
  getSplAmplitude?: () => number;
}

export class VisualEQConfig {
  private container: HTMLElement;
  private instanceId: string;
  private callbacks: VisualEQConfigCallbacks;
  private streamingManager: StreamingManager;

  // EQ Modal and UI elements
  private eqModal: HTMLElement | null = null;
  private eqBackdrop: HTMLElement | null = null;
  private eqModalCloseBtn: HTMLButtonElement | null = null;
  private eqTableContainer: HTMLElement | null = null;
  private playbackOptionsContainer: HTMLElement | null = null;
  private eqConfigBtn: HTMLElement | null = null;

  // EQ Graph properties (modal)
  private eqGraphCanvas: HTMLCanvasElement | null = null;
  private eqGraphCtx: CanvasRenderingContext2D | null = null;

  // Mini EQ Graph (in main UI)
  private eqMiniCanvas: HTMLCanvasElement | null = null;
  private eqMiniCtx: CanvasRenderingContext2D | null = null;
  private selectedFilterIndex: number = -1;
  private isDraggingHandle: boolean = false;
  private dragMode: "ring" | "bar" = "ring";
  private lastMouseY: number = 0;

  // EQ Response data
  private eqResponseData: any = null; // Cached response from backend
  private eqResponseDebounceTimer: number | null = null;

  // EQ Graph constants
  private readonly EQ_GRAPH_MIN_FREQ = 20;
  private readonly EQ_GRAPH_MAX_FREQ = 20000;
  private readonly EQ_GRAPH_MIN_Q = 0.1;
  private readonly EQ_GRAPH_MAX_Q = 3.0;
  private readonly EQ_GRAPH_FREQ_POINTS = 256; // Number of points for response curve

  // EQ Graph dynamic gain range (computed from response data)
  private eqGraphMinGain = -18; // Default: -6 * max_db (3.0)
  private eqGraphMaxGain = 3; // Default: max_db

  // EQ state
  private currentFilterParams: ExtendedFilterParam[] = [
    { frequency: 100, q: 1.0, gain: 0, enabled: true, filter_type: "Peak" },
    { frequency: 1000, q: 1.0, gain: 0, enabled: true, filter_type: "Peak" },
    { frequency: 10000, q: 1.0, gain: 0, enabled: true, filter_type: "Peak" },
  ];
  private eqEnabled: boolean = true;

  constructor(
    container: HTMLElement,
    instanceId: string,
    streamingManager: StreamingManager,
    callbacks: VisualEQConfigCallbacks = {},
    eqMiniCanvas: HTMLCanvasElement | null = null,
  ) {
    this.container = container;
    this.instanceId = instanceId;
    this.streamingManager = streamingManager;
    this.callbacks = callbacks;

    // Initialize mini canvas
    this.eqMiniCanvas = eqMiniCanvas;
    if (this.eqMiniCanvas) {
      this.eqMiniCtx = this.eqMiniCanvas.getContext("2d");
    }

    this.setupInlineContainers();
    this.setupEventListeners();

    // Render content immediately
    this.renderPlaybackOptions();
    this.renderEQTable();

    // Compute initial EQ response to populate graphs
    // Use setTimeout to ensure canvas is fully laid out
    setTimeout(() => {
      this.computeEQResponse();
    }, 0);
  }

  // ===== INLINE CONTAINERS SETUP =====

  private setupInlineContainers(): void {
    console.log("[EQ Debug] Setting up inline containers");

    // Find the inline containers in the audio player
    this.playbackOptionsContainer = this.container.querySelector(
      ".playback-options-container",
    );
    this.eqTableContainer = this.container.querySelector(".eq-table-container");

    console.log(
      "[EQ Debug] Playback options container:",
      this.playbackOptionsContainer,
    );
    console.log("[EQ Debug] EQ table container:", this.eqTableContainer);
  }

  private setupEventListeners(): void {
    // No modal-specific event listeners needed for inline display
    // Table and playback option events are set up when those elements are rendered
  }

  // Modal methods removed - now using inline display

  // ===== PLAYBACK OPTIONS RENDERING =====

  private renderPlaybackOptions(): void {
    console.log("[EQ Debug] Rendering playback options");

    if (!this.playbackOptionsContainer) {
      console.error("[EQ Debug] Playback options container not found");
      return;
    }

    // Clear existing content
    this.playbackOptionsContainer.innerHTML = "";

    // Get current values
    const autoGain = this.callbacks.getAutoGain?.() ?? true;
    const loudnessComp = this.callbacks.getLoudnessCompensation?.() ?? false;
    const splAmplitude = this.callbacks.getSplAmplitude?.() ?? -20;

    this.playbackOptionsContainer.innerHTML = `
      <div class="option-row">
        <label class="option-label">
          <input type="checkbox" class="auto-gain-toggle" ${autoGain ? "checked" : ""}>
          Auto Gain
        </label>
        <span class="option-help">Automatically adjust volume to prevent clipping</span>
      </div>
      <div class="option-row">
        <label class="option-label">
          <input type="checkbox" class="loudness-compensation-toggle" ${loudnessComp ? "checked" : ""}>
          Loudness Compensation
        </label>
        <span class="option-help">Apply equal-loudness curve adjustment</span>
      </div>
      <div class="option-row spl-amplitude-row" style="display: ${loudnessComp ? "flex" : "none"}; padding-left: 24px;">
        <label class="option-label" style="flex-direction: column; align-items: flex-start; gap: 4px;">
          <span>SPL Amplitude: <span class="spl-value">${splAmplitude}</span> dB</span>
          <div style="display: flex; align-items: center; gap: 8px; width: 100%;">
            <span style="font-size: 0.85em; color: var(--text-secondary);">-30</span>
            <input type="range" class="spl-amplitude-slider"
                   min="-30" max="0" step="1" value="${splAmplitude}"
                   style="flex: 1;">
            <span style="font-size: 0.85em; color: var(--text-secondary);">0</span>
          </div>
        </label>
        <span class="option-help">Reference SPL for loudness compensation curve</span>
      </div>
    `;

    // Setup event listeners for playback options
    const autoGainToggle = this.playbackOptionsContainer.querySelector(
      ".auto-gain-toggle",
    ) as HTMLInputElement;
    const loudnessToggle = this.playbackOptionsContainer.querySelector(
      ".loudness-compensation-toggle",
    ) as HTMLInputElement;
    const splSlider = this.playbackOptionsContainer.querySelector(
      ".spl-amplitude-slider",
    ) as HTMLInputElement;
    const splValue = this.playbackOptionsContainer.querySelector(
      ".spl-value",
    ) as HTMLSpanElement;
    const splRow = this.playbackOptionsContainer.querySelector(
      ".spl-amplitude-row",
    ) as HTMLDivElement;

    autoGainToggle?.addEventListener("change", () => {
      this.callbacks.onAutoGainChange?.(autoGainToggle.checked);
    });

    loudnessToggle?.addEventListener("change", () => {
      this.callbacks.onLoudnessCompensationChange?.(loudnessToggle.checked);
      // Show/hide SPL slider
      if (splRow) {
        splRow.style.display = loudnessToggle.checked ? "flex" : "none";
      }
    });

    splSlider?.addEventListener("input", () => {
      const value = parseFloat(splSlider.value);
      if (splValue) {
        splValue.textContent = value.toString();
      }
      this.callbacks.onSplAmplitudeChange?.(value);
    });
  }

  // ===== EQ TABLE RENDERING =====

  private renderEQTable(): void {
    console.log("[EQ Debug] Rendering EQ table");

    if (!this.eqTableContainer) {
      console.error("[EQ Debug] EQ table container not found");
      return;
    }

    // Clear existing content
    this.eqTableContainer.innerHTML = "";

    // Render EQ table section
    const eqSection = document.createElement("div");
    eqSection.className = "eq-section";

    // Create header
    const header = document.createElement("div");
    header.className = "eq-section-header";
    header.innerHTML = "<h4>Filter Configuration</h4>";

    // Create graph container
    const graphContainer = document.createElement("div");
    graphContainer.className = "eq-graph-container";

    // Create canvas for EQ graph
    const canvas = document.createElement("canvas");
    canvas.className = "eq-graph-canvas";
    canvas.width = 600;
    canvas.height = 300;

    graphContainer.appendChild(canvas);

    // Create table
    const table = document.createElement("table");
    table.className = "eq-table-vertical";

    // Create table header
    const thead = document.createElement("thead");
    thead.innerHTML = `
      <tr class="eq-row">
        <th class="eq-row-label"></th>
        ${this.currentFilterParams
          .map(
            (_, index) => `
            <th data-filter-index="${index}" class="eq-column-header ${index === this.selectedFilterIndex ? "selected" : ""}">
              <div style="display: flex; align-items: center; justify-content: space-between; gap: 4px;">
                <span>Filter ${index + 1}</span>
                <button class="filter-remove-btn" data-index="${index}" title="Remove filter" style="background: none; border: none; color: var(--text-secondary); cursor: pointer; font-size: 16px; padding: 0 4px; line-height: 1;">&times;</button>
              </div>
            </th>
          `,
          )
          .join("")}
        <th class="eq-row-label">
          <button class="filter-add-btn" title="Add filter" style="background: var(--button-primary); border: none; color: white; cursor: pointer; font-size: 18px; padding: 4px 8px; border-radius: 4px; line-height: 1;">+</button>
        </th>
      </tr>
    `;
    table.appendChild(thead);

    // Create table body
    const tbody = document.createElement("tbody");

    // Filter type row
    const typeRow = document.createElement("tr");
    typeRow.className = "eq-row";
    typeRow.innerHTML = `
      <td class="eq-row-label">Type</td>
      ${this.currentFilterParams
        .map(
          (filter, index) => `
          <td data-filter-index="${index}" class="${index === this.selectedFilterIndex ? "selected" : ""}">
            <select data-index="${index}" class="eq-filter-type">
              ${Object.entries(FILTER_TYPES)
                .map(
                  ([type, config]) =>
                    `<option value="${type}" ${filter.filter_type === type ? "selected" : ""}>${config.label}</option>`,
                )
                .join("")}
            </select>
          </td>
        `,
        )
        .join("")}
      <td class="eq-row-label"></td>
    `;
    tbody.appendChild(typeRow);

    // Enable row
    const enableRow = document.createElement("tr");
    enableRow.className = "eq-row";
    enableRow.innerHTML = `
      <td class="eq-row-label">Enable</td>
      ${this.currentFilterParams
        .map(
          (filter, index) => `
          <td data-filter-index="${index}" class="${index === this.selectedFilterIndex ? "selected" : ""}">
            <input type="checkbox" data-index="${index}" class="eq-enabled" ${filter.enabled ? "checked" : ""}>
          </td>
        `,
        )
        .join("")}
      <td class="eq-row-label"></td>
    `;
    tbody.appendChild(enableRow);

    // Frequency row
    const freqRow = document.createElement("tr");
    freqRow.className = "eq-row";
    freqRow.innerHTML = `
      <td class="eq-row-label">Freq (Hz)</td>
      ${this.currentFilterParams
        .map(
          (filter, index) => `
          <td data-filter-index="${index}" class="${index === this.selectedFilterIndex ? "selected" : ""}">
            <input type="number" data-index="${index}" class="eq-frequency" value="${filter.frequency.toFixed(1)}" step="1" min="20" max="20000">
          </td>
        `,
        )
        .join("")}
      <td class="eq-row-label"></td>
    `;
    tbody.appendChild(freqRow);

    // Gain row
    const gainRow = document.createElement("tr");
    gainRow.className = "eq-row";
    gainRow.innerHTML = `
      <td class="eq-row-label">Gain (dB)</td>
      ${this.currentFilterParams
        .map(
          (filter, index) => `
          <td data-filter-index="${index}" class="${index === this.selectedFilterIndex ? "selected" : ""}">
            <input type="number" data-index="${index}" class="eq-gain" value="${filter.gain.toFixed(2)}" step="0.1">
          </td>
        `,
        )
        .join("")}
      <td class="eq-row-label"></td>
    `;
    tbody.appendChild(gainRow);

    // Q row
    const qRow = document.createElement("tr");
    qRow.className = "eq-row";
    qRow.innerHTML = `
      <td class="eq-row-label">Q</td>
      ${this.currentFilterParams
        .map(
          (filter, index) => `
          <td data-filter-index="${index}" class="${index === this.selectedFilterIndex ? "selected" : ""}">
            <input type="number" data-index="${index}" class="eq-q" value="${filter.q.toFixed(2)}" step="0.1" min="0.1" max="3.0">
          </td>
        `,
        )
        .join("")}
      <td class="eq-row-label"></td>
    `;
    tbody.appendChild(qRow);

    table.appendChild(tbody);

    // Assemble the section
    this.eqTableContainer.innerHTML = "";
    eqSection.appendChild(header);
    eqSection.appendChild(graphContainer);
    eqSection.appendChild(table);
    this.eqTableContainer.appendChild(eqSection);

    // Cache canvas and setup graph
    this.eqGraphCanvas = canvas;
    this.eqGraphCtx = canvas.getContext("2d");
    this.resizeEQGraphCanvas();

    // Setup table interactions
    this.setupTableInteractions(table);

    // Initial graph draw
    this.computeEQResponse();
  }

  private setupTableInteractions(table: HTMLTableElement): void {
    // Handle column selection
    table.addEventListener("click", (e) => {
      const target = e.target as HTMLElement;

      // Handle remove filter button
      if (target.classList.contains("filter-remove-btn")) {
        e.stopPropagation();
        const index = parseInt(target.dataset.index!, 10);
        this.removeFilter(index);
        return;
      }

      // Handle add filter button
      if (target.classList.contains("filter-add-btn")) {
        e.stopPropagation();
        this.addFilter();
        return;
      }

      const cell = target.closest("td, th") as HTMLElement | null;
      if (cell && cell.dataset.filterIndex) {
        const index = parseInt(cell.dataset.filterIndex, 10);
        this.selectedFilterIndex = index;
        this.drawEQGraph();
        this.renderEQTable(); // Re-render to update selection
      }
    });

    // Handle input changes
    table.addEventListener("input", (e) => this.handleEQTableChange(e));
  }

  private handleEQTableChange(e: Event): void {
    const target = e.target as HTMLInputElement | HTMLSelectElement;
    const index = parseInt(target.dataset.index || "", 10);

    if (isNaN(index) || !this.currentFilterParams[index]) return;

    let type = target.className.replace("eq-", "");

    // Handle filter type select separately
    if (type === "filter-type") {
      type = "filter_type";
    }

    let value: any = target.value;

    // Convert numeric values
    if (type === "frequency" || type === "gain" || type === "q") {
      value = parseFloat(value);
      // Enforce bounds
      if (type === "frequency") {
        value = Math.max(20, Math.min(20000, value));
      } else if (type === "q") {
        value = Math.max(0.1, Math.min(3.0, value));
      }
    } else if (type === "enabled") {
      value = (target as HTMLInputElement).checked;
    }

    // Update the filter parameter
    (this.currentFilterParams[index] as unknown as Record<string, any>)[type] =
      value;

    // Select this filter in the graph
    this.selectedFilterIndex = index;

    // Apply filters to audio backend immediately
    this.setupEQFilters();

    // Request graph update (debounced for backend computation)
    this.requestEQResponseUpdate();

    // Redraw graphs immediately for responsive UI
    this.drawEQGraph();
    this.drawMiniEQ();

    // Notify callback of parameter change
    this.callbacks.onFilterParamsChange?.(this.currentFilterParams);

    // Note: We don't call renderEQTable() here because it would disrupt user input
    // The table values are already updated via the input event
  }

  // ===== FILTER PARAMETER MANAGEMENT =====

  private addFilter(): void {
    // Add a new filter with default values
    const newFilter: ExtendedFilterParam = {
      frequency: 1000,
      q: 1.0,
      gain: 0,
      enabled: true,
      filter_type: "Peak",
    };

    this.currentFilterParams.push(newFilter);
    this.selectedFilterIndex = this.currentFilterParams.length - 1;
    this.setupEQFilters();
    this.renderEQTable();
    this.requestEQResponseUpdate();

    // Notify callback
    this.callbacks.onFilterParamsChange?.(this.currentFilterParams);
  }

  private removeFilter(index: number): void {
    if (this.currentFilterParams.length <= 1) {
      console.warn("[VisualEQConfig] Cannot remove last filter");
      return;
    }

    this.currentFilterParams.splice(index, 1);

    // Adjust selected index if needed
    if (this.selectedFilterIndex >= this.currentFilterParams.length) {
      this.selectedFilterIndex = this.currentFilterParams.length - 1;
    }

    this.setupEQFilters();
    this.renderEQTable();
    this.requestEQResponseUpdate();

    // Notify callback
    this.callbacks.onFilterParamsChange?.(this.currentFilterParams);
  }

  updateFilterParams(filterParams: Partial<ExtendedFilterParam>[]): void {
    this.currentFilterParams = filterParams.map((p) => ({
      enabled: p.enabled ?? true,
      frequency: p.frequency || 0,
      q: p.q || 1,
      gain: p.gain || 0,
      filter_type: p.filter_type || "Peak",
    })) as ExtendedFilterParam[];

    // Recalculate and apply filters
    this.setupEQFilters();

    // Re-render the table to show updated values
    this.renderEQTable();

    // Update graphs (including mini EQ)
    this.requestEQResponseUpdate();

    // Notify callback
    this.callbacks.onFilterParamsChange?.(this.currentFilterParams);
  }

  clearEQFilters(): void {
    this.currentFilterParams = [];
    this.setupEQFilters();
    this.renderEQTable(); // Update table to show no filters
  }

  private setupEQFilters(): void {
    let activeFilterCount = 0;
    let maxPositiveGain = 0;

    // Create new filters from parameters
    this.currentFilterParams.forEach((param) => {
      if (param.enabled) {
        activeFilterCount++;
        if (param.gain > maxPositiveGain) {
          maxPositiveGain = param.gain;
        }
      }
    });

    console.log(
      `Created ${activeFilterCount} EQ filters with gain compensation`,
      maxPositiveGain,
    );

    // Notify streaming manager if playing
    if (this.eqEnabled) {
      const filters = this.currentFilterParams
        .filter((p) => p.enabled)
        .map((p) => ({
          frequency: p.frequency,
          q: p.q,
          gain: p.gain,
        }));

      this.streamingManager.updateFilters(filters).catch((error) => {
        console.error("Failed to update filters in real-time:", error);
      });
    }
  }

  setEQEnabled(enabled: boolean): void {
    // Skip filter update if already in the desired state to avoid duplicate calls
    const alreadyInDesiredState = this.eqEnabled === enabled;

    if (alreadyInDesiredState) {
      // Still notify callbacks even if state hasn't changed
      this.callbacks.onEQToggle?.(enabled);
      return;
    }

    this.eqEnabled = enabled;

    // Apply EQ changes in real-time if playing
    const filters = enabled
      ? this.currentFilterParams
          .filter((p) => p.enabled)
          .map((p) => ({
            frequency: p.frequency,
            q: p.q,
            gain: p.gain,
          }))
      : [];

    this.streamingManager.updateFilters(filters).catch((error: unknown) => {
      console.error("Failed to update filters:", error);
    });

    console.log(`EQ ${enabled ? "enabled" : "disabled"}`);
    this.callbacks.onEQToggle?.(enabled);
  }

  isEQEnabled(): boolean {
    return this.eqEnabled;
  }

  getFilterParams(): ExtendedFilterParam[] {
    return [...this.currentFilterParams];
  }

  // ===== EQ GRAPH IMPLEMENTATION =====

  private resizeEQGraphCanvas(): void {
    if (!this.eqGraphCanvas) return;
    const container = this.eqGraphCanvas.parentElement;
    if (!container) return;

    const rect = container.getBoundingClientRect();
    const width = Math.max(400, rect.width - 40);
    const height = 300;

    this.eqGraphCanvas.width = width;
    this.eqGraphCanvas.height = height;
    this.drawEQGraph();
  }

  private async computeEQResponse(): Promise<void> {
    if (!this.currentFilterParams || this.currentFilterParams.length === 0) {
      this.eqResponseData = null;
      this.drawEQGraph();
      this.drawMiniEQ();
      return;
    }

    const logMin = Math.log10(this.EQ_GRAPH_MIN_FREQ);
    const logMax = Math.log10(this.EQ_GRAPH_MAX_FREQ);
    const frequencies: number[] = [];
    for (let i = 0; i < this.EQ_GRAPH_FREQ_POINTS; i++) {
      const logFreq =
        logMin + (logMax - logMin) * (i / (this.EQ_GRAPH_FREQ_POINTS - 1));
      frequencies.push(Math.pow(10, logFreq));
    }

    const filters = this.currentFilterParams.map((f) => ({
      filter_type: f.filter_type || "Peak",
      frequency: f.frequency,
      q: f.q,
      gain: f.gain,
      enabled: f.enabled,
    }));

    console.log("[EQ Graph] Computing response with filters:", filters);

    try {
      const result = await invoke("compute_eq_response", {
        filters,
        sampleRate: 48000,
        frequencies,
      });

      console.log("[EQ Graph] Response data received:", result);
      this.eqResponseData = result;
      this.drawEQGraph();
      this.drawMiniEQ(); // Update mini EQ visualization
    } catch (error) {
      console.error("[EQ Graph] Failed to compute response:", error);
    }
  }

  private requestEQResponseUpdate(): void {
    if (this.eqResponseDebounceTimer) {
      clearTimeout(this.eqResponseDebounceTimer);
    }

    this.eqResponseDebounceTimer = window.setTimeout(() => {
      this.computeEQResponse();
      this.eqResponseDebounceTimer = null;
    }, 100);
  }

  private drawEQGraph(): void {
    if (!this.eqGraphCanvas || !this.eqGraphCtx) {
      console.log("[EQ Graph] Canvas or context not available");
      return;
    }

    const ctx = this.eqGraphCtx;
    const width = this.eqGraphCanvas.width;
    const height = this.eqGraphCanvas.height;

    // Clear canvas
    ctx.clearRect(0, 0, width, height);

    // Draw background
    ctx.fillStyle = "#1a1a1a";
    ctx.fillRect(0, 0, width, height);

    console.log(
      "[EQ Graph] Drawing graph - canvas:",
      width,
      "x",
      height,
      "hasData:",
      !!this.eqResponseData,
      "selectedFilter:",
      this.selectedFilterIndex,
    );

    if (this.eqResponseData) {
      console.log("[EQ Graph] Drawing response curves");
      this.drawGrid(ctx, width, height);
      this.drawFrequencyLabels(ctx, width, height);
      this.drawGainLabels(ctx, width, height);
      this.drawCombinedResponse(ctx, width, height);
      this.drawIndividualResponses(ctx, width, height);
    } else {
      console.log("[EQ Graph] No response data available");
    }

    this.drawFilterHandles(ctx, width, height, true); // isDarkMode = true
  }

  private computeDynamicYAxisRange(): void {
    let minGain = Infinity;
    let maxGain = -Infinity;

    // Include filter gain values in range calculation
    this.currentFilterParams.forEach((filter) => {
      if (filter.enabled) {
        minGain = Math.min(minGain, filter.gain);
        maxGain = Math.max(maxGain, filter.gain);
      }
    });

    // Include response data if available
    if (this.eqResponseData) {
      if (
        this.eqResponseData.combined_response &&
        Array.isArray(this.eqResponseData.combined_response)
      ) {
        this.eqResponseData.combined_response.forEach((gain: number) => {
          minGain = Math.min(minGain, gain);
          maxGain = Math.max(maxGain, gain);
        });
      }

      if (this.eqResponseData.individual_responses) {
        const responses = this.eqResponseData.individual_responses;
        if (Array.isArray(responses)) {
          responses.forEach((response: number[]) => {
            if (Array.isArray(response)) {
              response.forEach((gain: number) => {
                minGain = Math.min(minGain, gain);
                maxGain = Math.max(maxGain, gain);
              });
            }
          });
        } else if (typeof responses === "object") {
          Object.values(responses).forEach((response: any) => {
            if (Array.isArray(response)) {
              response.forEach((gain: number) => {
                minGain = Math.min(minGain, gain);
                maxGain = Math.max(maxGain, gain);
              });
            }
          });
        }
      }
    }

    // Default range if no data
    if (minGain === Infinity || maxGain === -Infinity) {
      minGain = -18;
      maxGain = 3;
    }

    // Add padding and ensure minimum range
    const padding = 1;
    const minRange = 6; // Minimum 6 dB range
    const range = maxGain - minGain;

    if (range < minRange) {
      const center = (minGain + maxGain) / 2;
      minGain = center - minRange / 2;
      maxGain = center + minRange / 2;
    }

    this.eqGraphMinGain = minGain - padding;
    this.eqGraphMaxGain = maxGain + padding;

    console.log(
      "[EQ Graph] Dynamic Y-axis range:",
      this.eqGraphMinGain.toFixed(1),
      "to",
      this.eqGraphMaxGain.toFixed(1),
      "dB",
    );
  }

  private drawGrid(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
  ): void {
    ctx.strokeStyle = "rgba(255, 255, 255, 0.1)";
    ctx.lineWidth = 1;

    // Vertical frequency lines
    const freqMarkers = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
    freqMarkers.forEach((freq) => {
      const x = this.freqToX(freq, width);
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, height - 30);
      ctx.stroke();
    });

    // Horizontal gain lines (match label logic)
    const gainRange = this.eqGraphMaxGain - this.eqGraphMinGain;
    const maxLabels = 7;
    const idealStep = gainRange / (maxLabels - 1);
    const niceSteps = [1, 2, 3, 5, 6, 10, 12, 15, 20, 30, 50, 60];
    let gainStep = niceSteps[0];
    for (const step of niceSteps) {
      if (step >= idealStep) {
        gainStep = step;
        break;
      }
    }
    if (idealStep > niceSteps[niceSteps.length - 1]) {
      gainStep = Math.ceil(idealStep / 10) * 10;
    }

    for (
      let gain = Math.ceil(this.eqGraphMinGain / gainStep) * gainStep;
      gain <= this.eqGraphMaxGain;
      gain += gainStep
    ) {
      const y = this.gainToY(gain, height);
      ctx.beginPath();
      ctx.moveTo(60, y);
      ctx.lineTo(width - 20, y);

      if (Math.abs(gain) < 0.01) {
        ctx.lineWidth = 2;
        ctx.strokeStyle = "rgba(255, 255, 255, 0.3)";
      } else {
        ctx.lineWidth = 1;
        ctx.strokeStyle = "rgba(255, 255, 255, 0.1)";
      }
      ctx.stroke();
    }
  }

  private drawFrequencyLabels(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
  ): void {
    ctx.fillStyle = "rgba(255, 255, 255, 0.7)";
    ctx.font = "11px sans-serif";
    ctx.textAlign = "center";
    ctx.textBaseline = "top";

    const freqMarkers = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
    freqMarkers.forEach((freq) => {
      const x = this.freqToX(freq, width);
      const label = freq >= 1000 ? `${freq / 1000}k` : `${freq}`;
      ctx.fillText(label, x, height - 25);
    });
  }

  private drawGainLabels(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
  ): void {
    ctx.fillStyle = "rgba(255, 255, 255, 0.7)";
    ctx.font = "11px sans-serif";
    ctx.textAlign = "right";
    ctx.textBaseline = "middle";

    const gainRange = this.eqGraphMaxGain - this.eqGraphMinGain;
    const maxLabels = 7;

    // Calculate appropriate step size to get max 7 labels
    const idealStep = gainRange / (maxLabels - 1);
    // Round to nice numbers: 1, 2, 3, 5, 6, 10, 12, 15, etc.
    const niceSteps = [1, 2, 3, 5, 6, 10, 12, 15, 20, 30, 50, 60];
    let gainStep = niceSteps[0];
    for (const step of niceSteps) {
      if (step >= idealStep) {
        gainStep = step;
        break;
      }
    }
    if (idealStep > niceSteps[niceSteps.length - 1]) {
      gainStep = Math.ceil(idealStep / 10) * 10;
    }

    // Generate labels
    const labels: number[] = [];
    for (
      let gain = Math.ceil(this.eqGraphMinGain / gainStep) * gainStep;
      gain <= this.eqGraphMaxGain && labels.length < maxLabels;
      gain += gainStep
    ) {
      labels.push(gain);
    }

    // Draw labels on the left
    labels.forEach((gain) => {
      const y = this.gainToY(gain, height);
      const label = `${gain > 0 ? "+" : ""}${gain.toFixed(0)}dB`;
      ctx.fillText(label, 55, y);
    });
  }

  private drawCombinedResponse(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
  ): void {
    if (!this.eqResponseData?.combined_response) return;
    const { frequencies, combined_response } = this.eqResponseData;

    this.computeDynamicYAxisRange();

    ctx.strokeStyle = "rgba(100, 200, 255, 0.8)";
    ctx.lineWidth = 2;
    ctx.beginPath();

    frequencies.forEach((freq: number, i: number) => {
      const x = this.freqToX(freq, width);
      const y = this.gainToY(combined_response[i], height);

      if (i === 0) {
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    });

    ctx.stroke();
  }

  private drawIndividualResponses(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
  ): void {
    if (!this.eqResponseData?.individual_responses) return;
    const { frequencies, individual_responses } = this.eqResponseData;

    const colors = [
      "rgba(255, 100, 100, 0.6)",
      "rgba(100, 255, 100, 0.6)",
      "rgba(255, 255, 100, 0.6)",
      "rgba(100, 100, 255, 0.6)",
      "rgba(255, 100, 255, 0.6)",
      "rgba(100, 255, 255, 0.6)",
    ];

    this.currentFilterParams.forEach((filter, filterIdx) => {
      if (!filter.enabled || Math.abs(filter.gain) < 0.1) return;

      const response = individual_responses[filterIdx];
      if (!response || !Array.isArray(response)) return;

      const isSelected = filterIdx === this.selectedFilterIndex;

      // Highlight selected filter with brighter, thicker line
      if (isSelected) {
        ctx.strokeStyle = "#00ff00"; // Bright green for selected
        ctx.lineWidth = 3.5;
      } else {
        ctx.strokeStyle = colors[filterIdx % colors.length];
        ctx.lineWidth = 1.5;
      }

      ctx.beginPath();

      frequencies.forEach((freq: number, i: number) => {
        const x = this.freqToX(freq, width);
        const y = this.gainToY(response[i], height);

        if (i === 0) {
          ctx.moveTo(x, y);
        } else {
          ctx.lineTo(x, y);
        }
      });

      ctx.stroke();
    });
  }

  private drawFilterHandles(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    isDarkMode: boolean,
  ): void {
    this.currentFilterParams.forEach((filter, idx) => {
      if (!filter.enabled) return;

      const x = this.freqToX(filter.frequency, width);
      const y = this.gainToY(filter.gain, height);
      const isSelected = idx === this.selectedFilterIndex;

      // Draw Q bar (horizontal line showing Q bandwidth)
      ctx.strokeStyle = isSelected
        ? "rgba(255, 200, 100, 0.8)"
        : "rgba(255, 255, 255, 0.4)";
      ctx.lineWidth = isSelected ? 3 : 2;

      const barWidth = 40 / filter.q;
      ctx.beginPath();
      ctx.moveTo(x - barWidth / 2, y);
      ctx.lineTo(x + barWidth / 2, y);
      ctx.stroke();

      // Draw handle point
      ctx.fillStyle = isSelected
        ? "rgba(255, 200, 100, 1)"
        : "rgba(255, 255, 255, 0.8)";
      ctx.beginPath();
      ctx.arc(x, y, isSelected ? 6 : 4, 0, Math.PI * 2);
      ctx.fill();

      // Draw selection ring
      if (isSelected) {
        ctx.strokeStyle = "rgba(255, 200, 100, 0.6)";
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(x, y, 10, 0, Math.PI * 2);
        ctx.stroke();
      }
    });
  }

  // ===== COORDINATE CONVERSION =====

  private freqToX(freq: number, width: number): number {
    const logMin = Math.log10(this.EQ_GRAPH_MIN_FREQ);
    const logMax = Math.log10(this.EQ_GRAPH_MAX_FREQ);
    const logFreq = Math.log10(
      Math.max(this.EQ_GRAPH_MIN_FREQ, Math.min(this.EQ_GRAPH_MAX_FREQ, freq)),
    );
    const normalized = (logFreq - logMin) / (logMax - logMin);
    return 60 + normalized * (width - 80); // 60px left margin, 20px right margin
  }

  private xToFreq(x: number, width: number): number {
    const normalized = (x - 60) / (width - 80); // Match new margins
    const logMin = Math.log10(this.EQ_GRAPH_MIN_FREQ);
    const logMax = Math.log10(this.EQ_GRAPH_MAX_FREQ);
    const logFreq = logMin + normalized * (logMax - logMin);
    return Math.pow(10, logFreq);
  }

  private gainToY(gain: number, height: number): number {
    const range = this.eqGraphMaxGain - this.eqGraphMinGain;
    const normalized = (gain - this.eqGraphMinGain) / range;
    return height - 30 - normalized * (height - 60);
  }

  private yToGain(y: number, height: number): number {
    const range = this.eqGraphMaxGain - this.eqGraphMinGain;
    const normalized = (height - 30 - y) / (height - 60);
    return this.eqGraphMinGain + normalized * range;
  }

  // ===== GRAPH INTERACTIONS =====

  private setupGraphInteractions(): void {
    if (!this.eqGraphCanvas) return;

    this.eqGraphCanvas.addEventListener("mousedown", (e) =>
      this.handleGraphMouseDown(e),
    );
    document.addEventListener("mousemove", (e) => this.handleGraphMouseMove(e));
    document.addEventListener("mouseup", () => this.handleGraphMouseUp());

    // Set cursor style
    this.eqGraphCanvas.style.cursor = "crosshair";
  }

  handleGraphMouseDown(e: MouseEvent): void {
    if (!this.eqGraphCanvas) return;

    const rect = this.eqGraphCanvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    const width = this.eqGraphCanvas.width;
    const clickedFreq = this.xToFreq(x, width);

    // Find closest filter by frequency
    let closestIdx = -1;
    let minFreqDist = Infinity;

    this.currentFilterParams.forEach((filter, idx) => {
      if (!filter.enabled) return;
      const freqDist = Math.abs(
        Math.log10(filter.frequency) - Math.log10(clickedFreq),
      );
      if (freqDist < minFreqDist) {
        minFreqDist = freqDist;
        closestIdx = idx;
      }
    });

    if (closestIdx >= 0) {
      this.selectedFilterIndex = closestIdx;
      this.isDraggingHandle = true;
      this.lastMouseY = y;
      this.drawEQGraph();
      this.renderEQTable(); // Update table to show selection
    }
  }

  handleGraphMouseMove(e: MouseEvent): void {
    if (!this.isDraggingHandle || !this.eqGraphCanvas) return;

    const rect = this.eqGraphCanvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    const height = this.eqGraphCanvas.height;
    const filter = this.currentFilterParams[this.selectedFilterIndex];
    if (!filter) return;

    // Up/down: change gain
    filter.gain = Math.max(
      this.eqGraphMinGain,
      Math.min(this.eqGraphMaxGain, this.yToGain(y, height)),
    );

    // Left/right: change Q
    // Calculate Q based on horizontal distance from initial click
    const xDelta = x - this.freqToX(filter.frequency, this.eqGraphCanvas.width);
    // Map horizontal distance to Q: center = current Q, moving away increases Q
    const qSensitivity = 0.05; // Adjust this to control sensitivity
    const qDelta = xDelta * qSensitivity;
    const newQ = 1.0 + qDelta;
    filter.q = Math.max(0.1, Math.min(3.0, newQ));

    // Recompute Y-axis range when gain changes
    this.computeDynamicYAxisRange();

    this.requestEQResponseUpdate();
    this.drawEQGraph();
    this.drawMiniEQ(); // Update mini EQ immediately while dragging
    this.updateTableInputs();
  }

  handleGraphMouseUp(): void {
    if (this.isDraggingHandle) {
      this.isDraggingHandle = false;
      this.updateFilterParams(this.currentFilterParams);
    }
  }

  handleGraphMouseLeave(): void {
    this.isDraggingHandle = false;
  }

  private updateTableSelection(): void {
    if (!this.eqTableContainer) return;

    const table = this.eqTableContainer.querySelector("table");
    if (!table) return;

    // Update selected column
    const cells = table.querySelectorAll(
      `td[data-filter-index="${this.selectedFilterIndex}"], th[data-filter-index="${this.selectedFilterIndex}"]`,
    );
    cells.forEach((cell) => cell.classList.add("selected"));

    // Remove selection from other columns
    const allCells = table.querySelectorAll(
      "td[data-filter-index], th[data-filter-index]",
    );
    allCells.forEach((cell) => {
      const index = parseInt(
        cell.getAttribute("data-filter-index") || "-1",
        10,
      );
      if (index !== this.selectedFilterIndex) {
        cell.classList.remove("selected");
      }
    });
  }

  private updateTableInputs(): void {
    if (!this.eqTableContainer) return;
    const filter = this.currentFilterParams[this.selectedFilterIndex];
    if (!filter) return;

    const table = this.eqTableContainer.querySelector("table");
    if (!table) return;

    const cells = table.querySelectorAll(
      `td[data-filter-index="${this.selectedFilterIndex}"]`,
    );
    cells.forEach((cell) => {
      const freqInput = cell.querySelector(".eq-frequency") as HTMLInputElement;
      const qInput = cell.querySelector(".eq-q") as HTMLInputElement;
      const gainInput = cell.querySelector(".eq-gain") as HTMLInputElement;

      if (freqInput) freqInput.value = filter.frequency.toFixed(1);
      if (qInput) qInput.value = filter.q.toFixed(2);
      if (gainInput) gainInput.value = filter.gain.toFixed(2);
    });
  }

  // ===== MINI EQ VISUALIZATION =====

  private drawMiniEQ(): void {
    if (!this.eqMiniCanvas || !this.eqMiniCtx) return;

    const ctx = this.eqMiniCtx;
    const width = this.eqMiniCanvas.width;
    const height = this.eqMiniCanvas.height;

    // Clear canvas
    ctx.clearRect(0, 0, width, height);

    // Draw background with theme awareness
    const isDark = document.documentElement.classList.contains("dark");
    ctx.fillStyle = isDark ? "#1a1a1a" : "#ffffff";
    ctx.fillRect(0, 0, width, height);

    // Draw center line (0 dB)
    ctx.strokeStyle = isDark ? "#404040" : "#d0d0d0";
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, height / 2);
    ctx.lineTo(width, height / 2);
    ctx.stroke();

    // Draw EQ curve if we have response data
    if (
      this.eqResponseData &&
      this.eqResponseData.frequencies &&
      this.eqResponseData.combined_response
    ) {
      const frequencies = this.eqResponseData.frequencies;
      const magnitudes = this.eqResponseData.combined_response; // Use combined_response, not magnitude_db

      // Determine gain range from response data
      let minGain = Math.min(...magnitudes);
      let maxGain = Math.max(...magnitudes);
      const gainRange = Math.max(Math.abs(minGain), Math.abs(maxGain));
      const displayRange = Math.max(6, gainRange); // At least ±6dB range

      ctx.strokeStyle = isDark ? "#4a9eff" : "#2563eb";
      ctx.lineWidth = 2;
      ctx.beginPath();

      for (let i = 0; i < frequencies.length; i++) {
        const freq = frequencies[i];
        const mag = magnitudes[i];

        // Map frequency to x (logarithmic)
        const x =
          (Math.log10(freq / this.EQ_GRAPH_MIN_FREQ) /
            Math.log10(this.EQ_GRAPH_MAX_FREQ / this.EQ_GRAPH_MIN_FREQ)) *
          width;

        // Map magnitude to y (inverted, 0dB at center)
        const y = height / 2 - (mag / displayRange) * (height / 2);

        if (i === 0) {
          ctx.moveTo(x, y);
        } else {
          ctx.lineTo(x, y);
        }
      }

      ctx.stroke();
    }
  }

  // ===== CLEANUP =====

  destroy(): void {
    // Clear timers
    if (this.eqResponseDebounceTimer) {
      clearTimeout(this.eqResponseDebounceTimer);
      this.eqResponseDebounceTimer = null;
    }

    // Clear containers
    if (this.playbackOptionsContainer) {
      this.playbackOptionsContainer.innerHTML = "";
    }
    if (this.eqTableContainer) {
      this.eqTableContainer.innerHTML = "";
    }
  }
}
