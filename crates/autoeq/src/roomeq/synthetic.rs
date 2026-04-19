//! Synthetic speaker curve generation for QA testing.
//!
//! Provides deterministic test scenarios with known ground truth for validating
//! optimization algorithms without relying on real measurement data.

use crate::Curve;
use math_audio_iir_fir::Biquad;
use ndarray::Array1;

/// Generate a flat curve at 0 dB SPL with log-spaced frequency points.
///
/// # Panics
/// Panics if `n_points < 2` (need at least two points for a frequency range).
pub fn generate_flat_curve(min_freq: f64, max_freq: f64, n_points: usize) -> Curve {
    assert!(
        n_points >= 2,
        "generate_flat_curve requires n_points >= 2, got {}",
        n_points
    );
    let log_min = min_freq.log10();
    let log_max = max_freq.log10();
    let freq: Vec<f64> = (0..n_points)
        .map(|i| 10.0_f64.powf(log_min + (log_max - log_min) * i as f64 / (n_points - 1) as f64))
        .collect();

    Curve {
        freq: Array1::from(freq),
        spl: Array1::zeros(n_points),
        phase: None,
        ..Default::default()
    }
}

/// Generate a Harman-style tilt curve (-0.8 dB/octave from 200 Hz reference).
///
/// # Panics
/// Panics if `n_points < 2` (need at least two points for a frequency range).
pub fn generate_harman_tilt_curve(min_freq: f64, max_freq: f64, n_points: usize) -> Curve {
    assert!(
        n_points >= 2,
        "generate_harman_tilt_curve requires n_points >= 2, got {}",
        n_points
    );
    let tilt_db_per_octave = -0.8;
    let reference_freq = 200.0;

    let log_min = min_freq.log10();
    let log_max = max_freq.log10();
    let freq: Vec<f64> = (0..n_points)
        .map(|i| 10.0_f64.powf(log_min + (log_max - log_min) * i as f64 / (n_points - 1) as f64))
        .collect();

    let spl: Vec<f64> = freq
        .iter()
        .map(|&f| tilt_db_per_octave * (f / reference_freq).log2())
        .collect();

    Curve {
        freq: Array1::from(freq),
        spl: Array1::from(spl),
        phase: None,
        ..Default::default()
    }
}

/// Generate a speaker-like rolloff curve: 0 dB above `crossover_freq`, rolling off
/// at `slope_db_per_oct` dB/octave below it (negative slope = highpass rolloff).
///
/// Models a real main speaker that is flat in-band but loses output below its
/// low-frequency limit. Log-spaced frequency points.
pub fn generate_speaker_rolloff_curve(
    min_freq: f64,
    max_freq: f64,
    n_points: usize,
    crossover_freq: f64,
    slope_db_per_oct: f64,
) -> Curve {
    assert!(n_points >= 2);
    let log_min = min_freq.log10();
    let log_max = max_freq.log10();
    let freq: Vec<f64> = (0..n_points)
        .map(|i| 10.0_f64.powf(log_min + (log_max - log_min) * i as f64 / (n_points - 1) as f64))
        .collect();

    let spl: Vec<f64> = freq
        .iter()
        .map(|&f| {
            if f >= crossover_freq {
                0.0
            } else {
                // octaves below crossover (negative value)
                slope_db_per_oct * (crossover_freq / f).log2()
            }
        })
        .collect();

    Curve {
        freq: Array1::from(freq),
        spl: Array1::from(spl),
        phase: None,
        ..Default::default()
    }
}

