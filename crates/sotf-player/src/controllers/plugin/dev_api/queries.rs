//! Shared plugin introspection query helper.

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
/// - `plugins.plugin.{index}.param.{i}.name|value|type|min|max|choice_count`
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
        other => {
            let prefix = "param.";
            let rest = other
                .strip_prefix(prefix)
                .ok_or_else(|| anyhow!("unknown plugin path: `{other}`"))?;
            resolve_param_path(&plugin.settings, rest)
        }
    }
}

fn resolve_param_path(settings: &crate::PluginSettings, rest: &str) -> Result<Value> {
    let (idx_str, tail) = rest
        .split_once('.')
        .ok_or_else(|| anyhow!("expected param.<i>.<prop>"))?;
    let idx: usize = idx_str.parse()?;
    if idx >= crate::get_param_count(settings) {
        return Err(anyhow!("param index out of range"));
    }
    let spec =
        query_param_spec(settings, idx).ok_or_else(|| anyhow!("param index out of range"))?;

    Ok(match tail {
        "name" => json!(spec.name),
        "value" => json!(query_param_value(settings, idx).unwrap_or(0.0)),
        "type" => json!(param_type_name(&spec.param_type)),
        "min" => json!(spec.min_f64()),
        "max" => json!(spec.max_f64()),
        "choice_count" => json!(spec.choice_labels().len()),
        other => return Err(anyhow!("unknown param property: `{other}`")),
    })
}

fn query_param_spec(
    settings: &crate::PluginSettings,
    idx: usize,
) -> Option<&'static sotf_plugins::param_specs::ParamSpec> {
    match settings {
        crate::PluginSettings::EQ { .. } => {
            sotf_plugins::param_specs::eq::BAND_TEMPLATE.get(idx % 4)
        }
        crate::PluginSettings::LinearPhaseEq { .. } => {
            sotf_plugins::param_specs::linear_phase_eq::BAND_TEMPLATE.get(idx % 5)
        }
        _ => settings.param_specs().get(idx),
    }
}

fn query_param_value(settings: &crate::PluginSettings, idx: usize) -> Option<f64> {
    match settings {
        crate::PluginSettings::EQ { filters, .. } => {
            let filter = filters.get(idx / 4)?;
            match idx % 4 {
                0 => Some(filter.frequency),
                1 => Some(filter.q),
                2 => Some(filter.gain_db),
                3 => super::super::eq_band_types(true)
                    .iter()
                    .position(|filter_type| *filter_type == filter.filter_type)
                    .map(|index| index as f64),
                _ => None,
            }
        }
        crate::PluginSettings::LinearPhaseEq { filters, .. } => {
            let filter = filters.get(idx / 5)?;
            match idx % 5 {
                0 => super::super::eq_band_types(false)
                    .iter()
                    .position(|filter_type| *filter_type == filter.filter_type)
                    .map(|index| index as f64),
                1 => Some(filter.frequency),
                2 => Some(filter.q),
                3 => Some(filter.gain_db),
                4 => Some(if filter.muted { 0.0 } else { 1.0 }),
                _ => None,
            }
        }
        _ => settings.param_value(idx),
    }
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
