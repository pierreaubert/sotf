// ============================================================================
// Denoiser Plugin Configuration
// ============================================================================

use super::super::param_specs::denoiser::*;
use serde::{Deserialize, Serialize};

// Default functions for serde
pub fn default_reduction_db() -> f32 {
    REDUCTION_DB_DEFAULT
}

pub fn default_floor_db() -> f32 {
    FLOOR_DB_DEFAULT
}

pub fn default_smoothing() -> f32 {
    SMOOTHING_DEFAULT
}

pub fn default_attack_ms() -> f32 {
    ATTACK_MS_DEFAULT
}

pub fn default_release_ms() -> f32 {
    RELEASE_MS_DEFAULT
}

pub fn default_low_latency() -> bool {
    LOW_LATENCY_DEFAULT
}

// MCRA-specific defaults
pub fn default_mcra_alpha_s() -> f32 {
    MCRA_ALPHA_S_DEFAULT
}

pub fn default_mcra_alpha_p() -> f32 {
    MCRA_ALPHA_P_DEFAULT
}

pub fn default_mcra_l() -> usize {
    MCRA_L_DEFAULT
}

pub fn default_mcra_delta() -> f32 {
    MCRA_DELTA_DEFAULT
}

pub fn default_polyphonic_detection() -> bool {
    POLYPHONIC_DETECTION_DEFAULT
}

/// Configuration parameters for DenoiserPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DenoiserPluginParams {
    /// How much noise reduction to apply (0-40 dB)
    #[serde(default = "default_reduction_db")]
    pub reduction_db: f32,

    /// Minimum gain floor to prevent musical noise (-60 to -10 dB)
    #[serde(default = "default_floor_db")]
    pub floor_db: f32,

    /// Temporal smoothing factor (0-0.99)
    #[serde(default = "default_smoothing")]
    pub smoothing: f32,

    /// Attack time for gain changes (ms)
    #[serde(default = "default_attack_ms")]
    pub attack_ms: f32,

    /// Release time for gain changes (ms)
    #[serde(default = "default_release_ms")]
    pub release_ms: f32,

    /// Use smaller FFT for lower latency (512 vs 2048)
    #[serde(default = "default_low_latency")]
    pub low_latency: bool,

    /// Enable Polyphonic Note Detection mode (Spectral Gating)
    #[serde(default = "default_polyphonic_detection")]
    pub polyphonic_detection: bool,

    // Advanced MCRA parameters (expert use)
    /// Noise PSD smoothing factor
    #[serde(default = "default_mcra_alpha_s")]
    pub mcra_alpha_s: f32,

    /// Speech presence probability smoothing factor
    #[serde(default = "default_mcra_alpha_p")]
    pub mcra_alpha_p: f32,

    /// Minimum tracking window in frames
    #[serde(default = "default_mcra_l")]
    pub mcra_l: usize,

    /// Speech presence detection threshold
    #[serde(default = "default_mcra_delta")]
    pub mcra_delta: f32,
}

impl Default for DenoiserPluginParams {
    fn default() -> Self {
        Self {
            reduction_db: default_reduction_db(),
            floor_db: default_floor_db(),
            smoothing: default_smoothing(),
            attack_ms: default_attack_ms(),
            release_ms: default_release_ms(),
            low_latency: default_low_latency(),
            polyphonic_detection: default_polyphonic_detection(),
            mcra_alpha_s: default_mcra_alpha_s(),
            mcra_alpha_p: default_mcra_alpha_p(),
            mcra_l: default_mcra_l(),
            mcra_delta: default_mcra_delta(),
        }
    }
}
