// ============================================================================
// Binaural Decoder Plugin
// ============================================================================

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::{complex_mul_add_simd, enable_ftz_daz, flush_denormals_inplace};
use super::smoothing::{Smoother};
use super::speaker_config::{SpeakerConfig, get_speaker_config_by_channels};
use crate::sofa::SofaFile;
use parking_lot::RwLock;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::path::PathBuf;
use std::sync::Arc;

pub mod error;
pub mod filter;
pub mod hrtf;
pub mod params;
pub mod room;

pub use self::error::BinauralError;
pub use self::params::{BinauralDecoderParams, default_enable_optimization as binaural_default_enable_optimization};
pub use self::room::{Reflection, RoomModel};

struct BinauralState {
    hrtf_filters_freq: Vec<Vec<Complex<f32>>>,
    diffuse_field_eq_filter: Option<[Vec<Complex<f32>>; 2]>,
    hrtf_data: Option<SofaFile>,
}

pub struct BinauralDecoderPlugin {
    input_channels: usize, fft_size: usize, hop_size: usize, sample_rate: u32,
    hrtf_path: Option<PathBuf>, speaker_config: &'static SpeakerConfig,
    fft_r2c: Arc<dyn RealToComplex<f32>>, fft_c2r: Arc<dyn ComplexToReal<f32>>, freq_size: usize,
    state: Arc<RwLock<BinauralState>>, lfe_lowpass_filter: Vec<Complex<f32>>, lfe_gain: f32,
    lfe_channels: Vec<usize>, input_buffer: Vec<f32>, input_buffer_fill: usize,
    output_accumulator: Vec<Vec<f32>>, output_accumulator_fill: usize,
    next_add_position: usize, output_read_position: usize,
    temp_input_block: Vec<f32>, temp_output_block: Vec<f32>, temp_freq_buffer: Vec<Complex<f32>>,
    temp_time_buffer: Vec<f32>, temp_fft_scratch: Vec<Complex<f32>>,
    sum_left: Vec<Complex<f32>>, sum_right: Vec<Complex<f32>>,
    left_output: Vec<f32>, right_output: Vec<f32>,
    lfe_freq: Vec<Complex<f32>>, lfe_output: Vec<f32>,
    enable_optimization: bool, externalization: Smoother,
    near_field_strength: f32, diffuse_field_eq: bool,
    lfe_crossover: f32, lfe_distance: f32, lfe_level: f32,
    room_model: RoomModel, cached_reflections: Vec<Reflection>,
}

impl BinauralDecoderPlugin {
    pub fn new(input_channels: usize, fft_size: usize, hrtf_path: Option<PathBuf>, enable_optimization: bool,
               externalization: f32, near_field_strength: f32, diffuse_field_eq: bool,
               lfe_crossover: f32, lfe_distance: f32, lfe_level: f32, room_model: RoomModel) -> Self {
        let hop_size = fft_size / 2;
        let sr = 44100;
        let freq_size = fft_size / 2 + 1;
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_r2c = planner.plan_fft_forward(fft_size);
        let fft_c2r = planner.plan_fft_inverse(fft_size);
        let scratch_len = fft_r2c.get_scratch_len().max(fft_c2r.get_scratch_len());
        let speaker_config = get_speaker_config_by_channels(input_channels).unwrap_or_else(|| get_speaker_config_by_channels(2).unwrap());
        let lfe_channels = speaker_config.speakers.iter().filter(|s| s.is_lfe).map(|s| s.channel).collect();

        Self {
            input_channels, fft_size, hop_size, sample_rate: sr, hrtf_path, speaker_config,
            fft_r2c, fft_c2r, freq_size,
            state: Arc::new(RwLock::new(BinauralState { hrtf_filters_freq: vec![vec![Complex::new(0.0, 0.0); freq_size * 2]; input_channels], diffuse_field_eq_filter: None, hrtf_data: None })),
            lfe_lowpass_filter: vec![Complex::new(1.0, 0.0); freq_size], lfe_gain: 1.0, lfe_channels,
            input_buffer: vec![0.0; fft_size * input_channels], input_buffer_fill: 0,
            output_accumulator: vec![vec![0.0; fft_size * 4]; 2], output_accumulator_fill: 0,
            next_add_position: 0, output_read_position: 0,
            temp_input_block: vec![0.0; fft_size * input_channels], temp_output_block: vec![0.0; fft_size * 2],
            temp_freq_buffer: vec![Complex::new(0.0, 0.0); freq_size], temp_time_buffer: vec![0.0; fft_size],
            temp_fft_scratch: vec![Complex::new(0.0, 0.0); scratch_len],
            sum_left: vec![Complex::new(0.0, 0.0); freq_size], sum_right: vec![Complex::new(0.0, 0.0); freq_size],
            left_output: vec![0.0; fft_size], right_output: vec![0.0; fft_size],
            lfe_freq: vec![Complex::new(0.0, 0.0); freq_size], lfe_output: vec![0.0; fft_size],
            enable_optimization, externalization: Smoother::new(externalization, 50.0, sr),
            near_field_strength, diffuse_field_eq, lfe_crossover, lfe_distance, lfe_level,
            room_model, cached_reflections: Vec::new(),
        }
    }

