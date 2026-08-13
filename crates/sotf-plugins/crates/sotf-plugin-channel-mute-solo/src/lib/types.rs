use crate::params::PARAMS;
use serde::{Deserialize, Serialize};
use sotf_host::param_specs::find_by_key as param_by_key;

pub(super) fn default_dim_gain_db() -> f32 {
    param_by_key(PARAMS, "dim_gain_db").default_f32()
}

pub(super) fn default_fade_ms() -> f32 {
    param_by_key(PARAMS, "fade_ms").default_f32()
}

/// State for a single channel
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ChannelState {
    pub muted: bool,
    pub soloed: bool,
    #[serde(default)]
    pub dimmed: bool,
}

/// Configuration parameters for ChannelMuteSoloPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMuteSoloParams {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub channel_states: Vec<ChannelState>,
    /// Dim gain in dB (default -20.0)
    #[serde(default = "default_dim_gain_db")]
    pub dim_gain_db: f32,
    /// One-pole time constant in ms for mute/solo/dim transitions (default 5.0)
    #[serde(default = "default_fade_ms")]
    pub fade_ms: f32,
}
