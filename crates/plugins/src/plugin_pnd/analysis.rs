// ============================================================================
// PND Analysis Logic — Windowed Hop-Based Drift Estimation
// ============================================================================

use rustfft::{Fft, FftPlanner, num_complex::Complex, num_traits::Zero};
use std::sync::Arc;

pub struct PndAnalyzer {
    fft_size: usize,
    sample_rate: u32,
    hop_size: usize,
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    fft_buffer: Vec<Complex<f32>>,
    fft_scratch: Vec<Complex<f32>>,

    // Ring buffer for accumulating samples across process() calls
    ring_buffer: Vec<f32>,
    ring_write_pos: usize,
    ring_samples_accumulated: usize,
    ring_filled: bool,

    // Partial tracking state
    prev_peaks: Vec<(f32, f32)>, // (frequency_hz, magnitude)

    // Pre-allocated scratch buffers (reused via .clear())
    peak_scratch: Vec<(f32, f32)>,
    ratio_scratch: Vec<f32>,

    // Drift history circular buffer (sized by analysis_window_ms)
    drift_history: Vec<f32>,
    drift_write_pos: usize,
    drift_count: usize,
    drift_history_capacity: usize,

    // Pre-allocated sort buffer for median computation
    median_scratch: Vec<f32>,
}

impl PndAnalyzer {
    pub fn new(fft_size: usize, sample_rate: u32, analysis_window_ms: f32) -> Self {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);

        let hop_size = fft_size / 4; // 512 for fft_size=2048

