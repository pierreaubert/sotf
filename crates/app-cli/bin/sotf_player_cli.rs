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

/// Create upmixer PluginConfig from parameters
fn create_upmixer_plugin_config(
    speaker_config: String,
    fft_size: usize,
    gain_front_direct: f32,
    gain_front_ambient: f32,
    gain_rear_ambient: f32,
    lfe_cutoff_hz: f32,
    stereo_width: f32,
    bandpass_hz: f32,
    height_gain: f32,
    lfe_gain: f32,
    enable_subharmonic_synth: bool,
    subharmonic_gain: f32,
    enable_hr_direct: bool,
    hr_sharpen: f32,
    safety_cap_db: f32,
) -> Result<PluginConfig, String> {
    use serde_json::json;

    // Validate FFT size
    if !fft_size.is_power_of_two() {
        return Err(format!(
            "Upmixer FFT size must be power of 2, got {}",
            fft_size
        ));
    }

    // Validate speaker configuration
    let _ = get_speaker_config_channels(&speaker_config)?;

    let parameters = json!({
        "speaker_config": speaker_config,
        "fft_size": fft_size,
        "gain_front_direct": gain_front_direct,
        "gain_front_ambient": gain_front_ambient,
        "gain_rear_ambient": gain_rear_ambient,
        "lfe_cutoff_hz": lfe_cutoff_hz,
        "stereo_width": stereo_width,
        "bandpass_hz": bandpass_hz,
        "height_gain": height_gain,
        "lfe_gain": lfe_gain,
        "enable_subharmonic_synth": enable_subharmonic_synth,
        "subharmonic_gain": subharmonic_gain,
        "enable_hr_direct": enable_hr_direct,
        "hr_sharpen": hr_sharpen,
        "safety_cap_db": safety_cap_db,
    });

    Ok(PluginConfig {
        plugin_type: "upmixer".to_string(),
        parameters,
    })
}

/// Convert loudness compensation to PluginConfig
fn create_loudness_compensation_plugin_config(
    lc: &LoudnessCompensation,
) -> Result<PluginConfig, String> {
    use serde_json::json;

    // Map from LoudnessCompensation fields to plugin parameters
    // reference_level and attenuate_mid are not used by the plugin
    // The plugin uses fixed frequencies (100Hz low, 10kHz high)
    let parameters = json!({
        "low_freq": 100.0,  // Fixed low-shelf frequency
        "low_gain": lc.low_boost,
        "high_freq": 10000.0,  // Fixed high-shelf frequency
        "high_gain": lc.high_boost,
    });

    Ok(PluginConfig {
        plugin_type: "loudness_compensation".to_string(),
        parameters,
    })
}

/// Create binaural decoder PluginConfig from parameters
fn create_binaural_decoder_plugin_config(
    sofa_file: PathBuf,
    input_channels: usize,
    fft_size: usize,
    enable_optimization: bool,
    externalization: f32,
    near_field_strength: f32,
) -> Result<PluginConfig, String> {
    use serde_json::json;

    // Validate FFT size
    if !fft_size.is_power_of_two() {
        return Err(format!(
            "Binaural decoder FFT size must be power of 2, got {}",
            fft_size
        ));
    }

    // Validate that SOFA file exists
    if !sofa_file.exists() {
        return Err(format!("SOFA file does not exist: {:?}", sofa_file));
    }

    let parameters = json!({
        "sofa_file": sofa_file.to_string_lossy().to_string(),
        "input_channels": input_channels,
        "fft_size": fft_size,
        "enable_optimization": enable_optimization,
        "externalization": externalization,
        "near_field_strength": near_field_strength,
    });

    Ok(PluginConfig {
        plugin_type: "binaural_decoder".to_string(),
        parameters,
    })
}

/// Create loudness analyzer PluginConfig
fn create_loudness_analyzer_plugin_config() -> Result<PluginConfig, String> {
    use serde_json::json;

    let parameters = json!({});

    Ok(PluginConfig {
        plugin_type: "loudness_monitor".to_string(),
        parameters,
    })
}

/// Create expander PluginConfig with default parameters
fn create_expander_plugin_config() -> Result<PluginConfig, String> {
    use serde_json::json;
    use sotf_plugins::param_specs::expander;

    let parameters = json!({
        "threshold_db": expander::THRESHOLD_DEFAULT,
        "ratio": expander::RATIO_DEFAULT,
        "attack_ms": expander::ATTACK_DEFAULT,
        "release_ms": expander::RELEASE_DEFAULT,
        "range_db": expander::RANGE_DEFAULT,
        "knee_db": expander::KNEE_DEFAULT,
        "hysteresis_db": expander::HYSTERESIS_DEFAULT,
        "hold_ms": expander::HOLD_DEFAULT,
        "mix": expander::MIX_DEFAULT,
        "link_channels": expander::LINK_CHANNELS_DEFAULT,
        "sidechain_hpf_hz": expander::SIDECHAIN_HPF_HZ_DEFAULT,
    });

    Ok(PluginConfig {
        plugin_type: "expander".to_string(),
        parameters,
    })
}

