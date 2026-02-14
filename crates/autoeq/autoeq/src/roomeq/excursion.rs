//! Excursion protection for bookshelf speakers
//!
//! Detects the speaker's F3 rolloff point and generates a highpass filter
//! to prevent dangerous over-boost of bass frequencies during room correction.

use crate::Curve;
use crate::error::{AutoeqError, Result};
use log::info;
use math_audio_iir_fir::Biquad;
use ndarray::Array1;

use super::types::{ExcursionProtectionConfig, HighpassType};

/// Result of F3 detection
#[derive(Debug, Clone)]
pub struct F3DetectionResult {
    /// Detected F3 frequency in Hz
    pub f3_hz: f64,
    /// Reference level used (dB)
    pub reference_level_db: f64,
    /// Method used for detection
    pub method: String,
}

/// Result of excursion protection
#[derive(Debug, Clone)]
pub struct ExcursionProtectionResult {
    /// Highpass filter frequency in Hz
    pub hpf_frequency: f64,
    /// Highpass filters (biquads)
    pub filters: Vec<Biquad>,
    /// Detected or specified F3
    pub f3_hz: f64,
    /// Whether F3 was auto-detected
    pub auto_detected: bool,
}

/// Detect F3 (-3dB point) from a frequency response curve
///
/// # Algorithm
/// 1. Smooth the measurement curve (1/3 octave)
/// 2. Find reference level at ~100-200Hz
/// 3. Search downward for -3dB point
///
/// # Arguments
/// * `curve` - Frequency response measurement
/// * `smoothing_octaves` - Smoothing width in octaves (default: 1/3)
///
/// # Returns
/// * F3 detection result with frequency and method
pub fn detect_f3(curve: &Curve, smoothing_octaves: Option<f64>) -> Result<F3DetectionResult> {
    let _smoothing = smoothing_octaves.unwrap_or(1.0 / 3.0);

    // Apply simple smoothing via moving average in log-frequency space
    let smoothed = smooth_curve_simple(curve, 5);

    // Find reference level in 100-200 Hz range
    let mut ref_sum = 0.0;
    let mut ref_count = 0;
    for i in 0..smoothed.freq.len() {
        if smoothed.freq[i] >= 100.0 && smoothed.freq[i] <= 200.0 {
            ref_sum += smoothed.spl[i];
            ref_count += 1;
        }
    }

    if ref_count == 0 {
        return Err(AutoeqError::InvalidMeasurement {
            message: "No data points in 100-200 Hz range for F3 detection".to_string(),
        });
    }

    let reference_level = ref_sum / ref_count as f64;
    let target_level = reference_level - 3.0;

    // Search downward from 100 Hz for the -3dB point
    let mut f3_hz = 20.0; // Default to 20 Hz if not found

    // Find the highest frequency below 100 Hz where level drops below target
    for i in (0..smoothed.freq.len()).rev() {
        let f = smoothed.freq[i];
        if f >= 100.0 {
            continue; // Start search below 100 Hz
        }

        if smoothed.spl[i] < target_level {
            // Interpolate between this point and the previous one
            if i + 1 < smoothed.freq.len() {
                let f_low = smoothed.freq[i];
                let f_high = smoothed.freq[i + 1];
                let spl_low = smoothed.spl[i];
                let spl_high = smoothed.spl[i + 1];

                // Linear interpolation in log-frequency space
                let t = (target_level - spl_low) / (spl_high - spl_low);
                let log_f = f_low.log10() + t * (f_high.log10() - f_low.log10());
                f3_hz = 10.0_f64.powf(log_f);
            } else {
                f3_hz = smoothed.freq[i];
            }
            break;
        }
    }

    Ok(F3DetectionResult {
        f3_hz,
        reference_level_db: reference_level,
        method: "smoothed_threshold".to_string(),
    })
}

/// Simple smoothing via moving average
fn smooth_curve_simple(curve: &Curve, window_size: usize) -> Curve {
    let half_window = window_size / 2;
    let mut smoothed_spl = Array1::zeros(curve.spl.len());

    for i in 0..curve.spl.len() {
        let start = i.saturating_sub(half_window);
        let end = (i + half_window + 1).min(curve.spl.len());

        let mut sum = 0.0;
        for j in start..end {
            sum += curve.spl[j];
        }
        smoothed_spl[i] = sum / (end - start) as f64;
    }

    Curve {
        freq: curve.freq.clone(),
        spl: smoothed_spl,
        phase: curve.phase.clone(),
    }
}

/// Generate excursion protection highpass filter
///
/// # Arguments
/// * `curve` - Frequency response measurement
/// * `config` - Excursion protection configuration
/// * `sample_rate` - Sample rate for filter design
///
/// # Returns
/// * Excursion protection result with HPF filters
pub fn generate_excursion_protection(
    curve: &Curve,
    config: &ExcursionProtectionConfig,
    sample_rate: f64,
) -> Result<ExcursionProtectionResult> {
    // Determine F3
    let (f3_hz, auto_detected) = if config.auto_detect_f3 {
        let detection = detect_f3(curve, None)?;
        info!(
            "  Auto-detected F3: {:.1} Hz (ref level: {:.1} dB)",
            detection.f3_hz, detection.reference_level_db
        );
        (detection.f3_hz, true)
    } else {
        let f3 = config
            .manual_f3_hz
            .ok_or_else(|| AutoeqError::InvalidConfiguration {
                message: "Manual F3 not specified and auto-detection disabled".to_string(),
            })?;
        info!("  Using manual F3: {:.1} Hz", f3);
        (f3, false)
    };

    // Calculate HPF frequency with safety margin
    // HPF at F3 * 2^(-margin_octaves)
    let hpf_frequency = f3_hz * 2.0_f64.powf(-config.margin_octaves);
    info!(
        "  HPF frequency: {:.1} Hz (F3 - {:.2} octaves)",
        hpf_frequency, config.margin_octaves
    );

    // Generate highpass filters
    let filters = generate_highpass_filters(
        hpf_frequency,
        config.filter_order,
        &config.filter_type,
        sample_rate,
    );

    Ok(ExcursionProtectionResult {
        hpf_frequency,
        filters,
        f3_hz,
        auto_detected,
    })
}

