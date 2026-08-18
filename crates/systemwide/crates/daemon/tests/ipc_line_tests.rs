//! End-to-end IPC round-trip tests for the sotf-daemon binary.
//!
//! These tests spawn the daemon as a subprocess with the null HAL driver and a
//! private Unix socket, then send JSON commands and verify JSON responses.
//! They do not require a real audio capture driver.

#![cfg(unix)]

use serial_test::serial;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

static DAEMON_PROCESS_LOCK: Mutex<()> = Mutex::new(());

struct DaemonFixture {
    child: Child,
    socket_path: PathBuf,
    _temp_dir: tempfile::TempDir,
    _process_guard: MutexGuard<'static, ()>,
}

impl DaemonFixture {
    // The `Child` handle is stored in the fixture and reaped by `shutdown()`;
    // clippy cannot see the cross-method lifecycle.
    #[allow(clippy::zombie_processes)]
    fn start() -> Self {
        Self::start_with_driver("null")
    }

    #[allow(clippy::zombie_processes)]
    fn start_with_driver(driver: &str) -> Self {
        let process_guard = DAEMON_PROCESS_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let socket_path = temp_dir.path().join("daemon.sock");

        let mut child = Command::new(env!("CARGO_BIN_EXE_sotf-daemon"))
            .env("SOTF_DAEMON_SOCKET_PATH", &socket_path)
            .env("SOTF_SYSTEMWIDE_RUNTIME_DIR", temp_dir.path())
            .env("SOTF_SYSTEMWIDE_DRIVER", driver)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn sotf-daemon");

        // Wait for the daemon to bind its socket.
        for _ in 0..100 {
            if socket_path.exists() {
                return Self {
                    child,
                    socket_path,
                    _temp_dir: temp_dir,
                    _process_guard: process_guard,
                };
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        let _ = child.kill();
        let status = child.wait().ok();
        let mut stderr = String::new();
        if let Some(mut pipe) = child.stderr.take() {
            let _ = pipe.read_to_string(&mut stderr);
        }
        panic!("daemon did not create socket in time (status={status:?}): {stderr}");
    }

    fn send(&self, command_json: &str) -> serde_json::Value {
        let mut stream = UnixStream::connect(&self.socket_path).expect("connect to daemon socket");
        writeln!(stream, "{}", command_json).expect("write command");

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).expect("read response line");

        serde_json::from_str(&line).expect("response is valid JSON")
    }

    fn shutdown(mut self) {
        let _ = self.send(r#"{"command":"shutdown"}"#);

        for _ in 0..100 {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
#[serial]
fn daemon_status_roundtrip_over_unix_socket() {
    let daemon = DaemonFixture::start();

    let response = daemon.send(r#"{"command":"status"}"#);

    assert_eq!(response["success"], true);
    assert!(response["data"]["state"].is_string());
    assert!(response["data"]["volume"].is_number());
    assert!(response["data"]["input_channels"].is_number());
    assert!(response["data"]["output_channels"].is_number());

    daemon.shutdown();
}

#[test]
#[serial]
fn second_daemon_cannot_take_ownership_of_a_live_runtime() {
    let daemon = DaemonFixture::start();

    let mut second = Command::new(env!("CARGO_BIN_EXE_sotf-daemon"))
        .env("SOTF_DAEMON_SOCKET_PATH", &daemon.socket_path)
        .env("SOTF_SYSTEMWIDE_RUNTIME_DIR", daemon._temp_dir.path())
        .env("SOTF_SYSTEMWIDE_DRIVER", "null")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn second sotf-daemon");

    let mut second_status = None;
    for _ in 0..100 {
        if let Some(status) = second.try_wait().expect("poll second daemon") {
            second_status = Some(status);
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    if second_status.is_none() {
        let _ = second.kill();
        let _ = second.wait();
        panic!("second daemon must exit while the first owns the runtime lock");
    }

    let status = second_status.expect("second daemon exit status");
    assert!(!status.success(), "second daemon unexpectedly became owner");
    assert!(
        daemon.socket_path.exists(),
        "second daemon must not remove the live daemon socket"
    );
    let first_status = daemon.send(r#"{"command":"status"}"#);
    assert_eq!(first_status["success"], true);

    daemon.shutdown();
}

#[test]
#[serial]
fn daemon_get_metering_roundtrip_over_unix_socket() {
    let daemon = DaemonFixture::start();

    let response = daemon.send(r#"{"command":"get_metering"}"#);

    assert_eq!(response["success"], true);
    assert!(response["data"]["input"].is_object());
    assert!(response["data"]["output"].is_object());
    assert!(response["data"]["sources"]["input"].is_object());
    assert!(response["data"]["sources"]["output"].is_object());

    daemon.shutdown();
}

#[test]
#[serial]
fn daemon_set_volume_roundtrip_over_unix_socket() {
    let daemon = DaemonFixture::start();

    let response = daemon.send(r#"{"command":"set_volume","volume":0.37}"#);

    assert_eq!(response["success"], true);

    daemon.shutdown();
}

#[test]
#[serial]
fn daemon_shutdown_over_unix_socket_stops_process() {
    let mut daemon = DaemonFixture::start();

    // Send shutdown; the daemon may exit before writing a response, so we
    // only verify that the process terminates afterwards.
    let mut stream = UnixStream::connect(&daemon.socket_path).expect("connect to daemon socket");
    writeln!(stream, r#"{{"command":"shutdown"}}"#).expect("write shutdown");
    drop(stream);

    for _ in 0..100 {
        if daemon.child.try_wait().ok().flatten().is_some() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    panic!("daemon did not exit after shutdown");
}

#[test]
#[serial]
fn daemon_sigterm_clears_runtime_socket_and_reaps_process() {
    let mut daemon = DaemonFixture::start();
    let pid = daemon.child.id().to_string();

    let status = Command::new("/bin/kill")
        .args(["-TERM", &pid])
        .status()
        .expect("send SIGTERM to daemon");
    assert!(status.success(), "kill should deliver SIGTERM: {status}");

    for _ in 0..100 {
        if daemon.child.try_wait().ok().flatten().is_some() {
            assert!(
                !daemon.socket_path.exists(),
                "SIGTERM shutdown must remove the daemon-owned socket"
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let _ = daemon.child.kill();
    let _ = daemon.child.wait();
    panic!("daemon did not exit after SIGTERM");
}

#[test]
#[serial]
fn systemwide_lab_scenario_matrix_over_unix_socket() {
    let daemon = DaemonFixture::start_with_driver("lab");

    let initial = daemon.send(r#"{"command":"get_snapshot"}"#);
    assert_eq!(initial["success"], true);
    assert_eq!(
        initial["data"]["observed"]["driver"]["driver_name"],
        "Systemwide Lab Driver"
    );
    assert_eq!(
        initial["data"]["observed"]["driver"]["platform_supported"],
        true
    );
    assert_eq!(
        initial["data"]["observed"]["driver"]["driver_installed"],
        true
    );
    assert_eq!(
        initial["data"]["observed"]["driver"]["capture_active"],
        true
    );
    assert!(initial["data"]["desired"]["input_channels"].is_number());
    assert!(initial["data"]["desired"]["output_channels"].is_number());
    assert!(initial["data"]["diagnostics"]["faults"].is_array());

    let initial_driver_config = daemon.send(r#"{"command":"get_driver_config"}"#);
    assert_eq!(initial_driver_config["success"], true);
    assert_eq!(initial_driver_config["data"]["sample_rate"], 48_000);
    assert_eq!(initial_driver_config["data"]["buffer_frames"], 512);

    // Live timing changes are intentionally rejected while the engine is
    // active; stop first, then verify the idle configuration path.
    let stopped = daemon.send(r#"{"command":"stop"}"#);
    assert_eq!(stopped["success"], true, "{stopped}");

    let sample_rate = daemon.send(r#"{"command":"set_sample_rate","rate":96000}"#);
    assert_eq!(sample_rate["success"], true, "{sample_rate}");
    let buffer_frames = daemon.send(r#"{"command":"set_buffer_frames","frames":256}"#);
    assert_eq!(buffer_frames["success"], true, "{buffer_frames}");

    let reconfigured_driver = daemon.send(r#"{"command":"get_driver_config"}"#);
    assert_eq!(reconfigured_driver["data"]["sample_rate"], 96_000);
    assert_eq!(reconfigured_driver["data"]["actual_sample_rate"], 96_000);
    assert_eq!(reconfigured_driver["data"]["buffer_frames"], 256);
    assert_eq!(reconfigured_driver["data"]["actual_buffer_frames"], 256);
    assert_eq!(reconfigured_driver["data"]["active"], true);

    let invalid_sample_rate = daemon.send(r#"{"command":"set_sample_rate","rate":12345}"#);
    assert_eq!(invalid_sample_rate["success"], false);
    let invalid_buffer = daemon.send(r#"{"command":"set_buffer_frames","frames":32}"#);
    assert_eq!(invalid_buffer["success"], false);
    let config_after_rejection = daemon.send(r#"{"command":"get_driver_config"}"#);
    assert_eq!(
        config_after_rejection["data"], reconfigured_driver["data"],
        "invalid transport requests must preserve the active lab-driver config"
    );

    let encryption_enabled = daemon.send(r#"{"command":"set_encryption","enabled":true}"#);
    if cfg!(all(target_os = "macos", feature = "hal")) {
        assert_eq!(encryption_enabled["success"], true, "{encryption_enabled}");
        assert_eq!(encryption_enabled["data"]["enabled"], true);
        let first_fingerprint = encryption_enabled["data"]["fingerprint"]
            .as_str()
            .expect("enabled encryption publishes a fingerprint")
            .to_string();

        let rotated = daemon.send(r#"{"command":"rotate_encryption_key"}"#);
        assert_eq!(rotated["success"], true, "{rotated}");
        let rotated_fingerprint = rotated["data"]["fingerprint"]
            .as_str()
            .expect("rotation publishes the replacement fingerprint");
        assert_ne!(rotated_fingerprint, first_fingerprint);

        let encryption_status = daemon.send(r#"{"command":"encryption_status"}"#);
        assert_eq!(encryption_status["success"], true);
        assert_eq!(encryption_status["data"]["enabled"], true);
        assert_eq!(
            encryption_status["data"]["fingerprint"],
            rotated["data"]["fingerprint"]
        );
        let hal_key_path = PathBuf::from(
            encryption_status["data"]["key_path"]
                .as_str()
                .expect("encryption status publishes the HAL key path"),
        );
        assert!(hal_key_path.starts_with(daemon._temp_dir.path()));
        assert!(hal_key_path.exists());
        assert!(daemon._temp_dir.path().join("daemon-session.key").exists());
    } else {
        assert_eq!(encryption_enabled["success"], false);
        assert!(
            encryption_enabled["error"]
                .as_str()
                .is_some_and(|error| error.contains("no session cipher"))
        );
        let rotation = daemon.send(r#"{"command":"rotate_encryption_key"}"#);
        assert_eq!(rotation["success"], false);
        let encryption_status = daemon.send(r#"{"command":"encryption_status"}"#);
        assert_eq!(encryption_status["success"], true);
        assert_eq!(encryption_status["data"]["enabled"], false);
    }

    let reconfigured = daemon
        .send(r#"{"command":"set_pipeline_channels","input_channels":10,"output_channels":2}"#);
    assert_eq!(reconfigured["success"], true);

    let after_reconfigure = daemon.send(r#"{"command":"get_snapshot"}"#);
    assert_eq!(after_reconfigure["data"]["desired"]["input_channels"], 10);
    assert_eq!(after_reconfigure["data"]["desired"]["output_channels"], 2);

    let loaded = daemon.send(
        r#"{"command":"load_plugin_artifact","artifact":{"plugins":[{"plugin_type":"gain","parameters":{"gain_db":-3.0}}]}}"#,
    );
    assert_eq!(loaded["success"], true, "{loaded}");

    let before_rejected_artifact = daemon.send(r#"{"command":"get_snapshot"}"#);
    let rejected = daemon.send(
        r#"{"command":"load_plugin_artifact","artifact":{"global_plugins":[{"plugin_type":"eq","parameters":{}}],"channels":{"L":{"plugins":[{"plugin_type":"gain","parameters":{}}]}}}}"#,
    );
    assert_eq!(rejected["success"], false);
    assert!(
        rejected["error"]
            .as_str()
            .is_some_and(|error| error.contains("Unsupported graph plugin artifact")),
        "{rejected}"
    );

    let after_rejected_artifact = daemon.send(r#"{"command":"get_snapshot"}"#);
    assert_eq!(
        after_rejected_artifact["data"]["desired"], before_rejected_artifact["data"]["desired"],
        "a rejected artifact must preserve desired pipeline state"
    );
    assert_eq!(
        after_rejected_artifact["data"]["applied"], before_rejected_artifact["data"]["applied"],
        "a rejected artifact must preserve the active pipeline"
    );

    let restored = daemon
        .send(r#"{"command":"set_pipeline_channels","input_channels":2,"output_channels":2}"#);
    assert_eq!(restored["success"], true);

    let final_snapshot = daemon.send(r#"{"command":"get_snapshot"}"#);
    assert_eq!(final_snapshot["data"]["desired"]["input_channels"], 2);
    assert_eq!(final_snapshot["data"]["desired"]["output_channels"], 2);
    assert_eq!(
        final_snapshot["data"]["observed"]["driver"]["driver_name"],
        "Systemwide Lab Driver"
    );
    assert_eq!(
        final_snapshot["data"]["observed"]["driver"]["sample_rate"],
        96_000
    );
    assert_eq!(
        final_snapshot["data"]["observed"]["driver"]["buffer_frames"],
        256
    );

    let diagnostic_dump = daemon.send(r#"{"command":"dump_state"}"#);
    assert_eq!(diagnostic_dump["success"], true);
    assert!(diagnostic_dump["data"]["snapshot"]["diagnostics"]["health"].is_string());
    assert!(diagnostic_dump["data"]["snapshot"]["diagnostics"]["faults"].is_array());
    assert!(diagnostic_dump["data"]["plugins"].is_array());

    daemon.shutdown();
}

#[test]
#[serial]
fn systemwide_lab_restarts_with_a_fresh_coherent_snapshot() {
    let first = DaemonFixture::start_with_driver("lab");
    let changed =
        first.send(r#"{"command":"set_pipeline_channels","input_channels":6,"output_channels":2}"#);
    assert_eq!(changed["success"], true);
    first.shutdown();

    let restarted = DaemonFixture::start_with_driver("lab");
    let snapshot = restarted.send(r#"{"command":"get_snapshot"}"#);
    assert_eq!(snapshot["success"], true);
    assert_eq!(
        snapshot["data"]["observed"]["driver"]["driver_name"],
        "Systemwide Lab Driver"
    );
    assert!(snapshot["data"]["desired"]["input_channels"].is_number());
    assert!(snapshot["data"]["desired"]["output_channels"].is_number());
    assert!(snapshot["data"]["diagnostics"]["health"].is_string());
    assert!(snapshot["data"]["diagnostics"]["faults"].is_array());
    restarted.shutdown();
}
