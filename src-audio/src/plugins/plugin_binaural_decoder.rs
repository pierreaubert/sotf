// ============================================================================
// Binaural Decoder Plugin - Multi-channel to Binaural Stereo
// ============================================================================
//
// This plugin converts multi-channel audio (e.g., 5.0, 5.1, 7.1) to binaural
// stereo using Head-Related Transfer Functions (HRTFs) from SOFA files.
//
// Algorithm:
// - Uses standard Overlap-Add (OLA) convolution
// - Input is processed in blocks of 'hop_size' (fft_size / 2)
// - Input blocks are zero-padded to 'fft_size'
// - HRTF Impulse Responses are truncated to 'hop_size' to avoid circular convolution aliasing
// - Output is stereo (left/right ears) suitable for headphone playback
//
// Supported input formats (using speaker_config module):
// - 2.0: Stereo (L/R at ±30°)
// - 5.0: FL/FR/C/SL/SR (standard surround without LFE)
// - 5.1: FL/FR/C/LFE/SL/SR (LFE passed through)
// - 7.1: FL/FR/C/LFE/SL/SR/RL/RR
// - Plus all configurations from speaker_config module

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::speaker_config::{SpeakerConfig, SpeakerPosition, get_speaker_config_by_channels};
use crate::sofa::{SofaFile, SourcePosition};
use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

// ============================================================================
// Configuration
// ============================================================================

fn default_fft_size() -> usize {
    2048
}

fn default_sofa_path() -> String {
    "".to_string()
}

/// Helper to convert SpeakerPosition to SourcePosition
fn speaker_to_source_position(speaker: &SpeakerPosition) -> SourcePosition {
    // Use a fixed distance of 1.0 for all speakers
    SourcePosition::new(speaker.azimuth, speaker.elevation, 1.0)
}

/// Configuration parameters for BinauralDecoderPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinauralDecoderParams {
    /// Path to SOFA file containing HRTFs
    #[serde(default = "default_sofa_path")]
    pub sofa_file: String,
    /// FFT size for convolution (must be power of 2)
    #[serde(default = "default_fft_size")]
    pub fft_size: usize,
    /// Number of input channels
    pub input_channels: usize,
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// Binaural decoder using HRTFs from SOFA file
pub struct BinauralDecoderPlugin {
    /// Number of input channels
    input_channels: usize,
    /// FFT size for convolution
    fft_size: usize,
    /// Hop size (50% overlap)
    hop_size: usize,
    /// Sample rate
    sample_rate: u32,

    /// SOFA file containing HRTFs
    sofa: Option<SofaFile>,
    /// Path to SOFA file
    sofa_path: Option<PathBuf>,

    /// Speaker configuration for input channels
    speaker_config: &'static SpeakerConfig,

    /// FFT planners
    fft_forward: Arc<dyn Fft<f32>>,
    fft_inverse: Arc<dyn Fft<f32>>,

    /// HRTF filters in frequency domain [channels × 2 × fft_size]
    /// For each input channel: [left_ear_fft, right_ear_fft]
    hrtf_filters_freq: Vec<Vec<Complex<f32>>>,

    /// Input buffer accumulator for block-based processing (interleaved multi-channel)
    input_buffer: Vec<f32>,
    /// Number of samples currently in input buffer (counts samples, not frames)
    input_buffer_fill: usize,

    /// Output accumulator for overlap-add [2 × accumulator_size]
    output_accumulator: Vec<Vec<f32>>,
    output_accumulator_fill: usize,
    next_add_position: usize,

    /// Temporary buffers (reused to avoid allocations)
    temp_input_block: Vec<f32>,
    temp_output_block: Vec<f32>,
    temp_freq_buffer: Vec<Complex<f32>>,
    temp_time_buffer: Vec<Complex<f32>>,
}

