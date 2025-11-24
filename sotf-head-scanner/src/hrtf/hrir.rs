//! HRIR (Head-Related Impulse Response) computation
//!
//! Converts frequency-domain HRTFs to time-domain HRIRs using inverse FFT.
//!
//! # Process
//!
//! 1. Add 0 Hz bin (HRTF = 1 at DC, since head doesn't filter DC)
//! 2. Make Nyquist frequency real-valued (required for real IFFT)
//! 3. Apply inverse real FFT (irfft) with complex conjugate
//! 4. Circular shift to enforce causality
//!
//! # Example
//!
//! ```rust,no_run
//! use head_scanner::hrtf::*;
//!
//! // After parsing NumCalc output
//! let hrtf_data = parser.parse_source(0)?;
//!
//! // Compute HRIR with 48 kHz sampling rate
//! let hrir = compute_hrir(&hrtf_data.eval_pressure, 48000.0, 32)?;
//!
//! // hrir now contains time-domain impulse responses
//! # Ok::<(), anyhow::Error>(())
//! ```

use crate::hrtf::types::{HrirData, PressureData};
use anyhow::{Context, Result};
use ndarray::{Array1, Array2};
use num_complex::Complex64;
use rustfft::{FftPlanner, num_complex::Complex};

/// Compute HRIRs from HRTFs using inverse FFT
///
/// # Arguments
///
/// * `pressure_data` - Complex pressure data from NumCalc (frequency domain)
/// * `sample_rate` - Desired sampling rate in Hz (e.g., 44100.0, 48000.0)
/// * `n_shift` - Samples to shift for causality (typically 20-40 for 44.1 kHz)
///
/// # Returns
///
/// * `HrirData` - Time-domain impulse responses
///
/// # Process
///
/// The HRTF must go from f_0 > 0 to f_N (Nyquist = fs/2) in uniform steps.
/// This function:
/// 1. Adds a 0 Hz bin (value = 1, since HRTF is 0 dB at DC)
/// 2. Makes the Nyquist bin real-valued
/// 3. Mirrors the spectrum and applies inverse real FFT
/// 4. Circularly shifts the result to make it causal
pub fn compute_hrir(
    pressure_data: &PressureData,
    sample_rate: f64,
    n_shift: usize,
) -> Result<HrirData> {
    let num_points = pressure_data.pressure.nrows();
    let num_freqs = pressure_data.pressure.ncols();

    if num_freqs == 0 {
        anyhow::bail!("No frequency data available");
    }

    // Validate frequency spacing
    if pressure_data.frequencies.len() != num_freqs {
        anyhow::bail!(
            "Frequency vector length mismatch: {} vs {}",
            pressure_data.frequencies.len(),
            num_freqs
        );
    }

    // Check frequency spacing (should be uniform)
    if pressure_data.frequencies.len() > 2 {
        let diffs: Vec<f64> = pressure_data
            .frequencies
            .windows(2)
            .map(|w| w[1] - w[0])
            .collect();
        let mean_diff = diffs.iter().sum::<f64>() / diffs.len() as f64;
        let max_deviation = diffs
            .iter()
            .map(|&d| (d - mean_diff).abs())
            .fold(0.0, f64::max);

        if max_deviation > 0.1 {
            anyhow::bail!(
                "Frequency spacing not uniform (max deviation: {:.3} Hz)",
                max_deviation
            );
        }
    }

    // Determine FFT size
    // Add 1 for 0 Hz bin
    let fft_size = 2 * num_freqs; // Real FFT produces N/2+1 complex bins

    // Allocate output
    let mut hrir_output = Array2::zeros((num_points, fft_size));

    // Process each point
    for point_idx in 0..num_points {
        // Extract pressure for this point
        let mut spectrum = Vec::with_capacity(num_freqs + 1);

        // Add 0 Hz bin (DC component = 1.0 + 0j, since HRTF is 0 dB at DC)
        spectrum.push(Complex::new(1.0, 0.0));

        // Add existing frequency bins (with complex conjugate due to FFT sign convention)
        for freq_idx in 0..num_freqs {
            let p = pressure_data.pressure[[point_idx, freq_idx]];
            spectrum.push(Complex::new(p.re, -p.im)); // Complex conjugate
        }

        // Make Nyquist frequency real (take magnitude)
        if let Some(last) = spectrum.last_mut() {
            *last = Complex::new(last.norm(), 0.0);
        }

        // Apply inverse real FFT
        let ir = irfft(&spectrum, fft_size)?;

        // Circular shift to make causal
        let shifted_ir = circular_shift(&ir, n_shift);

        // Store result
        for (i, &val) in shifted_ir.iter().enumerate() {
            hrir_output[[point_idx, i]] = val;
        }
    }

    Ok(HrirData {
        impulse_response: hrir_output,
        sample_rate,
        node_ids: pressure_data.node_ids.clone(),
    })
}

