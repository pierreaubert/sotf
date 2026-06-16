// ============================================================================
// Property-Based Tests for Audio Plugins
// ============================================================================
//
// This module uses proptest for property-based testing to verify plugin
// behavior across a wide range of inputs.

use proptest::prelude::*;
use sotf_plugins::{GainPlugin, ParametricPlugin, ParametricPluginAdapter, PluginHost};

// ============================================================================
// Gain Plugin Tests
// ============================================================================

proptest! {
    #[test]
    fn test_gain_plugin_unity_gain(input in (0.0f32..1.0f32).prop_map(|v| vec![v; 1024])) {
        let mut gain = GainPlugin::new(2, 0.0);
        gain.plugin_initialize(48000).unwrap();

        let context = sotf_plugins::ProcessContext::new(48000, 512);

        let mut buffer = input.clone();
        gain.process(&input, &mut buffer, &context).unwrap();

        let mut max_error = 0.0f32;
        for (i_chunk, b_chunk) in input.chunks(2).zip(buffer.chunks(2)) {
            for (i, b) in i_chunk.iter().zip(b_chunk.iter()) {
                max_error = max_error.max((i - b).abs());
            }
        }

        prop_assert!(max_error < 1e-5, "Unity gain should pass signal through unchanged");
    }

    #[test]
    fn test_gain_plugin_6db(input in (0.0f32..1.0f32).prop_map(|v| vec![v; 512])) {
        let mut gain = GainPlugin::new(2, 6.0);
        gain.plugin_initialize(48000).unwrap();

        let context = sotf_plugins::ProcessContext::new(48000, 256);

        let mut buffer = input.clone();
        gain.process(&input, &mut buffer, &context).unwrap();

        let expected_scale = 10.0_f32.powf(6.0 / 20.0);

        let mut max_error = 0.0f32;
        for (i_chunk, b_chunk) in input.chunks(2).zip(buffer.chunks(2)) {
            for (i, b) in i_chunk.iter().zip(b_chunk.iter()) {
                max_error = max_error.max((i * expected_scale - b).abs());
            }
        }

        prop_assert!(max_error < 0.01, "6 dB gain should double amplitude");
    }

    #[test]
    fn test_gain_plugin_mute(input in (0.0f32..1.0f32).prop_map(|v| vec![v; 256])) {
        let mut gain = GainPlugin::new(2, -60.0);
        gain.plugin_initialize(48000).unwrap();

        let context = sotf_plugins::ProcessContext::new(48000, 128);

        let mut buffer = input.clone();
        gain.process(&input, &mut buffer, &context).unwrap();

        // -60 dB = 0.001, so output should be very small
        prop_assert!(buffer.iter().all(|o| o.abs() < 0.01),
            "Very low gain should produce near-silence");
    }

    #[test]
    fn test_gain_plugin_no_nan(input in (0.0f32..1.0f32).prop_map(|v| vec![v; 128]), gain_db in -100.0f32..100.0f32) {
        let mut gain = GainPlugin::new(2, gain_db);
        gain.plugin_initialize(48000).unwrap();

        let context = sotf_plugins::ProcessContext::new(48000, 64);

        let mut buffer = input.clone();
        let result = gain.process(&input, &mut buffer, &context);

        prop_assert!(result.is_ok(), "Process should succeed");
        prop_assert!(buffer.iter().all(|o| o.is_finite()), "Output should not contain NaN/Inf");
    }
}

// ============================================================================
// Host Processing Tests
// ============================================================================

proptest! {
    #[test]
    fn test_host_empty_graph(input in (0.0f32..1.0f32).prop_map(|v| vec![v; 512])) {
        let mut host = PluginHost::new(2, 48000);

        let mut output = vec![0.0f32; input.len()];
        host.process(&input, &mut output).unwrap();

        let mut max_error = 0.0f32;
        for (i, o) in input.iter().zip(output.iter()) {
            max_error = max_error.max((i - o).abs());
        }

        prop_assert!(max_error < 1e-5, "Empty host should pass signal through");
    }

    #[test]
    fn test_host_single_gain_chain(input in (0.0f32..1.0f32).prop_map(|v| vec![v; 256])) {
        let mut host = PluginHost::new(2, 48000);

        let gain = GainPlugin::new(2, 3.0);
        host.add_plugin(Box::new(ParametricPluginAdapter::new(gain)))
            .unwrap();

        let mut output = vec![0.0f32; input.len()];
        host.process(&input, &mut output).unwrap();

        let expected_scale = 10.0_f32.powf(3.0 / 20.0);

        let mut max_error = 0.0f32;
        for (i, o) in input.iter().zip(output.iter()) {
            max_error = max_error.max((i * expected_scale - o).abs());
        }

        prop_assert!(max_error < 0.1, "Gain chain should apply 3 dB gain");
    }
}