        // Hann window
        let window: Vec<f32> = (0..fft_size)
            .map(|i| {
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / fft_size as f32).cos())
            })
            .collect();

        let drift_history_capacity =
            compute_drift_history_capacity(analysis_window_ms, sample_rate, hop_size);

        Self {
            fft_size,
            sample_rate,
            hop_size,
            fft,
            window,
            fft_buffer: vec![Complex::zero(); fft_size],
            fft_scratch: vec![Complex::zero(); fft_size],

            ring_buffer: vec![0.0; fft_size],
            ring_write_pos: 0,
            ring_samples_accumulated: 0,
            ring_filled: false,

            prev_peaks: Vec::new(),
            peak_scratch: Vec::new(),
            ratio_scratch: Vec::new(),

            drift_history: vec![0.0; drift_history_capacity],
            drift_write_pos: 0,
            drift_count: 0,
            drift_history_capacity,

            median_scratch: Vec::new(),
        }
    }

    /// Feed samples and return the current drift estimate.
    /// Samples are accumulated in a ring buffer; FFT is triggered every hop_size new samples
    /// once the ring buffer has been filled at least once (fft_size samples).
    pub fn analyze(&mut self, samples: &[f32]) -> f32 {
        for &sample in samples {
            // Write into ring buffer
            self.ring_buffer[self.ring_write_pos] = sample;
            self.ring_write_pos = (self.ring_write_pos + 1) % self.fft_size;
            self.ring_samples_accumulated += 1;

            // Mark filled once we've written fft_size samples total
            if !self.ring_filled && self.ring_samples_accumulated >= self.fft_size {
                self.ring_filled = true;
            }

            // Trigger FFT every hop_size samples, but only after ring is filled
            if self.ring_filled && self.ring_samples_accumulated >= self.hop_size {
                self.ring_samples_accumulated = 0;
                self.process_fft_frame();
            }
        }

        self.current_drift_estimate()
    }

    fn process_fft_frame(&mut self) {
        // Copy from ring buffer with Hann window.
        // ring_write_pos points to the oldest sample (just wrapped).
        let start = self.ring_write_pos; // oldest sample
        for i in 0..self.fft_size {
            let idx = (start + i) % self.fft_size;
            self.fft_buffer[i] = Complex::new(self.ring_buffer[idx] * self.window[i], 0.0);
        }

        self.fft
            .process_with_scratch(&mut self.fft_buffer, &mut self.fft_scratch);

        // Peak picking
        let bin_hz = self.sample_rate as f32 / self.fft_size as f32;
        self.peak_scratch.clear();

        let threshold = 0.001; // ~-60dB
        for i in 1..self.fft_size / 2 - 1 {
            let mag_prev = self.fft_buffer[i - 1].norm();
            let mag_curr = self.fft_buffer[i].norm();
            let mag_next = self.fft_buffer[i + 1].norm();

            if mag_curr > threshold && mag_curr > mag_prev && mag_curr > mag_next {
                // Parabolic interpolation for more accurate frequency
                let alpha = mag_prev;
                let beta = mag_curr;
                let gamma = mag_next;
                let denom = alpha - 2.0 * beta + gamma;
                let p = if denom.abs() > f32::EPSILON {
                    0.5 * (alpha - gamma) / denom
                } else {
                    0.0
                };
                let freq = (i as f32 + p) * bin_hz;
                self.peak_scratch.push((freq, mag_curr));
            }
        }

        // Partial tracking: match current peaks against previous frame
        self.ratio_scratch.clear();

        if !self.prev_peaks.is_empty() {
            for &(freq, _mag) in &self.peak_scratch {
                let mut min_diff = f32::MAX;
                let mut best_prev_freq = 0.0_f32;

                for &(prev_freq, _prev_mag) in &self.prev_peaks {
                    let diff = (freq - prev_freq).abs();
                    if diff < min_diff {
                        min_diff = diff;
                        best_prev_freq = prev_freq;
                    }
                }

                // Within 50 cents (~3% change) → same partial
                if best_prev_freq > 0.0 && min_diff < best_prev_freq * 0.03 {
                    let ratio = freq / best_prev_freq;
                    self.ratio_scratch.push(ratio);
                }
            }
        }

        // Swap peaks: reuse prev_peaks capacity
        self.prev_peaks.clear();
        self.prev_peaks.extend_from_slice(&self.peak_scratch);

        // Push median of ratios into drift history
        if !self.ratio_scratch.is_empty() {
            self.ratio_scratch
                .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mid = self.ratio_scratch.len() / 2;
            let frame_drift = self.ratio_scratch[mid];

            self.drift_history[self.drift_write_pos] = frame_drift;
            self.drift_write_pos = (self.drift_write_pos + 1) % self.drift_history_capacity;
            if self.drift_count < self.drift_history_capacity {
                self.drift_count += 1;
            }
        }
    }

    fn current_drift_estimate(&mut self) -> f32 {
        if self.drift_count == 0 {
            return 1.0;
        }

        // Compute median of drift_history[..drift_count]
        self.median_scratch.clear();
        if self.drift_count <= self.drift_history_capacity {
            // Not yet wrapped: entries are at [0..drift_count] or scattered
            // Since it's circular, collect all valid entries
            if self.drift_count < self.drift_history_capacity {
                // Haven't wrapped yet: entries [0..drift_count]
                self.median_scratch
                    .extend_from_slice(&self.drift_history[..self.drift_count]);
            } else {
                // Full buffer
                self.median_scratch
                    .extend_from_slice(&self.drift_history[..self.drift_history_capacity]);
            }
        }

        self.median_scratch
            .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = self.median_scratch.len() / 2;
        self.median_scratch[mid]
    }

    pub fn update_analysis_window(&mut self, analysis_window_ms: f32) {
        let new_capacity =
            compute_drift_history_capacity(analysis_window_ms, self.sample_rate, self.hop_size);
        if new_capacity != self.drift_history_capacity {
            self.drift_history = vec![0.0; new_capacity];
            self.drift_write_pos = 0;
            self.drift_count = 0;
            self.drift_history_capacity = new_capacity;
        }
    }

    pub fn reset(&mut self) {
        // Zero ring buffer state
        self.ring_buffer.fill(0.0);
        self.ring_write_pos = 0;
        self.ring_samples_accumulated = 0;
        self.ring_filled = false;

        // Clear peak tracking
        self.prev_peaks.clear();
        self.peak_scratch.clear();
        self.ratio_scratch.clear();

        // Reset drift history
        self.drift_history.fill(0.0);
        self.drift_write_pos = 0;
        self.drift_count = 0;
        self.median_scratch.clear();
    }
}

