// Integration tests for PND (Varispeed) plugin

use sotf_host::{ParameterId, ParameterValue, Plugin, ProcessContext};
use sotf_plugin_pnd::{PndPlugin, PndPluginParams};

#[test]
fn test_pnd_instantiation() {
    let params = PndPluginParams::default();
    let mut plugin = PndPlugin::from_params(2, params);

    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
    assert_eq!(plugin.info().name, "Pitch Drift Corrector");

    plugin.initialize(44100).unwrap();
}

#[test]
fn test_pnd_processing_silence() {
    let mut plugin = PndPlugin::new(2);
    plugin.initialize(44100).unwrap();

    let num_frames = 1024;
    let input = vec![0.0; num_frames * 2];
    let mut output = vec![0.0; num_frames * 2];

    let context = ProcessContext {
        sample_rate: 44100,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    // Output should be silent
    let output_rms: f32 = output.iter().map(|x| x * x).sum::<f32>();
    assert_eq!(output_rms, 0.0);
}

#[test]
fn test_pnd_processing_signal() {
    let mut plugin = PndPlugin::new(2);
    plugin.initialize(44100).unwrap();

    let num_frames = 1024;
    let mut input = vec![0.0; num_frames * 2];

    // Generate 440Hz sine
    for i in 0..num_frames {
        let t = i as f32 / 44100.0;
        let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin();
        input[i * 2] = s;
        input[i * 2 + 1] = s;
    }

    let mut output = vec![0.0; num_frames * 2];

    let context = ProcessContext {
        sample_rate: 44100,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    // Check output has energy (resampler adds some latency but should produce output)
    // Note: Due to initial latency, the first block might be quiet or ramp up
    // We check that it's not all zeros or NaN
    let output_energy: f32 = output.iter().map(|x| x * x).sum();
    assert!(output_energy > 0.0, "Output should contain signal");
    assert!(!output_energy.is_nan(), "Output should not be NaN");
}

#[test]
fn test_pnd_parameters() {
    let mut plugin = PndPlugin::new(2);

    // Check default params
    let str_param = plugin.get_parameter(&ParameterId::from("correction_strength"));
    assert!(matches!(str_param, Some(ParameterValue::Float(v)) if (v - 1.0).abs() < 0.001));

    // Set param
    plugin
        .set_parameter(
            ParameterId::from("correction_strength"),
            ParameterValue::Float(0.5),
        )
        .unwrap();

    let new_val = plugin.get_parameter(&ParameterId::from("correction_strength"));
    assert_eq!(new_val, Some(ParameterValue::Float(0.5)));
}
