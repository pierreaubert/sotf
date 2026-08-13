// Integration tests for sotf-plugin-speech-denoiser exercising the public InPlacePlugin trait.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::ParameterSet;
use sotf_host::plugin::ProcessContext;
use sotf_host::{CountingAlloc, assert_no_allocs};
use sotf_plugin_speech_denoiser::{
    SPEECH_DENOISER_FRAME_SIZE, SpeechDenoiserPlugin, SpeechDenoiserPluginParams,
};

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

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
    // followed by the first input frame.
    let mut buffer: Vec<f32> = (0..1920)
        .map(|i| ((i % 100) as f32 - 50.0) / 100.0)
        .collect();
    let input = buffer.clone();
    let ctx = ProcessContext::new(48000, 960);
    let written = plugin.process_in_place(&mut buffer, &ctx).unwrap();
    assert_eq!(written, 960);
    assert_eq!(&buffer[960..1920], &input[..960]);
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

    // Arbitrary host block sizes are accepted.
    for &bad_size in &[64usize, 128, 256, 512, 1024] {
        let mut buffer = vec![0.0f32; bad_size];
        let ctx = ProcessContext::new(48000, bad_size);
        assert_eq!(
            plugin.process_in_place(&mut buffer, &ctx).unwrap(),
            bad_size
        );
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

#[test]
fn first_callback_and_live_toggle_are_allocation_free_without_warmup() {
    let mut plugin = SpeechDenoiserPlugin::new(2);
    plugin.initialize(48000).unwrap();
    let mut buffer = vec![0.1; SPEECH_DENOISER_FRAME_SIZE * 2];
    let context = ProcessContext::new(48000, SPEECH_DENOISER_FRAME_SIZE);
    assert_no_allocs("Speech Denoiser cold first callback", || {
        plugin.process_in_place(&mut buffer, &context).unwrap();
    });
    let mut values = ParameterSet::new();
    values.insert(ParameterId::from("enabled"), ParameterValue::Bool(false));
    assert_no_allocs("Speech Denoiser live bypass setter", || {
        plugin.apply_values(values).unwrap();
    });
    assert_no_allocs("Speech Denoiser first bypass-transition callback", || {
        plugin.process_in_place(&mut buffer, &context).unwrap();
    });
}

#[test]
fn construction_and_process_contract_reject_invalid_dimensions_and_rate() {
    assert!(
        SpeechDenoiserPlugin::try_from_params(0, SpeechDenoiserPluginParams::default()).is_err()
    );
    for channels in [3, 6, 8, 12] {
        assert!(
            SpeechDenoiserPlugin::try_from_params(channels, SpeechDenoiserPluginParams::default())
                .is_err()
        );
    }

    let mut plugin = SpeechDenoiserPlugin::new(2);
    let mut empty = [];
    assert!(
        plugin
            .process_in_place(&mut empty, &ProcessContext::new(48000, 0))
            .unwrap_err()
            .contains("initialized")
    );
    plugin.initialize(48000).unwrap();
    assert!(
        plugin
            .process_in_place(&mut empty, &ProcessContext::new(44100, 0))
            .unwrap_err()
            .contains("context rate")
    );
    assert!(
        plugin
            .process_in_place(&mut empty, &ProcessContext::new(48000, usize::MAX))
            .unwrap_err()
            .contains("overflow")
    );
}

#[test]
fn factory_parameter_json_is_strict_and_backward_compatible() {
    let missing: SpeechDenoiserPluginParams = serde_json::from_str("{}").unwrap();
    assert!(missing.enabled);
    for enabled in [false, true] {
        let json = format!(r#"{{"enabled":{enabled}}}"#);
        let decoded: SpeechDenoiserPluginParams = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.enabled, enabled);
    }
    assert!(serde_json::from_str::<SpeechDenoiserPluginParams>(r#"{"enabled":1}"#).is_err());
    assert!(
        serde_json::from_str::<SpeechDenoiserPluginParams>(r#"{"enabled":true,"unknown":1}"#)
            .is_err()
    );
}
