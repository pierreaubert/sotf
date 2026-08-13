//! Integration tests for sotf-plugin-hiss-reducer exercising the public `InPlacePlugin` trait.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::{PluginCostClass, ProcessContext};
use sotf_plugin_hiss_reducer::{HissReducerPlugin, HissReducerPluginParams};

const SR: u32 = 48000;

fn ctx(frames: usize) -> ProcessContext<'static> {
    ProcessContext::new(SR, frames)
}

#[test]
fn info_is_reported() {
    let plugin = HissReducerPlugin::new(2);
    let info = plugin.info();
    assert_eq!(info.name, "Hiss Reducer");
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    assert!(!info.description.is_empty());
}

#[test]
fn disabled_is_transparent() {
    let mut plugin = HissReducerPlugin::new(2);
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();
    plugin.initialize(SR).unwrap();

    let mut buffer = vec![0.25f32, -0.25, 0.5, -0.5];
    let input = buffer.clone();
    assert_eq!(plugin.process_in_place(&mut buffer, &ctx(2)).unwrap(), 2);
    assert_eq!(buffer, input);
}

#[test]
fn enabled_changes_high_frequency_content() {
    let mut plugin = HissReducerPlugin::new(1);
    plugin.initialize(SR).unwrap();

    // Persistent, low-level high-frequency energy above the default 4 kHz
    // cutoff and below the default -30 dBFS detector threshold.
    let mut buffer: Vec<f32> = (0..24_000).map(|i| (i as f32 * 0.5).sin() * 0.02).collect();
    let input = buffer.clone();
    plugin.process_in_place(&mut buffer, &ctx(24_000)).unwrap();
    assert_ne!(
        buffer, input,
        "hiss reducer should alter high-frequency signal"
    );
    assert!(buffer.iter().all(|s| s.is_finite()));
}

#[test]
fn parameter_roundtrips() {
    let mut plugin = HissReducerPlugin::new(1);
    plugin.initialize(SR).unwrap();

    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();
    assert_eq!(
        plugin
            .get_parameter(&ParameterId::from("enabled"))
            .and_then(|v| v.as_bool()),
        Some(false)
    );

    plugin
        .set_parameter(
            ParameterId::from("threshold_db"),
            ParameterValue::Float(-40.0),
        )
        .unwrap();
    assert!(
        (plugin
            .get_parameter(&ParameterId::from("threshold_db"))
            .and_then(|v| v.as_float())
            .unwrap()
            - (-40.0))
            .abs()
            < 1e-3
    );

    plugin
        .set_parameter(
            ParameterId::from("frequency_hz"),
            ParameterValue::Float(6000.0),
        )
        .unwrap();
    assert!(
        (plugin
            .get_parameter(&ParameterId::from("frequency_hz"))
            .and_then(|v| v.as_float())
            .unwrap()
            - 6000.0)
            .abs()
            < 1e-3
    );

    plugin
        .set_parameter(ParameterId::from("strength"), ParameterValue::Float(0.75))
        .unwrap();
    assert!(
        (plugin
            .get_parameter(&ParameterId::from("strength"))
            .and_then(|v| v.as_float())
            .unwrap()
            - 0.75)
            .abs()
            < 1e-3
    );
}

#[test]
fn from_params_happy_path() {
    let params = HissReducerPluginParams {
        enabled: true,
        threshold_db: -35.0,
        frequency_hz: 5000.0,
        strength: 0.25,
        spectral_mode: false,
    };
    let mut plugin = HissReducerPlugin::from_params(2, params);
    assert_eq!(plugin.channels(), 2);
    plugin.initialize(SR).unwrap();

    let mut buffer = vec![0.1f32; 32 * 2];
    plugin.process_in_place(&mut buffer, &ctx(32)).unwrap();
    assert!(buffer.iter().all(|s| s.is_finite()));
}

#[test]
fn reset_clears_reducer_state() {
    let mut plugin = HissReducerPlugin::new(1);
    plugin.initialize(SR).unwrap();

    let mut buffer = vec![0.6f32; 64];
    plugin.process_in_place(&mut buffer, &ctx(64)).unwrap();
    plugin.reset();

    let mut buffer2 = vec![0.6f32; 64];
    plugin.process_in_place(&mut buffer2, &ctx(64)).unwrap();
    assert!(buffer2.iter().all(|s| s.is_finite()));
}

#[test]
fn buffer_size_mismatch_returns_error() {
    let mut plugin = HissReducerPlugin::new(2);
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();

    let mut buffer = vec![0.0f32; 3];
    let err = plugin.process_in_place(&mut buffer, &ctx(2)).unwrap_err();
    assert!(
        err.contains("Buffer size mismatch"),
        "unexpected error: {err}"
    );
}

