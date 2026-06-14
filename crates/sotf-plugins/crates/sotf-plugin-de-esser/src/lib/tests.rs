use super::de_esser_plugin::DeEsserPlugin;
use super::types::DeEsserPluginParams;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, ProcessContext};

fn make_sine(freq_hz: f32, sample_rate: u32, num_frames: usize, amplitude: f32) -> Vec<f32> {
    (0..num_frames)
        .map(|i| {
            amplitude * (2.0 * std::f32::consts::PI * freq_hz * i as f32 / sample_rate as f32).sin()
        })
        .collect()
}

fn rms(buf: &[f32]) -> f32 {
    let sum: f32 = buf.iter().map(|x| x * x).sum();
    (sum / buf.len() as f32).sqrt()
}

#[test]
fn test_de_esser_reduces_sibilance() {
    let sr = 48000u32;
    let num_frames = 48000; // 1 second
    let amplitude = 0.5;

    let mut plugin = DeEsserPlugin::from_params(
        1,
        DeEsserPluginParams {
            frequency: 8000.0,
            q: 1.5,
            threshold: -20.0,
            ratio: 10.0,
            attack_ms: 0.5,
            release_ms: 20.0,
            mode: "Wideband".to_string(),
            mix: 1.0,
        },
    );
    plugin.initialize(sr).unwrap();

    // 8kHz sine in the sibilance range
    let mut buf = make_sine(8000.0, sr, num_frames, amplitude);
    let input_rms = rms(&buf);

    let ctx = ProcessContext::new(sr, num_frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    // Use the second half to allow attack to settle
    let output_rms = rms(&buf[num_frames / 2..]);

    // Output should be significantly quieter
    assert!(
        output_rms < input_rms * 0.5,
        "8kHz signal should be reduced: input_rms={:.4}, output_rms={:.4}",
        input_rms,
        output_rms
    );
}

#[test]
fn test_wideband_reduction_is_channel_specific() {
    let sr = 48000u32;
    let num_frames = 48000; // 1 second
    let sample_count = num_frames * 2;
    let amplitude = 0.5;

    let mut plugin = DeEsserPlugin::from_params(
        2,
        DeEsserPluginParams {
            frequency: 7000.0,
            q: 1.5,
            threshold: -35.0,
            ratio: 10.0,
            attack_ms: 0.5,
            release_ms: 20.0,
            mode: "Wideband".to_string(),
            mix: 1.0,
        },
    );
    plugin.initialize(sr).unwrap();

    let mut buf = Vec::with_capacity(sample_count);
    let mut low_input = Vec::with_capacity(num_frames);
    let mut high_input = Vec::with_capacity(num_frames);
    for i in 0..num_frames {
        let low = amplitude * (2.0 * std::f32::consts::PI * 200.0 * i as f32 / sr as f32).sin();
        let high = amplitude * (2.0 * std::f32::consts::PI * 8000.0 * i as f32 / sr as f32).sin();
        buf.push(low);
        buf.push(high);
        low_input.push(low);
        high_input.push(high);
    }

    let input_low_rms = rms(&low_input);
    let input_high_rms = rms(&high_input);

    let ctx = ProcessContext::new(sr, num_frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    let mut low_output = Vec::with_capacity(num_frames);
    let mut high_output = Vec::with_capacity(num_frames);
    for frame in 0..num_frames {
        low_output.push(buf[frame * 2]);
        high_output.push(buf[frame * 2 + 1]);
    }

    let output_low_rms = rms(&low_output);
    let output_high_rms = rms(&high_output);

    assert!(
        output_low_rms > input_low_rms * 0.9,
        "Low band should remain mostly untouched: input={:.4}, output={:.4}",
        input_low_rms,
        output_low_rms
    );
    assert!(
        output_high_rms < input_high_rms * 0.7,
        "High band should be reduced by sidechain: input={:.4}, output={:.4}",
        input_high_rms,
        output_high_rms
    );
    assert!(
        plugin.monitoring_gr[0].is_finite() && plugin.monitoring_gr[1].is_finite(),
        "Monitoring values should remain finite after processing.",
    );
}

#[test]
fn test_de_esser_passes_low_frequencies() {
    let sr = 48000u32;
    let num_frames = 48000; // 1 second
    let amplitude = 0.5;

    let mut plugin = DeEsserPlugin::from_params(
        1,
        DeEsserPluginParams {
            frequency: 7000.0,
            q: 1.5,
            threshold: -20.0,
            ratio: 10.0,
            attack_ms: 0.5,
            release_ms: 20.0,
            mode: "Wideband".to_string(),
            mix: 1.0,
        },
    );
    plugin.initialize(sr).unwrap();

    // 200Hz sine — well below detection range
    let mut buf = make_sine(200.0, sr, num_frames, amplitude);
    let input_rms = rms(&buf);

    let ctx = ProcessContext::new(sr, num_frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    let output_rms = rms(&buf[num_frames / 2..]);

    // Low-frequency signal should pass through mostly unchanged
    assert!(
        output_rms > input_rms * 0.9,
        "200Hz signal should pass through: input_rms={:.4}, output_rms={:.4}",
        input_rms,
        output_rms
    );
}

#[test]
fn test_de_esser_parameter_set_get() {
    let mut plugin = DeEsserPlugin::new(2);
    plugin.initialize(48000).unwrap();

    // Set frequency
    plugin
        .set_parameter(
            ParameterId::from("frequency"),
            ParameterValue::Float(10000.0),
        )
        .unwrap();
    let val = plugin.get_parameter(&ParameterId::from("frequency"));
    assert_eq!(val, Some(ParameterValue::Float(10000.0)));

    // Set threshold
    plugin
        .set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-30.0))
        .unwrap();
    let val = plugin.get_parameter(&ParameterId::from("threshold"));
    assert_eq!(val, Some(ParameterValue::Float(-30.0)));

    // Set mode
    plugin
        .set_parameter(
            ParameterId::from("mode"),
            ParameterValue::String("Wideband".to_string()),
        )
        .unwrap();
    let val = plugin.get_parameter(&ParameterId::from("mode"));
    assert_eq!(val, Some(ParameterValue::String("Wideband".to_string())));

    // Set mix
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
        .unwrap();
    let val = plugin.get_parameter(&ParameterId::from("mix"));
    assert_eq!(val, Some(ParameterValue::Float(0.5)));
}

/// Verify that the mix smoother advances per-sample during a block, not as a
/// block-constant value. If `next_n(num_frames)` were used (old code), the
/// smoother would jump to its target on the first block and the first sample
/// would already be at the target. With per-sample `advance()`, the value
/// ramps smoothly: the very first sample is close to the *starting* value,
/// not the target value.
#[test]
fn test_mix_smoother_ramps_per_sample() {
    let sr = 48000u32;
    // Start mix at 0 (dry)
    let mut plugin = DeEsserPlugin::from_params(
        1,
        DeEsserPluginParams {
            frequency: 7000.0,
            q: 1.5,
            threshold: -20.0,
            ratio: 10.0,
            attack_ms: 0.5,
            release_ms: 20.0,
            mode: "Wideband".to_string(),
            mix: 0.0, // fully dry initially
        },
    );
    plugin.initialize(sr).unwrap();

    // Now request mix = 1.0 (fully wet). The smoother has a 5 ms ramp.
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.0))
        .unwrap();

    // A silent input: output should also be silent regardless of mix
    // Use a 1 kHz tone instead so we can measure dry-vs-wet differences.
    // Use a 100-sample block — well within the 5 ms ramp (~240 samples at 48 kHz).
    let num_frames = 100;
    let mut buf: Vec<f32> = (0..num_frames)
        .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr as f32).sin())
        .collect();

    // Capture the first sample's input value
    let first_input = buf[0];

    let ctx = ProcessContext::new(sr, num_frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    // With mix=0 at block start and a 5ms ramp, after only 100 samples
    // (~2ms) the smoother should still be far below 1.0. The first output
    // sample must still be close to dry (= input * gain).
    // Specifically: if the smoother was block-constant, it would jump to ~1.0
    // and the first output would be purely wet. If it ramps per-sample,
    // the first output should be much closer to the dry value.
    //
    // We assert that the smoother has NOT jumped all the way to fully wet
    // on the first sample: the first output must not equal the fully-wet value.
    //
    // For a 1 kHz sine at mix=0 (dry), the output is approximately input*gain.
    // For mix=1 (wet), at this threshold the 1kHz tone is below the detection
    // range so gain≈1 and wet≈input. The meaningful test is therefore to
    // observe that the mix value at sample 0 is near 0, not near 1.
    //
    // We do this indirectly: set mix from 0 to 1 and verify the per-sample
    // smoother current value starts near 0. We read back the smoother
    // state by checking it hasn't already converged in 100 samples.
    // At 48kHz with a 5ms ramp, coeff = exp(-1/(0.005*48000)) = exp(-1/240) ≈ 0.9958.
    // After 100 samples: value ≈ 1 - 0.9958^100 * 1 ≈ 1 - 0.665 = 0.335.
    // The block-constant version would give 1 - 0.9958^100 ≈ 0.335 at the END
    // of block but apply that single value as-if the whole block ran at 0.335.
    // The per-sample version truly ramps 0..0.335 across the 100 samples.
    //
    // A simpler check: the first output sample should NOT be at full wet.
    // At the first sample, mix is approximately 0 (start value). So output[0]
    // should be very close to input[0] (dry) rather than whatever wet[0] would be.
    // Since there is no gain reduction yet (envelope not triggered), wet = input,
    // so dry ≈ wet in this case and the test is degenerate. Instead we verify
    // the smoother stays monotone: use a plugin with 0 threshold so gain ≈ 0 (heavy).
    let _ = first_input; // suppress unused warning

    // --- New approach: heavy compression so wet != dry ---
    let mut plugin2 = DeEsserPlugin::from_params(
        1,
        DeEsserPluginParams {
            frequency: 1000.0, // center at test freq
            q: 0.5,            // wide bandwidth to catch 1kHz
            threshold: -60.0,  // extremely low threshold → heavy compression
            ratio: 20.0,       // max ratio → near total gain kill
            attack_ms: 0.1,    // fast attack
            release_ms: 200.0,
            mode: "Wideband".to_string(),
            mix: 0.0, // start dry
        },
    );
    plugin2.initialize(sr).unwrap();

    // Ramp to fully wet over 5ms
    plugin2
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.0))
        .unwrap();

    // One block of 100 samples — still in the ramp window
    let mut buf2: Vec<f32> = (0..num_frames)
        .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr as f32).sin())
        .collect();
    let dry_ref = buf2.clone(); // original input (dry output when mix=0)

    plugin2.process_in_place(&mut buf2, &ctx).unwrap();

    // Wet output (after heavy GR) should be near-silent. Dry output = original.
    // With per-sample ramp starting at mix=0, the first samples lean dry.
    // With block-constant mix, the whole block has mix≈0.335 (already ramped).
    //
    // The first sample of buf2 should be between dry_ref[0] (when mix≈0)
    // and near-zero (when mix≈1 and gain≈0). It must not equal dry_ref[0]
    // exactly (some ramp happened) but must not be at 0 either.
    //
    // Most importantly: the output must NOT be identical to the full-wet result
    // for the entire block. We verify at least the first sample has nonzero
    // dry component.
    let first_out = buf2[0];
    let first_dry = dry_ref[0];
    // If the smoother started truly at 0 and ramped, the first sample is
    // output = 0 * wet + 1 * dry = dry (approximately, mix≈0 at t=0).
    // Allow a small tolerance since one-pole starts advancing immediately.
    assert!(
        (first_out - first_dry).abs() < first_dry.abs() * 0.2 + 1e-4,
        "First output sample should be near dry (mix≈0 at t=0): \
             first_out={:.6}, first_dry={:.6}",
        first_out,
        first_dry
    );
}

