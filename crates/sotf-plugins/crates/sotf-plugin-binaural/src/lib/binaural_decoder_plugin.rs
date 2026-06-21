pub use super::config::BinauralDecoderParams;
pub use super::error::BinauralError;
pub use super::room::{Reflection, RoomModel};
use super::types::BinauralState;
use crate::params::PARAMS as BN;
use arc_swap::ArcSwap;
use math_audio_dsp::rtpghi::RtpghiProcessor;
use plugins_spatial::validate_interleaved_io;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use sotf_host::param_bridge;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{complex_mul_add_simd, enable_ftz_daz, window_mul_simd};
use sotf_host::smoothing::Smoother;
use sotf_host::sofa::SofaFile;
use sotf_host::speaker_config::{SpeakerConfig, get_speaker_config_by_channels};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread::JoinHandle;

#[derive(Clone)]
pub(super) struct BinauralConfig {
    pub(super) input_channels: usize,
    pub(super) fft_size: usize,
    pub(super) hop_size: usize,
    pub(super) sample_rate: u32,
    pub(super) hrtf_path: Option<PathBuf>,
    pub(super) speaker_config: &'static SpeakerConfig,
    pub(super) freq_size: usize,
    pub(super) srir_file: Option<PathBuf>,
    pub(super) room_model: RoomModel,
    pub(super) hrtf_database_dir: String,
    pub(super) head_width_cm: f32,
    pub(super) ear_height_cm: f32,
    pub(super) diffuse_field_eq: bool,
    pub(super) lfe_crossover: f32,
    pub(super) lfe_distance: f32,
    pub(super) lfe_level: f32,
    pub(super) near_field_strength: f32,
    pub(super) crossfade_ms: f32,
    pub(super) crossfade_mode_index: usize,
    pub(super) late_reverb_enabled: bool,
    pub(super) late_reverb_mix: f32,
    pub(super) late_reverb_rt60: f32,
    pub(super) late_reverb_damping: f32,
    pub(super) cached_parameters: Vec<Parameter>,
}

pub(super) struct BinauralFft {
    pub(super) fft_r2c: Arc<dyn RealToComplex<f32>>,
    pub(super) fft_c2r: Arc<dyn ComplexToReal<f32>>,
    pub(super) analysis_window: Vec<f32>,
}

pub(super) struct BinauralAnalysis {
    pub(super) temp_freq_buffer: Vec<Complex<f32>>,
    pub(super) temp_fft_scratch: Vec<Complex<f32>>,
    pub(super) sum_left: Vec<Complex<f32>>,
    pub(super) sum_right: Vec<Complex<f32>>,
    pub(super) lfe_freq: Vec<Complex<f32>>,
    pub(super) ifft_output_buf: Vec<f32>,
}

pub(super) struct BinauralCoefficients {
    pub(super) lfe_lowpass_filter: Vec<Complex<f32>>,
    pub(super) lfe_gain: f32,
    pub(super) lfe_channels: Vec<usize>,
    pub(super) main_channels: Vec<usize>,
}

pub(super) struct BinauralInput {
    pub(super) input_buffer: Vec<f32>,
    pub(super) input_fill: usize,
}

pub(super) struct BinauralOutput {
    pub(super) output_accumulator: Vec<f32>,
    pub(super) output_accumulator_mask: usize,
    pub(super) output_accumulator_fill: usize,
    pub(super) next_add_position: usize,
    pub(super) output_read_position: usize,
    pub(super) latency_filled: usize,
    pub(super) output_scale: f32,
}

pub(super) struct BinauralRoom {
    pub(super) reflection_delay_line: Vec<f32>,
    pub(super) reflection_delay_pos: usize,
    pub(super) reflection_delay_mask: usize,
    pub(super) cached_reflections: Vec<Reflection>,
    pub(super) fdn: math_audio_dsp::fdn::Fdn,
}

pub(super) struct BinauralCrossfade {
    pub(super) current_state_snapshot: Arc<BinauralState>,
    pub(super) crossfade_prev_state: Option<Arc<BinauralState>>,
    pub(super) crossfade_remaining: usize,
    pub(super) crossfade_total: usize,
    pub(super) crossfade_sum_left: Vec<Complex<f32>>,
    pub(super) crossfade_sum_right: Vec<Complex<f32>>,
    pub(super) rtpghi_left: Option<RtpghiProcessor>,
    pub(super) rtpghi_right: Option<RtpghiProcessor>,
    pub(super) crossfade_mag_left: Vec<f32>,
    pub(super) crossfade_mag_right: Vec<f32>,
    pub(super) crossfade_phase_left: Vec<f32>,
    pub(super) crossfade_phase_right: Vec<f32>,
}

pub(super) struct BinauralSmoothing {
    pub(super) externalization: Smoother,
    pub(super) head_yaw_deg: Smoother,
    pub(super) head_pitch_deg: Smoother,
    pub(super) head_roll_deg: Smoother,
    pub(super) last_hrtf_yaw: f32,
    pub(super) last_hrtf_pitch: f32,
    pub(super) last_hrtf_roll: f32,
}

pub struct BinauralDecoderPlugin {
    pub(super) state: Arc<ArcSwap<BinauralState>>,
    pub(super) config: BinauralConfig,
    pub(super) fft: BinauralFft,
    pub(super) analysis: BinauralAnalysis,
    pub(super) coefficients: BinauralCoefficients,
    pub(super) input: BinauralInput,
    pub(super) output: BinauralOutput,
    pub(super) room: BinauralRoom,
    pub(super) crossfade: BinauralCrossfade,
    pub(super) smoothing: BinauralSmoothing,
    /// Channel used to request head-angle HRTF recomputations from the
    /// background thread. Capacity 1; stale requests are dropped when the
    /// thread is still busy.
    pub(super) hrtf_update_tx: Option<SyncSender<(f32, f32, f32)>>,
    /// Background HRTF update thread handle.
    pub(super) hrtf_update_thread: Option<JoinHandle<()>>,
}

