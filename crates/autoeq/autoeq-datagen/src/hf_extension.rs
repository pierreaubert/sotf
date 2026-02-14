//! High-frequency extension for simulation output
//!
//! Below 500 Hz room interaction dominates and is computed by BEM/FEM.
//! Above 500 Hz the response is mostly the speaker's direct sound, so we
//! synthesise a realistic-looking curve: a gentle speaker response shape
//! plus slowly-varying random deviations (seeded per source/LP pair for
//! reproducibility).
//!
//! Subwoofer sources (names starting with "sub") are rolled off steeply
//! since they already have an 80 Hz lowpass crossover.

use crate::SimulationOutput;
use math_audio_xem_common::log_space;
use num_complex::Complex64;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::f64::consts::PI;

/// Number of log-spaced points in the 500 Hz – 20 kHz extension
const HF_NUM_POINTS: usize = 100;

/// Upper frequency limit of the extension
const HF_MAX_FREQ: f64 = 20_000.0;

/// Number of random control points for cosine-interpolated noise
const NOISE_CONTROL_POINTS: usize = 12;

/// Reference pressure for SPL conversion (20 µPa)
const P_REF: f64 = 20e-6;

/// Extend a `SimulationOutput` (20–500 Hz) to cover 20 Hz – 20 kHz.
///
/// The original simulation data is kept verbatim; 100 additional log-spaced
/// points are appended from just above 500 Hz up to 20 kHz.
pub fn extend_to_full_range(output: &SimulationOutput) -> SimulationOutput {
    let last_sim_freq = *output.frequencies.last().expect("empty frequency list");

    // Start slightly above the last simulated point to avoid a duplicate
    let hf_start = last_sim_freq * (1.0 + 1.0 / HF_NUM_POINTS as f64);
    let hf_freqs = log_space(hf_start, HF_MAX_FREQ, HF_NUM_POINTS);

    let mut extended_freqs = output.frequencies.clone();
    extended_freqs.extend_from_slice(&hf_freqs);

    let mut extended_pressures = Vec::with_capacity(output.pressures.len());

    for (src_idx, src_pressures) in output.pressures.iter().enumerate() {
        let is_sub = output.source_names[src_idx].starts_with("sub");
        let mut src_extended = Vec::with_capacity(src_pressures.len());

        for (lp_idx, lp_pressures) in src_pressures.iter().enumerate() {
            let last_pressure = *lp_pressures.last().expect("empty pressure list");
            let last_spl = pressure_to_spl(last_pressure);
            let last_phase = last_pressure.arg();

            let mut extended = lp_pressures.clone();

            if is_sub {
                // Steep rolloff: -60 dB/octave above the simulation range
                for &freq in &hf_freqs {
                    let octaves_above = (freq / last_sim_freq).log2();
                    let spl = last_spl - 60.0 * octaves_above;
                    let phase = last_phase - 2.0 * PI * freq * 0.001;
                    extended.push(spl_phase_to_pressure(spl, phase));
                }
            } else {
                // Seeded RNG for reproducible per-source-per-LP noise
                let seed = (src_idx as u64) * 10_000 + (lp_idx as u64) + 0xCAFE;
                let noise = generate_smooth_noise(&hf_freqs, seed);

                for (i, &freq) in hf_freqs.iter().enumerate() {
                    let shape_db = speaker_response_shape(freq);
                    let spl = last_spl + shape_db + noise[i];
                    // Gentle linear phase accumulation (group delay ~ 0.3 ms)
                    let phase = last_phase - 2.0 * PI * freq * 0.0003;
                    extended.push(spl_phase_to_pressure(spl, phase));
                }
            }

            src_extended.push(extended);
        }

        extended_pressures.push(src_extended);
    }

    SimulationOutput {
        frequencies: extended_freqs,
        pressures: extended_pressures,
        source_names: output.source_names.clone(),
    }
}

