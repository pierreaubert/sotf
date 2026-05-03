//! FFT-based frequency analysis for recorded signals
//!
//! This module provides functions to analyze recorded audio signals and extract:
//! - Frequency spectrum (magnitude in dBFS)
//! - Phase spectrum (compensated for latency)
//! - Latency estimation via cross-correlation
//! - Microphone compensation for calibrated measurements
//! - Standalone WAV buffer analysis (wav2csv functionality)

use hound::WavReader;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use std::cell::RefCell;
use std::f32::consts::PI;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

/// Spectrum result: (frequencies, magnitudes_db, phases_deg)
type SpectrumResult = Result<(Vec<f32>, Vec<f32>, Vec<f32>), String>;

thread_local! {
    static FFT_PLANNER: RefCell<FftPlanner<f32>> = RefCell::new(FftPlanner::new());
}

/// Get a cached forward FFT plan for the given size (f32).
///
/// Uses a thread-local planner so repeated calls with the same size
/// return the same plan without recomputing twiddle factors.
pub fn plan_fft_forward(size: usize) -> Arc<dyn rustfft::Fft<f32>> {
    FFT_PLANNER.with(|p| p.borrow_mut().plan_fft_forward(size))
}

/// Get a cached inverse FFT plan for the given size (f32).
pub fn plan_fft_inverse(size: usize) -> Arc<dyn rustfft::Fft<f32>> {
    FFT_PLANNER.with(|p| p.borrow_mut().plan_fft_inverse(size))
}

/// Microphone compensation data (frequency response correction)
#[derive(Debug, Clone)]
pub struct MicrophoneCompensation {
    /// Frequency points in Hz
    pub frequencies: Vec<f32>,
    /// SPL deviation in dB (positive = mic is louder, negative = mic is quieter)
    pub spl_db: Vec<f32>,
}

impl MicrophoneCompensation {
    /// Apply pre-compensation to a sweep signal
    ///
    /// For log sweeps, this modulates the amplitude based on the instantaneous frequency
    /// to pre-compensate for the microphone's response.
    ///
    /// # Arguments
    /// * `signal` - The sweep signal to compensate
    /// * `start_freq` - Start frequency of the sweep in Hz
    /// * `end_freq` - End frequency of the sweep in Hz
    /// * `sample_rate` - Sample rate in Hz
    /// * `inverse` - If true, applies inverse compensation (boost where mic is weak)
    ///
    /// # Returns
    /// Pre-compensated signal
    pub fn apply_to_sweep(
        &self,
        signal: &[f32],
        start_freq: f32,
        end_freq: f32,
        sample_rate: u32,
        inverse: bool,
    ) -> Vec<f32> {
        let duration = signal.len() as f32 / sample_rate as f32;
        let mut compensated = Vec::with_capacity(signal.len());

        // Debug: print some sample points
        let debug_points = [0, signal.len() / 4, signal.len() / 2, 3 * signal.len() / 4];

        for (i, &sample) in signal.iter().enumerate() {
            let t = i as f32 / sample_rate as f32;

            // Compute instantaneous frequency for log sweep
            // f(t) = f0 * exp(t * ln(f1/f0) / T)
            let freq = start_freq * ((t * (end_freq / start_freq).ln()) / duration).exp();

            // Get compensation at this frequency (in dB)
            let comp_db = self.interpolate_at(freq);

            // Apply inverse or direct compensation
            let gain_db = if inverse { -comp_db } else { comp_db };

            // Convert dB to linear gain
            let gain = 10_f32.powf(gain_db / 20.0);

            // Debug output for sample points
            if debug_points.contains(&i) {
                log::debug!(
                    "[apply_to_sweep] t={:.3}s, freq={:.1}Hz, comp_db={:.2}dB, gain_db={:.2}dB, gain={:.3}x",
                    t,
                    freq,
                    comp_db,
                    gain_db,
                    gain
                );
            }

            compensated.push(sample * gain);
        }

        log::debug!(
            "[apply_to_sweep] Processed {} samples, duration={:.2}s",
            signal.len(),
            duration
        );
        compensated
    }

    /// Load microphone compensation from a CSV or TXT file
    ///
    /// File format:
    /// - CSV: frequency_hz,spl_db (with or without header, comma-separated)
    /// - TXT: freq spl (space/tab-separated, no header assumed)
    pub fn from_file(path: &Path) -> Result<Self, String> {
        use std::fs::File;
        use std::io::{BufRead, BufReader};

        log::debug!("[MicrophoneCompensation] Loading from {:?}", path);

        let file = File::open(path)
            .map_err(|e| format!("Failed to open compensation file {:?}: {}", path, e))?;
        let reader = BufReader::new(file);

        // Determine if this is a .txt file (no header expected)
        let is_txt_file = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase() == "txt")
            .unwrap_or(false);

        if is_txt_file {
            log::info!(
                "[MicrophoneCompensation] Detected .txt file - assuming space/tab-separated without header"
            );
        }

        let mut frequencies = Vec::new();
        let mut spl_db = Vec::new();

        for (line_num, line) in reader.lines().enumerate() {
            let line = line.map_err(|e| format!("Failed to read line {}: {}", line_num + 1, e))?;
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // For CSV files, skip header line
            if !is_txt_file && line.starts_with("frequency") {
                continue;
            }

            // For TXT files, skip lines that don't start with a number
            if is_txt_file {
                let first_char = line.chars().next().unwrap_or(' ');
                if !first_char.is_ascii_digit() && first_char != '-' && first_char != '+' {
                    log::info!(
                        "[MicrophoneCompensation] Skipping non-numeric line {}: '{}'",
                        line_num + 1,
                        line
                    );
                    continue;
                }
            }

            // Parse based on file type with auto-detection for TXT
            let parts: Vec<&str> = if is_txt_file {
                // TXT: Try to auto-detect separator
                // First, try comma (in case it's mislabeled CSV)
                let comma_parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                if comma_parts.len() >= 2
                    && comma_parts[0].parse::<f32>().is_ok()
                    && comma_parts[1].parse::<f32>().is_ok()
                {
                    comma_parts
                } else {
                    // Try tab
                    let tab_parts: Vec<&str> = line.split('\t').map(|s| s.trim()).collect();
                    if tab_parts.len() >= 2
                        && tab_parts[0].parse::<f32>().is_ok()
                        && tab_parts[1].parse::<f32>().is_ok()
                    {
                        tab_parts
                    } else {
                        // Fall back to whitespace
                        line.split_whitespace().collect()
                    }
                }
            } else {
                // CSV: comma separated
                line.split(',').collect()
            };

            if parts.len() < 2 {
                let separator = if is_txt_file {
                    "separator (comma/tab/space)"
                } else {
                    "comma"
                };
                return Err(format!(
                    "Invalid format at line {}: expected {} with 2+ values but got '{}'",
                    line_num + 1,
                    separator,
                    line
                ));
            }

            let freq: f32 = parts[0]
                .trim()
                .parse()
                .map_err(|e| format!("Invalid frequency at line {}: {}", line_num + 1, e))?;
            let spl: f32 = parts[1]
                .trim()
                .parse()
                .map_err(|e| format!("Invalid SPL at line {}: {}", line_num + 1, e))?;

            frequencies.push(freq);
            spl_db.push(spl);
        }

        if frequencies.is_empty() {
            return Err(format!("No compensation data found in {:?}", path));
        }

        // Validate that frequencies are sorted
        for i in 1..frequencies.len() {
            if frequencies[i] <= frequencies[i - 1] {
                return Err(format!(
                    "Frequencies must be strictly increasing: found {} after {} at line {}",
                    frequencies[i],
                    frequencies[i - 1],
                    i + 1
                ));
            }
        }

        log::info!(
            "[MicrophoneCompensation] Loaded {} calibration points: {:.1} Hz - {:.1} Hz",
            frequencies.len(),
            frequencies[0],
            frequencies[frequencies.len() - 1]
        );
        log::info!(
            "[MicrophoneCompensation] SPL range: {:.2} dB to {:.2} dB",
            spl_db.iter().fold(f32::INFINITY, |a, &b| a.min(b)),
            spl_db.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b))
        );

        Ok(Self {
            frequencies,
            spl_db,
        })
    }

    /// Interpolate compensation value at a given frequency
    ///
    /// Uses linear interpolation in dB domain.
    /// Returns 0.0 for frequencies outside the calibration range.
    pub fn interpolate_at(&self, freq: f32) -> f32 {
        if freq < self.frequencies[0] || freq > self.frequencies[self.frequencies.len() - 1] {
            // Outside calibration range - no compensation
            return 0.0;
        }

        // Find the two nearest points
        let idx = match self
            .frequencies
            .binary_search_by(|f| f.partial_cmp(&freq).unwrap_or(std::cmp::Ordering::Equal))
        {
            Ok(i) => return self.spl_db[i], // Exact match
            Err(i) => i,
        };

        if idx == 0 {
            return self.spl_db[0];
        }
        if idx >= self.frequencies.len() {
            return self.spl_db[self.frequencies.len() - 1];
        }

        // Linear interpolation
        let f0 = self.frequencies[idx - 1];
        let f1 = self.frequencies[idx];
        let s0 = self.spl_db[idx - 1];
        let s1 = self.spl_db[idx];

        let t = (freq - f0) / (f1 - f0);
        s0 + t * (s1 - s0)
    }
}

// ============================================================================
// WAV Buffer Analysis (wav2csv functionality)
// ============================================================================

/// Configuration for standalone WAV buffer analysis
#[derive(Debug, Clone)]
pub struct WavAnalysisConfig {
    /// Number of output frequency points (default: 2000)
    pub num_points: usize,
    /// Minimum frequency in Hz (default: 20)
    pub min_freq: f32,
    /// Maximum frequency in Hz (default: 20000)
    pub max_freq: f32,
    /// FFT size (if None, auto-computed based on signal length and mode)
    pub fft_size: Option<usize>,
    /// Window overlap ratio for Welch's method (0.0-1.0, default: 0.5)
    pub overlap: f32,
    /// Use single FFT instead of Welch's method (better for sweeps and impulse responses)
    pub single_fft: bool,
    /// Apply pink compensation (-3dB/octave) for log sweeps
    pub pink_compensation: bool,
    /// Use rectangular window instead of Hann
    pub no_window: bool,
}

impl Default for WavAnalysisConfig {
    fn default() -> Self {
        Self {
            num_points: 2000,
            min_freq: 20.0,
            max_freq: 20000.0,
            fft_size: None,
            overlap: 0.5,
            single_fft: false,
            pink_compensation: false,
            no_window: false,
        }
    }
}

impl WavAnalysisConfig {
    /// Create config optimized for log sweep analysis
    pub fn for_log_sweep() -> Self {
        Self {
            single_fft: true,
            pink_compensation: true,
            no_window: true,
            ..Default::default()
        }
    }

    /// Create config optimized for impulse response analysis
    pub fn for_impulse_response() -> Self {
        Self {
            single_fft: true,
            ..Default::default()
        }
    }

    /// Create config for stationary signals (music, noise)
    pub fn for_stationary() -> Self {
        Self::default()
    }
}

/// Result of standalone WAV buffer analysis
#[derive(Debug, Clone)]
pub struct WavAnalysisOutput {
    /// Frequency points in Hz (log-spaced)
    pub frequencies: Vec<f32>,
    /// Magnitude in dB
    pub magnitude_db: Vec<f32>,
    /// Phase in degrees
    pub phase_deg: Vec<f32>,
}

/// Analyze a buffer of audio samples and return frequency response
///
/// # Arguments
/// * `samples` - Mono audio samples (f32, -1.0 to 1.0)
/// * `sample_rate` - Sample rate in Hz
/// * `config` - Analysis configuration
///
/// # Returns
/// Analysis result with frequency, magnitude, and phase data
pub fn analyze_wav_buffer(
    samples: &[f32],
    sample_rate: u32,
    config: &WavAnalysisConfig,
) -> Result<WavAnalysisOutput, String> {
    if samples.is_empty() {
        return Err("Signal is empty".to_string());
    }

    // Determine FFT size
    let fft_size = if config.single_fft {
        config
            .fft_size
            .unwrap_or_else(|| wav_next_power_of_two(samples.len()))
    } else {
        config.fft_size.unwrap_or(16384)
    };

    // Compute spectrum
    let (freqs, magnitudes_db, phases_deg) = if config.single_fft {
        compute_single_fft_spectrum_internal(samples, sample_rate, fft_size, config.no_window)?
    } else {
        compute_welch_spectrum_internal(samples, sample_rate, fft_size, config.overlap)?
    };

    // Generate logarithmically spaced frequency points
    let log_freqs = generate_log_frequencies(config.num_points, config.min_freq, config.max_freq);

    // Interpolate magnitude and phase at log frequencies
    let mut interp_mag = interpolate_log(&freqs, &magnitudes_db, &log_freqs);
    let interp_phase = interpolate_log_phase(&freqs, &phases_deg, &log_freqs);

    // Apply pink compensation if requested (for log sweeps)
    if config.pink_compensation {
        let ref_freq = 1000.0;
        for (i, freq) in log_freqs.iter().enumerate() {
            if *freq > 0.0 {
                let correction = 10.0 * (freq / ref_freq).log10();
                interp_mag[i] += correction;
            }
        }
    }

    Ok(WavAnalysisOutput {
        frequencies: log_freqs,
        magnitude_db: interp_mag,
        phase_deg: interp_phase,
    })
}

/// Analyze a WAV file and return frequency response
///
/// # Arguments
/// * `path` - Path to WAV file
/// * `config` - Analysis configuration
///
/// # Returns
/// Analysis result with frequency, magnitude, and phase data
pub fn analyze_wav_file(
    path: &Path,
    config: &WavAnalysisConfig,
) -> Result<WavAnalysisOutput, String> {
    let (samples, sample_rate) = load_wav_mono_with_rate(path)?;
    analyze_wav_buffer(&samples, sample_rate, config)
}

/// Load WAV file as mono and return samples with sample rate
fn load_wav_mono_with_rate(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let mut reader =
        WavReader::open(path).map_err(|e| format!("Failed to open WAV file: {}", e))?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels as usize;

    let samples: Result<Vec<f32>, _> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect(),
        hound::SampleFormat::Int => {
            let max_val = (1_i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max_val))
                .collect()
        }
    };

    let samples = samples.map_err(|e| format!("Failed to read samples: {}", e))?;

    // Convert to mono by averaging channels
    let mono = if channels == 1 {
        samples
    } else {
        samples
            .chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect()
    };

    Ok((mono, sample_rate))
}

/// Write WAV analysis result to CSV file
///
/// # Arguments
/// * `result` - Analysis output
/// * `path` - Path to output CSV file
pub fn write_wav_analysis_csv(result: &WavAnalysisOutput, path: &Path) -> Result<(), String> {
    let mut file =
        std::fs::File::create(path).map_err(|e| format!("Failed to create CSV: {}", e))?;

    writeln!(file, "frequency_hz,spl_db,phase_deg")
        .map_err(|e| format!("Failed to write CSV header: {}", e))?;

    for i in 0..result.frequencies.len() {
        writeln!(
            file,
            "{:.2},{:.2},{:.2}",
            result.frequencies[i], result.magnitude_db[i], result.phase_deg[i]
        )
        .map_err(|e| format!("Failed to write CSV row: {}", e))?;
    }

    Ok(())
}

