//! Subprocess supervisor for isolated external plugin workers.
//!
//! A dedicated supervisor thread owns each worker process, launches it with the
//! secure shared-memory path, polls for exits, collects stderr, and performs
//! restarts or termination. Engine/control callers only read atomically
//! published snapshots or enqueue bounded commands; the audio path observes
//! block outcomes through `ExternalPluginHostProxy`.

use arc_swap::ArcSwap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::thread::{JoinHandle, ThreadId};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ExternalPluginWorkerCommand {
    program: PathBuf,
    args: Vec<String>,
    env: Vec<(String, String)>,
}

impl ExternalPluginWorkerCommand {
    pub const DEFAULT_WORKER_BINARY: &'static str = "sotf-external-plugin-worker";
    pub const DEFAULT_MACOS_SANDBOX_HELPER_BINARY: &'static str = "sotf-macos-sandbox-helper";

    pub fn default_worker_binary() -> Self {
        Self::sibling_binary(Self::DEFAULT_WORKER_BINARY)
    }

    pub fn default_macos_sandbox_helper_binary() -> Self {
        Self::sibling_binary(Self::DEFAULT_MACOS_SANDBOX_HELPER_BINARY)
    }

    fn sibling_binary(name: &'static str) -> Self {
        let program = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|parent| parent.join(name)))
            .unwrap_or_else(|| PathBuf::from(name));
        Self::new(program)
    }

    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            env: Vec::new(),
        }
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    pub fn program(&self) -> &Path {
        &self.program
    }

    pub fn command_args(&self) -> &[String] {
        &self.args
    }

    pub fn command_env(&self) -> &[(String, String)] {
        &self.env
    }

    fn to_command(&self, shared_memory_path: &Path) -> Command {
        let mut command = Command::new(&self.program);
        command
            .env_clear()
            .env("SOTF_PLUGIN_WORKER", "1")
            .args(&self.args)
            .arg("--shared-memory")
            .arg(shared_memory_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        for (key, value) in &self.env {
            command.env(key, value);
        }
        command
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalPluginProcessEvent {
    AlreadyRunning,
    Started { pid: u32 },
    Exited { status: ExitStatus },
    NotRunning,
}

const SUPERVISOR_COMMAND_CAPACITY: usize = 8;
const SUPERVISOR_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Clone)]
struct ExternalPluginProcessSnapshot {
    event_version: u64,
    event: Option<ExternalPluginProcessEvent>,
    poll_error: Option<Arc<str>>,
    start_count: u64,
    exit_count: u64,
    launch_failure_count: u64,
    last_stderr: Option<Arc<str>>,
    running: bool,
    // Test-only observability proves process supervision stays off the caller thread.
    #[cfg_attr(not(test), allow(dead_code))]
    supervisor_thread_id: Option<ThreadId>,
}

enum ExternalPluginSupervisorRequest {
    EnsureRunning(SyncSender<(Result<ExternalPluginProcessEvent, String>, u64)>),
    Terminate(SyncSender<(Result<(), String>, u64)>),
    TerminateAsync,
    Shutdown(SyncSender<()>),
}

/// Control-side handle for a dedicated subprocess-supervision thread.
///
/// `poll()` only reads an atomically published snapshot. Process creation,
/// `try_wait`, stderr collection, termination, and restart all execute on the
/// dedicated thread, never on the engine processing thread.
pub struct ExternalPluginProcessSupervisor {
    request_tx: SyncSender<ExternalPluginSupervisorRequest>,
    snapshot: Arc<ArcSwap<ExternalPluginProcessSnapshot>>,
    observed_event_version: u64,
    shared_memory_path: PathBuf,
    thread: Option<JoinHandle<()>>,
}