/// Generate a subwoofer-like rolloff curve: 0 dB below `crossover_freq`, rolling off
/// at `slope_db_per_oct` dB/octave above it (negative slope = lowpass rolloff).
///
/// Models a real subwoofer that is flat in its passband but loses output above its
/// upper limit. Log-spaced frequency points.
pub fn generate_subwoofer_rolloff_curve(
    min_freq: f64,
    max_freq: f64,
    n_points: usize,
    crossover_freq: f64,
    slope_db_per_oct: f64,
) -> Curve {
    assert!(n_points >= 2);
    let log_min = min_freq.log10();
    let log_max = max_freq.log10();
    let freq: Vec<f64> = (0..n_points)
        .map(|i| 10.0_f64.powf(log_min + (log_max - log_min) * i as f64 / (n_points - 1) as f64))
        .collect();

    let spl: Vec<f64> = freq
        .iter()
        .map(|&f| {
            if f <= crossover_freq {
                0.0
            } else {
                // octaves above crossover (negative value)
                slope_db_per_oct * (f / crossover_freq).log2()
            }
        })
        .collect();

    Curve {
        freq: Array1::from(freq),
        spl: Array1::from(spl),
        phase: None,
        ..Default::default()
    }
}

/// Add Gaussian noise (in dB domain) with configurable RMS and deterministic seed.
///
/// Uses a simple xorshift64 PRNG with Box-Muller transform for reproducibility
/// without requiring an external random crate.
pub fn add_noise(curve: &Curve, noise_db_rms: f64, seed: u64) -> Curve {
    let noise = generate_gaussian_noise(curve.spl.len(), noise_db_rms, seed);
    let spl = &curve.spl + &Array1::from(noise);

    Curve {
        freq: curve.freq.clone(),
        spl,
        phase: curve.phase.clone(),
        ..Default::default()
    }
}

/// Apply known biquad filters to a curve (simulates room modes).
///
/// Computes the combined dB response of the given filters at each frequency
/// point and adds it to the SPL.
pub fn apply_known_eq(curve: &Curve, filters: &[Biquad], _sample_rate: f64) -> Curve {
    let mut spl = curve.spl.clone();

    for filter in filters {
        let response = filter.np_log_result(&curve.freq);
        spl += &response;
    }

    Curve {
        freq: curve.freq.clone(),
        spl,
        phase: curve.phase.clone(),
        ..Default::default()
    }
}

/// Full synthetic scenario with known ground truth.
#[derive(Debug, Clone)]
pub struct SyntheticScenario {
    /// Human-readable name for the scenario
    pub name: String,
    /// The original target curve (what we want to achieve)
    pub perfect_curve: Curve,
    /// The degraded measurement (after noise + room modes)
    pub degraded_curve: Curve,
    /// The room modes that were applied
    pub known_modes: Vec<Biquad>,
    /// Pre-mode noise RMS in dB
    pub pre_noise_rms_db: f64,
    /// Post-mode noise RMS in dB
    pub post_noise_rms_db: f64,
}

/// Build a complete test scenario: target → +noise1 → +room_modes → +noise2
pub fn generate_scenario(
    name: &str,
    target: &Curve,
    modes: &[Biquad],
    pre_noise_rms: f64,
    post_noise_rms: f64,
    seed: u64,
    sample_rate: f64,
) -> SyntheticScenario {
    // Step 1: Add pre-mode noise to represent measurement imprecision
    let after_pre_noise = if pre_noise_rms > 0.0 {
        add_noise(target, pre_noise_rms, seed)
    } else {
        target.clone()
    };

    // Step 2: Apply room modes
    let after_modes = if !modes.is_empty() {
        apply_known_eq(&after_pre_noise, modes, sample_rate)
    } else {
        after_pre_noise
    };

    // Step 3: Add post-mode noise (represents measurement noise)
    let degraded = if post_noise_rms > 0.0 {
        add_noise(&after_modes, post_noise_rms, seed.wrapping_add(1000))
    } else {
        after_modes
    };

    SyntheticScenario {
        name: name.to_string(),
        perfect_curve: target.clone(),
        degraded_curve: degraded,
        known_modes: modes.to_vec(),
        pre_noise_rms_db: pre_noise_rms,
        post_noise_rms_db: post_noise_rms,
    }
}

// ---------------------------------------------------------------------------
// Multi-sub scenario generation
// ---------------------------------------------------------------------------