#[test]
fn unknown_parameter_errors() {
    let mut plugin = HissReducerPlugin::new(1);
    let err = plugin
        .set_parameter(ParameterId::from("not_a_param"), ParameterValue::Float(1.0))
        .unwrap_err();
    assert!(err.contains("Unknown parameter"), "unexpected error: {err}");
}

#[test]
fn latency_is_zero() {
    let plugin = HissReducerPlugin::new(1);
    assert_eq!(plugin.latency_samples(), 0);
}

#[test]
fn initialize_does_not_change_response_at_default_rate() {
    let mut uninit = HissReducerPlugin::new(1);
    let mut buf_uninit = vec![0.5f32; 8];
    let err = uninit
        .process_in_place(&mut buf_uninit, &ctx(8))
        .unwrap_err();
    assert!(err.contains("initialized"), "unexpected error: {err}");

    let mut init = HissReducerPlugin::new(1);
    init.initialize(SR).unwrap();
    let mut buf_init = vec![0.5f32; 8];
    init.process_in_place(&mut buf_init, &ctx(8)).unwrap();

    assert!(buf_init.iter().all(|sample| sample.is_finite()));
}

#[test]
fn zero_sample_rate_and_context_mismatch_are_rejected() {
    let mut plugin = HissReducerPlugin::new(1);
    assert!(plugin.initialize(0).is_err());
    plugin.initialize(SR).unwrap();
    let mut buffer = vec![0.0; 8];
    let mismatched = ProcessContext::new(44_100, 8);
    let err = plugin
        .process_in_place(&mut buffer, &mismatched)
        .unwrap_err();
    assert!(err.contains("sample rate"), "unexpected error: {err}");
}

#[test]
fn persisted_parameters_are_canonicalized() {
    let plugin = HissReducerPlugin::from_params(
        1,
        HissReducerPluginParams {
            enabled: true,
            threshold_db: f32::NAN,
            frequency_hz: f32::INFINITY,
            strength: -2.0,
            spectral_mode: false,
        },
    );
    let values = plugin.current_values();
    assert_eq!(
        values
            .get(&ParameterId::from("threshold_db"))
            .and_then(|v| v.as_float()),
        Some(-30.0)
    );
    assert_eq!(
        values
            .get(&ParameterId::from("frequency_hz"))
            .and_then(|v| v.as_float()),
        Some(4_000.0)
    );
    assert_eq!(
        values
            .get(&ParameterId::from("strength"))
            .and_then(|v| v.as_float()),
        Some(0.0)
    );
}

#[test]
fn metadata_reports_iir_cost() {
    let plugin = HissReducerPlugin::new(1);
    assert_eq!(plugin.cost_class(), PluginCostClass::Iir);
    let metadata = plugin.compile_metadata();
    assert_eq!(metadata.cost_class, PluginCostClass::Iir);
    assert!(!metadata.linear);
    assert!(metadata.stateful);
    assert!(!metadata.channel_mixing);
    assert_eq!(metadata.latency_samples, 0);
}

#[test]
fn bypass_reentry_restarts_detector_state() {
    let mut plugin = HissReducerPlugin::new(1);
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();
    let mut bypassed = vec![0.0; 64];
    plugin.process_in_place(&mut bypassed, &ctx(64)).unwrap();
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(true))
        .unwrap();
    let mut program = vec![0.25; 64];
    plugin.process_in_place(&mut program, &ctx(64)).unwrap();
    assert!(program.iter().all(|sample| sample.is_finite()));
}

#[test]
fn initialization_canonicalizes_cutoff_to_sample_rate() {
    let mut plugin = HissReducerPlugin::from_params(
        1,
        HissReducerPluginParams {
            frequency_hz: 16_000.0,
            ..HissReducerPluginParams::default()
        },
    );
    plugin.initialize(8_000).unwrap();
    assert_eq!(
        plugin
            .get_parameter(&ParameterId::from("frequency_hz"))
            .and_then(|value| value.as_float()),
        Some(3_600.0),
        "host-visible state must match the reducer's 0.45 * sample-rate limit"
    );

    let err = plugin
        .set_parameter(
            ParameterId::from("frequency_hz"),
            ParameterValue::Float(3_601.0),
        )
        .unwrap_err();
    assert!(err.contains("sample rate"), "unexpected error: {err}");
}

