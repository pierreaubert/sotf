//! Keybindings configuration module.
//!
//! Provides configurable keymaps with preset support for different editing styles:
//! - Default: Custom keybindings optimized for audio player
//! - Vim: Vim-style navigation (hjkl, etc.)
//! - Emacs: Emacs-style navigation (C-n, C-p, etc.)
//! - VSCode: VSCode-style shortcuts

use crate::app::actions;
use gpui::*;
use serde::{Deserialize, Serialize};

/// Available keymap presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
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

    bindings
}

/// Bindings common to all presets (playback, screen switching, etc.)
fn common_bindings() -> Vec<KeyBinding> {
    vec![
        // Playback controls - universal
        KeyBinding::new("space", actions::PlayPause, None),
        // Screen navigation with Shift + letter
        KeyBinding::new("shift-l", actions::SwitchToLibrary, None),
        KeyBinding::new("shift-q", actions::SwitchToQueue, None),
        KeyBinding::new("shift-p", actions::SwitchToPlugins, None),
        KeyBinding::new("shift-o", actions::SwitchToDevices, None),
        KeyBinding::new("shift-d", actions::SwitchToDirectoryManager, None),
        KeyBinding::new("shift-r", actions::SwitchToRoomEQ, None),
        KeyBinding::new("shift-h", actions::SwitchToHeadphoneEQ, None),
        KeyBinding::new("R", actions::SwitchToRecording, None),
        // Screen navigation with Cmd + number (Show menu shortcuts)
        KeyBinding::new("cmd-`", actions::SwitchToLibrary, None), // cmd-§ on macOS
        KeyBinding::new("cmd-0", actions::SwitchToLibrary, None),
        KeyBinding::new("cmd-1", actions::SwitchToStudio, None),
        KeyBinding::new("cmd-2", actions::SwitchToPluginGraph, None),
        KeyBinding::new("cmd-3", actions::SwitchToRecording, None),
        KeyBinding::new("cmd-4", actions::SwitchToRoomEQ, None),
        KeyBinding::new("cmd-5", actions::SwitchToHeadphoneEQ, None),
        KeyBinding::new("cmd-6", actions::SwitchToSpinorma, None),
        // Menu bar actions (platform convention)
        KeyBinding::new("cmd-,", actions::OpenConfig, None),
        KeyBinding::new("cmd-q", actions::QuitApp, None),
        // Cancel/Escape is universal
        KeyBinding::new("escape", actions::Cancel, None),
        // Quick add plugins (Shift + number keys) - same across all
        KeyBinding::new("!", actions::QuickAddEQ, None),
        KeyBinding::new("@", actions::QuickAddUpmixer, None),
        KeyBinding::new("#", actions::QuickAddCompressor, None),
        KeyBinding::new("$", actions::QuickAddGate, None),
        KeyBinding::new("%", actions::QuickAddLimiter, None),
        KeyBinding::new("^", actions::QuickAddLoudness, None),
        KeyBinding::new("&", actions::QuickAddBinaural, None),
        // Direct sort selection (number keys)
        KeyBinding::new("1", actions::SetSortArtist, None),
        KeyBinding::new("2", actions::SetSortAlbum, None),
        KeyBinding::new("3", actions::SetSortTitle, None),
        KeyBinding::new("4", actions::SetSortYear, None),
        // Direct filter selection
        KeyBinding::new("5", actions::SetFilterAll, None),
        KeyBinding::new("6", actions::SetFilterMono, None),
        KeyBinding::new("7", actions::SetFilterStereo, None),
        KeyBinding::new("8", actions::SetFilterSurround, None),
        KeyBinding::new("9", actions::SetFilterSurround71, None),
        KeyBinding::new("0", actions::SetFilterSurroundPlus, None),
    ]
}

