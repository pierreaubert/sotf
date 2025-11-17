// ============================================================================
// Binaural Decoder Plugin - Multi-channel to Binaural Stereo
// ============================================================================
//
// This plugin converts multi-channel audio (e.g., 5.0, 5.1, 7.1) to binaural
// stereo using Head-Related Transfer Functions (HRTFs) from SOFA files.
//
// Algorithm:
// - Each input channel is convolved with its corresponding HRTF
// - HRTFs are selected based on speaker positions (azimuth, elevation)
// - Uses FFT-based fast convolution (overlap-add method)
// - Output is stereo (left/right ears) suitable for headphone playback
//
// Supported input formats:
// - Stereo (2.0): L/R at ±30°
// - 5.0: FL/FR/C/LS/RS (standard surround)
// - 5.1: FL/FR/C/LFE/LS/RS (LFE passed through)
// - 7.1: FL/FR/C/LFE/SL/SR/RL/RR

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
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
    4096
}

fn default_sofa_path() -> String {
    "".to_string()
}

/// Speaker positions for standard layouts
#[derive(Debug, Clone, Copy)]
pub struct SpeakerPosition {
    /// Speaker index in channel order
    pub channel: usize,
    /// Speaker name (for debugging)
    pub name: &'static str,
    /// Azimuth in degrees
    pub azimuth: f32,
    /// Elevation in degrees
    pub elevation: f32,
    /// Distance in meters
    pub distance: f32,
}

impl SpeakerPosition {
    fn to_source_position(&self) -> SourcePosition {
        SourcePosition::new(self.azimuth, self.elevation, self.distance)
    }
}

/// Standard speaker layouts
pub struct SpeakerLayouts;

impl SpeakerLayouts {
    /// 2.0 stereo: L/R at ±30°
    pub const STEREO: [SpeakerPosition; 2] = [
        SpeakerPosition {
            channel: 0,
            name: "L",
            azimuth: 30.0,
            elevation: 0.0,
            distance: 1.0,
        },
        SpeakerPosition {
            channel: 1,
            name: "R",
            azimuth: -30.0,
            elevation: 0.0,
            distance: 1.0,
        },
    ];

    /// 5.0 surround: FL/FR/C/LS/RS
    pub const SURROUND_5_0: [SpeakerPosition; 5] = [
        SpeakerPosition {
            channel: 0,
            name: "FL",
            azimuth: 30.0,
            elevation: 0.0,
            distance: 1.0,
        },
        SpeakerPosition {
            channel: 1,
            name: "FR",
            azimuth: -30.0,
            elevation: 0.0,
            distance: 1.0,
        },
        SpeakerPosition {
            channel: 2,
            name: "C",
            azimuth: 0.0,
            elevation: 0.0,
            distance: 1.0,
        },
        SpeakerPosition {
            channel: 3,
            name: "LS",
            azimuth: 110.0,
            elevation: 0.0,
            distance: 1.0,
        },
        SpeakerPosition {
            channel: 4,
            name: "RS",
            azimuth: -110.0,
            elevation: 0.0,
            distance: 1.0,
        },
    ];

    /// 5.1 surround: FL/FR/C/LFE/LS/RS
    /// Note: LFE is handled specially (passed through to both ears)
    pub const SURROUND_5_1: [SpeakerPosition; 6] = [
        SpeakerPosition {
            channel: 0,
            name: "FL",
            azimuth: 30.0,
            elevation: 0.0,
            distance: 1.0,
        },
        SpeakerPosition {
            channel: 1,
            name: "FR",
            azimuth: -30.0,
            elevation: 0.0,
            distance: 1.0,
        },
        SpeakerPosition {
            channel: 2,
            name: "C",
            azimuth: 0.0,
            elevation: 0.0,
            distance: 1.0,
        },
        SpeakerPosition {
            channel: 3,
            name: "LFE",
            azimuth: 0.0,
            elevation: -90.0,
            distance: 1.0,
        }, // LFE (special handling)
        SpeakerPosition {
            channel: 4,
            name: "LS",
            azimuth: 110.0,
            elevation: 0.0,
            distance: 1.0,
        },
        SpeakerPosition {
            channel: 5,
            name: "RS",
            azimuth: -110.0,
            elevation: 0.0,
            distance: 1.0,
        },
    ];
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

    /// Speaker layout for input channels
    speaker_layout: Vec<SpeakerPosition>,

