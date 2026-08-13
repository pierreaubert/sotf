// ============================================================================
// Integration tests for sotf-plugin-matrix
//
// These tests exercise the crate's public API as a black box through the
// Plugin trait with realistic end-to-end routing workflows.
// ============================================================================

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_channel_mute_solo::ChannelState;
use sotf_plugin_matrix::MatrixPlugin;

const SAMPLE_RATE: u32 = 48_000;
const CONVERGE_FRAMES: usize = 2_048;

/// Process enough frames for gain smoothers to converge, return the last frame.
fn last_output_frame(plugin: &mut MatrixPlugin, frames: usize) -> Vec<f32> {
    let channels_in = plugin.input_channels();
    let channels_out = plugin.output_channels();
    let input = vec![1.0f32; frames * channels_in];
    let mut output = vec![0.0f32; frames * channels_out];
    let context = ProcessContext::new(SAMPLE_RATE, frames);
    plugin.process(&input, &mut output, &context).unwrap();
    output[output.len() - channels_out..].to_vec()
}

// ----------------------------------------------------------------------------
// Instantiation and metadata
// ----------------------------------------------------------------------------

#[test]
fn info_returns_expected_metadata() {
    let plugin = MatrixPlugin::new(2, 2);
    let info = plugin.info();
    assert_eq!(info.name, "Matrix");
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(info.author, "SotF");
}

#[test]
fn channel_counts_match_constructor() {
    let plugin = MatrixPlugin::new(2, 4);
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 4);
}

#[test]
fn parameters_include_routing_params() {
    let plugin = MatrixPlugin::new(2, 2);
    let ids: Vec<String> = plugin
        .parameters()
        .iter()
        .map(|p| p.id.to_string())
        .collect();

    assert!(ids.contains(&"gain".to_string()));
    assert!(ids.contains(&"preset".to_string()));
    assert!(ids.contains(&"gain_0_0".to_string()));
    assert!(ids.contains(&"gain_1_0".to_string()));
    assert!(ids.contains(&"phase_invert_0_0".to_string()));
    assert!(ids.contains(&"mute_0".to_string()));
    assert!(ids.contains(&"dim_0".to_string()));
    assert!(ids.contains(&"solo_0".to_string()));
    assert!(ids.contains(&"channel_states".to_string()));
}

// ----------------------------------------------------------------------------
// Happy-path processing
// ----------------------------------------------------------------------------

#[test]
fn identity_passthrough() {
    let mut plugin = MatrixPlugin::new(2, 2);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let last = last_output_frame(&mut plugin, CONVERGE_FRAMES);
    assert_eq!(last.len(), 2);
    assert!((last[0] - 1.0).abs() < 1e-3, "left should pass through");
    assert!((last[1] - 1.0).abs() < 1e-3, "right should pass through");
}

