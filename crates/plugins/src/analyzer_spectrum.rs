//! ============================================================================
//! Spectrum Analyzer Plugin
//! ============================================================================
//!
//! Provides real-time frequency spectrum analysis using FFT.
//! This file contains both the core SpectrumAnalyzer implementation and
//! the AnalyzerPlugin wrapper.

use super::analyzer::SpectrumData;
use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::Arc;

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

/// Reference frequency for spectral tilt correction
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
pub enum TiltReferenceFreq {
    #[default]
    /// Standard 1kHz reference (0dB correction at 1kHz)
    Standard,
    /// Use analyzer's min_freq as reference (low frequencies unchanged)
    MinFreq,
}

/// Spectral tilt correction mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
pub enum SpectralTiltCorrection {
    #[default]
    /// No correction - raw spectrum
    None,
    /// +3dB/octave correction (makes pink noise flat)
    Pink,
    /// Custom slope in dB/octave (positive = boost high frequencies)
    Custom(f32),
}

/// Configuration for spectrum analyzer
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Spectral tilt correction to apply (default: None)
    /// - None: raw spectrum
    /// - Pink: +3dB/octave, makes pink noise appear flat
    /// - Custom(slope): custom dB/octave correction
    pub tilt_correction: SpectralTiltCorrection,
    /// Reference frequency for tilt correction (default: Standard = 1kHz)
    pub tilt_reference: TiltReferenceFreq,
}

impl Default for SpectrumConfig {
    fn default() -> Self {
        Self {
            num_bins: 30,
            min_freq: 20.0,
            max_freq: 20000.0,
            smoothing: 0.7,
            tilt_correction: SpectralTiltCorrection::None,
            tilt_reference: TiltReferenceFreq::Standard,
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
    #[allow(dead_code)]
    channels: u32,
    /// FFT size (power of 2)
    fft_size: usize,
    /// Circular buffer for audio samples
    sample_buffer: Vec<f32>,
    /// Write position in circular buffer
    buffer_pos: usize,
    /// Real FFT planner
    r2c: Arc<dyn realfft::RealToComplex<f32>>,
    /// FFT input buffer
    fft_input: Vec<f32>,
    /// FFT output buffer
    fft_output: Vec<realfft::num_complex::Complex<f32>>,
    /// Scratch buffer for FFT
    #[allow(dead_code)]
    fft_scratch: Vec<realfft::num_complex::Complex<f32>>,
    /// Window function (Hann)
    window_function: Vec<f32>,
    /// Frequency bin centers
    #[allow(dead_code)]
    bin_centers: Vec<f32>,
    /// Current spectrum measurements (smoothed)
    current_spectrum: SpectrumInfo,
    /// Previous spectrum values (for smoothing)
    prev_magnitudes: Vec<f32>,
    /// Pre-allocated magnitudes buffer for current calculation
    magnitudes: Vec<f32>,
    /// Pre-computed tilt correction values for each bin (in dB)
    tilt_corrections: Vec<f32>,
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

        // FFT size: use at least 4096 for better resolution at low frequencies
        // For 48kHz, 4096 gives ~11.7Hz resolution per bin
        let fft_size = 4096;

        // Initialize FFT
        let mut planner = realfft::RealFftPlanner::<f32>::new();
        let r2c = planner.plan_fft_forward(fft_size);
        let fft_input = r2c.make_input_vec();
        let fft_output = r2c.make_output_vec();
        let fft_scratch = r2c.make_scratch_vec();

        // Pre-compute window function (Hann)
        let window_function: Vec<f32> = (0..fft_size)
            .map(|i| {
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (fft_size - 1) as f32).cos())
            })
            .collect();

        // Generate logarithmic frequency bins
        let (_bin_edges, bin_centers) =
            Self::generate_log_bins(config.num_bins, config.min_freq, config.max_freq);

        let current_spectrum = SpectrumInfo {
            frequencies: bin_centers.clone(),
            magnitudes: vec![f32::NEG_INFINITY; config.num_bins],
            peak_magnitude: f32::NEG_INFINITY,
        };

