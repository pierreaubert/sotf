// ============================================================================
// Cross-Format Audio Comparison Test
// ============================================================================
//
// Validates that the parameter bridge (used by AU/VST3/CLAP wrappers)
// produces identical audio output to direct plugin API usage.
//
// Tests three critical invariants:
// 1. Parameter normalize/denormalize round-trip accuracy
// 2. Audio output equivalence: bridge path vs direct path
// 3. State save/load round-trip preserves parameters
//
// These tests catch:
// - Log/linear normalization mismatches
// - Parameter clamping differences between bridge and plugin
// - Interleave/deinterleave bugs in the buffer bridge
// - State serialization losing precision

use plugins_bridge::factory::{available_plugin_types, create_plugin};
use plugins_bridge::param_bridge::ParamBridge;
use sotf_host::param_specs::ParamType;
use sotf_host::plugin::ProcessContext;

const SAMPLE_RATE: u32 = 48000;
const NUM_FRAMES: usize = 1024;
const CHANNELS: usize = 2;

/// Generate a stereo test signal (440Hz sine + 1kHz sine)
fn test_signal(num_frames: usize, channels: usize) -> Vec<f32> {
    let mut buf = vec![0.0f32; num_frames * channels];
    for i in 0..num_frames {
        let t = i as f32 / SAMPLE_RATE as f32;
        let sample = 0.25 * (2.0 * std::f32::consts::PI * 440.0 * t).sin()
            + 0.15 * (2.0 * std::f32::consts::PI * 1000.0 * t).sin();
        for ch in 0..channels {
            buf[i * channels + ch] = sample;
        }
    }
    buf
}

/// Get ParamSpec for a plugin type (via its PARAMS constant)
fn get_param_specs(plugin_type: &str) -> &'static [sotf_host::param_specs::ParamSpec] {
    match plugin_type {
        "Gain" => sotf_plugin_gain::params::PARAMS,
        "Limiter" => sotf_plugin_limiter::params::PARAMS,
        "Gate" => sotf_plugin_gate::params::PARAMS,
        "Delay" => sotf_plugin_delay::params::PARAMS,
        "Crossfeed" => sotf_plugin_crossfeed::params::PARAMS,
        "Saturation" => sotf_plugin_saturation::params::PARAMS,
        "Denoiser" => sotf_plugin_denoiser::params::PARAMS,
        "ChannelMuteSolo" => sotf_plugin_channel_mute_solo::params::PARAMS,
        "PND" => sotf_plugin_pnd::params::PARAMS,
        "MonoToStereo" => sotf_plugin_mono_to_stereo::params::PARAMS,
        "StereoImager" => sotf_plugin_stereo_imager::params::PARAMS,
        "TransientShaper" => sotf_plugin_transient_shaper::params::PARAMS,
        "ABCompare" => sotf_plugin_ab_compare::params::PARAMS,
        "Dither" => sotf_plugin_dither::params::PARAMS,
        _ => &[], // Plugins with dynamic/complex params (EQ, Upmixer, etc.)
    }
}

/// Plugins suitable for cross-format testing (simple parameter model, stereo in/out)
fn testable_plugins() -> Vec<&'static str> {
    vec![
        "Gain",
        "Limiter",
        "Gate",
        "Delay",
        "Saturation",
        "ChannelMuteSolo",
        "PND",
        "StereoImager",
        "TransientShaper",
        "ABCompare",
        "Dither",
    ]
}

// ============================================================================
// Test 1: Parameter normalize/denormalize round-trip
// ============================================================================

