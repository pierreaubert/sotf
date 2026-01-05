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

    bindings
}

/// Bindings common to all presets (playback, screen switching, etc.)
fn common_bindings() -> Vec<KeyBinding> {
    vec![
        // Playback controls - PlayerView context to allow typing space in search
        KeyBinding::new("space", actions::PlayPause, Some("PlayerView")),
        // Screen navigation with Shift + letter - PlayerView context to allow typing in search
        KeyBinding::new("shift-l", actions::SwitchToLibrary, Some("PlayerView")),
        KeyBinding::new("shift-q", actions::SwitchToQueue, Some("PlayerView")),
        KeyBinding::new("shift-p", actions::SwitchToPlugins, Some("PlayerView")),
        KeyBinding::new("shift-o", actions::SwitchToDevices, Some("PlayerView")),
        KeyBinding::new("shift-r", actions::SwitchToRoomEQ, Some("PlayerView")),
        KeyBinding::new("shift-h", actions::SwitchToHeadphoneEQ, Some("PlayerView")),
        KeyBinding::new("R", actions::SwitchToRecording, Some("PlayerView")),
        // Screen navigation with Cmd + number (Show menu shortcuts) - keep global
        KeyBinding::new("cmd-`", actions::SwitchToLibrary, None), // cmd-§ on macOS
        KeyBinding::new("cmd-0", actions::SwitchToLibrary, None),
        KeyBinding::new("cmd-1", actions::SwitchToStudio, None),
        KeyBinding::new("cmd-2", actions::SwitchToPluginGraph, None),
        KeyBinding::new("cmd-3", actions::SwitchToRecording, None),
        KeyBinding::new("cmd-4", actions::SwitchToRoomEQ, None),
        KeyBinding::new("cmd-5", actions::SwitchToHeadphoneEQ, None),
        KeyBinding::new("cmd-6", actions::SwitchToSpinorma, None),
        // Menu bar actions (platform convention) - keep global
        KeyBinding::new("cmd-,", actions::OpenConfig, None),
        KeyBinding::new("cmd-q", actions::QuitApp, None),
        // Cancel/Escape is universal - keep global
        KeyBinding::new("escape", actions::Cancel, None),
        // Quick add plugins (Shift + number keys) - PlayerView context
        KeyBinding::new("!", actions::QuickAddEQ, Some("PlayerView")),
        KeyBinding::new("@", actions::QuickAddUpmixer, Some("PlayerView")),
        KeyBinding::new("#", actions::QuickAddCompressor, Some("PlayerView")),
        KeyBinding::new("$", actions::QuickAddGate, Some("PlayerView")),
        KeyBinding::new("%", actions::QuickAddLimiter, Some("PlayerView")),
        KeyBinding::new("^", actions::QuickAddLoudness, Some("PlayerView")),
        KeyBinding::new("&", actions::QuickAddBinaural, Some("PlayerView")),
        // Direct sort selection (number keys) - PlayerView context to allow typing numbers in search
        KeyBinding::new("1", actions::SetSortArtist, Some("PlayerView")),
        KeyBinding::new("2", actions::SetSortAlbum, Some("PlayerView")),
        KeyBinding::new("3", actions::SetSortTitle, Some("PlayerView")),
        KeyBinding::new("4", actions::SetSortYear, Some("PlayerView")),
        // Direct filter selection - PlayerView context to allow typing numbers in search
        KeyBinding::new("5", actions::SetFilterAll, Some("PlayerView")),
        KeyBinding::new("6", actions::SetFilterMono, Some("PlayerView")),
        KeyBinding::new("7", actions::SetFilterStereo, Some("PlayerView")),
        KeyBinding::new("8", actions::SetFilterSurround, Some("PlayerView")),
        KeyBinding::new("9", actions::SetFilterSurround71, Some("PlayerView")),
        KeyBinding::new("0", actions::SetFilterSurroundPlus, Some("PlayerView")),
    ]
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
        // Navigation - arrow keys can remain global (useful for navigation even in search)
        KeyBinding::new("left", actions::SelectLeft, None),
        KeyBinding::new("right", actions::SelectRight, None),
        KeyBinding::new("up", actions::SelectUp, None),
        KeyBinding::new("down", actions::SelectDown, None),
        // Vim-style navigation alternatives (hjkl)
        KeyBinding::new("h", actions::SelectLeft, Some("PlayerView")),
        KeyBinding::new("l", actions::SelectRight, Some("PlayerView")),
        KeyBinding::new("k", actions::SelectUp, Some("PlayerView")),
        KeyBinding::new("j", actions::SelectDown, Some("PlayerView")),
        // Page navigation - keep global
        KeyBinding::new("pageup", actions::SelectPrevPage, None),
        KeyBinding::new("pagedown", actions::SelectNextPage, None),
        // Library pagination (Ctrl/Cmd for page switching) - keep global
        KeyBinding::new("ctrl-left", actions::PrevPage, None),
        KeyBinding::new("ctrl-right", actions::NextPage, None),
        KeyBinding::new("cmd-left", actions::PrevPage, None),
        KeyBinding::new("cmd-right", actions::NextPage, None),
        // Enter action - keep global but handler checks input mode
        KeyBinding::new("enter", actions::Enter, None),
        KeyBinding::new("a", actions::Enter, Some("PlayerView")),
        // Remove/delete
        KeyBinding::new("d", actions::RemoveItem, Some("PlayerView")),
        KeyBinding::new("delete", actions::RemoveItem, None),
        // Plugin controls - PlayerView context
        KeyBinding::new("u", actions::MovePluginUp, Some("PlayerView")),
        KeyBinding::new("shift-n", actions::MovePluginDown, Some("PlayerView")),
        // Directory management
        KeyBinding::new("shift-a", actions::AddDirectory, Some("PlayerView")),
        KeyBinding::new("shift-s", actions::ScanLibrary, Some("PlayerView")),
        KeyBinding::new("S", actions::SwitchToSettings, Some("PlayerView")),
        // Level meter controls - tab can stay global, letter keys need PlayerView
        KeyBinding::new("tab", actions::SelectNextMeterGroup, None),
        KeyBinding::new("shift-tab", actions::SelectPrevMeterGroup, None),
        KeyBinding::new("m", actions::ToggleMeterMute, Some("PlayerView")),
        KeyBinding::new("shift-m", actions::ToggleMeterSolo, Some("PlayerView")),
        KeyBinding::new("ctrl-m", actions::ToggleMeterDim, None),
        KeyBinding::new("x", actions::ClearMeterMutesSolos, Some("PlayerView")),
    ]
}