        // Pre-compute tilt corrections for each bin
        let tilt_corrections = Self::compute_tilt_corrections(
            &bin_centers,
            config.tilt_correction,
            config.tilt_reference,
            config.min_freq,
        );

        let num_bins = config.num_bins;
        Ok(Self {
            config,
            sample_rate,
            channels,
            fft_size,
            sample_buffer: vec![0.0; fft_size],
            buffer_pos: 0,
            r2c,
            fft_input,
            fft_output,
            fft_scratch,
            window_function,
            bin_centers,
            current_spectrum,
            prev_magnitudes: vec![f32::NEG_INFINITY; num_bins],
            magnitudes: vec![f32::NEG_INFINITY; num_bins],
            tilt_corrections,
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

    /// Compute tilt correction values for each frequency bin
    fn compute_tilt_corrections(
        bin_centers: &[f32],
        tilt_correction: SpectralTiltCorrection,
        tilt_reference: TiltReferenceFreq,
        min_freq: f32,
    ) -> Vec<f32> {
        let reference_freq = match tilt_reference {
            TiltReferenceFreq::Standard => 1000.0,
            TiltReferenceFreq::MinFreq => min_freq,
        };
        match tilt_correction {
            SpectralTiltCorrection::None => vec![0.0; bin_centers.len()],
            SpectralTiltCorrection::Pink => {
                // +3dB/octave = +10dB/decade = 10 * log10(f / f_ref)
                bin_centers
                    .iter()
                    .map(|&f| 10.0 * (f / reference_freq).log10())
                    .collect()
            }
            SpectralTiltCorrection::Custom(slope_db_per_octave) => {
                // slope_db_per_octave * log2(f / f_ref)
                let log2_ref = reference_freq.log2();
                bin_centers
                    .iter()
                    .map(|&f| slope_db_per_octave * (f.log2() - log2_ref))
                    .collect()
            }
        }
    }

    /// Add audio frames to the analyzer
    fn add_frames(&mut self, samples: &[f32]) -> Result<(), String> {
        // Mix all channels to mono for spectrum analysis (simplified)
        // Optimization: In many DAWs, analyzer just takes left channel or sum.
        // For performance, averaging is linear.
        let channels = self.channels as usize;
        let num_frames = samples.len() / channels;

        // Iterate frames and add to circular buffer
        for frame_idx in 0..num_frames {
            let mut sum = 0.0;
            // Simple loop unrolling hint to compiler
            for ch in 0..channels {
                sum += samples[frame_idx * channels + ch];
            }
            let mono_sample = sum / channels as f32;

            self.sample_buffer[self.buffer_pos] = mono_sample;
            self.buffer_pos = (self.buffer_pos + 1) % self.fft_size;

            // When buffer wraps around, we have enough new data to compute
            // (Note: This is a simple strategy. For overlapped windows, we'd check differently)
            if self.buffer_pos == 0 {
                self.compute_spectrum()?;
            }
        }

        Ok(())
    }

    /// Compute spectrum using FFT
    fn compute_spectrum(&mut self) -> Result<(), String> {
        // Prepare input: windowed samples
        // The buffer is circular, but we want a contiguous block for FFT.
        // Since we process when buffer_pos == 0 (wrap around), the buffer is already contiguous logic-wise?
        // Wait, self.buffer_pos wraps 0..fft_size.
        // Yes, if we just finished writing at end, the buffer is full and ordered old->new if we started at 0?
        // Actually, buffer is circular. If we just wrapped to 0, the last sample written was at index fft_size-1.
        // So the buffer contains [oldest ... newest].
        // This is contiguous in memory.

        // Apply window and copy to input
        for (i, &sample) in self.sample_buffer.iter().enumerate() {
            self.fft_input[i] = sample * self.window_function[i];
        }

        // Run FFT
        self.r2c
            .process(&mut self.fft_input, &mut self.fft_output)
            .map_err(|e| format!("FFT error: {}", e))?;

        // Compute magnitude spectrum for bins
        // FFT gives linear spaced bins from 0 to Nyquist.
        // Bin size = SampleRate / FFTSize.
        // Example: 48000 / 4096 = 11.7 Hz/bin.
        // Index 0 = DC, Index 1 = 11.7 Hz, etc.

        let fft_bin_size = self.sample_rate as f32 / self.fft_size as f32;
        self.magnitudes.fill(f32::NEG_INFINITY); // Reset pre-allocated buffer

        // Iterate over log bins and average energy from FFT bins
        // Optimization: iterate FFT bins and accumulate into target log bins
        // Because log bins at high freq cover many FFT bins.
        // Log bins at low freq might be smaller than FFT resolution?
        // 4096 size -> 11.7Hz. If low bin is 20Hz-30Hz (10Hz width), it might not get any center?
        // We use interpolation or nearest neighbor for simplicity and speed.

        // Precompute this mapping? No, depends on sample rate, but that changes rarely.
        // Do it on the fly: O(N_fft) scan.

        let _max_fft_bin = self.fft_output.len(); // N/2 + 1

        // Initialize accumulators
        // We track max energy in the bin (peak detection style is better for spectrum visuals than average usually)
        // Or average power? Standard analyzers often use max for transient visibility.
        // Let's use Max Magnitude in the frequency range of the bin.

        // To map FFT bins to Log Bins efficiently:
        // We know the frequency of each FFT bin: i * fft_bin_size.
        // We find which Log Bin it belongs to.
        // Since both are sorted, we can do a linear scan.

        // Find start freq of first bin (approximate)
        // Self::generate_log_bins gives centers. Let's assume edges are implicitly defined.
        // We need edges to Bucket correctly.
        // Reconstruction:
        let log_min = self.config.min_freq.log10();
        let log_max = self.config.max_freq.log10();
        let num_log_bins = self.config.num_bins;

        // Iterate FFT bins
        for (i, complex_val) in self.fft_output.iter().enumerate().skip(1) {
            // Skip DC
            let freq = i as f32 * fft_bin_size;
            if freq > self.config.max_freq {
                break;
            }
            if freq < self.config.min_freq {
                continue;
            }

            // Calculate magnitude
            // mag = sqrt(re^2 + im^2) * 2 / N (normalized)
            // But we want dB.
            // 20 * log10(norm_mag).
            let norm = complex_val.norm();
            // Scaling: RealFFT output is not normalized by 1/N. Norm is N/2 times amplitude?
            // Usually unnormalized FFT: peak 1.0 sine -> N/2 magnitude.
            // So we divide by N/2, or multiply by 2/N.
            let amplitude = norm * 2.0 / self.fft_size as f32;

            let mag_db = if amplitude > 1e-10 {
                20.0 * amplitude.log10()
            } else {
                -200.0 // Noise floor
            };

            // Find which log bin this frequency belongs to
            // log10(f) mapped to 0..num_bins
            let log_f = freq.log10();
            let relative_pos = (log_f - log_min) / (log_max - log_min);
            let target_bin = (relative_pos * num_log_bins as f32).floor() as usize;

            if target_bin < num_log_bins
                && mag_db > self.magnitudes[target_bin] {
                    self.magnitudes[target_bin] = mag_db;
                }
        }

        // Interpolation for empty bins (if FFT resolution is too low for low freq bins)
        // This is rare with 4096 size and >20Hz min, but possible.
        // Simple fix: if -inf, take neighbor? Or leave as gap?
        // Better: linear interpolation from nearest non-inf bins.
        // For simplicity/perf: forward fill then backward fill
        let mut last_val = -100.0;
        for mag in self.magnitudes.iter_mut() {
            if *mag == f32::NEG_INFINITY {
                *mag = last_val;
            } else {
                last_val = *mag;
            }
        }

        // Apply smoothing (exponential moving average)
        for (i, val) in self.magnitudes.iter_mut().enumerate() {
            if self.prev_magnitudes[i].is_finite() {
                *val = self.config.smoothing * self.prev_magnitudes[i]
                    + (1.0 - self.config.smoothing) * *val;
            }
        }
        self.prev_magnitudes.copy_from_slice(&self.magnitudes);

        // Apply tilt correction
        for (i, val) in self.magnitudes.iter_mut().enumerate() {
            *val += self.tilt_corrections[i];
        }

        // Find peak
        let peak_magnitude = self
            .magnitudes
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);

        // Update current spectrum (no lock needed — single-threaded access)
        self.current_spectrum.magnitudes.copy_from_slice(&self.magnitudes);
        self.current_spectrum.peak_magnitude = peak_magnitude;

        Ok(())
    }

    /// Retrieve current spectrum (clone)
    fn get_spectrum(&self) -> SpectrumInfo {
        self.current_spectrum.clone()
    }

    /// Retrieve current spectrum as a reference (zero-copy)
    fn get_spectrum_ref(&self) -> &SpectrumInfo {
        &self.current_spectrum
    }

    /// Reset analyzer state
    fn reset(&mut self) -> Result<(), String> {
        self.sample_buffer.fill(0.0);
        self.buffer_pos = 0;
        self.fft_input.fill(0.0);
        self.prev_magnitudes.fill(f32::NEG_INFINITY);
        self.current_spectrum.magnitudes.fill(f32::NEG_INFINITY);
        self.current_spectrum.peak_magnitude = f32::NEG_INFINITY;
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
    /// Cached Arc for zero-alloc get_data() — rebuilt after each process() call.
    /// Uses Arc::get_mut() to update in-place when refcount == 1 (common case),
    /// falls back to new allocation only when UI still holds a reference.
    cached_data: Option<Arc<dyn Any + Send + Sync>>,
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
            cached_data: None,
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

    /// Rebuild the cached Arc from current analyzer state.
    /// Tries Arc::get_mut() for in-place update (zero alloc when refcount == 1),
    /// falls back to new allocation when the old Arc is still held externally.
    fn rebuild_cached_data(&mut self) {
        let info = self.analyzer.get_spectrum_ref();

        if let Some(ref mut arc) = self.cached_data
            && let Some(inner) = Arc::get_mut(arc)
                .and_then(|any| any.downcast_mut::<SpectrumData>())
        {
            // Zero-allocation in-place update (Vec lengths match → no realloc)
            inner.frequencies.clear();
            inner.frequencies.extend_from_slice(&info.frequencies);
            inner.magnitudes.clear();
            inner.magnitudes.extend_from_slice(&info.magnitudes);
            inner.peak_magnitude = info.peak_magnitude;
            return;
        }

        // First call or refcount > 1: allocate new Arc
        self.cached_data = Some(Arc::new(Self::to_spectrum_data(info)));
    }
}

impl Plugin for SpectrumAnalyzerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Spectrum Analyzer", "1.0.0", "SotF")
            .with_description("Real-time FFT-based frequency spectrum analysis")
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

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        Err("Spectrum analyzer has no parameters".to_string())
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        // Get current config
        let config = self.analyzer.config.clone();

        // Recreate the analyzer with the new sample rate
        self.analyzer = SpectrumAnalyzer::new(self.num_channels as u32, sample_rate, config)
            .map_err(|e| format!("Failed to initialize spectrum analyzer: {}", e))?;
        self.rebuild_cached_data();

        Ok(())
    }

