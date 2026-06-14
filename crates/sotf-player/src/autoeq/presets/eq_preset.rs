use super::super::params::OptimizationParamsSerializable;
use super::consts::HEADPHONE_PRESETS;
use super::consts::ROOMEQ_PRESETS;
use super::consts::SPINORAMA_PRESETS;
use super::types::EqWorkflow;
use super::types::PresetParams;

/// A named preset that maps to a complete parameter bundle.
#[derive(Debug, Clone)]
pub struct EqPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub workflow: EqWorkflow,
    /// None means "Custom" -- user controls all parameters.
    pub(super) params: Option<PresetParams>,
}

impl EqPreset {
    /// Returns true if this is the "Custom" preset (no fixed params).
    pub fn is_custom(&self) -> bool {
        self.params.is_none()
    }

    /// Apply this preset's parameters onto a serializable config.
    /// Returns None for the Custom preset (caller keeps existing params).
    pub fn apply(&self) -> Option<OptimizationParamsSerializable> {
        let p = self.params.as_ref()?;
        let base = match self.workflow {
            EqWorkflow::Headphone => autoeq::Args::headphone_defaults(),
            EqWorkflow::Spinorama => autoeq::Args::speaker_defaults(),
            EqWorkflow::RoomEq => autoeq::Args::roomeq_defaults(),
        };
        let mut params = OptimizationParamsSerializable::from(&base);
        params.num_filters = p.num_filters;
        params.loss = p.loss.to_string();
        params.peq_model = p.peq_model.to_string();
        params.population = p.population;
        params.maxeval = p.maxeval;
        params.refine = p.refine;
        params.min_freq = p.min_freq;
        params.max_freq = p.max_freq;
        params.min_db = p.min_db;
        params.max_db = p.max_db;
        params.min_q = p.min_q;
        params.max_q = p.max_q;
        params.smooth = p.smooth;
        params.smooth_n = p.smooth_n;
        Some(params)
    }
}

/// Get all presets for a given workflow.
pub fn presets_for(workflow: EqWorkflow) -> &'static [EqPreset] {
    match workflow {
        EqWorkflow::Headphone => HEADPHONE_PRESETS,
        EqWorkflow::Spinorama => SPINORAMA_PRESETS,
        EqWorkflow::RoomEq => ROOMEQ_PRESETS,
    }
}

/// Find a preset by id within a workflow.
pub fn find_preset(workflow: EqWorkflow, id: &str) -> Option<&'static EqPreset> {
    presets_for(workflow).iter().find(|p| p.id == id)
}
