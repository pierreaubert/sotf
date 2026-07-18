// ============================================================================
// Property-Based Tests for sotf-types
// ============================================================================
//
// This module uses proptest to verify invariants of shared configuration types
// across a wide range of generated inputs.

use proptest::prelude::*;
use serde_json::json;
use sotf_types::{
    AudioFrame, EngineConfig, PluginConfig, PluginGraphConfig, PluginGraphEdgeConfig,
    PluginGraphNodeConfig,
};

// ============================================================================
// PluginGraphConfig Property Tests
// ============================================================================

fn plugin_graph_node_strategy(id: usize) -> impl Strategy<Value = PluginGraphNodeConfig> {
    (
        "[a-zA-Z_][a-zA-Z0-9_]*",
        1usize..16,
        prop::collection::vec(-100.0f32..100.0f32, 0..8),
    )
        .prop_map(
            move |(plugin_type, input_channels, _params)| PluginGraphNodeConfig {
                id,
                plugin_type,
                parameters: json!({}),
                input_channels,
                bypassed: false,
            },
        )
}

/// Build a valid DAG: nodes 0..n, edges only from smaller to larger ids.
fn valid_dag_strategy() -> BoxedStrategy<PluginGraphConfig> {
    prop::collection::vec(plugin_graph_node_strategy(0), 0..12)
        .prop_flat_map(|nodes| {
            let n = nodes.len();
            let nodes: Vec<PluginGraphNodeConfig> = nodes
                .into_iter()
                .enumerate()
                .map(|(i, mut node)| {
                    node.id = i;
                    node
                })
                .collect();

            if n < 2 {
                // No valid forward edges possible.
                return Just(PluginGraphConfig {
                    nodes: nodes.clone(),
                    edges: vec![],
                })
                .boxed();
            }

            let edge_strategy = (0usize..(n - 1)).prop_flat_map(move |from| {
                ((from + 1)..n).prop_map(move |to| PluginGraphEdgeConfig::new(from, to))
            });

            prop::collection::vec(edge_strategy, 0..=((n * (n - 1) / 2).min(32)))
                .prop_map(move |edges| PluginGraphConfig {
                    nodes: nodes.clone(),
                    edges,
                })
                .boxed()
        })
        .boxed()
}

/// Build a valid DAG that contains at least one edge (so reversing an edge creates a cycle).
fn valid_dag_with_edge_strategy() -> BoxedStrategy<PluginGraphConfig> {
    prop::collection::vec(plugin_graph_node_strategy(0), 2..12)
        .prop_flat_map(|nodes| {
            let n = nodes.len();
            let nodes: Vec<PluginGraphNodeConfig> = nodes
                .into_iter()
                .enumerate()
                .map(|(i, mut node)| {
                    node.id = i;
                    node
                })
                .collect();

            // First edge is mandatory; additional edges are optional.
            let mandatory_edge = (0usize..(n - 1)).prop_flat_map(move |from| {
                ((from + 1)..n).prop_map(move |to| PluginGraphEdgeConfig::new(from, to))
            });

            let extra_edge_strategy = (0usize..(n - 1)).prop_flat_map(move |from| {
                ((from + 1)..n).prop_map(move |to| PluginGraphEdgeConfig::new(from, to))
            });

            (
                mandatory_edge,
                prop::collection::vec(extra_edge_strategy, 0..=((n * (n - 1) / 2).min(32))),
            )
                .prop_map(move |(mandatory, mut extras)| {
                    extras.push(mandatory);
                    PluginGraphConfig {
                        nodes: nodes.clone(),
                        edges: extras,
                    }
                })
                .boxed()
        })
        .boxed()
}

