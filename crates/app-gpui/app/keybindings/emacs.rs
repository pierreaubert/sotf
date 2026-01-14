use crate::app::actions;
use gpui::KeyBinding;

use super::{DocumentedKeybinding, KeybindingCategory};

/// Emacs preset - Ctrl key combinations
pub(super) fn emacs_bindings() -> Vec<KeyBinding> {
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

pub(super) fn emacs_documented_keybindings() -> Vec<DocumentedKeybinding> {
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
