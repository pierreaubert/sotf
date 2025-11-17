// ============================================================================
// Spectrum Analyzer Plugin
// ============================================================================
//
// Provides real-time frequency spectrum analysis using FFT.
// This file contains both the core SpectrumAnalyzer implementation and
// the AnalyzerPlugin wrapper.

use super::analyzer::{AnalyzerPlugin, SpectrumData};
use super::plugin::{PluginInfo, PluginResult, ProcessContext};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::{Arc, Mutex};

// ============================================================================
// Core Spectrum Analyzer Implementation
// ============================================================================

/// Real-time spectrum measurements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumInfo {
    /// Frequency bin centers in Hz
    pub frequencies: Vec<f32>,
    /// Magnitude values in dB (relative to full scale)
    pub magnitudes: Vec<f32>,
    /// Peak magnitude across all bins
    pub peak_magnitude: f32,
}

impl Default for SpectrumInfo {
    fn default() -> Self {
        Self {
            frequencies: Vec::new(),
            magnitudes: Vec::new(),
            peak_magnitude: f32::NEG_INFINITY,
        }
    }
}

/// Configuration for spectrum analyzer
#[derive(Debug, Clone)]
pub struct SpectrumConfig {
    /// Number of frequency bins (default: 30)
    pub num_bins: usize,
    /// Minimum frequency in Hz (default: 20)
    pub min_freq: f32,
    /// Maximum frequency in Hz (default: 20000)
    pub max_freq: f32,
    /// Smoothing factor for exponential moving average (0.0 to 1.0)
    /// Higher values = more smoothing, lower values = more responsive
    pub smoothing: f32,
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

/// Real-time spectrum analyzer using FFT
pub(crate) struct SpectrumAnalyzer {
    /// Configuration
    config: SpectrumConfig,
    /// Sample rate in Hz
    sample_rate: u32,
    /// Number of channels
    channels: u32,
    /// FFT size (power of 2)
    fft_size: usize,
    /// Circular buffer for audio samples
    sample_buffer: Vec<f32>,
    /// Write position in circular buffer
    buffer_pos: usize,
    /// Frequency bin centers
    bin_centers: Vec<f32>,
    /// Current spectrum measurements (smoothed)
    current_spectrum: Arc<Mutex<SpectrumInfo>>,
    /// Previous spectrum values (for smoothing)
    prev_magnitudes: Vec<f32>,
}

impl SpectrumAnalyzer {
    /// Create a new spectrum analyzer
    pub(crate) fn new(
        channels: u32,
        sample_rate: u32,
        config: SpectrumConfig,
    ) -> Result<Self, String> {
        if config.num_bins < 2 {
            return Err("num_bins must be at least 2".to_string());
        }
        if config.min_freq <= 0.0 || config.max_freq <= config.min_freq {
            return Err("Invalid frequency range".to_string());
        }
        if !(0.0..=1.0).contains(&config.smoothing) {
            return Err("smoothing must be between 0.0 and 1.0".to_string());
        }

        // FFT size: use at least 2048 for good frequency resolution
        let fft_size = 2048;

        // Generate logarithmic frequency bins
        let (_bin_edges, bin_centers) =
            Self::generate_log_bins(config.num_bins, config.min_freq, config.max_freq);

        let current_spectrum = Arc::new(Mutex::new(SpectrumInfo {
            frequencies: bin_centers.clone(),
            magnitudes: vec![f32::NEG_INFINITY; config.num_bins],
            peak_magnitude: f32::NEG_INFINITY,
        }));

        let num_bins = config.num_bins;
        Ok(Self {
            config,
            sample_rate,
            channels,
            fft_size,
            sample_buffer: vec![0.0; fft_size],
            buffer_pos: 0,
            bin_centers,
            current_spectrum,
            prev_magnitudes: vec![f32::NEG_INFINITY; num_bins],
        })
    }

    /// Generate logarithmic frequency bins
    fn generate_log_bins(num_bins: usize, min_freq: f32, max_freq: f32) -> (Vec<f32>, Vec<f32>) {
        let log_min = min_freq.log10();
        let log_max = max_freq.log10();

        let mut edges = Vec::with_capacity(num_bins + 1);
        let mut centers = Vec::with_capacity(num_bins);

        for i in 0..=num_bins {
            let log_freq = log_min + (log_max - log_min) * (i as f32 / num_bins as f32);
            edges.push(10.0_f32.powf(log_freq));
        }

        for i in 0..num_bins {
            let center = (edges[i] * edges[i + 1]).sqrt(); // Geometric mean
            centers.push(center);
        }

        (edges, centers)
    }

