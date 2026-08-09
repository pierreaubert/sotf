use super::plugin_controller::PluginController;
use super::types::{EqEditTarget, PluginUpdateEffect};
use crate::plugin_graph::PluginGraph;
use crate::{PluginSettings, PluginType};

use crate::plugin_graph::NodePosition;

/// Build a controller with a single non-permanent EQ plugin and return
/// its graph node id. Mirrors how the room-EQ-as-graph apply path leaves
/// state when the user double-clicks a plugin in the graph view.
fn make_controller_with_eq() -> (PluginController, crate::plugin_graph::GraphNodeId) {
    let mut ctrl = PluginController::new();
    ctrl.graph = PluginGraph::new();
    let node_id = ctrl
        .graph
        .add_plugin_node(&PluginType::EQ, NodePosition::new(0.0, 0.0))
        .unwrap();
    // The EQ plugin defaults to a non-empty filter list — we only need
    // to know how many bands it starts with.
    (ctrl, node_id)
}

fn eq_filter_count(ctrl: &PluginController, id: crate::plugin_graph::GraphNodeId) -> usize {
    let node = ctrl.graph.nodes.get(&id).unwrap();
    match &node.plugin.settings {
        PluginSettings::EQ { filters, .. } => filters.len(),
        other => panic!("expected EQ settings, got {:?}", other),
    }
}

#[test]
fn add_eq_band_by_node_id_appends_a_filter() {
    let (mut ctrl, id) = make_controller_with_eq();
    let before = eq_filter_count(&ctrl, id);
    let effect = ctrl.add_eq_band_by_node_id(id).expect("add succeeds");
    assert!(matches!(effect, PluginUpdateEffect::Structural));
    assert_eq!(eq_filter_count(&ctrl, id), before + 1);
}

#[test]
fn add_eq_band_by_node_id_rejects_unknown_node() {
    let (mut ctrl, _) = make_controller_with_eq();
    let bogus = crate::plugin_graph::GraphNodeId::new_v4();
    assert!(ctrl.add_eq_band_by_node_id(bogus).is_err());
}

#[test]
fn add_eq_band_by_node_id_rejects_non_eq_plugin() {
    let mut ctrl = PluginController::new();
    ctrl.graph = PluginGraph::new();
    let id = ctrl
        .graph
        .add_plugin_node(&PluginType::Gain, NodePosition::new(0.0, 0.0))
        .unwrap();
    assert!(ctrl.add_eq_band_by_node_id(id).is_err());
}

#[test]
fn remove_eq_band_by_node_id_drops_the_band() {
    let (mut ctrl, id) = make_controller_with_eq();
    // Ensure the EQ has at least one band to remove.
    ctrl.add_eq_band_by_node_id(id).unwrap();
    let before = eq_filter_count(&ctrl, id);
    ctrl.remove_eq_band_by_node_id(id, before - 1).unwrap();
    assert_eq!(eq_filter_count(&ctrl, id), before - 1);
}

#[test]
fn toggle_eq_band_mute_by_node_id_flips_the_flag() {
    let (mut ctrl, id) = make_controller_with_eq();
    ctrl.add_eq_band_by_node_id(id).unwrap();
    let band = eq_filter_count(&ctrl, id) - 1;

    let initial = match &ctrl.graph.nodes.get(&id).unwrap().plugin.settings {
        PluginSettings::EQ { filters, .. } => filters[band].muted,
        _ => unreachable!(),
    };
    ctrl.toggle_eq_band_mute_by_node_id(id, band).unwrap();
    let after = match &ctrl.graph.nodes.get(&id).unwrap().plugin.settings {
        PluginSettings::EQ { filters, .. } => filters[band].muted,
        _ => unreachable!(),
    };
    assert_eq!(after, !initial);
}

