// Optimization Step Component
// EQ Design and Optimization configuration for the step-by-step workflow

import {
  OPTIMIZATION_DEFAULTS,
  LOSS_OPTIONS,
  CURVE_NAME_OPTIONS,
  ALGORITHM_OPTIONS,
  DE_STRATEGY_OPTIONS,
  LOCAL_ALGO_OPTIONS,
} from "./optimization-constants";

export interface OptimizationStepConfig {
  onOptimizationStart?: () => void;
  onOptimizationComplete?: () => void;
}

export class OptimizationStep {
  private container: HTMLElement;
  private config: OptimizationStepConfig;

  constructor(container: HTMLElement, config: OptimizationStepConfig = {}) {
    this.container = container;
    this.config = config;
    this.render();
    this.attachEventListeners();
  }

  /**
   * Render the optimization step UI
   */
  private render(): void {
    this.container.classList.add("optimization-step");
    this.container.innerHTML = this.generateHTML();
  }

  /**
   * Generate HTML for the optimization step
   */
  private generateHTML(): string {
    return `
      <div class="step-content-wrapper">
        <div class="step-header-section">
          <h2 class="step-title">Optimization Configuration</h2>
          <p class="step-description">
            Configure EQ design parameters and optimization settings to achieve the best results.
          </p>
        </div>

        <div class="optimization-content">
          <!-- EQ Design Section -->
          <div class="section-group">
            <h3>EQ Design</h3>

            <div class="param-row">
              <div class="param-item">
                <label for="opt_loss">Loss Function</label>
                <select id="opt_loss" name="loss">
                  ${this.generateOptions(LOSS_OPTIONS, "speaker-flat")}
                </select>
                <p class="input-hint">Optimization objective (flat response vs. Harman score)</p>
              </div>

              <div class="param-item">
                <label for="opt_curve_name">Curve</label>
                <select id="opt_curve_name" name="curve_name">
                  ${this.generateOptions(CURVE_NAME_OPTIONS, "Listening Window")}
                </select>
                <p class="input-hint">Speaker measurement curve to optimize</p>
              </div>
            </div>

            <div class="param-row">
              <div class="param-item">
                <label for="opt_num_filters">Number of Filters</label>
                <input type="number" id="opt_num_filters" name="num_filters" value="${OPTIMIZATION_DEFAULTS.num_filters}" min="1" max="20" />
                <p class="input-hint">More filters = more precise but complex EQ</p>
              </div>

              <div class="param-item">
                <label for="opt_sample_rate">Sample Rate (Hz)</label>
                <input type="number" id="opt_sample_rate" name="sample_rate" value="${OPTIMIZATION_DEFAULTS.sample_rate}" step="1000" />
                <p class="input-hint">Audio sample rate (typically 48000)</p>
              </div>
            </div>

            <div class="param-row">
              <div class="param-item">
                <label for="opt_min_db">Min Gain (dB)</label>
                <input type="number" id="opt_min_db" name="min_db" value="${OPTIMIZATION_DEFAULTS.min_db}" step="0.5" />
              </div>

              <div class="param-item">
                <label for="opt_max_db">Max Gain (dB)</label>
                <input type="number" id="opt_max_db" name="max_db" value="${OPTIMIZATION_DEFAULTS.max_db}" step="0.5" />
              </div>

              <div class="param-item">
                <label for="opt_min_q">Min Q</label>
                <input type="number" id="opt_min_q" name="min_q" value="${OPTIMIZATION_DEFAULTS.min_q}" step="0.1" />
              </div>

              <div class="param-item">
                <label for="opt_max_q">Max Q</label>
                <input type="number" id="opt_max_q" name="max_q" value="${OPTIMIZATION_DEFAULTS.max_q}" step="0.1" />
              </div>
            </div>

            <div class="param-row">
              <div class="param-item">
                <label for="opt_min_freq">Min Frequency (Hz)</label>
                <input type="number" id="opt_min_freq" name="min_freq" value="${OPTIMIZATION_DEFAULTS.min_freq}" step="10" />
              </div>

              <div class="param-item">
                <label for="opt_max_freq">Max Frequency (Hz)</label>
                <input type="number" id="opt_max_freq" name="max_freq" value="${OPTIMIZATION_DEFAULTS.max_freq}" step="100" />
              </div>
            </div>

            <div class="param-row">
              <div class="param-item full-width">
                <label for="opt_peq_model">PEQ Model</label>
                <select id="opt_peq_model" name="peq_model">
                  <option value="pk">PK - All Peak Filters</option>
                  <option value="hp-pk">HP+PK - Highpass + Peaks</option>
                  <option value="hp-pk-lp">HP+PK+LP - Highpass + Peaks + Lowpass</option>
                  <option value="ls-pk">LS+PK - Low Shelf + Peaks</option>
                  <option value="ls-pk-hs">LS+PK+HS - Low Shelf + Peaks + High Shelf</option>
                  <option value="free-pk-free">Free+PK+Free - Flexible ends, peaks middle</option>
                  <option value="free">Free - All filters flexible</option>
                </select>
                <p class="input-hint">Filter architecture (shelves, peaks, passes)</p>
              </div>
            </div>

            <div class="param-row">
              <div class="param-item">
                <label for="opt_min_spacing_oct">Min Spacing (octaves)</label>
                <input type="number" id="opt_min_spacing_oct" name="min_spacing_oct" value="${OPTIMIZATION_DEFAULTS.min_spacing_oct}" step="0.1" />
                <p class="input-hint">Minimum frequency separation</p>
              </div>

              <div class="param-item">
                <label for="opt_spacing_weight">Spacing Weight</label>
                <input type="number" id="opt_spacing_weight" name="spacing_weight" value="${OPTIMIZATION_DEFAULTS.spacing_weight}" step="0.01" />
                <p class="input-hint">Penalty for violating spacing constraint</p>
              </div>
            </div>
          </div>

          <!-- Optimization Fine Tuning Section -->
          <div class="section-group">
            <h3>Optimization Fine Tuning</h3>

            <div class="param-row">
              <div class="param-item full-width">
                <label for="opt_algo">Algorithm</label>
                <select id="opt_algo" name="algo">
                  ${this.generateAlgorithmOptions()}
                </select>
                <p class="input-hint">Global optimizer (DE recommended) or local optimizer</p>
              </div>
            </div>

            <div class="param-row">
              <div class="param-item">
                <label for="opt_population">Population</label>
                <input type="number" id="opt_population" name="population" value="${OPTIMIZATION_DEFAULTS.population}" min="10" />
                <p class="input-hint">Population size (higher = slower but more thorough)</p>
              </div>

              <div class="param-item">
                <label for="opt_maxeval">Max Evaluations</label>
                <input type="number" id="opt_maxeval" name="maxeval" value="${OPTIMIZATION_DEFAULTS.maxeval}" min="100" step="100" />
                <p class="input-hint">Maximum function evaluations</p>
              </div>
            </div>

            <div class="param-row" id="de_params_row">
              <div class="param-item">
                <label for="opt_strategy">DE Strategy</label>
                <select id="opt_strategy" name="strategy">
                  ${this.generateStrategyOptions()}
                </select>
              </div>

              <div class="param-item">
                <label for="opt_de_f">Mutation (F)</label>
                <input type="number" id="opt_de_f" name="de_f" value="${OPTIMIZATION_DEFAULTS.de_f}" step="0.1" min="0" max="2" />
              </div>

              <div class="param-item">
                <label for="opt_de_cr">Crossover (CR)</label>
                <input type="number" id="opt_de_cr" name="de_cr" value="${OPTIMIZATION_DEFAULTS.de_cr}" step="0.1" min="0" max="1" />
              </div>
            </div>

            <div class="param-row">
              <div class="param-item">
                <label for="opt_tolerance">Tolerance</label>
                <input type="number" id="opt_tolerance" name="tolerance" value="${OPTIMIZATION_DEFAULTS.tolerance}" step="0.0001" />
              </div>

              <div class="param-item">
                <label for="opt_abs_tolerance">Absolute Tolerance</label>
                <input type="number" id="opt_abs_tolerance" name="abs_tolerance" value="${OPTIMIZATION_DEFAULTS.abs_tolerance}" step="0.0001" />
              </div>
            </div>

            <div class="param-row">
              <div class="param-item checkbox-item">
                <label class="checkbox-label">
                  <input type="checkbox" id="opt_refine" name="refine" />
                  <span>Enable Local Refinement</span>
                </label>
                <p class="input-hint">Polish results with local optimizer</p>
              </div>

              <div class="param-item">
                <label for="opt_local_algo">Local Algorithm</label>
                <select id="opt_local_algo" name="local_algo" disabled>
                  ${this.generateOptions(LOCAL_ALGO_OPTIONS, "cobyla")}
                </select>
              </div>
            </div>

            <div class="param-row">
              <div class="param-item checkbox-item">
                <label class="checkbox-label">
                  <input type="checkbox" id="opt_smooth" name="smooth" />
                  <span>Enable Smoothing</span>
                </label>
                <p class="input-hint">Apply 1/N octave smoothing</p>
              </div>

              <div class="param-item">
                <label for="opt_smooth_n">Smoothing (1/N octave)</label>
                <input type="number" id="opt_smooth_n" name="smooth_n" value="${OPTIMIZATION_DEFAULTS.smooth_n}" disabled />
              </div>
            </div>
          </div>

          <!-- Run Optimization Button -->
          <div class="optimization-actions">
            <button type="button" id="run_optimization_btn" class="btn-primary btn-large">
              ▶ Run Optimization
            </button>
            <button type="button" id="reset_form_btn" class="btn-secondary">
              Reset to Defaults
            </button>
          </div>

          <!-- Results Placeholder -->
          <div id="optimization_results" class="optimization-results" style="display: none">
            <h3>Optimization Results</h3>
            <div class="results-summary">
              <div class="result-item">
                <label>Score Before:</label>
                <span id="score_before">-</span>
              </div>
              <div class="result-item">
                <label>Score After:</label>
                <span id="score_after">-</span>
              </div>
              <div class="result-item improvement">
                <label>Improvement:</label>
                <span id="score_improvement">-</span>
              </div>
            </div>
            <p class="results-note">✅ Optimization complete! Proceed to the next step to test your EQ.</p>
          </div>
        </div>
      </div>
    `;
  }

