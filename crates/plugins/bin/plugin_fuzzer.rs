// ============================================================================
// Plugin Fuzzer - Test plugins with random parameter combinations
// ============================================================================
//
// This tool loads an audio file, processes it with a plugin using N random
// parameter combinations, and detects abnormal outputs (NaN, Inf, extreme
// values, DC offset, clipping, etc.)
//
// Usage:
//   plugin_fuzzer --file audio.wav --plugin gain --iterations 100
//   plugin_fuzzer --file audio.flac --plugin eq --iterations 1000 --seed 42

use clap::Parser;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use sotf_plugins::{
    ChannelMuteSoloParams, ChannelMuteSoloPlugin, ChannelState, CompressorPlugin,
    CompressorPluginParams, CrossoverPlugin, CrossoverPluginParams, DawHost, DelayPlugin,
    DelayPluginParams, DenoiserPlugin, DenoiserPluginParams, EqPlugin, EqPluginParams,
    ExpanderPlugin, ExpanderPluginParams, FletcherMunsonPlugin, FletcherMunsonPluginParams,
    GainPlugin, GainPluginParams, GatePlugin, GatePluginParams, InPlacePluginAdapter,
    LimiterPlugin, LimiterPluginParams, LoudnessCompensationPlugin,
    LoudnessCompensationPluginParams, MatrixPlugin, MultibandCompressorPlugin,
    MultibandCompressorPluginParams, MultibandExpanderPlugin, MultibandExpanderPluginParams,
    Plugin, SpectrumAnalyzerPlugin, SpectrumConfig, UpmixerPlugin, UpmixerPluginParams,
};
use std::fs::File;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use symphonia::core::audio::{AudioBufferRef, SampleBuffer, SignalSpec};
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

// ============================================================================
// CLI Arguments
// ============================================================================

#[derive(Parser, Debug)]
#[command(name = "plugin_fuzzer")]
#[command(about = "Fuzz test audio plugins with random parameter combinations")]
struct Args {
    /// Audio file path
    #[arg(short, long)]
    file: PathBuf,

    /// Plugin to test (gain, eq, compressor, limiter, gate, delay, loudness, crossover, upmixer,
    /// expander, multiband_compressor/mbcomp, multiband_expander/mbexp, matrix, mutesolo, denoiser)
    #[arg(short, long)]
    plugin: String,

    /// Number of iterations (parameter combinations to test)
    #[arg(short, long, default_value = "100")]
    iterations: usize,

    /// Random seed for reproducibility
    #[arg(short, long)]
    seed: Option<u64>,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Maximum allowed sample value before flagging
    #[arg(long, default_value = "10.0")]
    max_value: f32,

    /// Maximum allowed DC offset before flagging (average absolute value)
    #[arg(long, default_value = "0.5")]
    max_dc_offset: f32,
}

// ============================================================================
// Abnormality Detection
// ============================================================================

#[derive(Debug, Clone)]
struct AbnormalityReport {
    iteration: usize,
    has_nan: bool,
    has_inf: bool,
    has_extreme_values: bool,
    has_dc_offset: bool,
    has_clipping: bool,
    has_denormals: bool,
    is_silent: bool,
    max_value: f32,
    min_value: f32,
    dc_offset: f32,
    clipping_error_db: Option<f32>,
    denormal_count: usize,
    parameters: String,
}

impl AbnormalityReport {
    fn has_issues(&self) -> bool {
        self.has_nan
            || self.has_inf
            || self.has_extreme_values
            || self.has_dc_offset
            || self.has_clipping
            || self.has_denormals
            || self.is_silent
    }

    fn print(&self) {
        println!("\n[ISSUE FOUND] Iteration {}", self.iteration);
        println!("  Parameters: {}", self.parameters);
        if self.has_nan {
            println!("  - Contains NaN values");
        }
        if self.has_inf {
            println!("  - Contains Inf values");
        }
        if self.has_extreme_values {
            println!(
                "  - Extreme values detected (max={:.2}, min={:.2})",
                self.max_value, self.min_value
            );
        }
        if self.has_dc_offset {
            println!("  - DC offset detected ({:.4})", self.dc_offset);
        }
        if self.has_clipping {
            if let Some(error_db) = self.clipping_error_db {
                println!(
                    "  - Clipping detected (peak: {:.2} dBFS, {:.2} dB above threshold)",
                    error_db, error_db
                );
            } else {
                println!("  - Clipping detected (values >= 1.0 or <= -1.0)");
            }
        }
        if self.has_denormals {
            println!("  - Denormals detected ({} samples)", self.denormal_count);
        }
        if self.is_silent {
            println!("  - Output is silent (all zeros)");
        }
    }
}

fn detect_abnormalities(
    output: &[f32],
    iteration: usize,
    parameters: String,
    max_value_threshold: f32,
    max_dc_threshold: f32,
) -> AbnormalityReport {
    let mut has_nan = false;
    let mut has_inf = false;
    let mut has_extreme_values = false;
    let mut has_clipping = false;
    let mut has_denormals = false;
    let mut max_value = f32::NEG_INFINITY;
    let mut min_value = f32::INFINITY;
    let mut max_clipping_value = 1.0f32;
    let mut sum = 0.0;
    let mut non_zero_count = 0;
    let mut denormal_count = 0;

    // Denormal threshold: smallest normalized f32 is ~1.175494e-38
    // We consider anything smaller than this (but non-zero) as denormal
    const DENORMAL_THRESHOLD: f32 = 1.175494e-38;

    for &sample in output {
        if sample.is_nan() {
            has_nan = true;
        }
        if sample.is_infinite() {
            has_inf = true;
        }
        if sample.abs() > max_value_threshold {
            has_extreme_values = true;
        }
        if sample.abs() >= 1.0 {
            has_clipping = true;
            max_clipping_value = max_clipping_value.max(sample.abs());
        }
        if sample != 0.0 {
            non_zero_count += 1;
            // Check for denormals (non-zero but below normalized threshold)
            if sample.abs() < DENORMAL_THRESHOLD {
                has_denormals = true;
                denormal_count += 1;
            }
        }
        max_value = max_value.max(sample);
        min_value = min_value.min(sample);
        sum += sample;
    }

    let is_silent = non_zero_count == 0;
    let dc_offset = if output.is_empty() {
        0.0
    } else {
        sum / output.len() as f32
    };
    let has_dc_offset = dc_offset.abs() > max_dc_threshold;

    // Calculate clipping error in dB (relative to 0dBFS threshold)
    let clipping_error_db = if has_clipping {
        Some(20.0 * max_clipping_value.log10())
    } else {
        None
    };

    AbnormalityReport {
        iteration,
        has_nan,
        has_inf,
        has_extreme_values,
        has_dc_offset,
        has_clipping,
        has_denormals,
        is_silent,
        max_value,
        min_value,
        dc_offset,
        clipping_error_db,
        denormal_count,
        parameters,
    }
}

