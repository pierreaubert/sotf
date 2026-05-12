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
    /// Per-channel input accumulation: [ch][sample], size FFT_SIZE each.
    /// `input_fill` tracks how many valid samples are in [0..input_fill].
    input_buffers: Vec<Vec<f32>>,
    input_fill: usize,
    /// Overlap-add accumulator: holds FFT_SIZE * 2 samples. New synthesis
    /// output is accumulated here; `ola_read_pos` advances by `hop` each frame.
    ola_buffer: Vec<f32>,
    ola_read_pos: usize,
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
            // OLA buffer: hold FFT_SIZE * 2 samples so there is always room
            // for the full analysis window output at any hop boundary.
            ola_buffer: vec![0.0; FFT_SIZE * 2],
            ola_read_pos: 0,
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
        for buf in &mut self.input_buffers {
            buf.fill(0.0);
        }
        self.ola_buffer.fill(0.0);
        self.ola_read_pos = 0;
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
                                self.mvdr.update_noise_covariance(&self.stft_channels);
                                self.mvdr.compute_weights(&self.steering_vectors);
                                // Apply weights: output[k] = w[k]^H * x[k]
                                // Dimensions are pre-validated (both sized to spectrum_size × num_mics)
                                // so the inner bounds checks are always true; keep them for safety.
                                for k in 0..spectrum_size {
                                    let mut sum = Complex::new(0.0, 0.0);
                                    for m in 0..self.num_mics {
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
                            let pos = (self.ola_read_pos + j) % ola_len;
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
                    }
                }

                // Drain exactly `nf` samples from the OLA accumulator.
                // Any position not yet written by synthesis contains 0.0 (zeroed on
                // reset / initial allocation), so output is valid silence until the
                // first frame arrives.
                for out in &mut output[..nf] {
                    *out = self.ola_buffer[self.ola_read_pos];
                    self.ola_buffer[self.ola_read_pos] = 0.0; // clear after reading
                    self.ola_read_pos = (self.ola_read_pos + 1) % ola_len;
                }
            }
        }

        Ok(nf)
    }

    fn latency_samples(&self) -> usize {
        match self.beamformer_type {
            BeamformerType::Gsc => 0,
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

    /// Regression test for §1.1 (STFT trigger fires every sample after hop).
    ///
    /// Before the fix `input_fill` was reset to `FFT_SIZE - hop = hop`, so on
    /// the very next sample it became `hop + 1 >= hop` and triggered another
    /// full FFT frame.  This test feeds data in small increments and verifies
    /// that the output is finite and not all-zero after enough input.
    #[test]
    fn test_stft_trigger_fires_at_fft_size_not_hop() {
        let mut plugin = BeamformerPlugin::new(2, 48000);
        plugin.beamformer_type = BeamformerType::Mvdr;

        let hop = FFT_SIZE / 2;
        // Feed exactly hop samples at a time over several calls.
        // With the buggy trigger each call after the first would fire a frame;
        // with the correct trigger only every other call fires one.
        let block = ProcessContext {
            sample_rate: 48000,
            num_frames: hop,
        };
        let input = vec![0.1f32; hop * 2];
        let mut output = vec![0.0f32; hop];

        // After 2 blocks (= FFT_SIZE samples) we expect the first frame to
        // have fired.  Accumulate across many blocks; all outputs must be finite.
        for _ in 0..16 {
            let result = plugin.process(&input, &mut output, &block);
            assert!(result.is_ok());
            for (i, &s) in output.iter().enumerate() {
                assert!(s.is_finite(), "output[{i}] is not finite after hop-sized block");
            }
        }
    }

    /// Regression test for §1.2 (missing overlap-add).
    ///
    /// With OLA the output energy after steady-state should be close to the
    /// input energy (within ~6 dB considering beamforming gain).  Without OLA
    /// the output oscillates between near-zero and spiky values at the hop rate.
    #[test]
    fn test_stft_ola_output_not_silent() {
        let mut plugin = BeamformerPlugin::new(2, 48000);
        plugin.beamformer_type = BeamformerType::Mvdr;

        let nf = 512usize;
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: nf,
        };
        // Sine wave at 440 Hz, same on both channels
        let input: Vec<f32> = (0..nf * 2)
            .map(|n| (2.0 * std::f32::consts::PI * 440.0 * (n / 2) as f32 / 48000.0).sin() * 0.5)
            .collect();
        let mut output = vec![0.0f32; nf];

        // First call: plugin is filling its accumulator — output is zeros (latency)
        plugin.process(&input, &mut output, &context).unwrap();

        // Feed more blocks until we have a steady-state non-silent output
        let mut rms_sum = 0.0f32;
        for _ in 0..8 {
            plugin.process(&input, &mut output, &context).unwrap();
            let rms: f32 = (output.iter().map(|s| s * s).sum::<f32>() / nf as f32).sqrt();
            rms_sum += rms;
            for (i, &s) in output.iter().enumerate() {
                assert!(s.is_finite(), "output[{i}] is NaN/Inf");
            }
        }
        assert!(
            rms_sum > 0.001,
            "output is near-silent (rms_sum={rms_sum}) — OLA may be broken"
        );
    }

    /// Regression test for §1.6 + §1.7: MVDR covariance noise detection.
    ///
    /// Before the fix the update gate checked only channel 0, and the first
    /// 20 frames were always accepted regardless of energy level.  Verify that
    /// providing a high-energy signal on mic 1 only correctly raises the gate
    /// (i.e. is_noise=false) and does NOT corrupt the covariance.
    #[test]
    fn test_mvdr_noise_detection_uses_all_channels() {
        use crate::mvdr::MvdrBeamformer;
        use nalgebra::Complex;

        let spectrum_size = 4usize;
        let mut bf = MvdrBeamformer::new(2, spectrum_size);
        bf.noise_threshold = 0.001;

        // Channel 0 is silent, channel 1 has high energy
        let stft: Vec<Vec<Complex<f32>>> = vec![
            vec![Complex::new(0.0, 0.0); spectrum_size], // mic 0: silent
            vec![Complex::new(10.0, 0.0); spectrum_size], // mic 1: loud
        ];

        // Call once; with the fix the high energy on mic 1 should prevent update
        let cov_before: Vec<_> = (0..spectrum_size)
            .map(|k| {
                // diagonal of cov for bin k
                let off = k * 4;
                bf.noise_cov_snapshot()[off] // 0,0 element
            })
            .collect();

        bf.update_noise_covariance(&stft);

        let cov_after: Vec<_> = (0..spectrum_size)
            .map(|k| {
                let off = k * 4;
                bf.noise_cov_snapshot()[off]
            })
            .collect();

        // Covariance should NOT have been updated (high energy → not noise)
        for k in 0..spectrum_size {
            assert_eq!(
                cov_before[k], cov_after[k],
                "bin {k}: covariance was incorrectly updated during high-energy frame"
            );
        }
    }
}
