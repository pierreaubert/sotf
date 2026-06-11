//! Bass phase confidence gate — GD-Opt v2 Phase GD-1g.
//!
//! The gate decides whether the measured phase in a batch of
//! [`Curve`]s is trustworthy enough for the GD-Opt v2 bass-band
//! optimiser to consume. It returns a [`BassPhaseConfidence`] verdict:
//! either `Trustworthy { mean_coherence }` or a `Degraded { reason }`
//! identifier matching one of the advisories enumerated in
//! `docs/gd_opt_v2_plan.md` §2.8 / §3.5.
//!
//! The gate is **read-only**: it never modifies the curves or the
//! recording configuration, and it emits no log output. Advisory
//! surfacing (logs, `RoomEqReport`) happens at a higher level based on
//! the returned reason.
//!
//! The spec's narrow signature is
//!   `bass_phase_confidence(curves: &[Curve], band: (f64, f64))
//!     -> BassPhaseConfidence`
//! but several of the §2.8 triggers depend on the
//! [`RecordingConfiguration`] that produced the curves
//! (e.g. `num_sweeps`, `bass_octave_duration_s`). The extended
//! signature takes an `Option<&RecordingConfiguration>` so config-
//! driven checks can run when the caller has the data and gracefully
//! skip when they don't.
//!
//! # Advisory reasons (in evaluation order)
//!
//! - `"no_curves"` — caller passed an empty slice.
//! - `"invalid_band"` — `band` is not a valid `(lo, hi)` with `0 ≤ lo < hi`.
//! - `"no_phase_data"` — at least one curve has no `phase` column
//!   (pre-GD-Opt-v2 curve or measurement without phase).
//! - `"no_coherence_data"` — at least one curve has no `coherence`
//!   column (single-sweep capture or legacy session file).
//! - `"insufficient_bass_duration"` — `RecordingConfiguration` reports
//!   `num_sweeps < 4` or `bass_octave_duration_s < 2.0`.
//! - `"coherence_below_threshold"` — mean γ² across the band is below
//!   the `coherence_threshold` from the recording config (default
//!   [`DEFAULT_COHERENCE_THRESHOLD`] = 0.9).
//! - `"snr_below_10db"` — at any bin in the band the signal SPL sits
//!   within [`MIN_SNR_DB`] dB of the captured `noise_floor_db`. Only
//!   evaluated when every curve carries a `noise_floor_db` column.
//!
//! The soft-warning advisories from §2.8 (`"mic_phase_uncalibrated"`,
//! `"bass_anchor_unreliable"`, `"no_spl_calibration"`) are **not**
//! produced by this gate — they are "warn, proceed" cases that
//! belong to a separate advisory pass the optimiser emits alongside
//! the verdict.

use crate::Curve;
use crate::roomeq::types::RecordingConfiguration;

