use math_audio_iir_fir::{Biquad, BiquadFilterType};

const SRATE: f64 = 48000.0;

#[test]
fn test_allpass_magnitude() {
    let freq = 1000.0;
    let q = 0.707;
    let bq = Biquad::new(BiquadFilterType::AllPass, freq, SRATE, q, 0.0);

    // Magnitude response should be 0 dB (1.0) across the spectrum
    let test_freqs = [20.0, 100.0, 1000.0, 5000.0, 20000.0];
    for &f in &test_freqs {
        let mag_db = bq.log_result(f);
        assert!(
            (mag_db - 0.0).abs() < 1e-10,
            "Magnitude at {}Hz should be 0dB, got {}dB",
            f,
            mag_db
        );

        let mag_linear = bq.result(f);
        assert!(
            (mag_linear - 1.0).abs() < 1e-10,
            "Magnitude at {}Hz should be 1.0, got {}",
            f,
            mag_linear
        );
    }
}

#[test]
fn test_allpass_phase() {
    let freq = 1000.0;
    let q = 1.0;
    let bq = Biquad::new(BiquadFilterType::AllPass, freq, SRATE, q, 0.0);

    // At center frequency, a second-order All-Pass filter has exactly 180 degrees phase shift
    // complex_response returns H(z) = (b0 + b1*z^-1 + b2*z^-2) / (1 + a1*z^-1 + a2*z^-2)
    let resp = bq.complex_response(freq);

    // Phase should be -PI (or PI) at center frequency
    let phase = resp.arg();
    assert!(
        (phase.abs() - std::f64::consts::PI).abs() < 1e-10,
        "Phase at center freq should be PI, got {}",
        phase
    );
}

#[test]
fn test_allpass_try_new() {
    // try_new should work for AllPass
    let result = Biquad::try_new(BiquadFilterType::AllPass, 1000.0, SRATE, 1.0, 0.0);
    assert!(result.is_ok());
}

#[test]
fn test_peq_allpass() {
    let freq = 1000.0;
    let q = 1.0;
    let peq = math_audio_iir_fir::peq_allpass(freq, SRATE, q);
    assert_eq!(peq.len(), 1);
    assert_eq!(peq[0].1.filter_type, BiquadFilterType::AllPass);
    assert_eq!(peq[0].1.freq, freq);
    assert_eq!(peq[0].1.q, q);
}
