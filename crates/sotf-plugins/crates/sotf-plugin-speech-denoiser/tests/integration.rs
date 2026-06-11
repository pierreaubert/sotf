// Integration tests for sotf-plugin-speech-denoiser exercising the public InPlacePlugin trait.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, ProcessContext};
use sotf_plugin_speech_denoiser::{
    SPEECH_DENOISER_FRAME_SIZE, SpeechDenoiserPlugin, SpeechDenoiserPluginParams,
};

#[test]
fn integration_plugin_info_and_channels() {
    let plugin = SpeechDenoiserPlugin::new(2);
    assert_eq!(plugin.channels(), 2);
    assert_eq!(plugin.input_channels(), 2);
    let info = plugin.info();
    assert_eq!(info.name, "Speech Denoiser");
}

#[test]
fn integration_default_parameters() {
    let plugin = SpeechDenoiserPlugin::new(1);
    let params = plugin.parameters();
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].id, ParameterId::from("enabled"));

    let v = plugin.get_parameter(&ParameterId::from("enabled")).unwrap();
    assert_eq!(v, ParameterValue::Bool(true));
}

#[test]
fn integration_parameter_roundtrip_and_validation() {
    let mut plugin = SpeechDenoiserPlugin::new(1);

    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();
    let v = plugin.get_parameter(&ParameterId::from("enabled")).unwrap();
    assert_eq!(v, ParameterValue::Bool(false));

    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(true))
        .unwrap();
    let v = plugin.get_parameter(&ParameterId::from("enabled")).unwrap();
    assert_eq!(v, ParameterValue::Bool(true));

    // Unknown parameter.
    let res = plugin.set_parameter(ParameterId::from("strength"), ParameterValue::Float(0.5));
    assert!(res.is_err());

    // Type mismatch: enabled expects a bool.
    let res = plugin.set_parameter(ParameterId::from("enabled"), ParameterValue::Float(1.0));
    assert!(res.is_err());
}

#[test]
fn integration_initialize_rejects_non_48khz() {
    let mut plugin = SpeechDenoiserPlugin::new(1);
    assert!(plugin.initialize(44100).is_err());
    assert!(plugin.initialize(96000).is_err());
    assert!(plugin.initialize(48000).is_ok());
}

#[test]
fn integration_disabled_is_transparent_after_latency() {
    let mut plugin = SpeechDenoiserPlugin::new(2);
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();
    plugin.initialize(48000).unwrap();

    // Process two frames: the first 480-sample frame is the startup delay,
    // the second passes through.
    let mut buffer: Vec<f32> = (0..1920)
        .map(|i| ((i % 100) as f32 - 50.0) / 100.0)
        .collect();
    let input = buffer.clone();
    let ctx = ProcessContext::new(48000, 960);
    let written = plugin.process_in_place(&mut buffer, &ctx).unwrap();
    assert_eq!(written, 480);
    assert_eq!(&buffer[..960], &input[960..1920]);
}

#[test]
fn integration_enabled_processes_frame_size_blocks() {
    let mut plugin = SpeechDenoiserPlugin::new(1);
    plugin.initialize(48000).unwrap();

    let mut buffer: Vec<f32> = (0..SPEECH_DENOISER_FRAME_SIZE)
        .map(|i| ((i % 50) as f32 - 25.0) / 100.0)
        .collect();
    let ctx = ProcessContext::new(48000, SPEECH_DENOISER_FRAME_SIZE);
    plugin.process_in_place(&mut buffer, &ctx).unwrap();
    assert!(buffer.iter().all(|s| s.is_finite()));

    // A second frame should produce output.
    let written = plugin
        .process_in_place(&mut buffer, &ctx)
        .expect("second frame must process");
    assert_eq!(written, SPEECH_DENOISER_FRAME_SIZE);
}

#[test]
fn integration_reset_is_recoverable() {
    let mut plugin = SpeechDenoiserPlugin::new(1);
    plugin.initialize(48000).unwrap();

    let mut buffer = vec![0.2f32; SPEECH_DENOISER_FRAME_SIZE];
    let ctx = ProcessContext::new(48000, SPEECH_DENOISER_FRAME_SIZE);
    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    let latency_before = plugin.latency_samples();
    plugin.reset();
    let latency_after = plugin.latency_samples();
    assert_eq!(latency_after, latency_before);

    plugin.process_in_place(&mut buffer, &ctx).unwrap();
}

#[test]
fn integration_process_rejects_bad_block_size_and_buffer() {
    let mut plugin = SpeechDenoiserPlugin::new(1);
    plugin.initialize(48000).unwrap();

    // Non-multiple of 480 must fail.
    for &bad_size in &[64usize, 128, 256, 512, 1024] {
        let mut buffer = vec![0.0f32; bad_size];
        let ctx = ProcessContext::new(48000, bad_size);
        assert!(plugin.process_in_place(&mut buffer, &ctx).is_err());
    }

    // Buffer smaller than the declared frame count must fail.
    let mut small_buffer = vec![0.0f32; 480];
    let ctx = ProcessContext::new(48000, 960);
    assert!(plugin.process_in_place(&mut small_buffer, &ctx).is_err());
}

#[test]
fn integration_from_params_applies_initial_state() {
    let plugin =
        SpeechDenoiserPlugin::from_params(1, SpeechDenoiserPluginParams { enabled: false });
    let v = plugin.get_parameter(&ParameterId::from("enabled")).unwrap();
    assert_eq!(v, ParameterValue::Bool(false));
    assert_eq!(plugin.channels(), 1);
}
