use crate::params::PARAMS as SC;
use sotf_host::param_specs::find_by_key as pk;

pub(super) fn default_fft_size_index() -> usize {
    pk(SC, "fft_size").default_f64() as usize
}

pub(super) fn default_threshold() -> f32 {
    pk(SC, "threshold").default_f64() as f32
}

pub(super) fn default_ratio() -> f32 {
    pk(SC, "ratio").default_f64() as f32
}

pub(super) fn default_attack() -> f32 {
    pk(SC, "attack").default_f64() as f32
}

pub(super) fn default_release() -> f32 {
    pk(SC, "release").default_f64() as f32
}

pub(super) fn default_knee() -> f32 {
    pk(SC, "knee").default_f64() as f32
}

pub(super) fn default_spectral_smoothing() -> f32 {
    pk(SC, "spectral_smoothing").default_f64() as f32
}

pub(super) fn default_mix() -> f32 {
    pk(SC, "mix").default_f64() as f32
}
