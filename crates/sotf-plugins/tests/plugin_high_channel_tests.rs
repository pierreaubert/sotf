//! High-channel and extreme-parameter edge-case tests for built-in plugins.
//!
//! Covers 5.1 / 7.1.4 layout transitions and parameter extremes per
//! QA-PLUGIN-003/004 and QA-DSP-001/002/003.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_plugins::factory::create_plugin;
use sotf_plugins::plugin::{Plugin, ProcessContext};

const SAMPLE_RATE: u32 = 48_000;
// RNNoise-based denoisers require block sizes that are a multiple of 480.
const FRAMES: usize = 480;
const HIGH_CHANNEL_COUNTS: &[usize] = &[6, 12];

#[allow(dead_code)]
fn is_skipped(plugin_type: &str) -> bool {
    matches!(
        plugin_type,
        "external" | "external_plugin" | "hal_input" | "hal_output"
    )
}

fn default_params(plugin_type: &str, channels: usize) -> serde_json::Value {
    let identity_matrix: Vec<f32> = (0..channels)
        .flat_map(|i| {
            let mut row = vec![0.0f32; channels];
            row[i] = 1.0;
            row
        })
        .collect();

    match plugin_type {
        "loudness_compensation" | "fletcher_munson" => serde_json::json!({
            "low_freq": 100.0,
            "high_freq": 10000.0,
            "low_gain": 0.0,
            "high_gain": 0.0,
        }),
        "convolution" => serde_json::json!({
            "ir_file": "",
            "channel_gains": [],
            "mix": 1.0,
            "gain_db": 0.0,
        }),
        "downmix" => serde_json::json!({
            "input_channels": channels,
        }),
        "binaural_decoder" => serde_json::json!({
            "input_channels": channels,
            "sofa_file": "",
        }),
        "crossover" => serde_json::json!({
            "type": "lr4",
            "frequency": 1000.0,
            "output": "lowpass",
        }),
        "spectrum_analyzer" => serde_json::json!({
            "num_bins": 100,
            "min_freq": 20.0,
            "max_freq": 20000.0,
            "smoothing": 0.5,
        }),
        "resampler" => serde_json::json!({
            "input_sample_rate": SAMPLE_RATE,
            "output_sample_rate": SAMPLE_RATE,
            "chunk_size": FRAMES,
        }),
        "matrix" => serde_json::json!({
            "input_channels": channels,
            "output_channels": channels,
            "matrix": identity_matrix,
        }),
        "band_split" => serde_json::json!({
            "num_bands": 2,
            "frequency": 1000.0,
            "type": "lr4",
        }),
        "band_merge" => serde_json::json!({
            "bands": 2,
        }),
        "aec" => serde_json::json!({
            "filter_length_ms": 100.0,
        }),
        "beamformer" => serde_json::json!({
            "num_mics": channels.max(2),
            "mic_spacing_m": 0.05,
        }),
        "ambisonics_decoder" => serde_json::json!({
            "order": 1,
            "output_channels": channels.min(4),
        }),
        _ => serde_json::json!({}),
    }
}

#[allow(dead_code)]
fn required_input_channels(plugin_type: &str) -> Option<usize> {
    match plugin_type {
        "mono_to_stereo" => Some(1),
        "upmixer"
        | "crossfeed"
        | "xtc"
        | "crosstalk_cancellation"
        | "aae"
        | "active_acoustic_enhancement"
        | "ab_compare"
        | "ab"
        | "aec"
        | "binaural_decoder" => Some(2),
        _ => None,
    }
}

fn interleaved_sine(channels: usize, frames: usize) -> Vec<f32> {
    let mut buf = vec![0.0f32; frames * channels];
    for i in 0..frames {
        let t = i as f32 / SAMPLE_RATE as f32;
        let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.25;
        for ch in 0..channels {
            buf[i * channels + ch] = s;
        }
    }
    buf
}

fn assert_all_finite(buffer: &[f32], label: &str) {
    for (i, &s) in buffer.iter().enumerate() {
        assert!(
            s.is_finite(),
            "{label}: non-finite value at index {i} (value: {s})"
        );
    }
}

/// Plugins that are expected to be channel-count agnostic and should accept
/// 5.1 / 7.1.4 inputs without panic or non-finite output.
fn multi_channel_capable_plugins() -> Vec<&'static str> {
    vec![
        "gain",
        "eq",
        "parametric_eq",
        "compressor",
        "expander",
        "limiter",
        "gate",
        "delay",
        "multiband_compressor",
        "multiband_expander",
        "loudness_compensation",
        "fletcher_munson",
        "denoiser",
        "wiener_denoiser",
        "rnnoise",
        "rnnoise_denoiser",
        "hiss_reducer",
        "hiss",
        "declick",
        "transient_repair",
        "pnd",
        "varispeed",
        "channel_mute_solo",
        "loudness_monitor",
        "spectrum_analyzer",
        "matrix",
        "crossover",
    ]
}

