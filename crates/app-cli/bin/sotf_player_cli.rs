use clap::{Parser, Subcommand};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use sotf_audio::LoudnessCompensation;
use sotf_audio::plugins::{EQFilter, PluginChain, PluginSettings, PluginType};
use sotf_audio::{AudioEngineManager, PluginConfig, StreamingState, run_preflight_checks};
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

fn parse_loudness_compensation(vals: &Vec<f64>) -> Result<Option<LoudnessCompensation>, String> {
    let (ref_level, low, high) = match vals.as_slice() {
        [r, l] => (*r, *l, *l),
        [r, l, h] => (*r, *l, *h),
        _ => return Err("Expected 2 or 3 values: REF,LOW[,HIGH]".to_string()),
    };
    LoudnessCompensation::new(ref_level, low, high)
        .map(Some)
        .map_err(|e| e.to_string())
}

/// Get channel count for a speaker configuration
fn get_speaker_config_channels(config: &str) -> Result<usize, String> {
    match config {
        "2.0" => Ok(2),
        "5.0" => Ok(5),
        "5.1" => Ok(6),
        "7.1" => Ok(8),
        "5.1.2" => Ok(8),
        "5.1.4" => Ok(10),
        "7.1.2" => Ok(10),
        "7.1.4" => Ok(12),
        "9.1.4" => Ok(14),
        "9.1.6" => Ok(16),
        _ => Err(format!(
            "Invalid speaker configuration '{}'. Valid options: 2.0, 5.0, 5.1, 7.1, 5.1.2, 5.1.4, 7.1.2, 7.1.4, 9.1.4, 9.1.6",
            config
        )),
    }
}

// ============================================================================
// Per-plugin CLI argument structs
// ============================================================================

#[derive(Debug, Clone, clap::Args)]
struct UpmixerArgs {
    /// Enable stereo-to-surround upmixer (converts 2ch to multi-channel surround)
    #[arg(id = "upmixer_enabled", long = "upmixer", default_value_t = false)]
    enabled: bool,

    /// Upmixer speaker configuration (2.0, 5.0, 5.1, 7.1, 5.1.2, 5.1.4, 7.1.2, 7.1.4, 9.1.4, 9.1.6)
    #[arg(long = "upmixer-config", default_value = "5.1")]
    config: String,

    /// Upmixer FFT size (must be power of 2: 1024, 2048, 4096)
    #[arg(
        id = "upmixer_fft_size",
        long = "upmixer-fft-size",
        default_value = "2048"
    )]
    fft_size: usize,

    /// Upmixer front direct gain (0.0-2.0)
    #[arg(long = "upmixer-gain-front-direct", default_value = "1.0")]
    gain_front_direct: f32,

    /// Upmixer front ambient gain (0.0-2.0)
    #[arg(long = "upmixer-gain-front-ambient", default_value = "0.5")]
    gain_front_ambient: f32,

    /// Upmixer rear ambient gain (0.0-2.0)
    #[arg(long = "upmixer-gain-rear-ambient", default_value = "1.0")]
    gain_rear_ambient: f32,

    /// Upmixer LFE cutoff frequency (Hz, 40-200)
    #[arg(long = "upmixer-lfe-cutoff", default_value = "120.0")]
    lfe_cutoff_hz: f32,

    /// Upmixer stereo width (0.0-1.0)
    #[arg(
        id = "upmixer_stereo_width",
        long = "upmixer-stereo-width",
        default_value = "0.5"
    )]
    stereo_width: f32,

    /// Upmixer bandpass / upmix crossover frequency (Hz)
    #[arg(long = "upmixer-bandpass", default_value = "250.0")]
    bandpass_hz: f32,

    /// Upmixer height gain (0.0-2.0)
    #[arg(long = "upmixer-height-gain", default_value = "1.0")]
    height_gain: f32,

    /// Upmixer LFE gain (0.0-2.0)
    #[arg(long = "upmixer-lfe-gain", default_value = "1.0")]
    lfe_gain: f32,

    /// Enable Upmixer Sub-Harmonic Synthesizer (adds low-end impact)
    #[arg(long = "upmixer-subharmonic", default_value_t = false)]
    subharmonic: bool,

    /// Upmixer Sub-Harmonic Synthesizer gain (0.0-1.0)
    #[arg(long = "upmixer-subharmonic-gain", default_value = "0.5")]
    subharmonic_gain: f32,

    /// Enable Upmixer high-resolution direct path
    #[arg(long = "upmixer-hr-direct", default_value_t = false)]
    hr_direct: bool,

    /// Upmixer HR Sharpen depth (0.0-1.0)
    #[arg(long = "upmixer-hr-sharpen", default_value = "1.0")]
    hr_sharpen: f32,

    /// Upmixer safety cap in dB (0.0-12.0, 3.0 = default safety)
    #[arg(long = "upmixer-safety-cap-db", default_value = "3.0")]
    safety_cap_db: f32,
}

#[derive(Debug, Clone, clap::Args)]
struct BinauralArgs {
    /// Enable binaural decoder (converts multi-channel to binaural stereo using HRTFs)
    #[arg(id = "binaural_enabled", long = "binaural", default_value_t = false)]
    enabled: bool,

    /// Path to SOFA file for binaural decoder (required when --binaural is enabled)
    #[arg(long = "sofa-file")]
    sofa_file: Option<PathBuf>,

    /// Binaural decoder FFT size (must be power of 2: 2048, 4096, 8192)
    #[arg(
        id = "binaural_fft_size",
        long = "binaural-fft-size",
        default_value = "4096"
    )]
    fft_size: usize,

    /// Enable Binaural Decoder Sum-Before-IFFT optimization
    #[arg(long = "binaural-optimization", default_value_t = true)]
    optimization: bool,

    /// Binaural Decoder Externalization (0.0-1.0)
    #[arg(long = "binaural-externalization", default_value = "0.0")]
    externalization: f32,

    /// Binaural Decoder Near-Field Strength (0.0-1.0)
    #[arg(long = "binaural-near-field", default_value = "0.0")]
    near_field: f32,
}

#[derive(Debug, Clone, clap::Args)]
struct GainArgs {
    /// Enable gain plugin (simple volume control)
    #[arg(id = "gain_enabled", long = "gain", default_value_t = false)]
    enabled: bool,

    /// Gain in dB (-60 to +20)
    #[arg(id = "gain_db", long = "gain-db", default_value = "0.0")]
    gain_db: f32,
}

#[derive(Debug, Clone, clap::Args)]
struct CompressorArgs {
    /// Enable single-band compressor plugin
    #[arg(
        id = "compressor_enabled",
        long = "compressor",
        default_value_t = false
    )]
    enabled: bool,

    /// Compressor threshold in dB (-60 to 0)
    #[arg(
        id = "compressor_threshold_db",
        long = "compressor-threshold-db",
        default_value = "-20.0"
    )]
    threshold_db: f32,

    /// Compressor ratio (1.0 to 20.0)
    #[arg(
        id = "compressor_ratio",
        long = "compressor-ratio",
        default_value = "4.0"
    )]
    ratio: f32,

    /// Compressor attack time in ms (0.1 to 100)
    #[arg(
        id = "compressor_attack_ms",
        long = "compressor-attack-ms",
        default_value = "5.0"
    )]
    attack_ms: f32,

    /// Compressor release time in ms (10 to 1000)
    #[arg(
        id = "compressor_release_ms",
        long = "compressor-release-ms",
        default_value = "50.0"
    )]
    release_ms: f32,

    /// Compressor knee width in dB (0 to 20)
    #[arg(
        id = "compressor_knee_db",
        long = "compressor-knee-db",
        default_value = "6.0"
    )]
    knee_db: f32,

    /// Compressor makeup gain in dB (-24 to 24)
    #[arg(long = "compressor-makeup-gain-db", default_value = "0.0")]
    makeup_gain_db: f32,

    /// Compressor wet/dry mix (0.0 to 1.0)
    #[arg(id = "compressor_mix", long = "compressor-mix", default_value = "1.0")]
    mix: f32,

    /// Enable compressor auto-makeup gain
    #[arg(long = "compressor-auto-makeup", default_value_t = false)]
    auto_makeup: bool,

    /// Disable compressor channel linking (channels linked by default)
    #[arg(
        id = "compressor_unlink_channels",
        long = "compressor-unlink-channels",
        default_value_t = false
    )]
    unlink_channels: bool,

    /// Compressor sidechain HPF frequency in Hz (0 to 200)
    #[arg(
        id = "compressor_sidechain_hpf_hz",
        long = "compressor-sidechain-hpf-hz",
        default_value = "80.0"
    )]
    sidechain_hpf_hz: f32,
}

#[derive(Debug, Clone, clap::Args)]
struct GateArgs {
    /// Enable noise gate plugin
    #[arg(id = "gate_enabled", long = "gate", default_value_t = false)]
    enabled: bool,

    /// Gate threshold in dB (-80 to 0)
    #[arg(
        id = "gate_threshold_db",
        long = "gate-threshold-db",
        default_value = "-40.0"
    )]
    threshold_db: f32,

    /// Gate ratio (1.0 to 100.0)
    #[arg(id = "gate_ratio", long = "gate-ratio", default_value = "10.0")]
    ratio: f32,

    /// Gate attack time in ms (0.1 to 50)
    #[arg(id = "gate_attack_ms", long = "gate-attack-ms", default_value = "1.0")]
    attack_ms: f32,

    /// Gate hold time in ms (0 to 1000)
    #[arg(id = "gate_hold_ms", long = "gate-hold-ms", default_value = "10.0")]
    hold_ms: f32,

    /// Gate release time in ms (10 to 2000)
    #[arg(
        id = "gate_release_ms",
        long = "gate-release-ms",
        default_value = "100.0"
    )]
    release_ms: f32,

    /// Gate wet/dry mix (0.0 to 1.0)
    #[arg(id = "gate_mix", long = "gate-mix", default_value = "1.0")]
    mix: f32,

    /// Disable gate channel linking (channels linked by default)
    #[arg(
        id = "gate_unlink_channels",
        long = "gate-unlink-channels",
        default_value_t = false
    )]
    unlink_channels: bool,

    /// Gate sidechain HPF frequency in Hz (0 to 200)
    #[arg(
        id = "gate_sidechain_hpf_hz",
        long = "gate-sidechain-hpf-hz",
        default_value = "0.0"
    )]
    sidechain_hpf_hz: f32,
}

#[derive(Debug, Clone, clap::Args)]
struct LimiterArgs {
    /// Enable peak limiter plugin
    #[arg(id = "limiter_enabled", long = "limiter", default_value_t = false)]
    enabled: bool,

    /// Limiter threshold in dB (-20 to 0)
    #[arg(
        id = "limiter_threshold_db",
        long = "limiter-threshold-db",
        default_value = "-0.1"
    )]
    threshold_db: f32,

    /// Limiter release time in ms (10 to 1000)
    #[arg(
        id = "limiter_release_ms",
        long = "limiter-release-ms",
        default_value = "50.0"
    )]
    release_ms: f32,

    /// Limiter lookahead time in ms (0 to 20)
    #[arg(long = "limiter-lookahead-ms", default_value = "5.0")]
    lookahead_ms: f32,

    /// Enable soft-knee limiting
    #[arg(long = "limiter-soft", default_value_t = false)]
    soft: bool,

    /// Limiter wet/dry mix (0.0 to 1.0)
    #[arg(id = "limiter_mix", long = "limiter-mix", default_value = "1.0")]
    mix: f32,
}

