//! Shared "apply Room EQ result to chain" logic — used by both the GPUI and
//! TUI frontends so the algorithm lives in one place.
//!
//! Two entry points mirror the two flows the user experiences:
//!
//! - [`apply_room_eq_rack_to_chain`] — when the optimizer output is rack-
//!   compatible (no global plugins, no per-channel drivers, no routed bass
//!   management), insert/update two named EQ plugins ("Broadband EQ" +
//!   "Room EQ") into the linear `PluginGraph`. Caller flushes via
//!   `update_plugins`.
//!
//! - [`apply_room_eq_graph_to_chain`] — for routed multi-driver/bass-
//!   management chains, build a `PluginGraphConfig` for the engine and a
//!   matching UI [`PluginGraph`] with auto-laid-out node positions.
//!   Caller flushes via `update_plugin_graph(config)`.
//!
//! The algorithms here used to live in `app-gpui/components/room_eq/
//! step_6_export.rs`. Moving them to `sotf-player` keeps UIs as thin shells
//! and ensures the TUI sees the same behavior as the GPUI app.

use crate::EQFilter;
use crate::plugin_graph::{NodePosition, PluginGraph, SpecialNodeType};
#[cfg(test)]
use crate::room_eq_types::ChannelDspChain;
use crate::room_eq_types::{DspChainOutput, parse_eq_filters_from_json};
use sotf_audio::engine::PluginGraphConfig;
use sotf_audio::plugins::{PluginSettings, PluginType};

mod misc;
mod types;

pub use misc::*;
pub use types::*;

use misc::derive_plugin_name;

/// Apply a rack-compatible `DspChainOutput` to the chain by inserting/
/// updating the named "Broadband EQ" and "Room EQ" plugins.
///
/// `channel_names` carries the recording-config output channel order
/// (FL, FR, C, …). It must match the audio engine's channel layout —
/// EQ plugins map `channel_filters[i]` to audio channel `i`.
///
/// Returns an outcome with the totals so the UI can decide whether to
/// surface a "no filters detected" notice.
pub fn apply_room_eq_rack_to_chain(
    graph: &mut PluginGraph,
    dsp_output: &DspChainOutput,
    channel_names: &[String],
) -> RackApplyOutcome {
    // Collect EQ filters per channel in output channel order.
    let mut per_channel_filters: Vec<Vec<EQFilter>> = Vec::with_capacity(channel_names.len());
    let mut per_channel_broadband: Vec<Vec<EQFilter>> = Vec::with_capacity(channel_names.len());
    for channel_name in channel_names {
        if let Some(channel_dsp) = dsp_output.channels.get(channel_name) {
            let (channel_eq_filters, channel_bb_filters) = classify_channel_eq_filters(channel_dsp);
            log::info!(
                "Channel '{}': {} EQ filters, {} broadband filters",
                channel_name,
                channel_eq_filters.len(),
                channel_bb_filters.len(),
            );
            per_channel_filters.push(channel_eq_filters);
            per_channel_broadband.push(channel_bb_filters);
        } else {
            log::info!(
                "Channel '{}': no DSP output, using empty filters",
                channel_name
            );
            per_channel_filters.push(Vec::new());
            per_channel_broadband.push(Vec::new());
        }
    }

    let total_filters: usize = per_channel_filters.iter().map(|f| f.len()).sum();
    let total_bb: usize = per_channel_broadband.iter().map(|f| f.len()).sum();

    let num_channels = per_channel_filters.len();
    let global_filters = per_channel_filters.first().cloned().unwrap_or_default();
    let global_bb = per_channel_broadband.first().cloned().unwrap_or_default();

    log::info!(
        "Applying room EQ with {} channels, {} total filters (per-channel mode)",
        num_channels,
        total_filters
    );

    upsert_named_room_eq_plugins(
        graph,
        num_channels,
        &global_bb,
        &per_channel_broadband,
        total_bb,
        &global_filters,
        &per_channel_filters,
    );

    RackApplyOutcome {
        num_channels,
        total_filters,
        total_broadband: total_bb,
    }
}

