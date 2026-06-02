//! Plugin wrapper that routes an external plugin through an isolated worker.
//!
//! This implements the normal [`Plugin`] trait while keeping unknown plugin
//! execution in a worker process. The audio callback path only publishes a block
//! to shared memory and consumes the worker result; restart decisions remain on
//! the owner/control side through the process supervisor.

use std::time::Duration;

use crate::external_plugin::{
    ExternalPluginHostingPlan, ExternalPluginSandboxMode, ExternalPluginState, PluginDescriptor,
    plan_external_plugin_hosting,
};
use crate::external_plugin_host::{ExternalPluginHostBlockStatus, ExternalPluginHostProxy};
use crate::external_plugin_ipc::{PluginIpcLayout, PluginSandboxRuntimeStatus};
use crate::external_plugin_process::{
    ExternalPluginProcessEvent, ExternalPluginProcessSupervisor, ExternalPluginWorkerCommand,
};
use crate::external_plugin_sandbox::{
    ExternalPluginSandboxPolicy, PluginSandboxLaunchBackend, PluginSandboxPolicy,
    current_plugin_sandbox_launch_backend, default_plugin_sandbox_launcher_command_for_backend,
};
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use crate::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};

const DEFAULT_MAX_BLOCK_FRAMES: u32 = 8192;
const DEFAULT_DEADLINE_MICROS: u64 = 2_000;
const DEFAULT_MAX_CONSECUTIVE_BLOCK_FAILURES: u32 = 8;

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
    pub max_consecutive_block_failures: u32,
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
            max_consecutive_block_failures: DEFAULT_MAX_CONSECUTIVE_BLOCK_FAILURES,
        }
    }
}

pub struct IsolatedExternalPlugin {
    descriptor: PluginDescriptor,
    proxy: ExternalPluginHostProxy,
    supervisor: Option<ExternalPluginProcessSupervisor>,
    input_channels: usize,
    output_channels: usize,
    launch_error: Option<String>,
    consecutive_block_failures: u32,
    max_consecutive_block_failures: u32,
    quarantined: bool,
}

impl IsolatedExternalPlugin {
    pub fn new(
        descriptor: PluginDescriptor,
        sample_rate: u32,
        config: IsolatedExternalPluginConfig,
    ) -> Result<Self, String> {
        if sample_rate == 0 {
            return Err("sample rate must be positive".into());
        }

        let input_channels = descriptor.audio_inputs;
        let output_channels = descriptor.audio_outputs.max(1);
        let layout = PluginIpcLayout::new(
            sample_rate,
            config.max_block_frames,
            input_channels as u32,
            output_channels as u32,
        )
        .map_err(|err| format!("invalid isolated external-plugin layout: {err}"))?;
        let proxy = ExternalPluginHostProxy::new(layout, config.deadline)?;
        let descriptor_json = serde_json::to_string(&descriptor)
            .map_err(|err| format!("failed to serialize external plugin descriptor: {err}"))?;
        let sandbox_args = match &config.capability_sandbox_policy {
            Some(policy) => policy.command_args_for_backend(config.sandbox_launch_backend)?,
            None => config.sandbox_policy.command_args(),
        };
        let command = build_worker_launch_command(&config, descriptor_json, sandbox_args)?;
        let mut supervisor =
            ExternalPluginProcessSupervisor::new(command, proxy.shared_path().to_path_buf());
        let launch_error = if config.start_worker {
            supervisor.ensure_running().err()
        } else {
            None
        };

        Ok(Self {
            descriptor,
            proxy,
            supervisor: Some(supervisor),
            input_channels,
            output_channels,
            launch_error,
            consecutive_block_failures: 0,
            max_consecutive_block_failures: config.max_consecutive_block_failures,
            quarantined: false,
        })
    }

    pub fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    pub fn hosting_plan(&self) -> ExternalPluginHostingPlan {
        plan_external_plugin_hosting(&self.descriptor)
    }

    pub fn placeholder_state(&self) -> ExternalPluginState {
        ExternalPluginState::new(
            self.descriptor.clone(),
            ExternalPluginSandboxMode::Isolated,
            Vec::new(),
        )
    }