/// Compute spectrum using Welch's method (averaged periodograms) - internal version
fn compute_welch_spectrum_internal(
    signal: &[f32],
    sample_rate: u32,
    fft_size: usize,
    overlap: f32,
) -> SpectrumResult {
    if signal.is_empty() {
        return Err("Signal is empty".to_string());
    }

    let overlap_samples = (fft_size as f32 * overlap.clamp(0.0, 0.95)) as usize;
    let hop_size = fft_size - overlap_samples;

    let num_windows = if signal.len() >= fft_size {
        1 + (signal.len() - fft_size) / hop_size
    } else {
        1
    };

    let num_bins = fft_size / 2;
    let mut magnitude_sum = vec![0.0_f32; num_bins];
    let mut phase_real_sum = vec![0.0_f32; num_bins];
    let mut phase_imag_sum = vec![0.0_f32; num_bins];

    // Precompute symmetric Hann window (N-1 divisor for spectral analysis)
    let hann_window = crate::stft::generate_hann_window_symmetric(fft_size);

    let window_power: f32 = hann_window.iter().map(|&w| w * w).sum();
    let scale_factor = 2.0 / window_power;

    let fft = plan_fft_forward(fft_size);

    let mut windowed = vec![0.0_f32; fft_size];
    let mut buffer = vec![Complex::new(0.0, 0.0); fft_size];

    for window_idx in 0..num_windows {
        let start = window_idx * hop_size;
        let end = (start + fft_size).min(signal.len());
        let window_len = end - start;

        // Apply window
        for i in 0..window_len {
            windowed[i] = signal[start + i] * hann_window[i];
        }
        // Zero-pad the rest if necessary
        windowed[window_len..fft_size].fill(0.0);

        // Convert to complex
        for (i, &val) in windowed.iter().enumerate() {
            buffer[i] = Complex::new(val, 0.0);
        }

        fft.process(&mut buffer);

        for i in 0..num_bins {
            let mag = buffer[i].norm() * scale_factor.sqrt();
            magnitude_sum[i] += mag * mag;
            phase_real_sum[i] += buffer[i].re;
            phase_imag_sum[i] += buffer[i].im;
        }
    }

    let magnitudes_db: Vec<f32> = magnitude_sum
        .iter()
        .map(|&mag_sq| {
            let mag = (mag_sq / num_windows as f32).sqrt();
            if mag > 1e-10 {
                20.0 * mag.log10()
            } else {
                -200.0
            }
        })
        .collect();

    let phases_deg: Vec<f32> = phase_real_sum
        .iter()
        .zip(phase_imag_sum.iter())
        .map(|(&re, &im)| (im / num_windows as f32).atan2(re / num_windows as f32) * 180.0 / PI)
        .collect();

    let freqs: Vec<f32> = (0..num_bins)
        .map(|i| i as f32 * sample_rate as f32 / fft_size as f32)
        .collect();

    Ok((freqs, magnitudes_db, phases_deg))
}

/// Compute spectrum using a single FFT - internal version
fn compute_single_fft_spectrum_internal(
    signal: &[f32],
    sample_rate: u32,
    fft_size: usize,
    no_window: bool,
) -> SpectrumResult {
    if signal.is_empty() {
        return Err("Signal is empty".to_string());
    }

    let mut windowed = vec![0.0_f32; fft_size];
    let copy_len = signal.len().min(fft_size);
    windowed[..copy_len].copy_from_slice(&signal[..copy_len]);

    let window_scale_factor = if no_window {
        1.0
    } else {
        let hann_window = crate::stft::generate_hann_window_symmetric(fft_size);

        for (i, sample) in windowed.iter_mut().enumerate() {
            *sample *= hann_window[i];
        }

        hann_window.iter().map(|&w| w * w).sum::<f32>()
    };

    let mut buffer: Vec<Complex<f32>> = windowed.iter().map(|&x| Complex::new(x, 0.0)).collect();

    let fft = plan_fft_forward(fft_size);
    fft.process(&mut buffer);

    let scale_factor = if no_window {
        (2.0 / fft_size as f32).sqrt()
    } else {
        (2.0 / window_scale_factor).sqrt()
    };

    let num_bins = fft_size / 2;
    let magnitudes_db: Vec<f32> = buffer[..num_bins]
        .iter()
        .map(|c| {
            let mag = c.norm() * scale_factor;
            if mag > 1e-10 {
                20.0 * mag.log10()
            } else {
                -200.0
            }
        })
        .collect();

    let phases_deg: Vec<f32> = buffer[..num_bins]
        .iter()
        .map(|c| c.arg() * 180.0 / PI)
        .collect();

    let freqs: Vec<f32> = (0..num_bins)
        .map(|i| i as f32 * sample_rate as f32 / fft_size as f32)
        .collect();

    Ok((freqs, magnitudes_db, phases_deg))
}

/// Next power of two for wav analysis (capped at 1M)
fn wav_next_power_of_two(n: usize) -> usize {
    let mut p = 1;
    while p < n {
        p *= 2;
    }
    p.min(1048576)
}

/// Generate logarithmically spaced frequencies
fn generate_log_frequencies(num_points: usize, min_freq: f32, max_freq: f32) -> Vec<f32> {
    let log_min = min_freq.ln();
    let log_max = max_freq.ln();
    let step = (log_max - log_min) / (num_points - 1) as f32;

    (0..num_points)
        .map(|i| (log_min + i as f32 * step).exp())
        .collect()
}

/// Logarithmic interpolation
fn interpolate_log(x: &[f32], y: &[f32], x_new: &[f32]) -> Vec<f32> {
    x_new
        .iter()
        .map(|&freq| {
            let idx = x.partition_point(|&f| f < freq).min(x.len() - 1);

            if idx == 0 {
                return y[0];
            }

            let x0 = x[idx - 1];
            let x1 = x[idx];
            let y0 = y[idx - 1];
            let y1 = y[idx];

            if x1 <= x0 {
                return y0;
            }

            let t = (freq - x0) / (x1 - x0);
            y0 + t * (y1 - y0)
        })
        .collect()
}

/// Logarithmic interpolation for phase data (degrees).
/// Uses circular interpolation to correctly handle ±180° wrap boundaries.
fn interpolate_log_phase(x: &[f32], phase_deg: &[f32], x_new: &[f32]) -> Vec<f32> {
    x_new
        .iter()
        .map(|&freq| {
            let idx = x.partition_point(|&f| f < freq).min(x.len() - 1);

            if idx == 0 {
                return phase_deg[0];
            }

            let x0 = x[idx - 1];
            let x1 = x[idx];

            if x1 <= x0 {
                return phase_deg[idx - 1];
            }

            let t = (freq - x0) / (x1 - x0);

            // Circular interpolation: find shortest arc between the two angles
            let p0 = phase_deg[idx - 1];
            let p1 = phase_deg[idx];
            let mut diff = p1 - p0;
            // Wrap diff to [-180, 180]
            diff -= 360.0 * (diff / 360.0).round();
            p0 + t * diff
        })
        .collect()
}

// ============================================================================
// Recording Analysis (reference vs recorded comparison)
// ============================================================================

/// Result of FFT analysis
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    /// Frequency bins in Hz
    pub frequencies: Vec<f32>,
    /// Magnitude in dBFS
    pub spl_db: Vec<f32>,
    /// Phase in degrees (compensated for latency)
    pub phase_deg: Vec<f32>,
    /// Estimated latency in samples
    pub estimated_lag_samples: isize,
    /// Impulse response (time domain)
    pub impulse_response: Vec<f32>,
    /// Time vector for impulse response in ms
    pub impulse_time_ms: Vec<f32>,
    /// Excess group delay in ms
    pub excess_group_delay_ms: Vec<f32>,
    /// Total Harmonic Distortion + Noise (%)
    pub thd_percent: Vec<f32>,
    /// Harmonic distortion curves (2nd, 3rd, etc) in dB
    pub harmonic_distortion_db: Vec<Vec<f32>>,
    /// RT60 decay time in ms
    pub rt60_ms: Vec<f32>,
    /// Clarity C50 in dB
    pub clarity_c50_db: Vec<f32>,
    /// Clarity C80 in dB
    pub clarity_c80_db: Vec<f32>,
    /// Spectrogram (Time x Freq magnitude in dB)
    pub spectrogram_db: Vec<Vec<f32>>,
}

