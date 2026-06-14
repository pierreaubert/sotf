use super::plugin_theme_id::PluginThemeId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Per-rack theme state.
///
/// `rack_theme` cascades to every plugin in the rack by default. Entries in
/// `overrides` (keyed by plugin index in the rack) replace that default for
/// that one plugin instance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RackThemeState {
    pub rack_theme: PluginThemeId,
    pub overrides: HashMap<usize, PluginThemeId>,
}

impl RackThemeState {
    /// Set the rack-level theme.
    pub fn set_rack_theme(&mut self, theme: PluginThemeId) {
        self.rack_theme = theme;
    }

    /// Pin `theme` to the plugin at `plugin_idx`, replacing the rack default
    /// for that instance.
    pub fn set_override(&mut self, plugin_idx: usize, theme: PluginThemeId) {
        self.overrides.insert(plugin_idx, theme);
    }

    /// Drop the override for `plugin_idx`, reverting to the rack theme.
    pub fn clear_override(&mut self, plugin_idx: usize) {
        self.overrides.remove(&plugin_idx);
    }

    /// Return the resolved theme id for `plugin_idx` (override if present,
    /// else rack default).
    pub fn resolved_id(&self, plugin_idx: usize) -> PluginThemeId {
        self.overrides
            .get(&plugin_idx)
            .copied()
            .unwrap_or(self.rack_theme)
    }

    /// Compact the override map after a plugin is removed at `removed_idx`.
    /// Entries for indices > removed_idx are shifted down by one. The
    /// removed entry itself is dropped.
    pub fn on_plugin_removed(&mut self, removed_idx: usize) {
        let mut compacted: HashMap<usize, PluginThemeId> = HashMap::new();
        for (idx, theme) in self.overrides.drain() {
            if idx == removed_idx {
                continue;
            }
            let new_idx = if idx > removed_idx { idx - 1 } else { idx };
            compacted.insert(new_idx, theme);
        }
        self.overrides = compacted;
    }

    /// Swap override entries for two plugin indices. Called when a plugin
    /// is reordered (move-up / move-down) so per-instance themes follow
    /// their plugin.
    pub fn swap_overrides(&mut self, a: usize, b: usize) {
        if a == b {
            return;
        }
        let ta = self.overrides.remove(&a);
        let tb = self.overrides.remove(&b);
        if let Some(t) = tb {
            self.overrides.insert(a, t);
        }
        if let Some(t) = ta {
            self.overrides.insert(b, t);
        }
    }
}
