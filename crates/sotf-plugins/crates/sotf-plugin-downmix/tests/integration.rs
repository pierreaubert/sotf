//! Integration tests for sotf-plugin-downmix exercising the public `Plugin` trait.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_downmix::{DownmixPlugin, DownmixPluginParams};

const SR: u32 = 48000;

fn ctx(frames: usize) -> ProcessContext<'static> {
    ProcessContext::new(SR, frames)
}

#[test]
fn info_is_reported() {
    let plugin = DownmixPlugin::new(6);
    let info = plugin.info();
    assert_eq!(info.name, "Downmix");
    assert_eq!(info.version, "2.0.0");
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
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("phase_coherence"),
            ParameterValue::Bool(false),
        )
        .unwrap();

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
fn phase_coherence_toggles_latency() {
    let mut plugin = DownmixPlugin::new(2);
    plugin.initialize(SR).unwrap();
    assert!(
        plugin.latency_samples() > 0,
        "phase coherence on by default -> latency"
    );

    plugin
        .set_parameter(
            ParameterId::from("phase_coherence"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    assert_eq!(plugin.latency_samples(), 0);

    plugin
        .set_parameter(
            ParameterId::from("phase_coherence"),
            ParameterValue::Bool(true),
        )
        .unwrap();
    assert!(plugin.latency_samples() > 0);
}

#[test]
fn from_params_happy_path() {
    let params = DownmixPluginParams {
        input_channels: 6,
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
