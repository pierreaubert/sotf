//! 8-line Feedback Delay Network with time-variant modulation.
//!
//! Core of the LARES-inspired reverb. Implements:
//! - 8 delay lines with mutually-prime lengths
//! - Hadamard mixing matrix (energy-preserving, O(N log N))
//! - Per-line tone correction filters (frequency-dependent RT60)
//! - Time-variant delay modulation (Griesinger's key innovation)
//! - Safety limiter in the feedback path

#![allow(
    clippy::needless_range_loop,
    reason = "fixed-size DSP loops over FDN_SIZE; the index is also used to access multiple parallel arrays"
)]

use crate::delay_line::DelayLine;
use crate::hadamard::hadamard8;
use crate::tone_filter::ToneFilter;

/// Number of delay lines in the FDN.
pub const FDN_SIZE: usize = 8;

/// Base delay lengths in samples at 48 kHz, room_size=1.0.
/// All prime to maximize echo density and minimize repetition patterns.
const BASE_DELAYS_48K: [usize; FDN_SIZE] = [1553, 1907, 2311, 2719, 3187, 3557, 4001, 4507];
const MAX_ROOM_SIZE: f32 = 3.0;

/// Per-line modulation frequencies in Hz (all different to avoid beating).
const MOD_FREQUENCIES: [f32; FDN_SIZE] = [0.07, 0.11, 0.13, 0.17, 0.19, 0.23, 0.29, 0.31];

/// 8-line Feedback Delay Network.
pub struct Fdn {
    delay_lines: [DelayLine; FDN_SIZE],
    tone_filters: [ToneFilter; FDN_SIZE],
    /// Current delay lengths in samples (scaled by room_size and sample_rate)
    delay_lengths: [usize; FDN_SIZE],
    previous_delay_lengths: [usize; FDN_SIZE],
    /// Allpass interpolation states for modulated reads
    interp_states: [f32; FDN_SIZE],
    previous_interp_states: [f32; FDN_SIZE],
    delay_transition_remaining: usize,
    delay_transition_samples: usize,
    /// LFO phases for time-variant modulation (0..1, one per line)
    mod_phases: [f32; FDN_SIZE],
    /// LFO phase increments per sample
    mod_phase_incs: [f32; FDN_SIZE],
    /// Modulation depth in samples
    mod_depth_samples: f32,
    /// Safety limiter threshold (linear)
    limiter_threshold: f32,
    /// Input gain distribution (equal for all lines)
    input_gain: f32,
    sample_rate: f32,
}

impl Fdn {
    /// Create a new FDN.
    ///
    /// - `sample_rate`: audio sample rate
    /// - `room_size`: scales delay lengths (0.2–3.0, default 1.0)
    /// - `rt60`: mid-frequency reverberation time in seconds
    /// - `bass_ratio`: RT60_bass / RT60_mid (e.g., 1.2)
    /// - `treble_ratio`: RT60_treble / RT60_mid (e.g., 0.5)
    /// - `mod_depth`: modulation depth 0.0–1.0 (maps to 0–8 samples)
    /// - `safety_limit_db`: limiter threshold below 0 dBFS
    pub fn new(
        sample_rate: u32,
        room_size: f32,
        rt60: f32,
        bass_ratio: f32,
        treble_ratio: f32,
        mod_depth: f32,
        safety_limit_db: f32,
    ) -> Self {
        let sr = sample_rate as f32;
        let scale = room_size * sr / 48000.0;

        let mut delay_lengths = [0usize; FDN_SIZE];
        for i in 0..FDN_SIZE {
            delay_lengths[i] = (BASE_DELAYS_48K[i] as f32 * scale).round() as usize;
            delay_lengths[i] = delay_lengths[i].max(1);
        }

        // Allocate for the maximum supported room size so room_size changes can
        // update delay lengths without reallocating on the audio/control path.
        let max_scale = MAX_ROOM_SIZE * sr / 48000.0;
        let max_delay = BASE_DELAYS_48K
            .iter()
            .map(|delay| (*delay as f32 * max_scale).ceil() as usize)
            .max()
            .unwrap_or(1);
        let alloc_size = max_delay + 16;
        let delay_lines = std::array::from_fn(|_| DelayLine::new(alloc_size));

        // Compute per-line tone correction filters
        let rt60_bass = rt60 * bass_ratio;
        let rt60_treble = rt60 * treble_ratio;
        let tone_filters = std::array::from_fn(|i| {
            let m = delay_lengths[i] as f32;
            let g_dc = 10.0_f32.powf(-3.0 * m / (rt60_bass * sr));
            let g_ny = 10.0_f32.powf(-3.0 * m / (rt60_treble * sr));
            ToneFilter::new(g_dc, g_ny)
        });

        // Modulation phase increments
        let mod_phase_incs = std::array::from_fn(|i| MOD_FREQUENCIES[i] / sr);

        let mod_depth_samples = mod_depth * 8.0;
        let limiter_threshold = 10.0_f32.powf(safety_limit_db / 20.0);
        let input_gain = 1.0 / (FDN_SIZE as f32).sqrt();

        Self {
            delay_lines,
            tone_filters,
            delay_lengths,
            previous_delay_lengths: delay_lengths,
            interp_states: [0.0; FDN_SIZE],
            previous_interp_states: [0.0; FDN_SIZE],
            delay_transition_remaining: 0,
            delay_transition_samples: (sr * 0.01).round().max(1.0) as usize,
            mod_phases: std::array::from_fn(|i| i as f32 / FDN_SIZE as f32),
            mod_phase_incs,
            mod_depth_samples,
            limiter_threshold,
            input_gain,
            sample_rate: sr,
        }
    }