/// Normalize audio output to prevent gain-related clipping while preserving
/// signal characteristics. This allows us to isolate numerical issues from
/// legitimate gain changes.
///
/// Returns the gain compensation applied in dB (positive value = attenuation)
fn normalize_output(output: &mut [f32]) -> f32 {
    // First, check for NaN/Inf which shouldn't be normalized
    for &sample in output.iter() {
        if sample.is_nan() || sample.is_infinite() {
            return 0.0; // Don't normalize if there are NaN/Inf values
        }
    }

    // Find peak absolute value
    let peak = output
        .iter()
        .map(|&s| s.abs())
        .fold(f32::NEG_INFINITY, f32::max);

    // If peak is above threshold, normalize to target level
    const NORMALIZATION_THRESHOLD: f32 = 0.95; // Start normalizing above -0.5dB
    const TARGET_PEAK: f32 = 0.89; // Target -1dB peak to leave headroom

    if peak > NORMALIZATION_THRESHOLD {
        let gain = TARGET_PEAK / peak;
        let gain_db = 20.0 * (1.0 / gain).log10(); // Positive = attenuation

        // Apply gain compensation
        for sample in output.iter_mut() {
            *sample *= gain;
        }

        gain_db
    } else {
        0.0
    }
}

// ============================================================================
// Audio File Loading
// ============================================================================

fn load_audio_file(path: &PathBuf) -> Result<(Vec<f32>, usize, u32), String> {
    let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension() {
        hint.with_extension(ext.to_str().unwrap_or(""));
    }

    let format_opts = FormatOptions::default();
    let metadata_opts = MetadataOptions::default();
    let decoder_opts = DecoderOptions::default();

    // Create probe with explicit format support
    let mut probe = symphonia::core::probe::Probe::default();
    probe.register_all::<symphonia_format_riff::WavReader>();
    probe.register_all::<symphonia_bundle_flac::FlacReader>();
    probe.register_all::<symphonia_bundle_mp3::MpaReader>();

    let probed = probe
        .format(&hint, mss, &format_opts, &metadata_opts)
        .map_err(|e| format!("Failed to probe file: {}", e))?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("No valid audio track found")?;

    let track_id = track.id;
    // Create codec registry with explicit codec support
    let mut codecs = symphonia::core::codecs::CodecRegistry::new();
    codecs.register_all::<symphonia_codec_pcm::PcmDecoder>();
    codecs.register_all::<symphonia_bundle_flac::FlacDecoder>();
    codecs.register_all::<symphonia_bundle_mp3::MpaDecoder>();

    let mut decoder = codecs
        .make(&track.codec_params, &decoder_opts)
        .map_err(|e| format!("Failed to create decoder: {}", e))?;

    let channels_info = track.codec_params.channels.ok_or("No channel info")?;
    let channels = channels_info.count();
    let sample_rate = track.codec_params.sample_rate.ok_or("No sample rate")?;

    let mut audio_data = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(_)) => break,
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(format!("Failed to read packet: {}", e)),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let num_frames = match &decoded {
                    AudioBufferRef::F32(b) => b.capacity(),
                    AudioBufferRef::U8(b) => b.capacity(),
                    AudioBufferRef::U16(b) => b.capacity(),
                    AudioBufferRef::U24(b) => b.capacity(),
                    AudioBufferRef::U32(b) => b.capacity(),
                    AudioBufferRef::S8(b) => b.capacity(),
                    AudioBufferRef::S16(b) => b.capacity(),
                    AudioBufferRef::S24(b) => b.capacity(),
                    AudioBufferRef::S32(b) => b.capacity(),
                    AudioBufferRef::F64(b) => b.capacity(),
                };

                let mut sample_buf = SampleBuffer::<f32>::new(
                    num_frames as u64,
                    SignalSpec::new(sample_rate, channels_info),
                );
                sample_buf.copy_interleaved_ref(decoded);

                audio_data.extend_from_slice(sample_buf.samples());
            }
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(_) => break,
        }
    }

    Ok((audio_data, channels, sample_rate))
}

// ============================================================================
// Plugin Fuzzing
// ============================================================================

trait PluginFuzzer {
    /// Create a plugin with random parameters and return both the plugin and a description
    /// of the actual parameters used for debugging.
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String);
}

struct GainFuzzer;

impl PluginFuzzer for GainFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let gain_db = rng.random_range(-60.0..0.0);
        let params = GainPluginParams {
            gain_db,
            channel_gains: vec![],
        };
        let plugin = Box::new(InPlacePluginAdapter::new(
            GainPlugin::from_params(channels, params).expect("Failed to create GainPlugin"),
        ));
        let desc = format!("gain_db={:.2}", gain_db);
        (plugin, desc)
    }
}