/// Synthetic multi-sub scenario with per-subwoofer measurements.
#[derive(Debug, Clone)]
pub struct MultiSubSyntheticScenario {
    pub name: String,
    /// Perfect flat bass target (what we want the combined response to be)
    pub perfect_curve: Curve,
    /// Per-subwoofer degraded measurements (with phase from simulated delays)
    pub sub_curves: Vec<Curve>,
    /// Number of subwoofers
    pub n_subs: usize,
    /// Room modes shared by all subs (room resonances)
    pub shared_modes: Vec<Biquad>,
    /// Per-sub unique modes (placement-dependent resonances)
    pub per_sub_modes: Vec<Vec<Biquad>>,
}

/// Generate a bass-range curve with simulated propagation delay phase.
///
/// Creates a log-spaced curve from `min_freq` to `max_freq` with flat SPL
/// and linear phase corresponding to a propagation delay.
pub fn generate_sub_curve_with_phase(
    min_freq: f64,
    max_freq: f64,
    n_points: usize,
    delay_ms: f64,
) -> Curve {
    let base = generate_flat_curve(min_freq, max_freq, n_points);
    let delay_s = delay_ms / 1000.0;

    // Linear phase from propagation delay: φ = -360 * f * τ degrees
    let phase: Vec<f64> = base.freq.iter().map(|&f| -360.0 * f * delay_s).collect();

    Curve {
        freq: base.freq,
        spl: base.spl,
        phase: Some(Array1::from(phase)),
        ..Default::default()
    }
}

/// Build a multi-sub synthetic scenario.
///
/// Each subwoofer gets:
/// - The same shared room modes (common room resonances)
/// - Unique per-sub modes (simulates different sub placement exciting different modes)
/// - Different propagation delay (simulates different distance to listening position)
/// - Independent measurement noise (different seed per sub)
///
/// # Arguments
/// * `name` - Scenario name
/// * `n_subs` - Number of subwoofers
/// * `shared_modes` - Room modes applied to all subs
/// * `per_sub_modes` - Per-sub unique modes (len must equal n_subs, or empty for none)
/// * `delays_ms` - Per-sub propagation delay in ms (len must equal n_subs)
/// * `noise_rms` - Measurement noise RMS in dB
/// * `seed` - Base seed for deterministic generation
/// * `sample_rate` - Sample rate for biquad computation
pub fn generate_multisub_scenario(
    name: &str,
    n_subs: usize,
    shared_modes: &[Biquad],
    per_sub_modes: &[Vec<Biquad>],
    delays_ms: &[f64],
    noise_rms: f64,
    seed: u64,
    sample_rate: f64,
) -> MultiSubSyntheticScenario {
    assert_eq!(delays_ms.len(), n_subs);
    assert!(per_sub_modes.is_empty() || per_sub_modes.len() == n_subs);

    let min_freq = 20.0;
    let max_freq = 200.0;
    let n_points = 100;

    let perfect = generate_flat_curve(min_freq, max_freq, n_points);

    let mut sub_curves = Vec::with_capacity(n_subs);
    let mut all_per_sub = Vec::with_capacity(n_subs);

    for i in 0..n_subs {
        // Start with flat curve + propagation delay phase
        let base = generate_sub_curve_with_phase(min_freq, max_freq, n_points, delays_ms[i]);

        // Apply shared room modes
        let after_shared = if !shared_modes.is_empty() {
            apply_known_eq(&base, shared_modes, sample_rate)
        } else {
            base
        };

        // Apply per-sub unique modes
        let unique = if !per_sub_modes.is_empty() {
            &per_sub_modes[i]
        } else {
            &vec![]
        };
        let after_unique = if !unique.is_empty() {
            apply_known_eq(&after_shared, unique, sample_rate)
        } else {
            after_shared
        };

        // Add measurement noise (unique seed per sub)
        let noisy = if noise_rms > 0.0 {
            add_noise(&after_unique, noise_rms, seed.wrapping_add(i as u64 * 1000))
        } else {
            after_unique
        };

        sub_curves.push(noisy);
        all_per_sub.push(unique.clone());
    }

    MultiSubSyntheticScenario {
        name: name.to_string(),
        perfect_curve: perfect,
        sub_curves,
        n_subs,
        shared_modes: shared_modes.to_vec(),
        per_sub_modes: all_per_sub,
    }
}

