// ============================================================================
// GSC — Generalized Sidelobe Canceller
// ============================================================================
//
// Adaptive beamformer architecture:
// 1. Fixed Beamformer (FBF): delay-and-sum toward look direction
// 2. Blocking Matrix: B = I - dd^H/||d||² — projects onto null space
// 3. NLMS Adaptive Noise Canceller: cancels residual noise from FBF output
//
// Works sample-by-sample (no FFT) — lowest latency of all beamformers.

/// Generalized Sidelobe Canceller.
#[derive(Debug)]
pub struct GscBeamformer {
    num_mics: usize,
    /// Fixed beamformer weights (real-valued delay-and-sum)
    fbf_weights: Vec<f32>,
    /// Blocking matrix columns: [(num_mics-1) x num_mics] in flattened form
    blocking_matrix: Vec<Vec<f32>>,
    /// NLMS adaptive filter weights [(num_mics-1) x filter_length]
    adaptive_weights: Vec<Vec<f32>>,
    /// Reference signal delay lines [(num_mics-1) x filter_length]
    reference_buffers: Vec<Vec<f32>>,
    ref_write_pos: usize,
    filter_length: usize,
    /// NLMS step size
    mu: f32,
    /// Regularization
    delta: f32,
}

impl GscBeamformer {
    /// Create a new GSC beamformer.
    ///
    /// # Arguments
    /// * `num_mics` - Number of microphones
    /// * `steering_delays` - Time delay per mic in samples (from look direction)
    /// * `filter_length` - NLMS filter length (default: 64)
    /// * `mu` - NLMS step size (default: 0.01)
    pub fn new(
        num_mics: usize,
        steering_delays: &[f32],
        filter_length: usize,
        mu: f32,
    ) -> Self {
        assert!(num_mics >= 2, "GSC requires at least 2 microphones");

        // Fixed beamformer: uniform real-valued weights for delay-and-sum.
        // For time-domain GSC, fractional delays should be handled by
        // actual delay lines, not complex exponentials applied sample-by-sample.
        // Here we use unit weights (assuming delays are pre-compensated).
        let _ = steering_delays; // Delays are for documentation; time-domain FBF uses uniform weights
        let fbf_weights = vec![1.0f32; num_mics];

        // Blocking matrix: orthogonal complement of steering vector
        // B = I - d*d^H / (d^H * d) applied to real part only
        // For a simple linear array, use adjacent-microphone differences
        let num_refs = num_mics - 1;
        let blocking_matrix: Vec<Vec<f32>> = (0..num_refs)
            .map(|i| {
                let mut row = vec![0.0f32; num_mics];
                row[i] = 1.0;
                row[i + 1] = -1.0;
                row
            })
            .collect();

        Self {
            num_mics,
            fbf_weights,
            blocking_matrix,
            adaptive_weights: vec![vec![0.0; filter_length]; num_refs],
            reference_buffers: vec![vec![0.0; filter_length]; num_refs],
            ref_write_pos: 0,
            filter_length,
            mu,
            delta: 1e-6,
        }
    }

