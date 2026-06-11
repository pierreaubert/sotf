// Integration tests for sotf-plugin-loudness-compensation exercising the public Plugin trait.

use sotf_host::{InPlacePluginAdapter, ParameterId, ParameterValue, Plugin, ProcessContext};
use sotf_plugin_loudness_compensation::{
    LoudnessCompensationPlugin, LoudnessCompensationPluginParams,
};

fn sine_buffer(num_frames: usize, channels: usize, freq: f32, sample_rate: u32) -> Vec<f32> {
    let mut buf = vec![0.0_f32; num_frames * channels];
    for i in 0..num_frames {
        let t = i as f32 / sample_rate as f32;
        let s = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.2;
        for ch in 0..channels {
            buf[i * channels + ch] = s;
        }
    }
    buf
}

fn broadband_buffer(num_frames: usize, channels: usize, sample_rate: u32) -> Vec<f32> {
    let mut buf = vec![0.0_f32; num_frames * channels];
    for i in 0..num_frames {
        let t = i as f32 / sample_rate as f32;
        let mut s = 0.0_f32;
        for harmonic in 1..=8 {
            let freq = 110.0 * harmonic as f32;
            s += (2.0 * std::f32::consts::PI * freq * t).sin() / harmonic as f32;
        }
        s *= 0.1;
        for ch in 0..channels {
            buf[i * channels + ch] = s;
        }
    }
    buf
}

fn rms(samples: &[f32]) -> f32 {
    let sum = samples.iter().map(|s| s * s).sum::<f32>();
    (sum / samples.len().max(1) as f32).sqrt()
}

#[test]
fn plugin_info_and_channels() {
    let plugin = LoudnessCompensationPlugin::new(2, 100.0, 6.0, 10000.0, 6.0);
    let adapter = InPlacePluginAdapter::new(plugin);

    assert!(adapter.info().name.contains("Loudness"));
    assert_eq!(adapter.input_channels(), 2);
    assert_eq!(adapter.output_channels(), 2);
    assert!(!adapter.parameters().is_empty());
}

#[test]
fn plugin_processes_stereo_and_five_channel() {
    let mut stereo =
        InPlacePluginAdapter::new(LoudnessCompensationPlugin::new(2, 100.0, 6.0, 10000.0, 6.0));
    stereo.initialize(48000).unwrap();

    let input = sine_buffer(512, 2, 440.0, 48000);
    let mut output = vec![0.0_f32; input.len()];
    stereo
        .process(&input, &mut output, &ProcessContext::new(48000, 512))
        .unwrap();
    assert!(output.iter().all(|s| s.is_finite()));
    assert!(rms(&output) > 0.0);

    let mut five =
        InPlacePluginAdapter::new(LoudnessCompensationPlugin::new(5, 100.0, 6.0, 10000.0, 6.0));
    five.initialize(48000).unwrap();

    let input5 = sine_buffer(512, 5, 440.0, 48000);
    let mut output5 = vec![0.0_f32; input5.len()];
    five.process(&input5, &mut output5, &ProcessContext::new(48000, 512))
        .unwrap();
    assert!(output5.iter().all(|s| s.is_finite()));
    assert!(rms(&output5) > 0.0);
}

