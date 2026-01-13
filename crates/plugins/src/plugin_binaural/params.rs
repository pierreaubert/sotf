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
    true // Enable by default for better timbre
}

fn default_lfe_crossover() -> f32 {
    120.0 // Hz - typical subwoofer crossover
}

fn default_lfe_distance() -> f32 {
    2.0 // meters - typical subwoofer distance in home theater
}

fn default_lfe_level() -> f32 {
    0.0 // dB - no additional boost/cut by default
}

/// Configuration parameters for BinauralDecoderPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinauralDecoderParams {
    /// Path to HRTF file containing HRTFs (.sofa or .polar)
    #[serde(default = "default_hrtf_path")]
    pub hrtf_file: String,
    /// FFT size for convolution (must be power of 2)
    #[serde(default = "default_fft_size")]
    pub fft_size: usize,
    /// Number of input channels
    pub input_channels: usize,
    /// Enable Sum-Before-IFFT optimization
    #[serde(default = "default_enable_optimization")]
    pub enable_optimization: bool,
    /// Externalization factor (0.0 to 1.0)
    #[serde(default = "default_externalization")]
    pub externalization: f32,
    /// Near-field shadowing strength (0.0 to 1.0)
    #[serde(default = "default_near_field_strength")]
    pub near_field_strength: f32,
    /// Enable diffuse-field equalization to compensate for HRTF coloration
    #[serde(default = "default_diffuse_field_eq")]
    pub diffuse_field_eq: bool,
    /// LFE low-pass crossover frequency in Hz
    #[serde(default = "default_lfe_crossover")]
    pub lfe_crossover: f32,
    /// LFE (subwoofer) distance in meters for distance attenuation
    #[serde(default = "default_lfe_distance")]
    pub lfe_distance: f32,
    /// LFE level adjustment in dB
    #[serde(default = "default_lfe_level")]
    pub lfe_level: f32,
    /// Room model for externalization (optional, uses defaults if not specified)
    #[serde(default)]
    pub room_model: RoomModel,
}
