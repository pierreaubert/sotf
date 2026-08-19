use super::build::build_octave_sweep_with_silence;
use super::consts::CANCELLED_ERR;
use super::consts::DEFAULT_MLS_ORDER;
use super::consts::PROBE_SEED;
use super::generate::generate_output_filenames;
use super::generate::generate_output_filenames_stereo;
use super::generate::generate_signal;
use super::generate::prepare_measurement_signal;
use super::measurement::measurement_amplitude_from_level_db;
use super::misc::CLIP_BLOCK_SAMPLES;
use super::misc::CLIP_THRESHOLD;
use super::misc::actionable_capture_error;
use super::misc::analyze_clipping;
#[cfg(not(target_os = "ios"))]
use super::misc::check_capture_clipping;
use super::misc::parse_channel_list;
use super::misc::prepare_signal;
use super::probe::gen_schroeder_narrowband_probe;
#[cfg(not(target_os = "ios"))]
use super::quality::{DriftAction, build_capture_quality, check_lag_lock, drift_action};
#[cfg(not(target_os = "ios"))]
use super::record::record_and_analyze;
#[cfg(not(target_os = "ios"))]
use super::record::record_and_analyze_multi;
#[cfg(not(target_os = "ios"))]
use super::record::resample_reference_signal;
use super::recording_session::RecordingSession;
use super::signal_params::sweep_params_from_config;
use super::signal_params::validate_signal_params;
use super::signal_type::SignalType;
#[cfg(not(target_os = "ios"))]
use super::types::CancelFlag;
use super::types::ChannelRecordingInfo;
use super::types::SignalParams;
use super::types::analyze_bass_anchor_recording;
use super::types::pick_direct_arrival_from_envelope;
use super::write::write_temp_wav;
use super::write::write_wav_file;
#[cfg(not(target_os = "ios"))]
use crate::signal_analysis::{ClockDriftEstimate, LagEstimate, MeasurementQualityConfig};
use hound::SampleFormat;
use hound::WavReader;
use std::path::{Path, PathBuf};
use std::str::FromStr;
#[cfg(not(target_os = "ios"))]
use std::sync::atomic::Ordering;
use tempfile::tempdir;

mod misc;

#[test]
fn schroeder_probe_has_release_grade_crest_factor() {
    for sample_rate in [44_100, 48_000, 96_000] {
        for duration_ms in [15.0_f32, 20.0, 125.0, 137.3, 1_000.0] {
            let frames = (duration_ms * sample_rate as f32 / 1_000.0).round() as usize;
            let signal =
                gen_schroeder_narrowband_probe(frames, sample_rate, 0.5, 800.0, 2_000.0).unwrap();
            let peak = signal
                .iter()
                .map(|sample| sample.abs())
                .fold(0.0_f32, f32::max);
            let rms = (signal.iter().map(|sample| sample * sample).sum::<f32>()
                / signal.len() as f32)
                .sqrt();
            let crest_factor = peak / rms;

            assert!((peak - 0.5).abs() < 1e-4);
            assert!(
                crest_factor <= 2.0,
                "{sample_rate} Hz, {duration_ms} ms probe crest factor \
                 {crest_factor:.3} exceeds 6.02 dB"
            );
        }
    }
}

#[test]
fn schroeder_probe_rejects_too_few_tones_and_invalid_amplitude() {
    assert!(gen_schroeder_narrowband_probe(96, 48_000, 0.5, 800.0, 2_000.0).is_err());
    assert!(gen_schroeder_narrowband_probe(48_000, 48_000, f32::NAN, 800.0, 2_000.0).is_err());

    let bounded = gen_schroeder_narrowband_probe(48_000, 48_000, 10.0, 800.0, 2_000.0).unwrap();
    let peak = bounded
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0_f32, f32::max);
    assert!((peak - 1.0).abs() < 1e-4);
}

#[cfg(not(target_os = "ios"))]
#[test]
fn schroeder_probe_cross_rate_reference_resamples_exact_playback_signal() {
    for duration_ms in [125.0_f32, 137.3] {
        let playback_frames = (duration_ms * 48_000.0 / 1_000.0).round() as usize;
        let playback =
            gen_schroeder_narrowband_probe(playback_frames, 48_000, 0.5, 800.0, 2_000.0).unwrap();
        let analysis = resample_reference_signal(&playback, 48_000, 44_100).unwrap();
        let expected = (playback.len() as f64 * 44_100.0 / 48_000.0).ceil() as usize;

        assert_eq!(analysis.len(), expected);
        assert!(analysis.iter().all(|sample| sample.is_finite()));
        assert!(analysis.iter().any(|sample| sample.abs() > 0.1));
    }
}

#[cfg(not(target_os = "ios"))]
#[test]
fn reference_resampling_tracks_the_capture_sample_rate() {
    let input: Vec<f32> = (0..4_410)
        .map(|i| (2.0 * std::f32::consts::PI * 1_000.0 * i as f32 / 44_100.0).sin())
        .collect();
    let output = resample_reference_signal(&input, 44_100, 48_000).unwrap();
    assert!(
        (4_798..=4_802).contains(&output.len()),
        "100 ms reference should remain 100 ms after resampling, got {} frames",
        output.len()
    );
}

#[cfg(not(target_os = "ios"))]
#[test]
fn reference_resampling_removes_filter_delay() {
    let mut input = vec![0.0f32; 4_410];
    let impulse_index = 1_000usize;
    input[impulse_index] = 1.0;

    let output = resample_reference_signal(&input, 44_100, 48_000).unwrap();
    let peak_index = output
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
        .map(|(index, _)| index)
        .unwrap();
    let expected_index = (impulse_index as f64 * 48_000.0 / 44_100.0).round() as usize;
    assert!(
        peak_index.abs_diff(expected_index) <= 1,
        "resampled impulse was shifted: expected {expected_index}, got {peak_index}"
    );
}

#[cfg(not(target_os = "ios"))]
#[test]
fn reference_resampling_preserves_late_signal_content() {
    let mut input = vec![0.0f32; 4_410];
    let tone_start = input.len() - 64;
    for (offset, sample) in input[tone_start..].iter_mut().enumerate() {
        *sample = (2.0 * std::f32::consts::PI * 1_000.0 * offset as f32 / 44_100.0).sin();
    }

    let output = resample_reference_signal(&input, 44_100, 48_000).unwrap();
    let output_tone_start = (tone_start as f64 * 48_000.0 / 44_100.0).round() as usize;
    let tail_energy: f32 = output[output_tone_start..]
        .iter()
        .map(|sample| sample * sample)
        .sum();
    assert!(
        tail_energy > 20.0,
        "late reference content was truncated (tail energy {tail_energy})"
    );
}

#[test]
fn cpal_input_callbacks_stay_lock_free() {
    let source = include_str!("../signal_recorder.rs");

    assert!(
        !source.contains(concat!("try", "_", "lock")),
        "signal_recorder input callbacks must not use mutex try-locks"
    );
    assert!(
        !source.contains(concat!("Mut", "ex<")),
        "signal_recorder capture buffers should stay lock-free"
    );
}

#[test]
fn cpal_input_callbacks_use_chunked_ring_buffer_writes() {
    let source = include_str!("../signal_recorder.rs");

    assert!(
        !source.contains(concat!("recorded_producer.", "push(")),
        "single-channel input callback should commit captured samples in ring-buffer chunks"
    );
    assert!(
        !source.contains(concat!("recorded_producers[mic_i].", "push(")),
        "multi-mic input callback should commit captured samples in ring-buffer chunks"
    );
    assert!(
        !source.contains(concat!(
            "capture_producer\n                        .",
            "push("
        )),
        "loopback capture callback should commit sample pairs in ring-buffer chunks"
    );
}

#[test]
fn cpal_input_stream_errors_are_warn_rate_limited() {
    let source = include_str!("../signal_recorder.rs");

    assert!(
        !source.contains(concat!(
            "log::debug!(\"[record_and_analyze] ",
            "Input stream error:"
        )),
        "single-channel input stream errors should be visible and rate-limited"
    );
    assert!(
        !source.contains(concat!(
            "log::debug!(\"[record_and_analyze_multi] ",
            "Input stream error:"
        )),
        "multi-channel input stream errors should be visible and rate-limited"
    );
    assert!(
        !source.contains(concat!(
            "log::debug!(\"[{log_tag_for_error}] ",
            "Input stream error:"
        )),
        "direct capture input stream errors should be visible and rate-limited"
    );
}

#[test]
fn test_effective_calibration_per_channel_priority() {
    let mut session = RecordingSession::new(48000, "sweep", 5.0, -20.0, None);
    session.mic_calibration_path = Some("/global/cal.txt".to_string());
    session.mic_calibration_paths = vec![
        Some("/ch0/cal.txt".to_string()),
        None,
        Some("/ch2/cal.txt".to_string()),
    ];
    // Per-channel takes priority over global
    assert_eq!(
        session.effective_calibration_for_channel(0),
        Some("/ch0/cal.txt")
    );
    // Falls back to global when per-channel is None
    assert_eq!(
        session.effective_calibration_for_channel(1),
        Some("/global/cal.txt")
    );
    // Per-channel takes priority
    assert_eq!(
        session.effective_calibration_for_channel(2),
        Some("/ch2/cal.txt")
    );
    // Out-of-bounds falls back to global
    assert_eq!(
        session.effective_calibration_for_channel(5),
        Some("/global/cal.txt")
    );
}

#[test]
fn test_effective_calibration_empty_string_falls_back() {
    let mut session = RecordingSession::new(48000, "sweep", 5.0, -20.0, None);
    session.mic_calibration_path = Some("/global/cal.txt".to_string());
    session.mic_calibration_paths = vec![Some("".to_string())];
    // Empty string should fall back to global
    assert_eq!(
        session.effective_calibration_for_channel(0),
        Some("/global/cal.txt")
    );
}

#[test]
fn test_effective_calibration_no_global_no_per_channel() {
    let session = RecordingSession::new(48000, "sweep", 5.0, -20.0, None);
    assert!(session.effective_calibration_for_channel(0).is_none());
}

#[test]
fn test_recording_session_serde_backward_compat() {
    // Old format without mic_calibration_paths
    let json = r#"{
            "version": "2.0",
            "timestamp": "2024-01-01T00:00:00Z",
            "sample_rate": 48000,
            "signal_type": "sweep",
            "signal_duration_secs": 5.0,
            "signal_level_db": -20.0,
            "sweep_range": null,
            "playback_device": null,
            "recording_device": null,
            "mic_calibration_path": "/global.txt",
            "channels": []
        }"#;
    let session: RecordingSession = serde_json::from_str(json).unwrap();
    assert!(session.mic_calibration_paths.is_empty());
    assert_eq!(
        session.effective_calibration_for_channel(0),
        Some("/global.txt")
    );
}

#[test]
fn test_channel_recording_info_serde_backward_compat() {
    // Old format without per-channel mic_calibration_path
    let json = r#"{
            "channel_index": 0,
            "channel_name": "L",
            "output_channel": 0,
            "input_channel": 0,
            "wav_path": "ch0.wav",
            "csv_path": "ch0.csv",
            "success": true,
            "error": null
        }"#;
    let info: ChannelRecordingInfo = serde_json::from_str(json).unwrap();
    assert!(info.mic_calibration_path.is_none());
}

#[test]
fn test_signal_type_from_str() {
    assert_eq!(SignalType::from_str("tone").unwrap(), SignalType::Tone);
    assert_eq!(
        SignalType::from_str("two-tone").unwrap(),
        SignalType::TwoTone
    );
    assert_eq!(SignalType::from_str("sweep").unwrap(), SignalType::Sweep);
    assert_eq!(
        SignalType::from_str("white-noise").unwrap(),
        SignalType::WhiteNoise
    );
    assert_eq!(SignalType::from_str("mls").unwrap(), SignalType::Mls);
    assert_eq!(
        SignalType::from_str("maximum-length-sequence").unwrap(),
        SignalType::Mls
    );
    assert_eq!(SignalType::from_str("dirac").unwrap(), SignalType::Dirac);
    assert!(SignalType::from_str("invalid").is_err());
}

