// Integration tests for the plugin system

use sotf_audio::plugins::Host;
use sotf_audio::{GainPlugin, InPlacePluginAdapter, PluginHost};

#[test]
fn test_plugin_host_single_plugin() {
    let mut host = PluginHost::new(2, 44100);

    let gain_plugin = GainPlugin::new(2, -12.0); // -12 dB
    let adapter = InPlacePluginAdapter::new(gain_plugin);
    host.add_plugin(Box::new(adapter)).unwrap();

    let input = vec![1.0, 1.0, 1.0, 1.0]; // 2 frames, 2 channels
    let mut output = vec![0.0; 4];

    let frames = host.process(&input, &mut output).unwrap();
    assert_eq!(frames, 2);

    // -12 dB ≈ 0.25x amplitude
    for &sample in &output {
        assert!((sample - 0.25_f32).abs() < 0.01_f32);
    }
}

#[test]
fn test_plugin_host_chain() {
    let mut host = PluginHost::new(2, 44100);

    // Chain two gain plugins: -6dB then -6dB = -12dB total
    let gain1 = GainPlugin::new(2, -6.0);
    host.add_plugin(Box::new(InPlacePluginAdapter::new(gain1)))
        .unwrap();

    let gain2 = GainPlugin::new(2, -6.0);
    host.add_plugin(Box::new(InPlacePluginAdapter::new(gain2)))
        .unwrap();

    let input = vec![1.0, 1.0, 1.0, 1.0];
    let mut output = vec![0.0; 4];

    host.process(&input, &mut output).unwrap();

    // -6dB + -6dB = -12dB ≈ 0.25x amplitude
    for &sample in &output {
        assert!((sample - 0.25_f32).abs() < 0.01_f32);
    }
}

#[test]
fn test_empty_plugin_host() {
    let mut host = PluginHost::new(2, 44100);

    // With no plugins, should pass through unchanged
    let input = vec![1.0, 2.0, 3.0, 4.0];
    let mut output = vec![0.0; 4];

    host.process(&input, &mut output).unwrap();
    assert_eq!(output, input);
}

// Note: Most comprehensive tests are in the unit tests within each plugin module
// Integration tests here focus on the PluginHost API which is the main public interface
