// Dynamic HTML template generation module
// Replaces the static HTML templates with TypeScript-generated content

import {
  ALGORITHM_OPTIONS,
  DE_STRATEGY_OPTIONS,
  LOSS_OPTIONS,
  CURVE_NAME_OPTIONS,
  LOCAL_ALGO_OPTIONS,
  WARNING_THRESHOLDS,
} from "./optimization-constants";

// Helper function to generate option elements from a record of options
function generateOptions(
  options: Record<string, string>,
  defaultValue?: string,
): string {
  return Object.entries(options)
    .map(([value, label]) => {
      const selected = defaultValue === value ? " selected" : "";
      return `                <option value="${value}"${selected}>${label}</option>`;
    })
    .join("\n");
}

// Helper function to group algorithms by category
function generateAlgorithmOptions(): string {
  const autoEQ: string[] = [];
  const nloptGlobal: string[] = [];
  const nloptLocal: string[] = [];
  const metaheuristics: string[] = [];

  Object.entries(ALGORITHM_OPTIONS).forEach(([value, label]) => {
    if (value.startsWith("autoeq:")) {
      autoEQ.push(
        `                    <option value="${value}">${label}</option>`,
      );
    } else if (value.startsWith("nlopt:")) {
      // Determine if it's global or local based on the algorithm name
      const localAlgos = ["cobyla", "bobyqa", "neldermead", "sbplx", "slsqp"];
      const algoName = value.split(":")[1];
      if (localAlgos.includes(algoName)) {
        nloptLocal.push(
          `                    <option value="${value}">${label}</option>`,
        );
      } else {
        nloptGlobal.push(
          `                    <option value="${value}">${label}</option>`,
        );
      }
    } else if (value.startsWith("mh:")) {
      metaheuristics.push(
        `                    <option value="${value}">${label}</option>`,
      );
    }
  });

  return `
                <optgroup label="AutoEQ Algorithms">
${autoEQ.join("\n")}
                </optgroup>
                <optgroup label="NLOPT Global Optimizers">
${nloptGlobal.join("\n")}
                </optgroup>
                <optgroup label="NLOPT Local Optimizers">
${nloptLocal.join("\n")}
                </optgroup>
                <optgroup label="Metaheuristics">
${metaheuristics.join("\n")}
                </optgroup>`;
}

// Generate DE Strategy options
function generateStrategyOptions(): string {
  return Object.entries(DE_STRATEGY_OPTIONS)
    .map(([value, label]) => {
      const recommended = value === "currenttobest1bin" ? " (Recommended)" : "";
      const selected = value === "currenttobest1bin" ? " selected" : "";
      return `                <option value="${value}"${selected}>${label}${recommended}</option>`;
    })
    .join("\n");
}

// Generate Head section
export function generateHead(): string {
  return `<head>
    <meta charset="UTF-8" />
    <link rel="stylesheet" href="/src/styles.css" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>AutoEQ - Automatic Speaker Equalization</title>
    <script type="module" src="/src/main.ts" defer></script>
</head>`;
}

