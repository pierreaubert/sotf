use super::App;
use crate::app::{EarTrainingTab, Screen};
use sotf_audio::decoder::AudioSource;
use sotf_audio_player::controllers::ab_compare_path::PathConfig;
use sotf_audio_player::{
    EarTrainingCourse, EqTrainingExercise, EqTrainingSession, PluginSettings, PluginType,
};

impl App {
    pub fn start_ear_training(&mut self, course: Option<EarTrainingCourse>) {
        if let Some(course) = course {
            self.ui.ear_training.config = course.config();
            self.ui.ear_training.active_course = Some(course);
        }
        if let Err(error) = self.ensure_ear_training_audition() {
            self.ui.status_message = Some(error);
            return;
        }
        self.ui.ear_training.config.seed = self.ui.ear_training.config.seed.wrapping_add(1);
        match EqTrainingSession::new(self.ui.ear_training.config.clone()).and_then(|mut session| {
            session.start()?;
            Ok(session)
        }) {
            Ok(session) => {
                self.ui.ear_training.session = Some(session);
                self.ui.ear_training.selected_answer = 0;
                self.ui.ear_training.filtered = false;
                self.ui.status_message = Some("Ear-training session started".into());
                self.activate_ear_training_path(false);
            }
            Err(error) => self.ui.status_message = Some(error.to_string()),
        }
    }

    pub fn start_selected_ear_training_course(&mut self) {
        let course = EarTrainingCourse::ALL[self
            .ui
            .ear_training
            .course_selection
            .min(EarTrainingCourse::ALL.len() - 1)];
        self.ui.ear_training.tab = EarTrainingTab::Practice;
        self.start_ear_training(Some(course));
    }

    pub fn cycle_ear_training_exercise(&mut self) {
        self.ui.ear_training.config.exercise = match self.ui.ear_training.config.exercise {
            EqTrainingExercise::BandIdentification => EqTrainingExercise::BoostCutIdentification,
            EqTrainingExercise::BoostCutIdentification => EqTrainingExercise::GainIdentification,
            EqTrainingExercise::GainIdentification => EqTrainingExercise::BandIdentification,
        };
        self.ui.ear_training.session = None;
        self.ui.ear_training.active_course = None;
    }

    pub fn toggle_ear_training_adaptive(&mut self) {
        self.ui.ear_training.adaptive = !self.ui.ear_training.adaptive;
        if self.ui.ear_training.adaptive {
            let exercise = self.ui.ear_training.config.exercise;
            self.ui.ear_training.config = self.ui.ear_training.progress.adaptive_config();
            self.ui.ear_training.config.exercise = exercise;
            self.ui.ear_training.active_course = None;
        }
    }

    pub fn move_ear_training_answer(&mut self, delta: i32) {
        let count = self.ui.ear_training.answer_count();
        if count > 0
            && !self
                .ui
                .ear_training
                .session
                .as_ref()
                .is_some_and(EqTrainingSession::current_is_answered)
        {
            self.ui.ear_training.selected_answer =
                (self.ui.ear_training.selected_answer as i32 + delta).rem_euclid(count as i32)
                    as usize;
        }
    }

    pub fn submit_ear_training_answer(&mut self) {
        let selected = self.ui.ear_training.selected_answer;
        let status = match self.ui.ear_training.session.as_mut() {
            Some(session) => match session.submit_answer(selected) {
                Ok(result) if result.correct => "Correct".into(),
                Ok(result) => format!(
                    "Answer: {:.0} Hz, {:+.0} dB",
                    result.question.center_frequency_hz,
                    result.question.signed_gain_db()
                ),
                Err(error) => error.to_string(),
            },
            None => "Start a session first".into(),
        };
        self.ui.status_message = Some(status);
    }

