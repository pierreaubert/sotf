use sotf_audio_player::{
    AbTestController, TrialMode,
    controllers::{ab_compare_path::PathConfig, ab_test_session::LevelMatchConfig},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AbTestingStep {
    #[default]
    Setup,
    Trial,
    Results,
}

/// Presentation-only state for the TUI A/B tool.
///
/// Trial assignment, scoring, and runtime routing remain owned by the shared
/// `AbTestController`.
#[derive(Debug, Clone)]
pub struct AbTestingTuiState {
    pub controller: AbTestController,
    pub step: AbTestingStep,
    pub path_a: Option<PathConfig>,
    pub path_b: Option<PathConfig>,
    pub trial_mode: TrialMode,
    pub level_match: LevelMatchConfig,
    pub confidence: u8,
    pub status: String,
}

impl Default for AbTestingTuiState {
    fn default() -> Self {
        Self {
            controller: AbTestController::default(),
            step: AbTestingStep::Setup,
            path_a: None,
            path_b: None,
            trial_mode: TrialMode::Abx,
            level_match: LevelMatchConfig::default(),
            confidence: 50,
            status: "Capture Path A and Path B from the current plugin graph.".into(),
        }
    }
}
