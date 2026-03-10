//! Debug state history tests (from app/debug.rs)

use sotf_audio_player_gpui::{InputMode, Screen, StateHistory, MAX_HISTORY_SIZE};

#[test]
fn test_state_history_capture() {
    let mut history = StateHistory::new();

    history.capture(
        Screen::Library,
        InputMode::Normal,
        Some("Test Device".to_string()),
        "initial",
    );

    assert_eq!(history.len(), 1);

    let snapshots = history.last_n(1);
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].screen, Screen::Library);
    assert_eq!(snapshots[0].input_mode, InputMode::Normal);
    assert_eq!(snapshots[0].trigger, "initial");
}

#[test]
fn test_state_history_max_size() {
    let mut history = StateHistory::new();

    for i in 0..150 {
        history.capture(
            Screen::Library,
            InputMode::Normal,
            None,
            format!("trigger_{}", i),
        );
    }

    assert_eq!(history.len(), MAX_HISTORY_SIZE);

    // First snapshot should be trigger_50 (150 - 100)
    let all: Vec<_> = history.all().collect();
    assert_eq!(all[0].trigger, "trigger_50");
}
