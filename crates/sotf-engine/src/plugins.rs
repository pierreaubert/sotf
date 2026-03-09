use crate::engine::PluginConfig;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};
use serde_json::json;
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
}

impl PluginType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::EQ => "EQ",
            Self::Gain => "Gain",
            Self::Upmixer => "Upmixer",
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
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::EQ => "Parametric Equalizer IIR",
            Self::Gain => "Simple Volume/Gain Control",
            Self::Upmixer => "Stereo to Surround 5.1 to 9.1.6",
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
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::EQ,
            Self::Gain,
            Self::Upmixer,
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
        ]
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
		| Self::XTC
		=> ReleaseChannel::Prod,

            | Self::ABCompare
		| Self::BandSplit
		| Self::BandMerge
		| Self::Downmix
		| Self::LoudnessCompensation
		| Self::MonoToStereo
		=> ReleaseChannel::Beta,

            Self::BinauralDecoder
		| Self::Convolution
		| Self::Pnd
		| Self::Denoiser
		=> ReleaseChannel::Alpha,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EQFilter {
    pub filter_type: BiquadFilterType,
    pub frequency: f64,
    pub q: f64,
    pub gain_db: f64,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub solo: bool,
}

impl EQFilter {
    pub fn new(filter_type: BiquadFilterType, frequency: f64, q: f64, gain_db: f64) -> Self {
        Self {
            filter_type,
            frequency,
            q,
            gain_db,
            muted: false,
            solo: false,
        }
    }

    pub fn to_biquad(&self, sample_rate: f64) -> Biquad {
        Biquad::new(
            self.filter_type,
            self.frequency,
            sample_rate,
            self.q,
            self.gain_db,
        )
    }

    /// Parse a single APO filter line
    /// Format: "Filter N: ON FILTERTYPE Fc FREQ Hz Gain GAIN dB Q QVAL"
    /// Example: "Filter 1: ON PK Fc 100 Hz Gain -2.0 dB Q 1.41"
    pub fn from_apo_line(line: &str) -> Result<Self, String> {
        let line = line.trim();

        // Skip if filter is OFF
        if line.contains("OFF") {
            return Err("Filter is disabled".to_string());
        }

        // Parse filter type
        let filter_type = if line.contains(" PK ") || line.contains(" PEQ ") {
            BiquadFilterType::Peak
        } else if line.contains(" LSC ") || line.contains(" LOW_SHELF ") || line.contains(" LS ") {
            BiquadFilterType::Lowshelf
        } else if line.contains(" HSC ") || line.contains(" HIGH_SHELF ") || line.contains(" HS ") {
            BiquadFilterType::Highshelf
        } else if line.contains(" LP ") || line.contains(" LPQ ") {
            BiquadFilterType::Lowpass
        } else if line.contains(" HP ") || line.contains(" HPQ ") {
            BiquadFilterType::Highpass
        } else if line.contains(" NO ") || line.contains(" NOTCH ") {
            BiquadFilterType::Notch
        } else if line.contains(" BP ") {
            BiquadFilterType::Bandpass
        } else {
            return Err(format!("Unknown filter type in line: {}", line));
        };

        // Parse frequency (look for "Fc" followed by number)
        let frequency = line
            .split_whitespace()
            .skip_while(|&s| s != "Fc")
            .nth(1)
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| format!("Could not parse frequency from line: {}", line))?;

        // Parse gain (look for "Gain" followed by number)
        let gain_db = line
            .split_whitespace()
            .skip_while(|&s| s != "Gain")
            .nth(1)
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0); // Default to 0 dB if not found (for LP/HP/BP/NO filters)

        // Parse Q (look for "Q" followed by number)
        let q = line
            .split_whitespace()
            .skip_while(|&s| s != "Q")
            .nth(1)
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.707); // Default Q value

        Ok(Self::new(filter_type, frequency, q, gain_db))
    }

    /// Parse APO format file and return a vector of EQ filters
    /// Format:
    /// ```text
    /// Preamp: -6.0 dB
    /// Filter 1: ON PK Fc 100 Hz Gain -2.0 dB Q 1.41
    /// Filter 2: ON LSC Fc 105 Hz Gain 4.1 dB Q 0.71
    /// ```
    pub fn from_apo_file(path: &std::path::Path) -> Result<Vec<Self>, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;

        let mut filters = Vec::new();

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            // Skip preamp lines for now
            if line.to_lowercase().starts_with("preamp:") {
                continue;
            }

            // Try to parse as filter line
            if line.to_lowercase().contains("filter") && line.contains(':') {
                match Self::from_apo_line(line) {
                    Ok(filter) => filters.push(filter),
                    Err(e) => log::warn!("Skipping line '{}': {}", line, e),
                }
            }
        }

        if filters.is_empty() {
            Err("No valid filters found in APO file".to_string())
        } else {
            Ok(filters)
        }
    }
}

// Import param_specs for plugin defaults and serde_param_default! macro
use sotf_plugins::param_specs::ab_compare as ab_compare_specs;
use sotf_plugins::param_specs::band_merge as band_merge_specs;
use sotf_plugins::param_specs::band_split as band_split_specs;
use sotf_plugins::param_specs::binaural as binaural_specs;
use sotf_plugins::param_specs::channel_mute_solo as cms_specs;
use sotf_plugins::param_specs::compressor as compressor_specs;
use sotf_plugins::param_specs::convolution as convolution_specs;
use sotf_plugins::param_specs::crossfeed as crossfeed_specs;
use sotf_plugins::param_specs::denoiser as denoiser_specs;
use sotf_plugins::param_specs::downmix as downmix_specs;
use sotf_plugins::param_specs::expander as expander_specs;
use sotf_plugins::param_specs::find_by_key as pk;
use sotf_plugins::param_specs::fletcher_munson as fm_specs;
use sotf_plugins::param_specs::gain as gain_specs;
use sotf_plugins::param_specs::gate as gate_specs;
use sotf_plugins::param_specs::limiter as limiter_specs;
use sotf_plugins::param_specs::loudness_compensation as lc_specs;
use sotf_plugins::param_specs::matrix as matrix_specs;
use sotf_plugins::param_specs::mono_to_stereo as mono_to_stereo_specs;
use sotf_plugins::param_specs::multiband_compressor as mb_compressor_specs;
use sotf_plugins::param_specs::multiband_expander as mb_expander_specs;
use sotf_plugins::param_specs::pnd as pnd_specs;
use sotf_plugins::param_specs::spectrum as spectrum_specs;
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
}

sotf_plugins::serde_param_default! {
    compressor_specs::PARAMS;
    fn default_compressor_link_channels() -> bool = "link_channels";
    fn default_compressor_sidechain_hpf_hz() -> f64 = "sidechain_hpf_hz";
}

sotf_plugins::serde_param_default! {
    binaural_specs::PARAMS;
    fn default_binaural_enable_optimization() -> bool = "enable_optimization";
}

sotf_plugins::serde_param_default! {
    lc_specs::PARAMS;
    fn default_auto_gain_max_db() -> f64 = "auto_gain_max_db";
    fn default_auto_gain_smoothing_ms() -> f64 = "auto_gain_smoothing_ms";
}

sotf_plugins::serde_param_default! {
    fm_specs::PARAMS;
    fn default_fm_reference_level_db() -> f64 = "reference_level_db";
    fn default_fm_enabled() -> bool = "enabled";
    fn default_fm_smoothing_ms() -> f64 = "smoothing_ms";
    fn default_fm_auto_gain_max_db() -> f64 = "auto_gain_max_db";
    fn default_fm_auto_gain_smoothing_ms() -> f64 = "auto_gain_smoothing_ms";
    fn default_fm_band1_freq() -> f64 = "band1_freq";
    fn default_fm_band1_q() -> f64 = "band1_q";
    fn default_fm_band1_max_gain() -> f64 = "band1_max_gain";
    fn default_fm_band1_slope() -> f64 = "band1_slope";
    fn default_fm_band2_freq() -> f64 = "band2_freq";
    fn default_fm_band2_q() -> f64 = "band2_q";
    fn default_fm_band2_max_gain() -> f64 = "band2_max_gain";
    fn default_fm_band2_slope() -> f64 = "band2_slope";
    fn default_fm_band3_freq() -> f64 = "band3_freq";
    fn default_fm_band3_q() -> f64 = "band3_q";
    fn default_fm_band3_max_gain() -> f64 = "band3_max_gain";
    fn default_fm_band3_slope() -> f64 = "band3_slope";
    fn default_fm_band4_freq() -> f64 = "band4_freq";
    fn default_fm_band4_q() -> f64 = "band4_q";
    fn default_fm_band4_max_gain() -> f64 = "band4_max_gain";
    fn default_fm_band4_slope() -> f64 = "band4_slope";
}

sotf_plugins::serde_param_default! {
    limiter_specs::PARAMS;
    fn default_limiter_lookahead_ms() -> f64 = "lookahead";
    fn default_limiter_soft() -> bool = "soft";
    fn default_limiter_mix() -> f64 = "mix";
}

sotf_plugins::serde_param_default! {
    gate_specs::PARAMS;
    fn default_gate_hold_ms() -> f64 = "hold";
    fn default_gate_mix() -> f64 = "mix";
    fn default_gate_link_channels() -> bool = "link_channels";
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
}

fn default_xtc_head_tracking_smooth_s() -> f64 {
    pk(xtc_specs::PARAMS, "head_tracking_smooth_s").default_f64()
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
}

