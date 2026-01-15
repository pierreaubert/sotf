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
    1.0 // Normalized from 1.1 to maintain energy balance
}

pub fn default_lfe_cutoff_hz() -> f32 {
    120.0
}

pub fn default_stereo_width() -> f32 {
    0.5
}

pub fn default_bandpass_hz() -> f32 {
    250.0 // Lowered from 300Hz for more mid-range content in surrounds
}

pub fn default_speaker_config() -> String {
    "5.1".to_string()
}

pub fn default_height_gain() -> f32 {
    0.5
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
    0.0 // Set to 0.0dB to strictly prevent clipping by default
}

// Sub-harmonic synthesis defaults
pub fn default_subharmonic_freq_hz() -> f32 {
    40.0
}

pub fn default_subharmonic_attack_ms() -> f32 {
    10.0
}

pub fn default_subharmonic_release_ms() -> f32 {
    50.0
}

// Decorrelation defaults
pub fn default_decorrelation_lfo_rate_hz() -> f32 {
    0.15
}

pub fn default_velvet_noise_duration_ms() -> f32 {
    30.0
}

pub fn default_velvet_noise_density() -> f32 {
    2000.0
}

// Height channel defaults
pub fn default_height_hf_cap_hz() -> f32 {
    16000.0
}

pub fn default_height_transient_reduction() -> f32 {
    0.6
}

pub fn default_height_direct_leak() -> f32 {
    0.15
}

// Surround routing defaults
pub fn default_surround_direct_bleed() -> f32 {
    0.50
}

pub fn default_rear_ambient_boost() -> f32 {
    1.0 // Normalized from 1.2 to prevent excessive surround levels
}

pub fn default_rear_late_reflection() -> f32 {
    0.10
}

// Ambient gain boost (sqrt(1-coherence) multiplier)
pub fn default_ambient_boost() -> f32 {
    1.0
}

// Dialogue detection defaults
pub fn default_dialogue_weight() -> f32 {
    0.4
}

pub fn default_voice_freq_min_hz() -> f32 {
    500.0
}

pub fn default_voice_freq_max_hz() -> f32 {
    3000.0
}

// Diagnostic bypass parameters (for isolating audio artifacts)
pub fn default_bypass_decorrelation() -> bool {
    false
}

pub fn default_bypass_transient_detection() -> bool {
    false
}

pub fn default_bypass_all_processing() -> bool {
    false
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

    // Sub-harmonic synthesis parameters
    /// Sub-harmonic frequency in Hz (20-80 Hz, default 40 Hz)
    #[serde(default = "default_subharmonic_freq_hz")]
    pub subharmonic_freq_hz: f32,

    /// Sub-harmonic attack time in ms (1-100 ms, default 10 ms)
    #[serde(default = "default_subharmonic_attack_ms")]
    pub subharmonic_attack_ms: f32,

    /// Sub-harmonic release time in ms (10-500 ms, default 50 ms)
    #[serde(default = "default_subharmonic_release_ms")]
    pub subharmonic_release_ms: f32,

    // Decorrelation parameters
    /// LFO rate for decorrelation in Hz (0.01-1.0 Hz, default 0.15 Hz)
    #[serde(default = "default_decorrelation_lfo_rate_hz")]
    pub decorrelation_lfo_rate_hz: f32,

    /// Velvet noise duration in ms (10-100 ms, default 30 ms)
    #[serde(default = "default_velvet_noise_duration_ms")]
    pub velvet_noise_duration_ms: f32,

    /// Velvet noise pulse density (500-5000 pulses/sec, default 2000)
    #[serde(default = "default_velvet_noise_density")]
    pub velvet_noise_density: f32,

    // Height channel parameters
    /// Height channel high-frequency cap in Hz (8000-20000 Hz, default 16000 Hz)
    #[serde(default = "default_height_hf_cap_hz")]
    pub height_hf_cap_hz: f32,

    /// Height channel transient reduction (0.0-1.0, default 0.6)
    #[serde(default = "default_height_transient_reduction")]
    pub height_transient_reduction: f32,

    /// Direct signal leak into height channels (0.0-0.5, default 0.15)
    #[serde(default = "default_height_direct_leak")]
    pub height_direct_leak: f32,

    // Surround routing parameters
    /// Direct signal bleed into surround/height channels (0.0-1.0, default 0.50)
    #[serde(default = "default_surround_direct_bleed")]
    pub surround_direct_bleed: f32,

    /// Rear ambient gain boost multiplier (1.0-3.0, default 1.0)
    #[serde(default = "default_rear_ambient_boost")]
    pub rear_ambient_boost: f32,

    /// Rear height late reflection level (0.0-0.5, default 0.10)
    #[serde(default = "default_rear_late_reflection")]
    pub rear_late_reflection: f32,

    // Ambient/coherence parameters
    /// Ambient gain boost factor (0.5-2.0, default 1.0)
    #[serde(default = "default_ambient_boost")]
    pub ambient_boost: f32,

    // Dialogue detection parameters
    /// Dialogue weight for center routing (0.0-1.0, default 0.4)
    #[serde(default = "default_dialogue_weight")]
    pub dialogue_weight: f32,

    /// Voice frequency range minimum in Hz (200-800 Hz, default 500 Hz)
    #[serde(default = "default_voice_freq_min_hz")]
    pub voice_freq_min_hz: f32,

    /// Voice frequency range maximum in Hz (2000-5000 Hz, default 3000 Hz)
    #[serde(default = "default_voice_freq_max_hz")]
    pub voice_freq_max_hz: f32,

    // Diagnostic bypass parameters (for isolating audio artifacts)
    /// Bypass decorrelation filters (sets all to identity/no phase change)
    /// Use this to test if decorrelation is causing audio artifacts
    #[serde(default = "default_bypass_decorrelation")]
    pub bypass_decorrelation: bool,

    /// Bypass transient detection (forces hr_transient_env to 0.0)
    /// Use this to test if transient-adaptive processing is causing artifacts
    #[serde(default = "default_bypass_transient_detection")]
    pub bypass_transient_detection: bool,

    /// Bypass ALL frequency domain processing - pure stereo pass-through
    /// Use this to test if the FFT/IFFT or overlap-add is causing artifacts
    #[serde(default = "default_bypass_all_processing")]
    pub bypass_all_processing: bool,
}
