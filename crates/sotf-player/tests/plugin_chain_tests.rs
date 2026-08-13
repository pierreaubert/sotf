//! Regression tests for PluginChain operations.
//!
//! Covers: move_plugin, insert_plugin, user_plugin_insert_index, is_permanent,
//! and PluginController integration.

use sotf_audio_player::{
    PluginChain, PluginController, PluginGraph, PluginSettings, PluginType, PluginUpdateEffect,
    param_specs, resize_matrix,
};
use sotf_plugins::{
    ExternalPluginSandboxMode, ExternalPluginState, PluginDescriptor, PluginFormat,
    PluginScanStatus,
};

fn external_settings(
    scan_status: PluginScanStatus,
    is_instrument: bool,
) -> (tempfile::TempDir, PluginSettings) {
    external_settings_with_layout(
        scan_status,
        is_instrument,
        if is_instrument { 0 } else { 2 },
        4,
    )
}

fn external_settings_with_layout(
    scan_status: PluginScanStatus,
    is_instrument: bool,
    audio_inputs: usize,
    audio_outputs: usize,
) -> (tempfile::TempDir, PluginSettings) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.clap");
    std::fs::write(&path, b"fixture").unwrap();
    let descriptor = PluginDescriptor {
        id: "clap.test".into(),
        name: "Test Plug-in".into(),
        vendor: "SOTF".into(),
        version: "1.0".into(),
        format: PluginFormat::Clap,
        path,
        audio_inputs,
        audio_outputs,
        is_instrument,
        categories: vec!["Effect".into()],
        scan_status,
    };
    let state =
        ExternalPluginState::new(descriptor, ExternalPluginSandboxMode::Isolated, Vec::new());
    (dir, PluginSettings::External { state })
}

fn matrix_settings(input_channels: usize, output_channels: usize) -> PluginSettings {
    let mut settings = PluginSettings::default_for(&PluginType::Matrix).unwrap();
    let PluginSettings::Matrix {
        input_channels: current_input,
        output_channels: current_output,
        matrix,
        ..
    } = &mut settings
    else {
        unreachable!();
    };
    resize_matrix(
        matrix,
        *current_input,
        *current_output,
        input_channels,
        output_channels,
    );
    *current_input = input_channels;
    *current_output = output_channels;
    settings
}

#[test]
fn default_rack_has_permanent_plugins() {
    let chain = PluginChain::with_default_rack();
    let plugins = chain.plugins();
    assert!(
        plugins.len() >= 3,
        "default rack should have at least 3 plugins"
    );

    // First plugin (InputMonitor/Gain) should be permanent
    assert!(
        plugins[0].is_permanent(),
        "first plugin in default rack should be permanent"
    );

    // Last plugin (OutputMonitor) should be permanent
    assert!(
        plugins.last().unwrap().is_permanent(),
        "last plugin in default rack should be permanent"
    );
}

#[test]
fn user_plugin_insert_index_before_permanent_tail() {
    let chain = PluginChain::with_default_rack();
    let idx = chain.user_plugin_insert_index();

    // Insert index should be between permanent head and permanent tail
    assert!(
        idx > 0,
        "insert index should be after first permanent plugin"
    );
    assert!(
        idx < chain.plugins().len(),
        "insert index should be before last plugin"
    );

    // The plugin at insert_index should be permanent (we insert *before* it)
    let plugin_at_idx = &chain.plugins()[idx];
    assert!(
        plugin_at_idx.is_permanent(),
        "plugin at insert index should be permanent (we insert before it)"
    );
}

#[test]
fn insert_plugin_grows_chain() {
    let mut chain = PluginChain::with_default_rack();
    let initial_count = chain.plugins().len();
    let insert_idx = chain.user_plugin_insert_index();

    // insert_plugin returns the plugin ID, not the index
    let _id = chain.insert_plugin(insert_idx, &PluginType::Gain);

    assert_eq!(
        chain.plugins().len(),
        initial_count + 1,
        "chain should grow by 1"
    );

    // The plugin at insert_idx should be the newly inserted one (not permanent)
    assert!(
        !chain.plugins()[insert_idx].is_permanent(),
        "user-inserted plugin should not be permanent"
    );
    assert_eq!(
        chain.plugins()[insert_idx].plugin_type(),
        PluginType::Gain,
        "inserted plugin should be Gain"
    );
}

#[test]
fn add_binaural_decoder_does_not_require_removed_ui_params() {
    let mut controller = PluginController::new();

    let _ = controller.add_plugin(&PluginType::BinauralDecoder);
}

