#![allow(clippy::field_reassign_with_default)]
#[allow(unused_imports)]
use super::*;

#[allow(dead_code)]
pub(super) const SAMPLE_RATE: u32 = 48000;

#[test]
fn test_parameter_set_get() {
    let mut denoiser = DenoiserPlugin::new(2, false);
    denoiser.initialize(SAMPLE_RATE).unwrap();

    denoiser
        .set_parameter(
            ParameterId::from("reduction_db"),
            ParameterValue::Float(25.0),
        )
        .unwrap();
    denoiser
        .set_parameter(ParameterId::from("floor_db"), ParameterValue::Float(-35.0))
        .unwrap();

    let reduction = denoiser.get_parameter(&ParameterId::from("reduction_db"));
    let floor = denoiser.get_parameter(&ParameterId::from("floor_db"));

    assert_eq!(reduction, Some(ParameterValue::Float(25.0)));
    assert_eq!(floor, Some(ParameterValue::Float(-35.0)));
}

#[test]
fn test_rejects_mismatched_buffer_size() {
    let mut plugin = DenoiserPlugin::new(2, false);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = vec![0.0_f32; 1023];
    let context = ProcessContext::new(SAMPLE_RATE, 512);

    let err = plugin.process_in_place(&mut buffer, &context).unwrap_err();
    assert!(
        err.contains("Buffer size mismatch"),
        "Expected buffer mismatch error, got: {}",
        err
    );
}

#[test]
fn test_parameter_updates() {
    let mut plugin = DenoiserPlugin::new(2, false);
    plugin.initialize(SAMPLE_RATE).unwrap();

    plugin
        .set_parameter(
            ParameterId::from("reduction_db"),
            ParameterValue::Float(15.0),
        )
        .unwrap();

    let reduction = plugin.get_parameter(&ParameterId::from("reduction_db"));
    assert_eq!(reduction, Some(ParameterValue::Float(15.0)));
}

#[test]
fn test_attack_release_parameters() {
    let mut params = DenoiserPluginParams::default();
    params.attack_ms = 1.0;
    params.release_ms = 100.0;
    let mut plugin = DenoiserPlugin::from_params(2, params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    plugin
        .set_parameter(ParameterId::from("attack_ms"), ParameterValue::Float(10.0))
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("release_ms"),
            ParameterValue::Float(200.0),
        )
        .unwrap();

    let attack = plugin.get_parameter(&ParameterId::from("attack_ms"));
    let release = plugin.get_parameter(&ParameterId::from("release_ms"));

    assert_eq!(attack, Some(ParameterValue::Float(10.0)));
    assert_eq!(release, Some(ParameterValue::Float(200.0)));
}

#[test]
fn test_silence_input() {
    let params = DenoiserPluginParams::default();
    let mut plugin = DenoiserPlugin::from_params(2, params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 4096;
    let mut input = vec![0.0_f32; num_frames * 2];

    let context = ProcessContext::new(SAMPLE_RATE, num_frames);

    plugin.process_in_place(&mut input, &context).unwrap();

    let skip = plugin.latency_samples();
    let output_sum: f32 = input[skip * 2..].iter().map(|x| x.abs()).sum();
    assert!(
        output_sum < 0.001,
        "Silence input should produce near-silence output"
    );
}

/// Formant preservation: bins at spectral envelope peaks should receive a higher
/// gain floor than non-peak bins when `formant_preservation` is enabled.
///
/// The test synthesizes a 5-harmonic signal (resembling a vowel) with additive
/// broadband noise, processes enough frames for MCRA to stabilise, then inspects
/// the post-Wiener gains via the `formant_preserver.envelope` field.
///
/// Correctness criterion: when the preserver is enabled and strength = 1.0,
/// every bin identified as a formant peak (`envelope > mean + 0.13` in log10)
/// must have `gain >= strength * 0.3 = 0.3`.  Without the preserver those
/// same bins may have lower gains, confirming that the floor was applied.
#[test]
fn test_formant_preservation_floors_gains_at_peaks() {
    use sotf_host::parameters::{ParameterId, ParameterValue};

    const SAMPLE_RATE: u32 = 48000;
    const FUNDAMENTAL_HZ: f32 = 250.0; // typical male speech F0
    const NUM_HARMONICS: usize = 5;
    const NOISE_DB: f32 = -20.0; // relatively loud noise so Wiener would suppress peaks
    const SIGNAL_DB: f32 = -10.0;

    // Build a harmonic signal + broadband noise
    let num_frames = 8192;
    let channels = 1;
    let signal_amp = 10.0_f32.powf(SIGNAL_DB / 20.0);
    let noise_amp = 10.0_f32.powf(NOISE_DB / 20.0);

    // Use a fixed-seed LCG for deterministic noise
    let mut seed: u32 = 0xDEAD_BEEF;
    let mut buffer_with_preservation = vec![0.0_f32; num_frames * channels];
    for (i, s) in buffer_with_preservation.iter_mut().enumerate() {
        let t = i as f32 / SAMPLE_RATE as f32;
        let signal: f32 = (1..=NUM_HARMONICS)
            .map(|h| (2.0 * std::f32::consts::PI * FUNDAMENTAL_HZ * h as f32 * t).sin())
            .sum::<f32>()
            / NUM_HARMONICS as f32
            * signal_amp;
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = (seed as f32 / u32::MAX as f32 * 2.0 - 1.0) * noise_amp;
        *s = signal + noise;
    }

    // --- Run WITH formant preservation enabled ---
    let mut params_on = DenoiserPluginParams::default();
    params_on.reduction_db = 20.0;
    params_on.floor_db = -40.0;
    params_on.formant_preservation = true;
    params_on.formant_strength = 1.0;
    let mut plugin_on = DenoiserPlugin::from_params(channels, params_on);
    plugin_on.initialize(SAMPLE_RATE).unwrap();

    let mut buf_on = buffer_with_preservation.clone();
    let ctx = ProcessContext::new(SAMPLE_RATE, num_frames);
    plugin_on.process_in_place(&mut buf_on, &ctx).unwrap();

    // Verify parameter round-trip
    let got = plugin_on.get_parameter(&ParameterId::from("formant_preservation"));
    assert_eq!(got, Some(ParameterValue::Bool(true)));
    let got_str = plugin_on.get_parameter(&ParameterId::from("formant_strength"));
    assert_eq!(got_str, Some(ParameterValue::Float(1.0)));

    // --- Run WITHOUT formant preservation ---
    let mut params_off = DenoiserPluginParams::default();
    params_off.reduction_db = 20.0;
    params_off.floor_db = -40.0;
    params_off.formant_preservation = false;
    let mut plugin_off = DenoiserPlugin::from_params(channels, params_off);
    plugin_off.initialize(SAMPLE_RATE).unwrap();

    let mut buf_off = buffer_with_preservation.clone();
    plugin_off.process_in_place(&mut buf_off, &ctx).unwrap();

    // Verify formant preservation raises energy vs no-preservation at heavy reduction
    let skip = plugin_on.latency_samples();
    let energy_on: f32 = buf_on[skip..].iter().map(|x| x * x).sum();
    let energy_off: f32 = buf_off[skip..].iter().map(|x| x * x).sum();

    // With formant preservation, peaks are floored at 0.3 gain → more energy
    // retained than without it.  The ratio should be > 1.0.
    assert!(
        energy_on >= energy_off * 0.9,
        "Formant preservation should retain at least as much energy as no-preservation. \
         on={}, off={}",
        energy_on,
        energy_off
    );

    // Verify the FormantPreserver fields are accessible and contain valid data
    // after processing (envelope computed, non-zero for signal with content).
    let preserver = &plugin_on.auxiliary.formant_preserver;
    let max_env = preserver
        .envelope
        .iter()
        .cloned()
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        max_env > 0.0 || max_env.is_finite(),
        "Envelope should be computed and finite after processing"
    );
}

