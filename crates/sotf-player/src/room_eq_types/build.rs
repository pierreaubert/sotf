use super::infer::infer_plugin_output_channels;
use super::linear::linear_room_eq_initial_channels;
use super::linear::linear_room_eq_output_order;
use super::misc::single_channel_matrix_parameters;
use super::types::append_channel_dsp_graph_branch;
use super::types::plugin_stage;
use crate::recording_types::{ChannelRecording, ChannelRecordingState};
pub use autoeq::roomeq::DspChainOutput;
use std::collections::{BTreeMap, HashMap};

/// Build the per-channel speaker map for an `autoeq::RoomConfig` from a
/// finished recording session. All completed takes for the same
/// `channel_index` (every microphone, every measurement position) are
/// folded into a single `SpeakerConfig`, so roomeq emits one EQ chain per
/// real output channel instead of one per (channel × mic × position) take.
///
/// * `channel_names` is indexed by `channel_index` and supplies the bare
///   output name (e.g. `"L"`, `"R"`, `"LFE"`) used as the map key.
/// * `channel_speakers` (optional) maps a channel name to the speaker
///   model string (e.g. `"Genelec 8361A"`); when present it is recorded
///   as `speaker_name` in the resulting source so the optimizer / UI can
///   reference the catalog entry.
///
/// Each take is exported as `MeasurementRef::Inline` carrying the
/// session-relative `wav_path` / `csv_path` so the autoeq optimizer can
/// pick up the WAV for FDW analysis even though the SPL data lives in
/// the CSV file.
pub fn build_speakers_from_recordings(
    recordings: &[ChannelRecording],
    channel_names: &[String],
    channel_speakers: Option<&std::collections::HashMap<String, String>>,
) -> HashMap<String, autoeq::SpeakerConfig> {
    use autoeq::read::{InlineMeasurement, MeasurementMultiple, MeasurementRef, MeasurementSingle};
    use autoeq::{MeasurementSource, SpeakerConfig};

    let mut grouped: BTreeMap<usize, Vec<&ChannelRecording>> = BTreeMap::new();
    for rec in recordings {
        if rec.state != ChannelRecordingState::Done {
            continue;
        }
        if rec.result.is_none() {
            continue;
        }
        grouped.entry(rec.channel_index).or_default().push(rec);
    }

    let mut speakers: HashMap<String, SpeakerConfig> = HashMap::new();
    for (channel_index, mut group) in grouped {
        group.sort_by_key(|r| (r.mic_position_index, r.mic_index));

        let primary = group[0];
        let base_name = channel_names
            .get(channel_index)
            .cloned()
            .unwrap_or_else(|| {
                primary
                    .channel_name
                    .find(" (")
                    .map_or(primary.channel_name.as_str(), |pos| {
                        &primary.channel_name[..pos]
                    })
                    .to_string()
            });

        let measurement_refs: Vec<MeasurementRef> = group
            .iter()
            .filter_map(|rec| {
                let result = rec.result.as_ref()?;
                let relative_wav = result
                    .wav_path
                    .as_ref()
                    .and_then(|p| std::path::Path::new(p).file_name())
                    .map(|f| f.to_string_lossy().to_string());
                let relative_csv = result
                    .csv_path
                    .as_ref()
                    .and_then(|p| std::path::Path::new(p).file_name())
                    .map(|f| f.to_string_lossy().to_string());
                if relative_wav.is_none() && relative_csv.is_none() {
                    return None;
                }
                Some(MeasurementRef::Inline(InlineMeasurement {
                    frequencies: Vec::new(),
                    magnitude_db: Vec::new(),
                    phase_deg: None,
                    name: Some(rec.channel_name.clone()),
                    wav_path: relative_wav,
                    csv_path: relative_csv,
                }))
            })
            .collect();

        if measurement_refs.is_empty() {
            continue;
        }

        let speaker_name = channel_speakers
            .and_then(|m| m.get(&base_name))
            .map(|s| s.to_string());

        let source = if measurement_refs.len() == 1 {
            MeasurementSource::Single(MeasurementSingle {
                measurement: measurement_refs.into_iter().next().unwrap(),
                speaker_name,
            })
        } else {
            MeasurementSource::Multiple(MeasurementMultiple {
                measurements: measurement_refs,
                speaker_name,
            })
        };

        speakers.insert(base_name, SpeakerConfig::Single(source));
    }
    speakers
}