#[derive(Debug, Clone, clap::Args)]
struct ExpanderArgs {
    /// Enable expander plugin (dynamic range expansion with hysteresis)
    #[arg(id = "expander_enabled", long = "expander", default_value_t = false)]
    enabled: bool,

    /// Expander threshold in dB (-80 to 0)
    #[arg(
        id = "expander_threshold_db",
        long = "expander-threshold-db",
        default_value = "-40.0"
    )]
    threshold_db: f32,

    /// Expander ratio (1.0 to 20.0)
    #[arg(id = "expander_ratio", long = "expander-ratio", default_value = "2.0")]
    ratio: f32,

    /// Expander attack time in ms (0.1 to 50)
    #[arg(
        id = "expander_attack_ms",
        long = "expander-attack-ms",
        default_value = "1.0"
    )]
    attack_ms: f32,

    /// Expander release time in ms (10 to 2000)
    #[arg(
        id = "expander_release_ms",
        long = "expander-release-ms",
        default_value = "100.0"
    )]
    release_ms: f32,

    /// Expander range in dB (0 to 80)
    #[arg(
        id = "expander_range_db",
        long = "expander-range-db",
        default_value = "40.0"
    )]
    range_db: f32,

    /// Expander knee width in dB (0 to 20)
    #[arg(
        id = "expander_knee_db",
        long = "expander-knee-db",
        default_value = "6.0"
    )]
    knee_db: f32,

    /// Expander hysteresis in dB (0 to 12)
    #[arg(
        id = "expander_hysteresis_db",
        long = "expander-hysteresis-db",
        default_value = "4.0"
    )]
    hysteresis_db: f32,

    /// Expander hold time in ms (0 to 500)
    #[arg(
        id = "expander_hold_ms",
        long = "expander-hold-ms",
        default_value = "10.0"
    )]
    hold_ms: f32,

    /// Expander wet/dry mix (0.0 to 1.0)
    #[arg(id = "expander_mix", long = "expander-mix", default_value = "1.0")]
    mix: f32,

    /// Disable expander channel linking (channels linked by default)
    #[arg(
        id = "expander_unlink_channels",
        long = "expander-unlink-channels",
        default_value_t = false
    )]
    unlink_channels: bool,

    /// Expander sidechain HPF frequency in Hz (0 to 500)
    #[arg(
        id = "expander_sidechain_hpf_hz",
        long = "expander-sidechain-hpf-hz",
        default_value = "80.0"
    )]
    sidechain_hpf_hz: f32,
}

#[derive(Debug, Clone, clap::Args)]
struct MultibandCompressorArgs {
    /// Enable multiband compressor plugin (3-band compression with crossovers)
    #[arg(
        id = "mb_compressor_enabled",
        long = "multiband-compressor",
        default_value_t = false
    )]
    enabled: bool,

    /// Multiband compressor threshold in dB (-60 to 0)
    #[arg(
        id = "mb_compressor_threshold_db",
        long = "mb-compressor-threshold-db",
        default_value = "-20.0"
    )]
    threshold_db: f32,

    /// Multiband compressor ratio (1.0 to 20.0)
    #[arg(
        id = "mb_compressor_ratio",
        long = "mb-compressor-ratio",
        default_value = "4.0"
    )]
    ratio: f32,

    /// Multiband compressor attack time in ms (0.1 to 100)
    #[arg(
        id = "mb_compressor_attack_ms",
        long = "mb-compressor-attack-ms",
        default_value = "5.0"
    )]
    attack_ms: f32,

    /// Multiband compressor release time in ms (10 to 1000)
    #[arg(
        id = "mb_compressor_release_ms",
        long = "mb-compressor-release-ms",
        default_value = "50.0"
    )]
    release_ms: f32,

    /// Multiband compressor knee width in dB (0 to 20)
    #[arg(
        id = "mb_compressor_knee_db",
        long = "mb-compressor-knee-db",
        default_value = "6.0"
    )]
    knee_db: f32,

    /// Multiband compressor wet/dry mix (0.0 to 1.0)
    #[arg(
        id = "mb_compressor_mix",
        long = "mb-compressor-mix",
        default_value = "1.0"
    )]
    mix: f32,
}

#[derive(Debug, Clone, clap::Args)]
struct MultibandExpanderArgs {
    /// Enable multiband expander plugin (3-band expansion with crossovers)
    #[arg(
        id = "mb_expander_enabled",
        long = "multiband-expander",
        default_value_t = false
    )]
    enabled: bool,

    /// Multiband expander threshold in dB (-80 to 0)
    #[arg(
        id = "mb_expander_threshold_db",
        long = "mb-expander-threshold-db",
        default_value = "-40.0"
    )]
    threshold_db: f32,

    /// Multiband expander ratio (1.0 to 20.0)
    #[arg(
        id = "mb_expander_ratio",
        long = "mb-expander-ratio",
        default_value = "2.0"
    )]
    ratio: f32,

    /// Multiband expander attack time in ms (0.1 to 50)
    #[arg(
        id = "mb_expander_attack_ms",
        long = "mb-expander-attack-ms",
        default_value = "1.0"
    )]
    attack_ms: f32,

    /// Multiband expander release time in ms (10 to 2000)
    #[arg(
        id = "mb_expander_release_ms",
        long = "mb-expander-release-ms",
        default_value = "100.0"
    )]
    release_ms: f32,

    /// Multiband expander range in dB (0 to 80)
    #[arg(
        id = "mb_expander_range_db",
        long = "mb-expander-range-db",
        default_value = "40.0"
    )]
    range_db: f32,

    /// Multiband expander knee width in dB (0 to 20)
    #[arg(
        id = "mb_expander_knee_db",
        long = "mb-expander-knee-db",
        default_value = "6.0"
    )]
    knee_db: f32,

    /// Multiband expander hysteresis in dB (0 to 12)
    #[arg(
        id = "mb_expander_hysteresis_db",
        long = "mb-expander-hysteresis-db",
        default_value = "4.0"
    )]
    hysteresis_db: f32,

    /// Multiband expander hold time in ms (0 to 500)
    #[arg(
        id = "mb_expander_hold_ms",
        long = "mb-expander-hold-ms",
        default_value = "10.0"
    )]
    hold_ms: f32,

    /// Multiband expander wet/dry mix (0.0 to 1.0)
    #[arg(
        id = "mb_expander_mix",
        long = "mb-expander-mix",
        default_value = "1.0"
    )]
    mix: f32,
}

#[derive(Debug, Clone, clap::Args)]
struct XtcArgs {
    /// Enable XTC (crosstalk cancellation) plugin for speaker playback
    #[arg(id = "xtc_enabled", long = "xtc", default_value_t = false)]
    enabled: bool,

    /// XTC speaker distance in meters
    #[arg(long = "xtc-distance-m", default_value = "2.0")]
    distance_m: f32,

    /// XTC speaker angle in degrees
    #[arg(long = "xtc-speaker-angle-deg", default_value = "30.0")]
    speaker_angle_deg: f32,

    /// XTC head radius in meters
    #[arg(long = "xtc-head-radius-m", default_value = "0.0875")]
    head_radius_m: f32,

    /// XTC regularization base parameter
    #[arg(long = "xtc-beta-base", default_value = "0.001")]
    beta_base: f32,

    /// XTC head shadow cutoff frequency in Hz
    #[arg(long = "xtc-head-shadow-cutoff-hz", default_value = "4000.0")]
    head_shadow_cutoff_hz: f32,

    /// XTC head shadow slope in dB/octave
    #[arg(long = "xtc-head-shadow-slope", default_value = "6.0")]
    head_shadow_slope: f32,
}

#[derive(Debug, Clone, clap::Args)]
struct DenoiserArgs {
    /// Enable denoiser plugin (Wiener filter with MCRA noise estimation)
    #[arg(id = "denoiser_enabled", long = "denoiser", default_value_t = false)]
    enabled: bool,

    /// Denoiser noise reduction strength (0-40 dB)
    #[arg(long = "denoiser-reduction-db", default_value = "12.0")]
    reduction_db: f32,

    /// Denoiser floor/minimum gain (-60 to -10 dB, prevents musical noise)
    #[arg(long = "denoiser-floor-db", default_value = "-30.0")]
    floor_db: f32,

    /// Denoiser temporal smoothing (0.0-0.99)
    #[arg(long = "denoiser-smoothing", default_value = "0.8")]
    smoothing: f32,

    /// Denoiser attack time (ms)
    #[arg(
        id = "denoiser_attack_ms",
        long = "denoiser-attack-ms",
        default_value = "5.0"
    )]
    attack_ms: f32,

    /// Denoiser release time (ms)
    #[arg(
        id = "denoiser_release_ms",
        long = "denoiser-release-ms",
        default_value = "50.0"
    )]
    release_ms: f32,

    /// Enable low-latency mode for denoiser (512 FFT vs 2048)
    #[arg(long = "denoiser-low-latency", default_value_t = false)]
    low_latency: bool,
}

#[derive(Debug, Clone, clap::Args)]
struct PndArgs {
    /// Enable PND (Polyphonic Note Detection) varispeed correction plugin
    #[arg(id = "pnd_enabled", long = "pnd", default_value_t = false)]
    enabled: bool,

    /// PND correction strength (0.0-2.0, 1.0 = full correction)
    #[arg(long = "pnd-correction-strength", default_value = "1.0")]
    correction_strength: f32,

    /// PND analysis window size in milliseconds (20-500)
    #[arg(long = "pnd-analysis-window-ms", default_value = "100.0")]
    analysis_window_ms: f32,

    /// PND drift smoothing factor (0.001-1.0)
    #[arg(long = "pnd-drift-smoothing", default_value = "0.1")]
    drift_smoothing: f32,
}

#[derive(Debug, Clone, clap::Args)]
struct FletcherMunsonArgs {
    /// Enable Fletcher-Munson equal-loudness compensation
    #[arg(id = "fm_enabled", long = "fletcher-munson", default_value_t = false)]
    enabled: bool,

    /// Fletcher-Munson reference level in dB (-40 to 0, default -14 ≈ 80 dB SPL)
    #[arg(long = "fm-reference-level-db", default_value = "-14.0")]
    reference_level_db: f32,

    /// Fletcher-Munson smoothing time in ms (1-200)
    #[arg(long = "fm-smoothing-ms", default_value = "30.0")]
    smoothing_ms: f32,
}

#[derive(Debug, Clone, clap::Args)]
struct ConvolutionArgs {
    /// Enable convolution plugin (FIR/impulse response processing)
    #[arg(
        id = "convolution_enabled",
        long = "convolution",
        default_value_t = false
    )]
    enabled: bool,

    /// Path to impulse response file (required when --convolution is enabled)
    #[arg(long = "convolution-ir-file")]
    ir_file: Option<PathBuf>,

    /// Convolution wet/dry mix (0.0 to 1.0)
    #[arg(
        id = "convolution_mix",
        long = "convolution-mix",
        default_value = "1.0"
    )]
    mix: f32,

    /// Convolution gain in dB (-20 to 20)
    #[arg(
        id = "convolution_gain_db",
        long = "convolution-gain-db",
        default_value = "0.0"
    )]
    gain_db: f32,
}

