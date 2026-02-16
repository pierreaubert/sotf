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
use math_audio_xem_common::{log_space, RoomSimulation};
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

pub fn extend_to_full_range(output: &SimulationOutput) -> SimulationOutput {
    extend_to_full_range_with_simulation(output, None)
}

pub fn extend_to_full_range_with_room(
    output: &SimulationOutput,
    simulation: &RoomSimulation,
) -> SimulationOutput {
    extend_to_full_range_with_simulation(output, Some(simulation))
}

fn extend_to_full_range_with_simulation(
    output: &SimulationOutput,
    simulation: Option<&RoomSimulation>,
) -> SimulationOutput {
    let last_sim_freq = *output.frequencies.last().expect("empty frequency list");

    // Start slightly above the last simulated point to avoid a duplicate
    let hf_start = last_sim_freq * (1.0 + 1.0 / HF_NUM_POINTS as f64);
    let hf_freqs = log_space(hf_start, HF_MAX_FREQ, HF_NUM_POINTS);

    let mut extended_freqs = output.frequencies.clone();
    extended_freqs.extend_from_slice(&hf_freqs);

    let mut extended_pressures = Vec::with_capacity(output.pressures.len());

    for (src_idx, src_pressures) in output.pressures.iter().enumerate() {
        let source_name = &output.source_names[src_idx];
        let is_sub = source_name.starts_with("sub");
        let mut src_extended = Vec::with_capacity(src_pressures.len());

        for (lp_idx, lp_pressures) in src_pressures.iter().enumerate() {
            let last_pressure = *lp_pressures.last().expect("empty pressure list");
            let last_spl = pressure_to_spl(last_pressure);
            let last_phase = last_pressure.arg();

            let mut extended = lp_pressures.clone();

            match is_sub {
                true => {
                    for &freq in &hf_freqs {
                        let octaves_above = (freq / last_sim_freq).log2();
                        let spl = last_spl - 60.0 * octaves_above;
                        let phase = last_phase - 2.0 * PI * freq * 0.001;
                        extended.push(spl_phase_to_pressure(spl, phase));
                    }
                }
                false => {
                    let seed = (src_idx as u64) * 10_000 + (lp_idx as u64) + 0xCAFE;
                    let noise = generate_smooth_noise(&hf_freqs, seed);
                    let angle_deg = simulation
                        .map(|sim| off_axis_angle_degrees(sim, src_idx, lp_idx))
                        .unwrap_or(0.0);

                    for (i, &freq) in hf_freqs.iter().enumerate() {
                        let shape_db = speaker_response_shape_for_name(source_name, freq);
                        let off_axis_db = hf_off_axis_tilt(freq, angle_deg);
                        let spl = last_spl + shape_db + noise[i] + off_axis_db;
                        let phase = last_phase - 2.0 * PI * freq * 0.0003;
                        extended.push(spl_phase_to_pressure(spl, phase));
                    }
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

fn surround_response_shape(freq: f64) -> f64 {
    let base = speaker_response_shape(freq);
    if freq > 4000.0 {
        base - 2.0 * ((freq / 4000.0).log2()).min(1.5)
    } else {
        base
    }
}

fn center_response_shape(freq: f64) -> f64 {
    let base = speaker_response_shape(freq);
    if freq < 300.0 {
        base - 1.0
    } else {
        base
    }
}

fn speaker_response_shape_for_name(name: &str, freq: f64) -> f64 {
    if name.contains("surround") {
        surround_response_shape(freq)
    } else if name == "center" {
        center_response_shape(freq)
    } else {
        speaker_response_shape(freq)
    }
}

fn off_axis_angle_degrees(simulation: &RoomSimulation, src_idx: usize, lp_idx: usize) -> f64 {
    let src = &simulation.sources[src_idx];
    let lp = simulation.listening_positions[lp_idx];
    let dx = lp.x - src.position.x;
    let dy = lp.y - src.position.y;
    let dz = lp.z - src.position.z;
    let r = (dx * dx + dy * dy + dz * dz).sqrt();
    if r < 1e-9 {
        return 0.0;
    }

    let n = simulation.listening_positions.len() as f64;
    let mut cx = 0.0;
    let mut cy = 0.0;
    let mut cz = 0.0;
    for p in &simulation.listening_positions {
        cx += p.x;
        cy += p.y;
        cz += p.z;
    }
    cx /= n;
    cy /= n;
    cz /= n;

    let fx = cx - src.position.x;
    let fy = cy - src.position.y;
    let fz = cz - src.position.z;
    let fr = (fx * fx + fy * fy + fz * fz).sqrt();
    if fr < 1e-9 {
        return 0.0;
    }

    let dot = dx * fx + dy * fy + dz * fz;
    let cos_angle = (dot / (r * fr)).clamp(-1.0, 1.0);
    cos_angle.acos().to_degrees()
}

fn hf_off_axis_tilt(freq: f64, angle_deg: f64) -> f64 {
    if freq < 2000.0 {
        return 0.0;
    }

    let clamped_angle = angle_deg.max(0.0).min(120.0);
    let base_db = if clamped_angle <= 15.0 {
        0.0
    } else {
        let t = ((clamped_angle - 15.0) / 75.0).min(1.0);
        -6.0 * t
    };

    let hf_ratio = (freq / 2000.0).log2() / (HF_MAX_FREQ / 2000.0).log2();
    let hf_factor = hf_ratio.max(0.0).min(1.0);
    base_db * hf_factor
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
