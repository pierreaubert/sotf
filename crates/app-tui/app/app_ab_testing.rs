use super::{AbTestingStep, App, PendingParameterUpdate};
use sotf_audio::plugins::PluginSettings;
use sotf_audio_player::{
    PluginUpdateEffect, TrialAnswer, TrialCue,
    config::get_app_config_dir,
    controllers::{
        ab_compare_path::path_config_from_plugin_graph,
        ab_test_execution::{
            AbTestSessionPreparationRequest, load_ab_test_session, prepare_ab_test_session,
            save_ab_test_session,
        },
    },
};

impl App {
    pub fn capture_ab_testing_path(&mut self, path_a: bool) {
        let sample_rate = self.get_current_sample_rate();
        match path_config_from_plugin_graph(&self.plugin_rack.graph, sample_rate) {
            Ok(path) => {
                if path_a {
                    self.ui.ab_testing.path_a = Some(path);
                    self.ui.ab_testing.status = "Captured current graph as Path A.".into();
                } else {
                    self.ui.ab_testing.path_b = Some(path);
                    self.ui.ab_testing.status = "Captured current graph as Path B.".into();
                }
                let _ = self.ui.ab_testing.controller.clear_session();
                self.ui.ab_testing.step = AbTestingStep::Setup;
            }
            Err(error) => self.ui.ab_testing.status = error,
        }
        self.ui.needs_redraw = true;
    }

    pub fn prepare_ab_testing_session(&mut self) {
        let Some(path_a) = self.ui.ab_testing.path_a.clone() else {
            self.ui.ab_testing.status = "Capture Path A first.".into();
            return;
        };
        let Some(path_b) = self.ui.ab_testing.path_b.clone() else {
            self.ui.ab_testing.status = "Capture Path B first.".into();
            return;
        };
        let Some(media_path) = self.current_track_path() else {
            self.ui.ab_testing.status = "Load a local track before preparing a test.".into();
            return;
        };

        self.ui.ab_testing.status = "Measuring and level-matching the selected segment…".into();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let session_id = format!("sotf-tui-listening-{timestamp}");
        let result = prepare_ab_test_session(AbTestSessionPreparationRequest {
            session_id: &session_id,
            assignment_seed: timestamp as u64,
            path_a_label: "Path A",
            path_b_label: "Path B",
            path_a: &path_a,
            path_b: &path_b,
            media_path: &media_path,
            start_ms: (self.playback.position_secs.max(0.0) * 1_000.0).round() as u64,
            level_match: self.ui.ab_testing.level_match,
            block_frames: 1_024,
            switch_transition_ms: 20.0,
            participant_id: None,
            app_version: env!("CARGO_PKG_VERSION"),
        });

        match result {
            Ok((session, preparation)) => {
                let correction = preparation.measurement.correction_b_db;
                match self.ui.ab_testing.controller.replace_session(session) {
                    Ok(()) => {
                        self.ui.ab_testing.status =
                            format!("Ready · Path B correction {correction:+.2} dB");
                    }
                    Err(error) => self.ui.ab_testing.status = error.to_string(),
                }
            }
            Err(error) => self.ui.ab_testing.status = error.to_string(),
        }
        self.ui.needs_redraw = true;
    }

    pub fn start_ab_testing_trial(&mut self) {
        let mode = self.ui.ab_testing.trial_mode;
        if !self.ui.ab_testing.controller.view().runtime_active {
            let mut controller = std::mem::take(&mut self.ui.ab_testing.controller);
            let effect = controller.enter_runtime(&mut self.plugin_rack.graph);
            self.ui.ab_testing.controller = controller;
            match effect {
                Ok(effect) => self.apply_ab_testing_effect(effect),
                Err(error) => {
                    self.ui.ab_testing.status = error.to_string();
                    return;
                }
            }
        }
        match self.ui.ab_testing.controller.start_trial(mode) {
            Ok(index) => {
                self.ui.ab_testing.step = AbTestingStep::Trial;
                self.ui.ab_testing.status = format!("Trial #{} started.", index + 1);
            }
            Err(error) => self.ui.ab_testing.status = error.to_string(),
        }
        self.ui.needs_redraw = true;
    }

