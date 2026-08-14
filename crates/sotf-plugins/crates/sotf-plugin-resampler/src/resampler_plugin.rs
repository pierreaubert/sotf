use super::resampler_quality::ResamplerQuality;
use audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{
    Async, FixedAsync, Indexing, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};
use sotf_host::param_specs::UpdateMode;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{
    Plugin, PluginCompileMetadata, PluginCostClass, PluginDrainResult, PluginInfo, PluginResult,
    ProcessContext,
};

/// Resampler plugin using rubato
///
/// This plugin resamples audio from one sample rate to another using high-quality
/// sinc interpolation. It maintains the same number of channels.
///
/// Note: The output buffer size will differ from input size based on the resampling ratio.
/// For example, resampling from 44.1kHz to 48kHz will produce more output frames.
pub struct ResamplerPlugin {
    /// Number of channels
    pub(super) num_channels: usize,
    /// Input sample rate
    pub(super) input_sample_rate: u32,
    /// Output sample rate
    pub(super) output_sample_rate: u32,
    /// Rubato resampler (planar format)
    pub(super) resampler: Option<Async<f32>>,
    /// Chunk size for processing (number of frames per chunk)
    pub(super) chunk_size: usize,
    /// Output buffer (planar: one vec per channel, pre-allocated to max output size)
    pub(super) output_buffer: Vec<Vec<f32>>,
    /// Actual output frames from last process() call
    pub(super) last_output_frames: usize,
    /// Residual input buffer for variable-length input support (planar, per-channel)
    pub(super) residual_input: Vec<Vec<f32>>,
    /// Number of residual frames buffered
    pub(super) residual_frames: usize,
    /// Quality preset
    pub(super) quality: ResamplerQuality,
    /// Whether dynamic ratio changes are enabled
    pub(super) dynamic_ratio: bool,
    /// Current effective ratio (may differ from nominal when dynamic_ratio is enabled)
    pub(super) current_ratio: f64,
    /// Parameter IDs
    pub(super) param_quality: ParameterId,
    pub(super) param_dynamic_ratio: ParameterId,
    pub(super) param_ratio: ParameterId,
    /// Cached parameters
    pub(super) cached_parameters: Vec<Parameter>,
    /// Set after the host negotiates the configured input rate.
    pub(super) initialized: bool,
    /// Programme frames accepted since the last reset.
    pub(super) stream_input_frames: u64,
    /// Raw rubato output frames already exposed, including leading delay.
    pub(super) stream_output_frames: u64,
    /// Cumulative programme duration expressed in output-rate frames.
    pub(super) expected_signal_frames: f64,
    /// Frozen raw-output target once end-of-stream draining begins.
    pub(super) drain_target_frames: Option<u64>,
}

impl ResamplerPlugin {
    /// Minimum queued-work horizon used by the engine scheduler.
    ///
    /// The streaming adapter accepts smaller callback partitions and buffers
    /// them, but sinc work is performed when a complete chunk is assembled.
    /// A queued engine therefore keeps at least this many input-rate frames
    /// ahead of hardware consumption instead of assuming the cost is spread
    /// uniformly over every sub-chunk callback. This is not a longer physical
    /// callback deadline for fixed-rate plugin-format hosts.
    pub fn realtime_quantum_frames(&self) -> usize {
        if self.is_unity_passthrough() {
            1
        } else {
            self.chunk_size
        }
    }

    /// Create a new resampler plugin
    ///
    /// # Arguments
    /// * `num_channels` - Number of audio channels
    /// * `input_sample_rate` - Input sample rate in Hz
    /// * `output_sample_rate` - Output sample rate in Hz
    /// * `chunk_size` - Number of input frames to process at once (default: 1024)
    pub fn new(
        num_channels: usize,
        input_sample_rate: u32,
        output_sample_rate: u32,
        chunk_size: usize,
    ) -> Result<Self, String> {
        Self::with_quality(
            num_channels,
            input_sample_rate,
            output_sample_rate,
            chunk_size,
            ResamplerQuality::Medium,
        )
    }

