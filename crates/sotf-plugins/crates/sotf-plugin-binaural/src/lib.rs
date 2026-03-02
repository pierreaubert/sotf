// ============================================================================
// Binaural Decoder Plugin
// ============================================================================

use arc_swap::ArcSwap;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{complex_mul_add_simd, enable_ftz_daz, window_mul_simd};
use sotf_host::smoothing::Smoother;
use sotf_host::sofa::SofaFile;
use sotf_host::speaker_config::{SpeakerConfig, get_speaker_config_by_channels};
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

    latency_filled: usize,
    cached_parameters: Vec<Parameter>,
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
            state: Arc::new(ArcSwap::from_pointee(BinauralState {
                hrtf_filters_freq,
                diffuse_field_eq_filter: None,
                _hrtf_data: None,
            })),
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
            latency_filled: 0,
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        p
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![Parameter::new_float(
            "externalization",
            "Space",
            self.externalization.target(),
            0.0,
            1.0,
        )];
    }

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

    fn process_audio_block(&mut self) {
        let state = self.state.load();
        let filters = &state.hrtf_filters_freq;
        let df_eq = &state.diffuse_field_eq_filter;
        let n = self.fft_size;
        let freq_size = self.freq_size;
        let mask = self.output_accumulator_mask;
        let scale = self.output_scale;

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
                .unwrap();
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
                .unwrap();
            complex_mul_add_simd(
                &mut self.lfe_freq,
                &self.temp_freq_buffer,
                &self.lfe_lowpass_filter,
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

        // Left
        self.sum_left[0].im = 0.0;
        self.sum_left[freq_size - 1].im = 0.0;
        self.fft_c2r
            .process_with_scratch(
                &mut self.sum_left,
                &mut self.ifft_output_buf,
                &mut self.temp_fft_scratch,
            )
            .unwrap();
        for i in 0..n {
            let idx = (self.next_add_position + i) & mask;
            self.output_accumulator[idx * 2] += self.ifft_output_buf[i] * scale;
        }

        // Right
        self.sum_right[0].im = 0.0;
        self.sum_right[freq_size - 1].im = 0.0;
        self.fft_c2r
            .process_with_scratch(
                &mut self.sum_right,
                &mut self.ifft_output_buf,
                &mut self.temp_fft_scratch,
            )
            .unwrap();
        for i in 0..n {
            let idx = (self.next_add_position + i) & mask;
            self.output_accumulator[idx * 2 + 1] += self.ifft_output_buf[i] * scale;
        }

        // LFE
        if !self.lfe_channels.is_empty() {
            self.lfe_freq[0].im = 0.0;
            self.lfe_freq[freq_size - 1].im = 0.0;
            self.fft_c2r
                .process_with_scratch(
                    &mut self.lfe_freq,
                    &mut self.ifft_output_buf,
                    &mut self.temp_fft_scratch,
                )
                .unwrap();
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

    fn reset_state(&mut self) {
        self.input_fill = 0;
        self.output_accumulator.fill(0.0);
        self.output_accumulator_fill = 0;
        self.next_add_position = 0;
        self.output_read_position = 0;
        self.latency_filled = 0;
        self.reflection_delay_line.fill(0.0);
        self.reflection_delay_pos = 0;
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
        self.validate_parameter(&id, &val)?;
        if id.0 == "externalization" {
            let v = val
                .as_float()
                .ok_or_else(|| "externalization must be a float".to_string())?;
            if v.is_finite() {
                self.externalization.set_target(v);
                self.rebuild_cached_parameters();
            }
            Ok(())
        } else {
            Err(format!("Unknown: {}", id))
        }
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id.0 == "externalization" {
            Some(ParameterValue::Float(self.externalization.target()))
        } else {
            None
        }
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        enable_ftz_daz();
        self.sample_rate = sr;
        self.externalization.set_time(50.0, sr);
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
        if let Some(p) = &self.hrtf_path {
            let sofa = SofaFile::load(p)?;
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
            self.state.store(Arc::new(BinauralState {
                hrtf_filters_freq: filters,
                diffuse_field_eq_filter: eq,
                _hrtf_data: Some(sofa),
            }));
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
}