#[test]
fn toggle_eq_band_solo_by_node_id_flips_the_flag() {
    let (mut ctrl, id) = make_controller_with_eq();
    ctrl.add_eq_band_by_node_id(id).unwrap();
    let band = eq_filter_count(&ctrl, id) - 1;

    let initial = match &ctrl.graph.nodes.get(&id).unwrap().plugin.settings {
        PluginSettings::EQ { filters, .. } => filters[band].solo,
        _ => unreachable!(),
    };
    ctrl.toggle_eq_band_solo_by_node_id(id, band).unwrap();
    let after = match &ctrl.graph.nodes.get(&id).unwrap().plugin.settings {
        PluginSettings::EQ { filters, .. } => filters[band].solo,
        _ => unreachable!(),
    };
    assert_eq!(after, !initial);
}

#[test]
fn eq_band_by_node_id_does_not_affect_sibling_node() {
    // Two EQ nodes in the graph; mutating one via node-id must leave
    // the other untouched.
    let mut ctrl = PluginController::new();
    ctrl.graph = PluginGraph::new();
    let a = ctrl
        .graph
        .add_plugin_node(&PluginType::EQ, NodePosition::new(0.0, 0.0))
        .unwrap();
    let b = ctrl
        .graph
        .add_plugin_node(&PluginType::EQ, NodePosition::new(100.0, 0.0))
        .unwrap();
    let a_before = eq_filter_count(&ctrl, a);
    let b_before = eq_filter_count(&ctrl, b);
    ctrl.add_eq_band_by_node_id(a).unwrap();
    assert_eq!(eq_filter_count(&ctrl, a), a_before + 1);
    assert_eq!(eq_filter_count(&ctrl, b), b_before);
}

/// Construct a controller with a linear-rack-friendly EQ instance and
/// return both its linear index and the band count.
fn make_linear_eq() -> (PluginController, usize) {
    let mut ctrl = PluginController::new();
    // PluginController::new() starts with the default rack (a linear
    // chain). Add an EQ via the same helper the UI uses so it's in the
    // user portion of the rack and addressable by linear index.
    let _ = ctrl.add_plugin(&PluginType::EQ);
    let idx = ctrl.selected_plugin_index;
    (ctrl, idx)
}

#[test]
fn per_channel_eq_target_isolated_and_copy_actions_are_explicit() {
    let (mut ctrl, idx) = make_linear_eq();
    ctrl.set_eq_per_channel_mode(idx, true);

    let original_global = match &ctrl.graph.get_plugin(idx).unwrap().settings {
        PluginSettings::EQ { filters, .. } => filters[0].frequency,
        _ => unreachable!(),
    };
    let effect = ctrl.set_eq_param(idx, EqEditTarget::Channel(0), 0, 2_500.0);
    assert!(matches!(effect, PluginUpdateEffect::Structural));

    let plugin = ctrl.graph.get_plugin(idx).unwrap();
    let PluginSettings::EQ {
        filters,
        channel_filters,
        ..
    } = &plugin.settings
    else {
        unreachable!()
    };
    let channels = channel_filters.as_ref().unwrap();
    assert_eq!(filters[0].frequency, original_global);
    assert_eq!(channels[0][0].frequency, 2_500.0);
    assert_eq!(channels[1][0].frequency, original_global);

    ctrl.copy_eq_channel_to_all(idx, 0).unwrap();
    let PluginSettings::EQ {
        channel_filters, ..
    } = &ctrl.graph.get_plugin(idx).unwrap().settings
    else {
        unreachable!()
    };
    assert!(
        channel_filters
            .as_ref()
            .unwrap()
            .iter()
            .all(|channel| channel[0].frequency == 2_500.0)
    );

    ctrl.copy_eq_global_to_channel(idx, 0).unwrap();
    let PluginSettings::EQ {
        channel_filters, ..
    } = &ctrl.graph.get_plugin(idx).unwrap().settings
    else {
        unreachable!()
    };
    assert_eq!(
        channel_filters.as_ref().unwrap()[0][0].frequency,
        original_global
    );
}

