#![allow(clippy::needless_range_loop)]
use super::misc::time_to_coeff;
use super::transient_shaper_plugin::TransientShaperPlugin;
use super::types::TransientShaperPluginParams;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;

mod misc;

#[test]
fn test_parameter_roundtrip() {
    let channels = 2;
    let mut plugin = TransientShaperPlugin::new(channels);
    plugin.initialize(48000).unwrap();

    // Set attack to 50%
    plugin
        .parametric_set_parameter(ParameterId::from("attack"), ParameterValue::Float(50.0))
        .unwrap();
    let val = plugin.parametric_get_parameter(&ParameterId::from("attack"));
    assert_eq!(val, Some(ParameterValue::Float(50.0)));

    // Set sustain to -75%
    plugin
        .parametric_set_parameter(ParameterId::from("sustain"), ParameterValue::Float(-75.0))
        .unwrap();
    let val = plugin.parametric_get_parameter(&ParameterId::from("sustain"));
    assert_eq!(val, Some(ParameterValue::Float(-75.0)));

    // Set sensitivity
    plugin
        .parametric_set_parameter(ParameterId::from("sensitivity"), ParameterValue::Float(6.0))
        .unwrap();
    let val = plugin.parametric_get_parameter(&ParameterId::from("sensitivity"));
    assert_eq!(val, Some(ParameterValue::Float(6.0)));

    // Set output gain
    plugin
        .parametric_set_parameter(
            ParameterId::from("output_gain"),
            ParameterValue::Float(-3.0),
        )
        .unwrap();
    let val = plugin.parametric_get_parameter(&ParameterId::from("output_gain"));
    assert_eq!(val, Some(ParameterValue::Float(-3.0)));

    // Set mix
    plugin
        .parametric_set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
        .unwrap();
    let val = plugin.parametric_get_parameter(&ParameterId::from("mix"));
    assert_eq!(val, Some(ParameterValue::Float(0.5)));
}

#[test]
fn test_time_to_coeff_handles_bad_inputs() {
    assert_eq!(time_to_coeff(0.0, 48000), 1.0);
    assert_eq!(time_to_coeff(-1.0, 48000), 1.0);
    assert_eq!(time_to_coeff(10.0, 0), 1.0);
    assert!(time_to_coeff(10.0, 48000).is_finite());
}

#[test]
fn test_fallible_constructor_and_buffer_validation() {
    assert!(TransientShaperPlugin::from_params(0, TransientShaperPluginParams::default()).is_err());
    let invalid = TransientShaperPluginParams {
        attack: f32::NAN,
        ..Default::default()
    };
    assert!(TransientShaperPlugin::from_params(1, invalid).is_err());

    let mut plugin = TransientShaperPlugin::new(2);
    plugin.initialize(48_000).unwrap();
    let mut short = vec![0.0; 7];
    assert!(
        plugin
            .process_in_place(&mut short, &ProcessContext::new(48_000, 4))
            .is_err()
    );
}

#[test]
fn test_attack_component_is_positive_only() {
    assert_eq!(TransientShaperPlugin::attack_component(0.2, 0.5), 0.0);
    assert!((TransientShaperPlugin::attack_component(0.8, 0.5) - 0.3).abs() < 1e-6);
}

#[test]
fn sensitivity_and_output_gain_automation_are_smoothed() {
    let mut plugin = TransientShaperPlugin::new(1);
    plugin.initialize(48_000).unwrap();
    plugin
        .parametric_set_parameter(
            ParameterId::from("output_gain"),
            ParameterValue::Float(12.0),
        )
        .unwrap();
    plugin
        .parametric_set_parameter(
            ParameterId::from("sensitivity"),
            ParameterValue::Float(12.0),
        )
        .unwrap();

    assert!(plugin.output_gain_smoother.current() < plugin.output_gain_smoother.target());
    assert!(plugin.sensitivity_smoother.current() < plugin.sensitivity_smoother.target());

    let mut sample = vec![0.25];
    plugin
        .process_in_place(&mut sample, &ProcessContext::new(48_000, 1))
        .unwrap();
    assert!(
        sample[0] > 0.25 && sample[0] < 0.252,
        "linear output gain must begin its 10 ms transition without jumping to the target: {}",
        sample[0]
    );
}

