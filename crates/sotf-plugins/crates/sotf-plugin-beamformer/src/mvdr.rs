// ============================================================================
// MVDR Beamformer — Minimum Variance Distortionless Response
// ============================================================================
// Matrix math uses index-based loops for clarity with multi-array access.
#![allow(clippy::needless_range_loop)]
//
// Adaptive beamformer that minimizes output power while preserving signals
// from the look direction. The weight computation is:
//   w(k) = R^{-1} d(k) / (d(k)^H R^{-1} d(k))
//
// where R is the noise spatial covariance matrix and d(k) is the steering vector.
//
// All math is done with pre-allocated flat buffers — zero heap allocations
// in update_noise_covariance, compute_weights, and apply_weights_into.

use nalgebra::Complex;

/// Maximum number of microphones supported.
pub const MAX_MICS: usize = 8;

/// MVDR beamformer core.
#[derive(Debug)]
pub struct MvdrBeamformer {
    num_mics: usize,
    spectrum_size: usize,
    /// Per-bin noise covariance matrices, stored as flat row-major [bin * M*M]
    noise_cov: Vec<Complex<f32>>,
    /// Diagonal loading factor
    diag_load: f32,
    /// Smoothing factor for covariance estimation
    alpha: f32,
    /// Energy threshold for noise detection (public for testing)
    pub noise_threshold: f32,
    /// Frame counter for initial learning period
    frame_count: usize,
    /// Pre-allocated weight buffer: [bin][mic]
    pub weights_buf: Vec<Vec<Complex<f32>>>,
    /// Pre-allocated beamformed output buffer: [bin]
    pub output_buf: Vec<Complex<f32>>,
    // Scratch buffers for compute_weights (avoid per-call allocation)
    scratch_r_loaded: [Complex<f32>; MAX_MICS * MAX_MICS],
    scratch_r_inv: [Complex<f32>; MAX_MICS * MAX_MICS],
    scratch_r_inv_d: [Complex<f32>; MAX_MICS],
    scratch_outer: [Complex<f32>; MAX_MICS * MAX_MICS],
}

impl MvdrBeamformer {
    /// Create a new MVDR beamformer.
    ///
    /// # Arguments
    /// * `num_mics` - Number of microphones (max MAX_MICS)
    /// * `spectrum_size` - Number of frequency bins (fft_size/2 + 1)
    pub fn new(num_mics: usize, spectrum_size: usize) -> Self {
        assert!(
            num_mics <= MAX_MICS,
            "num_mics ({num_mics}) > MAX_MICS ({MAX_MICS})"
        );

        // Initialize noise covariance as identity for each bin
        let mut noise_cov = vec![Complex::new(0.0, 0.0); spectrum_size * num_mics * num_mics];
        for k in 0..spectrum_size {
            for i in 0..num_mics {
                noise_cov[k * num_mics * num_mics + i * num_mics + i] = Complex::new(1.0, 0.0);
            }
        }

        Self {
            num_mics,
            spectrum_size,
            noise_cov,
            diag_load: 0.01,
            alpha: 0.95,
            noise_threshold: 0.01,
            frame_count: 0,
            weights_buf: vec![vec![Complex::new(0.0, 0.0); num_mics]; spectrum_size],
            output_buf: vec![Complex::new(0.0, 0.0); spectrum_size],
            scratch_r_loaded: [Complex::new(0.0, 0.0); MAX_MICS * MAX_MICS],
            scratch_r_inv: [Complex::new(0.0, 0.0); MAX_MICS * MAX_MICS],
            scratch_r_inv_d: [Complex::new(0.0, 0.0); MAX_MICS],
            scratch_outer: [Complex::new(0.0, 0.0); MAX_MICS * MAX_MICS],
        }
    }

