// ============================================================================
// PND Plugin Demo — Wow & Flutter Removal
// ============================================================================
//
// Takes a mono WAV input file (e.g. an old tape or vinyl recording suffering
// from wow and flutter — slow pitch drift) and writes a stabilised mono WAV.
//
// Run with:
//   cargo run -p sotf-plugin-pnd --example pnd_demo --release -- input.wav output.wav

use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_pnd::{PndData, PndPlugin, PndPluginParams};
use std::env;
use std::process;

/// Processing block size in frames.
const BLOCK_SIZE: usize = 4096;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: pnd_demo <input.wav> <output.wav>");
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

    println!("=== PND — Wow & Flutter Removal ===\n");
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

    // ── Configure PND for wow/flutter correction ────────────────────────
    let params = PndPluginParams {
        correction_strength: 0.0,
        drift_smoothing: 0.85,
        confidence_threshold: 0.3,
        phase_vocoder: false,
        ..PndPluginParams::default()
    };

    println!("PND configuration:");
    println!("  Correction strength:  {}", params.correction_strength);
    println!("  Drift smoothing:      {}", params.drift_smoothing);
    println!("  Confidence threshold: {}", params.confidence_threshold);
    println!("  Phase vocoder:        {}", params.phase_vocoder);
    println!();

    let mut plugin = PndPlugin::from_params(channels, params);

    plugin
        .initialize(sample_rate)
        .expect("Failed to initialize PND");

    let latency = plugin.latency_samples();
    println!(
        "Plugin latency: {latency} samples ({:.2} ms)",
        latency as f64 * 1000.0 / sample_rate as f64
    );

    // ── Process in blocks ───────────────────────────────────────────────
    // PND implements Plugin (not InPlacePlugin) — same in/out channels.
    let out_channels = plugin.output_channels();
    let mut output_samples: Vec<f32> = Vec::with_capacity(total_frames * out_channels);
    let mut pos = 0;

    while pos < total_frames {
        let frames_this_block = BLOCK_SIZE.min(total_frames - pos);
        let sample_start = pos * channels;
        let sample_end = sample_start + frames_this_block * channels;
        let input_block = &samples[sample_start..sample_end];

        let mut output_block = vec![0.0_f32; frames_this_block * out_channels];
        let context = ProcessContext::new(sample_rate, frames_this_block);

        plugin
            .process(input_block, &mut output_block, &context)
            .expect("Failed to process block");

        output_samples.extend_from_slice(&output_block);
        pos += frames_this_block;
    }

    // Strip plugin latency.
    let skip_samples = latency * out_channels;
    let output_trimmed = if skip_samples < output_samples.len() {
        &output_samples[skip_samples..]
    } else {
        &output_samples[..]
    };
    let output_frames = output_trimmed.len() / out_channels;

    // ── Show monitoring data ────────────────────────────────────────────
    if let Some(data) = plugin.get_data()
        && let Some(d) = data.downcast_ref::<PndData>()
    {
        println!();
        println!("PND monitoring (last block):");
        println!("  Drift ratio:      {:.6}", d.drift_ratio);
        println!("  Correction ratio: {:.6}", d.correction_ratio);
        println!("  Confidence:       {:.3}", d.confidence);
        println!("  Matched partials: {}", d.matched_partials);
        println!("  Total peaks:      {}", d.total_peaks);
    }

    // ── Write output WAV ────────────────────────────────────────────────
    let out_spec = hound::WavSpec {
        channels: out_channels as u16,
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
