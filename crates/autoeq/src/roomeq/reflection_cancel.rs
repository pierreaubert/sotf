//! First-reflection cancellation using LP-filtered echo subtraction.
//!
//! Based on Johnston (AES): applies partial cancellation of the first
//! strong reflection below ~500Hz, removing "boxiness" without creating
//! artifacts outside the sweet spot.
//!
//! Algorithm: `y[n] = x[n] - g * LP(x[n - d])`
//! where g = reflection gain, d = reflection delay, LP = Butterworth lowpass.

use math_audio_iir_fir::{Biquad, BiquadFilterType};

/// Configuration for first-reflection cancellation.
#[derive(Debug, Clone)]
pub struct ReflectionCancellationConfig {
    /// Enable/disable reflection cancellation.
    pub enabled: bool,
    /// Maximum frequency for cancellation (Hz). Above this, reflections are left alone.
    /// Per Johnston: ~500 Hz gives ~0.5ft sweet spot radius.
    pub max_freq_hz: f64,
    /// Maximum attenuation of the reflection (dB). Partial cancellation avoids
    /// bizarre artifacts outside the sweet spot.
    pub max_attenuation_db: f64,
    /// Butterworth lowpass filter order (typically 4).
    pub lp_order: usize,
}

impl Default for ReflectionCancellationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_freq_hz: 500.0,
            max_attenuation_db: 6.0,
            lp_order: 4,
        }
    }
}

/// Result of reflection cancellation analysis for one channel.
#[derive(Debug, Clone)]
pub struct ReflectionCancellationResult {
    /// Delay of the first reflection in samples.
    pub delay_samples: usize,
    /// Delay in milliseconds.
    pub delay_ms: f64,
    /// Attenuation to apply (linear, 0..1).
    pub attenuation_linear: f64,
    /// LP filter cutoff frequency.
    pub lp_cutoff_hz: f64,
    /// Biquad cascade for the lowpass filter.
    pub lp_biquads: Vec<Biquad>,
}