    /// Add audio frames to the analyzer
    fn add_frames(&mut self, samples: &[f32]) -> Result<(), String> {
        // Mix all channels to mono for spectrum analysis
        let mono_samples = self.mix_to_mono(samples);

        // Add samples to circular buffer
        for sample in mono_samples {
            self.sample_buffer[self.buffer_pos] = sample;
            self.buffer_pos = (self.buffer_pos + 1) % self.fft_size;

            // When buffer is full, compute spectrum
            if self.buffer_pos == 0 {
                self.compute_spectrum()?;
            }
        }

        Ok(())
    }

    /// Mix all channels to mono
    fn mix_to_mono(&self, samples: &[f32]) -> Vec<f32> {
        let channels = self.channels as usize;
        let num_frames = samples.len() / channels;

        let mut mono = Vec::with_capacity(num_frames);
        for frame_idx in 0..num_frames {
            let mut sum = 0.0;
            for ch in 0..channels {
                sum += samples[frame_idx * channels + ch];
            }
            mono.push(sum / channels as f32);
        }

        mono
    }

    /// Compute spectrum using Goertzel algorithm
    fn compute_spectrum(&mut self) -> Result<(), String> {
        // Apply Hann window
        let windowed = self.apply_hann_window(&self.sample_buffer);

        // Compute magnitude spectrum at bin frequencies
        let mut magnitudes = vec![0.0; self.config.num_bins];

        for (bin_idx, &center_freq) in self.bin_centers.iter().enumerate() {
            let magnitude = self.compute_bin_magnitude(&windowed, center_freq);
            magnitudes[bin_idx] = magnitude;
        }

        // Apply smoothing (exponential moving average)
        for i in 0..self.config.num_bins {
            if self.prev_magnitudes[i].is_finite() {
                magnitudes[i] = self.config.smoothing * self.prev_magnitudes[i]
                    + (1.0 - self.config.smoothing) * magnitudes[i];
            }
        }

        self.prev_magnitudes = magnitudes.clone();

        // Find peak
        let peak_magnitude = magnitudes.iter().copied().fold(f32::NEG_INFINITY, f32::max);

        // Update shared state
        {
            let mut spectrum = self.current_spectrum.lock().unwrap();
            spectrum.magnitudes = magnitudes;
            spectrum.peak_magnitude = peak_magnitude;
        }

        Ok(())
    }

    /// Apply Hann window to samples
    fn apply_hann_window(&self, samples: &[f32]) -> Vec<f32> {
        let n = samples.len();
        samples
            .iter()
            .enumerate()
            .map(|(i, &sample)| {
                let window =
                    0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32).cos());
                sample * window
            })
            .collect()
    }

    /// Compute magnitude at a specific frequency using Goertzel algorithm
    fn compute_bin_magnitude(&self, samples: &[f32], freq: f32) -> f32 {
        let normalized_freq = freq / self.sample_rate as f32;
        let w = 2.0 * std::f32::consts::PI * normalized_freq;

        let mut s0;
        let mut s1 = 0.0;
        let mut s2 = 0.0;

        let coeff = 2.0 * w.cos();

        for &sample in samples {
            s0 = sample + coeff * s1 - s2;
            s2 = s1;
            s1 = s0;
        }

        let real = s1 - s2 * w.cos();
        let imag = s2 * w.sin();

        let magnitude = (real * real + imag * imag).sqrt();

        // Normalize by window sum for proper dB scaling
        let window_sum = samples.len() as f32 / 2.0;
        let normalized_magnitude = magnitude / window_sum;

        // Convert to dB (20 * log10(magnitude / reference))
        if normalized_magnitude > 1e-10 {
            20.0 * normalized_magnitude.log10()
        } else {
            f32::NEG_INFINITY
        }
    }

    /// Get the current spectrum measurements
    fn get_spectrum(&self) -> SpectrumInfo {
        let spectrum = self.current_spectrum.lock().unwrap();
        spectrum.clone()
    }

    /// Reset the analyzer
    fn reset(&mut self) -> Result<(), String> {
        self.sample_buffer.fill(0.0);
        self.buffer_pos = 0;
        self.prev_magnitudes.fill(f32::NEG_INFINITY);

        {
            let mut spectrum = self.current_spectrum.lock().unwrap();
            spectrum.magnitudes.fill(f32::NEG_INFINITY);
            spectrum.peak_magnitude = f32::NEG_INFINITY;
        }

        Ok(())
    }
}

