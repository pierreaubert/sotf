/// Focused high-frequency stationary noise reducer.
///
/// This is intentionally simpler than the full STFT denoiser: it splits each
/// channel into a low-passed body and a high-frequency residual, tracks the
/// residual envelope slowly, and attenuates only the residual when it looks
/// stationary and low-level.
pub struct HissReducer {
    channels: usize,
    sample_rate: u32,
    cutoff_hz: f32,
    threshold_db: f32,
    strength: f32,
    lowpass_state: Vec<f32>,
    noise_env: Vec<f32>,
    alpha: f32,
    threshold_linear: f32,
}

impl HissReducer {
    pub fn new(channels: usize) -> Self {
        let mut reducer = Self {
            channels,
            sample_rate: 48000,
            cutoff_hz: 4000.0,
            threshold_db: -30.0,
            strength: 0.5,
            lowpass_state: vec![0.0; channels],
            noise_env: vec![0.0; channels],
            alpha: 0.0,
            threshold_linear: 0.0,
        };
        reducer.update_coefficients();
        reducer
    }

    pub fn initialize(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate.max(1);
        self.update_coefficients();
    }

    pub fn set_params(&mut self, cutoff_hz: f32, threshold_db: f32, strength: f32) {
        self.cutoff_hz = cutoff_hz.max(20.0);
        self.threshold_db = threshold_db;
        self.strength = strength.clamp(0.0, 1.0);
        self.update_coefficients();
    }

    pub fn reset(&mut self) {
        self.lowpass_state.fill(0.0);
        self.noise_env.fill(0.0);
    }

    /// Algorithmic latency in samples.
    ///
    /// HissReducer is a sample-by-sample first-order IIR lowpass with an
    /// envelope follower. It has no lookahead, FFT buffering, or block
    /// processing, so its latency is zero.
    pub fn latency_samples(&self) -> usize {
        0
    }

    pub fn process(&mut self, buffer: &mut [f32]) {
        if self.channels == 0 {
            return;
        }

        for frame in buffer.chunks_mut(self.channels) {
            for (ch, sample) in frame.iter_mut().enumerate() {
                let low = self.alpha * *sample + (1.0 - self.alpha) * self.lowpass_state[ch];
                self.lowpass_state[ch] = low;

                let high = *sample - low;
                let high_abs = high.abs();
                self.noise_env[ch] = self.noise_env[ch] * 0.999 + high_abs * 0.001;

                let stationary_ratio = self.noise_env[ch] / (high_abs + 1e-9);
                let below_threshold = self.noise_env[ch] < self.threshold_linear;
                let stationary = stationary_ratio > 0.25;

                let attenuation = if below_threshold && stationary {
                    1.0 - self.strength
                } else {
                    1.0
                };
                *sample = low + high * attenuation;
            }
        }
    }

