// ============================================================================
// Spectrum Analyzer Plugin
// ============================================================================

use crate::analyzer::{RealTimeCache, SpectrumData};
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use crate::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use math_audio_dsp::fast_math::fast_log10;

use rtrb::{Consumer, RingBuffer};
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::Arc;

const FFT_SIZE: usize = 4096;

pub struct SpectrumAnalyzer {
    pub config: SpectrumConfig,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumInfo {
    pub frequencies: Vec<f32>,
    pub magnitudes: Vec<f32>,
    pub peak_magnitude: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumConfig {
    pub num_bins: usize,
    pub min_freq: f32,
    pub max_freq: f32,
    pub smoothing: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SpectralTiltCorrection {
    None,
    ThreeDbPerOctave,
    SixDbPerOctave,
    Pink,
    Custom(f32),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TiltReferenceFreq {
    Standard,
    OneKilohertz,
    TwoKilohertz,
    MinFreq,
}

impl Default for SpectrumConfig {
    fn default() -> Self {
        Self {
            num_bins: 30,
            min_freq: 20.0,
            max_freq: 20000.0,
            smoothing: 0.7,
        }
    }
}

pub struct SpectrumAnalyzerPlugin {
    num_channels: usize,
    sample_rate: u32,
    config: SpectrumConfig,
    producer: rtrb::Producer<f32>,
    consumer: Consumer<f32>,
    cache: RealTimeCache<SpectrumData>,
    // Pre-allocated FFT resources (zero per-frame allocation)
    fft_r2c: Arc<dyn realfft::RealToComplex<f32>>,
    fft_output: Vec<Complex<f32>>,
    windowed: Vec<f32>,
    new_mags: Vec<f32>,
    // Mutable copy of magnitudes for smoothing (avoids cloning shared_data each frame)
    current_magnitudes: Vec<f32>,
    // Pre-computed window coefficients (avoid cos() per sample in hot path)
    window: Vec<f32>,
    // Pre-computed FFT bin → display bin mapping (avoid log10() per bin in hot path)
    bin_to_display: Vec<Option<usize>>,
    // Cached constants derived from config and sample_rate
    fft_bin_hz: f32,
    cached_parameters: Vec<Parameter>,
}

impl SpectrumAnalyzerPlugin {
    fn generate_window() -> Vec<f32> {
        (0..FFT_SIZE)
            .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / FFT_SIZE as f32).cos()))
            .collect()
    }

    fn build_bin_to_display(config: &SpectrumConfig, sample_rate: u32) -> Vec<Option<usize>> {
        let fft_bin_hz = sample_rate as f32 / FFT_SIZE as f32;
        let log_min = config.min_freq.log10();
        let log_max = config.max_freq.log10();
        let log_range = log_max - log_min;
        let spectrum_size = FFT_SIZE / 2 + 1;
        (0..spectrum_size)
            .map(|i| {
                let freq = i as f32 * fft_bin_hz;
                if freq < config.min_freq || freq > config.max_freq {
                    return None;
                }
                let bin_idx = (((freq.log10() - log_min) / log_range) * config.num_bins as f32)
                    .floor() as usize;
                if bin_idx < config.num_bins {
                    Some(bin_idx)
                } else {
                    None
                }
            })
            .collect()
    }

    fn build_common(num_channels: usize, config: SpectrumConfig) -> Result<Self, String> {
        let (p, c) = RingBuffer::new(FFT_SIZE * 4);
        let log_min = config.min_freq.log10();
        let log_max = config.max_freq.log10();
        let mut freqs = Vec::with_capacity(config.num_bins);
        for i in 0..config.num_bins {
            let f1 =
                10.0f32.powf(log_min + (log_max - log_min) * (i as f32 / config.num_bins as f32));
            let f2 = 10.0f32
                .powf(log_min + (log_max - log_min) * ((i + 1) as f32 / config.num_bins as f32));
            freqs.push((f1 * f2).sqrt());
        }
        let initial_data = SpectrumData {
            frequencies: freqs.into(),
            magnitudes: vec![-100.0; config.num_bins].into(),
            peak_magnitude: -100.0,
        };
        let cache = RealTimeCache::new(initial_data);
        let mut planner = realfft::RealFftPlanner::<f32>::new();
        let fft_r2c = planner.plan_fft_forward(FFT_SIZE);
        let fft_output = fft_r2c.make_output_vec();
        let num_bins = config.num_bins;
        let sample_rate = 48000u32;
        let bin_to_display = Self::build_bin_to_display(&config, sample_rate);
        let fft_bin_hz = sample_rate as f32 / FFT_SIZE as f32;
        let mut p = Self {
            num_channels,
            sample_rate,
            config,
            producer: p,
            consumer: c,
            cache,
            fft_r2c,
            fft_output,
            windowed: vec![0.0; FFT_SIZE],
            new_mags: vec![-100.0; num_bins],
            current_magnitudes: vec![-100.0; num_bins],
            window: Self::generate_window(),
            bin_to_display,
            fft_bin_hz,
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_float("smoothing", "Smoothing", self.config.smoothing, 0.0, 0.999),
            Parameter::new_int("num_bins", "Bins", self.config.num_bins as i32, 10, 100),
            Parameter::new_float("min_freq", "Min Freq", self.config.min_freq, 10.0, 500.0),
            Parameter::new_float(
                "max_freq",
                "Max Freq",
                self.config.max_freq,
                1000.0,
                22050.0,
            ),
        ];
    }

    pub fn new(num_channels: usize) -> Result<Self, String> {
        Self::build_common(num_channels, SpectrumConfig::default())
    }

    pub fn with_config(num_channels: usize, config: SpectrumConfig) -> Result<Self, String> {
        Self::build_common(num_channels, config)
    }

    fn rebuild_config_dependent(&mut self) {
        let log_min = self.config.min_freq.log10();
        let log_max = self.config.max_freq.log10();
        let mut freqs = Vec::with_capacity(self.config.num_bins);
        for i in 0..self.config.num_bins {
            let f1 = 10.0f32
                .powf(log_min + (log_max - log_min) * (i as f32 / self.config.num_bins as f32));
            let f2 = 10.0f32.powf(
                log_min + (log_max - log_min) * ((i + 1) as f32 / self.config.num_bins as f32),
            );
            freqs.push((f1 * f2).sqrt());
        }

        self.new_mags.resize(self.config.num_bins, -100.0);
        self.current_magnitudes.resize(self.config.num_bins, -100.0);
        self.bin_to_display = Self::build_bin_to_display(&self.config, self.sample_rate);

        // Update cache structure
        self.cache.update(|data| {
            data.frequencies = freqs.clone().into();
            data.magnitudes = vec![-100.0; self.config.num_bins].into();
            data.peak_magnitude = -100.0;
        });
    }
}

impl Plugin for SpectrumAnalyzerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Spectrum Analyzer", "1.1.0", "Sotf")
    }
    fn input_channels(&self) -> usize {
        self.num_channels
    }
    fn output_channels(&self) -> usize {
        self.num_channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;
        match id.0.as_str() {
            "smoothing" => {
                let v = value.as_float().unwrap_or(0.7);
                if v.is_finite() {
                    self.config.smoothing = v.clamp(0.0, 0.999);
                }
            }
            "num_bins" => {
                let v = value.as_int().unwrap_or(30) as usize;
                if v != self.config.num_bins {
                    self.config.num_bins = v.clamp(10, 100);
                    self.rebuild_config_dependent();
                }
            }
            "min_freq" => {
                let v = value.as_float().unwrap_or(20.0);
                if v.is_finite() && v < self.config.max_freq {
                    self.config.min_freq = v.clamp(10.0, 500.0);
                    self.rebuild_config_dependent();
                }
            }
            "max_freq" => {
                let v = value.as_float().unwrap_or(20000.0);
                if v.is_finite() && v > self.config.min_freq {
                    self.config.max_freq = v.clamp(1000.0, 22050.0);
                    self.rebuild_config_dependent();
                }
            }
            _ => return Err(format!("Unknown parameter: {}", id)),
        }
        self.rebuild_cached_parameters();
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        match id.as_str() {
            "smoothing" => Some(ParameterValue::Float(self.config.smoothing)),
            "num_bins" => Some(ParameterValue::Int(self.config.num_bins as i32)),
            "min_freq" => Some(ParameterValue::Float(self.config.min_freq)),
            "max_freq" => Some(ParameterValue::Float(self.config.max_freq)),
            _ => None,
        }
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        self.fft_bin_hz = sr as f32 / FFT_SIZE as f32;
        self.bin_to_display = Self::build_bin_to_display(&self.config, sr);
        Ok(())
    }
    fn reset(&mut self) {
        self.current_magnitudes.fill(-100.0);
        self.cache.update(|data| {
            if let Some(mut_mags) = Arc::get_mut(&mut data.magnitudes) {
                mut_mags.fill(-100.0);
            }
            data.peak_magnitude = -100.0;
        });
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        output.copy_from_slice(input);

        // Downmix to mono for analysis
        let mut dropped = 0usize;
        if self.num_channels == 2 {
            for i in 0..context.num_frames {
                let s = (input[i * 2] + input[i * 2 + 1]) * 0.5;
                if self.producer.push(s).is_err() {
                    dropped += 1;
                }
            }
        } else {
            let inv_ch = 1.0 / self.num_channels as f32;
            for i in 0..context.num_frames {
                let mut sum = 0.0f32;
                for ch in 0..self.num_channels {
                    sum += input[i * self.num_channels + ch];
                }
                if self.producer.push(sum * inv_ch).is_err() {
                    dropped += 1;
                }
            }
        }
        if dropped > 0 {
            crate::rate_limited_log!(
                warn,
                5,
                "spectrum ring buffer full, dropped {dropped} samples"
            );
        }

        let slots = self.consumer.slots();
        if slots >= FFT_SIZE {
            if let Ok(chunk) = self.consumer.read_chunk(FFT_SIZE) {
                let (s1, s2) = chunk.as_slices();
                let s1_len = s1.len();

                // Window each chunk using SIMD
                crate::simd::window_mul_simd(
                    &mut self.windowed[..s1_len],
                    s1,
                    &self.window[..s1_len],
                );
                crate::simd::window_mul_simd(
                    &mut self.windowed[s1_len..],
                    s2,
                    &self.window[s1_len..FFT_SIZE],
                );

                chunk.commit_all();
            }
            if let Err(e) = self
                .fft_r2c
                .process(&mut self.windowed, &mut self.fft_output)
            {
                crate::rate_limited_log!(error, 5, "spectrum FFT process failed: {e}");
                return Ok(context.num_frames);
            }
            let scale = 2.0 / FFT_SIZE as f32;
            let scale_sq = scale * scale;
            self.new_mags.fill(-100.0);

            for (i, bin) in self.fft_output.iter().enumerate().skip(1) {
                if let Some(display_bin) = self.bin_to_display.get(i).copied().flatten() {
                    // Use norm_sqr to avoid sqrt; convert with 10*log10 instead of 20*log10
                    let norm_sq = bin.norm_sqr() * scale_sq;
                    let db = 10.0 * fast_log10(norm_sq.max(1e-10));
                    self.new_mags[display_bin] = self.new_mags[display_bin].max(db);
                }
            }

            // Smooth magnitudes
            let s = self.config.smoothing;
            let inv_s = 1.0 - s;
            for i in 0..self.config.num_bins {
                self.current_magnitudes[i] =
                    s * self.current_magnitudes[i] + inv_s * self.new_mags[i];
            }

            // Find peak using SIMD-optimized function
            let peak = crate::simd::find_max_abs_simd(&self.current_magnitudes);

            // Update cache in-place (real-time safe)
            self.cache.update(|data| {
                data.update_magnitudes(&self.current_magnitudes);
                data.peak_magnitude = peak;
            });
        }
        Ok(context.num_frames)
    }
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
    fn take_cache_contention_stats(&mut self) -> (u64, u64) {
        self.cache.take_contention_stats()
    }
}