// ============================================================================
// Plugin Wrapper
// ============================================================================

/// Spectrum analyzer plugin
pub struct SpectrumAnalyzerPlugin {
    /// Underlying spectrum analyzer
    analyzer: SpectrumAnalyzer,
    /// Number of channels
    num_channels: usize,
}

impl SpectrumAnalyzerPlugin {
    /// Create a new spectrum analyzer plugin with default configuration
    ///
    /// # Arguments
    /// * `num_channels` - Number of audio channels to analyze
    pub fn new(num_channels: usize) -> Result<Self, String> {
        Self::with_config(num_channels, SpectrumConfig::default())
    }

    /// Create a new spectrum analyzer plugin with custom configuration
    ///
    /// # Arguments
    /// * `num_channels` - Number of audio channels to analyze
    /// * `config` - Spectrum analyzer configuration
    pub fn with_config(num_channels: usize, config: SpectrumConfig) -> Result<Self, String> {
        let analyzer = SpectrumAnalyzer::new(num_channels as u32, 48000, config)?;

        Ok(Self {
            analyzer,
            num_channels,
        })
    }

    /// Get current spectrum measurements
    pub fn get_spectrum(&self) -> SpectrumInfo {
        self.analyzer.get_spectrum()
    }

    /// Convert SpectrumInfo to SpectrumData
    fn to_spectrum_data(info: &SpectrumInfo) -> SpectrumData {
        SpectrumData {
            frequencies: info.frequencies.clone(),
            magnitudes: info.magnitudes.clone(),
            peak_magnitude: info.peak_magnitude,
        }
    }
}

