use super::catalog::catalog_entry;
use super::create::create_plugin;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::create::create_plugin_with_sandbox_grants;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::create::create_plugin_with_sandbox_grants_for_backend;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::create::create_plugin_with_sandbox_grants_for_backend_and_launcher;
use super::is::is_supported_plugin_type;
use super::parse::parse_external_plugin_descriptor;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::parse::parse_isolated_external_plugin_config;
use super::validate::validate_plugin_security_config;
use crate::{
    ExternalPluginSandboxMode, ExternalPluginState, PluginDescriptor, PluginFormat,
    PluginScanStatus,
};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use crate::{ExternalPluginSandboxTiming, ExternalPluginTrust};
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::Plugin;
use std::path::PathBuf;

use tempfile::tempdir;

mod misc;

#[test]
fn band_merge_factory_rejects_invalid_or_unknown_state() {
    for (channels, parameters) in [
        (0, serde_json::json!({"bands": 2})),
        (2, serde_json::json!({"bands": 1})),
        (2, serde_json::json!({"bands": 9})),
        (2, serde_json::json!({"bands": 2, "band_gains_db": [25.0]})),
        (2, serde_json::json!({"bands": 2, "obsolete": true})),
    ] {
        assert!(
            create_plugin("band_merge", &parameters, channels, 48_000).is_err(),
            "invalid Band Merge preset was accepted: {parameters}"
        );
    }
    let plugin = create_plugin(
        "band_merge",
        &serde_json::json!({
            "bands": 4,
            "band_gains_db": [0.0, -3.0, 2.0, 0.0],
            "band_mutes": [false, true, false, false]
        }),
        8,
        48_000,
    )
    .expect("valid Band Merge preset must construct");
    assert_eq!(plugin.input_channels(), 8);
    assert_eq!(plugin.output_channels(), 2);
}

#[test]
fn band_split_factory_rejects_invalid_topology_and_unknown_state() {
    assert!(create_plugin("band_split", &serde_json::json!({}), 0, 48_000).is_err());
    for parameters in [
        serde_json::json!({"frequencies": [500.0, 500.0]}),
        serde_json::json!({"frequencies": [2_000.0, 500.0]}),
        serde_json::json!({"frequency": 0.0}),
        serde_json::json!({"type": "LR96"}),
        serde_json::json!({"obsolete_split_field": true}),
    ] {
        assert!(
            create_plugin("band_split", &parameters, 2, 48_000).is_err(),
            "invalid Band Split preset was accepted: {parameters}"
        );
    }
    let plugin = create_plugin(
        "band_split",
        &serde_json::json!({"frequencies": [500.0, 2_000.0, 8_000.0], "type": "LR48"}),
        12,
        48_000,
    )
    .expect("valid Band Split preset must construct");
    assert_eq!(plugin.input_channels(), 12);
    assert_eq!(plugin.output_channels(), 48);
}

#[test]
fn gate_factory_rejects_invalid_or_unknown_preset_state() {
    assert!(create_plugin("gate", &serde_json::json!({}), 0, 48_000).is_err());
    for parameters in [
        serde_json::json!({"attack_ms": 0.0}),
        serde_json::json!({"release_ms": 2_001.0}),
        serde_json::json!({"sidechain_hpf_order": "8th"}),
        serde_json::json!({"detection_mode": "average"}),
        serde_json::json!({"obsolete_gate_field": true}),
    ] {
        assert!(
            create_plugin("gate", &parameters, 2, 48_000).is_err(),
            "invalid Gate preset was accepted: {parameters}"
        );
    }

    let plugin = create_plugin(
        "gate",
        &serde_json::json!({
            "threshold_db": -35.0,
            "attack_ms": 2.0,
            "release_ms": 150.0,
            "detection_mode": "RMS",
            "sidechain_hpf_order": "4th"
        }),
        2,
        48_000,
    )
    .expect("valid Gate preset must construct through the factory");
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
}

#[test]
fn hiss_reducer_factory_validates_topology_rate_and_persisted_state() {
    assert!(create_plugin("hiss_reducer", &serde_json::json!({}), 0, 48_000).is_err());
    assert!(create_plugin("hiss_reducer", &serde_json::json!({}), 1, 0).is_err());
    assert!(
        create_plugin(
            "hiss_reducer",
            &serde_json::json!({"obsolete_fft_mode": true}),
            1,
            48_000,
        )
        .is_err()
    );

    let plugin = create_plugin(
        "hiss_reducer",
        &serde_json::json!({"frequency_hz": 16_000.0}),
        1,
        8_000,
    )
    .unwrap();
    assert_eq!(
        plugin
            .get_parameter(&ParameterId::from("frequency_hz"))
            .and_then(|value| value.as_float()),
        Some(3_600.0)
    );
}