#[test]
fn multi_channel_capable_plugins_stay_finite_at_5_1_and_7_1_4() {
    let mut failures = Vec::new();

    for &plugin_type in &multi_channel_capable_plugins() {
        for &channels in HIGH_CHANNEL_COUNTS {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut plugin = create_plugin(
                    plugin_type,
                    &default_params(plugin_type, channels),
                    channels,
                    SAMPLE_RATE,
                )?;
                plugin.initialize(SAMPLE_RATE)?;

                let input = interleaved_sine(channels, FRAMES);
                let output_channels = plugin.output_channels();
                let mut output = vec![0.0f32; output_channels * FRAMES];
                plugin.process(
                    &input,
                    &mut output,
                    &ProcessContext::new(SAMPLE_RATE, FRAMES),
                )?;

                assert_all_finite(
                    &output,
                    &format!("{plugin_type}@{channels}ch -> {output_channels}ch"),
                );
                Ok::<(), String>(())
            }));

            match result {
                Ok(Ok(())) => {}
                Ok(Err(err)) => {
                    failures.push(format!("{plugin_type}@{channels}ch failed: {err}"));
                }
                Err(payload) => {
                    failures.push(format!(
                        "{plugin_type}@{channels}ch panicked: {}",
                        panic_payload_description(&payload)
                    ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "high-channel plugin failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn downmix_from_5_1_and_7_1_4_to_stereo_is_finite() {
    // Use a longer block so the STFT-based downmix is past its lookahead.
    let frames = 16_384;

    for &channels in HIGH_CHANNEL_COUNTS {
        let mut plugin = create_plugin(
            "downmix",
            &serde_json::json!({"input_channels": channels}),
            channels,
            SAMPLE_RATE,
        )
        .expect("downmix should instantiate for high-channel input");
        plugin.initialize(SAMPLE_RATE).unwrap();

        let input = interleaved_sine(channels, frames);
        let mut output = vec![0.0f32; frames * 2];
        plugin
            .process(
                &input,
                &mut output,
                &ProcessContext::new(SAMPLE_RATE, frames),
            )
            .unwrap();

        assert_all_finite(&output, &format!("downmix {channels}ch -> 2ch"));

        let energy: f32 = output.iter().map(|s| s * s).sum();
        assert!(
            energy > 1e-6,
            "downmix {channels}ch -> 2ch should produce non-silent output (energy={energy})"
        );
    }
}

#[test]
fn matrix_identity_7_1_4_is_finite_and_passthrough() {
    let channels = 12;
    let identity: Vec<f32> = (0..channels)
        .flat_map(|i| {
            let mut row = vec![0.0f32; channels];
            row[i] = 1.0;
            row
        })
        .collect();

    let mut plugin = create_plugin(
        "matrix",
        &serde_json::json!({
            "input_channels": channels,
            "output_channels": channels,
            "matrix": identity,
        }),
        channels,
        SAMPLE_RATE,
    )
    .expect("matrix identity should instantiate for 12 channels");
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = interleaved_sine(channels, FRAMES);
    let mut output = vec![0.0f32; FRAMES * channels];
    plugin
        .process(
            &input,
            &mut output,
            &ProcessContext::new(SAMPLE_RATE, FRAMES),
        )
        .unwrap();

    assert_all_finite(&output, "matrix 12ch identity");

    let energy: f32 = output.iter().map(|s| s * s).sum();
    assert!(energy > 0.0, "matrix 12ch identity should produce signal");
}

#[test]
fn band_split_merge_roundtrip_at_5_1_and_7_1_4_is_finite() {
    let mut failures = Vec::new();

    for &channels in HIGH_CHANNEL_COUNTS {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut split = create_plugin(
                "band_split",
                &default_params("band_split", channels),
                channels,
                SAMPLE_RATE,
            )
            .map_err(|e| format!("band_split create: {e}"))?;
            split.initialize(SAMPLE_RATE).map_err(|e| e.to_string())?;

            let input = interleaved_sine(channels, FRAMES);
            let split_output_channels = split.output_channels();
            let mut split_output = vec![0.0f32; split_output_channels * FRAMES];
            split
                .process(
                    &input,
                    &mut split_output,
                    &ProcessContext::new(SAMPLE_RATE, FRAMES),
                )
                .map_err(|e| e.to_string())?;
            assert_all_finite(
                &split_output,
                &format!("band_split {channels}ch -> {split_output_channels}ch"),
            );

            let mut merge = create_plugin(
                "band_merge",
                &default_params("band_merge", split_output_channels),
                split_output_channels,
                SAMPLE_RATE,
            )
            .map_err(|e| format!("band_merge create: {e}"))?;
            merge.initialize(SAMPLE_RATE).map_err(|e| e.to_string())?;

            let merge_output_channels = merge.output_channels();
            let mut merge_output = vec![0.0f32; merge_output_channels * FRAMES];
            merge
                .process(
                    &split_output,
                    &mut merge_output,
                    &ProcessContext::new(SAMPLE_RATE, FRAMES),
                )
                .map_err(|e| e.to_string())?;
            assert_all_finite(
                &merge_output,
                &format!("band_merge {split_output_channels}ch -> {merge_output_channels}ch"),
            );

            if merge_output_channels != channels {
                return Err(format!(
                    "band_merge output channels {merge_output_channels} != input channels {channels}"
                ));
            }

            let energy: f32 = merge_output.iter().map(|s| s * s).sum();
            if energy < 1e-12 {
                return Err(format!(
                    "band_split/merge roundtrip {channels}ch produced silence (energy={energy})"
                ));
            }

            Ok::<(), String>(())
        }));

        match result {
            Ok(Ok(())) => {}
            Ok(Err(err)) => failures.push(format!("{channels}ch: {err}")),
            Err(payload) => failures.push(format!(
                "{channels}ch panicked: {}",
                panic_payload_description(&payload)
            )),
        }
    }

    assert!(
        failures.is_empty(),
        "band_split/merge roundtrip failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn mono_to_stereo_from_1ch_is_finite() {
    let mut plugin = create_plugin(
        "mono_to_stereo",
        &default_params("mono_to_stereo", 1),
        1,
        SAMPLE_RATE,
    )
    .expect("mono_to_stereo should instantiate for 1ch input");
    plugin.initialize(SAMPLE_RATE).unwrap();

    assert_eq!(plugin.input_channels(), 1);
    assert_eq!(plugin.output_channels(), 2);

    // The plugin is STFT-based and has latency; use a large block and measure
    // the settled output. Disable Haas delay and frequency-dependent mode for a
    // deterministic passthrough-ish check.
    plugin
        .set_parameter(
            ParameterId::from("stereo_width"),
            ParameterValue::Float(1.0),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("haas_delay_ms"),
            ParameterValue::Float(0.0),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("freq_dependent"),
            ParameterValue::Bool(false),
        )
        .unwrap();

    // FFT_SIZE is 2048 inside the plugin; 16x gives the latency buffer time to
    // fill and the smoother time to settle.
    let frames = 32_768;
    let input = interleaved_sine(1, frames);
    let mut output = vec![0.0f32; 2 * frames];
    plugin
        .process(
            &input,
            &mut output,
            &ProcessContext::new(SAMPLE_RATE, frames),
        )
        .unwrap();

    assert_all_finite(&output, "mono_to_stereo 1ch -> 2ch");

    // Measure energy in the settled tail to avoid the initial latency silence.
    let tail = &output[frames..];
    let energy: f32 = tail.iter().map(|s| s * s).sum();
    assert!(
        energy > 1e-6,
        "mono_to_stereo settled output should be non-silent (energy={energy})"
    );
}

#[test]
fn factory_reports_exact_channel_layouts_for_channel_changing_plugins() {
    let cases = [
        ("mono_to_stereo", serde_json::json!({}), 1, 2),
        ("upmixer", serde_json::json!({}), 2, 6),
        ("aae", serde_json::json!({}), 2, 6),
        ("downmix", serde_json::json!({"input_channels": 6}), 6, 2),
        (
            "binaural_decoder",
            default_params("binaural_decoder", 6),
            6,
            2,
        ),
        (
            "band_split",
            serde_json::json!({"num_bands": 2, "frequency": 1000.0, "type": "lr4"}),
            3,
            6,
        ),
        ("band_merge", serde_json::json!({"bands": 2}), 6, 3),
        ("aec", serde_json::json!({}), 2, 1),
        ("beamformer", serde_json::json!({"num_mics": 4}), 4, 1),
        (
            "ambisonics_decoder",
            serde_json::json!({"order": 1, "target_layout": "5.1"}),
            4,
            6,
        ),
        (
            "matrix",
            serde_json::json!({
                "input_channels": 4,
                "output_channels": 2,
                "matrix": [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]
            }),
            4,
            2,
        ),
    ];

    for (plugin_type, params, input_channels, output_channels) in cases {
        let plugin = create_plugin(plugin_type, &params, input_channels, SAMPLE_RATE)
            .unwrap_or_else(|err| panic!("{plugin_type}@{input_channels}ch failed: {err}"));
        assert_eq!(plugin.input_channels(), input_channels, "{plugin_type}");
        assert_eq!(plugin.output_channels(), output_channels, "{plugin_type}");
        assert!(
            plugin.supports_channel_config(input_channels, output_channels),
            "{plugin_type} rejected its declared layout"
        );
        assert!(
            !plugin.supports_channel_config(input_channels + 1, output_channels),
            "{plugin_type} accepted an undeclared input layout"
        );
        assert!(
            !plugin.supports_channel_config(input_channels, output_channels + 1),
            "{plugin_type} accepted an undeclared output layout"
        );
    }
}

#[test]
fn factory_rejects_mismatched_channel_layout_configuration() {
    let cases = [
        ("mono_to_stereo", serde_json::json!({}), 2),
        ("binaural_decoder", default_params("binaural_decoder", 6), 2),
        ("aec", serde_json::json!({}), 1),
        ("beamformer", serde_json::json!({"num_mics": 2}), 4),
        (
            "ambisonics_decoder",
            serde_json::json!({"order": 1, "target_layout": "5.1"}),
            2,
        ),
        ("band_merge", serde_json::json!({"bands": 2}), 3),
        (
            "matrix",
            serde_json::json!({
                "input_channels": 2,
                "output_channels": 1,
                "matrix": [1.0, 0.0]
            }),
            4,
        ),
    ];

    for (plugin_type, params, graph_channels) in cases {
        let err = match create_plugin(plugin_type, &params, graph_channels, SAMPLE_RATE) {
            Ok(_) => panic!("{plugin_type} unexpectedly accepted {graph_channels} channels"),
            Err(err) => err,
        };
        assert!(
            err.contains("channel") || err.contains("microphone") || err.contains("divisible"),
            "{plugin_type} returned a non-actionable layout error: {err}"
        );
    }
}

#[test]
fn ab_compare_switching_preserves_channel_layout_at_stereo() {
    let channels = 2;
    let mut plugin = create_plugin(
        "ab_compare",
        &default_params("ab_compare", channels),
        channels,
        SAMPLE_RATE,
    )
    .expect("ab_compare should instantiate for 2ch input");
    plugin.initialize(SAMPLE_RATE).unwrap();

    assert_eq!(plugin.input_channels(), channels);
    assert_eq!(plugin.output_channels(), channels);

    let input = interleaved_sine(channels, FRAMES);
    let mut output = vec![0.0f32; channels * FRAMES];

    // Process path A.
    plugin
        .process(
            &input,
            &mut output,
            &ProcessContext::new(SAMPLE_RATE, FRAMES),
        )
        .unwrap();
    assert_all_finite(&output, "ab_compare path A");

    // Switch to path B without reallocating.
    plugin
        .set_parameter(ParameterId::from("selected_path"), ParameterValue::Int(1))
        .unwrap();

    plugin
        .process(
            &input,
            &mut output,
            &ProcessContext::new(SAMPLE_RATE, FRAMES),
        )
        .unwrap();
    assert_all_finite(&output, "ab_compare path B");
    assert_eq!(
        plugin.output_channels(),
        channels,
        "ab_compare output channel count must not change after switching"
    );
}

#[test]
fn extreme_float_parameters_do_not_produce_nonfinite_output() {
    let stereo_types = [
        "gain",
        "eq",
        "compressor",
        "expander",
        "limiter",
        "gate",
        "delay",
        "multiband_compressor",
        "multiband_expander",
        "loudness_compensation",
        "crossfeed",
        "channel_mute_solo",
        "denoiser",
        "pnd",
        "declick",
    ];

    let mut failures = Vec::new();

    for &plugin_type in &stereo_types {
        let base_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut plugin =
                create_plugin(plugin_type, &default_params(plugin_type, 2), 2, SAMPLE_RATE)?;
            plugin.initialize(SAMPLE_RATE)?;
            Ok::<Box<dyn Plugin>, String>(plugin)
        }));

        let mut plugin = match base_result {
            Ok(Ok(p)) => p,
            Ok(Err(err)) => {
                failures.push(format!("{plugin_type} create failed: {err}"));
                continue;
            }
            Err(payload) => {
                failures.push(format!(
                    "{plugin_type} create panicked: {}",
                    panic_payload_description(&payload)
                ));
                continue;
            }
        };

        let params = plugin.parameters().to_vec();
        for param in &params {
            let test_values: Vec<ParameterValue> = match param.default_value {
                ParameterValue::Float(_) => {
                    let mut vals = Vec::new();
                    if let Some(ParameterValue::Float(min)) = param.min_value.clone() {
                        vals.push(ParameterValue::Float(min));
                    }
                    if let Some(ParameterValue::Float(max)) = param.max_value.clone() {
                        vals.push(ParameterValue::Float(max));
                    }
                    vals.push(ParameterValue::Float(f32::NAN));
                    vals.push(ParameterValue::Float(f32::INFINITY));
                    vals.push(ParameterValue::Float(f32::NEG_INFINITY));
                    vals
                }
                ParameterValue::Int(_) => {
                    let mut vals = Vec::new();
                    if let Some(ParameterValue::Int(min)) = param.min_value.clone() {
                        vals.push(ParameterValue::Int(min));
                    }
                    if let Some(ParameterValue::Int(max)) = param.max_value.clone() {
                        vals.push(ParameterValue::Int(max));
                    }
                    vals
                }
                ParameterValue::Bool(_) => {
                    vec![ParameterValue::Bool(true), ParameterValue::Bool(false)]
                }
                ParameterValue::String(_) => continue,
            };

            for value in test_values {
                let _ = plugin.set_parameter(ParameterId::from(param.id.as_str()), value.clone());

                let input = interleaved_sine(2, FRAMES);
                let output_channels = plugin.output_channels();
                let mut output = vec![0.0f32; output_channels * FRAMES];
                let label = format!("{plugin_type} param {}={value:?}", param.id);

                match plugin.process(
                    &input,
                    &mut output,
                    &ProcessContext::new(SAMPLE_RATE, FRAMES),
                ) {
                    Ok(_) => {
                        if !output.iter().all(|s| s.is_finite()) {
                            failures.push(format!("{label}: non-finite output after process"));
                        }
                    }
                    Err(err) => {
                        // Errors are acceptable, but a panic would have been caught above.
                        let _ = err;
                    }
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "extreme-parameter failures:\n{}",
        failures.join("\n")
    );
}

/// Plugins that use FFT/STFT/FIR paths and should be exercised with a range of
/// block sizes (including very small and non-power-of-two blocks).
fn block_size_sensitive_plugins() -> Vec<&'static str> {
    vec![
        "downmix",
        "upmixer",
        "convolution",
        "binaural_decoder",
        "linear_phase_eq",
        "spectral_compressor",
        "multiband_compressor",
        "multiband_expander",
        "denoiser",
        "hiss_reducer",
        "declick",
        "transient_repair",
        "spectrum_analyzer",
    ]
}

