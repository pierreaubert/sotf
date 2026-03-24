// ============================================================================
// PBFDAF — Partitioned Block Frequency Domain Adaptive Filter
// ============================================================================
//
// Core adaptive filter for acoustic echo cancellation. Operates in the
// frequency domain using partitioned convolution for efficient long-filter
// adaptation (echo tails of 50-500ms).
//
// The filter partitions the echo path into blocks, FFTs each block, and
// uses NLMS-style weight updates in the frequency domain.

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use std::sync::Arc;

/// Partitioned Block Frequency Domain Adaptive Filter.
pub struct Pbfdaf {
    block_size: usize,
    fft_size: usize,
    num_partitions: usize,
    /// Adaptive filter weights [partition][bin]
    weights: Vec<Vec<Complex<f32>>>,
    /// Frequency Domain delay Line [partition][bin] — ring buffer of FFT'd reference blocks
    fdl: Vec<Vec<Complex<f32>>>,
    fdl_head: usize,
    /// Pre-allocated error output buffer
    error_buf: Vec<f32>,
    /// Per-bin power across all FDL partitions (for NLMS normalization)
    power_sum: Vec<f32>,
    /// Step size (learning rate)
    mu: f32,
    /// Regularization constant
    delta: f32,
    /// FFT processors
    fft_forward: Arc<dyn Fft<f32>>,
    fft_inverse: Arc<dyn Fft<f32>>,
    /// Scratch buffers
    fft_scratch: Vec<Complex<f32>>,
    fft_buf: Vec<Complex<f32>>,
    error_freq: Vec<Complex<f32>>,
    output_buf: Vec<Complex<f32>>,
}

impl Pbfdaf {
    /// Create a new PBFDAF.
    ///
    /// # Arguments
    /// * `block_size` - Processing block size (B). Determines latency.
    /// * `echo_tail_samples` - Length of echo path to model in samples.
    /// * `mu` - Step size (0.1-0.9, default 0.5).
    /// * `delta` - Regularization (default 1e-6).
    pub fn new(block_size: usize, echo_tail_samples: usize, mu: f32, delta: f32) -> Self {
        let fft_size = block_size * 2;
        let num_partitions = echo_tail_samples.div_ceil(block_size).max(1);

        let mut planner = FftPlanner::new();
        let fft_forward = planner.plan_fft_forward(fft_size);
        let fft_inverse = planner.plan_fft_inverse(fft_size);
        let scratch_len = fft_forward
            .get_inplace_scratch_len()
            .max(fft_inverse.get_inplace_scratch_len());

        Self {
            block_size,
            fft_size,
            num_partitions,
            weights: vec![vec![Complex::new(0.0, 0.0); fft_size]; num_partitions],
            fdl: vec![vec![Complex::new(0.0, 0.0); fft_size]; num_partitions],
            fdl_head: 0,
            error_buf: vec![0.0; block_size],
            power_sum: vec![delta; fft_size],
            mu,
            delta,
            fft_forward,
            fft_inverse,
            fft_scratch: vec![Complex::new(0.0, 0.0); scratch_len],
            fft_buf: vec![Complex::new(0.0, 0.0); fft_size],
            error_freq: vec![Complex::new(0.0, 0.0); fft_size],
            output_buf: vec![Complex::new(0.0, 0.0); fft_size],
        }
    }

    /// Process one block: cancel echo from microphone using reference signal.
    ///
    /// # Arguments
    /// * `mic` - Microphone input (contains echo + near-end speech)
    /// * `reference` - Far-end reference signal (echo source)
    ///
    /// # Returns
    /// Error signal (mic with echo cancelled). Length = `block_size`.
    pub fn process(&mut self, mic: &[f32], reference: &[f32]) -> &[f32] {
        let b = self.block_size;
        debug_assert!(mic.len() >= b);
        debug_assert!(reference.len() >= b);

        // FFT reference block (zero-padded to fft_size)
        for (dst, &src) in self.fft_buf[..b].iter_mut().zip(&reference[..b]) {
            *dst = Complex::new(src, 0.0);
        }
        for i in b..self.fft_size {
            self.fft_buf[i] = Complex::new(0.0, 0.0);
        }
        self.fft_forward
            .process_with_scratch(&mut self.fft_buf, &mut self.fft_scratch);

        // Push into FDL ring buffer
        self.fdl_head = if self.fdl_head == 0 {
            self.num_partitions - 1
        } else {
            self.fdl_head - 1
        };
        self.fdl[self.fdl_head].copy_from_slice(&self.fft_buf);

        // Compute echo estimate: Y = Σ_m W[m] ⊙ FDL[m]
        self.output_buf.fill(Complex::new(0.0, 0.0));
        for p in 0..self.num_partitions {
            let fdl_idx = (self.fdl_head + p) % self.num_partitions;
            for k in 0..self.fft_size {
                self.output_buf[k] += self.weights[p][k] * self.fdl[fdl_idx][k];
            }
        }

        // IFFT echo estimate
        self.fft_inverse
            .process_with_scratch(&mut self.output_buf, &mut self.fft_scratch);

        // Extract error: e = mic - y (take last B samples for overlap-save)
        let inv_n = 1.0 / self.fft_size as f32;
        for (i, (err, &m)) in self.error_buf[..b].iter_mut().zip(&mic[..b]).enumerate() {
            let echo_est = self.output_buf[b + i].re * inv_n;
            *err = m - echo_est;
        }

        // FFT error for weight update
        for i in 0..b {
            self.error_freq[i] = Complex::new(0.0, 0.0); // Zero-pad first half
        }
        for i in 0..b {
            self.error_freq[b + i] = Complex::new(self.error_buf[i], 0.0);
        }
        self.fft_forward
            .process_with_scratch(&mut self.error_freq, &mut self.fft_scratch);

        // Compute total power across all FDL partitions per bin (proper NLMS)
        self.power_sum.fill(0.0);
        for p in 0..self.num_partitions {
            let fdl_idx = (self.fdl_head + p) % self.num_partitions;
            for k in 0..self.fft_size {
                self.power_sum[k] += self.fdl[fdl_idx][k].norm_sqr();
            }
        }

        // Update weights: W[m] += μ * conj(FDL[m]) ⊙ E / (total_power + δ)
        let leak = 1.0 - 1e-5;
        for p in 0..self.num_partitions {
            let fdl_idx = (self.fdl_head + p) % self.num_partitions;
            for k in 0..self.fft_size {
                let norm = self.power_sum[k] + self.delta;
                let update = self.error_freq[k] * self.fdl[fdl_idx][k].conj() * self.mu / norm;
                let w = &mut self.weights[p][k];
                *w = (*w + update) * leak;
                // Prevent NaN propagation
                if !w.re.is_finite() {
                    w.re = 0.0;
                }
                if !w.im.is_finite() {
                    w.im = 0.0;
                }
            }
        }

        &self.error_buf[..b]
    }

