// Centralized optimization constants and default values

export interface OptimizationDefaults {
  // Core EQ Parameters
  num_filters: number;
  sample_rate: number;
  min_db: number;
  max_db: number;
  min_q: number;
  max_q: number;
  min_freq: number;
  max_freq: number;

  // Algorithm Parameters
  algo: string;
  population: number;
  maxeval: number;

  // DE-specific Parameters
  de_f: number;
  de_cr: number;
  strategy: string;

  // Adaptive Parameters
  adaptive_weight_f: number;
  adaptive_weight_cr: number;

  // Spacing Parameters
  min_spacing_oct: number;
  spacing_weight: number;

  // Tolerance Parameters
  tolerance: number;
  abs_tolerance: number;

  // Refinement Parameters
  refine: boolean;
  local_algo: string;

  // Smoothing Parameters
  smooth: boolean;
  smooth_n: number;

  // Loss Function
  loss: string;

  // Filter Type
  iir_hp_pk: boolean; // Deprecated, kept for backward compatibility
  peq_model: string; // New PEQ model system

  // Curve Selection
  curve_name: string;

  // Input Source
  input_source: string;
}

export interface OptimizationLimits {
  num_filters: { min: number; max: number };
  sample_rate: { min: number; max: number };
  min_db: { min: number; max: number };
  max_db: { min: number; max: number };
  min_q: { min: number; max: number };
  max_q: { min: number; max: number };
  min_freq: { min: number; max: number };
  max_freq: { min: number; max: number };
  population: { min: number; max: number };
  maxeval: { min: number; max: number };
  de_f: { min: number; max: number };
  de_cr: { min: number; max: number };
  adaptive_weight_f: { min: number; max: number };
  adaptive_weight_cr: { min: number; max: number };
  min_spacing_oct: { min: number; max: number };
  spacing_weight: { min: number; max: number };
  tolerance: { min: number; max: number };
  abs_tolerance: { min: number; max: number };
  smooth_n: { min: number; max: number };
}

export interface OptimizationSteps {
  num_filters: number;
  sample_rate: number;
  min_db: number;
  max_db: number;
  min_q: number;
  max_q: number;
  min_freq: number;
  max_freq: number;
  population: number;
  maxeval: number;
  de_f: number;
  de_cr: number;
  adaptive_weight_f: number;
  adaptive_weight_cr: number;
  min_spacing_oct: number;
  spacing_weight: number;
  tolerance: number;
  abs_tolerance: number;
  smooth_n: number;
}

// Default values for all optimization parameters
export const OPTIMIZATION_DEFAULTS: OptimizationDefaults = {
  // Core EQ Parameters
  num_filters: 5,
  sample_rate: 48000,
  min_db: 1.0,
  max_db: 3.0,
  min_q: 1.0,
  max_q: 3.0,
  min_freq: 60,
  max_freq: 16000,

  // Algorithm Parameters
  algo: "autoeq:de",
  population: 30,
  maxeval: 20000,

  // DE-specific Parameters
  de_f: 0.8,
  de_cr: 0.9,
  strategy: "currenttobest1bin",

  // Adaptive Parameters
  adaptive_weight_f: 0.8,
  adaptive_weight_cr: 0.7,

  // Spacing Parameters
  min_spacing_oct: 0.5,
  spacing_weight: 20.0,

  // Tolerance Parameters
  tolerance: 1e-3,
  abs_tolerance: 1e-4,

  // Refinement Parameters
  refine: false,
  local_algo: "cobyla",

  // Smoothing Parameters
  smooth: true,
  smooth_n: 1,

  // Loss Function
  loss: "speaker-flat",

  // Filter Type
  iir_hp_pk: false, // Deprecated
  peq_model: "pk", // Default to all peak filters

  // Curve Selection
  curve_name: "Listening Window",

  // Input Source
  input_source: "file",
};

// Minimum and maximum limits for parameters
export const OPTIMIZATION_LIMITS: OptimizationLimits = {
  num_filters: { min: 1, max: 20 },
  sample_rate: { min: 8000, max: 192000 },
  min_db: { min: 0.1, max: 25 },
  max_db: { min: 0.1, max: 25 },
  min_q: { min: 0.1, max: 10 },
  max_q: { min: 0.1, max: 10 },
  min_freq: { min: 20, max: 20000 },
  max_freq: { min: 20, max: 20000 },
  population: { min: 10, max: 10000 },
  maxeval: { min: 200, max: 1000000 },
  de_f: { min: 0.0, max: 2.0 },
  de_cr: { min: 0.0, max: 1.0 },
  adaptive_weight_f: { min: 0.1, max: 1.0 },
  adaptive_weight_cr: { min: 0.1, max: 1.0 },
  min_spacing_oct: { min: 0.01, max: 10.0 },
  spacing_weight: { min: 0.0, max: 1000.0 },
  tolerance: { min: 1e-15, max: 1.0 },
  abs_tolerance: { min: 1e-15, max: 1.0 },
  smooth_n: { min: 1, max: 24 },
};

