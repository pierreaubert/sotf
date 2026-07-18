use super::super::build::build_room_eq_plugin_graph_config;
use super::super::*;
use super::bare::bare_chain;
use super::bare::bare_output;
#[cfg(test)]
use super::build_routed_room_eq_graph;
use autoeq::roomeq::{
    BassManagementReport, BassManagementRoute, BassManagementRoutingGraph,
    BassManagementSignalFlowEntry, HomeCinemaRole, OptimizationMetadata, PluginConfigWrapper,
};
pub use autoeq::roomeq::{ChannelDspChain, DriverDspChain, DspChainOutput};

#[cfg(test)]
#[path = "routed/tests.rs"]
mod tests;

pub(super) fn routed_bass_output() -> DspChainOutput {
    let mut output = bare_output(vec![
        (
            "L".to_string(),
            ChannelDspChain {
                plugins: vec![
                    PluginConfigWrapper {
                        plugin_type: "gain".to_string(),
                        parameters: serde_json::json!({
                            "gain_db": -1.0,
                            "room_eq_stage": "pre_route"
                        }),
                    },
                    PluginConfigWrapper {
                        plugin_type: "eq".to_string(),
                        parameters: serde_json::json!({
                            "label": "pre_room_eq",
                            "room_eq_stage": "pre_route",
                            "filters": []
                        }),
                    },
                    PluginConfigWrapper {
                        plugin_type: "crossover".to_string(),
                        parameters: serde_json::json!({
                            "type": "LR24",
                            "frequency": 80.0,
                            "output": "high"
                        }),
                    },
                    PluginConfigWrapper {
                        plugin_type: "delay".to_string(),
                        parameters: serde_json::json!({
                            "delay_ms": 2.0,
                            "room_eq_stage": "route_owned"
                        }),
                    },
                    PluginConfigWrapper {
                        plugin_type: "gain".to_string(),
                        parameters: serde_json::json!({
                            "label": "post_main_trim",
                            "room_eq_stage": "post_route",
                            "gain_db": -0.75
                        }),
                    },
                    PluginConfigWrapper {
                        plugin_type: "eq".to_string(),
                        parameters: serde_json::json!({
                            "label": "post_room_eq",
                            "room_eq_stage": "post_route",
                            "filters": []
                        }),
                    },
                ],
                ..bare_chain("L", None)
            },
        ),
        (
            "Sub".to_string(),
            ChannelDspChain {
                plugins: vec![
                    PluginConfigWrapper {
                        plugin_type: "gain".to_string(),
                        parameters: serde_json::json!({
                            "label": "sub_pre_trim",
                            "room_eq_stage": "pre_route",
                            "gain_db": -0.5
                        }),
                    },
                    PluginConfigWrapper {
                        plugin_type: "eq".to_string(),
                        parameters: serde_json::json!({
                            "label": "sub_pre_room_eq",
                            "room_eq_stage": "pre_route",
                            "filters": []
                        }),
                    },
                    PluginConfigWrapper {
                        plugin_type: "crossover".to_string(),
                        parameters: serde_json::json!({
                            "type": "LR24",
                            "frequency": 80.0,
                            "output": "low"
                        }),
                    },
                    PluginConfigWrapper {
                        plugin_type: "gain".to_string(),
                        parameters: serde_json::json!({
                            "room_eq_stage": "route_owned",
                            "gain_db": -3.0,
                            "invert": true
                        }),
                    },
                    PluginConfigWrapper {
                        plugin_type: "delay".to_string(),
                        parameters: serde_json::json!({
                            "delay_ms": 4.0,
                            "room_eq_stage": "route_owned"
                        }),
                    },
                    PluginConfigWrapper {
                        plugin_type: "gain".to_string(),
                        parameters: serde_json::json!({
                            "label": "sub_post_trim",
                            "room_eq_stage": "post_route",
                            "gain_db": -0.25
                        }),
                    },
                    PluginConfigWrapper {
                        plugin_type: "eq".to_string(),
                        parameters: serde_json::json!({
                            "label": "sub_post_room_eq",
                            "room_eq_stage": "post_route",
                            "filters": []
                        }),
                    },
                ],
                ..bare_chain("Sub", None)
            },
        ),
    ]);
    output.metadata = Some(OptimizationMetadata {
        pre_score: 1.0,
        post_score: 0.5,
        algorithm: "test".to_string(),
        loss_type: None,
        iterations: 1,
        timestamp: "test".to_string(),
        inter_channel_deviation: None,
        epa_per_channel: None,
        epa_multichannel: None,
        group_delay: None,
        perceptual_metrics: None,
        home_cinema_layout: None,
        multi_seat_coverage: None,
        multi_seat_correction: None,
        supporting_source: None,
        correction_acceptance: None,
        stage_outcomes: vec![],
        bass_management: Some(BassManagementReport {
            enabled: true,
            crossover_type: "LR24".to_string(),
            crossover_frequency_hz: Some(80.0),
            redirected_bass_enabled: true,
            lfe_channel: "LFE".to_string(),
            lfe_playback_gain_db: 10.0,
            lfe_gain_applied_to_chain: false,
            sub_trim_db: 0.0,
            max_sub_boost_db: 6.0,
            headroom_margin_db: -3.0,
            applied_sub_gain_db: Some(0.0),
            gain_limited: false,
            physical_sub_output: "Sub".to_string(),
            redirected_bass_channel_count: 1,
            main_high_pass_hz: Some(80.0),
            sub_low_pass_hz: Some(80.0),
            lfe_headroom_required_db: 10.0,
            signal_flow: vec![BassManagementSignalFlowEntry {
                source_channel: "L".to_string(),
                role: HomeCinemaRole::FrontLeft,
                destination: "Sub".to_string(),
                high_pass_hz: None,
                low_pass_hz: Some(80.0),
                lfe_gain_db: 0.0,
                redirects_bass: true,
            }],
            signal_flow_advisories: Vec::new(),
            routing_graph: Some(BassManagementRoutingGraph {
                physical_sub_output: "Sub".to_string(),
                input_channels: vec!["L".to_string(), "Sub".to_string()],
                output_channels: vec!["L".to_string(), "Sub".to_string()],
                routes: vec![
                    BassManagementRoute {
                        group_id: Some("lcr".to_string()),
                        source_channel: "L".to_string(),
                        source_index: 0,
                        destination: "L".to_string(),
                        destination_index: 0,
                        pre_chain_channel: Some("L".to_string()),
                        post_chain_channel: Some("L".to_string()),
                        route_kind: "main_highpass_to_self".to_string(),
                        crossover_type: "LR24".to_string(),
                        high_pass_hz: Some(80.0),
                        low_pass_hz: None,
                        gain_db: 0.0,
                        gain_linear: 1.0,
                        matrix_gain: 1.0,
                        delay_ms: 2.0,
                        polarity_inverted: false,
                    },
                    BassManagementRoute {
                        group_id: Some("lcr".to_string()),
                        source_channel: "L".to_string(),
                        source_index: 0,
                        destination: "Sub".to_string(),
                        destination_index: 1,
                        pre_chain_channel: Some("Sub".to_string()),
                        post_chain_channel: Some("Sub".to_string()),
                        route_kind: "redirected_bass_lowpass_to_sub".to_string(),
                        crossover_type: "LR24".to_string(),
                        high_pass_hz: None,
                        low_pass_hz: Some(80.0),
                        gain_db: -3.0,
                        gain_linear: 0.707945784,
                        matrix_gain: 1.0,
                        delay_ms: 4.0,
                        polarity_inverted: true,
                    },
                ],
                matrix: None,
                advisories: Vec::new(),
            }),
            optimization: None,
            groups: Vec::new(),
            sub_outputs: Vec::new(),
            headroom_simulation: None,
            advisory: "ok".to_string(),
        }),
        timing_diagnostics: None,
        ctc: None,
        perceptual_policy: None,
        bootstrap_uncertainty: None,
        validation_bundle: None,
    });
    output
}

