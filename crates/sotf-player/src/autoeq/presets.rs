//! EQ optimization presets for non-expert users.
//!
//! Provides named parameter bundles that map a single user choice to a complete
//! optimizer configuration. Three tiers of UI detail control which parameters
//! are visible.

use serde::{Deserialize, Serialize};

use super::params::OptimizationParamsSerializable;

/// Which optimization workflow the user is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EqWorkflow {
    Headphone,
    Spinorama,
    RoomEq,
}

/// How much detail to show in the configuration UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DetailLevel {
    /// Preset selector + optimize button. No individual parameters.
    #[default]
    Simple,
    /// Curated parameter subset: goal, filter design, quality slider.
    Intermediate,
    /// Full parameter form for experts.
    Expert,
}

/// A named preset that maps to a complete parameter bundle.
#[derive(Debug, Clone)]
pub struct EqPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub workflow: EqWorkflow,
    /// None means "Custom" -- user controls all parameters.
    params: Option<PresetParams>,
}

#[derive(Debug, Clone)]
struct PresetParams {
    num_filters: usize,
    loss: &'static str,
    peq_model: &'static str,
    population: usize,
    maxeval: usize,
    refine: bool,
    min_freq: f64,
    max_freq: f64,
    min_db: f64,
    max_db: f64,
    min_q: f64,
    max_q: f64,
    smooth: bool,
    smooth_n: usize,
}

impl EqPreset {
    /// Returns true if this is the "Custom" preset (no fixed params).
    pub fn is_custom(&self) -> bool {
        self.params.is_none()
    }

    /// Apply this preset's parameters onto a serializable config.
    /// Returns None for the Custom preset (caller keeps existing params).
    pub fn apply(&self) -> Option<OptimizationParamsSerializable> {
        let p = self.params.as_ref()?;
        let base = match self.workflow {
            EqWorkflow::Headphone => autoeq::Args::headphone_defaults(),
            EqWorkflow::Spinorama => autoeq::Args::speaker_defaults(),
            EqWorkflow::RoomEq => autoeq::Args::roomeq_defaults(),
        };
        let mut params = OptimizationParamsSerializable::from(&base);
        params.num_filters = p.num_filters;
        params.loss = p.loss.to_string();
        params.peq_model = p.peq_model.to_string();
        params.population = p.population;
        params.maxeval = p.maxeval;
        params.refine = p.refine;
        params.min_freq = p.min_freq;
        params.max_freq = p.max_freq;
        params.min_db = p.min_db;
        params.max_db = p.max_db;
        params.min_q = p.min_q;
        params.max_q = p.max_q;
        params.smooth = p.smooth;
        params.smooth_n = p.smooth_n;
        Some(params)
    }
}

// ---------------------------------------------------------------------------
// Headphone presets
// ---------------------------------------------------------------------------

pub const HEADPHONE_PRESETS: &[EqPreset] = &[
    EqPreset {
        id: "quick",
        name: "Quick Fix",
        description: "Fast correction with 5 filters. Good for a quick improvement.",
        workflow: EqWorkflow::Headphone,
        params: Some(PresetParams {
            num_filters: 5,
            loss: "headphone-score",
            peq_model: "pk",
            population: 40,
            maxeval: 2000,
            refine: false,
            min_freq: 20.0,
            max_freq: 20000.0,
            min_db: -12.0,
            max_db: 6.0,
            min_q: 0.5,
            max_q: 6.0,
            smooth: true,
            smooth_n: 1,
        }),
    },
    EqPreset {
        id: "balanced",
        name: "Balanced",
        description: "Good balance of quality and speed. Recommended for most headphones.",
        workflow: EqWorkflow::Headphone,
        params: Some(PresetParams {
            num_filters: 7,
            loss: "headphone-score",
            peq_model: "pk",
            population: 80,
            maxeval: 5000,
            refine: true,
            min_freq: 20.0,
            max_freq: 20000.0,
            min_db: -12.0,
            max_db: 6.0,
            min_q: 0.5,
            max_q: 6.0,
            smooth: true,
            smooth_n: 1,
        }),
    },
    EqPreset {
        id: "max-quality",
        name: "Maximum Quality",
        description: "Best possible correction with shelves. Slower optimization.",
        workflow: EqWorkflow::Headphone,
        params: Some(PresetParams {
            num_filters: 10,
            loss: "headphone-score",
            peq_model: "ls-pk-hs",
            population: 200,
            maxeval: 20000,
            refine: true,
            min_freq: 20.0,
            max_freq: 20000.0,
            min_db: -12.0,
            max_db: 6.0,
            min_q: 0.5,
            max_q: 6.0,
            smooth: true,
            smooth_n: 1,
        }),
    },
    EqPreset {
        id: "custom",
        name: "Custom",
        description: "Full control over all optimization parameters.",
        workflow: EqWorkflow::Headphone,
        params: None,
    },
];

// ---------------------------------------------------------------------------
// Spinorama presets
// ---------------------------------------------------------------------------