#[test]
fn test_parse_channel_list() {
    assert_eq!(parse_channel_list("0").unwrap(), vec![0]); // Channel 0 is valid (0-based indexing)
    assert_eq!(parse_channel_list("1").unwrap(), vec![1]);
    assert_eq!(parse_channel_list("1,2,3").unwrap(), vec![1, 2, 3]);
    assert_eq!(parse_channel_list(" 1 , 2 , 3 ").unwrap(), vec![1, 2, 3]);
    assert_eq!(parse_channel_list("0,1,2").unwrap(), vec![0, 1, 2]); // 0-based channels

    assert!(parse_channel_list("1,1").is_err()); // Duplicate
    assert!(parse_channel_list("").is_err()); // Empty
    assert!(parse_channel_list("abc").is_err()); // Non-numeric
}

#[test]
fn test_validate_signal_params_tone() {
    let params = SignalParams::Tone {
        freq: 1000.0,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Tone, &params, 1.0, 48000).is_ok());

    let params_bad_freq = SignalParams::Tone {
        freq: 30000.0,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Tone, &params_bad_freq, 1.0, 48000).is_err());

    let params_bad_amp = SignalParams::Tone {
        freq: 1000.0,
        amp: 2.0,
    };
    assert!(validate_signal_params(SignalType::Tone, &params_bad_amp, 1.0, 48000).is_err());
}

#[test]
fn test_generate_output_filenames_stereo() {
    let (wav, csv) = generate_output_filenames_stereo(
        Some("test"),
        SignalType::Sweep,
        2, // send channel
        1, // record channel
        48000,
    );
    assert_eq!(wav, PathBuf::from("test_sweep_send2_rec1_48000.wav"));
    assert_eq!(csv, PathBuf::from("test_sweep_send2_rec1_48000.csv"));

    let (wav, csv) = generate_output_filenames_stereo(
        None,
        SignalType::Tone,
        1, // send channel
        3, // record channel
        44100,
    );
    assert_eq!(wav, PathBuf::from("tone_send1_rec3_44100.wav"));
    assert_eq!(csv, PathBuf::from("tone_send1_rec3_44100.csv"));
}

#[test]
fn test_generate_output_filenames() {
    let (wav, csv) = generate_output_filenames(Some("test"), SignalType::Sweep, 1, 48000);
    assert_eq!(wav, PathBuf::from("test_sweep_ch1_48000.wav"));
    assert_eq!(csv, PathBuf::from("test_sweep_ch1_48000.csv"));

    let (wav, csv) = generate_output_filenames(None, SignalType::Tone, 2, 44100);
    assert_eq!(wav, PathBuf::from("tone_ch2_44100.wav"));
    assert_eq!(csv, PathBuf::from("tone_ch2_44100.csv"));
}

#[test]
fn test_generate_signal_tone() {
    let params = SignalParams::Tone {
        freq: 1000.0,
        amp: 0.5,
    };
    let signal =
        generate_signal(SignalType::Tone, &params, 0.1, 48000).expect("Failed to generate tone");

    assert_eq!(signal.len(), 4800); // 0.1s * 48000 Hz

    // Check signal is non-zero and within amplitude bounds
    let max_val = signal
        .iter()
        .map(|&x| x.abs())
        .fold(0.0_f32, |a, b| a.max(b));
    assert!(
        max_val > 0.4 && max_val <= 0.5,
        "Tone amplitude out of range: {}",
        max_val
    );
}

#[test]
fn test_generate_signal_sweep() {
    let params = SignalParams::Sweep {
        start_freq: 20.0,
        end_freq: 20000.0,
        amp: 0.5,
    };
    let signal =
        generate_signal(SignalType::Sweep, &params, 1.0, 48000).expect("Failed to generate sweep");

    assert_eq!(signal.len(), 48000);

    let max_val = signal
        .iter()
        .map(|&x| x.abs())
        .fold(0.0_f32, |a, b| a.max(b));
    assert!(
        max_val > 0.4 && max_val <= 0.5,
        "Sweep amplitude out of range: {}",
        max_val
    );
}

#[test]
fn test_generate_signal_noise() {
    let params = SignalParams::Noise { amp: 0.5 };
    let signal = generate_signal(SignalType::WhiteNoise, &params, 1.0, 48000)
        .expect("Failed to generate white noise");

    assert_eq!(signal.len(), 48000);

    // Check that noise has content (not all zeros) - matches existing test pattern
    assert!(
        signal.iter().any(|&x| x.abs() > 0.01),
        "Noise signal should have non-zero samples"
    );
}

#[test]
fn test_generate_signal_mls() {
    let params = SignalParams::Mls {
        order: DEFAULT_MLS_ORDER,
        amp: 0.5,
    };
    let signal =
        generate_signal(SignalType::Mls, &params, 0.0, 48000).expect("Failed to generate MLS");

    assert_eq!(signal.len(), 65_535);
    assert!(signal.iter().all(|&s| s == 0.5 || s == -0.5));
}

#[test]
fn test_generate_signal_dirac() {
    let params = SignalParams::Dirac { amp: 0.5 };
    let signal =
        generate_signal(SignalType::Dirac, &params, 0.1, 48000).expect("Failed to generate Dirac");

    assert_eq!(signal.len(), 4800);
    assert_eq!(signal[0], 0.5);
    assert!(signal[1..].iter().all(|&s| s == 0.0));
}

#[test]
fn test_generate_signal_type_mismatch() {
    // Wrong params for signal type should fail
    let params = SignalParams::Tone {
        freq: 1000.0,
        amp: 0.5,
    };
    let result = generate_signal(SignalType::Sweep, &params, 1.0, 48000);
    assert!(result.is_err());
}

#[test]
fn test_prepare_signal_adds_padding() {
    let signal = vec![1.0; 4800]; // 0.1s at 48kHz
    let prepared = prepare_signal(signal.clone(), 48000);

    // Should be longer due to fades and padding
    assert!(
        prepared.len() > signal.len(),
        "Prepared signal should be longer than original"
    );

    // First samples should be faded (smaller than original)
    assert!(
        prepared[0].abs() < signal[0].abs(),
        "First sample should be faded in"
    );

    // Last samples should be faded
    assert!(
        prepared[prepared.len() - 1].abs() < 0.1,
        "Last sample should be faded out or padded"
    );
}

#[test]
fn test_write_and_read_wav_roundtrip() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let wav_path = temp_dir.path().join("test.wav");

    // Generate a simple signal
    let sample_rate = 48000;
    let duration = 0.1;
    let signal: Vec<f32> = (0..(sample_rate as f32 * duration) as usize)
        .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sample_rate as f32).sin() * 0.5)
        .collect();

    // Write WAV
    write_wav_file(&wav_path, &signal, sample_rate, 1).expect("Failed to write WAV");

    assert!(wav_path.exists(), "WAV file should exist");

    // Read it back using hound
    let mut reader = WavReader::open(&wav_path).expect("Failed to open WAV for reading");

    let spec = reader.spec();
    assert_eq!(spec.channels, 1);
    assert_eq!(spec.sample_rate, sample_rate);
    assert_eq!(spec.sample_format, SampleFormat::Float);

    let read_samples: Vec<f32> = reader
        .samples::<f32>()
        .collect::<Result<Vec<_>, _>>()
        .expect("Failed to read samples");

    // Verify samples match (with small floating point tolerance)
    assert_eq!(read_samples.len(), signal.len());
    for (i, (&original, &read)) in signal.iter().zip(read_samples.iter()).enumerate() {
        assert!(
            (original - read).abs() < 1e-6,
            "Sample {} mismatch: original={}, read={}",
            i,
            original,
            read
        );
    }
}

#[test]
fn test_write_temp_wav() {
    let signal = vec![0.5, 0.3, -0.2, -0.4, 0.0];
    let sample_rate = 48000;

    let temp_file = write_temp_wav(&signal, sample_rate, 1).expect("Failed to write temp WAV");

    assert!(temp_file.path().exists());

    // Verify it's a valid WAV
    let reader = WavReader::open(temp_file.path()).expect("Failed to open temp WAV");
    let spec = reader.spec();
    assert_eq!(spec.channels, 1);
    assert_eq!(spec.sample_rate, sample_rate);
}

#[test]
fn test_validate_signal_params_duration() {
    let params = SignalParams::Tone {
        freq: 1000.0,
        amp: 0.5,
    };

    // Valid duration
    assert!(validate_signal_params(SignalType::Tone, &params, 1.0, 48000).is_ok());

    // Invalid duration
    assert!(validate_signal_params(SignalType::Tone, &params, 0.0, 48000).is_err());
    assert!(validate_signal_params(SignalType::Tone, &params, -1.0, 48000).is_err());
}

#[test]
fn test_validate_signal_params_mls() {
    let params = SignalParams::Mls {
        order: DEFAULT_MLS_ORDER,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Mls, &params, 0.0, 48000).is_ok());

    let bad_order = SignalParams::Mls {
        order: 25,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Mls, &bad_order, 1.0, 48000).is_err());

    let bad_amp = SignalParams::Mls {
        order: DEFAULT_MLS_ORDER,
        amp: 2.0,
    };
    assert!(validate_signal_params(SignalType::Mls, &bad_amp, 1.0, 48000).is_err());
}

#[test]
fn test_validate_signal_params_dirac() {
    assert!(
        validate_signal_params(
            SignalType::Dirac,
            &SignalParams::Dirac { amp: 0.5 },
            0.1,
            48000
        )
        .is_ok()
    );
    assert!(
        validate_signal_params(
            SignalType::Dirac,
            &SignalParams::Dirac { amp: 0.0 },
            0.1,
            48000
        )
        .is_err()
    );
}

#[test]
fn test_validate_signal_params_frequency_nyquist() {
    let sample_rate = 48000;
    let nyquist = sample_rate as f32 / 2.0;

    // Valid frequency
    let params_valid = SignalParams::Tone {
        freq: 1000.0,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Tone, &params_valid, 1.0, sample_rate).is_ok());

    // Frequency above Nyquist
    let params_high = SignalParams::Tone {
        freq: nyquist + 100.0,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Tone, &params_high, 1.0, sample_rate).is_err());

    // Zero frequency
    let params_zero = SignalParams::Tone {
        freq: 0.0,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Tone, &params_zero, 1.0, sample_rate).is_err());
}

#[test]
fn test_validate_signal_params_sweep_order() {
    let sample_rate = 48000;

    // Valid sweep (ascending)
    let params_valid = SignalParams::Sweep {
        start_freq: 20.0,
        end_freq: 20000.0,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Sweep, &params_valid, 1.0, sample_rate).is_ok());

    // Invalid sweep (start >= end)
    let params_reversed = SignalParams::Sweep {
        start_freq: 20000.0,
        end_freq: 20.0,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Sweep, &params_reversed, 1.0, sample_rate).is_err());

    let params_equal = SignalParams::Sweep {
        start_freq: 1000.0,
        end_freq: 1000.0,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Sweep, &params_equal, 1.0, sample_rate).is_err());
}