/// Create multiband compressor PluginConfig with default parameters
fn create_multiband_compressor_plugin_config() -> Result<PluginConfig, String> {
    use serde_json::json;
    use sotf_plugins::param_specs::multiband_compressor;

    let parameters = json!({
        "num_bands": multiband_compressor::NUM_BANDS_DEFAULT,
        "crossover_preset": multiband_compressor::CROSSOVER_PRESET_DEFAULT,
        "crossover_freq_1": multiband_compressor::CROSSOVER_FREQ_1_DEFAULT,
        "crossover_freq_2": multiband_compressor::CROSSOVER_FREQ_2_DEFAULT,
        "crossover_freq_3": multiband_compressor::CROSSOVER_FREQ_3_DEFAULT,
        "crossover_freq_4": multiband_compressor::CROSSOVER_FREQ_4_DEFAULT,
        "threshold_db": multiband_compressor::THRESHOLD_DEFAULT,
        "ratio": multiband_compressor::RATIO_DEFAULT,
        "attack_ms": multiband_compressor::ATTACK_DEFAULT,
        "release_ms": multiband_compressor::RELEASE_DEFAULT,
        "knee_db": multiband_compressor::KNEE_DEFAULT,
        "mix": multiband_compressor::MIX_DEFAULT,
        "link_channels": multiband_compressor::LINK_CHANNELS_DEFAULT,
    });

    Ok(PluginConfig {
        plugin_type: "multiband_compressor".to_string(),
        parameters,
    })
}

/// Create multiband expander PluginConfig with default parameters
fn create_multiband_expander_plugin_config() -> Result<PluginConfig, String> {
    use serde_json::json;
    use sotf_plugins::param_specs::multiband_expander;

    let parameters = json!({
        "num_bands": multiband_expander::NUM_BANDS_DEFAULT,
        "crossover_preset": multiband_expander::CROSSOVER_PRESET_DEFAULT,
        "crossover_freq_1": multiband_expander::CROSSOVER_FREQ_1_DEFAULT,
        "crossover_freq_2": multiband_expander::CROSSOVER_FREQ_2_DEFAULT,
        "crossover_freq_3": multiband_expander::CROSSOVER_FREQ_3_DEFAULT,
        "crossover_freq_4": multiband_expander::CROSSOVER_FREQ_4_DEFAULT,
        "threshold_db": multiband_expander::THRESHOLD_DEFAULT,
        "ratio": multiband_expander::RATIO_DEFAULT,
        "attack_ms": multiband_expander::ATTACK_DEFAULT,
        "release_ms": multiband_expander::RELEASE_DEFAULT,
        "range_db": multiband_expander::RANGE_DEFAULT,
        "knee_db": multiband_expander::KNEE_DEFAULT,
        "hysteresis_db": multiband_expander::HYSTERESIS_DEFAULT,
        "hold_ms": multiband_expander::HOLD_DEFAULT,
        "mix": multiband_expander::MIX_DEFAULT,
        "link_channels": multiband_expander::LINK_CHANNELS_DEFAULT,
    });

    Ok(PluginConfig {
        plugin_type: "multiband_expander".to_string(),
        parameters,
    })
}

/// Create XTC (Crosstalk Cancellation) PluginConfig with default parameters
fn create_xtc_plugin_config() -> Result<PluginConfig, String> {
    use serde_json::json;

    let parameters = json!({
        "distance_m": 2.0,
        "speaker_angle_deg": 30.0,
        "head_radius_m": 0.0875,
        "beta_base": 0.001,
        "beta_low_freq_boost": 10.0,
        "beta_high_freq_boost": 10.0,
        "head_shadow_cutoff_hz": 4000.0,
        "head_shadow_slope_db_per_octave": 6.0,
    });

    Ok(PluginConfig {
        plugin_type: "xtc".to_string(),
        parameters,
    })
}

/// Create denoiser plugin config
fn create_denoiser_plugin_config(
    reduction_db: f32,
    floor_db: f32,
    smoothing: f32,
    attack_ms: f32,
    release_ms: f32,
    low_latency: bool,
) -> Result<PluginConfig, String> {
    use serde_json::json;

    let parameters = json!({
        "reduction_db": reduction_db,
        "floor_db": floor_db,
        "smoothing": smoothing,
        "attack_ms": attack_ms,
        "release_ms": release_ms,
        "low_latency": low_latency,
    });

    Ok(PluginConfig {
        plugin_type: "denoiser".to_string(),
        parameters,
    })
}

/// Create PND (Polyphonic Note Detection) varispeed plugin config
fn create_pnd_plugin_config(
    correction_strength: f32,
    analysis_window_ms: f32,
    drift_smoothing: f32,
) -> Result<PluginConfig, String> {
    use serde_json::json;

    let parameters = json!({
        "correction_strength": correction_strength,
        "analysis_window_ms": analysis_window_ms,
        "drift_smoothing": drift_smoothing,
    });

    Ok(PluginConfig {
        plugin_type: "pnd".to_string(),
        parameters,
    })
}

