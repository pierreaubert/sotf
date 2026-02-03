// ============================================================================
// Binaural Decoder Plugin - Multi-channel to Binaural Stereo
// ============================================================================

use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::{complex_mul_add_simd, complex_mul_simd};
use super::smoothing::Smoother;
use super::speaker_config::{SpeakerConfig, get_speaker_config_by_channels};

use crate::sofa::SofaFile;
use parking_lot::RwLock;
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

/// Holds the heavy state that needs to be swapped atomically
struct BinauralState {
    /// HRTF filters in frequency domain [channels × 2 × freq_size]
    hrtf_filters_freq: Vec<Vec<Complex<f32>>>,

    /// Diffuse-field equalization filter
    diffuse_field_eq_filter: Option<[Vec<Complex<f32>>; 2]>,

    /// Loaded HRTF data (needed for updates)
    hrtf_data: Option<SofaFile>,
}

/// Binaural decoder using HRTFs from a file
pub struct BinauralDecoderPlugin {
    /// Number of input channels
    input_channels: usize,
    /// FFT size for convolution
    fft_size: usize,
    /// Hop size (50% overlap)
    hop_size: usize,
    /// Sample rate
    sample_rate: u32,

    /// Path to HRTF file
    hrtf_path: Option<PathBuf>,

    /// Speaker configuration for input channels
    speaker_config: &'static SpeakerConfig,

    /// Real FFT planners
    fft_r2c: Arc<dyn RealToComplex<f32>>,
    fft_c2r: Arc<dyn ComplexToReal<f32>>,
    freq_size: usize,

    /// Thread-safe state container
    state: Arc<RwLock<BinauralState>>,

    /// LFE low-pass filter
    lfe_lowpass_filter: Vec<Complex<f32>>,
    /// LFE gain
    lfe_gain: f32,

    /// LFE channel indices
    lfe_channels: Vec<usize>,

    /// Input buffer accumulator
    input_buffer: Vec<f32>,
    input_buffer_fill: usize,

    /// Output accumulator
    output_accumulator: Vec<Vec<f32>>,
    output_accumulator_fill: usize,
    next_add_position: usize,
    output_read_position: usize,

    /// Temporary buffers
    temp_input_block: Vec<f32>,
    temp_output_block: Vec<f32>,
    temp_freq_buffer: Vec<Complex<f32>>,
    temp_time_buffer: Vec<f32>,
    temp_fft_scratch: Vec<Complex<f32>>,

    // Working buffers for process_audio_block (RT safety)
    sum_left: Vec<Complex<f32>>,
    sum_right: Vec<Complex<f32>>,
    left_output: Vec<f32>,
    right_output: Vec<f32>,
    channel_output: Vec<f32>,
    lfe_time: Vec<f32>,
    lfe_freq: Vec<Complex<f32>>,
    lfe_output: Vec<f32>,

    // Parameters
    param_enable_optimization: ParameterId,
    enable_optimization: bool,

    param_externalization: ParameterId,
    externalization: Smoother, // Smoothed

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
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        input_channels: usize,
        fft_size: usize,
        hrtf_path: Option<PathBuf>,
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
        let sample_rate = 44100;

        // Overflow checks for buffer allocations
        let input_buffer_size = hop_size
            .checked_mul(input_channels)
            .expect("Buffer size overflow");
        let freq_size = fft_size / 2 + 1;
        let output_acc_size = fft_size.checked_mul(2).expect("Buffer size overflow");

        assert!(
            input_buffer_size <= 1 << 24,
            "Input buffer size unreasonably large (> 16MB)"
        );
        assert!(fft_size <= 1 << 16, "FFT size unreasonably large (> 65536)");

        let mut planner = RealFftPlanner::<f32>::new();
        let fft_r2c = planner.plan_fft_forward(fft_size);
        let fft_c2r = planner.plan_fft_inverse(fft_size);

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

        let scratch_len = fft_r2c.get_scratch_len().max(fft_c2r.get_scratch_len());

        // Initial state
        let state = Arc::new(RwLock::new(BinauralState {
            hrtf_filters_freq: vec![vec![Complex::new(0.0, 0.0); freq_size * 2]; input_channels],
            diffuse_field_eq_filter: None,
            hrtf_data: None,
        }));