/// Default preset - custom bindings optimized for the audio player
fn default_bindings() -> Vec<KeyBinding> {
    vec![
        // Track navigation
        KeyBinding::new("n", actions::NextTrack, Some("PlayerView")),
        KeyBinding::new(">", actions::NextTrack, None),
        KeyBinding::new("b", actions::PrevTrack, Some("PlayerView")),
        KeyBinding::new("<", actions::PrevTrack, None),
        // Volume
        KeyBinding::new("+", actions::VolumeUp, None),
        KeyBinding::new("=", actions::VolumeUp, None),
        KeyBinding::new("-", actions::VolumeDown, None),
        KeyBinding::new("_", actions::VolumeDown, None),
        // Theme and language
        KeyBinding::new("shift-t", actions::CycleTheme, None),
        KeyBinding::new("T", actions::CycleTheme, None),
        KeyBinding::new("alt-l", actions::CycleLanguage, None),
        // Search and view toggles
        KeyBinding::new("/", actions::ToggleSearch, None),
        KeyBinding::new("t", actions::ToggleLibraryView, Some("PlayerView")),
        KeyBinding::new("?", actions::ToggleHelp, None),
        // Sort and filter cycling
        KeyBinding::new("s", actions::CycleSortOrder, Some("PlayerView")),
        KeyBinding::new("c", actions::CycleChannelFilter, Some("PlayerView")),
        // Navigation - arrow keys and hjkl for grid/list navigation
        KeyBinding::new("left", actions::SelectLeft, None),
        KeyBinding::new("right", actions::SelectRight, None),
        KeyBinding::new("up", actions::SelectUp, None),
        KeyBinding::new("down", actions::SelectDown, None),
        // Vim-style navigation alternatives (hjkl)
        KeyBinding::new("h", actions::SelectLeft, Some("PlayerView")),
        KeyBinding::new("l", actions::SelectRight, Some("PlayerView")),
        KeyBinding::new("k", actions::SelectUp, Some("PlayerView")),
        KeyBinding::new("j", actions::SelectDown, Some("PlayerView")),
        // Page navigation
        KeyBinding::new("pageup", actions::SelectPrevPage, None),
        KeyBinding::new("pagedown", actions::SelectNextPage, None),
        // Library pagination (Ctrl/Cmd for page switching)
        KeyBinding::new("ctrl-left", actions::PrevPage, None),
        KeyBinding::new("ctrl-right", actions::NextPage, None),
        KeyBinding::new("cmd-left", actions::PrevPage, None),
        KeyBinding::new("cmd-right", actions::NextPage, None),
        // Enter action
        KeyBinding::new("enter", actions::Enter, None),
        KeyBinding::new("a", actions::Enter, Some("PlayerView")),
        // Remove/delete
        KeyBinding::new("d", actions::RemoveItem, Some("PlayerView")),
        KeyBinding::new("delete", actions::RemoveItem, None),
        // Plugin controls
        KeyBinding::new("u", actions::MovePluginUp, Some("PlayerView")),
        KeyBinding::new("shift-n", actions::MovePluginDown, None),
        // Directory management
        KeyBinding::new("shift-a", actions::AddDirectory, None),
        KeyBinding::new("shift-s", actions::ScanLibrary, None),
        KeyBinding::new("S", actions::SwitchToSettings, None),
        // Level meter controls
        KeyBinding::new("tab", actions::SelectNextMeterGroup, None),
        KeyBinding::new("shift-tab", actions::SelectPrevMeterGroup, None),
        KeyBinding::new("m", actions::ToggleMeterMute, Some("PlayerView")),
        KeyBinding::new("shift-m", actions::ToggleMeterSolo, None),
        KeyBinding::new("ctrl-m", actions::ToggleMeterDim, None),
        KeyBinding::new("x", actions::ClearMeterMutesSolos, Some("PlayerView")),
    ]
}

