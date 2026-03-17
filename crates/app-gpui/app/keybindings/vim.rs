use crate::app::actions;
use gpui::KeyBinding;

use super::{DocumentedKeybinding, KeybindingCategory};

/// Vim preset - hjkl navigation, familiar to Vim users
pub(super) fn vim_bindings() -> Vec<KeyBinding> {
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
        KeyBinding::new("up", actions::SelectPrev, Some("PlayerView")),
        KeyBinding::new("j", actions::SelectNext, Some("PlayerView")),
        KeyBinding::new("down", actions::SelectNext, Some("PlayerView")),
        KeyBinding::new("ctrl-u", actions::SelectPrevPage, None),
        KeyBinding::new("ctrl-d", actions::SelectNextPage, None),
        KeyBinding::new("pageup", actions::SelectPrevPage, Some("PlayerView")),
        KeyBinding::new("pagedown", actions::SelectNextPage, Some("PlayerView")),
        KeyBinding::new("g g", actions::PrevPage, Some("PlayerView")),
        KeyBinding::new("G", actions::NextPage, Some("PlayerView")),
        // Expand/collapse - Vim fold style
        KeyBinding::new("h", actions::ToggleExpand, Some("PlayerView")),
        KeyBinding::new("l", actions::ToggleExpand, Some("PlayerView")),
        KeyBinding::new("left", actions::ToggleExpand, Some("PlayerView")),
        KeyBinding::new("right", actions::ToggleExpand, Some("PlayerView")),
        KeyBinding::new("z o", actions::ToggleExpand, Some("PlayerView")),
        KeyBinding::new("z c", actions::ToggleExpand, Some("PlayerView")),
        // Enter action - enter stays global but handler checks mode
        KeyBinding::new("enter", actions::Enter, Some("PlayerView")),
        KeyBinding::new("o", actions::Enter, Some("PlayerView")),
        // Remove/delete - Vim style
        KeyBinding::new("d d", actions::RemoveItem, Some("PlayerView")),
        KeyBinding::new("x", actions::RemoveItem, Some("PlayerView")),
        KeyBinding::new("delete", actions::RemoveItem, Some("PlayerView")),
        // Plugin controls - Vim style - PlayerView context
        KeyBinding::new("K", actions::MovePluginUp, Some("PlayerView")),
        KeyBinding::new("J", actions::MovePluginDown, Some("PlayerView")),
        // Directory management - multi-key sequences need PlayerView
        KeyBinding::new("g a", actions::AddDirectory, Some("PlayerView")),
        KeyBinding::new("g s", actions::ScanLibrary, Some("PlayerView")),
        KeyBinding::new("S", actions::SwitchToSettings, Some("PlayerView")),
        // Level meter controls - letter keys need PlayerView
        KeyBinding::new("tab", actions::SelectNextMeterGroup, Some("PlayerView")),
        KeyBinding::new(
            "shift-tab",
            actions::SelectPrevMeterGroup,
            Some("PlayerView"),
        ),
        KeyBinding::new("m", actions::ToggleMeterMute, Some("PlayerView")),
        KeyBinding::new("M", actions::ToggleMeterSolo, Some("PlayerView")),
        KeyBinding::new("ctrl-m", actions::ToggleMeterDim, None),
        KeyBinding::new("g x", actions::ClearMeterMutesSolos, Some("PlayerView")),
    ]
}

pub(super) fn vim_documented_keybindings() -> Vec<DocumentedKeybinding> {
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
