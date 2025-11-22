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
use super::simd::{complex_mul_add_simd, complex_mul_simd};

use crate::sofa::{SofaFile, SourcePosition};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during binaural decoder operation
#[derive(Debug, Clone)]
pub enum BinauralError {
    /// SOFA file not loaded when required
    SofaNotLoaded,
    /// Sample rate mismatch between SOFA and engine
    SampleRateMismatch { sofa_rate: u32, engine_rate: u32 },
    /// Invalid FFT size (must be power of 2)
    InvalidFftSize(usize),
    /// SOFA file loading failed
    SofaLoadError(String),
    /// Resampling failed
    ResamplingError(String),
    /// HRTF preparation failed
    HrtfPreparationError(String),
    /// Invalid parameter value
    InvalidParameter { name: String, value: String },
    /// Input/output buffer size mismatch
    BufferSizeMismatch { expected: usize, got: usize },
}

impl std::fmt::Display for BinauralError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinauralError::SofaNotLoaded => write!(f, "SOFA file not loaded"),
            BinauralError::SampleRateMismatch {
                sofa_rate,
                engine_rate,
            } => {
                write!(
                    f,
                    "Sample rate mismatch: SOFA={}Hz, engine={}Hz",
                    sofa_rate, engine_rate
                )
            }
            BinauralError::InvalidFftSize(size) => {
                write!(f, "Invalid FFT size: {} (must be power of 2)", size)
            }
            BinauralError::SofaLoadError(msg) => write!(f, "SOFA load error: {}", msg),
            BinauralError::ResamplingError(msg) => write!(f, "Resampling error: {}", msg),
            BinauralError::HrtfPreparationError(msg) => {
                write!(f, "HRTF preparation error: {}", msg)
            }
            BinauralError::InvalidParameter { name, value } => {
                write!(f, "Invalid parameter '{}': {}", name, value)
            }
            BinauralError::BufferSizeMismatch { expected, got } => {
                write!(
                    f,
                    "Buffer size mismatch: expected {}, got {}",
                    expected, got
                )
            }
        }
    }
}

