// ============================================================================
// Property-Based Tests for math-dsp
// ============================================================================
//
// Focus areas:
//   - audio_features utils (mean / std_dev round-trip under shift/scale)
//   - SIMD scale/add identity and finite-output properties
//   - analysis helper monotonicity (find_db_point vs target_db)

use proptest::prelude::*;

use math_audio_dsp::analysis::find_db_point;
use math_audio_dsp::audio_features::utils::{mean, std_deviation};
use math_audio_dsp::simd::{scale_add_simd, scale_add_simd_inplace};

// ============================================================================
// Strategies
// ============================================================================

fn finite_f32_vec() -> impl Strategy<Value = Vec<f32>> {
    prop::collection::vec(-1.0f32..1.0f32, 0..32)
}

fn constant_buffer(len: usize) -> impl Strategy<Value = Vec<f32>> {
    (-1.0f32..1.0f32).prop_map(move |v| vec![v; len])
}

// ============================================================================
// audio_features::utils — mean / std_dev
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// INVARIANT: mean of a constant buffer returns that constant.
    #[test]
    fn mean_of_constant_is_constant(c in -1.0f32..1.0f32, len in 1usize..32) {
        let buf = vec![c; len];
        let m = mean(&buf);
        prop_assert!(
            (m - c).abs() < 1e-5,
            "mean of constant {} over {} samples was {}",
            c, len, m
        );
    }

    /// INVARIANT: std_dev of a constant buffer is zero.
    #[test]
    fn std_dev_of_constant_is_zero(c in -1.0f32..1.0f32, len in 2usize..32) {
        let buf = vec![c; len];
        let s = std_deviation(&buf);
        prop_assert!(
            s.abs() < 1e-5,
            "std_dev of constant {} over {} samples was {}",
            c, len, s
        );
    }

    /// INVARIANT: mean and std_dev produce finite output for finite input.
    #[test]
    fn mean_std_dev_outputs_finite(samples in finite_f32_vec()) {
        let m = mean(&samples);
        let s = std_deviation(&samples);
        prop_assert!(m.is_finite(), "mean produced non-finite value {}", m);
        prop_assert!(s.is_finite() || samples.len() <= 1,
            "std_dev produced non-finite value {}", s);
    }

    /// ROUND-TRIP: shifting every sample by a constant shifts the mean by the
    /// same constant and leaves std_dev unchanged.
    #[test]
    fn shift_preserves_std_dev_and_shifts_mean(
        samples in finite_f32_vec(),
        shift in -1.0f32..1.0f32,
    ) {
        if samples.len() < 2 {
            return Ok(());
        }
        let shifted: Vec<f32> = samples.iter().map(|x| x + shift).collect();
        let m0 = mean(&samples);
        let m1 = mean(&shifted);
        let s0 = std_deviation(&samples);
        let s1 = std_deviation(&shifted);

        prop_assert!(
            (m1 - (m0 + shift)).abs() < 1e-4,
            "shifted mean {} != expected {}", m1, m0 + shift
        );
        prop_assert!(
            (s1 - s0).abs() < 1e-4,
            "std_dev changed after shift: {} -> {}", s0, s1
        );
    }

    /// ROUND-TRIP: centering a buffer zeroes its mean without changing std_dev.
    #[test]
    fn centering_zeroes_mean(samples in finite_f32_vec()) {
        if samples.len() < 2 {
            return Ok(());
        }
        let m = mean(&samples);
        let centered: Vec<f32> = samples.iter().map(|x| x - m).collect();
        let mc = mean(&centered);
        let s0 = std_deviation(&samples);
        let sc = std_deviation(&centered);

        prop_assert!(
            mc.abs() < 1e-4,
            "centered mean was {}", mc
        );
        prop_assert!(
            (sc - s0).abs() < 1e-4,
            "std_dev changed after centering: {} -> {}", s0, sc
        );
    }
}

// ============================================================================
// SIMD scale/add
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// IDENTITY: scale_add_simd_inplace with scale = 1.0 is a passthrough.
    #[test]
    fn scale_add_inplace_identity_1(buffer in constant_buffer(32)) {
        let mut out = buffer.clone();
        scale_add_simd_inplace(&mut out, 1.0);
        for (o, b) in out.iter().zip(buffer.iter()) {
            prop_assert!(
                (o - b).abs() < 1e-6,
                "scale_add_simd_inplace identity mismatch: {} != {}", o, b
            );
        }
    }

    /// IDENTITY: scale_add_simd with scale = 0.0 leaves dst unchanged.
    #[test]
    fn scale_add_zero_scale_leaves_dst(
        dst in constant_buffer(32),
        src in constant_buffer(32),
    ) {
        let mut out = dst.clone();
        scale_add_simd(&mut out, &src, 0.0);
        for (o, d) in out.iter().zip(dst.iter()) {
            prop_assert!(
                (o - d).abs() < 1e-6,
                "scale_add_simd zero-scale mismatch: {} != {}", o, d
            );
        }
    }

    /// FINITE OUTPUT: scale/add SIMD never produces NaN/Inf for finite input.
    #[test]
    fn scale_add_finite_output(
        dst in constant_buffer(32),
        src in constant_buffer(32),
        scale in -10.0f32..10.0f32,
    ) {
        let mut out = dst.clone();
        scale_add_simd(&mut out, &src, scale);
        prop_assert!(
            out.iter().all(|x| x.is_finite()),
            "scale_add_simd produced non-finite output for scale {}",
            scale
        );
    }

    /// FINITE OUTPUT: in-place scale never produces NaN/Inf for finite input.
    #[test]
    fn scale_add_inplace_finite_output(
        mut buffer in constant_buffer(32),
        scale in -10.0f32..10.0f32,
    ) {
        scale_add_simd_inplace(&mut buffer, scale);
        prop_assert!(
            buffer.iter().all(|x| x.is_finite()),
            "scale_add_simd_inplace produced non-finite output for scale {}",
            scale
        );
    }
}

// ============================================================================
// Analysis helper monotonicity
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// MONOTONICITY: For a strictly increasing magnitude curve, a higher
    /// target_dB searched from the start yields a higher (or equal) frequency.
    #[test]
    fn find_db_point_monotonic_in_target(
        mut freqs in prop::collection::vec(20.0f32..20_000.0f32, 4..12),
        base_db in -40.0f32..0.0f32,
        step_db in 0.5f32..3.0f32,
    ) {
        freqs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = freqs.len();
        let mags: Vec<f32> = (0..n).map(|i| base_db + i as f32 * step_db).collect();

        let min_t = mags[0];
        let max_t = mags[n - 1];
        let span = max_t - min_t;
        if span <= 0.0 {
            return Ok(());
        }

        let t1 = min_t + span * 0.25;
        let t2 = min_t + span * 0.75;
        let f1 = find_db_point(&freqs, &mags, t1, true);
        let f2 = find_db_point(&freqs, &mags, t2, true);

        if let (Some(a), Some(b)) = (f1, f2) {
            prop_assert!(
                a <= b,
                "find_db_point not monotonic: target {} -> freq {}, target {} -> freq {}",
                t1, a, t2, b
            );
        }
    }
}
