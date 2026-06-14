use crate::params::PARAMS as DM;
use sotf_host::param_specs::find_by_key as pk;

pub(super) fn default_center_gain_db() -> f32 {
    pk(DM, "center_gain_db").default_f64() as f32
}

pub(super) fn default_surround_gain_db() -> f32 {
    pk(DM, "surround_gain_db").default_f64() as f32
}

pub(super) fn default_height_gain_db() -> f32 {
    pk(DM, "height_gain_db").default_f64() as f32
}

pub(super) fn default_lfe_gain_db() -> f32 {
    pk(DM, "lfe_gain_db").default_f64() as f32
}

pub(super) fn default_phase_coherence() -> bool {
    pk(DM, "phase_coherence").default_bool()
}

pub(super) fn default_phase_blend_low_hz() -> f32 {
    pk(DM, "phase_blend_low_hz").default_f64() as f32
}

pub(super) fn default_phase_blend_high_hz() -> f32 {
    pk(DM, "phase_blend_high_hz").default_f64() as f32
}

pub(super) fn default_itu_mode() -> bool {
    pk(DM, "itu_mode").default_bool()
}

pub(super) fn default_matrix_ltrt() -> bool {
    false
}
