use crate::app::actions;
use gpui::KeyBinding;

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
        KeyBinding::new("F1", actions::ToggleScreenGuide, None),
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
        KeyBinding::new("m", actions::ToggleMeterMute, Some("PlayerView")),
        KeyBinding::new("M", actions::ToggleMeterSolo, Some("PlayerView")),
        KeyBinding::new("ctrl-m", actions::ToggleMeterDim, None),
        KeyBinding::new("g x", actions::ClearMeterMutesSolos, Some("PlayerView")),
    ]
}
