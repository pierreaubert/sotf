use crate::params::{HPF_ORDERS, PARAMS as GT};
use sotf_host::param_specs::find_by_key as pk;

pub(super) fn default_threshold_db() -> f32 {
    pk(GT, "threshold").default_f64() as f32
}

pub(super) fn default_ratio() -> f32 {
    pk(GT, "ratio").default_f64() as f32
}

pub(super) fn default_attack_ms() -> f32 {
    pk(GT, "attack").default_f64() as f32
}

pub(super) fn default_hold_ms() -> f32 {
    pk(GT, "hold").default_f64() as f32
}

pub(super) fn default_release_ms() -> f32 {
    pk(GT, "release").default_f64() as f32
}

pub(super) fn default_mix() -> f32 {
    pk(GT, "mix").default_f64() as f32
}

pub(super) fn default_link_channels() -> bool {
    pk(GT, "link_channels").default_bool()
}

pub(super) fn default_sidechain_hpf_hz() -> f32 {
    pk(GT, "sidechain_hpf_hz").default_f64() as f32
}

pub(super) fn default_sidechain_hpf_order() -> String {
    HPF_ORDERS[0].to_string()
}

pub(super) fn default_detection_mode() -> String {
    "peak".to_string()
}

pub(super) fn default_sidechain_external() -> bool {
    pk(GT, "sidechain_external").default_bool()
}

pub(super) fn default_range_db() -> f32 {
    80.0
}
