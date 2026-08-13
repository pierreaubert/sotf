use super::channel_mute_solo_plugin::ChannelMuteSoloPlugin;
use super::types::{ChannelMuteSoloParams, ChannelState, default_dim_gain_db, default_fade_ms};
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::ParameterSet;
use sotf_host::plugin::{PluginCompiledOp, ProcessContext};
use sotf_host::{CountingAlloc, assert_no_allocs};

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

#[test]
fn test_bypass() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, false); // disabled
    let mut buffer = vec![1.0, 2.0, 3.0, 4.0]; // 2 frames, 2 channels
    let context = ProcessContext::new(44100, 2);

    plugin.process_in_place(&mut buffer, &context).unwrap();

    // Should be unchanged when disabled
    assert_eq!(buffer, vec![1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn test_from_params() {
    let params = ChannelMuteSoloParams {
        enabled: true,
        channel_states: vec![
            ChannelState {
                muted: true,
                soloed: false,
                dimmed: false,
            },
            ChannelState {
                muted: false,
                soloed: false,
                dimmed: false,
            },
        ],
        dim_gain_db: default_dim_gain_db(),
        fade_ms: default_fade_ms(),
    };

    let plugin = ChannelMuteSoloPlugin::from_params(2, params);
    assert!(plugin.is_enabled());
    assert!(plugin.get_channel_state(0).unwrap().muted);
    assert!(!plugin.get_channel_state(1).unwrap().muted);
}

#[test]
fn test_get_dim_gain_parameter() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.set_dim_gain_db(-12.0);

    let val = plugin.get_parameter(&ParameterId::from("dim_gain_db"));
    assert_eq!(val, Some(ParameterValue::Float(-12.0)));
}

#[test]
fn test_fade_ms_via_set_parameter() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin
        .set_parameter(ParameterId::from("fade_ms"), ParameterValue::Float(50.0))
        .unwrap();

    assert!((plugin.fade_ms() - 50.0).abs() < f32::EPSILON);
}

#[test]
fn test_get_fade_ms_parameter() {
    let plugin = ChannelMuteSoloPlugin::new(2, true);
    let val = plugin.get_parameter(&ParameterId::from("fade_ms"));
    assert_eq!(val, Some(ParameterValue::Float(default_fade_ms())));
}

#[test]
fn test_dim_gain_out_of_range_rejected() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    // Above max (0.0)
    let result = plugin.set_parameter(ParameterId::from("dim_gain_db"), ParameterValue::Float(1.0));
    assert!(result.is_err());
    // Below min (-60.0)
    let result = plugin.set_parameter(
        ParameterId::from("dim_gain_db"),
        ParameterValue::Float(-70.0),
    );
    assert!(result.is_err());
}

#[test]
fn test_fade_ms_out_of_range_rejected() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    // Below min (0.0)
    let result = plugin.set_parameter(ParameterId::from("fade_ms"), ParameterValue::Float(-1.0));
    assert!(result.is_err());
    // Above max (100.0)
    let result = plugin.set_parameter(ParameterId::from("fade_ms"), ParameterValue::Float(200.0));
    assert!(result.is_err());
}

#[test]
fn test_params_serde_defaults() {
    // When deserializing JSON without dim_gain_db/fade_ms, defaults should apply
    let json = r#"{"enabled": true, "channel_states": []}"#;
    let params: ChannelMuteSoloParams = serde_json::from_str(json).unwrap();
    assert!((params.dim_gain_db - default_dim_gain_db()).abs() < f32::EPSILON);
    assert!((params.fade_ms - default_fade_ms()).abs() < f32::EPSILON);
}

/// The runtime/serde default must come from the canonical PARAMS schema.
#[test]
fn test_params_spec_fade_ms_default_matches_dsp_default() {
    use crate::params::PARAMS;
    use sotf_host::param_specs::find_by_key as pk;
    let spec_default = pk(PARAMS, "fade_ms").default_f64() as f32;
    assert!(
        (spec_default - default_fade_ms()).abs() < f32::EPSILON,
        "params.rs PARAMS fade_ms default ({}) must equal the runtime default ({})",
        spec_default,
        default_fade_ms()
    );
}