/// Compute drift history capacity: how many FFT frames fit in `analysis_window_ms`.
fn compute_drift_history_capacity(
    analysis_window_ms: f32,
    sample_rate: u32,
    hop_size: usize,
) -> usize {
    let samples_in_window = (analysis_window_ms / 1000.0 * sample_rate as f32) as usize;
    let capacity = samples_in_window / hop_size;
    capacity.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyzer_silence_returns_no_drift() {
        let mut analyzer = PndAnalyzer::new(2048, 44100, 100.0);
        let silence = vec![0.0; 4096];
        let drift = analyzer.analyze(&silence);
        assert!(
            (drift - 1.0).abs() < f32::EPSILON,
            "Silence should produce no drift, got {drift}"
        );
    }

    #[test]
    fn test_analyzer_stable_tone_returns_near_unity() {
        let mut analyzer = PndAnalyzer::new(2048, 44100, 100.0);

        // Generate several blocks of 440 Hz sine (enough to fill ring buffer + multiple hops)
        let num_samples = 44100; // 1 second
        let samples: Vec<f32> = (0..num_samples)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();

        let drift = analyzer.analyze(&samples);

        // Stable tone should produce drift very close to 1.0
        assert!(
            (drift - 1.0).abs() < 0.01,
            "Stable 440Hz tone should produce drift ~1.0, got {drift}"
        );
    }

    #[test]
    fn test_analyzer_processes_small_blocks() {
        let mut analyzer = PndAnalyzer::new(2048, 44100, 100.0);

        // Feed 1024-sample blocks (typical process() call size)
        let block_size = 1024;
        let samples: Vec<f32> = (0..block_size)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();

        // First block: ring not yet filled (need 2048), should return 1.0
        let drift1 = analyzer.analyze(&samples);
        assert!(
            (drift1 - 1.0).abs() < f32::EPSILON,
            "First 1024 samples shouldn't trigger FFT, got {drift1}"
        );

        // Second block: ring fills at sample 2048, then triggers FFT at 2048+hop
        let drift2 = analyzer.analyze(&samples);
        // Should still be ~1.0 (no prev_peaks for first FFT frame, or stable tone)
        assert!(
            (drift2 - 1.0).abs() < 0.01,
            "Second block drift should be ~1.0, got {drift2}"
        );
    }

    #[test]
    fn test_analyzer_reset() {
        let mut analyzer = PndAnalyzer::new(2048, 44100, 100.0);

        // Feed some data
        let samples: Vec<f32> = (0..4096)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();
        analyzer.analyze(&samples);

        // Reset
        analyzer.reset();

        // After reset, should return 1.0 (no drift history)
        let drift = analyzer.analyze(&vec![0.0; 512]);
        assert!(
            (drift - 1.0).abs() < f32::EPSILON,
            "After reset, drift should be 1.0, got {drift}"
        );
    }

    #[test]
    fn test_update_analysis_window() {
        let mut analyzer = PndAnalyzer::new(2048, 44100, 100.0);
        let initial_capacity = analyzer.drift_history_capacity;

        analyzer.update_analysis_window(200.0);
        assert!(
            analyzer.drift_history_capacity > initial_capacity,
            "Doubling window should increase capacity"
        );
        assert_eq!(analyzer.drift_count, 0, "Should reset drift count");
    }

    #[test]
    fn test_drift_history_capacity_computation() {
        // 100ms at 44100 Hz with hop_size 512
        // samples_in_window = 0.1 * 44100 = 4410
        // capacity = 4410 / 512 = 8
        let cap = compute_drift_history_capacity(100.0, 44100, 512);
        assert_eq!(cap, 8);

        // Very small window should clamp to 1
        let cap_min = compute_drift_history_capacity(1.0, 44100, 512);
        assert_eq!(cap_min, 1.max(44 / 512).max(1));
    }
}
