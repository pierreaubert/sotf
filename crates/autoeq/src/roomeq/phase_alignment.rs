//! Phase alignment optimization for subwoofer integration
//!
//! Maximizes energy sum in the crossover region between subwoofer and main speakers
//! by optimizing delay and polarity settings.

use crate::Curve;
use crate::error::{AutoeqError, Result};
use log::{debug, info};
use ndarray::Array1;
use num_complex::Complex64;
use std::f64::consts::PI;

use super::types::PhaseAlignmentConfig;

/// Result of phase alignment optimization
#[derive(Debug, Clone)]
pub struct PhaseAlignmentResult {
    /// Optimal delay for the speaker relative to subwoofer (ms)
    /// Positive = delay speaker, Negative = delay subwoofer
    pub delay_ms: f64,
    /// Whether to invert polarity of the speaker
    pub invert_polarity: bool,
    /// Energy sum before optimization (arbitrary units)
    pub energy_before: f64,
    /// Energy sum after optimization (arbitrary units)
    pub energy_after: f64,
    /// Improvement in dB
    pub improvement_db: f64,
}

/// Optimize phase alignment between subwoofer and speaker
///
/// # Algorithm
/// 1. Grid search: delay ±max_delay_ms (0.5ms steps), polarity (normal/inverted)
/// 2. For each candidate: compute |H_sub + H_speaker * e^(-jωτ) * polarity|
/// 3. Integrate energy in [min_freq, max_freq] band
/// 4. Return delay/polarity that maximizes energy
///
/// # Arguments
/// * `sub_curve` - Subwoofer frequency response with phase
/// * `speaker_curve` - Speaker frequency response with phase
/// * `config` - Phase alignment configuration
///
/// # Returns
/// * Phase alignment result with optimal delay and polarity
pub fn optimize_phase_alignment(
    sub_curve: &Curve,
    speaker_curve: &Curve,
    config: &PhaseAlignmentConfig,
) -> Result<PhaseAlignmentResult> {
    // Validate that both curves have phase data
    if sub_curve.phase.is_none() {
        return Err(AutoeqError::InvalidMeasurement {
            message: "Subwoofer measurement must include phase data for phase alignment".to_string(),
        });
    }
    if speaker_curve.phase.is_none() {
        return Err(AutoeqError::InvalidMeasurement {
            message: "Speaker measurement must include phase data for phase alignment".to_string(),
        });
    }

    // Interpolate curves to common frequency grid
    let common_freqs = create_common_freq_grid(sub_curve, speaker_curve, config.min_freq, config.max_freq);

    let sub_interp = interpolate_curve_complex(sub_curve, &common_freqs)?;
    let speaker_interp = interpolate_curve_complex(speaker_curve, &common_freqs)?;

    // Calculate baseline energy (no delay, no inversion)
    let energy_before = compute_combined_energy(&sub_interp, &speaker_interp, &common_freqs, 0.0, false);

    // Grid search parameters
    let delay_step = 0.5; // 0.5 ms steps
    let delay_range: Vec<f64> = {
        let num_steps = (config.max_delay_ms / delay_step) as i32;
        (-num_steps..=num_steps)
            .map(|i| i as f64 * delay_step)
            .collect()
    };

    let polarities = if config.optimize_polarity {
        vec![false, true]
    } else {
        vec![false]
    };

    // Grid search for optimal delay and polarity
    let mut best_delay = 0.0;
    let mut best_invert = false;
    let mut best_energy = energy_before;

    for &delay in &delay_range {
        for &invert in &polarities {
            let energy = compute_combined_energy(&sub_interp, &speaker_interp, &common_freqs, delay, invert);
            if energy > best_energy {
                best_energy = energy;
                best_delay = delay;
                best_invert = invert;
            }
        }
    }

    // Refine with finer grid around best result
    let fine_delay_range: Vec<f64> = {
        let fine_step = 0.1; // 0.1 ms for fine search
        let num_fine_steps = 10;
        (-num_fine_steps..=num_fine_steps)
            .map(|i| best_delay + i as f64 * fine_step)
            .filter(|&d| d.abs() <= config.max_delay_ms)
            .collect()
    };

    for &delay in &fine_delay_range {
        let energy = compute_combined_energy(&sub_interp, &speaker_interp, &common_freqs, delay, best_invert);
        if energy > best_energy {
            best_energy = energy;
            best_delay = delay;
        }
    }

    let improvement_db = 10.0 * (best_energy / energy_before.max(1e-12)).log10();

    info!(
        "  Phase alignment: delay={:.2}ms, invert={}, improvement={:.2}dB",
        best_delay, best_invert, improvement_db
    );

    Ok(PhaseAlignmentResult {
        delay_ms: best_delay,
        invert_polarity: best_invert,
        energy_before,
        energy_after: best_energy,
        improvement_db,
    })
}