#[test]
fn asymmetric_stereo_transient_uses_linked_gain() {
    let params = TransientShaperPluginParams {
        attack: 100.0,
        sustain: 0.0,
        sensitivity_db: -12.0,
        output_gain_db: 0.0,
        mix: 1.0,
    };
    let mut plugin = TransientShaperPlugin::from_validated_params(2, params);
    plugin.initialize(48_000).unwrap();
    plugin.reset();
    let mut buffer = vec![0.8, 0.2];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(48_000, 1))
        .unwrap();
    let left_gain = buffer[0] / 0.8;
    let right_gain = buffer[1] / 0.2;
    assert!((left_gain - right_gain).abs() < 1e-6);
}

#[test]
fn extreme_shaping_has_bounded_output() {
    let params = TransientShaperPluginParams {
        attack: 100.0,
        sustain: 100.0,
        sensitivity_db: -12.0,
        output_gain_db: 12.0,
        mix: 1.0,
    };
    let mut plugin = TransientShaperPlugin::from_validated_params(2, params);
    plugin.initialize(48_000).unwrap();
    let mut buffer = vec![1.0; 48_000 * 2];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(48_000, 48_000))
        .unwrap();
    assert!(
        buffer
            .iter()
            .all(|sample| sample.is_finite() && sample.abs() < 2.0)
    );
}

#[test]
fn neutral_controls_preserve_overrange_input() {
    let mut plugin = TransientShaperPlugin::new(2);
    plugin.initialize(48_000).unwrap();
    let mut buffer = vec![1.25, -1.5];
    let expected = buffer.clone();
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(48_000, 1))
        .unwrap();
    assert_eq!(buffer, expected);
}

#[test]
fn monitoring_is_sample_cadenced_and_partition_invariant() {
    fn render(block: usize) -> (u64, f32) {
        let mut plugin = TransientShaperPlugin::from_validated_params(
            1,
            TransientShaperPluginParams {
                attack: -100.0,
                sustain: 0.0,
                sensitivity_db: -12.0,
                output_gain_db: 0.0,
                mix: 1.0,
            },
        );
        plugin.initialize(48_000).unwrap();
        let total = 3_200;
        let mut offset = 0;
        while offset < total {
            let frames = block.min(total - offset);
            let mut samples = vec![0.8; frames];
            plugin
                .process_in_place(&mut samples, &ProcessContext::new(48_000, frames))
                .unwrap();
            offset += frames;
        }
        let (_, updates) = plugin.cache.take_contention_stats();
        (updates, plugin.cache.load().gain)
    }
    let a = render(64);
    let b = render(512);
    assert_eq!(a.0, 2);
    assert_eq!(a.0, b.0);
    assert!((a.1 - b.1).abs() < 1e-6);
}

#[test]
fn attenuation_only_window_is_reported_by_gain_meter() {
    let mut plugin = TransientShaperPlugin::from_validated_params(
        1,
        TransientShaperPluginParams {
            attack: 0.0,
            sustain: -100.0,
            sensitivity_db: -12.0,
            output_gain_db: 0.0,
            mix: 1.0,
        },
    );
    plugin.initialize(48_000).unwrap();
    let mut buffer = vec![0.8; 1_600];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(48_000, 1_600))
        .unwrap();
    assert!(plugin.cache.load().gain < 1.0);
}

