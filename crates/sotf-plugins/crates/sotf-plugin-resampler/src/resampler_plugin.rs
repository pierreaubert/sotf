use super::resampler_quality::ResamplerQuality;
use audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{
    Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{
    Plugin, PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
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
    /// Input buffer (planar: one vec per channel)
    pub(super) input_buffer: Vec<Vec<f32>>,
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
}

impl ResamplerPlugin {
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
            input_buffer: vec![vec![0.0; chunk_size]; num_channels],
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
    /// This reuses the pre-allocated `input_buffer`, `output_buffer`, and `residual_input`
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
            Parameter::new_string("quality", "Quality", self.quality.as_str().to_string())
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
        // Use rubato's output_frames_max() for safe buffer allocation
        // The actual output varies based on resampler internal state
        if let Some(ref resampler) = self.resampler {
            let pending = self.residual_frames.saturating_add(input_frames);
            let chunks = pending.div_ceil(self.chunk_size);
            chunks.saturating_mul(resampler.output_frames_max())
        } else {
            // Fallback estimate if resampler not initialized
            let ratio = self.output_sample_rate as f64 / self.input_sample_rate as f64;
            (input_frames as f64 * ratio).ceil() as usize + 1
        }
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
        self.rebuild_cached_parameters();
        Ok(())
    }

    /// Flush any buffered residual frames through the resampler.
    ///
    /// When `process()` receives input that is not a multiple of `chunk_size`, the remaining
    /// frames are held in an internal residual buffer and will not be processed until the next
    /// `process()` call that fills it.  Call `flush()` at the end of a stream to drain those
    /// frames.  The residual is zero-padded to a full `chunk_size` before being sent to rubato;
    /// callers should discard the trailing zero-padded portion of the output (approximately
    /// `(chunk_size - residual_frames) * ratio` frames from the end).
    ///
    /// Returns the number of output frames written into `output`.
    ///
    /// `output` must be at least `output_frames_for_input(0) * num_channels` samples long
    /// (i.e., large enough for one chunk's maximum output).
    pub fn flush(&mut self, output: &mut [f32]) -> Result<(usize, usize), String> {
        if self.residual_frames == 0 {
            return Ok((0, 0));
        }

        let resampler = self.resampler.as_mut().ok_or("Resampler not initialized")?;
        let chunk_size = self.chunk_size;
        let max_output_frames = resampler.output_frames_max();
        let residual = self.residual_frames;

        // Zero-pad residual input to a full chunk.
        // residual_input already contains the valid frames at [0..residual_frames];
        // zero the tail so rubato sees silence for the padded portion.
        for ch in 0..self.num_channels {
            self.residual_input[ch][self.residual_frames..chunk_size].fill(0.0);
            // Copy into input_buffer for rubato (residual_input is the canonical source).
            self.input_buffer[ch][..chunk_size]
                .copy_from_slice(&self.residual_input[ch][..chunk_size]);
        }
        self.residual_frames = 0;

        let input_adapter =
            SequentialSliceOfVecs::new(&self.input_buffer, self.num_channels, chunk_size)
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

        let required_samples = output_frames * self.num_channels;
        if required_samples > output.len() {
            return Err(format!(
                "Output buffer too small for flush: need {required_samples} samples, got {}",
                output.len()
            ));
        }

        Self::planar_to_interleaved(
            &self.output_buffer,
            output,
            output_frames,
            self.num_channels,
        );

        // Compute how many trailing output frames are garbage from zero-padding.
        let valid_output_estimate = (residual as f64 * self.current_ratio).ceil() as usize;
        let discard = output_frames.saturating_sub(valid_output_estimate);
        Ok((output_frames, discard))
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
        self.rebuild_cached_parameters();
        Ok(())
    }
}

impl Plugin for ResamplerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Resampler", "2.0.0", "SotF").with_description(format!(
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
        self.validate_parameter(&id, &value)?;

        if id == self.param_quality {
            let s = value
                .as_string()
                .ok_or_else(|| "quality must be a string".to_string())?;
            let new_quality = ResamplerQuality::from_str(s).ok_or_else(|| {
                format!("Invalid quality '{}': expected fast, medium, or high", s)
            })?;
            if new_quality != self.quality {
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
            // rebuild_cached_parameters already called by set_ratio
            return Ok(());
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_quality {
            Some(ParameterValue::String(self.quality.as_str().to_string()))
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
            log::warn!(
                "[Resampler] Host sample rate ({} Hz) differs from configured input rate ({} Hz)",
                sample_rate,
                self.input_sample_rate
            );
        }
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
                // Copy residual into input_buffer for processing
                for ch in 0..self.num_channels {
                    self.input_buffer[ch][..chunk_size]
                        .copy_from_slice(&self.residual_input[ch][..chunk_size]);
                }
                self.residual_frames = 0;

                let input_adapter =
                    SequentialSliceOfVecs::new(&self.input_buffer, self.num_channels, chunk_size)
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

        Ok(total_output_frames)
    }

    fn latency_samples(&self) -> usize {
        // Use rubato's exact output_delay() which accounts for the full FIR group delay,
        // ring-buffer offsets, and polyphase filter delays — not just sinc_len / 2.
        // Also add the chunking buffer latency: up to chunk_size - 1 frames can sit in
        // residual_input before producing output.
        let rubato_delay = self.output_delay_frames();
        rubato_delay + self.chunk_size - 1
    }

    fn output_frames_for_input(&self, input_frames: usize) -> usize {
        // Use rubato's output_frames_max() for safe buffer allocation
        // The actual output varies based on resampler internal state
        if let Some(ref resampler) = self.resampler {
            let pending = self.residual_frames.saturating_add(input_frames);
            let chunks = pending.div_ceil(self.chunk_size);
            chunks.saturating_mul(resampler.output_frames_max())
        } else {
            // Fallback estimate if resampler not initialized
            let ratio = self.output_sample_rate as f64 / self.input_sample_rate as f64;
            (input_frames as f64 * ratio).ceil() as usize + 1
        }
    }

    fn output_sample_rate(&self, _input_rate: u32) -> u32 {
        self.output_sample_rate
    }

    fn last_output_frames(&self) -> Option<usize> {
        if self.last_output_frames > 0 {
            Some(self.last_output_frames)
        } else {
            None
        }
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

    // Flush should produce output for the buffered partial chunk, with discard > 0
    let mut flush_buf = vec![0.0_f32; max_output * 2];
    let (flush_output, discard) = resampler.flush(&mut flush_buf).unwrap();
    assert!(flush_output > 0, "Flush should produce trailing output");
    assert!(
        discard > 0,
        "Flushing a partial chunk should report discard frames > 0"
    );
}

#[test]
fn test_rebuild_resampler_reuses_buffers() {
    let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();

    let input_ptr = resampler.input_buffer[0].as_ptr();
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
        resampler.input_buffer[0].as_ptr(),
        input_ptr,
        "input_buffer was reallocated"
    );
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
