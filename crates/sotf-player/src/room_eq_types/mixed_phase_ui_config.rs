use serde::{Deserialize, Serialize};

/// Mixed-phase correction configuration (IIR for minimum-phase + short FIR for excess phase)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixedPhaseUiConfig {
    /// Maximum FIR length in milliseconds for excess phase correction (default: 10.0)
    pub max_fir_length_ms: f64,
    /// Pre-ringing threshold in dB (default: -30.0)
    pub pre_ringing_threshold_db: f64,
    /// Minimum spatial correction depth (default: 0.5)
    pub min_spatial_depth: f64,
    /// Phase smoothing width in octaves (default: 0.167 = 1/6 octave)
    pub phase_smoothing_octaves: f64,
}

impl Default for MixedPhaseUiConfig {
    fn default() -> Self {
        Self {
            max_fir_length_ms: 10.0,
            pre_ringing_threshold_db: -30.0,
            min_spatial_depth: 0.5,
            phase_smoothing_octaves: 0.167,
        }
    }
}