struct EqFuzzer {
    sample_rate: u32,
}

impl PluginFuzzer for EqFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        use math_audio_iir_fir::{Biquad, BiquadFilterType, Peq, peq_loudness_gain};
        use sotf_plugins::BiquadFilterConfig;

        // Generate 1-5 random filters
        let num_filters = rng.random_range(1..=5);
        let mut filters = Vec::new();

        for _ in 0..num_filters {
            let filter_type = match rng.random_range(0..3) {
                0 => "peak",
                1 => "lowshelf",
                _ => "highshelf",
            };

            let freq = rng.random_range(20.0..20000.0);
            let q = rng.random_range(0.1..10.0);
            let db_gain = rng.random_range(-20.0..20.0);

            filters.push(BiquadFilterConfig {
                filter_type: filter_type.to_string(),
                freq,
                q,
                db_gain,
            });
        }

        // Convert to Biquad structs to calculate loudness gain
        let peq: Peq = filters
            .iter()
            .map(|f| {
                let filter_type = match f.filter_type.as_str() {
                    "peak" => BiquadFilterType::Peak,
                    "lowshelf" => BiquadFilterType::Lowshelf,
                    "highshelf" => BiquadFilterType::Highshelf,
                    _ => BiquadFilterType::Peak,
                };
                let biquad =
                    Biquad::new(filter_type, f.freq, self.sample_rate as f64, f.q, f.db_gain);
                (1.0, biquad)
            })
            .collect();

        // Calculate loudness gain and compensate
        let loudness_gain = peq_loudness_gain(&peq, "k");

        // Apply compensation by reducing all filter gains
        for filter in &mut filters {
            filter.db_gain -= loudness_gain;
        }

        // Build parameter description
        let mut desc = format!(
            "filters={} loudness_comp={:.2}dB [",
            filters.len(),
            loudness_gain
        );
        for (i, f) in filters.iter().enumerate() {
            if i > 0 {
                desc.push_str(", ");
            }
            desc.push_str(&format!(
                "{}:{:.0}Hz q={:.2} gain={:.2}dB",
                f.filter_type, f.freq, f.q, f.db_gain
            ));
        }
        desc.push(']');

        let params = EqPluginParams {
            filters,
            channel_filters: None,
            ..Default::default()
        };
        let plugin = Box::new(EqPlugin::from_params(channels, self.sample_rate, params).unwrap());
        (plugin, desc)
    }
}

struct CompressorFuzzer;

impl PluginFuzzer for CompressorFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let threshold_db = rng.random_range(-60.0..0.0);
        let ratio = rng.random_range(1.0..20.0);
        let attack_ms = rng.random_range(0.1..100.0);
        let release_ms = rng.random_range(10.0..1000.0);
        let knee_db = rng.random_range(0.0..20.0);
        let makeup_gain_db = rng.random_range(-24.0..24.0);
        let mix = rng.random_range(0.0..1.0);
        let auto_makeup = rng.random_bool(0.5);
        let link_channels = rng.random_bool(0.5);
        let sidechain_hpf_hz = rng.random_range(0.0..200.0);

        let params = CompressorPluginParams {
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
        };
        let plugin = CompressorPlugin::from_params(channels, params);

        let desc = format!(
            "threshold={:.1}dB ratio={:.2}:1 attack={:.1}ms release={:.0}ms knee={:.1}dB makeup={:.1}dB mix={:.2} auto_makeup={} link={} sc_hpf={:.0}Hz",
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            makeup_gain_db,
            mix,
            auto_makeup,
            link_channels,
            sidechain_hpf_hz
        );

        (Box::new(InPlacePluginAdapter::new(plugin)), desc)
    }
}

struct LimiterFuzzer;

impl PluginFuzzer for LimiterFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let threshold_db = rng.random_range(-20.0..0.0);
        let release_ms = rng.random_range(10.0..1000.0);
        let lookahead_ms = rng.random_range(0.0..20.0);
        let soft = rng.random_bool(0.5);
        let mix = rng.random_range(0.0..1.0);

        let params = LimiterPluginParams {
            threshold_db,
            release_ms,
            lookahead_ms,
            soft,
            mix,
        };
        let plugin = LimiterPlugin::from_params(channels, params);

        let desc = format!(
            "threshold={:.1}dB release={:.0}ms lookahead={:.1}ms soft={} mix={:.2}",
            threshold_db, release_ms, lookahead_ms, soft, mix
        );

        (Box::new(InPlacePluginAdapter::new(plugin)), desc)
    }
}

struct GateFuzzer;

impl PluginFuzzer for GateFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let threshold_db = rng.random_range(-80.0..0.0);
        let ratio = rng.random_range(1.0..100.0);
        let attack_ms = rng.random_range(0.1..50.0);
        let hold_ms = rng.random_range(0.0..1000.0);
        let release_ms = rng.random_range(10.0..2000.0);
        let mix = rng.random_range(0.0..1.0);
        let link_channels = rng.random_bool(0.5);
        let sidechain_hpf_hz = rng.random_range(0.0..200.0);

        let params = GatePluginParams {
            threshold_db,
            ratio,
            attack_ms,
            hold_ms,
            release_ms,
            mix,
            link_channels,
            sidechain_hpf_hz,
        };
        let plugin = GatePlugin::from_params(channels, params);

        let desc = format!(
            "threshold={:.1}dB ratio={:.1}:1 attack={:.1}ms hold={:.0}ms release={:.0}ms mix={:.2} link={} sc_hpf={:.0}Hz",
            threshold_db,
            ratio,
            attack_ms,
            hold_ms,
            release_ms,
            mix,
            link_channels,
            sidechain_hpf_hz
        );

        (Box::new(InPlacePluginAdapter::new(plugin)), desc)
    }
}

struct DelayFuzzer;

