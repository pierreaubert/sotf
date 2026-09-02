use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sotf_dev_api::{Capabilities, DevReply, NamedCapability, Snapshot};

use super::super::{
    model::{
        Action, ActionPayload, AdapterKind, CoverageDelta, EndpointSpec, FUZZ_SCHEMA_VERSION,
        Observation, ProcessObservation, TargetId, TargetSpec, opt_in,
    },
    supervisor::{FuzzTarget, LaunchContext, TargetError},
};

const MAX_IPC_LINE_BYTES: usize = 256 * 1024;
const MAX_IPC_RESPONSE_BYTES: u64 = 4 * 1024 * 1024;
const IPC_TIMEOUT: Duration = Duration::from_secs(2);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
/// Conservative AF_UNIX `sun_path` capacity (macOS allows 104 bytes including
/// the terminator); deeper paths must fall back to a short runtime dir.
const MAX_SOCKET_PATH_BYTES: usize = 100;

fn socket_path_too_long(path: &Path) -> bool {
    path.as_os_str().len() >= MAX_SOCKET_PATH_BYTES
}

#[derive(Debug, Clone)]
pub struct SystemwideTargetConfig {
    pub executable: PathBuf,
}

impl SystemwideTargetConfig {
    pub fn new(executable: PathBuf) -> Self {
        Self { executable }
    }
}

pub struct SystemwideTarget {
    config: SystemwideTargetConfig,
    child: Option<Child>,
    socket_path: Option<PathBuf>,
    owned_socket_dir: Option<PathBuf>,
    stdout_path: Option<PathBuf>,
    stderr_path: Option<PathBuf>,
    stdout_offset: u64,
    stderr_offset: u64,
    semantic_revision: u64,
    state_hash: Option<String>,
}

impl SystemwideTarget {
    pub fn new(config: SystemwideTargetConfig) -> Self {
        Self {
            config,
            child: None,
            socket_path: None,
            owned_socket_dir: None,
            stdout_path: None,
            stderr_path: None,
            stdout_offset: 0,
            stderr_offset: 0,
            semantic_revision: 0,
            state_hash: None,
        }
    }

    fn cleanup_owned_socket_dir(&mut self) {
        if let Some(dir) = self.owned_socket_dir.take() {
            let _ = fs::remove_dir_all(dir);
        }
    }

    fn socket_path(&self) -> Result<&Path, TargetError> {
        self.socket_path
            .as_deref()
            .ok_or_else(|| TargetError::Protocol("systemwide target is not launched".into()))
    }

    fn send_bytes(&self, bytes: &[u8]) -> Result<Value, TargetError> {
        if bytes.len() > MAX_IPC_LINE_BYTES || bytes.contains(&b'\n') {
            return Err(TargetError::Protocol(
                "systemwide IPC input exceeds the bounded single-line policy".into(),
            ));
        }
        let mut stream = UnixStream::connect(self.socket_path()?)?;
        stream.set_read_timeout(Some(IPC_TIMEOUT))?;
        stream.set_write_timeout(Some(IPC_TIMEOUT))?;
        stream.write_all(bytes).map_err(super::map_io_timeout)?;
        stream.write_all(b"\n").map_err(super::map_io_timeout)?;
        stream.flush().map_err(super::map_io_timeout)?;

        let mut response = Vec::new();
        BufReader::new(stream)
            .take(MAX_IPC_RESPONSE_BYTES + 1)
            .read_until(b'\n', &mut response)
            .map_err(super::map_io_timeout)?;
        if response.len() as u64 > MAX_IPC_RESPONSE_BYTES {
            return Err(TargetError::Protocol(
                "systemwide IPC response exceeded 4 MiB".into(),
            ));
        }
        if response.is_empty() {
            return Err(TargetError::Protocol(
                "systemwide IPC closed without a response".into(),
            ));
        }
        Ok(serde_json::from_slice(&response)?)
    }

    fn send_json(&self, command: &Value) -> Result<Value, TargetError> {
        self.send_bytes(&serde_json::to_vec(command)?)
    }

