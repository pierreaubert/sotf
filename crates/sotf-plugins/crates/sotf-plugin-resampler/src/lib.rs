// ============================================================================
// Resampler Plugin
// ============================================================================
//
// High-quality audio resampling using the rubato library.
// Supports arbitrary sample rate conversion with minimal artifacts.

use audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{
    Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};

/// Quality preset for the resampler, controlling filter length and CPU usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResamplerQuality {
    /// 64-tap sinc filter. Lowest CPU, adequate for non-critical paths.
    Fast,
    /// 128-tap sinc filter. Good balance of quality and CPU.
    Medium,
    /// 256-tap sinc filter. Best quality, highest CPU.
    High,
}

impl ResamplerQuality {
    fn sinc_len(self) -> usize {
        match self {
            Self::Fast => 64,
            Self::Medium => 128,
            Self::High => 256,
        }
    }

    fn oversampling_factor(self) -> usize {
        match self {
            Self::Fast => 128,
            Self::Medium => 256,
            Self::High => 256,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "fast" => Some(Self::Fast),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }
}

/// Resampler plugin using rubato
///
/// This plugin resamples audio from one sample rate to another using high-quality
/// sinc interpolation. It maintains the same number of channels.
///
/// Note: The output buffer size will differ from input size based on the resampling ratio.
/// For example, resampling from 44.1kHz to 48kHz will produce more output frames.
pub struct ResamplerPlugin {
    /// Number of channels
    num_channels: usize,
    /// Input sample rate
    input_sample_rate: u32,
    /// Output sample rate
    output_sample_rate: u32,
    /// Rubato resampler (planar format)
    resampler: Option<Async<f32>>,
    /// Chunk size for processing (number of frames per chunk)
    chunk_size: usize,
    /// Input buffer (planar: one vec per channel)
    input_buffer: Vec<Vec<f32>>,
    /// Output buffer (planar: one vec per channel, pre-allocated to max output size)
    output_buffer: Vec<Vec<f32>>,
    /// Actual output frames from last process() call
    last_output_frames: usize,
    /// Residual input buffer for variable-length input support (planar, per-channel)
    residual_input: Vec<Vec<f32>>,
    /// Number of residual frames buffered
    residual_frames: usize,
    /// Quality preset
    quality: ResamplerQuality,
    /// Whether dynamic ratio changes are enabled
    dynamic_ratio: bool,
    /// Current effective ratio (may differ from nominal when dynamic_ratio is enabled)
    current_ratio: f64,
    /// Parameter IDs
    param_quality: ParameterId,
    param_dynamic_ratio: ParameterId,
    param_ratio: ParameterId,
    /// Cached parameters
    cached_parameters: Vec<Parameter>,
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
    fn create_resampler(
        num_channels: usize,
        input_sample_rate: u32,
        output_sample_rate: u32,
        chunk_size: usize,
        quality: ResamplerQuality,
    ) -> Result<Async<f32>, String> {
        let params = SincInterpolationParameters {
            sinc_len: quality.sinc_len(),
            f_cutoff: 0.95,
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
    fn rebuild_resampler(&mut self) -> Result<(), String> {
        let resampler = Self::create_resampler(
            self.num_channels,
            self.input_sample_rate,
            self.output_sample_rate,
            self.chunk_size,
            self.quality,
        )?;
        let max_output_frames = resampler.output_frames_max();
        self.output_buffer = vec![vec![0.0; max_output_frames]; self.num_channels];
        self.residual_input = vec![vec![0.0; self.chunk_size]; self.num_channels];
        self.residual_frames = 0;
        self.resampler = Some(resampler);
        self.current_ratio = self.output_sample_rate as f64 / self.input_sample_rate as f64;
        Ok(())
    }

    fn rebuild_cached_parameters(&mut self) {
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
    fn planar_to_interleaved(
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
        // Clear residual buffer
        self.residual_frames = 0;
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
        // Rubato has some latency due to the sinc filter
        // Approximately half the sinc filter length
        self.quality.sinc_len() / 2
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

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_resampler_creation() {
        let resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
        assert_eq!(resampler.input_channels(), 2);
        assert_eq!(resampler.output_channels(), 2);
        assert!(resampler.ratio() > 1.0); // Upsampling
    }

    #[test]
    fn test_resampler_44100_to_48000() {
        let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
        resampler.initialize(44100).unwrap();

        // Create test signal: 1kHz sine wave at 44.1kHz
        let num_frames = 1024;
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 44100.0;
            let sample = phase.sin() * 0.5;
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }

        // Calculate maximum output buffer size (conservative)
        let max_output_frames = resampler.output_frames_for_input(num_frames);
        let mut output = vec![0.0_f32; max_output_frames * 2];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames,
        };

        // Process
        resampler.process(&input, &mut output, &context).unwrap();

        log::info!("Input frames: {}", num_frames);
        log::info!("Max output frames (buffer size): {}", max_output_frames);
        log::info!("Expected ratio: {:.4}", 48000.0 / 44100.0);

        // Check that output contains signal (actual frames may be less than max)
        // We check the first portion of the output buffer
        let expected_frames = (num_frames as f64 * 48000.0 / 44100.0) as usize;
        let check_samples = expected_frames * 2;
        let rms: f32 =
            output[..check_samples].iter().map(|x| x * x).sum::<f32>() / check_samples as f32;
        let rms = rms.sqrt();
        log::info!("Output RMS (first {} frames): {:.4}", expected_frames, rms);
        assert!(rms > 0.1, "Output should contain signal");
    }

    #[test]
    fn test_output_frame_estimate_covers_multi_chunk_input() {
        let chunk_size = 1024;
        let mut resampler = ResamplerPlugin::new(2, 44100, 48000, chunk_size).unwrap();
        resampler.initialize(44100).unwrap();

        let num_frames = chunk_size * 3;
        let input = vec![0.25_f32; num_frames * 2];
        let max_output_frames = resampler.output_frames_for_input(num_frames);
        let mut output = vec![0.0_f32; max_output_frames * 2];
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames,
        };

        let produced = resampler.process(&input, &mut output, &context).unwrap();

        assert!(
            produced <= max_output_frames,
            "produced {produced} frames, estimate only allowed {max_output_frames}"
        );
        assert!(produced > chunk_size);
    }

    #[test]
    fn test_resampler_48000_to_44100() {
        let mut resampler = ResamplerPlugin::new(2, 48000, 44100, 1024).unwrap();
        resampler.initialize(48000).unwrap();

        // Create test signal at 48kHz
        let num_frames = 1024;
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 0.5;
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }

        let max_output_frames = resampler.output_frames_for_input(num_frames);
        let mut output = vec![0.0_f32; max_output_frames * 2];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        resampler.process(&input, &mut output, &context).unwrap();

        log::info!("Input frames: {}", num_frames);
        log::info!("Max output frames (buffer size): {}", max_output_frames);
        log::info!("Expected ratio: {:.4}", 44100.0 / 48000.0);

        // Check signal (actual frames may be less than max buffer)
        let expected_frames = (num_frames as f64 * 44100.0 / 48000.0) as usize;
        let check_samples = expected_frames * 2;
        let rms: f32 =
            output[..check_samples].iter().map(|x| x * x).sum::<f32>() / check_samples as f32;
        let rms = rms.sqrt();
        log::info!("Output RMS (first {} frames): {:.4}", expected_frames, rms);
        assert!(rms > 0.1);
    }