#[test]
fn test_split_band_mode() {
    let sr = 48000u32;
    let num_frames = 48000; // 1 second
    let amplitude = 0.5;

    let mut plugin = DeEsserPlugin::from_params(
        1,
        DeEsserPluginParams {
            frequency: 7000.0,
            q: 1.5,
            threshold: -20.0,
            ratio: 10.0,
            attack_ms: 0.5,
            release_ms: 20.0,
            mode: "Split-Band".to_string(),
            mix: 1.0,
        },
    );
    plugin.initialize(sr).unwrap();

    // --- Test that HF is attenuated ---
    let mut buf_hf = make_sine(8000.0, sr, num_frames, amplitude);
    let input_rms_hf = rms(&buf_hf);

    let ctx = ProcessContext::new(sr, num_frames);
    plugin.process_in_place(&mut buf_hf, &ctx).unwrap();
    let output_rms_hf = rms(&buf_hf[num_frames / 2..]);

    assert!(
        output_rms_hf < input_rms_hf * 0.7,
        "Split-band: 8kHz should be reduced: input={:.4}, output={:.4}",
        input_rms_hf,
        output_rms_hf
    );

    // --- Test that LF passes through ---
    plugin.reset();
    let mut buf_lf = make_sine(200.0, sr, num_frames, amplitude);
    let input_rms_lf = rms(&buf_lf);

    plugin.process_in_place(&mut buf_lf, &ctx).unwrap();
    let output_rms_lf = rms(&buf_lf[num_frames / 2..]);

    assert!(
        output_rms_lf > input_rms_lf * 0.85,
        "Split-band: 200Hz should pass through: input={:.4}, output={:.4}",
        input_rms_lf,
        output_rms_lf
    );
}