/// Typical speaker direct-sound response shape (dB relative to midband).
///
/// - Flat through the midrange (500 Hz – 3 kHz)
/// - Small presence rise ~+1.5 dB around 2–4 kHz
/// - Gentle treble rolloff starting at ~8 kHz, about -6 dB at 20 kHz
fn speaker_response_shape(freq: f64) -> f64 {
    // Presence bump: Gaussian centred at 3 kHz, ~1 octave wide
    let presence = 1.5 * (-0.5 * ((freq / 3000.0).ln() / 0.5_f64.ln()).powi(2)).exp();

    // Treble rolloff: starts at ~8 kHz, -6 dB at 20 kHz
    let rolloff = if freq > 8000.0 {
        -6.0 * ((freq / 8000.0).log2() / (20000.0_f64 / 8000.0).log2())
    } else {
        0.0
    };

    presence + rolloff
}

/// Generate slowly-varying noise via cosine interpolation between random
/// control points.  Values are in the range roughly [-5, +3] dB.
fn generate_smooth_noise(freqs: &[f64], seed: u64) -> Vec<f64> {
    let mut rng = SmallRng::seed_from_u64(seed);

    let n = freqs.len();
    if n == 0 {
        return Vec::new();
    }

    let log_min = freqs[0].ln();
    let log_max = freqs[n - 1].ln();
    let log_span = log_max - log_min;

    // Generate control-point dB values: biased slightly negative [-5, +3]
    let control_vals: Vec<f64> = (0..NOISE_CONTROL_POINTS)
        .map(|_| rng.random_range(-5.0..3.0))
        .collect();

    // Cosine-interpolate at each frequency
    freqs
        .iter()
        .map(|&f| {
            let t = (f.ln() - log_min) / log_span; // 0..1
            let scaled = t * (NOISE_CONTROL_POINTS - 1) as f64;
            let idx = (scaled as usize).min(NOISE_CONTROL_POINTS - 2);
            let frac = scaled - idx as f64;

            // Cosine interpolation for smooth transitions
            let cos_frac = (1.0 - (frac * PI).cos()) * 0.5;
            control_vals[idx] * (1.0 - cos_frac) + control_vals[idx + 1] * cos_frac
        })
        .collect()
}

fn pressure_to_spl(p: Complex64) -> f64 {
    let mag = p.norm();
    if mag < 1e-30 {
        -120.0
    } else {
        20.0 * (mag / P_REF).log10()
    }
}