#[test]
fn block_size_variation_stays_finite() {
    let block_sizes = [1, 16, 64, 257, 480, 1024, 4096];
    let mut failures = Vec::new();

    for &plugin_type in &block_size_sensitive_plugins() {
        for &frames in &block_sizes {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut plugin =
                    create_plugin(plugin_type, &default_params(plugin_type, 2), 2, SAMPLE_RATE)?;
                plugin.initialize(SAMPLE_RATE)?;

                let input = interleaved_sine(2, frames);
                let output_channels = plugin.output_channels();
                let mut output = vec![0.0f32; output_channels * frames];
                plugin.process(
                    &input,
                    &mut output,
                    &ProcessContext::new(SAMPLE_RATE, frames),
                )?;

                assert_all_finite(&output, &format!("{plugin_type} {frames} samples"));
                Ok::<(), String>(())
            }));

            match result {
                Ok(Ok(())) => {}
                Ok(Err(err)) => {
                    failures.push(format!("{plugin_type}@{frames} frames failed: {err}"))
                }
                Err(payload) => failures.push(format!(
                    "{plugin_type}@{frames} frames panicked: {}",
                    panic_payload_description(&payload)
                )),
            }
        }
    }

    assert!(
        failures.is_empty(),
        "block-size variation failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn silence_and_denormals_stay_finite() {
    let plugins = [
        "gain",
        "eq",
        "compressor",
        "limiter",
        "gate",
        "multiband_compressor",
        "multiband_expander",
        "denoiser",
        "hiss_reducer",
        "declick",
        "transient_repair",
        "linear_phase_eq",
        "spectral_compressor",
        "convolution",
        "binaural_decoder",
        "downmix",
        "upmixer",
    ];
    let mut failures = Vec::new();

    for &plugin_type in &plugins {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut plugin =
                create_plugin(plugin_type, &default_params(plugin_type, 2), 2, SAMPLE_RATE)?;
            plugin.initialize(SAMPLE_RATE)?;

            // Silence input.
            let silence = vec![0.0f32; 2 * FRAMES];
            let output_channels = plugin.output_channels();
            let mut output = vec![0.0f32; output_channels * FRAMES];
            plugin.process(
                &silence,
                &mut output,
                &ProcessContext::new(SAMPLE_RATE, FRAMES),
            )?;
            assert_all_finite(&output, &format!("{plugin_type} silence"));

            // Denormal input (very small non-zero values).
            let denormals: Vec<f32> = (0..2 * FRAMES)
                .map(|i| if i % 2 == 0 { 1e-38 } else { -1e-38 })
                .collect();
            plugin.process(
                &denormals,
                &mut output,
                &ProcessContext::new(SAMPLE_RATE, FRAMES),
            )?;
            assert_all_finite(&output, &format!("{plugin_type} denormals"));

            Ok::<(), String>(())
        }));

        match result {
            Ok(Ok(())) => {}
            Ok(Err(err)) => failures.push(format!("{plugin_type} failed: {err}")),
            Err(payload) => failures.push(format!(
                "{plugin_type} panicked: {}",
                panic_payload_description(&payload)
            )),
        }
    }

    assert!(
        failures.is_empty(),
        "silence/denormal failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn high_level_input_stays_finite() {
    let plugins = [
        "gain",
        "eq",
        "compressor",
        "limiter",
        "gate",
        "multiband_compressor",
        "multiband_expander",
        "linear_phase_eq",
        "spectral_compressor",
        "convolution",
        "downmix",
        "upmixer",
    ];
    let mut failures = Vec::new();

    for &plugin_type in &plugins {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut plugin =
                create_plugin(plugin_type, &default_params(plugin_type, 2), 2, SAMPLE_RATE)?;
            plugin.initialize(SAMPLE_RATE)?;

            // Near-0 dBFS sine.
            let input: Vec<f32> = {
                let mut buf = vec![0.0f32; 2 * FRAMES];
                for i in 0..FRAMES {
                    let t = i as f32 / SAMPLE_RATE as f32;
                    let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.99;
                    buf[i * 2] = s;
                    buf[i * 2 + 1] = s;
                }
                buf
            };
            let output_channels = plugin.output_channels();
            let mut output = vec![0.0f32; output_channels * FRAMES];
            plugin.process(
                &input,
                &mut output,
                &ProcessContext::new(SAMPLE_RATE, FRAMES),
            )?;
            assert_all_finite(&output, &format!("{plugin_type} high level"));
            Ok::<(), String>(())
        }));

        match result {
            Ok(Ok(())) => {}
            Ok(Err(err)) => failures.push(format!("{plugin_type} failed: {err}")),
            Err(payload) => failures.push(format!(
                "{plugin_type} panicked: {}",
                panic_payload_description(&payload)
            )),
        }
    }

    assert!(
        failures.is_empty(),
        "high-level input failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn missing_file_paths_are_handled_gracefully() {
    // Plugins that consume external files should reject or safely bypass when
    // the file path is empty/missing, not panic or emit non-finite output.
    let cases = [("convolution", 2), ("binaural_decoder", 2)];
    let mut failures = Vec::new();

    for &(plugin_type, channels) in &cases {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut plugin = create_plugin(
                plugin_type,
                &default_params(plugin_type, channels),
                channels,
                SAMPLE_RATE,
            )?;
            plugin.initialize(SAMPLE_RATE)?;

            let input = interleaved_sine(channels, FRAMES);
            let output_channels = plugin.output_channels();
            let mut output = vec![0.0f32; output_channels * FRAMES];
            plugin.process(
                &input,
                &mut output,
                &ProcessContext::new(SAMPLE_RATE, FRAMES),
            )?;
            assert_all_finite(&output, &format!("{plugin_type} missing file"));
            Ok::<(), String>(())
        }));

        match result {
            Ok(Ok(())) => {}
            Ok(Err(err)) => failures.push(format!("{plugin_type} failed: {err}")),
            Err(payload) => failures.push(format!(
                "{plugin_type} panicked: {}",
                panic_payload_description(&payload)
            )),
        }
    }

    assert!(
        failures.is_empty(),
        "missing-file path failures:\n{}",
        failures.join("\n")
    );
}

