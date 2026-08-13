use crate::plugins::{PluginSettings, PluginType};
use sotf_plugins::param_specs;

#[test]
fn dither_parameters_round_trip_through_engine_accessors() {
    let mut settings = PluginSettings::default_for(&PluginType::Dither).unwrap();
    let specs = settings.param_specs();
    assert_eq!(specs.len(), 3);
    assert_eq!(
        specs.iter().map(|spec| spec.engine_key).collect::<Vec<_>>(),
        ["bit_depth", "noise_shaping", "dither_type"]
    );

    assert_eq!(settings.param_value(0), Some(0.0));
    assert_eq!(settings.param_value(1), Some(1.0));
    assert_eq!(settings.param_value(2), Some(0.0));

    settings.set_param_value(0, 2.0);
    settings.set_param_value(1, 0.0);
    settings.set_param_value(2, 2.0);

    assert_eq!(settings.param_value(0), Some(2.0));
    assert_eq!(settings.param_value(1), Some(0.0));
    assert_eq!(settings.param_value(2), Some(2.0));
    assert_eq!(
        settings.engine_param_at(0),
        Some(("bit_depth".to_string(), "2".to_string()))
    );
    assert_eq!(
        settings.engine_param_at(1),
        Some(("noise_shaping".to_string(), "false".to_string()))
    );
    assert_eq!(
        settings.engine_param_at(2),
        Some(("dither_type".to_string(), "2".to_string()))
    );
}

/// Validate that every plugin's LAYOUT indices are within bounds of its PARAMS.
///
/// Iterates all plugin types dynamically so new plugins are automatically covered.
/// This prevents the class of bug where a layout built for one param set
/// (e.g., multiband GLOBAL_PARAMS with crossover entries at 0-5) is accidentally
/// paired with a different param set (e.g., single-band PARAMS starting at threshold=0).
#[test]
fn validate_all_plugin_layout_indices() {
    let mut all_errors = Vec::new();
    for pt in PluginType::all() {
        let name = pt.name();
        let settings = PluginSettings::default_for(&pt).unwrap();
        let params = settings.param_specs();
        if let Some(layout) = settings.layout() {
            let errors = layout.validate(params.len(), name);
            all_errors.extend(errors);
        }
    }

    if !all_errors.is_empty() {
        panic!(
            "LAYOUT/PARAMS index mismatches found:\n  {}",
            all_errors.join("\n  ")
        );
    }
}

/// Validate that every PARAMS index appears somewhere in the LAYOUT.
/// Catches the bug where new params are added to PARAMS but not to LAYOUT,
/// making them invisible in the UI view (while still showing in table view).
///
/// Iterates all plugin types dynamically so new plugins are automatically covered.
#[test]
fn validate_all_params_have_layout_coverage() {
    let mut all_errors = Vec::new();
    for pt in PluginType::all() {
        let name = pt.name();
        let settings = PluginSettings::default_for(&pt).unwrap();
        let params = settings.param_specs();
        if let Some(layout) = settings.layout() {
            let errors = layout.validate_coverage(params, name);
            all_errors.extend(errors);
        }
    }

    if !all_errors.is_empty() {
        panic!(
            "PARAMS entries missing from LAYOUT ({} total):\n  {}",
            all_errors.len(),
            all_errors.join("\n  ")
        );
    }
}

/// Validate that every non-structural PARAMS engine_key that `engine_param_at()`
/// can emit actually exists in the DSP plugin's parameter list.
///
/// This catches the bug where a parameter is declared in PARAMS (so the UI
/// shows it and `engine_param_at()` can send it) but the DSP plugin never
/// registered it in `parameters()` / `set_parameter()`, causing silent drops.
#[test]
fn validate_engine_keys_exist_in_dsp_plugin() {
    let known_gaps: std::collections::HashSet<(&str, &str)> = [
        ("Compressor", "sidechain_hpf_hz"),
        ("Compressor", "sidechain_hpf_order"),
        ("Compressor", "detection_mode"),
        ("Compressor", "program_dependent_release"),
        ("Compressor", "sidechain_external"),
    ]
    .into_iter()
    .collect();

    let mut all_errors = Vec::new();
    for pt in PluginType::all() {
        let name = pt.name();
        let settings = PluginSettings::default_for(&pt).unwrap();
        let specs = settings.param_specs();

        if specs.is_empty() {
            continue;
        }

        // Create the DSP plugin via the factory
        let config = settings.to_plugin_config(44100.0);
        let input_channels = settings.required_input_channels().unwrap_or(2);
        let plugin = match sotf_plugins::create_plugin(
            &config.plugin_type,
            &config.parameters,
            input_channels,
            44100,
        ) {
            Ok(p) => p,
            Err(e) => {
                all_errors.push(format!("{}: failed to create DSP plugin: {}", name, e));
                continue;
            }
        };

        let dsp_params = plugin.parameters();
        let dsp_keys: std::collections::HashSet<&str> =
            dsp_params.iter().map(|p| p.id.as_str()).collect();

        for (i, spec) in specs.iter().enumerate() {
            let Some((engine_key, _)) = settings.engine_param_at(i) else {
                continue;
            };
            if known_gaps.contains(&(name, engine_key.as_str())) {
                continue;
            }
            if !dsp_keys.contains(engine_key.as_str()) {
                all_errors.push(format!(
                    "{} param {} ({}): engine_key '{}' not found in DSP plugin parameters",
                    name, i, spec.name, engine_key
                ));
            }
        }
    }

    if !all_errors.is_empty() {
        panic!(
            "Engine keys missing from DSP plugin ({} total):\n  {}",
            all_errors.len(),
            all_errors.join("\n  ")
        );
    }
}

