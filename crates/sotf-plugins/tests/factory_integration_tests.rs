// ============================================================================
// Factory + Host Integration Tests
// ============================================================================
//
// Exercises the public facade API of `sotf-plugins`: plugin factory
// registration, `DawHost`/`PluginHost` chain construction, and processing audio
// through multiple factory-created plugins.

use sotf_plugins::{
    DawHost, PluginHost, SUPPORTED_PLUGIN_TYPES, create_plugin, is_supported_plugin_type};
use std::collections::HashSet;

const SAMPLE_RATE: u32 = 48_000;

// ----------------------------------------------------------------------------
// Factory registration
// ----------------------------------------------------------------------------

#[test]
fn supported_plugin_types_includes_core_dsp_plugins() {
    let expected = [
        "gain",
        "eq",
        "parametric_eq",
        "compressor",
        "limiter",
        "gate",
        "delay",
        "upmixer",
        "downmix",
        "mono_to_stereo",
        "multiband_compressor",
        "multiband_expander",
        "loudness_compensation",
        "crossfeed",
        "matrix",
        "channel_mute_solo",
        "loudness_monitor",
        "spectrum_analyzer",
    ];
    for t in expected {
        assert!(
            is_supported_plugin_type(t),
            "expected '{t}' to be reported as supported"
        );
        assert!(
            SUPPORTED_PLUGIN_TYPES.contains(&t),
            "expected '{t}' to appear in SUPPORTED_PLUGIN_TYPES"
        );
    }
}

#[test]
fn is_supported_plugin_type_is_case_insensitive() {
    assert!(is_supported_plugin_type("GAIN"));
    assert!(is_supported_plugin_type("Eq"));
    assert!(is_supported_plugin_type("Parametric_Eq"));
}

#[test]
fn unknown_plugin_type_not_supported() {
    assert!(!is_supported_plugin_type("not_a_real_plugin"));
    assert!(!is_supported_plugin_type(""));
}