#[test]
fn per_channel_eq_topology_edit_does_not_touch_global_or_siblings() {
    use sotf_audio::plugins::eq::EqFilterTopology;
    let (mut ctrl, idx) = make_linear_eq();
    ctrl.set_eq_per_channel_mode(idx, true);
    ctrl.cycle_eq_filter_topology_for_target(idx, EqEditTarget::Channel(0), 0);

    let PluginSettings::EQ {
        filters,
        channel_filters,
        ..
    } = &ctrl.graph.get_plugin(idx).unwrap().settings
    else {
        unreachable!()
    };
    let channels = channel_filters.as_ref().unwrap();
    assert_eq!(filters[0].topology, EqFilterTopology::Biquad);
    assert_eq!(channels[0][0].topology, EqFilterTopology::WarpedBiquad);
    assert_eq!(channels[1][0].topology, EqFilterTopology::Biquad);
}

#[test]
fn per_channel_eq_reset_uses_dynamic_band_defaults() {
    let (mut ctrl, idx) = make_linear_eq();
    ctrl.set_eq_per_channel_mode(idx, true);
    ctrl.set_eq_param(idx, EqEditTarget::Channel(0), 0, 2_500.0);

    let effect = ctrl.reset_eq_param(idx, EqEditTarget::Channel(0), 0);
    assert!(matches!(effect, PluginUpdateEffect::Structural));

    let PluginSettings::EQ {
        filters,
        channel_filters,
        ..
    } = &ctrl.graph.get_plugin(idx).unwrap().settings
    else {
        unreachable!()
    };
    let channels = channel_filters.as_ref().unwrap();
    assert_eq!(channels[0][0].frequency, filters[0].frequency);
    assert_eq!(channels[1][0].frequency, filters[0].frequency);
}

fn topology_at(
    ctrl: &PluginController,
    idx: usize,
    band: usize,
) -> sotf_audio::plugins::eq::EqFilterTopology {
    match &ctrl.graph.get_plugin(idx).unwrap().settings {
        PluginSettings::EQ { filters, .. } => filters[band].topology,
        other => panic!("expected EQ settings, got {:?}", other),
    }
}

#[test]
fn cycle_eq_filter_topology_walks_biquad_warped_kautz() {
    use sotf_audio::plugins::eq::EqFilterTopology;
    let (mut ctrl, idx) = make_linear_eq();
    let band = 0;
    assert_eq!(topology_at(&ctrl, idx, band), EqFilterTopology::Biquad);

    let effect = ctrl.cycle_eq_filter_topology(idx, band);
    assert!(matches!(effect, PluginUpdateEffect::Structural));
    assert_eq!(
        topology_at(&ctrl, idx, band),
        EqFilterTopology::WarpedBiquad
    );

    ctrl.cycle_eq_filter_topology(idx, band);
    assert_eq!(topology_at(&ctrl, idx, band), EqFilterTopology::KautzFilter);

    ctrl.cycle_eq_filter_topology(idx, band);
    assert_eq!(topology_at(&ctrl, idx, band), EqFilterTopology::Biquad);
}

#[test]
fn cycle_eq_filter_topology_keeps_per_channel_filters_in_sync() {
    // Regression test for the bug where per-channel filters cycled
    // independently of the global slot, leaving them out of sync.
    use sotf_audio::plugins::eq::EqFilterTopology;
    let (mut ctrl, idx) = make_linear_eq();

    // Seed per-channel filters from the current globals.
    {
        let plugin = ctrl.graph.get_plugin_mut(idx).unwrap();
        if let PluginSettings::EQ {
            filters,
            channel_filters,
            ..
        } = &mut plugin.settings
        {
            *channel_filters = Some(vec![filters.clone(), filters.clone()]);
        }
    }

    ctrl.cycle_eq_filter_topology(idx, 0);

    let plugin = ctrl.graph.get_plugin(idx).unwrap();
    let PluginSettings::EQ {
        filters,
        channel_filters,
        ..
    } = &plugin.settings
    else {
        panic!("expected EQ settings");
    };
    assert_eq!(filters[0].topology, EqFilterTopology::WarpedBiquad);
    for ch in channel_filters.as_ref().expect("channel_filters set") {
        assert_eq!(ch[0].topology, EqFilterTopology::WarpedBiquad);
    }
}

