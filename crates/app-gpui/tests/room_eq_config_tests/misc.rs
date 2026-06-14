use sotf_audio_player::room_eq_types::{DriverDspChain, DspChainOutputExt};
use sotf_audio_player::{
    EQFilter, PluginGraph, PluginSettings, PluginType, room_eq_types::parse_eq_filters_from_json,
};
use sotf_audio_player_gpui::{ChannelDspChain, DspChainOutput, DspPluginConfig};

/// Build a `ChannelDspChain` with no optional curves / impulse responses
/// populated. These tests only exercise channel ordering, broadband /
/// main-EQ separation, and rack compatibility — none of which look at
/// the curve/IR fields — so spelling them out at every call site is
/// pure noise.
pub(super) fn chain(
    name: &str,
    plugins: Vec<DspPluginConfig>,
    drivers: Option<Vec<DriverDspChain>>,
) -> ChannelDspChain {
    ChannelDspChain {
        channel: name.to_string(),
        plugins,
        drivers,
        initial_curve: None,
        final_curve: None,
        eq_response: None,
        target_curve: None,
        pre_ir: None,
        post_ir: None,
        fir_temporal_masking: None,
        direct_early_late_correction: None,
    }
}

/// Build a `DriverDspChain` with no optional `initial_curve`.
fn driver(name: &str, index: usize, plugins: Vec<DspPluginConfig>) -> DriverDspChain {
    DriverDspChain {
        name: name.to_string(),
        index,
        plugins,
        initial_curve: None,
    }
}

/// Build a `DspChainOutput` from a channels map with default version and
/// no metadata.
pub(super) fn output(
    channels: std::collections::HashMap<String, ChannelDspChain>,
) -> DspChainOutput {
    DspChainOutput {
        version: "1.0.0".to_string(),
        global_plugins: Vec::new(),
        channels,
        metadata: None,
    }
}

/// Extract per-channel filter frequencies in the order they'd be applied
/// to audio channels (reproducing the logic from apply_room_eq_to_player).
pub(super) fn extract_filter_freqs(
    channel_result_names: &[String],
    dsp_output: &DspChainOutput,
) -> Vec<f64> {
    let mut freqs = Vec::new();
    for name in channel_result_names {
        if let Some(chain) = dsp_output.channels.get(name) {
            for plugin in &chain.plugins {
                if plugin.plugin_type.eq_ignore_ascii_case("eq")
                    && let Some(filters) =
                        plugin.parameters.get("filters").and_then(|f| f.as_array())
                {
                    for f in filters {
                        if let Some(freq) = f.get("frequency").and_then(|v| v.as_f64()) {
                            freqs.push(freq);
                        }
                    }
                }
            }
        } else {
            freqs.push(0.0); // placeholder for missing channel
        }
    }
    freqs
}

/// Test that HashMap key ordering doesn't affect the result.
/// This verifies the fix: we use channel_result_names (output order),
/// NOT dsp_output.channels.keys() (arbitrary HashMap order).
#[test]
fn test_hashmap_insertion_order_irrelevant() {
    // Insert channels into HashMap in reverse order
    let labels = ["FL", "FR", "C", "LFE", "SL", "SR"];
    let mut channels = std::collections::HashMap::new();

    for (idx, &label) in labels.iter().enumerate().rev() {
        let freq = (idx + 1) as f64 * 100.0;
        channels.insert(
            label.to_string(),
            chain(
                label,
                vec![DspPluginConfig {
                    plugin_type: "EQ".to_string(),
                    parameters: serde_json::json!({
                        "filters": [{"filter_type": "peak", "frequency": freq, "q": 1.0, "gain_db": -3.0}]
                    }),
                }],
                None,
            ),
        );
    }

    // channel_result_names in correct output order
    let channel_result_names: Vec<String> = labels.iter().map(|s| s.to_string()).collect();
    let freqs = extract_filter_freqs(&channel_result_names, &output(channels));

    for (idx, &freq) in freqs.iter().enumerate() {
        let expected = (idx + 1) as f64 * 100.0;
        assert_eq!(freq, expected, "Channel {} has wrong freq", idx);
    }
}

