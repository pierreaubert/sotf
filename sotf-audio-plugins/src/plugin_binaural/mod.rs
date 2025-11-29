// ============================================================================
// Binaural Decoder Plugin - Multi-channel to Binaural Stereo
// ============================================================================

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::{complex_mul_add_simd, complex_mul_simd};
use super::speaker_config::{SpeakerConfig, get_speaker_config_by_channels};

use crate::sofa::SofaFile;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use rustfft::num_complex::Complex;
use std::path::PathBuf;
use std::sync::Arc;

pub mod error;
pub mod filter;
pub mod hrtf;
pub mod params;
pub mod room;

pub use self::error::BinauralError;
pub use self::params::{
    BinauralDecoderParams, default_enable_optimization as binaural_default_enable_optimization,
};
pub use self::room::{Reflection, RoomModel};

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

    /// Real FFT planners (more efficient for real-valued audio signals)
    /// R2C: N real samples -> N/2+1 complex frequency bins
    fft_r2c: Arc<dyn RealToComplex<f32>>,
    /// C2R: N/2+1 complex frequency bins -> N real samples
    fft_c2r: Arc<dyn ComplexToReal<f32>>,
    /// Number of complex frequency bins (fft_size/2 + 1)
    freq_size: usize,

    /// HRTF filters in frequency domain [channels × 2 × freq_size]
    /// For each input channel: [left_ear_fft, right_ear_fft]
    /// Uses half-spectrum representation (N/2+1 bins) for real signals
    /// LFE channels have zero HRTFs and are handled separately
    hrtf_filters_freq: Vec<Vec<Complex<f32>>>,

    /// Diffuse-field equalization filter (inverse of diffuse-field response)
    /// Applied to both ears to compensate for HRTF coloration
    /// [left_eq, right_eq] in frequency domain
    diffuse_field_eq_filter: Option<[Vec<Complex<f32>>; 2]>,

    /// LFE low-pass filter in frequency domain (band-limits LFE to subwoofer range)
    /// Uses half-spectrum representation (N/2+1 bins)
    lfe_lowpass_filter: Vec<Complex<f32>>,
    /// LFE gain including distance attenuation and level adjustment
    lfe_gain: f32,

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
    /// Ring buffer read position
    output_read_position: usize,

    /// Temporary buffers (reused to avoid allocations)
    temp_input_block: Vec<f32>,
    temp_output_block: Vec<f32>,
    /// Frequency domain buffer (N/2+1 complex bins for real FFT)
    temp_freq_buffer: Vec<Complex<f32>>,
    /// Time domain buffer for real FFT input (N real samples)
    temp_time_buffer: Vec<f32>,
    /// Scratch buffer for real FFT operations
    temp_fft_scratch: Vec<Complex<f32>>,

    // Parameters
    param_enable_optimization: ParameterId,
    enable_optimization: bool,
    param_externalization: ParameterId,
    externalization: f32,
    param_near_field_strength: ParameterId,
    near_field_strength: f32,
    param_diffuse_field_eq: ParameterId,
    diffuse_field_eq: bool,

    // LFE parameters
    lfe_crossover: f32,
    lfe_distance: f32,
    lfe_level: f32,

    /// Room model for externalization
    room_model: RoomModel,
    /// Cached reflections for current room configuration
    cached_reflections: Vec<Reflection>,
}