#[test]
fn cycle_eq_filter_lambda_only_walks_when_warped() {
    let (mut ctrl, idx) = make_linear_eq();

    // Biquad band → no-op.
    let effect = ctrl.cycle_eq_filter_lambda(idx, 0);
    assert!(matches!(effect, PluginUpdateEffect::None));

    // Switch to warped, then cycle through lambda presets.
    ctrl.cycle_eq_filter_topology(idx, 0);
    let lambda_at = |ctrl: &PluginController| {
        let plugin = ctrl.graph.get_plugin(idx).unwrap();
        let PluginSettings::EQ { filters, .. } = &plugin.settings else {
            unreachable!()
        };
        filters[0].lambda
    };
    assert_eq!(lambda_at(&ctrl), None);
    ctrl.cycle_eq_filter_lambda(idx, 0);
    assert_eq!(lambda_at(&ctrl), Some(0.4));
    ctrl.cycle_eq_filter_lambda(idx, 0);
    assert_eq!(lambda_at(&ctrl), Some(0.6));
    ctrl.cycle_eq_filter_lambda(idx, 0);
    assert_eq!(lambda_at(&ctrl), Some(0.8));
    ctrl.cycle_eq_filter_lambda(idx, 0);
    assert_eq!(lambda_at(&ctrl), None);
}

/// Lambda values imported from JSON between the preset stops still walk
/// to the next preset — regression for the original strict `<` cycle
/// which skipped over 0.6 when starting from 0.5 or 0.55.
#[test]
fn cycle_eq_filter_lambda_snaps_off_preset_imports() {
    let (mut ctrl, idx) = make_linear_eq();
    ctrl.cycle_eq_filter_topology(idx, 0);

    let set_lambda = |ctrl: &mut PluginController, v: f64| {
        let plugin = ctrl.graph.get_plugin_mut(idx).unwrap();
        if let PluginSettings::EQ { filters, .. } = &mut plugin.settings {
            filters[0].lambda = Some(v);
        }
    };
    let lambda_at = |ctrl: &PluginController| {
        let plugin = ctrl.graph.get_plugin(idx).unwrap();
        let PluginSettings::EQ { filters, .. } = &plugin.settings else {
            unreachable!()
        };
        filters[0].lambda
    };

    // Imported as 0.5 — should snap up to the next preset (0.6), not
    // skip it and jump straight to 0.8.
    set_lambda(&mut ctrl, 0.5);
    ctrl.cycle_eq_filter_lambda(idx, 0);
    assert_eq!(lambda_at(&ctrl), Some(0.6));

    // Imported as 0.55 — same snap behaviour.
    set_lambda(&mut ctrl, 0.55);
    ctrl.cycle_eq_filter_lambda(idx, 0);
    assert_eq!(lambda_at(&ctrl), Some(0.6));

    // Imported as 0.7 — the next step is the last preset (0.8), not None.
    set_lambda(&mut ctrl, 0.7);
    ctrl.cycle_eq_filter_lambda(idx, 0);
    assert_eq!(lambda_at(&ctrl), Some(0.8));
}

#[test]
fn add_and_pop_eq_kautz_section() {
    let (mut ctrl, idx) = make_linear_eq();

    // Not Kautz yet → both calls are no-ops.
    assert!(matches!(
        ctrl.add_eq_kautz_section(idx, 0, 80.0, 10.0, -2.0),
        PluginUpdateEffect::None
    ));
    assert!(matches!(
        ctrl.pop_eq_kautz_section(idx, 0),
        PluginUpdateEffect::None
    ));

    // Switch to Kautz topology.
    ctrl.cycle_eq_filter_topology(idx, 0);
    ctrl.cycle_eq_filter_topology(idx, 0);
    let kautz_count = |ctrl: &PluginController| {
        let plugin = ctrl.graph.get_plugin(idx).unwrap();
        let PluginSettings::EQ { filters, .. } = &plugin.settings else {
            unreachable!()
        };
        filters[0].kautz_sections.len()
    };

    let before = kautz_count(&ctrl);
    let effect = ctrl.add_eq_kautz_section(idx, 0, 80.0, 10.0, -2.0);
    assert!(matches!(effect, PluginUpdateEffect::Structural));
    assert_eq!(kautz_count(&ctrl), before + 1);

    let effect = ctrl.pop_eq_kautz_section(idx, 0);
    assert!(matches!(effect, PluginUpdateEffect::Structural));
    assert_eq!(kautz_count(&ctrl), before);
}

