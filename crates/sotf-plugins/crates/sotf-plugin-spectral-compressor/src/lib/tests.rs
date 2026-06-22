use super::misc::compress_gr;
use super::misc::fft_size_from_index;
use super::misc::smooth_spectral_envelope;
use super::spectral_compressor_plugin::SpectralCompressorPlugin;
use super::spectral_compressor_plugin_params::SpectralCompressorPluginParams;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;

mod misc;

#[test]
fn test_mix_zero_passthrough_during_latency_fill() {
    let params = SpectralCompressorPluginParams {
        mix: 0.0,
        ..Default::default()
    };
    let mut plugin = SpectralCompressorPlugin::from_params(2, params);
    plugin.initialize(48000).unwrap();

    let frames = 128;
    let mut buffer = vec![0.0f32; frames * 2];
    for i in 0..frames {
        buffer[i * 2] = i as f32 * 0.001;
        buffer[i * 2 + 1] = -(i as f32) * 0.001;
    }
    let original = buffer.clone();
    let ctx = ProcessContext::new(48000, frames);

    plugin.process_in_place(&mut buffer, &ctx).unwrap();
    assert_eq!(buffer, original);
}

/// Verify that constructing a plugin with attack_ms=0 or release_ms=0 does
/// not produce NaN/inf coefficients. Zero → instant response (coeff=0).
#[test]
fn test_zero_attack_release_coefficients() {
    let params = SpectralCompressorPluginParams {
        attack_ms: 0.0,
        release_ms: 0.0,
        ..Default::default()
    };
    let plugin = SpectralCompressorPlugin::from_params(2, params);
    assert!(
        plugin.attack_coeff.is_finite(),
        "attack_coeff should be finite when attack_ms=0, got {}",
        plugin.attack_coeff
    );
    assert!(
        plugin.release_coeff.is_finite(),
        "release_coeff should be finite when release_ms=0, got {}",
        plugin.release_coeff
    );
    assert_eq!(
        plugin.attack_coeff, 0.0,
        "attack_ms=0 should give instant coeff=0"
    );
    assert_eq!(
        plugin.release_coeff, 0.0,
        "release_ms=0 should give instant coeff=0"
    );
}

#[test]
fn test_spectral_smoothing_symmetric_bounds() {
    // Regression test for asymmetric smoothing loop ranges.
    // Forward and backward passes must visit the same set of bins.
    let alpha = 0.5;

    // Spike at DC (bin 0)
    let mut dc_spike = vec![10.0, 0.0, 0.0, 0.0];
    smooth_spectral_envelope(&mut dc_spike, alpha);
    // Forward:  [10.0, 5.0, 2.5, 1.25]
    // Backward: [6.71875, 3.4375, 1.875, 1.25]
    assert!((dc_spike[0] - 6.71875).abs() < 1e-6);
    assert!((dc_spike[1] - 3.4375).abs() < 1e-6);
    assert!((dc_spike[2] - 1.875).abs() < 1e-6);
    assert!((dc_spike[3] - 1.25).abs() < 1e-6);

    // Spike at Nyquist (last bin)
    let mut nyq_spike = vec![0.0, 0.0, 0.0, 10.0];
    smooth_spectral_envelope(&mut nyq_spike, alpha);
    // Forward:  [0.0, 0.0, 0.0, 5.0]
    // Backward: [0.625, 1.25, 2.5, 5.0]
    assert!((nyq_spike[0] - 0.625).abs() < 1e-6);
    assert!((nyq_spike[1] - 1.25).abs() < 1e-6);
    assert!((nyq_spike[2] - 2.5).abs() < 1e-6);
    assert!((nyq_spike[3] - 5.0).abs() < 1e-6);

    // Both boundary spikes should propagate through the full range,
    // confirming that forward visits bin 0 and backward visits bin N-1.
}

// -------------------------------------------------------------------------
// Pure helper tests
// -------------------------------------------------------------------------

#[test]
fn test_compress_gr_hard_knee() {
    assert_eq!(compress_gr(-10.0, -5.0, 4.0, 0.0), 0.0);
    let slope = 1.0 - 1.0 / 4.0;
    let gr = compress_gr(5.0, -5.0, 4.0, 0.0);
    assert!((gr - 10.0 * slope).abs() < 1e-5);
}

#[test]
fn test_compress_gr_soft_knee() {
    let slope = 1.0 - 1.0 / 4.0;
    assert_eq!(compress_gr(-10.0, 0.0, 4.0, 4.0), 0.0);
    let gr_above = compress_gr(10.0, 0.0, 4.0, 4.0);
    assert!((gr_above - 10.0 * slope).abs() < 1e-5);
    let gr_mid = compress_gr(0.0, 0.0, 4.0, 4.0);
    assert!(gr_mid > 0.0 && gr_mid < 2.0 * slope);
}