/// Apply DSP output parameters to a `PluginSettings` in-place.
///
/// Handles the common plugin types from roomeq: EQ (filters / channel_filters),
/// Gain (gain_db / channel_gains), and Delay (delay_ms / channel_delays_ms).
///
/// The factored RoomEQ builder emits multichannel plugins carrying
/// per-channel parameter arrays (`channel_gains`, `channel_filters`,
/// `channel_delays_ms`). `PluginSettings::EQ` carries `channel_filters` +
/// `per_channel_mode` natively. `PluginSettings::Gain` and `::Delay` are
/// scalar today — for those we surface the channel-0 value as a
/// representative default so the UI plugin-graph editor displays
/// something meaningful instead of an unrelated default. The actual
/// engine plugin gets the full per-channel array via the JSON the
/// factory consumes; this function only feeds the UI representation.
fn apply_dsp_params_to_settings(
    settings: &mut PluginSettings,
    plugin_type_str: &str,
    parameters: &serde_json::Value,
) {
    let lower = plugin_type_str.to_lowercase();
    match lower.as_str() {
        "eq" => {
            if let PluginSettings::EQ {
                filters,
                channel_filters,
                per_channel_mode,
                ..
            } = settings
            {
                // Prefer per-channel filter list when present (factored builder).
                if let Some(per_ch) = parameters.get("channel_filters").and_then(|v| v.as_array()) {
                    let per_channel: Vec<Vec<EQFilter>> = per_ch
                        .iter()
                        .map(|ch| {
                            ch.as_array()
                                .map(|arr| parse_eq_filters_from_json(arr))
                                .unwrap_or_default()
                        })
                        .collect();
                    *channel_filters = Some(per_channel);
                    *per_channel_mode = true;
                    // Keep a representative `filters` vec so the legacy
                    // single-channel-EQ UI path shows something useful when
                    // per-channel mode is off.
                    if let Some(first) = channel_filters.as_ref().and_then(|cf| cf.first()) {
                        *filters = first.clone();
                    }
                } else if let Some(filter_arr) =
                    parameters.get("filters").and_then(|v| v.as_array())
                {
                    *filters = parse_eq_filters_from_json(filter_arr);
                }
            }
        }
        "gain" => {
            if let PluginSettings::Gain { gain_db, .. } = settings {
                // Per-channel array takes precedence; surface channel 0 as
                // the scalar representative.
                if let Some(per_ch) = parameters
                    .get("channel_gains")
                    .and_then(|v| v.as_array())
                    .filter(|arr| !arr.is_empty())
                {
                    if let Some(v) = per_ch[0].as_f64() {
                        *gain_db = v;
                    }
                } else if let Some(v) = parameters.get("gain_db").and_then(|v| v.as_f64()) {
                    *gain_db = v;
                }
            }
        }
        "delay" => {
            if let PluginSettings::Delay { delay_ms, .. } = settings {
                if let Some(per_ch) = parameters
                    .get("channel_delays_ms")
                    .and_then(|v| v.as_array())
                    .filter(|arr| !arr.is_empty())
                {
                    if let Some(v) = per_ch[0].as_f64() {
                        *delay_ms = v;
                    }
                } else if let Some(v) = parameters.get("delay_ms").and_then(|v| v.as_f64()) {
                    *delay_ms = v;
                }
            }
        }
        _ => {} // Other types keep defaults (crossover lacks a PluginSettings
                // variant today — its per-channel fields are visible via the
                // parameters() API at runtime).
    }
}

