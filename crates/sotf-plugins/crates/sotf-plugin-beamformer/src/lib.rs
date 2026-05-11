// ============================================================================
// Beamformer Plugin — Adaptive and Fixed Beamformers
// ============================================================================
//
// Supports multiple beamforming algorithms:
// - MVDR (Minimum Variance Distortionless Response) — adaptive
// - Superdirective — fixed, precomputed weights
// - GSC (Generalized Sidelobe Canceller) — adaptive, sample-by-sample
//
// Input: M-channel interleaved (one channel per microphone)
// Output: 1-channel (beamformed)

pub mod gsc;
pub mod mvdr;
pub mod params;
pub mod steering;
pub mod superdirective;

use math_audio_dsp::stft::RealFftProcessor;
use nalgebra::Complex;
use serde::{Deserialize, Serialize};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use std::any::Any;
use std::sync::Arc;

use crate::gsc::GscBeamformer;
use crate::mvdr::MvdrBeamformer;
use crate::steering::{ArrayGeometry, compute_all_steering_vectors, compute_steering_delays};
use crate::superdirective::SuperdirectiveBeamformer;

/// Beamformer algorithm selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BeamformerType {
    Mvdr,
    Superdirective,
    Gsc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeamformerPluginParams {
    #[serde(default = "default_num_mics")]
    pub num_mics: usize,
    #[serde(default = "default_mic_spacing")]
    pub mic_spacing_cm: f32,
    #[serde(default)]
    pub steer_angle_deg: f32,
    /// 0=MVDR, 1=Superdirective, 2=GSC
    #[serde(default)]
    pub beamformer_type: usize,
}

fn default_num_mics() -> usize {
    2
}
fn default_mic_spacing() -> f32 {
    5.0
}

impl Default for BeamformerPluginParams {
    fn default() -> Self {
        Self {
            num_mics: 2,
            mic_spacing_cm: 5.0,
            steer_angle_deg: 0.0,
            beamformer_type: 0,
        }
    }
}

impl BeamformerPluginParams {
    fn to_beamformer_type(&self) -> BeamformerType {
        match self.beamformer_type {
            0 => BeamformerType::Mvdr,
            1 => BeamformerType::Superdirective,
            _ => BeamformerType::Gsc,
        }
    }
}

const FFT_SIZE: usize = 512;

/// Beamformer plugin — M-channel input to 1-channel output.
pub struct BeamformerPlugin {
    num_mics: usize,
    sample_rate: u32,
    mic_spacing_cm: f32,
    steer_angle_deg: f32,
    beamformer_type: BeamformerType,
    /// MVDR beamformer
    mvdr: MvdrBeamformer,
    /// Superdirective beamformer
    superdirective: Option<SuperdirectiveBeamformer>,
    /// GSC beamformer
    gsc: GscBeamformer,
    /// Steering vectors (precomputed)
    steering_vectors: Vec<Vec<Complex<f32>>>,
    /// Per-channel input accumulation
    input_buffers: Vec<Vec<f32>>,
    input_fill: usize,
    /// Output buffer
    output_buffer: Vec<f32>,
    output_read_pos: usize,
    output_write_pos: usize,
    /// FFT processor
    fft: RealFftProcessor,
    /// Per-channel frequency domain data
    stft_channels: Vec<Vec<Complex<f32>>>,
    /// Pre-allocated mic samples buffer for GSC path
    gsc_samples: Vec<f32>,
    /// sqrt(Hann) window for WOLA
    window: Vec<f32>,
    /// STFT first-frame flag
    stft_filled: bool,
    /// Overlap-add accumulator
    ola_buffer: Vec<f32>,
    ola_write_pos: usize,
    /// Parameters
    param_steer_angle: ParameterId,
    param_type: ParameterId,
    cached_parameters: Vec<Parameter>,
}

