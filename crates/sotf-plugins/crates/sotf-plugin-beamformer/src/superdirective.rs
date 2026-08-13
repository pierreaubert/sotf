// ============================================================================
// Superdirective Fixed Beamformer
// ============================================================================
//
// A precomputed beamformer that assumes diffuse noise field and computes
// optimal weights offline. At runtime, it's just a complex dot product
// per bin — O(M) per bin, no adaptation overhead.
//
// The weights are computed as:
//   w(k) = Γ_reg^{-1} d(k) / (d(k)^H Γ_reg^{-1} d(k))
//
// where Γ is the diffuse noise coherence matrix and d is the steering vector.

use crate::steering::{ArrayGeometry, compute_all_steering_vectors};
use nalgebra::{Complex, DMatrix};

/// Precomputed superdirective beamformer.
pub struct SuperdirectiveBeamformer {
    /// Precomputed weights: [bin][mic]
    weights: Vec<Vec<Complex<f32>>>,
    spectrum_size: usize,
    /// Pre-allocated output buffer (avoids per-hop allocation)
    output_buf: Vec<Complex<f32>>,
}

impl SuperdirectiveBeamformer {
    /// Compute superdirective weights for a given array and steering direction.
    ///
    /// # Arguments
    /// * `geometry` - Microphone array geometry
    /// * `azimuth_deg` - Look direction in degrees
    /// * `fft_size` - FFT size
    /// * `sample_rate` - Sample rate in Hz
    /// * `regularization` - Regularization factor (ε, typically 0.001-0.1)
    pub fn new(
        geometry: &ArrayGeometry,
        azimuth_deg: f32,
        fft_size: usize,
        sample_rate: f32,
        regularization: f32,
    ) -> Self {
        let spectrum_size = fft_size / 2 + 1;
        let m = geometry.num_mics();
        let positions = geometry.positions();
        let steering = compute_all_steering_vectors(geometry, azimuth_deg, fft_size, sample_rate);

        let weights: Vec<Vec<Complex<f32>>> = (0..spectrum_size)
            .map(|k| {
                let freq = k as f32 * sample_rate / fft_size as f32;

                // Compute diffuse noise coherence matrix Γ
                // Γ_ij(f) = sinc(2π f d_ij / c) where d_ij is inter-mic distance
                let gamma = DMatrix::from_fn(m, m, |i, j| {
                    let dx = positions[i].0 - positions[j].0;
                    let dy = positions[i].1 - positions[j].1;
                    let dz = positions[i].2 - positions[j].2;
                    let dist = (dx * dx + dy * dy + dz * dz).sqrt();

                    if dist < 1e-10 || freq < 1e-2 {
                        // At DC or zero distance, coherence is 1.0 (sinc(0) = 1)
                        Complex::new(1.0, 0.0)
                    } else {
                        let arg = 2.0 * std::f32::consts::PI * freq * dist / 343.0;
                        let sinc = if arg.abs() < 1e-6 {
                            1.0
                        } else {
                            arg.sin() / arg
                        };
                        Complex::new(sinc, 0.0)
                    }
                });

                // Regularize: Γ_reg = (1-ε)Γ + εI
                let eps = regularization;
                let identity = DMatrix::identity(m, m).map(|x: f64| Complex::new(x as f32, 0.0));
                let gamma_reg =
                    &gamma * Complex::new(1.0 - eps, 0.0) + &identity * Complex::new(eps, 0.0);

                // Compute weights: w = Γ_reg^{-1} d / (d^H Γ_reg^{-1} d)
                let d = DMatrix::from_fn(m, 1, |i, _| {
                    if k < steering.len() && i < steering[k].len() {
                        steering[k][i]
                    } else {
                        Complex::new(0.0, 0.0)
                    }
                });

                let gamma_inv_d = match gamma_reg.try_inverse() {
                    Some(inv) => &inv * &d,
                    // Coherence matrix is singular: fall back immediately to
                    // explicit uniform delay-and-sum weights (1/m per mic).
                    // Returning here avoids the confusing path where d.clone()
                    // accidentally produced the same result through cancellation.
                    None => {
                        return steered_delay_and_sum(&steering[k]);
                    }
                };

                let d_h = d.adjoint();
                let denom = (&d_h * &gamma_inv_d)[(0, 0)];

                if denom.norm_sqr() > 1e-20 {
                    let w = &gamma_inv_d / denom;
                    (0..m).map(|i| w[(i, 0)]).collect()
                } else {
                    steered_delay_and_sum(&steering[k])
                }
            })
            .collect();

        Self {
            weights,
            output_buf: vec![Complex::new(0.0, 0.0); spectrum_size],
            spectrum_size,
        }
    }

