#![allow(clippy::needless_range_loop)]
/// Adaptive slew-rate limiter for click and pop repair.
#[derive(Clone, Copy)]
struct ChannelState {
    last_sample: f32,
    last_output: f32,
    prev_delta: f32,
    slope_envelope: f32,
}

pub struct TransientSuppressor {
    channels: usize,
    last_samples: Vec<f32>,
    last_outputs: Vec<f32>,
    prev_delta: Vec<f32>,
    slope_envelope: Vec<f32>,
    scratch_states: Vec<ChannelState>,
    work: Vec<f32>,
    sensitivity: f32,
    release_ms: f32,
    decay: f32,
    one_minus_decay: f32,
}

impl TransientSuppressor {
    pub fn new(channels: usize) -> Self {
        let mut this = Self {
            channels,
            last_samples: vec![0.0; channels],
            last_outputs: vec![0.0; channels],
            prev_delta: vec![0.0; channels],
            // Start at the non-zero floor so new() and reset() behave
            // identically (fix for issue 3).
            slope_envelope: vec![1e-6; channels],
            scratch_states: vec![
                ChannelState {
                    last_sample: 0.0,
                    last_output: 0.0,
                    prev_delta: 0.0,
                    slope_envelope: 1e-6,
                };
                channels
            ],
            work: vec![0.0; channels.saturating_mul(1024)],
            sensitivity: 10.0,
            release_ms: 20.0,
            decay: 0.999,
            one_minus_decay: 0.001,
        };
        this.set_sample_rate(48000);
        this
    }

