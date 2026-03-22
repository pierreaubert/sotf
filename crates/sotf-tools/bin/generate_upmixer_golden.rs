//! Generate golden reference files for upmixer plugin regression testing.
//!
//! This binary processes stereo audio through the upmixer plugin with various
//! configurations and saves the outputs as WAV files for regression testing.
//!
//! Usage:
//!     cargo run --bin generate_upmixer_golden --release
//!
//! Output: data_generated/test-plugin-upmixer/

use clap::Parser;
use hound::{SampleFormat, WavSpec, WavWriter};
use serde::{Deserialize, Serialize};
use sotf_plugins::plugin::{Plugin, ProcessContext};
use sotf_plugins::plugin_upmixer::UpmixerPlugin;
use std::fs;
use std::path::{Path, PathBuf};

const OUTPUT_DIR: &str = "data_generated/test-plugin-upmixer";

const CONFIGS: &[(&str, &str)] = &[
    ("5.1", "5.1 surround"),
    ("7.1", "7.1 surround"),
    ("5.1.2", "5.1.2 with height"),
    ("7.1.4", "7.1.4 with height"),
    ("9.1.6", "9.1.6 immersive"),
];

const SIGNALS: &[SignalDef] = &[
    SignalDef {
        name: "multisine",
        description: "Multi-sine from 40Hz to 16kHz",
    },
    SignalDef {
        name: "sweep_20_20k",
        description: "Logarithmic sweep 20Hz to 20kHz",
    },
    SignalDef {
        name: "dialogue",
        description: "Voice-like signal for dialogue testing",
    },
    SignalDef {
        name: "pink_noise",
        description: "Pink noise for diffuse content",
    },
];

#[derive(Clone)]
struct SignalDef {
    name: &'static str,
    description: &'static str,
}

#[derive(Parser)]
#[command(name = "generate_upmixer_golden")]
#[command(about = "Generate golden reference files for upmixer regression tests")]
struct Cli {
    /// Output directory
    #[arg(long, default_value = OUTPUT_DIR)]
    out_dir: PathBuf,

    /// Sample rate for generated files
    #[arg(long, default_value_t = 48000)]
    sample_rate: u32,

