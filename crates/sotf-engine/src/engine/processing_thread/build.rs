use super::super::PluginConfig;
use super::misc::configure_host_oversampling;
use super::misc::create_plugin;
use sotf_plugins::{EXTERNAL_PLUGIN_INSTANCE_ID_PARAMETER, PluginHost};
use sotf_types::{EngineOversamplingPolicy, PluginBuildDiagnostic};

fn plugin_instance_id(parameters: &serde_json::Value) -> Option<usize> {
    parameters
        .get(EXTERNAL_PLUGIN_INSTANCE_ID_PARAMETER)
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

/// Build a plugin host from configs.
///
/// Plugins that fail to create or have channel mismatches are skipped rather
/// than aborting the entire chain. The second element of the returned tuple
/// contains warnings about skipped plugins.
pub fn build_plugin_host(
    configs: &[PluginConfig],
    sample_rate: u32,
    channels: usize,
) -> Result<(PluginHost, Vec<PluginBuildDiagnostic>), PluginBuildDiagnostic> {
    build_plugin_host_with_policy(
        configs,
        sample_rate,
        channels,
        EngineOversamplingPolicy::PluginPreferred,
    )
}

pub fn build_plugin_host_with_policy(
    configs: &[PluginConfig],
    sample_rate: u32,
    channels: usize,
    oversampling_policy: EngineOversamplingPolicy,
) -> Result<(PluginHost, Vec<PluginBuildDiagnostic>), PluginBuildDiagnostic> {
    let mut host = PluginHost::new(channels, sample_rate);
    configure_host_oversampling(&mut host, oversampling_policy)
        .map_err(PluginBuildDiagnostic::host)?;
    let mut current_channels = channels;
    let mut warnings: Vec<PluginBuildDiagnostic> = Vec::new();

    for (i, config) in configs.iter().enumerate() {
        log::info!(
            "[Processing Thread] Loading plugin {}: {}",
            i,
            config.plugin_type
        );

        match create_plugin(
            &config.plugin_type,
            &config.parameters,
            current_channels,
            sample_rate,
        ) {
            Ok(plugin) => {
                // Check channel compatibility
                if plugin.input_channels() != current_channels {
                    let msg = format!(
                        "Plugin '{}' skipped: expects {} input channels, but chain provides {}",
                        config.plugin_type,
                        plugin.input_channels(),
                        current_channels
                    );
                    log::warn!("[Processing Thread] {}", msg);
                    warnings.push(PluginBuildDiagnostic::chain_plugin(
                        i,
                        plugin_instance_id(&config.parameters),
                        &config.plugin_type,
                        msg,
                    ));
                    continue;
                }

                // Update current channel count for next plugin
                current_channels = plugin.output_channels();

                log::info!(
                    "[Processing Thread] Plugin '{}' loaded: {}ch -> {}ch",
                    config.plugin_type,
                    plugin.input_channels(),
                    plugin.output_channels()
                );

                host.add_plugin(plugin).map_err(|error| {
                    PluginBuildDiagnostic::chain_plugin(
                        i,
                        plugin_instance_id(&config.parameters),
                        &config.plugin_type,
                        format!(
                            "Plugin '{}' could not be added: {error}",
                            config.plugin_type
                        ),
                    )
                })?;
            }
            Err(e) => {
                let msg = format!("Plugin '{}' skipped: {}", config.plugin_type, e);
                log::warn!("[Processing Thread] {}", msg);
                warnings.push(PluginBuildDiagnostic::chain_plugin(
                    i,
                    plugin_instance_id(&config.parameters),
                    &config.plugin_type,
                    msg,
                ));
            }
        }
    }

    log::info!(
        "[Processing Thread] Plugin chain loaded: {} plugins ({}ch -> {}ch), {} skipped",
        configs.len() - warnings.len(),
        channels,
        host.output_channels(),
        warnings.len()
    );

    Ok((host, warnings))
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::build_plugin_graph_host;
    use crate::engine::{PluginGraphConfig, PluginGraphNodeConfig};

    #[test]
    fn serialized_graph_bypass_reaches_plugin_host() {
        let mut node = PluginGraphNodeConfig::try_new(7, "gain", serde_json::json!({}), 2).unwrap();
        node.bypassed = true;
        let graph = PluginGraphConfig::try_new(vec![node], vec![]).unwrap();

        let (host, warnings) = build_plugin_graph_host(&graph, 48_000, 2).unwrap();

        assert!(warnings.is_empty());
        assert!(host.is_node_bypassed(0).unwrap());
    }
}

/// Build a plugin host from a graph config (DAG topology).
///
/// Unlike `build_plugin_host` which chains plugins linearly, this uses
/// `DawHost::add_node()` + `add_edge()` to create arbitrary graph topologies
/// needed for multi-driver crossover setups.
///
/// Graph construction is atomic: every declared node and edge must load before
/// a host is returned. Skipping a failed node would silently change topology
/// and can leave an apparently built but disconnected processing DAG.
#[allow(dead_code)]
pub fn build_plugin_graph_host(
    config: &super::super::types::PluginGraphConfig,
    sample_rate: u32,
    channels: usize,
) -> Result<(PluginHost, Vec<PluginBuildDiagnostic>), PluginBuildDiagnostic> {
    build_plugin_graph_host_with_policy(
        config,
        sample_rate,
        channels,
        EngineOversamplingPolicy::PluginPreferred,
    )
}

pub fn build_plugin_graph_host_with_policy(
    config: &super::super::types::PluginGraphConfig,
    sample_rate: u32,
    channels: usize,
    oversampling_policy: EngineOversamplingPolicy,
) -> Result<(PluginHost, Vec<PluginBuildDiagnostic>), PluginBuildDiagnostic> {
    use sotf_plugins::GraphEdge;
    use std::collections::HashMap;

    config.validate().map_err(|error| {
        PluginBuildDiagnostic::host(format!("Invalid plugin graph configuration: {error}"))
    })?;

    let mut host = PluginHost::new(channels, sample_rate);
    configure_host_oversampling(&mut host, oversampling_policy)
        .map_err(PluginBuildDiagnostic::host)?;
    let mut id_map: HashMap<usize, usize> = HashMap::new();

    for node_config in &config.nodes {
        let plugin = create_plugin(
            &node_config.plugin_type,
            &node_config.parameters,
            node_config.input_channels,
            sample_rate,
        )
        .map_err(|error| {
            PluginBuildDiagnostic::graph_node(
                node_config.id,
                plugin_instance_id(&node_config.parameters),
                &node_config.plugin_type,
                format!(
                    "Plugin graph node {} ('{}') failed to load: {error}",
                    node_config.id, node_config.plugin_type
                ),
            )
        })?;
        let host_id = host
            .add_node(format!("node_{}", node_config.id), plugin)
            .map_err(|error| {
                PluginBuildDiagnostic::graph_node(
                    node_config.id,
                    plugin_instance_id(&node_config.parameters),
                    &node_config.plugin_type,
                    format!(
                        "Plugin graph node {} ('{}') could not be added: {error}",
                        node_config.id, node_config.plugin_type
                    ),
                )
            })?;
        if node_config.bypassed {
            host.bypass_node(host_id).map_err(|error| {
                PluginBuildDiagnostic::graph_node(
                    node_config.id,
                    plugin_instance_id(&node_config.parameters),
                    &node_config.plugin_type,
                    format!(
                        "Plugin graph node {} ('{}') could not be bypassed: {error}",
                        node_config.id, node_config.plugin_type
                    ),
                )
            })?;
        }
        id_map.insert(node_config.id, host_id);
    }

    for edge in &config.edges {
        let from = *id_map.get(&edge.from_node).ok_or_else(|| {
            PluginBuildDiagnostic::graph_edge(
                edge.from_node,
                edge.to_node,
                format!(
                    "Plugin graph edge {} -> {} references unloaded from_node {}",
                    edge.from_node, edge.to_node, edge.from_node
                ),
            )
        })?;
        let to = *id_map.get(&edge.to_node).ok_or_else(|| {
            PluginBuildDiagnostic::graph_edge(
                edge.from_node,
                edge.to_node,
                format!(
                    "Plugin graph edge {} -> {} references unloaded to_node {}",
                    edge.from_node, edge.to_node, edge.to_node
                ),
            )
        })?;
        host.add_edge(GraphEdge::new(from, to)).map_err(|error| {
            PluginBuildDiagnostic::graph_edge(
                edge.from_node,
                edge.to_node,
                format!(
                    "Plugin graph edge {} -> {} is invalid: {error}",
                    edge.from_node, edge.to_node
                ),
            )
        })?;
    }

    host.build().map_err(|error| {
        PluginBuildDiagnostic::host(format!("Plugin graph host build failed: {error}"))
    })?;

    log::info!(
        "[Processing Thread] Plugin graph loaded: {} nodes, {} edges ({}ch -> {}ch), {} warnings",
        id_map.len(),
        config.edges.len(),
        channels,
        host.output_channels(),
        0
    );

    Ok((host, Vec::new()))
}
