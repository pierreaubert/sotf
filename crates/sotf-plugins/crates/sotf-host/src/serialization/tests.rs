use super::error::PresetLoadError;
use super::plugin_preset::PluginPreset;
use super::preset_bank::PresetBank;
use super::types::PresetSearchField;
use crate::external_plugin::ExternalPluginState;
use crate::parameters::ParameterValue;

fn preset(name: &str, plugin_id: &str, tags: &[&str]) -> PluginPreset {
    let mut preset = PluginPreset::new(name.into(), plugin_id.into(), "1.2.3".into());
    for tag in tags {
        preset.add_tag(*tag);
    }
    preset
}

#[test]
fn presets_with_tag_remains_exact() {
    let mut bank = PresetBank::new("Factory");
    bank.add_preset(preset("Warm Bus", "sotf-eq", &["Bus"]));

    assert_eq!(bank.presets_with_tag("Bus").len(), 1);
    assert!(bank.presets_with_tag("bus").is_empty());
}

#[test]
fn search_matches_name_case_insensitively() {
    let mut bank = PresetBank::new("Factory");
    bank.add_preset(preset("Warm Analog Bus", "sotf-eq", &["mix"]));
    bank.add_preset(preset("Clean Vocal", "sotf-compressor", &["voice"]));

    let results = bank.search("analog");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].preset.name, "Warm Analog Bus");
    assert!(results[0].matched_fields.contains(&PresetSearchField::Name));
}

#[test]
fn search_matches_author_comment_and_tags() {
    let mut bank = PresetBank::new("Factory");
    let mut vocal = preset("Smooth Lead", "sotf-compressor", &["vocal"]);
    vocal.set_author("Ada");
    vocal.set_comment("Gentle leveling for spoken voice");
    bank.add_preset(vocal);

    assert_eq!(bank.search("ada")[0].preset.name, "Smooth Lead");
    assert_eq!(bank.search("spoken")[0].preset.name, "Smooth Lead");
    assert_eq!(bank.search("vocal")[0].preset.name, "Smooth Lead");
}

#[test]
fn search_requires_all_terms_but_allows_partial_terms() {
    let mut bank = PresetBank::new("Factory");
    bank.add_preset(preset("Warm Analog Bus", "sotf-eq", &["mixbus"]));
    bank.add_preset(preset("Warm Vocal", "sotf-compressor", &["voice"]));

    let results = bank.search("warm ana");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].preset.name, "Warm Analog Bus");
}

#[test]
fn search_ranks_exact_name_above_tag_match() {
    let mut bank = PresetBank::new("Factory");
    bank.add_preset(preset("Glue", "sotf-compressor", &["master"]));
    bank.add_preset(preset("Master", "sotf-eq", &["utility"]));

    let results = bank.search("master");

    assert_eq!(results[0].preset.name, "Master");
    assert_eq!(results[1].preset.name, "Glue");
    assert!(results[0].score > results[1].score);
}

#[test]
fn empty_search_returns_no_results() {
    let mut bank = PresetBank::new("Factory");
    bank.add_preset(preset("Warm Analog Bus", "sotf-eq", &["mix"]));

    assert!(bank.search("  ").is_empty());
}

#[test]
fn is_version_compatible_major_only() {
    let mut bank = PresetBank::new("Factory");
    bank.add_preset(PluginPreset::new(
        "Legacy".into(),
        "sotf-eq".into(),
        "2.4.0".into(),
    ));
    bank.add_preset(PluginPreset::new(
        "Next".into(),
        "sotf-eq".into(),
        "3.0.0".into(),
    ));

    let presets = bank.presets_for_plugin_version("sotf-eq", "2.9.7");
    assert_eq!(presets.len(), 1);
    assert_eq!(presets[0].name, "Legacy");
}

#[test]
fn find_preset_for_load_reports_errors() {
    let mut bank = PresetBank::new("Factory");
    let mut mismatch = preset("EQPreset", "sotf-eq", &[]);
    mismatch.version = "1.2.9".into();
    bank.add_preset(mismatch);
    bank.add_preset(preset("Comp", "sotf-compressor", &[]));

    assert_eq!(
        bank.find_preset_for_load("Nope", "sotf-eq", "1.2.3"),
        Err(PresetLoadError::MissingPreset)
    );
    assert_eq!(
        bank.find_preset_for_load("EQPreset", "sotf-compressor", "1.2.3"),
        Err(PresetLoadError::PluginMismatch)
    );
    assert_eq!(
        bank.find_preset_for_load("EQPreset", "sotf-eq", "2.0.0"),
        Err(PresetLoadError::VersionMismatch)
    );
    assert!(
        bank.find_preset_for_load("EQPreset", "sotf-eq", "1.2.9")
            .is_ok()
    );
}