    /// Process one input sample, producing 8 decorrelated output taps.
    ///
    /// Returns an array of 8 output samples, one per delay line.
    /// These are distributed across speakers by the plugin's routing stage.
    #[inline]
    pub fn process(&mut self, input: f32) -> [f32; FDN_SIZE] {
        let scaled_input = input * self.input_gain;

        // Read outputs from all delay lines
        let mut outputs = [0.0_f32; FDN_SIZE];
        for i in 0..FDN_SIZE {
            let base_delay = self.delay_lengths[i] as f32;

            if self.mod_depth_samples > 0.01 {
                // Time-variant: modulated delay with allpass interpolation
                let mod_offset =
                    (self.mod_phases[i] * std::f32::consts::TAU).sin() * self.mod_depth_samples;
                let effective_delay = (base_delay + mod_offset).max(1.0);
                let current =
                    self.delay_lines[i].read_allpass(effective_delay, &mut self.interp_states[i]);
                outputs[i] = if self.delay_transition_remaining > 0 {
                    let previous_delay = self.previous_delay_lengths[i] as f32 + mod_offset;
                    let previous = self.delay_lines[i]
                        .read_allpass(previous_delay.max(1.0), &mut self.previous_interp_states[i]);
                    let progress = 1.0
                        - self.delay_transition_remaining as f32
                            / self.delay_transition_samples as f32;
                    let angle = progress * std::f32::consts::FRAC_PI_2;
                    previous * angle.cos() + current * angle.sin()
                } else {
                    current
                };

                // Advance LFO phase
                self.mod_phases[i] += self.mod_phase_incs[i];
                if self.mod_phases[i] >= 1.0 {
                    self.mod_phases[i] -= 1.0;
                }
            } else {
                // Static delay — integer read
                let current = self.delay_lines[i].read(self.delay_lengths[i]);
                outputs[i] = if self.delay_transition_remaining > 0 {
                    let previous = self.delay_lines[i].read(self.previous_delay_lengths[i]);
                    let progress = 1.0
                        - self.delay_transition_remaining as f32
                            / self.delay_transition_samples as f32;
                    let angle = progress * std::f32::consts::FRAC_PI_2;
                    previous * angle.cos() + current * angle.sin()
                } else {
                    current
                };
            }
        }
        self.delay_transition_remaining = self.delay_transition_remaining.saturating_sub(1);

        // Apply tone correction (frequency-dependent decay)
        for i in 0..FDN_SIZE {
            outputs[i] = self.tone_filters[i].process(outputs[i]);
        }

        // Hadamard mixing (energy-preserving feedback matrix)
        let mut feedback = outputs;
        hadamard8(&mut feedback);

        // Safety limiter + write back into delay lines
        for i in 0..FDN_SIZE {
            let fb = feedback[i] + scaled_input;
            let limited = soft_clip(fb, self.limiter_threshold);
            self.delay_lines[i].push(limited);
        }

        outputs
    }

