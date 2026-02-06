// ============================================================================
// PND Analysis Logic
// ============================================================================

use rustfft::{Fft, FftPlanner, num_complex::Complex, num_traits::Zero};
use std::sync::Arc;

pub struct PndAnalyzer {
    fft_size: usize,
    sample_rate: u32,
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    buffer: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,

    // Partial tracking state
    prev_peaks: Vec<(f32, f32)>, // (frequency_hz, magnitude)
}

impl PndAnalyzer {
    pub fn new(fft_size: usize, sample_rate: u32) -> Self {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);

        // Hann window
        let window: Vec<f32> = (0..fft_size)
            .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / fft_size as f32).cos()))
            .collect();

        Self {
            fft_size,
            sample_rate,
            fft,
            window,
            buffer: vec![Complex::zero(); fft_size],
            scratch: vec![Complex::zero(); fft_size],
            prev_peaks: Vec::new(),
        }
    }

    pub fn analyze(&mut self, samples: &[f32]) -> f32 {
        if samples.len() < self.fft_size {
            return 1.0; // No drift
        }

        // 1. Window and FFT
        for (i, &sample) in samples.iter().enumerate().take(self.fft_size) {
            self.buffer[i] = Complex::new(sample * self.window[i], 0.0);
        }
        self.fft
            .process_with_scratch(&mut self.buffer, &mut self.scratch);

        // 2. Peak Picking
        let bin_hz = self.sample_rate as f32 / self.fft_size as f32;
        let mut peaks = Vec::new();

        // Simple peak picker: magnitude > neighbors and magnitude > threshold
        let threshold = 0.001; // -60dB approx
        for i in 1..self.fft_size / 2 - 1 {
            let mag_prev = self.buffer[i - 1].norm();
            let mag_curr = self.buffer[i].norm();
            let mag_next = self.buffer[i + 1].norm();

            if mag_curr > threshold && mag_curr > mag_prev && mag_curr > mag_next {
                // Parabolic interpolation for more accurate frequency
                let alpha = mag_prev;
                let beta = mag_curr;
                let gamma = mag_next;
                let p = 0.5 * (alpha - gamma) / (alpha - 2.0 * beta + gamma);
                let freq = (i as f32 + p) * bin_hz;
                peaks.push((freq, mag_curr));
            }
        }

        // 3. Drift Estimation (Partial Tracking)
        let mut ratios = Vec::new();

        if !self.prev_peaks.is_empty() {
            for (freq, _mag) in &peaks {
                // Find closest peak in previous frame
                let mut min_diff = f32::MAX;
                let mut best_prev_freq = 0.0;

                for (prev_freq, _prev_mag) in &self.prev_peaks {
                    let diff = (freq - prev_freq).abs();
                    if diff < min_diff {
                        min_diff = diff;
                        best_prev_freq = *prev_freq;
                    }
                }

                // If reasonably close (e.g., within 50 cents), consider it the same partial
                // 50 cents is approx 3% change
                if min_diff < best_prev_freq * 0.03 {
                    let ratio = freq / best_prev_freq;
                    ratios.push(ratio);
                }
            }
        }

        self.prev_peaks = peaks;

        if ratios.is_empty() {
            return 1.0;
        }

        // Median drift ratio
        ratios.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = ratios.len() / 2;
        ratios[mid]
    }
}