#[test]
fn controller_adds_concrete_external_settings_without_a_generic_default() {
    let (_dir, settings) = external_settings(PluginScanStatus::Loadable, false);
    let expected = serde_json::to_value(&settings).unwrap();
    let mut controller = PluginController::new();

    assert!(matches!(
        controller.add_plugin_settings(settings).unwrap(),
        PluginUpdateEffect::Structural
    ));

    let plugin = controller
        .graph
        .get_plugin(controller.selected_plugin_index)
        .expect("inserted external plugin");
    assert_eq!(plugin.plugin_type(), PluginType::External);
    assert_eq!(plugin.display_name(), "Test Plug-in");
    assert_eq!(serde_json::to_value(&plugin.settings).unwrap(), expected);
    let config = plugin.to_plugin_config(48_000.0).unwrap();
    assert_eq!(config.plugin_type, "external");
    assert_eq!(config.parameters["descriptor"]["audio_outputs"], 4);
    assert_eq!(config.parameters["_sotf_instance_id"], plugin.id);
}

#[test]
fn generic_external_construction_returns_an_actionable_error_without_mutation() {
    let error = PluginSettings::default_for(&PluginType::External).unwrap_err();
    assert!(error.contains("concrete discovered settings"), "{error}");

    let mut graph = PluginGraph::with_default_rack();
    let before = serde_json::to_value(&graph).unwrap();
    let error = graph.add_plugin(&PluginType::External).unwrap_err();
    assert!(error.contains("concrete discovered settings"), "{error}");
    assert_eq!(serde_json::to_value(&graph).unwrap(), before);
}

#[test]
fn controller_rejects_external_instruments_and_unloadable_scan_results() {
    let (_instrument_dir, instrument) = external_settings(PluginScanStatus::Loadable, true);
    let mut controller = PluginController::new();
    let error = controller.add_plugin_settings(instrument).unwrap_err();
    assert!(error.contains("instrument"), "{error}");

    let (_unsupported_dir, unsupported) =
        external_settings(PluginScanStatus::UnsupportedByBuild, false);
    let error = controller.add_plugin_settings(unsupported).unwrap_err();
    assert!(error.contains("not loadable"), "{error}");
}

#[test]
fn external_graph_json_round_trip_preserves_descriptor_state_topology_and_channels() {
    let (_dir, mut settings) = external_settings(PluginScanStatus::Loadable, false);
    let PluginSettings::External { state } = &mut settings else {
        unreachable!();
    };
    state.opaque_state = vec![0, 1, 2, 127, 128, 255];

    let mut controller = PluginController::new();
    controller.add_plugin_settings(settings).unwrap();
    let expected_json = serde_json::to_value(&controller.graph).unwrap();
    let restored: PluginGraph = serde_json::from_value(expected_json.clone()).unwrap();

    assert_eq!(serde_json::to_value(&restored).unwrap(), expected_json);
    assert_eq!(restored.compute_channel_flow(), (2, 4));
    assert_eq!(restored.output_channels(), 4);
    let external = restored
        .plugins_linear()
        .expect("restored graph remains a rack")
        .into_iter()
        .find(|node| node.plugin.plugin_type() == PluginType::External)
        .expect("external node survives graph restore");
    assert_eq!((external.input_channels, external.output_channels), (2, 4));
    let PluginSettings::External { state } = &external.plugin.settings else {
        unreachable!();
    };
    assert_eq!(state.descriptor.id, "clap.test");
    assert_eq!(state.opaque_state, [0, 1, 2, 127, 128, 255]);
    assert_eq!(state.sandbox_mode, ExternalPluginSandboxMode::Isolated);

    let config = external.plugin.to_plugin_config(48_000.0).unwrap();
    assert_eq!(config.parameters["descriptor"]["audio_inputs"], 2);
    assert_eq!(config.parameters["descriptor"]["audio_outputs"], 4);
    assert_eq!(config.parameters["isolated"], true);
}

#[test]
fn controller_rejects_external_plugin_when_saved_path_is_stale() {
    let (dir, settings) = external_settings(PluginScanStatus::Loadable, false);
    drop(dir);

    let error = PluginController::new()
        .add_plugin_settings(settings)
        .unwrap_err();
    assert!(error.contains("does not exist"), "{error}");
}

