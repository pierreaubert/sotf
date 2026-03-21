// ============================================================================
// Denoiser Plugin Configuration
// ============================================================================

use serde::{Deserialize, Serialize};
use sotf_host::param_specs::{denoiser::PARAMS as DN, find_by_key as pk};

// Default functions for serde
pub fn default_reduction_db() -> f32 {
    pk(DN, "reduction_db").default_f32()
}

pub fn default_floor_db() -> f32 {
    pk(DN, "floor_db").default_f32()
}

pub fn default_smoothing() -> f32 {
    pk(DN, "smoothing").default_f32()
}

pub fn default_attack_ms() -> f32 {
    pk(DN, "attack_ms").default_f32()
}

pub fn default_release_ms() -> f32 {
    pk(DN, "release_ms").default_f32()
}

pub fn default_low_latency() -> bool {
    pk(DN, "low_latency").default_bool()
}

// MCRA-specific defaults
pub fn default_mcra_alpha_s() -> f32 {
    pk(DN, "mcra_alpha_s").default_f32()
}

pub fn default_mcra_alpha_p() -> f32 {
    pk(DN, "mcra_alpha_p").default_f32()
}

pub fn default_mcra_l() -> usize {
    pk(DN, "mcra_l").default_usize()
}

pub fn default_mcra_delta() -> f32 {
    pk(DN, "mcra_delta").default_f32()
}

pub fn default_polyphonic_detection() -> bool {
    pk(DN, "polyphonic_detection").default_bool()
}

pub fn default_crack_sensitivity() -> f32 {
    pk(DN, "crack_sensitivity").default_f32()
}

pub fn default_psychoacoustic_masking() -> bool {
    pk(DN, "psychoacoustic_masking").default_bool()
}

pub fn default_use_captured_profile() -> bool {
    pk(DN, "use_captured_profile").default_bool()
}

pub fn default_transient_enabled() -> bool {
    pk(DN, "transient_enabled").default_bool()
}

pub fn default_spectral_smoothing_enabled() -> bool {
    pk(DN, "spectral_smoothing_enabled").default_bool()
}

pub fn default_temporal_smoothing_enabled() -> bool {
    pk(DN, "temporal_smoothing_enabled").default_bool()
}

pub fn default_hiss_enabled() -> bool {
    pk(DN, "hiss_enabled").default_bool()
}

pub fn default_hiss_threshold_db() -> f32 {
    pk(DN, "hiss_threshold_db").default_f32()
}

pub fn default_hiss_frequency_hz() -> f32 {
    pk(DN, "hiss_frequency_hz").default_f32()
}

pub fn default_hiss_strength() -> f32 {
    pk(DN, "hiss_strength").default_f32()
}

pub fn default_spectral_sub_enabled() -> bool {
    pk(DN, "spectral_sub_enabled").default_bool()
}

pub fn default_spectral_sub_alpha() -> f32 {
    pk(DN, "spectral_sub_alpha").default_f32()
}

pub fn default_spectral_sub_beta() -> f32 {
    pk(DN, "spectral_sub_beta").default_f32()
}

pub fn default_transparency() -> f32 {
    pk(DN, "transparency").default_f32()
}

pub fn default_dd_enabled() -> bool {
    pk(DN, "dd_enabled").default_bool()
}

pub fn default_dd_alpha() -> f32 {
    pk(DN, "dd_alpha").default_f32()
}

pub fn default_formant_preservation() -> bool {
    pk(DN, "formant_preservation").default_bool()
}

pub fn default_formant_strength() -> f32 {
    pk(DN, "formant_strength").default_f32()
}

