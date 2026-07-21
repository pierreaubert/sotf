use super::PlayerCommand;
use crate::app::{App, EarTrainingTab};
use crate::ui::keybinding_catalog::{
    EarTrainingCommand, TuiCommand, TuiKeyContext, resolve_command,
};
use crossterm::event::{KeyCode, KeyEvent};
use sotf_audio_player::{EarTrainingCourse, EqChangeMode};

pub fn handle_ear_training_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    let command = match resolve_command(TuiKeyContext::EarTraining, key) {
        Some(TuiCommand::EarTraining(command)) => command,
        Some(command) => {
            unreachable!("non-ear-training command in EarTraining context: {command:?}")
        }
        None => return None,
    };

    let player_command = match command {
        EarTrainingCommand::SwitchTab => {
            app.ui.ear_training.tab = match key.code {
                KeyCode::F(1) => EarTrainingTab::Practice,
                KeyCode::F(2) => EarTrainingTab::Courses,
                KeyCode::F(3) => EarTrainingTab::Progress,
                _ => unreachable!("non-tab chord resolved as SwitchTab: {key:?}"),
            };
            None
        }
        EarTrainingCommand::Activate => match app.ui.ear_training.tab {
            EarTrainingTab::Practice => {
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
                None
            }
            EarTrainingTab::Courses => {
                app.start_selected_ear_training_course();
                None
            }
            EarTrainingTab::Progress => None,
        },
        EarTrainingCommand::NavigateCourse
            if app.ui.ear_training.tab == EarTrainingTab::Courses =>
        {
            navigate_course(app, key);
            None
        }
        command if app.ui.ear_training.tab == EarTrainingTab::Practice => {
            handle_practice_command(app, key, command)
        }
        _ => None,
    };
    app.ui.needs_redraw = true;
    player_command
}

fn handle_practice_command(
    app: &mut App,
    key: KeyEvent,
    command: EarTrainingCommand,
) -> Option<PlayerCommand> {
    match command {
        EarTrainingCommand::StartSession => app.start_ear_training(None),
        EarTrainingCommand::CycleExercise => app.cycle_ear_training_exercise(),
        EarTrainingCommand::ToggleAdaptive => app.toggle_ear_training_adaptive(),
        EarTrainingCommand::CycleChangeMode => {
            app.ui.ear_training.config.change_mode = match app.ui.ear_training.config.change_mode {
                EqChangeMode::Boost => EqChangeMode::Cut,
                EqChangeMode::Cut => EqChangeMode::Mixed,
                EqChangeMode::Mixed => EqChangeMode::Boost,
            };
            app.ui.ear_training.session = None;
            app.ui.ear_training.active_course = None;
        }
        EarTrainingCommand::AdjustBandCount => {
            if key.code == KeyCode::Char('b') {
                adjust_config(app, |config| {
                    config.band_count = config.band_count.saturating_sub(1).max(2)
                });
            } else {
                adjust_config(app, |config| {
                    config.band_count = (config.band_count + 1).min(25)
                });
            }
        }
        EarTrainingCommand::AdjustGain => {
            if key.code == KeyCode::Char('g') {
                adjust_config(app, |config| {
                    config.gain_db = (config.gain_db - 1.0).max(1.0)
                });
            } else {
                adjust_config(app, |config| {
                    config.gain_db = (config.gain_db + 1.0).min(15.0)
                });
            }
        }
        EarTrainingCommand::AdjustQ => {
            if key.code == KeyCode::Char('v') {
                adjust_config(app, |config| config.q = (config.q - 0.1).max(0.2));
            } else {
                adjust_config(app, |config| config.q = (config.q + 0.1).min(10.0));
            }
        }
        EarTrainingCommand::AdjustTrialCount => {
            if key.code == KeyCode::Char('t') {
                adjust_config(app, |config| {
                    config.trial_count = config.trial_count.saturating_sub(5).max(5)
                });
            } else {
                adjust_config(app, |config| {
                    config.trial_count = (config.trial_count + 5).min(500)
                });
            }
        }
        EarTrainingCommand::SelectAnswer => {
            let delta = if matches!(key.code, KeyCode::Left | KeyCode::Char('h')) {
                -1
            } else {
                1
            };
            app.move_ear_training_answer(delta);
        }
        EarTrainingCommand::NextTrial => app.advance_ear_training(),
        EarTrainingCommand::Audition => {
            app.activate_ear_training_path(key.code == KeyCode::Char('2'))
        }
        EarTrainingCommand::AddSource => app.add_current_ear_training_source(),
        EarTrainingCommand::NavigateSource if key.code == KeyCode::Char(',') => {
            return app
                .navigate_ear_training_source(-1)
                .map(PlayerCommand::Play);
        }
        EarTrainingCommand::NavigateSource => {
            return app.navigate_ear_training_source(1).map(PlayerCommand::Play);
        }
        EarTrainingCommand::SetLoopBoundary => {
            app.set_ear_training_loop_boundary(key.code == KeyCode::Char('['))
        }
        EarTrainingCommand::ToggleLoop => app.toggle_ear_training_loop(),
        EarTrainingCommand::SwitchTab
        | EarTrainingCommand::Activate
        | EarTrainingCommand::NavigateCourse => {
            unreachable!("command handled before practice dispatch: {command:?}")
        }
    }
    None
}

fn navigate_course(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.ui.ear_training.course_selection =
                app.ui.ear_training.course_selection.saturating_sub(1)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.ui.ear_training.course_selection =
                (app.ui.ear_training.course_selection + 1).min(EarTrainingCourse::ALL.len() - 1)
        }
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
            app.ui
                .ear_training
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