proptest! {
    // INVARIANT: Any graph with only forward edges (by id) is a valid DAG.
    #[test]
    fn valid_dag_accepts(graph in valid_dag_strategy()) {
        prop_assert!(graph.validate().is_ok(), "Forward-edge graph should be a valid DAG");
    }

    // INVARIANT: A self-loop on any node creates a cycle and is rejected.
    #[test]
    fn self_loop_rejects(graph in valid_dag_strategy(), node_idx in 0usize..12) {
        if node_idx < graph.nodes.len() {
            let mut graph = graph;
            let id = graph.nodes[node_idx].id;
            graph.edges.push(PluginGraphEdgeConfig::new(id, id));
            let result = graph.validate();
            prop_assert!(result.is_err(), "Self-loop should be rejected");
            prop_assert!(result.unwrap_err().contains("acyclic"));
        }
    }

    // INVARIANT: Reversing any edge in a DAG creates a cycle.
    #[test]
    fn backward_edge_rejects(
        graph in valid_dag_with_edge_strategy(),
        edge_idx in 0usize..32,
    ) {
        if !graph.edges.is_empty() {
            let idx = edge_idx % graph.edges.len();
            let edge = graph.edges[idx].clone();
            let mut graph = graph;
            graph.edges.push(PluginGraphEdgeConfig::new(edge.to_node, edge.from_node));
            let result = graph.validate();
            prop_assert!(result.is_err(), "Reversing an edge should create a cycle");
            prop_assert!(result.unwrap_err().contains("acyclic"));
        }
    }

    // INVARIANT: Duplicate node ids are rejected.
    #[test]
    fn duplicate_node_ids_reject(
        nodes in prop::collection::vec(plugin_graph_node_strategy(0), 2..8)
    ) {
        if nodes.len() >= 2 {
            let mut nodes = nodes;
            nodes[1].id = nodes[0].id;
            let result = PluginGraphConfig::try_new(nodes, vec![]);
            prop_assert!(result.is_err(), "Duplicate node ids should be rejected");
            prop_assert!(result.unwrap_err().contains("duplicate"));
        }
    }

    // INVARIANT: Edges referencing missing endpoints are rejected.
    #[test]
    fn missing_edge_endpoint_rejects(
        nodes in prop::collection::vec(plugin_graph_node_strategy(0), 1..8),
        missing_from in any::<bool>(),
    ) {
        if !nodes.is_empty() {
            let nodes: Vec<PluginGraphNodeConfig> = nodes
                .into_iter()
                .enumerate()
                .map(|(i, mut node)| {
                    node.id = i;
                    node
                })
                .collect();
            let max_id = nodes.iter().map(|n| n.id).max().unwrap_or(0);
            let bad_edge = if missing_from {
                PluginGraphEdgeConfig::new(max_id + 1, nodes[0].id)
            } else {
                PluginGraphEdgeConfig::new(nodes[0].id, max_id + 1)
            };
            let result = PluginGraphConfig::try_new(nodes, vec![bad_edge]);
            prop_assert!(result.is_err(), "Missing edge endpoint should be rejected");
        }
    }

    // INVARIANT: An empty graph validates successfully.
    #[test]
    fn empty_graph_accepts(_dummy in 0u8..1) {
        let graph = PluginGraphConfig::try_new(vec![], vec![]).unwrap();
        prop_assert!(graph.validate().is_ok(), "Empty graph should validate");
    }

    // INVARIANT: Serialization round-trip preserves validation result.
    #[test]
    fn graph_serde_roundtrip_preserves_validation(graph in valid_dag_strategy()) {
        let json = serde_json::to_string(&graph).unwrap();
        let decoded: PluginGraphConfig = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(
            graph.validate().is_ok(),
            decoded.validate().is_ok(),
            "Serialization round-trip should preserve validation result"
        );
    }

    // INVARIANT: Disconnected nodes (no edges) still form a valid graph.
    #[test]
    fn disconnected_nodes_accept(
        nodes in prop::collection::vec(plugin_graph_node_strategy(0), 1..12),
    ) {
        let nodes: Vec<PluginGraphNodeConfig> = nodes
            .into_iter()
            .enumerate()
            .map(|(i, mut node)| {
                node.id = i;
                node
            })
            .collect();
        let result = PluginGraphConfig::try_new(nodes, vec![]);
        prop_assert!(result.is_ok(), "Disconnected nodes should validate: {:?}", result);
    }

    // INVARIANT: A node with input_channels == 0 is rejected.
    #[test]
    fn zero_input_channels_reject(
        mut graph in valid_dag_strategy(),
        node_idx in 0usize..12,
    ) {
        if node_idx < graph.nodes.len() {
            graph.nodes[node_idx].input_channels = 0;
            let result = graph.validate();
            prop_assert!(result.is_err(), "Zero input_channels should be rejected");
            prop_assert!(result.unwrap_err().contains("input_channels"));
        }
    }

    // INVARIANT: Adding an edge that creates a cycle via a longer path is rejected.
    #[test]
    fn cycle_via_longer_path_rejects(
        graph in valid_dag_with_edge_strategy(),
        chain_len in 2usize..8,
    ) {
        if graph.nodes.len() >= chain_len {
            let mut graph = graph;
            let path: Vec<usize> = (0..chain_len).collect();
            for window in path.windows(2) {
                graph.edges.push(PluginGraphEdgeConfig::new(window[0], window[1]));
            }
            // Close the cycle.
            graph
                .edges
                .push(PluginGraphEdgeConfig::new(path[path.len() - 1], path[0]));
            let result = graph.validate();
            prop_assert!(result.is_err(), "Cycle should be rejected");
            prop_assert!(result.unwrap_err().contains("acyclic"));
        }
    }
}