#[test]
fn preset_load_revalidates_external_plugin_paths_before_commit() {
    let (plugin_dir, settings) = external_settings(PluginScanStatus::Loadable, false);
    let preset_dir = tempfile::tempdir().unwrap();
    let mut controller = PluginController::new();
    controller.add_plugin_settings(settings).unwrap();
    controller
        .graph
        .save_to_file(preset_dir.path(), "external")
        .unwrap();

    let mut restored = PluginGraph::new();
    assert!(
        restored
            .load_from_file(preset_dir.path(), "external")
            .unwrap()
            .is_empty()
    );
    assert!(
        restored
            .plugins_linear()
            .unwrap()
            .iter()
            .any(|node| node.plugin.plugin_type() == PluginType::External)
    );

    drop(plugin_dir);
    let warnings = restored
        .load_from_file(preset_dir.path(), "external")
        .unwrap();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("does not exist"), "{}", warnings[0]);
    assert!(
        !restored
            .plugins_linear()
            .unwrap()
            .iter()
            .any(|node| node.plugin.plugin_type() == PluginType::External)
    );
}

#[test]
fn controller_rejects_non_isolated_external_plugin_state() {
    let (_dir, mut settings) = external_settings(PluginScanStatus::Loadable, false);
    let PluginSettings::External { state } = &mut settings else {
        unreachable!();
    };
    state.sandbox_mode = ExternalPluginSandboxMode::InProcess;

    let error = PluginController::new()
        .add_plugin_settings(settings)
        .unwrap_err();
    assert!(error.contains("isolated"), "{error}");
}

#[test]
fn controller_rejects_external_plugin_with_incompatible_rack_input_layout() {
    let (_dir, mut settings) = external_settings(PluginScanStatus::Loadable, false);
    let PluginSettings::External { state } = &mut settings else {
        unreachable!();
    };
    state.descriptor.audio_inputs = 1;

    let error = PluginController::new()
        .add_plugin_settings(settings)
        .unwrap_err();
    assert!(error.contains("requires 1 input channel"), "{error}");
    assert!(error.contains("provides 2"), "{error}");
}

#[test]
fn external_output_width_is_used_for_downstream_channel_conflicts() {
    let (_dir, external) = external_settings(PluginScanStatus::Loadable, false);
    let mut binaural = PluginSettings::default_for(&PluginType::BinauralDecoder).unwrap();
    let PluginSettings::BinauralDecoder { input_channels, .. } = &mut binaural else {
        unreachable!();
    };
    *input_channels = 4;

    let mut controller = PluginController::new();
    controller.add_plugin_settings(external).unwrap();
    controller.add_plugin_settings(binaural).unwrap();

    assert_eq!(controller.graph.compute_channel_flow(), (2, 2));
    assert!(controller.graph.find_channel_conflicts(2).is_empty());

    controller.suspend_incompatible(2);
    assert!(!controller.has_suspensions());
    let nodes = controller.graph.plugins_linear().unwrap();
    let external = nodes
        .iter()
        .find(|node| node.plugin.plugin_type() == PluginType::External)
        .unwrap();
    let binaural = nodes
        .iter()
        .find(|node| node.plugin.plugin_type() == PluginType::BinauralDecoder)
        .unwrap();
    assert_eq!((external.input_channels, external.output_channels), (2, 4));
    assert_eq!((binaural.input_channels, binaural.output_channels), (4, 2));
}

#[test]
fn incompatible_external_reorder_is_rejected_without_graph_mutation() {
    let (_external_dir, external) =
        external_settings_with_layout(PluginScanStatus::Loadable, false, 2, 2);
    let mut graph = PluginGraph::with_default_rack();
    let external_index = graph.user_plugin_insert_index();
    graph
        .insert_plugin_settings(external_index, external)
        .unwrap();
    let matrix_index = graph.user_plugin_insert_index();
    graph
        .insert_plugin_settings(matrix_index, matrix_settings(2, 4))
        .unwrap();
    let before = serde_json::to_value(&graph).unwrap();

    let error = graph.move_plugin(external_index, matrix_index).unwrap_err();

    assert!(error.contains("Test Plug-in"), "{error}");
    assert!(error.contains("requires 2 input channel"), "{error}");
    assert!(error.contains("upstream provides 4"), "{error}");
    assert_eq!(serde_json::to_value(&graph).unwrap(), before);
}

