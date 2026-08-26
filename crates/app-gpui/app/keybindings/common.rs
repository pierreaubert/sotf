use crate::app::actions;
use gpui::KeyBinding;

/// Bindings common to all presets (playback, screen switching, etc.)
pub(super) fn common_bindings() -> Vec<KeyBinding> {
    vec![
        // Font size controls (global) - Cmd/Ctrl + (or =), -, Shift+0 to reset
        KeyBinding::new("secondary-=", actions::IncreaseFontSize, None),
        KeyBinding::new("secondary-+", actions::IncreaseFontSize, None),
        KeyBinding::new("secondary--", actions::DecreaseFontSize, None),
        KeyBinding::new("secondary-shift-0", actions::ResetFontSize, None),
        // Screen navigation with Cmd/Ctrl + number (Show menu shortcuts) - keep global
        KeyBinding::new("secondary-0", actions::SwitchToLibrary, None),
        KeyBinding::new("secondary-`", actions::SwitchToLibrary, None), // cmd-§ on macOS
        KeyBinding::new("secondary-1", actions::SwitchToStudio, None),
        KeyBinding::new("secondary-2", actions::SwitchToPluginGraph, None),
        KeyBinding::new("secondary-3", actions::SwitchToRecording, None),
        KeyBinding::new("secondary-4", actions::SwitchToRoomEQ, None),
        KeyBinding::new("secondary-5", actions::SwitchToHeadphoneEQ, None),
        KeyBinding::new("secondary-6", actions::SwitchToSpinorama, None),
        // Documented, preset-independent screen jumps.
        KeyBinding::new("shift-l", actions::SwitchToLibrary, Some("PlayerView")),
        KeyBinding::new("shift-q", actions::SwitchToQueue, Some("PlayerView")),
        KeyBinding::new("shift-p", actions::SwitchToStudio, Some("PlayerView")),
        KeyBinding::new("shift-o", actions::SwitchToDevices, Some("PlayerView")),
        KeyBinding::new("shift-d", actions::SwitchToDirectories, Some("PlayerView")),
        // Shared domain-wizard navigation. Keep this in PlayerView so focused
        // text and number inputs retain their native Alt+Arrow behavior.
        KeyBinding::new(
            "alt-left",
            actions::PreviousWorkflowStep,
            Some("PlayerView"),
        ),
        KeyBinding::new("alt-right", actions::NextWorkflowStep, Some("PlayerView")),
        // Menu bar actions (platform convention) - keep global
        KeyBinding::new("secondary-,", actions::OpenConfig, None),
        KeyBinding::new("secondary-q", actions::QuitApp, None),
        // Cancel/Escape is universal - keep global
        KeyBinding::new("escape", actions::Cancel, None),
        // Media keys - global scope (work from anywhere)
        KeyBinding::new("f8", actions::PlayPause, None),
        KeyBinding::new("mediaplaypause", actions::PlayPause, None),
        KeyBinding::new("f9", actions::NextTrack, None),
        KeyBinding::new("medianexttrack", actions::NextTrack, None),
        KeyBinding::new("f7", actions::PrevTrack, None),
        KeyBinding::new("mediaprevioustrack", actions::PrevTrack, None),
        KeyBinding::new("mediastop", actions::Stop, None),
        KeyBinding::new("audiovolumeup", actions::VolumeUp, None),
        KeyBinding::new("audiovolumedown", actions::VolumeDown, None),
        KeyBinding::new("audiovolumemute", actions::ToggleMute, None),
        KeyBinding::new("f12", actions::VolumeUp, None),
        KeyBinding::new("f11", actions::VolumeDown, None),
        KeyBinding::new("f10", actions::ToggleMute, None),
        // Playback controls - PlayerView context to allow typing space in search
        KeyBinding::new("ctrl-space", actions::PlayPause, Some("PlayerView")),
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

/// Rack bindings are appended after preset bindings so the rack-specific
/// meaning wins when `PlayerView` and `PluginRack` contexts are both active.
pub(super) fn plugin_rack_bindings() -> Vec<KeyBinding> {
    vec![
        // Quick add plugins (Shift+number keys)
        KeyBinding::new("!", actions::QuickAddEQ, Some("PluginRack")),
        KeyBinding::new("@", actions::QuickAddUpmixer, Some("PluginRack")),
        KeyBinding::new("#", actions::QuickAddCompressor, Some("PluginRack")),
        KeyBinding::new("$", actions::QuickAddGate, Some("PluginRack")),
        KeyBinding::new("%", actions::QuickAddLimiter, Some("PluginRack")),
        KeyBinding::new("^", actions::QuickAddLoudness, Some("PluginRack")),
        KeyBinding::new("&", actions::QuickAddBinaural, Some("PluginRack")),
        KeyBinding::new("*", actions::QuickAddDownmix, Some("PluginRack")),
        KeyBinding::new("(", actions::QuickAddMonoToStereo, Some("PluginRack")),
        KeyBinding::new(")", actions::QuickAddSpectrum, Some("PluginRack")),
        // Plugin navigation
        KeyBinding::new("up", actions::SelectPrev, Some("PluginRack")),
        KeyBinding::new("down", actions::SelectNext, Some("PluginRack")),
        KeyBinding::new("left", actions::SelectLeft, Some("PluginRack")),
        KeyBinding::new("right", actions::SelectRight, Some("PluginRack")),
        // Plugin reordering
        KeyBinding::new("secondary-up", actions::MovePluginUp, Some("PluginRack")),
        KeyBinding::new(
            "secondary-down",
            actions::MovePluginDown,
            Some("PluginRack"),
        ),
        // Plugin enable/disable
        KeyBinding::new("enter", actions::TogglePlugin, Some("PluginRack")),
        // Plugin removal
        KeyBinding::new("backspace", actions::RemoveItem, Some("PluginRack")),
        KeyBinding::new("delete", actions::RemoveItem, Some("PluginRack")),
        // Plugin parameter adjustment
        KeyBinding::new("=", actions::IncrementPluginParam, Some("PluginRack")),
        KeyBinding::new("-", actions::DecrementPluginParam, Some("PluginRack")),
        KeyBinding::new(
            "shift-=",
            actions::IncrementPluginParamLarge,
            Some("PluginRack"),
        ),
        KeyBinding::new(
            "shift--",
            actions::DecrementPluginParamLarge,
            Some("PluginRack"),
        ),
        KeyBinding::new(
            "alt-=",
            actions::IncrementPluginParamSmall,
            Some("PluginRack"),
        ),
        KeyBinding::new(
            "alt--",
            actions::DecrementPluginParamSmall,
            Some("PluginRack"),
        ),
    ]
}
