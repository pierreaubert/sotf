// ============================================================================
// Binaural Decoder Plugin
// ============================================================================

use arc_swap::ArcSwap;
use math_audio_dsp::rtpghi::RtpghiProcessor;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use crate::params::PARAMS as BN;
use sotf_host::param_bridge;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{complex_mul_add_simd, enable_ftz_daz, window_mul_simd};
use sotf_host::smoothing::Smoother;
use sotf_host::sofa::SofaFile;
use sotf_host::speaker_config::{SpeakerConfig, get_speaker_config_by_channels};
use std::path::PathBuf;
use std::sync::Arc;

pub mod config;
pub mod error;
pub mod filter;
pub mod hrtf;
pub mod hrtf_database;
pub mod params;
pub mod room;

pub use self::config::{
    BinauralDecoderParams, default_enable_optimization as binaural_default_enable_optimization,
};
pub use self::error::BinauralError;
pub use self::room::{Reflection, RoomModel};

struct BinauralState {
    hrtf_filters_freq: Vec<Vec<Complex<f32>>>,
    diffuse_field_eq_filter: Option<[Vec<Complex<f32>>; 2]>,
    _hrtf_data: Option<SofaFile>,
}

pub struct BinauralDecoderPlugin {
    input_channels: usize,
    fft_size: usize,
    hop_size: usize,
    sample_rate: u32,
    hrtf_path: Option<PathBuf>,
    speaker_config: &'static SpeakerConfig,
    fft_r2c: Arc<dyn RealToComplex<f32>>,
    fft_c2r: Arc<dyn ComplexToReal<f32>>,
    freq_size: usize,
    state: Arc<ArcSwap<BinauralState>>,

    lfe_lowpass_filter: Vec<Complex<f32>>,
    lfe_gain: f32,
    lfe_channels: Vec<usize>,
    main_channels: Vec<usize>,

    /// Flat input buffer
    input_buffer: Vec<f32>,
    input_fill: usize,

    /// Interleaved output ring buffer [L0, R0, L1, R1, ...]
    output_accumulator: Vec<f32>,
    output_accumulator_mask: usize,
    output_accumulator_fill: usize,
    next_add_position: usize,
    output_read_position: usize,

    output_scale: f32,
    analysis_window: Vec<f32>,

    /// Temporary working buffers
    temp_freq_buffer: Vec<Complex<f32>>,
    temp_fft_scratch: Vec<Complex<f32>>,
    sum_left: Vec<Complex<f32>>,
    sum_right: Vec<Complex<f32>>,
    lfe_freq: Vec<Complex<f32>>,
    ifft_output_buf: Vec<f32>,

    externalization: Smoother,
    near_field_strength: f32,
    diffuse_field_eq: bool,
    lfe_crossover: f32,
    lfe_distance: f32,
    lfe_level: f32,
    room_model: RoomModel,
    cached_reflections: Vec<Reflection>,

    /// Delay line for room reflections
    reflection_delay_line: Vec<f32>,
    reflection_delay_pos: usize,
    reflection_delay_mask: usize,

    // --- Phase 4E: Late reverb FDN ---
    fdn: math_audio_dsp::fdn::Fdn,
    late_reverb_enabled: bool,
    late_reverb_mix: f32,
    late_reverb_rt60: f32,
    late_reverb_damping: f32,
    headphone_eq_enabled: bool,

    /// Crossfade state for smooth HRTF transitions.
    /// When HRTF filters change, we blend from old to new over ~50ms.
    /// `current_state_snapshot` tracks the last-seen Arc so we can detect changes.
    current_state_snapshot: Arc<BinauralState>,
    crossfade_prev_state: Option<Arc<BinauralState>>,
    crossfade_remaining: usize,
    crossfade_total: usize,
    /// Temporary buffers for crossfade blending (old filter output)
    crossfade_sum_left: Vec<Complex<f32>>,
    crossfade_sum_right: Vec<Complex<f32>>,

    /// Crossfade mode: 0 = Linear (complex blend), 1 = Spectral (magnitude interpolation + RTPGHI)
    crossfade_mode_index: usize,
    /// RTPGHI processors for spectral crossfade (one per ear), created lazily in initialize()
    rtpghi_left: Option<RtpghiProcessor>,
    rtpghi_right: Option<RtpghiProcessor>,
    /// Pre-allocated magnitude scratch buffers for spectral crossfade
    crossfade_mag_left: Vec<f32>,
    crossfade_mag_right: Vec<f32>,
    /// Pre-allocated phase output buffers for RTPGHI
    crossfade_phase_left: Vec<f32>,
    crossfade_phase_right: Vec<f32>,

    latency_filled: usize,
    cached_parameters: Vec<Parameter>,

    /// Crossfade duration in milliseconds (range: 10–500ms, default: 50ms).
    crossfade_ms: f32,

    /// Head tracking angles in degrees. Positive yaw = head turned left.
    /// The inverse rotation is applied to speaker positions before VBAP lookup,
    /// so a head turn left makes all virtual sources shift right (world-locked).
    head_yaw_deg: Smoother,
    head_pitch_deg: Smoother,
    head_roll_deg: Smoother,

    /// Last head angles used when computing the current HRTF state.
    /// Used for the 0.5° change threshold to avoid unnecessary recomputes.
    last_hrtf_yaw: f32,
    last_hrtf_pitch: f32,
    last_hrtf_roll: f32,

    // ---- Personalized HRTF selection ----
    /// Directory to scan for `.sofa` files.  When non-empty the plugin picks
    /// the best-matching file based on the anthropometric parameters below.
    hrtf_database_dir: String,
    /// Target head width in centimetres (range: 10–25 cm, default: 15 cm).
    head_width_cm: f32,
    /// Target ear height in centimetres (range: 4–16 cm, default: 10 cm).
    ear_height_cm: f32,
    enable_optimization: bool,
}

impl BinauralDecoderPlugin {
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
        let hop_size = fft_size / 4;
        let sr = 44100;
        let freq_size = fft_size / 2 + 1;
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_r2c = planner.plan_fft_forward(fft_size);
        let fft_c2r = planner.plan_fft_inverse(fft_size);
        let scratch_len = fft_r2c.get_scratch_len().max(fft_c2r.get_scratch_len());

        let speaker_config = get_speaker_config_by_channels(input_channels)
            .unwrap_or_else(|| get_speaker_config_by_channels(2).unwrap());

        let mut lfe_channels = Vec::new();
        let mut main_channels = Vec::new();
        for s in speaker_config.speakers {
            if s.channel < input_channels {
                if s.is_lfe {
                    lfe_channels.push(s.channel);
                } else {
                    main_channels.push(s.channel);
                }
            }
        }

