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

use crate::plugin_graph::{NodePosition, PluginGraph, SpecialNodeType};
use crate::room_eq_types::{
    ChannelDspChain, DspChainOutput, parse_eq_filters_from_json,
};
use sotf_audio::engine::PluginGraphConfig;
use sotf_audio::plugins::{PluginSettings, PluginType};

use crate::EQFilter;

/// Outcome of [`apply_room_eq_rack_to_chain`] — useful for log lines or
/// status messages in the UI.
#[derive(Debug, Clone, Copy)]
pub struct RackApplyOutcome {
    /// Number of output channels the EQ plugins were configured for.
    pub num_channels: usize,
    /// Total number of main-room-correction filters (sum across channels).
    pub total_filters: usize,
    /// Total number of broadband pre-correction filters (sum across channels).
    pub total_broadband: usize,
}

/// Outcome of [`apply_room_eq_graph_to_chain`].
///
/// The `config` field is the engine-bound [`PluginGraphConfig`] the caller
/// must pass to `Player::update_plugin_graph(config)`. The UI graph is
/// already mutated in-place on the `PluginGraph` reference passed in.
#[derive(Debug, Clone)]
pub struct GraphApplyOutcome {
    pub config: PluginGraphConfig,
    pub num_nodes: usize,
    pub num_edges: usize,
}

/// Split a channel's DSP plugins into main-room-EQ filters and broadband
/// pre-correction filters based on the `parameters.label` tag each plugin
/// carries.
///
/// The optimizer emits multiple EQ plugins per channel — main room
/// correction is **unlabeled**, broadband pre-correction is labeled
/// `"broadband"` (see `autoeq::roomeq::spectral_align::create_alignment_plugins`),
/// and other stages (`cea2034`, `user_preference`, `channel_matching`) are
/// not user-editable and are filtered out.
pub fn classify_channel_eq_filters(channel_dsp: &ChannelDspChain) -> (Vec<EQFilter>, Vec<EQFilter>) {
    let mut main_filters: Vec<EQFilter> = Vec::new();
    let mut bb_filters: Vec<EQFilter> = Vec::new();

    for plugin in &channel_dsp.plugins {
        if !plugin.plugin_type.eq_ignore_ascii_case("eq") {
            continue;
        }
        let Some(filters) = plugin.parameters.get("filters").and_then(|f| f.as_array()) else {
            continue;
        };
        let label = plugin.parameters.get("label").and_then(|l| l.as_str());
        match label {
            Some("broadband") => bb_filters.extend(parse_eq_filters_from_json(filters)),
            None => main_filters.extend(parse_eq_filters_from_json(filters)),
            _ => {} // Skip cea2034, user_preference, channel_matching, …
        }
    }

    (main_filters, bb_filters)
}

/// Linear index of the first **user** EQ plugin with no custom name.
///
/// A user who ran "Apply to Rack" in an older build will have an anonymous
/// EQ sitting in the chain. Re-running Apply in the current build needs to
/// reclaim that node as "Room EQ" instead of inserting a third EQ alongside
/// it — otherwise the rack accumulates stale plugins across runs.
fn unnamed_user_eq_index(graph: &PluginGraph) -> Option<usize> {
    use crate::plugin_graph::NodeRole;
    graph.plugins_linear()?.iter().position(|n| {
        matches!(n.plugin.plugin_type(), PluginType::EQ)
            && n.plugin.name.as_deref().is_none_or(str::is_empty)
            && !n.plugin.permanent
            && n.role == NodeRole::User
    })
}