sotf_plugins::serde_param_default! {
    pnd_specs::PARAMS;
    fn default_pnd_correction_strength() -> f64 = "correction_strength";
    fn default_pnd_analysis_window_ms() -> f64 = "analysis_window_ms";
    fn default_pnd_drift_smoothing() -> f64 = "drift_smoothing";
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

fn default_band_split_crossover_type() -> String {
    let spec = pk(band_split_specs::PARAMS, "type");
    spec.choice_labels()[spec.default_usize()].to_string()
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
}

sotf_plugins::serde_param_default! {
    crossfeed_specs::PARAMS;
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
}

fn default_spectrum_num_bins() -> usize {
    pk(spectrum_specs::PARAMS, "num_bins").default_usize()
}
fn default_spectrum_min_freq() -> f32 {
    pk(spectrum_specs::PARAMS, "min_freq").default_f64() as f32
}
fn default_spectrum_max_freq() -> f32 {
    pk(spectrum_specs::PARAMS, "max_freq").default_f64() as f32
}
fn default_spectrum_smoothing() -> f32 {
    pk(spectrum_specs::PARAMS, "smoothing").default_f64() as f32
}
fn default_spectrum_tilt_correction() -> SpectralTiltCorrection {
    SpectralTiltCorrection::None
}
fn default_spectrum_tilt_reference() -> TiltReferenceFreq {
    TiltReferenceFreq::Standard
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
    },
    Gain {
        #[serde(default = "default_channels")]
        channels: usize,
        gain_db: f64,
    },
    Upmixer {
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
    },
    Limiter {
        threshold_db: f64,
        release_ms: f64,
        #[serde(default = "default_limiter_lookahead_ms")]
        lookahead_ms: f64,
        #[serde(default = "default_limiter_soft")]
        soft: bool,
        #[serde(default = "default_limiter_mix")]
        mix: f64,
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
        bands: Vec<BandCompressorParams>,
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
        #[serde(default)]
        bands: Vec<BandExpanderParams>,
    },
    LoudnessCompensation {
        low_freq: f64,
        low_gain: f64,
        high_freq: f64,
        high_gain: f64,
        #[serde(default)]
        auto_gain_enabled: bool,
        #[serde(default = "default_auto_gain_max_db")]
        auto_gain_max_db: f64,
        #[serde(default = "default_auto_gain_smoothing_ms")]
        auto_gain_smoothing_ms: f64,
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
    },
    Convolution {
        ir_file: String,
        mix: f64,
        gain_db: f64,
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
        #[serde(default)]
        learn_noise: bool,
        #[serde(default = "default_denoiser_use_captured_profile")]
        use_captured_profile: bool,
        #[serde(default)]
        clear_profile: bool,
    },
    Pnd {
        #[serde(default = "default_pnd_correction_strength")]
        correction_strength: f64,
        #[serde(default = "default_pnd_analysis_window_ms")]
        analysis_window_ms: f64,
        #[serde(default = "default_pnd_drift_smoothing")]
        drift_smoothing: f64,
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
    },
    Crossfeed {
        #[serde(default)]
        mode: CrossfeedMode,
        #[serde(default)]
        preset: CrossfeedPreset,
        #[serde(default)]
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
        }
    }

    /// Returns the fixed input channel count this plugin requires, or None if it adapts to any.
    pub fn required_input_channels(&self) -> Option<usize> {
        match self {
            Self::Upmixer { .. } => Some(2),
            Self::XTC { .. } => Some(2),
            Self::Crossfeed { .. } => Some(2),
            Self::MonoToStereo { .. } => Some(1),
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
                    "autogain_enabled": autogain_enabled,
                    "autogain_target_lufs": autogain_target_lufs,
                    "autogain_max_gain_db": autogain_max_gain_db,
                    "autogain_smoothing_ms": autogain_smoothing_ms,
                }),
            ),
            Self::EQ {
                channels,
                filters,
                channel_filters,
                per_channel_mode,
                max_filters: _,
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
                        }),
                    )
                }
            }
            Self::Gain { channels, gain_db } => PluginConfig::new(
                "gain",
                json!({
                    "channels": channels,
                    "gain_db": gain_db,
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
                }),
            ),
            Self::Limiter {
                threshold_db,
                release_ms,
                lookahead_ms,
                soft,
                mix,
            } => PluginConfig::new(
                "limiter",
                json!({
                    "threshold_db": threshold_db,
                    "release_ms": release_ms,
                    "lookahead_ms": lookahead_ms,
                    "soft": soft,
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
                bands,
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
                    "bands": bands,
                }),
            ),
            Self::LoudnessCompensation {
                low_freq,
                low_gain,
                high_freq,
                high_gain,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
            } => PluginConfig::new(
                "loudness_compensation",
                json!({
                    "low_freq": low_freq,
                    "low_gain": low_gain,
                    "high_freq": high_freq,
                    "high_gain": high_gain,
                    "auto_gain_enabled": auto_gain_enabled,
                    "auto_gain_max_db": auto_gain_max_db,
                    "auto_gain_smoothing_ms": auto_gain_smoothing_ms,
                }),
            ),
            Self::FletcherMunson {
                playback_volume_db,
                reference_level_db,
                enabled,
                band1_freq,
                band1_q,
                band1_max_gain,
                band1_slope,
                band2_freq,
                band2_q,
                band2_max_gain,
                band2_slope,
                band3_freq,
                band3_q,
                band3_max_gain,
                band3_slope,
                band4_freq,
                band4_q,
                band4_max_gain,
                band4_slope,
                smoothing_ms,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
                auto_gain_loudness_type,
            } => PluginConfig::new(
                "fletcher_munson",
                json!({
                    "playback_volume_db": playback_volume_db,
                    "reference_level_db": reference_level_db,
                    "band1": {
                        "frequency": band1_freq,
                        "q": band1_q,
                        "max_gain_db": band1_max_gain,
                        "slope": band1_slope,
                    },
                    "band2": {
                        "frequency": band2_freq,
                        "q": band2_q,
                        "max_gain_db": band2_max_gain,
                        "slope": band2_slope,
                    },
                    "band3": {
                        "frequency": band3_freq,
                        "q": band3_q,
                        "max_gain_db": band3_max_gain,
                        "slope": band3_slope,
                    },
                    "band4": {
                        "frequency": band4_freq,
                        "q": band4_q,
                        "max_gain_db": band4_max_gain,
                        "slope": band4_slope,
                    },
                    "smoothing_ms": smoothing_ms,
                    "enabled": enabled,
                    "auto_gain_enabled": auto_gain_enabled,
                    "auto_gain_max_db": auto_gain_max_db,
                    "auto_gain_smoothing_ms": auto_gain_smoothing_ms,
                    "auto_gain_loudness_type": auto_gain_loudness_type,
                }),
            ),
            Self::BinauralDecoder {
                sofa_file,
                input_channels,
                enable_optimization,
                externalization,
                near_field_strength,
            } => PluginConfig::new(
                "binaural_decoder",
                json!({
                    "sofa_file": sofa_file,
                    "input_channels": input_channels,
                    "enable_optimization": enable_optimization,
                    "externalization": externalization,
                    "near_field_strength": near_field_strength,
                }),
            ),
            Self::Convolution {
                ir_file,
                mix,
                gain_db,
            } => PluginConfig::new(
                "convolution",
                json!({
                    "ir_file": ir_file,
                    "mix": mix,
                    "gain_db": gain_db,
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
                channel_states,
            } => PluginConfig::new(
                "channel_mute_solo",
                json!({
                    "enabled": enabled,
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
                learn_noise,
                use_captured_profile,
                clear_profile,
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
                    "learn_noise": learn_noise,
                    "use_captured_profile": use_captured_profile,
                    "clear_profile": clear_profile,
                }),
            ),
            Self::Pnd {
                correction_strength,
                analysis_window_ms,
                drift_smoothing,
            } => PluginConfig::new(
                "pnd",
                json!({
                    "correction_strength": correction_strength,
                    "analysis_window_ms": analysis_window_ms,
                    "drift_smoothing": drift_smoothing,
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
                }),
            ),
            Self::MonoToStereo {
                stereo_width,
                haas_delay_ms,
                enable_comp_eq,
                comp_eq_depth_db,
                decor_low_hz,
                decor_high_hz,
            } => PluginConfig::new(
                "mono_to_stereo",
                json!({
                    "stereo_width": stereo_width,
                    "haas_delay_ms": haas_delay_ms,
                    "enable_comp_eq": enable_comp_eq,
                    "comp_eq_depth_db": comp_eq_depth_db,
                    "decor_low_hz": decor_low_hz,
                    "decor_high_hz": decor_high_hz,
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
            },
            PluginType::Gain => Self::Gain {
                channels: default_channels(),
                gain_db: p(gain_specs::PARAMS, "gain_db").default_f64(),
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
                }
            }
            PluginType::Limiter => {
                let l = limiter_specs::PARAMS;
                Self::Limiter {
                    threshold_db: p(l, "threshold").default_f64(),
                    release_ms: p(l, "release").default_f64(),
                    lookahead_ms: p(l, "lookahead").default_f64(),
                    soft: p(l, "soft").default_bool(),
                    mix: p(l, "mix").default_f64(),
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
                    link_channels: p(e, "link_channels").default_bool(),
                    sidechain_hpf_hz: p(e, "sidechain_hpf_hz").default_f64(),
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
                    bands: Vec::new(),
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
                    auto_gain_enabled: p(lc, "auto_gain_enabled").default_bool(),
                    auto_gain_max_db: p(lc, "auto_gain_max_db").default_f64(),
                    auto_gain_smoothing_ms: p(lc, "auto_gain_smoothing_ms").default_f64(),
                }
            }
            PluginType::FletcherMunson => {
                let fm = fm_specs::PARAMS;
                Self::FletcherMunson {
                    playback_volume_db: 0.0,
                    reference_level_db: p(fm, "reference_level_db").default_f64(),
                    enabled: p(fm, "enabled").default_bool(),
                    band1_freq: p(fm, "band1_freq").default_f64(),
                    band1_q: p(fm, "band1_q").default_f64(),
                    band1_max_gain: p(fm, "band1_max_gain").default_f64(),
                    band1_slope: p(fm, "band1_slope").default_f64(),
                    band2_freq: p(fm, "band2_freq").default_f64(),
                    band2_q: p(fm, "band2_q").default_f64(),
                    band2_max_gain: p(fm, "band2_max_gain").default_f64(),
                    band2_slope: p(fm, "band2_slope").default_f64(),
                    band3_freq: p(fm, "band3_freq").default_f64(),
                    band3_q: p(fm, "band3_q").default_f64(),
                    band3_max_gain: p(fm, "band3_max_gain").default_f64(),
                    band3_slope: p(fm, "band3_slope").default_f64(),
                    band4_freq: p(fm, "band4_freq").default_f64(),
                    band4_q: p(fm, "band4_q").default_f64(),
                    band4_max_gain: p(fm, "band4_max_gain").default_f64(),
                    band4_slope: p(fm, "band4_slope").default_f64(),
                    smoothing_ms: p(fm, "smoothing_ms").default_f64(),
                    auto_gain_enabled: p(fm, "auto_gain_enabled").default_bool(),
                    auto_gain_max_db: p(fm, "auto_gain_max_db").default_f64(),
                    auto_gain_smoothing_ms: p(fm, "auto_gain_smoothing_ms").default_f64(),
                    auto_gain_loudness_type: pk(fm_specs::PARAMS, "auto_gain_loudness_type")
                        .default_f64() as i32,
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
                }
            }
            PluginType::Convolution => {
                let cv = convolution_specs::PARAMS;
                Self::Convolution {
                    ir_file: String::new(),
                    mix: p(cv, "mix").default_f64(),
                    gain_db: p(cv, "gain_db").default_f64(),
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
                    learn_noise: p(d, "learn_noise").default_bool(),
                    use_captured_profile: p(d, "use_captured_profile").default_bool(),
                    clear_profile: p(d, "clear_profile").default_bool(),
                }
            }
            PluginType::Pnd => {
                let pn = pnd_specs::PARAMS;
                Self::Pnd {
                    correction_strength: p(pn, "correction_strength").default_f64(),
                    analysis_window_ms: p(pn, "analysis_window_ms").default_f64(),
                    drift_smoothing: p(pn, "drift_smoothing").default_f64(),
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
                    autogain_enabled: p(cf, "autogain_enabled").default_bool(),
                    autogain_target_lufs: p(cf, "autogain_target_lufs").default_f64(),
                    autogain_max_gain_db: p(cf, "autogain_max_gain_db").default_f64(),
                    autogain_smoothing_ms: p(cf, "autogain_smoothing_ms").default_f64(),
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
pub fn get_channel_label(index: usize, total: usize) -> String {
    if let Some(groups) = sotf_plugins::get_meter_groups_by_channels(total) {
        for group in groups {
            for ch in group.channels {
                if ch.index == index {
                    return ch.label.to_string();
                }
            }
        }
    }
    format!("Ch{}", index)
}

/// Get channel label using a speaker config ID for disambiguation.
/// Falls back to channel-count lookup via `get_channel_label()`.
pub fn get_channel_label_from_config(
    index: usize,
    total: usize,
    speaker_config: Option<&str>,
) -> String {
    if let Some(groups) = speaker_config.and_then(sotf_plugins::get_meter_groups) {
        for group in groups {
            for ch in group.channels {
                if ch.index == index {
                    return ch.label.to_string();
                }
            }
        }
    }
    get_channel_label(index, total)
}

/// Convert linear gain to dB string for display
/// Returns "-∞" for gains below threshold (effectively silent)
pub fn linear_to_db_string(linear: f32) -> String {
    const SILENCE_THRESHOLD: f32 = 0.001; // -60 dB

    if linear < SILENCE_THRESHOLD {
        "-∞".to_string()
    } else {
        format!("{:.1}", 20.0 * linear.log10())
    }
}

/// Convert dB value to linear gain
pub fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

/// Detect which preset a matrix matches, if any
/// Returns the list of presets valid for the given channel configuration.
/// Identity is always available. Other presets require specific channel counts.
pub fn available_matrix_presets(in_ch: usize, out_ch: usize) -> Vec<&'static str> {
    let mut presets = vec!["Identity"];
    if in_ch >= 2 && out_ch >= 2 {
        presets.push("Swap L/R");
    }
    // Mono Mix is only distinct from Identity when in_ch > 1
    if in_ch > 1 {
        presets.push("Mono Mix");
    }
    if in_ch >= 2 && out_ch >= 2 {
        presets.push("M/S Encode");
        presets.push("M/S Decode");
    }
    presets
}

pub fn detect_matrix_preset(in_ch: usize, out_ch: usize, matrix: &[f32]) -> &'static str {
    if is_identity_matrix(in_ch, out_ch, matrix) {
        "Identity"
    } else if is_swap_matrix(in_ch, out_ch, matrix) {
        "Swap L/R"
    } else if is_mono_mix_matrix(in_ch, out_ch, matrix) {
        "Mono Mix"
    } else if is_ms_encode_matrix(in_ch, out_ch, matrix) {
        "M/S Encode"
    } else if is_ms_decode_matrix(in_ch, out_ch, matrix) {
        "M/S Decode"
    } else {
        "Custom"
    }
}

/// Check if matrix is identity (diagonal = 1, rest = 0)
fn is_identity_matrix(in_ch: usize, out_ch: usize, matrix: &[f32]) -> bool {
    if matrix.len() != in_ch * out_ch {
        return false;
    }

    for out in 0..out_ch {
        for inp in 0..in_ch {
            let value = matrix[out * in_ch + inp];
            let expected = if inp == out { 1.0 } else { 0.0 };
            if (value - expected).abs() > 0.001 {
                return false;
            }
        }
    }
    true
}

/// Check if matrix swaps L/R (first two channels swapped, rest pass-through)
/// Requires at least 2 input and 2 output channels.
fn is_swap_matrix(in_ch: usize, out_ch: usize, matrix: &[f32]) -> bool {
    if in_ch < 2 || out_ch < 2 || matrix.len() != in_ch * out_ch {
        return false;
    }

    // Expected pattern: out0←in1, out1←in0, remaining diagonal pass-through
    for out in 0..out_ch {
        for inp in 0..in_ch {
            let value = matrix[out * in_ch + inp];
            let expected =
                if (out == 0 && inp == 1) || (out == 1 && inp == 0) || (out >= 2 && inp == out) {
                    1.0
                } else {
                    0.0
                };
            if (value - expected).abs() > 0.001 {
                return false;
            }
        }
    }
    true
}

/// Check if matrix is a mono mix (all inputs summed equally to all outputs)
/// Uses equal-voltage summing: gain = 1/N where N = number of inputs
/// For stereo: 1/2 = 0.5 = -6dB per channel (preserves level for mono-compatible content)
fn is_mono_mix_matrix(in_ch: usize, out_ch: usize, matrix: &[f32]) -> bool {
    if matrix.len() != in_ch * out_ch || in_ch == 0 {
        return false;
    }

    // Expected gain for equal voltage (mono-compatible) mix
    let expected_gain = 1.0 / (in_ch as f32);

    for value in matrix {
        if (*value - expected_gain).abs() > 0.001 {
            return false;
        }
    }
    true
}

/// Check if matrix is M/S Encode (first two channels encoded, rest pass-through)
/// Mid = 0.5*L + 0.5*R, Side = 0.5*L - 0.5*R
fn is_ms_encode_matrix(in_ch: usize, out_ch: usize, matrix: &[f32]) -> bool {
    if in_ch < 2 || out_ch < 2 || matrix.len() != in_ch * out_ch {
        return false;
    }

    for out in 0..out_ch {
        for inp in 0..in_ch {
            let value = matrix[out * in_ch + inp];
            let expected = match (out, inp) {
                (0, 0) | (0, 1) | (1, 0) => 0.5,
                (1, 1) => -0.5,
                (o, i) if o >= 2 && i == o => 1.0,
                _ => 0.0,
            };
            if (value - expected).abs() > 0.001 {
                return false;
            }
        }
    }
    true
}

/// Check if matrix is M/S Decode (first two channels decoded, rest pass-through)
/// L = Mid + Side, R = Mid - Side
fn is_ms_decode_matrix(in_ch: usize, out_ch: usize, matrix: &[f32]) -> bool {
    if in_ch < 2 || out_ch < 2 || matrix.len() != in_ch * out_ch {
        return false;
    }

    for out in 0..out_ch {
        for inp in 0..in_ch {
            let value = matrix[out * in_ch + inp];
            let expected = match (out, inp) {
                (0, 0) | (0, 1) | (1, 0) => 1.0,
                (1, 1) => -1.0,
                (o, i) if o >= 2 && i == o => 1.0,
                _ => 0.0,
            };
            if (value - expected).abs() > 0.001 {
                return false;
            }
        }
    }
    true
}

/// Apply a preset to the matrix
pub fn apply_matrix_preset(in_ch: usize, out_ch: usize, matrix: &mut Vec<f32>, preset: &str) {
    matrix.resize(in_ch * out_ch, 0.0);
    matrix.fill(0.0);

    match preset {
        "Identity" => {
            for i in 0..in_ch.min(out_ch) {
                matrix[i * in_ch + i] = 1.0;
            }
        }
        "Swap L/R" => {
            if in_ch >= 2 && out_ch >= 2 {
                // Swap first two channels
                matrix[1] = 1.0; // Out 0 <- In 1
                matrix[in_ch] = 1.0; // Out 1 <- In 0
                // Pass through remaining channels
                for i in 2..in_ch.min(out_ch) {
                    matrix[i * in_ch + i] = 1.0;
                }
            }
        }
        "Mono Mix" => {
            // Equal-voltage summing: 1/N per channel
            // For stereo: 1/2 = 0.5 = -6dB per channel
            // This preserves level for mono-compatible content (L=R)
            let gain = 1.0 / (in_ch as f32);
            matrix.fill(gain);
        }
        "M/S Encode" => {
            // Mid/Side encoding (stereo only)
            // Mid = 0.5*L + 0.5*R, Side = 0.5*L - 0.5*R
            if in_ch >= 2 && out_ch >= 2 {
                matrix[0] = 0.5; // Out 0 (Mid) <- 0.5 * In 0 (L)
                matrix[1] = 0.5; // Out 0 (Mid) <- 0.5 * In 1 (R)
                matrix[in_ch] = 0.5; // Out 1 (Side) <- 0.5 * In 0 (L)
                matrix[in_ch + 1] = -0.5; // Out 1 (Side) <- -0.5 * In 1 (R)
                // Pass through remaining channels
                for i in 2..in_ch.min(out_ch) {
                    matrix[i * in_ch + i] = 1.0;
                }
            }
        }
        "M/S Decode" => {
            // Mid/Side decoding (stereo only)
            // L = Mid + Side, R = Mid - Side
            if in_ch >= 2 && out_ch >= 2 {
                matrix[0] = 1.0; // Out 0 (L) <- 1.0 * In 0 (Mid)
                matrix[1] = 1.0; // Out 0 (L) <- 1.0 * In 1 (Side)
                matrix[in_ch] = 1.0; // Out 1 (R) <- 1.0 * In 0 (Mid)
                matrix[in_ch + 1] = -1.0; // Out 1 (R) <- -1.0 * In 1 (Side)
                // Pass through remaining channels
                for i in 2..in_ch.min(out_ch) {
                    matrix[i * in_ch + i] = 1.0;
                }
            }
        }
        _ => {
            // Custom or unknown - set to identity as fallback
            for i in 0..in_ch.min(out_ch) {
                matrix[i * in_ch + i] = 1.0;
            }
        }
    }
}

/// Resize matrix preserving existing values where possible
/// New cells on diagonal get 1.0 (identity), others get 0.0
pub fn resize_matrix(
    matrix: &mut Vec<f32>,
    old_in: usize,
    old_out: usize,
    new_in: usize,
    new_out: usize,
) {
    let mut new_matrix = vec![0.0; new_in * new_out];

    // Copy existing values
    for out in 0..old_out.min(new_out) {
        for inp in 0..old_in.min(new_in) {
            new_matrix[out * new_in + inp] = matrix[out * old_in + inp];
        }
    }

    // Fill diagonal for new channels
    for i in old_in.min(old_out)..new_in.min(new_out) {
        new_matrix[i * new_in + i] = 1.0;
    }

    *matrix = new_matrix;
}

/// Map a speaker configuration string to its output channel count.
fn upmixer_output_channels(speaker_config: &str) -> usize {
    match speaker_config {
        "2.0" => 2,
        "2.1" => 3,
        "2.2" => 4,
        "5.0" => 5,
        "5.1" => 6,
        "7.1" => 8,
        "9.1" => 8,
        "5.1.2" => 8,
        "5.1.4" => 10,
        "7.1.2" => 10,
        "7.1.4" => 12,
        "9.1.2" => 12,
        "9.1.4" => 14,
        "9.1.6" => 16,
        _ => {
            log::warn!(
                "Unknown speaker config '{}', defaulting to 5.1 (6 channels)",
                speaker_config
            );
            6
        }
    }
}

// ============================================================================

/// Versioned wrapper for plugin presets
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PluginPreset {
    #[serde(default = "default_plugin_preset_version")]
    version: u32,
    plugins: Vec<Plugin>,
}

fn default_plugin_preset_version() -> u32 {
    1
}

/// Convert plugin entries from a preset file into a PathConfig JSON string
/// suitable for the AB Compare plugin's path_a_config / path_b_config fields.
pub fn plugins_to_path_config_json(plugins: &[Plugin], sample_rate: f64) -> String {
    let configs: Vec<serde_json::Value> = plugins
        .iter()
        .filter(|p| p.enabled)
        .map(|p| {
            let pc = p.settings.to_plugin_config(sample_rate);
            json!({"plugin_type": pc.plugin_type, "parameters": pc.parameters})
        })
        .collect();
    let path_config = match configs.len() {
        0 => json!({"type": "None"}),
        1 => {
            json!({
                "type": "Plugin",
                "plugin_type": configs[0]["plugin_type"],
                "parameters": configs[0]["parameters"],
            })
        }
        _ => json!({"type": "Rack", "plugins": configs}),
    };
    serde_json::to_string(&path_config).unwrap()
}

/// Parse a preset JSON file into a PathConfig JSON string for use in AB Compare.
/// The file is expected to be a `PluginPreset` (with version + plugins array).
pub fn preset_file_to_path_config_json(json_content: &str, sample_rate: f64) -> Result<String, String> {
    let preset: PluginPreset = serde_json::from_str(json_content)
        .map_err(|e| format!("Invalid preset file: {}", e))?;
    Ok(plugins_to_path_config_json(&preset.plugins, sample_rate))
}

/// A plugin that is incompatible with the current channel count.
#[derive(Debug, Clone)]
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
}

impl Plugin {
    pub fn new(id: usize, plugin_type: &PluginType) -> Self {
        Self {
            id,
            enabled: true,
            settings: PluginSettings::default_for(plugin_type),
            permanent: false,
            suspended: false,
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
        }
    }

    pub fn plugin_type(&self) -> PluginType {
        self.settings.plugin_type()
    }

    /// Returns true if this plugin is permanent and cannot be removed
    pub fn is_permanent(&self) -> bool {
        self.permanent
    }

    pub fn to_plugin_config(&self, sample_rate: f64) -> Option<PluginConfig> {
        if self.enabled && !self.suspended {
            Some(self.settings.to_plugin_config(sample_rate))
        } else {
            None
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct PluginChain {
    plugins: Vec<Plugin>,
    next_id: usize,
}

impl PluginChain {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_plugin(&mut self, plugin_type: &PluginType) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.plugins.push(Plugin::new(id, plugin_type));
        id
    }

    /// Add a permanent plugin that cannot be removed
    pub fn add_permanent_plugin(&mut self, plugin_type: &PluginType) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.plugins.push(Plugin::new_permanent(id, plugin_type));
        id
    }

    /// Add a permanent plugin that starts disabled (passthrough)
    pub fn add_permanent_disabled_plugin(&mut self, plugin_type: &PluginType) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        let mut plugin = Plugin::new_permanent(id, plugin_type);
        plugin.enabled = false;
        self.plugins.push(plugin);
        id
    }

    /// Create a default rack with permanent Input Monitor, ReplayGain, Matrix, and Output Monitor
    pub fn with_default_rack() -> Self {
        let mut chain = Self::new();
        // Input monitor (permanent) - monitors input signal
        chain.add_permanent_plugin(&PluginType::LoudnessMonitor);
        // ReplayGain (permanent) - applies track/album replay gain correction
        chain.add_permanent_disabled_plugin(&PluginType::Gain);
        // Matrix (permanent) - channel routing
        chain.add_permanent_plugin(&PluginType::Matrix);
        // Output monitor (permanent) - monitors output signal
        chain.add_permanent_plugin(&PluginType::LoudnessMonitor);
        chain
    }

    /// Ensure the default rack (input monitor, replay gain, matrix, output monitor) is present.
    /// Adds missing permanent plugins without disturbing existing user plugins.
    /// Call this after loading a preset to guarantee the rack structure.
    pub fn ensure_default_rack(&mut self) {
        let has_permanent_lm = self
            .plugins
            .iter()
            .any(|p| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor));
        let has_permanent_matrix = self
            .plugins
            .iter()
            .any(|p| p.permanent && matches!(p.plugin_type(), PluginType::Matrix));
        let has_permanent_gain = self
            .plugins
            .iter()
            .any(|p| p.permanent && matches!(p.plugin_type(), PluginType::Gain));

        if has_permanent_lm && has_permanent_matrix && has_permanent_gain {
            // Check we have at least two permanent LoudnessMonitors (input + output)
            let lm_count = self
                .plugins
                .iter()
                .filter(|p| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor))
                .count();
            if lm_count >= 2 {
                return; // Rack is already complete
            }
        }

        // Rebuild: collect user (non-permanent) plugins, then wrap them in the default rack
        let user_plugins: Vec<Plugin> = self.plugins.drain(..).filter(|p| !p.permanent).collect();

        // Build fresh rack
        let input_id = self.next_id;
        self.next_id += 1;
        self.plugins.push(Plugin::new_permanent(
            input_id,
            &PluginType::LoudnessMonitor,
        ));

        // ReplayGain (permanent, starts disabled)
        let gain_id = self.next_id;
        self.next_id += 1;
        let mut gain_plugin = Plugin::new_permanent(gain_id, &PluginType::Gain);
        gain_plugin.enabled = false;
        self.plugins.push(gain_plugin);

        // Insert user plugins between replay gain and matrix
        self.plugins.extend(user_plugins);

        let matrix_id = self.next_id;
        self.next_id += 1;
        self.plugins
            .push(Plugin::new_permanent(matrix_id, &PluginType::Matrix));

        let output_id = self.next_id;
        self.next_id += 1;
        self.plugins.push(Plugin::new_permanent(
            output_id,
            &PluginType::LoudnessMonitor,
        ));

        log::info!("Ensured default rack: {} plugins total", self.plugins.len());
    }

    /// Find the index where user plugins should be inserted (before Matrix)
    /// Returns the index of the Matrix plugin, or the first permanent plugin after user plugins
    pub fn user_plugin_insert_index(&self) -> usize {
        // Find the Matrix plugin - user plugins go before it
        for (idx, plugin) in self.plugins.iter().enumerate() {
            if plugin.plugin_type() == PluginType::Matrix && plugin.is_permanent() {
                return idx;
            }
        }
        // Fallback: find processing insert index
        self.find_processing_insert_index()
    }

    /// Set the replay gain value on the permanent Gain plugin.
    /// When `gain_db` is `Some`, the plugin is enabled with the given gain.
    /// When `None`, the plugin is disabled (passthrough).
    pub fn set_replay_gain(&mut self, gain_db: Option<f64>) {
        if let Some(plugin) = self
            .plugins
            .iter_mut()
            .find(|p| p.permanent && matches!(p.plugin_type(), PluginType::Gain))
        {
            match gain_db {
                Some(db) => {
                    plugin.enabled = true;
                    plugin.settings = PluginSettings::Gain {
                        channels: match &plugin.settings {
                            PluginSettings::Gain { channels, .. } => *channels,
                            _ => 2,
                        },
                        gain_db: db,
                    };
                }
                None => {
                    plugin.enabled = false;
                }
            }
        }
    }

    /// Read the current replay gain value from the permanent Gain plugin.
    /// Returns `None` if the plugin is disabled or not found.
    pub fn replay_gain_db(&self) -> Option<f64> {
        self.plugins
            .iter()
            .find(|p| p.permanent && matches!(p.plugin_type(), PluginType::Gain))
            .and_then(|p| {
                if p.enabled {
                    match &p.settings {
                        PluginSettings::Gain { gain_db, .. } => Some(*gain_db),
                        _ => None,
                    }
                } else {
                    None
                }
            })
    }

    pub fn remove_plugin(&mut self, index: usize) -> Option<Plugin> {
        if index < self.plugins.len() {
            // Don't remove permanent plugins
            if self.plugins[index].is_permanent() {
                return None;
            }
            Some(self.plugins.remove(index))
        } else {
            None
        }
    }

    /// Check if a plugin at the given index can be removed
    pub fn can_remove_plugin(&self, index: usize) -> bool {
        if let Some(plugin) = self.plugins.get(index) {
            !plugin.is_permanent()
        } else {
            false
        }
    }

    pub fn get_plugin(&self, index: usize) -> Option<&Plugin> {
        self.plugins.get(index)
    }

    pub fn get_plugin_mut(&mut self, index: usize) -> Option<&mut Plugin> {
        self.plugins.get_mut(index)
    }

    pub fn plugins(&self) -> &[Plugin] {
        &self.plugins
    }

    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    pub fn toggle_plugin(&mut self, index: usize) {
        if let Some(plugin) = self.plugins.get_mut(index) {
            plugin.enabled = !plugin.enabled;
        }
    }

    pub fn move_plugin(&mut self, from: usize, to: usize) {
        if from < self.plugins.len()
            && to < self.plugins.len()
            && !self.plugins[from].is_permanent()
            && !self.plugins[to].is_permanent()
        {
            let plugin = self.plugins.remove(from);
            self.plugins.insert(to, plugin);
        }
    }

    /// Check if a plugin at the given index can be moved in the given direction
    pub fn can_move_plugin_up(&self, index: usize) -> bool {
        index > 0
            && index < self.plugins.len()
            && !self.plugins[index].is_permanent()
            && !self.plugins[index - 1].is_permanent()
    }

    /// Check if a plugin at the given index can be moved down
    pub fn can_move_plugin_down(&self, index: usize) -> bool {
        index < self.plugins.len().saturating_sub(1)
            && !self.plugins[index].is_permanent()
            && !self.plugins[index + 1].is_permanent()
    }

    /// Insert a plugin at a specific index
    pub fn insert_plugin(&mut self, index: usize, plugin_type: &PluginType) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        let insert_idx = index.min(self.plugins.len());
        self.plugins
            .insert(insert_idx, Plugin::new(id, plugin_type));
        id
    }

    /// Find the index of the first plugin of a given type
    pub fn find_plugin_index(&self, plugin_type: &PluginType) -> Option<usize> {
        self.plugins
            .iter()
            .position(|p| p.plugin_type() == *plugin_type)
    }

    /// Returns true if the plugin at `index` is the input monitor
    /// (first permanent LoudnessMonitor in the chain)
    pub fn is_input_monitor(&self, index: usize) -> bool {
        let first_permanent_lm = self
            .plugins
            .iter()
            .position(|p| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor));
        first_permanent_lm == Some(index)
    }

    /// Returns true if the plugin at `index` is the output monitor
    /// (last permanent LoudnessMonitor in the chain, distinct from the input monitor)
    pub fn is_output_monitor(&self, index: usize) -> bool {
        let last_permanent_lm = self
            .plugins
            .iter()
            .enumerate()
            .rev()
            .find(|(_, p)| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor))
            .map(|(i, _)| i);
        let first_permanent_lm = self
            .plugins
            .iter()
            .position(|p| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor));
        // Only true if the last permanent LM is different from the first (i.e., there are at least two)
        last_permanent_lm == Some(index) && last_permanent_lm != first_permanent_lm
    }

    /// Check if the chain has an enabled spectrum analyzer plugin
    pub fn has_enabled_spectrum_analyzer(&self) -> bool {
        self.plugins
            .iter()
            .any(|p| p.enabled && matches!(p.settings, PluginSettings::SpectrumAnalyzer { .. }))
    }

    /// Find the insertion index for a new processing plugin (before monitoring plugins)
    pub fn find_processing_insert_index(&self) -> usize {
        // Find the first monitoring plugin
        for (idx, plugin) in self.plugins.iter().enumerate() {
            if plugin.plugin_type().is_monitoring() {
                return idx;
            }
        }
        // No monitoring plugins, insert at end
        self.plugins.len()
    }

    /// Map a UI plugin index (from self.plugins) to the index in the engine's processing chain.
    /// Returns None if the plugin is disabled (not in engine).
    ///
    /// The engine order is:
    /// 1. First LoudnessMonitor (input monitor) - index 0
    /// 2. Processing plugins - indices 1..N
    /// 3. Other monitoring plugins (subsequent LoudnessMonitors, Spectrum, etc.) - at the end
    pub fn get_engine_index(&self, ui_index: usize) -> Option<usize> {
        let target_plugin = self.plugins.get(ui_index)?;
        if !target_plugin.enabled || target_plugin.suspended {
            return None;
        }

        // Determine if this is the first permanent LoudnessMonitor (input monitor)
        let first_permanent_loudness_idx = self
            .plugins
            .iter()
            .position(|p| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor));
        let target_is_first_loudness = first_permanent_loudness_idx == Some(ui_index)
            && matches!(target_plugin.plugin_type(), PluginType::LoudnessMonitor);

        if target_is_first_loudness {
            // First permanent LoudnessMonitor is always at engine index 0
            return Some(0);
        }

        let target_is_monitor = target_plugin.plugin_type().is_monitoring();

        // Check if there's an enabled input monitor (counts toward engine offset)
        // An input monitor exists in the engine if the first permanent one is enabled.
        let has_input_monitor = first_permanent_loudness_idx
            .and_then(|idx| self.plugins.get(idx))
            .map(|p| p.enabled && !p.suspended)
            .unwrap_or(false);
        let input_monitor_offset = if has_input_monitor { 1 } else { 0 };

        if !target_is_monitor {
            // Target is a processing plugin.
            // Engine index is input_monitor_offset + count of enabled processing plugins before it.
            let mut engine_idx = input_monitor_offset;
            for (i, p) in self.plugins.iter().enumerate() {
                if i == ui_index {
                    return Some(engine_idx);
                }
                if p.enabled && !p.suspended && !p.plugin_type().is_monitoring() {
                    engine_idx += 1;
                }
            }
        } else {
            // Target is a monitoring plugin (but not first permanent LoudnessMonitor).
            // Engine index is input_monitor_offset + (all enabled processing plugins) + (count of enabled monitors before it, excluding first permanent LoudnessMonitor).

            // 1. Count all enabled processing plugins
            let mut engine_idx = input_monitor_offset;
            for p in &self.plugins {
                if p.enabled && !p.suspended && !p.plugin_type().is_monitoring() {
                    engine_idx += 1;
                }
            }

            // 2. Count enabled monitors until we hit target (skip first permanent LoudnessMonitor)
            for (i, p) in self.plugins.iter().enumerate() {
                if Some(i) == first_permanent_loudness_idx {
                    continue; // Skip first permanent LoudnessMonitor
                }
                if i == ui_index {
                    return Some(engine_idx);
                }
                if p.enabled && !p.suspended && p.plugin_type().is_monitoring() {
                    engine_idx += 1;
                }
            }
        }

        None
    }

    /// Get the engine index of the input loudness monitor (first permanent LoudnessMonitor).
    pub fn input_monitor_engine_index(&self) -> Option<usize> {
        let ui_idx = self
            .plugins
            .iter()
            .position(|p| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor))?;
        self.get_engine_index(ui_idx)
    }

    /// Get the engine index of the output loudness monitor
    /// (last permanent LoudnessMonitor, if distinct from input).
    pub fn output_monitor_engine_index(&self) -> Option<usize> {
        let ui_idx = self
            .plugins
            .iter()
            .enumerate()
            .rev()
            .find(|(_, p)| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor))
            .map(|(i, _)| i)?;
        // Only valid if different from the input monitor
        let first = self
            .plugins
            .iter()
            .position(|p| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor));
        if first == Some(ui_idx) {
            return None;
        }
        self.get_engine_index(ui_idx)
    }

    /// Get the engine index of the permanent Matrix plugin.
    pub fn matrix_engine_index(&self) -> Option<usize> {
        let ui_idx = self
            .plugins
            .iter()
            .position(|p| p.permanent && matches!(p.plugin_type(), PluginType::Matrix))?;
        self.get_engine_index(ui_idx)
    }

    /// Get the engine index of the first enabled spectrum analyzer.
    pub fn spectrum_engine_index(&self) -> Option<usize> {
        let ui_idx = self
            .plugins
            .iter()
            .position(|p| p.enabled && matches!(p.plugin_type(), PluginType::SpectrumAnalyzer))?;
        self.get_engine_index(ui_idx)
    }

    /// Get the engine index of the first enabled compressor.
    pub fn compressor_engine_index(&self) -> Option<usize> {
        let ui_idx = self
            .plugins
            .iter()
            .position(|p| p.enabled && matches!(p.plugin_type(), PluginType::Compressor))?;
        self.get_engine_index(ui_idx)
    }

    pub fn to_plugin_configs(&self, sample_rate: f64) -> Vec<PluginConfig> {
        // Separate plugins into three categories:
        // 1. Input monitor (the first permanent LoudnessMonitor)
        // 2. Processing plugins - transform the audio
        // 3. Output analyzers (subsequent LoudnessMonitors, Spectrum, etc.)
        let mut input_monitor: Option<PluginConfig> = None;
        let mut processing_plugins = Vec::new();
        let mut analyzer_plugins = Vec::new();

        // Identify which plugin should be the input monitor.
        // It's the first permanent LoudnessMonitor.
        let first_permanent_loudness_idx = self
            .plugins
            .iter()
            .position(|p| p.permanent && matches!(p.plugin_type(), PluginType::LoudnessMonitor));

        for (idx, plugin) in self.plugins.iter().enumerate() {
            if let Some(config) = plugin.to_plugin_config(sample_rate) {
                match plugin.plugin_type() {
                    PluginType::LoudnessMonitor => {
                        if Some(idx) == first_permanent_loudness_idx {
                            input_monitor = Some(config);
                        } else {
                            analyzer_plugins.push(config);
                        }
                    }
                    // Other analyzer plugins always go at the end
                    PluginType::SpectrumAnalyzer | PluginType::ChannelMuteSolo => {
                        analyzer_plugins.push(config);
                    }
                    // Processing plugins maintain their order
                    _ => {
                        processing_plugins.push(config);
                    }
                }
            }
        }

        // Concatenate: input monitor, then processing, then output analyzers
        let mut result = Vec::new();
        if let Some(monitor) = input_monitor {
            result.push(monitor);
        }
        result.extend(processing_plugins);
        result.extend(analyzer_plugins);
        result
    }

    /// Get the speaker configuration ID from the last enabled upmixer/binaural decoder
    /// Returns None if no channel-changing plugin is active
    pub fn output_speaker_config(&self) -> Option<&str> {
        for plugin in self.plugins.iter().rev() {
            if !plugin.enabled {
                continue;
            }

            match &plugin.settings {
                PluginSettings::Upmixer { speaker_config, .. } => {
                    return Some(speaker_config.as_str());
                }
                PluginSettings::BinauralDecoder { .. } => {
                    return Some("2.0");
                }
                _ => continue,
            }
        }
        None
    }

    /// Get the speaker configuration string active at a given plugin index
    /// Walks forward through the chain, tracking config changes from upmixer/binaural/downmix/mono-to-stereo
    pub fn speaker_config_at_index(&self, target_index: usize) -> Option<String> {
        let mut config: Option<String> = None;
        for (i, plugin) in self.plugins.iter().enumerate() {
            if i >= target_index {
                break;
            }
            if !plugin.enabled {
                continue;
            }
            match &plugin.settings {
                PluginSettings::Upmixer { speaker_config, .. } => {
                    config = Some(speaker_config.clone());
                }
                PluginSettings::BinauralDecoder { .. }
                | PluginSettings::Downmix { .. }
                | PluginSettings::MonoToStereo { .. } => {
                    config = Some("2.0".to_string());
                }
                _ => {}
            }
        }
        config
    }

    pub fn output_channels(&self) -> usize {
        self.output_channels_for_input(2)
    }

    /// Returns the output channel count of the plugin chain given the input channel count.
    /// If no channel-changing plugin is found, the input channel count passes through unchanged.
    pub fn output_channels_for_input(&self, input_channels: usize) -> usize {
        // Walk backwards through the chain to find the last channel-count-changing plugin
        for plugin in self.plugins.iter().rev() {
            if !plugin.enabled || plugin.suspended {
                continue;
            }

            match &plugin.settings {
                PluginSettings::Upmixer { speaker_config, .. } => {
                    return upmixer_output_channels(speaker_config);
                }
                PluginSettings::BinauralDecoder { .. } => {
                    return 2;
                }
                PluginSettings::Downmix { .. } => {
                    return 2;
                }
                PluginSettings::MonoToStereo { .. } => {
                    return 2;
                }
                PluginSettings::Matrix {
                    output_channels, ..
                } => {
                    return *output_channels;
                }
                _ => continue,
            }
        }

        // No channel-changing plugin found, input channels pass through
        input_channels
    }

    /// Adapt the matrix plugin to match the file's channel count.
    /// When a multichannel file is loaded but the matrix was configured for stereo
    /// (or vice versa), this resizes the matrix and its channel states to match.
    /// Should be called before `to_plugin_configs()` when the file channel count is known.
    pub fn adapt_matrix_to_input(&mut self, file_channels: usize) {
        let mut running_channels = file_channels;
        for plugin in &mut self.plugins {
            if !plugin.enabled || plugin.suspended {
                continue;
            }
            // Track channel changes from plugins before the matrix
            match &plugin.settings {
                PluginSettings::Upmixer { speaker_config, .. } => {
                    running_channels = upmixer_output_channels(speaker_config);
                    continue;
                }
                PluginSettings::BinauralDecoder { .. } => {
                    running_channels = 2;
                    continue;
                }
                PluginSettings::Downmix { .. } => {
                    running_channels = 2;
                    continue;
                }
                PluginSettings::MonoToStereo { .. } => {
                    running_channels = 2;
                    continue;
                }
                _ => {}
            }
            if let PluginSettings::Matrix {
                input_channels,
                output_channels,
                matrix,
                channel_states,
            } = &mut plugin.settings
            {
                if *input_channels != running_channels {
                    log::info!(
                        "[PluginChain] Adapting matrix from {}x{} to {}x{} (file={}, after chain)",
                        input_channels,
                        output_channels,
                        running_channels,
                        running_channels,
                        file_channels
                    );
                    resize_matrix(
                        matrix,
                        *input_channels,
                        *output_channels,
                        running_channels,
                        running_channels,
                    );
                    *input_channels = running_channels;
                    *output_channels = running_channels;
                    channel_states.resize(running_channels, sotf_plugins::ChannelState::default());
                }
                break; // Only adapt the first enabled matrix
            }
        }
    }

    /// Find all enabled (non-suspended) plugins incompatible with the given input channel count.
    /// Walks the chain tracking running channel count through channel-changing plugins.
    pub fn find_channel_conflicts(&self, input_channels: usize) -> Vec<ChannelConflict> {
        let mut conflicts = Vec::new();
        let mut running_channels = input_channels;

        for (index, plugin) in self.plugins.iter().enumerate() {
            if !plugin.enabled || plugin.suspended {
                continue;
            }

            if let Some(required) = plugin.settings.required_input_channels()
                && required != running_channels {
                    conflicts.push(ChannelConflict {
                        index,
                        plugin_type: plugin.plugin_type(),
                        required_channels: required,
                        actual_channels: running_channels,
                    });
                    continue;
                }

            // Track channel changes through the chain
            match &plugin.settings {
                PluginSettings::Upmixer { speaker_config, .. } => {
                    running_channels = upmixer_output_channels(speaker_config);
                }
                PluginSettings::BinauralDecoder { .. } => {
                    running_channels = 2;
                }
                PluginSettings::Downmix { .. } => {
                    running_channels = 2;
                }
                PluginSettings::MonoToStereo { .. } => {
                    running_channels = 2;
                }
                PluginSettings::Matrix {
                    output_channels, ..
                } => {
                    running_channels = *output_channels;
                }
                PluginSettings::BandSplit { .. } => {
                    running_channels *= 2;
                }
                PluginSettings::BandMerge { bands, .. } => {
                    running_channels /= if *bands > 0 { *bands } else { 2 };
                }
                _ => {}
            }
        }

        conflicts
    }

    /// Suspend the plugins at the given indices (set suspended = true).
    pub fn suspend_plugins(&mut self, indices: &[usize]) {
        for &idx in indices {
            if let Some(plugin) = self.plugins.get_mut(idx) {
                plugin.suspended = true;
            }
        }
    }

    /// Clear all suspensions (set suspended = false on all plugins).
    pub fn clear_suspensions(&mut self) {
        for plugin in &mut self.plugins {
            plugin.suspended = false;
        }
    }

    /// Returns true if any plugin is currently suspended.
    pub fn has_suspensions(&self) -> bool {
        self.plugins.iter().any(|p| p.suspended)
    }

    /// Save the plugin chain to a JSON file
    ///
    /// # Arguments
    /// * `presets_dir` - Directory to save the preset file
    /// * `filename` - The preset filename (with or without .json extension)
    ///
    /// # Returns
    /// * Ok(()) on success
    /// * Err if the extension is not .json or if saving fails
    pub fn save_to_file(
        &self,
        presets_dir: &std::path::Path,
        filename: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Validate extension - must be .json or none
        let path = std::path::Path::new(filename);
        let extension = path.extension().and_then(|ext| ext.to_str());

        // Check if user specified a non-json extension
        if let Some(ext) = extension
            && ext != "json"
        {
            return Err(format!(
                "Only .json files are supported. Please use .json extension instead of .{}",
                ext
            )
            .into());
        }

        // Auto-append .json if no extension provided
        let filename = if extension.is_none() {
            format!("{}.json", filename)
        } else {
            filename.to_string()
        };

        let full_path = presets_dir.join(&filename);

        // Wrap plugins in versioned preset
        let preset = PluginPreset {
            version: default_plugin_preset_version(),
            plugins: self.plugins.clone(),
        };

        // Save to file
        let json = serde_json::to_string_pretty(&preset)?;
        std::fs::write(&full_path, json)?;

        log::info!("Saved plugin chain to {}", full_path.display());
        Ok(())
    }

    /// Load the plugin chain from a JSON file
    ///
    /// # Arguments
    /// * `presets_dir` - Directory containing the preset files
    /// * `filename` - The preset filename (with or without .json extension)
    ///
    /// # Returns
    /// * Ok(()) on success
    /// * Err if the file doesn't exist or loading fails
    pub fn load_from_file(
        &mut self,
        presets_dir: &std::path::Path,
        filename: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Auto-append .json if not already present
        let path = std::path::Path::new(filename);
        let final_filename = if path.extension().and_then(|e| e.to_str()) == Some("json") {
            filename.to_string()
        } else {
            format!("{}.json", filename)
        };

        log::debug!(
            "Loading plugin chain from filename: {} (original: {})",
            final_filename,
            filename
        );

        let full_path = presets_dir.join(&final_filename);
        log::debug!("Full path: {}", full_path.display());

        // Load from file
        let json = std::fs::read_to_string(&full_path)?;
        log::debug!("Read {} bytes from file", json.len());

        // Try to load as versioned preset first
        let mut preset: PluginPreset = match serde_json::from_str(&json) {
            Ok(p) => p,
            Err(_) => {
                // Fall back to loading as legacy format (direct Vec<Plugin>)
                log::info!("Loading legacy plugin preset format (no version field)");
                let plugins: Vec<Plugin> = serde_json::from_str(&json)?;
                PluginPreset {
                    version: 0, // Mark as legacy
                    plugins,
                }
            }
        };

        // Check if migration is needed
        const LATEST_VERSION: u32 = 1;
        let original_version = preset.version;

        if preset.version < LATEST_VERSION {
            log::info!(
                "Migrating plugin preset from version {} to {}",
                original_version,
                LATEST_VERSION
            );

            // Apply migrations
            preset = Self::migrate_preset(preset)?;

            // Save upgraded preset back to disk
            self.plugins = preset.plugins.clone();
            self.save_to_file(presets_dir, &final_filename)?;

            log::info!(
                "Successfully migrated plugin preset from version {} to {}",
                original_version,
                LATEST_VERSION
            );
        }

        log::debug!("Deserialized {} plugins", preset.plugins.len());

        // Update next_id to be higher than any loaded plugin id
        let max_id = preset.plugins.iter().map(|p| p.id).max().unwrap_or(0);
        self.next_id = max_id + 1;

        self.plugins = preset.plugins;

        // Ensure the default rack (input monitor, matrix, output monitor) is present
        // even if the saved preset predates the rack system.
        self.ensure_default_rack();

        log::info!(
            "Loaded plugin chain from {} ({} plugins)",
            full_path.display(),
            self.plugins.len()
        );
        Ok(())
    }

    /// Apply all necessary migrations to bring a plugin preset to the latest version
    fn migrate_preset(
        mut preset: PluginPreset,
    ) -> Result<PluginPreset, Box<dyn std::error::Error>> {
        const LATEST_VERSION: u32 = 1;

        // Apply migrations sequentially
        while preset.version < LATEST_VERSION {
            match preset.version {
                // Migration from legacy format (version 0) to version 1
                0 => {
                    log::info!("Applying plugin preset migration: v0 (legacy) -> v1");
                    // No structural changes needed for now
                    // Future migrations might need to transform plugin settings
                    preset.version = 1;
                }

                // Example migration from version 1 to 2:
                // 1 => {
                //     log::info!("Applying plugin preset migration: v1 -> v2");
                //     // Apply migration logic here
                //     // e.g., transform plugin parameters, rename fields, etc.
                //     preset.version = 2;
                // }

                // If we reach here with no match, we have an unknown version
                v => {
                    return Err(format!("Unknown plugin preset version: {}", v).into());
                }
            }
        }

        Ok(preset)
    }

    /// Update input channels for plugins that depend on the output of previous plugins (BinauralDecoder, Matrix)
    /// This should be called after any plugin chain modification (add, remove, move, toggle)
    pub fn update_channel_dependent_plugins(&mut self) {
        let mut current_channels = 2; // Start with stereo

        for i in 0..self.plugins.len() {
            // Update plugins that depend on input channels
            // We use a temporary clone to check if update is needed to avoid borrow checker issues if we modify in place
            // actually we can modify in place if we match &mut settings

            let mut updated_settings = None;

            match &self.plugins[i].settings {
                PluginSettings::EQ {
                    channels,
                    filters,
                    channel_filters,
                    per_channel_mode,
                    max_filters,
                } => {
                    if *channels != current_channels {
                        updated_settings = Some(PluginSettings::EQ {
                            channels: current_channels,
                            filters: filters.clone(),
                            channel_filters: channel_filters.clone(),
                            per_channel_mode: *per_channel_mode,
                            max_filters: *max_filters,
                        });
                    }
                }
                PluginSettings::Gain { channels, gain_db } => {
                    if *channels != current_channels {
                        updated_settings = Some(PluginSettings::Gain {
                            channels: current_channels,
                            gain_db: *gain_db,
                        });
                    }
                }
                PluginSettings::BinauralDecoder {
                    sofa_file,
                    input_channels,
                    enable_optimization,
                    externalization,
                    near_field_strength,
                } => {
                    if *input_channels != current_channels {
                        updated_settings = Some(PluginSettings::BinauralDecoder {
                            sofa_file: sofa_file.clone(),
                            input_channels: current_channels,
                            enable_optimization: *enable_optimization,
                            externalization: *externalization,
                            near_field_strength: *near_field_strength,
                        });
                    }
                }
                PluginSettings::Matrix {
                    input_channels,
                    output_channels,
                    matrix,
                    channel_states,
                } => {
                    if *input_channels != current_channels {
                        // Resize matrix to match new input channels (square matrix)
                        // allowing it to act as pass-through/identity by default
                        let mut new_matrix = matrix.clone();
                        resize_matrix(
                            &mut new_matrix,
                            *input_channels,
                            *output_channels,
                            current_channels,
                            current_channels,
                        );

                        updated_settings = Some(PluginSettings::Matrix {
                            input_channels: current_channels,
                            output_channels: current_channels,
                            matrix: new_matrix,
                            channel_states: channel_states.clone(),
                        });
                    }
                }
                PluginSettings::Downmix {
                    input_channels,
                    center_gain_db,
                    surround_gain_db,
                    height_gain_db,
                    lfe_gain_db,
                    phase_coherence,
                    phase_blend_low_hz,
                    phase_blend_high_hz,
                } => {
                    if *input_channels != current_channels {
                        updated_settings = Some(PluginSettings::Downmix {
                            input_channels: current_channels,
                            center_gain_db: *center_gain_db,
                            surround_gain_db: *surround_gain_db,
                            height_gain_db: *height_gain_db,
                            lfe_gain_db: *lfe_gain_db,
                            phase_coherence: *phase_coherence,
                            phase_blend_low_hz: *phase_blend_low_hz,
                            phase_blend_high_hz: *phase_blend_high_hz,
                        });
                    }
                }
                PluginSettings::BandSplit {
                    channels,
                    frequency,
                    crossover_type,
                } => {
                    if *channels != current_channels {
                        updated_settings = Some(PluginSettings::BandSplit {
                            channels: current_channels,
                            frequency: *frequency,
                            crossover_type: crossover_type.clone(),
                        });
                    }
                }
                PluginSettings::BandMerge { channels, bands } => {
                    if *channels != current_channels {
                        updated_settings = Some(PluginSettings::BandMerge {
                            channels: current_channels,
                            bands: *bands,
                        });
                    }
                }
                _ => {}
            }

            if let Some(new_settings) = updated_settings {
                self.plugins[i].settings = new_settings;
            }

            // Update output channels for next plugin
            if self.plugins[i].enabled && !self.plugins[i].suspended {
                match &self.plugins[i].settings {
                    PluginSettings::Upmixer { speaker_config, .. } => {
                        current_channels = upmixer_output_channels(speaker_config);
                    }
                    PluginSettings::BinauralDecoder { .. } => {
                        current_channels = 2;
                    }
                    PluginSettings::Matrix {
                        output_channels, ..
                    } => {
                        current_channels = *output_channels;
                    }
                    PluginSettings::Downmix { .. } => {
                        current_channels = 2; // Downmix always produces stereo
                    }
                    PluginSettings::MonoToStereo { .. } => {
                        current_channels = 2; // MonoToStereo always produces stereo
                    }
                    PluginSettings::BandSplit { .. } => {
                        current_channels *= 2; // Split into 2 bands
                    }
                    PluginSettings::BandMerge { bands, .. } => {
                        current_channels /= if *bands > 0 { *bands } else { 2 };
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_chain() {
        let mut chain = PluginChain::new();
        assert_eq!(chain.len(), 0);

        chain.add_plugin(&PluginType::EQ);
        chain.add_plugin(&PluginType::Upmixer);
        assert_eq!(chain.len(), 2);

        let configs = chain.to_plugin_configs(48000.0);
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].plugin_type, "eq");
        assert_eq!(configs[1].plugin_type, "upmixer");
    }

    #[test]
    fn test_output_channels() {
        let mut chain = PluginChain::new();
        assert_eq!(chain.output_channels(), 2);

        // Add default upmixer (5.1 = 6 channels)
        chain.add_plugin(&PluginType::Upmixer);
        assert_eq!(chain.output_channels(), 6);

        // Test that speaker_config is correctly mapped
        let idx = 0;
        if let Some(plugin) = chain.get_plugin_mut(idx) {
            plugin.settings = PluginSettings::Upmixer {
                speaker_config: "7.1".to_string(),
                gain_front_direct: 1.0,
                gain_front_ambient: 0.5,
                gain_rear_ambient: 1.0,
                lfe_cutoff_hz: 120.0,
                stereo_width: 0.5,
                center_spread: 0.3,
                surround_direct_bleed: 0.15,
                rear_late_reflection: 0.2,
                bandpass_hz: 250.0,
                height_gain: 1.0,
                lfe_gain: 1.0,
                enable_subharmonic_synth: false,
                subharmonic_gain: 0.5,
                subharmonic_freq_hz: 56.0,
                subharmonic_attack_ms: 20.0,
                subharmonic_release_ms: 100.0,
                decorrelation_mode: 0,
                decorrelation_lfo_rate_hz: 0.3,
                velvet_noise_duration_ms: 30.0,
                velvet_noise_density: 2000.0,
                enable_hr_direct: false,
                hr_sharpen: 1.0,
                height_hf_cap_hz: 8000.0,
                height_transient_reduction: 0.3,
                height_direct_leak: 0.1,
                ambient_boost: 1.0,
                safety_cap_db: 3.0,
                rear_ambient_boost: 1.0,
                dialogue_weight: 0.5,
                voice_freq_min_hz: 300.0,
                voice_freq_max_hz: 3400.0,
                dialogue_centroid_weight: 0.3,
                dialogue_variance_weight: 0.2,
                dialogue_coherence_weight: 0.5,
                bypass_decorrelation: false,
                bypass_transient_detection: false,
                bypass_all_processing: false,
                enable_ml_detection: false,
            };
        }
        assert_eq!(chain.output_channels(), 8);
    }

    #[test]
    fn test_binaural_decoder_channel_update() {
        let mut chain = PluginChain::new();

        // Add upmixer (5.1 = 6 channels) and binaural decoder
        chain.add_plugin(&PluginType::Upmixer);
        chain.add_plugin(&PluginType::BinauralDecoder);

        // Initially, BinauralDecoder should have default 6 channels (from default_for)
        if let Some(plugin) = chain.get_plugin(1) {
            if let PluginSettings::BinauralDecoder { input_channels, .. } = plugin.settings {
                assert_eq!(input_channels, 6); // Default value
            }
        }

        // Update binaural decoder channels
        chain.update_channel_dependent_plugins();

        // Now it should be correctly set to 6 (output of upmixer)
        if let Some(plugin) = chain.get_plugin(1) {
            if let PluginSettings::BinauralDecoder { input_channels, .. } = plugin.settings {
                assert_eq!(input_channels, 6);
            }
        }

        // Change upmixer to 7.1 (8 channels)
        if let Some(plugin) = chain.get_plugin_mut(0) {
            plugin.settings = PluginSettings::Upmixer {
                speaker_config: "7.1".to_string(),
                gain_front_direct: 1.0,
                gain_front_ambient: 0.5,
                gain_rear_ambient: 1.0,
                lfe_cutoff_hz: 120.0,
                stereo_width: 0.5,
                center_spread: 0.3,
                surround_direct_bleed: 0.15,
                rear_late_reflection: 0.2,
                bandpass_hz: 250.0,
                height_gain: 1.0,
                lfe_gain: 1.0,
                enable_subharmonic_synth: false,
                subharmonic_gain: 0.5,
                subharmonic_freq_hz: 56.0,
                subharmonic_attack_ms: 20.0,
                subharmonic_release_ms: 100.0,
                decorrelation_mode: 0,
                decorrelation_lfo_rate_hz: 0.3,
                velvet_noise_duration_ms: 30.0,
                velvet_noise_density: 2000.0,
                enable_hr_direct: false,
                hr_sharpen: 1.0,
                height_hf_cap_hz: 8000.0,
                height_transient_reduction: 0.3,
                height_direct_leak: 0.1,
                ambient_boost: 1.0,
                safety_cap_db: 3.0,
                rear_ambient_boost: 1.0,
                dialogue_weight: 0.5,
                voice_freq_min_hz: 300.0,
                voice_freq_max_hz: 3400.0,
                dialogue_centroid_weight: 0.3,
                dialogue_variance_weight: 0.2,
                dialogue_coherence_weight: 0.5,
                bypass_decorrelation: false,
                bypass_transient_detection: false,
                bypass_all_processing: false,
                enable_ml_detection: false,
            };
        }

        // Update binaural decoder channels
        chain.update_channel_dependent_plugins();

        // Now BinauralDecoder should have 8 input channels
        if let Some(plugin) = chain.get_plugin(1) {
            if let PluginSettings::BinauralDecoder { input_channels, .. } = plugin.settings {
                assert_eq!(input_channels, 8);
            }
        }

        // Remove the upmixer
        chain.remove_plugin(0);
        chain.update_channel_dependent_plugins();

        // Now BinauralDecoder should have 2 input channels (stereo)
        if let Some(plugin) = chain.get_plugin(0) {
            if let PluginSettings::BinauralDecoder { input_channels, .. } = plugin.settings {
                assert_eq!(input_channels, 2);
            }
        }
    }

    #[test]
    fn test_default_rack_structure() {
        let chain = PluginChain::with_default_rack();
        assert_eq!(chain.len(), 4);

        // [InputLM, Gain(disabled), Matrix, OutputLM] - all permanent
        let plugins = chain.plugins();
        assert!(matches!(
            plugins[0].plugin_type(),
            PluginType::LoudnessMonitor
        ));
        assert!(matches!(plugins[1].plugin_type(), PluginType::Gain));
        assert!(!plugins[1].enabled); // ReplayGain starts disabled
        assert!(matches!(plugins[2].plugin_type(), PluginType::Matrix));
        assert!(matches!(
            plugins[3].plugin_type(),
            PluginType::LoudnessMonitor
        ));

        assert!(plugins[0].is_permanent());
        assert!(plugins[1].is_permanent());
        assert!(plugins[2].is_permanent());
        assert!(plugins[3].is_permanent());
    }

    #[test]
    fn test_is_input_output_monitor() {
        let chain = PluginChain::with_default_rack();

        // Index 0 = input monitor
        assert!(chain.is_input_monitor(0));
        assert!(!chain.is_output_monitor(0));

        // Index 1 = Gain (neither)
        assert!(!chain.is_input_monitor(1));
        assert!(!chain.is_output_monitor(1));

        // Index 2 = Matrix (neither)
        assert!(!chain.is_input_monitor(2));
        assert!(!chain.is_output_monitor(2));

        // Index 3 = output monitor
        assert!(!chain.is_input_monitor(3));
        assert!(chain.is_output_monitor(3));
    }

    #[test]
    fn test_default_rack_to_plugin_configs() {
        let chain = PluginChain::with_default_rack();
        let configs = chain.to_plugin_configs(48000.0);

        // Gain is disabled, so it's excluded from configs
        // Engine order: InputLM(0), Matrix(1), OutputLM(2)
        assert_eq!(configs.len(), 3);
        assert_eq!(configs[0].plugin_type, "loudness_monitor"); // input monitor
        assert_eq!(configs[1].plugin_type, "matrix"); // processing
        assert_eq!(configs[2].plugin_type, "loudness_monitor"); // output monitor
    }

    #[test]
    fn test_default_rack_get_engine_index() {
        let chain = PluginChain::with_default_rack();

        // UI index 0 (input LM) → engine index 0
        assert_eq!(chain.get_engine_index(0), Some(0));
        // UI index 1 (Gain, disabled) → None (not in engine)
        assert_eq!(chain.get_engine_index(1), None);
        // UI index 2 (Matrix) → engine index 1
        assert_eq!(chain.get_engine_index(2), Some(1));
        // UI index 3 (output LM) → engine index 2
        assert_eq!(chain.get_engine_index(3), Some(2));
    }

    #[test]
    fn test_default_rack_with_user_plugin() {
        let mut chain = PluginChain::with_default_rack();

        // Insert a user EQ plugin at the user insert point (before Matrix)
        let insert_idx = chain.user_plugin_insert_index();
        assert_eq!(insert_idx, 2); // Before Matrix (after InputLM and Gain)
        chain.insert_plugin(insert_idx, &PluginType::EQ);

        // Chain should be [InputLM, Gain(disabled), EQ, Matrix, OutputLM]
        assert_eq!(chain.len(), 5);
        assert!(matches!(
            chain.plugins()[0].plugin_type(),
            PluginType::LoudnessMonitor
        ));
        assert!(matches!(chain.plugins()[1].plugin_type(), PluginType::Gain));
        assert!(matches!(chain.plugins()[2].plugin_type(), PluginType::EQ));
        assert!(matches!(
            chain.plugins()[3].plugin_type(),
            PluginType::Matrix
        ));
        assert!(matches!(
            chain.plugins()[4].plugin_type(),
            PluginType::LoudnessMonitor
        ));

        // Monitor identification still correct
        assert!(chain.is_input_monitor(0));
        assert!(!chain.is_input_monitor(2));
        assert!(!chain.is_output_monitor(3));
        assert!(chain.is_output_monitor(4));

        // Gain is disabled, so not in engine configs
        // Engine indices: InputLM(0), EQ(1), Matrix(2), OutputLM(3)
        assert_eq!(chain.get_engine_index(0), Some(0)); // input monitor
        assert_eq!(chain.get_engine_index(1), None); // Gain (disabled)
        assert_eq!(chain.get_engine_index(2), Some(1)); // EQ (processing)
        assert_eq!(chain.get_engine_index(3), Some(2)); // Matrix (processing)
        assert_eq!(chain.get_engine_index(4), Some(3)); // output monitor

        // to_plugin_configs order: InputLM, EQ, Matrix, OutputLM (Gain excluded)
        let configs = chain.to_plugin_configs(48000.0);
        assert_eq!(configs.len(), 4);
        assert_eq!(configs[0].plugin_type, "loudness_monitor");
        assert_eq!(configs[1].plugin_type, "eq");
        assert_eq!(configs[2].plugin_type, "matrix");
        assert_eq!(configs[3].plugin_type, "loudness_monitor");
    }

    #[test]
    fn test_single_loudness_monitor_not_output() {
        // A chain with only one permanent LoudnessMonitor should not be an output monitor
        let mut chain = PluginChain::new();
        chain.add_permanent_plugin(&PluginType::LoudnessMonitor);

        assert!(chain.is_input_monitor(0));
        assert!(!chain.is_output_monitor(0));
    }

    #[test]
    fn test_matrix_preset_roundtrip() {
        let presets = ["Identity", "Swap L/R", "Mono Mix"];
        // Test 2x2 (all presets should work)
        for preset in &presets {
            let mut matrix = vec![0.0f32; 4];
            apply_matrix_preset(2, 2, &mut matrix, preset);
            let detected = detect_matrix_preset(2, 2, &matrix);
            assert_eq!(detected, *preset, "2x2 roundtrip failed for {}", preset);
        }
        // Test non-square: 5x2, 2x5
        for (in_ch, out_ch) in [(5, 2), (2, 5), (1, 1), (8, 8)] {
            let mut matrix = vec![0.0f32; in_ch * out_ch];
            apply_matrix_preset(in_ch, out_ch, &mut matrix, "Identity");
            let detected = detect_matrix_preset(in_ch, out_ch, &matrix);
            assert_eq!(
                detected, "Identity",
                "{}x{} identity roundtrip failed",
                in_ch, out_ch
            );
        }
    }

    #[test]
    fn test_matrix_preset_cycling() {
        // Simulate the TUI cycling logic using available_matrix_presets
        for (in_ch, out_ch) in [(2, 2), (3, 3), (5, 2), (2, 5), (1, 1)] {
            let presets = available_matrix_presets(in_ch, out_ch);
            let mut matrix = vec![0.0f32; in_ch * out_ch];
            apply_matrix_preset(in_ch, out_ch, &mut matrix, "Identity");

            // Cycle forward through all presets twice
            let mut seen = Vec::new();
            for _ in 0..presets.len() * 2 {
                let current = detect_matrix_preset(in_ch, out_ch, &matrix);
                seen.push(current.to_string());
                let current_idx = presets.iter().position(|&p| p == current).unwrap_or(0);
                let new_idx = (current_idx + 1) % presets.len();
                apply_matrix_preset(in_ch, out_ch, &mut matrix, presets[new_idx]);
            }

            // Every available preset should be reachable
            for preset in &presets {
                assert!(
                    seen.contains(&preset.to_string()),
                    "{} not reachable for {}x{}, cycle: {:?}",
                    preset,
                    in_ch,
                    out_ch,
                    seen
                );
            }
            // No "Custom" should appear (all valid presets should round-trip)
            assert!(
                !seen.contains(&"Custom".to_string()),
                "Custom appeared in cycle for {}x{}: {:?}",
                in_ch,
                out_ch,
                seen
            );
        }
    }

    // ========================================================================
    // Channel flow tests: output_channels_for_input & adapt_matrix_to_input
    // ========================================================================

    /// Helper: build a chain and set the upmixer's speaker_config.
    fn chain_with_upmixer(speaker_config: &str) -> PluginChain {
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::Upmixer);
        if let Some(p) = chain.get_plugin_mut(0) {
            if let PluginSettings::Upmixer {
                speaker_config: sc, ..
            } = &mut p.settings
            {
                *sc = speaker_config.to_string();
            }
        }
        chain
    }

    // -- output_channels_for_input -----------------------------------------

    #[test]
    fn test_output_channels_passthrough() {
        // Empty chain: input passes through unchanged
        let chain = PluginChain::new();
        assert_eq!(chain.output_channels_for_input(1), 1);
        assert_eq!(chain.output_channels_for_input(2), 2);
        assert_eq!(chain.output_channels_for_input(8), 8);
    }

    #[test]
    fn test_output_channels_non_channel_plugins_passthrough() {
        // Plugins that don't change channels should pass through
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::EQ);
        chain.add_plugin(&PluginType::Gain);
        assert_eq!(chain.output_channels_for_input(2), 2);
        assert_eq!(chain.output_channels_for_input(6), 6);
    }

    #[test]
    fn test_output_channels_upmixer_configs() {
        for (config, expected) in [
            ("2.0", 2),
            ("5.0", 5),
            ("5.1", 6),
            ("7.1", 8),
            ("5.1.2", 8),
            ("5.1.4", 10),
            ("7.1.2", 10),
            ("7.1.4", 12),
            ("9.1.4", 14),
            ("9.1.6", 16),
        ] {
            let chain = chain_with_upmixer(config);
            assert_eq!(
                chain.output_channels_for_input(2),
                expected,
                "upmixer {} should output {} channels",
                config,
                expected
            );
        }
    }

    #[test]
    fn test_output_channels_downmix() {
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::Downmix);
        assert_eq!(chain.output_channels_for_input(6), 2);
        assert_eq!(chain.output_channels_for_input(10), 2);
    }

    #[test]
    fn test_output_channels_mono_to_stereo() {
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::MonoToStereo);
        assert_eq!(chain.output_channels_for_input(1), 2);
    }

    #[test]
    fn test_output_channels_binaural_decoder() {
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::BinauralDecoder);
        assert_eq!(chain.output_channels_for_input(6), 2);
        assert_eq!(chain.output_channels_for_input(10), 2);
    }

    #[test]
    fn test_output_channels_matrix() {
        // Matrix with custom output size
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::Matrix);
        if let Some(p) = chain.get_plugin_mut(0) {
            if let PluginSettings::Matrix {
                input_channels,
                output_channels,
                matrix,
                channel_states,
            } = &mut p.settings
            {
                resize_matrix(matrix, *input_channels, *output_channels, 6, 4);
                *input_channels = 6;
                *output_channels = 4;
                channel_states.resize(4, sotf_plugins::ChannelState::default());
            }
        }
        assert_eq!(chain.output_channels_for_input(6), 4);
    }

    #[test]
    fn test_output_channels_upmixer_then_binaural() {
        // Last channel-changing plugin wins (reverse walk)
        let mut chain = chain_with_upmixer("5.1.4");
        chain.add_plugin(&PluginType::BinauralDecoder);
        // Binaural is last → output is 2
        assert_eq!(chain.output_channels_for_input(2), 2);
    }

    #[test]
    fn test_output_channels_upmixer_then_downmix() {
        let mut chain = chain_with_upmixer("7.1");
        chain.add_plugin(&PluginType::Downmix);
        assert_eq!(chain.output_channels_for_input(2), 2);
    }

    #[test]
    fn test_output_channels_disabled_plugin_skipped() {
        let mut chain = chain_with_upmixer("5.1");
        // Disable the upmixer → passthrough
        if let Some(p) = chain.get_plugin_mut(0) {
            p.enabled = false;
        }
        assert_eq!(chain.output_channels_for_input(2), 2);
    }

    #[test]
    fn test_output_channels_eq_after_upmixer() {
        // EQ doesn't change channels → upmixer still determines output
        let mut chain = chain_with_upmixer("5.1");
        chain.add_plugin(&PluginType::EQ);
        assert_eq!(chain.output_channels_for_input(2), 6);
    }

    // -- adapt_matrix_to_input ---------------------------------------------

    fn get_matrix_dims(chain: &PluginChain) -> Option<(usize, usize)> {
        for p in chain.plugins() {
            if let PluginSettings::Matrix {
                input_channels,
                output_channels,
                ..
            } = &p.settings
            {
                return Some((*input_channels, *output_channels));
            }
        }
        None
    }

    #[test]
    fn test_adapt_matrix_stereo_file_no_upmixer() {
        // Matrix alone with stereo input stays 2x2
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(2);
        assert_eq!(get_matrix_dims(&chain), Some((2, 2)));
    }

    #[test]
    fn test_adapt_matrix_multichannel_file_no_upmixer() {
        // 6-channel file → matrix adapts to 6x6
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(6);
        assert_eq!(get_matrix_dims(&chain), Some((6, 6)));
    }

    #[test]
    fn test_adapt_matrix_upmixer_before_matrix() {
        // Stereo file, upmixer 5.1.4 (10ch) before matrix → matrix should be 10x10
        let mut chain = chain_with_upmixer("5.1.4");
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(2);
        assert_eq!(get_matrix_dims(&chain), Some((10, 10)));
    }

    #[test]
    fn test_adapt_matrix_upmixer_various_configs() {
        for (config, expected) in [
            ("5.1", 6),
            ("7.1", 8),
            ("5.1.4", 10),
            ("7.1.4", 12),
            ("9.1.6", 16),
        ] {
            let mut chain = chain_with_upmixer(config);
            chain.add_plugin(&PluginType::Matrix);
            chain.adapt_matrix_to_input(2);
            assert_eq!(
                get_matrix_dims(&chain),
                Some((expected, expected)),
                "upmixer {} → matrix should be {}x{}",
                config,
                expected,
                expected
            );
        }
    }

    #[test]
    fn test_adapt_matrix_downmix_before_matrix() {
        // Downmix before matrix → matrix gets 2x2 regardless of file channels
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::Downmix);
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(6);
        assert_eq!(get_matrix_dims(&chain), Some((2, 2)));
    }

    #[test]
    fn test_adapt_matrix_mono_to_stereo_before_matrix() {
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::MonoToStereo);
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(1);
        assert_eq!(get_matrix_dims(&chain), Some((2, 2)));
    }

    #[test]
    fn test_adapt_matrix_binaural_before_matrix() {
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::BinauralDecoder);
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(6);
        assert_eq!(get_matrix_dims(&chain), Some((2, 2)));
    }

    #[test]
    fn test_adapt_matrix_upmixer_then_binaural_then_matrix() {
        // Chain: upmixer(5.1.4=10ch) → binaural(→2ch) → matrix
        // Matrix should see 2 channels (binaural is last before it)
        let mut chain = chain_with_upmixer("5.1.4");
        chain.add_plugin(&PluginType::BinauralDecoder);
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(2);
        assert_eq!(get_matrix_dims(&chain), Some((2, 2)));
    }

    #[test]
    fn test_adapt_matrix_disabled_upmixer_ignored() {
        // Disabled upmixer should be skipped → matrix uses file channels
        let mut chain = chain_with_upmixer("5.1.4");
        if let Some(p) = chain.get_plugin_mut(0) {
            p.enabled = false;
        }
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(2);
        assert_eq!(get_matrix_dims(&chain), Some((2, 2)));
    }

    #[test]
    fn test_adapt_matrix_eq_between_upmixer_and_matrix() {
        // EQ doesn't change channels → upmixer output carries through
        let mut chain = chain_with_upmixer("7.1");
        chain.add_plugin(&PluginType::EQ);
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(2);
        assert_eq!(get_matrix_dims(&chain), Some((8, 8)));
    }

    #[test]
    fn test_adapt_matrix_noop_when_already_correct() {
        // If matrix already matches, nothing should change
        let mut chain = chain_with_upmixer("5.1");
        chain.add_plugin(&PluginType::Matrix);
        // First adapt: 2x2 → 6x6
        chain.adapt_matrix_to_input(2);
        assert_eq!(get_matrix_dims(&chain), Some((6, 6)));
        // Second adapt: already 6x6 → no change
        chain.adapt_matrix_to_input(2);
        assert_eq!(get_matrix_dims(&chain), Some((6, 6)));
    }

    #[test]
    fn test_adapt_matrix_readapt_on_config_change() {
        // Simulate changing upmixer config and re-adapting
        let mut chain = chain_with_upmixer("5.1");
        chain.add_plugin(&PluginType::Matrix);
        chain.adapt_matrix_to_input(2);
        assert_eq!(get_matrix_dims(&chain), Some((6, 6)));

        // Change upmixer to 7.1.4
        if let Some(p) = chain.get_plugin_mut(0) {
            if let PluginSettings::Upmixer { speaker_config, .. } = &mut p.settings {
                *speaker_config = "7.1.4".to_string();
            }
        }
        chain.adapt_matrix_to_input(2);
        assert_eq!(get_matrix_dims(&chain), Some((12, 12)));
    }

    // -- update_channel_dependent_plugins ----------------------------------

    #[test]
    fn test_update_channels_upmixer_then_eq() {
        let mut chain = chain_with_upmixer("5.1.4");
        chain.add_plugin(&PluginType::EQ);
        chain.update_channel_dependent_plugins();

        if let Some(p) = chain.get_plugin(1) {
            if let PluginSettings::EQ { channels, .. } = &p.settings {
                assert_eq!(
                    *channels, 10,
                    "EQ after 5.1.4 upmixer should have 10 channels"
                );
            } else {
                panic!("expected EQ");
            }
        }
    }

    #[test]
    fn test_update_channels_upmixer_then_gain() {
        let mut chain = chain_with_upmixer("7.1");
        chain.add_plugin(&PluginType::Gain);
        chain.update_channel_dependent_plugins();

        if let Some(p) = chain.get_plugin(1) {
            if let PluginSettings::Gain { channels, .. } = &p.settings {
                assert_eq!(
                    *channels, 8,
                    "Gain after 7.1 upmixer should have 8 channels"
                );
            } else {
                panic!("expected Gain");
            }
        }
    }

    #[test]
    fn test_update_channels_bandsplit_doubles() {
        // BandSplit doubles the channel count
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::BandSplit);
        chain.update_channel_dependent_plugins();

        // Default input is 2, split → 4 output channels
        // Check via output_channels_for_input (BandSplit isn't in that fn,
        // but update_channel_dependent_plugins tracks it)
        // Instead check that a Gain after the split gets the doubled count
        chain.add_plugin(&PluginType::Gain);
        chain.update_channel_dependent_plugins();
        if let Some(p) = chain.get_plugin(1) {
            if let PluginSettings::Gain { channels, .. } = &p.settings {
                assert_eq!(
                    *channels, 4,
                    "Gain after BandSplit(2ch) should have 4 channels"
                );
            } else {
                panic!("expected Gain");
            }
        }
    }

    #[test]
    fn test_update_channels_bandsplit_then_bandmerge() {
        // Split doubles, merge halves → back to original
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::BandSplit);
        chain.add_plugin(&PluginType::BandMerge);
        chain.add_plugin(&PluginType::Gain);
        chain.update_channel_dependent_plugins();

        if let Some(p) = chain.get_plugin(2) {
            if let PluginSettings::Gain { channels, .. } = &p.settings {
                assert_eq!(*channels, 2, "Gain after Split+Merge should be back to 2");
            } else {
                panic!("expected Gain");
            }
        }
    }

    #[test]
    fn test_update_channels_upmixer_split_merge_gain() {
        // Upmixer(5.1=6) → Split(→12) → Merge(→6) → Gain(6)
        let mut chain = chain_with_upmixer("5.1");
        chain.add_plugin(&PluginType::BandSplit);
        chain.add_plugin(&PluginType::BandMerge);
        chain.add_plugin(&PluginType::Gain);
        chain.update_channel_dependent_plugins();

        // BandSplit should have 6 channels
        if let Some(p) = chain.get_plugin(1) {
            if let PluginSettings::BandSplit { channels, .. } = &p.settings {
                assert_eq!(*channels, 6, "BandSplit after 5.1 upmixer");
            } else {
                panic!("expected BandSplit");
            }
        }
        // BandMerge should have 12 channels (doubled by split)
        if let Some(p) = chain.get_plugin(2) {
            if let PluginSettings::BandMerge { channels, .. } = &p.settings {
                assert_eq!(*channels, 12, "BandMerge after BandSplit(6ch)");
            } else {
                panic!("expected BandMerge");
            }
        }
        // Gain should be back to 6
        if let Some(p) = chain.get_plugin(3) {
            if let PluginSettings::Gain { channels, .. } = &p.settings {
                assert_eq!(*channels, 6, "Gain after Split+Merge should be 6");
            } else {
                panic!("expected Gain");
            }
        }
    }

    #[test]
    fn test_update_channels_downmix_then_eq() {
        // Downmix → EQ: EQ should have 2 channels
        let mut chain = chain_with_upmixer("7.1");
        chain.add_plugin(&PluginType::Downmix);
        chain.add_plugin(&PluginType::EQ);
        chain.update_channel_dependent_plugins();

        // Downmix input should be set to 8
        if let Some(p) = chain.get_plugin(1) {
            if let PluginSettings::Downmix { input_channels, .. } = &p.settings {
                assert_eq!(*input_channels, 8, "Downmix input after 7.1 upmixer");
            } else {
                panic!("expected Downmix");
            }
        }
        // EQ after downmix should be 2
        if let Some(p) = chain.get_plugin(2) {
            if let PluginSettings::EQ { channels, .. } = &p.settings {
                assert_eq!(*channels, 2, "EQ after Downmix should be 2");
            } else {
                panic!("expected EQ");
            }
        }
    }

    #[test]
    fn test_update_channels_mono_to_stereo_then_gain() {
        let mut chain = PluginChain::new();
        chain.add_plugin(&PluginType::MonoToStereo);
        chain.add_plugin(&PluginType::Gain);
        chain.update_channel_dependent_plugins();

        if let Some(p) = chain.get_plugin(1) {
            if let PluginSettings::Gain { channels, .. } = &p.settings {
                assert_eq!(*channels, 2, "Gain after MonoToStereo");
            } else {
                panic!("expected Gain");
            }
        }
    }
}
