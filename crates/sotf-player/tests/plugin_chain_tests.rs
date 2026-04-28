//! Regression tests for PluginChain operations.
//!
//! Covers: move_plugin, insert_plugin, user_plugin_insert_index, is_permanent,
//! and PluginController integration.

use sotf_audio_player::{
    PluginChain, PluginController, PluginSettings, PluginType, PluginUpdateEffect, param_specs,
};

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
fn user_added_matrix_is_not_permanent() {
    let mut chain = PluginChain::with_default_rack();
    let insert_idx = chain.user_plugin_insert_index();
    chain.insert_plugin(insert_idx, &PluginType::Matrix);

    // The plugin at insert_idx is the newly inserted Matrix — should NOT be permanent
    assert!(
        !chain.plugins()[insert_idx].is_permanent(),
        "user-added Matrix plugin should not be permanent"
    );
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
    chain.insert_plugin(idx1, &PluginType::Gain);
    // After insert, the next insert point shifts by 1
    let idx2 = chain.user_plugin_insert_index();
    chain.insert_plugin(idx2, &PluginType::EQ);
    let idx3 = chain.user_plugin_insert_index();
    chain.insert_plugin(idx3, &PluginType::Compressor);

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
    chain.insert_plugin(idx_before, &PluginType::Gain);
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
    chain.insert_plugin(insert_idx, &PluginType::Gain);

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