impl BinauralDecoderPlugin {
    /// Create a new binaural decoder plugin
    ///
    /// # Arguments
    /// * `input_channels` - Number of input channels
    /// * `fft_size` - FFT size for convolution (must be power of 2)
    /// * `sofa_path` - Path to SOFA file (optional, can be loaded later)
    pub fn new(input_channels: usize, fft_size: usize, sofa_path: Option<PathBuf>) -> Self {
        assert!(fft_size.is_power_of_two(), "FFT size must be power of 2");
        assert!(input_channels > 0, "Must have at least 1 input channel");

        let hop_size = fft_size / 2;

        let mut planner = FftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(fft_size);
        let fft_inverse = planner.plan_fft_inverse(fft_size);

        // Determine speaker configuration based on channel count
        let speaker_config = get_speaker_config_by_channels(input_channels)
            .unwrap_or_else(|| {
                log::warn!(
                    "[BinauralDecoder] No standard configuration for {} channels, using default circular layout",
                    input_channels
                );
                // Fall back to a generic circular layout for unsupported channel counts
                // For now, default to stereo for safety
                get_speaker_config_by_channels(2).unwrap()
            });

        log::info!(
            "[BinauralDecoder] Created with {} input channels ({}), FFT size {}",
            input_channels,
            speaker_config.name,
            fft_size
        );
        for speaker in speaker_config.speakers {
            log::info!(
                "[BinauralDecoder]   Ch{}: {} at az={:.1}°, el={:.1}°",
                speaker.channel,
                speaker.name,
                speaker.azimuth,
                speaker.elevation
            );
        }

        Self {
            input_channels,
            fft_size,
            hop_size,
            sample_rate: 48000, // Will be set in initialize()

            sofa: None,
            sofa_path,
            speaker_config,

            fft_forward,
            fft_inverse,

            hrtf_filters_freq: vec![vec![Complex::new(0.0, 0.0); fft_size * 2]; input_channels],

            input_buffer: vec![0.0; hop_size * input_channels], // Interleaved, size for one hop
            input_buffer_fill: 0,

            output_accumulator: vec![vec![0.0; fft_size * 2]; 2], // 2 output channels, enough space for overlap
            output_accumulator_fill: 0,
            next_add_position: 0,

            temp_input_block: vec![0.0; hop_size * input_channels], // Interleaved multi-channel input
            temp_output_block: vec![0.0; fft_size * 2], // Stereo output
            temp_freq_buffer: vec![Complex::new(0.0, 0.0); fft_size],
            temp_time_buffer: vec![Complex::new(0.0, 0.0); fft_size],
        }
    }

    /// Create from parameters
    pub fn from_params(params: BinauralDecoderParams) -> Self {
        let sofa_path = if params.sofa_file.is_empty() {
            None
        } else {
            Some(PathBuf::from(params.sofa_file))
        };

        Self::new(params.input_channels, params.fft_size, sofa_path)
    }

    /// Load SOFA file and prepare HRTFs
    pub fn load_sofa(&mut self, path: PathBuf) -> Result<(), String> {
        log::debug!("[BinauralDecoder] Loading SOFA file: {:?}", path);

        let sofa = SofaFile::load(&path)?;

        log::info!(
            "[BinauralDecoder] SOFA loaded: {} measurements, IR length: {}, sample rate: {} Hz",
            sofa.num_measurements,
            sofa.ir_length,
            sofa.sample_rate
        );

        // Prepare HRTF filters for each speaker
        self.prepare_hrtf_filters(&sofa)?;

        self.sofa = Some(sofa);
        self.sofa_path = Some(path);

        log::debug!("[BinauralDecoder] SOFA file loaded and HRTFs prepared");

        Ok(())
    }

