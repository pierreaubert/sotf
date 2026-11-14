//! Shared plugin introspection query helper.

use crate::controllers::plugin::{
    desktop_param_indices as query_param_indices, desktop_param_spec as query_param_spec,
    desktop_param_value as query_param_value,
};
use crate::plugin_graph::PluginGraph;
use anyhow::{Result, anyhow};
use serde_json::{Value, json};

/// Resolve a plugin introspection query path against `graph`.
///
/// Supported paths:
/// - `plugins.count` -> number of plugin nodes
/// - `plugins.list` -> array of `{ index, type, enabled, param_count }`
/// - `plugins.plugin.{index}.type` -> plugin type display name
/// - `plugins.plugin.{index}.enabled` -> whether the plugin is enabled
/// - `plugins.plugin.{index}.param_count` -> number of parameters
/// - `plugins.plugin.{index}.crosspoint.{output}.{input}.value` -> stored Matrix gain
/// - `plugins.plugin.{index}.param_indices` -> actual queryable parameter IDs
/// - `plugins.plugin.{index}.param.{i}.name|value|type|min|max|choice_count`
///
/// # Errors
/// Returns an error if the query path or plugin index is invalid.
pub fn plugin_query(graph: &PluginGraph, path: &str) -> Result<Value> {
    Ok(match path {
        "plugins.count" => json!(graph.len()),
        "plugins.list" => {
            let list: Vec<Value> = (0..graph.len())
                .filter_map(|idx| {
                    graph.get_plugin(idx).map(|plugin| {
                        json!({
                            "index": idx,
                            "type": plugin.plugin_type().name(),
                            "enabled": plugin.enabled,
                            "param_count": crate::get_param_count(&plugin.settings),
                            "permanent": plugin.permanent,
                        })
                    })
                })
                .collect();
            json!(list)
        }
        other => {
            if let Some(rest) = other.strip_prefix("plugins.plugin.") {
                resolve_plugin_path(graph, rest)?
            } else {
                return Err(anyhow!("unknown plugin query path: `{other}`"));
            }
        }
    })
}

fn resolve_plugin_path(graph: &PluginGraph, rest: &str) -> Result<Value> {
    let (idx_str, tail) = rest
        .split_once('.')
        .ok_or_else(|| anyhow!("expected plugins.plugin.<index>.<tail>"))?;
    let idx: usize = idx_str.parse()?;
    let plugin = graph
        .get_plugin(idx)
        .ok_or_else(|| anyhow!("plugin index {idx} out of range"))?;

    match tail {
        "type" => Ok(json!(plugin.plugin_type().name())),
        "enabled" => Ok(json!(plugin.enabled)),
        "param_count" => Ok(json!(crate::get_param_count(&plugin.settings))),
        "param_indices" => Ok(json!(query_param_indices(&plugin.settings))),
        "param_descriptors" => Ok(json!({
            "instance_id": plugin.id,
            "plugin_type": plugin.plugin_type().name(),
            "plugin_enabled": plugin.enabled,
            "input_channels": graph.plugin_input_channels(idx),
            "stored_channel_states": match &plugin.settings {
                crate::PluginSettings::ChannelMuteSolo { channel_states, .. } => Some(channel_states.len()),
                _ => None,
            },
            "parameters": query_param_descriptors(&plugin.settings)?,
            "scoped_fields": query_scoped_fields(&plugin.settings, graph.plugin_input_channels(idx)),
        })),
        other if other.starts_with("crosspoint.") => {
            resolve_matrix_crosspoint(&plugin.settings, &other["crosspoint.".len()..])
        }
        other => {
            let prefix = "param.";
            let rest = other
                .strip_prefix(prefix)
                .ok_or_else(|| anyhow!("unknown plugin path: `{other}`"))?;
            resolve_param_path(&plugin.settings, rest)
        }
    }
}