#[test]
fn incompatible_external_settings_replacement_preserves_old_settings() {
    let (_external_dir, external) =
        external_settings_with_layout(PluginScanStatus::Loadable, false, 2, 2);
    let (_replacement_dir, replacement) =
        external_settings_with_layout(PluginScanStatus::Loadable, false, 1, 2);
    let mut graph = PluginGraph::with_default_rack();
    let external_index = graph.user_plugin_insert_index();
    graph
        .insert_plugin_settings(external_index, external)
        .unwrap();
    let before = serde_json::to_value(&graph).unwrap();

    let error = graph
        .set_plugin_settings_by_index(external_index, replacement)
        .unwrap_err();

    assert!(error.contains("requires 1 input channel"), "{error}");
    assert!(error.contains("upstream provides 2"), "{error}");
    assert_eq!(serde_json::to_value(&graph).unwrap(), before);
}

#[test]
fn enabling_disabled_incompatible_external_is_rejected_and_stays_disabled() {
    let (_external_dir, external) =
        external_settings_with_layout(PluginScanStatus::Loadable, false, 2, 2);
    let (_replacement_dir, replacement) =
        external_settings_with_layout(PluginScanStatus::Loadable, false, 1, 2);
    let mut graph = PluginGraph::with_default_rack();
    let external_index = graph.user_plugin_insert_index();
    graph
        .insert_plugin_settings(external_index, external)
        .unwrap();
    graph.toggle_plugin_by_index(external_index).unwrap();
    graph
        .set_plugin_settings_by_index(external_index, replacement)
        .unwrap();
    assert!(graph.find_channel_conflicts(2).is_empty());
    let before_enable = serde_json::to_value(&graph).unwrap();

    let error = graph.toggle_plugin_by_index(external_index).unwrap_err();

    assert!(error.contains("requires 1 input channel"), "{error}");
    assert_eq!(serde_json::to_value(&graph).unwrap(), before_enable);
    assert!(!graph.get_plugin(external_index).unwrap().enabled);
}

#[test]
fn incompatible_preset_chain_is_rejected_without_replacing_current_graph() {
    let (_external_dir, external) =
        external_settings_with_layout(PluginScanStatus::Loadable, false, 2, 2);
    let mut source = PluginGraph::with_default_rack();
    let external_index = source.user_plugin_insert_index();
    source
        .insert_plugin_settings(external_index, external)
        .unwrap();
    let matrix_index = source.user_plugin_insert_index();
    source
        .insert_plugin_settings(matrix_index, matrix_settings(2, 4))
        .unwrap();

    let mut plugins: Vec<serde_json::Value> = source
        .plugins()
        .into_iter()
        .map(|plugin| serde_json::to_value(plugin).unwrap())
        .collect();
    plugins.swap(external_index, matrix_index);
    let preset_dir = tempfile::tempdir().unwrap();
    let preset_path = preset_dir.path().join("incompatible.json");
    std::fs::write(
        &preset_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "version": 2,
            "plugins": plugins,
        }))
        .unwrap(),
    )
    .unwrap();

    let mut current = PluginGraph::with_default_rack();
    current.add_plugin(&PluginType::Gain).unwrap();
    let before = serde_json::to_value(&current).unwrap();

    let error = current
        .load_from_file(preset_dir.path(), "incompatible")
        .unwrap_err()
        .to_string();

    assert!(error.contains("Test Plug-in"), "{error}");
    assert!(error.contains("upstream provides 4"), "{error}");
    assert_eq!(serde_json::to_value(&current).unwrap(), before);
}

#[test]
fn user_added_matrix_is_not_permanent() {
    let mut chain = PluginChain::with_default_rack();
    let insert_idx = chain.user_plugin_insert_index();
    chain
        .insert_plugin(insert_idx, &PluginType::Matrix)
        .unwrap();

    // The plugin at insert_idx is the newly inserted Matrix — should NOT be permanent
    assert!(
        !chain.plugins()[insert_idx].is_permanent(),
        "user-added Matrix plugin should not be permanent"
    );
}

#[test]
fn adapting_to_mono_updates_the_graph_contract_and_allows_mono_to_stereo() {
    let mut graph = PluginGraph::with_default_rack();

    graph.adapt_matrix_to_input(1);

    assert_eq!(graph.input_channel_count(), 1);
    let insert_idx = graph.user_plugin_insert_index();
    graph
        .insert_plugin(insert_idx, &PluginType::MonoToStereo)
        .unwrap();
    assert_eq!(graph.output_channels(), 2);
    assert!(graph.find_channel_conflicts(1).is_empty());
}

