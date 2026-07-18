use super::default::default_bo_acquisition;
use super::misc::canonical_multi_measurement_strategy;
use super::room_eq_optimizer_config::RoomEqOptimizerConfig;
use crate::ReleaseChannel;
pub use autoeq::roomeq::{
    SimpleCrossoverChoice, SimpleLossChoice, SimplePresetConfig, SimpleProcessingChoice,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Speaker layouts supported by the RoomEQ beginner workflow.
///
/// The optimizer and graph builder continue to own DSP routing. This type only
/// makes the beginner workflow's channel-role contract explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoomEqEasyLayout {
    #[default]
    Stereo20,
    Stereo21,
    Surround51,
}

impl RoomEqEasyLayout {
    pub const ALL: [Self; 3] = [Self::Stereo20, Self::Stereo21, Self::Surround51];

    pub fn label(self) -> &'static str {
        match self {
            Self::Stereo20 => "2.0",
            Self::Stereo21 => "2.1",
            Self::Surround51 => "5.1",
        }
    }

    pub fn expected_roles(self) -> &'static [&'static str] {
        match self {
            Self::Stereo20 => &["FL", "FR"],
            Self::Stereo21 => &["FL", "FR", "LFE"],
            Self::Surround51 => &["FL", "FR", "C", "LFE", "SL", "SR"],
        }
    }

    pub fn uses_bass_management(self) -> bool {
        !matches!(self, Self::Stereo20)
    }

    /// Apply deterministic defaults when the user selects a layout.
    pub fn configure_preset_defaults(self, preset: &mut SimplePresetConfig) {
        preset.crossover = SimpleCrossoverChoice::Lr24;
        if self.uses_bass_management() {
            preset.bass_management = "Standard".to_string();
        } else {
            preset.bass_management.clear();
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Stereo20 => Self::Stereo21,
            Self::Stereo21 => Self::Surround51,
            Self::Surround51 => Self::Stereo20,
        }
    }
}

/// A measurement set does not match the beginner layout selected by the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomEqEasyLayoutError {
    pub layout: RoomEqEasyLayout,
    pub missing_roles: Vec<&'static str>,
    pub unexpected_channels: Vec<String>,
    pub duplicate_roles: Vec<&'static str>,
}

impl fmt::Display for RoomEqEasyLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} layout requires channels {}",
            self.layout.label(),
            self.layout.expected_roles().join(", ")
        )?;
        if !self.missing_roles.is_empty() {
            write!(f, "; missing {}", self.missing_roles.join(", "))?;
        }
        if !self.unexpected_channels.is_empty() {
            write!(
                f,
                "; unrecognized or extra {}",
                self.unexpected_channels.join(", ")
            )?;
        }
        if !self.duplicate_roles.is_empty() {
            write!(f, "; duplicate {}", self.duplicate_roles.join(", "))?;
        }
        Ok(())
    }
}

impl std::error::Error for RoomEqEasyLayoutError {}

fn canonical_easy_channel_role(name: &str) -> Option<&'static str> {
    let normalized = name
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect::<String>();

    match normalized.as_str() {
        "L" | "FL" | "LEFT" | "FRONTLEFT" => Some("FL"),
        "R" | "FR" | "RIGHT" | "FRONTRIGHT" => Some("FR"),
        "C" | "FC" | "CENTER" | "CENTRE" | "FRONTCENTER" | "FRONTCENTRE" => Some("C"),
        "LFE" | "LFE1" | "SUB" | "SUB1" | "SW" | "SW1" | "SUBWOOFER" => Some("LFE"),
        "SL" | "LS" | "BL" | "RL" | "LEFTSURROUND" | "SURROUNDLEFT" | "REARLEFT" => Some("SL"),
        "SR" | "RS" | "BR" | "RR" | "RIGHTSURROUND" | "SURROUNDRIGHT" | "REARRIGHT" => Some("SR"),
        _ => None,
    }
}

pub fn validate_room_eq_easy_layout<I, S>(
    layout: RoomEqEasyLayout,
    channel_names: I,
) -> Result<(), RoomEqEasyLayoutError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let expected = layout
        .expected_roles()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut role_counts = BTreeMap::<&'static str, usize>::new();
    let mut unexpected_channels = Vec::new();

    for name in channel_names {
        let name = name.as_ref();
        match canonical_easy_channel_role(name) {
            Some(role) if expected.contains(role) => {
                *role_counts.entry(role).or_default() += 1;
            }
            _ => unexpected_channels.push(name.to_string()),
        }
    }

    let missing_roles = expected
        .iter()
        .copied()
        .filter(|role| !role_counts.contains_key(role))
        .collect::<Vec<_>>();
    let duplicate_roles = role_counts
        .into_iter()
        .filter_map(|(role, count)| (count > 1).then_some(role))
        .collect::<Vec<_>>();

    if missing_roles.is_empty() && unexpected_channels.is_empty() && duplicate_roles.is_empty() {
        Ok(())
    } else {
        Err(RoomEqEasyLayoutError {
            layout,
            missing_roles,
            unexpected_channels,
            duplicate_roles,
        })
    }
}