/// Verdict of the bass phase confidence gate.
///
/// Semantics mirror §3.5 of `docs/gd_opt_v2_plan.md`: a `Degraded`
/// verdict means the optimiser must refuse to touch bass correction
/// and surface the `reason` as an advisory to the user. A
/// `Trustworthy` verdict carries the mean coherence used by the
/// optimiser's bin-weighting term.
#[derive(Debug, Clone, PartialEq)]
pub enum BassPhaseConfidence {
    /// Phase data meets every precondition for GD-Opt v2 bass
    /// correction. `mean_coherence` is the mean γ² across the
    /// evaluated band — used as the optimiser's objective weight.
    Trustworthy { mean_coherence: f64 },
    /// One or more preconditions failed. `reason` is the first
    /// triggering identifier (evaluation order is documented at the
    /// module level); the optimiser must not run on bass.
    Degraded { reason: &'static str },
}

/// Default coherence threshold — γ² ≥ 0.9 across the evaluated band
/// is the cut-off for "trustworthy" declared in §2.8 of the plan.
pub const DEFAULT_COHERENCE_THRESHOLD: f64 = 0.9;

/// Minimum in-band signal-to-noise-floor ratio in dB. Pairs with the
/// `"snr_below_10db"` advisory from §2.8.
pub const MIN_SNR_DB: f64 = 10.0;

/// Minimum bass sweep duration per octave to trust phase below the
/// Schroeder frequency. Enforces part of `"insufficient_bass_duration"`.
pub const MIN_BASS_OCTAVE_DURATION_S: f32 = 2.0;

/// Minimum number of sweeps required for coherence averaging to be
/// meaningful. Enforces part of `"insufficient_bass_duration"`.
pub const MIN_NUM_SWEEPS: u8 = 4;

/// Run the bass phase confidence gate.
///
/// # Arguments
/// * `curves` — the measured response per channel. All curves must
///   have the same phase/coherence availability; one curve missing
///   phase degrades the whole decision.
/// * `band` — `(lo, hi)` frequency range in Hz over which to
///   evaluate. Typically `(min_freq, schroeder_freq)` or the derived
///   bass band `(band_lo, band_hi)` from §3.4 of the plan.
/// * `recording` — optional recording configuration. When present,
///   config-driven checks (`num_sweeps`, `bass_octave_duration_s`,
///   `coherence_threshold` override) are applied. When `None`, those
///   checks are skipped — the gate trusts the caller to have
///   validated them out of band.
pub fn bass_phase_confidence(
    curves: &[Curve],
    band: (f64, f64),
    recording: Option<&RecordingConfiguration>,
) -> BassPhaseConfidence {
    if curves.is_empty() {
        return BassPhaseConfidence::Degraded {
            reason: "no_curves",
        };
    }

    let (band_lo, band_hi) = band;
    if !band_lo.is_finite() || !band_hi.is_finite() || band_lo < 0.0 || band_hi <= band_lo {
        return BassPhaseConfidence::Degraded {
            reason: "invalid_band",
        };
    }

    if curves.iter().any(|c| c.phase.is_none()) {
        return BassPhaseConfidence::Degraded {
            reason: "no_phase_data",
        };
    }

    if curves.iter().any(|c| c.coherence.is_none()) {
        return BassPhaseConfidence::Degraded {
            reason: "no_coherence_data",
        };
    }

    if let Some(rec) = recording {
        let num_sweeps = rec.num_sweeps.unwrap_or(1);
        let octave_duration = rec.bass_octave_duration_s.unwrap_or(0.0);
        if num_sweeps < MIN_NUM_SWEEPS || octave_duration < MIN_BASS_OCTAVE_DURATION_S {
            return BassPhaseConfidence::Degraded {
                reason: "insufficient_bass_duration",
            };
        }
    }

    let coh_threshold = recording
        .and_then(|r| r.coherence_threshold)
        .map(|v| v as f64)
        .unwrap_or(DEFAULT_COHERENCE_THRESHOLD);

    let mean_coh = mean_coherence_in_band(curves, band_lo, band_hi);
    if mean_coh < coh_threshold {
        return BassPhaseConfidence::Degraded {
            reason: "coherence_below_threshold",
        };
    }

    if has_snr_data(curves) && !snr_above_threshold(curves, band_lo, band_hi, MIN_SNR_DB) {
        return BassPhaseConfidence::Degraded {
            reason: "snr_below_10db",
        };
    }

    BassPhaseConfidence::Trustworthy {
        mean_coherence: mean_coh,
    }
}

/// Mean γ² across `[band_lo, band_hi]` over every bin of every curve.
/// Returns `0.0` if no bins fall inside the band.
fn mean_coherence_in_band(curves: &[Curve], band_lo: f64, band_hi: f64) -> f64 {
    let mut sum = 0.0_f64;
    let mut count = 0_usize;
    for c in curves {
        let coh = c
            .coherence
            .as_ref()
            .expect("coherence presence checked by the caller");
        if c.freq.len() != coh.len() {
            continue; // malformed curve; skip
        }
        for (f, cv) in c.freq.iter().zip(coh.iter()) {
            if *f >= band_lo && *f <= band_hi && cv.is_finite() {
                sum += cv;
                count += 1;
            }
        }
    }
    if count == 0 { 0.0 } else { sum / count as f64 }
}

fn has_snr_data(curves: &[Curve]) -> bool {
    curves.iter().all(|c| c.noise_floor_db.is_some())
}

/// `true` iff every in-band bin has `spl - noise_floor_db >= min_snr_db`.
fn snr_above_threshold(curves: &[Curve], band_lo: f64, band_hi: f64, min_snr_db: f64) -> bool {
    for c in curves {
        let nf = c
            .noise_floor_db
            .as_ref()
            .expect("noise_floor_db presence checked by the caller");
        if c.freq.len() != c.spl.len() || c.freq.len() != nf.len() {
            continue; // malformed curve; skip rather than misreport
        }
        for ((&f, &signal_db), &noise_db) in c.freq.iter().zip(c.spl.iter()).zip(nf.iter()) {
            if f >= band_lo
                && f <= band_hi
                && signal_db.is_finite()
                && noise_db.is_finite()
                && signal_db - noise_db < min_snr_db
            {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    fn log_freqs(n: usize, lo: f64, hi: f64) -> Array1<f64> {
        Array1::from_vec(
            (0..n)
                .map(|i| lo * (hi / lo).powf(i as f64 / (n - 1) as f64))
                .collect(),
        )
    }

    /// Build a curve where every optional column has the same length
    /// as `freq` and `spl` — i.e. a curve that passes every gate
    /// precondition.
    fn healthy_curve(n: usize, coherence: f64, spl_db: f64, noise_db: f64) -> Curve {
        let freq = log_freqs(n, 20.0, 200.0);
        let spl = Array1::from_elem(n, spl_db);
        let phase = Array1::from_elem(n, 0.0);
        let coh = Array1::from_elem(n, coherence);
        let noise = Array1::from_elem(n, noise_db);
        Curve {
            freq,
            spl,
            phase: Some(phase),
            coherence: Some(coh),
            noise_floor_db: Some(noise),
            ..Default::default()
        }
    }

    fn healthy_recording() -> RecordingConfiguration {
        RecordingConfiguration {
            num_sweeps: Some(4),
            bass_octave_duration_s: Some(3.0),
            coherence_threshold: Some(0.9),
            ..Default::default()
        }
    }

    #[test]
    fn empty_curves_returns_no_curves() {
        let v = bass_phase_confidence(&[], (20.0, 100.0), None);
        assert_eq!(
            v,
            BassPhaseConfidence::Degraded {
                reason: "no_curves"
            }
        );
    }

    #[test]
    fn invalid_band_returns_invalid_band() {
        let c = [healthy_curve(16, 0.95, 85.0, -60.0)];
        // hi <= lo
        assert_eq!(
            bass_phase_confidence(&c, (100.0, 100.0), None),
            BassPhaseConfidence::Degraded {
                reason: "invalid_band"
            }
        );
        // lo negative
        assert_eq!(
            bass_phase_confidence(&c, (-10.0, 100.0), None),
            BassPhaseConfidence::Degraded {
                reason: "invalid_band"
            }
        );
        // non-finite
        assert_eq!(
            bass_phase_confidence(&c, (20.0, f64::INFINITY), None),
            BassPhaseConfidence::Degraded {
                reason: "invalid_band"
            }
        );
    }

    #[test]
    fn missing_phase_returns_no_phase_data() {
        let mut c = healthy_curve(16, 0.95, 85.0, -60.0);
        c.phase = None;
        assert_eq!(
            bass_phase_confidence(&[c], (20.0, 100.0), None),
            BassPhaseConfidence::Degraded {
                reason: "no_phase_data"
            }
        );
    }

    #[test]
    fn missing_coherence_returns_no_coherence_data() {
        let mut c = healthy_curve(16, 0.95, 85.0, -60.0);
        c.coherence = None;
        assert_eq!(
            bass_phase_confidence(&[c], (20.0, 100.0), None),
            BassPhaseConfidence::Degraded {
                reason: "no_coherence_data"
            }
        );
    }

    #[test]
    fn too_few_sweeps_returns_insufficient_bass_duration() {
        let c = [healthy_curve(16, 0.95, 85.0, -60.0)];
        let mut rec = healthy_recording();
        rec.num_sweeps = Some(1);
        assert_eq!(
            bass_phase_confidence(&c, (20.0, 100.0), Some(&rec)),
            BassPhaseConfidence::Degraded {
                reason: "insufficient_bass_duration"
            }
        );
    }

    #[test]
    fn too_short_bass_octave_returns_insufficient_bass_duration() {
        let c = [healthy_curve(16, 0.95, 85.0, -60.0)];
        let mut rec = healthy_recording();
        rec.bass_octave_duration_s = Some(1.0); // below the 2.0 floor
        assert_eq!(
            bass_phase_confidence(&c, (20.0, 100.0), Some(&rec)),
            BassPhaseConfidence::Degraded {
                reason: "insufficient_bass_duration"
            }
        );
    }

    #[test]
    fn low_coherence_returns_coherence_below_threshold() {
        let c = [healthy_curve(16, 0.5, 85.0, -60.0)];
        let rec = healthy_recording();
        assert_eq!(
            bass_phase_confidence(&c, (20.0, 100.0), Some(&rec)),
            BassPhaseConfidence::Degraded {
                reason: "coherence_below_threshold"
            }
        );
    }

    #[test]
    fn low_snr_returns_snr_below_10db() {
        // signal − noise = 85 − 80 = 5 dB, well below the 10 dB floor
        let c = [healthy_curve(16, 0.95, 85.0, 80.0)];
        let rec = healthy_recording();
        assert_eq!(
            bass_phase_confidence(&c, (20.0, 100.0), Some(&rec)),
            BassPhaseConfidence::Degraded {
                reason: "snr_below_10db"
            }
        );
    }

    #[test]
    fn trustworthy_when_everything_passes() {
        let c = [healthy_curve(32, 0.95, 85.0, -60.0)];
        let rec = healthy_recording();
        match bass_phase_confidence(&c, (20.0, 100.0), Some(&rec)) {
            BassPhaseConfidence::Trustworthy { mean_coherence } => {
                assert!(
                    (mean_coherence - 0.95).abs() < 1e-6,
                    "mean_coherence should ≈ 0.95, got {mean_coherence}"
                );
            }
            other => panic!("expected Trustworthy, got {other:?}"),
        }
    }

    #[test]
    fn trustworthy_without_recording_config_uses_defaults() {
        let c = [healthy_curve(32, 0.95, 85.0, -60.0)];
        match bass_phase_confidence(&c, (20.0, 100.0), None) {
            BassPhaseConfidence::Trustworthy { .. } => {}
            other => panic!("expected Trustworthy, got {other:?}"),
        }
    }

    #[test]
    fn no_noise_floor_skips_snr_check() {
        // With healthy coherence, absent noise_floor_db should NOT
        // trigger snr_below_10db — the check is only evaluated when
        // every curve carries the column.
        let mut c = healthy_curve(16, 0.95, 85.0, -60.0);
        c.noise_floor_db = None;
        let rec = healthy_recording();
        // Missing coherence would trigger "no_coherence_data", so only
        // drop noise_floor_db and keep coherence intact.
        match bass_phase_confidence(&[c], (20.0, 100.0), Some(&rec)) {
            BassPhaseConfidence::Trustworthy { .. } => {}
            other => panic!("expected Trustworthy, got {other:?}"),
        }
    }

    #[test]
    fn override_coherence_threshold_via_recording_config() {
        // A curve at γ² = 0.85 fails the default 0.9 but passes a
        // relaxed 0.8 threshold carried on the recording config.
        let c = [healthy_curve(32, 0.85, 85.0, -60.0)];
        let default_verdict = bass_phase_confidence(&c, (20.0, 100.0), None);
        assert_eq!(
            default_verdict,
            BassPhaseConfidence::Degraded {
                reason: "coherence_below_threshold"
            }
        );

        let mut rec = healthy_recording();
        rec.coherence_threshold = Some(0.8);
        match bass_phase_confidence(&c, (20.0, 100.0), Some(&rec)) {
            BassPhaseConfidence::Trustworthy { mean_coherence } => {
                assert!((mean_coherence - 0.85).abs() < 1e-6);
            }
            other => panic!("expected Trustworthy, got {other:?}"),
        }
    }

    #[test]
    fn priority_order_no_phase_beats_low_coherence() {
        // Two curves: one missing phase, the other with low coherence.
        // `"no_phase_data"` is evaluated first so it wins.
        let c0 = {
            let mut c = healthy_curve(16, 0.95, 85.0, -60.0);
            c.phase = None;
            c
        };
        let c1 = healthy_curve(16, 0.5, 85.0, -60.0);
        assert_eq!(
            bass_phase_confidence(&[c0, c1], (20.0, 100.0), None),
            BassPhaseConfidence::Degraded {
                reason: "no_phase_data"
            }
        );
    }

    #[test]
    fn nan_band_lo_returns_invalid_band() {
        let c = [healthy_curve(16, 0.95, 85.0, -60.0)];
        assert_eq!(
            bass_phase_confidence(&c, (f64::NAN, 100.0), None),
            BassPhaseConfidence::Degraded {
                reason: "invalid_band"
            }
        );
    }

    #[test]
    fn nan_band_hi_returns_invalid_band() {
        let c = [healthy_curve(16, 0.95, 85.0, -60.0)];
        assert_eq!(
            bass_phase_confidence(&c, (20.0, f64::NAN), None),
            BassPhaseConfidence::Degraded {
                reason: "invalid_band"
            }
        );
    }

    #[test]
    fn zero_band_width_returns_invalid_band() {
        let c = [healthy_curve(16, 0.95, 85.0, -60.0)];
        assert_eq!(
            bass_phase_confidence(&c, (50.0, 50.0), None),
            BassPhaseConfidence::Degraded {
                reason: "invalid_band"
            }
        );
    }

    #[test]
    fn negative_band_lo_returns_invalid_band() {
        let c = [healthy_curve(16, 0.95, 85.0, -60.0)];
        assert_eq!(
            bass_phase_confidence(&c, (-1.0, 100.0), None),
            BassPhaseConfidence::Degraded {
                reason: "invalid_band"
            }
        );
    }

    #[test]
    fn multiple_curves_one_missing_coherence() {
        let mut c0 = healthy_curve(16, 0.95, 85.0, -60.0);
        c0.coherence = None;
        let c1 = healthy_curve(16, 0.95, 85.0, -60.0);
        assert_eq!(
            bass_phase_confidence(&[c0, c1], (20.0, 100.0), None),
            BassPhaseConfidence::Degraded {
                reason: "no_coherence_data"
            }
        );
    }

    #[test]
    fn mean_coherence_computed_only_in_band() {
        // Build a curve where coherence is high inside the band and low outside
        let freq = log_freqs(32, 20.0, 200.0);
        let spl = Array1::from_elem(32, 85.0);
        let phase = Array1::from_elem(32, 0.0);
        let coh: Vec<f64> = freq
            .iter()
            .map(|&f| if f <= 100.0 { 0.95 } else { 0.5 })
            .collect();
        let noise = Array1::from_elem(32, -60.0);
        let c = Curve {
            freq,
            spl,
            phase: Some(phase),
            coherence: Some(Array1::from(coh)),
            noise_floor_db: Some(noise),
            ..Default::default()
        };
        let rec = healthy_recording();
        match bass_phase_confidence(&[c], (20.0, 100.0), Some(&rec)) {
            BassPhaseConfidence::Trustworthy { mean_coherence } => {
                assert!(
                    (mean_coherence - 0.95).abs() < 1e-6,
                    "mean_coherence should be 0.95 when only in-band bins are counted, got {mean_coherence}"
                );
            }
            other => panic!("expected Trustworthy, got {other:?}"),
        }
    }

    #[test]
    fn malformed_curve_skipped_in_mean_coherence() {
        // freq and coherence have mismatched lengths → should be skipped
        let mut c = healthy_curve(16, 0.95, 85.0, -60.0);
        c.coherence = Some(Array1::from_elem(8, 0.95)); // wrong length
        let rec = healthy_recording();
        // No valid bins → mean coherence = 0.0 → below threshold
        assert_eq!(
            bass_phase_confidence(&[c], (20.0, 100.0), Some(&rec)),
            BassPhaseConfidence::Degraded {
                reason: "coherence_below_threshold"
            }
        );
    }

    #[test]
    fn recording_with_none_coherence_threshold_uses_default() {
        let c = [healthy_curve(16, 0.95, 85.0, -60.0)];
        let rec = RecordingConfiguration {
            num_sweeps: Some(4),
            bass_octave_duration_s: Some(3.0),
            coherence_threshold: None,
            ..Default::default()
        };
        // None coherence_threshold should fall back to DEFAULT_COHERENCE_THRESHOLD (0.9)
        match bass_phase_confidence(&c, (20.0, 100.0), Some(&rec)) {
            BassPhaseConfidence::Trustworthy { mean_coherence } => {
                assert!((mean_coherence - 0.95).abs() < 1e-6);
            }
            other => panic!("expected Trustworthy, got {other:?}"),
        }
    }

    #[test]
    fn recording_with_none_sweeps_uses_default_of_one() {
        // num_sweeps: None → unwrap_or(1) → 1 < 4 → insufficient_bass_duration
        let c = [healthy_curve(16, 0.95, 85.0, -60.0)];
        let rec = RecordingConfiguration {
            num_sweeps: None,
            bass_octave_duration_s: Some(3.0),
            coherence_threshold: None,
            ..Default::default()
        };
        assert_eq!(
            bass_phase_confidence(&c, (20.0, 100.0), Some(&rec)),
            BassPhaseConfidence::Degraded {
                reason: "insufficient_bass_duration"
            }
        );
    }
}
