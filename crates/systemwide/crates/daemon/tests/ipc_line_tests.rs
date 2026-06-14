//! End-to-end IPC round-trip tests for the sotf-daemon binary.
//!
//! These tests spawn the daemon as a subprocess with the null HAL driver and a
//! private Unix socket, then send JSON commands and verify JSON responses.
//! They do not require a real audio capture driver.

#![cfg(unix)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

struct DaemonFixture {
    child: Child,
    socket_path: PathBuf,
    _temp_dir: tempfile::TempDir,
}

impl DaemonFixture {
    // The `Child` handle is stored in the fixture and reaped by `shutdown()`;
    // clippy cannot see the cross-method lifecycle.
    #[allow(clippy::zombie_processes)]
    fn start() -> Self {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let socket_path = temp_dir.path().join("daemon.sock");

        let mut child = Command::new(env!("CARGO_BIN_EXE_sotf-daemon"))
            .env("SOTF_DAEMON_SOCKET_PATH", &socket_path)
            .env("SOTF_SYSTEMWIDE_DRIVER", "null")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sotf-daemon");

        // Wait for the daemon to bind its socket.
        for _ in 0..100 {
            if socket_path.exists() {
                return Self {
                    child,
                    socket_path,
                    _temp_dir: temp_dir,
                };
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        let _ = child.kill();
        let _ = child.wait();
        panic!("daemon did not create socket in time");
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
fn daemon_set_volume_roundtrip_over_unix_socket() {
    let daemon = DaemonFixture::start();

    let response = daemon.send(r#"{"command":"set_volume","volume":0.37}"#);

    assert_eq!(response["success"], true);

    daemon.shutdown();
}

#[test]
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
