use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sotf_dev_api::{Capabilities, DevReply, Snapshot};

use crate::fuzz::model::{
    Action, ActionPayload, AdapterKind, CoverageDelta, FUZZ_SCHEMA_VERSION, Observation,
    ProcessObservation, TargetId, TargetSpec,
};
use crate::fuzz::supervisor::{FuzzTarget, LaunchContext, TargetError};

const MAX_ARG_COUNT: usize = 64;
const MAX_ARG_BYTES: usize = 4096;
const MAX_ARGV_BYTES: usize = 32 * 1024;
const MAX_LOG_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone)]
pub struct ProcessTargetConfig {
    pub target: TargetId,
    pub executable: PathBuf,
    pub environment: BTreeMap<String, String>,
}

pub struct ProcessTarget {
    config: ProcessTargetConfig,
    run_dir: Option<PathBuf>,
    revision: u64,
    launches: u64,
    last_process: ProcessObservation,
    active_child: Option<Child>,
}

impl ProcessTarget {
    pub fn new(config: ProcessTargetConfig) -> Self {
        Self {
            config,
            run_dir: None,
            revision: 0,
            launches: 0,
            last_process: ProcessObservation {
                pid: None,
                alive: true,
                exit_code: None,
                signal_or_exception: None,
            },
            active_child: None,
        }
    }

    fn run_dir(&self) -> Result<&Path, TargetError> {
        self.run_dir
            .as_deref()
            .ok_or_else(|| TargetError::Protocol("process target is not launched".into()))
    }

    fn validate_argv(argv: &[String]) -> Result<(), TargetError> {
        if argv.len() > MAX_ARG_COUNT {
            return Err(TargetError::Protocol(format!(
                "argv has {} arguments; limit is {MAX_ARG_COUNT}",
                argv.len()
            )));
        }
        let total = argv.iter().try_fold(0usize, |total, argument| {
            if argument.len() > MAX_ARG_BYTES {
                return Err(TargetError::Protocol(format!(
                    "argument is {} bytes; limit is {MAX_ARG_BYTES}",
                    argument.len()
                )));
            }
            total
                .checked_add(argument.len())
                .ok_or_else(|| TargetError::Protocol("argv byte count overflowed".into()))
        })?;
        if total > MAX_ARGV_BYTES {
            return Err(TargetError::Protocol(format!(
                "argv is {total} bytes; limit is {MAX_ARGV_BYTES}"
            )));
        }
        Ok(())
    }

    fn spawn_argv(
        &mut self,
        action: &Action,
        argv: &[String],
    ) -> Result<(DevReply, ProcessObservation, Vec<String>), TargetError> {
        Self::validate_argv(argv)?;
        let run_dir = self.run_dir()?.to_owned();
        let log_dir = run_dir.join("logs/process");
        fs::create_dir_all(&log_dir)?;
        let stdout_path = log_dir.join(format!("{:06}.stdout.log", action.sequence));
        let stderr_path = log_dir.join(format!("{:06}.stderr.log", action.sequence));
        let stdout = File::create(&stdout_path)?;
        let stderr = File::create(&stderr_path)?;

        let mut command = Command::new(&self.config.executable);
        command.env_clear();
        for name in ["PATH", "LANG", "LC_ALL", "RUST_BACKTRACE"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command
            .env("SOTF_QA_DIR", run_dir.join("qa"))
            .env("TMPDIR", run_dir.join("tmp"))
            .current_dir(&run_dir)
            .args(argv)
            .stdin(Stdio::piped())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        for (name, value) in &self.config.environment {
            command.env(name, value);
        }

        let child = command.spawn()?;
        let pid = child.id();
        self.active_child = Some(child);
        self.launches = self.launches.saturating_add(1);
        let deadline = Instant::now() + Duration::from_millis(action.timeout_ms.max(1));
        let status = loop {
            let child = self
                .active_child
                .as_mut()
                .ok_or_else(|| TargetError::Protocol("active child disappeared".into()))?;
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill()?;
                let _ = child.wait();
                self.active_child = None;
                return Err(TargetError::Timeout(format!(
                    "{} did not exit within {} ms",
                    self.config.executable.display(),
                    action.timeout_ms
                )));
            }
            thread::sleep(Duration::from_millis(5));
        };
        self.active_child = None;

        let process = process_from_status(pid, &status);
        let logs = [stdout_path, stderr_path]
            .iter()
            .flat_map(|path| read_bounded_lines(path).unwrap_or_default())
            .collect::<Vec<_>>();
        let reply = if status.success() {
            DevReply::success(json!({
                "exit_code": status.code(),
                "argv_count": argv.len(),
            }))
        } else {
            DevReply::failure("process_exit", format!("process exited with {status}"))
        };
        Ok((reply, process, logs))
    }

    fn stop_active(&mut self) -> Result<ProcessObservation, TargetError> {
        let Some(mut child) = self.active_child.take() else {
            return Ok(self.last_process.clone());
        };
        let pid = child.id();
        child.kill()?;
        let status = child.wait()?;
        Ok(process_from_status(pid, &status))
    }
}