#[test]
fn aec_catalog_factory_and_runtime_schema_are_canonical() {
    let entry = catalog_entry("aec").expect("AEC catalog entry");
    assert_eq!(entry.metadata.owning_crate, "sotf-plugin-aec");
    assert_eq!(
        entry.metadata.parameter_schema,
        super::catalog::PluginParameterSchema::Static("sotf_plugin_aec::params::PARAMS")
    );
    assert!(create_plugin("aec", &serde_json::json!({}), 1, 48_000).is_err());
    assert!(create_plugin("aec", &serde_json::json!({}), 3, 48_000).is_err());
    let plugin = create_plugin(
        "aec",
        &serde_json::json!({
            "echo_tail_ms": 100.0,
            "step_size": 0.4,
            "post_filter_enabled": false
        }),
        2,
        48_000,
    )
    .expect("canonical factory must construct AEC");
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 1);
    assert_eq!(
        plugin.get_parameter(&sotf_host::parameters::ParameterId::from("step_size")),
        Some(sotf_host::parameters::ParameterValue::Float(0.4))
    );
}

#[test]
fn ambisonics_catalog_admits_every_supported_order() {
    let entry = catalog_entry("ambisonics_decoder").unwrap();
    assert_eq!(
        entry.metadata.channel_layout.supported_inputs,
        super::catalog::PluginSupportedInputLayouts::Enumerated(&[4, 9, 16])
    );
    for (order, channels, layout) in [(1, 4, "5.1"), (2, 9, "7.1.4"), (3, 16, "9.1.6")] {
        let plugin = create_plugin(
            "ambisonics_decoder",
            &serde_json::json!({"order": order, "target_layout": layout}),
            channels,
            48_000,
        )
        .unwrap();
        assert_eq!(plugin.input_channels(), channels);
    }
}

#[test]
fn dither_catalog_and_factory_are_canonical() {
    let entry = catalog_entry("dither").expect("dither catalog entry");
    assert_eq!(entry.metadata.owning_crate, "sotf-plugin-dither");
    assert_eq!(entry.metadata.exposed_name, "Dither");
    assert_eq!(
        entry.metadata.parameter_schema,
        super::catalog::PluginParameterSchema::Static("sotf_plugin_dither::params::PARAMS")
    );

    let plugin = create_plugin(
        "dither",
        &serde_json::json!({
            "bit_depth": 2,
            "noise_shaping": false,
            "dither_type": 1
        }),
        2,
        96_000,
    )
    .expect("canonical factory must construct Dither");
    assert_eq!(plugin.info().name, "Dither");
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
    assert_eq!(
        plugin.get_parameter(&sotf_host::parameters::ParameterId::from("bit_depth")),
        Some(sotf_host::parameters::ParameterValue::Int(2))
    );
}

#[test]
fn compressor_catalog_and_factory_expose_true_broadband_mode() {
    let entry = catalog_entry("compressor").expect("compressor catalog entry");
    assert_eq!(entry.metadata.exposed_name, "Compressor");
    assert_eq!(
        entry.metadata.parameter_schema,
        super::catalog::PluginParameterSchema::Static(
            "runtime broadband schema (unsupported legacy sidechain controls rejected)"
        )
    );

    let config = serde_json::json!({
        "threshold_db": -24.0,
        "ratio": 4.0,
        "attack_ms": 1.0,
        "release_ms": 40.0,
        "knee_db": 3.0
    });
    let mut plugin = create_plugin("compressor", &config, 2, 48_000).unwrap();
    plugin.initialize(48_000).unwrap();
    assert_eq!(plugin.info().name, "Compressor");
    assert!(
        plugin
            .parameters()
            .iter()
            .all(|parameter| parameter.id.as_str() != "num_bands"
                && !parameter.id.as_str().starts_with("crossover"))
    );

    let mut params: crate::MultibandCompressorPluginParams =
        serde_json::from_value(config).unwrap();
    params.num_bands = 1;
    let mut reference =
        crate::MultibandCompressorPlugin::try_from_params(2, params, 48_000).unwrap();
    sotf_host::ParametricInPlacePlugin::initialize(&mut reference, 48_000).unwrap();
    let frames = 4096;
    let input: Vec<f32> = (0..frames)
        .flat_map(|frame| {
            let t = frame as f32 / 48_000.0;
            let sample = 0.3 * (2.0 * std::f32::consts::PI * 110.0 * t).sin()
                + 0.2 * (2.0 * std::f32::consts::PI * 4_000.0 * t).sin();
            [sample, sample * 0.7]
        })
        .collect();
    let mut factory_output = vec![0.0; input.len()];
    plugin
        .process(
            &input,
            &mut factory_output,
            &sotf_host::ProcessContext::new(48_000, frames),
        )
        .unwrap();
    let mut reference_output = input;
    sotf_host::ParametricInPlacePlugin::process_in_place(
        &mut reference,
        &mut reference_output,
        &sotf_host::ProcessContext::new(48_000, frames),
    )
    .unwrap();
    assert_eq!(factory_output, reference_output);
}

#[test]
fn ambisonics_catalog_matches_factory_order_contract() {
    let entry = catalog_entry("ambisonics_decoder").expect("ambisonics catalog entry");
    let super::catalog::PluginSupportedInputLayouts::Enumerated(widths) =
        entry.metadata.channel_layout.supported_inputs
    else {
        panic!("Ambisonics channel contract must enumerate supported HOA widths");
    };
    assert_eq!(widths, &[4]);

    for (channels, order, layout) in [(4, 1, "5.1")] {
        let plugin = create_plugin(
            "ambisonics_decoder",
            &serde_json::json!({
                "order": order,
                "target_layout": layout,
            }),
            channels,
            48_000,
        )
        .unwrap_or_else(|error| panic!("order-{order} factory contract failed: {error}"));
        assert_eq!(plugin.input_channels(), channels);
    }

    let order3 = create_plugin(
        "ambisonics_decoder",
        &serde_json::json!({"order": 3, "target_layout": "9.1.6"}),
        16,
        48_000,
    );
    assert!(
        order3.is_err(),
        "order-3 must not be advertised until a built-in layout has 16 non-LFE feeds"
    );
}

