//! Cross-cutting integration test: instantiate every built-in plugin type through
//! the factory with default parameters, exercise parameter discovery and
//! round-trips, and ensure processing produces finite output.
//!
//! This test does not replace per-plugin integration tests; it catches plugin
//! types that lack dedicated coverage and ensures new parameters are at least
//! reachable through `set_parameter`/`get_parameter`.

use sotf_host::param_specs::UpdateMode;
use sotf_host::{ParameterId, ParameterValue, Plugin, ProcessContext};
use sotf_plugins::factory::{create_plugin, supported_plugin_types};

const SAMPLE_RATE: u32 = 48_000;
// RNNoise requires block sizes that are a multiple of 480.
const FRAMES: usize = 480;

/// Plugin types that require a specific input channel count.
fn channels_for_type(plugin_type: &str) -> usize {
    match plugin_type {
        "upmixer"
        | "crossfeed"
        | "xtc"
        | "crosstalk_cancellation"
        | "aae"
        | "active_acoustic_enhancement"
        | "ab_compare"
        | "ab" => 2,
        "binaural_decoder" => 2,
        "aec" => 2,
        "beamformer" => 4,
        "ambisonics_decoder" => 4,
        "mono_to_stereo" => 1,
        "resampler" => 2,
        "loudness_monitor" => 2,
        "spectrum_analyzer" => 2,
        _ => 2,
    }
}

/// Special default parameters required by some plugin types.
fn default_params(plugin_type: &str) -> serde_json::Value {
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
            "input_channels": 2,
            "output_channels": 2,
            "matrix": [1.0, 0.0, 0.0, 1.0],
        }),
        "binaural_decoder" => serde_json::json!({
            "input_channels": 2,
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
            "input_channels": 2,
            "output_channels": 2,
            "matrix": [1.0, 0.0, 0.0, 1.0],
        }),
        "band_split" => serde_json::json!({
            "bands": 2,
            "crossover_frequencies": [1000.0],
        }),
        "band_merge" => serde_json::json!({
            "bands": 2,
        }),
        "aec" => serde_json::json!({
            "filter_length_ms": 100.0,
        }),
        "beamformer" => serde_json::json!({
            "num_mics": 4,
            "mic_spacing_m": 0.05,
        }),
        "ambisonics_decoder" => serde_json::json!({
            "order": 1,
            "output_channels": 4,
        }),
        _ => serde_json::json!({}),
    }
}

fn interleaved_sine(channels: usize, frames: usize, freq: f32) -> Vec<f32> {
    let mut buf = vec![0.0f32; frames * channels];
    for i in 0..frames {
        let t = i as f32 / SAMPLE_RATE as f32;
        let s = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.25;
        for ch in 0..channels {
            buf[i * channels + ch] = s;
        }
    }
    buf
}

fn process_plugin(plugin: &mut Box<dyn Plugin>, channels: usize) {
    plugin.initialize(SAMPLE_RATE).expect("initialize failed");
    process_initialized_plugin(plugin, channels).expect("process failed");
}

fn process_initialized_plugin(plugin: &mut Box<dyn Plugin>, channels: usize) -> Result<(), String> {
    let input = interleaved_sine(channels, FRAMES, 440.0);
    let output_channels = plugin.output_channels();
    // Variable-rate plugins can need more storage than one output frame per
    // input frame. Ask the plugin for its bounded capacity; fixed-rate
    // plugins continue to receive an exact frame-sized buffer.
    let output_frames = plugin.output_frames_for_input(FRAMES).max(FRAMES);
    let mut output = vec![0.0f32; output_channels * output_frames];
    plugin.process(
        &input,
        &mut output,
        &ProcessContext::new(SAMPLE_RATE, FRAMES),
    )?;

    if output.iter().any(|sample| !sample.is_finite()) {
        return Err(format!(
            "{} produced non-finite samples",
            plugin.info().name
        ));
    }
    Ok(())
}

