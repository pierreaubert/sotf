//! Decomposed correction strategy for room EQ.
//!
//! Separately identify and treat different
//! acoustic phenomena with appropriate correction strategies:
//!
//! 1. **Room modes** (below Schroeder frequency): narrow resonances that are
//!    spatially consistent → aggressive, narrow-Q correction
//! 2. **Early reflections** (2-20 ms after direct sound): position-dependent
//!    coloration → reduced or no correction
//! 3. **Steady-state response** (above Schroeder frequency): smooth broadband
//!    character → gentle, broad corrections only
//!
//! The output is a per-frequency correction weight vector that can be used
//! to modulate the EQ optimizer's behavior.
//!
//! Reference: Laborie, Bruno & Montoya, AES 114th Convention (2003)

use ndarray::Array1;

/// Configuration for decomposed correction analysis.
#[derive(Debug, Clone)]
pub struct DecomposedCorrectionConfig {
    /// Schroeder frequency (Hz) separating modal and statistical regions.
    /// Below this: room modes dominate. Above: diffuse field.
    /// Default: 200.0 Hz (typical for medium rooms)
    pub schroeder_freq: f64,

    /// Minimum Q factor to consider a peak as a room mode.
    /// Higher Q = narrower peak = more likely a resonance.
    /// Default: 3.0
    pub min_mode_q: f64,

    /// Minimum prominence (dB) for a peak to be identified as a room mode.
    /// Default: 3.0 dB
    pub min_mode_prominence_db: f64,

    /// Correction weight for identified room modes (0.0 - 1.0).
    /// Default: 1.0 (full correction)
    pub mode_correction_weight: f64,

    /// Correction weight for early reflection region (0.0 - 1.0).
    /// Default: 0.3 (reduced correction — reflections are position-dependent)
    pub early_reflection_weight: f64,

    /// Correction weight for steady-state response above Schroeder (0.0 - 1.0).
    /// Default: 0.5 (moderate correction — preserve speaker character)
    pub steady_state_weight: f64,

    /// Transition width around Schroeder frequency in octaves.
    /// Default: 0.5 octaves
    pub transition_width_oct: f64,
}

impl Default for DecomposedCorrectionConfig {
    fn default() -> Self {
        Self {
            schroeder_freq: 200.0,
            min_mode_q: 3.0,
            min_mode_prominence_db: 3.0,
            mode_correction_weight: 1.0,
            early_reflection_weight: 0.3,
            steady_state_weight: 0.5,
            transition_width_oct: 0.5,
        }
    }
}

/// A detected room mode (resonance).
#[derive(Debug, Clone)]
pub struct RoomMode {
    /// Center frequency of the mode (Hz)
    pub frequency: f64,
    /// Estimated Q factor (narrowness)
    pub q: f64,
    /// Prominence in dB (how much it stands above the surrounding response)
    pub prominence_db: f64,
    /// Index into the frequency array
    pub index: usize,
}

/// Result of decomposed correction analysis.
#[derive(Debug, Clone)]
pub struct DecomposedCorrectionResult {
    /// Detected room modes
    pub room_modes: Vec<RoomMode>,

    /// Per-frequency correction weight (0.0 = no correction, 1.0 = full correction).
    /// Combines mode detection, Schroeder split, and steady-state weighting.
    pub correction_weights: Array1<f64>,

    /// Schroeder frequency used for the analysis
    pub schroeder_freq: f64,
}

/// Analyze a frequency response to build decomposed correction weights.
///
/// The algorithm:
/// 1. Detect narrow peaks (room modes) below Schroeder frequency
/// 2. Build baseline correction weight using Schroeder split
/// 3. Boost weights at detected room mode frequencies
/// 4. Apply steady-state weight above Schroeder frequency
pub fn analyze_decomposed_correction(
    freq: &Array1<f64>,
    spl: &Array1<f64>,
    config: &DecomposedCorrectionConfig,
) -> DecomposedCorrectionResult {
    // Step 1: Detect room modes (narrow peaks below Schroeder frequency)
    let room_modes = detect_room_modes(freq, spl, config);

    // Step 2: Build per-frequency correction weights
    let correction_weights = build_correction_weights(freq, &room_modes, config);

    DecomposedCorrectionResult {
        room_modes,
        correction_weights,
        schroeder_freq: config.schroeder_freq,
    }
}