/// Analyze a recorded WAV file against a reference signal
///
/// # Arguments
/// * `recorded_path` - Path to the recorded WAV file
/// * `reference_signal` - Reference signal (should match the signal used for playback)
/// * `sample_rate` - Sample rate in Hz
/// * `sweep_range` - Optional (start_freq, end_freq) if the signal is a log sweep
///
/// # Returns
/// Analysis result with frequency, SPL, and phase data
pub fn analyze_recording(
    recorded_path: &Path,
    reference_signal: &[f32],
    sample_rate: u32,
    sweep_range: Option<(f32, f32)>,
) -> Result<AnalysisResult, String> {
    // Load recorded WAV
    log::debug!("[FFT Analysis] Loading recorded file: {:?}", recorded_path);
    let recorded = load_wav_mono(recorded_path)?;
    log::debug!(
        "[FFT Analysis] Loaded {} samples from recording",
        recorded.len()
    );
    log::debug!(
        "[FFT Analysis] Reference has {} samples",
        reference_signal.len()
    );

    if recorded.is_empty() {
        return Err("Recorded signal is empty!".to_string());
    }
    if reference_signal.is_empty() {
        return Err("Reference signal is empty!".to_string());
    }

    // Don't truncate yet - we need full signals for lag estimation
    let recorded = &recorded[..];
    let reference = reference_signal;

    // Debug: Check signal statistics (guarded to skip O(n) computation when disabled)
    if log::log_enabled!(log::Level::Debug) {
        let ref_max = reference
            .iter()
            .map(|&x| x.abs())
            .fold(0.0_f32, |a, b| a.max(b));
        let rec_max = recorded
            .iter()
            .map(|&x| x.abs())
            .fold(0.0_f32, |a, b| a.max(b));
        let ref_rms =
            (reference.iter().map(|&x| x * x).sum::<f32>() / reference.len() as f32).sqrt();
        let rec_rms = (recorded.iter().map(|&x| x * x).sum::<f32>() / recorded.len() as f32).sqrt();

        log::debug!(
            "[FFT Analysis] Reference: max={:.4}, RMS={:.4}",
            ref_max,
            ref_rms
        );
        log::debug!(
            "[FFT Analysis] Recorded:  max={:.4}, RMS={:.4}",
            rec_max,
            rec_rms
        );
        log::debug!(
            "[FFT Analysis] First 5 reference samples: {:?}",
            &reference[..5.min(reference.len())]
        );
        log::debug!(
            "[FFT Analysis] First 5 recorded samples:  {:?}",
            &recorded[..5.min(recorded.len())]
        );

        let check_len = reference.len().min(recorded.len());
        let mut identical_count = 0;
        for (r, c) in reference[..check_len]
            .iter()
            .zip(recorded[..check_len].iter())
        {
            if (r - c).abs() < 1e-6 {
                identical_count += 1;
            }
        }
        log::debug!(
            "[FFT Analysis] Identical samples: {}/{} ({:.1}%)",
            identical_count,
            check_len,
            identical_count as f32 * 100.0 / check_len as f32
        );
    }

    // Estimate lag using cross-correlation
    let lag = estimate_lag(reference, recorded)?;

    log::debug!(
        "[FFT Analysis] Estimated lag: {} samples ({:.2} ms)",
        lag,
        lag as f32 * 1000.0 / sample_rate as f32
    );

    // Time-align the signals before FFT
    // If recorded is delayed (positive lag), skip the lag samples in recorded
    let (aligned_ref, aligned_rec) = if lag >= 0 {
        let lag_usize = lag as usize;
        if lag_usize >= recorded.len() {
            return Err("Lag is larger than recorded signal length".to_string());
        }
        // Capture full tail
        (reference, &recorded[lag_usize..])
    } else {
        // Recorded leads reference - rare
        let lag_usize = (-lag) as usize;
        if lag_usize >= reference.len() {
            return Err("Negative lag is larger than reference signal length".to_string());
        }
        // Pad reference start? No, just slice reference
        (&reference[lag_usize..], recorded)
    };

    log::debug!(
        "[FFT Analysis] Aligned lengths: ref={}, rec={} (tail included)",
        aligned_ref.len(),
        aligned_rec.len()
    );

    // Compute FFT size to include the longer of the two (usually rec with tail)
    let fft_size = next_power_of_two(aligned_ref.len().max(aligned_rec.len()));

    let ref_spectrum = compute_fft(aligned_ref, fft_size, WindowType::Tukey(0.1))?;
    let rec_spectrum = compute_fft(aligned_rec, fft_size, WindowType::Tukey(0.1))?;

    // Generate 2000 log-spaced frequency points between 20 Hz and 20 kHz
    let num_output_points = 2000;
    let log_start = 20.0_f32.ln();
    let log_end = 20000.0_f32.ln();

    let mut frequencies = Vec::with_capacity(num_output_points);
    let mut spl_db = Vec::with_capacity(num_output_points);
    let mut phase_deg = Vec::with_capacity(num_output_points);

    let freq_resolution = sample_rate as f32 / fft_size as f32;
    let num_bins = fft_size / 2; // Single-sided spectrum

    // Compute regularization threshold relative to the peak reference energy.
    // Bins where the reference has very little energy (e.g., disconnected speaker
    // with a misaligned sweep) produce unreliable transfer functions — division by
    // near-zero gives spurious high-dB peaks. We skip bins where the reference
    // energy is more than 60 dB below the peak.
    let ref_peak_mag_sq = ref_spectrum[1..num_bins.min(ref_spectrum.len())]
        .iter()
        .map(|c| c.norm_sqr())
        .fold(0.0_f32, |a, b| a.max(b));
    // 60 dB below peak = 10^(-6) in power
    let ref_regularization_threshold = ref_peak_mag_sq * 1e-6;

    // Apply 1/24 octave smoothing for each target frequency
    let mut skipped_count = 0;
    for i in 0..num_output_points {
        // Log-spaced target frequency
        let target_freq =
            (log_start + (log_end - log_start) * i as f32 / (num_output_points - 1) as f32).exp();

        // 1/24 octave bandwidth: +/- 1/48 octave around target frequency
        // Lower and upper frequency bounds: f * 2^(+/- 1/48)
        let octave_fraction = 1.0 / 48.0;
        let freq_lower = target_freq * 2.0_f32.powf(-octave_fraction);
        let freq_upper = target_freq * 2.0_f32.powf(octave_fraction);

        // Find FFT bins within this frequency range
        let bin_lower = ((freq_lower / freq_resolution).floor() as usize).max(1);
        let bin_upper = ((freq_upper / freq_resolution).ceil() as usize).min(num_bins);

        if bin_lower > bin_upper || bin_upper >= ref_spectrum.len() {
            if skipped_count < 5 {
                log::debug!(
                    "[FFT Analysis] Skipping freq {:.1} Hz: bin_lower={}, bin_upper={}, ref_spectrum.len()={}",
                    target_freq,
                    bin_lower,
                    bin_upper,
                    ref_spectrum.len()
                );
            }
            skipped_count += 1;
            // Output noise-floor placeholder so all channels produce the same
            // number of frequency points (prevents ndarray shape mismatches).
            frequencies.push(target_freq);
            spl_db.push(-200.0);
            phase_deg.push(0.0);
            continue;
        }

        // Average transfer function magnitude and phase across bins in the smoothing range
        let mut sum_magnitude = 0.0;
        let mut sum_sin = 0.0; // For circular averaging of phase
        let mut sum_cos = 0.0;
        let mut bin_count = 0;

        for k in bin_lower..=bin_upper {
            if k >= ref_spectrum.len() {
                break;
            }

            // Compute transfer function: H(f) = recorded / reference
            // This gives the system response (for loopback, should be ~1.0 or 0 dB)
            // Skip bins where the reference energy is too low (>60 dB below peak):
            // dividing by near-zero produces unreliable, spuriously high values
            // (e.g., disconnected speaker where the recording is just noise).
            let ref_mag_sq = ref_spectrum[k].norm_sqr();
            if ref_mag_sq <= ref_regularization_threshold {
                continue;
            }
            let transfer_function = rec_spectrum[k] / ref_spectrum[k];
            let magnitude = transfer_function.norm();

            // Phase from cross-spectrum (signals are already time-aligned)
            let cross_spectrum = ref_spectrum[k].conj() * rec_spectrum[k];
            let phase_rad = cross_spectrum.arg();

            // Accumulate for averaging
            sum_magnitude += magnitude;
            sum_sin += phase_rad.sin();
            sum_cos += phase_rad.cos();
            bin_count += 1;
        }

        // When no valid bins contribute (reference energy too low at this frequency,
        // e.g., LFE sweep above 500 Hz), output a noise-floor value instead of skipping.
        // Skipping would produce fewer output points than other channels, causing
        // ndarray shape mismatches when curves are combined downstream.
        let (avg_magnitude, db) = if bin_count == 0 {
            (0.0, -200.0)
        } else {
            let avg = sum_magnitude / bin_count as f32;
            (avg, 20.0 * avg.max(1e-10).log10())
        };

        if frequencies.len() < 5 {
            log::debug!(
                "[FFT Analysis] freq={:.1} Hz: avg_magnitude={:.6}, dB={:.2}",
                target_freq,
                avg_magnitude,
                db
            );
        }

        // Average phase using circular mean
        let avg_phase_rad = sum_sin.atan2(sum_cos);
        let phase = avg_phase_rad * 180.0 / PI;

        frequencies.push(target_freq);
        spl_db.push(db);
        phase_deg.push(phase);
    }

    log::debug!(
        "[FFT Analysis] Generated {} frequency points for CSV output",
        frequencies.len()
    );
    log::debug!(
        "[FFT Analysis] Skipped {} frequency points (out of {})",
        skipped_count,
        num_output_points
    );

    if log::log_enabled!(log::Level::Debug) && !spl_db.is_empty() {
        let min_spl = spl_db.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_spl = spl_db.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        log::debug!(
            "[FFT Analysis] SPL range: {:.2} dB to {:.2} dB",
            min_spl,
            max_spl
        );
    }

    // --- Compute Impulse Response ---
    // H(f) = Recorded(f) / Reference(f)
    let mut transfer_function = vec![Complex::new(0.0, 0.0); fft_size];
    for k in 0..fft_size {
        // Handle DC and Nyquist specially if needed, but for complex FFT it's just bins
        // Avoid division by zero
        let ref_mag_sq = ref_spectrum[k].norm_sqr();
        if ref_mag_sq > 1e-20 {
            transfer_function[k] = rec_spectrum[k] / ref_spectrum[k];
        }
    }

    // IFFT to get Impulse Response
    let ifft = plan_fft_inverse(fft_size);
    ifft.process(&mut transfer_function);

    // Normalize and take real part (input was real, so output should be real-ish)
    // Scale by 1.0/N is done by IFFT? rustfft typically does NOT scale.
    // Standard IFFT definition: sum(X[k] * exp(...)) / N?
    // RustFFT inverse is unnormalized sum. So we divide by N.
    let norm = 1.0 / fft_size as f32;
    let mut impulse_response: Vec<f32> = transfer_function.iter().map(|c| c.re * norm).collect();

    // Find the peak and shift the IR so the peak is near the beginning
    // This is necessary because the IFFT result has the peak at an arbitrary position
    // due to the phase of the transfer function (system latency)
    let peak_idx = impulse_response
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0);

    // Shift the IR so peak is at a small offset (e.g., 5ms for pre-ringing visibility)
    let pre_ring_samples = (0.005 * sample_rate as f32) as usize; // 5ms pre-ring buffer
    let shift_amount = peak_idx.saturating_sub(pre_ring_samples);

    if shift_amount > 0 {
        impulse_response.rotate_left(shift_amount);
        log::info!(
            "[FFT Analysis] IR peak was at index {}, shifted by {} samples to put peak near beginning",
            peak_idx,
            shift_amount
        );
    }

    // Generate time vector for IR (0 to duration)
    let _ir_duration_sec = fft_size as f32 / sample_rate as f32;
    let impulse_time_ms: Vec<f32> = (0..fft_size)
        .map(|i| i as f32 / sample_rate as f32 * 1000.0)
        .collect();

    // --- Compute THD if sweep range is provided ---
    let (thd_percent, harmonic_distortion_db) = if let Some((start, end)) = sweep_range {
        // Assume sweep duration is same as impulse length (circular convolution)
        // or derived from reference signal length
        let duration = reference_signal.len() as f32 / sample_rate as f32;
        compute_thd_from_ir(
            &impulse_response,
            sample_rate as f32,
            &frequencies,
            &spl_db,
            start,
            end,
            duration,
        )
    } else {
        (vec![0.0; frequencies.len()], Vec::new())
    };

    // --- Compute Excess Group Delay ---
    // (Placeholder)
    let excess_group_delay_ms = vec![0.0; frequencies.len()];

    // --- Compute Acoustic Metrics ---
    // Debug: Log impulse response stats
    let ir_max = impulse_response.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
    let ir_len = impulse_response.len();
    log::info!(
        "[Analysis] Impulse response: len={}, max_abs={:.6}, sample_rate={}",
        ir_len,
        ir_max,
        sample_rate
    );

    let rt60_ms = compute_rt60_spectrum(&impulse_response, sample_rate as f32, &frequencies);
    let (clarity_c50_db, clarity_c80_db) =
        compute_clarity_spectrum(&impulse_response, sample_rate as f32, &frequencies);

    // Debug: Log computed metrics
    if !rt60_ms.is_empty() {
        let rt60_min = rt60_ms.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let rt60_max = rt60_ms.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        log::info!(
            "[Analysis] RT60 range: {:.1} - {:.1} ms",
            rt60_min,
            rt60_max
        );
    }
    if !clarity_c50_db.is_empty() {
        let c50_min = clarity_c50_db.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let c50_max = clarity_c50_db
            .iter()
            .fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        log::info!(
            "[Analysis] Clarity C50 range: {:.1} - {:.1} dB",
            c50_min,
            c50_max
        );
    }

    // Compute Spectrogram
    let (spectrogram_db, _, _) =
        compute_spectrogram(&impulse_response, sample_rate as f32, 512, 128);

    Ok(AnalysisResult {
        frequencies,
        spl_db,
        phase_deg,
        estimated_lag_samples: lag,
        impulse_response,
        impulse_time_ms,
        excess_group_delay_ms,
        thd_percent,
        harmonic_distortion_db,
        rt60_ms,
        clarity_c50_db,
        clarity_c80_db,
        spectrogram_db,
    })
}

/// Compute Total Harmonic Distortion (THD) from Impulse Response
///
/// Uses Farina's method to extract harmonics from the impulse response of a log sweep.
fn compute_thd_from_ir(
    impulse: &[f32],
    sample_rate: f32,
    frequencies: &[f32],
    fundamental_db: &[f32],
    start_freq: f32,
    end_freq: f32,
    duration: f32,
) -> (Vec<f32>, Vec<Vec<f32>>) {
    if frequencies.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let n = impulse.len();
    if n == 0 {
        return (vec![0.0; frequencies.len()], Vec::new());
    }

    let num_harmonics = 4; // Compute 2nd, 3rd, 4th, 5th
    // Initialize to -120 dB (very low but not absurdly so)
    let mut harmonics_db = vec![vec![-120.0; frequencies.len()]; num_harmonics];

    // Find main peak index (t=0)
    let peak_idx = impulse
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0);

    let sweep_ratio = end_freq / start_freq;
    log::debug!(
        "[THD] Impulse len={}, peak_idx={}, duration={:.3}s, sweep {:.0}-{:.0} Hz (ratio {:.1})",
        n,
        peak_idx,
        duration,
        start_freq,
        end_freq,
        sweep_ratio
    );

    // Compute harmonics
    for (k_idx, harmonic_db) in harmonics_db.iter_mut().enumerate().take(num_harmonics) {
        let harmonic_order = k_idx + 2; // 2nd harmonic is k=2

        // Calculate delay for this harmonic
        // dt = T * ln(k) / ln(f2/f1)
        let dt = duration * (harmonic_order as f32).ln() / sweep_ratio.ln();
        let dn = (dt * sample_rate).round() as isize;

        // Center of harmonic impulse (negative time wraps to end of array)
        let center = peak_idx as isize - dn;
        let center_wrapped = center.rem_euclid(n as isize) as usize;

        // Window size logic: distance to next harmonic * 0.8 to avoid overlap
        let dt_next_rel = duration
            * ((harmonic_order as f32 + 1.0).ln() - (harmonic_order as f32).ln())
            / sweep_ratio.ln();
        let win_len = ((dt_next_rel * sample_rate * 0.8).max(256.0) as usize).min(n / 2);

        // Extract windowed harmonic IR
        let mut harmonic_ir = vec![0.0f32; win_len];
        let mut max_harmonic_sample = 0.0f32;
        for (i, harmonic_ir_val) in harmonic_ir.iter_mut().enumerate() {
            let src_idx =
                (center - (win_len as isize / 2) + i as isize).rem_euclid(n as isize) as usize;
            // Apply Hann window
            let w = 0.5 * (1.0 - (2.0 * PI * i as f32 / (win_len as f32 - 1.0)).cos());
            *harmonic_ir_val = impulse[src_idx] * w;
            max_harmonic_sample = max_harmonic_sample.max(harmonic_ir_val.abs());
        }

        if k_idx == 0 {
            log::debug!(
                "[THD] H{}: dt={:.3}s, dn={}, center_wrapped={}, win_len={}, max_sample={:.2e}",
                harmonic_order,
                dt,
                dn,
                center_wrapped,
                win_len,
                max_harmonic_sample
            );
        }

        // Compute spectrum
        let fft_size = next_power_of_two(win_len);
        let nyquist_bin = fft_size / 2; // Only use positive frequency bins
        if let Ok(spectrum) = compute_fft_padded(&harmonic_ir, fft_size) {
            let freq_resolution = sample_rate / fft_size as f32;

            for (i, &f) in frequencies.iter().enumerate() {
                let bin = (f / freq_resolution).round() as usize;
                // Only access positive frequency bins (0 to nyquist)
                if bin < nyquist_bin && bin < spectrum.len() {
                    // compute_fft_padded already applies 1/N normalization, matching
                    // the scale of fundamental_db (derived from transfer function ratios)
                    let mag = spectrum[bin].norm();
                    // Convert to dB (threshold at -120 dB to avoid log of tiny values)
                    if mag > 1e-6 {
                        harmonic_db[i] = 20.0 * mag.log10();
                    }
                }
            }
        }
    }

    // Log a summary of detected harmonic levels
    if !frequencies.is_empty() {
        let mid_idx = frequencies.len() / 2;
        log::debug!(
            "[THD] Harmonic levels at {:.0} Hz: H2={:.1}dB, H3={:.1}dB, H4={:.1}dB, H5={:.1}dB, fundamental={:.1}dB",
            frequencies[mid_idx],
            harmonics_db[0][mid_idx],
            harmonics_db[1][mid_idx],
            harmonics_db[2][mid_idx],
            harmonics_db[3][mid_idx],
            fundamental_db[mid_idx]
        );
    }

    // Compute THD %
    let mut thd_percent = Vec::with_capacity(frequencies.len());
    for i in 0..frequencies.len() {
        let fundamental = 10.0f32.powf(fundamental_db[i] / 20.0);
        let mut harmonic_sum_sq = 0.0;

        for harmonic_db in harmonics_db.iter().take(num_harmonics) {
            let h_mag = 10.0f32.powf(harmonic_db[i] / 20.0);
            harmonic_sum_sq += h_mag * h_mag;
        }

        // THD = sqrt(sum(harmonics^2)) / fundamental
        let thd = if fundamental > 1e-9 {
            (harmonic_sum_sq.sqrt() / fundamental) * 100.0
        } else {
            0.0
        };
        thd_percent.push(thd);
    }

    // Log THD summary
    if !thd_percent.is_empty() {
        let max_thd = thd_percent.iter().fold(0.0f32, |a, &b| a.max(b));
        let min_thd = thd_percent.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        log::debug!("[THD] THD range: {:.4}% to {:.4}%", min_thd, max_thd);
    }

    (thd_percent, harmonics_db)
}