#[test]
fn cycle_eq_filter_topology_no_op_when_no_band() {
    let (mut ctrl, idx) = make_linear_eq();

    // Empty the filter list first
    if let Some(plugin) = ctrl.graph.get_plugin_mut(idx) {
        if let PluginSettings::EQ { filters, .. } = &mut plugin.settings {
            filters.clear();
        }
    }

    let effect = ctrl.cycle_eq_filter_topology(idx, 0);
    assert!(matches!(effect, PluginUpdateEffect::None));
}

#[test]
fn cycle_eq_filter_lambda_no_op_when_not_warped() {
    let (mut ctrl, idx) = make_linear_eq();

    // Default is Biquad, not warped → no-op
    let effect = ctrl.cycle_eq_filter_lambda(idx, 0);
    assert!(matches!(effect, PluginUpdateEffect::None));
}

#[test]
fn cycle_eq_filter_lambda_no_op_when_band_missing() {
    let (mut ctrl, idx) = make_linear_eq();

    let effect = ctrl.cycle_eq_filter_lambda(idx, 999);
    assert!(matches!(effect, PluginUpdateEffect::None));
}

#[test]
fn move_user_plugin_by_index_swaps_two_user_plugins() {
    let mut ctrl = PluginController::new();
    let _ = ctrl.add_plugin(&PluginType::EQ);
    let _ = ctrl.add_plugin(&PluginType::Compressor);

    let order_before: Vec<String> = ctrl
        .graph
        .plugins()
        .iter()
        .map(|p| p.display_name().to_string())
        .collect();

    // Move first user plugin down to second user plugin position
    ctrl.graph.move_plugin(2, 3).unwrap();

    let order_after: Vec<String> = ctrl
        .graph
        .plugins()
        .iter()
        .map(|p| p.display_name().to_string())
        .collect();
    assert_ne!(order_before, order_after);
    assert_eq!(order_after[2], order_before[3]);
    assert_eq!(order_after[3], order_before[2]);
}

#[test]
fn ab_config_source_path_persists_for_linear_and_node_addressing() {
    const PATH_A_IDX: usize = sotf_plugins::param_specs::index_of(
        sotf_plugins::param_specs::ab_compare::PARAMS,
        "path_a_config",
    );
    const PATH_B_IDX: usize = sotf_plugins::param_specs::index_of(
        sotf_plugins::param_specs::ab_compare::PARAMS,
        "path_b_config",
    );

    let mut ctrl = PluginController::new();
    let linear_idx = ctrl.graph.add_plugin(&PluginType::ABCompare).unwrap();
    assert!(matches!(
        ctrl.set_ab_compare_config_file(
            linear_idx,
            PATH_B_IDX,
            "{\"plugins\":[]}".to_string(),
            "/tmp/path-b.json".to_string(),
        ),
        PluginUpdateEffect::Structural
    ));
    let linear = ctrl.graph.get_plugin(linear_idx).unwrap();
    assert_eq!(
        linear.settings.param_value_string(PATH_B_IDX).as_deref(),
        Some("/tmp/path-b.json")
    );

    ctrl.graph = PluginGraph::new();
    let target = ctrl
        .graph
        .add_plugin_node(&PluginType::ABCompare, NodePosition::new(0.0, 0.0))
        .unwrap();
    let sibling = ctrl
        .graph
        .add_plugin_node(&PluginType::ABCompare, NodePosition::new(100.0, 0.0))
        .unwrap();
    ctrl.set_ab_compare_config_file_by_node_id(
        target,
        PATH_A_IDX,
        "{\"plugins\":[]}".to_string(),
        "/tmp/path-a.json".to_string(),
    );

    let target_settings = &ctrl.graph.nodes[&target].plugin.settings;
    assert_eq!(
        target_settings.param_value_string(PATH_A_IDX).as_deref(),
        Some("/tmp/path-a.json")
    );
    assert_eq!(
        ctrl.graph.nodes[&sibling]
            .plugin
            .settings
            .param_value_string(PATH_A_IDX)
            .as_deref(),
        Some("")
    );

    let json = serde_json::to_string(target_settings).unwrap();
    let reloaded: PluginSettings = serde_json::from_str(&json).unwrap();
    assert_eq!(
        reloaded.param_value_string(PATH_A_IDX).as_deref(),
        Some("/tmp/path-a.json"),
        "the selected basename/path must survive settings reload"
    );
}