// Step sizes for input controls
export const OPTIMIZATION_STEPS: OptimizationSteps = {
  num_filters: 1,
  sample_rate: 48000,
  min_db: 0.1,
  max_db: 0.1,
  min_q: 0.1,
  max_q: 0.1,
  min_freq: 1,
  max_freq: 100,
  population: 1,
  maxeval: 1,
  de_f: 0.01,
  de_cr: 0.01,
  adaptive_weight_f: 0.01,
  adaptive_weight_cr: 0.01,
  min_spacing_oct: 0.01,
  spacing_weight: 1,
  tolerance: 1e-12,
  abs_tolerance: 1e-15,
  smooth_n: 1,
};

// Algorithm options
export const ALGORITHM_OPTIONS = {
  "autoeq:de": "Auto DE (Recommended)",
  "nlopt:isres": "NLOPT ISRES",
  "nlopt:ags": "NLOPT AGS",
  "nlopt:origdirect": "NLOPT Original DIRECT",
  "nlopt:crs2lm": "NLOPT CRS2 LM",
  "nlopt:direct": "NLOPT DIRECT",
  "nlopt:directl": "NLOPT DIRECT-L",
  "nlopt:gmlsl": "NLOPT GMLSL",
  "nlopt:gmlsllds": "NLOPT GMLSL LDS",
  "nlopt:stogo": "NLOPT StoGO",
  "nlopt:stogorand": "NLOPT StoGO Rand",
  "nlopt:cobyla": "NLOPT COBYLA",
  "nlopt:bobyqa": "NLOPT BOBYQA",
  "nlopt:neldermead": "NLOPT Nelder-Mead",
  "nlopt:sbplx": "NLOPT Subplex",
  "nlopt:slsqp": "NLOPT SLSQP",
  "mh:de": "MH Differential Evolution",
  "mh:pso": "MH Particle Swarm",
  "mh:rga": "MH Genetic Algorithm",
  "mh:tlbo": "MH TLBO",
  "mh:firefly": "MH Firefly",
};

// DE Strategy options
export const DE_STRATEGY_OPTIONS = {
  currenttobest1bin: "Current-to-Best/1/Bin (Recommended)",
  rand1bin: "Rand/1/Bin",
  best1bin: "Best/1/Bin",
  rand2bin: "Rand/2/Bin",
  best2bin: "Best/2/Bin",
  randtobest1bin: "Rand-to-Best/1/Bin",
  rand1exp: "Rand/1/Exp",
  best1exp: "Best/1/Exp",
  rand2exp: "Rand/2/Exp",
  best2exp: "Best/2/Exp",
  currenttobest1exp: "Current-to-Best/1/Exp",
  randtobest1exp: "Rand-to-Best/1/Exp",
  adaptivebin: "Adaptive/Bin (Experimental)",
  adaptiveexp: "Adaptive/Exp (Experimental)",
};

// Loss function options
export const LOSS_OPTIONS = {
  "speaker-flat": "Speaker Flat",
  "speaker-score": "Speaker Score",
  "headphone-flat": "Headphone Flat",
  "headphone-score": "Headphone Score",
};

// Speaker-specific loss options
export const SPEAKER_LOSS_OPTIONS = {
  "speaker-flat": "Speaker Flat",
  "speaker-score": "Speaker Score",
};

// Headphone-specific loss options
export const HEADPHONE_LOSS_OPTIONS = {
  "headphone-flat": "Headphone Flat",
  "headphone-score": "Headphone Score",
};

// Curve name options
export const CURVE_NAME_OPTIONS = {
  "Listening Window": "Listening Window",
  "On Axis": "On Axis",
  "Early Reflections": "Early Reflections",
  "Sound Power": "Sound Power",
  "Estimated In-Room Response": "Estimated In-Room Response",
};

// Local algorithm options
export const LOCAL_ALGO_OPTIONS = {
  cobyla: "COBYLA",
  bobyqa: "BOBYQA",
  newuoa: "NEWUOA",
};

// Warning thresholds
export const WARNING_THRESHOLDS = {
  population: {
    yellow: 3000,
    red: 30000,
  },
};

// Helper function to get default value for a parameter
export function getDefaultValue(
  paramName: keyof OptimizationDefaults,
): number | string | boolean {
  return OPTIMIZATION_DEFAULTS[paramName];
}

// Helper function to get limits for a parameter
export function getLimits(
  paramName: keyof OptimizationLimits,
): { min: number; max: number } | undefined {
  return OPTIMIZATION_LIMITS[paramName];
}

// Helper function to get step size for a parameter
export function getStepSize(
  paramName: keyof OptimizationSteps,
): number | undefined {
  return OPTIMIZATION_STEPS[paramName];
}