impl BinauralDecoderPlugin {
    /// Create a new binaural decoder plugin
    pub fn new(
        input_channels: usize,
        fft_size: usize,
        sofa_path: Option<PathBuf>,
        enable_optimization: bool,
        externalization: f32,
        near_field_strength: f32,
        diffuse_field_eq: bool,
        lfe_crossover: f32,
        lfe_distance: f32,
        lfe_level: f32,
        room_model: RoomModel,
    ) -> Self {
        assert!(fft_size.is_power_of_two(), "FFT size must be power of 2");
        assert!(input_channels > 0, "Must have at least 1 input channel");

        let hop_size = fft_size / 2;

        // Overflow checks for buffer allocations
        let input_buffer_size = hop_size
            .checked_mul(input_channels)
            .expect("Buffer size overflow: hop_size * input_channels too large");
        // For real FFT: freq_size = fft_size/2 + 1, store 2 ears × freq_size bins
        let freq_size_check = fft_size / 2 + 1;
        let _hrtf_buffer_per_channel = freq_size_check
            .checked_mul(2)
            .expect("Buffer size overflow: freq_size * 2 too large");
        let output_acc_size = fft_size
            .checked_mul(2)
            .expect("Buffer size overflow: output accumulator size too large");

        assert!(
            input_buffer_size <= 1 << 24,
            "Input buffer size unreasonably large (> 16MB)"
        );
        assert!(fft_size <= 1 << 16, "FFT size unreasonably large (> 65536)");

        // Use real FFT for efficiency (audio signals are real-valued)
        // R2C: N real -> N/2+1 complex, C2R: N/2+1 complex -> N real
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_r2c = planner.plan_fft_forward(fft_size);
        let fft_c2r = planner.plan_fft_inverse(fft_size);
        let freq_size = fft_size / 2 + 1;

        let speaker_config = get_speaker_config_by_channels(input_channels)
            .unwrap_or_else(|| {
                log::warn!(
                    "[BinauralDecoder] No standard configuration for {} channels, using default circular layout",
                    input_channels
                );
                get_speaker_config_by_channels(2).unwrap()
            });

        let lfe_channels: Vec<usize> = speaker_config
            .speakers
            .iter()
            .filter(|s| s.is_lfe)
            .map(|s| s.channel)
            .collect();

        log::info!(
            "[BinauralDecoder] Created with {} input channels ({}), FFT size {}, LFE channels: {:?}",
            input_channels,
            speaker_config.name,
            fft_size,
            lfe_channels
        );

        // Get scratch buffer size from FFT planner
        let scratch_len = fft_r2c.get_scratch_len().max(fft_c2r.get_scratch_len());

        Self {
            input_channels,
            fft_size,
            hop_size,
            sample_rate: 48000, // Will be set in initialize()

            sofa: None,
            sofa_path,
            speaker_config,

            fft_r2c,
            fft_c2r,
            freq_size,

            // HRTF storage: 2 ears × freq_size bins per channel
            hrtf_filters_freq: vec![vec![Complex::new(0.0, 0.0); freq_size * 2]; input_channels],
            diffuse_field_eq_filter: None,
            lfe_lowpass_filter: vec![Complex::new(1.0, 0.0); freq_size],
            lfe_gain: 1.0,
            lfe_channels,

            input_buffer: vec![0.0; input_buffer_size],
            input_buffer_fill: 0,

            output_accumulator: vec![vec![0.0; output_acc_size]; 2],
            output_accumulator_fill: 0,
            next_add_position: 0,
            output_read_position: 0,

            temp_input_block: vec![0.0; input_buffer_size],
            temp_output_block: vec![0.0; output_acc_size],
            temp_freq_buffer: vec![Complex::new(0.0, 0.0); freq_size],
            temp_time_buffer: vec![0.0; fft_size],
            temp_fft_scratch: vec![Complex::new(0.0, 0.0); scratch_len],

            param_enable_optimization: ParameterId::from("enable_optimization"),
            enable_optimization,
            param_externalization: ParameterId::from("externalization"),
            externalization,
            param_near_field_strength: ParameterId::from("near_field_strength"),
            near_field_strength,
            param_diffuse_field_eq: ParameterId::from("diffuse_field_eq"),
            diffuse_field_eq,

            lfe_crossover,
            lfe_distance,
            lfe_level,

            room_model,
            cached_reflections: Vec::new(),
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
            params.diffuse_field_eq,
            params.lfe_crossover,
            params.lfe_distance,
            params.lfe_level,
            params.room_model,
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

        let ratio = target_rate as f64 / source_rate as f64;
        let new_ir_length = (sofa.ir_length as f64 * ratio).ceil() as usize;

        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };

        let mut resampler = SincFixedIn::<f32>::new(ratio, 2.0, params, sofa.ir_length, 2)
            .map_err(|e| format!("Failed to create resampler: {:?}", e))?;

        let mut resampled_data = Vec::with_capacity(sofa.num_measurements * 2 * new_ir_length);

        for m in 0..sofa.num_measurements {
            let offset = m * 2 * sofa.ir_length;
            let ir_left = &sofa.impulse_responses[offset..offset + sofa.ir_length];
            let ir_right =
                &sofa.impulse_responses[offset + sofa.ir_length..offset + 2 * sofa.ir_length];

            let input = vec![ir_left.to_vec(), ir_right.to_vec()];

            let output = resampler
                .process(&input, None)
                .map_err(|e| format!("Resampling failed for measurement {}: {:?}", m, e))?;

            resampled_data.extend_from_slice(&output[0]);
            resampled_data.extend_from_slice(&output[1]);
            resampler.reset();
        }

        let expected_total = sofa.num_measurements * 2 * new_ir_length;
        let actual_total = resampled_data.len();

        if actual_total != expected_total {
            let actual_ir_length = actual_total / (sofa.num_measurements * 2);
            sofa.ir_length = actual_ir_length;
            sofa.impulse_responses = resampled_data;
            sofa.sample_rate = target_sample_rate as f32;
        } else {
            sofa.impulse_responses = resampled_data;
            sofa.ir_length = new_ir_length;
            sofa.sample_rate = target_sample_rate as f32;
        }

        Ok(())
    }