// Generate Data Acquisition section
export function generateDataAcquisition(): string {
  return `<!-- Data Source -->
<div class="section-group">
    <h3>Data Acquisition</h3>
    <div class="input-source-tabs">
      <label class="tab-label active" data-tab="file" title="Files">
        <input type="radio" name="input_source" value="file" checked />
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"></path>
          <polyline points="13 2 13 9 20 9"></polyline>
        </svg>
      </label>
      <label class="tab-label" data-tab="speaker" title="Speakers">
        <input type="radio" name="input_source" value="speaker" />
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <rect width="16" height="20" x="4" y="2" rx="2"></rect>
          <path d="M12 6h.01"></path>
          <circle cx="12" cy="14" r="4"></circle>
          <path d="M12 14h.01"></path>
        </svg>
      </label>
      <label class="tab-label" data-tab="headphone" title="Headphones">
        <input type="radio" name="input_source" value="headphone" />
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M3 18v-6a9 9 0 0 1 18 0v6"></path>
          <path d="M21 19a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3zM3 19a2 2 0 0 0 2 2h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H3z"></path>
        </svg>
      </label>
      <label class="tab-label" data-tab="capture" title="Capture">
        <input type="radio" name="input_source" value="capture" />
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"></path>
          <path d="M19 10v2a7 7 0 0 1-14 0v-2"></path>
          <line x1="12" y1="19" x2="12" y2="23"></line>
          <line x1="8" y1="23" x2="16" y2="23"></line>
        </svg>
      </label>
    </div>

    <div id="file_inputs" class="tab-content active">
      <div class="compact-row">
        <input type="text" id="curve_path" name="curve_path" placeholder="Input CSV path" />
        <button type="button" id="browse_curve" class="browse-btn">📁</button>
      </div>
      <div class="compact-row">
        <input type="text" id="target_path" name="target_path" placeholder="Target CSV path (optional)" />
        <button type="button" id="browse_target" class="browse-btn">📁</button>
      </div>
    </div>

    <div id="speaker_inputs" class="tab-content">
      <div class="autocomplete-container">
        <input type="text" id="speaker" name="speaker" placeholder="Start typing speaker name..." autocomplete="off" />
        <div id="speaker_dropdown" class="autocomplete-dropdown"></div>
      </div>
      <select id="version" name="version" disabled>
        <option value="">Select Version</option>
      </select>
      <select id="measurement" name="measurement" disabled>
        <option value="">Select Measurement</option>
      </select>
    </div>

    <div id="headphone_inputs" class="tab-content">
      <div class="compact-row">
        <input type="text" id="headphone_curve_path" name="headphone_curve_path" placeholder="Headphone curve CSV path" />
        <button type="button" id="browse_headphone_curve" class="browse-btn">📁</button>
      </div>
      <select id="headphone_target" name="headphone_target">
        <option value="">Select Target...</option>
        <option value="harman-over-ear-2018">Harman Over-Ear 2018</option>
        <option value="harman-over-ear-2015">Harman Over-Ear 2015</option>
        <option value="harman-over-ear-2013">Harman Over-Ear 2013</option>
        <option value="harman-in-ear-2019">Harman In-Ear 2019</option>
      </select>
    </div>

    <div id="capture_inputs" class="tab-content">
      <div id="capture_panel_container" class="capture-panel-container">
        <!-- Full capture interface will be rendered here by the capture-panel web component -->
      </div>
    </div>
</div>`;
}

// Generate EQ Design section
export function generateEQDesign(): string {
  return `<!-- EQ Design -->
<div class="section-group">
    <h3>EQ Design</h3>

    <div class="param-group-section">
      <div class="inline-params">
	<div class="inline-item">
          <label>Loss</label>
          <select id="loss" name="loss">
	    ${generateOptions(LOSS_OPTIONS, "speaker-flat")}
          </select>
	</div>
	<div class="param-item">
          <label>Curve</label>
          <select id="curve_name" name="curve_name">
	    ${generateOptions(CURVE_NAME_OPTIONS, "Listening Window")}
          </select>
	</div>
      </div>
    </div>

    <div class="param-group-section">
      <div class="inline-params">
	<div class="param-item">
          <label>Filters</label>
          <input type="number" id="num_filters" name="num_filters" />
        </div>

        <!-- Basic Settings -->
        <div class="param-item">
          <label>Sample Rate</label>
          <input type="number" id="sample_rate" name="sample_rate" />
        </div>
      </div>
    </div>

    <div class="param-group-section">
      <div class="inline-params">
	<div class="param-item">
          <label>Min dB</label>
          <input type="number" id="min_db" name="min_db" />
	</div>
	<div class="param-item">
          <label>Max dB</label>
          <input type="number" id="max_db" name="max_db" />
	</div>
      <div class="param-item">
	  <label>Min Q</label>
	  <input type="number" id="min_q" name="min_q" />
	</div>
	<div class="param-item">
	  <label>Max Q</label>
	  <input type="number" id="max_q" name="max_q" />
	</div>
      </div>
    </div>

    <div class="param-group-section">
      <div class="inline-params">
	<div class="param-item">
	  <label>Min Freq</label>
	  <input type="number" id="min_freq" name="min_freq" />
	</div>
	<div class="param-item">
	  <label>Max Freq</label>
	  <input type="number" id="max_freq" name="max_freq" />
	</div>
      </div>
    </div>

    <div class="param-group-section">
      <div class="param-item">
        <label>PEQ Model</label>
        <select id="peq_model" name="peq_model">
          <option value="pk">PK - All Peak Filters</option>
          <option value="hp-pk">HP+PK - Highpass + Peaks</option>
          <option value="hp-pk-lp">HP+PK+LP - Highpass + Peaks + Lowpass</option>
          <option value="ls-pk">LS+PK - Low Shelf + Peaks</option>
          <option value="ls-pk-hs">LS+PK+HS - Low Shelf + Peaks + High Shelf</option>
          <option value="free-pk-free">Free+PK+Free - Flexible ends, peaks middle</option>
          <option value="free">Free - All filters flexible</option>
        </select>
      </div>
    </div>

    <div class="param-group-section">
      <div class="inline-params">
        <div class="param-item">
      	  <label>Min Spacing</label>
      	  <input type="number" id="min_spacing_oct" name="min_spacing_oct" />
        </div>
        <div class="param-item">
          <label>Spacing Weight</label>
      	  <input type="number" id="spacing_weight" name="spacing_weight" />
        </div>
      </div>
    </div>

    <div class="checkbox-group" style="display: none;">
      <!-- Hidden: deprecated in favor of PEQ model -->
      <label><input type="checkbox" id="iir_hp_pk" name="iir_hp_pk" /> Use HP+PK Filters (deprecated)</label>
    </div>
</div>`;
}