impl PluginFuzzer for DelayFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let delay_ms = rng.random_range(0.1..5000.0);
        let feedback = rng.random_range(0.0..0.95);
        let mix = rng.random_range(0.0..1.0);

        let params = DelayPluginParams {
            delay_ms,
            feedback,
            mix,
        };

        let desc = format!(
            "delay={:.1}ms feedback={:.2} mix={:.2}",
            delay_ms, feedback, mix
        );

        (
            Box::new(InPlacePluginAdapter::new(DelayPlugin::from_params(
                channels, params,
            ))),
            desc,
        )
    }
}

struct LoudnessCompensationFuzzer;

impl PluginFuzzer for LoudnessCompensationFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let low_freq = rng.random_range(20.0..500.0);
        let low_gain = rng.random_range(0.0..20.0);
        let high_freq = rng.random_range(5000.0..20000.0);
        let high_gain = rng.random_range(0.0..20.0);

        let params = LoudnessCompensationPluginParams {
            low_freq,
            low_gain,
            high_freq,
            high_gain,
            channel_params: vec![],
            auto_gain_enabled: false,
            auto_gain_max_db: 12.0,
            auto_gain_smoothing_ms: 100.0,
        };
        let plugin = LoudnessCompensationPlugin::from_params(channels, params)
            .expect("Failed to create LoudnessCompensationPlugin");

        let desc = format!(
            "low_freq={:.0}Hz low_gain={:.1}dB high_freq={:.0}Hz high_gain={:.1}dB",
            low_freq, low_gain, high_freq, high_gain
        );

        (Box::new(plugin), desc)
    }
}

struct CrossoverFuzzer;

impl PluginFuzzer for CrossoverFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let crossover_types = vec!["LR24", "LR48", "Butterworth24", "Butterworth12"];
        let crossover_type =
            crossover_types[rng.random_range(0..crossover_types.len())].to_string();
        let frequency = rng.random_range(20.0..20000.0);
        let outputs = vec!["low", "high"];
        let output = outputs[rng.random_range(0..outputs.len())].to_string();

        let params = CrossoverPluginParams {
            crossover_type: crossover_type.clone(),
            frequency,
            output: output.clone(),
        };
        let plugin = CrossoverPlugin::from_params(channels, &params).unwrap();

        let desc = format!(
            "type={} freq={:.0}Hz output={}",
            crossover_type, frequency, output
        );

        (Box::new(InPlacePluginAdapter::new(plugin)), desc)
    }
}

struct ExpanderFuzzer;

impl PluginFuzzer for ExpanderFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let threshold_db = rng.random_range(-80.0..0.0);
        let ratio = rng.random_range(1.0..20.0);
        let attack_ms = rng.random_range(0.1..50.0);
        let release_ms = rng.random_range(10.0..2000.0);
        let range_db = rng.random_range(0.0..80.0);
        let knee_db = rng.random_range(0.0..20.0);
        let hysteresis_db = rng.random_range(0.0..12.0);
        let hold_ms = rng.random_range(0.0..500.0);
        let mix = rng.random_range(0.0..1.0);
        let link_channels = rng.random_bool(0.5);
        let sidechain_hpf_hz = rng.random_range(0.0..500.0);

        let params = ExpanderPluginParams {
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
        };
        let plugin = ExpanderPlugin::from_params(channels, params);

        let desc = format!(
            "threshold={:.1}dB ratio={:.2}:1 attack={:.1}ms release={:.0}ms range={:.1}dB knee={:.1}dB hyst={:.1}dB hold={:.0}ms mix={:.2} link={} sc_hpf={:.0}Hz",
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
            sidechain_hpf_hz
        );

        (Box::new(InPlacePluginAdapter::new(plugin)), desc)
    }
}

struct MultibandCompressorFuzzer;

impl PluginFuzzer for MultibandCompressorFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let num_bands = rng.random_range(2..=5);
        let crossover_preset = rng.random_range(0..=3);
        let threshold_db = rng.random_range(-60.0..0.0);
        let ratio = rng.random_range(1.0..20.0);
        let attack_ms = rng.random_range(0.1..100.0);
        let release_ms = rng.random_range(10.0..1000.0);
        let knee_db = rng.random_range(0.0..20.0);
        let mix = rng.random_range(0.0..1.0);
        let link_channels = rng.random_bool(0.5);

        // Generate random crossover frequencies (sorted ascending)
        let mut freqs = vec![
            rng.random_range(20.0..500.0),
            rng.random_range(500.0..5000.0),
            rng.random_range(5000.0..15000.0),
            rng.random_range(10000.0..18000.0),
        ];
        freqs.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let params = MultibandCompressorPluginParams {
            num_bands,
            crossover_preset,
            crossover_frequencies: freqs.clone(),
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            link_channels,
            mix,
            bands: vec![], // Use defaults for per-band params
        };
        let plugin = MultibandCompressorPlugin::from_params(channels, params);

        let desc = format!(
            "bands={} preset={} threshold={:.1}dB ratio={:.2}:1 attack={:.1}ms release={:.0}ms knee={:.1}dB mix={:.2} link={}",
            num_bands,
            crossover_preset,
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            mix,
            link_channels
        );

        (Box::new(InPlacePluginAdapter::new(plugin)), desc)
    }
}

struct MultibandExpanderFuzzer;