#[test]
fn stereo_swap_routes_correctly() {
    let mut plugin = MatrixPlugin::with_matrix(2, 2, vec![0.0, 1.0, 1.0, 0.0]).unwrap();
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 4;
    let input = vec![0.1f32, 0.9, 0.2, 0.8, 0.3, 0.7, 0.4, 0.6];
    let mut output = vec![0.0f32; num_frames * 2];
    let context = ProcessContext::new(SAMPLE_RATE, num_frames);

    // Process once: smoothers are still ramping, so values won't be exact.
    // Process enough frames for convergence before checking known frames.
    for _ in 0..CONVERGE_FRAMES {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    // After convergence the swap matrix maps L->R and R->L for every frame.
    assert!((output[0] - 0.9).abs() < 1e-3, "L_out should be R_in");
    assert!((output[1] - 0.1).abs() < 1e-3, "R_out should be L_in");
}

#[test]
fn mono_downmix_sums_inputs() {
    let mut plugin = MatrixPlugin::with_matrix(2, 1, vec![0.5, 0.5]).unwrap();
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 2;
    let input = vec![1.0f32, 0.0, 0.0, 1.0];
    let mut output = vec![0.0f32; num_frames];
    let context = ProcessContext::new(SAMPLE_RATE, num_frames);

    for _ in 0..CONVERGE_FRAMES {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    assert!((output[0] - 0.5).abs() < 1e-3);
    assert!((output[1] - 0.5).abs() < 1e-3);
}

#[test]
fn sparse_mapping_routes_to_physical_channels() {
    let mut plugin = MatrixPlugin::with_sparse_mapping(
        vec![1, 2],   // logical inputs 0,1 map to physical 1,2
        vec![15, 16], // logical outputs 0,1 map to physical 15,16
        vec![1.0, 0.0, 0.0, 1.0],
    )
    .unwrap();

    let mut input = vec![0.0f32; 3];
    input[1] = 10.0;
    input[2] = 20.0;
    let mut output = vec![0.0f32; 17];
    let context = ProcessContext::new(SAMPLE_RATE, 1);

    plugin.process(&input, &mut output, &context).unwrap();

    assert_eq!(output[15], 10.0);
    assert_eq!(output[16], 20.0);
}

// ----------------------------------------------------------------------------
// Parameter roundtrips and state transitions
// ----------------------------------------------------------------------------

#[test]
fn parameter_roundtrip_gain_and_phase_invert() {
    let mut plugin = MatrixPlugin::new(2, 2);

    plugin
        .set_parameter(ParameterId::from("gain_0_0"), ParameterValue::Float(0.75))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("gain_0_0")),
        Some(ParameterValue::Float(0.75))
    );

    plugin
        .set_parameter(
            ParameterId::from("phase_invert_0_0"),
            ParameterValue::Bool(true),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("phase_invert_0_0")),
        Some(ParameterValue::Bool(true))
    );
}

#[test]
fn parameter_roundtrip_channel_states() {
    let mut plugin = MatrixPlugin::new(2, 2);
    let states = vec![
        ChannelState {
            muted: true,
            soloed: false,
            dimmed: false,
        },
        ChannelState {
            muted: false,
            soloed: true,
            dimmed: false,
        },
    ];
    let json = serde_json::to_string(&states).unwrap();

    plugin
        .set_parameter(
            ParameterId::from("channel_states"),
            ParameterValue::String(json),
        )
        .unwrap();

    let got = plugin
        .get_parameter(&ParameterId::from("channel_states"))
        .unwrap();
    let got_str = got.as_string().unwrap();
    let got_states: Vec<ChannelState> = serde_json::from_str(got_str).unwrap();
    assert_eq!(got_states, states);
}

#[test]
fn stereo_downmix_is_identity_for_stereo_and_preserves_correlated_headroom() {
    let mut plugin = MatrixPlugin::new(2, 2);
    plugin.initialize(SAMPLE_RATE).unwrap();

    // Apply the stereo_downmix preset via its parameter index.
    // PRESET_CHOICES = ["custom", "stereo_downmix", "ms_encode", "ms_decode", "5.1_remap"]
    plugin
        .set_parameter(ParameterId::from("preset"), ParameterValue::Int(1))
        .unwrap();

    let last = last_output_frame(&mut plugin, CONVERGE_FRAMES);
    assert!((last[0] - 1.0).abs() < 1e-3);
    assert!((last[1] - 1.0).abs() < 1e-3);
}

#[test]
fn five_one_downmix_has_bounded_correlated_output() {
    let mut plugin = MatrixPlugin::new(6, 2);
    plugin
        .set_parameter(ParameterId::from("preset"), ParameterValue::Int(1))
        .unwrap();
    let last = last_output_frame(&mut plugin, CONVERGE_FRAMES);
    assert!(last.iter().all(|sample| (*sample - 1.0).abs() < 1e-3));
}

#[test]
fn preset_failure_is_atomic() {
    let mut plugin = MatrixPlugin::new(1, 1);
    let before = plugin.get_parameter(&ParameterId::from("preset"));
    let gain_before = plugin.get_gain(0, 0);
    assert!(
        plugin
            .set_parameter(ParameterId::from("preset"), ParameterValue::Int(1))
            .is_err()
    );
    assert_eq!(plugin.get_parameter(&ParameterId::from("preset")), before);
    assert_eq!(plugin.get_gain(0, 0), gain_before);
}

