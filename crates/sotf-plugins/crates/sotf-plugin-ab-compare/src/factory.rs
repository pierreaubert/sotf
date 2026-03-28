//! Plugin factory and path builder for A/B Compare plugin.

use super::config::{GraphEdgeConfig, GraphNodeConfig, PathConfig};
use sotf_host::InPlacePluginAdapter;
use sotf_host::host::{DawHost, GraphEdge};
use sotf_host::plugin::Plugin;
use sotf_host::PluginFactoryFn;
use sotf_plugin_multiband_compressor::{MultibandCompressorPlugin, MultibandCompressorPluginParams};
use sotf_plugin_delay::{DelayPlugin, DelayPluginParams};
use sotf_plugin_eq::{EqPlugin, EqPluginParams};
use sotf_plugin_gain::{GainPlugin, GainPluginParams};
use sotf_plugin_gate::{GatePlugin, GatePluginParams};
use sotf_plugin_limiter::{LimiterPlugin, LimiterPluginParams};
use std::collections::HashMap;

/// Create a plugin, delegating to the external factory if provided,
/// falling back to the built-in limited factory.
fn create_plugin(
    plugin_type: &str,
    parameters: &serde_json::Value,
    num_channels: usize,
    sample_rate: u32,
    external_factory: Option<PluginFactoryFn>,
) -> Result<Box<dyn Plugin>, String> {
    if let Some(factory) = external_factory {
        return factory(plugin_type, parameters, num_channels, sample_rate);
    }
    // Fallback: built-in limited factory (6 types)
    create_plugin_builtin(plugin_type, parameters, num_channels, sample_rate)
}

/// Built-in factory supporting a minimal set of plugin types.
/// Used when no external factory is provided (e.g., in unit tests).
fn create_plugin_builtin(
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
            let params: MultibandCompressorPluginParams = serde_json::from_value(parameters.clone())
                .map_err(|e| format!("Invalid Compressor params: {}", e))?;
            let plugin = MultibandCompressorPlugin::from_params(num_channels, params);
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

/// Build a DawHost from a PathConfig, optionally using an external plugin factory.
pub fn build_path_from_config(
    config: &PathConfig,
    num_channels: usize,
    sample_rate: u32,
) -> Result<DawHost, String> {
    build_path_from_config_with_factory(config, num_channels, sample_rate, None)
}

/// Build a DawHost from a PathConfig with an explicit factory function.
pub fn build_path_from_config_with_factory(
    config: &PathConfig,
    num_channels: usize,
    sample_rate: u32,
    factory: Option<PluginFactoryFn>,
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
            let plugin = create_plugin(plugin_type, parameters, num_channels, sample_rate, factory)?;
            host.add_plugin(plugin)?;
        }
        PathConfig::Rack { plugins } => {
            for p in plugins {
                let plugin =
                    create_plugin(&p.plugin_type, &p.parameters, num_channels, sample_rate, factory)?;
                host.add_plugin(plugin)?;
            }
        }
        PathConfig::Graph { nodes, edges } => {
            build_graph(&mut host, nodes, edges, num_channels, sample_rate, factory)?;
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
    factory: Option<PluginFactoryFn>,
) -> Result<(), String> {
    let mut node_ids: HashMap<String, usize> = HashMap::new();

    for node in nodes {
        let plugin = create_plugin(
            &node.plugin_type,
            &node.parameters,
            num_channels,
            sample_rate,
            factory,
        )?;
        let id = host.add_node(node.id.clone(), plugin)?;
        node_ids.insert(node.id.clone(), id);
    }

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

    host.build()?;
    Ok(())
}