    pub fn advance_ear_training(&mut self) {
        let mut completed = None;
        let status = match self.ui.ear_training.session.as_mut() {
            Some(session) => match session.advance() {
                Ok(Some(_)) => {
                    self.ui.ear_training.selected_answer = 0;
                    "Next trial".into()
                }
                Ok(None) => {
                    completed = Some(session.clone());
                    format!(
                        "Session complete: {}/{} ({:.0}%)",
                        session.correct_count(),
                        session.trials.len(),
                        session.accuracy() * 100.0
                    )
                }
                Err(error) => error.to_string(),
            },
            None => "Start a session first".into(),
        };
        if let Some(session) = completed {
            self.ui
                .ear_training
                .progress
                .record(&session, self.ui.ear_training.active_course);
            if self.ui.ear_training.adaptive {
                let exercise = self.ui.ear_training.config.exercise;
                self.ui.ear_training.config = self.ui.ear_training.progress.adaptive_config();
                self.ui.ear_training.config.exercise = exercise;
            }
            if let Some(path) = sotf_audio_player::config::get_ear_training_progress_path()
                && let Err(error) = self.ui.ear_training.progress.save_atomic(&path)
            {
                log::warn!("Failed to save TUI ear-training progress: {error}");
            }
        }
        self.ui.status_message = Some(status);
        self.activate_ear_training_path(false);
    }

    pub fn activate_ear_training_path(&mut self, filtered: bool) {
        let Some(question) = self
            .ui
            .ear_training
            .session
            .as_ref()
            .and_then(|session| session.current_question.clone())
        else {
            return;
        };
        let Some(node_id) = self.find_ab_compare_node() else {
            self.ui.status_message = Some("A/B audition path unavailable".into());
            return;
        };
        let path_a = serde_json::to_string(&PathConfig::None).unwrap_or_default();
        let path_b = serde_json::to_string(&PathConfig::Plugin {
            plugin_type: "eq".into(),
            parameters: question.plugin_parameters(),
        })
        .unwrap_or_default();
        if let Some(node) = self.plugin_rack.graph.nodes.get_mut(&node_id)
            && let PluginSettings::ABCompare {
                mix,
                mix_mode,
                selected_path,
                auto_gain_enabled,
                difference_mode,
                mix_transition_ms,
                path_a_config,
                path_b_config,
                ..
            } = &mut node.plugin.settings
        {
            *path_a_config = path_a;
            *path_b_config = path_b;
            *mix = 0.0;
            *mix_mode = 1;
            *selected_path = i32::from(filtered);
            *auto_gain_enabled = false;
            *difference_mode = false;
            *mix_transition_ms = 20.0;
            self.ui.ear_training.filtered = filtered;
            self.request_plugin_update();
            self.ui.status_message = Some(if filtered {
                "Filtered audition active".into()
            } else {
                "Original audition active".into()
            });
        }
    }

    pub fn leave_ear_training(&mut self) {
        if let Some(node_id) = self.ui.ear_training.audition_node_id.take() {
            let _ = self.plugin_rack.graph.remove_user_plugin(node_id);
            self.plugin_rack.graph.update_channel_dependent_plugins();
            self.request_plugin_update();
        }
        self.ui.ear_training.filtered = false;
    }

    pub fn switch_screen(&mut self, screen: Screen) {
        if self.current_screen == Screen::EarTraining && screen != Screen::EarTraining {
            self.leave_ear_training();
        }
        if self.current_screen == Screen::AbTesting && screen != Screen::AbTesting {
            self.leave_ab_testing();
        }
        self.current_screen = screen;
    }

    pub fn add_current_ear_training_source(&mut self) {
        let Some(path) = self.current_track_path() else {
            self.ui.status_message = Some("No local track selected".into());
            return;
        };
        if !self.ui.ear_training.sources.contains(&path) {
            self.ui.ear_training.sources.push(path);
            self.ui.ear_training.source_index = self.ui.ear_training.sources.len() - 1;
        }
        self.ui.status_message = Some(format!(
            "{} training sources",
            self.ui.ear_training.sources.len()
        ));
    }

    pub fn navigate_ear_training_source(&mut self, delta: i32) -> Option<AudioSource> {
        let count = self.ui.ear_training.sources.len();
        if count == 0 {
            self.ui.status_message = Some("Add the current track first".into());
            return None;
        }
        self.ui.ear_training.source_index =
            (self.ui.ear_training.source_index as i32 + delta).rem_euclid(count as i32) as usize;
        Some(AudioSource::File(
            self.ui.ear_training.sources[self.ui.ear_training.source_index].clone(),
        ))
    }