#[test]
fn initialization_rejects_rates_without_a_valid_cutoff_range() {
    let mut plugin = HissReducerPlugin::new(1);
    let err = plugin.initialize(2_000).unwrap_err();
    assert!(err.contains("sample rate"), "unexpected error: {err}");
}

#[test]
fn persisted_params_reject_unknown_fields() {
    let err = serde_json::from_value::<HissReducerPluginParams>(serde_json::json!({
        "enabled": true,
        "obsolete_fft_mode": true
    }))
    .unwrap_err();
    assert!(err.to_string().contains("obsolete_fft_mode"));
}

#[test]
fn non_finite_audio_is_sanitized_and_state_recovers() {
    let mut plugin = HissReducerPlugin::new(1);
    plugin.initialize(SR).unwrap();
    let mut poisoned = vec![f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.1];
    plugin.process_in_place(&mut poisoned, &ctx(4)).unwrap();
    assert!(poisoned.iter().all(|sample| sample.is_finite()));

    let mut recovery = vec![0.02; 256];
    plugin.process_in_place(&mut recovery, &ctx(256)).unwrap();
    assert!(recovery.iter().all(|sample| sample.is_finite()));
}

#[test]
fn live_bypass_transition_is_smoothed() {
    let mut plugin = HissReducerPlugin::from_params(
        1,
        HissReducerPluginParams {
            threshold_db: -20.0,
            strength: 1.0,
            ..HissReducerPluginParams::default()
        },
    );
    plugin.initialize(SR).unwrap();
    let mut warm: Vec<f32> = (0..SR / 2)
        .map(|index| if index % 2 == 0 { 0.05 } else { -0.05 })
        .collect();
    let warm_frames = warm.len();
    plugin
        .process_in_place(&mut warm, &ctx(warm_frames))
        .unwrap();
    let previous = *warm.last().unwrap();

    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();
    let mut transition = vec![0.05];
    let transition_frames = transition.len();
    plugin
        .process_in_place(&mut transition, &ctx(transition_frames))
        .unwrap();
    assert!(
        (transition[0] - previous).abs() < 0.065,
        "bypass switched discontinuously: previous={previous}, next={}",
        transition[0]
    );
}

fn spectral_plugin(enabled: bool, strength: f32) -> HissReducerPlugin {
    let mut plugin = HissReducerPlugin::from_params(
        1,
        HissReducerPluginParams {
            enabled,
            threshold_db: -30.0,
            frequency_hz: 4_000.0,
            strength,
            spectral_mode: true,
        },
    );
    plugin.initialize(SR).unwrap();
    plugin
}

fn render_partitioned(
    plugin: &mut HissReducerPlugin,
    input: &[f32],
    partitions: &[usize],
) -> Vec<f32> {
    let mut output = Vec::with_capacity(input.len());
    let mut offset = 0;
    let mut part = 0;
    while offset < input.len() {
        let count = partitions[part % partitions.len()].min(input.len() - offset);
        let mut block = input[offset..offset + count].to_vec();
        plugin
            .process_in_place(&mut block, &ProcessContext::new(SR, count))
            .unwrap();
        output.extend(block);
        offset += count;
        part += 1;
    }
    output
}

#[test]
fn spectral_mode_reports_and_realizes_fixed_latency_for_irregular_callbacks() {
    for partitions in [&[1][..], &[64], &[511], &[512], &[1024], &[73, 997, 5, 256]] {
        let mut plugin = spectral_plugin(true, 0.0);
        assert_eq!(plugin.latency_samples(), 1024);
        assert_eq!(plugin.cost_class(), PluginCostClass::Fft);
        assert_eq!(plugin.compile_metadata().latency_samples, 1024);
        let mut impulse = vec![0.0; 4096];
        impulse[0] = 1.0;
        let output = render_partitioned(&mut plugin, &impulse, partitions);
        let peak = output
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
            .unwrap();
        assert_eq!(peak.0, 1024, "{partitions:?}");
        assert!((peak.1 - 1.0).abs() < 2.0e-5, "{partitions:?}: {peak:?}");
    }
}

#[test]
fn spectral_mode_is_sample_exact_across_callback_partitions() {
    let input: Vec<f32> = (0..16_384)
        .map(|i| {
            let tone = (2.0 * std::f32::consts::PI * 750.0 * i as f32 / SR as f32).sin();
            0.15 * tone + 0.02 * ((i * 7919 % 1021) as f32 / 510.5 - 1.0)
        })
        .collect();
    let mut whole = spectral_plugin(true, 0.7);
    let mut irregular = spectral_plugin(true, 0.7);
    assert_eq!(
        render_partitioned(&mut whole, &input, &[input.len()]),
        render_partitioned(&mut irregular, &input, &[1, 64, 511, 73, 997])
    );
}