    /// Update noise covariance estimate from input frame.
    ///
    /// Zero allocations: uses inline math on the flat noise_cov buffer.
    pub fn update_noise_covariance(&mut self, stft_channels: &[Vec<Complex<f32>>]) {
        let m = self.num_mics.min(stft_channels.len());

        // Energy-based noise detection: average energy across all channels so
        // that a quiet mic 0 while other mics are active does not misclassify
        // target frames as noise (bug §1.6).  The unconditional 20-frame
        // learning period is removed because it contaminates the covariance
        // with the target signal whenever the source is active at startup
        // (bug §1.7).
        let total_energy: f32 = stft_channels[..m]
            .iter()
            .map(|ch| ch.iter().map(|c| c.norm_sqr()).sum::<f32>())
            .sum::<f32>()
            / (self.spectrum_size * m) as f32;

        let is_noise = total_energy < self.noise_threshold;
        self.frame_count += 1;

        if !is_noise {
            return;
        }

        let alpha = self.alpha;
        let one_minus_alpha = 1.0 - self.alpha;
        let mm = m * m;

        for k in 0..self.spectrum_size {
            let cov_off = k * self.num_mics * self.num_mics;

            // Build outer product x * x^H directly into scratch_outer
            // x[i] = stft_channels[i][k]
            for i in 0..m {
                let xi = if k < stft_channels[i].len() {
                    stft_channels[i][k]
                } else {
                    Complex::new(0.0, 0.0)
                };
                for j in 0..m {
                    let xj = if k < stft_channels[j].len() {
                        stft_channels[j][k]
                    } else {
                        Complex::new(0.0, 0.0)
                    };
                    // outer[i,j] = x[i] * conj(x[j])
                    self.scratch_outer[i * m + j] = xi * xj.conj();
                }
            }

            // R = α*R + (1-α)*outer
            // Use real scalar multiplies — a purely-real complex multiply
            // costs 4 muls + 2 adds, but here the scalars are real so we
            // can do 2 muls + 1 add per element (§3.3).
            for idx in 0..mm {
                let r = &mut self.noise_cov[cov_off + idx];
                let s = self.scratch_outer[idx];
                *r = Complex::new(
                    r.re * alpha + s.re * one_minus_alpha,
                    r.im * alpha + s.im * one_minus_alpha,
                );
            }
        }
    }

    /// Compute MVDR weights for all bins.
    ///
    /// Zero allocations: uses fixed-size scratch buffers for all matrix math.
    pub fn compute_weights(&mut self, steering: &[Vec<Complex<f32>>]) -> &[Vec<Complex<f32>>] {
        let m = self.num_mics;
        let mm = m * m;

        for (k, d_vec) in steering.iter().enumerate() {
            if k >= self.spectrum_size || d_vec.len() != m {
                self.weights_buf[k].fill(Complex::new(0.0, 0.0));
                continue;
            }

            let cov_off = k * m * m;

            // Diagonal loading: R_loaded = R + σ*I
            let trace: f32 = (0..m).map(|i| self.noise_cov[cov_off + i * m + i].re).sum();
            let sigma = self.diag_load * trace / m as f32;

            // Copy R into scratch_r_loaded and add diagonal loading
            self.scratch_r_loaded[..mm].copy_from_slice(&self.noise_cov[cov_off..cov_off + mm]);
            for i in 0..m {
                self.scratch_r_loaded[i * m + i] += Complex::new(sigma, 0.0);
            }

            // Invert R_loaded in-place using Gauss-Jordan on scratch buffers
            let invertible = self.invert_matrix(m);

            if invertible {
                // r_inv_d = R^{-1} * d
                for i in 0..m {
                    let mut sum = Complex::new(0.0, 0.0);
                    for j in 0..m {
                        sum += self.scratch_r_inv[i * m + j] * d_vec[j];
                    }
                    self.scratch_r_inv_d[i] = sum;
                }

                // denom = d^H * r_inv_d (scalar)
                let mut denom = Complex::new(0.0, 0.0);
                for i in 0..m {
                    denom += d_vec[i].conj() * self.scratch_r_inv_d[i];
                }

                if denom.norm_sqr() > 1e-20 {
                    // w = r_inv_d / denom
                    for i in 0..m {
                        self.weights_buf[k][i] = self.scratch_r_inv_d[i] / denom;
                    }
                } else {
                    self.weights_buf[k].fill(Complex::new(1.0 / m as f32, 0.0));
                }
            } else {
                // Fallback to delay-and-sum (uniform weights)
                self.weights_buf[k].fill(Complex::new(1.0 / m as f32, 0.0));
            }
        }
        &self.weights_buf
    }

