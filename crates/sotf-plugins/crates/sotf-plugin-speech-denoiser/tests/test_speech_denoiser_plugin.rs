use sotf_host::parameters::ParameterValue;
use sotf_host::plugin::{InPlacePlugin, ProcessContext};
use sotf_plugin_speech_denoiser::SpeechDenoiserPlugin;

#[test]
fn disabled_is_transparent() {
    let mut plugin = SpeechDenoiserPlugin::new(2);
    plugin
        .set_parameter("enabled".into(), ParameterValue::Bool(false))
        .expect("set enabled");
    plugin.initialize(48000).expect("initialize");

    // Process 960 frames: first 480 discarded (startup delay), second 480 pass through.
    let mut buffer: Vec<f32> = (0..1920)
        .map(|i| ((i % 100) as f32 - 50.0) / 100.0)
        .collect();
    let input = buffer.clone();
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: 960,
    };
    let written = plugin.process_in_place(&mut buffer, &context).unwrap();
    // First frame discarded, so only 480 frames written.
    assert_eq!(written, 480);
    // The second 480 frames of input should appear at the start of the output.
    assert_eq!(&buffer[..960], &input[960..1920]);
}

#[test]
fn latency_is_constant_when_disabled() {
    let mut plugin = SpeechDenoiserPlugin::new(1);
    plugin.initialize(48000).expect("initialize");
    assert_eq!(plugin.latency_samples(), 480);

    plugin
        .set_parameter("enabled".into(), ParameterValue::Bool(false))
        .expect("set enabled");
    assert_eq!(plugin.latency_samples(), 480);
}

#[test]
fn rejects_non_multiple_of_480() {
    let mut plugin = SpeechDenoiserPlugin::new(1);
    plugin.initialize(48000).expect("initialize");

    let mut buffer = vec![0.0f32; 512];
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: 512,
    };
    let result = plugin.process_in_place(&mut buffer, &context);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("480"));
}

#[test]
fn rejects_non_48khz() {
    let mut plugin = SpeechDenoiserPlugin::new(1);
    let result = plugin.initialize(44100);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("48 kHz"));
}
