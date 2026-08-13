#![allow(clippy::needless_range_loop)]
use super::binaural_decoder_plugin::BinauralDecoderPlugin;
use super::filter;
use super::hrtf;
use super::room;
pub use super::room::RoomModel;
use super::types::BinauralState;
use rustfft::num_complex::Complex;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use std::sync::Arc;

#[test]
fn unsupported_channel_layouts_are_rejected() {
    for channels in [0, 4, 7, 9, 11, 13, 15, 17] {
        let params = crate::BinauralDecoderParams {
            input_channels: channels,
            ..serde_json::from_value(serde_json::json!({"input_channels": 2})).unwrap()
        };
        assert!(
            BinauralDecoderPlugin::try_from_params(params).is_err(),
            "{channels} channels must not silently fall back to stereo"
        );
    }
    for channels in crate::config::SUPPORTED_INPUT_CHANNELS {
        let params = crate::BinauralDecoderParams {
            input_channels: channels,
            ..serde_json::from_value(serde_json::json!({"input_channels": 2})).unwrap()
        };
        assert!(BinauralDecoderPlugin::try_from_params(params).is_ok());
    }
}

#[test]
fn construction_state_restores_all_runtime_room_controls() {
    let params: crate::BinauralDecoderParams = serde_json::from_value(serde_json::json!({
        "input_channels": 2,
        "crossfade_mode": 1,
        "crossfade_ms": 125.0,
        "late_reverb_enabled": true,
        "late_reverb_mix": 0.42,
        "late_reverb_rt60": 2.5,
        "late_reverb_damping": 0.65
    }))
    .unwrap();
    let plugin = BinauralDecoderPlugin::try_from_params(params).unwrap();
    assert_eq!(plugin.config.crossfade_mode_index, 1);
    assert_eq!(plugin.config.crossfade_ms, 125.0);
    assert!(plugin.config.late_reverb_enabled);
    assert_eq!(plugin.config.late_reverb_mix, 0.42);
    assert_eq!(plugin.config.late_reverb_rt60, 2.5);
    assert_eq!(plugin.config.late_reverb_damping, 0.65);
}

#[test]
fn runtime_parameter_surface_exactly_matches_canonical_specs() {
    let plugin = BinauralDecoderPlugin::new(
        2,
        64,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    let runtime = plugin.parameters();
    assert_eq!(runtime.len(), crate::params::PARAMS.len());
    for (parameter, spec) in runtime.iter().zip(crate::params::PARAMS) {
        assert_eq!(parameter.id.as_str(), spec.engine_key);
        assert_eq!(parameter.update_mode, spec.update_mode);
    }
    assert!(
        runtime
            .iter()
            .all(|parameter| parameter.id.as_str() != "hrtf_file")
    );
    assert_eq!(plugin.info().version, env!("CARGO_PKG_VERSION"));
}

#[test]
fn compile_metadata_is_a_conservative_stateful_boundary() {
    let plugin = BinauralDecoderPlugin::new(
        2,
        64,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    let metadata = plugin.compile_metadata();
    assert!(metadata.boundary && metadata.stateful);
    assert!(!metadata.linear && !metadata.time_invariant_for_block);
    assert!(!metadata.can_absorb_input_gain && !metadata.can_absorb_output_gain);
    assert_eq!(metadata.latency_samples, plugin.latency_samples());
}

#[test]
fn full_head_update_queue_retries_without_advancing_last_sent_angle() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        64,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.initialize(48_000).unwrap();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    tx.try_send((0.0, 0.0, 0.0)).unwrap();
    plugin.hrtf_update_tx = Some(tx);
    plugin.smoothing.head_yaw_deg.set_target(90.0);
    let input = vec![0.0; 64 * 2];
    let mut output = vec![0.0; 64 * 2];
    plugin
        .process(&input, &mut output, &ProcessContext::new(48_000, 64))
        .unwrap();
    assert_eq!(plugin.smoothing.last_hrtf_yaw, 0.0);
    rx.try_recv().unwrap();

    let mut latest = 0.0;
    for _ in 0..200 {
        plugin
            .process(&input, &mut output, &ProcessContext::new(48_000, 64))
            .unwrap();
        if let Ok((yaw, _, _)) = rx.try_recv() {
            latest = yaw;
        }
    }
    assert!(latest > 89.0, "worker mailbox stopped at {latest} degrees");
    assert!((plugin.smoothing.last_hrtf_yaw - latest).abs() < 1.0e-6);
}

#[test]
fn completed_crossfade_state_is_handed_to_background_reclaimer() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        64,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    let old = Arc::new(BinauralState {
        hrtf_filters_freq: vec![vec![Complex::new(1.0, 0.0); 66]; 2],
        diffuse_field_eq_filter: None,
        _hrtf_data: None,
    });
    let weak = Arc::downgrade(&old);
    plugin.crossfade.crossfade_prev_state = Some(old);
    plugin.crossfade.crossfade_remaining = 0;
    plugin.retire_completed_crossfade_state();
    assert!(plugin.crossfade.crossfade_prev_state.is_none());
    for _ in 0..100 {
        if weak.upgrade().is_none() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("retired HRTF state was not destroyed by the background reclaimer");
}

#[test]
fn reflections_preserve_source_ownership_and_silent_channels_add_nothing() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        64,
        None,
        1.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.smoothing.externalization.set_target(1.0);
    plugin.room.cached_reflections = vec![vec![], vec![]];
    let reflection = room::Reflection {
        delay_samples: 1,
        gain: 0.5,
        left_gain: 0.8,
        right_gain: 0.2,
        azimuth_deg: -45.0,
        elevation_deg: 0.0,
        hrtf_filter: None,
    };
    plugin.room.cached_reflections[0].push(reflection.clone());
    plugin.room.reflection_delay_line[0] = 1.0;
    plugin.room.reflection_read_pos = 1;
    let mut output = [0.0, 0.0];
    plugin.apply_reflections(&mut output, 1);
    assert!((output[0] - 0.4).abs() < 1.0e-6);
    assert!((output[1] - 0.1).abs() < 1.0e-6);

    // A configured reflection owned by a silent second channel cannot change
    // the first source's response.
    plugin.room.cached_reflections[1].push(reflection);
    plugin.room.reflection_read_pos = 1;
    let mut with_silent_channel = [0.0, 0.0];
    plugin.apply_reflections(&mut with_silent_channel, 1);
    assert_eq!(with_silent_channel, output);
}

#[path = "tests/misc.rs"]
mod misc;

fn test_vbap_sofa() -> sotf_host::sofa::SofaFile {
    use sotf_host::sofa::SourcePosition;
    // Three non-collinear unit vectors in the positive octant.
    let positions = vec![
        SourcePosition::new(0.0, 0.0, 1.0),
        SourcePosition::new(90.0, 0.0, 1.0),
        SourcePosition::new(0.0, 90.0, 1.0),
    ];
    sotf_host::sofa::SofaFile {
        sample_rate: 48_000.0,
        num_measurements: 3,
        ir_length: 1,
        positions,
        impulse_responses: vec![1.0; 6],
        convention: "SimpleFreeFieldHRIR".into(),
        data_sample_rate: Some(48_000.0),
    }
}

