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
    /// Pre-allocated scratch for reference signals (avoids per-sample allocation)
    reference_scratch: Vec<f32>,
    /// Delay-aligned microphone samples shared by the fixed and blocking paths.
    aligned_samples: Vec<f32>,
    /// Per-mic delay lines for fractional delay compensation
    delay_lines: Vec<Vec<f32>>,
    delay_write_pos: usize,
    max_delay_samples: usize,
    steering_delays: Vec<f32>,
}

impl GscBeamformer {
    /// Create a new GSC beamformer.
    ///
    /// # Arguments
    /// * `num_mics` - Number of microphones
    /// * `steering_delays` - Time delay per mic in samples (from look direction)
    /// * `filter_length` - NLMS filter length (default: 64)
    /// * `mu` - NLMS step size (default: 0.01)
    pub fn new(num_mics: usize, steering_delays: &[f32], filter_length: usize, mu: f32) -> Self {
        assert!(num_mics >= 2, "GSC requires at least 2 microphones");

        // Fixed beamformer: uniform real-valued weights for delay-and-sum.
        let fbf_weights = vec![1.0f32 / num_mics as f32; num_mics];

        // Blocking matrix: orthogonal complement of steering vector
        // B = I - d*d^H / (d^H * d)
        let num_refs = num_mics - 1;
        let d_norm_sq = fbf_weights.iter().map(|w| w * w).sum::<f32>();
        let mut blocking_matrix = Vec::with_capacity(num_refs);
        for r in 0..num_mics {
            let mut row = vec![0.0f32; num_mics];
            for c in 0..num_mics {
                row[c] = if r == c { 1.0 } else { 0.0 };
                row[c] -= fbf_weights[r] * fbf_weights[c] / d_norm_sq;
            }
            let norm_sq = row.iter().map(|x| x * x).sum::<f32>();
            if norm_sq > 1e-10 && blocking_matrix.len() < num_refs {
                blocking_matrix.push(row);
            }
        }
        // Fallback: should never happen for num_mics >= 2, but keep safe
        while blocking_matrix.len() < num_refs {
            let mut row = vec![0.0f32; num_mics];
            row[blocking_matrix.len()] = 1.0;
            row[blocking_matrix.len() + 1] = -1.0;
            blocking_matrix.push(row);
        }

        // Delay lines
        let max_delay = steering_delays
            .iter()
            .map(|d| d.ceil() as usize + 1)
            .max()
            .unwrap_or(1)
            .max(1);
        let delay_lines = vec![vec![0.0f32; max_delay]; num_mics];

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
            reference_scratch: vec![0.0; num_refs],
            aligned_samples: vec![0.0; num_mics],
            delay_lines,
            delay_write_pos: 0,
            max_delay_samples: max_delay,
            steering_delays: steering_delays.to_vec(),
        }
    }

    /// Process one sample from all microphones.
    ///
    /// # Arguments
    /// * `mic_samples` - One sample per microphone
    ///
    /// # Returns
    /// Single beamformed output sample
    #[allow(clippy::needless_range_loop)]
    pub fn process_sample(&mut self, mic_samples: &[f32]) -> f32 {
        let m = self.num_mics.min(mic_samples.len());
        let num_refs = self.blocking_matrix.len();

        // Write current samples to delay lines
        for i in 0..m {
            self.delay_lines[i][self.delay_write_pos] = mic_samples[i];
        }

        // 1. Fixed beamformer output: delay-compensated sum
        let mut fbf_output = 0.0f32;
        for i in 0..m {
            let delay = self.steering_delays[i];
            let int_delay = delay.floor() as usize;
            let frac = delay - delay.floor();
            let buf_len = self.delay_lines[i].len();
            let idx0 = (self.delay_write_pos + buf_len - int_delay) % buf_len;
            // Fractional delay lies between the integer-delayed sample and
            // the next older sample in ring time.
            let idx1 = (idx0 + buf_len - 1) % buf_len;
            let delayed_sample =
                self.delay_lines[i][idx0] * (1.0 - frac) + self.delay_lines[i][idx1] * frac;
            self.aligned_samples[i] = delayed_sample;
            fbf_output += delayed_sample * self.fbf_weights[i];
        }

        // Advance delay write position
        self.delay_write_pos = (self.delay_write_pos + 1) % self.max_delay_samples;

        // 2. Blocking matrix: compute reference signals
        // u = B * x (num_refs reference signals)
        self.reference_scratch[..num_refs].fill(0.0);
        for (r, row) in self.blocking_matrix.iter().enumerate() {
            for i in 0..m {
                self.reference_scratch[r] += row[i] * self.aligned_samples[i];
            }
        }
        let references = &self.reference_scratch[..num_refs];

        // Store references in delay lines
        for (buf, &ref_val) in self.reference_buffers[..num_refs]
            .iter_mut()
            .zip(references)
        {
            buf[self.ref_write_pos] = ref_val;
        }

        // 3. NLMS adaptive noise canceller
        // Compute noise estimate: y = Σ_r w_r^T * u_r
        let mut noise_estimate = 0.0f32;
        let mut total_ref_power = self.delta;

        for r in 0..num_refs {
            let mut buf_idx = self.ref_write_pos;
            for j in 0..self.filter_length {
                noise_estimate += self.adaptive_weights[r][j] * self.reference_buffers[r][buf_idx];
                total_ref_power += self.reference_buffers[r][buf_idx].powi(2);
                buf_idx = if buf_idx == 0 {
                    self.filter_length - 1
                } else {
                    buf_idx - 1
                };
            }
        }

        // Error signal (beamformed output minus noise estimate)
        let error = fbf_output - noise_estimate;

        // NLMS weight update: w += μ * e * u / (||u||² + δ)
        let instantaneous_reference_power: f32 = references.iter().map(|value| value * value).sum();
        let target_dominant = instantaneous_reference_power < fbf_output * fbf_output * 0.01;
        if !target_dominant {
            let step = self.mu * error / total_ref_power;
            for r in 0..num_refs {
                let mut buf_idx = self.ref_write_pos;
                for j in 0..self.filter_length {
                    self.adaptive_weights[r][j] += step * self.reference_buffers[r][buf_idx];
                    buf_idx = if buf_idx == 0 {
                        self.filter_length - 1
                    } else {
                        buf_idx - 1
                    };
                }
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
        for d in &mut self.delay_lines {
            d.fill(0.0);
        }
        self.ref_write_pos = 0;
        self.delay_write_pos = 0;
    }

    /// Common steering-compensation latency in whole samples.
    pub fn latency_samples(&self) -> usize {
        self.steering_delays
            .iter()
            .copied()
            .fold(0.0_f32, f32::max)
            .ceil() as usize
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
        let num_samples = 20_000;
        let mut error_power = 0.0f32;
        let mut raw_error_power = 0.0f32;

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
                raw_error_power += noise.powi(2);
            }
        }

        // The GSC should improve the output compared to raw microphone
        assert!(
            error_power < raw_error_power * 0.2,
            "GSC must improve target-referenced error by at least 7 dB: output={error_power}, raw={raw_error_power}"
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
        let has_nonzero = gsc
            .adaptive_weights
            .iter()
            .any(|w| w.iter().any(|&v| v != 0.0));
        assert!(has_nonzero, "Weights should be non-zero after adaptation");

        gsc.reset();

        for w in &gsc.adaptive_weights {
            for &v in w {
                assert_eq!(v, 0.0, "Weights should be zero after reset");
            }
        }
    }

    #[test]
    fn test_gsc_blocking_matrix_orthogonal() {
        let gsc = GscBeamformer::new(3, &[0.0, 0.001, 0.002], 16, 0.01);

        // The FBF weights form the steering vector for aligned signals
        let d = &gsc.fbf_weights;

        for (r, row) in gsc.blocking_matrix.iter().enumerate() {
            let dot: f32 = row.iter().zip(d).map(|(a, b)| a * b).sum();
            assert!(
                dot.abs() < 1e-5,
                "Blocking matrix row {r} not orthogonal to steering vector: dot={dot}"
            );
        }

        // Should have exactly num_mics - 1 rows
        assert_eq!(gsc.blocking_matrix.len(), 2);
    }

    #[test]
    fn test_gsc_delay_compensation() {
        // 2 mics, mic0 receives 1 sample before mic1.
        // Compensation delays [1.0, 0.0] delay mic0 by 1 to align with mic1.
        // mu = 0.0 disables adaptation so output = fbf_output.
        let mut gsc = GscBeamformer::new(2, &[1.0, 0.0], 16, 0.0);

        // Feed a ramp where mic1 is already delayed by 1 sample relative to mic0.
        for t in 10..100 {
            let mic0 = t as f32;
            let mic1 = (t - 1) as f32;
            let output = gsc.process_sample(&[mic0, mic1]);

            // After delay compensation, both mics see (t-1); fbf averages to (t-1).
            if t >= 12 {
                let expected = (t - 1) as f32;
                assert!(
                    (output - expected).abs() < 0.01,
                    "GSC delay compensation failed at t={t}: expected {expected}, got {output}"
                );
            }
        }
    }

    #[test]
    fn fractional_delay_uses_next_older_sample() {
        let mut gsc = GscBeamformer::new(2, &[1.5, 0.0], 16, 0.0);
        for t in 0..100 {
            let mic0 = t as f32;
            let mic1 = t as f32 - 1.5;
            let output = gsc.process_sample(&[mic0, mic1]);
            if t >= 4 {
                assert!((output - mic1).abs() < 1.0e-5, "t={t}: {output} != {mic1}");
                assert!(
                    gsc.reference_scratch
                        .iter()
                        .all(|value| value.abs() < 1.0e-5),
                    "aligned look source leaked into blocking references: {:?}",
                    gsc.reference_scratch
                );
            }
        }
        assert_eq!(gsc.latency_samples(), 2);
    }

    #[test]
    fn target_plane_waves_are_distortionless_across_angles() {
        use crate::steering::{ArrayGeometry, compute_steering_delays};

        let geometry = ArrayGeometry::Linear {
            num_mics: 4,
            spacing_m: 0.05,
        };
        let sample_rate = 48_000.0;
        let frequency = 700.0;
        for angle in [0.0, 30.0, 60.0, 90.0] {
            let compensation = compute_steering_delays(&geometry, angle, 0.0, sample_rate);
            let common_delay = compensation.iter().copied().fold(0.0_f32, f32::max);
            let propagation: Vec<f32> = compensation
                .iter()
                .map(|delay| common_delay - delay)
                .collect();
            let mut gsc = GscBeamformer::new(4, &compensation, 32, 0.03);
            let mut signal_energy = 0.0_f32;
            let mut error_energy = 0.0_f32;
            let mut reference_energy = 0.0_f32;
            for frame in 0..20_000 {
                let samples: Vec<f32> = propagation
                    .iter()
                    .map(|delay| {
                        (std::f32::consts::TAU * frequency * (frame as f32 - delay) / sample_rate)
                            .sin()
                    })
                    .collect();
                let output = gsc.process_sample(&samples);
                if frame > 2_000 {
                    let expected =
                        (std::f32::consts::TAU * frequency * (frame as f32 - common_delay)
                            / sample_rate)
                            .sin();
                    signal_energy += expected * expected;
                    error_energy += (output - expected).powi(2);
                    reference_energy += gsc.reference_scratch.iter().map(|x| x * x).sum::<f32>();
                }
            }
            assert!(
                error_energy / signal_energy < 1e-3,
                "angle={angle}, normalized error={}",
                error_energy / signal_energy
            );
            assert!(
                reference_energy / signal_energy < 1e-4,
                "angle={angle}, target leakage={}",
                reference_energy / signal_energy
            );
        }
    }
}