// ============================================================================
// AudioFrame Property Tests
// ============================================================================

fn valid_audio_frame_strategy() -> impl Strategy<Value = AudioFrame> {
    (1usize..32, 1usize..8, 8000u32..192000u32, -1.0f32..1.0f32).prop_map(
        |(num_frames, num_channels, sample_rate, fill_value)| {
            let data = vec![fill_value; num_frames * num_channels];
            AudioFrame::new(data, num_frames, num_channels, sample_rate)
        },
    )
}

proptest! {
    // INVARIANT: try_new succeeds when data length matches num_frames * num_channels.
    #[test]
    fn audio_frame_try_new_succeeds_when_dimensions_match(
        num_frames in 0usize..32,
        num_channels in 1usize..8,
        sample_rate in 8000u32..192000u32,
    ) {
        let data = vec![0.0f32; num_frames * num_channels];
        let result = AudioFrame::try_new(data, num_frames, num_channels, sample_rate);
        prop_assert!(result.is_ok(), "Matching dimensions should succeed");
        let frame = result.unwrap();
        prop_assert_eq!(frame.num_frames, num_frames);
        prop_assert_eq!(frame.num_channels, num_channels);
        prop_assert_eq!(frame.sample_rate, sample_rate);
    }

    // INVARIANT: try_new fails when data length does not match dimensions.
    #[test]
    fn audio_frame_try_new_fails_on_length_mismatch(
        num_frames in 0usize..16,
        num_channels in 1usize..8,
        sample_rate in 8000u32..192000u32,
        extra in 1usize..8,
    ) {
        let expected = num_frames * num_channels;
        let data = vec![0.0f32; expected + extra];
        let result = AudioFrame::try_new(data, num_frames, num_channels, sample_rate);
        prop_assert!(result.is_err(), "Length mismatch should fail");
        prop_assert!(result.unwrap_err().contains("data length"));
    }

    // INVARIANT: silent() creates a frame filled with zeros.
    #[test]
    fn audio_frame_silent_creates_zeros(
        num_frames in 0usize..32,
        num_channels in 1usize..8,
        sample_rate in 8000u32..192000u32,
    ) {
        let frame = AudioFrame::silent(num_frames, num_channels, sample_rate);
        prop_assert_eq!(frame.num_frames, num_frames);
        prop_assert_eq!(frame.num_channels, num_channels);
        prop_assert_eq!(frame.sample_rate, sample_rate);
        prop_assert!(frame.data.iter().all(|&s| s == 0.0), "Silent frame must be all zeros");
    }

    // INVARIANT: num_samples() is always num_frames * num_channels.
    #[test]
    fn audio_frame_num_samples_consistent(frame in valid_audio_frame_strategy()) {
        prop_assert_eq!(
            frame.num_samples(),
            frame.num_frames * frame.num_channels,
            "num_samples must equal num_frames * num_channels"
        );
        prop_assert_eq!(frame.data.len(), frame.num_samples(), "data length must match num_samples");
    }

    // INVARIANT: clear() zeros the data without changing dimensions.
    #[test]
    fn audio_frame_clear_preserves_dimensions(frame in valid_audio_frame_strategy()) {
        let mut frame = frame;
        let original_frames = frame.num_frames;
        let original_channels = frame.num_channels;
        let original_rate = frame.sample_rate;
        frame.clear();
        prop_assert_eq!(frame.num_frames, original_frames);
        prop_assert_eq!(frame.num_channels, original_channels);
        prop_assert_eq!(frame.sample_rate, original_rate);
        prop_assert!(frame.data.iter().all(|&s| s == 0.0), "clear() must zero all samples");
    }

    // INVARIANT: try_new rejects overflowing dimensions.
    #[test]
    fn audio_frame_try_new_rejects_overflow(_dummy in 0u8..1) {
        let result = AudioFrame::try_new(vec![], usize::MAX, 2, 48000);
        prop_assert!(result.is_err(), "Overflowing dimensions should fail");
        prop_assert!(result.unwrap_err().contains("overflow"));
    }
}