/// Vim preset - hjkl navigation, familiar to Vim users
fn vim_bindings() -> Vec<KeyBinding> {
    vec![
        // Track navigation - Vim style
        KeyBinding::new("ctrl-n", actions::NextTrack, None),
        KeyBinding::new("]", actions::NextTrack, None),
        KeyBinding::new("ctrl-p", actions::PrevTrack, None),
        KeyBinding::new("[", actions::PrevTrack, None),
        // Volume - Vim style with leader-like patterns
        KeyBinding::new("+", actions::VolumeUp, None),
        KeyBinding::new("=", actions::VolumeUp, None),
        KeyBinding::new("-", actions::VolumeDown, None),
        KeyBinding::new("_", actions::VolumeDown, None),
        // Theme and language
        KeyBinding::new("g t", actions::CycleTheme, None),
        KeyBinding::new("g l", actions::CycleLanguage, None),
        // Search - Vim style
        KeyBinding::new("/", actions::ToggleSearch, None),
        KeyBinding::new("g v", actions::ToggleLibraryView, None),
        KeyBinding::new("?", actions::ToggleHelp, None),
        KeyBinding::new("F1", actions::ToggleHelp, None),
        // Sort and filter
        KeyBinding::new("o s", actions::CycleSortOrder, None),
        KeyBinding::new("o c", actions::CycleChannelFilter, None),
        // Navigation - pure Vim hjkl
        KeyBinding::new("k", actions::SelectPrev, Some("PlayerView")),
        KeyBinding::new("up", actions::SelectPrev, None),
        KeyBinding::new("j", actions::SelectNext, Some("PlayerView")),
        KeyBinding::new("down", actions::SelectNext, None),
        KeyBinding::new("ctrl-u", actions::SelectPrevPage, None),
        KeyBinding::new("ctrl-d", actions::SelectNextPage, None),
        KeyBinding::new("pageup", actions::SelectPrevPage, None),
        KeyBinding::new("pagedown", actions::SelectNextPage, None),
        KeyBinding::new("g g", actions::PrevPage, None),
        KeyBinding::new("G", actions::NextPage, None),
        // Expand/collapse - Vim fold style
        KeyBinding::new("h", actions::ToggleExpand, Some("PlayerView")),
        KeyBinding::new("l", actions::ToggleExpand, Some("PlayerView")),
        KeyBinding::new("left", actions::ToggleExpand, None),
        KeyBinding::new("right", actions::ToggleExpand, None),
        KeyBinding::new("z o", actions::ToggleExpand, None),
        KeyBinding::new("z c", actions::ToggleExpand, None),
        // Enter action
        KeyBinding::new("enter", actions::Enter, None),
        KeyBinding::new("o", actions::Enter, Some("PlayerView")),
        // Remove/delete - Vim style
        KeyBinding::new("d d", actions::RemoveItem, None),
        KeyBinding::new("x", actions::RemoveItem, Some("PlayerView")),
        KeyBinding::new("delete", actions::RemoveItem, None),
        // Plugin controls - Vim style
        KeyBinding::new("K", actions::MovePluginUp, None),
        KeyBinding::new("J", actions::MovePluginDown, None),
        // Directory management
        KeyBinding::new("g a", actions::AddDirectory, None),
        KeyBinding::new("g s", actions::ScanLibrary, None),
        KeyBinding::new("S", actions::SwitchToSettings, None),
        // Level meter controls
        KeyBinding::new("tab", actions::SelectNextMeterGroup, None),
        KeyBinding::new("shift-tab", actions::SelectPrevMeterGroup, None),
        KeyBinding::new("m", actions::ToggleMeterMute, Some("PlayerView")),
        KeyBinding::new("M", actions::ToggleMeterSolo, None),
        KeyBinding::new("ctrl-m", actions::ToggleMeterDim, None),
        KeyBinding::new("g x", actions::ClearMeterMutesSolos, None),
    ]
}