  /**
   * Generate option elements from a record
   */
  private generateOptions(
    options: Record<string, string>,
    defaultValue?: string,
  ): string {
    return Object.entries(options)
      .map(([value, label]) => {
        const selected = defaultValue === value ? " selected" : "";
        return `<option value="${value}"${selected}>${label}</option>`;
      })
      .join("\n");
  }

  /**
   * Generate algorithm options with optgroups
   */
  private generateAlgorithmOptions(): string {
    const autoEQ: string[] = [];
    const nloptGlobal: string[] = [];
    const nloptLocal: string[] = [];
    const metaheuristics: string[] = [];

    Object.entries(ALGORITHM_OPTIONS).forEach(([value, label]) => {
      if (value.startsWith("autoeq:")) {
        autoEQ.push(`<option value="${value}">${label}</option>`);
      } else if (value.startsWith("nlopt:")) {
        const localAlgos = ["cobyla", "bobyqa", "neldermead", "sbplx", "slsqp"];
        const algoName = value.split(":")[1];
        if (localAlgos.includes(algoName)) {
          nloptLocal.push(`<option value="${value}">${label}</option>`);
        } else {
          nloptGlobal.push(`<option value="${value}">${label}</option>`);
        }
      } else if (value.startsWith("mh:")) {
        metaheuristics.push(`<option value="${value}">${label}</option>`);
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
      </optgroup>
    `;
  }

  /**
   * Generate DE strategy options
   */
  private generateStrategyOptions(): string {
    return Object.entries(DE_STRATEGY_OPTIONS)
      .map(([value, label]) => {
        const recommended =
          value === "currenttobest1bin" ? " (Recommended)" : "";
        const selected = value === "currenttobest1bin" ? " selected" : "";
        return `<option value="${value}"${selected}>${label}${recommended}</option>`;
      })
      .join("\n");
  }

  /**
   * Attach event listeners
   */
  private attachEventListeners(): void {
    // Refinement checkbox
    const refineCheckbox = this.container.querySelector(
      "#opt_refine",
    ) as HTMLInputElement;
    const localAlgoSelect = this.container.querySelector(
      "#opt_local_algo",
    ) as HTMLSelectElement;

    if (refineCheckbox && localAlgoSelect) {
      refineCheckbox.addEventListener("change", () => {
        localAlgoSelect.disabled = !refineCheckbox.checked;
      });
    }

    // Smoothing checkbox
    const smoothCheckbox = this.container.querySelector(
      "#opt_smooth",
    ) as HTMLInputElement;
    const smoothNInput = this.container.querySelector(
      "#opt_smooth_n",
    ) as HTMLInputElement;

    if (smoothCheckbox && smoothNInput) {
      smoothCheckbox.addEventListener("change", () => {
        smoothNInput.disabled = !smoothCheckbox.checked;
      });
    }

    // Run optimization button
    const runBtn = this.container.querySelector(
      "#run_optimization_btn",
    ) as HTMLButtonElement;
    if (runBtn) {
      runBtn.addEventListener("click", () => this.handleRunOptimization());
    }

    // Reset button
    const resetBtn = this.container.querySelector(
      "#reset_form_btn",
    ) as HTMLButtonElement;
    if (resetBtn) {
      resetBtn.addEventListener("click", () => this.resetToDefaults());
    }
  }

  /**
   * Handle run optimization button click
   */
  private handleRunOptimization(): void {
    console.log("🚀 Starting optimization...");

    if (this.config.onOptimizationStart) {
      this.config.onOptimizationStart();
    }

    // For demo purposes, show mock results after a delay
    setTimeout(() => {
      this.showMockResults();
    }, 2000);
  }

  /**
   * Show mock optimization results (for demo)
   */
  private showMockResults(): void {
    const resultsDiv = this.container.querySelector(
      "#optimization_results",
    ) as HTMLElement;
    const scoreBefore = this.container.querySelector(
      "#score_before",
    ) as HTMLElement;
    const scoreAfter = this.container.querySelector(
      "#score_after",
    ) as HTMLElement;
    const scoreImprovement = this.container.querySelector(
      "#score_improvement",
    ) as HTMLElement;

    if (resultsDiv && scoreBefore && scoreAfter && scoreImprovement) {
      scoreBefore.textContent = "3.45";
      scoreAfter.textContent = "7.82";
      scoreImprovement.textContent = "+4.37 (+126%)";
      resultsDiv.style.display = "block";

      if (this.config.onOptimizationComplete) {
        this.config.onOptimizationComplete();
      }
    }
  }

  /**
   * Reset form to default values
   */
  private resetToDefaults(): void {
    // Reset all inputs to defaults
    (
      this.container.querySelector("#opt_num_filters") as HTMLInputElement
    ).value = String(OPTIMIZATION_DEFAULTS.num_filters);
    (
      this.container.querySelector("#opt_sample_rate") as HTMLInputElement
    ).value = String(OPTIMIZATION_DEFAULTS.sample_rate);
    (this.container.querySelector("#opt_min_db") as HTMLInputElement).value =
      String(OPTIMIZATION_DEFAULTS.min_db);
    (this.container.querySelector("#opt_max_db") as HTMLInputElement).value =
      String(OPTIMIZATION_DEFAULTS.max_db);
    (this.container.querySelector("#opt_min_q") as HTMLInputElement).value =
      String(OPTIMIZATION_DEFAULTS.min_q);
    (this.container.querySelector("#opt_max_q") as HTMLInputElement).value =
      String(OPTIMIZATION_DEFAULTS.max_q);
    (this.container.querySelector("#opt_min_freq") as HTMLInputElement).value =
      String(OPTIMIZATION_DEFAULTS.min_freq);
    (this.container.querySelector("#opt_max_freq") as HTMLInputElement).value =
      String(OPTIMIZATION_DEFAULTS.max_freq);
    (
      this.container.querySelector("#opt_min_spacing_oct") as HTMLInputElement
    ).value = String(OPTIMIZATION_DEFAULTS.min_spacing_oct);
    (
      this.container.querySelector("#opt_spacing_weight") as HTMLInputElement
    ).value = String(OPTIMIZATION_DEFAULTS.spacing_weight);
    (
      this.container.querySelector("#opt_population") as HTMLInputElement
    ).value = String(OPTIMIZATION_DEFAULTS.population);
    (this.container.querySelector("#opt_maxeval") as HTMLInputElement).value =
      String(OPTIMIZATION_DEFAULTS.maxeval);
    (this.container.querySelector("#opt_de_f") as HTMLInputElement).value =
      String(OPTIMIZATION_DEFAULTS.de_f);
    (this.container.querySelector("#opt_de_cr") as HTMLInputElement).value =
      String(OPTIMIZATION_DEFAULTS.de_cr);
    (this.container.querySelector("#opt_tolerance") as HTMLInputElement).value =
      String(OPTIMIZATION_DEFAULTS.tolerance);
    (
      this.container.querySelector("#opt_abs_tolerance") as HTMLInputElement
    ).value = String(OPTIMIZATION_DEFAULTS.abs_tolerance);
    (this.container.querySelector("#opt_smooth_n") as HTMLInputElement).value =
      String(OPTIMIZATION_DEFAULTS.smooth_n);

    console.log("✅ Reset to default values");
  }

  /**
   * Get form data
   */
  public getFormData(): FormData {
    const formData = new FormData();

    // Collect all form fields
    const inputs = this.container.querySelectorAll("input, select");
    inputs.forEach((input) => {
      const element = input as HTMLInputElement | HTMLSelectElement;
      if (element.name) {
        if (element.type === "checkbox") {
          formData.append(
            element.name,
            (element as HTMLInputElement).checked ? "true" : "false",
          );
        } else {
          formData.append(element.name, element.value);
        }
      }
    });

    return formData;
  }

  /**
   * Update configuration
   */
  public updateConfig(config: Partial<OptimizationStepConfig>): void {
    this.config = { ...this.config, ...config };
  }

  /**
   * Refresh the component
   */
  public refresh(): void {
    this.render();
    this.attachEventListeners();
  }

  /**
   * Destroy the component
   */
  public destroy(): void {
    this.container.innerHTML = "";
    this.container.classList.remove("optimization-step");
  }
}
