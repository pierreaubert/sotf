use super::plugin_controller::PluginController;
use super::types::{EqEditTarget, PluginUpdateEffect};
use crate::plugin_graph::PluginGraph;
use crate::{PluginSettings, PluginType};

use crate::plugin_graph::NodePosition;

#[test]
fn spectrum_controller_edits_share_widget_ranges_and_steps() {
    for by_node in [false, true] {
        let mut controller = PluginController::new();
        let node = controller
            .graph
            .add_user_plugin(&PluginType::SpectrumAnalyzer)
            .unwrap();
        let index = controller.graph.linear_index_of_node(node).unwrap();
        controller.editing_plugin_index = Some(index);
        for (param, initial, delta, expected) in [
            (0, 8.0, -1.0, 8.0),
            (0, 119.0, 1.0, 120.0),
            (0, 120.0, 1.0, 120.0),
            (1, 20.0, 1.0, 21.0),
            (2, 5000.0, -1.0, 5000.0),
            (2, 20000.0, 1.0, 20100.0),
            (2, 22050.0, 1.0, 22050.0),
            (3, 0.7, 1.0, 0.71),
        ] {
            controller.set_plugin_param(index, param, initial);
            if by_node {
                assert!(controller.adjust_param_by_node_id(node, param, delta));
            } else {
                controller.plugin_param_selection = param;
                let (adjusted, effect) = controller.adjust_selected_param(delta);
                assert!(adjusted);
                if param < 3 {
                    assert!(matches!(effect, PluginUpdateEffect::Structural));
                } else {
                    assert!(matches!(effect, PluginUpdateEffect::Parameter { .. }));
                }
            }
            let settings = &controller.graph.get_plugin(index).unwrap().settings;
            let actual = super::parameter_metadata::desktop_param_value(settings, param).unwrap();
            assert!(
                (actual - expected).abs() < 1e-6,
                "{by_node}/{param}: {actual}"
            );
        }
        let before = serde_json::to_value(&controller.graph).unwrap();
        for param in 0..4 {
            for delta in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                if by_node {
                    assert!(!controller.adjust_param_by_node_id(node, param, delta));
                } else {
                    controller.plugin_param_selection = param;
                    let (adjusted, effect) = controller.adjust_selected_param(delta);
                    assert!(!adjusted);
                    assert!(matches!(effect, PluginUpdateEffect::None));
                }
                assert_eq!(serde_json::to_value(&controller.graph).unwrap(), before);
            }
        }
    }
}

#[test]
fn try_add_plugin_selects_the_inserted_plugin() {
    let mut controller = PluginController::new();
    let index = controller.graph.user_plugin_insert_index();
    let effect = controller.try_add_plugin(&PluginType::EQ).unwrap();
    assert!(matches!(effect, PluginUpdateEffect::Structural));
    assert_eq!(controller.selected_plugin_index, index);
    assert_eq!(
        controller.graph.get_plugin(index).unwrap().plugin_type(),
        PluginType::EQ
    );
}

#[test]
fn try_add_plugin_returns_channel_error_without_mutating_graph_or_selection() {
    let mut controller = PluginController::new();
    controller.try_add_plugin(&PluginType::AAE).unwrap();
    let before = serde_json::to_value(&controller.graph).unwrap();
    let selected = controller.selected_plugin_index;

    let error = controller.try_add_plugin(&PluginType::Upmixer).unwrap_err();
    assert!(
        error.contains("requires 2 input channel(s), but upstream provides 6"),
        "{error}"
    );
    assert_eq!(serde_json::to_value(&controller.graph).unwrap(), before);
    assert_eq!(controller.selected_plugin_index, selected);

    // Existing non-GPUI callers retain their no-effect result on rejection.
    assert!(matches!(
        controller.add_plugin(&PluginType::Upmixer),
        PluginUpdateEffect::None
    ));
    assert_eq!(serde_json::to_value(&controller.graph).unwrap(), before);
    assert_eq!(controller.selected_plugin_index, selected);
}

#[test]
fn linear_gate_rejects_unrouted_external_sidechain_without_mutation() {
    let mut controller = PluginController::new();
    let gate_idx = controller.graph.user_plugin_insert_index();
    controller.try_add_plugin(&PluginType::Gate).unwrap();
    let before = serde_json::to_value(&controller.graph).unwrap();
    let external_idx = sotf_plugins::param_specs::index_of(
        sotf_plugins::param_specs::gate::PARAMS,
        "sidechain_external",
    );
    assert!(matches!(
        controller.set_plugin_param(gate_idx, external_idx, 1.0),
        PluginUpdateEffect::None
    ));
    assert_eq!(serde_json::to_value(&controller.graph).unwrap(), before);

    // Graph edits retain the external-key path; the graph host validates the
    // actual sidechain edges before committing that structural candidate.
    let node_id = controller.graph.plugins_linear().unwrap()[gate_idx].id;
    assert!(matches!(
        controller.set_plugin_param_by_node_id(node_id, external_idx, 1.0),
        PluginUpdateEffect::Structural
    ));
}

#[test]
fn linear_de_esser_rejects_unrouted_external_sidechain_without_mutation() {
    let mut controller = PluginController::new();
    let index = controller.graph.user_plugin_insert_index();
    controller.try_add_plugin(&PluginType::DeEsser).unwrap();
    let before = serde_json::to_value(&controller.graph).unwrap();
    let external = sotf_plugins::param_specs::index_of(
        sotf_plugins::param_specs::de_esser::PARAMS,
        "sidechain_external",
    );
    assert!(matches!(
        controller.set_plugin_param(index, external, 1.0),
        PluginUpdateEffect::None
    ));
    assert_eq!(serde_json::to_value(&controller.graph).unwrap(), before);
    let node_id = controller.graph.plugins_linear().unwrap()[index].id;
    assert!(matches!(
        controller.set_plugin_param_by_node_id(node_id, external, 1.0),
        PluginUpdateEffect::Structural
    ));
}

