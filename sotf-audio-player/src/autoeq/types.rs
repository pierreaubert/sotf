//! Room EQ Types
//!
//! Types for room EQ optimization.

use autoeq_iir::Biquad;
use ndarray::Array1;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

// ============================================================================
// Curve Type (simplified from autoeq)
// ============================================================================

/// A frequency response curve
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Curve {
    /// Frequency points in Hz
    pub freq: Array1<f64>,
    /// SPL values in dB
    pub spl: Array1<f64>,
}

impl Curve {
    /// Create a new curve from frequency and SPL arrays
    pub fn new(freq: Array1<f64>, spl: Array1<f64>) -> Self {
        Self { freq, spl }
    }

    /// Create a flat curve at 0 dB
    pub fn flat(freq: Array1<f64>) -> Self {
        let spl = Array1::zeros(freq.len());
        Self { freq, spl }
    }

    /// Number of points in the curve
    pub fn len(&self) -> usize {
        self.freq.len()
    }

    /// Check if the curve is empty
    pub fn is_empty(&self) -> bool {
        self.freq.is_empty()
    }
}

// ============================================================================
// Crossover Types
// ============================================================================

/// Crossover filter type for multi-driver speakers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CrossoverType {
    /// Butterworth 12 dB/octave (2nd order)
    Butterworth12,
    /// Linkwitz-Riley 12 dB/octave (2nd order)
    LR12,
    /// Linkwitz-Riley 24 dB/octave (4th order) - most common
    #[default]
    LR24,
    /// Linkwitz-Riley 48 dB/octave (8th order)
    LR48,
}

impl CrossoverType {
    /// Convert to plugin-compatible string
    pub fn to_plugin_string(&self) -> &'static str {
        match self {
            CrossoverType::Butterworth12 => "Butterworth12",
            CrossoverType::LR12 => "LR12",
            CrossoverType::LR24 => "LR24",
            CrossoverType::LR48 => "LR48",
        }
    }
}

impl FromStr for CrossoverType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "butterworth12" | "bw12" | "butterworth2" => Ok(CrossoverType::Butterworth12),
            "lr12" | "linkwitzriley12" | "linkwitzriley2" => Ok(CrossoverType::LR12),
            "lr24" | "linkwitzriley24" | "linkwitzriley4" => Ok(CrossoverType::LR24),
            "lr48" | "linkwitzriley48" | "linkwitzriley8" => Ok(CrossoverType::LR48),
            _ => Err(format!("Unknown crossover type: {}", s)),
        }
    }
}

// ============================================================================
// Optimizer Configuration
// ============================================================================

/// Algorithm for optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Algorithm {
    /// COBYLA (Constrained Optimization BY Linear Approximation)
    #[default]
    Cobyla,
    /// Differential Evolution
    DifferentialEvolution,
    /// Nelder-Mead simplex method
    NelderMead,
}

impl Algorithm {
    /// Convert to string identifier
    pub fn to_string_id(&self) -> &'static str {
        match self {
            Algorithm::Cobyla => "cobyla",
            Algorithm::DifferentialEvolution => "de",
            Algorithm::NelderMead => "neldermead",
        }
    }
}

/// Configuration for the optimization process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizerConfig {
    /// Optimization algorithm
    pub algorithm: Algorithm,

    /// Number of PEQ filters per channel
    pub num_filters: usize,

    /// Minimum Q factor
    pub min_q: f64,

    /// Maximum Q factor
    pub max_q: f64,

    /// Minimum gain in dB
    pub min_db: f64,

    /// Maximum gain in dB
    pub max_db: f64,

    /// Minimum frequency in Hz
    pub min_freq: f64,

    /// Maximum frequency in Hz
    pub max_freq: f64,

    /// Maximum number of iterations
    pub max_iter: usize,

    /// Sample rate for filter design (Hz)
    pub sample_rate: f64,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            algorithm: Algorithm::default(),
            num_filters: 10,
            min_q: 0.5,
            max_q: 10.0,
            min_db: -12.0,
            max_db: 12.0,
            min_freq: 20.0,
            max_freq: 20000.0,
            max_iter: 10000,
            sample_rate: 48000.0,
        }
    }
}

