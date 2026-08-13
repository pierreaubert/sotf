#![cfg(feature = "external-plugin-vst3")]

use sotf_host::external_plugin::{
    ExternalHostingBackend, ExternalPlugin, PluginDescriptor, PluginFormat, PluginScanStatus,
};
use sotf_host::plugin::{
    MidiEvent, MidiMessage, ParameterEvent, Plugin, ProcessContext, TransportInfo,
};
use sotf_host::serialization::SerializablePlugin;
use std::path::PathBuf;

#[test]
#[ignore = "requires SOTF_TEST_VST3_PLUGIN to point to the built plugins-nih gain VST3 library"]
fn native_vst3_gain_processes_audio() {
    let path = PathBuf::from(
        std::env::var_os("SOTF_TEST_VST3_PLUGIN")
            .expect("SOTF_TEST_VST3_PLUGIN must point to a .vst3 file or bundle"),
    );
    let descriptor = PluginDescriptor {
        id: "vst3.SOTF: Gain".into(),
        name: "SOTF: Gain".into(),
        vendor: "SOTF".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        format: PluginFormat::Vst3,
        path,
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec!["audio-effect".into()],
        scan_status: PluginScanStatus::Loadable,
    };

    let mut plugin = ExternalPlugin::new(&descriptor, 48_000).expect("load native VST3 gain");
    assert_eq!(plugin.hosting_backend(), ExternalHostingBackend::Vst3);
    assert_eq!(plugin.descriptor().name, "SOTF: Gain");
    assert_ne!(plugin.descriptor().id, descriptor.id);
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
    let gain = plugin
        .parameters()
        .into_iter()
        .find(|parameter| {
            parameter.name.to_ascii_lowercase().contains("gain")
                && matches!(
                    parameter.default_value,
                    sotf_host::parameters::ParameterValue::Float(_)
                )
        })
        .expect("VST3 gain parameter metadata");
    plugin
        .set_parameter(gain.id.clone(), gain.default_value.clone())
        .expect("queue VST3 parameter change");
    assert_eq!(
        plugin.get_parameter(&gain.id),
        Some(gain.default_value.clone())
    );

    let frames = 127;
    let input = (0..frames * 2)
        .map(|sample| (sample as f32 / (frames * 2) as f32) * 0.5 - 0.25)
        .collect::<Vec<_>>();
    let mut output = vec![f32::NAN; input.len()];
    let midi = [MidiEvent::new(17, MidiMessage::note_on(0, 60, 100))];
    let automation = [ParameterEvent::new(
        83,
        gain.id.clone(),
        gain.default_value.clone(),
    )];
    let context = ProcessContext::new(48_000, frames)
        .with_transport(TransportInfo::at_sample(96_000, 48_000).with_tempo(75.0, 48_000))
        .with_all_events(&midi, &[], &automation);
    assert_eq!(
        plugin.process(&input, &mut output, &context).unwrap(),
        frames
    );
    for (actual, expected) in output.iter().zip(&input) {
        assert!(actual.is_finite());
        assert!((actual - expected).abs() < 1.0e-5);
    }

    let preset = plugin.serialize().expect("save VST3 state");
    let state = preset
        .external_plugin_state()
        .unwrap()
        .expect("external state envelope");
    assert!(!state.opaque_state.is_empty());

    let mut restored =
        ExternalPlugin::from_placeholder_state(&state, 48_000).expect("restore VST3 state");
    let mut restored_output = vec![0.0; input.len()];
    restored
        .process(&input, &mut restored_output, &context)
        .unwrap();
    assert_eq!(restored_output, output);

    let minimum = gain.min_value.expect("gain minimum");
    plugin
        .set_parameter(gain.id, minimum)
        .expect("queue non-default VST3 gain");
    let mut attenuated = vec![0.0; input.len()];
    for _ in 0..32 {
        plugin.process(&input, &mut attenuated, &context).unwrap();
    }
    assert!(
        attenuated
            .iter()
            .zip(&input)
            .any(|(actual, original)| (actual - original).abs() > 1.0e-4),
        "VST3 parameter event did not change native processing"
    );
}
