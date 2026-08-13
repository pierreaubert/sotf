use super::isolated_external_plugin_config::IsolatedExternalPluginConfig;
use super::isolated_external_plugin_config::build_worker_launch_command;
use crate::external_plugin::{
    ExternalPluginHostingPlan, ExternalPluginSandboxMode, ExternalPluginState, PluginDescriptor,
    plan_external_plugin_hosting,
};
use crate::external_plugin_host::{ExternalPluginHostBlockStatus, ExternalPluginHostProxy};
use crate::external_plugin_ipc::{PluginIpcControlRequest, PluginIpcControlResponse};
use crate::external_plugin_ipc::{PluginIpcLayout, PluginSandboxRuntimeStatus};
use crate::external_plugin_process::{ExternalPluginProcessEvent, ExternalPluginProcessSupervisor};
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use crate::plugin::{
    Plugin, PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub struct IsolatedExternalPlugin {
    pub(super) descriptor: PluginDescriptor,
    pub(super) plugin_instance_id: Option<usize>,
    pub(super) proxy: ExternalPluginHostProxy,
    pub(super) supervisor: Option<ExternalPluginProcessSupervisor>,
    pub(super) input_channels: usize,
    pub(super) output_channels: usize,
    pub(super) latency_samples: usize,
    pub(super) launch_error: Option<String>,
    pub(super) consecutive_block_failures: u32,
    pub(super) max_consecutive_block_failures: u32,
    pub(super) quarantined: bool,
    pub(super) quarantine_reason: Option<String>,
    pub(super) opaque_state: Vec<u8>,
    pub(super) state_file_path: Option<PathBuf>,
    pub(super) parameters: Vec<Parameter>,
    pub(super) parameter_values: HashMap<ParameterId, ParameterValue>,
    pub(super) control_timeout: Duration,
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
        if descriptor.audio_outputs == 0 {
            return Err(format!(
                "isolated external plugin '{}' has unprobed channel metadata; probe the native plugin before allocating IPC",
                descriptor.name
            ));
        }
        if let Some(state) = config.initial_state.as_ref() {
            state.validate()?;
            if state.descriptor != descriptor {
                return Err(format!(
                    "External plugin state targets '{}' at {}, not '{}' at {}",
                    state.descriptor.id,
                    state.descriptor.path.display(),
                    descriptor.id,
                    descriptor.path.display()
                ));
            }
            if state.sandbox_mode != ExternalPluginSandboxMode::Isolated {
                return Err(format!(
                    "External plugin state sandbox mode {:?} cannot restore isolated plugin",
                    state.sandbox_mode
                ));
            }
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
        let path = proxy.shared_path().with_extension("state.json");
        let launch_state = config.initial_state.clone().unwrap_or_else(|| {
            ExternalPluginState::new(
                descriptor.clone(),
                ExternalPluginSandboxMode::Isolated,
                Vec::new(),
            )
        });
        write_initial_state_file(&path, &launch_state)?;
        let state_file_path = Some(path);
        let sandbox_args = match &config.capability_sandbox_policy {
            Some(policy) => policy.command_args_for_backend(config.sandbox_launch_backend)?,
            None => config.sandbox_policy.command_args(),
        };
        let command = build_worker_launch_command(
            &config,
            descriptor_json,
            state_file_path.as_deref(),
            sandbox_args,
        )?;
        let mut supervisor =
            ExternalPluginProcessSupervisor::new(command, proxy.shared_path().to_path_buf())?;
        let launch_error = if config.start_worker {
            supervisor.ensure_running().err()
        } else {
            None
        };
        let quarantine_reason = format!(
            "isolated external plugin '{}' worker quarantined after {} consecutive block failures",
            descriptor.name, config.max_consecutive_block_failures
        );

        let mut plugin = Self {
            descriptor,
            plugin_instance_id: config.plugin_instance_id,
            proxy,
            supervisor: Some(supervisor),
            input_channels,
            output_channels,
            latency_samples: 0,
            quarantined: launch_error.is_some(),
            launch_error,
            consecutive_block_failures: 0,
            max_consecutive_block_failures: config.max_consecutive_block_failures,
            quarantine_reason: Some(quarantine_reason),
            opaque_state: config
                .initial_state
                .as_ref()
                .map(|state| state.opaque_state.clone())
                .unwrap_or_default(),
            state_file_path,
            parameters: Vec::new(),
            parameter_values: HashMap::new(),
            control_timeout: config.worker_startup_timeout,
        };

        if let Some(error) = plugin.launch_error.take() {
            if let Some(supervisor) = plugin.supervisor.as_mut() {
                let _ = supervisor.terminate();
            }
            return Err(format!(
                "failed to launch isolated external plugin '{}': {error}",
                plugin.descriptor.name
            ));
        }

        if config.start_worker
            && let Err(mut error) = plugin
                .wait_for_worker_latency_metadata(config.worker_startup_timeout)
                .and_then(|latency| {
                    plugin.proxy.configure_fallback_latency(latency)?;
                    plugin.latency_samples = latency;
                    Ok(latency)
                })
        {
            if let Some(supervisor) = plugin.supervisor.as_mut() {
                if let Ok(Some(ExternalPluginProcessEvent::Exited { status })) = supervisor.poll() {
                    let detail = supervisor
                        .last_stderr()
                        .map(|stderr| format!(": {stderr}"))
                        .unwrap_or_default();
                    error = format!(
                        "isolated external plugin '{}' worker exited with {status} before publishing latency metadata{detail}",
                        plugin.descriptor.name,
                    );
                }
                let _ = supervisor.terminate();
            }
            return Err(error);
        }

        if config.start_worker {
            match plugin.proxy.request_control(
                &PluginIpcControlRequest::Describe,
                config.worker_startup_timeout,
            )? {
                PluginIpcControlResponse::Description { parameters } => {
                    plugin.parameter_values = parameters
                        .iter()
                        .map(|parameter| (parameter.id.clone(), parameter.default_value.clone()))
                        .collect();
                    plugin.proxy.configure_parameters(
                        parameters
                            .iter()
                            .map(|parameter| parameter.id.clone())
                            .collect(),
                    );
                    plugin.parameters = parameters;
                }
                PluginIpcControlResponse::Error(error) => return Err(error),
                _ => return Err("external-plugin worker returned invalid description".to_string()),
            }
        }

        Ok(plugin)
    }

    pub fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    pub fn plugin_instance_id(&self) -> Option<usize> {
        self.plugin_instance_id
    }

    pub fn hosting_plan(&self) -> ExternalPluginHostingPlan {
        plan_external_plugin_hosting(&self.descriptor)
    }

    pub fn placeholder_state(&self) -> ExternalPluginState {
        ExternalPluginState::new(
            self.descriptor.clone(),
            ExternalPluginSandboxMode::Isolated,
            self.opaque_state.clone(),
        )
    }

    pub fn capture_worker_state(&mut self) -> Result<ExternalPluginState, String> {
        let worker_running = match self.supervisor.as_mut() {
            Some(supervisor) => supervisor.is_running()?,
            None => false,
        };
        if worker_running {
            match self
                .proxy
                .request_control(&PluginIpcControlRequest::SaveState, self.control_timeout)?
            {
                PluginIpcControlResponse::State(state) => self.opaque_state = state,
                PluginIpcControlResponse::Error(error) => return Err(error),
                _ => return Err("external-plugin worker returned invalid state response".into()),
            }
        }
        let state = self.placeholder_state();
        if let Some(path) = self.state_file_path.as_ref() {
            write_initial_state_file(path, &state)?;
        }
        Ok(state)
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
        let mut config = config;
        config.initial_state = Some(state.clone());
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

    pub fn worker_reported_latency_samples(&self) -> Option<usize> {
        self.proxy.worker_latency_samples()
    }

    /// Wait for immutable worker metadata on the control/build thread.
    ///
    /// `DawHost::build` caches both total latency and compensation delays, so
    /// construction must not return a running plugin whose latency is still
    /// unknown. This bounded wait never runs from `Plugin::process`.
    pub(super) fn wait_for_worker_latency_metadata(
        &self,
        timeout: Duration,
    ) -> Result<usize, String> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| "external plugin worker startup timeout is too large".to_string())?;
        loop {
            if let Some(latency_samples) = self.worker_reported_latency_samples() {
                return Ok(latency_samples);
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(format!(
                    "isolated external plugin '{}' worker did not publish latency metadata within {} ms",
                    self.descriptor.name,
                    timeout.as_millis(),
                ));
            }
            std::thread::sleep((deadline - now).min(Duration::from_millis(1)));
        }
    }

    pub(super) fn validate_process_buffers(
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

    pub(super) fn write_fallback(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        frames: usize,
    ) -> usize {
        self.proxy.process_fallback(input, output, frames)
    }

    pub(super) fn record_block_status(&mut self, status: ExternalPluginHostBlockStatus) {
        if matches!(status, ExternalPluginHostBlockStatus::Processed) {
            self.consecutive_block_failures = 0;
            return;
        }

        self.consecutive_block_failures = self.consecutive_block_failures.saturating_add(1);
        if self.max_consecutive_block_failures > 0
            && self.consecutive_block_failures >= self.max_consecutive_block_failures
            && let Some(reason) = self.quarantine_reason.take()
        {
            self.quarantine_worker(reason);
        }
    }

    pub(super) fn quarantine_worker(&mut self, reason: String) {
        if self.quarantined {
            return;
        }
        if let Some(supervisor) = self.supervisor.as_ref() {
            let _ = supervisor.request_terminate();
        }
        self.launch_error = Some(reason);
        self.quarantined = true;
        if let Some(reason) = self.launch_error.as_deref() {
            crate::rate_limited_log!(warn, 1, "{reason}");
        }
    }
}

fn write_initial_state_file(path: &Path, state: &ExternalPluginState) -> Result<(), String> {
    let bytes = serde_json::to_vec(state)
        .map_err(|error| format!("failed to serialize external plugin state: {error}"))?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("failed to create external plugin state file: {error}"))?;
    file.write_all(&bytes)
        .map_err(|error| format!("failed to write external plugin state file: {error}"))
}