/// Write analysis results to CSV file with optional microphone compensation
///
/// # Arguments
/// * `result` - Analysis result
/// * `output_path` - Path to output CSV file
/// * `compensation` - Optional microphone compensation to apply (inverse)
///
/// When compensation is provided, the inverse is applied: the microphone's
/// SPL deviation is subtracted from the measured SPL to get the true SPL.
///
/// CSV format includes all analysis metrics:
/// frequency_hz, spl_db, phase_deg, thd_percent, rt60_ms, c50_db, c80_db, group_delay_ms
pub fn write_analysis_csv(
    result: &AnalysisResult,
    output_path: &Path,
    compensation: Option<&MicrophoneCompensation>,
) -> Result<(), String> {
    use std::fs::File;
    use std::io::Write;

    log::info!(
        "[write_analysis_csv] Writing {} frequency points to {:?}",
        result.frequencies.len(),
        output_path
    );

    if let Some(comp) = compensation {
        log::info!(
            "[write_analysis_csv] Applying inverse microphone compensation ({} calibration points)",
            comp.frequencies.len()
        );
    }

    if result.frequencies.is_empty() {
        return Err("Cannot write CSV: Analysis result has no frequency points!".to_string());
    }

    let mut file =
        File::create(output_path).map_err(|e| format!("Failed to create CSV file: {}", e))?;

    // Write header with all metrics
    writeln!(
        file,
        "frequency_hz,spl_db,phase_deg,thd_percent,rt60_ms,c50_db,c80_db,group_delay_ms"
    )
    .map_err(|e| format!("Failed to write header: {}", e))?;

    // Write data with compensation applied
    for i in 0..result.frequencies.len() {
        let freq = result.frequencies[i];
        let mut spl = result.spl_db[i];

        // Apply inverse compensation: subtract microphone deviation
        // If mic reads +2dB at this frequency, the true level is 2dB lower
        if let Some(comp) = compensation {
            let mic_deviation = comp.interpolate_at(freq);
            spl -= mic_deviation;
        }

        let phase = result.phase_deg[i];
        let thd = result.thd_percent.get(i).copied().unwrap_or(0.0);
        let rt60 = result.rt60_ms.get(i).copied().unwrap_or(0.0);
        let c50 = result.clarity_c50_db.get(i).copied().unwrap_or(0.0);
        let c80 = result.clarity_c80_db.get(i).copied().unwrap_or(0.0);
        let gd = result.excess_group_delay_ms.get(i).copied().unwrap_or(0.0);

        writeln!(
            file,
            "{:.6},{:.3},{:.6},{:.6},{:.3},{:.3},{:.3},{:.6}",
            freq, spl, phase, thd, rt60, c50, c80, gd
        )
        .map_err(|e| format!("Failed to write data: {}", e))?;
    }

    log::info!(
        "[write_analysis_csv] Successfully wrote {} data rows to CSV",
        result.frequencies.len()
    );

    Ok(())
}

/// Read analysis results from CSV file
///
/// Parses CSV with columns: frequency_hz, spl_db, phase_deg, thd_percent, rt60_ms, c50_db, c80_db, group_delay_ms
/// Also supports legacy format with just: frequency_hz, spl_db, phase_deg
pub fn read_analysis_csv(csv_path: &Path) -> Result<AnalysisResult, String> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let file = File::open(csv_path).map_err(|e| format!("Failed to open CSV: {}", e))?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    // Read header
    let header = lines
        .next()
        .ok_or("Empty CSV file")?
        .map_err(|e| format!("Failed to read header: {}", e))?;

    let columns: Vec<&str> = header.split(',').map(|s| s.trim()).collect();
    let has_extended_format = columns.len() >= 8;

    let mut frequencies = Vec::new();
    let mut spl_db = Vec::new();
    let mut phase_deg = Vec::new();
    let mut thd_percent = Vec::new();
    let mut rt60_ms = Vec::new();
    let mut clarity_c50_db = Vec::new();
    let mut clarity_c80_db = Vec::new();
    let mut excess_group_delay_ms = Vec::new();

    for line in lines {
        let line = line.map_err(|e| format!("Failed to read line: {}", e))?;
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();

        if parts.len() < 3 {
            continue;
        }

        let freq: f32 = parts[0].parse().unwrap_or(0.0);
        let spl: f32 = parts[1].parse().unwrap_or(0.0);
        let phase: f32 = parts[2].parse().unwrap_or(0.0);

        frequencies.push(freq);
        spl_db.push(spl);
        phase_deg.push(phase);

        if has_extended_format && parts.len() >= 8 {
            thd_percent.push(parts[3].parse().unwrap_or(0.0));
            rt60_ms.push(parts[4].parse().unwrap_or(0.0));
            clarity_c50_db.push(parts[5].parse().unwrap_or(0.0));
            clarity_c80_db.push(parts[6].parse().unwrap_or(0.0));
            excess_group_delay_ms.push(parts[7].parse().unwrap_or(0.0));
        }
    }

    // If legacy format, fill with zeros
    let n = frequencies.len();
    if thd_percent.is_empty() {
        thd_percent = vec![0.0; n];
        rt60_ms = vec![0.0; n];
        clarity_c50_db = vec![0.0; n];
        clarity_c80_db = vec![0.0; n];
        excess_group_delay_ms = vec![0.0; n];
    }

    Ok(AnalysisResult {
        frequencies,
        spl_db,
        phase_deg,
        estimated_lag_samples: 0,
        impulse_response: Vec::new(),
        impulse_time_ms: Vec::new(),
        thd_percent,
        harmonic_distortion_db: Vec::new(),
        rt60_ms,
        clarity_c50_db,
        clarity_c80_db,
        excess_group_delay_ms,
        spectrogram_db: Vec::new(),
    })
}

/// Window function type for FFT
#[derive(Debug, Clone, Copy)]
enum WindowType {
    Hann,
    Tukey(f32), // alpha parameter (0.0-1.0)
}

/// Estimate lag between reference and recorded signals using cross-correlation
///
/// Uses FFT-based cross-correlation for efficiency
///
/// # Arguments
/// * `reference` - Reference signal
/// * `recorded` - Recorded signal
///
/// # Returns
/// Estimated lag in samples (negative means recorded leads)
fn estimate_lag(reference: &[f32], recorded: &[f32]) -> Result<isize, String> {
    let len = reference.len().min(recorded.len());

    // Zero-pad to avoid circular correlation artifacts
    let fft_size = next_power_of_two(len * 2);

    // Use Hann window for correlation to suppress edge effects
    let ref_fft = compute_fft(reference, fft_size, WindowType::Hann)?;
    let rec_fft = compute_fft(recorded, fft_size, WindowType::Hann)?;

    // Cross-correlation in frequency domain: conj(X) * Y
    let mut cross_corr_fft: Vec<Complex<f32>> = ref_fft
        .iter()
        .zip(rec_fft.iter())
        .map(|(x, y)| x.conj() * y)
        .collect();

    // IFFT to get cross-correlation in time domain
    let ifft = plan_fft_inverse(fft_size);
    ifft.process(&mut cross_corr_fft);

    // Find peak
    let mut max_val = 0.0;
    let mut max_idx = 0;

    for (i, &val) in cross_corr_fft.iter().enumerate() {
        let magnitude = val.norm();
        if magnitude > max_val {
            max_val = magnitude;
            max_idx = i;
        }
    }

    // Convert index to lag (handle wrap-around)
    Ok(if max_idx <= fft_size / 2 {
        max_idx as isize
    } else {
        max_idx as isize - fft_size as isize
    })
}

/// Result of cross-correlation with analytic envelope detection.
///
/// The envelope peak corresponds to the probe's arrival time, detected
/// via Hilbert transform of the cross-correlation.
#[derive(Debug, Clone)]
pub struct CrossCorrelationEnvelopeResult {
    /// Analytic envelope of the cross-correlation
    pub envelope: Vec<f32>,
    /// Sample index of the peak (integer arrival time)
    pub peak_sample: usize,
    /// Sub-sample refined peak position via parabolic interpolation
    pub peak_sample_refined: f64,
    /// Peak envelope value (proportional to channel gain)
    pub peak_value: f32,
    /// Arrival time in milliseconds (sub-sample precision)
    pub arrival_ms: f64,
}

/// Cross-correlate a probe with a recording and compute the analytic envelope.
///
/// Uses FFT-based cross-correlation followed by the Hilbert transform
/// (via `analytic_signal`) to extract a smooth envelope whose peak
/// indicates the arrival time with sub-sample precision.
///
/// This is the matched-filter approach recommended by Johnston (AES):
/// narrowband probes give excellent noise rejection, and the analytic
/// envelope provides a clean, unambiguous peak even in reverberant rooms.
///
/// # Arguments
/// * `probe` - The known probe signal that was played
/// * `recorded` - The recorded signal from the microphone
/// * `sample_rate` - Sample rate in Hz
pub fn cross_correlate_envelope(
    probe: &[f32],
    recorded: &[f32],
    sample_rate: u32,
) -> Result<CrossCorrelationEnvelopeResult, String> {
    if probe.is_empty() || recorded.is_empty() {
        return Err("Probe and recorded signals must be non-empty".to_string());
    }

    // Zero-pad to avoid circular correlation artifacts
    let fft_size = next_power_of_two(probe.len() + recorded.len());

    // Raw FFT (no normalization) — we handle normalization once after IFFT.
    // Using unnormalized FFT avoids the scale-dependent gain errors that
    // occur when compute_fft_padded's 1/N normalization interacts with IFFT.
    let fft_forward = plan_fft_forward(fft_size);

    let mut probe_buf: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); fft_size];
    for (dst, &src) in probe_buf.iter_mut().zip(probe.iter()) {
        dst.re = src;
    }
    fft_forward.process(&mut probe_buf);

    let mut rec_buf: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); fft_size];
    for (dst, &src) in rec_buf.iter_mut().zip(recorded.iter()) {
        dst.re = src;
    }
    fft_forward.process(&mut rec_buf);

    // Cross-correlation: conj(Probe) * Recorded
    let mut cross_fft: Vec<Complex<f32>> = probe_buf
        .iter()
        .zip(rec_buf.iter())
        .map(|(p, r)| p.conj() * r)
        .collect();

    // IFFT to get cross-correlation in time domain
    let ifft = plan_fft_inverse(fft_size);
    ifft.process(&mut cross_fft);

    // Single 1/N normalization (standard for round-trip FFT→IFFT)
    let norm = 1.0 / fft_size as f32;
    let xcorr: Vec<f32> = cross_fft.iter().map(|c| c.re * norm).collect();

    // Compute analytic envelope via Hilbert transform
    let analytic = crate::instantaneous_frequency::analytic_signal(&xcorr);
    let envelope: Vec<f32> = analytic.iter().map(|c| c.norm()).collect();

    // Find peak in the causal part (first half — positive lags only)
    let search_len = fft_size / 2;
    let mut peak_sample = 0_usize;
    let mut peak_value = 0.0_f32;
    for (i, &val) in envelope.iter().enumerate().take(search_len) {
        if val > peak_value {
            peak_value = val;
            peak_sample = i;
        }
    }

    // Parabolic interpolation for sub-sample precision
    let peak_refined = if peak_sample > 0 && peak_sample < search_len - 1 {
        let y_prev = envelope[peak_sample - 1] as f64;
        let y_peak = envelope[peak_sample] as f64;
        let y_next = envelope[peak_sample + 1] as f64;
        let denom = 2.0 * (2.0 * y_peak - y_prev - y_next);
        if denom.abs() > 1e-12 {
            peak_sample as f64 + (y_prev - y_next) / denom
        } else {
            peak_sample as f64
        }
    } else {
        peak_sample as f64
    };

    let arrival_ms = peak_refined / sample_rate as f64 * 1000.0;

    Ok(CrossCorrelationEnvelopeResult {
        envelope,
        peak_sample,
        peak_sample_refined: peak_refined,
        peak_value,
        arrival_ms,
    })
}

/// Frequency responses computed from different time windows of an impulse response.
///
/// Direct sound, early reflections, and late reverb each have different
/// perceptual roles (Toole, Johnston) and should be corrected differently.
#[derive(Debug, Clone)]
pub struct WindowedFrequencyResponse {
    /// Direct sound frequency response (frequencies in Hz, SPL in dB)
    pub direct_sound_freq: Vec<f32>,
    pub direct_sound_spl: Vec<f32>,
    /// Early reflections frequency response
    pub early_reflections_freq: Vec<f32>,
    pub early_reflections_spl: Vec<f32>,
    /// Late/reverberant field frequency response
    pub late_reverb_freq: Vec<f32>,
    pub late_reverb_spl: Vec<f32>,
    /// Time boundaries used (in ms)
    pub direct_end_ms: f64,
    pub early_end_ms: f64,
}

/// Compute frequency responses for different time windows of the impulse response.
///
/// Uses SSIR segmentation boundaries to separate:
/// - Direct sound: \[0, first_reflection_onset)
/// - Early reflections: \[first_reflection_onset, mixing_time)
/// - Late reverb: \[mixing_time, end)
///
/// Each window gets a half-Hann fade at edges to avoid spectral leakage,
/// then FFT -> magnitude -> 1/24 octave smoothing.
pub fn compute_windowed_fr(
    impulse_response: &[f32],
    direct_end_sample: usize,
    early_end_sample: usize,
    sample_rate: u32,
    num_output_points: usize,
) -> Result<WindowedFrequencyResponse, String> {
    if impulse_response.is_empty() {
        return Err("Impulse response must be non-empty".to_string());
    }
    if num_output_points == 0 {
        return Err("num_output_points must be > 0".to_string());
    }

    let ir_len = impulse_response.len();
    let direct_end = direct_end_sample.min(ir_len);
    let early_end = early_end_sample.max(direct_end).min(ir_len);

    let direct_end_ms = direct_end as f64 / sample_rate as f64 * 1000.0;
    let early_end_ms = early_end as f64 / sample_rate as f64 * 1000.0;

    // Fade length: 1ms or half the window, whichever is smaller
    let fade_1ms = (sample_rate as usize) / 1000;

    let window_to_fr = |start: usize, end: usize| -> (Vec<f32>, Vec<f32>) {
        let win_len = end.saturating_sub(start);
        if win_len == 0 {
            // Return silence at the output frequencies
            let log_start = 20.0_f32.ln();
            let log_end = 20000.0_f32.ln();
            let freqs: Vec<f32> = (0..num_output_points)
                .map(|i| {
                    (log_start
                        + (log_end - log_start) * i as f32 / (num_output_points.max(2) - 1) as f32)
                        .exp()
                })
                .collect();
            let spl = vec![-200.0_f32; num_output_points];
            return (freqs, spl);
        }

        // Extract and fade the window edges to reduce spectral leakage.
        // Skip fade-in at the physical start of the IR (start==0) to avoid
        // attenuating the direct sound impulse.
        let mut window: Vec<f32> = impulse_response[start..end].to_vec();
        let fade_len = fade_1ms.min(win_len / 2).max(1);
        if start > 0 {
            crate::signals::apply_fade_in(&mut window, fade_len);
        }
        crate::signals::apply_fade_out(&mut window, fade_len);

        // Zero-pad to next power of 2
        let fft_size = next_power_of_two(win_len);
        let fft_forward = plan_fft_forward(fft_size);

        let mut buf: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); fft_size];
        for (dst, &src) in buf.iter_mut().zip(window.iter()) {
            dst.re = src;
        }
        fft_forward.process(&mut buf);

        // Normalize by FFT size
        let norm = 1.0 / fft_size as f32;

        // Generate log-spaced output frequencies and compute magnitude in dB
        let log_start = 20.0_f32.ln();
        let log_end = 20000.0_f32.ln();
        let freq_resolution = sample_rate as f32 / fft_size as f32;
        let num_bins = fft_size / 2;

        let mut freqs = Vec::with_capacity(num_output_points);
        let mut raw_db = Vec::with_capacity(num_output_points);

        for i in 0..num_output_points {
            let target_freq = (log_start
                + (log_end - log_start) * i as f32 / (num_output_points.max(2) - 1) as f32)
                .exp();
            freqs.push(target_freq);

            // Map to nearest FFT bin
            let bin = ((target_freq / freq_resolution).round() as usize).clamp(1, num_bins - 1);
            let mag = buf[bin].norm() * norm;
            let db = if mag > 1e-20 {
                20.0 * mag.log10()
            } else {
                -200.0
            };
            raw_db.push(db);
        }

        // Apply 1/24 octave smoothing
        let smoothed = smooth_response_f32(&freqs, &raw_db, 1.0 / 24.0);
        (freqs, smoothed)
    };

    let (direct_sound_freq, direct_sound_spl) = window_to_fr(0, direct_end);
    let (early_reflections_freq, early_reflections_spl) = window_to_fr(direct_end, early_end);
    let (late_reverb_freq, late_reverb_spl) = window_to_fr(early_end, ir_len);

    Ok(WindowedFrequencyResponse {
        direct_sound_freq,
        direct_sound_spl,
        early_reflections_freq,
        early_reflections_spl,
        late_reverb_freq,
        late_reverb_spl,
        direct_end_ms,
        early_end_ms,
    })
}