fn spl_phase_to_pressure(spl: f64, phase: f64) -> Complex64 {
    let mag = P_REF * 10.0_f64.powf(spl / 20.0);
    Complex64::new(mag * phase.cos(), mag * phase.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speaker_response_shape_flat_at_midband() {
        // Should be close to 0 dB at 1 kHz (midrange, away from presence peak)
        let db = speaker_response_shape(1000.0);
        assert!(
            db.abs() < 0.5,
            "Expected near-flat at 1 kHz, got {db:.2} dB"
        );
    }

    #[test]
    fn test_speaker_response_shape_presence_peak() {
        let db = speaker_response_shape(3000.0);
        assert!(db > 0.5, "Expected presence rise at 3 kHz, got {db:.2} dB");
        assert!(db < 3.0, "Presence peak too large at 3 kHz: {db:.2} dB");
    }

    #[test]
    fn test_speaker_response_shape_rolloff_at_20khz() {
        let db = speaker_response_shape(20000.0);
        assert!(db < -4.0, "Expected rolloff at 20 kHz, got {db:.2} dB");
        assert!(db > -8.0, "Rolloff too steep at 20 kHz: {db:.2} dB");
    }

    #[test]
    fn test_smooth_noise_range() {
        let freqs = log_space(500.0, 20000.0, 100);
        let noise = generate_smooth_noise(&freqs, 42);
        assert_eq!(noise.len(), 100);
        for &v in &noise {
            assert!(
                v >= -6.0 && v <= 4.0,
                "Noise value {v:.2} out of expected range"
            );
        }
    }

    #[test]
    fn test_smooth_noise_deterministic() {
        let freqs = log_space(500.0, 20000.0, 50);
        let a = generate_smooth_noise(&freqs, 123);
        let b = generate_smooth_noise(&freqs, 123);
        assert_eq!(a, b, "Same seed should produce identical noise");
    }

    #[test]
    fn test_smooth_noise_varies_slowly() {
        let freqs = log_space(500.0, 20000.0, 200);
        let noise = generate_smooth_noise(&freqs, 99);
        // Adjacent samples should not jump more than ~2 dB
        for i in 1..noise.len() {
            let jump = (noise[i] - noise[i - 1]).abs();
            assert!(
                jump < 2.0,
                "Noise jumps {jump:.3} dB between samples {i}-{}: too fast",
                i - 1
            );
        }
    }

    #[test]
    fn test_spl_pressure_roundtrip() {
        let original_spl = 80.0;
        let phase = 0.5;
        let p = spl_phase_to_pressure(original_spl, phase);
        let recovered = pressure_to_spl(p);
        assert!(
            (recovered - original_spl).abs() < 0.01,
            "SPL roundtrip failed: {original_spl} -> {recovered}"
        );
    }

    #[test]
    fn test_extend_dimensions() {
        // Minimal SimulationOutput: 2 sources, 1 LP, 10 frequencies
        let n_sim = 10;
        let freqs: Vec<f64> = log_space(20.0, 500.0, n_sim);
        let pressures = vec![
            // source 0 ("left"): 1 LP
            vec![vec![Complex64::new(0.01, 0.002); n_sim]],
            // source 1 ("sub1"): 1 LP
            vec![vec![Complex64::new(0.005, 0.001); n_sim]],
        ];
        let output = SimulationOutput {
            frequencies: freqs,
            pressures,
            source_names: vec!["left".to_string(), "sub1".to_string()],
        };

        let extended = extend_to_full_range(&output);

        assert_eq!(
            extended.frequencies.len(),
            n_sim + HF_NUM_POINTS,
            "Expected {} frequencies",
            n_sim + HF_NUM_POINTS
        );
        assert_eq!(extended.pressures.len(), 2);
        assert_eq!(extended.pressures[0][0].len(), n_sim + HF_NUM_POINTS);
        assert_eq!(extended.pressures[1][0].len(), n_sim + HF_NUM_POINTS);

        // First N frequencies unchanged
        assert_eq!(extended.frequencies[0], output.frequencies[0]);
        // Last frequency near 20 kHz
        assert!(extended.frequencies.last().unwrap() > &19000.0);
    }

    #[test]
    fn test_extend_subwoofer_rolls_off() {
        let n_sim = 10;
        let freqs: Vec<f64> = log_space(20.0, 500.0, n_sim);
        let pressures = vec![vec![vec![Complex64::new(0.01, 0.0); n_sim]]];
        let output = SimulationOutput {
            frequencies: freqs,
            pressures,
            source_names: vec!["sub1".to_string()],
        };

        let extended = extend_to_full_range(&output);

        // SPL at 20 kHz should be far below SPL at 500 Hz
        let spl_500 = pressure_to_spl(extended.pressures[0][0][n_sim - 1]);
        let spl_20k = pressure_to_spl(*extended.pressures[0][0].last().unwrap());
        assert!(
            spl_20k < spl_500 - 100.0,
            "Sub should be >100 dB below at 20 kHz: {spl_500:.1} vs {spl_20k:.1}"
        );
    }

    #[test]
    fn test_extend_main_speaker_continuous() {
        let n_sim = 50;
        let freqs: Vec<f64> = log_space(20.0, 500.0, n_sim);
        let pressures = vec![vec![vec![Complex64::new(0.02, 0.005); n_sim]]];
        let output = SimulationOutput {
            frequencies: freqs,
            pressures,
            source_names: vec!["left".to_string()],
        };

        let extended = extend_to_full_range(&output);

        // SPL should be continuous at the junction (within ~5 dB due to noise)
        let spl_last_sim = pressure_to_spl(extended.pressures[0][0][n_sim - 1]);
        let spl_first_hf = pressure_to_spl(extended.pressures[0][0][n_sim]);
        let jump = (spl_first_hf - spl_last_sim).abs();
        assert!(
            jump < 6.0,
            "SPL jump at 500 Hz junction: {jump:.1} dB (last_sim={spl_last_sim:.1}, first_hf={spl_first_hf:.1})"
        );
    }
}
