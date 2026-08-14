//! Integration tests for sotf-plugin-downmix exercising the public `Plugin` trait.

use rustfft::num_complex::Complex;
use sotf_host::param_specs::UpdateMode;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_downmix::{DownmixPlugin, DownmixPluginParams};

const SR: u32 = 48000;

fn ctx(frames: usize) -> ProcessContext<'static> {
    ProcessContext::new(SR, frames)
}

fn render_partitioned(
    plugin: DownmixPlugin,
    input: &[f32],
    channels: usize,
    blocks: &[usize],
) -> Vec<f32> {
    render_partitioned_at_rate(plugin, input, channels, blocks, SR)
}

fn render_partitioned_at_rate(
    mut plugin: DownmixPlugin,
    input: &[f32],
    channels: usize,
    blocks: &[usize],
    sample_rate: u32,
) -> Vec<f32> {
    plugin.initialize(sample_rate).unwrap();
    let frames = input.len() / channels;
    let mut output = vec![0.0; frames * 2];
    let mut position = 0;
    let mut block_index = 0;
    while position < frames {
        let count = blocks[block_index % blocks.len()].min(frames - position);
        plugin
            .process(
                &input[position * channels..(position + count) * channels],
                &mut output[position * 2..(position + count) * 2],
                &ProcessContext::new(sample_rate, count),
            )
            .unwrap();
        position += count;
        block_index += 1;
    }
    output
}

#[test]
fn info_is_reported() {
    let plugin = DownmixPlugin::new(6);
    let info = plugin.info();
    assert_eq!(info.name, "Downmix");
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    assert!(!info.description.is_empty());
}

