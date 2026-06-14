use serde::{Deserialize, Serialize};

/// Target response configuration (UI-facing).
///
/// Mirrors the backend `autoeq::roomeq::TargetResponseConfig` but flattened
/// into a single struct for simpler binding in UI widgets. Covers the target
/// shape (flat / Harman / custom slope / file / derived-from-measurement),
/// the preference shelves (bass / treble), and the broadband pre-correction
/// toggle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetResponseUiConfig {
    /// Whether any target shaping is applied. When `false` the optimiser
    /// sees a flat target regardless of the other fields.
    pub enabled: bool,
    /// Target shape: "flat" | "harman" | "custom" | "file" | "from_measurement".
    pub shape: String,
    /// Slope in dB/octave (used when `shape == "custom"`).
    pub slope_db_per_octave: f64,
    /// Reference frequency where the slope passes through 0 dB.
    pub reference_freq: f64,
    /// Path to CSV target file (used when `shape == "file"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve_path: Option<std::path::PathBuf>,
    /// Bass shelf preference in dB (layered on top of the target shape).
    pub bass_shelf_db: f64,
    /// Bass shelf frequency in Hz.
    pub bass_shelf_freq: f64,
    /// Treble shelf preference in dB.
    pub treble_shelf_db: f64,
    /// Treble shelf frequency in Hz.
    pub treble_shelf_freq: f64,
    /// Enable broadband pre-correction (shelf+gain fit before fine EQ).
    pub broadband_precorrection: bool,
}

impl Default for TargetResponseUiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            shape: "harman".to_string(),
            slope_db_per_octave: -0.8,
            reference_freq: 1000.0,
            curve_path: None,
            bass_shelf_db: 0.0,
            bass_shelf_freq: 200.0,
            treble_shelf_db: 0.0,
            treble_shelf_freq: 8000.0,
            broadband_precorrection: false,
        }
    }
}
