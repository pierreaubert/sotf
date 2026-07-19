//! Keybindings configuration module.
//!
//! Provides configurable keymaps with preset support for different editing styles:
//! - Default: Custom keybindings optimized for audio player
//! - Vim: Vim-style navigation (hjkl, etc.)
//! - Emacs: Emacs-style navigation (C-n, C-p, etc.)
//! - VSCode: VSCode-style shortcuts

mod catalog;
mod common;
mod emacs;
pub(crate) mod listening_test;
mod plugin_graph;
mod plugins;
mod vim;
mod volume;
mod vscode;

use crate::app::actions;
use gpui::*;

use common::{common_bindings, plugin_rack_bindings};
use emacs::emacs_bindings;
use listening_test::listening_test_bindings;
use plugin_graph::plugin_graph_bindings;
use plugins::plugin_control_bindings;
use vim::vim_bindings;
use volume::volume_control_bindings;
use vscode::vscode_bindings;

// Re-export core types from gpui-keybinding
pub use gpui_keybinding::KeymapPreset;

// App-specific keybinding category — wraps the generic one with audio-player-specific categories.
// We keep this local because the categories (Playback, Queue, Plugins, LevelMeters, ScreenSwitch)
// are specific to the audio player, not generic enough for the framework crate.
/// Keybinding category for help display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeybindingCategory {
    Playback,
    Navigation,
    ScreenSwitch,
    Library,
    Queue,
    Plugins,
    ListeningTests,
    LevelMeters,
    System,
}

impl KeybindingCategory {
    pub fn name(&self) -> &'static str {
        match self {
            KeybindingCategory::Playback => "Playback",
            KeybindingCategory::Navigation => "Navigation",
            KeybindingCategory::ScreenSwitch => "Screen Switching",
            KeybindingCategory::Library => "Library",
            KeybindingCategory::Queue => "Queue",
            KeybindingCategory::Plugins => "Plugins",
            KeybindingCategory::ListeningTests => "Listening Tests",
            KeybindingCategory::LevelMeters => "Level Meters",
            KeybindingCategory::System => "System",
        }
    }

    pub fn all() -> &'static [KeybindingCategory] {
        &[
            KeybindingCategory::Playback,
            KeybindingCategory::Navigation,
            KeybindingCategory::ScreenSwitch,
            KeybindingCategory::Library,
            KeybindingCategory::Queue,
            KeybindingCategory::Plugins,
            KeybindingCategory::ListeningTests,
            KeybindingCategory::LevelMeters,
            KeybindingCategory::System,
        ]
    }
}

/// A documented keybinding for help display
#[derive(Debug, Clone)]
pub struct DocumentedKeybinding {
    pub key: String,
    pub raw_key_spec: String,
    pub description: &'static str,
    pub category: KeybindingCategory,
    pub action_name: Option<&'static str>,
}

/// One localized, executable command-palette row derived from a documented
/// runtime keybinding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPaletteCommand {
    pub action_name: &'static str,
    pub key: String,
    pub description: String,
    pub category: String,
}

/// Get all keybindings for a given preset
pub fn get_keybindings(preset: KeymapPreset) -> Vec<KeyBinding> {
    let mut bindings = Vec::new();

    // Common bindings shared across all presets
    bindings.extend(common_bindings());

    // Preset-specific bindings
    match preset {
        KeymapPreset::Default => bindings.extend(default_bindings()),
        KeymapPreset::Vim => bindings.extend(vim_bindings()),
        KeymapPreset::Emacs => bindings.extend(emacs_bindings()),
        KeymapPreset::VSCode => bindings.extend(vscode_bindings()),
    }

    bindings.push(command_palette_binding(preset));

    // These overlap general navigation and volume keys. Register them after
    // preset bindings so the more specific PluginRack context wins in Studio.
    bindings.extend(plugin_rack_bindings());
    bindings.extend(plugin_graph_bindings());

    // Volume control context bindings (always active when volume control is focused)
    bindings.extend(volume_control_bindings());

    // Plugin control context bindings (active when a plugin parameter control is focused)
    bindings.extend(plugin_control_bindings());
    bindings.extend(listening_test_bindings());

    bindings
}

fn command_palette_binding(preset: KeymapPreset) -> KeyBinding {
    let key = match preset {
        KeymapPreset::Emacs => "ctrl-x p",
        KeymapPreset::Default | KeymapPreset::Vim | KeymapPreset::VSCode => "secondary-k",
    };
    KeyBinding::new(key, actions::ToggleCommandPalette, None)
}

