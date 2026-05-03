use gpui::KeyBinding;

use crate::{
    DocumentedKeybinding, KeyConflict, KeybindingProvider, KeymapPreset, detect_conflicts,
};

/// Collects keybindings from multiple providers and aggregates them.
///
/// Register providers for different parts of your application, then
/// retrieve the combined bindings for any preset.
///
/// # Example
///
/// ```ignore
/// let mut registry = KeybindingRegistry::new();
/// registry.register(MyAppBindings);
/// registry.register(PluginBindings);
///
/// let bindings = registry.get_bindings(KeymapPreset::Vim);
/// cx.bind_keys(bindings);
/// ```
pub struct KeybindingRegistry {
    providers: Vec<Box<dyn KeybindingProvider>>,
}

impl KeybindingRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a keybinding provider.
    pub fn register(&mut self, provider: impl KeybindingProvider + 'static) {
        self.providers.push(Box::new(provider));
    }

    /// Get all GPUI `KeyBinding`s for a preset, collected from all providers.
    pub fn get_bindings(&self, preset: KeymapPreset) -> Vec<KeyBinding> {
        let mut bindings = Vec::new();
        for provider in &self.providers {
            bindings.extend(provider.bindings(preset));
        }
        bindings
    }

    /// Get all documented keybindings for help/settings UI.
    pub fn get_documented(&self, preset: KeymapPreset) -> Vec<DocumentedKeybinding> {
        let mut docs = Vec::new();
        for provider in &self.providers {
            docs.extend(provider.documented_bindings(preset));
        }
        docs
    }

    /// Detect conflicting keybindings (same display key) within a preset.
    pub fn detect_conflicts(&self, preset: KeymapPreset) -> Vec<KeyConflict> {
        let docs = self.get_documented(preset);
        detect_conflicts(&docs)
    }
}

impl Default for KeybindingRegistry {
    fn default() -> Self {
        Self::new()
    }
}