    /// Prepare HRTF filters in frequency domain for all speakers
    fn prepare_hrtf_filters(&mut self, sofa: &SofaFile) -> Result<(), String> {
        for (i, speaker) in self.speaker_config.speakers.iter().enumerate() {
            let hrtf = sofa
                .get_hrtf_at_position(&speaker_to_source_position(speaker))
                .ok_or_else(|| format!("No HRTF found for speaker {}", speaker.name))?;

            log::info!(
                "[BinauralDecoder] Speaker {}: {} (az={:.1}°, el={:.1}°) -> HRTF at az={:.1}°, el={:.1}°",
                i,
                speaker.name,
                speaker.azimuth,
                speaker.elevation,
                hrtf.position.azimuth,
                hrtf.position.elevation
            );

            // Convert HRTFs to frequency domain
            // We need to pad/truncate IRs to hop_size (NOT fft_size) to ensure
            // L_input + L_ir - 1 <= N_fft
            // With L_input = hop_size and N_fft = 2 * hop_size, we need L_ir <= hop_size + 1
            let left_fft = self.ir_to_freq(&hrtf.ir_left);
            let right_fft = self.ir_to_freq(&hrtf.ir_right);

            // Store both left and right HRTFs
            // IMPORTANT: Explicit type annotation is required here to avoid type inference bug
            // in release builds that can cause the wrong vector type to be created
            let combined: Vec<Complex<f32>> = left_fft.into_iter().chain(right_fft.into_iter()).collect();

            debug_assert_eq!(
                combined.len(),
                self.fft_size * 2,
                "combined HRTF has wrong length"
            );

            self.hrtf_filters_freq[i] = combined;
        }

        Ok(())
    }

    /// Convert impulse response to frequency domain
    fn ir_to_freq(&self, ir: &[f32]) -> Vec<Complex<f32>> {
        let mut buffer = vec![Complex::new(0.0, 0.0); self.fft_size];

        // Copy IR data (pad with zeros if IR is shorter, truncate if longer)
        // CRITICAL: Truncate to hop_size to avoid circular convolution aliasing
        let copy_len = ir.len().min(self.hop_size);
        let mut max_val = 0.0f32;
        for i in 0..copy_len {
            buffer[i] = Complex::new(ir[i], 0.0);
            max_val = max_val.max(ir[i].abs());
        }
        
        if max_val > 0.9 {
             log::warn!("[BinauralDecoder] HRTF IR peak is very high: {:.4} (near 0dBFS). This might cause clipping.", max_val);
        } else {
             log::debug!("[BinauralDecoder] HRTF IR peak: {:.4}", max_val);
        }

        // FFT
        let mut freq = buffer.clone();
        self.fft_forward.process(&mut freq);

        freq
    }

    /// Process one block using standard Overlap-Add
    /// Input: `hop_size` frames (interleaved)
    /// Output: `fft_size` frames (stereo, interleaved) - tail is overlap
    fn process_block(&mut self, input: &[f32], output: &mut [f32]) {
        assert_eq!(input.len(), self.hop_size * self.input_channels);
        assert_eq!(output.len(), self.fft_size * 2); // Stereo output

        output.fill(0.0);

        // Check if we have HRTF filters loaded
        if self.hrtf_filters_freq.is_empty() || self.hrtf_filters_freq[0].len() != self.fft_size * 2
        {
            // No HRTFs loaded - pass through first 2 channels or silence
            // Since we are doing OLA, we just copy input to output (padded with zeros)
            for i in 0..self.hop_size {
                if self.input_channels >= 1 {
                    output[i * 2] = input[i * self.input_channels]; // Left
                }
                if self.input_channels >= 2 {
                    output[i * 2 + 1] = input[i * self.input_channels + 1]; // Right
                }
            }
            return;
        }

        let fft_scale = 1.0 / self.fft_size as f32;

        // Normalize by input channels to prevent gain accumulation
        // When summing multiple channels, we need to reduce gain to avoid clipping
        // Using slightly more aggressive normalization (0.9x) to prevent occasional peaks
        let channel_scale = 0.9 / (self.input_channels as f32).sqrt();

        // Process each input channel
        for ch in 0..self.input_channels {
            // 1. Extract channel data and zero-pad to FFT size
            for i in 0..self.fft_size {
                if i < self.hop_size {
                    let sample = input[i * self.input_channels + ch];
                    self.temp_time_buffer[i] = Complex::new(sample, 0.0);
                } else {
                    self.temp_time_buffer[i] = Complex::new(0.0, 0.0);
                }
            }

            // 2. Forward FFT
            self.temp_freq_buffer
                .copy_from_slice(&self.temp_time_buffer);
            self.fft_forward.process(&mut self.temp_freq_buffer);

            // 3. Convolve with left ear HRTF
            for i in 0..self.fft_size {
                self.temp_time_buffer[i] = self.temp_freq_buffer[i] * self.hrtf_filters_freq[ch][i];
            }

            // 4. Inverse FFT for left ear
            self.fft_inverse.process(&mut self.temp_time_buffer);

            // 5. Accumulate to left output with channel normalization
            for i in 0..self.fft_size {
                let mut sample = self.temp_time_buffer[i].re * fft_scale * channel_scale;
                // Flush denormals to zero to prevent CPU spikes and audio glitches
                if sample.abs() < 1e-30 {
                    sample = 0.0;
                }
                output[i * 2] += sample;
            }

            // 6. Convolve with right ear HRTF
            for i in 0..self.fft_size {
                self.temp_time_buffer[i] =
                    self.temp_freq_buffer[i] * self.hrtf_filters_freq[ch][self.fft_size + i];
            }

            // 7. Inverse FFT for right ear
            self.fft_inverse.process(&mut self.temp_time_buffer);

            // 8. Accumulate to right output with channel normalization
            for i in 0..self.fft_size {
                let mut sample = self.temp_time_buffer[i].re * fft_scale * channel_scale;
                // Flush denormals to zero to prevent CPU spikes and audio glitches
                if sample.abs() < 1e-30 {
                    sample = 0.0;
                }
                output[i * 2 + 1] += sample;
            }
        }
    }
}