/// Regression test: Verify that record_and_analyze doesn't just copy the input file
///
/// This test ensures that the recording function actually performs recording,
/// not just file copying. It checks that:
/// 1. The function signature includes both input and output paths
/// 2. The implementation uses proper recording mechanisms
///
/// Note: This is a compile-time/documentation test. The actual E2E test
/// should verify that recorded audio differs from input when there's
/// actual signal processing or latency.
#[test]
fn test_record_and_analyze_signature() {
    // This test documents the expected signature of record_and_analyze.
    // It takes separate paths for input (playback) and output (recording),
    // which is the first line of defense against the "copy instead of record" bug.

    // Verify function exists with correct parameter count and types
    // by calling it with dummy parameters (compile-time check only)
    let _check = || async {
        let temp_path = Path::new("/tmp/input.wav");
        let output_path = Path::new("/tmp/output.wav");
        let csv_path = Path::new("/tmp/output.csv");
        let reference: Vec<f32> = vec![];

        // This won't run, but ensures the signature is correct
        if false {
            let outcome = record_and_analyze(
                temp_path,   // temp_wav_path (for playback)
                output_path, // recorded_wav_path (for recording output)
                &reference,  // reference_signal
                48000_u32,   // sample_rate
                csv_path,    // output_csv_path
                1_u16,       // output_channel
                1_u16,       // input_channel
                None,        // output_device_name
                None,        // input_device_name
                None,        // microphone_compensation_path
                None,        // sweep_range
                1_u16,       // num_sweeps (1 = legacy single-sweep capture)
                None,        // cancel flag
            );
            // Task 7: the return value is a CaptureAnalysis wrapper carrying
            // the math-dsp analysis plus the per-take quality report, drift
            // estimate, and dropout count (R6). Task 8 adds the truthful
            // accepted/rejected take counts for `num_sweeps` persistence.
            if let Ok(capture) = outcome {
                let _ = &capture.result;
                let _ = &capture.quality;
                let _ = &capture.drift;
                let _ = capture.drift_corrected;
                let _ = capture.dropped_samples;
                let _ = capture.accepted_count;
                let _ = capture.rejected_count;
            }
        }
    };

    // Just verify it compiles
    let _ = _check;
}

// --- Task 7: lag-lock gate, drift thresholds, quality wrapper ---

#[cfg(not(target_os = "ios"))]
fn lag_estimate_with_confidence(confidence: f32) -> LagEstimate {
    LagEstimate {
        lag_samples: 100,
        normalized_peak: 0.9,
        peak_to_sidelobe_db: 20.0,
        confidence,
    }
}

#[cfg(not(target_os = "ios"))]
fn drift_estimate(ppm: f64, confidence: f32) -> ClockDriftEstimate {
    ClockDriftEstimate {
        ppm,
        start_lag_samples: 4800,
        end_lag_samples: 4800,
        confidence,
    }
}

#[test]
#[cfg(not(target_os = "ios"))]
fn test_drift_action_thresholds() {
    // No estimate, or a low-confidence estimate, never corrects.
    assert_eq!(drift_action(None), DriftAction::None);
    assert_eq!(
        drift_action(Some(&drift_estimate(500.0, 0.1))),
        DriftAction::None
    );
    // Within tolerance (|ppm| <= 20): no correction.
    assert_eq!(
        drift_action(Some(&drift_estimate(0.0, 1.0))),
        DriftAction::None
    );
    assert_eq!(
        drift_action(Some(&drift_estimate(19.9, 1.0))),
        DriftAction::None
    );
    assert_eq!(
        drift_action(Some(&drift_estimate(20.0, 1.0))),
        DriftAction::None
    );
    // Above 20 ppm: correct (sign-independent).
    assert_eq!(
        drift_action(Some(&drift_estimate(21.0, 1.0))),
        DriftAction::Correct
    );
    assert_eq!(
        drift_action(Some(&drift_estimate(-45.0, 1.0))),
        DriftAction::Correct
    );
    // Above 100 ppm: correct and advise.
    assert_eq!(
        drift_action(Some(&drift_estimate(101.0, 0.9))),
        DriftAction::CorrectAndAdvise
    );
    assert_eq!(
        drift_action(Some(&drift_estimate(-250.0, 1.0))),
        DriftAction::CorrectAndAdvise
    );
}

#[test]
#[cfg(not(target_os = "ios"))]
fn test_lag_lock_gate_threshold() {
    let minimum = MeasurementQualityConfig::default().minimum_lag_confidence;
    assert!(check_lag_lock(&lag_estimate_with_confidence(minimum), "test").is_ok());
    assert!(check_lag_lock(&lag_estimate_with_confidence(0.9), "test").is_ok());

    let err = check_lag_lock(&lag_estimate_with_confidence(minimum - 0.01), "test")
        .expect_err("below-threshold confidence must be rejected");
    assert!(
        err.contains("No reliable signal lock"),
        "error must be actionable: {err}"
    );
    assert!(err.contains("check mic connection"), "error: {err}");
}

#[test]
#[cfg(not(target_os = "ios"))]
fn test_build_capture_quality_clean_take_is_trustworthy() {
    let lag = lag_estimate_with_confidence(0.9);
    let clean = vec![0.0_f32; 4096];
    let report = build_capture_quality(&clean, &lag, None, None, None, Vec::new());
    assert!(report.trustworthy, "issues: {:?}", report.issues);
    assert!(report.issues.is_empty());
    // Single-take captures do not wire coherence / SNR inputs (repeat-sweep
    // averaging, Task 8, does): reported missing, but not treated as
    // failures under the default config.
    assert!(!report.quality_data_complete);
    assert!(report.missing_metrics.iter().any(|m| m == "coherence"));
    assert!(report.missing_metrics.iter().any(|m| m == "noise_floor_db"));
}

#[test]
#[cfg(not(target_os = "ios"))]
fn test_build_capture_quality_extra_issue_marks_untrustworthy() {
    let lag = lag_estimate_with_confidence(0.9);
    let clean = vec![0.0_f32; 4096];
    let report = build_capture_quality(
        &clean,
        &lag,
        None,
        None,
        None,
        vec!["clock drift 150.0 ppm".to_string()],
    );
    assert!(!report.trustworthy);
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.contains("clock drift")),
        "issues: {:?}",
        report.issues
    );
}

#[test]
#[cfg(not(target_os = "ios"))]
fn test_build_capture_quality_flags_clipping() {
    let lag = lag_estimate_with_confidence(0.9);
    // 1% of samples at full scale — above the 0.1% default clip threshold
    // but below the Task-1 hard-abort block rule, so this path is reachable.
    let mut clipped = vec![0.0_f32; 4096];
    for sample in clipped.iter_mut().take(41) {
        *sample = 1.0;
    }
    let report = build_capture_quality(&clipped, &lag, None, None, None, Vec::new());
    assert!(!report.trustworthy);
    assert!(
        report.issues.iter().any(|issue| issue.contains("clipping")),
        "issues: {:?}",
        report.issues
    );
    assert!(report.clipping.clipped_samples == 41);
}

#[test]
#[cfg(not(target_os = "ios"))]
fn test_active_reference_span_trims_padding() {
    let mut padded = vec![0.0_f32; 4800];
    padded.extend_from_slice(&[0.5, -0.5, 0.25]);
    padded.extend_from_slice(&[0.0_f32; 2400]);
    let active = super::quality::active_reference_span(&padded);
    assert_eq!(active, &[0.5, -0.5, 0.25]);

    // An all-silence reference is returned unchanged (no active span).
    let silence = vec![0.0_f32; 128];
    assert_eq!(super::quality::active_reference_span(&silence).len(), 128);
}

/// Offline plumbing test (no hardware): a delayed log sweep must lock with
/// high confidence at exactly the injected lag, with negligible measured
/// clock drift.
#[test]
#[cfg(not(target_os = "ios"))]
fn test_lag_and_drift_on_delayed_sweep() {
    let sample_rate = 48_000_u32;
    let sweep = crate::signals::gen_log_sweep(20.0, 20_000.0, 0.5, sample_rate, 1.0);
    let delay = 4_800_usize; // 100 ms pre-silence
    let mut recording = vec![0.0_f32; delay];
    recording.extend_from_slice(&sweep);

    let lag = crate::signal_analysis::estimate_lag_with_confidence(&sweep, &recording)
        .expect("lag estimation on a clean delayed sweep");
    assert_eq!(lag.lag_samples, delay as isize);
    assert!(
        lag.confidence >= MeasurementQualityConfig::default().minimum_lag_confidence,
        "clean sweep must pass the lag-lock gate (confidence {:.3})",
        lag.confidence
    );
    assert!(check_lag_lock(&lag, "test").is_ok());

    let drift = crate::signal_analysis::estimate_clock_drift(&sweep, &recording, sample_rate)
        .expect("drift estimation on a clean delayed sweep");
    assert!(
        drift.ppm.abs() <= super::quality::DRIFT_CORRECT_PPM,
        "no synthetic drift: expected |ppm| <= 20, got {:.2}",
        drift.ppm
    );
    assert_eq!(drift_action(Some(&drift)), DriftAction::None);
}

/// Offline plumbing test (no hardware): an injected +150 ppm capture-clock
/// stretch must be recovered by `estimate_clock_drift`, and
/// `correct_clock_drift` must bring the residual back under the correction
/// threshold.
///
/// Regression guard for the math-dsp ppm-scale bug fixed in math-audio
/// 0.5.26 (previously `estimate_clock_drift` returned true ppm multiplied
/// by the sample rate — it divided the sample-domain lag change by elapsed
/// *seconds*; the engine used to compensate via `normalize_clock_drift_ppm`,
/// removed once the pin moved to the fixed release).
#[test]
#[cfg(not(target_os = "ios"))]
fn test_clock_drift_estimate_and_correct_roundtrip() {
    let sample_rate = 48_000_u32;
    // Start at 100 Hz (not 20 Hz): the drift estimator's 8192-sample start
    // window must span enough cycles that periodic correlation sidelobes
    // fall inside its guard band, keeping the estimate unambiguous.
    let sweep = crate::signals::gen_log_sweep(100.0, 20_000.0, 0.5, sample_rate, 2.0);
    let delay = 4_800_usize;
    let mut reference_take = vec![0.0_f32; delay];
    reference_take.extend_from_slice(&sweep);

    // Simulate a capture clock running 150 ppm fast: content lands at
    // later indices than a stable clock would put it (linear interp, same
    // scheme as correct_clock_drift).
    // The trailing silence matters: with a nonzero tail, 1-sample overlaps
    // at the maximum candidate lag normalize to exactly 1.0 and outscore
    // the true (interpolated, < 1.0) correlation peak.
    let injected_ppm = 150.0_f64;
    let scale = 1.0 - injected_ppm / 1e6;
    let out_len = reference_take.len() + 4_800;
    let mut drifted = vec![0.0_f32; out_len];
    for (index, sample) in drifted.iter_mut().enumerate() {
        let source = index as f64 * scale;
        if source <= (reference_take.len() - 1) as f64 {
            let left = source.floor() as usize;
            let right = (left + 1).min(reference_take.len() - 1);
            let frac = (source - left as f64) as f32;
            *sample = reference_take[left] + frac * (reference_take[right] - reference_take[left]);
        }
    }

    let estimate = crate::signal_analysis::estimate_clock_drift(&sweep, &drifted, sample_rate)
        .expect("drift estimation on a drifted sweep");
    // Integer-lag quantization: 1 sample over the ~87808-sample window
    // baseline is ~11 ppm; interpolation peak shift adds another sample.
    assert!(
        (estimate.ppm - injected_ppm).abs() < 45.0,
        "expected ~{injected_ppm} ppm, got {:.2} (ppm-scale regression? math-audio >= 0.5.26 required)",
        estimate.ppm
    );
    assert_eq!(drift_action(Some(&estimate)), DriftAction::CorrectAndAdvise);

    let corrected =
        crate::signal_analysis::correct_clock_drift(&drifted, &estimate).expect("drift correction");
    assert_eq!(corrected.len(), drifted.len());
    let residual = crate::signal_analysis::estimate_clock_drift(&sweep, &corrected, sample_rate)
        .expect("drift re-estimation after correction");
    assert!(
        residual.ppm.abs() < 60.0,
        "residual drift after correction should be small, got {:.2} ppm",
        residual.ppm
    );
    // Correct-then-verify (task-8 review A1): a genuine correction keeps the
    // lag lock, so the verify step must not discard it.
    assert!(
        super::record::correction_keeps_lock(&reference_take, &drifted, &corrected, sample_rate),
        "a genuine drift correction must keep the lag lock"
    );
}

