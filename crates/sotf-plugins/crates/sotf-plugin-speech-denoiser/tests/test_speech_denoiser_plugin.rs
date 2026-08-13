use sotf_host::parameters::ParameterValue;
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;
use sotf_plugin_speech_denoiser::{
    SPEECH_DENOISER_FRAME_SIZE, SpeechDenoiserData, SpeechDenoiserPlugin,
};

#[test]
fn disabled_is_transparent() {
    let mut plugin = SpeechDenoiserPlugin::new(2);
    plugin
        .set_parameter("enabled".into(), ParameterValue::Bool(false))
        .expect("set enabled");
    plugin.initialize(48000).expect("initialize");

    // Process an arbitrary host block. The first 480 output samples are the
    // declared startup latency, followed by the beginning of the dry stream.
    let mut buffer: Vec<f32> = (0..1920)
        .map(|i| ((i % 100) as f32 - 50.0) / 100.0)
        .collect();
    let input = buffer.clone();
    let context = ProcessContext::new(48000, 960);
    let written = plugin.process_in_place(&mut buffer, &context).unwrap();
    assert_eq!(written, 960);
    assert_eq!(&buffer[960..1920], &input[..960]);
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
fn accepts_non_multiple_of_480_and_preserves_latency() {
    let mut plugin = SpeechDenoiserPlugin::new(1);
    plugin.initialize(48000).expect("initialize");

    let mut buffer = vec![0.0f32; 512];
    let context = ProcessContext::new(48000, 512);
    assert_eq!(plugin.process_in_place(&mut buffer, &context).unwrap(), 512);
    assert!(buffer[..480].iter().all(|sample| *sample == 0.0));
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

#[test]
fn arbitrary_partitions_match_one_continuous_stream() {
    let mut plugin = SpeechDenoiserPlugin::new(1);
    plugin
        .set_parameter("enabled".into(), ParameterValue::Bool(true))
        .unwrap();
    plugin.initialize(48000).expect("initialize");

    let source: Vec<f32> = (0..1440)
        .map(|i| ((i * 17 % 101) as f32 - 50.0) / 100.0)
        .collect();
    let mut continuous = source.clone();
    let mut partitioned = source;
    let whole = ProcessContext::new(48000, continuous.len());
    assert_eq!(
        plugin.process_in_place(&mut continuous, &whole).unwrap(),
        1440
    );
    let continuous_data = *plugin
        .get_data()
        .unwrap()
        .downcast::<SpeechDenoiserData>()
        .unwrap();

    plugin.reset();
    let mut offset = 0;
    for requested in [1usize, 16, 63, 64, 127, 128, 256, 479, 480, 481, 512, 1024] {
        if offset == partitioned.len() {
            break;
        }
        let size = requested.min(partitioned.len() - offset);
        let mut block = partitioned[offset..offset + size].to_vec();
        let ctx = ProcessContext::new(48000, size);
        assert_eq!(plugin.process_in_place(&mut block, &ctx).unwrap(), size);
        partitioned[offset..offset + size].copy_from_slice(&block);
        offset += size;
    }
    assert_eq!(offset, partitioned.len());
    for (actual, expected) in partitioned.iter().zip(continuous) {
        assert!(
            (actual - expected).abs() < 1e-5,
            "stream changed: {actual} vs {expected}"
        );
    }
    let partitioned_data = *plugin
        .get_data()
        .unwrap()
        .downcast::<SpeechDenoiserData>()
        .unwrap();
    assert_eq!(partitioned_data, continuous_data);
}

#[test]
fn non_finite_input_is_sanitized_and_does_not_poison_following_audio() {
    let mut plugin = SpeechDenoiserPlugin::new(1);
    plugin
        .set_parameter("enabled".into(), ParameterValue::Bool(true))
        .unwrap();
    plugin.initialize(48000).expect("initialize");

    let mut buffer = vec![0.1f32; SPEECH_DENOISER_FRAME_SIZE];
    buffer[10] = f32::NAN;
    buffer[11] = f32::INFINITY;
    buffer[12] = f32::NEG_INFINITY;
    let ctx = ProcessContext::new(48000, buffer.len());
    assert_eq!(plugin.process_in_place(&mut buffer, &ctx).unwrap(), 480);
    assert!(buffer.iter().all(|sample| sample.is_finite()));

    let mut next = vec![0.1f32; SPEECH_DENOISER_FRAME_SIZE];
    assert_eq!(plugin.process_in_place(&mut next, &ctx).unwrap(), 480);
    assert!(next.iter().all(|sample| sample.is_finite()));
}

#[test]
fn published_frame_size_can_drive_allocation_benchmark() {
    let mut plugin = SpeechDenoiserPlugin::new(2);
    plugin.initialize(48000).expect("initialize");

    let mut buffer = vec![0.0f32; SPEECH_DENOISER_FRAME_SIZE * 2];
    let ctx = ProcessContext::new(48000, SPEECH_DENOISER_FRAME_SIZE);

    plugin
        .process_in_place(&mut buffer, &ctx)
        .expect("published frame size must be a valid processing block");
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
