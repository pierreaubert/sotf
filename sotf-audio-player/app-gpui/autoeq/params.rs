//! Optimization parameter types and defaults
//!
//! Centralized optimization constants reused for headphone, room EQ, and speaker optimization

use serde::{Deserialize, Serialize};

/// Complete set of optimization parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationParams {
    // EQ Design Parameters
    pub num_filters: usize,
    pub sample_rate: u32,
    pub min_db: f64,
    pub max_db: f64,
    pub min_q: f64,
    pub max_q: f64,
    pub min_freq: f64,
    pub max_freq: f64,
    pub peq_model: String,
    pub min_spacing_oct: f64,
    pub spacing_weight: f64,

    // Algorithm Parameters
    pub algo: String,
    pub population: usize,
    pub maxeval: usize,

    // DE-specific Parameters
    pub de_f: f64,
    pub de_cr: f64,
    pub strategy: String,
    pub adaptive_weight_f: f64,
    pub adaptive_weight_cr: f64,

    // Tolerance Parameters
    pub tolerance: f64,
    pub abs_tolerance: f64,

    // Refinement Parameters
    pub refine: bool,
    pub local_algo: String,

    // Smoothing Parameters
    pub smooth: bool,
    pub smooth_n: usize,

    // Loss Function
    pub loss: String,

    // Curve Selection (for speakers)
    pub curve_name: String,
}

impl Default for OptimizationParams {
    fn default() -> Self {
        Self {
            // Core EQ Parameters
            num_filters: 5,
            sample_rate: 48000,
            min_db: -12.0,
            max_db: 12.0,
            min_q: 0.5,
            max_q: 10.0,
            min_freq: 20.0,
            max_freq: 20000.0,
            peq_model: "pk".to_string(),
            min_spacing_oct: 0.5,
            spacing_weight: 20.0,

            // Algorithm Parameters
            algo: "autoeq:de".to_string(),
            population: 50,
            maxeval: 2000,

            // DE-specific Parameters
            de_f: 0.8,
            de_cr: 0.9,
            strategy: "currenttobest1bin".to_string(),
            adaptive_weight_f: 0.8,
            adaptive_weight_cr: 0.7,

            // Tolerance Parameters
            tolerance: 1e-3,
            abs_tolerance: 1e-4,

            // Refinement Parameters
            refine: false,
            local_algo: "cobyla".to_string(),

            // Smoothing Parameters
            smooth: true,
            smooth_n: 1,

            // Loss Function
            loss: "speaker-flat".to_string(),

            // Curve Selection
            curve_name: "Listening Window".to_string(),
        }
    }
}

impl OptimizationParams {
    /// Get defaults for headphone optimization
    pub fn headphone_defaults() -> Self {
        Self {
            loss: "headphone-score".to_string(),
            num_filters: 7,
            min_db: -12.0,
            max_db: 12.0,
            min_q: 0.5,
            max_q: 10.0,
            ..Default::default()
        }
    }

    /// Get defaults for speaker optimization
    pub fn speaker_defaults() -> Self {
        Self {
            loss: "speaker-flat".to_string(),
            curve_name: "Listening Window".to_string(),
            ..Default::default()
        }
    }

    /// Get defaults for room EQ optimization
    pub fn roomeq_defaults() -> Self {
        Self {
            loss: "speaker-flat".to_string(),
            num_filters: 10,
            min_freq: 20.0,
            max_freq: 500.0, // Room EQ typically focuses on low frequencies
            ..Default::default()
        }
    }
}