/// Compute FFT of a signal with specified windowing
///
/// # Arguments
/// * `signal` - Input signal
/// * `fft_size` - FFT size (should be power of 2)
/// * `window_type` - Type of window to apply
///
/// # Returns
/// Complex FFT spectrum
fn compute_fft(
    signal: &[f32],
    fft_size: usize,
    window_type: WindowType,
) -> Result<Vec<Complex<f32>>, String> {
    // Apply window
    let windowed = match window_type {
        WindowType::Hann => apply_hann_window(signal),
        WindowType::Tukey(alpha) => apply_tukey_window(signal, alpha),
    };

    compute_fft_padded(&windowed, fft_size)
}

/// Compute FFT with zero-padding
fn compute_fft_padded(signal: &[f32], fft_size: usize) -> Result<Vec<Complex<f32>>, String> {
    // Single allocation at final size; trailing elements are already zero-padded
    let mut buffer = vec![Complex::new(0.0, 0.0); fft_size];
    for (dst, &src) in buffer.iter_mut().zip(signal.iter()) {
        dst.re = src;
    }

    // Compute FFT
    let fft = plan_fft_forward(fft_size);
    fft.process(&mut buffer);

    // Normalize by FFT size (standard FFT normalization)
    let norm_factor = 1.0 / fft_size as f32;
    for val in buffer.iter_mut() {
        *val *= norm_factor;
    }

    Ok(buffer)
}

/// Apply Hann window to a signal
fn apply_hann_window(signal: &[f32]) -> Vec<f32> {
    let len = signal.len();
    if len < 2 {
        return signal.to_vec();
    }
    signal
        .iter()
        .enumerate()
        .map(|(i, &x)| {
            let window = 0.5 * (1.0 - (2.0 * PI * i as f32 / (len - 1) as f32).cos());
            x * window
        })
        .collect()
}

/// Apply Tukey window to a signal
///
/// Tukey window is a "tapered cosine" window.
/// alpha=0.0 is rectangular, alpha=1.0 is Hann.
fn apply_tukey_window(signal: &[f32], alpha: f32) -> Vec<f32> {
    let len = signal.len();
    if len < 2 {
        return signal.to_vec();
    }

    let alpha = alpha.clamp(0.0, 1.0);
    let limit = (alpha * (len as f32 - 1.0) / 2.0).round() as usize;

    if limit == 0 {
        return signal.to_vec();
    }

    signal
        .iter()
        .enumerate()
        .map(|(i, &x)| {
            let w = if i < limit {
                // Fade in (Half-Hann)
                0.5 * (1.0 - (PI * i as f32 / limit as f32).cos())
            } else if i >= len - limit {
                // Fade out (Half-Hann)
                let n = len - 1 - i;
                0.5 * (1.0 - (PI * n as f32 / limit as f32).cos())
            } else {
                // Flat top
                1.0
            };
            x * w
        })
        .collect()
}

/// Find the next power of two greater than or equal to n
fn next_power_of_two(n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    n.next_power_of_two()
}

/// Load a mono WAV file and convert to f32 samples
/// Load a WAV file and extract a specific channel or convert to mono
///
/// # Arguments
/// * `path` - Path to WAV file
/// * `channel_index` - Optional channel index to extract (0-based). If None, will average all channels for mono
fn load_wav_mono_channel(path: &Path, channel_index: Option<usize>) -> Result<Vec<f32>, String> {
    let mut reader =
        WavReader::open(path).map_err(|e| format!("Failed to open WAV file: {}", e))?;

    let spec = reader.spec();
    let channels = spec.channels as usize;

    log::info!(
        "[load_wav_mono_channel] WAV file: {} channels, {} Hz, {:?} format",
        channels,
        spec.sample_rate,
        spec.sample_format
    );

    // Read all samples and convert to f32
    let samples: Result<Vec<f32>, _> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect(),
        hound::SampleFormat::Int => reader
            .samples::<i32>()
            .map(|s| s.map(|v| v as f32 / i32::MAX as f32))
            .collect(),
    };

    let samples = samples.map_err(|e| format!("Failed to read samples: {}", e))?;
    log::info!(
        "[load_wav_mono_channel] Read {} total samples",
        samples.len()
    );

    // Handle mono file - return as-is
    if channels == 1 {
        log::info!(
            "[load_wav_mono_channel] File is already mono, returning {} samples",
            samples.len()
        );
        return Ok(samples);
    }

    // Handle multi-channel file
    if let Some(ch_idx) = channel_index {
        // Extract specific channel
        if ch_idx >= channels {
            return Err(format!(
                "Channel index {} out of range (file has {} channels)",
                ch_idx, channels
            ));
        }
        log::info!(
            "[load_wav_mono_channel] Extracting channel {} from {} channels",
            ch_idx,
            channels
        );
        Ok(samples
            .chunks(channels)
            .map(|chunk| chunk[ch_idx])
            .collect())
    } else {
        // Average all channels to mono
        log::info!(
            "[load_wav_mono_channel] Averaging {} channels to mono",
            channels
        );
        Ok(samples
            .chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect())
    }
}

/// Load a WAV file as mono (averages channels if multi-channel)
fn load_wav_mono(path: &Path) -> Result<Vec<f32>, String> {
    load_wav_mono_channel(path, None)
}

// ============================================================================
// DSP Utilities (Moved from frontend dsp.rs)
// ============================================================================

/// Apply octave smoothing to frequency response data (f64 version)
///
/// Frequencies must be sorted in ascending order (as from FFT or log-spaced grids).
/// Uses a prefix sum with two-pointer sliding window for O(n) complexity.
pub fn smooth_response_f64(frequencies: &[f64], values: &[f64], octaves: f64) -> Vec<f64> {
    if octaves <= 0.0 || frequencies.is_empty() || values.is_empty() {
        return values.to_vec();
    }

    let n = values.len();

    // Prefix sum for O(1) range averages
    let mut prefix = Vec::with_capacity(n + 1);
    prefix.push(0.0);
    for &v in values {
        prefix.push(prefix.last().unwrap() + v);
    }

    let ratio = 2.0_f64.powf(octaves / 2.0);
    let mut smoothed = Vec::with_capacity(n);
    let mut lo = 0usize;
    let mut hi = 0usize;

    for (i, &center_freq) in frequencies.iter().enumerate() {
        if center_freq <= 0.0 {
            smoothed.push(values[i]);
            continue;
        }

        let low_freq = center_freq / ratio;
        let high_freq = center_freq * ratio;

        // Advance lo past frequencies below the window
        while lo < n && frequencies[lo] < low_freq {
            lo += 1;
        }
        // Advance hi to include frequencies within the window
        while hi < n && frequencies[hi] <= high_freq {
            hi += 1;
        }

        let count = hi - lo;
        if count > 0 {
            smoothed.push((prefix[hi] - prefix[lo]) / count as f64);
        } else {
            smoothed.push(values[i]);
        }
    }

    smoothed
}

/// Apply octave smoothing to frequency response data (f32 version)
///
/// Frequencies must be sorted in ascending order (as from FFT or log-spaced grids).
/// Uses a prefix sum with two-pointer sliding window for O(n) complexity.
pub fn smooth_response_f32(frequencies: &[f32], values: &[f32], octaves: f32) -> Vec<f32> {
    if octaves <= 0.0 || frequencies.is_empty() || values.is_empty() {
        return values.to_vec();
    }

    let n = values.len();

    // Prefix sum for O(1) range averages (accumulate in f64 to avoid precision loss)
    let mut prefix = Vec::with_capacity(n + 1);
    prefix.push(0.0_f64);
    for &v in values {
        prefix.push(prefix.last().unwrap() + v as f64);
    }

    let ratio = 2.0_f32.powf(octaves / 2.0);
    let mut smoothed = Vec::with_capacity(n);
    let mut lo = 0usize;
    let mut hi = 0usize;

    for (i, &center_freq) in frequencies.iter().enumerate() {
        if center_freq <= 0.0 {
            smoothed.push(values[i]);
            continue;
        }

        let low_freq = center_freq / ratio;
        let high_freq = center_freq * ratio;

        // Advance lo past frequencies below the window
        while lo < n && frequencies[lo] < low_freq {
            lo += 1;
        }
        // Advance hi to include frequencies within the window
        while hi < n && frequencies[hi] <= high_freq {
            hi += 1;
        }

        let count = hi - lo;
        if count > 0 {
            smoothed.push(((prefix[hi] - prefix[lo]) / count as f64) as f32);
        } else {
            smoothed.push(values[i]);
        }
    }

    smoothed
}

/// Compute group delay from phase data
/// Group delay = -d(phase)/d(frequency) / (2*pi)
///
/// Phase is unwrapped before differentiation to avoid spurious spikes
/// at ±180° wrap boundaries.
pub fn compute_group_delay(frequencies: &[f32], phase_deg: &[f32]) -> Vec<f32> {
    if frequencies.len() < 2 {
        return vec![0.0; frequencies.len()];
    }

    // Unwrap phase to remove ±180° discontinuities before differentiation
    let unwrapped = unwrap_phase_deg(phase_deg);

    let mut group_delay_ms = Vec::with_capacity(frequencies.len());

    for i in 0..frequencies.len() {
        let delay = if i == 0 {
            // Forward difference at start
            let df = frequencies[1] - frequencies[0];
            let dp = unwrapped[1] - unwrapped[0];
            if df.abs() > 1e-6 {
                -dp / df / 360.0 * 1000.0 // Convert to ms
            } else {
                0.0
            }
        } else if i == frequencies.len() - 1 {
            // Backward difference at end
            let df = frequencies[i] - frequencies[i - 1];
            let dp = unwrapped[i] - unwrapped[i - 1];
            if df.abs() > 1e-6 {
                -dp / df / 360.0 * 1000.0
            } else {
                0.0
            }
        } else {
            // Central difference
            let df = frequencies[i + 1] - frequencies[i - 1];
            let dp = unwrapped[i + 1] - unwrapped[i - 1];
            if df.abs() > 1e-6 {
                -dp / df / 360.0 * 1000.0
            } else {
                0.0
            }
        };
        group_delay_ms.push(delay);
    }

    group_delay_ms
}

/// Unwrap phase in degrees to produce a continuous phase curve.
/// Wraps each inter-sample difference to [-180, 180] and accumulates,
/// handling arbitrarily large jumps (not just single ±360° wraps).
fn unwrap_phase_deg(phase_deg: &[f32]) -> Vec<f32> {
    if phase_deg.is_empty() {
        return Vec::new();
    }

    let mut unwrapped = Vec::with_capacity(phase_deg.len());
    unwrapped.push(phase_deg[0]);

    for i in 1..phase_deg.len() {
        let diff = phase_deg[i] - phase_deg[i - 1];
        let wrapped_diff = diff - 360.0 * (diff / 360.0).round();
        unwrapped.push(unwrapped[i - 1] + wrapped_diff);
    }

    unwrapped
}

/// Compute impulse response from frequency response via inverse FFT.
///
/// The input frequency/magnitude/phase data (possibly irregularly spaced) is
/// interpolated onto a uniform FFT frequency grid, assembled into a complex
/// spectrum with Hermitian symmetry, and transformed with an inverse FFT.
///
/// Returns (times_ms, impulse) where impulse is peak-normalized to [-1, 1].
pub fn compute_impulse_response_from_fr(
    frequencies: &[f32],
    magnitude_db: &[f32],
    phase_deg: &[f32],
    sample_rate: f32,
) -> (Vec<f32>, Vec<f32>) {
    let fft_size = 1024;
    let half = fft_size / 2; // Number of positive-frequency bins (excluding DC)
    let freq_bin = sample_rate / fft_size as f32;

    // Unwrap phase before interpolation to avoid discontinuities
    let unwrapped_phase = unwrap_phase_deg(phase_deg);

    // Build complex spectrum on uniform FFT grid via linear interpolation
    let mut spectrum = vec![Complex::new(0.0_f32, 0.0); fft_size];

    for (k, spectrum_bin) in spectrum.iter_mut().enumerate().take(half + 1) {
        let f = k as f32 * freq_bin;

        // Interpolate magnitude (dB) and phase (deg) at this bin frequency
        let (mag_db, phase_d) = interpolate_fr(frequencies, magnitude_db, &unwrapped_phase, f);

        let mag_linear = 10.0_f32.powf(mag_db / 20.0);
        let phase_rad = phase_d * PI / 180.0;

        *spectrum_bin = Complex::new(mag_linear * phase_rad.cos(), mag_linear * phase_rad.sin());
    }

    // Enforce Hermitian symmetry: X[N-k] = conj(X[k])
    for k in 1..half {
        spectrum[fft_size - k] = spectrum[k].conj();
    }

    // Inverse FFT (uses thread-local cached planner)
    let ifft = plan_fft_inverse(fft_size);
    ifft.process(&mut spectrum);

    // Extract real part and scale by 1/N (rustfft doesn't normalize)
    let scale = 1.0 / fft_size as f32;
    let mut impulse: Vec<f32> = spectrum.iter().map(|c| c.re * scale).collect();

    // Normalize to [-1, 1]
    let max_val = impulse.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
    if max_val > 0.0 {
        for v in &mut impulse {
            *v /= max_val;
        }
    }

    let time_step = 1.0 / sample_rate;
    let times: Vec<f32> = (0..fft_size)
        .map(|i| i as f32 * time_step * 1000.0)
        .collect();

    (times, impulse)
}