#[test]
fn sensitivity_and_output_automation_are_partition_invariant() {
    fn render(block: usize) -> Vec<f32> {
        let mut plugin = TransientShaperPlugin::new(1);
        plugin.initialize(48_000).unwrap();
        plugin
            .parametric_set_parameter(
                ParameterId::from("sensitivity"),
                ParameterValue::Float(12.0),
            )
            .unwrap();
        plugin
            .parametric_set_parameter(ParameterId::from("output_gain"), ParameterValue::Float(6.0))
            .unwrap();
        let mut output = Vec::new();
        for offset in (0..960).step_by(block) {
            let frames = block.min(960 - offset);
            let mut samples = vec![0.25; frames];
            plugin
                .process_in_place(&mut samples, &ProcessContext::new(48_000, frames))
                .unwrap();
            output.extend(samples);
        }
        output
    }
    assert_eq!(render(32), render(240));
}

#[test]
fn process_rejects_oversized_buffer_without_state_change() {
    let mut plugin = TransientShaperPlugin::new(2);
    plugin.initialize(48_000).unwrap();
    let fast_before = plugin.fast_env.clone();
    let mut buffer = vec![0.5; 9];
    assert!(
        plugin
            .process_in_place(&mut buffer, &ProcessContext::new(48_000, 4))
            .is_err()
    );
    assert_eq!(plugin.fast_env, fast_before);
}

#[test]
fn parameter_updates_reuse_cached_schema_storage() {
    let mut plugin = TransientShaperPlugin::new(2);
    let ptr = plugin.cached_parameters.as_ptr();
    let capacity = plugin.cached_parameters.capacity();
    for value in [-100.0, 0.0, 100.0] {
        plugin
            .parametric_set_parameter(ParameterId::from("attack"), ParameterValue::Float(value))
            .unwrap();
        assert_eq!(plugin.cached_parameters.as_ptr(), ptr);
        assert_eq!(plugin.cached_parameters.capacity(), capacity);
    }
}

// -------------------------------------------------------------------------
// Process smoke tests
// -------------------------------------------------------------------------

#[test]
fn test_process_silence_is_silent() {
    let mut plugin = TransientShaperPlugin::new(1);
    plugin.initialize(48000).unwrap();

    let mut buf = vec![0.0f32; 512];
    let ctx = ProcessContext::new(48000, 512);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    for &s in &buf {
        assert_eq!(s, 0.0);
    }
}

#[test]
fn test_process_stereo_passthrough_when_bypassed() {
    let sr = 48000u32;
    let mut plugin = TransientShaperPlugin::from_validated_params(
        2,
        TransientShaperPluginParams {
            attack: 0.0,
            sustain: 0.0,
            sensitivity_db: 0.0,
            output_gain_db: 0.0,
            mix: 0.0,
        },
    );
    plugin.initialize(sr).unwrap();

    let mut buf = vec![0.0f32; 256 * 2];
    for frame in 0..256 {
        buf[frame * 2] = 0.3;
        buf[frame * 2 + 1] = -0.3;
    }
    let ctx = ProcessContext::new(sr, 256);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    for frame in 0..256 {
        assert!((buf[frame * 2] - 0.3).abs() < 1e-5);
        assert!((buf[frame * 2 + 1] - (-0.3)).abs() < 1e-5);
    }
}

#[test]
fn test_attack_boost_increases_transients() {
    let sr = 48000u32;
    let mut plugin = TransientShaperPlugin::from_validated_params(
        1,
        TransientShaperPluginParams {
            attack: 100.0, // +100% attack boost
            sustain: 0.0,
            sensitivity_db: -12.0, // low threshold so signal is detected
            output_gain_db: 0.0,
            mix: 1.0,
        },
    );
    plugin.initialize(sr).unwrap();

    // Impulse-like signal: quiet then loud then quiet
    let mut buf = vec![0.0f32; sr as usize];
    for i in 0..sr as usize {
        if (1000..1100).contains(&i) {
            buf[i] = 0.5;
        } else if (1100..1200).contains(&i) {
            buf[i] = 0.1;
        }
    }

    let ctx = ProcessContext::new(sr, sr as usize);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    // Transient portion should be boosted above original 0.5
    let transient_peak = buf[1000..1100]
        .iter()
        .map(|x| x.abs())
        .fold(0.0f32, f32::max);
    assert!(
        transient_peak > 0.55,
        "attack boost should raise transient peak above 0.5, got {}",
        transient_peak
    );
}

