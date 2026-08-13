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
    /// Maximum coherent look-direction energy fraction still treated as noise.
    pub target_presence_threshold: f32,
    /// Frame counter for initial learning period
    frame_count: usize,
    weights_dirty: bool,
    #[cfg(test)]
    pub weight_solve_count: usize,
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
            target_presence_threshold: 0.65,
            frame_count: 0,
            weights_dirty: true,
            #[cfg(test)]
            weight_solve_count: 0,
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
    pub fn update_noise_covariance(
        &mut self,
        stft_channels: &[Vec<Complex<f32>>],
        steering: &[Vec<Complex<f32>>],
    ) -> bool {
        let m = self.num_mics.min(stft_channels.len());
        let bins = self.spectrum_size.min(steering.len());
        let mut total_energy = 0.0_f32;
        let mut look_energy = 0.0_f32;
        for k in 0..bins {
            let mut projection = Complex::new(0.0, 0.0);
            for mic in 0..m {
                let sample = stft_channels[mic].get(k).copied().unwrap_or_default();
                total_energy += sample.norm_sqr();
                if let Some(direction) = steering[k].get(mic) {
                    projection += direction.conj() * sample;
                }
            }
            look_energy += projection.norm_sqr() / m as f32;
        }
        let coherent_fraction = look_energy / total_energy.max(1e-20);
        let is_noise = total_energy <= 1e-20 || coherent_fraction < self.target_presence_threshold;
        self.frame_count += 1;

        if !is_noise {
            return false;
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
        self.weights_dirty = true;
        true
    }

    /// Compute MVDR weights for all bins.
    ///
    /// Zero allocations: uses fixed-size scratch buffers for all matrix math.
    pub fn compute_weights(&mut self, steering: &[Vec<Complex<f32>>]) -> &[Vec<Complex<f32>>] {
        #[cfg(test)]
        {
            self.weight_solve_count += 1;
        }
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

            // Hermitian positive-definite Cholesky solve. This computes
            // R^-1 d directly, avoiding both an explicit inverse and the
            // extra O(M^3) inverse materialization.
            if self.solve_loaded_hpd(d_vec, m) {
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
                    write_steered_delay_and_sum(&mut self.weights_buf[k], d_vec);
                }
            } else {
                write_steered_delay_and_sum(&mut self.weights_buf[k], d_vec);
            }
        }
        self.weights_dirty = false;
        &self.weights_buf
    }

    pub fn weights_dirty(&self) -> bool {
        self.weights_dirty
    }

    fn solve_loaded_hpd(&mut self, rhs: &[Complex<f32>], m: usize) -> bool {
        self.scratch_r_inv[..m * m].fill(Complex::new(0.0, 0.0));
        for row in 0..m {
            for col in 0..=row {
                let mut value = self.scratch_r_loaded[row * m + col];
                for k in 0..col {
                    value -=
                        self.scratch_r_inv[row * m + k] * self.scratch_r_inv[col * m + k].conj();
                }
                if row == col {
                    if !value.re.is_finite() || value.re <= 1e-20 || value.im.abs() > 1e-3 {
                        return false;
                    }
                    self.scratch_r_inv[row * m + col] = Complex::new(value.re.sqrt(), 0.0);
                } else {
                    self.scratch_r_inv[row * m + col] = value / self.scratch_r_inv[col * m + col];
                }
            }
        }

        // Forward solve L y = d; reuse the first M loaded scratch cells for y.
        for row in 0..m {
            let mut value = rhs[row];
            for col in 0..row {
                value -= self.scratch_r_inv[row * m + col] * self.scratch_r_loaded[col];
            }
            self.scratch_r_loaded[row] = value / self.scratch_r_inv[row * m + row];
        }
        // Back solve L^H x = y.
        for row in (0..m).rev() {
            let mut value = self.scratch_r_loaded[row];
            for col in row + 1..m {
                value -= self.scratch_r_inv[col * m + row].conj() * self.scratch_r_inv_d[col];
            }
            self.scratch_r_inv_d[row] = value / self.scratch_r_inv[row * m + row];
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
        self.weights_dirty = true;
        for w in &mut self.weights_buf {
            w.fill(Complex::new(0.0, 0.0));
        }
        self.output_buf.fill(Complex::new(0.0, 0.0));
    }
}

fn write_steered_delay_and_sum(dest: &mut [Complex<f32>], steering: &[Complex<f32>]) {
    let scale = 1.0 / steering.len().max(1) as f32;
    for (weight, direction) in dest.iter_mut().zip(steering) {
        *weight = *direction * scale;
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

    #[test]
    fn singular_fallback_is_steered_and_distortionless() {
        let steering = [vec![Complex::new(1.0, 0.0), Complex::new(0.0, -1.0)]];
        let mut weights = vec![Complex::new(0.0, 0.0); 2];
        write_steered_delay_and_sum(&mut weights, &steering[0]);
        let response = weights[0].conj() * steering[0][0] + weights[1].conj() * steering[0][1];
        assert!((response - Complex::new(1.0, 0.0)).norm() < 1e-6);
        assert!(
            weights[1].im.abs() > 0.1,
            "fallback must retain steering phase"
        );
    }

    #[test]
    fn target_presence_estimator_is_scale_invariant_and_marks_noise_dirty() {
        let mut bf = MvdrBeamformer::new(2, 2);
        let steering = vec![vec![Complex::new(1.0, 0.0); 2]; 2];
        for amplitude in [0.001, 1.0, 100.0] {
            let target = vec![vec![Complex::new(amplitude, 0.0); 2]; 2];
            assert!(!bf.update_noise_covariance(&target, &steering));
        }
        let diffuse = vec![
            vec![Complex::new(1.0, 0.0); 2],
            vec![Complex::new(0.0, 1.0); 2],
        ];
        assert!(bf.update_noise_covariance(&diffuse, &steering));
    }
}