/// Simulate the save-to-rack flow: extract per-channel filters from DSP output,
/// insert or update EQ in the plugin graph, return the resulting graph.
pub(super) fn simulate_save_to_rack(
    channel_result_names: &[&str],
    dsp_output: &DspChainOutput,
    graph: &mut PluginGraph,
) -> (usize, Vec<Vec<EQFilter>>) {
    let mut per_channel_filters: Vec<Vec<EQFilter>> = Vec::new();
    for name in channel_result_names {
        if let Some(chain) = dsp_output.channels.get(*name) {
            let mut channel_eq_filters: Vec<EQFilter> = Vec::new();
            for plugin in &chain.plugins {
                if plugin.plugin_type.eq_ignore_ascii_case("eq")
                    && let Some(filters) =
                        plugin.parameters.get("filters").and_then(|f| f.as_array())
                {
                    channel_eq_filters.extend(parse_eq_filters_from_json(filters));
                }
            }
            per_channel_filters.push(channel_eq_filters);
        } else {
            per_channel_filters.push(Vec::new());
        }
    }

    let total_filters: usize = per_channel_filters.iter().map(|f| f.len()).sum();
    let num_channels = per_channel_filters.len();
    let global_filters = per_channel_filters.first().cloned().unwrap_or_default();

    if total_filters > 0 {
        let new_settings = PluginSettings::EQ {
            channels: num_channels,
            filters: global_filters,
            channel_filters: Some(per_channel_filters.clone()),
            per_channel_mode: true,
            max_filters: 10,
            tdf2: false,
            topology: 0.0,
        };

        if let Some(eq_idx) = graph.find_plugin_index(&PluginType::EQ) {
            if let Some(eq_plugin) = graph.get_plugin_mut(eq_idx) {
                eq_plugin.settings = new_settings;
            }
        } else {
            let insert_idx = graph.user_plugin_insert_index();
            if graph.insert_plugin(insert_idx, &PluginType::EQ).is_ok()
                && let Some(eq_plugin) = graph.get_plugin_mut(insert_idx)
            {
                eq_plugin.settings = new_settings;
            }
        }
    }

    (total_filters, per_channel_filters)
}

#[test]
fn test_save_to_rack_no_filters_detected() {
    let dsp = output(
        [
            (
                "L".to_string(),
                chain(
                    "L",
                    vec![DspPluginConfig {
                        plugin_type: "eq".to_string(),
                        parameters: serde_json::json!({ "filters": [] }),
                    }],
                    None,
                ),
            ),
            (
                "R".to_string(),
                chain(
                    "R",
                    vec![DspPluginConfig {
                        plugin_type: "eq".to_string(),
                        parameters: serde_json::json!({ "filters": [] }),
                    }],
                    None,
                ),
            ),
        ]
        .into_iter()
        .collect(),
    );

    let mut graph = PluginGraph::with_default_rack();
    let (total, _) = simulate_save_to_rack(&["L", "R"], &dsp, &mut graph);
    assert_eq!(total, 0);
    // No EQ should be inserted
    assert!(graph.find_plugin_index(&PluginType::EQ).is_none());
}

#[test]
fn test_save_to_rack_multiple_eq_plugins_merged() {
    let dsp = output(
        [(
            "L".to_string(),
            chain(
                "L",
                vec![
                    DspPluginConfig {
                        plugin_type: "eq".to_string(),
                        parameters: serde_json::json!({ "filters": [
                            {"filter_type": "peak", "freq": 100.0, "q": 1.0, "db_gain": -2.0}
                        ]}),
                    },
                    DspPluginConfig {
                        plugin_type: "EQ".to_string(), // different casing
                        parameters: serde_json::json!({ "filters": [
                            {"filter_type": "peak", "freq": 500.0, "q": 2.0, "db_gain": -4.0}
                        ]}),
                    },
                ],
                None,
            ),
        )]
        .into_iter()
        .collect(),
    );

    let mut graph = PluginGraph::with_default_rack();
    let (total, per_ch) = simulate_save_to_rack(&["L"], &dsp, &mut graph);
    assert_eq!(total, 2);
    assert_eq!(per_ch[0].len(), 2);
    assert_eq!(per_ch[0][0].frequency, 100.0);
    assert_eq!(per_ch[0][1].frequency, 500.0);
}

