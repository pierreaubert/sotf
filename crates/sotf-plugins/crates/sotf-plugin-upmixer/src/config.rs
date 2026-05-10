// ============================================================================
// Configuration
// ============================================================================

use serde::{Deserialize, Deserializer, Serialize};

/// Accept both a string (`"5.1"`) and a legacy integer index (`2` → `"5.1"`).
fn deserialize_speaker_config<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de;

    struct Visitor;
    impl<'de> de::Visitor<'de> for Visitor {
        type Value = String;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a speaker config string or legacy integer index")
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<String, E> {
            Ok(v.to_string())
        }
        fn visit_string<E: de::Error>(self, v: String) -> Result<String, E> {
            Ok(v)
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<String, E> {
            const C: &[&str] = &[
                "2.0", "5.0", "5.1", "7.0", "7.1", "7.1.2", "7.1.4", "9.1", "9.1.4", "9.1.6",
            ];
            Ok(C.get(v as usize).unwrap_or(&"5.1").to_string())
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<String, E> {
            self.visit_u64(v as u64)
        }
        fn visit_f64<E: de::Error>(self, v: f64) -> Result<String, E> {
            self.visit_u64(v as u64)
        }
    }
    deserializer.deserialize_any(Visitor)
}

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
    0.05 // Reduced from 0.15 to prevent voice leakage
}

// Surround routing defaults
pub fn default_surround_direct_bleed() -> f32 {
    0.15 // Reduced from 0.50 to prevent voice leakage
}

pub fn default_rear_ambient_boost() -> f32 {
    1.0
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

// ML vocal detection defaults
pub fn default_enable_ml_detection() -> bool {
    false
}

pub fn default_ml_model_path() -> String {
    String::new()
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

pub fn default_frequency_resolution() -> String {
    "erb".to_string()
}

pub fn default_enable_hr_direct() -> bool {
    true
}

pub fn default_low_latency() -> bool {
    false
}

pub fn default_dialogue_centroid_weight() -> f32 {
    0.3
}

pub fn default_dialogue_variance_weight() -> f32 {
    0.2
}

pub fn default_dialogue_coherence_weight() -> f32 {
    0.5
}

pub fn default_multi_source_extraction() -> bool {
    false
}

pub fn default_multi_source_threshold() -> f32 {
    0.1
}

pub fn default_binaural_preview() -> bool {
    false
}

pub fn default_auto_gain_enabled() -> bool {
    false
}

pub fn default_auto_gain_max_db() -> f32 {
    12.0
}

pub fn default_auto_gain_smoothing_ms() -> f32 {
    100.0
}

/// Configuration parameters for UpmixerPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpmixerPluginParams {
    #[serde(default = "default_fft_size")]
    pub fft_size: usize,

    /// Low-latency mode: uses 1024-point FFT (21ms at 48kHz) instead of 2048 (43ms).
    /// Halves analysis latency at the cost of lower frequency resolution in spatial analysis.
    /// The hop size and window are adjusted accordingly.
    #[serde(default = "default_low_latency")]
    pub low_latency: bool,

    /// Speaker configuration ("5.1", "7.1", "5.1.4", etc.)
    #[serde(
        default = "default_speaker_config",
        deserialize_with = "deserialize_speaker_config"
    )]
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
    #[serde(default = "default_enable_hr_direct")]
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

    /// Dialogue centroid sub-weight (0.0-1.0, default 0.3)
    #[serde(default = "default_dialogue_centroid_weight")]
    pub dialogue_centroid_weight: f32,

    /// Dialogue variance sub-weight (0.0-1.0, default 0.2)
    #[serde(default = "default_dialogue_variance_weight")]
    pub dialogue_variance_weight: f32,

    /// Dialogue coherence sub-weight (0.0-1.0, default 0.5)
    #[serde(default = "default_dialogue_coherence_weight")]
    pub dialogue_coherence_weight: f32,

    // ML vocal detection parameters
    /// Enable ML-based vocal detection (requires ml_model_path to be set)
    #[serde(default = "default_enable_ml_detection")]
    pub enable_ml_detection: bool,

    /// Path to ONNX model file for ML vocal detection
    #[serde(default = "default_ml_model_path")]
    pub ml_model_path: String,

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

    /// Frequency resolution for ERB band analysis.
    /// "erb" = standard ERB bands (~40-50 bands, default)
    /// "fine_erb" = half-ERB width bands (~100 bands, finer spatial resolution)
    /// "per_bin" = one band per FFT bin (~1025 bands at 2048-point FFT, maximum resolution)
    #[serde(default = "default_frequency_resolution")]
    pub frequency_resolution: String,

    /// Enable extraction of a second source during spatial analysis.
    #[serde(default = "default_multi_source_extraction")]
    pub multi_source_extraction: bool,

    /// Source separation sensitivity.
    #[serde(default = "default_multi_source_threshold")]
    pub multi_source_threshold: f32,

    /// Preview surround output as binaural stereo.
    #[serde(default = "default_binaural_preview")]
    pub binaural_preview: bool,

    /// Match rendered output loudness to stereo input.
    #[serde(default = "default_auto_gain_enabled")]
    pub auto_gain_enabled: bool,

    /// Maximum auto gain correction.
    #[serde(default = "default_auto_gain_max_db")]
    pub auto_gain_max_db: f32,

    /// Auto gain transition time.
    #[serde(default = "default_auto_gain_smoothing_ms")]
    pub auto_gain_smoothing_ms: f32,
}

