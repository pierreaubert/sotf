/// Estimate timeout duration based on plugin complexity
/// Complex plugins (SOFA loading, large convolutions) need more time
pub(super) fn estimate_update_timeout(
    plugins: &[super::super::PluginConfig],
) -> std::time::Duration {
    let mut timeout_ms: u64 = 200; // Base timeout for crossfade

    for plugin in plugins {
        timeout_ms += match plugin.plugin_type.as_str() {
            "convolution" => {
                // SOFA/IR loading can be very slow
                2000
            }
            "upmixer" => {
                // FFT setup and buffer allocation
                300
            }
            "aae" => {
                // FDN + early reflection setup
                200
            }
            "crossover" => {
                // Multiple filter banks
                200
            }
            "EQ" => {
                // Count number of filters if available
                if let Some(filters) = plugin.parameters.get("filters") {
                    if let Some(array) = filters.as_array() {
                        array.len() as u64 * 10 // ~10ms per filter
                    } else {
                        50
                    }
                } else {
                    50
                }
            }
            "resampler" => 150,
            "limiter" | "compressor" | "gate" => 100,
            "gain" | "matrix" => 20,
            _ => 50,
        };
    }

    // Cap at 10 seconds (for very complex chains)
    std::time::Duration::from_millis(timeout_ms.min(10000))
}

pub(super) fn estimate_graph_update_timeout(
    graph_config: &super::super::types::PluginGraphConfig,
) -> std::time::Duration {
    let plugins: Vec<_> = graph_config
        .nodes
        .iter()
        .map(|node| super::super::PluginConfig {
            plugin_type: node.plugin_type.clone(),
            parameters: node.parameters.clone(),
        })
        .collect();
    estimate_update_timeout(&plugins)
}
