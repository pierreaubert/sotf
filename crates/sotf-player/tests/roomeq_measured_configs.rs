//! DSP execution tests for precomputed measured RoomEQ configurations.
//!
//! Fixtures under `tests/fixtures/roomeq_measured/` are slim copies of the
//! approved artifacts in `../autoeq/data_generated/roomeq/measured/`
//! (regenerate with `scripts/slim-measured-roomeq-fixtures.py`): two real
//! measured rooms — a 2.0 setup and a 5.1.4 home cinema — each in the four
//! optimizer modes (iir, fir, mixed, mixed-phase). Every approved chain is
//! built with the engine's own graph builder and exercised with real
//! audio blocks; linear (unrouted) chains additionally have their measured
//! magnitude response compared against the stored per-channel EQ curves.
//! The one configuration the optimizer rejected upstream
//! (5.1.4_genelec/mixed) is asserted to stay unpublished.

use std::collections::HashSet;
use std::path::PathBuf;

use sotf_audio::engine::build_plugin_graph_host;
use sotf_audio_player::autoeq::DspChainOutput;
use sotf_audio_player::room_eq_types::build_room_eq_plugin_graph_config;
use sotf_plugins::PluginHost;

const SAMPLE_RATE: u32 = 48_000;
// Half a second of impulse capture resolves the correction band while
// keeping the direct DFT evaluation cheap.
const CAPTURE_FRAMES: usize = 24_000;
// Stored EQ grids are evaluated at most at this many log-spaced points.
const MAX_EVAL_POINTS: usize = 80;

struct MeasuredConfig {
    dataset: &'static str,
    mode: &'static str,
    input_channels: usize,
}

const CONFIGS: &[MeasuredConfig] = &[
    MeasuredConfig {
        dataset: "2.0_t7v_2026",
        mode: "iir",
        input_channels: 2,
    },
    MeasuredConfig {
        dataset: "2.0_t7v_2026",
        mode: "fir",
        input_channels: 2,
    },
    MeasuredConfig {
        dataset: "2.0_t7v_2026",
        mode: "mixed",
        input_channels: 2,
    },
    MeasuredConfig {
        dataset: "2.0_t7v_2026",
        mode: "mixed-phase",
        input_channels: 2,
    },
    MeasuredConfig {
        dataset: "5.1.4_genelec",
        mode: "iir",
        input_channels: 10,
    },
    MeasuredConfig {
        dataset: "5.1.4_genelec",
        mode: "fir",
        input_channels: 10,
    },
    MeasuredConfig {
        dataset: "5.1.4_genelec",
        mode: "mixed-phase",
        input_channels: 10,
    },
];

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("roomeq_measured")
}

fn load_chain(config: &MeasuredConfig) -> DspChainOutput {
    let path = fixture_dir()
        .join(config.dataset)
        .join(format!("dsp-{}.json", config.mode));
    let bytes =
        std::fs::read(&path).unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("parse fixture {}: {e}", path.display()))
}

fn load_eq_csv(config: &MeasuredConfig, channel: &str) -> Vec<(f64, f64)> {
    let path = fixture_dir()
        .join(config.dataset)
        .join(format!("{}-eq", config.mode))
        .join(format!("{channel}__eq.csv"));
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let mut columns = line.split(',');
            let freq: f64 = columns.next()?.parse().ok()?;
            let spl: f64 = columns.next()?.parse().ok()?;
            Some((freq, spl))
        })
        .collect()
}

fn stored_eq_at(curve: &[(f64, f64)], target_hz: f64) -> f64 {
    curve
        .iter()
        .min_by(|(a, _), (b, _)| {
            (a.ln() - target_hz.ln())
                .abs()
                .total_cmp(&(b.ln() - target_hz.ln()).abs())
        })
        .map(|(_, spl)| *spl)
        .expect("eq curve must not be empty")
}

/// Process deterministic multi-sine programme material in varied block
/// sizes (including a partial final block) and assert every output sample
/// is finite and bounded. Returns total output energy across channels.
fn run_programme(host: &mut PluginHost, channels: usize) -> f64 {
    let frames = 4_096usize;
    let mut input = vec![0.0f32; frames * channels];
    for frame in 0..frames {
        let t = frame as f32 / SAMPLE_RATE as f32;
        let sample = 0.3 * (std::f32::consts::TAU * 120.0 * t).sin()
            + 0.2 * (std::f32::consts::TAU * 1_000.0 * t).sin()
            + 0.1 * (std::f32::consts::TAU * 6_000.0 * t).sin();
        for channel in 0..channels {
            input[frame * channels + channel] = sample;
        }
    }

    let out_channels = host.output_channels();
    let mut energy = 0.0f64;
    let mut offset = 0usize;
    let block_pattern = [512usize, 256, 333, 1024];
    let mut pattern_index = 0usize;
    while offset < frames {
        let block = block_pattern[pattern_index % block_pattern.len()].min(frames - offset);
        pattern_index += 1;
        let mut output = vec![0.0f32; block * out_channels];
        let produced = host
            .process(
                &input[offset * channels..(offset + block) * channels],
                &mut output,
            )
            .expect("graph processing must not fail");
        assert!(produced <= block, "host over-reported frames");
        for sample in &output[..produced * out_channels] {
            assert!(sample.is_finite(), "non-finite output sample");
            assert!(
                sample.abs() < 100.0,
                "output sample {sample} out of sane bounds"
            );
            energy += f64::from(*sample) * f64::from(*sample);
        }
        offset += block;
    }
    energy
}