/// Parameter limits for UI validation
pub struct ParamLimits {
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

impl ParamLimits {
    pub const NUM_FILTERS: Self = Self {
        min: 1.0,
        max: 20.0,
        step: 1.0,
    };
    pub const SAMPLE_RATE: Self = Self {
        min: 8000.0,
        max: 192000.0,
        step: 1000.0,
    };
    pub const DB: Self = Self {
        min: -25.0,
        max: 25.0,
        step: 0.5,
    };
    pub const Q: Self = Self {
        min: 0.1,
        max: 10.0,
        step: 0.1,
    };
    pub const FREQUENCY: Self = Self {
        min: 20.0,
        max: 20000.0,
        step: 10.0,
    };
    pub const POPULATION: Self = Self {
        min: 10.0,
        max: 10000.0,
        step: 10.0,
    };
    pub const MAXEVAL: Self = Self {
        min: 100.0,
        max: 100000.0,
        step: 100.0,
    };
    pub const DE_FACTOR: Self = Self {
        min: 0.0,
        max: 2.0,
        step: 0.1,
    };
    pub const DE_CR: Self = Self {
        min: 0.0,
        max: 1.0,
        step: 0.1,
    };
    pub const SPACING_OCT: Self = Self {
        min: 0.01,
        max: 10.0,
        step: 0.1,
    };
    pub const SPACING_WEIGHT: Self = Self {
        min: 0.0,
        max: 1000.0,
        step: 1.0,
    };
}

/// Algorithm options for dropdown
pub const ALGORITHM_OPTIONS: &[(&str, &str)] = &[
    ("autoeq:de", "Auto DE (Recommended)"),
    ("nlopt:isres", "NLOPT ISRES"),
    ("nlopt:ags", "NLOPT AGS"),
    ("nlopt:cobyla", "NLOPT COBYLA"),
    ("nlopt:bobyqa", "NLOPT BOBYQA"),
    ("nlopt:neldermead", "NLOPT Nelder-Mead"),
];

/// DE strategy options
pub const DE_STRATEGY_OPTIONS: &[(&str, &str)] = &[
    ("currenttobest1bin", "Current-to-Best/1/Bin (Recommended)"),
    ("rand1bin", "Rand/1/Bin"),
    ("best1bin", "Best/1/Bin"),
    ("rand2bin", "Rand/2/Bin"),
    ("randtobest1bin", "Rand-to-Best/1/Bin"),
    ("adaptivebin", "Adaptive/Bin (Experimental)"),
];

/// PEQ model options
pub const PEQ_MODEL_OPTIONS: &[(&str, &str)] = &[
    ("pk", "PK - All Peak Filters"),
    ("hp-pk", "HP+PK - Highpass + Peaks"),
    ("hp-pk-lp", "HP+PK+LP - Highpass + Peaks + Lowpass"),
    ("ls-pk", "LS+PK - Low Shelf + Peaks"),
    ("ls-pk-hs", "LS+PK+HS - Low Shelf + Peaks + High Shelf"),
    ("free-pk-free", "Free+PK+Free - Flexible ends, peaks middle"),
    ("free", "Free - All filters flexible"),
];

/// Loss function options for speakers
pub const SPEAKER_LOSS_OPTIONS: &[(&str, &str)] = &[
    ("speaker-flat", "Flat Response"),
    ("speaker-score", "Harman Score"),
];

/// Loss function options for headphones
pub const HEADPHONE_LOSS_OPTIONS: &[(&str, &str)] = &[
    ("headphone-flat", "Flat Response"),
    ("headphone-score", "Harman Score"),
];

/// Curve name options for speakers
pub const CURVE_NAME_OPTIONS: &[(&str, &str)] = &[
    ("Listening Window", "Listening Window"),
    ("On Axis", "On Axis"),
    ("Early Reflections", "Early Reflections"),
    ("Sound Power", "Sound Power"),
    ("Estimated In-Room Response", "Estimated In-Room Response"),
];

/// Local refinement algorithm options
pub const LOCAL_ALGO_OPTIONS: &[(&str, &str)] = &[
    ("cobyla", "COBYLA"),
    ("bobyqa", "BOBYQA"),
    ("newuoa", "NEWUOA"),
];

/// EQ export format options
pub const EQ_EXPORT_FORMAT_OPTIONS: &[(&str, &str, &str)] = &[
    ("json", "JSON", ".json"),
    ("apo", "EqualizerAPO", ".txt"),
    ("rme-channel", "RME TotalMix (Channel)", ".xml"),
    ("rme-room", "RME TotalMix (Room)", ".xml"),
    ("aupreset", "Apple AUNBandEQ", ".aupreset"),
];

/// Get file extension for export format
pub fn get_export_extension(format: &str) -> &'static str {
    EQ_EXPORT_FORMAT_OPTIONS
        .iter()
        .find(|(id, _, _)| *id == format)
        .map(|(_, _, ext)| *ext)
        .unwrap_or(".json")
}
