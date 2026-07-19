use crate::app::actions;
use gpui::KeyBinding;

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
        KeyBinding::new("secondary-f", actions::ToggleSearch, None),
        KeyBinding::new("/", actions::ToggleSearch, Some("PlayerView")),
        KeyBinding::new("ctrl-shift-e", actions::ToggleLibraryView, None),
        KeyBinding::new("ctrl-shift-?", actions::ToggleHelp, None),
        KeyBinding::new("ctrl-alt-?", actions::ToggleHelpSupport, None),
        KeyBinding::new("F1", actions::ToggleScreenGuide, None),
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