// --- Task 10: residuals (cancel masking, drift correct-then-verify) ---

/// Item 5: on in-loop cancel, a `manager.stop()` failure must not mask the
/// cancellation — `CANCELLED_ERR` always wins.
#[test]
#[cfg(not(target_os = "ios"))]
fn cancel_aware_stop_lets_cancel_win_over_stop_failure() {
    use super::record::cancel_aware_stop;

    let err = cancel_aware_stop(Err("boom"), false).unwrap_err();
    assert!(err.contains("Failed to stop playback"), "{err}");
    assert!(err.contains("boom"), "{err}");

    assert!(cancel_aware_stop(Err("boom"), true).is_ok());
    assert!(cancel_aware_stop(Ok::<(), &str>(()), true).is_ok());
    assert!(cancel_aware_stop(Ok::<(), &str>(()), false).is_ok());
}

/// A1: estimates above DRIFT_IMPLAUSIBLE_PPM are never applied.
#[test]
#[cfg(not(target_os = "ios"))]
fn drift_action_rejects_physically_implausible_estimates() {
    let mk = |ppm: f64| ClockDriftEstimate {
        ppm,
        start_lag_samples: 0,
        end_lag_samples: 0,
        confidence: 0.9,
    };
    assert_eq!(drift_action(Some(&mk(5000.0))), DriftAction::Implausible);
    assert_eq!(drift_action(Some(&mk(-2500.0))), DriftAction::Implausible);
    // At/under the clamp the normal thresholds still apply.
    assert_eq!(
        drift_action(Some(&mk(2000.0))),
        DriftAction::CorrectAndAdvise
    );
    assert_eq!(drift_action(Some(&mk(50.0))), DriftAction::Correct);
    assert_eq!(drift_action(Some(&mk(5.0))), DriftAction::None);
    // Low-confidence estimates are never acted upon, implausible or not.
    let low_conf = ClockDriftEstimate {
        confidence: 0.1,
        ..mk(9000.0)
    };
    assert_eq!(drift_action(Some(&low_conf)), DriftAction::None);
}

/// A1: the correct-then-verify step discards a "correction" that collapses
/// the lag lock and keeps one that preserves it.
#[test]
#[cfg(not(target_os = "ios"))]
fn correction_keeps_lock_detects_collapsed_lock() {
    let sample_rate = 48_000_u32;
    let reference = padded_sweep_reference(sample_rate);
    let ir = test_room_ir();
    let raw = simulate_sweep_take(&reference, &ir, 0.003, 0xC0FF_EE00);

    // Identity "correction" keeps the lock.
    let corrected_good = raw.clone();
    assert!(super::record::correction_keeps_lock(
        &reference,
        &raw,
        &corrected_good,
        sample_rate,
    ));

    // A garbage correction (pure noise) collapses the lock — and shifts the
    // apparent lag wildly, which is what actually catches it: the confidence
    // estimator reports spuriously high scores on noise.
    let mut state = 0xDEAD_BEEF_u64;
    let corrected_garbage: Vec<f32> = (0..raw.len())
        .map(|_| xorshift_noise(&mut state) * 0.5)
        .collect();
    assert!(!super::record::correction_keeps_lock(
        &reference,
        &raw,
        &corrected_garbage,
        sample_rate,
    ));
}

#[cfg(not(target_os = "ios"))]
#[test]
fn discarded_drift_correction_has_user_visible_quality_advisory() {
    let message = super::record::drift_correction_discard_message(50.0);
    assert!(message.contains("50.0 ppm"));
    assert!(message.contains("lag-lock verification failed"));
    assert!(message.contains("raw capture was kept"));
}

// --- Task 8: repeat-sweep averaging, coherence, extended CSV ---

