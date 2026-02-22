//! Plugin factory and path builder for A/B Compare plugin.

use super::config::{GraphEdgeConfig, GraphNodeConfig, PathConfig};
use crate::host::{DawHost, GraphEdge};
use crate::plugin::Plugin;
use crate::{
    CompressorPlugin, CompressorPluginParams, DelayPlugin, DelayPluginParams, EqPlugin,
    EqPluginParams, GainPlugin, GainPluginParams, GatePlugin, GatePluginParams,
    InPlacePluginAdapter, LimiterPlugin, LimiterPluginParams,
};
use std::collections::HashMap;

/// Create a plugin from type name and parameters
pub fn create_plugin(
    plugin_type: &str,
    parameters: &serde_json::Value,
    num_channels: usize,
    sample_rate: u32,
) -> Result<Box<dyn Plugin>, String> {
    match plugin_type.to_lowercase().as_str() {
        "eq" => {
            let params: EqPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Invalid EQ params: {}", e))?;
            let plugin = EqPlugin::from_params(num_channels, sample_rate, params)?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }
        "gain" => {
            let params: GainPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Invalid Gain params: {}", e))?;
            let plugin = GainPlugin::from_params(num_channels, params)?;
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }
        "compressor" => {
            let params: CompressorPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Invalid Compressor params: {}", e))?;
            let plugin = CompressorPlugin::from_params(num_channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }
        "limiter" => {
            let params: LimiterPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Invalid Limiter params: {}", e))?;
            let plugin = LimiterPlugin::from_params(num_channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }
        "gate" => {
            let params: GatePluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Invalid Gate params: {}", e))?;
            let plugin = GatePlugin::from_params(num_channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }
        "delay" => {
            let params: DelayPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Invalid Delay params: {}", e))?;
            let plugin = DelayPlugin::from_params(num_channels, params);
            Ok(Box::new(InPlacePluginAdapter::new(plugin)))
        }
        _ => Err(format!("Unknown plugin type: {}", plugin_type)),
    }
}

/// Build a DawHost from a PathConfig
pub fn build_path_from_config(
    config: &PathConfig,
    num_channels: usize,
    sample_rate: u32,
) -> Result<DawHost, String> {
    let mut host = DawHost::new(num_channels, sample_rate);

    match config {
        PathConfig::None => {
            // Empty host = pass-through
        }
        PathConfig::Plugin {
            plugin_type,
            parameters,
        } => {
            let plugin = create_plugin(plugin_type, parameters, num_channels, sample_rate)?;
            host.add_plugin(plugin)?;
        }
        PathConfig::Rack { plugins } => {
            for p in plugins {
                let plugin =
                    create_plugin(&p.plugin_type, &p.parameters, num_channels, sample_rate)?;
                host.add_plugin(plugin)?;
            }
        }
        PathConfig::Graph { nodes, edges } => {
            build_graph(&mut host, nodes, edges, num_channels, sample_rate)?;
        }
    }

    Ok(host)
}

fn build_graph(
    host: &mut DawHost,
    nodes: &[GraphNodeConfig],
    edges: &[GraphEdgeConfig],
    num_channels: usize,
    sample_rate: u32,
) -> Result<(), String> {
    let mut node_ids: HashMap<String, usize> = HashMap::new();

    // Add all nodes
    for node in nodes {
        let plugin = create_plugin(
            &node.plugin_type,
            &node.parameters,
            num_channels,
            sample_rate,
        )?;
        let id = host.add_node(node.id.clone(), plugin)?;
        node_ids.insert(node.id.clone(), id);
    }

    // Add all edges
    for edge in edges {
        let from_id = *node_ids
            .get(&edge.from)
            .ok_or_else(|| format!("Unknown node id in edge: {}", edge.from))?;
        let to_id = *node_ids
            .get(&edge.to)
            .ok_or_else(|| format!("Unknown node id in edge: {}", edge.to))?;

        let graph_edge = match &edge.channel_map {
            Some(map) => GraphEdge::with_channels(from_id, to_id, map.clone()),
            None => GraphEdge::new(from_id, to_id),
        };
        host.add_edge(graph_edge)?;
    }

    // Build the graph
    host.build()?;

    Ok(())
}