/// Fix 2.3: from_params with fewer channel_states than channels should pad with defaults.
#[test]
fn test_from_params_fewer_channel_states_pads_defaults() {
    let params = ChannelMuteSoloParams {
        enabled: true,
        channel_states: vec![ChannelState {
            muted: true,
            soloed: false,
            dimmed: false,
        }],
        dim_gain_db: default_dim_gain_db(),
        fade_ms: default_fade_ms(),
    };
    // 2-channel plugin, only 1 state provided
    let plugin = ChannelMuteSoloPlugin::from_params(2, params);
    // Ch0 from provided state
    assert!(plugin.get_channel_state(0).unwrap().muted);
    // Ch1 padded with default (not muted)
    assert!(!plugin.get_channel_state(1).unwrap().muted);
}

/// Fix 2.3: from_params with more channel_states than channels should truncate.
#[test]
fn test_from_params_more_channel_states_truncates() {
    let params = ChannelMuteSoloParams {
        enabled: true,
        channel_states: vec![
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
            ChannelState {
                muted: false,
                soloed: false,
                dimmed: true,
            },
        ],
        dim_gain_db: default_dim_gain_db(),
        fade_ms: default_fade_ms(),
    };
    // 2-channel plugin, 3 states provided — should use first 2
    let plugin = ChannelMuteSoloPlugin::from_params(2, params);
    assert!(plugin.get_channel_state(0).unwrap().muted);
    assert!(plugin.get_channel_state(1).unwrap().soloed);
    assert!(plugin.get_channel_state(2).is_none());
}

/// Fix 2.4: process_in_place buffer length mismatch must panic in debug (via debug_assert).
/// In release builds we just verify it processes normally when the length is correct.
#[test]
fn test_process_correct_buffer_length_succeeds() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    let mut buffer = vec![1.0f32; 4]; // 2 frames × 2 channels
    let ctx = ProcessContext::new(48000, 2);
    let result = plugin.process_in_place(&mut buffer, &ctx);
    assert!(result.is_ok());
}

/// Fix 3.2: lazy rebuild — parameters() channel_states JSON must reflect current state after
/// set_channel_state() mutates mute/solo/dim flags.
#[test]
fn test_lazy_rebuild_reflects_current_state_after_mute_toggle() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    // Mutate via the direct method (not set_parameter, which requires per-channel params to be
    // registered — see companion test below). Then call parameters() and verify the JSON blob
    // reflects the change, proving rebuild_cached_parameters ran.
    plugin.set_channel_state(0, true, false, false).unwrap();

    let params = plugin.parameters();
    let cs_param = params
        .iter()
        .find(|p| p.id.as_str() == "channel_states")
        .unwrap();
    let json = cs_param.default_value.as_string().unwrap();
    let states: Vec<ChannelState> = serde_json::from_str(json).unwrap();
    assert!(
        states[0].muted,
        "channel_states JSON in parameters() must reflect set_channel_state() change"
    );
}

/// Per-channel set_parameter (mute_N / solo_N / dim_N) must work via set_parameter interface.
/// validate_parameter currently rejects these because they are not in cached_parameters.
/// This test documents the fix: per-channel params must be skipped in validate_parameter.
#[test]
fn test_per_channel_set_parameter_mute_works() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin
        .set_parameter(ParameterId::from("mute_0"), ParameterValue::Bool(true))
        .unwrap();
    assert!(
        plugin.get_channel_state(0).unwrap().muted,
        "set_parameter mute_0=true must mute channel 0"
    );
}

/// Fix 3.3: set_channel_states should accept a borrowed slice to avoid needless Vec allocation.
#[test]
fn test_set_channel_states_accepts_slice() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    let states = [
        ChannelState {
            muted: true,
            soloed: false,
            dimmed: false,
        },
        ChannelState {
            muted: false,
            soloed: true,
            dimmed: true,
        },
    ];

    plugin.set_channel_states(&states).unwrap();

    let ch0 = plugin.get_channel_state(0).unwrap();
    let ch1 = plugin.get_channel_state(1).unwrap();
    assert!(ch0.muted);
    assert!(ch1.soloed);
    assert!(ch1.dimmed);
}