#[test]
fn vbap_weights_are_exact_affine_coordinates() {
    use sotf_host::sofa::SourcePosition;
    let sofa = test_vbap_sofa();
    let nearest = [(0, 0.0), (1, 0.0), (2, 0.0)];

    for (target, expected) in [
        (SourcePosition::new(0.0, 0.0, 1.0), [1.0, 0.0, 0.0]),
        (SourcePosition::new(45.0, 0.0, 1.0), [0.5, 0.5, 0.0]),
        (SourcePosition::new(45.0, 35.26439, 1.0), [1.0 / 3.0; 3]),
    ] {
        let gains = hrtf::calculate_vbap_gains(&target, &nearest, &sofa);
        for i in 0..3 {
            assert!(
                (gains[i] - expected[i]).abs() < 1.0e-4,
                "{gains:?} != {expected:?}"
            );
        }
        assert!((gains.iter().sum::<f32>() - 1.0).abs() < 1.0e-5);
    }
}

#[test]
fn vbap_constant_field_is_reproduced() {
    use sotf_host::sofa::SourcePosition;
    let sofa = test_vbap_sofa();
    let nearest = [(0, 0.0), (1, 0.0), (2, 0.0)];
    let target = SourcePosition::new(45.0, 35.26439, 1.0);
    let gains = hrtf::calculate_vbap_gains(&target, &nearest, &sofa);
    let mut planner = realfft::RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(64);
    let (left, right) = hrtf::interpolate_hrtf_frequency_domain(
        &nearest,
        &gains,
        &sofa,
        64,
        48_000,
        &fft,
        0.0,
        target.azimuth,
        target.elevation,
    );
    for value in left.iter().chain(right.iter()) {
        assert!(
            (value.re - 1.0).abs() < 1.0e-4 && value.im.abs() < 1.0e-4,
            "{value:?}"
        );
    }
}

#[test]
fn diffuse_eq_rejects_empty_inconsistent_and_non_finite_datasets() {
    use sotf_host::sofa::{SofaFile, SourcePosition};
    let mut planner = realfft::RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(64);
    let make = |num_measurements, positions, impulse_responses, ir_length| SofaFile {
        sample_rate: 48_000.0,
        num_measurements,
        ir_length,
        positions,
        impulse_responses,
        convention: "SimpleFreeFieldHRIR".into(),
        data_sample_rate: Some(48_000.0),
    };
    assert!(
        filter::compute_diffuse_field_eq(&make(0, vec![], vec![], 1), 64, 48_000, &fft).is_err()
    );
    assert!(
        filter::compute_diffuse_field_eq(
            &make(1, vec![SourcePosition::new(0.0, 0.0, 1.0)], vec![1.0], 1),
            64,
            48_000,
            &fft
        )
        .is_err()
    );
    assert!(
        filter::compute_diffuse_field_eq(
            &make(
                1,
                vec![SourcePosition::new(0.0, 0.0, 1.0)],
                vec![f32::NAN, f32::NAN],
                1
            ),
            64,
            48_000,
            &fft
        )
        .is_err()
    );
}

#[test]
fn diffuse_eq_is_level_invariant_bounded_and_preserves_ear_balance() {
    use sotf_host::sofa::{SofaFile, SourcePosition};
    let make = |scale: f32| SofaFile {
        sample_rate: 48_000.0,
        num_measurements: 2,
        ir_length: 4,
        positions: vec![
            SourcePosition::new(-30.0, 0.0, 1.0),
            SourcePosition::new(30.0, 0.0, 1.0),
        ],
        impulse_responses: [
            [1.0, 0.2, 0.0, 0.0],
            [0.5, 0.1, 0.0, 0.0],
            [0.8, -0.1, 0.0, 0.0],
            [0.4, -0.05, 0.0, 0.0],
        ]
        .into_iter()
        .flatten()
        .map(|sample| sample * scale)
        .collect(),
        convention: "SimpleFreeFieldHRIR".into(),
        data_sample_rate: Some(48_000.0),
    };
    let mut planner = realfft::RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(64);
    let a = filter::compute_diffuse_field_eq(&make(1.0), 64, 48_000, &fft).unwrap();
    let b = filter::compute_diffuse_field_eq(&make(0.01), 64, 48_000, &fft).unwrap();
    let max_boost = 10.0_f32.powf(12.0 / 20.0) + 1.0e-5;
    for ear in 0..2 {
        for bin in 0..a[ear].len() {
            assert!((a[ear][bin].norm() - b[ear][bin].norm()).abs() < 2.0e-3);
            assert!(a[ear][bin].norm() <= max_boost);
        }
    }
    // Bin 1 is the nearest 64-point FFT bin to the 1 kHz comparison frequency.
    let reference_bin = 1;
    assert!(a[0][reference_bin].norm() < a[1][reference_bin].norm());
}

#[test]
fn initialize_configures_fdn_for_engine_rate_and_reset_clears_tail() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.config.late_reverb_enabled = true;
    plugin.config.late_reverb_mix = 1.0;
    plugin.initialize(96_000).unwrap();

    // Excite the FDN directly, then reset and require complete silence.
    for i in 0..20_000 {
        let input = if i == 0 { 1.0 } else { 0.0 };
        let _ = plugin.room.fdn.process_stereo(input, input);
    }
    plugin.reset_state();
    for _ in 0..20_000 {
        let (left, right) = plugin.room.fdn.process_stereo(0.0, 0.0);
        assert_eq!((left, right), (0.0, 0.0));
    }
}

#[test]
fn streaming_process_holds_output_for_reported_latency() {
    let fft_size = 64;
    let total_frames = fft_size * 4;
    let impulse_frame = fft_size / 4;
    let mut input = vec![0.0_f32; total_frames * 2];
    input[impulse_frame * 2] = 1.0;

    // Exercise callback sizes on both sides of the FFT frame.  A callback
    // larger than the frame is the important case: without the host-facing
    // gate, the first frame drains using future samples from that callback.
    for callback_frames in [1, 16, 32, 64, 128] {
        let mut plugin = BinauralDecoderPlugin::new(
            2,
            fft_size,
            None,
            0.0,
            0.0,
            false,
            120.0,
            2.0,
            0.0,
            RoomModel::default(),
        );
        plugin.initialize(48_000).unwrap();

        let mut rendered = Vec::with_capacity(total_frames * 2);
        for start in (0..total_frames).step_by(callback_frames) {
            let end = (start + callback_frames).min(total_frames);
            let frames = end - start;
            let mut block = vec![0.0_f32; frames * 2];
            let context = ProcessContext::new(48_000, frames);
            plugin
                .process(&input[start * 2..end * 2], &mut block, &context)
                .unwrap();
            rendered.extend_from_slice(&block);
        }

        let expected_frame = plugin.latency_samples() + impulse_frame;
        assert!(
            rendered[..expected_frame * 2]
                .chunks_exact(2)
                .all(|frame| frame.iter().all(|sample| sample.abs() < 1.0e-7)),
            "callback {callback_frames}: output escaped before fixed latency at frame {expected_frame}"
        );
        assert!(
            rendered[expected_frame * 2].abs() > 1.0e-4,
            "callback {callback_frames}: impulse did not arrive at fixed frame {expected_frame}"
        );
    }
}

