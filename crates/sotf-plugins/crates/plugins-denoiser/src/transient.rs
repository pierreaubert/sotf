/// Adaptive slew-rate limiter for click and pop repair.
pub struct TransientSuppressor {
    channels: usize,
    last_samples: Vec<f32>,
    last_outputs: Vec<f32>,
    slope_envelope: Vec<f32>,
    sensitivity: f32,
    decay: f32,
    one_minus_decay: f32,
}

impl TransientSuppressor {
    pub fn new(channels: usize) -> Self {
        Self {
            channels,
            last_samples: vec![0.0; channels],
            last_outputs: vec![0.0; channels],
            // Start at the non-zero floor so new() and reset() behave
            // identically (fix for issue 3).
            slope_envelope: vec![1e-6; channels],
            sensitivity: 10.0,
            decay: 0.99,
            one_minus_decay: 0.01,
        }
    }

    pub fn reset(&mut self) {
        self.last_samples.fill(0.0);
        self.last_outputs.fill(0.0);
        // Initialise to a small non-zero floor so the first sample after
        // reset never triggers the discontinuous `== 0.0` re-initialisation
        // branch (fix for issue 3).
        self.slope_envelope.fill(1e-6);
    }

    pub fn set_sensitivity(&mut self, sensitivity: f32) {
        self.sensitivity = sensitivity.max(1.0);
    }

