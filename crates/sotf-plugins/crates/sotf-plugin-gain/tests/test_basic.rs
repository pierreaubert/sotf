// Tests for basic Gain plugin

use sotf_host::{InPlacePlugin, ProcessContext};
use sotf_plugin_gain::GainPlugin;

#[test]
fn test_gain_plugin() {
    // Test +6dB gain (2x amplitude)
    let mut gain = GainPlugin::new(2, 6.0);
    gain.initialize(44100).unwrap();

    let mut buffer = vec![0.5; 100];
    let context = ProcessContext {
        sample_rate: 44100,
        num_frames: 50,
    };

    gain.process_in_place(&mut buffer, &context).unwrap();

    // Check first sample
    let expected = 0.5 * 10.0_f32.powf(6.0 / 20.0); // approx 1.0
    assert!((buffer[0] - expected).abs() < 0.001);
}
