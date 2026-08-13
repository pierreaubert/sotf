// Integration tests for sotf-plugin-pnd exercising the public Plugin trait.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_pnd::{PndData, PndPlugin, PndPluginParams};

#[test]
fn integration_plugin_info_and_channels() {
    let plugin = PndPlugin::from_params(2, PndPluginParams::default());
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
    let info = plugin.info();
    assert_eq!(info.name, "Pitch Drift Corrector");
    assert!(!info.version.is_empty());
}

#[test]
fn integration_default_parameters() {
    let plugin = PndPlugin::new(2);
    let params = plugin.parameters();
    assert!(!params.is_empty());

    let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"correction_strength"));
    assert!(ids.contains(&"analysis_window_ms"));
    assert!(ids.contains(&"drift_smoothing"));
    assert!(ids.contains(&"multi_channel_analysis"));
    assert!(ids.contains(&"confidence_threshold"));
    assert!(ids.contains(&"phase_vocoder"));
}

#[test]
fn integration_parameter_roundtrip() {
    let mut plugin = PndPlugin::new(2);
    plugin.initialize(44100).unwrap();

    // Correction strength default should be 1.0
    let v = plugin
        .get_parameter(&ParameterId::from("correction_strength"))
        .unwrap();
    assert!(matches!(v, ParameterValue::Float(x) if (x - 1.0).abs() < 1e-3));

    plugin
        .set_parameter(
            ParameterId::from("correction_strength"),
            ParameterValue::Float(0.75),
        )
        .unwrap();
    let v = plugin
        .get_parameter(&ParameterId::from("correction_strength"))
        .unwrap();
    assert_eq!(v, ParameterValue::Float(0.75));
}

#[test]
fn integration_parameter_validation_errors() {
    let mut plugin = PndPlugin::new(2);

    // Unknown parameter
    let res = plugin.set_parameter(
        ParameterId::from("does_not_exist"),
        ParameterValue::Float(0.5),
    );
    assert!(res.is_err());

    // NaN float
    let res = plugin.set_parameter(
        ParameterId::from("correction_strength"),
        ParameterValue::Float(f32::NAN),
    );
    assert!(res.is_err());

    // Out-of-range float (range 0.0..=2.0)
    let res = plugin.set_parameter(
        ParameterId::from("correction_strength"),
        ParameterValue::Float(10.0),
    );
    assert!(res.is_err());

    // Type mismatch
    let res = plugin.set_parameter(
        ParameterId::from("multi_channel_analysis"),
        ParameterValue::Float(1.0),
    );
    assert!(res.is_err());
}

#[test]
fn integration_phase_vocoder_state_transition() {
    let mut plugin = PndPlugin::new(2);
    // Toggle phase vocoder on and off through the public parameter API.
    plugin
        .set_parameter(
            ParameterId::from("phase_vocoder"),
            ParameterValue::Bool(true),
        )
        .unwrap();
    plugin.initialize(44100).unwrap();
    let v = plugin
        .get_parameter(&ParameterId::from("phase_vocoder"))
        .unwrap();
    assert_eq!(v, ParameterValue::Bool(true));

    let num_frames = 4096;
    let mut input = vec![0.0f32; num_frames * 2];
    for i in 0..num_frames {
        let t = i as f32 / 44100.0;
        let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
        input[i * 2] = s;
        input[i * 2 + 1] = s;
    }
    let mut output = vec![0.0f32; num_frames * 2];
    let ctx = ProcessContext::new(44100, num_frames);
    let written = plugin.process(&input, &mut output, &ctx).unwrap();
    assert_eq!(written, num_frames);
    assert!(output.iter().all(|s| s.is_finite()));

    assert!(
        plugin
            .set_parameter(
                ParameterId::from("phase_vocoder"),
                ParameterValue::Bool(false),
            )
            .is_err()
    );
}

