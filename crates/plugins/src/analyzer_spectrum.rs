// ============================================================================
// Spectrum Analyzer Plugin
// ============================================================================

use super::analyzer::{RealTimeCache, SpectrumData};
use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};

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
}

impl SpectrumAnalyzerPlugin {
    pub fn new(num_channels: usize) -> Result<Self, String> {
        let (p, c) = RingBuffer::new(FFT_SIZE * 4);
        let config = SpectrumConfig::default();
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
        Ok(Self {
            num_channels,
            sample_rate: 48000,
            config,
            producer: p,
            consumer: c,
            cache,
            fft_r2c,
            fft_output,
            windowed: vec![0.0; FFT_SIZE],
            new_mags: vec![-100.0; num_bins],
            current_magnitudes: vec![-100.0; num_bins],
        })
    }

    pub fn with_config(num_channels: usize, config: SpectrumConfig) -> Result<Self, String> {
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
        Ok(Self {
            num_channels,
            sample_rate: 48000,
            config,
            producer: p,
            consumer: c,
            cache,
            fft_r2c,
            fft_output,
            windowed: vec![0.0; FFT_SIZE],
            new_mags: vec![-100.0; num_bins],
            current_magnitudes: vec![-100.0; num_bins],
        })
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
        Vec::new()
    }
    fn set_parameter(&mut self, _: ParameterId, _: ParameterValue) -> PluginResult<()> {
        Ok(())
    }
    fn get_parameter(&self, _: &ParameterId) -> Option<ParameterValue> {
        None
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
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
        for i in 0..context.num_frames {
            let mut sum = 0.0f32;
            for ch in 0..self.num_channels {
                sum += input[i * self.num_channels + ch];
            }
            let _ = self.producer.push(sum / self.num_channels as f32);
        }
        let slots = self.consumer.slots();
        if slots >= FFT_SIZE {
            if let Ok(chunk) = self.consumer.read_chunk(FFT_SIZE) {
                let (s1, s2) = chunk.as_slices();
                let mut idx = 0;
                for &s in s1 {
                    self.windowed[idx] = s
                        * (0.5
                            * (1.0
                                - (2.0 * std::f32::consts::PI * idx as f32 / FFT_SIZE as f32)
                                    .cos()));
                    idx += 1;
                }
                for &s in s2 {
                    self.windowed[idx] = s
                        * (0.5
                            * (1.0
                                - (2.0 * std::f32::consts::PI * idx as f32 / FFT_SIZE as f32)
                                    .cos()));
                    idx += 1;
                }
                chunk.commit_all();
            }
            self.fft_r2c
                .process(&mut self.windowed, &mut self.fft_output)
                .unwrap();
            let log_min = self.config.min_freq.log10();
            let log_max = self.config.max_freq.log10();
            let fft_bin_hz = self.sample_rate as f32 / FFT_SIZE as f32;
            self.new_mags.fill(-100.0);
            for (i, bin) in self.fft_output.iter().enumerate().skip(1) {
                let freq = i as f32 * fft_bin_hz;
                if freq < self.config.min_freq {
                    continue;
                }
                if freq > self.config.max_freq {
                    break;
                }
                let amp = bin.norm() * 2.0 / FFT_SIZE as f32;
                let db = 20.0 * amp.max(1e-5).log10();
                let bin_idx = (((freq.log10() - log_min) / (log_max - log_min))
                    * self.config.num_bins as f32)
                    .floor() as usize;
                if bin_idx < self.config.num_bins {
                    self.new_mags[bin_idx] = self.new_mags[bin_idx].max(db);
                }
            }
            for i in 0..self.config.num_bins {
                self.current_magnitudes[i] = self.config.smoothing * self.current_magnitudes[i]
                    + (1.0 - self.config.smoothing) * self.new_mags[i];
            }
            let peak = self
                .current_magnitudes
                .iter()
                .copied()
                .fold(-100.0, f32::max);

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
}
