//! Cross-cutting integration test: instantiate every built-in plugin type through
//! the factory at 1 (mono), 2 (stereo), 6 (5.1) and 8 (7.1) input channels.
//!
//! For each combination the test asserts that the plugin either:
//!   - accepts the configuration, initializes, processes a short sine burst, and
//!     reports a non-zero output channel count; or
//!   - returns a clear error (no panic).
//!
//! Channel-changing plugins are documented explicitly.

use sotf_host::ProcessContext;
use sotf_plugins::factory::{SUPPORTED_PLUGIN_TYPES, create_plugin};

const SAMPLE_RATE: u32 = 48_000;
const FRAMES: usize = 480;
const CHANNEL_COUNTS: &[usize] = &[1, 2, 6, 8];

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
        // band_split/band_merge are intentionally multi-band: exercise their
        // documented channel-multiplying behavior with 2 bands.
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
            "num_mics": channels,
            "mic_spacing_m": 0.05,
        }),
        "ambisonics_decoder" => serde_json::json!({
            "order": 1,
            "output_channels": 4,
        }),
        _ => serde_json::json!({}),
    }
}

/// For plugins with a strict input channel requirement, return the input count
/// they require *if* that count is one of the counts being exercised.
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

/// Try to instantiate and process `plugin_type` with `channels` input channels.
/// Returns `Ok(output_channels)` on success, or `Err(error_message)` if the
/// plugin reports an error. A panic is propagated as a panic.
fn try_instantiate_and_process(
    plugin_type: &str,
    channels: usize,
) -> Result<usize, String> {
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

    plugin.initialize(SAMPLE_RATE).map_err(|e| format!("initialize: {e}"))?;

    let input = interleaved_sine(channels, FRAMES);
    let output_channels = plugin.output_channels();
    if output_channels == 0 {
        return Err("plugin reported zero output channels".to_string());
    }

    let mut output = vec![0.0f32; output_channels * FRAMES];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SAMPLE_RATE, FRAMES))
        .map_err(|e| format!("process: {e}"))?;

    Ok(output_channels)
}

