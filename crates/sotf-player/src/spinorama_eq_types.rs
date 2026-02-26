//! Shared Spinorama EQ domain types used by both GPUI and TUI apps.

use serde::{Deserialize, Serialize};

/// Spinorama EQ workflow step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpinoramaStep {
    #[default]
    SelectSpeaker,
    Configure,
    Review,
    Export,
}

impl SpinoramaStep {
    pub fn all() -> &'static [SpinoramaStep] {
        &[
            SpinoramaStep::SelectSpeaker,
            SpinoramaStep::Configure,
            SpinoramaStep::Review,
            SpinoramaStep::Export,
        ]
    }

    pub fn index(&self) -> usize {
        match self {
            SpinoramaStep::SelectSpeaker => 0,
            SpinoramaStep::Configure => 1,
            SpinoramaStep::Review => 2,
            SpinoramaStep::Export => 3,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SpinoramaStep::SelectSpeaker => "Select",
            SpinoramaStep::Configure => "Configure",
            SpinoramaStep::Review => "Review",
            SpinoramaStep::Export => "Export",
        }
    }

    pub fn next(&self) -> Option<SpinoramaStep> {
        match self {
            SpinoramaStep::SelectSpeaker => Some(SpinoramaStep::Configure),
            SpinoramaStep::Configure => Some(SpinoramaStep::Review),
            SpinoramaStep::Review => Some(SpinoramaStep::Export),
            SpinoramaStep::Export => None,
        }
    }

    pub fn previous(&self) -> Option<SpinoramaStep> {
        match self {
            SpinoramaStep::SelectSpeaker => None,
            SpinoramaStep::Configure => Some(SpinoramaStep::SelectSpeaker),
            SpinoramaStep::Review => Some(SpinoramaStep::Configure),
            SpinoramaStep::Export => Some(SpinoramaStep::Review),
        }
    }
}

/// Optimization mode for Spinorama EQ
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SpinoramaOptimizationMode {
    #[default]
    FlatOnPir,
    SpeakerScore,
}

impl SpinoramaOptimizationMode {
    pub fn all() -> &'static [SpinoramaOptimizationMode] {
        &[
            SpinoramaOptimizationMode::FlatOnPir,
            SpinoramaOptimizationMode::SpeakerScore,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            SpinoramaOptimizationMode::FlatOnPir => "Target",
            SpinoramaOptimizationMode::SpeakerScore => "Score",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            SpinoramaOptimizationMode::FlatOnPir => "Flatten the Estimated In-Room Response curve",
            SpinoramaOptimizationMode::SpeakerScore => {
                "Optimize for Harman/Olive speaker preference score"
            }
        }
    }

    pub fn to_loss_string(&self) -> &'static str {
        match self {
            SpinoramaOptimizationMode::FlatOnPir => "speaker-flat",
            SpinoramaOptimizationMode::SpeakerScore => "speaker-score",
        }
    }
}

/// Target curve types for spinorama optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SpinoramaTargetCurve {
    OnAxis,
    ListeningWindow,
    #[default]
    EstimatedInRoom,
    EarlyReflections,
}

impl SpinoramaTargetCurve {
    pub fn all() -> &'static [SpinoramaTargetCurve] {
        &[
            SpinoramaTargetCurve::OnAxis,
            SpinoramaTargetCurve::ListeningWindow,
            SpinoramaTargetCurve::EstimatedInRoom,
            SpinoramaTargetCurve::EarlyReflections,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            SpinoramaTargetCurve::OnAxis => "ON (On-Axis)",
            SpinoramaTargetCurve::ListeningWindow => "LW (Listening Window)",
            SpinoramaTargetCurve::EstimatedInRoom => "PIR (In-Room)",
            SpinoramaTargetCurve::EarlyReflections => "ER (Early Reflections)",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            SpinoramaTargetCurve::OnAxis => "ON",
            SpinoramaTargetCurve::ListeningWindow => "LW",
            SpinoramaTargetCurve::EstimatedInRoom => "PIR",
            SpinoramaTargetCurve::EarlyReflections => "ER",
        }
    }

    pub fn api_name(&self) -> &'static str {
        match self {
            SpinoramaTargetCurve::OnAxis => "On Axis",
            SpinoramaTargetCurve::ListeningWindow => "Listening Window",
            SpinoramaTargetCurve::EstimatedInRoom => "Estimated In-Room Response",
            SpinoramaTargetCurve::EarlyReflections => "Early Reflections",
        }
    }
}

/// Optimizer configuration for Spinorama EQ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpinoramaOptimizerConfig {
    pub mode: SpinoramaOptimizationMode,
    pub target_curve: SpinoramaTargetCurve,
    pub algorithm: crate::room_eq_types::RoomEqAlgorithm,
    pub num_filters: usize,
    pub sample_rate: u32,
    pub fir_taps: usize,
    pub fir_phase: String,
    pub min_q: f64,
    pub max_q: f64,
    pub min_db: f64,
    pub max_db: f64,
    pub min_freq: f64,
    pub max_freq: f64,
    pub max_iter: usize,
    pub peq_model: String,
    pub population: usize,
    pub de_f: f64,
    pub de_cr: f64,
    pub strategy: String,
    pub refine: bool,
    pub local_algo: String,
    pub smooth: bool,
    pub smooth_n: usize,
    pub spacing_weight: f64,
    pub min_spacing_oct: f64,
    pub tolerance: f64,
    pub atolerance: f64,
    pub psychoacoustic: bool,
    /// Loss function: "flat", "flat-asymmetric", or "score"
    pub loss_function: String,
}

impl Default for SpinoramaOptimizerConfig {
    fn default() -> Self {
        Self {
            mode: SpinoramaOptimizationMode::FlatOnPir,
            target_curve: SpinoramaTargetCurve::default(),
            algorithm: crate::room_eq_types::RoomEqAlgorithm::DifferentialEvolution,
            num_filters: 5,
            sample_rate: 48000,
            fir_taps: 4096,
            fir_phase: "kirkeby".to_string(),
            min_q: 0.5,
            max_q: 6.0,
            min_db: -12.0,
            max_db: 4.0,
            min_freq: 60.0,
            max_freq: 16000.0,
            max_iter: 10000,
            peq_model: "pk".to_string(),
            population: 40,
            de_f: 0.8,
            de_cr: 0.9,
            strategy: "currenttobest1bin".to_string(),
            refine: false,
            local_algo: "cobyla".to_string(),
            smooth: false,
            smooth_n: 6,
            spacing_weight: 1.0,
            min_spacing_oct: 0.08,
            tolerance: 0.00001,
            atolerance: 0.00001,
            psychoacoustic: true,
            loss_function: "flat-asymmetric".to_string(),
        }
    }
}

/// Result of Spinorama EQ optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpinoramaEqResult {
    pub biquads: Vec<SpinoramaBiquad>,
    pub pre_score: f64,
    pub post_score: f64,
    pub original_response: Option<Vec<(f64, f64)>>,
    pub corrected_response: Option<Vec<(f64, f64)>>,
    pub target_response: Option<Vec<(f64, f64)>>,
}

/// Biquad filter for Spinorama EQ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpinoramaBiquad {
    pub filter_type: String,
    pub freq: f64,
    pub q: f64,
    pub db_gain: f64,
}