impl PluginFuzzer for MultibandExpanderFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let num_bands = rng.random_range(2..=5);
        let crossover_preset = rng.random_range(0..=3);
        let threshold_db = rng.random_range(-80.0..0.0);
        let ratio = rng.random_range(1.0..20.0);
        let attack_ms = rng.random_range(0.1..50.0);
        let release_ms = rng.random_range(10.0..2000.0);
        let knee_db = rng.random_range(0.0..20.0);
        let range_db = rng.random_range(0.0..80.0);
        let hysteresis_db = rng.random_range(0.0..12.0);
        let hold_ms = rng.random_range(0.0..500.0);
        let mix = rng.random_range(0.0..1.0);
        let link_channels = rng.random_bool(0.5);

        // Generate random crossover frequencies (sorted ascending)
        let mut freqs = vec![
            rng.random_range(20.0..500.0),
            rng.random_range(500.0..5000.0),
            rng.random_range(5000.0..15000.0),
            rng.random_range(10000.0..18000.0),
        ];
        freqs.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let params = MultibandExpanderPluginParams {
            num_bands,
            crossover_preset,
            crossover_frequencies: freqs.clone(),
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            range_db,
            hysteresis_db,
            hold_ms,
            link_channels,
            mix,
            bands: vec![], // Use defaults for per-band params
        };
        let plugin = MultibandExpanderPlugin::from_params(channels, params);

        let desc = format!(
            "bands={} preset={} threshold={:.1}dB ratio={:.2}:1 attack={:.1}ms release={:.0}ms range={:.1}dB mix={:.2} link={}",
            num_bands,
            crossover_preset,
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            range_db,
            mix,
            link_channels
        );

        (Box::new(InPlacePluginAdapter::new(plugin)), desc)
    }
}

struct MatrixFuzzer;

impl PluginFuzzer for MatrixFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        // Generate a random matrix with values in [0, 1]
        // Keep it reasonable: output same channel count as input
        let mut matrix = vec![0.0_f32; channels * channels];

        // Fill with random values
        for i in 0..matrix.len() {
            matrix[i] = rng.random_range(0.0..1.0);
        }

        // Optionally make it more identity-like sometimes
        if rng.random_bool(0.3) {
            // 30% chance of mostly-identity matrix
            matrix.fill(0.0);
            for i in 0..channels {
                matrix[i * channels + i] = rng.random_range(0.5..1.0);
            }
        }

        let plugin = MatrixPlugin::with_matrix(channels, channels, matrix.clone())
            .expect("Failed to create MatrixPlugin");

        // Describe the matrix briefly
        let desc = if channels <= 4 {
            format!(
                "{}x{} matrix {:?}",
                channels,
                channels,
                &matrix[..matrix.len().min(8)]
            )
        } else {
            format!("{}x{} matrix (truncated)", channels, channels)
        };

        (Box::new(plugin), desc)
    }
}

struct ChannelMuteSoloFuzzer;

impl PluginFuzzer for ChannelMuteSoloFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let enabled = rng.random_bool(0.8); // 80% enabled

        let mut channel_states = Vec::with_capacity(channels);
        let mut desc_parts = Vec::new();

        for ch in 0..channels {
            let muted = rng.random_bool(0.2);
            let soloed = rng.random_bool(0.1);
            let dimmed = rng.random_bool(0.1);

            channel_states.push(ChannelState {
                muted,
                soloed,
                dimmed,
            });

            if muted || soloed || dimmed {
                desc_parts.push(format!(
                    "ch{}:{}{}{}",
                    ch,
                    if muted { "M" } else { "" },
                    if soloed { "S" } else { "" },
                    if dimmed { "D" } else { "" }
                ));
            }
        }

        let params = ChannelMuteSoloParams {
            enabled,
            channel_states,
        };
        let plugin = ChannelMuteSoloPlugin::from_params(channels, params);

        let desc = format!(
            "enabled={} {}",
            enabled,
            if desc_parts.is_empty() {
                "no_changes".to_string()
            } else {
                desc_parts.join(" ")
            }
        );

        (Box::new(InPlacePluginAdapter::new(plugin)), desc)
    }
}

struct DenoiserFuzzer;

impl PluginFuzzer for DenoiserFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let reduction_db = rng.random_range(0.0..40.0);
        let floor_db = rng.random_range(-60.0..-10.0);
        let smoothing = rng.random_range(0.0..0.99);
        let attack_ms = rng.random_range(0.1..100.0);
        let release_ms = rng.random_range(10.0..500.0);
        let low_latency = rng.random_bool(0.5);
        let polyphonic_detection = rng.random_bool(0.3);
        let crack_sensitivity = rng.random_range(1.0..100.0);

        let params = DenoiserPluginParams {
            reduction_db,
            floor_db,
            smoothing,
            attack_ms,
            release_ms,
            low_latency,
            polyphonic_detection,
            crack_sensitivity,
            ..Default::default()
        };

        let plugin = DenoiserPlugin::from_params(channels, params);

        let desc = format!(
            "reduction={:.1}dB floor={:.1}dB smooth={:.2} attack={:.1}ms release={:.0}ms low_lat={} poly={} crack={:.1}",
            reduction_db,
            floor_db,
            smoothing,
            attack_ms,
            release_ms,
            low_latency,
            polyphonic_detection,
            crack_sensitivity
        );

        (Box::new(InPlacePluginAdapter::new(plugin)), desc)
    }
}

struct UpmixerFuzzer;

