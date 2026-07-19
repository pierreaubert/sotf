use super::isolated_external_plugin::IsolatedExternalPlugin;
use super::isolated_external_plugin_config::IsolatedExternalPluginConfig;
use super::isolated_external_plugin_config::build_worker_launch_command;
use crate::external_plugin::{
    ExternalPluginSandboxMode, ExternalPluginState, PluginDescriptor, plan_external_plugin_hosting,
};
use crate::external_plugin_ipc::SecurePluginSharedMemory;
use crate::external_plugin_process::{ExternalPluginProcessEvent, ExternalPluginWorkerCommand};
use crate::external_plugin_sandbox::{PluginSandboxLaunchBackend, PluginSandboxPolicy};
use crate::host::DawHost;
use crate::plugin::{Plugin, ProcessContext};
use std::time::Duration;

use std::path::Path;

use crate::external_plugin::PluginFormat;

fn descriptor() -> PluginDescriptor {
    PluginDescriptor {
        id: "test.external".into(),
        name: "External Test".into(),
        vendor: "Test".into(),
        version: "0.1".into(),
        format: PluginFormat::Clap,
        path: "/tmp/fake.clap".into(),
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: Vec::new(),
        scan_status: crate::external_plugin::PluginScanStatus::Discovered,
    }
}

#[test]
fn isolated_external_plugin_times_out_to_passthrough_without_worker() {
    let mut plugin = IsolatedExternalPlugin::new(
        descriptor(),
        48_000,
        IsolatedExternalPluginConfig {
            deadline: Duration::ZERO,
            start_worker: false,
            ..Default::default()
        },
    )
    .unwrap();

    let input = vec![0.25, -0.5, 1.0, -1.0];
    let mut output = vec![0.0; input.len()];
    let frames = plugin
        .process(&input, &mut output, &ProcessContext::new(48_000, 2))
        .unwrap();

    assert_eq!(frames, 2);
    assert_eq!(output, input);
}