// ---------------------------------------------------------------------------
// Cardioid scenario generation
// ---------------------------------------------------------------------------

/// Synthetic cardioid subwoofer scenario (front + rear sub with separation).
#[derive(Debug, Clone)]
pub struct CardioidSyntheticScenario {
    pub name: String,
    pub perfect_curve: Curve,
    pub front_curve: Curve,
    pub rear_curve: Curve,
    pub separation_meters: f64,
}

/// Generate a synthetic cardioid subwoofer scenario.
///
/// The front and rear subs share the same room modes but have different
/// propagation delays (rear delay = separation / 343 m/s). Both get
/// independent measurement noise.
pub fn generate_cardioid_scenario(
    name: &str,
    shared_modes: &[Biquad],
    separation_meters: f64,
    noise_rms: f64,
    seed: u64,
    sample_rate: f64,
) -> CardioidSyntheticScenario {
    let min_freq = 20.0;
    let max_freq = 200.0;
    let n_points = 100;

    let perfect = generate_flat_curve(min_freq, max_freq, n_points);
    let rear_delay_ms = separation_meters / 343.0 * 1000.0;

    // Front sub: 0 ms delay
    let front_base = generate_sub_curve_with_phase(min_freq, max_freq, n_points, 0.0);
    let front_eq = if !shared_modes.is_empty() {
        apply_known_eq(&front_base, shared_modes, sample_rate)
    } else {
        front_base
    };
    let front = if noise_rms > 0.0 {
        add_noise(&front_eq, noise_rms, seed)
    } else {
        front_eq
    };

    // Rear sub: delay from physical separation
    let rear_base = generate_sub_curve_with_phase(min_freq, max_freq, n_points, rear_delay_ms);
    let rear_eq = if !shared_modes.is_empty() {
        apply_known_eq(&rear_base, shared_modes, sample_rate)
    } else {
        rear_base
    };
    let rear = if noise_rms > 0.0 {
        add_noise(&rear_eq, noise_rms, seed.wrapping_add(1000))
    } else {
        rear_eq
    };

    CardioidSyntheticScenario {
        name: name.to_string(),
        perfect_curve: perfect,
        front_curve: front,
        rear_curve: rear,
        separation_meters,
    }
}

// ---------------------------------------------------------------------------
// DBA scenario generation
// ---------------------------------------------------------------------------

/// Synthetic Double Bass Array scenario (front array + rear array).
#[derive(Debug, Clone)]
pub struct DbaSyntheticScenario {
    pub name: String,
    pub perfect_curve: Curve,
    pub front_curves: Vec<Curve>,
    pub rear_curves: Vec<Curve>,
}

/// Generate a synthetic DBA scenario.
///
/// Front subs are at ~0 ms delay, rear subs are delayed (simulating room depth).
/// Both arrays share room modes but get independent noise.
pub fn generate_dba_scenario(
    name: &str,
    n_front: usize,
    n_rear: usize,
    shared_modes: &[Biquad],
    rear_delay_ms: f64,
    noise_rms: f64,
    seed: u64,
    sample_rate: f64,
) -> DbaSyntheticScenario {
    let min_freq = 20.0;
    let max_freq = 200.0;
    let n_points = 100;

    let perfect = generate_flat_curve(min_freq, max_freq, n_points);

    let mut front_curves = Vec::with_capacity(n_front);
    for i in 0..n_front {
        let delay = i as f64 * 0.5; // slight spread within front array
        let base = generate_sub_curve_with_phase(min_freq, max_freq, n_points, delay);
        let after_eq = if !shared_modes.is_empty() {
            apply_known_eq(&base, shared_modes, sample_rate)
        } else {
            base
        };
        let noisy = if noise_rms > 0.0 {
            add_noise(&after_eq, noise_rms, seed.wrapping_add(i as u64 * 100))
        } else {
            after_eq
        };
        front_curves.push(noisy);
    }

    let mut rear_curves = Vec::with_capacity(n_rear);
    for i in 0..n_rear {
        let delay = rear_delay_ms + i as f64 * 0.5;
        let base = generate_sub_curve_with_phase(min_freq, max_freq, n_points, delay);
        let after_eq = if !shared_modes.is_empty() {
            apply_known_eq(&base, shared_modes, sample_rate)
        } else {
            base
        };
        let noisy = if noise_rms > 0.0 {
            add_noise(
                &after_eq,
                noise_rms,
                seed.wrapping_add(500 + i as u64 * 100),
            )
        } else {
            after_eq
        };
        rear_curves.push(noisy);
    }

    DbaSyntheticScenario {
        name: name.to_string(),
        perfect_curve: perfect,
        front_curves,
        rear_curves,
    }
}