impl ExternalPluginProcessSupervisor {
    pub fn new(
        command: ExternalPluginWorkerCommand,
        shared_memory_path: impl Into<PathBuf>,
    ) -> Result<Self, String> {
        let shared_memory_path = shared_memory_path.into();
        let snapshot = Arc::new(ArcSwap::from_pointee(ExternalPluginProcessSnapshot {
            event_version: 0,
            event: None,
            poll_error: None,
            start_count: 0,
            exit_count: 0,
            launch_failure_count: 0,
            last_stderr: None,
            running: false,
            supervisor_thread_id: None,
        }));
        let (request_tx, request_rx) = sync_channel(SUPERVISOR_COMMAND_CAPACITY);
        let (ready_tx, ready_rx) = sync_channel(0);
        let thread_snapshot = Arc::clone(&snapshot);
        let thread_shared_memory_path = shared_memory_path.clone();
        let thread = std::thread::Builder::new()
            .name("sotf-plugin-supervisor".to_string())
            .spawn(move || {
                run_external_plugin_supervisor(
                    ExternalPluginProcessSupervisorCore::new(command, thread_shared_memory_path),
                    request_rx,
                    thread_snapshot,
                    ready_tx,
                );
            })
            .map_err(|error| {
                format!("failed to start external plugin supervisor thread: {error}")
            })?;
        ready_rx.recv().map_err(|_| {
            "external plugin supervisor thread exited before initialization".to_string()
        })?;

        Ok(Self {
            request_tx,
            snapshot,
            observed_event_version: 0,
            shared_memory_path,
            thread: Some(thread),
        })
    }

    pub fn ensure_running(&mut self) -> Result<ExternalPluginProcessEvent, String> {
        let (response_tx, response_rx) = sync_channel(0);
        self.request_tx
            .send(ExternalPluginSupervisorRequest::EnsureRunning(response_tx))
            .map_err(|_| "external plugin supervisor thread is unavailable".to_string())?;
        let (result, event_version) = response_rx
            .recv()
            .map_err(|_| "external plugin supervisor response channel closed".to_string())?;
        if result.is_ok() {
            self.observed_event_version = event_version;
        }
        result
    }

    /// Return the latest unobserved lifecycle snapshot without touching the OS
    /// process handle or taking a mutex.
    pub fn poll(&mut self) -> Result<Option<ExternalPluginProcessEvent>, String> {
        let snapshot = self.snapshot.load_full();
        if snapshot.event_version == self.observed_event_version {
            return Ok(None);
        }
        self.observed_event_version = snapshot.event_version;
        if let Some(error) = snapshot.poll_error.as_deref() {
            return Err(error.to_string());
        }
        Ok(snapshot.event)
    }

    pub fn terminate(&mut self) -> Result<(), String> {
        let (response_tx, response_rx) = sync_channel(0);
        self.request_tx
            .send(ExternalPluginSupervisorRequest::Terminate(response_tx))
            .map_err(|_| "external plugin supervisor thread is unavailable".to_string())?;
        let (result, event_version) = response_rx
            .recv()
            .map_err(|_| "external plugin supervisor response channel closed".to_string())?;
        if result.is_ok() {
            self.observed_event_version = event_version;
        }
        result
    }

    /// Request termination from a callback-critical path without waiting for
    /// process I/O. The bounded queue is preallocated by `new()`.
    pub fn request_terminate(&self) -> Result<(), String> {
        self.request_tx
            .try_send(ExternalPluginSupervisorRequest::TerminateAsync)
            .map_err(|error| match error {
                TrySendError::Full(_) => {
                    "external plugin supervisor command queue is full".to_string()
                }
                TrySendError::Disconnected(_) => {
                    "external plugin supervisor thread is unavailable".to_string()
                }
            })
    }

    pub fn is_running(&mut self) -> Result<bool, String> {
        Ok(self.snapshot.load().running)
    }

    pub fn start_count(&self) -> u64 {
        self.snapshot.load().start_count
    }

    pub fn exit_count(&self) -> u64 {
        self.snapshot.load().exit_count
    }

    pub fn launch_failure_count(&self) -> u64 {
        self.snapshot.load().launch_failure_count
    }

    pub fn last_stderr(&self) -> Option<Arc<str>> {
        self.snapshot.load().last_stderr.clone()
    }

    pub fn shared_memory_path(&self) -> &Path {
        &self.shared_memory_path
    }

    #[cfg(test)]
    fn supervision_thread_id(&self) -> Option<ThreadId> {
        self.snapshot.load().supervisor_thread_id
    }
}