    /// Create a new resampler plugin with a specified quality preset
    pub fn with_quality(
        num_channels: usize,
        input_sample_rate: u32,
        output_sample_rate: u32,
        chunk_size: usize,
        quality: ResamplerQuality,
    ) -> Result<Self, String> {
        if num_channels == 0 {
            return Err("num_channels must be > 0".to_string());
        }
        if input_sample_rate == 0 || output_sample_rate == 0 {
            return Err("sample rates must be > 0".to_string());
        }
        if chunk_size == 0 {
            return Err("chunk_size must be > 0".to_string());
        }

        let nominal_ratio = output_sample_rate as f64 / input_sample_rate as f64;

        // Create resampler
        let resampler = Self::create_resampler(
            num_channels,
            input_sample_rate,
            output_sample_rate,
            chunk_size,
            quality,
        )?;

        let max_output_frames = resampler.output_frames_max();

        let mut plugin = Self {
            num_channels,
            input_sample_rate,
            output_sample_rate,
            resampler: Some(resampler),
            chunk_size,
            output_buffer: vec![vec![0.0; max_output_frames]; num_channels],
            last_output_frames: 0,
            residual_input: vec![vec![0.0; chunk_size]; num_channels],
            residual_frames: 0,
            quality,
            dynamic_ratio: false,
            current_ratio: nominal_ratio,
            param_quality: ParameterId::from("quality"),
            param_dynamic_ratio: ParameterId::from("dynamic_ratio"),
            param_ratio: ParameterId::from("ratio"),
            cached_parameters: Vec::new(),
            initialized: false,
            stream_input_frames: 0,
            stream_output_frames: 0,
            expected_signal_frames: 0.0,
            drain_target_frames: None,
        };
        plugin.rebuild_cached_parameters();
        Ok(plugin)
    }

    /// Create a new resampler with default chunk size (1024)
    pub fn new_default(
        num_channels: usize,
        input_sample_rate: u32,
        output_sample_rate: u32,
    ) -> Result<Self, String> {
        Self::new(num_channels, input_sample_rate, output_sample_rate, 1024)
    }

    /// Create the rubato resampler with quality-dependent parameters
    pub(super) fn create_resampler(
        num_channels: usize,
        input_sample_rate: u32,
        output_sample_rate: u32,
        chunk_size: usize,
        quality: ResamplerQuality,
    ) -> Result<Async<f32>, String> {
        let params = SincInterpolationParameters {
            sinc_len: quality.sinc_len(),
            f_cutoff: quality.f_cutoff(),
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: quality.oversampling_factor(),
            window: WindowFunction::BlackmanHarris2,
        };

        let resampler = Async::<f32>::new_sinc(
            output_sample_rate as f64 / input_sample_rate as f64,
            2.0, // Maximum relative ratio deviation
            &params,
            chunk_size,
            num_channels,
            FixedAsync::Input,
        )
        .map_err(|e| format!("Failed to create resampler: {:?}", e))?;

        Ok(resampler)
    }

    /// Rebuild the resampler with current quality settings.
    /// Called when quality changes.
    ///
    /// This reuses the pre-allocated `output_buffer` and `residual_input`
    /// rather than creating new `Vec`s, so it is safe to call from a context where heap
    /// allocation is undesirable (though note that rubato's internal `create_resampler`
    /// still allocates the sinc table).  The output frame size depends only on chunk_size
    /// and ratio, not on quality, so the existing buffers remain correctly sized.
    pub(super) fn rebuild_resampler(&mut self) -> Result<(), String> {
        let resampler = Self::create_resampler(
            self.num_channels,
            self.input_sample_rate,
            self.output_sample_rate,
            self.chunk_size,
            self.quality,
        )?;
        // output_frames_max() depends only on chunk_size, ratio, and max_relative_ratio —
        // not on sinc_len or oversampling_factor.  The existing buffers are already sized
        // for this chunk_size/ratio pair, so we reuse them in-place.
        debug_assert_eq!(
            resampler.output_frames_max(),
            self.output_buffer
                .first()
                .map(|v| v.len())
                .unwrap_or(resampler.output_frames_max()),
            "output_frames_max changed on quality rebuild — buffer reuse assumption violated"
        );
        // Zero residual to avoid stale data from the previous resampler.
        self.residual_frames = 0;
        for ch in 0..self.num_channels {
            self.residual_input[ch].fill(0.0);
            self.output_buffer[ch].fill(0.0);
        }
        self.resampler = Some(resampler);
        self.current_ratio = self.output_sample_rate as f64 / self.input_sample_rate as f64;
        Ok(())
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        let nominal_ratio = self.output_sample_rate as f64 / self.input_sample_rate as f64;
        self.cached_parameters = vec![
            Parameter::new_int("quality", "Quality", self.quality.index(), 0, 2)
                .with_update_mode(UpdateMode::Structural)
                .with_description(
                    "Resampling quality: fast (64-tap), medium (128-tap), high (256-tap)",
                ),
            Parameter::new_bool("dynamic_ratio", "Dynamic Ratio", self.dynamic_ratio)
                .with_description("Enable runtime ratio changes without rebuilding"),
            Parameter::new_float(
                "ratio",
                "Ratio",
                self.current_ratio as f32,
                (nominal_ratio / 2.0) as f32,
                (nominal_ratio * 2.0) as f32,
            )
            .with_description(
                "Current resampling ratio (only adjustable when dynamic_ratio is enabled)",
            ),
        ];
    }

