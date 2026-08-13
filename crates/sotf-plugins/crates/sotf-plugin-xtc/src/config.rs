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

    /// Extra regularization at low frequencies (default: 10.0)
    #[serde(default = "default_beta_low_freq_boost")]
    pub beta_low_freq_boost: f32,

    /// Extra regularization at high frequencies (default: 10.0)
    #[serde(default = "default_beta_high_freq_boost")]
    pub beta_high_freq_boost: f32,

    /// Condition number target for regularization (default: 100.0).
    /// Controls how aggressively the inverse is regularized at ill-conditioned
    /// frequency bins. Lower values = more regularization = less cancellation.
    /// Range: 1-1000.
    #[serde(default = "default_kappa_target")]
    pub kappa_target: f32,

    /// Maximum filter gain in dB (default: 6.0)
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

    /// Head diffraction model: 0 = Woodworth (classic), 1 = Brown-Duda (rigid sphere)
    #[serde(default)]
    pub head_model: usize,

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

    /// Enable spectral energy normalization (default: true).
    /// When enabled, normalizes per-bin energy to reduce tonal coloration,
    /// but can degrade cancellation depth.
    #[serde(default = "default_spectral_normalization")]
    pub spectral_normalization: bool,

    /// Enable plugin (default: true)
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Enable room reflection compensation (default: false)
    #[serde(default)]
    pub room_reflections_enabled: bool,

    /// Path to room IR WAV file. When set, overrides image source model.
    #[serde(default)]
    pub room_ir_file: Option<String>,

    /// Room width in meters (X axis) — image source model (default: 4.0)
    #[serde(default = "default_room_width")]
    pub room_width_m: f32,

    /// Room depth in meters (Z axis, listening axis) — image source model (default: 5.0)
    #[serde(default = "default_room_depth")]
    pub room_depth_m: f32,

    /// Wall absorption coefficient, 0.0=reflective, 1.0=absorptive — image source model (default: 0.3)
    #[serde(default = "default_wall_absorption")]
    pub wall_absorption: f32,

    /// Beta boost multiplier at comb-filter null frequencies (default: 3.0)
    #[serde(default = "default_reflection_beta_boost")]
    pub reflection_beta_boost: f32,

    // Diagnostic bypass parameters (for isolating audio artifacts)
    /// Bypass XTC filters (use identity: output = input in freq domain).
    /// Tests if STFT framework (windowing + OLA) is causing distortion.
    #[serde(default)]
    pub bypass_xtc_filters: bool,

    /// Bypass spectral normalization only.
    /// Tests if normalization is over-correcting.
    #[serde(default)]
    pub bypass_spectral_normalization: bool,

    /// Bypass Neumann series refinement (use first-order inverse only).
    /// Tests if the refinement step is diverging at ill-conditioned frequencies.
    #[serde(default)]
    pub bypass_neumann_refinement: bool,

    /// Enable automatic gain compensation (default: true).
    /// Matches output loudness to input loudness, preventing distortion from
    /// filter gain accumulation.
    #[serde(default = "default_auto_gain_enabled")]
    pub auto_gain_enabled: bool,

    /// Maximum auto-gain compensation in dB (default: 6.0)
    #[serde(default = "default_auto_gain_max_db")]
    pub auto_gain_max_db: f32,

    /// Enable pinna resonance model (default: false).
    /// When enabled, applies ear canal, concha, and pinna notch resonances
    /// to the transfer functions. Adds +10-12 dB at 2.7-4.5 kHz which can
    /// cause aggressive spectral reshaping. Disable for cleaner output.
    #[serde(default)]
    pub pinna_model_enabled: bool,

    /// Path to HRTF/SOFA file. When set, uses measured HRTF data as the
    /// plant matrix C(f) instead of the Woodworth analytical model.
    /// Supports .sofa and .hrtfdb (SQLite) formats.
    #[serde(default)]
    pub hrtf_file: Option<String>,

    /// Filter source mode: "synthetic", "hrtf_file", or "roomeq_recommended".
    /// The default preserves legacy behavior: synthetic geometry unless
    /// `hrtf_file` is set.
    #[serde(default = "default_source_mode")]
    pub source_mode: String,

    /// Path to a roomEQ `recommended_xtc_matrix.json` artifact.
    #[serde(default)]
    pub recommended_matrix_file: Option<String>,

    /// Smoothing time for auto-gain transitions in ms (default: 100.0)
    #[serde(default = "default_auto_gain_smoothing_ms")]
    pub auto_gain_smoothing_ms: f32,

    /// ITD modeling mode for low-frequency cancellation improvement.
    ///
    /// At low frequencies (<300 Hz) the Woodworth model's implicit phase from
    /// path-length differences is numerically inaccurate because the wavelength
    /// is much larger than the head. An explicit fractional-sample delay applied
    /// in the frequency domain gives a more reliable LF phase relationship.
    ///
    /// - `"phase_only"` (default): use implicit phase from the path-length
    ///   difference encoded in the plant matrix transfer functions.
    /// - `"explicit_delay"`: apply an explicit time-delay phase shift
    ///   `e^{-j*2*pi*f*itd}` to the contralateral path at low frequencies,
    ///   blended out above 300 Hz via a sigmoid crossover.
    #[serde(default = "default_itd_modeling")]
    pub itd_modeling: String,
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
    0.001
}
fn default_beta_low_freq_boost() -> f32 {
    10.0
}
fn default_beta_high_freq_boost() -> f32 {
    10.0
}
fn default_max_gain_db() -> f32 {
    12.0 // Increased from 6.0 for better cancellation depth
}
fn default_kappa_target() -> f32 {
    50.0
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
fn default_room_width() -> f32 {
    4.0
}
fn default_room_depth() -> f32 {
    5.0
}
fn default_wall_absorption() -> f32 {
    0.3
}
fn default_reflection_beta_boost() -> f32 {
    3.0
}
fn default_auto_gain_enabled() -> bool {
    true
}
fn default_auto_gain_max_db() -> f32 {
    12.0
}
fn default_spectral_normalization() -> bool {
    true
}
fn default_auto_gain_smoothing_ms() -> f32 {
    100.0
}
fn default_source_mode() -> String {
    "synthetic".to_string()
}
fn default_itd_modeling() -> String {
    "phase_only".to_string()
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
            kappa_target: default_kappa_target(),
            max_gain_db: default_max_gain_db(),
            head_shadow_cutoff_hz: default_head_shadow_cutoff(),
            head_shadow_slope_db_per_octave: default_head_shadow_slope(),
            head_model: 0,
            head_offset_x: 0.0,
            head_offset_z: 0.0,
            head_yaw_deg: 0.0,
            head_tracking_smooth_s: default_head_tracking_smooth(),
            spectral_normalization: true, // Enabled by default to fix tonal coloration
            enabled: default_enabled(),
            room_reflections_enabled: false,
            room_ir_file: None,
            room_width_m: default_room_width(),
            room_depth_m: default_room_depth(),
            wall_absorption: default_wall_absorption(),
            reflection_beta_boost: default_reflection_beta_boost(),
            bypass_xtc_filters: false,
            bypass_spectral_normalization: false,
            bypass_neumann_refinement: false,
            auto_gain_enabled: default_auto_gain_enabled(),
            auto_gain_max_db: default_auto_gain_max_db(),
            auto_gain_smoothing_ms: default_auto_gain_smoothing_ms(),
            pinna_model_enabled: false,
            hrtf_file: None,
            source_mode: default_source_mode(),
            recommended_matrix_file: None,
            itd_modeling: default_itd_modeling(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_spectral_normalization_is_true() {
        let params = XtcPluginParams::default();
        assert!(
            params.spectral_normalization,
            "Default spectral_normalization should be true"
        );
    }

    #[test]
    fn test_deserialize_empty_json_spectral_normalization_true() {
        let json = "{}";
        let params: XtcPluginParams = serde_json::from_str(json).unwrap();
        assert!(
            params.spectral_normalization,
            "Deserializing empty JSON should default spectral_normalization to true"
        );
    }

    #[test]
    fn test_deserialize_explicit_false_spectral_normalization() {
        let json = r#"{"spectral_normalization": false}"#;
        let params: XtcPluginParams = serde_json::from_str(json).unwrap();
        assert!(
            !params.spectral_normalization,
            "Explicitly setting spectral_normalization=false should be respected"
        );
    }

    #[test]
    fn test_default_values_are_sensible() {
        let params = XtcPluginParams::default();
        assert!((params.distance_m - 2.0).abs() < f32::EPSILON);
        assert!((params.speaker_angle_deg - 30.0).abs() < f32::EPSILON);
        assert!(params.enabled);
        assert!(params.auto_gain_enabled);
        assert!(!params.room_reflections_enabled);
        assert!(!params.pinna_model_enabled);
    }
}