#[test]
fn five_one_center_fold_down() {
    let mut plugin = DownmixPlugin::new(6);
    // Configure before initialize so coefficients start at their targets.
    plugin
        .set_parameter(
            ParameterId::from("phase_coherence"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("center_gain_db"),
            ParameterValue::Float(0.0),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("surround_gain_db"),
            ParameterValue::Float(-60.0),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("lfe_gain_db"),
            ParameterValue::Float(-60.0),
        )
        .unwrap();
    plugin.initialize(SR).unwrap();

    // 5.1 channel order: L, R, C, LFE, SL, SR
    let mut input = vec![0.0f32; 64 * 6];
    for frame in 0..64 {
        input[frame * 6 + 2] = 1.0; // center only
    }
    let mut output = vec![0.0f32; 64 * 2];
    plugin.process(&input, &mut output, &ctx(64)).unwrap();

    let l = output.iter().step_by(2).copied().sum::<f32>() / 64.0;
    let r = output.iter().skip(1).step_by(2).copied().sum::<f32>() / 64.0;
    assert!(
        (l - 0.707).abs() < 0.05,
        "expected center in left ~0.707, got {l}"
    );
    assert!(
        (r - 0.707).abs() < 0.05,
        "expected center in right ~0.707, got {r}"
    );
}

#[test]
fn stereo_left_passes_to_left() {
    let mut plugin = DownmixPlugin::new(2);
    plugin
        .set_parameter(
            ParameterId::from("phase_coherence"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    plugin.initialize(SR).unwrap();

    let mut input = vec![0.0f32; 64 * 2];
    for frame in 0..64 {
        input[frame * 2] = 0.8; // left only
    }
    let mut output = vec![0.0f32; 64 * 2];
    plugin.process(&input, &mut output, &ctx(64)).unwrap();

    let l = output.iter().step_by(2).copied().sum::<f32>() / 64.0;
    let r = output.iter().skip(1).step_by(2).copied().sum::<f32>() / 64.0;
    assert!((l - 0.8).abs() < 0.01, "left should pass through, got {l}");
    assert!(r.abs() < 0.01, "right should be silent, got {r}");
}

#[test]
fn mono_input_goes_to_both_channels() {
    let mut plugin = DownmixPlugin::new(1);
    plugin
        .set_parameter(
            ParameterId::from("phase_coherence"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("center_gain_db"),
            ParameterValue::Float(0.0),
        )
        .unwrap();
    plugin.initialize(SR).unwrap();

    let input = vec![0.6f32; 64];
    let mut output = vec![0.0f32; 64 * 2];
    plugin.process(&input, &mut output, &ctx(64)).unwrap();

    let l = output.iter().step_by(2).copied().sum::<f32>() / 64.0;
    let r = output.iter().skip(1).step_by(2).copied().sum::<f32>() / 64.0;
    assert!((l - 0.424).abs() < 0.05, "mono left ~0.707*0.6, got {l}");
    assert!((r - 0.424).abs() < 0.05, "mono right ~0.707*0.6, got {r}");
}

#[test]
fn itu_mode_center_fold_down() {
    let mut plugin = DownmixPlugin::new(6);
    plugin
        .set_parameter(
            ParameterId::from("phase_coherence"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("itu_mode"), ParameterValue::Bool(true))
        .unwrap();
    plugin.initialize(SR).unwrap();

    let mut input = vec![0.0f32; 64 * 6];
    for frame in 0..64 {
        input[frame * 6 + 2] = 1.0; // center
    }
    let mut output = vec![0.0f32; 64 * 2];
    plugin.process(&input, &mut output, &ctx(64)).unwrap();

    let l = output.iter().step_by(2).copied().sum::<f32>() / 64.0;
    let r = output.iter().skip(1).step_by(2).copied().sum::<f32>() / 64.0;
    assert!((l - 0.707).abs() < 0.05, "ITU center left ~0.707, got {l}");
    assert!((r - 0.707).abs() < 0.05, "ITU center right ~0.707, got {r}");
}

#[test]
fn center_gain_roundtrip() {
    let mut plugin = DownmixPlugin::new(6);
    plugin
        .set_parameter(
            ParameterId::from("center_gain_db"),
            ParameterValue::Float(-6.0),
        )
        .unwrap();
    let got = plugin
        .get_parameter(&ParameterId::from("center_gain_db"))
        .and_then(|v| v.as_float())
        .unwrap();
    assert!((got - (-6.0)).abs() < 1e-3);
}

#[test]
fn structural_phase_change_requires_reconstruction_after_initialize() {
    let mut plugin = DownmixPlugin::new(2);
    plugin.initialize(SR).unwrap();
    assert!(
        plugin.latency_samples() > 0,
        "phase coherence on by default -> latency"
    );

    let err = plugin
        .set_parameter(
            ParameterId::from("phase_coherence"),
            ParameterValue::Bool(false),
        )
        .unwrap_err();
    assert!(err.contains("requires plugin reconstruction"));
    assert!(plugin.latency_samples() > 0);
}

#[test]
fn phase_mode_is_structural() {
    let plugin = DownmixPlugin::new(2);
    let phase = plugin
        .parameters()
        .into_iter()
        .find(|parameter| parameter.id == ParameterId::from("phase_coherence"))
        .unwrap();
    assert_eq!(phase.update_mode, UpdateMode::Structural);
}

#[test]
fn phase_output_is_partition_invariant() {
    let frames = 8192;
    let mut input = vec![0.0f32; frames * 2];
    for frame in 0..frames {
        input[frame * 2] = (frame as f32 * 0.013).sin() * 0.3;
        input[frame * 2 + 1] = (frame as f32 * 0.021).cos() * 0.2;
    }

    let contiguous = render_partitioned(DownmixPlugin::new(2), &input, 2, &[frames]);
    let varied = render_partitioned(
        DownmixPlugin::new(2),
        &input,
        2,
        &[1, 16, 31, 64, 257, 480, 512, 1024, 2048, 4093],
    );
    assert_eq!(contiguous, varied);
}

#[derive(Clone, Copy, Debug)]
enum PhaseStressSignal {
    AntiCorrelated,
    Diffuse,
    Transient,
    MovingPhase,
}

fn phase_stress_input(kind: PhaseStressSignal, frames: usize) -> Vec<f32> {
    const CHANNELS: usize = 6;
    let mut input = vec![0.0; frames * CHANNELS];
    match kind {
        PhaseStressSignal::AntiCorrelated => {
            for frame in 0..frames {
                let phase = 2.0 * std::f32::consts::PI * 997.0 * frame as f32 / SR as f32;
                let tone = phase.sin() * 0.35;
                input[frame * CHANNELS] = tone;
                input[frame * CHANNELS + 1] = -tone;
                input[frame * CHANNELS + 2] = -tone * 0.45;
                input[frame * CHANNELS + 4] = (phase + 1.1).sin() * 0.22;
                input[frame * CHANNELS + 5] = -(phase + 1.1).sin() * 0.22;
            }
        }
        PhaseStressSignal::Diffuse => {
            let mut state = 0xc001_d00d_u32;
            for sample in &mut input {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                *sample = (state as f32 / u32::MAX as f32 - 0.5) * 0.36;
            }
            for frame in 0..frames {
                input[frame * CHANNELS + 3] = 0.0;
            }
        }
        PhaseStressSignal::Transient => {
            for event in 0..48 {
                let frame = 307 + event * 431;
                if frame >= frames {
                    break;
                }
                let sign = if event & 1 == 0 { 1.0 } else { -1.0 };
                input[frame * CHANNELS] = 0.55 * sign;
                input[frame * CHANNELS + 1] = -0.55 * sign;
                input[frame * CHANNELS + 2] = 0.35 * sign;
                input[frame * CHANNELS + 4] = -0.3 * sign;
                input[frame * CHANNELS + 5] = 0.3 * sign;
            }
        }
        PhaseStressSignal::MovingPhase => {
            for frame in 0..frames {
                let carrier = 2.0 * std::f32::consts::PI * 1300.0 * frame as f32 / SR as f32;
                let drift = 1.7 * (2.0 * std::f32::consts::PI * frame as f32 / 8192.0).sin();
                input[frame * CHANNELS] = (carrier + drift).sin() * 0.24;
                input[frame * CHANNELS + 1] = (carrier - drift).sin() * 0.24;
                input[frame * CHANNELS + 2] = carrier.sin() * 0.12;
                input[frame * CHANNELS + 4] = (carrier + 2.0 * drift).sin() * 0.18;
                input[frame * CHANNELS + 5] = (carrier - 2.0 * drift).sin() * 0.18;
            }
        }
    }
    input
}

fn phase_stress_plugin(enabled: bool) -> DownmixPlugin {
    DownmixPlugin::try_from_params(DownmixPluginParams {
        input_channels: 6,
        input_layout: Some("5.1".to_string()),
        center_gain_db: -3.0,
        surround_gain_db: -3.0,
        height_gain_db: -6.0,
        lfe_gain_db: -60.0,
        phase_coherence: enabled,
        phase_blend_low_hz: 300.0,
        phase_blend_high_hz: 4000.0,
        itu_mode: false,
        matrix_ltrt: false,
    })
    .unwrap()
}

#[derive(Debug)]
struct StereoMetrics {
    peak: f32,
    mean_power: f32,
    image_balance_db: f32,
    max_jump: f32,
}

fn stereo_metrics(samples: &[f32]) -> StereoMetrics {
    let mut peak = 0.0_f32;
    let mut sum_power = 0.0_f32;
    let mut left_power = 0.0_f32;
    let mut right_power = 0.0_f32;
    let mut max_jump = 0.0_f32;
    let mut previous = [0.0_f32; 2];
    for (frame, stereo) in samples.chunks_exact(2).enumerate() {
        for channel in 0..2 {
            peak = peak.max(stereo[channel].abs());
            sum_power += stereo[channel] * stereo[channel];
            if frame > 0 {
                max_jump = max_jump.max((stereo[channel] - previous[channel]).abs());
            }
            previous[channel] = stereo[channel];
        }
        left_power += stereo[0] * stereo[0];
        right_power += stereo[1] * stereo[1];
    }
    StereoMetrics {
        peak,
        mean_power: sum_power / samples.len() as f32,
        image_balance_db: (10.0 * ((left_power + 1.0e-20) / (right_power + 1.0e-20)).log10()).abs(),
        max_jump,
    }
}

#[test]
fn phase_aligner_stress_matrix_has_bounded_level_image_and_phase_jumps() {
    const CHANNELS: usize = 6;
    const FRAMES: usize = 12 * 2048;
    const PARTITIONS: &[usize] = &[1, 17, 63, 257, 511, 1024, 2053];

    for kind in [
        PhaseStressSignal::AntiCorrelated,
        PhaseStressSignal::Diffuse,
        PhaseStressSignal::Transient,
        PhaseStressSignal::MovingPhase,
    ] {
        let input = phase_stress_input(kind, FRAMES);
        let coherent_contiguous =
            render_partitioned(phase_stress_plugin(true), &input, CHANNELS, &[FRAMES]);
        let coherent_partitioned =
            render_partitioned(phase_stress_plugin(true), &input, CHANNELS, PARTITIONS);
        assert_eq!(
            coherent_contiguous, coherent_partitioned,
            "{kind:?} changed with callback partitioning"
        );

        let ordinary = render_partitioned(phase_stress_plugin(false), &input, CHANNELS, &[FRAMES]);
        let latency = phase_stress_plugin(true).latency_samples();
        let coherent_start = latency * 2;
        let coherent_end = FRAMES - latency;
        let ordinary_start = coherent_start - latency;
        let ordinary_end = coherent_end - latency;
        let coherent_metrics =
            stereo_metrics(&coherent_contiguous[coherent_start * 2..coherent_end * 2]);
        let ordinary_metrics = stereo_metrics(&ordinary[ordinary_start * 2..ordinary_end * 2]);
        assert!(
            coherent_metrics.peak <= ordinary_metrics.peak * 1.1 + 0.001,
            "{kind:?} peak grew unexpectedly: coherent={coherent_metrics:?}, ordinary={ordinary_metrics:?}"
        );
        let power_ratio = coherent_metrics.mean_power / ordinary_metrics.mean_power.max(1.0e-12);
        assert!(
            (0.9..=1.1).contains(&power_ratio),
            "{kind:?} power ratio {power_ratio}: coherent={coherent_metrics:?}, ordinary={ordinary_metrics:?}"
        );
        assert!(
            (coherent_metrics.image_balance_db - ordinary_metrics.image_balance_db).abs() <= 0.25,
            "{kind:?} image shifted: coherent={coherent_metrics:?}, ordinary={ordinary_metrics:?}"
        );
        assert!(
            coherent_metrics.max_jump <= ordinary_metrics.max_jump * 1.1 + 0.001,
            "{kind:?} discontinuity bound exceeded: coherent={coherent_metrics:?}, ordinary={ordinary_metrics:?}"
        );
        assert!(
            coherent_metrics.mean_power > 1.0e-9,
            "{kind:?} did not exercise output"
        );
    }
}

#[test]
fn process_rejects_inexact_buffer_lengths() {
    let mut plugin = DownmixPlugin::new(6);
    plugin.initialize(SR).unwrap();
    let context = ctx(16);
    let mut output = vec![0.0; 32];
    assert!(
        plugin
            .process(&vec![0.0; 16 * 6 - 1], &mut output, &context)
            .is_err()
    );
    assert!(
        plugin
            .process(&vec![0.0; 16 * 6 + 1], &mut output, &context)
            .is_err()
    );

    let input = vec![0.0; 16 * 6];
    assert!(plugin.process(&input, &mut [0.0; 31], &context).is_err());
    assert!(plugin.process(&input, &mut [0.0; 33], &context).is_err());
}

#[test]
fn construction_rejects_invalid_dimensions_and_modes() {
    assert!(DownmixPlugin::try_new(0).is_err());
    assert!(DownmixPlugin::try_new(usize::MAX).is_err());

    let mut params = DownmixPluginParams {
        input_channels: 6,
        input_layout: Some("5.1".to_string()),
        center_gain_db: -3.0,
        surround_gain_db: -3.0,
        height_gain_db: -6.0,
        lfe_gain_db: -10.0,
        phase_coherence: true,
        phase_blend_low_hz: 500.0,
        phase_blend_high_hz: 2000.0,
        itu_mode: false,
        matrix_ltrt: true,
    };
    assert!(DownmixPlugin::try_from_params(params.clone()).is_err());
    params.matrix_ltrt = false;
    params.center_gain_db = f32::NAN;
    assert!(DownmixPlugin::try_from_params(params).is_err());
}

#[test]
fn ltrt_surround_rotation_preserves_matrix_magnitude_and_polarity() {
    let frames = 8 * 2048;
    let frequency = 1000.0;
    let mut input = vec![0.0f32; frames * 6];
    for frame in 0..frames {
        input[frame * 6 + 4] =
            (2.0 * std::f32::consts::PI * frequency * frame as f32 / SR as f32).sin();
    }
    let params = DownmixPluginParams {
        input_channels: 6,
        input_layout: Some("5.1".to_string()),
        center_gain_db: -3.0,
        surround_gain_db: -3.0,
        height_gain_db: -6.0,
        lfe_gain_db: -10.0,
        phase_coherence: false,
        phase_blend_low_hz: 500.0,
        phase_blend_high_hz: 2000.0,
        itu_mode: false,
        matrix_ltrt: true,
    };
    let output = render_partitioned(
        DownmixPlugin::try_from_params(params).unwrap(),
        &input,
        6,
        &[31, 257, 480, 1024],
    );

    let start = 3 * 2048;
    let mut left_power = 0.0;
    let mut right_power = 0.0;
    let mut polarity_error = 0.0;
    let count = frames - start;
    for frame in start..frames {
        let left = output[frame * 2];
        let right = output[frame * 2 + 1];
        left_power += left * left;
        right_power += right * right;
        polarity_error = f32::max(polarity_error, (left + right).abs());
    }
    let expected_rms = std::f32::consts::FRAC_1_SQRT_2 * std::f32::consts::FRAC_1_SQRT_2;
    let left_rms = (left_power / count as f32).sqrt();
    let right_rms = (right_power / count as f32).sqrt();
    assert!(
        (left_rms - expected_rms).abs() < 0.03,
        "left RMS {left_rms}"
    );
    assert!(
        (right_rms - expected_rms).abs() < 0.03,
        "right RMS {right_rms}"
    );
    assert!(
        polarity_error < 1e-5,
        "Lt/Rt surround outputs must have opposite polarity"
    );
}

#[derive(Clone, Copy, Debug)]
enum ReferenceLtRtRole {
    Left,
    Right,
    Center,
    SurroundLeft,
    SurroundRight,
}

#[derive(Clone, Copy)]
struct ReferenceLtRtRoute {
    channel: usize,
    role: ReferenceLtRtRole,
}

fn project_tone(
    stereo: &[f32],
    output_channel: usize,
    start_frame: usize,
    frames: usize,
    cycles: usize,
) -> Complex<f32> {
    let mut projection = Complex::new(0.0_f32, 0.0_f32);
    for frame in 0..frames {
        let angle = -2.0 * std::f32::consts::PI * cycles as f32 * frame as f32 / frames as f32;
        projection +=
            Complex::from_polar(1.0, angle) * stereo[(start_frame + frame) * 2 + output_channel];
    }
    projection * (2.0 / frames as f32)
}

fn reference_ltrt_decode(
    role: ReferenceLtRtRole,
    lt: Complex<f32>,
    rt: Complex<f32>,
) -> (Complex<f32>, Complex<f32>) {
    // Independent published Scheiber/Dolby matrix oracle. This deliberately
    // does not call the plugin's routing, WOLA, or quadrature implementation.
    // The sum bus recovers an isolated centre, while +90-degree rotation of
    // the difference bus recovers the signed mono-surround channel.
    let passive_matrix_gain = 0.5_f32.sqrt();
    match role {
        ReferenceLtRtRole::Left => (lt, rt),
        ReferenceLtRtRole::Right => (rt, lt),
        ReferenceLtRtRole::Center => (
            (lt + rt) * passive_matrix_gain,
            (lt - rt) * passive_matrix_gain,
        ),
        ReferenceLtRtRole::SurroundLeft | ReferenceLtRtRole::SurroundRight => (
            (lt - rt) * passive_matrix_gain * Complex::new(0.0, 1.0),
            (lt + rt) * passive_matrix_gain,
        ),
    }
}

#[test]
fn ltrt_matches_independent_decoder_oracle_across_rates_layouts_and_partitions() {
    const ANALYSIS_FRAMES: usize = 8192;
    const TOTAL_FRAMES: usize = 12 * 2048;
    const AMPLITUDE: f32 = 0.08;
    const PARTITIONS: &[usize] = &[1, 31, 257, 480, 1024, 2048, 4093];
    const ROUTES_5_1: &[ReferenceLtRtRoute] = &[
        ReferenceLtRtRoute {
            channel: 0,
            role: ReferenceLtRtRole::Left,
        },
        ReferenceLtRtRoute {
            channel: 1,
            role: ReferenceLtRtRole::Right,
        },
        ReferenceLtRtRoute {
            channel: 2,
            role: ReferenceLtRtRole::Center,
        },
        ReferenceLtRtRoute {
            channel: 4,
            role: ReferenceLtRtRole::SurroundLeft,
        },
        ReferenceLtRtRoute {
            channel: 5,
            role: ReferenceLtRtRole::SurroundRight,
        },
    ];
    const ROUTES_7_1: &[ReferenceLtRtRoute] = &[
        ReferenceLtRtRoute {
            channel: 0,
            role: ReferenceLtRtRole::Left,
        },
        ReferenceLtRtRoute {
            channel: 1,
            role: ReferenceLtRtRole::Right,
        },
        ReferenceLtRtRoute {
            channel: 2,
            role: ReferenceLtRtRole::Center,
        },
        ReferenceLtRtRoute {
            channel: 4,
            role: ReferenceLtRtRole::SurroundLeft,
        },
        ReferenceLtRtRoute {
            channel: 5,
            role: ReferenceLtRtRole::SurroundRight,
        },
        ReferenceLtRtRoute {
            channel: 6,
            role: ReferenceLtRtRole::SurroundLeft,
        },
        ReferenceLtRtRoute {
            channel: 7,
            role: ReferenceLtRtRole::SurroundRight,
        },
    ];
    const TONE_CYCLES: &[usize] = &[173, 229, 293, 359, 431, 503, 577];

    for sample_rate in [44_100, 48_000, 96_000] {
        for (layout, channels, routes) in [("5.1", 6, ROUTES_5_1), ("7.1", 8, ROUTES_7_1)] {
            let mut input = vec![0.0_f32; TOTAL_FRAMES * channels];
            for (route_index, route) in routes.iter().enumerate() {
                let cycles = TONE_CYCLES[route_index];
                let phase = 0.19 + route_index as f32 * 0.37;
                for frame in 0..TOTAL_FRAMES {
                    let angle = 2.0 * std::f32::consts::PI * cycles as f32 * frame as f32
                        / ANALYSIS_FRAMES as f32
                        + phase;
                    input[frame * channels + route.channel] += AMPLITUDE * angle.cos();
                }
            }

            let make_plugin = || {
                DownmixPlugin::try_from_params(DownmixPluginParams {
                    input_channels: channels,
                    input_layout: Some(layout.to_string()),
                    center_gain_db: -3.0,
                    surround_gain_db: -3.0,
                    height_gain_db: -6.0,
                    lfe_gain_db: -60.0,
                    phase_coherence: false,
                    phase_blend_low_hz: 500.0,
                    phase_blend_high_hz: 5000.0,
                    itu_mode: false,
                    matrix_ltrt: true,
                })
                .unwrap()
            };
            let latency = make_plugin().latency_samples();
            let contiguous = render_partitioned_at_rate(
                make_plugin(),
                &input,
                channels,
                &[TOTAL_FRAMES],
                sample_rate,
            );
            let partitioned = render_partitioned_at_rate(
                make_plugin(),
                &input,
                channels,
                PARTITIONS,
                sample_rate,
            );
            assert_eq!(
                contiguous, partitioned,
                "{layout} at {sample_rate} Hz changed with callback partitions"
            );
            assert!(
                contiguous[..latency * 2]
                    .iter()
                    .all(|sample| *sample == 0.0),
                "{layout} at {sample_rate} Hz escaped before reported latency {latency}"
            );

            let analysis_start = latency * 3;
            for (route_index, route) in routes.iter().enumerate() {
                let cycles = TONE_CYCLES[route_index];
                let lt = project_tone(&contiguous, 0, analysis_start, ANALYSIS_FRAMES, cycles);
                let rt = project_tone(&contiguous, 1, analysis_start, ANALYSIS_FRAMES, cycles);
                let (decoded, rejected) = reference_ltrt_decode(route.role, lt, rt);
                let source_phase = 0.19 + route_index as f32 * 0.37;
                let aligned_source_phase =
                    2.0 * std::f32::consts::PI * cycles as f32 * (analysis_start - latency) as f32
                        / ANALYSIS_FRAMES as f32;
                let sign = if matches!(route.role, ReferenceLtRtRole::SurroundRight) {
                    -1.0
                } else {
                    1.0
                };
                let expected =
                    Complex::from_polar(AMPLITUDE * sign, source_phase + aligned_source_phase);
                let reconstruction_error = (decoded - expected).norm() / AMPLITUDE;
                let rejection = rejected.norm() / AMPLITUDE;
                assert!(
                    reconstruction_error < 0.04,
                    "{layout} {sample_rate} Hz {:?}: reconstruction error {reconstruction_error}, decoded={decoded:?}, expected={expected:?}",
                    route.role
                );
                assert!(
                    rejection < 0.02,
                    "{layout} {sample_rate} Hz {:?}: decoder rejection {rejection}, rejected={rejected:?}",
                    route.role
                );
            }
        }
    }
}

#[test]
fn from_params_happy_path() {
    let params = DownmixPluginParams {
        input_channels: 6,
        input_layout: Some("5.1".to_string()),
        center_gain_db: -6.0,
        surround_gain_db: -6.0,
        height_gain_db: -6.0,
        lfe_gain_db: -12.0,
        phase_coherence: false,
        phase_blend_low_hz: 400.0,
        phase_blend_high_hz: 1500.0,
        itu_mode: false,
        matrix_ltrt: false,
    };
    let mut plugin = DownmixPlugin::from_params(params);
    assert_eq!(plugin.input_channels(), 6);
    assert_eq!(plugin.output_channels(), 2);
    plugin.initialize(SR).unwrap();

    let input = vec![0.1f32; 64 * 6];
    let mut output = vec![0.0f32; 64 * 2];
    plugin.process(&input, &mut output, &ctx(64)).unwrap();
    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn reset_clears_buffers() {
    let mut plugin = DownmixPlugin::new(2);
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("phase_coherence"),
            ParameterValue::Bool(true),
        )
        .unwrap();

    let input = vec![0.2f32; 512 * 2];
    let mut output = vec![0.0f32; 512 * 2];
    plugin.process(&input, &mut output, &ctx(512)).unwrap();
    plugin.reset();

    let mut output2 = vec![0.0f32; 512 * 2];
    plugin.process(&input, &mut output2, &ctx(512)).unwrap();
    assert!(output2.iter().all(|s| s.is_finite()));
}

#[test]
fn unknown_parameter_errors() {
    let mut plugin = DownmixPlugin::new(2);
    let err = plugin
        .set_parameter(ParameterId::from("not_a_param"), ParameterValue::Float(1.0))
        .unwrap_err();
    assert!(err.contains("Unknown parameter"), "unexpected error: {err}");
}

#[test]
fn parameter_list_contains_expected_keys() {
    let plugin = DownmixPlugin::new(6);
    let ids: Vec<_> = plugin
        .parameters()
        .iter()
        .map(|p| p.id.to_string())
        .collect();
    assert!(ids.contains(&"center_gain_db".to_string()));
    assert!(ids.contains(&"phase_coherence".to_string()));
    assert!(ids.contains(&"itu_mode".to_string()));
}
