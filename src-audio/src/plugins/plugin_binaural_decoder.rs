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

fn default_enable_optimization() -> bool {
    true
}

fn default_externalization() -> f32 {
    0.0
}

fn default_near_field_strength() -> f32 {
    0.0
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
    /// Enable Sum-Before-IFFT optimization
    #[serde(default = "default_enable_optimization")]
    pub enable_optimization: bool,
    /// Externalization factor (0.0 to 1.0)
    #[serde(default = "default_externalization")]
    pub externalization: f32,
    /// Near-field shadowing strength (0.0 to 1.0)
    #[serde(default = "default_near_field_strength")]
    pub near_field_strength: f32,
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

    // Parameters
    param_enable_optimization: ParameterId,
    enable_optimization: bool,
    param_externalization: ParameterId,
    externalization: f32,
    param_near_field_strength: ParameterId,
    near_field_strength: f32,
}

impl BinauralDecoderPlugin {
    /// Create a new binaural decoder plugin
    ///
    /// # Arguments
    /// * `input_channels` - Number of input channels
    /// * `fft_size` - FFT size for convolution (must be power of 2)
    /// * `sofa_path` - Path to SOFA file (optional, can be loaded later)
    /// * `enable_optimization` - Enable Sum-Before-IFFT optimization
    /// * `externalization` - Externalization factor (0.0 to 1.0)
    /// * `near_field_strength` - Near-field shadowing strength (0.0 to 1.0)
    pub fn new(
        input_channels: usize,
        fft_size: usize,
        sofa_path: Option<PathBuf>,
        enable_optimization: bool,
        externalization: f32,
        near_field_strength: f32,
    ) -> Self {
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
            temp_output_block: vec![0.0; fft_size * 2],             // Stereo output
            temp_freq_buffer: vec![Complex::new(0.0, 0.0); fft_size],
            temp_time_buffer: vec![Complex::new(0.0, 0.0); fft_size],

            param_enable_optimization: ParameterId::from("enable_optimization"),
            enable_optimization,
            param_externalization: ParameterId::from("externalization"),
            externalization,
            param_near_field_strength: ParameterId::from("near_field_strength"),
            near_field_strength,
        }
    }

    /// Create from parameters
    pub fn from_params(params: BinauralDecoderParams) -> Self {
        let sofa_path = if params.sofa_file.is_empty() {
            None
        } else {
            Some(PathBuf::from(params.sofa_file))
        };

        Self::new(
            params.input_channels,
            params.fft_size,
            sofa_path,
            params.enable_optimization,
            params.externalization,
            params.near_field_strength,
        )
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

        // Store SOFA first so prepare_hrtf_filters can use it
        self.sofa = Some(sofa);
        self.sofa_path = Some(path);

        // Prepare HRTF filters for each speaker
        self.prepare_hrtf_filters()?;

        log::debug!("[BinauralDecoder] SOFA file loaded and HRTFs prepared");

        Ok(())
    }

    /// Prepare HRTF filters in frequency domain for all speakers
    fn prepare_hrtf_filters(&mut self) -> Result<(), String> {
        let sofa = self.sofa.as_ref().ok_or("SOFA file not loaded")?;
        
        for (i, speaker) in self.speaker_config.speakers.iter().enumerate() {
            let target_pos = speaker_to_source_position(speaker);
            
            // Find 3 nearest HRTFs
            let nearest = sofa.find_three_nearest(&target_pos);
            
            // Calculate VBAP gains
            let gains = Self::calculate_vbap_gains(&target_pos, &nearest, sofa);
            
            log::info!(
                "[BinauralDecoder] Speaker {}: {} (az={:.1}°, el={:.1}°) -> VBAP: {:.2}*[{}] + {:.2}*[{}] + {:.2}*[{}]",
                i,
                speaker.name,
                speaker.azimuth,
                speaker.elevation,
                gains[0], nearest[0].0,
                gains[1], nearest[1].0,
                gains[2], nearest[2].0
            );

            // Interpolate HRTF
            let mut ir_left = vec![0.0; self.hop_size];
            let mut ir_right = vec![0.0; self.hop_size];
            
            for (k, (idx, _)) in nearest.iter().enumerate() {
                if let Some(hrtf) = sofa.get_hrtf(*idx) {
                    // Add weighted contribution
                    // Truncate to hop_size
                    let len = self.hop_size.min(hrtf.ir_left.len());
                    for s in 0..len {
                        ir_left[s] += hrtf.ir_left[s] * gains[k];
                        ir_right[s] += hrtf.ir_right[s] * gains[k];
                    }
                }
            }

            // Convert HRTFs to frequency domain
            let mut left_fft = self.ir_to_freq(&ir_left);
            let mut right_fft = self.ir_to_freq(&ir_right);
            
            // Apply Near-Field Shadowing
            if self.near_field_strength > 0.0 {
                let az = speaker.azimuth;
                // Shadowing effect depends on azimuth (max at +/- 90 degrees)
                let shadow_amount = (az.abs() / 90.0).min(1.0) * self.near_field_strength;
                
                if shadow_amount > 0.01 {
                    // Simple LPF simulation: attenuate high frequencies on contralateral ear
                    // H(f) = 1 / sqrt(1 + (f/fc)^2)
                    // fc decreases as shadow_amount increases
                    
                    // Map shadow_amount 0.0-1.0 to fc 20kHz-500Hz
                    // Logarithmic mapping feels more natural
                    let min_fc = 500.0f32;
                    let max_fc = 20000.0f32;
                    let fc = max_fc * (min_fc / max_fc).powf(shadow_amount);
                    
                    for k in 0..self.fft_size {
                        // Frequency for bin k
                        let freq = if k <= self.fft_size / 2 {
                            k as f32 * self.sample_rate as f32 / self.fft_size as f32
                        } else {
                            (self.fft_size - k) as f32 * self.sample_rate as f32 / self.fft_size as f32
                        };
                        
                        let gain = 1.0 / (1.0 + (freq / fc).powi(2)).sqrt();
                        
                        if az > 0.0 {
                            // Source is Left (positive azimuth), shadow Right ear
                            right_fft[k] = right_fft[k] * gain;
                        } else {
                            // Source is Right (negative azimuth), shadow Left ear
                            left_fft[k] = left_fft[k] * gain;
                        }
                    }
                }
            }

            // Store both left and right HRTFs
            let combined: Vec<Complex<f32>> =
                left_fft.into_iter().chain(right_fft.into_iter()).collect();

            debug_assert_eq!(
                combined.len(),
                self.fft_size * 2,
                "combined HRTF has wrong length"
            );

            self.hrtf_filters_freq[i] = combined;
        }

        Ok(())
    }

    /// Calculate VBAP gains for 3 source positions relative to a target
    fn calculate_vbap_gains(
        target: &SourcePosition,
        nearest: &[(usize, f32); 3],
        sofa: &SofaFile,
    ) -> [f32; 3] {
        let p = target.to_cartesian_unit_vector();
        
        let l1 = sofa.positions[nearest[0].0].to_cartesian_unit_vector();
        let l2 = sofa.positions[nearest[1].0].to_cartesian_unit_vector();
        let l3 = sofa.positions[nearest[2].0].to_cartesian_unit_vector();
        
        // Matrix L = [l1, l2, l3] (columns)
        // We need L^-1 * p
        
        // Invert 3x3 matrix manually
        // | a b c |
        // | d e f |
        // | g h i |
        
        let a = l1[0]; let b = l2[0]; let c = l3[0];
        let d = l1[1]; let e = l2[1]; let f = l3[1];
        let g = l1[2]; let h = l2[2]; let i = l3[2];
        
        let det = a*(e*i - f*h) - b*(d*i - f*g) + c*(d*h - e*g);
        
        if det.abs() < 1e-6 {
            // Singular matrix (collinear points), fallback to nearest neighbor
            return [1.0, 0.0, 0.0];
        }
        
        let inv_det = 1.0 / det;
        
        // Inverse matrix elements
        let ia = (e*i - f*h) * inv_det;
        let ib = (c*h - b*i) * inv_det;
        let ic = (b*f - c*e) * inv_det;
        let id = (f*g - d*i) * inv_det;
        let ie = (a*i - c*g) * inv_det;
        let if_ = (c*d - a*f) * inv_det; // if is keyword
        let ig = (d*h - e*g) * inv_det;
        let ih = (b*g - a*h) * inv_det;
        let ii = (a*e - b*d) * inv_det;
        
        // Multiply by p
        let g1 = ia*p[0] + ib*p[1] + ic*p[2];
        let g2 = id*p[0] + ie*p[1] + if_*p[2];
        let g3 = ig*p[0] + ih*p[1] + ii*p[2];
        
        // Normalize energy
        // Avoid negative gains? VBAP allows negative gains but usually we clamp or keep them.
        // Negative gains can cause phase issues.
        // Ideally we select the triangle that encloses the point so gains are non-negative.
        // Since we just picked 3 nearest, we might get negative gains.
        // We can clamp them to 0? Or just use absolute values?
        // Or just use them as is (might cause comb filtering).
        // Let's clamp to 0 for safety and re-normalize.
        
        let g1 = g1.max(0.0);
        let g2 = g2.max(0.0);
        let g3 = g3.max(0.0);
        
        let energy = g1*g1 + g2*g2 + g3*g3;
        if energy > 0.0 {
            let scale = 1.0 / energy.sqrt();
            [g1 * scale, g2 * scale, g3 * scale]
        } else {
            [1.0, 0.0, 0.0]
        }
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
            log::warn!(
                "[BinauralDecoder] HRTF IR peak is very high: {:.4} (near 0dBFS). This might cause clipping.",
                max_val
            );
        } else {
            log::debug!("[BinauralDecoder] HRTF IR peak: {:.4}", max_val);
        }

        // FFT
        let mut freq = buffer.clone();
        self.fft_forward.process(&mut freq);

        freq
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
        vec![
            Parameter::new_bool("enable_optimization", "Optimization", true)
                .with_description("Enable Sum-Before-IFFT optimization"),
            Parameter::new_float("externalization", "Externalization", 0.0, 0.0, 1.0)
                .with_description("Room simulation / externalization factor"),
            Parameter::new_float("near_field_strength", "Near-Field", 0.0, 0.0, 1.0)
                .with_description("Near-field shadowing strength"),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_enable_optimization {
            if let Some(v) = value.as_bool() {
                self.enable_optimization = v;
                return Ok(());
            }
        } else if id == self.param_externalization {
            if let Some(v) = value.as_float() {
                if (0.0..=1.0).contains(&v) {
                    self.externalization = v;
                    return Ok(());
                }
            }
        } else if id == self.param_near_field_strength {
            if let Some(v) = value.as_float() {
                if (0.0..=1.0).contains(&v) {
                    self.near_field_strength = v;
                    // Re-calculate filters to apply shadowing
                    if self.sofa.is_some() {
                        self.prepare_hrtf_filters().map_err(|e| format!("Failed to update filters: {}", e))?;
                    }
                    return Ok(());
                }
            }
        }
        Err(format!("Unknown parameter or invalid value: {}", id))
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_enable_optimization {
            Some(ParameterValue::Bool(self.enable_optimization))
        } else if id == &self.param_externalization {
            Some(ParameterValue::Float(self.externalization))
        } else if id == &self.param_near_field_strength {
            Some(ParameterValue::Float(self.near_field_strength))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;

        // Load SOFA file if path was provided
        if let Some(path) = self.sofa_path.clone() {
            self.load_sofa(path)
                .map_err(|e| format!("Failed to load SOFA file: {}", e))?;

            // Check sample rate match
            if let Some(sofa) = &self.sofa
                && (sofa.sample_rate - sample_rate as f32).abs() > 1.0
            {
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

        // If no SOFA file is loaded, act as passthrough
        if self.sofa.is_none() {
            for frame in 0..context.num_frames {
                if self.input_channels == 1 {
                    // Mono -> Stereo: duplicate to both channels
                    let sample = input[frame];
                    output[frame * 2] = sample;
                    output[frame * 2 + 1] = sample;
                } else if self.input_channels == 2 {
                    // Stereo -> Stereo: direct copy
                    output[frame * 2] = input[frame * 2];
                    output[frame * 2 + 1] = input[frame * 2 + 1];
                } else {
                    // Multi-channel -> Stereo: take first two channels
                    output[frame * 2] = input[frame * self.input_channels];
                    output[frame * 2 + 1] = input[frame * self.input_channels + 1];
                }
            }
            return Ok(());
        }

        let start_time = std::time::Instant::now();

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

                // Clear temp freq buffer for optimization
                self.temp_freq_buffer.fill(Complex::new(0.0, 0.0));

                if self.enable_optimization {
                    // OPTIMIZATION: Sum-Before-IFFT
                    // 1. Transform all input channels to frequency domain
                    // 2. Multiply by HRTFs and sum in frequency domain (Left and Right sums)
                    // 3. Perform only 2 IFFTs (one for Left, one for Right)
                    
                    // We need two accumulators in frequency domain
                    let mut sum_left = vec![Complex::new(0.0, 0.0); self.fft_size];
                    let mut sum_right = vec![Complex::new(0.0, 0.0); self.fft_size];
                    
                    // Re-use temp_time_buffer for input FFT
                    
                    for ch in 0..self.input_channels {
                        // Extract input channel to temp buffer
                        for i in 0..self.hop_size {
                            self.temp_time_buffer[i] = Complex::new(input_block[i * self.input_channels + ch], 0.0);
                        }
                        // Zero pad
                        for i in self.hop_size..self.fft_size {
                            self.temp_time_buffer[i] = Complex::new(0.0, 0.0);
                        }
                        
                        // FFT
                        self.fft_forward.process(&mut self.temp_time_buffer);
                        
                        // Multiply and accumulate
                        let hrtf = &self.hrtf_filters_freq[ch];
                        // hrtf is [left_fft, right_fft] concatenated
                        
                        for k in 0..self.fft_size {
                            sum_left[k] += self.temp_time_buffer[k] * hrtf[k];
                            sum_right[k] += self.temp_time_buffer[k] * hrtf[self.fft_size + k];
                        }
                    }
                    
                    // IFFT Left
                    self.fft_inverse.process(&mut sum_left);
                    // IFFT Right
                    self.fft_inverse.process(&mut sum_right);
                    
                    // Scale and output
                    let scale = 1.0 / self.fft_size as f32;
                    for i in 0..self.fft_size {
                        output_block[i * 2] = sum_left[i].re * scale;
                        output_block[i * 2 + 1] = sum_right[i].re * scale;
                    }
                    
                } else {
                    // Standard per-channel IFFT (Reference implementation)
                    output_block.fill(0.0);
                    
                    for ch in 0..self.input_channels {
                        // Extract input channel
                        for i in 0..self.hop_size {
                            self.temp_time_buffer[i] = Complex::new(input_block[i * self.input_channels + ch], 0.0);
                        }
                        for i in self.hop_size..self.fft_size {
                            self.temp_time_buffer[i] = Complex::new(0.0, 0.0);
                        }
                        
                        // FFT
                        self.fft_forward.process(&mut self.temp_time_buffer);
                        
                        // Convolve with Left HRTF
                        for k in 0..self.fft_size {
                            self.temp_freq_buffer[k] = self.temp_time_buffer[k] * self.hrtf_filters_freq[ch][k];
                        }
                        self.fft_inverse.process(&mut self.temp_freq_buffer);
                        
                        // Accumulate Left
                        let scale = 1.0 / self.fft_size as f32;
                        for i in 0..self.fft_size {
                            output_block[i * 2] += self.temp_freq_buffer[i].re * scale;
                        }
                        
                        // Convolve with Right HRTF
                        for k in 0..self.fft_size {
                            self.temp_freq_buffer[k] = self.temp_time_buffer[k] * self.hrtf_filters_freq[ch][self.fft_size + k];
                        }
                        self.fft_inverse.process(&mut self.temp_freq_buffer);
                        
                        // Accumulate Right
                        for i in 0..self.fft_size {
                            output_block[i * 2 + 1] += self.temp_freq_buffer[i].re * scale;
                        }
                    }
                }

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
                let samples_to_copy =
                    (input.len() - input_pos).min(input_needed - self.input_buffer_fill);

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
        
        let elapsed = start_time.elapsed();
        if elapsed > std::time::Duration::from_millis(3) {
            log::warn!(
                "[BinauralDecoder] Slow processing: {:.2}ms for {} input frames",
                elapsed.as_secs_f64() * 1000.0,
                context.num_frames
            );
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
        let plugin = BinauralDecoderPlugin::new(5, 4096, None, true, 0.0, 0.0);
        assert_eq!(plugin.input_channels(), 5);
        assert_eq!(plugin.output_channels(), 2);
        assert_eq!(plugin.fft_size, 4096);
        assert_eq!(plugin.hop_size, 2048);
        assert_eq!(plugin.enable_optimization, true);
        assert_eq!(plugin.externalization, 0.0);
        assert_eq!(plugin.near_field_strength, 0.0);
    }

    #[test]
    fn test_binaural_decoder_parameters() {
        let mut plugin = BinauralDecoderPlugin::new(2, 2048, None, true, 0.0, 0.0);
        
        // Test optimization
        plugin.set_parameter(ParameterId::from("enable_optimization"), ParameterValue::Bool(false)).unwrap();
        assert_eq!(plugin.enable_optimization, false);
        
        // Test externalization
        plugin.set_parameter(ParameterId::from("externalization"), ParameterValue::Float(0.5)).unwrap();
        assert_eq!(plugin.externalization, 0.5);
        
        // Test near-field
        plugin.set_parameter(ParameterId::from("near_field_strength"), ParameterValue::Float(0.8)).unwrap();
        assert_eq!(plugin.near_field_strength, 0.8);
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

    #[test]
    fn test_passthrough_without_sofa() {
        // Test stereo passthrough
        let mut plugin = BinauralDecoderPlugin::new(2, 2048, None, true, 0.0, 0.0);
        plugin.initialize(48000).unwrap();

        let input = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6]; // 3 stereo frames
        let mut output = vec![0.0; 6];
        let context = ProcessContext {
            num_frames: 3,
            sample_rate: 48000,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Should pass through directly
        assert_eq!(output, input);
    }

    #[test]
    fn test_passthrough_mono_to_stereo() {
        // Test mono to stereo passthrough
        let mut plugin = BinauralDecoderPlugin::new(1, 2048, None, true, 0.0, 0.0);
        plugin.initialize(48000).unwrap();

        let input = vec![0.1, 0.2, 0.3]; // 3 mono frames
        let mut output = vec![0.0; 6]; // 3 stereo frames
        let context = ProcessContext {
            num_frames: 3,
            sample_rate: 48000,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Mono should be duplicated to both channels
        assert_eq!(output[0], 0.1);
        assert_eq!(output[1], 0.1);
        assert_eq!(output[2], 0.2);
        assert_eq!(output[3], 0.2);
        assert_eq!(output[4], 0.3);
        assert_eq!(output[5], 0.3);
    }

    #[test]
    fn test_passthrough_multichannel_to_stereo() {
        // Test 5.0 to stereo passthrough (takes first 2 channels)
        let mut plugin = BinauralDecoderPlugin::new(5, 2048, None, true, 0.0, 0.0);
        plugin.initialize(48000).unwrap();

        // 2 frames of 5-channel audio: [FL, FR, C, SL, SR, FL, FR, C, SL, SR]
        let input = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
        let mut output = vec![0.0; 4]; // 2 stereo frames
        let context = ProcessContext {
            num_frames: 2,
            sample_rate: 48000,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Should take first 2 channels (FL, FR)
        assert_eq!(output[0], 0.1); // Frame 0, FL
        assert_eq!(output[1], 0.2); // Frame 0, FR
        assert_eq!(output[2], 0.6); // Frame 1, FL
        assert_eq!(output[3], 0.7); // Frame 1, FR
    }
}
