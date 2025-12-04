// Types for optimization parameters and results

export type PeqModel =
  | "pk"
  | "hp-pk"
  | "hp-pk-lp"
  | "ls-pk"
  | "ls-pk-hs"
  | "free-pk-free"
  | "free";

export const PEQ_MODEL_DESCRIPTIONS: Record<PeqModel, string> = {
  pk: "All filters are peak/bell filters",
  "hp-pk": "First filter is highpass, rest are peak filters",
  "hp-pk-lp":
    "First filter is highpass, last is lowpass, rest are peak filters",
  "ls-pk": "First filter is low shelve, rest are peak filters",
  "ls-pk-hs":
    "First filter is low shelve, last is high shelve, rest are peak filters",
  "free-pk-free":
    "First and last filters can be any type, middle filters are peak",
  free: "All filters can be any type",
};

export interface OptimizationParams {
  num_filters: number;
  curve_path?: string;
  target_path?: string;
  sample_rate: number;
  max_db: number;
  min_db: number;
  max_q: number;
  min_q: number;
  min_freq: number;
  max_freq: number;
  speaker?: string;
  version?: string;
  measurement?: string;
  curve_name: string;
  algo: string;
  population: number;
  maxeval: number;
  refine: boolean;
  local_algo: string;
  min_spacing_oct: number;
  spacing_weight: number;
  smooth: boolean;
  smooth_n: number;
  loss: string; // "flat", "score", "mixed", "drivers-flat", etc.
  peq_model?: PeqModel; // New PEQ model system
  iir_hp_pk: boolean; // Deprecated, kept for backward compatibility
  // DE-specific parameters
  strategy?: string;
  de_f?: number;
  de_cr?: number;
  adaptive_weight_f?: number;
  adaptive_weight_cr?: number;
  // Tolerance parameters
  tolerance: number;
  atolerance: number;
  // Captured curve data
  captured_frequencies?: number[];
  captured_magnitudes?: number[];
  // Target curve data (for headphones)
  target_frequencies?: number[];
  target_magnitudes?: number[];
  // Multi-driver crossover optimization parameters
  driver1_path?: string;
  driver2_path?: string;
  driver3_path?: string;
  driver4_path?: string;
  crossover_type?: string; // "butterworth2", "linkwitzriley2", "linkwitzriley4"
}

export interface PlotData {
  frequencies: number[];
  curves: { [name: string]: number[] };
  metadata: Record<string, unknown>;
}

export interface OptimizationResult {
  success: boolean;
  error_message?: string;
  filter_params?: number[];
  objective_value?: number;
  preference_score_before?: number;
  preference_score_after?: number;
  filter_response?: PlotData;
  spin_details?: PlotData;
  filter_plots?: PlotData;
  input_curve?: PlotData; // Original normalized input curve
  deviation_curve?: PlotData; // Target - Input (what needs to be corrected)
}

export interface ProgressData {
  iteration: number;
  fitness: number;
  convergence: number;
}

export interface OptimizationStage {
  status: string;
  startTime?: number;
  endTime?: number;
  details?: string;
}
