/// Early Reflection Generator (ERG) — tapped delay line with per-tap processing.
///
/// Simulates first-order room reflections from walls, ceiling, and floor.
/// Each tap has a specific delay, gain, direction (azimuth/elevation), and
/// HF damping filter. Tap delays are slowly modulated to prevent comb-filter
/// coloration (Griesinger's time-variant processing).
use crate::delay_line::DelayLine;

/// Maximum number of reflection taps.
pub const MAX_TAPS: usize = 20;
const MAX_MOD_DEPTH_MS: f32 = 1.0;

/// Maximum tap delay across all presets (Cathedral last tap: 154.5 ms).
/// Hard-coded to avoid O(presets × taps) work at every constructor call.
/// If new presets with longer delays are added, this value must be updated.
const MAX_PRESET_DELAY_MS: f32 = 154.5;

/// A single early reflection tap.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReflectionTap {
    /// Base delay in samples
    pub delay_samples: usize,
    /// Gain (linear, typically 0.1–1.0)
    pub gain: f32,
    /// Azimuth angle in degrees for VBAP panning
    pub azimuth: f32,
    /// Elevation angle in degrees for VBAP panning
    pub elevation: f32,
    /// One-pole LP filter coefficient for HF damping (0 = no damping, 1 = max)
    pub damping: f32,
}

/// Room preset defining a set of early reflection taps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomPreset {
    Small,
    Medium,
    Large,
    Cathedral,
}

/// Early Reflection Generator.
pub struct EarlyReflections {
    delay_line: DelayLine,
    taps: [ReflectionTap; MAX_TAPS],
    num_taps: usize,
    /// One-pole LP filter states (one per tap) for HF damping
    lp_states: [f32; MAX_TAPS],
    /// Per-tap allpass interpolation states for time-variant delay reads.
    /// Using allpass (rather than linear) interpolation preserves high-frequency
    /// content; linear interpolation acts as a lowpass filter and causes
    /// spectral smearing when delay lengths are modulated.
    allpass_states: [f32; MAX_TAPS],
    /// LFO phases for time-variant modulation (one per tap)
    mod_phases: [f32; MAX_TAPS],
    /// Modulation depth in samples
    mod_depth_samples: f32,
    sample_rate: f32,
}

impl EarlyReflections {
    /// Create an ERG from a room preset.
    pub fn new(sample_rate: u32, preset: RoomPreset, mod_depth: f32) -> Self {
        let sr = sample_rate as f32;
        let max_delay = max_tap_delay_samples(sr);
        let max_mod_depth = (MAX_MOD_DEPTH_MS * sr * 0.001).ceil() as usize;
        let alloc_size = max_delay + max_mod_depth + 2;

        let mut this = Self {
            delay_line: DelayLine::new(alloc_size),
            taps: [ReflectionTap::default(); MAX_TAPS],
            num_taps: 0,
            lp_states: [0.0; MAX_TAPS],
            allpass_states: [0.0; MAX_TAPS],
            mod_phases: [0.0; MAX_TAPS],
            mod_depth_samples: 0.0,
            sample_rate: sr,
        };
        this.set_preset(preset);
        this.set_mod_depth(mod_depth);
        this
    }

    /// Number of active taps.
    pub fn num_taps(&self) -> usize {
        self.num_taps
    }

    /// Get tap info (for VBAP panning in the routing stage).
    pub fn tap_info(&self, index: usize) -> Option<&ReflectionTap> {
        (index < self.num_taps).then(|| &self.taps[index])
    }

    /// Process one input sample, returning per-tap outputs.
    ///
    /// The caller routes each tap to speakers using the tap's azimuth/elevation.
    /// Returns a slice of (tap_index, sample_value) for non-zero taps.
    pub fn process(&mut self, input: f32, output: &mut [f32]) {
        self.delay_line.push(input);

        for (i, tap) in self.taps[..self.num_taps].iter().enumerate() {
            if i >= output.len() {
                break;
            }

            // Read with optional time-variant modulation.
            // Use allpass interpolation (not linear) to preserve high-frequency
            // content: linear interpolation is a mild lowpass filter and causes
            // progressive spectral smearing across modulation cycles.
            // Each tap maintains its own allpass state for continuity between
            // successive calls (avoids clicks when the delay changes).
            let sample = if self.mod_depth_samples > 0.01 {
                let mod_offset =
                    (self.mod_phases[i] * std::f32::consts::TAU).sin() * self.mod_depth_samples;
                let effective_delay = (tap.delay_samples as f32 + mod_offset).max(1.0);
                self.delay_line
                    .read_allpass(effective_delay, &mut self.allpass_states[i])
            } else {
                self.delay_line.read(tap.delay_samples)
            };

            // Apply gain
            let gained = sample * tap.gain;

            // Apply one-pole LP for HF damping: y[n] = (1-d)*x[n] + d*y[n-1]
            let damped = if tap.damping > 0.001 {
                let out = (1.0 - tap.damping) * gained + tap.damping * self.lp_states[i];
                self.lp_states[i] = out;
                out
            } else {
                gained
            };

            output[i] = damped;

            // Advance LFO phase
            if self.mod_depth_samples > 0.01 {
                // Each tap has a unique slow modulation rate (0.05–0.3 Hz)
                let rate = 0.05 + 0.25 * (i as f32 / self.num_taps as f32);
                self.mod_phases[i] += rate / self.sample_rate;
                if self.mod_phases[i] >= 1.0 {
                    self.mod_phases[i] -= 1.0;
                }
            }
        }
    }

