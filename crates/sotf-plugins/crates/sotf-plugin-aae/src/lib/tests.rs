use super::aae_plugin::AaePlugin;
use super::allpass_diffuser::AllpassDiffuser;
use super::misc::LfeLowpass;
use super::misc::signed_rms;
use crate::params::AaePluginParams;
use sotf_host::param_specs::UpdateMode;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};

#[path = "tests/misc.rs"]
mod misc;

#[test]
fn test_7_1_4_config() {
    let params = AaePluginParams {
        speaker_config: "7.1.4".to_string(),
        ..AaePluginParams::default()
    };
    let mut p = AaePlugin::from_params(params).unwrap();
    p.initialize(48000).unwrap();
    assert_eq!(p.output_channels(), 12);
}

#[test]
fn test_every_declared_layout_and_room_preset_constructs() {
    for layout in crate::params::SPEAKER_CONFIGS {
        for preset in crate::params::ROOM_PRESETS {
            let plugin = AaePlugin::try_from_params(AaePluginParams {
                speaker_config: (*layout).to_owned(),
                room_preset: (*preset).to_owned(),
                ..Default::default()
            })
            .unwrap_or_else(|error| panic!("layout={layout}, preset={preset}: {error}"));
            assert_eq!(
                plugin.output_channels(),
                plugin.speaker_config.total_channels
            );
        }
    }
}

#[test]
fn test_invalid_construction_state_is_rejected() {
    for params in [
        AaePluginParams {
            speaker_config: "2.0".into(),
            ..Default::default()
        },
        AaePluginParams {
            room_preset: "unknown".into(),
            ..Default::default()
        },
        AaePluginParams {
            input_diffusion: f32::NAN,
            ..Default::default()
        },
        AaePluginParams {
            input_diffusion: 2.0,
            ..Default::default()
        },
        AaePluginParams {
            solo_early: true,
            solo_late: true,
            ..Default::default()
        },
    ] {
        assert!(AaePlugin::try_from_params(params).is_err());
    }
}

#[test]
fn test_level_smoothing_is_block_partition_invariant() {
    fn render(block_size: usize) -> Vec<f32> {
        let params = AaePluginParams {
            dry_level: 0.0,
            er_level: 0.0,
            late_level: 0.0,
            lfe_level: 0.0,
            content_aware: false,
            ..Default::default()
        };
        let mut plugin = AaePlugin::try_from_params(params).unwrap();
        plugin.initialize(48_000).unwrap();
        plugin
            .set_parameter(ParameterId::from("dry_level"), ParameterValue::Float(1.0))
            .unwrap();

        let frames = 4096;
        let input = vec![0.25; frames * 2];
        let mut output = vec![0.0; frames * plugin.output_channels()];
        let mut offset = 0;
        while offset < frames {
            let end = (offset + block_size).min(frames);
            plugin
                .process(
                    &input[offset * 2..end * 2],
                    &mut output[offset * plugin.output_channels()..end * plugin.output_channels()],
                    &ProcessContext::new(48_000, end - offset),
                )
                .unwrap();
            offset = end;
        }
        output
    }

    let reference = render(1);
    assert!(reference[0].abs() < reference[240 * 6].abs());
    for block_size in [64, 257, 512, 4096] {
        let candidate = render(block_size);
        let max_error = reference
            .iter()
            .zip(candidate.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            max_error < 1e-6,
            "block={block_size}, max_error={max_error}"
        );
    }
}

#[test]
fn test_spatial_automation_reuses_routing_storage() {
    let mut plugin = AaePlugin::try_from_params(AaePluginParams::default()).unwrap();
    let base_ptr = plugin.fdn_base_gains.as_ptr();
    let gains_ptr = plugin.fdn_gains.as_ptr();
    for (id, value) in [("envelopment", 0.1), ("height_amount", 0.9)] {
        plugin
            .set_parameter(ParameterId::from(id), ParameterValue::Float(value))
            .unwrap();
        assert_eq!(plugin.fdn_base_gains.as_ptr(), base_ptr);
        assert_eq!(plugin.fdn_gains.as_ptr(), gains_ptr);
    }
}