/// Deterministic xorshift PRNG returning samples in [-1, 1).
#[cfg(not(target_os = "ios"))]
fn xorshift_noise(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    ((*state >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
}

#[cfg(not(target_os = "ios"))]
fn convolve(signal: &[f32], ir: &[f32]) -> Vec<f32> {
    let mut out = vec![0.0_f32; signal.len() + ir.len() - 1];
    for (i, &x) in signal.iter().enumerate() {
        if x == 0.0 {
            continue;
        }
        for (j, &h) in ir.iter().enumerate() {
            out[i + j] += x * h;
        }
    }
    out
}

/// Simulate one noisy capture of `reference` played through a known room IR:
/// 100 ms of pre-roll room noise, the convolved sweep with additive noise,
/// then a digital-silence tail (keeps the lag estimator's max-candidate-lag
/// edge case dormant — see the Task-7 ledger note on 1-sample overlaps).
#[cfg(not(target_os = "ios"))]
fn simulate_sweep_take(reference: &[f32], ir: &[f32], noise_amp: f32, seed: u64) -> Vec<f32> {
    let mut state = seed | 1;
    let mut take: Vec<f32> = (0..4800)
        .map(|_| xorshift_noise(&mut state) * noise_amp)
        .collect();
    let convolved = convolve(reference, ir);
    take.extend(
        convolved
            .iter()
            .map(|&s| s + xorshift_noise(&mut state) * noise_amp),
    );
    take.extend(std::iter::repeat_n(0.0, 4800));
    take
}

/// Reference for the synthetic-capture tests: the raw sweep plus 250 ms of
/// trailing digital-silence padding, mirroring `prepare_signal`. The lag
/// estimator's extreme-candidate-lag (few-sample) overlaps then have zero
/// variance on the reference side and cannot outscore the true peak (see the
/// Task-7 ledger note on the lag-estimator edge fragility).
#[cfg(not(target_os = "ios"))]
fn padded_sweep_reference(sample_rate: u32) -> Vec<f32> {
    let mut reference = crate::signals::gen_log_sweep(20.0, 20_000.0, 0.5, sample_rate, 1.0);
    reference.extend(std::iter::repeat_n(0.0, sample_rate as usize / 4));
    reference
}

/// A plausible room IR: direct + two discrete reflections + a short
/// exponentially decaying diffuse tail.
#[cfg(not(target_os = "ios"))]
fn test_room_ir() -> Vec<f32> {
    let mut ir = vec![0.0_f32; 512];
    ir[0] = 0.5;
    ir[96] = 0.2; // 2 ms reflection
    ir[240] = 0.1; // 5 ms reflection
    let mut state = 0x1234_5678_9abc_def0_u64;
    for (i, tap) in ir.iter_mut().enumerate().skip(24) {
        *tap += xorshift_noise(&mut state) * 0.05 * (-(i as f32) / 150.0).exp();
    }
    ir
}

/// A grossly different "room" — the deconvolved response of a take captured
/// through this IR is a global outlier against `test_room_ir` takes and must
/// be median/MAD-rejected.
#[cfg(not(target_os = "ios"))]
fn grossly_different_ir() -> Vec<f32> {
    let mut ir = vec![0.0_f32; 512];
    ir[0] = 0.3;
    ir[200] = -0.6;
    ir[400] = 0.45;
    ir
}

/// Synthetic integration test (no hardware): five noisy captures of a sweep
/// through a known room IR, one grossly corrupted. The averaging must reject
/// the outlier, yield high coherence from the four clean takes, write both
/// new CSV columns (still parseable by math-dsp's positional reader),
/// preserve the raw takes, and report truthful take counts.
#[test]
#[cfg(not(target_os = "ios"))]
fn test_repeat_sweep_averaging_rejects_outlier_and_extends_csv() {
    let sample_rate = 48_000_u32;
    let reference = padded_sweep_reference(sample_rate);
    let ir = test_room_ir();
    let takes: Vec<Vec<f32>> = (0..4_u64)
        .map(|i| simulate_sweep_take(&reference, &ir, 0.003, 0xC1EA_0000 + i))
        .chain(std::iter::once(simulate_sweep_take(
            &reference,
            &grossly_different_ir(),
            0.003,
            0xBAD0_0000,
        )))
        .collect();

    let dir = tempdir().unwrap();
    let wav = dir.path().join("L.wav");
    let csv = dir.path().join("L.csv");
    // Plant a stale higher-N sibling from a fictional previous run (task-8
    // review A4): it must be removed when this 5-take run writes its takes.
    let stale = dir.path().join("L.take9.wav");
    std::fs::write(&stale, b"stale").unwrap();
    // An unrelated user file sharing the `.take` prefix must survive the
    // cleanup (task-10 review): only `{stem}.take{N}.wav` is matched.
    let keep = dir.path().join("L.takeaway.wav");
    std::fs::write(&keep, b"keep").unwrap();
    let capture = super::record::analyze_sweep_takes(
        &wav,
        &csv,
        &takes,
        &reference,
        sample_rate,
        sample_rate,
        Some((20.0, 20_000.0)),
        None,
        0,
        "test",
    )
    .expect("repeat-sweep averaging on synthetic takes");

    assert!(
        !stale.exists(),
        "stale L.take9.wav from a higher-N run must be cleaned up"
    );
    assert!(
        keep.exists(),
        "unrelated L.takeaway.wav must survive the take-WAV cleanup"
    );

    // Outlier rejected; num_sweeps metadata is truthful.
    assert_eq!(capture.accepted_count, 4);
    assert_eq!(capture.rejected_count, 1);
    // High coherence from the clean takes, wired into the quality report
    // together with the noise-floor SNR pair.
    let mean_coherence = capture
        .quality
        .mean_coherence
        .expect("4 accepted takes yield coherence");
    assert!(
        mean_coherence > 0.8,
        "mean coherence should be high for clean takes, got {mean_coherence}"
    );
    assert!(
        capture.quality.median_snr_db.is_some(),
        "noise floor wired into the quality report"
    );
    assert!(
        capture.quality.trustworthy,
        "clean averaged capture should be trustworthy, issues: {:?}",
        capture.quality.issues
    );

    // Raw (pre-correction) takes preserved next to the averaged WAV.
    assert!(wav.exists());
    for i in 1..=5 {
        assert!(
            dir.path().join(format!("L.take{i}.wav")).exists(),
            "raw take {i} WAV preserved"
        );
    }

    // CSV: the new columns are appended after the original 8, correctly named.
    let text = std::fs::read_to_string(&csv).unwrap();
    let mut lines = text.lines();
    assert_eq!(
        lines.next().unwrap(),
        "frequency_hz,spl_db,phase_deg,thd_percent,rt60_ms,c50_db,c80_db,group_delay_ms,coherence,noise_floor_db"
    );
    for line in lines {
        let parts: Vec<&str> = line.split(',').collect();
        assert_eq!(parts.len(), 10, "row has both appended columns");
        let coherence: f32 = parts[8].parse().unwrap();
        assert!(
            (0.0..=1.0).contains(&coherence),
            "coherence is gamma^2 in [0, 1], got {coherence}"
        );
        let noise: f32 = parts[9].parse().unwrap();
        assert!((-400.0..0.0).contains(&noise), "noise floor dB: {noise}");
    }

    // math-dsp's reader parses columns >= 8 POSITIONALLY: the extended file
    // must still round-trip with the SPL column intact.
    let roundtrip =
        crate::signal_analysis::read_analysis_csv(&csv).expect("read_analysis_csv on extended CSV");
    assert_eq!(
        roundtrip.frequencies.len(),
        capture.result.frequencies.len()
    );
    for (read_back, written) in roundtrip.spl_db.iter().zip(capture.result.spl_db.iter()) {
        assert!(
            (read_back - written).abs() < 0.01,
            "spl column round-trips: {read_back} vs {written}"
        );
    }
}

/// With only three takes, coherence cannot be computed (math-dsp needs at
/// least 4 accepted takes): the CSV must OMIT the `coherence` column rather
/// than fabricate an all-ones one, while the real noise floor is still
/// written.
#[test]
#[cfg(not(target_os = "ios"))]
fn test_repeat_sweep_three_takes_omits_coherence_column() {
    let sample_rate = 48_000_u32;
    let reference = padded_sweep_reference(sample_rate);
    let ir = test_room_ir();
    let takes: Vec<Vec<f32>> = (0..3_u64)
        .map(|i| simulate_sweep_take(&reference, &ir, 0.003, 0xFEED_0000 + i))
        .collect();

    let dir = tempdir().unwrap();
    let wav = dir.path().join("R.wav");
    let csv = dir.path().join("R.csv");
    let capture = super::record::analyze_sweep_takes(
        &wav,
        &csv,
        &takes,
        &reference,
        sample_rate,
        sample_rate,
        Some((20.0, 20_000.0)),
        None,
        0,
        "test",
    )
    .expect("3-take repeat capture");

    assert_eq!(capture.accepted_count + capture.rejected_count, 3);
    assert!(
        capture.quality.mean_coherence.is_none(),
        "no coherence with < 4 accepted takes"
    );
    assert!(
        capture
            .quality
            .missing_metrics
            .iter()
            .any(|m| m == "coherence"),
        "coherence reported missing, not faked"
    );

    let header = std::fs::read_to_string(&csv).unwrap();
    let header = header.lines().next().unwrap();
    assert!(
        !header.split(',').any(|col| col == "coherence"),
        "coherence column omitted: {header}"
    );
    assert!(
        header.split(',').any(|col| col == "noise_floor_db"),
        "noise floor column present: {header}"
    );
    assert!(
        header.ends_with(",noise_floor_db"),
        "new columns stay appended last: {header}"
    );
    crate::signal_analysis::read_analysis_csv(&csv).expect("round-trip without coherence column");
}

/// A take that never locks (pure noise) aborts the whole set — REW abort
/// semantics, no silent partial average — before any WAV is written.
#[test]
#[cfg(not(target_os = "ios"))]
fn test_repeat_sweep_aborts_when_a_take_loses_lock() {
    let sample_rate = 48_000_u32;
    let reference = padded_sweep_reference(sample_rate);
    let ir = test_room_ir();
    let clean = simulate_sweep_take(&reference, &ir, 0.003, 0xAAAA_0000);
    let mut state = 0xDEAD_0000_u64;
    let noise_take: Vec<f32> = (0..clean.len())
        .map(|_| xorshift_noise(&mut state) * 0.1)
        .collect();

    let dir = tempdir().unwrap();
    let wav = dir.path().join("L.wav");
    let csv = dir.path().join("L.csv");
    let err = super::record::analyze_sweep_takes(
        &wav,
        &csv,
        &[clean, noise_take],
        &reference,
        sample_rate,
        sample_rate,
        Some((20.0, 20_000.0)),
        None,
        0,
        "test",
    )
    .expect_err("a take with no signal lock must abort the set");
    assert!(
        err.contains("check mic connection"),
        "error is actionable: {err}"
    );
    assert!(!wav.exists(), "aborted set leaves no averaged WAV");
    assert!(
        !dir.path().join("L.take1.wav").exists(),
        "aborted set leaves no raw takes"
    );
}

/// Single-take capture (`num_sweeps == 1`) keeps the Task-7 behavior: the
/// corrected take is written directly, no raw-take siblings, quality report
/// keeps the None-metrics — but the CSV still gains the real noise-floor
/// column from the pre-silence window.
#[test]
#[cfg(not(target_os = "ios"))]
fn test_single_take_behavior_with_noise_floor_column() {
    let sample_rate = 48_000_u32;
    let reference = padded_sweep_reference(sample_rate);
    let ir = test_room_ir();
    let take = simulate_sweep_take(&reference, &ir, 0.003, 0x5EED_0000);

    let dir = tempdir().unwrap();
    let wav = dir.path().join("L.wav");
    let csv = dir.path().join("L.csv");
    let capture = super::record::analyze_sweep_takes(
        &wav,
        &csv,
        &[take],
        &reference,
        sample_rate,
        sample_rate,
        Some((20.0, 20_000.0)),
        None,
        0,
        "test",
    )
    .expect("single-take analysis");

    assert_eq!(capture.accepted_count, 1);
    assert_eq!(capture.rejected_count, 0);
    assert!(capture.quality.mean_coherence.is_none());
    assert!(
        capture
            .quality
            .missing_metrics
            .iter()
            .any(|m| m == "coherence")
    );
    assert!(
        !dir.path().join("L.take1.wav").exists(),
        "single-take captures write no raw-take siblings"
    );

    let header = std::fs::read_to_string(&csv).unwrap();
    let header = header.lines().next().unwrap().to_string();
    assert!(
        !header.split(',').any(|col| col == "coherence"),
        "no fabricated coherence: {header}"
    );
    assert!(
        header.ends_with(",noise_floor_db"),
        "real noise floor column present: {header}"
    );
    crate::signal_analysis::read_analysis_csv(&csv).expect("single-take CSV round-trip");
}

/// Log-frequency interpolation onto the CSV grid: log-linear between
/// bracketing points, endpoint clamping, DC-bracket fallback.
#[test]
#[cfg(not(target_os = "ios"))]
fn test_interpolate_log_frequency_grid() {
    let source_freqs = [0.0_f32, 100.0, 1_000.0, 10_000.0];
    let source_values = [9.0_f32, 0.0, 10.0, 20.0];
    // Midpoint in log frequency between 100 Hz and 1 kHz (~316 Hz) must
    // interpolate to the value midpoint (5), not the linear one.
    let target = [316.227_77_f32];
    let out = super::write::interpolate_log_frequency_grid(&source_freqs, &source_values, &target);
    assert!((out[0] - 5.0).abs() < 0.05, "log midpoint: {}", out[0]);
    // Endpoints clamp; exact grid points pass through.
    let out = super::write::interpolate_log_frequency_grid(
        &source_freqs,
        &source_values,
        &[50.0, 100.0, 10_000.0, 20_000.0],
    );
    assert_eq!(out, [0.0, 0.0, 20.0, 20.0]);
    // A target bracketed by the DC bin falls back to the upper sample.
    let out = super::write::interpolate_log_frequency_grid(&source_freqs, &source_values, &[50.0]);
    assert_eq!(out, [0.0]);
    // Empty source yields zeros.
    let out = super::write::interpolate_log_frequency_grid(&[], &[], &[100.0, 200.0]);
    assert_eq!(out, [0.0, 0.0]);
}

/// Diagnostic test: reads the real probe WAV from a recording session,
/// regenerates the narrowband probe, and runs cross-correlation per
/// channel to inspect arrival times, gains, and SNR. Reproduces the
/// analysis path from `probe_channel_delays_core` without live audio.
///
/// ```sh
/// cargo test -p sotf-engine --lib -- test_probe_wav_analysis --nocapture
/// ```
#[test]
fn test_probe_wav_analysis() {
    let wav_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data_generated/recording-20260413-163800/probe_all_channels.wav");
    if !wav_path.exists() {
        eprintln!("SKIP: probe WAV not found at {}", wav_path.display());
        return;
    }

    // Read mono probe recording
    let mut reader = WavReader::open(&wav_path).expect("failed to open probe WAV");
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    eprintln!(
        "WAV: {} ch, {} Hz, {} frames",
        spec.channels,
        sample_rate,
        reader.len() / spec.channels as u32
    );

    let recorded: Vec<f32> = if spec.sample_format == hound::SampleFormat::Float {
        reader.samples::<f32>().map(|s| s.unwrap()).collect()
    } else {
        reader
            .samples::<i32>()
            .map(|s| {
                let v = s.unwrap();
                v as f32 / (1_i64 << (spec.bits_per_sample - 1)) as f32
            })
            .collect()
    };

    let mono: Vec<f32> = if spec.channels > 1 {
        recorded
            .chunks(spec.channels as usize)
            .map(|frame| frame[0])
            .collect()
    } else {
        recorded
    };

    eprintln!(
        "Mono samples: {} ({:.3}s)",
        mono.len(),
        mono.len() as f64 / sample_rate as f64
    );

    // Probe parameters matching the recording session
    let probe_duration_ms = 1000.0_f32;
    let silence_duration_ms = 500.0_f32;
    let num_channels = 2_usize;

    let probe_samples = (probe_duration_ms / 1000.0 * sample_rate as f32) as usize;
    let silence_samples = (silence_duration_ms / 1000.0 * sample_rate as f32) as usize;
    let segment_len = silence_samples + probe_samples;

    let expected_offsets: Vec<usize> = (0..num_channels)
        .map(|i| silence_samples + i * segment_len)
        .collect();

    eprintln!(
        "Probe: {} samples ({:.1}ms), silence: {} samples ({:.1}ms)",
        probe_samples, probe_duration_ms, silence_samples, silence_duration_ms
    );
    eprintln!("Expected playback offsets: {:?}", expected_offsets);

    let probe = math_audio_dsp::signals::gen_narrowband_probe(
        probe_samples,
        sample_rate,
        0.5,
        PROBE_SEED,
        800.0,
        2000.0,
    );

    let auto_result =
        math_audio_dsp::analysis::cross_correlate_envelope(&probe, &probe, sample_rate)
            .expect("autocorrelation failed");
    let auto_peak = auto_result.peak_value as f64;
    eprintln!("Probe autocorrelation peak: {:.6}", auto_peak);

    // New approach: search from expected_offset, peak position within
    // the segment = system_latency + acoustic_propagation. System
    // latency cancels when computing alignment differences.
    eprintln!("\n=== Per-Channel Analysis (absolute-position, no system-latency step) ===");

    let channel_names = ["L", "R"];
    let mut arrivals = Vec::new();

    for (i, &expected) in expected_offsets.iter().enumerate() {
        let end = (expected + segment_len).min(mono.len());
        let segment = &mono[expected..end];

        let xcorr =
            math_audio_dsp::analysis::cross_correlate_envelope(&probe, segment, sample_rate)
                .expect("channel xcorr failed");

        let arrival_ms = xcorr.peak_sample_refined / sample_rate as f64 * 1000.0;

        let gain_linear = if auto_peak > 1e-10 {
            xcorr.peak_value as f64 / auto_peak
        } else {
            0.0
        };
        let gain_db = if gain_linear > 1e-10 {
            20.0 * gain_linear.log10()
        } else {
            -120.0
        };
        let mut sorted_env = xcorr.envelope.to_vec();
        sorted_env.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = sorted_env[sorted_env.len() / 2].max(1e-10) as f64;
        let snr_db = 20.0 * (xcorr.peak_value as f64 / median).log10();

        eprintln!(
            "\nCh {} '{}' (window [{}, {}]):",
            i, channel_names[i], expected, end
        );
        eprintln!(
            "  Arrival:  {:.3} ms (sample {:.1})",
            arrival_ms, xcorr.peak_sample_refined
        );
        eprintln!("  Gain:     {:.1} dB", gain_db);
        eprintln!("  SNR:      {:.1} dB", snr_db);

        arrivals.push(arrival_ms);
    }

    if !arrivals.is_empty() {
        let min = arrivals.iter().copied().fold(f64::INFINITY, f64::min);
        if min.is_finite() {
            for arrival in &mut arrivals {
                *arrival -= min;
            }
            eprintln!("\nNormalized arrivals by subtracting common latency floor: {min:.3} ms");
        }

        let max = arrivals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        eprintln!("\n=== Alignment Delays ===");
        for (i, name) in channel_names.iter().enumerate() {
            eprintln!("  {} — {:.3} ms", name, max - arrivals[i]);
        }
    }

    // Stored arrivals are relative acoustic offsets, not absolute
    // host/device latency.
    if let Some(min) = arrivals.iter().copied().reduce(f64::min) {
        assert!(
            min.abs() < 0.001,
            "normalized arrival floor should be 0ms, got {min:.3}ms"
        );
    }
}

#[test]
fn test_build_octave_sweep_with_silence_layout() {
    // The returned signal must have the structure:
    //   [pre_n zeros | sweep | post_n zeros]
    let sr = 48000_u32;
    let pre_s = 2.0_f32;
    let post_s = 1.5_f32;

    let out = build_octave_sweep_with_silence(10.0, 20_000.0, 0.5, 3.0, pre_s, post_s, sr);

    let pre_n = (pre_s * sr as f32).round() as usize;
    let post_n = (post_s * sr as f32).round() as usize;

    // Pre-silence window must be all zeros.
    for (i, &s) in out[..pre_n].iter().enumerate() {
        assert_eq!(s, 0.0, "Pre-silence sample {i} is not zero");
    }

    // Post-silence window must be all zeros.
    let post_start = out.len() - post_n;
    for (i, &s) in out[post_start..].iter().enumerate() {
        assert_eq!(s, 0.0, "Post-silence sample {i} is not zero");
    }

    // The swept middle section must contain non-zero energy.
    let sweep_region = &out[pre_n..post_start];
    let has_energy = sweep_region.iter().any(|&x| x.abs() > 1e-4);
    assert!(has_energy, "Sweep region contains no non-zero samples");
}

#[test]
fn test_sweep_params_from_config_octave_path() {
    let params = sweep_params_from_config(10.0, 20_000.0, 0.5, Some(3.0), Some(2.0), Some(1.5));
    match params {
        SignalParams::OctaveSweep {
            start_freq,
            end_freq,
            amp,
            bass_octave_duration_s,
            pre_silence_s,
            post_silence_s,
        } => {
            assert_eq!(start_freq, 10.0);
            assert_eq!(end_freq, 20_000.0);
            assert_eq!(amp, 0.5);
            assert_eq!(bass_octave_duration_s, 3.0);
            assert_eq!(pre_silence_s, 2.0);
            assert_eq!(post_silence_s, 1.5);
        }
        other => panic!("Expected OctaveSweep, got {other:?}"),
    }
}

#[test]
fn test_sweep_params_from_config_legacy_path() {
    // When bass_octave_duration_s is None, must fall back to legacy Sweep.
    let params = sweep_params_from_config(20.0, 20_000.0, 0.5, None, None, None);
    assert!(
        matches!(params, SignalParams::Sweep { .. }),
        "Expected legacy Sweep variant"
    );
}

#[test]
fn test_generate_signal_octave_sweep_variant() {
    // generate_signal must accept OctaveSweep params and return a non-empty signal.
    let params = SignalParams::OctaveSweep {
        start_freq: 20.0,
        end_freq: 20_000.0,
        amp: 0.5,
        bass_octave_duration_s: 3.0,
        pre_silence_s: 0.1,
        post_silence_s: 0.1,
    };
    let result = generate_signal(SignalType::Sweep, &params, 0.0, 48000);
    assert!(
        result.is_ok(),
        "generate_signal returned error: {:?}",
        result.err()
    );
    let signal = result.unwrap();
    assert!(!signal.is_empty(), "OctaveSweep signal should not be empty");
    // The signal must be longer than just the silence windows (sweep content present).
    let min_expected = (0.2_f32 * 48000_f32).round() as usize + 1;
    assert!(
        signal.len() > min_expected,
        "Signal too short: {} samples",
        signal.len()
    );
}

#[test]
fn test_octave_sweep_bass_clamp() {
    // Bass duration must be clamped to [1.0..10.0] in build_octave_sweep_with_silence.
    let sr = 48000_u32;
    // 0.1 clamped to 1.0.
    let out_clamped = build_octave_sweep_with_silence(20.0, 20_000.0, 0.5, 0.1, 0.0, 0.0, sr);
    // 1.0 explicit.
    let out_min = build_octave_sweep_with_silence(20.0, 20_000.0, 0.5, 1.0, 0.0, 0.0, sr);
    assert_eq!(
        out_clamped.len(),
        out_min.len(),
        "Clamped and min-1.0 lengths differ"
    );
}

#[test]
fn probe_direct_arrival_picker_prefers_weak_direct_over_late_reflection() {
    let sample_rate = 48_000_u32;
    let mut envelope = vec![1.0e-5_f32; sample_rate as usize / 2];
    let direct = (0.004 * sample_rate as f64).round() as usize;
    let reflection = (0.111 * sample_rate as f64).round() as usize;

    for offset in -4_i32..=4 {
        let idx = (direct as i32 + offset) as usize;
        envelope[idx] = 0.08 * (1.0 - offset.unsigned_abs() as f32 / 6.0);
    }
    for offset in -8_i32..=8 {
        let idx = (reflection as i32 + offset) as usize;
        envelope[idx] = 0.8 * (1.0 - offset.unsigned_abs() as f32 / 10.0);
    }

    let peak = pick_direct_arrival_from_envelope(&envelope, sample_rate, envelope.len())
        .expect("direct peak should be detected");
    let arrival_ms = peak.peak_sample_refined / sample_rate as f64 * 1000.0;

    assert!(
        (arrival_ms - 4.0).abs() < 0.2,
        "expected weak direct arrival near 4ms, got {arrival_ms:.3}ms"
    );
}

/// Build a synthetic multi-channel steady-state mic recording with
/// known per-channel phase shifts (sin-referenced) and run the
/// analysis path. Asserts circular-mean phase recovery to < 0.5°.
#[test]
fn bass_anchor_replay_recovers_known_phase_shifts() {
    use std::f32::consts::PI;

    let sample_rate = 48_000_u32;
    let bass_freq = 30.0_f32;
    let bass_duration_s = 1.0_f32;
    let fade_ms = 50.0_f32;
    let num_windows = 8_u16;
    let silence_ms = 500.0_f32;

    let channel_indices = vec![0_u16, 1, 2];
    let channel_names = vec!["L".to_string(), "R".to_string(), "Sub".to_string()];
    let injected_phases_deg = [0.0_f32, 30.0, -45.0];

    let tone_samples = (bass_duration_s * sample_rate as f32).round() as usize;
    let fade_n = ((fade_ms / 1000.0) * sample_rate as f32).round() as usize;
    let silence_samples = (silence_ms / 1000.0 * sample_rate as f32) as usize;
    let segment_len = silence_samples + tone_samples;
    let total_frames = silence_samples + channel_indices.len() * segment_len;

    let mut recorded = vec![0.0_f32; total_frames];
    let offsets: Vec<usize> = (0..channel_indices.len())
        .map(|i| silence_samples + i * segment_len)
        .collect();

    let omega = 2.0 * PI * bass_freq / sample_rate as f32;
    for (ch_i, &start) in offsets.iter().enumerate() {
        let phase_shift = injected_phases_deg[ch_i].to_radians();
        for k in 0..tone_samples {
            let env = if k < fade_n {
                0.5 * (1.0 - (PI * k as f32 / fade_n as f32).cos())
            } else if k >= tone_samples - fade_n {
                let kk = (tone_samples - 1 - k) as f32;
                0.5 * (1.0 - (PI * kk / fade_n as f32).cos())
            } else {
                1.0
            };
            // Use the GLOBAL sample index (start + k) so the
            // injected phase matches what `extract_tone_phase_windowed`
            // recovers — the helper anchors its sin/cos basis at
            // the recording's t = 0, not the start of the segment.
            let t = (start + k) as f32;
            recorded[start + k] = 0.5 * env * (omega * t + phase_shift).sin();
        }
    }

    let results = analyze_bass_anchor_recording(
        &recorded,
        None,
        &channel_names,
        &channel_indices,
        sample_rate,
        bass_freq,
        bass_duration_s,
        num_windows,
        &offsets,
        tone_samples,
    )
    .expect("analysis should succeed");

    assert_eq!(results.channels.len(), 3);
    assert_eq!(results.sample_rate, sample_rate);
    assert_eq!(results.bass_freq_hz, bass_freq);
    assert_eq!(results.bass_duration_s, bass_duration_s);

    for (i, cr) in results.channels.iter().enumerate() {
        let expected = injected_phases_deg[i] as f64;
        let got = cr.bass_anchor_phase_deg;
        let mut diff = got - expected;
        while diff > 180.0 {
            diff -= 360.0;
        }
        while diff <= -180.0 {
            diff += 360.0;
        }
        assert!(
            diff.abs() < 0.5,
            "Channel {} ({}): expected {:+.1}°, got {:+.2}° (err {:+.3}°)",
            i,
            cr.channel_name,
            expected,
            got,
            diff
        );
        assert!(cr.bass_anchor_magnitude > 0.0);
        assert!(
            cr.bass_anchor_stability_deg < 1.0,
            "Pure-sin steady tone should give near-zero circular-std, got {:.3}°",
            cr.bass_anchor_stability_deg
        );
        // No loopback recorded.
        assert!(cr.bass_anchor_loopback_phase_deg.is_none());
        assert!(cr.bass_anchor_coherence.is_none());
    }
}

/// Loopback subtraction: the mic phase carries a per-channel
/// acoustic shift PLUS a common source-side delay. The reported
/// phase must equal the per-channel shift alone (the loopback
/// cancels the common term).
#[test]
fn bass_anchor_replay_loopback_cancels_source_side_delay() {
    use std::f32::consts::PI;

    let sample_rate = 48_000_u32;
    let bass_freq = 30.0_f32;
    let bass_duration_s = 1.0_f32;
    let fade_ms = 50.0_f32;
    let num_windows = 8_u16;
    let silence_ms = 500.0_f32;

    let channel_indices = vec![0_u16, 1];
    let channel_names = vec!["L".to_string(), "R".to_string()];
    let acoustic_phase_deg = [10.0_f32, -20.0]; // what we want to recover
    let source_phase_deg = 70.0_f32; // common source-side shift

    let tone_samples = (bass_duration_s * sample_rate as f32).round() as usize;
    let fade_n = ((fade_ms / 1000.0) * sample_rate as f32).round() as usize;
    let silence_samples = (silence_ms / 1000.0 * sample_rate as f32) as usize;
    let segment_len = silence_samples + tone_samples;
    let total_frames = silence_samples + channel_indices.len() * segment_len;

    let omega = 2.0 * PI * bass_freq / sample_rate as f32;
    let envelope = |k: usize| -> f32 {
        if k < fade_n {
            0.5 * (1.0 - (PI * k as f32 / fade_n as f32).cos())
        } else if k >= tone_samples - fade_n {
            let kk = (tone_samples - 1 - k) as f32;
            0.5 * (1.0 - (PI * kk / fade_n as f32).cos())
        } else {
            1.0
        }
    };

    let offsets: Vec<usize> = (0..channel_indices.len())
        .map(|i| silence_samples + i * segment_len)
        .collect();

    let mut mic = vec![0.0_f32; total_frames];
    let mut loopback = vec![0.0_f32; total_frames];
    for (ch_i, &start) in offsets.iter().enumerate() {
        let mic_phase = (acoustic_phase_deg[ch_i] + source_phase_deg).to_radians();
        let lb_phase = source_phase_deg.to_radians();
        for k in 0..tone_samples {
            let env = envelope(k);
            let t = (start + k) as f32;
            mic[start + k] = 0.5 * env * (omega * t + mic_phase).sin();
            loopback[start + k] = 0.5 * env * (omega * t + lb_phase).sin();
        }
    }

    let results = analyze_bass_anchor_recording(
        &mic,
        Some(&loopback),
        &channel_names,
        &channel_indices,
        sample_rate,
        bass_freq,
        bass_duration_s,
        num_windows,
        &offsets,
        tone_samples,
    )
    .expect("analysis should succeed");

    for (i, cr) in results.channels.iter().enumerate() {
        let expected = acoustic_phase_deg[i] as f64;
        let mut diff = cr.bass_anchor_phase_deg - expected;
        while diff > 180.0 {
            diff -= 360.0;
        }
        while diff <= -180.0 {
            diff += 360.0;
        }
        assert!(
            diff.abs() < 0.5,
            "Loopback-corrected phase for {} should be {:+.1}°, got {:+.2}° (err {:+.3}°)",
            cr.channel_name,
            expected,
            cr.bass_anchor_phase_deg,
            diff
        );
        assert!(cr.bass_anchor_loopback_phase_deg.is_some());
        assert!(cr.bass_anchor_coherence.is_some());
        // With identical envelopes both per-window stds are ~0,
        // so combined stability also stays near zero.
        assert!(
            cr.bass_anchor_stability_deg < 1.0,
            "Combined std should stay near zero, got {:.3}°",
            cr.bass_anchor_stability_deg
        );
    }
}

#[test]
fn bass_anchor_replay_rejects_loopback_length_mismatch() {
    // Mic and loopback must have identical lengths — anything else
    // means the cpal callback dropped frames asymmetrically and we
    // refuse to analyse rather than emit per-channel garbage.
    let mic = vec![0.0_f32; 100_000];
    let loopback = vec![0.0_f32; 90_000]; // 10 % short
    let err = analyze_bass_anchor_recording(
        &mic,
        Some(&loopback),
        &["L".to_string()],
        &[0_u16],
        48_000,
        30.0,
        1.0,
        8,
        &[0_usize],
        48_000,
    )
    .expect_err("loopback length mismatch must fail");
    assert!(
        err.contains("Loopback length"),
        "error should mention loopback length mismatch, got: {err}"
    );
}

#[test]
fn bass_anchor_replay_rejects_length_mismatch() {
    let recorded = vec![0.0_f32; 100];
    let err = analyze_bass_anchor_recording(
        &recorded,
        None,
        &["L".to_string()],
        &[0_u16],
        48_000,
        30.0,
        1.0,
        8,
        &[0_usize, 50],
        48,
    )
    .expect_err("length mismatch must fail");
    assert!(
        err.contains("length mismatch"),
        "error should mention length mismatch, got: {err}"
    );
}

#[test]
fn bass_anchor_replay_errors_when_start_past_eof() {
    let recorded = vec![0.0_f32; 10];
    let err = analyze_bass_anchor_recording(
        &recorded,
        None,
        &["L".to_string()],
        &[0_u16],
        48_000,
        30.0,
        1.0,
        8,
        &[100_usize],
        48,
    )
    .expect_err("start past EOF must fail");
    assert!(err.contains("exceeds recording length"));
}

#[test]
fn test_validate_signal_params_two_tone() {
    let sample_rate = 48000;
    let nyquist = sample_rate as f32 / 2.0;

    // Valid
    let params = SignalParams::TwoTone {
        freq1: 100.0,
        amp1: 0.5,
        freq2: 1000.0,
        amp2: 0.3,
    };
    assert!(validate_signal_params(SignalType::TwoTone, &params, 1.0, sample_rate).is_ok());

    // freq1 at boundary
    let p = SignalParams::TwoTone {
        freq1: 0.0,
        amp1: 0.5,
        freq2: 1000.0,
        amp2: 0.3,
    };
    assert!(validate_signal_params(SignalType::TwoTone, &p, 1.0, sample_rate).is_err());
    let p = SignalParams::TwoTone {
        freq1: nyquist,
        amp1: 0.5,
        freq2: 1000.0,
        amp2: 0.3,
    };
    assert!(validate_signal_params(SignalType::TwoTone, &p, 1.0, sample_rate).is_err());

    // freq2 at boundary
    let p = SignalParams::TwoTone {
        freq1: 100.0,
        amp1: 0.5,
        freq2: 0.0,
        amp2: 0.3,
    };
    assert!(validate_signal_params(SignalType::TwoTone, &p, 1.0, sample_rate).is_err());
    let p = SignalParams::TwoTone {
        freq1: 100.0,
        amp1: 0.5,
        freq2: nyquist,
        amp2: 0.3,
    };
    assert!(validate_signal_params(SignalType::TwoTone, &p, 1.0, sample_rate).is_err());

    // amp1 at boundary
    let p = SignalParams::TwoTone {
        freq1: 100.0,
        amp1: 0.0,
        freq2: 1000.0,
        amp2: 0.3,
    };
    assert!(validate_signal_params(SignalType::TwoTone, &p, 1.0, sample_rate).is_err());
    let p = SignalParams::TwoTone {
        freq1: 100.0,
        amp1: 1.0,
        freq2: 1000.0,
        amp2: 0.3,
    };
    assert!(validate_signal_params(SignalType::TwoTone, &p, 1.0, sample_rate).is_ok());
    let p = SignalParams::TwoTone {
        freq1: 100.0,
        amp1: 1.1,
        freq2: 1000.0,
        amp2: 0.3,
    };
    assert!(validate_signal_params(SignalType::TwoTone, &p, 1.0, sample_rate).is_err());

    // amp2 at boundary
    let p = SignalParams::TwoTone {
        freq1: 100.0,
        amp1: 0.5,
        freq2: 1000.0,
        amp2: 0.0,
    };
    assert!(validate_signal_params(SignalType::TwoTone, &p, 1.0, sample_rate).is_err());
    let p = SignalParams::TwoTone {
        freq1: 100.0,
        amp1: 0.5,
        freq2: 1000.0,
        amp2: 1.0,
    };
    assert!(validate_signal_params(SignalType::TwoTone, &p, 1.0, sample_rate).is_ok());
}

#[test]
fn test_validate_signal_params_noise_amp() {
    let sample_rate = 48000;

    // Valid amplitude for all noise types
    let p = SignalParams::Noise { amp: 0.5 };
    assert!(validate_signal_params(SignalType::WhiteNoise, &p, 1.0, sample_rate).is_ok());
    assert!(validate_signal_params(SignalType::PinkNoise, &p, 1.0, sample_rate).is_ok());
    assert!(validate_signal_params(SignalType::MNoise, &p, 1.0, sample_rate).is_ok());

    // amp = 0 is invalid
    let p = SignalParams::Noise { amp: 0.0 };
    assert!(validate_signal_params(SignalType::WhiteNoise, &p, 1.0, sample_rate).is_err());

    // amp = 1.0 is valid
    let p = SignalParams::Noise { amp: 1.0 };
    assert!(validate_signal_params(SignalType::WhiteNoise, &p, 1.0, sample_rate).is_ok());

    // amp > 1.0 is invalid
    let p = SignalParams::Noise { amp: 1.1 };
    assert!(validate_signal_params(SignalType::WhiteNoise, &p, 1.0, sample_rate).is_err());
}

#[test]
fn test_validate_signal_params_sweep_edge_cases() {
    let sample_rate = 48000;
    let nyquist = sample_rate as f32 / 2.0;

    // start_freq = 0
    let p = SignalParams::Sweep {
        start_freq: 0.0,
        end_freq: 1000.0,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Sweep, &p, 1.0, sample_rate).is_err());

    // start_freq = nyquist
    let p = SignalParams::Sweep {
        start_freq: nyquist,
        end_freq: nyquist + 100.0,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Sweep, &p, 1.0, sample_rate).is_err());

    // end_freq = 0
    let p = SignalParams::Sweep {
        start_freq: 100.0,
        end_freq: 0.0,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Sweep, &p, 1.0, sample_rate).is_err());

    // end_freq = nyquist
    let p = SignalParams::Sweep {
        start_freq: 100.0,
        end_freq: nyquist,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Sweep, &p, 1.0, sample_rate).is_err());

    // amp = 0
    let p = SignalParams::Sweep {
        start_freq: 100.0,
        end_freq: 1000.0,
        amp: 0.0,
    };
    assert!(validate_signal_params(SignalType::Sweep, &p, 1.0, sample_rate).is_err());

    // amp = 1.0
    let p = SignalParams::Sweep {
        start_freq: 100.0,
        end_freq: 1000.0,
        amp: 1.0,
    };
    assert!(validate_signal_params(SignalType::Sweep, &p, 1.0, sample_rate).is_ok());
}

