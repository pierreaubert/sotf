//! Shared recording-session helpers for all player frontends (TUI, GPUI, CLI).
//!
//! Consolidates the per-frontend recording logic flagged in
//! `reviews/20260818-recording.md` (B1, B3, B4, B5, C1) so every shell maps
//! signal types, converts levels, sanitizes file names, resolves mic
//! calibration, and builds capture parameters identically. The
//! `RecordingConfiguration` builder lives on
//! [`crate::ui_models::recording::RecordingScreenModel::build_recording_configuration`]
//! next to the other save-time helpers.

use crate::recording_types::{
    ChannelRecording, ChannelRecordingState, RecordingResult, RecordingSignalType,
    TakeQualitySummary,
};
use sotf_audio::signal_recorder::{
    CaptureAnalysis, DEFAULT_MLS_ORDER, MIN_REPEAT_SWEEPS, SignalParams, SignalType,
    sweep_params_from_config,
};

/// Canonical filename for the saved recording session JSON (B5).
///
/// Room EQ's "FromFile" flow and the i18n strings point users at this name,
/// so every frontend must save the session as `<dir>/recordings.json`
/// regardless of the user-chosen session name.
pub const RECORDINGS_FILENAME: &str = "recordings.json";

/// Shared default sweep start frequency (Hz) for all frontends (C2).
pub const DEFAULT_SWEEP_START_FREQ: f32 = 20.0;

/// Shared default sweep end frequency (Hz) for all frontends (C2).
pub const DEFAULT_SWEEP_END_FREQ: f32 = 20000.0;

/// Map the wizard's [`RecordingSignalType`] to the engine's [`SignalType`].
///
/// `DelayProbe` has no per-channel generator — it uses the separate
/// multi-channel `probe_channel_delays` workflow — so it falls back to
/// `Sweep` here, matching what both frontends did inline.
pub fn signal_type_for(recording_signal_type: RecordingSignalType) -> SignalType {
    match recording_signal_type {
        RecordingSignalType::Sweep => SignalType::Sweep,
        RecordingSignalType::WhiteNoise => SignalType::WhiteNoise,
        RecordingSignalType::PinkNoise => SignalType::PinkNoise,
        RecordingSignalType::Mls => SignalType::Mls,
        RecordingSignalType::Dirac => SignalType::Dirac,
        RecordingSignalType::DelayProbe => {
            log::warn!(
                "DelayProbe selected in per-channel mode; use probe_channel_delays() instead. Falling back to Sweep."
            );
            SignalType::Sweep
        }
    }
}

/// Convert a signal level in dBFS to a linear amplitude, clamped to
/// `[-40, 0] dB` so the result never exceeds full scale (≤ 0 dBFS).
///
/// Mirrors the engine's `measurement_amplitude_from_level_db` clamp added
/// with `prepare_measurement_signal`; frontends previously used an
/// unclamped `10^(level_db / 20)`.
pub fn measurement_amplitude(level_db: f64) -> f32 {
    10.0_f64.powf(level_db.clamp(-40.0, 0.0) / 20.0) as f32
}

/// Filesystem-safe file-name stem for a recording session or channel name.
///
/// Keeps alphanumerics, `_` and `-`; everything else (including path
/// separators) becomes `_`. An empty or all-unsafe input falls back to
/// `"recording"` so the result is always a usable file name.
pub fn sanitize_recording_name(name: &str) -> String {
    let safe: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();

    if safe.is_empty() {
        "recording".to_string()
    } else {
        safe
    }
}

/// Resolve the mic-calibration file for a capture: the per-mic entry at
/// `mic_index` wins, falling back to the session-global `global` path (B3).
///
/// `paths` is parallel to the recording channel mappings (indexed by mic
/// slot, **not** by hardware input channel). Empty strings are treated as
/// "no calibration". Returns the selected path, or `None` when neither the
/// per-mic slot nor the global path is set.
pub fn resolve_mic_calibration(
    paths: &[Option<String>],
    mic_index: usize,
    global: Option<&str>,
) -> Option<String> {
    paths
        .get(mic_index)
        .and_then(|p| p.clone())
        .filter(|s| !s.is_empty())
        .or_else(|| global.filter(|s| !s.is_empty()).map(str::to_string))
}

/// Transfer-function average level (dB rel. unity) below which a completed
/// capture is flagged as suspiciously low (R10). Shared by all frontends.
pub const LOW_MEASURED_LEVEL_THRESHOLD_DB: f32 = -50.0;