    /// Convert planar output to interleaved format
    pub(super) fn planar_to_interleaved(
        planar: &[Vec<f32>],
        output: &mut [f32],
        num_frames: usize,
        num_channels: usize,
    ) {
        for frame in 0..num_frames {
            for ch in 0..num_channels {
                output[frame * num_channels + ch] = planar[ch][frame];
            }
        }
    }

    /// Get the maximum number of output frames for a given number of input frames
    ///
    /// Returns the maximum possible output frame count from rubato.
    /// This should be used for buffer allocation to ensure the buffer is always large enough.
    /// The actual output frame count may be less and is returned by process().
    pub fn output_frames_for_input(&self, input_frames: usize) -> usize {
        if self.is_unity_passthrough() {
            return input_frames;
        }
        // Use rubato's output_frames_max() for safe buffer allocation
        // The actual output varies based on resampler internal state
        if let Some(ref resampler) = self.resampler {
            let pending = self.residual_frames.saturating_add(input_frames);
            let chunks = pending / self.chunk_size;
            chunks.saturating_mul(resampler.output_frames_max())
        } else {
            // Fallback estimate if resampler not initialized
            let ratio = self.output_sample_rate as f64 / self.input_sample_rate as f64;
            (input_frames as f64 * ratio).ceil() as usize + 1
        }
    }

    /// Frames immediately available from complete buffered input chunks.
    /// This is a scheduling estimate, not a destination-capacity bound.
    pub fn available_output_frames(&self, input_frames: usize) -> usize {
        if self.is_unity_passthrough() {
            return input_frames;
        }
        let pending = self.residual_frames.saturating_add(input_frames);
        let chunks = pending / self.chunk_size;
        match (chunks, self.resampler.as_ref()) {
            (0, _) => 0,
            (1, Some(resampler)) => resampler.output_frames_next().saturating_add(4),
            (count, Some(resampler)) => count.saturating_mul(resampler.output_frames_max()),
            _ => 0,
        }
    }

    /// Maximum frames written by one complete-stream drain step.
    pub fn flush_output_frames_max(&self) -> usize {
        if self.stream_input_frames == 0
            || self.is_unity_passthrough()
            || self
                .drain_target_frames
                .is_some_and(|target| self.stream_output_frames >= target)
        {
            0
        } else {
            self.resampler
                .as_ref()
                .map(Resampler::output_frames_max)
                .unwrap_or(0)
        }
    }

    fn is_unity_passthrough(&self) -> bool {
        self.input_sample_rate == self.output_sample_rate && !self.dynamic_ratio
    }

    /// Get the nominal resampling ratio (output_rate / input_rate)
    pub fn ratio(&self) -> f64 {
        self.output_sample_rate as f64 / self.input_sample_rate as f64
    }

    /// Intrinsic output-domain delay of rubato's interpolation filter.
    ///
    /// Offline callers can trim this many leading frames after feeding enough
    /// zero tail to retain the complete time-aligned signal.
    pub fn output_delay_frames(&self) -> usize {
        self.resampler
            .as_ref()
            .map(|resampler| resampler.output_delay())
            .unwrap_or(self.quality.sinc_len() / 2)
    }

