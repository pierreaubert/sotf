use super::PluginGraph;
use super::node_position::NodePosition;
use super::types::NodeRole;
use super::types::SpecialNodeType;
use math_audio_iir_fir::BiquadFilterType;
use sotf_audio::plugins::EQFilter;
use sotf_audio::plugins::{PluginSettings, PluginType};

#[test]
fn chain_autogain_target_limits_accumulated_correction() {
    let mut graph = PluginGraph::new();
    graph.set_chain_auto_gain(Some(4.0));
    assert_eq!(graph.chain_auto_gain_target(20.0), Some(20.0));
    assert_eq!(graph.chain_auto_gain_target(-100.0), Some(-60.0));
    assert_eq!(graph.chain_auto_gain_target(-1.5), Some(2.5));
    assert_eq!(graph.chain_auto_gain_target(f64::NAN), None);
    assert_eq!(graph.chain_auto_gain_target(f64::INFINITY), None);
    assert_eq!(graph.chain_auto_gain_db(), Some(4.0));
}

#[test]
fn test_add_and_remove_nodes() {
    let mut graph = PluginGraph::new();

    let node_id = graph
        .add_plugin_node(&PluginType::Gain, NodePosition::new(100.0, 100.0))
        .unwrap();
    assert!(graph.nodes.contains_key(&node_id));
    assert_eq!(graph.nodes.len(), 1);

    graph.remove_node(node_id);
    assert!(!graph.nodes.contains_key(&node_id));
    assert!(graph.nodes.is_empty());
}

#[test]
fn test_connections() {
    let mut graph = PluginGraph::new();

    let node1 = graph
        .add_plugin_node(&PluginType::EQ, NodePosition::new(100.0, 100.0))
        .unwrap();
    let node2 = graph
        .add_plugin_node(&PluginType::Gain, NodePosition::new(300.0, 100.0))
        .unwrap();

    // Add connection
    let conn_id = graph.add_connection(node1, 0, node2, 0).unwrap();
    assert_eq!(graph.connections.len(), 1);

    // Remove connection
    graph.remove_connection(conn_id);
    assert!(graph.connections.is_empty());
}

#[test]
fn test_cycle_detection() {
    let mut graph = PluginGraph::new();

    let node1 = graph
        .add_plugin_node(&PluginType::EQ, NodePosition::new(100.0, 100.0))
        .unwrap();
    let node2 = graph
        .add_plugin_node(&PluginType::Gain, NodePosition::new(300.0, 100.0))
        .unwrap();

    // Valid connection
    graph.add_connection(node1, 0, node2, 0).unwrap();

    // This would create a cycle
    let result = graph.add_connection(node2, 0, node1, 0);
    assert!(result.is_err());
}

#[test]
fn test_topological_sort() {
    let mut graph = PluginGraph::new();

    let node1 = graph
        .add_plugin_node(&PluginType::EQ, NodePosition::new(100.0, 100.0))
        .unwrap();
    let node2 = graph
        .add_plugin_node(&PluginType::Gain, NodePosition::new(300.0, 100.0))
        .unwrap();
    let node3 = graph
        .add_plugin_node(&PluginType::Compressor, NodePosition::new(500.0, 100.0))
        .unwrap();

    graph.add_connection(node1, 0, node2, 0).unwrap();
    graph.add_connection(node2, 0, node3, 0).unwrap();

    let sorted = graph.topological_sort().unwrap();
    assert_eq!(sorted.len(), 3);

    // node1 should come before node2, node2 before node3
    let pos1 = sorted.iter().position(|&id| id == node1).unwrap();
    let pos2 = sorted.iter().position(|&id| id == node2).unwrap();
    let pos3 = sorted.iter().position(|&id| id == node3).unwrap();

    assert!(pos1 < pos2);
    assert!(pos2 < pos3);
}

#[test]
fn test_default_rack_is_linear() {
    let g = PluginGraph::with_default_rack();
    assert!(g.is_linear());

    let order = g.linear_order().unwrap();
    // Input + 4 plugins + Output = 6 nodes
    assert_eq!(order.len(), 6);

    // First node is Input, last is Output
    assert!(g.special_nodes.contains_key(&order[0]));
    assert!(g.special_nodes.contains_key(&order[5]));

    // Roles are in order
    let plugins: Vec<_> = g.plugins_linear().unwrap();
    assert_eq!(plugins.len(), 4);
    assert_eq!(plugins[0].role, NodeRole::InputMonitor);
    assert_eq!(plugins[1].role, NodeRole::ReplayGain);
    assert_eq!(plugins[2].role, NodeRole::Matrix);
    assert_eq!(plugins[3].role, NodeRole::OutputMonitor);
}

#[test]
fn test_upmixer_propagates_per_channel_connections_through_chain() {
    // Reproduces the user-visible bug where a chain that contained an
    // upmixer (or a Matrix configured 2→N) only routed 2 channels to
    // every plugin downstream, even though the downstream plugins'
    // input/output port counts had been propagated up to N. The fix
    // is for `update_channel_dependent_plugins` to re-establish a
    // per-channel connection for every port the chain now carries.
    let mut g = PluginGraph::with_default_rack();
    let upmixer_id = g.add_user_plugin(&PluginType::Upmixer).unwrap();

    let order = g.linear_order().expect("chain must stay linear");
    let upmixer_pos = order
        .iter()
        .position(|&id| id == upmixer_id)
        .expect("upmixer in chain");
    let next_id = order[upmixer_pos + 1];

    let upmixer_out = g.node_output_channels(upmixer_id);
    let next_in = g.node_input_channels(next_id);
    assert!(upmixer_out >= 5, "upmixer should produce >2 channels");
    assert_eq!(
        next_in, upmixer_out,
        "downstream plugin should adopt upmixer's channel count"
    );

    let output_monitor_id = g
        .node_id_for_role(NodeRole::OutputMonitor)
        .expect("default rack has output monitor");
    let output_id = g.output_node_id().expect("default rack has output node");
    assert_eq!(
        g.node_input_channels(output_monitor_id),
        upmixer_out,
        "output monitor input ports should follow the expanded chain"
    );
    assert_eq!(
        g.node_output_channels(output_monitor_id),
        upmixer_out,
        "output monitor output ports should pass through the expanded chain"
    );
    assert_eq!(
        g.node_input_channels(output_id),
        upmixer_out,
        "audio output input ports should follow the expanded chain"
    );

    // Every adjacent pair after the upmixer must have a connection for
    // every channel the expanded chain now carries.
    for pair in order[upmixer_pos..].windows(2) {
        let from = pair[0];
        let to = pair[1];
        let ports = g.node_output_channels(from).min(g.node_input_channels(to));
        assert_eq!(ports, upmixer_out, "expanded chain should stay wide");

        for port in 0..upmixer_out {
            assert!(
                g.connections.iter().any(|c| {
                    c.from_node == from
                        && c.to_node == to
                        && c.from_port == port
                        && c.to_port == port
                }),
                "missing connection on port {port} from {} to {}",
                g.node_display_name(from),
                g.node_display_name(to)
            );
        }
    }
}

#[test]
fn test_add_user_plugin() {
    let mut g = PluginGraph::with_default_rack();
    let eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();

    assert!(g.is_linear());
    let plugins = g.plugins_linear().unwrap();
    assert_eq!(plugins.len(), 5);

    // EQ should be between ReplayGain and Matrix
    assert_eq!(plugins[2].role, NodeRole::User);
    assert_eq!(plugins[2].id, eq_id);
    assert_eq!(plugins[3].role, NodeRole::Matrix);
}

#[test]
fn test_remove_user_plugin() {
    let mut g = PluginGraph::with_default_rack();
    let eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();

    g.remove_user_plugin(eq_id).unwrap();
    assert!(g.is_linear());
    let plugins = g.plugins_linear().unwrap();
    assert_eq!(plugins.len(), 4); // Back to default
}

#[test]
fn test_cannot_remove_permanent() {
    let mut g = PluginGraph::with_default_rack();
    let matrix_id = g.node_id_for_role(NodeRole::Matrix).unwrap();
    assert!(g.remove_user_plugin(matrix_id).is_err());
}

#[test]
fn test_move_user_plugin() {
    let mut g = PluginGraph::with_default_rack();
    let eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();
    let comp_id = g.add_user_plugin(&PluginType::Compressor).unwrap();

    // Order: InputMon, ReplayGain, EQ, Comp, Matrix, OutputMon
    let plugins = g.plugins_linear().unwrap();
    assert_eq!(plugins[2].id, eq_id);
    assert_eq!(plugins[3].id, comp_id);

    // Move comp up (swap with EQ)
    g.move_user_plugin_up(comp_id).unwrap();

    let plugins = g.plugins_linear().unwrap();
    assert_eq!(plugins[2].id, comp_id);
    assert_eq!(plugins[3].id, eq_id);
}

#[test]
fn test_non_linear_graph() {
    let mut g = PluginGraph::new();
    let input = g.add_special_node(SpecialNodeType::Input, NodePosition::new(0.0, 0.0), 2);
    let a = g
        .add_plugin_node(&PluginType::EQ, NodePosition::new(100.0, 0.0))
        .unwrap();
    let b = g
        .add_plugin_node(&PluginType::Gain, NodePosition::new(100.0, 100.0))
        .unwrap();
    let output = g.add_special_node(SpecialNodeType::Output, NodePosition::new(200.0, 50.0), 2);

    // Input splits to both A and B (non-linear)
    g.add_connection(input, 0, a, 0).unwrap();
    g.add_connection(input, 1, b, 0).unwrap();
    g.add_connection(a, 0, output, 0).unwrap();
    g.add_connection(b, 0, output, 1).unwrap();

    assert!(!g.is_linear());
    assert!(g.linear_order().is_none());
}