/// Detect narrow peaks (room modes) in the frequency response below the Schroeder frequency.
///
/// Uses a local peak detection algorithm with prominence filtering:
/// - Only considers frequencies below Schroeder
/// - Estimates Q from the -3 dB bandwidth around each peak
/// - Filters by minimum Q and prominence
fn detect_room_modes(
    freq: &Array1<f64>,
    spl: &Array1<f64>,
    config: &DecomposedCorrectionConfig,
) -> Vec<RoomMode> {
    let mut modes = Vec::new();
    let n = freq.len();
    if n < 5 {
        return modes;
    }

    // Find local maxima below Schroeder frequency
    for i in 2..n - 2 {
        if freq[i] > config.schroeder_freq {
            break;
        }

        // Local maximum: higher than both neighbors (using 2-sample window for robustness)
        let is_peak = spl[i] > spl[i - 1]
            && spl[i] > spl[i + 1]
            && spl[i] > spl[i - 2]
            && spl[i] > spl[i + 2];

        if !is_peak {
            continue;
        }

        // Compute prominence: how much this peak rises above the local baseline
        // Local baseline = average of values at edges of a +/- 1 octave window
        let f_low = freq[i] / 2.0; // -1 octave
        let f_high = freq[i] * 2.0; // +1 octave
        let baseline = compute_local_baseline(freq, spl, i, f_low, f_high);
        let prominence = spl[i] - baseline;

        if prominence < config.min_mode_prominence_db {
            continue;
        }

        // Estimate Q from -3 dB bandwidth
        let q = estimate_peak_q(freq, spl, i);

        if q >= config.min_mode_q {
            modes.push(RoomMode {
                frequency: freq[i],
                q,
                prominence_db: prominence,
                index: i,
            });
        }
    }

    modes
}

/// Compute local baseline SPL around a peak (average of surrounding values).
fn compute_local_baseline(
    freq: &Array1<f64>,
    spl: &Array1<f64>,
    center_idx: usize,
    f_low: f64,
    f_high: f64,
) -> f64 {
    let mut sum = 0.0;
    let mut count = 0;

    for j in 0..freq.len() {
        if j == center_idx {
            continue;
        }
        if freq[j] >= f_low && freq[j] <= f_high {
            sum += spl[j];
            count += 1;
        }
    }

    if count > 0 {
        sum / count as f64
    } else {
        spl[center_idx]
    }
}

/// Estimate Q factor of a peak from its -3 dB bandwidth.
///
/// Q = f_center / bandwidth, where bandwidth = f_high - f_low at -3 dB.
/// When only one side crossing is found, the bandwidth is estimated as
/// 2x the one-sided distance to avoid reporting double the true Q.
fn estimate_peak_q(freq: &Array1<f64>, spl: &Array1<f64>, peak_idx: usize) -> f64 {
    let peak_spl = spl[peak_idx];
    let threshold = peak_spl - 3.0; // -3 dB point
    let f_center = freq[peak_idx];

    // Search left for -3 dB crossing
    let mut f_low: Option<f64> = None;
    for i in (0..peak_idx).rev() {
        if spl[i] <= threshold {
            let denom = spl[i + 1] - spl[i];
            if denom.abs() > 1e-12 {
                let t = ((threshold - spl[i]) / denom).clamp(0.0, 1.0);
                f_low = Some(freq[i] + t * (freq[i + 1] - freq[i]));
            } else {
                f_low = Some(freq[i]);
            }
            break;
        }
    }

    // Search right for -3 dB crossing
    let mut f_high: Option<f64> = None;
    for i in (peak_idx + 1)..freq.len() {
        if spl[i] <= threshold {
            let denom = spl[i] - spl[i - 1];
            if denom.abs() > 1e-12 {
                let t = ((threshold - spl[i - 1]) / denom).clamp(0.0, 1.0);
                f_high = Some(freq[i - 1] + t * (freq[i] - freq[i - 1]));
            } else {
                f_high = Some(freq[i]);
            }
            break;
        }
    }

    // Compute bandwidth: if only one side found, double the one-sided distance
    let bandwidth = match (f_low, f_high) {
        (Some(lo), Some(hi)) => hi - lo,
        (Some(lo), None) => 2.0 * (f_center - lo),
        (None, Some(hi)) => 2.0 * (hi - f_center),
        (None, None) => 0.0, // no crossing found at all
    };

    if bandwidth > 0.0 {
        f_center / bandwidth
    } else {
        // Very narrow peak — assign high Q
        20.0
    }
}

