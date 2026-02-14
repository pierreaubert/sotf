//! Enhanced loss functions with configurable frequency band weights.
//!
//! Based on research:
//! - "Perceptually-Motivated Audio Equalization" (Kulkarni et al.)
//! - "Frequency-Dependent Weighting in Audio Equalization" (Zölzer et al.)

use ndarray::Array1;

/// Frequency band configuration for weighted loss
#[derive(Debug, Clone)]
pub struct FrequencyBandWeights {
    /// Bass band minimum frequency (Hz)
    pub bass_min: f64,
    /// Bass band maximum frequency (Hz)
    pub bass_max: f64,
    /// Midrange band minimum frequency (Hz)
    pub mid_min: f64,
    /// Midrange band maximum frequency (Hz)
    pub mid_max: f64,
    /// Treble band minimum frequency (Hz)
    pub treble_min: f64,
    /// Treble band maximum frequency (Hz)
    pub treble_max: f64,
    /// Weight for bass band (default: 2.0 - bass is more critical for room correction)
    pub bass_weight: f64,
    /// Weight for midrange band (default: 1.0)
    pub mid_weight: f64,
    /// Weight for treble band (default: 0.8 - less critical for room issues)
    pub treble_weight: f64,
}

impl Default for FrequencyBandWeights {
    fn default() -> Self {
        Self {
            bass_min: 20.0,
            bass_max: 200.0,
            mid_min: 200.0,
            mid_max: 4000.0,
            treble_min: 4000.0,
            treble_max: 20000.0,
            bass_weight: 2.0,
            mid_weight: 1.0,
            treble_weight: 0.8,
        }
    }
}

/// Compute ERB (Equivalent Rectangular Bandwidth) for a frequency
/// ERB formula: 24.7 * (1 + 4.37 * f / 1000)
pub fn erb(frequency: f64) -> f64 {
    24.7 * (1.0 + 4.37 * frequency / 1000.0)
}

/// Compute ERB-weighted error
///
/// The ERB scale provides better perceptual relevance than linear frequency.
/// Lower frequencies have smaller ERBs, meaning we get more resolution where
/// the human auditory system is more sensitive.
pub fn erb_weighted_loss(freqs: &Array1<f64>, error: &Array1<f64>) -> f64 {
    assert_eq!(freqs.len(), error.len());

    let erbs: Array1<f64> = freqs.mapv(erb);

    // Weight inversely proportional to ERB (more weight at low frequencies)
    let weights: Array1<f64> = erbs.mapv(|e| 1.0 / e);

    // Normalize weights
    let total_weight: f64 = weights.iter().sum();
    if total_weight == 0.0 {
        return 0.0;
    }

    // Compute weighted mean squared error
    let weighted_sum: f64 = error
        .iter()
        .zip(weights.iter())
        .map(|(e, w)| e * e * w)
        .sum();

    (weighted_sum / total_weight).sqrt()
}

/// Compute frequency band weighted error
pub fn band_weighted_loss(
    freqs: &Array1<f64>,
    error: &Array1<f64>,
    bands: &FrequencyBandWeights,
) -> f64 {
    assert_eq!(freqs.len(), error.len());

    let mut bass_ss = 0.0;
    let mut bass_n = 0usize;
    let mut mid_ss = 0.0;
    let mut mid_n = 0usize;
    let mut treble_ss = 0.0;
    let mut treble_n = 0usize;

    for (&f, &e) in freqs.iter().zip(error.iter()) {
        if f >= bands.bass_min && f <= bands.bass_max {
            bass_ss += e * e;
            bass_n += 1;
        } else if f >= bands.mid_min && f <= bands.mid_max {
            mid_ss += e * e;
            mid_n += 1;
        } else if f >= bands.treble_min && f <= bands.treble_max {
            treble_ss += e * e;
            treble_n += 1;
        }
    }

    let bass_rms = if bass_n > 0 {
        (bass_ss / bass_n as f64).sqrt()
    } else {
        0.0
    };
    let mid_rms = if mid_n > 0 {
        (mid_ss / mid_n as f64).sqrt()
    } else {
        0.0
    };
    let treble_rms = if treble_n > 0 {
        (treble_ss / treble_n as f64).sqrt()
    } else {
        0.0
    };

    bands.bass_weight * bass_rms + bands.mid_weight * mid_rms + bands.treble_weight * treble_rms
}

/// Combine ERB-weighted and band-weighted approaches
///
/// This provides both perceptual relevance (ERB) and user control (bands)
pub fn combined_weighted_loss(
    freqs: &Array1<f64>,
    error: &Array1<f64>,
    bands: &FrequencyBandWeights,
    erb_weight: f64,
    band_weight: f64,
) -> f64 {
    let erb_loss = erb_weighted_loss(freqs, error);
    let band_loss = band_weighted_loss(freqs, error, bands);

    erb_weight * erb_loss + band_weight * band_loss
}