// Matrix routes are structural fields, not synthetic numeric parameter IDs.
fn resolve_matrix_crosspoint(settings: &crate::PluginSettings, rest: &str) -> Result<Value> {
    let crate::PluginSettings::Matrix {
        input_channels,
        output_channels,
        matrix,
        ..
    } = settings
    else {
        return Err(anyhow!("crosspoint queries require a Matrix plugin"));
    };
    let mut parts = rest.split('.');
    let output: usize = parts
        .next()
        .ok_or_else(|| anyhow!("missing output"))?
        .parse()?;
    let input: usize = parts
        .next()
        .ok_or_else(|| anyhow!("missing input"))?
        .parse()?;
    if parts.next() != Some("value") || parts.next().is_some() {
        return Err(anyhow!("expected crosspoint.<output>.<input>.value"));
    }
    if output >= *output_channels || input >= *input_channels {
        return Err(anyhow!("crosspoint channel out of range"));
    }
    let index = output
        .checked_mul(*input_channels)
        .and_then(|index| index.checked_add(input))
        .ok_or_else(|| anyhow!("crosspoint index overflow"))?;
    let value = matrix
        .get(index)
        .ok_or_else(|| anyhow!("crosspoint gain is missing"))?;
    Ok(json!(value))
}

fn resolve_param_path(settings: &crate::PluginSettings, rest: &str) -> Result<Value> {
    let (idx_str, tail) = rest
        .split_once('.')
        .ok_or_else(|| anyhow!("expected param.<i>.<prop>"))?;
    let idx: usize = idx_str.parse()?;
    if !query_param_indices(settings).contains(&idx) {
        return Err(anyhow!("param index out of range"));
    }
    let spec =
        query_param_spec(settings, idx).ok_or_else(|| anyhow!("param index out of range"))?;

    Ok(match tail {
        "name" => json!(spec.name),
        "value" => json!(
            query_param_value(settings, idx)
                .ok_or_else(|| anyhow!("param index {idx} has no numeric value getter"))?
        ),
        "type" => json!(param_type_name(&spec.param_type)),
        "min" => json!(spec.min_f64()),
        "max" => json!(spec.max_f64()),
        "choice_count" => json!(spec.choice_labels().len()),
        other => return Err(anyhow!("unknown param property: `{other}`")),
    })
}

fn query_param_descriptors(settings: &crate::PluginSettings) -> Result<Vec<Value>> {
    query_param_indices(settings)
        .into_iter()
        .map(|index| {
            let spec = query_param_spec(settings, index)
                .ok_or_else(|| anyhow!("param index {index} has no descriptor"))?;
            let (default, step) = match spec.param_type {
                sotf_plugins::param_specs::ParamType::Float { default, step, .. } =>
                    (json!(default), json!(step)),
                sotf_plugins::param_specs::ParamType::Int { default, step, .. } =>
                    (json!(default), json!(step)),
                sotf_plugins::param_specs::ParamType::Bool { default, .. } =>
                    (json!(default), Value::Null),
                sotf_plugins::param_specs::ParamType::Choice { default_index, .. } =>
                    (json!(default_index), Value::Null),
                sotf_plugins::param_specs::ParamType::FilePath => (Value::Null, Value::Null),
            };
            let value = query_param_value(settings, index).map_or(Value::Null, |v| json!(v));
            let file_value = query_file_path_value(settings, spec.engine_key);
            let string_value = file_value.flatten().map_or(Value::Null, |v| json!(v));
            let source_path = match (settings, spec.engine_key) {
                (crate::PluginSettings::ABCompare { path_a_file, .. }, "path_a_config") => json!(path_a_file),
                (crate::PluginSettings::ABCompare { path_b_file, .. }, "path_b_config") => json!(path_b_file),
                _ => Value::Null,
            };
            let engine_param = crate::controllers::plugin_param_map::param_index_to_engine_param(settings, index);
            let value_kind = if matches!(settings, crate::PluginSettings::ABCompare { .. })
                && matches!(spec.engine_key, "path_a_config" | "path_b_config")
            {
                "config_json"
            } else if matches!(spec.param_type, sotf_plugins::param_specs::ParamType::FilePath) {
                if file_value.is_some() { "file_path" } else { "missing_getter" }
            } else if value.is_null() {
                "missing_getter"
            } else {
                "numeric"
            };
            Ok(json!({
                "index": index,
                "key": spec.engine_key,
                "name": spec.name,
                "type": param_type_name(&spec.param_type),
                "unit": spec.unit,
                "group": spec.group,
                "category": format!("{:?}", spec.category),
                "update_mode": format!("{:?}", spec.update_mode),
                "dispatch_route": if matches!(spec.param_type, sotf_plugins::param_specs::ParamType::FilePath) {
                    "file_structural"
                } else if engine_param.is_some() {
                    "parameter"
                } else {
                    "structural"
                },
                "dispatch_key": engine_param.as_ref().map(|(key, _)| key),
                "default": default,
                "step": step,
                "min": spec.min_f64(),
                "max": spec.max_f64(),
                "display_scale": spec.display_scale,
                "choice_labels": spec.choice_labels(),
                "numeric_value": value,
                "string_value": string_value,
                "source_path": source_path,
                "value_kind": value_kind,
            }))
        })
        .collect()
}