/// Linearly interpolate magnitude and phase at a target frequency.
/// Clamps to the nearest endpoint if `target_freq` is outside the data range.
///
/// Phase must be pre-unwrapped (continuous) for correct interpolation.
fn interpolate_fr(
    frequencies: &[f32],
    magnitude_db: &[f32],
    unwrapped_phase_deg: &[f32],
    target_freq: f32,
) -> (f32, f32) {
    if frequencies.is_empty() {
        return (0.0, 0.0);
    }
    if target_freq <= frequencies[0] {
        return (magnitude_db[0], unwrapped_phase_deg[0]);
    }
    let last = frequencies.len() - 1;
    if target_freq >= frequencies[last] {
        return (magnitude_db[last], unwrapped_phase_deg[last]);
    }

    // Binary search for the interval containing target_freq
    let idx = match frequencies.binary_search_by(|f| f.partial_cmp(&target_freq).unwrap()) {
        Ok(i) => return (magnitude_db[i], unwrapped_phase_deg[i]),
        Err(i) => i, // target_freq is between frequencies[i-1] and frequencies[i]
    };

    let f0 = frequencies[idx - 1];
    let f1 = frequencies[idx];
    let t = (target_freq - f0) / (f1 - f0);

    let mag = magnitude_db[idx - 1] + t * (magnitude_db[idx] - magnitude_db[idx - 1]);
    let phase = unwrapped_phase_deg[idx - 1]
        + t * (unwrapped_phase_deg[idx] - unwrapped_phase_deg[idx - 1]);
    (mag, phase)
}

/// Compute Schroeder energy decay curve
fn compute_schroeder_decay(impulse: &[f32]) -> Vec<f32> {
    let mut energy = 0.0;
    let mut decay = vec![0.0; impulse.len()];

    // Backward integration
    for i in (0..impulse.len()).rev() {
        energy += impulse[i] * impulse[i];
        decay[i] = energy;
    }

    // Normalize to 0dB max (1.0 linear)
    let max_energy = decay.first().copied().unwrap_or(1.0);
    if max_energy > 0.0 {
        for v in &mut decay {
            *v /= max_energy;
        }
    }

    decay
}

/// Compute RT60 from Impulse Response (Broadband)
/// Uses T20 (-5dB to -25dB) extrapolation
pub fn compute_rt60_broadband(impulse: &[f32], sample_rate: f32) -> f32 {
    let decay = compute_schroeder_decay(impulse);
    let decay_db: Vec<f32> = decay.iter().map(|&v| 10.0 * v.max(1e-9).log10()).collect();

    // Find -5dB and -25dB points
    let t_minus_5 = decay_db.iter().position(|&v| v < -5.0);
    let t_minus_25 = decay_db.iter().position(|&v| v < -25.0);

    match (t_minus_5, t_minus_25) {
        (Some(start), Some(end)) if end > start => {
            let dt = (end - start) as f32 / sample_rate; // Time for 20dB decay
            dt * 3.0 // Extrapolate to 60dB (T20 * 3)
        }
        _ => 0.0,
    }
}

/// Compute Clarity (C50, C80) from Impulse Response (Broadband)
/// Returns (C50_dB, C80_dB)
pub fn compute_clarity_broadband(impulse: &[f32], sample_rate: f32) -> (f32, f32) {
    let mut energy_0_50 = 0.0;
    let mut energy_50_inf = 0.0;
    let mut energy_0_80 = 0.0;
    let mut energy_80_inf = 0.0;

    let samp_50ms = (0.050 * sample_rate) as usize;
    let samp_80ms = (0.080 * sample_rate) as usize;

    for (i, &samp) in impulse.iter().enumerate() {
        let sq = samp * samp;

        if i < samp_50ms {
            energy_0_50 += sq;
        } else {
            energy_50_inf += sq;
        }

        if i < samp_80ms {
            energy_0_80 += sq;
        } else {
            energy_80_inf += sq;
        }
    }

    // When late energy is negligible, clarity is very high (capped at 60 dB)
    // When early energy is negligible, clarity is very low (capped at -60 dB)
    const MAX_CLARITY_DB: f32 = 60.0;

    let c50 = if energy_50_inf > 1e-12 && energy_0_50 > 1e-12 {
        let ratio = energy_0_50 / energy_50_inf;
        (10.0 * ratio.log10()).clamp(-MAX_CLARITY_DB, MAX_CLARITY_DB)
    } else if energy_0_50 > energy_50_inf {
        MAX_CLARITY_DB // Early energy dominates - excellent clarity
    } else {
        -MAX_CLARITY_DB // Late energy dominates - poor clarity
    };

    let c80 = if energy_80_inf > 1e-12 && energy_0_80 > 1e-12 {
        let ratio = energy_0_80 / energy_80_inf;
        (10.0 * ratio.log10()).clamp(-MAX_CLARITY_DB, MAX_CLARITY_DB)
    } else if energy_80_inf > energy_0_80 {
        MAX_CLARITY_DB // Early energy dominates - excellent clarity
    } else {
        -MAX_CLARITY_DB // Late energy dominates - poor clarity
    };

    (c50, c80)
}

/// Compute RT60 spectrum using octave band filtering
pub fn compute_rt60_spectrum(impulse: &[f32], sample_rate: f32, frequencies: &[f32]) -> Vec<f32> {
    if impulse.is_empty() {
        return vec![0.0; frequencies.len()];
    }

    // Octave band center frequencies
    let centers = [
        63.0f32, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
    ];
    let mut band_rt60s = Vec::with_capacity(centers.len());
    let mut valid_centers = Vec::with_capacity(centers.len());

    // Compute RT60 for each band
    for &freq in &centers {
        // Skip if frequency is too high for sample rate
        if freq >= sample_rate / 2.0 {
            continue;
        }

        // Apply bandpass filter
        // Q=1.414 (sqrt(2)) gives approx 1 octave bandwidth
        let mut biquad = Biquad::new(
            BiquadFilterType::Bandpass,
            freq as f64,
            sample_rate as f64,
            1.414,
            0.0,
        );

        // Process in f64
        let mut filtered: Vec<f64> = impulse.iter().map(|&x| x as f64).collect();
        biquad.process_block(&mut filtered);
        let filtered_f32: Vec<f32> = filtered.iter().map(|&x| x as f32).collect();

        // Compute RT60 for this band
        let rt60 = compute_rt60_broadband(&filtered_f32, sample_rate);

        band_rt60s.push(rt60);
        valid_centers.push(freq);
    }

    // Log per-band values
    log::info!(
        "[RT60] Per-band values: {:?}",
        valid_centers
            .iter()
            .zip(band_rt60s.iter())
            .map(|(f, v)| format!("{:.0}Hz:{:.1}ms", f, v))
            .collect::<Vec<_>>()
    );

    if valid_centers.is_empty() {
        return vec![0.0; frequencies.len()];
    }

    // Interpolate to output frequencies
    interpolate_log(&valid_centers, &band_rt60s, frequencies)
}

/// Compute Clarity spectrum (C50, C80) using octave band filtering
/// Returns (C50_vec, C80_vec)
pub fn compute_clarity_spectrum(
    impulse: &[f32],
    sample_rate: f32,
    frequencies: &[f32],
) -> (Vec<f32>, Vec<f32>) {
    if impulse.is_empty() || frequencies.is_empty() {
        return (vec![0.0; frequencies.len()], vec![0.0; frequencies.len()]);
    }

    // Octave band center frequencies
    let centers = [
        63.0f32, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
    ];
    let mut band_c50s = Vec::with_capacity(centers.len());
    let mut band_c80s = Vec::with_capacity(centers.len());
    let mut valid_centers = Vec::with_capacity(centers.len());

    // Time boundaries for clarity calculation
    let samp_50ms = (0.050 * sample_rate) as usize;
    let samp_80ms = (0.080 * sample_rate) as usize;

    // Compute Clarity for each band using cascaded bandpass for better selectivity
    for &freq in &centers {
        if freq >= sample_rate / 2.0 {
            continue;
        }

        // Use cascaded biquads for sharper filter response (reduces filter ringing effects)
        let mut biquad1 = Biquad::new(
            BiquadFilterType::Bandpass,
            freq as f64,
            sample_rate as f64,
            0.707, // Lower Q per stage, cascaded gives Q ~ 1.0
            0.0,
        );
        let mut biquad2 = Biquad::new(
            BiquadFilterType::Bandpass,
            freq as f64,
            sample_rate as f64,
            0.707,
            0.0,
        );

        let mut filtered: Vec<f64> = impulse.iter().map(|&x| x as f64).collect();
        biquad1.process_block(&mut filtered);
        biquad2.process_block(&mut filtered);

        // Compute energy in early and late windows directly
        let mut energy_0_50 = 0.0f64;
        let mut energy_50_inf = 0.0f64;
        let mut energy_0_80 = 0.0f64;
        let mut energy_80_inf = 0.0f64;

        for (i, &samp) in filtered.iter().enumerate() {
            let sq = samp * samp;

            if i < samp_50ms {
                energy_0_50 += sq;
            } else {
                energy_50_inf += sq;
            }

            if i < samp_80ms {
                energy_0_80 += sq;
            } else {
                energy_80_inf += sq;
            }
        }

        // Compute C50 and C80 with proper handling
        // When late energy is very small, clarity is high (capped at 40 dB for display)
        const MAX_CLARITY_DB: f32 = 40.0;
        const MIN_ENERGY: f64 = 1e-20;

        let c50 = if energy_50_inf > MIN_ENERGY && energy_0_50 > MIN_ENERGY {
            let ratio = energy_0_50 / energy_50_inf;
            (10.0 * ratio.log10() as f32).clamp(-MAX_CLARITY_DB, MAX_CLARITY_DB)
        } else if energy_0_50 > energy_50_inf {
            MAX_CLARITY_DB
        } else {
            -MAX_CLARITY_DB
        };

        let c80 = if energy_80_inf > MIN_ENERGY && energy_0_80 > MIN_ENERGY {
            let ratio = energy_0_80 / energy_80_inf;
            (10.0 * ratio.log10() as f32).clamp(-MAX_CLARITY_DB, MAX_CLARITY_DB)
        } else if energy_0_80 > energy_80_inf {
            MAX_CLARITY_DB
        } else {
            -MAX_CLARITY_DB
        };

        band_c50s.push(c50);
        band_c80s.push(c80);
        valid_centers.push(freq);
    }

    // Log per-band values
    log::info!(
        "[Clarity] Per-band C50: {:?}",
        valid_centers
            .iter()
            .zip(band_c50s.iter())
            .map(|(f, v)| format!("{:.0}Hz:{:.1}dB", f, v))
            .collect::<Vec<_>>()
    );

    if valid_centers.is_empty() {
        return (vec![0.0; frequencies.len()], vec![0.0; frequencies.len()]);
    }

    // Interpolate to output frequency grid
    let c50_interp = interpolate_log(&valid_centers, &band_c50s, frequencies);
    let c80_interp = interpolate_log(&valid_centers, &band_c80s, frequencies);

    (c50_interp, c80_interp)
}

/// Compute Spectrogram from Impulse Response
/// Returns (spectrogram_matrix_db, frequency_bins, time_bins)
/// `window_size` samples (e.g. 512), `hop_size` samples (e.g. 128).
pub fn compute_spectrogram(
    impulse: &[f32],
    sample_rate: f32,
    window_size: usize,
    hop_size: usize,
) -> (Vec<Vec<f32>>, Vec<f32>, Vec<f32>) {
    use rustfft::num_complex::Complex;

    if impulse.len() < window_size {
        return (Vec::new(), Vec::new(), Vec::new());
    }

    let num_frames = (impulse.len() - window_size) / hop_size;
    let mut spectrogram = Vec::with_capacity(num_frames);
    let mut times = Vec::with_capacity(num_frames);

    // Precompute Hann window
    let window: Vec<f32> = (0..window_size)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (window_size as f32 - 1.0)).cos()))
        .collect();

    // Setup FFT
    let fft = plan_fft_forward(window_size);

    for i in 0..num_frames {
        let start = i * hop_size;
        let time_ms = (start as f32 / sample_rate) * 1000.0;
        times.push(time_ms);

        let mut buffer: Vec<Complex<f32>> = (0..window_size)
            .map(|j| {
                let sample = impulse.get(start + j).copied().unwrap_or(0.0);
                Complex::new(sample * window[j], 0.0)
            })
            .collect();

        fft.process(&mut buffer);

        // Take magnitude of first half (up to Nyquist)
        // Store as dB
        let magnitude_db: Vec<f32> = buffer[..window_size / 2]
            .iter()
            .map(|c| {
                let mag = c.norm();
                if mag > 1e-9 {
                    20.0 * mag.log10()
                } else {
                    -180.0
                }
            })
            .collect();

        spectrogram.push(magnitude_db);
    }

    // Generate frequency bins
    let num_bins = window_size / 2;
    let freq_step = sample_rate / window_size as f32;
    let freqs: Vec<f32> = (0..num_bins).map(|i| i as f32 * freq_step).collect();

    (spectrogram, freqs, times)
}

/// Find a frequency point where the magnitude reaches a specific dB level
///
/// # Arguments
/// * `frequencies` - Frequency points in Hz
/// * `magnitude_db` - Magnitude in dB
/// * `target_db` - The target level to find (e.g., -3.0)
/// * `from_start` - If true, search from the beginning of the curve. If false, search from the end.
///
/// # Returns
/// The interpolated frequency where the target dB is reached, or None if not found.
pub fn find_db_point(
    frequencies: &[f32],
    magnitude_db: &[f32],
    target_db: f32,
    from_start: bool,
) -> Option<f32> {
    if frequencies.len() < 2 || frequencies.len() != magnitude_db.len() {
        return None;
    }

    if from_start {
        for i in 0..magnitude_db.len() - 1 {
            let m0 = magnitude_db[i];
            let m1 = magnitude_db[i + 1];

            // Check if target_db is between m0 and m1
            if (m0 <= target_db && target_db <= m1) || (m1 <= target_db && target_db <= m0) {
                // Linear interpolation: m0 + t * (m1 - m0) = target_db
                let denominator = m1 - m0;
                if denominator.abs() < 1e-9 {
                    return Some(frequencies[i]);
                }
                let t = (target_db - m0) / denominator;
                return Some(frequencies[i] + t * (frequencies[i + 1] - frequencies[i]));
            }
        }
    } else {
        for i in (1..magnitude_db.len()).rev() {
            let m0 = magnitude_db[i];
            let m1 = magnitude_db[i - 1];

            // Check if target_db is between m0 and m1
            if (m0 <= target_db && target_db <= m1) || (m1 <= target_db && target_db <= m0) {
                let denominator = m1 - m0;
                if denominator.abs() < 1e-9 {
                    return Some(frequencies[i]);
                }
                let t = (target_db - m0) / denominator;
                return Some(frequencies[i] + t * (frequencies[i - 1] - frequencies[i]));
            }
        }
    }

    None
}

