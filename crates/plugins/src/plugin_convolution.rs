// ============================================================================
// Convolution Plugin - Partitioned FFT-based convolution
// ============================================================================

use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::{complex_mul_add_simd, flush_denormals_inplace};
use super::smoothing::Smoother;
use arc_swap::ArcSwap;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;

const PARTITION_SIZE: usize = 1024;
const FFT_SIZE: usize = PARTITION_SIZE * 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvolutionPluginParams {
    pub ir_file: String,
    pub mix: f32,
    pub gain_db: f32,
}

struct ConvolutionState {
    partitions: Vec<Vec<Vec<Complex<f32>>>>, // [channel][partition][bin]
    num_partitions: usize,
    ir_channels: usize,
    fft_forward: Arc<dyn rustfft::Fft<f32>>,
    fft_inverse: Arc<dyn rustfft::Fft<f32>>,
}

pub struct ConvolutionPlugin {
    channels: usize,
    sample_rate: u32,
    ir_file: String,
    param_mix: ParameterId,
    mix: Smoother,
    mix_value: f32,
    param_gain_db: ParameterId,
    gain_linear: Smoother,
    gain_db_value: f32,
    state: Arc<ArcSwap<Option<ConvolutionState>>>,
    input_buffers: Vec<Vec<f32>>,
    input_fill: usize,
    fdl: Vec<Vec<Vec<Complex<f32>>>>, // [channel][partition][bin]
    fdl_head: usize,                  // ring buffer head for FDL (avoids rotate_right)
    output_accum: Vec<Vec<f32>>,
    // Pre-allocated scratch buffers (avoid heap allocs in audio callback)
    fft_spectrum: Vec<Complex<f32>>,
    fft_sum: Vec<Complex<f32>>,
    fft_scratch: Vec<Complex<f32>>,
}

impl ConvolutionPlugin {
    pub fn new(channels: usize, sample_rate: u32) -> Self {
        Self {
            channels,
            sample_rate,
            ir_file: String::new(),
            param_mix: ParameterId::from("mix"),
            mix: Smoother::new(1.0, 20.0, sample_rate),
            mix_value: 1.0,
            param_gain_db: ParameterId::from("gain_db"),
            gain_linear: Smoother::new(1.0, 20.0, sample_rate),
            gain_db_value: 0.0,
            state: Arc::new(ArcSwap::from_pointee(None)),
            input_buffers: vec![vec![0.0; PARTITION_SIZE]; channels],
            input_fill: 0,
            fdl: vec![vec![vec![Complex::new(0.0, 0.0); FFT_SIZE]; 0]; channels],
            fdl_head: 0,
            output_accum: vec![vec![0.0; FFT_SIZE]; channels],
            fft_spectrum: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            fft_sum: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            fft_scratch: Vec::new(),
        }
    }

    pub fn from_params(
        channels: usize,
        sample_rate: u32,
        params: ConvolutionPluginParams,
    ) -> Result<Self, String> {
        let mut plugin = Self::new(channels, sample_rate);
        if !params.ir_file.is_empty() {
            let _ = plugin.load_ir(&params.ir_file);
        }
        plugin.mix_value = params.mix;
        plugin.mix.set_target(params.mix);
        plugin.gain_db_value = params.gain_db;
        plugin
            .gain_linear
            .set_target(10.0f32.powf(params.gain_db / 20.0));
        Ok(plugin)
    }

    pub fn load_ir(&mut self, path: &str) -> Result<(), String> {
        let ir_samples = Self::load_wav_file(path)?;
        let ir_channels = ir_samples.len();
        let mut planner = FftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(FFT_SIZE);
        let fft_inverse = planner.plan_fft_inverse(FFT_SIZE);

        let mut partitions = Vec::with_capacity(ir_channels);
        for ch_samples in ir_samples {
            let num_parts = (ch_samples.len() + PARTITION_SIZE - 1) / PARTITION_SIZE;
            let mut ch_parts = Vec::with_capacity(num_parts);
            for p in 0..num_parts {
                let mut block = vec![Complex::new(0.0, 0.0); FFT_SIZE];
                let start = p * PARTITION_SIZE;
                let end = (start + PARTITION_SIZE).min(ch_samples.len());
                for (i, &s) in ch_samples[start..end].iter().enumerate() {
                    block[i] = Complex::new(s, 0.0);
                }
                fft_forward.process(&mut block);
                ch_parts.push(block);
            }
            partitions.push(ch_parts);
        }

        let num_partitions = partitions[0].len();
        let fft_scratch_len = fft_forward
            .get_inplace_scratch_len()
            .max(fft_inverse.get_inplace_scratch_len());
        self.state.store(Arc::new(Some(ConvolutionState {
            partitions,
            num_partitions,
            ir_channels,
            fft_forward,
            fft_inverse,
        })));
        self.fdl =
            vec![vec![vec![Complex::new(0.0, 0.0); FFT_SIZE]; num_partitions]; self.channels];
        self.fdl_head = 0;
        self.fft_scratch = vec![Complex::new(0.0, 0.0); fft_scratch_len];
        self.ir_file = path.to_string();
        Ok(())
    }

