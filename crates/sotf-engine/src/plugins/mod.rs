//! Plugin type definitions, settings, and utilities

pub mod chain;
pub mod eq;
pub mod matrix;
pub mod utility;

// Re-export main items from submodules
pub use chain::PluginChain;
pub use eq::EQFilter;
pub use matrix::{
    apply_matrix_preset, available_matrix_presets, detect_matrix_preset, resize_matrix,
    upmixer_output_channels,
};
pub use utility::{
    db_to_linear, get_channel_label, get_channel_label_from_config, linear_to_db_string,
    plugins_to_path_config_json, preset_file_to_path_config_json,
};

use crate::engine::PluginConfig;
use math_audio_iir_fir::BiquadFilterType;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;

/// Deserialize `speaker_config` accepting both a string (e.g. `"5.1"`) and
/// a legacy integer index (e.g. `0` → `"2.0"`, `2` → `"5.1"`).
fn deserialize_speaker_config<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de;

    struct SpeakerConfigVisitor;

    impl<'de> de::Visitor<'de> for SpeakerConfigVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a speaker config string (e.g. \"5.1\") or a legacy integer index")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<String, E> {
            Ok(v.to_string())
        }

        fn visit_string<E: de::Error>(self, v: String) -> Result<String, E> {
            Ok(v)
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> Result<String, E> {
            const CONFIGS: &[&str] = &[
                "2.0", "5.0", "5.1", "7.0", "7.1", "7.1.2", "7.1.4", "9.1", "9.1.4", "9.1.6",
            ];
            Ok(CONFIGS.get(v as usize).unwrap_or(&"5.1").to_string())
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> Result<String, E> {
            self.visit_u64(v as u64)
        }

        fn visit_f64<E: de::Error>(self, v: f64) -> Result<String, E> {
            self.visit_u64(v as u64)
        }
    }

    deserializer.deserialize_any(SpeakerConfigVisitor)
}
use sotf_plugins::{
    BandCompressorParams, BandExpanderParams, CrossfeedMode, CrossfeedPreset,
    SpectralTiltCorrection, TiltReferenceFreq,
};

/// Feature maturity classification for gating experimental features.
///
/// Ordering: Prod < Beta < Alpha. A user on `Beta` channel sees Prod + Beta features.
/// `allows(item_level)` returns true when `self >= item_level`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum ReleaseChannel {
    /// Stable, production-ready features (default)
    #[default]
    Prod,
    /// Features in testing, mostly stable
    Beta,
    /// Experimental features, may change or break
    Alpha,
}

impl ReleaseChannel {
    pub fn all() -> &'static [ReleaseChannel] {
        &[
            ReleaseChannel::Prod,
            ReleaseChannel::Beta,
            ReleaseChannel::Alpha,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            ReleaseChannel::Prod => "Stable",
            ReleaseChannel::Beta => "Beta",
            ReleaseChannel::Alpha => "Alpha",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ReleaseChannel::Prod => "Only stable, production-ready features",
            ReleaseChannel::Beta => "Includes beta features in testing",
            ReleaseChannel::Alpha => "All features including experimental ones",
        }
    }

    /// Returns true if this channel level allows access to features at `item_level`.
    pub fn allows(&self, item_level: ReleaseChannel) -> bool {
        *self >= item_level
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PluginType {
    EQ,
    Gain,
    Upmixer,
    AAE,
    Compressor,
    Limiter,
    Gate,
    Expander,
    MultibandCompressor,
    MultibandExpander,
    LoudnessCompensation,
    FletcherMunson,
    BinauralDecoder,
    Convolution,
    LoudnessMonitor,
    SpectrumAnalyzer,
    ChannelMuteSolo,
    Matrix,
    XTC,
    Denoiser,
    Pnd,
    ABCompare,
    BandSplit,
    BandMerge,
    Downmix,
    MonoToStereo,
    Crossfeed,
    Delay,
    Aec,
    Beamformer,
    AmbisonicsDecoder,
    StereoImager,
    DeEsser,
    TransientShaper,
    Saturation,
    DynamicEq,
    LinearPhaseEq,
    SpectralCompressor,
}

impl PluginType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::EQ => "EQ",
            Self::Gain => "Gain",
            Self::Upmixer => "Upmixer",
            Self::AAE => "AAE",
            Self::Compressor => "Compressor",
            Self::Gate => "Gate",
            Self::Limiter => "Limiter",
            Self::Expander => "Expander",
            Self::MultibandCompressor => "Multiband Compressor",
            Self::MultibandExpander => "Multiband Expander",
            Self::LoudnessCompensation => "Loudness Compensation",
            Self::FletcherMunson => "Fletcher-Munson",
            Self::BinauralDecoder => "Binaural Decoder",
            Self::Convolution => "Convolution",
            Self::LoudnessMonitor => "Loudness Monitor",
            Self::SpectrumAnalyzer => "Spectrum Analyzer",
            Self::ChannelMuteSolo => "Channel Mute/Solo",
            Self::Matrix => "Matrix Mixer",
            Self::XTC => "Crosstalk Cancellation",
            Self::Denoiser => "Denoiser",
            Self::Pnd => "PND Varispeed",
            Self::ABCompare => "A/B Compare",
            Self::BandSplit => "Band Split",
            Self::BandMerge => "Band Merge",
            Self::Downmix => "Downmix",
            Self::MonoToStereo => "Mono to Stereo",
            Self::Crossfeed => "Crossfeed",
            Self::Delay => "Delay",
            Self::Aec => "AEC",
            Self::Beamformer => "Beamformer",
            Self::AmbisonicsDecoder => "Ambisonics Decoder",
            Self::StereoImager => "Stereo Imager",
            Self::DeEsser => "De-Esser",
            Self::TransientShaper => "Transient Shaper",
            Self::Saturation => "Saturation",
            Self::DynamicEq => "Dynamic EQ",
            Self::LinearPhaseEq => "Linear-Phase EQ",
            Self::SpectralCompressor => "Spectral Compressor",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::EQ => "Parametric Equalizer IIR",
            Self::Gain => "Simple Volume/Gain Control",
            Self::Upmixer => "Stereo to Surround 5.1 to 9.1.6",
            Self::AAE => "Active Acoustic Enhancement (LARES-inspired reverb)",
            Self::Compressor => "Dynamic Range Compressor",
            Self::Limiter => "Peak Limiter",
            Self::Gate => "Noise Gate",
            Self::Expander => "Dynamic Range Expander with Hysteresis",
            Self::MultibandCompressor => "Multiband Dynamic Range Compressor",
            Self::MultibandExpander => "Multiband Dynamic Range Expander",
            Self::LoudnessCompensation => "Equal Loudness Compensation",
            Self::FletcherMunson => "Volume-dependent ISO 226 loudness curves",
            Self::BinauralDecoder => "Multi-channel to Binaural (HRTF)",
            Self::Convolution => "FFT-based Convolution (IR Processing)",
            Self::LoudnessMonitor => "Real-time EBU R128 loudness monitoring",
            Self::SpectrumAnalyzer => "Real-time frequency spectrum analysis",
            Self::ChannelMuteSolo => "Mute or solo individual channels",
            Self::Matrix => "Channel routing and mixing matrix",
            Self::XTC => "Crosstalk cancellation for speaker playback",
            Self::Denoiser => "Wiener filter denoiser with MCRA noise estimation",
            Self::Pnd => "Polyphonic note detection and varispeed correction",
            Self::ABCompare => "A/B comparison with auto-gain loudness matching",
            Self::BandSplit => "Split audio into low/high frequency bands",
            Self::BandMerge => "Merge frequency bands back together",
            Self::Downmix => "Phase-coherent surround to stereo downmix",
            Self::MonoToStereo => "Convert mono signal to pseudo-stereo",
            Self::Crossfeed => "Headphone crossfeed for speaker-like listening",
            Self::Delay => "Simple delay effect with feedback",
            Self::Aec => "Acoustic Echo Cancellation (PBFDAF + Two-Path + Post-Filter)",
            Self::Beamformer => "Microphone array beamformer (MVDR / Superdirective / GSC)",
            Self::AmbisonicsDecoder => "HOA Ambisonics decoder (AllRAD) to speaker layouts",
            Self::StereoImager => "Multi-band M/S stereo width control",
            Self::DeEsser => "Sibilance reduction via bandpass detection and compression",
            Self::TransientShaper => "SPL Transient Designer — attack/sustain shaping",
            Self::Saturation => "Harmonic saturation / exciter with multiple modes",
            Self::DynamicEq => "Frequency-selective dynamics (hybrid EQ + compressor)",
            Self::LinearPhaseEq => "Parametric EQ with linear-phase FIR convolution",
            Self::SpectralCompressor => {
                "Per-bin FFT dynamics processor for surgical spectral compression"
            }
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::EQ,
            Self::Gain,
            Self::Upmixer,
            Self::AAE,
            Self::Compressor,
            Self::Limiter,
            Self::Gate,
            Self::Expander,
            Self::MultibandCompressor,
            Self::MultibandExpander,
            Self::LoudnessCompensation,
            Self::FletcherMunson,
            Self::BinauralDecoder,
            Self::Convolution,
            Self::LoudnessMonitor,
            Self::SpectrumAnalyzer,
            Self::ChannelMuteSolo,
            Self::Matrix,
            Self::XTC,
            Self::Denoiser,
            Self::Pnd,
            Self::ABCompare,
            Self::BandSplit,
            Self::BandMerge,
            Self::Downmix,
            Self::MonoToStereo,
            Self::Crossfeed,
            Self::Delay,
            Self::Aec,
            Self::Beamformer,
            Self::AmbisonicsDecoder,
            Self::StereoImager,
            Self::DeEsser,
            Self::TransientShaper,
            Self::Saturation,
            Self::DynamicEq,
            Self::LinearPhaseEq,
            Self::SpectralCompressor,
        ]
    }

    /// Parse a plugin type from its name or serde variant (case-insensitive).
    ///
    /// Accepts both human names (e.g. `"Loudness Compensation"`) and short
    /// serde names (e.g. `"loudnesscompensation"`, `"eq"`, `"EQ"`).
    pub fn from_name(name: &str) -> Option<Self> {
        let lower = name.to_lowercase();
        Self::all()
            .into_iter()
            .find(|pt| pt.name().to_lowercase() == lower)
            .or_else(|| {
                // Also try matching with spaces/hyphens/underscores stripped
                let normalized = lower.replace([' ', '-', '_'], "");
                Self::all().into_iter().find(|pt| {
                    let variant = format!("{:?}", pt).to_lowercase();
                    variant == normalized
                })
            })
    }

    /// Returns true if this is a monitoring/analyzer plugin (non-processing)
    pub fn is_monitoring(&self) -> bool {
        matches!(
            self,
            Self::LoudnessMonitor | Self::SpectrumAnalyzer | Self::ChannelMuteSolo
        )
    }

    /// Returns the maturity level of this plugin type.
    pub fn maturity(&self) -> ReleaseChannel {
        match self {
            Self::EQ
            | Self::Gain
            | Self::Compressor
            | Self::ChannelMuteSolo
            | Self::Crossfeed
            | Self::Delay
            | Self::Expander
            | Self::FletcherMunson
            | Self::Gate
            | Self::Limiter
            | Self::LoudnessMonitor
            | Self::Matrix
            | Self::MultibandCompressor
            | Self::MultibandExpander
            | Self::SpectrumAnalyzer
            | Self::Upmixer
            | Self::XTC => ReleaseChannel::Prod,

            Self::AAE
            | Self::ABCompare
            | Self::BandSplit
            | Self::BandMerge
            | Self::Downmix
            | Self::LoudnessCompensation
            | Self::MonoToStereo
            | Self::StereoImager
            | Self::DeEsser
            | Self::TransientShaper
            | Self::Saturation
            | Self::DynamicEq
            | Self::LinearPhaseEq => ReleaseChannel::Beta,

            Self::BinauralDecoder
            | Self::Convolution
            | Self::Pnd
            | Self::Denoiser
            | Self::Aec
            | Self::Beamformer
            | Self::AmbisonicsDecoder
            | Self::SpectralCompressor => ReleaseChannel::Alpha,
        }
    }
}

// Import param_specs for plugin defaults and serde_param_default! macro
use sotf_plugins::param_specs::ab_compare as ab_compare_specs;
use sotf_plugins::param_specs::aec as aec_specs;
use sotf_plugins::param_specs::ambisonics as ambisonics_specs;
use sotf_plugins::param_specs::band_merge as band_merge_specs;
use sotf_plugins::param_specs::band_split as band_split_specs;
use sotf_plugins::param_specs::beamformer as beamformer_specs;
use sotf_plugins::param_specs::binaural as binaural_specs;
use sotf_plugins::param_specs::channel_mute_solo as cms_specs;
use sotf_plugins::param_specs::compressor as compressor_specs;
use sotf_plugins::param_specs::convolution as convolution_specs;
use sotf_plugins::param_specs::crossfeed as crossfeed_specs;
use sotf_plugins::param_specs::de_esser as de_esser_specs;
use sotf_plugins::param_specs::delay as delay_specs;
use sotf_plugins::param_specs::denoiser as denoiser_specs;
use sotf_plugins::param_specs::downmix as downmix_specs;
use sotf_plugins::param_specs::dynamic_eq as dynamic_eq_specs;
use sotf_plugins::param_specs::expander as expander_specs;
use sotf_plugins::param_specs::find_by_key as pk;
// fletcher_munson specs removed — merged into loudness_compensation
use sotf_plugins::param_specs::gain as gain_specs;
use sotf_plugins::param_specs::gate as gate_specs;
use sotf_plugins::param_specs::limiter as limiter_specs;
use sotf_plugins::param_specs::linear_phase_eq as linear_phase_eq_specs;
use sotf_plugins::param_specs::loudness_compensation as lc_specs;
use sotf_plugins::param_specs::matrix as matrix_specs;
use sotf_plugins::param_specs::mono_to_stereo as mono_to_stereo_specs;
use sotf_plugins::param_specs::multiband_compressor as mb_compressor_specs;
use sotf_plugins::param_specs::multiband_expander as mb_expander_specs;
use sotf_plugins::param_specs::pnd as pnd_specs;
use sotf_plugins::param_specs::saturation as saturation_specs;
use sotf_plugins::param_specs::spectral_compressor as spectral_compressor_specs;
use sotf_plugins::param_specs::spectrum as spectrum_specs;
use sotf_plugins::param_specs::stereo_imager as stereo_imager_specs;
use sotf_plugins::param_specs::transient_shaper as transient_shaper_specs;
use sotf_plugins::param_specs::upmixer as upmixer_specs;
use sotf_plugins::param_specs::xtc as xtc_specs;

sotf_plugins::serde_param_default! {
    upmixer_specs::PARAMS;
    fn default_upmixer_subharmonic_gain() -> f64 = "subharmonic_gain";
    fn default_upmixer_hr_sharpen() -> f64 = "hr_sharpen";
    fn default_upmixer_safety_cap_db() -> f64 = "safety_cap_db";
    fn default_upmixer_center_spread() -> f64 = "center_spread";
    fn default_upmixer_surround_direct_bleed() -> f64 = "surround_direct_bleed";
    fn default_upmixer_rear_late_reflection() -> f64 = "rear_late_reflection";
    fn default_upmixer_subharmonic_freq_hz() -> f64 = "subharmonic_freq_hz";
    fn default_upmixer_subharmonic_attack_ms() -> f64 = "subharmonic_attack_ms";
    fn default_upmixer_subharmonic_release_ms() -> f64 = "subharmonic_release_ms";
    fn default_upmixer_decorrelation_lfo_rate_hz() -> f64 = "decorrelation_lfo_rate_hz";
    fn default_upmixer_velvet_noise_duration_ms() -> f64 = "velvet_noise_duration_ms";
    fn default_upmixer_velvet_noise_density() -> f64 = "velvet_noise_density";
    fn default_upmixer_height_hf_cap_hz() -> f64 = "height_hf_cap_hz";
    fn default_upmixer_height_transient_reduction() -> f64 = "height_transient_reduction";
    fn default_upmixer_height_direct_leak() -> f64 = "height_direct_leak";
    fn default_upmixer_rear_ambient_boost() -> f64 = "rear_ambient_boost";
    fn default_upmixer_ambient_boost() -> f64 = "ambient_boost";
    fn default_upmixer_dialogue_weight() -> f64 = "dialogue_weight";
    fn default_upmixer_dialogue_centroid_weight() -> f64 = "dialogue_centroid_weight";
    fn default_upmixer_dialogue_variance_weight() -> f64 = "dialogue_variance_weight";
    fn default_upmixer_dialogue_coherence_weight() -> f64 = "dialogue_coherence_weight";
    fn default_upmixer_voice_freq_min_hz() -> f64 = "voice_freq_min_hz";
    fn default_upmixer_voice_freq_max_hz() -> f64 = "voice_freq_max_hz";
    fn default_upmixer_enable_hr_direct() -> bool = "enable_hr_direct";
    fn default_upmixer_multi_source_threshold() -> f64 = "multi_source_threshold";
    fn default_upmixer_frequency_resolution() -> usize = "frequency_resolution";
}

use sotf_plugins::param_specs::aae as aae_specs;

sotf_plugins::serde_param_default! {
    aae_specs::PARAMS;
    fn default_aae_speaker_config() -> String = "speaker_config";
    fn default_aae_room_size() -> f64 = "room_size";
    fn default_aae_rt60() -> f64 = "rt60";
    fn default_aae_bass_ratio() -> f64 = "bass_ratio";
    fn default_aae_treble_ratio() -> f64 = "treble_ratio";
    fn default_aae_pre_delay_ms() -> f64 = "pre_delay_ms";
    fn default_aae_room_preset() -> String = "room_preset";
    fn default_aae_dry_level() -> f64 = "dry_level";
    fn default_aae_er_level() -> f64 = "er_level";
    fn default_aae_late_level() -> f64 = "late_level";
    fn default_aae_lfe_level() -> f64 = "lfe_level";
    fn default_aae_mod_depth() -> f64 = "mod_depth";
    fn default_aae_er_mod_depth() -> f64 = "er_mod_depth";
    fn default_aae_input_diffusion() -> f64 = "input_diffusion";
    fn default_aae_envelopment() -> f64 = "envelopment";
    fn default_aae_height_amount() -> f64 = "height_amount";
    fn default_aae_content_aware() -> bool = "content_aware";
    fn default_aae_dialogue_attenuation_db() -> f64 = "dialogue_attenuation_db";
    fn default_aae_safety_limit_db() -> f64 = "safety_limit_db";
}

sotf_plugins::serde_param_default! {
    gain_specs::PARAMS;
    fn default_gain_smoothing_ms() -> f64 = "smoothing_ms";
}

