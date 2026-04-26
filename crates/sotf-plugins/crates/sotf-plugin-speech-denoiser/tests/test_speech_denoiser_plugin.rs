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

    let mut buffer = vec![0.25, -0.25, 0.5, -0.5];
    let input = buffer.clone();
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: 2,
    };
    assert_eq!(plugin.process_in_place(&mut buffer, &context).unwrap(), 2);
    assert_eq!(buffer, input);
}