#[test]
fn test_to_plugin_graph_config_preserves_parallel_branches() {
    // Build the same parallel split as test_non_linear_graph; verify
    // `to_plugin_graph_config` emits both nodes (no flattening) and
    // drops the per-port duplicates into a single logical edge per pair.
    let mut g = PluginGraph::new();
    let input = g.add_special_node(SpecialNodeType::Input, NodePosition::new(0.0, 0.0), 2);
    let a = g
        .add_plugin_node(&PluginType::EQ, NodePosition::new(100.0, 0.0))
        .unwrap();
    let b = g
        .add_plugin_node(&PluginType::Gain, NodePosition::new(100.0, 100.0))
        .unwrap();
    let output = g.add_special_node(SpecialNodeType::Output, NodePosition::new(200.0, 50.0), 2);

    // Two stereo wires from Input to A; one wire from Input to B; two
    // from A to Output port pair, one from B to Output. This mimics how
    // the canvas stores per-port connections.
    g.add_connection(input, 0, a, 0).unwrap();
    g.add_connection(input, 1, a, 1).unwrap();
    g.add_connection(input, 0, b, 0).unwrap();
    g.add_connection(a, 0, output, 0).unwrap();
    g.add_connection(a, 1, output, 1).unwrap();
    g.add_connection(b, 0, output, 0).unwrap();

    let config = g.to_plugin_graph_config(48000.0);

    // Both plugin nodes are emitted; special I/O nodes are not.
    assert_eq!(
        config.nodes.len(),
        2,
        "expected only plugin nodes (no Input/Output), got {:?}",
        config
            .nodes
            .iter()
            .map(|n| n.plugin_type.as_str())
            .collect::<Vec<_>>()
    );

    // Edges between plugin nodes only — Input→A/Input→B/A→Output/B→Output
    // collapse to zero (touch special nodes); A↔B never connected.
    assert_eq!(
        config.edges.len(),
        0,
        "no plugin-to-plugin edges in this fan-out topology"
    );
}

#[test]
fn test_to_plugin_graph_config_dedups_per_port_edges() {
    // Two plugin nodes with a stereo wire (port 0→0, port 1→1). The
    // engine config carries logical edges only — both ports collapse
    // into one edge.
    let mut g = PluginGraph::new();
    let a = g
        .add_plugin_node(&PluginType::EQ, NodePosition::new(0.0, 0.0))
        .unwrap();
    let b = g
        .add_plugin_node(&PluginType::Gain, NodePosition::new(100.0, 0.0))
        .unwrap();
    g.add_connection(a, 0, b, 0).unwrap();
    g.add_connection(a, 1, b, 1).unwrap();

    let config = g.to_plugin_graph_config(48000.0);
    assert_eq!(config.nodes.len(), 2);
    assert_eq!(
        config.edges.len(),
        1,
        "two per-port wires should collapse into one logical edge"
    );
}

#[test]
fn test_to_plugin_graph_config_retains_disabled_routes() {
    let mut g = PluginGraph::new();
    let a = g
        .add_plugin_node(&PluginType::EQ, NodePosition::new(0.0, 0.0))
        .unwrap();
    let b = g
        .add_plugin_node(&PluginType::Gain, NodePosition::new(100.0, 0.0))
        .unwrap();
    g.add_connection(a, 0, b, 0).unwrap();
    g.nodes.get_mut(&b).unwrap().plugin.enabled = false;
    let config = g.to_plugin_graph_config(48000.0);
    assert_eq!(config.nodes.len(), 2);
    assert_eq!(config.edges.len(), 1);
    let bypass = config.nodes.iter().find(|node| node.bypassed).unwrap();
    assert_eq!(bypass.plugin_type, "gain");
    assert_eq!(bypass.parameters["gain_db"], 0.0);
    assert_eq!(bypass.input_channels, 2);
    assert_eq!(config.edges[0].to_node, bypass.id);
}

#[test]
fn test_toggle_plugin() {
    let mut g = PluginGraph::with_default_rack();
    let eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();

    assert!(g.nodes.get(&eq_id).unwrap().plugin.enabled);
    g.toggle_plugin(eq_id).unwrap();
    assert!(!g.nodes.get(&eq_id).unwrap().plugin.enabled);
}

#[test]
fn test_to_plugin_configs_ordering() {
    let mut g = PluginGraph::with_default_rack();
    let _eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();
    let _comp_id = g.add_user_plugin(&PluginType::Compressor).unwrap();

    let configs = g.to_plugin_configs(48000.0);

    // Input monitor first, then processing (ReplayGain disabled so skipped,
    // EQ, Compressor, Matrix), then output monitor
    // ReplayGain is disabled → not in configs
    // Order: InputMonitor, EQ, Compressor, Matrix, OutputMonitor
    let types: Vec<&str> = configs.iter().map(|c| c.plugin_type.as_str()).collect();

    // First should be loudness_monitor (input)
    assert_eq!(types[0], "loudness_monitor");
    // Processing plugins in order
    assert!(types.contains(&"eq"));
    assert!(types.contains(&"compressor"));
    assert!(types.contains(&"matrix"));
    // Last should be loudness_monitor (output, analyzer)
    assert_eq!(types[types.len() - 1], "loudness_monitor");

    // EQ should come before compressor
    let eq_pos = types.iter().position(|&t| t == "eq").unwrap();
    let comp_pos = types.iter().position(|&t| t == "compressor").unwrap();
    assert!(eq_pos < comp_pos);
}

#[test]
fn test_chain_auto_gain_injected_before_output_monitor() {
    let mut g = PluginGraph::with_default_rack();
    let _eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();
    g.set_chain_auto_gain(Some(-6.0));

    let configs = g.to_plugin_configs(48000.0);
    let types: Vec<&str> = configs.iter().map(|c| c.plugin_type.as_str()).collect();

    assert_eq!(types.last(), Some(&"loudness_monitor"));
    assert_eq!(types[types.len() - 2], "gain");
    assert_eq!(
        configs[types.len() - 2]
            .parameters
            .get("gain_db")
            .and_then(|v| v.as_f64()),
        Some(-6.0)
    );
    assert_eq!(g.chain_auto_gain_engine_index(), Some(types.len() - 2));
    assert_eq!(g.output_monitor_engine_index(), Some(types.len() - 1));
}

#[test]
fn test_get_engine_index() {
    let mut g = PluginGraph::with_default_rack();
    let eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();

    // Input monitor at engine index 0
    let im_id = g.node_id_for_role(NodeRole::InputMonitor).unwrap();
    assert_eq!(g.get_engine_index(im_id), Some(0));

    // EQ is the first processing plugin → index 1
    assert_eq!(g.get_engine_index(eq_id), Some(1));

    // Disabled plugin returns None
    g.toggle_plugin(eq_id).unwrap();
    assert_eq!(g.get_engine_index(eq_id), None);
}

#[test]
fn test_get_engine_index_by_linear_position() {
    let mut g = PluginGraph::with_default_rack();
    let _eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();

    // Linear position 0 = InputMonitor → engine index 0
    assert_eq!(g.get_engine_index_by_linear_position(0), Some(0));

    // Linear position 1 = ReplayGain (disabled) → None
    assert_eq!(g.get_engine_index_by_linear_position(1), None);

    // Linear position 2 = EQ → engine index 1
    assert_eq!(g.get_engine_index_by_linear_position(2), Some(1));
}

#[test]
fn test_role_engine_index_helpers() {
    let g = PluginGraph::with_default_rack();

    assert_eq!(g.input_monitor_engine_index(), Some(0));
    assert!(g.output_monitor_engine_index().is_some());
    assert!(g.matrix_engine_index().is_some());

    // Output monitor is an analyzer → comes after processing plugins
    let matrix_idx = g.matrix_engine_index().unwrap();
    let output_idx = g.output_monitor_engine_index().unwrap();
    assert!(output_idx > matrix_idx);
}

#[test]
fn test_move_plugin_drag_down() {
    // Regression: move_plugin must track by node_id, not re-lookup by index
    let mut g = PluginGraph::with_default_rack();
    let a_id = g.add_user_plugin(&PluginType::EQ).unwrap();
    let b_id = g.add_user_plugin(&PluginType::Compressor).unwrap();
    let c_id = g.add_user_plugin(&PluginType::Gain).unwrap();

    // Indices: ..., EQ(2), Comp(3), Gain(4), Matrix(5), ...
    let plugins = g.plugins_linear().unwrap();
    assert_eq!(plugins[2].id, a_id);
    assert_eq!(plugins[3].id, b_id);
    assert_eq!(plugins[4].id, c_id);

    // Move EQ from index 2 to index 4
    g.move_plugin(2, 4).unwrap();

    let plugins = g.plugins_linear().unwrap();
    assert_eq!(plugins[2].id, b_id);
    assert_eq!(plugins[3].id, c_id);
    assert_eq!(plugins[4].id, a_id);
}

#[test]
fn test_move_plugin_drag_up() {
    let mut g = PluginGraph::with_default_rack();
    let a_id = g.add_user_plugin(&PluginType::EQ).unwrap();
    let b_id = g.add_user_plugin(&PluginType::Compressor).unwrap();
    let c_id = g.add_user_plugin(&PluginType::Gain).unwrap();

    // Move Gain from index 4 to index 2
    g.move_plugin(4, 2).unwrap();

    let plugins = g.plugins_linear().unwrap();
    assert_eq!(plugins[2].id, c_id);
    assert_eq!(plugins[3].id, a_id);
    assert_eq!(plugins[4].id, b_id);
}

#[test]
fn test_user_spectrum_analyzer_stays_in_position() {
    // Regression: user-placed analyzers should not be moved to end
    let mut g = PluginGraph::with_default_rack();
    g.add_user_plugin(&PluginType::SpectrumAnalyzer).unwrap();
    g.add_user_plugin(&PluginType::Compressor).unwrap();

    let configs = g.to_plugin_configs(48000.0);
    let types: Vec<&str> = configs.iter().map(|c| c.plugin_type.as_str()).collect();

    // Spectrum should come before compressor (user ordering preserved)
    let spec_pos = types
        .iter()
        .position(|&t| t == "spectrum_analyzer")
        .unwrap();
    let comp_pos = types.iter().position(|&t| t == "compressor").unwrap();
    assert!(
        spec_pos < comp_pos,
        "spectrum_analyzer should come before compressor but got {:?}",
        types
    );
}

