//! Simple Wizard preset types for Room EQ.
//!
//! These types encode the guided "Simple Wizard" choices that map to sane
//! optimizer defaults.  They live in autoeq (not the player crate) because
//! the knowledge of what constitutes a good default is optimizer domain
//! knowledge.

use super::config::{
    DecomposedCorrectionSerdeConfig, OptimizerConfig, ProcessingMode, TargetResponseConfig,
    TargetShape,
};

fn canonical_multi_measurement_strategy(strategy: &str) -> Option<&'static str> {
    let normalized = strategy
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-'], "_")
        .replace(['(', ')'], "");
    match normalized.as_str() {
        "average" | "average_rms" => Some("average"),
        "weighted_sum" => Some("weighted_sum"),
        "minimax" | "minmax" | "minimax_worst_case" => Some("minimax"),
        "variance_penalized" | "minimize_variance" | "variance" => Some("variance_penalized"),
        "spatial_robustness" => Some("spatial_robustness"),
        "minimax_uncertainty" | "minimax_bootstrap_uncertainty" => Some("minimax_uncertainty"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Listening distance tier used by the Simple Wizard to set the target
/// curve family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum SpeakerTier {
    NearField,
    #[default]
    MidField,
    FarField,
}

impl SpeakerTier {
    pub fn label(&self) -> &'static str {
        match self {
            Self::NearField => "Near-field (<1.5m)",
            Self::MidField => "Mid-field (1.5–3m)",
            Self::FarField => "Far-field (>3m)",
        }
    }

    pub fn all() -> &'static [SpeakerTier] {
        &[Self::NearField, Self::MidField, Self::FarField]
    }
}

/// Simple-mode loss function choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum SimpleLossChoice {
    #[default]
    Flat,
    Epa,
}

impl SimpleLossChoice {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Flat => "Flat (minimize deviation)",
            Self::Epa => "EPA (perceptual quality)",
        }
    }
}

/// Simple-mode processing capability choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum SimpleProcessingChoice {
    #[default]
    Iir,
    MixedPhase,
}

impl SimpleProcessingChoice {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Iir => "IIR (low latency)",
            Self::MixedPhase => "Mixed Phase (best quality)",
        }
    }
}

/// Simple-mode crossover choice (shown for 2.1+ configs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum SimpleCrossoverChoice {
    #[default]
    Lr24,
    Lr48,
}

impl SimpleCrossoverChoice {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Lr24 => "Linkwitz-Riley 24 dB/oct",
            Self::Lr48 => "Linkwitz-Riley 48 dB/oct",
        }
    }
}

// ---------------------------------------------------------------------------
// SimplePresetConfig
// ---------------------------------------------------------------------------

/// Collected choices from the Simple Wizard's Configure step.
///
/// [`to_optimizer_config`](Self::to_optimizer_config) translates these
/// directly into an [`OptimizerConfig`] so the optimizer sees a fully
/// populated configuration regardless of which wizard path the user took.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SimplePresetConfig {
    pub target: SpeakerTier,
    pub loss: SimpleLossChoice,
    pub processing: SimpleProcessingChoice,
    /// Only meaningful for 2.1+ configs.
    pub crossover: SimpleCrossoverChoice,
    /// Only meaningful for 5.0+ or >2 subs.
    pub bass_management: String,
    /// Multi-position strategy (only when multi-position data detected).
    pub multi_position_strategy: String,
}

