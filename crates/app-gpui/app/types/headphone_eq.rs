// ============================================================================
// Headphone EQ Screen Types
// ============================================================================
//
// Domain types are shared via the player crate. UI-specific state stays here.

use sotf_audio_player::PluginGraph;
use sotf_audio_player::autoeq::HeadphoneEasyApplyOutcome;
use sotf_audio_player::ui_models::headphone_eq::HeadphoneEqScreenModel;
use std::ops::{Deref, DerefMut};

#[cfg(feature = "dev-api")]
use std::collections::HashMap;

use super::room_eq::AutoEqField;

// Re-export shared domain types from player crate
pub use sotf_audio_player::headphone_eq_types::{
    HeadphoneEqBiquad, HeadphoneEqOptimizerConfig, HeadphoneEqResult, HeadphoneEqStep,
    HeadphoneMeasurementSource,
};

/// UI state for Headphone EQ dropdowns
#[derive(Debug, Clone, Default)]
pub struct HeadphoneEqDropdowns {
    pub target_open: bool,
    pub algorithm_open: bool,
    pub peq_model_open: bool,
    pub export_format_open: bool,
    pub loss_type_open: bool,
    pub target_curve_open: bool,
    pub strategy_open: bool,
    pub local_algo_open: bool,
    pub bo_acquisition_open: bool,
    /// AutoEQ form editing state
    pub autoeq_editing_field: Option<AutoEqField>,
    /// AutoEQ form edit text
    pub autoeq_edit_text: String,
}

/// Complete Headphone EQ screen state.
///
/// Domain fields live in the shared [`HeadphoneEqScreenModel`]; this struct
/// only holds view state that is specific to the GPUI shell.
#[derive(Debug, Clone)]
pub struct HeadphoneEqState {
    /// Shared, UI-agnostic Headphone EQ wizard domain model.
    pub model: HeadphoneEqScreenModel,

    // === UI State ===
    pub dropdowns: HeadphoneEqDropdowns,
    /// Detail level for the configuration form (Simple / Intermediate / Expert)
    pub detail_level: sotf_audio_player::autoeq::DetailLevel,
    /// Currently selected preset id
    pub selected_preset: String,
    /// Expanded accordion sections
    pub expanded_sections: Vec<gpui::SharedString>,
    /// Exact graph snapshot retained until the easy-mode apply is undone or
    /// replaced by another easy-mode apply.
    pub easy_mode_undo_graph: Option<PluginGraph>,
    /// Safety and calibration summary for the currently applied easy chain.
    pub easy_mode_last_apply: Option<HeadphoneEasyApplyOutcome>,
    /// Monotonic request IDs ensure stale network completions cannot overwrite
    /// a newer Headphone EQ request.
    pub headphone_list_request_id: u64,
    pub download_request_id: u64,
    /// Deterministic discovery responses used only by the dev-api fixture
    /// runner. Shipping builds neither contain nor consult this data.
    #[cfg(feature = "dev-api")]
    pub qa_discovery_fixture: Option<QaHeadphoneDiscoveryFixture>,
    /// Last export written by the dev-api build, retained for rendered QA
    /// assertions. Production builds do not contain this observability state.
    #[cfg(feature = "dev-api")]
    pub qa_last_export_path: Option<std::path::PathBuf>,
}

/// Hermetic responses for the real Headphone EQ discovery controls.
#[cfg(feature = "dev-api")]
#[derive(Debug, Clone, Default)]
pub struct QaHeadphoneDiscoveryFixture {
    pub catalog: Vec<String>,
    pub downloads: HashMap<String, QaHeadphoneDownloadFixture>,
}

/// A single deterministic fixture download. Delays and failures exercise the
/// same asynchronous state transitions as production network requests.
#[cfg(feature = "dev-api")]
#[derive(Debug, Clone)]
pub struct QaHeadphoneDownloadFixture {
    pub path: String,
    pub curve: Vec<(f64, f64)>,
    pub delay_ms: u64,
    pub failures_remaining: usize,
    pub failure_message: String,
}

impl Default for HeadphoneEqState {
    fn default() -> Self {
        Self {
            model: HeadphoneEqScreenModel::default(),
            dropdowns: HeadphoneEqDropdowns::default(),
            detail_level: sotf_audio_player::autoeq::DetailLevel::Simple,
            selected_preset: "balanced".to_string(),
            expanded_sections: vec!["measurement".into(), "target".into(), "eq-design".into()],
            easy_mode_undo_graph: None,
            easy_mode_last_apply: None,
            headphone_list_request_id: 0,
            download_request_id: 0,
            #[cfg(feature = "dev-api")]
            qa_discovery_fixture: None,
            #[cfg(feature = "dev-api")]
            qa_last_export_path: None,
        }
    }
}

impl Deref for HeadphoneEqState {
    type Target = HeadphoneEqScreenModel;

    fn deref(&self) -> &Self::Target {
        &self.model
    }
}

impl DerefMut for HeadphoneEqState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.model
    }
}

impl HeadphoneEqState {
    pub fn begin_headphone_list_request(&mut self) -> u64 {
        self.headphone_list_request_id = self.headphone_list_request_id.wrapping_add(1);
        self.headphone_list_request_id
    }

    pub fn begin_download_request(&mut self) -> u64 {
        self.download_request_id = self.download_request_id.wrapping_add(1);
        self.download_request_id
    }

    /// Check if we can proceed from the current step.
    pub fn can_advance(&self) -> bool {
        self.model.can_advance()
    }

    /// Check if optimization is running.
    pub fn is_optimizing(&self) -> bool {
        self.model.is_optimizing()
    }

    /// Reset optimization state.
    pub fn reset_optimization(&mut self) {
        self.model.reset_optimization();
    }

    /// Update headphone suggestions based on search query with fuzzy matching.
    pub fn update_headphone_suggestions(&mut self) {
        self.model.update_headphone_suggestions();
    }

    /// Check if headphones cache needs refresh (older than 1 hour or not loaded).
    pub fn needs_headphone_refresh(&self) -> bool {
        self.model.needs_headphone_refresh()
    }
}
