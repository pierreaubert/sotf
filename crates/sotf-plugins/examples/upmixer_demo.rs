// ============================================================================
// Upmixer Demo — Stereo to Surround
// ============================================================================
//
// Takes a stereo WAV input and upmixes it to a multichannel surround WAV
// using the UpmixerPlugin.
//
// Usage:
//   cargo run -p sotf-plugins --example upmixer_demo --release -- \
//     input.wav --config upmixer.toml
//
//   cargo run -p sotf-plugins --example upmixer_demo --release -- \
//     input.wav --config upmixer.toml --format 7.1
//
// The TOML config file contains UpmixerPluginParams.  All fields are optional;
// missing values use the plugin defaults.
//
// Example upmixer.toml:
//   speaker_config = "5.1"
//   gain_front_direct = 1.0
//   gain_front_ambient = 0.5
//   gain_rear_ambient = 1.0
//   height_gain = 1.0
//   lfe_gain = 1.0
//   stereo_width = 1.0
//   center_spread = 0.5
//   low_latency = false

use clap::Parser;
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_host::speaker_config::get_speaker_config;
use sotf_plugin_upmixer::{UpmixerPlugin, UpmixerPluginParams};
use std::fs;
use std::path::PathBuf;
use std::process;

/// Processing block size in frames.
const BLOCK_SIZE: usize = 4096;

#[derive(Parser, Debug)]
#[command(name = "upmixer_demo")]
#[command(about = "Upmix stereo WAV to multichannel surround WAV")]
struct Cli {
    /// Input stereo WAV file
    input: PathBuf,

    /// Path to TOML configuration file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Output speaker configuration (e.g. 5.1, 7.1, 5.1.4).
    /// Overrides the value in the config file.
    #[arg(short, long)]
    format: Option<String>,

    /// Output WAV file (default: <input>_upmixed.wav)
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();

    // ── Read input WAV ──────────────────────────────────────────────────
    let mut reader = hound::WavReader::open(&cli.input).unwrap_or_else(|e| {
        eprintln!("Failed to open input '{}': {e}", cli.input.display());
        process::exit(1);
    });
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let in_channels = spec.channels as usize;

    if in_channels != 2 {
        eprintln!("Expected stereo input (2 channels), got {in_channels} channels");
        process::exit(1);
    }

    println!("=== Upmixer Demo ===\n");
    println!("Input:  {}", cli.input.display());
    println!("  Sample rate: {sample_rate} Hz");
    println!("  Channels:    {in_channels}");
    println!(
        "  Bit depth:   {} ({})",
        spec.bits_per_sample,
        match spec.sample_format {
            hound::SampleFormat::Float => "float",
            hound::SampleFormat::Int => "int",
        }
    );

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.unwrap() as f32 / max)
                .collect()
        }
    };
    let total_frames = samples.len() / in_channels;
    let duration_s = total_frames as f64 / sample_rate as f64;
    println!("  Duration:    {duration_s:.2} s ({total_frames} frames)");

    // ── Load config ─────────────────────────────────────────────────────
    let mut params = if let Some(config_path) = &cli.config {
        println!("\nConfig: {}", config_path.display());
        let toml_str = fs::read_to_string(config_path).unwrap_or_else(|e| {
            eprintln!("Failed to read config '{}': {e}", config_path.display());
            process::exit(1);
        });
        toml::from_str::<UpmixerPluginParams>(&toml_str).unwrap_or_else(|e| {
            eprintln!("Failed to parse config TOML: {e}");
            process::exit(1);
        })
    } else {
        println!("\nConfig: (defaults)");
        UpmixerPluginParams::default()
    };

    // Override speaker config from CLI if provided
    if let Some(fmt) = &cli.format {
        params.speaker_config = fmt.clone();
        println!("  Format override: {fmt}");
    }

    // Validate speaker config and get channel count
    let speaker_cfg = get_speaker_config(&params.speaker_config).unwrap_or_else(|| {
        eprintln!(
            "Unknown speaker config '{}'. Supported: 2.0, 5.0, 5.1, 7.1, 5.1.2, 5.1.4, 7.1.2, 7.1.4, 9.1.4, 9.1.6",
            params.speaker_config
        );
        process::exit(1);
    });
    let out_channels = speaker_cfg.total_channels;
    println!(
        "  Speaker config:  {} ({} channels)",
        params.speaker_config, out_channels
    );

    // ── Create upmixer ──────────────────────────────────────────────────
    println!("\n--- Upmixing ---");
    let mut plugin = UpmixerPlugin::from_params(params);
    plugin
        .initialize(sample_rate)
        .expect("Failed to initialize upmixer");
    let latency = plugin.latency_samples();
    println!(
        "  Latency: {latency} samples ({:.2} ms)",
        latency as f64 * 1000.0 / sample_rate as f64
    );
    println!("  Output channels: {out_channels}");

    let upmixed = process_upmixer(
        &mut plugin,
        &samples,
        total_frames,
        out_channels,
        sample_rate,
    );
    let upmixed = strip_latency(&upmixed, latency, out_channels);
    let output_frames = upmixed.len() / out_channels;
    println!("  Output frames: {output_frames}");

    // ── Write output WAV ────────────────────────────────────────────────
    let output_path = cli.output.unwrap_or_else(|| {
        let stem = cli.input.file_stem().unwrap_or_default().to_string_lossy();
        cli.input.with_file_name(format!("{}_upmixed.wav", stem))
    });

    let out_spec = hound::WavSpec {
        channels: out_channels as u16,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(&output_path, out_spec).unwrap_or_else(|e| {
        eprintln!("Failed to create output '{}': {e}", output_path.display());
        process::exit(1);
    });
    for &s in &upmixed {
        writer.write_sample(s).unwrap();
    }
    writer.finalize().unwrap();

    println!("\nOutput: {}", output_path.display());
    println!("  Channels: {out_channels}");
    println!("  Frames:   {output_frames}");
    println!("\n=== Done ===");
}

/// Process stereo input through the upmixer, producing multichannel output.
fn process_upmixer(
    plugin: &mut UpmixerPlugin,
    input: &[f32],
    total_frames: usize,
    out_ch: usize,
    sample_rate: u32,
) -> Vec<f32> {
    let in_ch = 2;
    let mut output = Vec::with_capacity(total_frames * out_ch);
    let mut pos = 0;
    while pos < total_frames {
        let frames = BLOCK_SIZE.min(total_frames - pos);
        let start = pos * in_ch;
        let end = start + frames * in_ch;
        let block_in = &input[start..end];
        let mut block_out = vec![0.0_f32; frames * out_ch];
        let ctx = ProcessContext::new(sample_rate, frames);
        plugin
            .process(block_in, &mut block_out, &ctx)
            .expect("upmixer process failed");
        output.extend_from_slice(&block_out);
        pos += frames;
    }
    output
}

/// Strip `latency` frames from the beginning of an interleaved buffer.
fn strip_latency(buf: &[f32], latency_frames: usize, channels: usize) -> Vec<f32> {
    let skip = latency_frames * channels;
    if skip < buf.len() {
        buf[skip..].to_vec()
    } else {
        buf.to_vec()
    }
}