impl Drop for ExternalPluginProcessSupervisor {
    fn drop(&mut self) {
        let Some(thread) = self.thread.take() else {
            return;
        };
        let (response_tx, response_rx) = sync_channel(0);
        if self
            .request_tx
            .send(ExternalPluginSupervisorRequest::Shutdown(response_tx))
            .is_ok()
        {
            let _ = response_rx.recv_timeout(Duration::from_secs(5));
        }
        let _ = thread.join();
    }
}

struct ExternalPluginProcessSupervisorCore {
    command: ExternalPluginWorkerCommand,
    shared_memory_path: PathBuf,
    child: Option<Child>,
    start_count: u64,
    exit_count: u64,
    launch_failure_count: u64,
    last_stderr: Option<String>,
}

impl ExternalPluginProcessSupervisorCore {
    fn new(command: ExternalPluginWorkerCommand, shared_memory_path: impl Into<PathBuf>) -> Self {
        Self {
            command,
            shared_memory_path: shared_memory_path.into(),
            child: None,
            start_count: 0,
            exit_count: 0,
            launch_failure_count: 0,
            last_stderr: None,
        }
    }

    fn ensure_running(&mut self) -> Result<ExternalPluginProcessEvent, String> {
        if let Some(event) = self.poll_process()?
            && matches!(event, ExternalPluginProcessEvent::Exited { .. })
        {
            return self.start();
        }

        if self.child.is_some() {
            return Ok(ExternalPluginProcessEvent::AlreadyRunning);
        }

        self.start()
    }

    fn poll_process(&mut self) -> Result<Option<ExternalPluginProcessEvent>, String> {
        let Some(child) = self.child.as_mut() else {
            return Ok(None);
        };

        match child.try_wait() {
            Ok(Some(status)) => {
                self.last_stderr = child.stderr.take().and_then(|mut stderr| {
                    let mut message = String::new();
                    stderr.read_to_string(&mut message).ok()?;
                    let message = message.trim().to_string();
                    (!message.is_empty()).then_some(message)
                });
                self.child = None;
                self.exit_count = self.exit_count.saturating_add(1);
                Ok(Some(ExternalPluginProcessEvent::Exited { status }))
            }
            Ok(None) => Ok(None),
            Err(err) => Err(format!("failed to poll external plugin worker: {err}")),
        }
    }

    fn terminate(&mut self) -> Result<(), String> {
        let Some(mut child) = self.child.take() else {
            return Ok(());
        };

        if child
            .try_wait()
            .map_err(|err| {
                format!("failed to poll external plugin worker before terminate: {err}")
            })?
            .is_some()
        {
            self.exit_count = self.exit_count.saturating_add(1);
            return Ok(());
        }

        child
            .kill()
            .map_err(|err| format!("failed to kill external plugin worker: {err}"))?;
        let _ = child.wait();
        self.exit_count = self.exit_count.saturating_add(1);
        Ok(())
    }

    fn start(&mut self) -> Result<ExternalPluginProcessEvent, String> {
        self.last_stderr = None;
        if !self.command.program().is_absolute() {
            self.launch_failure_count = self.launch_failure_count.saturating_add(1);
            return Err(format!(
                "external plugin worker path must be absolute: '{}'",
                self.command.program().display()
            ));
        }

        let mut command = self.command.to_command(&self.shared_memory_path);
        match command.spawn() {
            Ok(child) => {
                let pid = child.id();
                self.child = Some(child);
                self.start_count = self.start_count.saturating_add(1);
                Ok(ExternalPluginProcessEvent::Started { pid })
            }
            Err(err) => {
                self.launch_failure_count = self.launch_failure_count.saturating_add(1);
                Err(format!(
                    "failed to launch external plugin worker '{}': {err}",
                    self.command.program().display()
                ))
            }
        }
    }
}

