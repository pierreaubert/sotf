//! Integration tests for the SOTF Crosstalk Cancellation (XTC) plugin.
//!
//! Tests exercise the public `Plugin` trait: instantiation, parameter get/set,
//! audio processing, output verification, bypass/enable state changes and error paths.

use sotf_host::{ParameterId, ParameterValue, Plugin, ProcessContext};
use sotf_plugin_xtc::{XtcPlugin, XtcPluginParams};

#[test]
fn xtc_plugin_info_and_channels() {
    let params = XtcPluginParams::default();
    let plugin = XtcPlugin::from_params(params, 44100).unwrap();

    assert!(plugin.info().name.contains("Crosstalk Cancellation"));
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
}

#[test]
fn xtc_instantiate_default_params() {
    let plugin = XtcPlugin::new(XtcPluginParams::default(), 44100).unwrap();
    assert_eq!(plugin.input_channels(), 2);
}

#[test]
fn xtc_parameter_roundtrip() {
    let mut plugin = XtcPlugin::new(XtcPluginParams::default(), 44100).unwrap();
    plugin.initialize(44100).unwrap();

    let params = plugin.parameters();
    assert!(params.iter().any(|p| p.id.as_str() == "distance_m"));
    assert!(params.iter().any(|p| p.id.as_str() == "speaker_angle_deg"));
    assert!(params.iter().any(|p| p.id.as_str() == "enabled"));

    plugin
        .set_parameter(ParameterId::from("distance_m"), ParameterValue::Float(2.5))
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("speaker_angle_deg"),
            ParameterValue::Float(45.0),
        )
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();

    assert_eq!(
        plugin.get_parameter(&ParameterId::from("distance_m")),
        Some(ParameterValue::Float(2.5))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("speaker_angle_deg")),
        Some(ParameterValue::Float(45.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("enabled")),
        Some(ParameterValue::Bool(false))
    );
}

#[test]
fn xtc_unknown_parameter_error() {
    let mut plugin = XtcPlugin::new(XtcPluginParams::default(), 44100).unwrap();
    let err = plugin
        .set_parameter(ParameterId::from("nonexistent"), ParameterValue::Float(1.0))
        .unwrap_err();
    assert!(err.contains("Unknown parameter") || err.contains("nonexistent"));
}

#[test]
fn xtc_invalid_fft_size_error() {
    let mut params = XtcPluginParams::default();
    params.fft_size = 1000; // not a power of 2
    match XtcPlugin::new(params, 44100) {
        Ok(_) => panic!("expected an error for non-power-of-2 FFT size"),
        Err(err) => assert!(err.contains("power of 2")),
    }
}

#[test]
fn xtc_invalid_source_mode_error() {
    let mut plugin = XtcPlugin::new(XtcPluginParams::default(), 44100).unwrap();
    plugin.initialize(44100).unwrap();

    let err = plugin
        .set_parameter(
            ParameterId::from("source_mode"),
            ParameterValue::String("invalid_mode".to_string()),
        )
        .unwrap_err();
    assert!(err.contains("source_mode"));
}

#[test]
fn xtc_process_silence() {
    let mut plugin = XtcPlugin::new(XtcPluginParams::default(), 44100).unwrap();
    plugin.initialize(44100).unwrap();

    let num_frames = 4096;
    let input = vec![0.0_f32; num_frames * 2];
    let mut output = vec![0.0_f32; num_frames * 2];
    let context = ProcessContext::new(44100, num_frames);

    plugin.process(&input, &mut output, &context).unwrap();

    let energy: f32 = output.iter().map(|s| s * s).sum();
    assert_eq!(energy, 0.0, "silent input should produce silent output");
}

