use super::beamformer_plugin_params::BeamformerPluginParams;
use super::misc::FFT_SIZE;
use super::types::BeamformerType;
use crate::gsc::GscBeamformer;
use crate::mvdr::MvdrBeamformer;
use crate::steering::{ArrayGeometry, compute_all_steering_vectors, compute_steering_delays};
use crate::superdirective::SuperdirectiveBeamformer;
use math_audio_dsp::stft::RealFftProcessor;
use nalgebra::Complex;
use plugins_spatial::validate_interleaved_io;
use sotf_host::param_specs::UpdateMode;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{
    Plugin, PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use std::any::Any;
use std::sync::Arc;

/// Beamformer plugin — M-channel input to 1-channel output.
pub struct BeamformerPlugin {
    pub(super) num_mics: usize,
    pub(super) sample_rate: u32,
    pub(super) mic_spacing_cm: f32,
    pub(super) steer_angle_deg: f32,
    pub(super) beamformer_type: BeamformerType,
    /// MVDR beamformer
    pub(super) mvdr: MvdrBeamformer,
    /// Superdirective beamformer
    pub(super) superdirective: Option<SuperdirectiveBeamformer>,
    /// GSC beamformer
    pub(super) gsc: GscBeamformer,
    /// Steering vectors (precomputed)
    pub(super) steering_vectors: Vec<Vec<Complex<f32>>>,
    /// Per-channel input accumulation: [ch][sample], size FFT_SIZE each.
    /// `input_fill` tracks how many valid samples are in [0..input_fill].
    pub(super) input_buffers: Vec<Vec<f32>>,
    pub(super) input_fill: usize,
    /// Overlap-add accumulator: holds FFT_SIZE * 2 samples. New synthesis
    /// output is accumulated here; `ola_read_pos` advances by `hop` each frame.
    pub(super) ola_buffer: Vec<f32>,
    pub(super) ola_read_pos: usize,
    pub(super) ola_write_pos: usize,
    /// FFT processor
    pub(super) fft: RealFftProcessor,
    /// Per-channel frequency domain data
    pub(super) stft_channels: Vec<Vec<Complex<f32>>>,
    /// Pre-allocated mic samples buffer for GSC path
    pub(super) gsc_samples: Vec<f32>,
    /// sqrt(Hann) window for WOLA
    pub(super) window: Vec<f32>,
    /// STFT first-frame flag
    pub(super) stft_filled: bool,
    /// Parameters
    pub(super) param_steer_angle: ParameterId,
    pub(super) param_type: ParameterId,
    pub(super) param_num_mics: ParameterId,
    pub(super) param_mic_spacing: ParameterId,
    pub(super) cached_parameters: Vec<Parameter>,
}

impl BeamformerPlugin {
    pub fn new(num_mics: usize, sample_rate: u32) -> PluginResult<Self> {
        if !(2..=8).contains(&num_mics) {
            return Err(format!(
                "Beamformer requires 2..=8 microphones, got {num_mics}"
            ));
        }
        let spectrum_size = FFT_SIZE / 2 + 1;
        let geometry = ArrayGeometry::Linear {
            num_mics,
            spacing_m: 0.05,
        };

        let steering_vectors =
            compute_all_steering_vectors(&geometry, 0.0, FFT_SIZE, sample_rate as f32);

        let superdirective =
            SuperdirectiveBeamformer::new(&geometry, 0.0, FFT_SIZE, sample_rate as f32, 0.01);

        let delays = compute_steering_delays(&geometry, 0.0, 0.0, sample_rate as f32);
        let window = math_audio_dsp::stft::generate_sqrt_hann_window(FFT_SIZE);

        let mut p = Self {
            num_mics,
            sample_rate,
            mic_spacing_cm: 5.0,
            steer_angle_deg: 0.0,
            beamformer_type: BeamformerType::Mvdr,
            mvdr: MvdrBeamformer::new(num_mics, spectrum_size),
            superdirective: Some(superdirective),
            gsc: GscBeamformer::new(num_mics, &delays, 32, 0.01),
            steering_vectors,
            input_buffers: vec![vec![0.0; FFT_SIZE]; num_mics],
            input_fill: 0,
            // OLA buffer: hold FFT_SIZE * 2 samples so there is always room
            // for the full analysis window output at any hop boundary.
            ola_buffer: vec![0.0; FFT_SIZE * 2],
            ola_read_pos: 0,
            fft: RealFftProcessor::new_bidirectional(FFT_SIZE),
            stft_channels: vec![vec![Complex::new(0.0, 0.0); spectrum_size]; num_mics],
            gsc_samples: vec![0.0; num_mics],
            window,
            stft_filled: false,
            // The first synthesis frame is emitted after exactly FFT_SIZE
            // samples of startup latency.
            ola_write_pos: FFT_SIZE,
            param_steer_angle: ParameterId::from("steer_angle_deg"),
            param_type: ParameterId::from("beamformer_type"),
            param_num_mics: ParameterId::from("num_mics"),
            param_mic_spacing: ParameterId::from("mic_spacing_cm"),
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    pub fn from_params(sample_rate: u32, params: BeamformerPluginParams) -> PluginResult<Self> {
        if !(2..=8).contains(&params.num_mics) {
            return Err(format!(
                "Beamformer requires 2..=8 microphones, got {}",
                params.num_mics
            ));
        }
        if !params.mic_spacing_cm.is_finite() || !(1.0..=50.0).contains(&params.mic_spacing_cm) {
            return Err(format!(
                "Beamformer microphone spacing must be finite in 1..=50 cm, got {}",
                params.mic_spacing_cm
            ));
        }
        if !params.steer_angle_deg.is_finite()
            || !(-180.0..=180.0).contains(&params.steer_angle_deg)
        {
            return Err(format!(
                "Beamformer steering angle must be finite in -180..=180 degrees, got {}",
                params.steer_angle_deg
            ));
        }
        if params.beamformer_type > 2 {
            return Err(format!(
                "Unknown Beamformer algorithm {}",
                params.beamformer_type
            ));
        }
        let mut plugin = Self::new(params.num_mics, sample_rate)?;
        plugin.mic_spacing_cm = params.mic_spacing_cm;
        plugin.steer_angle_deg = params.steer_angle_deg;
        plugin.beamformer_type = params.to_beamformer_type();
        plugin.update_steering();
        plugin.rebuild_cached_parameters();
        Ok(plugin)
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_int("num_mics", "Microphones", self.num_mics as i32, 2, 8)
                .with_group("Array")
                .with_importance(ParameterImportance::Critical)
                .with_update_mode(UpdateMode::Structural),
            Parameter::new_float(
                "mic_spacing_cm",
                "Mic Spacing",
                self.mic_spacing_cm,
                1.0,
                50.0,
            )
            .with_unit("cm")
            .with_group("Array")
            .with_importance(ParameterImportance::Critical)
            .with_update_mode(UpdateMode::Structural),
            Parameter::new_float(
                "steer_angle_deg",
                "Steer Angle",
                self.steer_angle_deg,
                -180.0,
                180.0,
            )
            .with_description("Steering direction in degrees")
            .with_group("Beamformer")
            .with_importance(ParameterImportance::Critical)
            .with_update_mode(UpdateMode::Structural),
            Parameter::new_string(
                "beamformer_type",
                "Algorithm",
                match self.beamformer_type {
                    BeamformerType::Mvdr => "MVDR",
                    BeamformerType::Superdirective => "Superdirective",
                    BeamformerType::Gsc => "GSC",
                }
                .to_string(),
            )
            .with_description("MVDR, Superdirective, or GSC")
            .with_group("Beamformer")
            .with_importance(ParameterImportance::Critical)
            .with_update_mode(UpdateMode::Structural),
        ];
    }

    pub(super) fn update_steering(&mut self) {
        let geometry = ArrayGeometry::Linear {
            num_mics: self.num_mics,
            spacing_m: self.mic_spacing_cm / 100.0,
        };
        self.steering_vectors = compute_all_steering_vectors(
            &geometry,
            self.steer_angle_deg,
            FFT_SIZE,
            self.sample_rate as f32,
        );
        self.superdirective = Some(SuperdirectiveBeamformer::new(
            &geometry,
            self.steer_angle_deg,
            FFT_SIZE,
            self.sample_rate as f32,
            0.01,
        ));
        let delays = compute_steering_delays(
            &geometry,
            self.steer_angle_deg,
            0.0,
            self.sample_rate as f32,
        );
        self.gsc = GscBeamformer::new(self.num_mics, &delays, 32, 0.01);
    }
}

impl Plugin for BeamformerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Beamformer", env!("CARGO_PKG_VERSION"), "Sotf")
            .with_description("MVDR / Superdirective / GSC Beamformer")
    }

    fn input_channels(&self) -> usize {
        self.num_mics
    }

    fn output_channels(&self) -> usize {
        1
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::nonlinear(PluginCostClass::Fft, None, self.latency_samples(), true)
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;
        if id == self.param_steer_angle || id == self.param_num_mics || id == self.param_mic_spacing
        {
            Err(format!(
                "{id} is structural; rebuild the plugin host to change it"
            ))
        } else if id == self.param_type {
            Err("beamformer_type is structural; rebuild the plugin host to change it".into())
        } else {
            Err(format!("Unknown parameter: {id}"))
        }
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_steer_angle {
            Some(ParameterValue::Float(self.steer_angle_deg))
        } else if id == &self.param_type {
            Some(ParameterValue::String(
                match self.beamformer_type {
                    BeamformerType::Mvdr => "MVDR",
                    BeamformerType::Superdirective => "Superdirective",
                    BeamformerType::Gsc => "GSC",
                }
                .to_string(),
            ))
        } else if id == &self.param_num_mics {
            Some(ParameterValue::Int(self.num_mics as i32))
        } else if id == &self.param_mic_spacing {
            Some(ParameterValue::Float(self.mic_spacing_cm))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.update_steering();
        self.reset();
        Ok(())
    }

    fn reset(&mut self) {
        self.mvdr.reset();
        self.gsc.reset();
        self.input_fill = 0;
        for buf in &mut self.input_buffers {
            buf.fill(0.0);
        }
        self.ola_buffer.fill(0.0);
        self.ola_read_pos = 0;
        self.ola_write_pos = FFT_SIZE;
        self.stft_filled = false;
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let nf = context.num_frames;
        let hop = FFT_SIZE / 2;
        let spectrum_size = FFT_SIZE / 2 + 1;
        let ola_len = self.ola_buffer.len();
        validate_interleaved_io(
            "Beamformer",
            nf,
            self.num_mics,
            1,
            input.len(),
            output.len(),
        )?;
        if context.sample_rate != self.sample_rate {
            return Err(format!(
                "Beamformer sample-rate mismatch: initialized at {} Hz, context is {} Hz",
                self.sample_rate, context.sample_rate
            ));
        }

        match self.beamformer_type {
            BeamformerType::Gsc => {
                // GSC: sample-by-sample processing (no allocation)
                for i in 0..nf {
                    for ch in 0..self.num_mics {
                        self.gsc_samples[ch] = input[i * self.num_mics + ch];
                    }
                    output[i] = self.gsc.process_sample(&self.gsc_samples);
                }
            }
            _ => {
                // MVDR / Superdirective: STFT-based with overlap-add.
                //
                // `input_fill` is the count of valid samples currently in
                // input_buffers[..][0..input_fill].  A frame is ready once
                // input_fill reaches FFT_SIZE.  After processing we keep the
                // last (FFT_SIZE - hop) samples as the overlap for the next
                // frame, so input_fill is reset to FFT_SIZE - hop (= hop for
                // 50% overlap).  The trigger condition is therefore
                // `input_fill >= FFT_SIZE`, not `>= hop` — the bug was that
                // it triggered every sample once input_fill first reached hop.
                for i in 0..nf {
                    // Drain before ingesting this timeline sample. A frame that
                    // completes below can therefore only become audible on the
                    // following sample, enforcing the declared FFT_SIZE latency.
                    output[i] = self.ola_buffer[self.ola_read_pos];
                    self.ola_buffer[self.ola_read_pos] = 0.0;
                    self.ola_read_pos = (self.ola_read_pos + 1) % ola_len;

                    // Deinterleave one sample per channel into the accumulator
                    for ch in 0..self.num_mics {
                        self.input_buffers[ch][self.input_fill] = input[i * self.num_mics + ch];
                    }
                    self.input_fill += 1;

                    if self.input_fill >= FFT_SIZE {
                        // STFT analysis for each channel
                        for ch in 0..self.num_mics {
                            for j in 0..FFT_SIZE {
                                self.fft.time_buffer[j] =
                                    self.input_buffers[ch][j] * self.window[j];
                            }
                            self.fft.forward();
                            self.stft_channels[ch][..spectrum_size]
                                .copy_from_slice(&self.fft.freq_buffer[..spectrum_size]);
                        }

                        // Beamform — result lands in fft.freq_buffer
                        match self.beamformer_type {
                            BeamformerType::Mvdr => {
                                let covariance_dirty = self.mvdr.update_noise_covariance(
                                    &self.stft_channels,
                                    &self.steering_vectors,
                                );
                                if covariance_dirty || self.mvdr.weights_dirty() {
                                    self.mvdr.compute_weights(&self.steering_vectors);
                                }
                                let beamformed = self.mvdr.apply_weights_into(&self.stft_channels);
                                self.fft.freq_buffer[..spectrum_size].copy_from_slice(beamformed);
                            }
                            BeamformerType::Superdirective => {
                                if let Some(ref mut sd) = self.superdirective {
                                    let result = sd.apply(&self.stft_channels);
                                    self.fft.freq_buffer[..spectrum_size]
                                        .copy_from_slice(&result[..spectrum_size]);
                                } else {
                                    self.fft.freq_buffer[..spectrum_size]
                                        .fill(Complex::new(0.0, 0.0));
                                }
                            }
                            BeamformerType::Gsc => unreachable!(),
                        };

                        // Fix DC/Nyquist imaginary parts (must be real for real IFFT)
                        self.fft.freq_buffer[0].im = 0.0;
                        self.fft.freq_buffer[spectrum_size - 1].im = 0.0;
                        self.fft.inverse();

                        // Overlap-add synthesis with Hann window (COLA property).
                        // Scale by 1/FFT_SIZE to normalise the IFFT, then multiply
                        // by the synthesis window.  The OLA accumulator is circular
                        // with length FFT_SIZE * 2 — large enough that the write
                        // head never catches the read head for any practical block.
                        let scale = 1.0 / FFT_SIZE as f32;
                        for j in 0..FFT_SIZE {
                            let pos = (self.ola_write_pos + j) % ola_len;
                            self.ola_buffer[pos] +=
                                self.fft.time_buffer[j] * self.window[j] * scale;
                        }

                        // Shift input buffers: keep the last (FFT_SIZE - hop) samples
                        // as the overlap tail for the next frame.
                        for ch in 0..self.num_mics {
                            self.input_buffers[ch].copy_within(hop..FFT_SIZE, 0);
                            self.input_buffers[ch][FFT_SIZE - hop..].fill(0.0);
                        }
                        // Carried-over samples fill positions [0..FFT_SIZE-hop]
                        self.input_fill = FFT_SIZE - hop;
                        self.ola_write_pos = (self.ola_write_pos + hop) % ola_len;
                    }
                }
            }
        }

        Ok(nf)
    }

    fn latency_samples(&self) -> usize {
        match self.beamformer_type {
            BeamformerType::Gsc => self.gsc.latency_samples(),
            // First output samples appear after one full FFT_SIZE of input has
            // accumulated. The OLA read pointer advances in lock-step with the
            // input, so output lags by FFT_SIZE samples.
            _ => FFT_SIZE,
        }
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        None
    }
}

impl std::fmt::Debug for BeamformerPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BeamformerPlugin")
            .field("num_mics", &self.num_mics)
            .field("beamformer_type", &self.beamformer_type)
            .field("steer_angle_deg", &self.steer_angle_deg)
            .finish()
    }
}
