// ============================================================================
// Convolution Plugin - Partitioned FFT-based convolution
// ============================================================================

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::{flush_denormals_inplace, complex_mul_add_simd};
use super::smoothing::Smoother;
use parking_lot::RwLock;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::path::{Path};
use std::sync::Arc;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, Decoder};
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;

const PARTITION_SIZE: usize = 1024;
const FFT_SIZE: usize = PARTITION_SIZE * 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvolutionPluginParams {
    pub ir_file: String, pub mix: f32, pub gain_db: f32,
}

struct ConvolutionState {
    partitions: Vec<Vec<Vec<Complex<f32>>>>, // [channel][partition][bin]
    num_partitions: usize, ir_channels: usize,
    fft_forward: Arc<dyn rustfft::Fft<f32>>, fft_inverse: Arc<dyn rustfft::Fft<f32>>,
}

pub struct ConvolutionPlugin {
    channels: usize, sample_rate: u32, ir_file: String,
    mix: Smoother, gain_linear: Smoother,
    state: Arc<RwLock<Option<ConvolutionState>>>,
    input_buffers: Vec<Vec<f32>>, input_fill: usize,
    fdl: Vec<Vec<Vec<Complex<f32>>>>, // [channel][partition][bin]
    output_accum: Vec<Vec<f32>>,
}

impl ConvolutionPlugin {
    pub fn new(channels: usize, sample_rate: u32) -> Self {
        Self {
            channels, sample_rate, ir_file: String::new(),
            mix: Smoother::new(1.0, 20.0, sample_rate),
            gain_linear: Smoother::new(1.0, 20.0, sample_rate),
            state: Arc::new(RwLock::new(None)),
            input_buffers: vec![vec![0.0; PARTITION_SIZE]; channels], input_fill: 0,
            fdl: vec![vec![vec![Complex::new(0.0, 0.0); FFT_SIZE]; 0]; channels],
            output_accum: vec![vec![0.0; FFT_SIZE]; channels],
        }
    }

    pub fn from_params(channels: usize, sample_rate: u32, params: ConvolutionPluginParams) -> Result<Self, String> {
        let mut plugin = Self::new(channels, sample_rate);
        if !params.ir_file.is_empty() { let _ = plugin.load_ir(&params.ir_file); }
        plugin.mix.set_target(params.mix);
        plugin.gain_linear.set_target(10.0f32.powf(params.gain_db / 20.0));
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
                for (i, &s) in ch_samples[start..end].iter().enumerate() { block[i] = Complex::new(s, 0.0); }
                fft_forward.process(&mut block);
                ch_parts.push(block);
            }
            partitions.push(ch_parts);
        }

        let num_partitions = partitions[0].len();
        *self.state.write() = Some(ConvolutionState {
            partitions, num_partitions, ir_channels, fft_forward, fft_inverse
        });
        self.fdl = vec![vec![vec![Complex::new(0.0, 0.0); FFT_SIZE]; num_partitions]; self.channels];
        self.ir_file = path.to_string();
        Ok(())
    }

    fn load_wav_file(path: &str) -> Result<Vec<Vec<f32>>, String> {
        use std::fs::File;
        let file = File::open(Path::new(path)).map_err(|e| format!("IO: {}", e))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut reader = symphonia_format_riff::WavReader::try_new(mss, &FormatOptions::default()).map_err(|e| format!("Probe: {}", e))?;
        let track = reader.default_track().ok_or("No track")?;
        let mut decoder = symphonia_codec_pcm::PcmDecoder::try_new(&track.codec_params, &DecoderOptions::default()).map_err(|e| format!("Decoder: {}", e))?;
        let mut samples = vec![Vec::new(); track.codec_params.channels.unwrap().count()];
        while let Ok(packet) = reader.next_packet() {
            let decoded = decoder.decode(&packet).map_err(|e| format!("Decode: {}", e))?;
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
    fn info(&self) -> PluginInfo { PluginInfo::new("Convolution", "1.2.0", "Sotf") }
    fn input_channels(&self) -> usize { self.channels }
    fn output_channels(&self) -> usize { self.channels }
    fn parameters(&self) -> Vec<Parameter> { vec![] }
    fn set_parameter(&mut self, _: ParameterId, _: ParameterValue) -> PluginResult<()> { Ok(()) }
    fn get_parameter(&self, _: &ParameterId) -> Option<ParameterValue> { None }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> { self.sample_rate = sr; self.mix.set_time(20.0, sr); Ok(()) }
    fn reset(&mut self) { for ch in 0..self.channels { for p in 0..self.fdl[ch].len() { self.fdl[ch][p].fill(Complex::new(0.0, 0.0)); } } }

    fn process(&mut self, input: &[f32], output: &mut [f32], context: &ProcessContext) -> Result<usize, String> {
        let nf = context.num_frames;
        let state_guard = self.state.read();
        let state = match &*state_guard { Some(s) => s, None => { output.copy_from_slice(input); return Ok(nf); } };

        let mut in_pos = 0;
        while in_pos < nf {
            let to_copy = (PARTITION_SIZE - self.input_fill).min(nf - in_pos);
            for ch in 0..self.channels {
                for i in 0..to_copy { self.input_buffers[ch][self.input_fill + i] = input[(in_pos + i) * self.channels + ch]; }
            }
            self.input_fill += to_copy;
            
            if self.input_fill == PARTITION_SIZE {
                let m = self.mix.next(); let g = self.gain_linear.next();
                let wet_g = m * g; let dry_g = 1.0 - m;
                let inv_n = 1.0 / FFT_SIZE as f32;

                for ch in 0..self.channels {
                    self.fdl[ch].rotate_right(1);
                    let mut spectrum = vec![Complex::new(0.0, 0.0); FFT_SIZE];
                    for i in 0..PARTITION_SIZE { spectrum[i] = Complex::new(self.input_buffers[ch][i], 0.0); }
                    state.fft_forward.process(&mut spectrum);
                    self.fdl[ch][0] = spectrum;

                    let mut sum = vec![Complex::new(0.0, 0.0); FFT_SIZE];
                    let ir_ch = if state.ir_channels == 1 { 0 } else { ch.min(state.ir_channels - 1) };
                    for p in 0..state.num_partitions { complex_mul_add_simd(&mut sum, &self.fdl[ch][p], &state.partitions[ir_ch][p]); }
                    state.fft_inverse.process(&mut sum);
                    for i in 0..FFT_SIZE { self.output_accum[ch][i] += sum[i].re * inv_n; }
                }

                for i in 0..PARTITION_SIZE {
                    for ch in 0..self.channels {
                        let out_idx = (in_pos - (PARTITION_SIZE - to_copy) + i) * self.channels + ch;
                        let dry = input[out_idx];
                        output[out_idx] = dry * dry_g + self.output_accum[ch][i] * wet_g;
                        self.output_accum[ch][i] = self.output_accum[ch][PARTITION_SIZE + i];
                        self.output_accum[ch][PARTITION_SIZE + i] = 0.0;
                    }
                }
                self.input_fill = 0;
            }
            in_pos += to_copy;
        }
        flush_denormals_inplace(output);
        Ok(nf)
    }
}
