// ============================================================================
// Spinorama EQ Screen Types
// ============================================================================

use sotf_audio_player::ui_models::spinorama_eq::SpinoramaEqScreenModel;
use std::ops::{Deref, DerefMut};

use super::room_eq::AutoEqField;

// Re-export shared domain types from player crate
pub use sotf_audio_player::spinorama_eq_types::{
    SpinoramaBiquad, SpinoramaEqResult, SpinoramaOptimizationMode, SpinoramaOptimizerConfig,
    SpinoramaStep, SpinoramaTargetCurve,
};
pub use sotf_audio_player::ui_models::spinorama_eq::{DirectivityCurve, SpinoramaCurves};

/// UI state for Spinorama EQ dropdowns
#[derive(Debug, Clone)]
pub struct SpinoramaEqDropdowns {
    pub version_open: bool,
    pub measurement_open: bool,
    pub curve_open: bool,
    pub mode_open: bool,
    pub algorithm_open: bool,
    pub export_format_open: bool,
    /// Target curve dropdown (ON, LW, PIR, ER)
    pub target_curve_open: bool,
    /// AutoEQ form: EQ mode dropdown (IIR/FIR)
    pub opt_mode_open: bool,
    /// Selected EQ mode ("iir", "fir", "mixed")
    pub opt_mode: String,
    /// AutoEQ form: FIR phase dropdown
    pub fir_phase_open: bool,
    /// AutoEQ form: PEQ model dropdown
    pub peq_model_open: bool,
    /// AutoEQ form: DE strategy dropdown
    pub strategy_open: bool,
    /// AutoEQ form: local algorithm dropdown
    pub local_algo_open: bool,
    /// AutoEQ form: BO acquisition dropdown
    pub bo_acquisition_open: bool,
    /// AutoEQ form: loss type dropdown
    pub loss_type_open: bool,
    /// AutoEQ form editing state
    pub autoeq_editing_field: Option<AutoEqField>,
    /// AutoEQ form edit text
    pub autoeq_edit_text: String,
}

impl Default for SpinoramaEqDropdowns {
    fn default() -> Self {
        Self {
            version_open: false,
            measurement_open: false,
            curve_open: false,
            mode_open: false,
            algorithm_open: false,
            export_format_open: false,
            target_curve_open: false,
            opt_mode_open: false,
            opt_mode: "iir".to_string(),
            fir_phase_open: false,
            peq_model_open: false,
            strategy_open: false,
            local_algo_open: false,
            bo_acquisition_open: false,
            loss_type_open: false,
            autoeq_editing_field: None,
            autoeq_edit_text: String::new(),
        }
    }
}

/// GPUI-specific UI state for the Spinorama EQ wizard.
///
/// Domain state lives in the embedded [`SpinoramaEqScreenModel`]; this struct
/// only holds view state that is specific to the GPUI shell.
#[derive(Debug, Clone)]
pub struct SpinoramaEqState {
    /// Shared, UI-agnostic Spinorama EQ wizard domain model.
    pub model: SpinoramaEqScreenModel,

    /// Dropdown open states and edit buffers.
    pub dropdowns: SpinoramaEqDropdowns,
    /// Detail level for the configuration form (Simple / Intermediate / Expert).
    pub detail_level: sotf_audio_player::autoeq::DetailLevel,
    /// Currently selected preset id.
    pub selected_preset: String,
    /// Expanded accordion sections.
    pub expanded_sections: Vec<gpui::SharedString>,
    /// Focus handle for the search input.
    pub search_focus_handle: Option<gpui::FocusHandle>,
}

impl Default for SpinoramaEqState {
    fn default() -> Self {
        Self {
            model: SpinoramaEqScreenModel::default(),
            dropdowns: SpinoramaEqDropdowns::default(),
            detail_level: sotf_audio_player::autoeq::DetailLevel::Simple,
            selected_preset: "balanced".to_string(),
            expanded_sections: vec!["speaker".into(), "options".into()],
            search_focus_handle: None,
        }
    }
}

impl Deref for SpinoramaEqState {
    type Target = SpinoramaEqScreenModel;

    fn deref(&self) -> &Self::Target {
        &self.model
    }
}

impl DerefMut for SpinoramaEqState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.model
    }
}

impl SpinoramaEqState {
    /// Reset optimization state.
    pub fn reset_optimization(&mut self) {
        self.model.reset_optimization();
    }

    /// Check if we can proceed from the current step.
    pub fn can_advance(&self) -> bool {
        self.model.can_advance()
    }

    /// Check if optimization is running.
    pub fn is_optimizing(&self) -> bool {
        self.model.is_optimizing()
    }

    /// Update speaker suggestions based on search query with fuzzy matching.
    pub fn update_suggestions(&mut self) {
        self.model.update_suggestions();
    }

    /// Check if speakers cache needs to be refreshed (older than 1 hour or not loaded).
    pub fn needs_speaker_refresh(&self) -> bool {
        self.model.needs_speaker_refresh()
    }

    pub fn supported_eq_modes(&self) -> &'static [&'static str] {
        self.model.supported_eq_modes()
    }

    pub fn selected_eq_mode(&self) -> &'static str {
        self.model.selected_eq_mode()
    }

    pub fn set_selected_eq_mode(&mut self, mode: &str) {
        self.model.set_selected_eq_mode(mode);
        self.dropdowns.opt_mode = "iir".to_string();
    }
}