/// Capture the impulse response for one excited input channel: an impulse
/// on frame 0 followed by silence, plus the drained tail.
fn capture_impulse_response(
    host: &mut PluginHost,
    input_channel: usize,
    channels: usize,
) -> Vec<Vec<f32>> {
    let out_channels = host.output_channels();
    // Warm-up: plugin parameter smoothing ramps from defaults after build,
    // so settle the chain with silence before measuring its steady-state
    // transfer function.
    let warmup = vec![0.0f32; CAPTURE_FRAMES * channels];
    let mut warmup_out = vec![0.0f32; CAPTURE_FRAMES * out_channels];
    host.process(&warmup, &mut warmup_out)
        .expect("warm-up processing must not fail");
    let mut captured = vec![Vec::with_capacity(CAPTURE_FRAMES); out_channels];

    let mut input = vec![0.0f32; CAPTURE_FRAMES * channels];
    input[input_channel] = 1.0;
    let mut output = vec![0.0f32; CAPTURE_FRAMES * out_channels];
    let produced = host
        .process(&input, &mut output)
        .expect("impulse processing must not fail");
    for frame in 0..produced {
        for channel in 0..out_channels {
            captured[channel].push(output[frame * out_channels + channel]);
        }
    }

    for _ in 0..1024 {
        let mut chunk = vec![0.0f32; host.drain_output_frames_max() * out_channels];
        let result = host.drain(&mut chunk).expect("drain must not fail");
        for frame in 0..result.frames {
            for channel in 0..out_channels {
                captured[channel].push(chunk[frame * out_channels + channel]);
            }
        }
        if result.complete {
            break;
        }
    }
    captured
}

/// Magnitude response in dB at `freqs` via direct DFT summation of the
/// captured impulse response (linear convolution identity: the impulse
/// response's DTFT is the transfer function).
fn magnitude_db_at(impulse: &[f32], freqs: &[f64]) -> Vec<f64> {
    freqs
        .iter()
        .map(|&freq| {
            let step = -std::f64::consts::TAU * freq / f64::from(SAMPLE_RATE);
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (n, &sample) in impulse.iter().enumerate() {
                if sample == 0.0 {
                    continue;
                }
                let phase = step * n as f64;
                re += f64::from(sample) * phase.cos();
                im += f64::from(sample) * phase.sin();
            }
            20.0 * (re * re + im * im).sqrt().max(1e-12).log10()
        })
        .collect()
}

fn build_host(config: &MeasuredConfig) -> (PluginHost, DspChainOutput) {
    let output = load_chain(config);
    let graph =
        build_room_eq_plugin_graph_config(&output, f64::from(SAMPLE_RATE)).unwrap_or_else(|e| {
            panic!(
                "{} {}: graph build failed: {e}",
                config.dataset, config.mode
            )
        });
    assert!(
        !graph.nodes.is_empty(),
        "{} {}: graph must contain nodes",
        config.dataset,
        config.mode
    );
    let (host, warnings) = build_plugin_graph_host(&graph, SAMPLE_RATE, config.input_channels)
        .unwrap_or_else(|e| panic!("{} {}: host build failed: {e}", config.dataset, config.mode));
    assert!(
        warnings.is_empty(),
        "{} {}: unexpected host warnings: {warnings:?}",
        config.dataset,
        config.mode
    );
    (host, output)
}

fn has_routing_graph(output: &DspChainOutput) -> bool {
    output
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.bass_management.as_ref())
        .and_then(|report| report.routing_graph.as_ref())
        .is_some_and(|graph| !graph.routes.is_empty())
}

#[test]
fn measured_configs_build_and_process_finite_audio() {
    for config in CONFIGS {
        let (mut host, _output) = build_host(config);
        assert!(
            host.output_channels() >= 1,
            "{} {}: host must expose outputs",
            config.dataset,
            config.mode
        );
        let _ = host.total_latency_samples();
        let energy = run_programme(&mut host, config.input_channels);
        assert!(
            energy > 0.0,
            "{} {}: programme material must produce output energy",
            config.dataset,
            config.mode
        );
    }
}

