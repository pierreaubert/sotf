//! Integration tests for the SOTF Active Acoustic Enhancement (AAE) plugin.
//!
//! These tests exercise the public `Plugin` API as a black box: instantiation,
//! parameter get/set, initialization, audio processing, bypass, reset, and
//! public error paths.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_aae::AaePlugin;
use sotf_plugin_aae::params::AaePluginParams;

fn ctx(sample_rate: u32, num_frames: usize) -> ProcessContext<'static> {
    ProcessContext::new(sample_rate, num_frames)
}

#[test]
fn info_and_channels() {
    let params = AaePluginParams::default();
    let plugin = AaePlugin::from_params(params);

    let info = plugin.info();
    assert_eq!(info.name, "AAE");
    assert_eq!(info.version, "0.5.1");
    assert_eq!(info.author, "SotF");

    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 6); // default 5.1 layout

    let params = plugin.parameters();
    assert!(!params.is_empty());
    assert!(
        params
            .iter()
            .any(|p| p.id == ParameterId::from("room_size"))
    );
}

#[test]
fn parameter_roundtrip_and_validation() {
    let mut plugin = AaePlugin::from_params(AaePluginParams::default());

    // Float roundtrip
    plugin
        .set_parameter(ParameterId::from("rt60"), ParameterValue::Float(2.5))
        .expect("valid rt60");
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("rt60")),
        Some(ParameterValue::Float(2.5))
    );

    // Boolean roundtrip
    plugin
        .set_parameter(ParameterId::from("bypass"), ParameterValue::Bool(true))
        .expect("valid bypass");
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("bypass")),
        Some(ParameterValue::Bool(true))
    );

    // String roundtrip
    plugin
        .set_parameter(
            ParameterId::from("room_preset"),
            ParameterValue::String("large".to_string()),
        )
        .expect("valid room preset");
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("room_preset")),
        Some(ParameterValue::String("large".to_string()))
    );

    // Out-of-range value is rejected by the public validation helper
    assert!(
        plugin
            .set_parameter(ParameterId::from("rt60"), ParameterValue::Float(100.0))
            .is_err(),
        "rt60 above maximum should fail"
    );

    // Unknown parameter
    assert!(
        plugin
            .set_parameter(ParameterId::from("not_a_param"), ParameterValue::Float(1.0))
            .is_err(),
        "unknown parameter should fail"
    );

    // Type mismatch
    assert!(
        plugin
            .set_parameter(ParameterId::from("rt60"), ParameterValue::Bool(true))
            .is_err(),
        "type mismatch should fail"
    );
}

#[test]
fn initialize_and_process_silence() {
    let mut plugin = AaePlugin::from_params(AaePluginParams::default());
    plugin.initialize(48_000).expect("initialize succeeds");

    let num_frames = 64;
    let input = vec![0.0f32; num_frames * plugin.input_channels()];
    let mut output = vec![0.0f32; num_frames * plugin.output_channels()];

    let frames = plugin
        .process(&input, &mut output, &ctx(48_000, num_frames))
        .expect("process succeeds");
    assert_eq!(frames, num_frames);

    assert!(
        output.iter().all(|s| s.is_finite()),
        "silent input must produce finite output"
    );
    assert!(
        output.iter().all(|s| s.abs() < 1e-6),
        "silent input should remain silent"
    );
}

#[test]
fn bypass_copies_stereo_input() {
    let mut plugin = AaePlugin::from_params(AaePluginParams::default());
    plugin.initialize(48_000).expect("initialize succeeds");
    plugin
        .set_parameter(ParameterId::from("bypass"), ParameterValue::Bool(true))
        .unwrap();

    let num_frames = 32;
    let l = 0.3f32;
    let r = -0.7f32;
    let mut input = Vec::with_capacity(num_frames * 2);
    for _ in 0..num_frames {
        input.push(l);
        input.push(r);
    }
    let mut output = vec![0.0f32; num_frames * plugin.output_channels()];

    plugin
        .process(&input, &mut output, &ctx(48_000, num_frames))
        .unwrap();

    for frame in 0..num_frames {
        let base = frame * plugin.output_channels();
        assert!(
            (output[base] - l).abs() < 1e-6,
            "bypass should copy left input to first output channel"
        );
        assert!(
            (output[base + 1] - r).abs() < 1e-6,
            "bypass should copy right input to second output channel"
        );
        for ch in 2..plugin.output_channels() {
            assert!(
                output[base + ch].abs() < 1e-6,
                "non-front channels should be silent in bypass"
            );
        }
    }
}