/// Build native playback from AutoEQ's resolved physical routing contract.
/// Input processing precedes fan-out; each route retains its own transfer;
/// physical-output processing follows summation at that destination.
pub fn build_room_eq_plugin_graph_config(
    output: &DspChainOutput,
    _sample_rate: f64,
) -> anyhow::Result<sotf_audio::engine::PluginGraphConfig> {
    let routed_graph = output
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.bass_management.as_ref())
        .and_then(|report| report.routing_graph.as_ref())
        .filter(|graph| !graph.routes.is_empty());

    if let Some(graph) = routed_graph {
        return build_routed_room_eq_graph(output, graph);
    }

    build_linear_room_eq_graph(output)
}

fn build_routed_room_eq_graph(
    output: &DspChainOutput,
    graph: &autoeq::roomeq::BassManagementRoutingGraph,
) -> anyhow::Result<sotf_audio::engine::PluginGraphConfig> {
    let physical =
        autoeq::roomeq_engine::physical_routing::resolve_physical_routing(&output.channels, graph)?;
    super::physical::build_physical_room_eq_graph(&physical, &output.global_plugins)
}

fn build_linear_room_eq_graph(
    output: &DspChainOutput,
) -> anyhow::Result<sotf_audio::engine::PluginGraphConfig> {
    use sotf_audio::engine::{PluginGraphConfig, PluginGraphEdgeConfig, PluginGraphNodeConfig};

    // Driver branches need parallel paths into a per-channel summing matrix
    // and can't be collapsed into a single multichannel plugin without
    // changing audio behavior. Fall through to the legacy per-channel
    // emission when any channel has drivers.
    let has_drivers = output
        .channels
        .values()
        .any(|chain| chain.drivers.as_ref().is_some_and(|d| !d.is_empty()));
    if has_drivers {
        return build_linear_room_eq_graph_legacy(output);
    }

    // No drivers: emit the factored form. After global plugins (which may
    // change channel width — upmixer/downmix/XTC), the per-channel chains
    // collapse to one multichannel `gain_pre`, one `eq_pre`, one `eq_post`
    // at the post-global width. Each chain's gain/eq lands at its channel
    // index in the per-channel parameter arrays.
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut next_id = 0usize;
    let output_order = linear_room_eq_output_order(output);
    let mut current_channels = linear_room_eq_initial_channels(output, output_order.len());

    let mut add_node =
        |plugin_type: String, parameters: serde_json::Value, input_channels: usize| -> usize {
            let id = next_id;
            next_id += 1;
            nodes.push(PluginGraphNodeConfig {
                id,
                plugin_type,
                parameters,
                input_channels,
                bypassed: false,
            });
            id
        };

    let mut global_tail = None;
    for plugin in &output.global_plugins {
        let node = add_node(
            plugin.plugin_type.clone(),
            plugin.parameters.clone(),
            current_channels,
        );
        if let Some(prev) = global_tail {
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: node,
            });
        }
        global_tail = Some(node);
        current_channels = infer_plugin_output_channels(plugin, current_channels);
    }

    // Walk each channel's chain once and pull per-stage values into the
    // factored arrays.
    let channel_count = current_channels.max(output_order.len());
    let mut gain_pre_db = vec![0.0f32; channel_count];
    let mut gain_post_db = vec![0.0f32; channel_count];
    let mut filters_pre: Vec<Vec<serde_json::Value>> = vec![Vec::new(); channel_count];
    let mut filters_post: Vec<Vec<serde_json::Value>> = vec![Vec::new(); channel_count];

    for (idx, channel_name) in output_order.iter().enumerate() {
        if idx >= channel_count {
            break;
        }
        let Some(chain) = output.channels.get(channel_name) else {
            continue;
        };
        for plugin in &chain.plugins {
            // For the linear case, stage tagging is often missing. Treat
            // unlabelled gain/eq as pre_route by default.
            let stage = plugin_stage(plugin).unwrap_or("pre_route");
            match (plugin.plugin_type.as_str(), stage) {
                ("gain", "post_route") => {
                    gain_post_db[idx] += plugin
                        .parameters
                        .get("gain_db")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0) as f32;
                }
                ("gain", _) => {
                    gain_pre_db[idx] += plugin
                        .parameters
                        .get("gain_db")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0) as f32;
                }
                ("eq", "post_route") => {
                    if let Some(arr) = plugin.parameters.get("filters").and_then(|v| v.as_array()) {
                        filters_post[idx].extend(arr.iter().cloned());
                    }
                }
                ("eq", _) => {
                    if let Some(arr) = plugin.parameters.get("filters").and_then(|v| v.as_array()) {
                        filters_pre[idx].extend(arr.iter().cloned());
                    }
                }
                _ => {
                    // Other plugin types in the linear (no-routing, no-driver)
                    // path are uncommon. If we encounter them, emit them
                    // verbatim into the global tail so behavior isn't silently
                    // dropped.
                    let node = add_node(
                        plugin.plugin_type.clone(),
                        plugin.parameters.clone(),
                        current_channels,
                    );
                    if let Some(prev) = global_tail {
                        edges.push(PluginGraphEdgeConfig {
                            from_node: prev,
                            to_node: node,
                        });
                    }
                    global_tail = Some(node);
                }
            }
        }
    }

    let gain_pre_id = add_node(
        "gain".to_string(),
        serde_json::json!({
            "label": "room_eq_gain_pre",
            "gain_db": 0.0,
            "channel_gains": gain_pre_db,
        }),
        current_channels,
    );
    if let Some(prev) = global_tail {
        edges.push(PluginGraphEdgeConfig {
            from_node: prev,
            to_node: gain_pre_id,
        });
    }
    let eq_pre_id = add_node(
        "eq".to_string(),
        serde_json::json!({
            "label": "room_eq_eq_pre",
            "channel_filters": filters_pre,
        }),
        current_channels,
    );
    edges.push(PluginGraphEdgeConfig {
        from_node: gain_pre_id,
        to_node: eq_pre_id,
    });
    let eq_post_id = add_node(
        "eq".to_string(),
        serde_json::json!({
            "label": "room_eq_eq_post",
            "channel_filters": filters_post,
        }),
        current_channels,
    );
    edges.push(PluginGraphEdgeConfig {
        from_node: eq_pre_id,
        to_node: eq_post_id,
    });
    let _gain_post_id = add_node(
        "gain".to_string(),
        serde_json::json!({
            "label": "room_eq_gain_post",
            "gain_db": 0.0,
            "channel_gains": gain_post_db,
        }),
        current_channels,
    );
    edges.push(PluginGraphEdgeConfig {
        from_node: eq_post_id,
        to_node: _gain_post_id,
    });

    if nodes.is_empty() {
        anyhow::bail!("No plugins in DSP output");
    }

    Ok(PluginGraphConfig { nodes, edges })
}

