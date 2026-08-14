//! Cross-plugin DSP matrix for release evidence.
//!
//! This complements plugin-specific analytical/reference tests by proving that
//! every factory-exposed built-in concept honours its declared sample-rate and
//! channel contract on a deterministic signal. External and platform-I/O
//! plugins have dedicated negotiated-runtime suites and are not fabricated
//! here.

use sotf_plugins::factory::{PLUGIN_CATALOG, PluginCategory, create_plugin};
use sotf_plugins::plugin::ProcessContext;

const SAMPLE_RATES: &[u32] = &[44_100, 48_000, 88_200, 96_000, 192_000];
const FRAMES: usize = 480;
const BLOCK_SIZES: &[usize] = &[1, 31, 127, 480, 1_024, 4_093];

fn input_channels(plugin_type: &str) -> usize {
    match plugin_type {
        "mono_to_stereo" => 1,
        "ambisonics_decoder" => 4,
        _ => 2,
    }
}

fn fixture_params(plugin_type: &str, channels: usize, sample_rate: u32) -> serde_json::Value {
    let identity_matrix: Vec<f32> = (0..channels)
        .flat_map(|row| (0..channels).map(move |column| f32::from(row == column)))
        .collect();

    match plugin_type {
        "loudness_compensation" | "fletcher_munson" => serde_json::json!({
            "low_freq": 100.0,
            "high_freq": 10_000.0,
            "low_gain": 0.0,
            "high_gain": 0.0,
        }),
        "convolution" => serde_json::json!({
            "ir_file": "",
            "channel_gains": [],
            "mix": 1.0,
            "gain_db": 0.0,
        }),
        "downmix" => serde_json::json!({"input_channels": channels}),
        "binaural_decoder" => serde_json::json!({"input_channels": channels}),
        "crossover" => serde_json::json!({
            "type": "lr4",
            "frequency": 1_000.0,
            "output": "lowpass",
        }),
        "spectrum_analyzer" => serde_json::json!({
            "num_bins": 30,
            "min_freq": 20.0,
            "max_freq": 20_000.0_f32.min(sample_rate as f32 * 0.49),
            "smoothing": 0.0,
        }),
        "resampler" => serde_json::json!({
            "input_sample_rate": sample_rate,
            "output_sample_rate": sample_rate,
            "chunk_size": FRAMES,
        }),
        "matrix" => serde_json::json!({
            "input_channels": channels,
            "output_channels": channels,
            "matrix": identity_matrix,
        }),
        "band_split" => serde_json::json!({
            "num_bands": 2,
            "frequency": 1_000.0,
            "type": "LR24",
        }),
        "band_merge" => serde_json::json!({"bands": 2}),
        "beamformer" => serde_json::json!({"num_mics": channels}),
        "ambisonics_decoder" => serde_json::json!({
            "order": 1,
            "target_layout": "5.1",
        }),
        _ => serde_json::json!({}),
    }
}

fn interleaved_sine(channels: usize, frames: usize, sample_rate: u32) -> Vec<f32> {
    let mut input = vec![0.0_f32; channels * frames];
    for frame in 0..frames {
        let sample =
            (2.0 * std::f32::consts::PI * 440.0 * frame as f32 / sample_rate as f32).sin() * 0.2;
        for channel in 0..channels {
            input[frame * channels + channel] = sample * (1.0 - channel as f32 * 0.05);
        }
    }
    input
}

fn process_input_fixture(
    plugin_type: &str,
    sample_rate: u32,
    frames: usize,
    input: &[f32],
) -> Result<Vec<f32>, String> {
    let channels = input_channels(plugin_type);
    let mut plugin = create_plugin(
        plugin_type,
        &fixture_params(plugin_type, channels, sample_rate),
        channels,
        sample_rate,
    )?;
    plugin.initialize(sample_rate)?;

    if input.len() != channels * frames {
        return Err(format!(
            "fixture has {} samples, expected {}",
            input.len(),
            channels * frames
        ));
    }
    let output_frames = plugin.output_frames_for_input(frames);
    let mut output = vec![0.0_f32; output_frames * plugin.output_channels()];
    let produced = plugin.process(
        input,
        &mut output,
        &ProcessContext::new(sample_rate, frames),
    )?;

    if produced > output_frames {
        return Err(format!(
            "reported {produced} frames for an output capacity of {output_frames}"
        ));
    }
    if let Some((index, value)) = output
        .iter()
        .copied()
        .enumerate()
        .find(|(_, value)| !value.is_finite())
    {
        return Err(format!("non-finite output at sample {index}: {value}"));
    }
    Ok(output)
}

fn process_fixture(plugin_type: &str, sample_rate: u32, frames: usize) -> Result<(), String> {
    let input = interleaved_sine(input_channels(plugin_type), frames, sample_rate);
    process_input_fixture(plugin_type, sample_rate, frames, &input).map(|_| ())
}