    pub fn from_params(params: BinauralDecoderParams) -> Self {
        let hrtf_path = if params.hrtf_file.is_empty() { None } else { Some(std::path::PathBuf::from(params.hrtf_file)) };
        Self::new(
            params.input_channels, params.fft_size, hrtf_path, params.enable_optimization,
            params.externalization, params.near_field_strength, params.diffuse_field_eq,
            params.lfe_crossover, params.lfe_distance, params.lfe_level, params.room_model
        )
    }

    fn process_audio_block(&mut self) {
        let input_needed = self.hop_size * self.input_channels;
        self.temp_input_block[..input_needed].copy_from_slice(&self.input_buffer[..input_needed]);
        let state = self.state.read();
        let filters = &state.hrtf_filters_freq;
        let df_eq = &state.diffuse_field_eq_filter;

        self.sum_left.fill(Complex::new(0.0, 0.0));
        self.sum_right.fill(Complex::new(0.0, 0.0));

        for ch in 0..self.input_channels {
            if self.lfe_channels.contains(&ch) { continue; }
            for i in 0..self.hop_size { self.temp_time_buffer[i] = self.temp_input_block[i * self.input_channels + ch]; }
            self.temp_time_buffer[self.hop_size..self.fft_size].fill(0.0);
            self.fft_r2c.process_with_scratch(&mut self.temp_time_buffer, &mut self.temp_freq_buffer, &mut self.temp_fft_scratch).unwrap();
            let hrtf = &filters[ch];
            complex_mul_add_simd(&mut self.sum_left, &self.temp_freq_buffer, &hrtf[0..self.freq_size]);
            complex_mul_add_simd(&mut self.sum_right, &self.temp_freq_buffer, &hrtf[self.freq_size..]);
        }

        if let Some(eq) = df_eq {
            for k in 0..self.freq_size { self.sum_left[k] *= eq[0][k]; self.sum_right[k] *= eq[1][k]; }
        }

        self.sum_left[0].im = 0.0; self.sum_right[0].im = 0.0;
        self.sum_left[self.freq_size - 1].im = 0.0; self.sum_right[self.freq_size - 1].im = 0.0;

        self.fft_c2r.process_with_scratch(&mut self.sum_left, &mut self.left_output, &mut self.temp_fft_scratch).unwrap();
        self.fft_c2r.process_with_scratch(&mut self.sum_right, &mut self.right_output, &mut self.temp_fft_scratch).unwrap();

        let scale = 1.0 / self.fft_size as f32;
        for i in 0..self.fft_size {
            self.temp_output_block[i * 2] = self.left_output[i] * scale;
            self.temp_output_block[i * 2 + 1] = self.right_output[i] * scale;
        }

        let ext = self.externalization.next();
        if ext > 0.01 {
            for r in &self.cached_reflections {
                let g = r.gain * ext;
                if r.delay_samples < self.fft_size {
                    for i in r.delay_samples..self.fft_size {
                        let si = (i - r.delay_samples) * 2; let di = i * 2;
                        self.temp_output_block[di] += self.temp_output_block[si] * g * r.left_gain;
                        self.temp_output_block[di+1] += self.temp_output_block[si+1] * g * r.right_gain;
                    }
                }
            }
        }

        let buf_size = self.output_accumulator[0].len();
        for i in 0..self.fft_size {
            let wi = (self.next_add_position + i) % buf_size;
            self.output_accumulator[0][wi] += self.temp_output_block[i * 2];
            self.output_accumulator[1][wi] += self.temp_output_block[i * 2 + 1];
        }
        self.next_add_position = (self.next_add_position + self.hop_size) % buf_size;
        self.output_accumulator_fill = (self.output_accumulator_fill + self.hop_size).min(buf_size);
        let shift = self.hop_size * self.input_channels;
        self.input_buffer.copy_within(shift..self.input_buffer_fill, 0);
        self.input_buffer_fill -= shift;
    }