/// Convert Biquad filters to PluginConfig for EQ plugin
fn create_eq_plugin_config(filters: &[Biquad]) -> Result<PluginConfig, String> {
    use serde_json::json;

    // Convert Biquad to BiquadFilterConfig format
    let filter_configs: Result<Vec<_>, String> = filters
        .iter()
        .map(|f| {
            // Use long_name() from BiquadFilterType
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

        /// Enable stereo-to-surround upmixer (converts 2ch to multi-channel surround)
        #[arg(long = "upmixer", default_value_t = false)]
        upmixer: bool,

        /// Upmixer speaker configuration (2.0, 5.0, 5.1, 7.1, 5.1.2, 5.1.4, 7.1.2, 7.1.4, 9.1.4, 9.1.6)
        #[arg(long = "upmixer-config", default_value = "5.1")]
        upmixer_config: String,

        /// Upmixer FFT size (must be power of 2: 1024, 2048, 4096)
        #[arg(long = "upmixer-fft-size", default_value = "2048")]
        upmixer_fft_size: usize,

        /// Upmixer front direct gain (0.0-2.0)
        #[arg(long = "upmixer-gain-front-direct", default_value = "1.0")]
        upmixer_gain_front_direct: f32,

        /// Upmixer front ambient gain (0.0-2.0)
        #[arg(long = "upmixer-gain-front-ambient", default_value = "0.5")]
        upmixer_gain_front_ambient: f32,

        /// Upmixer rear ambient gain (0.0-2.0)
        #[arg(long = "upmixer-gain-rear-ambient", default_value = "1.0")]
        upmixer_gain_rear_ambient: f32,

        /// Upmixer LFE cutoff frequency (Hz, 40-200)
        #[arg(long = "upmixer-lfe-cutoff", default_value = "120.0")]
        upmixer_lfe_cutoff_hz: f32,

        /// Upmixer stereo width (0.0-1.0)
        #[arg(long = "upmixer-stereo-width", default_value = "0.5")]
        upmixer_stereo_width: f32,

        /// Upmixer bandpass / upmix crossover frequency (Hz)
        #[arg(long = "upmixer-bandpass", default_value = "250.0")]
        upmixer_bandpass_hz: f32,

        /// Upmixer height gain (0.0-2.0)
        #[arg(long = "upmixer-height-gain", default_value = "1.0")]
        upmixer_height_gain: f32,

        /// Upmixer LFE gain (0.0-2.0)
        #[arg(long = "upmixer-lfe-gain", default_value = "1.0")]
        upmixer_lfe_gain: f32,

        /// Enable Upmixer Sub-Harmonic Synthesizer (adds low-end impact)
        #[arg(long = "upmixer-subharmonic", default_value_t = false)]
        upmixer_subharmonic: bool,

        /// Upmixer Sub-Harmonic Synthesizer gain (0.0-1.0)
        #[arg(long = "upmixer-subharmonic-gain", default_value = "0.5")]
        upmixer_subharmonic_gain: f32,

        /// Enable Upmixer high-resolution direct path
        #[arg(long = "upmixer-hr-direct", default_value_t = false)]
        upmixer_hr_direct: bool,

        /// Upmixer HR Sharpen depth (0.0-1.0)
        #[arg(long = "upmixer-hr-sharpen", default_value = "1.0")]
        upmixer_hr_sharpen: f32,

        /// Upmixer safety cap in dB (0.0-12.0, 3.0 = default safety)
        #[arg(long = "upmixer-safety-cap-db", default_value = "3.0")]
        upmixer_safety_cap_db: f32,

        /// Enable binaural decoder (converts multi-channel to binaural stereo using HRTFs)
        #[arg(long = "binaural", default_value_t = false)]
        binaural: bool,

        /// Path to SOFA file for binaural decoder (required when --binaural is enabled)
        #[arg(long = "sofa-file")]
        sofa_file: Option<PathBuf>,

        /// Binaural decoder FFT size (must be power of 2: 2048, 4096, 8192)
        #[arg(long = "binaural-fft-size", default_value = "4096")]
        binaural_fft_size: usize,

        /// Enable Binaural Decoder Sum-Before-IFFT optimization
        #[arg(long = "binaural-optimization", default_value_t = true)]
        binaural_optimization: bool,

        /// Binaural Decoder Externalization (0.0-1.0)
        #[arg(long = "binaural-externalization", default_value = "0.0")]
        binaural_externalization: f32,

        /// Binaural Decoder Near-Field Strength (0.0-1.0)
        #[arg(long = "binaural-near-field", default_value = "0.0")]
        binaural_near_field: f32,

        /// Enable expander plugin (dynamic range expansion with hysteresis)
        #[arg(long = "expander", default_value_t = false)]
        expander: bool,

        /// Enable multiband compressor plugin (3-band compression with crossovers)
        #[arg(long = "multiband-compressor", default_value_t = false)]
        multiband_compressor: bool,

        /// Enable multiband expander plugin (3-band expansion with crossovers)
        #[arg(long = "multiband-expander", default_value_t = false)]
        multiband_expander: bool,

        /// Enable XTC (crosstalk cancellation) plugin for speaker playback
        #[arg(long = "xtc", default_value_t = false)]
        xtc: bool,

        /// Enable denoiser plugin (Wiener filter with MCRA noise estimation)
        #[arg(long = "denoiser", default_value_t = false)]
        denoiser: bool,

        /// Denoiser noise reduction strength (0-40 dB)
        #[arg(long = "denoiser-reduction-db", default_value = "12.0")]
        denoiser_reduction_db: f32,

        /// Denoiser floor/minimum gain (-60 to -10 dB, prevents musical noise)
        #[arg(long = "denoiser-floor-db", default_value = "-30.0")]
        denoiser_floor_db: f32,

        /// Denoiser temporal smoothing (0.0-0.99)
        #[arg(long = "denoiser-smoothing", default_value = "0.8")]
        denoiser_smoothing: f32,

        /// Denoiser attack time (ms)
        #[arg(long = "denoiser-attack-ms", default_value = "5.0")]
        denoiser_attack_ms: f32,

        /// Denoiser release time (ms)
        #[arg(long = "denoiser-release-ms", default_value = "50.0")]
        denoiser_release_ms: f32,

        /// Enable low-latency mode for denoiser (512 FFT vs 2048)
        #[arg(long = "denoiser-low-latency", default_value_t = false)]
        denoiser_low_latency: bool,

        /// Enable PND (Polyphonic Note Detection) varispeed correction plugin
        #[arg(long = "pnd", default_value_t = false)]
        pnd: bool,

        /// PND correction strength (0.0-2.0, 1.0 = full correction)
        #[arg(long = "pnd-correction-strength", default_value = "1.0")]
        pnd_correction_strength: f32,

        /// PND analysis window size in milliseconds (20-500)
        #[arg(long = "pnd-analysis-window-ms", default_value = "100.0")]
        pnd_analysis_window_ms: f32,

        /// PND drift smoothing factor (0.001-1.0)
        #[arg(long = "pnd-drift-smoothing", default_value = "0.1")]
        pnd_drift_smoothing: f32,

        /// Use rack mode with specified plugin order (matches GPUI app plugin behavior)
        ///
        /// Available plugins: eq, upmixer, binaural, loudness, expander, compressor,
        /// mb-expander, xtc, denoiser, pnd, lufs
        ///
        /// Example: --rack upmixer eq lufs
        #[arg(long = "rack", value_name = "PLUGIN", num_args = 1..)]
        rack: Vec<String>,
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
        // Log all modules including Symphonia at debug level
        .filter_module("symphonia_core", log::LevelFilter::Debug)
        .init();

    log::info!("SOTF CLI Player starting...");

    // Run pre-flight checks before initializing the player
    // Skip for non-playback commands (devices, replay-gain, status)
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
                log::info!("ReplayGain analysis:");
                log::info!("  File: {:?}", file);
                log::info!("  Gain: {:+.2} dB", info.gain);
                log::info!("  Peak: {:.6}", info.peak);
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
            upmixer,
            upmixer_config,
            upmixer_fft_size,
            upmixer_gain_front_direct,
            upmixer_gain_front_ambient,
            upmixer_gain_rear_ambient,
            upmixer_lfe_cutoff_hz,
            upmixer_stereo_width,
            upmixer_bandpass_hz,
            upmixer_height_gain,
            upmixer_lfe_gain,
            upmixer_subharmonic,
            upmixer_subharmonic_gain,
            upmixer_hr_direct,
            upmixer_hr_sharpen,
            upmixer_safety_cap_db,
            binaural,
            sofa_file,
            binaural_fft_size,
            binaural_optimization,
            binaural_externalization,
            binaural_near_field,
            expander,
            multiband_compressor,
            multiband_expander,
            xtc,
            denoiser,
            denoiser_reduction_db,
            denoiser_floor_db,
            denoiser_smoothing,
            denoiser_attack_ms,
            denoiser_release_ms,
            denoiser_low_latency,
            pnd,
            pnd_correction_strength,
            pnd_analysis_window_ms,
            pnd_drift_smoothing,
            rack,
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
                upmixer,
                upmixer_config,
                upmixer_fft_size,
                upmixer_gain_front_direct,
                upmixer_gain_front_ambient,
                upmixer_gain_rear_ambient,
                upmixer_lfe_cutoff_hz,
                upmixer_stereo_width,
                upmixer_bandpass_hz,
                upmixer_height_gain,
                upmixer_lfe_gain,
                upmixer_subharmonic,
                upmixer_subharmonic_gain,
                upmixer_hr_direct,
                upmixer_hr_sharpen,
                upmixer_safety_cap_db,
                binaural,
                sofa_file,
                binaural_fft_size,
                binaural_optimization,
                binaural_externalization,
                binaural_near_field,
                expander,
                multiband_compressor,
                multiband_expander,
                xtc,
                denoiser,
                denoiser_reduction_db,
                denoiser_floor_db,
                denoiser_smoothing,
                denoiser_attack_ms,
                denoiser_release_ms,
                denoiser_low_latency,
                pnd,
                pnd_correction_strength,
                pnd_analysis_window_ms,
                pnd_drift_smoothing,
                rack,
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
    log::info!("Enumerating audio devices...\n");

    let devices = sotf_audio::devices::get_audio_devices()
        .map_err(|e| format!("Failed to get devices: {}", e))?;

    // Print input devices
    if let Some(input_devices) = devices.get("input") {
        log::info!("Input Devices:");
        log::info!("{}", "=".repeat(80));
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

                log::info!(
                    "  [{}] {}{} - {} ch, {} (current: {} Hz), {}",
                    idx + 1,
                    device.name,
                    default_marker,
                    config.channels,
                    rate_range,
                    config.sample_rate,
                    config.sample_format
                );
            } else {
                log::info!("  [{}] {}{}", idx + 1, device.name, default_marker);
            }
        }
        log::info!("");
    }

    // Print output devices
    if let Some(output_devices) = devices.get("output") {
        log::info!("Output Devices:");
        log::info!("{}", "=".repeat(80));
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

                log::info!(
                    "  [{}] {}{} - {} ch, {} (current: {} Hz), {}",
                    idx + 1,
                    device.name,
                    default_marker,
                    config.channels,
                    rate_range,
                    config.sample_rate,
                    config.sample_format
                );
            } else {
                log::info!("  [{}] {}{}", idx + 1, device.name, default_marker);
            }
        }
        log::info!("");
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

            // Support both formats:
            // - 3 parts: freq:q:gain (defaults to Peak)
            // - 4 parts: type:freq:q:gain
            let (filter_type, frequency, q, gain) = match parts.len() {
                3 => {
                    // Format: freq:q:gain (default to Peak)
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
                    // Format: type:freq:q:gain
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

            // Validate ranges
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

            // Use placeholder sample rate - will be updated by EqPlugin::initialize()
            Ok(Biquad::new(filter_type, frequency, 48000.0, q, gain))
        })
        .collect()
}