impl Drop for IsolatedExternalPlugin {
    fn drop(&mut self) {
        if let Some(supervisor) = self.supervisor.as_mut() {
            let _ = supervisor.terminate();
        }
        if let Some(path) = self.state_file_path.as_ref() {
            let _ = std::fs::remove_file(path);
        }
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

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::External
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::boundary(PluginCostClass::External, self.latency_samples())
    }

    fn input_channels(&self) -> usize {
        self.input_channels
    }

    fn output_channels(&self) -> usize {
        self.output_channels
    }

    fn latency_samples(&self) -> usize {
        self.latency_samples
            .max(self.worker_reported_latency_samples().unwrap_or(0))
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let parameter = self
            .parameters
            .iter()
            .find(|parameter| parameter.id == id)
            .ok_or_else(|| {
                format!(
                    "isolated external plugin '{}' has no parameter '{id}'",
                    self.descriptor.name
                )
            })?;
        parameter.validate(&value)?;
        match self.proxy.request_control(
            &PluginIpcControlRequest::Set {
                id: id.clone(),
                value: value.clone(),
            },
            self.control_timeout,
        )? {
            PluginIpcControlResponse::Ack => {
                self.parameter_values.insert(id, value);
                Ok(())
            }
            PluginIpcControlResponse::Error(error) => Err(error),
            _ => Err("external-plugin worker returned invalid parameter response".into()),
        }
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        self.parameter_values.get(id).cloned()
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
            .process_block_with_context(input, output, context)
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
