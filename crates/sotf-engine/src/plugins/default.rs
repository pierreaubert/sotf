use super::default_dyneq_num_bands;
use sotf_plugins::param_specs::de_esser as de_esser_specs;
use sotf_plugins::{DynEqBandParams, SpectralTiltCorrection, TiltReferenceFreq};

pub(super) fn default_de_esser_mode() -> String {
    de_esser_specs::MODES[1].to_string()
}

pub(super) fn default_fm_reference_level_db() -> f64 {
    -14.0
}

pub(super) fn default_fm_enabled() -> bool {
    true
}

pub(super) fn default_fm_smoothing_ms() -> f64 {
    30.0
}

pub(super) fn default_fm_auto_gain_max_db() -> f64 {
    12.0
}

pub(super) fn default_fm_auto_gain_smoothing_ms() -> f64 {
    100.0
}

pub(super) fn default_fm_band1_freq() -> f64 {
    60.0
}

pub(super) fn default_fm_band1_q() -> f64 {
    0.5
}

pub(super) fn default_fm_band1_max_gain() -> f64 {
    15.0
}

pub(super) fn default_fm_band1_slope() -> f64 {
    0.6
}

pub(super) fn default_fm_band2_freq() -> f64 {
    250.0
}

pub(super) fn default_fm_band2_q() -> f64 {
    0.707
}

pub(super) fn default_fm_band2_max_gain() -> f64 {
    8.0
}

pub(super) fn default_fm_band2_slope() -> f64 {
    0.4
}

pub(super) fn default_fm_band3_freq() -> f64 {
    3500.0
}

pub(super) fn default_fm_band3_q() -> f64 {
    1.0
}

pub(super) fn default_fm_band3_max_gain() -> f64 {
    4.0
}

pub(super) fn default_fm_band3_slope() -> f64 {
    0.2
}

pub(super) fn default_fm_band4_freq() -> f64 {
    12000.0
}

pub(super) fn default_fm_band4_q() -> f64 {
    0.707
}

pub(super) fn default_fm_band4_max_gain() -> f64 {
    6.0
}

pub(super) fn default_fm_band4_slope() -> f64 {
    0.3
}

pub(super) fn default_ab_path_config() -> String {
    r#"{"type":"None"}"#.to_string()
}

pub(super) fn default_spectrum_tilt_correction() -> SpectralTiltCorrection {
    SpectralTiltCorrection::None
}

pub(super) fn default_spectrum_tilt_reference() -> TiltReferenceFreq {
    TiltReferenceFreq::Standard
}

pub(super) fn default_channels() -> usize {
    2
}

pub(super) fn default_max_filters() -> usize {
    10
}

pub(super) fn default_dyneq_bands() -> Vec<DynEqBandParams> {
    (0..default_dyneq_num_bands() as usize)
        .map(|_| DynEqBandParams::default())
        .collect()
}