#[test]
fn dynamic_eq_reset_preserves_global_inheritance_in_linear_and_node_routes() {
    let mut controller = PluginController::new();
    let index = controller.graph.user_plugin_insert_index();
    controller.try_add_plugin(&PluginType::DynamicEq).unwrap();
    let node_id = controller.graph.plugins_linear().unwrap()[index].id;
    controller.set_plugin_param(index, 113, -35.0);
    controller.set_plugin_param(index, 114, 8.0);
    controller.set_plugin_param(index, 1, -30.0);
    controller.set_plugin_param_by_node_id(node_id, 2, 4.0);
    controller.reset_plugin_param(index, 1);
    controller.reset_plugin_param_by_node_id(node_id, 2);
    let PluginSettings::DynamicEq {
        threshold,
        ratio,
        bands,
        ..
    } = &controller.graph.get_plugin(index).unwrap().settings
    else {
        panic!("expected Dynamic EQ settings");
    };
    assert_eq!(*threshold, -20.0);
    assert_eq!(*ratio, 2.0);
    assert_eq!(bands[0].band_threshold, -20.0);
    assert_eq!(bands[0].band_ratio, 2.0);
    assert_eq!(bands[1].band_threshold, -35.0);
    assert_eq!(bands[1].band_ratio, 8.0);
}

#[test]
fn incompatible_linear_modes_preserve_the_existing_chain() {
    let mut downmix = PluginController::new();
    let index = downmix.graph.user_plugin_insert_index();
    downmix.try_add_plugin(&PluginType::Downmix).unwrap();
    let ltrt = sotf_plugins::param_specs::index_of(
        sotf_plugins::param_specs::downmix::PARAMS,
        "matrix_ltrt",
    );
    let phase = sotf_plugins::param_specs::index_of(
        sotf_plugins::param_specs::downmix::PARAMS,
        "phase_coherence",
    );
    let before = serde_json::to_value(&downmix.graph).unwrap();
    assert!(matches!(
        downmix.set_plugin_param(index, ltrt, 1.0),
        PluginUpdateEffect::None
    ));
    assert_eq!(serde_json::to_value(&downmix.graph).unwrap(), before);
    downmix.set_plugin_param(index, phase, 0.0);
    assert!(matches!(
        downmix.set_plugin_param(index, ltrt, 1.0),
        PluginUpdateEffect::Structural
    ));
    let after_ltrt = serde_json::to_value(&downmix.graph).unwrap();
    assert!(matches!(
        downmix.set_plugin_param(index, phase, 1.0),
        PluginUpdateEffect::None
    ));
    assert_eq!(serde_json::to_value(&downmix.graph).unwrap(), after_ltrt);
    let downmix_id = downmix.graph.plugins_linear().unwrap()[index].id;
    assert!(matches!(
        downmix.reset_plugin_param(index, phase),
        PluginUpdateEffect::None
    ));
    assert_eq!(serde_json::to_value(&downmix.graph).unwrap(), after_ltrt);
    assert!(matches!(
        downmix.set_plugin_param_by_node_id(downmix_id, phase, 1.0),
        PluginUpdateEffect::None
    ));
    assert!(matches!(
        downmix.reset_plugin_param_by_node_id(downmix_id, phase),
        PluginUpdateEffect::None
    ));
    assert_eq!(serde_json::to_value(&downmix.graph).unwrap(), after_ltrt);

    let mut loudness = PluginController::new();
    let index = loudness.graph.user_plugin_insert_index();
    loudness
        .try_add_plugin(&PluginType::LoudnessCompensation)
        .unwrap();
    let calibration = sotf_plugins::param_specs::index_of(
        sotf_plugins::param_specs::loudness_compensation::PARAMS,
        "auto_calibrated",
    );
    let mode = sotf_plugins::param_specs::index_of(
        sotf_plugins::param_specs::loudness_compensation::PARAMS,
        "mode",
    );
    loudness.set_plugin_param(index, calibration, 1.0);
    loudness.set_plugin_param(index, mode, 2.0);
    let before = serde_json::to_value(&loudness.graph).unwrap();
    assert!(matches!(
        loudness.set_plugin_param(index, calibration, 0.0),
        PluginUpdateEffect::None
    ));
    assert!(matches!(
        loudness.reset_plugin_param(index, calibration),
        PluginUpdateEffect::None
    ));
    assert_eq!(serde_json::to_value(&loudness.graph).unwrap(), before);
    let calibrated_id = loudness.graph.plugins_linear().unwrap()[index].id;
    assert!(matches!(
        loudness.reset_plugin_param_by_node_id(calibrated_id, calibration),
        PluginUpdateEffect::None
    ));
    assert_eq!(serde_json::to_value(&loudness.graph).unwrap(), before);
    loudness.set_plugin_param(index, mode, 0.0);
    assert!(matches!(
        loudness.set_plugin_param(index, calibration, 0.0),
        PluginUpdateEffect::Structural
    ));
    let uncalibrated = serde_json::to_value(&loudness.graph).unwrap();
    assert!(matches!(
        loudness.set_plugin_param(index, mode, 2.0),
        PluginUpdateEffect::None
    ));
    assert_eq!(serde_json::to_value(&loudness.graph).unwrap(), uncalibrated);
    let loudness_id = loudness.graph.plugins_linear().unwrap()[index].id;
    assert!(matches!(
        loudness.set_plugin_param_by_node_id(loudness_id, mode, 2.0),
        PluginUpdateEffect::None
    ));
    assert_eq!(serde_json::to_value(&loudness.graph).unwrap(), uncalibrated);
}