    fn drain_output_accumulator(&mut self, output: &mut [f32], output_pos: usize) -> usize {
        let frames_avail = (output.len() - output_pos) / 2;
        let frames_to_drain = self.output_accumulator_fill.min(frames_avail);
        if frames_to_drain > 0 {
            let buf_size = self.output_accumulator[0].len();
            for i in 0..frames_to_drain {
                let ri = (self.output_read_position + i) % buf_size;
                output[output_pos + i*2] = self.output_accumulator[0][ri];
                output[output_pos + i*2+1] = self.output_accumulator[1][ri];
                self.output_accumulator[0][ri] = 0.0;
                self.output_accumulator[1][ri] = 0.0;
            }
            self.output_read_position = (self.output_read_position + frames_to_drain) % buf_size;
            self.output_accumulator_fill -= frames_to_drain;
        }
        frames_to_drain
    }
}

impl Plugin for BinauralDecoderPlugin {
    fn info(&self) -> PluginInfo { PluginInfo::new("Binaural Decoder", "1.3.0", "SotF") }
    fn input_channels(&self) -> usize { self.input_channels }
    fn output_channels(&self) -> usize { 2 }
    fn parameters(&self) -> Vec<Parameter> { vec![Parameter::new_float("externalization", "Space", 0.0, 0.0, 1.0)] }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id.0 == "externalization" { self.externalization.set_target(value.as_float().ok_or("val")?); Ok(()) } else { Err("unk".into()) }
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id.0 == "externalization" { Some(ParameterValue::Float(self.externalization.target())) } else { None }
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr; self.externalization.set_time(50.0, sr);
        let (f, g) = filter::compute_lfe_filter(self.fft_size, sr, self.lfe_crossover, self.lfe_distance, self.lfe_level);
        self.lfe_lowpass_filter = f; self.lfe_gain = g;
        Ok(())
    }
    fn reset(&mut self) { self.input_buffer_fill = 0; self.output_accumulator_fill = 0; self.output_read_position = 0; self.next_add_position = 0; }
    fn process(&mut self, input: &[f32], output: &mut [f32], context: &ProcessContext) -> Result<usize, String> {
        enable_ftz_daz();
        let mut input_pos = 0;
        let mut output_pos = 0;
        loop {
            let drained = self.drain_output_accumulator(output, output_pos);
            output_pos += drained * 2;
            let needed = self.hop_size * self.input_channels;
            if self.input_buffer_fill >= needed && self.next_add_position + self.fft_size <= self.output_accumulator[0].len() {
                self.process_audio_block(); continue;
            }
            if input_pos < input.len() {
                let to_copy = (input.len() - input_pos).min(needed - self.input_buffer_fill);
                self.input_buffer[self.input_buffer_fill..self.input_buffer_fill+to_copy].copy_from_slice(&input[input_pos..input_pos+to_copy]);
                self.input_buffer_fill += to_copy; input_pos += to_copy; continue;
            }
            break;
        }
        flush_denormals_inplace(output);
        Ok(output_pos / 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binaural_decoder_creation() {
        let plugin = BinauralDecoderPlugin::new(5, 4096, None, true, 0.0, 0.0, false, 120.0, 2.0, 0.0, RoomModel::default());
        assert_eq!(plugin.input_channels(), 5);
        assert_eq!(plugin.output_channels(), 2);
        assert_eq!(plugin.fft_size, 4096);
        assert_eq!(plugin.hop_size, 2048);
    }
}