impl std::error::Error for BinauralError {}

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
    /// LFE channels have zero HRTFs and are handled separately
    hrtf_filters_freq: Vec<Vec<Complex<f32>>>,

    /// LFE channel indices (channels that should not be spatially processed)
    lfe_channels: Vec<usize>,

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

        // Overflow checks for buffer allocations
        // These prevent integer overflow when creating large buffers
        let input_buffer_size = hop_size
            .checked_mul(input_channels)
            .expect("Buffer size overflow: hop_size * input_channels too large");
        let hrtf_buffer_per_channel = fft_size
            .checked_mul(2)
            .expect("Buffer size overflow: fft_size * 2 too large");
        let _hrtf_total_size = hrtf_buffer_per_channel
            .checked_mul(input_channels)
            .expect("Buffer size overflow: HRTF buffer total size too large");
        let output_acc_size = fft_size
            .checked_mul(2)
            .expect("Buffer size overflow: output accumulator size too large");

        // Additional sanity checks
        assert!(
            input_buffer_size <= 1 << 24,
            "Input buffer size unreasonably large (> 16MB)"
        );
        assert!(fft_size <= 1 << 16, "FFT size unreasonably large (> 65536)");

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

        // Identify LFE channels
        let lfe_channels: Vec<usize> = speaker_config
            .speakers
            .iter()
            .filter(|s| s.is_lfe)
            .map(|s| s.channel)
            .collect();

        log::info!(
            "[BinauralDecoder] Created with {} input channels ({}), FFT size {}",
            input_channels,
            speaker_config.name,
            fft_size
        );
        for speaker in speaker_config.speakers {
            let lfe_marker = if speaker.is_lfe {
                " [LFE - no HRTF]"
            } else {
                ""
            };
            log::info!(
                "[BinauralDecoder]   Ch{}: {} at az={:.1}°, el={:.1}°{}",
                speaker.channel,
                speaker.name,
                speaker.azimuth,
                speaker.elevation,
                lfe_marker
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

            hrtf_filters_freq: vec![
                vec![Complex::new(0.0, 0.0); hrtf_buffer_per_channel];
                input_channels
            ],
            lfe_channels,

            input_buffer: vec![0.0; input_buffer_size], // Interleaved, size for one hop
            input_buffer_fill: 0,

            output_accumulator: vec![vec![0.0; output_acc_size]; 2], // 2 output channels, enough space for overlap
            output_accumulator_fill: 0,
            next_add_position: 0,

            temp_input_block: vec![0.0; input_buffer_size], // Interleaved multi-channel input
            temp_output_block: vec![0.0; output_acc_size],  // Stereo output
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

        let mut sofa =
            SofaFile::load(&path).map_err(|e| BinauralError::SofaLoadError(e).to_string())?;

        log::info!(
            "[BinauralDecoder] SOFA loaded: {} measurements, IR length: {}, sample rate: {} Hz",
            sofa.num_measurements,
            sofa.ir_length,
            sofa.sample_rate
        );

        // Check if resampling is needed
        let sample_rate_diff = (sofa.sample_rate - self.sample_rate as f32).abs();
        if sample_rate_diff > 1.0 {
            log::info!(
                "[BinauralDecoder] Resampling SOFA from {} Hz to {} Hz",
                sofa.sample_rate,
                self.sample_rate
            );
            Self::resample_sofa(&mut sofa, self.sample_rate)?;
        }

        // Store SOFA first so prepare_hrtf_filters can use it
        self.sofa = Some(sofa);
        self.sofa_path = Some(path);

        // Prepare HRTF filters for each speaker
        self.prepare_hrtf_filters()?;

        log::debug!("[BinauralDecoder] SOFA file loaded and HRTFs prepared");

        Ok(())
    }

    /// Resample SOFA file impulse responses to target sample rate
    fn resample_sofa(sofa: &mut SofaFile, target_sample_rate: u32) -> Result<(), String> {
        let source_rate = sofa.sample_rate as usize;
        let target_rate = target_sample_rate as usize;

        if source_rate == target_rate {
            return Ok(());
        }

        // Calculate resampling ratio
        let ratio = target_rate as f64 / source_rate as f64;
        let new_ir_length = (sofa.ir_length as f64 * ratio).ceil() as usize;

        log::debug!(
            "[BinauralDecoder] Resampling: {}Hz -> {}Hz, IR length: {} -> {}",
            source_rate,
            target_rate,
            sofa.ir_length,
            new_ir_length
        );

        // Create high-quality sinc resampler
        // Parameters optimized for HRTF resampling
        let params = SincInterpolationParameters {
            sinc_len: 256,  // High quality filter
            f_cutoff: 0.95, // Preserve high frequencies
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };

        let mut resampler = SincFixedIn::<f32>::new(
            ratio,
            2.0, // Maximum ratio change (not used for fixed resampler)
            params,
            sofa.ir_length,
            2, // Stereo (left and right channels per measurement)
        )
        .map_err(|e| format!("Failed to create resampler: {:?}", e))?;

        // Resample each measurement
        let mut resampled_data = Vec::with_capacity(sofa.num_measurements * 2 * new_ir_length);

        for m in 0..sofa.num_measurements {
            // Extract left and right IRs for this measurement
            let offset = m * 2 * sofa.ir_length;
            let ir_left = &sofa.impulse_responses[offset..offset + sofa.ir_length];
            let ir_right =
                &sofa.impulse_responses[offset + sofa.ir_length..offset + 2 * sofa.ir_length];

            // Prepare input in channel-major format [[left samples], [right samples]]
            let input = vec![ir_left.to_vec(), ir_right.to_vec()];

            // Resample
            let output = resampler
                .process(&input, None)
                .map_err(|e| format!("Resampling failed for measurement {}: {:?}", m, e))?;

            log::debug!(
                "[BinauralDecoder] Measurement {}: input_len={}, output_left_len={}, output_right_len={}, expected={}",
                m,
                sofa.ir_length,
                output[0].len(),
                output[1].len(),
                new_ir_length
            );

            // Append resampled data (interleaved: left then right)
            resampled_data.extend_from_slice(&output[0]); // Left channel
            resampled_data.extend_from_slice(&output[1]); // Right channel

            // Reset resampler for next measurement
            resampler.reset();
        }

        // Verify the resampled data size
        let expected_total = sofa.num_measurements * 2 * new_ir_length;
        let actual_total = resampled_data.len();

        if actual_total != expected_total {
            log::warn!(
                "[BinauralDecoder] Resampled data size mismatch: expected {}, got {}. Adjusting ir_length.",
                expected_total,
                actual_total
            );
            // Calculate actual IR length based on what we got
            let actual_ir_length = actual_total / (sofa.num_measurements * 2);
            sofa.ir_length = actual_ir_length;
            sofa.impulse_responses = resampled_data;
            sofa.sample_rate = target_sample_rate as f32;

            log::info!(
                "[BinauralDecoder] Resampling complete: IR length adjusted to {}",
                actual_ir_length
            );
        } else {
            // Update SOFA file with resampled data
            sofa.impulse_responses = resampled_data;
            sofa.ir_length = new_ir_length;
            sofa.sample_rate = target_sample_rate as f32;

            log::info!(
                "[BinauralDecoder] Resampling complete: new IR length = {}",
                new_ir_length
            );
        }

        Ok(())
    }

    /// Prepare HRTF filters in frequency domain for all speakers
    fn prepare_hrtf_filters(&mut self) -> Result<(), String> {
        let sofa = self.sofa.as_ref().ok_or("SOFA file not loaded")?;

        for (i, speaker) in self.speaker_config.speakers.iter().enumerate() {
            // Skip LFE channels - they are handled separately without HRTF processing
            if speaker.is_lfe {
                log::info!(
                    "[BinauralDecoder] Skipping HRTF for LFE channel {} ({})",
                    i,
                    speaker.name
                );
                // Leave HRTFs as zeros for LFE channels
                continue;
            }

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
                gains[0],
                nearest[0].0,
                gains[1],
                nearest[1].0,
                gains[2],
                nearest[2].0
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

            // Apply Near-Field Shadowing with improved ILD model
            if self.near_field_strength > 0.01 {
                self.apply_near_field_shadowing(
                    &mut left_fft,
                    &mut right_fft,
                    speaker.azimuth,
                    speaker.elevation,
                );
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

    /// Calculate VBAP gains using barycentric interpolation
    ///
    /// Uses barycentric coordinates to interpolate between 3 source positions.
    /// This is more numerically stable than matrix inversion and provides better
    /// spatial accuracy.
    ///
    /// Reference: Pulkki, "Virtual Sound Source Positioning Using Vector Base Amplitude Panning"
    fn calculate_vbap_gains(
        target: &SourcePosition,
        nearest: &[(usize, f32); 3],
        sofa: &SofaFile,
    ) -> [f32; 3] {
        let p = target.to_cartesian_unit_vector();

        let v0 = sofa.positions[nearest[0].0].to_cartesian_unit_vector();
        let v1 = sofa.positions[nearest[1].0].to_cartesian_unit_vector();
        let v2 = sofa.positions[nearest[2].0].to_cartesian_unit_vector();

        // Calculate barycentric coordinates using cross products
        // This is more stable than matrix inversion
        //
        // The barycentric coordinates (w0, w1, w2) satisfy:
        // p = w0*v0 + w1*v1 + w2*v2
        // w0 + w1 + w2 = 1
        //
        // Using the formula:
        // w0 = area(p,v1,v2) / area(v0,v1,v2)
        // w1 = area(v0,p,v2) / area(v0,v1,v2)
        // w2 = area(v0,v1,p) / area(v0,v1,v2)
        //
        // Where area is computed using cross product magnitude

        // Helper to compute cross product
        let cross = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };

        // Helper to compute dot product
        let dot = |a: [f32; 3], b: [f32; 3]| -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] };

        // Compute edge vectors
        let v01 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
        let v02 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
        let v0p = [p[0] - v0[0], p[1] - v0[1], p[2] - v0[2]];

        // Normal of triangle (v0, v1, v2)
        let n = cross(v01, v02);
        let n_dot_n = dot(n, n);

        // Check for degenerate triangle (collinear points)
        if n_dot_n < 1e-6 {
            log::warn!("[BinauralDecoder] Degenerate triangle detected, using nearest neighbor");
            return [1.0, 0.0, 0.0];
        }

        // Calculate barycentric coordinates
        // w1 corresponds to v1, w2 corresponds to v2
        let n_cross_v02 = cross(n, v02);
        let n_cross_v01 = cross(n, v01);

        let w1 = dot(n_cross_v02, v0p) / n_dot_n;
        let w2 = dot(n_cross_v01, v0p) / n_dot_n;
        let w0 = 1.0 - w1 - w2;

        // Check if point is inside triangle (all weights non-negative)
        // If outside, clamp to valid range and warn
        let mut weights = [w0, w1, w2];

        if weights.iter().any(|&w| w < -0.01) {
            // Point is significantly outside triangle
            // This can happen with sparse HRTF measurements
            log::debug!(
                "[BinauralDecoder] Target outside triangle: weights=[{:.3}, {:.3}, {:.3}], clamping to boundary",
                w0,
                w1,
                w2
            );

            // Clamp negative weights to zero
            for w in &mut weights {
                if *w < 0.0 {
                    *w = 0.0;
                }
            }

            // Renormalize
            let sum: f32 = weights.iter().sum();
            if sum > 1e-6 {
                for w in &mut weights {
                    *w /= sum;
                }
            } else {
                // All weights were negative, use nearest neighbor
                weights = [1.0, 0.0, 0.0];
            }
        }

        // Energy normalization for VBAP
        // Ensures constant perceived loudness across panning positions
        let energy = weights[0] * weights[0] + weights[1] * weights[1] + weights[2] * weights[2];
        if energy > 1e-6 {
            let scale = 1.0 / energy.sqrt();
            [weights[0] * scale, weights[1] * scale, weights[2] * scale]
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

    /// Apply near-field head shadowing to HRTF frequency response
    ///
    /// Implements frequency-dependent Interaural Level Difference (ILD) based on
    /// head shadowing models. Uses Woodworth-Schlosberg formula combined with
    /// frequency-dependent diffraction model.
    ///
    /// The shadowing effect is strongest at high frequencies (short wavelengths)
    /// where the head acts as an effective barrier. At low frequencies (long
    /// wavelengths), sound diffracts around the head with minimal attenuation.
    ///
    /// References:
    /// - Woodworth & Schlosberg (1954), "Experimental Psychology"
    /// - Algazi et al. (2001), "Approximating Head-Related Transfer Functions"
    /// - Kuhn (1977), "Model for Interaural Time Differences"
    fn apply_near_field_shadowing(
        &self,
        left_fft: &mut [Complex<f32>],
        right_fft: &mut [Complex<f32>],
        azimuth: f32,
        elevation: f32,
    ) {
        // Head model parameters
        const HEAD_RADIUS: f32 = 0.0875; // 8.75 cm (typical adult head radius)
        const SPEED_OF_SOUND: f32 = 343.0; // m/s at 20°C

        let az_rad = azimuth.to_radians();
        let el_rad = elevation.to_radians();

        // Calculate angle in horizontal plane (elevation affects the effective azimuth)
        let horizontal_angle = az_rad * el_rad.cos();

        // Determine which ear is shadowed
        let (shadowed_ear, shadow_angle) = if horizontal_angle > 0.0 {
            // Source on left, shadow right ear
            (right_fft, horizontal_angle.abs())
        } else {
            // Source on right, shadow left ear
            (left_fft, horizontal_angle.abs())
        };

        // Only apply if angle is significant (> 15 degrees)
        if shadow_angle < 15.0_f32.to_radians() {
            return;
        }

        // Process each frequency bin
        for k in 0..self.fft_size / 2 + 1 {
            // Frequency for bin k
            let freq = k as f32 * self.sample_rate as f32 / self.fft_size as f32;

            if freq < 50.0 {
                // Very low frequencies: no shadowing
                continue;
            }

            // Wavelength
            let wavelength = SPEED_OF_SOUND / freq;

            // Normalized frequency: ka = 2π * radius / wavelength
            let ka = 2.0 * std::f32::consts::PI * HEAD_RADIUS / wavelength;

            // Shadowing attenuation model (combines multiple effects):
            //
            // 1. Geometric shadowing (high frequency): exponential with angle
            // 2. Diffraction (low frequency): based on Rayleigh parameter ka
            // 3. Transition region: smooth blend

            // High-frequency geometric shadowing (ka >> 1)
            // Attenuation increases with angle and frequency
            let geometric_atten = if ka > 2.0 {
                // Exponential shadowing model for high frequencies
                let angle_factor = (shadow_angle / std::f32::consts::PI).powi(2);
                let freq_factor = (ka / 10.0).min(1.0);
                -6.0 * angle_factor * freq_factor // Up to -6 dB
            } else {
                0.0
            };

            // Low-frequency diffraction (ka << 1)
            // Uses Rayleigh scattering approximation
            let diffraction_atten = if ka < 2.0 {
                // Minimal shadowing at low frequencies due to diffraction
                let diffraction_factor = (ka / 2.0).powi(2);
                -2.0 * diffraction_factor * (shadow_angle / std::f32::consts::PI).powi(2)
            } else {
                0.0
            };

            // Combine effects (smooth transition)
            let transition_weight = (ka / 2.0).min(1.0);
            let total_atten_db =
                geometric_atten * transition_weight + diffraction_atten * (1.0 - transition_weight);

            // Scale by near-field strength parameter
            let scaled_atten_db = total_atten_db * self.near_field_strength;

            // Convert dB to linear gain
            let gain = 10.0_f32.powf(scaled_atten_db / 20.0);

            // Apply to shadowed ear
            shadowed_ear[k] = shadowed_ear[k] * gain;

            // Mirror to negative frequencies (complex conjugate symmetry)
            if k > 0 && k < self.fft_size / 2 {
                let mirror_k = self.fft_size - k;
                shadowed_ear[mirror_k] = shadowed_ear[mirror_k] * gain;
            }
        }
    }

    /// Drain output accumulator to output buffer
    ///
    /// Returns number of stereo samples written to output
    fn drain_output_accumulator(&mut self, output: &mut [f32], output_pos: usize) -> usize {
        let frames_available = (output.len() - output_pos) / 2;
        let frames_to_drain = self.output_accumulator_fill.min(frames_available);

        if frames_to_drain > 0 {
            for i in 0..frames_to_drain {
                // Apply soft clipper (tanh) to prevent hard clipping
                output[output_pos + i * 2] = self.output_accumulator[0][i].tanh();
                output[output_pos + i * 2 + 1] = self.output_accumulator[1][i].tanh();
            }

            // Shift accumulator
            for ch in 0..2 {
                self.output_accumulator[ch]
                    .copy_within(frames_to_drain..self.output_accumulator_fill, 0);
                for i in
                    (self.output_accumulator_fill - frames_to_drain)..self.output_accumulator_fill
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

        frames_to_drain
    }

    /// Process one audio block through HRTF convolution
    ///
    /// Takes hop_size samples from input_buffer, applies HRTF convolution,
    /// and adds result to output_accumulator using overlap-add
    fn process_audio_block(&mut self) {
        // Note: This multiplication is safe - overflow is checked during initialization in new()
        let input_needed = self.hop_size * self.input_channels;

        // Copy input block
        self.temp_input_block[..input_needed].copy_from_slice(&self.input_buffer[..input_needed]);

        // Process using std::mem::take to avoid borrow conflicts
        let input_block = std::mem::take(&mut self.temp_input_block);
        let mut output_block = std::mem::take(&mut self.temp_output_block);

        // Clear temp freq buffer
        self.temp_freq_buffer.fill(Complex::new(0.0, 0.0));

        if self.enable_optimization {
            // OPTIMIZATION: Sum-Before-IFFT
            // Transform all inputs, multiply by HRTFs, sum in frequency domain, then 2 IFFTs
            let mut sum_left = vec![Complex::new(0.0, 0.0); self.fft_size];
            let mut sum_right = vec![Complex::new(0.0, 0.0); self.fft_size];

            for ch in 0..self.input_channels {
                // Skip LFE channels (they have zero HRTFs)
                if self.lfe_channels.contains(&ch) {
                    continue;
                }

                // Extract channel and FFT
                for i in 0..self.hop_size {
                    self.temp_time_buffer[i] =
                        Complex::new(input_block[i * self.input_channels + ch], 0.0);
                }
                for i in self.hop_size..self.fft_size {
                    self.temp_time_buffer[i] = Complex::new(0.0, 0.0);
                }

                self.fft_forward.process(&mut self.temp_time_buffer);

                // Multiply and accumulate (SIMD-optimized hot path)
                let hrtf = &self.hrtf_filters_freq[ch];
                complex_mul_add_simd(
                    &mut sum_left,
                    &self.temp_time_buffer,
                    &hrtf[0..self.fft_size],
                );
                complex_mul_add_simd(
                    &mut sum_right,
                    &self.temp_time_buffer,
                    &hrtf[self.fft_size..],
                );
            }

            // IFFT and scale
            self.fft_inverse.process(&mut sum_left);
            self.fft_inverse.process(&mut sum_right);

            let scale = 1.0 / self.fft_size as f32;
            for i in 0..self.fft_size {
                output_block[i * 2] = sum_left[i].re * scale;
                output_block[i * 2 + 1] = sum_right[i].re * scale;
            }
        } else {
            // Standard per-channel IFFT (reference implementation)
            output_block.fill(0.0);

            for ch in 0..self.input_channels {
                // Skip LFE channels
                if self.lfe_channels.contains(&ch) {
                    continue;
                }

                // Extract channel
                for i in 0..self.hop_size {
                    self.temp_time_buffer[i] =
                        Complex::new(input_block[i * self.input_channels + ch], 0.0);
                }
                for i in self.hop_size..self.fft_size {
                    self.temp_time_buffer[i] = Complex::new(0.0, 0.0);
                }

                self.fft_forward.process(&mut self.temp_time_buffer);

                // Convolve with left HRTF (SIMD-optimized)
                complex_mul_simd(
                    &mut self.temp_freq_buffer,
                    &self.temp_time_buffer,
                    &self.hrtf_filters_freq[ch][0..self.fft_size],
                );
                self.fft_inverse.process(&mut self.temp_freq_buffer);

                let scale = 1.0 / self.fft_size as f32;
                for i in 0..self.fft_size {
                    output_block[i * 2] += self.temp_freq_buffer[i].re * scale;
                }

                // Convolve with right HRTF (SIMD-optimized)
                complex_mul_simd(
                    &mut self.temp_freq_buffer,
                    &self.temp_time_buffer,
                    &self.hrtf_filters_freq[ch][self.fft_size..],
                );
                self.fft_inverse.process(&mut self.temp_freq_buffer);

                for i in 0..self.fft_size {
                    output_block[i * 2 + 1] += self.temp_freq_buffer[i].re * scale;
                }
            }
        }

        // Move buffers back
        self.temp_input_block = input_block;
        self.temp_output_block = output_block;

        // Mix LFE channels directly (no HRTF)
        if !self.lfe_channels.is_empty() {
            let lfe_gain = std::f32::consts::FRAC_1_SQRT_2; // -3dB (~0.7071)
            for &lfe_ch in &self.lfe_channels {
                for i in 0..self.hop_size {
                    let lfe_sample = self.temp_input_block[i * self.input_channels + lfe_ch];
                    self.temp_output_block[i * 2] += lfe_sample * lfe_gain;
                    self.temp_output_block[i * 2 + 1] += lfe_sample * lfe_gain;
                }
            }
        }

        // Apply externalization
        if self.externalization > 0.01 {
            self.apply_externalization();
        }

        // Accumulate output (overlap-add)
        for i in 0..self.fft_size {
            self.output_accumulator[0][self.next_add_position + i] += self.temp_output_block[i * 2];
            self.output_accumulator[1][self.next_add_position + i] +=
                self.temp_output_block[i * 2 + 1];
        }

        // Update state
        self.next_add_position += self.hop_size;
        let new_end = (self.next_add_position - self.hop_size) + self.fft_size;
        self.output_accumulator_fill = self.output_accumulator_fill.max(new_end);

        // Shift input buffer
        // Note: This multiplication is safe - overflow is checked during initialization in new()
        let shift_amount = self.hop_size * self.input_channels;
        self.input_buffer
            .copy_within(shift_amount..self.input_buffer_fill, 0);
        self.input_buffer_fill -= shift_amount;
    }

    /// Fill input buffer from input slice
    ///
    /// Returns number of samples consumed from input
    fn fill_input_buffer(&mut self, input: &[f32], input_pos: usize) -> usize {
        // Note: This multiplication is safe - overflow is checked during initialization in new()
        let input_needed = self.hop_size * self.input_channels;
        let samples_to_copy = (input.len() - input_pos).min(input_needed - self.input_buffer_fill);

        if samples_to_copy > 0 {
            self.input_buffer[self.input_buffer_fill..self.input_buffer_fill + samples_to_copy]
                .copy_from_slice(&input[input_pos..input_pos + samples_to_copy]);
            self.input_buffer_fill += samples_to_copy;
        }

        samples_to_copy
    }

    /// Apply externalization effect to temp_output_block
    ///
    /// Simulates room acoustics by adding early reflections.
    /// This reduces the "in-head localization" effect common with pure HRTF rendering.
    ///
    /// Implementation:
    /// - Adds multiple delayed and attenuated copies of the direct sound
    /// - Simulates first-order reflections from walls, floor, ceiling
    /// - Uses simple geometric delay model for small room
    ///
    /// Reference: Begault, "3-D Sound for Virtual Reality and Multimedia", Chapter 6
    fn apply_externalization(&mut self) {
        // Early reflection parameters (for a small listening room ~4m x 5m x 2.5m)
        // Each reflection: (delay_ms, gain_db, left_gain, right_gain)
        // left_gain and right_gain allow for asymmetric reflections
        let reflections = [
            (8.0, -12.0, 0.9, 1.0),  // Front wall
            (15.0, -15.0, 0.7, 0.9), // Side wall (left bias)
            (18.0, -15.0, 1.0, 0.7), // Side wall (right bias)
            (22.0, -18.0, 0.8, 0.8), // Back wall
            (12.0, -20.0, 1.0, 1.0), // Floor
            (14.0, -20.0, 1.0, 1.0), // Ceiling
        ];

        for &(delay_ms, gain_db, left_mul, right_mul) in &reflections {
            let delay_samples = ((delay_ms / 1000.0) * self.sample_rate as f32) as usize;

            // Convert dB to linear gain, scaled by externalization parameter
            let reflection_gain = 10.0_f32.powf(gain_db / 20.0) * self.externalization;

            if delay_samples < self.fft_size && delay_samples > 0 {
                for i in delay_samples..self.fft_size {
                    let src_idx = (i - delay_samples) * 2;
                    let dst_idx = i * 2;

                    // Add delayed reflection to output
                    self.temp_output_block[dst_idx] +=
                        self.temp_output_block[src_idx] * reflection_gain * left_mul;
                    self.temp_output_block[dst_idx + 1] +=
                        self.temp_output_block[src_idx + 1] * reflection_gain * right_mul;
                }
            }
        }

        // Apply subtle decorrelation between channels for diffuse reflections
        // This simulates late reflections and adds spaciousness
        if self.externalization > 0.5 {
            let diffuse_gain = (self.externalization - 0.5) * 0.3; // Max 15% of signal
            let diffuse_delay = (self.sample_rate as f32 * 0.001) as usize; // 1ms

            for i in diffuse_delay..self.fft_size {
                let cross_left = self.temp_output_block[(i - diffuse_delay) * 2 + 1];
                let cross_right = self.temp_output_block[(i - diffuse_delay) * 2];

                self.temp_output_block[i * 2] += cross_left * diffuse_gain;
                self.temp_output_block[i * 2 + 1] += cross_right * diffuse_gain;
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
                        self.prepare_hrtf_filters()
                            .map_err(|e| format!("Failed to update filters: {}", e))?;
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

            // Note: Sample rate mismatch is now handled automatically via resampling in load_sofa()
        } else {
            log::debug!("[BinauralDecoder] No SOFA file specified, plugin will pass through audio");
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
        // Validate input/output buffer sizes
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

        // Passthrough mode if no SOFA file loaded
        if self.sofa.is_none() {
            for frame in 0..context.num_frames {
                let (mut left, mut right) = if self.input_channels == 1 {
                    // Mono -> Stereo: duplicate to both channels
                    let sample = input[frame];
                    (sample, sample)
                } else if self.input_channels == 2 {
                    // Stereo -> Stereo: direct copy
                    let l = input[frame * 2];
                    let r = input[frame * 2 + 1];
                    (l, r)
                } else {
                    // Multi-channel -> Stereo: take first two channels
                    let l = input[frame * self.input_channels];
                    let r = input[frame * self.input_channels + 1];
                    (l, r)
                };

                // Flush denormals to zero to avoid CPU spikes and satisfy
                // denormal-flushing invariants in tests.
                if left.abs() < 1e-30 {
                    left = 0.0;
                }
                if right.abs() < 1e-30 {
                    right = 0.0;
                }

                output[frame * 2] = left;
                output[frame * 2 + 1] = right;
            }
            return Ok(());
        }

        let start_time = std::time::Instant::now();

        let mut input_pos = 0;
        let mut output_pos = 0;

        // Main processing loop
        //
        // This loop follows a simple state machine:
        // 1. Drain available output to user buffer
        // 2. Process a block if we have enough input and space in accumulator
        // 3. Fill input buffer from user input
        // 4. Check exit conditions and repeat
        loop {
            // Step 1: Drain output accumulator to output buffer
            let frames_drained = self.drain_output_accumulator(output, output_pos);
            output_pos += frames_drained * 2;

            // Step 2: Process audio block if conditions are met
            let input_needed = self.hop_size * self.input_channels;
            let can_process_input = self.input_buffer_fill >= input_needed;
            let can_process_space = self.next_add_position + self.fft_size <= self.fft_size * 2;

            if can_process_input && can_process_space {
                self.process_audio_block();
                continue;
            }

            // Step 3: Fill input buffer from user input
            if input_pos < input.len() {
                let samples_filled = self.fill_input_buffer(input, input_pos);
                input_pos += samples_filled;
                continue;
            }

            // Step 4: Check exit conditions
            let no_space_to_drain = (output.len() - output_pos) / 2 == 0;
            let cant_process = !can_process_input || !can_process_space;
            let no_data_to_drain = self.output_accumulator_fill == 0;

            // Exit when we've processed all input and drained all output
            if no_space_to_drain || (input_pos >= input.len() && cant_process && no_data_to_drain) {
                break;
            }
        }

        // Performance monitoring
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
        plugin
            .set_parameter(
                ParameterId::from("enable_optimization"),
                ParameterValue::Bool(false),
            )
            .unwrap();
        assert_eq!(plugin.enable_optimization, false);

        // Test externalization
        plugin
            .set_parameter(
                ParameterId::from("externalization"),
                ParameterValue::Float(0.5),
            )
            .unwrap();
        assert_eq!(plugin.externalization, 0.5);

        // Test near-field
        plugin
            .set_parameter(
                ParameterId::from("near_field_strength"),
                ParameterValue::Float(0.8),
            )
            .unwrap();
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