impl SimplePresetConfig {
    /// Produce a backend [`OptimizerConfig`] directly from the Simple
    /// Wizard choices.
    ///
    /// The returned config uses sensible defaults for every field the
    /// Simple Wizard doesn't expose (num_filters, population, tolerances,
    /// etc.).  Callers can further mutate the result before passing it to
    /// [`optimize_room`](crate::roomeq::optimize_room).
    pub fn to_optimizer_config(&self) -> OptimizerConfig {
        let processing_mode = match self.processing {
            SimpleProcessingChoice::Iir => ProcessingMode::LowLatency,
            SimpleProcessingChoice::MixedPhase => ProcessingMode::MixedPhase,
        };

        let loss_type = match self.loss {
            SimpleLossChoice::Flat => "flat".to_string(),
            SimpleLossChoice::Epa => "epa".to_string(),
        };

        // Target response: use measurement's own broadband slope.
        let target_response = Some(TargetResponseConfig {
            shape: TargetShape::FromMeasurement,
            slope_db_per_octave: 0.0,
            broadband_precorrection: true,
            ..Default::default()
        });

        // Schroeder split: enable when bass management is configured or
        // LR48 crossover is selected.
        let schroeder_split =
            if !self.bass_management.is_empty() || self.crossover == SimpleCrossoverChoice::Lr48 {
                Some(super::config::SchroederSplitConfig {
                    enabled: true,
                    ..Default::default()
                })
            } else {
                None
            };

        // Multi-position measurement strategy
        let multi_measurement = if !self.multi_position_strategy.is_empty() {
            let strategy_key = canonical_multi_measurement_strategy(&self.multi_position_strategy)
                .unwrap_or_else(|| {
                    log::warn!(
                        "Unknown multi_position_strategy '{}'; falling back to average",
                        self.multi_position_strategy
                    );
                    "average"
                });
            let strategy = match strategy_key {
                "average" => super::config::MultiMeasurementStrategy::Average,
                "weighted_sum" => super::config::MultiMeasurementStrategy::WeightedSum,
                "minimax" => super::config::MultiMeasurementStrategy::Minimax,
                "variance_penalized" => super::config::MultiMeasurementStrategy::VariancePenalized,
                "spatial_robustness" => super::config::MultiMeasurementStrategy::SpatialRobustness,
                "minimax_uncertainty" => {
                    super::config::MultiMeasurementStrategy::MinimaxUncertainty
                }
                _ => super::config::MultiMeasurementStrategy::Average,
            };
            Some(super::config::MultiMeasurementConfig {
                strategy,
                weights: None,
                variance_lambda: 0.5,
                spatial_robustness: None,
                bootstrap_uncertainty: None,
            })
        } else {
            None
        };

        OptimizerConfig {
            processing_mode,
            loss_type,
            target_response,
            schroeder_split,
            multi_measurement,
            // Sane defaults for fields not exposed in Simple mode
            num_filters: 7,
            algorithm: "autoeq:cmaes".to_string(),
            population: 300,
            max_iter: 50_000,
            min_freq: 20.0,
            max_freq: 1600.0,
            min_db: -12.0,
            max_db: 4.0,
            min_q: 0.5,
            max_q: 6.0,
            peq_model: "pk".to_string(),
            tolerance: 1e-5,
            atolerance: 1e-5,
            psychoacoustic: true,
            asymmetric_loss: true,
            refine: true,
            local_algo: "cobyla".to_string(),
            decomposed_correction: Some(DecomposedCorrectionSerdeConfig::default()),
            ..OptimizerConfig::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_preset_default_produces_valid_config() {
        let preset = SimplePresetConfig::default();
        let config = preset.to_optimizer_config();
        assert_eq!(config.processing_mode, ProcessingMode::LowLatency);
        assert_eq!(config.loss_type, "flat");
        assert_eq!(config.algorithm, "autoeq:cmaes");
        assert_eq!(config.num_filters, 7);
        assert!(config.target_response.is_some());
        assert!(config.schroeder_split.is_none());
        assert!(config.multi_measurement.is_none());
    }

    #[test]
    fn test_simple_preset_mixed_phase_epa() {
        let preset = SimplePresetConfig {
            processing: SimpleProcessingChoice::MixedPhase,
            loss: SimpleLossChoice::Epa,
            ..Default::default()
        };
        let config = preset.to_optimizer_config();
        assert_eq!(config.processing_mode, ProcessingMode::MixedPhase);
        assert_eq!(config.loss_type, "epa");
    }

    #[test]
    fn test_simple_preset_with_crossover_enables_schroeder() {
        let preset = SimplePresetConfig {
            crossover: SimpleCrossoverChoice::Lr48,
            ..Default::default()
        };
        let config = preset.to_optimizer_config();
        assert!(config.schroeder_split.is_some());
    }

    #[test]
    fn test_simple_preset_with_bass_management_enables_schroeder() {
        let preset = SimplePresetConfig {
            bass_management: "some_config".to_string(),
            ..Default::default()
        };
        let config = preset.to_optimizer_config();
        assert!(config.schroeder_split.is_some());
    }

    #[test]
    fn test_simple_preset_with_multi_position() {
        let preset = SimplePresetConfig {
            multi_position_strategy: "average".to_string(),
            ..Default::default()
        };
        let config = preset.to_optimizer_config();
        assert!(config.multi_measurement.is_some());
    }

    #[test]
    fn test_simple_preset_accepts_multi_position_display_label() {
        let preset = SimplePresetConfig {
            multi_position_strategy: "Minimize Variance".to_string(),
            ..Default::default()
        };
        let config = preset.to_optimizer_config();
        assert_eq!(
            config.multi_measurement.unwrap().strategy,
            super::super::config::MultiMeasurementStrategy::VariancePenalized
        );
    }
}