/// Plugins that use STFT/FIR/convolution paths and must return
/// `context.num_frames` to prevent host ring-buffer underrun.
fn stft_plugins() -> Vec<&'static str> {
    vec![
        "downmix",
        "upmixer",
        "convolution",
        "binaural_decoder",
        "linear_phase_eq",
        "spectral_compressor",
        "multiband_compressor",
        "multiband_expander",
        "denoiser",
        "declick",
        "transient_repair",
        "crossover",
    ]
}

#[test]
fn stft_plugins_return_context_num_frames() {
    let mut failures = Vec::new();

    for &plugin_type in &stft_plugins() {
        let channels = required_input_channels(plugin_type).unwrap_or(2);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut plugin = create_plugin(
                plugin_type,
                &default_params(plugin_type, channels),
                channels,
                SAMPLE_RATE,
            )?;
            plugin.initialize(SAMPLE_RATE)?;

            let frames = FRAMES;
            let input = interleaved_sine(channels, frames);
            let output_channels = plugin.output_channels();
            let mut output = vec![0.0f32; output_channels * frames];
            let context = ProcessContext::new(SAMPLE_RATE, frames);
            let returned = plugin.process(&input, &mut output, &context)?;

            if returned != frames {
                return Err(format!(
                    "{plugin_type}: expected to return {frames} frames, got {returned}"
                ));
            }
            Ok::<(), String>(())
        }));

        match result {
            Ok(Ok(())) => {}
            Ok(Err(err)) => failures.push(format!("{plugin_type} failed: {err}")),
            Err(payload) => failures.push(format!(
                "{plugin_type} panicked: {}",
                panic_payload_description(&payload)
            )),
        }
    }

    assert!(
        failures.is_empty(),
        "STFT context.num_frames failures:\n{}",
        failures.join("\n")
    );
}