pub const SPINORAMA_PRESETS: &[EqPreset] = &[
    EqPreset {
        id: "quick",
        name: "Quick",
        description: "Fast correction targeting flat in-room response.",
        workflow: EqWorkflow::Spinorama,
        params: Some(PresetParams {
            num_filters: 5,
            loss: "speaker-flat",
            peq_model: "pk",
            population: 40,
            maxeval: 2000,
            refine: false,
            min_freq: 20.0,
            max_freq: 20000.0,
            min_db: -12.0,
            max_db: 6.0,
            min_q: 0.5,
            max_q: 6.0,
            smooth: true,
            smooth_n: 1,
        }),
    },
    EqPreset {
        id: "balanced",
        name: "Balanced",
        description: "Good balance of quality and speed. Targets flat in-room response.",
        workflow: EqWorkflow::Spinorama,
        params: Some(PresetParams {
            num_filters: 7,
            loss: "speaker-flat",
            peq_model: "pk",
            population: 80,
            maxeval: 5000,
            refine: true,
            min_freq: 20.0,
            max_freq: 20000.0,
            min_db: -12.0,
            max_db: 6.0,
            min_q: 0.5,
            max_q: 6.0,
            smooth: true,
            smooth_n: 1,
        }),
    },
    EqPreset {
        id: "score",
        name: "Score Optimized",
        description: "Optimizes for listener preference rating. Allows a natural bass shelf.",
        workflow: EqWorkflow::Spinorama,
        params: Some(PresetParams {
            num_filters: 7,
            loss: "speaker-score",
            peq_model: "pk",
            population: 100,
            maxeval: 10000,
            refine: true,
            min_freq: 20.0,
            max_freq: 20000.0,
            min_db: -12.0,
            max_db: 6.0,
            min_q: 0.5,
            max_q: 6.0,
            smooth: true,
            smooth_n: 1,
        }),
    },
    EqPreset {
        id: "custom",
        name: "Custom",
        description: "Full control over all optimization parameters.",
        workflow: EqWorkflow::Spinorama,
        params: None,
    },
];

// ---------------------------------------------------------------------------
// Room EQ presets
// ---------------------------------------------------------------------------

pub const ROOMEQ_PRESETS: &[EqPreset] = &[
    EqPreset {
        id: "quick",
        name: "Quick Correction",
        description: "Fast modal correction below 500 Hz. Best for taming room resonances.",
        workflow: EqWorkflow::RoomEq,
        params: Some(PresetParams {
            num_filters: 5,
            loss: "speaker-flat",
            peq_model: "pk",
            population: 50,
            maxeval: 5000,
            refine: false,
            min_freq: 20.0,
            max_freq: 500.0,
            min_db: -12.0,
            max_db: 4.0,
            min_q: 0.5,
            max_q: 6.0,
            smooth: false,
            smooth_n: 1,
        }),
    },
    EqPreset {
        id: "full-range",
        name: "Full Range",
        description: "Correction up to 1600 Hz with psychoacoustic weighting.",
        workflow: EqWorkflow::RoomEq,
        params: Some(PresetParams {
            num_filters: 7,
            loss: "speaker-flat",
            peq_model: "pk",
            population: 50,
            maxeval: 20000,
            refine: false,
            min_freq: 20.0,
            max_freq: 1600.0,
            min_db: -12.0,
            max_db: 4.0,
            min_q: 0.5,
            max_q: 6.0,
            smooth: false,
            smooth_n: 1,
        }),
    },
    EqPreset {
        id: "audiophile",
        name: "Audiophile",
        description: "Maximum quality with shelves, refinement, and perceptual optimization.",
        workflow: EqWorkflow::RoomEq,
        params: Some(PresetParams {
            num_filters: 10,
            loss: "speaker-flat",
            peq_model: "ls-pk-hs",
            population: 100,
            maxeval: 50000,
            refine: true,
            min_freq: 20.0,
            max_freq: 1600.0,
            min_db: -12.0,
            max_db: 4.0,
            min_q: 0.5,
            max_q: 6.0,
            smooth: false,
            smooth_n: 1,
        }),
    },
    EqPreset {
        id: "custom",
        name: "Custom",
        description: "Full control over all optimization parameters.",
        workflow: EqWorkflow::RoomEq,
        params: None,
    },
];

/// Get all presets for a given workflow.
pub fn presets_for(workflow: EqWorkflow) -> &'static [EqPreset] {
    match workflow {
        EqWorkflow::Headphone => HEADPHONE_PRESETS,
        EqWorkflow::Spinorama => SPINORAMA_PRESETS,
        EqWorkflow::RoomEq => ROOMEQ_PRESETS,
    }
}

/// Find a preset by id within a workflow.
pub fn find_preset(workflow: EqWorkflow, id: &str) -> Option<&'static EqPreset> {
    presets_for(workflow).iter().find(|p| p.id == id)
}

/// Get the default preset id for a workflow.
pub fn default_preset_id(workflow: EqWorkflow) -> &'static str {
    match workflow {
        EqWorkflow::Headphone => "balanced",
        EqWorkflow::Spinorama => "balanced",
        EqWorkflow::RoomEq => "full-range",
    }
}