impl BeamformerPlugin {
    pub fn new(num_mics: usize, sample_rate: u32) -> Self {
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
            output_buffer: vec![0.0; FFT_SIZE * 16],
            output_read_pos: 0,
            output_write_pos: 0,
            fft: RealFftProcessor::new_bidirectional(FFT_SIZE),
            stft_channels: vec![vec![Complex::new(0.0, 0.0); spectrum_size]; num_mics],
            gsc_samples: vec![0.0; num_mics],
            window,
            stft_filled: false,
            ola_buffer: vec![0.0; FFT_SIZE * 2],
            ola_write_pos: 0,
            param_steer_angle: ParameterId::from("steer_angle_deg"),
            param_type: ParameterId::from("beamformer_type"),
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        p
    }

    pub fn from_params(sample_rate: u32, params: BeamformerPluginParams) -> Self {
        let mut plugin = Self::new(params.num_mics, sample_rate);
        plugin.mic_spacing_cm = params.mic_spacing_cm;
        plugin.steer_angle_deg = params.steer_angle_deg;
        plugin.beamformer_type = params.to_beamformer_type();
        plugin.update_steering();
        plugin.rebuild_cached_parameters();
        plugin
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_float(
                "steer_angle_deg",
                "Steer Angle",
                self.steer_angle_deg,
                -180.0,
                180.0,
            )
            .with_description("Steering direction in degrees")
            .with_group("Beamformer")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_int(
                "beamformer_type",
                "Algorithm",
                self.beamformer_type as i32,
                0,
                2,
            )
            .with_description("0=MVDR, 1=Superdirective, 2=GSC")
            .with_group("Beamformer")
            .with_importance(ParameterImportance::Critical),
        ];
    }

    fn update_steering(&mut self) {
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
        let delays =
            compute_steering_delays(&geometry, self.steer_angle_deg, 0.0, self.sample_rate as f32);
        self.gsc = GscBeamformer::new(self.num_mics, &delays, 32, 0.01);
    }

    fn available_output(&self) -> usize {
        if self.output_write_pos >= self.output_read_pos {
            self.output_write_pos - self.output_read_pos
        } else {
            self.output_buffer.len() - self.output_read_pos + self.output_write_pos
        }
    }

    fn push_output(&mut self, sample: f32) {
        self.output_buffer[self.output_write_pos] = sample;
        self.output_write_pos = (self.output_write_pos + 1) % self.output_buffer.len();
        debug_assert!(
            self.available_output() < self.output_buffer.len() - 1,
            "Output buffer overflow"
        );
    }

    fn pop_output(&mut self) -> f32 {
        let s = self.output_buffer[self.output_read_pos];
        self.output_read_pos = (self.output_read_pos + 1) % self.output_buffer.len();
        s
    }
}

