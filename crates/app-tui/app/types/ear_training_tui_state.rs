use sotf_audio_player::{
    EarTrainingCourse, EarTrainingProgress, EqTrainingConfig, EqTrainingSession, GraphNodeId,
};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EarTrainingTab {
    #[default]
    Practice,
    Courses,
    Progress,
}

#[derive(Debug, Clone)]
pub struct EarTrainingTuiState {
    pub tab: EarTrainingTab,
    pub config: EqTrainingConfig,
    pub session: Option<EqTrainingSession>,
    pub selected_answer: usize,
    pub filtered: bool,
    pub progress: EarTrainingProgress,
    pub active_course: Option<EarTrainingCourse>,
    pub adaptive: bool,
    pub audition_node_id: Option<GraphNodeId>,
    pub sources: Vec<PathBuf>,
    pub source_index: usize,
    pub loop_enabled: bool,
    pub loop_range: Option<(f64, f64)>,
    pub course_selection: usize,
}

impl Default for EarTrainingTuiState {
    fn default() -> Self {
        let progress = sotf_audio_player::config::get_ear_training_progress_path()
            .and_then(|path| EarTrainingProgress::load(&path).ok())
            .unwrap_or_default();
        Self {
            tab: EarTrainingTab::Practice,
            config: EqTrainingConfig::default(),
            session: None,
            selected_answer: 0,
            filtered: false,
            progress,
            active_course: None,
            adaptive: false,
            audition_node_id: None,
            sources: Vec::new(),
            source_index: 0,
            loop_enabled: false,
            loop_range: None,
            course_selection: 0,
        }
    }
}

impl EarTrainingTuiState {
    pub fn answer_count(&self) -> usize {
        self.session
            .as_ref()
            .and_then(|session| {
                session.current_question.as_ref().map(|question| {
                    question
                        .answer_labels(session.config.exercise, &session.band_frequencies)
                        .len()
                })
            })
            .unwrap_or(0)
    }

    pub fn should_loop(&self, position_secs: f64) -> Option<f64> {
        let (start, end) = self.loop_range?;
        (self.loop_enabled && position_secs >= end).then_some(start)
    }
}