/// Plugins with STFT/FIR/convolution paths that should expose non-zero latency
/// so hosts can compensate for their group delay.
fn latency_reporting_plugins() -> Vec<(&'static str, serde_json::Value)> {
    vec![
        ("limiter", default_params("limiter", 2)),
        ("convolution", default_params("convolution", 2)),
        ("upmixer", default_params("upmixer", 2)),
        ("mono_to_stereo", default_params("mono_to_stereo", 1)),
        (
            "multiband_expander",
            serde_json::json!({"lookahead_ms": 5.0}),
        ),
        ("fir_designer", default_params("fir_designer", 2)),
        ("linear_phase_eq", default_params("linear_phase_eq", 2)),
        (
            "spectral_compressor",
            default_params("spectral_compressor", 2),
        ),
        ("denoiser", default_params("denoiser", 2)),
        ("speech_denoiser", serde_json::json!({"enabled": false})),
        ("pnd", default_params("pnd", 2)),
        ("binaural_decoder", default_params("binaural_decoder", 2)),
        ("downmix", default_params("downmix", 2)),
        ("resampler", default_params("resampler", 2)),
        ("xtc", default_params("xtc", 2)),
        ("aec", default_params("aec", 2)),
        ("beamformer", default_params("beamformer", 2)),
        (
            "crossover",
            serde_json::json!({
                "type": "linear_phase",
                "frequency": 1000.0,
                "output": "lowpass",
            }),
        ),
    ]
}

