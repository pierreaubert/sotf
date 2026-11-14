use super::eq_preset::presets_for;
use super::types::EqWorkflow;

/// Get the default preset id for a workflow.
#[must_use]
pub fn default_preset_id(workflow: EqWorkflow) -> &'static str {
    match workflow {
        EqWorkflow::Headphone | EqWorkflow::Spinorama => "balanced",
        EqWorkflow::RoomEq => "full-range",
    }
}

/// Preset dropdown options as `(id, label)` pairs for UI consumption.
#[must_use]
pub fn preset_options(workflow: EqWorkflow) -> Vec<(&'static str, &'static str)> {
    presets_for(workflow)
        .iter()
        .map(|p| (p.id, p.name))
        .collect()
}
