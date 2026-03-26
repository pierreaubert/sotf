// ============================================================================
// MVDR Beamformer — Minimum Variance Distortionless Response
// ============================================================================
//
// Adaptive beamformer that minimizes output power while preserving signals
// from the look direction. The weight computation is:
//   w(k) = R^{-1} d(k) / (d(k)^H R^{-1} d(k))
//
// where R is the noise spatial covariance matrix and d(k) is the steering vector.

use nalgebra::{Complex, DMatrix};

/// MVDR beamformer core.
#[derive(Debug)]
pub struct MvdrBeamformer {
    num_mics: usize,
    spectrum_size: usize,
    /// Per-bin noise covariance matrices [bin] → M×M complex matrix
    noise_cov: Vec<DMatrix<Complex<f32>>>,
    /// Diagonal loading factor
    diag_load: f32,
    /// Smoothing factor for covariance estimation
    alpha: f32,
    /// Energy threshold for noise detection
    noise_threshold: f32,
    /// Frame counter for initial learning period
    frame_count: usize,
    /// Pre-allocated weight buffer: [bin][mic]
    pub weights_buf: Vec<Vec<Complex<f32>>>,
    /// Pre-allocated beamformed output buffer: [bin]
    pub output_buf: Vec<Complex<f32>>,
}

impl MvdrBeamformer {
    /// Create a new MVDR beamformer.
    ///
    /// # Arguments
    /// * `num_mics` - Number of microphones
    /// * `spectrum_size` - Number of frequency bins (fft_size/2 + 1)
    pub fn new(num_mics: usize, spectrum_size: usize) -> Self {
        let identity = DMatrix::identity(num_mics, num_mics)
            .map(|x: f64| Complex::new(x as f32, 0.0));

        Self {
            num_mics,
            spectrum_size,
            noise_cov: vec![identity.clone(); spectrum_size],
            diag_load: 0.01,
            alpha: 0.95,
            noise_threshold: 0.01,
            frame_count: 0,
            weights_buf: vec![vec![Complex::new(0.0, 0.0); num_mics]; spectrum_size],
            output_buf: vec![Complex::new(0.0, 0.0); spectrum_size],
        }
    }

    /// Update noise covariance estimate from input frame.
    ///
    /// # Arguments
    /// * `stft_channels` - STFT data per mic: [mic][bin]
    pub fn update_noise_covariance(&mut self, stft_channels: &[Vec<Complex<f32>>]) {
        let m = self.num_mics.min(stft_channels.len());

        // Simple energy-based noise detection
        let total_energy: f32 = stft_channels[0].iter().map(|c| c.norm_sqr()).sum::<f32>()
            / self.spectrum_size as f32;

        // During initial frames or when energy is low, assume noise
        let is_noise = self.frame_count < 20 || total_energy < self.noise_threshold;
        self.frame_count += 1;

        if !is_noise {
            return;
        }

        for k in 0..self.spectrum_size {
            // Build observation vector x(k) for this bin
            let mut x = DMatrix::zeros(m, 1).map(|_: f64| Complex::new(0.0f32, 0.0));
            for i in 0..m {
                if k < stft_channels[i].len() {
                    x[(i, 0)] = stft_channels[i][k];
                }
            }

            // Rank-1 update: R = α*R + (1-α)*x*x^H
            let x_h = x.adjoint();
            let outer = &x * &x_h;

            let a = Complex::new(self.alpha, 0.0);
            let b = Complex::new(1.0 - self.alpha, 0.0);
            self.noise_cov[k] = &self.noise_cov[k] * a + outer * b;
        }
    }

    /// Compute MVDR weights for all bins.
    ///
    /// # Arguments
    /// * `steering` - Steering vectors per bin: [bin][mic]
    ///
    /// # Returns
    /// Beamforming weights per bin: [bin][mic]
    /// Compute MVDR weights into the pre-allocated weights_buf.
    /// Returns a reference to the internal buffer.
    pub fn compute_weights(&mut self, steering: &[Vec<Complex<f32>>]) -> &[Vec<Complex<f32>>] {
        let m = self.num_mics;

        for (k, d_vec) in steering.iter().enumerate() {
            if k >= self.spectrum_size || d_vec.len() != m {
                self.weights_buf[k].fill(Complex::new(0.0, 0.0));
                continue;
            }

            // Build steering vector as column matrix
            let d = DMatrix::from_fn(m, 1, |i, _| d_vec[i]);

            // Diagonal loading: R_loaded = R + σ*I
            let trace: f32 = (0..m).map(|i| self.noise_cov[k][(i, i)].re).sum();
            let sigma = self.diag_load * trace / m as f32;
            let r_loaded = &self.noise_cov[k]
                + DMatrix::identity(m, m).map(|x: f64| Complex::new(x as f32 * sigma, 0.0));

            // Compute R^{-1} d
            let r_inv_d = match r_loaded.clone().try_inverse() {
                Some(r_inv) => &r_inv * &d,
                None => d.clone(), // Fallback to delay-and-sum
            };

            // Compute d^H R^{-1} d (scalar)
            let d_h = d.adjoint();
            let denom = (&d_h * &r_inv_d)[(0, 0)];

            // w = R^{-1} d / (d^H R^{-1} d)
            if denom.norm_sqr() > 1e-20 {
                let w = &r_inv_d / denom;
                for i in 0..m {
                    self.weights_buf[k][i] = w[(i, 0)];
                }
            } else {
                // Fallback: uniform weights
                self.weights_buf[k].fill(Complex::new(1.0 / m as f32, 0.0));
            }
        }
        &self.weights_buf
    }

    /// Apply beamforming weights to produce single-channel output.
    ///
    /// # Arguments
    /// * `stft_channels` - STFT data per mic: [mic][bin]
    /// * `weights` - Beamforming weights: [bin][mic]
    ///
    /// # Returns
    /// Single-channel STFT output
    /// Apply beamforming weights in-place, writing to the pre-allocated output buffer.
    /// Returns a slice of the output.
    pub fn apply_weights_into(
        &mut self,
        stft_channels: &[Vec<Complex<f32>>],
        weights: &[Vec<Complex<f32>>],
    ) -> &[Complex<f32>] {
        let spectrum_size = weights.len().min(self.spectrum_size);
        let num_mics = stft_channels.len();

        for k in 0..spectrum_size {
            let mut sum = Complex::new(0.0, 0.0);
            for m in 0..num_mics {
                if k < stft_channels[m].len() && m < weights[k].len() {
                    sum += weights[k][m].conj() * stft_channels[m][k];
                }
            }
            self.output_buf[k] = sum;
        }
        &self.output_buf[..spectrum_size]
    }

    /// Reset noise covariance to identity.
    pub fn reset(&mut self) {
        // Reset noise covariance matrices in-place (avoids heap allocation)
        for mat in &mut self.noise_cov {
            mat.fill(Complex::new(0.0, 0.0));
            for i in 0..self.num_mics.min(mat.nrows()) {
                mat[(i, i)] = Complex::new(1.0, 0.0);
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
        let weights = vec![
            vec![Complex::new(0.5, 0.0), Complex::new(0.5, 0.0)],
            vec![Complex::new(0.5, 0.0), Complex::new(0.5, 0.0)],
        ];

        let mut bf = MvdrBeamformer::new(2, 2);
        let output = bf.apply_weights_into(&stft_channels, &weights);
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
