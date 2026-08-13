//! Integration tests for the SOTF Stereo-to-Surround Upmixer plugin.
//!
//! Tests exercise the public `Plugin` trait: instantiation, parameter get/set,
//! audio processing, output channel configuration, bypass modes and reset.

use sotf_host::param_specs::ParamType;
use sotf_host::{ParameterId, ParameterValue, Plugin, ProcessContext};
use sotf_plugin_upmixer::params::PARAMS;
use sotf_plugin_upmixer::{UpmixerPlugin, UpmixerPluginParams};

#[test]
fn upmixer_plugin_info_and_channels() {
    let plugin = UpmixerPlugin::from_params(UpmixerPluginParams::default());
    assert!(plugin.info().name.contains("Upmixer"));
    assert_eq!(plugin.input_channels(), 2);
    // Default config is 5.1 => 6 channels.
    assert_eq!(plugin.output_channels(), 6);
}

#[test]
fn empty_factory_defaults_match_every_catalog_parameter() {
    let params: UpmixerPluginParams = serde_json::from_str("{}").unwrap();
    let plugin = UpmixerPlugin::from_params(params);

    for spec in PARAMS {
        let actual = plugin
            .get_parameter(&ParameterId::from(spec.engine_key))
            .unwrap_or_else(|| panic!("catalog parameter {} has no getter", spec.engine_key));
        let expected = match spec.param_type {
            ParamType::Float { default, .. } => ParameterValue::Float(default as f32),
            ParamType::Int { default, .. } => ParameterValue::Int(default as i32),
            ParamType::Bool { default, .. } => ParameterValue::Bool(default),
            ParamType::Choice { default_index, .. } => ParameterValue::Int(default_index as i32),
            ParamType::FilePath => continue,
        };
        assert_eq!(actual, expected, "default mismatch for {}", spec.engine_key);
    }
}

#[test]
fn upmixer_reports_causal_overlap_latency() {
    let params = UpmixerPluginParams::default();
    let fft_size = params.core.fft_size;
    let plugin = UpmixerPlugin::from_params(params);
    assert_eq!(plugin.latency_samples(), fft_size);
}

#[test]
fn upmixer_streamed_impulse_matches_reported_latency() {
    for block_size in [128usize, 512, 1024, 2048] {
        let params = UpmixerPluginParams::default();
        let fft_size = params.core.fft_size;
        let mut plugin = UpmixerPlugin::from_params(params);
        plugin.initialize(48_000).unwrap();

        let impulse_index = fft_size / 2;
        let total_frames = fft_size * 4;
        let mut input = vec![0.0f32; total_frames * 2];
        input[impulse_index * 2] = 1.0;
        input[impulse_index * 2 + 1] = 1.0;
        let out_channels = plugin.output_channels();
        let mut output = Vec::with_capacity(total_frames * out_channels);
        for chunk in input.chunks(block_size * 2) {
            let frames = chunk.len() / 2;
            let mut block_output = vec![0.0f32; frames * out_channels];
            plugin
                .process(
                    chunk,
                    &mut block_output,
                    &ProcessContext::new(48_000, frames),
                )
                .unwrap();
            output.extend_from_slice(&block_output);
        }

        let peak_frame = output
            .chunks_exact(out_channels)
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                let a_peak = a.iter().copied().map(f32::abs).fold(0.0f32, f32::max);
                let b_peak = b.iter().copied().map(f32::abs).fold(0.0f32, f32::max);
                a_peak.total_cmp(&b_peak)
            })
            .map(|(frame, _)| frame)
            .unwrap();
        assert_eq!(
            peak_frame.saturating_sub(impulse_index),
            plugin.latency_samples(),
            "block size {block_size} changed upmixer latency"
        );
    }
}

#[test]
fn upmixer_instantiate_from_params_custom_config() {
    let mut params = UpmixerPluginParams::default();
    params.core.speaker_config = "7.1".to_string();
    let plugin = UpmixerPlugin::from_params(params);
    assert_eq!(plugin.output_channels(), 8);
}