// Generate Optimization Fine Tuning section
export function generateOptimizationFineTuning(): string {
  const yellowThreshold = WARNING_THRESHOLDS.population.yellow;
  const redThreshold = WARNING_THRESHOLDS.population.red;

  return `<!-- Optimization Fine Tuning -->
<div class="section-group">
  <h3>Optimization Fine Tuning</h3>

  <div class="param-grid">
    <div class="param-item">
      <label>Algorithm</label>
      <select id="algo" name="algo">${generateAlgorithmOptions()}</select>
    </div>
  </div>

  <div class="param-group-section">
    <div class="inline-params">
      <div class="param-item global_algo_param">
	<label>Population</label>
	<input type="number" id="population" name="population"/>
	<div class="param-warning"
	     id="population_warning_yellow"
	     style="display: none; color: #ffc107; font-size: 0.8em; margin-top: 2px;">
	  ⚠️ Values above ${yellowThreshold} may be slow
	</div>
	<div class="param-warning"
	     id="population_warning_red"
	     style="display: none; color: #dc3545; font-size: 0.8em; margin-top: 2px;">
	  🚨 Values above ${redThreshold} will be very slow and may cause issues
	</div>
      </div>
      <div class="param-item global_algo_param">
	<label>Max Eval</label>
	<input type="number" id="maxeval" name="maxeval"/>
      </div>
    </div>
  </div>

  <div class="param-group-section">
    <div class="inline-params">
      <div class="param-item de-param" id="strategy_param">
	<label>Strategy</label>
	<select id="strategy" name="strategy">${generateStrategyOptions()}</select>
      </div>
      <div class="param-item de-param" id="mutation_param">
        <label>F</label>
        <input type="number" id="de_f" name="de_f" />
      </div>
      <div class="param-item de-param" id="recombination_param">
        <label>CR</label>
        <input type="number" id="de_cr" name="de_cr"/>
      </div>
      <div
	class="param-item adaptive-param"
	id="adaptive_weight_f_param"
	style="display: none"
	>
	<label>Adaptive F</label>
	<input
          type="number"
          id="adaptive_weight_f"
          name="adaptive_weight_f"
	  />
      </div>
      <div
	class="param-item adaptive-param"
	id="adaptive_weight_cr_param"
	style="display: none"
	>
	<label>Adaptive CR</label>
	<input
          type="number"
          id="adaptive_weight_cr"
          name="adaptive_weight_cr"
	  />
      </div>
    </div>
  </div>

  <div class="param-group-section">
    <div class="inline-params">
      <div class="param-item">
	<label>Tolerance</label>
	<input
          type="number"
          id="tolerance"
          name="tolerance"
	  />
      </div>
      <div class="param-item">
	<label>Abs Tolerance</label>
	<input
          type="number"
          id="abs_tolerance"
          name="abs_tolerance"
	  />
      </div>
    </div>
  </div>

  <div class="param-group-section">
    <div class="inline-params">
      <div class="inline-item checkbox-item">
        <label class="checkbox-label"
               ><input
		  type="checkbox"
		  id="refine"
		  name="refine"
		  />Enable</label
			    >
      </div>
      <div class="inline-item flex-grow">
        <label>Local Optimiser</label>
        <select
          id="local_algo"
          name="local_algo"
          disabled
          >
	  ${generateOptions(LOCAL_ALGO_OPTIONS, "cobyla")}
        </select>
      </div>
    </div>
  </div>

  <div class="param-group-section">
    <div class="inline-params">
      <div class="inline-item checkbox-item">
        <label class="checkbox-label"><input type="checkbox" id="smooth" name="smooth" />Enable</label>
      </div>
      <div class="inline-item">
        <label>Smooth 1/N octave</label>
        <input type="number" id="smooth_n" name="smooth_n"/>
      </div>
    </div>
  </div>
</div>`;
}

