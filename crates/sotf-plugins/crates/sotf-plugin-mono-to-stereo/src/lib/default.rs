use crate::params::PARAMS as MS;
use sotf_host::param_specs::find_by_key as pk;

pub(super) fn default_stereo_width() -> f32 {
    pk(MS, "stereo_width").default_f64() as f32
}

pub(super) fn default_freq_dependent() -> bool {
    pk(MS, "freq_dependent").default_bool()
}

pub(super) fn default_haas_delay_ms() -> f32 {
    pk(MS, "haas_delay_ms").default_f64() as f32
}

pub(super) fn default_decor_low_hz() -> f32 {
    pk(MS, "decor_low_hz").default_f64() as f32
}

pub(super) fn default_decor_high_hz() -> f32 {
    pk(MS, "decor_high_hz").default_f64() as f32
}