    #[test]
    fn test_resampler_multichannel() {
        // Test with 5 channels (5.0 surround)
        let mut resampler = ResamplerPlugin::new(5, 44100, 48000, 1024).unwrap();
        resampler.initialize(44100).unwrap();

        let num_frames = 1024;
        let mut input = vec![0.0_f32; num_frames * 5];

        // Different frequency on each channel
        for i in 0..num_frames {
            let t = i as f32 / 44100.0;
            input[i * 5] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.2; // FL
            input[i * 5 + 1] = (2.0 * std::f32::consts::PI * 550.0 * t).sin() * 0.2; // FR
            input[i * 5 + 2] = (2.0 * std::f32::consts::PI * 660.0 * t).sin() * 0.2; // C
            input[i * 5 + 3] = (2.0 * std::f32::consts::PI * 220.0 * t).sin() * 0.2; // RL
            input[i * 5 + 4] = (2.0 * std::f32::consts::PI * 330.0 * t).sin() * 0.2; // RR
        }

        let max_output_frames = resampler.output_frames_for_input(num_frames);
        let mut output = vec![0.0_f32; max_output_frames * 5];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames,
        };

        resampler.process(&input, &mut output, &context).unwrap();

        log::info!(
            "5-channel resampling: {} input frames, {} max output frames",
            num_frames,
            max_output_frames
        );

        // Check each channel has signal (check expected number of frames)
        let expected_frames = (num_frames as f64 * 48000.0 / 44100.0) as usize;
        for ch in 0..5 {
            let channel_samples: Vec<f32> =
                (0..expected_frames).map(|i| output[i * 5 + ch]).collect();
            let rms: f32 =
                channel_samples.iter().map(|x| x * x).sum::<f32>() / channel_samples.len() as f32;
            let rms = rms.sqrt();
            log::info!("Channel {} RMS: {:.4}", ch, rms);
            assert!(rms > 0.05, "Channel {} should have signal", ch);
        }
    }

