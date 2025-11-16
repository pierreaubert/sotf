//! FFT-based frequency analysis for recorded signals
//!
//! This module provides functions to analyze recorded audio signals and extract:
//! - Frequency spectrum (magnitude in dBFS)
//! - Phase spectrum (compensated for latency)
//! - Latency estimation via cross-correlation
//! - Microphone compensation for calibrated measurements

use hound::WavReader;
use rustfft::{FftPlanner, num_complex::Complex};
use std::f32::consts::PI;
use std::path::Path;

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
                eprintln!(
                    "[apply_to_sweep] t={:.3}s, freq={:.1}Hz, comp_db={:.2}dB, gain_db={:.2}dB, gain={:.3}x",
                    t, freq, comp_db, gain_db, gain
                );
            }

            compensated.push(sample * gain);
        }

        eprintln!(
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

        eprintln!("[MicrophoneCompensation] Loading from {:?}", path);

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
            eprintln!(
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
                    eprintln!(
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

        eprintln!(
            "[MicrophoneCompensation] Loaded {} calibration points: {:.1} Hz - {:.1} Hz",
            frequencies.len(),
            frequencies[0],
            frequencies[frequencies.len() - 1]
        );
        eprintln!(
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
}

/// Analyze a recorded WAV file against a reference signal
///
/// # Arguments
/// * `recorded_path` - Path to the recorded WAV file
/// * `reference_signal` - Reference signal (should match the signal used for playback)
/// * `sample_rate` - Sample rate in Hz
///
/// # Returns
/// Analysis result with frequency, SPL, and phase data
pub fn analyze_recording(
    recorded_path: &Path,
    reference_signal: &[f32],
    sample_rate: u32,
) -> Result<AnalysisResult, String> {
    // Load recorded WAV
    println!("[FFT Analysis] Loading recorded file: {:?}", recorded_path);
    let recorded = load_wav_mono(recorded_path)?;
    println!(
        "[FFT Analysis] Loaded {} samples from recording",
        recorded.len()
    );
    println!(
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

    // Debug: Check signal statistics
    let ref_max = reference
        .iter()
        .map(|&x| x.abs())
        .fold(0.0_f32, |a, b| a.max(b));
    let rec_max = recorded
        .iter()
        .map(|&x| x.abs())
        .fold(0.0_f32, |a, b| a.max(b));
    let ref_rms = (reference.iter().map(|&x| x * x).sum::<f32>() / reference.len() as f32).sqrt();
    let rec_rms = (recorded.iter().map(|&x| x * x).sum::<f32>() / recorded.len() as f32).sqrt();

    println!(
        "[FFT Analysis] Reference: max={:.4}, RMS={:.4}",
        ref_max, ref_rms
    );
    println!(
        "[FFT Analysis] Recorded:  max={:.4}, RMS={:.4}",
        rec_max, rec_rms
    );

    // Show first 10 samples for comparison
    println!(
        "[FFT Analysis] First 5 reference samples: {:?}",
        &reference[..5.min(reference.len())]
    );
    println!(
        "[FFT Analysis] First 5 recorded samples:  {:?}",
        &recorded[..5.min(recorded.len())]
    );

    // Check if signals are identical (compare overlap region)
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
    println!(
        "[FFT Analysis] Identical samples: {}/{} ({:.1}%)",
        identical_count,
        check_len,
        identical_count as f32 * 100.0 / check_len as f32
    );

    // Estimate lag using cross-correlation
    let lag = estimate_lag(reference, recorded);

    println!(
        "[FFT Analysis] Estimated lag: {} samples ({:.2} ms)",
        lag,
        lag as f32 * 1000.0 / sample_rate as f32
    );

    // Time-align the signals before FFT
    // Use the full reference signal length and align the recorded signal to it
    // If recorded is delayed (positive lag), skip the lag samples in recorded
    // If recorded leads (negative lag), we need to handle it differently
    let analysis_len = reference.len();

    let (aligned_ref, aligned_rec) = if lag >= 0 {
        let lag_usize = lag as usize;
        if lag_usize >= recorded.len() {
            return Err("Lag is larger than recorded signal length".to_string());
        }
        // Check if we have enough recorded samples after the lag
        let available_rec_len = recorded.len() - lag_usize;
        if available_rec_len < analysis_len {
            println!(
                "[FFT Analysis] Warning: Only {} samples available after lag alignment (need {})",
                available_rec_len, analysis_len
            );
            println!("[FFT Analysis] Analysis will be truncated to available length");
            let truncated_len = available_rec_len;
            (
                &reference[..truncated_len],
                &recorded[lag_usize..lag_usize + truncated_len],
            )
        } else {
            // We have enough samples - use full reference length
            (reference, &recorded[lag_usize..lag_usize + analysis_len])
        }
    } else {
        // Recorded leads reference - this shouldn't happen in normal loopback
        let lag_usize = (-lag) as usize;
        if lag_usize >= reference.len() {
            return Err("Negative lag is larger than reference signal length".to_string());
        }
        let new_len = (reference.len() - lag_usize).min(recorded.len());
        (
            &reference[lag_usize..lag_usize + new_len],
            &recorded[..new_len],
        )
    };

    println!(
        "[FFT Analysis] Aligned signal length: {} samples ({:.2}s)",
        aligned_ref.len(),
        aligned_ref.len() as f32 / sample_rate as f32
    );

    // Compute FFT for both aligned signals
    let fft_size = next_power_of_two(aligned_ref.len());

    let ref_spectrum = compute_fft(aligned_ref, fft_size)?;
    let rec_spectrum = compute_fft(aligned_rec, fft_size)?;

    // Generate 200 log-spaced frequency points between 20 Hz and 20 kHz
    let num_output_points = 200;
    let log_start = 20.0_f32.ln();
    let log_end = 20000.0_f32.ln();

    let mut frequencies = Vec::with_capacity(num_output_points);
    let mut spl_db = Vec::with_capacity(num_output_points);
    let mut phase_deg = Vec::with_capacity(num_output_points);

    let freq_resolution = sample_rate as f32 / fft_size as f32;
    let num_bins = fft_size / 2; // Single-sided spectrum

    // Apply 1/24 octave smoothing for each target frequency
    let mut skipped_count = 0;
    for i in 0..num_output_points {
        // Log-spaced target frequency
        let target_freq =
            (log_start + (log_end - log_start) * i as f32 / (num_output_points - 1) as f32).exp();

        // 1/24 octave bandwidth: ±1/48 octave around target frequency
        // Lower and upper frequency bounds: f * 2^(±1/48)
        let octave_fraction = 1.0 / 48.0;
        let freq_lower = target_freq * 2.0_f32.powf(-octave_fraction);
        let freq_upper = target_freq * 2.0_f32.powf(octave_fraction);

        // Find FFT bins within this frequency range
        let bin_lower = ((freq_lower / freq_resolution).floor() as usize).max(1);
        let bin_upper = ((freq_upper / freq_resolution).ceil() as usize).min(num_bins);

        if bin_lower > bin_upper || bin_upper >= ref_spectrum.len() {
            if skipped_count < 5 {
                println!(
                    "[FFT Analysis] Skipping freq {:.1} Hz: bin_lower={}, bin_upper={}, ref_spectrum.len()={}",
                    target_freq,
                    bin_lower,
                    bin_upper,
                    ref_spectrum.len()
                );
            }
            skipped_count += 1;
            continue; // Skip if range is invalid
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
            let transfer_function = rec_spectrum[k] / ref_spectrum[k];
            let magnitude = transfer_function.norm();

            // Phase from cross-spectrum (signals are already time-aligned)
            let cross_spectrum = ref_spectrum[k].conj() * rec_spectrum[k];
            let mut phase_rad = cross_spectrum.arg();

            // Wrap phase to [-π, π] range
            phase_rad = phase_rad.sin().atan2(phase_rad.cos());

            // Accumulate for averaging
            sum_magnitude += magnitude;
            sum_sin += phase_rad.sin();
            sum_cos += phase_rad.cos();
            bin_count += 1;
        }

        if bin_count == 0 {
            continue; // Skip if no bins in range
        }

        // Average magnitude
        let avg_magnitude = sum_magnitude / bin_count as f32;

        // Convert to dB
        let db = 20.0 * avg_magnitude.max(1e-10).log10();

        // Average phase using circular mean
        let avg_phase_rad = sum_sin.atan2(sum_cos);
        let phase = avg_phase_rad * 180.0 / PI;

        frequencies.push(target_freq);
        spl_db.push(db);
        phase_deg.push(phase);
    }

    println!(
        "[FFT Analysis] Generated {} frequency points for CSV output",
        frequencies.len()
    );
    println!(
        "[FFT Analysis] Skipped {} frequency points (out of {})",
        skipped_count, num_output_points
    );

    if frequencies.is_empty() {
        println!("[FFT Analysis] WARNING: No frequency points generated!");
        println!("[FFT Analysis] ref_spectrum.len() = {}", ref_spectrum.len());
        println!("[FFT Analysis] fft_size = {}", fft_size);
        println!("[FFT Analysis] freq_resolution = {}", freq_resolution);
        println!("[FFT Analysis] num_bins = {}", num_bins);
    }

    Ok(AnalysisResult {
        frequencies,
        spl_db,
        phase_deg,
        estimated_lag_samples: lag,
    })
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
pub fn write_analysis_csv(
    result: &AnalysisResult,
    output_path: &Path,
    compensation: Option<&MicrophoneCompensation>,
) -> Result<(), String> {
    use std::fs::File;
    use std::io::Write;

    println!(
        "[write_analysis_csv] Writing {} frequency points to {:?}",
        result.frequencies.len(),
        output_path
    );

    if let Some(comp) = compensation {
        println!(
            "[write_analysis_csv] Applying inverse microphone compensation ({} calibration points)",
            comp.frequencies.len()
        );
    }

    if result.frequencies.is_empty() {
        return Err("Cannot write CSV: Analysis result has no frequency points!".to_string());
    }

    let mut file =
        File::create(output_path).map_err(|e| format!("Failed to create CSV file: {}", e))?;

    // Write header
    writeln!(file, "frequency_hz,spl_db,phase_deg")
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

        writeln!(file, "{:.6},{:.3},{:.6}", freq, spl, result.phase_deg[i])
            .map_err(|e| format!("Failed to write data: {}", e))?;
    }

    println!(
        "[write_analysis_csv] Successfully wrote {} data rows to CSV",
        result.frequencies.len()
    );

    Ok(())
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
fn estimate_lag(reference: &[f32], recorded: &[f32]) -> isize {
    let len = reference.len().min(recorded.len());

    // Zero-pad to avoid circular correlation artifacts
    let fft_size = next_power_of_two(len * 2);

    let ref_fft = compute_fft_padded(reference, fft_size).unwrap();
    let rec_fft = compute_fft_padded(recorded, fft_size).unwrap();

    // Cross-correlation in frequency domain: conj(X) * Y
    let mut cross_corr_fft: Vec<Complex<f32>> = ref_fft
        .iter()
        .zip(rec_fft.iter())
        .map(|(x, y)| x.conj() * y)
        .collect();

    // IFFT to get cross-correlation in time domain
    let mut planner = FftPlanner::new();
    let ifft = planner.plan_fft_inverse(fft_size);
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

    if max_idx <= fft_size / 2 {
        max_idx as isize
    } else {
        max_idx as isize - fft_size as isize
    }
}

/// Compute FFT of a signal with Hann windowing
///
/// # Arguments
/// * `signal` - Input signal
/// * `fft_size` - FFT size (should be power of 2)
///
/// # Returns
/// Complex FFT spectrum
fn compute_fft(signal: &[f32], fft_size: usize) -> Result<Vec<Complex<f32>>, String> {
    // Apply Hann window
    let windowed = apply_hann_window(signal);

    compute_fft_padded(&windowed, fft_size)
}

/// Compute FFT with zero-padding
fn compute_fft_padded(signal: &[f32], fft_size: usize) -> Result<Vec<Complex<f32>>, String> {
    // Zero-pad to fft_size
    let mut buffer: Vec<Complex<f32>> = signal.iter().map(|&x| Complex::new(x, 0.0)).collect();
    buffer.resize(fft_size, Complex::new(0.0, 0.0));

    // Compute FFT
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_size);
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
    signal
        .iter()
        .enumerate()
        .map(|(i, &x)| {
            let window = 0.5 * (1.0 - (2.0 * PI * i as f32 / (len - 1) as f32).cos());
            x * window
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

    println!(
        "[load_wav_mono_channel] WAV file: {} channels, {} Hz, {:?} format",
        channels, spec.sample_rate, spec.sample_format
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
    println!(
        "[load_wav_mono_channel] Read {} total samples",
        samples.len()
    );

    // Handle mono file - return as-is
    if channels == 1 {
        println!(
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
        println!(
            "[load_wav_mono_channel] Extracting channel {} from {} channels",
            ch_idx, channels
        );
        Ok(samples
            .chunks(channels)
            .map(|chunk| chunk[ch_idx])
            .collect())
    } else {
        // Average all channels to mono
        println!(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signals;

    #[test]
    fn test_fft_sine_wave_consistency() {
        // Test FFT analysis with simple sine waves at different frequencies
        let sample_rate = 48000;
        let duration = 1.0;
        let amp = 0.5;
        let frequencies = [100.0, 1000.0, 10000.0];

        let mut spl_values = Vec::new();

        for &freq in &frequencies {
            // Generate a simple sine wave
            let signal = generate_sine_wave(freq, amp, sample_rate, duration);

            // Analyze the signal
            let result = analyze_recording_direct(&signal, &signal, sample_rate)
                .expect("Failed to analyze recording");

            // Find the SPL at the target frequency - look for peak in a wider range
            let search_range = 200.0; // Wider search range for better peak detection
            if let Some(spl) = result
                .frequencies
                .iter()
                .zip(&result.spl_db)
                .filter(|&(&f, _)| (f - freq).abs() < search_range)
                .max_by(|&(_, spl1), &(_, spl2)| spl1.partial_cmp(spl2).unwrap())
                .map(|(_, &spl)| spl)
            {
                spl_values.push(spl);
                println!("Sine wave {} Hz: SPL = {:.2} dB", freq, spl);
            }
        }

        // Check consistency
        if spl_values.len() >= 2 {
            let min_spl = spl_values.iter().fold(f32::INFINITY, |a, &b| a.min(b));
            let max_spl = spl_values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
            let variation = max_spl - min_spl;

            println!(
                "Sine wave SPL variation: {:.2} dB (min: {:.2}, max: {:.2})",
                variation, min_spl, max_spl
            );

            // Simple sine waves should have very consistent SPL
            assert!(
                variation < 1.0,
                "Sine wave SPL variation {:.2} dB exceeds 1 dB tolerance",
                variation
            );
        }
    }

    fn generate_sine_wave(
        frequency: f32,
        amplitude: f32,
        sample_rate: u32,
        duration: f32,
    ) -> Vec<f32> {
        let n_frames = (duration * sample_rate as f32) as usize;
        let mut signal = Vec::with_capacity(n_frames);

        for n in 0..n_frames {
            let t = n as f32 / sample_rate as f32;
            let sample = amplitude * (2.0 * std::f32::consts::PI * frequency * t).sin();
            signal.push(sample);
        }

        signal
    }

    #[test]
    fn test_fft_constant_amplitude_analysis() {
        // Generate a perfect log sweep with constant amplitude
        let amp = 0.5;
        let sample_rate = 48000;
        let duration = 0.5; // Shorter for faster test
        let signal = signals::gen_log_sweep(20.0, 20000.0, amp, sample_rate, duration);

        // Analyze the signal directly (simulating a perfect recording)
        let result = analyze_recording_direct(&signal, &signal, sample_rate)
            .expect("Failed to analyze recording");

        // Check SPL consistency across the practical audio range (100 Hz and above)
        let mut spl_values = Vec::new();
        let freq_checkpoints = [100.0, 1000.0, 10000.0]; // Practical audio range frequencies

        for &target_freq in &freq_checkpoints {
            // Find the peak SPL in the frequency range around the target frequency
            let search_range = 200.0; // Wider search range for better peak detection
            if let Some(spl) = result
                .frequencies
                .iter()
                .zip(&result.spl_db)
                .filter(|&(&f, _)| (f - target_freq).abs() < search_range)
                .max_by(|&(_, spl1), &(_, spl2)| spl1.partial_cmp(spl2).unwrap())
                .map(|(_, &spl)| spl)
            {
                spl_values.push(spl);
                println!("Frequency ~{} Hz: SPL = {:.2} dB", target_freq, spl);
            }
        }

        // For a constant amplitude signal in a loopback test,
        // we should see very consistent SPL across the practical audio range (100 Hz+)
        if spl_values.len() >= 2 {
            let min_spl = spl_values.iter().fold(f32::INFINITY, |a, &b| a.min(b));
            let max_spl = spl_values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
            let variation = max_spl - min_spl;

            println!(
                "SPL variation: {:.2} dB (min: {:.2}, max: {:.2})",
                variation, min_spl, max_spl
            );

            // For a loopback test with constant amplitude sweep, we expect sub-0.5 dB accuracy
            // This accounts only for FFT windowing effects and peak detection
            assert!(
                variation < 0.5,
                "SPL variation {:.2} dB exceeds 0.5 dB tolerance for constant amplitude loopback test",
                variation
            );
        }
    }

    fn analyze_recording_direct(
        recorded: &[f32],
        reference: &[f32],
        sample_rate: u32,
    ) -> Result<AnalysisResult, String> {
        // Ensure both signals have the same length for analysis
        let min_len = recorded.len().min(reference.len());
        let recorded = &recorded[..min_len];
        let reference = &reference[..min_len];

        // Estimate lag using cross-correlation
        let lag = estimate_lag(reference, recorded);

        println!(
            "[FFT Analysis] Estimated lag: {} samples ({:.2} ms)",
            lag,
            lag as f32 * 1000.0 / sample_rate as f32
        );

        // Compute FFT for both signals
        let fft_size = next_power_of_two(min_len);

        let ref_spectrum = compute_fft(reference, fft_size)?;
        let rec_spectrum = compute_fft(recorded, fft_size)?;

        // Compute frequency bins
        let num_bins = fft_size / 2; // Single-sided spectrum
        let mut frequencies = Vec::with_capacity(num_bins);
        let mut spl_db = Vec::with_capacity(num_bins);
        let mut phase_deg = Vec::with_capacity(num_bins);

        let freq_resolution = sample_rate as f32 / fft_size as f32;

        // Skip DC bin (k=0), compute for k=1..num_bins
        for k in 1..=num_bins {
            let freq = k as f32 * freq_resolution;

            // Magnitude from recorded signal
            // Compute transfer function: H(f) = recorded / reference
            // This gives us the system response (for loopback, should be ~1.0 or 0 dB)
            let transfer_function = rec_spectrum[k] / ref_spectrum[k];
            let magnitude = transfer_function.norm();

            // Convert to dB (no windowing correction needed for transfer function)
            let db = 20.0 * magnitude.max(1e-10).log10();

            // Phase from cross-spectrum with lag compensation
            let cross_spectrum = ref_spectrum[k].conj() * rec_spectrum[k];
            let mut phase_rad = cross_spectrum.arg();

            // Compensate for lag
            let lag_phase = -2.0 * PI * freq * lag as f32 / sample_rate as f32;
            phase_rad += lag_phase;

            // Keep phase unwrapped (continuous) - convert directly to degrees
            let phase_degrees = phase_rad * 180.0 / PI;

            frequencies.push(freq);
            spl_db.push(db);
            phase_deg.push(phase_degrees);
        }

        Ok(AnalysisResult {
            frequencies,
            spl_db,
            phase_deg,
            estimated_lag_samples: lag,
        })
    }

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
        let lag = estimate_lag(&signal, &signal);
        assert_eq!(lag, 0);
    }

    #[test]
    fn test_estimate_lag_positive() {
        // Reference leads recorded (recorded is delayed)
        let reference = vec![1.0, 2.0, 3.0, 4.0, 5.0, 0.0, 0.0];
        let recorded = vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
        let lag = estimate_lag(&reference, &recorded);
        assert_eq!(lag, 2);
    }

    /// Regression test: Detect suspiciously identical signals
    ///
    /// In a real recording scenario, the recorded signal should NEVER be
    /// 100% identical to the reference due to:
    /// - Latency/lag
    /// - Noise
    /// - DAC/ADC quantization
    /// - Audio path differences
    ///
    /// If signals are 100% identical, it indicates a bug where the
    /// reference file was copied instead of actually recorded.
    #[test]
    fn test_detect_suspicious_identical_signals() {
        let sample_rate = 48000;

        // Generate a reference signal
        let reference = generate_sine_wave(1000.0, 0.5, sample_rate, 0.1);

        // Simulate a realistic recording with small latency and noise
        let mut realistic_recording = reference.clone();
        // Add 10 sample delay
        realistic_recording.splice(0..0, vec![0.0; 10]);
        realistic_recording.truncate(reference.len());
        // Add tiny noise
        for sample in &mut realistic_recording {
            *sample += 0.0001 * (rand::random::<f32>() - 0.5);
        }

        // Test with realistic recording - should work fine
        let result = analyze_recording_direct(&realistic_recording, &reference, sample_rate);
        assert!(
            result.is_ok(),
            "Realistic recording should analyze successfully"
        );

        // Test with identical signals - this is suspicious!
        let identical_count = reference
            .iter()
            .zip(&realistic_recording)
            .filter(|(r, c)| (*r - *c).abs() < 1e-6)
            .count();

        let identical_percent = identical_count as f32 * 100.0 / reference.len() as f32;

        // In a realistic recording, we should NOT have 100% identical samples
        assert!(
            identical_percent < 99.0,
            "Recording is suspiciously identical to reference ({:.1}% identical). \
             This suggests a file copy bug instead of actual recording.",
            identical_percent
        );
    }

    #[test]
    fn test_identical_signals_have_zero_lag() {
        // When signals are truly identical (like in the bug case),
        // lag should be exactly zero
        let signal = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let lag = estimate_lag(&signal, &signal);
        assert_eq!(lag, 0, "Identical signals should have zero lag");
    }

    #[test]
    fn test_microphone_compensation_from_csv() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temporary CSV file
        let mut temp_file = NamedTempFile::with_suffix(".csv").unwrap();
        writeln!(temp_file, "frequency_hz,spl_db").unwrap();
        writeln!(temp_file, "100,0.0").unwrap();
        writeln!(temp_file, "1000,2.0").unwrap();
        writeln!(temp_file, "10000,-1.0").unwrap();
        temp_file.flush().unwrap();

        // Load compensation
        let comp = MicrophoneCompensation::from_file(temp_file.path())
            .expect("Failed to load compensation");

        // Verify data
        assert_eq!(comp.frequencies.len(), 3);
        assert_eq!(comp.spl_db.len(), 3);
        assert_eq!(comp.frequencies[0], 100.0);
        assert_eq!(comp.frequencies[1], 1000.0);
        assert_eq!(comp.frequencies[2], 10000.0);
        assert_eq!(comp.spl_db[0], 0.0);
        assert_eq!(comp.spl_db[1], 2.0);
        assert_eq!(comp.spl_db[2], -1.0);
    }

    #[test]
    fn test_microphone_compensation_from_txt() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temporary TXT file (space-separated, no header)
        let mut temp_file = NamedTempFile::with_suffix(".txt").unwrap();
        writeln!(temp_file, "100 0.0").unwrap();
        writeln!(temp_file, "1000 2.0").unwrap();
        writeln!(temp_file, "10000 -1.0").unwrap();
        temp_file.flush().unwrap();

        // Load compensation
        let comp = MicrophoneCompensation::from_file(temp_file.path())
            .expect("Failed to load compensation from TXT");

        // Verify data
        assert_eq!(comp.frequencies.len(), 3);
        assert_eq!(comp.spl_db.len(), 3);
        assert_eq!(comp.frequencies[0], 100.0);
        assert_eq!(comp.frequencies[1], 1000.0);
        assert_eq!(comp.frequencies[2], 10000.0);
        assert_eq!(comp.spl_db[0], 0.0);
        assert_eq!(comp.spl_db[1], 2.0);
        assert_eq!(comp.spl_db[2], -1.0);
    }

    #[test]
    fn test_microphone_compensation_txt_with_tabs() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create TXT file with tabs
        let mut temp_file = NamedTempFile::with_suffix(".txt").unwrap();
        writeln!(temp_file, "100\t0.0").unwrap();
        writeln!(temp_file, "1000\t2.0").unwrap();
        temp_file.flush().unwrap();

        // Should successfully parse
        let comp = MicrophoneCompensation::from_file(temp_file.path())
            .expect("Failed to load compensation with tabs");

        assert_eq!(comp.frequencies.len(), 2);
        assert_eq!(comp.frequencies[0], 100.0);
        assert_eq!(comp.frequencies[1], 1000.0);
    }

    #[test]
    fn test_microphone_compensation_txt_skip_non_numeric() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create TXT file with header and comments that should be skipped
        let mut temp_file = NamedTempFile::with_suffix(".txt").unwrap();
        writeln!(temp_file, "Frequency SPL").unwrap(); // Non-numeric, skip
        writeln!(temp_file, "# Comment line").unwrap(); // Comment, skip
        writeln!(temp_file, "100 0.0").unwrap();
        writeln!(temp_file, "Some notes here").unwrap(); // Non-numeric, skip
        writeln!(temp_file, "1000 2.0").unwrap();
        writeln!(temp_file, "").unwrap(); // Empty, skip
        writeln!(temp_file, "10000 -1.0").unwrap();
        temp_file.flush().unwrap();

        // Should parse only the 3 numeric lines
        let comp = MicrophoneCompensation::from_file(temp_file.path())
            .expect("Failed to load compensation with non-numeric lines");

        assert_eq!(comp.frequencies.len(), 3);
        assert_eq!(comp.frequencies[0], 100.0);
        assert_eq!(comp.frequencies[1], 1000.0);
        assert_eq!(comp.frequencies[2], 10000.0);
    }

    #[test]
    fn test_microphone_compensation_txt_auto_detect_separator() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Test auto-detection of comma separator in .txt file
        let mut temp_file = NamedTempFile::with_suffix(".txt").unwrap();
        writeln!(temp_file, "100,0.0").unwrap();
        writeln!(temp_file, "1000,2.0").unwrap();
        temp_file.flush().unwrap();

        let comp = MicrophoneCompensation::from_file(temp_file.path())
            .expect("Failed to auto-detect comma separator");

        assert_eq!(comp.frequencies.len(), 2);
        assert_eq!(comp.frequencies[0], 100.0);
        assert_eq!(comp.spl_db[0], 0.0);
    }

    #[test]
    fn test_microphone_compensation_txt_mixed_separators() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Test file with different separators on different lines
        let mut temp_file = NamedTempFile::with_suffix(".txt").unwrap();
        writeln!(temp_file, "100,0.0").unwrap(); // Comma
        writeln!(temp_file, "1000\t2.0").unwrap(); // Tab
        writeln!(temp_file, "10000 -1.0").unwrap(); // Space
        temp_file.flush().unwrap();

        let comp = MicrophoneCompensation::from_file(temp_file.path())
            .expect("Failed to parse mixed separators");

        assert_eq!(comp.frequencies.len(), 3);
        assert_eq!(comp.frequencies[0], 100.0);
        assert_eq!(comp.frequencies[1], 1000.0);
        assert_eq!(comp.frequencies[2], 10000.0);
    }

    #[test]
    fn test_microphone_compensation_interpolation() {
        // Create compensation data: +2dB at 1000 Hz, 0dB at 100 Hz and 10000 Hz
        let comp = MicrophoneCompensation {
            frequencies: vec![100.0, 1000.0, 10000.0],
            spl_db: vec![0.0, 2.0, 0.0],
        };

        // Test exact matches
        assert_eq!(comp.interpolate_at(100.0), 0.0);
        assert_eq!(comp.interpolate_at(1000.0), 2.0);
        assert_eq!(comp.interpolate_at(10000.0), 0.0);

        // Test interpolation
        let mid_point = comp.interpolate_at(550.0); // Halfway between 100 and 1000 in linear scale
        assert!(
            mid_point > 0.5 && mid_point < 1.5,
            "Expected interpolated value around 1.0, got {}",
            mid_point
        );

        // Test out-of-range (should return 0)
        assert_eq!(comp.interpolate_at(50.0), 0.0); // Below range
        assert_eq!(comp.interpolate_at(20000.0), 0.0); // Above range
    }

    #[test]
    fn test_microphone_compensation_csv_with_comments() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create CSV with comments and header
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "# This is a comment").unwrap();
        writeln!(temp_file, "frequency_hz,spl_db").unwrap();
        writeln!(temp_file, "# Another comment").unwrap();
        writeln!(temp_file, "100,1.0").unwrap();
        writeln!(temp_file, "").unwrap(); // Empty line
        writeln!(temp_file, "1000,2.0").unwrap();
        temp_file.flush().unwrap();

        // Should successfully parse
        let comp = MicrophoneCompensation::from_file(temp_file.path())
            .expect("Failed to load compensation with comments");

        assert_eq!(comp.frequencies.len(), 2);
        assert_eq!(comp.frequencies[0], 100.0);
        assert_eq!(comp.frequencies[1], 1000.0);
    }

    #[test]
    fn test_microphone_compensation_unsorted_frequencies() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create CSV with unsorted frequencies
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "frequency_hz,spl_db").unwrap();
        writeln!(temp_file, "1000,2.0").unwrap();
        writeln!(temp_file, "100,1.0").unwrap(); // Out of order!
        temp_file.flush().unwrap();

        // Should fail with error about sorting
        let result = MicrophoneCompensation::from_file(temp_file.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("strictly increasing"));
    }

    #[test]
    fn test_write_analysis_csv_with_compensation() {
        use tempfile::NamedTempFile;

        // Create test analysis result
        let result = AnalysisResult {
            frequencies: vec![100.0, 1000.0, 10000.0],
            spl_db: vec![-10.0, -20.0, -30.0], // Measured values
            phase_deg: vec![0.0, 45.0, -90.0],
            estimated_lag_samples: 0,
        };

        // Create compensation: mic reads +2dB louder at 1000 Hz
        let compensation = MicrophoneCompensation {
            frequencies: vec![100.0, 1000.0, 10000.0],
            spl_db: vec![0.0, 2.0, 0.0],
        };

        // Write CSV with compensation
        let temp_file = NamedTempFile::new().unwrap();
        write_analysis_csv(&result, temp_file.path(), Some(&compensation))
            .expect("Failed to write CSV");

        // Read back and verify
        let content = std::fs::read_to_string(temp_file.path()).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        // Should have header + 3 data rows
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], "frequency_hz,spl_db,phase_deg");

        // Parse second line (1000 Hz): measured -20.0, mic deviation +2.0, true level = -20 - 2 = -22
        let parts: Vec<&str> = lines[2].split(',').collect();
        let compensated_spl: f32 = parts[1].parse().unwrap();
        assert!(
            (compensated_spl - (-22.0)).abs() < 0.01,
            "Expected -22.0, got {}",
            compensated_spl
        );
    }

    #[test]
    fn test_write_analysis_csv_without_compensation() {
        use tempfile::NamedTempFile;

        // Create test analysis result
        let result = AnalysisResult {
            frequencies: vec![1000.0],
            spl_db: vec![-20.0],
            phase_deg: vec![45.0],
            estimated_lag_samples: 0,
        };

        // Write CSV without compensation
        let temp_file = NamedTempFile::new().unwrap();
        write_analysis_csv(&result, temp_file.path(), None).expect("Failed to write CSV");

        // Read back and verify
        let content = std::fs::read_to_string(temp_file.path()).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        // Should have header + 1 data row
        assert_eq!(lines.len(), 2);

        // Parse data line - should match original
        let parts: Vec<&str> = lines[1].split(',').collect();
        let spl: f32 = parts[1].parse().unwrap();
        assert!((spl - (-20.0)).abs() < 0.01);
    }

    #[test]
    fn test_apply_sweep_precompensation() {
        use crate::signals;

        // Create compensation: +6dB at 1000 Hz, 0dB elsewhere
        let compensation = MicrophoneCompensation {
            frequencies: vec![100.0, 1000.0, 10000.0],
            spl_db: vec![0.0, 6.0, 0.0],
        };

        // Generate a constant-amplitude sweep
        let sample_rate = 48000;
        let duration = 0.1; // Short for testing
        let sweep = signals::gen_log_sweep(100.0, 10000.0, 0.5, sample_rate, duration);

        // Apply inverse pre-compensation
        let compensated = compensation.apply_to_sweep(&sweep, 100.0, 10000.0, sample_rate, true);

        // Check that signal length is preserved
        assert_eq!(compensated.len(), sweep.len());

        // At 1000 Hz (roughly middle of the sweep), the signal should be attenuated by -6dB
        // to compensate for the mic's +6dB boost
        // Find approximate middle sample
        let mid_idx = sweep.len() / 2;

        // The compensated signal should have ~0.5x amplitude (≈ -6dB) compared to original
        let original_amp = sweep[mid_idx].abs();
        let compensated_amp = compensated[mid_idx].abs();

        // -6dB = 20*log10(0.5) ≈ 0.501 linear ratio
        let expected_ratio = 10_f32.powf(-6.0 / 20.0); // ≈ 0.501
        let actual_ratio = compensated_amp / original_amp;

        // Allow 10% tolerance due to sweep not being exactly at 1000 Hz at midpoint
        assert!(
            (actual_ratio - expected_ratio).abs() / expected_ratio < 0.1,
            "Expected ratio ~{:.3}, got {:.3}",
            expected_ratio,
            actual_ratio
        );
    }

    #[test]
    fn test_apply_sweep_precompensation_direct() {
        // Test direct compensation (not inverse)
        let compensation = MicrophoneCompensation {
            frequencies: vec![100.0, 1000.0, 10000.0],
            spl_db: vec![0.0, -3.0, 0.0],
        };

        let sample_rate = 48000;
        let sweep = vec![0.5; 4800]; // Constant signal for simplicity

        // Apply direct compensation (boost by -3dB means attenuate by 3dB)
        let compensated = compensation.apply_to_sweep(&sweep, 100.0, 10000.0, sample_rate, false);

        // Around 1000 Hz, signal should be attenuated
        let mid_idx = sweep.len() / 2;
        assert!(compensated[mid_idx] < sweep[mid_idx]);
    }
}