#[test]
fn profile_actions_dispatch_named_commands_without_latching_saved_flags() {
    for (plugin_type, learn, clear) in [
        (PluginType::Denoiser, 20, 22),
        (PluginType::HissReducer, 5, 7),
    ] {
        let mut controller = PluginController::new();
        let index = controller.graph.user_plugin_insert_index();
        controller.try_add_plugin(&plugin_type).unwrap();
        let before = serde_json::to_value(&controller.graph).unwrap();
        let node_id = controller.graph.plugins_linear().unwrap()[index].id;
        for (param, key) in [(learn, "learn_noise"), (clear, "clear_profile")] {
            assert_eq!(
                super::super::plugin_param_map::momentary_action_param(
                    &controller.graph.get_plugin(index).unwrap().settings,
                    param,
                ),
                Some(key)
            );
            assert!(
                super::plugin_controller::param_index_to_engine_param(
                    &controller.graph.get_plugin(index).unwrap().settings,
                    param,
                )
                .is_none()
            );
            assert!(matches!(
                controller.set_plugin_param(index, param, 1.0),
                PluginUpdateEffect::Action { plugin_index, param_index, key: dispatched, value: true, .. }
                    if plugin_index == index && param_index == param && dispatched == key
            ));
            assert!(matches!(
                controller.set_plugin_param_by_node_id(node_id, param, 1.0),
                PluginUpdateEffect::Action { node_id: Some(dispatched), value: true, .. }
                    if dispatched == node_id
            ));
            if key == "learn_noise" {
                assert!(matches!(
                    controller.set_plugin_param(index, param, 0.0),
                    PluginUpdateEffect::Action { value: false, .. }
                ));
            } else {
                assert!(matches!(
                    controller.set_plugin_param(index, param, 0.0),
                    PluginUpdateEffect::None
                ));
            }
            assert_eq!(serde_json::to_value(&controller.graph).unwrap(), before);
        }
    }
}

#[test]
fn output_layout_choices_normalize_downstream_matrix_before_save() {
    for (plugin_type, param_idx) in [(PluginType::AAE, 0), (PluginType::AmbisonicsDecoder, 1)] {
        let mut controller = PluginController::new();
        if plugin_type == PluginType::AmbisonicsDecoder {
            controller.graph.input_node_mut().unwrap().channels = 4;
            controller.graph.update_channel_dependent_plugins();
        }
        let target_idx = controller.graph.user_plugin_insert_index();
        controller.try_add_plugin(&plugin_type).unwrap();
        assert!(matches!(
            controller.set_plugin_param(target_idx, param_idx, 2.0),
            PluginUpdateEffect::Structural
        ));
        let matrix = controller.graph.get_plugin(target_idx + 1).unwrap();
        let PluginSettings::Matrix {
            input_channels,
            output_channels,
            channel_states,
            matrix,
            ..
        } = &matrix.settings
        else {
            panic!("expected downstream Matrix");
        };
        assert_eq!((*input_channels, *output_channels), (8, 8));
        assert_eq!(channel_states.len(), 8);
        assert_eq!(matrix.len(), 64);

        let serialized = serde_json::to_string(&controller.graph).unwrap();
        let mut restored: PluginGraph = serde_json::from_str(&serialized).unwrap();
        restored.update_channel_dependent_plugins();
        assert_eq!(
            serde_json::to_value(restored).unwrap(),
            serde_json::to_value(&controller.graph).unwrap(),
            "{plugin_type:?} should already be normalized before reload"
        );
    }
}

#[test]
fn custom_ambisonics_choice_without_geometry_preserves_the_working_chain() {
    let mut controller = PluginController::new();
    controller.graph.input_node_mut().unwrap().channels = 4;
    controller.graph.update_channel_dependent_plugins();
    let index = controller.graph.user_plugin_insert_index();
    controller
        .try_add_plugin(&PluginType::AmbisonicsDecoder)
        .unwrap();
    let before = serde_json::to_value(&controller.graph).unwrap();
    assert!(matches!(
        controller.set_plugin_param(index, 1, 8.0),
        PluginUpdateEffect::None
    ));
    assert_eq!(serde_json::to_value(&controller.graph).unwrap(), before);
    assert!(matches!(
        controller.set_plugin_param(index, 1, 2.0),
        PluginUpdateEffect::Structural
    ));
}

#[test]
fn node_id_output_layout_edit_normalizes_downstream_matrix() {
    let mut controller = PluginController::new();
    controller.try_add_plugin(&PluginType::AAE).unwrap();
    let node_id = *controller
        .graph
        .nodes
        .iter()
        .find(|(_, node)| node.plugin.plugin_type() == PluginType::AAE)
        .map(|(id, _)| id)
        .unwrap();
    assert!(matches!(
        controller.set_plugin_param_by_node_id(node_id, 0, 2.0),
        PluginUpdateEffect::Structural
    ));
    let matrix = controller
        .graph
        .plugins_linear()
        .unwrap()
        .into_iter()
        .find(|node| node.plugin.plugin_type() == PluginType::Matrix)
        .unwrap();
    let PluginSettings::Matrix {
        input_channels,
        output_channels,
        ..
    } = &matrix.plugin.settings
    else {
        panic!("expected downstream Matrix");
    };
    assert_eq!((*input_channels, *output_channels), (8, 8));
}

