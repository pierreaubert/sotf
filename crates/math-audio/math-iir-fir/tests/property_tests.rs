// ============================================================================
// Property-Based Tests for math-iir-fir
// ============================================================================
//
// This module uses proptest to verify invariants of biquad IIR filters, FIR
// filters, and window functions across a wide range of generated parameters.

use math_audio_iir_fir::{
    Biquad, BiquadCoefficients, BiquadFilterType, Fir, FirFilterType, WindowType, generate_window,
};
use proptest::prelude::*;

// ============================================================================
// Strategies
// ============================================================================

fn biquad_filter_type_strategy() -> impl Strategy<Value = BiquadFilterType> {
    prop_oneof![
        Just(BiquadFilterType::Lowpass),
        Just(BiquadFilterType::Highpass),
        Just(BiquadFilterType::HighpassVariableQ),
        Just(BiquadFilterType::Bandpass),
        Just(BiquadFilterType::Peak),
        Just(BiquadFilterType::Notch),
        Just(BiquadFilterType::Lowshelf),
        Just(BiquadFilterType::Highshelf),
        Just(BiquadFilterType::AllPass),
        Just(BiquadFilterType::LowshelfOrf),
        Just(BiquadFilterType::HighshelfOrf),
        Just(BiquadFilterType::PeakMatched),
    ]
}

fn window_type_strategy() -> impl Strategy<Value = WindowType> {
    prop_oneof![
        Just(WindowType::Rectangular),
        Just(WindowType::Hamming),
        Just(WindowType::Hann),
        Just(WindowType::Blackman),
        Just(WindowType::Kaiser),
    ]
}

fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

/// Schur-Cohn stability check for a biquad with normalized a0 = 1.
fn biquad_coefficients_stable(coeffs: &BiquadCoefficients<f64>) -> bool {
    let a1 = coeffs.a1;
    let a2 = coeffs.a2;
    let eps = 1e-9;
    // Poles are inside the unit circle iff:
    //   |a2| < 1   and   |a1| < 1 + a2
    a2.abs() <= 1.0 + eps && a1.abs() <= 1.0 + a2 + eps
}

// ============================================================================
// Biquad Properties
// ============================================================================

