//! Keybindings configuration module.
//!
//! Provides configurable keymaps with preset support for different editing styles:
//! - Default: Custom keybindings optimized for audio player
//! - Vim: Vim-style navigation (hjkl, etc.)
//! - Emacs: Emacs-style navigation (C-n, C-p, etc.)
//! - VSCode: VSCode-style shortcuts

mod common;
mod emacs;
mod plugins;
mod vim;
mod volume;
mod vscode;

use crate::app::actions;
use gpui::*;
use serde::{Deserialize, Serialize};

use common::common_bindings;
use emacs::{emacs_bindings, emacs_documented_keybindings};
use plugins::plugin_control_bindings;
use vim::{vim_bindings, vim_documented_keybindings};
use volume::volume_control_bindings;
use vscode::{vscode_bindings, vscode_documented_keybindings};

/// Available keymap presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum KeymapPreset {
    #[default]
    Default,
    Vim,
    Emacs,
    VSCode,
}

impl KeymapPreset {
    pub fn name(&self) -> &'static str {
        match self {
            KeymapPreset::Default => "Default",
            KeymapPreset::Vim => "Vim",
            KeymapPreset::Emacs => "Emacs",
            KeymapPreset::VSCode => "VSCode",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            KeymapPreset::Default => "Custom keybindings optimized for audio player",
            KeymapPreset::Vim => "Vim-style navigation with hjkl keys",
            KeymapPreset::Emacs => "Emacs-style navigation with Ctrl combinations",
            KeymapPreset::VSCode => "VSCode-style shortcuts familiar to many developers",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            KeymapPreset::Default => KeymapPreset::Vim,
            KeymapPreset::Vim => KeymapPreset::Emacs,
            KeymapPreset::Emacs => KeymapPreset::VSCode,
            KeymapPreset::VSCode => KeymapPreset::Default,
        }
    }

    pub fn all() -> &'static [KeymapPreset] {
        &[
            KeymapPreset::Default,
            KeymapPreset::Vim,
            KeymapPreset::Emacs,
            KeymapPreset::VSCode,
        ]
    }
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

    // Volume control context bindings (always active when volume control is focused)
    bindings.extend(volume_control_bindings());

    // Plugin control context bindings (active when a plugin parameter control is focused)
    bindings.extend(plugin_control_bindings());

    bindings
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
        KeyBinding::new("T", actions::CycleTheme, Some("PlayerView")),
        KeyBinding::new("alt-l", actions::CycleLanguage, None),
        // Search and view toggles - "/" toggles search, so keep it working in PlayerView
        KeyBinding::new("/", actions::ToggleSearch, Some("PlayerView")),
        KeyBinding::new("t", actions::ToggleLibraryView, Some("PlayerView")),
        KeyBinding::new("?", actions::ToggleHelp, Some("PlayerView")),
        KeyBinding::new("shift-?", actions::ToggleHelpSupport, Some("PlayerView")),
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
        KeyBinding::new("shift-s", actions::ScanLibrary, Some("PlayerView")),
        KeyBinding::new("S", actions::SwitchToSettings, Some("PlayerView")),
        // Level meter controls - PlayerView context so text editing isn't intercepted
        KeyBinding::new("tab", actions::SelectNextMeterGroup, Some("PlayerView")),
        KeyBinding::new("shift-tab", actions::SelectPrevMeterGroup, Some("PlayerView")),
        KeyBinding::new("m", actions::ToggleMeterMute, Some("PlayerView")),
        KeyBinding::new("shift-m", actions::ToggleMeterSolo, Some("PlayerView")),
        KeyBinding::new("ctrl-m", actions::ToggleMeterDim, None),
        KeyBinding::new("x", actions::ClearMeterMutesSolos, Some("PlayerView")),
    ]
}

/// Keybinding category for help display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeybindingCategory {
    Playback,
    Navigation,
    ScreenSwitch,
    Library,
    Queue,
    Plugins,
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
            KeybindingCategory::LevelMeters,
            KeybindingCategory::System,
        ]
    }
}

/// A documented keybinding for help display
#[derive(Debug, Clone)]
pub struct DocumentedKeybinding {
    pub key: &'static str,
    pub description: &'static str,
    pub category: KeybindingCategory,
}

/// Get documented keybindings for help display (preset-aware)
pub fn get_documented_keybindings(preset: KeymapPreset) -> Vec<DocumentedKeybinding> {
    match preset {
        KeymapPreset::Default => default_documented_keybindings(),
        KeymapPreset::Vim => vim_documented_keybindings(),
        KeymapPreset::Emacs => emacs_documented_keybindings(),
        KeymapPreset::VSCode => vscode_documented_keybindings(),
    }
}

