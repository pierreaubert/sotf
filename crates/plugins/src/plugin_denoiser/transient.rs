// ============================================================================
// Time-Domain Transient Suppressor (De-clicker)
// ============================================================================
//
// This module implements an adaptive slew-rate limiter to remove
// impulsive noise (clicks/pops) before spectral processing.
//
// Algorithm:
// 1. Tracks the local derivative envelope (rate of change) of the signal.
// 2. Detects sudden slope changes that exceed the local envelope by a significant factor.
// 3. Limits the slew rate of these spikes to match the local average.

pub struct TransientSuppressor {
    channels: usize,
    // Per-channel state
    last_samples: Vec<f32>,
    slope_envelope: Vec<f32>,

    // Parameters
    sensitivity: f32, // Threshold multiplier (higher = less sensitive)
    decay: f32,       // Envelope decay factor
}

impl TransientSuppressor {
    pub fn new(channels: usize) -> Self {
        Self {
            channels,
            last_samples: vec![0.0; channels],
            slope_envelope: vec![0.0; channels],
            sensitivity: 10.0, // Default sensitivity
            decay: 0.99,       // Fast tracking for transients
        }
    }

    pub fn reset(&mut self) {
        self.last_samples.fill(0.0);
        self.slope_envelope.fill(0.0);
    }

    pub fn set_sensitivity(&mut self, sensitivity: f32) {
        self.sensitivity = sensitivity.max(1.0);
    }

    /// Process interleaved audio buffer in-place
    pub fn process(&mut self, buffer: &mut [f32]) {
        for frame in buffer.chunks_mut(self.channels) {
            for (ch, sample) in frame.iter_mut().enumerate() {
                // Calculate derivative (delta)
                let last = self.last_samples[ch];
                let delta = *sample - last;
                let abs_delta = delta.abs();

                // Initialize envelope
                if self.slope_envelope[ch] == 0.0 {
                    self.slope_envelope[ch] = abs_delta + 1e-6;
                }

                let current_envelope = self.slope_envelope[ch];

                // Detect click: sudden large slope change
                // We add a small floor to envelope to avoid triggering on silence
                let threshold = current_envelope * self.sensitivity + 1e-5;

                let processed_sample;

                if abs_delta > threshold {
                    // It's a pop/click. Limit the slew rate.
                    // Instead of clamping the value, we clamp the change.
                    let sign = if delta >= 0.0 { 1.0 } else { -1.0 };
                    let limited_delta = sign * threshold;
                    processed_sample = last + limited_delta;
                    *sample = processed_sample;

                    // Do not update envelope with the spike, use the threshold
                    // to keep the average stable during the click event.
                } else {
                    processed_sample = *sample;
                    // Update envelope (Adaptive tracking)
                    // Fast attack, slower decay to envelope the derivative
                    if abs_delta > self.slope_envelope[ch] {
                        self.slope_envelope[ch] = abs_delta;
                    } else {
                        self.slope_envelope[ch] =
                            self.slope_envelope[ch] * self.decay + abs_delta * (1.0 - self.decay);
                    }
                }

                self.last_samples[ch] = processed_sample;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transient_suppression_slew_rate() {
        let mut suppressor = TransientSuppressor::new(1);
        suppressor.set_sensitivity(5.0); // Make it sensitive for test

        let mut buffer = Vec::new();

        // 1. Quiet section
        for _ in 0..10 {
            buffer.push(0.0);
        }

        // 2. Normal signal (Sine wave approximation - smooth changes)
        for i in 0..100 {
            let val = (i as f32 * 0.1).sin() * 0.5;
            buffer.push(val);
        }

        // 3. The CLICK (Huge jump in one sample)
        // Previous val is approx 0.5. Next sample jumps to 2.0.
        // Delta ~ 1.5. Normal delta in sine is ~0.05.
        // Threshold ~ 0.05 * 5 = 0.25.
        // So this should trigger.
        let click_idx = buffer.len();
        buffer.push(2.0);

        // 4. Return to normal
        for _ in 0..10 {
            buffer.push(0.0);
        }

        let original_click = buffer[click_idx];
        suppressor.process(&mut buffer);
        let processed_click = buffer[click_idx];

        println!(
            "Original: {}, Processed: {}",
            original_click, processed_click
        );

        // The click should be significantly reduced (slew limited)
        assert!(
            processed_click < original_click,
            "Click was not attenuated. Orig: {}, Proc: {}",
            original_click,
            processed_click
        );

        // It should be close to the previous value (plus threshold)
        // Previous value was ~ -0.25 (sin(9.9))
        // It shouldn't be 2.0.
        assert!(
            processed_click < 1.0,
            "Click attenuation insufficient. Value: {}",
            processed_click
        );
    }
}