#[test]
fn test_process_mute_attenuates_channel() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.set_fade_ms(0.0);
    plugin.set_channel_state(0, true, false, false).unwrap();
    let mut buffer = vec![1.0, 2.0, 3.0, 4.0]; // 2 frames, 2 channels
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(48000, 2))
        .unwrap();
    assert!(buffer[0].abs() < 1e-6, "muted ch0 frame0 got {}", buffer[0]);
    assert!(buffer[2].abs() < 1e-6, "muted ch0 frame1 got {}", buffer[2]);
    assert!((buffer[1] - 2.0).abs() < 1e-6);
    assert!((buffer[3] - 4.0).abs() < 1e-6);
}

#[test]
fn test_process_solo_mutes_non_soloed() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.set_fade_ms(0.0);
    plugin.set_channel_state(0, false, true, false).unwrap();
    let mut buffer = vec![1.0, 2.0, 3.0, 4.0];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(48000, 2))
        .unwrap();
    assert!((buffer[0] - 1.0).abs() < 1e-6);
    assert!(buffer[1].abs() < 1e-6, "non-soloed ch1 should be silent");
    assert!((buffer[2] - 3.0).abs() < 1e-6);
    assert!(buffer[3].abs() < 1e-6, "non-soloed ch1 should be silent");
}

#[test]
fn test_process_dim_applies_dim_gain() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.set_fade_ms(0.0);
    plugin.set_dim_gain_db(-20.0);
    plugin.set_channel_state(0, false, false, true).unwrap();
    let mut buffer = vec![1.0, 2.0, 3.0, 4.0];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(48000, 2))
        .unwrap();
    let expected = 0.1;
    assert!((buffer[0] - expected).abs() < 1e-6);
    assert!((buffer[2] - expected * 3.0).abs() < 1e-5);
    assert!((buffer[1] - 2.0).abs() < 1e-6);
    assert!((buffer[3] - 4.0).abs() < 1e-6);
}

#[test]
fn test_compiled_channel_mute_solo_matches_in_place_process() {
    let input = vec![1.0, 2.0, 3.0, 4.0, -1.0, -2.0, -3.0, -4.0];
    let context = ProcessContext::new(48000, input.len() / 2);
    let mut regular = ChannelMuteSoloPlugin::new(2, true);
    let mut compiled = ChannelMuteSoloPlugin::new(2, true);
    regular.set_fade_ms(0.0);
    compiled.set_fade_ms(0.0);
    regular.set_channel_state(0, true, false, false).unwrap();
    compiled.set_channel_state(0, true, false, false).unwrap();
    let mut regular_output = input.clone();
    let mut compiled_output = vec![0.0; input.len()];

    let regular_frames = regular
        .process_in_place(&mut regular_output, &context)
        .unwrap();
    let compiled_frames = compiled
        .process_compiled_f32(
            PluginCompiledOp::ChannelMuteSolo,
            &input,
            &mut compiled_output,
            &context,
        )
        .expect("channel mute/solo should accept compiled op")
        .unwrap();

    assert_eq!(regular_frames, context.num_frames);
    assert_eq!(compiled_frames, context.num_frames);
    assert_eq!(compiled_output, regular_output);
}

#[test]
fn test_process_enabled_all_unmuted_is_passthrough() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.set_fade_ms(0.0);
    let original = vec![1.0, 2.0, 3.0, 4.0];
    let mut buffer = original.clone();
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(48000, 2))
        .unwrap();
    for (i, (a, b)) in buffer.iter().zip(original.iter()).enumerate() {
        assert!((a - b).abs() < 1e-6, "sample {i} changed from {b} to {a}");
    }
}

#[test]
fn test_process_zero_frames_is_no_op() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    let mut buffer = vec![];
    let result = plugin.process_in_place(&mut buffer, &ProcessContext::new(48000, 0));
    assert_eq!(result.unwrap(), 0);
}

#[test]
fn test_set_parameter_solo_and_dim() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin
        .set_parameter(ParameterId::from("solo_0"), ParameterValue::Bool(true))
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("dim_1"), ParameterValue::Bool(true))
        .unwrap();
    assert!(plugin.get_channel_state(0).unwrap().soloed);
    assert!(plugin.get_channel_state(1).unwrap().dimmed);
}

#[test]
fn test_set_parameter_invalid_channel_index() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    let result = plugin.set_parameter(ParameterId::from("mute_5"), ParameterValue::Bool(true));
    assert!(result.is_err(), "out-of-range channel index must error");
}