#[test]
fn isolated_external_plugin_records_launch_error() {
    let plugin = IsolatedExternalPlugin::new(
        descriptor(),
        48_000,
        IsolatedExternalPluginConfig {
            worker_command: ExternalPluginWorkerCommand::new(
                "/definitely/not/a/real/sotf/external/plugin/worker",
            ),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(plugin.launch_error().is_some());
    assert_eq!(plugin.worker_launch_failure_count(), 1);
}

#[test]
fn isolated_external_plugin_exposes_worker_poll_state() {
    let mut plugin = IsolatedExternalPlugin::new(
        descriptor(),
        48_000,
        IsolatedExternalPluginConfig {
            start_worker: false,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(matches!(
        plugin.poll_worker().unwrap(),
        Some(ExternalPluginProcessEvent::NotRunning)
    ));
    assert_eq!(plugin.worker_start_count(), 0);
    assert_eq!(plugin.worker_exit_count(), 0);
}

#[test]
fn graph_host_reports_external_worker_with_stable_plugin_instance_id() {
    let plugin = IsolatedExternalPlugin::new(
        descriptor(),
        48_000,
        IsolatedExternalPluginConfig {
            start_worker: false,
            plugin_instance_id: Some(91),
            ..Default::default()
        },
    )
    .unwrap();
    let mut host = DawHost::new(2, 48_000);
    let node_id = host
        .add_node("external-graph-node".to_string(), Box::new(plugin))
        .unwrap();
    host.build().unwrap();

    let reports = host.poll_isolated_external_plugin_workers();

    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].node_id, node_id);
    assert_eq!(reports[0].plugin_instance_id, Some(91));
    assert!(matches!(
        reports[0].event,
        Some(ExternalPluginProcessEvent::NotRunning)
    ));
}

#[test]
fn isolated_external_plugin_reads_worker_reported_latency() {
    let plugin = IsolatedExternalPlugin::new(
        descriptor(),
        48_000,
        IsolatedExternalPluginConfig {
            start_worker: false,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(plugin.worker_reported_latency_samples(), None);
    assert_eq!(plugin.latency_samples(), 0);
    let worker_shared =
        SecurePluginSharedMemory::open_existing(plugin.proxy.shared_path()).unwrap();
    worker_shared.publish_worker_latency_samples(320);
    assert_eq!(plugin.worker_reported_latency_samples(), Some(320));
    assert_eq!(plugin.latency_samples(), 320);
}

#[test]
fn worker_latency_handshake_precedes_daw_host_latency_cache() {
    let plugin = IsolatedExternalPlugin::new(
        descriptor(),
        48_000,
        IsolatedExternalPluginConfig {
            start_worker: false,
            ..Default::default()
        },
    )
    .unwrap();
    let shared_path = plugin.proxy.shared_path().to_path_buf();
    let publisher = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(10));
        let worker_shared = SecurePluginSharedMemory::open_existing(shared_path).unwrap();
        worker_shared.publish_worker_latency_samples(320);
    });

    assert_eq!(
        plugin
            .wait_for_worker_latency_metadata(Duration::from_secs(1))
            .unwrap(),
        320
    );
    publisher.join().unwrap();

    let mut host = DawHost::new(2, 48_000);
    host.add_plugin(Box::new(plugin)).unwrap();
    host.build().unwrap();
    assert_eq!(host.total_latency_samples(), 320);
}

#[test]
fn worker_latency_handshake_timeout_is_actionable() {
    let plugin = IsolatedExternalPlugin::new(
        descriptor(),
        48_000,
        IsolatedExternalPluginConfig {
            start_worker: false,
            ..Default::default()
        },
    )
    .unwrap();

    let error = plugin
        .wait_for_worker_latency_metadata(Duration::ZERO)
        .unwrap_err();
    assert!(error.contains("External Test"), "{error}");
    assert!(error.contains("latency metadata"), "{error}");
    assert!(error.contains("within 0 ms"), "{error}");
}

#[test]
fn isolated_external_plugin_exposes_block_failure_counters() {
    let mut plugin = IsolatedExternalPlugin::new(
        descriptor(),
        48_000,
        IsolatedExternalPluginConfig {
            deadline: Duration::ZERO,
            start_worker: false,
            ..Default::default()
        },
    )
    .unwrap();

    let input = vec![0.25, -0.5, 1.0, -1.0];
    let mut output = vec![0.0; input.len()];
    plugin
        .process(&input, &mut output, &ProcessContext::new(48_000, 2))
        .unwrap();

    assert_eq!(plugin.block_timeout_count(), 1);
    assert_eq!(plugin.block_worker_failure_count(), 0);
    assert_eq!(plugin.block_wrong_sequence_count(), 0);
}

#[test]
fn isolated_external_plugin_quarantines_after_repeated_block_failures() {
    let mut plugin = IsolatedExternalPlugin::new(
        descriptor(),
        48_000,
        IsolatedExternalPluginConfig {
            deadline: Duration::ZERO,
            start_worker: false,
            max_consecutive_block_failures: 2,
            ..Default::default()
        },
    )
    .unwrap();

    let input = vec![0.25, -0.5, 1.0, -1.0];
    let mut output = vec![0.0; input.len()];
    plugin
        .process(&input, &mut output, &ProcessContext::new(48_000, 2))
        .unwrap();
    plugin
        .process(&input, &mut output, &ProcessContext::new(48_000, 2))
        .unwrap();

    assert_eq!(
        plugin.launch_error(),
        Some(
            "isolated external plugin 'External Test' worker quarantined after 2 consecutive block failures"
        )
    );
    assert!(plugin.ensure_worker_running_event().is_err());
    output.fill(0.0);
    plugin
        .process(&input, &mut output, &ProcessContext::new(48_000, 2))
        .unwrap();
    assert_eq!(output, input);
}

#[test]
fn isolated_external_plugin_placeholder_state_round_trips() {
    let plugin_file = tempfile::Builder::new().suffix(".clap").tempfile().unwrap();
    let mut descriptor = descriptor();
    descriptor.path = plugin_file.path().to_path_buf();
    let plugin = IsolatedExternalPlugin::new(
        descriptor,
        48_000,
        IsolatedExternalPluginConfig {
            start_worker: false,
            ..Default::default()
        },
    )
    .unwrap();
    let mut state = plugin.placeholder_state();
    state.opaque_state = vec![9, 8, 7];

    let json = serde_json::to_string(&state).unwrap();
    let decoded: ExternalPluginState = serde_json::from_str(&json).unwrap();
    let restored = IsolatedExternalPlugin::from_placeholder_state(
        &decoded,
        48_000,
        IsolatedExternalPluginConfig {
            start_worker: false,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(decoded.sandbox_mode, ExternalPluginSandboxMode::Isolated);
    assert_eq!(decoded.opaque_state, vec![9, 8, 7]);
    assert_eq!(restored.descriptor(), plugin.descriptor());

    let mut incompatible = decoded;
    incompatible.sandbox_mode = ExternalPluginSandboxMode::InProcess;
    let error = IsolatedExternalPlugin::from_placeholder_state(
        &incompatible,
        48_000,
        IsolatedExternalPluginConfig {
            start_worker: false,
            ..Default::default()
        },
    )
    .err()
    .expect("in-process state must not restore in isolated host");
    assert!(error.contains("cannot restore isolated plugin"));
}

#[test]
fn isolated_external_plugin_persists_initial_state_for_worker_and_removes_sidecar_on_drop() {
    let plugin_file = tempfile::Builder::new().suffix(".clap").tempfile().unwrap();
    let mut descriptor = descriptor();
    descriptor.path = plugin_file.path().to_path_buf();
    let state = ExternalPluginState::new(
        descriptor,
        ExternalPluginSandboxMode::Isolated,
        vec![9, 8, 7, 6],
    );
    let plugin = IsolatedExternalPlugin::new(
        state.descriptor.clone(),
        48_000,
        IsolatedExternalPluginConfig {
            start_worker: false,
            initial_state: Some(state.clone()),
            ..Default::default()
        },
    )
    .unwrap();

    let state_path = plugin
        .state_file_path
        .clone()
        .expect("initial state must create a worker sidecar");
    let on_disk: ExternalPluginState =
        serde_json::from_slice(&std::fs::read(&state_path).unwrap()).unwrap();
    assert_eq!(on_disk, state);
    assert_eq!(plugin.placeholder_state(), state);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&state_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    drop(plugin);
    assert!(!state_path.exists());
}

#[test]
fn isolated_external_plugin_reports_same_hosting_plan_as_descriptor() {
    let plugin = IsolatedExternalPlugin::new(
        descriptor(),
        48_000,
        IsolatedExternalPluginConfig {
            start_worker: false,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(
        plugin.hosting_plan(),
        plan_external_plugin_hosting(plugin.descriptor())
    );
}

#[test]
fn isolated_external_plugin_uses_selected_capability_sandbox_backend() {
    let config = IsolatedExternalPluginConfig {
        capability_sandbox_policy: Some(PluginSandboxPolicy::strict_with_preset_dir(
            "/tmp/sotf-presets",
        )),
        sandbox_launch_backend: PluginSandboxLaunchBackend::MacosAppSandboxHelper,
        sandbox_launcher_command: Some(ExternalPluginWorkerCommand::new(
            "/tmp/sotf-sandbox-helper",
        )),
        start_worker: false,
        ..Default::default()
    };

    let plugin = IsolatedExternalPlugin::new(descriptor(), 48_000, config).unwrap();

    assert_eq!(plugin.worker_start_count(), 0);
}

#[test]
fn isolated_external_plugin_rejects_helper_backend_without_launcher() {
    let config = IsolatedExternalPluginConfig {
        capability_sandbox_policy: Some(PluginSandboxPolicy::strict_with_preset_dir(
            "/tmp/sotf-presets",
        )),
        sandbox_launch_backend: PluginSandboxLaunchBackend::MacosAppSandboxHelper,
        start_worker: false,
        ..Default::default()
    };

    let err = match IsolatedExternalPlugin::new(descriptor(), 48_000, config) {
        Ok(_) => panic!("expected helper backend to require launcher command"),
        Err(err) => err,
    };

    assert!(err.contains("requires a host-owned sandbox launcher command"));
}

#[test]
fn sandbox_launcher_command_receives_worker_metadata() {
    let config = IsolatedExternalPluginConfig {
        worker_command: ExternalPluginWorkerCommand::new("/tmp/sotf-worker")
            .arg("--idle-sleep-micros")
            .arg("50")
            .env("SOTF_WORKER_TEST", "1"),
        sandbox_launch_backend: PluginSandboxLaunchBackend::WindowsAppContainerWorker,
        sandbox_launcher_command: Some(ExternalPluginWorkerCommand::new(
            "/tmp/sotf-appcontainer-launcher",
        )),
        start_worker: false,
        ..Default::default()
    };

    let command =
        build_worker_launch_command(&config, "{\"id\":\"test\"}".to_string(), None, Vec::new())
            .unwrap();

    assert_eq!(
        command.program(),
        Path::new("/tmp/sotf-appcontainer-launcher")
    );
    assert_eq!(
        command.command_args(),
        &[
            "--sandbox-worker-binary".to_string(),
            "/tmp/sotf-worker".to_string(),
            "--sandbox-worker-arg".to_string(),
            "--idle-sleep-micros".to_string(),
            "--sandbox-worker-arg".to_string(),
            "50".to_string(),
            "--sandbox-worker-env".to_string(),
            "SOTF_WORKER_TEST=1".to_string(),
            "--descriptor-json".to_string(),
            "{\"id\":\"test\"}".to_string(),
        ]
    );
}

#[test]
fn sandbox_launcher_command_receives_external_state_sidecar() {
    let config = IsolatedExternalPluginConfig {
        worker_command: ExternalPluginWorkerCommand::new("/tmp/sotf-worker"),
        sandbox_launch_backend: PluginSandboxLaunchBackend::WindowsAppContainerWorker,
        sandbox_launcher_command: Some(ExternalPluginWorkerCommand::new(
            "/tmp/sotf-appcontainer-launcher",
        )),
        start_worker: false,
        ..Default::default()
    };
    let state_path = Path::new("/tmp/sotf-external-state.json");

    let command = build_worker_launch_command(
        &config,
        "{\"id\":\"test\"}".to_string(),
        Some(state_path),
        Vec::new(),
    )
    .unwrap();

    assert_eq!(
        command.command_args(),
        &[
            "--sandbox-worker-binary".to_string(),
            "/tmp/sotf-worker".to_string(),
            "--descriptor-json".to_string(),
            "{\"id\":\"test\"}".to_string(),
            "--external-state-file".to_string(),
            state_path.display().to_string(),
            "--sandbox-read-path".to_string(),
            state_path.display().to_string(),
        ]
    );
}

#[test]
fn isolated_external_plugin_rejects_capability_policy_for_process_only_backend() {
    let config = IsolatedExternalPluginConfig {
        capability_sandbox_policy: Some(PluginSandboxPolicy::strict_with_preset_dir(
            "/tmp/sotf-presets",
        )),
        sandbox_launch_backend: PluginSandboxLaunchBackend::ProcessIsolationOnly {
            platform: "test-process-only",
        },
        start_worker: false,
        ..Default::default()
    };

    let err = match IsolatedExternalPlugin::new(descriptor(), 48_000, config) {
        Ok(_) => panic!("expected process-only backend to reject strict capability policy"),
        Err(err) => err,
    };

    assert!(err.contains("cannot satisfy required policy"));
}
