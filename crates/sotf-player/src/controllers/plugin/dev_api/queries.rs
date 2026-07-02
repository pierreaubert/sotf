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
    let specs = settings.param_specs();
    let spec = specs
        .get(idx)
        .ok_or_else(|| anyhow!("param index out of range"))?;

    Ok(match tail {
        "name" => json!(spec.name),
        "value" => json!(settings.param_value(idx).unwrap_or(0.0)),
        "type" => json!(param_type_name(&spec.param_type)),
        "min" => json!(spec.min_f64()),
        "max" => json!(spec.max_f64()),
        "choice_count" => json!(spec.choice_labels().len()),
        other => return Err(anyhow!("unknown param property: `{other}`")),
    })
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
