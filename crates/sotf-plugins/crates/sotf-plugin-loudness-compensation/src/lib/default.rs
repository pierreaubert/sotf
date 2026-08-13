use crate::params::PARAMS as LC;
use sotf_host::param_specs::find_by_key as pk;

pub(super) fn default_low_freq() -> f32 {
    pk(LC, "low_freq").default_f32()
}

pub(super) fn default_low_gain() -> f32 {
    pk(LC, "low_gain").default_f32()
}

pub(super) fn default_high_freq() -> f32 {
    pk(LC, "high_freq").default_f32()
}

pub(super) fn default_high_gain() -> f32 {
    pk(LC, "high_gain").default_f32()
}

pub(super) fn default_mid_freq() -> f32 {
    pk(LC, "mid_freq").default_f32()
}

pub(super) fn default_mid_gain() -> f32 {
    pk(LC, "mid_gain").default_f32()
}

pub(super) fn default_mid_q() -> f32 {
    pk(LC, "mid_q").default_f32()
}

pub(super) fn default_mid_enabled() -> bool {
    true
}

pub(super) fn default_auto_gain_position() -> String {
    "post".to_string()
}

pub(super) fn default_auto_gain_enabled() -> bool {
    pk(LC, "auto_gain_enabled").default_bool()
}

pub(super) fn default_auto_gain_max_db() -> f32 {
    pk(LC, "auto_gain_max_db").default_f32()
}

pub(super) fn default_auto_gain_smoothing_ms() -> f32 {
    pk(LC, "auto_gain_smoothing_ms").default_f32()
}

pub(super) fn default_playback_level_db() -> f32 {
    pk(LC, "playback_level_db").default_f32()
}

pub(super) fn default_reference_level_db() -> f32 {
    pk(LC, "reference_level_db").default_f32()
}

pub(super) fn default_playback_volume_db() -> f32 {
    pk(LC, "playback_volume_db").default_f32()
}

pub(super) fn default_headroom_normalized() -> bool {
    false
}

pub(super) fn default_auto_calibrated() -> bool {
    false
}

pub(super) fn default_fm_compat_reference() -> f32 {
    -14.0
}