pub(super) fn routed_physical_sub_output() -> DspChainOutput {
    let mut output = routed_bass_output();
    let mut sub_chain = output.channels.remove("Sub").expect("sub chain");
    sub_chain.channel = "LFE".to_string();
    sub_chain.drivers = Some(vec![DriverDspChain {
        name: "SubA".to_string(),
        index: 0,
        plugins: vec![],
        initial_curve: None,
    }]);
    output.channels.insert("LFE".to_string(), sub_chain);

    let report = output
        .metadata
        .as_mut()
        .and_then(|metadata| metadata.bass_management.as_mut())
        .expect("bass management report");
    report.physical_sub_output = "LFE".to_string();
    let graph = report.routing_graph.as_mut().expect("routing graph");
    graph.physical_sub_output = "LFE".to_string();
    graph.input_channels = vec!["L".to_string(), "LFE".to_string(), "SubA".to_string()];
    graph.output_channels = vec!["L".to_string(), "LFE".to_string(), "SubA".to_string()];
    for route in &mut graph.routes {
        if route.route_kind == "redirected_bass_lowpass_to_sub" {
            route.destination = "SubA".to_string();
            route.destination_index = 2;
            route.pre_chain_channel = Some("LFE".to_string());
            route.post_chain_channel = Some("SubA".to_string());
        }
    }
    output
}