// ============================================================================
// EngineConfig Property Tests
// ============================================================================

fn valid_engine_config_strategy() -> impl Strategy<Value = EngineConfig> {
    (
        1usize..4096,
        1u32..2000u32,
        8000u32..192000u32,
        1usize..16,
        1usize..16,
        0.0f32..1.0f32,
    )
        .prop_map(
            |(
                frame_size,
                buffer_ms,
                output_sample_rate,
                input_channels,
                output_channels,
                volume,
            )| {
                EngineConfig {
                    frame_size,
                    buffer_ms,
                    output_sample_rate,
                    input_channels,
                    output_channels,
                    volume,
                    ..Default::default()
                }
            },
        )
}

proptest! {
    // INVARIANT: A config with all fields in valid ranges validates successfully.
    #[test]
    fn valid_engine_config_accepts(config in valid_engine_config_strategy()) {
        prop_assert!(config.validate().is_ok(), "Valid config should validate");
    }

    // INVARIANT: validate() rejects frame_size == 0.
    #[test]
    fn engine_config_rejects_zero_frame_size(config in valid_engine_config_strategy()) {
        let mut config = config;
        config.frame_size = 0;
        let result = config.validate();
        prop_assert!(result.is_err(), "frame_size == 0 should be rejected");
        prop_assert!(result.unwrap_err().contains("frame_size"));
    }

    // INVARIANT: validate() rejects buffer_ms == 0.
    #[test]
    fn engine_config_rejects_zero_buffer_ms(config in valid_engine_config_strategy()) {
        let mut config = config;
        config.buffer_ms = 0;
        let result = config.validate();
        prop_assert!(result.is_err(), "buffer_ms == 0 should be rejected");
        prop_assert!(result.unwrap_err().contains("buffer_ms"));
    }

    // INVARIANT: validate() rejects output_sample_rate == 0.
    #[test]
    fn engine_config_rejects_zero_sample_rate(config in valid_engine_config_strategy()) {
        let mut config = config;
        config.output_sample_rate = 0;
        let result = config.validate();
        prop_assert!(result.is_err(), "output_sample_rate == 0 should be rejected");
        prop_assert!(result.unwrap_err().contains("output_sample_rate"));
    }

    // INVARIANT: validate() rejects input_channels == 0.
    #[test]
    fn engine_config_rejects_zero_input_channels(config in valid_engine_config_strategy()) {
        let mut config = config;
        config.input_channels = 0;
        let result = config.validate();
        prop_assert!(result.is_err(), "input_channels == 0 should be rejected");
        prop_assert!(result.unwrap_err().contains("input_channels"));
    }

    // INVARIANT: validate() rejects output_channels == 0.
    #[test]
    fn engine_config_rejects_zero_output_channels(config in valid_engine_config_strategy()) {
        let mut config = config;
        config.output_channels = 0;
        let result = config.validate();
        prop_assert!(result.is_err(), "output_channels == 0 should be rejected");
        prop_assert!(result.unwrap_err().contains("output_channels"));
    }

    // INVARIANT: validate() rejects volume outside [0.0, 1.0].
    #[test]
    fn engine_config_rejects_out_of_bounds_volume(
        config in valid_engine_config_strategy(),
        is_high in any::<bool>(),
        offset in 0.0001f32..100.0f32,
    ) {
        let mut config = config;
        config.volume = if is_high { 1.0 + offset } else { -offset };
        let result = config.validate();
        prop_assert!(result.is_err(), "volume {} should be rejected", config.volume);
        prop_assert!(result.unwrap_err().contains("volume"));
    }

    // INVARIANT: validate() rejects non-finite volume (NaN / Inf).
    #[test]
    fn engine_config_rejects_non_finite_volume(
        config in valid_engine_config_strategy(),
        volume in prop::sample::select(&[f32::NAN, f32::INFINITY, f32::NEG_INFINITY]),
    ) {
        let mut config = config;
        config.volume = volume;
        let result = config.validate();
        prop_assert!(result.is_err(), "non-finite volume should be rejected");
        prop_assert!(result.unwrap_err().contains("volume"));
    }

    // INVARIANT: validate() rejects plugins with empty plugin_type.
    #[test]
    fn engine_config_rejects_invalid_plugin(config in valid_engine_config_strategy()) {
        let mut config = config;
        config.plugins = vec![PluginConfig::new("", json!({}))];
        let result = config.validate();
        prop_assert!(result.is_err(), "Invalid plugin should be rejected");
        prop_assert!(result.unwrap_err().contains("plugins[0]"));
    }

    // INVARIANT: Default EngineConfig validates successfully.
    #[test]
    fn engine_config_default_validates(_dummy in 0u8..1) {
        prop_assert!(EngineConfig::default().validate().is_ok(), "Default config should validate");
    }

    // INVARIANT: sanitize() fixes zero frame_size and output_sample_rate.
    #[test]
    fn engine_config_sanitize_fixes_zeros(config in valid_engine_config_strategy()) {
        let mut config = config;
        config.frame_size = 0;
        config.output_sample_rate = 0;
        config.sanitize();
        prop_assert_eq!(config.frame_size, 1024);
        prop_assert_eq!(config.output_sample_rate, 48000);
    }

    // INVARIANT: try_new returns Ok for valid configs and Err for invalid configs.
    #[test]
    fn engine_config_try_new_roundtrip(config in valid_engine_config_strategy()) {
        let cloned = config.clone();
        let result = EngineConfig::try_new(config);
        prop_assert!(result.is_ok(), "try_new should accept valid config");
        let returned = result.unwrap();
        prop_assert_eq!(returned.frame_size, cloned.frame_size);
        prop_assert_eq!(returned.buffer_ms, cloned.buffer_ms);
        prop_assert_eq!(returned.output_sample_rate, cloned.output_sample_rate);
        prop_assert_eq!(returned.input_channels, cloned.input_channels);
        prop_assert_eq!(returned.output_channels, cloned.output_channels);
    }

    // INVARIANT: total_buffer_frames is monotonic in buffer_ms.
    #[test]
    fn engine_config_buffer_frames_monotonic(
        frame_size in 1usize..4096,
        sample_rate in 8000u32..192000u32,
        ms_a in 1u32..1000u32,
        ms_b in 1u32..1000u32,
    ) {
        let config_a = EngineConfig {
            frame_size,
            buffer_ms: ms_a.min(ms_b),
            output_sample_rate: sample_rate,
            ..Default::default()
        };
        let config_b = EngineConfig {
            frame_size,
            buffer_ms: ms_a.max(ms_b),
            output_sample_rate: sample_rate,
            ..Default::default()
        };
        prop_assert!(
            config_a.total_buffer_frames() <= config_b.total_buffer_frames(),
            "total_buffer_frames must be monotonic in buffer_ms"
        );
    }

    // INVARIANT: Serialization round-trip preserves validate() result.
    #[test]
    fn engine_config_serde_roundtrip_preserves_validation(config in valid_engine_config_strategy()) {
        let json = serde_json::to_string(&config).unwrap();
        let decoded: EngineConfig = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(
            config.validate().is_ok(),
            decoded.validate().is_ok(),
            "Serialization round-trip should preserve validation result"
        );
    }
}
