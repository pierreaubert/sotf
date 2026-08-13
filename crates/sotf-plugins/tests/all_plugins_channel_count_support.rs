//! Cross-cutting integration test: instantiate every built-in plugin type through
//! the factory at mono, stereo, first-order ambisonic, 5.1, 7.1, and 7.1.4
//! input widths.
//!
//! For each combination the test asserts that the plugin either:
//!   - accepts the configuration, initializes, processes a short sine burst, and
//!     reports a non-zero output channel count; or
//!   - returns a clear error (no panic).
//!
//! Channel-changing plugins are documented explicitly.

use sotf_host::ProcessContext;
use sotf_plugins::factory::{
    PLUGIN_CATALOG, PluginChannelOutputModel, PluginSupportedInputLayouts, create_plugin,
};

const SAMPLE_RATE: u32 = 48_000;
const FRAMES: usize = 480;
const CHANNEL_COUNTS: &[usize] = &[1, 2, 4, 6, 8, 12];

/// Plugins that are intentionally skipped from this test (external plugins and
/// macOS HAL plugins require artifacts or platform features unavailable here).
fn is_skipped(plugin_type: &str) -> bool {
    matches!(
        plugin_type,
        "external" | "external_plugin" | "hal_input" | "hal_output"
    )
}

/// Return sensible default parameters for `plugin_type`, adapted so that the
/// configuration is legal for the requested input channel count.
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
        "downmix" => {
            let input_layout = match channels {
                // Eight channels are ambiguous (for example 7.1 versus
                // 5.1.2), so the catalog probe must choose one explicitly.
                8 => Some("7.1"),
                _ => None,
            };
            serde_json::json!({
                "input_channels": channels,
                "input_layout": input_layout,
            })
        }
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
        // band_split/band_merge are intentionally multi-band: exercise their
        // documented channel-multiplying behavior with 2 bands.
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
            "num_mics": channels,
            "mic_spacing_m": 0.05,
        }),
        "ambisonics_decoder" => {
            let order = match channels {
                9 => 2,
                16 => 3,
                // First order is also the rejection probe for channel widths
                // that the catalog does not advertise.
                _ => 1,
            };
            serde_json::json!({
                "order": order,
                "target_layout": "5.1",
            })
        }
        _ => serde_json::json!({}),
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

/// Try to instantiate and process `plugin_type` with `channels` input channels.
/// Returns `Ok(output_channels)` on success, or `Err(error_message)` if the
/// plugin reports an error. A panic is propagated as a panic.
fn try_instantiate_and_process(plugin_type: &str, channels: usize) -> Result<usize, String> {
    // Beamformer panics internally when asked for fewer than 2 mics.
    if plugin_type == "beamformer" && channels < 2 {
        return Err("beamformer requires at least 2 microphones".to_string());
    }

    let mut plugin = create_plugin(
        plugin_type,
        &default_params(plugin_type, channels),
        channels,
        SAMPLE_RATE,
    )?;

    plugin
        .initialize(SAMPLE_RATE)
        .map_err(|e| format!("initialize: {e}"))?;

    let input = interleaved_sine(channels, FRAMES);
    let output_channels = plugin.output_channels();
    if output_channels == 0 {
        return Err("plugin reported zero output channels".to_string());
    }

    let mut output = vec![0.0f32; output_channels * FRAMES];
    plugin
        .process(
            &input,
            &mut output,
            &ProcessContext::new(SAMPLE_RATE, FRAMES),
        )
        .map_err(|e| format!("process: {e}"))?;

    Ok(output_channels)
}

#[test]
fn all_plugins_channel_count_support() {
    let mut panics = Vec::new();
    let mut unexpected_failures = Vec::new();

    for entry in PLUGIN_CATALOG {
        let plugin_type = entry.canonical_type;
        if is_skipped(plugin_type) {
            continue;
        }

        let PluginSupportedInputLayouts::Enumerated(supported_inputs) =
            entry.metadata.channel_layout.supported_inputs
        else {
            continue;
        };

        for &channels in CHANNEL_COUNTS {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                try_instantiate_and_process(plugin_type, channels)
            }));

            match outcome {
                Ok(result) => {
                    let expected = supported_inputs.contains(&channels);
                    if expected != result.is_ok() {
                        unexpected_failures.push(format!(
                            "{plugin_type}@{channels}ch expected {} but got {result:?}",
                            if expected { "success" } else { "rejection" }
                        ));
                    } else if let Err(ref err) = result
                        && err.trim().is_empty()
                    {
                        unexpected_failures.push(format!(
                            "{plugin_type}@{channels}ch returned an empty error"
                        ));
                    }
                }
                Err(payload) => {
                    let reason = panic_payload_description(&payload);
                    panics.push(format!("{plugin_type}@{channels}ch panicked: {reason}"));
                }
            }
        }
    }

    assert!(
        panics.is_empty(),
        "plugins panicked during channel-count probing:\n{}",
        panics.join("\n")
    );

    assert!(
        unexpected_failures.is_empty(),
        "unexpected channel-count failures:\n{}",
        unexpected_failures.join("\n")
    );
}

#[test]
fn stereo_imager_catalog_matches_stereo_only_factory_contract() {
    let entry = PLUGIN_CATALOG
        .iter()
        .find(|entry| entry.canonical_type == "stereo_imager")
        .expect("Stereo Imager must have a catalog entry");
    let PluginSupportedInputLayouts::Enumerated(supported_inputs) =
        entry.metadata.channel_layout.supported_inputs
    else {
        panic!("Stereo Imager must enumerate its supported input layout");
    };

    assert_eq!(supported_inputs, &[2]);

    for channels in [1, 4, 6, 8, 12] {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            try_instantiate_and_process("stereo_imager", channels)
        }))
        .expect("Stereo Imager factory must reject unsupported layouts without panicking");
        assert!(
            result.is_err(),
            "Stereo Imager unexpectedly accepted {channels} channels"
        );
    }
}

#[test]
fn catalog_default_channel_output_contracts_hold() {
    let mut failures = Vec::new();

    for entry in PLUGIN_CATALOG {
        let plugin_type = entry.canonical_type;
        if is_skipped(plugin_type) {
            continue;
        }
        let PluginSupportedInputLayouts::Enumerated(supported_inputs) =
            entry.metadata.channel_layout.supported_inputs
        else {
            continue;
        };

        for &channels in supported_inputs {
            let expected_output = match entry.metadata.channel_layout.output {
                PluginChannelOutputModel::PreservesInput => channels,
                PluginChannelOutputModel::Fixed(output) => output,
                PluginChannelOutputModel::Configurable { default_output, .. } => {
                    default_output.channels(channels)
                }
                PluginChannelOutputModel::InputTimesBands => channels * 2,
                PluginChannelOutputModel::InputDividedByBands => channels / 2,
                PluginChannelOutputModel::DescriptorDefined
                | PluginChannelOutputModel::PlatformNegotiated => continue,
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                try_instantiate_and_process(plugin_type, channels)
            }));

            match result {
                Ok(Ok(out_ch)) => {
                    if out_ch != expected_output {
                        failures.push(format!(
                            "{plugin_type}@{channels}ch produced {out_ch} output channels instead of catalog default {expected_output}"
                        ));
                    }
                }
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
        "catalog channel-output contracts failed:\n{}",
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