    /// Get the current effective resampling ratio (may differ from nominal when dynamic_ratio is used)
    pub fn current_ratio(&self) -> f64 {
        self.current_ratio
    }

    /// Get the current quality preset
    pub fn quality(&self) -> ResamplerQuality {
        self.quality
    }

    /// Check if dynamic ratio is enabled
    pub fn is_dynamic_ratio(&self) -> bool {
        self.dynamic_ratio
    }

    /// Set the resampling ratio at runtime (only works when dynamic_ratio is enabled).
    /// The ratio is clamped to the allowed range (nominal / 2.0 .. nominal * 2.0).
    /// When `ramp` is true, the ratio change is smoothly interpolated.
    pub fn set_ratio(&mut self, new_ratio: f64, ramp: bool) -> Result<(), String> {
        if !self.dynamic_ratio {
            return Err(
                "Dynamic ratio is not enabled. Set dynamic_ratio to true first.".to_string(),
            );
        }
        let resampler = self.resampler.as_mut().ok_or("Resampler not initialized")?;
        resampler
            .set_resample_ratio(new_ratio, ramp)
            .map_err(|e| format!("Failed to set ratio: {:?}", e))?;
        self.current_ratio = new_ratio;
        Ok(())
    }

    /// Finish the current stream using rubato's documented complete-stream contract.
    ///
    /// When `process()` receives input that is not a multiple of `chunk_size`, the remaining
    /// frames are held in an internal residual buffer and will not be processed until the next
    /// `process()` call that fills it.  Call `flush()` at the end of a stream to drain those
    /// frames. The final partial chunk is submitted with rubato's `partial_len`, then zero-input
    /// chunks are pumped until the cumulative raw output contains the complete delayed signal.
    /// The returned output is already trimmed at the exact cumulative boundary; `discard` is
    /// retained for source compatibility and is always zero.
    ///
    /// Returns the number of output frames written into `output`.
    ///
    /// `output` must be at least `flush_output_frames_max() * num_channels` samples long
    /// (i.e., large enough for one chunk's maximum output).
    pub fn flush(&mut self, output: &mut [f32]) -> Result<(usize, usize), String> {
        let result = self.drain(output, &ProcessContext::new(self.input_sample_rate, 0))?;
        Ok((result.frames, 0))
    }

    /// Set the resampling ratio relative to the current ratio (only works when dynamic_ratio is enabled).
    /// For example, `rel_ratio=1.01` increases the ratio by 1%.
    pub fn set_ratio_relative(&mut self, rel_ratio: f64, ramp: bool) -> Result<(), String> {
        if !self.dynamic_ratio {
            return Err(
                "Dynamic ratio is not enabled. Set dynamic_ratio to true first.".to_string(),
            );
        }
        let resampler = self.resampler.as_mut().ok_or("Resampler not initialized")?;
        resampler
            .set_resample_ratio_relative(rel_ratio, ramp)
            .map_err(|e| format!("Failed to set relative ratio: {:?}", e))?;
        // Update our tracked ratio
        self.current_ratio *= rel_ratio;
        Ok(())
    }
}