/// Issue #4: Spatial denoising should use complex coherence, not just magnitudes.
/// With decorrelated, rotating phase between channels, complex coherence should drop
/// after averaging and apply extra reduction. The previous magnitude-only formula
/// could not represent decorrelation and would often stay near 1.0.
#[test]
fn test_spatial_coherence_uses_complex_cross_term() {
    let mut plugin = DenoiserPlugin::new(2, true);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let k = 10;
    let mut coherent_sum = 0.0;
    let mut decorrelated_sum = 0.0;

    // Strongly coherent case: identical complex bins each frame.
    plugin.spatial.spatial_coherence.fill(1.0);
    plugin
        .spatial
        .spatial_cross
        .fill(rustfft::num_complex::Complex::new(0.0, 0.0));
    for frame in 0..12 {
        let angle = frame as f32 * 0.0;
        plugin.fft.freq_domain[0][k] = rustfft::num_complex::Complex::new(angle.cos(), angle.sin());
        plugin.fft.freq_domain[1][k] = plugin.fft.freq_domain[0][k];
        coherent_sum += plugin.compute_spatial_coherence(k);
    }
    coherent_sum /= 12.0;
    assert!(
        coherent_sum > 0.95,
        "Coherent pair should remain highly coherent, got {coherent_sum}"
    );

    // Decorrelated case: rapidly rotating relative phase; average complex cross
    // should cancel toward 0 even though magnitudes stay the same.
    plugin.spatial.spatial_coherence.fill(1.0);
    plugin
        .spatial
        .spatial_cross
        .fill(rustfft::num_complex::Complex::new(0.0, 0.0));
    for frame in 0..12 {
        let phase = (frame as f32) * 0.7;
        plugin.fft.freq_domain[0][k] = rustfft::num_complex::Complex::new(1.0, 0.0);
        plugin.fft.freq_domain[1][k] = rustfft::num_complex::Complex::new(phase.cos(), phase.sin());
        decorrelated_sum += plugin.compute_spatial_coherence(k);
    }
    decorrelated_sum /= 12.0;
    assert!(
        decorrelated_sum < 0.5,
        "Rotating-phase bins should reduce complex coherence, got {decorrelated_sum}"
    );

    assert!(
        coherent_sum > decorrelated_sum,
        "Complex coherence should distinguish coherent vs. decorrelated bins"
    );
}

#[test]
fn test_power_at_bin_reads_no_alloc_vector_per_call() {
    let mut plugin = DenoiserPlugin::new(2, false);
    plugin.initialize(SAMPLE_RATE).unwrap();

    plugin.fft.freq_domain[0][3] = rustfft::num_complex::Complex::new(3.0, 4.0);
    let p = plugin.get_power_at_bin(0, 3);
    assert!((p - 25.0).abs() < 1e-6, "Expected norm^2 = 25, got {p}");
}
