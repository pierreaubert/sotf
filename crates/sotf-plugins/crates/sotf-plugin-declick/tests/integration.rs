//! Public contract and DSP integration tests for Declick.

use plugins_denoiser::transient::LOOKAHEAD_SAMPLES;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::ParameterSet;
use sotf_host::plugin::{PluginCostClass, ProcessContext};
use sotf_plugin_declick::{DeclickPlugin, DeclickPluginParams};

const SR: u32 = 48_000;

fn plugin(channels: usize) -> DeclickPlugin {
    DeclickPlugin::new(channels, SR).unwrap()
}

fn ctx(frames: usize) -> ProcessContext<'static> {
    ProcessContext::new(SR, frames)
}

fn process_and_flush(plugin: &mut DeclickPlugin, input: &[f32], channels: usize) -> Vec<f32> {
    let mut stream = input.to_vec();
    stream.extend(std::iter::repeat_n(0.0, LOOKAHEAD_SAMPLES * channels));
    let frames = stream.len() / channels;
    plugin.process_in_place(&mut stream, &ctx(frames)).unwrap();
    stream
}

#[test]
fn info_and_compile_metadata_match_the_dsp_contract() {
    let plugin = plugin(2);
    let info = plugin.info();
    assert_eq!(info.name, "Declick");
    assert_eq!(info.version, "1.1.0");
    assert_eq!(plugin.cost_class(), PluginCostClass::Dynamics);
    assert_eq!(plugin.latency_samples(), LOOKAHEAD_SAMPLES);
    let metadata = plugin.compile_metadata();
    assert_eq!(metadata.cost_class, PluginCostClass::Dynamics);
    assert_eq!(metadata.latency_samples, LOOKAHEAD_SAMPLES);
    assert!(!metadata.linear);
}

#[test]
fn construction_rejects_invalid_topology_and_rate() {
    assert!(DeclickPlugin::new(0, SR).is_err());
    assert!(DeclickPlugin::new(1, 0).is_err());
}

#[test]
fn malformed_persisted_sensitivity_is_canonicalized() {
    for sensitivity in [f32::NAN, f32::INFINITY, -1.0, 101.0] {
        let plugin = DeclickPlugin::from_params(
            1,
            SR,
            DeclickPluginParams {
                enabled: true,
                sensitivity,
                link_channels: true,
            },
        )
        .unwrap();
        let value = plugin
            .get_parameter(&ParameterId::from("sensitivity"))
            .and_then(|value| value.as_float())
            .unwrap();
        assert!(value.is_finite() && (1.0..=100.0).contains(&value));
    }
}

#[test]
fn host_values_roundtrip_without_stale_cache() {
    let mut plugin = plugin(1);
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("sensitivity"),
            ParameterValue::Float(42.0),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("link_channels"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("enabled")),
        Some(ParameterValue::Bool(false))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("sensitivity")),
        Some(ParameterValue::Float(42.0))
    );
    let schema = plugin.parameter_schema();
    assert_eq!(schema[0].default_value, ParameterValue::Bool(false));
    assert_eq!(schema[1].default_value, ParameterValue::Float(42.0));
    assert_eq!(schema[2].default_value, ParameterValue::Bool(false));
}

#[test]
fn parameter_batch_is_atomic_on_error() {
    let mut plugin = plugin(1);
    let mut values = ParameterSet::new();
    values.insert(ParameterId::from("enabled"), ParameterValue::Bool(false));
    values.insert(ParameterId::from("unknown"), ParameterValue::Float(1.0));
    assert!(plugin.apply_values(values).is_err());
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("enabled")),
        Some(ParameterValue::Bool(true))
    );
}

#[test]
fn successful_single_parameter_setter_does_not_allocate() {
    use sotf_host::assert_no_allocs;

    let mut plugin = plugin(1);
    let id = ParameterId::from("sensitivity");
    let value = ParameterValue::Float(25.0);
    assert_no_allocs("Declick set_parameter", || {
        plugin.set_parameter(id, value).unwrap();
    });
}