        let analysis_window: Vec<f32> = (0..fft_size)
            .map(|i| {
                let x = i as f32 / fft_size as f32;
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * x).cos())
            })
            .collect();

        let output_scale = 1.0 / (fft_size as f32 * 2.0);

        let mut hrtf_filters_freq =
            vec![vec![Complex::new(0.0, 0.0); freq_size * 2]; input_channels];
        for &ch in &main_channels {
            if ch == 0 {
                hrtf_filters_freq[ch][0..freq_size].fill(Complex::new(1.0, 0.0));
            } else if ch == 1 {
                hrtf_filters_freq[ch][freq_size..].fill(Complex::new(1.0, 0.0));
            } else {
                hrtf_filters_freq[ch][0..freq_size].fill(Complex::new(0.707, 0.0));
                hrtf_filters_freq[ch][freq_size..].fill(Complex::new(0.707, 0.0));
            }
        }

        // Normalize default gains to prevent clipping
        hrtf::normalize_hrtf_gains(
            &mut hrtf_filters_freq,
            &lfe_channels,
            freq_size,
            input_channels,
        );

        let delay_size = 16384;

        let initial_state = Arc::new(BinauralState {
            hrtf_filters_freq,
            diffuse_field_eq_filter: None,
            _hrtf_data: None,
        });

        let mut p = Self {
            input_channels,
            fft_size,
            hop_size,
            sample_rate: sr,
            hrtf_path,
            speaker_config,
            fft_r2c,
            fft_c2r,
            freq_size,
            state: Arc::new(ArcSwap::from(initial_state.clone())),
            lfe_lowpass_filter: vec![Complex::new(1.0, 0.0); freq_size],
            lfe_gain: 1.0,
            lfe_channels,
            main_channels,
            input_buffer: vec![0.0; fft_size * input_channels],
            input_fill: 0,
            output_accumulator: vec![0.0; fft_size * 4 * 2],
            output_accumulator_mask: (fft_size * 4) - 1,
            output_accumulator_fill: 0,
            next_add_position: 0,
            output_read_position: 0,
            output_scale,
            analysis_window,
            temp_freq_buffer: vec![Complex::new(0.0, 0.0); freq_size],
            temp_fft_scratch: vec![Complex::new(0.0, 0.0); scratch_len],
            sum_left: vec![Complex::new(0.0, 0.0); freq_size],
            sum_right: vec![Complex::new(0.0, 0.0); freq_size],
            lfe_freq: vec![Complex::new(0.0, 0.0); freq_size],
            ifft_output_buf: vec![0.0; fft_size],
            externalization: Smoother::new(externalization, 50.0, sr),
            near_field_strength,
            diffuse_field_eq,
            lfe_crossover,
            lfe_distance,
            lfe_level,
            room_model,
            cached_reflections: Vec::new(),
            reflection_delay_line: vec![0.0; delay_size * 2],
            reflection_delay_pos: 0,
            reflection_delay_mask: delay_size - 1,
            // Phase 4E: Late reverb FDN
            fdn: math_audio_dsp::fdn::Fdn::new(8, sr),
            late_reverb_enabled: false,
            late_reverb_mix: 0.3,
            late_reverb_rt60: 1.0,
            late_reverb_damping: 0.3,
            headphone_eq_enabled: false,
            current_state_snapshot: initial_state,
            crossfade_prev_state: None,
            crossfade_remaining: 0,
            crossfade_total: 0,
            crossfade_sum_left: vec![Complex::new(0.0, 0.0); freq_size],
            crossfade_sum_right: vec![Complex::new(0.0, 0.0); freq_size],
            crossfade_mode_index: 0,
            rtpghi_left: None,
            rtpghi_right: None,
            crossfade_mag_left: vec![0.0; freq_size],
            crossfade_mag_right: vec![0.0; freq_size],
            crossfade_phase_left: vec![0.0; freq_size],
            crossfade_phase_right: vec![0.0; freq_size],
            latency_filled: 0,
            cached_parameters: Vec::new(),
            crossfade_ms: 50.0,
            head_yaw_deg: Smoother::new(0.0, 10.0, sr),
            head_pitch_deg: Smoother::new(0.0, 10.0, sr),
            head_roll_deg: Smoother::new(0.0, 10.0, sr),
            last_hrtf_yaw: 0.0,
            last_hrtf_pitch: 0.0,
            last_hrtf_roll: 0.0,
            hrtf_database_dir: String::new(),
            head_width_cm: 15.0,
            ear_height_cm: 10.0,
            enable_optimization,
        };
        p.rebuild_cached_parameters();
        p
    }

    /// Get the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => None, // sofa_file (FilePath — handled separately)
            1 => Some(self.input_channels as f64),
            2 => Some(if self.enable_optimization { 1.0 } else { 0.0 }),
            3 => Some(self.externalization.target() as f64),
            4 => Some(self.near_field_strength as f64),
            5 => Some(self.crossfade_mode_index as f64),
            6 => Some(if self.late_reverb_enabled { 1.0 } else { 0.0 }),
            7 => Some(self.late_reverb_mix as f64),
            8 => Some(self.late_reverb_rt60 as f64),
            9 => Some(self.late_reverb_damping as f64),
            10 => Some(if self.headphone_eq_enabled { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    /// Set the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => {} // sofa_file (FilePath — handled separately)
            1 => {}  // input_channels (construction-only, requires buffer rebuild)
            2 => self.enable_optimization = value > 0.5,
            3 => self.externalization.set_target(value as f32),
            4 => self.near_field_strength = value as f32,
            5 => self.crossfade_mode_index = value as usize,
            6 => self.late_reverb_enabled = value > 0.5,
            7 => self.late_reverb_mix = value as f32,
            8 => self.late_reverb_rt60 = value as f32,
            9 => self.late_reverb_damping = value as f32,
            10 => self.headphone_eq_enabled = value > 0.5,
            _ => {}
        }
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = param_bridge::build_parameters(BN, |i| self.param_value(i));
        // Append parameters not in PARAMS
        let hrtf_path_str = self
            .hrtf_path
            .as_ref()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_string();
        self.cached_parameters.push(
            Parameter::new_float("crossfade_ms", "Crossfade (ms)", self.crossfade_ms, 10.0, 500.0)
        );
        self.cached_parameters.push(
            Parameter::new_string("hrtf_file", "HRTF File", hrtf_path_str)
        );
        self.cached_parameters.push(
            Parameter::new_float("head_yaw_deg", "Head Yaw (deg)", self.head_yaw_deg.target(), -180.0, 180.0)
        );
        self.cached_parameters.push(
            Parameter::new_float("head_pitch_deg", "Head Pitch (deg)", self.head_pitch_deg.target(), -180.0, 180.0)
        );
        self.cached_parameters.push(
            Parameter::new_float("head_roll_deg", "Head Roll (deg)", self.head_roll_deg.target(), -180.0, 180.0)
        );
        self.cached_parameters.push(
            Parameter::new_string("hrtf_database_dir", "HRTF Database Dir", self.hrtf_database_dir.clone())
        );
        self.cached_parameters.push(
            Parameter::new_float("head_width_cm", "Head Width (cm)", self.head_width_cm, 10.0, 25.0)
        );
        self.cached_parameters.push(
            Parameter::new_float("ear_height_cm", "Ear Height (cm)", self.ear_height_cm, 4.0, 16.0)
        );
    }

    pub fn from_params(params: BinauralDecoderParams) -> Self {
        let hrtf_path = if params.hrtf_file.is_empty() {
            None
        } else {
            Some(PathBuf::from(params.hrtf_file))
        };
        let mut plugin = Self::new(
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
        );
        plugin.hrtf_database_dir = params.hrtf_database_dir;
        plugin.head_width_cm = params.head_width_cm;
        plugin.ear_height_cm = params.ear_height_cm;
        plugin.rebuild_cached_parameters();
        plugin
    }

    fn process_audio_block(&mut self) {
        // Detect state changes for crossfade
        let new_state = self.state.load_full();
        if !Arc::ptr_eq(&new_state, &self.current_state_snapshot) {
            // State changed -- start crossfade from old to new
            // Crossfade duration in samples, rounded up to hop_size boundary
            let crossfade_samples =
                (self.sample_rate as f32 * self.crossfade_ms * 0.001) as usize;
            let crossfade_hops = crossfade_samples.div_ceil(self.hop_size);
            let total = crossfade_hops * self.hop_size;

            log::debug!(
                "[BinauralDecoder] HRTF state changed, crossfading over {} samples ({} hops)",
                total,
                crossfade_hops
            );

            self.crossfade_prev_state = Some(self.current_state_snapshot.clone());
            self.crossfade_total = total;
            self.crossfade_remaining = total;
            self.current_state_snapshot = new_state.clone();

            // Reset RTPGHI state when starting a new crossfade so stale phase
            // history from a previous crossfade does not contaminate this one.
            if self.crossfade_mode_index == 1 {
                if let Some(ref mut rtpghi) = self.rtpghi_left {
                    rtpghi.reset();
                }
                if let Some(ref mut rtpghi) = self.rtpghi_right {
                    rtpghi.reset();
                }
            }
        }

        let state = &new_state;
        let filters = &state.hrtf_filters_freq;
        let df_eq = &state.diffuse_field_eq_filter;
        let n = self.fft_size;
        let freq_size = self.freq_size;
        let mask = self.output_accumulator_mask;
        let scale = self.output_scale;

        // Check if we need crossfade blending
        let crossfading = self.crossfade_remaining > 0 && self.crossfade_prev_state.is_some();

        if crossfading {
            let prev = self.crossfade_prev_state.as_ref().unwrap().clone();
            let prev_filters = &prev.hrtf_filters_freq;
            let prev_df_eq = &prev.diffuse_field_eq_filter;

            self.sum_left.fill(Complex::new(0.0, 0.0));
            self.sum_right.fill(Complex::new(0.0, 0.0));
            self.lfe_freq.fill(Complex::new(0.0, 0.0));

            // We need the FFT of each channel's input. Since both old and new use the same input,
            // we compute the FFT once per channel and apply both filter sets.
            // But the FFT output is stored in temp_freq_buffer, so we need to process per-channel.

            // Old state accumulators
            self.crossfade_sum_left.fill(Complex::new(0.0, 0.0));
            self.crossfade_sum_right.fill(Complex::new(0.0, 0.0));

            for &ch in &self.main_channels {
                let ch_offset = ch * n;
                window_mul_simd(
                    &mut self.ifft_output_buf,
                    &self.input_buffer[ch_offset..ch_offset + n],
                    &self.analysis_window,
                );

                self.fft_r2c
                    .process_with_scratch(
                        &mut self.ifft_output_buf,
                        &mut self.temp_freq_buffer,
                        &mut self.temp_fft_scratch,
                    )
                    .ok(); // Defensive: silence on FFT error instead of panic

                // New filters
                let hrtf_new = &filters[ch];
                complex_mul_add_simd(
                    &mut self.sum_left,
                    &self.temp_freq_buffer,
                    &hrtf_new[0..freq_size],
                );
                complex_mul_add_simd(
                    &mut self.sum_right,
                    &self.temp_freq_buffer,
                    &hrtf_new[freq_size..],
                );

                // Old filters
                let hrtf_old = &prev_filters[ch];
                complex_mul_add_simd(
                    &mut self.crossfade_sum_left,
                    &self.temp_freq_buffer,
                    &hrtf_old[0..freq_size],
                );
                complex_mul_add_simd(
                    &mut self.crossfade_sum_right,
                    &self.temp_freq_buffer,
                    &hrtf_old[freq_size..],
                );
            }

            // Apply diffuse field EQ to new output
            if let Some(eq) = df_eq {
                for (k, (sl, sr)) in self
                    .sum_left
                    .iter_mut()
                    .zip(self.sum_right.iter_mut())
                    .enumerate()
                    .take(freq_size)
                {
                    *sl *= eq[0][k];
                    *sr *= eq[1][k];
                }
            }

            // Apply diffuse field EQ to old output
            if let Some(eq) = prev_df_eq {
                for (k, (sl, sr)) in self
                    .crossfade_sum_left
                    .iter_mut()
                    .zip(self.crossfade_sum_right.iter_mut())
                    .enumerate()
                    .take(freq_size)
                {
                    *sl *= eq[0][k];
                    *sr *= eq[1][k];
                }
            }

            // Blend old and new in frequency domain using crossfade gain
            // Linear crossfade: new_gain goes from 0.0 to 1.0 over the crossfade period
            let new_gain = if self.crossfade_total > 0 {
                1.0 - (self.crossfade_remaining as f32 / self.crossfade_total as f32)
            } else {
                1.0
            };
            let old_gain = 1.0 - new_gain;

            let use_spectral = self.crossfade_mode_index == 1
                && self.rtpghi_left.is_some()
                && self.rtpghi_right.is_some();

            if use_spectral {
                // Spectral mode: magnitude interpolation + RTPGHI phase reconstruction
                // This avoids comb-filter artifacts from complex-domain blending.
                for k in 0..freq_size {
                    let mag_new_l = (self.sum_left[k].re * self.sum_left[k].re
                        + self.sum_left[k].im * self.sum_left[k].im)
                        .sqrt();
                    let mag_old_l = (self.crossfade_sum_left[k].re
                        * self.crossfade_sum_left[k].re
                        + self.crossfade_sum_left[k].im * self.crossfade_sum_left[k].im)
                        .sqrt();
                    self.crossfade_mag_left[k] = mag_old_l * old_gain + mag_new_l * new_gain;

                    let mag_new_r = (self.sum_right[k].re * self.sum_right[k].re
                        + self.sum_right[k].im * self.sum_right[k].im)
                        .sqrt();
                    let mag_old_r = (self.crossfade_sum_right[k].re
                        * self.crossfade_sum_right[k].re
                        + self.crossfade_sum_right[k].im * self.crossfade_sum_right[k].im)
                        .sqrt();
                    self.crossfade_mag_right[k] = mag_old_r * old_gain + mag_new_r * new_gain;
                }

                // RTPGHI phase reconstruction from interpolated magnitudes
                // Safety: use_spectral already checked is_some() above.
                let rtpghi_l = self.rtpghi_left.as_mut().expect("checked above");
                rtpghi_l.process_frame_into(
                    &self.crossfade_mag_left[..freq_size],
                    &mut self.crossfade_phase_left[..freq_size],
                );
                let rtpghi_r = self.rtpghi_right.as_mut().expect("checked above");
                rtpghi_r.process_frame_into(
                    &self.crossfade_mag_right[..freq_size],
                    &mut self.crossfade_phase_right[..freq_size],
                );

                // Reconstruct complex spectrum from blended magnitude + reconstructed phase
                for k in 0..freq_size {
                    let (sin_l, cos_l) = (self.crossfade_phase_left[k] as f64).sin_cos();
                    self.sum_left[k] = Complex::new(
                        self.crossfade_mag_left[k] * cos_l as f32,
                        self.crossfade_mag_left[k] * sin_l as f32,
                    );

                    let (sin_r, cos_r) = (self.crossfade_phase_right[k] as f64).sin_cos();
                    self.sum_right[k] = Complex::new(
                        self.crossfade_mag_right[k] * cos_r as f32,
                        self.crossfade_mag_right[k] * sin_r as f32,
                    );
                }
            } else {
                // Linear mode: simple complex-domain blend (original behavior)
                for k in 0..freq_size {
                    self.sum_left[k] =
                        self.sum_left[k] * new_gain + self.crossfade_sum_left[k] * old_gain;
                    self.sum_right[k] =
                        self.sum_right[k] * new_gain + self.crossfade_sum_right[k] * old_gain;
                }
            }

            // Advance crossfade
            self.crossfade_remaining = self.crossfade_remaining.saturating_sub(self.hop_size);
            if self.crossfade_remaining == 0 {
                self.crossfade_prev_state = None;
                log::debug!("[BinauralDecoder] HRTF crossfade complete");
            }
        } else {
            // Normal path -- no crossfade
            self.sum_left.fill(Complex::new(0.0, 0.0));
            self.sum_right.fill(Complex::new(0.0, 0.0));
            self.lfe_freq.fill(Complex::new(0.0, 0.0));

            for &ch in &self.main_channels {
                let ch_offset = ch * n;
                window_mul_simd(
                    &mut self.ifft_output_buf,
                    &self.input_buffer[ch_offset..ch_offset + n],
                    &self.analysis_window,
                );

                self.fft_r2c
                    .process_with_scratch(
                        &mut self.ifft_output_buf,
                        &mut self.temp_freq_buffer,
                        &mut self.temp_fft_scratch,
                    )
                    .ok(); // Defensive: silence on FFT error instead of panic
                let hrtf = &filters[ch];
                complex_mul_add_simd(
                    &mut self.sum_left,
                    &self.temp_freq_buffer,
                    &hrtf[0..freq_size],
                );
                complex_mul_add_simd(
                    &mut self.sum_right,
                    &self.temp_freq_buffer,
                    &hrtf[freq_size..],
                );
            }

            if let Some(eq) = df_eq {
                for (k, (sl, sr)) in self
                    .sum_left
                    .iter_mut()
                    .zip(self.sum_right.iter_mut())
                    .enumerate()
                    .take(freq_size)
                {
                    *sl *= eq[0][k];
                    *sr *= eq[1][k];
                }
            }
        }

        // LFE processing (same for both paths -- LFE doesn't use HRTF filters)
        if !crossfading {
            // lfe_freq already zeroed above in normal path
        } else {
            self.lfe_freq.fill(Complex::new(0.0, 0.0));
        }
        for &ch in &self.lfe_channels {
            let ch_offset = ch * n;
            window_mul_simd(
                &mut self.ifft_output_buf,
                &self.input_buffer[ch_offset..ch_offset + n],
                &self.analysis_window,
            );

            self.fft_r2c
                .process_with_scratch(
                    &mut self.ifft_output_buf,
                    &mut self.temp_freq_buffer,
                    &mut self.temp_fft_scratch,
                )
                .ok(); // Defensive: silence on FFT error instead of panic
            complex_mul_add_simd(
                &mut self.lfe_freq,
                &self.temp_freq_buffer,
                &self.lfe_lowpass_filter,
            );
        }

        // Left IFFT
        self.sum_left[0].im = 0.0;
        self.sum_left[freq_size - 1].im = 0.0;
        self.fft_c2r
            .process_with_scratch(
                &mut self.sum_left,
                &mut self.ifft_output_buf,
                &mut self.temp_fft_scratch,
            )
            .ok(); // Defensive: silence on FFT error instead of panic
        for i in 0..n {
            let idx = (self.next_add_position + i) & mask;
            self.output_accumulator[idx * 2] += self.ifft_output_buf[i] * scale;
        }

        // Right IFFT
        self.sum_right[0].im = 0.0;
        self.sum_right[freq_size - 1].im = 0.0;
        self.fft_c2r
            .process_with_scratch(
                &mut self.sum_right,
                &mut self.ifft_output_buf,
                &mut self.temp_fft_scratch,
            )
            .ok(); // Defensive: silence on FFT error instead of panic
        for i in 0..n {
            let idx = (self.next_add_position + i) & mask;
            self.output_accumulator[idx * 2 + 1] += self.ifft_output_buf[i] * scale;
        }

        // LFE IFFT
        if !self.lfe_channels.is_empty() {
            self.lfe_freq[0].im = 0.0;
            self.lfe_freq[freq_size - 1].im = 0.0;
            self.fft_c2r
                .process_with_scratch(
                    &mut self.lfe_freq,
                    &mut self.ifft_output_buf,
                    &mut self.temp_fft_scratch,
                )
                .ok(); // Defensive: silence on FFT error instead of panic
            let lfe_g = scale * self.lfe_gain;
            for i in 0..n {
                let idx = (self.next_add_position + i) & mask;
                let s = self.ifft_output_buf[i] * lfe_g;
                self.output_accumulator[idx * 2] += s;
                self.output_accumulator[idx * 2 + 1] += s;
            }
        }

        self.next_add_position = (self.next_add_position + self.hop_size) & mask;
        self.output_accumulator_fill += self.hop_size;
        self.latency_filled += self.hop_size;
    }

    fn apply_reflections(&mut self, output: &mut [f32], nf: usize) {
        let ext = self.externalization.current();
        let delay_mask = self.reflection_delay_mask;

        for i in 0..nf {
            let l = output[i * 2];
            let r = output[i * 2 + 1];
            self.reflection_delay_line[self.reflection_delay_pos * 2] = l;
            self.reflection_delay_line[self.reflection_delay_pos * 2 + 1] = r;

            if ext > 0.01 && !self.cached_reflections.is_empty() {
                let mut rl = 0.0;
                let mut rr = 0.0;
                for ref_ in &self.cached_reflections {
                    let r_pos = (self.reflection_delay_pos + delay_mask + 1 - ref_.delay_samples)
                        & delay_mask;
                    let g = ref_.gain * ext;
                    rl += self.reflection_delay_line[r_pos * 2] * g * ref_.left_gain;
                    rr += self.reflection_delay_line[r_pos * 2 + 1] * g * ref_.right_gain;
                }
                output[i * 2] += rl;
                output[i * 2 + 1] += rr;
            }
            self.reflection_delay_pos = (self.reflection_delay_pos + 1) & delay_mask;
        }
    }

    /// Apply the inverse of the head rotation to a speaker's (azimuth, elevation) pair.
    ///
    /// Head rotation convention:
    ///   - Yaw (Z axis): positive = head turned left
    ///   - Pitch (Y axis): positive = head tilted up
    ///   - Roll (X axis): positive = head tilted right
    ///
    /// To make virtual sources appear world-locked (they stay fixed while the head moves),
    /// we apply the *inverse* head rotation to the speaker positions before VBAP lookup.
    /// The inverse rotation is R_roll^T * R_pitch^T * R_yaw^T.
    fn rotate_speaker_position(azimuth: f32, elevation: f32, yaw: f32, pitch: f32, roll: f32) -> (f32, f32) {
        if yaw == 0.0 && pitch == 0.0 && roll == 0.0 {
            return (azimuth, elevation);
        }

        // Convert spherical -> Cartesian unit vector
        // Coordinate system matches SourcePosition::to_cartesian_unit_vector:
        //   x = cos(el)*cos(az), y = cos(el)*sin(az), z = sin(el)
        let az = azimuth.to_radians();
        let el = elevation.to_radians();
        let x = el.cos() * az.cos();
        let y = el.cos() * az.sin();
        let z = el.sin();

        // Apply inverse head rotation (transpose = inverse for orthogonal matrices).
        // Forward rotation order is Rz(yaw) * Ry(pitch) * Rx(roll).
        // Inverse is Rx(-roll) * Ry(-pitch) * Rz(-yaw).

        let yaw_r = yaw.to_radians();
        let pitch_r = pitch.to_radians();
        let roll_r = roll.to_radians();

        let (sy, cy) = yaw_r.sin_cos();
        let (sp, cp) = pitch_r.sin_cos();
        let (sr, cr) = roll_r.sin_cos();

        // Rz(-yaw): rotate around Z by -yaw
        let x1 =  cy * x + sy * y;
        let y1 = -sy * x + cy * y;
        let z1 = z;

        // Ry(-pitch): rotate around Y by -pitch
        let x2 = cp * x1 + sp * z1;
        let y2 = y1;
        let z2 = -sp * x1 + cp * z1;

        // Rx(-roll): rotate around X by -roll
        let x3 = x2;
        let y3 =  cr * y2 + sr * z2;
        let z3 = -sr * y2 + cr * z2;

        // Convert back to spherical coordinates
        let new_az = y3.atan2(x3).to_degrees();
        let horiz = (x3 * x3 + y3 * y3).sqrt();
        let new_el = z3.atan2(horiz).to_degrees();

        (new_az, new_el)
    }

    /// Recompute HRTF filters with head-angle-rotated speaker positions and push a new state.
    ///
    /// This is called whenever head angles have changed by more than 0.5° since the last
    /// recompute. It only does meaningful work when a SOFA file is loaded.
    fn recompute_hrtf_for_head_angles(&mut self, yaw: f32, pitch: f32, roll: f32) -> PluginResult<()> {
        self.last_hrtf_yaw = yaw;
        self.last_hrtf_pitch = pitch;
        self.last_hrtf_roll = roll;

        // Without a SOFA file there are no measured HRTFs to re-query, so we leave
        // the default (identity) filters in place.
        let hrtf_path = match self.hrtf_path.clone() {
            Some(p) => p,
            None => return Ok(()),
        };

        if self.sample_rate == 0 {
            return Ok(());
        }

        let mut sofa = SofaFile::load(&hrtf_path)
            .map_err(|e| format!("Failed to load HRTF file for head tracking: {}", e))?;

        let sofa_rate = sofa.sample_rate.round() as u32;
        if sofa_rate != self.sample_rate {
            hrtf::resample_sofa(&mut sofa, self.sample_rate)
                .map_err(|e| format!("HRTF resample failed during head tracking: {}", e))?;
        }

        let mut filters =
            vec![vec![Complex::new(0.0, 0.0); self.freq_size * 2]; self.input_channels];

        for spk in self.speaker_config.speakers {
            let ch = spk.channel;
            if ch >= self.input_channels || self.lfe_channels.contains(&ch) {
                continue;
            }
            // Apply inverse head rotation to the speaker's nominal position
            let (rotated_az, rotated_el) =
                Self::rotate_speaker_position(spk.azimuth, spk.elevation, yaw, pitch, roll);
            let tgt = sotf_host::sofa::SourcePosition::new(rotated_az, rotated_el, 1.0);
            let near = sofa.find_three_nearest(&tgt);
            let gains = hrtf::calculate_vbap_gains(&tgt, &near, &sofa);
            let (l_fft, r_fft) = hrtf::interpolate_hrtf_frequency_domain(
                &near,
                &gains,
                &sofa,
                self.fft_size,
                self.sample_rate,
                &self.fft_r2c,
                self.near_field_strength,
                tgt.azimuth,
                tgt.elevation,
            );
            filters[ch][..self.freq_size].copy_from_slice(&l_fft[..self.freq_size]);
            filters[ch][self.freq_size..].copy_from_slice(&r_fft[..self.freq_size]);
        }

        hrtf::normalize_hrtf_gains(
            &mut filters,
            &self.lfe_channels,
            self.freq_size,
            self.input_channels,
        );

        let eq = if self.diffuse_field_eq {
            Some(
                filter::compute_diffuse_field_eq(
                    &sofa,
                    self.fft_size,
                    self.sample_rate,
                    &self.fft_r2c,
                )
                .map_err(|e| format!("Diffuse field EQ calculation failed: {}", e))?,
            )
        } else {
            None
        };

        let new_state = Arc::new(BinauralState {
            hrtf_filters_freq: filters,
            diffuse_field_eq_filter: eq,
            _hrtf_data: Some(sofa),
        });
        self.state.store(new_state);
        Ok(())
    }

    fn reset_state(&mut self) {
        self.input_fill = 0;
        self.output_accumulator.fill(0.0);
        self.output_accumulator_fill = 0;
        self.next_add_position = 0;
        self.output_read_position = 0;
        self.latency_filled = 0;
        self.reflection_delay_line.fill(0.0);
        self.reflection_delay_pos = 0;
        // Clear crossfade state on reset
        self.crossfade_prev_state = None;
        self.crossfade_remaining = 0;
        // Reset RTPGHI state so stale phase history is not carried across resets
        if let Some(ref mut rtpghi) = self.rtpghi_left {
            rtpghi.reset();
        }
        if let Some(ref mut rtpghi) = self.rtpghi_right {
            rtpghi.reset();
        }
    }
}