    pub fn set_ear_training_loop_boundary(&mut self, start: bool) {
        let position = self.playback.position_secs.max(0.0);
        let (mut loop_start, mut loop_end) = self
            .ui
            .ear_training
            .loop_range
            .unwrap_or((0.0, position + 5.0));
        if start {
            loop_start = position.min(loop_end - 0.1);
        } else {
            loop_end = position.max(loop_start + 0.1);
        }
        self.ui.ear_training.loop_range = Some((loop_start, loop_end));
        self.ui.status_message = Some(format!("Loop {loop_start:.1}–{loop_end:.1} s"));
    }

    pub fn toggle_ear_training_loop(&mut self) {
        self.ui.ear_training.loop_enabled =
            !self.ui.ear_training.loop_enabled && self.ui.ear_training.loop_range.is_some();
        self.ui.status_message = Some(if self.ui.ear_training.loop_enabled {
            "Clip loop enabled".into()
        } else {
            "Clip loop disabled".into()
        });
    }

    fn ensure_ear_training_audition(&mut self) -> Result<(), String> {
        if self.find_ab_compare_node().is_some() {
            return Ok(());
        }
        self.add_plugin(&PluginType::ABCompare);
        let node_id = self
            .find_ab_compare_node()
            .ok_or_else(|| "Could not create A/B audition path".to_string())?;
        self.ui.ear_training.audition_node_id = Some(node_id);
        Ok(())
    }

    fn find_ab_compare_node(&self) -> Option<sotf_audio_player::GraphNodeId> {
        self.plugin_rack
            .graph
            .nodes
            .values()
            .find(|node| node.plugin.plugin_type() == PluginType::ABCompare)
            .map(|node| node.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    fn ab_compare_count(app: &App) -> usize {
        app.plugin_rack
            .graph
            .nodes
            .values()
            .filter(|node| node.plugin.plugin_type() == PluginType::ABCompare)
            .count()
    }

    #[test]
    fn trainer_owned_audition_node_is_removed_on_exit() {
        let mut app = App::new(Theme::default(), false);
        app.current_screen = Screen::EarTraining;
        app.start_ear_training(None);
        assert_eq!(ab_compare_count(&app), 1);
        assert!(app.ui.ear_training.audition_node_id.is_some());

        app.switch_screen(Screen::Library);
        assert_eq!(ab_compare_count(&app), 0);
        assert!(app.ui.ear_training.audition_node_id.is_none());
    }

    #[test]
    fn existing_audition_node_is_reused_and_retained() {
        let mut app = App::new(Theme::default(), false);
        app.add_plugin(&PluginType::ABCompare);
        app.current_screen = Screen::EarTraining;
        app.start_ear_training(None);
        assert_eq!(ab_compare_count(&app), 1);
        assert!(app.ui.ear_training.audition_node_id.is_none());

        app.switch_screen(Screen::Library);
        assert_eq!(ab_compare_count(&app), 1);
    }

    #[test]
    fn course_and_adaptive_sessions_use_shared_domain_model() {
        let mut app = App::new(Theme::default(), false);
        app.ui.ear_training.course_selection = EarTrainingCourse::ALL.len() - 1;
        app.start_selected_ear_training_course();
        assert_eq!(app.ui.ear_training.config.band_count, 15);
        assert!(app.ui.ear_training.session.is_some());

        app.cycle_ear_training_exercise();
        assert_eq!(
            app.ui.ear_training.config.exercise,
            EqTrainingExercise::BoostCutIdentification
        );
        assert!(app.ui.ear_training.session.is_none());
    }

    #[test]
    fn loop_only_seeks_after_enabled_end_boundary() {
        let mut app = App::new(Theme::default(), false);
        app.ui.ear_training.loop_range = Some((3.0, 8.0));
        assert_eq!(app.ui.ear_training.should_loop(9.0), None);
        app.toggle_ear_training_loop();
        assert_eq!(app.ui.ear_training.should_loop(7.9), None);
        assert_eq!(app.ui.ear_training.should_loop(8.0), Some(3.0));
    }
}