/// Default preset - custom bindings optimized for the audio player
fn default_bindings() -> Vec<KeyBinding> {
    vec![
        // Track navigation - PlayerView context to allow typing in search
        KeyBinding::new("n", actions::NextTrack, Some("PlayerView")),
        KeyBinding::new(">", actions::NextTrack, Some("PlayerView")),
        KeyBinding::new("b", actions::PrevTrack, Some("PlayerView")),
        KeyBinding::new("<", actions::PrevTrack, Some("PlayerView")),
        // Volume - PlayerView context to allow typing +/- in search
        KeyBinding::new("+", actions::VolumeUp, Some("PlayerView")),
        KeyBinding::new("=", actions::VolumeUp, Some("PlayerView")),
        KeyBinding::new("-", actions::VolumeDown, Some("PlayerView")),
        KeyBinding::new("_", actions::VolumeDown, Some("PlayerView")),
        KeyBinding::new("ctrl-up", actions::VolumeUpSmall, None),
        KeyBinding::new("ctrl-down", actions::VolumeDownSmall, None),
        // Theme and language - PlayerView context to allow typing T in search
        KeyBinding::new("shift-t", actions::CycleTheme, Some("PlayerView")),
        KeyBinding::new("alt-l", actions::CycleLanguage, None),
        // Search and view toggles - "/" toggles search, so keep it working in PlayerView
        KeyBinding::new("/", actions::ToggleSearch, Some("PlayerView")),
        KeyBinding::new("t", actions::ToggleLibraryView, Some("PlayerView")),
        KeyBinding::new("?", actions::ToggleHelp, Some("PlayerView")),
        KeyBinding::new("shift-?", actions::ToggleHelpSupport, Some("PlayerView")),
        KeyBinding::new("f1", actions::ToggleScreenGuide, None),
        // Sort and filter cycling
        KeyBinding::new("s", actions::CycleSortOrder, Some("PlayerView")),
        KeyBinding::new("c", actions::CycleChannelFilter, Some("PlayerView")),
        // Navigation - PlayerView context so NumberInput/Input editing isn't intercepted
        KeyBinding::new("left", actions::SelectLeft, Some("PlayerView")),
        KeyBinding::new("right", actions::SelectRight, Some("PlayerView")),
        KeyBinding::new("up", actions::SelectUp, Some("PlayerView")),
        KeyBinding::new("down", actions::SelectDown, Some("PlayerView")),
        // Vim-style navigation alternatives (hjkl)
        KeyBinding::new("h", actions::SelectLeft, Some("PlayerView")),
        KeyBinding::new("l", actions::SelectRight, Some("PlayerView")),
        KeyBinding::new("k", actions::SelectUp, Some("PlayerView")),
        KeyBinding::new("j", actions::SelectDown, Some("PlayerView")),
        // Page navigation - PlayerView context so text editing isn't intercepted
        KeyBinding::new("pageup", actions::SelectPrevPage, Some("PlayerView")),
        KeyBinding::new("pagedown", actions::SelectNextPage, Some("PlayerView")),
        // Library pagination (Ctrl/Cmd for page switching) - keep global
        KeyBinding::new("ctrl-left", actions::PrevPage, None),
        KeyBinding::new("ctrl-right", actions::NextPage, None),
        KeyBinding::new("cmd-left", actions::PrevPage, None),
        KeyBinding::new("cmd-right", actions::NextPage, None),
        // Enter action - PlayerView context so text editing can use Enter
        KeyBinding::new("enter", actions::Enter, Some("PlayerView")),
        KeyBinding::new("a", actions::Enter, Some("PlayerView")),
        // Remove/delete
        KeyBinding::new("d", actions::RemoveItem, Some("PlayerView")),
        KeyBinding::new("delete", actions::RemoveItem, Some("PlayerView")),
        // Plugin controls - PlayerView context
        KeyBinding::new("u", actions::MovePluginUp, Some("PlayerView")),
        KeyBinding::new("shift-n", actions::MovePluginDown, Some("PlayerView")),
        // Directory management
        KeyBinding::new("shift-a", actions::AddDirectory, Some("PlayerView")),
        KeyBinding::new(
            "secondary-shift-s",
            actions::ScanLibrary,
            Some("PlayerView"),
        ),
        KeyBinding::new("S", actions::SwitchToSettings, Some("PlayerView")),
        // Level meter controls - PlayerView context so text editing isn't intercepted
        KeyBinding::new("tab", actions::SelectNextMeterGroup, Some("PlayerView")),
        KeyBinding::new(
            "shift-tab",
            actions::SelectPrevMeterGroup,
            Some("PlayerView"),
        ),
        KeyBinding::new("m", actions::ToggleMeterMute, Some("PlayerView")),
        KeyBinding::new("shift-m", actions::ToggleMeterSolo, Some("PlayerView")),
        KeyBinding::new("ctrl-m", actions::ToggleMeterDim, None),
        KeyBinding::new("x", actions::ClearMeterMutesSolos, Some("PlayerView")),
    ]
}

