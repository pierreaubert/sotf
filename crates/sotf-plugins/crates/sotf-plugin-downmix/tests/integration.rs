//! Integration tests for sotf-plugin-downmix exercising the public `Plugin` trait.

use sotf_host::param_specs::UpdateMode;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_downmix::{DownmixPlugin, DownmixPluginParams};

const SR: u32 = 48000;

fn ctx(frames: usize) -> ProcessContext<'static> {
    ProcessContext::new(SR, frames)
}

fn render_partitioned(
    mut plugin: DownmixPlugin,
    input: &[f32],
    channels: usize,
    blocks: &[usize],
) -> Vec<f32> {
    plugin.initialize(SR).unwrap();
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
                &ctx(count),
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
    assert!(
        plugin
            .process(&input, &mut vec![0.0; 31], &context)
            .is_err()
    );
    assert!(
        plugin
            .process(&input, &mut vec![0.0; 33], &context)
            .is_err()
    );
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