impl PluginFuzzer for UpmixerFuzzer {
    fn create_plugin(&self, _channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        // Upmixer always takes 2 channels input
        let speaker_configs = ["5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4"];
        let speaker_config =
            speaker_configs[rng.random_range(0..speaker_configs.len())].to_string();

        // Random FFT size (power of 2)
        let fft_sizes = [1024, 2048, 4096];
        let fft_size = fft_sizes[rng.random_range(0..fft_sizes.len())];

        // Random parameters with reasonable ranges
        let gain_front_direct = rng.random_range(0.5..1.5);
        let gain_front_ambient = rng.random_range(0.0..1.0);
        let gain_rear_ambient = rng.random_range(0.5..2.0);
        let lfe_cutoff_hz = rng.random_range(80.0..150.0);
        let stereo_width = rng.random_range(0.0..1.0);
        let bandpass_hz = rng.random_range(150.0..400.0);
        let center_spread = rng.random_range(0.0..0.5);
        let height_gain = rng.random_range(0.0..0.5);
        let lfe_gain = rng.random_range(0.5..1.5);
        let subharmonic_gain = rng.random_range(0.0..1.0);
        let hr_sharpen = rng.random_range(0.5..2.0);
        let safety_cap_db = rng.random_range(0.0..6.0);

        let enable_subharmonic_synth = rng.random_bool(0.5);
        let enable_hr_direct = rng.random_bool(0.3); // Less frequent, experimental feature
        let decorrelation_mode = rng.random_range(0..=1); // 0=Velvet, 1=LFO

        let params = UpmixerPluginParams {
            fft_size,
            speaker_config: speaker_config.clone(),
            gain_front_direct,
            gain_front_ambient,
            gain_rear_ambient,
            lfe_cutoff_hz,
            stereo_width,
            bandpass_hz,
            center_spread,
            height_gain,
            lfe_gain,
            enable_subharmonic_synth,
            subharmonic_gain,
            enable_hr_direct,
            hr_sharpen,
            safety_cap_db,
            decorrelation_mode,
            // Use defaults for the new parameters
            subharmonic_freq_hz: 40.0,
            subharmonic_attack_ms: 10.0,
            subharmonic_release_ms: 50.0,
            decorrelation_lfo_rate_hz: 0.15,
            velvet_noise_duration_ms: 30.0,
            velvet_noise_density: 2000.0,
            height_hf_cap_hz: 16000.0,
            height_transient_reduction: 0.6,
            height_direct_leak: 0.15,
            surround_direct_bleed: 0.50,
            rear_ambient_boost: 1.5,
            rear_late_reflection: 0.10,
            ambient_boost: 1.2,
            dialogue_weight: 0.4,
            voice_freq_min_hz: 500.0,
            voice_freq_max_hz: 3000.0,
            // Diagnostic bypass parameters
            bypass_decorrelation: false,
            bypass_transient_detection: false,
            bypass_all_processing: false,
        };

        let desc = format!(
            "config={} fft={} g_fd={:.2} g_fa={:.2} g_ra={:.2} lfe_co={:.0}Hz sw={:.2} bp={:.0}Hz cs={:.2} hg={:.2} lfeg={:.2} subh={}/{:.2} hr={}/{:.2} cap={:.1}dB decor={}",
            speaker_config,
            fft_size,
            gain_front_direct,
            gain_front_ambient,
            gain_rear_ambient,
            lfe_cutoff_hz,
            stereo_width,
            bandpass_hz,
            center_spread,
            height_gain,
            lfe_gain,
            enable_subharmonic_synth,
            subharmonic_gain,
            enable_hr_direct,
            hr_sharpen,
            safety_cap_db,
            decorrelation_mode
        );

        (Box::new(UpmixerPlugin::from_params(params)), desc)
    }
}

struct FletcherMunsonFuzzer;

impl PluginFuzzer for FletcherMunsonFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let playback_volume_db = rng.random_range(-60.0..0.0);
        let reference_level_db = rng.random_range(-30.0..0.0);

        let params = FletcherMunsonPluginParams {
            playback_volume_db,
            reference_level_db,
            ..Default::default()
        };
        let plugin = FletcherMunsonPlugin::from_params(channels, params);

        let desc = format!(
            "playback_vol={:.1}dB ref_level={:.1}dB",
            playback_volume_db, reference_level_db
        );

        (Box::new(plugin), desc)
    }
}

struct SpectrumAnalyzerFuzzer;

impl PluginFuzzer for SpectrumAnalyzerFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let num_bins = rng.random_range(10..100);
        let min_freq = rng.random_range(10.0..100.0);
        let max_freq = rng.random_range(10000.0..22000.0);
        let smoothing = rng.random_range(0.0..1.0);

        let config = SpectrumConfig {
            num_bins,
            min_freq,
            max_freq,
            smoothing,
            ..Default::default()
        };

        let plugin = SpectrumAnalyzerPlugin::with_config(channels, config)
            .expect("Failed to create SpectrumAnalyzerPlugin");

        let desc = format!(
            "bins={} min_freq={:.0}Hz max_freq={:.0}Hz smoothing={:.2}",
            num_bins, min_freq, max_freq, smoothing
        );

        (Box::new(plugin), desc)
    }
}

fn get_fuzzer(plugin_name: &str, sample_rate: u32) -> Result<Box<dyn PluginFuzzer>, String> {
    match plugin_name.to_lowercase().as_str() {
        "gain" => Ok(Box::new(GainFuzzer)),
        "eq" => Ok(Box::new(EqFuzzer { sample_rate })),
        "compressor" | "comp" => Ok(Box::new(CompressorFuzzer)),
        "limiter" | "limit" => Ok(Box::new(LimiterFuzzer)),
        "gate" => Ok(Box::new(GateFuzzer)),
        "delay" => Ok(Box::new(DelayFuzzer)),
        "loudness" | "loudness_compensation" => Ok(Box::new(LoudnessCompensationFuzzer)),
        "crossover" | "xover" => Ok(Box::new(CrossoverFuzzer)),
        "upmixer" | "upmix" => Ok(Box::new(UpmixerFuzzer)),
        "expander" | "expand" => Ok(Box::new(ExpanderFuzzer)),
        "multiband_compressor" | "mbcomp" | "multiband_comp" => {
            Ok(Box::new(MultibandCompressorFuzzer))
        }
        "multiband_expander" | "mbexp" | "multiband_exp" => Ok(Box::new(MultibandExpanderFuzzer)),
        "matrix" => Ok(Box::new(MatrixFuzzer)),
        "channel_mute_solo" | "mute_solo" | "mutesolo" => Ok(Box::new(ChannelMuteSoloFuzzer)),
        "denoiser" | "denoise" => Ok(Box::new(DenoiserFuzzer)),
        "fletcher_munson" | "fletcher" => Ok(Box::new(FletcherMunsonFuzzer)),
        "spectrum" | "spectrum_analyzer" => Ok(Box::new(SpectrumAnalyzerFuzzer)),
        _ => Err(format!(
            "Unknown plugin type: {}. Supported: gain, eq, compressor, limiter, gate, delay, loudness, crossover, upmixer, expander, multiband_compressor (mbcomp), multiband_expander (mbexp), matrix, channel_mute_solo (mutesolo), denoiser, fletcher_munson (fletcher), spectrum",
            plugin_name
        )),
    }
}