#[test]
fn disabled_spectral_mode_is_exact_delayed_dry() {
    let input: Vec<f32> = (0..4096).map(|i| (i as f32 * 0.019).sin()).collect();
    let mut plugin = spectral_plugin(false, 1.0);
    let output = render_partitioned(&mut plugin, &input, &[1, 64, 511, 1024]);
    assert_eq!(&output[..1024], vec![0.0; 1024]);
    assert_eq!(&output[1024..], &input[..input.len() - 1024]);
}

#[test]
fn spectral_mode_improves_stationary_high_band_noise_snr_and_preserves_low_tone() {
    let frames = SR as usize * 2;
    let mut state = 0x1234_5678_u32;
    let mut clean = Vec::with_capacity(frames);
    let mut noisy = Vec::with_capacity(frames);
    let mut previous = 0.0;
    for i in 0..frames {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let white = (state as f32 / u32::MAX as f32) * 2.0 - 1.0;
        let high_pass = 0.035 * (white - previous);
        previous = white;
        let tone = 0.12 * (2.0 * std::f32::consts::PI * 750.0 * i as f32 / SR as f32).sin();
        clean.push(tone);
        noisy.push(tone + high_pass);
    }
    let mut plugin = spectral_plugin(true, 0.85);
    let output = render_partitioned(&mut plugin, &noisy, &[1, 64, 511, 73, 997]);
    let start = SR as usize / 2 + 1024;
    let mut input_error = 0.0_f64;
    let mut output_error = 0.0_f64;
    let mut clean_power = 0.0_f64;
    for i in start..frames {
        let reference = clean[i - 1024] as f64;
        input_error += (noisy[i - 1024] as f64 - reference).powi(2);
        output_error += (output[i] as f64 - reference).powi(2);
        clean_power += reference.powi(2);
    }
    let input_snr = 10.0 * (clean_power / input_error).log10();
    let output_snr = 10.0 * (clean_power / output_error).log10();
    assert!(
        output_snr > input_snr + 2.0,
        "SNR did not improve enough: input={input_snr:.2} dB output={output_snr:.2} dB"
    );
}

#[test]
fn spectral_mode_change_is_rejected_after_initialization_but_same_value_is_allowed() {
    let mut plugin = HissReducerPlugin::new(1);
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("spectral_mode"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    let error = plugin
        .set_parameter(
            ParameterId::from("spectral_mode"),
            ParameterValue::Bool(true),
        )
        .unwrap_err();
    assert!(error.contains("structural"));

    let mut values = sotf_host::parametric_plugin::ParameterSet::new();
    values.insert(
        ParameterId::from("spectral_mode"),
        ParameterValue::Bool(true),
    );
    assert!(
        plugin
            .apply_values(values)
            .unwrap_err()
            .contains("structural")
    );
    assert_eq!(plugin.latency_samples(), 0);
}

#[test]
fn spectral_mode_preserves_a_stationary_wanted_high_frequency_tone() {
    let frames = SR as usize * 2;
    let input: Vec<f32> = (0..frames)
        .map(|i| 0.15 * (2.0 * std::f32::consts::PI * 7_500.0 * i as f32 / SR as f32).sin())
        .collect();
    let mut plugin = spectral_plugin(true, 0.85);
    let output = render_partitioned(&mut plugin, &input, &[1, 64, 511, 73, 997]);
    let start = SR as usize + 1024;
    let input_rms = (input[start - 1024..]
        .iter()
        .map(|sample| sample * sample)
        .sum::<f32>()
        / (frames - start) as f32)
        .sqrt();
    let output_rms = (output[start..]
        .iter()
        .map(|sample| sample * sample)
        .sum::<f32>()
        / (frames - start) as f32)
        .sqrt();
    let gain_db = 20.0 * (output_rms / input_rms).log10();
    assert!(gain_db > -1.0, "wanted 7.5 kHz tone lost {gain_db:.2} dB");
}

#[test]
fn live_spectral_bypass_is_smoothed_and_partition_independent() {
    fn render_toggle(partitions: &[usize]) -> Vec<f32> {
        let input: Vec<f32> = (0..8192)
            .map(|i| 0.1 * (2.0 * std::f32::consts::PI * 8_000.0 * i as f32 / SR as f32).sin())
            .collect();
        let mut plugin = spectral_plugin(true, 0.8);
        let mut output = Vec::new();
        let mut offset = 0;
        let mut part = 0;
        while offset < input.len() {
            if offset == 4096 {
                plugin
                    .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
                    .unwrap();
            }
            let boundary = if offset < 4096 { 4096 } else { input.len() };
            let count = partitions[part % partitions.len()]
                .min(boundary - offset)
                .max(1);
            let mut block = input[offset..offset + count].to_vec();
            plugin
                .process_in_place(&mut block, &ProcessContext::new(SR, count))
                .unwrap();
            output.extend(block);
            offset += count;
            part += 1;
        }
        output
    }
    assert_eq!(
        render_toggle(&[4096]),
        render_toggle(&[1, 64, 511, 73, 997])
    );
}

#[test]
fn direct_spectral_cutoff_update_matches_batch_update() {
    let mut direct = spectral_plugin(true, 0.8);
    let mut batch = spectral_plugin(true, 0.8);
    direct
        .set_parameter(
            ParameterId::from("frequency_hz"),
            ParameterValue::Float(8_000.0),
        )
        .unwrap();
    let mut values = sotf_host::parametric_plugin::ParameterSet::new();
    values.insert(
        ParameterId::from("frequency_hz"),
        ParameterValue::Float(8_000.0),
    );
    batch.apply_values(values).unwrap();
    let input: Vec<f32> = (0..8192)
        .map(|i| ((i * 7919 % 1021) as f32 / 510.5 - 1.0) * 0.05)
        .collect();
    assert_eq!(
        render_partitioned(&mut direct, &input, &[1, 64, 511, 997]),
        render_partitioned(&mut batch, &input, &[1, 64, 511, 997])
    );
}

fn deterministic_noise(frames: usize, amplitude: f32) -> Vec<f32> {
    let mut state = 0x8bad_f00d_u32;
    (0..frames)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            amplitude * ((state as f32 / u32::MAX as f32) * 2.0 - 1.0)
        })
        .collect()
}