#[cfg(feature = "dev-api")]
mod plugin_action_tests {
    use super::super::dev_api::actions::plugin_action;
    use crate::plugin_graph::PluginGraph;
    use serde_json::json;

    #[test]
    fn plugin_action_add_and_remove() {
        let mut graph = PluginGraph::with_default_rack();
        let before = graph.len();
        let idx = graph.user_plugin_insert_index();
        plugin_action(&mut graph, "PluginAdd", Some(json!({"plugin_type":"Gain"}))).unwrap();
        assert_eq!(graph.len(), before + 1);
        plugin_action(&mut graph, "PluginRemove", Some(json!({"index": idx}))).unwrap();
        assert_eq!(graph.len(), before);
    }
}

#[cfg(feature = "dev-api")]
mod plugin_query_tests {
    use super::super::dev_api::queries::plugin_query;
    use crate::plugin_graph::PluginGraph;

    #[test]
    fn plugin_query_count_empty_graph() {
        let graph = PluginGraph::with_default_rack();
        let value = plugin_query(&graph, "plugins.count").unwrap();
        assert!(value.as_u64().unwrap() > 0); // default rack has permanent nodes
    }

    #[test]
    fn plugin_query_list_contains_default_rack_plugins() {
        let graph = PluginGraph::with_default_rack();
        let value = plugin_query(&graph, "plugins.list").unwrap();
        let list = value.as_array().unwrap();
        assert_eq!(list.len(), graph.len());
        assert_eq!(list[0]["type"], "Loudness Monitor");
        assert_eq!(list[1]["type"], "Gain");
    }

    #[test]
    fn plugin_query_plugin_type() {
        let graph = PluginGraph::with_default_rack();
        let value = plugin_query(&graph, "plugins.plugin.1.type").unwrap();
        assert_eq!(value.as_str().unwrap(), "Gain");
    }

    #[test]
    fn plugin_query_plugin_param_count() {
        let graph = PluginGraph::with_default_rack();
        let value = plugin_query(&graph, "plugins.plugin.1.param_count").unwrap();
        assert_eq!(value.as_u64().unwrap(), 2);
    }

    #[test]
    fn plugin_query_param_properties() {
        let graph = PluginGraph::with_default_rack();
        assert_eq!(
            plugin_query(&graph, "plugins.plugin.1.param.0.name")
                .unwrap()
                .as_str()
                .unwrap(),
            "Gain"
        );
        assert_eq!(
            plugin_query(&graph, "plugins.plugin.1.param.0.type")
                .unwrap()
                .as_str()
                .unwrap(),
            "float"
        );
        assert_eq!(
            plugin_query(&graph, "plugins.plugin.1.param.0.min")
                .unwrap()
                .as_f64()
                .unwrap(),
            -60.0
        );
        assert_eq!(
            plugin_query(&graph, "plugins.plugin.1.param.0.max")
                .unwrap()
                .as_f64()
                .unwrap(),
            20.0
        );
    }

    #[test]
    fn plugin_query_unknown_path_errors() {
        let graph = PluginGraph::with_default_rack();
        assert!(plugin_query(&graph, "plugins.foo").is_err());
        assert!(plugin_query(&graph, "plugins.plugin.99.type").is_err());
        assert!(plugin_query(&graph, "plugins.plugin.1.param.99.name").is_err());
        assert!(plugin_query(&graph, "plugins.plugin.1.param.0.xyz").is_err());
    }
}