#[test]
fn transient_shaper_facade_factory_validates_constructor_contract() {
    let out_of_range = create_plugin(
        "transient_shaper",
        &serde_json::json!({"attack": 101.0}),
        2,
        48_000,
    );
    assert!(
        out_of_range.is_err(),
        "facade factory must reject transient-shaper values outside the parameter schema"
    );

    let zero_channels = create_plugin("transient_shaper", &serde_json::json!({}), 0, 48_000);
    assert!(
        zero_channels.is_err(),
        "facade factory must reject a zero-channel transient shaper"
    );
}

#[test]
fn delay_facade_factory_validates_scalar_and_per_channel_contracts() {
    for parameters in [
        serde_json::json!({"delay_ms": -0.01}),
        serde_json::json!({"delay_ms": 5_000.01}),
        serde_json::json!({"feedback": -0.96}),
        serde_json::json!({"feedback": 0.96}),
        serde_json::json!({"mix": -0.01}),
        serde_json::json!({"mix": 1.01}),
        serde_json::json!({"lfo_rate_hz": 20.01}),
        serde_json::json!({"lfo_depth_ms": 10.01}),
        serde_json::json!({"allpass_coeff": 0.991}),
    ] {
        assert!(
            create_plugin("delay", &parameters, 2, 48_000).is_err(),
            "facade factory accepted invalid Delay parameters: {parameters}"
        );
    }
    assert!(create_plugin("delay", &serde_json::json!({}), 0, 48_000).is_err());

    for parameters in [
        serde_json::json!({
            "channel_delays_ms": [1.0, 2.0],
            "feedback": 0.1,
            "mix": 1.0
        }),
        serde_json::json!({
            "channel_delays_ms": [1.0, 2.0],
            "feedback": 0.0,
            "mix": 1.0,
            "lfo_rate_hz": 1.0,
            "lfo_depth_ms": 1.0
        }),
        serde_json::json!({
            "channel_delays_ms": [1.0, 2.0],
            "feedback": 0.0,
            "mix": 1.0,
            "allpass_feedback": true
        }),
    ] {
        assert!(
            create_plugin("delay", &parameters, 2, 48_000).is_err(),
            "per-channel routing mode accepted effect controls: {parameters}"
        );
    }

    let plugin = create_plugin(
        "delay",
        &serde_json::json!({
            "channel_delays_ms": [0.0, 2.0],
            "feedback": 0.0,
            "mix": 1.0
        }),
        2,
        48_000,
    )
    .expect("pure per-channel routing delay must construct");
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
}

#[test]
fn expander_factory_is_broadband_and_validates_presets() {
    let plugin = create_plugin(
        "expander",
        &serde_json::json!({
            "threshold_db": -35.0,
            "ratio": 4.0,
            "detection_mode": "RMS",
            "sidechain_hpf_hz": 80.0
        }),
        2,
        48_000,
    )
    .expect("valid broadband expander");
    assert_eq!(plugin.info().name, "Expander");
    let ids: Vec<_> = plugin
        .parameters()
        .into_iter()
        .map(|parameter| parameter.id)
        .collect();
    assert!(!ids.iter().any(|id| id.as_str() == "num_bands"));
    assert!(!ids.iter().any(|id| id.as_str().starts_with("crossover_")));

    for parameters in [
        serde_json::json!({"threshold_db": f64::NAN}),
        serde_json::json!({"ratio": 0.9}),
        serde_json::json!({"detection_mode": "average"}),
    ] {
        assert!(
            create_plugin("expander", &parameters, 2, 48_000).is_err(),
            "invalid expander preset accepted: {parameters}"
        );
    }
    assert!(create_plugin("expander", &serde_json::json!({}), 0, 48_000).is_err());

    assert!(
        create_plugin(
            "multiband_expander",
            &serde_json::json!({"num_bands": 1}),
            2,
            48_000,
        )
        .is_err()
    );
}

#[test]
fn spectral_compressor_factory_preserves_complete_state_and_rejects_drift() {
    let plugin = create_plugin(
        "spectral_compressor",
        &serde_json::json!({
            "fft_size_index": 0,
            "threshold_db": -31.0,
            "ratio": 4.0,
            "attack_ms": 7.0,
            "release_ms": 90.0,
            "knee_db": 3.0,
            "spectral_smoothing": 0.4,
            "mix": 0.8,
            "target_mode": 2,
            "delta_listen": true,
            "adaptive_threshold": true,
            "adaptive_offset_db": 4.0,
            "channel_link": 0.75
        }),
        2,
        48_000,
    )
    .expect("complete spectral-compressor state must construct");
    for (id, expected) in [
        ("target_mode", ParameterValue::Int(2)),
        ("delta_listen", ParameterValue::Bool(true)),
        ("adaptive_threshold", ParameterValue::Bool(true)),
        ("adaptive_offset_db", ParameterValue::Float(4.0)),
        ("channel_link", ParameterValue::Float(0.75)),
    ] {
        assert_eq!(plugin.get_parameter(&ParameterId::from(id)), Some(expected));
    }

    for parameters in [
        serde_json::json!({"channel_link": 1.01}),
        serde_json::json!({"target_mode": 3}),
        serde_json::json!({"unknown_future_control": true}),
    ] {
        assert!(
            create_plugin("spectral_compressor", &parameters, 2, 48_000).is_err(),
            "invalid spectral-compressor state was accepted: {parameters}"
        );
    }
}

