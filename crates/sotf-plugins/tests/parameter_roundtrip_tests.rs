//! Per-plugin parameter round-trip tests (Phase 2.1).
//!
//! Discovers every built-in plugin type from the shared factory, instantiates it
//! with safe defaults, and verifies that each exposed parameter can be set to a
//! deterministic legal value and then read back unchanged (within floating-point
//! tolerance). A small block of silence is processed after every parameter change
//! to ensure the new value is handled by the real-time code path.
//!
//! Each parameter is tested against a freshly-created plugin instance so that a
//! crash or error in one parameter does not taint the remaining parameters.
//!
//! String-typed parameters are round-tripped using their current value rather
//! than an arbitrary string, because many strings encode file paths, enum
//! choices, or serialized structures that cannot be safely invented here.
//! Parameters that are read-only or known to require a concrete backend resource
//! are skipped with an explanatory comment rather than failing the test.

use sotf_host::param_specs::UpdateMode;
use sotf_host::{Parameter, ParameterId, ParameterValue, Plugin, ProcessContext};
use sotf_plugins::factory::{create_plugin, supported_plugin_types};

const SAMPLE_RATE: u32 = 48_000;
// 480 keeps frame-size-sensitive plugins (e.g. speech denoiser) happy.
const FRAMES: usize = 480;

/// Pick a sensible input channel count for each plugin type.
fn channels_for_type(plugin_type: &str) -> usize {
    match plugin_type {
        "upmixer"
        | "crossfeed"
        | "xtc"
        | "crosstalk_cancellation"
        | "aae"
        | "active_acoustic_enhancement"
        | "ab_compare"
        | "ab"
        | "binaural_decoder"
        | "aec" => 2,
        "beamformer" => 4,
        "ambisonics_decoder" => 4,
        "mono_to_stereo" => 1,
        "loudness_monitor" => 2,
        "spectrum_analyzer" => 2,
        _ => 2,
    }
}

/// Some plugin types need non-empty default parameters to instantiate cleanly.
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
            "num_bands": 2,
            "frequency": 1000.0,
            "type": "LR24",
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

#[derive(Clone, Copy, Debug)]
enum ExceptionContract {
    ConditionalSetter,
    ReadOnly,
    Trigger,
}

struct ParameterException {
    plugin_type: &'static str,
    parameter_id: &'static str,
    contract: ExceptionContract,
    reason: &'static str,
}

/// Non-standard parameter contracts. These are executable specifications in
/// `documented_parameter_exceptions_are_exact_and_enforced`, not silent skips.
const PARAMETER_EXCEPTIONS: &[ParameterException] = &[
    ParameterException {
        plugin_type: "resampler",
        parameter_id: "ratio",
        contract: ExceptionContract::ConditionalSetter,
        reason: "ratio is mutable only while dynamic_ratio is enabled",
    },
    ParameterException {
        plugin_type: "band_merge",
        parameter_id: "reconstruction_error_db",
        contract: ExceptionContract::ReadOnly,
        reason: "read-only reconstruction meter",
    },
    ParameterException {
        plugin_type: "gate",
        parameter_id: "sidechain_external",
        contract: ExceptionContract::ConditionalSetter,
        reason: "sidechain topology requires a graph rebuild",
    },
    ParameterException {
        plugin_type: "spectral_compressor",
        parameter_id: "fft_size",
        contract: ExceptionContract::ConditionalSetter,
        reason: "FFT topology requires a graph rebuild",
    },
    ParameterException {
        plugin_type: "spectrum_analyzer",
        parameter_id: "num_bins",
        contract: ExceptionContract::ConditionalSetter,
        reason: "display shape is setup-only",
    },
    ParameterException {
        plugin_type: "spectrum_analyzer",
        parameter_id: "min_freq",
        contract: ExceptionContract::ConditionalSetter,
        reason: "frequency bounds are setup-only",
    },
    ParameterException {
        plugin_type: "spectrum_analyzer",
        parameter_id: "max_freq",
        contract: ExceptionContract::ConditionalSetter,
        reason: "frequency bounds are setup-only",
    },
    ParameterException {
        plugin_type: "beamformer",
        parameter_id: "beamformer_type",
        contract: ExceptionContract::ConditionalSetter,
        reason: "algorithm topology requires a graph rebuild",
    },
    ParameterException {
        plugin_type: "denoiser",
        parameter_id: "clear_profile",
        contract: ExceptionContract::Trigger,
        reason: "one-shot trigger resets after execution",
    },
    ParameterException {
        plugin_type: "wiener_denoiser",
        parameter_id: "clear_profile",
        contract: ExceptionContract::Trigger,
        reason: "alias of denoiser one-shot trigger",
    },
];