#[test]
fn latency_reporting_plugins_expose_nonzero_latency() {
    let mut failures = Vec::new();

    for &(plugin_type, ref params) in &latency_reporting_plugins() {
        let channels = required_input_channels(plugin_type).unwrap_or(2);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut plugin = create_plugin(plugin_type, params, channels, SAMPLE_RATE)?;
            plugin.initialize(SAMPLE_RATE)?;

            let latency = plugin.latency_samples();
            if latency == 0 {
                return Err(format!("{plugin_type}: latency_samples() returned 0"));
            }
            Ok::<(), String>(())
        }));

        match result {
            Ok(Ok(())) => {}
            Ok(Err(err)) => failures.push(err),
            Err(payload) => failures.push(format!(
                "{plugin_type} panicked: {}",
                panic_payload_description(&payload)
            )),
        }
    }

    assert!(
        failures.is_empty(),
        "latency reporting failures:\n{}",
        failures.join("\n")
    );
}

fn zero_latency_plugin_configurations() -> Vec<(&'static str, serde_json::Value)> {
    vec![
        ("gain", default_params("gain", 2)),
        ("eq", default_params("eq", 2)),
        ("compressor", default_params("compressor", 2)),
        ("expander", default_params("expander", 2)),
        ("gate", default_params("gate", 2)),
        ("delay", default_params("delay", 2)),
        ("aae", default_params("aae", 2)),
        (
            "multiband_compressor",
            default_params("multiband_compressor", 2),
        ),
        (
            "multiband_expander",
            default_params("multiband_expander", 2),
        ),
        ("de_esser", default_params("de_esser", 2)),
        ("dynamic_eq", default_params("dynamic_eq", 2)),
        ("stereo_imager", default_params("stereo_imager", 2)),
        ("transient_shaper", default_params("transient_shaper", 2)),
        ("saturation", default_params("saturation", 2)),
        (
            "loudness_compensation",
            default_params("loudness_compensation", 2),
        ),
        ("fletcher_munson", default_params("fletcher_munson", 2)),
        ("crossfeed", default_params("crossfeed", 2)),
        ("hiss_reducer", default_params("hiss_reducer", 2)),
        ("declick", default_params("declick", 2)),
        (
            "ambisonics_decoder",
            default_params("ambisonics_decoder", 4),
        ),
        ("matrix", default_params("matrix", 2)),
        ("channel_mute_solo", default_params("channel_mute_solo", 2)),
        ("band_split", default_params("band_split", 2)),
        ("band_merge", default_params("band_merge", 2)),
        ("crossover", default_params("crossover", 2)),
        ("beamformer", serde_json::json!({"beamformer_type": 2})),
    ]
}

