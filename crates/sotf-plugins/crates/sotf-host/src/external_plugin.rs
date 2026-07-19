use crate::error::PluginError;
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use crate::plugin::{
    Plugin, PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use crate::serialization::{PluginPreset, SerializablePlugin};
use std::collections::HashMap;

/// Reserved engine-config parameter used to correlate a hosted worker with
/// the persisted player plugin instance that created it.
pub const EXTERNAL_PLUGIN_INSTANCE_ID_PARAMETER: &str = "_sotf_instance_id";
#[cfg(all(feature = "external-plugin-au", target_os = "macos"))]
mod au_backend;
#[cfg(feature = "external-plugin-clap")]
mod clap_backend;
mod external_hosting_backend;
mod external_plugin_state;
mod format;
mod load;
mod misc;
mod native_backend;
mod plugin;
mod plugin_descriptor;
mod plugin_format;
mod plugin_scan_summary;
mod plugin_scanner;
#[cfg(test)]
mod tests;
mod types;
#[cfg(feature = "external-plugin-vst3")]
mod vst3_backend;

pub use external_hosting_backend::*;
pub use external_plugin_state::*;
pub use misc::*;
pub use plugin::*;
pub use plugin_descriptor::*;
pub use plugin_format::*;
pub use plugin_scan_summary::*;
pub use plugin_scanner::*;
pub use types::*;

use external_hosting_backend::try_load_dynamic_backend;
use native_backend::NativeExternalPluginBackend;

pub struct ExternalPlugin {
    descriptor: PluginDescriptor,
    input_channels: usize,
    output_channels: usize,
    sample_rate: u32,
    parameters: Vec<Parameter>,
    hosting_backend: ExternalHostingBackend,
    restore_error: Option<String>,
    opaque_state: Vec<u8>,
    native_backend: Option<Box<dyn NativeExternalPluginBackend>>,
}

impl ExternalPlugin {
    /// Create a new external plugin wrapper from a descriptor.
    ///
    /// Native backend selection is feature-gated by format:
    /// - CLAP: `external-plugin-clap`
    /// - VST3: `external-plugin-vst3`
    /// - AU: `external-plugin-au`
    ///
    /// If a backend is unavailable at compile-time, we fall back to
    /// deterministic passthrough behavior.
    pub fn new(descriptor: &PluginDescriptor, sample_rate: u32) -> Result<Self, String> {
        if sample_rate == 0 {
            return Err("sample rate must be positive".into());
        }

        descriptor.validate()?;
        let hosting_plan = plan_external_plugin_hosting(descriptor);
        let native_backend =
            try_load_dynamic_backend(descriptor, hosting_plan.backend, sample_rate)?;
        let mut resolved_descriptor = descriptor.clone();
        let (input_channels, output_channels) = native_backend
            .as_ref()
            .map(|backend| {
                let metadata = backend.metadata();
                resolved_descriptor.id.clone_from(&metadata.id);
                resolved_descriptor.name.clone_from(&metadata.name);
                resolved_descriptor.vendor.clone_from(&metadata.vendor);
                resolved_descriptor.version.clone_from(&metadata.version);
                resolved_descriptor.audio_inputs = metadata.input_channels;
                resolved_descriptor.audio_outputs = metadata.output_channels;
                (metadata.input_channels, metadata.output_channels)
            })
            .unwrap_or((descriptor.audio_inputs, descriptor.audio_outputs.max(1)));
        let parameters = native_backend
            .as_ref()
            .map_or_else(Vec::new, |backend| backend.parameters());

        Ok(Self {
            descriptor: resolved_descriptor,
            input_channels,
            output_channels,
            sample_rate,
            parameters,
            hosting_backend: hosting_plan.backend,
            restore_error: None,
            opaque_state: Vec::new(),
            native_backend,
        })
    }

    /// Get the plugin descriptor.
    pub fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    pub fn hosting_backend(&self) -> ExternalHostingBackend {
        self.hosting_backend
    }

    pub fn hosting_plan(&self) -> ExternalPluginHostingPlan {
        plan_external_plugin_hosting(&self.descriptor)
    }

    pub fn restore_error(&self) -> Option<&str> {
        self.restore_error.as_deref()
    }

    /// Serialize descriptor and placeholder state for project/preset storage.
    pub fn placeholder_state(&self) -> ExternalPluginState {
        ExternalPluginState::new(
            self.descriptor.clone(),
            ExternalPluginSandboxMode::InProcess,
            self.opaque_state.clone(),
        )
    }

    /// Recreate an external plugin wrapper from a serialized placeholder state.
    pub fn from_placeholder_state(
        state: &ExternalPluginState,
        sample_rate: u32,
    ) -> Result<Self, String> {
        if sample_rate == 0 {
            return Err("sample rate must be positive".into());
        }
        state.validate_descriptor_consistency()?;
        if state.sandbox_mode != ExternalPluginSandboxMode::InProcess {
            return Err(format!(
                "External plugin state sandbox mode {:?} cannot restore in-process plugin",
                state.sandbox_mode
            ));
        }
        let mut plugin = match Self::new(&state.descriptor, sample_rate) {
            Ok(plugin) => plugin,
            Err(err) => Self::unavailable_placeholder(state.descriptor.clone(), sample_rate, err),
        };
        plugin.opaque_state = state.opaque_state.clone();
        if let Some(backend) = plugin.native_backend.as_mut()
            && let Err(error) = backend.load_state(&state.opaque_state)
        {
            let mut unavailable =
                Self::unavailable_placeholder(state.descriptor.clone(), sample_rate, error);
            unavailable.opaque_state = state.opaque_state.clone();
            return Ok(unavailable);
        }
        Ok(plugin)
    }

    fn unavailable_placeholder(
        descriptor: PluginDescriptor,
        sample_rate: u32,
        restore_error: String,
    ) -> Self {
        Self {
            input_channels: descriptor.audio_inputs,
            output_channels: descriptor.audio_outputs.max(1),
            descriptor,
            sample_rate,
            parameters: Vec::new(),
            hosting_backend: ExternalHostingBackend::Passthrough,
            restore_error: Some(restore_error),
            opaque_state: Vec::new(),
            native_backend: None,
        }
    }

    pub fn to_placeholder_preset(
        &self,
        name: impl Into<String>,
    ) -> Result<PluginPreset, PluginError> {
        let mut preset = PluginPreset::new(
            name.into(),
            EXTERNAL_PLUGIN_PRESET_ID.to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        );
        let opaque_state = match self.native_backend.as_ref() {
            Some(backend) => backend.save_state().map_err(|error| {
                PluginError::InvalidConfiguration(format!(
                    "failed to save external plugin '{}': {error}",
                    self.descriptor.name
                ))
            })?,
            None => None,
        }
        .unwrap_or_else(|| self.opaque_state.clone());
        preset.set_external_plugin_state(&ExternalPluginState::new(
            self.descriptor.clone(),
            ExternalPluginSandboxMode::InProcess,
            opaque_state,
        ))?;
        Ok(preset)
    }

    fn expected_input_len(&self, ctx: &ProcessContext) -> usize {
        ctx.num_frames.saturating_mul(self.input_channels)
    }

    fn expected_output_len(&self, ctx: &ProcessContext) -> usize {
        ctx.num_frames.saturating_mul(self.output_channels)
    }

    fn process_passthrough(
        &self,
        input: &[f32],
        output: &mut [f32],
        ctx: &ProcessContext,
    ) -> usize {
        for sample in output.iter_mut().take(self.expected_output_len(ctx)) {
            *sample = 0.0;
        }

        if self.input_channels == 0 {
            return ctx.num_frames;
        }

        let copy_channels = self.output_channels.min(self.input_channels);
        for frame in 0..ctx.num_frames {
            let src_base = frame.saturating_mul(self.input_channels);
            let dst_base = frame.saturating_mul(self.output_channels);
            output[dst_base..dst_base + copy_channels]
                .copy_from_slice(&input[src_base..src_base + copy_channels]);
        }
        ctx.num_frames
    }

    fn process_native(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        ctx: &ProcessContext,
    ) -> Result<usize, String> {
        let backend = self.native_backend.as_mut().ok_or_else(|| {
            format!(
                "external plugin '{}' selected {:?} hosting without a native instance",
                self.descriptor.name, self.hosting_backend
            )
        })?;
        backend.process(
            input,
            output,
            ctx.num_frames,
            self.input_channels,
            self.output_channels,
        )?;
        Ok(ctx.num_frames)
    }
}

impl Plugin for ExternalPlugin {
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

    fn latency_samples(&self) -> usize {
        self.native_backend
            .as_ref()
            .map_or(0, |backend| backend.latency_samples())
    }

    fn input_channels(&self) -> usize {
        self.input_channels
    }

    fn output_channels(&self) -> usize {
        self.output_channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let parameter = self.parameters.iter().find(|parameter| parameter.id == id);
        let Some(parameter) = parameter else {
            return Err(format!(
                "parameter '{id}' is not exposed by external plugin '{}'",
                self.descriptor.name
            ));
        };
        parameter.validate(&value)?;
        self.native_backend
            .as_mut()
            .ok_or_else(|| {
                format!(
                    "parameter '{id}' cannot be changed because external plugin '{}' is unavailable",
                    self.descriptor.name
                )
            })?
            .set_parameter(&id, &value)
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        self.native_backend.as_ref()?.get_parameter(id)
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        ctx: &ProcessContext,
    ) -> PluginResult<usize> {
        let expected_input = self.expected_input_len(ctx);
        let expected_output = self.expected_output_len(ctx);

        if input.len() < expected_input {
            return Err(format!(
                "external plugin '{}' received {} input samples but expected {expected_input} ({} channels x {} frames)",
                self.descriptor.name,
                input.len(),
                self.input_channels,
                ctx.num_frames
            ));
        }
        if output.len() < expected_output {
            return Err(format!(
                "external plugin '{}' received {} output samples but expected {expected_output} ({} channels x {} frames)",
                self.descriptor.name,
                output.len(),
                self.output_channels,
                ctx.num_frames
            ));
        }

        match self.hosting_backend {
            ExternalHostingBackend::Passthrough => Ok(self.process_passthrough(input, output, ctx)),
            ExternalHostingBackend::Clap
            | ExternalHostingBackend::Vst3
            | ExternalHostingBackend::AudioUnit => self.process_native(input, output, ctx),
        }
    }
}

impl SerializablePlugin for ExternalPlugin {
    fn serialize(&self) -> Result<PluginPreset, PluginError> {
        self.to_placeholder_preset(self.descriptor.name.clone())
    }

    fn deserialize(&mut self, preset: &PluginPreset) -> Result<(), PluginError> {
        if !preset.is_compatible(EXTERNAL_PLUGIN_PRESET_ID) {
            return Err(PluginError::InvalidConfiguration(format!(
                "external plugin preset expected plugin_id '{}', got '{}'",
                EXTERNAL_PLUGIN_PRESET_ID, preset.plugin_id
            )));
        }

        self.parameters_from_map(&preset.parameters)?;

        let state = preset.external_plugin_state()?.ok_or_else(|| {
            PluginError::InvalidConfiguration(
                "external plugin preset is missing external plugin state".to_string(),
            )
        })?;
        if state.sandbox_mode != ExternalPluginSandboxMode::InProcess {
            return Err(PluginError::InvalidConfiguration(format!(
                "external plugin preset sandbox mode {:?} cannot restore in-process plugin",
                state.sandbox_mode
            )));
        }
        if state.format != self.descriptor.format
            || state.plugin_id != self.descriptor.id
            || state.plugin_path != self.descriptor.path
        {
            return Err(PluginError::InvalidConfiguration(format!(
                "external plugin preset targets '{}' at {}, not '{}' at {}",
                state.plugin_id,
                state.plugin_path.display(),
                self.descriptor.id,
                self.descriptor.path.display()
            )));
        }

        let replacement =
            try_load_dynamic_backend(&self.descriptor, self.hosting_backend, self.sample_rate)?;
        if let Some(mut replacement) = replacement {
            replacement.load_state(&state.opaque_state)?;
            self.native_backend = Some(replacement);
        }
        self.opaque_state = state.opaque_state;

        Ok(())
    }

    fn parameters_to_map(&self) -> HashMap<String, ParameterValue> {
        HashMap::new()
    }

    fn parameters_from_map(
        &mut self,
        params: &HashMap<String, ParameterValue>,
    ) -> Result<(), PluginError> {
        if params.is_empty() {
            Ok(())
        } else {
            Err(PluginError::InvalidConfiguration(
                "external plugin placeholder presets do not store host-side parameters".to_string(),
            ))
        }
    }
}