// ============================================================================
// Channel Configuration
// ============================================================================

/// Speaker configuration type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SpeakerConfigType {
    /// Single measurement (simple speaker)
    #[default]
    Single,
    /// Multiple drivers with crossover
    MultiDriver,
}

/// Configuration for a single channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Channel name
    pub name: String,

    /// Speaker configuration type
    pub config_type: SpeakerConfigType,

    /// Crossover type (for multi-driver)
    pub crossover_type: Option<CrossoverType>,

    /// Driver names (for multi-driver, in order from low to high frequency)
    pub driver_names: Vec<String>,

    /// Initial crossover frequency hints (optional)
    pub crossover_freq_hints: Vec<f64>,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            config_type: SpeakerConfigType::Single,
            crossover_type: None,
            driver_names: Vec::new(),
            crossover_freq_hints: Vec::new(),
        }
    }
}

// ============================================================================
// Measurement Data
// ============================================================================

/// A single frequency response measurement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurement {
    /// Name/identifier
    pub name: String,

    /// Frequency response curve
    pub curve: Curve,

    /// Optional phase data
    pub phase: Option<Vec<f64>>,
}

impl Measurement {
    /// Create a new measurement from a curve
    pub fn new(name: impl Into<String>, curve: Curve) -> Self {
        Self {
            name: name.into(),
            curve,
            phase: None,
        }
    }

    /// Create with phase data
    pub fn with_phase(mut self, phase: Vec<f64>) -> Self {
        self.phase = Some(phase);
        self
    }
}

/// Measurements for a channel (single or multi-driver)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMeasurements {
    /// Channel name
    pub channel_name: String,

    /// Main measurement (single speaker) or combined response (multi-driver post-optimization)
    pub main: Measurement,

    /// Per-driver measurements (for multi-driver only)
    pub drivers: Vec<Measurement>,

    /// Whether this is a multi-driver configuration
    pub is_multi_driver: bool,
}

impl ChannelMeasurements {
    /// Create single-driver channel measurements
    pub fn single(channel_name: impl Into<String>, measurement: Measurement) -> Self {
        Self {
            channel_name: channel_name.into(),
            main: measurement,
            drivers: Vec::new(),
            is_multi_driver: false,
        }
    }

    /// Create multi-driver channel measurements
    pub fn multi_driver(channel_name: impl Into<String>, drivers: Vec<Measurement>) -> Self {
        let combined = if !drivers.is_empty() {
            // Use first driver as placeholder for main until optimization
            Measurement::new("combined", drivers[0].curve.clone())
        } else {
            Measurement::new("combined", Curve::default())
        };

        Self {
            channel_name: channel_name.into(),
            main: combined,
            drivers,
            is_multi_driver: true,
        }
    }
}

// ============================================================================
// Optimization Results
// ============================================================================

/// EQ filter configuration (simplified version of Biquad for serialization)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqFilterResult {
    /// Filter type (peak, lowshelf, highshelf)
    pub filter_type: String,

    /// Center frequency (Hz)
    pub frequency: f64,

    /// Q factor
    pub q: f64,

    /// Gain (dB)
    pub gain_db: f64,
}

impl From<&Biquad> for EqFilterResult {
    fn from(biquad: &Biquad) -> Self {
        Self {
            filter_type: biquad.filter_type.long_name().to_lowercase(),
            frequency: biquad.freq,
            q: biquad.q,
            gain_db: biquad.db_gain,
        }
    }
}