#[test]
fn channel_mute_solo_facade_factory_validates_constructor_contract() {
    let out_of_range = create_plugin(
        "channel_mute_solo",
        &serde_json::json!({"dim_gain_db": 1.0}),
        2,
        48_000,
    );
    assert!(
        out_of_range.is_err(),
        "facade factory must reject dim gain above the attenuation range"
    );

    let zero_channels = create_plugin("channel_mute_solo", &serde_json::json!({}), 0, 48_000);
    assert!(
        zero_channels.is_err(),
        "facade factory must reject a zero-channel mute/solo plugin"
    );
}

#[test]
fn mono_to_stereo_facade_factory_validates_constructor_contract() {
    for parameters in [
        serde_json::json!({"stereo_width": -0.01}),
        serde_json::json!({"stereo_width": 1.01}),
        serde_json::json!({"haas_delay_ms": -0.01}),
        serde_json::json!({"haas_delay_ms": 5.01}),
        serde_json::json!({"decor_low_hz": 99.0}),
        serde_json::json!({"decor_low_hz": 501.0}),
        serde_json::json!({"decor_high_hz": 999.0}),
        serde_json::json!({"decor_high_hz": 5_001.0}),
    ] {
        assert!(
            create_plugin("mono_to_stereo", &parameters, 1, 48_000).is_err(),
            "facade factory must reject values outside the Mono-to-Stereo schema: {parameters}"
        );
    }

    assert!(
        create_plugin("mono_to_stereo", &serde_json::json!({}), 2, 48_000).is_err(),
        "facade factory must reject a non-mono input layout"
    );

    for parameters in [
        serde_json::json!({
            "stereo_width": 0.0,
            "haas_delay_ms": 0.0,
            "decor_low_hz": 100.0,
            "decor_high_hz": 1_000.0
        }),
        serde_json::json!({
            "stereo_width": 1.0,
            "haas_delay_ms": 5.0,
            "decor_low_hz": 500.0,
            "decor_high_hz": 5_000.0
        }),
    ] {
        let plugin = create_plugin("mono_to_stereo", &parameters, 1, 48_000)
            .unwrap_or_else(|error| panic!("schema endpoint must construct: {error}"));
        assert_eq!(plugin.input_channels(), 1);
        assert_eq!(plugin.output_channels(), 2);
        assert_eq!(
            plugin.get_parameter(&sotf_host::parameters::ParameterId::from("decor_low_hz")),
            Some(sotf_host::parameters::ParameterValue::Float(
                parameters["decor_low_hz"].as_f64().unwrap() as f32
            ))
        );
        assert_eq!(
            plugin.get_parameter(&sotf_host::parameters::ParameterId::from("decor_high_hz")),
            Some(sotf_host::parameters::ParameterValue::Float(
                parameters["decor_high_hz"].as_f64().unwrap() as f32
            ))
        );
    }

    assert!(
        create_plugin(
            "mono_to_stereo",
            &serde_json::json!({"decor_high_hz": 5_000.0}),
            1,
            8_000,
        )
        .is_err(),
        "facade factory must reject a decorrelator crossover above Nyquist"
    );
}

#[test]
fn ab_compare_facade_injects_factory_before_initial_path_build() {
    let parameters = serde_json::json!({
        "path_a": {
            "type": "Plugin",
            "plugin_type": "expander",
            "parameters": {}
        },
        "path_b": {
            "type": "Rack",
            "plugins": [{"plugin_type": "hiss_reducer", "parameters": {}}]
        },
        "auto_gain_enabled": false
    });
    let plugin = create_plugin("ab_compare", &parameters, 2, 48_000)
        .expect("canonical factory must be installed before nested path construction");
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
}

#[test]
fn binaural_catalog_and_factory_share_exact_layout_contract() {
    let entry = catalog_entry("binaural_decoder").unwrap();
    assert_eq!(
        entry.metadata.channel_layout.supported_inputs,
        super::catalog::PluginSupportedInputLayouts::Enumerated(&[
            1, 2, 3, 5, 6, 8, 10, 12, 14, 16
        ])
    );
    for channels in [1, 2, 3, 5, 6, 8, 10, 12, 14, 16] {
        let plugin = create_plugin(
            "binaural_decoder",
            &serde_json::json!({"input_channels": channels, "diffuse_field_eq": false}),
            channels,
            48_000,
        )
        .unwrap();
        assert_eq!(plugin.input_channels(), channels);
    }
    for channels in [4, 7, 9, 11, 13, 15] {
        assert!(
            create_plugin(
                "binaural_decoder",
                &serde_json::json!({"input_channels": channels}),
                channels,
                48_000,
            )
            .is_err()
        );
    }
}