#[test]
fn solo_parameter_roundtrips_and_applies_multi_solo() {
    let mut plugin = MatrixPlugin::new(3, 3);
    for channel in [0, 2] {
        plugin
            .set_parameter(
                ParameterId::from(format!("solo_{channel}")),
                ParameterValue::Bool(true),
            )
            .unwrap();
    }
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("solo_0")),
        Some(ParameterValue::Bool(true))
    );
    let last = last_output_frame(&mut plugin, CONVERGE_FRAMES);
    assert!((last[0] - 1.0).abs() < 1e-3);
    assert!(last[1].abs() < 1e-3);
    assert!((last[2] - 1.0).abs() < 1e-3);
}

#[test]
fn mute_via_channel_states_silences_output() {
    let mut plugin = MatrixPlugin::new(2, 2);
    let states = vec![
        ChannelState {
            muted: true,
            soloed: false,
            dimmed: false,
        },
        ChannelState::default(),
    ];
    let json = serde_json::to_string(&states).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("channel_states"),
            ParameterValue::String(json),
        )
        .unwrap();

    let last = last_output_frame(&mut plugin, CONVERGE_FRAMES);
    assert!(last[0].abs() < 1e-3, "muted channel 0 should be silent");
    assert!(
        (last[1] - 1.0).abs() < 1e-3,
        "channel 1 should pass through"
    );
}

#[test]
fn solo_via_channel_states_mutes_others() {
    let mut plugin = MatrixPlugin::new(3, 3);
    let states = vec![
        ChannelState::default(),
        ChannelState {
            muted: false,
            soloed: true,
            dimmed: false,
        },
        ChannelState::default(),
    ];
    let json = serde_json::to_string(&states).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("channel_states"),
            ParameterValue::String(json),
        )
        .unwrap();

    let last = last_output_frame(&mut plugin, CONVERGE_FRAMES);
    assert!(
        last[0].abs() < 1e-3,
        "channel 0 should be silent when ch1 is soloed"
    );
    assert!(
        (last[1] - 1.0).abs() < 1e-3,
        "soloed channel 1 should pass through"
    );
    assert!(
        last[2].abs() < 1e-3,
        "channel 2 should be silent when ch1 is soloed"
    );
}

#[test]
fn dim_via_channel_states_attenuates_output() {
    let mut plugin = MatrixPlugin::new(2, 2);
    let states = vec![
        ChannelState {
            muted: false,
            soloed: false,
            dimmed: true,
        },
        ChannelState::default(),
    ];
    let json = serde_json::to_string(&states).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("channel_states"),
            ParameterValue::String(json),
        )
        .unwrap();

    let last = last_output_frame(&mut plugin, CONVERGE_FRAMES);
    assert!(
        (last[0] - 0.1).abs() < 1e-3,
        "dimmed channel should be at -20 dB"
    );
    assert!(
        (last[1] - 1.0).abs() < 1e-3,
        "channel 1 should pass through"
    );
}

#[test]
fn reset_then_process_continues() {
    let mut plugin = MatrixPlugin::new(2, 2);
    plugin.initialize(SAMPLE_RATE).unwrap();

    // Run briefly, reset, and make sure processing still converges to identity.
    let _ = last_output_frame(&mut plugin, 256);
    plugin.reset();
    let last = last_output_frame(&mut plugin, CONVERGE_FRAMES);

    assert!((last[0] - 1.0).abs() < 1e-3);
    assert!((last[1] - 1.0).abs() < 1e-3);
}

#[test]
fn reset_and_reinitialize_snap_transitions_to_configured_targets() {
    fn one_sample(plugin: &mut MatrixPlugin) -> f32 {
        let mut output = [0.0];
        plugin
            .process(&[1.0], &mut output, &ProcessContext::new(SAMPLE_RATE, 1))
            .unwrap();
        output[0]
    }

    let mut reset_plugin = MatrixPlugin::new(1, 1);
    reset_plugin.set_gain(0, 0, 0.25).unwrap();
    assert!(one_sample(&mut reset_plugin) > 0.25);
    reset_plugin.reset();
    assert!((one_sample(&mut reset_plugin) - 0.25).abs() < 1e-6);

    let mut reinitialized = MatrixPlugin::new(1, 1);
    reinitialized.set_phase_invert(0, 0, true).unwrap();
    assert!(one_sample(&mut reinitialized) > -1.0);
    reinitialized.initialize(96_000).unwrap();
    let mut output = [0.0];
    reinitialized
        .process(&[1.0], &mut output, &ProcessContext::new(96_000, 1))
        .unwrap();
    assert!((output[0] + 1.0).abs() < 1e-6);
}