impl Plugin for BinauralDecoderPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Binaural Decoder", "2.0.0", "SotF")
    }
    fn input_channels(&self) -> usize {
        self.input_channels
    }
    fn output_channels(&self) -> usize {
        2
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, val: ParameterValue) -> PluginResult<()> {
        // Parameters not in PARAMS — handle separately
        if id.0 == "crossfade_ms" {
            let v = val
                .as_float()
                .ok_or_else(|| "crossfade_ms must be a float".to_string())?;
            if v.is_finite() && (10.0..=500.0).contains(&v) {
                self.crossfade_ms = v;
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }
        if id.0 == "hrtf_file" {
            let path_str = val
                .as_string()
                .ok_or_else(|| "hrtf_file must be a string".to_string())?
                .to_string();
            let new_path = if path_str.is_empty() {
                None
            } else {
                Some(PathBuf::from(&path_str))
            };
            self.hrtf_path = new_path;

            if let Some(ref p) = self.hrtf_path.clone()
                && self.sample_rate > 0
            {
                let mut sofa = SofaFile::load(p)
                    .map_err(|e| format!("Failed to load HRTF file '{}': {}", path_str, e))?;

                let sofa_rate = sofa.sample_rate.round() as u32;
                if sofa_rate != self.sample_rate {
                    hrtf::resample_sofa(&mut sofa, self.sample_rate)
                        .map_err(|e| format!("HRTF resample failed: {}", e))?;
                }

                let mut filters = vec![
                    vec![Complex::new(0.0, 0.0); self.freq_size * 2];
                    self.input_channels
                ];
                for spk in self.speaker_config.speakers {
                    let ch = spk.channel;
                    if ch >= self.input_channels || self.lfe_channels.contains(&ch) {
                        continue;
                    }
                    let tgt = room::speaker_to_source_position(spk);
                    let near = sofa.find_three_nearest(&tgt);
                    let gains = hrtf::calculate_vbap_gains(&tgt, &near, &sofa);
                    let (l_fft, r_fft) = hrtf::interpolate_hrtf_frequency_domain(
                        &near,
                        &gains,
                        &sofa,
                        self.fft_size,
                        self.sample_rate,
                        &self.fft_r2c,
                        self.near_field_strength,
                        tgt.azimuth,
                        tgt.elevation,
                    );
                    filters[ch][..self.freq_size].copy_from_slice(&l_fft[..self.freq_size]);
                    filters[ch][self.freq_size..].copy_from_slice(&r_fft[..self.freq_size]);
                }
                hrtf::normalize_hrtf_gains(
                    &mut filters,
                    &self.lfe_channels,
                    self.freq_size,
                    self.input_channels,
                );
                let eq = if self.diffuse_field_eq {
                    Some(
                        filter::compute_diffuse_field_eq(
                            &sofa,
                            self.fft_size,
                            self.sample_rate,
                            &self.fft_r2c,
                        )
                        .map_err(|e| {
                            format!("Diffuse field EQ calculation failed: {}", e)
                        })?,
                    )
                } else {
                    None
                };
                let new_state = Arc::new(BinauralState {
                    hrtf_filters_freq: filters,
                    diffuse_field_eq_filter: eq,
                    _hrtf_data: Some(sofa),
                });
                self.state.store(new_state);
            }

            self.rebuild_cached_parameters();
            return Ok(());
        }
        if id.0 == "head_yaw_deg" {
            let v = val
                .as_float()
                .ok_or_else(|| "head_yaw_deg must be a float".to_string())?;
            if v.is_finite() {
                self.head_yaw_deg.set_target(v.clamp(-180.0, 180.0));
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }
        if id.0 == "head_pitch_deg" {
            let v = val
                .as_float()
                .ok_or_else(|| "head_pitch_deg must be a float".to_string())?;
            if v.is_finite() {
                self.head_pitch_deg.set_target(v.clamp(-180.0, 180.0));
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }
        if id.0 == "head_roll_deg" {
            let v = val
                .as_float()
                .ok_or_else(|| "head_roll_deg must be a float".to_string())?;
            if v.is_finite() {
                self.head_roll_deg.set_target(v.clamp(-180.0, 180.0));
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }
        if id.0 == "hrtf_database_dir" {
            let dir = val
                .as_string()
                .ok_or_else(|| "hrtf_database_dir must be a string".to_string())?
                .to_string();
            self.hrtf_database_dir = dir.clone();
            if self.sample_rate > 0 && !dir.is_empty() {
                if let Some(best) =
                    hrtf_database::best_match(std::path::Path::new(&dir), self.head_width_cm, self.ear_height_cm)
                {
                    log::info!(
                        "[BinauralDecoder] hrtf_database_dir scan: best match = {}",
                        best.display()
                    );
                    self.hrtf_path = Some(best.clone());
                    let path_str = best.to_string_lossy().to_string();
                    return self.set_parameter(
                        ParameterId::from("hrtf_file"),
                        ParameterValue::String(path_str),
                    );
                } else {
                    log::warn!(
                        "[BinauralDecoder] hrtf_database_dir '{}' contains no .sofa files",
                        dir
                    );
                }
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }
        if id.0 == "head_width_cm" {
            let v = val
                .as_float()
                .ok_or_else(|| "head_width_cm must be a float".to_string())?;
            if v.is_finite() && (10.0..=25.0).contains(&v) {
                self.head_width_cm = v;
                self.rebuild_cached_parameters();
                if self.sample_rate > 0 && !self.hrtf_database_dir.is_empty() {
                    let dir = self.hrtf_database_dir.clone();
                    if let Some(best) = hrtf_database::best_match(
                        std::path::Path::new(&dir),
                        self.head_width_cm,
                        self.ear_height_cm,
                    ) {
                        let path_str = best.to_string_lossy().to_string();
                        return self.set_parameter(
                            ParameterId::from("hrtf_file"),
                            ParameterValue::String(path_str),
                        );
                    }
                }
            }
            return Ok(());
        }
        if id.0 == "ear_height_cm" {
            let v = val
                .as_float()
                .ok_or_else(|| "ear_height_cm must be a float".to_string())?;
            if v.is_finite() && (4.0..=16.0).contains(&v) {
                self.ear_height_cm = v;
                self.rebuild_cached_parameters();
                if self.sample_rate > 0 && !self.hrtf_database_dir.is_empty() {
                    let dir = self.hrtf_database_dir.clone();
                    if let Some(best) = hrtf_database::best_match(
                        std::path::Path::new(&dir),
                        self.head_width_cm,
                        self.ear_height_cm,
                    ) {
                        let path_str = best.to_string_lossy().to_string();
                        return self.set_parameter(
                            ParameterId::from("hrtf_file"),
                            ParameterValue::String(path_str),
                        );
                    }
                }
            }
            return Ok(());
        }

        let idx = param_bridge::set_parameter(BN, &id, &val, |i, v| self.set_param_value(i, v))?;

        // Side effects based on parameter index
        match idx {
            8 => {
                // late_reverb_rt60
                self.fdn.set_room_params(self.late_reverb_rt60, self.late_reverb_damping, 1.0, self.sample_rate);
            }
            9 => {
                // late_reverb_damping
                self.fdn.set_room_params(self.late_reverb_rt60, self.late_reverb_damping, 1.0, self.sample_rate);
            }
            _ => {}
        }

        self.rebuild_cached_parameters();
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        // Parameters not in PARAMS — handle separately
        if id.0 == "crossfade_ms" {
            return Some(ParameterValue::Float(self.crossfade_ms));
        }
        if id.0 == "hrtf_file" {
            let path_str = self
                .hrtf_path
                .as_ref()
                .and_then(|p| p.to_str())
                .unwrap_or("")
                .to_string();
            return Some(ParameterValue::String(path_str));
        }
        if id.0 == "head_yaw_deg" {
            return Some(ParameterValue::Float(self.head_yaw_deg.target()));
        }
        if id.0 == "head_pitch_deg" {
            return Some(ParameterValue::Float(self.head_pitch_deg.target()));
        }
        if id.0 == "head_roll_deg" {
            return Some(ParameterValue::Float(self.head_roll_deg.target()));
        }
        if id.0 == "hrtf_database_dir" {
            return Some(ParameterValue::String(self.hrtf_database_dir.clone()));
        }
        if id.0 == "head_width_cm" {
            return Some(ParameterValue::Float(self.head_width_cm));
        }
        if id.0 == "ear_height_cm" {
            return Some(ParameterValue::Float(self.ear_height_cm));
        }
        param_bridge::get_parameter(BN, id, |i| self.param_value(i))
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        enable_ftz_daz();
        self.sample_rate = sr;
        self.externalization.set_time(50.0, sr);
        self.head_yaw_deg.set_time(10.0, sr);
        self.head_pitch_deg.set_time(10.0, sr);
        self.head_roll_deg.set_time(10.0, sr);

        // Initialize RTPGHI processors for spectral crossfade mode
        self.rtpghi_left = Some(RtpghiProcessor::new(self.fft_size, self.hop_size));
        self.rtpghi_right = Some(RtpghiProcessor::new(self.fft_size, self.hop_size));
        // Ensure magnitude/phase scratch buffers are correctly sized
        self.crossfade_mag_left.resize(self.freq_size, 0.0);
        self.crossfade_mag_right.resize(self.freq_size, 0.0);
        self.crossfade_phase_left.resize(self.freq_size, 0.0);
        self.crossfade_phase_right.resize(self.freq_size, 0.0);
        let (f, g) = filter::compute_lfe_filter(
            self.fft_size,
            sr,
            self.lfe_crossover,
            self.lfe_distance,
            self.lfe_level,
        );
        self.lfe_lowpass_filter = f;
        self.lfe_gain = g;
        self.cached_reflections.clear();
        let refs = room::calculate_reflections(&self.room_model, self.speaker_config, sr);
        for (ch, cr) in refs.into_iter().enumerate() {
            if !self.lfe_channels.contains(&ch) {
                self.cached_reflections.extend(cr);
            }
        }

        // If a database directory is configured, scan it now and pick the best
        // match.  This overrides any hrtf_path that was set individually.
        if !self.hrtf_database_dir.is_empty() {
            let dir = std::path::Path::new(&self.hrtf_database_dir);
            match hrtf_database::best_match(dir, self.head_width_cm, self.ear_height_cm) {
                Some(best) => {
                    log::info!(
                        "[BinauralDecoder] HRTF database scan: selected '{}'",
                        best.display()
                    );
                    self.hrtf_path = Some(best);
                }
                None => {
                    log::warn!(
                        "[BinauralDecoder] HRTF database dir '{}' contains no .sofa files; \
                         falling back to hrtf_path",
                        self.hrtf_database_dir
                    );
                }
            }
        }

        if let Some(p) = &self.hrtf_path {
            let mut sofa = SofaFile::load(p)?;

            // Resample HRTF IRs if sample rate differs from engine rate
            let sofa_rate = sofa.sample_rate.round() as u32;
            if sofa_rate != sr {
                log::info!(
                    "[BinauralDecoder] HRTF sample rate ({} Hz) differs from engine ({} Hz), resampling",
                    sofa_rate,
                    sr
                );
                hrtf::resample_sofa(&mut sofa, sr)?;
            }

            let mut filters =
                vec![vec![Complex::new(0.0, 0.0); self.freq_size * 2]; self.input_channels];
            for spk in self.speaker_config.speakers {
                let ch = spk.channel;
                if ch >= self.input_channels || self.lfe_channels.contains(&ch) {
                    continue;
                }
                let tgt = room::speaker_to_source_position(spk);
                let near = sofa.find_three_nearest(&tgt);
                let gains = hrtf::calculate_vbap_gains(&tgt, &near, &sofa);
                let (l_fft, r_fft) = hrtf::interpolate_hrtf_frequency_domain(
                    &near,
                    &gains,
                    &sofa,
                    self.fft_size,
                    sr,
                    &self.fft_r2c,
                    self.near_field_strength,
                    tgt.azimuth,
                    tgt.elevation,
                );
                filters[ch][..self.freq_size].copy_from_slice(&l_fft[..self.freq_size]);
                filters[ch][self.freq_size..].copy_from_slice(&r_fft[..self.freq_size]);
            }
            hrtf::normalize_hrtf_gains(
                &mut filters,
                &self.lfe_channels,
                self.freq_size,
                self.input_channels,
            );
            let eq = if self.diffuse_field_eq {
                Some(
                    filter::compute_diffuse_field_eq(&sofa, self.fft_size, sr, &self.fft_r2c)
                        .map_err(|e| format!("Diffuse field EQ calculation failed: {}", e))?,
                )
            } else {
                None
            };
            let new_state = Arc::new(BinauralState {
                hrtf_filters_freq: filters,
                diffuse_field_eq_filter: eq,
                _hrtf_data: Some(sofa),
            });
            self.state.store(new_state.clone());
            self.current_state_snapshot = new_state;
            // Clear any in-progress crossfade on re-initialize
            self.crossfade_prev_state = None;
            self.crossfade_remaining = 0;
        }
        Ok(())
    }
    fn reset(&mut self) {
        self.reset_state();
    }
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let nf = context.num_frames;

        // Advance head-angle smoothers and check whether the angles have changed
        // enough (> 0.5°) to require an HRTF recompute.
        let yaw = self.head_yaw_deg.next_n(nf);
        let pitch = self.head_pitch_deg.next_n(nf);
        let roll = self.head_roll_deg.next_n(nf);
        let angle_changed = (yaw - self.last_hrtf_yaw).abs() > 0.5
            || (pitch - self.last_hrtf_pitch).abs() > 0.5
            || (roll - self.last_hrtf_roll).abs() > 0.5;
        if angle_changed {
            // Recompute is best-effort: log errors but continue with old filters.
            if let Err(e) = self.recompute_hrtf_for_head_angles(yaw, pitch, roll) {
                log::warn!("[BinauralDecoder] Head tracking HRTF recompute failed: {}", e);
                // Still update the cached angles so we don't spam errors every frame.
                self.last_hrtf_yaw = yaw;
                self.last_hrtf_pitch = pitch;
                self.last_hrtf_roll = roll;
            }
        }

        let mut ip = 0;
        let mut op = 0;
        let mask = self.output_accumulator_mask;
        let n = self.fft_size;
        while op < nf {
            if ip < nf {
                let to_copy = (n - self.input_fill).min(nf - ip);
                for ch in 0..self.input_channels {
                    let off = ch * n;
                    for i in 0..to_copy {
                        self.input_buffer[off + self.input_fill + i] =
                            input[(ip + i) * self.input_channels + ch];
                    }
                }
                self.input_fill += to_copy;
                ip += to_copy;
            }
            while self.input_fill >= n {
                self.process_audio_block();
                for ch in 0..self.input_channels {
                    let off = ch * n;
                    self.input_buffer[off..off + n].copy_within(self.hop_size..n, 0);
                }
                self.input_fill = n - self.hop_size;
            }
            let to_drain = self.output_accumulator_fill.min(nf - op);
            if to_drain > 0 {
                let drain_slice = &mut output[op * 2..(op + to_drain) * 2];
                for i in 0..to_drain {
                    let ri = (self.output_read_position + i) & mask;
                    drain_slice[i * 2] = self.output_accumulator[ri * 2];
                    drain_slice[i * 2 + 1] = self.output_accumulator[ri * 2 + 1];
                    self.output_accumulator[ri * 2] = 0.0;
                    self.output_accumulator[ri * 2 + 1] = 0.0;
                }
                self.apply_reflections(drain_slice, to_drain);
                // Phase 4E: Apply late reverb FDN to output
                if self.late_reverb_enabled {
                    let mix = self.late_reverb_mix;
                    for i in 0..to_drain {
                        let l = drain_slice[i * 2];
                        let r = drain_slice[i * 2 + 1];
                        let (rl, rr) = self.fdn.process_stereo(l, r);
                        drain_slice[i * 2] = l * (1.0 - mix) + rl * mix;
                        drain_slice[i * 2 + 1] = r * (1.0 - mix) + rr * mix;
                    }
                }
                self.output_read_position = (self.output_read_position + to_drain) & mask;
                self.output_accumulator_fill -= to_drain;
                op += to_drain;
            } else if ip >= nf {
                for i in op..nf {
                    output[i * 2] = 0.0;
                    output[i * 2 + 1] = 0.0;
                }
                op = nf;
            } else {
                break;
            }
        }
        self.externalization.next_n(nf);
        Ok(op)
    }
    fn latency_samples(&self) -> usize {
        self.fft_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_host::plugin::ProcessContext;
    use sotf_host::sofa::{SofaFile, SourcePosition};

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
        assert_eq!(plugin.hop_size, 1024);
    }

    /// 5.1 surround (6 input channels) should produce binaural stereo (2 output channels).
    #[test]
    fn test_binaural_decoder_6ch_input_produces_2ch_output() {
        let input_channels = 6; // 5.1 surround
        let plugin = BinauralDecoderPlugin::new(
            input_channels,
            2048,
            None,
            true,
            0.5,
            0.0,
            false,
            120.0,
            2.0,
            0.0,
            RoomModel::default(),
        );
        assert_eq!(plugin.input_channels(), 6);
        assert_eq!(
            plugin.output_channels(),
            2,
            "Binaural decoder should always output 2 channels (binaural stereo)"
        );
    }

    /// Create a minimal synthetic SofaFile for testing (no file I/O needed)
    fn make_test_sofa(sample_rate: f32, ir_length: usize, num_measurements: usize) -> SofaFile {
        let mut positions = Vec::with_capacity(num_measurements);
        let mut impulse_responses = Vec::with_capacity(num_measurements * 2 * ir_length);

        for i in 0..num_measurements {
            let az = (i as f32 / num_measurements as f32) * 360.0 - 180.0;
            positions.push(SourcePosition::new(az, 0.0, 1.0));

            // Create a simple delta impulse for each ear
            for _ear in 0..2 {
                let mut ir = vec![0.0f32; ir_length];
                ir[0] = 1.0; // delta impulse
                impulse_responses.extend_from_slice(&ir);
            }
        }

        SofaFile {
            sample_rate,
            num_measurements,
            ir_length,
            positions,
            impulse_responses,
            convention: "SimpleFreeFieldHRIR".to_string(),
            data_sample_rate: Some(sample_rate),
        }
    }

    #[test]
    fn test_resample_sofa_same_rate() {
        let mut sofa = make_test_sofa(48000.0, 128, 4);
        // No-op when rates match
        hrtf::resample_sofa(&mut sofa, 48000).unwrap();
        assert_eq!(sofa.sample_rate, 48000.0);
        assert_eq!(sofa.ir_length, 128);
    }

    #[test]
    fn test_resample_sofa_upsample() {
        // Use a longer IR with a wider pulse to survive resampler latency and filtering
        let original_ir_length = 512;
        let num_measurements = 4;
        let mut sofa = make_test_sofa(44100.0, original_ir_length, num_measurements);

        // Put a wider pulse (first 8 samples = 1.0) so energy survives sinc interpolation
        for m in 0..num_measurements {
            for ear in 0..2 {
                let offset = m * 2 * original_ir_length + ear * original_ir_length;
                for i in 0..8 {
                    sofa.impulse_responses[offset + i] = 1.0;
                }
            }
        }

        hrtf::resample_sofa(&mut sofa, 48000).unwrap();

        // After resampling 44100->48000, IR length should increase proportionally
        let expected_length =
            (original_ir_length as f64 * 48000.0 / 44100.0).ceil() as usize;
        assert_eq!(sofa.sample_rate, 48000.0);
        assert_eq!(sofa.ir_length, expected_length);
        assert_eq!(
            sofa.impulse_responses.len(),
            num_measurements * 2 * expected_length
        );

        // Check that the resampled IR has non-zero energy
        let ir_left = &sofa.impulse_responses[0..expected_length];
        let energy: f32 = ir_left.iter().map(|x| x * x).sum();
        assert!(
            energy > 0.1,
            "Resampled IR energy ({}) should be non-trivial",
            energy
        );
    }

    #[test]
    fn test_resample_sofa_downsample() {
        let original_ir_length = 256;
        let mut sofa = make_test_sofa(96000.0, original_ir_length, 2);
        hrtf::resample_sofa(&mut sofa, 48000).unwrap();

        let expected_length =
            (original_ir_length as f64 * 48000.0 / 96000.0).ceil() as usize;
        assert_eq!(sofa.sample_rate, 48000.0);
        assert_eq!(sofa.ir_length, expected_length);
        assert_eq!(
            sofa.impulse_responses.len(),
            2 * 2 * expected_length
        );
    }

    #[test]
    fn test_crossfade_fields_initialized() {
        let plugin = BinauralDecoderPlugin::new(
            2,
            2048,
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

        assert!(plugin.crossfade_prev_state.is_none());
        assert_eq!(plugin.crossfade_remaining, 0);
        assert_eq!(plugin.crossfade_total, 0);
        assert_eq!(plugin.crossfade_sum_left.len(), plugin.freq_size);
        assert_eq!(plugin.crossfade_sum_right.len(), plugin.freq_size);
    }

    #[test]
    fn test_crossfade_triggers_on_state_change() {
        let mut plugin = BinauralDecoderPlugin::new(
            2,
            1024,
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
        plugin.initialize(44100).unwrap();

        // Simulate state change by storing a new state
        let freq_size = plugin.freq_size;
        let new_state = Arc::new(BinauralState {
            hrtf_filters_freq: vec![
                vec![Complex::new(0.5, 0.0); freq_size * 2];
                plugin.input_channels
            ],
            diffuse_field_eq_filter: None,
            _hrtf_data: None,
        });
        plugin.state.store(new_state);

        // Process a block -- this should detect the state change and start crossfade
        // Fill input buffer to trigger a block
        plugin.input_buffer.fill(0.0);
        plugin.input_fill = plugin.fft_size;
        plugin.process_audio_block();

        // Crossfade should have been initiated and partially consumed
        // After one hop, remaining should be total - hop_size
        assert!(
            plugin.crossfade_total > 0,
            "Crossfade total should be > 0 after state change"
        );
    }

    #[test]
    fn test_crossfade_completes() {
        let mut plugin = BinauralDecoderPlugin::new(
            2,
            1024,
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
        plugin.initialize(44100).unwrap();

        // Trigger a state change
        let freq_size = plugin.freq_size;
        let new_state = Arc::new(BinauralState {
            hrtf_filters_freq: vec![
                vec![Complex::new(0.5, 0.0); freq_size * 2];
                plugin.input_channels
            ],
            diffuse_field_eq_filter: None,
            _hrtf_data: None,
        });
        plugin.state.store(new_state);

        // Process enough blocks to complete the crossfade
        // 50ms at 44100 Hz = 2205 samples; hop_size=256 => ~9 hops
        for _ in 0..20 {
            plugin.input_buffer.fill(0.0);
            plugin.input_fill = plugin.fft_size;
            plugin.process_audio_block();
        }

        // After enough blocks, crossfade should be complete
        assert_eq!(plugin.crossfade_remaining, 0);
        assert!(plugin.crossfade_prev_state.is_none());
    }

    #[test]
    fn test_process_produces_output_without_hrtf() {
        let mut plugin = BinauralDecoderPlugin::new(
            2,
            1024,
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
        plugin.initialize(48000).unwrap();

        let num_frames = 4096;
        let input = vec![0.1f32; num_frames * 2]; // stereo
        let mut output = vec![0.0f32; num_frames * 2];
        let context = ProcessContext {
            num_frames,
            sample_rate: 48000,
        };

        let processed = plugin.process(&input, &mut output, &context).unwrap();
        assert_eq!(processed, num_frames);

        // Should produce some non-zero output (passthrough with default HRTF)
        let has_signal = output.iter().any(|&s| s.abs() > 1e-6);
        assert!(has_signal, "Output should contain signal with default passthrough HRTF");
    }

    #[test]
    fn test_near_field_smoke() {
        // Create plugin with near_field_strength > 0 and verify output is
        // finite and non-zero (basic smoke test for the near-field path).
        let mut plugin = BinauralDecoderPlugin::new(
            2,
            2048,
            None,
            true,
            0.5, // externalization
            0.8, // near_field_strength > 0
            false,
            120.0,
            2.0,
            0.0,
            RoomModel::default(),
        );
        plugin.initialize(48000).unwrap();

        // Process enough audio to fill the STFT pipeline and produce output
        let num_frames = 8192;
        let input: Vec<f32> = (0..num_frames * 2)
            .map(|i| {
                let phase = 2.0 * std::f32::consts::PI * 440.0 * (i / 2) as f32 / 48000.0;
                phase.sin() * 0.3
            })
            .collect();
        let mut output = vec![0.0f32; num_frames * 2];
        let context = ProcessContext {
            num_frames,
            sample_rate: 48000,
        };

        let processed = plugin.process(&input, &mut output, &context).unwrap();
        assert_eq!(processed, num_frames);

        // All outputs should be finite
        assert!(
            output.iter().all(|s| s.is_finite()),
            "All output samples must be finite"
        );

        // At least some output should be non-zero (after STFT latency fills)
        let has_signal = output.iter().any(|&s| s.abs() > 1e-6);
        assert!(
            has_signal,
            "Near-field binaural output should contain signal"
        );
    }

    #[test]
    fn test_reset_clears_crossfade() {
        let mut plugin = BinauralDecoderPlugin::new(
            2,
            1024,
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
        plugin.initialize(44100).unwrap();

        // Trigger a state change
        let freq_size = plugin.freq_size;
        let new_state = Arc::new(BinauralState {
            hrtf_filters_freq: vec![
                vec![Complex::new(0.5, 0.0); freq_size * 2];
                plugin.input_channels
            ],
            diffuse_field_eq_filter: None,
            _hrtf_data: None,
        });
        plugin.state.store(new_state);

        // Process one block to start crossfade
        plugin.input_buffer.fill(0.0);
        plugin.input_fill = plugin.fft_size;
        plugin.process_audio_block();

        // Now reset
        plugin.reset();

        assert!(plugin.crossfade_prev_state.is_none());
        assert_eq!(plugin.crossfade_remaining, 0);
    }

    /// Verify that the `crossfade_ms` parameter can be get/set and that the change
    /// is reflected in the crossfade duration (measured in samples) when a state
    /// transition is detected in `process_audio_block()`.
    #[test]
    fn test_crossfade_ms_parameter_set_get_and_affects_duration() {
        use sotf_host::parameters::ParameterId;

        let mut plugin = BinauralDecoderPlugin::new(
            2,
            1024,
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
        plugin.initialize(44100).unwrap();

        // Default should be 50ms
        let default_val = plugin
            .get_parameter(&ParameterId::from("crossfade_ms"))
            .expect("crossfade_ms parameter must exist");
        assert_eq!(
            default_val,
            ParameterValue::Float(50.0),
            "Default crossfade_ms should be 50.0"
        );

        // Set to 200ms and confirm the stored value changes
        plugin
            .set_parameter(
                ParameterId::from("crossfade_ms"),
                ParameterValue::Float(200.0),
            )
            .expect("set_parameter crossfade_ms should succeed");

        let new_val = plugin
            .get_parameter(&ParameterId::from("crossfade_ms"))
            .expect("crossfade_ms must still exist after set");
        assert_eq!(
            new_val,
            ParameterValue::Float(200.0),
            "crossfade_ms should be updated to 200.0"
        );

        // Verify range rejection: value below minimum should not update the field
        let _ = plugin.set_parameter(
            ParameterId::from("crossfade_ms"),
            ParameterValue::Float(5.0), // below the 10ms minimum -- validate_parameter should reject
        );
        let after_invalid = plugin
            .get_parameter(&ParameterId::from("crossfade_ms"))
            .unwrap();
        // The value must still be 200.0 (the last valid value)
        assert_eq!(
            after_invalid,
            ParameterValue::Float(200.0),
            "crossfade_ms must not be updated to an out-of-range value"
        );

        // Now verify that the duration used in process_audio_block() reflects the
        // new setting. Trigger a state change and measure crossfade_total.
        let freq_size = plugin.freq_size;
        let new_state = Arc::new(BinauralState {
            hrtf_filters_freq: vec![
                vec![Complex::new(0.5, 0.0); freq_size * 2];
                plugin.input_channels
            ],
            diffuse_field_eq_filter: None,
            _hrtf_data: None,
        });
        plugin.state.store(new_state);

        plugin.input_buffer.fill(0.0);
        plugin.input_fill = plugin.fft_size;
        plugin.process_audio_block();

        // At 44100 Hz and 200ms, crossfade_samples = 44100 * 0.200 = 8820.
        // hop_size = 1024/4 = 256.
        // crossfade_hops = ceil(8820 / 256) = 35.
        // crossfade_total = 35 * 256 = 8960.
        let expected_samples = (44100.0_f32 * 0.200) as usize; // 8820
        let hop = plugin.hop_size;
        let expected_hops = expected_samples.div_ceil(hop);
        let expected_total = expected_hops * hop;

        assert_eq!(
            plugin.crossfade_total, expected_total,
            "crossfade_total should reflect the 200ms setting"
        );
    }

    // -------------------------------------------------------------------------
    // Head tracking tests
    // -------------------------------------------------------------------------

    /// Verify head angle parameters can be set and retrieved.
    #[test]
    fn test_head_angle_parameters_set_get() {
        use sotf_host::parameters::ParameterId;

        let mut plugin = BinauralDecoderPlugin::new(
            2,
            1024,
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

        // Default values should all be 0.
        for name in &["head_yaw_deg", "head_pitch_deg", "head_roll_deg"] {
            let v = plugin
                .get_parameter(&ParameterId::from(*name))
                .expect("parameter must exist");
            assert_eq!(
                v,
                ParameterValue::Float(0.0),
                "{} default should be 0.0",
                name
            );
        }

        // Set each to a distinct value and verify.
        plugin
            .set_parameter(
                ParameterId::from("head_yaw_deg"),
                ParameterValue::Float(30.0),
            )
            .unwrap();
        plugin
            .set_parameter(
                ParameterId::from("head_pitch_deg"),
                ParameterValue::Float(-15.0),
            )
            .unwrap();
        plugin
            .set_parameter(
                ParameterId::from("head_roll_deg"),
                ParameterValue::Float(10.0),
            )
            .unwrap();

        assert_eq!(
            plugin.get_parameter(&ParameterId::from("head_yaw_deg")),
            Some(ParameterValue::Float(30.0))
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("head_pitch_deg")),
            Some(ParameterValue::Float(-15.0))
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("head_roll_deg")),
            Some(ParameterValue::Float(10.0))
        );
    }

    /// Verify that head angles appear in the parameter list returned by `parameters()`.
    #[test]
    fn test_head_angle_parameters_listed() {
        let plugin = BinauralDecoderPlugin::new(
            2,
            1024,
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
        let params = plugin.parameters();
        let names: Vec<_> = params.iter().map(|p| p.id.0.as_str()).collect();
        assert!(
            names.contains(&"head_yaw_deg"),
            "head_yaw_deg should be listed"
        );
        assert!(
            names.contains(&"head_pitch_deg"),
            "head_pitch_deg should be listed"
        );
        assert!(
            names.contains(&"head_roll_deg"),
            "head_roll_deg should be listed"
        );
    }

    /// With a synthetic SOFA dataset, verify that yaw=30 produces different HRTF filters
    /// than yaw=0. At yaw=30 the speaker positions are rotated by -30 degrees in azimuth,
    /// so the VBAP lookup will select a different part of the SOFA dataset.
    #[test]
    fn test_yaw_changes_hrtf_filters() {
        const NUM_MEAS: usize = 36;
        const IR_LEN: usize = 64;
        const SAMPLE_RATE: f32 = 44100.0;

        let mut positions = Vec::with_capacity(NUM_MEAS);
        let mut impulse_responses = Vec::with_capacity(NUM_MEAS * 2 * IR_LEN);

        for i in 0..NUM_MEAS {
            let az = -180.0 + (i as f32) * (360.0 / NUM_MEAS as f32);
            positions.push(sotf_host::sofa::SourcePosition::new(az, 0.0, 1.0));

            // Left-ear IR: amplitude encodes the azimuth index so filters differ per position.
            let mut ir_l = vec![0.0f32; IR_LEN];
            ir_l[0] = 1.0 + i as f32 * 0.01;
            let ir_r = vec![0.0f32; IR_LEN];

            impulse_responses.extend_from_slice(&ir_l);
            impulse_responses.extend_from_slice(&ir_r);
        }

        let sofa = sotf_host::sofa::SofaFile {
            sample_rate: SAMPLE_RATE,
            num_measurements: NUM_MEAS,
            ir_length: IR_LEN,
            positions,
            impulse_responses,
            convention: "SimpleFreeFieldHRIR".to_string(),
            data_sample_rate: Some(SAMPLE_RATE),
        };

        // Compute the left-ear HRTF frequency spectrum for the L stereo speaker
        // (az=+30, el=0) with a given head yaw applied via inverse rotation.
        let compute_left_filter = |yaw: f32| -> Vec<Complex<f32>> {
            let (rot_az, rot_el) =
                BinauralDecoderPlugin::rotate_speaker_position(30.0, 0.0, yaw, 0.0, 0.0);
            let tgt = sotf_host::sofa::SourcePosition::new(rot_az, rot_el, 1.0);
            let near = sofa.find_three_nearest(&tgt);
            let gains = hrtf::calculate_vbap_gains(&tgt, &near, &sofa);

            let fft_size = 512usize;
            let mut planner = realfft::RealFftPlanner::<f32>::new();
            let fft_r2c = planner.plan_fft_forward(fft_size);

            let (l_fft, _) = hrtf::interpolate_hrtf_frequency_domain(
                &near,
                &gains,
                &sofa,
                fft_size,
                44100,
                &fft_r2c,
                0.0,
                tgt.azimuth,
                tgt.elevation,
            );
            l_fft
        };

        let filters_yaw0 = compute_left_filter(0.0);
        let filters_yaw30 = compute_left_filter(30.0);

        let max_diff = filters_yaw0
            .iter()
            .zip(filters_yaw30.iter())
            .map(|(a, b)| (a - b).norm())
            .fold(0.0f32, f32::max);

        assert!(
            max_diff > 1e-4,
            "HRTF filters with yaw=30 must differ from yaw=0 (max_diff={})",
            max_diff
        );
    }

    /// rotate_speaker_position must be identity when all angles are 0.
    #[test]
    fn test_rotate_speaker_position_identity() {
        let (az, el) =
            BinauralDecoderPlugin::rotate_speaker_position(45.0, 20.0, 0.0, 0.0, 0.0);
        assert!(
            (az - 45.0).abs() < 1e-3,
            "azimuth should be unchanged: {}",
            az
        );
        assert!(
            (el - 20.0).abs() < 1e-3,
            "elevation should be unchanged: {}",
            el
        );
    }

    /// For yaw-only rotation the rotated speaker azimuth should shift by -yaw.
    #[test]
    fn test_rotate_speaker_position_yaw_only() {
        // Speaker at az=30, el=0. Head yaw=30 => inverse shift of -30 => az near 0.
        let (az, el) =
            BinauralDecoderPlugin::rotate_speaker_position(30.0, 0.0, 30.0, 0.0, 0.0);
        assert!(
            (az - 0.0).abs() < 1e-3,
            "azimuth after yaw should be near 0, got {}",
            az
        );
        assert!((el - 0.0).abs() < 1e-3, "elevation should stay 0, got {}", el);
    }

    /// Processing with non-zero head yaw must not produce NaN or Inf output.
    #[test]
    fn test_head_yaw_produces_finite_output() {
        use sotf_host::parameters::ParameterId;

        let mut plugin = BinauralDecoderPlugin::new(
            2,
            1024,
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
        plugin.initialize(44100).unwrap();

        // Set yaw to 30. Without a SOFA file the default filters remain in place;
        // the smoother must advance without causing NaN/Inf.
        plugin
            .set_parameter(
                ParameterId::from("head_yaw_deg"),
                ParameterValue::Float(30.0),
            )
            .unwrap();

        let num_frames = 4096;
        let input: Vec<f32> = (0..num_frames * 2)
            .map(|i| (i as f32 * 0.01).sin() * 0.5)
            .collect();
        let mut output = vec![0.0f32; num_frames * 2];
        let context = ProcessContext {
            num_frames,
            sample_rate: 44100,
        };

        let processed = plugin.process(&input, &mut output, &context).unwrap();
        assert_eq!(processed, num_frames);
        assert!(
            output.iter().all(|s| s.is_finite()),
            "All output samples must be finite with non-zero yaw"
        );
    }

    /// Verify that spectral crossfade mode (magnitude interpolation + RTPGHI)
    /// produces a smoother magnitude spectrum than linear complex blending
    /// during an HRTF transition.
    ///
    /// The test triggers a crossfade between two different HRTF filter sets and
    /// processes audio through both modes. The spectral mode should produce a
    /// magnitude spectrum without the comb-filter dips that linear mode creates
    /// when old and new HRTFs have different phase responses.
    #[test]
    fn test_spectral_crossfade_no_tonal_shift() {
        let fft_size = 1024;
        let freq_size = fft_size / 2 + 1;
        let sample_rate = 44100u32;

        // Create two plugins: one linear (mode 0), one spectral (mode 1)
        let make_plugin = |mode: usize| {
            let mut p = BinauralDecoderPlugin::new(
                2,
                fft_size,
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
            p.crossfade_mode_index = mode;
            p.initialize(sample_rate).unwrap();
            p
        };

        let mut linear_plugin = make_plugin(0);
        let mut spectral_plugin = make_plugin(1);

        // Create two distinct HRTF states to trigger a crossfade.
        // State A: passthrough-like (all 1.0)
        // State B: different phase response (rotated complex values)
        let make_state = |phase_shift: f32, channels: usize| {
            let mut filters = vec![vec![Complex::new(0.0, 0.0); freq_size * 2]; channels];
            for ch in 0..channels {
                for k in 0..freq_size {
                    // Different phase per frequency bin for state B, creating phase
                    // differences that would cause comb-filtering in linear blend.
                    let angle = phase_shift * (k as f32 / freq_size as f32) * std::f32::consts::PI;
                    let (sin_a, cos_a) = angle.sin_cos();
                    let val = Complex::new(cos_a * 0.7, sin_a * 0.7);
                    filters[ch][k] = val;               // left ear
                    filters[ch][freq_size + k] = val;   // right ear
                }
            }
            Arc::new(BinauralState {
                hrtf_filters_freq: filters,
                diffuse_field_eq_filter: None,
                _hrtf_data: None,
            })
        };

        // Start with state A
        let state_a = make_state(0.0, 2);
        linear_plugin.state.store(state_a.clone());
        spectral_plugin.state.store(state_a.clone());
        // Force state snapshot update
        linear_plugin.current_state_snapshot = linear_plugin.state.load_full();
        spectral_plugin.current_state_snapshot = spectral_plugin.state.load_full();

        // Process a few frames to fill pipeline
        let num_frames = fft_size * 4;
        let input: Vec<f32> = (0..num_frames * 2)
            .map(|i| {
                let phase = 2.0 * std::f32::consts::PI * 1000.0 * (i / 2) as f32 / sample_rate as f32;
                phase.sin() * 0.5
            })
            .collect();
        let mut output_warmup = vec![0.0f32; num_frames * 2];
        let ctx = ProcessContext {
            num_frames,
            sample_rate,
        };
        linear_plugin.process(&input, &mut output_warmup, &ctx).unwrap();
        spectral_plugin.process(&input, &mut output_warmup, &ctx).unwrap();

        // Now switch to state B -- this triggers crossfade
        let state_b = make_state(4.0, 2);
        linear_plugin.state.store(state_b.clone());
        spectral_plugin.state.store(state_b);

        // Process during the crossfade
        let mut output_linear = vec![0.0f32; num_frames * 2];
        let mut output_spectral = vec![0.0f32; num_frames * 2];
        linear_plugin.process(&input, &mut output_linear, &ctx).unwrap();
        spectral_plugin.process(&input, &mut output_spectral, &ctx).unwrap();

        // Both outputs must be finite
        assert!(
            output_linear.iter().all(|s| s.is_finite()),
            "Linear crossfade output must be finite"
        );
        assert!(
            output_spectral.iter().all(|s| s.is_finite()),
            "Spectral crossfade output must be finite"
        );

        // Both outputs should have signal (not silence)
        let linear_energy: f32 = output_linear.iter().map(|s| s * s).sum();
        let spectral_energy: f32 = output_spectral.iter().map(|s| s * s).sum();
        assert!(
            linear_energy > 1e-6,
            "Linear crossfade should produce signal, energy={}",
            linear_energy
        );
        assert!(
            spectral_energy > 1e-6,
            "Spectral crossfade should produce signal, energy={}",
            spectral_energy
        );

        // Compute magnitude spectra of a chunk during crossfade to verify
        // spectral mode has smoother magnitude (fewer comb-filter dips).
        // Take a section from the middle of the output (skip latency).
        let analysis_start = fft_size; // skip initial latency
        if analysis_start + fft_size <= num_frames {
            let compute_spectrum = |output: &[f32]| -> Vec<f32> {
                let mut mags = vec![0.0f32; freq_size];
                // Simple DFT magnitude for left channel
                for k in 0..freq_size {
                    let mut re = 0.0f64;
                    let mut im = 0.0f64;
                    for n in 0..fft_size {
                        let sample = output[(analysis_start + n) * 2] as f64;
                        let angle = -2.0 * std::f64::consts::PI * k as f64 * n as f64 / fft_size as f64;
                        re += sample * angle.cos();
                        im += sample * angle.sin();
                    }
                    mags[k] = (re * re + im * im).sqrt() as f32;
                }
                mags
            };

            let linear_mags = compute_spectrum(&output_linear);
            let spectral_mags = compute_spectrum(&output_spectral);

            // Count "deep nulls" in the magnitude spectrum (bins where magnitude
            // drops to less than 10% of the peak). Linear complex blending with
            // phase-mismatched HRTFs creates many such nulls. Spectral mode should
            // create fewer.
            let count_nulls = |mags: &[f32]| -> usize {
                let peak = mags.iter().copied().fold(0.0f32, f32::max);
                if peak < 1e-10 {
                    return 0;
                }
                let threshold = peak * 0.1;
                // Only count nulls in the first half of the spectrum (audible range)
                mags[1..freq_size / 2]
                    .iter()
                    .filter(|&&m| m < threshold && m > 0.0)
                    .count()
            };

            let linear_nulls = count_nulls(&linear_mags);
            let spectral_nulls = count_nulls(&spectral_mags);

            // Spectral mode should not have MORE nulls than linear mode.
            // (It should have fewer or equal.)
            assert!(
                spectral_nulls <= linear_nulls + 3, // small tolerance for edge effects
                "Spectral crossfade should not produce more comb-filter nulls than linear: \
                 spectral={}, linear={}",
                spectral_nulls,
                linear_nulls
            );
        }
    }

    /// Verify that the crossfade_mode parameter can be set and retrieved.
    #[test]
    fn test_crossfade_mode_parameter_set_get() {
        let mut plugin = BinauralDecoderPlugin::new(
            2,
            1024,
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

        // Default should be 0 (Linear)
        let val = plugin
            .get_parameter(&ParameterId::from("crossfade_mode"))
            .expect("crossfade_mode must exist");
        assert_eq!(val, ParameterValue::Int(0));

        // Set to Spectral (1)
        plugin
            .set_parameter(
                ParameterId::from("crossfade_mode"),
                ParameterValue::Int(1),
            )
            .unwrap();
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("crossfade_mode")),
            Some(ParameterValue::Int(1))
        );
        assert_eq!(plugin.crossfade_mode_index, 1);

        // Out-of-range value (2) is clamped to max valid index (1) by param_bridge
        plugin
            .set_parameter(
                ParameterId::from("crossfade_mode"),
                ParameterValue::Int(2),
            )
            .unwrap();
        assert_eq!(plugin.crossfade_mode_index, 1, "Out-of-range mode should be clamped to max");

        // Set back to Linear (0)
        plugin
            .set_parameter(
                ParameterId::from("crossfade_mode"),
                ParameterValue::Int(0),
            )
            .unwrap();
        assert_eq!(plugin.crossfade_mode_index, 0);
    }
}