    fn daemon_reply(response: Value) -> DevReply {
        if response
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            DevReply::success(response.get("data").cloned().unwrap_or(Value::Null))
        } else {
            DevReply::failure(
                "daemon_rejected",
                response
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("systemwide daemon rejected the command"),
            )
        }
    }

    fn process_observation(&mut self) -> ProcessObservation {
        let Some(child) = self.child.as_mut() else {
            return ProcessObservation {
                pid: None,
                alive: false,
                exit_code: None,
                signal_or_exception: None,
            };
        };
        match child.try_wait() {
            Ok(None) => ProcessObservation {
                pid: Some(child.id()),
                alive: true,
                exit_code: None,
                signal_or_exception: None,
            },
            Ok(Some(status)) => process_from_status(child.id(), &status),
            Err(error) => ProcessObservation {
                pid: Some(child.id()),
                alive: false,
                exit_code: None,
                signal_or_exception: Some(error.to_string()),
            },
        }
    }

    fn collect_logs(&mut self) -> Vec<String> {
        let mut logs = Vec::new();
        if let Some(path) = self.stdout_path.as_ref() {
            logs.extend(read_new_lines(path, &mut self.stdout_offset));
        }
        if let Some(path) = self.stderr_path.as_ref() {
            logs.extend(read_new_lines(path, &mut self.stderr_offset));
        }
        logs
    }

    fn wait_for_socket(&mut self) -> Result<(), TargetError> {
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        while Instant::now() < deadline {
            if fs::metadata(self.socket_path()?)
                .is_ok_and(|metadata| metadata.file_type().is_socket())
            {
                return Ok(());
            }
            if let Some(status) = self
                .child
                .as_mut()
                .and_then(|child| child.try_wait().ok())
                .flatten()
            {
                return Err(TargetError::ProcessExited(format!(
                    "sotf-daemon exited during startup with {status}"
                )));
            }
            thread::sleep(Duration::from_millis(25));
        }
        Err(TargetError::Timeout(
            "sotf-daemon did not create its private socket within 5 seconds".into(),
        ))
    }
}

impl FuzzTarget for SystemwideTarget {
    fn target_id(&self) -> TargetId {
        TargetId::SystemwideDaemon
    }