/// Legacy per-channel-isolator emission for the linear path when channels
/// have drivers. Driver branches need parallel paths into a per-channel
/// summing matrix and can't be collapsed without changing audio behavior.
fn build_linear_room_eq_graph_legacy(
    output: &DspChainOutput,
) -> anyhow::Result<sotf_audio::engine::PluginGraphConfig> {
    use sotf_audio::engine::{PluginGraphConfig, PluginGraphEdgeConfig, PluginGraphNodeConfig};

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut next_id = 0usize;
    let output_order = linear_room_eq_output_order(output);
    let mut current_channels = linear_room_eq_initial_channels(output, output_order.len());

    let mut add_node =
        |plugin_type: String, parameters: serde_json::Value, input_channels: usize| -> usize {
            let id = next_id;
            next_id += 1;
            nodes.push(PluginGraphNodeConfig {
                id,
                plugin_type,
                parameters,
                input_channels,
                bypassed: false,
            });
            id
        };

    let mut global_tail = None;
    for plugin in &output.global_plugins {
        let node = add_node(
            plugin.plugin_type.clone(),
            plugin.parameters.clone(),
            current_channels,
        );
        if let Some(prev) = global_tail {
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: node,
            });
        }
        global_tail = Some(node);
        current_channels = infer_plugin_output_channels(plugin, current_channels);
    }

    for (channel_index, channel_name) in output_order.iter().enumerate() {
        let isolate = add_node(
            "matrix".to_string(),
            single_channel_matrix_parameters(
                current_channels,
                channel_index,
                channel_index,
                1.0,
                format!("room_eq_output_isolate_{channel_name}"),
                None,
            ),
            current_channels,
        );
        if let Some(global_tail) = global_tail {
            edges.push(PluginGraphEdgeConfig {
                from_node: global_tail,
                to_node: isolate,
            });
        }
        if let Some(chain) = output.channels.get(channel_name) {
            append_channel_dsp_graph_branch(
                &mut add_node,
                &mut edges,
                isolate,
                Some(chain),
                chain.plugins.iter(),
                current_channels,
                channel_name,
            );
        }
    }

    if nodes.is_empty() {
        anyhow::bail!("No plugins in DSP output");
    }

    Ok(PluginGraphConfig { nodes, edges })
}
