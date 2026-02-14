//! Configuration types and parameters for the XTC plugin.

use serde::{Deserialize, Serialize};

/// XTC plugin configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XtcPluginParams {
    /// Distance to speakers in meters (default: 2.0m)
    #[serde(default = "default_distance")]
    pub distance_m: f32,

    /// Speaker angle in degrees (default: 30°)
    #[serde(default = "default_speaker_angle")]
    pub speaker_angle_deg: f32,

    /// Head radius in meters (default: 0.0875m, typical adult)
    #[serde(default = "default_head_radius")]
    pub head_radius_m: f32,

    /// FFT size (default: 1024, must be power of 2)
    #[serde(default = "default_fft_size")]
    pub fft_size: usize,

    /// Base regularization parameter β (default: 0.0003)
    /// Higher values = more stable but less cancellation
    #[serde(default = "default_beta_base")]
    pub beta_base: f32,

    /// Regularization boost at low frequencies (<100Hz) (default: 10.0)
    #[serde(default = "default_beta_low_freq_boost")]
    pub beta_low_freq_boost: f32,

    /// Regularization boost at high frequencies (>12kHz) (default: 10.0)
    #[serde(default = "default_beta_high_freq_boost")]
    pub beta_high_freq_boost: f32,

    /// Maximum filter gain in dB (default: 25.0)
    /// Limits how much the cancellation filter can boost any frequency bin.
    /// Lower values are safer but reduce cancellation depth.
    #[serde(default = "default_max_gain_db")]
    pub max_gain_db: f32,

    /// Head shadowing filter cutoff frequency in Hz (default: 4000 Hz)
    #[serde(default = "default_head_shadow_cutoff")]
    pub head_shadow_cutoff_hz: f32,

    /// Head shadowing filter slope (default: 6.0 dB/octave)
    #[serde(default = "default_head_shadow_slope")]
    pub head_shadow_slope_db_per_octave: f32,

    /// Head tracking: lateral offset in meters (default: 0.0)
    #[serde(default)]
    pub head_offset_x: f32,

    /// Head tracking: depth offset in meters (default: 0.0)
    #[serde(default)]
    pub head_offset_z: f32,

    /// Head tracking: yaw angle in degrees (-90 to +90, 0 = facing forward)
    #[serde(default)]
    pub head_yaw_deg: f32,

    /// Smoothing time constant for head tracking updates in seconds (default: 0.1s)
    #[serde(default = "default_head_tracking_smooth")]
    pub head_tracking_smooth_s: f32,

    /// Enable spectral energy normalization (default: false).
    /// When enabled, normalizes per-bin energy to reduce tonal coloration,
    /// but can degrade cancellation depth.
    #[serde(default)]
    pub spectral_normalization: bool,

    /// Enable plugin (default: true)
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_distance() -> f32 {
    2.0
}
fn default_speaker_angle() -> f32 {
    30.0
}
fn default_head_radius() -> f32 {
    0.0875
}
fn default_fft_size() -> usize {
    2048
}
fn default_beta_base() -> f32 {
    0.0003
}
fn default_max_gain_db() -> f32 {
    25.0
}
fn default_beta_low_freq_boost() -> f32 {
    10.0
}
fn default_beta_high_freq_boost() -> f32 {
    10.0
}
fn default_head_shadow_cutoff() -> f32 {
    4000.0
}
fn default_head_shadow_slope() -> f32 {
    6.0
}
fn default_head_tracking_smooth() -> f32 {
    0.1
}
fn default_enabled() -> bool {
    true
}

impl Default for XtcPluginParams {
    fn default() -> Self {
        Self {
            distance_m: default_distance(),
            speaker_angle_deg: default_speaker_angle(),
            head_radius_m: default_head_radius(),
            fft_size: default_fft_size(),
            beta_base: default_beta_base(),
            beta_low_freq_boost: default_beta_low_freq_boost(),
            beta_high_freq_boost: default_beta_high_freq_boost(),
            max_gain_db: default_max_gain_db(),
            head_shadow_cutoff_hz: default_head_shadow_cutoff(),
            head_shadow_slope_db_per_octave: default_head_shadow_slope(),
            head_offset_x: 0.0,
            head_offset_z: 0.0,
            head_yaw_deg: 0.0,
            head_tracking_smooth_s: default_head_tracking_smooth(),
            spectral_normalization: false,
            enabled: default_enabled(),
        }
    }
}
