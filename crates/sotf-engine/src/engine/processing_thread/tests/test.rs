use super::super::super::{ProcessingCommand, ProcessingResponse, ThreadEvent};
use super::super::processing_state::ProcessingState;
use super::super::processing_state::handle_processing_command;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use crate::IsolatedExternalPluginWorkerEvent;
use sotf_plugins::PluginHost;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use sotf_plugins::{
    ExternalPluginWorkerCommand, IsolatedExternalPlugin, IsolatedExternalPluginConfig,
    PluginDescriptor, PluginFormat,
};
use std::time::Duration;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn test_isolated_external_plugin_descriptor(name: &str) -> (tempfile::TempDir, PluginDescriptor) {
    let dir = tempfile::tempdir().unwrap();
    let plugin_path = dir.path().join(format!("{name}.clap"));
    std::fs::write(&plugin_path, b"stub external plugin").unwrap();
    let plugin_path = plugin_path.canonicalize().unwrap();
    let descriptor = PluginDescriptor {
        id: format!("test.{name}"),
        name: name.into(),
        vendor: "SOTF Test".into(),
        version: "0.1.0".into(),
        format: PluginFormat::Clap,
        path: plugin_path,
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: Vec::new(),
        scan_status: sotf_plugins::PluginScanStatus::Discovered,
    };
    (dir, descriptor)
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn test_processing_state_with_invalid_isolated_plugin() -> (ProcessingState, tempfile::TempDir) {
    let (tempdir, descriptor) =
        test_isolated_external_plugin_descriptor("engine-processing-invalid");
    let plugin = IsolatedExternalPlugin::new(
        descriptor,
        48_000,
        IsolatedExternalPluginConfig {
            worker_command: ExternalPluginWorkerCommand::new(
                "/definitely/not/a/real/sotf/external/plugin/worker",
            ),
            start_worker: false,
            ..Default::default()
        },
    )
    .unwrap();

    let mut host = PluginHost::new(2, 48_000);
    host.add_plugin(Box::new(plugin)).unwrap();

    let mut state = ProcessingState::new(
        2,
        48_000,
        #[cfg(feature = "streaming")]
        None,
    );
    *state.host = host;
    (state, tempdir)
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn test_handle_processing_command_polls_isolated_external_plugin_statuses_without_launching() {
    let (mut state, _tempdir) = test_processing_state_with_invalid_isolated_plugin();

    let (response_tx, _response_rx) = std::sync::mpsc::channel::<ProcessingResponse>();
    let (event_tx, event_rx) = std::sync::mpsc::channel::<ThreadEvent>();

    let shutdown = handle_processing_command(
        ProcessingCommand::PollIsolatedExternalPluginWorkers,
        &mut state,
        &response_tx,
        &event_tx,
    );
    assert!(!shutdown);

    let statuses = match event_rx.recv_timeout(Duration::from_secs(1)).unwrap() {
        ThreadEvent::IsolatedExternalPluginWorkerStatuses(statuses) => statuses,
        event => panic!("expected status event, got {:?}", event),
    };
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].plugin_index, 0);
    assert_eq!(statuses[0].node_id, 0);
    assert_eq!(statuses[0].error, None);
    assert_eq!(statuses[0].worker_launch_failure_count, 0);
    assert!(matches!(
        statuses[0].event,
        Some(IsolatedExternalPluginWorkerEvent::NotRunning)
    ));
}
