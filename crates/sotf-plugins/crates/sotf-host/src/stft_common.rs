// ============================================================================
// Shared STFT Infrastructure
// ============================================================================
//
// Reusable components for STFT-based plugins:
// - generate_hann_window: Hann window generation for STFT analysis
// - RealFftProcessor: Thin wrapper around realfft for single-channel use
// - RingAccumulator: Sample accumulator with hop-based triggering

use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::sync::Arc;

// ============================================================================
// Hann Window
// ============================================================================

/// Generate a Hann window of the given size.
/// Uses N (not N-1) divisor for perfect COLA with 50% overlap.
pub fn generate_hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| 0.5 * (1.0 - ((2.0 * std::f32::consts::PI * i as f32) / size as f32).cos()))
        .collect()
}

/// Generate a sqrt(Hann) window for WOLA (Weighted Overlap-Add) processing.
/// When used as both analysis and synthesis window, the product is Hann,
/// which has perfect COLA at 50% overlap.
pub fn generate_sqrt_hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| {
            let hann = 0.5 * (1.0 - ((2.0 * std::f32::consts::PI * i as f32) / size as f32).cos());
            hann.sqrt()
        })
        .collect()
}

// ============================================================================
// RealFftProcessor
// ============================================================================

/// Thin wrapper around `realfft` encapsulating planner + buffers for
/// single-channel use. Provides forward (real→complex) and optional
/// inverse (complex→real) FFT.
pub struct RealFftProcessor {
    #[allow(dead_code)]
    pub fft_size: usize,
    pub spectrum_size: usize,
    fft_forward: Arc<dyn RealToComplex<f32>>,
    fft_inverse: Option<Arc<dyn ComplexToReal<f32>>>,
    pub time_buffer: Vec<f32>,
    pub freq_buffer: Vec<Complex<f32>>,
}

impl RealFftProcessor {
    /// Create a forward-only FFT processor (no inverse).
    pub fn new_forward_only(fft_size: usize) -> Self {
        let spectrum_size = fft_size / 2 + 1;
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(fft_size);

        Self {
            fft_size,
            spectrum_size,
            fft_forward,
            fft_inverse: None,
            time_buffer: vec![0.0; fft_size],
            freq_buffer: vec![Complex::new(0.0, 0.0); spectrum_size],
        }
    }

    /// Create a bidirectional FFT processor (forward + inverse).
    #[allow(dead_code)]
    pub fn new_bidirectional(fft_size: usize) -> Self {
        let spectrum_size = fft_size / 2 + 1;
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(fft_size);
        let fft_inverse = planner.plan_fft_inverse(fft_size);

        Self {
            fft_size,
            spectrum_size,
            fft_forward,
            fft_inverse: Some(fft_inverse),
            time_buffer: vec![0.0; fft_size],
            freq_buffer: vec![Complex::new(0.0, 0.0); spectrum_size],
        }
    }

    /// Perform forward FFT: time_buffer → freq_buffer.
    /// The caller should fill `time_buffer` before calling this.
    pub fn forward(&mut self) {
        self.fft_forward
            .process(&mut self.time_buffer, &mut self.freq_buffer)
            .expect("FFT forward failed");
    }

    /// Perform inverse FFT: freq_buffer → time_buffer.
    /// Panics if this processor was created with `new_forward_only`.
    #[allow(dead_code)]
    pub fn inverse(&mut self) {
        self.fft_inverse
            .as_ref()
            .expect("Inverse FFT not available (forward-only processor)")
            .process(&mut self.freq_buffer, &mut self.time_buffer)
            .expect("FFT inverse failed");
    }
}

// ============================================================================
// RingAccumulator
// ============================================================================

/// Sample accumulator with hop-based triggering.
/// Accumulates samples into a circular buffer and signals when `hop_size`
/// new samples have been written (and the buffer has been filled at least once).
pub struct RingAccumulator {
    buffer: Vec<f32>,
    write_pos: usize,
    samples_since_trigger: usize,
    filled: bool,
    window_size: usize,
    hop_size: usize,
}

impl RingAccumulator {
    pub fn new(window_size: usize, hop_size: usize) -> Self {
        Self {
            buffer: vec![0.0; window_size],
            write_pos: 0,
            samples_since_trigger: 0,
            filled: false,
            window_size,
            hop_size,
        }
    }