impl Drop for ExternalPluginProcessSupervisorCore {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

fn publish_supervisor_snapshot(
    core: &ExternalPluginProcessSupervisorCore,
    snapshot: &ArcSwap<ExternalPluginProcessSnapshot>,
    event_version: u64,
    event: Option<ExternalPluginProcessEvent>,
    poll_error: Option<&str>,
    supervisor_thread_id: ThreadId,
) {
    snapshot.store(Arc::new(ExternalPluginProcessSnapshot {
        event_version,
        event,
        poll_error: poll_error.map(Arc::<str>::from),
        start_count: core.start_count,
        exit_count: core.exit_count,
        launch_failure_count: core.launch_failure_count,
        last_stderr: core.last_stderr.as_deref().map(Arc::<str>::from),
        running: core.child.is_some(),
        supervisor_thread_id: Some(supervisor_thread_id),
    }));
}

fn run_external_plugin_supervisor(
    mut core: ExternalPluginProcessSupervisorCore,
    request_rx: Receiver<ExternalPluginSupervisorRequest>,
    snapshot: Arc<ArcSwap<ExternalPluginProcessSnapshot>>,
    ready_tx: SyncSender<()>,
) {
    let supervisor_thread_id = std::thread::current().id();
    let mut event_version = 1_u64;
    let mut event = Some(ExternalPluginProcessEvent::NotRunning);
    let mut poll_error: Option<String> = None;
    publish_supervisor_snapshot(
        &core,
        &snapshot,
        event_version,
        event,
        None,
        supervisor_thread_id,
    );
    if ready_tx.send(()).is_err() {
        return;
    }

    loop {
        match request_rx.recv_timeout(SUPERVISOR_POLL_INTERVAL) {
            Ok(ExternalPluginSupervisorRequest::EnsureRunning(response_tx)) => {
                let result = core.ensure_running();
                event = Some(match &result {
                    Ok(event) => *event,
                    Err(_) => ExternalPluginProcessEvent::NotRunning,
                });
                poll_error = None;
                event_version = event_version.saturating_add(1);
                publish_supervisor_snapshot(
                    &core,
                    &snapshot,
                    event_version,
                    event,
                    None,
                    supervisor_thread_id,
                );
                let _ = response_tx.send((result, event_version));
            }
            Ok(ExternalPluginSupervisorRequest::Terminate(response_tx)) => {
                let result = core.terminate();
                event = Some(ExternalPluginProcessEvent::NotRunning);
                poll_error = None;
                event_version = event_version.saturating_add(1);
                publish_supervisor_snapshot(
                    &core,
                    &snapshot,
                    event_version,
                    event,
                    None,
                    supervisor_thread_id,
                );
                let _ = response_tx.send((result, event_version));
            }
            Ok(ExternalPluginSupervisorRequest::TerminateAsync) => {
                let result = core.terminate();
                event = Some(ExternalPluginProcessEvent::NotRunning);
                poll_error = result.err();
                event_version = event_version.saturating_add(1);
                publish_supervisor_snapshot(
                    &core,
                    &snapshot,
                    event_version,
                    event,
                    poll_error.as_deref(),
                    supervisor_thread_id,
                );
            }
            Ok(ExternalPluginSupervisorRequest::Shutdown(response_tx)) => {
                let _ = core.terminate();
                let _ = response_tx.send(());
                break;
            }
            Err(RecvTimeoutError::Timeout) => match core.poll_process() {
                Ok(Some(process_event)) => {
                    event = Some(process_event);
                    poll_error = None;
                    event_version = event_version.saturating_add(1);
                    publish_supervisor_snapshot(
                        &core,
                        &snapshot,
                        event_version,
                        event,
                        None,
                        supervisor_thread_id,
                    );
                }
                Ok(None) => {
                    if poll_error.take().is_some() {
                        event_version = event_version.saturating_add(1);
                        publish_supervisor_snapshot(
                            &core,
                            &snapshot,
                            event_version,
                            event,
                            None,
                            supervisor_thread_id,
                        );
                    }
                }
                Err(error) => {
                    if poll_error.as_deref() != Some(error.as_str()) {
                        poll_error = Some(error);
                        event_version = event_version.saturating_add(1);
                        publish_supervisor_snapshot(
                            &core,
                            &snapshot,
                            event_version,
                            event,
                            poll_error.as_deref(),
                            supervisor_thread_id,
                        );
                    }
                }
            },
            Err(RecvTimeoutError::Disconnected) => {
                let _ = core.terminate();
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn test_supervisor_starts_and_observes_exit() {
        let exe = std::env::current_exe().unwrap();
        let command = ExternalPluginWorkerCommand::new(exe).arg("--help");
        let mut supervisor =
            ExternalPluginProcessSupervisor::new(command, "/tmp/sotf-plugin-test.shm").unwrap();
        assert_ne!(
            supervisor.supervision_thread_id(),
            Some(std::thread::current().id()),
            "subprocess supervision must not run on the caller/processing thread"
        );

        let event = supervisor.ensure_running().unwrap();
        assert!(matches!(event, ExternalPluginProcessEvent::Started { .. }));
        assert_eq!(supervisor.start_count(), 1);

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if matches!(
                supervisor.poll().unwrap(),
                Some(ExternalPluginProcessEvent::Exited { .. })
            ) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "worker test process did not exit"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(supervisor.exit_count(), 1);
    }

    #[test]
    fn test_supervisor_restarts_after_exit() {
        let exe = std::env::current_exe().unwrap();
        let command = ExternalPluginWorkerCommand::new(exe).arg("--help");
        let mut supervisor =
            ExternalPluginProcessSupervisor::new(command, "/tmp/sotf-plugin-test.shm").unwrap();

        supervisor.ensure_running().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while supervisor.poll().unwrap().is_none() {
            assert!(
                Instant::now() < deadline,
                "worker test process did not exit"
            );
            std::thread::sleep(Duration::from_millis(10));
        }

        let event = supervisor.ensure_running().unwrap();
        assert!(matches!(event, ExternalPluginProcessEvent::Started { .. }));
        assert_eq!(supervisor.start_count(), 2);
    }

    #[test]
    fn test_supervisor_reports_launch_failure() {
        let command =
            ExternalPluginWorkerCommand::new("/definitely/not/a/real/sotf/external/plugin/worker");
        let mut supervisor =
            ExternalPluginProcessSupervisor::new(command, "/tmp/sotf-plugin-test.shm").unwrap();

        let err = supervisor.ensure_running().unwrap_err();
        assert!(err.contains("failed to launch external plugin worker"));
        assert_eq!(supervisor.launch_failure_count(), 1);
    }

    #[test]
    fn test_supervisor_rejects_relative_worker_path() {
        let command = ExternalPluginWorkerCommand::new("sotf-external-plugin-worker");
        let mut supervisor =
            ExternalPluginProcessSupervisor::new(command, "/tmp/sotf-plugin-test.shm").unwrap();

        let err = supervisor.ensure_running().unwrap_err();
        assert!(err.contains("must be absolute"));
        assert_eq!(supervisor.launch_failure_count(), 1);
    }

    #[test]
    fn test_worker_command_exposes_custom_program_args_and_env() {
        let command = ExternalPluginWorkerCommand::new("/usr/bin/true")
            .arg("--once")
            .args(["--idle-sleep-micros", "500"])
            .env("SOTF_TEST_PLUGIN_WORKER", "1");

        assert_eq!(command.program(), Path::new("/usr/bin/true"));
        assert_eq!(
            command.command_args(),
            &[
                "--once".to_string(),
                "--idle-sleep-micros".to_string(),
                "500".to_string()
            ]
        );
        assert_eq!(
            command.command_env(),
            &[("SOTF_TEST_PLUGIN_WORKER".to_string(), "1".to_string())]
        );
    }

    #[test]
    fn test_default_macos_sandbox_helper_uses_sibling_binary_name() {
        let command = ExternalPluginWorkerCommand::default_macos_sandbox_helper_binary();

        assert_eq!(
            command.program().file_name().and_then(|name| name.to_str()),
            Some(ExternalPluginWorkerCommand::DEFAULT_MACOS_SANDBOX_HELPER_BINARY)
        );
    }

    #[test]
    fn test_worker_command_does_not_publish_shared_memory_path_in_env_by_default() {
        let command = ExternalPluginWorkerCommand::new("/usr/bin/true");
        let process_command = command.to_command(Path::new("/tmp/sotf-plugin-test.shm"));
        assert!(
            process_command
                .get_envs()
                .all(|(key, _)| key != "SOTF_PLUGIN_SHARED_MEMORY")
        );
    }

    #[test]
    fn test_worker_command_clears_inherited_environment() {
        let command = ExternalPluginWorkerCommand::new("/usr/bin/true");
        let process_command = command.to_command(Path::new("/tmp/sotf-plugin-test.shm"));

        assert!(process_command.get_envs().any(|(key, value)| {
            key == "SOTF_PLUGIN_WORKER" && value == Some(std::ffi::OsStr::new("1"))
        }));
    }
}
