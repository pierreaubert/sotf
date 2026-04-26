// ============================================================================
// Full Mono Restoration Pipeline
// ============================================================================
//
// Takes a mono WAV input (old recording) and produces a restored stereo WAV
// by chaining the dedicated repair plugins:
//
//   1. Declick      — repairs short clicks / pops in the time domain
//   2. Hiss reducer — attenuates stationary high-frequency hiss
//   3. Denoiser     — removes broadband noise (Wiener / MCRA)
//   4. PND          — corrects wow and flutter (pitch drift)
//   5. Mono→Stereo  — widens the mono signal into natural-sounding stereo
//
// Run with:
//   cargo run -p sotf-plugins --example restore_mono --release -- input.wav output.wav

use sotf_host::plugin::{InPlacePluginAdapter, Plugin, ProcessContext};
use sotf_plugin_declick::{DeclickPlugin, DeclickPluginParams};
use sotf_plugin_denoiser::{DenoiserData, DenoiserPlugin, DenoiserPluginParams};
use sotf_plugin_hiss_reducer::{HissReducerPlugin, HissReducerPluginParams};
use sotf_plugin_mono_to_stereo::{MonoToStereoPlugin, MonoToStereoPluginParams};
use sotf_plugin_pnd::{PndData, PndPlugin, PndPluginParams};
use std::env;
use std::process;

/// Processing block size in frames.
const BLOCK_SIZE: usize = 4096;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: restore_mono <input.wav> <output.wav>");
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
    let in_channels = spec.channels as usize;

    println!("=== Mono Restoration Pipeline ===\n");
    println!("Input:  {input_path}");
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
    println!();

    // If input is stereo+, collapse to mono for the pipeline.
    let mono_samples: Vec<f32> = if in_channels == 1 {
        samples.clone()
    } else {
        (0..total_frames)
            .map(|f| {
                let mut sum = 0.0_f32;
                for ch in 0..in_channels {
                    sum += samples[f * in_channels + ch];
                }
                sum / in_channels as f32
            })
            .collect()
    };

    // ── Stage 1: Declick ────────────────────────────────────────────────
    println!("--- Stage 1: Declick ---");
    let declick_params = DeclickPluginParams {
        enabled: true,
        sensitivity: 5.0,
    };
    let declick = DeclickPlugin::from_params(1, declick_params);
    let mut declick_plugin = InPlacePluginAdapter::new(declick);
    declick_plugin
        .initialize(sample_rate)
        .expect("Failed to initialize declick");
    let declick_latency = declick_plugin.latency_samples();
    let declicked = process_plugin_mono(
        &mut declick_plugin,
        &mono_samples,
        total_frames,
        sample_rate,
    );
    let declicked = strip_latency(&declicked, declick_latency, 1);
    println!("  Output frames: {}", declicked.len());

    // ── Stage 2: Hiss reducer ───────────────────────────────────────────
    println!("\n--- Stage 2: Hiss reducer ---");
    let hiss_params = HissReducerPluginParams {
        enabled: true,
        threshold_db: -35.0,
        frequency_hz: 3000.0,
        strength: 0.7,
        low_latency: false,
    };
    let hiss = HissReducerPlugin::from_params(1, hiss_params);
    let mut hiss_plugin = InPlacePluginAdapter::new(hiss);
    hiss_plugin
        .initialize(sample_rate)
        .expect("Failed to initialize hiss reducer");
    let hiss_latency = hiss_plugin.latency_samples();
    let hiss_frames = declicked.len();
    let dehissed = process_plugin_mono(&mut hiss_plugin, &declicked, hiss_frames, sample_rate);
    let dehissed = strip_latency(&dehissed, hiss_latency, 1);
    println!("  Output frames: {}", dehissed.len());

    // ── Stage 3: Denoiser ───────────────────────────────────────────────
    println!("\n--- Stage 3: Denoiser ---");
    let denoiser_params = DenoiserPluginParams {
        reduction_db: 18.0,
        floor_db: -50.0,
        smoothing: 0.85,
        attack_ms: 1.0,
        release_ms: 50.0,
        low_latency: false,
        spectral_sub_enabled: true,
        spectral_smoothing_enabled: true,
        temporal_smoothing_enabled: true,
        multi_resolution: true,
        ..DenoiserPluginParams::default()
    };

    let denoiser = DenoiserPlugin::from_params(1, denoiser_params);
    let mut denoiser_plugin = InPlacePluginAdapter::new(denoiser);
    denoiser_plugin
        .initialize(sample_rate)
        .expect("Failed to initialize denoiser");
    let denoiser_latency = denoiser_plugin.latency_samples();
    println!(
        "  Latency: {denoiser_latency} samples ({:.2} ms)",
        denoiser_latency as f64 * 1000.0 / sample_rate as f64
    );

    let denoiser_frames = dehissed.len();
    let denoised = process_plugin_mono(
        &mut denoiser_plugin,
        &dehissed,
        denoiser_frames,
        sample_rate,
    );

    if let Some(data) = denoiser_plugin.get_data()
        && let Some(d) = data.downcast_ref::<DenoiserData>()
    {
        println!("  Avg reduction: {:.1} dB", d.avg_reduction_db);
    }

    let denoised = strip_latency(&denoised, denoiser_latency, 1);
    println!("  Output frames: {}", denoised.len());

    // ── Stage 4: PND (wow/flutter removal) ──────────────────────────────
    println!("\n--- Stage 4: PND (Wow & Flutter) ---");
    let pnd_params = PndPluginParams {
        correction_strength: 0.9,
        drift_smoothing: 0.85,
        confidence_threshold: 0.3,
        phase_vocoder: true,
        ..PndPluginParams::default()
    };

    let mut pnd_plugin = PndPlugin::from_params(1, pnd_params);
    pnd_plugin
        .initialize(sample_rate)
        .expect("Failed to initialize PND");
    let pnd_latency = pnd_plugin.latency_samples();
    println!(
        "  Latency: {pnd_latency} samples ({:.2} ms)",
        pnd_latency as f64 * 1000.0 / sample_rate as f64
    );

    let pnd_frames = denoised.len();
    let stabilised =
        process_plugin_variable(&mut pnd_plugin, &denoised, pnd_frames, 1, 1, sample_rate);

    if let Some(data) = pnd_plugin.get_data()
        && let Some(d) = data.downcast_ref::<PndData>()
    {
        println!("  Last drift:  {:.6}", d.drift_ratio);
        println!("  Confidence:  {:.3}", d.confidence);
    }

    let stabilised = strip_latency(&stabilised, pnd_latency, 1);
    println!("  Output frames: {}", stabilised.len());

    // ── Stage 5: Mono → Stereo ──────────────────────────────────────────
    println!("\n--- Stage 5: Mono to Stereo ---");
    let m2s_params = MonoToStereoPluginParams {
        stereo_width: 0.6, // moderate width — natural, not exaggerated
        freq_dependent: true,
        haas_delay_ms: 0.8, // subtle Haas delay
    };

    let mut m2s_plugin = MonoToStereoPlugin::from_params(1, m2s_params);
    m2s_plugin
        .initialize(sample_rate)
        .expect("Failed to initialize Mono→Stereo");
    let m2s_latency = m2s_plugin.latency_samples();
    let out_channels = m2s_plugin.output_channels();
    println!(
        "  Latency: {m2s_latency} samples ({:.2} ms)",
        m2s_latency as f64 * 1000.0 / sample_rate as f64
    );
    println!("  Output channels: {out_channels}");

    let m2s_frames = stabilised.len();
    let stereo = process_plugin_variable(
        &mut m2s_plugin,
        &stabilised,
        m2s_frames,
        1,
        out_channels,
        sample_rate,
    );

    let stereo = strip_latency(&stereo, m2s_latency, out_channels);
    let output_frames = stereo.len() / out_channels;
    println!("  Output frames: {output_frames}");

    // ── Summary ─────────────────────────────────────────────────────────
    let total_latency =
        declick_latency + hiss_latency + denoiser_latency + pnd_latency + m2s_latency;
    println!("\n--- Summary ---");
    println!(
        "  Total pipeline latency: {total_latency} samples ({:.2} ms)",
        total_latency as f64 * 1000.0 / sample_rate as f64
    );
    println!("  Input:  {total_frames} frames mono ({duration_s:.2} s)");
    println!(
        "  Output: {output_frames} frames stereo ({:.2} s)",
        output_frames as f64 / sample_rate as f64
    );

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
    for &s in &stereo {
        writer.write_sample(s).unwrap();
    }
    writer.finalize().unwrap();

    println!("\nOutput: {output_path}");
    println!("\n=== Done ===");
}