// ============================================================================
// Signal Processing Properties
// ============================================================================

proptest! {
    #[test]
    fn test_signal_energy_conservation(input in (0.1f32..1.0f32).prop_map(|v| vec![v; 256])) {
        let input_energy: f32 = input.iter().map(|x| x * x).sum();

        let mut gain = GainPlugin::new(2, 0.0);
        gain.plugin_initialize(48000).unwrap();

        let context = sotf_plugins::ProcessContext::new(48000, 128);

        let mut buffer = input.clone();
        gain.process(&input, &mut buffer, &context).unwrap();

        let output_energy: f32 = buffer.iter().map(|x| x * x).sum();
        let ratio = output_energy / input_energy;

        prop_assert!((ratio - 1.0).abs() < 1e-4,
            "Unity gain should preserve signal energy");
    }

    #[test]
    fn test_gain_linearity(input in (0.0f32..1.0f32).prop_map(|v| vec![v; 128]), gain_db in -60.0f32..60.0f32) {
        let mut gain = GainPlugin::new(2, gain_db);
        gain.plugin_initialize(48000).unwrap();

        let context = sotf_plugins::ProcessContext::new(48000, 64);

        let mut buffer = input.clone();
        gain.process(&input, &mut buffer, &context).unwrap();

        let expected_scale = 10.0_f32.powf(gain_db / 20.0);

        let mut max_error = 0.0f32;
        for (i, o) in input.iter().zip(buffer.iter()) {
            max_error = max_error.max((i * expected_scale - o).abs());
        }

        prop_assert!(max_error < 0.01 * expected_scale.abs().max(1.0),
            "Gain should scale input linearly");
    }

    #[test]
    fn test_no_nan_propagation(input in (0.0f32..1.0f32).prop_map(|v| vec![v; 64])) {
        let mut host = PluginHost::new(2, 48000);

        let gain = GainPlugin::new(2, 0.0);
        host.add_plugin(Box::new(ParametricPluginAdapter::new(gain)))
            .unwrap();

        let mut output = vec![0.0f32; input.len()];
        let result = host.process(&input, &mut output);

        prop_assert!(result.is_ok(), "Processing should succeed");
        prop_assert!(output.iter().all(|o| o.is_finite()),
            "Output should not contain NaN or Inf");
    }
}

// ============================================================================
// Boundary Value Tests
// ============================================================================

#[test]
fn test_gain_at_boundary_values() {
    let test_cases = [-120.0, -60.0, 0.0, 60.0, 120.0];

    for gain_db in test_cases {
        let mut gain = GainPlugin::new(2, gain_db);
        let result = gain.plugin_initialize(48000);
        assert!(
            result.is_ok(),
            "Initialization should succeed for {} dB",
            gain_db
        );

        let context = sotf_plugins::ProcessContext::new(48000, 64);

        let input = [0.5f32; 128];
        let mut buffer = input.to_vec();
        let result = gain.process(&input, &mut buffer, &context);

        assert!(
            result.is_ok(),
            "Processing should succeed for {} dB",
            gain_db
        );
        assert!(
            buffer.iter().all(|o| o.is_finite()),
            "Output should be finite for {} dB",
            gain_db
        );
    }
}

#[test]
fn test_processing_at_different_sample_rates() {
    let sample_rates = [8000, 44100, 48000, 96000, 192000];

    for sample_rate in sample_rates {
        let mut gain = GainPlugin::new(2, 0.0);
        let result = gain.plugin_initialize(sample_rate);
        assert!(
            result.is_ok(),
            "Initialization should succeed for {} Hz",
            sample_rate
        );

        let context = sotf_plugins::ProcessContext::new(sample_rate, 64);

        let input = [0.5f32; 128];
        let mut buffer = input.to_vec();
        let result = gain.process(&input, &mut buffer, &context);

        assert!(
            result.is_ok(),
            "Processing should succeed for {} Hz",
            sample_rate
        );
    }
}

#[test]
fn test_processing_at_different_buffer_sizes() {
    let buffer_sizes = [32, 64, 128, 256, 512, 1024];

    for buffer_size in buffer_sizes {
        let mut gain = GainPlugin::new(2, 0.0);
        gain.plugin_initialize(48000).unwrap();

        let context = sotf_plugins::ProcessContext::new(48000, buffer_size);

        let input = vec![0.5f32; buffer_size * 2];
        let mut buffer = input.to_vec();
        let result = gain.process(&input, &mut buffer, &context);

        assert!(
            result.is_ok(),
            "Processing should succeed for buffer size {}",
            buffer_size
        );
        assert!(
            buffer.iter().all(|o| (o - 0.5).abs() < 0.001),
            "Output should match input for unity gain with buffer size {}",
            buffer_size
        );
    }
}