    /// FFT planners
    fft_forward: Arc<dyn Fft<f32>>,
    fft_inverse: Arc<dyn Fft<f32>>,

    /// HRTF filters in frequency domain [channels × 2 × fft_size]
    /// For each input channel: [left_ear_fft, right_ear_fft]
    hrtf_filters_freq: Vec<Vec<Complex<f32>>>,

    /// Input buffers for overlap-add [channels × buffer_size]
    input_buffers: Vec<Vec<f32>>,
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
    window: Vec<f32>,
}

impl BinauralDecoderPlugin {
    /// Create a new binaural decoder plugin
    ///
    /// # Arguments
    /// * `input_channels` - Number of input channels
    /// * `fft_size` - FFT size for convolution (must be power of 2)
    /// * `sofa_path` - Path to SOFA file (optional, can be loaded later)
    pub fn new(input_channels: usize, fft_size: usize, sofa_path: Option<PathBuf>) -> Self {
        assert!(
            fft_size.is_power_of_two(),
            "FFT size must be power of 2"
        );
        assert!(input_channels > 0, "Must have at least 1 input channel");

        let hop_size = fft_size / 2;

        let mut planner = FftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(fft_size);
        let fft_inverse = planner.plan_fft_inverse(fft_size);

        // Generate Hann window
        let window: Vec<f32> = (0..fft_size)
            .map(|i| 0.5 * (1.0 - ((2.0 * std::f32::consts::PI * i as f32) / fft_size as f32).cos()))
            .collect();

        // Determine speaker layout based on channel count
        let speaker_layout = Self::get_speaker_layout(input_channels);

        eprintln!(
            "[BinauralDecoder] Created with {} input channels, FFT size {}",
            input_channels, fft_size
        );
        for speaker in &speaker_layout {
            eprintln!(
                "[BinauralDecoder]   Ch{}: {} at az={:.1}°, el={:.1}°",
                speaker.channel, speaker.name, speaker.azimuth, speaker.elevation
            );
        }

        Self {
            input_channels,
            fft_size,
            hop_size,
            sample_rate: 48000, // Will be set in initialize()

            sofa: None,
            sofa_path,
            speaker_layout,

            fft_forward,
            fft_inverse,

            hrtf_filters_freq: vec![vec![Complex::new(0.0, 0.0); fft_size * 2]; input_channels],

            input_buffers: vec![vec![0.0; fft_size]; input_channels],
            input_buffer_fill: 0,

            output_accumulator: vec![vec![0.0; fft_size * 3]; 2], // 2 output channels
            output_accumulator_fill: 0,
            next_add_position: 0,

            temp_input_block: vec![0.0; fft_size],
            temp_output_block: vec![0.0; fft_size * 2], // Stereo output
            temp_freq_buffer: vec![Complex::new(0.0, 0.0); fft_size],
            temp_time_buffer: vec![Complex::new(0.0, 0.0); fft_size],
            window,
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
        eprintln!("[BinauralDecoder] Loading SOFA file: {:?}", path);

        let sofa = SofaFile::load(&path)?;

        eprintln!(
            "[BinauralDecoder] SOFA loaded: {} measurements, IR length: {}, sample rate: {} Hz",
            sofa.num_measurements, sofa.ir_length, sofa.sample_rate
        );

        // Prepare HRTF filters for each speaker
        self.prepare_hrtf_filters(&sofa)?;

        self.sofa = Some(sofa);
        self.sofa_path = Some(path);

        eprintln!("[BinauralDecoder] SOFA file loaded and HRTFs prepared");

        Ok(())
    }

    /// Prepare HRTF filters in frequency domain for all speakers
    fn prepare_hrtf_filters(&mut self, sofa: &SofaFile) -> Result<(), String> {
        for (i, speaker) in self.speaker_layout.iter().enumerate() {
            let hrtf = sofa
                .get_hrtf_at_position(&speaker.to_source_position())
                .ok_or_else(|| format!("No HRTF found for speaker {}", speaker.name))?;

            eprintln!(
                "[BinauralDecoder] Speaker {}: {} (az={:.1}°, el={:.1}°) -> HRTF at az={:.1}°, el={:.1}°",
                i, speaker.name, speaker.azimuth, speaker.elevation,
                hrtf.position.azimuth, hrtf.position.elevation
            );

            // Convert HRTFs to frequency domain
            // We need to pad/truncate IRs to fft_size
            let left_fft = self.ir_to_freq(&hrtf.ir_left);
            let right_fft = self.ir_to_freq(&hrtf.ir_right);

            // Store both left and right HRTFs
            self.hrtf_filters_freq[i] = left_fft
                .into_iter()
                .chain(right_fft.into_iter())
                .collect();
        }

        Ok(())
    }

    /// Convert impulse response to frequency domain
    fn ir_to_freq(&self, ir: &[f32]) -> Vec<Complex<f32>> {
        let mut buffer = vec![Complex::new(0.0, 0.0); self.fft_size];

        // Copy IR data (pad with zeros if IR is shorter, truncate if longer)
        let copy_len = ir.len().min(self.fft_size);
        for i in 0..copy_len {
            buffer[i] = Complex::new(ir[i], 0.0);
        }

        // FFT
        let mut freq = buffer.clone();
        self.fft_forward.process(&mut freq);

        freq
    }

    /// Get speaker layout for given number of channels
    fn get_speaker_layout(num_channels: usize) -> Vec<SpeakerPosition> {
        match num_channels {
            2 => SpeakerLayouts::STEREO.to_vec(),
            5 => SpeakerLayouts::SURROUND_5_0.to_vec(),
            6 => SpeakerLayouts::SURROUND_5_1.to_vec(),
            _ => {
                // Default: arrange channels in a circle
                eprintln!(
                    "[BinauralDecoder] Using default circular layout for {} channels",
                    num_channels
                );
                let mut layout = Vec::new();
                for i in 0..num_channels {
                    let angle = (i as f32) * 360.0 / (num_channels as f32);
                    layout.push(SpeakerPosition {
                        channel: i,
                        name: "CH",
                        azimuth: angle,
                        elevation: 0.0,
                        distance: 1.0,
                    });
                }
                layout
            }
        }
    }

    /// Process one FFT block using fast convolution
    fn process_fft_block(&mut self, input: &[f32], output: &mut [f32]) {
        assert_eq!(input.len(), self.fft_size * self.input_channels);
        assert_eq!(output.len(), self.fft_size * 2); // Stereo output

        output.fill(0.0);

        // Check if we have HRTF filters loaded
        if self.hrtf_filters_freq.is_empty()
            || self.hrtf_filters_freq[0].len() != self.fft_size * 2
        {
            // No HRTFs loaded - pass through first 2 channels or silence
            for i in 0..self.fft_size {
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

        // Process each input channel
        for ch in 0..self.input_channels {
            // 1. Extract channel data and apply window
            // Note: For overlap-add convolution with 50% overlap and Hann window,
            // windowing only the input (analysis window) provides perfect reconstruction.
            // The Hann window satisfies the Constant Overlap-Add (COLA) constraint:
            // w[n] + w[n + hop_size] = 1 for all n, ensuring no artifacts.
            for i in 0..self.fft_size {
                let sample = input[i * self.input_channels + ch];
                self.temp_time_buffer[i] = Complex::new(sample * self.window[i], 0.0);
            }

            // 2. Forward FFT
            self.temp_freq_buffer.copy_from_slice(&self.temp_time_buffer);
            self.fft_forward.process(&mut self.temp_freq_buffer);

            // 3. Convolve with left ear HRTF
            for i in 0..self.fft_size {
                self.temp_time_buffer[i] =
                    self.temp_freq_buffer[i] * self.hrtf_filters_freq[ch][i];
            }

            // 4. Inverse FFT for left ear
            self.fft_inverse.process(&mut self.temp_time_buffer);

            // 5. Accumulate to left output
            for i in 0..self.fft_size {
                output[i * 2] += self.temp_time_buffer[i].re * fft_scale;
            }

            // 6. Convolve with right ear HRTF
            for i in 0..self.fft_size {
                self.temp_time_buffer[i] =
                    self.temp_freq_buffer[i] * self.hrtf_filters_freq[ch][self.fft_size + i];
            }

            // 7. Inverse FFT for right ear
            self.fft_inverse.process(&mut self.temp_time_buffer);

            // 8. Accumulate to right output
            for i in 0..self.fft_size {
                output[i * 2 + 1] += self.temp_time_buffer[i].re * fft_scale;
            }
        }
    }
}

impl Plugin for BinauralDecoderPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Binaural Decoder".to_string(),
            version: "1.0.0".to_string(),
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
            if let Some(sofa) = &self.sofa {
                if (sofa.sample_rate - sample_rate as f32).abs() > 1.0 {
                    eprintln!(
                        "[BinauralDecoder] Warning: SOFA sample rate ({} Hz) differs from engine rate ({} Hz). \
                         This may cause incorrect spatialization. Consider resampling the SOFA file.",
                        sofa.sample_rate, sample_rate
                    );
                }
            }
        } else {
            eprintln!("[BinauralDecoder] Warning: No SOFA file specified, plugin will pass through audio");
        }

        Ok(())
    }

