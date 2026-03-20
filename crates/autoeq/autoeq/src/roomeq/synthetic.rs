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
    assert!(n_points >= 2, "generate_flat_curve requires n_points >= 2, got {}", n_points);
    let log_min = min_freq.log10();
    let log_max = max_freq.log10();
    let freq: Vec<f64> = (0..n_points)
        .map(|i| 10.0_f64.powf(log_min + (log_max - log_min) * i as f64 / (n_points - 1) as f64))
        .collect();

    Curve {
        freq: Array1::from(freq),
        spl: Array1::zeros(n_points),
        phase: None,
    }
}

/// Generate a Harman-style tilt curve (-0.8 dB/octave from 200 Hz reference).
///
/// # Panics
/// Panics if `n_points < 2` (need at least two points for a frequency range).
pub fn generate_harman_tilt_curve(min_freq: f64, max_freq: f64, n_points: usize) -> Curve {
    assert!(n_points >= 2, "generate_harman_tilt_curve requires n_points >= 2, got {}", n_points);
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
        spl = spl + &response;
    }

    Curve {
        freq: curve.freq.clone(),
        spl,
        phase: curve.phase.clone(),
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
            assert!((s - 0.0).abs() < 1e-10, "Flat curve SPL should be 0, got {}", s);
        }

        // Freq range check
        assert!((curve.freq[0] - 20.0).abs() < 0.1);
        assert!((curve.freq[199] - 20000.0).abs() < 1.0);
    }

    #[test]
    fn test_generate_harman_tilt_curve() {
        let curve = generate_harman_tilt_curve(20.0, 20000.0, 200);

        // At 200 Hz (reference), SPL should be 0
        let idx_200 = curve.freq.iter().enumerate()
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
        assert!(max_deviation > 0.1, "Noise should be non-trivial, max deviation: {}", max_deviation);
    }

    #[test]
    fn test_apply_known_eq() {
        let curve = generate_flat_curve(20.0, 20000.0, 200);
        let filter = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 6.0);

        let result = apply_known_eq(&curve, &[filter], 48000.0);

        // At 1000 Hz, the peak filter should add ~6 dB
        let idx_1k = result.freq.iter().enumerate()
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
        let diff: f64 = scenario.degraded_curve.spl.iter()
            .zip(scenario.perfect_curve.spl.iter())
            .map(|(&d, &p)| (d - p).powi(2))
            .sum::<f64>() / scenario.degraded_curve.spl.len() as f64;
        let rms_diff = diff.sqrt();
        assert!(rms_diff > 1.0, "Degraded curve should differ from perfect, RMS diff: {:.2}", rms_diff);
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

        let actual_rms = (noisy.spl.iter().map(|&s| s * s).sum::<f64>() / noisy.spl.len() as f64).sqrt();
        assert!(
            (actual_rms - rms_target).abs() < 0.3,
            "Noise RMS should be ~{}, got {:.3}",
            rms_target,
            actual_rms
        );
    }
}
