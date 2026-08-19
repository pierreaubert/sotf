//! Per-take measurement-quality gate and clock-drift handling (R5).
//!
//! After a sweep take passes the capture gates (silence, clipping, cancel —
//! see `record.rs`), this module provides:
//!
//! - a hard **lag-lock gate** ([`check_lag_lock`]): if the cross-correlation
//!   between reference and capture has no confident peak, the take is
//!   refused instead of being analyzed at an arbitrary lag;
//! - **clock-drift decisions** ([`drift_action`]): split DAC/ADC clock setups
//!   (e.g. a USB mic against an interface output) accumulate lag over the
//!   sweep; large drift is corrected via
//!   `math_audio_dsp::analysis::correct_clock_drift`, very large drift
//!   additionally flags the take;
//! - the per-take quality report ([`build_capture_quality`]) carried back to
//!   callers inside [`CaptureAnalysis`].

use crate::signal_analysis::AnalysisResult;
use crate::signal_analysis::ClockDriftEstimate;
use crate::signal_analysis::MeasurementQualityReport;
#[cfg(not(target_os = "ios"))]
use crate::signal_analysis::{LagEstimate, MeasurementQualityConfig};

/// Outcome of one capture + analysis pass (`record_and_analyze`,
/// `record_and_analyze_multi`): the math-dsp analysis plus the engine-side
/// per-take quality verdict and capture diagnostics.
///
/// `AnalysisResult` is a math-dsp type and cannot carry these fields, so
/// they live in this wrapper. `quality` follows the math-dsp
/// `MeasurementQualityConfig::default()` thresholds. With a single-sweep
/// capture (`num_sweeps == 1`) coherence and the SNR metrics are not
/// populated and appear in `quality.missing_metrics` rather than as issues;
/// repeat captures (`num_sweeps > 1`) supply real coherence (when ≥ 4 takes
/// are accepted) and a measured-spectrum/noise-floor pair.
#[derive(Debug)]
pub struct CaptureAnalysis {
    /// Frequency-response / distortion / decay analysis of the capture. For
    /// repeat captures this is the analysis of the drift-corrected,
    /// lag-aligned, synchronously averaged accepted takes (REW-style
    /// pre-deconvolution averaging).
    pub result: AnalysisResult,
    /// Per-take measurement-quality verdict (lag confidence, clipping,
    /// issues). `trustworthy == false` is advisory: the capture still
    /// succeeded; callers decide whether to confirm with the user.
    pub quality: MeasurementQualityReport,
    /// Estimated relative playback/capture clock drift, when the estimation
    /// itself succeeded (regardless of whether it was acted upon). For
    /// repeat captures this is the first ACCEPTED take's estimate (all takes
    /// share the same clock pair; per-take estimates are logged).
    pub drift: Option<ClockDriftEstimate>,
    /// True when the capture was time-rescaled via `correct_clock_drift`
    /// before analysis (and before the WAV on disk was written). For repeat
    /// captures, true when ANY take was corrected.
    pub drift_corrected: bool,
    /// Input samples dropped because the capture ring buffer filled (R6),
    /// accumulated over all takes. For multi-mic captures this is the shared
    /// overrun counter, identical on every mic's report.
    pub dropped_samples: u64,
    /// Takes accepted into the final measurement (Task 8): 1 for a
    /// single-sweep capture, otherwise the number of takes that survived
    /// median/MAD outlier rejection during averaging. This is the truthful
    /// `num_sweeps` value callers should persist into
    /// `autoeq::roomeq::RecordingConfiguration`.
    pub accepted_count: usize,
    /// Takes rejected as median/MAD outliers during repeat-sweep averaging
    /// (always 0 for single-sweep captures).
    pub rejected_count: usize,
}