#[test]
fn every_builtin_obeys_its_sample_rate_contract() {
    let mut failures = Vec::new();

    for entry in PLUGIN_CATALOG {
        if matches!(
            entry.category,
            PluginCategory::ExternalHost | PluginCategory::PlatformIo
        ) {
            continue;
        }

        for &sample_rate in SAMPLE_RATES {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                process_fixture(entry.canonical_type, sample_rate, FRAMES)
            }));
            let rnnoise_unsupported =
                entry.canonical_type == "speech_denoiser" && sample_rate != 48_000;

            match (rnnoise_unsupported, outcome) {
                (false, Ok(Ok(()))) => {}
                (true, Ok(Err(error))) if error.contains("48000") || error.contains("48") => {}
                (true, Ok(Ok(()))) => failures.push(format!(
                    "{}@{sample_rate} unexpectedly accepted RNNoise's unsupported rate",
                    entry.canonical_type
                )),
                (_, Ok(Err(error))) => failures.push(format!(
                    "{}@{sample_rate} failed: {error}",
                    entry.canonical_type
                )),
                (_, Err(payload)) => failures.push(format!(
                    "{}@{sample_rate} panicked: {}",
                    entry.canonical_type,
                    panic_payload_description(&payload)
                )),
            }
        }
    }

    assert!(
        failures.is_empty(),
        "sample-rate contract failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_builtin_obeys_its_block_size_contract() {
    let mut failures = Vec::new();

    for entry in PLUGIN_CATALOG {
        if matches!(
            entry.category,
            PluginCategory::ExternalHost | PluginCategory::PlatformIo
        ) {
            continue;
        }

        for &frames in BLOCK_SIZES {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                process_fixture(entry.canonical_type, 48_000, frames)
            }));
            match outcome {
                Ok(Ok(())) => {}
                Ok(Err(error)) => failures.push(format!(
                    "{}@{frames} frames failed: {error}",
                    entry.canonical_type
                )),
                Err(payload) => failures.push(format!(
                    "{}@{frames} frames panicked: {}",
                    entry.canonical_type,
                    panic_payload_description(&payload)
                )),
            }
        }
    }

    assert!(
        failures.is_empty(),
        "block-size contract failures:\n{}",
        failures.join("\n")
    );
}

#[derive(Debug, Clone, Copy)]
enum RobustnessSignal {
    Silence,
    ChannelImpulse,
    Dc,
    Step,
    Denormal,
}

fn robustness_input(signal: RobustnessSignal, channels: usize, frames: usize) -> Vec<f32> {
    let mut input = vec![0.0_f32; channels * frames];
    match signal {
        RobustnessSignal::Silence => {}
        RobustnessSignal::ChannelImpulse => {
            for channel in 0..channels {
                // Distinct frame and amplitude identify each source channel.
                let frame = channel.min(frames - 1);
                input[frame * channels + channel] = 1.0 - channel as f32 * 0.05;
            }
        }
        RobustnessSignal::Dc => {
            for frame in 0..frames {
                for channel in 0..channels {
                    input[frame * channels + channel] = 0.1 - channel as f32 * 0.005;
                }
            }
        }
        RobustnessSignal::Step => {
            for frame in frames / 2..frames {
                for channel in 0..channels {
                    input[frame * channels + channel] = 0.2 - channel as f32 * 0.01;
                }
            }
        }
        RobustnessSignal::Denormal => input.fill(f32::MIN_POSITIVE * 0.5),
    }
    input
}

#[test]
fn every_builtin_handles_release_robustness_signals() {
    const SIGNALS: &[RobustnessSignal] = &[
        RobustnessSignal::Silence,
        RobustnessSignal::ChannelImpulse,
        RobustnessSignal::Dc,
        RobustnessSignal::Step,
        RobustnessSignal::Denormal,
    ];

    let mut failures = Vec::new();
    for entry in PLUGIN_CATALOG {
        if matches!(
            entry.category,
            PluginCategory::ExternalHost | PluginCategory::PlatformIo
        ) {
            continue;
        }

        let channels = input_channels(entry.canonical_type);
        for &signal in SIGNALS {
            let input = robustness_input(signal, channels, FRAMES);
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                process_input_fixture(entry.canonical_type, 48_000, FRAMES, &input)
            }));
            match outcome {
                Ok(Ok(output)) => {
                    let peak = output.iter().map(|sample| sample.abs()).fold(0.0, f32::max);
                    if peak > 64.0 {
                        failures.push(format!(
                            "{} {signal:?} produced an unbounded peak of {peak}",
                            entry.canonical_type
                        ));
                    }
                }
                Ok(Err(error)) => failures.push(format!(
                    "{} {signal:?} failed: {error}",
                    entry.canonical_type
                )),
                Err(payload) => failures.push(format!(
                    "{} {signal:?} panicked: {}",
                    entry.canonical_type,
                    panic_payload_description(&payload)
                )),
            }
        }
    }

    assert!(
        failures.is_empty(),
        "release robustness signal failures:\n{}",
        failures.join("\n")
    );
}

fn panic_payload_description(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_owned()
    }
}