impl BinauralDecoderPlugin {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        input_channels: usize,
        fft_size: usize,
        hrtf_path: Option<PathBuf>,
        _enable_optimization: bool,
        externalization: f32,
        near_field_strength: f32,
        diffuse_field_eq: bool,
        lfe_crossover: f32,
        lfe_distance: f32,
        lfe_level: f32,
        room_model: RoomModel,
    ) -> Self {
        assert!(
            fft_size.is_power_of_two(),
            "{}",
            BinauralError::InvalidFftSize(fft_size)
        );

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
        super::hrtf::normalize_hrtf_gains(
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
            state: Arc::new(ArcSwap::from(initial_state.clone())),
            config: BinauralConfig {
                input_channels,
                fft_size,
                hop_size,
                sample_rate: sr,
                hrtf_path,
                speaker_config,
                freq_size,
                srir_file: None,
                room_model,
                hrtf_database_dir: String::new(),
                head_width_cm: 15.0,
                ear_height_cm: 10.0,
                diffuse_field_eq,
                lfe_crossover,
                lfe_distance,
                lfe_level,
                near_field_strength,
                crossfade_ms: 50.0,
                crossfade_mode_index: 0,
                late_reverb_enabled: false,
                late_reverb_mix: 0.3,
                late_reverb_rt60: 1.0,
                late_reverb_damping: 0.3,
                cached_parameters: Vec::new(),
            },
            fft: BinauralFft {
                fft_r2c,
                fft_c2r,
                analysis_window,
            },
            analysis: BinauralAnalysis {
                temp_freq_buffer: vec![Complex::new(0.0, 0.0); freq_size],
                temp_fft_scratch: vec![Complex::new(0.0, 0.0); scratch_len],
                sum_left: vec![Complex::new(0.0, 0.0); freq_size],
                sum_right: vec![Complex::new(0.0, 0.0); freq_size],
                lfe_freq: vec![Complex::new(0.0, 0.0); freq_size],
                ifft_output_buf: vec![0.0; fft_size],
            },
            coefficients: BinauralCoefficients {
                lfe_lowpass_filter: vec![Complex::new(1.0, 0.0); freq_size],
                lfe_gain: 1.0,
                lfe_channels,
                main_channels,
            },
            input: BinauralInput {
                input_buffer: vec![0.0; fft_size * input_channels],
                input_fill: 0,
            },
            output: BinauralOutput {
                output_accumulator: vec![0.0; fft_size * 4 * 2],
                output_accumulator_mask: (fft_size * 4) - 1,
                output_accumulator_fill: 0,
                next_add_position: 0,
                output_read_position: 0,
                latency_filled: 0,
                output_scale,
            },
            room: BinauralRoom {
                reflection_delay_line: vec![0.0; delay_size * 2],
                reflection_delay_pos: 0,
                reflection_delay_mask: delay_size - 1,
                cached_reflections: Vec::new(),
                fdn: math_audio_dsp::fdn::Fdn::new(8, sr),
            },
            crossfade: BinauralCrossfade {
                current_state_snapshot: initial_state,
                crossfade_prev_state: None,
                crossfade_remaining: 0,
                crossfade_total: 0,
                crossfade_sum_left: vec![Complex::new(0.0, 0.0); freq_size],
                crossfade_sum_right: vec![Complex::new(0.0, 0.0); freq_size],
                rtpghi_left: None,
                rtpghi_right: None,
                crossfade_mag_left: vec![0.0; freq_size],
                crossfade_mag_right: vec![0.0; freq_size],
                crossfade_phase_left: vec![0.0; freq_size],
                crossfade_phase_right: vec![0.0; freq_size],
            },
            smoothing: BinauralSmoothing {
                externalization: Smoother::new(externalization, 50.0, sr),
                head_yaw_deg: Smoother::new(0.0, 10.0, sr),
                head_pitch_deg: Smoother::new(0.0, 10.0, sr),
                head_roll_deg: Smoother::new(0.0, 10.0, sr),
                last_hrtf_yaw: 0.0,
                last_hrtf_pitch: 0.0,
                last_hrtf_roll: 0.0,
            },
            hrtf_update_tx: None,
            hrtf_update_thread: None,
        };
        p.rebuild_cached_parameters();
        p
    }

    /// Get the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    pub(super) fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => None, // sofa_file (FilePath — handled separately)
            1 => Some(self.config.input_channels as f64),
            2 => Some(self.smoothing.externalization.target() as f64),
            3 => Some(self.config.near_field_strength as f64),
            4 => Some(self.config.crossfade_mode_index as f64),
            5 => Some(if self.config.late_reverb_enabled {
                1.0
            } else {
                0.0
            }),
            6 => Some(self.config.late_reverb_mix as f64),
            7 => Some(self.config.late_reverb_rt60 as f64),
            8 => Some(self.config.late_reverb_damping as f64),
            _ => None,
        }
    }

    /// Set the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    pub(super) fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => {} // sofa_file (FilePath — handled separately)
            1 => {} // input_channels (construction-only, requires buffer rebuild)
            2 => self.smoothing.externalization.set_target(value as f32),
            3 => self.config.near_field_strength = value as f32,
            4 => self.config.crossfade_mode_index = value as usize,
            5 => self.config.late_reverb_enabled = value > 0.5,
            6 => self.config.late_reverb_mix = value as f32,
            7 => self.config.late_reverb_rt60 = value as f32,
            8 => self.config.late_reverb_damping = value as f32,
            _ => {}
        }
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        self.config.cached_parameters = param_bridge::build_parameters(BN, |i| self.param_value(i));
        // Append parameters not in PARAMS
        let hrtf_path_str = self
            .config
            .hrtf_path
            .as_ref()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_string();
        self.config.cached_parameters.push(Parameter::new_float(
            "crossfade_ms",
            "Crossfade (ms)",
            self.config.crossfade_ms,
            10.0,
            500.0,
        ));
        self.config.cached_parameters.push(Parameter::new_string(
            "hrtf_file",
            "HRTF File",
            hrtf_path_str,
        ));
        self.config.cached_parameters.push(Parameter::new_float(
            "head_yaw_deg",
            "Head Yaw (deg)",
            self.smoothing.head_yaw_deg.target(),
            -180.0,
            180.0,
        ));
        self.config.cached_parameters.push(Parameter::new_float(
            "head_pitch_deg",
            "Head Pitch (deg)",
            self.smoothing.head_pitch_deg.target(),
            -180.0,
            180.0,
        ));
        self.config.cached_parameters.push(Parameter::new_float(
            "head_roll_deg",
            "Head Roll (deg)",
            self.smoothing.head_roll_deg.target(),
            -180.0,
            180.0,
        ));
        self.config.cached_parameters.push(Parameter::new_string(
            "hrtf_database_dir",
            "HRTF Database Dir",
            self.config.hrtf_database_dir.clone(),
        ));
        self.config.cached_parameters.push(Parameter::new_float(
            "head_width_cm",
            "Head Width (cm)",
            self.config.head_width_cm,
            10.0,
            25.0,
        ));
        self.config.cached_parameters.push(Parameter::new_float(
            "ear_height_cm",
            "Ear Height (cm)",
            self.config.ear_height_cm,
            4.0,
            16.0,
        ));
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
        plugin.config.hrtf_database_dir = params.hrtf_database_dir;
        plugin.config.head_width_cm = params.head_width_cm;
        plugin.config.ear_height_cm = params.ear_height_cm;
        if !params.srir_file.is_empty() {
            plugin.config.srir_file = Some(PathBuf::from(params.srir_file));
        }
        plugin.rebuild_cached_parameters();
        plugin
    }

    pub(super) fn process_audio_block(&mut self) {
        // Detect state changes for crossfade.
        // Use load() (borrow guard) instead of load_full() (Arc clone) to avoid
        // an atomic refcount increment on every audio block call.
        let new_state = self.state.load();
        if !Arc::ptr_eq(&new_state, &self.crossfade.current_state_snapshot) {
            // State changed -- start crossfade from old to new
            // Crossfade duration in samples, rounded up to hop_size boundary
            let crossfade_samples =
                (self.config.sample_rate as f32 * self.config.crossfade_ms * 0.001) as usize;
            let crossfade_hops = crossfade_samples.div_ceil(self.config.hop_size);
            let total = crossfade_hops * self.config.hop_size;

            log::debug!(
                "[BinauralDecoder] HRTF state changed, crossfading over {} samples ({} hops)",
                total,
                crossfade_hops
            );

            self.crossfade.crossfade_prev_state =
                Some(self.crossfade.current_state_snapshot.clone());
            self.crossfade.crossfade_total = total;
            self.crossfade.crossfade_remaining = total;
            // Guard derefs to Arc<BinauralState>; clone the Arc to store as snapshot.
            self.crossfade.current_state_snapshot = Arc::clone(&new_state);

            // Reset RTPGHI state when starting a new crossfade so stale phase
            // history from a previous crossfade does not contaminate this one.
            if self.config.crossfade_mode_index == 1 {
                if let Some(ref mut rtpghi) = self.crossfade.rtpghi_left {
                    rtpghi.reset();
                }
                if let Some(ref mut rtpghi) = self.crossfade.rtpghi_right {
                    rtpghi.reset();
                }
            }
        }

        let state = &new_state;
        let filters = &state.hrtf_filters_freq;
        let df_eq = &state.diffuse_field_eq_filter;
        let n = self.config.fft_size;
        let freq_size = self.config.freq_size;
        let mask = self.output.output_accumulator_mask;
        let scale = self.output.output_scale;

        // Check if we need crossfade blending
        let crossfading =
            self.crossfade.crossfade_remaining > 0 && self.crossfade.crossfade_prev_state.is_some();

        if crossfading {
            let prev = self
                .crossfade
                .crossfade_prev_state
                .as_ref()
                .unwrap()
                .clone();
            let prev_filters = &prev.hrtf_filters_freq;
            let prev_df_eq = &prev.diffuse_field_eq_filter;

            self.analysis.sum_left.fill(Complex::new(0.0, 0.0));
            self.analysis.sum_right.fill(Complex::new(0.0, 0.0));
            self.analysis.lfe_freq.fill(Complex::new(0.0, 0.0));

            // We need the FFT of each channel's input. Since both old and new use the same input,
            // we compute the FFT once per channel and apply both filter sets.
            // But the FFT output is stored in temp_freq_buffer, so we need to process per-channel.

            // Old state accumulators
            self.crossfade
                .crossfade_sum_left
                .fill(Complex::new(0.0, 0.0));
            self.crossfade
                .crossfade_sum_right
                .fill(Complex::new(0.0, 0.0));

            for &ch in &self.coefficients.main_channels {
                let ch_offset = ch * n;
                window_mul_simd(
                    &mut self.analysis.ifft_output_buf,
                    &self.input.input_buffer[ch_offset..ch_offset + n],
                    &self.fft.analysis_window,
                );

                self.fft
                    .fft_r2c
                    .process_with_scratch(
                        &mut self.analysis.ifft_output_buf,
                        &mut self.analysis.temp_freq_buffer,
                        &mut self.analysis.temp_fft_scratch,
                    )
                    .unwrap_or_else(|e| log::error!("[BinauralDecoder] FFT error: {e}"));

                // New filters
                let hrtf_new = &filters[ch];
                complex_mul_add_simd(
                    &mut self.analysis.sum_left,
                    &self.analysis.temp_freq_buffer,
                    &hrtf_new[0..freq_size],
                );
                complex_mul_add_simd(
                    &mut self.analysis.sum_right,
                    &self.analysis.temp_freq_buffer,
                    &hrtf_new[freq_size..],
                );

                // Old filters
                let hrtf_old = &prev_filters[ch];
                complex_mul_add_simd(
                    &mut self.crossfade.crossfade_sum_left,
                    &self.analysis.temp_freq_buffer,
                    &hrtf_old[0..freq_size],
                );
                complex_mul_add_simd(
                    &mut self.crossfade.crossfade_sum_right,
                    &self.analysis.temp_freq_buffer,
                    &hrtf_old[freq_size..],
                );
            }

            // Apply diffuse field EQ to new output
            if let Some(eq) = df_eq {
                for (k, (sl, sr)) in self
                    .analysis
                    .sum_left
                    .iter_mut()
                    .zip(self.analysis.sum_right.iter_mut())
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
                    .crossfade
                    .crossfade_sum_left
                    .iter_mut()
                    .zip(self.crossfade.crossfade_sum_right.iter_mut())
                    .enumerate()
                    .take(freq_size)
                {
                    *sl *= eq[0][k];
                    *sr *= eq[1][k];
                }
            }

            // Blend old and new in frequency domain using crossfade gain
            // Linear crossfade: new_gain goes from 0.0 to 1.0 over the crossfade period
            let new_gain = if self.crossfade.crossfade_total > 0 {
                1.0 - (self.crossfade.crossfade_remaining as f32
                    / self.crossfade.crossfade_total as f32)
            } else {
                1.0
            };
            let old_gain = 1.0 - new_gain;

            let use_spectral = self.config.crossfade_mode_index == 1
                && self.crossfade.rtpghi_left.is_some()
                && self.crossfade.rtpghi_right.is_some();

            if use_spectral {
                // Spectral mode: magnitude interpolation + RTPGHI phase reconstruction
                // This avoids comb-filter artifacts from complex-domain blending.
                for k in 0..freq_size {
                    let mag_new_l = (self.analysis.sum_left[k].re * self.analysis.sum_left[k].re
                        + self.analysis.sum_left[k].im * self.analysis.sum_left[k].im)
                        .sqrt();
                    let mag_old_l = (self.crossfade.crossfade_sum_left[k].re
                        * self.crossfade.crossfade_sum_left[k].re
                        + self.crossfade.crossfade_sum_left[k].im
                            * self.crossfade.crossfade_sum_left[k].im)
                        .sqrt();
                    self.crossfade.crossfade_mag_left[k] =
                        mag_old_l * old_gain + mag_new_l * new_gain;

                    let mag_new_r = (self.analysis.sum_right[k].re * self.analysis.sum_right[k].re
                        + self.analysis.sum_right[k].im * self.analysis.sum_right[k].im)
                        .sqrt();
                    let mag_old_r = (self.crossfade.crossfade_sum_right[k].re
                        * self.crossfade.crossfade_sum_right[k].re
                        + self.crossfade.crossfade_sum_right[k].im
                            * self.crossfade.crossfade_sum_right[k].im)
                        .sqrt();
                    self.crossfade.crossfade_mag_right[k] =
                        mag_old_r * old_gain + mag_new_r * new_gain;
                }

                // RTPGHI phase reconstruction from interpolated magnitudes
                // Safety: use_spectral already checked is_some() above.
                let rtpghi_l = self.crossfade.rtpghi_left.as_mut().expect("checked above");
                rtpghi_l.process_frame_into(
                    &self.crossfade.crossfade_mag_left[..freq_size],
                    &mut self.crossfade.crossfade_phase_left[..freq_size],
                );
                let rtpghi_r = self.crossfade.rtpghi_right.as_mut().expect("checked above");
                rtpghi_r.process_frame_into(
                    &self.crossfade.crossfade_mag_right[..freq_size],
                    &mut self.crossfade.crossfade_phase_right[..freq_size],
                );

                // Reconstruct complex spectrum from blended magnitude + reconstructed phase
                for k in 0..freq_size {
                    let (sin_l, cos_l) = (self.crossfade.crossfade_phase_left[k] as f64).sin_cos();
                    self.analysis.sum_left[k] = Complex::new(
                        self.crossfade.crossfade_mag_left[k] * cos_l as f32,
                        self.crossfade.crossfade_mag_left[k] * sin_l as f32,
                    );

                    let (sin_r, cos_r) = (self.crossfade.crossfade_phase_right[k] as f64).sin_cos();
                    self.analysis.sum_right[k] = Complex::new(
                        self.crossfade.crossfade_mag_right[k] * cos_r as f32,
                        self.crossfade.crossfade_mag_right[k] * sin_r as f32,
                    );
                }
            } else {
                // Linear mode: simple complex-domain blend (original behavior)
                for k in 0..freq_size {
                    self.analysis.sum_left[k] = self.analysis.sum_left[k] * new_gain
                        + self.crossfade.crossfade_sum_left[k] * old_gain;
                    self.analysis.sum_right[k] = self.analysis.sum_right[k] * new_gain
                        + self.crossfade.crossfade_sum_right[k] * old_gain;
                }
            }

            // Advance crossfade
            self.crossfade.crossfade_remaining = self
                .crossfade
                .crossfade_remaining
                .saturating_sub(self.config.hop_size);
            if self.crossfade.crossfade_remaining == 0 {
                self.crossfade.crossfade_prev_state = None;
                log::debug!("[BinauralDecoder] HRTF crossfade complete");
            }
        } else {
            // Normal path -- no crossfade
            self.analysis.sum_left.fill(Complex::new(0.0, 0.0));
            self.analysis.sum_right.fill(Complex::new(0.0, 0.0));
            self.analysis.lfe_freq.fill(Complex::new(0.0, 0.0));

            for &ch in &self.coefficients.main_channels {
                let ch_offset = ch * n;
                window_mul_simd(
                    &mut self.analysis.ifft_output_buf,
                    &self.input.input_buffer[ch_offset..ch_offset + n],
                    &self.fft.analysis_window,
                );

                self.fft
                    .fft_r2c
                    .process_with_scratch(
                        &mut self.analysis.ifft_output_buf,
                        &mut self.analysis.temp_freq_buffer,
                        &mut self.analysis.temp_fft_scratch,
                    )
                    .unwrap_or_else(|e| log::error!("[BinauralDecoder] FFT error: {e}"));
                let hrtf = &filters[ch];
                complex_mul_add_simd(
                    &mut self.analysis.sum_left,
                    &self.analysis.temp_freq_buffer,
                    &hrtf[0..freq_size],
                );
                complex_mul_add_simd(
                    &mut self.analysis.sum_right,
                    &self.analysis.temp_freq_buffer,
                    &hrtf[freq_size..],
                );
            }

            if let Some(eq) = df_eq {
                for (k, (sl, sr)) in self
                    .analysis
                    .sum_left
                    .iter_mut()
                    .zip(self.analysis.sum_right.iter_mut())
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
            self.analysis.lfe_freq.fill(Complex::new(0.0, 0.0));
        }
        for &ch in &self.coefficients.lfe_channels {
            let ch_offset = ch * n;
            window_mul_simd(
                &mut self.analysis.ifft_output_buf,
                &self.input.input_buffer[ch_offset..ch_offset + n],
                &self.fft.analysis_window,
            );

            self.fft
                .fft_r2c
                .process_with_scratch(
                    &mut self.analysis.ifft_output_buf,
                    &mut self.analysis.temp_freq_buffer,
                    &mut self.analysis.temp_fft_scratch,
                )
                .unwrap_or_else(|e| log::error!("[BinauralDecoder] FFT error: {e}"));
            complex_mul_add_simd(
                &mut self.analysis.lfe_freq,
                &self.analysis.temp_freq_buffer,
                &self.coefficients.lfe_lowpass_filter,
            );
        }

        // Left IFFT
        self.analysis.sum_left[0].im = 0.0;
        self.analysis.sum_left[freq_size - 1].im = 0.0;
        self.fft
            .fft_c2r
            .process_with_scratch(
                &mut self.analysis.sum_left,
                &mut self.analysis.ifft_output_buf,
                &mut self.analysis.temp_fft_scratch,
            )
            .unwrap_or_else(|e| log::error!("[BinauralDecoder] FFT error: {e}"));
        for i in 0..n {
            let idx = (self.output.next_add_position + i) & mask;
            self.output.output_accumulator[idx * 2] += self.analysis.ifft_output_buf[i] * scale;
        }

        // Right IFFT
        self.analysis.sum_right[0].im = 0.0;
        self.analysis.sum_right[freq_size - 1].im = 0.0;
        self.fft
            .fft_c2r
            .process_with_scratch(
                &mut self.analysis.sum_right,
                &mut self.analysis.ifft_output_buf,
                &mut self.analysis.temp_fft_scratch,
            )
            .unwrap_or_else(|e| log::error!("[BinauralDecoder] FFT error: {e}"));
        for i in 0..n {
            let idx = (self.output.next_add_position + i) & mask;
            self.output.output_accumulator[idx * 2 + 1] += self.analysis.ifft_output_buf[i] * scale;
        }

        // LFE IFFT
        if !self.coefficients.lfe_channels.is_empty() {
            self.analysis.lfe_freq[0].im = 0.0;
            self.analysis.lfe_freq[freq_size - 1].im = 0.0;
            self.fft
                .fft_c2r
                .process_with_scratch(
                    &mut self.analysis.lfe_freq,
                    &mut self.analysis.ifft_output_buf,
                    &mut self.analysis.temp_fft_scratch,
                )
                .unwrap_or_else(|e| log::error!("[BinauralDecoder] FFT error: {e}"));
            let lfe_g = scale * self.coefficients.lfe_gain;
            for i in 0..n {
                let idx = (self.output.next_add_position + i) & mask;
                let s = self.analysis.ifft_output_buf[i] * lfe_g;
                self.output.output_accumulator[idx * 2] += s;
                self.output.output_accumulator[idx * 2 + 1] += s;
            }
        }

        self.output.next_add_position =
            (self.output.next_add_position + self.config.hop_size) & mask;
        self.output.output_accumulator_fill += self.config.hop_size;
        self.output.latency_filled += self.config.hop_size;
    }

    pub(super) fn apply_reflections(&mut self, output: &mut [f32], nf: usize) {
        let ext = self.smoothing.externalization.current();
        let delay_mask = self.room.reflection_delay_mask;

        for i in 0..nf {
            let l = output[i * 2];
            let r = output[i * 2 + 1];
            self.room.reflection_delay_line[self.room.reflection_delay_pos * 2] = l;
            self.room.reflection_delay_line[self.room.reflection_delay_pos * 2 + 1] = r;

            if ext > 0.01 && !self.room.cached_reflections.is_empty() {
                let mut rl = 0.0;
                let mut rr = 0.0;
                for ref_ in &self.room.cached_reflections {
                    let r_pos = (self.room.reflection_delay_pos + delay_mask + 1
                        - ref_.delay_samples)
                        & delay_mask;
                    let g = ref_.gain * ext;

                    // Use HRTF-derived L/R gains when available (SSIR reflections),
                    // otherwise fall back to simple azimuth-based panning (ISM reflections).
                    let (lg, rg) = if let Some(hrtf) = &ref_.hrtf_filter {
                        // Broadband energy from pre-computed HRTF gives perceptually
                        // accurate ILD (interaural level difference) for each reflection DOA.
                        (hrtf.left_gain_broadband, hrtf.right_gain_broadband)
                    } else {
                        (ref_.left_gain, ref_.right_gain)
                    };

                    rl += self.room.reflection_delay_line[r_pos * 2] * g * lg;
                    rr += self.room.reflection_delay_line[r_pos * 2 + 1] * g * rg;
                }
                output[i * 2] += rl;
                output[i * 2 + 1] += rr;
            }
            self.room.reflection_delay_pos = (self.room.reflection_delay_pos + 1) & delay_mask;
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
    pub(super) fn rotate_speaker_position(
        azimuth: f32,
        elevation: f32,
        yaw: f32,
        pitch: f32,
        roll: f32,
    ) -> (f32, f32) {
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
        let x1 = cy * x + sy * y;
        let y1 = -sy * x + cy * y;
        let z1 = z;

        // Ry(-pitch): rotate around Y by -pitch
        let x2 = cp * x1 + sp * z1;
        let y2 = y1;
        let z2 = -sp * x1 + cp * z1;

        // Rx(-roll): rotate around X by -roll
        let x3 = x2;
        let y3 = cr * y2 + sr * z2;
        let z3 = -sr * y2 + cr * z2;

        // Convert back to spherical coordinates
        let new_az = y3.atan2(x3).to_degrees();
        let horiz = (x3 * x3 + y3 * y3).sqrt();
        let new_el = z3.atan2(horiz).to_degrees();

        (new_az, new_el)
    }

    /// Recompute HRTF filters with head-angle-rotated speaker positions and push a new state.
    ///
    /// Spawn (or restart) the background HRTF update thread.
    ///
    /// The thread listens for head-angle triples and recomputes the HRTF state
    /// off the audio thread. It is only started when a SOFA file has been
    /// loaded into the current state.
    pub(super) fn spawn_hrtf_update_thread(&mut self) {
        self.shutdown_hrtf_update_thread();

        // Only spawn if we have HRTF data to rotate. Without a SOFA the default
        // identity filters are used and there is nothing to recompute.
        if self.state.load()._hrtf_data.is_none() {
            return;
        }

        let state = Arc::clone(&self.state);
        let config = self.config.clone();
        let fft_r2c = Arc::clone(&self.fft.fft_r2c);
        let lfe_channels = self.coefficients.lfe_channels.clone();

        let (tx, rx) = sync_channel::<(f32, f32, f32)>(1);
        let handle = std::thread::spawn(move || {
            while let Ok((yaw, pitch, roll)) = rx.recv() {
                let new_state = match Self::compute_head_rotated_hrtf_state(
                    &state,
                    &config,
                    &fft_r2c,
                    &lfe_channels,
                    yaw,
                    pitch,
                    roll,
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        log::warn!("[BinauralDecoder] Background HRTF recompute failed: {}", e);
                        continue;
                    }
                };
                state.store(new_state);
            }
        });

        self.hrtf_update_tx = Some(tx);
        self.hrtf_update_thread = Some(handle);
    }

    fn shutdown_hrtf_update_thread(&mut self) {
        // Dropping the sender unblocks the receiver's recv() with an error,
        // causing the thread to exit cleanly.
        self.hrtf_update_tx.take();
        if let Some(handle) = self.hrtf_update_thread.take()
            && let Err(e) = handle.join()
        {
            log::warn!("[BinauralDecoder] HRTF update thread panicked: {:?}", e);
        }
    }

    /// Recompute a new `BinauralState` for the given head angles.
    /// This is the CPU-heavy work that now runs off the audio thread.
    fn compute_head_rotated_hrtf_state(
        state: &Arc<ArcSwap<BinauralState>>,
        config: &BinauralConfig,
        fft_r2c: &Arc<dyn RealToComplex<f32>>,
        lfe_channels: &[usize],
        yaw: f32,
        pitch: f32,
        roll: f32,
    ) -> PluginResult<Arc<BinauralState>> {
        if config.sample_rate == 0 {
            return Ok(Arc::clone(&state.load_full()));
        }

        let state_guard = state.load();
        let sofa_ref: &SofaFile = match state_guard._hrtf_data.as_ref() {
            Some(s) => s,
            None => return Ok(Arc::clone(&state.load_full())),
        };

        let mut filters =
            vec![vec![Complex::new(0.0, 0.0); config.freq_size * 2]; config.input_channels];

        for spk in config.speaker_config.speakers {
            let ch = spk.channel;
            if ch >= config.input_channels || lfe_channels.contains(&ch) {
                continue;
            }
            let (rotated_az, rotated_el) =
                Self::rotate_speaker_position(spk.azimuth, spk.elevation, yaw, pitch, roll);
            let tgt = sotf_host::sofa::SourcePosition::new(rotated_az, rotated_el, 1.0);
            let near = sofa_ref.find_three_nearest(&tgt);
            let gains = super::hrtf::calculate_vbap_gains(&tgt, &near, sofa_ref);
            let (l_fft, r_fft) = super::hrtf::interpolate_hrtf_frequency_domain(
                &near,
                &gains,
                sofa_ref,
                config.fft_size,
                config.sample_rate,
                fft_r2c,
                config.near_field_strength,
                tgt.azimuth,
                tgt.elevation,
            );
            filters[ch][..config.freq_size].copy_from_slice(&l_fft[..config.freq_size]);
            filters[ch][config.freq_size..].copy_from_slice(&r_fft[..config.freq_size]);
        }

        super::hrtf::normalize_hrtf_gains(
            &mut filters,
            lfe_channels,
            config.freq_size,
            config.input_channels,
        );

        let eq = if config.diffuse_field_eq {
            Some(
                super::filter::compute_diffuse_field_eq(
                    sofa_ref,
                    config.fft_size,
                    config.sample_rate,
                    fft_r2c,
                )
                .map_err(|e| format!("Diffuse field EQ calculation failed: {}", e))?,
            )
        } else {
            None
        };

        let sofa_clone = state_guard._hrtf_data.clone();
        drop(state_guard);

        Ok(Arc::new(BinauralState {
            hrtf_filters_freq: filters,
            diffuse_field_eq_filter: eq,
            _hrtf_data: sofa_clone,
        }))
    }

    pub(super) fn reset_state(&mut self) {
        self.input.input_fill = 0;
        self.output.output_accumulator.fill(0.0);
        self.output.output_accumulator_fill = 0;
        self.output.next_add_position = 0;
        self.output.output_read_position = 0;
        self.output.latency_filled = 0;
        self.room.reflection_delay_line.fill(0.0);
        self.room.reflection_delay_pos = 0;
        // Clear crossfade state on reset
        self.crossfade.crossfade_prev_state = None;
        self.crossfade.crossfade_remaining = 0;
        // Reset RTPGHI state so stale phase history is not carried across resets
        if let Some(ref mut rtpghi) = self.crossfade.rtpghi_left {
            rtpghi.reset();
        }
        if let Some(ref mut rtpghi) = self.crossfade.rtpghi_right {
            rtpghi.reset();
        }
    }
}

impl Drop for BinauralDecoderPlugin {
    fn drop(&mut self) {
        self.shutdown_hrtf_update_thread();
    }
}

impl Plugin for BinauralDecoderPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Binaural Decoder", "2.1.0", "SotF")
    }
    fn input_channels(&self) -> usize {
        self.config.input_channels
    }
    fn output_channels(&self) -> usize {
        2
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.config.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, val: ParameterValue) -> PluginResult<()> {
        // Parameters not in PARAMS — handle separately
        if id.as_str() == "crossfade_ms" {
            let v = val
                .as_float()
                .ok_or_else(|| "crossfade_ms must be a float".to_string())?;
            if v.is_finite() && (10.0..=500.0).contains(&v) {
                self.config.crossfade_ms = v;
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }
        if id.as_str() == "hrtf_file" {
            let path_str = val
                .as_string()
                .ok_or_else(|| "hrtf_file must be a string".to_string())?
                .to_string();
            let new_path = if path_str.is_empty() {
                None
            } else {
                Some(PathBuf::from(&path_str))
            };
            self.config.hrtf_path = new_path;

            if let Some(ref p) = self.config.hrtf_path.clone()
                && self.config.sample_rate > 0
            {
                let mut sofa = SofaFile::load(p)
                    .map_err(|e| format!("Failed to load HRTF file '{}': {}", path_str, e))?;

                let sofa_rate = sofa.sample_rate.round() as u32;
                if sofa_rate != self.config.sample_rate {
                    super::hrtf::resample_sofa(&mut sofa, self.config.sample_rate)
                        .map_err(|e| format!("HRTF resample failed: {}", e))?;
                }

                let mut filters = vec![
                    vec![Complex::new(0.0, 0.0); self.config.freq_size * 2];
                    self.config.input_channels
                ];
                for spk in self.config.speaker_config.speakers {
                    let ch = spk.channel;
                    if ch >= self.config.input_channels
                        || self.coefficients.lfe_channels.contains(&ch)
                    {
                        continue;
                    }
                    let tgt = super::room::speaker_to_source_position(spk);
                    let near = sofa.find_three_nearest(&tgt);
                    let gains = super::hrtf::calculate_vbap_gains(&tgt, &near, &sofa);
                    let (l_fft, r_fft) = super::hrtf::interpolate_hrtf_frequency_domain(
                        &near,
                        &gains,
                        &sofa,
                        self.config.fft_size,
                        self.config.sample_rate,
                        &self.fft.fft_r2c,
                        self.config.near_field_strength,
                        tgt.azimuth,
                        tgt.elevation,
                    );
                    filters[ch][..self.config.freq_size]
                        .copy_from_slice(&l_fft[..self.config.freq_size]);
                    filters[ch][self.config.freq_size..]
                        .copy_from_slice(&r_fft[..self.config.freq_size]);
                }
                super::hrtf::normalize_hrtf_gains(
                    &mut filters,
                    &self.coefficients.lfe_channels,
                    self.config.freq_size,
                    self.config.input_channels,
                );
                let eq = if self.config.diffuse_field_eq {
                    Some(
                        super::filter::compute_diffuse_field_eq(
                            &sofa,
                            self.config.fft_size,
                            self.config.sample_rate,
                            &self.fft.fft_r2c,
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
            }

            self.rebuild_cached_parameters();
            return Ok(());
        }
        if id.as_str() == "head_yaw_deg" {
            let v = val
                .as_float()
                .ok_or_else(|| "head_yaw_deg must be a float".to_string())?;
            if v.is_finite() {
                self.smoothing
                    .head_yaw_deg
                    .set_target(v.clamp(-180.0, 180.0));
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }
        if id.as_str() == "head_pitch_deg" {
            let v = val
                .as_float()
                .ok_or_else(|| "head_pitch_deg must be a float".to_string())?;
            if v.is_finite() {
                self.smoothing
                    .head_pitch_deg
                    .set_target(v.clamp(-180.0, 180.0));
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }
        if id.as_str() == "head_roll_deg" {
            let v = val
                .as_float()
                .ok_or_else(|| "head_roll_deg must be a float".to_string())?;
            if v.is_finite() {
                self.smoothing
                    .head_roll_deg
                    .set_target(v.clamp(-180.0, 180.0));
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }
        if id.as_str() == "hrtf_database_dir" {
            let dir = val
                .as_string()
                .ok_or_else(|| "hrtf_database_dir must be a string".to_string())?
                .to_string();
            self.config.hrtf_database_dir = dir.clone();
            if self.config.sample_rate > 0 && !dir.is_empty() {
                if let Some(best) = super::hrtf_database::best_match(
                    std::path::Path::new(&dir),
                    self.config.head_width_cm,
                    self.config.ear_height_cm,
                ) {
                    log::info!(
                        "[BinauralDecoder] hrtf_database_dir scan: best match = {}",
                        best.display()
                    );
                    self.config.hrtf_path = Some(best.clone());
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
        if id.as_str() == "head_width_cm" {
            let v = val
                .as_float()
                .ok_or_else(|| "head_width_cm must be a float".to_string())?;
            if v.is_finite() && (10.0..=25.0).contains(&v) {
                self.config.head_width_cm = v;
                self.rebuild_cached_parameters();
                if self.config.sample_rate > 0 && !self.config.hrtf_database_dir.is_empty() {
                    let dir = self.config.hrtf_database_dir.clone();
                    if let Some(best) = super::hrtf_database::best_match(
                        std::path::Path::new(&dir),
                        self.config.head_width_cm,
                        self.config.ear_height_cm,
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
        if id.as_str() == "ear_height_cm" {
            let v = val
                .as_float()
                .ok_or_else(|| "ear_height_cm must be a float".to_string())?;
            if v.is_finite() && (4.0..=16.0).contains(&v) {
                self.config.ear_height_cm = v;
                self.rebuild_cached_parameters();
                if self.config.sample_rate > 0 && !self.config.hrtf_database_dir.is_empty() {
                    let dir = self.config.hrtf_database_dir.clone();
                    if let Some(best) = super::hrtf_database::best_match(
                        std::path::Path::new(&dir),
                        self.config.head_width_cm,
                        self.config.ear_height_cm,
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
            7 => {
                // late_reverb_rt60
                self.room.fdn.set_room_params(
                    self.config.late_reverb_rt60,
                    self.config.late_reverb_damping,
                    1.0,
                    self.config.sample_rate,
                );
            }
            8 => {
                // late_reverb_damping
                self.room.fdn.set_room_params(
                    self.config.late_reverb_rt60,
                    self.config.late_reverb_damping,
                    1.0,
                    self.config.sample_rate,
                );
            }
            _ => {}
        }

        self.rebuild_cached_parameters();
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        // Parameters not in PARAMS — handle separately
        if id.as_str() == "crossfade_ms" {
            return Some(ParameterValue::Float(self.config.crossfade_ms));
        }
        if id.as_str() == "sofa_file" || id.as_str() == "hrtf_file" {
            let path_str = self
                .config
                .hrtf_path
                .as_ref()
                .and_then(|p| p.to_str())
                .unwrap_or("")
                .to_string();
            return Some(ParameterValue::String(path_str));
        }
        if id.as_str() == "head_yaw_deg" {
            return Some(ParameterValue::Float(self.smoothing.head_yaw_deg.target()));
        }
        if id.as_str() == "head_pitch_deg" {
            return Some(ParameterValue::Float(
                self.smoothing.head_pitch_deg.target(),
            ));
        }
        if id.as_str() == "head_roll_deg" {
            return Some(ParameterValue::Float(self.smoothing.head_roll_deg.target()));
        }
        if id.as_str() == "hrtf_database_dir" {
            return Some(ParameterValue::String(
                self.config.hrtf_database_dir.clone(),
            ));
        }
        if id.as_str() == "head_width_cm" {
            return Some(ParameterValue::Float(self.config.head_width_cm));
        }
        if id.as_str() == "ear_height_cm" {
            return Some(ParameterValue::Float(self.config.ear_height_cm));
        }
        param_bridge::get_parameter(BN, id, |i| self.param_value(i))
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        enable_ftz_daz();
        self.config.sample_rate = sr;
        self.smoothing.externalization.set_time(50.0, sr);
        self.smoothing.head_yaw_deg.set_time(10.0, sr);
        self.smoothing.head_pitch_deg.set_time(10.0, sr);
        self.smoothing.head_roll_deg.set_time(10.0, sr);

        // Initialize RTPGHI processors for spectral crossfade mode
        self.crossfade.rtpghi_left = Some(RtpghiProcessor::new(
            self.config.fft_size,
            self.config.hop_size,
        ));
        self.crossfade.rtpghi_right = Some(RtpghiProcessor::new(
            self.config.fft_size,
            self.config.hop_size,
        ));
        // Ensure magnitude/phase scratch buffers are correctly sized
        self.crossfade
            .crossfade_mag_left
            .resize(self.config.freq_size, 0.0);
        self.crossfade
            .crossfade_mag_right
            .resize(self.config.freq_size, 0.0);
        self.crossfade
            .crossfade_phase_left
            .resize(self.config.freq_size, 0.0);
        self.crossfade
            .crossfade_phase_right
            .resize(self.config.freq_size, 0.0);
        let (f, g) = super::filter::compute_lfe_filter(
            self.config.fft_size,
            sr,
            self.config.lfe_crossover,
            self.config.lfe_distance,
            self.config.lfe_level,
        );
        self.coefficients.lfe_lowpass_filter = f;
        self.coefficients.lfe_gain = g;
        self.room.cached_reflections.clear();
        if let Some(srir_path) = &self.config.srir_file {
            // SSIR-based measured room reflections
            match super::room::calculate_reflections_from_srir(srir_path, sr) {
                Ok(refs) => {
                    log::info!(
                        "[BinauralDecoder] SSIR: detected {} reflections from '{}'",
                        refs.len(),
                        srir_path.display()
                    );
                    self.room.cached_reflections = refs;
                }
                Err(e) => {
                    log::warn!(
                        "[BinauralDecoder] Failed to load SRIR '{}': {}. Falling back to ISM.",
                        srir_path.display(),
                        e
                    );
                    let refs = super::room::calculate_reflections(
                        &self.config.room_model,
                        self.config.speaker_config,
                        sr,
                    );
                    for (ch, cr) in refs.into_iter().enumerate() {
                        if !self.coefficients.lfe_channels.contains(&ch) {
                            self.room.cached_reflections.extend(cr);
                        }
                    }
                }
            }
        } else {
            // Synthetic ISM room model (existing behavior)
            let refs = super::room::calculate_reflections(
                &self.config.room_model,
                self.config.speaker_config,
                sr,
            );
            for (ch, cr) in refs.into_iter().enumerate() {
                if !self.coefficients.lfe_channels.contains(&ch) {
                    self.room.cached_reflections.extend(cr);
                }
            }
        }

        // AL2: Clamp reflection delay_samples to the delay-line capacity.
        // The delay line is 16384 samples (≈341 ms at 48 kHz). Reflections from
        // large rooms or SRIRs can exceed this. Without clamping the bitmask wraps
        // and the reflection appears at the wrong (possibly negative-relative) time.
        let max_delay = self.room.reflection_delay_mask; // == delay_size - 1
        for refl in &mut self.room.cached_reflections {
            if refl.delay_samples > max_delay {
                log::warn!(
                    "[BinauralDecoder] Reflection delay {} samples exceeds delay-line capacity {}; \
                     clamping. Increase delay_size for rooms with reflections > {:.0} ms.",
                    refl.delay_samples,
                    max_delay,
                    max_delay as f32 / sr as f32 * 1000.0,
                );
                refl.delay_samples = max_delay;
            }
        }

        // If a database directory is configured, scan it now and pick the best
        // match.  This overrides any hrtf_path that was set individually.
        if !self.config.hrtf_database_dir.is_empty() {
            let dir = std::path::Path::new(&self.config.hrtf_database_dir);
            match super::hrtf_database::best_match(
                dir,
                self.config.head_width_cm,
                self.config.ear_height_cm,
            ) {
                Some(best) => {
                    log::info!(
                        "[BinauralDecoder] HRTF database scan: selected '{}'",
                        best.display()
                    );
                    self.config.hrtf_path = Some(best);
                }
                None => {
                    log::warn!(
                        "[BinauralDecoder] HRTF database dir '{}' contains no .sofa files; \
                         falling back to hrtf_path",
                        self.config.hrtf_database_dir
                    );
                }
            }
        }

        if let Some(p) = &self.config.hrtf_path {
            let mut sofa = SofaFile::load(p)?;

            // Resample HRTF IRs if sample rate differs from engine rate
            let sofa_rate = sofa.sample_rate.round() as u32;
            if sofa_rate != sr {
                log::info!(
                    "[BinauralDecoder] HRTF sample rate ({} Hz) differs from engine ({} Hz), resampling",
                    sofa_rate,
                    sr
                );
                super::hrtf::resample_sofa(&mut sofa, sr)?;
            }

            let mut filters = vec![
                vec![Complex::new(0.0, 0.0); self.config.freq_size * 2];
                self.config.input_channels
            ];
            for spk in self.config.speaker_config.speakers {
                let ch = spk.channel;
                if ch >= self.config.input_channels || self.coefficients.lfe_channels.contains(&ch)
                {
                    continue;
                }
                let tgt = super::room::speaker_to_source_position(spk);
                let near = sofa.find_three_nearest(&tgt);
                let gains = super::hrtf::calculate_vbap_gains(&tgt, &near, &sofa);
                let (l_fft, r_fft) = super::hrtf::interpolate_hrtf_frequency_domain(
                    &near,
                    &gains,
                    &sofa,
                    self.config.fft_size,
                    sr,
                    &self.fft.fft_r2c,
                    self.config.near_field_strength,
                    tgt.azimuth,
                    tgt.elevation,
                );
                filters[ch][..self.config.freq_size]
                    .copy_from_slice(&l_fft[..self.config.freq_size]);
                filters[ch][self.config.freq_size..]
                    .copy_from_slice(&r_fft[..self.config.freq_size]);
            }
            super::hrtf::normalize_hrtf_gains(
                &mut filters,
                &self.coefficients.lfe_channels,
                self.config.freq_size,
                self.config.input_channels,
            );
            let eq = if self.config.diffuse_field_eq {
                Some(
                    super::filter::compute_diffuse_field_eq(
                        &sofa,
                        self.config.fft_size,
                        sr,
                        &self.fft.fft_r2c,
                    )
                    .map_err(|e| format!("Diffuse field EQ calculation failed: {}", e))?,
                )
            } else {
                None
            };
            // Pre-compute per-reflection HRTF filters for SSIR reflections
            for refl in &mut self.room.cached_reflections {
                if refl.hrtf_filter.is_some() {
                    continue;
                }
                let tgt =
                    sotf_host::sofa::SourcePosition::new(refl.azimuth_deg, refl.elevation_deg, 1.0);
                let near = sofa.find_three_nearest(&tgt);
                let gains_vbap = super::hrtf::calculate_vbap_gains(&tgt, &near, &sofa);
                let (l_fft, r_fft) = super::hrtf::interpolate_hrtf_frequency_domain(
                    &near,
                    &gains_vbap,
                    &sofa,
                    self.config.fft_size,
                    sr,
                    &self.fft.fft_r2c,
                    0.0, // no near-field for reflections
                    refl.azimuth_deg,
                    refl.elevation_deg,
                );
                refl.hrtf_filter =
                    Some(super::room::ReflectionHrtf::from_freq_domain(l_fft, r_fft));
            }

            let new_state = Arc::new(BinauralState {
                hrtf_filters_freq: filters,
                diffuse_field_eq_filter: eq,
                _hrtf_data: Some(sofa),
            });
            self.state.store(new_state.clone());
            self.crossfade.current_state_snapshot = new_state;
            // Clear any in-progress crossfade on re-initialize
            self.crossfade.crossfade_prev_state = None;
            self.crossfade.crossfade_remaining = 0;
        }

        // Start the background HRTF rotation worker now that the SOFA (if any)
        // is loaded and the shared state is initialized.
        self.spawn_hrtf_update_thread();

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
        validate_interleaved_io(
            "BinauralDecoder",
            nf,
            self.config.input_channels,
            2,
            input.len(),
            output.len(),
        )?;

        // Advance head-angle smoothers and check whether the angles have changed
        // enough (> 0.5°) to require an HRTF recompute.
        let yaw = self.smoothing.head_yaw_deg.next_n(nf);
        let pitch = self.smoothing.head_pitch_deg.next_n(nf);
        let roll = self.smoothing.head_roll_deg.next_n(nf);
        let angle_changed = (yaw - self.smoothing.last_hrtf_yaw).abs() > 0.5
            || (pitch - self.smoothing.last_hrtf_pitch).abs() > 0.5
            || (roll - self.smoothing.last_hrtf_roll).abs() > 0.5;
        if angle_changed {
            // Request a background recomputation instead of blocking the audio
            // thread. The update thread will store the new state; process_audio_block
            // detects the change and crossfades. If the channel is full the thread
            // is still busy, so we drop the stale request; the smoother will send
            // a newer angle on the next frame.
            if let Some(tx) = &self.hrtf_update_tx {
                let _ = tx.try_send((yaw, pitch, roll));
            }
            self.smoothing.last_hrtf_yaw = yaw;
            self.smoothing.last_hrtf_pitch = pitch;
            self.smoothing.last_hrtf_roll = roll;
        }

        let mut ip = 0;
        let mut op = 0;
        let mask = self.output.output_accumulator_mask;
        let n = self.config.fft_size;
        while op < nf {
            if ip < nf {
                let to_copy = (n - self.input.input_fill).min(nf - ip);
                for ch in 0..self.config.input_channels {
                    let off = ch * n;
                    for i in 0..to_copy {
                        self.input.input_buffer[off + self.input.input_fill + i] =
                            input[(ip + i) * self.config.input_channels + ch];
                    }
                }
                self.input.input_fill += to_copy;
                ip += to_copy;
            }
            while self.input.input_fill >= n {
                self.process_audio_block();
                for ch in 0..self.config.input_channels {
                    let off = ch * n;
                    self.input.input_buffer[off..off + n].copy_within(self.config.hop_size..n, 0);
                }
                self.input.input_fill = n - self.config.hop_size;
            }
            let to_drain = self.output.output_accumulator_fill.min(nf - op);
            if to_drain > 0 {
                let drain_slice = &mut output[op * 2..(op + to_drain) * 2];
                for i in 0..to_drain {
                    let ri = (self.output.output_read_position + i) & mask;
                    drain_slice[i * 2] = self.output.output_accumulator[ri * 2];
                    drain_slice[i * 2 + 1] = self.output.output_accumulator[ri * 2 + 1];
                    self.output.output_accumulator[ri * 2] = 0.0;
                    self.output.output_accumulator[ri * 2 + 1] = 0.0;
                }
                self.apply_reflections(drain_slice, to_drain);
                // Phase 4E: Apply late reverb FDN to output
                if self.config.late_reverb_enabled {
                    let mix = self.config.late_reverb_mix;
                    for i in 0..to_drain {
                        let l = drain_slice[i * 2];
                        let r = drain_slice[i * 2 + 1];
                        let (rl, rr) = self.room.fdn.process_stereo(l, r);
                        drain_slice[i * 2] = l * (1.0 - mix) + rl * mix;
                        drain_slice[i * 2 + 1] = r * (1.0 - mix) + rr * mix;
                    }
                }
                self.output.output_read_position =
                    (self.output.output_read_position + to_drain) & mask;
                self.output.output_accumulator_fill -= to_drain;
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
        self.smoothing.externalization.next_n(nf);
        Ok(op)
    }
    fn latency_samples(&self) -> usize {
        self.config.fft_size
    }
}
