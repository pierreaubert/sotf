//! Keyboard adapter for the shared multi-microphone capture workflow.

use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent};
use sotf_audio_player::ui_models::room_eq::RoomEqViewEvent;
use std::path::PathBuf;

pub(super) fn handle_capture_keys(app: &mut App, key: KeyEvent) -> bool {
    let panel = &mut app.recording.multi_capture;
    if !panel.active {
        if key.code != KeyCode::F(8) {
            return false;
        }
        if app.recording.model.workflow_is_busy() {
            app.recording.model.status_message =
                "Finish or cancel the current recording operation first".into();
        } else {
            panel.active = true;
        }
        return true;
    }

    if panel.editing {
        let field = match panel.selected_field {
            0 => &mut panel.plan_path,
            1 => &mut panel.raw_directory,
            _ => &mut panel.processed_directory,
        };
        match key.code {
            KeyCode::Enter | KeyCode::Esc => panel.editing = false,
            KeyCode::Backspace => {
                field.pop();
            }
            KeyCode::Char(character) => field.push(character),
            _ => {}
        }
        return true;
    }

    let result = match key.code {
        KeyCode::Esc | KeyCode::F(8) => {
            if panel.workflow.is_busy() {
                panel.workflow.message =
                    "Operation in progress. C cancels recording; processing must finish.".into();
            } else {
                panel.active = false;
            }
            Ok(())
        }
        KeyCode::Up | KeyCode::BackTab => {
            panel.selected_field = (panel.selected_field + 2) % 3;
            Ok(())
        }
        KeyCode::Down | KeyCode::Tab => {
            panel.selected_field = (panel.selected_field + 1) % 3;
            Ok(())
        }
        KeyCode::PageUp => {
            panel.scroll = panel.scroll.saturating_sub(8);
            Ok(())
        }
        KeyCode::PageDown => {
            let max = panel.workflow.review_lines.len() + panel.workflow.qa_lines.len();
            panel.scroll = panel
                .scroll
                .saturating_add(8)
                .min(max.min(u16::MAX as usize) as u16);
            Ok(())
        }
        KeyCode::Enter if !panel.workflow.is_busy() => panel
            .workflow
            .inputs_changed(panel.selected_field == 0)
            .map(|()| panel.editing = true),
        KeyCode::Char('v') if !panel.plan_path.trim().is_empty() => {
            panel.scroll = 0;
            panel.workflow.load_plan(PathBuf::from(&panel.plan_path))
        }
        KeyCode::Char('r') if !panel.raw_directory.trim().is_empty() => {
            panel.workflow.record(PathBuf::from(&panel.raw_directory))
        }
        KeyCode::Char('p')
            if !panel.raw_directory.trim().is_empty()
                && !panel.processed_directory.trim().is_empty() =>
        {
            panel.scroll = 0;
            panel.workflow.process(
                PathBuf::from(&panel.raw_directory),
                PathBuf::from(&panel.processed_directory),
            )
        }
        KeyCode::Char('c') => {
            panel.workflow.cancel_recording();
            Ok(())
        }
        KeyCode::Char('i') => {
            if let Some(channels) = panel.workflow.import_channels() {
                app.room_eq
                    .model
                    .apply(RoomEqViewEvent::LoadMeasurements(channels));
                panel.workflow.message = "Imported processed measurements into RoomEQ with capture provenance. Open RoomEQ to configure optimization.".into();
                Ok(())
            } else {
                Err("No complete processed RoomEQ manifest is available for import".into())
            }
        }
        KeyCode::Char('v' | 'r' | 'p') => {
            Err("Enter the session plan and required output directory paths first".into())
        }
        _ => Ok(()),
    };
    if let Err(error) = result {
        panel.workflow.message = error;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn multi_capture_keyboard_edits_paths_without_starting_audio() {
        let mut app = crate::events::tests::make_app();
        assert!(!handle_capture_keys(&mut app, key(KeyCode::Char('r'))));
        assert!(handle_capture_keys(&mut app, key(KeyCode::F(8))));
        assert!(app.recording.multi_capture.active);
        handle_capture_keys(&mut app, key(KeyCode::Enter));
        for character in "capture.json".chars() {
            handle_capture_keys(&mut app, key(KeyCode::Char(character)));
        }
        assert_eq!(app.recording.multi_capture.plan_path, "capture.json");
        assert!(!app.recording.multi_capture.workflow.is_busy());
        handle_capture_keys(&mut app, key(KeyCode::Esc));
        assert!(!app.recording.multi_capture.editing);
        assert!(app.recording.multi_capture.active);
        handle_capture_keys(&mut app, key(KeyCode::Tab));
        assert_eq!(app.recording.multi_capture.selected_field, 1);
        handle_capture_keys(&mut app, key(KeyCode::BackTab));
        assert_eq!(app.recording.multi_capture.selected_field, 0);
        handle_capture_keys(&mut app, key(KeyCode::Esc));
        assert!(!app.recording.multi_capture.active);
    }

    #[test]
    fn raw_or_missing_evidence_cannot_replace_room_eq_measurements() {
        let mut app = crate::events::tests::make_app();
        app.recording.multi_capture.active = true;
        let before = serde_json::to_value(&app.room_eq.model.channel_measurements).unwrap();
        handle_capture_keys(&mut app, key(KeyCode::Char('i')));
        assert_eq!(
            serde_json::to_value(&app.room_eq.model.channel_measurements).unwrap(),
            before
        );
        assert!(
            app.recording
                .multi_capture
                .workflow
                .message
                .contains("No complete")
        );
        app.recording.multi_capture.raw_directory = "must-not-be-created".into();
        handle_capture_keys(&mut app, key(KeyCode::Char('r')));
        assert!(!app.recording.multi_capture.workflow.is_busy());
        assert!(
            app.recording
                .multi_capture
                .workflow
                .message
                .contains("Load and review")
        );
    }
}
