#![cfg(feature = "external-plugin-clap")]

use sotf_host::external_plugin::{
    ExternalHostingBackend, ExternalPlugin, PluginDescriptor, PluginFormat, PluginScanStatus,
};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_host::serialization::SerializablePlugin;
use sotf_host::{
    ExternalPluginSandboxMode, ExternalPluginSandboxPolicy, ExternalPluginState,
    ExternalPluginWorkerCommand, IsolatedExternalPlugin, IsolatedExternalPluginConfig,
};
use std::path::PathBuf;
use std::time::Duration;

#[test]
#[ignore = "requires SOTF_TEST_CLAP_PLUGIN to point to the built plugins-nih gain CLAP library"]
fn native_clap_gain_processes_and_round_trips_state() {
    let descriptor = clap_gain_descriptor();

    let mut plugin = ExternalPlugin::new(&descriptor, 48_000).expect("load native CLAP gain");
    assert_eq!(plugin.hosting_backend(), ExternalHostingBackend::Clap);
    assert_eq!(plugin.descriptor().id, "org.spinorama.sotf.gain");
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
    let gain = plugin
        .parameters()
        .into_iter()
        .find(|parameter| parameter.name.to_ascii_lowercase().contains("gain"))
        .expect("CLAP gain parameter metadata");
    plugin
        .set_parameter(gain.id.clone(), gain.default_value.clone())
        .expect("queue CLAP parameter event");
    assert_eq!(plugin.get_parameter(&gain.id), Some(gain.default_value));

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
    for (actual, expected) in output.iter().zip(&input) {
        assert!(actual.is_finite());
        assert!((actual - expected).abs() < 1.0e-5);
    }

    let preset = plugin.serialize().expect("save CLAP state");
    let state = preset
        .external_plugin_state()
        .unwrap()
        .expect("external state envelope");
    assert!(!state.opaque_state.is_empty());

    let mut restored =
        ExternalPlugin::from_placeholder_state(&state, 48_000).expect("restore CLAP state");
    let mut restored_output = vec![0.0; input.len()];
    restored
        .process(&input, &mut restored_output, &context)
        .unwrap();
    assert_eq!(restored_output, output);
}

#[test]
#[ignore = "requires SOTF_TEST_CLAP_PLUGIN to point to the built plugins-nih gain CLAP library"]
fn isolated_native_clap_worker_processes_and_restores_state() {
    let descriptor = clap_gain_descriptor();
    let mut source = ExternalPlugin::new(&descriptor, 48_000).expect("load native CLAP gain");
    let gain = source
        .parameters()
        .into_iter()
        .find(|parameter| parameter.name.to_ascii_lowercase().contains("gain"))
        .expect("CLAP gain parameter metadata");
    source
        .set_parameter(gain.id, gain.min_value.expect("gain minimum"))
        .expect("queue non-default CLAP gain");

    let frames = 127;
    let input = (0..frames * 2)
        .map(|sample| (sample as f32 / (frames * 2) as f32) * 0.5 - 0.25)
        .collect::<Vec<_>>();
    let context = ProcessContext::new(48_000, frames);
    let mut source_output = vec![0.0; input.len()];
    for _ in 0..32 {
        source
            .process(&input, &mut source_output, &context)
            .unwrap();
    }

    let preset = source.serialize().expect("save CLAP state");
    let in_process_state = preset
        .external_plugin_state()
        .unwrap()
        .expect("external state envelope");
    let isolated_state = ExternalPluginState::new(
        in_process_state.descriptor.clone(),
        ExternalPluginSandboxMode::Isolated,
        in_process_state.opaque_state.clone(),
    );

    let worker_binary = env!("CARGO_BIN_EXE_sotf-external-plugin-worker");
    let config = || IsolatedExternalPluginConfig {
        deadline: Duration::from_secs(2),
        worker_command: ExternalPluginWorkerCommand::new(worker_binary)
            .arg("--idle-sleep-micros")
            .arg("50"),
        sandbox_policy: ExternalPluginSandboxPolicy::disabled(),
        initial_state: Some(isolated_state.clone()),
        ..Default::default()
    };

    let mut first =
        IsolatedExternalPlugin::from_placeholder_state(&isolated_state, 48_000, config())
            .expect("start native CLAP worker from serialized state");
    assert_eq!(first.launch_error(), None);
    let mut first_output = vec![0.0; input.len()];
    for _ in 0..32 {
        first.process(&input, &mut first_output, &context).unwrap();
    }

    let mut second =
        IsolatedExternalPlugin::from_placeholder_state(&isolated_state, 48_000, config())
            .expect("start fresh native CLAP worker from serialized state");
    assert_eq!(second.launch_error(), None);
    let mut second_output = vec![0.0; input.len()];
    for _ in 0..32 {
        second
            .process(&input, &mut second_output, &context)
            .unwrap();
    }

    assert_eq!(first.latency_samples(), source.latency_samples());
    assert_eq!(second.latency_samples(), source.latency_samples());
    assert_eq!(second_output, first_output);
    assert!(
        first_output
            .iter()
            .zip(&input)
            .any(|(actual, original)| (actual - original).abs() > 1.0e-4),
        "serialized CLAP gain state did not affect isolated native processing"
    );
}

fn clap_gain_descriptor() -> PluginDescriptor {
    let path = PathBuf::from(
        std::env::var_os("SOTF_TEST_CLAP_PLUGIN")
            .expect("SOTF_TEST_CLAP_PLUGIN must point to a .clap file or bundle"),
    );
    PluginDescriptor {
        id: "org.spinorama.sotf.gain".into(),
        name: "SOTF: Gain".into(),
        vendor: "SOTF".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        format: PluginFormat::Clap,
        path,
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec!["audio-effect".into()],
        scan_status: PluginScanStatus::Loadable,
    }
}