fn is_skipped(plugin_type: &str, param_id: &str) -> bool {
    PARAMETER_EXCEPTIONS
        .iter()
        .any(|entry| entry.plugin_type == plugin_type && entry.parameter_id == param_id)
}

#[test]
fn documented_parameter_exceptions_are_exact_and_enforced() {
    for exception in PARAMETER_EXCEPTIONS {
        assert!(!exception.reason.is_empty());
        let channels = channels_for_type(exception.plugin_type);
        let params = default_params(exception.plugin_type);
        let mut plugin = create_and_init_plugin(exception.plugin_type, &params, channels)
            .unwrap_or_else(|error| panic!("{}: {error}", exception.plugin_type));
        let parameter = plugin
            .parameters()
            .into_iter()
            .find(|parameter| parameter.id.as_str() == exception.parameter_id)
            .unwrap_or_else(|| {
                panic!(
                    "{}/{} no longer exists; remove its exception",
                    exception.plugin_type, exception.parameter_id
                )
            });
        let id = parameter.id.clone();

        match exception.contract {
            ExceptionContract::ConditionalSetter => {
                let current = plugin
                    .get_parameter(&id)
                    .expect("parameter must be readable");
                let changed = deterministic_value(&parameter, Some(&current));
                assert!(
                    plugin.set_parameter(id, changed).is_err(),
                    "{}/{} unexpectedly became directly writable",
                    exception.plugin_type,
                    exception.parameter_id
                );
            }
            ExceptionContract::ReadOnly => {
                let current = plugin
                    .get_parameter(&id)
                    .expect("parameter must be readable");
                assert!(
                    plugin.set_parameter(id, current).is_err(),
                    "{}/{} unexpectedly became directly writable",
                    exception.plugin_type,
                    exception.parameter_id
                );
            }
            ExceptionContract::Trigger => {
                plugin
                    .set_parameter(id.clone(), ParameterValue::Bool(true))
                    .unwrap();
                assert_eq!(plugin.get_parameter(&id), Some(ParameterValue::Bool(false)));
            }
        }
    }
}

/// Build a deterministic legal value for `param`.
///
/// For numeric parameters the value is chosen strictly inside the declared
/// [min, max] range. Booleans are toggled. Strings reuse the current value so
/// that file-path / enum / JSON parameters are not corrupted.
fn deterministic_value(param: &Parameter, current: Option<&ParameterValue>) -> ParameterValue {
    match &param.default_value {
        ParameterValue::Float(_) => {
            let min = param.min_value.as_ref().and_then(|v| v.as_float());
            let max = param.max_value.as_ref().and_then(|v| v.as_float());
            let value = match (min, max) {
                (Some(min), Some(max)) if min < max => min + 0.37 * (max - min),
                (Some(min), _) => min,
                _ => current.and_then(|v| v.as_float()).unwrap_or_else(|| {
                    param
                        .default_value
                        .as_float()
                        .expect("float parameter has no float default")
                }),
            };
            ParameterValue::Float(value)
        }
        ParameterValue::Int(_) => {
            let min = param.min_value.as_ref().and_then(|v| v.as_int());
            let max = param.max_value.as_ref().and_then(|v| v.as_int());
            let value = match (min, max) {
                (Some(min), Some(max)) if min < max => {
                    let range = max - min;
                    min + (range * 3) / 7
                }
                (Some(min), _) => min,
                _ => current.and_then(|v| v.as_int()).unwrap_or_else(|| {
                    param
                        .default_value
                        .as_int()
                        .expect("int parameter has no int default")
                }),
            };
            ParameterValue::Int(value)
        }
        ParameterValue::Bool(current_bool) => {
            let value = current.and_then(|v| v.as_bool()).unwrap_or(*current_bool);
            ParameterValue::Bool(!value)
        }
        ParameterValue::String(_) => {
            let value = current
                .and_then(|v| v.as_string())
                .map(|s| s.to_string())
                .unwrap_or_default();
            ParameterValue::String(value)
        }
    }
}

fn assert_values_equal(set: &ParameterValue, got: &ParameterValue) {
    match (set, got) {
        (ParameterValue::Float(a), ParameterValue::Float(b)) => {
            let scale = a.abs().max(b.abs()).max(1.0);
            assert!(
                (a - b).abs() <= 1e-6 * scale,
                "float round-trip mismatch: set {a}, got {b}"
            );
        }
        _ => assert_eq!(set, got, "parameter round-trip mismatch"),
    }
}