    pub fn from_placeholder_state(
        state: &ExternalPluginState,
        sample_rate: u32,
        config: IsolatedExternalPluginConfig,
    ) -> Result<Self, String> {
        state.validate_descriptor_consistency()?;
        if state.sandbox_mode != ExternalPluginSandboxMode::Isolated {
            return Err(format!(
                "External plugin state sandbox mode {:?} cannot restore isolated plugin",
                state.sandbox_mode
            ));
        }
        Self::new(state.descriptor.clone(), sample_rate, config)
    }

    pub fn launch_error(&self) -> Option<&str> {
        self.launch_error.as_deref()
    }

    pub fn ensure_worker_running(&mut self) -> Result<(), String> {
        self.ensure_worker_running_event().map(|_| ())
    }

    pub fn ensure_worker_running_event(&mut self) -> Result<ExternalPluginProcessEvent, String> {
        if self.quarantined {
            return Err(self.launch_error.clone().unwrap_or_else(|| {
                format!(
                    "isolated external plugin '{}' worker is quarantined",
                    self.descriptor.name
                )
            }));
        }
        let Some(supervisor) = self.supervisor.as_mut() else {
            return Ok(ExternalPluginProcessEvent::NotRunning);
        };
        supervisor.ensure_running().inspect_err(|err| {
            self.launch_error = Some(err.clone());
        })
    }

    pub fn poll_worker(&mut self) -> Result<Option<ExternalPluginProcessEvent>, String> {
        let Some(supervisor) = self.supervisor.as_mut() else {
            return Ok(Some(ExternalPluginProcessEvent::NotRunning));
        };
        supervisor.poll()
    }

    pub fn worker_start_count(&self) -> u64 {
        self.supervisor
            .as_ref()
            .map_or(0, ExternalPluginProcessSupervisor::start_count)
    }

    pub fn worker_exit_count(&self) -> u64 {
        self.supervisor
            .as_ref()
            .map_or(0, ExternalPluginProcessSupervisor::exit_count)
    }

    pub fn worker_launch_failure_count(&self) -> u64 {
        self.supervisor
            .as_ref()
            .map_or(0, ExternalPluginProcessSupervisor::launch_failure_count)
    }

    pub fn block_timeout_count(&self) -> u64 {
        self.proxy.timeout_count()
    }

    pub fn block_worker_failure_count(&self) -> u64 {
        self.proxy.worker_failure_count()
    }

    pub fn block_wrong_sequence_count(&self) -> u64 {
        self.proxy.wrong_sequence_count()
    }

    pub fn worker_sandbox_status(&self) -> PluginSandboxRuntimeStatus {
        self.proxy.worker_sandbox_status()
    }

    fn validate_process_buffers(
        &self,
        input: &[f32],
        output: &[f32],
        frames: usize,
    ) -> Result<(), String> {
        let expected_input = frames
            .checked_mul(self.input_channels)
            .ok_or_else(|| "isolated external plugin input length overflow".to_string())?;
        let expected_output = frames
            .checked_mul(self.output_channels)
            .ok_or_else(|| "isolated external plugin output length overflow".to_string())?;
        if input.len() < expected_input {
            return Err(format!(
                "isolated external plugin '{}' received {} input samples but expected at least {expected_input}",
                self.descriptor.name,
                input.len()
            ));
        }
        if output.len() < expected_output {
            return Err(format!(
                "isolated external plugin '{}' received {} output samples but expected at least {expected_output}",
                self.descriptor.name,
                output.len()
            ));
        }
        Ok(())
    }

    fn write_fallback(&self, input: &[f32], output: &mut [f32], frames: usize) -> usize {
        let output_len = frames
            .saturating_mul(self.output_channels)
            .min(output.len());
        output[..output_len].fill(0.0);

        if self.input_channels == 0 || self.output_channels == 0 {
            return frames;
        }

        let copy_channels = self.input_channels.min(self.output_channels);
        for frame in 0..frames {
            let src_base = frame.saturating_mul(self.input_channels);
            let dst_base = frame.saturating_mul(self.output_channels);
            if src_base >= input.len() || dst_base >= output_len {
                break;
            }

            let src_end = (src_base + copy_channels).min(input.len());
            let dst_end = (dst_base + copy_channels).min(output_len);
            let copied = (src_end - src_base).min(dst_end - dst_base);
            output[dst_base..dst_base + copied]
                .copy_from_slice(&input[src_base..src_base + copied]);
        }

        frames
    }