#[test]
fn test_channel_states_resized_on_update() {
    let mut g = PluginGraph::with_default_rack();
    g.add_user_plugin(&PluginType::Upmixer).unwrap();

    // After update, matrix should have 6 channel states (5.1)
    g.update_channel_dependent_plugins();

    let matrix = g.node_for_role(NodeRole::Matrix).unwrap();
    if let PluginSettings::Matrix {
        channel_states,
        input_channels,
        ..
    } = &matrix.plugin.settings
    {
        assert_eq!(*input_channels, 6);
        assert_eq!(
            channel_states.len(),
            6,
            "channel_states should be resized to match input_channels"
        );
    } else {
        panic!("Expected Matrix settings");
    }
}

#[test]
fn test_ascii_diagram_linear() {
    let g = PluginGraph::with_default_rack();
    let lines = g.to_ascii_diagram();
    assert!(!lines.is_empty());
    let joined = lines.join(" ");
    assert!(
        joined.contains("→"),
        "Linear diagram should contain arrows: {}",
        joined
    );
    assert!(joined.contains("Audio Input"), "Should show Input node");
    assert!(joined.contains("Audio Output"), "Should show Output node");
}

#[test]
fn test_ascii_diagram_non_linear() {
    let mut g = PluginGraph::new();
    let input = g.add_special_node(SpecialNodeType::Input, NodePosition::new(0.0, 0.0), 2);
    let a = g
        .add_plugin_node(&PluginType::EQ, NodePosition::new(100.0, 0.0))
        .unwrap();
    let b = g
        .add_plugin_node(&PluginType::Gain, NodePosition::new(100.0, 100.0))
        .unwrap();
    let output = g.add_special_node(SpecialNodeType::Output, NodePosition::new(200.0, 50.0), 2);
    g.add_connection(input, 0, a, 0).unwrap();
    g.add_connection(input, 1, b, 0).unwrap();
    g.add_connection(a, 0, output, 0).unwrap();
    g.add_connection(b, 0, output, 1).unwrap();

    let lines = g.to_ascii_diagram();
    assert!(!lines.is_empty());
    let joined = lines.join("\n");
    assert!(
        joined.contains("non-linear"),
        "Non-linear diagram should indicate non-linearity"
    );
}

#[test]
fn test_find_plugin_index_no_eq() {
    let g = PluginGraph::with_default_rack();
    assert!(g.find_plugin_index(&PluginType::EQ).is_none());
}

#[test]
fn test_insert_eq_and_configure_per_channel() {
    let mut g = PluginGraph::with_default_rack();
    let insert_idx = g.user_plugin_insert_index();
    let node_id = g.insert_plugin(insert_idx, &PluginType::EQ).unwrap();
    assert!(!node_id.is_nil());

    // Configure with per-channel settings
    let ch0_filters = vec![EQFilter::new(BiquadFilterType::Peak, 100.0, 1.5, -3.0)];
    let ch1_filters = vec![EQFilter::new(BiquadFilterType::Peak, 200.0, 2.0, -5.0)];
    let eq_plugin = g.get_plugin_mut(insert_idx).unwrap();
    eq_plugin.settings = PluginSettings::EQ {
        channels: 2,
        filters: ch0_filters.clone(),
        channel_filters: Some(vec![ch0_filters.clone(), ch1_filters.clone()]),
        stereo_pairs: None,
        per_channel_mode: true,
        max_filters: 10,
        tdf2: false,
        topology: 0.0,
        auto_gain_enabled: false,
        oversampling: 1.0,
    };

    // Verify the graph is still linear
    assert!(g.is_linear());

    // Verify settings are preserved
    let eq = g.get_plugin(insert_idx).unwrap();
    if let PluginSettings::EQ {
        channels,
        per_channel_mode,
        channel_filters,
        ..
    } = &eq.settings
    {
        assert_eq!(*channels, 2);
        assert!(*per_channel_mode);
        let cf = channel_filters.as_ref().unwrap();
        assert_eq!(cf.len(), 2);
        assert_eq!(cf[0][0].frequency, 100.0);
        assert_eq!(cf[1][0].frequency, 200.0);
    } else {
        panic!("Expected EQ settings");
    }

    // Verify EQ is before Matrix
    let eq_idx = g.find_plugin_index(&PluginType::EQ).unwrap();
    let matrix_idx = g.find_plugin_index(&PluginType::Matrix).unwrap();
    assert!(eq_idx < matrix_idx, "EQ should be before Matrix");
}

#[test]
fn test_update_existing_eq_preserves_position() {
    let mut g = PluginGraph::with_default_rack();
    // Add EQ then Compressor
    let _eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();
    let _comp_id = g.add_user_plugin(&PluginType::Compressor).unwrap();

    let eq_idx = g.find_plugin_index(&PluginType::EQ).unwrap();
    let comp_idx = g.find_plugin_index(&PluginType::Compressor).unwrap();
    assert!(eq_idx < comp_idx);

    // Update EQ with new per-channel settings
    let new_filters = vec![EQFilter::new(BiquadFilterType::Peak, 500.0, 3.0, -8.0)];
    let eq_plugin = g.get_plugin_mut(eq_idx).unwrap();
    eq_plugin.settings = PluginSettings::EQ {
        channels: 2,
        filters: new_filters.clone(),
        channel_filters: Some(vec![new_filters.clone(), new_filters.clone()]),
        stereo_pairs: None,
        per_channel_mode: true,
        max_filters: 10,
        tdf2: false,
        topology: 0.0,
        auto_gain_enabled: false,
        oversampling: 1.0,
    };

    // Verify position unchanged
    let eq_idx_after = g.find_plugin_index(&PluginType::EQ).unwrap();
    let comp_idx_after = g.find_plugin_index(&PluginType::Compressor).unwrap();
    assert_eq!(eq_idx, eq_idx_after, "EQ position should not change");
    assert_eq!(
        comp_idx, comp_idx_after,
        "Compressor position should not change"
    );

    // Verify new settings
    let eq = g.get_plugin(eq_idx).unwrap();
    if let PluginSettings::EQ {
        filters,
        per_channel_mode,
        ..
    } = &eq.settings
    {
        assert!(*per_channel_mode);
        assert_eq!(filters[0].frequency, 500.0);
    } else {
        panic!("Expected EQ settings");
    }
}

#[test]
fn test_to_plugin_configs_per_channel_eq() {
    let mut g = PluginGraph::with_default_rack();
    let insert_idx = g.user_plugin_insert_index();
    g.insert_plugin(insert_idx, &PluginType::EQ).unwrap();

    let ch0 = vec![EQFilter::new(BiquadFilterType::Peak, 100.0, 1.5, -3.0)];
    let ch1 = vec![EQFilter::new(BiquadFilterType::Peak, 200.0, 2.0, -5.0)];
    let eq = g.get_plugin_mut(insert_idx).unwrap();
    eq.settings = PluginSettings::EQ {
        channels: 2,
        filters: ch0.clone(),
        channel_filters: Some(vec![ch0, ch1]),
        stereo_pairs: None,
        per_channel_mode: true,
        max_filters: 10,
        tdf2: false,
        topology: 0.0,
        auto_gain_enabled: false,
        oversampling: 1.0,
    };

    let configs = g.to_plugin_configs(48000.0);
    let eq_config = configs
        .iter()
        .find(|c| c.plugin_type == "eq")
        .expect("EQ config should be present");
    let params = &eq_config.parameters;

    // per_channel_mode should produce "channel_filters" key
    assert!(
        params.get("channel_filters").is_some(),
        "Should have channel_filters key"
    );
    let ch_filters = params["channel_filters"].as_array().unwrap();
    assert_eq!(ch_filters.len(), 2);

    // Verify different frequencies per channel
    let ch0_freq = ch_filters[0][0]["freq"].as_f64().unwrap();
    let ch1_freq = ch_filters[1][0]["freq"].as_f64().unwrap();
    assert!((ch0_freq - 100.0).abs() < 0.1);
    assert!((ch1_freq - 200.0).abs() < 0.1);
}

#[test]
fn test_to_plugin_configs_global_eq() {
    let mut g = PluginGraph::with_default_rack();
    let insert_idx = g.user_plugin_insert_index();
    g.insert_plugin(insert_idx, &PluginType::EQ).unwrap();

    let filters = vec![EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.4, 3.0)];
    let eq = g.get_plugin_mut(insert_idx).unwrap();
    eq.settings = PluginSettings::EQ {
        channels: 2,
        filters,
        channel_filters: None,
        stereo_pairs: None,
        per_channel_mode: false,
        max_filters: 10,
        tdf2: false,
        topology: 0.0,
        auto_gain_enabled: false,
        oversampling: 1.0,
    };

    let configs = g.to_plugin_configs(48000.0);
    let eq_config = configs
        .iter()
        .find(|c| c.plugin_type == "eq")
        .expect("EQ config");
    let params = &eq_config.parameters;

    // Global mode should produce "filters" key, NOT "channel_filters"
    assert!(params.get("filters").is_some(), "Should have filters key");
    assert!(
        params.get("channel_filters").is_none(),
        "Should NOT have channel_filters key"
    );
}

#[test]
fn test_disabled_eq_excluded_from_configs() {
    let mut g = PluginGraph::with_default_rack();
    let eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();
    g.toggle_plugin(eq_id).unwrap(); // disable it

    let configs = g.to_plugin_configs(48000.0);
    assert!(
        configs.iter().all(|c| c.plugin_type != "eq"),
        "Disabled EQ should not appear in configs"
    );
}

