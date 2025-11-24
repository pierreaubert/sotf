//! Output generation for room EQ DSP chains

use super::types::{ChannelDspChain, DspChainOutput, OptimizationMetadata, PluginConfigWrapper};
use autoeq_iir::Biquad;
use serde_json::json;
use std::collections::HashMap;
use std::error::Error;

/// Convert Biquad filter to JSON configuration
fn biquad_to_json(biquad: &Biquad) -> serde_json::Value {
    json!({
        "filter_type": biquad.filter_type.long_name().to_lowercase(),
        "freq": biquad.freq,
        "q": biquad.q,
        "db_gain": biquad.db_gain,
    })
}

/// Create a gain plugin configuration
pub fn create_gain_plugin(gain_db: f64) -> PluginConfigWrapper {
    PluginConfigWrapper {
        plugin_type: "gain".to_string(),
        parameters: json!({
            "gain_db": gain_db
        }),
    }
}

/// Create an EQ plugin configuration from Biquad filters
pub fn create_eq_plugin(filters: &[Biquad]) -> PluginConfigWrapper {
    let filter_configs: Vec<serde_json::Value> = filters.iter().map(biquad_to_json).collect();

    PluginConfigWrapper {
        plugin_type: "eq".to_string(),
        parameters: json!({
            "filters": filter_configs
        }),
    }
}

/// Create a crossover plugin configuration
#[allow(dead_code)]
pub fn create_crossover_plugin(
    crossover_type: &str,
    frequency: f64,
    output: &str, // "low" or "high"
) -> PluginConfigWrapper {
    PluginConfigWrapper {
        plugin_type: "crossover".to_string(),
        parameters: json!({
            "type": crossover_type,
            "frequency": frequency,
            "output": output
        }),
    }
}

/// Build a DSP chain for a single channel
pub fn build_channel_dsp_chain(
    channel_name: &str,
    gain_db: Option<f64>,
    crossovers: Vec<PluginConfigWrapper>,
    eq_filters: &[Biquad],
) -> ChannelDspChain {
    let mut plugins = Vec::new();

    // Add gain if specified
    if let Some(gain) = gain_db {
        if gain.abs() > 0.01 {
            // Only add if gain is non-zero
            plugins.push(create_gain_plugin(gain));
        }
    }

    // Add crossover filters
    plugins.extend(crossovers);

    // Add EQ
    if !eq_filters.is_empty() {
        plugins.push(create_eq_plugin(eq_filters));
    }

    ChannelDspChain {
        channel: channel_name.to_string(),
        plugins,
    }
}

/// Create complete DSP chain output
pub fn create_dsp_chain_output(
    channels: HashMap<String, ChannelDspChain>,
    metadata: Option<OptimizationMetadata>,
) -> DspChainOutput {
    DspChainOutput { channels, metadata }
}

/// Save DSP chain to JSON file
pub fn save_dsp_chain(
    output: &DspChainOutput,
    path: &std::path::Path,
) -> Result<(), Box<dyn Error>> {
    let json = serde_json::to_string_pretty(output)?;
    std::fs::write(path, json)?;
    Ok(())
}
