// ============================================================================
// Spectrum Analyzer Plugin
// ============================================================================

use crate::analyzer::{RealTimeCache, SpectrumData};
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use crate::plugin::{
    Plugin, PluginCompileMetadata, PluginCompiledOp, PluginCostClass, PluginInfo, PluginResult,
    ProcessContext,
};
use math_audio_dsp::fast_math::fast_log10;

use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::Arc;

const FFT_SIZE: usize = 4096;
const HANN_ENBW_BINS: f32 = 1.5;
const MIN_BINS: usize = 8;
const MAX_BINS: usize = 120;

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
#[serde(default)]
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
    // Latest analysis window per channel. Keeping only the newest samples gives
    // a display analyzer bounded freshness for arbitrarily large blocks.
    recent_samples: Vec<f32>,
    recent_write: usize,
    samples_since_analysis: usize,
    samples_since_publish: usize,
    cache: RealTimeCache<SpectrumData>,
    // Pre-allocated FFT resources (zero per-frame allocation)
    fft_r2c: Arc<dyn realfft::RealToComplex<f32>>,
    fft_output: Vec<Complex<f32>>,
    fft_line_max_power: Vec<f32>,
    windowed: Vec<f32>,
    new_mags: Vec<f32>,
    // Mutable copy of magnitudes for smoothing (avoids cloning shared_data each frame)
    current_magnitudes: Vec<f32>,
    current_peak_magnitude: f32,
    has_analysis: bool,
    // Pre-computed window coefficients (avoid cos() per sample in hot path)
    window: Vec<f32>,
    // Pre-computed FFT bin → display bin mapping (avoid log10() per bin in hot path)
    bin_to_display: Vec<Option<usize>>,
    display_bin_has_fft_line: Vec<bool>,
    // Cached constants derived from config and sample_rate
    fft_bin_hz: f32,
    cached_parameters: Vec<Parameter>,
    initialized: bool,
}