#[test]
fn xtc_process_stereo_produces_output() {
    let params = XtcPluginParams {
        auto_gain_enabled: false,
        ..Default::default()
    };
    let mut plugin = XtcPlugin::new(params, 44100).unwrap();
    plugin.initialize(44100).unwrap();

    let num_frames = 4096;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let t = i as f32 / 44100.0;
        input[i * 2] = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5;
        input[i * 2 + 1] = (2.0 * std::f32::consts::PI * 1000.0 * t).cos() * 0.5;
    }

    let mut output = vec![0.0_f32; num_frames * 2];
    let context = ProcessContext::new(44100, num_frames);
    plugin.process(&input, &mut output, &context).unwrap();

    let total_energy: f32 = output.iter().map(|s| s * s).sum();
    assert!(total_energy > 0.0, "XTC output should have energy");

    let left_energy: f32 = output.iter().step_by(2).map(|s| s * s).sum();
    let right_energy: f32 = output.iter().skip(1).step_by(2).map(|s| s * s).sum();
    assert!(left_energy > 0.0, "left output channel should have energy");
    assert!(
        right_energy > 0.0,
        "right output channel should have energy"
    );
}

#[test]
fn xtc_disabled_state_passes_through() {
    let params = XtcPluginParams {
        enabled: false,
        ..Default::default()
    };
    let mut plugin = XtcPlugin::new(params, 44100).unwrap();
    plugin.initialize(44100).unwrap();

    let num_frames = 512;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        input[i * 2] = 0.3;
        input[i * 2 + 1] = -0.3;
    }

    let mut output = vec![0.0_f32; num_frames * 2];
    let context = ProcessContext::new(44100, num_frames);
    plugin.process(&input, &mut output, &context).unwrap();

    for i in 0..num_frames {
        assert!((output[i * 2] - 0.3).abs() < 1e-5);
        assert!((output[i * 2 + 1] - (-0.3)).abs() < 1e-5);
    }
}

#[test]
fn xtc_state_change_enable_disable() {
    let mut plugin = XtcPlugin::new(XtcPluginParams::default(), 44100).unwrap();
    plugin.initialize(44100).unwrap();

    let num_frames = 4096;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let t = i as f32 / 44100.0;
        input[i * 2] = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5;
        input[i * 2 + 1] = (2.0 * std::f32::consts::PI * 1000.0 * t).cos() * 0.5;
    }

    let mut out_enabled = vec![0.0_f32; num_frames * 2];
    let context = ProcessContext::new(44100, num_frames);
    plugin.process(&input, &mut out_enabled, &context).unwrap();

    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();
    let mut out_disabled = vec![0.0_f32; num_frames * 2];
    plugin.process(&input, &mut out_disabled, &context).unwrap();

    let energy_enabled: f32 = out_enabled.iter().map(|s| s * s).sum();
    let energy_disabled: f32 = out_disabled.iter().map(|s| s * s).sum();

    // Disabled state should be a close pass-through of the input.
    let input_energy: f32 = input.iter().map(|s| s * s).sum();
    assert!(
        (energy_disabled - input_energy).abs() < 1e-3,
        "disabled state should pass input through nearly unchanged"
    );

    // Enabled state should differ from disabled/pass-through.
    assert!(
        (energy_enabled - energy_disabled).abs() > 0.01,
        "enabled and disabled outputs should differ"
    );
}

#[test]
fn xtc_reset_clears_state() {
    let params = XtcPluginParams {
        auto_gain_enabled: false,
        ..Default::default()
    };
    let mut plugin = XtcPlugin::new(params, 44100).unwrap();
    plugin.initialize(44100).unwrap();

    let num_frames = 4096;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let t = i as f32 / 44100.0;
        input[i * 2] = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5;
        input[i * 2 + 1] = (2.0 * std::f32::consts::PI * 1000.0 * t).cos() * 0.5;
    }

    let mut out1 = vec![0.0_f32; num_frames * 2];
    let context = ProcessContext::new(44100, num_frames);
    plugin.process(&input, &mut out1, &context).unwrap();

    plugin.reset();

    let mut out2 = vec![0.0_f32; num_frames * 2];
    plugin.process(&input, &mut out2, &context).unwrap();

    let energy: f32 = out2.iter().map(|s| s * s).sum();
    assert!(energy > 0.0, "output after reset should still have energy");
}

#[test]
fn xtc_latency_matches_fft_size_minus_hop() {
    let params = XtcPluginParams::default();
    let fft_size = params.fft_size;
    let plugin = XtcPlugin::new(params, 44100).unwrap();
    let expected = fft_size - fft_size / 4;
    assert_eq!(plugin.latency_samples(), expected);
}