/// Get documented keybindings from the exact runtime registrations for the
/// selected preset. Semantic descriptions live in the catalog, while key
/// labels are generated from `KeyBinding::keystrokes()` so they cannot drift.
pub fn get_documented_keybindings(preset: KeymapPreset) -> Vec<DocumentedKeybinding> {
    let runtime_bindings = get_keybindings(preset);
    catalog::documented_keybindings_from_runtime(&runtime_bindings)
}

/// Get contextual help for one screen from the authoritative runtime
/// registrations. The screen catalog contributes descriptions and semantic
/// grouping only; key labels always come from `KeyBinding::keystrokes()`.
pub fn get_documented_keybindings_for_screen(
    screen: crate::app::Screen,
    preset: KeymapPreset,
) -> Vec<DocumentedKeybinding> {
    let runtime_bindings = get_keybindings(preset);
    catalog::documented_keybindings_for_screen_from_runtime(screen, &runtime_bindings)
}

/// Search executable command-palette rows using gpui-keybinding's discovery
/// backend. Copy and categories are localized before indexing, so queries work
/// in the language currently shown by the application.
pub fn search_command_palette_commands(
    preset: KeymapPreset,
    query: &str,
    action_text: impl Fn(&'static str) -> &'static str,
    category_text: impl Fn(KeybindingCategory) -> &'static str,
) -> Vec<CommandPaletteCommand> {
    use gpui_keybinding::{
        DocumentedKeybinding as ToolkitDocumentedKeybinding,
        KeybindingCategory as ToolkitKeybindingCategory, command_palette_entries,
        search_command_palette,
    };

    let source = get_documented_keybindings(preset)
        .into_iter()
        .filter_map(|binding| {
            let action_name = binding.action_name?;
            Some((
                action_name,
                binding.key,
                binding.raw_key_spec,
                action_text(binding.description).to_string(),
                category_text(binding.category).to_string(),
            ))
        })
        .collect::<Vec<_>>();

    let searchable = source
        .iter()
        .map(|(_, key, raw_key_spec, description, category)| {
            ToolkitDocumentedKeybinding::new(
                key.clone(),
                description.clone(),
                ToolkitKeybindingCategory::Custom(category.clone()),
            )
            .with_raw_key_spec(raw_key_spec.clone())
        })
        .collect::<Vec<_>>();
    let entries = command_palette_entries(&searchable);

    search_command_palette(&entries, query)
        .into_iter()
        .filter_map(|entry| {
            source
                .iter()
                .find(|(_, key, _, description, category)| {
                    *key == entry.key
                        && *description == entry.description
                        && category.as_str() == entry.category.name()
                })
                .map(
                    |(action_name, key, _, description, category)| CommandPaletteCommand {
                        action_name,
                        key: key.clone(),
                        description: description.clone(),
                        category: category.clone(),
                    },
                )
        })
        .collect()
}

/// Clone the exact action registered for a palette row. Dispatching this box
/// therefore reaches the same handler as pressing the documented shortcut.
pub fn command_palette_action(preset: KeymapPreset, action_name: &str) -> Option<Box<dyn Action>> {
    get_keybindings(preset)
        .into_iter()
        .find(|binding| binding.action().name() == action_name)
        .map(|binding| binding.action().boxed_clone())
}

/// Validate the authoritative runtime/documentation registry for one preset.
/// This is intentionally public so integration and release QA can enforce the
/// same conflict and missing-command rules used by the application.
pub fn validate_keybinding_registry(preset: KeymapPreset) -> Result<(), String> {
    use std::collections::BTreeMap;

    let runtime_bindings = get_keybindings(preset);
    let mut seen = BTreeMap::new();
    for binding in &runtime_bindings {
        let keys = binding
            .keystrokes()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        let context = format!("{:?}", binding.predicate());
        let action = binding.action().name();
        let identity = (context, keys);
        if let Some(previous) = seen.insert(identity.clone(), action) {
            return Err(format!(
                "{preset:?} registers both {previous} and {action} for {identity:?}"
            ));
        }
    }

    let missing = catalog::missing_documented_commands(&runtime_bindings);
    if !missing.is_empty() {
        return Err(format!(
            "{preset:?} is missing runtime commands documented as: {}",
            missing.join(", ")
        ));
    }
    Ok(())
}