#[test]
fn presets_for_plugin_version_only_matches_plugin_and_major() {
    let mut bank = PresetBank::new("Factory");
    bank.add_preset(preset("Good", "sotf-eq", &[]));
    bank.add_preset(preset("BadPlugin", "sotf-compressor", &[]));
    let mut incompatible = preset("OtherMajor", "sotf-eq", &[]);
    incompatible.version = "9.8.7".into();
    bank.add_preset(incompatible);

    let mut good = preset("Another", "sotf-eq", &[]);
    good.version = "1.0.0".into();
    bank.add_preset(good);

    let mut names: Vec<_> = bank
        .presets_for_plugin_version("sotf-eq", "1.2.3")
        .into_iter()
        .map(|p| p.name.as_str())
        .collect();
    names.sort_unstable();

    assert_eq!(names, vec!["Another", "Good"]);
}

#[test]
fn preset_round_trips_external_plugin_state_data() {
    let descriptor = crate::external_plugin::PluginDescriptor {
        id: "com.example.delay".into(),
        name: "Example Delay".into(),
        vendor: "Example".into(),
        version: "1.0".into(),
        format: crate::external_plugin::PluginFormat::Clap,
        path: "/Library/Audio/Plug-Ins/CLAP/ExampleDelay.clap".into(),
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec!["delay".into()],
        scan_status: crate::external_plugin::PluginScanStatus::UnsupportedByBuild,
    };
    let state = ExternalPluginState::new(
        descriptor.clone(),
        crate::external_plugin::ExternalPluginSandboxMode::Isolated,
        vec![1, 3, 5, 8],
    );
    let mut preset = PluginPreset::new("External".into(), "external-plugin".into(), "1.0.0".into());

    preset.set_external_plugin_state(&state).unwrap();
    let json = serde_json::to_string(&preset).unwrap();
    let decoded: PluginPreset = serde_json::from_str(&json).unwrap();
    let restored = decoded.external_plugin_state().unwrap().unwrap();

    assert_eq!(restored.descriptor, descriptor);
    assert_eq!(
        restored.sandbox_mode,
        crate::external_plugin::ExternalPluginSandboxMode::Isolated
    );
    assert_eq!(restored.opaque_state, vec![1, 3, 5, 8]);
}

#[test]
fn preset_document_round_trips_parameter_boundaries_and_metadata() {
    let mut document = preset("Boundary", "sotf-test", &["qa"]);
    document
        .parameters
        .insert("minimum".into(), ParameterValue::Float(-24.0));
    document
        .parameters
        .insert("maximum".into(), ParameterValue::Int(512));
    document
        .parameters
        .insert("enabled".into(), ParameterValue::Bool(true));
    document.parameters.insert(
        "configuration".into(),
        ParameterValue::String("{\"mode\":\"linear\"}".into()),
    );
    document.set_author("SOTF QA");

    let json = serde_json::to_string(&document).unwrap();
    let restored: PluginPreset = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, document);
}

#[test]
fn preset_document_tolerates_unknown_fields_and_defaults_missing_metadata() {
    let json = serde_json::json!({
        "name": "Legacy",
        "plugin_id": "sotf-eq",
        "version": "1.0.0",
        "parameters": {},
        "data": {},
        "future_field": { "preserved_by_future_hosts": true }
    });

    let restored: PluginPreset = serde_json::from_value(json).unwrap();
    assert_eq!(restored.name, "Legacy");
    assert_eq!(restored.metadata, Default::default());
}

#[test]
fn preset_document_rejects_missing_required_fields_and_invalid_parameter_values() {
    let missing_parameters = serde_json::json!({
        "name": "Broken",
        "plugin_id": "sotf-eq",
        "version": "1.0.0",
        "data": {}
    });
    assert!(serde_json::from_value::<PluginPreset>(missing_parameters).is_err());

    let invalid_parameter = serde_json::json!({
        "name": "Broken",
        "plugin_id": "sotf-eq",
        "version": "1.0.0",
        "parameters": { "gain": { "Float": "not-a-number" } },
        "data": {}
    });
    assert!(serde_json::from_value::<PluginPreset>(invalid_parameter).is_err());
}

#[test]
fn preset_document_version_contract_accepts_same_major_only() {
    let document = PluginPreset::new("Versioned".into(), "sotf-eq".into(), "2.0.0".into());

    assert!(document.is_loadable_for("sotf-eq", "2.99.4"));
    assert!(!document.is_loadable_for("sotf-eq", "3.0.0"));
    assert!(!document.is_loadable_for("sotf-compressor", "2.0.0"));
}

#[test]
fn preset_rejects_inconsistent_external_plugin_descriptor_state() {
    let descriptor = crate::external_plugin::PluginDescriptor {
        id: "com.example.delay".into(),
        name: "Example Delay".into(),
        vendor: "Example".into(),
        version: "1.0".into(),
        format: crate::external_plugin::PluginFormat::Clap,
        path: "/Library/Audio/Plug-Ins/CLAP/ExampleDelay.clap".into(),
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec![],
        scan_status: crate::external_plugin::PluginScanStatus::UnsupportedByBuild,
    };
    let mut state = ExternalPluginState::new(
        descriptor,
        crate::external_plugin::ExternalPluginSandboxMode::Isolated,
        vec![1, 2, 3],
    );
    state.plugin_id = "com.example.different".into();
    let mut document =
        PluginPreset::new("External".into(), "external-plugin".into(), "1.0.0".into());

    assert!(document.set_external_plugin_state(&state).is_err());
}