    /// FFT size for processing
    #[arg(long, default_value_t = 2048)]
    fft_size: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    generator: String,
    version: String,
    sample_rate: u32,
    fft_size: usize,
    configs: Vec<ConfigEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ConfigEntry {
    config: String,
    description: String,
    signals: Vec<SignalEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SignalEntry {
    name: String,
    description: String,
    output_file: String,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    println!(
        "Generating upmixer golden files to {}",
        cli.out_dir.display()
    );

    // Create output directory
    fs::create_dir_all(&cli.out_dir)?;

    let mut manifest = Manifest {
        generator: "generate_upmixer_golden".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        sample_rate: cli.sample_rate,
        fft_size: cli.fft_size,
        configs: Vec::new(),
    };

    // Generate for each configuration
    for (config_id, config_desc) in CONFIGS {
        println!("\nGenerating config: {}", config_id);

        let config_entry = generate_config(
            &cli.out_dir,
            config_id,
            config_desc,
            cli.sample_rate,
            cli.fft_size,
        )?;
        manifest.configs.push(config_entry);
    }

    // Write manifest
    let manifest_path = cli.out_dir.join("manifest.json");
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    fs::write(&manifest_path, manifest_json)?;

    println!("\nDone! Manifest: {}", manifest_path.display());

    Ok(())
}

fn generate_config(
    out_dir: &Path,
    config_id: &str,
    config_desc: &str,
    sample_rate: u32,
    fft_size: usize,
) -> anyhow::Result<ConfigEntry> {
    let config_dir = out_dir.join(config_id);
    fs::create_dir_all(&config_dir)?;

    let mut config_entry = ConfigEntry {
        config: config_id.to_string(),
        description: config_desc.to_string(),
        signals: Vec::new(),
    };

    // Create plugin using new() with valid parameters
    let mut plugin = UpmixerPlugin::new(
        fft_size, config_id, 1.0,   // gain_front_direct
        0.5,   // gain_front_ambient
        1.0,   // gain_rear_ambient
        120.0, // lfe_cutoff_hz
        0.5,   // stereo_width
        250.0, // bandpass_hz
        0.5,   // height_gain
        1.0,   // lfe_gain
        false, // enable_subharmonic_synth
        0.5,   // subharmonic_gain
    );

    plugin
        .initialize(sample_rate)
        .map_err(|e| anyhow::anyhow!("initialize failed: {}", e))?;

    let num_output_channels = plugin.output_channels();
    println!(
        "  Config {} -> {} output channels",
        config_id, num_output_channels
    );

    // Generate each signal
    for signal in SIGNALS {
        let output_file = format!("{}.wav", signal.name);
        let output_path = config_dir.join(&output_file);

        println!("    Signal: {} -> {}", signal.name, output_file);

        // Generate input signal (process multiple blocks for longer output)
        let num_blocks = 10;
        let num_frames = fft_size * num_blocks;
        let input = generate_signal(signal.name, sample_rate, num_frames)?;

        // Process through upmixer using PluginHost-like approach
        let output = process_upmixer(
            &mut plugin,
            &input,
            sample_rate,
            fft_size,
            num_output_channels,
        )?;

        // Write WAV
        write_wav(
            &output_path,
            &output,
            sample_rate,
            num_output_channels as u16,
            32,
        )?;

        config_entry.signals.push(SignalEntry {
            name: signal.name.to_string(),
            description: signal.description.to_string(),
            output_file: format!("{}/{}", config_id, output_file),
        });
    }

    Ok(config_entry)
}

fn generate_signal(name: &str, sample_rate: u32, num_frames: usize) -> anyhow::Result<Vec<f32>> {
    // Ensure num_frames is a multiple of fft_size for block processing
    let fft_size = 2048;
    let num_blocks = num_frames.div_ceil(fft_size);
    let actual_frames = num_blocks * fft_size;
    let mut data = vec![0.0_f32; actual_frames * 2]; // Stereo

    match name {
        "multisine" => {
            // Multi-sine from 40Hz to 16kHz
            let freqs = [
                40.0, 80.0, 160.0, 320.0, 640.0, 1280.0, 2560.0, 5120.0, 10240.0, 16000.0,
            ];
            for (i, sample) in data.iter_mut().enumerate() {
                let t = i as f32 / sample_rate as f32;
                let ch = i % 2;
                let mut sum = 0.0_f32;
                for (fi, &freq) in freqs.iter().enumerate() {
                    let phase = 2.0 * std::f32::consts::PI * freq * t + (fi as f32 * 0.1);
                    sum += phase.sin() * 0.1;
                }
                *sample = if ch == 0 { sum } else { sum * 0.95 }; // Slight stereo difference
            }
        }
        "sweep_20_20k" => {
            // Logarithmic sweep
            let log_start = 20.0_f32.ln();
            let log_end = 20000.0_f32.ln();
            for i in 0..actual_frames {
                let t = i as f32 / sample_rate as f32;
                let freq = (log_start + t * (log_end - log_start)).exp();
                let phase = 2.0 * std::f32::consts::PI * freq * t;
                data[i * 2] = phase.sin() * 0.5; // L
                data[i * 2 + 1] = phase.sin() * 0.5; // R
            }
        }
        "dialogue" => {
            // Voice-like: fundamental + harmonics with envelope
            let fundamental = 180.0_f32;
            for i in 0..actual_frames {
                let t = i as f32 / sample_rate as f32;
                // Simple envelope (syllable-like)
                let envelope = ((t * 3.0).sin() * 0.5 + 0.5).max(0.0);
                let mut sum = 0.0_f32;
                // Harmonics
                for h in 1..=6 {
                    let freq = fundamental * h as f32;
                    let phase = 2.0 * std::f32::consts::PI * freq * t;
                    sum += phase.sin() * (1.0 / h as f32);
                }
                let sample = sum * envelope * 0.3;
                data[i * 2] = sample;
                data[i * 2 + 1] = sample * 0.98; // Slight stereo difference
            }
        }
        "pink_noise" => {
            // Simple pink noise approximation
            let mut b0 = 0.0_f32;
            let mut b1 = 0.0_f32;
            let mut b2 = 0.0_f32;
            let mut b3 = 0.0_f32;
            let mut b4 = 0.0_f32;
            let mut b5 = 0.0_f32;
            let mut b6 = 0.0_f32;

            let mut seed = 12345u32;
            let mut rand_f32 = || {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
            };

            for i in 0..actual_frames {
                let white = rand_f32();
                b0 = 0.99886 * b0 + white * 0.0555179;
                b1 = 0.99332 * b1 + white * 0.0750759;
                b2 = 0.96900 * b2 + white * 0.153_852;
                b3 = 0.86650 * b3 + white * 0.3104856;
                b4 = 0.55000 * b4 + white * 0.5329522;
                b5 = -0.7616 * b5 - white * 0.0168980;
                let pink = b0 + b1 + b2 + b3 + b4 + b5 + b6 + white * 0.5362;
                b6 = white * 0.115926;

                let sample = pink * 0.11; // Normalize
                data[i * 2] = sample;
                data[i * 2 + 1] = sample * 0.98;
            }
        }
        _ => anyhow::bail!("Unknown signal: {}", name),
    }

    Ok(data)
}

fn process_upmixer(
    plugin: &mut UpmixerPlugin,
    input: &[f32],
    sample_rate: u32,
    fft_size: usize,
    num_output_channels: usize,
) -> anyhow::Result<Vec<f32>> {
    // Process block by block - each call processes exactly fft_size frames
    let num_blocks = input.len() / (fft_size * 2);
    let num_frames = num_blocks * fft_size;
    let mut output = vec![0.0_f32; num_frames * num_output_channels];

    for block in 0..num_blocks {
        let input_offset = block * fft_size * 2;
        let output_offset = block * fft_size * num_output_channels;

        let input_block = &input[input_offset..input_offset + fft_size * 2];
        let mut output_block = vec![0.0_f32; fft_size * num_output_channels];

        let context = ProcessContext {
            sample_rate,
            num_frames: fft_size,
        };

        plugin
            .process(input_block, &mut output_block, &context)
            .map_err(|e| anyhow::anyhow!("process failed: {}", e))?;

        // Copy to output
        output[output_offset..output_offset + output_block.len()].copy_from_slice(&output_block);
    }

    Ok(output)
}

fn write_wav(
    path: &PathBuf,
    data: &[f32],
    sr: u32,
    channels: u16,
    bits: u16,
) -> anyhow::Result<()> {
    let spec = WavSpec {
        channels,
        sample_rate: sr,
        bits_per_sample: bits,
        sample_format: SampleFormat::Float,
    };

    let mut writer = WavWriter::create(path, spec)?;

    for &sample in data {
        writer.write_sample(sample)?;
    }

    writer.finalize()?;

    Ok(())
}