sotf_plugins::serde_param_default! {
    compressor_specs::PARAMS;
    fn default_compressor_link_channels() -> bool = "link_channels";
    fn default_compressor_sidechain_hpf_hz() -> f64 = "sidechain_hpf_hz";
    fn default_compressor_sidechain_hpf_order() -> String = "sidechain_hpf_order";
    fn default_compressor_detection_mode() -> String = "detection_mode";
}

sotf_plugins::serde_param_default! {
    gate_specs::PARAMS;
    fn default_gate_sidechain_hpf_order() -> String = "sidechain_hpf_order";
    fn default_gate_detection_mode() -> String = "detection_mode";
}

sotf_plugins::serde_param_default! {
    de_esser_specs::PARAMS;
    fn default_de_esser_frequency() -> f64 = "frequency";
    fn default_de_esser_q() -> f64 = "q";
    fn default_de_esser_threshold() -> f64 = "threshold";
    fn default_de_esser_ratio() -> f64 = "ratio";
    fn default_de_esser_attack() -> f64 = "attack";
    fn default_de_esser_release() -> f64 = "release";
    fn default_de_esser_mix() -> f64 = "mix";
}

fn default_de_esser_mode() -> String {
    de_esser_specs::MODES[1].to_string()
}

sotf_plugins::serde_param_default! {
    binaural_specs::PARAMS;
    fn default_binaural_enable_optimization() -> bool = "enable_optimization";
    fn default_binaural_late_reverb_mix() -> f64 = "late_reverb_mix";
    fn default_binaural_late_reverb_rt60() -> f64 = "late_reverb_rt60";
    fn default_binaural_late_reverb_damping() -> f64 = "late_reverb_damping";
}

sotf_plugins::serde_param_default! {
    lc_specs::PARAMS;
    fn default_auto_gain_max_db() -> f64 = "auto_gain_max_db";
    fn default_auto_gain_smoothing_ms() -> f64 = "auto_gain_smoothing_ms";
    fn default_lc_mid_enabled() -> bool = "mid_enabled";
    fn default_lc_mid_freq() -> f64 = "mid_freq";
    fn default_lc_mid_gain() -> f64 = "mid_gain";
    fn default_lc_mid_q() -> f64 = "mid_q";
    fn default_lc_mode() -> usize = "mode";
    fn default_lc_playback_level_db() -> f64 = "playback_level_db";
    fn default_lc_reference_level_db() -> f64 = "reference_level_db";
}

// Hardcoded defaults for backward-compat FletcherMunson deserialization.
// These match the old sotf-plugin-fletcher-munson defaults.
fn default_fm_reference_level_db() -> f64 {
    -14.0
}
fn default_fm_enabled() -> bool {
    true
}
fn default_fm_smoothing_ms() -> f64 {
    30.0
}
fn default_fm_auto_gain_max_db() -> f64 {
    12.0
}
fn default_fm_auto_gain_smoothing_ms() -> f64 {
    100.0
}
fn default_fm_band1_freq() -> f64 {
    60.0
}
fn default_fm_band1_q() -> f64 {
    0.5
}
fn default_fm_band1_max_gain() -> f64 {
    15.0
}
fn default_fm_band1_slope() -> f64 {
    0.6
}
fn default_fm_band2_freq() -> f64 {
    250.0
}
fn default_fm_band2_q() -> f64 {
    0.707
}
fn default_fm_band2_max_gain() -> f64 {
    8.0
}
fn default_fm_band2_slope() -> f64 {
    0.4
}
fn default_fm_band3_freq() -> f64 {
    3500.0
}
fn default_fm_band3_q() -> f64 {
    1.0
}
fn default_fm_band3_max_gain() -> f64 {
    4.0
}
fn default_fm_band3_slope() -> f64 {
    0.2
}
fn default_fm_band4_freq() -> f64 {
    12000.0
}
fn default_fm_band4_q() -> f64 {
    0.707
}
fn default_fm_band4_max_gain() -> f64 {
    6.0
}
fn default_fm_band4_slope() -> f64 {
    0.3
}

sotf_plugins::serde_param_default! {
    limiter_specs::PARAMS;
    fn default_limiter_lookahead_ms() -> f64 = "lookahead";
    fn default_limiter_soft() -> bool = "soft";
    fn default_limiter_mix() -> f64 = "mix";
    fn default_limiter_link_amount() -> f64 = "link_amount";
}

sotf_plugins::serde_param_default! {
    gate_specs::PARAMS;
    fn default_gate_hold_ms() -> f64 = "hold";
    fn default_gate_mix() -> f64 = "mix";
    fn default_gate_link_channels() -> bool = "link_channels";
    fn default_gate_range_db() -> f64 = "range_db";
}

sotf_plugins::serde_param_default! {
    expander_specs::PARAMS;
    fn default_expander_threshold_db() -> f64 = "threshold";
    fn default_expander_ratio() -> f64 = "ratio";
    fn default_expander_attack_ms() -> f64 = "attack";
    fn default_expander_release_ms() -> f64 = "release";
    fn default_expander_range_db() -> f64 = "range";
    fn default_expander_knee_db() -> f64 = "knee";
    fn default_expander_hysteresis_db() -> f64 = "hysteresis";
    fn default_expander_hold_ms() -> f64 = "hold";
    fn default_expander_mix() -> f64 = "mix";
    fn default_expander_link_channels() -> bool = "link_channels";
    fn default_expander_sidechain_hpf_hz() -> f64 = "sidechain_hpf_hz";
    fn default_expander_detection_mode() -> String = "detection_mode";
}

sotf_plugins::serde_param_default! {
    mb_compressor_specs::GLOBAL_PARAMS;
    fn default_mb_compressor_num_bands() -> usize = "num_bands";
    fn default_mb_compressor_crossover_preset() -> i32 = "crossover_preset";
    fn default_mb_compressor_crossover_freq_1() -> f64 = "crossover_freq_1";
    fn default_mb_compressor_crossover_freq_2() -> f64 = "crossover_freq_2";
    fn default_mb_compressor_crossover_freq_3() -> f64 = "crossover_freq_3";
    fn default_mb_compressor_crossover_freq_4() -> f64 = "crossover_freq_4";
    fn default_mb_compressor_threshold_db() -> f64 = "threshold";
    fn default_mb_compressor_ratio() -> f64 = "ratio";
    fn default_mb_compressor_attack_ms() -> f64 = "attack";
    fn default_mb_compressor_release_ms() -> f64 = "release";
    fn default_mb_compressor_knee_db() -> f64 = "knee";
    fn default_mb_compressor_mix() -> f64 = "mix";
    fn default_mb_compressor_link_channels() -> bool = "link_channels";
    fn default_mb_compressor_link_amount() -> f64 = "link_amount";
}

sotf_plugins::serde_param_default! {
    mb_expander_specs::GLOBAL_PARAMS;
    fn default_mb_expander_num_bands() -> usize = "num_bands";
    fn default_mb_expander_crossover_preset() -> i32 = "crossover_preset";
    fn default_mb_expander_crossover_freq_1() -> f64 = "crossover_freq_1";
    fn default_mb_expander_crossover_freq_2() -> f64 = "crossover_freq_2";
    fn default_mb_expander_crossover_freq_3() -> f64 = "crossover_freq_3";
    fn default_mb_expander_crossover_freq_4() -> f64 = "crossover_freq_4";
    fn default_mb_expander_threshold_db() -> f64 = "threshold";
    fn default_mb_expander_ratio() -> f64 = "ratio";
    fn default_mb_expander_attack_ms() -> f64 = "attack";
    fn default_mb_expander_release_ms() -> f64 = "release";
    fn default_mb_expander_range_db() -> f64 = "range";
    fn default_mb_expander_knee_db() -> f64 = "knee";
    fn default_mb_expander_hysteresis_db() -> f64 = "hysteresis";
    fn default_mb_expander_hold_ms() -> f64 = "hold";
    fn default_mb_expander_mix() -> f64 = "mix";
    fn default_mb_expander_link_channels() -> bool = "link_channels";
    fn default_mb_expander_detection_mode() -> String = "detection_mode";
}

sotf_plugins::serde_param_default! {
    xtc_specs::PARAMS;
    fn default_xtc_distance_m() -> f64 = "distance_m";
    fn default_xtc_speaker_angle_deg() -> f64 = "speaker_angle_deg";
    fn default_xtc_head_radius_m() -> f64 = "head_radius_m";
    fn default_xtc_beta_base() -> f64 = "beta_base";
    fn default_xtc_beta_low_freq_boost() -> f64 = "beta_low_freq_boost";
    fn default_xtc_beta_high_freq_boost() -> f64 = "beta_high_freq_boost";
    fn default_xtc_head_shadow_cutoff_hz() -> f64 = "head_shadow_cutoff_hz";
    fn default_xtc_head_shadow_slope() -> f64 = "head_shadow_slope_db_per_octave";
    fn default_xtc_max_gain_db() -> f64 = "max_gain_db";
    fn default_xtc_auto_gain_enabled() -> bool = "auto_gain_enabled";
    fn default_xtc_auto_gain_max_db() -> f64 = "auto_gain_max_db";
    fn default_xtc_auto_gain_smoothing_ms() -> f64 = "auto_gain_smoothing_ms";
    fn default_xtc_room_width() -> f64 = "room_width_m";
    fn default_xtc_room_depth() -> f64 = "room_depth_m";
    fn default_xtc_wall_absorption() -> f64 = "wall_absorption";
    fn default_xtc_reflection_beta_boost() -> f64 = "reflection_beta_boost";
    fn default_xtc_spectral_normalization() -> bool = "spectral_normalization";
    fn default_xtc_room_reflections_enabled() -> bool = "room_reflections_enabled";
    fn default_xtc_pinna_model_enabled() -> bool = "pinna_model_enabled";
    fn default_xtc_head_tracking_smooth_s() -> f64 = "head_tracking_smooth_s";
}

sotf_plugins::serde_param_default! {
    denoiser_specs::PARAMS;
    fn default_denoiser_reduction_db() -> f64 = "reduction_db";
    fn default_denoiser_floor_db() -> f64 = "floor_db";
    fn default_denoiser_smoothing() -> f64 = "smoothing";
    fn default_denoiser_attack_ms() -> f64 = "attack_ms";
    fn default_denoiser_release_ms() -> f64 = "release_ms";
    fn default_denoiser_low_latency() -> bool = "low_latency";
    fn default_denoiser_polyphonic_detection() -> bool = "polyphonic_detection";
    fn default_denoiser_psychoacoustic_masking() -> bool = "psychoacoustic_masking";
    fn default_denoiser_use_captured_profile() -> bool = "use_captured_profile";
    fn default_denoiser_transparency() -> f64 = "transparency";
    fn default_denoiser_dd_enabled() -> bool = "dd_enabled";
    fn default_denoiser_dd_alpha() -> f64 = "dd_alpha";
    fn default_denoiser_mcra_alpha_s() -> f64 = "mcra_alpha_s";
    fn default_denoiser_mcra_alpha_p() -> f64 = "mcra_alpha_p";
    fn default_denoiser_mcra_l() -> usize = "mcra_l";
    fn default_denoiser_mcra_delta() -> f64 = "mcra_delta";
    fn default_denoiser_crack_sensitivity() -> f64 = "crack_sensitivity";
    fn default_denoiser_transient_enabled() -> bool = "transient_enabled";
    fn default_denoiser_spectral_smoothing_enabled() -> bool = "spectral_smoothing_enabled";
    fn default_denoiser_temporal_smoothing_enabled() -> bool = "temporal_smoothing_enabled";
    fn default_denoiser_hiss_enabled() -> bool = "hiss_enabled";
    fn default_denoiser_hiss_threshold_db() -> f64 = "hiss_threshold_db";
    fn default_denoiser_hiss_frequency_hz() -> f64 = "hiss_frequency_hz";
    fn default_denoiser_hiss_strength() -> f64 = "hiss_strength";
    fn default_denoiser_spectral_sub_enabled() -> bool = "spectral_sub_enabled";
    fn default_denoiser_spectral_sub_alpha() -> f64 = "spectral_sub_alpha";
    fn default_denoiser_spectral_sub_beta() -> f64 = "spectral_sub_beta";
    fn default_denoiser_algorithm() -> usize = "algorithm";
    fn default_denoiser_formant_strength() -> f64 = "formant_strength";
    fn default_spatial_strength() -> f64 = "spatial_strength";
}

sotf_plugins::serde_param_default! {
    convolution_specs::PARAMS;
    fn default_use_nupc() -> bool = "use_nupc";
    fn default_head_taps() -> usize = "head_taps";
}

sotf_plugins::serde_param_default! {
    pnd_specs::PARAMS;
    fn default_pnd_correction_strength() -> f64 = "correction_strength";
    fn default_pnd_analysis_window_ms() -> f64 = "analysis_window_ms";
    fn default_pnd_drift_smoothing() -> f64 = "drift_smoothing";
    fn default_pnd_multi_channel_analysis() -> bool = "multi_channel_analysis";
    fn default_pnd_confidence_threshold() -> f64 = "confidence_threshold";
}

sotf_plugins::serde_param_default! {
    ab_compare_specs::PARAMS;
    fn default_ab_auto_gain_enabled() -> bool = "auto_gain_enabled";
    fn default_ab_max_auto_gain_db() -> f64 = "max_auto_gain_db";
    fn default_ab_gain_smoothing_ms() -> f64 = "gain_smoothing_ms";
    fn default_ab_mix_transition_ms() -> f64 = "mix_transition_ms";
}

fn default_ab_path_config() -> String {
    r#"{"type":"None"}"#.to_string()
}

sotf_plugins::serde_param_default! {
    band_split_specs::PARAMS;
    fn default_band_split_frequency() -> f64 = "frequency";
}

sotf_plugins::serde_param_default! {
    band_split_specs::PARAMS;
    fn default_band_split_crossover_type() -> String = "crossover_type";
}

sotf_plugins::serde_param_default! {
    band_merge_specs::PARAMS;
    fn default_band_merge_bands() -> usize = "bands";
}

sotf_plugins::serde_param_default! {
    downmix_specs::PARAMS;
    fn default_downmix_center_gain_db() -> f64 = "center_gain_db";
    fn default_downmix_surround_gain_db() -> f64 = "surround_gain_db";
    fn default_downmix_height_gain_db() -> f64 = "height_gain_db";
    fn default_downmix_lfe_gain_db() -> f64 = "lfe_gain_db";
    fn default_downmix_phase_coherence() -> bool = "phase_coherence";
    fn default_downmix_phase_blend_low_hz() -> f64 = "phase_blend_low_hz";
    fn default_downmix_phase_blend_high_hz() -> f64 = "phase_blend_high_hz";
}

sotf_plugins::serde_param_default! {
    mono_to_stereo_specs::PARAMS;
    fn default_mono_to_stereo_width() -> f64 = "stereo_width";
    fn default_mono_to_stereo_haas_delay_ms() -> f64 = "haas_delay_ms";
    fn default_mono_to_stereo_enable_comp_eq() -> bool = "enable_comp_eq";
    fn default_mono_to_stereo_comp_eq_depth_db() -> f64 = "comp_eq_depth_db";
    fn default_mono_to_stereo_decor_low_hz() -> f64 = "decor_low_hz";
    fn default_mono_to_stereo_decor_high_hz() -> f64 = "decor_high_hz";
    fn default_mono_to_stereo_freq_dependent() -> bool = "freq_dependent";
}

sotf_plugins::serde_param_default! {
    crossfeed_specs::PARAMS;
    fn default_crossfeed_enabled() -> bool = "enabled";
    fn default_crossfeed_bauer_fcut_hz() -> f64 = "bauer_fcut_hz";
    fn default_crossfeed_bauer_feed_db() -> f64 = "bauer_feed_db";
    fn default_crossfeed_meier_level() -> f64 = "meier_level";
    fn default_crossfeed_mb_low_freq_hz() -> f64 = "mb_low_freq_hz";
    fn default_crossfeed_mb_mid_high_freq_hz() -> f64 = "mb_mid_high_freq_hz";
    fn default_crossfeed_mb_low_feed_db() -> f64 = "mb_low_feed_db";
    fn default_crossfeed_mb_mid_feed_db() -> f64 = "mb_mid_feed_db";
    fn default_crossfeed_mb_high_feed_db() -> f64 = "mb_high_feed_db";
    fn default_crossfeed_autogain_target_lufs() -> f64 = "autogain_target_lufs";
    fn default_crossfeed_autogain_max_gain_db() -> f64 = "autogain_max_gain_db";
    fn default_crossfeed_autogain_smoothing_ms() -> f64 = "autogain_smoothing_ms";
    fn default_crossfeed_mix() -> f64 = "mix";
    fn default_delay_ms() -> f64 = "delay_ms";
    fn default_delay_feedback() -> f64 = "feedback";
    fn default_delay_mix() -> f64 = "mix";
}

sotf_plugins::serde_param_default! {
    aec_specs::PARAMS;
    fn default_aec_echo_tail_ms() -> f64 = "echo_tail_ms";
    fn default_aec_step_size() -> f64 = "step_size";
    fn default_aec_post_filter_enabled() -> bool = "post_filter_enabled";
}

sotf_plugins::serde_param_default! {
    beamformer_specs::PARAMS;
    fn default_beamformer_num_mics() -> usize = "num_mics";
    fn default_beamformer_mic_spacing_cm() -> f64 = "mic_spacing_cm";
    fn default_beamformer_steer_angle_deg() -> f64 = "steer_angle_deg";
    fn default_beamformer_type() -> usize = "beamformer_type";
}

sotf_plugins::serde_param_default! {
    ambisonics_specs::PARAMS;
    fn default_ambisonics_order() -> usize = "order";
    fn default_ambisonics_target_layout() -> String = "target_layout";
    fn default_ambisonics_max_re() -> bool = "max_re_weighting";
}

sotf_plugins::serde_param_default! {
    cms_specs::PARAMS;
    fn default_cms_dim_gain_db() -> f64 = "dim_gain_db";
    fn default_cms_fade_ms() -> f64 = "fade_ms";
}

sotf_plugins::serde_param_default! {
    spectrum_specs::PARAMS;
    fn default_spectrum_num_bins() -> usize = "num_bins";
    fn default_spectrum_min_freq() -> f32 = "min_freq";
    fn default_spectrum_max_freq() -> f32 = "max_freq";
    fn default_spectrum_smoothing() -> f32 = "smoothing";
}
fn default_spectrum_tilt_correction() -> SpectralTiltCorrection {
    SpectralTiltCorrection::None
}
fn default_spectrum_tilt_reference() -> TiltReferenceFreq {
    TiltReferenceFreq::Standard
}

sotf_plugins::serde_param_default! {
    stereo_imager_specs::PARAMS;
    fn default_si_width() -> f64 = "width";
    fn default_si_low_mid_freq() -> f64 = "low_mid_freq";
    fn default_si_mid_high_freq() -> f64 = "mid_high_freq";
    fn default_si_low_width() -> f64 = "low_width";
    fn default_si_mid_width() -> f64 = "mid_width";
    fn default_si_high_width() -> f64 = "high_width";
    fn default_si_mono_bass() -> bool = "mono_bass";
    fn default_si_mix() -> f64 = "mix";
}