    pub fn activate_ab_testing_cue(&mut self, cue: TrialCue) {
        let mut controller = std::mem::take(&mut self.ui.ab_testing.controller);
        let effect = controller.activate_cue(&mut self.plugin_rack.graph, cue);
        self.ui.ab_testing.controller = controller;
        match effect {
            Ok(effect) => {
                self.apply_ab_testing_effect(effect);
                self.ui.ab_testing.status = "Cue active.".into();
            }
            Err(error) => self.ui.ab_testing.status = error.to_string(),
        }
        self.ui.needs_redraw = true;
    }

    pub fn commit_ab_testing_answer(&mut self, answer: TrialAnswer) {
        match self.ui.ab_testing.controller.commit_trial(
            answer,
            Some(self.ui.ab_testing.confidence),
            None,
        ) {
            Ok(()) => {
                self.ui.ab_testing.step = AbTestingStep::Results;
                self.ui.ab_testing.status = "Answer recorded.".into();
            }
            Err(error) => self.ui.ab_testing.status = error.to_string(),
        }
        self.ui.needs_redraw = true;
    }

    pub fn leave_ab_testing(&mut self) {
        let mut controller = std::mem::take(&mut self.ui.ab_testing.controller);
        let effect = controller.leave_runtime(&mut self.plugin_rack.graph);
        self.ui.ab_testing.controller = controller;
        if let Ok(effect) = effect {
            self.apply_ab_testing_effect(effect);
        }
        self.ui.ab_testing.step = if self.ui.ab_testing.controller.session().is_some() {
            AbTestingStep::Results
        } else {
            AbTestingStep::Setup
        };
    }

    pub fn save_ab_testing_session(&mut self) {
        let Some(path) = ab_testing_session_path() else {
            self.ui.ab_testing.status = "Configuration directory is unavailable.".into();
            return;
        };
        let Some(session) = self.ui.ab_testing.controller.session() else {
            self.ui.ab_testing.status = "No session to save.".into();
            return;
        };
        self.ui.ab_testing.status = match save_ab_test_session(session, &path) {
            Ok(()) => format!("Saved {}", path.display()),
            Err(error) => error.to_string(),
        };
    }

    pub fn load_ab_testing_session(&mut self) {
        let Some(path) = ab_testing_session_path() else {
            self.ui.ab_testing.status = "Configuration directory is unavailable.".into();
            return;
        };
        match load_ab_test_session(&path) {
            Ok(session) => {
                self.ui.ab_testing.path_a = Some(session.setup.path_a.config.clone());
                self.ui.ab_testing.path_b = Some(session.setup.path_b.config.clone());
                self.ui.ab_testing.level_match = session.setup.level_match.config();
                match self.ui.ab_testing.controller.replace_session(session) {
                    Ok(()) => {
                        self.ui.ab_testing.step = AbTestingStep::Results;
                        self.ui.ab_testing.status = format!("Loaded {}", path.display());
                    }
                    Err(error) => self.ui.ab_testing.status = error.to_string(),
                }
            }
            Err(error) => self.ui.ab_testing.status = error.to_string(),
        }
    }

    fn apply_ab_testing_effect(&mut self, effect: PluginUpdateEffect) {
        match effect {
            PluginUpdateEffect::None => {}
            PluginUpdateEffect::Structural | PluginUpdateEffect::Parameter { .. } => {
                self.request_plugin_update();
            }
            PluginUpdateEffect::ParameterByNodeId { node_id, .. } => {
                let Some(plugin_index) = self.plugin_rack.graph.get_engine_index(node_id) else {
                    self.request_plugin_update();
                    return;
                };
                let Some(node) = self.plugin_rack.graph.nodes.get(&node_id) else {
                    return;
                };
                let PluginSettings::ABCompare { selected_path, .. } = &node.plugin.settings else {
                    return;
                };
                self.plugin_rack.pending_param_update = Some(PendingParameterUpdate {
                    plugin_index,
                    param_id: "selected_path".into(),
                    value: selected_path.to_string(),
                });
            }
        }
    }
}

fn ab_testing_session_path() -> Option<std::path::PathBuf> {
    get_app_config_dir().map(|directory| directory.join("ab-testing-session.json"))
}
