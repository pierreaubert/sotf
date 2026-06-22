use super::misc::interleaved_to_planar;
use super::misc::planar_to_interleaved;
use super::oversampler::Oversampler;
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use crate::plugin::{Plugin, PluginCompileMetadata, PluginInfo, PluginResult, ProcessContext};
use std::any::Any;
use std::sync::Arc;

/// Runtime wrapper used by `DawHost` for `Plugin::preferred_oversampling()`.
///
/// The generic `OversampledPlugin<P>` remains the zero-cost wrapper for
/// concrete `InPlacePlugin` types. This dyn wrapper lets the host honor
/// oversampling preferences for already-erased `Box<dyn Plugin>` values.
pub struct AutoOversampledPlugin {
    pub(super) inner: Box<dyn Plugin>,
    pub(super) oversampler: Oversampler,
    pub(super) factor: u32,
    pub(super) channels: usize,
    pub(super) os_input_interleaved: Vec<f32>,
    pub(super) os_interleaved: Vec<f32>,
}

impl AutoOversampledPlugin {
    pub fn new(inner: Box<dyn Plugin>, factor: u32) -> Result<Self, String> {
        let channels = inner.input_channels();
        if channels != inner.output_channels() {
            return Err(format!(
                "Cannot auto-oversample plugin '{}' with mismatched I/O channels ({} -> {})",
                inner.info().name,
                inner.input_channels(),
                inner.output_channels()
            ));
        }
        let oversampler = Oversampler::new(factor, channels)?;
        let os_buf_size = 8192 * factor as usize * channels;
        Ok(Self {
            inner,
            oversampler,
            factor,
            channels,
            os_input_interleaved: vec![0.0; os_buf_size],
            os_interleaved: vec![0.0; os_buf_size],
        })
    }
}

impl Plugin for AutoOversampledPlugin {
    fn info(&self) -> PluginInfo {
        let mut info = self.inner.info();
        info.name = format!("{}({}x)", info.name, self.factor);
        info
    }

    fn input_channels(&self) -> usize {
        self.channels
    }

    fn output_channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.inner.parameters()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.inner.set_parameter(id, value)
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        self.inner.get_parameter(id)
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.inner.initialize(sample_rate * self.factor)?;
        self.oversampler.reset();
        Ok(())
    }

    fn reset(&mut self) {
        self.inner.reset();
        self.oversampler.reset();
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        output[..input.len()].copy_from_slice(input);
        let nf = context.num_frames;
        let nc = self.channels;
        let os_rate = context.sample_rate * self.factor;
        let inner = &mut self.inner;
        let os_input_interleaved = &mut self.os_input_interleaved;
        let os_interleaved = &mut self.os_interleaved;
        let mut inner_error = None;

        self.oversampler
            .process(&mut output[..input.len()], nf, |planar, os_frames| {
                if inner_error.is_some() {
                    return;
                }
                let total_os = os_frames * nc;
                if os_interleaved.capacity() < total_os
                    || os_input_interleaved.capacity() < total_os
                {
                    crate::rate_limited_log!(
                        warn,
                        5,
                        "auto-oversampling: os_interleaved grew from {} to {} on hot path",
                        os_interleaved.capacity(),
                        total_os
                    );
                }
                if os_interleaved.len() < total_os {
                    os_interleaved.resize(total_os, 0.0);
                }
                if os_input_interleaved.len() < total_os {
                    os_input_interleaved.resize(total_os, 0.0);
                }
                planar_to_interleaved(planar, &mut os_input_interleaved[..total_os], os_frames, nc);
                let ctx = ProcessContext::new(os_rate, os_frames);
                match inner.process(
                    &os_input_interleaved[..total_os],
                    &mut os_interleaved[..total_os],
                    &ctx,
                ) {
                    Ok(frames) if frames == os_frames => {}
                    Ok(frames) => {
                        inner_error = Some(format!(
                            "auto-oversampled inner processed {frames} frames, expected {os_frames}"
                        ));
                        return;
                    }
                    Err(err) => {
                        inner_error = Some(err);
                        return;
                    }
                }
                interleaved_to_planar(&os_interleaved[..total_os], planar, os_frames, nc);
            })?;

        if let Some(err) = inner_error {
            return Err(err);
        }
        Ok(nf)
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        let mut metadata = self.inner.compile_metadata();
        metadata.compiled_op = None;
        metadata.static_gain = None;
        metadata.latency_samples = self.latency_samples();
        metadata.boundary = true;
        metadata
    }

    fn latency_samples(&self) -> usize {
        self.oversampler.latency_samples() + self.inner.latency_samples() / self.factor as usize
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        self.inner.get_data()
    }

    fn take_cache_contention_stats(&mut self) -> (u64, u64) {
        self.inner.take_cache_contention_stats()
    }

    fn output_frames_for_input(&self, input_frames: usize) -> usize {
        input_frames
    }

    fn output_sample_rate(&self, input_rate: u32) -> u32 {
        input_rate
    }

    fn preferred_oversampling(&self) -> Option<u32> {
        None
    }

    fn supports_f64(&self) -> bool {
        self.inner.supports_f64()
    }
}
