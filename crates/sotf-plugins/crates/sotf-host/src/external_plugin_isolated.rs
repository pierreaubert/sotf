//! Plugin wrapper that routes an external plugin through an isolated worker.
//!
//! This implements the normal [`Plugin`] trait while keeping unknown plugin
//! execution in a worker process. The audio callback path only publishes a block
//! to shared memory and consumes the worker result; restart decisions remain on
//! the owner/control side through the process supervisor.

use std::time::Duration;

use crate::external_plugin::PluginDescriptor;
use crate::external_plugin_host::{ExternalPluginHostBlockStatus, ExternalPluginHostProxy};
use crate::external_plugin_ipc::{PluginIpcLayout, PluginSandboxRuntimeStatus};
use crate::external_plugin_process::{
    ExternalPluginProcessEvent, ExternalPluginProcessSupervisor, ExternalPluginWorkerCommand,
};
use crate::external_plugin_sandbox::ExternalPluginSandboxPolicy;
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use crate::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};

const DEFAULT_MAX_BLOCK_FRAMES: u32 = 8192;
const DEFAULT_DEADLINE_MICROS: u64 = 2_000;

#[derive(Debug, Clone)]
pub struct IsolatedExternalPluginConfig {
    pub max_block_frames: u32,
    pub deadline: Duration,
    pub worker_command: ExternalPluginWorkerCommand,
    pub sandbox_policy: ExternalPluginSandboxPolicy,
    pub start_worker: bool,
}

impl Default for IsolatedExternalPluginConfig {
    fn default() -> Self {
        Self {
            max_block_frames: DEFAULT_MAX_BLOCK_FRAMES,
            deadline: Duration::from_micros(DEFAULT_DEADLINE_MICROS),
            worker_command: ExternalPluginWorkerCommand::default_worker_binary(),
            sandbox_policy: ExternalPluginSandboxPolicy::default(),
            start_worker: true,
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
        let command = config
            .worker_command
            .clone()
            .arg("--descriptor-json")
            .arg(descriptor_json)
            .args(config.sandbox_policy.command_args());
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
        })
    }

    pub fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    pub fn launch_error(&self) -> Option<&str> {
        self.launch_error.as_deref()
    }

    pub fn ensure_worker_running(&mut self) -> Result<(), String> {
        self.ensure_worker_running_event().map(|_| ())
    }

    pub fn ensure_worker_running_event(&mut self) -> Result<ExternalPluginProcessEvent, String> {
        let Some(supervisor) = self.supervisor.as_mut() else {
            return Ok(ExternalPluginProcessEvent::NotRunning);
        };
        supervisor.ensure_running().map_err(|err| {
            self.launch_error = Some(err.clone());
            err
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
        let (frames, status) = self
            .proxy
            .process_block(input, output, context.num_frames)
            .map_err(|err| format!("isolated external plugin processing failed: {err}"))?;

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
}