#[test]
fn integration_process_silence_and_sine() {
    let mut plugin = PndPlugin::new(2);
    plugin.initialize(44100).unwrap();

    let num_frames = 1024;
    let silence = vec![0.0f32; num_frames * 2];
    let mut out_silence = vec![0.0f32; num_frames * 2];
    let ctx = ProcessContext::new(44100, num_frames);
    plugin.process(&silence, &mut out_silence, &ctx).unwrap();
    assert!(out_silence.iter().all(|s| *s == 0.0));

    let mut sine = vec![0.0f32; num_frames * 2];
    for i in 0..num_frames {
        let t = i as f32 / 44100.0;
        let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
        sine[i * 2] = s;
        sine[i * 2 + 1] = s;
    }
    let mut out_sine = vec![0.0f32; num_frames * 2];
    plugin.process(&sine, &mut out_sine, &ctx).unwrap();
    assert!(out_sine.iter().all(|s| s.is_finite()));
    let energy: f32 = out_sine.iter().map(|s| s * s).sum();
    assert!(energy > 0.0, "sine should produce non-zero output energy");
}

#[test]
fn integration_reset_clears_state() {
    let mut plugin = PndPlugin::new(2);
    plugin.initialize(44100).unwrap();

    let num_frames = 1024;
    let mut input = vec![0.0f32; num_frames * 2];
    for i in 0..num_frames {
        input[i * 2] = 0.5;
        input[i * 2 + 1] = 0.5;
    }
    let mut output = vec![0.0f32; num_frames * 2];
    let ctx = ProcessContext::new(44100, num_frames);
    plugin.process(&input, &mut output, &ctx).unwrap();

    plugin.reset();

    let mut output2 = vec![0.0f32; num_frames * 2];
    plugin.process(&input, &mut output2, &ctx).unwrap();
    assert!(output2.iter().all(|s| s.is_finite()));

    // Latency should remain stable across reset.
    assert_eq!(plugin.latency_samples(), 1024);
}

#[test]
fn integration_process_rejects_buffer_mismatch() {
    let mut plugin = PndPlugin::new(2);
    plugin.initialize(44100).unwrap();

    let ctx = ProcessContext::new(44100, 1024);
    let input = vec![0.0f32; 1024 * 2];
    let mut output = vec![0.0f32; 1024]; // too short
    let res = plugin.process(&input, &mut output, &ctx);
    assert!(res.is_err());

    let input_short = vec![0.0f32; 512];
    let mut output_ok = vec![0.0f32; 1024 * 2];
    let res = plugin.process(&input_short, &mut output_ok, &ctx);
    assert!(res.is_err());
}

#[test]
fn integration_get_data_returns_pnd_data() {
    let mut plugin = PndPlugin::new(2);
    plugin.initialize(44100).unwrap();

    let data = plugin
        .get_data()
        .expect("PND exposes data")
        .downcast_ref::<PndData>()
        .expect("data is PndData")
        .clone();
    assert!(data.drift_ratio.is_finite());
    assert!(data.confidence.is_finite());
    assert!(data.confidence >= 0.0 && data.confidence <= 1.0);

    // Process several blocks to ensure the diagnostic cache is updated.
    let block = vec![0.0f32; 1024 * 2];
    let mut out = vec![0.0f32; 1024 * 2];
    let ctx = ProcessContext::new(44100, 1024);
    for _ in 0..12 {
        plugin.process(&block, &mut out, &ctx).unwrap();
    }

    let data2 = plugin
        .get_data()
        .unwrap()
        .downcast_ref::<PndData>()
        .unwrap()
        .clone();
    assert!(data2.confidence.is_finite());
}

#[test]
fn integration_from_params_applies_initial_state() {
    let params = PndPluginParams {
        correction_strength: 0.5,
        analysis_window_ms: 200.0,
        drift_smoothing: 0.2,
        multi_channel_analysis: false,
        confidence_threshold: 0.75,
        phase_vocoder: true,
    };
    let plugin = PndPlugin::from_params(1, params);
    assert_eq!(plugin.input_channels(), 1);
    assert_eq!(plugin.output_channels(), 1);
}

#[test]
fn integration_try_from_params_rejects_non_finite_and_invalid_values() {
    let params = PndPluginParams {
        drift_smoothing: f32::NAN,
        ..PndPluginParams::default()
    };
    assert!(PndPlugin::try_from_params(1, params).is_err());

    let params = PndPluginParams {
        analysis_window_ms: f32::INFINITY,
        ..PndPluginParams::default()
    };
    assert!(PndPlugin::try_from_params(1, params).is_err());

    let params = PndPluginParams {
        confidence_threshold: -0.1,
        ..PndPluginParams::default()
    };
    assert!(PndPlugin::try_from_params(1, params).is_err());

    assert!(PndPlugin::try_from_params(0, PndPluginParams::default()).is_err());
}
