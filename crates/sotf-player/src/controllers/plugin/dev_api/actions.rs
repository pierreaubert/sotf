//! Shared plugin dev-API mutation helper.

use crate::PluginType;
use crate::controllers::plugin::set::set_plugin_param_value;
use crate::plugin_graph::PluginGraph;
use anyhow::{Result, anyhow, bail};
use serde_json::Value;
use std::path::Path;

/// Execute a plugin-chain mutation action against `graph`.
///
/// Supported actions:
/// - `PluginAdd` -> insert a new plugin of type `plugin_type`
/// - `PluginRemove` -> remove plugin at `index`
/// - `PluginToggle` -> toggle enabled state of plugin at `index`
/// - `PluginMoveUp` / `PluginMoveDown` -> move plugin at `index`
/// - `PluginSetParam` -> set numeric parameter at `param_index`
/// - `PluginSetParamString` -> set string parameter at `param_index`
/// - `PluginChainSave` / `PluginChainLoad` -> save/load user plugin chain
pub fn plugin_action(graph: &mut PluginGraph, name: &str, payload: Option<Value>) -> Result<()> {
    match name {
        "PluginAdd" => {
            let plugin_type = payload_str(&payload, "plugin_type")?;
            let ty = PluginType::from_name(plugin_type)
                .ok_or_else(|| anyhow!("unknown plugin type `{plugin_type}`"))?;
            let idx = graph.user_plugin_insert_index();
            graph.insert_plugin(idx, &ty).map_err(|e| anyhow!(e))?;
            graph.update_channel_dependent_plugins();
            Ok(())
        }
        "PluginRemove" => {
            let idx = payload_u64(&payload, "index")? as usize;
            graph.remove_plugin_by_index(idx).map_err(|e| anyhow!(e))?;
            graph.update_channel_dependent_plugins();
            Ok(())
        }
        "PluginToggle" => {
            let idx = payload_u64(&payload, "index")? as usize;
            graph.toggle_plugin_by_index(idx).map_err(|e| anyhow!(e))?;
            graph.update_channel_dependent_plugins();
            Ok(())
        }
        "PluginMoveUp" => {
            let idx = payload_u64(&payload, "index")? as usize;
            if graph.can_move_up_by_index(idx) {
                graph.move_plugin(idx, idx - 1);
                graph.update_channel_dependent_plugins();
            }
            Ok(())
        }
        "PluginMoveDown" => {
            let idx = payload_u64(&payload, "index")? as usize;
            if graph.can_move_down_by_index(idx) {
                graph.move_plugin(idx, idx + 1);
                graph.update_channel_dependent_plugins();
            }
            Ok(())
        }
        "PluginSetParam" => {
            let idx = payload_u64(&payload, "index")? as usize;
            let param_idx = payload_u64(&payload, "param_index")? as usize;
            let value = payload_f64(&payload, "value")?;
            let plugin = graph
                .get_plugin_mut(idx)
                .ok_or_else(|| anyhow!("plugin index {idx} out of range"))?;
            let mut channel_count_changed = false;
            if !set_plugin_param_value(
                &mut plugin.settings,
                param_idx,
                value,
                &mut channel_count_changed,
            ) {
                bail!("failed to set plugin {idx} param {param_idx} to {value}");
            }
            if channel_count_changed {
                graph.update_channel_dependent_plugins();
            }
            Ok(())
        }
        "PluginSetParamString" => {
            let idx = payload_u64(&payload, "index")? as usize;
            let param_idx = payload_u64(&payload, "param_index")? as usize;
            let value = payload_str(&payload, "value")?.to_string();
            let plugin = graph
                .get_plugin_mut(idx)
                .ok_or_else(|| anyhow!("plugin index {idx} out of range"))?;
            set_string_param(&mut plugin.settings, param_idx, value)?;
            graph.update_channel_dependent_plugins();
            Ok(())
        }
        "PluginClear" => {
            graph.clear_user_plugins().map_err(|e| anyhow!(e))?;
            graph.update_channel_dependent_plugins();
            Ok(())
        }
        "PluginChainSave" => {
            let path = payload_str(&payload, "path")?;
            let path = Path::new(path);
            let dir = path
                .parent()
                .ok_or_else(|| anyhow!("path has no parent directory"))?;
            let file = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow!("invalid filename"))?;
            graph
                .save_to_file(dir, file)
                .map_err(|e| anyhow!(e.to_string()))?;
            Ok(())
        }
        "PluginChainLoad" => {
            let path = payload_str(&payload, "path")?;
            let path = Path::new(path);
            let dir = path
                .parent()
                .ok_or_else(|| anyhow!("path has no parent directory"))?;
            let file = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow!("invalid filename"))?;
            graph
                .load_from_file(dir, file)
                .map_err(|e| anyhow!(e.to_string()))?;
            graph.update_channel_dependent_plugins();
            Ok(())
        }
        other => Err(anyhow!("unknown plugin action `{other}`")),
    }
}

fn payload_str<'a>(payload: &'a Option<Value>, key: &str) -> Result<&'a str> {
    payload
        .as_ref()
        .and_then(|v| v.get(key))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("payload needs string `{key}`"))
}

fn payload_u64(payload: &Option<Value>, key: &str) -> Result<u64> {
    payload
        .as_ref()
        .and_then(|v| v.get(key))
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("payload needs u64 `{key}`"))
}

fn payload_f64(payload: &Option<Value>, key: &str) -> Result<f64> {
    payload
        .as_ref()
        .and_then(|v| v.get(key))
        .and_then(Value::as_f64)
        .ok_or_else(|| anyhow!("payload needs f64 `{key}`"))
}

/// Set a string-valued plugin parameter by index.
pub fn set_string_param(
    settings: &mut crate::PluginSettings,
    param_idx: usize,
    value: String,
) -> Result<()> {
    use crate::security::validate_plugin_file_path;
    use std::path::Path;

    match settings {
        crate::PluginSettings::ABCompare { path_a_config, .. } if param_idx == 9 => {
            *path_a_config = value;
        }
        crate::PluginSettings::ABCompare { path_b_config, .. } if param_idx == 10 => {
            *path_b_config = value;
        }
        crate::PluginSettings::Convolution { ir_file, .. } if param_idx == 0 => {
            if !value.is_empty() {
                validate_plugin_file_path(Path::new(&value))?;
            }
            *ir_file = value;
        }
        crate::PluginSettings::XTC { room_ir_file, .. } if param_idx == 16 => {
            if !value.is_empty() {
                validate_plugin_file_path(Path::new(&value))?;
            }
            *room_ir_file = if value.is_empty() { None } else { Some(value) };
        }
        crate::PluginSettings::BinauralDecoder { sofa_file, .. } if param_idx == 0 => {
            if !value.is_empty() {
                validate_plugin_file_path(Path::new(&value))?;
            }
            *sofa_file = value;
        }
        _ => {
            return Err(anyhow!(
                "param {param_idx} is not a string parameter for this plugin"
            ));
        }
    }
    Ok(())
}