#[expect(
    clippy::option_option,
    reason = "The query distinguishes unsupported parameters, absent paths, and present paths "
)]
fn query_file_path_value<'a>(
    settings: &'a crate::PluginSettings,
    key: &str,
) -> Option<Option<&'a str>> {
    match (settings, key) {
        (crate::PluginSettings::Convolution { ir_file, .. }, "ir_file") => Some(Some(ir_file)),
        (crate::PluginSettings::BinauralDecoder { sofa_file, .. }, "sofa_file") => {
            Some(Some(sofa_file))
        }
        (
            crate::PluginSettings::BinauralDecoder {
                hrtf_database_dir, ..
            },
            "hrtf_database_dir",
        ) => Some(Some(hrtf_database_dir)),
        (crate::PluginSettings::XTC { room_ir_file, .. }, "room_ir_file") => {
            Some(room_ir_file.as_deref())
        }
        (crate::PluginSettings::ABCompare { path_a_config, .. }, "path_a_config") => {
            Some(Some(path_a_config))
        }
        (crate::PluginSettings::ABCompare { path_b_config, .. }, "path_b_config") => {
            Some(Some(path_b_config))
        }
        _ => None,
    }
}

fn query_scoped_fields(
    settings: &crate::PluginSettings,
    expected_channels: Option<usize>,
) -> Vec<Value> {
    let mut fields = Vec::new();
    match settings {
        crate::PluginSettings::ChannelMuteSolo { channel_states, .. } => {
            for channel in 0..expected_channels.unwrap_or(channel_states.len()) {
                let state = channel_states.get(channel);
                fields.push(json!({"scope": "channel", "channel": channel, "field": "mute", "value": state.map(|s| s.muted), "dispatch_route": "structural"}));
                fields.push(json!({"scope": "channel", "channel": channel, "field": "solo", "value": state.map(|s| s.soloed), "dispatch_route": "structural"}));
                fields.push(json!({"scope": "channel", "channel": channel, "field": "dim", "value": state.map(|s| s.dimmed), "dispatch_route": "structural"}));
            }
        }
        crate::PluginSettings::Matrix {
            input_channels,
            output_channels,
            matrix,
            channel_states,
        } => {
            for output in 0..*output_channels {
                for input in 0..*input_channels {
                    let value = matrix
                        .get(output * *input_channels + input)
                        .map_or(Value::Null, |gain| json!(gain));
                    fields.push(
                        json!({"scope": "crosspoint", "input": input, "output": output,
                        "field": "linear_gain", "value": value, "dispatch_route": "structural"}),
                    );
                }
                let state = channel_states.get(output);
                for (field, value) in [
                    ("mute", state.map(|s| s.muted)),
                    ("solo", state.map(|s| s.soloed)),
                    ("dim", state.map(|s| s.dimmed)),
                ] {
                    fields.push(json!({"scope": "output_channel", "channel": output,
                        "field": field, "value": value, "dispatch_route": "structural"}));
                }
            }
        }
        _ => {}
    }
    fields
}

