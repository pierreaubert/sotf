//! Scenario-based integration tests.
//!
//! Each scenario is a sequence of key presses and assertions that verify
//! end-to-end TUI workflows without manual state mutations between steps.

use super::handle_key_event;
use super::tests::{key, make_app};
use crate::app::{App, ConfigureSubScreen, InputMode, Screen, SpinoramaStep};
use sotf_audio_player::room_eq_types::RoomEqStep;
use crossterm::event::KeyCode;
use std::panic::{catch_unwind, AssertUnwindSafe};

// ── Framework types ──────────────────────────────────────────────────────────

enum Input {
    Key(KeyCode),
    /// Types each character as a `KeyCode::Char` press.
    TypeText(&'static str),
}

struct ScenarioStep {
    description: &'static str,
    inputs: Vec<Input>,
    assert: Box<dyn Fn(&App)>,
}

fn step(
    description: &'static str,
    inputs: Vec<Input>,
    assert_fn: impl Fn(&App) + 'static,
) -> ScenarioStep {
    ScenarioStep {
        description,
        inputs,
        assert: Box::new(assert_fn),
    }
}

// ── Runner ───────────────────────────────────────────────────────────────────

fn run_scenario(app: &mut App, steps: Vec<ScenarioStep>) {
    for (i, s) in steps.iter().enumerate() {
        // Send all inputs for this step
        for input in &s.inputs {
            match input {
                Input::Key(code) => {
                    handle_key_event(app, key(*code));
                }
                Input::TypeText(text) => {
                    for ch in text.chars() {
                        handle_key_event(app, key(KeyCode::Char(ch)));
                    }
                }
            }
        }

        // Run assertion with augmented panic message
        let result = catch_unwind(AssertUnwindSafe(|| (s.assert)(app)));
        if let Err(payload) = result {
            let msg = if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = payload.downcast_ref::<&str>() {
                (*s).to_string()
            } else {
                "assertion failed".to_string()
            };
            panic!(
                "Scenario step {} ({:?}) failed: {}",
                i + 1,
                s.description,
                msg
            );
        }
    }
}

// ── Setup helpers ────────────────────────────────────────────────────────────

/// App on Library screen with 6 pre-populated speakers for spinorama tests.
fn app_for_spinorama_scenario() -> App {
    let mut app = make_app();
    app.current_screen = Screen::Library;
    // Pre-populate speakers: 4 Adam models + 2 others
    app.spinorama_eq.available_speakers = vec![
        "Adam A7V".to_string(),
        "Adam S2V".to_string(),
        "Adam T5V".to_string(),
        "Adam D3V".to_string(),
        "Genelec 8030C".to_string(),
        "KRK Rokit 5".to_string(),
    ];
    app.spinorama_eq.update_filter();
    app
}

// ── Spinorama EQ scenario ────────────────────────────────────────────────────

#[test]
fn scenario_spinorama() {
    let mut app = app_for_spinorama_scenario();

    let steps = vec![
        // 1. C → Screen::Configure, tab bar focused
        step(
            "C opens Configure screen",
            vec![Input::Key(KeyCode::Char('C'))],
            |app| {
                assert_eq!(app.current_screen, Screen::Configure);
                assert!(app.configure_tab_focused);
            },
        ),
        // 2. 5 → SpinoramaEq sub-screen, step=Select
        step(
            "5 selects SpinoramaEq sub-screen",
            vec![Input::Key(KeyCode::Char('5'))],
            |app| {
                assert_eq!(app.configure_sub_screen, ConfigureSubScreen::SpinoramaEq);
                assert!(!app.configure_tab_focused);
                assert_eq!(app.spinorama_eq.step, SpinoramaStep::Select);
            },
        ),
        // 3. Type "Adam" → filtered to 4 Adam speakers
        step(
            "typing Adam filters speaker list",
            vec![Input::TypeText("Adam")],
            |app| {
                assert_eq!(app.spinorama_eq.filtered_speakers.len(), 4);
                assert!(app.spinorama_eq.filtered_speakers.iter().all(|s| s.contains("Adam")));
            },
        ),
        // 4. Down Down Down → selected_speaker_idx = 3 (Adam D3V)
        step(
            "Down x3 selects Adam D3V",
            vec![
                Input::Key(KeyCode::Down),
                Input::Key(KeyCode::Down),
                Input::Key(KeyCode::Down),
            ],
            |app| {
                assert_eq!(app.spinorama_eq.selected_speaker_idx, 3);
                assert_eq!(app.spinorama_eq.filtered_speakers[3], "Adam D3V");
            },
        ),
        // 5. Enter → selects speaker, step=Configure
        step(
            "Enter selects speaker and advances to Configure",
            vec![Input::Key(KeyCode::Enter)],
            |app| {
                assert_eq!(app.spinorama_eq.selected_speaker, Some("Adam D3V".to_string()));
                assert_eq!(app.spinorama_eq.step, SpinoramaStep::Configure);
            },
        ),
        // 6. Right → loss_function cycles from "flat-asymmetric" to "score"
        step(
            "Right cycles loss function to score",
            vec![Input::Key(KeyCode::Right)],
            |app| {
                assert_eq!(app.spinorama_eq.config.loss_function, "score");
            },
        ),
        // 7. Tab → step=Optimize
        step(
            "Tab advances to Optimize",
            vec![Input::Key(KeyCode::Tab)],
            |app| {
                assert_eq!(app.spinorama_eq.step, SpinoramaStep::Optimize);
            },
        ),
        // 8. Tab → step=Results (skip actual optimization)
        step(
            "Tab advances to Results",
            vec![Input::Key(KeyCode::Tab)],
            |app| {
                assert_eq!(app.spinorama_eq.step, SpinoramaStep::Results);
            },
        ),
        // 9. Tab → step=UpdatePlugin
        step(
            "Tab advances to UpdatePlugin",
            vec![Input::Key(KeyCode::Tab)],
            |app| {
                assert_eq!(app.spinorama_eq.step, SpinoramaStep::UpdatePlugin);
            },
        ),
        // 10. Enter → applies EQ, status_message set
        step(
            "Enter applies EQ to plugin chain",
            vec![Input::Key(KeyCode::Enter)],
            |app| {
                assert!(
                    app.status_message.is_some(),
                    "expected status_message to be set after applying EQ"
                );
            },
        ),
        // 11. BackTab x4 → back through Results→Optimize→Configure→Select
        step(
            "BackTab x4 returns to Select",
            vec![
                Input::Key(KeyCode::BackTab),
                Input::Key(KeyCode::BackTab),
                Input::Key(KeyCode::BackTab),
                Input::Key(KeyCode::BackTab),
            ],
            |app| {
                assert_eq!(app.spinorama_eq.step, SpinoramaStep::Select);
            },
        ),
        // 12. Esc → step_tab_focused
        step(
            "Esc focuses step tab bar",
            vec![Input::Key(KeyCode::Esc)],
            |app| {
                assert!(app.spinorama_eq.step_tab_focused);
                assert!(!app.configure_tab_focused);
                assert_eq!(app.current_screen, Screen::Configure);
            },
        ),
        // 13. Esc → configure_tab_focused
        step(
            "Esc focuses configure tab bar",
            vec![Input::Key(KeyCode::Esc)],
            |app| {
                assert!(app.configure_tab_focused);
                assert!(!app.spinorama_eq.step_tab_focused);
                assert_eq!(app.current_screen, Screen::Configure);
            },
        ),
        // 14. Esc → Screen::Library
        step(
            "Esc returns to Library",
            vec![Input::Key(KeyCode::Esc)],
            |app| {
                assert_eq!(app.current_screen, Screen::Library);
            },
        ),
    ];

    run_scenario(&mut app, steps);
}

// ── Room EQ scenario ─────────────────────────────────────────────────────────

#[test]
fn scenario_room_eq_load_data() {
    let mut app = make_app();
    app.current_screen = Screen::Library;

    let steps = vec![
        // 1. C → Configure screen
        step(
            "C opens Configure screen",
            vec![Input::Key(KeyCode::Char('C'))],
            |app| {
                assert_eq!(app.current_screen, Screen::Configure);
                assert!(app.configure_tab_focused);
            },
        ),
        // 2. 3 → RoomEq, auto-opens file explorer (no data loaded)
        step(
            "3 selects RoomEq, auto-opens file explorer",
            vec![Input::Key(KeyCode::Char('3'))],
            |app| {
                assert_eq!(app.configure_sub_screen, ConfigureSubScreen::RoomEq);
                assert!(!app.configure_tab_focused);
                assert_eq!(app.room_eq.step, RoomEqStep::LoadData);
                assert_eq!(
                    app.input_mode,
                    InputMode::FileExplorer,
                    "file explorer should auto-open when no data is loaded"
                );
            },
        ),
        // 3. Esc → close file explorer, back to LoadData content
        step(
            "Esc closes file explorer",
            vec![Input::Key(KeyCode::Esc)],
            |app| {
                assert_eq!(app.input_mode, InputMode::Normal);
            },
        ),
        // 4. Tab → advance to Configure step
        step(
            "Tab advances to Configure step",
            vec![Input::Key(KeyCode::Tab)],
            |app| {
                assert_eq!(app.room_eq.step, RoomEqStep::Configure);
            },
        ),
        // 5. Tab → advance to Optimize step
        step(
            "Tab advances to Optimize step",
            vec![Input::Key(KeyCode::Tab)],
            |app| {
                assert_eq!(app.room_eq.step, RoomEqStep::Optimize);
            },
        ),
        // 6. Tab → advance to Review step
        step(
            "Tab advances to Review step",
            vec![Input::Key(KeyCode::Tab)],
            |app| {
                assert_eq!(app.room_eq.step, RoomEqStep::Review);
            },
        ),
        // 7. Tab → advance to Export step
        step(
            "Tab advances to Export step",
            vec![Input::Key(KeyCode::Tab)],
            |app| {
                assert_eq!(app.room_eq.step, RoomEqStep::Export);
            },
        ),
        // 8. Esc → step tab bar
        step(
            "Esc focuses step tab bar",
            vec![Input::Key(KeyCode::Esc)],
            |app| {
                assert!(app.room_eq.step_tab_focused);
                assert!(!app.configure_tab_focused);
            },
        ),
        // 9. Esc → configure tab bar
        step(
            "Esc focuses configure tab bar",
            vec![Input::Key(KeyCode::Esc)],
            |app| {
                assert!(app.configure_tab_focused);
                assert!(!app.room_eq.step_tab_focused);
            },
        ),
        // 10. Esc → Library
        step(
            "Esc returns to Library",
            vec![Input::Key(KeyCode::Esc)],
            |app| {
                assert_eq!(app.current_screen, Screen::Library);
            },
        ),
    ];

    run_scenario(&mut app, steps);
}