#[test]
fn measured_linear_configs_match_stored_eq_response() {
    for config in CONFIGS {
        let (mut host, output) = build_host(config);
        if has_routing_graph(&output) {
            // Routed (bass-managed) graphs mix crossover/matrix gains into
            // the diagonal response, so the stored EQ-only curve is not a
            // valid oracle there; those configurations are covered by the
            // build/process/finite assertions above.
            continue;
        }

        let channel_names: Vec<String> = output.channels.keys().cloned().collect();
        assert_eq!(
            channel_names.len(),
            config.input_channels,
            "{} {}: channel count mismatch",
            config.dataset,
            config.mode
        );

        // Evaluation grid: the first stored EQ curve's frequencies,
        // stride-limited and restricted to the reliable correction band.
        let reference = load_eq_csv(config, &channel_names[0]);
        let in_band: Vec<f64> = reference
            .iter()
            .map(|(freq, _)| *freq)
            .filter(|freq| (25.0..=18_000.0).contains(freq))
            .collect();
        let stride = (in_band.len() / MAX_EVAL_POINTS).max(1);
        let grid: Vec<f64> = in_band.iter().step_by(stride).copied().collect();
        assert!(!grid.is_empty());

        // Measure every diagonal (input i -> output i) transfer function.
        let mut measured: Vec<Vec<f64>> = Vec::new();
        for input_channel in 0..config.input_channels {
            let captured =
                capture_impulse_response(&mut host, input_channel, config.input_channels);
            assert!(
                input_channel < captured.len(),
                "{} {}: missing diagonal output {input_channel}",
                config.dataset,
                config.mode
            );
            let impulse = &captured[input_channel];
            assert!(
                impulse.iter().all(|sample| sample.is_finite()),
                "{} {}: non-finite impulse response",
                config.dataset,
                config.mode
            );
            measured.push(magnitude_db_at(impulse, &grid));
            host.reset();
        }

        // Set-match each stored EQ curve to its closest measured diagonal
        // (channel index order is a builder detail; identity is not).
        let mut used: HashSet<usize> = HashSet::new();
        for name in &channel_names {
            let stored = load_eq_csv(config, name);
            let stored_db: Vec<f64> = grid.iter().map(|&f| stored_eq_at(&stored, f)).collect();
            let mut best: Option<(usize, f64, f64)> = None;
            for (index, candidate) in measured.iter().enumerate() {
                if used.contains(&index) {
                    continue;
                }
                let diffs: Vec<f64> = candidate
                    .iter()
                    .zip(stored_db.iter())
                    .map(|(m, s)| m - s)
                    .collect();
                let offset = {
                    let mut sorted = diffs.clone();
                    sorted.sort_by(f64::total_cmp);
                    sorted[sorted.len() / 2]
                };
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "Grid lengths normalize floating-point statistics over small test buffers."
                )]
                let rms = (diffs.iter().map(|d| (d - offset).powi(2)).sum::<f64>()
                    / diffs.len() as f64)
                    .sqrt();
                if best.is_none_or(|(_, best_rms, _)| rms < best_rms) {
                    best = Some((index, rms, offset));
                }
            }
            let (index, rms, offset) = best.expect("every stored curve needs a measured match");
            used.insert(index);
            assert!(
                rms < 0.75,
                "{} {} channel {name}: measured response deviates {rms:.2} dB RMS \
                 from the stored EQ curve (offset {offset:.2} dB)",
                config.dataset,
                config.mode
            );
            assert!(
                offset.abs() < 3.0,
                "{} {} channel {name}: gain staging offset {offset:.2} dB out of bounds",
                config.dataset,
                config.mode
            );
        }
    }
}

#[test]
fn rejected_genelec_mixed_configuration_stays_unpublished() {
    // Upstream RoomEQ rejected this configuration (`no approved playback
    // result: Rejected`); the fixture set must keep that explicit instead
    // of silently gaining a playable chain.
    let dataset_dir = fixture_dir().join("5.1.4_genelec");
    assert!(
        !dataset_dir.join("dsp-mixed.json").exists(),
        "rejected configuration must not publish a DSP chain"
    );
    let diagnostic_path = dataset_dir.join("mixed-diagnostic.json");
    let diagnostic: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&diagnostic_path).expect("diagnostic must exist"))
            .expect("diagnostic must parse");
    assert_eq!(
        diagnostic["scope"], "not_approved_for_playback",
        "diagnostic scope must stay explicit"
    );
}
