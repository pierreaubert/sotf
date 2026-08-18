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
/// `MeasurementQualityConfig::default()` thresholds; `coherence` and the
/// SNR metrics are not populated yet (repeat-sweep averaging is Task 8), so
/// they appear in `quality.missing_metrics` rather than as issues.
#[derive(Debug)]
pub struct CaptureAnalysis {
    /// Frequency-response / distortion / decay analysis of the capture.
    pub result: AnalysisResult,
    /// Per-take measurement-quality verdict (lag confidence, clipping,
    /// issues). `trustworthy == false` is advisory: the capture still
    /// succeeded; callers decide whether to confirm with the user.
    pub quality: MeasurementQualityReport,
    /// Estimated relative playback/capture clock drift, when the estimation
    /// itself succeeded (regardless of whether it was acted upon).
    pub drift: Option<ClockDriftEstimate>,
    /// True when the capture was time-rescaled via `correct_clock_drift`
    /// before analysis (and before the WAV on disk was written).
    pub drift_corrected: bool,
    /// Input samples dropped because the capture ring buffer filled (R6).
    /// For multi-mic captures this is the shared overrun counter, identical
    /// on every mic's report.
    pub dropped_samples: u64,
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
    let peak = reference
        .iter()
        .filter(|sample| sample.is_finite())
        .map(|sample| sample.abs())
        .fold(0.0_f32, f32::max);
    if peak <= f32::EPSILON {
        return reference;
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
    &reference[start..end.max(start)]
}

/// Normalize the ppm scale of a math-dsp `ClockDriftEstimate`.
///
/// math-dsp 0.5.23 (pin 1c79399) computes `ppm = lag_change /
/// elapsed_seconds * 1e6` — i.e. true ppm multiplied by the sample rate —
/// while `correct_clock_drift` and the thresholds above expect true ppm
/// (`lag_change / elapsed_samples * 1e6`). This helper rescales the estimate
/// to true ppm so both sides agree. Canary:
/// `test_clock_drift_estimate_and_correct_roundtrip` in `tests.rs` injects a
/// known drift; if a future math-audio pin fixes the upstream formula, that
/// test fails and this normalization must be removed.
#[cfg(not(target_os = "ios"))]
pub(super) fn normalize_clock_drift_ppm(
    raw: ClockDriftEstimate,
    sample_rate: u32,
) -> ClockDriftEstimate {
    debug_assert!(sample_rate > 0);
    ClockDriftEstimate {
        ppm: raw.ppm / sample_rate as f64,
        ..raw
    }
}

/// Decide whether a clock-drift estimate warrants correction / flagging.
/// Pure threshold logic, unit-tested without hardware. Drift alone never
/// fails a take.
#[cfg(not(target_os = "ios"))]
pub(super) fn drift_action(drift: Option<&ClockDriftEstimate>) -> DriftAction {
    match drift {
        Some(estimate) if estimate.confidence >= DRIFT_MIN_CONFIDENCE => {
            let ppm = estimate.ppm.abs();
            if ppm > DRIFT_SEVERE_PPM {
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
/// Coherence is `None` until repeat-sweep averaging lands (Task 8). The
/// measured spectrum on the deconvolution grid is not exposed by
/// `analyze_recording` (its `AnalysisResult.spl_db` is log-interpolated onto
/// a different grid), so SNR inputs are `None` too — deliberately through
/// `assess_measurement_quality` rather than `…_from_silence`: passing a
/// silence-derived noise floor without a matching measured spectrum makes
/// math-dsp flag "noise floor supplied without a measured spectrum" as an
/// issue, which would mark every take untrustworthy. With the default config
/// (`require_snr: false`, `require_coherence: false`) the missing metrics are
/// reported via `missing_metrics`, not as issues.
///
/// `extra_issues` (e.g. the severe-drift advisory) are appended and force
/// `trustworthy = false`, keeping the flag consistent with a non-empty
/// issue list.
#[cfg(not(target_os = "ios"))]
pub(super) fn build_capture_quality(
    recorded: &[f32],
    lag: &LagEstimate,
    extra_issues: Vec<String>,
) -> MeasurementQualityReport {
    let mut report = crate::signal_analysis::assess_measurement_quality(
        recorded,
        lag,
        None,
        None,
        None,
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