    fn launch(&mut self, context: &LaunchContext<'_>) -> Result<TargetSpec, TargetError> {
        if !self.config.executable.is_file() {
            return Err(TargetError::Unsupported(
                super::super::model::StructuredSkip {
                    target_id: TargetId::SystemwideDaemon,
                    reason_code: "feature_missing".into(),
                    reason: format!(
                        "{} is not built; run cargo build -p sotf-daemon --bin sotf-daemon",
                        self.config.executable.display()
                    ),
                },
            ));
        }
        let executable = self.config.executable.canonicalize()?;
        if context.opt_ins.contains(opt_in::HAL_INSTALL)
            || context.opt_ins.contains(opt_in::HARDWARE_AUDIO)
        {
            return Err(TargetError::Protocol(
                "systemwide fuzz target is lab-only; HAL and hardware opt-ins are not accepted"
                    .into(),
            ));
        }

        let runtime_dir = context.run_dir.join("runtime/systemwide");
        fs::create_dir_all(&runtime_dir)?;
        let runtime_dir = runtime_dir.canonicalize()?;
        if !runtime_dir.starts_with(context.run_dir) {
            return Err(TargetError::Protocol(
                "systemwide runtime escaped the fuzzer run directory".into(),
            ));
        }
        // Deep checkouts put the in-run-dir socket beyond the AF_UNIX
        // sun_path limit; fall back to a short private runtime dir, like the
        // daemon's own IPC tests do with tempdir(). Logs/artifacts stay in
        // the run dir; the actual socket location is recorded in the target
        // endpoint below.
        let mut runtime_dir = runtime_dir;
        if socket_path_too_long(&runtime_dir.join("daemon.sock")) {
            let short_id: String = context.run_id.as_str().chars().take(8).collect();
            let dir = std::env::temp_dir().join(format!("sotf-fuzz-{short_id}"));
            if dir.exists() {
                fs::remove_dir_all(&dir)?;
            }
            fs::create_dir_all(&dir)?;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
            self.owned_socket_dir = Some(dir.clone());
            runtime_dir = dir;
        }
        let socket_path = runtime_dir.join("daemon.sock");
        let stdout_path = context.run_dir.join("logs/systemwide.stdout.log");
        let stderr_path = context.run_dir.join("logs/systemwide.stderr.log");
        let stdout = File::create(&stdout_path)?;
        let stderr = File::create(&stderr_path)?;

        let mut command = Command::new(&executable);
        command
            .env_clear()
            .env("SOTF_DAEMON_SOCKET_PATH", &socket_path)
            .env("SOTF_SYSTEMWIDE_RUNTIME_DIR", &runtime_dir)
            .env("SOTF_SYSTEMWIDE_DRIVER", "lab")
            .env("RUST_BACKTRACE", "1")
            .current_dir(context.run_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        self.child = Some(command.spawn()?);
        self.socket_path = Some(socket_path.clone());
        self.stdout_path = Some(stdout_path);
        self.stderr_path = Some(stderr_path);
        if let Err(error) = self.wait_for_socket() {
            self.cleanup_owned_socket_dir();
            return Err(error);
        }

        Ok(TargetSpec {
            schema_version: FUZZ_SCHEMA_VERSION,
            target_id: TargetId::SystemwideDaemon,
            adapter: AdapterKind::Ipc,
            executable: Some(executable.clone()),
            app_identity: Some("sotf-daemon-lab".into()),
            platform: std::env::consts::OS.into(),
            fixture_profile: context.fixture_profile.into(),
            environment_names: vec![
                "RUST_BACKTRACE".into(),
                "SOTF_DAEMON_SOCKET_PATH".into(),
                "SOTF_SYSTEMWIDE_DRIVER".into(),
                "SOTF_SYSTEMWIDE_RUNTIME_DIR".into(),
            ],
            endpoints: vec![EndpointSpec {
                name: "daemon".into(),
                address: socket_path.display().to_string(),
                protocol: "bounded-json-lines".into(),
            }],
            run_id_hash: context.run_id.redacted_hash(),
            capability_fingerprint: String::new(),
            build_id: executable_build_id(&executable)?,
        })
    }

    fn capabilities(&mut self) -> Result<Capabilities, TargetError> {
        let mut capabilities = Capabilities::new("systemwide-daemon", "sotf-daemon");
        capabilities.manifest_version = 1;
        for (name, family) in [
            ("get-snapshot", "ipc-read"),
            ("get-status", "ipc-read"),
            ("malformed-bounded", "ipc-chaos"),
        ] {
            capabilities.actions.push(NamedCapability {
                name: name.into(),
                family: family.into(),
                payload_schema: None,
            });
        }
        capabilities.debug_features.push("audio-driver:lab".into());
        capabilities
            .debug_features
            .push("network:unix-socket-private".into());
        Ok(capabilities)
    }

    fn snapshot(&mut self) -> Result<Snapshot, TargetError> {
        let response = self.send_json(&json!({"command": "get_snapshot"}))?;
        if !response
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(TargetError::Protocol(format!(
                "get_snapshot failed: {}",
                response
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown daemon error")
            )));
        }
        let state = response.get("data").cloned().unwrap_or_else(|| json!({}));
        let hash = sotf_dev_api::canonical_json_hash(&state)?;
        if self.state_hash.as_ref() != Some(&hash) {
            self.semantic_revision = self.semantic_revision.wrapping_add(1);
            self.state_hash = Some(hash);
        }
        Snapshot::new("systemwide-daemon", self.semantic_revision, state).map_err(TargetError::from)
    }

    fn execute(&mut self, action: &Action) -> Result<Observation, TargetError> {
        let response = match &action.payload {
            ActionPayload::Ipc { command } => self.send_json(command)?,
            ActionPayload::Stdin { bytes, eof: _ } => self.send_bytes(bytes)?,
            ActionPayload::Wait { duration_ms } => {
                thread::sleep(Duration::from_millis((*duration_ms).min(action.timeout_ms)));
                json!({"success": true, "data": null})
            }
            payload => {
                return Err(TargetError::Protocol(format!(
                    "unsupported systemwide action payload {payload:?}"
                )));
            }
        };
        let reply = Self::daemon_reply(response);
        let snapshot = self.snapshot().ok();
        let process = self.process_observation();
        let new_logs = self.collect_logs();
        Ok(Observation {
            schema_version: FUZZ_SCHEMA_VERSION,
            sequence: action.sequence,
            reply: Some(reply),
            snapshot,
            process,
            resource: None,
            new_logs,
            crash_files: vec![],
            coverage: CoverageDelta::default(),
            screenshot: None,
            failure_candidate: None,
        })
    }