    /// Reset all adaptive filter state.
    pub fn reset(&mut self) {
        for w in &mut self.weights {
            w.fill(Complex::new(0.0, 0.0));
        }
        for fdl in &mut self.fdl {
            fdl.fill(Complex::new(0.0, 0.0));
        }
        self.fdl_head = 0;
        self.power_sum.fill(self.delta);
    }

    /// Get current ERLE (Echo Return Loss Enhancement) in dB.
    /// Requires tracking of mic and error power externally.
    pub fn block_size(&self) -> usize {
        self.block_size
    }
}

impl std::fmt::Debug for Pbfdaf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pbfdaf")
            .field("block_size", &self.block_size)
            .field("num_partitions", &self.num_partitions)
            .field("mu", &self.mu)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pbfdaf_creation() {
        let filter = Pbfdaf::new(256, 4800, 0.5, 1e-6);
        assert_eq!(filter.block_size, 256);
        assert_eq!(filter.num_partitions, 19); // ceil(4800/256)
    }

    #[test]
    fn test_pbfdaf_reset() {
        let mut filter = Pbfdaf::new(256, 2400, 0.5, 1e-6);

        // Process some data
        let mic = vec![0.1; 256];
        let reference = vec![0.2; 256];
        let _ = filter.process(&mic, &reference);

        // Verify weights are non-zero
        let has_nonzero = filter.weights.iter().any(|part| part.iter().any(|c| c.norm() > 0.0));
        assert!(has_nonzero);

        filter.reset();

        // After reset, all weights should be zero
        for part in &filter.weights {
            for c in part {
                assert_eq!(c.norm(), 0.0);
            }
        }
    }

    #[test]
    fn test_pbfdaf_echo_cancellation() {
        let block_size = 256;
        let delay_samples = 100;
        let echo_tail = 512;
        let mut filter = Pbfdaf::new(block_size, echo_tail, 0.7, 1e-6);

        // Generate reference signal
        let num_blocks = 200;
        let mut reference_history = Vec::new();

        let mut mic_power_sum = 0.0f32;
        let mut error_power_sum = 0.0f32;
        let mut block_count = 0;

        for block_idx in 0..num_blocks {
            // Generate reference (pseudo-random)
            let reference: Vec<f32> = (0..block_size)
                .map(|i| {
                    let t = (block_idx * block_size + i) as f32;
                    (t * 0.1).sin() * 0.5 + (t * 0.37).sin() * 0.3
                })
                .collect();
            reference_history.extend_from_slice(&reference);

            // Simulate echo: delayed version of reference with attenuation
            let mic: Vec<f32> = (0..block_size)
                .map(|i| {
                    let global_i = block_idx * block_size + i;
                    if global_i >= delay_samples {
                        let ref_idx = global_i - delay_samples;
                        if ref_idx < reference_history.len() {
                            reference_history[ref_idx] * 0.6 // -4.4 dB echo
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    }
                })
                .collect();

            let error = filter.process(&mic, &reference);

            // Measure power in last quarter of blocks (after convergence)
            if block_idx >= num_blocks * 3 / 4 {
                mic_power_sum += mic.iter().map(|x| x * x).sum::<f32>();
                error_power_sum += error.iter().map(|x| x * x).sum::<f32>();
                block_count += 1;
            }
        }

        if block_count > 0 && mic_power_sum > 0.0 {
            let erle_db = 10.0 * (mic_power_sum / error_power_sum.max(1e-20)).log10();
            assert!(
                erle_db > 5.0,
                "ERLE should be > 5 dB after convergence, got {erle_db:.1} dB"
            );
        }
    }
}
