// ============================================================================
// Denoiser Plugin Demo — Vinyl/Old Recording Restoration
// ============================================================================
//
// Takes a mono WAV input file (e.g. an old recording with crackles, hiss, and
// background noise) and writes a cleaned mono WAV output.
//
// Run with:
//   cargo run -p sotf-plugin-denoiser --example denoiser_demo --release -- input.wav output.wav

use sotf_host::plugin::{InPlacePluginAdapter, Plugin, ProcessContext};
use sotf_plugin_denoiser::{DenoiserData, DenoiserPlugin, DenoiserPluginParams};
use std::env;
use std::process;

/// Processing block size in frames.
const BLOCK_SIZE: usize = 4096;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: denoiser_demo <input.wav> <output.wav>");
        process::exit(1);
    }
    let input_path = &args[1];
    let output_path = &args[2];

    // ── Read input WAV ──────────────────────────────────────────────────
    let mut reader = hound::WavReader::open(input_path).unwrap_or_else(|e| {
        eprintln!("Failed to open {input_path}: {e}");
        process::exit(1);
    });
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels as usize;

    println!("=== Denoiser — Old Recording Restoration ===\n");
    println!("Input:  {input_path}");
    println!("  Sample rate:  {sample_rate} Hz");
    println!("  Channels:     {channels}");
    println!(
        "  Bit depth:    {} ({})",
        spec.bits_per_sample,
        match spec.sample_format {
            hound::SampleFormat::Float => "float",
            hound::SampleFormat::Int => "int",
        }
    );

    // Read all samples into a flat interleaved f32 buffer.
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
    let total_frames = samples.len() / channels;
    let duration_s = total_frames as f64 / sample_rate as f64;
    println!("  Duration:     {duration_s:.2} s ({total_frames} frames)");
    println!();

    // ── Configure denoiser for old-recording restoration ────────────────
    let params = DenoiserPluginParams {
        // Moderate noise reduction — aggressive enough for old recordings
        // without destroying the signal.
        reduction_db: 18.0,
        floor_db: -50.0,
        smoothing: 0.85,
        attack_ms: 1.0,
        release_ms: 50.0,
        low_latency: false, // use larger FFT for better quality
        // Crackle / click removal
        transient_enabled: true,
        crack_sensitivity: 5.0,
        // Hiss removal (tape hiss, surface noise)
        hiss_enabled: true,
        hiss_threshold_db: -35.0,
        hiss_frequency_hz: 3000.0,
        hiss_strength: 0.7,
        // Spectral subtraction for broadband noise
        spectral_sub_enabled: true,
        // Temporal + spectral smoothing to reduce musical noise artifacts
        spectral_smoothing_enabled: true,
        temporal_smoothing_enabled: true,
        // Multi-resolution for better transient tracking
        multi_resolution: true,
        ..DenoiserPluginParams::default()
    };

    println!("Denoiser configuration:");
    println!("  Reduction:           {} dB", params.reduction_db);
    println!("  Floor:               {} dB", params.floor_db);
    println!(
        "  Crackle removal:     on (sensitivity {})",
        params.crack_sensitivity
    );
    println!(
        "  Hiss removal:        on (>{} Hz, strength {})",
        params.hiss_frequency_hz, params.hiss_strength
    );
    println!("  Spectral sub:        on");
    println!("  Multi-resolution:    on");
    println!();

    let denoiser = DenoiserPlugin::from_params(channels, params);
    let mut plugin = InPlacePluginAdapter::new(denoiser);

    plugin
        .initialize(sample_rate)
        .expect("Failed to initialize denoiser");

    let latency = plugin.latency_samples();
    println!(
        "Plugin latency: {latency} samples ({:.2} ms)",
        latency as f64 * 1000.0 / sample_rate as f64
    );

    // ── Process in blocks ───────────────────────────────────────────────
    let mut output_samples: Vec<f32> = Vec::with_capacity(samples.len());
    let mut pos = 0;

    while pos < total_frames {
        let frames_this_block = BLOCK_SIZE.min(total_frames - pos);
        let sample_start = pos * channels;
        let sample_end = sample_start + frames_this_block * channels;
        let input_block = &samples[sample_start..sample_end];

        let mut output_block = vec![0.0_f32; frames_this_block * channels];
        let context = ProcessContext {
            sample_rate,
            num_frames: frames_this_block,
        };

        plugin
            .process(input_block, &mut output_block, &context)
            .expect("Failed to process block");

        output_samples.extend_from_slice(&output_block);
        pos += frames_this_block;
    }

    // Strip plugin latency from the start and trim the end to match.
    let skip_samples = latency * channels;
    let output_trimmed = if skip_samples < output_samples.len() {
        &output_samples[skip_samples..]
    } else {
        &output_samples[..]
    };
    let output_frames = output_trimmed.len() / channels;

    // ── Compute energy stats ────────────────────────────────────────────
    let usable_input = if skip_samples < samples.len() {
        &samples[skip_samples..]
    } else {
        &samples[..]
    };
    let len = usable_input.len().min(output_trimmed.len());
    let input_energy: f64 = usable_input[..len]
        .iter()
        .map(|&x| (x as f64).powi(2))
        .sum();
    let output_energy: f64 = output_trimmed[..len]
        .iter()
        .map(|&x| (x as f64).powi(2))
        .sum();

    println!();
    println!("Results:");
    println!("  Input energy:  {input_energy:.4}");
    println!("  Output energy: {output_energy:.4}");
    if input_energy > 0.0 {
        let ratio = output_energy / input_energy;
        println!(
            "  Energy ratio:  {ratio:.3} ({:.1} dB)",
            10.0 * ratio.log10()
        );
    }

    // Show denoiser monitoring data.
    if let Some(data) = plugin.get_data()
        && let Some(d) = data.downcast_ref::<DenoiserData>()
    {
        println!("  Avg reduction: {:.1} dB", d.avg_reduction_db);
        println!("  Learning:      {}", d.learning_active);
    }

    // ── Write output WAV ────────────────────────────────────────────────
    let out_spec = hound::WavSpec {
        channels: channels as u16,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(output_path, out_spec).unwrap_or_else(|e| {
        eprintln!("Failed to create {output_path}: {e}");
        process::exit(1);
    });
    for &s in output_trimmed {
        writer.write_sample(s).unwrap();
    }
    writer.finalize().unwrap();

    println!();
    println!("Output: {output_path}");
    println!(
        "  Frames: {output_frames} ({:.2} s)",
        output_frames as f64 / sample_rate as f64
    );
    println!("\n=== Done ===");
}