/// Compute a log-frequency weighted reference response level in dB.
///
/// # Arguments
/// * `frequencies` - Frequency points in Hz
/// * `magnitude_db` - Magnitude in dB
/// * `freq_range` - Optional (start_freq, end_freq) to limit the averaging range.
///   If None, averages over the full bandwidth.
///
/// # Returns
/// The log-frequency weighted average in the dB domain.
///
/// This is intended as a stable acoustic reference level for comparison and
/// normalization. It is not a pressure- or energy-domain average.
pub fn compute_average_response(
    frequencies: &[f32],
    magnitude_db: &[f32],
    freq_range: Option<(f32, f32)>,
) -> f32 {
    if frequencies.len() < 2 || frequencies.len() != magnitude_db.len() {
        return magnitude_db.first().copied().unwrap_or(0.0);
    }

    let (start_freq, end_freq) =
        freq_range.unwrap_or((frequencies[0], frequencies[frequencies.len() - 1]));

    let mut sum_weighted_db = 0.0;
    let mut sum_weights = 0.0;

    for i in 0..frequencies.len() - 1 {
        let f0 = frequencies[i];
        let f1 = frequencies[i + 1];

        // Check if this segment overlaps with the target range
        if f1 < start_freq || f0 > end_freq {
            continue;
        }

        // Clamp segment to target range
        let fa = f0.max(start_freq);
        let fb = f1.min(end_freq);

        if fb <= fa {
            continue;
        }

        // For acoustic data, we weight by log frequency (octaves)
        // weight = log2(fb/fa)
        let weight = (fb / fa).log2();

        // Average magnitude in this segment
        // We'll use the midpoint value of the segment (or average of endpoints)
        // If the segment is partially outside start_freq/end_freq, we should interpolate
        // but for many points simple average of endpoints in the segment is fine.
        let m0 = magnitude_db[i];
        let m1 = magnitude_db[i + 1];
        let avg_m = (m0 + m1) / 2.0;

        sum_weighted_db += avg_m * weight;
        sum_weights += weight;
    }

    if sum_weights > 0.0 {
        sum_weighted_db / sum_weights
    } else {
        magnitude_db.first().copied().unwrap_or(0.0)
    }
}

// ---------------------------------------------------------------------------
// GD-Opt v2 Phase GD-1c — multi-sweep coherence + noise-floor primitives
//
// These three pure functions let callers (sotf-engine's
// `record_multi_sweep`) turn a batch of N recorded sweeps + a
// pre-silence noise-floor window into the extended `Curve` columns
// `coherence` and `noise_floor_db` documented in §2.3 / §2.4 of
// `docs/gd_opt_v2_plan.md`.
// ---------------------------------------------------------------------------

/// Magnitude-squared coherence γ²(f) across N complex spectra that
/// should be measurements of the same deterministic transfer
/// function. Per `docs/gd_opt_v2_plan.md` §2.2:
///
/// ```text
///    γ²(f) = |H̄(f)|² / ⟨|H(f)|²⟩
///    H̄(f) = (1/N) Σ_i H_i(f)              (complex average)
///    ⟨|H(f)|²⟩ = (1/N) Σ_i |H_i(f)|²       (power average)
/// ```
///
/// Returns a vector of per-bin γ² in `[0, 1]`. For N = 1 γ² ≡ 1 by
/// construction — a single measurement is trivially "consistent with
/// itself" so callers must enforce the "at least 4 sweeps" rule at
/// a higher level (the `"insufficient_bass_duration"` advisory in
/// GD-1g).
///
/// Returns `Err` iff `realizations` is empty or any realization has
/// a different length than the first.
pub fn compute_coherence_from_realizations(
    realizations: &[Vec<Complex<f32>>],
) -> Result<Vec<f32>, String> {
    let n = realizations.len();
    if n == 0 {
        return Err("compute_coherence: empty realizations".to_string());
    }
    let bins = realizations[0].len();
    if bins == 0 {
        return Ok(Vec::new());
    }
    for (i, r) in realizations.iter().enumerate() {
        if r.len() != bins {
            return Err(format!(
                "compute_coherence: realization {i} has {} bins, expected {bins}",
                r.len()
            ));
        }
    }

    let mut coherence = Vec::with_capacity(bins);
    for k in 0..bins {
        let mut sum = Complex::new(0.0_f64, 0.0_f64);
        let mut sum_sq = 0.0_f64;
        for r in realizations {
            let h = Complex::new(r[k].re as f64, r[k].im as f64);
            sum += h;
            sum_sq += h.re * h.re + h.im * h.im;
        }
        let mean = sum / (n as f64);
        let mean_sq = sum_sq / (n as f64);
        if mean_sq <= f64::EPSILON {
            // Dead bin — report as zero-confidence rather than NaN so
            // downstream thresholds just drop it.
            coherence.push(0.0);
        } else {
            let coh = (mean.norm_sqr() / mean_sq).clamp(0.0, 1.0);
            coherence.push(coh as f32);
        }
    }

    Ok(coherence)
}

/// Deconvolve a single recorded log sweep by dividing the recording's
/// spectrum by the emitted sweep's spectrum, producing a complex
/// frequency response on the FFT grid `[0, Nyquist]`.
///
/// The inverse-filter approach is the standard log-sweep
/// deconvolution:
///
/// ```text
///    H(f) = Y(f) / X(f)
/// ```
///
/// where Y is the recording and X is the emitted sweep. A small
/// regularisation term ε is added to the denominator to keep out-
/// of-band bins from blowing up — 60 dB below the sweep's peak is a
/// safe default.
///
/// The returned spectrum has `recording.len().next_power_of_two() / 2 + 1`
/// complex bins, indexed so bin k corresponds to frequency
/// `k * sample_rate / fft_size`.
///
/// Callers that want multiple realisations pass each captured sweep
/// through this function in turn and feed the collected `Vec<Vec<_>>`
/// to [`compute_coherence_from_realizations`].
pub fn deconvolve_sweep(
    recording: &[f32],
    reference: &[f32],
    sample_rate: u32,
) -> Result<Vec<Complex<f32>>, String> {
    if recording.len() != reference.len() {
        return Err(format!(
            "deconvolve_sweep: recording len {} != reference len {}",
            recording.len(),
            reference.len()
        ));
    }
    if recording.is_empty() {
        return Err("deconvolve_sweep: empty input".to_string());
    }
    if sample_rate == 0 {
        return Err("deconvolve_sweep: zero sample_rate".to_string());
    }

    let n = recording.len();
    let fft_size = n.next_power_of_two();

    let mut y: Vec<Complex<f32>> = recording.iter().map(|&s| Complex::new(s, 0.0)).collect();
    y.resize(fft_size, Complex::new(0.0, 0.0));
    let mut x: Vec<Complex<f32>> = reference.iter().map(|&s| Complex::new(s, 0.0)).collect();
    x.resize(fft_size, Complex::new(0.0, 0.0));

    let fft = plan_fft_forward(fft_size);
    fft.process(&mut y);
    fft.process(&mut x);

    // Regularisation: 60 dB below the sweep's peak bin magnitude.
    let x_peak = x
        .iter()
        .map(|c| c.norm())
        .fold(0.0_f32, f32::max)
        .max(1e-20);
    let epsilon = x_peak * 1e-3; // 60 dB below peak
    let eps_sq = epsilon * epsilon;

    let spectrum_size = fft_size / 2 + 1;
    let mut h = Vec::with_capacity(spectrum_size);
    for k in 0..spectrum_size {
        // H = Y / X with Tikhonov-style regularisation:
        //   H = (Y · conj(X)) / (|X|² + ε²)
        let yk = y[k];
        let xk = x[k];
        let num = yk * xk.conj();
        let den = xk.norm_sqr() + eps_sq;
        h.push(num / den);
    }
    Ok(h)
}

/// Estimate per-bin noise floor in dB from a silence window.
///
/// Takes the pre-silence samples captured before the sweep starts,
/// windows the FFT the same way the sweep analysis does, and returns
/// one dB value per positive-frequency bin (including DC and Nyquist,
/// i.e. `fft_size / 2 + 1` values). Result is reference-to-full-scale
/// (i.e., a silence bin at 0.001 linear amplitude maps to -60 dB).
///
/// The FFT size is taken as `silence.len().next_power_of_two()` so
/// the bin grid matches [`deconvolve_sweep`] when the silence window
/// is the same length as the sweep.
///
/// A Hann window is applied before the FFT to reduce spectral
/// leakage that would otherwise push DC noise into every other bin
/// and make bass SNR look healthier than it is.
pub fn estimate_noise_floor_db_from_silence(silence: &[f32], _sample_rate: u32) -> Vec<f32> {
    if silence.is_empty() {
        return Vec::new();
    }
    let n = silence.len();
    let fft_size = n.next_power_of_two();
    let spectrum_size = fft_size / 2 + 1;

    // Hann-window the silence before FFT.
    let mut buf: Vec<Complex<f32>> = silence
        .iter()
        .enumerate()
        .map(|(k, &s)| {
            let w = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * k as f32 / (n as f32 - 1.0)).cos());
            Complex::new(s * w, 0.0)
        })
        .collect();
    buf.resize(fft_size, Complex::new(0.0, 0.0));

    let fft = plan_fft_forward(fft_size);
    fft.process(&mut buf);

    // Windowed amplitude normalisation for a real sinusoid on a bin
    // centre. The FFT of `sin(2π·m·k/N)` has magnitude `N/2` at bin
    // `m`, and Hann windowing multiplies that by its coherent gain
    // of `0.5` — so the windowed peak is `N/4`. Multiply by `4/N` to
    // recover the underlying sinusoid amplitude (and let
    // `20·log10(mag)` match the tone's dBFS).
    let norm = 4.0 / n as f32;

    buf.into_iter()
        .take(spectrum_size)
        .map(|c| {
            let mag = c.norm() * norm;
            if mag > 1e-20 {
                20.0 * mag.log10()
            } else {
                -400.0 // effectively "nothing"; avoids -inf leaking downstream
            }
        })
        .collect()
}

