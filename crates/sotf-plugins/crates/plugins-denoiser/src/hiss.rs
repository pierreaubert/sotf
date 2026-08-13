/// Focused high-frequency stationary noise reducer.
///
/// This is intentionally simpler than the full STFT denoiser: it splits each
/// channel into a low-passed body and a high-frequency residual, tracks the
/// residual power at fast and slow time scales, and attenuates only persistent,
/// low-level residual energy. It is a zero-latency high-band downward expander,
/// not a spectral noise estimator.
pub struct HissReducer {
    channels: usize,
    sample_rate: u32,
    cutoff_hz: f32,
    threshold_db: f32,
    strength: f32,
    lowpass_state: Vec<f32>,
    fast_env: Vec<f32>,
    noise_env: Vec<f32>,
    alpha: f32,
    target_alpha: f32,
    alpha_smoothing_coeff: f32,
    threshold_power: f32,
    fast_coeff: f32,
    slow_coeff: f32,
    gain_attack_coeff: f32,
    gain_release_coeff: f32,
    gain: Vec<f32>,
    reducing: Vec<bool>,
    candidate_samples: Vec<u32>,
    hold_remaining: Vec<u32>,
    persistence_samples: u32,
    hold_samples: u32,
    wet_mix: f32,
    target_wet_mix: f32,
    wet_mix_coeff: f32,
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
            fast_env: vec![0.0; channels],
            noise_env: vec![0.0; channels],
            alpha: 0.0,
            target_alpha: 0.0,
            alpha_smoothing_coeff: 0.0,
            threshold_power: 0.0,
            fast_coeff: 0.0,
            slow_coeff: 0.0,
            gain_attack_coeff: 0.0,
            gain_release_coeff: 0.0,
            gain: vec![1.0; channels],
            reducing: vec![false; channels],
            candidate_samples: vec![0; channels],
            hold_remaining: vec![0; channels],
            persistence_samples: 0,
            hold_samples: 0,
            wet_mix: 1.0,
            target_wet_mix: 1.0,
            wet_mix_coeff: 0.0,
        };
        reducer.update_coefficients(true);
        reducer
    }

    pub fn initialize(&mut self, sample_rate: u32) -> Result<(), String> {
        if sample_rate == 0 {
            return Err("sample rate must be nonzero".to_string());
        }
        self.sample_rate = sample_rate;
        self.update_coefficients(true);
        Ok(())
    }

    pub fn set_params(&mut self, cutoff_hz: f32, threshold_db: f32, strength: f32) {
        self.cutoff_hz = if cutoff_hz.is_finite() {
            cutoff_hz.max(20.0)
        } else {
            4_000.0
        };
        self.threshold_db = if threshold_db.is_finite() {
            threshold_db
        } else {
            -30.0
        };
        self.strength = if strength.is_finite() {
            strength.clamp(0.0, 1.0)
        } else {
            0.5
        };
        self.update_coefficients(false);
    }

    /// Set the live bypass target. `immediate` is reserved for initialization
    /// and reset; automation uses the smoothed transition.
    pub fn set_enabled(&mut self, enabled: bool, immediate: bool) {
        self.target_wet_mix = if enabled { 1.0 } else { 0.0 };
        if immediate {
            self.wet_mix = self.target_wet_mix;
        }
    }

    pub fn reset(&mut self) {
        self.lowpass_state.fill(0.0);
        self.fast_env.fill(0.0);
        self.noise_env.fill(0.0);
        self.gain.fill(1.0);
        self.reducing.fill(false);
        self.candidate_samples.fill(0);
        self.hold_remaining.fill(0);
        self.wet_mix = self.target_wet_mix;
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
            self.alpha =
                self.target_alpha + self.alpha_smoothing_coeff * (self.alpha - self.target_alpha);
            if (self.alpha - self.target_alpha).abs() < 1e-8 {
                self.alpha = self.target_alpha;
            }
            self.wet_mix =
                self.target_wet_mix + self.wet_mix_coeff * (self.wet_mix - self.target_wet_mix);
            if (self.wet_mix - self.target_wet_mix).abs() < 1e-8 {
                self.wet_mix = self.target_wet_mix;
            }
            for (ch, sample) in frame.iter_mut().enumerate() {
                let dry = if sample.is_finite() { *sample } else { 0.0 };
                let low = self.alpha * dry + (1.0 - self.alpha) * self.lowpass_state[ch];
                self.lowpass_state[ch] = low;

                let high = dry - low;
                let high_power = high * high;
                self.fast_env[ch] =
                    self.fast_coeff * self.fast_env[ch] + (1.0 - self.fast_coeff) * high_power;
                self.noise_env[ch] =
                    self.slow_coeff * self.noise_env[ch] + (1.0 - self.slow_coeff) * high_power;
                if self.lowpass_state[ch].abs() < 1e-20 {
                    self.lowpass_state[ch] = 0.0;
                }
                if self.fast_env[ch].abs() < 1e-20 {
                    self.fast_env[ch] = 0.0;
                }
                if self.noise_env[ch].abs() < 1e-20 {
                    self.noise_env[ch] = 0.0;
                }

                // Fast/slow power ratio rejects attacks and zero-crossing
                // modulation. Hysteresis plus a short hold prevents chatter.
                let noise_power = self.noise_env[ch];
                let power_ratio = self.fast_env[ch] / noise_power.max(1e-20);
                let persistent = (0.5..=2.0).contains(&power_ratio);
                let enter_level = noise_power < self.threshold_power;
                let exit_level = noise_power < self.threshold_power * 2.0;
                if self.reducing[ch] {
                    if exit_level && power_ratio <= 4.0 {
                        self.hold_remaining[ch] = self.hold_samples;
                    } else if self.hold_remaining[ch] > 0 {
                        self.hold_remaining[ch] -= 1;
                    } else {
                        self.reducing[ch] = false;
                    }
                } else if noise_power > 1e-12 && enter_level && persistent {
                    self.candidate_samples[ch] = self.candidate_samples[ch].saturating_add(1);
                    if self.candidate_samples[ch] >= self.persistence_samples {
                        self.reducing[ch] = true;
                        self.hold_remaining[ch] = self.hold_samples;
                        self.candidate_samples[ch] = 0;
                    }
                } else {
                    self.candidate_samples[ch] = 0;
                }

                let level_depth =
                    (1.0 - noise_power / self.threshold_power.max(1e-20)).clamp(0.0, 1.0);
                let steady_depth = (1.0 - (power_ratio - 1.0).abs()).clamp(0.0, 1.0);
                let reduction_depth = if self.reducing[ch] {
                    level_depth * steady_depth
                } else {
                    0.0
                };
                let target_gain = 1.0 - self.strength * reduction_depth;
                let coeff = if target_gain < self.gain[ch] {
                    self.gain_attack_coeff
                } else {
                    self.gain_release_coeff
                };
                self.gain[ch] = target_gain + coeff * (self.gain[ch] - target_gain);
                if (self.gain[ch] - target_gain).abs() < 1e-8 {
                    self.gain[ch] = target_gain;
                }
                let processed = if self.gain[ch] == 1.0 {
                    dry
                } else {
                    low + high * self.gain[ch]
                };
                *sample = dry + self.wet_mix * (processed - dry);
            }
        }
    }

    fn update_coefficients(&mut self, snap_cutoff: bool) {
        let sr = self.sample_rate.max(1) as f32;
        let cutoff = self.cutoff_hz.min(sr * 0.45).max(20.0);
        self.target_alpha = 1.0 - (-2.0 * std::f32::consts::PI * cutoff / sr).exp();
        if snap_cutoff {
            self.alpha = self.target_alpha;
        }
        self.alpha_smoothing_coeff = (-1.0 / (0.005 * sr)).exp();
        self.threshold_power = 10.0_f32.powf(self.threshold_db / 10.0);
        self.fast_coeff = (-1.0 / (0.005 * sr)).exp();
        self.slow_coeff = (-1.0 / (0.100 * sr)).exp();
        self.gain_attack_coeff = (-1.0 / (0.001 * sr)).exp();
        self.gain_release_coeff = (-1.0 / (0.050 * sr)).exp();
        self.persistence_samples = (0.030 * sr).round() as u32;
        self.hold_samples = (0.020 * sr).round() as u32;
        self.wet_mix_coeff = (-1.0 / (0.005 * sr)).exp();
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
        reducer.initialize(96000).unwrap();
        assert_eq!(reducer.sample_rate, 96000);
        // Higher sample rate -> smaller alpha (slower filter)
        assert!(reducer.alpha < alpha_before);
    }

    #[test]
    fn initialize_rejects_zero_sample_rate() {
        let mut reducer = HissReducer::new(1);
        assert!(reducer.initialize(0).is_err());
        assert_eq!(reducer.sample_rate, 48000);
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
        let mut buffer: Vec<f32> = (0..24_000)
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

    #[test]
    fn isolated_transients_do_not_open_the_steady_noise_reducer() {
        let mut reducer = HissReducer::new(1);
        reducer.set_params(4_000.0, -20.0, 1.0);
        let mut signal = vec![0.0; 48_000 / 2];
        for sample in signal.iter_mut().step_by(2_400) {
            *sample = 0.08;
        }
        reducer.process(&mut signal);
        assert!(
            reducer.gain[0] > 0.98,
            "sparse impulses are transient programme content, gain={}",
            reducer.gain[0]
        );
    }

    #[test]
    fn cutoff_automation_ramps_the_filter_coefficient() {
        let mut reducer = HissReducer::new(1);
        let before = reducer.alpha;
        reducer.set_params(12_000.0, -30.0, 0.5);
        assert_eq!(
            reducer.alpha, before,
            "live cutoff updates must not jump the active IIR coefficient"
        );
        let mut sample = [0.1];
        reducer.process(&mut sample);
        assert_ne!(reducer.alpha, before);
    }

    #[test]
    fn long_tail_states_snap_out_of_the_denormal_range() {
        let mut reducer = HissReducer::new(1);
        reducer.lowpass_state[0] = 1.0e-30;
        reducer.fast_env[0] = 1.0e-30;
        reducer.noise_env[0] = 1.0e-30;
        let mut silence = [0.0];
        reducer.process(&mut silence);
        assert_eq!(reducer.lowpass_state[0], 0.0);
        assert_eq!(reducer.fast_env[0], 0.0);
        assert_eq!(reducer.noise_env[0], 0.0);
    }

    #[test]
    fn detector_timing_is_equivalent_across_sample_rates() {
        fn ending_gain(sample_rate: u32) -> f32 {
            let mut reducer = HissReducer::new(1);
            reducer.initialize(sample_rate).unwrap();
            reducer.set_params(sample_rate as f32 / 12.0, -20.0, 0.8);
            let mut signal: Vec<f32> = (0..sample_rate / 2)
                .map(|index| if index % 2 == 0 { 0.03 } else { -0.03 })
                .collect();
            reducer.process(&mut signal);
            reducer.gain[0]
        }

        let reference = ending_gain(48_000);
        for sample_rate in [22_050, 44_100, 96_000, 192_000] {
            let gain = ending_gain(sample_rate);
            assert!(
                (gain - reference).abs() < 0.015,
                "sample_rate={sample_rate}, gain={gain}, reference={reference}"
            );
        }
    }

    #[test]
    fn zero_strength_reconstructs_input_exactly() {
        let mut reducer = HissReducer::new(2);
        reducer.set_params(4_000.0, -10.0, 0.0);
        let mut signal: Vec<f32> = (0..20_000)
            .map(|index| ((index as f32 * 0.731).sin() * 0.2).clamp(-1.0, 1.0))
            .collect();
        let expected = signal.clone();
        reducer.process(&mut signal);
        assert_eq!(signal, expected);
    }

    #[test]
    fn live_bypass_ramps_and_eventually_reaches_exact_dry() {
        let mut reducer = HissReducer::new(1);
        reducer.set_enabled(false, false);
        let mut first = [0.05];
        reducer.process(&mut first);
        assert!(reducer.wet_mix > 0.0 && reducer.wet_mix < 1.0);

        let mut settle = vec![0.05; 48_000];
        reducer.process(&mut settle);
        assert_eq!(reducer.wet_mix, 0.0);
        assert_eq!(*settle.last().unwrap(), 0.05);
    }

    #[test]
    fn output_is_independent_of_callback_partitioning() {
        let make_signal = || {
            (0..12_000)
                .map(|index| {
                    let carrier = (index as f32 * 0.41).sin() * 0.04;
                    if index % 997 == 0 { 0.08 } else { carrier }
                })
                .collect::<Vec<_>>()
        };
        let mut whole = HissReducer::new(1);
        let mut partitioned = HissReducer::new(1);
        whole.set_params(4_000.0, -20.0, 0.8);
        partitioned.set_params(4_000.0, -20.0, 0.8);
        let mut whole_signal = make_signal();
        let mut partitioned_signal = whole_signal.clone();
        whole.process(&mut whole_signal[..6_000]);
        whole.set_enabled(false, false);
        whole.process(&mut whole_signal[6_000..]);

        let partitions = [1, 17, 64, 3, 511, 29, 128];
        let mut offset = 0;
        let mut partition_index = 0;
        while offset < partitioned_signal.len() {
            if offset == 6_000 {
                partitioned.set_enabled(false, false);
            }
            let next_event = if offset < 6_000 {
                6_000
            } else {
                partitioned_signal.len()
            };
            let end = (offset + partitions[partition_index % partitions.len()])
                .min(next_event)
                .min(partitioned_signal.len());
            partitioned.process(&mut partitioned_signal[offset..end]);
            offset = end;
            partition_index += 1;
        }
        assert_eq!(partitioned_signal, whole_signal);
    }
}