/// Parse channel mapping specification and create matrix plugin config
///
/// Format: "in1,in2,...->out1,out2,..." where channels are 1-indexed
/// Use "_" in output to skip a channel position
///
/// Examples:
///   "1,2->9,10"                 - Route stereo to HW channels 9,10
///   "1,2,3,4,5->1,2,3,_,5,6"    - Route 5ch with gap (skip position 4)
///   "1,2,3,4,5,6->13,14,15,16,17,18"  - Route 5.1 to channels 13-18
///
/// Returns: (input_channel_map, output_channel_map, matrix)
fn parse_channel_mapping(mapping_str: &str) -> Result<(Vec<usize>, Vec<usize>, Vec<f32>), String> {
    let parts: Vec<&str> = mapping_str.split("->").collect();
    if parts.len() != 2 {
        return Err(format!(
            "Invalid mapping format '{}'. Expected 'in1,in2,...->out1,out2,...'",
            mapping_str
        ));
    }

    // Parse input channels (1-indexed)
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

    // Parse output channel mapping (1-indexed, with "_" for gaps)
    let output_spec: Vec<&str> = parts[1].split(',').map(|s| s.trim()).collect();
    if output_spec.is_empty() {
        return Err("No output channels specified".to_string());
    }

    // Build mapping: input_ch_idx -> output_hw_ch (0-indexed internally, but 1-indexed in spec)
    let mut channel_map: Vec<Option<usize>> = Vec::new();
    let mut max_hw_channel = 0;

    for spec in output_spec.iter() {
        if *spec == "_" {
            channel_map.push(None); // Gap/skip
        } else {
            let hw_ch = spec
                .parse::<usize>()
                .map_err(|_| format!("Invalid output channel: '{}'", spec))?;
            if hw_ch == 0 {
                return Err("Channel indices must be >= 1 (1-indexed)".to_string());
            }
            channel_map.push(Some(hw_ch - 1)); // Convert to 0-indexed
            max_hw_channel = max_hw_channel.max(hw_ch);
        }
    }

    // Check that we have enough output specs for input channels
    let non_gap_outputs: Vec<_> = channel_map.iter().filter_map(|&x| x).collect();
    if non_gap_outputs.len() != input_channels.len() {
        return Err(format!(
            "Mismatch: {} input channels but {} non-gap output positions",
            input_channels.len(),
            non_gap_outputs.len()
        ));
    }

    // Build sparse channel mapping
    // For "1,2->15,16":
    //   input_channel_map = [0, 1] (read from logical channels 0,1)
    //   output_channel_map = [14, 15] (write to physical channels 14,15)
    //   matrix = 2x2 identity

    // Convert input channels from 1-indexed to 0-indexed
    let input_channel_map: Vec<usize> = input_channels.iter().map(|&ch| ch - 1).collect();

    // Extract non-gap output channels (already 0-indexed from parsing)
    let output_channel_map: Vec<usize> = channel_map.iter().filter_map(|&x| x).collect();

    let input_count = input_channel_map.len();
    let output_count = output_channel_map.len();

    // Create identity matrix for sparse mapping (logical channels map 1:1)
    // Matrix is row-major: matrix[out_ch * input_count + in_ch]
    let mut matrix = vec![0.0f32; output_count * input_count];
    for i in 0..output_count.min(input_count) {
        matrix[i * input_count + i] = 1.0;
    }

    Ok((input_channel_map, output_channel_map, matrix))
}