/// Build a UI-level [`PluginGraph`] from an engine [`PluginGraphConfig`].
///
/// Creates plugin nodes with default settings for each engine node,
/// applies the optimizer's actual parameters via [`apply_dsp_params_to_settings`],
/// adds Input/Output special nodes, wires connections, and auto-lays-out
/// positions left-to-right by topological depth.
pub fn build_ui_graph_from_config(config: &PluginGraphConfig) -> PluginGraph {
    let mut graph = PluginGraph::new();
    let graph_channels = config
        .nodes
        .iter()
        .map(|node| node.input_channels)
        .max()
        .unwrap_or(2)
        .max(1);

    // Add Input special node at the left
    let input_id = graph.add_special_node(
        SpecialNodeType::Input,
        NodePosition::new(50.0, 200.0),
        graph_channels,
    );

    let mut id_map: std::collections::HashMap<usize, crate::plugin_graph::GraphNodeId> =
        std::collections::HashMap::new();

    let x_spacing = 200.0;
    let y_spacing = 120.0;

    // Topological layout: depth from longest-incoming-path BFS.
    let mut node_depth: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for node in &config.nodes {
        node_depth.entry(node.id).or_insert(0);
    }
    for edge in &config.edges {
        let from_depth = node_depth.get(&edge.from_node).copied().unwrap_or(0);
        let to_entry = node_depth.entry(edge.to_node).or_insert(0);
        *to_entry = (*to_entry).max(from_depth + 1);
    }
    for _ in 0..config.nodes.len() {
        for edge in &config.edges {
            let from_depth = node_depth.get(&edge.from_node).copied().unwrap_or(0);
            let to_entry = node_depth.entry(edge.to_node).or_insert(0);
            *to_entry = (*to_entry).max(from_depth + 1);
        }
    }

    let mut depth_counts: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();

    for node_config in &config.nodes {
        let depth = node_depth.get(&node_config.id).copied().unwrap_or(0);
        let y_index = depth_counts.entry(depth).or_insert(0);
        let x = 250.0 + (depth as f32) * x_spacing;
        let y = 100.0 + (*y_index as f32) * y_spacing;
        *y_index += 1;

        let plugin_type = PluginType::from_name(&node_config.plugin_type).unwrap_or(PluginType::EQ);

        let node_id = graph.add_plugin_node(&plugin_type, NodePosition::new(x, y));

        let derived_name = derive_plugin_name(&node_config.plugin_type, &node_config.parameters);

        if let Some(node) = graph.nodes.get_mut(&node_id) {
            apply_dsp_params_to_settings(
                &mut node.plugin.settings,
                &node_config.plugin_type,
                &node_config.parameters,
            );
            node.plugin.name = derived_name;
        }

        id_map.insert(node_config.id, node_id);
    }

    // Output special node at the right.
    let max_depth = node_depth.values().max().copied().unwrap_or(0);
    let output_x = 250.0 + ((max_depth + 1) as f32) * x_spacing;
    let output_id = graph.add_special_node(
        SpecialNodeType::Output,
        NodePosition::new(output_x, 200.0),
        graph_channels,
    );

    // Wire connections between plugin nodes.
    for edge in &config.edges {
        if let (Some(&from), Some(&to)) = (id_map.get(&edge.from_node), id_map.get(&edge.to_node)) {
            for ch in 0..graph_channels {
                let _ = graph.add_connection(from, ch, to, ch);
            }
        }
    }

    // Connect Input → first-depth plugin nodes (no incoming edges).
    let nodes_with_incoming: std::collections::HashSet<usize> =
        config.edges.iter().map(|e| e.to_node).collect();
    for node_config in &config.nodes {
        if !nodes_with_incoming.contains(&node_config.id)
            && let Some(&graph_id) = id_map.get(&node_config.id)
        {
            for ch in 0..graph_channels {
                let _ = graph.add_connection(input_id, ch, graph_id, ch);
            }
        }
    }

    // Connect last-depth plugin nodes (no outgoing edges) → Output.
    let nodes_with_outgoing: std::collections::HashSet<usize> =
        config.edges.iter().map(|e| e.from_node).collect();
    for node_config in &config.nodes {
        if !nodes_with_outgoing.contains(&node_config.id)
            && let Some(&graph_id) = id_map.get(&node_config.id)
        {
            for ch in 0..graph_channels {
                let _ = graph.add_connection(graph_id, ch, output_id, ch);
            }
        }
    }

    graph
}

/// Apply a `DspChainOutput` to a UI plugin graph, auto-selecting between
/// the linear-rack and routed-graph paths.
///
/// `channel_names` is consumed only by the rack path (used to map filter
/// lists to output channel slots). The graph path derives channel order
/// from `dsp_output.metadata.bass_management.routing_graph.input_channels`
/// directly. Callers may pass an empty slice when the output is known to
/// require a graph; the parameter is kept here so call sites that don't
/// know in advance which path will be taken can hand it through unchanged.
pub fn apply_room_eq_to_chain(
    graph: &mut PluginGraph,
    dsp_output: &DspChainOutput,
    sample_rate: f64,
    channel_names: &[String],
) -> Result<RoomEqApplyOutcome, String> {
    use crate::room_eq_types::DspChainOutputExt;
    if dsp_output.is_rack_compatible() {
        let outcome = apply_room_eq_rack_to_chain(graph, dsp_output, channel_names);
        Ok(RoomEqApplyOutcome::Rack(outcome))
    } else {
        let outcome = apply_room_eq_graph_to_chain(graph, dsp_output, sample_rate)?;
        Ok(RoomEqApplyOutcome::Graph(outcome))
    }
}

