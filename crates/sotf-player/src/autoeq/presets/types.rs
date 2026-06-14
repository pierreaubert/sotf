use serde::{Deserialize, Serialize};

/// Which optimization workflow the user is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EqWorkflow {
    Headphone,
    Spinorama,
    RoomEq,
}

/// How much detail to show in the configuration UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DetailLevel {
    /// Preset selector + optimize button. No individual parameters.
    #[default]
    Simple,
    /// Curated parameter subset: goal, filter design, quality slider.
    Intermediate,
    /// Full parameter form for experts.
    Expert,
}

#[derive(Debug, Clone)]
pub(super) struct PresetParams {
    pub(super) num_filters: usize,
    pub(super) loss: &'static str,
    pub(super) peq_model: &'static str,
    pub(super) population: usize,
    pub(super) maxeval: usize,
    pub(super) refine: bool,
    pub(super) min_freq: f64,
    pub(super) max_freq: f64,
    pub(super) min_db: f64,
    pub(super) max_db: f64,
    pub(super) min_q: f64,
    pub(super) max_q: f64,
    pub(super) smooth: bool,
    pub(super) smooth_n: usize,
}
