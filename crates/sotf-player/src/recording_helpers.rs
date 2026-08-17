//! Shared recording-session helpers for all player frontends (TUI, GPUI, CLI).
//!
//! Consolidates the per-frontend recording logic flagged in
//! `reviews/20260818-recording.md` (B1, B3, B4, B5, C1) so every shell maps
//! signal types, converts levels, sanitizes file names, resolves mic
//! calibration, and builds capture parameters identically. The
//! `RecordingConfiguration` builder lives on
//! [`crate::ui_models::recording::RecordingScreenModel::build_recording_configuration`]
//! next to the other save-time helpers.

use crate::recording_types::RecordingSignalType;
use sotf_audio::signal_recorder::{
    DEFAULT_MLS_ORDER, SignalParams, SignalType, sweep_params_from_config,
};

/// Canonical filename for the saved recording session JSON (B5).
///
/// Room EQ's "FromFile" flow and the i18n strings point users at this name,
/// so every frontend must save the session as `<dir>/recordings.json`
/// regardless of the user-chosen session name.
pub const RECORDINGS_FILENAME: &str = "recordings.json";

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
        let paths = vec![
            Some("mic0.txt".to_string()),
            Some("mic1.txt".to_string()),
        ];
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
}
