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
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use sotf_plugins::{
    BiquadFilterConfig, DawHost, EqPlugin, EqPluginParams, GainPlugin, GainPluginParams, Host,
    InPlacePluginAdapter, Plugin, ProcessContext,
};
use std::fs::File;
use std::path::PathBuf;
use symphonia::core::audio::{AudioBufferRef, SampleBuffer, SignalSpec};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
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

    /// Plugin to test (gain, eq, compressor, limiter, etc.)
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
    is_silent: bool,
    max_value: f32,
    min_value: f32,
    dc_offset: f32,
    parameters: String,
}

impl AbnormalityReport {
    fn has_issues(&self) -> bool {
        self.has_nan
            || self.has_inf
            || self.has_extreme_values
            || self.has_dc_offset
            || self.has_clipping
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
            println!("  - Extreme values detected (max={:.2}, min={:.2})", self.max_value, self.min_value);
        }
        if self.has_dc_offset {
            println!("  - DC offset detected ({:.4})", self.dc_offset);
        }
        if self.has_clipping {
            println!("  - Clipping detected (values >= 1.0 or <= -1.0)");
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
    let mut max_value = f32::NEG_INFINITY;
    let mut min_value = f32::INFINITY;
    let mut sum = 0.0;
    let mut non_zero_count = 0;

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
        }
        if sample != 0.0 {
            non_zero_count += 1;
        }
        max_value = max_value.max(sample);
        min_value = min_value.min(sample);
        sum += sample;
    }

    let is_silent = non_zero_count == 0;
    let dc_offset = if output.is_empty() { 0.0 } else { sum / output.len() as f32 };
    let has_dc_offset = dc_offset.abs() > max_dc_threshold;

    AbnormalityReport {
        iteration,
        has_nan,
        has_inf,
        has_extreme_values,
        has_dc_offset,
        has_clipping,
        is_silent,
        max_value,
        min_value,
        dc_offset,
        parameters,
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
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> Box<dyn Plugin>;
    fn parameter_description(&self) -> String;
}

struct GainFuzzer;

impl PluginFuzzer for GainFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> Box<dyn Plugin> {
        let gain_db = rng.gen_range(-60.0..20.0);
        let params = GainPluginParams { gain_db };
        Box::new(InPlacePluginAdapter::new(GainPlugin::from_params(channels, params)))
    }

    fn parameter_description(&self) -> String {
        "gain_db: -60.0 to 20.0 dB".to_string()
    }
}

struct EqFuzzer {
    sample_rate: u32,
}

impl PluginFuzzer for EqFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> Box<dyn Plugin> {
        use sotf_plugins::BiquadFilterConfig;

        // Generate 1-5 random filters
        let num_filters = rng.gen_range(1..=5);
        let mut filters = Vec::new();

        for _ in 0..num_filters {
            let filter_type = match rng.gen_range(0..3) {
                0 => "peak",
                1 => "lowshelf",
                _ => "highshelf",
            };

            let freq = rng.gen_range(20.0..20000.0);
            let q = rng.gen_range(0.1..10.0);
            let db_gain = rng.gen_range(-20.0..20.0);

            filters.push(BiquadFilterConfig {
                filter_type: filter_type.to_string(),
                freq,
                q,
                db_gain,
            });
        }

        let params = EqPluginParams { filters, channel_filters: None };
        Box::new(EqPlugin::from_params(channels, self.sample_rate, params).unwrap())
    }

    fn parameter_description(&self) -> String {
        "1-5 filters, freq: 20-20000 Hz, Q: 0.1-10.0, gain: -20 to +20 dB".to_string()
    }
}

fn get_fuzzer(plugin_name: &str, sample_rate: u32) -> Result<Box<dyn PluginFuzzer>, String> {
    match plugin_name.to_lowercase().as_str() {
        "gain" => Ok(Box::new(GainFuzzer)),
        "eq" => Ok(Box::new(EqFuzzer { sample_rate })),
        _ => Err(format!("Unknown plugin type: {}. Supported: gain, eq", plugin_name)),
    }
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
    let (audio_data, channels, sample_rate) = load_audio_file(&args.file)?;
    let num_frames = audio_data.len() / channels;
    println!("  Channels: {}", channels);
    println!("  Sample rate: {} Hz", sample_rate);
    println!("  Frames: {}", num_frames);
    println!("  Duration: {:.2}s\n", num_frames as f32 / sample_rate as f32);

    // Get fuzzer
    let fuzzer = get_fuzzer(&args.plugin, sample_rate)?;
    println!("Plugin parameter ranges:");
    println!("  {}\n", fuzzer.parameter_description());

    // Initialize RNG
    let mut rng = if let Some(seed) = args.seed {
        StdRng::seed_from_u64(seed)
    } else {
        StdRng::from_os_rng()
    };

    // Run fuzzing
    println!("Running fuzzing tests...");
    let mut issues_found = Vec::new();

    for i in 0..args.iterations {
        if args.verbose {
            println!("  Iteration {}/{}", i + 1, args.iterations);
        } else if (i + 1) % 10 == 0 {
            print!(".");
            use std::io::Write;
            std::io::stdout().flush().unwrap();
        }

        // Create plugin with random parameters
        let mut plugin = fuzzer.create_plugin(channels, &mut rng);

        // Build host and add plugin
        let mut host = DawHost::new(channels, sample_rate);
        host.add_plugin(plugin)?;

        // Process audio
        let mut output = vec![0.0; audio_data.len()];
        let context = ProcessContext {
            sample_rate,
            num_frames,
        };

        // Split into chunks if needed (process in blocks of 4096 frames)
        const BLOCK_SIZE: usize = 4096;
        let mut pos = 0;

        while pos < num_frames {
            let frames_to_process = (num_frames - pos).min(BLOCK_SIZE);
            let samples_to_process = frames_to_process * channels;

            let input_slice = &audio_data[pos * channels..(pos + frames_to_process) * channels];
            let output_slice = &mut output[pos * channels..(pos + frames_to_process) * channels];

            host.process(input_slice, output_slice)?;

            pos += frames_to_process;
        }

        // Detect abnormalities
        let param_desc = format!("iteration_{}", i);
        let report = detect_abnormalities(
            &output,
            i,
            param_desc,
            args.max_value,
            args.max_dc_offset,
        );

        if report.has_issues() {
            report.print();
            issues_found.push(report);
        }
    }

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