impl Plugin for BinauralDecoderPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Binaural Decoder".to_string(),
            version: "1.1.0".to_string(),
            author: "AutoEQ".to_string(),
            description: format!(
                "Converts {}-channel audio to binaural stereo using HRTFs from SOFA file",
                self.input_channels
            ),
        }
    }

    fn input_channels(&self) -> usize {
        self.input_channels
    }

    fn output_channels(&self) -> usize {
        2 // Always stereo output
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![]
    }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        Err("No parameters available".to_string())
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;

        // Load SOFA file if path was provided
        if let Some(path) = self.sofa_path.clone() {
            self.load_sofa(path)
                .map_err(|e| format!("Failed to load SOFA file: {}", e))?;

            // Check sample rate match
            if let Some(sofa) = &self.sofa
                && (sofa.sample_rate - sample_rate as f32).abs() > 1.0 {
                    log::info!(
                        "[BinauralDecoder] Warning: SOFA sample rate ({} Hz) differs from engine rate ({} Hz). \
                         This may cause incorrect spatialization. Consider resampling the SOFA file.",
                        sofa.sample_rate,
                        sample_rate
                    );
                }
        } else {
            log::debug!(
                "[BinauralDecoder] Warning: No SOFA file specified, plugin will pass through audio"
            );
        }

        Ok(())
    }

    fn reset(&mut self) {
        self.input_buffer_fill = 0;
        self.input_buffer.fill(0.0);
        for buf in &mut self.output_accumulator {
            buf.fill(0.0);
        }
        self.output_accumulator_fill = 0;
        self.next_add_position = 0;
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        let input_samples = context.num_frames * self.input_channels;
        let output_samples = context.num_frames * 2; // Stereo

        if input.len() != input_samples {
            return Err(format!(
                "Input size mismatch: expected {}, got {}",
                input_samples,
                input.len()
            ));
        }

        if output.len() != output_samples {
            return Err(format!(
                "Output size mismatch: expected {}, got {}",
                output_samples,
                output.len()
            ));
        }

        output.fill(0.0);

        let mut input_pos = 0;
        let mut output_pos = 0;

        // Main processing loop
        loop {
            // Step 1: Drain output accumulator
            let frames_available = (output.len() - output_pos) / 2;
            let frames_to_drain = self.output_accumulator_fill.min(frames_available);

            if frames_to_drain > 0 {
                for i in 0..frames_to_drain {
                    // Apply Soft Clipper (tanh) to prevent hard clipping
                    // This limits output to [-1.0, 1.0] range smoothly
                    output[output_pos + i * 2] = self.output_accumulator[0][i].tanh();
                    output[output_pos + i * 2 + 1] = self.output_accumulator[1][i].tanh();
                }
                output_pos += frames_to_drain * 2;

                // Shift accumulator
                for ch in 0..2 {
                    self.output_accumulator[ch]
                        .copy_within(frames_to_drain..self.output_accumulator_fill, 0);
                    for i in (self.output_accumulator_fill - frames_to_drain)
                        ..self.output_accumulator_fill
                    {
                        self.output_accumulator[ch][i] = 0.0;
                    }
                }
                self.output_accumulator_fill -= frames_to_drain;
                self.next_add_position = self.next_add_position.saturating_sub(frames_to_drain);

                if self.output_accumulator_fill == 0 {
                    self.next_add_position = 0;
                }
            }

            // Step 2: Process block if we have enough input (hop_size) and accumulator space
            let input_needed = self.hop_size * self.input_channels;
            let can_process_input = self.input_buffer_fill >= input_needed;
            let can_process_space = self.next_add_position + self.fft_size <= self.fft_size * 2;

            if can_process_input && can_process_space {
                // Copy to temp buffer (direct copy, already interleaved)
                self.temp_input_block[..input_needed]
                    .copy_from_slice(&self.input_buffer[..input_needed]);

                // Process block
                // Use std::mem::take to temporarily move buffers out to avoid borrow conflict
                let input_block = std::mem::take(&mut self.temp_input_block);
                let mut output_block = std::mem::take(&mut self.temp_output_block);
                
                self.process_block(&input_block, &mut output_block);
                
                // Move buffers back
                self.temp_input_block = input_block;
                self.temp_output_block = output_block;

                // Accumulate output (overlap-add)
                for i in 0..self.fft_size {
                    self.output_accumulator[0][self.next_add_position + i] +=
                        self.temp_output_block[i * 2];
                    self.output_accumulator[1][self.next_add_position + i] +=
                        self.temp_output_block[i * 2 + 1];
                }

                // Update state
                // In standard OLA, we advance by hop_size
                self.next_add_position += self.hop_size;
                
                // Update fill count.
                let new_end = (self.next_add_position - self.hop_size) + self.fft_size;
                self.output_accumulator_fill = self.output_accumulator_fill.max(new_end);

                // Shift input buffer by hop_size (interleaved)
                let shift_amount = self.hop_size * self.input_channels;
                self.input_buffer
                    .copy_within(shift_amount..self.input_buffer_fill, 0);
                self.input_buffer_fill -= shift_amount;

                continue;
            }

            // Step 3: Fill input buffer
            if input_pos < input.len() {
                let input_needed = self.hop_size * self.input_channels;
                let samples_to_copy = (input.len() - input_pos)
                    .min(input_needed - self.input_buffer_fill);

                self.input_buffer[self.input_buffer_fill..self.input_buffer_fill + samples_to_copy]
                    .copy_from_slice(&input[input_pos..input_pos + samples_to_copy]);

                self.input_buffer_fill += samples_to_copy;
                input_pos += samples_to_copy;

                continue;
            }

            // Exit conditions
            let no_space_to_drain = (output.len() - output_pos) / 2 == 0;
            if no_space_to_drain {
                break;
            }

            let input_needed = self.hop_size * self.input_channels;
            let cant_process = self.input_buffer_fill < input_needed
                || self.next_add_position + self.fft_size > self.fft_size * 2;
            let no_data_to_drain = self.output_accumulator_fill == 0;

            if input_pos >= input.len() && cant_process && no_data_to_drain {
                break;
            }
        }

        Ok(())
    }

    fn latency_samples(&self) -> usize {
        self.hop_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binaural_decoder_creation() {
        let plugin = BinauralDecoderPlugin::new(5, 4096, None);
        assert_eq!(plugin.input_channels(), 5);
        assert_eq!(plugin.output_channels(), 2);
        assert_eq!(plugin.fft_size, 4096);
        assert_eq!(plugin.hop_size, 2048);
    }

    #[test]
    fn test_speaker_configs() {
        use super::super::speaker_config::get_speaker_config;

        let config_2_0 = get_speaker_config("2.0").unwrap();
        assert_eq!(config_2_0.total_channels, 2);

        let config_5_0 = get_speaker_config("5.0").unwrap();
        assert_eq!(config_5_0.total_channels, 5);

        let config_5_1 = get_speaker_config("5.1").unwrap();
        assert_eq!(config_5_1.total_channels, 6);
    }
}