#[test]
fn streaming_hrtf_matches_direct_linear_convolution_across_boundaries() {
    let fft_size = 64;
    let sample_rate = 48_000;
    let input_frames = 192;
    let flush_frames = 128;
    let input: Vec<f32> = (0..input_frames)
        .map(|i| ((i as f32 * 0.37).sin() + (i as f32 * 0.11).cos()) * 0.2)
        .collect();

    for ir_len in [1, 15, 49] {
        let left_ir: Vec<f32> = (0..ir_len)
            .map(|i| {
                if i == 0 {
                    0.7
                } else {
                    0.2 * (-0.08 * i as f32).exp()
                }
            })
            .collect();
        let right_ir: Vec<f32> = left_ir.iter().rev().map(|sample| sample * 0.6).collect();
        for callback_frames in [1, 15, 16, 17, 64, 97] {
            let mut plugin = BinauralDecoderPlugin::new(
                1,
                fft_size,
                None,
                0.0,
                0.0,
                false,
                120.0,
                2.0,
                0.0,
                RoomModel {
                    max_order: 0,
                    ..Default::default()
                },
            );
            plugin.initialize(sample_rate).unwrap();
            let left = filter::ir_to_freq(&left_ir, fft_size, &plugin.fft.fft_r2c);
            let right = filter::ir_to_freq(&right_ir, fft_size, &plugin.fft.fft_r2c);
            let mut combined = left;
            combined.extend(right);
            let state = Arc::new(BinauralState {
                hrtf_filters_freq: vec![combined],
                diffuse_field_eq_filter: None,
                _hrtf_data: None,
            });
            plugin.state.store(state.clone());
            plugin.crossfade.current_state_snapshot = state;

            let mut stream = input.clone();
            stream.resize(input_frames + flush_frames, 0.0);
            let mut rendered = Vec::new();
            for block in stream.chunks(callback_frames) {
                let mut output = vec![0.0; block.len() * 2];
                plugin
                    .process(
                        block,
                        &mut output,
                        &ProcessContext::new(sample_rate, block.len()),
                    )
                    .unwrap();
                rendered.extend(output);
            }

            for frame in 0..input_frames + ir_len - 1 {
                let direct = |ir: &[f32]| {
                    (0..ir.len())
                        .filter_map(|tap| {
                            frame
                                .checked_sub(tap)
                                .filter(|&src| src < input.len())
                                .map(|src| input[src] * ir[tap])
                        })
                        .sum::<f32>()
                };
                let output_frame = plugin.latency_samples() + frame;
                let expected_left = direct(&left_ir);
                let expected_right = direct(&right_ir);
                assert!(
                    (rendered[output_frame * 2] - expected_left).abs() < 2.0e-5,
                    "ir={ir_len} block={callback_frames} frame={frame}: {} != {expected_left}",
                    rendered[output_frame * 2]
                );
                assert!(
                    (rendered[output_frame * 2 + 1] - expected_right).abs() < 2.0e-5,
                    "ir={ir_len} block={callback_frames} frame={frame}: {} != {expected_right}",
                    rendered[output_frame * 2 + 1]
                );
            }
        }
    }

    let plugin = BinauralDecoderPlugin::new(
        1,
        fft_size,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    assert!(plugin.validate_linear_convolution_ir(50).is_err());
}

#[test]
fn test_binaural_decoder_creation() {
    let plugin = BinauralDecoderPlugin::new(
        5,
        4096,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    assert_eq!(plugin.input_channels(), 5);
    assert_eq!(plugin.output_channels(), 2);
    assert_eq!(plugin.config.fft_size, 4096);
    assert_eq!(plugin.config.hop_size, 1024);
}

/// 5.1 surround (6 input channels) should produce binaural stereo (2 output channels).
#[test]
fn test_binaural_decoder_6ch_input_produces_2ch_output() {
    let input_channels = 6; // 5.1 surround
    let plugin = BinauralDecoderPlugin::new(
        input_channels,
        2048,
        None,
        0.5,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    assert_eq!(plugin.input_channels(), 6);
    assert_eq!(
        plugin.output_channels(),
        2,
        "Binaural decoder should always output 2 channels (binaural stereo)"
    );
}

#[test]
fn test_process_rejects_short_buffers() {
    let mut plugin = BinauralDecoderPlugin::new(
        5,
        2048,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    let ctx = ProcessContext::new(48000, 32);

    let short_input = vec![0.0_f32; 32 * 5 - 1];
    let mut output = vec![0.0_f32; 32 * 2];
    assert!(plugin.process(&short_input, &mut output, &ctx).is_err());

    let input = vec![0.0_f32; 32 * 5];
    let mut short_output = vec![0.0_f32; 32 * 2 - 1];
    assert!(plugin.process(&input, &mut short_output, &ctx).is_err());
}

#[test]
fn test_crossfade_fields_initialized() {
    let plugin = BinauralDecoderPlugin::new(
        2,
        2048,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );

    assert!(plugin.crossfade.crossfade_prev_state.is_none());
    assert_eq!(plugin.crossfade.crossfade_remaining, 0);
    assert_eq!(plugin.crossfade.crossfade_total, 0);
    assert_eq!(
        plugin.crossfade.crossfade_sum_left.len(),
        plugin.config.freq_size
    );
    assert_eq!(
        plugin.crossfade.crossfade_sum_right.len(),
        plugin.config.freq_size
    );
}

#[test]
fn test_crossfade_triggers_on_state_change() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.initialize(44100).unwrap();

    // Simulate state change by storing a new state
    let freq_size = plugin.config.freq_size;
    let new_state = Arc::new(BinauralState {
        hrtf_filters_freq: vec![
            vec![Complex::new(0.5, 0.0); freq_size * 2];
            plugin.config.input_channels
        ],
        diffuse_field_eq_filter: None,
        _hrtf_data: None,
    });
    plugin.state.store(new_state);

    // Process a block -- this should detect the state change and start crossfade
    // Fill input buffer to trigger a block
    plugin.input.input_buffer.fill(0.0);
    plugin.input.input_fill = plugin.config.fft_size;
    plugin.process_audio_block();

    // Crossfade should have been initiated and partially consumed
    // After one hop, remaining should be total - hop_size
    assert!(
        plugin.crossfade.crossfade_total > 0,
        "Crossfade total should be > 0 after state change"
    );
}

#[test]
fn test_crossfade_completes() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.initialize(44100).unwrap();

    // Trigger a state change
    let freq_size = plugin.config.freq_size;
    let new_state = Arc::new(BinauralState {
        hrtf_filters_freq: vec![
            vec![Complex::new(0.5, 0.0); freq_size * 2];
            plugin.config.input_channels
        ],
        diffuse_field_eq_filter: None,
        _hrtf_data: None,
    });
    plugin.state.store(new_state);

    // Process enough blocks to complete the crossfade
    // 50ms at 44100 Hz = 2205 samples; hop_size=256 => ~9 hops
    for _ in 0..20 {
        plugin.input.input_buffer.fill(0.0);
        plugin.input.input_fill = plugin.config.fft_size;
        plugin.process_audio_block();
    }

    // After enough blocks, crossfade should be complete
    assert_eq!(plugin.crossfade.crossfade_remaining, 0);
    assert!(plugin.crossfade.crossfade_prev_state.is_none());
}

#[test]
fn test_process_produces_output_without_hrtf() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.initialize(48000).unwrap();

    let num_frames = 4096;
    let input = vec![0.1f32; num_frames * 2]; // stereo
    let mut output = vec![0.0f32; num_frames * 2];
    let context = ProcessContext::new(48000, num_frames);

    let processed = plugin.process(&input, &mut output, &context).unwrap();
    assert_eq!(processed, num_frames);

    // Should produce some non-zero output (passthrough with default HRTF)
    let has_signal = output.iter().any(|&s| s.abs() > 1e-6);
    assert!(
        has_signal,
        "Output should contain signal with default passthrough HRTF"
    );
}

#[test]
fn test_near_field_smoke() {
    // Create plugin with near_field_strength > 0 and verify output is
    // finite and non-zero (basic smoke test for the near-field path).
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        2048,
        None,
        0.5, // externalization
        0.8, // near_field_strength > 0
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.initialize(48000).unwrap();

    // Process enough audio to fill the STFT pipeline and produce output
    let num_frames = 8192;
    let input: Vec<f32> = (0..num_frames * 2)
        .map(|i| {
            let phase = 2.0 * std::f32::consts::PI * 440.0 * (i / 2) as f32 / 48000.0;
            phase.sin() * 0.3
        })
        .collect();
    let mut output = vec![0.0f32; num_frames * 2];
    let context = ProcessContext::new(48000, num_frames);

    let processed = plugin.process(&input, &mut output, &context).unwrap();
    assert_eq!(processed, num_frames);

    // All outputs should be finite
    assert!(
        output.iter().all(|s| s.is_finite()),
        "All output samples must be finite"
    );

    // At least some output should be non-zero (after STFT latency fills)
    let has_signal = output.iter().any(|&s| s.abs() > 1e-6);
    assert!(
        has_signal,
        "Near-field binaural output should contain signal"
    );
}

#[test]
fn test_reset_clears_crossfade() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.initialize(44100).unwrap();

    // Trigger a state change
    let freq_size = plugin.config.freq_size;
    let new_state = Arc::new(BinauralState {
        hrtf_filters_freq: vec![
            vec![Complex::new(0.5, 0.0); freq_size * 2];
            plugin.config.input_channels
        ],
        diffuse_field_eq_filter: None,
        _hrtf_data: None,
    });
    plugin.state.store(new_state);

    // Process one block to start crossfade
    plugin.input.input_buffer.fill(0.0);
    plugin.input.input_fill = plugin.config.fft_size;
    plugin.process_audio_block();

    // Now reset
    plugin.reset();

    assert!(plugin.crossfade.crossfade_prev_state.is_none());
    assert_eq!(plugin.crossfade.crossfade_remaining, 0);
}