    /// Gauss-Jordan inversion of scratch_r_loaded into scratch_r_inv.
    /// Returns false if the matrix is singular.
    fn invert_matrix(&mut self, m: usize) -> bool {
        // Initialize scratch_r_inv as identity
        for i in 0..m {
            for j in 0..m {
                self.scratch_r_inv[i * m + j] = if i == j {
                    Complex::new(1.0, 0.0)
                } else {
                    Complex::new(0.0, 0.0)
                };
            }
        }

        // Gauss-Jordan elimination with partial pivoting
        for col in 0..m {
            // Find pivot (largest magnitude in column)
            let mut max_mag = self.scratch_r_loaded[col * m + col].norm_sqr();
            let mut pivot_row = col;
            for row in (col + 1)..m {
                let mag = self.scratch_r_loaded[row * m + col].norm_sqr();
                if mag > max_mag {
                    max_mag = mag;
                    pivot_row = row;
                }
            }

            if max_mag < 1e-30 {
                return false; // Singular
            }

            // Swap rows if needed
            if pivot_row != col {
                for j in 0..m {
                    self.scratch_r_loaded.swap(col * m + j, pivot_row * m + j);
                    self.scratch_r_inv.swap(col * m + j, pivot_row * m + j);
                }
            }

            // Scale pivot row
            let pivot = self.scratch_r_loaded[col * m + col];
            let inv_pivot = Complex::new(1.0, 0.0) / pivot;
            for j in 0..m {
                self.scratch_r_loaded[col * m + j] *= inv_pivot;
                self.scratch_r_inv[col * m + j] *= inv_pivot;
            }

            // Eliminate column in all other rows
            for row in 0..m {
                if row == col {
                    continue;
                }
                let factor = self.scratch_r_loaded[row * m + col];
                for j in 0..m {
                    self.scratch_r_loaded[row * m + j] -=
                        factor * self.scratch_r_loaded[col * m + j];
                    self.scratch_r_inv[row * m + j] -= factor * self.scratch_r_inv[col * m + j];
                }
            }
        }

        true
    }

    /// Apply beamforming weights to produce single-channel output.
    ///
    /// Zero allocations: writes directly to pre-allocated output_buf.
    pub fn apply_weights_into(&mut self, stft_channels: &[Vec<Complex<f32>>]) -> &[Complex<f32>] {
        let spectrum_size = self.weights_buf.len().min(self.spectrum_size);
        let num_mics = stft_channels.len();

        for k in 0..spectrum_size {
            let mut sum = Complex::new(0.0, 0.0);
            for m in 0..num_mics {
                if k < stft_channels[m].len() && m < self.weights_buf[k].len() {
                    sum += self.weights_buf[k][m].conj() * stft_channels[m][k];
                }
            }
            self.output_buf[k] = sum;
        }
        &self.output_buf[..spectrum_size]
    }

    /// Read-only view of the noise covariance flat buffer.
    /// Exposed for testing only.
    #[cfg(test)]
    pub fn noise_cov_snapshot(&self) -> &[Complex<f32>] {
        &self.noise_cov
    }

    /// Reset noise covariance to identity.
    pub fn reset(&mut self) {
        let m = self.num_mics;
        for k in 0..self.spectrum_size {
            let off = k * m * m;
            for idx in 0..(m * m) {
                self.noise_cov[off + idx] = Complex::new(0.0, 0.0);
            }
            for i in 0..m {
                self.noise_cov[off + i * m + i] = Complex::new(1.0, 0.0);
            }
        }
        self.frame_count = 0;
        for w in &mut self.weights_buf {
            w.fill(Complex::new(0.0, 0.0));
        }
        self.output_buf.fill(Complex::new(0.0, 0.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mvdr_creation() {
        let bf = MvdrBeamformer::new(4, 257);
        assert_eq!(bf.num_mics, 4);
        assert_eq!(bf.spectrum_size, 257);
    }

    #[test]
    fn test_mvdr_weights_no_nan() {
        let mut bf = MvdrBeamformer::new(2, 8);
        let steering: Vec<Vec<Complex<f32>>> = (0..8)
            .map(|_| vec![Complex::new(1.0, 0.0), Complex::new(0.7, 0.7)])
            .collect();

        let weights = bf.compute_weights(&steering);
        assert_eq!(weights.len(), 8);

        for (k, w) in weights.iter().enumerate() {
            assert_eq!(w.len(), 2);
            for (m, c) in w.iter().enumerate() {
                assert!(
                    c.re.is_finite() && c.im.is_finite(),
                    "Weight at bin {k}, mic {m} is not finite: {c}"
                );
            }
        }
    }

    #[test]
    fn test_mvdr_apply_weights() {
        let stft_channels = vec![
            vec![Complex::new(1.0, 0.0), Complex::new(0.5, 0.5)],
            vec![Complex::new(0.8, 0.2), Complex::new(0.3, -0.3)],
        ];
        let mut bf = MvdrBeamformer::new(2, 2);
        bf.weights_buf = vec![
            vec![Complex::new(0.5, 0.0), Complex::new(0.5, 0.0)],
            vec![Complex::new(0.5, 0.0), Complex::new(0.5, 0.0)],
        ];

        let output = bf.apply_weights_into(&stft_channels);
        assert_eq!(output.len(), 2);
        for c in output {
            assert!(c.re.is_finite() && c.im.is_finite());
        }
    }

    #[test]
    fn test_mvdr_reset() {
        let mut bf = MvdrBeamformer::new(2, 4);
        bf.frame_count = 100;
        bf.reset();
        assert_eq!(bf.frame_count, 0);
    }
}
