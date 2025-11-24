// ============================================================================
// Configuration
// ============================================================================

use serde::{Deserialize, Serialize};

pub fn default_fft_size() -> usize {
    2048
}

pub fn default_gain_front_direct() -> f32 {
    1.0
}

pub fn default_gain_front_ambient() -> f32 {
    0.5
}

pub fn default_gain_rear_ambient() -> f32 {
    1.2 // Boosted from 1.0 (20% increase) for better rear/height envelopment
}

pub fn default_lfe_cutoff_hz() -> f32 {
    120.0
}

pub fn default_stereo_width() -> f32 {
    0.5
}

pub fn default_bandpass_hz() -> f32 {
    220.0 // Lowered from 300Hz for more mid-range content in surrounds
}

pub fn default_speaker_config() -> String {
    "5.1".to_string()
}

pub fn default_height_gain() -> f32 {
    0.2
}

pub fn default_lfe_gain() -> f32 {
    1.0
}

pub fn default_subharmonic_gain() -> f32 {
    0.5
}

pub fn default_center_spread() -> f32 {
    0.0
}

pub fn default_hr_sharpen() -> f32 {
    1.0
}

pub fn default_safety_cap_db() -> f32 {
    3.0
}

/// Configuration parameters for UpmixerPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpmixerPluginParams {
    #[serde(default = "default_fft_size")]
    pub fft_size: usize,

    /// Speaker configuration ("5.1", "7.1", "5.1.4", etc.)
    #[serde(default = "default_speaker_config")]
    pub speaker_config: String,

    #[serde(default = "default_gain_front_direct")]
    pub gain_front_direct: f32,
    #[serde(default = "default_gain_front_ambient")]
    pub gain_front_ambient: f32,
    #[serde(default = "default_gain_rear_ambient")]
    pub gain_rear_ambient: f32,
    #[serde(default = "default_lfe_cutoff_hz")]
    pub lfe_cutoff_hz: f32,
    #[serde(default = "default_stereo_width")]
    pub stereo_width: f32,
    #[serde(default = "default_bandpass_hz")]
    pub bandpass_hz: f32,

    #[serde(default = "default_center_spread")]
    pub center_spread: f32,

    /// Height channel gain (0.0 to 2.0, default 1.0)
    /// Controls how much audio goes to overhead speakers
    #[serde(default = "default_height_gain")]
    pub height_gain: f32,

    /// LFE gain (0.0 to 2.0, default 1.0)
    /// Controls subwoofer level
    #[serde(default = "default_lfe_gain")]
    pub lfe_gain: f32,

    /// Enable Sub-Harmonic Synthesis for LFE
    #[serde(default)]
    pub enable_subharmonic_synth: bool,

    /// Gain for Sub-Harmonic Synthesis (0.0 to 1.0)
    #[serde(default = "default_subharmonic_gain")]
    pub subharmonic_gain: f32,

    /// Enable high-resolution direct-path enhancement (experimental)
    #[serde(default)]
    pub enable_hr_direct: bool,

    #[serde(default = "default_hr_sharpen")]
    pub hr_sharpen: f32,

    #[serde(default = "default_safety_cap_db")]
    pub safety_cap_db: f32,

    #[serde(default)]
    pub decorrelation_mode: usize,
}