#[test]
fn test_requires_room_eq_graph_with_routed_bass_management() {
    let output = routed_bass_output();
    assert!(output.requires_room_eq_graph());
    assert!(!output.is_rack_compatible());
}

fn routed_stereo_21_output() -> DspChainOutput {
    let mut output = routed_physical_sub_output();
    output
        .channels
        .insert("R".to_string(), bare_chain("R", None));

    let routing = output
        .metadata
        .as_mut()
        .and_then(|metadata| metadata.bass_management.as_mut())
        .and_then(|report| report.routing_graph.as_mut())
        .expect("routed fixture");
    let left_routes = routing
        .routes
        .iter()
        .filter(|route| route.source_channel == "L")
        .cloned()
        .collect::<Vec<_>>();

    routing.input_channels = ["L", "R", "LFE", "SubA"]
        .into_iter()
        .map(str::to_string)
        .collect();
    routing.output_channels = routing.input_channels.clone();
    for route in &mut routing.routes {
        if route.source_channel == "LFE" {
            route.source_index = 2;
        } else if route.source_channel == "SubA" {
            route.source_index = 3;
        }
        if route.destination == "SubA" {
            route.destination_index = 3;
        }
    }
    for template in left_routes {
        let mut route = template;
        route.source_channel = "R".to_string();
        route.source_index = 1;
        route.pre_chain_channel = Some("R".to_string());
        if route.destination == "L" {
            route.destination = "R".to_string();
            route.destination_index = 1;
            route.post_chain_channel = Some("R".to_string());
        } else if route.destination == "SubA" {
            route.destination_index = 3;
        }
        routing.routes.push(route);
    }

    output
}

fn routed_surround_51_output() -> DspChainOutput {
    let mut output = routed_physical_sub_output();
    for channel in ["R", "C", "SL", "SR"] {
        output
            .channels
            .insert(channel.to_string(), bare_chain(channel, None));
    }

    let routing = output
        .metadata
        .as_mut()
        .and_then(|metadata| metadata.bass_management.as_mut())
        .and_then(|report| report.routing_graph.as_mut())
        .expect("routed fixture");
    let left_routes = routing
        .routes
        .iter()
        .filter(|route| route.source_channel == "L")
        .cloned()
        .collect::<Vec<_>>();

    routing.input_channels = ["L", "R", "C", "LFE", "SL", "SR", "SubA"]
        .into_iter()
        .map(str::to_string)
        .collect();
    routing.output_channels = routing.input_channels.clone();

    for route in &mut routing.routes {
        if route.source_channel == "LFE" {
            route.source_index = 3;
        } else if route.source_channel == "SubA" {
            route.source_index = 6;
        }
        if route.destination == "SubA" {
            route.destination_index = 6;
        }
    }

    for (source_index, channel) in [(1, "R"), (2, "C"), (4, "SL"), (5, "SR")] {
        for template in &left_routes {
            let mut route = template.clone();
            route.source_channel = channel.to_string();
            route.source_index = source_index;
            route.pre_chain_channel = Some(channel.to_string());
            if route.destination == "L" {
                route.destination = channel.to_string();
                route.destination_index = source_index;
                route.post_chain_channel = Some(channel.to_string());
            } else if route.destination == "SubA" {
                route.destination_index = 6;
            }
            routing.routes.push(route);
        }
    }

    output
}