// =========================================================================
// add_connection edge cases
// =========================================================================

#[test]
fn test_add_connection_missing_source_node() {
    let mut graph = PluginGraph::new();
    let node1 = graph
        .add_plugin_node(&PluginType::EQ, NodePosition::new(100.0, 100.0))
        .unwrap();
    let node2 = graph
        .add_plugin_node(&PluginType::Gain, NodePosition::new(300.0, 100.0))
        .unwrap();
    graph.remove_node(node1);
    let result = graph.add_connection(node1, 0, node2, 0);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Source node"));
}

#[test]
fn test_add_connection_missing_target_node() {
    let mut graph = PluginGraph::new();
    let node1 = graph
        .add_plugin_node(&PluginType::EQ, NodePosition::new(100.0, 100.0))
        .unwrap();
    let node2 = graph
        .add_plugin_node(&PluginType::Gain, NodePosition::new(300.0, 100.0))
        .unwrap();
    graph.remove_node(node2);
    let result = graph.add_connection(node1, 0, node2, 0);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Target node"));
}

#[test]
fn test_add_connection_duplicate() {
    let mut graph = PluginGraph::new();
    let node1 = graph
        .add_plugin_node(&PluginType::EQ, NodePosition::new(100.0, 100.0))
        .unwrap();
    let node2 = graph
        .add_plugin_node(&PluginType::Gain, NodePosition::new(300.0, 100.0))
        .unwrap();
    graph.add_connection(node1, 0, node2, 0).unwrap();
    let result = graph.add_connection(node1, 0, node2, 0);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Connection already exists");
}

#[test]
fn test_add_connection_self_loop() {
    let mut graph = PluginGraph::new();
    let node1 = graph
        .add_plugin_node(&PluginType::EQ, NodePosition::new(100.0, 100.0))
        .unwrap();
    let result = graph.add_connection(node1, 0, node1, 0);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Connection would create a cycle");
}

// =========================================================================
// update_channel_dependent_plugins
// =========================================================================

#[test]
fn test_update_channel_dependent_plugins_band_split() {
    let mut g = PluginGraph::with_default_rack();
    let split_id = g.add_user_plugin(&PluginType::BandSplit).unwrap();

    g.update_channel_dependent_plugins();

    let split_node = g.nodes.get(&split_id).unwrap();
    assert_eq!(split_node.input_channels, 2);
    assert_eq!(
        split_node.output_channels, 4,
        "BandSplit should double channels"
    );

    let matrix = g.node_for_role(NodeRole::Matrix).unwrap();
    assert_eq!(
        matrix.input_channels, 4,
        "Matrix should adopt BandSplit output"
    );
}

#[test]
fn test_update_channel_dependent_plugins_band_merge() {
    let mut g = PluginGraph::with_default_rack();
    let _split_id = g.add_user_plugin(&PluginType::BandSplit).unwrap();
    let merge_id = g.add_user_plugin(&PluginType::BandMerge).unwrap();

    g.update_channel_dependent_plugins();

    let merge_node = g.nodes.get(&merge_id).unwrap();
    assert_eq!(
        merge_node.input_channels, 4,
        "BandMerge input should follow BandSplit output"
    );
    assert_eq!(
        merge_node.output_channels, 2,
        "BandMerge should halve channels (bands=2)"
    );

    let matrix = g.node_for_role(NodeRole::Matrix).unwrap();
    assert_eq!(
        matrix.input_channels, 2,
        "Matrix should follow BandMerge output"
    );
}

#[test]
fn test_update_channel_dependent_plugins_downmix() {
    let mut g = PluginGraph::with_default_rack();
    let _upmixer_id = g.add_user_plugin(&PluginType::Upmixer).unwrap();
    let downmix_id = g.add_user_plugin(&PluginType::Downmix).unwrap();

    g.update_channel_dependent_plugins();

    let downmix_node = g.nodes.get(&downmix_id).unwrap();
    // Upmixer default is 5.1 = 6 channels
    assert_eq!(
        downmix_node.input_channels, 6,
        "Downmix should adopt upmixer output"
    );
    assert_eq!(
        downmix_node.output_channels, 2,
        "Downmix should output 2 channels"
    );
}

#[test]
fn test_update_channel_dependent_plugins_suspended_skipped() {
    let mut g = PluginGraph::with_default_rack();
    let upmixer_id = g.add_user_plugin(&PluginType::Upmixer).unwrap();

    // Find linear index of upmixer and suspend it
    let upmixer_idx = g.linear_index_of_node(upmixer_id).unwrap();
    g.suspend_plugins(&[upmixer_idx]);

    g.update_channel_dependent_plugins();

    let matrix = g.node_for_role(NodeRole::Matrix).unwrap();
    assert_eq!(
        matrix.input_channels, 2,
        "Suspended upmixer should not expand channels"
    );
}

#[test]
fn test_update_channel_dependent_plugins_non_linear_returns_early() {
    let mut g = PluginGraph::new();
    let input = g.add_special_node(SpecialNodeType::Input, NodePosition::new(0.0, 0.0), 2);
    let a = g
        .add_plugin_node(&PluginType::EQ, NodePosition::new(100.0, 0.0))
        .unwrap();
    let b = g
        .add_plugin_node(&PluginType::Gain, NodePosition::new(100.0, 100.0))
        .unwrap();
    let output = g.add_special_node(SpecialNodeType::Output, NodePosition::new(200.0, 50.0), 2);

    g.add_connection(input, 0, a, 0).unwrap();
    g.add_connection(input, 1, b, 0).unwrap();
    g.add_connection(a, 0, output, 0).unwrap();
    g.add_connection(b, 0, output, 1).unwrap();

    // Should not panic on non-linear graph
    g.update_channel_dependent_plugins();

    // Nodes keep their default channel counts since linear_order returns None
    let a_node = g.nodes.get(&a).unwrap();
    assert_eq!(a_node.input_channels, 2);
}

// =========================================================================
// get_engine_index edge cases
// =========================================================================

#[test]
fn test_get_engine_index_suspended_returns_none() {
    let mut g = PluginGraph::with_default_rack();
    let eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();

    let eq_idx = g.linear_index_of_node(eq_id).unwrap();
    g.suspend_plugins(&[eq_idx]);

    assert_eq!(g.get_engine_index(eq_id), None);
}

#[test]
fn test_get_engine_index_unknown_node() {
    let g = PluginGraph::with_default_rack();
    let unknown_id = super::types::GraphNodeId::new_v4();
    assert_eq!(g.get_engine_index(unknown_id), None);
}

#[test]
fn test_get_engine_index_by_linear_position_out_of_bounds() {
    let g = PluginGraph::with_default_rack();
    assert_eq!(g.get_engine_index_by_linear_position(100), None);
}

// =========================================================================
// Channel count helpers
// =========================================================================

#[test]
fn test_node_input_output_channels_for_special_nodes() {
    let mut g = PluginGraph::new();
    let input = g.add_special_node(SpecialNodeType::Input, NodePosition::new(0.0, 0.0), 4);
    let output = g.add_special_node(SpecialNodeType::Output, NodePosition::new(0.0, 0.0), 6);
    let split = g.add_special_node(SpecialNodeType::Split, NodePosition::new(0.0, 0.0), 2);
    let merge = g.add_special_node(SpecialNodeType::Merge, NodePosition::new(0.0, 0.0), 2);

    assert_eq!(g.node_input_channels(input), 0);
    assert_eq!(g.node_output_channels(input), 4);

    assert_eq!(g.node_input_channels(output), 6);
    assert_eq!(g.node_output_channels(output), 0);

    assert_eq!(g.node_input_channels(split), 1);
    assert_eq!(g.node_output_channels(split), 2);

    assert_eq!(g.node_input_channels(merge), 2);
    assert_eq!(g.node_output_channels(merge), 1);
}

#[test]
fn test_node_input_output_channels_for_plugins() {
    let mut g = PluginGraph::new();
    let eq = g
        .add_plugin_node(&PluginType::EQ, NodePosition::new(0.0, 0.0))
        .unwrap();
    let upmixer = g
        .add_plugin_node(&PluginType::Upmixer, NodePosition::new(0.0, 0.0))
        .unwrap();

    assert_eq!(g.node_input_channels(eq), 2);
    assert_eq!(g.node_output_channels(eq), 2);

    assert_eq!(g.node_input_channels(upmixer), 2);
    assert!(g.node_output_channels(upmixer) >= 5);
}

#[test]
fn test_node_input_output_channels_for_missing_node() {
    let g = PluginGraph::new();
    let unknown_id = super::types::GraphNodeId::new_v4();
    assert_eq!(g.node_input_channels(unknown_id), 0);
    assert_eq!(g.node_output_channels(unknown_id), 0);
}

// =========================================================================
// ASCII diagram
// =========================================================================

#[test]
fn test_ascii_diagram_empty_graph() {
    let g = PluginGraph::new();
    let lines = g.to_ascii_diagram();
    assert_eq!(lines, vec!["(empty graph)"]);
}

#[test]
fn test_ascii_diagram_shows_disabled_plugins() {
    let mut g = PluginGraph::with_default_rack();
    let eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();
    g.toggle_plugin(eq_id).unwrap();

    let joined = g.to_ascii_diagram().join(" ");
    assert!(
        joined.contains("[off]"),
        "disabled plugin should be marked [off]: {joined}"
    );
}

#[test]
fn test_update_channel_dependent_plugins_eq_channels_propagate() {
    let mut g = PluginGraph::with_default_rack();
    let eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();

    // Change input to mono
    if let Some(input) = g.input_node_mut() {
        input.channels = 1;
    }

    g.update_channel_dependent_plugins();

    let eq_node = g.nodes.get(&eq_id).unwrap();
    match &eq_node.plugin.settings {
        PluginSettings::EQ { channels, .. } => {
            assert_eq!(*channels, 1, "EQ should adopt mono input channels");
        }
        _ => panic!("Expected EQ settings"),
    }
    assert_eq!(eq_node.input_channels, 1);
    assert_eq!(eq_node.output_channels, 1);
}

