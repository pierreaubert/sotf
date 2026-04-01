// ============================================================================
// DC Blocker — 1-pole high-pass filter for removing DC offset
// ============================================================================
//
// A simple first-order HPF that removes DC bias introduced by asymmetric
// nonlinear processing (e.g., tube saturation, tape hysteresis).
//
// Transfer function: H(z) = (1 - z^-1) / (1 - R*z^-1)
// where R = 1 - 2*pi*cutoff/sample_rate
//
// HARD RULES:
// - No allocations in process methods
// - All Vecs pre-allocated in new()

/// Per-channel DC blocker using a 1-pole high-pass filter.
///
/// Default cutoff is 5 Hz — low enough to preserve all audible content
/// while effectively removing DC offset from nonlinear processors.
#[derive(Debug, Clone)]
pub struct DcBlocker {
    x_prev: Vec<f32>,
    y_prev: Vec<f32>,
    coeff: f32,
    channels: usize,
}

impl DcBlocker {
    /// Create a new DC blocker.
    ///
    /// `cutoff_hz`: Corner frequency (typically 3-10 Hz).
    pub fn new(channels: usize, sample_rate: u32, cutoff_hz: f32) -> Self {
        Self {
            x_prev: vec![0.0; channels],
            y_prev: vec![0.0; channels],
            coeff: Self::calculate_coeff(cutoff_hz, sample_rate),
            channels,
        }
    }

    /// Create with default 5 Hz cutoff.
    pub fn new_default(channels: usize, sample_rate: u32) -> Self {
        Self::new(channels, sample_rate, 5.0)
    }

    fn calculate_coeff(cutoff_hz: f32, sample_rate: u32) -> f32 {
        if sample_rate == 0 {
            return 0.99999; // Maximum R = lowest cutoff, safe default
        }
        // R = 1 - 2*pi*fc/fs
        // Higher R = lower cutoff = less bass attenuation
        let r = 1.0 - (2.0 * std::f32::consts::PI * cutoff_hz / sample_rate as f32);
        r.clamp(0.9, 0.99999)
    }

    /// Process a single sample for one channel.
    ///
    /// y[n] = x[n] - x[n-1] + R * y[n-1]
    #[inline]
    pub fn process_sample(&mut self, sample: f32, channel: usize) -> f32 {
        let x_prev = self.x_prev[channel];
        let y_prev = self.y_prev[channel];
        let output = sample - x_prev + self.coeff * y_prev;
        self.x_prev[channel] = sample;
        self.y_prev[channel] = output;
        output
    }

    /// Process a block of interleaved audio in-place.
    ///
    /// `buffer`: Interleaved samples `[ch0_f0, ch1_f0, ch0_f1, ch1_f1, ...]`
    /// `channels`: Number of interleaved channels.
    /// `num_frames`: Number of frames (samples per channel).
    pub fn process_block_interleaved(
        &mut self,
        buffer: &mut [f32],
        channels: usize,
        num_frames: usize,
    ) {
        debug_assert_eq!(channels, self.channels);
        for frame in 0..num_frames {
            for ch in 0..channels {
                let idx = frame * channels + ch;
                buffer[idx] = self.process_sample(buffer[idx], ch);
            }
        }
    }

    /// Reset all filter state to zero.
    pub fn reset(&mut self) {
        self.x_prev.fill(0.0);
        self.y_prev.fill(0.0);
    }

    /// Update sample rate (recalculates coefficient).
    pub fn set_sample_rate(&mut self, sample_rate: u32, cutoff_hz: f32) {
        self.coeff = Self::calculate_coeff(cutoff_hz, sample_rate);
    }

    /// Update channel count (re-allocates state vectors).
    pub fn set_channels(&mut self, channels: usize) {
        self.channels = channels;
        self.x_prev.resize(channels, 0.0);
        self.y_prev.resize(channels, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dc_removal() {
        let mut blocker = DcBlocker::new(1, 48000, 5.0);
        // Feed DC offset of 1.0 for 2 seconds
        let num_samples = 96000;
        let mut last_output = 0.0f32;
        for _ in 0..num_samples {
            last_output = blocker.process_sample(1.0, 0);
        }
        // After convergence, DC should be effectively removed
        assert!(
            last_output.abs() < 0.01,
            "DC not removed: output = {last_output}"
        );
    }

    #[test]
    fn test_passband_preservation() {
        let sr = 48000;
        let mut blocker = DcBlocker::new(1, sr, 5.0);
        // 1 kHz sine — well above cutoff, should pass through
        let freq = 1000.0;
        let num_samples = 4800; // 100ms
        let mut max_input = 0.0f32;
        let mut max_output = 0.0f32;
        for i in 0..num_samples {
            let t = i as f32 / sr as f32;
            let sample = (2.0 * std::f32::consts::PI * freq * t).sin();
            let out = blocker.process_sample(sample, 0);
            if i > 2400 {
                // skip transient
                max_input = max_input.max(sample.abs());
                max_output = max_output.max(out.abs());
            }
        }
        let ratio = max_output / max_input;
        assert!(ratio > 0.99, "1kHz attenuated too much: ratio = {ratio}");
    }

    #[test]
    fn test_block_processing() {
        let mut blocker = DcBlocker::new(2, 48000, 5.0);
        // Interleaved stereo: DC on ch0, zero on ch1
        let mut buffer = vec![1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0];
        blocker.process_block_interleaved(&mut buffer, 2, 4);
        // Ch0 should start moving toward 0; ch1 stays 0
        assert!(buffer[1].abs() < 1e-10); // ch1 stays zero
        assert!(buffer[0] > 0.0); // ch0 initially positive (transient)
    }

    #[test]
    fn test_reset() {
        let mut blocker = DcBlocker::new(1, 48000, 5.0);
        blocker.process_sample(1.0, 0);
        blocker.reset();
        // After reset, state should be clean
        let out = blocker.process_sample(0.0, 0);
        assert!(out.abs() < 1e-10);
    }

    #[test]
    fn test_sample_rate_zero_no_panic() {
        // sample_rate=0 should not panic or produce NaN
        let mut blocker = DcBlocker::new(1, 0, 5.0);
        let out = blocker.process_sample(1.0, 0);
        assert!(!out.is_nan(), "NaN from sample_rate=0");
        assert!(!out.is_infinite(), "inf from sample_rate=0");
    }
}