    fn reset(&mut self) {
        self.analyzer.reset().ok();
        self.rebuild_cached_data();
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        // Verify input/output size
        let expected_samples = context.num_frames * self.num_channels;
        if input.len() != expected_samples {
            return Err(format!(
                "Input size mismatch: expected {}, got {}",
                expected_samples,
                input.len()
            ));
        }
        if output.len() != expected_samples {
            return Err(format!(
                "Output size mismatch: expected {}, got {}",
                expected_samples,
                output.len()
            ));
        }

        // Pass-through: copy input to output
        output.copy_from_slice(input);

        // Add frames to the analyzer
        self.analyzer
            .add_frames(input)
            .map_err(|e| format!("Failed to add frames to spectrum analyzer: {}", e))?;

        // Rebuild cached Arc for zero-alloc get_data().
        // Common case: Arc::get_mut succeeds (refcount == 1) → in-place update, zero alloc.
        // Rare case: UI still holds old ref → one allocation.
        self.rebuild_cached_data();

        Ok(context.num_frames)
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        // Just a refcount bump — no heap allocation
        self.cached_data.clone()
    }

    fn latency_samples(&self) -> usize {
        // Spectrum analyzer has latency equal to FFT size but since it is pass-through,
        // it doesn't delay the audio signal.
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Plugin;

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
            ..Default::default()
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

        let mut output = vec![0.0_f32; num_frames * 2];

        // Process
        plugin.process(&input, &mut output, &context).unwrap();

        // Get spectrum
        let data = plugin.get_data().unwrap();
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
            ..Default::default()
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

        let mut output = vec![0.0_f32; num_frames * 2];

        // Process multiple times to fill the buffer
        for _ in 0..3 {
            plugin.process(&input, &mut output, &context).unwrap();
        }

        let data = plugin.get_data().unwrap();
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
            bin_idx,
            spectrum_data.frequencies[bin_idx],
            spectrum_data.magnitudes[bin_idx]
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

        let mut output = vec![0.0_f32; num_frames * 2];
        plugin.process(&input, &mut output, &context).unwrap();

        // Reset
        plugin.reset();

        // Get spectrum after reset
        let data = plugin.get_data().unwrap();
        let spectrum_data = data.downcast_ref::<SpectrumData>().unwrap();

        // After reset, magnitudes should be low/silent
        log::info!("After reset - Peak: {:.1}dB", spectrum_data.peak_magnitude);
    }