    /// Prepare HRTF filters in frequency domain for all speakers
    fn prepare_hrtf_filters(&mut self) -> Result<(), String> {
        let sofa = self.sofa.as_ref().ok_or("SOFA file not loaded")?;

        for (i, speaker) in self.speaker_config.speakers.iter().enumerate() {
            if speaker.is_lfe {
                continue;
            }

            let target_pos = room::speaker_to_source_position(speaker);

            let nearest = sofa.find_three_nearest(&target_pos);
            let gains = hrtf::calculate_vbap_gains(&target_pos, &nearest, sofa);

            // Returns freq_size (N/2+1) complex bins per ear
            let (left_fft, right_fft) = hrtf::interpolate_hrtf_frequency_domain(
                &nearest,
                &gains,
                sofa,
                self.fft_size,
                self.sample_rate,
                &self.fft_r2c,
                self.near_field_strength,
                speaker.azimuth,
                speaker.elevation,
            );

            // Store left and right HRTFs contiguously [left_freq_size | right_freq_size]
            let combined: Vec<Complex<f32>> =
                left_fft.into_iter().chain(right_fft.into_iter()).collect();

            self.hrtf_filters_freq[i] = combined;
        }

        // Normalize HRTFs (now uses freq_size bins)
        hrtf::normalize_hrtf_gains(
            &mut self.hrtf_filters_freq,
            &self.lfe_channels,
            self.freq_size,
            self.input_channels,
        );

        // Compute and apply diffuse-field equalization if enabled
        if self.diffuse_field_eq {
            self.diffuse_field_eq_filter = Some(filter::compute_diffuse_field_eq(
                sofa,
                self.fft_size,
                self.sample_rate,
                &self.fft_r2c,
            )?);
        }

        Ok(())
    }

    fn drain_output_accumulator(&mut self, output: &mut [f32], output_pos: usize) -> usize {
        let frames_available = (output.len() - output_pos) / 2;
        let frames_to_drain = self.output_accumulator_fill.min(frames_available);

        if frames_to_drain > 0 {
            let buffer_size = self.output_accumulator[0].len();

            for i in 0..frames_to_drain {
                let read_idx = (self.output_read_position + i) % buffer_size;
                output[output_pos + i * 2] = self.output_accumulator[0][read_idx];
                output[output_pos + i * 2 + 1] = self.output_accumulator[1][read_idx];

                self.output_accumulator[0][read_idx] = 0.0;
                self.output_accumulator[1][read_idx] = 0.0;
            }

            self.output_read_position = (self.output_read_position + frames_to_drain) % buffer_size;
            self.output_accumulator_fill -= frames_to_drain;

            if self.output_accumulator_fill == 0 {
                self.output_read_position = 0;
                self.next_add_position = 0;
            }
        }

        frames_to_drain
    }