    fn load_wav_file(path: &str) -> Result<Vec<Vec<f32>>, String> {
        use std::fs::File;
        let file = File::open(Path::new(path)).map_err(|e| format!("IO: {}", e))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut reader = symphonia_format_riff::WavReader::try_new(mss, &FormatOptions::default())
            .map_err(|e| format!("Probe: {}", e))?;
        let track = reader.default_track().ok_or("No track")?;
        let mut decoder = symphonia_codec_pcm::PcmDecoder::try_new(
            &track.codec_params,
            &DecoderOptions::default(),
        )
        .map_err(|e| format!("Decoder: {}", e))?;
        let mut samples = vec![Vec::new(); track.codec_params.channels.unwrap().count()];
        while let Ok(packet) = reader.next_packet() {
            let decoded = decoder
                .decode(&packet)
                .map_err(|e| format!("Decode: {}", e))?;
            for ch in 0..samples.len() {
                match &decoded {
                    AudioBufferRef::F32(buf) => samples[ch].extend_from_slice(buf.chan(ch)),
                    _ => return Err("Format not supported".into()),
                }
            }
        }
        Ok(samples)
    }
}

impl Plugin for ConvolutionPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Convolution", "1.2.0", "Sotf")
    }
    fn input_channels(&self) -> usize {
        self.channels
    }
    fn output_channels(&self) -> usize {
        self.channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        use super::param_specs::convolution::*;
        vec![
            Parameter::new_float("mix", "Mix", MIX_DEFAULT, MIX_MIN, MIX_MAX)
                .with_description("Dry/wet mix (0 = dry, 1 = convolved)")
                .with_group("Output")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float("gain_db", "Gain", GAIN_DB_DEFAULT, GAIN_DB_MIN, GAIN_DB_MAX)
                .with_description("Output gain (dB)")
                .with_group("Output")
                .with_importance(ParameterImportance::Useful),
        ]
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_mix {
            let val = value.as_float().ok_or("val")?.clamp(0.0, 1.0);
            self.mix_value = val;
            self.mix.set_target(val);
        } else if id == self.param_gain_db {
            let val = value.as_float().ok_or("val")?;
            self.gain_db_value = val;
            self.gain_linear.set_target(10.0f32.powf(val / 20.0));
        } else {
            return Err(format!("Unknown parameter: {}", id));
        }
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_mix {
            Some(ParameterValue::Float(self.mix_value))
        } else if id == &self.param_gain_db {
            Some(ParameterValue::Float(self.gain_db_value))
        } else {
            None
        }
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        self.mix.set_time(20.0, sr);
        Ok(())
    }
    fn reset(&mut self) {
        for ch in 0..self.channels {
            for p in 0..self.fdl[ch].len() {
                self.fdl[ch][p].fill(Complex::new(0.0, 0.0));
            }
        }
        self.fdl_head = 0;
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let nf = context.num_frames;
        let state_guard = self.state.load();
        let state = match state_guard.as_ref() {
            Some(s) => s,
            None => {
                output.copy_from_slice(input);
                return Ok(nf);
            }
        };

        let num_partitions = state.num_partitions;

        let mut in_pos = 0;
        while in_pos < nf {
            let to_copy = (PARTITION_SIZE - self.input_fill).min(nf - in_pos);
            for ch in 0..self.channels {
                for i in 0..to_copy {
                    self.input_buffers[ch][self.input_fill + i] =
                        input[(in_pos + i) * self.channels + ch];
                }
            }
            self.input_fill += to_copy;

            if self.input_fill == PARTITION_SIZE {
                let m = self.mix.next();
                let g = self.gain_linear.next();
                let wet_g = m * g;
                let dry_g = 1.0 - m;
                let inv_n = 1.0 / FFT_SIZE as f32;

                // Advance FDL ring buffer head (replaces rotate_right)
                self.fdl_head = if self.fdl_head == 0 {
                    num_partitions - 1
                } else {
                    self.fdl_head - 1
                };

                for ch in 0..self.channels {
                    // Fill pre-allocated spectrum buffer (zero-padded)
                    for i in 0..PARTITION_SIZE {
                        self.fft_spectrum[i] =
                            Complex::new(self.input_buffers[ch][i], 0.0);
                    }
                    for i in PARTITION_SIZE..FFT_SIZE {
                        self.fft_spectrum[i] = Complex::new(0.0, 0.0);
                    }
                    state
                        .fft_forward
                        .process_with_scratch(&mut self.fft_spectrum, &mut self.fft_scratch);

                    // Store into FDL at ring head
                    self.fdl[ch][self.fdl_head].copy_from_slice(&self.fft_spectrum);

                    // Accumulate convolution sum using pre-allocated buffer
                    self.fft_sum.fill(Complex::new(0.0, 0.0));
                    let ir_ch = if state.ir_channels == 1 {
                        0
                    } else {
                        ch.min(state.ir_channels - 1)
                    };
                    for p in 0..num_partitions {
                        let fdl_idx = (self.fdl_head + p) % num_partitions;
                        complex_mul_add_simd(
                            &mut self.fft_sum,
                            &self.fdl[ch][fdl_idx],
                            &state.partitions[ir_ch][p],
                        );
                    }
                    state
                        .fft_inverse
                        .process_with_scratch(&mut self.fft_sum, &mut self.fft_scratch);
                    for i in 0..FFT_SIZE {
                        self.output_accum[ch][i] += self.fft_sum[i].re * inv_n;
                    }
                }

                // Drain output accumulator using copy_within + fill
                for ch in 0..self.channels {
                    let out_base =
                        (in_pos - (PARTITION_SIZE - to_copy)) * self.channels;
                    for i in 0..PARTITION_SIZE {
                        let out_idx = out_base + i * self.channels + ch;
                        let dry = input[out_idx];
                        output[out_idx] =
                            dry * dry_g + self.output_accum[ch][i] * wet_g;
                    }
                    self.output_accum[ch]
                        .copy_within(PARTITION_SIZE..FFT_SIZE, 0);
                    self.output_accum[ch][PARTITION_SIZE..].fill(0.0);
                }
                self.input_fill = 0;
            }
            in_pos += to_copy;
        }
        flush_denormals_inplace(output);
        Ok(nf)
    }
}
