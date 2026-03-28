//! Binaural decoder plugin configuration (construction parameters).
//!
//! This struct is used to construct the DSP plugin from JSON.
//! It contains all internal settings (FFT size, HRTF path, room model, etc.)
//! that go beyond the user-editable PARAMS.

use super::room::RoomModel;
use serde::{Deserialize, Serialize};

fn default_fft_size() -> usize {
    2048
}

fn default_hrtf_path() -> String {
    "".to_string()
}

pub fn default_enable_optimization() -> bool {
    true
}

fn default_externalization() -> f32 {
    0.0
}

fn default_near_field_strength() -> f32 {
    0.0
}

fn default_diffuse_field_eq() -> bool {
    true
}

fn default_lfe_crossover() -> f32 {
    120.0
}

fn default_lfe_distance() -> f32 {
    2.0
}

fn default_lfe_level() -> f32 {
    0.0
}

fn default_hrtf_database_dir() -> String {
    "".to_string()
}

fn default_srir_file() -> String {
    "".to_string()
}

fn default_head_width_cm() -> f32 {
    15.0
}

fn default_ear_height_cm() -> f32 {
    10.0
}

/// Configuration parameters for BinauralDecoderPlugin.
///
/// Used by the engine to construct the DSP plugin from JSON.
/// User-editable parameter specs are in `params.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinauralDecoderParams {
    #[serde(default = "default_hrtf_path")]
    pub hrtf_file: String,
    #[serde(default = "default_fft_size")]
    pub fft_size: usize,
    pub input_channels: usize,
    #[serde(default = "default_enable_optimization")]
    pub enable_optimization: bool,
    #[serde(default = "default_externalization")]
    pub externalization: f32,
    #[serde(default = "default_near_field_strength")]
    pub near_field_strength: f32,
    #[serde(default = "default_diffuse_field_eq")]
    pub diffuse_field_eq: bool,
    #[serde(default = "default_lfe_crossover")]
    pub lfe_crossover: f32,
    #[serde(default = "default_lfe_distance")]
    pub lfe_distance: f32,
    #[serde(default = "default_lfe_level")]
    pub lfe_level: f32,
    #[serde(default)]
    pub room_model: RoomModel,
    /// Path to a measured Spatial Room Impulse Response (WAV file).
    /// When set, SSIR analysis replaces the synthetic ISM room model.
    /// Supports mono (energy-based) or B-format (4+ channels, full DOA).
    #[serde(default = "default_srir_file")]
    pub srir_file: String,
    #[serde(default = "default_hrtf_database_dir")]
    pub hrtf_database_dir: String,
    #[serde(default = "default_head_width_cm")]
    pub head_width_cm: f32,
    #[serde(default = "default_ear_height_cm")]
    pub ear_height_cm: f32,
}