    /// Update RT60 parameters without reallocating.
    pub fn set_rt60(&mut self, rt60: f32, bass_ratio: f32, treble_ratio: f32) {
        let rt60_bass = rt60 * bass_ratio;
        let rt60_treble = rt60 * treble_ratio;
        for i in 0..FDN_SIZE {
            let m = self.delay_lengths[i] as f32;
            let g_dc = 10.0_f32.powf(-3.0 * m / (rt60_bass * self.sample_rate));
            let g_ny = 10.0_f32.powf(-3.0 * m / (rt60_treble * self.sample_rate));
            self.tone_filters[i].set_gains_smooth(
                g_dc,
                g_ny,
                (self.sample_rate * 0.005).round().max(1.0) as usize,
            );
        }
    }

    /// Update room size without reallocating delay buffers.
    ///
    /// Keeping delay-line state and modulator state avoids audible tail truncation when
    /// room_size changes at runtime.
    pub fn set_room_size(&mut self, room_size: f32, rt60: f32, bass_ratio: f32, treble_ratio: f32) {
        self.previous_delay_lengths = self.delay_lengths;
        self.previous_interp_states = self.interp_states;
        let scale = room_size.clamp(0.2, MAX_ROOM_SIZE) * self.sample_rate / 48000.0;
        for i in 0..FDN_SIZE {
            self.delay_lengths[i] = (BASE_DELAYS_48K[i] as f32 * scale).round() as usize;
            self.delay_lengths[i] = self.delay_lengths[i]
                .max(1)
                .min(self.delay_lines[i].max_delay_samples());
        }
        self.delay_transition_remaining = self.delay_transition_samples;
        self.set_rt60(rt60, bass_ratio, treble_ratio);
    }

    pub fn delay_transition_active(&self) -> bool {
        self.delay_transition_remaining > 0
    }

    /// Update modulation depth (0.0–1.0).
    pub fn set_mod_depth(&mut self, depth: f32) {
        self.mod_depth_samples = depth * 8.0;
    }

    /// Update safety limiter threshold.
    pub fn set_safety_limit_db(&mut self, db: f32) {
        self.limiter_threshold = 10.0_f32.powf(db / 20.0);
    }

    /// Reset all internal state (delay lines, filters, LFO phases).
    pub fn reset(&mut self) {
        for dl in &mut self.delay_lines {
            dl.reset();
        }
        for tf in &mut self.tone_filters {
            tf.reset();
        }
        self.interp_states = [0.0; FDN_SIZE];
        self.previous_interp_states = [0.0; FDN_SIZE];
        self.previous_delay_lengths = self.delay_lengths;
        self.delay_transition_remaining = 0;
        self.mod_phases = std::array::from_fn(|i| i as f32 / FDN_SIZE as f32);
    }
}

