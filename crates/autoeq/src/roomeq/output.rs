//! Output generation for room EQ DSP chains

use super::types::{
    ChannelDspChain, DriverDspChain, DspChainOutput, OptimizationMetadata, PluginConfigWrapper,
};
use math_audio_iir_fir::Biquad;
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
    build_channel_dsp_chain_with_curves(channel_name, gain_db, crossovers, eq_filters, None, None)
}

/// Build a DSP chain for a single channel with optional curves
pub fn build_channel_dsp_chain_with_curves(
    channel_name: &str,
    gain_db: Option<f64>,
    crossovers: Vec<PluginConfigWrapper>,
    eq_filters: &[Biquad],
    initial_curve: Option<&crate::Curve>,
    final_curve: Option<&crate::Curve>,
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
        drivers: None,
        initial_curve: initial_curve.map(|c| c.into()),
        final_curve: final_curve.map(|c| c.into()),
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
    build_multidriver_dsp_chain_with_curves(
        channel_name,
        gains,
        delays,
        crossover_freqs,
        crossover_type,
        eq_filters,
        None,
        None,
    )
}

/// Build a DSP chain for a multi-driver speaker with curves
pub fn build_multidriver_dsp_chain_with_curves(
    channel_name: &str,
    gains: &[f64],
    delays: &[f64],
    crossover_freqs: &[f64],
    crossover_type: &str,
    eq_filters: &[Biquad],
    initial_curve: Option<&crate::Curve>,
    final_curve: Option<&crate::Curve>,
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
        initial_curve: initial_curve.map(|c| c.into()),
        final_curve: final_curve.map(|c| c.into()),
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
    build_multisub_dsp_chain_with_curves(
        channel_name,
        group_name,
        n_subs,
        gains,
        delays,
        eq_filters,
        None,
        None,
    )
}

/// Build a DSP chain for a multi-subwoofer system with curves
pub fn build_multisub_dsp_chain_with_curves(
    channel_name: &str,
    group_name: &str,
    n_subs: usize,
    gains: &[f64],
    delays: &[f64],
    eq_filters: &[Biquad],
    initial_curve: Option<&crate::Curve>,
    final_curve: Option<&crate::Curve>,
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
        initial_curve: initial_curve.map(|c| c.into()),
        final_curve: final_curve.map(|c| c.into()),
    }
}

/// Build a DSP chain for a DBA system
pub fn build_dba_dsp_chain(
    channel_name: &str,
    gains: &[f64],
    delays: &[f64],
    eq_filters: &[Biquad],
) -> ChannelDspChain {
    build_dba_dsp_chain_with_curves(channel_name, gains, delays, eq_filters, None, None)
}