proptest! {
    /// INVARIANT: All standard biquad filter types produce stable coefficients
    /// (poles on or inside the unit circle) for typical positive-Q parameters.
    #[test]
    fn biquad_poles_inside_unit_circle(
        filter_type in biquad_filter_type_strategy(),
        freq in 100.0f64..10_000.0f64,
        q in 0.1f64..10.0f64,
        db_gain in -20.0f64..20.0f64,
    ) {
        let bq = Biquad::new(filter_type, freq, 48_000.0, q, db_gain);
        let coeffs = bq.coefficients();
        prop_assert!(
            biquad_coefficients_stable(&coeffs),
            "Unstable coefficients for {:?} at freq={} q={} gain={}: a1={} a2={}",
            filter_type, freq, q, db_gain, coeffs.a1, coeffs.a2
        );
    }

    /// INVARIANT: Processing a finite input buffer always yields finite output.
    #[test]
    fn biquad_process_block_finite(
        filter_type in biquad_filter_type_strategy(),
        freq in 100.0f64..10_000.0f64,
        q in 0.1f64..10.0f64,
        db_gain in -40.0f64..40.0f64,
        input in (-1.0f32..1.0f32).prop_map(|v| vec![v; 64]),
    ) {
        let mut bq = Biquad::<f32>::new(filter_type, freq as f32, 48_000.0f32, q as f32, db_gain as f32);
        let mut buffer = input.clone();
        bq.process_block(&mut buffer);
        prop_assert!(
            buffer.iter().all(|s| s.is_finite()),
            "Non-finite output sample detected for {:?}",
            filter_type
        );
    }

    /// INVARIANT: A Peak filter with 0 dB gain is an identity system (after the
    /// initial two-sample transient). This is a passthrough/identity property.
    #[test]
    fn biquad_peak_unity_gain_passthrough(
        freq in 100.0f64..10_000.0f64,
        q in 0.1f64..10.0f64,
        input in (-1.0f32..1.0f32).prop_map(|v| vec![v; 64]),
    ) {
        let mut bq = Biquad::<f32>::new(BiquadFilterType::Peak, freq as f32, 48_000.0f32, q as f32, 0.0f32);
        let mut buffer = input.clone();
        bq.process_block(&mut buffer);

        let tol = 1e-5;
        for (i, (inp, out)) in input.iter().zip(buffer.iter()).enumerate().skip(2) {
            prop_assert!(
                approx_eq(*out as f64, *inp as f64, tol),
                "Peak 0 dB should pass through at sample {}: expected {} got {}",
                i, inp, out
            );
        }
    }

    /// INVARIANT: Increasing the gain of a peaking filter increases its
    /// magnitude response at the center frequency.
    #[test]
    fn biquad_peak_gain_monotonic_at_center(
        freq in 100.0f64..10_000.0f64,
        q in 0.1f64..10.0f64,
        gain_db in -20.0f64..19.0f64,
    ) {
        let bq_lower = Biquad::new(BiquadFilterType::Peak, freq, 48_000.0, q, gain_db);
        let bq_higher = Biquad::new(BiquadFilterType::Peak, freq, 48_000.0, q, gain_db + 1.0);
        let mag_lower = bq_lower.result(freq);
        let mag_higher = bq_higher.result(freq);
        prop_assert!(
            mag_higher > mag_lower,
            "Increasing peak gain should increase center magnitude: {} dB -> {} dB gave {} -> {}",
            gain_db, gain_db + 1.0, mag_lower, mag_higher
        );
    }

    /// INVARIANT: update_params with the same values leaves the filter
    /// parameters unchanged (set -> get round-trip).
    #[test]
    fn biquad_update_params_roundtrip(
        filter_type in biquad_filter_type_strategy(),
        freq in 100.0f64..10_000.0f64,
        q in 0.1f64..10.0f64,
        db_gain in -20.0f64..20.0f64,
    ) {
        let mut bq = Biquad::new(filter_type, freq, 48_000.0, q, db_gain);
        bq.update_params(filter_type, freq, 48_000.0, q, db_gain);
        prop_assert_eq!(bq.filter_type, filter_type, "filter_type changed after round-trip");
        prop_assert!(
            approx_eq(bq.freq, freq, 1e-12),
            "freq changed after round-trip: {} vs {}",
            bq.freq, freq
        );
        prop_assert!(
            approx_eq(bq.srate, 48_000.0, 1e-12),
            "srate changed after round-trip"
        );
        prop_assert!(
            approx_eq(bq.q, q, 1e-12),
            "q changed after round-trip: {} vs {}",
            bq.q, q
        );
        prop_assert!(
            approx_eq(bq.db_gain, db_gain, 1e-12),
            "db_gain changed after round-trip: {} vs {}",
            bq.db_gain, db_gain
        );
    }

    /// INVARIANT: Coefficient interpolation at t=0 returns self, and at t=1
    /// returns the target (round-trip for lerp).
    #[test]
    fn biquad_coefficients_lerp_roundtrip(
        a1 in -1.9f64..1.9f64,
        a2 in -0.99f64..0.99f64,
        b0 in -2.0f64..2.0f64,
        b1 in -2.0f64..2.0f64,
        b2 in -2.0f64..2.0f64,
        c_b0 in -2.0f64..2.0f64,
        c_b1 in -2.0f64..2.0f64,
        c_b2 in -2.0f64..2.0f64,
        c_a1 in -1.9f64..1.9f64,
        c_a2 in -0.99f64..0.99f64,
    ) {
        let coeffs = BiquadCoefficients { b0, b1, b2, a1, a2 };
        let other = BiquadCoefficients { b0: c_b0, b1: c_b1, b2: c_b2, a1: c_a1, a2: c_a2 };

        let lerped_0 = coeffs.lerp(&other, 0.0);
        prop_assert!(
            approx_eq(lerped_0.b0, coeffs.b0, 1e-12)
                && approx_eq(lerped_0.b1, coeffs.b1, 1e-12)
                && approx_eq(lerped_0.b2, coeffs.b2, 1e-12)
                && approx_eq(lerped_0.a1, coeffs.a1, 1e-12)
                && approx_eq(lerped_0.a2, coeffs.a2, 1e-12),
            "lerp(t=0) should return self"
        );

        let lerped_1 = coeffs.lerp(&other, 1.0);
        prop_assert!(
            approx_eq(lerped_1.b0, other.b0, 1e-12)
                && approx_eq(lerped_1.b1, other.b1, 1e-12)
                && approx_eq(lerped_1.b2, other.b2, 1e-12)
                && approx_eq(lerped_1.a1, other.a1, 1e-12)
                && approx_eq(lerped_1.a2, other.a2, 1e-12),
            "lerp(t=1) should return other"
        );
    }
}