impl FuzzTarget for ProcessTarget {
    fn target_id(&self) -> TargetId {
        self.config.target
    }

    fn launch(&mut self, context: &LaunchContext<'_>) -> Result<TargetSpec, TargetError> {
        if !self.config.executable.is_file() {
            return Err(TargetError::Unsupported(
                crate::fuzz::model::StructuredSkip {
                    target_id: self.config.target,
                    reason_code: "runtime_missing".into(),
                    reason: format!(
                        "executable {} does not exist; build the target first",
                        self.config.executable.display()
                    ),
                },
            ));
        }
        self.config.executable = fs::canonicalize(&self.config.executable)?;
        fs::create_dir_all(context.run_dir.join("logs/process"))?;
        fs::create_dir_all(context.run_dir.join("qa"))?;
        fs::create_dir_all(context.run_dir.join("tmp"))?;
        self.run_dir = Some(context.run_dir.to_owned());
        Ok(TargetSpec {
            schema_version: FUZZ_SCHEMA_VERSION,
            target_id: self.config.target,
            adapter: AdapterKind::Process,
            executable: Some(self.config.executable.clone()),
            app_identity: self
                .config
                .executable
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()),
            platform: std::env::consts::OS.into(),
            fixture_profile: context.fixture_profile.into(),
            environment_names: self.config.environment.keys().cloned().collect(),
            endpoints: vec![],
            run_id_hash: context.run_id.redacted_hash(),
            capability_fingerprint: String::new(),
            build_id: "development".into(),
        })
    }

    fn capabilities(&mut self) -> Result<Capabilities, TargetError> {
        let process_name = self.config.executable.file_name().map_or_else(
            || "process".into(),
            |name| name.to_string_lossy().into_owned(),
        );
        let mut capabilities = Capabilities::new(self.config.target.as_str(), process_name);
        capabilities.debug_features = vec!["hermetic-process".into(), "fake-backends".into()];
        Ok(capabilities)
    }

    fn snapshot(&mut self) -> Result<Snapshot, TargetError> {
        Snapshot::new(
            self.config.target.as_str(),
            self.revision,
            json!({
                "phase": if self.active_child.is_some() { "running" } else { "ready" },
                "launches": self.launches,
                "last_exit_code": self.last_process.exit_code,
                "last_signal_or_exception": self.last_process.signal_or_exception,
            }),
        )
        .map_err(|error| TargetError::Protocol(error.to_string()))
    }

    fn execute(&mut self, action: &Action) -> Result<Observation, TargetError> {
        let (reply, process, new_logs) = match &action.payload {
            ActionPayload::ProcessArgv { argv } => self.spawn_argv(action, argv)?,
            ActionPayload::Wait { duration_ms } => {
                thread::sleep(Duration::from_millis((*duration_ms).min(action.timeout_ms)));
                (
                    DevReply::success(Value::Null),
                    self.last_process.clone(),
                    vec![],
                )
            }
            ActionPayload::Signal { signal } if signal == "TERM" || signal == "KILL" => (
                DevReply::success(json!({"signal": signal})),
                self.stop_active()?,
                vec![],
            ),
            ActionPayload::Restart => {
                let process = self.stop_active()?;
                (
                    DevReply::success(json!({"restarted": true})),
                    process,
                    vec![],
                )
            }
            ActionPayload::Stdin { .. } => (
                DevReply::failure("no_active_process", "stdin requires a running process"),
                self.last_process.clone(),
                vec![],
            ),
            other => {
                return Err(TargetError::Protocol(format!(
                    "payload {other:?} is not supported by process adapter"
                )));
            }
        };
        self.revision = self.revision.saturating_add(1);
        self.last_process = process.clone();
        let snapshot = self.snapshot()?;
        Ok(Observation {
            schema_version: FUZZ_SCHEMA_VERSION,
            sequence: action.sequence,
            reply: Some(reply),
            snapshot: Some(snapshot),
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
        Ok(true)
    }

    fn pid(&self) -> Option<u32> {
        self.active_child.as_ref().map(Child::id)
    }

    fn capture_hang(&mut self, _directory: &Path) -> Result<Vec<PathBuf>, TargetError> {
        Ok(vec![])
    }

    fn shutdown(&mut self) -> Result<(), TargetError> {
        let _ = self.stop_active()?;
        Ok(())
    }
}

fn read_bounded_lines(path: &Path) -> std::io::Result<Vec<String>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_LOG_BYTES)
        .read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes)
        .lines()
        .take(256)
        .map(str::to_owned)
        .collect())
}

fn process_from_status(pid: u32, status: &ExitStatus) -> ProcessObservation {
    let signal_or_exception = status_signal(status);
    ProcessObservation {
        pid: Some(pid),
        // A CLI invocation reaching an ordinary exit is a completed command,
        // not the supervisor itself dying unexpectedly.
        alive: signal_or_exception.is_none(),
        exit_code: status.code(),
        signal_or_exception,
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