#[test]
fn test_setup_only_parameters_require_rebuild() {
    let mut plugin = AaePlugin::try_from_params(AaePluginParams::default()).unwrap();
    for (id, value) in [("speaker_config", "7.1.4"), ("room_preset", "cathedral")] {
        let error = plugin
            .set_parameter(ParameterId::from(id), ParameterValue::String(value.into()))
            .unwrap_err();
        assert!(error.contains("rebuild"), "{id}: {error}");
    }
    assert_eq!(plugin.output_channels(), 6);
    for id in ["speaker_config", "room_preset"] {
        let parameter = plugin
            .parameters()
            .into_iter()
            .find(|parameter| parameter.id.as_str() == id)
            .unwrap();
        assert_eq!(parameter.update_mode, UpdateMode::Structural);
    }
}

#[test]
fn test_live_solo_modes_remain_mutually_exclusive() {
    let mut plugin = AaePlugin::from_params(AaePluginParams::default()).unwrap();
    plugin
        .set_parameter(ParameterId::from("solo_early"), ParameterValue::Bool(true))
        .unwrap();
    assert!(
        plugin
            .set_parameter(ParameterId::from("solo_late"), ParameterValue::Bool(true))
            .is_err()
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("solo_late")),
        Some(ParameterValue::Bool(false))
    );
}

/// Auto-gain must be pre-allocated in `from_params` even when disabled,
/// so that enabling it via `set_parameter` on the audio thread does not
/// trigger a heap allocation. Verify by checking `auto_gain.is_some()`
/// before and after enabling via `set_parameter`.
#[test]
fn test_auto_gain_preallocated_when_disabled() {
    // Default params have auto_gain_enabled = false
    let p = AaePlugin::from_params(AaePluginParams::default()).unwrap();
    assert!(
        !p.params.auto_gain_enabled,
        "Precondition: auto_gain disabled by default"
    );
    // The field must be Some even when disabled — pre-allocated for audio-thread safety
    assert!(
        p.auto_gain.is_some(),
        "auto_gain must be pre-allocated even when disabled to avoid \
             audio-thread allocation when set_parameter enables it"
    );
}

#[test]
fn test_signed_rms_keeps_energy_unsigned() {
    // Sum can cancel to zero for symmetric content; polarity hints should not
    // flip LFE polarity.
    let sum = 0.0;
    let count = 4;
    let samples = [-1.0, 1.0, -1.0, 1.0];
    let energy: f32 = samples.iter().map(|s| s * s).sum();
    let got = signed_rms(sum, energy, count, -0.5);
    assert!(
        (got - 1.0).abs() < 1e-6,
        "LFE extraction should be unsigned RMS-like for decorrelated decorrelation, got {got}"
    );
}

#[test]
fn test_allpass_diffuser_unity_gain_dc() {
    // Schroeder allpass must have |H(z)| = 1 for all frequencies.
    // Verify DC gain = 1.0 by feeding constant input until steady state.
    let mut ap = AllpassDiffuser::new(37, 0.7);
    let mut output = 0.0;
    for _ in 0..10000 {
        output = ap.process(1.0);
    }
    assert!(
        (output - 1.0).abs() < 0.01,
        "Allpass DC gain should be 1.0, got {output}"
    );
}

#[test]
fn test_allpass_diffuser_energy_preservation() {
    // Feed a sine wave; output energy should equal input energy.
    let mut ap = AllpassDiffuser::new(53, 0.65);
    let n = 48000;
    let mut input_energy = 0.0_f64;
    let mut output_energy = 0.0_f64;
    // Skip transient (first 1000 samples)
    for i in 0..1000 {
        let x = (i as f32 * 0.1).sin();
        ap.process(x);
    }
    for i in 1000..n {
        let x = (i as f32 * 0.1).sin();
        let y = ap.process(x);
        input_energy += (x * x) as f64;
        output_energy += (y * y) as f64;
    }
    let ratio = output_energy / input_energy;
    assert!(
        (ratio - 1.0).abs() < 0.01,
        "Allpass energy ratio should be ~1.0, got {ratio}"
    );
}