#[test]
fn crossover_catalog_and_factory_report_compiled_topology() {
    let entry = catalog_entry("crossover").unwrap();
    assert!(matches!(
        entry.metadata.channel_layout.output,
        super::catalog::PluginChannelOutputModel::Configurable { .. }
    ));
    let plugin = create_plugin(
        "crossover",
        &serde_json::json!({
            "type": "LR24",
            "frequency": 500.0,
            "output": "both",
            "extra_frequencies": [2_000.0]
        }),
        2,
        48_000,
    )
    .unwrap();
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 6);
    let parameters = plugin.parameters();
    let ids: Vec<_> = parameters
        .iter()
        .map(|parameter| parameter.id.as_str())
        .collect();
    assert_eq!(ids, ["type", "frequency", "mode", "frequency_2"]);
}

#[test]
fn compressor_factory_rejects_invalid_dsp_configuration() {
    let invalid_ratio = serde_json::json!({"ratio": 0.5});
    let error = match create_plugin("compressor", &invalid_ratio, 2, 48_000) {
        Err(error) => error,
        Ok(_) => panic!("compressor factory must reject ratios below its schema range"),
    };
    assert!(error.contains("ratio"), "unexpected error: {error}");

    let descending_crossovers = serde_json::json!({
        "crossover_frequencies": [200.0, 100.0, 8_000.0, 12_000.0]
    });
    let error = match create_plugin("multiband_compressor", &descending_crossovers, 2, 48_000) {
        Err(error) => error,
        Ok(_) => panic!("factory must reject descending crossover frequencies"),
    };
    assert!(error.contains("crossover"), "unexpected error: {error}");
}

#[test]
fn supported_plugin_type_list_covers_factory_aliases() {
    assert!(is_supported_plugin_type("gain"));
    assert!(is_supported_plugin_type("EQ"));
    assert!(is_supported_plugin_type("rnnoise"));
    assert!(is_supported_plugin_type("active_acoustic_enhancement"));
    assert!(is_supported_plugin_type("external"));
    assert!(is_supported_plugin_type("external_plugin"));
    assert!(!is_supported_plugin_type("definitely_missing"));
}

#[test]
fn factory_rejects_invalid_de_esser_configuration() {
    let invalid_mode = serde_json::json!({
        "mode": "not-a-de-esser-mode",
    });
    assert!(
        create_plugin("de_esser", &invalid_mode, 1, 48_000).is_err(),
        "factory must not silently map an unknown De-Esser mode to Split-Band"
    );

    let out_of_range = serde_json::json!({
        "frequency": 16_001.0,
    });
    assert!(
        create_plugin("de_esser", &out_of_range, 1, 48_000).is_err(),
        "factory must reject De-Esser values outside the public schema"
    );

    let nyquist_invalid = serde_json::json!({
        "frequency": 16_000.0,
    });
    assert!(
        create_plugin("de_esser", &nyquist_invalid, 1, 22_050).is_err(),
        "factory must reject a De-Esser band that cannot be represented at the host rate"
    );
}

#[test]
fn factory_rejects_invalid_dynamic_eq_configuration() {
    let invalid_global = serde_json::json!({
        "threshold": 1.0,
    });
    assert!(
        create_plugin("dynamic_eq", &invalid_global, 1, 48_000).is_err(),
        "factory must reject Dynamic EQ global values outside the public schema"
    );

    let invalid_band = serde_json::json!({
        "bands": [{"frequency": 0.0}],
    });
    assert!(
        create_plugin("dynamic_eq", &invalid_band, 1, 48_000).is_err(),
        "factory must reject Dynamic EQ per-band values outside the public schema"
    );

    let low_rate_invalid = serde_json::json!({
        "bands": [{"frequency": 10_000.0}],
    });
    assert!(
        create_plugin("dynamic_eq", &low_rate_invalid, 16, 16_000).is_err(),
        "factory must reject a Dynamic EQ band outside the host Nyquist margin"
    );
}

#[test]
fn create_external_plugin_from_path() {
    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-test-plugin.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();
    let params = serde_json::json!({
        "path": plugin_path.to_string_lossy(),
        "audio_inputs": 2,
        "audio_outputs": 2,
        "name": "External Test",
        "format": "clap",
    });

    let plugin = create_plugin("external", &params, 2, 48_000).unwrap();
    assert_eq!(plugin.input_channels(), 2);
}

#[test]
fn create_external_plugin_from_path_string() {
    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-test-plugin-string.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();

    let plugin = create_plugin(
        "external",
        &serde_json::json!({
            "path": plugin_path.to_string_lossy(),
            "audio_inputs": 2,
            "audio_outputs": 2,
            "name": "External Test",
            "format": "clap",
        }),
        2,
        48_000,
    )
    .unwrap();
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
}

