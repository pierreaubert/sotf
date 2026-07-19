#![cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]

use std::time::{Duration, Instant};

use sotf_host::{
    DawHost, ExternalPluginProcessEvent, ExternalPluginSandboxPolicy, ExternalPluginWorkerCommand,
    IsolatedExternalPlugin, IsolatedExternalPluginConfig, Plugin, PluginDescriptor, PluginFormat,
    ProcessContext,
};

#[test]
fn isolated_external_plugin_processes_block_through_worker_binary() {
    let (_dir, descriptor) = test_descriptor("external-worker-smoke");
    let worker_binary = env!("CARGO_BIN_EXE_sotf-external-plugin-worker");
    let mut plugin = IsolatedExternalPlugin::new(
        descriptor,
        48_000,
        IsolatedExternalPluginConfig {
            deadline: Duration::from_secs(2),
            worker_command: ExternalPluginWorkerCommand::new(worker_binary)
                .arg("--idle-sleep-micros")
                .arg("50"),
            sandbox_policy: ExternalPluginSandboxPolicy::disabled(),
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(plugin.launch_error(), None);
    assert_eq!(plugin.worker_start_count(), 1);

    let input = vec![0.25, -0.5, 1.0, -1.0];
    let mut output = vec![0.0; input.len()];
    let frames = plugin
        .process(&input, &mut output, &ProcessContext::new(48_000, 2))
        .unwrap();

    assert_eq!(frames, 2);
    assert_eq!(output, input);
    assert_eq!(plugin.block_timeout_count(), 0);
    assert_eq!(plugin.block_worker_failure_count(), 0);
    assert_eq!(plugin.block_wrong_sequence_count(), 0);
}

#[test]
fn isolated_external_plugin_worker_exit_can_be_restarted_by_control_side() {
    let (_dir, descriptor) = test_descriptor("external-worker-restart");
    let worker_binary = env!("CARGO_BIN_EXE_sotf-external-plugin-worker");
    let mut plugin = IsolatedExternalPlugin::new(
        descriptor,
        48_000,
        IsolatedExternalPluginConfig {
            worker_command: ExternalPluginWorkerCommand::new(worker_binary).arg("--once"),
            sandbox_policy: ExternalPluginSandboxPolicy::disabled(),
            ..Default::default()
        },
    )
    .unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if plugin.poll_worker().unwrap().is_some() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "external plugin worker did not exit"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(plugin.worker_exit_count(), 1);
    plugin.ensure_worker_running().unwrap();
    assert_eq!(plugin.worker_start_count(), 2);
}

#[test]
fn isolated_external_plugin_dead_worker_falls_back_without_crashing_host() {
    let (_dir, descriptor) = test_descriptor("external-worker-dead-fallback");
    let worker_binary = env!("CARGO_BIN_EXE_sotf-external-plugin-worker");
    let mut plugin = IsolatedExternalPlugin::new(
        descriptor,
        48_000,
        IsolatedExternalPluginConfig {
            deadline: Duration::from_millis(1),
            worker_command: ExternalPluginWorkerCommand::new(worker_binary).arg("--once"),
            sandbox_policy: ExternalPluginSandboxPolicy::disabled(),
            ..Default::default()
        },
    )
    .unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if plugin.poll_worker().unwrap().is_some() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "external plugin worker did not exit"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    let input = vec![0.25, -0.5, 1.0, -1.0];
    let mut output = vec![0.0; input.len()];
    let frames = plugin
        .process(&input, &mut output, &ProcessContext::new(48_000, 2))
        .unwrap();

    assert_eq!(frames, 2);
    assert_eq!(output, input);
    assert_eq!(plugin.block_timeout_count(), 1);
    assert_eq!(plugin.worker_start_count(), 1);
    assert_eq!(plugin.worker_exit_count(), 1);
}

#[test]
fn daw_host_can_poll_and_restart_isolated_external_plugin_workers() {
    let (_dir, descriptor) = test_descriptor("external-worker-host-control");
    let worker_binary = env!("CARGO_BIN_EXE_sotf-external-plugin-worker");
    let plugin = IsolatedExternalPlugin::new(
        descriptor,
        48_000,
        IsolatedExternalPluginConfig {
            worker_command: ExternalPluginWorkerCommand::new(worker_binary).arg("--once"),
            sandbox_policy: ExternalPluginSandboxPolicy::disabled(),
            ..Default::default()
        },
    )
    .unwrap();
    let mut host = DawHost::new(2, 48_000);
    host.add_plugin(Box::new(plugin)).unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let reports = host.poll_isolated_external_plugin_workers();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].plugin_index, 0);
        assert_eq!(reports[0].node_id, 0);
        assert_eq!(reports[0].error, None);
        assert_eq!(reports[0].worker_start_count, 1);
        if matches!(
            reports[0].event,
            Some(ExternalPluginProcessEvent::Exited { .. })
        ) {
            assert_eq!(reports[0].plugin_index, 0);
            assert_eq!(reports[0].worker_exit_count, 1);
            assert_eq!(reports[0].worker_launch_failure_count, 0);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "host did not observe external plugin worker exit"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    let reports = host.ensure_isolated_external_plugin_workers_running();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].plugin_index, 0);
    assert_eq!(reports[0].node_id, 0);
    assert_eq!(reports[0].error, None);
    assert!(matches!(
        reports[0].event,
        Some(ExternalPluginProcessEvent::Started { .. })
    ));
    assert_eq!(reports[0].worker_start_count, 2);
    assert_eq!(reports[0].worker_launch_failure_count, 0);
    assert_eq!(reports[0].block_timeout_count, 0);
    assert_eq!(reports[0].block_worker_failure_count, 0);
    assert_eq!(reports[0].block_wrong_sequence_count, 0);
}

#[test]
fn daw_host_can_report_isolated_external_plugin_worker_launch_failures() {
    let (_dir, descriptor) = test_descriptor("external-worker-host-launch-failure");
    let mut host = DawHost::new(2, 48_000);
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
    host.add_plugin(Box::new(plugin)).unwrap();

    let reports = host.ensure_isolated_external_plugin_workers_running();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].plugin_index, 0);
    assert_eq!(reports[0].node_id, 0);
    assert!(reports[0].event.is_none());
    let error = reports[0].error.as_deref().unwrap_or_default();
    assert!(
        error.contains("failed to launch external plugin worker"),
        "{error}"
    );
    assert_eq!(reports[0].worker_launch_failure_count, 1);
    assert_eq!(reports[0].worker_start_count, 0);
    assert_eq!(reports[0].worker_exit_count, 0);

    let reports = host.poll_isolated_external_plugin_workers();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].plugin_index, 0);
    assert_eq!(reports[0].node_id, 0);
    assert_eq!(
        reports[0].event,
        Some(ExternalPluginProcessEvent::NotRunning)
    );
    assert_eq!(reports[0].error, None);
}

fn test_descriptor(name: &str) -> (tempfile::TempDir, PluginDescriptor) {
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
        scan_status: sotf_host::PluginScanStatus::Discovered,
    };
    (dir, descriptor)
}