#[test]
fn test_param_normalize_denormalize_roundtrip() {
    let mut failures = Vec::new();

    for &plugin_type in &testable_plugins() {
        let specs = get_param_specs(plugin_type);
        if specs.is_empty() {
            continue;
        }

        let bridge = ParamBridge::new(specs);

        for idx in 0..bridge.count() {
            let info = bridge.info(idx).unwrap();
            let spec = bridge.spec(idx).unwrap();

            // Skip FilePath params — they can't be meaningfully normalized
            if matches!(spec.param_type, ParamType::FilePath) {
                continue;
            }

            // Test several values across the range
            for &normalized in &[0.0, 0.25, 0.5, 0.75, 1.0] {
                let raw = bridge.denormalize(idx, normalized).unwrap();
                let renormalized = bridge.normalize(idx, raw).unwrap();
                let tolerance = if info.steps > 0 && info.logarithmic {
                    // Log-scale with quantization: step rounding in log space
                    // causes larger normalized error than linear step rounding
                    0.05
                } else if info.steps > 0 {
                    // Discrete params may quantize — allow 1 step of error
                    1.0 / (info.steps as f64).max(1.0) + 1e-6
                } else if info.logarithmic {
                    // Log-scale round-trip has inherent precision loss
                    0.02
                } else {
                    1e-4
                };

                if (renormalized - normalized).abs() > tolerance {
                    failures.push(format!(
                        "{plugin_type}/{} (idx {idx}): norm {normalized:.4} -> raw {raw:.4} -> renorm {renormalized:.4} (diff {:.6}, tol {tolerance:.6})",
                        info.name,
                        (renormalized - normalized).abs()
                    ));
                }
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "Parameter round-trip failures ({}):\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }
}

// ============================================================================
// Test 2: Audio output equivalence — direct vs bridge
// ============================================================================

#[test]
fn test_audio_output_equivalence() {
    let mut failures = Vec::new();

    for &plugin_type in &testable_plugins() {
        // Create two identical plugins via the factory
        let mut plugin_direct =
            match create_plugin(plugin_type, CHANNELS, SAMPLE_RATE, "{}") {
                Ok(p) => p,
                Err(_) => continue,
            };
        let mut plugin_bridge =
            match create_plugin(plugin_type, CHANNELS, SAMPLE_RATE, "{}") {
                Ok(p) => p,
                Err(_) => continue,
            };

        plugin_direct.initialize(SAMPLE_RATE).ok();
        plugin_bridge.initialize(SAMPLE_RATE).ok();

        let specs = get_param_specs(plugin_type);
        if specs.is_empty() {
            continue;
        }
        let bridge = ParamBridge::new(specs);

        // Set parameters on bridge plugin via normalized path (simulates AU/VST3 host)
        for idx in 0..bridge.count() {
            let info = bridge.info(idx).unwrap();
            // Use default normalized value
            let default_norm = bridge.normalize(idx, info.default_value).unwrap_or(0.5);
            bridge
                .set_normalized(plugin_bridge.as_mut(), idx, default_norm)
                .ok();
        }

        // Process identical audio through both
        let signal = test_signal(NUM_FRAMES, CHANNELS);
        let mut buf_direct = signal.clone();
        let mut buf_bridge = signal.clone();
        let ctx = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: NUM_FRAMES,
        };

        // Process multiple blocks to let both converge past any transient differences
        for _ in 0..4 {
            buf_direct = signal.clone();
            buf_bridge = signal.clone();
            plugin_direct.process(&buf_direct.clone(), &mut buf_direct, &ctx).ok();
            plugin_bridge.process(&buf_bridge.clone(), &mut buf_bridge, &ctx).ok();
        }

        // Compare outputs
        let max_diff: f32 = buf_direct
            .iter()
            .zip(buf_bridge.iter())
            .map(|(d, b)| (d - b).abs())
            .fold(0.0f32, f32::max);

        // Allow small tolerance for floating-point parameter mapping differences
        let tolerance = 1e-4;
        if max_diff > tolerance {
            failures.push(format!(
                "{plugin_type}: max_diff={max_diff:.6} (tolerance={tolerance})"
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "Audio equivalence failures ({}):\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }
}

// ============================================================================
// Test 3: Parameter bridge covers all factory plugin types
// ============================================================================

#[test]
fn test_all_factory_plugins_create_successfully() {
    let mut failures = Vec::new();

    // Plugins that require non-empty config JSON (mandatory fields with no serde default)
    let needs_config: &[&str] = &[
        "Convolution", // requires ir_file
        "Downmix",     // requires input_channels
        "Binaural",    // requires input_channels
        "Crossover",   // requires type
    ];

    for &plugin_type in available_plugin_types() {
        if needs_config.contains(&plugin_type) {
            continue;
        }
        match create_plugin(plugin_type, 2, SAMPLE_RATE, "{}") {
            Ok(mut plugin) => {
                if let Err(e) = plugin.initialize(SAMPLE_RATE) {
                    failures.push(format!("{plugin_type}: initialize failed: {e}"));
                }
            }
            Err(e) => {
                failures.push(format!("{plugin_type}: create failed: {e}"));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "Factory plugin creation failures ({}):\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }
}

// ============================================================================
// Test 4: Bridge parameter set/get round-trip on live plugin
// ============================================================================

#[test]
fn test_bridge_set_get_roundtrip_on_plugin() {
    let mut failures = Vec::new();

    for &plugin_type in &testable_plugins() {
        let specs = get_param_specs(plugin_type);
        if specs.is_empty() {
            continue;
        }

        let mut plugin = match create_plugin(plugin_type, CHANNELS, SAMPLE_RATE, "{}") {
            Ok(p) => p,
            Err(_) => continue,
        };
        plugin.initialize(SAMPLE_RATE).ok();

        let bridge = ParamBridge::new(specs);

        for idx in 0..bridge.count() {
            let info = bridge.info(idx).unwrap();

            // Set to a known normalized value
            let test_norm = 0.6;
            if bridge
                .set_normalized(plugin.as_mut(), idx, test_norm)
                .is_err()
            {
                continue; // Some params may not support arbitrary values
            }

            // Read back
            let readback = bridge.get_normalized(plugin.as_ref(), idx);
            if let Some(rb) = readback {
                let tolerance = if info.steps > 0 {
                    1.0 / (info.steps as f64).max(1.0) + 1e-4
                } else {
                    0.02 // Allow 2% tolerance for float rounding through clamp/step
                };
                if (rb - test_norm).abs() > tolerance {
                    failures.push(format!(
                        "{plugin_type}/{} (idx {idx}): set {test_norm:.4}, got {rb:.4} (diff {:.6})",
                        info.name,
                        (rb - test_norm).abs()
                    ));
                }
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "Bridge set/get round-trip failures ({}):\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }
}

// ============================================================================
// Test 5: State save/load round-trip preserves output
// ============================================================================

#[test]
fn test_state_save_load_roundtrip() {
    for &plugin_type in &testable_plugins() {
        let mut plugin = match create_plugin(plugin_type, CHANNELS, SAMPLE_RATE, "{}") {
            Ok(p) => p,
            Err(_) => continue,
        };
        plugin.initialize(SAMPLE_RATE).ok();

        // Save state
        let state = plugins_bridge::state::save_state(plugin.as_ref());
        if state.is_empty() {
            continue;
        }

        // Create fresh plugin, load state
        let mut plugin2 = create_plugin(plugin_type, CHANNELS, SAMPLE_RATE, "{}").unwrap();
        plugin2.initialize(SAMPLE_RATE).ok();
        plugins_bridge::state::load_state(plugin2.as_mut(), &state).ok();

        // Process identical audio through both
        let signal = test_signal(NUM_FRAMES, CHANNELS);
        let mut buf1 = signal.clone();
        let mut buf2 = signal.clone();
        let ctx = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: NUM_FRAMES,
        };
        plugin.process(&signal, &mut buf1, &ctx).ok();
        plugin2.process(&signal, &mut buf2, &ctx).ok();

        // Compare
        let max_diff: f32 = buf1
            .iter()
            .zip(buf2.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);

        assert!(
            max_diff < 1e-4,
            "{plugin_type}: state round-trip output mismatch: max_diff={max_diff:.6}"
        );
    }
}