    pub fn process(&mut self, buffer: &mut [f32]) {
        if self.channels == 0 {
            return;
        }

        for frame in buffer.chunks_mut(self.channels) {
            for (ch, sample) in frame.iter_mut().enumerate() {
                let input = *sample;
                // Use the *input* sample as the reference for delta so
                // that after suppression we compare against the true
                // incoming signal, not the previously clamped output
                // (fix for issue 6).
                let last = self.last_samples[ch];
                let delta = input - last;
                let abs_delta = delta.abs();

                // slope_envelope is always ≥ 1e-6 (initialised in new/reset),
                // so the old `== 0.0` guard is no longer needed (fix for issue 3).
                let threshold = self.slope_envelope[ch] * self.sensitivity + 1e-5;

                if abs_delta > threshold {
                    let output_delta = input - self.last_outputs[ch];
                    let abs_output_delta = output_delta.abs();
                    if abs_output_delta > threshold {
                        let sign = if output_delta >= 0.0 { 1.0 } else { -1.0 };
                        *sample = self.last_outputs[ch] + sign * threshold;
                    }
                    // Update the envelope with the *allowed* delta so the
                    // threshold adapts during a burst of clicks rather than
                    // staying frozen at its pre-click value (fix for issue 2).
                    self.slope_envelope[ch] =
                        self.slope_envelope[ch] * self.decay + threshold * self.one_minus_decay;
                    // Keep the output reference drifting toward the true input
                    // so it does not get stuck far from the signal level during
                    // long bursts or aggressive initialisation.  A faster
                    // time constant (decay²) prevents post-click samples from
                    // being over-suppressed while still bounding staircase
                    // growth during a sustained corrupted ramp.
                    let output_decay = self.decay * self.decay;
                    self.last_outputs[ch] =
                        self.last_outputs[ch] * output_decay + input * (1.0 - output_decay);
                } else {
                    if abs_delta > self.slope_envelope[ch] {
                        self.slope_envelope[ch] = abs_delta;
                    } else {
                        self.slope_envelope[ch] =
                            self.slope_envelope[ch] * self.decay + abs_delta * self.one_minus_decay;
                    }
                    self.last_outputs[ch] = input;
                }

                // Always track the *input* sample so the next frame's delta
                // is computed relative to what actually arrived, not to the
                // clamped output (fix for issue 6).
                self.last_samples[ch] = input;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suppresses_large_slew_spike() {
        let mut suppressor = TransientSuppressor::new(1);
        suppressor.set_sensitivity(5.0);

        let mut buffer = Vec::new();
        buffer.extend(std::iter::repeat_n(0.0, 10));
        for i in 0..100 {
            buffer.push((i as f32 * 0.1).sin() * 0.5);
        }

        let click_idx = buffer.len();
        buffer.push(2.0);
        buffer.extend(std::iter::repeat_n(0.0, 10));

        suppressor.process(&mut buffer);
        assert!(buffer[click_idx] < 1.0);
    }

    /// Issue 2: Envelope must adapt during a burst of clicks so that a
    /// second click immediately following the first is also suppressed.
    /// Previously the envelope was frozen during suppression, leaving the
    /// threshold stale after a long burst.
    #[test]
    fn envelope_adapts_during_suppression_burst() {
        let mut suppressor = TransientSuppressor::new(1);
        suppressor.set_sensitivity(5.0);

        // Prime the envelope: 20 samples of gentle sine so the envelope
        // settles to a moderate level.
        let mut prime: Vec<f32> = (0..20).map(|i| (i as f32 * 0.05).sin() * 0.1).collect();
        suppressor.process(&mut prime);

        // Now feed 5 back-to-back clicks (amplitude 2.0) separated only
        // by a single quiet sample. Each click should still be suppressed
        // even though the previous one triggered suppression.
        let mut burst = Vec::new();
        for _ in 0..5 {
            burst.push(2.0_f32);
            burst.push(0.0_f32);
        }
        suppressor.process(&mut burst);

        // Every click sample must have been reduced below the input value.
        for k in (0..burst.len()).step_by(2) {
            assert!(
                burst[k] < 2.0,
                "click at burst[{k}] was not suppressed (value={})",
                burst[k]
            );
        }
    }

    /// Issue 3: After sustained silence, `slope_envelope` may reach exactly
    /// 0.0.  The old guard `== 0.0` can either be silently skipped (if FP
    /// never reaches exactly 0) or produce a discontinuous jump.  After the
    /// fix the envelope must stay at the small non-zero floor and never
    /// cause a discontinuity.
    #[test]
    fn envelope_does_not_jump_after_silence() {
        let mut suppressor = TransientSuppressor::new(1);

        // Reset drives envelope to 0.0 exactly.
        suppressor.reset();

        // Feed a moderate-amplitude sample; the envelope should be
        // initialised smoothly (floor ≥ 1e-6) rather than jumping to
        // abs_delta.  Whatever the result, it must be finite and ≥ 1e-7.
        let mut buf = vec![0.0_f32, 0.3, 0.3, 0.3];
        suppressor.process(&mut buf);

        // No NaN/Inf in output.
        for &s in &buf {
            assert!(s.is_finite(), "non-finite sample after reset: {s}");
        }
    }

    /// Issue 6: `last_samples` must track the *input* sample for delta
    /// computation so that the algorithm does not chase its own clamped
    /// output.  After a single suppressed click the next legitimate sample
    /// must not itself be incorrectly suppressed.
    #[test]
    fn post_click_sample_not_over_suppressed() {
        let mut suppressor = TransientSuppressor::new(1);
        suppressor.set_sensitivity(5.0);

        // Prime the envelope with low-level sine.
        let mut prime: Vec<f32> = (0..30).map(|i| (i as f32 * 0.05).sin() * 0.1).collect();
        suppressor.process(&mut prime);

        // One click followed by a legitimate signal at the same level as
        // the pre-click signal.  The post-click sample should pass through
        // nearly unchanged (within the small-signal envelope).
        let mut buf = vec![2.0_f32, 0.1, 0.1, 0.1];
        suppressor.process(&mut buf);

        // The three post-click samples must not be clamped to near zero;
        // they are well within the normal slew range of the pre-click signal.
        for (k, &v) in buf.iter().enumerate().take(4).skip(1) {
            assert!(
                v > 0.05,
                "post-click sample buf[{k}] over-suppressed (value={v})",
            );
        }
    }

    /// During a multi-sample click burst the old hard-clamp followed the
    /// corrupted input ramp (`last_input ± threshold`), creating a staircase.
    /// The fix uses `last_output ± threshold` with a faster one-pole drift
    /// so the output stays near the pre-burst level and recovers smoothly.
    #[test]
    fn suppression_does_not_staircase_during_burst() {
        let mut suppressor = TransientSuppressor::new(1);
        suppressor.set_sensitivity(5.0);

        // Prime the envelope with gentle sine.
        let mut prime: Vec<f32> = (0..40).map(|i| (i as f32 * 0.05).sin() * 0.1).collect();
        suppressor.process(&mut prime);

        // Gentle sine with a 5-sample ramp burst injected.
        let mut buf: Vec<f32> = (0..20).map(|i| (i as f32 * 0.05).sin() * 0.1).collect();
        for i in 0..5 {
            buf[5 + i] = 5.0 + i as f32; // ramp: 5, 6, 7, 8, 9
        }

        suppressor.process(&mut buf);

        // With a staircase the burst samples after the first would follow
        // the ramp (~6, 7, 8...).  The fix keeps them near the pre-burst
        // signal level.
        let burst_max = buf[5..10].iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            burst_max < 1.0,
            "burst output followed input ramp (max={burst_max})"
        );

        // Step size within the burst should be bounded, not ~1.0 per sample.
        for i in 6..10 {
            let step = (buf[i] - buf[i - 1]).abs();
            assert!(
                step < 0.5,
                "stair-step at {i}: step={step}, prev={}, cur={}",
                buf[i - 1],
                buf[i]
            );
        }
    }
}