/// Process a mono InPlacePluginAdapter (wraps InPlacePlugin → Plugin).
fn process_plugin_mono(
    plugin: &mut InPlacePluginAdapter<impl sotf_host::plugin::InPlacePlugin>,
    input: &[f32],
    total_frames: usize,
    sample_rate: u32,
) -> Vec<f32> {
    let mut output = Vec::with_capacity(total_frames);
    let mut pos = 0;
    while pos < total_frames {
        let frames = BLOCK_SIZE.min(total_frames - pos);
        let block_in = &input[pos..pos + frames];
        let mut block_out = vec![0.0_f32; frames];
        let ctx = ProcessContext {
            sample_rate,
            num_frames: frames,
        };
        plugin
            .process(block_in, &mut block_out, &ctx)
            .expect("denoiser process failed");
        output.extend_from_slice(&block_out);
        pos += frames;
    }
    output
}

/// Process a Plugin with potentially different input/output channel counts.
fn process_plugin_variable(
    plugin: &mut dyn Plugin,
    input: &[f32],
    total_frames: usize,
    in_ch: usize,
    out_ch: usize,
    sample_rate: u32,
) -> Vec<f32> {
    let mut output = Vec::with_capacity(total_frames * out_ch);
    let mut pos = 0;
    while pos < total_frames {
        let frames = BLOCK_SIZE.min(total_frames - pos);
        let start = pos * in_ch;
        let end = start + frames * in_ch;
        let block_in = &input[start..end];
        let mut block_out = vec![0.0_f32; frames * out_ch];
        let ctx = ProcessContext {
            sample_rate,
            num_frames: frames,
        };
        plugin
            .process(block_in, &mut block_out, &ctx)
            .expect("plugin process failed");
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