/// Soft-clip using tanh for smooth limiting.
#[inline]
fn soft_clip(x: f32, threshold: f32) -> f32 {
    if x.abs() <= threshold {
        x
    } else {
        threshold * (x / threshold).tanh()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fdn_impulse_decays() {
        let mut fdn = Fdn::new(48000, 1.0, 1.0, 1.0, 1.0, 0.0, 6.0);

        // Feed a single impulse
        let _ = fdn.process(1.0);

        // Process enough samples for ~1.5× RT60 (1.5 seconds at 48kHz)
        let mut max_energy = 0.0_f32;
        for _ in 0..72000 {
            let outputs = fdn.process(0.0);
            let energy: f32 = outputs.iter().map(|v| v * v).sum();
            if energy > max_energy {
                max_energy = energy;
            }
        }

        // After 1.5 RT60, signal should have decayed significantly
        let mut late_energy = 0.0_f32;
        for _ in 0..1000 {
            let outputs = fdn.process(0.0);
            late_energy += outputs.iter().map(|v| v * v).sum::<f32>();
        }
        late_energy /= 1000.0;

        assert!(
            late_energy < max_energy * 0.01,
            "Signal should decay: max_energy={max_energy}, late_avg={late_energy}"
        );
    }

    #[test]
    fn test_fdn_no_blowup() {
        let mut fdn = Fdn::new(48000, 1.0, 2.0, 1.5, 0.5, 0.5, 6.0);

        // Process 5 seconds of white-ish noise
        let mut max_sample = 0.0_f32;
        for i in 0..240000 {
            let input = ((i as f32 * 0.1234).sin() * 0.5).sin() * 0.3;
            let outputs = fdn.process(input);
            for &v in &outputs {
                if v.abs() > max_sample {
                    max_sample = v.abs();
                }
            }
        }

        assert!(
            max_sample < 2.0,
            "FDN should not blow up, max_sample={max_sample}"
        );
        assert!(max_sample.is_finite(), "Outputs must be finite");
    }

    #[test]
    fn test_fdn_decorrelation() {
        let mut fdn = Fdn::new(48000, 1.0, 1.5, 1.2, 0.5, 0.3, 6.0);

        // Feed impulse
        let _ = fdn.process(1.0);

        // Measure normalized cross-correlation of two output lines after the
        // first echo arrivals, rather than comparing only their sums.
        let mut cross = 0.0_f64;
        let mut energy_a = 0.0_f64;
        let mut energy_b = 0.0_f64;
        for _ in 0..48_000 {
            let outputs = fdn.process(0.0);
            let a = f64::from(outputs[0]);
            let b = f64::from(outputs[7]);
            cross += a * b;
            energy_a += a * a;
            energy_b += b * b;
        }
        let correlation = cross / (energy_a * energy_b).sqrt().max(1e-30);
        assert!(
            correlation.abs() < 0.5,
            "FDN lines should be decorrelated, correlation={correlation}"
        );
    }

    #[test]
    fn test_fdn_reset() {
        let mut fdn = Fdn::new(48000, 1.0, 1.5, 1.2, 0.5, 0.3, 6.0);
        let _ = fdn.process(1.0);
        for _ in 0..1000 {
            fdn.process(0.0);
        }

        fdn.reset();

        // After reset, processing silence should produce silence
        for _ in 0..100 {
            let outputs = fdn.process(0.0);
            for &v in &outputs {
                assert!(v.abs() < 1e-10, "After reset, output should be silent");
            }
        }
    }

    #[test]
    fn test_set_room_size_preserves_state() {
        let mut fdn = Fdn::new(48000, 1.0, 1.5, 1.2, 0.8, 0.4, 6.0);

        // Build up some feedback energy.
        for i in 0..20000 {
            let input = ((i as f32 * 0.123).sin() * 0.3).sin() * 0.5;
            fdn.process(input);
        }

        let before: Vec<f32> = (0..2000).flat_map(|_| fdn.process(0.0)).collect();
        let before_energy: f32 = before.iter().map(|v| v * v).sum();
        assert!(before_energy > 1e-8);

        fdn.set_room_size(2.5, 1.5, 1.2, 0.8);

        let after: Vec<f32> = (0..2000).flat_map(|_| fdn.process(0.0)).collect();
        let after_energy: f32 = after.iter().map(|v| v * v).sum();

        // Room-size updates should not truncate feedback energy to silence.
        assert!(
            after_energy > before_energy * 0.1,
            "FDN state should persist across room-size changes: before={before_energy}, after={after_energy}"
        );
    }

    #[test]
    fn test_soft_clip() {
        assert!((soft_clip(0.5, 1.0) - 0.5).abs() < 1e-6);
        assert!((soft_clip(-0.5, 1.0) - (-0.5)).abs() < 1e-6);
        // Above threshold, should be compressed
        let clipped = soft_clip(2.0, 1.0);
        assert!(clipped < 2.0 && clipped > 0.9);
        // Symmetric
        assert!((soft_clip(2.0, 1.0) + soft_clip(-2.0, 1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_safety_limit_is_headroom_above_nominal() {
        let fdn = Fdn::new(48_000, 1.0, 1.8, 1.2, 0.5, 0.0, 6.0);
        let expected = 10.0_f32.powf(6.0 / 20.0);
        assert!((fdn.limiter_threshold - expected).abs() < 1e-6);
        assert_eq!(soft_clip(1.0, fdn.limiter_threshold), 1.0);
        assert!(soft_clip(4.0, fdn.limiter_threshold).abs() < 4.0);
    }
}