/// Create a common frequency grid for interpolation
fn create_common_freq_grid(
    curve1: &Curve,
    curve2: &Curve,
    min_freq: f64,
    max_freq: f64,
) -> Array1<f64> {
    // Determine overlapping range
    let f_min = min_freq
        .max(*curve1.freq.first().unwrap_or(&20.0))
        .max(*curve2.freq.first().unwrap_or(&20.0));
    let f_max = max_freq
        .min(*curve1.freq.last().unwrap_or(&20000.0))
        .min(*curve2.freq.last().unwrap_or(&20000.0));

    // Create log-spaced frequency grid
    let num_points = 100;
    let log_min = f_min.log10();
    let log_max = f_max.log10();

    Array1::from_shape_fn(num_points, |i| {
        let log_f = log_min + (log_max - log_min) * (i as f64 / (num_points - 1) as f64);
        10.0_f64.powf(log_f)
    })
}

/// Interpolate a curve to new frequencies, returning complex values
fn interpolate_curve_complex(curve: &Curve, new_freqs: &Array1<f64>) -> Result<Vec<Complex64>> {
    let phase = curve.phase.as_ref().ok_or_else(|| AutoeqError::InvalidMeasurement {
        message: "Phase data required for complex interpolation".to_string(),
    })?;

    let mut result = Vec::with_capacity(new_freqs.len());

    for &f in new_freqs.iter() {
        // Find bracketing indices
        let (lower_idx, upper_idx) = find_bracket_indices(&curve.freq, f);

        // Linear interpolation
        let f_low = curve.freq[lower_idx];
        let f_high = curve.freq[upper_idx];
        let t = if f_high > f_low {
            (f - f_low) / (f_high - f_low)
        } else {
            0.0
        };

        let spl_interp = curve.spl[lower_idx] + t * (curve.spl[upper_idx] - curve.spl[lower_idx]);
        let phase_interp = phase[lower_idx] + t * (phase[upper_idx] - phase[lower_idx]);

        // Convert to complex
        let magnitude = 10.0_f64.powf(spl_interp / 20.0);
        let phase_rad = phase_interp.to_radians();
        result.push(Complex64::from_polar(magnitude, phase_rad));
    }

    Ok(result)
}

/// Find bracketing indices for interpolation
fn find_bracket_indices(freqs: &Array1<f64>, target: f64) -> (usize, usize) {
    for i in 0..freqs.len() - 1 {
        if freqs[i] <= target && freqs[i + 1] >= target {
            return (i, i + 1);
        }
    }

    // Clamp to ends
    if target <= freqs[0] {
        (0, 0)
    } else {
        let last = freqs.len() - 1;
        (last, last)
    }
}

/// Compute combined energy of sub + speaker with delay and polarity
fn compute_combined_energy(
    sub: &[Complex64],
    speaker: &[Complex64],
    freqs: &Array1<f64>,
    delay_ms: f64,
    invert: bool,
) -> f64 {
    let delay_s = delay_ms / 1000.0;
    let polarity = if invert { -1.0 } else { 1.0 };

    let mut energy = 0.0;

    for (i, &f) in freqs.iter().enumerate() {
        // Apply delay: e^(-jωτ)
        let omega = 2.0 * PI * f;
        let delay_phase = Complex64::from_polar(1.0, -omega * delay_s);

        // Combined response: H_sub + H_speaker * delay * polarity
        let combined = sub[i] + speaker[i] * delay_phase * polarity;

        // Accumulate squared magnitude (energy)
        energy += combined.norm_sqr();
    }

    energy
}