/// Inverse real FFT
///
/// Takes N/2+1 complex bins and produces N real samples
fn irfft(spectrum: &[Complex<f64>], n: usize) -> Result<Vec<f64>> {
    // Create full spectrum by mirroring (conjugate symmetry)
    let mut full_spectrum = vec![Complex::new(0.0, 0.0); n];

    // Copy positive frequencies
    for (i, &val) in spectrum.iter().enumerate() {
        full_spectrum[i] = val;
    }

    // Mirror for negative frequencies (conjugate symmetry)
    for i in 1..(n / 2) {
        let conj = full_spectrum[i].conj();
        full_spectrum[n - i] = conj;
    }

    // Setup inverse FFT
    let mut planner = FftPlanner::new();
    let ifft = planner.plan_fft_inverse(n);

    // Apply IFFT
    ifft.process(&mut full_spectrum);

    // Extract real parts and normalize
    let scale = 1.0 / n as f64;
    Ok(full_spectrum.iter().map(|c| c.re * scale).collect())
}

/// Circular shift array by n positions to the right
///
/// Used to make HRIRs causal (no sound before t=0)
fn circular_shift(data: &[f64], n: usize) -> Vec<f64> {
    let len = data.len();
    if len == 0 || n == 0 {
        return data.to_vec();
    }

    let shift = n % len;
    let mut result = vec![0.0; len];

    // Copy last n elements to beginning
    result[..shift].copy_from_slice(&data[len - shift..]);

    // Copy remaining elements
    result[shift..].copy_from_slice(&data[..len - shift]);

    result
}

/// Apply Hann window to time-domain signal
///
/// Hann window: w(n) = 0.5 * (1 - cos(2π*n/N))
pub fn apply_hann_window(data: &mut [f64]) {
    let n = data.len();
    for i in 0..n {
        let window_val = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / n as f64).cos());
        data[i] *= window_val;
    }
}

/// Apply Hamming window to time-domain signal
///
/// Hamming window: w(n) = 0.54 - 0.46 * cos(2π*n/N)
pub fn apply_hamming_window(data: &mut [f64]) {
    let n = data.len();
    for i in 0..n {
        let window_val = 0.54 - 0.46 * (2.0 * std::f64::consts::PI * i as f64 / n as f64).cos();
        data[i] *= window_val;
    }
}

/// Apply Blackman window to time-domain signal
///
/// Blackman window: w(n) = 0.42 - 0.5*cos(2π*n/N) + 0.08*cos(4π*n/N)
pub fn apply_blackman_window(data: &mut [f64]) {
    let n = data.len();
    for i in 0..n {
        let arg = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
        let window_val = 0.42 - 0.5 * arg.cos() + 0.08 * (2.0 * arg).cos();
        data[i] *= window_val;
    }
}

impl HrirData {
    /// Get HRIR for a specific point
    pub fn get_ir(&self, point_index: usize) -> Option<Array1<f64>> {
        if point_index < self.impulse_response.nrows() {
            Some(self.impulse_response.row(point_index).to_owned())
        } else {
            None
        }
    }

    /// Get number of samples in each IR
    pub fn num_samples(&self) -> usize {
        self.impulse_response.ncols()
    }

    /// Get number of points (measurement positions)
    pub fn num_points(&self) -> usize {
        self.impulse_response.nrows()
    }

    /// Get duration of IR in seconds
    pub fn duration(&self) -> f64 {
        self.num_samples() as f64 / self.sample_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circular_shift() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let shifted = circular_shift(&data, 2);
        assert_eq!(shifted, vec![4.0, 5.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_circular_shift_zero() {
        let data = vec![1.0, 2.0, 3.0];
        let shifted = circular_shift(&data, 0);
        assert_eq!(shifted, data);
    }

    #[test]
    fn test_hann_window() {
        let mut data = vec![1.0; 4];
        apply_hann_window(&mut data);

        // Hann window should be zero at endpoints
        assert!(data[0] < 0.01);
        assert!(data[3] < 0.01);
        // And peak in middle
        assert!(data[1] > 0.5);
        assert!(data[2] > 0.5);
    }

    #[test]
    fn test_irfft_simple() {
        // Test with simple DC signal
        let spectrum = vec![Complex::new(1.0, 0.0), Complex::new(0.0, 0.0)];
        let result = irfft(&spectrum, 4).unwrap();

        // DC component should give constant signal
        for &val in &result {
            assert!((val - 0.25).abs() < 1e-10); // Normalized by N
        }
    }

    #[test]
    fn test_hrir_data() {
        let ir = Array2::from_shape_fn((10, 256), |(i, j)| i as f64 + j as f64);
        let hrir = HrirData {
            impulse_response: ir,
            sample_rate: 48000.0,
            node_ids: (0..10).collect(),
        };

        assert_eq!(hrir.num_points(), 10);
        assert_eq!(hrir.num_samples(), 256);
        assert!((hrir.duration() - 256.0 / 48000.0).abs() < 1e-10);
    }
}