/// Build a DSP chain for a DBA system with curves
pub fn build_dba_dsp_chain_with_curves(
    channel_name: &str,
    gains: &[f64],
    delays: &[f64],
    eq_filters: &[Biquad],
    initial_curve: Option<&crate::Curve>,
    final_curve: Option<&crate::Curve>,
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
        initial_curve: initial_curve.map(|c| c.into()),
        final_curve: final_curve.map(|c| c.into()),
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

/// Add a delay plugin to an existing chain
pub fn add_delay_plugin(chain: &mut ChannelDspChain, delay_ms: f64) {
    let plugin = create_delay_plugin(delay_ms);
    // Insert at the beginning to ensure it applies before other processing (though usually commutative with linear filters)
    chain.plugins.insert(0, plugin);
}

#[cfg(test)]
mod tests {
    use super::*;
    use math_audio_iir_fir::BiquadFilterType;

    #[test]
    fn test_create_gain_plugin() {
        let plugin = create_gain_plugin(-3.5);
        assert_eq!(plugin.plugin_type, "gain");
        assert_eq!(
            plugin.parameters.get("gain_db").unwrap().as_f64().unwrap(),
            -3.5
        );
    }

    #[test]
    fn test_create_gain_plugin_with_invert() {
        let plugin = create_gain_plugin_with_invert(-2.0, true);
        assert_eq!(plugin.plugin_type, "gain");
        assert_eq!(
            plugin.parameters.get("gain_db").unwrap().as_f64().unwrap(),
            -2.0
        );
        assert!(plugin.parameters.get("invert").unwrap().as_bool().unwrap());

        let plugin_no_invert = create_gain_plugin_with_invert(1.5, false);
        assert!(
            !plugin_no_invert
                .parameters
                .get("invert")
                .unwrap()
                .as_bool()
                .unwrap()
        );
    }

    #[test]
    fn test_create_eq_plugin() {
        let sample_rate = 48000.0;
        let filters = vec![
            Biquad::new(BiquadFilterType::Peak, 1000.0, sample_rate, 2.0, -3.0),
            Biquad::new(BiquadFilterType::Peak, 4000.0, sample_rate, 1.5, 2.0),
        ];

        let plugin = create_eq_plugin(&filters);
        assert_eq!(plugin.plugin_type, "eq");

        let filters_arr = plugin
            .parameters
            .get("filters")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(filters_arr.len(), 2);

        let first_filter = &filters_arr[0];
        assert_eq!(first_filter.get("freq").unwrap().as_f64().unwrap(), 1000.0);
        assert_eq!(first_filter.get("q").unwrap().as_f64().unwrap(), 2.0);
        assert_eq!(first_filter.get("db_gain").unwrap().as_f64().unwrap(), -3.0);
    }

    #[test]
    fn test_create_crossover_plugin() {
        let plugin = create_crossover_plugin("LR24", 2500.0, "low");
        assert_eq!(plugin.plugin_type, "crossover");
        assert_eq!(
            plugin.parameters.get("type").unwrap().as_str().unwrap(),
            "LR24"
        );
        assert_eq!(
            plugin
                .parameters
                .get("frequency")
                .unwrap()
                .as_f64()
                .unwrap(),
            2500.0
        );
        assert_eq!(
            plugin.parameters.get("output").unwrap().as_str().unwrap(),
            "low"
        );
    }

    #[test]
    fn test_create_delay_plugin() {
        let plugin = create_delay_plugin(15.5);
        assert_eq!(plugin.plugin_type, "delay");
        assert_eq!(
            plugin.parameters.get("delay_ms").unwrap().as_f64().unwrap(),
            15.5
        );
    }

    #[test]
    fn test_create_convolution_plugin() {
        let plugin = create_convolution_plugin("left_fir.wav");
        assert_eq!(plugin.plugin_type, "convolution");
        assert_eq!(
            plugin.parameters.get("ir_file").unwrap().as_str().unwrap(),
            "left_fir.wav"
        );
    }

    #[test]
    fn test_build_channel_dsp_chain_with_gain_and_eq() {
        let sample_rate = 48000.0;
        let filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            sample_rate,
            2.0,
            -3.0,
        )];

        let chain = build_channel_dsp_chain("left", Some(-2.5), Vec::new(), &filters);

        assert_eq!(chain.channel, "left");
        assert_eq!(chain.plugins.len(), 2); // gain + eq
        assert_eq!(chain.plugins[0].plugin_type, "gain");
        assert_eq!(chain.plugins[1].plugin_type, "eq");
        assert!(chain.drivers.is_none());
    }

    #[test]
    fn test_build_channel_dsp_chain_zero_gain_not_added() {
        // Gain of 0.0 should not add gain plugin
        let chain = build_channel_dsp_chain("test", Some(0.0), Vec::new(), &[]);
        assert!(!chain.plugins.iter().any(|p| p.plugin_type == "gain"));
    }

    #[test]
    fn test_build_channel_dsp_chain_small_gain_not_added() {
        // Gain < 0.01 should not be added
        let chain = build_channel_dsp_chain("test", Some(0.005), Vec::new(), &[]);
        assert!(!chain.plugins.iter().any(|p| p.plugin_type == "gain"));
    }

    #[test]
    fn test_build_multidriver_dsp_chain_2way() {
        let gains = vec![-3.0, 0.0];
        let delays = vec![2.5, 0.0];
        let crossover_freqs = vec![2500.0];

        let chain =
            build_multidriver_dsp_chain("left", &gains, &delays, &crossover_freqs, "LR24", &[]);

        assert_eq!(chain.channel, "left");
        assert!(chain.drivers.is_some());

        let drivers = chain.drivers.as_ref().unwrap();
        assert_eq!(drivers.len(), 2);

        // Verify woofer (index 0)
        let woofer = &drivers[0];
        assert_eq!(woofer.name, "woofer");
        assert_eq!(woofer.index, 0);
        // Woofer should have: gain, delay, lowpass crossover
        assert!(woofer.plugins.iter().any(|p| p.plugin_type == "gain"));
        assert!(woofer.plugins.iter().any(|p| p.plugin_type == "delay"));
        assert!(woofer.plugins.iter().any(|p| {
            p.plugin_type == "crossover"
                && p.parameters.get("output").unwrap().as_str().unwrap() == "low"
        }));

        // Verify tweeter (index 1)
        let tweeter = &drivers[1];
        assert_eq!(tweeter.name, "tweeter");
        assert_eq!(tweeter.index, 1);
        // Tweeter should have highpass crossover (no gain since it's 0)
        assert!(tweeter.plugins.iter().any(|p| {
            p.plugin_type == "crossover"
                && p.parameters.get("output").unwrap().as_str().unwrap() == "high"
        }));
    }

    #[test]
    fn test_build_multidriver_dsp_chain_3way() {
        let gains = vec![0.0, -2.0, 1.0];
        let delays = vec![0.0, 1.0, 2.0];
        let crossover_freqs = vec![500.0, 3000.0];

        let chain =
            build_multidriver_dsp_chain("center", &gains, &delays, &crossover_freqs, "LR24", &[]);

        let drivers = chain.drivers.as_ref().unwrap();
        assert_eq!(drivers.len(), 3);

        assert_eq!(drivers[0].name, "woofer");
        assert_eq!(drivers[1].name, "midrange");
        assert_eq!(drivers[2].name, "tweeter");

        // Midrange should have both highpass (from woofer) and lowpass (to tweeter)
        let midrange = &drivers[1];
        let has_highpass = midrange.plugins.iter().any(|p| {
            p.plugin_type == "crossover"
                && p.parameters.get("output").unwrap().as_str().unwrap() == "high"
        });
        let has_lowpass = midrange.plugins.iter().any(|p| {
            p.plugin_type == "crossover"
                && p.parameters.get("output").unwrap().as_str().unwrap() == "low"
        });
        assert!(has_highpass, "Midrange should have highpass crossover");
        assert!(has_lowpass, "Midrange should have lowpass crossover");
    }

    #[test]
    fn test_build_multisub_dsp_chain() {
        let gains = vec![-2.0, 0.0, 1.0];
        let delays = vec![0.0, 5.0, 10.0];

        let chain = build_multisub_dsp_chain("lfe", "subs", 3, &gains, &delays, &[]);

        assert_eq!(chain.channel, "lfe");
        assert!(chain.drivers.is_some());

        let drivers = chain.drivers.as_ref().unwrap();
        assert_eq!(drivers.len(), 3);

        assert_eq!(drivers[0].name, "subs_1");
        assert_eq!(drivers[1].name, "subs_2");
        assert_eq!(drivers[2].name, "subs_3");

        // Sub 1 should have delay (5ms)
        assert!(drivers[1].plugins.iter().any(|p| p.plugin_type == "delay"));
    }

    #[test]
    fn test_build_dba_dsp_chain() {
        let gains = vec![0.0, -3.0];
        let delays = vec![0.0, 5.0];

        let chain = build_dba_dsp_chain("dba", &gains, &delays, &[]);

        assert_eq!(chain.channel, "dba");
        assert!(chain.drivers.is_some());

        let drivers = chain.drivers.as_ref().unwrap();
        assert_eq!(drivers.len(), 2);

        // Front array
        let front = &drivers[0];
        assert_eq!(front.name, "Front Array");
        assert_eq!(front.index, 0);

        // Rear array should have invert flag
        let rear = &drivers[1];
        assert_eq!(rear.name, "Rear Array");
        assert_eq!(rear.index, 1);

        let rear_gain = rear
            .plugins
            .iter()
            .find(|p| p.plugin_type == "gain")
            .expect("Rear should have gain plugin");
        assert!(
            rear_gain
                .parameters
                .get("invert")
                .unwrap()
                .as_bool()
                .unwrap(),
            "Rear should be inverted"
        );

        // Rear should have delay
        assert!(rear.plugins.iter().any(|p| p.plugin_type == "delay"));
    }

    #[test]
    fn test_add_delay_plugin() {
        let mut chain = ChannelDspChain {
            channel: "test".to_string(),
            plugins: vec![create_gain_plugin(-3.0)],
            drivers: None,
            initial_curve: None,
            final_curve: None,
        };

        add_delay_plugin(&mut chain, 10.0);

        // Delay should be inserted at the beginning
        assert_eq!(chain.plugins.len(), 2);
        assert_eq!(chain.plugins[0].plugin_type, "delay");
        assert_eq!(chain.plugins[1].plugin_type, "gain");
    }

    #[test]
    fn test_create_dsp_chain_output() {
        let mut channels = HashMap::new();
        channels.insert(
            "left".to_string(),
            build_channel_dsp_chain("left", Some(-2.0), Vec::new(), &[]),
        );

        let metadata = OptimizationMetadata {
            pre_score: 5.0,
            post_score: 2.0,
            algorithm: "cobyla".to_string(),
            iterations: 1000,
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };

        let output = create_dsp_chain_output(channels, Some(metadata));

        assert!(output.channels.contains_key("left"));
        assert!(output.metadata.is_some());

        let meta = output.metadata.unwrap();
        assert_eq!(meta.pre_score, 5.0);
        assert_eq!(meta.post_score, 2.0);
    }

    #[test]
    fn test_get_driver_name() {
        // 2-way
        assert_eq!(get_driver_name(0, 2), "woofer");
        assert_eq!(get_driver_name(1, 2), "tweeter");

        // 3-way
        assert_eq!(get_driver_name(0, 3), "woofer");
        assert_eq!(get_driver_name(1, 3), "midrange");
        assert_eq!(get_driver_name(2, 3), "tweeter");

        // 4-way
        assert_eq!(get_driver_name(0, 4), "woofer");
        assert_eq!(get_driver_name(1, 4), "lower_midrange");
        assert_eq!(get_driver_name(2, 4), "upper_midrange");
        assert_eq!(get_driver_name(3, 4), "tweeter");

        // Fallback
        assert_eq!(get_driver_name(5, 8), "driver_5");
    }
}