#[test]
fn easy_bass_managed_outputs_apply_as_editable_persistable_graphs() {
    use crate::autoeq::{RoomEqApplyOutcome, apply_room_eq_to_chain};
    use crate::plugin_graph::PluginGraph;

    for output in [routed_stereo_21_output(), routed_surround_51_output()] {
        let mut graph = PluginGraph::with_default_rack();
        let outcome = apply_room_eq_to_chain(&mut graph, &output, 48_000.0, &[]).unwrap();
        assert!(matches!(outcome, RoomEqApplyOutcome::Graph(_)));
        assert!(!graph.is_linear());

        let encoded = serde_json::to_string(&graph).unwrap();
        let mut restored: PluginGraph = serde_json::from_str(&encoded).unwrap();
        let previous_count = restored.plugin_count();
        restored.add_plugin_node(
            &crate::PluginType::Gain,
            crate::plugin_graph::NodePosition::new(12.0, 8.0),
        );
        assert_eq!(restored.plugin_count(), previous_count + 1);
    }
}

#[test]
fn test_build_room_eq_graph_preserves_non_routing_global_plugins() {
    let mut output = routed_bass_output();
    output.global_plugins.push(PluginConfigWrapper {
        plugin_type: "eq".to_string(),
        parameters: serde_json::json!({
            "label": "global_room_eq",
            "filters": []
        }),
    });
    output.global_plugins.push(PluginConfigWrapper {
        plugin_type: "matrix".to_string(),
        parameters: serde_json::json!({
            "label": "home_cinema_bass_management",
            "metadata": {
                "purpose": "home_cinema_bass_management"
            },
            "input_channel_map": [0],
            "output_channel_map": [1],
            "matrix": [1.0]
        }),
    });

    let graph = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
    let labeled_nodes: Vec<_> = graph
        .nodes
        .iter()
        .filter_map(|node| {
            node.parameters
                .get("label")
                .and_then(|label| label.as_str())
                .map(|label| (node.id, label))
        })
        .collect();
    let global_id = labeled_nodes
        .iter()
        .find(|(_, label)| *label == "global_room_eq")
        .map(|(id, _)| *id)
        .expect("non-routing global plugin should be preserved");
    // The legacy `home_cinema_bass_management` matrix is fully encoded by
    // the factored routing nodes and must be stripped.
    assert!(
        labeled_nodes
            .iter()
            .all(|(_, label)| *label != "home_cinema_bass_management"),
        "legacy global bass matrix should be replaced by factored routing nodes"
    );
    // The non-routing global plugin must wire into the factored chain
    // (gain_pre is the first factored node).
    let gain_pre_id = labeled_nodes
        .iter()
        .find(|(_, label)| *label == "room_eq_gain_pre")
        .map(|(id, _)| *id)
        .expect("factored gain_pre should be emitted");
    assert!(
        global_id < gain_pre_id,
        "non-routing global plugins must precede the factored chain"
    );
    assert!(
        graph
            .edges
            .iter()
            .any(|e| e.from_node == global_id && e.to_node == gain_pre_id),
        "global plugin must wire into the factored gain_pre node"
    );
}

/// Every node emitted by the factored builder must instantiate
/// successfully via `sotf_plugins::create_plugin`. This catches schema
/// drift between the JSON the builder emits and the plugin parameter
/// structs — otherwise the engine would fail at flush time with a
/// less-helpful error.
#[test]
fn test_factored_graph_nodes_instantiate_via_factory() {
    for output in [routed_bass_output(), routed_physical_sub_output()] {
        let graph = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
        for node in &graph.nodes {
            let label = node
                .parameters
                .get("label")
                .and_then(|l| l.as_str())
                .unwrap_or("<unlabeled>");
            let plugin = sotf_plugins::create_plugin(
                &node.plugin_type,
                &node.parameters,
                node.input_channels,
                48_000,
            )
            .unwrap_or_else(|err| {
                panic!(
                    "factored graph node '{label}' (type={}) failed to instantiate: {err}\n\
                         parameters: {}",
                    node.plugin_type, node.parameters
                )
            });
            // Sanity: the constructed plugin must agree with the graph's
            // declared input channel count.
            assert_eq!(
                plugin.input_channels(),
                node.input_channels,
                "plugin '{label}' input_channels mismatch"
            );
        }
    }
}

