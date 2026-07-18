use super::PlayerCommand;
use crate::app::{App, EarTrainingTab};
use crossterm::event::{KeyCode, KeyEvent};
use sotf_audio_player::{EarTrainingCourse, EqChangeMode};

pub fn handle_ear_training_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::F(1) => app.ui.ear_training.tab = EarTrainingTab::Practice,
        KeyCode::F(2) => app.ui.ear_training.tab = EarTrainingTab::Courses,
        KeyCode::F(3) => app.ui.ear_training.tab = EarTrainingTab::Progress,
        _ => match app.ui.ear_training.tab {
            EarTrainingTab::Practice => return handle_practice_keys(app, key),
            EarTrainingTab::Courses => handle_course_keys(app, key),
            EarTrainingTab::Progress => {}
        },
    }
    app.ui.needs_redraw = true;
    None
}

fn handle_practice_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Char('s') => app.start_ear_training(None),
        KeyCode::Char('e') => app.cycle_ear_training_exercise(),
        KeyCode::Char('a') => app.toggle_ear_training_adaptive(),
        KeyCode::Char('c') => {
            app.ui.ear_training.config.change_mode = match app.ui.ear_training.config.change_mode {
                EqChangeMode::Boost => EqChangeMode::Cut,
                EqChangeMode::Cut => EqChangeMode::Mixed,
                EqChangeMode::Mixed => EqChangeMode::Boost,
            };
            app.ui.ear_training.session = None;
            app.ui.ear_training.active_course = None;
        }
        KeyCode::Char('b') => adjust_config(app, |config| {
            config.band_count = config.band_count.saturating_sub(1).max(2)
        }),
        KeyCode::Char('B') => adjust_config(app, |config| {
            config.band_count = (config.band_count + 1).min(25)
        }),
        KeyCode::Char('g') => adjust_config(app, |config| {
            config.gain_db = (config.gain_db - 1.0).max(1.0)
        }),
        KeyCode::Char('G') => adjust_config(app, |config| {
            config.gain_db = (config.gain_db + 1.0).min(15.0)
        }),
        KeyCode::Char('v') => adjust_config(app, |config| config.q = (config.q - 0.1).max(0.2)),
        KeyCode::Char('V') => adjust_config(app, |config| config.q = (config.q + 0.1).min(10.0)),
        KeyCode::Char('t') => adjust_config(app, |config| {
            config.trial_count = config.trial_count.saturating_sub(5).max(5)
        }),
        KeyCode::Char('T') => adjust_config(app, |config| {
            config.trial_count = (config.trial_count + 5).min(500)
        }),
        KeyCode::Left | KeyCode::Char('h') => app.move_ear_training_answer(-1),
        KeyCode::Right | KeyCode::Char('l') => app.move_ear_training_answer(1),
        KeyCode::Enter => {
            if app
                .ui
                .ear_training
                .session
                .as_ref()
                .is_some_and(sotf_audio_player::EqTrainingSession::current_is_answered)
            {
                app.advance_ear_training();
            } else {
                app.submit_ear_training_answer();
            }
        }
        KeyCode::Char('n') => app.advance_ear_training(),
        KeyCode::Char('1') => app.activate_ear_training_path(false),
        KeyCode::Char('2') => app.activate_ear_training_path(true),
        KeyCode::Char('i') => app.add_current_ear_training_source(),
        KeyCode::Char(',') => {
            return app
                .navigate_ear_training_source(-1)
                .map(PlayerCommand::Play);
        }
        KeyCode::Char('.') => {
            return app.navigate_ear_training_source(1).map(PlayerCommand::Play);
        }
        KeyCode::Char('[') => app.set_ear_training_loop_boundary(true),
        KeyCode::Char(']') => app.set_ear_training_loop_boundary(false),
        KeyCode::Char('\\') => app.toggle_ear_training_loop(),
        _ => return None,
    }
    app.ui.needs_redraw = true;
    None
}

fn handle_course_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.ui.ear_training.course_selection =
                app.ui.ear_training.course_selection.saturating_sub(1)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.ui.ear_training.course_selection =
                (app.ui.ear_training.course_selection + 1).min(EarTrainingCourse::ALL.len() - 1)
        }
        KeyCode::Enter => app.start_selected_ear_training_course(),
        _ => {}
    }
}

fn adjust_config(app: &mut App, update: impl FnOnce(&mut sotf_audio_player::EqTrainingConfig)) {
    update(&mut app.ui.ear_training.config);
    app.ui.ear_training.session = None;
    app.ui.ear_training.active_course = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Screen;
    use crate::events::tests::{key, make_app};

    #[test]
    fn global_hotkey_and_escape_enter_and_leave_trainer() {
        let mut app = make_app();
        super::super::handle_key_event(&mut app, key(KeyCode::Char('E')));
        assert_eq!(app.current_screen, Screen::EarTraining);
        super::super::handle_key_event(&mut app, key(KeyCode::Esc));
        assert_eq!(app.current_screen, Screen::Library);
        assert!(!app.should_quit);
    }

    #[test]
    fn practice_keys_drive_session_and_answer_selection() {
        let mut app = make_app();
        app.current_screen = Screen::EarTraining;
        handle_ear_training_keys(&mut app, key(KeyCode::Char('s')));
        assert!(app.ui.ear_training.session.is_some());
        handle_ear_training_keys(&mut app, key(KeyCode::Right));
        assert_eq!(app.ui.ear_training.selected_answer, 1);
        handle_ear_training_keys(&mut app, key(KeyCode::Char('2')));
        assert!(app.ui.ear_training.filtered);
        handle_ear_training_keys(&mut app, key(KeyCode::Enter));
        assert!(
            app.ui.ear_training
                .session
                .as_ref()
                .is_some_and(sotf_audio_player::EqTrainingSession::current_is_answered)
        );
    }

    #[test]
    fn function_tabs_and_course_navigation_are_keyboard_first() {
        let mut app = make_app();
        app.current_screen = Screen::EarTraining;
        handle_ear_training_keys(&mut app, key(KeyCode::F(2)));
        assert_eq!(app.ui.ear_training.tab, EarTrainingTab::Courses);
        handle_ear_training_keys(&mut app, key(KeyCode::Down));
        assert_eq!(app.ui.ear_training.course_selection, 1);
        handle_ear_training_keys(&mut app, key(KeyCode::Enter));
        assert_eq!(app.ui.ear_training.tab, EarTrainingTab::Practice);
        assert!(app.ui.ear_training.session.is_some());
    }
}