impl Default for UpmixerPluginParams {
    fn default() -> Self {
        Self {
            fft_size: default_fft_size(),
            low_latency: default_low_latency(),
            speaker_config: default_speaker_config(),
            gain_front_direct: default_gain_front_direct(),
            gain_front_ambient: default_gain_front_ambient(),
            gain_rear_ambient: default_gain_rear_ambient(),
            lfe_cutoff_hz: default_lfe_cutoff_hz(),
            stereo_width: default_stereo_width(),
            bandpass_hz: default_bandpass_hz(),
            center_spread: default_center_spread(),
            height_gain: default_height_gain(),
            lfe_gain: default_lfe_gain(),
            enable_subharmonic_synth: false,
            subharmonic_gain: default_subharmonic_gain(),
            enable_hr_direct: default_enable_hr_direct(),
            hr_sharpen: default_hr_sharpen(),
            safety_cap_db: default_safety_cap_db(),
            decorrelation_mode: 0,
            subharmonic_freq_hz: default_subharmonic_freq_hz(),
            subharmonic_attack_ms: default_subharmonic_attack_ms(),
            subharmonic_release_ms: default_subharmonic_release_ms(),
            decorrelation_lfo_rate_hz: default_decorrelation_lfo_rate_hz(),
            velvet_noise_duration_ms: default_velvet_noise_duration_ms(),
            velvet_noise_density: default_velvet_noise_density(),
            height_hf_cap_hz: default_height_hf_cap_hz(),
            height_transient_reduction: default_height_transient_reduction(),
            height_direct_leak: default_height_direct_leak(),
            surround_direct_bleed: default_surround_direct_bleed(),
            rear_ambient_boost: default_rear_ambient_boost(),
            rear_late_reflection: default_rear_late_reflection(),
            ambient_boost: default_ambient_boost(),
            dialogue_weight: default_dialogue_weight(),
            voice_freq_min_hz: default_voice_freq_min_hz(),
            voice_freq_max_hz: default_voice_freq_max_hz(),
            dialogue_centroid_weight: default_dialogue_centroid_weight(),
            dialogue_variance_weight: default_dialogue_variance_weight(),
            dialogue_coherence_weight: default_dialogue_coherence_weight(),
            enable_ml_detection: default_enable_ml_detection(),
            ml_model_path: default_ml_model_path(),
            bypass_decorrelation: default_bypass_decorrelation(),
            bypass_transient_detection: default_bypass_transient_detection(),
            bypass_all_processing: default_bypass_all_processing(),
            frequency_resolution: default_frequency_resolution(),
            multi_source_extraction: default_multi_source_extraction(),
            multi_source_threshold: default_multi_source_threshold(),
            binaural_preview: default_binaural_preview(),
            auto_gain_enabled: default_auto_gain_enabled(),
            auto_gain_max_db: default_auto_gain_max_db(),
            auto_gain_smoothing_ms: default_auto_gain_smoothing_ms(),
        }
    }
}