/// Minimum drift-estimate confidence before drift is acted upon.
#[cfg(not(target_os = "ios"))]
pub(super) const DRIFT_MIN_CONFIDENCE: f32 = 0.2;
/// |ppm| above which the capture is time-rescaled before analysis (drift
/// this large means split DAC/ADC clocks, e.g. a USB mic).
#[cfg(not(target_os = "ios"))]
pub(super) const DRIFT_CORRECT_PPM: f64 = 20.0;
/// |ppm| above which the take additionally gets a quality issue: long
/// sweeps on split-clock setups smear HF phase even after correction.
#[cfg(not(target_os = "ios"))]
pub(super) const DRIFT_SEVERE_PPM: f64 = 100.0;
/// |ppm| above which a drift estimate is physically implausible for an audio
/// clock (even a drifting USB clock stays within a few hundred ppm) and is
/// therefore treated as a garbage estimate: no correction is applied and the
/// take is flagged for review. Defense-in-depth only — the main protection
/// against garbage estimates is the correct-then-verify lock check in
/// `correct_take_clock_drift`.
#[cfg(not(target_os = "ios"))]
pub(super) const DRIFT_IMPLAUSIBLE_PPM: f64 = 2000.0;

/// What to do about a measured clock drift.
#[cfg(not(target_os = "ios"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DriftAction {
    /// No reliable drift estimate, or drift within tolerance.
    None,
    /// Correct the capture before analysis; log the measured ppm.
    Correct,
    /// Correct, and add an advisory quality issue to the take report.
    CorrectAndAdvise,
    /// Estimate is physically implausible (|ppm| > [`DRIFT_IMPLAUSIBLE_PPM`]):
    /// do NOT correct; flag the take for review instead.
    Implausible,
}

/// Trim leading/trailing padding (pre/post silence, fades) from a prepared
/// reference, leaving the active sweep.
///
/// `estimate_clock_drift` windows the *ends* of the reference it is given;
/// the prepared playback reference ends in post-silence + padding, which
/// would put the end window on digital silence and make drift estimation
/// fail outright. Mirrors math-dsp's internal `active_signal_span`
/// threshold (peak × 1e-6); falls back to the full slice when nothing
/// exceeds the threshold.
#[cfg(not(target_os = "ios"))]
pub(super) fn active_reference_span(reference: &[f32]) -> &[f32] {
    let (start, end) = active_span_bounds(reference);
    &reference[start..end]
}

/// Start index of the active (non-padding) content in a prepared reference —
/// i.e. the length of its leading pre-silence/padding. Used to extend the
/// noise-floor window through the reference's own pre-silence (Task 8).
#[cfg(not(target_os = "ios"))]
pub(super) fn active_reference_start(reference: &[f32]) -> usize {
    active_span_bounds(reference).0
}

#[cfg(not(target_os = "ios"))]
fn active_span_bounds(reference: &[f32]) -> (usize, usize) {
    let peak = reference
        .iter()
        .filter(|sample| sample.is_finite())
        .map(|sample| sample.abs())
        .fold(0.0_f32, f32::max);
    if peak <= f32::EPSILON {
        return (0, reference.len());
    }
    let threshold = peak * 1e-6;
    let start = reference
        .iter()
        .position(|sample| sample.abs() > threshold)
        .unwrap_or(0);
    let end = reference
        .iter()
        .rposition(|sample| sample.abs() > threshold)
        .map(|index| index + 1)
        .unwrap_or(reference.len());
    (start, end.max(start))
}

/// Decide whether a clock-drift estimate warrants correction / flagging.
/// Pure threshold logic, unit-tested without hardware. Drift alone never
/// fails a take.
#[cfg(not(target_os = "ios"))]
pub(super) fn drift_action(drift: Option<&ClockDriftEstimate>) -> DriftAction {
    match drift {
        Some(estimate) if estimate.confidence >= DRIFT_MIN_CONFIDENCE => {
            let ppm = estimate.ppm.abs();
            if ppm > DRIFT_IMPLAUSIBLE_PPM {
                DriftAction::Implausible
            } else if ppm > DRIFT_SEVERE_PPM {
                DriftAction::CorrectAndAdvise
            } else if ppm > DRIFT_CORRECT_PPM {
                DriftAction::Correct
            } else {
                DriftAction::None
            }
        }
        _ => DriftAction::None,
    }
}

/// Run `estimate_lag_with_confidence`, mapping estimation failure to the
/// same actionable advice as the confidence gate.
#[cfg(not(target_os = "ios"))]
pub(super) fn estimate_lag_or_advise(
    reference: &[f32],
    recorded: &[f32],
    log_tag: &str,
) -> Result<LagEstimate, String> {
    crate::signal_analysis::estimate_lag_with_confidence(reference, recorded).map_err(|e| {
        format!(
            "[{log_tag}] No reliable signal lock — check mic connection, \
             input channel, and playback level; background noise may be too high \
             (lag estimation failed: {e})"
        )
    })
}

