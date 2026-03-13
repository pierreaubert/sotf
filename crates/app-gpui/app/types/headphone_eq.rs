// ============================================================================
// Headphone EQ Screen Types
// ============================================================================
//
// Domain types are shared via the player crate. UI-specific state stays here.

use super::room_eq::{AutoEqField, OptimizationStatus};

// Re-export shared domain types from player crate
pub use sotf_audio_player::headphone_eq_types::{
    HeadphoneEqBiquad, HeadphoneEqOptimizerConfig, HeadphoneEqResult, HeadphoneEqStep,
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
    /// AutoEQ form editing state
    pub autoeq_editing_field: Option<AutoEqField>,
    /// AutoEQ form edit text
    pub autoeq_edit_text: String,
}

/// Complete Headphone EQ screen state
#[derive(Debug, Clone)]
pub struct HeadphoneEqState {
    /// Current step in the workflow
    pub step: HeadphoneEqStep,

    // === Step 1: Select Files ===
    /// Path to headphone measurement file (CSV)
    pub measurement_path: Option<String>,

    // === Goals & Configuration ===
    /// Loss function type ("flat" or "score")
    pub loss_type: String,
    /// Target curve selection (preset name or "custom")
    pub target_preset: String,
    /// Path to custom target file (if target_preset == "custom")
    pub custom_target_path: Option<String>,

    // === Step 2: Configuration ===
    /// Optimizer configuration
    pub optimizer_config: HeadphoneEqOptimizerConfig,

    // === Step 3: Optimization ===
    /// Current optimization status
    pub optimization_status: OptimizationStatus,
    /// Progress (0.0 - 1.0)
    pub progress: f32,
    /// Progress history for loss curve (iteration, loss)
    pub progress_history: Vec<(usize, f64)>,

    // === Step 4: Apply ===
    /// Optimization result (biquads, etc.)
    pub result: Option<HeadphoneEqResult>,
    /// Export format selection
    pub export_format: String,
    /// EQ preset name for saving
    pub save_name: String,

    // === UI State ===
    pub dropdowns: HeadphoneEqDropdowns,
    pub status_message: String,
    pub error_message: Option<String>,
    /// Expanded accordion sections
    pub expanded_sections: Vec<gpui::SharedString>,
}

impl Default for HeadphoneEqState {
    fn default() -> Self {
        Self {
            step: HeadphoneEqStep::MeasurementTarget,
            measurement_path: None,
            loss_type: "score".to_string(),
            target_preset: "harman-over-ear-2018".to_string(),
            custom_target_path: None,
            optimizer_config: HeadphoneEqOptimizerConfig::default(),
            optimization_status: OptimizationStatus::Idle,
            progress: 0.0,
            progress_history: Vec::new(),
            result: None,
            export_format: "json".to_string(),
            save_name: String::new(),
            dropdowns: HeadphoneEqDropdowns::default(),
            status_message: String::new(),
            error_message: None,
            expanded_sections: vec!["measurement".into(), "target".into(), "eq-design".into()],
        }
    }
}

impl HeadphoneEqState {
    pub fn ui_loss_type(&self) -> &'static str {
        match self.optimizer_config.loss.as_str() {
            "flat" | "headphone-flat" => "flat",
            "score" | "headphone-score" => "score",
            _ => "score",
        }
    }

    pub fn set_ui_loss_type(&mut self, loss_type: &str) {
        self.loss_type = match loss_type {
            "flat" | "headphone-flat" => "flat".to_string(),
            _ => "score".to_string(),
        };
        self.optimizer_config.loss = match loss_type {
            "flat" | "headphone-flat" => "headphone-flat".to_string(),
            _ => "headphone-score".to_string(),
        };
    }

    pub fn requires_custom_target_path(&self) -> bool {
        self.target_preset == "custom"
    }

    pub fn has_custom_target_path(&self) -> bool {
        self.custom_target_path
            .as_ref()
            .is_some_and(|path| !path.trim().is_empty())
    }

    /// Check if we can proceed from the current step
    pub fn can_advance(&self) -> bool {
        match self.step {
            HeadphoneEqStep::MeasurementTarget => self.measurement_path.is_some(),
            HeadphoneEqStep::Optimization => {
                self.optimization_status == OptimizationStatus::Completed
            }
            HeadphoneEqStep::Listen => self.result.is_some(),
            HeadphoneEqStep::Save => true,
        }
    }

    /// Check if optimization is running
    pub fn is_optimizing(&self) -> bool {
        self.optimization_status == OptimizationStatus::Running
    }

    /// Reset optimization state
    pub fn reset_optimization(&mut self) {
        self.optimization_status = OptimizationStatus::Idle;
        self.progress = 0.0;
        self.progress_history.clear();
        self.result = None;
        self.error_message = None;
    }
}