#[test]
fn process_with_signal_produces_output() {
    let params = AaePluginParams {
        dry_level: 1.0,
        er_level: 0.0,
        late_level: 0.0,
        lfe_level: 0.0,
        ..Default::default()
    };

    let mut plugin = AaePlugin::from_params(params);
    plugin.initialize(48_000).unwrap();

    let num_frames = 256;
    let mut input = Vec::with_capacity(num_frames * 2);
    for i in 0..num_frames {
        let sample = (i as f32 * 0.1).sin() * 0.5;
        input.push(sample);
        input.push(sample * -0.5);
    }
    let mut output = vec![0.0f32; num_frames * plugin.output_channels()];

    plugin
        .process(&input, &mut output, &ctx(48_000, num_frames))
        .unwrap();

    assert!(output.iter().all(|s| s.is_finite()));
    let max_abs = output.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(
        max_abs > 1e-3,
        "non-silent input should produce audible output"
    );
}

#[test]
fn reset_clears_state() {
    let mut plugin = AaePlugin::from_params(AaePluginParams::default());
    plugin.initialize(48_000).unwrap();

    let num_frames = 128;
    let input: Vec<f32> = (0..num_frames * 2)
        .map(|i| (i as f32 * 0.05).sin() * 0.5)
        .collect();
    let mut output = vec![0.0f32; num_frames * plugin.output_channels()];

    plugin
        .process(&input, &mut output, &ctx(48_000, num_frames))
        .unwrap();
    let max_before = output.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(max_before > 1e-6);

    plugin.reset();

    let silent_input = vec![0.0f32; num_frames * 2];
    let mut output_after = vec![0.0f32; num_frames * plugin.output_channels()];
    plugin
        .process(&silent_input, &mut output_after, &ctx(48_000, num_frames))
        .unwrap();

    assert!(
        output_after.iter().all(|s| s.abs() < 1e-6),
        "after reset, silent input should produce silent output"
    );
}

#[test]
fn wrong_buffer_sizes_return_error() {
    let mut plugin = AaePlugin::from_params(AaePluginParams::default());
    plugin.initialize(48_000).unwrap();

    let num_frames = 32;
    let good_input = vec![0.0f32; num_frames * plugin.input_channels()];
    let mut good_output = vec![0.0f32; num_frames * plugin.output_channels()];
    let mut bad_output = vec![0.0f32; num_frames * plugin.output_channels() - 1];

    assert!(
        plugin
            .process(
                &good_input[..good_input.len() - 1],
                &mut bad_output,
                &ctx(48_000, num_frames)
            )
            .is_err(),
        "input size mismatch should fail"
    );
    assert!(
        plugin
            .process(&good_input, &mut bad_output, &ctx(48_000, num_frames))
            .is_err(),
        "output size mismatch should fail"
    );
    assert!(
        plugin
            .process(&good_input, &mut good_output, &ctx(48_000, num_frames))
            .is_ok(),
        "correct sizes should succeed"
    );
}

#[test]
fn speaker_config_change_updates_output_channels() {
    let mut plugin = AaePlugin::from_params(AaePluginParams::default());
    plugin.initialize(48_000).unwrap();
    assert_eq!(plugin.output_channels(), 6);

    plugin
        .set_parameter(
            ParameterId::from("speaker_config"),
            ParameterValue::String("2.0".to_string()),
        )
        .unwrap();

    assert_eq!(plugin.output_channels(), 2);

    let num_frames = 32;
    let input = vec![0.1f32; num_frames * 2];
    let mut output = vec![0.0f32; num_frames * 2];
    plugin
        .process(&input, &mut output, &ctx(48_000, num_frames))
        .unwrap();
    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn get_data_is_present() {
    let plugin = AaePlugin::from_params(AaePluginParams::default());
    assert!(plugin.get_data().is_some());
}
