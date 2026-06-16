// Integration tests for sotf-plugin-linear-phase-eq exercising the public Plugin trait.

use sotf_host::{ParametricInPlacePluginAdapter, ParameterId, ParameterValue, Plugin, ProcessContext};
use sotf_plugin_linear_phase_eq::{BandConfig, LinearPhaseEqPlugin, LinearPhaseEqPluginParams};

fn sine_buffer(num_frames: usize, channels: usize, freq: f32, sample_rate: u32) -> Vec<f32> {
    let mut buf = vec![0.0_f32; num_frames * channels];
    for i in 0..num_frames {
        let t = i as f32 / sample_rate as f32;
        let s = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5;
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
    let plugin = LinearPhaseEqPlugin::new(2, 48000);
    let adapter = ParametricInPlacePluginAdapter::new(plugin);

    assert!(adapter.info().name.contains("Linear-Phase"));
    assert_eq!(adapter.input_channels(), 2);
    assert_eq!(adapter.output_channels(), 2);
    assert!(!adapter.parameters().is_empty());
}

#[test]
fn plugin_processes_silence_and_sine() {
    let plugin = LinearPhaseEqPlugin::new(2, 48000);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(48000).unwrap();

    let num_frames = 512;
    let input = sine_buffer(num_frames, 2, 1000.0, 48000);
    let mut output = vec![0.0_f32; input.len()];

    let frames = adapter
        .process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();
    assert_eq!(frames, num_frames);
    assert!(
        output.iter().all(|s| s.is_finite()),
        "All output samples must be finite"
    );
}

#[test]
fn parameter_roundtrip() {
    let plugin = LinearPhaseEqPlugin::new(1, 48000);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);

    adapter
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.75))
        .unwrap();
    assert_eq!(
        adapter.get_parameter(&ParameterId::from("mix")),
        Some(ParameterValue::Float(0.75))
    );

    adapter
        .set_parameter(ParameterId::from("auto_gain"), ParameterValue::Bool(true))
        .unwrap();
    assert_eq!(
        adapter.get_parameter(&ParameterId::from("auto_gain")),
        Some(ParameterValue::Bool(true))
    );

    adapter
        .set_parameter(ParameterId::from("num_filters"), ParameterValue::Int(3))
        .unwrap();
    assert_eq!(
        adapter.get_parameter(&ParameterId::from("num_filters")),
        Some(ParameterValue::Int(3))
    );

    adapter
        .set_parameter(ParameterId::from("fir_length"), ParameterValue::Int(2))
        .unwrap();
    assert_eq!(
        adapter.get_parameter(&ParameterId::from("fir_length")),
        Some(ParameterValue::Int(2))
    );

    adapter
        .set_parameter(ParameterId::from("band_0_gain"), ParameterValue::Float(6.0))
        .unwrap();
    match adapter.get_parameter(&ParameterId::from("band_0_gain")) {
        Some(ParameterValue::Float(v)) => assert!((v - 6.0).abs() < 0.01),
        other => panic!("Expected Float, got {:?}", other),
    }
}

#[test]
fn dry_mix_passthrough() {
    // With mix=0 the plugin should pass the input straight through.
    let params = LinearPhaseEqPluginParams {
        num_filters: 1,
        fir_length_index: 0,
        auto_gain: false,
        mix: 0.0,
        filters: vec![],
    };
    let plugin = LinearPhaseEqPlugin::from_params(2, 48000, params).unwrap();
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(48000).unwrap();

    let num_frames = 256;
    let input = sine_buffer(num_frames, 2, 440.0, 48000);
    let mut output = vec![0.0_f32; input.len()];

    adapter
        .process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();

    let max_error = input
        .iter()
        .zip(output.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f32, f32::max);
    assert!(
        max_error < 1e-6,
        "mix=0 should be a dry passthrough: max_error={}",
        max_error
    );
}

#[test]
fn eq_boost_changes_amplitude() {
    let params = LinearPhaseEqPluginParams {
        num_filters: 1,
        fir_length_index: 0,
        auto_gain: false,
        mix: 1.0,
        filters: vec![BandConfig {
            filter_type: "Peak".to_string(),
            frequency: 1000.0,
            q: 1.0,
            gain_db: 9.0,
            active: true,
        }],
    };
    let plugin = LinearPhaseEqPlugin::from_params(1, 48000, params).unwrap();
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(48000).unwrap();

    let num_frames = 4096;
    let input = sine_buffer(num_frames, 1, 1000.0, 48000);
    let mut output = vec![0.0_f32; input.len()];

    adapter
        .process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();

    // Ignore the first part of the output while the linear-phase FIR latency settles.
    let steady_start = adapter.latency_samples().max(64);
    let input_rms = rms(&input[steady_start..]);
    let output_rms = rms(&output[steady_start..]);

    assert!(
        output_rms > input_rms * 1.5,
        "A +9 dB boost at 1 kHz should raise the 1 kHz sine amplitude"
    );
}

#[test]
fn latency_matches_fir_length() {
    let params = LinearPhaseEqPluginParams {
        num_filters: 1,
        fir_length_index: 2, // 4096 taps
        auto_gain: false,
        mix: 1.0,
        filters: vec![],
    };
    let plugin = LinearPhaseEqPlugin::from_params(1, 48000, params).unwrap();
    let adapter = ParametricInPlacePluginAdapter::new(plugin);

    let fir_length = 4096;
    let expected = (fir_length - 1) / 2;
    assert_eq!(adapter.latency_samples(), expected);
}

#[test]
fn unknown_parameter_is_rejected() {
    let plugin = LinearPhaseEqPlugin::new(1, 48000);
    let adapter = ParametricInPlacePluginAdapter::new(plugin);

    // The default Plugin::validate_parameter helper rejects unknown ids.
    let result = adapter.validate_parameter(
        &ParameterId::from("this_param_does_not_exist"),
        &ParameterValue::Float(1.0),
    );
    assert!(
        result.is_err(),
        "Validating an unknown parameter should fail"
    );
}

#[test]
fn invalid_parameter_value_is_rejected() {
    let plugin = LinearPhaseEqPlugin::new(1, 48000);
    let adapter = ParametricInPlacePluginAdapter::new(plugin);

    let result = adapter.validate_parameter(&ParameterId::from("mix"), &ParameterValue::Float(2.0));
    assert!(
        result.is_err(),
        "A mix value outside [0, 1] should fail validation"
    );
}

#[test]
fn reset_then_process_is_stable() {
    let plugin = LinearPhaseEqPlugin::new(2, 48000);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(48000).unwrap();

    let num_frames = 256;
    let input = sine_buffer(num_frames, 2, 500.0, 48000);
    let mut output = vec![0.0_f32; input.len()];

    adapter
        .process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();

    adapter.reset();

    let mut output2 = vec![0.0_f32; input.len()];
    adapter
        .process(
            &input,
            &mut output2,
            &ProcessContext::new(48000, num_frames),
        )
        .unwrap();

    assert!(output2.iter().all(|s| s.is_finite()));
}