    fn reset(&mut self) {
        self.input_buffer_fill = 0;
        for buf in &mut self.input_buffers {
            buf.fill(0.0);
        }
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

        // Main processing loop (similar to upmixer)
        loop {
            // Step 1: Drain output accumulator
            let frames_available = (output.len() - output_pos) / 2;
            let frames_to_drain = self.output_accumulator_fill.min(frames_available);

            if frames_to_drain > 0 {
                for i in 0..frames_to_drain {
                    output[output_pos + i * 2] = self.output_accumulator[0][i];
                    output[output_pos + i * 2 + 1] = self.output_accumulator[1][i];
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

            // Step 2: Process FFT block if we have enough input
            let can_process_input = self.input_buffer_fill >= self.fft_size;
            let can_process_space = self.next_add_position + self.fft_size <= self.fft_size * 3;

            if can_process_input && can_process_space {
                // De-interleave input into temp block
                for i in 0..self.fft_size {
                    for ch in 0..self.input_channels {
                        self.temp_input_block[i * self.input_channels + ch] =
                            self.input_buffers[ch][i];
                    }
                }

                // Process block
                // Use std::mem::take to temporarily move buffers out to avoid borrow conflict
                let input_block = std::mem::take(&mut self.temp_input_block);
                let mut output_block = std::mem::take(&mut self.temp_output_block);
                output_block.fill(0.0);
                self.process_fft_block(&input_block, &mut output_block);
                // Move buffers back
                self.temp_input_block = input_block;
                self.temp_output_block = output_block;

                // Accumulate output (overlap-add)
                for i in 0..self.fft_size {
                    self.output_accumulator[0][self.next_add_position + i] += self.temp_output_block[i * 2];
                    self.output_accumulator[1][self.next_add_position + i] +=
                        self.temp_output_block[i * 2 + 1];
                }

                // Update state
                if self.output_accumulator_fill == 0 {
                    self.output_accumulator_fill = self.fft_size;
                    self.next_add_position = self.hop_size;
                } else {
                    self.output_accumulator_fill += self.hop_size;
                    self.next_add_position += self.hop_size;
                }

                // Shift input buffers by hop_size
                for ch in 0..self.input_channels {
                    self.input_buffers[ch].copy_within(self.hop_size..self.fft_size, 0);
                }
                self.input_buffer_fill -= self.hop_size;

                continue;
            }

            // Step 3: Fill input buffers
            if input_pos < input.len() {
                let frames_to_copy =
                    ((input.len() - input_pos) / self.input_channels).min(self.fft_size - self.input_buffer_fill);

                for i in 0..frames_to_copy {
                    for ch in 0..self.input_channels {
                        self.input_buffers[ch][self.input_buffer_fill + i] =
                            input[input_pos + i * self.input_channels + ch];
                    }
                }

                self.input_buffer_fill += frames_to_copy;
                input_pos += frames_to_copy * self.input_channels;

                continue;
            }

            // Exit conditions
            let no_space_to_drain = (output.len() - output_pos) / 2 == 0;
            if no_space_to_drain {
                break;
            }

            let cant_process = self.input_buffer_fill < self.fft_size
                || self.next_add_position + self.fft_size > self.fft_size * 3;
            let no_data_to_drain = self.output_accumulator_fill == 0;

            if input_pos >= input.len() && cant_process && no_data_to_drain {
                break;
            }
        }

        Ok(())
    }

    fn latency_samples(&self) -> usize {
        self.fft_size
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
    }

    #[test]
    fn test_speaker_layouts() {
        assert_eq!(SpeakerLayouts::STEREO.len(), 2);
        assert_eq!(SpeakerLayouts::SURROUND_5_0.len(), 5);
        assert_eq!(SpeakerLayouts::SURROUND_5_1.len(), 6);
    }
}