impl Plugin for ResamplerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Resampler", env!("CARGO_PKG_VERSION"), "SotF").with_description(format!(
            "Sample rate converter: {}Hz -> {}Hz (ratio: {:.4}, quality: {})",
            self.input_sample_rate,
            self.output_sample_rate,
            self.current_ratio,
            self.quality.as_str()
        ))
    }

    fn input_channels(&self) -> usize {
        self.num_channels
    }

    fn output_channels(&self) -> usize {
        self.num_channels
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::linear_transform(
            PluginCostClass::Convolution,
            None,
            self.latency_samples(),
            false,
            true,
            false,
        )
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_quality {
            let new_quality = match &value {
                ParameterValue::Int(index) => ResamplerQuality::from_index(*index)
                    .ok_or_else(|| format!("Invalid quality index {index}: expected 0, 1, or 2"))?,
                ParameterValue::String(label) => {
                    ResamplerQuality::from_str(label).ok_or_else(|| {
                        format!("Invalid quality '{label}': expected fast, medium, or high")
                    })?
                }
                _ => return Err("quality must be a choice index".to_string()),
            };
            if new_quality != self.quality {
                if self.initialized || self.residual_frames != 0 {
                    return Err(
                        "quality is a structural setup parameter; rebuild the plugin to change it"
                            .to_string(),
                    );
                }
                self.quality = new_quality;
                self.rebuild_resampler()?;
            }
        } else if id == self.param_dynamic_ratio {
            let v = value
                .as_bool()
                .ok_or_else(|| "dynamic_ratio must be a bool".to_string())?;
            self.dynamic_ratio = v;
            if !v {
                // Reset ratio to nominal when disabling dynamic ratio
                let nominal = self.output_sample_rate as f64 / self.input_sample_rate as f64;
                if (self.current_ratio - nominal).abs() > 1e-10 {
                    if let Some(ref mut resampler) = self.resampler {
                        let _ = resampler.set_resample_ratio(nominal, true);
                    }
                    self.current_ratio = nominal;
                }
            }
        } else if id == self.param_ratio {
            let v = value
                .as_float()
                .ok_or_else(|| "ratio must be a float".to_string())?;
            if !self.dynamic_ratio {
                return Err("Cannot change ratio when dynamic_ratio is disabled".to_string());
            }
            self.set_ratio(v as f64, true)?;
            return Ok(());
        } else {
            return Err(format!("Unknown parameter: {id}"));
        }
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_quality {
            Some(ParameterValue::Int(self.quality.index()))
        } else if id == &self.param_dynamic_ratio {
            Some(ParameterValue::Bool(self.dynamic_ratio))
        } else if id == &self.param_ratio {
            Some(ParameterValue::Float(self.current_ratio as f32))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        // The resampler has its own fixed input/output rates. If the host's
        // processing rate differs from our input rate, log a warning since the
        // resampling ratio may not produce the expected output rate.
        if sample_rate != self.input_sample_rate && self.input_sample_rate > 0 {
            return Err(format!(
                "Host sample rate ({sample_rate} Hz) differs from configured input rate ({} Hz)",
                self.input_sample_rate
            ));
        }
        self.initialized = true;
        Ok(())
    }

    fn reset(&mut self) {
        // Reset the resampler state
        if let Some(ref mut resampler) = self.resampler {
            resampler.reset();
        }
        // Reset ratio to nominal
        self.current_ratio = self.output_sample_rate as f64 / self.input_sample_rate as f64;
        // Clear residual buffer — zero the data to prevent stale audio leaking through
        // if a future code path reads residual_input without tight bounds checking.
        self.residual_frames = 0;
        self.last_output_frames = 0;
        self.stream_input_frames = 0;
        self.stream_output_frames = 0;
        self.expected_signal_frames = 0.0;
        self.drain_target_frames = None;
        for ch in 0..self.num_channels {
            self.residual_input[ch].fill(0.0);
        }
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        if self.drain_target_frames.is_some() {
            return Err("stream has been finalized; reset before processing new input".to_string());
        }
        let num_input_frames = context.num_frames;
        let expected_input_samples = num_input_frames * self.num_channels;

        if input.len() != expected_input_samples {
            return Err(format!(
                "Input size mismatch: expected {} samples ({} frames x {} channels), got {}",
                expected_input_samples,
                num_input_frames,
                self.num_channels,
                input.len()
            ));
        }

        if self.is_unity_passthrough() {
            if output.len() < input.len() {
                return Err(format!(
                    "Output buffer too small: need {} samples, got {}",
                    input.len(),
                    output.len()
                ));
            }
            output[..input.len()].copy_from_slice(input);
            self.last_output_frames = num_input_frames;
            self.stream_input_frames = self
                .stream_input_frames
                .saturating_add(num_input_frames as u64);
            self.stream_output_frames = self
                .stream_output_frames
                .saturating_add(num_input_frames as u64);
            self.expected_signal_frames += num_input_frames as f64;
            return Ok(num_input_frames);
        }

        let complete_chunks =
            self.residual_frames.saturating_add(num_input_frames) / self.chunk_size;
        let capacity_frames = complete_chunks.saturating_mul(
            self.resampler
                .as_ref()
                .ok_or("Resampler not initialized")?
                .output_frames_max(),
        );
        let required_samples = capacity_frames.saturating_mul(self.num_channels);
        if required_samples > output.len() {
            return Err(format!(
                "Output buffer too small: need {required_samples} samples, got {}",
                output.len()
            ));
        }

        // Variable-length input support: buffer input frames in residual_input
        // and process full chunk_size blocks through the resampler.
        let resampler = self.resampler.as_mut().ok_or("Resampler not initialized")?;
        let max_output_frames = resampler.output_frames_max();
        let chunk_size = self.chunk_size;

        let mut total_output_frames = 0usize;
        let mut input_offset = 0usize;
        let mut remaining_frames = num_input_frames;

        // Fill residual buffer from input, process full chunks
        while remaining_frames > 0 {
            let space_in_residual = chunk_size - self.residual_frames;
            let frames_to_copy = remaining_frames.min(space_in_residual);

            // Copy interleaved input into planar residual buffer
            for ch in 0..self.num_channels {
                for frame in 0..frames_to_copy {
                    self.residual_input[ch][self.residual_frames + frame] =
                        input[(input_offset + frame) * self.num_channels + ch];
                }
            }
            self.residual_frames += frames_to_copy;
            input_offset += frames_to_copy;
            remaining_frames -= frames_to_copy;

            // When we have a full chunk, process it
            if self.residual_frames == chunk_size {
                let input_adapter =
                    SequentialSliceOfVecs::new(&self.residual_input, self.num_channels, chunk_size)
                        .map_err(|e| format!("Input adapter error: {:?}", e))?;
                let mut output_adapter = SequentialSliceOfVecs::new_mut(
                    &mut self.output_buffer,
                    self.num_channels,
                    max_output_frames,
                )
                .map_err(|e| format!("Output adapter error: {:?}", e))?;

                let (_, output_frames) = resampler
                    .process_into_buffer(&input_adapter, &mut output_adapter, None)
                    .map_err(|e| format!("Resampling failed: {:?}", e))?;
                self.residual_frames = 0;

                // Check output buffer capacity
                let out_sample_offset = total_output_frames * self.num_channels;
                let new_output_samples = output_frames * self.num_channels;
                if out_sample_offset + new_output_samples > output.len() {
                    return Err(format!(
                        "Output buffer too small: need {} samples, got {}",
                        out_sample_offset + new_output_samples,
                        output.len()
                    ));
                }

                // Convert planar to interleaved into output at current offset
                Self::planar_to_interleaved(
                    &self.output_buffer,
                    &mut output[out_sample_offset..],
                    output_frames,
                    self.num_channels,
                );

                total_output_frames += output_frames;
            }
        }

        // Store actual output frame count
        self.last_output_frames = total_output_frames;
        self.stream_input_frames = self
            .stream_input_frames
            .saturating_add(num_input_frames as u64);
        self.stream_output_frames = self
            .stream_output_frames
            .saturating_add(total_output_frames as u64);
        self.expected_signal_frames += num_input_frames as f64 * self.current_ratio;

        Ok(total_output_frames)
    }

    fn drain_output_frames_max(&self) -> usize {
        self.flush_output_frames_max()
    }

    fn drain(
        &mut self,
        output: &mut [f32],
        _context: &ProcessContext,
    ) -> PluginResult<PluginDrainResult> {
        if self.is_unity_passthrough() || self.stream_input_frames == 0 {
            self.last_output_frames = 0;
            self.drain_target_frames = Some(self.stream_output_frames);
            return Ok(PluginDrainResult::COMPLETE);
        }

        // Match rubato's process_all contract: ceil(total programme duration
        // in output frames), then retain enough raw output for leading-delay
        // trimming to preserve the matching final sinc tail.
        let computed_target = (self.expected_signal_frames.ceil() as u64)
            .saturating_add(self.output_delay_frames() as u64);
        let target = *self.drain_target_frames.get_or_insert(computed_target);
        if self.stream_output_frames >= target {
            self.last_output_frames = 0;
            return Ok(PluginDrainResult::COMPLETE);
        }

        let max_output_frames = self
            .resampler
            .as_ref()
            .ok_or("Resampler not initialized")?
            .output_frames_max();
        let remaining = target.saturating_sub(self.stream_output_frames) as usize;
        let frames_to_write_bound = remaining.min(max_output_frames);
        let required_samples = frames_to_write_bound.saturating_mul(self.num_channels);
        if output.len() < required_samples {
            return Err(format!(
                "Output buffer too small for drain: need {required_samples} samples, got {}",
                output.len()
            ));
        }

        let partial_len = self.residual_frames;
        for ch in 0..self.num_channels {
            self.residual_input[ch][partial_len..self.chunk_size].fill(0.0);
        }

        let input_adapter =
            SequentialSliceOfVecs::new(&self.residual_input, self.num_channels, self.chunk_size)
                .map_err(|e| format!("Input adapter error: {e:?}"))?;
        let mut output_adapter = SequentialSliceOfVecs::new_mut(
            &mut self.output_buffer,
            self.num_channels,
            max_output_frames,
        )
        .map_err(|e| format!("Output adapter error: {e:?}"))?;
        let indexing = Indexing {
            input_offset: 0,
            output_offset: 0,
            partial_len: Some(partial_len),
            active_channels_mask: None,
        };
        let (_, produced) = self
            .resampler
            .as_mut()
            .ok_or("Resampler not initialized")?
            .process_into_buffer(&input_adapter, &mut output_adapter, Some(&indexing))
            .map_err(|e| format!("Resampling drain failed: {e:?}"))?;

        self.residual_frames = 0;
        let frames = produced.min(remaining);
        Self::planar_to_interleaved(&self.output_buffer, output, frames, self.num_channels);
        self.stream_output_frames = self.stream_output_frames.saturating_add(produced as u64);
        self.last_output_frames = frames;
        Ok(PluginDrainResult {
            frames,
            complete: self.stream_output_frames >= target,
        })
    }

    fn latency_samples(&self) -> usize {
        if self.is_unity_passthrough() {
            return 0;
        }
        // Use rubato's exact output_delay() which accounts for the full FIR group delay,
        // ring-buffer offsets, and polyphase filter delays — not just sinc_len / 2.
        // Also add the chunking buffer latency: up to chunk_size - 1 frames can sit in
        // residual_input before producing output.
        let rubato_delay = self.output_delay_frames();
        let priming_output_frames = (((self.chunk_size - 1) as f64) * self.current_ratio).ceil();
        rubato_delay.saturating_add(priming_output_frames as usize)
    }

    fn realtime_quantum_frames(&self) -> usize {
        ResamplerPlugin::realtime_quantum_frames(self)
    }

    fn output_frames_for_input(&self, input_frames: usize) -> usize {
        ResamplerPlugin::output_frames_for_input(self, input_frames)
    }

    fn output_sample_rate(&self, _input_rate: u32) -> u32 {
        self.output_sample_rate
    }

    fn last_output_frames(&self) -> Option<usize> {
        Some(self.last_output_frames)
    }
}

