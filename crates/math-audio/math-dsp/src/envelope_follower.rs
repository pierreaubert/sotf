// ============================================================================
// Envelope Follower — Attack/Release envelope for modulation signals
// ============================================================================
//
// Produces a smooth envelope from an audio signal's amplitude. Used for:
// - Dynamic saturation (drive modulated by input level)
// - Adaptive thresholds in spectral processors
// - Any DSP where a control signal tracks audio energy
//
// Uses branching attack/release: attack coefficient when input > envelope,
// release coefficient when input < envelope.
//
// HARD RULES:
// - No allocations in process()
// - All state is f32 for cache efficiency

/// Attack-release envelope follower producing a smooth modulation signal.
#[derive(Debug, Clone, Copy)]
pub struct EnvelopeFollower {
    envelope: f32,
    attack_coeff: f32,
    release_coeff: f32,
}

impl EnvelopeFollower {
    /// Create a new envelope follower.
    ///
    /// `attack_ms`: Time to reach ~63% of a step increase.
    /// `release_ms`: Time to decay to ~37% of a step decrease.
    pub fn new(attack_ms: f32, release_ms: f32, sample_rate: u32) -> Self {
        Self {
            envelope: 0.0,
            attack_coeff: Self::ms_to_coeff(attack_ms, sample_rate),
            release_coeff: Self::ms_to_coeff(release_ms, sample_rate),
        }
    }

    fn ms_to_coeff(time_ms: f32, sample_rate: u32) -> f32 {
        if time_ms <= 0.0 {
            return 0.0;
        }
        (-1.0 / (time_ms * 0.001 * sample_rate as f32)).exp()
    }

    /// Process one sample (provide absolute value of input).
    ///
    /// Returns the current envelope value. NaN/inf inputs are treated as zero
    /// to prevent permanent envelope corruption.
    #[inline]
    pub fn process(&mut self, input_abs: f32) -> f32 {
        let input_abs = if input_abs.is_finite() {
            input_abs
        } else {
            0.0
        };
        let coeff = if input_abs > self.envelope {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.envelope = input_abs + coeff * (self.envelope - input_abs);
        self.envelope
    }

    /// Process a block and return the final envelope value.
    ///
    /// Useful when you only need the envelope at block boundaries.
    #[inline]
    pub fn process_block(&mut self, samples: &[f32]) -> f32 {
        for &sample in samples {
            self.process(sample.abs());
        }
        self.envelope
    }

    /// Update attack/release times.
    pub fn set_times(&mut self, attack_ms: f32, release_ms: f32, sample_rate: u32) {
        self.attack_coeff = Self::ms_to_coeff(attack_ms, sample_rate);
        self.release_coeff = Self::ms_to_coeff(release_ms, sample_rate);
    }

    /// Reset envelope to zero.
    pub fn reset(&mut self) {
        self.envelope = 0.0;
    }

    /// Get current envelope value without processing.
    #[inline]
    pub fn current(&self) -> f32 {
        self.envelope
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attack_from_silence() {
        let mut env = EnvelopeFollower::new(10.0, 100.0, 48000);
        // Feed constant 1.0 — envelope should rise
        let mut prev = 0.0;
        for _ in 0..480 {
            // 10ms
            let val = env.process(1.0);
            assert!(val >= prev);
            prev = val;
        }
        // After 10ms (one time constant), should be ~63% of target
        assert!(prev > 0.5, "Envelope too slow: {prev}");
        assert!(prev < 0.8, "Envelope too fast: {prev}");
    }

    #[test]
    fn test_release_from_peak() {
        let mut env = EnvelopeFollower::new(0.1, 50.0, 48000);
        // Instant attack
        for _ in 0..48 {
            env.process(1.0);
        }
        let peak = env.current();
        assert!(peak > 0.9);

        // Now release — feed silence
        for _ in 0..2400 {
            // 50ms
            env.process(0.0);
        }
        let after_release = env.current();
        assert!(after_release < 0.4, "Release too slow: {after_release}");
    }

    #[test]
    fn test_process_block() {
        let mut env = EnvelopeFollower::new(1.0, 100.0, 48000);
        let block: Vec<f32> = (0..480).map(|_| 0.5).collect();
        let result = env.process_block(&block);
        assert!(result > 0.3);
    }

    #[test]
    fn test_reset() {
        let mut env = EnvelopeFollower::new(1.0, 100.0, 48000);
        env.process(1.0);
        assert!(env.current() > 0.0);
        env.reset();
        assert_eq!(env.current(), 0.0);
    }

    #[test]
    fn test_zero_attack_time() {
        let mut env = EnvelopeFollower::new(0.0, 100.0, 48000);
        // Zero attack = instant tracking
        let val = env.process(0.75);
        assert!((val - 0.75).abs() < 1e-6);
    }
}