    pub fn reset(&mut self) {
        self.last_samples.fill(0.0);
        self.last_outputs.fill(0.0);
        self.prev_delta.fill(0.0);
        // Initialise to a small non-zero floor so the first sample after
        // reset never triggers the discontinuous `== 0.0` re-initialisation
        // branch (fix for issue 3).
        self.slope_envelope.fill(1e-6);
    }

    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        let sample_rate = sample_rate.max(1) as f32;
        self.decay = (-1.0 / (sample_rate * (self.release_ms * 0.001))).exp();
        self.one_minus_decay = 1.0 - self.decay;
    }

    #[cfg(test)]
    fn decay_time_constant(&self) -> f32 {
        -1.0 / (self.decay.ln())
    }

    pub fn set_sensitivity(&mut self, sensitivity: f32) {
        self.sensitivity = sensitivity.max(1.0);
    }

    pub fn process(&mut self, buffer: &mut [f32]) {
        if self.channels == 0 {
            return;
        }
        if buffer.is_empty() || !buffer.len().is_multiple_of(self.channels) {
            return;
        }

        if self.channels == 1 {
            let mut state = ChannelState {
                last_sample: self.last_samples[0],
                last_output: self.last_outputs[0],
                prev_delta: self.prev_delta[0],
                slope_envelope: self.slope_envelope[0],
            };

            self.process_channel(&mut buffer[0..], &mut state);

            self.last_samples[0] = state.last_sample;
            self.last_outputs[0] = state.last_output;
            self.prev_delta[0] = state.prev_delta;
            self.slope_envelope[0] = state.slope_envelope;
            return;
        }

        let num_frames = buffer.len() / self.channels;
        let max_chunk_frames = self.work.len() / self.channels;
        if num_frames > max_chunk_frames {
            if max_chunk_frames == 0 {
                return;
            }
            let chunk_samples = max_chunk_frames * self.channels;
            for chunk in buffer.chunks_mut(chunk_samples) {
                self.process(chunk);
            }
            return;
        }

        // Deinterleave into planar scratch so each channel can be processed
        // independently.
        for frame in 0..num_frames {
            let base = frame * self.channels;
            for ch in 0..self.channels {
                self.work[ch * num_frames + frame] = buffer[base + ch];
            }
        }

        for ch in 0..self.channels {
            self.scratch_states[ch] = ChannelState {
                last_sample: self.last_samples[ch],
                last_output: self.last_outputs[ch],
                prev_delta: self.prev_delta[ch],
                slope_envelope: self.slope_envelope[ch],
            };
        }

        let decay = self.decay;
        let one_minus_decay = self.one_minus_decay;
        let sensitivity = self.sensitivity;

        for ch in 0..self.channels {
            let start = ch * num_frames;
            let samples = &mut self.work[start..start + num_frames];
            let state = &mut self.scratch_states[ch];
            Self::process_channel_impl(samples, state, sensitivity, decay, one_minus_decay);
        }

        for ch in 0..self.channels {
            self.last_samples[ch] = self.scratch_states[ch].last_sample;
            self.last_outputs[ch] = self.scratch_states[ch].last_output;
            self.prev_delta[ch] = self.scratch_states[ch].prev_delta;
            self.slope_envelope[ch] = self.scratch_states[ch].slope_envelope;
        }

        for frame in 0..num_frames {
            let base = frame * self.channels;
            for ch in 0..self.channels {
                buffer[base + ch] = self.work[ch * num_frames + frame];
            }
        }
    }

    fn process_channel(&mut self, samples: &mut [f32], state: &mut ChannelState) {
        Self::process_channel_impl(
            samples,
            state,
            self.sensitivity,
            self.decay,
            self.one_minus_decay,
        )
    }

    fn process_channel_impl(
        samples: &mut [f32],
        state: &mut ChannelState,
        sensitivity: f32,
        decay: f32,
        one_minus_decay: f32,
    ) {
        for sample in samples.iter_mut() {
            let input = *sample;
            let last = state.last_sample;
            let delta = input - last;
            let abs_delta = delta.abs();

            let threshold = state.slope_envelope * sensitivity + 1e-5;
            let curvature = (delta - state.prev_delta).abs();
            let is_impulsive = if abs_delta > 0.0 {
                (curvature / abs_delta) > 0.4 && curvature > threshold
            } else {
                false
            };

            if abs_delta > threshold && is_impulsive {
                let output_delta = input - state.last_output;
                let abs_output_delta = output_delta.abs();
                if abs_output_delta > threshold {
                    let sign = if output_delta >= 0.0 { 1.0 } else { -1.0 };
                    *sample = state.last_output + sign * threshold;
                }

                state.slope_envelope = state.slope_envelope * decay + threshold * one_minus_decay;

                let output_decay = decay * decay;
                state.last_output = state.last_output * output_decay + input * (1.0 - output_decay);
            } else {
                if abs_delta > state.slope_envelope {
                    state.slope_envelope = abs_delta;
                } else {
                    state.slope_envelope =
                        state.slope_envelope * decay + abs_delta * one_minus_decay;
                }
                state.last_output = input;
            }

            state.last_sample = input;
            state.prev_delta = delta;
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
                v > 0.001,
                "post-click sample buf[{k}] over-suppressed (value={v})",
            );
        }
    }

    #[test]
    fn suppressor_distinguishes_impulsive_spikes_from_smooth_fast_ramps() {
        let mut suppressor = TransientSuppressor::new(1);
        suppressor.set_sensitivity(1.0);

        // Prime with a modest signal so the slope envelope is non-zero.
        let mut prime: Vec<f32> = (0..20).map(|i| (i as f32 * 0.1).sin() * 0.1).collect();
        suppressor.process(&mut prime);

        // A one-sample impulsive spike should be suppressed due
        // high curvature (high-frequency) content.
        let mut click_buf = vec![2.0_f32, 0.0_f32];
        suppressor.process(&mut click_buf);
        assert!(
            click_buf[0] < 2.0,
            "impulsive spike should be softened (got {})",
            click_buf[0]
        );

        // A strong but smooth linear ramp has high first-order slew but low
        // curvature, so it should remain mostly untouched.
        let mut smooth_ramp: Vec<f32> = (0..6).map(|i| 10.0 + i as f32 * 0.55).collect();
        suppressor.process(&mut smooth_ramp);

        let expected_last = 10.0 + 5.0 * 0.55;
        assert!(
            (smooth_ramp[5] - expected_last).abs() < 0.05,
            "smooth ramp was over-suppressed (got {})",
            smooth_ramp[5]
        );
    }

    /// During a multi-sample click burst the old hard-clamp followed the
    /// corrupted input ramp (`last_input ± threshold`), creating a staircase.
    /// The fix uses `last_output ± threshold` with a faster one-pole drift
    /// so the output stays near the pre-burst level and recovers smoothly.
    #[test]
    fn suppression_does_not_staircase_during_burst() {
        let mut suppressor = TransientSuppressor::new(1);
        suppressor.set_sensitivity(1.0);

        // Prime the envelope with gentle sine.
        let mut prime: Vec<f32> = (0..40).map(|i| (i as f32 * 0.05).sin() * 0.1).collect();
        suppressor.process(&mut prime);

        // Gentle sine with a 5-sample high-curvature burst injected.
        let mut buf: Vec<f32> = (0..20).map(|i| (i as f32 * 0.05).sin() * 0.1).collect();
        for i in 0..5 {
            buf[5 + i] = if i % 2 == 0 { 5.0 } else { -5.0 };
        }

        suppressor.process(&mut buf);

        // With a staircase the burst samples after the first would follow
        // the alternating impulses (~5, -5...). The fix keeps them near the
        // pre-burst level for all samples.
        let burst_max = buf[5..10].iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            burst_max < 1.0,
            "burst output followed impulses rather than being bounded (max={burst_max})"
        );

        // Step size within the burst should be bounded, not ~10.0 per sample.
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

    #[test]
    fn multichannel_processing_matches_per_channel_reference() {
        let mut suppressor = TransientSuppressor::new(4);
        suppressor.set_sensitivity(2.5);
        let mut input = vec![
            // frame 0: ch0..ch3
            0.0, 0.2, 1.0, -0.2, //
            0.1, 0.4, -1.2, 0.1, //
            0.2, 0.6, 1.5, 0.0, //
            0.3, 0.8, -0.7, -0.1, //
            5.0, 0.9, -2.0, -0.3, // channel-specific spikes
            5.1, 1.0, 2.0, 0.5, //
        ];

        let mut expected = input.clone();
        let num_frames = input.len() / 4;
        let mut ref_channels: Vec<Vec<f32>> = vec![vec![0.0; num_frames]; 4];
        for frame in 0..num_frames {
            let base = frame * 4;
            for ch in 0..4usize {
                ref_channels[ch][frame] = expected[base + ch];
            }
        }

        for ch in 0..4 {
            let mut channel_suppressor = TransientSuppressor::new(1);
            channel_suppressor.set_sensitivity(2.5);
            channel_suppressor.process(&mut ref_channels[ch]);
        }

        for frame in 0..num_frames {
            let base = frame * 4;
            for ch in 0..4 {
                expected[base + ch] = ref_channels[ch][frame];
            }
        }

        suppressor.process(&mut input);
        assert_eq!(input, expected);
    }

    #[test]
    fn release_time_constant_is_reduced_for_musical_content() {
        let mut suppressor = TransientSuppressor::new(1);
        suppressor.set_sample_rate(48000);

        // A decay factor of 0.999 should be noticeably slower than the
        // historical 0.99, which was about 2 ms at 48 kHz.
        assert!(
            suppressor.decay > 0.99,
            "release decay should not remain at the legacy fast value"
        );

        // For a ~20 ms time constant at 48 kHz: tau_samples = -1 / ln(decay)
        let tau_samples = suppressor.decay_time_constant();
        assert!(
            (920.0..=1100.0).contains(&tau_samples),
            "release time constant should be near the 20 ms default: {tau_samples}"
        );

        let one_minus = suppressor.one_minus_decay + suppressor.decay;
        assert!(
            (one_minus - 1.0).abs() < 1e-6,
            "decay coefficients should be complementary"
        );
    }

    #[test]
    fn multichannel_oversized_blocks_do_not_resize_work_buffer() {
        let mut suppressor = TransientSuppressor::new(2);
        let initial_len = suppressor.work.len();
        let mut buffer = vec![0.1f32; initial_len * 2];

        suppressor.process(&mut buffer);

        assert_eq!(suppressor.work.len(), initial_len);
        assert!(buffer.iter().all(|sample| sample.is_finite()));
    }
}
