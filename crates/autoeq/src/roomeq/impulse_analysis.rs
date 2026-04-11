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
    /// Default: 250.0 Hz (typical for medium rooms)
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
    /// Default: 0.4 (moderate correction — preserve speaker character)
    pub steady_state_weight: f64,

    /// Transition width around Schroeder frequency in octaves.
    /// Default: 0.5 octaves
    pub transition_width_oct: f64,
}

impl Default for DecomposedCorrectionConfig {
    fn default() -> Self {
        Self {
            schroeder_freq: 250.0,
            min_mode_q: 3.0,
            min_mode_prominence_db: 3.0,
            mode_correction_weight: 1.0,
            early_reflection_weight: 0.3,
            steady_state_weight: 0.4,
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
pub fn detect_room_modes(
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
            if f <= config.schroeder_freq { 0.0 } else { 1.0 }
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

// ============================================================================
// Narrow-null detection for asymmetric-loss dip suppression
// ============================================================================

/// Configuration for narrow-null detection.
///
/// A "narrow null" is a high-Q dip in the magnitude response caused by
/// destructive interference (room modes, SBIR, early reflections). These
/// nulls cannot be filled by EQ boost — the cancellation happens *after*
/// the EQ, so adding input energy just raises both the direct wave and
/// its anti-phase reflection by the same ratio. The asymmetric loss
/// should de-weight the dip branch at these frequencies.
///
/// Broad dips (low-Q), by contrast, are usually legitimate response
/// deficits (driver integration, baffle step, room absorption) and
/// remain fillable. They are *not* suppressed.
#[derive(Debug, Clone)]
pub struct NullDetectionConfig {
    /// Minimum Q factor to treat a local minimum as a narrow null.
    /// Mirrors `DecomposedCorrectionConfig::min_mode_q`. Default: 3.0.
    pub min_null_q: f64,

    /// Minimum depth (dB below the local baseline) for a minimum to be
    /// classified as a narrow null. Slightly stricter than the peak
    /// prominence default because we are gating suppression: it is safer
    /// to suppress too few nulls than too many. Default: 4.0 dB.
    pub min_null_depth_db: f64,
}

impl Default for NullDetectionConfig {
    fn default() -> Self {
        Self {
            min_null_q: 3.0,
            min_null_depth_db: 4.0,
        }
    }
}

/// A detected narrow null (acoustic cancellation).
#[derive(Debug, Clone)]
pub struct NarrowNull {
    /// Center frequency of the null (Hz)
    pub frequency: f64,
    /// Estimated Q factor (narrowness)
    pub q: f64,
    /// Depth in dB below the surrounding baseline (positive value)
    pub depth_db: f64,
    /// Index into the frequency array
    pub index: usize,
}

/// Detect narrow nulls in the frequency response across the whole
/// measurement band.
///
/// Mirrors `detect_room_modes` with the sign flipped: find local minima,
/// compute depth relative to a ±1 octave local baseline, estimate Q from
/// the +3 dB bandwidth around the nadir, and keep only the minima that
/// pass both the depth and Q thresholds.
///
/// Unlike room-mode peak detection, this scans the full frequency range
/// rather than stopping at the Schroeder frequency — narrow nulls exist
/// above Schroeder too (SBIR, early reflections, crossover interactions)
/// and they are just as unfillable there as in the modal region.
pub fn detect_narrow_nulls(
    freq: &Array1<f64>,
    spl: &Array1<f64>,
    config: &NullDetectionConfig,
) -> Vec<NarrowNull> {
    let mut nulls = Vec::new();
    let n = freq.len();
    if n < 5 {
        return nulls;
    }

    for i in 2..n - 2 {
        // Local minimum: lower than both neighbours at distance 1 and 2
        // (same 2-sample window as detect_room_modes for robustness).
        let is_min = spl[i] < spl[i - 1]
            && spl[i] < spl[i + 1]
            && spl[i] < spl[i - 2]
            && spl[i] < spl[i + 2];

        if !is_min {
            continue;
        }

        let f_low_window = freq[i] / 2.0;
        let f_high_window = freq[i] * 2.0;
        let baseline = compute_local_baseline(freq, spl, i, f_low_window, f_high_window);
        let depth = baseline - spl[i];

        if depth < config.min_null_depth_db {
            continue;
        }

        let q = estimate_dip_q(freq, spl, i);

        if q >= config.min_null_q {
            nulls.push(NarrowNull {
                frequency: freq[i],
                q,
                depth_db: depth,
                index: i,
            });
        }
    }

    nulls
}

/// Estimate Q factor of a dip from its +3 dB bandwidth.
///
/// Symmetric counterpart of `estimate_peak_q`: searches left and right
/// from the nadir for the first crossing of `spl[peak_idx] + 3 dB`. When
/// only one side is found the bandwidth is estimated as 2× the one-sided
/// distance, same convention as the peak helper.
fn estimate_dip_q(freq: &Array1<f64>, spl: &Array1<f64>, dip_idx: usize) -> f64 {
    let dip_spl = spl[dip_idx];
    let threshold = dip_spl + 3.0; // +3 dB from the nadir
    let f_center = freq[dip_idx];

    // Search left for the +3 dB crossing
    let mut f_low: Option<f64> = None;
    for i in (0..dip_idx).rev() {
        if spl[i] >= threshold {
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

    // Search right for the +3 dB crossing
    let mut f_high: Option<f64> = None;
    for i in (dip_idx + 1)..freq.len() {
        if spl[i] >= threshold {
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

    let bandwidth = match (f_low, f_high) {
        (Some(lo), Some(hi)) => hi - lo,
        (Some(lo), None) => 2.0 * (f_center - lo),
        (None, Some(hi)) => 2.0 * (hi - f_center),
        (None, None) => 0.0,
    };

    if bandwidth > 0.0 {
        f_center / bandwidth
    } else {
        // No +3 dB crossing found: extremely narrow dip, treat as very high Q
        20.0
    }
}

/// Build a per-frequency dip-suppression mask from a list of narrow nulls.
///
/// The mask starts at 1.0 everywhere and drops toward 0.0 inside the
/// -3 dB bandwidth `[f − bw/2, f + bw/2]` of each detected null. A short
/// raised-cosine taper on each side keeps the mask C⁰-continuous, which
/// behaves better in gradient-free optimizers than a hard step.
///
/// The asymmetric loss multiplies *only the dip branch* of its per-sample
/// weights by this mask — narrow peaks at the same frequency are not
/// suppressed.
pub fn build_null_suppression_mask(freq: &Array1<f64>, nulls: &[NarrowNull]) -> Array1<f64> {
    let n = freq.len();
    let mut mask = Array1::ones(n);

    if nulls.is_empty() {
        return mask;
    }

    // Raised-cosine taper width, expressed as a fraction of the null's
    // own half-bandwidth on each side. 0.5 means the taper spans the
    // outer half of the half-bandwidth.
    const TAPER_FRAC: f64 = 0.5;

    for null in nulls {
        let bw = null.frequency / null.q.max(1e-6);
        let f_inner = bw * 0.5 * (1.0 - TAPER_FRAC);
        let f_outer = bw * 0.5;

        for i in 0..n {
            let df = (freq[i] - null.frequency).abs();
            if df > f_outer {
                continue;
            }
            let w = if df <= f_inner {
                0.0
            } else {
                // Raised cosine rising from 0 at f_inner to 1 at f_outer.
                let t = (df - f_inner) / (f_outer - f_inner);
                0.5 * (1.0 - (std::f64::consts::PI * t).cos())
            };
            // Overlapping nulls: keep the strongest suppression (lowest w).
            if w < mask[i] {
                mask[i] = w;
            }
        }
    }

    mask
}

// ============================================================================
// SSIR-informed correction weights
// ============================================================================

/// Build per-frequency correction weights informed by SSIR time-domain analysis
/// of a measured room impulse response.
///
/// Instead of using a static Schroeder frequency to divide modal/diffuse regions,
/// this uses SSIR's detected reflection boundaries and mixing time to set data-driven
/// correction depths:
///
/// - **Direct sound segment**: full correction (weight = 1.0) — this is the speaker's
///   intrinsic response, correctable regardless of position.
/// - **Early reflection segments** (detected by SSIR): reduced correction — these are
///   position-dependent; over-correcting them creates a correction that only works
///   at the exact measurement position.
/// - **Reverberant tail** (after mixing time): moderate correction — the diffuse field
///   is spatially stable but represents room character rather than speaker defects.
/// - **Room modes**: full correction regardless of region — narrow resonances are
///   room-intrinsic and spatially consistent.
///
/// The result is the same shape as the frequency array, ready to multiply with
/// the deviation curve in the EQ optimizer.
pub fn build_ssir_correction_weights(
    freq: &Array1<f64>,
    spl: &Array1<f64>,
    ssir_result: &math_rir::SsirResult,
    config: &DecomposedCorrectionConfig,
) -> DecomposedCorrectionResult {
    let n = freq.len();

    // 1. Detect room modes from frequency-domain data (same as before)
    let room_modes = detect_room_modes(freq, spl, config);

    // 2. Boundary between modal region and diffuse region.
    //
    //    Previous versions derived this from the SSIR mixing time via a
    //    `1 / T_mix` heuristic, but that is dimensionally wrong: mixing
    //    time is a *time-domain* property (the transition from discrete
    //    early reflections to a diffuse reverberant tail), while the
    //    Schroeder frequency is a *frequency-domain* property (where
    //    modal density is high enough that individual modes overlap
    //    statistically). The two are not reciprocals of each other —
    //    there's no physical law relating them that way — and the
    //    heuristic systematically under-estimated Schroeder by ~5–10×
    //    for small listening rooms (e.g. a 30 m³ room reported ~26 Hz,
    //    clamped up to a 50 Hz floor, when the true Schroeder is
    //    ~230 Hz).
    //
    //    The correct Schroeder formula is `f_S ≈ 2000 · √(RT60 / V)`
    //    with V in m³ and RT60 in seconds. Computing it here would need
    //    a measured RT60 and known room volume, neither of which the
    //    SSIR path currently threads through. Until that plumbing
    //    exists, trust the caller-provided `config.schroeder_freq` —
    //    which defaults to a sensible 250 Hz and can be overridden per
    //    room in the JSON config. The config value stays accurate
    //    across rooms because the user can override it, whereas the
    //    heuristic was broken by physics regardless of input.
    let ssir_boundary_freq = config.schroeder_freq;

    // 3. Determine the time extent of early reflections for energy-based weighting.
    //    If SSIR detected many reflections, the early sound field is rich and
    //    position-dependent → lower correction weight above the modal region.
    //    If few reflections (dry room), we can correct more aggressively.
    let num_reflections = ssir_result.num_reflections();
    let reflection_weight = if num_reflections > 8 {
        // Rich early reflection field — be conservative
        config.early_reflection_weight * 0.7
    } else if num_reflections > 4 {
        config.early_reflection_weight
    } else {
        // Dry room — can correct more
        config.early_reflection_weight * 1.5_f64.min(1.0)
    };

    // 4. Build per-frequency correction weights using the SSIR-derived boundary
    let mut weights = Array1::zeros(n);

    let boundary_log = ssir_boundary_freq.log2();
    let half_transition = config.transition_width_oct / 2.0;

    for i in 0..n {
        let f = freq[i];
        let f_log = f.log2();

        // Smooth transition from modal to diffuse region
        let blend = if config.transition_width_oct <= 0.0 {
            if f <= ssir_boundary_freq { 0.0 } else { 1.0 }
        } else {
            let x = (f_log - boundary_log) / half_transition;
            1.0 / (1.0 + (-x).exp())
        };

        // Below boundary: early_reflection_weight (modal region)
        // Above boundary: steady_state_weight (diffuse field)
        let base_weight =
            reflection_weight + (config.steady_state_weight - reflection_weight) * blend;

        weights[i] = base_weight;
    }

    // 5. Boost weights at detected room mode frequencies (full correction)
    for mode in &room_modes {
        let bandwidth = mode.frequency / mode.q;
        let f_low = mode.frequency - bandwidth / 2.0;
        let f_high = mode.frequency + bandwidth / 2.0;

        for i in 0..n {
            if freq[i] >= f_low && freq[i] <= f_high {
                weights[i] = weights[i].max(config.mode_correction_weight);
            }
        }
    }

    DecomposedCorrectionResult {
        room_modes,
        correction_weights: weights,
        schroeder_freq: ssir_boundary_freq,
    }
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

        assert_eq!(result.schroeder_freq, 250.0);
        assert!(!result.correction_weights.iter().any(|w| w.is_nan()));
        assert!(
            result
                .correction_weights
                .iter()
                .all(|&w| (0.0..=1.0).contains(&w))
        );
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
        assert!(q > 5.0, "narrow peak should have high Q, got {:.1}", q);
    }

    #[test]
    fn test_estimate_peak_q_at_array_edge() {
        // Peak at the start of array — can't find left -3dB crossing
        let freq = Array1::from_vec(vec![20.0, 30.0, 40.0, 50.0, 60.0]);
        let spl = Array1::from_vec(vec![90.0, 88.0, 85.0, 83.0, 80.0]); // peak at idx 0
        let q = estimate_peak_q(&freq, &spl, 0);
        // Should return a reasonable Q (not NaN or negative)
        assert!(
            q.is_finite() && q > 0.0,
            "Q at edge should be positive finite, got {}",
            q
        );
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
        assert!(
            weights[0] < 0.4,
            "50 Hz weight should be near 0.2, got {}",
            weights[0]
        );
        // At 200 Hz (at Schroeder): should be midpoint ~0.4
        let midpoint = (0.2 + 0.6) / 2.0;
        assert!(
            (weights[1] - midpoint).abs() < 0.15,
            "200 Hz weight should be near {:.1}, got {:.2}",
            midpoint,
            weights[1]
        );
        // At 800 Hz (well above): close to steady_state_weight
        assert!(
            weights[2] > 0.4,
            "800 Hz weight should be near 0.6, got {}",
            weights[2]
        );
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
        let peak_idx = spl
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap();

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
        assert!(
            q.is_finite() && q > 0.0,
            "Q should be finite positive, got {:.1}",
            q
        );
    }

    #[test]
    fn test_decomposed_correction_config_defaults() {
        let config = DecomposedCorrectionConfig::default();
        assert_eq!(config.schroeder_freq, 250.0);
        assert_eq!(config.steady_state_weight, 0.4);
        assert_eq!(config.min_mode_q, 3.0);
        assert_eq!(config.min_mode_prominence_db, 3.0);
        assert_eq!(config.mode_correction_weight, 1.0);
        assert_eq!(config.early_reflection_weight, 0.3);
        assert_eq!(config.transition_width_oct, 0.5);
    }

    // --- narrow-null detection ---

    fn log_linspace(f_min: f64, f_max: f64, n: usize) -> Array1<f64> {
        let lo = f_min.ln();
        let hi = f_max.ln();
        Array1::from_iter((0..n).map(|i| (lo + (hi - lo) * i as f64 / (n - 1) as f64).exp()))
    }

    #[test]
    fn test_detect_narrow_nulls_flat_response_is_empty() {
        let freq = log_linspace(20.0, 20000.0, 512);
        let spl = Array1::from_elem(freq.len(), 80.0);
        let nulls = detect_narrow_nulls(&freq, &spl, &NullDetectionConfig::default());
        assert!(
            nulls.is_empty(),
            "flat response must not produce narrow nulls, got {nulls:?}"
        );
    }

    #[test]
    fn test_detect_narrow_nulls_finds_high_q_notch() {
        // -15 dB Lorentzian notch at 80 Hz with Q=10.
        let freq = log_linspace(20.0, 20000.0, 512);
        let f0 = 80.0;
        let q = 10.0;
        let bw = f0 / q;
        let spl: Array1<f64> = freq.mapv(|f| {
            let x = (f - f0) / (bw / 2.0);
            80.0 - 15.0 / (1.0 + x * x)
        });
        let nulls = detect_narrow_nulls(&freq, &spl, &NullDetectionConfig::default());
        assert!(
            !nulls.is_empty(),
            "should detect the 80 Hz Q=10 notch as a narrow null"
        );
        let nearest = nulls
            .iter()
            .min_by(|a, b| {
                (a.frequency - f0)
                    .abs()
                    .partial_cmp(&(b.frequency - f0).abs())
                    .unwrap()
            })
            .unwrap();
        assert!(
            (nearest.frequency - f0).abs() < 5.0,
            "detected null at {:.1} Hz should be near {f0} Hz",
            nearest.frequency
        );
        assert!(
            nearest.q >= 3.0,
            "detected Q={:.1} should exceed min_null_q=3",
            nearest.q
        );
        assert!(
            nearest.depth_db >= 4.0,
            "detected depth={:.1} should exceed min_null_depth_db=4",
            nearest.depth_db
        );
    }

    #[test]
    fn test_detect_narrow_nulls_ignores_broad_dip() {
        // Broad 8 dB dip centred at ~400 Hz with Q ~= 1 (unfillable-to-q check).
        let freq = log_linspace(20.0, 20000.0, 512);
        let f0 = 400.0;
        let q = 0.8;
        let bw = f0 / q;
        let spl: Array1<f64> = freq.mapv(|f| {
            let x = (f - f0) / (bw / 2.0);
            80.0 - 8.0 / (1.0 + x * x)
        });
        let nulls = detect_narrow_nulls(&freq, &spl, &NullDetectionConfig::default());
        assert!(
            nulls.is_empty(),
            "a broad Q=0.8 dip must not be flagged as a narrow null, got {nulls:?}"
        );
    }

    #[test]
    fn test_build_null_suppression_mask_is_zero_at_null() {
        let freq = log_linspace(20.0, 20000.0, 512);
        // Handcrafted null list instead of going through detect_narrow_nulls
        // so the test is purely about the mask construction.
        let nulls = vec![NarrowNull {
            frequency: 80.0,
            q: 10.0,
            depth_db: 15.0,
            index: 0,
        }];
        let mask = build_null_suppression_mask(&freq, &nulls);
        assert_eq!(mask.len(), freq.len());

        // At the null centre the mask must be close to zero.
        let center_idx = freq
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (*a - 80.0).abs().partial_cmp(&(*b - 80.0).abs()).unwrap())
            .unwrap()
            .0;
        assert!(
            mask[center_idx] < 1e-6,
            "mask at null centre must be ~0, got {}",
            mask[center_idx]
        );

        // Far away from the null the mask must be 1.0.
        let far_idx = freq
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (*a - 5000.0)
                    .abs()
                    .partial_cmp(&(*b - 5000.0).abs())
                    .unwrap()
            })
            .unwrap()
            .0;
        assert!(
            (mask[far_idx] - 1.0).abs() < 1e-12,
            "mask far from any null must be 1.0, got {}",
            mask[far_idx]
        );

        // The mask must be C⁰-continuous: no value outside [0, 1].
        for (i, &m) in mask.iter().enumerate() {
            assert!(
                (0.0..=1.0).contains(&m),
                "mask[{i}] = {m} must be in [0, 1]"
            );
        }
    }

    #[test]
    fn test_build_null_suppression_mask_empty_input_is_all_ones() {
        let freq = log_linspace(20.0, 20000.0, 256);
        let mask = build_null_suppression_mask(&freq, &[]);
        assert!(
            mask.iter().all(|&m| (m - 1.0).abs() < 1e-12),
            "empty null list must yield an all-ones mask"
        );
    }

    #[test]
    fn test_null_detection_config_defaults() {
        let config = NullDetectionConfig::default();
        assert_eq!(config.min_null_q, 3.0);
        assert_eq!(config.min_null_depth_db, 4.0);
    }
}