// ============================================================================
// Audio Resampling
// ============================================================================

/// Simple linear resampler for fuzzing purposes
fn resample_audio(audio_data: &[f32], channels: usize, from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return audio_data.to_vec();
    }

    let num_frames = audio_data.len() / channels;
    let ratio = to_rate as f64 / from_rate as f64;
    let new_num_frames = (num_frames as f64 * ratio).ceil() as usize;
    let mut resampled = vec![0.0f32; new_num_frames * channels];

    for out_frame in 0..new_num_frames {
        let in_pos = out_frame as f64 / ratio;
        let in_frame = in_pos.floor() as usize;
        let frac = in_pos - in_frame as f64;

        if in_frame + 1 < num_frames {
            // Linear interpolation between frames
            for ch in 0..channels {
                let sample1 = audio_data[in_frame * channels + ch];
                let sample2 = audio_data[(in_frame + 1) * channels + ch];
                resampled[out_frame * channels + ch] = sample1 + (sample2 - sample1) * frac as f32;
            }
        } else if in_frame < num_frames {
            // Last frame, no interpolation
            for ch in 0..channels {
                resampled[out_frame * channels + ch] = audio_data[in_frame * channels + ch];
            }
        }
    }

    resampled
}

// ============================================================================
// Main Fuzzing Loop
// ============================================================================