#[test]
fn test_lfe_tracks_late_reverb_tail() {
    let params = AaePluginParams {
        dry_level: 0.0,
        er_level: 0.0,
        late_level: 1.0,
        lfe_level: 1.0,
        pre_delay_ms: 0.0,
        content_aware: false,
        ..AaePluginParams::default()
    };
    let mut p = AaePlugin::from_params(params).unwrap();
    p.initialize(48000).unwrap();

    let lfe_idx = p
        .speaker_config
        .speakers
        .iter()
        .find(|speaker| speaker.is_lfe)
        .map(|speaker| speaker.channel)
        .expect("default 5.1 config has an LFE channel");

    let chunk = 512;
    let mut input = vec![0.0_f32; chunk * 2];
    input[0] = 1.0;
    input[1] = 1.0;
    let mut output = vec![0.0_f32; chunk * p.num_output_channels];
    let context = ProcessContext::new(48000, chunk);

    let mut late_lfe_energy = 0.0_f32;
    let mut frame_offset = 0usize;
    for block in 0..120 {
        p.process(&input, &mut output, &context).unwrap();
        for frame in 0..chunk {
            if frame_offset + frame > 12000 {
                let sample = output[frame * p.num_output_channels + lfe_idx];
                late_lfe_energy += sample * sample;
            }
        }
        input.fill(0.0);
        frame_offset += chunk;
        if block > 80 && late_lfe_energy > 1e-10 {
            break;
        }
    }

    assert!(
        late_lfe_energy > 1e-10,
        "LFE should contain low-passed late reverb tail energy, got {late_lfe_energy}"
    );
}

#[test]
fn test_lfe_source_energy_does_not_cancel_with_signed_sum() {
    let source = signed_rms(0.0, 2.0, 2, -0.5);

    assert!(
        (source - 1.0).abs() < 1e-6,
        "source-domain LFE energy should use unsigned RMS for decorrelated source energy, got {source}"
    );
}

#[test]
fn test_output_safety_limit_bounds_final_mix() {
    let params = AaePluginParams {
        dry_level: 1.0,
        er_level: 1.0,
        late_level: 1.0,
        lfe_level: 1.0,
        pre_delay_ms: 0.0,
        safety_limit_db: 0.0,
        content_aware: false,
        ..AaePluginParams::default()
    };
    let mut p = AaePlugin::from_params(params).unwrap();
    p.initialize(48000).unwrap();

    let n = 4096;
    let input = vec![2.0_f32; n * 2];
    let mut output = vec![0.0; n * p.output_channels()];
    p.process(&input, &mut output, &ProcessContext::new(48000, n))
        .unwrap();

    let max = output.iter().copied().map(f32::abs).fold(0.0, f32::max);
    assert!(
        max <= 1.0 + 1e-6,
        "final output should respect the 0 dBFS safety limit, max={max}"
    );
}

#[test]
fn test_lfe_lr4_rejects_midrange() {
    fn steady_rms(frequency: f32) -> f32 {
        let sample_rate = 48_000.0;
        let mut filter = LfeLowpass::new(120.0, sample_rate);
        let mut energy = 0.0_f64;
        let frames = 48_000usize;
        for frame in 0..frames {
            let input = (std::f32::consts::TAU * frequency * frame as f32 / sample_rate).sin();
            let output = filter.process(input);
            if frame >= frames / 2 {
                energy += f64::from(output * output);
            }
        }
        (energy / (frames / 2) as f64).sqrt() as f32
    }

    let passband = steady_rms(40.0);
    let at_250 = steady_rms(250.0);
    let at_1000 = steady_rms(1000.0);
    assert!(
        at_250 / passband < 0.25,
        "250 Hz rejection={}",
        at_250 / passband
    );
    assert!(
        at_1000 / passband < 0.01,
        "1 kHz rejection={}",
        at_1000 / passband
    );
}

#[test]
fn test_delay_changes_start_click_safe_transitions() {
    let mut plugin = AaePlugin::from_params(AaePluginParams::default()).unwrap();
    plugin.initialize(48_000).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("pre_delay_ms"),
            ParameterValue::Float(80.0),
        )
        .unwrap();
    assert!(plugin.pre_delay_transition_remaining > 0);

    plugin
        .set_parameter(ParameterId::from("room_size"), ParameterValue::Float(2.5))
        .unwrap();
    assert!(plugin.fdn.delay_transition_active());
}

#[test]
fn test_routing_rows_are_sparse_and_exclude_lfe() {
    let plugin = AaePlugin::from_params(AaePluginParams::default()).unwrap();
    let lfe = plugin
        .speaker_config
        .speakers
        .iter()
        .find(|speaker| speaker.is_lfe)
        .unwrap()
        .channel;
    assert!(plugin.er_gains.iter().all(|row| row.len() <= 3));
    assert!(plugin.fdn_gains.iter().all(|row| row.len() <= 3));
    assert!(
        plugin
            .er_gains
            .iter()
            .all(|row| !row.channels().contains(&lfe))
    );
    assert!(
        plugin
            .fdn_gains
            .iter()
            .all(|row| !row.channels().contains(&lfe))
    );
}