    fn update_coefficients(&mut self) {
        let sr = self.sample_rate.max(1) as f32;
        let cutoff = self.cutoff_hz.min(sr * 0.45);
        let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff.max(20.0));
        let dt = 1.0 / sr;
        self.alpha = dt / (rc + dt);
        self.threshold_linear = 10.0_f32.powf(self.threshold_db / 20.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_reducer() {
        let reducer = HissReducer::new(2);
        assert_eq!(reducer.channels, 2);
        assert_eq!(reducer.sample_rate, 48000);
        assert_eq!(reducer.latency_samples(), 0);
    }

    #[test]
    fn latency_is_zero() {
        let reducer = HissReducer::new(1);
        assert_eq!(reducer.latency_samples(), 0);
    }

    #[test]
    fn process_with_zero_channels_returns_early() {
        let mut reducer = HissReducer::new(0);
        let mut buffer = vec![1.0f32; 10];
        reducer.process(&mut buffer);
        assert!(buffer.iter().all(|&s| s == 1.0));
    }

    #[test]
    fn silence_stays_zero() {
        let mut reducer = HissReducer::new(1);
        let mut buffer = vec![0.0f32; 100];
        reducer.process(&mut buffer);
        for &s in &buffer {
            assert_eq!(s, 0.0, "Silence should remain zero, got {}", s);
        }
    }

    #[test]
    fn reset_clears_state() {
        let mut reducer = HissReducer::new(1);
        let mut buf = vec![0.5f32; 20];
        reducer.process(&mut buf);
        reducer.reset();
        assert!(reducer.lowpass_state.iter().all(|&s| s == 0.0));
        assert!(reducer.noise_env.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn initialize_updates_sample_rate() {
        let mut reducer = HissReducer::new(1);
        let alpha_before = reducer.alpha;
        reducer.initialize(96000);
        assert_eq!(reducer.sample_rate, 96000);
        // Higher sample rate -> smaller alpha (slower filter)
        assert!(reducer.alpha < alpha_before);
    }

    #[test]
    fn initialize_clamps_zero_sample_rate() {
        let mut reducer = HissReducer::new(1);
        reducer.initialize(0);
        assert_eq!(reducer.sample_rate, 1);
    }

    #[test]
    fn set_params_clamps() {
        let mut reducer = HissReducer::new(1);
        reducer.set_params(10.0, -40.0, 1.5);
        // cutoff_hz clamped to 20.0 minimum
        assert_eq!(reducer.cutoff_hz, 20.0);
        // strength clamped to 1.0 maximum
        assert_eq!(reducer.strength, 1.0);
        assert_eq!(reducer.threshold_db, -40.0);
    }

    #[test]
    fn set_params_clamps_negative_strength() {
        let mut reducer = HissReducer::new(1);
        reducer.set_params(4000.0, -30.0, -0.5);
        assert_eq!(reducer.strength, 0.0);
    }

    #[test]
    fn process_attenuates_stationary_high_freq() {
        let mut reducer = HissReducer::new(1);
        // Threshold at -20 dB (linear ~0.1) so a 0.05 amplitude signal is below it.
        reducer.set_params(4000.0, -20.0, 1.0);

        // Feed a high-frequency alternating signal (rapid sign changes) at low amplitude.
        let mut buffer: Vec<f32> = (0..1000)
            .map(|i| if i % 2 == 0 { 0.05 } else { -0.05 })
            .collect();
        let energy_before: f32 = buffer.iter().map(|s| s * s).sum();
        reducer.process(&mut buffer);
        let energy_after: f32 = buffer.iter().map(|s| s * s).sum();

        // With strength=1.0, stationary high-freq content should be heavily attenuated.
        assert!(
            energy_after < energy_before * 0.5,
            "Expected energy reduction, before={energy_before} after={energy_after}"
        );
    }

    #[test]
    fn process_passes_through_loud_signals() {
        let mut reducer = HissReducer::new(1);
        // High threshold so nothing is considered noise.
        reducer.set_params(4000.0, 0.0, 1.0);

        let mut buffer: Vec<f32> = (0..100).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
        let expected = buffer.clone();
        reducer.process(&mut buffer);

        // With threshold at 0 dB (linear 1.0), the noise_env should stay well below
        // threshold for this small signal, but the stationary_ratio may also be low.
        // We mainly assert the output is finite and similar in energy.
        assert!(buffer.iter().all(|s| s.is_finite()));
        let energy_before: f32 = expected.iter().map(|s| s * s).sum();
        let energy_after: f32 = buffer.iter().map(|s| s * s).sum();
        assert!(
            (energy_after - energy_before).abs() < energy_before * 0.1 || energy_before < 1e-6,
            "Small signal should pass through nearly unchanged with high threshold"
        );
    }

    #[test]
    fn multichannel_process_independent_channels() {
        let mut reducer = HissReducer::new(2);
        // Left: silence, Right: small sine.
        let mut buffer = vec![0.0f32; 200];
        for frame in 0..100 {
            buffer[frame * 2 + 1] = (frame as f32 * 0.1).sin() * 0.05;
        }
        let original_right: Vec<f32> = buffer.iter().skip(1).step_by(2).copied().collect();

        reducer.process(&mut buffer);

        // Left channel should stay silent.
        for frame in 0..100 {
            assert_eq!(buffer[frame * 2], 0.0, "Left channel should remain silent");
        }

        // Right channel should still have energy.
        let right_energy: f32 = buffer.iter().skip(1).step_by(2).map(|s| s * s).sum();
        let original_energy: f32 = original_right.iter().map(|s| s * s).sum();
        assert!(right_energy > 0.0);
        assert!(right_energy <= original_energy * 1.01);
    }
}