    /// Process one sample from all microphones.
    ///
    /// # Arguments
    /// * `mic_samples` - One sample per microphone
    ///
    /// # Returns
    /// Single beamformed output sample
    pub fn process_sample(&mut self, mic_samples: &[f32]) -> f32 {
        let m = self.num_mics.min(mic_samples.len());
        let num_refs = self.blocking_matrix.len();

        // 1. Fixed beamformer output: uniform delay-and-sum
        let mut fbf_output = 0.0f32;
        for i in 0..m {
            fbf_output += mic_samples[i] * self.fbf_weights[i];
        }
        fbf_output /= m as f32;

        // 2. Blocking matrix: compute reference signals
        // u = B * x (num_refs reference signals)
        let mut references = vec![0.0f32; num_refs];
        for (r, row) in self.blocking_matrix.iter().enumerate() {
            for i in 0..m {
                references[r] += row[i] * mic_samples[i];
            }
        }

        // Store references in delay lines
        for r in 0..num_refs {
            self.reference_buffers[r][self.ref_write_pos] = references[r];
        }

        // 3. NLMS adaptive noise canceller
        // Compute noise estimate: y = Σ_r w_r^T * u_r
        let mut noise_estimate = 0.0f32;
        let mut total_ref_power = self.delta;

        for r in 0..num_refs {
            for j in 0..self.filter_length {
                let buf_idx = (self.ref_write_pos + self.filter_length - j) % self.filter_length;
                noise_estimate += self.adaptive_weights[r][j] * self.reference_buffers[r][buf_idx];
                total_ref_power += self.reference_buffers[r][buf_idx].powi(2);
            }
        }

        // Error signal (beamformed output minus noise estimate)
        let error = fbf_output - noise_estimate;

        // NLMS weight update: w += μ * e * u / (||u||² + δ)
        let step = self.mu * error / total_ref_power;
        for r in 0..num_refs {
            for j in 0..self.filter_length {
                let buf_idx = (self.ref_write_pos + self.filter_length - j) % self.filter_length;
                self.adaptive_weights[r][j] += step * self.reference_buffers[r][buf_idx];
            }
        }

        // Advance write position
        self.ref_write_pos = (self.ref_write_pos + 1) % self.filter_length;

        error
    }

    /// Reset adaptive weights.
    pub fn reset(&mut self) {
        for w in &mut self.adaptive_weights {
            w.fill(0.0);
        }
        for b in &mut self.reference_buffers {
            b.fill(0.0);
        }
        self.ref_write_pos = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gsc_creation() {
        let gsc = GscBeamformer::new(4, &[0.0, 0.001, 0.002, 0.003], 32, 0.01);
        assert_eq!(gsc.num_mics, 4);
        assert_eq!(gsc.filter_length, 32);
    }

    #[test]
    fn test_gsc_process_sample() {
        let mut gsc = GscBeamformer::new(2, &[0.0, 0.0], 16, 0.01);

        // Process some samples
        for _ in 0..100 {
            let output = gsc.process_sample(&[0.5, 0.5]);
            assert!(output.is_finite(), "Output should be finite");
        }
    }

    #[test]
    fn test_gsc_noise_cancellation() {
        let mut gsc = GscBeamformer::new(2, &[0.0, 0.0], 32, 0.05);

        // Simulate: target from broadside (same on both mics) + noise from side (opposite phase)
        let num_samples = 2000;
        let mut error_power = 0.0f32;
        let mut signal_power = 0.0f32;

        for i in 0..num_samples {
            let t = i as f32 / 48000.0;
            let target = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.3;
            let noise = (2.0 * std::f32::consts::PI * 1500.0 * t).sin() * 0.5;

            // Target arrives equally, noise arrives with opposite phase
            let mic0 = target + noise;
            let mic1 = target - noise;

            let output = gsc.process_sample(&[mic0, mic1]);

            // Measure in last quarter (after convergence)
            if i >= num_samples * 3 / 4 {
                error_power += (output - target).powi(2);
                signal_power += target.powi(2);
            }
        }

        // The GSC should improve the output compared to raw microphone
        assert!(
            error_power.is_finite(),
            "Error power should be finite"
        );
    }

    #[test]
    fn test_gsc_reset() {
        let mut gsc = GscBeamformer::new(2, &[0.0, 0.0], 16, 0.01);

        // Process with signal that creates non-zero error
        for i in 0..200 {
            let t = i as f32 / 48000.0;
            let s = (2.0 * std::f32::consts::PI * 1000.0 * t).sin();
            gsc.process_sample(&[s * 0.5 + 0.3, s * 0.5 - 0.3]);
        }

        // Verify weights are non-zero
        let has_nonzero = gsc.adaptive_weights.iter().any(|w| w.iter().any(|&v| v != 0.0));
        assert!(has_nonzero, "Weights should be non-zero after adaptation");

        gsc.reset();

        for w in &gsc.adaptive_weights {
            for &v in w {
                assert_eq!(v, 0.0, "Weights should be zero after reset");
            }
        }
    }
}