#[test]
fn advertised_factory_types_are_smoke_covered_or_documented_special_cases() {
    let smoke_cases = [
        ("gain", serde_json::json!({}), 2),
        ("eq", serde_json::json!({"filters": []}), 2),
        ("parametric_eq", serde_json::json!({"filters": []}), 2),
        ("compressor", serde_json::json!({}), 2),
        ("expander", serde_json::json!({}), 2),
        ("limiter", serde_json::json!({}), 2),
        ("gate", serde_json::json!({}), 2),
        ("delay", serde_json::json!({}), 2),
        ("upmixer", serde_json::json!({}), 2),
        ("aae", serde_json::json!({}), 2),
        ("active_acoustic_enhancement", serde_json::json!({}), 2),
        ("downmix", serde_json::json!({"input_channels": 2}), 2),
        ("mono_to_stereo", serde_json::json!({}), 1),
        ("multiband_compressor", serde_json::json!({}), 2),
        ("multiband_expander", serde_json::json!({}), 2),
        ("de_esser", serde_json::json!({}), 2),
        ("dynamic_eq", serde_json::json!({}), 2),
        ("fir_designer", serde_json::json!({}), 2),
        ("linear_phase_eq", serde_json::json!({}), 2),
        ("spectral_compressor", serde_json::json!({}), 2),
        ("stereo_imager", serde_json::json!({}), 2),
        ("transient_shaper", serde_json::json!({}), 2),
        ("saturation", serde_json::json!({}), 2),
        ("loudness_compensation", serde_json::json!({}), 2),
        ("fletcher_munson", serde_json::json!({}), 2),
        ("crossfeed", serde_json::json!({}), 2),
        ("xtc", serde_json::json!({}), 2),
        ("crosstalk_cancellation", serde_json::json!({}), 2),
        ("denoiser", serde_json::json!({}), 2),
        ("wiener_denoiser", serde_json::json!({}), 2),
        ("speech_denoiser", serde_json::json!({}), 2),
        ("rnnoise", serde_json::json!({}), 2),
        ("rnnoise_denoiser", serde_json::json!({}), 2),
        ("hiss_reducer", serde_json::json!({}), 2),
        ("hiss", serde_json::json!({}), 2),
        ("declick", serde_json::json!({}), 2),
        ("transient_repair", serde_json::json!({}), 2),
        ("pnd", serde_json::json!({}), 2),
        ("varispeed", serde_json::json!({}), 2),
        (
            "crossover",
            serde_json::json!({
                "type": "LR24",
                "frequency": 1_000.0,
                "output": "low",
                "extra_frequencies": [],
                "channel_frequencies_hz": [],
                "channel_modes": [],
            }),
            2,
        ),
        (
            "matrix",
            serde_json::json!({
                "input_channels": 2,
                "output_channels": 2,
                "matrix": [1.0, 0.0, 0.0, 1.0],
            }),
            2,
        ),
        ("channel_mute_solo", serde_json::json!({}), 2),
        ("loudness_monitor", serde_json::json!(null), 2),
        ("spectrum_analyzer", serde_json::json!(null), 2),
        (
            "resampler",
            serde_json::json!({
                "input_sample_rate": SAMPLE_RATE,
                "output_sample_rate": SAMPLE_RATE,
            }),
            2,
        ),
        ("band_split", serde_json::json!({"bands": 2}), 2),
        ("band_merge", serde_json::json!({"bands": 2}), 4),
        ("ab_compare", serde_json::json!({}), 2),
        ("ab", serde_json::json!({}), 2),
        ("aec", serde_json::json!({}), 2),
        ("beamformer", serde_json::json!({}), 2),
    ];
    let special_cases = HashSet::from([
        "convolution",
        "binaural_decoder",
        "ambisonics_decoder",
        "external",
        "external_plugin",
        "hal_input",
        "hal_output",
    ]);

    let covered: HashSet<&str> = smoke_cases
        .iter()
        .map(|(plugin_type, _, _)| *plugin_type)
        .collect();
    for plugin_type in SUPPORTED_PLUGIN_TYPES {
        assert!(
            covered.contains(plugin_type) || special_cases.contains(plugin_type),
            "advertised plugin type '{plugin_type}' is neither smoke-tested nor documented as a special case"
        );
    }

    for (plugin_type, params, channels) in smoke_cases {
        let plugin =
            create_plugin(plugin_type, &params, channels, SAMPLE_RATE).unwrap_or_else(|err| {
                panic!("failed to create advertised plugin '{plugin_type}': {err}")
            });
        assert_eq!(
            plugin.input_channels(),
            channels,
            "{plugin_type} should report the chain input width it was created for"
        );
        assert!(
            plugin.output_channels() > 0,
            "{plugin_type} should expose at least one output channel"
        );
    }
}

#[test]
fn create_plugin_rejects_unknown_type() {
    let result = create_plugin("not_a_real_plugin", &serde_json::json!({}), 2, SAMPLE_RATE);
    match result {
        Err(err) => assert!(
            err.contains("Unknown plugin type"),
            "unexpected error: {err}"
        ),
        Ok(_) => panic!("expected error for unknown plugin type"),
    }
}

#[test]
fn create_plugin_rejects_malformed_parameters() {
    let result = create_plugin(
        "gain",
        &serde_json::json!({"gain_db": "not a number"}),
        2,
        SAMPLE_RATE,
    );
    match result {
        Err(err) => assert!(
            err.to_lowercase().contains("parse") || err.to_lowercase().contains("failed"),
            "unexpected error: {err}"
        ),
        Ok(_) => panic!("expected error for malformed parameters"),
    }
}

#[test]
fn create_plugin_rejects_channel_mismatch() {
    // Upmixer requires exactly 2 input channels.
    let result = create_plugin("upmixer", &serde_json::json!({}), 5, SAMPLE_RATE);
    match result {
        Err(err) => assert!(
            err.contains("2 input channels") || err.contains("requires"),
            "unexpected error: {err}"
        ),
        Ok(_) => panic!("expected error for channel mismatch"),
    }
}

// ----------------------------------------------------------------------------
// Single-plugin creation and processing
// ----------------------------------------------------------------------------