fn param_type_name(pt: &sotf_plugins::param_specs::ParamType) -> &'static str {
    match pt {
        sotf_plugins::param_specs::ParamType::Float { .. } => "float",
        sotf_plugins::param_specs::ParamType::Int { .. } => "int",
        sotf_plugins::param_specs::ParamType::Bool { .. } => "bool",
        sotf_plugins::param_specs::ParamType::Choice { .. } => "choice",
        sotf_plugins::param_specs::ParamType::FilePath => "file",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_path_without_numeric_getter_is_an_error_instead_of_false_zero() {
        let settings = crate::PluginSettings::Convolution {
            ir_file: String::new(),
            mix: 1.0,
            gain_db: 0.0,
            use_nupc: true,
            zero_latency_head: false,
            head_taps: 256,
            true_stereo: false,
        };
        let error = resolve_param_path(&settings, "0.value").unwrap_err();
        assert!(error.to_string().contains("no numeric value getter"));
    }

    #[test]
    fn ab_config_descriptor_distinguishes_engine_config_from_source_file() {
        let mut settings =
            crate::PluginSettings::default_for(&crate::PluginType::ABCompare).unwrap();
        if let crate::PluginSettings::ABCompare {
            path_a_config,
            path_a_file,
            path_b_config,
            path_b_file,
            ..
        } = &mut settings
        {
            *path_a_config = r#"{"plugins":[{"type":"Gain"}]}"#.into();
            *path_a_file = "/tmp/a.json".into();
            *path_b_config = r#"{"plugins":[]}"#.into();
            *path_b_file = "/tmp/b.json".into();
        }
        let descriptors = query_param_descriptors(&settings).unwrap();
        assert_eq!(
            descriptors[14]["string_value"],
            json!(r#"{"plugins":[{"type":"Gain"}]}"#)
        );
        assert_eq!(descriptors[14]["source_path"], "/tmp/a.json");
        assert_eq!(descriptors[15]["string_value"], json!(r#"{"plugins":[]}"#));
        assert_eq!(descriptors[15]["source_path"], "/tmp/b.json");
        assert_eq!(descriptors[14]["value_kind"], "config_json");
    }

    #[test]
    fn dynamic_eq_sparse_band_ids_follow_the_ui_contract() {
        let mut settings = crate::PluginSettings::DynamicEq {
            num_bands: 1.0,
            threshold: -20.0,
            ratio: 2.0,
            attack: 10.0,
            release: 100.0,
            knee: 6.0,
            link_channels: false,
            mix: 1.0,
            bands: vec![sotf_plugins::DynEqBandParams {
                shape: Default::default(),
                placement: Default::default(),
                shelf_slope: 1.0,
                frequency: 1234.0,
                q: 1.5,
                gain: -3.0,
                band_threshold: -30.0,
                band_ratio: 4.0,
                active: true,
                solo: false,
            }],
            stereo_pairs: None,
        };
        let indices = query_param_indices(&settings);
        assert_eq!(indices.len(), 18);
        assert_eq!(
            &indices[8..],
            &[100, 101, 102, 103, 104, 105, 106, 107, 108, 109]
        );
        assert_eq!(
            resolve_param_path(&settings, "100.value").unwrap(),
            json!(1234.0)
        );
        assert_eq!(
            resolve_param_path(&settings, "104.name").unwrap(),
            json!("Ratio")
        );
        assert_eq!(
            resolve_param_path(&settings, "107.name").unwrap(),
            json!("Shape")
        );
        assert_eq!(
            resolve_param_path(&settings, "108.value").unwrap(),
            json!(1.0)
        );
        assert_eq!(
            resolve_param_path(&settings, "109.name").unwrap(),
            json!("Placement")
        );
        assert!(resolve_param_path(&settings, "8.value").is_err());
        if let crate::PluginSettings::DynamicEq {
            num_bands, bands, ..
        } = &mut settings
        {
            *num_bands = 2.0;
            let mut neighbor = bands[0].clone();
            neighbor.frequency = 4321.0;
            bands.push(neighbor);
        }
        let indices = query_param_indices(&settings);
        assert_eq!(indices.len(), 28);
        assert_eq!(
            &indices[18..],
            &[110, 111, 112, 113, 114, 115, 116, 117, 118, 119]
        );
        assert_eq!(
            resolve_param_path(&settings, "100.value").unwrap(),
            json!(1234.0)
        );
        assert_eq!(
            resolve_param_path(&settings, "110.value").unwrap(),
            json!(4321.0)
        );
        assert!(resolve_param_path(&settings, "120.value").is_err());
        if let crate::PluginSettings::DynamicEq { bands, .. } = &mut settings {
            bands.pop();
        }
        assert_eq!(query_param_indices(&settings).len(), 28);
        let descriptors = query_param_descriptors(&settings).unwrap();
        assert_eq!(descriptors.len(), 28);
        assert_eq!(descriptors[18]["value_kind"], "missing_getter");
    }

    #[test]
    fn multiband_compressor_sparse_ids_keep_neighbor_inheritance() {
        let mut settings =
            crate::PluginSettings::default_for(&crate::PluginType::MultibandCompressor).unwrap();
        let global_threshold = if let crate::PluginSettings::MultibandCompressor {
            num_bands,
            threshold_db,
            bands,
            ..
        } = &mut settings
        {
            *num_bands = 2;
            bands.resize_with(2, Default::default);
            bands[0].threshold_db = Some(-31.0);
            *threshold_db
        } else {
            panic!("wrong default plugin type")
        };
        let indices = query_param_indices(&settings);
        assert!(indices.contains(&106));
        assert!(indices.contains(&206));
        assert!(indices.contains(&118));
        assert!(indices.contains(&119));
        assert!(indices.contains(&218));
        assert!(indices.contains(&219));
        assert!(!indices.contains(&111));
        assert!(!indices.contains(&306));
        assert_eq!(
            resolve_param_path(&settings, "106.value").unwrap(),
            json!(-31.0)
        );
        assert_eq!(
            resolve_param_path(&settings, "206.value").unwrap(),
            json!(global_threshold)
        );
        let descriptors = query_param_descriptors(&settings).unwrap();
        assert_eq!(descriptors.len(), indices.len());
        let first = descriptors.iter().find(|d| d["index"] == 106).unwrap();
        assert_eq!(first["key"], "threshold");
        assert_eq!(first["dispatch_route"], "parameter");
        for (index, key, expected) in [
            (118, "range_db", json!(120.0)),
            (119, "hold_ms", json!(0.0)),
            (218, "range_db", json!(120.0)),
            (219, "hold_ms", json!(0.0)),
        ] {
            let descriptor = descriptors.iter().find(|d| d["index"] == index).unwrap();
            assert_eq!(descriptor["key"], key);
            assert_eq!(descriptor["dispatch_route"], "parameter");
            assert_eq!(
                resolve_param_path(&settings, &format!("{index}.value")).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn matrix_crosspoint_query_preserves_polarity_and_rejects_invalid_routes() {
        let settings = crate::PluginSettings::Matrix {
            input_channels: 2,
            output_channels: 2,
            matrix: vec![1.0, 0.0, 0.25, -0.5],
            channel_states: vec![sotf_plugins::ChannelState::default(); 2],
        };
        assert_eq!(
            resolve_matrix_crosspoint(&settings, "0.1.value").unwrap(),
            json!(0.0)
        );
        assert_eq!(
            resolve_matrix_crosspoint(&settings, "1.0.value").unwrap(),
            json!(0.25)
        );
        assert_eq!(
            resolve_matrix_crosspoint(&settings, "1.1.value").unwrap(),
            json!(-0.5)
        );
        for route in ["2.0.value", "0.2.value", "1.1.other", "1.1.value.extra"] {
            assert!(
                resolve_matrix_crosspoint(&settings, route).is_err(),
                "{route}"
            );
        }
        let mut incomplete = settings.clone();
        if let crate::PluginSettings::Matrix { matrix, .. } = &mut incomplete {
            matrix.clear();
        }
        assert!(resolve_matrix_crosspoint(&incomplete, "0.0.value").is_err());
    }

    #[test]
    fn matrix_scoped_fields_include_each_declared_cell_and_output_flag() {
        let settings = crate::PluginSettings::Matrix {
            input_channels: 2,
            output_channels: 2,
            matrix: vec![1.0, 0.0, 0.25, 1.0],
            channel_states: vec![sotf_plugins::ChannelState::default(); 2],
        };
        let fields = query_scoped_fields(&settings, Some(2));
        assert_eq!(fields.len(), 10);
        assert!(fields.iter().any(|field| field["scope"] == "crosspoint"
            && field["input"] == 0
            && field["output"] == 1
            && field["value"] == json!(0.25)));
        assert!(fields.iter().any(|field| field["scope"] == "output_channel"
            && field["channel"] == 1
            && field["field"] == "dim"));
    }

    #[test]
    fn channel_mute_solo_reports_missing_expected_channel_state() {
        let settings = crate::PluginSettings::ChannelMuteSolo {
            enabled: true,
            dim_gain_db: -20.0,
            fade_ms: 5.0,
            channel_states: Vec::new(),
        };
        let fields = query_scoped_fields(&settings, Some(2));
        assert_eq!(fields.len(), 6);
        assert!(fields.iter().all(|field| field["value"].is_null()));
    }
}