#[test]
fn create_external_plugin_from_embedded_descriptor() {
    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-test-plugin.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();
    let descriptor = PluginDescriptor {
        id: "test.external".into(),
        name: "Embedded External Test".into(),
        vendor: "Test".into(),
        version: "0.1.0".into(),
        format: PluginFormat::Clap,
        path: plugin_path.clone(),
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec!["testing".into()],
        scan_status: PluginScanStatus::Discovered,
    };

    let plugin = create_plugin(
        "external_plugin",
        &serde_json::json!({"descriptor": descriptor}),
        2,
        48_000,
    )
    .unwrap();
    assert_eq!(plugin.output_channels(), 2);
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn factory_rejects_external_state_for_a_different_sandbox_mode() {
    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-state-mode.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();
    let descriptor = PluginDescriptor {
        id: "test.external.state-mode".into(),
        name: "External State Mode Test".into(),
        vendor: "Test".into(),
        version: "0.1.0".into(),
        format: PluginFormat::Clap,
        path: plugin_path,
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec!["testing".into()],
        scan_status: PluginScanStatus::Discovered,
    };
    let descriptor = parse_external_plugin_descriptor(&serde_json::json!({
        "descriptor": descriptor
    }))
    .unwrap();
    let state = ExternalPluginState::new(
        descriptor.clone(),
        ExternalPluginSandboxMode::InProcess,
        vec![1, 2, 3],
    );

    let error = match create_plugin(
        "external",
        &serde_json::json!({
            "descriptor": descriptor,
            "external_state": state,
            "start_worker": false,
        }),
        2,
        48_000,
    ) {
        Ok(_) => panic!("in-process state must not load into the default isolated host"),
        Err(error) => error,
    };
    assert!(error.contains("cannot restore isolated plugin"), "{error}");
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn create_external_plugin_defaults_to_isolated_when_trust_unknown() {
    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-test-plugin-isolated.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();
    let params = serde_json::json!({
        "path": plugin_path.to_string_lossy(),
        "audio_inputs": 2,
        "audio_outputs": 2,
        "name": "External Isolated Test",
        "format": "clap",
        "plugin_trust": "unknown",
        "start_worker": false,
        "deadline_micros": 0,
        "_sotf_instance_id": 37,
    });

    let mut plugin = create_plugin("external", &params, 2, 48_000).unwrap();
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
    let isolated = plugin
        .as_any()
        .and_then(|plugin| plugin.downcast_ref::<crate::IsolatedExternalPlugin>())
        .expect("factory must construct an isolated external plugin");
    assert_eq!(isolated.plugin_instance_id(), Some(37));

    let input = vec![0.25, -0.5, 1.0, -1.0];
    let mut output = vec![0.0; input.len()];
    let frames = plugin
        .process(
            &input,
            &mut output,
            &sotf_host::ProcessContext::new(48_000, 2),
        )
        .unwrap();
    assert_eq!(frames, 2);
    assert_eq!(output, input);
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn create_external_plugin_respects_backend_for_host_owned_sandbox_grants() {
    use crate::{
        PluginSandboxGrantStore, PluginSandboxIdentity, PluginSandboxNetworkGrant,
        PluginSandboxPermission, PluginSandboxUserGrant,
    };

    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-test-plugin-grants.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();
    let params = serde_json::json!({
        "path": plugin_path.to_string_lossy(),
        "audio_inputs": 2,
        "audio_outputs": 2,
        "name": "External Grant Test",
        "vendor": "Test Vendor",
        "id": "com.test.grants",
        "format": "clap",
        "plugin_trust": "unknown",
        "start_worker": false,
    });
    let descriptor = parse_external_plugin_descriptor(&params).unwrap();
    let identity = PluginSandboxIdentity::from_descriptor(&descriptor);
    let mut grants = PluginSandboxGrantStore::default();
    grants.remember(PluginSandboxUserGrant {
        identity,
        permission: PluginSandboxPermission::Network(PluginSandboxNetworkGrant::AnyOutbound),
    });

    let expected_policy = grants.strict_policy_for_plugin(&descriptor, dir.path().join("presets"));
    let backend_can_launch = expected_policy
        .current_backend_launch_plan()
        .validate_for_launch(&expected_policy)
        .is_ok();

    let result = create_plugin_with_sandbox_grants(
        "external",
        &params,
        2,
        48_000,
        &grants,
        dir.path().join("presets"),
    );

    if backend_can_launch {
        let plugin = result.unwrap();
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 2);
    } else {
        let err = match result {
            Ok(_) => panic!("expected unsupported sandbox backend to fail"),
            Err(err) => err,
        };
        assert!(err.contains("cannot satisfy required policy"));
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn create_external_plugin_accepts_host_selected_store_sandbox_backend() {
    use crate::{
        ExternalPluginWorkerCommand, PluginSandboxGrantStore, PluginSandboxLaunchBackend,
        PluginSandboxNetworkGrant, PluginSandboxPermission, PluginSandboxUserGrant,
    };

    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-test-plugin-store-helper.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();
    let params = serde_json::json!({
        "path": plugin_path.to_string_lossy(),
        "audio_inputs": 2,
        "audio_outputs": 2,
        "name": "External Store Helper Test",
        "vendor": "Test Vendor",
        "id": "com.test.store-helper",
        "format": "clap",
        "plugin_trust": "unknown",
        "start_worker": false,
    });
    let descriptor = parse_external_plugin_descriptor(&params).unwrap();
    let mut grants = PluginSandboxGrantStore::default();
    grants.remember(PluginSandboxUserGrant {
        identity: crate::PluginSandboxIdentity::from_descriptor(&descriptor),
        permission: PluginSandboxPermission::Network(PluginSandboxNetworkGrant::AnyOutbound),
    });

    let plugin = create_plugin_with_sandbox_grants_for_backend_and_launcher(
        "external",
        &params,
        2,
        48_000,
        &grants,
        dir.path().join("presets"),
        PluginSandboxLaunchBackend::MacosAppSandboxHelper,
        Some(ExternalPluginWorkerCommand::new("/tmp/sotf-sandbox-helper")),
    )
    .unwrap();

    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn create_external_plugin_rejects_helper_backend_without_launcher() {
    use crate::{PluginSandboxGrantStore, PluginSandboxLaunchBackend};

    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-test-plugin-no-helper.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();
    let params = serde_json::json!({
        "path": plugin_path.to_string_lossy(),
        "audio_inputs": 2,
        "audio_outputs": 2,
        "name": "External Missing Helper Test",
        "vendor": "Test Vendor",
        "id": "com.test.no-helper",
        "format": "clap",
        "plugin_trust": "unknown",
        "start_worker": false,
    });
    let grants = PluginSandboxGrantStore::default();

    let err = match create_plugin_with_sandbox_grants_for_backend(
        "external",
        &params,
        2,
        48_000,
        &grants,
        dir.path().join("presets"),
        PluginSandboxLaunchBackend::MacosAppSandboxHelper,
    ) {
        Ok(_) => panic!("expected helper backend to require launcher command"),
        Err(err) => err,
    };

    assert!(err.contains("requires a host-owned sandbox launcher command"));
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn create_external_plugin_rejects_unrepresentable_host_owned_sandbox_grants() {
    use crate::{
        PluginSandboxGrantStore, PluginSandboxIdentity, PluginSandboxNetworkGrant,
        PluginSandboxPermission, PluginSandboxUserGrant,
    };

    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-test-plugin-loopback.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();
    let params = serde_json::json!({
        "path": plugin_path.to_string_lossy(),
        "audio_inputs": 2,
        "audio_outputs": 2,
        "name": "External Loopback Test",
        "vendor": "Test Vendor",
        "id": "com.test.loopback",
        "format": "clap",
        "plugin_trust": "unknown",
        "start_worker": false,
    });
    let descriptor = parse_external_plugin_descriptor(&params).unwrap();
    let identity = PluginSandboxIdentity::from_descriptor(&descriptor);
    let mut grants = PluginSandboxGrantStore::default();
    grants.remember(PluginSandboxUserGrant {
        identity,
        permission: PluginSandboxPermission::Network(PluginSandboxNetworkGrant::LoopbackOnly),
    });

    let err = match create_plugin_with_sandbox_grants(
        "external",
        &params,
        2,
        48_000,
        &grants,
        dir.path().join("presets"),
    ) {
        Ok(_) => panic!("expected unrepresentable sandbox grant to fail"),
        Err(err) => err,
    };

    assert!(
        err.contains("cannot launch current worker policy")
            || err.contains("cannot satisfy required policy")
    );
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn create_external_plugin_rejects_worker_overrides_from_config() {
    let err = parse_isolated_external_plugin_config(
        &serde_json::json!({
            "worker_path": "/usr/bin/sotf-test-worker",
            "start_worker": false,
        }),
        ExternalPluginTrust::Unknown,
    )
    .unwrap_err();

    assert!(err.contains("worker_path"));
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn isolated_external_plugin_config_uses_bundled_worker() {
    let config = parse_isolated_external_plugin_config(
        &serde_json::json!({
            "start_worker": false,
            "deadline_micros": 250,
            "max_block_frames": 1024,
            "_sotf_instance_id": 37,
        }),
        ExternalPluginTrust::Unknown,
    )
    .unwrap();

    assert!(config.worker_command.program().is_absolute());
    assert!(config.worker_command.command_args().is_empty());
    assert!(config.worker_command.command_env().is_empty());
    assert!(!config.start_worker);
    assert_eq!(config.deadline, std::time::Duration::from_micros(250));
    assert_eq!(config.max_block_frames, 1024);
    assert_eq!(config.plugin_instance_id, Some(37));
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn isolated_external_plugin_config_maps_trust_to_sandbox_timing() {
    let signed = parse_isolated_external_plugin_config(
        &serde_json::json!({
            "sandbox_read_paths": ["/Library/Audio/Plug-Ins"],
            "sandbox_write_paths": ["/tmp/sotf-plugin-cache"],
        }),
        ExternalPluginTrust::Signed,
    )
    .unwrap();
    assert_eq!(
        signed.sandbox_policy.timing,
        ExternalPluginSandboxTiming::AfterPluginLoad
    );
    assert!(!signed.sandbox_policy.require_platform_sandbox);
    assert_eq!(
        signed.sandbox_policy.extra_read_paths,
        vec![PathBuf::from("/Library/Audio/Plug-Ins")]
    );

    let untrusted = parse_isolated_external_plugin_config(
        &serde_json::json!({
            "plugin_trust": "untrusted"
        }),
        ExternalPluginTrust::Untrusted,
    )
    .unwrap();
    assert_eq!(
        untrusted.sandbox_policy.timing,
        ExternalPluginSandboxTiming::BeforePluginLoad
    );
    assert_eq!(
        untrusted.sandbox_policy.require_platform_sandbox,
        cfg!(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "windows"
        ))
    );
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn external_plugin_security_rejects_self_declared_signed_trust() {
    let err = validate_plugin_security_config(
        "external",
        &serde_json::json!({
            "path": "/tmp/fake.clap",
            "plugin_trust": "signed"
        }),
    )
    .unwrap_err();

    assert!(err.contains("cannot mark external plugins as signed"));
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn external_plugin_security_rejects_untrusted_in_process() {
    let err = validate_plugin_security_config(
        "external",
        &serde_json::json!({
            "path": "/tmp/fake.clap",
            "isolated": false
        }),
    )
    .unwrap_err();

    assert!(err.contains("cannot disable process isolation"));
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn external_plugin_security_rejects_relaxed_untrusted_sandbox() {
    let err = validate_plugin_security_config(
        "external",
        &serde_json::json!({
            "path": "/tmp/fake.clap",
            "sandbox_timing": "disabled",
            "start_worker": false
        }),
    )
    .unwrap_err();

    assert!(err.contains("before plugin load"));
}

#[test]
fn create_external_plugin_reports_invalid_parameters() {
    let err = match create_plugin(
        "external",
        &serde_json::json!({"audio_inputs": 2}),
        2,
        48_000,
    ) {
        Ok(_) => panic!("external plugin creation should fail"),
        Err(err) => err,
    };
    assert!(
        err.contains("External plugin descriptor is missing required `path`")
            || err.contains("path")
    );
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn isolated_untrusted_config_rejects_broad_read_write_paths() {
    let err = parse_isolated_external_plugin_config(
        &serde_json::json!({
            "sandbox_read_paths": ["/"],
            "sandbox_write_paths": ["/tmp"],
        }),
        ExternalPluginTrust::Untrusted,
    )
    .unwrap_err();
    assert!(err.contains("cannot expand sandbox filesystem access"));
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn isolated_untrusted_config_rejects_network_grant() {
    let err = parse_isolated_external_plugin_config(
        &serde_json::json!({"sandbox_allow_network": true}),
        ExternalPluginTrust::Untrusted,
    )
    .unwrap_err();
    assert!(err.contains("cannot allow network access"));
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn isolated_untrusted_config_rejects_child_process_grant() {
    let err = parse_isolated_external_plugin_config(
        &serde_json::json!({"sandbox_allow_child_processes": true}),
        ExternalPluginTrust::Untrusted,
    )
    .unwrap_err();
    assert!(err.contains("cannot allow child processes"));
}

#[test]
fn create_external_plugin_rejects_missing_file_path() {
    let err = match create_plugin(
        "external",
        &serde_json::json!({
            "path": "/nonexistent/path/to/plugin.clap",
            "audio_inputs": 2,
            "audio_outputs": 2,
            "name": "Missing Plugin",
            "format": "clap",
        }),
        2,
        48_000,
    ) {
        Ok(_) => panic!("expected missing external plugin path to fail"),
        Err(err) => err,
    };
    assert!(
        err.to_ascii_lowercase().contains("path")
            || err.to_ascii_lowercase().contains("file")
            || err.to_ascii_lowercase().contains("no such")
    );
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn external_plugin_state_stays_consistent_after_invalid_parameter_changes() {
    let dir = tempdir().unwrap();
    let plugin_path = dir.path().join("external-param-corruption.clap");
    std::fs::write(&plugin_path, b"stub plugin").unwrap();
    let params = serde_json::json!({
        "path": plugin_path.to_string_lossy(),
        "audio_inputs": 2,
        "audio_outputs": 2,
        "name": "Param Corruption Test",
        "format": "clap",
        "plugin_trust": "unknown",
        "start_worker": false,
    });

    let mut plugin = create_plugin("external", &params, 2, 48_000).unwrap();
    plugin.initialize(48_000).unwrap();

    // Unknown parameter id should be ignored, not corrupt state.
    let _ = plugin.set_parameter(
        sotf_host::parameters::ParameterId::from("definitely_not_a_real_parameter"),
        sotf_host::parameters::ParameterValue::Float(1.0),
    );

    // Out-of-range value should be rejected, not corrupt state.
    let _ = plugin.set_parameter(
        sotf_host::parameters::ParameterId::from("mix"),
        sotf_host::parameters::ParameterValue::Float(f32::NAN),
    );

    let input = vec![0.25, -0.5, 1.0, -1.0];
    let mut output = vec![0.0; input.len()];
    let frames = plugin
        .process(
            &input,
            &mut output,
            &sotf_host::ProcessContext::new(48_000, 2),
        )
        .unwrap();
    assert_eq!(frames, 2);
}

#[test]
fn beamformer_factory_matches_fallible_constructor_validation() {
    for params in [
        serde_json::json!({"num_mics": 1}),
        serde_json::json!({"num_mics": 2, "mic_spacing_cm": 100.0}),
        serde_json::json!({"num_mics": 2, "steer_angle_deg": 200.0}),
        serde_json::json!({"num_mics": 2, "beamformer_type": "unknown"}),
    ] {
        assert!(
            create_plugin("beamformer", &params, 2, 48_000).is_err(),
            "{params}"
        );
    }
    assert!(
        create_plugin(
            "beamformer",
            &serde_json::json!({"num_mics": 2, "beamformer_type": "Superdirective"}),
            2,
            48_000,
        )
        .is_ok()
    );
}