#[test]
fn spectrum_tilt_params_do_not_emit_engine_updates() {
    let settings = PluginSettings::default_for(&PluginType::SpectrumAnalyzer).unwrap();

    let tilt_correction_idx =
        param_specs::index_of(param_specs::spectrum::PARAMS, "tilt_correction");
    let tilt_reference_idx = param_specs::index_of(param_specs::spectrum::PARAMS, "tilt_reference");

    assert_eq!(settings.engine_param_at(tilt_correction_idx), None);
    assert_eq!(settings.engine_param_at(tilt_reference_idx), None);
}

#[test]
fn compressor_legacy_link_toggle_does_not_emit_live_engine_updates() {
    let settings = PluginSettings::default_for(&PluginType::Compressor).unwrap();
    let link_channels_idx = param_specs::index_of(param_specs::compressor::PARAMS, "link_channels");

    assert_eq!(settings.engine_param_at(link_channels_idx), None);

    let config = settings.to_plugin_config(44_100.0);
    let plugin = sotf_plugins::create_plugin(
        &config.plugin_type,
        &config.parameters,
        settings.required_input_channels().unwrap_or(2),
        44_100,
    )
    .expect("default Compressor config must construct its DSP plugin");
    let dsp_keys: std::collections::HashSet<_> = plugin
        .parameters()
        .into_iter()
        .map(|parameter| parameter.id.as_str().to_owned())
        .collect();
    assert!(dsp_keys.contains("link_amount"));
    assert!(!dsp_keys.contains("link_channels"));
}

#[test]
fn crossover_is_exposed_with_editable_layout_and_dsp_config() {
    assert!(
        PluginType::all().contains(&PluginType::Crossover),
        "Crossover must stay in the app-facing plugin inventory"
    );

    let settings = PluginSettings::default_for(&PluginType::Crossover).unwrap();
    let layout = settings
        .layout()
        .expect("Crossover must have an editable declarative layout");
    assert!(
        layout
            .validate(settings.param_specs().len(), "Crossover")
            .is_empty()
    );
    assert!(
        layout
            .validate_coverage(settings.param_specs(), "Crossover")
            .is_empty(),
        "Crossover layout must expose every declared parameter"
    );

    let config = settings.to_plugin_config(48_000.0);
    assert_eq!(config.plugin_type, "crossover");
    sotf_plugins::create_plugin(&config.plugin_type, &config.parameters, 2, 48_000)
        .expect("Crossover settings must create the DSP plugin");
}

#[test]
fn eq_global_parameters_round_trip_through_engine_accessors() {
    let mut settings = PluginSettings::default_for(&PluginType::EQ).unwrap();
    let params = settings.param_specs();

    assert_eq!(params.len(), param_specs::eq::GLOBAL_PARAMS.len());
    let auto_gain = param_specs::index_of(params, "auto_gain_enabled");
    let oversampling = param_specs::index_of(params, "oversampling");

    assert_eq!(settings.param_value(auto_gain), Some(0.0));
    assert_eq!(settings.param_value(oversampling), Some(1.0));

    settings.set_param_value(auto_gain, 1.0);
    settings.set_param_value(oversampling, 4.0);

    assert_eq!(settings.param_value(auto_gain), Some(1.0));
    assert_eq!(settings.param_value(oversampling), Some(4.0));
    assert_eq!(
        settings.engine_param_at(auto_gain),
        Some(("auto_gain_enabled".to_string(), "true".to_string()))
    );
    assert_eq!(
        settings.engine_param_at(oversampling),
        None,
        "oversampling remains structural and is not sent as a runtime update"
    );
}

#[test]
fn downmix_matrix_ltrt_is_exposed_and_round_trips() {
    let mut settings = PluginSettings::default_for(&PluginType::Downmix).unwrap();
    let matrix_ltrt_idx = param_specs::index_of(param_specs::downmix::PARAMS, "matrix_ltrt");

    assert_eq!(matrix_ltrt_idx, 8);
    assert_eq!(settings.param_specs().len(), 9);
    assert_eq!(settings.param_value(matrix_ltrt_idx), Some(0.0));

    settings.set_param_value(matrix_ltrt_idx, 1.0);
    assert_eq!(settings.param_value(matrix_ltrt_idx), Some(1.0));

    let config = settings.to_plugin_config(48_000.0);
    assert_eq!(config.parameters["matrix_ltrt"], serde_json::json!(true));
}
