use super::consts::DEFAULT_DEADLINE_MICROS;
use super::consts::DEFAULT_MAX_BLOCK_FRAMES;
use super::consts::DEFAULT_MAX_CONSECUTIVE_BLOCK_FAILURES;
use super::consts::DEFAULT_WORKER_STARTUP_TIMEOUT_MILLIS;
use super::misc::decorate_sandbox_launcher_command;
use crate::ExternalPluginState;
use crate::external_plugin_process::ExternalPluginWorkerCommand;
use crate::external_plugin_sandbox::{
    ExternalPluginSandboxPolicy, PluginSandboxLaunchBackend, PluginSandboxPolicy,
    current_plugin_sandbox_launch_backend, default_plugin_sandbox_launcher_command_for_backend,
};
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct IsolatedExternalPluginConfig {
    pub max_block_frames: u32,
    pub deadline: Duration,
    pub worker_command: ExternalPluginWorkerCommand,
    pub sandbox_policy: ExternalPluginSandboxPolicy,
    pub capability_sandbox_policy: Option<PluginSandboxPolicy>,
    pub sandbox_launch_backend: PluginSandboxLaunchBackend,
    pub sandbox_launcher_command: Option<ExternalPluginWorkerCommand>,
    pub start_worker: bool,
    /// Bounded constructor-time handshake used to obtain immutable worker
    /// metadata before the host compiles and caches graph latency.
    pub worker_startup_timeout: Duration,
    pub max_consecutive_block_failures: u32,
    pub initial_state: Option<ExternalPluginState>,
    /// Stable player-side plugin identity when built from a serialized rack or graph.
    pub plugin_instance_id: Option<usize>,
}

impl Default for IsolatedExternalPluginConfig {
    fn default() -> Self {
        let sandbox_launch_backend = current_plugin_sandbox_launch_backend();
        Self {
            max_block_frames: DEFAULT_MAX_BLOCK_FRAMES,
            deadline: Duration::from_micros(DEFAULT_DEADLINE_MICROS),
            worker_command: ExternalPluginWorkerCommand::default_worker_binary(),
            sandbox_policy: ExternalPluginSandboxPolicy::default(),
            capability_sandbox_policy: None,
            sandbox_launch_backend,
            sandbox_launcher_command: default_plugin_sandbox_launcher_command_for_backend(
                sandbox_launch_backend,
            ),
            start_worker: true,
            worker_startup_timeout: Duration::from_millis(DEFAULT_WORKER_STARTUP_TIMEOUT_MILLIS),
            max_consecutive_block_failures: DEFAULT_MAX_CONSECUTIVE_BLOCK_FAILURES,
            initial_state: None,
            plugin_instance_id: None,
        }
    }
}

pub(super) fn build_worker_launch_command(
    config: &IsolatedExternalPluginConfig,
    descriptor_json: String,
    state_path: Option<&Path>,
    sandbox_args: Vec<String>,
) -> Result<ExternalPluginWorkerCommand, String> {
    let command = if config.sandbox_launch_backend.requires_host_launcher() {
        let launcher = config.sandbox_launcher_command.clone().ok_or_else(|| {
            format!(
                "sandbox backend '{}' requires a host-owned sandbox launcher command",
                config.sandbox_launch_backend.backend_id()
            )
        })?;
        decorate_sandbox_launcher_command(launcher, &config.worker_command)
    } else {
        config.worker_command.clone()
    };

    let command = command
        .arg("--descriptor-json")
        .arg(descriptor_json)
        .args(sandbox_args);
    Ok(match state_path {
        Some(state_path) => {
            let state_path = state_path.to_str().ok_or_else(|| {
                format!(
                    "external plugin state path is not valid UTF-8: {}",
                    state_path.display()
                )
            })?;
            command
                .arg("--external-state-file")
                .arg(state_path)
                .arg("--sandbox-read-path")
                .arg(state_path)
        }
        None => command,
    })
}