// Generate Plots Panel
export function generatePlotsPanel(): string {
  return `<div class="plots-vertical">
    <!-- Filter Response Graph - Always visible -->
    <div class="plot-vertical-item" id="filter_vertical_item" style="display: flex;">
        <div class="plot-vertical-header">
            <h4>Filter</h4>
        </div>
        <div id="filter_plot" class="plot-vertical-container">
            <div class="plot-placeholder">No data to display</div>
        </div>
    </div>

    <!-- Spinorama Graph - Visible when CEA2034 data available -->
    <div class="plot-vertical-item" id="spin_vertical_item" style="display: none;">
        <div class="plot-vertical-header">
            <h4>Spinorama</h4>
        </div>
        <div id="spin_plot" class="plot-vertical-container">
            <div class="plot-placeholder">No data to display</div>
        </div>
    </div>

    <!-- Details Plot - Visible when CEA2034 data available -->
    <div class="plot-vertical-item" id="details_vertical_item" style="display: none;">
        <div class="plot-vertical-header">
            <h4>Details</h4>
        </div>
        <div id="details_plot" class="plot-vertical-container">
            <div class="plot-placeholder">No data to display</div>
        </div>
    </div>

    <!-- Tonal Balance Graph - Visible when CEA2034 data available -->
    <div class="plot-vertical-item" id="tonal_vertical_item" style="display: none;">
        <div class="plot-vertical-header">
            <h4>Tonal</h4>
        </div>
        <div id="tonal_plot" class="plot-vertical-container">
            <div class="plot-placeholder">No data to display</div>
        </div>
    </div>
</div>

<div
    id="error_display"
    class="error-display"
    style="display: none"
>
    <h4>Error</h4>
    <div id="error_message" class="error-message"></div>
</div>`;
}

// Generate Bottom Row
export function generateBottomRow(): string {
  return `<!-- Unified Bottom Row with Controls and Results -->
<div class="bottom-row">
    <!-- Left side: Action buttons (centered in left panel) -->
    <div class="bottom-left">
        <div class="bottom-actions">
            <button
                type="submit"
                form="autoeq_form"
                id="optimize_btn"
                class="optimize-button"
            >
                Run Optimization
            </button>
            <button
                type="button"
                id="reset_btn"
                class="reset-button"
            >
                Reset
            </button>
        </div>
    </div>

    <!-- Resizer spacer -->
    <div class="bottom-resizer-spacer"></div>

    <!-- Right side: Optimization results (centered in right panel) -->
    <div class="bottom-right">
        <div class="optimization-results">
            <div class="score-item">
                <label>Before:</label>
                <span id="score_before">-</span>
            </div>
            <div class="score-item">
                <label>After:</label>
                <span id="score_after">-</span>
            </div>
            <div class="score-item improvement">
                <label>Improvement:</label>
                <span id="score_improvement">-</span>
            </div>
            <select
                id="export_format_select"
                class="export-format-select"
                title="Select export format"
            >
                <option value="apo">APO</option>
                <option value="aupreset">AUpreset</option>
                <option value="rme">RME Channel</option>
                <option value="rme-room">RME Room</option>
            </select>
            <button
                type="button"
                id="download_apo_btn"
                class="btn btn-secondary"
                disabled
                title="Download optimized EQ"
            >
                💾 Download
            </button>
        </div>
    </div>
</div>`;
}

// Generate Optimization Modal
export function generateOptimizationModal(): string {
  return `<div id="optimization_modal" class="modal" style="display: none">
    <div class="modal-content">
        <div class="modal-header">
          <h3>Optimization Progress</h3>
          <div class="progress-info"></div>
          <button id="modal_close" class="modal-close-btn">
            &times;
          </button>
        </div>
        <div class="modal-body">
          <div class="progress-graph-container">
            <div
              id="progress_graph"
              style="width: 400px; height: 400px"
            ></div>
          </div>
        </div>
        <div class="modal-footer">
          <h3 id="progress_status">
            Initializing...
          </h3>
          <button id="cancel_optimization" class="btn btn-danger">
            Cancel
          </button>
          <button
            id="done_optimization"
            class="btn btn-success"
            style="display: none"
          >
                Done
          </button>
        </div>
    </div>
</div>`;
}
