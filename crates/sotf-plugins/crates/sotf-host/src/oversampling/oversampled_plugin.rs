use super::misc::interleaved_to_planar;
use super::misc::planar_to_interleaved;
use super::oversampler::Oversampler;
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use crate::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use std::any::Any;
use std::sync::Arc;

/// Wraps any `InPlacePlugin` with transparent oversampling.
///
/// The inner plugin processes audio at `factor × sample_rate`. The wrapper
/// handles upsampling before and downsampling after `process_in_place()`.
///
/// This enables any plugin to be oversampled without modifying its internals:
/// ```ignore
/// let saturator = SaturationPlugin::new(2);
/// let oversampled = OversampledPlugin::new(saturator, 4, 2)?; // 4x, stereo
/// ```
pub struct OversampledPlugin<P: InPlacePlugin> {
    pub(super) inner: P,
    pub(super) oversampler: Oversampler,
    pub(super) factor: u32,
    pub(super) channels: usize,
    pub(super) sample_rate: u32,
    /// Pre-allocated interleaved buffer for oversampled processing
    pub(super) os_interleaved: Vec<f32>,
}

impl<P: InPlacePlugin> OversampledPlugin<P> {
    /// Create a new oversampled plugin wrapper.
    ///
    /// `factor` must be 2 or 4. The inner plugin will be initialized at
    /// `sample_rate * factor` when `initialize()` is called.
    pub fn new(inner: P, factor: u32, channels: usize) -> Result<Self, String> {
        let oversampler = Oversampler::new(factor, channels)?;
        // Pre-allocate for max expected oversampled block: 8192 * factor * channels
        let os_buf_size = 8192 * factor as usize * channels;
        Ok(Self {
            inner,
            oversampler,
            factor,
            channels,
            sample_rate: 48000,
            os_interleaved: vec![0.0; os_buf_size],
        })
    }

    /// Access the inner plugin.
    pub fn inner(&self) -> &P {
        &self.inner
    }

    /// Mutably access the inner plugin.
    pub fn inner_mut(&mut self) -> &mut P {
        &mut self.inner
    }
}

impl<P: InPlacePlugin> InPlacePlugin for OversampledPlugin<P> {
    fn info(&self) -> PluginInfo {
        let mut info = self.inner.info();
        info.name = format!("{}({}x)", info.name, self.factor);
        info
    }

    fn channels(&self) -> usize {
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
        self.sample_rate = sample_rate;
        // Initialize inner plugin at the oversampled rate
        let os_rate = sample_rate * self.factor;
        self.inner.initialize(os_rate)?;
        self.oversampler.reset();
        Ok(())
    }

    fn reset(&mut self) {
        self.inner.reset();
        self.oversampler.reset();
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        let nf = context.num_frames;
        let nc = self.channels;

        // The oversampler's callback receives planar buffers at the OS rate.
        // We need to convert to interleaved, call inner.process_in_place, then
        // convert back to planar.
        let os_context = ProcessContext::new(context.sample_rate * self.factor, 0);

        let inner = &mut self.inner;
        let os_interleaved = &mut self.os_interleaved;
        let mut inner_error: Option<String> = None;

        self.oversampler
            .process(buffer, nf, |planar, os_frames| {
                if inner_error.is_some() {
                    return;
                }
                let total_os = os_frames * nc;
                // Ensure buffer is large enough. A grow here means the pre-
                // allocation in `build()` was too small for this block; log so
                // the offending block size is visible. Allocation on the audio
                // thread is acceptable as a one-shot fallback but not as a
                // steady state.
                if os_interleaved.capacity() < total_os {
                    crate::rate_limited_log!(
                        warn,
                        5,
                        "oversampling: os_interleaved grew from {} to {} on hot path",
                        os_interleaved.capacity(),
                        total_os
                    );
                }
                if os_interleaved.len() < total_os {
                    os_interleaved.resize(total_os, 0.0);
                }
                // Convert planar → interleaved
                planar_to_interleaved(planar, &mut os_interleaved[..total_os], os_frames, nc);
                // Process at oversampled rate
                let ctx = ProcessContext::new(os_context.sample_rate, os_frames);
                match inner.process_in_place(&mut os_interleaved[..total_os], &ctx) {
                    Ok(frames) if frames == os_frames => {}
                    Ok(frames) => {
                        inner_error = Some(format!(
                            "oversampled inner processed {frames} frames, expected {os_frames}"
                        ));
                        return;
                    }
                    Err(err) => {
                        inner_error = Some(err);
                        return;
                    }
                }
                // Convert interleaved → planar (back)
                interleaved_to_planar(&os_interleaved[..total_os], planar, os_frames, nc);
            })
            .map_err(|e| e.to_string())?;

        if let Some(err) = inner_error {
            return Err(err);
        }

        Ok(nf)
    }

    fn latency_samples(&self) -> usize {
        // Oversampler latency + inner plugin's latency (scaled to 1x rate)
        let inner_latency_1x = self.inner.latency_samples() / self.factor as usize;
        self.oversampler.latency_samples() + inner_latency_1x
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        self.inner.get_data()
    }

    fn preferred_oversampling(&self) -> Option<u32> {
        None // Already oversampled — don't request more
    }
}