/// Apply a routed `DspChainOutput` (multi-driver crossovers, bass
/// management, etc.) by replacing the UI plugin graph with one that
/// matches the engine config the optimizer produced.
///
/// On success, `graph` is mutated to a fresh `PluginGraph` shaped from the
/// engine config and the returned `GraphApplyOutcome.config` carries the
/// `PluginGraphConfig` the caller must hand to
/// `Player::update_plugin_graph(config)`.
pub fn apply_room_eq_graph_to_chain(
    graph: &mut PluginGraph,
    dsp_output: &DspChainOutput,
    sample_rate: f64,
) -> Result<GraphApplyOutcome, String> {
    let config = crate::room_eq_types::build_room_eq_plugin_graph_config(dsp_output, sample_rate)
        .map_err(|e| format!("Failed to build Room EQ graph: {e}"))?;
    if config.nodes.is_empty() {
        return Err("No plugins in DSP output".to_string());
    }
    let num_nodes = config.nodes.len();
    let num_edges = config.edges.len();
    log::info!(
        "Applying room EQ as graph: {} nodes, {} edges",
        num_nodes,
        num_edges
    );
    *graph = build_ui_graph_from_config(&config);
    Ok(GraphApplyOutcome {
        config,
        num_nodes,
        num_edges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json_eq_plugin(label: Option<&str>, freq: f64) -> crate::room_eq_types::DspPluginConfig {
        let filters = serde_json::json!([{
            "type": "Peak",
            "freq": freq,
            "q": 1.0,
            "db_gain": -3.0,
        }]);
        let parameters = match label {
            Some(l) => serde_json::json!({ "filters": filters, "label": l }),
            None => serde_json::json!({ "filters": filters }),
        };
        crate::room_eq_types::DspPluginConfig {
            plugin_type: "eq".to_string(),
            parameters,
        }
    }

    fn make_channel_dsp(
        channel: &str,
        plugins: Vec<crate::room_eq_types::DspPluginConfig>,
    ) -> ChannelDspChain {
        // Build via JSON round-trip so we don't depend on field-level
        // construction of the autoeq type, which has many optional fields.
        let value = serde_json::json!({
            "channel": channel,
            "plugins": plugins,
        });
        serde_json::from_value(value).expect("valid ChannelDspChain")
    }

    fn make_dsp_output_for_channels(
        channel_specs: &[(&str, Vec<crate::room_eq_types::DspPluginConfig>)],
    ) -> DspChainOutput {
        let mut channels = serde_json::Map::new();
        for (name, plugins) in channel_specs {
            channels.insert(
                name.to_string(),
                serde_json::to_value(make_channel_dsp(name, plugins.clone())).unwrap(),
            );
        }
        let value = serde_json::json!({ "channels": channels });
        serde_json::from_value(value).expect("valid DspChainOutput")
    }

    #[test]
    fn apply_rack_inserts_room_eq_when_only_main_filters() {
        let mut graph = PluginGraph::with_default_rack();
        let dsp = make_dsp_output_for_channels(&[
            ("FL", vec![json_eq_plugin(None, 100.0)]),
            ("FR", vec![json_eq_plugin(None, 100.0)]),
        ]);
        let names = vec!["FL".to_string(), "FR".to_string()];
        let outcome = apply_room_eq_rack_to_chain(&mut graph, &dsp, &names);
        assert_eq!(outcome.num_channels, 2);
        assert!(outcome.total_filters >= 2);
        assert_eq!(outcome.total_broadband, 0);
        assert!(graph.find_plugin_index_by_name("Room EQ").is_some());
        assert!(graph.find_plugin_index_by_name("Broadband EQ").is_none());
    }

    #[test]
    fn apply_rack_inserts_both_named_eqs_when_broadband_present() {
        let mut graph = PluginGraph::with_default_rack();
        let dsp = make_dsp_output_for_channels(&[
            (
                "FL",
                vec![
                    json_eq_plugin(Some("broadband"), 80.0),
                    json_eq_plugin(None, 200.0),
                ],
            ),
            (
                "FR",
                vec![
                    json_eq_plugin(Some("broadband"), 80.0),
                    json_eq_plugin(None, 200.0),
                ],
            ),
        ]);
        let names = vec!["FL".to_string(), "FR".to_string()];
        let outcome = apply_room_eq_rack_to_chain(&mut graph, &dsp, &names);
        assert!(outcome.total_broadband >= 2);
        let bb_idx = graph.find_plugin_index_by_name("Broadband EQ").unwrap();
        let main_idx = graph.find_plugin_index_by_name("Room EQ").unwrap();
        assert!(
            bb_idx < main_idx,
            "Broadband EQ ({}) must come before Room EQ ({})",
            bb_idx,
            main_idx
        );
    }

    #[test]
    fn apply_rack_skips_missing_channel_silently() {
        let mut graph = PluginGraph::with_default_rack();
        let dsp = make_dsp_output_for_channels(&[("FL", vec![json_eq_plugin(None, 100.0)])]);
        // Optimizer didn't produce data for FR — should still apply, with
        // empty filters in the FR slot.
        let names = vec!["FL".to_string(), "FR".to_string()];
        let outcome = apply_room_eq_rack_to_chain(&mut graph, &dsp, &names);
        assert_eq!(outcome.num_channels, 2);
        assert!(graph.find_plugin_index_by_name("Room EQ").is_some());
    }

    // ========================================================================
    // apply_dsp_params_to_settings — per-channel JSON handling (factored
    // builder shapes).
    // ========================================================================

    #[test]
    fn apply_dsp_params_to_settings_gain_picks_channel_zero_from_array() {
        let mut settings = PluginSettings::Gain {
            channels: 2,
            gain_db: 0.0,
            smoothing_ms: 20.0,
        };
        let params = serde_json::json!({
            "channel_gains": [-3.0, -5.5],
            "gain_db": 0.0,
        });
        apply_dsp_params_to_settings(&mut settings, "gain", &params);
        match settings {
            PluginSettings::Gain { gain_db, .. } => assert_eq!(gain_db, -3.0),
            _ => panic!("expected Gain"),
        }
    }

    #[test]
    fn apply_dsp_params_to_settings_delay_picks_channel_zero_from_array() {
        let mut settings = PluginSettings::Delay {
            delay_ms: 0.0,
            feedback: 0.0,
            mix: 1.0,
            lfo_rate_hz: 0.0,
            lfo_depth_ms: 0.0,
            allpass_feedback: false,
            allpass_coeff: 0.5,
        };
        let params = serde_json::json!({
            "channel_delays_ms": [10.0, 25.0],
            "delay_ms": 0.0,
        });
        apply_dsp_params_to_settings(&mut settings, "delay", &params);
        match settings {
            PluginSettings::Delay { delay_ms, .. } => assert_eq!(delay_ms, 10.0),
            _ => panic!("expected Delay"),
        }
    }

    #[test]
    fn apply_dsp_params_to_settings_eq_populates_channel_filters() {
        let mut settings = PluginSettings::EQ {
            channels: 2,
            filters: Vec::new(),
            channel_filters: None,
            per_channel_mode: false,
            max_filters: 10,
            tdf2: false,
            topology: 0.0,
        };
        let params = serde_json::json!({
            "channel_filters": [
                [{"filter_type": "peak", "freq": 100.0, "q": 1.0, "db_gain": -3.0}],
                [{"filter_type": "peak", "freq": 200.0, "q": 2.0, "db_gain": 4.0}],
            ],
        });
        apply_dsp_params_to_settings(&mut settings, "eq", &params);
        match settings {
            PluginSettings::EQ {
                channel_filters,
                per_channel_mode,
                filters,
                ..
            } => {
                assert!(per_channel_mode, "per_channel_mode must be set");
                let per_ch = channel_filters.expect("channel_filters populated");
                assert_eq!(per_ch.len(), 2);
                assert_eq!(per_ch[0].len(), 1);
                assert_eq!(per_ch[1].len(), 1);
                // `filters` reflects channel 0 for legacy single-EQ UI paths.
                assert_eq!(filters.len(), 1);
            }
            _ => panic!("expected EQ"),
        }
    }

    #[test]
    fn apply_dsp_params_to_settings_falls_back_to_scalar_when_no_per_channel() {
        let mut settings = PluginSettings::Gain {
            channels: 2,
            gain_db: 0.0,
            smoothing_ms: 20.0,
        };
        apply_dsp_params_to_settings(&mut settings, "gain", &serde_json::json!({"gain_db": -7.5}));
        match settings {
            PluginSettings::Gain { gain_db, .. } => assert_eq!(gain_db, -7.5),
            _ => panic!("expected Gain"),
        }
    }

    // ========================================================================
    // End-to-end: build_ui_graph_from_config applies DSP params to UI graph.
    // ========================================================================

    #[test]
    fn build_ui_graph_from_config_applies_eq_filters_per_channel() {
        use sotf_audio::engine::{PluginGraphConfig, PluginGraphNodeConfig};

        let config = PluginGraphConfig {
            nodes: vec![PluginGraphNodeConfig {
                id: 0,
                plugin_type: "eq".to_string(),
                input_channels: 2,
                parameters: serde_json::json!({
                    "channel_filters": [
                        [{"filter_type": "peak", "freq": 100.0, "q": 1.5, "db_gain": -3.0}],
                        [{"filter_type": "peak", "freq": 200.0, "q": 2.0, "db_gain": 4.0}],
                    ],
                }),
            }],
            edges: vec![],
        };

        let graph = build_ui_graph_from_config(&config);
        let eq_node = graph
            .nodes
            .values()
            .find(|n| n.plugin.plugin_type().name() == "EQ")
            .expect("EQ node exists");

        match &eq_node.plugin.settings {
            PluginSettings::EQ {
                per_channel_mode,
                channel_filters,
                filters,
                ..
            } => {
                assert!(*per_channel_mode);
                let cf = channel_filters.as_ref().expect("channel filters set");
                assert_eq!(cf.len(), 2);
                assert!((cf[0][0].frequency - 100.0).abs() < 0.1);
                assert!((cf[1][0].frequency - 200.0).abs() < 0.1);
                assert_eq!(filters.len(), 1);
                assert!((filters[0].frequency - 100.0).abs() < 0.1);
            }
            _ => panic!("expected EQ settings"),
        }
    }

    #[test]
    fn build_ui_graph_from_config_applies_delay_per_channel_representative() {
        use sotf_audio::engine::{PluginGraphConfig, PluginGraphNodeConfig};

        let config = PluginGraphConfig {
            nodes: vec![PluginGraphNodeConfig {
                id: 0,
                plugin_type: "delay".to_string(),
                input_channels: 2,
                parameters: serde_json::json!({
                    "channel_delays_ms": [12.5, 25.0],
                    "delay_ms": 0.0,
                }),
            }],
            edges: vec![],
        };

        let graph = build_ui_graph_from_config(&config);
        let delay_node = graph
            .nodes
            .values()
            .find(|n| n.plugin.plugin_type().name() == "Delay")
            .expect("Delay node exists");

        match &delay_node.plugin.settings {
            PluginSettings::Delay { delay_ms, .. } => {
                assert!((delay_ms - 12.5).abs() < 0.01);
            }
            _ => panic!("expected Delay settings"),
        }
    }

    #[test]
    fn build_ui_graph_from_config_applies_gain_per_channel_representative() {
        use sotf_audio::engine::{PluginGraphConfig, PluginGraphNodeConfig};

        let config = PluginGraphConfig {
            nodes: vec![PluginGraphNodeConfig {
                id: 0,
                plugin_type: "gain".to_string(),
                input_channels: 2,
                parameters: serde_json::json!({
                    "channel_gains": [-6.0, -9.0],
                    "gain_db": 0.0,
                }),
            }],
            edges: vec![],
        };

        let graph = build_ui_graph_from_config(&config);
        let gain_node = graph
            .nodes
            .values()
            .find(|n| n.plugin.plugin_type().name() == "Gain")
            .expect("Gain node exists");

        match &gain_node.plugin.settings {
            PluginSettings::Gain { gain_db, .. } => {
                assert!((gain_db - -6.0).abs() < 0.01);
            }
            _ => panic!("expected Gain settings"),
        }
    }
}