#[test]
fn all_plugins_instantiate_with_defaults() {
    let mut failures = Vec::new();

    for plugin_type in supported_plugin_types() {
        // Skip external plugins (require filesystem artifacts) and HAL plugins
        // (require macOS + feature flag).
        if plugin_type == "external"
            || plugin_type == "external_plugin"
            || plugin_type == "hal_input"
            || plugin_type == "hal_output"
        {
            continue;
        }

        let channels = channels_for_type(plugin_type);
        let params = default_params(plugin_type);

        match create_plugin(plugin_type, &params, channels, SAMPLE_RATE) {
            Ok(mut plugin) => {
                process_plugin(&mut plugin, channels);
            }
            Err(err) => {
                failures.push(format!("{plugin_type}: {err}"));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "plugins failed to instantiate or process:\n{}",
        failures.join("\n")
    );
}

/// Parameter ids whose setter is expected to reject their legal current value.
/// Each entry is `(plugin_type, param_id, reason)`. Structural parameters are
/// changed through serialized host rebuilds, not the realtime setter.
const EXPECTED_SET_REJECTIONS: &[(&str, &str, &str)] = &[
    (
        "resampler",
        "ratio",
        "ratio cannot be changed when dynamic_ratio is disabled",
    ),
    (
        "band_merge",
        "reconstruction_error_db",
        "parameter is reported but not accepted by set_parameter",
    ),
    (
        "convolution",
        "use_nupc",
        "structural parameter changed through a host rebuild",
    ),
    (
        "convolution",
        "zero_latency_head",
        "structural parameter changed through a host rebuild",
    ),
    (
        "convolution",
        "head_taps",
        "structural parameter changed through a host rebuild",
    ),
    (
        "beamformer",
        "beamformer_type",
        "structural algorithm selection changed through a host rebuild",
    ),
];

/// Parameters that accept a no-op setter but reject a changed value while the
/// live instance is initialized. They must not be exercised by the realtime
/// update matrix; the host changes them by recreating the plugin.
const EXPECTED_LIVE_UPDATE_REJECTIONS: &[(&str, &str, &str)] = &[
    (
        "spectral_compressor",
        "fft_size",
        "structural FFT size changed through a host rebuild",
    ),
    (
        "spectrum_analyzer",
        "num_bins",
        "setup-only display shape changed by recreating the analyzer",
    ),
    (
        "spectrum_analyzer",
        "min_freq",
        "setup-only frequency bounds changed by recreating the analyzer",
    ),
    (
        "spectrum_analyzer",
        "max_freq",
        "setup-only frequency bounds changed by recreating the analyzer",
    ),
];

#[test]
fn all_plugins_expose_parameters_and_roundtrip_legal_values() {
    let mut unexpected_failures = Vec::new();
    let mut expected_rejections_seen = Vec::new();

    for plugin_type in supported_plugin_types() {
        if plugin_type == "external"
            || plugin_type == "external_plugin"
            || plugin_type == "hal_input"
            || plugin_type == "hal_output"
        {
            continue;
        }

        let channels = channels_for_type(plugin_type);
        let params = default_params(plugin_type);

        let mut plugin = match create_plugin(plugin_type, &params, channels, SAMPLE_RATE) {
            Ok(p) => p,
            Err(err) => {
                unexpected_failures.push(format!("{plugin_type}: instantiate failed: {err}"));
                continue;
            }
        };

        for param in plugin.parameters() {
            let id = ParameterId::from(param.id.as_str());
            if let Some(value) = plugin.get_parameter(&id)
                && let Err(err) = plugin.set_parameter(id, value)
            {
                let key = (plugin_type, param.id.as_str());
                let is_expected = EXPECTED_SET_REJECTIONS
                    .iter()
                    .any(|(t, p, _)| *t == key.0 && *p == key.1);
                if is_expected {
                    expected_rejections_seen.push(format!("{plugin_type}/{}", param.id));
                } else {
                    unexpected_failures.push(format!(
                        "{plugin_type}/{}: round-trip failed: {err}",
                        param.id
                    ));
                }
            }
        }

        process_plugin(&mut plugin, channels);
    }

    assert!(
        unexpected_failures.is_empty(),
        "unexpected parameter round-trip failures:\n{}",
        unexpected_failures.join("\n")
    );

    // Ensure the expected-rejection list does not drift from reality.
    assert_eq!(
        expected_rejections_seen.len(),
        EXPECTED_SET_REJECTIONS.len(),
        "expected {} documented setter rejections, got {}; some may have been fixed",
        EXPECTED_SET_REJECTIONS.len(),
        expected_rejections_seen.len()
    );
}

#[test]
fn all_realtime_numeric_and_boolean_parameters_tolerate_rapid_updates() {
    let mut failures = Vec::new();
    let mut exercised = 0usize;

    for plugin_type in supported_plugin_types() {
        if plugin_type == "external"
            || plugin_type == "external_plugin"
            || plugin_type == "hal_input"
            || plugin_type == "hal_output"
        {
            continue;
        }

        let channels = channels_for_type(plugin_type);
        let params = default_params(plugin_type);
        let mut plugin = match create_plugin(plugin_type, &params, channels, SAMPLE_RATE) {
            Ok(plugin) => plugin,
            Err(error) => {
                failures.push(format!("{plugin_type}: instantiate failed: {error}"));
                continue;
            }
        };
        if let Err(error) = plugin.initialize(SAMPLE_RATE) {
            failures.push(format!("{plugin_type}: initialize failed: {error}"));
            continue;
        }

        for parameter in plugin.parameters() {
            if parameter.update_mode != UpdateMode::Realtime
                || EXPECTED_SET_REJECTIONS
                    .iter()
                    .any(|(kind, id, _)| *kind == plugin_type && *id == parameter.id.as_str())
                || EXPECTED_LIVE_UPDATE_REJECTIONS
                    .iter()
                    .any(|(kind, id, _)| *kind == plugin_type && *id == parameter.id.as_str())
            {
                continue;
            }

            let values = match (
                parameter.min_value.as_ref(),
                parameter.max_value.as_ref(),
                &parameter.default_value,
            ) {
                (Some(ParameterValue::Float(minimum)), Some(ParameterValue::Float(maximum)), _)
                    if minimum < maximum =>
                {
                    vec![
                        ParameterValue::Float(*minimum),
                        ParameterValue::Float(*maximum),
                    ]
                }
                (Some(ParameterValue::Int(minimum)), Some(ParameterValue::Int(maximum)), _)
                    if minimum < maximum =>
                {
                    vec![ParameterValue::Int(*minimum), ParameterValue::Int(*maximum)]
                }
                (_, _, ParameterValue::Bool(default)) => vec![
                    ParameterValue::Bool(!default),
                    ParameterValue::Bool(*default),
                ],
                _ => continue,
            };
            let Some(original) = plugin.get_parameter(&parameter.id) else {
                failures.push(format!(
                    "{plugin_type}/{}: realtime parameter has no readable current value",
                    parameter.id
                ));
                continue;
            };

            let mut parameter_failed = false;
            for update in 0..6 {
                let value = values[update % values.len()].clone();
                if let Err(error) = plugin.set_parameter(parameter.id.clone(), value) {
                    failures.push(format!(
                        "{plugin_type}/{}: rapid update failed: {error}",
                        parameter.id
                    ));
                    parameter_failed = true;
                    break;
                }
                if let Err(error) = process_initialized_plugin(&mut plugin, channels) {
                    failures.push(format!(
                        "{plugin_type}/{}: processing after rapid update failed: {error}",
                        parameter.id
                    ));
                    parameter_failed = true;
                    break;
                }
            }
            if let Err(error) = plugin.set_parameter(parameter.id.clone(), original) {
                failures.push(format!(
                    "{plugin_type}/{}: restoring the pre-test value failed: {error}",
                    parameter.id
                ));
                parameter_failed = true;
            }
            if !parameter_failed {
                exercised += 1;
            }
        }
    }

    assert!(
        exercised > 0,
        "the rapid-update matrix exercised no parameters"
    );
    assert!(
        failures.is_empty(),
        "rapid realtime-parameter failures:\n{}",
        failures.join("\n")
    );
}

/// Plugins whose `set_parameter` silently ignores unknown parameter ids.
/// New plugin types should reject unknown parameters; this list only documents
/// existing behavior so the cross-cutting test stays green while fixes are
/// planned per-plugin.
const KNOWN_TO_ACCEPT_UNKNOWN_PARAMS: &[&str] = &[];

#[test]
fn all_plugins_reject_unknown_parameters() {
    let mut unexpected_acceptors = Vec::new();
    let mut known_acceptors = Vec::new();

    for plugin_type in supported_plugin_types() {
        if plugin_type == "external"
            || plugin_type == "external_plugin"
            || plugin_type == "hal_input"
            || plugin_type == "hal_output"
        {
            continue;
        }

        let channels = channels_for_type(plugin_type);
        let params = default_params(plugin_type);

        let mut plugin = match create_plugin(plugin_type, &params, channels, SAMPLE_RATE) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let result = plugin.set_parameter(
            ParameterId::from("__definitely_not_a_real_parameter__"),
            ParameterValue::Float(1.0),
        );

        if result.is_ok() {
            if KNOWN_TO_ACCEPT_UNKNOWN_PARAMS.contains(&plugin_type) {
                known_acceptors.push(plugin_type.to_string());
            } else {
                unexpected_acceptors.push(plugin_type.to_string());
            }
        }
    }

    assert!(
        unexpected_acceptors.is_empty(),
        "plugins unexpectedly accepted an unknown parameter: {}",
        unexpected_acceptors.join(", ")
    );

    // Defensive check: if a plugin is fixed, remove it from the known list.
    assert_eq!(
        known_acceptors.len(),
        KNOWN_TO_ACCEPT_UNKNOWN_PARAMS.len(),
        "some plugins in KNOWN_TO_ACCEPT_UNKNOWN_PARAMS now reject unknown params; update the list"
    );
}