impl SpectrumAnalyzerPlugin {
    fn generate_window() -> Vec<f32> {
        (0..FFT_SIZE)
            .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / FFT_SIZE as f32).cos()))
            .collect()
    }

    fn validate_config(
        num_channels: usize,
        sample_rate: u32,
        config: &SpectrumConfig,
    ) -> Result<(), String> {
        if num_channels == 0 {
            return Err("spectrum analyzer requires at least one channel".into());
        }
        if sample_rate == 0 {
            return Err("spectrum analyzer sample rate must be non-zero".into());
        }
        if !(MIN_BINS..=MAX_BINS).contains(&config.num_bins) {
            return Err(format!("num_bins must be in {MIN_BINS}..={MAX_BINS}"));
        }
        if !config.min_freq.is_finite()
            || !config.max_freq.is_finite()
            || config.min_freq <= 0.0
            || config.max_freq <= config.min_freq
        {
            return Err("frequency bounds must be finite, positive, and increasing".into());
        }
        let nyquist = sample_rate as f32 * 0.5;
        if config.max_freq > nyquist {
            return Err(format!(
                "max_freq {} exceeds Nyquist {nyquist} at {sample_rate} Hz",
                config.max_freq
            ));
        }
        if !config.smoothing.is_finite() || !(0.0..=1.0).contains(&config.smoothing) {
            return Err("smoothing must be finite and in 0..=1".into());
        }
        Ok(())
    }

    fn build_bin_to_display(
        config: &SpectrumConfig,
        sample_rate: u32,
    ) -> (Vec<Option<usize>>, Vec<bool>) {
        let fft_bin_hz = sample_rate as f32 / FFT_SIZE as f32;
        let log_min = config.min_freq.log10();
        let log_max = config.max_freq.log10();
        let log_range = log_max - log_min;
        let spectrum_size = FFT_SIZE / 2 + 1;
        let mapping: Vec<_> = (0..spectrum_size)
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
            .collect();
        let mut occupied = vec![false; config.num_bins];
        for display_bin in mapping.iter().skip(1).flatten() {
            occupied[*display_bin] = true;
        }
        (mapping, occupied)
    }

    fn build_frequencies(config: &SpectrumConfig) -> Vec<f32> {
        let log_min = config.min_freq.log10();
        let log_max = config.max_freq.log10();
        (0..config.num_bins)
            .map(|i| {
                let f1 = 10.0f32
                    .powf(log_min + (log_max - log_min) * (i as f32 / config.num_bins as f32));
                let f2 = 10.0f32.powf(
                    log_min + (log_max - log_min) * ((i + 1) as f32 / config.num_bins as f32),
                );
                (f1 * f2).sqrt()
            })
            .collect()
    }

    fn build_common(
        num_channels: usize,
        sample_rate: u32,
        config: SpectrumConfig,
    ) -> Result<Self, String> {
        Self::validate_config(num_channels, sample_rate, &config)?;
        let freqs = Self::build_frequencies(&config);
        let make_data = || SpectrumData {
            frequencies: Arc::new(freqs.clone()),
            magnitudes: vec![-100.0; config.num_bins].into(),
            peak_magnitude: -100.0,
        };
        // Build the two nested Arc payloads independently. Cloning one
        // SpectrumData would make the first audio-thread update allocate.
        let cache = RealTimeCache::new_triplet(make_data(), make_data(), make_data());
        let mut planner = realfft::RealFftPlanner::<f32>::new();
        let fft_r2c = planner.plan_fft_forward(FFT_SIZE);
        let fft_output = fft_r2c.make_output_vec();
        let num_bins = config.num_bins;
        let (bin_to_display, display_bin_has_fft_line) =
            Self::build_bin_to_display(&config, sample_rate);
        let fft_bin_hz = sample_rate as f32 / FFT_SIZE as f32;
        let mut p = Self {
            num_channels,
            sample_rate,
            config,
            recent_samples: vec![0.0; FFT_SIZE * num_channels],
            recent_write: 0,
            samples_since_analysis: 0,
            samples_since_publish: 0,
            cache,
            fft_r2c,
            fft_output,
            fft_line_max_power: vec![0.0; FFT_SIZE / 2 + 1],
            windowed: vec![0.0; FFT_SIZE],
            new_mags: vec![-100.0; num_bins],
            current_magnitudes: vec![-100.0; num_bins],
            current_peak_magnitude: -100.0,
            has_analysis: false,
            window: Self::generate_window(),
            bin_to_display,
            display_bin_has_fft_line,
            fft_bin_hz,
            cached_parameters: Vec::new(),
            initialized: false,
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_float("smoothing", "Smoothing", self.config.smoothing, 0.0, 0.999),
            Parameter::new_int(
                "num_bins",
                "Bins",
                self.config.num_bins as i32,
                MIN_BINS as i32,
                MAX_BINS as i32,
            ),
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
        Self::build_common(num_channels, 48_000, SpectrumConfig::default())
    }

    pub fn with_config(num_channels: usize, config: SpectrumConfig) -> Result<Self, String> {
        Self::build_common(num_channels, 48_000, config)
    }

    /// Construct with the host's actual sample rate so invalid Nyquist bounds
    /// are rejected before any configuration-sized allocation occurs.
    pub fn with_config_at_sample_rate(
        num_channels: usize,
        sample_rate: u32,
        config: SpectrumConfig,
    ) -> Result<Self, String> {
        Self::build_common(num_channels, sample_rate, config)
    }

    fn rebuild_config_dependent(&mut self) {
        let freqs = Self::build_frequencies(&self.config);

        self.new_mags.resize(self.config.num_bins, -100.0);
        self.current_magnitudes.resize(self.config.num_bins, -100.0);
        (self.bin_to_display, self.display_bin_has_fft_line) =
            Self::build_bin_to_display(&self.config, self.sample_rate);

        // Structural changes are setup-only. Replace both cache generations
        // before activation so the audio callback never has to resize arrays.
        let make_data = || SpectrumData {
            frequencies: Arc::new(freqs.clone()),
            magnitudes: Arc::from(vec![-100.0; self.config.num_bins]),
            peak_magnitude: -100.0,
        };
        self.cache = RealTimeCache::new_triplet(make_data(), make_data(), make_data());
    }
}