/// Generate highpass biquad filters
///
/// # Arguments
/// * `frequency` - Cutoff frequency in Hz
/// * `order` - Filter order (2 = 12dB/oct, 4 = 24dB/oct)
/// * `filter_type` - Butterworth or Linkwitz-Riley
/// * `sample_rate` - Sample rate in Hz
fn generate_highpass_filters(
    frequency: f64,
    order: usize,
    filter_type: &HighpassType,
    sample_rate: f64,
) -> Vec<Biquad> {
    use math_audio_iir_fir::BiquadFilterType;

    match filter_type {
        HighpassType::Butterworth => {
            // Butterworth: cascaded 2nd-order sections
            let num_sections = order / 2;
            (0..num_sections)
                .map(|_| {
                    Biquad::new(
                        BiquadFilterType::Highpass,
                        frequency,
                        sample_rate,
                        0.707,
                        0.0,
                    )
                })
                .collect()
        }
        HighpassType::LinkwitzRiley => {
            // Linkwitz-Riley: cascaded Butterworth sections
            // LR4 = 2 cascaded BW2, LR8 = 4 cascaded BW2
            let num_sections = order / 2;
            (0..num_sections)
                .map(|_| Biquad::new(BiquadFilterType::Highpass, frequency, sample_rate, 0.5, 0.0))
                .collect()
        }
    }
}

/// Convert excursion protection result to plugin configuration
pub fn excursion_result_to_plugin_params(result: &ExcursionProtectionResult) -> serde_json::Value {
    let filters: Vec<serde_json::Value> = result
        .filters
        .iter()
        .map(|biquad| {
            serde_json::json!({
                "filter_type": "highpass",
                "frequency": biquad.freq,
                "q": biquad.q,
                "gain_db": 0.0
            })
        })
        .collect();

    serde_json::json!({
        "filters": filters
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_curve_with_rolloff() -> Curve {
        // Simulate a bookshelf speaker with rolloff below ~80 Hz
        let freqs: Vec<f64> = (0..100)
            .map(|i| 20.0 * (1000.0 / 20.0_f64).powf(i as f64 / 99.0))
            .collect();

        let spl: Vec<f64> = freqs
            .iter()
            .map(|&f| {
                // Flat at 0 dB above 100 Hz, rolling off below
                // Use 2nd order highpass characteristic
                let fc = 60.0; // Simulated F3 around 60 Hz
                let ratio = f / fc;
                let magnitude = ratio.powi(2) / (1.0 + ratio.powi(2));
                20.0 * magnitude.max(1e-6).log10()
            })
            .collect();

        Curve {
            freq: Array1::from(freqs),
            spl: Array1::from(spl),
            phase: None,
        }
    }

    #[test]
    fn test_f3_detection() {
        let curve = create_test_curve_with_rolloff();
        let result = detect_f3(&curve, None).expect("F3 detection should succeed");

        // F3 should be detected around 60 Hz (our simulated rolloff point)
        assert!(
            result.f3_hz > 40.0 && result.f3_hz < 80.0,
            "F3 should be around 60 Hz, got {:.1} Hz",
            result.f3_hz
        );
    }

    #[test]
    fn test_excursion_protection_auto() {
        let curve = create_test_curve_with_rolloff();
        let config = ExcursionProtectionConfig {
            enabled: true,
            auto_detect_f3: true,
            manual_f3_hz: None,
            filter_order: 4,
            filter_type: HighpassType::LinkwitzRiley,
            margin_octaves: 0.25,
        };

        let result = generate_excursion_protection(&curve, &config, 48000.0)
            .expect("Excursion protection should succeed");

        assert!(result.auto_detected);
        assert!(
            result.hpf_frequency < result.f3_hz,
            "HPF should be below F3"
        );
        assert_eq!(result.filters.len(), 2, "LR4 should have 2 biquad sections");
    }

    #[test]
    fn test_excursion_protection_manual() {
        let curve = create_test_curve_with_rolloff();
        let config = ExcursionProtectionConfig {
            enabled: true,
            auto_detect_f3: false,
            manual_f3_hz: Some(50.0),
            filter_order: 4,
            filter_type: HighpassType::LinkwitzRiley,
            margin_octaves: 0.25,
        };

        let result = generate_excursion_protection(&curve, &config, 48000.0)
            .expect("Excursion protection should succeed");

        assert!(!result.auto_detected);
        assert_eq!(result.f3_hz, 50.0);

        // HPF at 50 * 2^(-0.25) ≈ 42 Hz
        let expected_hpf = 50.0 * 2.0_f64.powf(-0.25);
        assert!((result.hpf_frequency - expected_hpf).abs() < 0.1);
    }

    #[test]
    fn test_butterworth_filters() {
        let filters = generate_highpass_filters(80.0, 4, &HighpassType::Butterworth, 48000.0);
        assert_eq!(
            filters.len(),
            2,
            "4th order Butterworth should have 2 sections"
        );
    }

    #[test]
    fn test_linkwitz_riley_filters() {
        let filters = generate_highpass_filters(80.0, 4, &HighpassType::LinkwitzRiley, 48000.0);
        assert_eq!(filters.len(), 2, "LR4 should have 2 sections");
    }
}