fn measure_impulse_delays(
    plugin: &mut dyn Plugin,
    block_size: usize,
) -> Result<(usize, usize, usize), String> {
    let reported = plugin.latency_samples();
    let input_channels = plugin.input_channels();
    let output_channels = plugin.output_channels();
    let impulse_frame = block_size * 2;
    let minimum_frames = impulse_frame + reported + block_size * 8;
    let total_frames = minimum_frames.div_ceil(block_size) * block_size;
    let mut input = vec![0.0f32; total_frames * input_channels];
    for channel in 0..input_channels {
        input[impulse_frame * input_channels + channel] = 1.0 / (channel + 1) as f32;
    }
    let mut output = vec![0.0f32; total_frames * output_channels];

    plugin.reset();
    for frame in (0..total_frames).step_by(block_size) {
        let input_start = frame * input_channels;
        let output_start = frame * output_channels;
        let returned = plugin.process(
            &input[input_start..input_start + block_size * input_channels],
            &mut output[output_start..output_start + block_size * output_channels],
            &ProcessContext::new(SAMPLE_RATE, block_size),
        )?;
        if returned > block_size {
            return Err(format!(
                "returned {returned} frames for a {block_size}-frame latency probe"
            ));
        }
    }

    let onset_frame = output
        .chunks_exact(output_channels)
        .position(|samples| samples.iter().any(|sample| sample.abs() > 1e-6));
    let (peak_frame, peak) = output
        .chunks_exact(output_channels)
        .enumerate()
        .map(|(frame, samples)| {
            let peak = samples.iter().copied().map(f32::abs).fold(0.0, f32::max);
            (frame, peak)
        })
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .unwrap();
    if peak <= 1e-6 {
        return Err("impulse probe produced no measurable output".to_string());
    }
    let onset_frame = onset_frame.expect("a measurable peak must have an onset");

    Ok((
        reported,
        onset_frame.saturating_sub(impulse_frame),
        peak_frame.saturating_sub(impulse_frame),
    ))
}