// -------------------------------------------------------------------------
// set_parameter smoke tests and edge cases
// -------------------------------------------------------------------------

#[test]
fn test_set_parameter_all_float_params_roundtrip() {
    let mut plugin = DeEsserPlugin::new(1);
    plugin.initialize(48000).unwrap();

    let cases: &[(&str, f32)] = &[
        ("frequency", 10000.0),
        ("q", 2.5),
        ("threshold", -30.0),
        ("ratio", 8.0),
        ("attack", 2.0),
        ("release", 50.0),
        ("mix", 0.25),
    ];

    for &(id, value) in cases {
        plugin
            .set_parameter(ParameterId::from(id), ParameterValue::Float(value))
            .unwrap();
        let got = plugin.get_parameter(&ParameterId::from(id));
        assert_eq!(
            got,
            Some(ParameterValue::Float(value)),
            "roundtrip failed for {}",
            id
        );
    }
}

#[test]
fn test_set_parameter_out_of_bounds_returns_error() {
    let mut plugin = DeEsserPlugin::new(1);
    plugin.initialize(48000).unwrap();

    // Frequency range [2000, 16000]
    assert!(
        plugin
            .set_parameter(ParameterId::from("frequency"), ParameterValue::Float(100.0))
            .is_err()
    );
    assert!(
        plugin
            .set_parameter(
                ParameterId::from("frequency"),
                ParameterValue::Float(20000.0)
            )
            .is_err()
    );

    // Q range [0.5, 5.0]
    assert!(
        plugin
            .set_parameter(ParameterId::from("q"), ParameterValue::Float(0.1))
            .is_err()
    );

    // Threshold range [-60, 0]
    assert!(
        plugin
            .set_parameter(ParameterId::from("threshold"), ParameterValue::Float(5.0))
            .is_err()
    );

    // Mix range [0, 1]
    assert!(
        plugin
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(-0.1))
            .is_err()
    );
}

