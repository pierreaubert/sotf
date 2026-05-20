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
    let context = ProcessContext::new(48000, 960);
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
    let context = ProcessContext::new(48000, 512);
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

/// 1.3 CRITICAL: reject sample rates other than 48 kHz.
#[test]
fn initialize_rejects_non_48khz() {
    let mut plugin = SpeechDenoiserPlugin::new(1);
    assert!(
        plugin.initialize(44100).is_err(),
        "44100 Hz should be rejected"
    );
    assert!(
        plugin.initialize(96000).is_err(),
        "96000 Hz should be rejected"
    );
    assert!(
        plugin.initialize(48000).is_ok(),
        "48000 Hz must be accepted"
    );
}

/// 1.1 CRITICAL: latency must be constant regardless of the enabled flag.
#[test]
fn latency_is_constant_regardless_of_enabled() {
    let mut plugin = SpeechDenoiserPlugin::new(1);
    plugin.initialize(48000).expect("initialize");

    // Enabled
    plugin
        .set_parameter("enabled".into(), ParameterValue::Bool(true))
        .unwrap();
    let latency_on = plugin.latency_samples();

    // Disabled
    plugin
        .set_parameter("enabled".into(), ParameterValue::Bool(false))
        .unwrap();
    let latency_off = plugin.latency_samples();

    assert_eq!(latency_on, 480, "Latency when enabled must be 480");
    assert_eq!(latency_off, 480, "Latency when disabled must still be 480");
}

/// 1.2 CRITICAL: non-multiple-of-480 block sizes must return an error.
#[test]
fn process_rejects_non_multiple_of_480_when_enabled() {
    let mut plugin = SpeechDenoiserPlugin::new(1);
    plugin
        .set_parameter("enabled".into(), ParameterValue::Bool(true))
        .unwrap();
    plugin.initialize(48000).expect("initialize");

    for &bad_size in &[64usize, 128, 256, 512, 1024] {
        let mut buffer = vec![0.0f32; bad_size];
        let ctx = ProcessContext::new(48000, bad_size);
        assert!(
            plugin.process_in_place(&mut buffer, &ctx).is_err(),
            "Block size {bad_size} (not a multiple of 480) must be rejected"
        );
    }
}

/// 1.2: exact multiples of 480 must succeed.
#[test]
fn process_accepts_multiples_of_480() {
    let mut plugin = SpeechDenoiserPlugin::new(1);
    plugin
        .set_parameter("enabled".into(), ParameterValue::Bool(true))
        .unwrap();
    plugin.initialize(48000).expect("initialize");

    for &good_size in &[480usize, 960, 1440] {
        let mut buffer = vec![0.0f32; good_size];
        let ctx = ProcessContext::new(48000, good_size);
        assert!(
            plugin.process_in_place(&mut buffer, &ctx).is_ok(),
            "Block size {good_size} must be accepted"
        );
    }
}

/// 2.3: buffer too small for the declared num_frames must return an error.
#[test]
fn process_rejects_undersized_buffer() {
    let mut plugin = SpeechDenoiserPlugin::new(1);
    plugin
        .set_parameter("enabled".into(), ParameterValue::Bool(true))
        .unwrap();
    plugin.initialize(48000).expect("initialize");

    // buffer has 480 samples but context claims 960 frames (needs 960 samples for 1 ch)
    let mut buffer = vec![0.0f32; 480];
    let ctx = ProcessContext::new(48000, 960);
    assert!(
        plugin.process_in_place(&mut buffer, &ctx).is_err(),
        "Undersized buffer must be rejected"
    );
}