#[derive(Debug, Clone, clap::Args)]
struct SpectrumAnalyzerArgs {
    /// Enable spectrum analyzer plugin
    #[arg(
        id = "spectrum_enabled",
        long = "spectrum-analyzer",
        default_value_t = false
    )]
    enabled: bool,
}

#[derive(Debug, Clone, clap::Args)]
struct ChannelMuteSoloArgs {
    /// Enable channel mute/solo plugin
    #[arg(
        id = "mute_solo_enabled",
        long = "channel-mute-solo",
        default_value_t = false
    )]
    enabled: bool,
}

#[derive(Debug, Clone, clap::Args)]
struct ABCompareArgs {
    /// Enable A/B comparison plugin
    #[arg(
        id = "ab_compare_enabled",
        long = "ab-compare",
        default_value_t = false
    )]
    enabled: bool,
}

#[derive(Debug, Clone, clap::Args)]
struct BandSplitArgs {
    /// Enable band-split plugin (splits audio into low/high bands)
    #[arg(
        id = "band_split_enabled",
        long = "band-split",
        default_value_t = false
    )]
    enabled: bool,

    /// Band-split crossover frequency in Hz (20-20000)
    #[arg(long = "band-split-frequency", default_value = "300.0")]
    frequency: f64,

    /// Band-split crossover type (LR24 or LR48)
    #[arg(long = "band-split-crossover-type", default_value = "LR24")]
    crossover_type: String,
}

#[derive(Debug, Clone, clap::Args)]
struct BandMergeArgs {
    /// Enable band-merge plugin (merges frequency bands)
    #[arg(
        id = "band_merge_enabled",
        long = "band-merge",
        default_value_t = false
    )]
    enabled: bool,

    /// Number of bands to merge (2-8)
    #[arg(long = "band-merge-bands", default_value = "2")]
    bands: usize,
}

#[derive(Debug, Clone, clap::Args)]
struct DownmixArgs {
    /// Enable downmix plugin (phase-coherent surround to stereo)
    #[arg(id = "downmix_enabled", long = "downmix", default_value_t = false)]
    enabled: bool,

    /// Downmix center channel gain in dB (-12 to 0)
    #[arg(long = "downmix-center-gain-db", default_value = "-3.0")]
    center_gain_db: f32,

    /// Downmix surround channels gain in dB (-12 to 0)
    #[arg(long = "downmix-surround-gain-db", default_value = "-3.0")]
    surround_gain_db: f32,

    /// Downmix height channels gain in dB (-60 to 0)
    #[arg(long = "downmix-height-gain-db", default_value = "-6.0")]
    height_gain_db: f32,

    /// Downmix LFE channel gain in dB (-60 to 0)
    #[arg(long = "downmix-lfe-gain-db", default_value = "-10.0")]
    lfe_gain_db: f32,

    /// Enable phase-coherent downmixing
    #[arg(long = "downmix-phase-coherence", default_value_t = false)]
    phase_coherence: bool,
}

#[derive(Debug, Clone, clap::Args)]
struct MonoToStereoArgs {
    /// Enable mono-to-stereo conversion plugin
    #[arg(
        id = "mono_to_stereo_enabled",
        long = "mono-to-stereo",
        default_value_t = false
    )]
    enabled: bool,

    /// Stereo width (0.0 to 1.0)
    #[arg(
        id = "mono_to_stereo_width",
        long = "mono-to-stereo-width",
        default_value = "0.5"
    )]
    stereo_width: f32,

    /// Haas effect delay in ms (0.0 to 5.0)
    #[arg(long = "mono-to-stereo-haas-delay-ms", default_value = "1.5")]
    haas_delay_ms: f32,

    /// Enable complementary EQ decorrelation
    #[arg(long = "mono-to-stereo-comp-eq", default_value_t = false)]
    enable_comp_eq: bool,

    /// Complementary EQ depth in dB (0.0 to 3.0)
    #[arg(long = "mono-to-stereo-comp-eq-depth-db", default_value = "1.0")]
    comp_eq_depth_db: f32,

    /// Decorrelation low frequency in Hz (100-500)
    #[arg(long = "mono-to-stereo-decor-low-hz", default_value = "300.0")]
    decor_low_hz: f32,

    /// Decorrelation high frequency in Hz (1000-5000)
    #[arg(long = "mono-to-stereo-decor-high-hz", default_value = "2000.0")]
    decor_high_hz: f32,
}

// ============================================================================
// Umbrella PluginArgs struct
// ============================================================================

#[derive(Debug, Clone, clap::Args)]
struct PluginArgs {
    #[command(flatten)]
    upmixer: UpmixerArgs,
    #[command(flatten)]
    binaural: BinauralArgs,
    #[command(flatten)]
    gain: GainArgs,
    #[command(flatten)]
    compressor: CompressorArgs,
    #[command(flatten)]
    gate: GateArgs,
    #[command(flatten)]
    limiter: LimiterArgs,
    #[command(flatten)]
    expander: ExpanderArgs,
    #[command(flatten)]
    multiband_compressor: MultibandCompressorArgs,
    #[command(flatten)]
    multiband_expander: MultibandExpanderArgs,
    #[command(flatten)]
    xtc: XtcArgs,
    #[command(flatten)]
    denoiser: DenoiserArgs,
    #[command(flatten)]
    pnd: PndArgs,
    #[command(flatten)]
    fletcher_munson: FletcherMunsonArgs,
    #[command(flatten)]
    convolution: ConvolutionArgs,
    #[command(flatten)]
    spectrum_analyzer: SpectrumAnalyzerArgs,
    #[command(flatten)]
    channel_mute_solo: ChannelMuteSoloArgs,
    #[command(flatten)]
    ab_compare: ABCompareArgs,
    #[command(flatten)]
    band_split: BandSplitArgs,
    #[command(flatten)]
    band_merge: BandMergeArgs,
    #[command(flatten)]
    downmix: DownmixArgs,
    #[command(flatten)]
    mono_to_stereo: MonoToStereoArgs,
}

// ============================================================================
// Plugin config creation functions
// ============================================================================

fn create_upmixer_plugin_config(args: &UpmixerArgs) -> Result<PluginConfig, String> {
    use serde_json::json;

    if !args.fft_size.is_power_of_two() {
        return Err(format!(
            "Upmixer FFT size must be power of 2, got {}",
            args.fft_size
        ));
    }
    let _ = get_speaker_config_channels(&args.config)?;

    let parameters = json!({
        "speaker_config": args.config,
        "fft_size": args.fft_size,
        "gain_front_direct": args.gain_front_direct,
        "gain_front_ambient": args.gain_front_ambient,
        "gain_rear_ambient": args.gain_rear_ambient,
        "lfe_cutoff_hz": args.lfe_cutoff_hz,
        "stereo_width": args.stereo_width,
        "bandpass_hz": args.bandpass_hz,
        "height_gain": args.height_gain,
        "lfe_gain": args.lfe_gain,
        "enable_subharmonic_synth": args.subharmonic,
        "subharmonic_gain": args.subharmonic_gain,
        "enable_hr_direct": args.hr_direct,
        "hr_sharpen": args.hr_sharpen,
        "safety_cap_db": args.safety_cap_db,
    });

    Ok(PluginConfig {
        plugin_type: "upmixer".to_string(),
        parameters,
    })
}

fn create_loudness_compensation_plugin_config(
    lc: &LoudnessCompensation,
) -> Result<PluginConfig, String> {
    use serde_json::json;

    let parameters = json!({
        "low_freq": 100.0,
        "low_gain": lc.low_boost,
        "high_freq": 10000.0,
        "high_gain": lc.high_boost,
    });

    Ok(PluginConfig {
        plugin_type: "loudness_compensation".to_string(),
        parameters,
    })
}

fn create_binaural_decoder_plugin_config(
    args: &BinauralArgs,
    input_channels: usize,
) -> Result<PluginConfig, String> {
    use serde_json::json;

    if !args.fft_size.is_power_of_two() {
        return Err(format!(
            "Binaural decoder FFT size must be power of 2, got {}",
            args.fft_size
        ));
    }

    let sofa_path = args
        .sofa_file
        .as_ref()
        .ok_or("Binaural decoder requires --sofa-file to be specified")?;

    if !sofa_path.exists() {
        return Err(format!("SOFA file does not exist: {:?}", sofa_path));
    }

    let parameters = json!({
        "sofa_file": sofa_path.to_string_lossy().to_string(),
        "input_channels": input_channels,
        "fft_size": args.fft_size,
        "enable_optimization": args.optimization,
        "externalization": args.externalization,
        "near_field_strength": args.near_field,
    });

    Ok(PluginConfig {
        plugin_type: "binaural_decoder".to_string(),
        parameters,
    })
}

fn create_loudness_analyzer_plugin_config() -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "loudness_monitor".to_string(),
        parameters: json!({}),
    })
}

fn create_gain_plugin_config(args: &GainArgs) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "gain".to_string(),
        parameters: json!({
            "gain_db": args.gain_db,
        }),
    })
}

fn create_compressor_plugin_config(args: &CompressorArgs) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "compressor".to_string(),
        parameters: json!({
            "threshold_db": args.threshold_db,
            "ratio": args.ratio,
            "attack_ms": args.attack_ms,
            "release_ms": args.release_ms,
            "knee_db": args.knee_db,
            "makeup_gain_db": args.makeup_gain_db,
            "mix": args.mix,
            "auto_makeup": args.auto_makeup,
            "link_channels": !args.unlink_channels,
            "sidechain_hpf_hz": args.sidechain_hpf_hz,
        }),
    })
}

fn create_gate_plugin_config(args: &GateArgs) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "gate".to_string(),
        parameters: json!({
            "threshold_db": args.threshold_db,
            "ratio": args.ratio,
            "attack_ms": args.attack_ms,
            "hold_ms": args.hold_ms,
            "release_ms": args.release_ms,
            "mix": args.mix,
            "link_channels": !args.unlink_channels,
            "sidechain_hpf_hz": args.sidechain_hpf_hz,
        }),
    })
}

fn create_limiter_plugin_config(args: &LimiterArgs) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "limiter".to_string(),
        parameters: json!({
            "threshold_db": args.threshold_db,
            "release_ms": args.release_ms,
            "lookahead_ms": args.lookahead_ms,
            "soft": args.soft,
            "mix": args.mix,
        }),
    })
}

fn create_expander_plugin_config(args: &ExpanderArgs) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "expander".to_string(),
        parameters: json!({
            "threshold_db": args.threshold_db,
            "ratio": args.ratio,
            "attack_ms": args.attack_ms,
            "release_ms": args.release_ms,
            "range_db": args.range_db,
            "knee_db": args.knee_db,
            "hysteresis_db": args.hysteresis_db,
            "hold_ms": args.hold_ms,
            "mix": args.mix,
            "link_channels": !args.unlink_channels,
            "sidechain_hpf_hz": args.sidechain_hpf_hz,
        }),
    })
}

