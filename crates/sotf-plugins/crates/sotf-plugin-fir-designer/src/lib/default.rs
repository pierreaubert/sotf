use crate::params::PARAMS;
use sotf_host::param_specs::find_by_key as pk;

pub(super) fn default_num_filters() -> usize {
    pk(PARAMS, "num_filters").default_f64() as usize
}

pub(super) fn default_fir_length_index() -> usize {
    pk(PARAMS, "fir_length").default_f64() as usize
}

pub(super) fn default_phase_mode_index() -> usize {
    pk(PARAMS, "phase_mode").default_f64() as usize
}

pub(super) fn default_mix() -> f32 {
    pk(PARAMS, "mix").default_f64() as f32
}

pub(super) fn default_filter_type() -> String {
    "Peak".to_string()
}

pub(super) fn default_frequency() -> f64 {
    1000.0
}

pub(super) fn default_q() -> f64 {
    1.0
}

pub(super) fn default_active() -> bool {
    true
}