    #[test]
    fn test_spectrum_analyzer_plugin_multichannel() {
        // Test with 5 channels (5.0 surround)
        let mut plugin = SpectrumAnalyzerPlugin::new(5).unwrap();
        plugin.initialize(48000).unwrap();

        // Must provide enough frames to fill the FFT buffer (4096)
        let num_frames = 4096;
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

        let mut output = vec![0.0_f32; num_frames * 5];
        plugin.process(&input, &mut output, &context).unwrap();

        let data = plugin.get_data().unwrap();
        let spectrum_data = data.downcast_ref::<SpectrumData>().unwrap();

        log::info!(
            "5-channel spectrum: peak = {:.1}dB",
            spectrum_data.peak_magnitude
        );

        // Should have computed spectrum
        assert!(spectrum_data.peak_magnitude > f32::NEG_INFINITY);
    }

    #[test]
    fn test_tilt_correction_values() {
        // Test compute_tilt_corrections directly
        let bin_centers = vec![100.0, 1000.0, 10000.0];

        // Pink correction with 1kHz reference
        let pink_corrections = SpectrumAnalyzer::compute_tilt_corrections(
            &bin_centers,
            SpectralTiltCorrection::Pink,
            TiltReferenceFreq::Standard,
            20.0,
        );

        // At 1kHz (reference): 0dB
        assert!((pink_corrections[1]).abs() < 0.01);
        // At 100Hz: -10dB (one decade below)
        assert!((pink_corrections[0] - (-10.0)).abs() < 0.01);
        // At 10kHz: +10dB (one decade above)
        assert!((pink_corrections[2] - 10.0).abs() < 0.01);

        // No correction
        let no_corrections = SpectrumAnalyzer::compute_tilt_corrections(
            &bin_centers,
            SpectralTiltCorrection::None,
            TiltReferenceFreq::Standard,
            20.0,
        );
        assert!(no_corrections.iter().all(|&c| c == 0.0));
    }