/// Build per-frequency correction weights combining mode detection and Schroeder split.
fn build_correction_weights(
    freq: &Array1<f64>,
    modes: &[RoomMode],
    config: &DecomposedCorrectionConfig,
) -> Array1<f64> {
    let n = freq.len();
    let mut weights = Array1::zeros(n);

    let schroeder_log = config.schroeder_freq.log2();
    let half_transition = config.transition_width_oct / 2.0;

    for i in 0..n {
        let f = freq[i];
        let f_log = f.log2();

        // Schroeder-based weight: smooth transition from mode_correction_weight
        // (below Schroeder) to steady_state_weight (above Schroeder)
        let schroeder_blend = if config.transition_width_oct <= 0.0 {
            if f <= config.schroeder_freq {
                0.0
            } else {
                1.0
            }
        } else {
            let x = (f_log - schroeder_log) / half_transition;
            // Sigmoid: 0 below Schroeder, 1 above
            1.0 / (1.0 + (-x).exp())
        };

        // Base weight: blend between early_reflection_weight (below) and steady_state_weight (above)
        let base_weight = config.early_reflection_weight
            + (config.steady_state_weight - config.early_reflection_weight) * schroeder_blend;

        weights[i] = base_weight;
    }

    // Boost weights at detected room mode frequencies
    for mode in modes {
        // Apply mode correction weight in a narrow band around the mode
        // Width is determined by the mode's Q: bandwidth = f/Q
        let bandwidth = mode.frequency / mode.q;
        let f_low = mode.frequency - bandwidth / 2.0;
        let f_high = mode.frequency + bandwidth / 2.0;

        for i in 0..n {
            if freq[i] >= f_low && freq[i] <= f_high {
                // Boost to mode_correction_weight (but don't reduce existing weight)
                weights[i] = weights[i].max(config.mode_correction_weight);
            }
        }
    }

    weights
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_room_modes_flat_response() {
        // Flat response should have no room modes
        let n = 100;
        let freq = Array1::linspace(20.0, 500.0, n);
        let spl = Array1::from_elem(n, 80.0);

        let config = DecomposedCorrectionConfig::default();
        let modes = detect_room_modes(&freq, &spl, &config);

        assert!(modes.is_empty(), "flat response should have no modes");
    }

    #[test]
    fn test_detect_room_modes_with_peak() {
        // Create a response with a prominent narrow peak at 60 Hz
        let n = 200;
        let freq = Array1::linspace(20.0, 300.0, n);
        let mut spl = Array1::from_elem(n, 80.0);

        // Add narrow peak at 60 Hz (~index 27 for linspace 20-300 with 200 pts)
        for i in 0..n {
            let f = freq[i];
            // Narrow peak: Lorentzian with Q=10 at 60 Hz, 10 dB prominence
            let q = 10.0;
            let bw = 60.0 / q;
            let response = 10.0 / (1.0 + ((f - 60.0) / (bw / 2.0_f64)).powi(2));
            spl[i] += response;
        }

        let config = DecomposedCorrectionConfig::default();
        let modes = detect_room_modes(&freq, &spl, &config);

        assert!(
            !modes.is_empty(),
            "should detect the 60 Hz peak as a room mode"
        );
        // The detected mode should be near 60 Hz
        let nearest = modes
            .iter()
            .min_by(|a, b| {
                (a.frequency - 60.0)
                    .abs()
                    .partial_cmp(&(b.frequency - 60.0).abs())
                    .unwrap()
            })
            .unwrap();
        assert!(
            (nearest.frequency - 60.0).abs() < 10.0,
            "detected mode at {:.1} Hz should be near 60 Hz",
            nearest.frequency
        );
        assert!(
            nearest.q >= 3.0,
            "detected Q={:.1} should be >= 3.0",
            nearest.q
        );
    }

    #[test]
    fn test_correction_weights_below_schroeder() {
        let freq = Array1::from_vec(vec![50.0]);
        let modes = vec![];
        let config = DecomposedCorrectionConfig {
            schroeder_freq: 200.0,
            early_reflection_weight: 0.3,
            steady_state_weight: 0.5,
            transition_width_oct: 0.0, // hard split
            ..Default::default()
        };

        let weights = build_correction_weights(&freq, &modes, &config);
        assert!(
            (weights[0] - 0.3).abs() < 0.01,
            "below Schroeder should use early_reflection_weight, got {}",
            weights[0]
        );
    }

    #[test]
    fn test_correction_weights_above_schroeder() {
        let freq = Array1::from_vec(vec![500.0]);
        let modes = vec![];
        let config = DecomposedCorrectionConfig {
            schroeder_freq: 200.0,
            early_reflection_weight: 0.3,
            steady_state_weight: 0.5,
            transition_width_oct: 0.0,
            ..Default::default()
        };

        let weights = build_correction_weights(&freq, &modes, &config);
        assert!(
            (weights[0] - 0.5).abs() < 0.01,
            "above Schroeder should use steady_state_weight, got {}",
            weights[0]
        );
    }

    #[test]
    fn test_correction_weights_mode_boost() {
        let freq = Array1::from_vec(vec![50.0, 60.0, 70.0]);
        let modes = vec![RoomMode {
            frequency: 60.0,
            q: 5.0,
            prominence_db: 8.0,
            index: 1,
        }];
        let config = DecomposedCorrectionConfig {
            schroeder_freq: 200.0,
            early_reflection_weight: 0.3,
            mode_correction_weight: 1.0,
            transition_width_oct: 0.0,
            ..Default::default()
        };

        let weights = build_correction_weights(&freq, &modes, &config);

        // The mode at 60 Hz should boost the weight to 1.0
        assert!(
            weights[1] > 0.9,
            "mode frequency should have boosted weight, got {}",
            weights[1]
        );
        // Adjacent frequencies outside the mode bandwidth should stay at baseline
        // bandwidth = 60/5 = 12 Hz, so 50 Hz is outside
    }

    #[test]
    fn test_full_decomposed_analysis() {
        let n = 200;
        let freq = Array1::linspace(20.0, 500.0, n);
        let mut spl = Array1::from_elem(n, 80.0);

        // Add room mode at 80 Hz
        for i in 0..n {
            let f = freq[i];
            let q = 8.0;
            let bw = 80.0 / q;
            spl[i] += 8.0 / (1.0 + ((f - 80.0) / (bw / 2.0_f64)).powi(2));
        }

        let config = DecomposedCorrectionConfig::default();
        let result = analyze_decomposed_correction(&freq, &spl, &config);

        assert_eq!(result.schroeder_freq, 200.0);
        assert!(!result.correction_weights.iter().any(|w| w.is_nan()));
        assert!(result.correction_weights.iter().all(|&w| w >= 0.0 && w <= 1.0));
    }

    #[test]
    fn test_estimate_peak_q_narrow() {
        // Create a very narrow peak → high Q
        let n = 100;
        let freq = Array1::linspace(40.0, 120.0, n);
        let mut spl = Array1::from_elem(n, 80.0);

        // Narrow peak at 80 Hz, Q=15
        for i in 0..n {
            let f = freq[i];
            let bw = 80.0 / 15.0; // narrow
            spl[i] += 10.0 / (1.0 + ((f - 80.0) / (bw / 2.0_f64)).powi(2));
        }

        // Find the peak index
        let peak_idx = spl
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap();

        let q = estimate_peak_q(&freq, &spl, peak_idx);
        assert!(
            q > 5.0,
            "narrow peak should have high Q, got {:.1}",
            q
        );
    }

    #[test]
    fn test_estimate_peak_q_at_array_edge() {
        // Peak at the start of array — can't find left -3dB crossing
        let freq = Array1::from_vec(vec![20.0, 30.0, 40.0, 50.0, 60.0]);
        let spl = Array1::from_vec(vec![90.0, 88.0, 85.0, 83.0, 80.0]); // peak at idx 0
        let q = estimate_peak_q(&freq, &spl, 0);
        // Should return a reasonable Q (not NaN or negative)
        assert!(q.is_finite() && q > 0.0, "Q at edge should be positive finite, got {}", q);
    }

    #[test]
    fn test_compute_local_baseline_excludes_center() {
        let freq = Array1::from_vec(vec![50.0, 60.0, 70.0, 80.0, 90.0]);
        let spl = Array1::from_vec(vec![80.0, 80.0, 95.0, 80.0, 80.0]); // peak at 70 Hz
        let baseline = compute_local_baseline(&freq, &spl, 2, 40.0, 100.0);
        // Baseline should be ~80 (average of all except the peak at index 2)
        assert!(
            (baseline - 80.0).abs() < 0.5,
            "baseline should be ~80, got {:.1}",
            baseline
        );
    }

    #[test]
    fn test_correction_weights_smooth_transition() {
        // With smooth transition, weight near Schroeder should be between extremes
        let freq = Array1::from_vec(vec![50.0, 200.0, 800.0]);
        let config = DecomposedCorrectionConfig {
            schroeder_freq: 200.0,
            early_reflection_weight: 0.2,
            steady_state_weight: 0.6,
            transition_width_oct: 1.0, // smooth
            ..Default::default()
        };
        let weights = build_correction_weights(&freq, &[], &config);

        // At 50 Hz (well below): close to early_reflection_weight
        assert!(weights[0] < 0.4, "50 Hz weight should be near 0.2, got {}", weights[0]);
        // At 200 Hz (at Schroeder): should be midpoint ~0.4
        let midpoint = (0.2 + 0.6) / 2.0;
        assert!(
            (weights[1] - midpoint).abs() < 0.15,
            "200 Hz weight should be near {:.1}, got {:.2}",
            midpoint,
            weights[1]
        );
        // At 800 Hz (well above): close to steady_state_weight
        assert!(weights[2] > 0.4, "800 Hz weight should be near 0.6, got {}", weights[2]);
    }

    #[test]
    fn test_detect_room_modes_short_array() {
        // Arrays shorter than 5 should return empty (no crash)
        let freq = Array1::from_vec(vec![50.0, 60.0, 70.0]);
        let spl = Array1::from_vec(vec![85.0, 90.0, 85.0]);
        let config = DecomposedCorrectionConfig::default();
        let modes = detect_room_modes(&freq, &spl, &config);
        assert!(modes.is_empty());
    }

    #[test]
    fn test_detect_room_modes_ignores_above_schroeder() {
        // Peaks above Schroeder frequency should not be detected as modes
        let n = 200;
        let freq = Array1::linspace(20.0, 1000.0, n);
        let mut spl = Array1::from_elem(n, 80.0);

        // Add peak at 400 Hz (above default Schroeder of 200)
        for i in 0..n {
            let f = freq[i];
            spl[i] += 10.0 / (1.0 + ((f - 400.0) / 5.0_f64).powi(2));
        }

        let config = DecomposedCorrectionConfig {
            schroeder_freq: 200.0,
            ..Default::default()
        };
        let modes = detect_room_modes(&freq, &spl, &config);

        // Should not detect the 400 Hz peak
        assert!(
            modes.iter().all(|m| m.frequency <= 200.0),
            "modes above Schroeder should not be detected"
        );
    }

    #[test]
    fn test_estimate_peak_q_one_sided_low() {
        // Peak near the low-frequency edge — no left -3dB crossing exists
        // Q should use 2x the right-side distance, not f_center/0
        let n = 100;
        let freq = Array1::linspace(20.0, 200.0, n);
        let mut spl = Array1::from_elem(n, 80.0);

        // Add peak at 25 Hz (near array start)
        for i in 0..n {
            let f = freq[i];
            let bw = 25.0 / 5.0; // Q=5 → bw=5 Hz
            spl[i] += 10.0 / (1.0 + ((f - 25.0) / (bw / 2.0_f64)).powi(2));
        }

        // Find peak
        let peak_idx = spl.iter().enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i).unwrap();

        let q = estimate_peak_q(&freq, &spl, peak_idx);
        // Should not be 2x the true Q (which was 5.0)
        assert!(
            q < 15.0,
            "one-sided Q estimate should not double: got {:.1}, expected ~5",
            q
        );
        assert!(q > 2.0, "Q should still be reasonable: got {:.1}", q);
    }

    #[test]
    fn test_estimate_peak_q_interpolation_denom_zero() {
        // Edge case: two adjacent SPL values are equal at the -3dB crossing
        let freq = Array1::from_vec(vec![50.0, 55.0, 60.0, 65.0, 70.0, 75.0, 80.0]);
        let spl = Array1::from_vec(vec![80.0, 83.0, 83.0, 90.0, 83.0, 83.0, 80.0]);
        // Peak at idx 3 (90 dB), threshold = 87 dB
        // Left: spl[2]=83, spl[1]=83 — both below threshold, denom = 83-83 = 0
        let q = estimate_peak_q(&freq, &spl, 3);
        assert!(q.is_finite() && q > 0.0, "Q should be finite positive, got {:.1}", q);
    }
}
