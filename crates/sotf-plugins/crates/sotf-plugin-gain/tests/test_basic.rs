// Tests for basic Gain plugin

use sotf_host::ProcessContext;
use sotf_host::parametric_plugin::ParametricPlugin;
use sotf_plugin_gain::GainPlugin;

#[test]
fn test_gain_plugin() {
    // Test +6dB gain (2x amplitude)
    let mut gain = GainPlugin::new(2, 6.0);
    gain.plugin_initialize(44100).unwrap();

    let input = vec![0.5; 100];
    let mut buffer = vec![0.0; 100];
    let context = ProcessContext::new(44100, 50);

    gain.process(&input, &mut buffer, &context).unwrap();

    // Check first sample
    let expected = 0.5 * 10.0_f32.powf(6.0 / 20.0); // approx 1.0
    assert!((buffer[0] - expected).abs() < 0.001);
}