#[test]
fn upmixer_binaural_preview_resizes_graph_matrix_to_stereo() {
    let mut controller = PluginController::new();
    controller.add_plugin(&PluginType::Upmixer);
    let upmixer_idx = controller
        .graph
        .find_plugin_index(&PluginType::Upmixer)
        .expect("upmixer should be present");
    let preview_idx = param_specs::index_of(param_specs::upmixer::PARAMS, "binaural_preview");

    let effect = controller.set_plugin_param(upmixer_idx, preview_idx, 1.0);

    assert!(matches!(effect, PluginUpdateEffect::Structural));
    assert_eq!(controller.graph.output_channels(), 2);

    let upmixer_node = controller
        .graph
        .plugins_linear()
        .expect("default graph should be linear")
        .into_iter()
        .find(|node| node.plugin.plugin_type() == PluginType::Upmixer)
        .expect("upmixer should be present");
    assert_eq!(upmixer_node.output_channels, 2);

    let matrix = controller
        .graph
        .plugins_linear()
        .expect("default graph should be linear")
        .into_iter()
        .find(|node| node.plugin.plugin_type() == PluginType::Matrix)
        .expect("matrix should be present");

    match &matrix.plugin.settings {
        PluginSettings::Matrix {
            input_channels,
            output_channels,
            ..
        } => {
            assert_eq!(*input_channels, 2);
            assert_eq!(*output_channels, 2);
        }
        _ => panic!("expected matrix settings"),
    }
}

#[test]
fn move_plugin_between_user_plugins() {
    let mut chain = PluginChain::with_default_rack();

    // Add 3 user plugins at the insert point
    let idx1 = chain.user_plugin_insert_index();
    chain.insert_plugin(idx1, &PluginType::Gain).unwrap();
    // After insert, the next insert point shifts by 1
    let idx2 = chain.user_plugin_insert_index();
    chain.insert_plugin(idx2, &PluginType::EQ).unwrap();
    let idx3 = chain.user_plugin_insert_index();
    chain.insert_plugin(idx3, &PluginType::Compressor).unwrap();

    // Verify all 3 are non-permanent
    assert!(!chain.plugins()[idx1].is_permanent());
    assert!(!chain.plugins()[idx1 + 1].is_permanent());
    assert!(!chain.plugins()[idx1 + 2].is_permanent());

    // Move the first user plugin down by 1
    let first_type = chain.plugins()[idx1].plugin_type();
    chain.move_plugin(idx1, idx1 + 1);

    // The plugin that was at idx1 should now be at idx1+1
    assert_eq!(
        chain.plugins()[idx1 + 1].plugin_type(),
        first_type,
        "plugin should have moved down"
    );
}

#[test]
fn cannot_move_permanent_plugins() {
    let chain = PluginChain::with_default_rack();

    // First plugin is permanent — cannot move down
    assert!(
        !chain.can_move_plugin_down(0),
        "permanent first plugin should not be movable down"
    );

    // Last plugin is permanent — cannot move up
    let last_idx = chain.plugins().len() - 1;
    assert!(
        !chain.can_move_plugin_up(last_idx),
        "permanent last plugin should not be movable up"
    );
}

#[test]
fn insert_index_shifts_after_insert() {
    let mut chain = PluginChain::with_default_rack();
    let idx_before = chain.user_plugin_insert_index();
    chain.insert_plugin(idx_before, &PluginType::Gain).unwrap();
    let idx_after = chain.user_plugin_insert_index();

    assert_eq!(
        idx_after,
        idx_before + 1,
        "insert index should shift by 1 after inserting"
    );
}

#[test]
fn remove_plugin_does_not_affect_permanent() {
    let mut chain = PluginChain::with_default_rack();
    let insert_idx = chain.user_plugin_insert_index();
    chain.insert_plugin(insert_idx, &PluginType::Gain).unwrap();

    let initial_permanent_count = chain.plugins().iter().filter(|p| p.is_permanent()).count();

    // Remove the user-added plugin
    chain.remove_plugin(insert_idx);

    let after_permanent_count = chain.plugins().iter().filter(|p| p.is_permanent()).count();
    assert_eq!(
        initial_permanent_count, after_permanent_count,
        "removing a user plugin should not affect permanent plugins"
    );
}

#[test]
fn library_clear_all_filters() {
    use sotf_audio_player::controllers::library::LibraryController;

    let mut controller = LibraryController::default();
    controller.selected_genre = Some("Rock".to_string());
    controller.selected_year = Some(2020);

    assert!(controller.has_active_filters());

    controller.clear_all_filters();

    assert!(!controller.has_active_filters());
    assert!(controller.selected_genre.is_none());
    assert!(controller.selected_year.is_none());
}