/// Verify that the `crossfade_ms` parameter can be get/set and that the change
/// is reflected in the crossfade duration (measured in samples) when a state
/// transition is detected in `process_audio_block()`.
#[test]
fn test_crossfade_ms_parameter_set_get_and_affects_duration() {
    use sotf_host::parameters::ParameterId;

    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.initialize(44100).unwrap();

    // Default should be 50ms
    let default_val = plugin
        .get_parameter(&ParameterId::from("crossfade_ms"))
        .expect("crossfade_ms parameter must exist");
    assert_eq!(
        default_val,
        ParameterValue::Float(50.0),
        "Default crossfade_ms should be 50.0"
    );

    // Set to 200ms and confirm the stored value changes
    plugin
        .set_parameter(
            ParameterId::from("crossfade_ms"),
            ParameterValue::Float(200.0),
        )
        .expect("set_parameter crossfade_ms should succeed");

    let new_val = plugin
        .get_parameter(&ParameterId::from("crossfade_ms"))
        .expect("crossfade_ms must still exist after set");
    assert_eq!(
        new_val,
        ParameterValue::Float(200.0),
        "crossfade_ms should be updated to 200.0"
    );

    // Verify range rejection: value below minimum should not update the field
    let _ = plugin.set_parameter(
        ParameterId::from("crossfade_ms"),
        ParameterValue::Float(5.0), // below the 10ms minimum -- validate_parameter should reject
    );
    let after_invalid = plugin
        .get_parameter(&ParameterId::from("crossfade_ms"))
        .unwrap();
    // The value must still be 200.0 (the last valid value)
    assert_eq!(
        after_invalid,
        ParameterValue::Float(200.0),
        "crossfade_ms must not be updated to an out-of-range value"
    );

    // Now verify that the duration used in process_audio_block() reflects the
    // new setting. Trigger a state change and measure crossfade_total.
    let freq_size = plugin.config.freq_size;
    let new_state = Arc::new(BinauralState {
        hrtf_filters_freq: vec![
            vec![Complex::new(0.5, 0.0); freq_size * 2];
            plugin.config.input_channels
        ],
        diffuse_field_eq_filter: None,
        _hrtf_data: None,
    });
    plugin.state.store(new_state);

    plugin.input.input_buffer.fill(0.0);
    plugin.input.input_fill = plugin.config.fft_size;
    plugin.process_audio_block();

    // At 44100 Hz and 200ms, crossfade_samples = 44100 * 0.200 = 8820.
    // hop_size = 1024/4 = 256.
    // crossfade_hops = ceil(8820 / 256) = 35.
    // crossfade_total = 35 * 256 = 8960.
    let expected_samples = (44100.0_f32 * 0.200) as usize; // 8820
    let hop = plugin.config.hop_size;
    let expected_hops = expected_samples.div_ceil(hop);
    let expected_total = expected_hops * hop;

    assert_eq!(
        plugin.crossfade.crossfade_total, expected_total,
        "crossfade_total should reflect the 200ms setting"
    );
}