#[test]
fn test_eq_channel_remap_preserves_valid_explicit_stereo_pairs() {
    let mut graph = PluginGraph::with_default_rack();
    let eq_id = graph.add_user_plugin(&PluginType::EQ).unwrap();
    if let Some(input) = graph.input_node_mut() {
        input.channels = 4;
    }
    graph.update_channel_dependent_plugins();
    if let PluginSettings::EQ { stereo_pairs, .. } =
        &mut graph.nodes.get_mut(&eq_id).unwrap().plugin.settings
    {
        *stereo_pairs = Some(vec![[1, 0], [3, 2]]);
    } else {
        panic!("Expected EQ settings");
    }

    if let Some(input) = graph.input_node_mut() {
        input.channels = 2;
    }
    graph.update_channel_dependent_plugins();
    match &graph.nodes[&eq_id].plugin.settings {
        PluginSettings::EQ {
            channels,
            stereo_pairs,
            ..
        } => {
            assert_eq!(*channels, 2);
            assert_eq!(stereo_pairs.as_deref(), Some([[1, 0]].as_slice()));
        }
        _ => panic!("Expected EQ settings"),
    }

    if let PluginSettings::EQ { stereo_pairs, .. } =
        &mut graph.nodes.get_mut(&eq_id).unwrap().plugin.settings
    {
        *stereo_pairs = Some(Vec::new());
    }
    if let Some(input) = graph.input_node_mut() {
        input.channels = 4;
    }
    graph.update_channel_dependent_plugins();
    match &graph.nodes[&eq_id].plugin.settings {
        PluginSettings::EQ { stereo_pairs, .. } => assert_eq!(stereo_pairs, &Some(Vec::new())),
        _ => panic!("Expected EQ settings"),
    }
}

#[test]
fn channel_mute_solo_tracks_topology_and_preserves_retained_channels() {
    for channels in [1, 2, 6, 12] {
        let mut graph = PluginGraph::with_default_rack();
        graph.input_node_mut().unwrap().channels = channels;
        let id = graph.add_user_plugin(&PluginType::ChannelMuteSolo).unwrap();
        let linear_index = graph
            .find_plugin_index(&PluginType::ChannelMuteSolo)
            .unwrap();
        assert_eq!(graph.plugin_input_channels(linear_index), Some(channels));
        assert_eq!(graph.nodes[&id].input_channels, channels);
        let PluginSettings::ChannelMuteSolo { channel_states, .. } =
            &mut graph.nodes.get_mut(&id).unwrap().plugin.settings
        else {
            panic!("Expected ChannelMuteSolo settings");
        };
        assert_eq!(channel_states.len(), channels);
        channel_states[0].muted = true;

        graph.input_node_mut().unwrap().channels = channels + 1;
        graph.update_channel_dependent_plugins();
        let PluginSettings::ChannelMuteSolo { channel_states, .. } =
            &graph.nodes[&id].plugin.settings
        else {
            panic!("Expected ChannelMuteSolo settings");
        };
        assert_eq!(channel_states.len(), channels + 1);
        assert!(channel_states[0].muted);
        assert!(!channel_states[channels].muted);

        let saved = serde_json::to_string(&graph).unwrap();
        let mut reloaded: PluginGraph = serde_json::from_str(&saved).unwrap();
        reloaded.update_channel_dependent_plugins();
        let PluginSettings::ChannelMuteSolo { channel_states, .. } =
            &reloaded.nodes[&id].plugin.settings
        else {
            panic!("Expected ChannelMuteSolo settings after reload");
        };
        assert_eq!(channel_states.len(), channels + 1);
        assert!(channel_states[0].muted);

        reloaded.input_node_mut().unwrap().channels = channels;
        reloaded.update_channel_dependent_plugins();
        let PluginSettings::ChannelMuteSolo { channel_states, .. } =
            &reloaded.nodes[&id].plugin.settings
        else {
            panic!("Expected ChannelMuteSolo settings after shrinking");
        };
        assert_eq!(channel_states.len(), channels);
        assert!(channel_states[0].muted);
    }
}

#[test]
fn matrix_repairs_channel_states_without_replacing_same_topology_routing() {
    let mut graph = PluginGraph::with_default_rack();
    let matrix_id = graph.node_id_for_role(NodeRole::Matrix).unwrap();
    let routing = vec![0.75, 0.25, 0.1, 0.9];
    let PluginSettings::Matrix {
        input_channels,
        output_channels,
        matrix,
        channel_states,
    } = &mut graph.nodes.get_mut(&matrix_id).unwrap().plugin.settings
    else {
        panic!("Expected Matrix settings");
    };
    *input_channels = 2;
    *output_channels = 2;
    *matrix = routing.clone();
    channel_states.clear();

    graph.update_channel_dependent_plugins();
    let PluginSettings::Matrix {
        matrix,
        channel_states,
        ..
    } = &mut graph.nodes.get_mut(&matrix_id).unwrap().plugin.settings
    else {
        panic!("Expected Matrix settings");
    };
    assert_eq!(*matrix, routing);
    assert_eq!(channel_states.len(), 2);
    channel_states[0].muted = true;
    channel_states.pop();

    graph.update_channel_dependent_plugins();
    let PluginSettings::Matrix {
        matrix,
        channel_states,
        ..
    } = &graph.nodes[&matrix_id].plugin.settings
    else {
        panic!("Expected Matrix settings");
    };
    assert_eq!(*matrix, routing);
    assert_eq!(channel_states.len(), 2);
    assert!(channel_states[0].muted);
    assert!(!channel_states[1].muted);

    let PluginSettings::Matrix {
        output_channels,
        matrix,
        channel_states,
        ..
    } = &mut graph.nodes.get_mut(&matrix_id).unwrap().plugin.settings
    else {
        panic!("Expected Matrix settings");
    };
    *output_channels = 4;
    let rectangular_routing = vec![0.75, 0.25, 0.1, 0.9, 0.2, 0.8, 0.6, 0.4];
    *matrix = rectangular_routing.clone();
    channel_states[0].soloed = true;
    graph.update_channel_dependent_plugins();
    let PluginSettings::Matrix {
        output_channels,
        matrix,
        channel_states,
        ..
    } = &graph.nodes[&matrix_id].plugin.settings
    else {
        panic!("Expected Matrix settings");
    };
    assert_eq!(*output_channels, 4);
    assert_eq!(*matrix, rectangular_routing);
    assert_eq!(channel_states.len(), 4);
    assert!(channel_states[0].muted && channel_states[0].soloed);
}

#[test]
fn channel_mute_solo_renders_configured_channels_in_non_linear_graph() {
    let mut graph = PluginGraph::new();
    graph.add_special_node(SpecialNodeType::Input, NodePosition::new(0.0, 0.0), 6);
    let mute_id = graph
        .add_plugin_node(&PluginType::ChannelMuteSolo, NodePosition::new(100.0, 0.0))
        .unwrap();
    graph
        .add_plugin_node(&PluginType::Gain, NodePosition::new(100.0, 100.0))
        .unwrap();
    graph.nodes.get_mut(&mute_id).unwrap().input_channels = 6;
    graph.update_channel_dependent_plugins();
    let PluginSettings::ChannelMuteSolo { channel_states, .. } =
        &graph.nodes[&mute_id].plugin.settings
    else {
        panic!("Expected ChannelMuteSolo settings");
    };
    assert_eq!(channel_states.len(), 6);
}

#[test]
fn test_update_channel_dependent_plugins_gain_channels_propagate() {
    let mut g = PluginGraph::with_default_rack();
    let gain_id = g.add_user_plugin(&PluginType::Gain).unwrap();

    // Change input to mono
    if let Some(input) = g.input_node_mut() {
        input.channels = 1;
    }

    g.update_channel_dependent_plugins();

    let gain_node = g.nodes.get(&gain_id).unwrap();
    match &gain_node.plugin.settings {
        PluginSettings::Gain { channels, .. } => {
            assert_eq!(*channels, 1, "Gain should adopt mono input channels");
        }
        _ => panic!("Expected Gain settings"),
    }
}

#[test]
fn test_update_channel_dependent_plugins_binaural_preserves_acoustic_settings() {
    let mut g = PluginGraph::with_default_rack();
    let binaural_id = g.add_user_plugin(&PluginType::BinauralDecoder).unwrap();

    if let Some(node) = g.nodes.get_mut(&binaural_id) {
        if let PluginSettings::BinauralDecoder {
            crossfade_ms,
            head_yaw_deg,
            head_pitch_deg,
            head_roll_deg,
            hrtf_database_dir,
            head_width_cm,
            ear_height_cm,
            ..
        } = &mut node.plugin.settings
        {
            *crossfade_ms = 17.0;
            *head_yaw_deg = 11.0;
            *head_pitch_deg = -7.0;
            *head_roll_deg = 3.0;
            *hrtf_database_dir = "fixture/hrtf".into();
            *head_width_cm = 15.5;
            *ear_height_cm = 1.25;
        }
    }

    if let Some(input) = g.input_node_mut() {
        input.channels = 1;
    }
    g.update_channel_dependent_plugins();

    match &g.nodes[&binaural_id].plugin.settings {
        PluginSettings::BinauralDecoder {
            input_channels,
            crossfade_ms,
            head_yaw_deg,
            head_pitch_deg,
            head_roll_deg,
            hrtf_database_dir,
            head_width_cm,
            ear_height_cm,
            ..
        } => {
            assert_eq!(*input_channels, 1);
            assert_eq!(*crossfade_ms, 17.0);
            assert_eq!(*head_yaw_deg, 11.0);
            assert_eq!(*head_pitch_deg, -7.0);
            assert_eq!(*head_roll_deg, 3.0);
            assert_eq!(hrtf_database_dir, "fixture/hrtf");
            assert_eq!(*head_width_cm, 15.5);
            assert_eq!(*ear_height_cm, 1.25);
        }
        _ => panic!("expected Binaural Decoder settings"),
    }
}