/// Upsert the two named EQ plugins ("Broadband EQ" + "Room EQ") into the
/// plugin graph.
///
/// Behavior contract (regression-tested in `room_eq_apply_tests.rs`):
///
/// - When `total_bb > 0`, produces **two** named EQ plugins.
///   Main is "Room EQ" with `max_filters=10`; broadband is "Broadband EQ"
///   with `max_filters=4`. Both run in per-channel mode.
/// - Pre-existing unnamed user EQ plugins (e.g. from an older
///   Apply-to-Rack build) are adopted in-place as "Room EQ" so the rack
///   does not accumulate stale nodes on upgrade.
/// - Second Apply with same names is idempotent: the existing named EQ
///   is updated in place rather than duplicated.
pub fn upsert_named_room_eq_plugins(
    graph: &mut PluginGraph,
    num_channels: usize,
    global_bb: &[EQFilter],
    per_channel_broadband: &[Vec<EQFilter>],
    total_bb: usize,
    global_filters: &[EQFilter],
    per_channel_filters: &[Vec<EQFilter>],
) {
    // Step 1: migrate stale unnamed EQ (pre-release upgrade path).
    if let Some(existing_idx) = unnamed_user_eq_index(graph)
        && let Some(p) = graph.get_plugin_mut(existing_idx)
    {
        p.name = Some("Room EQ".to_string());
        log::info!(
            "Adopted pre-existing unnamed EQ at index {} as 'Room EQ'",
            existing_idx
        );
    }

    // Step 2: name-keyed upsert helper. Tracks new nodes by stable
    // GraphNodeId so inserts that shift sibling positions don't leave us
    // writing settings into a neighbouring plugin.
    let upsert_eq = |graph: &mut PluginGraph, settings: PluginSettings, name: &str| {
        if let Some(idx) = graph.find_plugin_index_by_name(name) {
            if let Some(p) = graph.get_plugin_mut(idx) {
                p.settings = settings;
                p.name = Some(name.to_string());
                log::info!("Updated existing '{}' EQ at index {}", name, idx);
            }
            return;
        }

        let insert_idx = graph.user_plugin_insert_index();
        match graph.insert_plugin(insert_idx, &PluginType::EQ) {
            Ok(node_id) => {
                if let Some(node) = graph.nodes.get_mut(&node_id) {
                    node.plugin.settings = settings;
                    node.plugin.name = Some(name.to_string());
                }
                log::info!(
                    "Inserted '{}' EQ at linear index {} (node {:?})",
                    name,
                    insert_idx,
                    node_id
                );
            }
            Err(e) => {
                log::error!("Failed to insert '{}' EQ: {}", name, e);
            }
        }
    };

    // Step 3: broadband correction EQ (first in chain)
    if total_bb > 0 {
        let bb_settings = PluginSettings::EQ {
            channels: num_channels,
            filters: global_bb.to_vec(),
            channel_filters: Some(per_channel_broadband.to_vec()),
            per_channel_mode: true,
            max_filters: 4,
            tdf2: false,
            topology: 0.0,
        };
        upsert_eq(graph, bb_settings, "Broadband EQ");
    }

    // Step 4: main room correction EQ (after broadband)
    let main_settings = PluginSettings::EQ {
        channels: num_channels,
        filters: global_filters.to_vec(),
        channel_filters: Some(per_channel_filters.to_vec()),
        per_channel_mode: true,
        max_filters: 10,
        tdf2: false,
        topology: 0.0,
    };
    upsert_eq(graph, main_settings, "Room EQ");

    // Step 5: post-condition sanity log.
    let named_eq_count = graph
        .plugins()
        .iter()
        .filter(|p| {
            matches!(p.plugin_type(), PluginType::EQ)
                && p.name
                    .as_deref()
                    .is_some_and(|n| n == "Room EQ" || n == "Broadband EQ")
        })
        .count();
    log::info!(
        "After upsert: {} named room-EQ plugins in graph (expected {}, total EQs {})",
        named_eq_count,
        if total_bb > 0 { 2 } else { 1 },
        graph
            .plugins()
            .iter()
            .filter(|p| matches!(p.plugin_type(), PluginType::EQ))
            .count()
    );
}

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
            let (channel_eq_filters, channel_bb_filters) =
                classify_channel_eq_filters(channel_dsp);
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

/// Derive a user-facing plugin name from the DSP params the optimizer emits.
///
/// Today the only plugin type that carries a semantic label is `EQ`
/// (`"broadband"` for the pre-correction EQ, unlabeled for the main room
/// correction). Returning `None` lets the UI fall back to the generic
/// plugin type display name.
fn derive_plugin_name(plugin_type_str: &str, parameters: &serde_json::Value) -> Option<String> {
    if !plugin_type_str.eq_ignore_ascii_case("eq") {
        return None;
    }
    match parameters.get("label").and_then(|l| l.as_str()) {
        Some("broadband") => Some("Broadband EQ".to_string()),
        Some("cea2034") => Some("Speaker EQ".to_string()),
        Some("user_preference") => Some("Preference EQ".to_string()),
        Some(other) if !other.is_empty() => Some(other.to_string()),
        // Unlabeled = main room correction EQ
        _ => Some("Room EQ".to_string()),
    }
}

/// Apply DSP output parameters to a `PluginSettings` in-place.
///
/// Handles the common plugin types from roomeq: EQ (filters), Gain (gain_db),
/// and Delay (delay_ms). Unknown types are left at their defaults.
fn apply_dsp_params_to_settings(
    settings: &mut PluginSettings,
    plugin_type_str: &str,
    parameters: &serde_json::Value,
) {
    let lower = plugin_type_str.to_lowercase();
    match lower.as_str() {
        "eq" => {
            if let PluginSettings::EQ { filters, .. } = settings
                && let Some(filter_arr) = parameters.get("filters").and_then(|v| v.as_array())
            {
                *filters = parse_eq_filters_from_json(filter_arr);
            }
        }
        "gain" => {
            if let PluginSettings::Gain { gain_db, .. } = settings
                && let Some(v) = parameters.get("gain_db").and_then(|v| v.as_f64())
            {
                *gain_db = v;
            }
        }
        "delay" => {
            if let PluginSettings::Delay { delay_ms, .. } = settings
                && let Some(v) = parameters.get("delay_ms").and_then(|v| v.as_f64())
            {
                *delay_ms = v;
            }
        }
        _ => {} // Other types keep defaults
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
    let mut node_depth: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();
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

        let plugin_type =
            PluginType::from_name(&node_config.plugin_type).unwrap_or(PluginType::EQ);

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
        if let (Some(&from), Some(&to)) =
            (id_map.get(&edge.from_node), id_map.get(&edge.to_node))
        {
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
        let outcome =
            apply_room_eq_rack_to_chain(&mut graph, &dsp, &names);
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
}