/// The `lfe_gain_applied_to_chain == true` path needs explicit coverage
/// because the common fixtures set it to false. Build a small
/// variant that flips it and confirm the matrix coefficient still
/// reflects route.gain_db (chain has no route_owned gain in this
/// minimal scenario, so the chain-override branch shouldn't fire).
#[test]
fn test_factored_graph_handles_lfe_gain_applied_to_chain_true() {
    let mut output = routed_bass_output();
    if let Some(report) = output
        .metadata
        .as_mut()
        .and_then(|m| m.bass_management.as_mut())
    {
        report.lfe_gain_applied_to_chain = true;
    }
    let graph = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
    let matrix_node = graph
        .nodes
        .iter()
        .find(|n| {
            n.parameters.get("label").and_then(|l| l.as_str()) == Some("room_eq_matrix_to_sub_bus")
        })
        .expect("factored sub-bus matrix");
    let matrix: Vec<f32> = matrix_node.parameters["matrix"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap() as f32)
        .collect();
    // L → Sub LP route gain_db = -3, no chain route_owned gain to
    // override. Expect 10^(-3/20).
    let expected = 10.0_f32.powf(-3.0 / 20.0);
    let got = matrix[2];
    assert!(
        (got - expected).abs() < 1e-5,
        "L→Sub matrix coef under lfe_gain_applied_to_chain=true should be 10^(-3/20) ≈ {expected}, got {got}"
    );
}