#[test]
fn test_save_to_rack_non_eq_plugins_skipped() {
    let dsp = output(
        [(
            "L".to_string(),
            chain(
                "L",
                vec![
                    DspPluginConfig {
                        plugin_type: "gain".to_string(),
                        parameters: serde_json::json!({ "gain_db": -6.0 }),
                    },
                    DspPluginConfig {
                        plugin_type: "eq".to_string(),
                        parameters: serde_json::json!({ "filters": [
                            {"filter_type": "peak", "freq": 1000.0, "q": 1.5, "db_gain": -3.0}
                        ]}),
                    },
                    DspPluginConfig {
                        plugin_type: "delay".to_string(),
                        parameters: serde_json::json!({ "delay_ms": 5.0 }),
                    },
                ],
                None,
            ),
        )]
        .into_iter()
        .collect(),
    );

    let mut graph = PluginGraph::with_default_rack();
    let (total, per_ch) = simulate_save_to_rack(&["L"], &dsp, &mut graph);
    assert_eq!(total, 1);
    assert_eq!(per_ch[0][0].frequency, 1000.0);
}

#[test]
fn test_save_to_rack_plugin_type_case_insensitive() {
    let dsp = output(
        [
            (
                "L".to_string(),
                chain(
                    "L",
                    vec![DspPluginConfig {
                        plugin_type: "EQ".to_string(), // uppercase
                        parameters: serde_json::json!({ "filters": [
                            {"filter_type": "peak", "freq": 100.0, "q": 1.0, "db_gain": -1.0}
                        ]}),
                    }],
                    None,
                ),
            ),
            (
                "R".to_string(),
                chain(
                    "R",
                    vec![DspPluginConfig {
                        plugin_type: "Eq".to_string(), // mixed case
                        parameters: serde_json::json!({ "filters": [
                            {"filter_type": "peak", "freq": 200.0, "q": 1.0, "db_gain": -2.0}
                        ]}),
                    }],
                    None,
                ),
            ),
        ]
        .into_iter()
        .collect(),
    );

    let mut graph = PluginGraph::with_default_rack();
    let (total, per_ch) = simulate_save_to_rack(&["L", "R"], &dsp, &mut graph);
    assert_eq!(total, 2);
    assert_eq!(per_ch[0][0].frequency, 100.0);
    assert_eq!(per_ch[1][0].frequency, 200.0);
}

#[test]
fn test_multi_driver_not_rack_compatible() {
    let dsp = output(
        [(
            "L".to_string(),
            chain(
                "L",
                vec![DspPluginConfig {
                    plugin_type: "eq".to_string(),
                    parameters: serde_json::json!({ "filters": [
                        {"filter_type": "peak", "freq": 100.0, "q": 1.0, "db_gain": -3.0}
                    ]}),
                }],
                Some(vec![
                    driver(
                        "woofer",
                        0,
                        vec![DspPluginConfig {
                            plugin_type: "crossover".to_string(),
                            parameters: serde_json::json!({ "type": "lowpass", "freq": 2000.0 }),
                        }],
                    ),
                    driver(
                        "tweeter",
                        1,
                        vec![DspPluginConfig {
                            plugin_type: "crossover".to_string(),
                            parameters: serde_json::json!({ "type": "highpass", "freq": 2000.0 }),
                        }],
                    ),
                ]),
            ),
        )]
        .into_iter()
        .collect(),
    );

    assert!(
        !dsp.is_rack_compatible(),
        "Multi-driver DSP output should NOT be rack compatible"
    );
}