    fn process_audio_block(&mut self) {
        let input_needed = self.hop_size * self.input_channels;
        self.temp_input_block[..input_needed].copy_from_slice(&self.input_buffer[..input_needed]);

        let input_block = std::mem::take(&mut self.temp_input_block);
        let mut output_block = std::mem::take(&mut self.temp_output_block);
        let mut time_buffer = std::mem::take(&mut self.temp_time_buffer);
        let mut freq_buffer = std::mem::take(&mut self.temp_freq_buffer);
        let mut scratch = std::mem::take(&mut self.temp_fft_scratch);

        freq_buffer.fill(Complex::new(0.0, 0.0));

        if self.enable_optimization {
            // Sum-Before-IFFT optimization: accumulate all channels in frequency domain
            // then do a single IFFT per ear
            let mut sum_left = vec![Complex::new(0.0, 0.0); self.freq_size];
            let mut sum_right = vec![Complex::new(0.0, 0.0); self.freq_size];

            for ch in 0..self.input_channels {
                if self.lfe_channels.contains(&ch) {
                    continue;
                }

                // Extract channel data into time buffer (zero-padded)
                for i in 0..self.hop_size {
                    time_buffer[i] = input_block[i * self.input_channels + ch];
                }
                for i in self.hop_size..self.fft_size {
                    time_buffer[i] = 0.0;
                }

                // Real-to-Complex FFT: N real -> N/2+1 complex
                self.fft_r2c
                    .process_with_scratch(&mut time_buffer, &mut freq_buffer, &mut scratch)
                    .expect("FFT forward failed");

                // Accumulate weighted by HRTF (freq_size bins per ear)
                let hrtf = &self.hrtf_filters_freq[ch];
                complex_mul_add_simd(&mut sum_left, &freq_buffer, &hrtf[0..self.freq_size]);
                complex_mul_add_simd(&mut sum_right, &freq_buffer, &hrtf[self.freq_size..]);
            }

            // Apply diffuse-field EQ if enabled
            if let Some(ref df_eq) = self.diffuse_field_eq_filter {
                for k in 0..self.freq_size {
                    sum_left[k] *= df_eq[0][k];
                    sum_right[k] *= df_eq[1][k];
                }
            }

            // Enforce real FFT constraints: DC and Nyquist bins must be real
            // (imaginary part must be zero for valid inverse transform)
            sum_left[0].im = 0.0;
            sum_right[0].im = 0.0;
            sum_left[self.freq_size - 1].im = 0.0;
            sum_right[self.freq_size - 1].im = 0.0;

            // Inverse FFT for left ear: N/2+1 complex -> N real
            let mut left_output = vec![0.0f32; self.fft_size];
            self.fft_c2r
                .process_with_scratch(&mut sum_left, &mut left_output, &mut scratch)
                .expect("FFT inverse failed");

            // Inverse FFT for right ear
            let mut right_output = vec![0.0f32; self.fft_size];
            self.fft_c2r
                .process_with_scratch(&mut sum_right, &mut right_output, &mut scratch)
                .expect("FFT inverse failed");

            // Real FFT normalization: output is scaled by fft_size
            let scale = 1.0 / self.fft_size as f32;
            for i in 0..self.fft_size {
                output_block[i * 2] = left_output[i] * scale;
                output_block[i * 2 + 1] = right_output[i] * scale;
            }
        } else {
            // Non-optimized path: process each channel separately
            output_block.fill(0.0);

            let mut channel_output = vec![0.0f32; self.fft_size];

            for ch in 0..self.input_channels {
                if self.lfe_channels.contains(&ch) {
                    continue;
                }

                // Extract channel data
                for i in 0..self.hop_size {
                    time_buffer[i] = input_block[i * self.input_channels + ch];
                }
                for i in self.hop_size..self.fft_size {
                    time_buffer[i] = 0.0;
                }

                // Forward FFT
                self.fft_r2c
                    .process_with_scratch(&mut time_buffer, &mut freq_buffer, &mut scratch)
                    .expect("FFT forward failed");

                // Process left ear
                let mut left_freq = vec![Complex::new(0.0, 0.0); self.freq_size];
                complex_mul_simd(
                    &mut left_freq,
                    &freq_buffer,
                    &self.hrtf_filters_freq[ch][0..self.freq_size],
                );

                if let Some(ref df_eq) = self.diffuse_field_eq_filter {
                    for k in 0..self.freq_size {
                        left_freq[k] *= df_eq[0][k];
                    }
                }

                // Enforce real FFT constraints
                left_freq[0].im = 0.0;
                left_freq[self.freq_size - 1].im = 0.0;

                self.fft_c2r
                    .process_with_scratch(&mut left_freq, &mut channel_output, &mut scratch)
                    .expect("FFT inverse failed");

                let scale = 1.0 / self.fft_size as f32;
                for i in 0..self.fft_size {
                    output_block[i * 2] += channel_output[i] * scale;
                }

                // Process right ear
                let mut right_freq = vec![Complex::new(0.0, 0.0); self.freq_size];
                complex_mul_simd(
                    &mut right_freq,
                    &freq_buffer,
                    &self.hrtf_filters_freq[ch][self.freq_size..],
                );

                if let Some(ref df_eq) = self.diffuse_field_eq_filter {
                    for k in 0..self.freq_size {
                        right_freq[k] *= df_eq[1][k];
                    }
                }

                // Enforce real FFT constraints
                right_freq[0].im = 0.0;
                right_freq[self.freq_size - 1].im = 0.0;

                self.fft_c2r
                    .process_with_scratch(&mut right_freq, &mut channel_output, &mut scratch)
                    .expect("FFT inverse failed");

                for i in 0..self.fft_size {
                    output_block[i * 2 + 1] += channel_output[i] * scale;
                }
            }
        }

        self.temp_input_block = input_block;
        self.temp_output_block = output_block;
        self.temp_time_buffer = time_buffer;
        self.temp_freq_buffer = freq_buffer;
        self.temp_fft_scratch = scratch;

        // Process LFE channels (mixed to both ears with lowpass filter)
        if !self.lfe_channels.is_empty() {
            let mut lfe_time = vec![0.0f32; self.fft_size];
            let mut lfe_freq = vec![Complex::new(0.0, 0.0); self.freq_size];
            let mut lfe_output = vec![0.0f32; self.fft_size];

            for &lfe_ch in &self.lfe_channels {
                // Extract LFE channel data
                for i in 0..self.hop_size {
                    lfe_time[i] = self.temp_input_block[i * self.input_channels + lfe_ch];
                }
                for i in self.hop_size..self.fft_size {
                    lfe_time[i] = 0.0;
                }

                // Forward FFT
                self.fft_r2c
                    .process_with_scratch(&mut lfe_time, &mut lfe_freq, &mut self.temp_fft_scratch)
                    .expect("LFE FFT forward failed");

                // Apply lowpass filter
                for k in 0..self.freq_size {
                    lfe_freq[k] *= self.lfe_lowpass_filter[k];
                }

                // Enforce real FFT constraints
                lfe_freq[0].im = 0.0;
                lfe_freq[self.freq_size - 1].im = 0.0;

                // Inverse FFT
                self.fft_c2r
                    .process_with_scratch(
                        &mut lfe_freq,
                        &mut lfe_output,
                        &mut self.temp_fft_scratch,
                    )
                    .expect("LFE FFT inverse failed");

                // Mix into both channels
                let scale = self.lfe_gain / self.fft_size as f32;
                for i in 0..self.hop_size {
                    let lfe_sample = lfe_output[i] * scale;
                    self.temp_output_block[i * 2] += lfe_sample;
                    self.temp_output_block[i * 2 + 1] += lfe_sample;
                }
            }
        }

        if self.externalization > 0.01 {
            self.apply_externalization();
        }

        let buffer_size = self.output_accumulator[0].len();
        for i in 0..self.fft_size {
            let write_idx = (self.next_add_position + i) % buffer_size;
            self.output_accumulator[0][write_idx] += self.temp_output_block[i * 2];
            self.output_accumulator[1][write_idx] += self.temp_output_block[i * 2 + 1];
        }

        self.next_add_position = (self.next_add_position + self.hop_size) % buffer_size;
        self.output_accumulator_fill =
            (self.output_accumulator_fill + self.hop_size).min(buffer_size);

        let shift_amount = self.hop_size * self.input_channels;
        self.input_buffer
            .copy_within(shift_amount..self.input_buffer_fill, 0);
        self.input_buffer_fill -= shift_amount;
    }

