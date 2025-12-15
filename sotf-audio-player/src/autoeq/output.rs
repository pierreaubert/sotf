//! Output generation for room EQ DSP chains
//!
//! Generates PluginConfig-compatible JSON output for the audio engine.

use autoeq_iir::Biquad;
use serde_json::json;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use super::types::{
    ChannelDspChain, ChannelOptimizationResult, CrossoverType, DriverDspChain, DspChainOutput,
    DspPluginConfig, EqFilterResult, OptimizationMetadata,
};

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
pub fn create_gain_plugin(gain_db: f64) -> DspPluginConfig {
    DspPluginConfig {
        plugin_type: "gain".to_string(),
        parameters: json!({
            "gain_db": gain_db
        }),
    }
}

/// Create an EQ plugin configuration from Biquad filters
pub fn create_eq_plugin(filters: &[Biquad]) -> DspPluginConfig {
    let filter_configs: Vec<serde_json::Value> = filters.iter().map(biquad_to_json).collect();

    DspPluginConfig {
        plugin_type: "eq".to_string(),
        parameters: json!({
            "filters": filter_configs
        }),
    }
}

/// Create an EQ plugin configuration from EqFilterResult
#[allow(dead_code)]
pub fn create_eq_plugin_from_results(filters: &[EqFilterResult]) -> DspPluginConfig {
    let filter_configs: Vec<serde_json::Value> = filters
        .iter()
        .map(|f| {
            json!({
                "filter_type": f.filter_type,
                "freq": f.frequency,
                "q": f.q,
                "db_gain": f.gain_db,
            })
        })
        .collect();

    DspPluginConfig {
        plugin_type: "eq".to_string(),
        parameters: json!({
            "filters": filter_configs
        }),
    }
}

/// Create a crossover plugin configuration
pub fn create_crossover_plugin(
    crossover_type: &CrossoverType,
    frequency: f64,
    output: &str, // "low" or "high"
) -> DspPluginConfig {
    DspPluginConfig {
        plugin_type: "crossover".to_string(),
        parameters: json!({
            "type": crossover_type.to_plugin_string(),
            "frequency": frequency,
            "output": output
        }),
    }
}

/// Get a descriptive name for a driver based on its index and total count
fn get_driver_name(index: usize, n_drivers: usize) -> String {
    match (n_drivers, index) {
        (2, 0) => "woofer",
        (2, 1) => "tweeter",
        (3, 0) => "woofer",
        (3, 1) => "midrange",
        (3, 2) => "tweeter",
        (4, 0) => "woofer",
        (4, 1) => "lower_midrange",
        (4, 2) => "upper_midrange",
        (4, 3) => "tweeter",
        _ => return format!("driver_{}", index),
    }
    .to_string()
}

/// Build a DSP chain for a single-driver channel
pub fn build_single_channel_dsp_chain(
    channel_name: &str,
    result: &ChannelOptimizationResult,
) -> ChannelDspChain {
    let mut plugins = Vec::new();

    // Add EQ if we have filters
    if !result.biquads.is_empty() {
        plugins.push(create_eq_plugin(&result.biquads));
    }

    ChannelDspChain {
        channel: channel_name.to_string(),
        plugins,
        drivers: None,
    }
}