/// Emacs preset - Ctrl key combinations
fn emacs_bindings() -> Vec<KeyBinding> {
    vec![
        // Track navigation - Emacs style
        KeyBinding::new("alt-n", actions::NextTrack, None),
        KeyBinding::new(">", actions::NextTrack, None),
        KeyBinding::new("alt-p", actions::PrevTrack, None),
        KeyBinding::new("<", actions::PrevTrack, None),
        // Volume - Emacs style
        KeyBinding::new("ctrl-x +", actions::VolumeUp, None),
        KeyBinding::new("+", actions::VolumeUp, None),
        KeyBinding::new("ctrl-x -", actions::VolumeDown, None),
        KeyBinding::new("-", actions::VolumeDown, None),
        // Theme and language
        KeyBinding::new("alt-x t", actions::CycleTheme, None),
        KeyBinding::new("alt-x l", actions::CycleLanguage, None),
        // Search - Emacs style
        KeyBinding::new("ctrl-s", actions::ToggleSearch, None),
        KeyBinding::new("/", actions::ToggleSearch, None),
        KeyBinding::new("ctrl-x v", actions::ToggleLibraryView, None),
        KeyBinding::new("ctrl-h", actions::ToggleHelp, None),
        KeyBinding::new("F1", actions::ToggleHelp, None),
        // Sort and filter - Emacs style
        KeyBinding::new("alt-s", actions::CycleSortOrder, None),
        KeyBinding::new("alt-c", actions::CycleChannelFilter, None),
        // Navigation - Emacs C-n/C-p
        KeyBinding::new("ctrl-p", actions::SelectPrev, None),
        KeyBinding::new("up", actions::SelectPrev, None),
        KeyBinding::new("ctrl-n", actions::SelectNext, None),
        KeyBinding::new("down", actions::SelectNext, None),
        KeyBinding::new("alt-v", actions::SelectPrevPage, None),
        KeyBinding::new("ctrl-v", actions::SelectNextPage, None),
        KeyBinding::new("pageup", actions::SelectPrevPage, None),
        KeyBinding::new("pagedown", actions::SelectNextPage, None),
        KeyBinding::new("alt-<", actions::PrevPage, None),
        KeyBinding::new("alt->", actions::NextPage, None),
        // Expand/collapse - Emacs style
        KeyBinding::new("ctrl-b", actions::ToggleExpand, None),
        KeyBinding::new("ctrl-f", actions::ToggleExpand, None),
        KeyBinding::new("left", actions::ToggleExpand, None),
        KeyBinding::new("right", actions::ToggleExpand, None),
        // Enter action
        KeyBinding::new("enter", actions::Enter, None),
        KeyBinding::new("ctrl-m", actions::Enter, None),
        KeyBinding::new("ctrl-j", actions::Enter, None),
        // Remove/delete - Emacs style
        KeyBinding::new("ctrl-d", actions::RemoveItem, None),
        KeyBinding::new("ctrl-k", actions::RemoveItem, None),
        KeyBinding::new("delete", actions::RemoveItem, None),
        // Plugin controls
        KeyBinding::new("alt-up", actions::MovePluginUp, None),
        KeyBinding::new("alt-down", actions::MovePluginDown, None),
        // Directory management - Emacs style
        KeyBinding::new("ctrl-x d", actions::AddDirectory, None),
        KeyBinding::new("ctrl-x s", actions::ScanLibrary, None),
        KeyBinding::new("alt-x S", actions::SwitchToSettings, None),
        // Level meter controls
        KeyBinding::new("tab", actions::SelectNextMeterGroup, None),
        KeyBinding::new("shift-tab", actions::SelectPrevMeterGroup, None),
        KeyBinding::new("alt-m", actions::ToggleMeterMute, None),
        KeyBinding::new("alt-M", actions::ToggleMeterSolo, None),
        KeyBinding::new("ctrl-alt-m", actions::ToggleMeterDim, None),
        KeyBinding::new("ctrl-g", actions::ClearMeterMutesSolos, None),
    ]
}