#[test]
fn test_set_parameter_malformed_channel_states() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    let result = plugin.set_parameter(
        ParameterId::from("channel_states"),
        ParameterValue::String("not json".to_string()),
    );
    assert!(result.is_err(), "malformed JSON must error");
}

#[test]
fn test_set_parameter_enabled_toggles() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();
    assert!(!plugin.is_enabled());
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(true))
        .unwrap();
    assert!(plugin.is_enabled());
}

#[test]
fn test_set_parameter_unknown_parameter() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    let result = plugin.set_parameter(ParameterId::from("unknown"), ParameterValue::Bool(true));
    assert!(result.is_err());
}

#[test]
fn test_set_parameter_wrong_type_fade_ms() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    let result = plugin.set_parameter(ParameterId::from("fade_ms"), ParameterValue::Bool(true));
    assert!(result.is_err(), "wrong type must error");
}

#[test]
fn test_block_smoothing_converges_to_target() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.set_fade_ms(50.0);
    plugin.set_channel_state(0, true, false, false).unwrap();
    let mut buffer = vec![1.0f32; 48000 * 2];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(48000, 48000))
        .unwrap();
    let final_gain = buffer[buffer.len() - 2];
    assert!(
        final_gain.abs() < 1e-3,
        "smoother should converge near zero, got {final_gain}"
    );
}

#[test]
fn final_sample_matches_advanced_smoother_state() {
    let mut plugin = ChannelMuteSoloPlugin::new(1, true);
    plugin.set_fade_ms(50.0);
    plugin.set_channel_state(0, true, false, false).unwrap();

    let mut buffer = vec![1.0f32; 8];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(48_000, 8))
        .unwrap();

    let smoother_end = plugin.channel_smoothers[0].current();
    assert!(
        (buffer[7] - smoother_end).abs() < 1.0e-7,
        "last sample {} must equal the advanced smoother state {smoother_end}",
        buffer[7]
    );
}

#[test]
fn smoothing_is_invariant_to_block_partition() {
    fn render(parts: &[usize]) -> Vec<f32> {
        let mut plugin = ChannelMuteSoloPlugin::new(1, true);
        plugin.initialize(48_000).unwrap();
        plugin.set_fade_ms(5.0);
        plugin.set_channel_state(0, true, false, false).unwrap();
        let mut output = Vec::new();
        for &frames in parts {
            let mut block = vec![1.0; frames];
            plugin
                .process_in_place(&mut block, &ProcessContext::new(48_000, frames))
                .unwrap();
            output.extend(block);
        }
        output
    }
    assert_eq!(render(&[512]), render(&[32; 16]));
}

#[test]
fn invalid_config_and_buffers_are_rejected() {
    let bad = ChannelMuteSoloParams {
        enabled: true,
        channel_states: vec![],
        dim_gain_db: f32::NAN,
        fade_ms: 5.0,
    };
    assert!(ChannelMuteSoloPlugin::try_from_params(1, bad).is_err());
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    let mut short = vec![1.0; 7];
    assert!(
        plugin
            .process_in_place(&mut short, &ProcessContext::new(48_000, 4))
            .is_err()
    );
}

#[test]
fn bulk_state_length_mismatch_is_rejected() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    assert!(
        plugin
            .set_channel_states(&[ChannelState::default()])
            .is_err()
    );
}

