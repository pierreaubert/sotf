use crate::app::types::OptimizationUiState;

/// Speaker optimization workflow state
#[derive(Debug)]
pub struct SpeakerOptState {
    pub model: String,
    pub params: sotf_audio_player::autoeq::OptimizationParams,
    pub running: bool,
    pub progress: Vec<(usize, f64)>,
    pub result: Option<sotf_audio_player::autoeq::SpeakerOptimizationResult>,
    pub export_format: String,
    pub ui: OptimizationUiState,
}

impl Default for SpeakerOptState {
    fn default() -> Self {
        Self {
            model: String::new(),
            params: sotf_audio_player::autoeq::OptimizationParams::speaker_defaults(),
            running: false,
            progress: Vec::new(),
            result: None,
            export_format: String::from("json"),
            ui: OptimizationUiState::default(),
        }
    }
}