/// Optimization result for a single channel
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChannelOptimizationResult {
    /// Channel name
    pub channel_name: String,

    /// Pre-optimization score (lower is better)
    pub pre_score: f64,

    /// Post-optimization score (lower is better)
    pub post_score: f64,

    /// Optimized EQ filters
    pub eq_filters: Vec<EqFilterResult>,

    /// Biquad filters (for direct application)
    #[serde(skip)]
    pub biquads: Vec<Biquad>,

    /// Crossover frequencies (for multi-driver)
    pub crossover_freqs: Option<Vec<f64>>,

    /// Per-driver gains (for multi-driver, dB)
    pub driver_gains: Option<Vec<f64>>,

    /// Original frequency response
    #[serde(skip)]
    pub original_response: Option<Curve>,

    /// Corrected frequency response
    #[serde(skip)]
    pub corrected_response: Option<Curve>,
}

// ============================================================================
// DSP Chain Output
// ============================================================================

/// DSP plugin configuration (matches AudioEngine PluginConfig format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspPluginConfig {
    /// Plugin type (e.g., "eq", "gain", "crossover")
    pub plugin_type: String,

    /// Plugin parameters as JSON
    pub parameters: serde_json::Value,
}

/// DSP chain for an individual driver
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverDspChain {
    /// Driver name
    pub name: String,

    /// Driver index (0 = lowest frequency)
    pub index: usize,

    /// Plugins for this driver (gain, crossover filters)
    pub plugins: Vec<DspPluginConfig>,
}

/// DSP chain for a single channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelDspChain {
    /// Channel name
    pub channel: String,

    /// Combined plugins (applied to summed output)
    pub plugins: Vec<DspPluginConfig>,

    /// Per-driver DSP chains (for multi-driver only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drivers: Option<Vec<DriverDspChain>>,
}

/// Metadata about the optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationMetadata {
    /// Pre-optimization score (average across channels)
    pub pre_score: f64,

    /// Post-optimization score (average across channels)
    pub post_score: f64,

    /// Algorithm used
    pub algorithm: String,

    /// Number of iterations
    pub iterations: usize,

    /// Timestamp (ISO 8601)
    pub timestamp: String,
}

/// Complete DSP chain output
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DspChainOutput {
    /// Per-channel DSP chains
    pub channels: HashMap<String, ChannelDspChain>,

    /// Optimization metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<OptimizationMetadata>,
}

// ============================================================================
// Progress Tracking
// ============================================================================

/// Status of optimization for a channel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ChannelOptStatus {
    /// Not yet started
    #[default]
    Pending,
    /// Currently optimizing crossover (multi-driver only)
    OptimizingCrossover,
    /// Currently optimizing EQ
    OptimizingEq,
    /// Completed successfully
    Completed,
    /// Failed with error
    Failed,
}

/// Progress update for optimization
#[derive(Debug, Clone, Default)]
pub struct OptimizationProgress {
    /// Current channel being processed
    pub current_channel: Option<String>,

    /// Status of current channel
    pub current_status: ChannelOptStatus,

    /// Overall progress (0.0 - 1.0)
    pub overall_progress: f32,

    /// Status message
    pub message: String,

    /// Per-channel status
    pub channel_statuses: HashMap<String, ChannelOptStatus>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crossover_type_conversion() {
        let ct = CrossoverType::LR24;
        assert_eq!(ct.to_plugin_string(), "LR24");
        assert_eq!(
            "lr24".parse::<CrossoverType>().ok(),
            Some(CrossoverType::LR24)
        );
    }

    #[test]
    fn test_optimizer_config_defaults() {
        let config = OptimizerConfig::default();
        assert_eq!(config.num_filters, 10);
        assert_eq!(config.min_freq, 20.0);
        assert_eq!(config.max_freq, 20000.0);
    }

    #[test]
    fn test_curve_creation() {
        let freq = Array1::linspace(20.0, 20000.0, 100);
        let curve = Curve::flat(freq.clone());
        assert_eq!(curve.len(), 100);
        assert!(!curve.is_empty());
    }
}