    fn record_block_status(&mut self, status: ExternalPluginHostBlockStatus) {
        if matches!(status, ExternalPluginHostBlockStatus::Processed) {
            self.consecutive_block_failures = 0;
            return;
        }

        self.consecutive_block_failures = self.consecutive_block_failures.saturating_add(1);
        if self.max_consecutive_block_failures > 0
            && self.consecutive_block_failures >= self.max_consecutive_block_failures
        {
            self.quarantine_worker(format!(
                "isolated external plugin '{}' worker quarantined after {} consecutive block failures",
                self.descriptor.name, self.consecutive_block_failures
            ));
        }
    }

    fn quarantine_worker(&mut self, reason: String) {
        if self.quarantined {
            return;
        }
        if let Some(supervisor) = self.supervisor.as_mut() {
            let _ = supervisor.terminate();
        }
        self.launch_error = Some(reason.clone());
        self.quarantined = true;
        crate::rate_limited_log!(warn, 1, "{reason}");
    }
}

fn build_worker_launch_command(
    config: &IsolatedExternalPluginConfig,
    descriptor_json: String,
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

    Ok(command
        .arg("--descriptor-json")
        .arg(descriptor_json)
        .args(sandbox_args))
}

fn decorate_sandbox_launcher_command(
    mut launcher: ExternalPluginWorkerCommand,
    worker: &ExternalPluginWorkerCommand,
) -> ExternalPluginWorkerCommand {
    launcher = launcher
        .arg("--sandbox-worker-binary")
        .arg(worker.program().display().to_string());

    for arg in worker.command_args() {
        launcher = launcher.arg("--sandbox-worker-arg").arg(arg.clone());
    }
    for (key, value) in worker.command_env() {
        launcher = launcher
            .arg("--sandbox-worker-env")
            .arg(format!("{key}={value}"));
    }

    launcher
}

impl Plugin for IsolatedExternalPlugin {
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }

    fn info(&self) -> PluginInfo {
        PluginInfo::new(
            &self.descriptor.name,
            &self.descriptor.version,
            &self.descriptor.vendor,
        )
    }

    fn input_channels(&self) -> usize {
        self.input_channels
    }

    fn output_channels(&self) -> usize {
        self.output_channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        Vec::new()
    }

    fn set_parameter(&mut self, id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        Err(format!(
            "isolated external plugin '{}' does not expose parameter '{id}' yet",
            self.descriptor.name
        ))
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        self.validate_process_buffers(input, output, context.num_frames)?;
        if self.quarantined {
            return Ok(self.write_fallback(input, output, context.num_frames));
        }

        let (frames, status) = self
            .proxy
            .process_block(input, output, context.num_frames)
            .map_err(|err| format!("isolated external plugin processing failed: {err}"))?;
        self.record_block_status(status);

        match status {
            ExternalPluginHostBlockStatus::Processed => {}
            ExternalPluginHostBlockStatus::TimedOut => {
                crate::rate_limited_log!(
                    warn,
                    5,
                    "isolated external plugin '{}' missed block deadline; using passthrough",
                    self.descriptor.name
                );
            }
            ExternalPluginHostBlockStatus::WorkerFailed => {
                crate::rate_limited_log!(
                    warn,
                    5,
                    "isolated external plugin '{}' worker failed; using passthrough",
                    self.descriptor.name
                );
            }
            ExternalPluginHostBlockStatus::WrongSequence => {
                crate::rate_limited_log!(
                    warn,
                    5,
                    "isolated external plugin '{}' returned stale block; using passthrough",
                    self.descriptor.name
                );
            }
        }

        Ok(frames)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
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

        assert!(plugin.launch_error().is_some());
        assert!(plugin.ensure_worker_running_event().is_err());
        output.fill(0.0);
        plugin
            .process(&input, &mut output, &ProcessContext::new(48_000, 2))
            .unwrap();
        assert_eq!(output, input);
    }

    #[test]
    fn isolated_external_plugin_placeholder_state_round_trips() {
        let plugin = IsolatedExternalPlugin::new(
            descriptor(),
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
            build_worker_launch_command(&config, "{\"id\":\"test\"}".to_string(), Vec::new())
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
}