    #[test]
    fn test_resampler_reset() {
        let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
        resampler.initialize(44100).unwrap();

        let num_frames = 1024;
        let input = vec![0.5_f32; num_frames * 2];
        let output_frames = resampler.output_frames_for_input(num_frames);
        let mut output = vec![0.0_f32; output_frames * 2];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames,
        };

        // Process
        resampler.process(&input, &mut output, &context).unwrap();

        // Reset
        resampler.reset();

        // Process again - should work
        resampler.process(&input, &mut output, &context).unwrap();

        // Should still have output
        let rms: f32 = output.iter().map(|x| x * x).sum::<f32>() / output.len() as f32;
        assert!(rms.sqrt() > 0.1);
    }

    /// After reset(), processing silence should produce silence (no residual
    /// from previously processed audio leaking through).
    #[test]
    fn test_reset_clears_residual() {
        let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
        resampler.initialize(44100).unwrap();

        let num_frames = 1024;
        let ctx = ProcessContext {
            sample_rate: 44100,
            num_frames,
        };

        // Process a loud signal
        let loud_input: Vec<f32> = (0..num_frames * 2)
            .map(|i| 0.9 * (2.0 * std::f32::consts::PI * 1000.0 * (i / 2) as f32 / 44100.0).sin())
            .collect();
        let max_output = resampler.output_frames_for_input(num_frames);
        let mut output = vec![0.0_f32; max_output * 2];
        resampler.process(&loud_input, &mut output, &ctx).unwrap();

        // Reset
        resampler.reset();

        // Process silence
        let silence = vec![0.0_f32; num_frames * 2];
        let mut output2 = vec![0.0_f32; max_output * 2];
        resampler.process(&silence, &mut output2, &ctx).unwrap();

        // After reset + processing silence, output RMS should be very low
        let rms: f32 = (output2.iter().map(|x| x * x).sum::<f32>() / output2.len() as f32).sqrt();
        assert!(
            rms < 0.01,
            "After reset and processing silence, output should be near-silent, \
             but RMS={rms:.6}"
        );
    }

    #[test]
    fn test_quality_presets() {
        // Fast
        let r_fast =
            ResamplerPlugin::with_quality(2, 44100, 48000, 1024, ResamplerQuality::Fast).unwrap();
        assert_eq!(r_fast.quality(), ResamplerQuality::Fast);
        assert_eq!(r_fast.latency_samples(), 32); // 64 / 2

        // Medium
        let r_med =
            ResamplerPlugin::with_quality(2, 44100, 48000, 1024, ResamplerQuality::Medium).unwrap();
        assert_eq!(r_med.quality(), ResamplerQuality::Medium);
        assert_eq!(r_med.latency_samples(), 64); // 128 / 2

        // High
        let r_high =
            ResamplerPlugin::with_quality(2, 44100, 48000, 1024, ResamplerQuality::High).unwrap();
        assert_eq!(r_high.quality(), ResamplerQuality::High);
        assert_eq!(r_high.latency_samples(), 128); // 256 / 2
    }

    #[test]
    fn test_quality_parameter_change() {
        let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
        resampler.initialize(44100).unwrap();
        assert_eq!(resampler.quality(), ResamplerQuality::Medium);

        // Switch to fast
        resampler
            .set_parameter(
                ParameterId::from("quality"),
                ParameterValue::String("fast".to_string()),
            )
            .unwrap();
        assert_eq!(resampler.quality(), ResamplerQuality::Fast);

        // Switch to high
        resampler
            .set_parameter(
                ParameterId::from("quality"),
                ParameterValue::String("high".to_string()),
            )
            .unwrap();
        assert_eq!(resampler.quality(), ResamplerQuality::High);

        // Invalid quality should fail
        assert!(
            resampler
                .set_parameter(
                    ParameterId::from("quality"),
                    ParameterValue::String("ultra".to_string()),
                )
                .is_err()
        );
    }

    #[test]
    fn test_quality_affects_processing() {
        // Verify that different quality presets all produce valid output
        let num_frames = 1024;
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 44100.0;
            let sample = phase.sin() * 0.5;
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }

        for quality in [
            ResamplerQuality::Fast,
            ResamplerQuality::Medium,
            ResamplerQuality::High,
        ] {
            let mut resampler =
                ResamplerPlugin::with_quality(2, 44100, 48000, 1024, quality).unwrap();
            resampler.initialize(44100).unwrap();

            let max_output = resampler.output_frames_for_input(num_frames);
            let mut output = vec![0.0_f32; max_output * 2];

            let context = ProcessContext {
                sample_rate: 44100,
                num_frames,
            };
            resampler.process(&input, &mut output, &context).unwrap();

            let expected_frames = (num_frames as f64 * 48000.0 / 44100.0) as usize;
            let check_samples = expected_frames * 2;
            let rms: f32 =
                output[..check_samples].iter().map(|x| x * x).sum::<f32>() / check_samples as f32;
            assert!(
                rms.sqrt() > 0.1,
                "Quality {:?} should produce valid output",
                quality
            );
        }
    }

    #[test]
    fn test_dynamic_ratio() {
        let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
        resampler.initialize(44100).unwrap();

        // Dynamic ratio should be disabled by default
        assert!(!resampler.is_dynamic_ratio());

        // Setting ratio should fail when disabled
        assert!(resampler.set_ratio(1.1, true).is_err());

        // Enable dynamic ratio
        resampler
            .set_parameter(
                ParameterId::from("dynamic_ratio"),
                ParameterValue::Bool(true),
            )
            .unwrap();
        assert!(resampler.is_dynamic_ratio());

        // Now setting ratio should succeed
        let nominal = resampler.ratio();
        let new_ratio = nominal * 1.01;
        resampler.set_ratio(new_ratio, true).unwrap();
        assert!((resampler.current_ratio() - new_ratio).abs() < 1e-10);

        // Process should still work
        let num_frames = 1024;
        let input = vec![0.5_f32; num_frames * 2];
        let max_output = resampler.output_frames_for_input(num_frames);
        let mut output = vec![0.0_f32; max_output * 2];
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames,
        };
        resampler.process(&input, &mut output, &context).unwrap();
    }

    #[test]
    fn test_dynamic_ratio_relative() {
        let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
        resampler.initialize(44100).unwrap();

        // Enable dynamic ratio
        resampler
            .set_parameter(
                ParameterId::from("dynamic_ratio"),
                ParameterValue::Bool(true),
            )
            .unwrap();

        let original = resampler.current_ratio();
        resampler.set_ratio_relative(1.01, true).unwrap();
        let expected = original * 1.01;
        assert!(
            (resampler.current_ratio() - expected).abs() < 1e-10,
            "Relative ratio should multiply: {} vs {}",
            resampler.current_ratio(),
            expected
        );
    }

    #[test]
    fn test_dynamic_ratio_via_parameter() {
        let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
        resampler.initialize(44100).unwrap();

        // Trying to set ratio parameter when dynamic is off should fail
        assert!(
            resampler
                .set_parameter(ParameterId::from("ratio"), ParameterValue::Float(1.1),)
                .is_err()
        );

        // Enable dynamic ratio
        resampler
            .set_parameter(
                ParameterId::from("dynamic_ratio"),
                ParameterValue::Bool(true),
            )
            .unwrap();

        // Now setting ratio via parameter should work
        let nominal = resampler.ratio() as f32;
        let new_ratio = nominal * 1.01;
        resampler
            .set_parameter(ParameterId::from("ratio"), ParameterValue::Float(new_ratio))
            .unwrap();
        assert!(
            (resampler.current_ratio() - new_ratio as f64).abs() < 1e-4,
            "Ratio via parameter: {} vs {}",
            resampler.current_ratio(),
            new_ratio
        );
    }

    #[test]
    fn test_parameter_getset() {
        let resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();

        // Quality
        assert_eq!(
            resampler.get_parameter(&ParameterId::from("quality")),
            Some(ParameterValue::String("medium".to_string()))
        );

        // Dynamic ratio
        assert_eq!(
            resampler.get_parameter(&ParameterId::from("dynamic_ratio")),
            Some(ParameterValue::Bool(false))
        );

        // Ratio
        let ratio_val = resampler.get_parameter(&ParameterId::from("ratio"));
        assert!(ratio_val.is_some());
        if let Some(ParameterValue::Float(r)) = ratio_val {
            assert!((r as f64 - 48000.0 / 44100.0).abs() < 1e-4);
        }

        // Unknown
        assert_eq!(
            resampler.get_parameter(&ParameterId::from("nonexistent")),
            None
        );
    }

    #[test]
    fn test_disable_dynamic_ratio_resets() {
        let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
        resampler.initialize(44100).unwrap();

        let nominal = resampler.ratio();

        // Enable, change ratio, then disable
        resampler
            .set_parameter(
                ParameterId::from("dynamic_ratio"),
                ParameterValue::Bool(true),
            )
            .unwrap();
        resampler.set_ratio(nominal * 1.05, true).unwrap();
        assert!((resampler.current_ratio() - nominal).abs() > 0.01);

        // Disable should reset to nominal
        resampler
            .set_parameter(
                ParameterId::from("dynamic_ratio"),
                ParameterValue::Bool(false),
            )
            .unwrap();
        assert!(
            (resampler.current_ratio() - nominal).abs() < 1e-10,
            "Disabling dynamic_ratio should reset to nominal"
        );
    }
}