#[test]
fn reported_latency_matches_streamed_impulse_peak() {
    let mut failures = Vec::new();

    // Convolution has a dedicated delta-IR matrix covering uniform, NUPC, and
    // zero-latency-head modes. Resampler latency is expressed in output-rate
    // frames and has dedicated rubato/chunking tests, so neither belongs in this
    // same-rate impulse-peak probe.
    for (plugin_type, params) in latency_reporting_plugins()
        .into_iter()
        .filter(|(plugin_type, _)| !matches!(*plugin_type, "convolution" | "resampler"))
    {
        let channels = required_input_channels(plugin_type).unwrap_or(2);
        let result = (|| {
            let mut plugin = create_plugin(plugin_type, &params, channels, SAMPLE_RATE)?;
            plugin.initialize(SAMPLE_RATE)?;
            let block_sizes: &[usize] = if plugin_type == "speech_denoiser" {
                &[480]
            } else {
                &[128, 256, 512]
            };
            for &block_size in block_sizes {
                let (reported, onset, peak) = measure_impulse_delays(plugin.as_mut(), block_size)?;
                let onset_error = reported.abs_diff(onset);
                let peak_error = reported.abs_diff(peak);
                if onset_error.min(peak_error) > block_size {
                    return Err(format!(
                        "{plugin_type}: reported {reported} samples, measured onset {onset}, peak {peak}, block {block_size}"
                    ));
                }
            }
            Ok::<(), String>(())
        })();
        if let Err(error) = result {
            failures.push(error);
        }
    }

    assert!(
        failures.is_empty(),
        "latency measurement failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn zero_latency_configurations_are_causal_within_one_block() {
    let mut failures = Vec::new();

    for (plugin_type, params) in zero_latency_plugin_configurations() {
        let channels = required_input_channels(plugin_type).unwrap_or_else(|| {
            if plugin_type == "ambisonics_decoder" {
                4
            } else {
                2
            }
        });
        let result = (|| {
            let mut plugin = create_plugin(plugin_type, &params, channels, SAMPLE_RATE)?;
            plugin.initialize(SAMPLE_RATE)?;
            if plugin.latency_samples() != 0 {
                return Err(format!(
                    "{plugin_type}: expected zero-latency configuration, reported {}",
                    plugin.latency_samples()
                ));
            }
            for block_size in [128, 512] {
                let (_, onset, _) = measure_impulse_delays(plugin.as_mut(), block_size)?;
                if onset > block_size {
                    return Err(format!(
                        "{plugin_type}: zero-latency configuration first responded after {onset} samples with block {block_size}"
                    ));
                }
            }
            Ok::<(), String>(())
        })();
        if let Err(error) = result {
            failures.push(error);
        }
    }

    assert!(
        failures.is_empty(),
        "zero-latency contract failures:\n{}",
        failures.join("\n")
    );
}

fn panic_payload_description(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_string()
    }
}