/// VSCode preset - familiar to many developers
fn vscode_bindings() -> Vec<KeyBinding> {
    vec![
        // Track navigation - VSCode media style
        KeyBinding::new("ctrl-shift-n", actions::NextTrack, None),
        KeyBinding::new("alt-right", actions::NextTrack, None),
        KeyBinding::new("ctrl-shift-p", actions::PrevTrack, None),
        KeyBinding::new("alt-left", actions::PrevTrack, None),
        // Volume
        KeyBinding::new("ctrl-up", actions::VolumeUp, None),
        KeyBinding::new("+", actions::VolumeUp, None),
        KeyBinding::new("ctrl-down", actions::VolumeDown, None),
        KeyBinding::new("-", actions::VolumeDown, None),
        // Theme and language - VSCode style
        KeyBinding::new("ctrl-k ctrl-t", actions::CycleTheme, None),
        KeyBinding::new("ctrl-shift-l", actions::CycleLanguage, None),
        // Search - VSCode style
        KeyBinding::new("ctrl-f", actions::ToggleSearch, None),
        KeyBinding::new("cmd-f", actions::ToggleSearch, None),
        KeyBinding::new("/", actions::ToggleSearch, None),
        KeyBinding::new("ctrl-shift-e", actions::ToggleLibraryView, None),
        KeyBinding::new("ctrl-shift-?", actions::ToggleHelp, None),
        KeyBinding::new("F1", actions::ToggleHelp, None),
        // Sort and filter
        KeyBinding::new("ctrl-shift-s", actions::CycleSortOrder, None),
        KeyBinding::new("ctrl-shift-c", actions::CycleChannelFilter, None),
        // Navigation - VSCode/standard style
        KeyBinding::new("up", actions::SelectPrev, None),
        KeyBinding::new("down", actions::SelectNext, None),
        KeyBinding::new("pageup", actions::SelectPrevPage, None),
        KeyBinding::new("pagedown", actions::SelectNextPage, None),
        KeyBinding::new("ctrl-home", actions::PrevPage, None),
        KeyBinding::new("ctrl-end", actions::NextPage, None),
        KeyBinding::new("home", actions::PrevPage, None),
        KeyBinding::new("end", actions::NextPage, None),
        // Expand/collapse - VSCode explorer style
        KeyBinding::new("left", actions::ToggleExpand, None),
        KeyBinding::new("right", actions::ToggleExpand, None),
        KeyBinding::new("ctrl-shift-[", actions::ToggleExpand, None),
        KeyBinding::new("ctrl-shift-]", actions::ToggleExpand, None),
        // Enter action
        KeyBinding::new("enter", actions::Enter, None),
        KeyBinding::new("ctrl-enter", actions::Enter, None),
        // Remove/delete - VSCode style
        KeyBinding::new("delete", actions::RemoveItem, None),
        KeyBinding::new("ctrl-shift-k", actions::RemoveItem, None),
        KeyBinding::new("backspace", actions::RemoveItem, None),
        // Plugin controls - VSCode style
        KeyBinding::new("alt-up", actions::MovePluginUp, None),
        KeyBinding::new("alt-down", actions::MovePluginDown, None),
        // Directory management - VSCode style
        KeyBinding::new("ctrl-shift-a", actions::AddDirectory, None),
        KeyBinding::new("ctrl-shift-r", actions::ScanLibrary, None),
        KeyBinding::new("S", actions::SwitchToSettings, None),
        // Level meter controls
        KeyBinding::new("tab", actions::SelectNextMeterGroup, None),
        KeyBinding::new("shift-tab", actions::SelectPrevMeterGroup, None),
        KeyBinding::new("m", actions::ToggleMeterMute, Some("PlayerView")),
        KeyBinding::new("shift-m", actions::ToggleMeterSolo, None),
        KeyBinding::new("ctrl-m", actions::ToggleMeterDim, None),
        KeyBinding::new("ctrl-shift-m", actions::ClearMeterMutesSolos, None),
    ]
}

/// Keybinding category for help display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    ]
}

fn vim_documented_keybindings() -> Vec<DocumentedKeybinding> {
    vec![
        // Playback
        DocumentedKeybinding {
            key: "Space",
            description: "Play/Pause",
            category: KeybindingCategory::Playback,
        },
        DocumentedKeybinding {
            key: "Ctrl+N / ]",
            description: "Next track",
            category: KeybindingCategory::Playback,
        },
        DocumentedKeybinding {
            key: "Ctrl+P / [",
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
            key: "h / l",
            description: "Expand/collapse",
            category: KeybindingCategory::Navigation,
        },
        DocumentedKeybinding {
            key: "Ctrl+U / Ctrl+D",
            description: "Page up/down",
            category: KeybindingCategory::Navigation,
        },
        DocumentedKeybinding {
            key: "gg / G",
            description: "First/last page",
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
            key: "gv",
            description: "Toggle tree/list view",
            category: KeybindingCategory::Library,
        },
        DocumentedKeybinding {
            key: "os",
            description: "Cycle sort order",
            category: KeybindingCategory::Library,
        },
        DocumentedKeybinding {
            key: "oc",
            description: "Cycle channel filter",
            category: KeybindingCategory::Library,
        },
        DocumentedKeybinding {
            key: "o / Enter",
            description: "Add to queue",
            category: KeybindingCategory::Library,
        },
        // Queue
        DocumentedKeybinding {
            key: "dd / x",
            description: "Remove item",
            category: KeybindingCategory::Queue,
        },
        // Plugins
        DocumentedKeybinding {
            key: "K",
            description: "Move plugin up",
            category: KeybindingCategory::Plugins,
        },
        DocumentedKeybinding {
            key: "J",
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
            key: "M",
            description: "Toggle solo",
            category: KeybindingCategory::LevelMeters,
        },
        DocumentedKeybinding {
            key: "Ctrl+M",
            description: "Toggle dim",
            category: KeybindingCategory::LevelMeters,
        },
        DocumentedKeybinding {
            key: "gx",
            description: "Clear mutes/solos",
            category: KeybindingCategory::LevelMeters,
        },
        // System
        DocumentedKeybinding {
            key: "gt",
            description: "Cycle theme",
            category: KeybindingCategory::System,
        },
        DocumentedKeybinding {
            key: "gl",
            description: "Cycle language",
            category: KeybindingCategory::System,
        },
        DocumentedKeybinding {
            key: "? / F1",
            description: "Show help",
            category: KeybindingCategory::System,
        },
        DocumentedKeybinding {
            key: "Esc",
            description: "Cancel/close",
            category: KeybindingCategory::System,
        },
        DocumentedKeybinding {
            key: "Cmd+Q",
            description: "Quit",
            category: KeybindingCategory::System,
        },
    ]
}