#[test]
fn create_gain_plugin_and_process() {
    let params = serde_json::json!({"gain_db": -6.0, "smoothing_ms": 0.0});
    let mut plugin = create_plugin("gain", &params, 2, SAMPLE_RATE).unwrap();

    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);

    let frames = 64;
    let input = vec![1.0_f32; frames * 2];
    let mut output = vec![0.0_f32; frames * 2];
    let ctx = sotf_plugins::plugin::ProcessContext::new(SAMPLE_RATE, frames);

    let out_frames = plugin.process(&input, &mut output, &ctx).unwrap();
    assert_eq!(out_frames, frames);

    // -6 dB => 10^(-6/20) ≈ 0.501187
    let expected = 10.0_f32.powf(-6.0 / 20.0);
    for &sample in &output {
        assert!(
            (sample - expected).abs() < 1e-4,
            "expected ~{expected}, got {sample}"
        );
    }
}

#[test]
fn create_eq_plugin_with_empty_filters_is_passthrough() {
    let params = serde_json::json!({"filters": []});
    let mut plugin = create_plugin("eq", &params, 2, SAMPLE_RATE).unwrap();

    let frames = 64;
    let input = vec![0.5_f32; frames * 2];
    let mut output = vec![0.0_f32; frames * 2];
    let ctx = sotf_plugins::plugin::ProcessContext::new(SAMPLE_RATE, frames);

    plugin.process(&input, &mut output, &ctx).unwrap();
    for (&out, &inp) in output.iter().zip(input.iter()) {
        assert!(
            (out - inp).abs() < 1e-5,
            "empty EQ filter bank should pass through; expected {inp}, got {out}"
        );
    }
}

#[test]
fn create_limiter_plugin_and_process() {
    let params = serde_json::json!({"threshold_db": -1.0});
    let mut plugin = create_plugin("limiter", &params, 2, SAMPLE_RATE).unwrap();

    let frames = 256;
    let input = vec![0.25_f32; frames * 2];
    let mut output = vec![0.0_f32; frames * 2];
    let ctx = sotf_plugins::plugin::ProcessContext::new(SAMPLE_RATE, frames);

    let out_frames = plugin.process(&input, &mut output, &ctx).unwrap();
    assert_eq!(out_frames, frames);

    // With a -1 dB threshold and 0.25 (-12 dBFS) input, the limiter should not
    // be actively reducing gain.
    for &sample in &output {
        assert!(sample.abs() < 1.0, "limiter output should remain bounded");
    }
}

// ----------------------------------------------------------------------------
// Plugin chain construction and multi-plugin processing
// ----------------------------------------------------------------------------

#[test]
fn host_chain_three_factory_gains() {
    let mut host = PluginHost::new(2, SAMPLE_RATE);

    for _ in 0..3 {
        let plugin = create_plugin(
            "gain",
            &serde_json::json!({"gain_db": -6.0, "smoothing_ms": 0.0}),
            2,
            SAMPLE_RATE,
        )
        .unwrap();
        host.add_plugin(plugin).unwrap();
    }

    assert_eq!(host.plugin_count(), 3);
    assert_eq!(host.input_channels(), 2);
    assert_eq!(host.output_channels(), 2);

    let frames = 64;
    let input = vec![1.0_f32; frames * 2];
    let mut output = vec![0.0_f32; frames * 2];

    let processed = host.process(&input, &mut output).unwrap();
    assert_eq!(processed, frames);

    // Three -6 dB stages => -18 dB total.
    let expected = 10.0_f32.powf(-18.0 / 20.0);
    for &sample in &output {
        assert!(
            (sample - expected).abs() < 1e-3,
            "expected ~{expected}, got {sample}"
        );
    }
}

#[test]
fn host_chain_gain_eq_gain_roundtrip() {
    let mut host = DawHost::new(2, SAMPLE_RATE);

    let gain1 = create_plugin(
        "gain",
        &serde_json::json!({"gain_db": -12.0, "smoothing_ms": 0.0}),
        2,
        SAMPLE_RATE,
    )
    .unwrap();
    host.add_plugin(gain1).unwrap();

    let eq = create_plugin("eq", &serde_json::json!({"filters": []}), 2, SAMPLE_RATE).unwrap();
    host.add_plugin(eq).unwrap();

    let gain2 = create_plugin(
        "gain",
        &serde_json::json!({"gain_db": -6.0, "smoothing_ms": 0.0}),
        2,
        SAMPLE_RATE,
    )
    .unwrap();
    host.add_plugin(gain2).unwrap();

    let frames = 256;
    let input = vec![0.8_f32; frames * 2];
    let mut output = vec![0.0_f32; frames * 2];

    let processed = host.process(&input, &mut output).unwrap();
    assert_eq!(processed, frames);

    // -12 dB then -6 dB => -18 dB total.
    let expected = 0.8 * 10.0_f32.powf(-18.0 / 20.0);
    for &sample in &output {
        assert!(
            (sample - expected).abs() < 1e-3,
            "expected ~{expected}, got {sample}"
        );
    }
}