#[test]
fn upmixer_parameter_roundtrip() {
    let mut plugin = UpmixerPlugin::from_params(UpmixerPluginParams::default());
    plugin.initialize(44100).unwrap();

    let params = plugin.parameters();
    assert!(params.iter().any(|p| p.id.as_str() == "gain_front_direct"));
    assert!(params.iter().any(|p| p.id.as_str() == "gain_rear_ambient"));
    assert!(params.iter().any(|p| p.id.as_str() == "lfe_gain"));

    plugin
        .set_parameter(
            ParameterId::from("gain_front_direct"),
            ParameterValue::Float(0.8),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("gain_rear_ambient"),
            ParameterValue::Float(1.25),
        )
        .unwrap();

    assert_eq!(
        plugin.get_parameter(&ParameterId::from("gain_front_direct")),
        Some(ParameterValue::Float(0.8))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("gain_rear_ambient")),
        Some(ParameterValue::Float(1.25))
    );
}

#[test]
fn upmixer_unknown_parameter_error() {
    let mut plugin = UpmixerPlugin::from_params(UpmixerPluginParams::default());
    let err = plugin
        .set_parameter(
            ParameterId::from("no_such_param"),
            ParameterValue::Float(1.0),
        )
        .unwrap_err();
    assert!(err.contains("Unknown parameter") || err.contains("no_such_param"));
}

#[test]
fn upmixer_process_silence() {
    let mut plugin = UpmixerPlugin::from_params(UpmixerPluginParams::default());
    plugin.initialize(44100).unwrap();

    let num_frames = 4096;
    let input = vec![0.0_f32; num_frames * 2];
    let mut output = vec![0.0_f32; num_frames * plugin.output_channels()];
    let context = ProcessContext::new(44100, num_frames);

    plugin.process(&input, &mut output, &context).unwrap();

    let energy: f32 = output.iter().map(|s| s * s).sum();
    assert_eq!(energy, 0.0, "silent input should produce silent output");
}

#[test]
fn upmixer_process_stereo_to_surround() {
    let mut plugin = UpmixerPlugin::from_params(UpmixerPluginParams::default());
    plugin.initialize(44100).unwrap();

    let num_frames = 4096;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let t = i as f32 / 44100.0;
        input[i * 2] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.3;
        input[i * 2 + 1] = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.3;
    }

    let out_ch = plugin.output_channels();
    let mut output = vec![0.0_f32; num_frames * out_ch];
    let context = ProcessContext::new(44100, num_frames);

    plugin.process(&input, &mut output, &context).unwrap();

    let total_energy: f32 = output.iter().map(|s| s * s).sum();
    assert!(total_energy > 0.0, "upmixed output should have energy");

    // Each output channel should carry some energy.
    for ch in 0..out_ch {
        let ch_energy: f32 = (0..num_frames)
            .map(|i| output[i * out_ch + ch].powi(2))
            .sum();
        assert!(ch_energy > 0.0, "channel {} should have energy", ch);
    }
}

#[test]
fn upmixer_bypass_all_processing_passes_stereo() {
    let mut params = UpmixerPluginParams::default();
    params.bypass.bypass_all_processing = true;
    let mut plugin = UpmixerPlugin::from_params(params);
    plugin.initialize(44100).unwrap();

    let num_frames = 512;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        input[i * 2] = 0.4;
        input[i * 2 + 1] = -0.4;
    }

    let out_ch = plugin.output_channels();
    let mut output = vec![0.0_f32; num_frames * out_ch];
    let context = ProcessContext::new(44100, num_frames);
    plugin.process(&input, &mut output, &context).unwrap();

    for i in 0..num_frames {
        assert!((output[i * out_ch] - 0.4).abs() < 1e-5);
        assert!((output[i * out_ch + 1] - (-0.4)).abs() < 1e-5);
        for ch in 2..out_ch {
            assert_eq!(output[i * out_ch + ch], 0.0);
        }
    }
}