/// Vim preset - hjkl navigation, familiar to Vim users
fn vim_bindings() -> Vec<KeyBinding> {
    vec![
        // Track navigation - Vim style (ctrl combos stay global, single chars need PlayerView)
        KeyBinding::new("ctrl-n", actions::NextTrack, None),
        KeyBinding::new("]", actions::NextTrack, Some("PlayerView")),
        KeyBinding::new("ctrl-p", actions::PrevTrack, None),
        KeyBinding::new("[", actions::PrevTrack, Some("PlayerView")),
        // Volume - PlayerView context to allow typing in search
        KeyBinding::new("+", actions::VolumeUp, Some("PlayerView")),
        KeyBinding::new("=", actions::VolumeUp, Some("PlayerView")),
        KeyBinding::new("-", actions::VolumeDown, Some("PlayerView")),
        KeyBinding::new("_", actions::VolumeDown, Some("PlayerView")),
        // Theme and language - multi-key sequences need PlayerView
        KeyBinding::new("g t", actions::CycleTheme, Some("PlayerView")),
        KeyBinding::new("g l", actions::CycleLanguage, Some("PlayerView")),
        // Search - Vim style - PlayerView context
        KeyBinding::new("/", actions::ToggleSearch, Some("PlayerView")),
        KeyBinding::new("g v", actions::ToggleLibraryView, Some("PlayerView")),
        KeyBinding::new("?", actions::ToggleHelp, Some("PlayerView")),
        KeyBinding::new("shift-?", actions::ToggleHelpSupport, Some("PlayerView")),
        KeyBinding::new("F1", actions::ToggleHelp, None),
        // Sort and filter - multi-key sequences need PlayerView
        KeyBinding::new("o s", actions::CycleSortOrder, Some("PlayerView")),
        KeyBinding::new("o c", actions::CycleChannelFilter, Some("PlayerView")),
        // Navigation - pure Vim hjkl (arrow keys stay global)
        KeyBinding::new("k", actions::SelectPrev, Some("PlayerView")),
        KeyBinding::new("up", actions::SelectPrev, None),
        KeyBinding::new("j", actions::SelectNext, Some("PlayerView")),
        KeyBinding::new("down", actions::SelectNext, None),
        KeyBinding::new("ctrl-u", actions::SelectPrevPage, None),
        KeyBinding::new("ctrl-d", actions::SelectNextPage, None),
        KeyBinding::new("pageup", actions::SelectPrevPage, None),
        KeyBinding::new("pagedown", actions::SelectNextPage, None),
        KeyBinding::new("g g", actions::PrevPage, Some("PlayerView")),
        KeyBinding::new("G", actions::NextPage, Some("PlayerView")),
        // Expand/collapse - Vim fold style
        KeyBinding::new("h", actions::ToggleExpand, Some("PlayerView")),
        KeyBinding::new("l", actions::ToggleExpand, Some("PlayerView")),
        KeyBinding::new("left", actions::ToggleExpand, None),
        KeyBinding::new("right", actions::ToggleExpand, None),
        KeyBinding::new("z o", actions::ToggleExpand, Some("PlayerView")),
        KeyBinding::new("z c", actions::ToggleExpand, Some("PlayerView")),
        // Enter action - enter stays global but handler checks mode
        KeyBinding::new("enter", actions::Enter, None),
        KeyBinding::new("o", actions::Enter, Some("PlayerView")),
        // Remove/delete - Vim style
        KeyBinding::new("d d", actions::RemoveItem, Some("PlayerView")),
        KeyBinding::new("x", actions::RemoveItem, Some("PlayerView")),
        KeyBinding::new("delete", actions::RemoveItem, None),
        // Plugin controls - Vim style - PlayerView context
        KeyBinding::new("K", actions::MovePluginUp, Some("PlayerView")),
        KeyBinding::new("J", actions::MovePluginDown, Some("PlayerView")),
        // Directory management - multi-key sequences need PlayerView
        KeyBinding::new("g a", actions::AddDirectory, Some("PlayerView")),
        KeyBinding::new("g s", actions::ScanLibrary, Some("PlayerView")),
        KeyBinding::new("S", actions::SwitchToSettings, Some("PlayerView")),
        // Level meter controls - letter keys need PlayerView
        KeyBinding::new("tab", actions::SelectNextMeterGroup, None),
        KeyBinding::new("shift-tab", actions::SelectPrevMeterGroup, None),
        KeyBinding::new("m", actions::ToggleMeterMute, Some("PlayerView")),
        KeyBinding::new("M", actions::ToggleMeterSolo, Some("PlayerView")),
        KeyBinding::new("ctrl-m", actions::ToggleMeterDim, None),
        KeyBinding::new("g x", actions::ClearMeterMutesSolos, Some("PlayerView")),
    ]
}

