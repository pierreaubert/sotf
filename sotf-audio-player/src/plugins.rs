use autoeq_iir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sotf_audio::engine::PluginConfig;

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
    BinauralDecoder,
    Convolution,
    LoudnessMonitor,
    SpectrumAnalyzer,
    ChannelMuteSolo,
    Matrix,
    XTC,
    Denoiser,
}

impl PluginType {
    pub fn name(&self) -> &str {
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
            Self::BinauralDecoder => "Binaural Decoder",
            Self::Convolution => "Convolution",
            Self::LoudnessMonitor => "Loudness Monitor",
            Self::SpectrumAnalyzer => "Spectrum Analyzer",
            Self::ChannelMuteSolo => "Channel Mute/Solo",
            Self::Matrix => "Matrix Mixer",
            Self::XTC => "Crosstalk Cancellation",
            Self::Denoiser => "Denoiser",
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
            Self::BinauralDecoder => "Multi-channel to Binaural (HRTF)",
            Self::Convolution => "FFT-based Convolution (IR Processing)",
            Self::LoudnessMonitor => "Real-time EBU R128 loudness monitoring",
            Self::SpectrumAnalyzer => "Real-time frequency spectrum analysis",
            Self::ChannelMuteSolo => "Mute or solo individual channels",
            Self::Matrix => "Channel routing and mixing matrix",
            Self::XTC => "Crosstalk cancellation for speaker playback",
            Self::Denoiser => "Wiener filter denoiser with MCRA noise estimation",
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
            Self::BinauralDecoder,
            Self::Convolution,
            Self::LoudnessMonitor,
            Self::SpectrumAnalyzer,
            Self::ChannelMuteSolo,
            Self::Matrix,
            Self::XTC,
            Self::Denoiser,
        ]
    }

    /// Returns true if this is a monitoring/analyzer plugin (non-processing)
    pub fn is_monitoring(&self) -> bool {
        matches!(
            self,
            Self::LoudnessMonitor | Self::SpectrumAnalyzer | Self::ChannelMuteSolo
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EQFilter {
    pub filter_type: BiquadFilterType,
    pub frequency: f64,
    pub q: f64,
    pub gain_db: f64,
}

impl EQFilter {
    pub fn new(filter_type: BiquadFilterType, frequency: f64, q: f64, gain_db: f64) -> Self {
        Self {
            filter_type,
            frequency,
            q,
            gain_db,
        }
    }

    pub fn to_biquad(&self, sample_rate: f64) -> Biquad {
        Biquad::new(
            self.filter_type,
            sample_rate,
            self.frequency,
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

// Import plugin defaults from sotf-audio-plugins for consistent preset migration
use sotf_plugins::{
    binaural_default_enable_optimization, compressor_default_link_channels,
    compressor_default_sidechain_hpf_hz, upmixer_default_hr_sharpen, upmixer_default_safety_cap_db,
    upmixer_default_subharmonic_gain,
};

// Import param_specs for new upmixer defaults
use sotf_plugins::param_specs::upmixer as upmixer_specs;

// Import param_specs for dynamics plugins
use sotf_plugins::param_specs::expander as expander_specs;
use sotf_plugins::param_specs::multiband_compressor as mb_compressor_specs;
use sotf_plugins::param_specs::multiband_expander as mb_expander_specs;

// Wrapper functions to convert f32 -> f64 for PluginSettings (which uses f64)
fn default_upmixer_subharmonic_gain() -> f64 {
    upmixer_default_subharmonic_gain() as f64
}

fn default_upmixer_hr_sharpen() -> f64 {
    upmixer_default_hr_sharpen() as f64
}

fn default_upmixer_safety_cap_db() -> f64 {
    upmixer_default_safety_cap_db() as f64
}

// New upmixer parameter defaults
fn default_upmixer_center_spread() -> f64 {
    upmixer_specs::CENTER_SPREAD_DEFAULT as f64
}

fn default_upmixer_surround_direct_bleed() -> f64 {
    upmixer_specs::SURROUND_DIRECT_BLEED_DEFAULT as f64
}

fn default_upmixer_rear_late_reflection() -> f64 {
    upmixer_specs::REAR_LATE_REFLECTION_DEFAULT as f64
}

fn default_upmixer_subharmonic_freq_hz() -> f64 {
    upmixer_specs::SUBHARMONIC_FREQ_HZ_DEFAULT as f64
}

fn default_upmixer_subharmonic_attack_ms() -> f64 {
    upmixer_specs::SUBHARMONIC_ATTACK_MS_DEFAULT as f64
}

fn default_upmixer_subharmonic_release_ms() -> f64 {
    upmixer_specs::SUBHARMONIC_RELEASE_MS_DEFAULT as f64
}

fn default_upmixer_decorrelation_lfo_rate_hz() -> f64 {
    upmixer_specs::DECORRELATION_LFO_RATE_HZ_DEFAULT as f64
}

fn default_upmixer_velvet_noise_duration_ms() -> f64 {
    upmixer_specs::VELVET_NOISE_DURATION_MS_DEFAULT as f64
}

fn default_upmixer_velvet_noise_density() -> f64 {
    upmixer_specs::VELVET_NOISE_DENSITY_DEFAULT as f64
}

fn default_upmixer_height_hf_cap_hz() -> f64 {
    upmixer_specs::HEIGHT_HF_CAP_HZ_DEFAULT as f64
}

fn default_upmixer_height_transient_reduction() -> f64 {
    upmixer_specs::HEIGHT_TRANSIENT_REDUCTION_DEFAULT as f64
}

fn default_upmixer_height_direct_leak() -> f64 {
    upmixer_specs::HEIGHT_DIRECT_LEAK_DEFAULT as f64
}

fn default_upmixer_rear_ambient_boost() -> f64 {
    upmixer_specs::REAR_AMBIENT_BOOST_DEFAULT as f64
}

fn default_upmixer_ambient_boost() -> f64 {
    upmixer_specs::AMBIENT_BOOST_DEFAULT as f64
}

fn default_upmixer_dialogue_weight() -> f64 {
    upmixer_specs::DIALOGUE_WEIGHT_DEFAULT as f64
}

fn default_upmixer_voice_freq_min_hz() -> f64 {
    upmixer_specs::VOICE_FREQ_MIN_HZ_DEFAULT as f64
}

fn default_upmixer_voice_freq_max_hz() -> f64 {
    upmixer_specs::VOICE_FREQ_MAX_HZ_DEFAULT as f64
}

fn default_compressor_link_channels() -> bool {
    compressor_default_link_channels()
}

fn default_compressor_sidechain_hpf_hz() -> f64 {
    compressor_default_sidechain_hpf_hz() as f64
}

fn default_binaural_enable_optimization() -> bool {
    binaural_default_enable_optimization()
}

// Gate/Limiter defaults (defined locally as they use f64 and match engine defaults)
fn default_limiter_mix() -> f64 {
    1.0 // Match plugin_limiter default
}

fn default_gate_mix() -> f64 {
    1.0 // Match plugin_gate default
}

fn default_gate_link_channels() -> bool {
    true
}

fn default_gate_sidechain_hpf_hz() -> f64 {
    0.0
}

// Expander defaults
fn default_expander_threshold_db() -> f64 {
    expander_specs::THRESHOLD_DEFAULT as f64
}

fn default_expander_ratio() -> f64 {
    expander_specs::RATIO_DEFAULT as f64
}

fn default_expander_attack_ms() -> f64 {
    expander_specs::ATTACK_DEFAULT as f64
}

fn default_expander_release_ms() -> f64 {
    expander_specs::RELEASE_DEFAULT as f64
}

fn default_expander_range_db() -> f64 {
    expander_specs::RANGE_DEFAULT as f64
}

fn default_expander_knee_db() -> f64 {
    expander_specs::KNEE_DEFAULT as f64
}

fn default_expander_hysteresis_db() -> f64 {
    expander_specs::HYSTERESIS_DEFAULT as f64
}

fn default_expander_hold_ms() -> f64 {
    expander_specs::HOLD_DEFAULT as f64
}

fn default_expander_mix() -> f64 {
    expander_specs::MIX_DEFAULT as f64
}

fn default_expander_link_channels() -> bool {
    expander_specs::LINK_CHANNELS_DEFAULT
}

fn default_expander_sidechain_hpf_hz() -> f64 {
    expander_specs::SIDECHAIN_HPF_HZ_DEFAULT as f64
}

// Multiband Compressor defaults
fn default_mb_compressor_num_bands() -> usize {
    mb_compressor_specs::NUM_BANDS_DEFAULT
}

fn default_mb_compressor_crossover_preset() -> i32 {
    mb_compressor_specs::CROSSOVER_PRESET_DEFAULT
}

fn default_mb_compressor_crossover_freq_1() -> f64 {
    mb_compressor_specs::CROSSOVER_FREQ_1_DEFAULT as f64
}

fn default_mb_compressor_crossover_freq_2() -> f64 {
    mb_compressor_specs::CROSSOVER_FREQ_2_DEFAULT as f64
}

fn default_mb_compressor_crossover_freq_3() -> f64 {
    mb_compressor_specs::CROSSOVER_FREQ_3_DEFAULT as f64
}

fn default_mb_compressor_crossover_freq_4() -> f64 {
    mb_compressor_specs::CROSSOVER_FREQ_4_DEFAULT as f64
}

fn default_mb_compressor_threshold_db() -> f64 {
    mb_compressor_specs::THRESHOLD_DEFAULT as f64
}

fn default_mb_compressor_ratio() -> f64 {
    mb_compressor_specs::RATIO_DEFAULT as f64
}

fn default_mb_compressor_attack_ms() -> f64 {
    mb_compressor_specs::ATTACK_DEFAULT as f64
}

fn default_mb_compressor_release_ms() -> f64 {
    mb_compressor_specs::RELEASE_DEFAULT as f64
}

fn default_mb_compressor_knee_db() -> f64 {
    mb_compressor_specs::KNEE_DEFAULT as f64
}

fn default_mb_compressor_mix() -> f64 {
    mb_compressor_specs::MIX_DEFAULT as f64
}

fn default_mb_compressor_link_channels() -> bool {
    mb_compressor_specs::LINK_CHANNELS_DEFAULT
}

// Multiband Expander defaults
fn default_mb_expander_num_bands() -> usize {
    mb_expander_specs::NUM_BANDS_DEFAULT
}

fn default_mb_expander_crossover_preset() -> i32 {
    mb_expander_specs::CROSSOVER_PRESET_DEFAULT
}

fn default_mb_expander_crossover_freq_1() -> f64 {
    mb_expander_specs::CROSSOVER_FREQ_1_DEFAULT as f64
}

fn default_mb_expander_crossover_freq_2() -> f64 {
    mb_expander_specs::CROSSOVER_FREQ_2_DEFAULT as f64
}

fn default_mb_expander_crossover_freq_3() -> f64 {
    mb_expander_specs::CROSSOVER_FREQ_3_DEFAULT as f64
}

fn default_mb_expander_crossover_freq_4() -> f64 {
    mb_expander_specs::CROSSOVER_FREQ_4_DEFAULT as f64
}

fn default_mb_expander_threshold_db() -> f64 {
    mb_expander_specs::THRESHOLD_DEFAULT as f64
}

fn default_mb_expander_ratio() -> f64 {
    mb_expander_specs::RATIO_DEFAULT as f64
}

fn default_mb_expander_attack_ms() -> f64 {
    mb_expander_specs::ATTACK_DEFAULT as f64
}

fn default_mb_expander_release_ms() -> f64 {
    mb_expander_specs::RELEASE_DEFAULT as f64
}

fn default_mb_expander_range_db() -> f64 {
    mb_expander_specs::RANGE_DEFAULT as f64
}

fn default_mb_expander_knee_db() -> f64 {
    mb_expander_specs::KNEE_DEFAULT as f64
}

fn default_mb_expander_hysteresis_db() -> f64 {
    mb_expander_specs::HYSTERESIS_DEFAULT as f64
}

fn default_mb_expander_hold_ms() -> f64 {
    mb_expander_specs::HOLD_DEFAULT as f64
}

fn default_mb_expander_mix() -> f64 {
    mb_expander_specs::MIX_DEFAULT as f64
}

fn default_mb_expander_link_channels() -> bool {
    mb_expander_specs::LINK_CHANNELS_DEFAULT
}

// SpectrumAnalyzer defaults
fn default_spectrum_num_bins() -> usize {
    512
}

fn default_spectrum_min_freq() -> f32 {
    20.0
}

fn default_spectrum_max_freq() -> f32 {
    20000.0
}

fn default_spectrum_smoothing() -> f32 {
    0.8
}

// XTC defaults
fn default_xtc_distance_m() -> f64 {
    2.0
}
fn default_xtc_speaker_angle_deg() -> f64 {
    30.0
}
fn default_xtc_head_radius_m() -> f64 {
    0.0875
}
fn default_xtc_beta_base() -> f64 {
    0.001
}
fn default_xtc_beta_low_freq_boost() -> f64 {
    10.0
}
fn default_xtc_beta_high_freq_boost() -> f64 {
    10.0
}
fn default_xtc_head_shadow_cutoff_hz() -> f64 {
    4000.0
}
fn default_xtc_head_shadow_slope() -> f64 {
    6.0
}

// Denoiser defaults
fn default_denoiser_reduction_db() -> f64 {
    12.0
}
fn default_denoiser_floor_db() -> f64 {
    -30.0
}
fn default_denoiser_smoothing() -> f64 {
    0.8
}
fn default_denoiser_attack_ms() -> f64 {
    5.0
}
fn default_denoiser_release_ms() -> f64 {
    50.0
}
fn default_denoiser_low_latency() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginSettings {
    EQ {
        filters: Vec<EQFilter>,
    },
    Gain {
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
        #[serde(default)] // false
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
        // Diagnostic bypass parameters
        #[serde(default)] // false
        bypass_decorrelation: bool,
        #[serde(default)] // false
        bypass_transient_detection: bool,
        #[serde(default)] // false
        bypass_all_processing: bool,
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
        #[serde(default = "default_limiter_mix")]
        mix: f64,
    },
    Gate {
        threshold_db: f64,
        ratio: f64,
        attack_ms: f64,
        release_ms: f64,
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
    },
    LoudnessCompensation {
        low_freq: f64,
        low_gain: f64,
        high_freq: f64,
        high_gain: f64,
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
    },
    ChannelMuteSolo {
        enabled: bool,
        channel_states: Vec<sotf_plugins::ChannelState>,
    },
    Matrix {
        input_channels: usize,
        output_channels: usize,
        matrix: Vec<f32>, // Row-major: matrix[out * in_count + in] = linear_gain
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
            Self::BinauralDecoder { .. } => PluginType::BinauralDecoder,
            Self::Convolution { .. } => PluginType::Convolution,
            Self::LoudnessMonitor => PluginType::LoudnessMonitor,
            Self::SpectrumAnalyzer { .. } => PluginType::SpectrumAnalyzer,
            Self::ChannelMuteSolo { .. } => PluginType::ChannelMuteSolo,
            Self::Matrix { .. } => PluginType::Matrix,
            Self::XTC { .. } => PluginType::XTC,
            Self::Denoiser { .. } => PluginType::Denoiser,
        }
    }

    pub fn to_plugin_config(&self, sample_rate: f64) -> PluginConfig {
        match self {
            Self::EQ { filters } => {
                let filter_configs: Vec<_> = filters
                    .iter()
                    .map(|f| {
                        let bq = f.to_biquad(sample_rate);
                        json!({
                            "filter_type": bq.filter_type.long_name().to_lowercase(),
                            "freq": bq.freq,
                            "q": bq.q,
                            "db_gain": bq.db_gain,
                        })
                    })
                    .collect();

                PluginConfig::new(
                    "eq",
                    json!({
                        "filters": filter_configs,
                    }),
                )
            }
            Self::Gain { gain_db } => PluginConfig::new(
                "gain",
                json!({
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
                bypass_decorrelation,
                bypass_transient_detection,
                bypass_all_processing,
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
                    "bypass_decorrelation": bypass_decorrelation,
                    "bypass_transient_detection": bypass_transient_detection,
                    "bypass_all_processing": bypass_all_processing,
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
                mix,
            } => PluginConfig::new(
                "limiter",
                json!({
                    "threshold_db": threshold_db,
                    "release_ms": release_ms,
                    "mix": mix,
                }),
            ),
            Self::Gate {
                threshold_db,
                ratio,
                attack_ms,
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
            } => PluginConfig::new(
                "multiband_compressor",
                json!({
                    "num_bands": num_bands,
                    "crossover_preset": crossover_preset,
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
            } => PluginConfig::new(
                "multiband_expander",
                json!({
                    "num_bands": num_bands,
                    "crossover_preset": crossover_preset,
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
                }),
            ),
            Self::LoudnessCompensation {
                low_freq,
                low_gain,
                high_freq,
                high_gain,
            } => PluginConfig::new(
                "loudness_compensation",
                json!({
                    "low_freq": low_freq,
                    "low_gain": low_gain,
                    "high_freq": high_freq,
                    "high_gain": high_gain,
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
            } => PluginConfig::new(
                "spectrum_analyzer",
                json!({
                    "num_bins": num_bins,
                    "min_freq": min_freq,
                    "max_freq": max_freq,
                    "smoothing": smoothing,
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
            } => PluginConfig::new(
                "matrix",
                json!({
                    "input_channels": input_channels,
                    "output_channels": output_channels,
                    "matrix": matrix,
                }),
            ),
            Self::XTC {
                distance_m,
                speaker_angle_deg,
                head_radius_m,
                beta_base,
                beta_low_freq_boost,
                beta_high_freq_boost,
                head_shadow_cutoff_hz,
                head_shadow_slope_db_per_octave,
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
                }),
            ),
            Self::Denoiser {
                reduction_db,
                floor_db,
                smoothing,
                attack_ms,
                release_ms,
                low_latency,
            } => PluginConfig::new(
                "denoiser",
                json!({
                    "reduction_db": reduction_db,
                    "floor_db": floor_db,
                    "smoothing": smoothing,
                    "attack_ms": attack_ms,
                    "release_ms": release_ms,
                    "low_latency": low_latency,
                }),
            ),
        }
    }

    /// Create default settings for a plugin type
    pub fn default_for(plugin_type: &PluginType) -> Self {
        match plugin_type {
            PluginType::EQ => Self::EQ {
                filters: vec![
                    // Default: 10-band flat EQ
                    EQFilter::new(BiquadFilterType::Peak, 32.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 64.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 125.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 250.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 500.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 2000.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 4000.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 8000.0, 1.4, 0.0),
                    EQFilter::new(BiquadFilterType::Peak, 16000.0, 1.4, 0.0),
                ],
            },
            PluginType::Gain => Self::Gain { gain_db: 0.0 },
            PluginType::Upmixer => Self::Upmixer {
                speaker_config: "5.1".to_string(),
                // Gains
                gain_front_direct: upmixer_specs::GAIN_FRONT_DIRECT_DEFAULT as f64,
                gain_front_ambient: upmixer_specs::GAIN_FRONT_AMBIENT_DEFAULT as f64,
                gain_rear_ambient: upmixer_specs::GAIN_REAR_AMBIENT_DEFAULT as f64,
                height_gain: upmixer_specs::GAIN_HEIGHT_DEFAULT as f64,
                stereo_width: upmixer_specs::STEREO_WIDTH_DEFAULT as f64,
                center_spread: upmixer_specs::CENTER_SPREAD_DEFAULT as f64,
                surround_direct_bleed: upmixer_specs::SURROUND_DIRECT_BLEED_DEFAULT as f64,
                rear_late_reflection: upmixer_specs::REAR_LATE_REFLECTION_DEFAULT as f64,
                // LFE
                lfe_cutoff_hz: upmixer_specs::LFE_CUTOFF_HZ_DEFAULT as f64,
                lfe_gain: upmixer_specs::LFE_GAIN_DEFAULT as f64,
                bandpass_hz: upmixer_specs::BANDPASS_HZ_DEFAULT as f64,
                // Sub-harmonic
                enable_subharmonic_synth: upmixer_specs::ENABLE_SUBHARMONIC_SYNTH_DEFAULT,
                subharmonic_gain: upmixer_specs::SUBHARMONIC_GAIN_DEFAULT as f64,
                subharmonic_freq_hz: upmixer_specs::SUBHARMONIC_FREQ_HZ_DEFAULT as f64,
                subharmonic_attack_ms: upmixer_specs::SUBHARMONIC_ATTACK_MS_DEFAULT as f64,
                subharmonic_release_ms: upmixer_specs::SUBHARMONIC_RELEASE_MS_DEFAULT as f64,
                // Decorrelation
                decorrelation_mode: upmixer_specs::DECORRELATION_MODE_DEFAULT as usize,
                decorrelation_lfo_rate_hz: upmixer_specs::DECORRELATION_LFO_RATE_HZ_DEFAULT as f64,
                velvet_noise_duration_ms: upmixer_specs::VELVET_NOISE_DURATION_MS_DEFAULT as f64,
                velvet_noise_density: upmixer_specs::VELVET_NOISE_DENSITY_DEFAULT as f64,
                // Height
                enable_hr_direct: upmixer_specs::ENABLE_HR_DIRECT_DEFAULT,
                hr_sharpen: upmixer_specs::HR_SHARPEN_DEFAULT as f64,
                height_hf_cap_hz: upmixer_specs::HEIGHT_HF_CAP_HZ_DEFAULT as f64,
                height_transient_reduction: upmixer_specs::HEIGHT_TRANSIENT_REDUCTION_DEFAULT
                    as f64,
                height_direct_leak: upmixer_specs::HEIGHT_DIRECT_LEAK_DEFAULT as f64,
                // Ambient
                ambient_boost: upmixer_specs::AMBIENT_BOOST_DEFAULT as f64,
                safety_cap_db: upmixer_specs::SAFETY_CAP_DB_DEFAULT as f64,
                rear_ambient_boost: upmixer_specs::REAR_AMBIENT_BOOST_DEFAULT as f64,
                // Dialogue
                dialogue_weight: upmixer_specs::DIALOGUE_WEIGHT_DEFAULT as f64,
                voice_freq_min_hz: upmixer_specs::VOICE_FREQ_MIN_HZ_DEFAULT as f64,
                voice_freq_max_hz: upmixer_specs::VOICE_FREQ_MAX_HZ_DEFAULT as f64,
                // Diagnostic bypass
                bypass_decorrelation: false,
                bypass_transient_detection: false,
                bypass_all_processing: false,
            },
            PluginType::Compressor => Self::Compressor {
                threshold_db: -20.0,
                ratio: 4.0,
                attack_ms: 5.0,
                release_ms: 100.0,
                knee_db: 3.0,
                makeup_gain_db: 0.0,
                mix: 0.95,
                auto_makeup: false,
                link_channels: true,
                sidechain_hpf_hz: 80.0,
            },
            PluginType::Limiter => Self::Limiter {
                threshold_db: -1.0,
                release_ms: 50.0,
                mix: default_limiter_mix(),
            },
            PluginType::Gate => Self::Gate {
                threshold_db: -40.0,
                ratio: 10.0,
                attack_ms: 1.0,
                release_ms: 100.0,
                mix: default_gate_mix(),
                link_channels: default_gate_link_channels(),
                sidechain_hpf_hz: default_gate_sidechain_hpf_hz(),
            },
            PluginType::Expander => Self::Expander {
                threshold_db: default_expander_threshold_db(),
                ratio: default_expander_ratio(),
                attack_ms: default_expander_attack_ms(),
                release_ms: default_expander_release_ms(),
                range_db: default_expander_range_db(),
                knee_db: default_expander_knee_db(),
                hysteresis_db: default_expander_hysteresis_db(),
                hold_ms: default_expander_hold_ms(),
                mix: default_expander_mix(),
                link_channels: default_expander_link_channels(),
                sidechain_hpf_hz: default_expander_sidechain_hpf_hz(),
            },
            PluginType::MultibandCompressor => Self::MultibandCompressor {
                num_bands: default_mb_compressor_num_bands(),
                crossover_preset: default_mb_compressor_crossover_preset(),
                crossover_freq_1: default_mb_compressor_crossover_freq_1(),
                crossover_freq_2: default_mb_compressor_crossover_freq_2(),
                crossover_freq_3: default_mb_compressor_crossover_freq_3(),
                crossover_freq_4: default_mb_compressor_crossover_freq_4(),
                threshold_db: default_mb_compressor_threshold_db(),
                ratio: default_mb_compressor_ratio(),
                attack_ms: default_mb_compressor_attack_ms(),
                release_ms: default_mb_compressor_release_ms(),
                knee_db: default_mb_compressor_knee_db(),
                mix: default_mb_compressor_mix(),
                link_channels: default_mb_compressor_link_channels(),
            },
            PluginType::MultibandExpander => Self::MultibandExpander {
                num_bands: default_mb_expander_num_bands(),
                crossover_preset: default_mb_expander_crossover_preset(),
                crossover_freq_1: default_mb_expander_crossover_freq_1(),
                crossover_freq_2: default_mb_expander_crossover_freq_2(),
                crossover_freq_3: default_mb_expander_crossover_freq_3(),
                crossover_freq_4: default_mb_expander_crossover_freq_4(),
                threshold_db: default_mb_expander_threshold_db(),
                ratio: default_mb_expander_ratio(),
                attack_ms: default_mb_expander_attack_ms(),
                release_ms: default_mb_expander_release_ms(),
                range_db: default_mb_expander_range_db(),
                knee_db: default_mb_expander_knee_db(),
                hysteresis_db: default_mb_expander_hysteresis_db(),
                hold_ms: default_mb_expander_hold_ms(),
                mix: default_mb_expander_mix(),
                link_channels: default_mb_expander_link_channels(),
            },
            PluginType::LoudnessCompensation => Self::LoudnessCompensation {
                low_freq: 100.0,    // param_specs::loudness_compensation::LOW_FREQ_DEFAULT
                low_gain: 6.0,      // param_specs::loudness_compensation::LOW_GAIN_DEFAULT
                high_freq: 10000.0, // param_specs::loudness_compensation::HIGH_FREQ_DEFAULT
                high_gain: 6.0,     // param_specs::loudness_compensation::HIGH_GAIN_DEFAULT
            },
            PluginType::BinauralDecoder => Self::BinauralDecoder {
                sofa_file: String::new(),
                input_channels: 6, // Default to 5.1
                enable_optimization: true,
                externalization: 0.0,
                near_field_strength: 0.0,
            },
            PluginType::Convolution => Self::Convolution {
                ir_file: String::new(),
                mix: 1.0,
                gain_db: 0.0,
            },
            PluginType::LoudnessMonitor => Self::LoudnessMonitor,
            PluginType::SpectrumAnalyzer => Self::SpectrumAnalyzer {
                num_bins: 30,
                min_freq: 20.0,
                max_freq: 20000.0,
                smoothing: 0.7,
            },
            PluginType::ChannelMuteSolo => Self::ChannelMuteSolo {
                enabled: false,
                channel_states: vec![],
            },
            PluginType::Matrix => Self::Matrix {
                input_channels: 2,
                output_channels: 2,
                matrix: vec![1.0, 0.0, 0.0, 1.0], // Identity 2x2
            },
            PluginType::XTC => Self::XTC {
                distance_m: default_xtc_distance_m(),
                speaker_angle_deg: default_xtc_speaker_angle_deg(),
                head_radius_m: default_xtc_head_radius_m(),
                beta_base: default_xtc_beta_base(),
                beta_low_freq_boost: default_xtc_beta_low_freq_boost(),
                beta_high_freq_boost: default_xtc_beta_high_freq_boost(),
                head_shadow_cutoff_hz: default_xtc_head_shadow_cutoff_hz(),
                head_shadow_slope_db_per_octave: default_xtc_head_shadow_slope(),
            },
            PluginType::Denoiser => Self::Denoiser {
                reduction_db: default_denoiser_reduction_db(),
                floor_db: default_denoiser_floor_db(),
                smoothing: default_denoiser_smoothing(),
                attack_ms: default_denoiser_attack_ms(),
                release_ms: default_denoiser_release_ms(),
                low_latency: default_denoiser_low_latency(),
            },
        }
    }
}

// ============================================================================
// Matrix Helper Functions
// ============================================================================

/// Get channel label for a given channel index and total channel count
/// Returns standard speaker labels (L, R, C, LFE, etc.) when possible
pub fn get_channel_label(index: usize, total: usize) -> String {
    const MONO: &[&str] = &["M"];
    const STEREO: &[&str] = &["L", "R"];
    const SURROUND_3_0: &[&str] = &["L", "R", "C"];
    const SURROUND_4_0: &[&str] = &["L", "R", "LS", "RS"];
    const SURROUND_5_0: &[&str] = &["L", "R", "C", "LS", "RS"];
    const SURROUND_5_1: &[&str] = &["L", "R", "C", "LFE", "LS", "RS"];
    const SURROUND_7_1: &[&str] = &["L", "R", "C", "LFE", "LS", "RS", "LB", "RB"];

    let labels: Option<&[&str]> = match total {
        1 => Some(MONO),
        2 => Some(STEREO),
        3 => Some(SURROUND_3_0),
        4 => Some(SURROUND_4_0),
        5 => Some(SURROUND_5_0),
        6 => Some(SURROUND_5_1),
        8 => Some(SURROUND_7_1),
        _ => None,
    };

    if let Some(labels) = labels {
        if index < labels.len() {
            return labels[index].to_string();
        }
    }

    // Fallback: generic channel number
    format!("Ch{}", index)
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
pub fn detect_matrix_preset(in_ch: usize, out_ch: usize, matrix: &[f32]) -> &'static str {
    if is_identity_matrix(in_ch, out_ch, matrix) {
        "Identity"
    } else if is_swap_matrix(in_ch, out_ch, matrix) {
        "Swap L/R"
    } else if is_mono_mix_matrix(in_ch, out_ch, matrix) {
        "Mono Mix"
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

/// Check if matrix swaps L/R (stereo only)
fn is_swap_matrix(in_ch: usize, out_ch: usize, matrix: &[f32]) -> bool {
    if in_ch != 2 || out_ch != 2 || matrix.len() != 4 {
        return false;
    }

    // Swap matrix: [[0, 1], [1, 0]]
    // Row-major: [0, 1, 1, 0]
    let expected = [0.0, 1.0, 1.0, 0.0];
    for (i, &exp) in expected.iter().enumerate() {
        if (matrix[i] - exp).abs() > 0.001 {
            return false;
        }
    }
    true
}

/// Check if matrix is a mono mix (all inputs summed equally to all outputs)
fn is_mono_mix_matrix(in_ch: usize, out_ch: usize, matrix: &[f32]) -> bool {
    if matrix.len() != in_ch * out_ch || in_ch == 0 {
        return false;
    }

    // Expected gain for equal power mix
    let expected_gain = 1.0 / (in_ch as f32).sqrt();

    for value in matrix {
        if (*value - expected_gain).abs() > 0.001 {
            return false;
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
                matrix[0 * in_ch + 1] = 1.0; // Out 0 <- In 1
                matrix[1 * in_ch + 0] = 1.0; // Out 1 <- In 0
                                             // Pass through remaining channels
                for i in 2..in_ch.min(out_ch) {
                    matrix[i * in_ch + i] = 1.0;
                }
            }
        }
        "Mono Mix" => {
            let gain = 1.0 / (in_ch as f32).sqrt();
            matrix.fill(gain);
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub id: usize,
    pub enabled: bool,
    pub settings: PluginSettings,
}

impl Plugin {
    pub fn new(id: usize, plugin_type: &PluginType) -> Self {
        Self {
            id,
            enabled: true,
            settings: PluginSettings::default_for(plugin_type),
        }
    }

    pub fn plugin_type(&self) -> PluginType {
        self.settings.plugin_type()
    }

    pub fn to_plugin_config(&self, sample_rate: f64) -> Option<PluginConfig> {
        if self.enabled {
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

    pub fn remove_plugin(&mut self, index: usize) -> Option<Plugin> {
        if index < self.plugins.len() {
            Some(self.plugins.remove(index))
        } else {
            None
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
        if from < self.plugins.len() && to < self.plugins.len() {
            let plugin = self.plugins.remove(from);
            self.plugins.insert(to, plugin);
        }
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

    /// Check if the chain has an enabled spectrum analyzer plugin
    pub fn has_enabled_spectrum_analyzer(&self) -> bool {
        self.plugins.iter().any(|p| {
            p.enabled && matches!(p.settings, PluginSettings::SpectrumAnalyzer { .. })
        })
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
    /// The engine order is: [Enabled Processing Plugins] followed by [Enabled Monitoring Plugins].
    pub fn get_engine_index(&self, ui_index: usize) -> Option<usize> {
        let target_plugin = self.plugins.get(ui_index)?;
        if !target_plugin.enabled {
            return None;
        }

        let target_is_monitor = target_plugin.plugin_type().is_monitoring();
        let mut engine_idx = 0;

        if !target_is_monitor {
            // Target is a processing plugin.
            // Engine index is the count of enabled processing plugins before it.
            for (i, p) in self.plugins.iter().enumerate() {
                if i == ui_index {
                    return Some(engine_idx);
                }
                if p.enabled && !p.plugin_type().is_monitoring() {
                    engine_idx += 1;
                }
            }
        } else {
            // Target is a monitoring plugin.
            // Engine index is (Count of ALL enabled processing plugins) + (Count of enabled monitors before it).

            // 1. Count all enabled processing plugins
            for p in &self.plugins {
                if p.enabled && !p.plugin_type().is_monitoring() {
                    engine_idx += 1;
                }
            }

            // 2. Count enabled monitors until we hit target
            for (i, p) in self.plugins.iter().enumerate() {
                if i == ui_index {
                    return Some(engine_idx);
                }
                if p.enabled && p.plugin_type().is_monitoring() {
                    engine_idx += 1;
                }
            }
        }

        None
    }

    pub fn to_plugin_configs(&self, sample_rate: f64) -> Vec<PluginConfig> {
        // Separate processing plugins from analyzer plugins
        // Analyzers should always be at the end to measure the final output
        let mut processing_plugins = Vec::new();
        let mut analyzer_plugins = Vec::new();

        for plugin in &self.plugins {
            if let Some(config) = plugin.to_plugin_config(sample_rate) {
                match plugin.plugin_type() {
                    // Analyzer plugins go at the end
                    PluginType::LoudnessMonitor
                    | PluginType::SpectrumAnalyzer
                    | PluginType::ChannelMuteSolo => {
                        analyzer_plugins.push(config);
                    }
                    // Processing plugins maintain their order
                    _ => {
                        processing_plugins.push(config);
                    }
                }
            }
        }

        // Concatenate: processing first, then analyzers
        processing_plugins.extend(analyzer_plugins);
        processing_plugins
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

    pub fn output_channels(&self) -> usize {
        // Walk backwards through the chain to find the last channel-count-changing plugin
        for plugin in self.plugins.iter().rev() {
            if !plugin.enabled {
                continue;
            }

            match &plugin.settings {
                PluginSettings::Upmixer { speaker_config, .. } => {
                    // Map speaker config to channel count
                    return match speaker_config.as_str() {
                        "2.0" => 2,
                        "5.0" => 5,
                        "5.1" => 6,
                        "7.1" => 8,
                        "5.1.2" => 8,
                        "5.1.4" => 10,
                        "7.1.2" => 10,
                        "7.1.4" => 12,
                        "9.1.4" => 14,
                        "9.1.6" => 16,
                        _ => {
                            log::warn!(
                                "Unknown speaker config '{}', defaulting to 5.1 (6 channels)",
                                speaker_config
                            );
                            6
                        }
                    };
                }
                PluginSettings::BinauralDecoder { .. } => {
                    // Binaural decoder always outputs stereo
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

        // No channel-changing plugin found, return stereo
        2
    }

    /// Save the plugin chain to a JSON file in the plugin_presets directory
    ///
    /// # Arguments
    /// * `filename` - The preset filename (with or without .json extension)
    ///
    /// # Returns
    /// * Ok(()) on success
    /// * Err if the extension is not .json or if saving fails
    pub fn save_to_file(&self, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
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

        // Get plugin_presets directory
        let presets_dir = crate::config::get_plugin_presets_dir()
            .ok_or("Could not access plugin presets directory")?;

        let full_path = presets_dir.join(&filename);

        // Security validation: ensure we're writing within config directory
        crate::security::validate_write_path(&full_path)?;

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

    /// Load the plugin chain from a JSON file in the plugin_presets directory
    ///
    /// # Arguments
    /// * `filename` - The preset filename (with or without .json extension)
    ///
    /// # Returns
    /// * Ok(()) on success
    /// * Err if the file doesn't exist or loading fails
    pub fn load_from_file(&mut self, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
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

        // Get plugin_presets directory
        let presets_dir = crate::config::get_plugin_presets_dir()
            .ok_or("Could not access plugin presets directory")?;

        let full_path = presets_dir.join(&final_filename);
        log::debug!("Full path: {}", full_path.display());

        // Security validation: ensure we're reading from within config directory
        crate::security::validate_config_read_path(&full_path)?;

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
            self.save_to_file(&final_filename)?;

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

    /// Update BinauralDecoder input_channels based on the output of plugins before them
    /// This should be called after any plugin chain modification (add, remove, move, toggle)
    pub fn update_binaural_decoder_channels(&mut self) {
        for i in 0..self.plugins.len() {
            if let PluginSettings::BinauralDecoder { sofa_file, .. } = &self.plugins[i].settings {
                // Calculate output channels from all plugins before this one
                let input_channels = if i == 0 {
                    2 // Stereo input by default
                } else {
                    // Create a temporary view of plugins before this one
                    let mut channels = 2; // Start with stereo
                    for j in 0..i {
                        if !self.plugins[j].enabled {
                            continue;
                        }
                        match &self.plugins[j].settings {
                            PluginSettings::Upmixer { speaker_config, .. } => {
                                channels = match speaker_config.as_str() {
                                    "2.0" => 2,
                                    "5.0" => 5,
                                    "5.1" => 6,
                                    "7.1" => 8,
                                    "5.1.2" => 8,
                                    "5.1.4" => 10,
                                    "7.1.2" => 10,
                                    "7.1.4" => 12,
                                    "9.1.4" => 14,
                                    "9.1.6" => 16,
                                    _ => 6, // Default to 5.1
                                };
                            }
                            PluginSettings::BinauralDecoder { .. } => {
                                channels = 2; // Binaural outputs stereo
                            }
                            PluginSettings::Matrix {
                                output_channels, ..
                            } => {
                                channels = *output_channels;
                            }
                            _ => {} // Other plugins don't change channel count
                        }
                    }
                    channels
                };

                // Update the BinauralDecoder with the calculated input channels
                // Preserve existing settings when updating input channels
                if let PluginSettings::BinauralDecoder {
                    enable_optimization,
                    externalization,
                    near_field_strength,
                    ..
                } = &self.plugins[i].settings
                {
                    let sofa_file = sofa_file.clone();
                    self.plugins[i].settings = PluginSettings::BinauralDecoder {
                        sofa_file,
                        input_channels,
                        enable_optimization: *enable_optimization,
                        externalization: *externalization,
                        near_field_strength: *near_field_strength,
                    };
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
                bypass_decorrelation: false,
                bypass_transient_detection: false,
                bypass_all_processing: false,
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
        chain.update_binaural_decoder_channels();

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
                bypass_decorrelation: false,
                bypass_transient_detection: false,
                bypass_all_processing: false,
            };
        }

        // Update binaural decoder channels
        chain.update_binaural_decoder_channels();

        // Now BinauralDecoder should have 8 input channels
        if let Some(plugin) = chain.get_plugin(1) {
            if let PluginSettings::BinauralDecoder { input_channels, .. } = plugin.settings {
                assert_eq!(input_channels, 8);
            }
        }

        // Remove the upmixer
        chain.remove_plugin(0);
        chain.update_binaural_decoder_channels();

        // Now BinauralDecoder should have 2 input channels (stereo)
        if let Some(plugin) = chain.get_plugin(0) {
            if let PluginSettings::BinauralDecoder { input_channels, .. } = plugin.settings {
                assert_eq!(input_channels, 2);
            }
        }
    }
}
