// Integration tests for PND (Varispeed) plugin

use sotf_host::{ParameterId, ParameterValue, Plugin, ProcessContext};
use sotf_plugin_pnd::{PndData, PndPlugin, PndPluginParams};

#[test]
fn test_pnd_instantiation() {
    let params = PndPluginParams::default();
    let mut plugin = PndPlugin::from_params(2, params);

    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
    assert_eq!(plugin.info().name, "Pitch Motion Monitor");

    plugin.initialize(44100).unwrap();
}

#[test]
fn test_pnd_processing_silence() {
    let mut plugin = PndPlugin::new(2);
    plugin.initialize(44100).unwrap();

    let num_frames = 1024;
    let input = vec![0.0; num_frames * 2];
    let mut output = vec![0.0; num_frames * 2];

    let context = ProcessContext::new(44100, num_frames);

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

    let context = ProcessContext::new(44100, num_frames);

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
    assert!(matches!(str_param, Some(ParameterValue::Float(v)) if v == 0.0));

    // Reference-free fixed-frame correction is intentionally unavailable.
    let err = plugin
        .set_parameter(
            ParameterId::from("correction_strength"),
            ParameterValue::Float(0.5),
        )
        .unwrap_err();
    assert!(err.contains("maximum 0"), "{err}");

    let new_val = plugin.get_parameter(&ParameterId::from("correction_strength"));
    assert_eq!(new_val, Some(ParameterValue::Float(0.0)));
}

#[test]
fn stable_tones_without_a_reference_are_not_treated_as_absolute_error() {
    fn correction_for(freq: f32) -> f64 {
        let sample_rate = 44_100;
        let mut plugin = PndPlugin::new(1);
        plugin.initialize(sample_rate).unwrap();
        let block_size = 1024;
        let input: Vec<f32> = (0..block_size)
            .map(|i| {
                (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32).sin() * 0.5
            })
            .collect();
        let mut output = vec![0.0; block_size];
        let context = ProcessContext::new(sample_rate, block_size);
        for _ in 0..48 {
            plugin.process(&input, &mut output, &context).unwrap();
        }
        plugin
            .get_data()
            .unwrap()
            .downcast_ref::<PndData>()
            .unwrap()
            .correction_ratio
    }

    let nominal = correction_for(440.0);
    let offset = correction_for(444.4);
    assert!((nominal - 1.0).abs() < 1e-6, "nominal={nominal}");
    assert!((offset - 1.0).abs() < 1e-6, "offset={offset}");
    assert!((nominal - offset).abs() < 1e-9);
}