fn create_multiband_compressor_plugin_config(
    args: &MultibandCompressorArgs,
) -> Result<PluginConfig, String> {
    use serde_json::json;
    use sotf_plugins::param_specs::multiband_compressor;
    Ok(PluginConfig {
        plugin_type: "multiband_compressor".to_string(),
        parameters: json!({
            "num_bands": multiband_compressor::NUM_BANDS_DEFAULT,
            "crossover_preset": multiband_compressor::CROSSOVER_PRESET_DEFAULT,
            "crossover_freq_1": multiband_compressor::CROSSOVER_FREQ_1_DEFAULT,
            "crossover_freq_2": multiband_compressor::CROSSOVER_FREQ_2_DEFAULT,
            "crossover_freq_3": multiband_compressor::CROSSOVER_FREQ_3_DEFAULT,
            "crossover_freq_4": multiband_compressor::CROSSOVER_FREQ_4_DEFAULT,
            "threshold_db": args.threshold_db,
            "ratio": args.ratio,
            "attack_ms": args.attack_ms,
            "release_ms": args.release_ms,
            "knee_db": args.knee_db,
            "mix": args.mix,
            "link_channels": multiband_compressor::LINK_CHANNELS_DEFAULT,
        }),
    })
}

fn create_multiband_expander_plugin_config(
    args: &MultibandExpanderArgs,
) -> Result<PluginConfig, String> {
    use serde_json::json;
    use sotf_plugins::param_specs::multiband_expander;
    Ok(PluginConfig {
        plugin_type: "multiband_expander".to_string(),
        parameters: json!({
            "num_bands": multiband_expander::NUM_BANDS_DEFAULT,
            "crossover_preset": multiband_expander::CROSSOVER_PRESET_DEFAULT,
            "crossover_freq_1": multiband_expander::CROSSOVER_FREQ_1_DEFAULT,
            "crossover_freq_2": multiband_expander::CROSSOVER_FREQ_2_DEFAULT,
            "crossover_freq_3": multiband_expander::CROSSOVER_FREQ_3_DEFAULT,
            "crossover_freq_4": multiband_expander::CROSSOVER_FREQ_4_DEFAULT,
            "threshold_db": args.threshold_db,
            "ratio": args.ratio,
            "attack_ms": args.attack_ms,
            "release_ms": args.release_ms,
            "range_db": args.range_db,
            "knee_db": args.knee_db,
            "hysteresis_db": args.hysteresis_db,
            "hold_ms": args.hold_ms,
            "mix": args.mix,
            "link_channels": multiband_expander::LINK_CHANNELS_DEFAULT,
        }),
    })
}

fn create_xtc_plugin_config(args: &XtcArgs) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "xtc".to_string(),
        parameters: json!({
            "distance_m": args.distance_m,
            "speaker_angle_deg": args.speaker_angle_deg,
            "head_radius_m": args.head_radius_m,
            "beta_base": args.beta_base,
            "beta_low_freq_boost": 10.0,
            "beta_high_freq_boost": 10.0,
            "head_shadow_cutoff_hz": args.head_shadow_cutoff_hz,
            "head_shadow_slope_db_per_octave": args.head_shadow_slope,
        }),
    })
}

fn create_denoiser_plugin_config(args: &DenoiserArgs) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "denoiser".to_string(),
        parameters: json!({
            "reduction_db": args.reduction_db,
            "floor_db": args.floor_db,
            "smoothing": args.smoothing,
            "attack_ms": args.attack_ms,
            "release_ms": args.release_ms,
            "low_latency": args.low_latency,
        }),
    })
}

fn create_pnd_plugin_config(args: &PndArgs) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "pnd".to_string(),
        parameters: json!({
            "correction_strength": args.correction_strength,
            "analysis_window_ms": args.analysis_window_ms,
            "drift_smoothing": args.drift_smoothing,
        }),
    })
}

fn create_fletcher_munson_plugin_config(args: &FletcherMunsonArgs) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "fletcher_munson".to_string(),
        parameters: json!({
            "playback_volume_db": 0.0,
            "reference_level_db": args.reference_level_db,
            "smoothing_ms": args.smoothing_ms,
            "enabled": true,
        }),
    })
}

fn create_convolution_plugin_config(args: &ConvolutionArgs) -> Result<PluginConfig, String> {
    use serde_json::json;

    let ir_path = args
        .ir_file
        .as_ref()
        .ok_or("Convolution plugin requires --convolution-ir-file to be specified")?;

    if !ir_path.exists() {
        return Err(format!(
            "Impulse response file does not exist: {:?}",
            ir_path
        ));
    }

    Ok(PluginConfig {
        plugin_type: "convolution".to_string(),
        parameters: json!({
            "ir_file": ir_path.to_string_lossy().to_string(),
            "mix": args.mix,
            "gain_db": args.gain_db,
        }),
    })
}

fn create_spectrum_analyzer_plugin_config() -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "spectrum_analyzer".to_string(),
        parameters: json!({}),
    })
}

fn create_channel_mute_solo_plugin_config() -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "channel_mute_solo".to_string(),
        parameters: json!({
            "enabled": true,
        }),
    })
}

fn create_ab_compare_plugin_config() -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "ab_compare".to_string(),
        parameters: json!({}),
    })
}

fn create_band_split_plugin_config(args: &BandSplitArgs) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "band_split".to_string(),
        parameters: json!({
            "frequency": args.frequency,
            "type": args.crossover_type,
        }),
    })
}

fn create_band_merge_plugin_config(args: &BandMergeArgs) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "band_merge".to_string(),
        parameters: json!({
            "bands": args.bands,
        }),
    })
}

fn create_downmix_plugin_config(
    args: &DownmixArgs,
    input_channels: usize,
) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "downmix".to_string(),
        parameters: json!({
            "input_channels": input_channels,
            "center_gain_db": args.center_gain_db,
            "surround_gain_db": args.surround_gain_db,
            "height_gain_db": args.height_gain_db,
            "lfe_gain_db": args.lfe_gain_db,
            "phase_coherence": args.phase_coherence,
        }),
    })
}

fn create_mono_to_stereo_plugin_config(args: &MonoToStereoArgs) -> Result<PluginConfig, String> {
    use serde_json::json;
    Ok(PluginConfig {
        plugin_type: "mono_to_stereo".to_string(),
        parameters: json!({
            "stereo_width": args.stereo_width,
            "haas_delay_ms": args.haas_delay_ms,
            "enable_comp_eq": args.enable_comp_eq,
            "comp_eq_depth_db": args.comp_eq_depth_db,
            "decor_low_hz": args.decor_low_hz,
            "decor_high_hz": args.decor_high_hz,
        }),
    })
}

/// Convert Biquad filters to PluginConfig for EQ plugin
fn create_eq_plugin_config(filters: &[Biquad]) -> Result<PluginConfig, String> {
    use serde_json::json;

    let filter_configs: Result<Vec<_>, String> = filters
        .iter()
        .map(|f| {
            let filter_type = match f.filter_type {
                BiquadFilterType::HighpassVariableQ => "highpass".to_string(),
                _ => f.filter_type.long_name().to_lowercase(),
            };

            Ok(json!({
                "filter_type": filter_type,
                "freq": f.freq,
                "q": f.q,
                "db_gain": f.db_gain,
            }))
        })
        .collect();

    let filter_configs = filter_configs?;

    let parameters = json!({
        "filters": filter_configs,
    });

    Ok(PluginConfig {
        plugin_type: "eq".to_string(),
        parameters,
    })
}

// ============================================================================
// CLI definition
// ============================================================================

#[derive(Parser)]
#[command(name = "sotf_player")]
#[command(about = "Audio player with EQ, upmixing, and LUFS monitoring", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List available audio devices
    Devices,

    /// Analyze an audio file and print ReplayGain data (gain and peak)
    #[command(name = "replay-gain")]
    ReplayGain {
        /// Path to audio file (supports WAV, FLAC, MP3, AAC/M4A, Vorbis/OGG, AIFF)
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },

    /// Play an audio file using the streaming decoder (supports seeking and LUFS)
    Play {
        /// Path to audio file (supports WAV, FLAC, MP3, AAC/M4A, Vorbis/OGG, AIFF)
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Output device name (optional, uses default)
        #[arg(short, long)]
        device: Option<String>,

        /// EQ filters: "freq:q:gain" (Peak) or "type:freq:q:gain"
        ///
        /// Filter types: PK/PEAK, LS/LOWSHELF, HS/HIGHSHELF, LP/LOWPASS, HP/HIGHPASS, NO/NOTCH, BP/BANDPASS
        ///
        /// Examples: "1000:1.5:3.0" (Peak +3dB), "LS:100:0.7:-2.0" (Lowshelf -2dB), "HP:80:0.707:0"
        #[arg(short, long = "filter", value_name = "FILTER")]
        filters: Vec<String>,

        /// Hardware output channel mapping: "input_channels->output_channels"
        ///
        /// Maps input channels to specific hardware output channels. Use "_" for gaps.
        ///
        /// Examples:
        ///   "1,2->9,10"                 - Route stereo to hardware channels 9,10
        ///   "1,2,3,4,5->1,2,3,_,5,6"    - Route 5ch with gap (skip channel 4 position)
        ///   "1,2,3,4,5,6->13,14,15,16,17,18"  - Route 5.1 to channels 13-18
        #[arg(long = "hwaudio-play")]
        hwaudio_play: Option<String>,

        /// Duration to play in seconds (0 = play until stopped)
        #[arg(short = 't', long, default_value = "0")]
        duration: u64,

        /// Start playback at specific time (seconds)
        #[arg(short = 's', long, default_value = "0")]
        start_time: f64,

        /// Buffer size in chunks (32=low latency, 128=balanced, 1024=high reliability)
        #[arg(long = "buffer-chunks", default_value = "32")]
        _buffer_chunks: usize,

        /// Enable real-time LUFS monitoring (prints momentary/short-term loudness)
        #[arg(long = "lufs", alias = "monitor-lufs", default_value_t = false)]
        lufs: bool,

        /// Loudness compensation: 2 or 3 floats: REF LOW [HIGH] (dB; REF -100..20, boosts 0..20)
        #[arg(long = "loudness-compensation", value_name = "REF,LOW[,HIGH]", value_parser = clap::value_parser!(f64), value_delimiter = ',')]
        loudness_compensation: Option<Vec<f64>>,

        /// Use rack mode with specified plugin order (matches GPUI app plugin behavior)
        ///
        /// Available plugins: eq, upmixer, binaural, loudness, expander, compressor,
        /// single-compressor, gate, limiter, mb-compressor, mb-expander, xtc, denoiser,
        /// pnd, fletcher-munson/fm, gain, convolution/ir, spectrum/spectrum-analyzer,
        /// channel-mute-solo/mute-solo, ab-compare/ab, band-split, band-merge,
        /// downmix, mono-to-stereo, lufs
        ///
        /// Example: --rack upmixer,eq,lufs
        #[arg(long = "rack", value_name = "PLUGIN", value_delimiter = ',')]
        rack: Vec<String>,

        #[command(flatten)]
        plugins: Box<PluginArgs>,
    },

    /// Get current playback status
    Status,
}