#[test]
fn crossover_and_band_split_edits_follow_engine_channel_width() {
    let mut crossover = PluginController::new();
    let crossover_idx = crossover.graph.user_plugin_insert_index();
    crossover.try_add_plugin(&PluginType::Crossover).unwrap();
    for (param_idx, value, channels) in [
        (2, 2.0, 4), // Both: two bands per input channel.
        (5, 1.0, 6), // Three bands.
        (5, 2.0, 8), // Four bands.
        (4, 1.0, 2), // Per-channel topology restores original width.
    ] {
        assert!(matches!(
            crossover.set_plugin_param(crossover_idx, param_idx, value),
            PluginUpdateEffect::Structural
        ));
        let target_id = crossover.graph.get_plugin(crossover_idx).unwrap().id;
        let target = crossover
            .graph
            .nodes
            .values()
            .find(|node| node.plugin.id == target_id)
            .unwrap();
        let matrix = crossover.graph.get_plugin(crossover_idx + 1).unwrap();
        assert_eq!(target.output_channels, channels);
        let PluginSettings::Matrix {
            input_channels,
            output_channels,
            ..
        } = &matrix.settings
        else {
            panic!("expected downstream Matrix");
        };
        assert_eq!((*input_channels, *output_channels), (channels, channels));
    }

    let mut split = PluginController::new();
    let split_idx = split.graph.user_plugin_insert_index();
    split.try_add_plugin(&PluginType::BandSplit).unwrap();
    for (value, channels) in [(1.0, 6), (2.0, 8), (0.0, 4)] {
        assert!(matches!(
            split.set_plugin_param(split_idx, 3, value),
            PluginUpdateEffect::Structural
        ));
        let target_id = split.graph.get_plugin(split_idx).unwrap().id;
        let target = split
            .graph
            .nodes
            .values()
            .find(|node| node.plugin.id == target_id)
            .unwrap();
        let matrix = split.graph.get_plugin(split_idx + 1).unwrap();
        assert_eq!(target.output_channels, channels);
        let PluginSettings::Matrix {
            input_channels,
            output_channels,
            ..
        } = &matrix.settings
        else {
            panic!("expected downstream Matrix");
        };
        assert_eq!((*input_channels, *output_channels), (channels, channels));
    }
}

#[test]
fn eq_order_edit_targets_one_channel_and_rejects_svf() {
    let (mut ctrl, id) = make_controller_with_eq();
    if let PluginSettings::EQ {
        filters,
        channel_filters,
        per_channel_mode,
        ..
    } = &mut ctrl.graph.nodes.get_mut(&id).unwrap().plugin.settings
    {
        *channel_filters = Some(vec![filters.clone(), filters.clone()]);
        *per_channel_mode = true;
    }
    let effect = ctrl
        .set_eq_filter_order_for_target_by_node_id(id, EqEditTarget::Channel(1), 0, 8)
        .unwrap();
    assert!(matches!(effect, PluginUpdateEffect::Structural));
    if let PluginSettings::EQ {
        filters,
        channel_filters,
        topology,
        ..
    } = &mut ctrl.graph.nodes.get_mut(&id).unwrap().plugin.settings
    {
        let channels = channel_filters.as_ref().unwrap();
        assert_eq!(filters[0].order, 2);
        assert_eq!(channels[0][0].order, 2);
        assert_eq!(channels[1][0].order, 8);
        *topology = 1.0;
    }
    assert!(
        ctrl.set_eq_filter_order_for_target_by_node_id(id, EqEditTarget::Global, 0, 4)
            .is_err()
    );
    assert!(
        ctrl.set_eq_filter_order_for_target_by_node_id(id, EqEditTarget::Global, 0, 3)
            .is_err()
    );
    assert!(
        ctrl.set_eq_filter_order_for_target_by_node_id(id, EqEditTarget::Channel(9), 0, 2)
            .is_err()
    );
}

#[test]
fn high_order_band_rejects_incompatible_topology_without_losing_order() {
    use sotf_audio::plugins::EqFilterTopology;
    let (mut ctrl, id) = make_controller_with_eq();
    ctrl.set_eq_filter_order_for_target_by_node_id(id, EqEditTarget::Global, 0, 8)
        .unwrap();
    for topology in [
        EqFilterTopology::WarpedBiquad,
        EqFilterTopology::KautzFilter,
    ] {
        assert!(matches!(
            ctrl.set_eq_filter_topology_for_target_by_node_id(
                id,
                EqEditTarget::Global,
                0,
                topology,
            ),
            PluginUpdateEffect::None
        ));
    }
    assert!(matches!(
        ctrl.cycle_eq_filter_topology_for_target_by_node_id(id, EqEditTarget::Global, 0,),
        PluginUpdateEffect::None
    ));
    let PluginSettings::EQ { filters, .. } = &ctrl.graph.nodes[&id].plugin.settings else {
        panic!("expected EQ")
    };
    assert_eq!(filters[0].order, 8);
    assert!(matches!(filters[0].topology, EqFilterTopology::Biquad));
}

#[test]
fn global_svf_rejects_high_order_without_mutation() {
    use super::plugin_controller::{eq_supports_svf, set_eq_global_topology};
    let (mut ctrl, id) = make_controller_with_eq();
    ctrl.set_eq_filter_order_for_target_by_node_id(id, EqEditTarget::Global, 0, 4)
        .unwrap();
    let settings = &mut ctrl.graph.nodes.get_mut(&id).unwrap().plugin.settings;
    assert!(!eq_supports_svf(settings));
    assert!(set_eq_global_topology(settings, 1.0).is_err());
    assert!(set_eq_global_topology(settings, f64::NAN).is_err());
    let PluginSettings::EQ {
        filters, topology, ..
    } = settings
    else {
        panic!("expected EQ")
    };
    assert_eq!(*topology, 0.0);
    assert_eq!(filters[0].order, 4);
    filters[0].order = 2;
    assert!(eq_supports_svf(settings));
    assert!(matches!(
        set_eq_global_topology(settings, 1.0).unwrap(),
        PluginUpdateEffect::Structural
    ));
}

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
fn linear_phase_eq_band_actions_keep_dsp_count_in_sync() {
    let mut ctrl = PluginController::new();
    let node_id = ctrl
        .graph
        .add_user_plugin(&PluginType::LinearPhaseEq)
        .unwrap();
    let index = ctrl.graph.linear_index_of_node(node_id).unwrap();
    ctrl.editing_plugin_index = Some(index);

    let count = |ctrl: &PluginController| {
        let PluginSettings::LinearPhaseEq {
            num_filters,
            filters,
            ..
        } = &ctrl.graph.nodes.get(&node_id).unwrap().plugin.settings
        else {
            panic!("expected Linear Phase EQ")
        };
        assert_eq!(*num_filters as usize, filters.len());
        filters.len()
    };

    let original = count(&ctrl);
    assert!(matches!(
        ctrl.add_eq_band().unwrap(),
        PluginUpdateEffect::Structural
    ));
    assert_eq!(count(&ctrl), original + 1);
    assert!(matches!(
        ctrl.remove_eq_band(original).unwrap(),
        PluginUpdateEffect::Structural
    ));
    assert_eq!(count(&ctrl), original);

    assert!(matches!(
        ctrl.add_eq_band_by_node_id(node_id).unwrap(),
        PluginUpdateEffect::Structural
    ));
    assert_eq!(count(&ctrl), original + 1);
    assert!(matches!(
        ctrl.remove_eq_band_by_node_id(node_id, original).unwrap(),
        PluginUpdateEffect::Structural
    ));
    assert_eq!(count(&ctrl), original);

    while count(&ctrl) > 1 {
        let last = count(&ctrl) - 1;
        ctrl.remove_eq_band(last).unwrap();
    }
    assert!(ctrl.remove_eq_band(0).is_err());
    assert!(ctrl.remove_eq_band_by_node_id(node_id, 0).is_err());
    assert_eq!(count(&ctrl), 1);

    while count(&ctrl) < 10 {
        ctrl.add_eq_band_by_node_id(node_id).unwrap();
    }
    assert!(ctrl.add_eq_band().is_err());
    assert!(ctrl.add_eq_band_by_node_id(node_id).is_err());
    assert_eq!(count(&ctrl), 10);
}

