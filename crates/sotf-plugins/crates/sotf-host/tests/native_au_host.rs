#![cfg(all(feature = "external-plugin-au", target_os = "macos"))]

use sotf_host::external_plugin::{
    ExternalHostingBackend, ExternalPlugin, PluginDescriptor, PluginFormat, PluginScanStatus,
};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_host::serialization::SerializablePlugin;
use std::path::PathBuf;

#[test]
#[ignore = "requires the built-in Apple AUDelay Audio Unit"]
fn native_apple_audio_unit_renders_audio() {
    let descriptor = PluginDescriptor {
        id: "au.AUDelay".into(),
        name: "AUDelay".into(),
        vendor: "Apple".into(),
        version: "system".into(),
        format: PluginFormat::AudioUnit,
        path: PathBuf::from("/System/Library/Components/CoreAudio.component"),
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec!["audio-effect".into()],
        scan_status: PluginScanStatus::Loadable,
    };

    let mut plugin = ExternalPlugin::new(&descriptor, 48_000).expect("load Apple AUDelay");
    assert_eq!(plugin.hosting_backend(), ExternalHostingBackend::AudioUnit);
    assert_ne!(plugin.descriptor().id, descriptor.id);
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
    let mix = plugin
        .parameters()
        .into_iter()
        .find(|parameter| {
            let name = parameter.name.to_ascii_lowercase();
            name.contains("dry") || name.contains("wet")
        })
        .expect("AUDelay dry/wet parameter metadata");
    let dry = mix.min_value.clone().expect("AUDelay dry/wet minimum");
    plugin
        .set_parameter(mix.id.clone(), dry.clone())
        .expect("set AUDelay dry/wet mix");
    assert_eq!(plugin.get_parameter(&mix.id), Some(dry));

    let frames = 127;
    let input = (0..frames * 2)
        .map(|sample| (sample as f32 / (frames * 2) as f32) * 0.5 - 0.25)
        .collect::<Vec<_>>();
    let mut output = vec![f32::NAN; input.len()];
    let context = ProcessContext::new(48_000, frames);
    assert_eq!(
        plugin.process(&input, &mut output, &context).unwrap(),
        frames
    );
    assert!(output.iter().all(|sample| sample.is_finite()));
    for (actual, expected) in output.iter().zip(&input) {
        assert!((actual - expected).abs() < 1.0e-5);
    }

    let preset = plugin.serialize().expect("save AudioUnit state");
    let state = preset
        .external_plugin_state()
        .unwrap()
        .expect("external state envelope");
    assert!(!state.opaque_state.is_empty());

    let mut restored =
        ExternalPlugin::from_placeholder_state(&state, 48_000).expect("restore AudioUnit state");
    let mut restored_output = vec![0.0; input.len()];
    restored
        .process(&input, &mut restored_output, &context)
        .unwrap();
    assert_eq!(restored_output, output);
}