#[test]
fn test_sustain_reduction_lowers_tail() {
    let sr = 48000u32;
    let mut plugin = TransientShaperPlugin::from_validated_params(
        1,
        TransientShaperPluginParams {
            attack: 0.0,
            sustain: -100.0, // full sustain reduction
            sensitivity_db: -12.0,
            output_gain_db: 0.0,
            mix: 1.0,
        },
    );
    plugin.initialize(sr).unwrap();

    // Constant loud signal - the sustain portion should be reduced
    let mut buf = vec![0.5f32; sr as usize / 10]; // 100ms
    let ctx = ProcessContext::new(sr, buf.len());
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    // After envelope settles, the sustained portion should be attenuated
    let tail_avg: f32 = buf[buf.len() - 480..].iter().sum::<f32>() / 480.0;
    assert!(
        tail_avg < 0.4,
        "sustain reduction should lower sustained signal, got avg {}",
        tail_avg
    );
}

#[test]
fn test_output_gain_applies_makeup() {
    let sr = 48000u32;
    let mut plugin = TransientShaperPlugin::from_validated_params(
        1,
        TransientShaperPluginParams {
            attack: 0.0,
            sustain: 0.0,
            sensitivity_db: 0.0,
            output_gain_db: 6.0,
            mix: 0.0, // fully dry - output gain still applies to mixed output
        },
    );
    plugin.initialize(sr).unwrap();

    let mut buf = vec![0.25f32; 256];
    let ctx = ProcessContext::new(sr, 256);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    let expected = 0.25 * 10.0f32.powf(6.0 / 20.0);
    assert!((buf[200] - expected).abs() < 0.001);
}

#[test]
fn test_sensitivity_gates_quiet_signals() {
    let sr = 48000u32;
    // High sensitivity threshold: only loud signals shape
    let mut plugin = TransientShaperPlugin::from_validated_params(
        1,
        TransientShaperPluginParams {
            attack: 100.0,
            sustain: 0.0,
            sensitivity_db: 12.0, // high threshold
            output_gain_db: 0.0,
            mix: 1.0,
        },
    );
    plugin.initialize(sr).unwrap();

    // Quiet signal should pass unshaped because it's below threshold
    let mut buf = vec![0.001f32; 256];
    let ctx = ProcessContext::new(sr, 256);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    // With sensitivity=12dB, threshold_lin = 10^(12/20)*1e-3 ≈ 0.00398
    // 0.001 is below threshold so gain should be 1.0
    assert!((buf[200] - 0.001).abs() < 1e-5);
}

#[test]
fn test_reset_clears_envelope_state() {
    let sr = 48000u32;
    let mut plugin = TransientShaperPlugin::from_validated_params(
        1,
        TransientShaperPluginParams {
            attack: 100.0,
            sustain: 0.0,
            sensitivity_db: -12.0,
            output_gain_db: 0.0,
            mix: 1.0,
        },
    );
    plugin.initialize(sr).unwrap();

    let mut buf = vec![0.5f32; 256];
    let ctx = ProcessContext::new(sr, 256);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    // Envelopes should be non-zero after processing
    assert!(plugin.fast_env[0] > 0.01);
    assert!(plugin.slow_env[0] > 0.01);

    plugin.reset();

    assert_eq!(plugin.fast_env[0], 0.0);
    assert_eq!(plugin.slow_env[0], 0.0);
}

#[test]
fn test_process_empty_buffer() {
    let mut plugin = TransientShaperPlugin::new(2);
    plugin.initialize(48000).unwrap();

    let mut buf = vec![0.0f32; 0];
    let ctx = ProcessContext::new(48000, 0);
    let frames = plugin.process_in_place(&mut buf, &ctx).unwrap();
    assert_eq!(frames, 0);
}