    fn fill_input_buffer(&mut self, input: &[f32], input_pos: usize) -> usize {
        let input_needed = self.hop_size * self.input_channels;
        let samples_to_copy = (input.len() - input_pos).min(input_needed - self.input_buffer_fill);

        if samples_to_copy > 0 {
            self.input_buffer[self.input_buffer_fill..self.input_buffer_fill + samples_to_copy]
                .copy_from_slice(&input[input_pos..input_pos + samples_to_copy]);
            self.input_buffer_fill += samples_to_copy;
        }

        samples_to_copy
    }

    fn calculate_reflections(&mut self) {
        self.cached_reflections.clear();

        if self.room_model.max_order == 0 {
            return;
        }

        let [room_width, room_depth, room_height] = self.room_model.dimensions;
        let [listener_x, listener_y, listener_z] = self.room_model.listener_position;

        let source_x = listener_x;
        let source_y = listener_y + 1.0;
        let source_z = listener_z;

        let walls = [
            (0, 0.0, 2),
            (0, room_width, 3),
            (1, 0.0, 0),
            (1, room_depth, 1),
            (2, 0.0, 4),
            (2, room_height, 5),
        ];

        for &(axis, wall_pos, abs_idx) in &walls {
            let mut image_source = [source_x, source_y, source_z];
            image_source[axis] = 2.0 * wall_pos - image_source[axis];

            let dx = image_source[0] - listener_x;
            let dy = image_source[1] - listener_y;
            let dz = image_source[2] - listener_z;
            let distance = (dx * dx + dy * dy + dz * dz).sqrt();

            let delay_seconds = distance / self.room_model.speed_of_sound;
            let delay_samples = (delay_seconds * self.sample_rate as f32) as usize;

            if delay_samples >= self.fft_size || delay_samples == 0 {
                continue;
            }

            let distance_attenuation = 1.0 / distance.max(0.1);
            let wall_reflection = 1.0 - self.room_model.absorption[abs_idx];
            let gain = distance_attenuation * wall_reflection;

            let azimuth = dy.atan2(dx);
            let left_gain =
                ((azimuth + std::f32::consts::FRAC_PI_2) / std::f32::consts::PI).clamp(0.0, 1.0);
            let right_gain = 1.0 - left_gain;

            self.cached_reflections.push(Reflection {
                delay_samples,
                gain,
                left_gain,
                right_gain,
            });
        }

        self.cached_reflections.sort_by_key(|r| r.delay_samples);
    }