#[test]
fn test_validate_signal_params_tone_edge_cases() {
    let sample_rate = 48000;
    let nyquist = sample_rate as f32 / 2.0;

    // freq = 0
    let p = SignalParams::Tone {
        freq: 0.0,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Tone, &p, 1.0, sample_rate).is_err());

    // freq = nyquist
    let p = SignalParams::Tone {
        freq: nyquist,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Tone, &p, 1.0, sample_rate).is_err());

    // amp = 0
    let p = SignalParams::Tone {
        freq: 1000.0,
        amp: 0.0,
    };
    assert!(validate_signal_params(SignalType::Tone, &p, 1.0, sample_rate).is_err());

    // amp = 1.0
    let p = SignalParams::Tone {
        freq: 1000.0,
        amp: 1.0,
    };
    assert!(validate_signal_params(SignalType::Tone, &p, 1.0, sample_rate).is_ok());
}

#[test]
fn test_validate_signal_params_mls_order_boundary() {
    let sample_rate = 48000;

    // order = 2 (minimum valid)
    let p = SignalParams::Mls { order: 2, amp: 0.5 };
    assert!(validate_signal_params(SignalType::Mls, &p, 0.0, sample_rate).is_ok());

    // order = 24 (maximum valid)
    let p = SignalParams::Mls {
        order: 24,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Mls, &p, 0.0, sample_rate).is_ok());

    // order = 1 (too low)
    let p = SignalParams::Mls { order: 1, amp: 0.5 };
    assert!(validate_signal_params(SignalType::Mls, &p, 0.0, sample_rate).is_err());

    // order = 25 (too high)
    let p = SignalParams::Mls {
        order: 25,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Mls, &p, 0.0, sample_rate).is_err());
}

