//! Business logic for managing A/B Compare plugin sub-racks.
//!
//! Pure functions that convert between `Vec<PluginInRack>` and JSON `PathConfig`,
//! and perform add/remove/move operations on the plugin list.

pub use sotf_plugins::plugin_ab_compare::{PathConfig, PluginInRack};

/// Allowed plugin types for A/B sub-racks.
///
/// Excludes plugins that duplicate the main host's mandatory rack
/// (LoudnessMonitor, Gain/ReplayGain, Matrix, LoudnessMonitor) and
/// infrastructure plugins (Limiter) that belong in the main chain.
/// The A/B plugin's built-in auto-gain handles level matching.
pub const ALLOWED_PLUGIN_TYPES: &[(&str, &str)] = &[
    ("eq", "EQ"),
    ("compressor", "Compressor"),
    ("gate", "Gate"),
    ("delay", "Delay"),
];

/// Parse a path config JSON string into a flat list of plugins.
/// Returns empty vec for `None`, single-element vec for `Plugin`, full vec for `Rack`.
/// Graph configs are not editable as a rack and return empty vec.
pub fn parse_path_config(json: &str) -> Vec<PluginInRack> {
    let config: PathConfig = match serde_json::from_str(json) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Failed to parse path config: {e}");
            return Vec::new();
        }
    };

    match config {
        PathConfig::None => Vec::new(),
        PathConfig::Plugin {
            plugin_type,
            parameters,
        } => vec![PluginInRack {
            plugin_type,
            parameters,
        }],
        PathConfig::Rack { plugins } => plugins,
        PathConfig::Graph { .. } => {
            log::warn!("Graph path configs cannot be edited as a rack");
            Vec::new()
        }
    }
}

/// Encode a list of plugins back into a PathConfig JSON string.
pub fn encode_path_config(plugins: &[PluginInRack]) -> String {
    let config = match plugins.len() {
        0 => PathConfig::None,
        _ => PathConfig::Rack {
            plugins: plugins.to_vec(),
        },
    };
    serde_json::to_string(&config).unwrap_or_else(|_| r#"{"type":"None"}"#.to_string())
}

/// Add a new plugin of the given type to the end of the rack.
pub fn add_path_plugin(plugins: &mut Vec<PluginInRack>, plugin_type: &str) {
    plugins.push(PluginInRack {
        plugin_type: plugin_type.to_string(),
        parameters: serde_json::json!({}),
    });
}

/// Remove a plugin at the given index.
pub fn remove_path_plugin(plugins: &mut Vec<PluginInRack>, index: usize) {
    if index < plugins.len() {
        plugins.remove(index);
    }
}

/// Move a plugin from one index to another.
pub fn move_path_plugin(plugins: &mut Vec<PluginInRack>, from: usize, to: usize) {
    if from >= plugins.len() || to >= plugins.len() || from == to {
        return;
    }
    let item = plugins.remove(from);
    plugins.insert(to, item);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_none() {
        let plugins = parse_path_config(r#"{"type":"None"}"#);
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_parse_single_plugin() {
        let plugins =
            parse_path_config(r#"{"type":"Plugin","plugin_type":"gain","parameters":{"gain_db":-3.0}}"#);
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].plugin_type, "gain");
    }

    #[test]
    fn test_parse_rack() {
        let json = r#"{"type":"Rack","plugins":[{"plugin_type":"gain","parameters":{"gain_db":-3.0}},{"plugin_type":"eq","parameters":{}}]}"#;
        let plugins = parse_path_config(json);
        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].plugin_type, "gain");
        assert_eq!(plugins[1].plugin_type, "eq");
    }

    #[test]
    fn test_round_trip() {
        let mut plugins = Vec::new();
        add_path_plugin(&mut plugins, "eq");
        add_path_plugin(&mut plugins, "gain");
        add_path_plugin(&mut plugins, "compressor");

        let json = encode_path_config(&plugins);
        let decoded = parse_path_config(&json);

        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0].plugin_type, "eq");
        assert_eq!(decoded[1].plugin_type, "gain");
        assert_eq!(decoded[2].plugin_type, "compressor");
    }

    #[test]
    fn test_encode_empty() {
        let json = encode_path_config(&[]);
        let config: PathConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(config, PathConfig::None));
    }

    #[test]
    fn test_remove() {
        let mut plugins = Vec::new();
        add_path_plugin(&mut plugins, "eq");
        add_path_plugin(&mut plugins, "gain");
        add_path_plugin(&mut plugins, "compressor");

        remove_path_plugin(&mut plugins, 1);
        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].plugin_type, "eq");
        assert_eq!(plugins[1].plugin_type, "compressor");
    }

    #[test]
    fn test_remove_out_of_bounds() {
        let mut plugins = Vec::new();
        add_path_plugin(&mut plugins, "eq");
        remove_path_plugin(&mut plugins, 5); // should not panic
        assert_eq!(plugins.len(), 1);
    }

    #[test]
    fn test_move_forward() {
        let mut plugins = Vec::new();
        add_path_plugin(&mut plugins, "eq");
        add_path_plugin(&mut plugins, "gain");
        add_path_plugin(&mut plugins, "compressor");

        move_path_plugin(&mut plugins, 0, 2);
        assert_eq!(plugins[0].plugin_type, "gain");
        assert_eq!(plugins[1].plugin_type, "compressor");
        assert_eq!(plugins[2].plugin_type, "eq");
    }

    #[test]
    fn test_move_backward() {
        let mut plugins = Vec::new();
        add_path_plugin(&mut plugins, "eq");
        add_path_plugin(&mut plugins, "gain");
        add_path_plugin(&mut plugins, "compressor");

        move_path_plugin(&mut plugins, 2, 0);
        assert_eq!(plugins[0].plugin_type, "compressor");
        assert_eq!(plugins[1].plugin_type, "eq");
        assert_eq!(plugins[2].plugin_type, "gain");
    }

    #[test]
    fn test_parse_invalid_json() {
        let plugins = parse_path_config("not json");
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_parse_graph_returns_empty() {
        let json = r#"{"type":"Graph","nodes":[],"edges":[]}"#;
        let plugins = parse_path_config(json);
        assert!(plugins.is_empty());
    }
}
