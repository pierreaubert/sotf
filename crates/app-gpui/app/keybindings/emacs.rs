use crate::app::actions;
use gpui::KeyBinding;

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
        KeyBinding::new("ctrl-h", actions::ToggleHelp, Some("PlayerView")),
        KeyBinding::new("ctrl-shift-h", actions::ToggleHelpSupport, None),
        KeyBinding::new("F1", actions::ToggleScreenGuide, None),
        // Sort and filter - Emacs style (alt combos are fine)
        KeyBinding::new("alt-s", actions::CycleSortOrder, None),
        KeyBinding::new("alt-c", actions::CycleChannelFilter, None),
        // Navigation - PlayerView context so text editing isn't intercepted
        KeyBinding::new("ctrl-p", actions::SelectPrev, Some("PlayerView")),
        KeyBinding::new("up", actions::SelectPrev, Some("PlayerView")),
        KeyBinding::new("ctrl-n", actions::SelectNext, Some("PlayerView")),
        KeyBinding::new("down", actions::SelectNext, Some("PlayerView")),
        KeyBinding::new("alt-v", actions::SelectPrevPage, None),
        KeyBinding::new("ctrl-v", actions::SelectNextPage, None),
        KeyBinding::new("pageup", actions::SelectPrevPage, Some("PlayerView")),
        KeyBinding::new("pagedown", actions::SelectNextPage, Some("PlayerView")),
        KeyBinding::new("alt-<", actions::PrevPage, None),
        KeyBinding::new("alt->", actions::NextPage, None),
        // Expand/collapse - PlayerView context so text editing isn't intercepted
        KeyBinding::new("ctrl-b", actions::ToggleExpand, Some("PlayerView")),
        KeyBinding::new("ctrl-f", actions::ToggleExpand, Some("PlayerView")),
        KeyBinding::new("left", actions::ToggleExpand, Some("PlayerView")),
        KeyBinding::new("right", actions::ToggleExpand, Some("PlayerView")),
        // Enter action - PlayerView context so text editing can use Enter
        KeyBinding::new("ctrl-m", actions::Enter, None),
        KeyBinding::new("ctrl-j", actions::Enter, None),
        // Remove/delete - PlayerView context so text editing isn't intercepted
        KeyBinding::new("ctrl-d", actions::RemoveItem, Some("PlayerView")),
        KeyBinding::new("ctrl-k", actions::RemoveItem, Some("PlayerView")),
        KeyBinding::new("delete", actions::RemoveItem, Some("PlayerView")),
        // Plugin controls - alt combos stay global
        KeyBinding::new("alt-up", actions::MovePluginUp, None),
        KeyBinding::new("alt-down", actions::MovePluginDown, None),
        // Directory management - Emacs style (multi-key with ctrl-x stay global)
        KeyBinding::new("ctrl-x d", actions::AddDirectory, None),
        KeyBinding::new("ctrl-x s", actions::ScanLibrary, None),
        KeyBinding::new("alt-x S", actions::SwitchToSettings, None),
        // Level meter controls - alt combos stay global
        KeyBinding::new(
            "ctrl-tab",
            actions::SelectNextMeterGroup,
            Some("PlayerView"),
        ),
        KeyBinding::new(
            "ctrl-shift-tab",
            actions::SelectPrevMeterGroup,
            Some("PlayerView"),
        ),
        KeyBinding::new("alt-m", actions::ToggleMeterMute, None),
        KeyBinding::new("alt-M", actions::ToggleMeterSolo, None),
        KeyBinding::new("ctrl-alt-m", actions::ToggleMeterDim, None),
        KeyBinding::new("ctrl-g", actions::ClearMeterMutesSolos, None),
    ]
}
