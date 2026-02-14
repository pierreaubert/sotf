//! Bass boost target curve implementation.
//!
//! Based on Harman International research:
//! - "Target Response for In-Room Loudspeaker Reproduction" (Olive et al.)
//! - "A Model for Predicting In-Room Listening Frequency Response" (Toole)

use ndarray::Array1;

/// Bass boost curve type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BassBoostCurve {
    /// Linear ramp from start to peak to end
    Linear,
    /// Harman curve based on research (smooth bell shape)
    Harman,
    /// Custom curve defined by control points
    Custom,
}

/// Bass boost configuration
#[derive(Debug, Clone)]
pub struct BassBoostConfig {
    /// Enable bass boost
    pub enabled: bool,
    /// Frequency where boost starts (Hz)
    pub start_freq: f64,
    /// Frequency of maximum boost (Hz)
    pub peak_freq: f64,
    /// Frequency where boost ends (Hz)
    pub end_freq: f64,
    /// Maximum boost in dB
    pub max_boost_db: f64,
    /// Curve shape
    pub curve_type: BassBoostCurve,
}

impl Default for BassBoostConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            start_freq: 20.0,
            peak_freq: 60.0,
            end_freq: 200.0,
            max_boost_db: 4.0,
            curve_type: BassBoostCurve::Harman,
        }
    }
}

/// Compute bass boost curve for given frequencies
pub fn compute_bass_boost_curve(freqs: &Array1<f64>, config: &BassBoostConfig) -> Array1<f64> {
    if !config.enabled {
        return Array1::zeros(freqs.len());
    }

    freqs.mapv(|f| match config.curve_type {
        BassBoostCurve::Linear => compute_linear_bass_boost(f, config),
        BassBoostCurve::Harman => compute_harman_bass_boost(f, config),
        BassBoostCurve::Custom => compute_linear_bass_boost(f, config), // Same as linear for now
    })
}

/// Linear bass boost ramp
fn compute_linear_bass_boost(f: f64, config: &BassBoostConfig) -> f64 {
    if f < config.start_freq {
        // Below start: no boost
        0.0
    } else if f < config.peak_freq {
        // Rising to peak
        let t = (f - config.start_freq) / (config.peak_freq - config.start_freq);
        config.max_boost_db * t
    } else if f < config.end_freq {
        // Falling from peak
        let t = (f - config.peak_freq) / (config.end_freq - config.peak_freq);
        config.max_boost_db * (1.0 - t)
    } else {
        // Above end: no boost
        0.0
    }
}

/// Harman bass boost curve (smoother, based on research)
///
/// The Harman curve provides a more natural-sounding bass boost
/// that compensates for room gain rolloff at low frequencies.
fn compute_harman_bass_boost(f: f64, config: &BassBoostConfig) -> f64 {
    if f < config.start_freq || f > config.end_freq {
        return 0.0;
    }

    // Use a smooth bell curve shape
    let peak = config.peak_freq;
    let width = (config.end_freq - config.start_freq) / 2.0;

    // Gaussian-like curve centered at peak_freq
    let z = (f - peak) / width;
    config.max_boost_db * (-0.5 * z * z).exp()
}

/// Create a combined target curve with bass boost
pub fn create_bass_boosted_target(
    base_target: &Array1<f64>,
    freqs: &Array1<f64>,
    config: &BassBoostConfig,
) -> Array1<f64> {
    let bass_boost = compute_bass_boost_curve(freqs, config);
    base_target + &bass_boost
}