// ============================================================================
// Window Properties
// ============================================================================

proptest! {
    /// INVARIANT: Generated windows are symmetric around their center sample.
    #[test]
    fn generated_window_is_symmetric(
        n in 1usize..33,
        window_type in window_type_strategy(),
        kaiser_beta in 0.0f64..10.0f64,
    ) {
        let beta = if window_type == WindowType::Kaiser { kaiser_beta } else { 0.0 };
        let window = generate_window(n, window_type, beta);
        prop_assert_eq!(window.len(), n, "Window length mismatch");

        let tol = 1e-12;
        for i in 0..n / 2 {
            prop_assert!(
                approx_eq(window[i], window[n - 1 - i], tol),
                "{:?} window not symmetric at {}: {} != {}",
                window_type, i, window[i], window[n - 1 - i]
            );
        }
    }
}

// ============================================================================
// FIR Properties
// ============================================================================

proptest! {
    /// INVARIANT: Windowed-sinc FIR lowpass filters have symmetric taps,
    /// guaranteeing linear phase.
    #[test]
    fn fir_lowpass_coeffs_symmetric(
        n_taps in (1usize..16usize).prop_map(|n| 2 * n + 1),
        cutoff in 200.0f64..10_000.0f64,
        window_type in window_type_strategy(),
    ) {
        let beta = if window_type == WindowType::Kaiser { 5.0 } else { 0.0 };
        let fir = Fir::lowpass(n_taps, cutoff, 48_000.0, window_type, beta);
        let coeffs = fir.coeffs();
        let n = coeffs.len();
        let tol = 1e-9;
        for i in 0..n / 2 {
            prop_assert!(
                approx_eq(coeffs[i], coeffs[n - 1 - i], tol),
                "Lowpass FIR not symmetric at {}: {} != {}",
                i, coeffs[i], coeffs[n - 1 - i]
            );
        }
    }

    /// INVARIANT: Windowed-sinc FIR highpass filters have symmetric taps.
    #[test]
    fn fir_highpass_coeffs_symmetric(
        n_taps in (1usize..16usize).prop_map(|n| 2 * n + 1),
        cutoff in 200.0f64..10_000.0f64,
        window_type in window_type_strategy(),
    ) {
        let beta = if window_type == WindowType::Kaiser { 5.0 } else { 0.0 };
        let fir = Fir::highpass(n_taps, cutoff, 48_000.0, window_type, beta);
        let coeffs = fir.coeffs();
        let n = coeffs.len();
        let tol = 1e-9;
        for i in 0..n / 2 {
            prop_assert!(
                approx_eq(coeffs[i], coeffs[n - 1 - i], tol),
                "Highpass FIR not symmetric at {}: {} != {}",
                i, coeffs[i], coeffs[n - 1 - i]
            );
        }
    }

    /// INVARIANT: Windowed-sinc FIR bandpass filters have symmetric taps.
    #[test]
    fn fir_bandpass_coeffs_symmetric(
        n_taps in (1usize..16usize).prop_map(|n| 2 * n + 1),
        freq_low in 200.0f64..5_000.0f64,
        freq_high in 5_500.0f64..15_000.0f64,
        window_type in window_type_strategy(),
    ) {
        let beta = if window_type == WindowType::Kaiser { 5.0 } else { 0.0 };
        let fir = Fir::bandpass(n_taps, freq_low, freq_high, 48_000.0, window_type, beta);
        let coeffs = fir.coeffs();
        let n = coeffs.len();
        let tol = 1e-9;
        for i in 0..n / 2 {
            prop_assert!(
                approx_eq(coeffs[i], coeffs[n - 1 - i], tol),
                "Bandpass FIR not symmetric at {}: {} != {}",
                i, coeffs[i], coeffs[n - 1 - i]
            );
        }
    }

    /// INVARIANT: Windowed-sinc FIR bandstop filters have symmetric taps.
    #[test]
    fn fir_bandstop_coeffs_symmetric(
        n_taps in (1usize..16usize).prop_map(|n| 2 * n + 1),
        freq_low in 200.0f64..5_000.0f64,
        freq_high in 5_500.0f64..15_000.0f64,
        window_type in window_type_strategy(),
    ) {
        let beta = if window_type == WindowType::Kaiser { 5.0 } else { 0.0 };
        let fir = Fir::bandstop(n_taps, freq_low, freq_high, 48_000.0, window_type, beta);
        let coeffs = fir.coeffs();
        let n = coeffs.len();
        let tol = 1e-9;
        for i in 0..n / 2 {
            prop_assert!(
                approx_eq(coeffs[i], coeffs[n - 1 - i], tol),
                "Bandstop FIR not symmetric at {}: {} != {}",
                i, coeffs[i], coeffs[n - 1 - i]
            );
        }
    }

    /// INVARIANT: Processing a finite input buffer through a FIR filter always
    /// yields finite output.
    #[test]
    fn fir_process_block_finite(
        filter_type in prop_oneof![
            Just(FirFilterType::Lowpass),
            Just(FirFilterType::Highpass),
            Just(FirFilterType::Bandpass),
            Just(FirFilterType::Bandstop),
        ],
        n_taps in (1usize..16usize).prop_map(|n| 2 * n + 1),
        cutoff in 200.0f64..10_000.0f64,
        input in (-1.0f32..1.0f32).prop_map(|v| vec![v; 64]),
    ) {
        let mut fir = match filter_type {
            FirFilterType::Lowpass => Fir::<f32>::lowpass(n_taps, cutoff as f32, 48_000.0f32, WindowType::Hann, 0.0f32),
            FirFilterType::Highpass => Fir::<f32>::highpass(n_taps, cutoff as f32, 48_000.0f32, WindowType::Hann, 0.0f32),
            FirFilterType::Bandpass => Fir::<f32>::bandpass(n_taps, cutoff as f32, (cutoff + 5_000.0f64).min(20_000.0f64) as f32, 48_000.0f32, WindowType::Hann, 0.0f32),
            FirFilterType::Bandstop => Fir::<f32>::bandstop(n_taps, cutoff as f32, (cutoff + 5_000.0f64).min(20_000.0f64) as f32, 48_000.0f32, WindowType::Hann, 0.0f32),
            FirFilterType::Custom => unreachable!("Custom not used in this property"),
        };
        let mut buffer = input.clone();
        fir.process_block(&mut buffer);
        prop_assert!(
            buffer.iter().all(|s| s.is_finite()),
            "Non-finite output sample detected for {:?}",
            filter_type
        );
    }
}