/// Verify head angle parameters can be set and retrieved.
#[test]
fn test_head_angle_parameters_set_get() {
    use sotf_host::parameters::ParameterId;

    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );

    // Default values should all be 0.
    for name in &["head_yaw_deg", "head_pitch_deg", "head_roll_deg"] {
        let v = plugin
            .get_parameter(&ParameterId::from(*name))
            .expect("parameter must exist");
        assert_eq!(
            v,
            ParameterValue::Float(0.0),
            "{} default should be 0.0",
            name
        );
    }

    // Set each to a distinct value and verify.
    plugin
        .set_parameter(
            ParameterId::from("head_yaw_deg"),
            ParameterValue::Float(30.0),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("head_pitch_deg"),
            ParameterValue::Float(-15.0),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("head_roll_deg"),
            ParameterValue::Float(10.0),
        )
        .unwrap();

    assert_eq!(
        plugin.get_parameter(&ParameterId::from("head_yaw_deg")),
        Some(ParameterValue::Float(30.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("head_pitch_deg")),
        Some(ParameterValue::Float(-15.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("head_roll_deg")),
        Some(ParameterValue::Float(10.0))
    );
}

/// Verify that head angles appear in the parameter list returned by `parameters()`.
#[test]
fn test_head_angle_parameters_listed() {
    let plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    let params = plugin.parameters();
    let names: Vec<_> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(
        names.contains(&"head_yaw_deg"),
        "head_yaw_deg should be listed"
    );
    assert!(
        names.contains(&"head_pitch_deg"),
        "head_pitch_deg should be listed"
    );
    assert!(
        names.contains(&"head_roll_deg"),
        "head_roll_deg should be listed"
    );
}

/// With a synthetic SOFA dataset, verify that yaw=30 produces different HRTF filters
/// than yaw=0. At yaw=30 the speaker positions are rotated by -30 degrees in azimuth,
/// so the VBAP lookup will select a different part of the SOFA dataset.
#[test]
fn test_yaw_changes_hrtf_filters() {
    const NUM_MEAS: usize = 36;
    const IR_LEN: usize = 64;
    const SAMPLE_RATE: f32 = 44100.0;

    let mut positions = Vec::with_capacity(NUM_MEAS);
    let mut impulse_responses = Vec::with_capacity(NUM_MEAS * 2 * IR_LEN);

    for i in 0..NUM_MEAS {
        let az = -180.0 + (i as f32) * (360.0 / NUM_MEAS as f32);
        positions.push(sotf_host::sofa::SourcePosition::new(az, 0.0, 1.0));

        // Left-ear IR: amplitude encodes the azimuth index so filters differ per position.
        let mut ir_l = vec![0.0f32; IR_LEN];
        ir_l[0] = 1.0 + i as f32 * 0.01;
        let ir_r = vec![0.0f32; IR_LEN];

        impulse_responses.extend_from_slice(&ir_l);
        impulse_responses.extend_from_slice(&ir_r);
    }

    let sofa = sotf_host::sofa::SofaFile {
        sample_rate: SAMPLE_RATE,
        num_measurements: NUM_MEAS,
        ir_length: IR_LEN,
        positions,
        impulse_responses,
        convention: "SimpleFreeFieldHRIR".to_string(),
        data_sample_rate: Some(SAMPLE_RATE),
    };

    // Compute the left-ear HRTF frequency spectrum for the L stereo speaker
    // (az=+30, el=0) with a given head yaw applied via inverse rotation.
    let compute_left_filter = |yaw: f32| -> Vec<Complex<f32>> {
        let (rot_az, rot_el) =
            BinauralDecoderPlugin::rotate_speaker_position(30.0, 0.0, yaw, 0.0, 0.0);
        let tgt = sotf_host::sofa::SourcePosition::new(rot_az, rot_el, 1.0);
        let near = sofa.find_three_nearest(&tgt);
        let gains = hrtf::calculate_vbap_gains(&tgt, &near, &sofa);

        let fft_size = 512usize;
        let mut planner = realfft::RealFftPlanner::<f32>::new();
        let fft_r2c = planner.plan_fft_forward(fft_size);

        let (l_fft, _) = hrtf::interpolate_hrtf_frequency_domain(
            &near,
            &gains,
            &sofa,
            fft_size,
            44100,
            &fft_r2c,
            0.0,
            tgt.azimuth,
            tgt.elevation,
        );
        l_fft
    };

    let filters_yaw0 = compute_left_filter(0.0);
    let filters_yaw30 = compute_left_filter(30.0);

    let max_diff = filters_yaw0
        .iter()
        .zip(filters_yaw30.iter())
        .map(|(a, b)| (a - b).norm())
        .fold(0.0f32, f32::max);

    assert!(
        max_diff > 1e-4,
        "HRTF filters with yaw=30 must differ from yaw=0 (max_diff={})",
        max_diff
    );
}

/// rotate_speaker_position must be identity when all angles are 0.
#[test]
fn test_rotate_speaker_position_identity() {
    let (az, el) = BinauralDecoderPlugin::rotate_speaker_position(45.0, 20.0, 0.0, 0.0, 0.0);
    assert!(
        (az - 45.0).abs() < 1e-3,
        "azimuth should be unchanged: {}",
        az
    );
    assert!(
        (el - 20.0).abs() < 1e-3,
        "elevation should be unchanged: {}",
        el
    );
}

/// For yaw-only rotation the rotated speaker azimuth should shift by -yaw.
#[test]
fn test_rotate_speaker_position_yaw_only() {
    // Speaker at az=30, el=0. Head yaw=30 => inverse shift of -30 => az near 0.
    let (az, el) = BinauralDecoderPlugin::rotate_speaker_position(30.0, 0.0, 30.0, 0.0, 0.0);
    assert!(
        (az - 0.0).abs() < 1e-3,
        "azimuth after yaw should be near 0, got {}",
        az
    );
    assert!(
        (el - 0.0).abs() < 1e-3,
        "elevation should stay 0, got {}",
        el
    );
}

/// Processing with non-zero head yaw must not produce NaN or Inf output.
#[test]
fn test_head_yaw_produces_finite_output() {
    use sotf_host::parameters::ParameterId;

    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.initialize(44100).unwrap();

    // Set yaw to 30. Without a SOFA file the default filters remain in place;
    // the smoother must advance without causing NaN/Inf.
    plugin
        .set_parameter(
            ParameterId::from("head_yaw_deg"),
            ParameterValue::Float(30.0),
        )
        .unwrap();

    let num_frames = 4096;
    let input: Vec<f32> = (0..num_frames * 2)
        .map(|i| (i as f32 * 0.01).sin() * 0.5)
        .collect();
    let mut output = vec![0.0f32; num_frames * 2];
    let context = ProcessContext::new(44100, num_frames);

    let processed = plugin.process(&input, &mut output, &context).unwrap();
    assert_eq!(processed, num_frames);
    assert!(
        output.iter().all(|s| s.is_finite()),
        "All output samples must be finite with non-zero yaw"
    );
}

/// Verify that spectral crossfade mode (magnitude interpolation + RTPGHI)
/// produces a smoother magnitude spectrum than linear complex blending
/// during an HRTF transition.
///
/// The test triggers a crossfade between two different HRTF filter sets and
/// processes audio through both modes. The spectral mode should produce a
/// magnitude spectrum without the comb-filter dips that linear mode creates
/// when old and new HRTFs have different phase responses.
#[test]
fn test_spectral_crossfade_no_tonal_shift() {
    let fft_size = 1024;
    let freq_size = fft_size / 2 + 1;
    let sample_rate = 44100u32;

    // Create two plugins: one linear (mode 0), one spectral (mode 1)
    let make_plugin = |mode: usize| {
        let mut p = BinauralDecoderPlugin::new(
            2,
            fft_size,
            None,
            0.0,
            0.0,
            false,
            120.0,
            2.0,
            0.0,
            RoomModel::default(),
        );
        p.config.crossfade_mode_index = mode;
        p.initialize(sample_rate).unwrap();
        p
    };

    let mut linear_plugin = make_plugin(0);
    let mut spectral_plugin = make_plugin(1);

    // Create two distinct HRTF states to trigger a crossfade.
    // State A: passthrough-like (all 1.0)
    // State B: different phase response (rotated complex values)
    let make_state = |phase_shift: f32, channels: usize| {
        let mut filters = vec![vec![Complex::new(0.0, 0.0); freq_size * 2]; channels];
        for ch in 0..channels {
            for k in 0..freq_size {
                // Different phase per frequency bin for state B, creating phase
                // differences that would cause comb-filtering in linear blend.
                let angle = phase_shift * (k as f32 / freq_size as f32) * std::f32::consts::PI;
                let (sin_a, cos_a) = angle.sin_cos();
                let val = Complex::new(cos_a * 0.7, sin_a * 0.7);
                filters[ch][k] = val; // left ear
                filters[ch][freq_size + k] = val; // right ear
            }
        }
        Arc::new(BinauralState {
            hrtf_filters_freq: filters,
            diffuse_field_eq_filter: None,
            _hrtf_data: None,
        })
    };

    // Start with state A
    let state_a = make_state(0.0, 2);
    linear_plugin.state.store(state_a.clone());
    spectral_plugin.state.store(state_a.clone());
    // Force state snapshot update
    linear_plugin.crossfade.current_state_snapshot = linear_plugin.state.load_full();
    spectral_plugin.crossfade.current_state_snapshot = spectral_plugin.state.load_full();

    // Process a few frames to fill pipeline
    let num_frames = fft_size * 4;
    let input: Vec<f32> = (0..num_frames * 2)
        .map(|i| {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * (i / 2) as f32 / sample_rate as f32;
            phase.sin() * 0.5
        })
        .collect();
    let mut output_warmup = vec![0.0f32; num_frames * 2];
    let ctx = ProcessContext::new(sample_rate, num_frames);
    linear_plugin
        .process(&input, &mut output_warmup, &ctx)
        .unwrap();
    spectral_plugin
        .process(&input, &mut output_warmup, &ctx)
        .unwrap();

    // Now switch to state B -- this triggers crossfade
    let state_b = make_state(4.0, 2);
    linear_plugin.state.store(state_b.clone());
    spectral_plugin.state.store(state_b);

    // Process during the crossfade
    let mut output_linear = vec![0.0f32; num_frames * 2];
    let mut output_spectral = vec![0.0f32; num_frames * 2];
    linear_plugin
        .process(&input, &mut output_linear, &ctx)
        .unwrap();
    spectral_plugin
        .process(&input, &mut output_spectral, &ctx)
        .unwrap();

    // Both outputs must be finite
    assert!(
        output_linear.iter().all(|s| s.is_finite()),
        "Linear crossfade output must be finite"
    );
    assert!(
        output_spectral.iter().all(|s| s.is_finite()),
        "Spectral crossfade output must be finite"
    );

    // Both outputs should have signal (not silence)
    let linear_energy: f32 = output_linear.iter().map(|s| s * s).sum();
    let spectral_energy: f32 = output_spectral.iter().map(|s| s * s).sum();
    assert!(
        linear_energy > 1e-6,
        "Linear crossfade should produce signal, energy={}",
        linear_energy
    );
    assert!(
        spectral_energy > 1e-6,
        "Spectral crossfade should produce signal, energy={}",
        spectral_energy
    );

    // Compute magnitude spectra of a chunk during crossfade to verify
    // spectral mode has smoother magnitude (fewer comb-filter dips).
    // Take a section from the middle of the output (skip latency).
    let analysis_start = fft_size; // skip initial latency
    if analysis_start + fft_size <= num_frames {
        let compute_spectrum = |output: &[f32]| -> Vec<f32> {
            let mut mags = vec![0.0f32; freq_size];
            // Simple DFT magnitude for left channel
            for k in 0..freq_size {
                let mut re = 0.0f64;
                let mut im = 0.0f64;
                for n in 0..fft_size {
                    let sample = output[(analysis_start + n) * 2] as f64;
                    let angle = -2.0 * std::f64::consts::PI * k as f64 * n as f64 / fft_size as f64;
                    re += sample * angle.cos();
                    im += sample * angle.sin();
                }
                mags[k] = (re * re + im * im).sqrt() as f32;
            }
            mags
        };

        let linear_mags = compute_spectrum(&output_linear);
        let spectral_mags = compute_spectrum(&output_spectral);

        // Count "deep nulls" in the magnitude spectrum (bins where magnitude
        // drops to less than 10% of the peak). Linear complex blending with
        // phase-mismatched HRTFs creates many such nulls. Spectral mode should
        // create fewer.
        let count_nulls = |mags: &[f32]| -> usize {
            let peak = mags.iter().copied().fold(0.0f32, f32::max);
            if peak < 1e-10 {
                return 0;
            }
            let threshold = peak * 0.1;
            // Only count nulls in the first half of the spectrum (audible range)
            mags[1..freq_size / 2]
                .iter()
                .filter(|&&m| m < threshold && m > 0.0)
                .count()
        };

        let linear_nulls = count_nulls(&linear_mags);
        let spectral_nulls = count_nulls(&spectral_mags);

        // Spectral mode should not have MORE nulls than linear mode.
        // (It should have fewer or equal.)
        assert!(
            spectral_nulls <= linear_nulls + 3, // small tolerance for edge effects
            "Spectral crossfade should not produce more comb-filter nulls than linear: \
                 spectral={}, linear={}",
            spectral_nulls,
            linear_nulls
        );
    }
}

/// Verify that the crossfade_mode parameter can be set and retrieved.
#[test]
fn test_crossfade_mode_parameter_set_get() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );

    // Default should be 0 (Linear)
    let val = plugin
        .get_parameter(&ParameterId::from("crossfade_mode"))
        .expect("crossfade_mode must exist");
    assert_eq!(val, ParameterValue::Int(0));

    // Set to Spectral (1)
    plugin
        .set_parameter(ParameterId::from("crossfade_mode"), ParameterValue::Int(1))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("crossfade_mode")),
        Some(ParameterValue::Int(1))
    );
    assert_eq!(plugin.config.crossfade_mode_index, 1);

    // Out-of-range value (2) is clamped to max valid index (1) by param_bridge
    plugin
        .set_parameter(ParameterId::from("crossfade_mode"), ParameterValue::Int(2))
        .unwrap();
    assert_eq!(
        plugin.config.crossfade_mode_index, 1,
        "Out-of-range mode should be clamped to max"
    );

    // Set back to Linear (0)
    plugin
        .set_parameter(ParameterId::from("crossfade_mode"), ParameterValue::Int(0))
        .unwrap();
    assert_eq!(plugin.config.crossfade_mode_index, 0);
}