#[test]
fn upmixer_bypass_round_trip_discards_queued_audio_and_restores_latency() {
    let mut plugin = UpmixerPlugin::from_params(UpmixerPluginParams::default());
    plugin.initialize(44100).unwrap();
    let fft_size = UpmixerPluginParams::default().core.fft_size;
    let out_ch = plugin.output_channels();

    // Build both partial input state and queued overlap-add output before entering
    // diagnostic bypass. A bypass round trip must not replay either after re-enable.
    let active_frames = fft_size * 3;
    let mut active_input = vec![0.0_f32; active_frames * 2];
    for frame in 0..active_frames {
        active_input[frame * 2] = 0.7;
        active_input[frame * 2 + 1] = -0.4;
    }
    let mut active_output = vec![0.0_f32; active_frames * out_ch];
    plugin
        .process(
            &active_input,
            &mut active_output,
            &ProcessContext::new(44100, active_frames),
        )
        .unwrap();

    plugin
        .set_parameter(
            ParameterId::from("bypass_all_processing"),
            ParameterValue::Bool(true),
        )
        .unwrap();
    assert_eq!(plugin.latency_samples(), 0);

    // Use a non-hop-aligned host block while bypassed.
    let bypass_frames = 257;
    let bypass_input = vec![0.11_f32; bypass_frames * 2];
    let mut bypass_output = vec![0.0_f32; bypass_frames * out_ch];
    plugin
        .process(
            &bypass_input,
            &mut bypass_output,
            &ProcessContext::new(44100, bypass_frames),
        )
        .unwrap();

    plugin
        .set_parameter(
            ParameterId::from("bypass_all_processing"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    assert_eq!(plugin.latency_samples(), fft_size);

    // Re-enable with silence. Any non-zero output here indicates stale queued
    // main/HR ring data leaked across the structural bypass transition.
    let post_frames = fft_size * 3;
    let post_input = vec![0.0_f32; post_frames * 2];
    let mut post_output = vec![0.0_f32; post_frames * out_ch];
    let written = plugin
        .process(
            &post_input,
            &mut post_output,
            &ProcessContext::new(44100, post_frames),
        )
        .unwrap();
    assert_eq!(written, post_frames);
    let residual = post_output
        .iter()
        .copied()
        .map(f32::abs)
        .fold(0.0, f32::max);
    assert!(
        residual < 1e-6,
        "stale audio survived bypass round trip: {residual}"
    );
}

#[test]
fn upmixer_state_change_low_latency_fft() {
    let mut plugin = UpmixerPlugin::from_params(UpmixerPluginParams::default());
    plugin.initialize(44100).unwrap();

    let latency_before = plugin.latency_samples();
    plugin
        .set_parameter(ParameterId::from("low_latency"), ParameterValue::Bool(true))
        .unwrap();
    let latency_after = plugin.latency_samples();

    assert!(
        latency_after < latency_before,
        "low-latency mode should reduce reported latency"
    );
}

#[test]
fn upmixer_reset_clears_state() {
    let mut plugin = UpmixerPlugin::from_params(UpmixerPluginParams::default());
    plugin.initialize(44100).unwrap();

    let latency = plugin.latency_samples();
    let num_frames = latency + 4096;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        input[i * 2] = (i as f32 * 0.01).sin() * 0.5;
        input[i * 2 + 1] = (i as f32 * 0.015).cos() * 0.5;
    }

    let out_ch = plugin.output_channels();
    let mut output1 = vec![0.0_f32; num_frames * out_ch];
    let context = ProcessContext::new(44100, num_frames);
    plugin.process(&input, &mut output1, &context).unwrap();

    // Output after the initial latency should be non-silent.
    let energy1: f32 = output1[latency * out_ch..].iter().map(|s| s * s).sum();
    assert!(
        energy1 > 0.0,
        "first process should produce non-silent output"
    );

    plugin.reset();

    let mut output2 = vec![0.0_f32; num_frames * out_ch];
    plugin.process(&input, &mut output2, &context).unwrap();

    // After reset, the plugin should still recover and produce non-silent output.
    let energy2: f32 = output2[latency * out_ch..].iter().map(|s| s * s).sum();
    assert!(
        energy2 > 0.0,
        "output after reset should still have energy beyond latency"
    );
}

#[test]
fn upmixer_invalid_sample_rate_error() {
    let mut plugin = UpmixerPlugin::from_params(UpmixerPluginParams::default());
    let err = plugin.initialize(0).unwrap_err();
    assert!(err.contains("Invalid sample rate"));
}

#[test]
fn upmixer_rejects_non_finite_input_without_poisoning_following_audio() {
    let mut plugin = UpmixerPlugin::from_params(UpmixerPluginParams::default());
    plugin.initialize(48_000).unwrap();
    let frames = 64;
    let channels = plugin.output_channels();
    let mut invalid = vec![0.0_f32; frames * 2];
    invalid[17] = f32::NAN;
    let mut output = vec![0.0_f32; frames * channels];
    let context = ProcessContext::new(48_000, frames);
    assert!(plugin.process(&invalid, &mut output, &context).is_err());

    let valid = vec![0.0_f32; frames * 2];
    plugin.process(&valid, &mut output, &context).unwrap();
    assert!(output.iter().all(|sample| sample.is_finite()));
}

#[test]
fn multi_source_reconstruction_has_a_bounded_energy_budget_for_stereo_extremes() {
    const WARMUP_FRAMES: usize = 8192;
    const MEASURE_FRAMES: usize = 8192;
    for layout in ["5.1", "9.1.6"] {
        for scenario in ["correlated", "anti_phase", "quadrature", "independent"] {
            let mut params = UpmixerPluginParams::default();
            params.core.speaker_config = layout.to_string();
            params.spectral.multi_source_extraction = true;
            let mut plugin = UpmixerPlugin::from_params(params);
            plugin.initialize(48_000).unwrap();
            let channels = plugin.output_channels();
            let latency = plugin.latency_samples();
            let total_frames = WARMUP_FRAMES + MEASURE_FRAMES + latency;
            let mut input = vec![0.0_f32; total_frames * 2];
            let mut noise_left = 0x1234_5678_u32;
            let mut noise_right = 0x9abc_def0_u32;
            for frame in 0..total_frames {
                let phase = frame as f32 * std::f32::consts::TAU * 997.0 / 48_000.0;
                let (left, right) = match scenario {
                    "correlated" => {
                        let sample = phase.sin() * 0.25;
                        (sample, sample)
                    }
                    "anti_phase" => {
                        let sample = phase.sin() * 0.25;
                        (sample, -sample)
                    }
                    "quadrature" => (phase.sin() * 0.25, phase.cos() * 0.25),
                    _ => {
                        noise_left = noise_left
                            .wrapping_mul(1_664_525)
                            .wrapping_add(1_013_904_223);
                        noise_right = noise_right.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                        (
                            (noise_left as f32 / u32::MAX as f32 - 0.5) * 0.5,
                            (noise_right as f32 / u32::MAX as f32 - 0.5) * 0.5,
                        )
                    }
                };
                input[frame * 2] = left;
                input[frame * 2 + 1] = right;
            }

            let mut output = vec![0.0_f32; total_frames * channels];
            plugin
                .process(
                    &input,
                    &mut output,
                    &ProcessContext::new(48_000, total_frames),
                )
                .unwrap();
            assert!(
                output.iter().all(|sample| sample.is_finite()),
                "{layout}/{scenario}"
            );
            let input_start = WARMUP_FRAMES * 2;
            let input_end = (WARMUP_FRAMES + MEASURE_FRAMES) * 2;
            let output_start = (WARMUP_FRAMES + latency) * channels;
            let output_end = (WARMUP_FRAMES + latency + MEASURE_FRAMES) * channels;
            let input_energy: f32 = input[input_start..input_end]
                .iter()
                .map(|sample| sample * sample)
                .sum();
            let output_energy: f32 = output[output_start..output_end]
                .iter()
                .map(|sample| sample * sample)
                .sum();
            let energy_delta_db = 10.0 * (output_energy / input_energy).log10();
            assert!(
                (-4.0..=2.0).contains(&energy_delta_db),
                "{layout}/{scenario} exceeded the latency-aligned reconstruction policy: \
                 input={input_energy}, output={output_energy}, delta={energy_delta_db:.3} dB"
            );
        }
    }
}

#[test]
fn multi_source_render_is_equivalent_across_host_block_partitions() {
    const FRAMES: usize = 12_317;
    let mut input = vec![0.0_f32; FRAMES * 2];
    let mut left_state = 0x3141_5926_u32;
    let mut right_state = 0x2718_2818_u32;
    for frame in 0..FRAMES {
        left_state = left_state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        right_state = right_state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        input[frame * 2] = (left_state as f32 / u32::MAX as f32 - 0.5) * 0.3;
        input[frame * 2 + 1] = (right_state as f32 / u32::MAX as f32 - 0.5) * 0.3;
    }

    let make_plugin = || {
        let mut params = UpmixerPluginParams::default();
        params.spectral.multi_source_extraction = true;
        let mut plugin = UpmixerPlugin::from_params(params);
        plugin.initialize(48_000).unwrap();
        plugin
    };

    let mut whole = make_plugin();
    let channels = whole.output_channels();
    let mut whole_output = vec![0.0_f32; FRAMES * channels];
    whole
        .process(
            &input,
            &mut whole_output,
            &ProcessContext::new(48_000, FRAMES),
        )
        .unwrap();

    let mut partitioned = make_plugin();
    let mut partitioned_output = vec![0.0_f32; FRAMES * channels];
    let mut frame = 0;
    for block_frames in [1usize, 257, 1024, 31, 4096].into_iter().cycle() {
        if frame == FRAMES {
            break;
        }
        let block_frames = block_frames.min(FRAMES - frame);
        partitioned
            .process(
                &input[frame * 2..(frame + block_frames) * 2],
                &mut partitioned_output[frame * channels..(frame + block_frames) * channels],
                &ProcessContext::new(48_000, block_frames),
            )
            .unwrap();
        frame += block_frames;
    }

    let max_delta = whole_output
        .iter()
        .zip(&partitioned_output)
        .map(|(whole, partitioned)| (whole - partitioned).abs())
        .fold(0.0_f32, f32::max);
    assert!(
        max_delta <= 2e-5,
        "multi-source output changed with host partitioning: max delta {max_delta}"
    );
}

#[test]
fn upmixer_params_serde_flatten_roundtrip() {
    let original = UpmixerPluginParams::default();
    let json = serde_json::to_value(&original).unwrap();
    // Flat serialization: no nested "core"/"gains" objects.
    assert!(json.get("fft_size").is_some());
    assert!(json.get("core").is_none());

    // Empty JSON and flat keys deserialize correctly.
    let from_empty: UpmixerPluginParams = serde_json::from_str("{}").unwrap();
    assert_eq!(from_empty.core.fft_size, original.core.fft_size);

    let from_flat: UpmixerPluginParams =
        serde_json::from_str(r#"{"fft_size":1024,"speaker_config":"7.1","height_gain":0.8}"#)
            .unwrap();
    assert_eq!(from_flat.core.fft_size, 1024);
    assert_eq!(from_flat.core.speaker_config, "7.1");
    assert_eq!(from_flat.height.height_gain, 0.8);
}