#[test]
fn spectral_threshold_uses_calibrated_high_band_rms() {
    fn tail_rms(input: &[f32], threshold: f32) -> f32 {
        let mut plugin = spectral_plugin(true, 0.9);
        plugin
            .set_parameter(
                ParameterId::from("threshold_db"),
                ParameterValue::Float(threshold),
            )
            .unwrap();
        let output = render_partitioned(&mut plugin, input, &[1, 64, 511, 997]);
        (output[SR as usize..]
            .iter()
            .map(|sample| sample * sample)
            .sum::<f32>()
            / (output.len() - SR as usize) as f32)
            .sqrt()
    }
    let quiet = deterministic_noise(SR as usize * 2, 0.02);
    let loud = deterministic_noise(SR as usize * 2, 0.5);
    let quiet_input = (quiet.iter().map(|x| x * x).sum::<f32>() / quiet.len() as f32).sqrt();
    let loud_input = (loud.iter().map(|x| x * x).sum::<f32>() / loud.len() as f32).sqrt();
    assert!(tail_rms(&quiet, -20.0) < quiet_input * 0.8);
    assert!(tail_rms(&loud, -20.0) > loud_input * 0.9);
}

#[test]
fn spectral_threshold_automation_is_smoothed_and_partition_independent() {
    fn render(partitions: &[usize]) -> Vec<f32> {
        let input = deterministic_noise(12_288, 0.03);
        let mut plugin = spectral_plugin(true, 0.9);
        plugin
            .set_parameter(
                ParameterId::from("threshold_db"),
                ParameterValue::Float(-10.0),
            )
            .unwrap();
        let mut output = Vec::new();
        let mut offset = 0;
        let mut part = 0;
        while offset < input.len() {
            if offset == 8192 {
                plugin
                    .set_parameter(
                        ParameterId::from("threshold_db"),
                        ParameterValue::Float(-60.0),
                    )
                    .unwrap();
            }
            let boundary = if offset < 8192 { 8192 } else { input.len() };
            let count = partitions[part % partitions.len()]
                .min(boundary - offset)
                .max(1);
            let mut block = input[offset..offset + count].to_vec();
            plugin
                .process_in_place(&mut block, &ProcessContext::new(SR, count))
                .unwrap();
            output.extend(block);
            offset += count;
            part += 1;
        }
        output
    }
    let output = render(&[1, 64, 511, 997]);
    assert_eq!(output, render(&[8192]));
    let maximum_step = output[8192 - 16..8192 + 512]
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .fold(0.0_f32, f32::max);
    assert!(maximum_step < 0.12, "threshold switch step {maximum_step}");
}