fn main() {
    let cli = Cli::parse();

    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("sotf_cli_player.log")
        .expect("Failed to open log file");

    env_logger::Builder::from_default_env()
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .filter_level(log::LevelFilter::Debug)
        .filter_module("symphonia_core", log::LevelFilter::Debug)
        .init();

    log::info!("SOTF CLI Player starting...");

    // Run pre-flight checks before initializing the player
    if matches!(cli.command, Commands::Play { .. })
        && let Err(e) = run_preflight_checks()
    {
        eprintln!("\nPre-flight check failed:\n");
        eprintln!("{}\n", e);
        log::error!("Pre-flight check failed: {}", e);
        std::process::exit(1);
    }

    match cli.command {
        Commands::Devices => {
            if let Err(e) = list_devices() {
                log::error!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::ReplayGain { file } => match sotf_audio::replaygain::analyze_file(&file) {
            Ok(info) => {
                let msg = format!(
                    "ReplayGain analysis:\n  File: {:?}\n  Gain: {:+.2} dB\n  Peak: {:.6}",
                    file, info.gain, info.peak
                );
                log::info!("{}", msg);
                println!("{}", msg);
            }
            Err(e) => {
                log::error!("Error: {}", e);
                std::process::exit(1);
            }
        },
        Commands::Play {
            file,
            device,
            filters,
            hwaudio_play,
            duration,
            start_time,
            _buffer_chunks,
            lufs,
            loudness_compensation,
            rack,
            plugins,
        } => {
            // Parse filters
            let filter_params = match parse_filters(&filters) {
                Ok(params) => params,
                Err(e) => {
                    log::error!("Error parsing filters: {}", e);
                    std::process::exit(1);
                }
            };

            // Parse loudness compensation
            let loudness: Option<LoudnessCompensation> = match loudness_compensation {
                Some(ref vals) => parse_loudness_compensation(vals).unwrap_or_else(|e| {
                    log::error!("Error in --loudness-compensation: {}", e);
                    std::process::exit(1);
                }),
                None => None,
            };

            if let Err(e) = play_stream(
                file,
                device,
                filter_params,
                duration,
                start_time,
                hwaudio_play,
                lufs,
                loudness,
                rack,
                &plugins,
            ) {
                log::error!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Status => {
            log::info!("Status command not yet implemented (requires running manager instance)");
        }
    }
}

fn list_devices() -> Result<(), String> {
    let msg = "Enumerating audio devices...\n";
    log::info!("{}", msg);
    println!("{}", msg);

    let devices = sotf_audio::devices::get_audio_devices()
        .map_err(|e| format!("Failed to get devices: {}", e))?;

    // Print input devices
    if let Some(input_devices) = devices.get("input") {
        let title = "Input Devices:";
        let separator = "=".repeat(80);
        log::info!("{}", title);
        log::info!("{}", separator);
        println!("{}", title);
        println!("{}", separator);

        for (idx, device) in input_devices.iter().enumerate() {
            let default_marker = if device.is_default { " (Default)" } else { "" };

            if let Some(config) = &device.default_config {
                let rate_range = if device.available_sample_rates.is_empty() {
                    "unknown".to_string()
                } else if device.available_sample_rates.len() == 1 {
                    format!("{} Hz", device.available_sample_rates[0])
                } else {
                    format!(
                        "{}-{} Hz",
                        device.available_sample_rates.first().unwrap(),
                        device.available_sample_rates.last().unwrap()
                    )
                };

                let line = format!(
                    "  [{}] {}{} - {} ch, {} (current: {} Hz), {}",
                    idx + 1,
                    device.name,
                    default_marker,
                    config.channels,
                    rate_range,
                    config.sample_rate,
                    config.sample_format
                );
                log::info!("{}", line);
                println!("{}", line);
            } else {
                let line = format!("  [{}] {}{}", idx + 1, device.name, default_marker);
                log::info!("{}", line);
                println!("{}", line);
            }
        }
        log::info!("");
        println!();
    }

    // Print output devices
    if let Some(output_devices) = devices.get("output") {
        let title = "Output Devices:";
        let separator = "=".repeat(80);
        log::info!("{}", title);
        log::info!("{}", separator);
        println!("{}", title);
        println!("{}", separator);

        for (idx, device) in output_devices.iter().enumerate() {
            let default_marker = if device.is_default { " (Default)" } else { "" };

            if let Some(config) = &device.default_config {
                let rate_range = if device.available_sample_rates.is_empty() {
                    "unknown".to_string()
                } else if device.available_sample_rates.len() == 1 {
                    format!("{} Hz", device.available_sample_rates[0])
                } else {
                    format!(
                        "{}-{} Hz",
                        device.available_sample_rates.first().unwrap(),
                        device.available_sample_rates.last().unwrap()
                    )
                };

                let line = format!(
                    "  [{}] {}{} - {} ch, {} (current: {} Hz), {}",
                    idx + 1,
                    device.name,
                    default_marker,
                    config.channels,
                    rate_range,
                    config.sample_rate,
                    config.sample_format
                );
                log::info!("{}", line);
                println!("{}", line);
            } else {
                let line = format!("  [{}] {}{}", idx + 1, device.name, default_marker);
                log::info!("{}", line);
                println!("{}", line);
            }
        }
        log::info!("");
        println!();
    }

    Ok(())
}

fn parse_filter_type(type_str: &str) -> Result<BiquadFilterType, String> {
    match type_str.to_uppercase().as_str() {
        "PK" | "PEAK" => Ok(BiquadFilterType::Peak),
        "LS" | "LOWSHELF" => Ok(BiquadFilterType::Lowshelf),
        "HS" | "HIGHSHELF" => Ok(BiquadFilterType::Highshelf),
        "LP" | "LOWPASS" => Ok(BiquadFilterType::Lowpass),
        "HP" | "HIGHPASS" => Ok(BiquadFilterType::Highpass),
        "NO" | "NOTCH" => Ok(BiquadFilterType::Notch),
        "BP" | "BANDPASS" => Ok(BiquadFilterType::Bandpass),
        _ => Err(format!(
            "Unknown filter type '{}'. Valid types: PK/PEAK, LS/LOWSHELF, HS/HIGHSHELF, LP/LOWPASS, HP/HIGHPASS, NO/NOTCH, BP/BANDPASS",
            type_str
        )),
    }
}

fn parse_filters(filter_strings: &[String]) -> Result<Vec<Biquad>, String> {
    filter_strings
        .iter()
        .map(|filter_str| {
            let parts: Vec<&str> = filter_str.split(':').collect();

            let (filter_type, frequency, q, gain) = match parts.len() {
                3 => {
                    let frequency = parts[0]
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid frequency: {}", parts[0]))?;
                    let q = parts[1]
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid Q: {}", parts[1]))?;
                    let gain = parts[2]
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid gain: {}", parts[2]))?;
                    (BiquadFilterType::Peak, frequency, q, gain)
                }
                4 => {
                    let filter_type = parse_filter_type(parts[0])?;
                    let frequency = parts[1]
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid frequency: {}", parts[1]))?;
                    let q = parts[2]
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid Q: {}", parts[2]))?;
                    let gain = parts[3]
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid gain: {}", parts[3]))?;
                    (filter_type, frequency, q, gain)
                }
                _ => {
                    return Err(format!(
                        "Invalid filter format '{}'. Expected 'freq:q:gain' or 'type:freq:q:gain'",
                        filter_str
                    ));
                }
            };

            if !(20.0..=20000.0).contains(&frequency) {
                return Err(format!(
                    "Frequency must be between 20 and 20000 Hz, got {}",
                    frequency
                ));
            }
            if q <= 0.0 || q > 100.0 {
                return Err(format!("Q must be between 0 and 100, got {}", q));
            }
            if gain.abs() > 30.0 {
                return Err(format!("Gain must be between -30 and +30 dB, got {}", gain));
            }

            Ok(Biquad::new(filter_type, frequency, 48000.0, q, gain))
        })
        .collect()
}

/// Parse channel mapping specification and create matrix plugin config
fn parse_channel_mapping(mapping_str: &str) -> Result<(Vec<usize>, Vec<usize>, Vec<f32>), String> {
    let parts: Vec<&str> = mapping_str.split("->").collect();
    if parts.len() != 2 {
        return Err(format!(
            "Invalid mapping format '{}'. Expected 'in1,in2,...->out1,out2,...'",
            mapping_str
        ));
    }

    let input_channels: Result<Vec<usize>, _> = parts[0]
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<usize>()
                .map_err(|_| format!("Invalid input channel: '{}'", s))
        })
        .collect();
    let input_channels = input_channels?;

    if input_channels.is_empty() {
        return Err("No input channels specified".to_string());
    }

    let output_spec: Vec<&str> = parts[1].split(',').map(|s| s.trim()).collect();
    if output_spec.is_empty() {
        return Err("No output channels specified".to_string());
    }

    let mut channel_map: Vec<Option<usize>> = Vec::new();
    let mut max_hw_channel = 0;

    for spec in output_spec.iter() {
        if *spec == "_" {
            channel_map.push(None);
        } else {
            let hw_ch = spec
                .parse::<usize>()
                .map_err(|_| format!("Invalid output channel: '{}'", spec))?;
            if hw_ch == 0 {
                return Err("Channel indices must be >= 1 (1-indexed)".to_string());
            }
            channel_map.push(Some(hw_ch - 1));
            max_hw_channel = max_hw_channel.max(hw_ch);
        }
    }

    let non_gap_outputs: Vec<_> = channel_map.iter().filter_map(|&x| x).collect();
    if non_gap_outputs.len() != input_channels.len() {
        return Err(format!(
            "Mismatch: {} input channels but {} non-gap output positions",
            input_channels.len(),
            non_gap_outputs.len()
        ));
    }

    let input_channel_map: Vec<usize> = input_channels.iter().map(|&ch| ch - 1).collect();
    let output_channel_map: Vec<usize> = channel_map.iter().filter_map(|&x| x).collect();

    let input_count = input_channel_map.len();
    let output_count = output_channel_map.len();

    let mut matrix = vec![0.0f32; output_count * input_count];
    for i in 0..output_count.min(input_count) {
        matrix[i * input_count + i] = 1.0;
    }

    Ok((input_channel_map, output_channel_map, matrix))
}

fn create_matrix_plugin_config(
    input_channel_map: Vec<usize>,
    output_channel_map: Vec<usize>,
    matrix: Vec<f32>,
) -> Result<PluginConfig, String> {
    use serde_json::json;

    let parameters = json!({
        "input_channel_map": input_channel_map,
        "output_channel_map": output_channel_map,
        "matrix": matrix,
    });

    Ok(PluginConfig {
        plugin_type: "matrix".to_string(),
        parameters,
    })
}

// ============================================================================
// Playback
// ============================================================================

#[allow(clippy::too_many_arguments)]
fn play_stream(
    file: PathBuf,
    device: Option<String>,
    filters: Vec<Biquad>,
    duration: u64,
    start_time: f64,
    hwaudio_play: Option<String>,
    lufs: bool,
    loudness: Option<LoudnessCompensation>,
    rack: Vec<String>,
    plugins: &PluginArgs,
) -> Result<(), String> {
    log::info!("Starting streaming playback...");
    log::info!("  File: {:?}", file);
    log::info!("  Device: {:?}", device.as_deref().unwrap_or("default"));
    if start_time > 0.0 {
        log::info!("  Start time: {:.2}s", start_time);
    }
    log::info!("  Filters: {}", filters.len());

    if !filters.is_empty() {
        log::info!("\nEQ Filters:");
        for (idx, filter) in filters.iter().enumerate() {
            log::info!(
                "  [{}] {} Hz, Q={:.2}, Gain={:.1} dB",
                idx + 1,
                filter.freq,
                filter.q,
                filter.db_gain
            );
        }
    }
    log::info!("");

    // Validate convolution IR file if convolution is enabled
    if plugins.convolution.enabled && plugins.convolution.ir_file.is_none() {
        return Err(
            "Convolution plugin requires --convolution-ir-file to be specified".to_string(),
        );
    }

    // Create streaming manager with signal watching enabled
    let mut streaming_manager = AudioEngineManager::with_signal_watching(true);

    // Load the audio file
    let audio_info = streaming_manager
        .load_file(&file)
        .map_err(|e| format!("Failed to load audio file: {}", e))?;

    let msg = format!(
        "Loaded audio file:\n  Format: {}\n  Sample rate: {}Hz\n  Channels: {}\n  Bits per sample: {}",
        audio_info.format,
        audio_info.spec.sample_rate,
        audio_info.spec.channels,
        audio_info.spec.bits_per_sample
    );
    log::info!("{}", msg);
    println!("{}", msg);

    if let Some(duration_secs) = audio_info.duration_seconds {
        let dur_msg = format!("  Duration: {:.2}s", duration_secs);
        log::info!("{}", dur_msg);
        println!("{}", dur_msg);
    }
    println!();

    // Build plugin chain
    let (plugin_configs, output_channels, loudness_plugin_index) = if !rack.is_empty() {
        build_rack_mode_plugins(
            &rack,
            &audio_info,
            &filters,
            &loudness,
            plugins,
            device.as_deref(),
        )?
    } else {
        build_traditional_mode_plugins(
            &audio_info,
            &filters,
            &loudness,
            lufs,
            hwaudio_play.as_deref(),
            plugins,
        )?
    };

    // Start playback
    streaming_manager
        .start_playback(device, plugin_configs, output_channels)
        .map_err(|e| format!("Failed to start streaming playback: {}", e))?;

    // Set loudness plugin index if monitoring is enabled
    if let Some(index) = loudness_plugin_index {
        streaming_manager.set_loudness_plugin_index(index);
    }

    // Seek to start time if specified
    if start_time > 0.0 {
        log::info!("Seeking to {:.2}s...", start_time);
        streaming_manager
            .seek(start_time)
            .map_err(|e| format!("Failed to seek: {}", e))?;
    }

    log::info!("Streaming playback started successfully!");
    log::info!("Press Ctrl+C to stop\n");

    // Monitor playback
    let start_time_instant = std::time::Instant::now();
    let mut last_state = StreamingState::Idle;
    let mut last_shortterm: Option<f64> = None;

    loop {
        streaming_manager.try_recv_event();

        let current_state = streaming_manager.get_state();

        if current_state != last_state {
            match current_state {
                StreamingState::Loading => log::info!("State: Loading..."),
                StreamingState::Ready => log::info!("State: Ready"),
                StreamingState::Playing => log::info!("State: Playing"),
                StreamingState::Paused => log::info!("State: Paused"),
                StreamingState::Seeking => log::info!("State: Seeking..."),
                StreamingState::Error => {
                    log::error!("State: Error!");
                    break;
                }
                StreamingState::Idle => {
                    if last_state == StreamingState::Playing {
                        log::info!("\nPlayback finished");
                    }
                    break;
                }
            }
            last_state = current_state;
        }

        // Print loudness measurements if monitoring is enabled
        if loudness_plugin_index.is_some()
            && current_state == StreamingState::Playing
            && let Some(loudness) = streaming_manager.get_loudness()
        {
            let st = loudness.shortterm_lufs;
            let changed = match last_shortterm {
                None => true,
                Some(prev) => (st - prev).abs() >= 0.1,
            };
            if changed {
                let momentary_str = if loudness.momentary_lufs.is_infinite() {
                    "-∞".to_string()
                } else {
                    format!("{:5.1}", loudness.momentary_lufs)
                };
                let shortterm_str = if st.is_infinite() {
                    "-∞".to_string()
                } else {
                    format!("{:5.1}", st)
                };
                let rg = if st.is_infinite() { 0.0 } else { -18.0 - st };
                log::debug!(
                    "LUFS: M={} S={}  RG={:+4.1} dB  Peak={:.3}",
                    momentary_str,
                    shortterm_str,
                    rg,
                    loudness.peak
                );
                last_shortterm = Some(st);
            }
        }

        // Check duration
        if duration > 0 && start_time_instant.elapsed().as_secs() >= duration {
            log::info!("\n\nDuration reached, stopping...");
            break;
        }

        sleep(Duration::from_millis(100));
    }

    log::info!("Streaming playback stopped successfully");
    Ok(())
}

/// Build plugin chain using rack mode (PluginChain with specified plugin order)
fn build_rack_mode_plugins(
    rack: &[String],
    audio_info: &sotf_audio::AudioFileInfo,
    filters: &[Biquad],
    loudness: &Option<LoudnessCompensation>,
    plugins: &PluginArgs,
    device: Option<&str>,
) -> Result<(Vec<PluginConfig>, usize, Option<usize>), String> {
    log::info!("Using rack mode with plugins: {:?}", rack);

    let sample_rate =
        sotf_audio::select_output_sample_rate(audio_info.spec.sample_rate, device) as f64;

    let mut chain = PluginChain::new();
    let mut has_lufs = false;

    for plugin_name in rack {
        match plugin_name.to_lowercase().as_str() {
            "upmixer" => {
                if audio_info.spec.channels != 2 {
                    return Err(format!(
                        "Upmixer requires stereo input, got {} channels",
                        audio_info.spec.channels
                    ));
                }

                let idx = chain.add_plugin(&PluginType::Upmixer);
                if let Some(plugin) = chain.get_plugin_mut(idx) {
                    plugin.settings = PluginSettings::Upmixer {
                        speaker_config: plugins.upmixer.config.clone(),
                        gain_front_direct: plugins.upmixer.gain_front_direct as f64,
                        gain_front_ambient: plugins.upmixer.gain_front_ambient as f64,
                        gain_rear_ambient: plugins.upmixer.gain_rear_ambient as f64,
                        height_gain: plugins.upmixer.height_gain as f64,
                        stereo_width: plugins.upmixer.stereo_width as f64,
                        center_spread: 0.5,
                        surround_direct_bleed: 0.3,
                        rear_late_reflection: 0.2,
                        lfe_cutoff_hz: plugins.upmixer.lfe_cutoff_hz as f64,
                        lfe_gain: plugins.upmixer.lfe_gain as f64,
                        bandpass_hz: plugins.upmixer.bandpass_hz as f64,
                        enable_subharmonic_synth: plugins.upmixer.subharmonic,
                        subharmonic_gain: plugins.upmixer.subharmonic_gain as f64,
                        subharmonic_freq_hz: 60.0,
                        subharmonic_attack_ms: 10.0,
                        subharmonic_release_ms: 100.0,
                        decorrelation_mode: 0,
                        decorrelation_lfo_rate_hz: 0.5,
                        velvet_noise_duration_ms: 50.0,
                        velvet_noise_density: 0.5,
                        enable_hr_direct: plugins.upmixer.hr_direct,
                        hr_sharpen: plugins.upmixer.hr_sharpen as f64,
                        height_hf_cap_hz: 8000.0,
                        height_transient_reduction: 0.5,
                        height_direct_leak: 0.1,
                        ambient_boost: 0.0,
                        safety_cap_db: plugins.upmixer.safety_cap_db as f64,
                        rear_ambient_boost: 0.0,
                        dialogue_weight: 1.0,
                        voice_freq_min_hz: 85.0,
                        voice_freq_max_hz: 3000.0,
                        bypass_decorrelation: false,
                        bypass_transient_detection: false,
                        bypass_all_processing: false,
                    };
                }
                log::info!("Rack: Added Upmixer plugin ({})", plugins.upmixer.config);
            }
            "binaural" => {
                let sofa_path = plugins
                    .binaural
                    .sofa_file
                    .clone()
                    .ok_or("Binaural decoder requires --sofa-file to be specified")?;
                let input_channels = chain.output_channels();

                let idx = chain.add_plugin(&PluginType::BinauralDecoder);
                if let Some(plugin) = chain.get_plugin_mut(idx) {
                    plugin.settings = PluginSettings::BinauralDecoder {
                        sofa_file: sofa_path.to_string_lossy().to_string(),
                        input_channels,
                        enable_optimization: plugins.binaural.optimization,
                        externalization: plugins.binaural.externalization as f64,
                        near_field_strength: plugins.binaural.near_field as f64,
                    };
                }
                log::info!("Rack: Added BinauralDecoder plugin");
            }
            "loudness" | "loudness-compensation" => {
                if let Some(lc) = loudness {
                    let idx = chain.add_plugin(&PluginType::LoudnessCompensation);
                    if let Some(plugin) = chain.get_plugin_mut(idx) {
                        plugin.settings = PluginSettings::LoudnessCompensation {
                            low_freq: 100.0,
                            low_gain: lc.low_boost,
                            high_freq: 10000.0,
                            high_gain: lc.high_boost,
                            auto_gain_enabled: false,
                            auto_gain_max_db: 12.0,
                            auto_gain_smoothing_ms: 100.0,
                        };
                    }
                } else {
                    chain.add_plugin(&PluginType::LoudnessCompensation);
                }
                log::info!("Rack: Added LoudnessCompensation plugin");
            }
            "eq" => {
                let channels = chain.output_channels();
                let idx = chain.add_plugin(&PluginType::EQ);
                if let Some(plugin) = chain.get_plugin_mut(idx) {
                    let eq_filters: Vec<EQFilter> = filters
                        .iter()
                        .map(|f| EQFilter::new(f.filter_type, f.freq, f.q, f.db_gain))
                        .collect();
                    plugin.settings = PluginSettings::EQ {
                        channels,
                        filters: eq_filters,
                        channel_filters: None,
                        per_channel_mode: false,
                    };
                }
                log::info!("Rack: Added EQ plugin with {} filters", filters.len());
            }
            "gain" => {
                let channels = chain.output_channels();
                let idx = chain.add_plugin(&PluginType::Gain);
                if let Some(plugin) = chain.get_plugin_mut(idx) {
                    plugin.settings = PluginSettings::Gain {
                        channels,
                        gain_db: plugins.gain.gain_db as f64,
                    };
                }
                log::info!("Rack: Added Gain plugin ({:.1} dB)", plugins.gain.gain_db);
            }
            "single-compressor" | "compressor"
                if plugins.compressor.enabled
                    || plugin_name.to_lowercase() == "single-compressor" =>
            {
                let idx = chain.add_plugin(&PluginType::Compressor);
                if let Some(plugin) = chain.get_plugin_mut(idx) {
                    plugin.settings = PluginSettings::Compressor {
                        threshold_db: plugins.compressor.threshold_db as f64,
                        ratio: plugins.compressor.ratio as f64,
                        attack_ms: plugins.compressor.attack_ms as f64,
                        release_ms: plugins.compressor.release_ms as f64,
                        knee_db: plugins.compressor.knee_db as f64,
                        makeup_gain_db: plugins.compressor.makeup_gain_db as f64,
                        mix: plugins.compressor.mix as f64,
                        auto_makeup: plugins.compressor.auto_makeup,
                        link_channels: !plugins.compressor.unlink_channels,
                        sidechain_hpf_hz: plugins.compressor.sidechain_hpf_hz as f64,
                    };
                }
                log::info!("Rack: Added Compressor plugin");
            }
            "compressor" | "mb-compressor" => {
                let idx = chain.add_plugin(&PluginType::MultibandCompressor);
                if let Some(plugin) = chain.get_plugin_mut(idx) {
                    plugin.settings = PluginSettings::MultibandCompressor {
                        num_bands: 3,
                        crossover_preset: 1,
                        crossover_freq_1: 200.0,
                        crossover_freq_2: 2000.0,
                        crossover_freq_3: 8000.0,
                        crossover_freq_4: 12000.0,
                        threshold_db: plugins.multiband_compressor.threshold_db as f64,
                        ratio: plugins.multiband_compressor.ratio as f64,
                        attack_ms: plugins.multiband_compressor.attack_ms as f64,
                        release_ms: plugins.multiband_compressor.release_ms as f64,
                        knee_db: plugins.multiband_compressor.knee_db as f64,
                        mix: plugins.multiband_compressor.mix as f64,
                        link_channels: true,
                        bands: vec![],
                    };
                }
                log::info!("Rack: Added MultibandCompressor plugin");
            }
            "gate" => {
                let idx = chain.add_plugin(&PluginType::Gate);
                if let Some(plugin) = chain.get_plugin_mut(idx) {
                    plugin.settings = PluginSettings::Gate {
                        threshold_db: plugins.gate.threshold_db as f64,
                        ratio: plugins.gate.ratio as f64,
                        attack_ms: plugins.gate.attack_ms as f64,
                        hold_ms: plugins.gate.hold_ms as f64,
                        release_ms: plugins.gate.release_ms as f64,
                        mix: plugins.gate.mix as f64,
                        link_channels: !plugins.gate.unlink_channels,
                        sidechain_hpf_hz: plugins.gate.sidechain_hpf_hz as f64,
                    };
                }
                log::info!("Rack: Added Gate plugin");
            }
            "limiter" => {
                let idx = chain.add_plugin(&PluginType::Limiter);
                if let Some(plugin) = chain.get_plugin_mut(idx) {
                    plugin.settings = PluginSettings::Limiter {
                        threshold_db: plugins.limiter.threshold_db as f64,
                        release_ms: plugins.limiter.release_ms as f64,
                        lookahead_ms: plugins.limiter.lookahead_ms as f64,
                        soft: plugins.limiter.soft,
                        mix: plugins.limiter.mix as f64,
                    };
                }
                log::info!("Rack: Added Limiter plugin");
            }
            "expander" => {
                let idx = chain.add_plugin(&PluginType::Expander);
                if let Some(plugin) = chain.get_plugin_mut(idx) {
                    plugin.settings = PluginSettings::Expander {
                        threshold_db: plugins.expander.threshold_db as f64,
                        ratio: plugins.expander.ratio as f64,
                        attack_ms: plugins.expander.attack_ms as f64,
                        release_ms: plugins.expander.release_ms as f64,
                        range_db: plugins.expander.range_db as f64,
                        knee_db: plugins.expander.knee_db as f64,
                        hysteresis_db: plugins.expander.hysteresis_db as f64,
                        hold_ms: plugins.expander.hold_ms as f64,
                        mix: plugins.expander.mix as f64,
                        link_channels: !plugins.expander.unlink_channels,
                        sidechain_hpf_hz: plugins.expander.sidechain_hpf_hz as f64,
                    };
                }
                log::info!("Rack: Added Expander plugin");
            }
            "mb-expander" => {
                let idx = chain.add_plugin(&PluginType::MultibandExpander);
                if let Some(plugin) = chain.get_plugin_mut(idx) {
                    plugin.settings = PluginSettings::MultibandExpander {
                        num_bands: 3,
                        crossover_preset: 1,
                        crossover_freq_1: 200.0,
                        crossover_freq_2: 2000.0,
                        crossover_freq_3: 8000.0,
                        crossover_freq_4: 12000.0,
                        threshold_db: plugins.multiband_expander.threshold_db as f64,
                        ratio: plugins.multiband_expander.ratio as f64,
                        attack_ms: plugins.multiband_expander.attack_ms as f64,
                        release_ms: plugins.multiband_expander.release_ms as f64,
                        range_db: plugins.multiband_expander.range_db as f64,
                        knee_db: plugins.multiband_expander.knee_db as f64,
                        hysteresis_db: plugins.multiband_expander.hysteresis_db as f64,
                        hold_ms: plugins.multiband_expander.hold_ms as f64,
                        mix: plugins.multiband_expander.mix as f64,
                        link_channels: true,
                        bands: vec![],
                    };
                }
                log::info!("Rack: Added MultibandExpander plugin");
            }
            "xtc" => {
                let idx = chain.add_plugin(&PluginType::XTC);
                if let Some(plugin) = chain.get_plugin_mut(idx) {
                    plugin.settings = PluginSettings::XTC {
                        distance_m: plugins.xtc.distance_m as f64,
                        speaker_angle_deg: plugins.xtc.speaker_angle_deg as f64,
                        head_radius_m: plugins.xtc.head_radius_m as f64,
                        beta_base: plugins.xtc.beta_base as f64,
                        beta_low_freq_boost: 10.0,
                        beta_high_freq_boost: 10.0,
                        head_shadow_cutoff_hz: plugins.xtc.head_shadow_cutoff_hz as f64,
                        head_shadow_slope_db_per_octave: plugins.xtc.head_shadow_slope as f64,
                    };
                }
                log::info!("Rack: Added XTC plugin");
            }
            "denoiser" => {
                let idx = chain.add_plugin(&PluginType::Denoiser);
                if let Some(plugin) = chain.get_plugin_mut(idx) {
                    plugin.settings = PluginSettings::Denoiser {
                        reduction_db: plugins.denoiser.reduction_db as f64,
                        floor_db: plugins.denoiser.floor_db as f64,
                        smoothing: plugins.denoiser.smoothing as f64,
                        attack_ms: plugins.denoiser.attack_ms as f64,
                        release_ms: plugins.denoiser.release_ms as f64,
                        low_latency: plugins.denoiser.low_latency,
                        polyphonic_detection: false,
                        transparency: 0.0,
                        dd_enabled: false,
                        dd_alpha: 0.98,
                        psychoacoustic_masking: true,
                        use_captured_profile: false,
                    };
                }
                log::info!("Rack: Added Denoiser plugin");
            }
            "pnd" => {
                let idx = chain.add_plugin(&PluginType::Pnd);
                if let Some(plugin) = chain.get_plugin_mut(idx) {
                    plugin.settings = PluginSettings::Pnd {
                        correction_strength: plugins.pnd.correction_strength as f64,
                        analysis_window_ms: plugins.pnd.analysis_window_ms as f64,
                        drift_smoothing: plugins.pnd.drift_smoothing as f64,
                    };
                }
                log::info!("Rack: Added PND plugin");
            }
            "fletcher-munson" | "fm" => {
                let idx = chain.add_plugin(&PluginType::FletcherMunson);
                if let Some(plugin) = chain.get_plugin_mut(idx) {
                    plugin.settings = PluginSettings::FletcherMunson {
                        playback_volume_db: 0.0,
                        reference_level_db: plugins.fletcher_munson.reference_level_db as f64,
                        enabled: true,
                        band1_freq: 60.0,
                        band1_q: 0.5,
                        band1_max_gain: 15.0,
                        band1_slope: 0.6,
                        band2_freq: 250.0,
                        band2_q: 0.707,
                        band2_max_gain: 8.0,
                        band2_slope: 0.4,
                        band3_freq: 3500.0,
                        band3_q: 1.0,
                        band3_max_gain: 4.0,
                        band3_slope: 0.2,
                        band4_freq: 12000.0,
                        band4_q: 0.707,
                        band4_max_gain: 6.0,
                        band4_slope: 0.3,
                        smoothing_ms: plugins.fletcher_munson.smoothing_ms as f64,
                        auto_gain_enabled: false,
                        auto_gain_max_db: 12.0,
                        auto_gain_smoothing_ms: 100.0,
                        auto_gain_loudness_type: 0,
                    };
                }
                log::info!("Rack: Added FletcherMunson plugin");
            }
            "convolution" | "ir" => {
                let ir_path = plugins
                    .convolution
                    .ir_file
                    .as_ref()
                    .ok_or("Convolution requires --convolution-ir-file to be specified")?;
                let idx = chain.add_plugin(&PluginType::Convolution);
                if let Some(plugin) = chain.get_plugin_mut(idx) {
                    plugin.settings = PluginSettings::Convolution {
                        ir_file: ir_path.to_string_lossy().to_string(),
                        mix: plugins.convolution.mix as f64,
                        gain_db: plugins.convolution.gain_db as f64,
                    };
                }
                log::info!("Rack: Added Convolution plugin");
            }
            "spectrum" | "spectrum-analyzer" => {
                chain.add_plugin(&PluginType::SpectrumAnalyzer);
                log::info!("Rack: Added SpectrumAnalyzer plugin");
            }
            "channel-mute-solo" | "mute-solo" => {
                chain.add_plugin(&PluginType::ChannelMuteSolo);
                log::info!("Rack: Added ChannelMuteSolo plugin");
            }
            "ab-compare" | "ab" => {
                chain.add_plugin(&PluginType::ABCompare);
                log::info!("Rack: Added ABCompare plugin");
            }
            "band-split" => {
                let channels = chain.output_channels();
                let idx = chain.add_plugin(&PluginType::BandSplit);
                if let Some(plugin) = chain.get_plugin_mut(idx) {
                    plugin.settings = PluginSettings::BandSplit {
                        channels,
                        frequency: plugins.band_split.frequency,
                        crossover_type: plugins.band_split.crossover_type.clone(),
                    };
                }
                log::info!("Rack: Added BandSplit plugin");
            }
            "band-merge" => {
                let channels = chain.output_channels();
                let idx = chain.add_plugin(&PluginType::BandMerge);
                if let Some(plugin) = chain.get_plugin_mut(idx) {
                    plugin.settings = PluginSettings::BandMerge {
                        channels,
                        bands: plugins.band_merge.bands,
                    };
                }
                log::info!("Rack: Added BandMerge plugin");
            }
            "downmix" => {
                let input_channels = chain.output_channels();
                let idx = chain.add_plugin(&PluginType::Downmix);
                if let Some(plugin) = chain.get_plugin_mut(idx) {
                    plugin.settings = PluginSettings::Downmix {
                        input_channels,
                        center_gain_db: plugins.downmix.center_gain_db as f64,
                        surround_gain_db: plugins.downmix.surround_gain_db as f64,
                        height_gain_db: plugins.downmix.height_gain_db as f64,
                        lfe_gain_db: plugins.downmix.lfe_gain_db as f64,
                        phase_coherence: plugins.downmix.phase_coherence,
                        phase_blend_low_hz: 500.0,
                        phase_blend_high_hz: 2000.0,
                    };
                }
                log::info!("Rack: Added Downmix plugin");
            }
            "mono-to-stereo" => {
                let idx = chain.add_plugin(&PluginType::MonoToStereo);
                if let Some(plugin) = chain.get_plugin_mut(idx) {
                    plugin.settings = PluginSettings::MonoToStereo {
                        stereo_width: plugins.mono_to_stereo.stereo_width as f64,
                        haas_delay_ms: plugins.mono_to_stereo.haas_delay_ms as f64,
                        enable_comp_eq: plugins.mono_to_stereo.enable_comp_eq,
                        comp_eq_depth_db: plugins.mono_to_stereo.comp_eq_depth_db as f64,
                        decor_low_hz: plugins.mono_to_stereo.decor_low_hz as f64,
                        decor_high_hz: plugins.mono_to_stereo.decor_high_hz as f64,
                    };
                }
                log::info!("Rack: Added MonoToStereo plugin");
            }
            "lufs" | "loudness-monitor" => {
                chain.add_plugin(&PluginType::LoudnessMonitor);
                has_lufs = true;
                log::info!("Rack: Added LoudnessMonitor plugin");
            }
            unknown => {
                return Err(format!(
                    "Unknown plugin '{}'. Available: eq, upmixer, binaural, loudness, gain, \
                    single-compressor, compressor/mb-compressor, gate, limiter, expander, \
                    mb-expander, xtc, denoiser, pnd, fletcher-munson/fm, convolution/ir, \
                    spectrum/spectrum-analyzer, channel-mute-solo/mute-solo, ab-compare/ab, \
                    band-split, band-merge, downmix, mono-to-stereo, lufs",
                    unknown
                ));
            }
        }
    }

    let plugin_configs = chain.to_plugin_configs(sample_rate);
    let output_channels = chain.output_channels();

    let actual_loudness_idx = if has_lufs {
        plugin_configs
            .iter()
            .position(|p| p.plugin_type == "loudness_monitor")
    } else {
        None
    };

    Ok((plugin_configs, output_channels, actual_loudness_idx))
}

/// Build plugin chain using traditional mode (manual PluginConfig building)
fn build_traditional_mode_plugins(
    audio_info: &sotf_audio::AudioFileInfo,
    filters: &[Biquad],
    loudness: &Option<LoudnessCompensation>,
    lufs: bool,
    hwaudio_play: Option<&str>,
    plugins: &PluginArgs,
) -> Result<(Vec<PluginConfig>, usize, Option<usize>), String> {
    let mut plugin_configs = Vec::new();

    // 1. Mono to Stereo (early, before upmixer)
    let output_channels = if plugins.mono_to_stereo.enabled {
        let plugin = create_mono_to_stereo_plugin_config(&plugins.mono_to_stereo)?;
        plugin_configs.push(plugin);
        log::info!("Enabled mono-to-stereo plugin");
        2 // mono_to_stereo outputs stereo
    } else {
        audio_info.spec.channels as usize
    };

    // 2. Upmixer (if enabled)
    let output_channels = if plugins.upmixer.enabled {
        if output_channels != 2 {
            return Err(format!(
                "Upmixer requires stereo input, got {} channels",
                output_channels
            ));
        }

        let output_channel_count = get_speaker_config_channels(&plugins.upmixer.config)?;

        log::info!(
            "Enabling stereo-to-{} upmixer plugin",
            plugins.upmixer.config
        );

        let upmixer_plugin = create_upmixer_plugin_config(&plugins.upmixer)?;
        plugin_configs.push(upmixer_plugin);
        output_channel_count
    } else {
        output_channels
    };

    // 3. Binaural decoder (if enabled, must come after upmixer)
    let output_channels = if plugins.binaural.enabled {
        let binaural_plugin =
            create_binaural_decoder_plugin_config(&plugins.binaural, output_channels)?;
        plugin_configs.push(binaural_plugin);
        log::info!(
            "Enabled binaural decoder plugin: {}ch -> 2ch",
            output_channels
        );
        2
    } else {
        output_channels
    };

    // 4. Loudness compensation
    if let Some(lc) = loudness {
        let lc_plugin = create_loudness_compensation_plugin_config(lc)?;
        plugin_configs.push(lc_plugin);
        log::debug!("Added loudness compensation plugin");
    }

    // 5. Fletcher-Munson
    if plugins.fletcher_munson.enabled {
        let fm_plugin = create_fletcher_munson_plugin_config(&plugins.fletcher_munson)?;
        plugin_configs.push(fm_plugin);
        log::info!("Enabled Fletcher-Munson loudness compensation");
    }

    // 6. EQ filters
    if !filters.is_empty() {
        let eq_plugin = create_eq_plugin_config(filters)?;
        plugin_configs.push(eq_plugin);
        log::debug!("Added EQ plugin with {} filters", filters.len());
    }

    // 7. Gain
    if plugins.gain.enabled {
        let gain_plugin = create_gain_plugin_config(&plugins.gain)?;
        plugin_configs.push(gain_plugin);
        log::info!("Enabled gain plugin ({:.1} dB)", plugins.gain.gain_db);
    }

    // 8. Compressor
    if plugins.compressor.enabled {
        let comp_plugin = create_compressor_plugin_config(&plugins.compressor)?;
        plugin_configs.push(comp_plugin);
        log::info!(
            "Enabled compressor plugin (threshold={:.1}dB, ratio={:.1})",
            plugins.compressor.threshold_db,
            plugins.compressor.ratio
        );
    }

    // 9. Gate
    if plugins.gate.enabled {
        let gate_plugin = create_gate_plugin_config(&plugins.gate)?;
        plugin_configs.push(gate_plugin);
        log::info!(
            "Enabled gate plugin (threshold={:.1}dB)",
            plugins.gate.threshold_db
        );
    }

    // 10. Expander
    if plugins.expander.enabled {
        let expander_plugin = create_expander_plugin_config(&plugins.expander)?;
        plugin_configs.push(expander_plugin);
        log::info!(
            "Enabled expander plugin (threshold={:.1}dB)",
            plugins.expander.threshold_db
        );
    }

    // 11. Limiter
    if plugins.limiter.enabled {
        let limiter_plugin = create_limiter_plugin_config(&plugins.limiter)?;
        plugin_configs.push(limiter_plugin);
        log::info!(
            "Enabled limiter plugin (threshold={:.1}dB)",
            plugins.limiter.threshold_db
        );
    }

    // 12. Multiband Compressor
    if plugins.multiband_compressor.enabled {
        let mb_comp_plugin =
            create_multiband_compressor_plugin_config(&plugins.multiband_compressor)?;
        plugin_configs.push(mb_comp_plugin);
        log::info!("Enabled multiband compressor plugin");
    }

    // 13. Multiband Expander
    if plugins.multiband_expander.enabled {
        let mb_exp_plugin = create_multiband_expander_plugin_config(&plugins.multiband_expander)?;
        plugin_configs.push(mb_exp_plugin);
        log::info!("Enabled multiband expander plugin");
    }

    // 14. Band Split
    if plugins.band_split.enabled {
        let bs_plugin = create_band_split_plugin_config(&plugins.band_split)?;
        plugin_configs.push(bs_plugin);
        log::info!("Enabled band-split plugin");
    }

    // 15. Band Merge
    if plugins.band_merge.enabled {
        let bm_plugin = create_band_merge_plugin_config(&plugins.band_merge)?;
        plugin_configs.push(bm_plugin);
        log::info!("Enabled band-merge plugin");
    }

    // 16. A/B Compare
    if plugins.ab_compare.enabled {
        let ab_plugin = create_ab_compare_plugin_config()?;
        plugin_configs.push(ab_plugin);
        log::info!("Enabled A/B compare plugin");
    }

    // 17. XTC
    if plugins.xtc.enabled {
        let xtc_plugin = create_xtc_plugin_config(&plugins.xtc)?;
        plugin_configs.push(xtc_plugin);
        log::info!("Enabled XTC (crosstalk cancellation) plugin");
    }

    // 18. Denoiser
    if plugins.denoiser.enabled {
        let denoiser_plugin = create_denoiser_plugin_config(&plugins.denoiser)?;
        plugin_configs.push(denoiser_plugin);
        log::info!(
            "Enabled denoiser plugin (reduction={:.1}dB)",
            plugins.denoiser.reduction_db
        );
    }

    // 19. PND
    if plugins.pnd.enabled {
        let pnd_plugin = create_pnd_plugin_config(&plugins.pnd)?;
        plugin_configs.push(pnd_plugin);
        log::info!(
            "Enabled PND varispeed plugin (strength={:.2})",
            plugins.pnd.correction_strength
        );
    }

    // 20. Convolution
    if plugins.convolution.enabled {
        let conv_plugin = create_convolution_plugin_config(&plugins.convolution)?;
        plugin_configs.push(conv_plugin);
        log::info!("Enabled convolution plugin");
    }

    // 21. Downmix
    let output_channels = if plugins.downmix.enabled {
        let downmix_plugin = create_downmix_plugin_config(&plugins.downmix, output_channels)?;
        plugin_configs.push(downmix_plugin);
        log::info!("Enabled downmix plugin ({}ch -> 2ch)", output_channels);
        2
    } else {
        output_channels
    };

    // 22. Channel mapping to hardware (last processing plugin)
    let output_channels = if let Some(mapping_str) = hwaudio_play {
        let (input_channel_map, output_channel_map, matrix) = parse_channel_mapping(mapping_str)?;

        if input_channel_map.len() != output_channels {
            return Err(format!(
                "Channel mapping input mismatch: mapping expects {} channels but plugin chain outputs {}",
                input_channel_map.len(),
                output_channels
            ));
        }

        let max_hw_ch = output_channel_map.iter().max().map(|&v| v + 1).unwrap_or(0);
        let logical_output_channels = output_channel_map.len();

        log::info!("\nChannel mapping enabled:");
        log::info!("  Mapping: {}", mapping_str);
        log::info!("  Logical input channels: {}", input_channel_map.len());
        log::info!("  Logical output channels: {}", logical_output_channels);

        let matrix_plugin =
            create_matrix_plugin_config(input_channel_map, output_channel_map, matrix)?;
        plugin_configs.push(matrix_plugin);

        max_hw_ch
    } else {
        output_channels
    };

    // 23. Channel Mute/Solo (analyzer, late)
    if plugins.channel_mute_solo.enabled {
        let cms_plugin = create_channel_mute_solo_plugin_config()?;
        plugin_configs.push(cms_plugin);
        log::info!("Enabled channel mute/solo plugin");
    }

    // 24. Spectrum Analyzer (analyzer, last)
    if plugins.spectrum_analyzer.enabled {
        let sa_plugin = create_spectrum_analyzer_plugin_config()?;
        plugin_configs.push(sa_plugin);
        log::info!("Enabled spectrum analyzer plugin");
    }

    // 25. Loudness monitor (analyzer, last)
    let loudness_plugin_index = if lufs {
        let analyzer_plugin = create_loudness_analyzer_plugin_config()?;
        let plugin_index = plugin_configs.len();
        plugin_configs.push(analyzer_plugin);
        log::info!(
            "Real-time LUFS monitoring enabled (plugin index: {})",
            plugin_index
        );
        Some(plugin_index)
    } else {
        None
    };

    Ok((plugin_configs, output_channels, loudness_plugin_index))
}
