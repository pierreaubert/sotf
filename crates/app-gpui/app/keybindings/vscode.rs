use crate::app::actions;
use gpui::KeyBinding;

use super::{DocumentedKeybinding, KeybindingCategory};

/// VSCode preset - familiar to many developers
pub(super) fn vscode_bindings() -> Vec<KeyBinding> {
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
        // Navigation - PlayerView context so text editing isn't intercepted
        KeyBinding::new("up", actions::SelectPrev, Some("PlayerView")),
        KeyBinding::new("down", actions::SelectNext, Some("PlayerView")),
        KeyBinding::new("pageup", actions::SelectPrevPage, Some("PlayerView")),
        KeyBinding::new("pagedown", actions::SelectNextPage, Some("PlayerView")),
        KeyBinding::new("ctrl-home", actions::PrevPage, None),
        KeyBinding::new("ctrl-end", actions::NextPage, None),
        KeyBinding::new("home", actions::PrevPage, Some("PlayerView")),
        KeyBinding::new("end", actions::NextPage, Some("PlayerView")),
        // Expand/collapse - PlayerView context so text editing isn't intercepted
        KeyBinding::new("left", actions::ToggleExpand, Some("PlayerView")),
        KeyBinding::new("right", actions::ToggleExpand, Some("PlayerView")),
        KeyBinding::new("ctrl-shift-[", actions::ToggleExpand, None),
        KeyBinding::new("ctrl-shift-]", actions::ToggleExpand, None),
        // Enter action - PlayerView context so text editing can use Enter
        KeyBinding::new("enter", actions::Enter, Some("PlayerView")),
        KeyBinding::new("ctrl-enter", actions::Enter, None),
        // Remove/delete - PlayerView context so text editing isn't intercepted
        KeyBinding::new("delete", actions::RemoveItem, Some("PlayerView")),
        KeyBinding::new("ctrl-shift-k", actions::RemoveItem, None),
        KeyBinding::new("backspace", actions::RemoveItem, Some("PlayerView")),
        // Plugin controls - VSCode style (alt combos stay global)
        KeyBinding::new("alt-up", actions::MovePluginUp, None),
        KeyBinding::new("alt-down", actions::MovePluginDown, None),
        // Directory management - VSCode style (single S needs PlayerView)
        KeyBinding::new("ctrl-shift-a", actions::AddDirectory, None),
        KeyBinding::new("ctrl-shift-r", actions::ScanLibrary, None),
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
        KeyBinding::new("ctrl-shift-m", actions::ClearMeterMutesSolos, None),
    ]
}

pub(super) fn vscode_documented_keybindings() -> Vec<DocumentedKeybinding> {
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