#[test]
fn five_one_remap_converts_wave_to_aac_order() {
    let mut plugin = MatrixPlugin::new(6, 6);
    plugin
        .set_parameter(ParameterId::from("preset"), ParameterValue::Int(4))
        .unwrap();
    plugin.reset();
    let input = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let mut output = [0.0; 6];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SAMPLE_RATE, 1))
        .unwrap();
    assert_eq!(output, [3.0, 1.0, 2.0, 5.0, 6.0, 4.0]);
}

// ----------------------------------------------------------------------------
// Error paths and edge cases
// ----------------------------------------------------------------------------

#[test]
fn with_matrix_rejects_wrong_size() {
    assert!(MatrixPlugin::with_matrix(2, 2, vec![1.0, 0.0]).is_err());
    assert!(MatrixPlugin::with_matrix(2, 1, vec![1.0, 0.0, 0.0, 1.0]).is_err());
}

#[test]
fn preset_requires_integer_index() {
    let mut plugin = MatrixPlugin::new(2, 2);
    let result = plugin.set_parameter(
        ParameterId::from("preset"),
        ParameterValue::String("stereo_downmix".to_string()),
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("must be an integer"));
}

#[test]
fn phase_invert_requires_bool() {
    let mut plugin = MatrixPlugin::new(2, 2);
    let result = plugin.set_parameter(
        ParameterId::from("phase_invert_0_0"),
        ParameterValue::Float(1.0),
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("must be a bool"));
}

#[test]
fn process_zero_frames_zeros_output() {
    let mut plugin = MatrixPlugin::with_matrix(2, 2, vec![1.0, 0.0, 0.0, 1.0]).unwrap();
    let input = Vec::new();
    let mut output = Vec::new();
    let context = ProcessContext::new(SAMPLE_RATE, 0);

    let processed = plugin.process(&input, &mut output, &context).unwrap();
    assert_eq!(processed, 0);
    assert!(output.is_empty());
}

#[test]
fn set_matrix_rejects_wrong_size() {
    let mut plugin = MatrixPlugin::new(2, 2);
    let result = plugin.set_matrix(vec![1.0, 0.0]);
    assert!(result.is_err());
}

#[test]
fn malformed_dynamic_parameter_ids_are_rejected_without_panicking() {
    let mut plugin = MatrixPlugin::new(2, 2);
    for id in [
        "gain_0",
        "gain_0_0_extra",
        "phase_invert_0",
        "phase_invert_0_0_extra",
    ] {
        assert!(
            plugin
                .set_parameter(ParameterId::from(id), ParameterValue::Float(1.0))
                .is_err(),
            "{id} should be rejected"
        );
        assert_eq!(plugin.get_parameter(&ParameterId::from(id)), None);
    }
}

#[test]
fn dynamic_parameter_indices_cannot_alias_other_crosspoints() {
    let mut plugin = MatrixPlugin::new(2, 2);
    let before = plugin.get_gain(0, 1).unwrap();
    assert!(
        plugin
            .set_parameter(ParameterId::from("gain_2_0"), ParameterValue::Float(0.25))
            .is_err()
    );
    assert_eq!(plugin.get_gain(0, 1).unwrap(), before);
}