#[test]
fn test_flush_produces_trailing_output() {
    let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
    resampler.initialize(44100).unwrap();

    // Process a partial chunk (512 frames)
    let num_frames = 512;
    let input = vec![0.5_f32; num_frames * 2];
    let max_output = resampler.output_frames_for_input(num_frames);
    let mut output = vec![0.0_f32; max_output * 2];
    let ctx = ProcessContext::new(44100, num_frames);
    let produced = resampler.process(&input, &mut output, &ctx).unwrap();
    assert_eq!(produced, 0, "Partial chunk should produce no output yet");

    // Flush returns the exact complete-stream prefix; no caller-side estimate is needed.
    let mut flush_buf = vec![0.0_f32; resampler.flush_output_frames_max() * 2];
    let (flush_output, discard) = resampler.flush(&mut flush_buf).unwrap();
    assert!(flush_output > 0, "Flush should produce trailing output");
    assert_eq!(discard, 0);
}

#[test]
fn test_rebuild_resampler_reuses_buffers() {
    let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();

    let output_ptr = resampler.output_buffer[0].as_ptr();
    let residual_ptr = resampler.residual_input[0].as_ptr();

    // Switch quality via set_parameter (calls rebuild_resampler internally)
    resampler
        .set_parameter(
            ParameterId::from("quality"),
            ParameterValue::String("high".to_string()),
        )
        .unwrap();

    assert_eq!(
        resampler.output_buffer[0].as_ptr(),
        output_ptr,
        "output_buffer was reallocated"
    );
    assert_eq!(
        resampler.residual_input[0].as_ptr(),
        residual_ptr,
        "residual_input was reallocated"
    );
}