/// Hard gate: refuse takes whose reference↔capture cross-correlation has no
/// confident peak. `analyze_recording` computes the same lag internally but
/// only exposes `estimated_lag_samples` (no confidence) in `AnalysisResult`,
/// so the engine runs `estimate_lag_with_confidence` itself — one extra
/// FFT-based cross-correlation per take, negligible against the multi-second
/// capture.
#[cfg(not(target_os = "ios"))]
pub(super) fn check_lag_lock(lag: &LagEstimate, log_tag: &str) -> Result<(), String> {
    let minimum = MeasurementQualityConfig::default().minimum_lag_confidence;
    if lag.confidence < minimum {
        return Err(format!(
            "[{log_tag}] No reliable signal lock (lag confidence {:.3} < {:.3}) — \
             check mic connection, input channel, and playback level; \
             background noise may be too high",
            lag.confidence, minimum,
        ));
    }
    Ok(())
}

/// Build the per-take quality report.
///
/// Single-sweep captures pass `None` for all three spectral inputs (see
/// below), keeping the Task-7 semantics. Repeat captures supply the real
/// coherence from the robust average (only when non-empty — math-dsp treats
/// `Some([])` as the issue "coherence data was supplied but empty") and the
/// measured-spectrum / noise-floor pair on the shared deconvolution FFT
/// grid, making the report complete.
///
/// The SNR inputs are deliberately a pair: passing a noise floor without a
/// measured spectrum on the same grid (or vice versa, or with mismatched
/// lengths) makes math-dsp flag it as an issue, which would mark every such
/// take untrustworthy. A half-supplied or mismatched pair is therefore
/// dropped here (with a warning) instead of poisoning the verdict. With the
/// default config (`require_snr: false`, `require_coherence: false`) the
/// missing metrics are reported via `missing_metrics`, not as issues.
///
/// `extra_issues` (e.g. the severe-drift advisory) are appended and force
/// `trustworthy = false`, keeping the flag consistent with a non-empty
/// issue list.
#[cfg(not(target_os = "ios"))]
pub(super) fn build_capture_quality(
    recorded: &[f32],
    lag: &LagEstimate,
    coherence: Option<&[f32]>,
    measured_spectrum_db: Option<&[f32]>,
    noise_floor_db: Option<&[f32]>,
    extra_issues: Vec<String>,
) -> MeasurementQualityReport {
    let coherence = coherence.filter(|values| !values.is_empty());
    let (measured_spectrum_db, noise_floor_db) = match (measured_spectrum_db, noise_floor_db) {
        (Some(measured), Some(noise))
            if !measured.is_empty() && measured.len() == noise.len() =>
        {
            (Some(measured), Some(noise))
        }
        (None, None) => (None, None),
        _ => {
            log::warn!(
                "[build_capture_quality] measured spectrum and noise floor must be supplied as a matched, equal-length pair — dropping both from the quality report"
            );
            (None, None)
        }
    };
    let mut report = crate::signal_analysis::assess_measurement_quality(
        recorded,
        lag,
        coherence,
        measured_spectrum_db,
        noise_floor_db,
        MeasurementQualityConfig::default(),
    );
    if !extra_issues.is_empty() {
        report.issues.extend(extra_issues);
        report.trustworthy = false;
    }
    report
}

/// Log the quality verdict for a take. Untrustworthy takes do not fail the
/// capture (the silence/clip/lag gates already hard-fail); they surface as a
/// warning until the UI confirmation flow lands (Task 9).
#[cfg(not(target_os = "ios"))]
pub(super) fn log_capture_quality(report: &MeasurementQualityReport, log_tag: &str) {
    if report.trustworthy {
        log::info!(
            "[{log_tag}] Take quality: trustworthy (score {:.2}, lag confidence {:.3})",
            report.score,
            report.lag_confidence,
        );
    } else {
        log::warn!(
            "[{log_tag}] Take quality: NOT trustworthy (score {:.2}): {}",
            report.score,
            report.issues.join("; "),
        );
    }
}