pub fn default_multi_resolution() -> bool {
    pk(DN, "multi_resolution").default_bool()
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

    /// Sensitivity of the transient suppressor (1.0-100.0)
    /// Higher values mean LESS sensitive (higher threshold multiplier)
    #[serde(default = "default_crack_sensitivity")]
    pub crack_sensitivity: f32,

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

    /// Transparency: blend denoised signal toward dry (0 = full denoising, 1 = pass-through)
    #[serde(default = "default_transparency")]
    pub transparency: f32,

    /// Enable Decision-Directed SNR estimation (Ephraim-Malah)
    #[serde(default = "default_dd_enabled")]
    pub dd_enabled: bool,

    /// DD smoothing factor (0.5-0.999)
    #[serde(default = "default_dd_alpha")]
    pub dd_alpha: f32,

    /// Skip denoising for perceptually masked noise bins
    #[serde(default = "default_psychoacoustic_masking")]
    pub psychoacoustic_masking: bool,

    /// Use captured noise profile instead of live MCRA estimation
    #[serde(default = "default_use_captured_profile")]
    pub use_captured_profile: bool,

    /// Enable transient suppression (de-clicking)
    #[serde(default = "default_transient_enabled")]
    pub transient_enabled: bool,

    /// Enable spectral (frequency-domain) gain smoothing
    #[serde(default = "default_spectral_smoothing_enabled")]
    pub spectral_smoothing_enabled: bool,

    /// Enable temporal (attack/release) gain smoothing
    #[serde(default = "default_temporal_smoothing_enabled")]
    pub temporal_smoothing_enabled: bool,

    /// Enable hiss remover (high-frequency noise reduction)
    #[serde(default = "default_hiss_enabled")]
    pub hiss_enabled: bool,

    /// Hiss detection SNR threshold (dB)
    #[serde(default = "default_hiss_threshold_db")]
    pub hiss_threshold_db: f32,

    /// Frequency above which hiss removal applies (Hz)
    #[serde(default = "default_hiss_frequency_hz")]
    pub hiss_frequency_hz: f32,

    /// Hiss removal strength (0.0 to 1.0)
    #[serde(default = "default_hiss_strength")]
    pub hiss_strength: f32,

    /// Enable spectral subtraction
    #[serde(default = "default_spectral_sub_enabled")]
    pub spectral_sub_enabled: bool,

    /// Spectral subtraction oversubtraction factor
    #[serde(default = "default_spectral_sub_alpha")]
    pub spectral_sub_alpha: f32,

    /// Spectral subtraction floor
    #[serde(default = "default_spectral_sub_beta")]
    pub spectral_sub_beta: f32,

    /// Algorithm backend: 0=Classical, 1=RNNoise, 2=DeepFilter, 3=HybridNeural
    #[serde(default)]
    pub algorithm: usize,

    /// Preserve speech formant structure during denoising
    #[serde(default = "default_formant_preservation")]
    pub formant_preservation: bool,

    /// Formant preservation strength (0.0 = no preservation, 1.0 = maximum)
    #[serde(default = "default_formant_strength")]
    pub formant_strength: f32,

    /// Enable dual-resolution STFT processing (512 + 2048 samples).
    /// Small FFT tracks transients; large FFT handles steady-state noise.
    /// Gains are blended by spectral-flux-based transient detection.
    #[serde(default = "default_multi_resolution")]
    pub multi_resolution: bool,
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
            crack_sensitivity: default_crack_sensitivity(),
            mcra_alpha_s: default_mcra_alpha_s(),
            mcra_alpha_p: default_mcra_alpha_p(),
            mcra_l: default_mcra_l(),
            mcra_delta: default_mcra_delta(),
            transparency: default_transparency(),
            dd_enabled: default_dd_enabled(),
            dd_alpha: default_dd_alpha(),
            psychoacoustic_masking: default_psychoacoustic_masking(),
            use_captured_profile: default_use_captured_profile(),
            transient_enabled: default_transient_enabled(),
            spectral_smoothing_enabled: default_spectral_smoothing_enabled(),
            temporal_smoothing_enabled: default_temporal_smoothing_enabled(),
            hiss_enabled: default_hiss_enabled(),
            hiss_threshold_db: default_hiss_threshold_db(),
            hiss_frequency_hz: default_hiss_frequency_hz(),
            hiss_strength: default_hiss_strength(),
            spectral_sub_enabled: default_spectral_sub_enabled(),
            spectral_sub_alpha: default_spectral_sub_alpha(),
            spectral_sub_beta: default_spectral_sub_beta(),
            algorithm: 0,
            formant_preservation: default_formant_preservation(),
            formant_strength: default_formant_strength(),
            multi_resolution: default_multi_resolution(),
        }
    }
}
