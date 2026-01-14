use crate::app::actions;
use gpui::KeyBinding;

/// Bindings common to all presets (playback, screen switching, etc.)
pub(super) fn common_bindings() -> Vec<KeyBinding> {
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
        KeyBinding::new("cmd-6", actions::SwitchToSpinorama, None),
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