    /// Push a single sample. Returns `true` when `hop_size` samples have
    /// accumulated since the last trigger (and the buffer is full).
    pub fn push(&mut self, sample: f32) -> bool {
        self.buffer[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % self.window_size;
        self.samples_since_trigger += 1;

        if !self.filled && self.samples_since_trigger >= self.window_size {
            self.filled = true;
        }

        if self.filled && self.samples_since_trigger >= self.hop_size {
            self.samples_since_trigger = 0;
            true
        } else {
            false
        }
    }

    /// Copy the current window (oldest-first) into `dest`.
    /// `dest` must be at least `window_size` long.
    /// Uses two contiguous copies instead of per-element modulo.
    pub fn read_window(&self, dest: &mut [f32]) {
        debug_assert!(dest.len() >= self.window_size);
        let start = self.write_pos; // oldest sample
        let first_len = self.window_size - start;
        dest[..first_len].copy_from_slice(&self.buffer[start..]);
        if start > 0 {
            dest[first_len..self.window_size].copy_from_slice(&self.buffer[..start]);
        }
    }

    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
        self.samples_since_trigger = 0;
        self.filled = false;
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hann_window_size_and_symmetry() {
        let window = generate_hann_window(8);
        assert_eq!(window.len(), 8);

        // Hann window should start near zero and peak at center
        assert!((window[0] - 0.0).abs() < 0.01);
        assert!((window[4] - 1.0).abs() < 0.01);

        // Symmetric: w[i] == w[N-i] for periodic Hann
        for i in 1..4 {
            assert!(
                (window[i] - window[8 - i]).abs() < 1e-6,
                "Window not symmetric at i={}: {} vs {}",
                i,
                window[i],
                window[8 - i]
            );
        }
    }

    #[test]
    fn test_sqrt_hann_cola_property() {
        // sqrt(Hann) analysis * sqrt(Hann) synthesis = Hann
        // Hann has perfect COLA at 50% overlap: w[i] + w[i+N/2] = 1.0
        let n = 256;
        let sqrt_window = generate_sqrt_hann_window(n);
        let hop = n / 2;

        for i in 0..hop {
            // Product of analysis and synthesis = Hann
            let hann_i = sqrt_window[i] * sqrt_window[i];
            let hann_shifted = sqrt_window[i + hop] * sqrt_window[i + hop];
            let sum = hann_i + hann_shifted;
            assert!(
                (sum - 1.0).abs() < 1e-5,
                "sqrt(Hann) COLA violated at i={}: sum={}, expected 1.0",
                i,
                sum
            );
        }
    }

    #[test]
    fn test_hann_window_cola_property() {
        // With 50% overlap, w[i] + w[i + N/2] should equal 1.0 (COLA)
        let n = 256;
        let window = generate_hann_window(n);
        let hop = n / 2;

        for i in 0..hop {
            let sum = window[i] + window[i + hop];
            assert!(
                (sum - 1.0).abs() < 1e-5,
                "COLA violated at i={}: sum={}, expected 1.0",
                i,
                sum
            );
        }
    }

    #[test]
    fn test_fft_roundtrip() {
        let fft_size = 256;
        let mut fft = RealFftProcessor::new_bidirectional(fft_size);

        // Fill with a known signal
        let original: Vec<f32> = (0..fft_size)
            .map(|i| (2.0 * std::f32::consts::PI * 10.0 * i as f32 / fft_size as f32).sin())
            .collect();
        fft.time_buffer.copy_from_slice(&original);

        // Forward then inverse
        fft.forward();
        fft.inverse();

        // Inverse FFT scales by fft_size, so divide
        let scale = 1.0 / fft_size as f32;
        for i in 0..fft_size {
            let recovered = fft.time_buffer[i] * scale;
            assert!(
                (recovered - original[i]).abs() < 1e-4,
                "FFT roundtrip mismatch at i={}: expected {}, got {}",
                i,
                original[i],
                recovered,
            );
        }
    }

    #[test]
    fn test_ring_accumulator_trigger_timing() {
        let window_size = 8;
        let hop_size = 4;
        let mut ring = RingAccumulator::new(window_size, hop_size);

        let mut triggers = Vec::new();
        for i in 0..24 {
            if ring.push(i as f32) {
                triggers.push(i);
            }
        }

        // First trigger at sample 7 (index 7 = 8th sample, filling window)
        // Then every hop_size (4) samples: 11, 15, 19, 23
        assert_eq!(triggers, vec![7, 11, 15, 19, 23]);
    }

    #[test]
    fn test_ring_accumulator_window_readout() {
        let window_size = 4;
        let hop_size = 2;
        let mut ring = RingAccumulator::new(window_size, hop_size);

        // Push 6 samples: [0, 1, 2, 3, 4, 5]
        // After 4 samples, ring is filled. After 6 samples (2 more = hop), trigger.
        // Ring state: write_pos = 2, buffer = [4, 5, 2, 3]
        // oldest-first read: [2, 3, 4, 5]
        for i in 0..6 {
            ring.push(i as f32);
        }

        let mut dest = vec![0.0; 4];
        ring.read_window(&mut dest);
        assert_eq!(dest, vec![2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn test_ring_accumulator_reset() {
        let mut ring = RingAccumulator::new(8, 4);

        // Fill and trigger
        for i in 0..12 {
            ring.push(i as f32);
        }
        assert!(ring.filled);

        ring.reset();
        assert!(!ring.filled);
        assert_eq!(ring.write_pos, 0);
        assert_eq!(ring.samples_since_trigger, 0);

        // Should not trigger until filled again
        let mut triggered = false;
        for _ in 0..4 {
            triggered |= ring.push(1.0);
        }
        assert!(!triggered, "Should not trigger before ring is filled again");
    }
}