fn process_one_block(
    plugin: &mut Box<dyn Plugin>,
    input_channels: usize,
    output_channels: usize,
) -> Result<(), String> {
    let input = vec![0.0f32; input_channels * FRAMES];
    let output_frames = plugin.output_frames_for_input(FRAMES);
    let mut output = vec![0.0f32; output_channels * output_frames];
    plugin.process(
        &input,
        &mut output,
        &ProcessContext::new(SAMPLE_RATE, FRAMES),
    )?;
    Ok(())
}

fn create_and_init_plugin(
    plugin_type: &str,
    params: &serde_json::Value,
    channels: usize,
) -> Result<Box<dyn Plugin>, String> {
    let mut plugin = create_plugin(plugin_type, params, channels, SAMPLE_RATE)
        .map_err(|e| format!("instantiate failed: {e}"))?;
    plugin
        .initialize(SAMPLE_RATE)
        .map_err(|e| format!("initialize failed: {e}"))?;
    Ok(plugin)
}

#[test]
fn all_plugins_roundtrip_parameters() {
    let mut failures = Vec::new();

    for plugin_type in supported_plugin_types() {
        // Skip plugin types that need external resources or macOS HAL.
        if plugin_type == "external"
            || plugin_type == "external_plugin"
            || plugin_type == "hal_input"
            || plugin_type == "hal_output"
        {
            continue;
        }

        let channels = channels_for_type(plugin_type);
        let params = default_params(plugin_type);

        // Discover parameters from a probe instance; some plugins have no
        // exposed parameters at all.
        let probe = match create_and_init_plugin(plugin_type, &params, channels) {
            Ok(p) => p,
            Err(err) => {
                failures.push(format!("{plugin_type}: {err}"));
                continue;
            }
        };
        let parameters = probe.parameters();
        drop(probe);
        if parameters.is_empty() {
            continue;
        }

        for param in &parameters {
            let param_id_str = param.id.as_str();
            let id = ParameterId::from(param_id_str);

            // Structural parameters are serialized into a new plugin instance by
            // the host. They are discoverable/readable here, but must not be
            // exercised through a live process block with stale topology or
            // scratch buffers.
            if param.update_mode == UpdateMode::Structural {
                if plugin_parameter_is_missing(plugin_type, &params, channels, &id) {
                    failures.push(format!(
                        "{plugin_type}/{param_id_str}: structural parameter is not readable"
                    ));
                }
                continue;
            }

            if is_skipped(plugin_type, param_id_str) {
                continue;
            }

            let mut plugin = match create_and_init_plugin(plugin_type, &params, channels) {
                Ok(p) => p,
                Err(err) => {
                    failures.push(format!("{plugin_type}/{param_id_str}: {err}"));
                    continue;
                }
            };

            let current = plugin.get_parameter(&id);
            let test_value = deterministic_value(param, current.as_ref());
            let input_channels = plugin.input_channels();
            let output_channels = plugin.output_channels();

            let roundtrip_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                move || -> Result<(), String> {
                    plugin
                        .set_parameter(id.clone(), test_value.clone())
                        .map_err(|e| format!("set failed for {test_value:?}: {e}"))?;
                    process_one_block(&mut plugin, input_channels, output_channels)
                        .map_err(|e| format!("process failed: {e}"))?;
                    let got = plugin
                        .get_parameter(&id)
                        .ok_or("get_parameter returned None after set")?;
                    assert_values_equal(&test_value, &got);
                    Ok(())
                },
            ));

            match roundtrip_result {
                Ok(Ok(())) => {}
                Ok(Err(err)) => {
                    failures.push(format!("{plugin_type}/{param_id_str}: {err}"));
                }
                Err(payload) => {
                    let msg = if let Some(s) = payload.downcast_ref::<String>() {
                        s.clone()
                    } else if let Some(s) = payload.downcast_ref::<&str>() {
                        s.to_string()
                    } else {
                        "panicked during round-trip".to_string()
                    };
                    failures.push(format!("{plugin_type}/{param_id_str}: {msg}"));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "parameter round-trip failures:\n{}",
        failures.join("\n")
    );
}

fn plugin_parameter_is_missing(
    plugin_type: &str,
    params: &serde_json::Value,
    channels: usize,
    id: &ParameterId,
) -> bool {
    create_and_init_plugin(plugin_type, params, channels)
        .ok()
        .and_then(|plugin| plugin.get_parameter(id))
        .is_none()
}