#[test]
fn linear_phase_eq_apo_import_keeps_count_and_rejects_unsupported_sizes() {
    let mut ctrl = PluginController::new();
    let node_id = ctrl
        .graph
        .add_user_plugin(&PluginType::LinearPhaseEq)
        .unwrap();
    let index = ctrl.graph.linear_index_of_node(node_id).unwrap();
    ctrl.editing_plugin_index = Some(index);
    let path = std::env::temp_dir().join(format!("sotf-lp-band-count-{}.apo", std::process::id()));
    let write_apo = |count: usize| {
        let contents = (0..count)
            .map(|i| format!("Filter {}: ON PK Fc 1000 Hz Gain 2 dB Q 1", i + 1))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&path, contents).unwrap();
    };
    let assert_count = |ctrl: &PluginController, expected: usize| {
        let PluginSettings::LinearPhaseEq {
            num_filters,
            filters,
            ..
        } = &ctrl.graph.nodes.get(&node_id).unwrap().plugin.settings
        else {
            panic!("expected Linear Phase EQ")
        };
        assert_eq!(*num_filters as usize, expected);
        assert_eq!(filters.len(), expected);
    };

    write_apo(6);
    assert!(matches!(
        ctrl.load_apo_filters(&path).unwrap(),
        PluginUpdateEffect::Structural
    ));
    assert_count(&ctrl, 6);
    write_apo(4);
    assert!(matches!(
        ctrl.load_apo_filters_by_node_id(node_id, &path).unwrap(),
        PluginUpdateEffect::Structural
    ));
    assert_count(&ctrl, 4);

    write_apo(11);
    assert!(ctrl.load_apo_filters(&path).is_err());
    assert!(ctrl.load_apo_filters_by_node_id(node_id, &path).is_err());
    assert_count(&ctrl, 4);
    std::fs::remove_file(path).unwrap();
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
    use super::super::dev_api::actions::plugin_action;
    use super::super::dev_api::queries::plugin_query;
    use crate::PluginSettings;
    use crate::plugin_graph::PluginGraph;
    use serde_json::json;

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
    fn plugin_query_enabled_tracks_bypass() {
        let mut graph = PluginGraph::with_default_rack();
        plugin_action(
            &mut graph,
            "PluginAdd",
            Some(json!({"plugin_type":"Matrix"})),
        )
        .unwrap();
        let path = "plugins.plugin.2.enabled";
        assert_eq!(plugin_query(&graph, path).unwrap(), json!(true));
        graph.toggle_plugin_by_index(2).unwrap();
        assert_eq!(plugin_query(&graph, path).unwrap(), json!(false));
        assert_eq!(
            plugin_query(&graph, "plugins.list").unwrap()[2]["enabled"],
            plugin_query(&graph, path).unwrap()
        );
        graph.toggle_plugin_by_index(2).unwrap();
        assert_eq!(plugin_query(&graph, path).unwrap(), json!(true));
        assert!(plugin_query(&graph, "plugins.plugin.99.enabled").is_err());
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
    fn plugin_query_dynamic_eq_param_properties_and_value() {
        let mut graph = PluginGraph::with_default_rack();
        let eq_idx = graph.user_plugin_insert_index();
        plugin_action(&mut graph, "PluginAdd", Some(json!({"plugin_type":"EQ"}))).unwrap();

        let plugin = graph.get_plugin_mut(eq_idx).unwrap();
        let PluginSettings::EQ { filters, .. } = &mut plugin.settings else {
            panic!("expected EQ settings");
        };
        assert_eq!(filters.len(), 5);
        filters[4].frequency = 4_321.0;

        let prefix = format!("plugins.plugin.{eq_idx}.param.16");
        assert_eq!(
            plugin_query(&graph, &format!("{prefix}.name")).unwrap(),
            json!("Frequency")
        );
        assert_eq!(
            plugin_query(&graph, &format!("{prefix}.type")).unwrap(),
            json!("float")
        );
        assert_eq!(
            plugin_query(&graph, &format!("{prefix}.value")).unwrap(),
            json!(4_321.0)
        );
        assert!(plugin_query(&graph, &format!("plugins.plugin.{eq_idx}.param.20.name")).is_err());
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

#[test]
fn multiband_count_reset_preserves_band_overrides() {
    for kind in [
        PluginType::MultibandCompressor,
        PluginType::MultibandExpander,
    ] {
        for by_node in [false, true] {
            let mut ctrl = PluginController::new();
            let id = ctrl.graph.add_user_plugin(&kind).unwrap();
            ctrl.set_plugin_param_by_node_id(id, 117, 0.0);
            ctrl.set_plugin_param_by_node_id(id, 106, -33.0);
            ctrl.set_plugin_param_by_node_id(id, 0, 5.0);
            if by_node {
                ctrl.reset_plugin_param_by_node_id(id, 0);
            } else {
                let index = ctrl.graph.linear_index_of_node(id).unwrap();
                ctrl.reset_plugin_param(index, 0);
            }
            let settings = &ctrl.graph.nodes.get(&id).unwrap().plugin.settings;
            match settings {
                PluginSettings::MultibandCompressor {
                    num_bands, bands, ..
                } => {
                    assert_eq!(*num_bands, 3);
                    assert_eq!(bands.len(), 3);
                    assert!(!bands[0].active);
                    assert_eq!(bands[0].threshold_db, Some(-33.0));
                }
                PluginSettings::MultibandExpander {
                    num_bands, bands, ..
                } => {
                    assert_eq!(*num_bands, 3);
                    assert_eq!(bands.len(), 3);
                    assert!(!bands[0].active);
                    assert_eq!(bands[0].threshold_db, Some(-33.0));
                }
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn multiband_incremental_edits_materialize_sparse_overrides() {
    for kind in [
        PluginType::MultibandCompressor,
        PluginType::MultibandExpander,
    ] {
        let mut ctrl = PluginController::new();
        let id = ctrl.graph.add_user_plugin(&kind).unwrap();
        let original = serde_json::to_value(&ctrl.graph.nodes[&id].plugin.settings).unwrap();
        for (index, delta) in [(406, 1.0), (399, 1.0), (306, f64::NAN)] {
            assert!(!ctrl.adjust_param_by_node_id(id, index, delta));
            assert_eq!(
                serde_json::to_value(&ctrl.graph.nodes[&id].plugin.settings).unwrap(),
                original
            );
        }
        // First adjustment selects an explicit value instead of inheritance;
        // subsequent adjustments move that value using the existing step rules.
        assert!(ctrl.adjust_param_by_node_id(id, 306, 1.0));
        assert!(ctrl.adjust_param_by_node_id(id, 306, 1.0));
        match &ctrl.graph.nodes[&id].plugin.settings {
            PluginSettings::MultibandCompressor {
                threshold_db,
                bands,
                ..
            } => {
                assert_eq!(bands.len(), 3);
                assert!(bands[0].threshold_db.is_none());
                assert_eq!(bands[2].threshold_db, Some(*threshold_db as f32 + 1.0));
            }
            PluginSettings::MultibandExpander {
                threshold_db,
                bands,
                ..
            } => {
                assert_eq!(bands.len(), 3);
                assert!(bands[0].threshold_db.is_none());
                assert_eq!(bands[2].threshold_db, Some(*threshold_db as f32 + 1.0));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn multiband_band_reset_restores_inheritance_without_erasing_other_edits() {
    for kind in [
        PluginType::MultibandCompressor,
        PluginType::MultibandExpander,
    ] {
        for by_node in [false, true] {
            let mut ctrl = PluginController::new();
            let id = ctrl.graph.add_user_plugin(&kind).unwrap();
            ctrl.set_plugin_param_by_node_id(id, 6, -25.0);
            ctrl.set_plugin_param_by_node_id(id, 106, -33.0);
            ctrl.set_plugin_param_by_node_id(id, 107, 5.0);
            ctrl.set_plugin_param_by_node_id(id, 206, -45.0);
            let effect = if by_node {
                ctrl.reset_plugin_param_by_node_id(id, 106)
            } else {
                let index = ctrl.graph.linear_index_of_node(id).unwrap();
                ctrl.reset_plugin_param(index, 106)
            };
            assert!(matches!(effect, PluginUpdateEffect::Structural));
            ctrl.set_plugin_param_by_node_id(id, 6, -20.0);
            match &ctrl.graph.nodes[&id].plugin.settings {
                PluginSettings::MultibandCompressor {
                    threshold_db,
                    bands,
                    ..
                } => {
                    assert_eq!(*threshold_db, -20.0);
                    assert_eq!(bands[0].threshold_db, None);
                    assert_eq!(bands[0].ratio, Some(5.0));
                    assert_eq!(bands[1].threshold_db, Some(-45.0));
                }
                PluginSettings::MultibandExpander {
                    threshold_db,
                    bands,
                    ..
                } => {
                    assert_eq!(*threshold_db, -20.0);
                    assert_eq!(bands[0].threshold_db, None);
                    assert_eq!(bands[0].ratio, Some(5.0));
                    assert_eq!(bands[1].threshold_db, Some(-45.0));
                }
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn multiband_adjust_below_min_rebuilds_inherited_band_state() {
    for (kind, index, key) in [
        (PluginType::MultibandCompressor, 118, "range_db"),
        (PluginType::MultibandExpander, 110, "range"),
    ] {
        let template = match &kind {
            PluginType::MultibandCompressor => {
                sotf_plugins::param_specs::multiband_compressor::BAND_TEMPLATE
            }
            _ => sotf_plugins::param_specs::multiband_expander::BAND_TEMPLATE,
        };
        let minimum = sotf_plugins::param_specs::find_by_key(template, key).min_f64();
        let mut ctrl = PluginController::new();
        let node = ctrl.graph.add_user_plugin(&kind).unwrap();
        let linear = ctrl.graph.linear_index_of_node(node).unwrap();
        ctrl.set_plugin_param_by_node_id(node, index, minimum);
        ctrl.editing_plugin_index = Some(linear);
        ctrl.plugin_param_selection = index;
        let (changed, effect) = ctrl.adjust_selected_param(-1.0);
        assert!(changed);
        assert!(matches!(effect, PluginUpdateEffect::Structural));
        assert!(matches!(
            ctrl.determine_update_effect_by_node_id(node, index, false),
            PluginUpdateEffect::Structural
        ));
        let settings = &ctrl.graph.nodes[&node].plugin.settings;
        match settings {
            PluginSettings::MultibandCompressor { bands, .. } => {
                assert_eq!(bands[0].range_db, None)
            }
            PluginSettings::MultibandExpander { bands, .. } => {
                assert_eq!(bands[0].range_db, None)
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn multiband_direct_entry_clamps_before_float_conversion() {
    for kind in [
        PluginType::MultibandCompressor,
        PluginType::MultibandExpander,
    ] {
        let mut ctrl = PluginController::new();
        let id = ctrl.graph.add_user_plugin(&kind).unwrap();
        ctrl.set_plugin_param_by_node_id(id, 106, f64::MAX);
        ctrl.set_plugin_param_by_node_id(id, 107, -f64::MAX);
        ctrl.set_plugin_param_by_node_id(id, 108, f64::MAX);
        match &ctrl.graph.nodes[&id].plugin.settings {
            PluginSettings::MultibandCompressor { bands, .. } => {
                assert_eq!(bands[0].threshold_db, Some(0.0));
                assert_eq!(bands[0].ratio, Some(1.0));
                assert_eq!(bands[0].attack_ms, Some(100.0));
            }
            PluginSettings::MultibandExpander { bands, .. } => {
                assert_eq!(bands[0].threshold_db, Some(0.0));
                assert_eq!(bands[0].ratio, Some(1.0));
                assert_eq!(bands[0].attack_ms, Some(50.0));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn dynamic_eq_new_band_fields_set_adjust_reset_in_both_routes() {
    use sotf_plugins::plugin_dynamic_eq::{DynEqPlacement, DynEqShape};
    for by_node in [false, true] {
        let mut controller = PluginController::new();
        let index = controller.graph.user_plugin_insert_index();
        controller.try_add_plugin(&PluginType::DynamicEq).unwrap();
        let node = controller.graph.plugins_linear().unwrap()[index].id;
        for (param, value) in [(117, 1.0), (118, 0.37), (119, 3.0)] {
            let effect = if by_node {
                controller.set_plugin_param_by_node_id(node, param, value)
            } else {
                controller.set_plugin_param(index, param, value)
            };
            assert!(matches!(effect, PluginUpdateEffect::Structural));
        }
        let PluginSettings::DynamicEq { bands, .. } =
            &controller.graph.get_plugin(index).unwrap().settings
        else {
            panic!("Dynamic EQ");
        };
        assert_eq!(bands[1].shape, DynEqShape::LowShelf);
        assert!((bands[1].shelf_slope - 0.37).abs() < 1e-6);
        assert_eq!(bands[1].placement, DynEqPlacement::Mid);
        assert_eq!(bands[0].shape, DynEqShape::Peak);
        let before = serde_json::to_value(&controller.graph).unwrap();
        let effect = if by_node {
            controller.set_plugin_param_by_node_id(node, 187, 2.0)
        } else {
            controller.set_plugin_param(index, 187, 2.0)
        };
        assert!(matches!(effect, PluginUpdateEffect::None));
        assert_eq!(before, serde_json::to_value(&controller.graph).unwrap());
        controller.editing_plugin_index = Some(index);
        controller.plugin_param_selection = 118;
        assert!(controller.adjust_selected_param(1.0).0);
        let PluginSettings::DynamicEq { bands, .. } =
            &controller.graph.get_plugin(index).unwrap().settings
        else {
            panic!("Dynamic EQ");
        };
        assert!((bands[1].shelf_slope - 0.38).abs() < 1e-6);
        for param in [117, 118, 119] {
            let effect = if by_node {
                controller.reset_plugin_param_by_node_id(node, param)
            } else {
                controller.reset_plugin_param(index, param)
            };
            assert!(matches!(effect, PluginUpdateEffect::Structural));
        }
        let PluginSettings::DynamicEq { bands, .. } =
            &controller.graph.get_plugin(index).unwrap().settings
        else {
            panic!("Dynamic EQ");
        };
        assert_eq!(bands[1].shape, DynEqShape::Peak);
        assert_eq!(bands[1].shelf_slope, 1.0);
        assert_eq!(bands[1].placement, DynEqPlacement::Stereo);
        // Reset must address newly added bands, beyond the four-band default.
        controller.set_plugin_param(index, 0, 8.0);
        controller.set_plugin_param(index, 171, 3.5);
        controller.set_plugin_param(index, 173, -45.0);
        controller.set_plugin_param(index, 1, -31.0);
        controller.reset_plugin_param_by_node_id(node, 171);
        controller.reset_plugin_param(index, 173);
        let PluginSettings::DynamicEq { bands, .. } =
            &controller.graph.get_plugin(index).unwrap().settings
        else {
            panic!("Dynamic EQ");
        };
        assert_eq!(bands[7].q, 1.0);
        assert_eq!(bands[7].band_threshold, -31.0);
    }
}

#[cfg(feature = "dev-api")]
#[test]
fn dynamic_eq_new_band_fields_have_sparse_query_metadata() {
    use super::dev_api::queries::plugin_query;
    let mut controller = PluginController::new();
    let index = controller.graph.user_plugin_insert_index();
    controller.try_add_plugin(&PluginType::DynamicEq).unwrap();
    let prefix = format!("plugins.plugin.{index}");
    assert_eq!(
        plugin_query(&controller.graph, &format!("{prefix}.param_count")).unwrap(),
        serde_json::json!(48)
    );
    let indices = plugin_query(&controller.graph, &format!("{prefix}.param_indices")).unwrap();
    assert_eq!(indices.as_array().unwrap().len(), 48);
    for (param, name, value) in [
        (117, "Shape", 2.0),
        (118, "Shelf Slope", 0.43),
        (119, "Placement", 4.0),
    ] {
        controller.set_plugin_param(index, param, value);
        assert_eq!(
            plugin_query(&controller.graph, &format!("{prefix}.param.{param}.name")).unwrap(),
            serde_json::json!(name)
        );
        let read = plugin_query(&controller.graph, &format!("{prefix}.param.{param}.value"))
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((read - value).abs() < 1e-6);
    }
    assert!(plugin_query(&controller.graph, &format!("{prefix}.param.147.name")).is_err());
}

#[test]
fn linear_phase_eq_dense_addresses_preserve_band_stride_and_globals() {
    for by_node in [false, true] {
        let mut controller = PluginController::new();
        let node = controller
            .graph
            .add_user_plugin(&PluginType::LinearPhaseEq)
            .unwrap();
        let index = controller.graph.linear_index_of_node(node).unwrap();
        for (param, value) in [
            (100, 10.0),
            (101, 2.0),
            (102, 1.0),
            (103, 1.0),
            (104, 65.0),
            (105, 1.0),
            (46, 750.0),
            (47, 2.35),
            (48, -3.5),
            (49, 0.0),
            (209, 4.0),
        ] {
            let effect = if by_node {
                controller.set_plugin_param_by_node_id(node, param, value)
            } else {
                controller.set_plugin_param(index, param, value)
            };
            assert!(matches!(effect, PluginUpdateEffect::Structural), "{param}");
        }
        let settings = &controller.graph.get_plugin(index).unwrap().settings;
        let PluginSettings::LinearPhaseEq {
            num_filters,
            filters,
            mix,
            ..
        } = settings
        else {
            unreachable!()
        };
        assert_eq!(super::linear_phase::value(settings, 105), Some(1.0));
        assert_eq!(*num_filters, 10.0);
        assert_eq!(filters.len(), 10);
        assert!((*mix - 0.65).abs() < 1e-6);
        assert_eq!(filters[9].frequency, 750.0);
        assert_eq!(filters[9].q, 2.35);
        assert_eq!(filters[9].gain_db, -3.5);
        assert!(filters[9].muted);
        assert_eq!(super::linear_phase::value(settings, 209), Some(4.0));
        let before = serde_json::to_value(&controller.graph).unwrap();
        assert!(matches!(
            controller.set_plugin_param(index, 210, 1.0),
            PluginUpdateEffect::None
        ));
        assert_eq!(serde_json::to_value(&controller.graph).unwrap(), before);
        controller.editing_plugin_index = Some(index);
        controller.plugin_param_selection = 47;
        controller.adjust_selected_param(1.0);
        assert!(
            (super::linear_phase::value(&controller.graph.get_plugin(index).unwrap().settings, 47)
                .unwrap()
                - 2.40)
                .abs()
                < 1e-6
        );
        controller.plugin_param_selection = 104;
        controller.adjust_selected_param(1.0);
        assert!(
            (super::linear_phase::value(
                &controller.graph.get_plugin(index).unwrap().settings,
                104
            )
            .unwrap()
                - 0.66)
                .abs()
                < 1e-6
        );
        for param in [46, 47, 48, 49, 209, 104, 105] {
            let effect = if by_node {
                controller.reset_plugin_param_by_node_id(node, param)
            } else {
                controller.reset_plugin_param(index, param)
            };
            assert!(
                matches!(effect, PluginUpdateEffect::Structural),
                "reset {param}"
            );
            let settings = &controller.graph.get_plugin(index).unwrap().settings;
            assert_eq!(
                super::linear_phase::value(settings, param),
                Some(super::linear_phase::spec(param).unwrap().default_f64())
            );
        }
        // A band type edit must not alias the filter-count resize side effect.
        controller.set_plugin_param(index, 0, 1.0);
        assert_eq!(
            super::linear_phase::value(&controller.graph.get_plugin(index).unwrap().settings, 100),
            Some(10.0)
        );
        controller.set_plugin_param(index, 100, 3.0);
        assert_eq!(
            crate::get_param_count(&controller.graph.get_plugin(index).unwrap().settings),
            24
        );
    }
}

#[cfg(feature = "dev-api")]
#[test]
fn linear_phase_eq_dense_query_addresses_cover_globals_and_placements() {
    use super::dev_api::queries::plugin_query;
    let mut controller = PluginController::new();
    let node = controller
        .graph
        .add_user_plugin(&PluginType::LinearPhaseEq)
        .unwrap();
    let index = controller.graph.linear_index_of_node(node).unwrap();
    controller.set_plugin_param(index, 100, 2.0);
    let key = format!("plugins.plugin.{index}");
    assert_eq!(
        plugin_query(&controller.graph, &format!("{key}.param_count")).unwrap(),
        serde_json::json!(18)
    );
    assert_eq!(
        plugin_query(&controller.graph, &format!("{key}.param_indices")).unwrap(),
        serde_json::json!([
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 100, 101, 102, 103, 104, 105, 200, 201
        ])
    );
    for (param, name, input, expected) in [
        (101, "FIR Length", 3.0, 3.0),
        (102, "Phase Mode", 1.0, 1.0),
        (103, "Auto Gain", 1.0, 1.0),
        (104, "Mix", 42.0, 0.42),
        (105, "Automatic Accuracy", 1.0, 1.0),
        (201, "Placement", 5.0, 5.0),
    ] {
        controller.set_plugin_param(index, param, input);
        assert_eq!(
            plugin_query(&controller.graph, &format!("{key}.param.{param}.name")).unwrap(),
            serde_json::json!(name)
        );
        let value = plugin_query(&controller.graph, &format!("{key}.param.{param}.value"))
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((value - expected).abs() < 1e-6);
    }
    assert!(plugin_query(&controller.graph, &format!("{key}.param.202.name")).is_err());
}
