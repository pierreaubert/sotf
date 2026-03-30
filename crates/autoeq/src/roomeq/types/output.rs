//! Room EQ Output Types
//!
//! Types for returning optimization results and DSP chain outputs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Re-export Curve for reference in output docs
pub use crate::Curve;

// ============================================================================
// Frequency Response Curve Data
// ============================================================================

/// Frequency response curve data for serialization
///
/// Represents a curve with frequency points and SPL values.
/// SPL values are normalized (mean-subtracted in the 1000-2000 Hz range)
/// for consistent comparison across measurements.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CurveData {
    /// Frequency points in Hz
    pub freq: Vec<f64>,
    /// Sound Pressure Level in dB (normalized)
    pub spl: Vec<f64>,
    /// Phase in degrees (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<Vec<f64>>,
    /// Optional frequency range used for normalization
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub norm_range: Option<(f64, f64)>,
}

impl From<Curve> for CurveData {
    fn from(curve: Curve) -> Self {
        CurveData {
            freq: curve.freq.to_vec(),
            spl: curve.spl.to_vec(),
            phase: curve.phase.map(|p| p.to_vec()),
            norm_range: None,
        }
    }
}

impl From<&Curve> for CurveData {
    fn from(curve: &Curve) -> Self {
        CurveData {
            freq: curve.freq.to_vec(),
            spl: curve.spl.to_vec(),
            phase: curve.phase.as_ref().map(|p| p.to_vec()),
            norm_range: None,
        }
    }
}

impl From<CurveData> for Curve {
    fn from(data: CurveData) -> Self {
        Curve {
            freq: ndarray::Array1::from(data.freq),
            spl: ndarray::Array1::from(data.spl),
            phase: data.phase.map(ndarray::Array1::from),
        }
    }
}

// ============================================================================
// Impulse Response Waveform
// ============================================================================

/// Impulse response waveform (time-domain)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IrWaveform {
    /// Time axis in milliseconds
    pub time_ms: Vec<f64>,
    /// Amplitude (normalized so pre-IR peak = 1.0)
    pub amplitude: Vec<f64>,
}

// ============================================================================
// DSP Chain Types
// ============================================================================

/// DSP chain output (AudioEngine PluginConfig format)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DspChainOutput {
    /// Output version
    #[serde(default = "crate::roomeq::types::default_config_version")]
    pub version: String,
    /// Per-channel DSP chains
    pub channels: HashMap<String, ChannelDspChain>,
    /// Metadata about the optimization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<OptimizationMetadata>,
}

/// DSP chain for a single channel
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ChannelDspChain {
    /// Channel name
    pub channel: String,
    /// Ordered list of plugins (AudioEngine PluginConfig format)
    pub plugins: Vec<PluginConfigWrapper>,
    /// Per-driver DSP chains for active crossover (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drivers: Option<Vec<DriverDspChain>>,
    /// Initial frequency response curve before optimization (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_curve: Option<CurveData>,
    /// Final frequency response curve after applying correction (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_curve: Option<CurveData>,
    /// EQ filter response curve (correction magnitude in dB) (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eq_response: Option<CurveData>,
    /// Effective target curve the optimizer worked against (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_curve: Option<CurveData>,
    /// Impulse response before correction (optional, requires phase data)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_ir: Option<IrWaveform>,
    /// Impulse response after correction (optional, requires phase data)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_ir: Option<IrWaveform>,
}

/// DSP chain for an individual driver in a multi-driver speaker
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DriverDspChain {
    /// Driver name (e.g. "woofer", "tweeter")
    pub name: String,
    /// Driver index in the array (0 = lowest frequency)
    pub index: usize,
    /// Ordered list of plugins for this driver (gain, crossover filters)
    pub plugins: Vec<PluginConfigWrapper>,
    /// Initial frequency response curve for this driver before optimization (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_curve: Option<CurveData>,
}

/// Wrapper for AudioEngine PluginConfig (re-exported from src-audio)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginConfigWrapper {
    pub plugin_type: String,
    pub parameters: serde_json::Value,
}

/// Optimization metadata
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OptimizationMetadata {
    /// Pre-optimization score
    pub pre_score: f64,
    /// Post-optimization score
    pub post_score: f64,
    /// Optimization algorithm used
    pub algorithm: String,
    /// Number of iterations
    pub iterations: usize,
    /// Timestamp
    pub timestamp: String,
    /// Inter-channel deviation metric (computed when >1 channel)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inter_channel_deviation: Option<crate::roomeq::types::InterChannelDeviation>,
}