/// Map a quality level (0.0 = fast, 1.0 = maximum) to population and maxeval.
/// Used by the Intermediate mode quality slider.
pub fn quality_to_optimizer_params(quality: f32) -> (usize, usize) {
    let quality = quality.clamp(0.0, 1.0);
    // Exponential interpolation for perceptually linear slider
    let population = lerp_exp(30.0, 300.0, quality) as usize;
    let maxeval = lerp_exp(1000.0, 50000.0, quality) as usize;
    (population, maxeval)
}

/// Inverse of `quality_to_optimizer_params`: map a population value back to 0.0-1.0.
pub fn population_to_quality(population: usize) -> f32 {
    const POP_MIN: f32 = 30.0;
    const POP_MAX: f32 = 300.0;
    if population as f32 <= POP_MIN {
        0.0
    } else if population as f32 >= POP_MAX {
        1.0
    } else {
        let log_min = POP_MIN.ln();
        let log_max = POP_MAX.ln();
        ((population as f32).ln() - log_min) / (log_max - log_min)
    }
}

/// User-facing label for a quality level.
pub fn quality_label(quality: f32) -> &'static str {
    if quality < 0.2 {
        "Fast (seconds)"
    } else if quality < 0.5 {
        "Balanced (minutes)"
    } else if quality < 0.8 {
        "Thorough (slow)"
    } else {
        "Maximum (very slow)"
    }
}

/// Preset dropdown options as `(id, label)` pairs for UI consumption.
pub fn preset_options(workflow: EqWorkflow) -> Vec<(&'static str, &'static str)> {
    presets_for(workflow)
        .iter()
        .map(|p| (p.id, p.name))
        .collect()
}

// Exponential interpolation between min and max.
fn lerp_exp(min: f32, max: f32, t: f32) -> f32 {
    let log_min = min.ln();
    let log_max = max.ln();
    (log_min + t * (log_max - log_min)).exp()
}

/// Validation warnings for parameter values.
/// Returns a warning message if the value is outside recommended ranges.
pub fn field_warning(field: &str, value: f64) -> Option<&'static str> {
    match field {
        "num_filters" if value > 12.0 => Some("Many filters may cause audible ringing"),
        "max_db" if value > 6.0 => Some("High boost risks distortion"),
        "max_q" if value > 8.0 => Some("Very narrow filters may sound unnatural"),
        "min_freq" if value < 20.0 => Some("Below audible range"),
        "max_freq" if value > 20000.0 => Some("Above audible range"),
        _ => None,
    }
}

/// Inline hint text for parameter fields (Intermediate mode).
pub fn field_hint(field: &str) -> Option<&'static str> {
    match field {
        "num_filters" => Some("5-7 for quick results, 10+ for surgical precision"),
        "min_freq" | "max_freq" => {
            Some("Narrow to the problem region for faster, better results")
        }
        "quality" => Some("Higher = better corrections, longer optimization"),
        "loss" => Some("Flat Match for accuracy, Listener Preference for enjoyment"),
        "peq_model" => Some("Peaks Only is simplest; Shelves + Peaks is most flexible"),
        "min_db" | "max_db" => Some("Smaller range = faster convergence"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_presets_have_valid_ids() {
        for workflow in [EqWorkflow::Headphone, EqWorkflow::Spinorama, EqWorkflow::RoomEq] {
            let presets = presets_for(workflow);
            assert!(!presets.is_empty());
            // Last preset must be "custom"
            assert_eq!(presets.last().unwrap().id, "custom");
            assert!(presets.last().unwrap().is_custom());
        }
    }

    #[test]
    fn test_default_preset_exists() {
        for workflow in [EqWorkflow::Headphone, EqWorkflow::Spinorama, EqWorkflow::RoomEq] {
            let id = default_preset_id(workflow);
            assert!(find_preset(workflow, id).is_some(), "default preset '{id}' not found");
        }
    }

    #[test]
    fn test_apply_preset_produces_valid_params() {
        for workflow in [EqWorkflow::Headphone, EqWorkflow::Spinorama, EqWorkflow::RoomEq] {
            for preset in presets_for(workflow) {
                if let Some(params) = preset.apply() {
                    assert!(params.num_filters > 0);
                    assert!(params.population > 0);
                    assert!(params.maxeval > 0);
                    assert!(params.min_freq < params.max_freq);
                }
            }
        }
    }

    #[test]
    fn test_custom_preset_returns_none() {
        let custom = find_preset(EqWorkflow::Headphone, "custom").unwrap();
        assert!(custom.apply().is_none());
    }

    #[test]
    fn test_quality_slider_bounds() {
        let (pop_low, eval_low) = quality_to_optimizer_params(0.0);
        let (pop_high, eval_high) = quality_to_optimizer_params(1.0);
        assert!(pop_low < pop_high);
        assert!(eval_low < eval_high);
        assert_eq!(pop_low, 30);
        assert_eq!(pop_high, 300);
    }

    #[test]
    fn test_field_warnings() {
        assert!(field_warning("num_filters", 15.0).is_some());
        assert!(field_warning("num_filters", 7.0).is_none());
        assert!(field_warning("max_db", 8.0).is_some());
        assert!(field_warning("max_db", 3.0).is_none());
    }
}
