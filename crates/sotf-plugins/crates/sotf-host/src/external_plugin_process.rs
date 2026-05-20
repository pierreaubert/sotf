//! Subprocess supervisor for isolated external plugin workers.
//!
//! The audio thread should not call `ensure_running()` or perform restarts.
//! A manager/control thread owns this supervisor, launches the worker with the
//! secure shared-memory path, polls for exits, and restarts after crashes. The
//! audio thread only observes missed deadlines via `ExternalPluginHostProxy`.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};

#[derive(Debug, Clone)]
pub struct ExternalPluginWorkerCommand {
    program: PathBuf,
    args: Vec<String>,
    env: Vec<(String, String)>,
}

impl ExternalPluginWorkerCommand {
    pub const DEFAULT_WORKER_BINARY: &'static str = "sotf-external-plugin-worker";

    pub fn default_worker_binary() -> Self {
        Self::new(Self::DEFAULT_WORKER_BINARY)
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
            .args(&self.args)
            .arg("--shared-memory")
            .arg(shared_memory_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
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

pub struct ExternalPluginProcessSupervisor {
    command: ExternalPluginWorkerCommand,
    shared_memory_path: PathBuf,
    child: Option<Child>,
    start_count: u64,
    exit_count: u64,
    launch_failure_count: u64,
}

impl ExternalPluginProcessSupervisor {
    pub fn new(
        command: ExternalPluginWorkerCommand,
        shared_memory_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            command,
            shared_memory_path: shared_memory_path.into(),
            child: None,
            start_count: 0,
            exit_count: 0,
            launch_failure_count: 0,
        }
    }

    pub fn ensure_running(&mut self) -> Result<ExternalPluginProcessEvent, String> {
        if let Some(event) = self.poll()? {
            if matches!(event, ExternalPluginProcessEvent::Exited { .. }) {
                return self.start();
            }
        }

        if self.child.is_some() {
            return Ok(ExternalPluginProcessEvent::AlreadyRunning);
        }

        self.start()
    }

    pub fn poll(&mut self) -> Result<Option<ExternalPluginProcessEvent>, String> {
        let Some(child) = self.child.as_mut() else {
            return Ok(Some(ExternalPluginProcessEvent::NotRunning));
        };

        match child.try_wait() {
            Ok(Some(status)) => {
                self.child = None;
                self.exit_count = self.exit_count.saturating_add(1);
                Ok(Some(ExternalPluginProcessEvent::Exited { status }))
            }
            Ok(None) => Ok(None),
            Err(err) => Err(format!("failed to poll external plugin worker: {err}")),
        }
    }

    pub fn terminate(&mut self) -> Result<(), String> {
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

    pub fn is_running(&mut self) -> Result<bool, String> {
        Ok(self.poll()?.is_none())
    }

    pub fn start_count(&self) -> u64 {
        self.start_count
    }

    pub fn exit_count(&self) -> u64 {
        self.exit_count
    }

    pub fn launch_failure_count(&self) -> u64 {
        self.launch_failure_count
    }

    pub fn shared_memory_path(&self) -> &Path {
        &self.shared_memory_path
    }

    fn start(&mut self) -> Result<ExternalPluginProcessEvent, String> {
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

impl Drop for ExternalPluginProcessSupervisor {
    fn drop(&mut self) {
        let _ = self.terminate();
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
            ExternalPluginProcessSupervisor::new(command, "/tmp/sotf-plugin-test.shm");

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
            ExternalPluginProcessSupervisor::new(command, "/tmp/sotf-plugin-test.shm");

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
            ExternalPluginProcessSupervisor::new(command, "/tmp/sotf-plugin-test.shm");

        let err = supervisor.ensure_running().unwrap_err();
        assert!(err.contains("failed to launch external plugin worker"));
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
    fn test_worker_command_does_not_publish_shared_memory_path_in_env_by_default() {
        let command = ExternalPluginWorkerCommand::new("/usr/bin/true");
        let process_command = command.to_command(Path::new("/tmp/sotf-plugin-test.shm"));
        assert!(
            process_command
                .get_envs()
                .all(|(key, _)| key != "SOTF_PLUGIN_SHARED_MEMORY")
        );
    }
}