fn run_fuzzer(args: Args) -> Result<(), String> {
    println!("Plugin Fuzzer");
    println!("=============");
    println!("File: {}", args.file.display());
    println!("Plugin: {}", args.plugin);
    println!("Iterations: {}", args.iterations);

    if let Some(seed) = args.seed {
        println!("Seed: {}", seed);
    }
    println!();

    // Load audio file
    println!("Loading audio file...");
    let (mut audio_data, channels, sample_rate) = load_audio_file(&args.file)?;
    let mut num_frames = audio_data.len() / channels;
    let duration = num_frames as f32 / sample_rate as f32;
    println!("  Channels: {}", channels);
    println!("  Sample rate: {} Hz", sample_rate);
    println!("  Frames: {}", num_frames);
    println!("  Duration: {:.2}s", duration);

    // Extract 30 seconds from middle if file is long enough
    const MAX_DURATION_SEC: f32 = 30.0;
    if duration > MAX_DURATION_SEC {
        let target_frames = (MAX_DURATION_SEC * sample_rate as f32) as usize;
        let start_frame = (num_frames - target_frames) / 2;
        let end_frame = start_frame + target_frames;

        let start_sample = start_frame * channels;
        let end_sample = end_frame * channels;

        println!(
            "  Extracting middle {:.1}s segment (frames {} to {} of {})...",
            MAX_DURATION_SEC, start_frame, end_frame, num_frames
        );

        audio_data = audio_data[start_sample..end_sample].to_vec();
        num_frames = audio_data.len() / channels;
        println!("  Using {} frames for fuzzing\n", num_frames);
    } else {
        println!();
    }

    // Check original audio for issues before fuzzing
    println!("Checking original audio file for abnormalities...");
    let original_report = detect_abnormalities(
        &audio_data,
        0,
        "original_file".to_string(),
        args.max_value,
        args.max_dc_offset,
    );

    if original_report.has_issues() {
        println!("\n[ERROR] Original audio file contains abnormalities:");
        original_report.print();
        println!("\nCannot proceed with fuzzing - input file is already problematic.");
        println!(
            "Please provide a clean audio file without NaN, Inf, extreme values, or other issues.\n"
        );
        return Err("Original audio file contains abnormalities".to_string());
    }
    println!("  Original file is clean - no abnormalities detected.\n");

    // Check if upmixer requires stereo input
    if args.plugin.to_lowercase() == "upmixer" || args.plugin.to_lowercase() == "upmix" {
        if channels != 2 {
            return Err(format!(
                "Upmixer requires stereo (2-channel) input, but the file has {} channels",
                channels
            ));
        }
    }

    // Prepare resampled versions for different sample rates
    const TARGET_RATES: [u32; 5] = [44100, 48000, 88200, 96000, 192000];
    println!("Preparing audio at multiple sample rates...");

    let mut audio_versions = Vec::new();
    for &target_rate in &TARGET_RATES {
        println!("  Resampling to {} Hz...", target_rate);
        let resampled = resample_audio(&audio_data, channels, sample_rate, target_rate);
        audio_versions.push((target_rate, resampled));
    }
    println!();

    // Determine base seed for RNG
    let base_seed = args.seed.unwrap_or_else(|| {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });

    // Progress counter for parallel execution
    let progress = AtomicUsize::new(0);

    // Run fuzzing in parallel
    println!("Running fuzzing tests with varying sample rates (parallel)...");

    let issues_found: Vec<AbnormalityReport> = (0..args.iterations)
        .into_par_iter()
        .filter_map(|i| {
            // Create RNG for this iteration (seeded deterministically)
            let mut rng = StdRng::seed_from_u64(base_seed.wrapping_add(i as u64));

            // Randomly select a sample rate
            let rate_idx = rng.random_range(0..TARGET_RATES.len());
            let (test_sample_rate, test_audio_data) = &audio_versions[rate_idx];
            let test_num_frames = test_audio_data.len() / channels;

            // Update progress - show current test on one line
            let completed = progress.fetch_add(1, Ordering::Relaxed) + 1;
            if !args.verbose {
                use std::io::Write;
                let stdout = std::io::stdout();
                let mut handle = stdout.lock();
                write!(
                    handle,
                    "\r[{}/{}] Testing @ {} Hz    ",
                    completed, args.iterations, test_sample_rate
                )
                .ok();
                handle.flush().ok();
            }

            // Get fuzzer for this sample rate
            let fuzzer = match get_fuzzer(&args.plugin, *test_sample_rate) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("\nError creating fuzzer: {}", e);
                    return None;
                }
            };

            // Create plugin with random parameters and get parameter description
            let (plugin, params_desc) = fuzzer.create_plugin(channels, &mut rng);

            // Build host and add plugin
            let mut host = DawHost::new(channels, *test_sample_rate);
            if let Err(e) = host.add_plugin(plugin) {
                eprintln!("\nError adding plugin: {}", e);
                return None;
            }

            // Get output channel count (may differ from input for plugins like upmixer)
            let output_channels = host.output_channels();

            // Process audio
            let output_samples = test_num_frames * output_channels;
            let mut output = vec![0.0; output_samples];

            // Split into chunks if needed (process in blocks of 4096 frames)
            const BLOCK_SIZE: usize = 4096;
            let mut pos = 0;

            while pos < test_num_frames {
                let frames_to_process = (test_num_frames - pos).min(BLOCK_SIZE);

                let input_slice =
                    &test_audio_data[pos * channels..(pos + frames_to_process) * channels];
                let output_slice =
                    &mut output[pos * output_channels..(pos + frames_to_process) * output_channels];

                if let Err(e) = host.process(input_slice, output_slice) {
                    eprintln!("\nError processing audio: {}", e);
                    return None;
                }

                pos += frames_to_process;
            }

            // Apply gain compensation to isolate numerical issues from gain changes
            // This is especially important for compressor/limiter plugins
            let gain_compensation_db = normalize_output(&mut output);

            // Build parameter description with all details for debugging
            let mut param_desc = format!("{} @{}Hz", params_desc, test_sample_rate);
            if gain_compensation_db > 0.1 {
                param_desc.push_str(&format!(" (normalized -{:.1}dB)", gain_compensation_db));
            }
            let report =
                detect_abnormalities(&output, i, param_desc, args.max_value, args.max_dc_offset);

            if report.has_issues() {
                // Print immediately when found (with lock for clean output)
                use std::io::Write;
                let stdout = std::io::stdout();
                let mut handle = stdout.lock();
                writeln!(handle, "\n[ISSUE FOUND] Iteration {}", report.iteration).ok();
                writeln!(handle, "  Parameters: {}", report.parameters).ok();
                if report.has_nan {
                    writeln!(handle, "  - Contains NaN values").ok();
                }
                if report.has_inf {
                    writeln!(handle, "  - Contains Inf values").ok();
                }
                if report.has_extreme_values {
                    writeln!(
                        handle,
                        "  - Extreme values detected (max={:.2}, min={:.2})",
                        report.max_value, report.min_value
                    )
                    .ok();
                }
                if report.has_dc_offset {
                    writeln!(handle, "  - DC offset detected ({:.4})", report.dc_offset).ok();
                }
                if report.has_clipping {
                    if let Some(error_db) = report.clipping_error_db {
                        writeln!(
                            handle,
                            "  - Clipping detected (peak: {:.2} dBFS, {:.2} dB above threshold)",
                            error_db, error_db
                        )
                        .ok();
                    } else {
                        writeln!(handle, "  - Clipping detected (values >= 1.0 or <= -1.0)").ok();
                    }
                }
                if report.has_denormals {
                    writeln!(
                        handle,
                        "  - Denormals detected ({} samples)",
                        report.denormal_count
                    )
                    .ok();
                }
                if report.is_silent {
                    writeln!(handle, "  - Output is silent (all zeros)").ok();
                }
                drop(handle);

                Some(report)
            } else {
                None
            }
        })
        .collect();

    // Print summary
    println!("\n\nFuzzing Summary");
    println!("===============");
    println!("Total iterations: {}", args.iterations);
    println!("Issues found: {}", issues_found.len());

    if !issues_found.is_empty() {
        println!("\nBreakdown of issues:");
        let nan_count = issues_found.iter().filter(|r| r.has_nan).count();
        let inf_count = issues_found.iter().filter(|r| r.has_inf).count();
        let extreme_count = issues_found.iter().filter(|r| r.has_extreme_values).count();
        let dc_count = issues_found.iter().filter(|r| r.has_dc_offset).count();
        let clip_count = issues_found.iter().filter(|r| r.has_clipping).count();
        let denormal_count = issues_found.iter().filter(|r| r.has_denormals).count();
        let silent_count = issues_found.iter().filter(|r| r.is_silent).count();

        if nan_count > 0 {
            println!("  - NaN values: {}", nan_count);
        }
        if inf_count > 0 {
            println!("  - Inf values: {}", inf_count);
        }
        if extreme_count > 0 {
            println!("  - Extreme values: {}", extreme_count);
        }
        if dc_count > 0 {
            println!("  - DC offset: {}", dc_count);
        }
        if clip_count > 0 {
            println!("  - Clipping: {}", clip_count);
        }
        if denormal_count > 0 {
            println!("  - Denormals: {}", denormal_count);
        }
        if silent_count > 0 {
            println!("  - Silent output: {}", silent_count);
        }
    } else {
        println!("\nNo issues detected. Plugin appears stable.");
    }

    Ok(())
}

// ============================================================================
// Main Entry Point
// ============================================================================

fn main() {
    let args = Args::parse();

    if let Err(e) = run_fuzzer(args) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