/// Build a DSP chain for a multi-driver channel
pub fn build_multidriver_channel_dsp_chain(
    channel_name: &str,
    result: &ChannelOptimizationResult,
    crossover_type: &CrossoverType,
) -> ChannelDspChain {
    let gains = result.driver_gains.as_deref().unwrap_or(&[]);
    let xover_freqs = result.crossover_freqs.as_deref().unwrap_or(&[]);
    let n_drivers = gains.len();

    // Build per-driver chains
    let mut driver_chains = Vec::new();

    for i in 0..n_drivers {
        let mut driver_plugins = Vec::new();

        // Add gain plugin if non-zero
        if gains[i].abs() > 0.01 {
            driver_plugins.push(create_gain_plugin(gains[i]));
        }

        // Add highpass crossover from previous driver (if not first driver)
        if i > 0 && i - 1 < xover_freqs.len() {
            let crossover_freq = xover_freqs[i - 1];
            driver_plugins.push(create_crossover_plugin(
                crossover_type,
                crossover_freq,
                "high",
            ));
        }

        // Add lowpass crossover to next driver (if not last driver)
        if i < n_drivers - 1 && i < xover_freqs.len() {
            let crossover_freq = xover_freqs[i];
            driver_plugins.push(create_crossover_plugin(
                crossover_type,
                crossover_freq,
                "low",
            ));
        }

        driver_chains.push(DriverDspChain {
            name: get_driver_name(i, n_drivers),
            index: i,
            plugins: driver_plugins,
        });
    }

    // Build combined EQ (applied to summed output)
    let mut combined_plugins = Vec::new();
    if !result.biquads.is_empty() {
        combined_plugins.push(create_eq_plugin(&result.biquads));
    }

    ChannelDspChain {
        channel: channel_name.to_string(),
        plugins: combined_plugins,
        drivers: Some(driver_chains),
    }
}

/// Build complete DSP chain output from optimization results
pub fn build_dsp_chain_output(
    results: &HashMap<String, ChannelOptimizationResult>,
    crossover_types: &HashMap<String, CrossoverType>,
    algorithm: &str,
    iterations: usize,
) -> DspChainOutput {
    let mut channels = HashMap::new();
    let mut pre_scores = Vec::new();
    let mut post_scores = Vec::new();

    for (channel_name, result) in results {
        let chain = if result.driver_gains.is_some() {
            // Multi-driver
            let crossover_type = crossover_types
                .get(channel_name)
                .copied()
                .unwrap_or_default();
            build_multidriver_channel_dsp_chain(channel_name, result, &crossover_type)
        } else {
            // Single driver
            build_single_channel_dsp_chain(channel_name, result)
        };

        channels.insert(channel_name.clone(), chain);
        pre_scores.push(result.pre_score);
        post_scores.push(result.post_score);
    }

    // Compute average scores
    let avg_pre = if !pre_scores.is_empty() {
        pre_scores.iter().sum::<f64>() / pre_scores.len() as f64
    } else {
        0.0
    };
    let avg_post = if !post_scores.is_empty() {
        post_scores.iter().sum::<f64>() / post_scores.len() as f64
    } else {
        0.0
    };

    DspChainOutput {
        channels,
        metadata: Some(OptimizationMetadata {
            pre_score: avg_pre,
            post_score: avg_post,
            algorithm: algorithm.to_string(),
            iterations,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }),
    }
}

/// Save DSP chain output to JSON file
pub fn save_dsp_chain(output: &DspChainOutput, path: &Path) -> Result<(), Box<dyn Error>> {
    let json = serde_json::to_string_pretty(output)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Load DSP chain output from JSON file
pub fn load_dsp_chain(path: &Path) -> Result<DspChainOutput, Box<dyn Error>> {
    let json = std::fs::read_to_string(path)?;
    let output: DspChainOutput = serde_json::from_str(&json)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_driver_name() {
        assert_eq!(get_driver_name(0, 2), "woofer");
        assert_eq!(get_driver_name(1, 2), "tweeter");
        assert_eq!(get_driver_name(1, 3), "midrange");
        assert_eq!(get_driver_name(5, 6), "driver_5");
    }

    #[test]
    fn test_create_gain_plugin() {
        let plugin = create_gain_plugin(3.5);
        assert_eq!(plugin.plugin_type, "gain");
        assert_eq!(plugin.parameters["gain_db"], 3.5);
    }

    #[test]
    fn test_create_crossover_plugin() {
        let plugin = create_crossover_plugin(&CrossoverType::LR24, 2000.0, "low");
        assert_eq!(plugin.plugin_type, "crossover");
        assert_eq!(plugin.parameters["type"], "LR24");
        assert_eq!(plugin.parameters["frequency"], 2000.0);
        assert_eq!(plugin.parameters["output"], "low");
    }
}