#[test]
fn sparse_constructor_validates_matrix_and_maps() {
    assert!(MatrixPlugin::with_sparse_mapping(vec![1, 2], vec![3, 4], vec![1.0]).is_err());
    assert!(MatrixPlugin::with_sparse_mapping(vec![1, 2], vec![3, 4], vec![0.0; 5]).is_err());
    assert!(MatrixPlugin::with_sparse_mapping(vec![usize::MAX], vec![0], vec![1.0]).is_err());
    assert!(MatrixPlugin::with_sparse_mapping(vec![0, 0], vec![0], vec![1.0; 2]).is_err());
    assert!(MatrixPlugin::with_sparse_mapping(vec![0], vec![0, 0], vec![1.0; 2]).is_err());
    assert!(MatrixPlugin::with_sparse_mapping(vec![128], vec![0], vec![1.0]).is_err());
    assert!(MatrixPlugin::with_matrix(129, 1, vec![0.0; 129]).is_err());
}

#[test]
fn global_gain_scales_audio_and_parameter_metadata_is_current() {
    let mut plugin = MatrixPlugin::new(1, 1);
    plugin
        .set_parameter(ParameterId::from("gain"), ParameterValue::Float(0.5))
        .unwrap();
    let input = [1.0];
    let mut output = [0.0];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SAMPLE_RATE, 1))
        .unwrap();
    assert!(output[0] < 1.0 && output[0] > 0.0);
    let params = plugin.parameters();
    let crosspoint = params.iter().find(|p| p.id.as_str() == "gain_0_0").unwrap();
    assert_eq!(crosspoint.default_value, ParameterValue::Float(1.0));
}

#[test]
fn channel_states_require_exact_output_width() {
    let mut plugin = MatrixPlugin::new(2, 2);
    let before = plugin
        .get_parameter(&ParameterId::from("channel_states"))
        .unwrap();
    for states in [
        vec![],
        vec![ChannelState::default()],
        vec![ChannelState::default(); 3],
    ] {
        let json = serde_json::to_string(&states).unwrap();
        assert!(
            plugin
                .set_parameter(
                    ParameterId::from("channel_states"),
                    ParameterValue::String(json)
                )
                .is_err()
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("channel_states")),
            Some(before.clone())
        );
    }
}

#[test]
fn process_rejects_short_buffers() {
    let mut plugin = MatrixPlugin::new(2, 2);
    let mut output = [0.0];
    assert!(
        plugin
            .process(&[1.0], &mut output, &ProcessContext::new(SAMPLE_RATE, 1))
            .is_err()
    );
}

#[test]
fn process_rejects_oversized_buffers_and_sample_rate_mismatch_atomically() {
    let mut plugin = MatrixPlugin::new(2, 2);
    let mut output = [7.0; 4];
    assert!(
        plugin
            .process(&[1.0; 4], &mut output, &ProcessContext::new(SAMPLE_RATE, 1))
            .is_err()
    );
    assert_eq!(output, [7.0; 4]);

    let mut exact_output = [7.0; 2];
    assert!(
        plugin
            .process(
                &[1.0; 2],
                &mut exact_output,
                &ProcessContext::new(44_100, 1)
            )
            .is_err()
    );
    assert_eq!(exact_output, [7.0; 2]);
}

#[test]
fn initialize_rejects_zero_sample_rate_without_changing_the_active_rate() {
    let mut plugin = MatrixPlugin::new(2, 2);
    assert!(plugin.initialize(0).is_err());

    let mut output = [0.0; 2];
    plugin
        .process(
            &[0.25, -0.5],
            &mut output,
            &ProcessContext::new(SAMPLE_RATE, 1),
        )
        .unwrap();
    assert_eq!(output, [0.25, -0.5]);
}

#[test]
fn parameter_metadata_reports_live_values() {
    let mut plugin = MatrixPlugin::new(2, 2);
    plugin
        .set_parameter(ParameterId::from("gain_0_0"), ParameterValue::Float(0.25))
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("solo_1"), ParameterValue::Bool(true))
        .unwrap();
    let params = plugin.parameters();
    assert_eq!(
        params
            .iter()
            .find(|p| p.id.as_str() == "gain_0_0")
            .unwrap()
            .default_value,
        ParameterValue::Float(0.25)
    );
    assert_eq!(
        params
            .iter()
            .find(|p| p.id.as_str() == "solo_1")
            .unwrap()
            .default_value,
        ParameterValue::Bool(true)
    );
}