// ---------------------------------------------------------------------------
// Multi-channel helpers
// ---------------------------------------------------------------------------

/// Generate a full-range degraded channel curve from a base target.
///
/// Applies channel-specific room modes, propagation delay phase, and noise.
/// Useful for generating per-speaker synthetic measurements in multi-channel layouts.
pub fn generate_channel_curve(
    base: &Curve,
    channel_modes: &[Biquad],
    delay_ms: f64,
    noise_rms: f64,
    seed: u64,
    sample_rate: f64,
) -> Curve {
    // Add phase from propagation delay
    let delay_s = delay_ms / 1000.0;
    let phase: Vec<f64> = base.freq.iter().map(|&f| -360.0 * f * delay_s).collect();
    let with_phase = Curve {
        freq: base.freq.clone(),
        spl: base.spl.clone(),
        phase: Some(Array1::from(phase)),
        ..Default::default()
    };

    // Apply room modes
    let after_eq = if !channel_modes.is_empty() {
        apply_known_eq(&with_phase, channel_modes, sample_rate)
    } else {
        with_phase
    };

    // Add noise
    if noise_rms > 0.0 {
        add_noise(&after_eq, noise_rms, seed)
    } else {
        after_eq
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Xorshift64 PRNG — simple, fast, deterministic.
fn xorshift64(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// Generate Gaussian noise samples using Box-Muller transform.
fn generate_gaussian_noise(n: usize, rms: f64, seed: u64) -> Vec<f64> {
    let mut state = seed;
    if state == 0 {
        state = 0xdeadbeef;
    }
    let mut samples = Vec::with_capacity(n);

    while samples.len() < n {
        // Generate two uniform [0,1) samples
        let u1 = (xorshift64(&mut state) as f64) / (u64::MAX as f64);
        let u2 = (xorshift64(&mut state) as f64) / (u64::MAX as f64);

        // Box-Muller transform
        let u1_clamped = u1.max(1e-15); // avoid log(0)
        let r = (-2.0 * u1_clamped.ln()).sqrt();
        let theta = 2.0 * std::f64::consts::PI * u2;

        samples.push(r * theta.cos() * rms);
        if samples.len() < n {
            samples.push(r * theta.sin() * rms);
        }
    }

    samples.truncate(n);
    samples
}

#[cfg(test)]
mod tests {
    use super::*;
    use math_audio_iir_fir::BiquadFilterType;

    #[test]
    fn test_generate_flat_curve() {
        let curve = generate_flat_curve(20.0, 20000.0, 200);
        assert_eq!(curve.freq.len(), 200);
        assert_eq!(curve.spl.len(), 200);
        assert!(curve.phase.is_none());

        // All SPL should be 0
        for &s in curve.spl.iter() {
            assert!(
                (s - 0.0).abs() < 1e-10,
                "Flat curve SPL should be 0, got {}",
                s
            );
        }

        // Freq range check
        assert!((curve.freq[0] - 20.0).abs() < 0.1);
        assert!((curve.freq[199] - 20000.0).abs() < 1.0);
    }

    #[test]
    fn test_generate_harman_tilt_curve() {
        let curve = generate_harman_tilt_curve(20.0, 20000.0, 200);

        // At 200 Hz (reference), SPL should be 0
        let idx_200 = curve
            .freq
            .iter()
            .enumerate()
            .min_by_key(|&(_, &f)| ((f - 200.0).abs() * 1000.0) as i64)
            .map(|(i, _)| i)
            .unwrap();
        assert!(
            curve.spl[idx_200].abs() < 0.5,
            "SPL at 200Hz should be ~0, got {:.2}",
            curve.spl[idx_200]
        );

        // At higher freqs, SPL should be negative (downward tilt)
        let idx_high = curve.freq.len() - 1;
        assert!(
            curve.spl[idx_high] < -3.0,
            "SPL at high freq should be significantly negative, got {:.2}",
            curve.spl[idx_high]
        );
    }

    #[test]
    fn test_add_noise_deterministic() {
        let curve = generate_flat_curve(20.0, 20000.0, 100);
        let noisy1 = add_noise(&curve, 1.0, 42);
        let noisy2 = add_noise(&curve, 1.0, 42);

        // Same seed → same result
        for i in 0..noisy1.spl.len() {
            assert!(
                (noisy1.spl[i] - noisy2.spl[i]).abs() < 1e-10,
                "Same seed should produce identical noise"
            );
        }

        // Noise should be non-zero
        let max_deviation = noisy1.spl.iter().map(|&s| s.abs()).fold(0.0_f64, f64::max);
        assert!(
            max_deviation > 0.1,
            "Noise should be non-trivial, max deviation: {}",
            max_deviation
        );
    }

    #[test]
    fn test_apply_known_eq() {
        let curve = generate_flat_curve(20.0, 20000.0, 200);
        let filter = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);

        let result = apply_known_eq(&curve, &[filter], 48000.0);

        // At 1000 Hz, the peak filter should add ~6 dB
        let idx_1k = result
            .freq
            .iter()
            .enumerate()
            .min_by_key(|&(_, &f)| ((f - 1000.0).abs() * 1000.0) as i64)
            .map(|(i, _)| i)
            .unwrap();

        assert!(
            (result.spl[idx_1k] - 6.0).abs() < 1.0,
            "Peak filter at 1kHz should add ~6dB, got {:.2}",
            result.spl[idx_1k]
        );

        // Far from 1000 Hz, effect should be minimal
        assert!(
            result.spl[0].abs() < 1.0,
            "Low freq should be near 0dB, got {:.2}",
            result.spl[0]
        );
    }

    #[test]
    fn test_generate_scenario() {
        let target = generate_flat_curve(20.0, 20000.0, 200);
        let modes = vec![
            Biquad::new(BiquadFilterType::Peak, 100.0, 48000.0, 4.0, -8.0),
            Biquad::new(BiquadFilterType::Peak, 200.0, 48000.0, 3.0, 5.0),
        ];

        let scenario = generate_scenario("test", &target, &modes, 0.5, 0.5, 42, 48000.0);

        assert_eq!(scenario.name, "test");
        assert_eq!(scenario.known_modes.len(), 2);

        // Degraded curve should differ from perfect
        let diff: f64 = scenario
            .degraded_curve
            .spl
            .iter()
            .zip(scenario.perfect_curve.spl.iter())
            .map(|(&d, &p)| (d - p).powi(2))
            .sum::<f64>()
            / scenario.degraded_curve.spl.len() as f64;
        let rms_diff = diff.sqrt();
        assert!(
            rms_diff > 1.0,
            "Degraded curve should differ from perfect, RMS diff: {:.2}",
            rms_diff
        );
    }

    #[test]
    #[should_panic(expected = "n_points >= 2")]
    fn test_generate_flat_curve_panics_on_single_point() {
        generate_flat_curve(20.0, 20000.0, 1);
    }

    #[test]
    #[should_panic(expected = "n_points >= 2")]
    fn test_generate_harman_tilt_curve_panics_on_zero_points() {
        generate_harman_tilt_curve(20.0, 20000.0, 0);
    }

    #[test]
    fn test_noise_rms_approximate() {
        // Verify that the noise generator approximately achieves the requested RMS
        let curve = generate_flat_curve(20.0, 20000.0, 10000);
        let rms_target = 2.0;
        let noisy = add_noise(&curve, rms_target, 12345);

        let actual_rms =
            (noisy.spl.iter().map(|&s| s * s).sum::<f64>() / noisy.spl.len() as f64).sqrt();
        assert!(
            (actual_rms - rms_target).abs() < 0.3,
            "Noise RMS should be ~{}, got {:.3}",
            rms_target,
            actual_rms
        );
    }

    #[test]
    fn test_generate_sub_curve_with_phase() {
        let curve = generate_sub_curve_with_phase(20.0, 200.0, 50, 2.0);
        assert_eq!(curve.freq.len(), 50);
        assert!(curve.phase.is_some());

        let phase = curve.phase.unwrap();
        // Phase at 100 Hz with 2 ms delay: -360 * 100 * 0.002 = -72 degrees
        let idx_100 = curve
            .freq
            .iter()
            .enumerate()
            .min_by_key(|&(_, &f)| ((f - 100.0).abs() * 1000.0) as i64)
            .map(|(i, _)| i)
            .unwrap();
        assert!(
            (phase[idx_100] - (-72.0)).abs() < 5.0,
            "Phase at 100 Hz with 2ms delay should be ~-72°, got {:.1}°",
            phase[idx_100]
        );

        // Phase should become more negative at higher frequencies
        assert!(phase[phase.len() - 1] < phase[0]);
    }

    #[test]
    fn test_generate_multisub_scenario_basic() {
        let shared = vec![Biquad::new(
            BiquadFilterType::Peak,
            60.0,
            48000.0,
            4.0,
            -6.0,
        )];
        let scenario = generate_multisub_scenario(
            "test_2sub",
            2,
            &shared,
            &[],         // no per-sub modes
            &[0.0, 2.0], // sub delays
            0.5,
            42,
            48000.0,
        );

        assert_eq!(scenario.n_subs, 2);
        assert_eq!(scenario.sub_curves.len(), 2);
        assert_eq!(scenario.shared_modes.len(), 1);

        // Both subs should have phase data
        for (i, sub) in scenario.sub_curves.iter().enumerate() {
            assert!(sub.phase.is_some(), "Sub {} should have phase data", i);
            assert_eq!(sub.freq.len(), 100);
        }

        // Sub 0 (0ms delay) and sub 1 (2ms delay) should have different phase
        let p0 = scenario.sub_curves[0].phase.as_ref().unwrap();
        let p1 = scenario.sub_curves[1].phase.as_ref().unwrap();
        let phase_diff: f64 = p0
            .iter()
            .zip(p1.iter())
            .map(|(&a, &b)| (a - b).abs())
            .sum::<f64>()
            / p0.len() as f64;
        assert!(
            phase_diff > 1.0,
            "Different delays should produce different phase"
        );
    }

    #[test]
    fn test_generate_multisub_scenario_with_per_sub_modes() {
        let shared = vec![Biquad::new(BiquadFilterType::Peak, 80.0, 48000.0, 3.0, 5.0)];
        let per_sub = vec![
            vec![Biquad::new(
                BiquadFilterType::Peak,
                50.0,
                48000.0,
                4.0,
                -4.0,
            )],
            vec![Biquad::new(
                BiquadFilterType::Peak,
                120.0,
                48000.0,
                4.0,
                -3.0,
            )],
        ];
        let scenario = generate_multisub_scenario(
            "test_per_sub",
            2,
            &shared,
            &per_sub,
            &[0.0, 3.0],
            0.3,
            99,
            48000.0,
        );

        assert_eq!(scenario.per_sub_modes.len(), 2);
        assert_eq!(scenario.per_sub_modes[0].len(), 1);
        assert_eq!(scenario.per_sub_modes[1].len(), 1);

        // Subs should have different SPL profiles due to different unique modes
        let spl_diff: f64 = scenario.sub_curves[0]
            .spl
            .iter()
            .zip(scenario.sub_curves[1].spl.iter())
            .map(|(&a, &b)| (a - b).abs())
            .sum::<f64>()
            / scenario.sub_curves[0].spl.len() as f64;
        assert!(
            spl_diff > 0.5,
            "Per-sub modes should cause SPL differences, got {:.2}",
            spl_diff
        );
    }

    #[test]
    fn test_generate_multisub_scenario_deterministic() {
        let shared = vec![Biquad::new(
            BiquadFilterType::Peak,
            60.0,
            48000.0,
            4.0,
            -6.0,
        )];
        let s1 = generate_multisub_scenario("a", 2, &shared, &[], &[0.0, 2.0], 0.5, 42, 48000.0);
        let s2 = generate_multisub_scenario("a", 2, &shared, &[], &[0.0, 2.0], 0.5, 42, 48000.0);

        // Same seeds → identical results
        for i in 0..2 {
            for j in 0..s1.sub_curves[i].spl.len() {
                assert!(
                    (s1.sub_curves[i].spl[j] - s2.sub_curves[i].spl[j]).abs() < 1e-10,
                    "Same seed should produce identical curves"
                );
            }
        }
    }

    #[test]
    fn test_generate_cardioid_scenario() {
        let modes = vec![Biquad::new(
            BiquadFilterType::Peak,
            60.0,
            48000.0,
            3.0,
            -5.0,
        )];
        let scenario = generate_cardioid_scenario("card", &modes, 1.0, 0.3, 42, 48000.0);

        assert!(scenario.front_curve.phase.is_some());
        assert!(scenario.rear_curve.phase.is_some());
        assert!((scenario.separation_meters - 1.0).abs() < 0.01);

        // Rear should have different phase due to delay from separation
        let fp = scenario.front_curve.phase.as_ref().unwrap();
        let rp = scenario.rear_curve.phase.as_ref().unwrap();
        let phase_diff: f64 = fp
            .iter()
            .zip(rp.iter())
            .map(|(&a, &b)| (a - b).abs())
            .sum::<f64>()
            / fp.len() as f64;
        assert!(
            phase_diff > 1.0,
            "front/rear should have different phase from delay"
        );
    }

    #[test]
    fn test_generate_dba_scenario() {
        let modes = vec![Biquad::new(BiquadFilterType::Peak, 80.0, 48000.0, 4.0, 5.0)];
        let scenario = generate_dba_scenario("dba", 2, 2, &modes, 10.0, 0.3, 42, 48000.0);

        assert_eq!(scenario.front_curves.len(), 2);
        assert_eq!(scenario.rear_curves.len(), 2);

        // All curves should have phase data
        for c in &scenario.front_curves {
            assert!(c.phase.is_some());
        }
        for c in &scenario.rear_curves {
            assert!(c.phase.is_some());
        }

        // Rear should have significantly more phase (larger delay)
        let front_max_phase = scenario.front_curves[0]
            .phase
            .as_ref()
            .unwrap()
            .iter()
            .map(|p| p.abs())
            .fold(0.0_f64, f64::max);
        let rear_max_phase = scenario.rear_curves[0]
            .phase
            .as_ref()
            .unwrap()
            .iter()
            .map(|p| p.abs())
            .fold(0.0_f64, f64::max);
        assert!(
            rear_max_phase > front_max_phase,
            "rear ({:.1}) should have more phase than front ({:.1})",
            rear_max_phase,
            front_max_phase,
        );
    }

    #[test]
    fn test_generate_channel_curve() {
        let base = generate_flat_curve(20.0, 20000.0, 200);
        let modes = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            48000.0,
            2.0,
            6.0,
        )];
        let result = generate_channel_curve(&base, &modes, 1.0, 0.5, 42, 48000.0);

        assert_eq!(result.freq.len(), 200);
        assert!(
            result.phase.is_some(),
            "channel curve should have phase from delay"
        );

        // Should have the room mode applied
        let idx_1k = result
            .freq
            .iter()
            .enumerate()
            .min_by_key(|&(_, &f)| ((f - 1000.0).abs() * 1000.0) as i64)
            .map(|(i, _)| i)
            .unwrap();
        assert!(
            result.spl[idx_1k] > 3.0,
            "should have mode boost at 1kHz, got {:.1}",
            result.spl[idx_1k]
        );
    }
}