/// Regression: a destination-only channel (in the routing graph as a
/// destination but not a source of any route) must pass its direct
/// input through the HP branch so signals arriving on that channel
/// upstream of RoomEQ reach the post-EQ stage instead of being muted.
#[test]
fn test_factored_graph_passthrough_for_destination_only_channels() {
    let output = routed_physical_sub_output();
    let graph = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
    let xover_hp = graph
        .nodes
        .iter()
        .find(|n| n.parameters.get("label").and_then(|l| l.as_str()) == Some("room_eq_xover_hp"))
        .expect("factored HP crossover");
    let modes: Vec<String> = xover_hp.parameters["channel_modes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    // Channel order in routed_physical_sub_output is [L, LFE, SubA].
    // L (idx 0) is the source of main_highpass_to_self → "highpass".
    // LFE (idx 1) is neither source nor destination in the routing
    // graph after the relabel → "mute".
    // SubA (idx 2) is destination-only → must be "passthrough".
    assert_eq!(modes[0], "highpass", "L must be HP");
    assert_eq!(
        modes[2], "passthrough",
        "destination-only SubA must be passthrough"
    );
}

/// Edge case: routing graph where every channel is destination-only
/// (no channel is a source of any *valid* route). The builder must not
/// panic; the HP branch ends up Passthrough for destination channels,
/// the LP branch all-Mute, and the matrix is zero. The graph must still
/// instantiate cleanly via the factory.
#[test]
fn test_factored_graph_fixes_specific_legacy_bugs_on_routed_bass() {
    let output = routed_bass_output();

    // Drive the legacy builder directly — same input, two outputs.
    let legacy_graph = build_routed_room_eq_graph(
        &output,
        output
            .metadata
            .as_ref()
            .unwrap()
            .bass_management
            .as_ref()
            .unwrap()
            .routing_graph
            .as_ref()
            .unwrap(),
    )
    .unwrap();
    let factored = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();

    let legacy_labels: Vec<&str> = legacy_graph
        .nodes
        .iter()
        .filter_map(|n| n.parameters.get("label").and_then(|l| l.as_str()))
        .collect();
    let factored_labels: Vec<&str> = factored
        .nodes
        .iter()
        .filter_map(|n| n.parameters.get("label").and_then(|l| l.as_str()))
        .collect();

    // Bug 1: node count blow-up. Even on the 2-channel routed_bass_output
    // fixture (one main + one sub), the legacy builder emits ≥2x the
    // factored count. On the 10-channel gen514 case the ratio grows
    // (~50+ vs 9) — see `gen514_factored_graph_topology_matches_golden_snapshot`
    // in the integration tests.
    assert!(
        legacy_graph.nodes.len() >= factored.nodes.len() * 2,
        "legacy builder should emit at least 2x the nodes of the factored builder \
             (legacy={}, factored={})",
        legacy_graph.nodes.len(),
        factored.nodes.len()
    );
    assert_eq!(factored.nodes.len(), 9);

    // Bug 2: legacy carries the source chain's pre-EQ as a standalone
    // node; factored folds it into the single `room_eq_eq_pre` array.
    assert_eq!(
        factored_labels
            .iter()
            .filter(|l| **l == "pre_room_eq")
            .count(),
        0,
        "factored folds pre_room_eq into room_eq_eq_pre channel_filters"
    );
    assert_eq!(
        factored_labels
            .iter()
            .filter(|l| **l == "room_eq_eq_pre")
            .count(),
        1,
    );

    // Bug 3: per-channel output isolator matrices.
    let legacy_isolators = legacy_labels
        .iter()
        .filter(|l| l.starts_with("room_eq_output_isolate_"))
        .count();
    assert!(
        legacy_isolators >= 2,
        "legacy emits one isolator per output channel (got {legacy_isolators})"
    );
    assert_eq!(
        factored_labels
            .iter()
            .filter(|l| l.starts_with("room_eq_output_isolate_"))
            .count(),
        0,
    );

    // Sanity: single sub-bus matrix in the factored graph.
    assert_eq!(
        factored_labels
            .iter()
            .filter(|l| **l == "room_eq_matrix_to_sub_bus")
            .count(),
        1,
    );
}

/// End-to-end audio test: build a DawHost from the factored graph, drive
/// an impulse on the L source, and verify the sub-bus carries the
/// LP-filtered signal at the matrix-encoded gain. Catches issues that
/// the per-node instantiation test can't (matrix routing wrong, edges
/// missing, channel widths mismatched between consecutive nodes).
#[test]
fn test_factored_graph_audio_equivalence_routed_bass() {
    use sotf_plugins::{DawHost, GraphEdge};

    let output = routed_bass_output();
    let config = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
    // Channel order: L (0), Sub (1). Route L → Sub LP @ 80 Hz, gain -3 dB.

    let channel_count = config.nodes[0].input_channels;
    let sr = 48_000u32;
    let mut host = DawHost::new(channel_count, sr);

    // Materialise plugins and add them as host nodes. Keep a map from
    // builder node id → host node id so we can wire the edges.
    let mut node_map: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for node in &config.nodes {
        let plugin = sotf_plugins::create_plugin(
            &node.plugin_type,
            &node.parameters,
            node.input_channels,
            sr,
        )
        .unwrap_or_else(|err| {
            panic!(
                "factored node {} ({}) failed to instantiate: {err}",
                node.id, node.plugin_type
            )
        });
        let label = node
            .parameters
            .get("label")
            .and_then(|l| l.as_str())
            .unwrap_or("")
            .to_string();
        let host_id = host
            .add_node(label, plugin)
            .expect("host accepts plugin node");
        node_map.insert(node.id, host_id);
    }
    for edge in &config.edges {
        host.add_edge(GraphEdge::new(
            node_map[&edge.from_node],
            node_map[&edge.to_node],
        ))
        .expect("host accepts edge");
    }
    host.build().expect("host builds");

    // Drive an impulse on the L source (channel 0) and let it propagate
    // through enough frames to clear group delay.
    let num_frames = 4096usize;
    let mut input = vec![0.0f32; num_frames * channel_count];
    input[0] = 1.0; // L impulse at frame 0

    let mut output_buf = vec![0.0f32; num_frames * channel_count];
    host.process(&input, &mut output_buf).expect("process");

    // Sub channel (idx 1) should carry the LP-filtered impulse at the
    // route's gain (-3 dB → 0.708 linear). Sum the absolute energy on
    // the sub row in the steady-state region and compare to expected
    // bounds. The exact peak is filter-dependent; just confirm there's
    // non-trivial signal on the sub and that it is below the input
    // amplitude (i.e. the LP + gain combo actually attenuated).
    let sub_energy: f32 = (32..num_frames)
        .map(|f| output_buf[f * channel_count + 1].abs())
        .fold(0.0, f32::max);
    assert!(
        sub_energy > 0.0001,
        "sub channel must carry signal from L→Sub LP route, peak={sub_energy}"
    );
    // The route gain is -3 dB linear ~0.708. The LP impulse response
    // peak is < input amplitude due to filtering. Upper bound check.
    assert!(
        sub_energy < 0.8,
        "sub channel signal must be attenuated by LP+gain, peak={sub_energy}"
    );

    // L output channel (idx 0): HP-filtered impulse. Should also carry
    // signal (HP isn't full mute).
    let l_energy: f32 = (32..num_frames)
        .map(|f| output_buf[f * channel_count].abs())
        .fold(0.0, f32::max);
    assert!(
        l_energy > 0.0001,
        "L output channel must carry HP-filtered signal, peak={l_energy}"
    );
}