impl Plugin for BeamformerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Beamformer", "1.0.0", "Sotf")
            .with_description("MVDR / Superdirective / GSC Beamformer")
    }

    fn input_channels(&self) -> usize {
        self.num_mics
    }

    fn output_channels(&self) -> usize {
        1
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;
        if id == self.param_steer_angle {
            self.steer_angle_deg = value.as_float().unwrap_or(0.0).clamp(-180.0, 180.0);
            self.update_steering();
            self.rebuild_cached_parameters();
        } else if id == self.param_type {
            let t = value.as_int().unwrap_or(0);
            self.beamformer_type = match t {
                0 => BeamformerType::Mvdr,
                1 => BeamformerType::Superdirective,
                _ => BeamformerType::Gsc,
            };
            self.rebuild_cached_parameters();
        } else {
            return Err(format!("Unknown parameter: {id}"));
        }
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_steer_angle {
            Some(ParameterValue::Float(self.steer_angle_deg))
        } else if id == &self.param_type {
            Some(ParameterValue::Int(self.beamformer_type as i32))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.update_steering();
        Ok(())
    }

    fn reset(&mut self) {
        self.mvdr.reset();
        self.gsc.reset();
        self.input_fill = 0;
        self.stft_filled = false;
        self.output_read_pos = 0;
        self.output_write_pos = 0;
        self.output_buffer.fill(0.0);
        self.ola_buffer.fill(0.0);
        self.ola_write_pos = 0;
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let nf = context.num_frames;
        let hop = FFT_SIZE / 2;

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
                // MVDR / Superdirective: STFT-based
                for i in 0..nf {
                    // Deinterleave and accumulate
                    for ch in 0..self.num_mics {
                        self.input_buffers[ch][self.input_fill] = input[i * self.num_mics + ch];
                    }
                    self.input_fill += 1;

                    let trigger = if !self.stft_filled {
                        self.input_fill >= hop
                    } else {
                        self.input_fill == FFT_SIZE
                    };

                    if trigger {
                        // STFT analysis for each channel
                        let spectrum_size = FFT_SIZE / 2 + 1;
                        for ch in 0..self.num_mics {
                            for j in 0..FFT_SIZE {
                                self.fft.time_buffer[j] =
                                    self.input_buffers[ch][j] * self.window[j];
                            }
                            self.fft.forward();
                            self.stft_channels[ch][..spectrum_size]
                                .copy_from_slice(&self.fft.freq_buffer[..spectrum_size]);
                        }

                        // Beamform
                        match self.beamformer_type {
                            BeamformerType::Mvdr => {
                                self.mvdr.update_noise_covariance(&self.stft_channels);
                                self.mvdr.compute_weights(&self.steering_vectors);
                                // Apply weights using internal buffers
                                for k in 0..spectrum_size {
                                    let mut sum = Complex::new(0.0, 0.0);
                                    for m in 0..self.stft_channels.len() {
                                        if k < self.stft_channels[m].len()
                                            && m < self.mvdr.weights_buf[k].len()
                                        {
                                            sum += self.mvdr.weights_buf[k][m].conj()
                                                * self.stft_channels[m][k];
                                        }
                                    }
                                    self.fft.freq_buffer[k] = sum;
                                }
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

                        // Synthesis — freq_buffer populated above; fix DC/Nyquist bins
                        self.fft.freq_buffer[0].im = 0.0;
                        self.fft.freq_buffer[spectrum_size - 1].im = 0.0;
                        self.fft.inverse();

                        // Overlap-add synthesis
                        let scale = 1.0 / FFT_SIZE as f32;
                        for j in 0..FFT_SIZE {
                            let pos = (self.ola_write_pos + j) % self.ola_buffer.len();
                            self.ola_buffer[pos] +=
                                self.fft.time_buffer[j] * scale * self.window[j];
                        }
                        for _j in 0..hop {
                            self.push_output(self.ola_buffer[self.ola_write_pos]);
                            self.ola_buffer[self.ola_write_pos] = 0.0;
                            self.ola_write_pos =
                                (self.ola_write_pos + 1) % self.ola_buffer.len();
                        }

                        // Shift input buffers: keep the last (FFT_SIZE - hop) samples
                        // for 50% overlap with the next frame
                        for ch in 0..self.num_mics {
                            self.input_buffers[ch].copy_within(hop..FFT_SIZE, 0);
                            self.input_buffers[ch][FFT_SIZE - hop..].fill(0.0);
                        }
                        self.stft_filled = true;
                        self.input_fill = FFT_SIZE - hop;
                    }
                }

                // Write output
                let available = self.available_output();
                let to_write = nf.min(available);
                for out in &mut output[..to_write] {
                    *out = self.pop_output();
                }
                output[to_write..nf].fill(0.0);
            }
        }

        Ok(nf)
    }

    fn latency_samples(&self) -> usize {
        match self.beamformer_type {
            BeamformerType::Gsc => 0,
            _ => FFT_SIZE / 2, // One hop of accumulation before first output
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_beamformer_plugin_creation() {
        let plugin = BeamformerPlugin::new(2, 48000);
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 1);
    }

    #[test]
    fn test_beamformer_parameters() {
        let mut plugin = BeamformerPlugin::new(2, 48000);
        plugin
            .set_parameter(
                ParameterId::from("steer_angle_deg"),
                ParameterValue::Float(45.0),
            )
            .unwrap();
        assert_eq!(plugin.steer_angle_deg, 45.0);
    }

    #[test]
    fn test_beamformer_gsc_process() {
        let mut plugin = BeamformerPlugin::new(2, 48000);
        plugin.beamformer_type = BeamformerType::Gsc;

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 256,
        };
        let input = vec![0.1f32; 256 * 2];
        let mut output = vec![0.0f32; 256];

        let result = plugin.process(&input, &mut output, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_beamformer_mvdr_process() {
        let mut plugin = BeamformerPlugin::new(2, 48000);
        plugin.beamformer_type = BeamformerType::Mvdr;

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 512,
        };
        let input = vec![0.1f32; 512 * 2];
        let mut output = vec![0.0f32; 512];

        let result = plugin.process(&input, &mut output, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_beamformer_superdirective_process() {
        let mut plugin = BeamformerPlugin::new(2, 48000);
        plugin.beamformer_type = BeamformerType::Superdirective;

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 512,
        };
        let input = vec![0.1f32; 512 * 2];
        let mut output = vec![0.0f32; 512];

        let result = plugin.process(&input, &mut output, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stft_trigger_and_ola() {
        let mut plugin = BeamformerPlugin::new(2, 48000);
        plugin.beamformer_type = BeamformerType::Superdirective;

        // DC input on both mics (broadside)
        let num_frames = 4096;
        let input = vec![0.5f32; num_frames * 2];
        let mut output = vec![0.0f32; num_frames];

        // Process in 512-sample chunks to avoid output buffer overflow
        let chunk_size = 512;
        for chunk in 0..(num_frames / chunk_size) {
            let start = chunk * chunk_size;
            let context = ProcessContext {
                sample_rate: 48000,
                num_frames: chunk_size,
            };
            plugin.process(
                &input[start * 2..(start + chunk_size) * 2],
                &mut output[start..start + chunk_size],
                &context,
            )
            .unwrap();
        }

        // Skip latency and transient
        let latency = plugin.latency_samples();
        let start = latency + 512;
        let end = num_frames - 512;

        // With correct OLA, DC input should produce near-constant DC output
        let mean = output[start..end].iter().sum::<f32>() / (end - start) as f32;
        let variance = output[start..end]
            .iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f32>()
            / (end - start) as f32;

        assert!(
            variance < 1e-3,
            "OLA reconstruction failed: variance={variance}, mean={mean}"
        );
        assert!(
            mean > 0.2,
            "Output mean too low: {mean} (expected ~0.5 for DC passthrough)"
        );
    }

    #[test]
    fn test_stft_sine_passthrough() {
        let mut plugin = BeamformerPlugin::new(2, 48000);
        plugin.beamformer_type = BeamformerType::Superdirective;

        let num_frames = 4096;
        let freq = 440.0;
        let input: Vec<f32> = (0..num_frames * 2)
            .map(|i| {
                let t = (i / 2) as f32 / 48000.0;
                (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5
            })
            .collect();
        let mut output = vec![0.0f32; num_frames];

        // Process in 512-sample chunks
        let chunk_size = 512;
        for chunk in 0..(num_frames / chunk_size) {
            let start = chunk * chunk_size;
            let context = ProcessContext {
                sample_rate: 48000,
                num_frames: chunk_size,
            };
            plugin.process(
                &input[start * 2..(start + chunk_size) * 2],
                &mut output[start..start + chunk_size],
                &context,
            )
            .unwrap();
        }

        let latency = plugin.latency_samples();
        let start = latency + 512;
        let end = num_frames - 512;

        let input_rms: f32 = input[start * 2..end * 2]
            .iter()
            .step_by(2)
            .map(|&x| x * x)
            .sum::<f32>()
            / (end - start) as f32;
        let output_rms: f32 = output[start..end]
            .iter()
            .map(|&x| x * x)
            .sum::<f32>()
            / (end - start) as f32;

        let ratio = output_rms.sqrt() / input_rms.sqrt();
        assert!(
            ratio > 0.3 && ratio < 2.0,
            "STFT sine passthrough failed: ratio={ratio}, input_rms={ir}, output_rms={or}",
            ratio = ratio,
            ir = input_rms.sqrt(),
            or = output_rms.sqrt()
        );
    }
}