#[test]
fn test_validate_signal_params_dirac_edge_cases() {
    let sample_rate = 48000;

    // amp = 0
    let p = SignalParams::Dirac { amp: 0.0 };
    assert!(validate_signal_params(SignalType::Dirac, &p, 0.1, sample_rate).is_err());

    // amp = 1.0
    let p = SignalParams::Dirac { amp: 1.0 };
    assert!(validate_signal_params(SignalType::Dirac, &p, 0.1, sample_rate).is_ok());

    // amp > 1.0
    let p = SignalParams::Dirac { amp: 1.1 };
    assert!(validate_signal_params(SignalType::Dirac, &p, 0.1, sample_rate).is_err());
}

#[test]
fn test_validate_signal_params_octave_sweep_valid() {
    let sample_rate = 48000;

    // OctaveSweep used with Sweep signal type should pass through
    let p = SignalParams::OctaveSweep {
        start_freq: 20.0,
        end_freq: 20000.0,
        amp: 0.5,
        bass_octave_duration_s: 3.0,
        pre_silence_s: 2.0,
        post_silence_s: 2.0,
    };
    assert!(validate_signal_params(SignalType::Sweep, &p, 1.0, sample_rate).is_ok());
}

#[test]
fn test_validate_signal_params_octave_sweep_invalid() {
    // The UI-default octave-scaled sweep must go through the same Nyquist /
    // ordering / amplitude validation as the plain Sweep arm (Task 10) —
    // previously the `_ => {}` fallthrough made these no-ops.
    let mk = |start_freq: f32, end_freq: f32, amp: f32| SignalParams::OctaveSweep {
        start_freq,
        end_freq,
        amp,
        bass_octave_duration_s: 3.0,
        pre_silence_s: 2.0,
        post_silence_s: 2.0,
    };

    // 24 kHz end at 44.1 kHz exceeds Nyquist (22050 Hz) — clear message.
    let err = validate_signal_params(SignalType::Sweep, &mk(20.0, 24000.0, 0.5), 1.0, 44100)
        .expect_err("24 kHz end must fail at 44.1 kHz sample rate");
    assert!(
        err.contains("End frequency") && err.contains("22050"),
        "unexpected message: {err}"
    );

    // Reversed / zero-width range.
    assert!(
        validate_signal_params(SignalType::Sweep, &mk(2000.0, 1000.0, 0.5), 1.0, 48000).is_err()
    );
    assert!(
        validate_signal_params(SignalType::Sweep, &mk(1000.0, 1000.0, 0.5), 1.0, 48000).is_err()
    );

    // Out-of-range amplitude.
    assert!(
        validate_signal_params(SignalType::Sweep, &mk(20.0, 20000.0, 0.0), 1.0, 48000).is_err()
    );
    assert!(
        validate_signal_params(SignalType::Sweep, &mk(20.0, 20000.0, 1.5), 1.0, 48000).is_err()
    );

    // Zero sample rate must not silently pass.
    assert!(validate_signal_params(SignalType::Sweep, &mk(20.0, 20000.0, 0.5), 1.0, 0).is_err());

    // A valid configuration still passes.
    assert!(validate_signal_params(SignalType::Sweep, &mk(20.0, 20000.0, 0.5), 1.0, 48000).is_ok());
}