    /// Apply precomputed weights to produce single-channel output.
    ///
    /// # Arguments
    /// * `stft_channels` - STFT data per mic: [mic][bin]
    ///
    /// # Returns
    /// Single-channel STFT output
    pub fn apply(&mut self, stft_channels: &[Vec<Complex<f32>>]) -> &[Complex<f32>] {
        let num_mics = stft_channels.len();

        for k in 0..self.spectrum_size {
            let mut sum = Complex::new(0.0, 0.0);
            for (m, ch) in stft_channels.iter().enumerate().take(num_mics) {
                if k < ch.len() && m < self.weights[k].len() {
                    sum += self.weights[k][m].conj() * ch[k];
                }
            }
            self.output_buf[k] = sum;
        }
        &self.output_buf[..self.spectrum_size]
    }

    /// Get spectrum size.
    pub fn spectrum_size(&self) -> usize {
        self.spectrum_size
    }
}

fn steered_delay_and_sum(steering: &[Complex<f32>]) -> Vec<Complex<f32>> {
    let scale = 1.0 / steering.len().max(1) as f32;
    steering
        .iter()
        .map(|direction| *direction * scale)
        .collect()
}

impl std::fmt::Debug for SuperdirectiveBeamformer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SuperdirectiveBeamformer")
            .field("spectrum_size", &self.spectrum_size)
            .field("num_mics", &self.weights.first().map_or(0, Vec::len))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_superdirective_creation() {
        let geom = ArrayGeometry::Linear {
            num_mics: 4,
            spacing_m: 0.04,
        };

        let bf = SuperdirectiveBeamformer::new(&geom, 0.0, 512, 48000.0, 0.01);
        assert_eq!(bf.spectrum_size(), 257);
    }

    #[test]
    fn test_superdirective_weights_finite() {
        let geom = ArrayGeometry::Linear {
            num_mics: 2,
            spacing_m: 0.05,
        };

        let bf = SuperdirectiveBeamformer::new(&geom, 0.0, 256, 48000.0, 0.01);

        for (k, w) in bf.weights.iter().enumerate() {
            for (m, c) in w.iter().enumerate() {
                assert!(
                    c.re.is_finite() && c.im.is_finite(),
                    "Weight at bin {k}, mic {m} is not finite: {c}"
                );
            }
        }
    }

    #[test]
    fn test_superdirective_apply() {
        let geom = ArrayGeometry::Linear {
            num_mics: 2,
            spacing_m: 0.05,
        };

        let mut bf = SuperdirectiveBeamformer::new(&geom, 0.0, 256, 48000.0, 0.01);

        let stft_channels = vec![
            vec![Complex::new(1.0, 0.0); 129],
            vec![Complex::new(1.0, 0.0); 129],
        ];

        let output = bf.apply(&stft_channels);
        assert_eq!(output.len(), 129);
        for c in output {
            assert!(c.re.is_finite() && c.im.is_finite());
        }
    }

    #[test]
    fn steered_fallback_preserves_look_direction() {
        let d = [Complex::new(1.0, 0.0), Complex::new(0.0, -1.0)];
        let weights = steered_delay_and_sum(&d);
        let response: Complex<f32> = weights.iter().zip(d).map(|(w, x)| w.conj() * x).sum();
        assert!((response - Complex::new(1.0, 0.0)).norm() < 1e-6);
    }
}