fn emacs_documented_keybindings() -> Vec<DocumentedKeybinding> {
    vec![
        // Playback
        DocumentedKeybinding {
            key: "Space",
            description: "Play/Pause",
            category: KeybindingCategory::Playback,
        },
        DocumentedKeybinding {
            key: "Alt+N / >",
            description: "Next track",
            category: KeybindingCategory::Playback,
        },
        DocumentedKeybinding {
            key: "Alt+P / <",
            description: "Previous track",
            category: KeybindingCategory::Playback,
        },
        DocumentedKeybinding {
            key: "Ctrl+X + / +",
            description: "Volume up",
            category: KeybindingCategory::Playback,
        },
        DocumentedKeybinding {
            key: "Ctrl+X - / -",
            description: "Volume down",
            category: KeybindingCategory::Playback,
        },
        // Navigation
        DocumentedKeybinding {
            key: "Ctrl+N / ↓",
            description: "Select next",
            category: KeybindingCategory::Navigation,
        },
        DocumentedKeybinding {
            key: "Ctrl+P / ↑",
            description: "Select previous",
            category: KeybindingCategory::Navigation,
        },
        DocumentedKeybinding {
            key: "Ctrl+F / Ctrl+B",
            description: "Expand/collapse",
            category: KeybindingCategory::Navigation,
        },
        DocumentedKeybinding {
            key: "Alt+V / Ctrl+V",
            description: "Page up/down",
            category: KeybindingCategory::Navigation,
        },
        DocumentedKeybinding {
            key: "Alt+< / Alt+>",
            description: "First/last page",
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
        // Library
        DocumentedKeybinding {
            key: "Ctrl+S / /",
            description: "Search",
            category: KeybindingCategory::Library,
        },
        DocumentedKeybinding {
            key: "Ctrl+X V",
            description: "Toggle tree/list view",
            category: KeybindingCategory::Library,
        },
        DocumentedKeybinding {
            key: "Alt+S",
            description: "Cycle sort order",
            category: KeybindingCategory::Library,
        },
        DocumentedKeybinding {
            key: "Alt+C",
            description: "Cycle channel filter",
            category: KeybindingCategory::Library,
        },
        DocumentedKeybinding {
            key: "Ctrl+M / Ctrl+J",
            description: "Add to queue",
            category: KeybindingCategory::Library,
        },
        // Queue
        DocumentedKeybinding {
            key: "Ctrl+D / Ctrl+K",
            description: "Remove item",
            category: KeybindingCategory::Queue,
        },
        // Plugins
        DocumentedKeybinding {
            key: "Alt+↑",
            description: "Move plugin up",
            category: KeybindingCategory::Plugins,
        },
        DocumentedKeybinding {
            key: "Alt+↓",
            description: "Move plugin down",
            category: KeybindingCategory::Plugins,
        },
        // Level meters
        DocumentedKeybinding {
            key: "Tab / Shift+Tab",
            description: "Next/prev meter group",
            category: KeybindingCategory::LevelMeters,
        },
        DocumentedKeybinding {
            key: "Alt+M",
            description: "Toggle mute",
            category: KeybindingCategory::LevelMeters,
        },
        DocumentedKeybinding {
            key: "Alt+Shift+M",
            description: "Toggle solo",
            category: KeybindingCategory::LevelMeters,
        },
        DocumentedKeybinding {
            key: "Ctrl+Alt+M",
            description: "Toggle dim",
            category: KeybindingCategory::LevelMeters,
        },
        DocumentedKeybinding {
            key: "Ctrl+G",
            description: "Clear mutes/solos",
            category: KeybindingCategory::LevelMeters,
        },
        // System
        DocumentedKeybinding {
            key: "Alt+X T",
            description: "Cycle theme",
            category: KeybindingCategory::System,
        },
        DocumentedKeybinding {
            key: "Alt+X L",
            description: "Cycle language",
            category: KeybindingCategory::System,
        },
        DocumentedKeybinding {
            key: "Ctrl+H / F1",
            description: "Show help",
            category: KeybindingCategory::System,
        },
        DocumentedKeybinding {
            key: "Esc",
            description: "Cancel/close",
            category: KeybindingCategory::System,
        },
        DocumentedKeybinding {
            key: "Cmd+Q",
            description: "Quit",
            category: KeybindingCategory::System,
        },
    ]
}

fn vscode_documented_keybindings() -> Vec<DocumentedKeybinding> {
    vec![
        // Playback
        DocumentedKeybinding {
            key: "Space",
            description: "Play/Pause",
            category: KeybindingCategory::Playback,
        },
        DocumentedKeybinding {
            key: "Ctrl+Shift+N / Alt+→",
            description: "Next track",
            category: KeybindingCategory::Playback,
        },
        DocumentedKeybinding {
            key: "Ctrl+Shift+P / Alt+←",
            description: "Previous track",
            category: KeybindingCategory::Playback,
        },
        DocumentedKeybinding {
            key: "Ctrl+↑ / +",
            description: "Volume up",
            category: KeybindingCategory::Playback,
        },
        DocumentedKeybinding {
            key: "Ctrl+↓ / -",
            description: "Volume down",
            category: KeybindingCategory::Playback,
        },
        // Navigation
        DocumentedKeybinding {
            key: "↓",
            description: "Select next",
            category: KeybindingCategory::Navigation,
        },
        DocumentedKeybinding {
            key: "↑",
            description: "Select previous",
            category: KeybindingCategory::Navigation,
        },
        DocumentedKeybinding {
            key: "← / →",
            description: "Expand/collapse",
            category: KeybindingCategory::Navigation,
        },
        DocumentedKeybinding {
            key: "PgUp / PgDn",
            description: "Page up/down",
            category: KeybindingCategory::Navigation,
        },
        DocumentedKeybinding {
            key: "Home / End",
            description: "First/last page",
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
            key: "Ctrl+F / Cmd+F",
            description: "Search",
            category: KeybindingCategory::Library,
        },
        DocumentedKeybinding {
            key: "Ctrl+Shift+E",
            description: "Toggle tree/list view",
            category: KeybindingCategory::Library,
        },
        DocumentedKeybinding {
            key: "Ctrl+Shift+S",
            description: "Cycle sort order",
            category: KeybindingCategory::Library,
        },
        DocumentedKeybinding {
            key: "Ctrl+Shift+C",
            description: "Cycle channel filter",
            category: KeybindingCategory::Library,
        },
        DocumentedKeybinding {
            key: "Enter / Ctrl+Enter",
            description: "Add to queue",
            category: KeybindingCategory::Library,
        },
        // Queue
        DocumentedKeybinding {
            key: "Del / Backspace",
            description: "Remove item",
            category: KeybindingCategory::Queue,
        },
        // Plugins
        DocumentedKeybinding {
            key: "Alt+↑",
            description: "Move plugin up",
            category: KeybindingCategory::Plugins,
        },
        DocumentedKeybinding {
            key: "Alt+↓",
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
            key: "M",
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
            key: "Ctrl+Shift+M",
            description: "Clear mutes/solos",
            category: KeybindingCategory::LevelMeters,
        },
        // System
        DocumentedKeybinding {
            key: "Ctrl+K Ctrl+T",
            description: "Cycle theme",
            category: KeybindingCategory::System,
        },
        DocumentedKeybinding {
            key: "Ctrl+Shift+L",
            description: "Cycle language",
            category: KeybindingCategory::System,
        },
        DocumentedKeybinding {
            key: "F1",
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
    ]
}