#[test]
fn test_validate_signal_params_mismatch_ok() {
    let sample_rate = 48000;

    // Mismatched signal type and params should fall through to Ok(())
    let p = SignalParams::Tone {
        freq: 1000.0,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Sweep, &p, 1.0, sample_rate).is_ok());

    let p = SignalParams::Sweep {
        start_freq: 100.0,
        end_freq: 1000.0,
        amp: 0.5,
    };
    assert!(validate_signal_params(SignalType::Tone, &p, 1.0, sample_rate).is_ok());
}

// --- Recording-pipeline hardening (reviews/20260818-recording.md R1, R3, R4, R8) ---

#[test]
fn measurement_level_db_clamps_to_full_scale() {
    // R3: hot levels must clamp to amplitude 1.0 instead of producing a
    // stimulus that gets hard-clipped sample-by-sample at the output.
    assert_eq!(measurement_amplitude_from_level_db(0.0), 1.0);
    assert_eq!(measurement_amplitude_from_level_db(6.0), 1.0);
    assert_eq!(measurement_amplitude_from_level_db(20.0), 1.0);
    assert!((measurement_amplitude_from_level_db(-6.0206) - 0.5).abs() < 1e-4);
    assert!((measurement_amplitude_from_level_db(-40.0) - 0.01).abs() < 1e-6);
    // Below the floor stays at the floor.
    assert_eq!(
        measurement_amplitude_from_level_db(-80.0),
        measurement_amplitude_from_level_db(-40.0)
    );
}

#[test]
fn analyze_clipping_counts_full_scale_samples() {
    // Empty buffer.
    let empty = analyze_clipping(&[]);
    assert_eq!(empty.clipped_samples, 0);
    assert_eq!(empty.clip_percent, 0.0);
    assert_eq!(empty.max_block_clip_percent, 0.0);

    // No clipping.
    let clean = analyze_clipping(&[0.5f32; 4096]);
    assert_eq!(clean.clipped_samples, 0);
    assert_eq!(clean.clip_percent, 0.0);

    // Exactly at the threshold counts as clipped (matches generator-side
    // clip() semantics: output hard-clamps at ±1.0).
    let mut buf = vec![0.0f32; CLIP_BLOCK_SAMPLES * 2];
    buf[10] = CLIP_THRESHOLD;
    buf[11] = -1.0;
    let stats = analyze_clipping(&buf);
    assert_eq!(stats.clipped_samples, 2);
    assert!((stats.clip_percent - 2.0 / buf.len() as f32 * 100.0).abs() < 1e-6);
    assert!(stats.max_block_clip_percent < 1.0);
}

#[test]
fn analyze_clipping_reports_worst_block() {
    // Second half fully clipped: 50% overall, 100% in the worst block.
    let mut buf = vec![0.0f32; CLIP_BLOCK_SAMPLES * 2];
    buf[CLIP_BLOCK_SAMPLES..].fill(1.0);
    let stats = analyze_clipping(&buf);
    assert_eq!(stats.clipped_samples, CLIP_BLOCK_SAMPLES);
    assert!((stats.clip_percent - 50.0).abs() < 1e-6);
    assert!((stats.max_block_clip_percent - 100.0).abs() < 1e-6);
}

#[cfg(not(target_os = "ios"))]
#[test]
fn check_capture_clipping_aborts_only_on_heavily_clipped_block() {
    // >30% of one block clipped → Err (REW abort rule).
    let mut buf = vec![0.0f32; CLIP_BLOCK_SAMPLES];
    buf[..CLIP_BLOCK_SAMPLES / 2].fill(1.0);
    let err = check_capture_clipping(&buf, "test").unwrap_err();
    assert!(err.contains("clipped"), "unexpected error: {err}");

    // ~1% overall, spread out so no block exceeds 30% → warn-only Ok.
    let mut buf = vec![0.0f32; CLIP_BLOCK_SAMPLES * 10];
    for i in 0..CLIP_BLOCK_SAMPLES / 10 {
        buf[i * 10] = 1.0;
    }
    assert!(check_capture_clipping(&buf, "test").is_ok());

    // Fully clean → Ok, no warning.
    assert!(check_capture_clipping(&[0.25f32; 4096], "test").is_ok());
}

#[cfg(not(target_os = "ios"))]
#[test]
fn check_capture_clipping_ignores_undersized_tail_block() {
    // Task 10: a capture whose length is ≡ small mod CLIP_BLOCK_SAMPLES with
    // clipped samples only in the tiny tail block must not hard-fail — with
    // a 2-sample tail, one clipped sample is 50% of the block and would
    // otherwise trip the 30% abort rule (false positive).
    let mut buf = vec![0.0f32; CLIP_BLOCK_SAMPLES * 3 + 2];
    *buf.last_mut().unwrap() = 1.0;
    assert!(
        check_capture_clipping(&buf, "test").is_ok(),
        "one clipped sample in a 2-sample tail block must not abort"
    );
    let stats = analyze_clipping(&buf);
    assert_eq!(stats.clipped_samples, 1);
    assert_eq!(stats.max_block_clip_percent, 0.0);

    // A heavily clipped tail is still caught when the tail is large enough
    // to be statistically meaningful (>= CLIP_BLOCK_SAMPLES / 4).
    let mut buf = vec![0.0f32; CLIP_BLOCK_SAMPLES * 2 + CLIP_BLOCK_SAMPLES / 2];
    buf[CLIP_BLOCK_SAMPLES * 2..].fill(1.0);
    assert!(check_capture_clipping(&buf, "test").is_err());
}

#[test]
fn prepare_measurement_signal_validates_generates_and_pads() {
    let sample_rate = 48_000;
    let params = SignalParams::Sweep {
        start_freq: 20.0,
        end_freq: 20_000.0,
        amp: 0.5,
    };
    let prepared =
        prepare_measurement_signal(SignalType::Sweep, &params, 1.0, sample_rate).unwrap();
    let raw = generate_signal(SignalType::Sweep, &params, 1.0, sample_rate).unwrap();
    let padding = (0.25 * sample_rate as f32) as usize; // prepare_signal: 250 ms each side
    assert_eq!(prepared.len(), raw.len() + 2 * padding);
    assert!(prepared[..padding].iter().all(|&s| s == 0.0));
    assert!(prepared[padding + raw.len()..].iter().all(|&s| s == 0.0));

    // Invalid params (start >= Nyquist, start >= end) fail before generation.
    let bad = SignalParams::Sweep {
        start_freq: 30_000.0,
        end_freq: 20_000.0,
        amp: 0.5,
    };
    assert!(prepare_measurement_signal(SignalType::Sweep, &bad, 1.0, sample_rate).is_err());
    let bad_amp = SignalParams::Noise { amp: 2.0 };
    assert!(
        prepare_measurement_signal(SignalType::WhiteNoise, &bad_amp, 1.0, sample_rate).is_err()
    );
}

#[cfg(not(target_os = "ios"))]
#[test]
fn record_and_analyze_honors_pre_set_cancel_flag() {
    // A flag that is already set must abort before any audio device is
    // touched, so this test needs no hardware.
    let dir = tempdir().unwrap();
    let temp_wav = dir.path().join("temp.wav");
    let recorded_wav = dir.path().join("recorded.wav");
    let csv = dir.path().join("analysis.csv");
    let flag = CancelFlag::default();
    flag.store(true, Ordering::Relaxed);

    let err = record_and_analyze(
        &temp_wav,
        &recorded_wav,
        &[0.0f32; 4800],
        48000,
        &csv,
        0,
        0,
        None,
        None,
        None,
        None,
        1, // num_sweeps
        Some(flag),
    )
    .unwrap_err();
    assert_eq!(err, CANCELLED_ERR);
    assert!(!recorded_wav.exists());
}

#[cfg(not(target_os = "ios"))]
#[test]
fn record_and_analyze_multi_honors_pre_set_cancel_flag() {
    let dir = tempdir().unwrap();
    let temp_wav = dir.path().join("temp.wav");
    let wav_paths = vec![dir.path().join("recorded.wav")];
    let csv_paths = vec![dir.path().join("analysis.csv")];
    let mic_calibrations = vec![None];
    let flag = CancelFlag::default();
    flag.store(true, Ordering::Relaxed);

    let err = record_and_analyze_multi(
        &temp_wav,
        &wav_paths,
        &[0.0f32; 4800],
        48000,
        &csv_paths,
        0,
        &[0],
        None,
        None,
        &mic_calibrations,
        None,
        1, // num_sweeps
        Some(flag),
    )
    .unwrap_err();
    assert_eq!(err, CANCELLED_ERR);
    assert!(!wav_paths[0].exists());
}

#[test]
fn actionable_capture_error_maps_permission_denial() {
    let msg = actionable_capture_error(
        "[test] Failed to build input stream",
        &"PermissionDenied: microphone access not authorized",
    );
    #[cfg(target_os = "macos")]
    assert!(
        msg.contains("System Settings") && msg.contains("Microphone"),
        "macOS permission advice: {msg}"
    );
    #[cfg(target_os = "linux")]
    assert!(
        msg.contains("'audio' group"),
        "Linux permission advice: {msg}"
    );
    #[cfg(target_os = "ios")]
    assert!(
        msg.contains("Privacy & Security"),
        "iOS permission advice: {msg}"
    );
    #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "linux")))]
    assert!(msg.contains("privacy"), "generic permission advice: {msg}");
    // Original error text is kept for debugging.
    assert!(msg.contains("original error"), "{msg}");
    assert!(msg.contains("PermissionDenied"), "{msg}");
}

#[test]
fn actionable_capture_error_maps_busy_device() {
    let msg = actionable_capture_error(
        "[test] Failed to start input stream",
        &"ALSA: Device or resource busy",
    );
    assert!(msg.contains("busy"), "{msg}");
    assert!(msg.contains("close other apps"), "{msg}");
    assert!(msg.contains("Device or resource busy"), "{msg}");
}

#[test]
fn actionable_capture_error_maps_missing_device() {
    let msg = actionable_capture_error(
        "[test] Input device not usable",
        &"Audio device 'UMIK-1' not found. Available input devices (1 total): Built-in Microphone",
    );
    assert!(msg.contains("--list-devices"), "{msg}");
    assert!(msg.contains("not found"), "{msg}");

    let no_default = actionable_capture_error("[test]", &"No default input device available");
    assert!(no_default.contains("--list-devices"), "{no_default}");
}

#[test]
fn actionable_capture_error_passes_through_unclassified_errors() {
    let msg = actionable_capture_error("[test] Failed to load file", &"no such file: temp.wav");
    assert_eq!(msg, "[test] Failed to load file: no such file: temp.wav");
    assert!(!msg.contains("original error"), "{msg}");
}