#[test]
fn transport_reset_preserves_every_in_flight_routing_fade() {
    fn assert_reset_continues(setup: impl Fn(&mut ChannelMuteSoloPlugin), observed_channel: usize) {
        let mut reference = ChannelMuteSoloPlugin::new(2, true);
        reference.initialize(48_000).unwrap();
        reference.set_fade_ms(20.0);
        setup(&mut reference);
        let mut reset = ChannelMuteSoloPlugin::new(2, true);
        reset.initialize(48_000).unwrap();
        reset.set_fade_ms(20.0);
        setup(&mut reset);

        let context = ProcessContext::new(48_000, 37);
        let mut reference_prefix = vec![1.0; 74];
        let mut reset_prefix = reference_prefix.clone();
        reference
            .process_in_place(&mut reference_prefix, &context)
            .unwrap();
        reset.process_in_place(&mut reset_prefix, &context).unwrap();
        reset.reset();

        let one_frame = ProcessContext::new(48_000, 1);
        let mut expected = vec![1.0; 2];
        let mut actual = expected.clone();
        reference
            .process_in_place(&mut expected, &one_frame)
            .unwrap();
        reset.process_in_place(&mut actual, &one_frame).unwrap();
        assert!(
            (actual[observed_channel] - expected[observed_channel]).abs() < 1.0e-7,
            "transport reset interrupted a routing fade: expected {}, got {}",
            expected[observed_channel],
            actual[observed_channel]
        );
    }

    assert_reset_continues(
        |plugin| plugin.set_channel_state(0, true, false, false).unwrap(),
        0,
    );
    assert_reset_continues(
        |plugin| plugin.set_channel_state(0, false, true, false).unwrap(),
        1,
    );
    assert_reset_continues(
        |plugin| plugin.set_channel_state(0, false, false, true).unwrap(),
        0,
    );

    for (initially_enabled, next_enabled) in [(true, false), (false, true)] {
        let muted = ChannelMuteSoloParams {
            enabled: initially_enabled,
            channel_states: vec![
                ChannelState {
                    muted: true,
                    ..ChannelState::default()
                },
                ChannelState::default(),
            ],
            dim_gain_db: default_dim_gain_db(),
            fade_ms: 20.0,
        };
        let mut reference = ChannelMuteSoloPlugin::from_params(2, muted.clone());
        let mut reset = ChannelMuteSoloPlugin::from_params(2, muted);
        reference.set_enabled(next_enabled);
        reset.set_enabled(next_enabled);
        let prefix_context = ProcessContext::new(48_000, 37);
        let mut expected_prefix = vec![1.0; 74];
        let mut actual_prefix = expected_prefix.clone();
        reference
            .process_in_place(&mut expected_prefix, &prefix_context)
            .unwrap();
        reset
            .process_in_place(&mut actual_prefix, &prefix_context)
            .unwrap();
        reset.reset();
        let one_frame = ProcessContext::new(48_000, 1);
        let mut expected = vec![1.0; 2];
        let mut actual = expected.clone();
        reference
            .process_in_place(&mut expected, &one_frame)
            .unwrap();
        reset.process_in_place(&mut actual, &one_frame).unwrap();
        assert!((actual[0] - expected[0]).abs() < 1.0e-7);
    }
}

#[test]
fn settled_processing_uses_static_block_path_and_transition_sensitive_metadata() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(48_000).unwrap();
    plugin.set_fade_ms(5.0);
    plugin.set_channel_state(0, true, false, false).unwrap();

    let transitioning = plugin.compile_metadata();
    assert!(transitioning.stateful);
    assert!(!transitioning.time_invariant_for_block);
    let mut transition = vec![1.0; 2];
    plugin
        .process_in_place(&mut transition, &ProcessContext::new(48_000, 1))
        .unwrap();
    assert_eq!(plugin.static_path_blocks, 0);

    let frames = 4096;
    let mut settle = vec![1.0; frames * 2];
    plugin
        .process_in_place(&mut settle, &ProcessContext::new(48_000, frames))
        .unwrap();
    let settled = plugin.compile_metadata();
    assert!(!settled.stateful);
    assert!(settled.time_invariant_for_block);

    let mut block = vec![1.0; 64];
    plugin
        .process_in_place(&mut block, &ProcessContext::new(48_000, 32))
        .unwrap();
    assert_eq!(plugin.static_path_blocks, 1);
    assert!(
        block
            .chunks_exact(2)
            .all(|frame| { frame[0].abs() < 1.0e-7 && (frame[1] - 1.0).abs() < 1.0e-7 })
    );

    for channels in [1, 2, 6, 8, 16, 32] {
        let mut static_plugin = ChannelMuteSoloPlugin::new(channels, true);
        static_plugin.set_fade_ms(0.0);
        static_plugin
            .set_channel_state(0, true, false, false)
            .unwrap();
        let mut static_block = vec![1.0; 64 * channels];
        static_plugin
            .process_in_place(&mut static_block, &ProcessContext::new(48_000, 64))
            .unwrap();
        assert_eq!(static_plugin.static_path_blocks, 1, "{channels} channels");
        assert!(static_block.chunks_exact(channels).all(|frame| {
            frame[0].abs() < 1.0e-7
                && frame[1.min(channels)..]
                    .iter()
                    .all(|sample| (*sample - 1.0).abs() < 1.0e-7)
        }));
    }
}