    fn apply_externalization(&mut self) {
        if self.cached_reflections.is_empty() {
            self.calculate_reflections();
        }

        for reflection in &self.cached_reflections {
            let delay_samples = reflection.delay_samples;
            let reflection_gain = reflection.gain * self.externalization;

            if delay_samples < self.fft_size && delay_samples > 0 {
                for i in delay_samples..self.fft_size {
                    let src_idx = (i - delay_samples) * 2;
                    let dst_idx = i * 2;

                    self.temp_output_block[dst_idx] +=
                        self.temp_output_block[src_idx] * reflection_gain * reflection.left_gain;
                    self.temp_output_block[dst_idx + 1] += self.temp_output_block[src_idx + 1]
                        * reflection_gain
                        * reflection.right_gain;
                }
            }
        }

        if self.externalization > 0.5 {
            let diffuse_gain = (self.externalization - 0.5) * 0.3;
            let diffuse_delay = (self.sample_rate as f32 * 0.001) as usize;

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
        2
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_bool("enable_optimization", "Optimization", true)
                .with_description("Enable Sum-Before-IFFT optimization"),
            Parameter::new_float("externalization", "Externalization", 0.0, 0.0, 1.0)
                .with_description("Room simulation / externalization factor"),
            Parameter::new_float("near_field_strength", "Near-Field", 0.0, 0.0, 1.0)
                .with_description("Near-field shadowing strength"),
            Parameter::new_bool("diffuse_field_eq", "Diffuse-Field EQ", true)
                .with_description("Compensate for HRTF coloration (improves timbre)"),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_enable_optimization {
            if let Some(v) = value.as_bool() {
                self.enable_optimization = v;
                return Ok(());
            }
        } else if id == self.param_externalization {
            if let Some(v) = value.as_float()
                && (0.0..=1.0).contains(&v)
            {
                self.externalization = v;
                return Ok(());
            }
        } else if id == self.param_near_field_strength {
            if let Some(v) = value.as_float()
                && (0.0..=1.0).contains(&v)
            {
                self.near_field_strength = v;
                if self.sofa.is_some() {
                    self.prepare_hrtf_filters()
                        .map_err(|e| format!("Failed to update filters: {}", e))?;
                }
                return Ok(());
            }
        } else if id == self.param_diffuse_field_eq
            && let Some(v) = value.as_bool()
        {
            self.diffuse_field_eq = v;
            if v && self.sofa.is_some() {
                self.diffuse_field_eq_filter = Some(
                    filter::compute_diffuse_field_eq(
                        self.sofa.as_ref().unwrap(),
                        self.fft_size,
                        self.sample_rate,
                        &self.fft_r2c,
                    )
                    .map_err(|e| format!("Failed to compute diffuse-field EQ: {}", e))?,
                );
            } else if !v {
                self.diffuse_field_eq_filter = None;
            }
            return Ok(());
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
        } else if id == &self.param_diffuse_field_eq {
            Some(ParameterValue::Bool(self.diffuse_field_eq))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;

        let (filter, gain) = filter::compute_lfe_filter(
            self.fft_size,
            self.sample_rate,
            self.lfe_crossover,
            self.lfe_distance,
            self.lfe_level,
        );
        self.lfe_lowpass_filter = filter;
        self.lfe_gain = gain;

        if let Some(path) = self.sofa_path.clone() {
            self.load_sofa(path)
                .map_err(|e| format!("Failed to load SOFA file: {}", e))?;
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
        let input_samples = context.num_frames * self.input_channels;
        let output_samples = context.num_frames * 2;

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

        if self.sofa.is_none() {
            for frame in 0..context.num_frames {
                let (mut left, mut right) = if self.input_channels == 1 {
                    let sample = input[frame];
                    (sample, sample)
                } else if self.input_channels == 2 {
                    let l = input[frame * 2];
                    let r = input[frame * 2 + 1];
                    (l, r)
                } else {
                    let l = input[frame * self.input_channels];
                    let r = input[frame * self.input_channels + 1];
                    (l, r)
                };

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

        loop {
            let frames_drained = self.drain_output_accumulator(output, output_pos);
            output_pos += frames_drained * 2;

            let input_needed = self.hop_size * self.input_channels;
            let can_process_input = self.input_buffer_fill >= input_needed;
            let can_process_space = self.next_add_position + self.fft_size <= self.fft_size * 2;

            if can_process_input && can_process_space {
                self.process_audio_block();
                continue;
            }

            if input_pos < input.len() {
                let samples_filled = self.fill_input_buffer(input, input_pos);
                input_pos += samples_filled;
                continue;
            }

            let no_space_to_drain = (output.len() - output_pos) / 2 == 0;
            let cant_process = !can_process_input || !can_process_space;
            let no_data_to_drain = self.output_accumulator_fill == 0;

            if no_space_to_drain || (input_pos >= input.len() && cant_process && no_data_to_drain) {
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
        let plugin = BinauralDecoderPlugin::new(
            5,
            4096,
            None,
            true,
            0.0,
            0.0,
            false,
            120.0,
            2.0,
            0.0,
            RoomModel::default(),
        );
        assert_eq!(plugin.input_channels(), 5);
        assert_eq!(plugin.output_channels(), 2);
        assert_eq!(plugin.fft_size, 4096);
        assert_eq!(plugin.hop_size, 2048);
        assert_eq!(plugin.enable_optimization, true);
        assert_eq!(plugin.externalization, 0.0);
        assert_eq!(plugin.near_field_strength, 0.0);
    }
}