fn default_documented_keybindings() -> Vec<DocumentedKeybinding> {
    vec![
        // Playback
        DocumentedKeybinding {
            key: "Space",
            description: "Play/Pause",
            category: KeybindingCategory::Playback,
        },
        DocumentedKeybinding {
            key: "n / >",
            description: "Next track",
            category: KeybindingCategory::Playback,
        },
        DocumentedKeybinding {
            key: "b / <",
            description: "Previous track",
            category: KeybindingCategory::Playback,
        },
        DocumentedKeybinding {
            key: "+ / =",
            description: "Volume up",
            category: KeybindingCategory::Playback,
        },
        DocumentedKeybinding {
            key: "- / _",
            description: "Volume down",
            category: KeybindingCategory::Playback,
        },
        // Navigation
        DocumentedKeybinding {
            key: "j / ↓",
            description: "Select next",
            category: KeybindingCategory::Navigation,
        },
        DocumentedKeybinding {
            key: "k / ↑",
            description: "Select previous",
            category: KeybindingCategory::Navigation,
        },
        DocumentedKeybinding {
            key: "h / l / ← →",
            description: "Expand/collapse",
            category: KeybindingCategory::Navigation,
        },
        DocumentedKeybinding {
            key: "PgUp/PgDn",
            description: "Page up/down",
            category: KeybindingCategory::Navigation,
        },
        DocumentedKeybinding {
            key: "Ctrl+←/→",
            description: "Previous/next page",
            category: KeybindingCategory::Navigation,
        },
        // Screen switching
        DocumentedKeybinding {
            key: "Shift+L",
            description: "Library",
            category: KeybindingCategory::ScreenSwitch,
        },
        DocumentedKeybinding {
            key: "Shift+Q",
            description: "Queue",
            category: KeybindingCategory::ScreenSwitch,
        },
        DocumentedKeybinding {
            key: "Shift+P",
            description: "Plugins",
            category: KeybindingCategory::ScreenSwitch,
        },
        DocumentedKeybinding {
            key: "Shift+O",
            description: "Devices",
            category: KeybindingCategory::ScreenSwitch,
        },
        DocumentedKeybinding {
            key: "Shift+D",
            description: "Directory Manager",
            category: KeybindingCategory::ScreenSwitch,
        },
        DocumentedKeybinding {
            key: "S",
            description: "Settings",
            category: KeybindingCategory::ScreenSwitch,
        },
        // Library
        DocumentedKeybinding {
            key: "/",
            description: "Search",
            category: KeybindingCategory::Library,
        },
        DocumentedKeybinding {
            key: "t",
            description: "Toggle tree/list view",
            category: KeybindingCategory::Library,
        },
        DocumentedKeybinding {
            key: "s",
            description: "Cycle sort order",
            category: KeybindingCategory::Library,
        },
        DocumentedKeybinding {
            key: "c",
            description: "Cycle channel filter",
            category: KeybindingCategory::Library,
        },
        DocumentedKeybinding {
            key: "1-4",
            description: "Set sort (Artist/Album/Title/Year)",
            category: KeybindingCategory::Library,
        },
        DocumentedKeybinding {
            key: "5-9",
            description: "Set filter (All/Mono/Stereo/Multi/Mixed)",
            category: KeybindingCategory::Library,
        },
        DocumentedKeybinding {
            key: "Enter / a",
            description: "Add to queue",
            category: KeybindingCategory::Library,
        },
        // Queue
        DocumentedKeybinding {
            key: "d / Del",
            description: "Remove item",
            category: KeybindingCategory::Queue,
        },
        // Plugins
        DocumentedKeybinding {
            key: "u",
            description: "Move plugin up",
            category: KeybindingCategory::Plugins,
        },
        DocumentedKeybinding {
            key: "Shift+N",
            description: "Move plugin down",
            category: KeybindingCategory::Plugins,
        },
        DocumentedKeybinding {
            key: "! @ # $ % ^ &",
            description: "Quick add plugins",
            category: KeybindingCategory::Plugins,
        },
        // Level meters
        DocumentedKeybinding {
            key: "Tab / Shift+Tab",
            description: "Next/prev meter group",
            category: KeybindingCategory::LevelMeters,
        },
        DocumentedKeybinding {
            key: "m",
            description: "Toggle mute",
            category: KeybindingCategory::LevelMeters,
        },
        DocumentedKeybinding {
            key: "Shift+M",
            description: "Toggle solo",
            category: KeybindingCategory::LevelMeters,
        },
        DocumentedKeybinding {
            key: "Ctrl+M",
            description: "Toggle dim",
            category: KeybindingCategory::LevelMeters,
        },
        DocumentedKeybinding {
            key: "x",
            description: "Clear mutes/solos",
            category: KeybindingCategory::LevelMeters,
        },
        // System
        DocumentedKeybinding {
            key: "Shift+T",
            description: "Cycle theme",
            category: KeybindingCategory::System,
        },
        DocumentedKeybinding {
            key: "Alt+L",
            description: "Cycle language",
            category: KeybindingCategory::System,
        },
        DocumentedKeybinding {
            key: "?",
            description: "Show help",
            category: KeybindingCategory::System,
        },
        DocumentedKeybinding {
            key: "Esc",
            description: "Cancel/close",
            category: KeybindingCategory::System,
        },
        DocumentedKeybinding {
            key: "Cmd+,",
            description: "Settings",
            category: KeybindingCategory::System,
        },
        DocumentedKeybinding {
            key: "Cmd+Q",
            description: "Quit",
            category: KeybindingCategory::System,
        },
        DocumentedKeybinding {
            key: "Cmd++",
            description: "Increase font size",
            category: KeybindingCategory::System,
        },
        DocumentedKeybinding {
            key: "Cmd+-",
            description: "Decrease font size",
            category: KeybindingCategory::System,
        },
        DocumentedKeybinding {
            key: "Cmd+Shift+0",
            description: "Reset font size",
            category: KeybindingCategory::System,
        },
    ]
}