#[test]
fn settled_block_kernel_matches_scalar_reference_across_layouts_and_blocks() {
    for channels in [2, 6, 8, 16, 32] {
        let states: Vec<_> = (0..channels)
            .map(|channel| ChannelState {
                muted: channel % 4 == 0,
                soloed: false,
                dimmed: channel % 4 == 1,
            })
            .collect();
        for frames in [64, 256, 1_024] {
            let mut plugin = ChannelMuteSoloPlugin::from_params(
                channels,
                ChannelMuteSoloParams {
                    enabled: true,
                    channel_states: states.clone(),
                    dim_gain_db: -20.0,
                    fade_ms: 5.0,
                },
            );
            plugin.initialize(48_000).unwrap();
            let mut actual: Vec<f32> = (0..channels * frames)
                .map(|index| ((index % 29) as f32 - 14.0) / 14.0)
                .collect();
            let mut expected = actual.clone();
            for frame in expected.chunks_exact_mut(channels) {
                for (sample, state) in frame.iter_mut().zip(&states) {
                    let gain = if state.muted {
                        0.0
                    } else if state.dimmed {
                        0.1
                    } else {
                        1.0
                    };
                    *sample *= gain;
                }
            }

            plugin
                .process_in_place(&mut actual, &ProcessContext::new(48_000, frames))
                .unwrap();

            assert_eq!(actual, expected, "{channels} channels, {frames} frames");
        }
    }
}

#[test]
fn settled_blocks_do_not_advance_state_before_the_next_automation_transition() {
    fn prepare() -> ChannelMuteSoloPlugin {
        let mut plugin = ChannelMuteSoloPlugin::new(2, true);
        plugin.initialize(48_000).unwrap();
        plugin.set_fade_ms(5.0);
        plugin.set_channel_state(0, true, false, false).unwrap();
        let mut settling = vec![1.0; 8_192];
        plugin
            .process_in_place(&mut settling, &ProcessContext::new(48_000, 4_096))
            .unwrap();
        assert!(!plugin.compile_metadata().stateful);
        plugin
    }

    let mut reference = prepare();
    let mut optimized = prepare();
    let before_static_block = optimized.channel_smoothers[0].current();
    let mut static_block = vec![1.0; 512];
    optimized
        .process_in_place(&mut static_block, &ProcessContext::new(48_000, 256))
        .unwrap();
    assert_eq!(
        optimized.channel_smoothers[0].current(),
        before_static_block,
        "settled processing must not mutate smoother state"
    );

    reference.set_channel_state(0, false, false, false).unwrap();
    optimized.set_channel_state(0, false, false, false).unwrap();
    let mut reference_output = vec![1.0; 512];
    let mut optimized_output = reference_output.clone();
    let context = ProcessContext::new(48_000, 256);
    reference
        .process_in_place(&mut reference_output, &context)
        .unwrap();
    optimized
        .process_in_place(&mut optimized_output, &context)
        .unwrap();
    assert_eq!(optimized_output, reference_output);
}

#[test]
fn settled_and_transitioning_callbacks_do_not_allocate() {
    let mut settled = ChannelMuteSoloPlugin::from_params(
        8,
        ChannelMuteSoloParams {
            enabled: true,
            channel_states: (0..8)
                .map(|channel| ChannelState {
                    muted: channel == 0,
                    soloed: false,
                    dimmed: channel == 1,
                })
                .collect(),
            dim_gain_db: -20.0,
            fade_ms: 5.0,
        },
    );
    settled.initialize(48_000).unwrap();
    let mut transitioning = ChannelMuteSoloPlugin::new(8, true);
    transitioning.initialize(48_000).unwrap();
    transitioning.set_fade_ms(5.0);
    transitioning
        .set_channel_state(0, true, false, false)
        .unwrap();
    let context = ProcessContext::new(48_000, 256);
    let mut settled_buffer = vec![0.5; 8 * 256];
    let mut transition_buffer = settled_buffer.clone();

    assert_no_allocs("Channel Mute/Solo settled callback", || {
        settled
            .process_in_place(&mut settled_buffer, &context)
            .unwrap();
    });
    assert_no_allocs("Channel Mute/Solo transition callback", || {
        transitioning
            .process_in_place(&mut transition_buffer, &context)
            .unwrap();
    });
}

