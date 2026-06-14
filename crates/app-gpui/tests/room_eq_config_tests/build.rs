use super::misc::chain;
use super::misc::extract_filter_freqs;
use super::misc::output;
use super::types::ChannelFilters;
use sotf_audio_player_gpui::{ChannelDspChain, ChannelOptResult, DspChainOutput, DspPluginConfig};

/// Simulate the channel-to-filter mapping logic from apply_room_eq_to_player.
/// Given channel names in output order and a DSP output HashMap, returns
/// the ordered list of channel names as they would be mapped to audio indices.
#[allow(dead_code)]
fn build_per_channel_order(
    channel_result_names: &[&str],
    dsp_channels: &std::collections::HashMap<String, ChannelDspChain>,
) -> Vec<String> {
    let mut ordered = Vec::new();
    for name in channel_result_names {
        if dsp_channels.contains_key(*name) {
            ordered.push(name.to_string());
        } else {
            ordered.push(format!("{}(empty)", name));
        }
    }
    ordered
}

/// Build mock optimization results and DSP output for a given speaker config.
/// Each channel gets a unique EQ filter frequency to verify ordering.
fn build_mock_results(speaker_labels: &[&str]) -> (Vec<ChannelOptResult>, DspChainOutput) {
    let mut channel_results = Vec::new();
    let mut channels = std::collections::HashMap::new();

    for (idx, &label) in speaker_labels.iter().enumerate() {
        // Each channel gets a unique filter frequency = (idx+1) * 100 Hz
        let unique_freq = (idx + 1) as f64 * 100.0;

        channel_results.push(ChannelOptResult {
            channel_name: label.to_string(),
            pre_score: 1.0,
            post_score: 0.5,
            eq_filters: vec![sotf_audio_player_gpui::app::types::EqFilterConfig {
                filter_type: "peak".to_string(),
                frequency: unique_freq,
                q: 1.0,
                gain_db: -3.0,
            }],
            broadband_filters: vec![],
            preamp_gain_db: 0.0,
            crossover_freqs: None,
            driver_gains: None,
            original_response: None,
            corrected_response: None,
            normalized_response: None,
            target_curve: None,
            group_delay_before: None,
            group_delay_after: None,
            phase_response_before: None,
            phase_response_after: None,
            impulse_response: None,
        });

        channels.insert(
            label.to_string(),
            chain(
                label,
                vec![DspPluginConfig {
                    plugin_type: "EQ".to_string(),
                    parameters: serde_json::json!({
                        "filters": [{
                            "filter_type": "peak",
                            "frequency": unique_freq,
                            "q": 1.0,
                            "gain_db": -3.0
                        }]
                    }),
                }],
                None,
            ),
        );
    }

    let dsp_output = output(channels);

    (channel_results, dsp_output)
}

/// Test that channel ordering is correct for a given speaker config.
/// The channel_result_names (from recordings) should map filter[i] to audio channel i.
fn assert_channel_ordering(config_name: &str, labels: &[&str]) {
    let (results, dsp_output) = build_mock_results(labels);

    // channel_result_names preserves the output channel order
    let channel_result_names: Vec<String> =
        results.iter().map(|r| r.channel_name.clone()).collect();

    // Extract filter frequencies in the order they'd be applied
    let freqs = extract_filter_freqs(&channel_result_names, &dsp_output);

    // Each channel's unique frequency should match its index: freq = (idx+1) * 100
    for (idx, &freq) in freqs.iter().enumerate() {
        let expected = (idx + 1) as f64 * 100.0;
        assert_eq!(
            freq, expected,
            "{}: channel {} ('{}') has filter freq {}, expected {} — wrong channel ordering!",
            config_name, idx, labels[idx], freq, expected
        );
    }

    // Verify we didn't lose any channels
    assert_eq!(
        freqs.len(),
        labels.len(),
        "{}: expected {} channels, got {}",
        config_name,
        labels.len(),
        freqs.len()
    );
}

#[test]
fn test_channel_ordering_2_0() {
    assert_channel_ordering("2.0", &["L", "R"]);
}

#[test]
fn test_channel_ordering_2_1() {
    assert_channel_ordering("2.1", &["L", "R", "LFE"]);
}

#[test]
fn test_channel_ordering_5_0() {
    assert_channel_ordering("5.0", &["FL", "FR", "C", "SL", "SR"]);
}

#[test]
fn test_channel_ordering_5_1() {
    assert_channel_ordering("5.1", &["FL", "FR", "C", "LFE", "SL", "SR"]);
}

#[test]
fn test_channel_ordering_7_1() {
    assert_channel_ordering("7.1", &["FL", "FR", "C", "LFE", "SL", "SR", "RL", "RR"]);
}

#[test]
fn test_channel_ordering_5_1_2() {
    assert_channel_ordering("5.1.2", &["FL", "FR", "C", "LFE", "SL", "SR", "TFL", "TFR"]);
}

#[test]
fn test_channel_ordering_5_1_4() {
    assert_channel_ordering(
        "5.1.4",
        &[
            "FL", "FR", "C", "LFE", "SL", "SR", "TFL", "TFR", "TRL", "TRR",
        ],
    );
}

#[test]
fn test_channel_ordering_7_1_2() {
    assert_channel_ordering(
        "7.1.2",
        &["FL", "FR", "C", "LFE", "SL", "SR", "RL", "RR", "TFL", "TFR"],
    );
}

#[test]
fn test_channel_ordering_7_1_4() {
    assert_channel_ordering(
        "7.1.4",
        &[
            "FL", "FR", "C", "LFE", "SL", "SR", "RL", "RR", "TFL", "TFR", "TRL", "TRR",
        ],
    );
}

#[test]
fn test_channel_ordering_9_1_4() {
    assert_channel_ordering(
        "9.1.4",
        &[
            "FL", "FR", "C", "LFE", "SL", "SR", "RL", "RR", "FWL", "FWR", "TFL", "TFR", "TRL",
            "TRR",
        ],
    );
}

#[test]
fn test_channel_ordering_9_1_6() {
    assert_channel_ordering(
        "9.1.6",
        &[
            "FL", "FR", "C", "LFE", "SL", "SR", "RL", "RR", "FWL", "FWR", "TFL", "TFR", "TSL",
            "TSR", "TRL", "TRR",
        ],
    );
}

/// Build a DspChainOutput using autoeq format keys ("freq", "db_gain")
/// which is what the real optimizer produces.
pub(super) fn build_autoeq_dsp_output(channels: &[ChannelFilters<'_>]) -> DspChainOutput {
    let mut map = std::collections::HashMap::new();
    for (name, filters) in channels {
        let filter_json: Vec<serde_json::Value> = filters
            .iter()
            .map(|&(freq, q, gain)| {
                serde_json::json!({
                    "filter_type": "peak",
                    "freq": freq,
                    "q": q,
                    "db_gain": gain
                })
            })
            .collect();
        map.insert(
            name.to_string(),
            chain(
                name,
                vec![DspPluginConfig {
                    plugin_type: "eq".to_string(),
                    parameters: serde_json::json!({ "filters": filter_json }),
                }],
                None,
            ),
        );
    }
    output(map)
}