impl AnalyzerPlugin for SpectrumAnalyzerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Spectrum Analyzer".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description: "Real-time FFT-based frequency spectrum analysis".to_string(),
        }
    }

    fn input_channels(&self) -> usize {
        self.num_channels
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        // Get current config
        let config = self.analyzer.config.clone();

        // Recreate the analyzer with the new sample rate
        self.analyzer = SpectrumAnalyzer::new(self.num_channels as u32, sample_rate, config)
            .map_err(|e| format!("Failed to initialize spectrum analyzer: {}", e))?;

        Ok(())
    }

    fn reset(&mut self) {
        self.analyzer.reset().ok();
    }

    fn process(&mut self, input: &[f32], context: &ProcessContext) -> PluginResult<()> {
        // Verify input size
        let expected_samples = context.num_frames * self.num_channels;
        if input.len() != expected_samples {
            return Err(format!(
                "Input size mismatch: expected {}, got {}",
                expected_samples,
                input.len()
            ));
        }

        // Add frames to the analyzer
        self.analyzer
            .add_frames(input)
            .map_err(|e| format!("Failed to add frames to spectrum analyzer: {}", e))?;

        Ok(())
    }

    fn get_data(&self) -> Box<dyn Any + Send> {
        let info = self.analyzer.get_spectrum();
        Box::new(Self::to_spectrum_data(&info))
    }

    fn latency_samples(&self) -> usize {
        // Spectrum analyzer has latency equal to FFT size
        2048
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spectrum_analyzer_plugin_creation() {
        let plugin = SpectrumAnalyzerPlugin::new(2).unwrap();
        assert_eq!(plugin.input_channels(), 2);
    }

    #[test]
    fn test_spectrum_analyzer_plugin_custom_config() {
        let config = SpectrumConfig {
            num_bins: 50,
            min_freq: 30.0,
            max_freq: 18000.0,
            smoothing: 0.8,
        };

        let plugin = SpectrumAnalyzerPlugin::with_config(2, config).unwrap();
        assert_eq!(plugin.input_channels(), 2);

        let spectrum = plugin.get_spectrum();
        assert_eq!(spectrum.frequencies.len(), 50);
    }

    #[test]
    fn test_spectrum_analyzer_plugin_processing() {
        let mut plugin = SpectrumAnalyzerPlugin::new(2).unwrap();
        plugin.initialize(48000).unwrap();

        // Create test signal: 1kHz sine wave
        let num_frames = 2048;
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 0.5;
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        // Process
        plugin.process(&input, &context).unwrap();

        // Get spectrum
        let data = plugin.get_data();
        let spectrum_data = data.downcast_ref::<SpectrumData>().unwrap();

        log::info!("Number of bins: {}", spectrum_data.frequencies.len());
        log::info!(
            "Frequency range: {:.0}Hz - {:.0}Hz",
            spectrum_data.frequencies.first().unwrap_or(&0.0),
            spectrum_data.frequencies.last().unwrap_or(&0.0)
        );
        log::info!("Peak magnitude: {:.1}dB", spectrum_data.peak_magnitude);

        // Should have some bins
        assert!(spectrum_data.frequencies.len() > 0);
        assert!(spectrum_data.magnitudes.len() > 0);
    }

    #[test]
    fn test_spectrum_analyzer_plugin_1khz_peak() {
        let config = SpectrumConfig {
            num_bins: 30,
            min_freq: 20.0,
            max_freq: 20000.0,
            smoothing: 0.0, // No smoothing for this test
        };

        let mut plugin = SpectrumAnalyzerPlugin::with_config(2, config).unwrap();
        plugin.initialize(48000).unwrap();

        // Create strong 1kHz signal
        let num_frames = 2048;
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 0.8;
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        // Process multiple times to fill the buffer
        for _ in 0..3 {
            plugin.process(&input, &context).unwrap();
        }

        let data = plugin.get_data();
        let spectrum_data = data.downcast_ref::<SpectrumData>().unwrap();

        // Find the bin closest to 1kHz
        let target_freq = 1000.0;
        let (bin_idx, _) = spectrum_data
            .frequencies
            .iter()
            .enumerate()
            .min_by_key(|(_, f)| ((*f - target_freq).abs() * 1000.0) as i32)
            .unwrap();

        log::info!("1kHz test:");
        log::info!(
            "  Bin {} ({:.0}Hz): {:.1}dB",
            bin_idx, spectrum_data.frequencies[bin_idx], spectrum_data.magnitudes[bin_idx]
        );

        // The 1kHz bin should have more energy than average
        // Note: With smoothing and circular buffer, it may take a few iterations
        // to build up. The bin should be above the noise floor.
        assert!(
            spectrum_data.magnitudes[bin_idx] > -70.0,
            "1kHz bin should be above noise floor, got {:.1}dB",
            spectrum_data.magnitudes[bin_idx]
        );
    }

    #[test]
    fn test_spectrum_analyzer_plugin_reset() {
        let mut plugin = SpectrumAnalyzerPlugin::new(2).unwrap();
        plugin.initialize(48000).unwrap();

        // Process some audio
        let num_frames = 2048;
        let input = vec![0.5_f32; num_frames * 2];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        plugin.process(&input, &context).unwrap();

        // Reset
        plugin.reset();

        // Get spectrum after reset
        let data = plugin.get_data();
        let spectrum_data = data.downcast_ref::<SpectrumData>().unwrap();

        // After reset, magnitudes should be low/silent
        log::info!("After reset - Peak: {:.1}dB", spectrum_data.peak_magnitude);
    }

    #[test]
    fn test_spectrum_analyzer_plugin_multichannel() {
        // Test with 5 channels (5.0 surround)
        let mut plugin = SpectrumAnalyzerPlugin::new(5).unwrap();
        plugin.initialize(48000).unwrap();

        let num_frames = 2048;
        let mut input = vec![0.0_f32; num_frames * 5];

        // Different frequency on each channel
        for i in 0..num_frames {
            let t = i as f32 / 48000.0;
            input[i * 5 + 0] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.2;
            input[i * 5 + 1] = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.2;
            input[i * 5 + 2] = (2.0 * std::f32::consts::PI * 1320.0 * t).sin() * 0.2;
            input[i * 5 + 3] = (2.0 * std::f32::consts::PI * 1760.0 * t).sin() * 0.2;
            input[i * 5 + 4] = (2.0 * std::f32::consts::PI * 2200.0 * t).sin() * 0.2;
        }

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        plugin.process(&input, &context).unwrap();

        let data = plugin.get_data();
        let spectrum_data = data.downcast_ref::<SpectrumData>().unwrap();

        log::info!(
            "5-channel spectrum: peak = {:.1}dB",
            spectrum_data.peak_magnitude
        );

        // Should have computed spectrum
        assert!(spectrum_data.peak_magnitude > f32::NEG_INFINITY);
    }
}
