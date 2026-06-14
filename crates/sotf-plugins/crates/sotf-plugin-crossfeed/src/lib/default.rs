use crate::params::PARAMS as CF;
use sotf_host::param_specs::find_by_key as pk;

pub(super) fn default_enabled() -> bool {
    true
}

pub(super) fn default_mix() -> f32 {
    pk(CF, "mix").default_f64() as f32
}

pub(super) fn default_bauer_fcut() -> f32 {
    pk(CF, "bauer_fcut_hz").default_f64() as f32
}

pub(super) fn default_bauer_feed() -> f32 {
    pk(CF, "bauer_feed_db").default_f64() as f32
}

pub(super) fn default_meier_level() -> f32 {
    pk(CF, "meier_level").default_f64() as f32
}

pub(super) fn default_mb_low_freq() -> f32 {
    pk(CF, "mb_low_freq_hz").default_f64() as f32
}

pub(super) fn default_mb_mid_high_freq() -> f32 {
    pk(CF, "mb_mid_high_freq_hz").default_f64() as f32
}

pub(super) fn default_mb_low_feed() -> f32 {
    pk(CF, "mb_low_feed_db").default_f64() as f32
}

pub(super) fn default_mb_mid_feed() -> f32 {
    pk(CF, "mb_mid_feed_db").default_f64() as f32
}

pub(super) fn default_mb_high_feed() -> f32 {
    pk(CF, "mb_high_feed_db").default_f64() as f32
}

pub(super) fn default_autogain_target() -> f32 {
    pk(CF, "autogain_target_lufs").default_f64() as f32
}

pub(super) fn default_autogain_max_gain() -> f32 {
    pk(CF, "autogain_max_gain_db").default_f64() as f32
}

pub(super) fn default_autogain_smoothing() -> f32 {
    pk(CF, "autogain_smoothing_ms").default_f64() as f32
}