    fn live(&mut self) -> Result<bool, TargetError> {
        // Exit precedence: a known-dead child is an exit, never a hang probe.
        if !self.process_observation().alive {
            return Err(TargetError::ProcessExited(
                "sotf-daemon process exited".into(),
            ));
        }
        let response = self.send_json(&json!({"command": "status"}))?;
        Ok(response
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    fn capture_hang(&mut self, directory: &Path) -> Result<Vec<PathBuf>, TargetError> {
        fs::create_dir_all(directory)?;
        let mut artifacts = Vec::new();
        #[cfg(target_os = "macos")]
        if let Some(pid) = self.pid() {
            let path = directory.join("sample.txt");
            let output = Command::new("/usr/bin/sample")
                .arg(pid.to_string())
                .arg("1")
                .arg("1")
                .output()?;
            fs::write(&path, output.stdout)?;
            artifacts.push(path);
        }
        Ok(artifacts)
    }

    fn shutdown(&mut self) -> Result<(), TargetError> {
        let Some(mut child) = self.child.take() else {
            self.cleanup_owned_socket_dir();
            return Ok(());
        };
        let _ = self.send_json(&json!({"command": "shutdown"}));
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut exited = false;
        while Instant::now() < deadline {
            if child.try_wait()?.is_some() {
                exited = true;
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        if !exited {
            child.kill()?;
            child.wait()?;
        }
        let stale_socket = self.socket_path.as_ref().is_some_and(|path| path.exists());
        self.cleanup_owned_socket_dir();
        if stale_socket {
            return Err(TargetError::Protocol(
                "sotf-daemon left its private socket after shutdown".into(),
            ));
        }
        Ok(())
    }
}

fn read_new_lines(path: &Path, offset: &mut u64) -> Vec<String> {
    let Ok(mut file) = File::open(path) else {
        return Vec::new();
    };
    if file.seek(SeekFrom::Start(*offset)).is_err() {
        return Vec::new();
    }
    let mut bytes = Vec::new();
    if Read::by_ref(&mut file)
        .take(64 * 1024)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return Vec::new();
    }
    *offset = offset.saturating_add(bytes.len() as u64);
    String::from_utf8_lossy(&bytes)
        .lines()
        .map(str::to_owned)
        .collect()
}

fn executable_build_id(path: &Path) -> Result<String, TargetError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn process_from_status(pid: u32, status: &ExitStatus) -> ProcessObservation {
    ProcessObservation {
        pid: Some(pid),
        alive: false,
        exit_code: status.code(),
        signal_or_exception: status_signal(status),
    }
}

#[cfg(unix)]
fn status_signal(status: &ExitStatus) -> Option<String> {
    use std::os::unix::process::ExitStatusExt;
    status.signal().map(|signal| format!("signal {signal}"))
}

#[cfg(not(unix))]
fn status_signal(_status: &ExitStatus) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use sotf_dev_api::RunId;

    use super::*;
    use crate::fuzz::model::opt_in;

    #[test]
    fn refuses_multiline_or_oversized_ipc_without_launching_a_process() {
        let target = SystemwideTarget::new(SystemwideTargetConfig::new("missing".into()));
        assert!(target.send_bytes(b"{}\n{}").is_err());
        assert!(
            target
                .send_bytes(&vec![b'x'; MAX_IPC_LINE_BYTES + 1])
                .is_err()
        );
    }

    #[test]
    fn detects_socket_paths_beyond_the_sun_path_limit() {
        assert!(socket_path_too_long(Path::new(&format!(
            "{}/daemon.sock",
            "/very-deep".repeat(20)
        ))));
        assert!(!socket_path_too_long(Path::new("/tmp/daemon.sock")));
    }

    #[test]
    fn rejects_hal_and_hardware_opt_ins_using_the_cli_flag_names() {
        // The CLI inserts opt_in::HAL_INSTALL for --allow-hal-install; the
        // lab-only adapter must reject exactly those shared names.
        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().join("sotf-daemon");
        fs::write(&executable, b"#!/bin/sh\nexit 1\n").unwrap();
        let run_id = RunId::parse("0123456789abcdef0123456789abcdef").unwrap();
        for name in [opt_in::HAL_INSTALL, opt_in::HARDWARE_AUDIO] {
            let opt_ins = BTreeSet::from([name.to_owned()]);
            let context = LaunchContext {
                run_id: &run_id,
                run_dir: dir.path(),
                fixture_profile: "none",
                opt_ins: &opt_ins,
            };
            let mut target = SystemwideTarget::new(SystemwideTargetConfig::new(executable.clone()));
            let error = target.launch(&context).unwrap_err();
            assert!(
                matches!(error, TargetError::Protocol(_)),
                "opt-in {name} must be rejected: {error}"
            );
        }
    }
}