/// Emacs preset - Ctrl key combinations
fn emacs_bindings() -> Vec<KeyBinding> {
    vec![
        // Track navigation - Emacs style (single chars need PlayerView)
        KeyBinding::new("alt-n", actions::NextTrack, None),
        KeyBinding::new(">", actions::NextTrack, Some("PlayerView")),
        KeyBinding::new("alt-p", actions::PrevTrack, None),
        KeyBinding::new("<", actions::PrevTrack, Some("PlayerView")),
        // Volume - Emacs style (single chars need PlayerView)
        KeyBinding::new("ctrl-x +", actions::VolumeUp, None),
        KeyBinding::new("+", actions::VolumeUp, Some("PlayerView")),
        KeyBinding::new("ctrl-x -", actions::VolumeDown, None),
        KeyBinding::new("-", actions::VolumeDown, Some("PlayerView")),
        // Theme and language - multi-key sequences stay global (need first key)
        KeyBinding::new("alt-x t", actions::CycleTheme, None),
        KeyBinding::new("alt-x l", actions::CycleLanguage, None),
        // Search - Emacs style (single / needs PlayerView)
        KeyBinding::new("ctrl-s", actions::ToggleSearch, None),
        KeyBinding::new("/", actions::ToggleSearch, Some("PlayerView")),
        KeyBinding::new("ctrl-x v", actions::ToggleLibraryView, None),
        KeyBinding::new("ctrl-h", actions::ToggleHelp, None),
        KeyBinding::new("ctrl-shift-h", actions::ToggleHelpSupport, None),
        KeyBinding::new("F1", actions::ToggleHelp, None),
        // Sort and filter - Emacs style (alt combos are fine)
        KeyBinding::new("alt-s", actions::CycleSortOrder, None),
        KeyBinding::new("alt-c", actions::CycleChannelFilter, None),
        // Navigation - Emacs C-n/C-p (ctrl combos stay global)
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
        // Expand/collapse - Emacs style (ctrl combos stay global)
        KeyBinding::new("ctrl-b", actions::ToggleExpand, None),
        KeyBinding::new("ctrl-f", actions::ToggleExpand, None),
        KeyBinding::new("left", actions::ToggleExpand, None),
        KeyBinding::new("right", actions::ToggleExpand, None),
        // Enter action - enter stays global, ctrl combos stay global
        KeyBinding::new("enter", actions::Enter, None),
        KeyBinding::new("ctrl-m", actions::Enter, None),
        KeyBinding::new("ctrl-j", actions::Enter, None),
        // Remove/delete - Emacs style (ctrl combos stay global)
        KeyBinding::new("ctrl-d", actions::RemoveItem, None),
        KeyBinding::new("ctrl-k", actions::RemoveItem, None),
        KeyBinding::new("delete", actions::RemoveItem, None),
        // Plugin controls - alt combos stay global
        KeyBinding::new("alt-up", actions::MovePluginUp, None),
        KeyBinding::new("alt-down", actions::MovePluginDown, None),
        // Directory management - Emacs style (multi-key with ctrl-x stay global)
        KeyBinding::new("ctrl-x d", actions::AddDirectory, None),
        KeyBinding::new("ctrl-x s", actions::ScanLibrary, None),
        KeyBinding::new("alt-x S", actions::SwitchToSettings, None),
        // Level meter controls - alt combos stay global
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
        // Track navigation - VSCode media style (modifier combos stay global)
        KeyBinding::new("ctrl-shift-n", actions::NextTrack, None),
        KeyBinding::new("alt-right", actions::NextTrack, None),
        KeyBinding::new("ctrl-shift-p", actions::PrevTrack, None),
        KeyBinding::new("alt-left", actions::PrevTrack, None),
        // Volume - single chars need PlayerView
        KeyBinding::new("ctrl-up", actions::VolumeUpSmall, None),
        KeyBinding::new("+", actions::VolumeUp, Some("PlayerView")),
        KeyBinding::new("ctrl-down", actions::VolumeDownSmall, None),
        KeyBinding::new("-", actions::VolumeDown, Some("PlayerView")),
        // Theme and language - VSCode style (ctrl combos stay global)
        KeyBinding::new("ctrl-k ctrl-t", actions::CycleTheme, None),
        KeyBinding::new("ctrl-shift-l", actions::CycleLanguage, None),
        // Search - VSCode style (/ needs PlayerView, ctrl-f/cmd-f stay global)
        KeyBinding::new("ctrl-f", actions::ToggleSearch, None),
        KeyBinding::new("cmd-f", actions::ToggleSearch, None),
        KeyBinding::new("/", actions::ToggleSearch, Some("PlayerView")),
        KeyBinding::new("ctrl-shift-e", actions::ToggleLibraryView, None),
        KeyBinding::new("ctrl-shift-?", actions::ToggleHelp, None),
        KeyBinding::new("ctrl-alt-?", actions::ToggleHelpSupport, None),
        KeyBinding::new("F1", actions::ToggleHelp, None),
        // Sort and filter (ctrl combos stay global)
        KeyBinding::new("ctrl-shift-s", actions::CycleSortOrder, None),
        KeyBinding::new("ctrl-shift-c", actions::CycleChannelFilter, None),
        // Navigation - VSCode/standard style (arrow keys stay global)
        KeyBinding::new("up", actions::SelectPrev, None),
        KeyBinding::new("down", actions::SelectNext, None),
        KeyBinding::new("pageup", actions::SelectPrevPage, None),
        KeyBinding::new("pagedown", actions::SelectNextPage, None),
        KeyBinding::new("ctrl-home", actions::PrevPage, None),
        KeyBinding::new("ctrl-end", actions::NextPage, None),
        KeyBinding::new("home", actions::PrevPage, None),
        KeyBinding::new("end", actions::NextPage, None),
        // Expand/collapse - VSCode explorer style (arrow keys stay global)
        KeyBinding::new("left", actions::ToggleExpand, None),
        KeyBinding::new("right", actions::ToggleExpand, None),
        KeyBinding::new("ctrl-shift-[", actions::ToggleExpand, None),
        KeyBinding::new("ctrl-shift-]", actions::ToggleExpand, None),
        // Enter action - stays global but handler checks mode
        KeyBinding::new("enter", actions::Enter, None),
        KeyBinding::new("ctrl-enter", actions::Enter, None),
        // Remove/delete - VSCode style (backspace needs PlayerView to allow text editing!)
        KeyBinding::new("delete", actions::RemoveItem, None),
        KeyBinding::new("ctrl-shift-k", actions::RemoveItem, None),
        KeyBinding::new("backspace", actions::RemoveItem, Some("PlayerView")),
        // Plugin controls - VSCode style (alt combos stay global)
        KeyBinding::new("alt-up", actions::MovePluginUp, None),
        KeyBinding::new("alt-down", actions::MovePluginDown, None),
        // Directory management - VSCode style (single S needs PlayerView)
        KeyBinding::new("ctrl-shift-a", actions::AddDirectory, None),
        KeyBinding::new("ctrl-shift-r", actions::ScanLibrary, None),
        KeyBinding::new("S", actions::SwitchToSettings, Some("PlayerView")),
        // Level meter controls - single letter keys need PlayerView
        KeyBinding::new("tab", actions::SelectNextMeterGroup, None),
        KeyBinding::new("shift-tab", actions::SelectPrevMeterGroup, None),
        KeyBinding::new("m", actions::ToggleMeterMute, Some("PlayerView")),
        KeyBinding::new("shift-m", actions::ToggleMeterSolo, Some("PlayerView")),
        KeyBinding::new("ctrl-m", actions::ToggleMeterDim, None),
        KeyBinding::new("ctrl-shift-m", actions::ClearMeterMutesSolos, None),
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
