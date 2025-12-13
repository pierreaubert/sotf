//! Output generation for room EQ DSP chains

use super::types::{
    ChannelDspChain, DriverDspChain, DspChainOutput, OptimizationMetadata, PluginConfigWrapper,
};
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

/// Create a gain plugin configuration with polarity inversion
pub fn create_gain_plugin_with_invert(gain_db: f64, invert: bool) -> PluginConfigWrapper {
    PluginConfigWrapper {
        plugin_type: "gain".to_string(),
        parameters: json!({
            "gain_db": gain_db,
            "invert": invert
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

/// Build a DSP chain for a single channel
pub fn build_channel_dsp_chain(
    channel_name: &str,
    gain_db: Option<f64>,
    crossovers: Vec<PluginConfigWrapper>,
    eq_filters: &[Biquad],
) -> ChannelDspChain {
    let mut plugins = Vec::new();

    // Add gain if specified
    if let Some(gain) = gain_db
        && gain.abs() > 0.01
    {
        // Only add if gain is non-zero
        plugins.push(create_gain_plugin(gain));
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
        drivers: None, // Single speakers don't have per-driver chains
    }
}

/// Create a delay plugin configuration
pub fn create_delay_plugin(delay_ms: f64) -> PluginConfigWrapper {
    PluginConfigWrapper {
        plugin_type: "delay".to_string(),
        parameters: json!({
            "delay_ms": delay_ms
        }),
    }
}

/// Create a convolution plugin configuration
pub fn create_convolution_plugin(wav_path: &str) -> PluginConfigWrapper {
    PluginConfigWrapper {
        plugin_type: "convolution".to_string(),
        parameters: json!({
            "ir_file": wav_path
        }),
    }
}

/// Build a DSP chain for a multi-driver speaker with active crossover
///
/// # Arguments
/// * `channel_name` - Channel name (e.g., "left")
/// * `gains` - Per-driver gains in dB (one per driver)
/// * `delays` - Per-driver delays in ms (one per driver)
/// * `crossover_freqs` - Crossover frequencies in Hz (n_drivers - 1 values)
/// * `crossover_type` - Crossover type string (e.g., "LR24", "Butterworth12")
/// * `eq_filters` - EQ filters for the combined response
///
/// # Returns
/// * ChannelDspChain with per-driver chains and combined EQ
pub fn build_multidriver_dsp_chain(
    channel_name: &str,
    gains: &[f64],
    delays: &[f64],
    crossover_freqs: &[f64],
    crossover_type: &str,
    eq_filters: &[Biquad],
) -> ChannelDspChain {
    let n_drivers = gains.len();

    // Build per-driver chains
    let mut driver_chains = Vec::new();

    for i in 0..n_drivers {
        let mut driver_plugins = Vec::new();

        // Add gain plugin if non-zero
        if gains[i].abs() > 0.01 {
            driver_plugins.push(create_gain_plugin(gains[i]));
        }

        // Add delay plugin if non-zero
        if i < delays.len() && delays[i].abs() > 0.001 {
            driver_plugins.push(create_delay_plugin(delays[i]));
        }

        // Add highpass crossover from previous driver (if not first driver)
        if i > 0 {
            let crossover_freq = crossover_freqs[i - 1];
            driver_plugins.push(create_crossover_plugin(
                crossover_type,
                crossover_freq,
                "high",
            ));
        }

        // Add lowpass crossover to next driver (if not last driver)
        if i < n_drivers - 1 {
            let crossover_freq = crossover_freqs[i];
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
    if !eq_filters.is_empty() {
        combined_plugins.push(create_eq_plugin(eq_filters));
    }

    ChannelDspChain {
        channel: channel_name.to_string(),
        plugins: combined_plugins,
        drivers: Some(driver_chains),
    }
}

/// Build a DSP chain for a multi-subwoofer system
///
/// # Arguments
/// * `channel_name` - Channel name (e.g., "lfe")
/// * `group_name` - Name of the sub group
/// * `n_subs` - Number of subwoofers
/// * `gains` - Per-sub gains in dB
/// * `delays` - Per-sub delays in ms
/// * `eq_filters` - Global EQ filters
pub fn build_multisub_dsp_chain(
    channel_name: &str,
    group_name: &str,
    n_subs: usize,
    gains: &[f64],
    delays: &[f64],
    eq_filters: &[Biquad],
) -> ChannelDspChain {
    // Build per-sub chains
    let mut driver_chains = Vec::new();

    for i in 0..n_subs {
        let mut sub_plugins = Vec::new();

        // Add gain plugin if non-zero
        if i < gains.len() && gains[i].abs() > 0.01 {
            sub_plugins.push(create_gain_plugin(gains[i]));
        }

        // Add delay plugin if non-zero
        if i < delays.len() && delays[i].abs() > 0.001 {
            sub_plugins.push(create_delay_plugin(delays[i]));
        }

        driver_chains.push(DriverDspChain {
            name: format!("{}_{}", group_name, i + 1),
            index: i,
            plugins: sub_plugins,
        });
    }

    // Build combined EQ
    let mut combined_plugins = Vec::new();
    if !eq_filters.is_empty() {
        combined_plugins.push(create_eq_plugin(eq_filters));
    }

    ChannelDspChain {
        channel: channel_name.to_string(),
        plugins: combined_plugins,
        drivers: Some(driver_chains),
    }
}

/// Build a DSP chain for a DBA system
pub fn build_dba_dsp_chain(
    channel_name: &str,
    gains: &[f64],
    delays: &[f64],
    eq_filters: &[Biquad],
) -> ChannelDspChain {
    // 2 "drivers": Front and Rear
    let mut driver_chains = Vec::new();

    // Front (Index 0)
    let mut front_plugins = Vec::new();
    if gains[0].abs() > 0.01 {
        front_plugins.push(create_gain_plugin(gains[0]));
    }
    if delays[0].abs() > 0.001 {
        front_plugins.push(create_delay_plugin(delays[0]));
    }
    driver_chains.push(DriverDspChain {
        name: "Front Array".to_string(),
        index: 0,
        plugins: front_plugins,
    });

    // Rear (Index 1) - Inverted
    let mut rear_plugins = Vec::new();
    // Always add gain plugin to handle inversion even if gain is 0
    rear_plugins.push(create_gain_plugin_with_invert(gains[1], true));

    if delays[1].abs() > 0.001 {
        rear_plugins.push(create_delay_plugin(delays[1]));
    }
    driver_chains.push(DriverDspChain {
        name: "Rear Array".to_string(),
        index: 1,
        plugins: rear_plugins,
    });

    // Combined EQ
    let mut combined_plugins = Vec::new();
    if !eq_filters.is_empty() {
        combined_plugins.push(create_eq_plugin(eq_filters));
    }

    ChannelDspChain {
        channel: channel_name.to_string(),
        plugins: combined_plugins,
        drivers: Some(driver_chains),
    }
}

/// Create complete DSP chain output
pub fn create_dsp_chain_output(
    channels: HashMap<String, ChannelDspChain>,
    metadata: Option<OptimizationMetadata>,
) -> DspChainOutput {
    DspChainOutput {
        version: super::types::default_config_version(),
        channels,
        metadata,
    }
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
