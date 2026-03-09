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
        if num_channels == 0 {
            return Err("num_channels must be > 0".to_string());
        }
        if input_sample_rate == 0 || output_sample_rate == 0 {
            return Err("sample rates must be > 0".to_string());
        }
        if chunk_size == 0 {
            return Err("chunk_size must be > 0".to_string());
        }

        // Create resampler
        let resampler = Self::create_resampler(
            num_channels,
            input_sample_rate,
            output_sample_rate,
            chunk_size,
        )?;

        let max_output_frames = resampler.output_frames_max();

        Ok(Self {
            num_channels,
            input_sample_rate,
            output_sample_rate,
            resampler: Some(resampler),
            chunk_size,
            input_buffer: vec![vec![0.0; chunk_size]; num_channels],
            output_buffer: vec![vec![0.0; max_output_frames]; num_channels],
            last_output_frames: 0,
        })
    }

    /// Create a new resampler with default chunk size (1024)
    pub fn new_default(
        num_channels: usize,
        input_sample_rate: u32,
        output_sample_rate: u32,
    ) -> Result<Self, String> {
        Self::new(num_channels, input_sample_rate, output_sample_rate, 1024)
    }

    /// Create the rubato resampler
    fn create_resampler(
        num_channels: usize,
        input_sample_rate: u32,
        output_sample_rate: u32,
        chunk_size: usize,
    ) -> Result<Async<f32>, String> {
        // Use high-quality sinc interpolation
        let params = SincInterpolationParameters {
            sinc_len: 256,                                // Sinc filter length
            f_cutoff: 0.95, // Cutoff frequency (0.95 = 95% of Nyquist)
            interpolation: SincInterpolationType::Linear, // Linear interpolation
            oversampling_factor: 256, // Quality factor
            window: WindowFunction::BlackmanHarris2, // Window function
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

    /// Convert interleaved input to planar format
    fn interleaved_to_planar(&mut self, input: &[f32], num_frames: usize) {
        for ch in 0..self.num_channels {
            for frame in 0..num_frames {
                self.input_buffer[ch][frame] = input[frame * self.num_channels + ch];
            }
        }
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
    pub fn output_frames_for_input(&self, _input_frames: usize) -> usize {
        // Use rubato's output_frames_max() for safe buffer allocation
        // The actual output varies based on resampler internal state
        if let Some(ref resampler) = self.resampler {
            resampler.output_frames_max()
        } else {
            // Fallback estimate if resampler not initialized
            let ratio = self.output_sample_rate as f64 / self.input_sample_rate as f64;
            (_input_frames as f64 * ratio).ceil() as usize + 1
        }
    }

    /// Get the resampling ratio (output_rate / input_rate)
    pub fn ratio(&self) -> f64 {
        self.output_sample_rate as f64 / self.input_sample_rate as f64
    }
}

impl Plugin for ResamplerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Resampler", "1.0.0", "SotF").with_description(format!(
            "High-quality sample rate converter: {}Hz -> {}Hz (ratio: {:.4})",
            self.input_sample_rate,
            self.output_sample_rate,
            self.ratio()
        ))
    }

    fn input_channels(&self) -> usize {
        self.num_channels
    }

    fn output_channels(&self) -> usize {
        self.num_channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        // Resampling ratio is fixed at creation time
        vec![]
    }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        Err("Resampler has no adjustable parameters".to_string())
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn initialize(&mut self, _sample_rate: u32) -> PluginResult<()> {
        // Recreate resampler with potentially new settings
        // Note: For now, we keep the original input/output sample rates
        // The sample_rate parameter refers to the host's processing rate
        Ok(())
    }

    fn reset(&mut self) {
        // Reset the resampler state
        if let Some(ref mut resampler) = self.resampler {
            resampler.reset();
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
                "Input size mismatch: expected {} samples ({} frames × {} channels), got {}",
                expected_input_samples,
                num_input_frames,
                self.num_channels,
                input.len()
            ));
        }

        // Check that we can process this many frames
        if num_input_frames != self.chunk_size {
            return Err(format!(
                "Input frames ({}) must match chunk size ({}). Resampler requires fixed input size.",
                num_input_frames, self.chunk_size
            ));
        }

        // Convert interleaved to planar
        self.interleaved_to_planar(input, num_input_frames);

        // Process resampling using pre-allocated buffers (zero-allocation path)
        let resampler = self.resampler.as_mut().ok_or("Resampler not initialized")?;

        let max_output_frames = resampler.output_frames_max();
        let input_adapter =
            SequentialSliceOfVecs::new(&self.input_buffer, self.num_channels, num_input_frames)
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

        // Store actual output frame count
        self.last_output_frames = output_frames;

        // Check output buffer size
        let expected_output_samples = output_frames * self.num_channels;
        if output.len() < expected_output_samples {
            return Err(format!(
                "Output buffer too small: need {} samples ({} frames × {} channels), got {}",
                expected_output_samples,
                output_frames,
                self.num_channels,
                output.len()
            ));
        }

        // Convert planar to interleaved
        Self::planar_to_interleaved(
            &self.output_buffer,
            output,
            output_frames,
            self.num_channels,
        );

        Ok(output_frames)
    }

    fn latency_samples(&self) -> usize {
        // Rubato has some latency due to the sinc filter
        // This is approximately half the sinc filter length
        128
    }

    fn output_frames_for_input(&self, _input_frames: usize) -> usize {
        // Use rubato's output_frames_max() for safe buffer allocation
        // The actual output varies based on resampler internal state
        if let Some(ref resampler) = self.resampler {
            resampler.output_frames_max()
        } else {
            // Fallback estimate if resampler not initialized
            let ratio = self.output_sample_rate as f64 / self.input_sample_rate as f64;
            (_input_frames as f64 * ratio).ceil() as usize + 1
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
}