/// Post-capture sanity check (R10): average the measured transfer-function
/// level (dB rel. unity) over the channel's own sweep band and return the
/// average when it falls below [`LOW_MEASURED_LEVEL_THRESHOLD_DB`].
///
/// The band is clamped to [20 Hz, 20 kHz]; bins at or below −150 dB are
/// treated as "no data" and ignored. The per-channel sweep band (not a
/// global band) is used so LFE channels with a narrow low-frequency sweep
/// are judged on their actual stimulus range. Returns `None` when the level
/// is healthy or when no usable bin falls inside the band.
///
/// Note this measures the *average measured level*, not a true acoustic
/// noise floor — phrase user-facing text accordingly (see
/// [`low_measured_level_warning`]).
pub fn check_low_measured_level(
    frequencies: &[f32],
    level_db: &[f32],
    sweep_start_freq: f32,
    sweep_end_freq: f32,
) -> Option<f32> {
    let band_min = sweep_start_freq.max(20.0);
    let band_max = sweep_end_freq.min(20000.0);
    let mut sum = 0.0_f32;
    let mut count = 0usize;
    for (&freq, &mag) in frequencies.iter().zip(level_db.iter()) {
        if freq >= band_min && freq <= band_max && mag > -150.0 {
            sum += mag;
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    let avg = sum / count as f32;
    (avg < LOW_MEASURED_LEVEL_THRESHOLD_DB).then_some(avg)
}

/// Canonical user-facing text for a [`check_low_measured_level`] hit,
/// worded honestly (low *measured level*, pointing at mic connection and
/// input gain — not a "noise floor"). `subjects` names the affected
/// channel(s) or speaker.
pub fn low_measured_level_warning(subjects: &str) -> String {
    format!("Very low measured level on {subjects} — check mic connection and input gain")
}

/// Build the engine [`SignalParams`] for a per-channel capture (B1).
///
/// The sweep path routes through the engine's [`sweep_params_from_config`]
/// so the UI "Bass precision" knob (`bass_octave_duration_s`) and the
/// silence windows actually shape the generated stimulus (octave-scaled
/// sweep with more bass dwell) instead of only changing persisted metadata.
/// Pass `None` for `bass_octave_duration_s` to keep the legacy plain
/// fixed-duration log sweep.
///
/// `amp` should come from [`measurement_amplitude`] so the stimulus never
/// exceeds full scale. Noise/MLS/Dirac ignore the sweep-specific arguments.
pub fn capture_signal_params(
    signal_type: SignalType,
    sweep_start_freq: f32,
    sweep_end_freq: f32,
    amp: f32,
    bass_octave_duration_s: Option<f32>,
    pre_silence_s: Option<f32>,
    post_silence_s: Option<f32>,
) -> SignalParams {
    match signal_type {
        SignalType::Sweep => sweep_params_from_config(
            sweep_start_freq,
            sweep_end_freq,
            amp,
            bass_octave_duration_s,
            pre_silence_s,
            post_silence_s,
        ),
        SignalType::WhiteNoise | SignalType::PinkNoise | SignalType::MNoise => {
            SignalParams::Noise { amp }
        }
        SignalType::Mls => SignalParams::Mls {
            order: DEFAULT_MLS_ORDER,
            amp,
        },
        SignalType::Dirac => SignalParams::Dirac { amp },
        // Matches the frontends' historical catch-all: unexpected signal
        // types degrade to a sweep rather than failing the capture.
        _ => sweep_params_from_config(
            sweep_start_freq,
            sweep_end_freq,
            amp,
            bass_octave_duration_s,
            pre_silence_s,
            post_silence_s,
        ),
    }
}

// ============================================================================
// Per-take quality gate (Task 9, review §4 item 1)
// ============================================================================
//
// Verdict logic (thresholds, wording) lives here so the TUI and GPUI shells
// only render text and collect the user's accept/re-record choice. The
// engine's verdict is advisory: the capture succeeded, so the take is parked
// in `ChannelRecordingState::ReviewNeeded` until the user decides.

/// Highest sweep count the UIs offer for `num_sweeps`.
pub const MAX_NUM_SWEEPS: u16 = 8;

/// Clamp a user-entered sweep count to the UI range: 1 (single sweep) or
/// `MIN_REPEAT_SWEEPS..=MAX_NUM_SWEEPS`. **2 is never returned**: the engine
/// bumps it to `MIN_REPEAT_SWEEPS` anyway because two takes have zero
/// outlier-rejection power (commit 43d677d27), so offering it would lie
/// about what runs.
pub fn clamp_num_sweeps(value: u16) -> u16 {
    match value.clamp(1, MAX_NUM_SWEEPS) {
        2 => MIN_REPEAT_SWEEPS,
        v => v,
    }
}

/// Step the sweep count by `delta` for +/- adjusters, skipping 2 in both
/// directions (see [`clamp_num_sweeps`]).
pub fn nudge_num_sweeps(current: u16, delta: i32) -> u16 {
    let next = current as i32 + delta;
    if next <= 1 {
        1
    } else if next == 2 {
        if delta > 0 { MIN_REPEAT_SWEEPS } else { 1 }
    } else {
        (next).min(MAX_NUM_SWEEPS as i32) as u16
    }
}

/// Extract the UI-facing quality summary from an engine [`CaptureAnalysis`]
/// (Task 7/8 wrapper). Pure field mapping — no decisions here.
///
/// `drift_ppm` stays an `Option`: `None` means the drift *estimation* was
/// unavailable (low-confidence end windows), which must never be rendered
/// as "0 ppm" (Task-7 review carry-forward).
pub fn summarize_take_quality(capture: &CaptureAnalysis) -> TakeQualitySummary {
    TakeQualitySummary {
        trustworthy: capture.quality.trustworthy,
        score: capture.quality.score,
        issues: capture.quality.issues.clone(),
        mean_coherence: capture.quality.mean_coherence,
        median_snr_db: capture.quality.median_snr_db,
        clip_fraction: capture.quality.clipping.fraction,
        drift_ppm: capture.drift.map(|d| d.ppm),
        drift_corrected: capture.drift_corrected,
        dropped_samples: capture.dropped_samples,
        accepted_count: capture.accepted_count,
        rejected_count: capture.rejected_count,
    }
}

/// One-line detailed verdict for a captured take — the issues plus every
/// available supporting metric (mean coherence, median SNR, clip fraction,
/// drift ppm, accepted/rejected takes). Shown when a take needs review and
/// in per-channel details.
///
/// Missing drift is rendered as `drift n/a`, never `0 ppm`.
pub fn take_verdict_text(q: &TakeQualitySummary) -> String {
    let mut parts: Vec<String> = Vec::new();
    if q.issues.is_empty() {
        parts.push(format!("score {:.2}", q.score));
    } else {
        parts.push(format!("score {:.2} — {}", q.score, q.issues.join("; ")));
    }
    if let Some(coherence) = q.mean_coherence {
        parts.push(format!("coherence {coherence:.2}"));
    }
    if let Some(snr) = q.median_snr_db {
        parts.push(format!("SNR {snr:.1} dB"));
    }
    if q.clip_fraction > 0.0 {
        parts.push(format!("clipped {:.2}%", q.clip_fraction * 100.0));
    }
    match q.drift_ppm {
        Some(ppm) => parts.push(format!(
            "drift {ppm:+.0} ppm{}",
            if q.drift_corrected { " (corrected)" } else { "" }
        )),
        None => parts.push("drift n/a".to_string()),
    }
    if q.rejected_count > 0 {
        parts.push(format!(
            "{}/{} takes accepted",
            q.accepted_count,
            q.accepted_count + q.rejected_count
        ));
    }
    parts.join(", ")
}

/// Compact per-channel quality cell for channel-list tables.
///
/// - `REVIEW <score>` — take is parked awaiting the user's decision;
/// - `OK* <score>` — accepted despite quality warnings (distinct from clean);
/// - `OK <score>` — clean take;
/// - empty string when there is nothing to say yet.
pub fn take_quality_cell(
    state: ChannelRecordingState,
    result: Option<&RecordingResult>,
) -> String {
    let quality = result.and_then(|r| r.quality.as_ref());
    match (state, quality) {
        (ChannelRecordingState::ReviewNeeded, Some(q)) => format!("REVIEW {:.2}", q.score),
        // Accepted-with-warning: deliberately distinct from a clean Done.
        (ChannelRecordingState::Done, Some(q)) if !q.trustworthy => {
            format!("OK* {:.2}", q.score)
        }
        (ChannelRecordingState::Done, Some(q)) => format!("OK {:.2}", q.score),
        // Done/ReviewNeeded without a quality summary means a legacy or
        // loaded session — say so rather than implying a verdict.
        (ChannelRecordingState::Done | ChannelRecordingState::ReviewNeeded, None) => {
            "no data".to_string()
        }
        _ => String::new(),
    }
}

/// Dropout warning (R6 / §4 item 1): `Some` text when the capture dropped
/// input samples to ring-buffer overruns.
pub fn dropout_warning(dropped_samples: u64) -> Option<String> {
    (dropped_samples > 0).then(|| {
        format!(
            "{dropped_samples} samples dropped during capture — USB/driver underrun; \
             results may be unreliable, consider re-measuring"
        )
    })
}

/// Multi-position guidance (Phase 3 item 10): one-line hint for the capture
/// flow when `num_positions > 1`. `pos` is the 0-based position about to be
/// recorded. The first position anchors delays/levels, so it must be the
/// main listening position; later positions should stay within roughly 60 cm
/// of it (Audyssey/Dirac guidance).
pub fn position_guidance(pos: usize, total: usize) -> String {
    if pos == 0 {
        format!(
            "Position 1 of {total}: place the mic(s) at the main listening position first"
        )
    } else {
        format!(
            "Move the mic(s) to position {} of {total} — stay within ~60 cm of the main listening position",
            pos + 1
        )
    }
}

/// Session quality summary (§4 item 1 / Phase 2 item 5): one line per
/// recorded channel with its score and warnings, so the user can see which
/// positions to re-measure before leaving the screen. Channels are listed in
/// recording order (position-major), which matches the on-screen channel
/// list.
pub fn session_quality_summary(channels: &[ChannelRecording]) -> Vec<String> {
    channels
        .iter()
        .filter(|c| c.result.is_some())
        .map(|c| {
            let quality = c.result.as_ref().and_then(|r| r.quality.as_ref());
            let mut line = match quality {
                Some(q) if q.trustworthy => format!(
                    "{}: OK (score {:.2}, {}/{} sweeps)",
                    c.channel_name,
                    q.score,
                    q.accepted_count,
                    q.accepted_count + q.rejected_count
                ),
                Some(q) => format!(
                    "{}: REVIEW (score {:.2}) — {}",
                    c.channel_name,
                    q.score,
                    take_verdict_text(q)
                ),
                None => format!("{}: recorded (no quality data)", c.channel_name),
            };
            if let Some(q) = quality
                && q.dropped_samples > 0
            {
                line.push_str(&format!("; {} dropped samples", q.dropped_samples));
            }
            line
        })
        .collect()
}

/// Truthful `num_sweeps` to persist in `RecordingConfiguration` (Task 8
/// metadata rule): the **minimum engine-reported accepted-take count** over
/// completed channels (one bad channel must not be covered up by the
/// requested count). `None` when no completed channel carries quality data
/// (legacy / loaded sessions), preserving the old "unknown" semantics
/// instead of persisting the requested count as if it were measured.
pub fn accepted_num_sweeps_for_save(channels: &[ChannelRecording]) -> Option<u8> {
    let mut min: Option<usize> = None;
    for c in channels {
        if c.state != ChannelRecordingState::Done {
            continue;
        }
        if let Some(q) = c.result.as_ref().and_then(|r| r.quality.as_ref()) {
            min = Some(min.map_or(q.accepted_count, |m: usize| m.min(q.accepted_count)));
        }
    }
    min.map(|m| m.min(u8::MAX as usize) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_type_for_maps_each_variant() {
        assert_eq!(
            signal_type_for(RecordingSignalType::Sweep),
            SignalType::Sweep
        );
        assert_eq!(
            signal_type_for(RecordingSignalType::WhiteNoise),
            SignalType::WhiteNoise
        );
        assert_eq!(
            signal_type_for(RecordingSignalType::PinkNoise),
            SignalType::PinkNoise
        );
        assert_eq!(signal_type_for(RecordingSignalType::Mls), SignalType::Mls);
        assert_eq!(
            signal_type_for(RecordingSignalType::Dirac),
            SignalType::Dirac
        );
    }

    #[test]
    fn signal_type_for_delay_probe_falls_back_to_sweep() {
        assert_eq!(
            signal_type_for(RecordingSignalType::DelayProbe),
            SignalType::Sweep
        );
    }

    #[test]
    fn measurement_amplitude_converts_db() {
        assert!((measurement_amplitude(-6.0206) - 0.5).abs() < 1e-4);
        assert_eq!(measurement_amplitude(0.0), 1.0);
        assert!((measurement_amplitude(-40.0) - 0.01).abs() < 1e-6);
    }

    #[test]
    fn measurement_amplitude_clamps_to_full_scale() {
        // Positive dB must never exceed 0 dBFS.
        assert_eq!(measurement_amplitude(6.0), 1.0);
        assert_eq!(measurement_amplitude(20.0), 1.0);
        // Below the floor stays at the floor.
        assert_eq!(measurement_amplitude(-80.0), measurement_amplitude(-40.0));
    }

    #[test]
    fn sanitize_recording_name_replaces_unsafe_chars() {
        assert_eq!(sanitize_recording_name("My Recording!"), "My_Recording_");
        assert_eq!(sanitize_recording_name("a/b\\c:d"), "a_b_c_d");
        assert_eq!(sanitize_recording_name("LFE-1_ok"), "LFE-1_ok");
        assert_eq!(sanitize_recording_name("  padded  "), "padded");
        // Unicode alphanumerics are kept.
        assert_eq!(sanitize_recording_name("café"), "café");
    }

    #[test]
    fn sanitize_recording_name_empty_falls_back() {
        assert_eq!(sanitize_recording_name(""), "recording");
        assert_eq!(sanitize_recording_name("   "), "recording");
        assert_eq!(sanitize_recording_name("!!!"), "___");
    }

    #[test]
    fn resolve_mic_calibration_prefers_per_mic_slot() {
        let paths = vec![Some("mic0.txt".to_string()), Some("mic1.txt".to_string())];
        assert_eq!(
            resolve_mic_calibration(&paths, 1, Some("global.txt")),
            Some("mic1.txt".to_string())
        );
    }

    #[test]
    fn resolve_mic_calibration_falls_back_to_global() {
        // Missing slot.
        let paths = vec![None, Some("mic1.txt".to_string())];
        assert_eq!(
            resolve_mic_calibration(&paths, 0, Some("global.txt")),
            Some("global.txt".to_string())
        );
        // Out-of-range index.
        assert_eq!(
            resolve_mic_calibration(&paths, 7, Some("global.txt")),
            Some("global.txt".to_string())
        );
        // Empty vec.
        assert_eq!(
            resolve_mic_calibration(&[], 0, Some("global.txt")),
            Some("global.txt".to_string())
        );
    }

    #[test]
    fn resolve_mic_calibration_treats_empty_strings_as_unset() {
        let paths = vec![Some(String::new())];
        assert_eq!(
            resolve_mic_calibration(&paths, 0, Some("global.txt")),
            Some("global.txt".to_string())
        );
        // Empty global is also filtered out.
        assert_eq!(resolve_mic_calibration(&paths, 0, Some("")), None);
        assert_eq!(resolve_mic_calibration(&[], 0, None), None);
    }

    #[test]
    fn check_low_measured_level_flags_low_band_average() {
        // Flat -80 dB across the band → flagged, average returned.
        let freqs: Vec<f32> = (0..100).map(|i| 20.0 + i as f32 * 200.0).collect();
        let levels = vec![-80.0; 100];
        let avg = check_low_measured_level(&freqs, &levels, 20.0, 20000.0)
            .expect("low band average should be flagged");
        assert!((avg - -80.0).abs() < 1e-4);
    }

    #[test]
    fn check_low_measured_level_passes_healthy_level() {
        let freqs: Vec<f32> = (0..100).map(|i| 20.0 + i as f32 * 200.0).collect();
        let levels = vec![-10.0; 100];
        assert!(check_low_measured_level(&freqs, &levels, 20.0, 20000.0).is_none());
        // Exactly at the threshold is not "below".
        let levels = vec![LOW_MEASURED_LEVEL_THRESHOLD_DB; 100];
        assert!(check_low_measured_level(&freqs, &levels, 20.0, 20000.0).is_none());
    }

    #[test]
    fn check_low_measured_level_restricts_to_sweep_band() {
        // Low level outside the sweep band must not trip the check (the
        // per-channel band semantics matter for LFE channels).
        let freqs = vec![5.0, 100.0, 1000.0];
        let levels = vec![-90.0, -10.0, -10.0];
        assert!(check_low_measured_level(&freqs, &levels, 20.0, 500.0).is_none());
        // Low level inside a narrow LFE band is caught.
        let levels = vec![-90.0, -80.0, -10.0];
        assert!(check_low_measured_level(&freqs, &levels, 20.0, 500.0).is_some());
    }

    #[test]
    fn check_low_measured_level_ignores_no_data_bins() {
        // Bins at or below -150 dB carry no information and are skipped.
        let freqs = vec![100.0, 200.0, 300.0];
        let levels = vec![-150.0, -200.0, -10.0];
        assert!(check_low_measured_level(&freqs, &levels, 20.0, 20000.0).is_none());
        // All bins unusable → no verdict rather than a false alarm.
        let levels = vec![-150.0, -200.0, -160.0];
        assert!(check_low_measured_level(&freqs, &levels, 20.0, 20000.0).is_none());
        assert!(check_low_measured_level(&[], &[], 20.0, 20000.0).is_none());
    }

    #[test]
    fn low_measured_level_warning_names_subjects() {
        let msg = low_measured_level_warning("FL, FR");
        assert!(msg.contains("Very low measured level"));
        assert!(msg.contains("FL, FR"));
        assert!(msg.contains("mic connection"));
    }

    #[test]
    fn capture_signal_params_sweep_uses_octave_sweep_when_configured() {
        let params = capture_signal_params(
            SignalType::Sweep,
            20.0,
            20000.0,
            0.5,
            Some(3.0),
            Some(2.0),
            None,
        );
        match params {
            SignalParams::OctaveSweep {
                start_freq,
                end_freq,
                amp,
                bass_octave_duration_s,
                pre_silence_s,
                post_silence_s,
            } => {
                assert_eq!(start_freq, 20.0);
                assert_eq!(end_freq, 20000.0);
                assert_eq!(amp, 0.5);
                assert_eq!(bass_octave_duration_s, 3.0);
                assert_eq!(pre_silence_s, 2.0);
                // Engine default applies when the UI leaves it unset.
                assert_eq!(post_silence_s, 2.0);
            }
            other => panic!("expected OctaveSweep, got {:?}", other),
        }
    }

    #[test]
    fn capture_signal_params_sweep_without_bass_duration_stays_legacy() {
        let params = capture_signal_params(SignalType::Sweep, 20.0, 20000.0, 0.5, None, None, None);
        assert!(matches!(params, SignalParams::Sweep { .. }));
    }

    #[test]
    fn capture_signal_params_other_types_ignore_sweep_args() {
        assert!(matches!(
            capture_signal_params(
                SignalType::PinkNoise,
                20.0,
                20000.0,
                0.25,
                Some(3.0),
                Some(2.0),
                None
            ),
            SignalParams::Noise { amp } if amp == 0.25
        ));
        assert!(matches!(
            capture_signal_params(SignalType::Mls, 20.0, 20000.0, 0.25, None, None, None),
            SignalParams::Mls {
                order: DEFAULT_MLS_ORDER,
                amp
            } if amp == 0.25
        ));
        assert!(matches!(
            capture_signal_params(SignalType::Dirac, 20.0, 20000.0, 0.25, None, None, None),
            SignalParams::Dirac { amp } if amp == 0.25
        ));
    }

    // === Task 9: num_sweeps range, quality verdict text, session summary ===

    #[test]
    fn clamp_num_sweeps_never_returns_two() {
        assert_eq!(clamp_num_sweeps(0), 1);
        assert_eq!(clamp_num_sweeps(1), 1);
        // 2 has zero outlier-rejection power; the engine bumps it to 3.
        assert_eq!(clamp_num_sweeps(2), MIN_REPEAT_SWEEPS);
        assert_eq!(clamp_num_sweeps(3), 3);
        assert_eq!(clamp_num_sweeps(8), 8);
        assert_eq!(clamp_num_sweeps(9), MAX_NUM_SWEEPS);
        assert_eq!(clamp_num_sweeps(100), MAX_NUM_SWEEPS);
    }

    #[test]
    fn nudge_num_sweeps_skips_two_in_both_directions() {
        assert_eq!(nudge_num_sweeps(1, 1), 3);
        assert_eq!(nudge_num_sweeps(1, -1), 1);
        assert_eq!(nudge_num_sweeps(3, -1), 1);
        assert_eq!(nudge_num_sweeps(3, 1), 4);
        assert_eq!(nudge_num_sweeps(8, 1), 8);
        assert_eq!(nudge_num_sweeps(4, -1), 3);
    }

    fn quality_summary(
        trustworthy: bool,
        issues: &[&str],
        drift_ppm: Option<f64>,
    ) -> TakeQualitySummary {
        TakeQualitySummary {
            trustworthy,
            score: 0.42,
            issues: issues.iter().map(|s| s.to_string()).collect(),
            mean_coherence: Some(0.71),
            median_snr_db: Some(18.2),
            clip_fraction: 0.003,
            drift_ppm,
            drift_corrected: false,
            dropped_samples: 0,
            accepted_count: 4,
            rejected_count: 1,
        }
    }

    #[test]
    fn take_verdict_text_lists_issues_and_metrics() {
        let q = quality_summary(false, &["clipping detected"], Some(-45.0));
        let text = take_verdict_text(&q);
        assert!(text.contains("score 0.42"), "{text}");
        assert!(text.contains("clipping detected"), "{text}");
        assert!(text.contains("coherence 0.71"), "{text}");
        assert!(text.contains("SNR 18.2 dB"), "{text}");
        assert!(text.contains("clipped 0.30%"), "{text}");
        assert!(text.contains("drift -45 ppm"), "{text}");
        assert!(text.contains("4/5 takes accepted"), "{text}");
    }

    #[test]
    fn take_verdict_text_distinguishes_drift_unavailable_from_zero() {
        // Task-7 review carry-forward: drift None must render as unavailable,
        // never as "0 ppm".
        let none = take_verdict_text(&quality_summary(true, &[], None));
        assert!(none.contains("drift n/a"), "{none}");
        assert!(!none.contains("0 ppm"), "{none}");

        let zero = take_verdict_text(&quality_summary(true, &[], Some(0.0)));
        assert!(zero.contains("drift +0 ppm"), "{zero}");
        assert!(!zero.contains("n/a"), "{zero}");
    }

    #[test]
    fn take_verdict_text_marks_drift_correction() {
        let mut q = quality_summary(true, &[], Some(30.0));
        q.drift_corrected = true;
        assert!(take_verdict_text(&q).contains("drift +30 ppm (corrected)"));
    }

    #[test]
    fn take_quality_cell_marks_review_accepted_and_clean() {
        use crate::recording_types::ChannelRecordingState::*;
        let warn = quality_summary(false, &["low coherence"], None);
        let clean = quality_summary(true, &[], Some(5.0));
        let mk = |q: TakeQualitySummary| RecordingResult {
            channel: 0,
            wav_path: None,
            csv_path: None,
            frequencies: vec![],
            magnitude_db: vec![],
            phase_deg: vec![],
            impulse_response: None,
            impulse_time_ms: None,
            thd_percent: None,
            harmonic_distortion_db: None,
            excess_group_delay_ms: None,
            rt60_ms: None,
            clarity_c50_db: None,
            clarity_c80_db: None,
            spectrogram_db: None,
            quality: Some(q),
        };
        let warned = mk(warn);
        let clean_res = mk(clean);
        assert_eq!(
            take_quality_cell(ReviewNeeded, Some(&warned)),
            "REVIEW 0.42"
        );
        // Accepted-with-warning stays distinct from a clean Done.
        assert_eq!(take_quality_cell(Done, Some(&warned)), "OK* 0.42");
        assert_eq!(take_quality_cell(Done, Some(&clean_res)), "OK 0.42");
        // Legacy / loaded results without quality data say so explicitly.
        assert_eq!(take_quality_cell(Done, None), "no data");
        assert!(take_quality_cell(Empty, None).is_empty());
        assert!(take_quality_cell(Recording, None).is_empty());
    }

    #[test]
    fn dropout_warning_only_when_samples_dropped() {
        assert!(dropout_warning(0).is_none());
        let w = dropout_warning(512).expect("warning for dropped samples");
        assert!(w.contains("512"), "{w}");
        assert!(w.contains("dropped during capture"), "{w}");
        assert!(w.contains("re-measuring"), "{w}");
    }

    #[test]
    fn session_quality_summary_one_line_per_recorded_channel() {
        use crate::recording_types::ChannelRecordingState;
        let mut clean = ChannelRecording::new(0, "FL".to_string());
        clean.state = ChannelRecordingState::Done;
        let mut clean_q = quality_summary(true, &[], None);
        clean_q.score = 0.95;
        clean_q.rejected_count = 0;
        clean.result = Some(RecordingResult {
            channel: 0,
            wav_path: None,
            csv_path: None,
            frequencies: vec![],
            magnitude_db: vec![],
            phase_deg: vec![],
            impulse_response: None,
            impulse_time_ms: None,
            thd_percent: None,
            harmonic_distortion_db: None,
            excess_group_delay_ms: None,
            rt60_ms: None,
            clarity_c50_db: None,
            clarity_c80_db: None,
            spectrogram_db: None,
            quality: Some(clean_q),
        });

        let mut bad = ChannelRecording::new(1, "FR (Pos 2)".to_string());
        bad.state = ChannelRecordingState::ReviewNeeded;
        let mut bad_q = quality_summary(false, &["clipping detected"], None);
        bad_q.dropped_samples = 128;
        bad.result = Some(RecordingResult {
            channel: 1,
            wav_path: None,
            csv_path: None,
            frequencies: vec![],
            magnitude_db: vec![],
            phase_deg: vec![],
            impulse_response: None,
            impulse_time_ms: None,
            thd_percent: None,
            harmonic_distortion_db: None,
            excess_group_delay_ms: None,
            rt60_ms: None,
            clarity_c50_db: None,
            clarity_c80_db: None,
            spectrogram_db: None,
            quality: Some(bad_q),
        });

        // Channels without a result yet are not listed.
        let pending = ChannelRecording::new(2, "C".to_string());

        let lines = session_quality_summary(&[clean, bad, pending]);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("FL: OK (score 0.95"), "{}", lines[0]);
        assert!(lines[0].contains("4/4 sweeps"), "{}", lines[0]);
        assert!(lines[1].starts_with("FR (Pos 2): REVIEW"), "{}", lines[1]);
        assert!(lines[1].contains("clipping detected"), "{}", lines[1]);
        assert!(lines[1].contains("128 dropped samples"), "{}", lines[1]);
    }

    #[test]
    fn accepted_num_sweeps_for_save_is_min_over_done_channels() {
        use crate::recording_types::ChannelRecordingState;
        let mk = |state, accepted| {
            let mut ch = ChannelRecording::new(0, "L".to_string());
            ch.state = state;
            let mut q = quality_summary(true, &[], None);
            q.accepted_count = accepted;
            ch.result = Some(RecordingResult {
                channel: 0,
                wav_path: None,
                csv_path: None,
                frequencies: vec![],
                magnitude_db: vec![],
                phase_deg: vec![],
                impulse_response: None,
                impulse_time_ms: None,
                thd_percent: None,
                harmonic_distortion_db: None,
                excess_group_delay_ms: None,
                rt60_ms: None,
                clarity_c50_db: None,
                clarity_c80_db: None,
                spectrogram_db: None,
                quality: Some(q),
            });
            ch
        };

        // No captures → None (legacy "unknown" semantics preserved).
        assert_eq!(accepted_num_sweeps_for_save(&[]), None);
        let legacy = ChannelRecording::new(0, "L".to_string());
        assert_eq!(accepted_num_sweeps_for_save(&[legacy]), None);

        // Min across Done channels; ReviewNeeded takes do not count (the
        // user has not accepted them yet).
        let channels = vec![
            mk(ChannelRecordingState::Done, 4),
            mk(ChannelRecordingState::Done, 3),
            mk(ChannelRecordingState::ReviewNeeded, 1),
        ];
        assert_eq!(accepted_num_sweeps_for_save(&channels), Some(3));
    }

    #[test]
    fn summarize_take_quality_maps_capture_analysis_fields() {
        use sotf_audio::signal_analysis::{
            AnalysisResult, ClockDriftEstimate, ClippingInfo, MeasurementQualityReport,
        };
        let result = AnalysisResult {
            frequencies: vec![],
            spl_db: vec![],
            phase_deg: vec![],
            estimated_lag_samples: 0,
            impulse_response: vec![],
            impulse_time_ms: vec![],
            excess_group_delay_ms: vec![],
            thd_percent: vec![],
            harmonic_distortion_db: vec![],
            rt60_ms: vec![],
            clarity_c50_db: vec![],
            clarity_c80_db: vec![],
            spectrogram_db: vec![],
        };
        let report = MeasurementQualityReport {
            trustworthy: false,
            score: 0.3,
            quality_data_complete: true,
            missing_metrics: vec![],
            lag_confidence: 0.9,
            mean_coherence: Some(0.66),
            snr_db: vec![10.0, 20.0],
            median_snr_db: Some(15.0),
            clipping: ClippingInfo {
                clipped_samples: 3,
                non_finite_samples: 0,
                total_samples: 1000,
                fraction: 0.003,
            },
            issues: vec!["clipping detected".to_string()],
        };
        let capture = CaptureAnalysis {
            result,
            quality: report,
            drift: Some(ClockDriftEstimate {
                ppm: -45.0,
                start_lag_samples: 10,
                end_lag_samples: 20,
                confidence: 0.8,
            }),
            drift_corrected: true,
            dropped_samples: 7,
            accepted_count: 3,
            rejected_count: 1,
        };

        let q = summarize_take_quality(&capture);
        assert!(!q.trustworthy);
        assert_eq!(q.score, 0.3);
        assert_eq!(q.issues, vec!["clipping detected".to_string()]);
        assert_eq!(q.mean_coherence, Some(0.66));
        assert_eq!(q.median_snr_db, Some(15.0));
        assert_eq!(q.clip_fraction, 0.003);
        assert_eq!(q.drift_ppm, Some(-45.0));
        assert!(q.drift_corrected);
        assert_eq!(q.dropped_samples, 7);
        assert_eq!(q.accepted_count, 3);
        assert_eq!(q.rejected_count, 1);

        // Drift unavailable stays None — never collapses to 0 ppm.
        let capture_no_drift = CaptureAnalysis {
            drift: None,
            ..capture
        };
        assert_eq!(summarize_take_quality(&capture_no_drift).drift_ppm, None);
    }

    #[test]
    fn position_guidance_anchors_first_position_at_mlp() {
        let first = position_guidance(0, 3);
        assert!(first.contains("Position 1 of 3"), "{first}");
        assert!(first.contains("main listening position"), "{first}");
        let later = position_guidance(1, 3);
        assert!(later.contains("position 2 of 3"), "{later}");
        assert!(later.contains("60 cm"), "{later}");
    }
}