sotf_plugins::serde_param_default! {
    spectral_compressor_specs::PARAMS;
    fn default_sc_threshold() -> f64 = "threshold";
    fn default_sc_ratio() -> f64 = "ratio";
    fn default_sc_attack() -> f64 = "attack";
    fn default_sc_release() -> f64 = "release";
    fn default_sc_knee() -> f64 = "knee";
    fn default_sc_spectral_smoothing() -> f64 = "spectral_smoothing";
    fn default_sc_mix() -> f64 = "mix";
    fn default_sc_fft_size() -> usize = "fft_size";
}

fn default_channels() -> usize {
    2
}

fn default_max_filters() -> usize {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginSettings {
    EQ {
        #[serde(default = "default_channels")]
        channels: usize,
        /// Global filters applied to all channels (used when per_channel_mode is false)
        filters: Vec<EQFilter>,
        /// Per-channel filters (used when per_channel_mode is true)
        /// Index corresponds to channel index
        #[serde(default, skip_serializing_if = "Option::is_none")]
        channel_filters: Option<Vec<Vec<EQFilter>>>,
        /// Whether to use per-channel mode (default: false = all channels share same EQ)
        #[serde(default)]
        per_channel_mode: bool,
        /// Maximum number of filters to display/use in the UI
        #[serde(default = "default_max_filters")]
        max_filters: usize,
        /// Use Transposed Direct Form II for biquad filters
        #[serde(default)]
        tdf2: bool,
        /// Filter topology: 0 = Biquad (default), 1 = SVF (zero-delay feedback)
        #[serde(default)]
        topology: f64,
    },
    Gain {
        #[serde(default = "default_channels")]
        channels: usize,
        gain_db: f64,
        #[serde(default = "default_gain_smoothing_ms")]
        smoothing_ms: f64,
    },
    Upmixer {
        #[serde(deserialize_with = "deserialize_speaker_config")]
        speaker_config: String,
        // Gain parameters (vertical sliders)
        gain_front_direct: f64,
        gain_front_ambient: f64,
        gain_rear_ambient: f64,
        height_gain: f64,
        stereo_width: f64,
        #[serde(default = "default_upmixer_center_spread")]
        center_spread: f64,
        #[serde(default = "default_upmixer_surround_direct_bleed")]
        surround_direct_bleed: f64,
        #[serde(default = "default_upmixer_rear_late_reflection")]
        rear_late_reflection: f64,
        // LFE parameters
        lfe_cutoff_hz: f64,
        lfe_gain: f64,
        bandpass_hz: f64,
        // Sub-harmonic parameters
        #[serde(default)] // false
        enable_subharmonic_synth: bool,
        #[serde(default = "default_upmixer_subharmonic_gain")]
        subharmonic_gain: f64,
        #[serde(default = "default_upmixer_subharmonic_freq_hz")]
        subharmonic_freq_hz: f64,
        #[serde(default = "default_upmixer_subharmonic_attack_ms")]
        subharmonic_attack_ms: f64,
        #[serde(default = "default_upmixer_subharmonic_release_ms")]
        subharmonic_release_ms: f64,
        // Decorrelation parameters
        #[serde(default)] // 0
        decorrelation_mode: usize,
        #[serde(default = "default_upmixer_decorrelation_lfo_rate_hz")]
        decorrelation_lfo_rate_hz: f64,
        #[serde(default = "default_upmixer_velvet_noise_duration_ms")]
        velvet_noise_duration_ms: f64,
        #[serde(default = "default_upmixer_velvet_noise_density")]
        velvet_noise_density: f64,
        // Height parameters
        #[serde(default = "default_upmixer_enable_hr_direct")]
        enable_hr_direct: bool,
        #[serde(default = "default_upmixer_hr_sharpen")]
        hr_sharpen: f64,
        #[serde(default = "default_upmixer_height_hf_cap_hz")]
        height_hf_cap_hz: f64,
        #[serde(default = "default_upmixer_height_transient_reduction")]
        height_transient_reduction: f64,
        #[serde(default = "default_upmixer_height_direct_leak")]
        height_direct_leak: f64,
        // Ambient parameters
        #[serde(default = "default_upmixer_ambient_boost")]
        ambient_boost: f64,
        #[serde(default = "default_upmixer_safety_cap_db")]
        safety_cap_db: f64,
        // Analysis window parameters
        #[serde(default)]
        low_latency: bool,
        #[serde(default = "default_upmixer_frequency_resolution")]
        frequency_resolution: usize,
        #[serde(default = "default_upmixer_rear_ambient_boost")]
        rear_ambient_boost: f64,
        // Dialogue parameters
        #[serde(default = "default_upmixer_dialogue_weight")]
        dialogue_weight: f64,
        #[serde(default = "default_upmixer_voice_freq_min_hz")]
        voice_freq_min_hz: f64,
        #[serde(default = "default_upmixer_voice_freq_max_hz")]
        voice_freq_max_hz: f64,
        #[serde(default = "default_upmixer_dialogue_centroid_weight")]
        dialogue_centroid_weight: f64,
        #[serde(default = "default_upmixer_dialogue_variance_weight")]
        dialogue_variance_weight: f64,
        #[serde(default = "default_upmixer_dialogue_coherence_weight")]
        dialogue_coherence_weight: f64,
        // Diagnostic bypass parameters
        #[serde(default)] // false
        bypass_decorrelation: bool,
        #[serde(default)] // false
        bypass_transient_detection: bool,
        #[serde(default)] // false
        bypass_all_processing: bool,
        // ML vocal detection
        #[serde(default)] // false
        enable_ml_detection: bool,
        // Multi-source extraction (2nd eigenvector)
        #[serde(default)]
        multi_source_extraction: bool,
        #[serde(default = "default_upmixer_multi_source_threshold")]
        multi_source_threshold: f64,
        // Binaural preview (Phase 4G)
        #[serde(default)]
        binaural_preview: bool,
    },
    AAE {
        #[serde(default = "default_aae_speaker_config")]
        speaker_config: String,
        #[serde(default = "default_aae_room_size")]
        room_size: f64,
        #[serde(default = "default_aae_rt60")]
        rt60: f64,
        #[serde(default = "default_aae_bass_ratio")]
        bass_ratio: f64,
        #[serde(default = "default_aae_treble_ratio")]
        treble_ratio: f64,
        #[serde(default = "default_aae_pre_delay_ms")]
        pre_delay_ms: f64,
        #[serde(default = "default_aae_room_preset")]
        room_preset: String,
        #[serde(default = "default_aae_dry_level")]
        dry_level: f64,
        #[serde(default = "default_aae_er_level")]
        er_level: f64,
        #[serde(default = "default_aae_late_level")]
        late_level: f64,
        #[serde(default = "default_aae_lfe_level")]
        lfe_level: f64,
        #[serde(default = "default_aae_mod_depth")]
        mod_depth: f64,
        #[serde(default = "default_aae_er_mod_depth")]
        er_mod_depth: f64,
        #[serde(default = "default_aae_input_diffusion")]
        input_diffusion: f64,
        #[serde(default = "default_aae_envelopment")]
        envelopment: f64,
        #[serde(default = "default_aae_height_amount")]
        height_amount: f64,
        #[serde(default = "default_aae_content_aware")]
        content_aware: bool,
        #[serde(default = "default_aae_dialogue_attenuation_db")]
        dialogue_attenuation_db: f64,
        #[serde(default = "default_aae_safety_limit_db")]
        safety_limit_db: f64,
        #[serde(default)]
        bypass: bool,
        #[serde(default)]
        solo_early: bool,
        #[serde(default)]
        solo_late: bool,
    },
    Compressor {
        threshold_db: f64,
        ratio: f64,
        attack_ms: f64,
        release_ms: f64,
        knee_db: f64,
        makeup_gain_db: f64,
        mix: f64,
        #[serde(default)] // false (matches plugin default)
        auto_makeup: bool,
        #[serde(default = "default_compressor_link_channels")]
        link_channels: bool,
        #[serde(default = "default_compressor_sidechain_hpf_hz")]
        sidechain_hpf_hz: f64,
        #[serde(default = "default_compressor_sidechain_hpf_order")]
        sidechain_hpf_order: String,
        #[serde(default = "default_compressor_detection_mode")]
        detection_mode: String,
        #[serde(default)]
        lookahead_ms: f64,
        #[serde(default)]
        program_dependent_release: bool,
        #[serde(default)]
        measured_auto_makeup: bool,
        #[serde(default)]
        sidechain_external: bool,
    },
    Limiter {
        threshold_db: f64,
        release_ms: f64,
        #[serde(default = "default_limiter_lookahead_ms")]
        lookahead_ms: f64,
        #[serde(default = "default_limiter_soft")]
        soft: bool,
        #[serde(default)]
        true_peak: bool,
        #[serde(default)]
        isp_mode: bool,
        #[serde(default)]
        dual_release: bool,
        #[serde(default = "default_limiter_mix")]
        mix: f64,
        #[serde(default = "default_limiter_link_amount")]
        link_amount: f64,
        #[serde(default)]
        feed_forward: bool,
    },
    Gate {
        threshold_db: f64,
        ratio: f64,
        attack_ms: f64,
        release_ms: f64,
        #[serde(default = "default_gate_hold_ms")]
        hold_ms: f64,
        #[serde(default = "default_gate_mix")]
        mix: f64,
        #[serde(default = "default_gate_link_channels")]
        link_channels: bool,
        #[serde(default)] // 0.0
        sidechain_hpf_hz: f64,
        #[serde(default = "default_gate_sidechain_hpf_order")]
        sidechain_hpf_order: String,
        #[serde(default = "default_gate_detection_mode")]
        detection_mode: String,
        #[serde(default)]
        sidechain_external: bool,
        #[serde(default = "default_gate_range_db")]
        range_db: f64,
        #[serde(default)]
        hysteresis_db: f64,
        #[serde(default)]
        knee_db: f64,
        #[serde(default)]
        lookahead_ms: f64,
    },
    Expander {
        #[serde(default = "default_expander_threshold_db")]
        threshold_db: f64,
        #[serde(default = "default_expander_ratio")]
        ratio: f64,
        #[serde(default = "default_expander_attack_ms")]
        attack_ms: f64,
        #[serde(default = "default_expander_release_ms")]
        release_ms: f64,
        #[serde(default = "default_expander_range_db")]
        range_db: f64,
        #[serde(default = "default_expander_knee_db")]
        knee_db: f64,
        #[serde(default = "default_expander_hysteresis_db")]
        hysteresis_db: f64,
        #[serde(default = "default_expander_hold_ms")]
        hold_ms: f64,
        #[serde(default = "default_expander_mix")]
        mix: f64,
        #[serde(default = "default_expander_link_channels")]
        link_channels: bool,
        #[serde(default = "default_expander_sidechain_hpf_hz")]
        sidechain_hpf_hz: f64,
        #[serde(default)]
        auto_makeup: bool,
        #[serde(default)]
        lookahead_ms: f64,
        #[serde(default = "default_expander_detection_mode")]
        detection_mode: String,
        #[serde(default)]
        measured_auto_makeup: bool,
    },
    MultibandCompressor {
        #[serde(default = "default_mb_compressor_num_bands")]
        num_bands: usize,
        #[serde(default = "default_mb_compressor_crossover_preset")]
        crossover_preset: i32,
        #[serde(default = "default_mb_compressor_crossover_freq_1")]
        crossover_freq_1: f64,
        #[serde(default = "default_mb_compressor_crossover_freq_2")]
        crossover_freq_2: f64,
        #[serde(default = "default_mb_compressor_crossover_freq_3")]
        crossover_freq_3: f64,
        #[serde(default = "default_mb_compressor_crossover_freq_4")]
        crossover_freq_4: f64,
        #[serde(default = "default_mb_compressor_threshold_db")]
        threshold_db: f64,
        #[serde(default = "default_mb_compressor_ratio")]
        ratio: f64,
        #[serde(default = "default_mb_compressor_attack_ms")]
        attack_ms: f64,
        #[serde(default = "default_mb_compressor_release_ms")]
        release_ms: f64,
        #[serde(default = "default_mb_compressor_knee_db")]
        knee_db: f64,
        #[serde(default = "default_mb_compressor_mix")]
        mix: f64,
        #[serde(default = "default_mb_compressor_link_channels")]
        link_channels: bool,
        #[serde(default)]
        per_band_lookahead_ms: f64,
        #[serde(default)]
        ms_mode: bool,
        #[serde(default)]
        bands: Vec<BandCompressorParams>,
        #[serde(default)]
        sidechain_tilt_db: f64,
        #[serde(default = "default_mb_compressor_link_amount")]
        link_amount: f64,
    },
    MultibandExpander {
        #[serde(default = "default_mb_expander_num_bands")]
        num_bands: usize,
        #[serde(default = "default_mb_expander_crossover_preset")]
        crossover_preset: i32,
        #[serde(default = "default_mb_expander_crossover_freq_1")]
        crossover_freq_1: f64,
        #[serde(default = "default_mb_expander_crossover_freq_2")]
        crossover_freq_2: f64,
        #[serde(default = "default_mb_expander_crossover_freq_3")]
        crossover_freq_3: f64,
        #[serde(default = "default_mb_expander_crossover_freq_4")]
        crossover_freq_4: f64,
        #[serde(default = "default_mb_expander_threshold_db")]
        threshold_db: f64,
        #[serde(default = "default_mb_expander_ratio")]
        ratio: f64,
        #[serde(default = "default_mb_expander_attack_ms")]
        attack_ms: f64,
        #[serde(default = "default_mb_expander_release_ms")]
        release_ms: f64,
        #[serde(default = "default_mb_expander_range_db")]
        range_db: f64,
        #[serde(default = "default_mb_expander_knee_db")]
        knee_db: f64,
        #[serde(default = "default_mb_expander_hysteresis_db")]
        hysteresis_db: f64,
        #[serde(default = "default_mb_expander_hold_ms")]
        hold_ms: f64,
        #[serde(default = "default_mb_expander_mix")]
        mix: f64,
        #[serde(default = "default_mb_expander_link_channels")]
        link_channels: bool,
        #[serde(default = "default_mb_expander_detection_mode")]
        detection_mode: String,
        #[serde(default)]
        bands: Vec<BandExpanderParams>,
    },
    LoudnessCompensation {
        low_freq: f64,
        low_gain: f64,
        high_freq: f64,
        high_gain: f64,
        #[serde(default = "default_lc_mid_enabled")]
        mid_enabled: bool,
        #[serde(default = "default_lc_mid_freq")]
        mid_freq: f64,
        #[serde(default = "default_lc_mid_gain")]
        mid_gain: f64,
        #[serde(default = "default_lc_mid_q")]
        mid_q: f64,
        #[serde(default)]
        auto_gain_enabled: bool,
        #[serde(default = "default_auto_gain_max_db")]
        auto_gain_max_db: f64,
        #[serde(default = "default_auto_gain_smoothing_ms")]
        auto_gain_smoothing_ms: f64,
        /// 0 = Manual, 1 = ISO 226, 2 = Auto
        #[serde(default = "default_lc_mode")]
        mode: usize,
        #[serde(default = "default_lc_playback_level_db")]
        playback_level_db: f64,
        #[serde(default = "default_lc_reference_level_db")]
        reference_level_db: f64,
        /// Engine playback volume in dB (used in Auto mode)
        #[serde(default)]
        playback_volume_db: f64,
    },
    FletcherMunson {
        /// Current playback volume (set by engine/UI)
        #[serde(default)]
        playback_volume_db: f64,
        /// Reference level where response is flat
        #[serde(default = "default_fm_reference_level_db")]
        reference_level_db: f64,
        /// Enabled bypass switch
        #[serde(default = "default_fm_enabled")]
        enabled: bool,
        /// Band 1 (sub-bass) parameters
        #[serde(default = "default_fm_band1_freq")]
        band1_freq: f64,
        #[serde(default = "default_fm_band1_q")]
        band1_q: f64,
        #[serde(default = "default_fm_band1_max_gain")]
        band1_max_gain: f64,
        #[serde(default = "default_fm_band1_slope")]
        band1_slope: f64,
        /// Band 2 (mid-bass) parameters
        #[serde(default = "default_fm_band2_freq")]
        band2_freq: f64,
        #[serde(default = "default_fm_band2_q")]
        band2_q: f64,
        #[serde(default = "default_fm_band2_max_gain")]
        band2_max_gain: f64,
        #[serde(default = "default_fm_band2_slope")]
        band2_slope: f64,
        /// Band 3 (presence) parameters
        #[serde(default = "default_fm_band3_freq")]
        band3_freq: f64,
        #[serde(default = "default_fm_band3_q")]
        band3_q: f64,
        #[serde(default = "default_fm_band3_max_gain")]
        band3_max_gain: f64,
        #[serde(default = "default_fm_band3_slope")]
        band3_slope: f64,
        /// Band 4 (air/brilliance) parameters
        #[serde(default = "default_fm_band4_freq")]
        band4_freq: f64,
        #[serde(default = "default_fm_band4_q")]
        band4_q: f64,
        #[serde(default = "default_fm_band4_max_gain")]
        band4_max_gain: f64,
        #[serde(default = "default_fm_band4_slope")]
        band4_slope: f64,
        /// Smoothing time for gain transitions (ms)
        #[serde(default = "default_fm_smoothing_ms")]
        smoothing_ms: f64,
        /// Auto-gain enabled
        #[serde(default)]
        auto_gain_enabled: bool,
        /// Auto-gain maximum correction in dB
        #[serde(default = "default_fm_auto_gain_max_db")]
        auto_gain_max_db: f64,
        /// Auto-gain smoothing time in ms
        #[serde(default = "default_fm_auto_gain_smoothing_ms")]
        auto_gain_smoothing_ms: f64,
        /// Auto-gain loudness type (0 = Momentary, 1 = ShortTerm)
        #[serde(default)]
        auto_gain_loudness_type: i32,
        /// Use ISO 226:2003 equal-loudness contours
        #[serde(default)]
        iso_226: bool,
    },
    BinauralDecoder {
        sofa_file: String,
        input_channels: usize,
        #[serde(default = "default_binaural_enable_optimization")]
        enable_optimization: bool,
        #[serde(default)] // 0.0
        externalization: f64,
        #[serde(default)] // 0.0
        near_field_strength: f64,
        #[serde(default)] // 0 = Linear
        crossfade_mode: usize,
        // Phase 4E: Late reverb
        #[serde(default)]
        late_reverb_enabled: bool,
        #[serde(default = "default_binaural_late_reverb_mix")]
        late_reverb_mix: f64,
        #[serde(default = "default_binaural_late_reverb_rt60")]
        late_reverb_rt60: f64,
        #[serde(default = "default_binaural_late_reverb_damping")]
        late_reverb_damping: f64,
        #[serde(default)]
        headphone_eq_enabled: bool,
    },
    Convolution {
        ir_file: String,
        mix: f64,
        gain_db: f64,
        #[serde(default = "default_use_nupc")]
        use_nupc: bool,
        #[serde(default)]
        zero_latency_head: bool,
        #[serde(default = "default_head_taps")]
        head_taps: usize,
    },
    LoudnessMonitor,
    SpectrumAnalyzer {
        #[serde(default = "default_spectrum_num_bins")]
        num_bins: usize,
        #[serde(default = "default_spectrum_min_freq")]
        min_freq: f32,
        #[serde(default = "default_spectrum_max_freq")]
        max_freq: f32,
        #[serde(default = "default_spectrum_smoothing")]
        smoothing: f32,
        #[serde(default = "default_spectrum_tilt_correction")]
        tilt_correction: SpectralTiltCorrection,
        #[serde(default = "default_spectrum_tilt_reference")]
        tilt_reference: TiltReferenceFreq,
    },
    ChannelMuteSolo {
        enabled: bool,
        #[serde(default = "default_cms_dim_gain_db")]
        dim_gain_db: f64,
        #[serde(default = "default_cms_fade_ms")]
        fade_ms: f64,
        channel_states: Vec<sotf_plugins::ChannelState>,
    },
    Matrix {
        input_channels: usize,
        output_channels: usize,
        matrix: Vec<f32>, // Row-major: matrix[out * in_count + in] = linear_gain
        #[serde(default)]
        channel_states: Vec<sotf_plugins::ChannelState>,
    },
    XTC {
        #[serde(default = "default_xtc_distance_m")]
        distance_m: f64,
        #[serde(default = "default_xtc_speaker_angle_deg")]
        speaker_angle_deg: f64,
        #[serde(default = "default_xtc_head_radius_m")]
        head_radius_m: f64,
        #[serde(default = "default_xtc_beta_base")]
        beta_base: f64,
        #[serde(default = "default_xtc_beta_low_freq_boost")]
        beta_low_freq_boost: f64,
        #[serde(default = "default_xtc_beta_high_freq_boost")]
        beta_high_freq_boost: f64,
        #[serde(default = "default_xtc_head_shadow_cutoff_hz")]
        head_shadow_cutoff_hz: f64,
        #[serde(default = "default_xtc_head_shadow_slope")]
        head_shadow_slope_db_per_octave: f64,
        #[serde(default = "default_xtc_max_gain_db")]
        max_gain_db: f64,
        #[serde(default)]
        head_offset_x: f64,
        #[serde(default)]
        head_offset_z: f64,
        #[serde(default)]
        head_yaw_deg: f64,
        #[serde(default = "default_xtc_head_tracking_smooth_s")]
        head_tracking_smooth_s: f64,
        #[serde(default = "default_xtc_spectral_normalization")]
        spectral_normalization: bool,
        #[serde(default = "default_xtc_room_reflections_enabled")]
        room_reflections_enabled: bool,
        #[serde(default)]
        room_ir_file: Option<String>,
        #[serde(default = "default_xtc_room_width")]
        room_width_m: f64,
        #[serde(default = "default_xtc_room_depth")]
        room_depth_m: f64,
        #[serde(default = "default_xtc_wall_absorption")]
        wall_absorption: f64,
        #[serde(default = "default_xtc_reflection_beta_boost")]
        reflection_beta_boost: f64,
        #[serde(default)]
        bypass_xtc_filters: bool,
        #[serde(default)]
        bypass_spectral_normalization: bool,
        #[serde(default)]
        bypass_neumann_refinement: bool,
        #[serde(default = "default_xtc_auto_gain_enabled")]
        auto_gain_enabled: bool,
        #[serde(default = "default_xtc_auto_gain_max_db")]
        auto_gain_max_db: f64,
        #[serde(default = "default_xtc_auto_gain_smoothing_ms")]
        auto_gain_smoothing_ms: f64,
        #[serde(default = "default_xtc_pinna_model_enabled")]
        pinna_model_enabled: bool,
        #[serde(default)]
        head_model: f64,
    },
    Denoiser {
        #[serde(default = "default_denoiser_reduction_db")]
        reduction_db: f64,
        #[serde(default = "default_denoiser_floor_db")]
        floor_db: f64,
        #[serde(default = "default_denoiser_smoothing")]
        smoothing: f64,
        #[serde(default = "default_denoiser_attack_ms")]
        attack_ms: f64,
        #[serde(default = "default_denoiser_release_ms")]
        release_ms: f64,
        #[serde(default = "default_denoiser_low_latency")]
        low_latency: bool,
        #[serde(default = "default_denoiser_polyphonic_detection")]
        polyphonic_detection: bool,
        #[serde(default = "default_denoiser_crack_sensitivity")]
        crack_sensitivity: f64,
        #[serde(default = "default_denoiser_mcra_alpha_s")]
        mcra_alpha_s: f64,
        #[serde(default = "default_denoiser_mcra_alpha_p")]
        mcra_alpha_p: f64,
        #[serde(default = "default_denoiser_mcra_l")]
        mcra_l: usize,
        #[serde(default = "default_denoiser_mcra_delta")]
        mcra_delta: f64,
        #[serde(default = "default_denoiser_transparency")]
        transparency: f64,
        #[serde(default = "default_denoiser_dd_enabled")]
        dd_enabled: bool,
        #[serde(default = "default_denoiser_dd_alpha")]
        dd_alpha: f64,
        #[serde(default = "default_denoiser_psychoacoustic_masking")]
        psychoacoustic_masking: bool,
        #[serde(default = "default_denoiser_transient_enabled")]
        transient_enabled: bool,
        #[serde(default = "default_denoiser_spectral_smoothing_enabled")]
        spectral_smoothing_enabled: bool,
        #[serde(default = "default_denoiser_temporal_smoothing_enabled")]
        temporal_smoothing_enabled: bool,
        #[serde(default = "default_denoiser_hiss_enabled")]
        hiss_enabled: bool,
        #[serde(default = "default_denoiser_hiss_threshold_db")]
        hiss_threshold_db: f64,
        #[serde(default = "default_denoiser_hiss_frequency_hz")]
        hiss_frequency_hz: f64,
        #[serde(default = "default_denoiser_hiss_strength")]
        hiss_strength: f64,
        #[serde(default = "default_denoiser_spectral_sub_enabled")]
        spectral_sub_enabled: bool,
        #[serde(default = "default_denoiser_spectral_sub_alpha")]
        spectral_sub_alpha: f64,
        #[serde(default = "default_denoiser_spectral_sub_beta")]
        spectral_sub_beta: f64,
        #[serde(default)]
        learn_noise: bool,
        #[serde(default = "default_denoiser_use_captured_profile")]
        use_captured_profile: bool,
        #[serde(default)]
        clear_profile: bool,
        #[serde(default = "default_denoiser_algorithm")]
        algorithm: usize,
        #[serde(default)]
        formant_preservation: bool,
        #[serde(default = "default_denoiser_formant_strength")]
        formant_strength: f64,
        #[serde(default)]
        multi_resolution: bool,
        #[serde(default)]
        harmonic_percussive: bool,
        #[serde(default)]
        spatial_denoise: bool,
        #[serde(default = "default_spatial_strength")]
        spatial_strength: f64,
    },
    Pnd {
        #[serde(default = "default_pnd_correction_strength")]
        correction_strength: f64,
        #[serde(default = "default_pnd_analysis_window_ms")]
        analysis_window_ms: f64,
        #[serde(default = "default_pnd_drift_smoothing")]
        drift_smoothing: f64,
        #[serde(default = "default_pnd_multi_channel_analysis")]
        multi_channel_analysis: bool,
        #[serde(default = "default_pnd_confidence_threshold")]
        confidence_threshold: f64,
        #[serde(default)]
        phase_vocoder: bool,
    },
    ABCompare {
        /// A/B mix: -1.0 = A only, 0.0 = 50/50, 1.0 = B only
        #[serde(default)]
        mix: f64,
        /// Mix mode: 0 = potentiometer (continuous), 1 = binary (A or B)
        #[serde(default)]
        mix_mode: i32,
        /// Selected path in binary mode: 0 = A, 1 = B
        #[serde(default)]
        selected_path: i32,
        /// Bypass: output original input
        #[serde(default)]
        bypass: bool,
        /// Enable automatic loudness matching
        #[serde(default = "default_ab_auto_gain_enabled")]
        auto_gain_enabled: bool,
        /// Loudness measurement type: 0 = momentary (400ms), 1 = short-term (3s)
        #[serde(default)]
        loudness_type: i32,
        /// Maximum auto-gain adjustment in dB
        #[serde(default = "default_ab_max_auto_gain_db")]
        max_auto_gain_db: f64,
        /// Gain smoothing time in ms
        #[serde(default = "default_ab_gain_smoothing_ms")]
        gain_smoothing_ms: f64,
        /// Mix transition time in ms
        #[serde(default = "default_ab_mix_transition_ms")]
        mix_transition_ms: f64,
        /// Path A configuration (JSON)
        #[serde(default = "default_ab_path_config")]
        path_a_config: String,
        /// Path B configuration (JSON)
        #[serde(default = "default_ab_path_config")]
        path_b_config: String,
        /// Path A config source file (for display only)
        #[serde(default)]
        path_a_file: String,
        /// Path B config source file (for display only)
        #[serde(default)]
        path_b_file: String,
        #[serde(default)]
        phase_invert_a: bool,
        #[serde(default)]
        phase_invert_b: bool,
        #[serde(default)]
        difference_mode: bool,
    },
    BandSplit {
        /// Number of input channels
        #[serde(default = "default_channels")]
        channels: usize,
        /// Crossover frequency in Hz
        #[serde(default = "default_band_split_frequency")]
        frequency: f64,
        /// Crossover type: "LR24" or "LR48"
        #[serde(default = "default_band_split_crossover_type")]
        crossover_type: String,
    },
    BandMerge {
        /// Number of output channels
        #[serde(default = "default_channels")]
        channels: usize,
        /// Number of bands to merge
        #[serde(default = "default_band_merge_bands")]
        bands: usize,
    },
    Downmix {
        #[serde(default = "default_channels")]
        input_channels: usize,
        #[serde(default = "default_downmix_center_gain_db")]
        center_gain_db: f64,
        #[serde(default = "default_downmix_surround_gain_db")]
        surround_gain_db: f64,
        #[serde(default = "default_downmix_height_gain_db")]
        height_gain_db: f64,
        #[serde(default = "default_downmix_lfe_gain_db")]
        lfe_gain_db: f64,
        #[serde(default = "default_downmix_phase_coherence")]
        phase_coherence: bool,
        #[serde(default = "default_downmix_phase_blend_low_hz")]
        phase_blend_low_hz: f64,
        #[serde(default = "default_downmix_phase_blend_high_hz")]
        phase_blend_high_hz: f64,
        #[serde(default)]
        itu_mode: bool,
    },
    MonoToStereo {
        #[serde(default = "default_mono_to_stereo_width")]
        stereo_width: f64,
        #[serde(default = "default_mono_to_stereo_haas_delay_ms")]
        haas_delay_ms: f64,
        #[serde(default = "default_mono_to_stereo_enable_comp_eq")]
        enable_comp_eq: bool,
        #[serde(default = "default_mono_to_stereo_comp_eq_depth_db")]
        comp_eq_depth_db: f64,
        #[serde(default = "default_mono_to_stereo_decor_low_hz")]
        decor_low_hz: f64,
        #[serde(default = "default_mono_to_stereo_decor_high_hz")]
        decor_high_hz: f64,
        #[serde(default = "default_mono_to_stereo_freq_dependent")]
        freq_dependent: bool,
    },
    Crossfeed {
        #[serde(default)]
        mode: CrossfeedMode,
        #[serde(default)]
        preset: CrossfeedPreset,
        #[serde(default = "default_crossfeed_enabled")]
        enabled: bool,
        #[serde(default = "default_crossfeed_mix")]
        mix: f64,
        // Bauer
        #[serde(default = "default_crossfeed_bauer_fcut_hz")]
        bauer_fcut_hz: f64,
        #[serde(default = "default_crossfeed_bauer_feed_db")]
        bauer_feed_db: f64,
        // Meier
        #[serde(default = "default_crossfeed_meier_level")]
        meier_level: f64,
        // Multiband
        #[serde(default = "default_crossfeed_mb_low_freq_hz")]
        mb_low_freq_hz: f64,
        #[serde(default = "default_crossfeed_mb_mid_high_freq_hz")]
        mb_mid_high_freq_hz: f64,
        #[serde(default = "default_crossfeed_mb_low_feed_db")]
        mb_low_feed_db: f64,
        #[serde(default = "default_crossfeed_mb_mid_feed_db")]
        mb_mid_feed_db: f64,
        #[serde(default = "default_crossfeed_mb_high_feed_db")]
        mb_high_feed_db: f64,
        // ITD
        #[serde(default)]
        itd_delay_ms: f64,
        // Auto gain
        #[serde(default)]
        autogain_enabled: bool,
        #[serde(default = "default_crossfeed_autogain_target_lufs")]
        autogain_target_lufs: f64,
        #[serde(default = "default_crossfeed_autogain_max_gain_db")]
        autogain_max_gain_db: f64,
        #[serde(default = "default_crossfeed_autogain_smoothing_ms")]
        autogain_smoothing_ms: f64,
    },
    Delay {
        #[serde(default = "default_delay_ms")]
        delay_ms: f64,
        #[serde(default = "default_delay_feedback")]
        feedback: f64,
        #[serde(default = "default_delay_mix")]
        mix: f64,
        #[serde(default)]
        lfo_rate_hz: f64,
        #[serde(default)]
        lfo_depth_ms: f64,
        #[serde(default)]
        allpass_feedback: bool,
    },
    Aec {
        #[serde(default = "default_aec_echo_tail_ms")]
        echo_tail_ms: f64,
        #[serde(default = "default_aec_step_size")]
        step_size: f64,
        #[serde(default = "default_aec_post_filter_enabled")]
        post_filter_enabled: bool,
    },
    Beamformer {
        #[serde(default = "default_beamformer_num_mics")]
        num_mics: usize,
        #[serde(default = "default_beamformer_mic_spacing_cm")]
        mic_spacing_cm: f64,
        #[serde(default = "default_beamformer_steer_angle_deg")]
        steer_angle_deg: f64,
        #[serde(default = "default_beamformer_type")]
        beamformer_type: usize,
    },
    AmbisonicsDecoder {
        #[serde(default = "default_ambisonics_order")]
        order: usize,
        #[serde(default = "default_ambisonics_target_layout")]
        target_layout: String,
        #[serde(default = "default_ambisonics_max_re")]
        max_re_weighting: bool,
        #[serde(default)]
        dual_band: bool,
    },
    StereoImager {
        #[serde(default = "default_si_width")]
        width: f64,
        #[serde(default = "default_si_low_mid_freq")]
        low_mid_freq: f64,
        #[serde(default = "default_si_mid_high_freq")]
        mid_high_freq: f64,
        #[serde(default = "default_si_low_width")]
        low_width: f64,
        #[serde(default = "default_si_mid_width")]
        mid_width: f64,
        #[serde(default = "default_si_high_width")]
        high_width: f64,
        #[serde(default = "default_si_mono_bass")]
        mono_bass: bool,
        #[serde(default = "default_si_mix")]
        mix: f64,
    },
    DeEsser {
        #[serde(default = "default_de_esser_frequency")]
        frequency: f64,
        #[serde(default = "default_de_esser_q")]
        q: f64,
        #[serde(default = "default_de_esser_threshold")]
        threshold: f64,
        #[serde(default = "default_de_esser_ratio")]
        ratio: f64,
        #[serde(default = "default_de_esser_attack")]
        attack: f64,
        #[serde(default = "default_de_esser_release")]
        release: f64,
        #[serde(default = "default_de_esser_mode")]
        mode: String,
        #[serde(default = "default_de_esser_mix")]
        mix: f64,
    },
    TransientShaper {
        #[serde(default)]
        attack: f64,
        #[serde(default)]
        sustain: f64,
        #[serde(default)]
        sensitivity_db: f64,
        #[serde(default)]
        output_gain_db: f64,
        #[serde(default = "default_ts_mix")]
        mix: f64,
    },
    Saturation {
        #[serde(default = "default_sat_mode")]
        mode: f64,
        #[serde(default = "default_sat_drive")]
        drive: f64,
        #[serde(default = "default_sat_tone")]
        tone: f64,
        #[serde(default = "default_sat_exciter_freq")]
        exciter_freq: f64,
        #[serde(default = "default_sat_oversampling")]
        oversampling: f64,
        #[serde(default = "default_sat_output_gain")]
        output_gain_db: f64,
        #[serde(default = "default_sat_mix")]
        mix: f64,
        #[serde(default)]
        dynamic_amount: f64,
        #[serde(default = "default_sat_dynamic_attack_ms")]
        dynamic_attack_ms: f64,
        #[serde(default = "default_sat_dynamic_release_ms")]
        dynamic_release_ms: f64,
        #[serde(default = "default_sat_dc_blocker")]
        dc_blocker: bool,
        #[serde(default = "default_sat_use_adaa")]
        use_adaa: bool,
    },
    DynamicEq {
        #[serde(default = "default_dyneq_num_bands")]
        num_bands: f64,
        #[serde(default = "default_dyneq_threshold")]
        threshold: f64,
        #[serde(default = "default_dyneq_ratio")]
        ratio: f64,
        #[serde(default = "default_dyneq_attack")]
        attack: f64,
        #[serde(default = "default_dyneq_release")]
        release: f64,
        #[serde(default = "default_dyneq_knee")]
        knee: f64,
        #[serde(default = "default_dyneq_link_channels")]
        link_channels: bool,
        #[serde(default = "default_dyneq_mix")]
        mix: f64,
    },
    LinearPhaseEq {
        #[serde(default = "default_lpeq_num_filters")]
        num_filters: f64,
        #[serde(default = "default_lpeq_fir_length")]
        fir_length: f64,
        #[serde(default)]
        auto_gain: bool,
        #[serde(default = "default_lpeq_mix")]
        mix: f64,
    },
    SpectralCompressor {
        #[serde(default = "default_sc_fft_size")]
        fft_size: usize,
        #[serde(default = "default_sc_threshold")]
        threshold: f64,
        #[serde(default = "default_sc_ratio")]
        ratio: f64,
        #[serde(default = "default_sc_attack")]
        attack: f64,
        #[serde(default = "default_sc_release")]
        release: f64,
        #[serde(default = "default_sc_knee")]
        knee: f64,
        #[serde(default = "default_sc_spectral_smoothing")]
        spectral_smoothing: f64,
        #[serde(default = "default_sc_mix")]
        mix: f64,
        #[serde(default)]
        target_mode: f64,
        #[serde(default)]
        delta_listen: bool,
        // Phase 4A: Adaptive threshold
        #[serde(default)]
        adaptive_threshold: bool,
        #[serde(default)]
        adaptive_offset_db: f64,
    },
}

sotf_plugins::serde_param_default! {
    transient_shaper_specs::PARAMS;
    fn default_ts_mix() -> f64 = "mix";
}

sotf_plugins::serde_param_default! {
    saturation_specs::PARAMS;
    fn default_sat_mode() -> f64 = "mode";
    fn default_sat_drive() -> f64 = "drive";
    fn default_sat_tone() -> f64 = "tone";
    fn default_sat_exciter_freq() -> f64 = "exciter_freq";
    fn default_sat_oversampling() -> f64 = "oversampling";
    fn default_sat_output_gain() -> f64 = "output_gain";
    fn default_sat_mix() -> f64 = "mix";
    fn default_sat_dynamic_attack_ms() -> f64 = "dynamic_attack_ms";
    fn default_sat_dynamic_release_ms() -> f64 = "dynamic_release_ms";
    fn default_sat_dc_blocker() -> bool = "dc_blocker";
    fn default_sat_use_adaa() -> bool = "use_adaa";
}

sotf_plugins::serde_param_default! {
    dynamic_eq_specs::PARAMS;
    fn default_dyneq_num_bands() -> f64 = "num_bands";
    fn default_dyneq_threshold() -> f64 = "threshold";
    fn default_dyneq_ratio() -> f64 = "ratio";
    fn default_dyneq_attack() -> f64 = "attack";
    fn default_dyneq_release() -> f64 = "release";
    fn default_dyneq_knee() -> f64 = "knee";
    fn default_dyneq_link_channels() -> bool = "link_channels";
    fn default_dyneq_mix() -> f64 = "mix";
}

sotf_plugins::serde_param_default! {
    linear_phase_eq_specs::PARAMS;
    fn default_lpeq_num_filters() -> f64 = "num_filters";
    fn default_lpeq_fir_length() -> f64 = "fir_length";
    fn default_lpeq_mix() -> f64 = "mix";
}

impl PluginSettings {
    pub fn plugin_type(&self) -> PluginType {
        match self {
            Self::EQ { .. } => PluginType::EQ,
            Self::Gain { .. } => PluginType::Gain,
            Self::Upmixer { .. } => PluginType::Upmixer,
            Self::Compressor { .. } => PluginType::Compressor,
            Self::Limiter { .. } => PluginType::Limiter,
            Self::Gate { .. } => PluginType::Gate,
            Self::Expander { .. } => PluginType::Expander,
            Self::MultibandCompressor { .. } => PluginType::MultibandCompressor,
            Self::MultibandExpander { .. } => PluginType::MultibandExpander,
            Self::LoudnessCompensation { .. } => PluginType::LoudnessCompensation,
            Self::FletcherMunson { .. } => PluginType::FletcherMunson,
            Self::BinauralDecoder { .. } => PluginType::BinauralDecoder,
            Self::Convolution { .. } => PluginType::Convolution,
            Self::LoudnessMonitor => PluginType::LoudnessMonitor,
            Self::SpectrumAnalyzer { .. } => PluginType::SpectrumAnalyzer,
            Self::ChannelMuteSolo { .. } => PluginType::ChannelMuteSolo,
            Self::Matrix { .. } => PluginType::Matrix,
            Self::XTC { .. } => PluginType::XTC,
            Self::Denoiser { .. } => PluginType::Denoiser,
            Self::Pnd { .. } => PluginType::Pnd,
            Self::ABCompare { .. } => PluginType::ABCompare,
            Self::BandSplit { .. } => PluginType::BandSplit,
            Self::BandMerge { .. } => PluginType::BandMerge,
            Self::Downmix { .. } => PluginType::Downmix,
            Self::MonoToStereo { .. } => PluginType::MonoToStereo,
            Self::Crossfeed { .. } => PluginType::Crossfeed,
            Self::Delay { .. } => PluginType::Delay,
            Self::Aec { .. } => PluginType::Aec,
            Self::Beamformer { .. } => PluginType::Beamformer,
            Self::AmbisonicsDecoder { .. } => PluginType::AmbisonicsDecoder,
            Self::StereoImager { .. } => PluginType::StereoImager,
            Self::DeEsser { .. } => PluginType::DeEsser,
            Self::TransientShaper { .. } => PluginType::TransientShaper,
            Self::Saturation { .. } => PluginType::Saturation,
            Self::DynamicEq { .. } => PluginType::DynamicEq,
            Self::LinearPhaseEq { .. } => PluginType::LinearPhaseEq,
            Self::SpectralCompressor { .. } => PluginType::SpectralCompressor,
            Self::AAE { .. } => PluginType::AAE,
        }
    }

    /// Returns the fixed input channel count this plugin requires, or None if it adapts to any.
    pub fn required_input_channels(&self) -> Option<usize> {
        match self {
            Self::Upmixer { .. } => Some(2),
            Self::AAE { .. } => Some(2),
            Self::StereoImager { .. } => Some(2),
            Self::XTC { .. } => Some(2),
            Self::Crossfeed { .. } => Some(2),
            Self::MonoToStereo { .. } => Some(1),
            Self::Aec { .. } => Some(2),
            Self::Beamformer { num_mics, .. } => Some(*num_mics),
            _ => None,
        }
    }

    pub fn to_plugin_config(&self, sample_rate: f64) -> PluginConfig {
        match self {
            Self::Crossfeed {
                mode,
                preset,
                enabled,
                mix,
                bauer_fcut_hz,
                bauer_feed_db,
                meier_level,
                mb_low_freq_hz,
                mb_mid_high_freq_hz,
                mb_low_feed_db,
                mb_mid_feed_db,
                mb_high_feed_db,
                itd_delay_ms,
                autogain_enabled,
                autogain_target_lufs,
                autogain_max_gain_db,
                autogain_smoothing_ms,
            } => PluginConfig::new(
                "crossfeed",
                json!({
                    "mode": mode,
                    "preset": preset,
                    "enabled": enabled,
                    "mix": mix,
                    "bauer_fcut_hz": bauer_fcut_hz,
                    "bauer_feed_db": bauer_feed_db,
                    "meier_level": meier_level,
                    "mb_low_freq_hz": mb_low_freq_hz,
                    "mb_mid_high_freq_hz": mb_mid_high_freq_hz,
                    "mb_low_feed_db": mb_low_feed_db,
                    "mb_mid_feed_db": mb_mid_feed_db,
                    "mb_high_feed_db": mb_high_feed_db,
                    "itd_delay_ms": itd_delay_ms,
                    "autogain_enabled": autogain_enabled,
                    "autogain_target_lufs": autogain_target_lufs,
                    "autogain_max_gain_db": autogain_max_gain_db,
                    "autogain_smoothing_ms": autogain_smoothing_ms,
                }),
            ),
            Self::Delay {
                delay_ms,
                feedback,
                mix,
                lfo_rate_hz,
                lfo_depth_ms,
                allpass_feedback,
            } => PluginConfig::new(
                "delay",
                json!({
                    "delay_ms": delay_ms,
                    "feedback": feedback,
                    "mix": mix,
                    "lfo_rate_hz": lfo_rate_hz,
                    "lfo_depth_ms": lfo_depth_ms,
                    "allpass_feedback": allpass_feedback,
                }),
            ),
            Self::Aec {
                echo_tail_ms,
                step_size,
                post_filter_enabled,
            } => PluginConfig::new(
                "aec",
                json!({
                    "echo_tail_ms": *echo_tail_ms as f32,
                    "step_size": *step_size as f32,
                    "post_filter_enabled": post_filter_enabled,
                }),
            ),
            Self::Beamformer {
                num_mics,
                mic_spacing_cm,
                steer_angle_deg,
                beamformer_type,
            } => PluginConfig::new(
                "beamformer",
                json!({
                    "num_mics": num_mics,
                    "mic_spacing_cm": *mic_spacing_cm as f32,
                    "steer_angle_deg": *steer_angle_deg as f32,
                    "beamformer_type": beamformer_type,
                }),
            ),
            Self::AmbisonicsDecoder {
                order,
                target_layout,
                max_re_weighting,
                dual_band,
            } => PluginConfig::new(
                "ambisonics_decoder",
                json!({
                    "order": order,
                    "target_layout": target_layout,
                    "max_re_weighting": max_re_weighting,
                    "dual_band": dual_band,
                }),
            ),
            Self::StereoImager {
                width,
                low_mid_freq,
                mid_high_freq,
                low_width,
                mid_width,
                high_width,
                mono_bass,
                mix,
            } => PluginConfig::new(
                "stereo_imager",
                json!({
                    "width": *width as f32,
                    "low_mid_freq": *low_mid_freq as f32,
                    "mid_high_freq": *mid_high_freq as f32,
                    "low_width": *low_width as f32,
                    "mid_width": *mid_width as f32,
                    "high_width": *high_width as f32,
                    "mono_bass": mono_bass,
                    "mix": *mix as f32,
                }),
            ),
            Self::DeEsser {
                frequency,
                q,
                threshold,
                ratio,
                attack,
                release,
                mode,
                mix,
            } => PluginConfig::new(
                "de_esser",
                json!({
                    "frequency": *frequency as f32,
                    "q": *q as f32,
                    "threshold": *threshold as f32,
                    "ratio": *ratio as f32,
                    "attack_ms": *attack as f32,
                    "release_ms": *release as f32,
                    "mode": mode,
                    "mix": *mix as f32,
                }),
            ),
            Self::TransientShaper {
                attack,
                sustain,
                sensitivity_db,
                output_gain_db,
                mix,
            } => PluginConfig::new(
                "transient_shaper",
                json!({
                    "attack": *attack as f32,
                    "sustain": *sustain as f32,
                    "sensitivity_db": *sensitivity_db as f32,
                    "output_gain_db": *output_gain_db as f32,
                    "mix": *mix as f32,
                }),
            ),
            Self::Saturation {
                mode,
                drive,
                tone,
                exciter_freq,
                oversampling,
                output_gain_db,
                mix,
                ..
            } => {
                let mode_str = saturation_specs::MODES
                    .get(*mode as usize)
                    .unwrap_or(&"Soft Clip");
                let os_str = saturation_specs::OVERSAMPLING_OPTIONS
                    .get(*oversampling as usize)
                    .unwrap_or(&"Off");
                PluginConfig::new(
                    "saturation",
                    json!({
                        "mode": mode_str,
                        "drive": *drive as f32,
                        "tone": *tone as f32,
                        "exciter_freq": *exciter_freq as f32,
                        "oversampling": os_str,
                        "output_gain_db": *output_gain_db as f32,
                        "mix": *mix as f32,
                    }),
                )
            }
            Self::DynamicEq {
                num_bands,
                threshold,
                ratio,
                attack,
                release,
                knee,
                link_channels,
                mix,
            } => PluginConfig::new(
                "dynamic_eq",
                json!({
                    "num_bands": *num_bands as usize,
                    "threshold": *threshold as f32,
                    "ratio": *ratio as f32,
                    "attack_ms": *attack as f32,
                    "release_ms": *release as f32,
                    "knee": *knee as f32,
                    "link_channels": link_channels,
                    "mix": *mix as f32,
                }),
            ),
            Self::LinearPhaseEq {
                num_filters,
                fir_length,
                auto_gain,
                mix,
            } => PluginConfig::new(
                "linear_phase_eq",
                json!({
                    "num_filters": *num_filters as usize,
                    "fir_length_index": *fir_length as usize,
                    "auto_gain": auto_gain,
                    "mix": *mix as f32,
                }),
            ),
            Self::SpectralCompressor {
                fft_size,
                threshold,
                ratio,
                attack,
                release,
                knee,
                spectral_smoothing,
                mix,
                target_mode,
                delta_listen,
                adaptive_threshold,
                adaptive_offset_db,
            } => PluginConfig::new(
                "spectral_compressor",
                json!({
                    "fft_size_index": *fft_size,
                    "threshold_db": *threshold as f32,
                    "ratio": *ratio as f32,
                    "attack_ms": *attack as f32,
                    "release_ms": *release as f32,
                    "knee_db": *knee as f32,
                    "spectral_smoothing": *spectral_smoothing as f32,
                    "mix": *mix as f32,
                    "target_mode": *target_mode as usize,
                    "delta_listen": delta_listen,
                    "adaptive_threshold": adaptive_threshold,
                    "adaptive_offset_db": adaptive_offset_db,
                }),
            ),
            Self::EQ {
                channels,
                filters,
                channel_filters,
                per_channel_mode,
                max_filters: _,
                tdf2,
                topology: _,
            } => {
                // Helper to convert filters with mute/solo logic
                let convert_filters = |filters: &[EQFilter]| -> Vec<serde_json::Value> {
                    let any_soloed = filters.iter().any(|f| f.solo);
                    filters
                        .iter()
                        .filter(|f| {
                            if f.muted {
                                return false;
                            }
                            if any_soloed && !f.solo {
                                return false;
                            }
                            true
                        })
                        .map(|f| {
                            let bq = f.to_biquad(sample_rate);
                            json!({
                                "filter_type": bq.filter_type.long_name().to_lowercase(),
                                "freq": bq.freq,
                                "q": bq.q,
                                "db_gain": bq.db_gain,
                            })
                        })
                        .collect()
                };

                if *per_channel_mode {
                    // Per-channel mode: send channel_filters to the plugin
                    if let Some(ch_filters) = channel_filters {
                        let channel_filter_configs: Vec<Vec<serde_json::Value>> =
                            ch_filters.iter().map(|f| convert_filters(f)).collect();

                        PluginConfig::new(
                            "eq",
                            json!({
                                "channels": channels,
                                "channel_filters": channel_filter_configs,
                                "tdf2": tdf2,
                            }),
                        )
                    } else {
                        // Fallback to global filters if channel_filters is None
                        let filter_configs = convert_filters(filters);
                        PluginConfig::new(
                            "eq",
                            json!({
                                "channels": channels,
                                "filters": filter_configs,
                                "tdf2": tdf2,
                            }),
                        )
                    }
                } else {
                    // Global mode: all channels share same EQ
                    let filter_configs = convert_filters(filters);
                    PluginConfig::new(
                        "eq",
                        json!({
                            "channels": channels,
                            "filters": filter_configs,
                            "tdf2": tdf2,
                        }),
                    )
                }
            }
            Self::Gain {
                channels,
                gain_db,
                smoothing_ms,
            } => PluginConfig::new(
                "gain",
                json!({
                    "channels": channels,
                    "gain_db": gain_db,
                    "smoothing_ms": smoothing_ms,
                }),
            ),
            Self::Upmixer {
                speaker_config,
                gain_front_direct,
                gain_front_ambient,
                gain_rear_ambient,
                height_gain,
                stereo_width,
                center_spread,
                surround_direct_bleed,
                rear_late_reflection,
                lfe_cutoff_hz,
                lfe_gain,
                bandpass_hz,
                enable_subharmonic_synth,
                subharmonic_gain,
                subharmonic_freq_hz,
                subharmonic_attack_ms,
                subharmonic_release_ms,
                decorrelation_mode,
                decorrelation_lfo_rate_hz,
                velvet_noise_duration_ms,
                velvet_noise_density,
                enable_hr_direct,
                hr_sharpen,
                height_hf_cap_hz,
                height_transient_reduction,
                height_direct_leak,
                ambient_boost,
                safety_cap_db,
                low_latency,
                frequency_resolution,
                rear_ambient_boost,
                dialogue_weight,
                voice_freq_min_hz,
                voice_freq_max_hz,
                dialogue_centroid_weight,
                dialogue_variance_weight,
                dialogue_coherence_weight,
                bypass_decorrelation,
                bypass_transient_detection,
                bypass_all_processing,
                enable_ml_detection,
                multi_source_extraction,
                multi_source_threshold,
                binaural_preview,
            } => PluginConfig::new(
                "upmixer",
                json!({
                    "speaker_config": speaker_config,
                    "gain_front_direct": gain_front_direct,
                    "gain_front_ambient": gain_front_ambient,
                    "gain_rear_ambient": gain_rear_ambient,
                    "height_gain": height_gain,
                    "stereo_width": stereo_width,
                    "center_spread": center_spread,
                    "surround_direct_bleed": surround_direct_bleed,
                    "rear_late_reflection": rear_late_reflection,
                    "lfe_cutoff_hz": lfe_cutoff_hz,
                    "lfe_gain": lfe_gain,
                    "bandpass_hz": bandpass_hz,
                    "enable_subharmonic_synth": enable_subharmonic_synth,
                    "subharmonic_gain": subharmonic_gain,
                    "subharmonic_freq_hz": subharmonic_freq_hz,
                    "subharmonic_attack_ms": subharmonic_attack_ms,
                    "subharmonic_release_ms": subharmonic_release_ms,
                    "decorrelation_mode": decorrelation_mode,
                    "decorrelation_lfo_rate_hz": decorrelation_lfo_rate_hz,
                    "velvet_noise_duration_ms": velvet_noise_duration_ms,
                    "velvet_noise_density": velvet_noise_density,
                    "enable_hr_direct": enable_hr_direct,
                    "hr_sharpen": hr_sharpen,
                    "height_hf_cap_hz": height_hf_cap_hz,
                    "height_transient_reduction": height_transient_reduction,
                    "height_direct_leak": height_direct_leak,
                    "ambient_boost": ambient_boost,
                    "safety_cap_db": safety_cap_db,
                    "low_latency": low_latency,
                    "frequency_resolution": match frequency_resolution {
                        0 => "erb",
                        1 => "fine_erb",
                        2 => "per_bin",
                        _ => "erb",
                    },
                    "rear_ambient_boost": rear_ambient_boost,
                    "dialogue_weight": dialogue_weight,
                    "voice_freq_min_hz": voice_freq_min_hz,
                    "voice_freq_max_hz": voice_freq_max_hz,
                    "dialogue_centroid_weight": dialogue_centroid_weight,
                    "dialogue_variance_weight": dialogue_variance_weight,
                    "dialogue_coherence_weight": dialogue_coherence_weight,
                    "bypass_decorrelation": bypass_decorrelation,
                    "bypass_transient_detection": bypass_transient_detection,
                    "bypass_all_processing": bypass_all_processing,
                    "enable_ml_detection": enable_ml_detection,
                    "multi_source_extraction": multi_source_extraction,
                    "multi_source_threshold": multi_source_threshold,
                    "binaural_preview": binaural_preview,
                }),
            ),
            Self::Compressor {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                knee_db,
                makeup_gain_db,
                mix,
                auto_makeup,
                link_channels,
                sidechain_hpf_hz,
                sidechain_hpf_order,
                detection_mode,
                lookahead_ms,
                program_dependent_release,
                measured_auto_makeup,
                sidechain_external,
            } => PluginConfig::new(
                "compressor",
                json!({
                    "threshold_db": threshold_db,
                    "ratio": ratio,
                    "attack_ms": attack_ms,
                    "release_ms": release_ms,
                    "knee_db": knee_db,
                    "makeup_gain_db": makeup_gain_db,
                    "mix": mix,
                    "auto_makeup": auto_makeup,
                    "link_channels": link_channels,
                    "sidechain_hpf_hz": sidechain_hpf_hz,
                    "sidechain_hpf_order": sidechain_hpf_order,
                    "detection_mode": detection_mode,
                    "lookahead_ms": lookahead_ms,
                    "program_dependent_release": program_dependent_release,
                    "measured_auto_makeup": measured_auto_makeup,
                    "sidechain_external": sidechain_external,
                }),
            ),
            Self::Limiter {
                threshold_db,
                release_ms,
                lookahead_ms,
                soft,
                true_peak,
                isp_mode,
                dual_release,
                mix,
                ..
            } => PluginConfig::new(
                "limiter",
                json!({
                    "threshold_db": threshold_db,
                    "release_ms": release_ms,
                    "lookahead_ms": lookahead_ms,
                    "soft": soft,
                    "true_peak": true_peak,
                    "isp_mode": isp_mode,
                    "dual_release": dual_release,
                    "mix": mix,
                }),
            ),
            Self::Gate {
                threshold_db,
                ratio,
                attack_ms,
                hold_ms,
                release_ms,
                mix,
                link_channels,
                sidechain_hpf_hz,
                sidechain_hpf_order,
                detection_mode,
                sidechain_external,
                range_db,
                hysteresis_db,
                knee_db,
                lookahead_ms,
            } => PluginConfig::new(
                "gate",
                json!({
                    "threshold_db": threshold_db,
                    "ratio": ratio,
                    "attack_ms": attack_ms,
                    "hold_ms": hold_ms,
                    "release_ms": release_ms,
                    "mix": mix,
                    "link_channels": link_channels,
                    "sidechain_hpf_hz": sidechain_hpf_hz,
                    "sidechain_hpf_order": sidechain_hpf_order,
                    "detection_mode": detection_mode,
                    "sidechain_external": sidechain_external,
                    "range_db": range_db,
                    "hysteresis_db": hysteresis_db,
                    "knee_db": knee_db,
                    "lookahead_ms": lookahead_ms,
                }),
            ),
            Self::Expander {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                range_db,
                knee_db,
                hysteresis_db,
                hold_ms,
                mix,
                link_channels,
                sidechain_hpf_hz,
                auto_makeup,
                lookahead_ms,
                detection_mode,
                measured_auto_makeup,
            } => PluginConfig::new(
                "expander",
                json!({
                    "threshold_db": threshold_db,
                    "ratio": ratio,
                    "attack_ms": attack_ms,
                    "release_ms": release_ms,
                    "range_db": range_db,
                    "knee_db": knee_db,
                    "hysteresis_db": hysteresis_db,
                    "hold_ms": hold_ms,
                    "mix": mix,
                    "link_channels": link_channels,
                    "sidechain_hpf_hz": sidechain_hpf_hz,
                    "auto_makeup": auto_makeup,
                    "lookahead_ms": lookahead_ms,
                    "detection_mode": detection_mode,
                    "measured_auto_makeup": measured_auto_makeup,
                }),
            ),
            Self::MultibandCompressor {
                num_bands,
                crossover_preset,
                crossover_freq_1,
                crossover_freq_2,
                crossover_freq_3,
                crossover_freq_4,
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                knee_db,
                mix,
                link_channels,
                per_band_lookahead_ms,
                ms_mode,
                bands,
                ..
            } => PluginConfig::new(
                "multiband_compressor",
                json!({
                    "num_bands": num_bands,
                    "crossover_preset": crossover_preset,
                    "crossover_frequencies": [crossover_freq_1, crossover_freq_2, crossover_freq_3, crossover_freq_4],
                    "crossover_freq_1": crossover_freq_1,
                    "crossover_freq_2": crossover_freq_2,
                    "crossover_freq_3": crossover_freq_3,
                    "crossover_freq_4": crossover_freq_4,
                    "threshold_db": threshold_db,
                    "ratio": ratio,
                    "attack_ms": attack_ms,
                    "release_ms": release_ms,
                    "knee_db": knee_db,
                    "mix": mix,
                    "link_channels": link_channels,
                    "per_band_lookahead_ms": per_band_lookahead_ms,
                    "ms_mode": ms_mode,
                    "bands": bands,
                }),
            ),
            Self::MultibandExpander {
                num_bands,
                crossover_preset,
                crossover_freq_1,
                crossover_freq_2,
                crossover_freq_3,
                crossover_freq_4,
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                range_db,
                knee_db,
                hysteresis_db,
                hold_ms,
                mix,
                link_channels,
                detection_mode,
                bands,
            } => PluginConfig::new(
                "multiband_expander",
                json!({
                    "num_bands": num_bands,
                    "crossover_preset": crossover_preset,
                    "crossover_frequencies": [crossover_freq_1, crossover_freq_2, crossover_freq_3, crossover_freq_4],
                    "crossover_freq_1": crossover_freq_1,
                    "crossover_freq_2": crossover_freq_2,
                    "crossover_freq_3": crossover_freq_3,
                    "crossover_freq_4": crossover_freq_4,
                    "threshold_db": threshold_db,
                    "ratio": ratio,
                    "attack_ms": attack_ms,
                    "release_ms": release_ms,
                    "range_db": range_db,
                    "knee_db": knee_db,
                    "hysteresis_db": hysteresis_db,
                    "hold_ms": hold_ms,
                    "mix": mix,
                    "link_channels": link_channels,
                    "detection_mode": detection_mode,
                    "bands": bands,
                }),
            ),
            Self::LoudnessCompensation {
                low_freq,
                low_gain,
                high_freq,
                high_gain,
                mid_enabled,
                mid_freq,
                mid_gain,
                mid_q,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
                mode,
                playback_level_db,
                reference_level_db,
                playback_volume_db,
            } => PluginConfig::new(
                "loudness_compensation",
                json!({
                    "low_freq": low_freq,
                    "low_gain": low_gain,
                    "high_freq": high_freq,
                    "high_gain": high_gain,
                    "mid_enabled": mid_enabled,
                    "mid_freq": mid_freq,
                    "mid_gain": mid_gain,
                    "mid_q": mid_q,
                    "auto_gain_enabled": auto_gain_enabled,
                    "auto_gain_max_db": auto_gain_max_db,
                    "auto_gain_smoothing_ms": auto_gain_smoothing_ms,
                    "mode": mode,
                    "playback_level_db": playback_level_db,
                    "reference_level_db": reference_level_db,
                    "playback_volume_db": playback_volume_db,
                }),
            ),
            Self::FletcherMunson {
                playback_volume_db,
                reference_level_db,
                ..
            } => {
                // Backward compat: emit as loudness_compensation with mode=2 (Auto)
                PluginConfig::new(
                    "loudness_compensation",
                    json!({
                        "low_freq": pk(lc_specs::PARAMS, "low_freq").default_f64(),
                        "low_gain": pk(lc_specs::PARAMS, "low_gain").default_f64(),
                        "high_freq": pk(lc_specs::PARAMS, "high_freq").default_f64(),
                        "high_gain": pk(lc_specs::PARAMS, "high_gain").default_f64(),
                        "mode": 2,
                        "playback_volume_db": playback_volume_db,
                        "reference_level_db": 83.0 + reference_level_db,
                        "playback_level_db": pk(lc_specs::PARAMS, "playback_level_db").default_f64(),
                    }),
                )
            }
            Self::BinauralDecoder {
                sofa_file,
                input_channels,
                enable_optimization,
                externalization,
                near_field_strength,
                crossfade_mode,
                late_reverb_enabled,
                late_reverb_mix,
                late_reverb_rt60,
                late_reverb_damping,
                headphone_eq_enabled,
            } => PluginConfig::new(
                "binaural_decoder",
                json!({
                    "sofa_file": sofa_file,
                    "input_channels": input_channels,
                    "enable_optimization": enable_optimization,
                    "externalization": externalization,
                    "near_field_strength": near_field_strength,
                    "crossfade_mode": crossfade_mode,
                    "late_reverb_enabled": late_reverb_enabled,
                    "late_reverb_mix": late_reverb_mix,
                    "late_reverb_rt60": late_reverb_rt60,
                    "late_reverb_damping": late_reverb_damping,
                    "headphone_eq_enabled": headphone_eq_enabled,
                }),
            ),
            Self::Convolution {
                ir_file,
                mix,
                gain_db,
                use_nupc,
                zero_latency_head,
                head_taps,
            } => PluginConfig::new(
                "convolution",
                json!({
                    "ir_file": ir_file,
                    "mix": mix,
                    "gain_db": gain_db,
                    "use_nupc": use_nupc,
                    "zero_latency_head": zero_latency_head,
                    "head_taps": head_taps,
                }),
            ),
            Self::LoudnessMonitor => PluginConfig::new("loudness_monitor", json!({})),
            Self::SpectrumAnalyzer {
                num_bins,
                min_freq,
                max_freq,
                smoothing,
                tilt_correction,
                tilt_reference,
            } => PluginConfig::new(
                "spectrum_analyzer",
                json!({
                    "num_bins": num_bins,
                    "min_freq": min_freq,
                    "max_freq": max_freq,
                    "smoothing": smoothing,
                    "tilt_correction": tilt_correction,
                    "tilt_reference": tilt_reference,
                }),
            ),
            Self::ChannelMuteSolo {
                enabled,
                dim_gain_db,
                fade_ms,
                channel_states,
            } => PluginConfig::new(
                "channel_mute_solo",
                json!({
                    "enabled": enabled,
                    "dim_gain_db": dim_gain_db,
                    "fade_ms": fade_ms,
                    "channel_states": channel_states,
                }),
            ),
            Self::Matrix {
                input_channels,
                output_channels,
                matrix,
                channel_states,
            } => {
                let off_diag_count = matrix
                    .iter()
                    .enumerate()
                    .filter(|(idx, v)| {
                        let row = idx / input_channels;
                        let col = idx % input_channels;
                        row != col && v.abs() > 1e-6
                    })
                    .count();
                if off_diag_count > 0 {
                    let off_diag_entries: Vec<_> = matrix
                        .iter()
                        .enumerate()
                        .filter(|(idx, v)| {
                            let row = idx / input_channels;
                            let col = idx % input_channels;
                            row != col && v.abs() > 1e-6
                        })
                        .map(|(idx, v)| {
                            let row = idx / input_channels;
                            let col = idx % input_channels;
                            format!("in{}→out{}={:.3}", col, row, v)
                        })
                        .collect();
                    log::debug!(
                        "[Matrix::to_plugin_config] {}x{} with {} off-diagonal: [{}]",
                        input_channels,
                        output_channels,
                        off_diag_count,
                        off_diag_entries.join(", "),
                    );
                }
                PluginConfig::new(
                    "matrix",
                    json!({
                        "input_channels": input_channels,
                        "output_channels": output_channels,
                        "matrix": matrix,
                        "channel_states": channel_states,
                    }),
                )
            }
            Self::XTC {
                distance_m,
                speaker_angle_deg,
                head_radius_m,
                beta_base,
                beta_low_freq_boost,
                beta_high_freq_boost,
                head_shadow_cutoff_hz,
                head_shadow_slope_db_per_octave,
                max_gain_db,
                head_offset_x,
                head_offset_z,
                head_yaw_deg,
                head_tracking_smooth_s,
                spectral_normalization,
                room_reflections_enabled,
                room_ir_file,
                room_width_m,
                room_depth_m,
                wall_absorption,
                reflection_beta_boost,
                bypass_xtc_filters,
                bypass_spectral_normalization,
                bypass_neumann_refinement,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
                pinna_model_enabled,
                head_model,
            } => PluginConfig::new(
                "xtc",
                json!({
                    "distance_m": distance_m,
                    "speaker_angle_deg": speaker_angle_deg,
                    "head_radius_m": head_radius_m,
                    "beta_base": beta_base,
                    "beta_low_freq_boost": beta_low_freq_boost,
                    "beta_high_freq_boost": beta_high_freq_boost,
                    "head_shadow_cutoff_hz": head_shadow_cutoff_hz,
                    "head_shadow_slope_db_per_octave": head_shadow_slope_db_per_octave,
                    "max_gain_db": max_gain_db,
                    "head_offset_x": head_offset_x,
                    "head_offset_z": head_offset_z,
                    "head_yaw_deg": head_yaw_deg,
                    "head_tracking_smooth_s": head_tracking_smooth_s,
                    "spectral_normalization": spectral_normalization,
                    "room_reflections_enabled": room_reflections_enabled,
                    "room_ir_file": room_ir_file,
                    "room_width_m": room_width_m,
                    "room_depth_m": room_depth_m,
                    "wall_absorption": wall_absorption,
                    "reflection_beta_boost": reflection_beta_boost,
                    "bypass_xtc_filters": bypass_xtc_filters,
                    "bypass_spectral_normalization": bypass_spectral_normalization,
                    "bypass_neumann_refinement": bypass_neumann_refinement,
                    "auto_gain_enabled": auto_gain_enabled,
                    "auto_gain_max_db": auto_gain_max_db,
                    "auto_gain_smoothing_ms": auto_gain_smoothing_ms,
                    "pinna_model_enabled": pinna_model_enabled,
                    "head_model": *head_model as usize,
                }),
            ),
            Self::Denoiser {
                reduction_db,
                floor_db,
                smoothing,
                attack_ms,
                release_ms,
                low_latency,
                polyphonic_detection,
                crack_sensitivity,
                mcra_alpha_s,
                mcra_alpha_p,
                mcra_l,
                mcra_delta,
                transparency,
                dd_enabled,
                dd_alpha,
                psychoacoustic_masking,
                transient_enabled,
                spectral_smoothing_enabled,
                temporal_smoothing_enabled,
                hiss_enabled,
                hiss_threshold_db,
                hiss_frequency_hz,
                hiss_strength,
                spectral_sub_enabled,
                spectral_sub_alpha,
                spectral_sub_beta,
                learn_noise,
                use_captured_profile,
                clear_profile,
                algorithm,
                formant_preservation,
                formant_strength,
                multi_resolution,
                harmonic_percussive,
                spatial_denoise,
                spatial_strength,
            } => PluginConfig::new(
                "denoiser",
                json!({
                    "reduction_db": reduction_db,
                    "floor_db": floor_db,
                    "smoothing": smoothing,
                    "attack_ms": attack_ms,
                    "release_ms": release_ms,
                    "low_latency": low_latency,
                    "polyphonic_detection": polyphonic_detection,
                    "crack_sensitivity": crack_sensitivity,
                    "mcra_alpha_s": mcra_alpha_s,
                    "mcra_alpha_p": mcra_alpha_p,
                    "mcra_l": mcra_l,
                    "mcra_delta": mcra_delta,
                    "transparency": transparency,
                    "dd_enabled": dd_enabled,
                    "dd_alpha": dd_alpha,
                    "psychoacoustic_masking": psychoacoustic_masking,
                    "transient_enabled": transient_enabled,
                    "spectral_smoothing_enabled": spectral_smoothing_enabled,
                    "temporal_smoothing_enabled": temporal_smoothing_enabled,
                    "hiss_enabled": hiss_enabled,
                    "hiss_threshold_db": hiss_threshold_db,
                    "hiss_frequency_hz": hiss_frequency_hz,
                    "hiss_strength": hiss_strength,
                    "spectral_sub_enabled": spectral_sub_enabled,
                    "spectral_sub_alpha": spectral_sub_alpha,
                    "spectral_sub_beta": spectral_sub_beta,
                    "learn_noise": learn_noise,
                    "use_captured_profile": use_captured_profile,
                    "clear_profile": clear_profile,
                    "algorithm": algorithm,
                    "formant_preservation": formant_preservation,
                    "formant_strength": formant_strength,
                    "multi_resolution": multi_resolution,
                    "harmonic_percussive": harmonic_percussive,
                    "spatial_denoise": spatial_denoise,
                    "spatial_strength": spatial_strength,
                }),
            ),
            Self::Pnd {
                correction_strength,
                analysis_window_ms,
                drift_smoothing,
                multi_channel_analysis,
                confidence_threshold,
                phase_vocoder,
            } => PluginConfig::new(
                "pnd",
                json!({
                    "correction_strength": correction_strength,
                    "analysis_window_ms": analysis_window_ms,
                    "drift_smoothing": drift_smoothing,
                    "multi_channel_analysis": multi_channel_analysis,
                    "confidence_threshold": confidence_threshold,
                    "phase_vocoder": phase_vocoder,
                }),
            ),
            Self::ABCompare {
                mix,
                mix_mode,
                selected_path,
                bypass,
                auto_gain_enabled,
                loudness_type,
                max_auto_gain_db,
                gain_smoothing_ms,
                mix_transition_ms,
                path_a_config,
                path_b_config,
                phase_invert_a,
                phase_invert_b,
                difference_mode,
                ..
            } => {
                let loudness_type_str = match loudness_type {
                    0 => "Momentary",
                    _ => "ShortTerm",
                };
                let mix_mode_str = match mix_mode {
                    0 => "Potentiometer",
                    _ => "Binary",
                };
                let path_a_val: serde_json::Value =
                    serde_json::from_str(path_a_config).unwrap_or(json!({"type": "None"}));
                let path_b_val: serde_json::Value =
                    serde_json::from_str(path_b_config).unwrap_or(json!({"type": "None"}));
                PluginConfig::new(
                    "ab_compare",
                    json!({
                        "mix": mix,
                        "mix_mode": mix_mode_str,
                        "selected_path": selected_path,
                        "bypass": bypass,
                        "auto_gain_enabled": auto_gain_enabled,
                        "loudness_type": loudness_type_str,
                        "max_auto_gain_db": max_auto_gain_db,
                        "gain_smoothing_ms": gain_smoothing_ms,
                        "mix_transition_ms": mix_transition_ms,
                        "path_a": path_a_val,
                        "path_b": path_b_val,
                        "phase_invert_a": phase_invert_a,
                        "phase_invert_b": phase_invert_b,
                        "difference_mode": difference_mode,
                    }),
                )
            }
            Self::BandSplit {
                channels,
                frequency,
                crossover_type,
            } => PluginConfig::new(
                "band_split",
                json!({
                    "channels": channels,
                    "frequency": frequency,
                    "type": crossover_type,
                }),
            ),
            Self::BandMerge { channels, bands } => PluginConfig::new(
                "band_merge",
                json!({
                    "channels": channels,
                    "bands": bands,
                }),
            ),
            Self::Downmix {
                input_channels,
                center_gain_db,
                surround_gain_db,
                height_gain_db,
                lfe_gain_db,
                phase_coherence,
                phase_blend_low_hz,
                phase_blend_high_hz,
                itu_mode,
            } => PluginConfig::new(
                "downmix",
                json!({
                    "input_channels": input_channels,
                    "center_gain_db": center_gain_db,
                    "surround_gain_db": surround_gain_db,
                    "height_gain_db": height_gain_db,
                    "lfe_gain_db": lfe_gain_db,
                    "phase_coherence": phase_coherence,
                    "phase_blend_low_hz": phase_blend_low_hz,
                    "phase_blend_high_hz": phase_blend_high_hz,
                    "itu_mode": itu_mode,
                }),
            ),
            Self::MonoToStereo {
                stereo_width,
                haas_delay_ms,
                enable_comp_eq,
                comp_eq_depth_db,
                decor_low_hz,
                decor_high_hz,
                freq_dependent,
            } => PluginConfig::new(
                "mono_to_stereo",
                json!({
                    "stereo_width": stereo_width,
                    "haas_delay_ms": haas_delay_ms,
                    "enable_comp_eq": enable_comp_eq,
                    "comp_eq_depth_db": comp_eq_depth_db,
                    "decor_low_hz": decor_low_hz,
                    "decor_high_hz": decor_high_hz,
                    "freq_dependent": freq_dependent,
                }),
            ),
            Self::AAE {
                speaker_config,
                room_size,
                rt60,
                bass_ratio,
                treble_ratio,
                pre_delay_ms,
                room_preset,
                dry_level,
                er_level,
                late_level,
                lfe_level,
                mod_depth,
                er_mod_depth,
                input_diffusion,
                envelopment,
                height_amount,
                content_aware,
                dialogue_attenuation_db,
                safety_limit_db,
                bypass,
                solo_early,
                solo_late,
            } => PluginConfig::new(
                "aae",
                json!({
                    "speaker_config": speaker_config,
                    "room_size": room_size,
                    "rt60": rt60,
                    "bass_ratio": bass_ratio,
                    "treble_ratio": treble_ratio,
                    "pre_delay_ms": pre_delay_ms,
                    "room_preset": room_preset,
                    "dry_level": dry_level,
                    "er_level": er_level,
                    "late_level": late_level,
                    "lfe_level": lfe_level,
                    "mod_depth": mod_depth,
                    "er_mod_depth": er_mod_depth,
                    "input_diffusion": input_diffusion,
                    "envelopment": envelopment,
                    "height_amount": height_amount,
                    "content_aware": content_aware,
                    "dialogue_attenuation_db": dialogue_attenuation_db,
                    "safety_limit_db": safety_limit_db,
                    "bypass": bypass,
                    "solo_early": solo_early,
                    "solo_late": solo_late,
                }),
            ),
        }
    }

    /// Create default settings for a plugin type
    pub fn default_for(plugin_type: &PluginType) -> Self {
        use sotf_plugins::param_specs::find_by_key as p;

        match plugin_type {
            PluginType::EQ => Self::EQ {
                channels: default_channels(),
                filters: vec![
                    // Default: 5-band flat EQ
                    EQFilter::new(BiquadFilterType::Peak, 100.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 300.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 3000.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 10000.0, 1.4, 0.0),
                ],
                channel_filters: None,
                per_channel_mode: false,
                max_filters: 5,
                tdf2: false,
                topology: 0.0,
            },
            PluginType::Gain => Self::Gain {
                channels: default_channels(),
                gain_db: p(gain_specs::PARAMS, "gain_db").default_f64(),
                smoothing_ms: p(gain_specs::PARAMS, "smoothing_ms").default_f64(),
            },
            PluginType::Upmixer => {
                let u = upmixer_specs::PARAMS;
                Self::Upmixer {
                    speaker_config: "5.1".to_string(),
                    gain_front_direct: p(u, "gain_front_direct").default_f64(),
                    gain_front_ambient: p(u, "gain_front_ambient").default_f64(),
                    gain_rear_ambient: p(u, "gain_rear_ambient").default_f64(),
                    height_gain: p(u, "height_gain").default_f64(),
                    stereo_width: p(u, "stereo_width").default_f64(),
                    center_spread: p(u, "center_spread").default_f64(),
                    surround_direct_bleed: p(u, "surround_direct_bleed").default_f64(),
                    rear_late_reflection: p(u, "rear_late_reflection").default_f64(),
                    lfe_cutoff_hz: p(u, "lfe_cutoff_hz").default_f64(),
                    lfe_gain: p(u, "lfe_gain").default_f64(),
                    bandpass_hz: p(u, "bandpass_hz").default_f64(),
                    enable_subharmonic_synth: p(u, "enable_subharmonic_synth").default_bool(),
                    subharmonic_gain: p(u, "subharmonic_gain").default_f64(),
                    subharmonic_freq_hz: p(u, "subharmonic_freq_hz").default_f64(),
                    subharmonic_attack_ms: p(u, "subharmonic_attack_ms").default_f64(),
                    subharmonic_release_ms: p(u, "subharmonic_release_ms").default_f64(),
                    decorrelation_mode: p(u, "decorrelation_mode").default_usize(),
                    decorrelation_lfo_rate_hz: p(u, "decorrelation_lfo_rate_hz").default_f64(),
                    velvet_noise_duration_ms: p(u, "velvet_noise_duration_ms").default_f64(),
                    velvet_noise_density: p(u, "velvet_noise_density").default_f64(),
                    enable_hr_direct: p(u, "enable_hr_direct").default_bool(),
                    hr_sharpen: p(u, "hr_sharpen").default_f64(),
                    height_hf_cap_hz: p(u, "height_hf_cap_hz").default_f64(),
                    height_transient_reduction: p(u, "height_transient_reduction").default_f64(),
                    height_direct_leak: p(u, "height_direct_leak").default_f64(),
                    ambient_boost: p(u, "ambient_boost").default_f64(),
                    safety_cap_db: p(u, "safety_cap_db").default_f64(),
                    low_latency: p(u, "low_latency").default_bool(),
                    frequency_resolution: p(u, "frequency_resolution").default_usize(),
                    rear_ambient_boost: p(u, "rear_ambient_boost").default_f64(),
                    dialogue_weight: p(u, "dialogue_weight").default_f64(),
                    voice_freq_min_hz: p(u, "voice_freq_min_hz").default_f64(),
                    voice_freq_max_hz: p(u, "voice_freq_max_hz").default_f64(),
                    dialogue_centroid_weight: p(u, "dialogue_centroid_weight").default_f64(),
                    dialogue_variance_weight: p(u, "dialogue_variance_weight").default_f64(),
                    dialogue_coherence_weight: p(u, "dialogue_coherence_weight").default_f64(),
                    bypass_decorrelation: false,
                    bypass_transient_detection: false,
                    bypass_all_processing: false,
                    enable_ml_detection: p(u, "enable_ml_detection").default_bool(),
                    multi_source_extraction: p(u, "multi_source_extraction").default_bool(),
                    multi_source_threshold: p(u, "multi_source_threshold").default_f64(),
                    binaural_preview: p(u, "binaural_preview").default_bool(),
                }
            }
            PluginType::Compressor => {
                let c = compressor_specs::PARAMS;
                Self::Compressor {
                    threshold_db: p(c, "threshold").default_f64(),
                    ratio: p(c, "ratio").default_f64(),
                    attack_ms: p(c, "attack").default_f64(),
                    release_ms: p(c, "release").default_f64(),
                    knee_db: p(c, "knee").default_f64(),
                    makeup_gain_db: p(c, "makeup_gain").default_f64(),
                    mix: p(c, "mix").default_f64(),
                    auto_makeup: p(c, "auto_makeup").default_bool(),
                    link_channels: p(c, "link_channels").default_bool(),
                    sidechain_hpf_hz: p(c, "sidechain_hpf_hz").default_f64(),
                    sidechain_hpf_order: default_compressor_sidechain_hpf_order(),
                    detection_mode: default_compressor_detection_mode(),
                    lookahead_ms: p(c, "lookahead_ms").default_f64(),
                    program_dependent_release: p(c, "program_dependent_release").default_bool(),
                    measured_auto_makeup: p(c, "measured_auto_makeup").default_bool(),
                    sidechain_external: p(c, "sidechain_external").default_bool(),
                }
            }
            PluginType::Limiter => {
                let l = limiter_specs::PARAMS;
                Self::Limiter {
                    threshold_db: p(l, "threshold").default_f64(),
                    release_ms: p(l, "release").default_f64(),
                    lookahead_ms: p(l, "lookahead").default_f64(),
                    soft: p(l, "soft").default_bool(),
                    true_peak: p(l, "true_peak").default_bool(),
                    isp_mode: p(l, "isp_mode").default_bool(),
                    dual_release: p(l, "dual_release").default_bool(),
                    mix: p(l, "mix").default_f64(),
                    link_amount: p(l, "link_amount").default_f64(),
                    feed_forward: p(l, "feed_forward").default_bool(),
                }
            }
            PluginType::Gate => {
                let g = gate_specs::PARAMS;
                Self::Gate {
                    threshold_db: p(g, "threshold").default_f64(),
                    ratio: p(g, "ratio").default_f64(),
                    attack_ms: p(g, "attack").default_f64(),
                    hold_ms: p(g, "hold").default_f64(),
                    release_ms: p(g, "release").default_f64(),
                    mix: p(g, "mix").default_f64(),
                    link_channels: p(g, "link_channels").default_bool(),
                    sidechain_hpf_hz: p(g, "sidechain_hpf_hz").default_f64(),
                    sidechain_hpf_order: default_gate_sidechain_hpf_order(),
                    detection_mode: default_gate_detection_mode(),
                    sidechain_external: p(g, "sidechain_external").default_bool(),
                    range_db: p(g, "range_db").default_f64(),
                    hysteresis_db: p(g, "hysteresis_db").default_f64(),
                    knee_db: p(g, "knee_db").default_f64(),
                    lookahead_ms: p(g, "lookahead_ms").default_f64(),
                }
            }
            PluginType::Expander => {
                let e = expander_specs::PARAMS;
                Self::Expander {
                    threshold_db: p(e, "threshold").default_f64(),
                    ratio: p(e, "ratio").default_f64(),
                    attack_ms: p(e, "attack").default_f64(),
                    release_ms: p(e, "release").default_f64(),
                    range_db: p(e, "range").default_f64(),
                    knee_db: p(e, "knee").default_f64(),
                    hysteresis_db: p(e, "hysteresis").default_f64(),
                    hold_ms: p(e, "hold").default_f64(),
                    mix: p(e, "mix").default_f64(),
                    auto_makeup: p(e, "auto_makeup").default_bool(),
                    link_channels: p(e, "link_channels").default_bool(),
                    sidechain_hpf_hz: p(e, "sidechain_hpf_hz").default_f64(),
                    lookahead_ms: p(e, "lookahead_ms").default_f64(),
                    detection_mode: default_expander_detection_mode(),
                    measured_auto_makeup: p(e, "measured_auto_makeup").default_bool(),
                }
            }
            PluginType::MultibandCompressor => {
                let mc = mb_compressor_specs::GLOBAL_PARAMS;
                Self::MultibandCompressor {
                    num_bands: p(mc, "num_bands").default_usize(),
                    crossover_preset: p(mc, "crossover_preset").default_i32(),
                    crossover_freq_1: p(mc, "crossover_freq_1").default_f64(),
                    crossover_freq_2: p(mc, "crossover_freq_2").default_f64(),
                    crossover_freq_3: p(mc, "crossover_freq_3").default_f64(),
                    crossover_freq_4: p(mc, "crossover_freq_4").default_f64(),
                    threshold_db: p(mc, "threshold").default_f64(),
                    ratio: p(mc, "ratio").default_f64(),
                    attack_ms: p(mc, "attack").default_f64(),
                    release_ms: p(mc, "release").default_f64(),
                    knee_db: p(mc, "knee").default_f64(),
                    mix: p(mc, "mix").default_f64(),
                    link_channels: p(mc, "link_channels").default_bool(),
                    per_band_lookahead_ms: p(mc, "per_band_lookahead_ms").default_f64(),
                    ms_mode: p(mc, "ms_mode").default_bool(),
                    bands: Vec::new(),
                    sidechain_tilt_db: 0.0,
                    link_amount: p(mc, "link_amount").default_f64(),
                }
            }
            PluginType::MultibandExpander => {
                let me = mb_expander_specs::GLOBAL_PARAMS;
                Self::MultibandExpander {
                    num_bands: p(me, "num_bands").default_usize(),
                    crossover_preset: p(me, "crossover_preset").default_i32(),
                    crossover_freq_1: p(me, "crossover_freq_1").default_f64(),
                    crossover_freq_2: p(me, "crossover_freq_2").default_f64(),
                    crossover_freq_3: p(me, "crossover_freq_3").default_f64(),
                    crossover_freq_4: p(me, "crossover_freq_4").default_f64(),
                    threshold_db: p(me, "threshold").default_f64(),
                    ratio: p(me, "ratio").default_f64(),
                    attack_ms: p(me, "attack").default_f64(),
                    release_ms: p(me, "release").default_f64(),
                    range_db: p(me, "range").default_f64(),
                    knee_db: p(me, "knee").default_f64(),
                    hysteresis_db: p(me, "hysteresis").default_f64(),
                    hold_ms: p(me, "hold").default_f64(),
                    mix: p(me, "mix").default_f64(),
                    link_channels: p(me, "link_channels").default_bool(),
                    detection_mode: default_mb_expander_detection_mode(),
                    bands: Vec::new(),
                }
            }
            PluginType::LoudnessCompensation => {
                let lc = lc_specs::PARAMS;
                Self::LoudnessCompensation {
                    low_freq: p(lc, "low_freq").default_f64(),
                    low_gain: p(lc, "low_gain").default_f64(),
                    high_freq: p(lc, "high_freq").default_f64(),
                    high_gain: p(lc, "high_gain").default_f64(),
                    mid_enabled: p(lc, "mid_enabled").default_bool(),
                    mid_freq: p(lc, "mid_freq").default_f64(),
                    mid_gain: p(lc, "mid_gain").default_f64(),
                    mid_q: p(lc, "mid_q").default_f64(),
                    auto_gain_enabled: p(lc, "auto_gain_enabled").default_bool(),
                    auto_gain_max_db: p(lc, "auto_gain_max_db").default_f64(),
                    auto_gain_smoothing_ms: p(lc, "auto_gain_smoothing_ms").default_f64(),
                    mode: p(lc, "mode").default_usize(),
                    playback_level_db: p(lc, "playback_level_db").default_f64(),
                    reference_level_db: p(lc, "reference_level_db").default_f64(),
                    playback_volume_db: 0.0,
                }
            }
            PluginType::FletcherMunson => {
                // Fletcher-Munson merged into LoudnessCompensation with mode=2 (Auto)
                let lc = lc_specs::PARAMS;
                Self::LoudnessCompensation {
                    low_freq: p(lc, "low_freq").default_f64(),
                    low_gain: p(lc, "low_gain").default_f64(),
                    high_freq: p(lc, "high_freq").default_f64(),
                    high_gain: p(lc, "high_gain").default_f64(),
                    mid_enabled: p(lc, "mid_enabled").default_bool(),
                    mid_freq: p(lc, "mid_freq").default_f64(),
                    mid_gain: p(lc, "mid_gain").default_f64(),
                    mid_q: p(lc, "mid_q").default_f64(),
                    auto_gain_enabled: p(lc, "auto_gain_enabled").default_bool(),
                    auto_gain_max_db: p(lc, "auto_gain_max_db").default_f64(),
                    auto_gain_smoothing_ms: p(lc, "auto_gain_smoothing_ms").default_f64(),
                    mode: 2, // Auto mode
                    playback_level_db: p(lc, "playback_level_db").default_f64(),
                    reference_level_db: p(lc, "reference_level_db").default_f64(),
                    playback_volume_db: 0.0,
                }
            }
            PluginType::BinauralDecoder => {
                let b = binaural_specs::PARAMS;
                Self::BinauralDecoder {
                    sofa_file: String::new(),
                    input_channels: 6, // Default to 5.1
                    enable_optimization: p(b, "enable_optimization").default_bool(),
                    externalization: p(b, "externalization").default_f64(),
                    near_field_strength: p(b, "near_field_strength").default_f64(),
                    crossfade_mode: p(b, "crossfade_mode").default_usize(),
                    late_reverb_enabled: false,
                    late_reverb_mix: p(b, "late_reverb_mix").default_f64(),
                    late_reverb_rt60: p(b, "late_reverb_rt60").default_f64(),
                    late_reverb_damping: p(b, "late_reverb_damping").default_f64(),
                    headphone_eq_enabled: false,
                }
            }
            PluginType::Convolution => {
                let cv = convolution_specs::PARAMS;
                Self::Convolution {
                    ir_file: String::new(),
                    mix: p(cv, "mix").default_f64(),
                    gain_db: p(cv, "gain_db").default_f64(),
                    use_nupc: p(cv, "use_nupc").default_bool(),
                    zero_latency_head: false,
                    head_taps: p(cv, "head_taps").default_usize(),
                }
            }
            PluginType::LoudnessMonitor => Self::LoudnessMonitor,
            PluginType::SpectrumAnalyzer => Self::SpectrumAnalyzer {
                num_bins: pk(spectrum_specs::PARAMS, "num_bins").default_usize(),
                min_freq: pk(spectrum_specs::PARAMS, "min_freq").default_f64() as f32,
                max_freq: pk(spectrum_specs::PARAMS, "max_freq").default_f64() as f32,
                smoothing: pk(spectrum_specs::PARAMS, "smoothing").default_f64() as f32,
                tilt_correction: SpectralTiltCorrection::None,
                tilt_reference: TiltReferenceFreq::Standard,
            },
            PluginType::ChannelMuteSolo => Self::ChannelMuteSolo {
                enabled: pk(cms_specs::PARAMS, "enabled").default_bool(),
                dim_gain_db: pk(cms_specs::PARAMS, "dim_gain_db").default_f64(),
                fade_ms: pk(cms_specs::PARAMS, "fade_ms").default_f64(),
                channel_states: vec![],
            },
            PluginType::Matrix => Self::Matrix {
                input_channels: 2,
                output_channels: 2,
                matrix: vec![
                    pk(matrix_specs::PARAMS, "gain").max_f64() as f32,
                    pk(matrix_specs::PARAMS, "gain").min_f64() as f32,
                    pk(matrix_specs::PARAMS, "gain").min_f64() as f32,
                    pk(matrix_specs::PARAMS, "gain").max_f64() as f32,
                ], // Identity 2x2
                channel_states: vec![],
            },
            PluginType::XTC => {
                let x = xtc_specs::PARAMS;
                Self::XTC {
                    distance_m: p(x, "distance_m").default_f64(),
                    speaker_angle_deg: p(x, "speaker_angle_deg").default_f64(),
                    head_radius_m: p(x, "head_radius_m").default_f64(),
                    beta_base: p(x, "beta_base").default_f64(),
                    beta_low_freq_boost: p(x, "beta_low_freq_boost").default_f64(),
                    beta_high_freq_boost: p(x, "beta_high_freq_boost").default_f64(),
                    head_shadow_cutoff_hz: p(x, "head_shadow_cutoff_hz").default_f64(),
                    head_shadow_slope_db_per_octave: p(x, "head_shadow_slope_db_per_octave")
                        .default_f64(),
                    max_gain_db: p(x, "max_gain_db").default_f64(),
                    head_offset_x: p(x, "head_offset_x").default_f64(),
                    head_offset_z: p(x, "head_offset_z").default_f64(),
                    head_yaw_deg: p(x, "head_yaw_deg").default_f64(),
                    head_tracking_smooth_s: pk(xtc_specs::PARAMS, "head_tracking_smooth_s")
                        .default_f64(),
                    spectral_normalization: p(x, "spectral_normalization").default_bool(),
                    room_reflections_enabled: p(x, "room_reflections_enabled").default_bool(),
                    room_ir_file: None,
                    room_width_m: p(x, "room_width_m").default_f64(),
                    room_depth_m: p(x, "room_depth_m").default_f64(),
                    wall_absorption: p(x, "wall_absorption").default_f64(),
                    reflection_beta_boost: p(x, "reflection_beta_boost").default_f64(),
                    bypass_xtc_filters: p(x, "bypass_xtc_filters").default_bool(),
                    bypass_spectral_normalization: p(x, "bypass_spectral_normalization")
                        .default_bool(),
                    bypass_neumann_refinement: p(x, "bypass_neumann_refinement").default_bool(),
                    auto_gain_enabled: p(x, "auto_gain_enabled").default_bool(),
                    auto_gain_max_db: p(x, "auto_gain_max_db").default_f64(),
                    auto_gain_smoothing_ms: p(x, "auto_gain_smoothing_ms").default_f64(),
                    pinna_model_enabled: p(x, "pinna_model_enabled").default_bool(),
                    head_model: p(x, "head_model").default_f64(),
                }
            }
            PluginType::Denoiser => {
                let d = denoiser_specs::PARAMS;
                Self::Denoiser {
                    reduction_db: p(d, "reduction_db").default_f64(),
                    floor_db: p(d, "floor_db").default_f64(),
                    smoothing: p(d, "smoothing").default_f64(),
                    attack_ms: p(d, "attack_ms").default_f64(),
                    release_ms: p(d, "release_ms").default_f64(),
                    low_latency: p(d, "low_latency").default_bool(),
                    polyphonic_detection: p(d, "polyphonic_detection").default_bool(),
                    crack_sensitivity: p(d, "crack_sensitivity").default_f64(),
                    mcra_alpha_s: p(d, "mcra_alpha_s").default_f64(),
                    mcra_alpha_p: p(d, "mcra_alpha_p").default_f64(),
                    mcra_l: p(d, "mcra_l").default_usize(),
                    mcra_delta: p(d, "mcra_delta").default_f64(),
                    transparency: p(d, "transparency").default_f64(),
                    dd_enabled: p(d, "dd_enabled").default_bool(),
                    dd_alpha: p(d, "dd_alpha").default_f64(),
                    psychoacoustic_masking: p(d, "psychoacoustic_masking").default_bool(),
                    transient_enabled: p(d, "transient_enabled").default_bool(),
                    spectral_smoothing_enabled: p(d, "spectral_smoothing_enabled").default_bool(),
                    temporal_smoothing_enabled: p(d, "temporal_smoothing_enabled").default_bool(),
                    hiss_enabled: p(d, "hiss_enabled").default_bool(),
                    hiss_threshold_db: p(d, "hiss_threshold_db").default_f64(),
                    hiss_frequency_hz: p(d, "hiss_frequency_hz").default_f64(),
                    hiss_strength: p(d, "hiss_strength").default_f64(),
                    spectral_sub_enabled: p(d, "spectral_sub_enabled").default_bool(),
                    spectral_sub_alpha: p(d, "spectral_sub_alpha").default_f64(),
                    spectral_sub_beta: p(d, "spectral_sub_beta").default_f64(),
                    learn_noise: p(d, "learn_noise").default_bool(),
                    use_captured_profile: p(d, "use_captured_profile").default_bool(),
                    clear_profile: p(d, "clear_profile").default_bool(),
                    algorithm: p(d, "algorithm").default_usize(),
                    formant_preservation: p(d, "formant_preservation").default_bool(),
                    formant_strength: p(d, "formant_strength").default_f64(),
                    multi_resolution: p(d, "multi_resolution").default_bool(),
                    harmonic_percussive: false,
                    spatial_denoise: false,
                    spatial_strength: p(d, "spatial_strength").default_f64(),
                }
            }
            PluginType::Pnd => {
                let pn = pnd_specs::PARAMS;
                Self::Pnd {
                    correction_strength: p(pn, "correction_strength").default_f64(),
                    analysis_window_ms: p(pn, "analysis_window_ms").default_f64(),
                    drift_smoothing: p(pn, "drift_smoothing").default_f64(),
                    multi_channel_analysis: p(pn, "multi_channel_analysis").default_bool(),
                    confidence_threshold: p(pn, "confidence_threshold").default_f64(),
                    phase_vocoder: p(pn, "phase_vocoder").default_bool(),
                }
            }
            PluginType::ABCompare => {
                let ab = ab_compare_specs::PARAMS;
                Self::ABCompare {
                    mix: p(ab, "mix").default_f64(),
                    mix_mode: p(ab, "mix_mode").default_i32(),
                    selected_path: p(ab, "selected_path").default_i32(),
                    bypass: p(ab, "bypass").default_bool(),
                    auto_gain_enabled: p(ab, "auto_gain_enabled").default_bool(),
                    loudness_type: p(ab, "loudness_type").default_i32(),
                    max_auto_gain_db: p(ab, "max_auto_gain_db").default_f64(),
                    gain_smoothing_ms: p(ab, "gain_smoothing_ms").default_f64(),
                    mix_transition_ms: p(ab, "mix_transition_ms").default_f64(),
                    path_a_config: default_ab_path_config(),
                    path_b_config: default_ab_path_config(),
                    path_a_file: String::new(),
                    path_b_file: String::new(),
                    phase_invert_a: p(ab, "phase_invert_a").default_bool(),
                    phase_invert_b: p(ab, "phase_invert_b").default_bool(),
                    difference_mode: p(ab, "difference_mode").default_bool(),
                }
            }
            PluginType::BandSplit => Self::BandSplit {
                channels: default_channels(),
                frequency: default_band_split_frequency(),
                crossover_type: default_band_split_crossover_type(),
            },
            PluginType::BandMerge => Self::BandMerge {
                channels: default_channels(),
                bands: default_band_merge_bands(),
            },
            PluginType::Downmix => {
                let dw = downmix_specs::PARAMS;
                Self::Downmix {
                    input_channels: 6, // Default to 5.1
                    center_gain_db: p(dw, "center_gain_db").default_f64(),
                    surround_gain_db: p(dw, "surround_gain_db").default_f64(),
                    height_gain_db: p(dw, "height_gain_db").default_f64(),
                    lfe_gain_db: p(dw, "lfe_gain_db").default_f64(),
                    phase_coherence: p(dw, "phase_coherence").default_bool(),
                    phase_blend_low_hz: p(dw, "phase_blend_low_hz").default_f64(),
                    phase_blend_high_hz: p(dw, "phase_blend_high_hz").default_f64(),
                    itu_mode: p(dw, "itu_mode").default_bool(),
                }
            }
            PluginType::MonoToStereo => {
                let ms = mono_to_stereo_specs::PARAMS;
                Self::MonoToStereo {
                    stereo_width: p(ms, "stereo_width").default_f64(),
                    haas_delay_ms: p(ms, "haas_delay_ms").default_f64(),
                    enable_comp_eq: p(ms, "enable_comp_eq").default_bool(),
                    comp_eq_depth_db: p(ms, "comp_eq_depth_db").default_f64(),
                    decor_low_hz: p(ms, "decor_low_hz").default_f64(),
                    decor_high_hz: p(ms, "decor_high_hz").default_f64(),
                    freq_dependent: p(ms, "freq_dependent").default_bool(),
                }
            }
            PluginType::Crossfeed => {
                let cf = crossfeed_specs::PARAMS;
                Self::Crossfeed {
                    mode: CrossfeedMode::Bauer,
                    preset: CrossfeedPreset::Default,
                    enabled: true,
                    mix: p(cf, "mix").default_f64(),
                    bauer_fcut_hz: p(cf, "bauer_fcut_hz").default_f64(),
                    bauer_feed_db: p(cf, "bauer_feed_db").default_f64(),
                    meier_level: p(cf, "meier_level").default_f64(),
                    mb_low_freq_hz: p(cf, "mb_low_freq_hz").default_f64(),
                    mb_mid_high_freq_hz: p(cf, "mb_mid_high_freq_hz").default_f64(),
                    mb_low_feed_db: p(cf, "mb_low_feed_db").default_f64(),
                    mb_mid_feed_db: p(cf, "mb_mid_feed_db").default_f64(),
                    mb_high_feed_db: p(cf, "mb_high_feed_db").default_f64(),
                    itd_delay_ms: p(cf, "itd_delay_ms").default_f64(),
                    autogain_enabled: p(cf, "autogain_enabled").default_bool(),
                    autogain_target_lufs: p(cf, "autogain_target_lufs").default_f64(),
                    autogain_max_gain_db: p(cf, "autogain_max_gain_db").default_f64(),
                    autogain_smoothing_ms: p(cf, "autogain_smoothing_ms").default_f64(),
                }
            }
            PluginType::Delay => {
                let d = delay_specs::PARAMS;
                Self::Delay {
                    delay_ms: p(d, "delay_ms").default_f64(),
                    feedback: p(d, "feedback").default_f64(),
                    mix: p(d, "mix").default_f64(),
                    lfo_rate_hz: p(d, "lfo_rate_hz").default_f64(),
                    lfo_depth_ms: p(d, "lfo_depth_ms").default_f64(),
                    allpass_feedback: p(d, "allpass_feedback").default_bool(),
                }
            }
            PluginType::Aec => {
                let a = aec_specs::PARAMS;
                Self::Aec {
                    echo_tail_ms: p(a, "echo_tail_ms").default_f64(),
                    step_size: p(a, "step_size").default_f64(),
                    post_filter_enabled: p(a, "post_filter_enabled").default_bool(),
                }
            }
            PluginType::Beamformer => {
                let b = beamformer_specs::PARAMS;
                Self::Beamformer {
                    num_mics: p(b, "num_mics").default_usize(),
                    mic_spacing_cm: p(b, "mic_spacing_cm").default_f64(),
                    steer_angle_deg: p(b, "steer_angle_deg").default_f64(),
                    beamformer_type: p(b, "beamformer_type").default_usize(),
                }
            }
            PluginType::AmbisonicsDecoder => {
                let a = ambisonics_specs::PARAMS;
                Self::AmbisonicsDecoder {
                    order: p(a, "order").default_usize(),
                    target_layout: default_ambisonics_target_layout(),
                    max_re_weighting: p(a, "max_re_weighting").default_bool(),
                    dual_band: p(a, "dual_band").default_bool(),
                }
            }
            PluginType::StereoImager => {
                let si = stereo_imager_specs::PARAMS;
                Self::StereoImager {
                    width: p(si, "width").default_f64(),
                    low_mid_freq: p(si, "low_mid_freq").default_f64(),
                    mid_high_freq: p(si, "mid_high_freq").default_f64(),
                    low_width: p(si, "low_width").default_f64(),
                    mid_width: p(si, "mid_width").default_f64(),
                    high_width: p(si, "high_width").default_f64(),
                    mono_bass: p(si, "mono_bass").default_bool(),
                    mix: p(si, "mix").default_f64(),
                }
            }
            PluginType::DeEsser => {
                let de = de_esser_specs::PARAMS;
                Self::DeEsser {
                    frequency: p(de, "frequency").default_f64(),
                    q: p(de, "q").default_f64(),
                    threshold: p(de, "threshold").default_f64(),
                    ratio: p(de, "ratio").default_f64(),
                    attack: p(de, "attack").default_f64(),
                    release: p(de, "release").default_f64(),
                    mode: default_de_esser_mode(),
                    mix: p(de, "mix").default_f64(),
                }
            }
            PluginType::TransientShaper => {
                let ts = transient_shaper_specs::PARAMS;
                Self::TransientShaper {
                    attack: p(ts, "attack").default_f64(),
                    sustain: p(ts, "sustain").default_f64(),
                    sensitivity_db: p(ts, "sensitivity").default_f64(),
                    output_gain_db: p(ts, "output_gain").default_f64(),
                    mix: p(ts, "mix").default_f64(),
                }
            }
            PluginType::Saturation => {
                let sat = saturation_specs::PARAMS;
                Self::Saturation {
                    mode: p(sat, "mode").default_f64(),
                    drive: p(sat, "drive").default_f64(),
                    tone: p(sat, "tone").default_f64(),
                    exciter_freq: p(sat, "exciter_freq").default_f64(),
                    oversampling: p(sat, "oversampling").default_f64(),
                    output_gain_db: p(sat, "output_gain").default_f64(),
                    mix: p(sat, "mix").default_f64(),
                    dynamic_amount: p(sat, "dynamic_amount").default_f64(),
                    dynamic_attack_ms: p(sat, "dynamic_attack_ms").default_f64(),
                    dynamic_release_ms: p(sat, "dynamic_release_ms").default_f64(),
                    dc_blocker: p(sat, "dc_blocker").default_bool(),
                    use_adaa: p(sat, "use_adaa").default_bool(),
                }
            }
            PluginType::DynamicEq => {
                let dq = dynamic_eq_specs::PARAMS;
                Self::DynamicEq {
                    num_bands: p(dq, "num_bands").default_f64(),
                    threshold: p(dq, "threshold").default_f64(),
                    ratio: p(dq, "ratio").default_f64(),
                    attack: p(dq, "attack").default_f64(),
                    release: p(dq, "release").default_f64(),
                    knee: p(dq, "knee").default_f64(),
                    link_channels: p(dq, "link_channels").default_bool(),
                    mix: p(dq, "mix").default_f64(),
                }
            }
            PluginType::LinearPhaseEq => {
                let lp = linear_phase_eq_specs::PARAMS;
                Self::LinearPhaseEq {
                    num_filters: p(lp, "num_filters").default_f64(),
                    fir_length: p(lp, "fir_length").default_f64(),
                    auto_gain: p(lp, "auto_gain").default_bool(),
                    mix: p(lp, "mix").default_f64(),
                }
            }
            PluginType::SpectralCompressor => {
                let sc = spectral_compressor_specs::PARAMS;
                Self::SpectralCompressor {
                    fft_size: p(sc, "fft_size").default_f64() as usize,
                    threshold: p(sc, "threshold").default_f64(),
                    ratio: p(sc, "ratio").default_f64(),
                    attack: p(sc, "attack").default_f64(),
                    release: p(sc, "release").default_f64(),
                    knee: p(sc, "knee").default_f64(),
                    spectral_smoothing: p(sc, "spectral_smoothing").default_f64(),
                    mix: p(sc, "mix").default_f64(),
                    target_mode: p(sc, "target_mode").default_f64(),
                    delta_listen: false,
                    adaptive_threshold: false,
                    adaptive_offset_db: 0.0,
                }
            }
            PluginType::AAE => {
                let a = aae_specs::PARAMS;
                Self::AAE {
                    speaker_config: p(a, "speaker_config").default_choice_label(),
                    room_size: p(a, "room_size").default_f64(),
                    rt60: p(a, "rt60").default_f64(),
                    bass_ratio: p(a, "bass_ratio").default_f64(),
                    treble_ratio: p(a, "treble_ratio").default_f64(),
                    pre_delay_ms: p(a, "pre_delay_ms").default_f64(),
                    room_preset: p(a, "room_preset").default_choice_label(),
                    dry_level: p(a, "dry_level").default_f64(),
                    er_level: p(a, "er_level").default_f64(),
                    late_level: p(a, "late_level").default_f64(),
                    lfe_level: p(a, "lfe_level").default_f64(),
                    mod_depth: p(a, "mod_depth").default_f64(),
                    er_mod_depth: p(a, "er_mod_depth").default_f64(),
                    input_diffusion: p(a, "input_diffusion").default_f64(),
                    envelopment: p(a, "envelopment").default_f64(),
                    height_amount: p(a, "height_amount").default_f64(),
                    content_aware: p(a, "content_aware").default_bool(),
                    dialogue_attenuation_db: p(a, "dialogue_attenuation_db").default_f64(),
                    safety_limit_db: p(a, "safety_limit_db").default_f64(),
                    bypass: false,
                    solo_early: false,
                    solo_late: false,
                }
            }
        }
    }
}

// ============================================================================
// Matrix Helper Functions
// ============================================================================

/// Get channel label for a given channel index and total channel count.
/// Reads labels from `speaker_config.rs` (the single source of truth).
/// Note: channel count alone is ambiguous for some layouts (e.g. 8ch = 7.1 or 5.1.2).
/// Use `get_channel_label_from_config()` with a config ID when available.
#[derive(Debug)]
pub struct ChannelConflict {
    pub index: usize,
    pub plugin_type: PluginType,
    pub required_channels: usize,
    pub actual_channels: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub id: usize,
    pub enabled: bool,
    pub settings: PluginSettings,
    /// If true, this plugin cannot be removed from the chain (part of default rack)
    #[serde(default)]
    pub permanent: bool,
    /// Temporarily disabled due to channel incompatibility; auto-restores on compatible tracks
    #[serde(skip)]
    pub suspended: bool,
    /// Optional user-facing name (e.g., "Room EQ", "Broadband EQ"). When None,
    /// the UI falls back to the plugin type display name. Persisted to JSON
    /// so named instances survive save/reload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Plugin {
    pub fn new(id: usize, plugin_type: &PluginType) -> Self {
        Self {
            id,
            enabled: true,
            settings: PluginSettings::default_for(plugin_type),
            permanent: false,
            suspended: false,
            name: None,
        }
    }

    /// Create a permanent plugin that cannot be removed
    pub fn new_permanent(id: usize, plugin_type: &PluginType) -> Self {
        Self {
            id,
            enabled: true,
            settings: PluginSettings::default_for(plugin_type),
            permanent: true,
            suspended: false,
            name: None,
        }
    }

    pub fn plugin_type(&self) -> PluginType {
        self.settings.plugin_type()
    }

    /// Returns true if this plugin is permanent and cannot be removed
    pub fn is_permanent(&self) -> bool {
        self.permanent
    }

    /// User-facing display name. Falls back to the plugin type's static name
    /// when no custom name has been set.
    pub fn display_name(&self) -> String {
        match &self.name {
            Some(n) if !n.is_empty() => n.clone(),
            _ => self.plugin_type().name().to_string(),
        }
    }

    pub fn to_plugin_config(&self, sample_rate: f64) -> Option<PluginConfig> {
        if self.enabled && !self.suspended {
            Some(self.settings.to_plugin_config(sample_rate))
        } else {
            None
        }
    }
}

// ============================================================================