/// Validate and apply a beginner layout and preset atomically.
pub fn apply_room_eq_easy_layout<I, S>(
    layout: RoomEqEasyLayout,
    channel_names: I,
    preset: &mut SimplePresetConfig,
    config: &mut RoomEqOptimizerConfig,
) -> Result<(), RoomEqEasyLayoutError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    validate_room_eq_easy_layout(layout, channel_names)?;

    let mut next_preset = preset.clone();
    if layout.uses_bass_management() {
        if next_preset.bass_management.is_empty() {
            next_preset.bass_management = "Standard".to_string();
        }
    } else {
        next_preset.crossover = SimpleCrossoverChoice::Lr24;
        next_preset.bass_management.clear();
    }

    let mut next_config = config.clone();
    apply_simple_preset(&next_preset, &mut next_config);
    if !layout.uses_bass_management() {
        next_config.schroeder_split.enabled = false;
    }

    *preset = next_preset;
    *config = next_config;
    Ok(())
}

/// Apply the user's Simple Wizard choices to a flat UI optimizer config.
///
/// Fields not controlled by the preset keep their current values so the
/// user doesn't lose any manual tuning done in a previous Full Wizard
/// session.  This is the "mutate in place" path used when the full wizard
/// needs to incorporate simple-mode choices into an existing config.
pub fn apply_simple_preset(preset: &SimplePresetConfig, config: &mut RoomEqOptimizerConfig) {
    // Processing mode
    config.mode = match preset.processing {
        SimpleProcessingChoice::Iir => RoomEqOptimizationMode::Iir,
        SimpleProcessingChoice::MixedPhase => RoomEqOptimizationMode::MixedPhase,
    };

    // Loss function
    config.loss_type = match preset.loss {
        SimpleLossChoice::Flat => "flat".to_string(),
        SimpleLossChoice::Epa => "epa".to_string(),
    };

    // Target response derived from measurement
    config.target_response.enabled = true;
    config.target_response.shape = "from_measurement".to_string();
    config.target_response.slope_db_per_octave = 0.0;

    // Crossover (2.1+ only)
    if !preset.bass_management.is_empty() || matches!(preset.crossover, SimpleCrossoverChoice::Lr48)
    {
        config.schroeder_split.enabled = true;
    }

    // Sane defaults for params not exposed in Simple mode
    config.num_filters = 7;
    config.algorithm = "autoeq:cmaes".to_string();
    config.population = 300;
    config.max_iter = 50_000;
    config.bo_initial_samples = 0;
    config.bo_batch_size = 0;
    config.bo_posterior_std_threshold = 0.0;
    config.bo_acquisition = default_bo_acquisition();
    config.bo_ehvi = false;
    config.min_freq = 20.0;
    config.max_freq = 1600.0;
    config.min_db = -12.0;
    config.max_db = 4.0;
    config.min_q = 0.5;
    config.max_q = 6.0;
    config.peq_model = "pk".to_string();
    config.tolerance = 1e-5;
    config.atolerance = 1e-5;
    config.psychoacoustic = true;
    config.asymmetric_loss = true;
    config.refine = true;
    config.local_algo = "cobyla".to_string();

    // Multi-position strategy
    if !preset.multi_position_strategy.is_empty() {
        config.multi_measurement.enabled = true;
        config.multi_measurement.strategy =
            canonical_multi_measurement_strategy(&preset.multi_position_strategy)
                .unwrap_or("average")
                .to_string();
    }
}

/// Optimization mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RoomEqOptimizationMode {
    #[default]
    Iir,
    Fir,
    Mixed,
    MixedPhase,
}

impl RoomEqOptimizationMode {
    pub fn all() -> &'static [RoomEqOptimizationMode] {
        &[
            RoomEqOptimizationMode::Iir,
            RoomEqOptimizationMode::Fir,
            RoomEqOptimizationMode::Mixed,
            RoomEqOptimizationMode::MixedPhase,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            RoomEqOptimizationMode::Iir => "IIR (Parametric EQ)",
            RoomEqOptimizationMode::Fir => "FIR (Convolution)",
            RoomEqOptimizationMode::Mixed => "Mixed (IIR + FIR)",
            RoomEqOptimizationMode::MixedPhase => "Mixed-Phase (IIR + short FIR)",
        }
    }

    pub fn to_code(&self) -> &'static str {
        match self {
            RoomEqOptimizationMode::Iir => "iir",
            RoomEqOptimizationMode::Fir => "fir",
            RoomEqOptimizationMode::Mixed => "mixed",
            RoomEqOptimizationMode::MixedPhase => "mixed_phase",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "fir" => RoomEqOptimizationMode::Fir,
            "mixed" => RoomEqOptimizationMode::Mixed,
            "mixed_phase" => RoomEqOptimizationMode::MixedPhase,
            _ => RoomEqOptimizationMode::Iir,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            RoomEqOptimizationMode::Iir => "Uses standard biquad filters. Low latency, efficient.",
            RoomEqOptimizationMode::Fir => {
                "Uses impulse response convolution. Can correct phase, but higher latency."
            }
            RoomEqOptimizationMode::Mixed => {
                "Combines IIR for high frequencies and FIR for low frequencies."
            }
            RoomEqOptimizationMode::MixedPhase => {
                "IIR for minimum-phase + short FIR for excess phase. Low latency (~10ms)."
            }
        }
    }

    pub fn maturity(&self) -> ReleaseChannel {
        match self {
            RoomEqOptimizationMode::Iir => ReleaseChannel::Beta,
            RoomEqOptimizationMode::Fir => ReleaseChannel::Alpha,
            RoomEqOptimizationMode::Mixed => ReleaseChannel::Alpha,
            RoomEqOptimizationMode::MixedPhase => ReleaseChannel::Alpha,
        }
    }

    pub fn available(channel: ReleaseChannel) -> Vec<Self> {
        Self::all()
            .iter()
            .copied()
            .filter(|mode| channel.allows(mode.maturity()))
            .collect()
    }
}