/// Create matrix plugin config from parsed mapping
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
    upmixer: bool,
    upmixer_config: String,
    upmixer_fft_size: usize,
    upmixer_gain_front_direct: f32,
    upmixer_gain_front_ambient: f32,
    upmixer_gain_rear_ambient: f32,
    upmixer_lfe_cutoff_hz: f32,
    upmixer_stereo_width: f32,
    upmixer_bandpass_hz: f32,
    upmixer_height_gain: f32,
    upmixer_lfe_gain: f32,
    enable_subharmonic_synth: bool,
    subharmonic_gain: f32,
    enable_hr_direct: bool,
    hr_sharpen: f32,
    safety_cap_db: f32,
    binaural: bool,
    sofa_file: Option<PathBuf>,
    binaural_fft_size: usize,
    enable_optimization: bool,
    externalization: f32,
    near_field_strength: f32,
    expander: bool,
    multiband_compressor: bool,
    multiband_expander: bool,
    xtc: bool,
    denoiser: bool,
    denoiser_reduction_db: f32,
    denoiser_floor_db: f32,
    denoiser_smoothing: f32,
    denoiser_attack_ms: f32,
    denoiser_release_ms: f32,
    denoiser_low_latency: bool,
    pnd: bool,
    pnd_correction_strength: f32,
    pnd_analysis_window_ms: f32,
    pnd_drift_smoothing: f32,
    rack: Vec<String>,
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

    // Create streaming manager with signal watching enabled (manager handles Ctrl+C)
    let mut streaming_manager = AudioEngineManager::with_signal_watching(true);

    // Load the audio file
    let audio_info = streaming_manager
        .load_file(&file)
        .map_err(|e| format!("Failed to load audio file: {}", e))?;

    log::info!("Loaded audio file:");
    log::info!("  Format: {}", audio_info.format);
    log::info!("  Sample rate: {}Hz", audio_info.spec.sample_rate);
    log::info!("  Channels: {}", audio_info.spec.channels);
    log::info!("  Bits per sample: {}", audio_info.spec.bits_per_sample);
    if let Some(duration_secs) = audio_info.duration_seconds {
        log::info!("  Duration: {:.2}s", duration_secs);
    }
    log::info!("");

    // Build plugin chain
    let (plugins, output_channels, loudness_plugin_index) = if !rack.is_empty() {
        // Use PluginChain with specified plugin order (matches GPUI app behavior)
        log::info!("Using rack mode with plugins: {:?}", rack);
        let sample_rate = audio_info.spec.sample_rate as f64;
        let mut chain = PluginChain::new();
        let mut has_lufs = false;

        // Add plugins in the order specified by --rack
        for plugin_name in &rack {
            match plugin_name.to_lowercase().as_str() {
                "upmixer" => {
                    // Check that input is stereo
                    if audio_info.spec.channels != 2 {
                        return Err(format!(
                            "Upmixer requires stereo input, got {} channels",
                            audio_info.spec.channels
                        ));
                    }

                    let idx = chain.add_plugin(&PluginType::Upmixer);
                    if let Some(plugin) = chain.get_plugin_mut(idx) {
                        plugin.settings = PluginSettings::Upmixer {
                            speaker_config: upmixer_config.clone(),
                            gain_front_direct: upmixer_gain_front_direct as f64,
                            gain_front_ambient: upmixer_gain_front_ambient as f64,
                            gain_rear_ambient: upmixer_gain_rear_ambient as f64,
                            height_gain: upmixer_height_gain as f64,
                            stereo_width: upmixer_stereo_width as f64,
                            center_spread: 0.5,
                            surround_direct_bleed: 0.3,
                            rear_late_reflection: 0.2,
                            lfe_cutoff_hz: upmixer_lfe_cutoff_hz as f64,
                            lfe_gain: upmixer_lfe_gain as f64,
                            bandpass_hz: upmixer_bandpass_hz as f64,
                            enable_subharmonic_synth,
                            subharmonic_gain: subharmonic_gain as f64,
                            subharmonic_freq_hz: 60.0,
                            subharmonic_attack_ms: 10.0,
                            subharmonic_release_ms: 100.0,
                            decorrelation_mode: 0,
                            decorrelation_lfo_rate_hz: 0.5,
                            velvet_noise_duration_ms: 50.0,
                            velvet_noise_density: 0.5,
                            enable_hr_direct,
                            hr_sharpen: hr_sharpen as f64,
                            height_hf_cap_hz: 8000.0,
                            height_transient_reduction: 0.5,
                            height_direct_leak: 0.1,
                            ambient_boost: 0.0,
                            safety_cap_db: safety_cap_db as f64,
                            rear_ambient_boost: 0.0,
                            dialogue_weight: 1.0,
                            voice_freq_min_hz: 85.0,
                            voice_freq_max_hz: 3000.0,
                            bypass_decorrelation: false,
                            bypass_transient_detection: false,
                            bypass_all_processing: false,
                        };
                    }
                    log::info!("Rack: Added Upmixer plugin ({})", upmixer_config);
                }
                "binaural" => {
                    let sofa_path = sofa_file
                        .clone()
                        .ok_or("Binaural decoder requires --sofa-file to be specified")?;
                    let input_channels = chain.output_channels();

                    let idx = chain.add_plugin(&PluginType::BinauralDecoder);
                    if let Some(plugin) = chain.get_plugin_mut(idx) {
                        plugin.settings = PluginSettings::BinauralDecoder {
                            sofa_file: sofa_path.to_string_lossy().to_string(),
                            input_channels,
                            enable_optimization,
                            externalization: externalization as f64,
                            near_field_strength: near_field_strength as f64,
                        };
                    }
                    log::info!("Rack: Added BinauralDecoder plugin");
                }
                "loudness" | "loudness-compensation" => {
                    chain.add_plugin(&PluginType::LoudnessCompensation);
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
                "expander" => {
                    chain.add_plugin(&PluginType::Expander);
                    log::info!("Rack: Added Expander plugin");
                }
                "compressor" | "mb-compressor" => {
                    chain.add_plugin(&PluginType::MultibandCompressor);
                    log::info!("Rack: Added MultibandCompressor plugin");
                }
                "mb-expander" => {
                    chain.add_plugin(&PluginType::MultibandExpander);
                    log::info!("Rack: Added MultibandExpander plugin");
                }
                "xtc" => {
                    chain.add_plugin(&PluginType::XTC);
                    log::info!("Rack: Added XTC plugin");
                }
                "denoiser" => {
                    let idx = chain.add_plugin(&PluginType::Denoiser);
                    if let Some(plugin) = chain.get_plugin_mut(idx) {
                        plugin.settings = PluginSettings::Denoiser {
                            reduction_db: denoiser_reduction_db as f64,
                            floor_db: denoiser_floor_db as f64,
                            smoothing: denoiser_smoothing as f64,
                            attack_ms: denoiser_attack_ms as f64,
                            release_ms: denoiser_release_ms as f64,
                            low_latency: denoiser_low_latency,
                            polyphonic_detection: false,
                        };
                    }
                    log::info!("Rack: Added Denoiser plugin");
                }
                "pnd" => {
                    let idx = chain.add_plugin(&PluginType::Pnd);
                    if let Some(plugin) = chain.get_plugin_mut(idx) {
                        plugin.settings = PluginSettings::Pnd {
                            correction_strength: pnd_correction_strength as f64,
                            analysis_window_ms: pnd_analysis_window_ms as f64,
                            drift_smoothing: pnd_drift_smoothing as f64,
                        };
                    }
                    log::info!("Rack: Added PND plugin");
                }
                "lufs" | "loudness-monitor" => {
                    chain.add_plugin(&PluginType::LoudnessMonitor);
                    has_lufs = true;
                    log::info!("Rack: Added LoudnessMonitor plugin");
                }
                unknown => {
                    return Err(format!(
                        "Unknown plugin '{}'. Available: eq, upmixer, binaural, loudness, \
                        expander, compressor, mb-expander, xtc, denoiser, pnd, lufs",
                        unknown
                    ));
                }
            }
        }

        // Convert to PluginConfig
        let plugins = chain.to_plugin_configs(sample_rate);
        let output_channels = chain.output_channels();

        // Find the actual index of loudness monitor in the plugins vec
        let actual_loudness_idx = if has_lufs {
            plugins
                .iter()
                .position(|p| p.plugin_type == "loudness_monitor")
        } else {
            None
        };

        (plugins, output_channels, actual_loudness_idx)
    } else {
        // Original plugin creation logic (manual PluginConfig building)
        let mut plugins = Vec::new();

        // Upmixer (if enabled)
        let output_channels = if upmixer {
            // Check that input is stereo
            if audio_info.spec.channels != 2 {
                return Err(format!(
                    "Upmixer requires stereo input, got {} channels",
                    audio_info.spec.channels
                ));
            }

            // Get channel count for the configuration
            let output_channel_count = get_speaker_config_channels(&upmixer_config)?;

            log::info!("Enabling stereo-to-{} upmixer plugin:", upmixer_config);
            log::info!("  Speaker configuration: {}", upmixer_config);
            log::info!("  Output channels: {}", output_channel_count);
            log::info!("  FFT size: {}", upmixer_fft_size);
            log::info!("  Front direct gain: {:.2}", upmixer_gain_front_direct);
            log::info!("  Front ambient gain: {:.2}", upmixer_gain_front_ambient);
            log::info!("  Rear ambient gain: {:.2}", upmixer_gain_rear_ambient);
            log::info!("  LFE cutoff: {:.1} Hz", upmixer_lfe_cutoff_hz);
            log::info!("  Stereo width: {:.2}", upmixer_stereo_width);
            log::info!("  Bandpass: {:.1} Hz", upmixer_bandpass_hz);
            log::info!("  Height gain: {:.2}", upmixer_height_gain);
            log::info!("  LFE gain: {:.2}", upmixer_lfe_gain);
            log::info!(
                "  HR direct: {} (sharpen {:.2}, safety cap {:.1} dB)",
                enable_hr_direct,
                hr_sharpen,
                safety_cap_db
            );
            log::info!("");

            let upmixer_plugin = create_upmixer_plugin_config(
                upmixer_config.clone(),
                upmixer_fft_size,
                upmixer_gain_front_direct,
                upmixer_gain_front_ambient,
                upmixer_gain_rear_ambient,
                upmixer_lfe_cutoff_hz,
                upmixer_stereo_width,
                upmixer_bandpass_hz,
                upmixer_height_gain,
                upmixer_lfe_gain,
                enable_subharmonic_synth,
                subharmonic_gain,
                enable_hr_direct,
                hr_sharpen,
                safety_cap_db,
            )?;
            plugins.push(upmixer_plugin);
            log::debug!(
                "Added upmixer plugin: 2ch -> {}ch ({})",
                output_channel_count,
                upmixer_config
            );
            output_channel_count
        } else {
            audio_info.spec.channels as usize
        };

        // Binaural decoder (if enabled, must come after upmixer)
        let output_channels = if binaural {
            // Validate that SOFA file is provided
            let sofa_path =
                sofa_file.ok_or("Binaural decoder requires --sofa-file to be specified")?;

            // Input channels come from previous plugin (upmixer or original audio)
            let input_channels = output_channels;

            log::info!("Enabling binaural decoder plugin:");
            log::info!("  Input channels: {}", input_channels);
            log::info!("  Output channels: 2 (binaural stereo)");
            log::info!("  SOFA file: {:?}", sofa_path);
            log::info!("  FFT size: {}", binaural_fft_size);
            log::info!("");

            let binaural_plugin = create_binaural_decoder_plugin_config(
                sofa_path,
                input_channels,
                binaural_fft_size,
                enable_optimization,
                externalization,
                near_field_strength,
            )?;
            plugins.push(binaural_plugin);
            log::debug!(
                "Added binaural decoder plugin: {}ch -> 2ch (binaural)",
                input_channels
            );

            2 // Binaural always outputs stereo
        } else {
            output_channels
        };

        // Loudness compensation (before channel mapping)
        if let Some(ref lc) = loudness {
            let lc_plugin = create_loudness_compensation_plugin_config(lc)?;
            plugins.push(lc_plugin);
            log::debug!("Added loudness compensation plugin");
        }

        // EQ filters (assuming it is room eq)
        if !filters.is_empty() {
            let eq_plugin = create_eq_plugin_config(&filters)?;
            plugins.push(eq_plugin);
            log::debug!("Added EQ plugin with {} filters", filters.len());
        }

        // Dynamics plugins (expander, compressor, etc.)
        if expander {
            let expander_plugin = create_expander_plugin_config()?;
            plugins.push(expander_plugin);
            log::info!("Enabled expander plugin (default parameters)");
        }

        if multiband_compressor {
            let mb_comp_plugin = create_multiband_compressor_plugin_config()?;
            plugins.push(mb_comp_plugin);
            log::info!("Enabled multiband compressor plugin (3-band, default parameters)");
        }

        if multiband_expander {
            let mb_exp_plugin = create_multiband_expander_plugin_config()?;
            plugins.push(mb_exp_plugin);
            log::info!("Enabled multiband expander plugin (3-band, default parameters)");
        }

        // XTC (Crosstalk Cancellation)
        if xtc {
            let xtc_plugin = create_xtc_plugin_config()?;
            plugins.push(xtc_plugin);
            log::info!("Enabled XTC (crosstalk cancellation) plugin");
        }

        // Denoiser
        if denoiser {
            let denoiser_plugin = create_denoiser_plugin_config(
                denoiser_reduction_db,
                denoiser_floor_db,
                denoiser_smoothing,
                denoiser_attack_ms,
                denoiser_release_ms,
                denoiser_low_latency,
            )?;
            plugins.push(denoiser_plugin);
            log::info!(
                "Enabled denoiser plugin (reduction={:.1}dB, floor={:.1}dB, low_latency={})",
                denoiser_reduction_db,
                denoiser_floor_db,
                denoiser_low_latency
            );
        }

        // PND (Polyphonic Note Detection) varispeed
        if pnd {
            let pnd_plugin = create_pnd_plugin_config(
                pnd_correction_strength,
                pnd_analysis_window_ms,
                pnd_drift_smoothing,
            )?;
            plugins.push(pnd_plugin);
            log::info!(
                "Enabled PND varispeed plugin (strength={:.2}, window={:.1}ms, smoothing={:.3})",
                pnd_correction_strength,
                pnd_analysis_window_ms,
                pnd_drift_smoothing
            );
        }

        // 4. Channel mapping to hardware (last plugin before output)
        let output_channels = if let Some(ref mapping_str) = hwaudio_play {
            let (input_channel_map, output_channel_map, matrix) =
                parse_channel_mapping(mapping_str)?;

            // Verify that mapping input matches current output channels
            if input_channel_map.len() != output_channels {
                return Err(format!(
                    "Channel mapping input mismatch: mapping expects {} channels but plugin chain outputs {}",
                    input_channel_map.len(),
                    output_channels
                ));
            }

            // Calculate the actual output channel count
            let max_hw_ch = output_channel_map.iter().max().map(|&v| v + 1).unwrap_or(0);
            let logical_output_channels = output_channel_map.len();

            log::info!("\nChannel mapping enabled:");
            log::info!("  Mapping: {}", mapping_str);
            log::info!("  Logical input channels: {}", input_channel_map.len());
            log::info!("  Logical output channels: {}", logical_output_channels);
            log::info!("  Physical output channels: {:?}", output_channel_map);
            log::info!("  Max HW channel: {}", max_hw_ch);

            let matrix_plugin =
                create_matrix_plugin_config(input_channel_map, output_channel_map, matrix)?;
            plugins.push(matrix_plugin);
            log::debug!(
                "Added matrix plugin: {}ch (logical) -> {} HW channels",
                logical_output_channels,
                max_hw_ch
            );

            max_hw_ch // Hardware will need this many channels
        } else {
            output_channels // No mapping, use current channel count
        };

        // Add loudness analyzer plugin if LUFS monitoring is requested
        let loudness_plugin_index = if lufs {
            let analyzer_plugin = create_loudness_analyzer_plugin_config()?;
            let plugin_index = plugins.len();
            plugins.push(analyzer_plugin);
            log::info!(
                "Real-time LUFS monitoring enabled (plugin index: {})",
                plugin_index
            );
            Some(plugin_index)
        } else {
            None
        };

        (plugins, output_channels, loudness_plugin_index)
    };

    // Start playback (signal handling is done by the manager)
    streaming_manager
        .start_playback(device, plugins, output_channels)
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
        // Check for events (this updates internal state based on engine state)
        streaming_manager.try_recv_event();

        let current_state = streaming_manager.get_state();

        // Print state changes
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
                // Dynamic ReplayGain relative to -18.0 LUFS reference
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

    // Manager handles its own cleanup via Drop
    // If stopped by signal, threads are already shut down
    // If stopped naturally (end of stream/duration), cleanup happens on drop
    log::info!("Streaming playback stopped successfully");
    Ok(())
}