#[test]
fn parameter_roundtrip() {
    let plugin = LoudnessCompensationPlugin::new(2, 100.0, 0.0, 10000.0, 0.0);
    let mut adapter = InPlacePluginAdapter::new(plugin);

    adapter
        .set_parameter(ParameterId::from("low_gain"), ParameterValue::Float(8.0))
        .unwrap();
    assert_eq!(
        adapter.get_parameter(&ParameterId::from("low_gain")),
        Some(ParameterValue::Float(8.0))
    );

    adapter
        .set_parameter(ParameterId::from("high_gain"), ParameterValue::Float(7.0))
        .unwrap();
    assert_eq!(
        adapter.get_parameter(&ParameterId::from("high_gain")),
        Some(ParameterValue::Float(7.0))
    );

    adapter
        .set_parameter(
            ParameterId::from("mid_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    assert_eq!(
        adapter.get_parameter(&ParameterId::from("mid_enabled")),
        Some(ParameterValue::Bool(false))
    );

    adapter
        .set_parameter(ParameterId::from("mode"), ParameterValue::Int(1))
        .unwrap();
    assert_eq!(
        adapter.get_parameter(&ParameterId::from("mode")),
        Some(ParameterValue::Int(1))
    );
}

#[test]
fn mid_band_toggle_changes_output() {
    // Process a midrange sine with the mid band enabled and disabled.
    let mut enabled =
        InPlacePluginAdapter::new(LoudnessCompensationPlugin::new(2, 100.0, 0.0, 10000.0, 0.0));
    enabled.initialize(48000).unwrap();
    enabled
        .set_parameter(ParameterId::from("mid_enabled"), ParameterValue::Bool(true))
        .unwrap();
    enabled
        .set_parameter(ParameterId::from("mid_gain"), ParameterValue::Float(10.0))
        .unwrap();

    let mut disabled =
        InPlacePluginAdapter::new(LoudnessCompensationPlugin::new(2, 100.0, 0.0, 10000.0, 0.0));
    disabled.initialize(48000).unwrap();
    disabled
        .set_parameter(
            ParameterId::from("mid_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();

    let num_frames = 2048;
    let input = sine_buffer(num_frames, 2, 3500.0, 48000);
    let mut out_enabled = vec![0.0_f32; input.len()];
    let mut out_disabled = vec![0.0_f32; input.len()];

    enabled
        .process(
            &input,
            &mut out_enabled,
            &ProcessContext::new(48000, num_frames),
        )
        .unwrap();
    disabled
        .process(
            &input,
            &mut out_disabled,
            &ProcessContext::new(48000, num_frames),
        )
        .unwrap();

    assert!(
        rms(&out_enabled) > rms(&out_disabled) * 1.5,
        "Enabling a +10 dB mid band should increase the midrange sine level"
    );
}

#[test]
fn mode_change_changes_spectral_balance() {
    let mut manual =
        InPlacePluginAdapter::new(LoudnessCompensationPlugin::new(2, 100.0, 0.0, 10000.0, 0.0));
    manual.initialize(48000).unwrap();

    let mut iso =
        InPlacePluginAdapter::new(LoudnessCompensationPlugin::new(2, 100.0, 0.0, 10000.0, 0.0));
    iso.initialize(48000).unwrap();
    iso.set_parameter(ParameterId::from("mode"), ParameterValue::Int(1))
        .unwrap();

    let num_frames = 4096;
    let input = broadband_buffer(num_frames, 2, 48000);
    let mut out_manual = vec![0.0_f32; input.len()];
    let mut out_iso = vec![0.0_f32; input.len()];

    manual
        .process(
            &input,
            &mut out_manual,
            &ProcessContext::new(48000, num_frames),
        )
        .unwrap();
    iso.process(
        &input,
        &mut out_iso,
        &ProcessContext::new(48000, num_frames),
    )
    .unwrap();

    assert!(
        (rms(&out_manual) - rms(&out_iso)).abs() > 0.001,
        "Switching from Manual to ISO 226 mode should change the spectral balance"
    );
}

#[test]
fn auto_gain_exposes_data() {
    let params = LoudnessCompensationPluginParams {
        auto_gain_enabled: true,
        auto_gain_position: "post".to_string(),
        ..Default::default()
    };
    let plugin = LoudnessCompensationPlugin::from_params(2, params).unwrap();
    let mut adapter = InPlacePluginAdapter::new(plugin);
    adapter.initialize(48000).unwrap();

    let input = broadband_buffer(2048, 2, 48000);
    let mut output = vec![0.0_f32; input.len()];
    adapter
        .process(&input, &mut output, &ProcessContext::new(48000, 2048))
        .unwrap();

    assert!(
        adapter.get_data().is_some(),
        "Auto-gain enabled plugin should expose analyzer data"
    );
}

#[test]
fn reset_then_process_is_stable() {
    let plugin = LoudnessCompensationPlugin::new(2, 100.0, 6.0, 10000.0, 6.0);
    let mut adapter = InPlacePluginAdapter::new(plugin);
    adapter.initialize(48000).unwrap();

    let input = sine_buffer(512, 2, 440.0, 48000);
    let mut output = vec![0.0_f32; input.len()];
    adapter
        .process(&input, &mut output, &ProcessContext::new(48000, 512))
        .unwrap();

    adapter.reset();

    let mut output2 = vec![0.0_f32; input.len()];
    adapter
        .process(&input, &mut output2, &ProcessContext::new(48000, 512))
        .unwrap();

    assert!(output2.iter().all(|s| s.is_finite()));
}

#[test]
fn unknown_parameter_is_rejected() {
    let plugin = LoudnessCompensationPlugin::new(2, 100.0, 0.0, 10000.0, 0.0);
    let mut adapter = InPlacePluginAdapter::new(plugin);

    let result = adapter.set_parameter(
        ParameterId::from("no_such_parameter"),
        ParameterValue::Float(1.0),
    );
    assert!(result.is_err());
}

#[test]
fn invalid_parameter_value_is_rejected() {
    let plugin = LoudnessCompensationPlugin::new(2, 100.0, 0.0, 10000.0, 0.0);
    let adapter = InPlacePluginAdapter::new(plugin);

    let result = adapter.validate_parameter(&ParameterId::from("mode"), &ParameterValue::Int(5));
    assert!(result.is_err(), "mode must be 0, 1, or 2");
}
