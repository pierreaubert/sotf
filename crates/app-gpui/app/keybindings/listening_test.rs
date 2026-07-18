use crate::app::actions;
use gpui::KeyBinding;

pub(crate) const DOCUMENTED_BINDINGS: &[(&str, &str)] = &[
    ("Ctrl+1 / Ctrl+2", "Capture current chain as path A or B"),
    (
        "Ctrl+Shift+M",
        "Prepare deterministic level and latency matching",
    ),
    ("Ctrl+B / Ctrl+X", "Start blind A/B or ABX trial"),
    ("Alt+1 / Alt+2 / Alt+3", "Play available trial cues"),
    ("Alt+A / Alt+B", "Commit the first/A or second/B answer"),
];

/// Screen-local bindings for the complete pointer-free listening-test workflow.
///
/// The root key context contains both `PlayerView` and `ListeningTest` while
/// this screen is active, so transport bindings remain available.
pub(super) fn listening_test_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new(
            "ctrl-1",
            actions::ListeningCapturePathA,
            Some("ListeningTest"),
        ),
        KeyBinding::new(
            "ctrl-2",
            actions::ListeningCapturePathB,
            Some("ListeningTest"),
        ),
        KeyBinding::new(
            "ctrl-shift-m",
            actions::ListeningPrepare,
            Some("ListeningTest"),
        ),
        KeyBinding::new(
            "ctrl-b",
            actions::ListeningStartBlindAb,
            Some("ListeningTest"),
        ),
        KeyBinding::new("ctrl-x", actions::ListeningStartAbx, Some("ListeningTest")),
        KeyBinding::new("alt-1", actions::ListeningPlayCue1, Some("ListeningTest")),
        KeyBinding::new("alt-2", actions::ListeningPlayCue2, Some("ListeningTest")),
        KeyBinding::new("alt-3", actions::ListeningPlayCue3, Some("ListeningTest")),
        KeyBinding::new(
            "alt-a",
            actions::ListeningCommitAnswer1,
            Some("ListeningTest"),
        ),
        KeyBinding::new(
            "alt-b",
            actions::ListeningCommitAnswer2,
            Some("ListeningTest"),
        ),
    ]
}
