use super::SpinUpdateSubStep;
use super::spinorama_step::SpinoramaStep;
use sotf_audio_player::room_eq_types::OptimizationStatus;
use sotf_audio_player::spinorama_eq_types::SpinoramaOptimizerConfig;
use sotf_audio_player::ui_models::spinorama_eq::SpinoramaEqScreenModel;

/// TUI state for the Spinorama EQ wizard.
///
/// Domain state (speaker selection, optimizer config, optimization
/// progress/results, curves) lives in the shared [`SpinoramaEqScreenModel`]
/// from `sotf-player`; this struct only holds view state that is specific to
/// the terminal UI.
#[derive(Debug, Clone)]
pub struct SpinoramaEqTuiState {
    /// Shared, UI-agnostic Spinorama EQ wizard domain model.
    pub model: SpinoramaEqScreenModel,

    /// Current step in the TUI workflow.
    pub step: SpinoramaStep,
    /// When true, the wizard step tab bar has focus (Left/Right change step).
    pub step_tab_focused: bool,

    // Step 1: speaker selection
    pub selected_speaker_idx: usize,
    pub speakers_error: Option<String>,

    // Step 2: configuration
    pub selected_field: usize, // which config field is selected
    /// True when a numerical field is being directly edited via keyboard.
    pub editing_value: bool,
    pub edit_buffer: String,

    // Step 3: optimization progress
    pub opt_max_iter: usize,

    // Step 5: update plugin confirmation
    pub update_substep: SpinUpdateSubStep,
    /// (slot_index, filter_count) of existing EQ to overwrite.
    pub update_existing_eq_info: Option<(usize, usize)>,
}

impl Default for SpinoramaEqTuiState {
    fn default() -> Self {
        // TUI uses slightly different defaults than GPUI.
        let optimizer_config = SpinoramaOptimizerConfig {
            population: 50,
            smooth: true,
            smooth_n: 1,
            spacing_weight: 20.0,
            min_spacing_oct: 0.5,
            tolerance: 1e-3,
            atolerance: 1e-4,
            ..SpinoramaOptimizerConfig::default()
        };
        let model = SpinoramaEqScreenModel {
            optimizer_config,
            ..SpinoramaEqScreenModel::default()
        };
        Self {
            model,
            step: SpinoramaStep::Select,
            step_tab_focused: false,
            selected_speaker_idx: 0,
            speakers_error: None,
            selected_field: 0,
            editing_value: false,
            edit_buffer: String::new(),
            opt_max_iter: 0,
            update_substep: SpinUpdateSubStep::Ready,
            update_existing_eq_info: None,
        }
    }
}

impl SpinoramaEqTuiState {
    /// Update filtered speakers based on search query.
    pub fn update_filter(&mut self) {
        self.model.update_suggestions();
        // Clamp index
        if !self.model.speaker_suggestions.is_empty() {
            self.selected_speaker_idx = self
                .selected_speaker_idx
                .min(self.model.speaker_suggestions.len() - 1);
        } else {
            self.selected_speaker_idx = 0;
        }
    }

    /// Reset optimization state.
    pub fn reset_optimization(&mut self) {
        self.model.reset_optimization();
        self.opt_max_iter = 0;
    }

    /// Check if optimization is running.
    pub fn is_optimizing(&self) -> bool {
        self.model.optimization_status == OptimizationStatus::Running
    }
}
