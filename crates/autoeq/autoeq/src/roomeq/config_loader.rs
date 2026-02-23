use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::roomeq::RoomConfig;

/// Keys that are shallow-merged (override individual fields within the object).
/// All other top-level keys are replaced entirely by the override value.
pub const SHALLOW_MERGE_KEYS: &[&str] = &["optimizer"];

/// Merge two JSON objects: for keys in `SHALLOW_MERGE_KEYS`, shallow-merge individual fields;
/// for all other keys, replace the base value entirely with the override value.
pub fn merge_json_objects(base: &mut serde_json::Value, overrides: &serde_json::Value) {
    if let (Some(base_obj), Some(override_obj)) = (base.as_object_mut(), overrides.as_object()) {
        for (key, override_value) in override_obj {
            if SHALLOW_MERGE_KEYS.contains(&key.as_str()) {
                // Shallow merge: override individual fields within the object
                if let (Some(base_inner), Some(override_inner)) = (
                    base_obj.get_mut(key).and_then(|v| v.as_object_mut()),
                    override_value.as_object(),
                ) {
                    for (k, v) in override_inner {
                        base_inner.insert(k.clone(), v.clone());
                    }
                } else {
                    base_obj.insert(key.clone(), override_value.clone());
                }
            } else {
                // Replace entirely (speakers, crossovers, etc.)
                base_obj.insert(key.clone(), override_value.clone());
            }
        }
    }
}

/// Load a room configuration from a base JSON file with optional overrides.
///
/// Reads the base config, applies override merging if provided, deserializes
/// into `RoomConfig`, and resolves relative paths against the config file's directory.
///
/// Returns the loaded config and the directory containing the base config file.
pub fn load_config(
    base_config_path: &Path,
    override_config_path: Option<&Path>,
) -> Result<(RoomConfig, PathBuf)> {
    let config_json = std::fs::read_to_string(base_config_path)
        .with_context(|| format!("Failed to read config: {:?}", base_config_path))?;

    let mut config_value: serde_json::Value =
        serde_json::from_str(&config_json).with_context(|| "Failed to parse config JSON")?;

    if let Some(override_path) = override_config_path {
        let override_json = std::fs::read_to_string(override_path)
            .with_context(|| format!("Failed to read override config: {:?}", override_path))?;
        let override_value: serde_json::Value = serde_json::from_str(&override_json)
            .with_context(|| "Failed to parse override config JSON")?;
        merge_json_objects(&mut config_value, &override_value);
    }

    let config_dir = base_config_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    let mut room_config: RoomConfig = serde_json::from_value(config_value)
        .with_context(|| "Failed to deserialize merged config into RoomConfig")?;

    room_config.resolve_paths(&config_dir);

    Ok((room_config, config_dir))
}