#[test]
fn host_reports_plugins_via_get_plugin() {
    let mut host = PluginHost::new(2, SAMPLE_RATE);
    assert!(host.get_plugin(0).is_none());

    let plugin =
        create_plugin("gain", &serde_json::json!({"gain_db": 0.0}), 2, SAMPLE_RATE).unwrap();
    host.add_plugin(plugin).unwrap();

    let info = host.get_plugin(0).unwrap().info();
    assert_eq!(info.name, "Gain");
}

// ----------------------------------------------------------------------------
// Error paths visible through the public API
// ----------------------------------------------------------------------------

#[test]
fn host_add_plugin_rejects_channel_mismatch() {
    let mut host = PluginHost::new(2, SAMPLE_RATE);
    host.add_plugin(
        create_plugin("gain", &serde_json::json!({"gain_db": 0.0}), 2, SAMPLE_RATE).unwrap(),
    )
    .unwrap();

    let wrong_channels =
        create_plugin("gain", &serde_json::json!({"gain_db": 0.0}), 5, SAMPLE_RATE).unwrap();
    assert!(host.add_plugin(wrong_channels).is_err());
}

#[test]
fn host_remove_plugin_rewires_chain() {
    let mut host = PluginHost::new(2, SAMPLE_RATE);

    host.add_plugin(
        create_plugin(
            "gain",
            &serde_json::json!({"gain_db": -6.0, "smoothing_ms": 0.0}),
            2,
            SAMPLE_RATE,
        )
        .unwrap(),
    )
    .unwrap();
    host.add_plugin(
        create_plugin(
            "gain",
            &serde_json::json!({"gain_db": -6.0, "smoothing_ms": 0.0}),
            2,
            SAMPLE_RATE,
        )
        .unwrap(),
    )
    .unwrap();
    host.add_plugin(
        create_plugin(
            "gain",
            &serde_json::json!({"gain_db": -6.0, "smoothing_ms": 0.0}),
            2,
            SAMPLE_RATE,
        )
        .unwrap(),
    )
    .unwrap();

    // Remove the middle plugin; the remaining two should still form a chain.
    host.remove_plugin(1).unwrap();
    assert_eq!(host.plugin_count(), 2);

    let frames = 64;
    let input = vec![1.0_f32; frames * 2];
    let mut output = vec![0.0_f32; frames * 2];
    host.process(&input, &mut output).unwrap();

    let expected = 10.0_f32.powf(-12.0 / 20.0);
    for &sample in &output {
        assert!(
            (sample - expected).abs() < 1e-3,
            "expected ~{expected}, got {sample}"
        );
    }
}

#[test]
fn host_remove_plugin_out_of_bounds_errors() {
    let mut host = PluginHost::new(2, SAMPLE_RATE);
    assert!(host.remove_plugin(0).is_err());
}

#[test]
fn host_reset_maintains_chain() {
    let mut host = PluginHost::new(2, SAMPLE_RATE);
    host.add_plugin(
        create_plugin(
            "gain",
            &serde_json::json!({"gain_db": -6.0, "smoothing_ms": 0.0}),
            2,
            SAMPLE_RATE,
        )
        .unwrap(),
    )
    .unwrap();

    let frames = 64;
    let input = vec![1.0_f32; frames * 2];
    let mut output_before = vec![0.0_f32; frames * 2];
    host.process(&input, &mut output_before).unwrap();

    host.reset();

    let mut output_after = vec![0.0_f32; frames * 2];
    host.process(&input, &mut output_after).unwrap();

    for (before, after) in output_before.iter().zip(output_after.iter()) {
        assert!(
            (before - after).abs() < 1e-6,
            "reset should leave deterministic plugin state; got {before} vs {after}"
        );
    }
}