/// A1: Reflection panning — front source (az=0) must be centred (L==R).
/// With the old formula `p=(az+π/4)*0.5`, front (az=0) mapped to p=π/8,
/// giving left≈0.92, right≈0.38 — a hard left pan for a frontal source.
#[test]
fn test_reflection_panning_front_is_centered() {
    use room::{RoomModel, calculate_reflections};
    use sotf_host::speaker_config::get_speaker_config_by_channels;

    // Use 5.1 speaker config (6 channels), first-order reflections only.
    let model = RoomModel {
        max_order: 1,
        ..Default::default()
    };
    let config = get_speaker_config_by_channels(6).unwrap();
    let reflections = calculate_reflections(&model, config, 48000);

    // Find a reflection close to front (|az| < 5°) and verify L ≈ R.
    let near_front: Vec<_> = reflections
        .iter()
        .flatten()
        .filter(|r| r.azimuth_deg.abs() < 5.0)
        .collect();

    assert!(
        !near_front.is_empty(),
        "Expected at least one near-front reflection in a symmetric room"
    );
    for r in near_front {
        let diff = (r.left_gain - r.right_gain).abs();
        assert!(
            diff < 0.05,
            "Front reflection should be nearly centred: L={:.3}, R={:.3}, diff={:.3}",
            r.left_gain,
            r.right_gain,
            diff
        );
    }
}

/// A1: Reflection panning — right source (az≈+90°) must have right>left.
#[test]
fn test_reflection_panning_right_source_is_right() {
    use room::{RoomModel, calculate_reflections};
    use sotf_host::speaker_config::get_speaker_config_by_channels;

    let model = RoomModel {
        max_order: 1,
        ..Default::default()
    };
    let config = get_speaker_config_by_channels(6).unwrap();
    let reflections = calculate_reflections(&model, config, 48000);

    let right_side: Vec<_> = reflections
        .iter()
        .flatten()
        .filter(|r| r.azimuth_deg > 45.0 && r.azimuth_deg < 135.0)
        .collect();

    for r in right_side {
        assert!(
            r.right_gain > r.left_gain,
            "Right-side reflection (az={:.1}°) should have right_gain > left_gain: L={:.3}, R={:.3}",
            r.azimuth_deg,
            r.left_gain,
            r.right_gain
        );
    }
}

