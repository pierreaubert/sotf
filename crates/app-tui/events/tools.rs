use super::PlayerCommand;
use crate::app::{App, Screen, Tool};
use crossterm::event::{KeyCode, KeyEvent};
use sotf_audio_player::TrialMode;

pub(super) fn handle_tools_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Up | KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('k') => {
            app.ui.selected_tool = match app.ui.selected_tool {
                Tool::EarTraining => Tool::AbTesting,
                Tool::AbTesting => Tool::EarTraining,
            };
        }
        KeyCode::Char('1') => app.ui.selected_tool = Tool::EarTraining,
        KeyCode::Char('2') => app.ui.selected_tool = Tool::AbTesting,
        KeyCode::Enter => {
            let screen = match app.ui.selected_tool {
                Tool::EarTraining => Screen::EarTraining,
                Tool::AbTesting => Screen::AbTesting,
            };
            app.switch_screen(screen);
        }
        _ => return None,
    }
    app.ui.needs_redraw = true;
    None
}

pub(super) fn handle_ab_testing_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Char('a') | KeyCode::Char('A') => app.capture_ab_testing_path(true),
        KeyCode::Char('b') | KeyCode::Char('B') => app.capture_ab_testing_path(false),
        KeyCode::Char('p') | KeyCode::Char('P') => app.prepare_ab_testing_session(),
        KeyCode::Tab | KeyCode::Char('m') => {
            app.ui.ab_testing.trial_mode = match app.ui.ab_testing.trial_mode {
                TrialMode::BlindAb => TrialMode::Abx,
                TrialMode::Abx => TrialMode::BlindAb,
            };
            app.ui.ab_testing.status =
                format!("Mode: {}", trial_mode_label(app.ui.ab_testing.trial_mode));
        }
        KeyCode::Enter | KeyCode::Char('n') | KeyCode::Char('N') => app.start_ab_testing_trial(),
        KeyCode::Char('q') | KeyCode::Char('w') | KeyCode::Char('e') => {
            let index = match key.code {
                KeyCode::Char('q') => 0,
                KeyCode::Char('w') => 1,
                _ => 2,
            };
            if let Some(cue) = app
                .ui
                .ab_testing
                .controller
                .view()
                .available_cues
                .get(index)
                .copied()
            {
                app.activate_ab_testing_cue(cue);
            }
        }
        KeyCode::Char('1') | KeyCode::Char('2') => {
            let index = usize::from(key.code == KeyCode::Char('2'));
            if let Some(answer) = app
                .ui
                .ab_testing
                .controller
                .view()
                .available_answers
                .get(index)
                .copied()
            {
                app.commit_ab_testing_answer(answer);
            }
        }
        KeyCode::Char('s') | KeyCode::Char('S') => app.save_ab_testing_session(),
        KeyCode::Char('l') | KeyCode::Char('L') => app.load_ab_testing_session(),
        _ => return None,
    }
    app.ui.needs_redraw = true;
    None
}

fn trial_mode_label(mode: TrialMode) -> &'static str {
    match mode {
        TrialMode::BlindAb => "Blind A/B",
        TrialMode::Abx => "ABX",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::tests::{key, make_app};

    #[test]
    fn tools_menu_opens_both_tools() {
        let mut app = make_app();
        app.current_screen = Screen::Tools;

        handle_tools_keys(&mut app, key(KeyCode::Enter));
        assert_eq!(app.current_screen, Screen::EarTraining);

        app.current_screen = Screen::Tools;
        handle_tools_keys(&mut app, key(KeyCode::Char('2')));
        handle_tools_keys(&mut app, key(KeyCode::Enter));
        assert_eq!(app.current_screen, Screen::AbTesting);
    }
}