#[test]
fn apply_values_defers_schema_serialization_until_schema_is_requested() {
    let mut plugin = ChannelMuteSoloPlugin::new(32, true);
    let initial_serializations = plugin.schema_state_serializations.get();
    let initial_states_json = plugin.cached_parameters.borrow()[1]
        .default_value
        .as_string()
        .unwrap()
        .to_owned();
    let mut values = ParameterSet::new();
    values.insert(ParameterId::from("mute_3"), ParameterValue::Bool(true));
    values.insert(ParameterId::from("solo_4"), ParameterValue::Bool(true));
    values.insert(ParameterId::from("dim_5"), ParameterValue::Bool(true));

    assert_no_allocs("Channel Mute/Solo bulk apply_values", || {
        plugin.apply_values(values).unwrap();
    });

    assert!(plugin.params_dirty.get());
    assert_eq!(
        plugin.schema_state_serializations.get(),
        initial_serializations,
        "normal apply_values must not serialize channel state metadata"
    );
    assert_eq!(
        plugin.cached_parameters.borrow()[1]
            .default_value
            .as_string()
            .unwrap(),
        initial_states_json
    );
    let current = plugin.current_values();
    assert_eq!(
        current.get(&ParameterId::from("mute_3")),
        Some(&ParameterValue::Bool(true))
    );
    assert_eq!(
        current.get(&ParameterId::from("solo_4")),
        Some(&ParameterValue::Bool(true))
    );
    assert_eq!(
        current.get(&ParameterId::from("dim_5")),
        Some(&ParameterValue::Bool(true))
    );

    let schema = plugin.parameter_schema();
    assert_eq!(
        plugin.schema_state_serializations.get(),
        initial_serializations + 1
    );
    let states_json = schema[1].default_value.as_string().unwrap();
    let states: Vec<ChannelState> = serde_json::from_str(states_json).unwrap();
    assert!(states[3].muted);
    assert!(states[4].soloed);
    assert!(states[5].dimmed);
}

#[test]
fn adapter_updates_defer_descriptor_refresh_without_reallocating_ids() {
    let mut plugin = ChannelMuteSoloPlugin::new(8, true);
    let initial_ptr = plugin.cached_parameters.borrow().as_ptr();
    let initial_capacity = plugin.cached_parameters.borrow().capacity();
    let initial_states_json = plugin.cached_parameters.borrow()[1]
        .default_value
        .as_string()
        .unwrap()
        .to_owned();

    plugin
        .set_parameter(ParameterId::from("mute_3"), ParameterValue::Bool(true))
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("solo_4"), ParameterValue::Bool(true))
        .unwrap();

    assert!(plugin.params_dirty.get());
    assert_eq!(plugin.cached_parameters.borrow().as_ptr(), initial_ptr);
    assert_eq!(
        plugin.cached_parameters.borrow().capacity(),
        initial_capacity
    );
    assert_eq!(
        plugin.cached_parameters.borrow()[1]
            .default_value
            .as_string()
            .unwrap(),
        initial_states_json,
        "repeated adapter validation must not serialize dirty state"
    );

    let schema = plugin.parameter_schema();
    assert!(!plugin.params_dirty.get());
    assert_eq!(plugin.cached_parameters.borrow().as_ptr(), initial_ptr);
    assert_eq!(
        plugin.cached_parameters.borrow().capacity(),
        initial_capacity
    );
    let states_json = schema
        .iter()
        .find(|parameter| parameter.id.as_str() == "channel_states")
        .and_then(|parameter| parameter.default_value.as_string())
        .unwrap();
    let states: Vec<ChannelState> = serde_json::from_str(states_json).unwrap();
    assert!(states[3].muted);
}

/// Bug fix: set_channel_state must return an error for out-of-bounds channel.
#[test]
fn test_set_channel_state_oob_returns_error() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    let result = plugin.set_channel_state(2, true, false, false);
    assert!(
        result.is_err(),
        "set_channel_state(2) on 2-channel plugin must error"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("out of bounds"),
        "error message should mention out of bounds: {err}"
    );
}