#[test]
fn test_smooth_spectral_envelope_edge_cases() {
    let mut empty: Vec<f32> = vec![];
    smooth_spectral_envelope(&mut empty, 0.5); // must not panic
    let mut one = vec![7.0f32];
    smooth_spectral_envelope(&mut one, 0.5);
    assert_eq!(one[0], 7.0);
    let mut flat = vec![1.0f32; 4];
    smooth_spectral_envelope(&mut flat, 0.5);
    assert!(flat.iter().all(|&s| (s - 1.0).abs() < 1e-6));
}

#[test]
fn test_fft_size_from_index_out_of_range() {
    assert_eq!(fft_size_from_index(0), 1024);
    assert_eq!(fft_size_from_index(1), 2048);
    assert_eq!(fft_size_from_index(2), 4096);
    assert_eq!(fft_size_from_index(99), 2048);
}

// -------------------------------------------------------------------------
// Process tests for additional modes
// -------------------------------------------------------------------------

#[test]
fn test_delta_listen_outputs_difference_signal() {
    // Delta mode outputs (wet - dry). With active compression the delta
    // must be non-zero after the initial STFT latency.
    let params = SpectralCompressorPluginParams {
        threshold_db: -40.0,
        ratio: 8.0,
        mix: 1.0,
        ..Default::default()
    };
    let mut plugin = SpectralCompressorPlugin::from_params(1, params);
    plugin.initialize(48000).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("delta_listen"),
            ParameterValue::Bool(true),
        )
        .unwrap();

    let nf = plugin.latency_samples() + 4096;
    let mut buf: Vec<f32> = (0..nf)
        .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 48000.0).sin())
        .collect();
    let original = buf.clone();
    plugin
        .process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();

    let latency = plugin.latency_samples();
    let max_diff = buf[latency..]
        .iter()
        .zip(&original[latency..])
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(
        max_diff > 0.001,
        "Delta listen should differ from dry when compression is active, got {max_diff}"
    );
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_adaptive_threshold_no_nan() {
    let params = SpectralCompressorPluginParams::default();
    let mut plugin = SpectralCompressorPlugin::from_params(2, params);
    plugin.initialize(48000).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("adaptive_threshold"),
            ParameterValue::Bool(true),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("adaptive_offset_db"),
            ParameterValue::Float(3.0),
        )
        .unwrap();

    let nf = 8192usize;
    let mut buf: Vec<f32> = (0..nf * 2)
        .map(|i| 0.1 * ((i / 2) as f32 * 0.05).sin())
        .collect();
    plugin
        .process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_target_mode_tonal_no_nan() {
    let params = SpectralCompressorPluginParams::default();
    let mut plugin = SpectralCompressorPlugin::from_params(2, params);
    plugin.initialize(48000).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("target_mode"),
            ParameterValue::String("Tonal".into()),
        )
        .unwrap();

    let nf = 8192usize;
    let mut buf: Vec<f32> = (0..nf * 2)
        .map(|i| 0.2 * ((i / 2) as f32 * 0.03).sin())
        .collect();
    plugin
        .process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_reset_clears_stft_state() {
    let params = SpectralCompressorPluginParams::default();
    let mut plugin = SpectralCompressorPlugin::from_params(1, params);
    plugin.initialize(48000).unwrap();

    let nf = plugin.latency_samples() + 256;
    let mut buf = vec![0.0f32; nf];
    buf[plugin.latency_samples()] = 1.0;
    plugin
        .process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();

    plugin.reset();
    let mut silence = vec![0.0f32; nf];
    plugin
        .process_in_place(&mut silence, &ProcessContext::new(48000, nf))
        .unwrap();
    let max_abs = silence.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(
        max_abs < 1e-3,
        "After reset, silence should produce near-zero output, got {max_abs}"
    );
}

#[test]
fn test_recompute_coefficients_after_parameter_change() {
    let params = SpectralCompressorPluginParams {
        attack_ms: 5.0,
        release_ms: 50.0,
        ..Default::default()
    };
    let mut plugin = SpectralCompressorPlugin::from_params(1, params);
    plugin.initialize(48000).unwrap();
    let old_attack = plugin.attack_coeff;

    plugin
        .set_parameter(ParameterId::from("attack"), ParameterValue::Float(0.5))
        .unwrap();

    assert!(plugin.attack_coeff.is_finite());
    assert!(
        plugin.attack_coeff < old_attack,
        "Faster attack should produce a smaller EMA coefficient"
    );
}