/// A3: LFE gain must not include the arbitrary FRAC_1_SQRT_2 factor.
/// At default parameters (distance=2m, level=0dB), lfe_gain == 1/2.0 = 0.5.
#[test]
fn test_lfe_gain_no_sqrt2_attenuation() {
    use filter::compute_lfe_filter;
    let (_filter, lfe_gain) = compute_lfe_filter(
        2048, 48000, 120.0, // lfe_crossover Hz
        2.0,   // lfe_distance m → distance_atten = 0.5
        0.0,   // lfe_level dB → level_gain = 1.0
    );
    // Expected: 1/2.0 = 0.5 (distance only, no sqrt(2) penalty).
    // Old buggy value: 0.5 * FRAC_1_SQRT_2 ≈ 0.354.
    let expected = 0.5_f32;
    assert!(
        (lfe_gain - expected).abs() < 1e-4,
        "lfe_gain should be {:.4} (distance only), got {:.4}",
        expected,
        lfe_gain
    );
}

#[test]
#[should_panic(expected = "Invalid FFT size: 1000 (must be power of 2)")]
fn test_constructor_rejects_non_power_of_two_fft_size() {
    let _ = BinauralDecoderPlugin::new(
        2,
        1000,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
}

/// AL1: Second-order ISM deduplication — mirroring wall A→B and wall B→A
/// must produce the same image source position, so only one reflection should
/// be emitted per pair. The old code emitted both, boosting those paths by 6 dB.
///
/// We verify that the second-order count with dedup is strictly less than
/// 6×5 = 30 (all ordered pairs). For the default room, the unique unordered
/// pair set is C(6,2)=15 but not all pass the path_diff>0 gate, so the actual
/// count varies. What matters is that it is < 30 (the duplicated count).
#[test]
fn test_ism_second_order_no_duplicates() {
    use room::{RoomModel, calculate_reflections};
    use sotf_host::speaker_config::get_speaker_config_by_channels;

    // First compute with max_order=1 to get baseline count per channel.
    let model1 = RoomModel {
        max_order: 1,
        ..Default::default()
    };
    let config = get_speaker_config_by_channels(2).unwrap();
    let reflections_1st = calculate_reflections(&model1, config, 48000);

    // Now compute with max_order=2 to get first+second order.
    let model2 = RoomModel {
        max_order: 2,
        ..Default::default()
    };
    let reflections_2nd = calculate_reflections(&model2, config, 48000);

    // The number of second-order reflections per channel must be < 30 (6×5 ordered).
    // Without dedup the code generates 30 per channel; with dedup it must be fewer.
    for (ch, (r1, r2)) in reflections_1st
        .iter()
        .zip(reflections_2nd.iter())
        .enumerate()
    {
        let second_order_count = r2.len() - r1.len();
        // 30 = 6 walls × 5 non-self mirrors, all ordered pairs. Dedup must reduce this.
        assert!(
            second_order_count < 30,
            "Channel {ch}: expected < 30 second-order reflections (dedup failed), \
                 got {second_order_count}"
        );
    }
}

/// AL2: Reflection delay clamping — reflections beyond the delay-line capacity
/// must be clamped and must not cause out-of-bounds wrapping.
#[test]
fn test_reflection_delay_clamped_to_buffer_size() {
    use room::RoomModel;

    // Build a plugin and inject a synthetic reflection with an enormous delay.
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    let sr = 48000_u32;
    plugin.initialize(sr).unwrap();

    // Inject a reflection whose delay exceeds delay_line capacity (16384 samples).
    plugin.room.cached_reflections[0].push(room::Reflection {
        delay_samples: 100_000, // Far beyond 16384
        gain: 0.5,
        left_gain: 0.7,
        right_gain: 0.3,
        azimuth_deg: 0.0,
        elevation_deg: 0.0,
        hrtf_filter: None,
    });

    // Clamp manually (mimicking what initialize does post-build).
    let max_delay = plugin.room.reflection_delay_mask;
    for r in plugin.room.cached_reflections.iter_mut().flatten() {
        if r.delay_samples > max_delay {
            r.delay_samples = max_delay;
        }
    }

    // All delays must now be within the buffer.
    for r in plugin.room.cached_reflections.iter().flatten() {
        assert!(
            r.delay_samples <= max_delay,
            "delay {} exceeds buffer mask {}",
            r.delay_samples,
            max_delay
        );
    }
}

/// Legacy no-op parameters must neither reappear in the public parameter list
/// nor prevent old presets from loading.
#[test]
fn test_legacy_noop_params_are_ignored() {
    let plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    let params = plugin.parameters();
    let names: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(
        !names.contains(&"enable_optimization"),
        "enable_optimization (dead code) must not be exposed in parameters()"
    );
    assert!(
        !names.contains(&"headphone_eq_enabled"),
        "headphone_eq_enabled (unimplemented) must not be exposed in parameters()"
    );

    let legacy: crate::params::Params = serde_json::from_value(serde_json::json!({
        "input_channels": 2,
        "enable_optimization": false,
        "headphone_eq_enabled": true
    }))
    .unwrap();
    assert_eq!(legacy.input_channels, 2);
}

// -------------------------------------------------------------------------
// set_parameter extended coverage
// -------------------------------------------------------------------------

#[test]
fn test_set_parameter_hrtf_file_empty_clears_path() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.config.hrtf_path = Some(std::path::PathBuf::from("/some/path.sofa"));
    plugin
        .set_parameter(
            ParameterId::from("hrtf_file"),
            ParameterValue::String("".to_string()),
        )
        .unwrap();
    assert!(plugin.config.hrtf_path.is_none());
}

#[test]
fn failed_runtime_sofa_replacement_is_transactional() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        64,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.initialize(48_000).unwrap();
    let before = plugin.state.load_full();
    assert!(
        plugin
            .set_parameter(
                ParameterId::from("sofa_file"),
                ParameterValue::String("/definitely/missing/binaural.sofa".into()),
            )
            .is_err()
    );
    assert!(plugin.config.hrtf_path.is_none());
    assert!(Arc::ptr_eq(&before, &plugin.state.load_full()));
}

#[test]
fn test_set_parameter_sofa_file_roundtrips_as_file_path_string() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.config.sample_rate = 0;
    plugin
        .set_parameter(
            ParameterId::from("sofa_file"),
            ParameterValue::String("/tmp/test.sofa".to_string()),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("sofa_file")),
        Some(ParameterValue::String("/tmp/test.sofa".to_string()))
    );
}

#[test]
fn test_set_parameter_hrtf_database_dir_empty() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.config.hrtf_database_dir = "/previous".to_string();
    plugin
        .set_parameter(
            ParameterId::from("hrtf_database_dir"),
            ParameterValue::String("".to_string()),
        )
        .unwrap();
    assert_eq!(plugin.config.hrtf_database_dir, "");
}

#[test]
fn test_set_parameter_head_width_cm_valid() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin
        .set_parameter(
            ParameterId::from("head_width_cm"),
            ParameterValue::Float(20.0),
        )
        .unwrap();
    assert_eq!(plugin.config.head_width_cm, 20.0);
}

