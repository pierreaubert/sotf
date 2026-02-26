// Tests for basic Delay plugin

use sotf_host::{InPlacePluginAdapter, PluginHost};
use sotf_plugin_delay::DelayPlugin;

#[test]
fn test_delay_plugin() {
    let mut host = PluginHost::new(2, 44100);

    // 10ms delay, 0 feedback, 100% wet
    let delay = DelayPlugin::new(2, 10.0, 0.0, 1.0);
    host.add_plugin(Box::new(InPlacePluginAdapter::new(delay)))
        .unwrap();

    // Input impulse
    let mut input = vec![0.0; 2000]; // enough for delay
    input[0] = 1.0; // Left impulse
    input[1] = 1.0; // Right impulse

    let mut output = vec![0.0; 2000];
    host.process(&input, &mut output).unwrap();

    // 10ms at 44100Hz = 441 samples
    let delay_samples = (10.0 * 44100.0 / 1000.0) as usize;
    let ch_idx = delay_samples * 2;

    // Check that initial output is silence (latency) or delay line filling
    // The delay plugin usually doesn't report latency, it effects the audio.
    // So output[0] should be 0.0 (if mix is 1.0)
    assert_eq!(output[0], 0.0);

    // Check delayed signal
    // Allow small interpolation error if fractional delay
    assert!(
        (output[ch_idx] - 1.0).abs() < 0.1,
        "Expected impulse at sample {}, got {}",
        delay_samples,
        output[ch_idx]
    );
}