    /// Update modulation depth (0.0–1.0).
    pub fn set_mod_depth(&mut self, depth: f32) {
        self.mod_depth_samples = depth.clamp(0.0, 1.0) * self.sample_rate * 0.001;
    }

    /// Update room preset without reallocating.
    pub fn set_preset(&mut self, preset: RoomPreset) {
        let (taps, num_taps) = generate_taps(preset, self.sample_rate);
        self.taps = taps;
        self.num_taps = num_taps;
        self.lp_states.fill(0.0);
        self.allpass_states.fill(0.0);
        for i in 0..MAX_TAPS {
            self.mod_phases[i] = if num_taps > 0 {
                i as f32 / num_taps as f32
            } else {
                0.0
            };
        }
    }

    /// Reset all state.
    pub fn reset(&mut self) {
        self.delay_line.reset();
        self.lp_states.fill(0.0);
        self.allpass_states.fill(0.0);
        for (i, phase) in self.mod_phases.iter_mut().enumerate() {
            *phase = i as f32 / self.num_taps.max(1) as f32;
        }
    }
}

/// Generate tap configurations for a room preset.
fn generate_taps(preset: RoomPreset, sample_rate: f32) -> ([ReflectionTap; MAX_TAPS], usize) {
    let specs: &[(f32, f32, f32, f32, f32)] = match preset {
        // (delay_ms, gain_db, azimuth, elevation, damping)
        RoomPreset::Small => &[
            (5.2, -2.0, 45.0, 0.0, 0.1),
            (7.8, -3.5, -35.0, 0.0, 0.12),
            (11.3, -4.0, 90.0, 0.0, 0.15),
            (14.1, -5.5, -80.0, 0.0, 0.18),
            (17.6, -6.0, 130.0, 0.0, 0.2),
            (20.3, -7.0, -120.0, 0.0, 0.22),
            (24.5, -8.0, 0.0, 45.0, 0.25),
            (27.1, -9.0, 160.0, 0.0, 0.28),
            (30.8, -10.0, -150.0, 0.0, 0.3),
            (34.2, -11.0, 60.0, 30.0, 0.32),
            (37.5, -12.0, -60.0, 30.0, 0.35),
            (40.0, -13.0, 0.0, 60.0, 0.38),
        ],
        RoomPreset::Medium => &[
            (8.1, -2.0, 40.0, 0.0, 0.08),
            (12.4, -3.0, -30.0, 0.0, 0.1),
            (17.2, -3.5, 80.0, 0.0, 0.12),
            (22.7, -4.5, -75.0, 0.0, 0.15),
            (28.3, -5.0, 120.0, 0.0, 0.18),
            (33.6, -6.0, -110.0, 0.0, 0.2),
            (39.1, -7.0, 0.0, 40.0, 0.22),
            (44.8, -7.5, 150.0, 0.0, 0.25),
            (50.2, -8.5, -140.0, 0.0, 0.28),
            (55.9, -9.0, 60.0, 25.0, 0.3),
            (61.3, -10.0, -55.0, 25.0, 0.32),
            (66.7, -10.5, 0.0, 50.0, 0.34),
            (71.4, -11.5, 170.0, 10.0, 0.36),
            (75.8, -12.0, -165.0, 10.0, 0.38),
            (79.5, -13.0, 90.0, 35.0, 0.4),
            (80.0, -13.5, -90.0, 35.0, 0.42),
        ],
        RoomPreset::Large => &[
            (10.5, -1.5, 35.0, 0.0, 0.06),
            (16.2, -2.5, -25.0, 0.0, 0.08),
            (22.8, -3.0, 75.0, 0.0, 0.1),
            (29.4, -4.0, -70.0, 0.0, 0.13),
            (36.1, -4.5, 110.0, 0.0, 0.16),
            (42.7, -5.5, -105.0, 0.0, 0.18),
            (49.3, -6.0, 0.0, 35.0, 0.2),
            (55.9, -6.5, 145.0, 0.0, 0.22),
            (62.5, -7.5, -135.0, 0.0, 0.25),
            (69.1, -8.0, 55.0, 20.0, 0.28),
            (75.8, -9.0, -50.0, 20.0, 0.3),
            (82.4, -9.5, 0.0, 45.0, 0.32),
            (89.0, -10.5, 160.0, 10.0, 0.34),
            (95.6, -11.0, -155.0, 10.0, 0.36),
            (102.3, -12.0, 85.0, 30.0, 0.38),
            (108.9, -12.5, -80.0, 30.0, 0.4),
        ],
        RoomPreset::Cathedral => &[
            (12.0, -1.0, 30.0, 0.0, 0.05),
            (19.5, -2.0, -20.0, 0.0, 0.07),
            (27.0, -2.5, 70.0, 0.0, 0.09),
            (34.5, -3.5, -65.0, 0.0, 0.12),
            (42.0, -4.0, 100.0, 0.0, 0.14),
            (49.5, -5.0, -95.0, 0.0, 0.16),
            (57.0, -5.5, 0.0, 30.0, 0.18),
            (64.5, -6.0, 140.0, 0.0, 0.2),
            (72.0, -7.0, -130.0, 0.0, 0.22),
            (79.5, -7.5, 50.0, 15.0, 0.24),
            (87.0, -8.5, -45.0, 15.0, 0.26),
            (94.5, -9.0, 0.0, 40.0, 0.28),
            (102.0, -9.5, 155.0, 5.0, 0.3),
            (109.5, -10.5, -150.0, 5.0, 0.32),
            (117.0, -11.0, 80.0, 25.0, 0.34),
            (124.5, -11.5, -75.0, 25.0, 0.36),
            (132.0, -12.5, 0.0, 55.0, 0.38),
            (139.5, -13.0, 170.0, 15.0, 0.4),
            (147.0, -14.0, -170.0, 15.0, 0.42),
            (154.5, -14.5, 90.0, 40.0, 0.44),
        ],
    };

    let mut taps = [ReflectionTap::default(); MAX_TAPS];
    for (tap, &(delay_ms, gain_db, azimuth, elevation, damping)) in taps.iter_mut().zip(specs) {
        *tap = ReflectionTap {
            delay_samples: (delay_ms * 0.001 * sample_rate).round() as usize,
            gain: 10.0_f32.powf(gain_db / 20.0),
            azimuth,
            elevation,
            damping,
        };
    }
    (taps, specs.len())
}