        Self {
            input_channels,
            fft_size,
            hop_size,
            sample_rate,

            hrtf_path,
            speaker_config,

            fft_r2c,
            fft_c2r,
            freq_size,

            state,
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

            sum_left: vec![Complex::new(0.0, 0.0); freq_size],
            sum_right: vec![Complex::new(0.0, 0.0); freq_size],
            left_output: vec![0.0; fft_size],
            right_output: vec![0.0; fft_size],
            channel_output: vec![0.0; fft_size],
            lfe_time: vec![0.0; fft_size],
            lfe_freq: vec![Complex::new(0.0, 0.0); freq_size],
            lfe_output: vec![0.0; fft_size],

            param_enable_optimization: ParameterId::from("enable_optimization"),
            enable_optimization,

            param_externalization: ParameterId::from("externalization"),
            externalization: Smoother::new(externalization, 50.0, sample_rate),

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
        let hrtf_path = if params.hrtf_file.is_empty() {
            None
        } else {
            Some(PathBuf::from(params.hrtf_file))
        };

        Self::new(
            params.input_channels,
            params.fft_size,
            hrtf_path,
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

    /// Update filters (Async or Sync)
    fn update_filters(&mut self, sync: bool) {
        // We need to clone specific data to move into the thread/closure
        // to avoid 'static lifetime issues with self
        let state_arc = self.state.clone();
        let speaker_config = self.speaker_config; // Static reference, cheap copy
        let fft_size = self.fft_size;
        let sample_rate = self.sample_rate;
        let near_field_strength = self.near_field_strength;
        let diffuse_field_eq = self.diffuse_field_eq;
        let fft_r2c = self.fft_r2c.clone();
        let freq_size = self.freq_size;
        let input_channels = self.input_channels;
        // Cloning Vec<usize> is cheap enough
        let lfe_channels = self.lfe_channels.clone();

        let task = move || {
            // Read existing SOFA data
            let sofa_opt = {
                let lock = state_arc.read();
                lock.hrtf_data.clone()
            };

            if let Some(sofa) = sofa_opt {
                // Compute new filters
                let mut new_filters =
                    vec![vec![Complex::new(0.0, 0.0); freq_size * 2]; input_channels];

                for (i, speaker) in speaker_config.speakers.iter().enumerate() {
                    if speaker.is_lfe {
                        continue;
                    }

                    let target_pos = room::speaker_to_source_position(speaker);
                    let nearest: [(usize, f32); 3] = sofa.find_three_nearest(&target_pos);
                    let gains = hrtf::calculate_vbap_gains(&target_pos, &nearest, &sofa);

                    let (left_fft, right_fft) = hrtf::interpolate_hrtf_frequency_domain(
                        &nearest,
                        &gains,
                        &sofa,
                        fft_size,
                        sample_rate,
                        &fft_r2c,
                        near_field_strength,
                        speaker.azimuth,
                        speaker.elevation,
                    );

                    let combined: Vec<Complex<f32>> =
                        left_fft.into_iter().chain(right_fft.into_iter()).collect();
                    new_filters[i] = combined;
                }

                hrtf::normalize_hrtf_gains(
                    &mut new_filters,
                    &lfe_channels,
                    freq_size,
                    input_channels,
                );

                let mut new_df_eq = None;
                if diffuse_field_eq {
                    if let Ok(eq) =
                        filter::compute_diffuse_field_eq(&sofa, fft_size, sample_rate, &fft_r2c)
                    {
                        new_df_eq = Some(eq);
                    }
                }

                // Write back
                let mut lock = state_arc.write();
                lock.hrtf_filters_freq = new_filters;
                lock.diffuse_field_eq_filter = new_df_eq;
            }
        };

        if sync {
            task();
        } else {
            rayon::spawn(task);
        }
    }

    /// Load HRTF data from a file
    pub fn load_hrtf(&mut self, path: PathBuf) -> Result<(), String> {
        log::debug!("[BinauralDecoder] Loading HRTF file: {:?}", path);

        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        let mut sofa = match extension {
            "hrtfdb" => SofaFile::load_sqlite(&path)
                .map_err(|e| BinauralError::SofaLoadError(e).to_string()),
            "sofa" => {
                #[cfg(feature = "sofa_support")]
                {
                    SofaFile::load(&path).map_err(|e| BinauralError::SofaLoadError(e).to_string())
                }
                #[cfg(not(feature = "sofa_support"))]
                {
                    Err(BinauralError::SofaLoadError(
                        "SOFA support not enabled in this build.".to_string(),
                    )
                    .to_string())
                }
            }
            _ => Err(BinauralError::SofaLoadError(format!(
                "Unsupported HRTF file extension: '{}'",
                extension
            ))
            .to_string()),
        }?;

        // Check if resampling is needed
        let sample_rate_diff = (sofa.sample_rate - self.sample_rate as f32).abs();
        if sample_rate_diff > 1.0 {
            log::info!(
                "[BinauralDecoder] Resampling HRTF data from {} Hz to {} Hz",
                sofa.sample_rate,
                self.sample_rate
            );
            Self::resample_sofa(&mut sofa, self.sample_rate)?;
        }

        // Update state with new SOFA data
        {
            let mut lock = self.state.write();
            lock.hrtf_data = Some(sofa);
        }
        self.hrtf_path = Some(path);

        // Update filters synchronously since we just loaded a file
        self.update_filters(true);

        log::debug!("[BinauralDecoder] HRTF file loaded and filters prepared");
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

        // Access filters via read lock
        let state_guard = self.state.read();
        let filters = &state_guard.hrtf_filters_freq;
        let df_eq = &state_guard.diffuse_field_eq_filter;

        let input_block = std::mem::take(&mut self.temp_input_block);
        let mut output_block = std::mem::take(&mut self.temp_output_block);
        let mut time_buffer = std::mem::take(&mut self.temp_time_buffer);
        let mut freq_buffer = std::mem::take(&mut self.temp_freq_buffer);
        let mut scratch = std::mem::take(&mut self.temp_fft_scratch);

        freq_buffer.fill(Complex::new(0.0, 0.0));

        if self.enable_optimization {
            // Sum-Before-IFFT optimization - use pre-allocated buffers
            let mut sum_left = std::mem::take(&mut self.sum_left);
            let mut sum_right = std::mem::take(&mut self.sum_right);
            sum_left.fill(Complex::new(0.0, 0.0));
            sum_right.fill(Complex::new(0.0, 0.0));

            for ch in 0..self.input_channels {
                if self.lfe_channels.contains(&ch) {
                    continue;
                }

                // Extract channel data
                for i in 0..self.hop_size {
                    time_buffer[i] = input_block[i * self.input_channels + ch];
                }
                time_buffer[self.hop_size..self.fft_size].fill(0.0);

                // Real-to-Complex FFT
                self.fft_r2c
                    .process_with_scratch(&mut time_buffer, &mut freq_buffer, &mut scratch)
                    .expect("FFT forward failed");

                // Accumulate weighted by HRTF
                let hrtf = &filters[ch];
                complex_mul_add_simd(&mut sum_left, &freq_buffer, &hrtf[0..self.freq_size]);
                complex_mul_add_simd(&mut sum_right, &freq_buffer, &hrtf[self.freq_size..]);
            }

            // Apply diffuse-field EQ if enabled
            if let Some(df_eq) = df_eq {
                for (k, val) in sum_left.iter_mut().enumerate().take(self.freq_size) {
                    *val *= df_eq[0][k];
                    sum_right[k] *= df_eq[1][k];
                }
            }

            // Enforce real FFT constraints
            sum_left[0].im = 0.0;
            sum_right[0].im = 0.0;
            sum_left[self.freq_size - 1].im = 0.0;
            sum_right[self.freq_size - 1].im = 0.0;

            // Inverse FFT for left ear
            let mut left_output = std::mem::take(&mut self.left_output);
            self.fft_c2r
                .process_with_scratch(&mut sum_left, &mut left_output, &mut scratch)
                .expect("FFT inverse failed");

            // Inverse FFT for right ear
            let mut right_output = std::mem::take(&mut self.right_output);
            self.fft_c2r
                .process_with_scratch(&mut sum_right, &mut right_output, &mut scratch)
                .expect("FFT inverse failed");

            // Normalization
            let scale = 1.0 / self.fft_size as f32;
            for i in 0..self.fft_size {
                output_block[i * 2] = left_output[i] * scale;
                output_block[i * 2 + 1] = right_output[i] * scale;
            }

            // Restore buffers
            self.sum_left = sum_left;
            self.sum_right = sum_right;
            self.left_output = left_output;
            self.right_output = right_output;
        } else {
            // Non-optimized path
            output_block.fill(0.0);
            let mut channel_output = std::mem::take(&mut self.channel_output);

            for ch in 0..self.input_channels {
                if self.lfe_channels.contains(&ch) {
                    continue;
                }

                // Extract channel data
                for i in 0..self.hop_size {
                    time_buffer[i] = input_block[i * self.input_channels + ch];
                }
                time_buffer[self.hop_size..self.fft_size].fill(0.0);

                // Forward FFT
                self.fft_r2c
                    .process_with_scratch(&mut time_buffer, &mut freq_buffer, &mut scratch)
                    .expect("FFT forward failed");

                // Process left ear
                let mut left_freq = std::mem::take(&mut self.sum_left); // reuse sum_left as temp freq
                left_freq.fill(Complex::new(0.0, 0.0));
                complex_mul_simd(
                    &mut left_freq,
                    &freq_buffer,
                    &filters[ch][0..self.freq_size],
                );

                if let Some(df_eq) = df_eq {
                    for (k, val) in left_freq.iter_mut().enumerate().take(self.freq_size) {
                        *val *= df_eq[0][k];
                    }
                }

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
                let mut right_freq = left_freq; // reuse
                right_freq.fill(Complex::new(0.0, 0.0));
                complex_mul_simd(
                    &mut right_freq,
                    &freq_buffer,
                    &filters[ch][self.freq_size..],
                );

                if let Some(df_eq) = df_eq {
                    for (k, val) in right_freq.iter_mut().enumerate().take(self.freq_size) {
                        *val *= df_eq[1][k];
                    }
                }

                right_freq[0].im = 0.0;
                right_freq[self.freq_size - 1].im = 0.0;

                self.fft_c2r
                    .process_with_scratch(&mut right_freq, &mut channel_output, &mut scratch)
                    .expect("FFT inverse failed");

                for i in 0..self.fft_size {
                    output_block[i * 2 + 1] += channel_output[i] * scale;
                }
                self.sum_left = right_freq; // restore
            }
            self.channel_output = channel_output;
        }

        // Release lock
        drop(state_guard);

        self.temp_input_block = input_block;
        self.temp_output_block = output_block;
        self.temp_time_buffer = time_buffer;
        self.temp_freq_buffer = freq_buffer;
        self.temp_fft_scratch = scratch;

        // Process LFE channels (mixed to both ears with lowpass filter)
        if !self.lfe_channels.is_empty() {
            let mut lfe_time = std::mem::take(&mut self.lfe_time);
            let mut lfe_freq = std::mem::take(&mut self.lfe_freq);
            let mut lfe_output = std::mem::take(&mut self.lfe_output);

            for &lfe_ch in &self.lfe_channels {
                // Extract LFE channel data
                for (i, val) in lfe_time.iter_mut().enumerate().take(self.hop_size) {
                    *val = self.temp_input_block[i * self.input_channels + lfe_ch];
                }
                lfe_time[self.hop_size..self.fft_size].fill(0.0);

                // Forward FFT
                self.fft_r2c
                    .process_with_scratch(&mut lfe_time, &mut lfe_freq, &mut self.temp_fft_scratch)
                    .expect("LFE FFT forward failed");

                // Apply lowpass filter
                for (k, val) in lfe_freq.iter_mut().enumerate().take(self.freq_size) {
                    *val *= self.lfe_lowpass_filter[k];
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
                for (i, val) in lfe_output.iter().enumerate().take(self.hop_size) {
                    let lfe_sample = *val * scale;
                    self.temp_output_block[i * 2] += lfe_sample;
                    self.temp_output_block[i * 2 + 1] += lfe_sample;
                }
            }
            self.lfe_time = lfe_time;
            self.lfe_freq = lfe_freq;
            self.lfe_output = lfe_output;
        }

        let externalization = self.externalization.next();
        if externalization > 0.01 {
            self.apply_externalization(externalization);
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

    fn apply_externalization(&mut self, externalization: f32) {
        if self.cached_reflections.is_empty() {
            self.calculate_reflections();
        }

        for reflection in &self.cached_reflections {
            let delay_samples = reflection.delay_samples;
            let reflection_gain = reflection.gain * externalization;

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

        if externalization > 0.5 {
            let diffuse_gain = (externalization - 0.5) * 0.3;
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
        PluginInfo::new("Binaural Decoder", "1.2.0", "SotF").with_description(format!(
            "Converts {}-channel audio to binaural stereo using HRTFs from a file (Async)",
            self.input_channels
        ))
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
                .with_description("Enable Sum-Before-IFFT optimization")
                .with_group("Optimization")
                .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float("externalization", "Externalization", 0.0, 0.0, 1.0)
                .with_description("Room simulation / externalization factor")
                .with_group("Space")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_float("near_field_strength", "Near-Field", 0.0, 0.0, 1.0)
                .with_description("Near-field shadowing strength")
                .with_group("Space")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("diffuse_field_eq", "Diffuse-Field EQ", true)
                .with_description("Compensate for HRTF coloration (improves timbre)")
                .with_group("Tone")
                .with_importance(ParameterImportance::Useful),
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
                self.externalization.set_target(v);
                return Ok(());
            }
        } else if id == self.param_near_field_strength {
            if let Some(v) = value.as_float()
                && (0.0..=1.0).contains(&v)
            {
                self.near_field_strength = v;
                // Trigger async update
                self.update_filters(false);
                return Ok(());
            }
        } else if id == self.param_diffuse_field_eq
            && let Some(v) = value.as_bool()
        {
            self.diffuse_field_eq = v;
            // Trigger async update
            self.update_filters(false);
            return Ok(());
        }
        Err(format!("Unknown parameter or invalid value: {}", id))
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_enable_optimization {
            Some(ParameterValue::Bool(self.enable_optimization))
        } else if id == &self.param_externalization {
            Some(ParameterValue::Float(self.externalization.target()))
        } else if id == &self.param_near_field_strength {
            Some(ParameterValue::Float(self.near_field_strength))
        } else if id == &self.param_diffuse_field_eq {
            Some(ParameterValue::Bool(self.diffuse_field_eq))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        const MIN_SAMPLE_RATE: u32 = 8_000;
        const MAX_SAMPLE_RATE: u32 = 384_000;

        if sample_rate < MIN_SAMPLE_RATE || sample_rate > MAX_SAMPLE_RATE {
            return Err(format!(
                "Invalid sample rate: {} Hz (valid range: {}-{} Hz)",
                sample_rate, MIN_SAMPLE_RATE, MAX_SAMPLE_RATE
            ));
        }

        self.sample_rate = sample_rate;
        self.externalization.set_time(50.0, sample_rate);

        let (filter, gain) = filter::compute_lfe_filter(
            self.fft_size,
            self.sample_rate,
            self.lfe_crossover,
            self.lfe_distance,
            self.lfe_level,
        );
        self.lfe_lowpass_filter = filter;
        self.lfe_gain = gain;

        if let Some(path) = self.hrtf_path.clone() {
            self.load_hrtf(path)
                .map_err(|e| format!("Failed to load HRTF file: {}", e))?;
        } else {
            log::debug!("[BinauralDecoder] No HRTF file specified, plugin will pass through audio");
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
    ) -> Result<usize, String> {
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

        // Check if HRTF data is loaded (via read lock on state)
        let has_data = {
            let lock = self.state.read();
            lock.hrtf_data.is_some()
        };

        if !has_data {
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
            return Ok(context.num_frames);
        }

        // let start_time = std::time::Instant::now();

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

        // let elapsed = start_time.elapsed();
        // if elapsed > std::time::Duration::from_millis(3) {
        //     log::warn!(
        //         "[BinauralDecoder] Slow processing: {:.2}ms for {} input frames",
        //         elapsed.as_secs_f64() * 1000.0,
        //         context.num_frames
        //     );
        // }

        Ok(context.num_frames)
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
        assert_eq!(plugin.externalization.target(), 0.0);
        assert_eq!(plugin.near_field_strength, 0.0);
    }
}