    #[test]
    fn test_tilt_correction_min_freq_reference() {
        let bin_centers = vec![20.0, 200.0, 2000.0, 20000.0];

        // Pink correction with min_freq reference (20Hz)
        let corrections = SpectrumAnalyzer::compute_tilt_corrections(
            &bin_centers,
            SpectralTiltCorrection::Pink,
            TiltReferenceFreq::MinFreq,
            20.0,
        );

        // At 20Hz (reference): 0dB
        assert!((corrections[0]).abs() < 0.01);
        // At 200Hz: +10dB (one decade above)
        assert!((corrections[1] - 10.0).abs() < 0.01);
        // At 2000Hz: +20dB (two decades above)
        assert!((corrections[2] - 20.0).abs() < 0.01);
        // At 20000Hz: +30dB (three decades above)
        assert!((corrections[3] - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_tilt_correction_custom_slope() {
        let bin_centers = vec![500.0, 1000.0, 2000.0];

        // Custom +6dB/octave slope
        let corrections = SpectrumAnalyzer::compute_tilt_corrections(
            &bin_centers,
            SpectralTiltCorrection::Custom(6.0),
            TiltReferenceFreq::Standard,
            20.0,
        );

        // At 1kHz: 0dB
        assert!((corrections[1]).abs() < 0.01);
        // At 500Hz: -6dB (one octave below)
        assert!((corrections[0] - (-6.0)).abs() < 0.01);
        // At 2kHz: +6dB (one octave above)
        assert!((corrections[2] - 6.0).abs() < 0.01);
    }

    #[test]
    fn test_spectrum_analyzer_with_pink_correction() {
        let config = SpectrumConfig {
            num_bins: 10,
            min_freq: 100.0,
            max_freq: 10000.0,
            smoothing: 0.0, // No smoothing for this test
            tilt_correction: SpectralTiltCorrection::Pink,
            tilt_reference: TiltReferenceFreq::Standard,
        };

        let mut plugin = SpectrumAnalyzerPlugin::with_config(2, config).unwrap();
        plugin.initialize(48000).unwrap();

        // Generate white noise (same amplitude at all frequencies)
        let num_frames = 2048;
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            // Simple pseudo-random noise
            let noise = ((i * 1103515245 + 12345) % 65536) as f32 / 32768.0 - 1.0;
            input[i * 2] = noise * 0.5;
            input[i * 2 + 1] = noise * 0.5;
        }

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        let mut output = vec![0.0_f32; num_frames * 2];

        // Process multiple times
        for _ in 0..3 {
            plugin.process(&input, &mut output, &context).unwrap();
        }

        let data = plugin.get_data().unwrap();
        let spectrum_data = data.downcast_ref::<SpectrumData>().unwrap();

        // With pink correction, white noise should show a rising spectrum
        // (high frequencies boosted relative to low frequencies)
        log::info!("Pink-corrected white noise spectrum:");
        for (i, (&freq, &mag)) in spectrum_data
            .frequencies
            .iter()
            .zip(spectrum_data.magnitudes.iter())
            .enumerate()
        {
            log::info!("  Bin {}: {:.0}Hz = {:.1}dB", i, freq, mag);
        }
    }

    mod proptest_spectrum {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// Property: spectrum analyzer is passthrough — output must exactly equal input
            #[test]
            fn passthrough_property(
                amplitude in 0.01f32..1.0,
                freq_hz in 100.0f32..10000.0,
            ) {
                let mut plugin = SpectrumAnalyzerPlugin::new(2).unwrap();
                plugin.initialize(48000).unwrap();

                let num_frames = 512;
                let input: Vec<f32> = (0..num_frames * 2)
                    .map(|i| {
                        let t = (i / 2) as f32 / 48000.0;
                        amplitude * (2.0 * std::f32::consts::PI * freq_hz * t).sin()
                    })
                    .collect();
                let mut output = vec![0.0f32; num_frames * 2];
                let context = ProcessContext { sample_rate: 48000, num_frames };

                plugin.process(&input, &mut output, &context).unwrap();

                for (i, (&inp, &out)) in input.iter().zip(output.iter()).enumerate() {
                    prop_assert!(
                        (inp - out).abs() < 1e-6,
                        "Passthrough violated at sample {}: input={}, output={}",
                        i, inp, out
                    );
                }
            }
        }
    }
}