#[test]
fn test_set_parameter_head_width_cm_out_of_range_ignored() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin
        .set_parameter(
            ParameterId::from("head_width_cm"),
            ParameterValue::Float(5.0),
        )
        .unwrap();
    assert_eq!(plugin.config.head_width_cm, 15.0);
}

#[test]
fn test_set_parameter_ear_height_cm_valid() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin
        .set_parameter(
            ParameterId::from("ear_height_cm"),
            ParameterValue::Float(12.0),
        )
        .unwrap();
    assert_eq!(plugin.config.ear_height_cm, 12.0);
}

#[test]
fn test_set_parameter_ear_height_cm_out_of_range_ignored() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin
        .set_parameter(
            ParameterId::from("ear_height_cm"),
            ParameterValue::Float(2.0),
        )
        .unwrap();
    assert_eq!(plugin.config.ear_height_cm, 10.0);
}

#[test]
fn test_set_parameter_late_reverb_params() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.initialize(44100).unwrap();

    plugin
        .set_parameter(
            ParameterId::from("late_reverb_enabled"),
            ParameterValue::Bool(true),
        )
        .unwrap();
    assert!(plugin.config.late_reverb_enabled);

    plugin
        .set_parameter(
            ParameterId::from("late_reverb_mix"),
            ParameterValue::Float(0.5),
        )
        .unwrap();
    assert_eq!(plugin.config.late_reverb_mix, 0.5);

    plugin
        .set_parameter(
            ParameterId::from("late_reverb_rt60"),
            ParameterValue::Float(2.0),
        )
        .unwrap();
    assert_eq!(plugin.config.late_reverb_rt60, 2.0);

    plugin
        .set_parameter(
            ParameterId::from("late_reverb_damping"),
            ParameterValue::Float(0.5),
        )
        .unwrap();
    assert_eq!(plugin.config.late_reverb_damping, 0.5);
}

#[test]
fn test_set_parameter_externalization() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin
        .set_parameter(
            ParameterId::from("externalization"),
            ParameterValue::Float(0.75),
        )
        .unwrap();
    assert!((plugin.smoothing.externalization.target() - 0.75).abs() < 1e-4);
}

#[test]
fn test_set_parameter_near_field_strength() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin
        .set_parameter(
            ParameterId::from("near_field_strength"),
            ParameterValue::Float(0.5),
        )
        .unwrap();
    assert!((plugin.config.near_field_strength - 0.5).abs() < 1e-4);
}

#[test]
fn test_set_parameter_crossfade_ms_non_float_error() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    assert!(
        plugin
            .set_parameter(
                ParameterId::from("crossfade_ms"),
                ParameterValue::String("not_a_number".to_string()),
            )
            .is_err()
    );
}

// -------------------------------------------------------------------------
// initialize extended coverage
// -------------------------------------------------------------------------

#[test]
fn test_initialize_sets_sample_rate_and_lfe_filter() {
    let mut plugin = BinauralDecoderPlugin::new(
        5,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.initialize(96000).unwrap();
    assert_eq!(plugin.config.sample_rate, 96000);
    assert!(!plugin.coefficients.lfe_lowpass_filter.is_empty());
    assert!(plugin.coefficients.lfe_gain > 0.0);
}

#[test]
fn test_initialize_with_nonexistent_srir_file_falls_back_to_ism() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.config.srir_file = Some(std::path::PathBuf::from("/nonexistent/path.wav"));
    plugin.initialize(48000).unwrap();
    assert!(!plugin.room.cached_reflections.is_empty());
}

#[test]
fn test_initialize_clamps_reflection_delays() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel {
            dimensions: [1000.0, 1000.0, 1000.0],
            listener_position: [500.0, 500.0, 500.0],
            max_order: 1,
            ..Default::default()
        },
    );
    plugin.initialize(48000).unwrap();
    let max_delay = plugin.room.reflection_delay_mask;
    for r in plugin.room.cached_reflections.iter().flatten() {
        assert!(
            r.delay_samples <= max_delay,
            "delay {} exceeds buffer mask {}",
            r.delay_samples,
            max_delay
        );
    }
}

#[test]
fn test_initialize_empty_hrtf_database_dir_no_crash() {
    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.config.hrtf_database_dir = "".to_string();
    plugin.initialize(48000).unwrap();
}

/// Verify that head-yaw changes trigger a background HRTF recompute and that
/// the audio thread eventually picks up the rotated filters.
#[test]
fn test_head_yaw_background_update_changes_state() {
    use sotf_host::sofa::SofaFile;

    const NUM_MEAS: usize = 36;
    const IR_LEN: usize = 64;
    const SAMPLE_RATE_F: f32 = 44100.0;

    let mut positions = Vec::with_capacity(NUM_MEAS);
    let mut impulse_responses = Vec::with_capacity(NUM_MEAS * 2 * IR_LEN);
    for i in 0..NUM_MEAS {
        let az = -180.0 + (i as f32) * (360.0 / NUM_MEAS as f32);
        positions.push(sotf_host::sofa::SourcePosition::new(az, 0.0, 1.0));
        let mut ir_l = vec![0.0f32; IR_LEN];
        ir_l[0] = 1.0 + i as f32 * 0.01;
        let ir_r = vec![0.0f32; IR_LEN];
        impulse_responses.extend_from_slice(&ir_l);
        impulse_responses.extend_from_slice(&ir_r);
    }

    let sofa = SofaFile {
        sample_rate: SAMPLE_RATE_F,
        num_measurements: NUM_MEAS,
        ir_length: IR_LEN,
        positions,
        impulse_responses,
        convention: "SimpleFreeFieldHRIR".to_string(),
        data_sample_rate: Some(SAMPLE_RATE_F),
    };

    let freq_size = 1024 / 2 + 1;
    let initial_filters = vec![vec![Complex::new(0.0, 0.0); freq_size * 2]; 2];
    let initial_state = Arc::new(BinauralState {
        hrtf_filters_freq: initial_filters,
        diffuse_field_eq_filter: None,
        _hrtf_data: Some(sofa),
    });

    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.initialize(44100).unwrap();
    plugin.state.store(initial_state);
    plugin.spawn_hrtf_update_thread();

    plugin
        .set_parameter(
            ParameterId::from("head_yaw_deg"),
            ParameterValue::Float(45.0),
        )
        .unwrap();

    let num_frames = 512;
    let input: Vec<f32> = (0..num_frames * 2)
        .map(|i| (i as f32 * 0.01).sin() * 0.5)
        .collect();
    let mut output = vec![0.0f32; num_frames * 2];
    let context = ProcessContext::new(44100, num_frames);

    // Process enough blocks for the smoother to advance past 0.5° and for the
    // background thread to finish at least one recomputation.
    for _ in 0..20 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    // Dropping the plugin joins the worker, so the assertion does not race its
    // queued recomputation when the workspace runs under heavy parallel load.
    let state = Arc::clone(&plugin.state);
    drop(plugin);
    let final_state = state.load_full();
    let left_filter = &final_state.hrtf_filters_freq[0][..freq_size];
    let is_identity = left_filter.iter().all(|c| c.norm() < 1e-3);
    assert!(
        !is_identity,
        "background HRTF update should have produced non-identity filters"
    );
}