// -------------------------------------------------------------------------
// set_parameter smoke tests
// -------------------------------------------------------------------------

#[test]
fn test_set_parameter_out_of_bounds_returns_error() {
    let mut plugin = TransientShaperPlugin::new(1);
    plugin.initialize(48000).unwrap();

    // Attack is bounded to [-100, 100] by ParamSpec validation
    assert!(
        plugin
            .set_parameter(ParameterId::from("attack"), ParameterValue::Float(200.0))
            .is_err()
    );
    assert!(
        plugin
            .set_parameter(ParameterId::from("attack"), ParameterValue::Float(-200.0))
            .is_err()
    );

    // Sensitivity is bounded to [-12, 12]
    assert!(
        plugin
            .set_parameter(
                ParameterId::from("sensitivity"),
                ParameterValue::Float(50.0),
            )
            .is_err()
    );
    assert!(
        plugin
            .set_parameter(
                ParameterId::from("sensitivity"),
                ParameterValue::Float(-50.0),
            )
            .is_err()
    );

    // Mix is bounded to [0, 1]
    assert!(
        plugin
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(2.0))
            .is_err()
    );
    assert!(
        plugin
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(-1.0))
            .is_err()
    );
}

#[test]
fn from_params_rejects_out_of_range_values_instead_of_clamping() {
    let result = TransientShaperPlugin::from_params(
        1,
        TransientShaperPluginParams {
            attack: 200.0,
            sustain: -200.0,
            sensitivity_db: 50.0,
            output_gain_db: -50.0,
            mix: 2.0,
        },
    );
    assert!(result.is_err());
}

#[test]
fn test_set_parameter_nan_returns_error() {
    let mut plugin = TransientShaperPlugin::new(1);
    plugin.initialize(48000).unwrap();

    let original = plugin.attack_amount;
    assert!(
        plugin
            .set_parameter(ParameterId::from("attack"), ParameterValue::Float(f32::NAN))
            .is_err()
    );
    assert_eq!(plugin.attack_amount, original);

    let original = plugin.sensitivity_db;
    assert!(
        plugin
            .set_parameter(
                ParameterId::from("sensitivity"),
                ParameterValue::Float(f32::NAN),
            )
            .is_err()
    );
    assert_eq!(plugin.sensitivity_db, original);
}

#[test]
fn test_set_parameter_unknown_id_returns_error() {
    let mut plugin = TransientShaperPlugin::new(1);
    plugin.initialize(48000).unwrap();

    let result = plugin.parametric_set_parameter(
        ParameterId::from("not_a_real_param"),
        ParameterValue::Float(1.0),
    );
    assert!(result.is_err());
}

#[test]
fn test_get_parameter_unknown_id_returns_none() {
    let plugin = TransientShaperPlugin::new(1);
    let val = plugin.parametric_get_parameter(&ParameterId::from("not_a_real_param"));
    assert_eq!(val, None);
}

#[test]
fn test_from_params_wiring() {
    let plugin = TransientShaperPlugin::from_validated_params(
        2,
        TransientShaperPluginParams {
            attack: 75.0,
            sustain: -50.0,
            sensitivity_db: 6.0,
            output_gain_db: -6.0,
            mix: 0.5,
        },
    );
    assert_eq!(plugin.channels, 2);
    assert!((plugin.attack_amount - 0.75).abs() < 1e-6);
    assert!((plugin.sustain_amount - (-0.5)).abs() < 1e-6);
    assert!((plugin.sensitivity_db - 6.0).abs() < 1e-6);
    assert!((plugin.output_gain_db - (-6.0)).abs() < 1e-6);
    assert!((plugin.mix - 0.5).abs() < 1e-6);
}

#[test]
fn test_info_and_channels() {
    let plugin = TransientShaperPlugin::new(4);
    assert_eq!(plugin.channels(), 4);
    let info = plugin.info();
    assert_eq!(info.name, "TransientShaper");
}