#[test]
fn test_update_channel_dependent_plugins_per_channel_mode_resets_when_mismatch() {
    let mut g = PluginGraph::with_default_rack();
    let eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();

    // Seed EQ with per-channel filters for 2 channels, then switch input to mono
    if let Some(node) = g.nodes.get_mut(&eq_id) {
        if let PluginSettings::EQ {
            channel_filters,
            per_channel_mode,
            ..
        } = &mut node.plugin.settings
        {
            *channel_filters = Some(vec![Vec::new(), Vec::new()]);
            *per_channel_mode = true;
        }
    }

    if let Some(input) = g.input_node_mut() {
        input.channels = 1;
    }
    g.update_channel_dependent_plugins();

    let eq_node = g.nodes.get(&eq_id).unwrap();
    match &eq_node.plugin.settings {
        PluginSettings::EQ {
            channels,
            per_channel_mode,
            channel_filters,
            ..
        } => {
            assert_eq!(*channels, 1);
            assert!(!per_channel_mode);
            assert!(channel_filters.is_none());
        }
        _ => panic!("Expected EQ settings"),
    }
}

#[test]
fn test_move_user_plugin_preserves_channel_counts() {
    let mut g = PluginGraph::with_default_rack();
    let eq_id = g.add_user_plugin(&PluginType::EQ).unwrap();
    let comp_id = g.add_user_plugin(&PluginType::Compressor).unwrap();

    g.update_channel_dependent_plugins();
    let eq_in_before = g.nodes.get(&eq_id).unwrap().input_channels;
    let comp_in_before = g.nodes.get(&comp_id).unwrap().input_channels;

    g.move_user_plugin_down(eq_id).unwrap();

    assert_eq!(
        g.nodes.get(&eq_id).unwrap().input_channels,
        eq_in_before,
        "EQ input channels should be preserved after move"
    );
    assert_eq!(
        g.nodes.get(&comp_id).unwrap().input_channels,
        comp_in_before,
        "Compressor input channels should be preserved after move"
    );
}

#[test]
fn preset_reload_preserves_live_permanent_controls() {
    let directory = tempfile::tempdir().unwrap();
    let mut preset = PluginGraph::with_default_rack();
    let user = preset.add_user_plugin(&PluginType::Gain).unwrap();
    if let PluginSettings::Gain { gain_db, .. } =
        &mut preset.nodes.get_mut(&user).unwrap().plugin.settings
    {
        *gain_db = -7.5;
    }
    preset.save_to_file(directory.path(), "processing").unwrap();

    let mut current = PluginGraph::with_default_rack();
    current.update_channel_dependent_plugins();
    for role in [NodeRole::InputMonitor, NodeRole::OutputMonitor] {
        let id = current.node_id_for_role(role).unwrap();
        current.nodes.get_mut(&id).unwrap().plugin.enabled = false;
    }
    let replay = current.node_id_for_role(NodeRole::ReplayGain).unwrap();
    let replay = &mut current.nodes.get_mut(&replay).unwrap().plugin;
    replay.enabled = true;
    if let PluginSettings::Gain { gain_db, .. } = &mut replay.settings {
        *gain_db = -3.0;
    }
    let matrix = current.node_id_for_role(NodeRole::Matrix).unwrap();
    if let PluginSettings::Matrix {
        matrix,
        channel_states,
        ..
    } = &mut current.nodes.get_mut(&matrix).unwrap().plugin.settings
    {
        *matrix = vec![0.75, 0.25, -0.25, 0.75];
        channel_states[0].muted = true;
        channel_states[1].soloed = true;
        channel_states[1].dimmed = true;
    }
    let expected: Vec<_> = [
        NodeRole::InputMonitor,
        NodeRole::ReplayGain,
        NodeRole::Matrix,
        NodeRole::OutputMonitor,
    ]
    .into_iter()
    .map(|role| {
        let id = current.node_id_for_role(role).unwrap();
        (
            role,
            serde_json::to_value(&current.nodes[&id].plugin).unwrap(),
        )
    })
    .collect();

    assert!(
        current
            .load_from_file(directory.path(), "processing")
            .unwrap()
            .is_empty()
    );
    for (role, plugin) in expected {
        let id = current.node_id_for_role(role).unwrap();
        assert_eq!(
            serde_json::to_value(&current.nodes[&id].plugin).unwrap(),
            plugin
        );
    }
    let processing = current
        .plugins_linear()
        .unwrap()
        .into_iter()
        .find(|node| node.role == NodeRole::User)
        .unwrap();
    assert!(matches!(
        processing.plugin.settings,
        PluginSettings::Gain { gain_db: -7.5, .. }
    ));
}

#[test]
fn preset_reload_adapts_output_count_without_losing_retained_flags() {
    let directory = tempfile::tempdir().unwrap();
    let mut preset = PluginGraph::with_default_rack();
    let user = preset.add_user_plugin(&PluginType::Upmixer).unwrap();
    let output_channels = preset.node_output_channels(user);
    assert!(output_channels > 2);
    preset.save_to_file(directory.path(), "surround").unwrap();

    let mut current = PluginGraph::with_default_rack();
    current.update_channel_dependent_plugins();
    let matrix = current.node_id_for_role(NodeRole::Matrix).unwrap();
    if let PluginSettings::Matrix { channel_states, .. } =
        &mut current.nodes.get_mut(&matrix).unwrap().plugin.settings
    {
        channel_states[0].muted = true;
        channel_states[1].soloed = true;
        channel_states[1].dimmed = true;
    }
    current
        .load_from_file(directory.path(), "surround")
        .unwrap();
    let matrix = current.node_id_for_role(NodeRole::Matrix).unwrap();
    let PluginSettings::Matrix {
        input_channels,
        output_channels: actual_output,
        channel_states,
        ..
    } = &current.nodes[&matrix].plugin.settings
    else {
        panic!("permanent Matrix missing")
    };
    assert_eq!(*input_channels, output_channels);
    assert_eq!(*actual_output, output_channels);
    assert!(channel_states[0].muted);
    assert!(channel_states[1].soloed && channel_states[1].dimmed);
    assert_eq!(channel_states.len(), output_channels);
    assert!(
        channel_states[2..]
            .iter()
            .all(|state| !state.muted && !state.soloed && !state.dimmed)
    );
}

