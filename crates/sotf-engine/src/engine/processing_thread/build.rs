use super::super::PluginConfig;
use super::misc::configure_host_oversampling;
use super::misc::create_plugin;
use sotf_plugins::PluginHost;
use sotf_types::EngineOversamplingPolicy;

/// Build a plugin host from configs.
///
/// Plugins that fail to create or have channel mismatches are skipped rather
/// than aborting the entire chain. The second element of the returned tuple
/// contains warnings about skipped plugins.
pub fn build_plugin_host(
    configs: &[PluginConfig],
    sample_rate: u32,
    channels: usize,
) -> Result<(PluginHost, Vec<String>), String> {
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
) -> Result<(PluginHost, Vec<String>), String> {
    let mut host = PluginHost::new(channels, sample_rate);
    configure_host_oversampling(&mut host, oversampling_policy)?;
    let mut current_channels = channels;
    let mut warnings: Vec<String> = Vec::new();

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
                    warnings.push(msg);
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

                host.add_plugin(plugin)?;
            }
            Err(e) => {
                let msg = format!("Plugin '{}' skipped: {}", config.plugin_type, e);
                log::warn!("[Processing Thread] {}", msg);
                warnings.push(msg);
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

/// Build a plugin host from a graph config (DAG topology).
///
/// Unlike `build_plugin_host` which chains plugins linearly, this uses
/// `DawHost::add_node()` + `add_edge()` to create arbitrary graph topologies
/// needed for multi-driver crossover setups.
///
/// Nodes that fail to create are skipped, and edges referencing them are dropped.
#[allow(dead_code)]
pub fn build_plugin_graph_host(
    config: &super::super::types::PluginGraphConfig,
    sample_rate: u32,
    channels: usize,
) -> Result<(PluginHost, Vec<String>), String> {
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
) -> Result<(PluginHost, Vec<String>), String> {
    use sotf_plugins::GraphEdge;
    use std::collections::HashMap;

    let mut host = PluginHost::new(channels, sample_rate);
    configure_host_oversampling(&mut host, oversampling_policy)?;
    let mut id_map: HashMap<usize, usize> = HashMap::new();
    let mut warnings: Vec<String> = Vec::new();

    for node_config in &config.nodes {
        match create_plugin(
            &node_config.plugin_type,
            &node_config.parameters,
            node_config.input_channels,
            sample_rate,
        ) {
            Ok(plugin) => {
                let host_id = host.add_node(format!("node_{}", node_config.id), plugin)?;
                id_map.insert(node_config.id, host_id);
            }
            Err(e) => {
                let msg = format!(
                    "Graph node {} ('{}') skipped: {}",
                    node_config.id, node_config.plugin_type, e
                );
                log::warn!("[Processing Thread] {}", msg);
                warnings.push(msg);
            }
        }
    }

    for edge in &config.edges {
        let from = match id_map.get(&edge.from_node) {
            Some(&id) => id,
            None => {
                let msg = format!(
                    "Edge {}->{} skipped: from_node {} was not loaded",
                    edge.from_node, edge.to_node, edge.from_node
                );
                log::warn!("[Processing Thread] {}", msg);
                warnings.push(msg);
                continue;
            }
        };
        let to = match id_map.get(&edge.to_node) {
            Some(&id) => id,
            None => {
                let msg = format!(
                    "Edge {}->{} skipped: to_node {} was not loaded",
                    edge.from_node, edge.to_node, edge.to_node
                );
                log::warn!("[Processing Thread] {}", msg);
                warnings.push(msg);
                continue;
            }
        };
        host.add_edge(GraphEdge::new(from, to))?;
    }

    host.build()?;

    log::info!(
        "[Processing Thread] Plugin graph loaded: {} nodes, {} edges ({}ch -> {}ch), {} warnings",
        id_map.len(),
        config.edges.len(),
        channels,
        host.output_channels(),
        warnings.len()
    );

    Ok((host, warnings))
}