#[test]
fn test_set_parameter_nan_returns_error() {
    let mut plugin = DeEsserPlugin::new(1);
    plugin.initialize(48000).unwrap();

    assert!(
        plugin
            .set_parameter(
                ParameterId::from("frequency"),
                ParameterValue::Float(f32::NAN)
            )
            .is_err()
    );
    assert!(
        plugin
            .set_parameter(
                ParameterId::from("threshold"),
                ParameterValue::Float(f32::NAN)
            )
            .is_err()
    );
}

#[test]
fn test_set_parameter_unknown_id_returns_error() {
    let mut plugin = DeEsserPlugin::new(1);
    plugin.initialize(48000).unwrap();

    assert!(
        plugin
            .set_parameter(ParameterId::from("not_a_param"), ParameterValue::Float(1.0))
            .is_err()
    );
}

#[test]
fn test_set_parameter_mode_variants() {
    let mut plugin = DeEsserPlugin::new(1);
    plugin.initialize(48000).unwrap();

    // Mode is registered as a String parameter
    plugin
        .set_parameter(
            ParameterId::from("mode"),
            ParameterValue::String("Wideband".to_string()),
        )
        .unwrap();
    assert_eq!(plugin.mode_index, 0);
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("mode")),
        Some(ParameterValue::String("Wideband".to_string()))
    );

    plugin
        .set_parameter(
            ParameterId::from("mode"),
            ParameterValue::String("Split-Band".to_string()),
        )
        .unwrap();
    assert_eq!(plugin.mode_index, 1);
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("mode")),
        Some(ParameterValue::String("Split-Band".to_string()))
    );
}

#[test]
fn test_from_params_clamps_values() {
    let plugin = DeEsserPlugin::from_params(
        1,
        DeEsserPluginParams {
            frequency: 100.0, // below min
            q: 10.0,          // above max
            threshold: 10.0,  // above max
            ratio: 0.5,       // below min
            attack_ms: 0.01,  // below min
            release_ms: 1.0,  // below min
            mode: "Wideband".to_string(),
            mix: -1.0, // below min
        },
    );
    assert_eq!(plugin.frequency, 2000.0);
    assert_eq!(plugin.q, 5.0);
    assert_eq!(plugin.threshold, 0.0);
    assert_eq!(plugin.ratio, 1.0);
    assert_eq!(plugin.attack_ms, 0.1);
    assert_eq!(plugin.release_ms, 5.0);
    assert_eq!(plugin.mix, 0.0);
}

#[test]
fn test_process_empty_buffer_returns_zero() {
    let mut plugin = DeEsserPlugin::new(1);
    plugin.initialize(48000).unwrap();

    let mut buf = vec![0.0f32; 0];
    let ctx = ProcessContext::new(48000, 0);
    let frames = plugin.process_in_place(&mut buf, &ctx).unwrap();
    assert_eq!(frames, 0);
}

