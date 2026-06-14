use super::default::default_center_gain_db;
use super::default::default_height_gain_db;
use super::default::default_itu_mode;
use super::default::default_lfe_gain_db;
use super::default::default_matrix_ltrt;
use super::default::default_phase_blend_high_hz;
use super::default::default_phase_blend_low_hz;
use super::default::default_phase_coherence;
use super::default::default_surround_gain_db;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownmixPluginParams {
    pub input_channels: usize,
    #[serde(default = "default_center_gain_db")]
    pub center_gain_db: f32,
    #[serde(default = "default_surround_gain_db")]
    pub surround_gain_db: f32,
    #[serde(default = "default_height_gain_db")]
    pub height_gain_db: f32,
    #[serde(default = "default_lfe_gain_db")]
    pub lfe_gain_db: f32,
    #[serde(default = "default_phase_coherence")]
    pub phase_coherence: bool,
    #[serde(default = "default_phase_blend_low_hz")]
    pub phase_blend_low_hz: f32,
    #[serde(default = "default_phase_blend_high_hz")]
    pub phase_blend_high_hz: f32,
    /// When true, use ITU-R BS.775 standard downmix coefficients for 5.1→stereo
    #[serde(default = "default_itu_mode")]
    pub itu_mode: bool,
    /// When true, use matrix Lt/Rt encoding for surround channels
    #[serde(default = "default_matrix_ltrt", alias = "dolby_ltrt")]
    pub matrix_ltrt: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct DownmixCoeffs {
    pub(super) left_gain: f32,
    pub(super) right_gain: f32,
}