#[cfg(test)]
mod gd_1c_tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn coherence_single_realization_is_unity() {
        let h = vec![
            Complex::new(1.0, 0.0),
            Complex::new(0.5, 0.5),
            Complex::new(0.0, 1.0),
        ];
        let coh = compute_coherence_from_realizations(&[h]).unwrap();
        assert_eq!(coh.len(), 3);
        for c in coh {
            // A single realization is trivially "coherent with itself"
            // — γ² = 1 by construction. Callers must enforce N ≥ 4
            // at a higher level.
            assert!(
                (c - 1.0).abs() < 1e-6,
                "single-realization γ² should be 1, got {c}"
            );
        }
    }

    #[test]
    fn coherence_identical_realizations_is_unity() {
        let r = vec![
            Complex::new(0.8, 0.2),
            Complex::new(0.0, 1.0),
            Complex::new(-0.5, 0.5),
        ];
        let realizations = vec![r.clone(), r.clone(), r.clone(), r];
        let coh = compute_coherence_from_realizations(&realizations).unwrap();
        for c in coh {
            assert!(
                (c - 1.0).abs() < 1e-6,
                "identical realizations → γ² = 1, got {c}"
            );
        }
    }

    #[test]
    fn coherence_random_realizations_is_zero() {
        // Four realizations whose phases cancel out on average:
        // ±1 and ±i. The complex mean is 0, so γ² = 0.
        let bins = 3;
        let r0: Vec<Complex<f32>> = (0..bins).map(|_| Complex::new(1.0, 0.0)).collect();
        let r1: Vec<Complex<f32>> = (0..bins).map(|_| Complex::new(-1.0, 0.0)).collect();
        let r2: Vec<Complex<f32>> = (0..bins).map(|_| Complex::new(0.0, 1.0)).collect();
        let r3: Vec<Complex<f32>> = (0..bins).map(|_| Complex::new(0.0, -1.0)).collect();
        let coh = compute_coherence_from_realizations(&[r0, r1, r2, r3]).unwrap();
        for c in coh {
            assert!(c < 1e-6, "canceling-phase realizations → γ² ≈ 0, got {c}");
        }
    }

    #[test]
    fn coherence_rejects_mismatched_lengths() {
        let r0 = vec![Complex::new(1.0_f32, 0.0); 3];
        let r1 = vec![Complex::new(1.0_f32, 0.0); 4];
        let err = compute_coherence_from_realizations(&[r0, r1]).unwrap_err();
        assert!(err.contains("has 4 bins, expected 3"), "got: {err}");
    }

    #[test]
    fn coherence_empty_input_errors() {
        let err = compute_coherence_from_realizations(&[]).unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn deconvolve_matches_unity_system() {
        // If the recorded signal IS the emitted sweep, H should be
        // approximately 1 across the passband.
        let n: usize = 1024;
        let sr = 48_000_u32;
        let sweep: Vec<f32> = (0..n)
            .map(|k| {
                let t = k as f32 / sr as f32;
                let f = 100.0 * (10.0_f32).powf(3.0 * t / (n as f32 / sr as f32));
                (2.0 * PI * f * t).sin() * 0.5
            })
            .collect();
        let recording = sweep.clone();
        let h = deconvolve_sweep(&recording, &sweep, sr).unwrap();
        assert_eq!(h.len(), n.next_power_of_two() / 2 + 1);
        // Mid-band bins should be ≈ 1 (within the regularisation
        // floor). Check bins 10..50 — avoids DC where the sweep has
        // no energy and the Nyquist edge where the log sweep dies out.
        let mid_slice = &h[10..50];
        for (i, c) in mid_slice.iter().enumerate() {
            let mag = c.norm();
            assert!(
                mag > 0.1 && mag < 10.0,
                "bin {} magnitude {mag} out of expected range",
                i + 10
            );
        }
    }

    #[test]
    fn deconvolve_rejects_length_mismatch() {
        let a = vec![0.0_f32; 10];
        let b = vec![0.0_f32; 11];
        let err = deconvolve_sweep(&a, &b, 48_000).unwrap_err();
        assert!(err.contains("!="), "got: {err}");
    }

    #[test]
    fn noise_floor_pure_silence_is_very_low() {
        let silence = vec![0.0_f32; 4096];
        let nf = estimate_noise_floor_db_from_silence(&silence, 48_000);
        assert_eq!(nf.len(), 4096 / 2 + 1);
        for (i, v) in nf.iter().enumerate() {
            assert!(
                *v < -200.0,
                "pure silence bin {i} should report extremely low dB, got {v}",
            );
        }
    }

    #[test]
    fn noise_floor_tone_peaks_at_exact_bin() {
        // Pick a frequency that lands exactly on an FFT bin centre
        // so there's no inter-bin leakage. Hann windowing still
        // splits ~half the peak energy into the two adjacent bins by
        // design; at the exact centre the main-lobe peak returns
        // within ~1 dB of the target.
        let sr = 48_000_u32;
        let n: usize = 4096;
        let target_bin = 100_usize;
        let freq = (target_bin as f32 * sr as f32) / n as f32; // 1171.875 Hz
        let amp_db = -40.0_f32;
        let amp = 10.0_f32.powf(amp_db / 20.0);
        let tone: Vec<f32> = (0..n)
            .map(|k| amp * (2.0 * PI * freq * k as f32 / sr as f32).sin())
            .collect();
        let nf = estimate_noise_floor_db_from_silence(&tone, sr);
        // Find the peak bin in a small bracket around the target.
        let mut peak_db = f32::NEG_INFINITY;
        let mut peak_bin = 0;
        for (k, v) in nf
            .iter()
            .enumerate()
            .take(target_bin + 3)
            .skip(target_bin - 2)
        {
            if *v > peak_db {
                peak_db = *v;
                peak_bin = k;
            }
        }
        assert_eq!(
            peak_bin, target_bin,
            "peak bin should be at the tone frequency"
        );
        assert!(
            (peak_db - amp_db).abs() < 1.5,
            "peak dB {peak_db} should be within ±1.5 dB of target {amp_db}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_power_of_two() {
        assert_eq!(next_power_of_two(1), 1);
        assert_eq!(next_power_of_two(2), 2);
        assert_eq!(next_power_of_two(3), 4);
        assert_eq!(next_power_of_two(1000), 1024);
        assert_eq!(next_power_of_two(1024), 1024);
        assert_eq!(next_power_of_two(1025), 2048);
    }

    #[test]
    fn test_hann_window() {
        let signal = vec![1.0; 100];
        let windowed = apply_hann_window(&signal);

        // First and last samples should be near zero
        assert!(windowed[0].abs() < 0.01);
        assert!(windowed[99].abs() < 0.01);

        // Middle sample should be near 1.0
        assert!((windowed[50] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_estimate_lag_zero() {
        // Identical signals should have zero lag
        let signal = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let lag = estimate_lag(&signal, &signal).unwrap();
        assert_eq!(lag, 0);
    }

    #[test]
    fn test_estimate_lag_positive() {
        // Reference leads recorded (recorded is delayed)
        // Use longer signals for reliable FFT-based cross-correlation
        let mut reference = vec![0.0; 100];
        let mut recorded = vec![0.0; 100];

        // Create a pulse pattern that will correlate well
        for (j, val) in reference[10..20].iter_mut().enumerate() {
            *val = j as f32 / 10.0;
        }
        // Same pattern but delayed by 5 samples
        for (j, val) in recorded[15..25].iter_mut().enumerate() {
            *val = j as f32 / 10.0;
        }

        let lag = estimate_lag(&reference, &recorded).unwrap();
        assert_eq!(lag, 5, "Recorded signal is delayed by 5 samples");
    }

    #[test]
    fn test_identical_signals_have_zero_lag() {
        // When signals are truly identical (like in the bug case),
        // lag should be exactly zero
        let signal = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let lag = estimate_lag(&signal, &signal).unwrap();
        assert_eq!(lag, 0, "Identical signals should have zero lag");
    }

    /// Write a mono f32 WAV file for testing
    fn write_test_wav(path: &std::path::Path, samples: &[f32], sample_rate: u32) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for &s in samples {
            writer.write_sample(s).unwrap();
        }
        writer.finalize().unwrap();
    }

    /// Generate a log sweep signal (same as the recording system uses)
    fn generate_test_sweep(
        start_freq: f32,
        end_freq: f32,
        duration_secs: f32,
        sample_rate: u32,
        amplitude: f32,
    ) -> Vec<f32> {
        let num_samples = (duration_secs * sample_rate as f32) as usize;
        let mut signal = Vec::with_capacity(num_samples);
        let ln_ratio = (end_freq / start_freq).ln();
        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            let phase = 2.0 * PI * start_freq * duration_secs / ln_ratio
                * ((t / duration_secs * ln_ratio).exp() - 1.0);
            signal.push(amplitude * phase.sin());
        }
        signal
    }

    #[test]
    fn test_analyze_recording_normal_channel() {
        // Simulate a normal speaker: reference sweep played back and recorded
        // with some attenuation and small delay
        let sample_rate = 48000;
        let duration = 1.0;
        let reference = generate_test_sweep(20.0, 20000.0, duration, sample_rate, 0.5);

        // Simulate recording: attenuate by ~-6dB (factor 0.5) and delay by 100 samples
        let delay = 100;
        let attenuation = 0.5;
        let mut recorded = vec![0.0_f32; reference.len() + delay];
        for (i, &s) in reference.iter().enumerate() {
            recorded[i + delay] = s * attenuation;
        }

        let dir = std::env::temp_dir().join(format!("sotf_test_normal_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let wav_path = dir.join("test_normal.wav");
        write_test_wav(&wav_path, &recorded, sample_rate);

        let result = analyze_recording(&wav_path, &reference, sample_rate, None).unwrap();
        std::fs::remove_dir_all(&dir).ok();

        // Compute average SPL in the passband (200 Hz - 10 kHz)
        let mut sum = 0.0_f32;
        let mut count = 0;
        for (&freq, &db) in result.frequencies.iter().zip(result.spl_db.iter()) {
            if (200.0..=10000.0).contains(&freq) {
                sum += db;
                count += 1;
            }
        }
        let avg_db = sum / count as f32;

        // Expected: ~-6 dB (attenuation factor 0.5)
        // Allow generous tolerance for windowing/FFT artifacts
        assert!(
            avg_db > -12.0 && avg_db < 0.0,
            "Normal channel avg SPL should be near -6 dB, got {:.1} dB",
            avg_db
        );

        // No bin should exceed +6 dB (physically implausible for passive attenuation)
        let max_db = result
            .spl_db
            .iter()
            .zip(result.frequencies.iter())
            .filter(|&(_, &f)| (200.0..=10000.0).contains(&f))
            .map(|(&db, _)| db)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            max_db < 6.0,
            "Normal channel should not have bins above +6 dB, got {:.1} dB",
            max_db
        );
    }

    #[test]
    fn test_analyze_recording_silent_channel() {
        // Simulate a disconnected speaker: reference sweep played but recording
        // is just low-level noise (no speaker output)
        let sample_rate = 48000;
        let duration = 1.0;
        let reference = generate_test_sweep(20.0, 20000.0, duration, sample_rate, 0.5);

        // Recording is pure noise at -60 dBFS (amplitude 0.001)
        let noise_amplitude = 0.001;
        let num_samples = reference.len();
        let mut recorded = Vec::with_capacity(num_samples);
        // Use deterministic "noise" (alternating small values)
        for i in 0..num_samples {
            let pseudo_noise =
                noise_amplitude * (((i as f32 * 0.1).sin() + (i as f32 * 0.37).cos()) * 0.5);
            recorded.push(pseudo_noise);
        }

        let dir = std::env::temp_dir().join(format!("sotf_test_silent_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let wav_path = dir.join("test_silent.wav");
        write_test_wav(&wav_path, &recorded, sample_rate);

        let result = analyze_recording(&wav_path, &reference, sample_rate, None).unwrap();
        std::fs::remove_dir_all(&dir).ok();

        // For a disconnected channel, the transfer function should be very low
        // (noise / sweep ≈ noise floor). It must NOT show spurious high-dB peaks.
        let max_db = result
            .spl_db
            .iter()
            .zip(result.frequencies.iter())
            .filter(|&(_, &f)| (100.0..=10000.0).contains(&f))
            .map(|(&db, _)| db)
            .fold(f32::NEG_INFINITY, f32::max);

        assert!(
            max_db < 0.0,
            "Silent/disconnected channel should not have positive dB values, got max {:.1} dB",
            max_db
        );
    }

    #[test]
    fn test_analyze_recording_lfe_narrow_sweep_same_point_count() {
        // Simulate a 5.1 scenario: LFE uses a narrow sweep (20-500 Hz) while
        // main channels use the full range (20-20000 Hz). Both must produce
        // the same number of output frequency points to avoid ndarray shape
        // mismatches when curves are combined in the optimizer.
        let sample_rate = 48000;
        let duration = 1.0;

        // Full-range reference (main channel)
        let ref_full = generate_test_sweep(20.0, 20000.0, duration, sample_rate, 0.5);
        // Narrow reference (LFE)
        let ref_lfe = generate_test_sweep(20.0, 500.0, duration, sample_rate, 0.5);

        // Simulate recordings: attenuated copies with delay
        let delay = 50;
        let atten = 0.3;

        let mut rec_full = vec![0.0_f32; ref_full.len() + delay];
        for (i, &s) in ref_full.iter().enumerate() {
            rec_full[i + delay] = s * atten;
        }

        let mut rec_lfe = vec![0.0_f32; ref_lfe.len() + delay];
        for (i, &s) in ref_lfe.iter().enumerate() {
            rec_lfe[i + delay] = s * atten;
        }

        let dir = std::env::temp_dir().join(format!("sotf_test_lfe_points_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let wav_full = dir.join("main.wav");
        let wav_lfe = dir.join("lfe.wav");
        write_test_wav(&wav_full, &rec_full, sample_rate);
        write_test_wav(&wav_lfe, &rec_lfe, sample_rate);

        let result_full = analyze_recording(&wav_full, &ref_full, sample_rate, None).unwrap();
        let result_lfe = analyze_recording(&wav_lfe, &ref_lfe, sample_rate, None).unwrap();
        std::fs::remove_dir_all(&dir).ok();

        // Both must produce the same number of frequency points
        assert_eq!(
            result_full.frequencies.len(),
            result_lfe.frequencies.len(),
            "Main ({}) and LFE ({}) must have the same number of frequency points",
            result_full.frequencies.len(),
            result_lfe.frequencies.len()
        );
        assert_eq!(
            result_full.spl_db.len(),
            result_lfe.spl_db.len(),
            "SPL arrays must match in length"
        );

        // LFE should have valid data below ~500 Hz and noise floor above
        let lfe_valid_count = result_lfe
            .spl_db
            .iter()
            .zip(result_lfe.frequencies.iter())
            .filter(|&(&db, &f)| f <= 500.0 && db > -100.0)
            .count();
        assert!(
            lfe_valid_count > 100,
            "LFE should have valid data below 500 Hz, got {} points",
            lfe_valid_count
        );

        let lfe_above_500_max = result_lfe
            .spl_db
            .iter()
            .zip(result_lfe.frequencies.iter())
            .filter(|&(_, &f)| f > 1000.0)
            .map(|(&db, _)| db)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            lfe_above_500_max <= -100.0,
            "LFE above 1 kHz should be at noise floor, got {:.1} dB",
            lfe_above_500_max
        );
    }

    #[test]
    fn test_cross_correlate_envelope_known_delay() {
        // Generate a narrowband probe, delay it, and verify detection
        let n = 4096;
        let sr = 48000_u32;
        let probe = crate::signals::gen_narrowband_probe(n, sr, 0.5, 42, 800.0, 2000.0);

        // Simulate recording: delay by 240 samples (~5ms) + attenuation
        let delay = 240_usize;
        let attenuation = 0.3;
        let mut recorded = vec![0.0_f32; n + delay + 1000];
        for (i, &s) in probe.iter().enumerate() {
            recorded[i + delay] += s * attenuation;
        }

        let result = cross_correlate_envelope(&probe, &recorded, sr).unwrap();

        // Peak should be near the known delay
        let detected_samples = result.peak_sample;
        assert!(
            (detected_samples as isize - delay as isize).unsigned_abs() <= 2,
            "Expected delay ~{} samples, got {}",
            delay,
            detected_samples
        );

        // Arrival time should be ~5ms
        assert!(
            (result.arrival_ms - 5.0).abs() < 0.1,
            "Expected ~5.0 ms, got {:.3} ms",
            result.arrival_ms
        );
    }

    #[test]
    fn test_cross_correlate_envelope_with_noise() {
        // Probe detection should work even with additive noise
        let n = 4096;
        let sr = 48000_u32;
        let probe = crate::signals::gen_narrowband_probe(n, sr, 0.5, 42, 800.0, 2000.0);

        let delay = 480_usize; // 10ms
        let mut recorded = vec![0.0_f32; n + delay + 1000];
        for (i, &s) in probe.iter().enumerate() {
            recorded[i + delay] += s * 0.5;
        }
        // Add noise
        let noise = crate::signals::gen_white_noise(0.1, sr, recorded.len() as f32 / sr as f32);
        for (r, &n_s) in recorded.iter_mut().zip(noise.iter()) {
            *r += n_s;
        }

        let result = cross_correlate_envelope(&probe, &recorded, sr).unwrap();

        assert!(
            (result.peak_sample as isize - delay as isize).unsigned_abs() <= 2,
            "Expected delay ~{}, got {} (with noise)",
            delay,
            result.peak_sample
        );
    }

    #[test]
    fn test_windowed_fr_synthetic() {
        // Create synthetic IR: impulse at sample 0 + delayed impulse at sample 240 (5ms)
        // Direct window [0, 240) should show flat response
        // Early window [240, 1920) should show the reflection's response
        let sr = 48000;
        let mut ir = vec![0.0f32; 4096];
        ir[0] = 1.0; // direct sound
        ir[240] = 0.5; // reflection at 5ms, -6dB

        let result = compute_windowed_fr(&ir, 240, 1920, sr, 200).unwrap();

        // Direct window should have content
        assert!(!result.direct_sound_spl.is_empty());
        assert!(!result.early_reflections_spl.is_empty());
        assert!(!result.late_reverb_spl.is_empty());

        // All frequency vectors should have the requested number of points
        assert_eq!(result.direct_sound_freq.len(), 200);
        assert_eq!(result.early_reflections_freq.len(), 200);
        assert_eq!(result.late_reverb_freq.len(), 200);

        // Time boundaries should match
        assert!((result.direct_end_ms - 5.0).abs() < 0.01);
        assert!((result.early_end_ms - 40.0).abs() < 0.01);

        // Direct sound should be roughly flat above the resolution limit.
        // Short window = poor LF resolution, but mid-HF should be flat.
        // Filter to frequencies above 500 Hz where the 240-sample window has resolution
        let mid_hf: Vec<f32> = result
            .direct_sound_freq
            .iter()
            .zip(result.direct_sound_spl.iter())
            .filter(|&(&f, _)| f > 500.0 && f < 18000.0)
            .map(|(_, &spl)| spl)
            .collect();
        if mid_hf.len() > 2 {
            let max = mid_hf.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
            let min = mid_hf.iter().fold(f32::INFINITY, |a, &b| a.min(b));
            let range = max - min;
            assert!(
                range < 12.0,
                "Direct sound mid-HF range too large: {:.1} dB",
                range
            );
        }
    }

    #[test]
    fn test_windowed_fr_empty_window() {
        // If early_end == direct_end (no early reflections), that window should be empty/silent
        let sr = 48000;
        let mut ir = vec![0.0f32; 2048];
        // Place impulse away from window edges so fading doesn't zero it out
        ir[50] = 1.0;

        let result = compute_windowed_fr(&ir, 200, 200, sr, 200).unwrap();

        // Early reflections window is zero-length — SPL should be very low
        assert_eq!(result.early_reflections_spl.len(), 200);
        for &spl in &result.early_reflections_spl {
            assert!(
                spl <= -199.0,
                "Expected silent early reflections, got {:.1} dB",
                spl
            );
        }

        // Direct and late should still have content
        let direct_max = result
            .direct_sound_spl
            .iter()
            .fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        assert!(
            direct_max > -100.0,
            "Direct sound should have content, max was {:.1} dB",
            direct_max
        );
    }
}