/// Compute reflection cancellation parameters from SSIR analysis.
///
/// Finds the first strong reflection, computes its delay and relative gain,
/// and designs the LP filter for the cancellation path.
///
/// Returns `None` if cancellation is disabled, no direct sound is found,
/// or no suitable reflection exists.
pub fn compute_reflection_cancellation(
    ssir_result: &math_rir::SsirResult,
    sample_rate: f64,
    config: &ReflectionCancellationConfig,
) -> Option<ReflectionCancellationResult> {
    if !config.enabled {
        log::info!("Reflection cancellation skipped: feature is disabled");
        return None;
    }

    // Step 1: Find the direct sound segment.
    let Some(direct) = ssir_result.direct_sound() else {
        log::info!("Reflection cancellation skipped: no detectable direct sound segment");
        return None;
    };

    // Step 2: Find the first non-direct-sound segment with significant energy.
    // "Significant" = peak_energy > 0 (any detectable reflection).
    let Some(first_reflection) = ssir_result.reflections().find(|s| s.peak_energy > 0.0) else {
        log::info!("Reflection cancellation skipped: no detectable first reflection");
        return None;
    };

    // Step 3: Compute delay in samples between direct sound TOA and reflection TOA.
    let delay_samples = first_reflection
        .toa_sample
        .saturating_sub(direct.toa_sample);
    if delay_samples == 0 {
        log::info!(
            "Reflection cancellation skipped: first reflection has zero delay relative to direct sound"
        );
        return None;
    }

    let delay_ms = delay_samples as f64 / sample_rate * 1000.0;

    // Step 4: Compute gain = sqrt(reflection_energy / direct_energy).
    // This is the amplitude ratio of the reflection relative to the direct sound.
    if direct.peak_energy <= 0.0 || first_reflection.peak_energy < 0.0 {
        log::warn!(
            "Reflection cancellation skipped: non-positive direct/reflection energy (direct={:.3e}, reflection={:.3e})",
            direct.peak_energy,
            first_reflection.peak_energy
        );
        return None;
    }
    let raw_gain = (first_reflection.peak_energy / direct.peak_energy).sqrt();

    // Step 5: Clamp gain so attenuation doesn't exceed max_attenuation_db.
    // max_attenuation_db limits how much we subtract. Convert dB limit to linear ceiling.
    let max_gain = 1.0 - 10f64.powf(-config.max_attenuation_db / 20.0);
    let gain = raw_gain.min(max_gain);

    // Step 6: Design Butterworth LP cascade at max_freq_hz.
    let q_values = math_audio_iir_fir::peq_butterworth_q(config.lp_order);
    let lp_biquads: Vec<Biquad> = q_values
        .into_iter()
        .map(|q| {
            Biquad::new(
                BiquadFilterType::Lowpass,
                config.max_freq_hz,
                sample_rate,
                q,
                0.0,
            )
        })
        .collect();

    Some(ReflectionCancellationResult {
        delay_samples,
        delay_ms,
        attenuation_linear: gain,
        lp_cutoff_hz: config.max_freq_hz,
        lp_biquads,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use math_rir::{RirSegment, SsirResult};

    fn make_ssir(segments: Vec<RirSegment>) -> SsirResult {
        let mixing_time = segments.last().map(|s| s.end_sample).unwrap_or(0);
        SsirResult {
            segments,
            mixing_time_samples: mixing_time,
            sample_rate: 48000.0,
        }
    }

    fn direct_segment(toa: usize, energy: f64) -> RirSegment {
        RirSegment {
            onset_sample: 0,
            end_sample: toa + 100,
            toa_sample: toa,
            doa: None,
            peak_energy: energy,
            is_direct_sound: true,
        }
    }

    fn reflection_segment(onset: usize, toa: usize, end: usize, energy: f64) -> RirSegment {
        RirSegment {
            onset_sample: onset,
            end_sample: end,
            toa_sample: toa,
            doa: None,
            peak_energy: energy,
            is_direct_sound: false,
        }
    }

    #[test]
    fn test_reflection_cancellation_basic() {
        // Direct at sample 0, reflection at sample 480 (10ms @ 48kHz), -6dB energy
        // energy ratio = 0.25 (since energy is amplitude squared, -6dB amplitude = 0.5, energy = 0.25)
        let ssir = make_ssir(vec![
            direct_segment(0, 1.0),
            reflection_segment(100, 480, 960, 0.25),
        ]);
        let config = ReflectionCancellationConfig::default();
        let result = compute_reflection_cancellation(&ssir, 48000.0, &config).unwrap();

        assert_eq!(result.delay_samples, 480);
        assert!((result.delay_ms - 10.0).abs() < 0.01);
        // gain = sqrt(0.25/1.0) = 0.5, max_gain = 1 - 10^(-6/20) ≈ 0.499
        // so gain should be clamped to ~0.499
        assert!(result.attenuation_linear > 0.0 && result.attenuation_linear < 1.0);
        assert_eq!(result.lp_cutoff_hz, 500.0);
        // Order 4 Butterworth = 2 biquad sections
        assert_eq!(result.lp_biquads.len(), 2);
    }

    #[test]
    fn test_no_reflection_returns_none() {
        // Only direct sound, no reflections.
        let ssir = make_ssir(vec![direct_segment(0, 1.0)]);
        let config = ReflectionCancellationConfig::default();
        assert!(compute_reflection_cancellation(&ssir, 48000.0, &config).is_none());
    }

    #[test]
    fn test_attenuation_clamped() {
        // Reflection at -1dB (very strong): energy ratio ~0.794 (amplitude 0.891, energy 0.794)
        let ssir = make_ssir(vec![
            direct_segment(0, 1.0),
            reflection_segment(100, 480, 960, 0.794),
        ]);
        let config = ReflectionCancellationConfig {
            max_attenuation_db: 6.0,
            ..Default::default()
        };
        let result = compute_reflection_cancellation(&ssir, 48000.0, &config).unwrap();

        // raw_gain = sqrt(0.794) ≈ 0.891
        // max_gain = 1 - 10^(-6/20) ≈ 0.499
        // Should be clamped.
        let max_gain = 1.0 - 10f64.powf(-6.0 / 20.0);
        assert!((result.attenuation_linear - max_gain).abs() < 0.001);
    }

    #[test]
    fn test_disabled_returns_none() {
        let ssir = make_ssir(vec![
            direct_segment(0, 1.0),
            reflection_segment(100, 480, 960, 0.25),
        ]);
        let config = ReflectionCancellationConfig {
            enabled: false,
            ..Default::default()
        };
        assert!(compute_reflection_cancellation(&ssir, 48000.0, &config).is_none());
    }
}