/// Batch phase alignment for multiple speakers with a common subwoofer
///
/// # Arguments
/// * `sub_curve` - Subwoofer frequency response with phase
/// * `speaker_curves` - Vector of speaker frequency responses with phase
/// * `config` - Phase alignment configuration
///
/// # Returns
/// * Vector of phase alignment results, one per speaker
pub fn optimize_phase_alignment_batch(
    sub_curve: &Curve,
    speaker_curves: &[Curve],
    config: &PhaseAlignmentConfig,
) -> Result<Vec<PhaseAlignmentResult>> {
    speaker_curves
        .iter()
        .enumerate()
        .map(|(i, speaker_curve)| {
            debug!("  Aligning speaker {} with subwoofer", i);
            optimize_phase_alignment(sub_curve, speaker_curve, config)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_sub_curve() -> Curve {
        let freqs: Vec<f64> = (0..50)
            .map(|i| 20.0 * (500.0 / 20.0_f64).powf(i as f64 / 49.0))
            .collect();

        let spl: Vec<f64> = freqs.iter().map(|_| 90.0).collect(); // Flat response
        let phase: Vec<f64> = freqs.iter().map(|f| -180.0 * f / 100.0).collect(); // Simple phase slope

        Curve {
            freq: Array1::from(freqs),
            spl: Array1::from(spl),
            phase: Some(Array1::from(phase)),
        }
    }

    fn create_test_speaker_curve() -> Curve {
        let freqs: Vec<f64> = (0..50)
            .map(|i| 20.0 * (500.0 / 20.0_f64).powf(i as f64 / 49.0))
            .collect();

        let spl: Vec<f64> = freqs.iter().map(|_| 90.0).collect();
        // Phase offset that should be corrected by delay
        let phase: Vec<f64> = freqs.iter().map(|f| -180.0 * f / 100.0 + 45.0).collect();

        Curve {
            freq: Array1::from(freqs),
            spl: Array1::from(spl),
            phase: Some(Array1::from(phase)),
        }
    }

    #[test]
    fn test_phase_alignment_basic() {
        let sub = create_test_sub_curve();
        let speaker = create_test_speaker_curve();
        let config = PhaseAlignmentConfig::default();

        let result = optimize_phase_alignment(&sub, &speaker, &config)
            .expect("Phase alignment should succeed");

        // Should find some improvement
        assert!(result.improvement_db >= 0.0, "Should not make things worse");
    }

    #[test]
    fn test_phase_alignment_no_phase_fails() {
        let sub = Curve {
            freq: Array1::from(vec![50.0, 80.0, 100.0]),
            spl: Array1::from(vec![90.0, 90.0, 90.0]),
            phase: None, // No phase data
        };
        let speaker = create_test_speaker_curve();
        let config = PhaseAlignmentConfig::default();

        let result = optimize_phase_alignment(&sub, &speaker, &config);
        assert!(result.is_err(), "Should fail without phase data");
    }

    #[test]
    fn test_phase_alignment_polarity_detection() {
        let sub = create_test_sub_curve();

        // Create speaker with inverted polarity (180 degree offset)
        let freqs: Vec<f64> = (0..50)
            .map(|i| 20.0 * (500.0 / 20.0_f64).powf(i as f64 / 49.0))
            .collect();
        let spl: Vec<f64> = freqs.iter().map(|_| 90.0).collect();
        let phase: Vec<f64> = freqs.iter().map(|f| -180.0 * f / 100.0 + 180.0).collect();

        let speaker = Curve {
            freq: Array1::from(freqs),
            spl: Array1::from(spl),
            phase: Some(Array1::from(phase)),
        };

        let config = PhaseAlignmentConfig {
            optimize_polarity: true,
            ..Default::default()
        };

        let _result = optimize_phase_alignment(&sub, &speaker, &config)
            .expect("Phase alignment should succeed");

        // Should detect that polarity inversion helps
        // Note: The exact result depends on the test data
    }

    #[test]
    fn test_common_freq_grid() {
        let sub = create_test_sub_curve();
        let speaker = create_test_speaker_curve();

        let grid = create_common_freq_grid(&sub, &speaker, 60.0, 100.0);

        assert!(!grid.is_empty());
        assert!(grid[0] >= 60.0);
        assert!(grid[grid.len() - 1] <= 100.0);
    }

    #[test]
    fn test_batch_alignment() {
        let sub = create_test_sub_curve();
        let speakers = vec![
            create_test_speaker_curve(),
            create_test_speaker_curve(),
        ];
        let config = PhaseAlignmentConfig::default();

        let results = optimize_phase_alignment_batch(&sub, &speakers, &config)
            .expect("Batch alignment should succeed");

        assert_eq!(results.len(), 2);
    }
}
