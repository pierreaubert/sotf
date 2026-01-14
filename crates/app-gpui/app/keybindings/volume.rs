use crate::app::actions;
use gpui::KeyBinding;

/// Bindings for the volume control context
pub(super) fn volume_control_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("up", actions::VolumeUp, Some("volume-control")),
        KeyBinding::new("right", actions::VolumeUp, Some("volume-control")),
        KeyBinding::new("down", actions::VolumeDown, Some("volume-control")),
        KeyBinding::new("left", actions::VolumeDown, Some("volume-control")),
        KeyBinding::new("=", actions::VolumeUp, Some("volume-control")),
        KeyBinding::new("+", actions::VolumeUp, Some("volume-control")),
        KeyBinding::new("-", actions::VolumeDown, Some("volume-control")),
        KeyBinding::new("pageup", actions::VolumeUpLarge, Some("volume-control")),
        KeyBinding::new("pagedown", actions::VolumeDownLarge, Some("volume-control")),
        KeyBinding::new("home", actions::VolumeMax, Some("volume-control")),
        KeyBinding::new("end", actions::VolumeMin, Some("volume-control")),
        KeyBinding::new("m", actions::ToggleMute, Some("volume-control")),
    ]
}