#[test]
fn crossover_per_channel_adapts_to_actual_graph_width() {
    use sotf_plugins::plugin_crossover::CrossoverTopology;
    let mut graph = PluginGraph::with_default_rack();
    graph.add_user_plugin(&PluginType::Upmixer).unwrap();
    let id = graph.add_user_plugin(&PluginType::Crossover).unwrap();
    let settings = &mut graph.nodes.get_mut(&id).unwrap().plugin.settings;
    let PluginSettings::Crossover {
        topology,
        frequency,
        ..
    } = settings
    else {
        panic!("crossover fixture");
    };
    *topology = Some(CrossoverTopology::PerChannel);
    *frequency = 700.0;
    graph.update_channel_dependent_plugins();
    let node = &graph.nodes[&id];
    assert_eq!(node.input_channels, 6);
    let PluginSettings::Crossover {
        channel_frequencies_hz,
        channel_modes,
        ..
    } = &node.plugin.settings
    else {
        panic!("crossover fixture");
    };
    assert_eq!(
        channel_frequencies_hz.as_deref(),
        Some([700.0; 6].as_slice())
    );
    assert_eq!(channel_modes.as_ref().unwrap().len(), 6);

    let PluginSettings::Crossover {
        crossover_type,
        frequency,
        output,
        topology,
        band_count,
        extra_frequencies,
        fir_taps,
        channel_frequencies_hz,
        channel_modes,
    } = &node.plugin.settings
    else {
        panic!("crossover fixture");
    };
    let params = sotf_plugins::plugin_crossover::CrossoverPluginParams {
        crossover_type: crossover_type.clone(),
        frequency: *frequency,
        output: output.clone(),
        topology: *topology,
        band_count: *band_count,
        extra_frequencies: extra_frequencies.clone(),
        fir_taps: Some(*fir_taps),
        channel_frequencies_hz: channel_frequencies_hz
            .as_ref()
            .unwrap()
            .iter()
            .map(|v| *v as f32)
            .collect(),
        channel_modes: channel_modes.clone(),
    };
    assert!(sotf_plugins::plugin_crossover::CrossoverPlugin::from_params(6, &params).is_ok());

    let settings = &mut graph.nodes.get_mut(&id).unwrap().plugin.settings;
    let PluginSettings::Crossover {
        channel_frequencies_hz,
        channel_modes,
        ..
    } = settings
    else {
        panic!("crossover fixture");
    };
    channel_frequencies_hz.as_mut().unwrap()[1] = 1200.0;
    channel_modes.as_mut().unwrap()[1] = "mute".into();
    let expected = settings.clone();
    graph.update_channel_dependent_plugins();
    assert_eq!(
        serde_json::to_value(&graph.nodes[&id].plugin.settings).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
}

#[test]
fn crossover_channel_adaptation_preserves_legacy_and_dormant_settings() {
    use sotf_plugins::plugin_crossover::CrossoverTopology;
    let mut graph = PluginGraph::with_default_rack();
    let id = graph.add_user_plugin(&PluginType::Crossover).unwrap();
    let settings = &mut graph.nodes.get_mut(&id).unwrap().plugin.settings;
    let PluginSettings::Crossover {
        topology,
        output,
        channel_frequencies_hz,
        channel_modes,
        ..
    } = settings
    else {
        panic!("crossover fixture");
    };
    *topology = None;
    *output = "highpass".into();
    *channel_frequencies_hz = Some(vec![400.0, 900.0]);
    *channel_modes = Some(vec!["mute".into()]);
    graph.update_channel_dependent_plugins();
    let PluginSettings::Crossover { channel_modes, .. } = &graph.nodes[&id].plugin.settings else {
        panic!("crossover fixture");
    };
    assert_eq!(channel_modes.as_ref().unwrap(), &["mute", "highpass"]);
    let settings = &mut graph.nodes.get_mut(&id).unwrap().plugin.settings;
    if let PluginSettings::Crossover { topology, .. } = settings {
        *topology = Some(CrossoverTopology::Bands);
    }
    let expected = settings.clone();
    super::misc::adapt_crossover_channels(settings, 16);
    assert_eq!(
        serde_json::to_value(&*settings).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
}

#[test]
fn crossover_channel_adaptation_resizes_and_retains_channel_values() {
    use sotf_plugins::plugin_crossover::CrossoverTopology;
    let mut settings = PluginSettings::Crossover {
        crossover_type: "LR24".into(),
        frequency: 1000.0,
        output: "both".into(),
        fir_taps: 1025,
        topology: Some(CrossoverTopology::PerChannel),
        band_count: None,
        extra_frequencies: vec![],
        channel_frequencies_hz: None,
        channel_modes: None,
    };
    super::misc::adapt_crossover_channels(&mut settings, 2);
    let PluginSettings::Crossover {
        channel_frequencies_hz,
        channel_modes,
        ..
    } = &mut settings
    else {
        panic!("crossover fixture");
    };
    assert_eq!(
        channel_modes.as_ref().unwrap(),
        &["passthrough", "passthrough"]
    );
    channel_frequencies_hz.as_mut().unwrap()[0] = 200.0;
    channel_modes.as_mut().unwrap()[0] = "lowpass".into();
    super::misc::adapt_crossover_channels(&mut settings, 4);
    let PluginSettings::Crossover {
        channel_frequencies_hz,
        channel_modes,
        ..
    } = &settings
    else {
        panic!("crossover fixture");
    };
    assert_eq!(
        channel_frequencies_hz.as_ref().unwrap(),
        &[200.0, 1000.0, 1000.0, 1000.0]
    );
    assert_eq!(
        channel_modes.as_ref().unwrap(),
        &["lowpass", "passthrough", "passthrough", "passthrough"]
    );
    super::misc::adapt_crossover_channels(&mut settings, 1);
    let PluginSettings::Crossover {
        channel_frequencies_hz,
        channel_modes,
        ..
    } = &settings
    else {
        panic!("crossover fixture");
    };
    assert_eq!(channel_frequencies_hz.as_ref().unwrap(), &[200.0]);
    assert_eq!(channel_modes.as_ref().unwrap(), &["lowpass"]);
}

#[test]
fn crossover_channel_adaptation_does_not_repair_invalid_explicit_modes() {
    use sotf_plugins::plugin_crossover::CrossoverTopology;
    let mut settings = PluginSettings::Crossover {
        crossover_type: "LR24".into(),
        frequency: 1000.0,
        output: "lowpass".into(),
        fir_taps: 1025,
        topology: Some(CrossoverTopology::PerChannel),
        band_count: None,
        extra_frequencies: vec![],
        channel_frequencies_hz: Some(vec![200.0, 400.0]),
        channel_modes: Some(vec!["mute".into()]),
    };
    let before = serde_json::to_value(&settings).unwrap();
    super::misc::adapt_crossover_channels(&mut settings, 4);
    assert_eq!(serde_json::to_value(&settings).unwrap(), before);
}

#[test]
fn restored_preset_cards_keep_signal_order_and_separate_hit_targets() {
    let directory = tempfile::tempdir().unwrap();
    let mut preset = PluginGraph::with_default_rack();
    for plugin in [
        PluginType::EQ,
        PluginType::DeEsser,
        PluginType::LoudnessCompensation,
    ] {
        preset.add_user_plugin(&plugin).unwrap();
    }
    preset
        .save_to_file(directory.path(), "separate-cards")
        .unwrap();
    let mut restored = PluginGraph::with_default_rack();
    let warnings = restored
        .load_from_file(directory.path(), "separate-cards")
        .unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    let order = restored.linear_order().unwrap();
    let positions: Vec<_> = order
        .iter()
        .map(|id| restored.node_position(*id).unwrap())
        .collect();
    assert_eq!(positions.len(), 9);
    for adjacent in positions.windows(2) {
        assert_eq!(adjacent[1].y, adjacent[0].y);
        assert_eq!(adjacent[1].x - adjacent[0].x, 180.0);
    }
    let users: Vec<_> = restored
        .plugins_linear()
        .unwrap()
        .into_iter()
        .filter(|node| !node.plugin.permanent)
        .map(|node| node.plugin.plugin_type())
        .collect();
    assert_eq!(
        users,
        vec![
            PluginType::EQ,
            PluginType::DeEsser,
            PluginType::LoudnessCompensation
        ]
    );
    // Repeated restoration must regenerate the same geometry, without drift.
    restored
        .load_from_file(directory.path(), "separate-cards")
        .unwrap();
    let repeated: Vec<_> = restored
        .linear_order()
        .unwrap()
        .into_iter()
        .map(|id| restored.node_position(id).unwrap().x)
        .collect();
    assert_eq!(
        repeated,
        positions
            .iter()
            .map(|position| position.x)
            .collect::<Vec<_>>()
    );
}

#[test]
fn added_user_cards_keep_full_slots_and_preserve_upstream_positions() {
    let mut graph = PluginGraph::with_default_rack();
    let upstream: Vec<_> = graph
        .linear_order()
        .unwrap()
        .into_iter()
        .take(3)
        .map(|id| (id, graph.node_position(id).unwrap()))
        .collect();
    let mut added = Vec::new();
    for (index, plugin_type) in [PluginType::EQ, PluginType::DeEsser, PluginType::Gain]
        .into_iter()
        .enumerate()
    {
        let id = match index {
            0 => graph.add_user_plugin(&plugin_type).unwrap(),
            1 => graph.insert_plugin(2 + index, &plugin_type).unwrap(),
            _ => {
                let settings = graph
                    .node_for_role(NodeRole::ReplayGain)
                    .unwrap()
                    .plugin
                    .settings
                    .clone();
                graph.insert_plugin_settings(2 + index, settings).unwrap()
            }
        };
        added.push(id);
        let positions: Vec<_> = graph
            .linear_order()
            .unwrap()
            .into_iter()
            .map(|id| graph.node_position(id).unwrap())
            .collect();
        for pair in positions.windows(2) {
            assert_eq!(pair[1].x - pair[0].x, 180.0);
            assert_eq!(pair[1].y, pair[0].y);
        }
        for (id, expected) in &upstream {
            let actual = graph.node_position(*id).unwrap();
            assert_eq!(actual.x, expected.x);
            assert_eq!(actual.y, expected.y);
        }
    }
    // Removing a middle card can leave a spare slot; the next addition must
    // still fit without moving or overlapping the surviving user cards.
    graph.remove_user_plugin(added[1]).unwrap();
    let survivors: Vec<_> = [added[0], added[2]]
        .into_iter()
        .map(|id| (id, graph.node_position(id).unwrap().x))
        .collect();
    graph.add_user_plugin(&PluginType::Compressor).unwrap();
    for (id, expected_x) in survivors {
        assert_eq!(graph.node_position(id).unwrap().x, expected_x);
    }
    let positions: Vec<_> = graph
        .linear_order()
        .unwrap()
        .into_iter()
        .map(|id| graph.node_position(id).unwrap().x)
        .collect();
    assert!(positions.windows(2).all(|pair| pair[1] - pair[0] >= 180.0));
}

#[test]
fn adding_a_card_preserves_manually_positioned_upstream_nodes() {
    let mut graph = PluginGraph::with_default_rack();
    let upstream_id = graph.node_id_for_role(NodeRole::ReplayGain).unwrap();
    // Canvas location does not determine signal order. An upstream node may
    // have been dragged beyond Matrix and must not be shifted by insertion.
    graph.nodes.get_mut(&upstream_id).unwrap().position = NodePosition::new(5000.0, 800.0);
    graph.insert_plugin(2, &PluginType::Gain).unwrap();
    let position = graph.node_position(upstream_id).unwrap();
    assert_eq!(position.x, 5000.0);
    assert_eq!(position.y, 800.0);
}

fn external_key_graph(
    plugin_type: &PluginType,
) -> (PluginGraph, super::GraphNodeId, super::GraphNodeId) {
    let mut graph = PluginGraph::with_default_rack();
    let source = graph.add_user_plugin(&PluginType::Gain).unwrap();
    let keyed = graph.add_user_plugin(plugin_type).unwrap();
    match &mut graph.nodes.get_mut(&keyed).unwrap().plugin.settings {
        PluginSettings::Gate {
            sidechain_external, ..
        }
        | PluginSettings::DeEsser {
            sidechain_external, ..
        } => *sidechain_external = true,
        _ => unreachable!("fixture requires an external-key processor"),
    }
    graph.update_channel_dependent_plugins();
    for channel in 0..2 {
        graph
            .add_connection(source, channel, keyed, channel + 2)
            .unwrap();
    }
    (graph, source, keyed)
}

#[test]
fn external_key_routes_keep_program_width_and_distinct_engine_edges() {
    use sotf_audio::engine::PluginGraphEdgeKind;
    for plugin_type in [PluginType::Gate, PluginType::DeEsser] {
        let (mut graph, source, keyed) = external_key_graph(&plugin_type);
        assert!(!graph.is_linear(), "key bus requires the graph host");
        assert!(
            graph.linear_order().is_some(),
            "node order remains available"
        );
        graph.update_channel_dependent_plugins();
        graph.update_channel_dependent_plugins();
        graph.validate_routing().unwrap();
        assert_eq!(graph.nodes[&keyed].input_channels, 4);
        assert_eq!(graph.nodes[&keyed].output_channels, 2);
        assert_eq!(
            graph
                .connections
                .iter()
                .filter(|edge| edge.from_node == source && edge.to_node == keyed)
                .count(),
            4
        );
        let config = graph.to_plugin_graph_config(48000.0);
        let source_index = config
            .nodes
            .iter()
            .find(|node| node.plugin_type == "gain" && !node.bypassed)
            .unwrap()
            .id;
        let target = config
            .nodes
            .iter()
            .find(|node| {
                node.plugin_type
                    == if plugin_type == PluginType::Gate {
                        "gate"
                    } else {
                        "de_esser"
                    }
            })
            .unwrap();
        assert_eq!(target.input_channels, 2, "factory appends the key bus");
        let edges: Vec<_> = config
            .edges
            .iter()
            .filter(|edge| edge.from_node == source_index && edge.to_node == target.id)
            .collect();
        assert_eq!(edges.len(), 2);
        assert!(
            edges
                .iter()
                .any(|edge| edge.kind == PluginGraphEdgeKind::Audio)
        );
        assert!(
            edges
                .iter()
                .any(|edge| edge.kind == PluginGraphEdgeKind::Sidechain)
        );
        let reloaded: PluginGraph =
            serde_json::from_str(&serde_json::to_string(&graph).unwrap()).unwrap();
        reloaded.validate_routing().unwrap();
        assert_eq!(
            serde_json::to_value(reloaded.to_plugin_graph_config(48000.0)).unwrap(),
            serde_json::to_value(config).unwrap()
        );
    }
}

#[test]
fn external_key_routes_reject_partial_crossed_and_disabled_sources() {
    for plugin_type in [PluginType::Gate, PluginType::DeEsser] {
        let (graph, source, keyed) = external_key_graph(&plugin_type);
        let mut partial = graph.clone();
        partial
            .connections
            .retain(|edge| !(edge.to_node == keyed && edge.to_port == 3));
        assert!(
            partial
                .validate_routing()
                .unwrap_err()
                .contains("Connect input 4")
        );
        let mut crossed = graph.clone();
        for edge in crossed
            .connections
            .iter_mut()
            .filter(|edge| edge.to_node == keyed && edge.to_port >= 2)
        {
            edge.from_port = 1 - edge.from_port;
        }
        assert!(
            crossed
                .validate_routing()
                .unwrap_err()
                .contains("Use a Matrix plugin")
        );
        let mut disabled = graph.clone();
        disabled.nodes.get_mut(&source).unwrap().plugin.enabled = false;
        assert!(
            disabled
                .validate_routing()
                .unwrap_err()
                .contains("External sidechain")
        );
        let mut suspended = graph;
        suspended.nodes.get_mut(&source).unwrap().plugin.suspended = true;
        assert!(
            suspended
                .validate_routing()
                .unwrap_err()
                .contains("External sidechain")
        );
    }
}

#[test]
fn disabling_external_key_removes_key_ports_and_restores_rack_eligibility() {
    for plugin_type in [PluginType::Gate, PluginType::DeEsser] {
        let (mut graph, _, keyed) = external_key_graph(&plugin_type);
        match &mut graph.nodes.get_mut(&keyed).unwrap().plugin.settings {
            PluginSettings::Gate {
                sidechain_external, ..
            }
            | PluginSettings::DeEsser {
                sidechain_external, ..
            } => *sidechain_external = false,
            _ => unreachable!(),
        }
        graph.update_channel_dependent_plugins();
        assert_eq!(graph.nodes[&keyed].input_channels, 2);
        assert!(
            graph
                .connections
                .iter()
                .filter(|edge| edge.to_node == keyed)
                .all(|edge| edge.to_port < 2)
        );
        graph.validate_routing().unwrap();
        assert!(graph.is_linear());
    }
}

#[test]
fn inactive_resource_nodes_use_transparent_stages_without_resource_configuration() {
    let mut graph = PluginGraph::with_default_rack();
    let resource = graph.add_user_plugin(&PluginType::Convolution).unwrap();
    graph.nodes.get_mut(&resource).unwrap().plugin.suspended = true;
    let node_count = graph.nodes.len();
    let config = graph.to_plugin_graph_config(48000.0);
    assert_eq!(config.nodes.len(), node_count);
    let inactive: Vec<_> = config.nodes.iter().filter(|node| node.bypassed).collect();
    assert_eq!(
        inactive.len(),
        2,
        "disabled ReplayGain and suspended Convolution"
    );
    for node in inactive {
        assert_eq!(node.plugin_type, "gain");
        assert_eq!(
            node.parameters,
            serde_json::json!({"channels": 2, "gain_db": 0.0})
        );
        assert!(config.edges.iter().any(|edge| edge.from_node == node.id));
        assert!(config.edges.iter().any(|edge| edge.to_node == node.id));
    }
}

#[test]
fn inactive_channel_changing_nodes_are_refused_before_routing_apply() {
    let (mut graph, _, keyed) = external_key_graph(&PluginType::DeEsser);
    graph.nodes.get_mut(&keyed).unwrap().plugin.enabled = false;
    assert!(
        graph
            .validate_routing()
            .unwrap_err()
            .contains("Cannot bypass")
    );
    // The config keeps the unsupported node so direct submission also receives
    // the engine's width error rather than a silently severed audio route.
    let config = graph.to_plugin_graph_config(48000.0);
    assert!(
        config
            .nodes
            .iter()
            .any(|node| node.plugin_type == "de_esser" && node.bypassed)
    );
}

#[test]
fn keyed_graph_control_indices_include_inactive_placeholders() {
    for plugin_type in [PluginType::Gate, PluginType::DeEsser] {
        let (graph, source, keyed) = external_key_graph(&plugin_type);
        let config = graph.to_plugin_graph_config(48000.0);
        let source_index = graph.get_engine_index(source).unwrap();
        let target_index = graph.get_engine_index(keyed).unwrap();
        assert_eq!(config.nodes[source_index].plugin_type, "gain");
        assert!(!config.nodes[source_index].bypassed);
        assert_eq!(
            config.nodes[target_index].plugin_type,
            plugin_type.wire_name()
        );
        assert_eq!(target_index, source_index + 1);
        assert!(
            config.nodes[..source_index]
                .iter()
                .any(|node| node.bypassed),
            "disabled ReplayGain retains its control slot"
        );
    }
}

#[test]
fn keyed_presets_restore_complete_routing_and_reject_invalid_graph_transactionally() {
    let directory = tempfile::tempdir().unwrap();
    for plugin_type in [PluginType::Gate, PluginType::DeEsser] {
        let (mut graph, _, keyed) = external_key_graph(&plugin_type);
        // Removed instances must not have their IDs reused after restoration.
        graph.next_plugin_id = 1000;
        graph.validate_routing().unwrap();
        graph.save_to_file(directory.path(), "keyed").unwrap();
        let path = directory.path().join("keyed.json");
        let saved: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved["version"], 3);
        let mut restored = PluginGraph::with_default_rack();
        assert!(
            restored
                .load_from_file(directory.path(), "keyed")
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            serde_json::to_value(&restored).unwrap(),
            serde_json::to_value(&graph).unwrap()
        );
        assert_eq!(
            serde_json::to_value(restored.to_plugin_graph_config(48000.0)).unwrap(),
            serde_json::to_value(graph.to_plugin_graph_config(48000.0)).unwrap()
        );
        restored.update_channel_dependent_plugins();
        assert_eq!(
            serde_json::to_value(&restored).unwrap(),
            serde_json::to_value(&graph).unwrap()
        );
        let before = serde_json::to_value(&restored).unwrap();
        let mut broken = graph.clone();
        broken
            .connections
            .retain(|edge| !(edge.to_node == keyed && edge.to_port == 3));
        std::fs::write(
            &path,
            serde_json::to_string(&serde_json::json!({
                "version": 3, "graph": broken,
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(restored.load_from_file(directory.path(), "keyed").is_err());
        assert_eq!(serde_json::to_value(&restored).unwrap(), before);
    }
}

#[test]
fn ordinary_rack_presets_keep_version_two() {
    let directory = tempfile::tempdir().unwrap();
    let graph = PluginGraph::with_default_rack();
    graph.save_to_file(directory.path(), "rack").unwrap();
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(directory.path().join("rack.json")).unwrap())
            .unwrap();
    assert_eq!(value["version"], 2);
    assert!(value.get("graph").is_none());
    assert!(value["plugins"].as_array().is_some());
}

#[test]
fn branched_graph_control_indices_survive_serialized_reload() {
    let mut graph = PluginGraph::new();
    let input = graph.add_special_node(SpecialNodeType::Input, NodePosition::new(0.0, 0.0), 2);
    let output = graph.add_special_node(SpecialNodeType::Output, NodePosition::new(500.0, 0.0), 2);
    let left = graph
        .add_plugin_node(&PluginType::Gain, NodePosition::new(100.0, 0.0))
        .unwrap();
    let right = graph
        .add_plugin_node(&PluginType::EQ, NodePosition::new(100.0, 100.0))
        .unwrap();
    for channel in 0..2 {
        for branch in [left, right] {
            graph
                .add_connection(input, channel, branch, channel)
                .unwrap();
            graph
                .add_connection(branch, channel, output, channel)
                .unwrap();
        }
    }
    assert!(!graph.is_linear());
    let expected = serde_json::to_value(graph.to_plugin_graph_config(48000.0)).unwrap();
    let serialized = serde_json::to_string(&graph).unwrap();
    for _ in 0..16 {
        let reloaded: PluginGraph = serde_json::from_str(&serialized).unwrap();
        let config = reloaded.to_plugin_graph_config(48000.0);
        assert_eq!(serde_json::to_value(&config).unwrap(), expected);
        for id in [left, right] {
            let index = reloaded.get_engine_index(id).unwrap();
            assert_eq!(
                config.nodes[index].plugin_type,
                reloaded.nodes[&id].plugin.plugin_type().wire_name()
            );
        }
    }
}