#[test]
fn disabled_path_is_transparent_after_reported_latency() {
    let mut plugin = DeclickPlugin::from_params(
        1,
        SR,
        DeclickPluginParams {
            enabled: false,
            sensitivity: 10.0,
            link_channels: true,
        },
    )
    .unwrap();
    let input: Vec<f32> = (0..64).map(|i| (i as f32 * 0.17).sin()).collect();
    let output = process_and_flush(&mut plugin, &input, 1);
    assert!(output[..LOOKAHEAD_SAMPLES].iter().all(|&x| x == 0.0));
    assert_eq!(
        &output[LOOKAHEAD_SAMPLES..LOOKAHEAD_SAMPLES + input.len()],
        input.as_slice()
    );
}

#[test]
fn enabled_repairs_an_isolated_click_against_clean_reference() {
    let clean: Vec<f32> = (0..256).map(|i| (i as f32 * 0.07).sin() * 0.2).collect();
    let mut corrupt = clean.clone();
    corrupt[128] += 3.0;
    let mut plugin = DeclickPlugin::from_params(
        1,
        SR,
        DeclickPluginParams {
            enabled: true,
            sensitivity: 3.0,
            link_channels: true,
        },
    )
    .unwrap();
    let output = process_and_flush(&mut plugin, &corrupt, 1);
    assert!((output[128 + LOOKAHEAD_SAMPLES] - clean[128]).abs() < 0.05);
}

#[test]
fn reset_preserves_a_legitimate_leading_onset() {
    let mut plugin = plugin(1);
    for _ in 0..2 {
        let input = vec![0.75; 64];
        let output = process_and_flush(&mut plugin, &input, 1);
        assert_eq!(
            &output[LOOKAHEAD_SAMPLES..LOOKAHEAD_SAMPLES + input.len()],
            input.as_slice()
        );
        plugin.reset();
    }
}

#[test]
fn non_finite_samples_recover_without_poisoning_a_channel() {
    let mut plugin = plugin(2);
    let mut input = vec![0.25; 128 * 2];
    input[40 * 2] = f32::NAN;
    input[80 * 2 + 1] = f32::NEG_INFINITY;
    let output = process_and_flush(&mut plugin, &input, 2);
    assert!(output.iter().all(|sample| sample.is_finite()));
}

#[test]
fn rate_and_buffer_contracts_are_rejected_before_mutation() {
    let mut plugin = plugin(2);
    let mut malformed = vec![0.5; 3];
    let original = malformed.clone();
    assert!(plugin.process_in_place(&mut malformed, &ctx(2)).is_err());
    assert_eq!(malformed, original);

    let mut valid = vec![0.5; 4];
    assert!(
        plugin
            .process_in_place(&mut valid, &ProcessContext::new(44_100, 2))
            .is_err()
    );
    assert_eq!(valid, vec![0.5; 4]);
}

#[test]
fn initialize_changes_the_accepted_context_rate_and_resets_latency() {
    let mut plugin = plugin(1);
    plugin.initialize(44_100).unwrap();
    let mut input = vec![0.5; 32];
    plugin
        .process_in_place(&mut input, &ProcessContext::new(44_100, 32))
        .unwrap();
    assert!(input[..LOOKAHEAD_SAMPLES].iter().all(|&x| x == 0.0));
    assert!(plugin.initialize(0).is_err());
}

#[test]
fn bypass_keeps_detector_history_warm_for_reentry() {
    let mut plugin = DeclickPlugin::from_params(
        1,
        SR,
        DeclickPluginParams {
            enabled: false,
            sensitivity: 2.0,
            link_channels: true,
        },
    )
    .unwrap();
    let mut warmup: Vec<f32> = (0..512).map(|i| (i as f32 * 0.04).sin() * 0.2).collect();
    plugin
        .process_in_place(&mut warmup, &ctx(512))
        .expect("bypassed detector should still run");
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(true))
        .unwrap();

    let mut reentry: Vec<f32> = (512..1024).map(|i| (i as f32 * 0.04).sin() * 0.2).collect();
    reentry[256] += 3.0;
    plugin.process_in_place(&mut reentry, &ctx(512)).unwrap();
    assert!(reentry.iter().all(|sample| sample.is_finite()));
    assert!(reentry[256 + LOOKAHEAD_SAMPLES].abs() < 1.0);
}