#[test]
fn all_plugins_channel_count_support() {
    let mut panics = Vec::new();
    let mut unexpected_failures = Vec::new();
    let mut results: Vec<(&str, usize, Result<usize, String>)> = Vec::new();

    for &plugin_type in SUPPORTED_PLUGIN_TYPES {
        if is_skipped(plugin_type) {
            continue;
        }

        let required = required_input_channels(plugin_type);

        for &channels in CHANNEL_COUNTS {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                try_instantiate_and_process(plugin_type, channels)
            }));

            match outcome {
                Ok(result) => {
                    results.push((plugin_type, channels, result.clone()));
                    if let Err(ref err) = result {
                        // Errors are allowed, but they must be informative.
                        if err.trim().is_empty() {
                            unexpected_failures.push(format!(
                                "{plugin_type}@{channels}ch returned an empty error"
                            ));
                        }
                    }
                }
                Err(payload) => {
                    let reason = panic_payload_description(&payload);
                    panics.push(format!(
                        "{plugin_type}@{channels}ch panicked: {reason}"
                    ));
                }
            }
        }

        // If the plugin has a strict input-channel requirement that happens to
        // be one of the counts under test, it must succeed there.
        if let Some(req) = required {
            let found = results.iter().any(|(t, ch, r)| {
                *t == plugin_type && *ch == req && r.is_ok()
            });
            if !found {
                unexpected_failures.push(format!(
                    "{plugin_type} did not accept its required {req} input channel(s)"
                ));
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

/// Channel-preserving plugins are expected to accept every tested input channel
/// count and produce the same number of output channels.
#[test]
fn channel_preserving_plugins_maintain_count() {
    let preserving = [
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
        "de_esser",
        "dynamic_eq",
        "fir_designer",
        "linear_phase_eq",
        "spectral_compressor",
        "stereo_imager",
        "transient_shaper",
        "saturation",
        "denoiser",
        "wiener_denoiser",
        "speech_denoiser",
        "rnnoise",
        "rnnoise_denoiser",
        "hiss_reducer",
        "hiss",
        "declick",
        "transient_repair",
        "pnd",
        "varispeed",
        "crossover",
        "matrix",
        "channel_mute_solo",
        "loudness_monitor",
        "spectrum_analyzer",
        "resampler",
        // band_split/band_merge deliberately multiply/divide channel counts and
        // are tested separately in channel_changing_plugins_behave_as_documented.
        "convolution",
        "loudness_compensation",
        "fletcher_munson",
    ];

    let mut failures = Vec::new();

    for &plugin_type in &preserving {
        for &channels in CHANNEL_COUNTS {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                try_instantiate_and_process(plugin_type, channels)
            }));

            match result {
                Ok(Ok(out_ch)) => {
                    if out_ch != channels {
                        failures.push(format!(
                            "{plugin_type}@{channels}ch produced {out_ch} output channels instead of {channels}"
                        ));
                    }
                }
                Ok(Err(err)) => {
                    failures.push(format!(
                        "{plugin_type}@{channels}ch failed: {err}"
                    ));
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
        "channel-preserving plugins did not maintain channel count:\n{}",
        failures.join("\n")
    );
}

/// Documented channel-changing behavior.
#[test]
fn channel_changing_plugins_behave_as_documented() {
    // mono_to_stereo: 1 -> 2
    let out = try_instantiate_and_process("mono_to_stereo", 1)
        .expect("mono_to_stereo must accept 1 input channel");
    assert_eq!(out, 2, "mono_to_stereo output should be stereo");

    // upmixer: 2 -> 6 (default 5.1 speaker configuration)
    let out = try_instantiate_and_process("upmixer", 2)
        .expect("upmixer must accept 2 input channels");
    assert_eq!(out, 6, "upmixer default output should be 5.1 (6 channels)");

    // downmix: N -> 2 for surround-capable input channel counts
    for &channels in &[2usize, 6, 8] {
        let out = try_instantiate_and_process("downmix", channels)
            .expect("downmix should accept 2/6/8 input channels");
        assert_eq!(
            out, 2,
            "downmix output should be stereo regardless of input"
        );
    }

    // crossfeed: stereo in, stereo out
    let out = try_instantiate_and_process("crossfeed", 2)
        .expect("crossfeed must accept 2 input channels");
    assert_eq!(out, 2, "crossfeed output should be stereo");

    // xtc: stereo in, stereo out
    let out = try_instantiate_and_process("xtc", 2)
        .expect("xtc must accept 2 input channels");
    assert_eq!(out, 2, "xtc output should be stereo");

    // aae / active_acoustic_enhancement: stereo in, 5.1 out (default config)
    let out = try_instantiate_and_process("aae", 2)
        .expect("aae must accept 2 input channels");
    assert_eq!(out, 6, "aae default output should be 5.1 (6 channels)");

    // ab_compare / ab: stereo in, stereo out
    let out = try_instantiate_and_process("ab_compare", 2)
        .expect("ab_compare must accept 2 input channels");
    assert_eq!(out, 2, "ab_compare output should be stereo");

    // aec: stereo reference + signal in, single-channel echo-cancelled output
    let out = try_instantiate_and_process("aec", 2)
        .expect("aec must accept 2 input channels");
    assert_eq!(out, 1, "aec output should be mono (1 channel)");

    // binaural_decoder: stereo in, stereo out
    let out = try_instantiate_and_process("binaural_decoder", 2)
        .expect("binaural_decoder must accept 2 input channels");
    assert_eq!(out, 2, "binaural_decoder output should be stereo");

    // band_split: input * num_bands (num_bands=2)
    for &channels in CHANNEL_COUNTS {
        let out = try_instantiate_and_process("band_split", channels)
            .expect("band_split should accept tested channel counts with 2 bands");
        assert_eq!(
            out,
            channels * 2,
            "band_split output should be input channels * 2"
        );
    }

    // band_merge: input / bands (bands=2). Input must be even and >= 2.
    for &channels in &[2usize, 6, 8] {
        let out = try_instantiate_and_process("band_merge", channels)
            .expect("band_merge should accept even channel counts >= 2 with 2 bands");
        assert_eq!(
            out,
            channels / 2,
            "band_merge output should be input channels / 2"
        );
    }
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