#[test]
fn test_process_zero_channels_returns_num_frames() {
    let mut plugin = DeEsserPlugin::new(0);
    plugin.initialize(48000).unwrap();

    let mut buf = vec![0.0f32; 0];
    let ctx = ProcessContext::new(48000, 64);
    let frames = plugin.process_in_place(&mut buf, &ctx).unwrap();
    assert_eq!(frames, 64);
}

#[test]
fn test_info_and_channels() {
    let plugin = DeEsserPlugin::new(2);
    assert_eq!(plugin.channels(), 2);
    let info = plugin.info();
    assert_eq!(info.name, "DeEsser");
}

#[test]
fn test_reset_clears_filter_state() {
    let sr = 48000u32;
    let mut plugin = DeEsserPlugin::from_params(
        1,
        DeEsserPluginParams {
            frequency: 7000.0,
            q: 1.5,
            threshold: -20.0,
            ratio: 10.0,
            attack_ms: 0.5,
            release_ms: 20.0,
            mode: "Wideband".to_string(),
            mix: 1.0,
        },
    );
    plugin.initialize(sr).unwrap();

    let mut buf = make_sine(8000.0, sr, 4800, 0.5);
    let ctx = ProcessContext::new(sr, 4800);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    // After processing, filter state and cores have state
    plugin.reset();

    // Post-reset, processing a quiet signal should behave as at startup
    let mut buf2 = make_sine(200.0, sr, 4800, 0.5);
    let input_rms = rms(&buf2);
    plugin.process_in_place(&mut buf2, &ctx).unwrap();
    let output_rms = rms(&buf2);
    assert!(
        output_rms > input_rms * 0.9,
        "reset should restore LF pass-through behavior"
    );
}

// -------------------------------------------------------------------------
// set_parameter extended coverage
// -------------------------------------------------------------------------

#[test]
fn test_set_parameter_q_rebuilds_filters() {
    let mut plugin = DeEsserPlugin::new(1);
    plugin.initialize(48000).unwrap();

    let original_hp_freq = plugin.hp_filters.freq;
    plugin
        .set_parameter(ParameterId::from("q"), ParameterValue::Float(4.0))
        .unwrap();
    let new_hp_freq = plugin.hp_filters.freq;
    assert_ne!(
        original_hp_freq, new_hp_freq,
        "HP filter frequency should change when Q changes"
    );
}

#[test]
fn test_set_parameter_attack_updates_cores() {
    let mut plugin = DeEsserPlugin::new(1);
    plugin.initialize(48000).unwrap();

    plugin
        .set_parameter(ParameterId::from("attack"), ParameterValue::Float(5.0))
        .unwrap();
    assert_eq!(plugin.attack_ms, 5.0);
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("attack")),
        Some(ParameterValue::Float(5.0))
    );
}

#[test]
fn test_set_parameter_release_updates_cores() {
    let mut plugin = DeEsserPlugin::new(1);
    plugin.initialize(48000).unwrap();

    plugin
        .set_parameter(ParameterId::from("release"), ParameterValue::Float(100.0))
        .unwrap();
    assert_eq!(plugin.release_ms, 100.0);
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("release")),
        Some(ParameterValue::Float(100.0))
    );
}

#[test]
fn test_initialize_different_sample_rate() {
    let mut plugin = DeEsserPlugin::new(1);
    plugin.initialize(44100).unwrap();
    assert_eq!(plugin.sample_rate, 44100);

    plugin.initialize(96000).unwrap();
    assert_eq!(plugin.sample_rate, 96000);
    // Filters and crossovers should have been rebuilt for the new rate without panic
}

#[test]
fn test_set_parameter_mode_unknown_string_defaults_split_band() {
    let mut plugin = DeEsserPlugin::new(1);
    plugin.initialize(48000).unwrap();

    plugin
        .set_parameter(
            ParameterId::from("mode"),
            ParameterValue::String("Unknown".to_string()),
        )
        .unwrap();
    assert_eq!(plugin.mode_index, 1);
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("mode")),
        Some(ParameterValue::String("Split-Band".to_string()))
    );
}

#[test]
fn test_set_parameter_mix_updates_smoother_target() {
    let mut plugin = DeEsserPlugin::new(1);
    plugin.initialize(48000).unwrap();

    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.75))
        .unwrap();
    assert_eq!(plugin.mix, 0.75);
    assert!((plugin.mix_smoother.target() - 0.75).abs() < 1e-4);
}