impl Plugin for SpectrumAnalyzerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Spectrum Analyzer", "1.1.0", "Sotf")
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Analyzer
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::analyzer(Some(PluginCompiledOp::AnalyzerTap))
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
        match id.as_str() {
            "smoothing" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "smoothing must be a float".to_string())?;
                if !v.is_finite() || !(0.0..=1.0).contains(&v) {
                    return Err("smoothing must be finite and in 0..=1".into());
                }
                if v == self.config.smoothing {
                    return Ok(());
                }
                self.config.smoothing = v;
                if let Some(parameter) = self.cached_parameters.first_mut() {
                    parameter.default_value = ParameterValue::Float(v);
                }
                return Ok(());
            }
            "num_bins" => {
                let v = value
                    .as_int()
                    .ok_or_else(|| "num_bins must be an integer".to_string())?;
                if !(MIN_BINS as i32..=MAX_BINS as i32).contains(&v) {
                    return Err(format!("num_bins must be in {MIN_BINS}..={MAX_BINS}"));
                }
                if v as usize == self.config.num_bins {
                    return Ok(());
                }
                if self.initialized {
                    return Err("num_bins is setup-only; recreate the analyzer".into());
                }
                self.config.num_bins = v as usize;
                self.rebuild_config_dependent();
            }
            "min_freq" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "min_freq must be a float".to_string())?;
                if !v.is_finite() || v < 10.0 || v > 500.0 || v >= self.config.max_freq {
                    return Err("min_freq must be finite, in 10..=500, and below max_freq".into());
                }
                if v == self.config.min_freq {
                    return Ok(());
                }
                if self.initialized {
                    return Err("min_freq is setup-only; recreate the analyzer".into());
                }
                self.config.min_freq = v;
                self.rebuild_config_dependent();
            }
            "max_freq" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "max_freq must be a float".to_string())?;
                let nyquist = self.sample_rate as f32 * 0.5;
                if !v.is_finite()
                    || v < 1000.0
                    || v > 22050.0
                    || v > nyquist
                    || v <= self.config.min_freq
                {
                    return Err("max_freq must be finite, within the declared range/Nyquist, and above min_freq".into());
                }
                if v == self.config.max_freq {
                    return Ok(());
                }
                if self.initialized {
                    return Err("max_freq is setup-only; recreate the analyzer".into());
                }
                self.config.max_freq = v;
                self.rebuild_config_dependent();
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
        Self::validate_config(self.num_channels, sr, &self.config)?;
        self.sample_rate = sr;
        self.fft_bin_hz = sr as f32 / FFT_SIZE as f32;
        (self.bin_to_display, self.display_bin_has_fft_line) =
            Self::build_bin_to_display(&self.config, sr);
        self.initialized = true;
        Ok(())
    }
    fn reset(&mut self) {
        self.recent_samples.fill(0.0);
        self.recent_write = 0;
        self.samples_since_analysis = 0;
        self.samples_since_publish = 0;
        self.windowed.fill(0.0);
        self.new_mags.fill(-100.0);
        self.current_magnitudes.fill(-100.0);
        self.current_peak_magnitude = -100.0;
        self.has_analysis = false;
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
        if !self.initialized {
            return Err("spectrum analyzer must be initialized before processing".into());
        }
        let expected = context
            .num_frames
            .checked_mul(self.num_channels)
            .ok_or_else(|| "spectrum analyzer buffer length overflow".to_string())?;
        if input.len() != expected || output.len() != expected {
            return Err(format!(
                "spectrum analyzer expected {expected} interleaved samples, got input={} output={}",
                input.len(),
                output.len()
            ));
        }
        if context.sample_rate != self.sample_rate {
            return Err(format!(
                "spectrum analyzer context rate {} does not match initialized rate {}",
                context.sample_rate, self.sample_rate
            ));
        }
        output.copy_from_slice(input);

        // Retain each channel independently. Analysis later takes the maximum
        // power at each FFT line, avoiding both phase cancellation and the false
        // harmonics/intermodulation produced by sample-wise channel switching.
        for frame in 0..context.num_frames {
            let frame_samples = &input[frame * self.num_channels..(frame + 1) * self.num_channels];
            for (channel, &sample) in frame_samples.iter().enumerate() {
                self.recent_samples[channel * FFT_SIZE + self.recent_write] =
                    if sample.is_finite() { sample } else { 0.0 };
            }
            self.recent_write = (self.recent_write + 1) % FFT_SIZE;
        }
        self.samples_since_analysis = self
            .samples_since_analysis
            .saturating_add(context.num_frames);
        self.samples_since_publish = self
            .samples_since_publish
            .saturating_add(context.num_frames);

        if self.samples_since_analysis >= FFT_SIZE {
            let elapsed_samples = self.samples_since_publish;
            self.samples_since_analysis %= FFT_SIZE;
            self.samples_since_publish = 0;
            // 2/N converts the positive-frequency FFT bin to peak amplitude;
            // periodic Hann has coherent gain 0.5, requiring another factor 2.
            let interior_scale = 4.0 / FFT_SIZE as f32;
            let endpoint_scale = 2.0 / FFT_SIZE as f32;
            let interior_scale_sq = interior_scale * interior_scale;
            let endpoint_scale_sq = endpoint_scale * endpoint_scale;
            self.new_mags.fill(0.0);
            self.fft_line_max_power.fill(0.0);
            let mut new_peak_magnitude = -100.0f32;

            for channel in 0..self.num_channels {
                let first_len = FFT_SIZE - self.recent_write;
                let channel_samples =
                    &self.recent_samples[channel * FFT_SIZE..(channel + 1) * FFT_SIZE];
                crate::simd::window_mul_simd(
                    &mut self.windowed[..first_len],
                    &channel_samples[self.recent_write..],
                    &self.window[..first_len],
                );
                crate::simd::window_mul_simd(
                    &mut self.windowed[first_len..],
                    &channel_samples[..self.recent_write],
                    &self.window[first_len..],
                );
                if let Err(e) = self
                    .fft_r2c
                    .process(&mut self.windowed, &mut self.fft_output)
                {
                    crate::rate_limited_log!(error, 5, "spectrum FFT process failed: {e}");
                    return Ok(context.num_frames);
                }
                for (i, bin) in self.fft_output.iter().enumerate().skip(1) {
                    let scale_sq = if i == FFT_SIZE / 2 {
                        endpoint_scale_sq
                    } else {
                        interior_scale_sq
                    };
                    self.fft_line_max_power[i] =
                        self.fft_line_max_power[i].max(bin.norm_sqr() * scale_sq);
                }
            }

            for (i, &norm_sq) in self.fft_line_max_power.iter().enumerate().skip(1) {
                if let Some(display_bin) = self.bin_to_display.get(i).copied().flatten() {
                    self.new_mags[display_bin] += norm_sq;
                    new_peak_magnitude =
                        new_peak_magnitude.max(10.0 * fast_log10(norm_sq.max(1e-10)));
                }
            }

            for (i, magnitude) in self.new_mags.iter_mut().enumerate() {
                *magnitude = if self.display_bin_has_fft_line[i] {
                    // Integrated band power, corrected for the periodic Hann
                    // window's 1.5-bin equivalent noise bandwidth.
                    10.0 * fast_log10((*magnitude / HANN_ENBW_BINS).max(1e-10))
                } else {
                    f32::NEG_INFINITY
                };
            }

            // The normalized control maps to a 0..1000 ms physical time constant.
            // Coefficients use actual elapsed samples, so decay is independent of
            // callback size and sample rate.
            let tau_seconds = self.config.smoothing;
            let s = if tau_seconds <= 0.0 {
                0.0
            } else {
                (-(elapsed_samples as f32) / (tau_seconds * self.sample_rate as f32)).exp()
            };
            for i in 0..self.config.num_bins {
                self.current_magnitudes[i] = if self.new_mags[i] == f32::NEG_INFINITY {
                    f32::NEG_INFINITY
                } else if !self.current_magnitudes[i].is_finite() {
                    self.new_mags[i]
                } else {
                    s * self.current_magnitudes[i] + (1.0 - s) * self.new_mags[i]
                };
            }

            self.current_peak_magnitude = if self.has_analysis {
                s * self.current_peak_magnitude + (1.0 - s) * new_peak_magnitude
            } else {
                self.has_analysis = true;
                new_peak_magnitude
            };
            let peak = self.current_peak_magnitude;

            // Update cache in-place (real-time safe)
            self.cache.update(|data| {
                data.update_magnitudes(&self.current_magnitudes);
                data.peak_magnitude = peak;
            });
        }
        Ok(context.num_frames)
    }
    fn process_compiled_f32(
        &mut self,
        op: PluginCompiledOp,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Option<Result<usize, String>> {
        if op != PluginCompiledOp::AnalyzerTap {
            return None;
        }
        Some(self.process(input, output, context))
    }
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
    fn take_cache_contention_stats(&mut self) -> (u64, u64) {
        self.cache.take_contention_stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(bin: usize, frames: usize, amplitude: f32) -> Vec<f32> {
        (0..frames)
            .map(|i| {
                amplitude
                    * (2.0 * std::f32::consts::PI * bin as f32 * i as f32 / FFT_SIZE as f32).sin()
            })
            .collect()
    }

    #[test]
    fn rejects_invalid_construction_before_allocating() {
        for config in [
            SpectrumConfig {
                num_bins: 0,
                ..Default::default()
            },
            SpectrumConfig {
                num_bins: usize::MAX,
                ..Default::default()
            },
            SpectrumConfig {
                min_freq: 0.0,
                ..Default::default()
            },
            SpectrumConfig {
                min_freq: f32::NAN,
                ..Default::default()
            },
            SpectrumConfig {
                max_freq: f32::INFINITY,
                ..Default::default()
            },
            SpectrumConfig {
                min_freq: 1_000.0,
                max_freq: 1_000.0,
                ..Default::default()
            },
        ] {
            assert!(SpectrumAnalyzerPlugin::with_config(2, config).is_err());
        }
        assert!(SpectrumAnalyzerPlugin::new(0).is_err());
    }

    #[test]
    fn initialize_rejects_zero_rate_and_range_above_nyquist() {
        let mut plugin = SpectrumAnalyzerPlugin::new(2).unwrap();
        assert!(plugin.initialize(0).is_err());
        assert!(plugin.initialize(32_000).is_err());
    }

    #[test]
    fn antiphase_stereo_tone_does_not_cancel() {
        let mut plugin = SpectrumAnalyzerPlugin::with_config(
            2,
            SpectrumConfig {
                smoothing: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
        plugin.initialize(48_000).unwrap();
        let mono = tone(128, FFT_SIZE, 1.0);
        let mut input = Vec::with_capacity(FFT_SIZE * 2);
        for sample in mono {
            input.extend_from_slice(&[sample, -sample]);
        }
        let mut output = vec![0.0; input.len()];
        plugin
            .process(&input, &mut output, &ProcessContext::new(48_000, FFT_SIZE))
            .unwrap();
        assert!(plugin.cache.load().peak_magnitude > -1.0);
    }

    #[test]
    fn disjoint_channel_tones_are_preserved_without_switching_artifacts() {
        let mut plugin = SpectrumAnalyzerPlugin::with_config(
            2,
            SpectrumConfig {
                num_bins: 100,
                smoothing: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
        plugin.initialize(48_000).unwrap();
        let low = tone(64, FFT_SIZE, 1.0);
        let high = tone(512, FFT_SIZE, 1.0);
        let mut input = Vec::with_capacity(FFT_SIZE * 2);
        for frame in 0..FFT_SIZE {
            input.extend_from_slice(&[low[frame], high[frame]]);
        }
        let mut output = vec![0.0; input.len()];
        plugin
            .process(&input, &mut output, &ProcessContext::new(48_000, FFT_SIZE))
            .unwrap();

        let data = plugin.cache.load();
        let low_display = plugin.bin_to_display[64].unwrap();
        let high_display = plugin.bin_to_display[512].unwrap();
        assert!(data.magnitudes[low_display] > -2.0);
        assert!(data.magnitudes[high_display] > -2.0);
        let mut tone_bands = [false; 100];
        for fft_bin in [62usize, 63, 64, 65, 66, 510, 511, 512, 513, 514] {
            if let Some(display_bin) = plugin.bin_to_display[fft_bin] {
                tone_bands[display_bin] = true;
            }
        }
        let strongest_spurious = data
            .magnitudes
            .iter()
            .enumerate()
            .filter(|(index, value)| !tone_bands[*index] && value.is_finite())
            .map(|(_, value)| *value)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            strongest_spurious < -50.0,
            "unexpected channel-switching component at {strongest_spurious:.2} dBFS"
        );
    }

    #[test]
    fn silent_extra_channels_do_not_attenuate_a_tone() {
        fn measure(channels: usize) -> f32 {
            let mut plugin = SpectrumAnalyzerPlugin::with_config(
                channels,
                SpectrumConfig {
                    smoothing: 0.0,
                    ..Default::default()
                },
            )
            .unwrap();
            plugin.initialize(48_000).unwrap();
            let mono = tone(128, FFT_SIZE, 1.0);
            let mut input = vec![0.0; FFT_SIZE * channels];
            for (frame, sample) in mono.into_iter().enumerate() {
                input[frame * channels] = sample;
            }
            let mut output = vec![0.0; input.len()];
            plugin
                .process(&input, &mut output, &ProcessContext::new(48_000, FFT_SIZE))
                .unwrap();
            plugin.cache.load().peak_magnitude
        }
        assert!((measure(1) - measure(16)).abs() < 0.01);
    }

    #[test]
    fn reset_publishes_clear_data_while_two_prior_generations_are_held() {
        let mut plugin = SpectrumAnalyzerPlugin::with_config(
            1,
            SpectrumConfig {
                smoothing: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
        plugin.initialize(48_000).unwrap();
        let first_generation = plugin.cache.load();
        let input = tone(128, FFT_SIZE, 1.0);
        let mut output = vec![0.0; input.len()];
        plugin
            .process(&input, &mut output, &ProcessContext::new(48_000, FFT_SIZE))
            .unwrap();
        let second_generation = plugin.cache.load();
        assert!(second_generation.peak_magnitude > -1.0);

        plugin.reset();
        let reset_generation = plugin.cache.load();
        assert_eq!(reset_generation.peak_magnitude, -100.0);
        assert!(
            reset_generation
                .magnitudes
                .iter()
                .all(|value| *value == -100.0)
        );
        drop((first_generation, second_generation));
    }

    #[test]
    fn process_requires_initialization_and_sanitizes_non_finite_samples() {
        let mut plugin = SpectrumAnalyzerPlugin::with_config(
            1,
            SpectrumConfig {
                smoothing: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
        let mut input = tone(128, FFT_SIZE, 1.0);
        let mut output = vec![0.0; input.len()];
        assert!(
            plugin
                .process(&input, &mut output, &ProcessContext::new(48_000, FFT_SIZE))
                .is_err()
        );
        plugin.initialize(48_000).unwrap();
        input[0] = f32::NAN;
        input[1] = f32::INFINITY;
        plugin
            .process(&input, &mut output, &ProcessContext::new(48_000, FFT_SIZE))
            .unwrap();
        let data = plugin.cache.load();
        assert!(data.peak_magnitude.is_finite());
        assert!(data.magnitudes.iter().all(|value| !value.is_nan()));
    }

    #[test]
    fn supported_sample_rates_publish_no_nan_and_empty_bands_are_explicit() {
        for sample_rate in [8_000, 32_000, 44_100, 48_000, 96_000, 192_000] {
            let max_freq = (sample_rate as f32 * 0.5).min(20_000.0);
            let mut plugin = SpectrumAnalyzerPlugin::with_config_at_sample_rate(
                1,
                sample_rate,
                SpectrumConfig {
                    num_bins: 120,
                    min_freq: 10.0,
                    max_freq,
                    smoothing: 0.0,
                },
            )
            .unwrap();
            plugin.initialize(sample_rate).unwrap();
            let input = vec![0.0; FFT_SIZE];
            let mut output = input.clone();
            plugin
                .process(
                    &input,
                    &mut output,
                    &ProcessContext::new(sample_rate, FFT_SIZE),
                )
                .unwrap();
            let data = plugin.cache.load();
            assert!(data.frequencies.iter().all(|value| value.is_finite()));
            assert!(data.magnitudes.iter().all(|value| !value.is_nan()));
            assert!(
                data.magnitudes
                    .iter()
                    .any(|value| *value == f32::NEG_INFINITY),
                "expected an explicit empty band at {sample_rate} Hz"
            );
        }
    }

    #[test]
    fn hann_band_power_is_enbw_normalized() {
        let mut plugin = SpectrumAnalyzerPlugin::with_config(
            1,
            SpectrumConfig {
                num_bins: 30,
                smoothing: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
        plugin.initialize(48_000).unwrap();
        let input = tone(128, FFT_SIZE, 1.0);
        let mut output = vec![0.0; input.len()];
        plugin
            .process(&input, &mut output, &ProcessContext::new(48_000, FFT_SIZE))
            .unwrap();
        let display_bin = plugin.bin_to_display[128].unwrap();
        let measured = plugin.cache.load().magnitudes[display_bin];
        assert!(
            measured.abs() < 0.1,
            "band power measured {measured:.2} dBFS"
        );
    }

    #[test]
    fn smoothing_uses_physical_elapsed_time_across_rates_and_callback_sizes() {
        fn decay(sample_rate: u32, chunk: usize) -> f32 {
            let max_freq = (sample_rate as f32 * 0.5).min(20_000.0);
            let mut plugin = SpectrumAnalyzerPlugin::with_config_at_sample_rate(
                1,
                sample_rate,
                SpectrumConfig {
                    smoothing: 0.5,
                    max_freq,
                    ..Default::default()
                },
            )
            .unwrap();
            plugin.initialize(sample_rate).unwrap();
            let input = tone(64, FFT_SIZE, 1.0);
            let mut output = vec![0.0; FFT_SIZE];
            plugin
                .process(
                    &input,
                    &mut output,
                    &ProcessContext::new(sample_rate, FFT_SIZE),
                )
                .unwrap();

            // The selected totals are exactly 1.024 seconds and divisible by
            // both callback sizes, isolating callback partitioning from time.
            let silence_frames = (sample_rate as usize * 1_024) / 1_000;
            let silence = vec![0.0; chunk];
            let mut silence_out = silence.clone();
            for _ in 0..silence_frames / chunk {
                plugin
                    .process(
                        &silence,
                        &mut silence_out,
                        &ProcessContext::new(sample_rate, chunk),
                    )
                    .unwrap();
            }
            plugin.cache.load().peak_magnitude
        }

        let reference = decay(48_000, 64);
        for sample_rate in [32_000, 48_000, 96_000] {
            for chunk in [64, 8_192] {
                let measured = decay(sample_rate, chunk);
                assert!(
                    (measured - reference).abs() < 0.05,
                    "{sample_rate} Hz/{chunk} frames decayed to {measured:.2}, reference {reference:.2}"
                );
            }
        }
    }

    #[test]
    fn large_callback_analyzes_the_latest_window() {
        for frames in [FFT_SIZE, FFT_SIZE * 2, FFT_SIZE * 4, 20_000] {
            let mut plugin = SpectrumAnalyzerPlugin::with_config(
                1,
                SpectrumConfig {
                    num_bins: 100,
                    smoothing: 0.0,
                    ..Default::default()
                },
            )
            .unwrap();
            plugin.initialize(48_000).unwrap();
            let mut input = vec![0.0; frames - FFT_SIZE];
            input.extend(tone(512, FFT_SIZE, 1.0));
            let mut output = vec![0.0; input.len()];
            plugin
                .process(&input, &mut output, &ProcessContext::new(48_000, frames))
                .unwrap();
            let data = plugin.cache.load();
            let peak_index = data
                .magnitudes
                .iter()
                .enumerate()
                .filter(|(_, value)| value.is_finite())
                .max_by(|a, b| a.1.total_cmp(b.1))
                .unwrap()
                .0;
            assert!(
                data.frequencies[peak_index] > 5_000.0,
                "{frames}-frame callback used stale peak at {} Hz",
                data.frequencies[peak_index]
            );
        }

        let mut partial = SpectrumAnalyzerPlugin::new(1).unwrap();
        partial.initialize(48_000).unwrap();
        let input = vec![0.0; FFT_SIZE - 1];
        let mut output = input.clone();
        partial
            .process(
                &input,
                &mut output,
                &ProcessContext::new(48_000, FFT_SIZE - 1),
            )
            .unwrap();
        assert_eq!(partial.cache.load().peak_magnitude, -100.0);
    }

    #[test]
    fn reset_discards_partial_pre_reset_window() {
        let mut plugin = SpectrumAnalyzerPlugin::with_config(
            1,
            SpectrumConfig {
                smoothing: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
        plugin.initialize(48_000).unwrap();
        let input = tone(128, FFT_SIZE - 1, 1.0);
        let mut output = vec![0.0; input.len()];
        plugin
            .process(
                &input,
                &mut output,
                &ProcessContext::new(48_000, FFT_SIZE - 1),
            )
            .unwrap();
        plugin.reset();
        let silence = vec![0.0; FFT_SIZE];
        let mut silence_out = silence.clone();
        plugin
            .process(
                &silence,
                &mut silence_out,
                &ProcessContext::new(48_000, FFT_SIZE),
            )
            .unwrap();
        assert_eq!(plugin.cache.load().peak_magnitude, -100.0);
    }

    #[test]
    fn process_rejects_malformed_buffers() {
        let mut plugin = SpectrumAnalyzerPlugin::new(2).unwrap();
        plugin.initialize(48_000).unwrap();
        let ctx = ProcessContext::new(48_000, 16);
        assert!(plugin.process(&[0.0; 31], &mut [0.0; 31], &ctx).is_err());
        assert!(plugin.process(&[0.0; 32], &mut [0.0; 31], &ctx).is_err());
        assert!(plugin.process(&[0.0; 34], &mut [0.0; 34], &ctx).is_err());
    }

    #[test]
    fn initialized_shape_parameters_require_recreation_but_unchanged_values_are_ok() {
        let mut plugin = SpectrumAnalyzerPlugin::new(2).unwrap();
        plugin.initialize(48_000).unwrap();
        assert!(
            plugin
                .set_parameter(ParameterId::from("num_bins"), ParameterValue::Int(30))
                .is_ok()
        );
        assert!(
            plugin
                .set_parameter(ParameterId::from("num_bins"), ParameterValue::Int(60))
                .is_err()
        );
        assert!(
            plugin
                .set_parameter(ParameterId::from("min_freq"), ParameterValue::Float(40.0))
                .is_err()
        );
    }

    #[test]
    fn bin_centered_full_scale_tone_reads_zero_dbfs() {
        let mut plugin = SpectrumAnalyzerPlugin::with_config(
            1,
            SpectrumConfig {
                num_bins: 100,
                min_freq: 10.0,
                max_freq: 20_000.0,
                smoothing: 0.0,
            },
        )
        .unwrap();
        plugin.initialize(48_000).unwrap();

        let bin = 128usize;
        let input: Vec<f32> = (0..FFT_SIZE)
            .map(|i| (2.0 * std::f32::consts::PI * bin as f32 * i as f32 / FFT_SIZE as f32).sin())
            .collect();
        let mut output = vec![0.0; FFT_SIZE];
        plugin
            .process(&input, &mut output, &ProcessContext::new(48_000, FFT_SIZE))
            .unwrap();

        let peak = plugin.cache.load().peak_magnitude;
        assert!(peak.abs() < 0.1, "full-scale tone measured {peak:.2} dBFS");
    }

    #[test]
    fn silence_cache_peak_remains_negative_floor() {
        let mut plugin = SpectrumAnalyzerPlugin::with_config(
            1,
            SpectrumConfig {
                num_bins: 100,
                min_freq: 10.0,
                max_freq: 20_000.0,
                smoothing: 0.0,
            },
        )
        .unwrap();
        plugin.initialize(48_000).unwrap();

        let input = vec![0.0; FFT_SIZE];
        let mut output = vec![0.0; FFT_SIZE];
        plugin
            .process(&input, &mut output, &ProcessContext::new(48_000, FFT_SIZE))
            .unwrap();

        assert_eq!(plugin.cache.load().peak_magnitude, -100.0);
    }

    #[test]
    fn full_scale_nyquist_tone_reads_zero_dbfs() {
        let sample_rate = 40_000;
        let mut plugin = SpectrumAnalyzerPlugin::with_config(
            1,
            SpectrumConfig {
                num_bins: 100,
                min_freq: 10.0,
                max_freq: 20_000.0,
                smoothing: 0.0,
            },
        )
        .unwrap();
        plugin.initialize(sample_rate).unwrap();

        let input: Vec<f32> = (0..FFT_SIZE)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let mut output = vec![0.0; FFT_SIZE];
        plugin
            .process(
                &input,
                &mut output,
                &ProcessContext::new(sample_rate, FFT_SIZE),
            )
            .unwrap();

        let peak = plugin.cache.load().peak_magnitude;
        assert!(
            peak.abs() < 0.1,
            "full-scale Nyquist tone measured {peak:.2} dBFS"
        );
    }
}