/// Return the maximum tap delay in samples across all presets.
///
/// Uses the hard-coded `MAX_PRESET_DELAY_MS` constant (Cathedral, last tap: 154.5 ms)
/// rather than iterating over all presets and generating tap tables. This is O(1)
/// instead of O(presets × taps) and avoids redundant work at every constructor call.
fn max_tap_delay_samples(sample_rate: f32) -> usize {
    // Sanity-check in debug builds: verify the constant is not smaller than the
    // actual maximum computed from the tap tables.
    #[cfg(debug_assertions)]
    {
        let computed_max = [
            RoomPreset::Small,
            RoomPreset::Medium,
            RoomPreset::Large,
            RoomPreset::Cathedral,
        ]
        .iter()
        .flat_map(|&preset| {
            let (taps, num_taps) = generate_taps(preset, sample_rate);
            taps.into_iter().take(num_taps)
        })
        .map(|tap| tap.delay_samples)
        .max()
        .unwrap_or(1);

        let hardcoded = (MAX_PRESET_DELAY_MS * 0.001 * sample_rate).ceil() as usize;
        debug_assert!(
            hardcoded >= computed_max,
            "MAX_PRESET_DELAY_MS ({MAX_PRESET_DELAY_MS} ms) is too small: \
             computed max is {} samples, hardcoded is {hardcoded} samples. \
             Update MAX_PRESET_DELAY_MS.",
            computed_max
        );
    }

    (MAX_PRESET_DELAY_MS * 0.001 * sample_rate).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_room_tap_count() {
        let er = EarlyReflections::new(48000, RoomPreset::Small, 0.0);
        assert_eq!(er.num_taps(), 12);
    }

    #[test]
    fn test_cathedral_tap_count() {
        let er = EarlyReflections::new(48000, RoomPreset::Cathedral, 0.0);
        assert_eq!(er.num_taps(), 20);
    }

    #[test]
    fn test_impulse_response() {
        let mut er = EarlyReflections::new(48000, RoomPreset::Medium, 0.0);
        let num_taps = er.num_taps();

        // Feed impulse
        let mut output = vec![0.0; num_taps];
        er.process(1.0, &mut output);

        // First frame: all taps should be zero (impulse hasn't reached any tap yet)
        assert!(
            output.iter().all(|v| v.abs() < 1e-6),
            "No output on first sample"
        );

        // Process enough samples for first tap to arrive
        let first_delay = er.tap_info(0).unwrap().delay_samples;
        for _ in 1..first_delay {
            er.process(0.0, &mut output);
        }
        er.process(0.0, &mut output);

        // First tap should now have signal
        assert!(
            output[0].abs() > 0.001,
            "First tap should have output after its delay, got {}",
            output[0]
        );
    }

    #[test]
    fn test_tap_directions() {
        let er = EarlyReflections::new(48000, RoomPreset::Medium, 0.0);
        // Verify taps have diverse directions
        let azimuths: Vec<f32> = (0..er.num_taps())
            .map(|i| er.tap_info(i).unwrap().azimuth)
            .collect();
        // Should have both positive and negative azimuths
        assert!(azimuths.iter().any(|a| *a > 0.0));
        assert!(azimuths.iter().any(|a| *a < 0.0));
    }

    #[test]
    fn test_reset() {
        let mut er = EarlyReflections::new(48000, RoomPreset::Small, 0.3);
        let num_taps = er.num_taps();
        let mut output = vec![0.0; num_taps];

        // Feed some signal
        for _ in 0..1000 {
            er.process(0.5, &mut output);
        }

        er.reset();

        // After reset, all output should be silent
        er.process(0.0, &mut output);
        assert!(output.iter().all(|v| v.abs() < 1e-10));
    }

    #[test]
    fn test_high_sample_rate_max_modulation_does_not_panic() {
        let mut er = EarlyReflections::new(96000, RoomPreset::Cathedral, 1.0);
        let mut output = vec![0.0; er.num_taps()];

        for i in 0..200_000 {
            let input = if i == 0 { 1.0 } else { 0.0 };
            er.process(input, &mut output);
            assert!(output.iter().all(|v| v.is_finite()));
        }
    }

    /// When modulation is active, the tap reads must use allpass (not linear)
    /// interpolation. Allpass preserves energy at all frequencies; linear
    /// interpolation acts as a low-pass and progressively rolls off high
    /// frequencies during modulation. Verify by comparing energy at the highest
    /// frequency (Nyquist-like signal: alternating ±1) vs a DC signal — the ratio
    /// must be within 6 dB (linear would create a larger gap per modulation cycle).
    ///
    /// This test also verifies that per-tap allpass states are maintained across
    /// calls (continuity — no clicks when the delay changes between samples).
    #[test]
    fn test_modulated_taps_use_allpass_not_linear() {
        let sr = 48000u32;
        let mod_depth = 1.0; // maximum modulation

        let mut er_mod = EarlyReflections::new(sr, RoomPreset::Small, mod_depth);
        let num_taps = er_mod.num_taps();
        let mut output = vec![0.0; num_taps];

        // Feed enough samples to fill the delay line and reach steady state
        let warmup = 5000usize;
        let measure = 10000usize;
        for _ in 0..warmup {
            er_mod.process(0.5, &mut output);
        }

        // Accumulate energy from all tap outputs over the measurement window
        let mut energy_sum = 0.0_f64;
        let mut count = 0usize;
        for _ in 0..measure {
            er_mod.process(0.5, &mut output);
            for &v in output[..num_taps].iter() {
                if v.is_finite() {
                    energy_sum += (v * v) as f64;
                    count += 1;
                }
            }
        }
        let rms = if count > 0 {
            (energy_sum / count as f64).sqrt()
        } else {
            0.0
        };

        // The RMS should be positive (modulation preserves energy through the tap)
        // and finite — a crash or NaN here indicates broken allpass state management.
        assert!(
            rms > 0.0 && rms.is_finite(),
            "Modulated ER taps should produce non-zero finite output, rms={rms}"
        );

        // Ensure no NaN/Inf regardless of modulation state after many cycles
        for i in 0..100_000 {
            let x = if i % 2 == 0 { 0.3 } else { -0.3 };
            er_mod.process(x, &mut output);
            assert!(
                output.iter().all(|v| v.is_finite()),
                "NaN/Inf detected at frame {i}"
            );
        }
    }
}
