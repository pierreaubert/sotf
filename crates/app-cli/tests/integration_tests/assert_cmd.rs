//! `assert_cmd`/`predicates` integration tests for the `app-cli` binaries.
//!
//! These tests exercise the public CLI surface of `player-cli` and
//! `sotf-recorder-cli` as a black box, checking exit codes, stdout and stderr.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;
use std::time::Duration;

/// Maximum time any CLI integration test is allowed to run before being killed.
///
/// The `play_*_parses` tests exercise argument parsing and plugin wiring only;
/// they do not require real audio hardware. A short timeout prevents CI runners
/// from hanging indefinitely when cpal cannot open the default device.
const TEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Build a `player-cli` command with logging redirected away from the crate
/// directory so tests do not leave `sotf_cli_player.log` behind.
fn player_cmd() -> Command {
    let mut cmd = Command::cargo_bin("player-cli").expect("player-cli binary available");
    cmd.env("SOTF_CLI_LOG", "/dev/null");
    cmd.timeout(TEST_TIMEOUT);
    cmd
}

fn recorder_cmd() -> Command {
    let mut cmd =
        Command::cargo_bin("sotf-recorder-cli").expect("sotf-recorder-cli binary available");
    cmd.timeout(TEST_TIMEOUT);
    // Redirect generated WAV/CSV files away from the source tree.
    let output_dir = std::env::var("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("sotf-recorder-tests"));
    cmd.arg("--output-dir").arg(output_dir);
    cmd
}

fn fixture_wav() -> PathBuf {
    std::env::var("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("sotf-recorder-tests"))
        .join("tone_send0_rec0_48000.wav")
}

/// Generate a deterministic mono tone WAV fixture under the target temp dir.
fn ensure_fixture_wav() {
    use sotf_audio::signal_recorder::{SignalParams, SignalType, generate_signal, write_wav_file};

    let path = fixture_wav();
    if path.is_file() {
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let signal = generate_signal(
        SignalType::Tone,
        &SignalParams::Tone {
            freq: 1000.0,
            amp: 0.5,
        },
        0.5,
        48000,
    )
    .expect("failed to generate tone signal");
    write_wav_file(&path, &signal, 48000, 1).expect("failed to write fixture WAV");
}

/// Seed a temporary library database with a small, predictable set of albums
/// and tracks for the `library`/`status` CLI tests.
fn seed_library_db(db_path: &std::path::Path) {
    use sotf_audio_player::{Album, MusicLibrary, Track};

    let mut lib = MusicLibrary::with_custom_database_for_testing(db_path)
        .expect("failed to open test library database");

    let albums = vec![
        Album {
            title: "Seeded Album One".to_string(),
            tracks: vec![
                Track {
                    path: std::path::PathBuf::from("/tmp/sotf-seed/track1.wav"),
                    title: Some("First Track".to_string()),
                    artist: Some("Seeded Artist A".to_string()),
                    album_artist: Some("Seeded Artist A".to_string()),
                    duration_secs: Some(180),
                    ..Default::default()
                },
                Track {
                    path: std::path::PathBuf::from("/tmp/sotf-seed/track2.wav"),
                    title: Some("Second Track".to_string()),
                    artist: Some("Seeded Artist A".to_string()),
                    album_artist: Some("Seeded Artist A".to_string()),
                    duration_secs: Some(200),
                    ..Default::default()
                },
            ],
            ..Default::default()
        },
        Album {
            title: "Seeded Album Two".to_string(),
            tracks: vec![Track {
                path: std::path::PathBuf::from("/tmp/sotf-seed/track3.wav"),
                title: Some("Third Track".to_string()),
                artist: Some("Seeded Artist B".to_string()),
                album_artist: Some("Seeded Artist B".to_string()),
                duration_secs: Some(240),
                ..Default::default()
            }],
            ..Default::default()
        },
    ];

    if let Some(db) = lib.get_database_mut() {
        db.save_albums(&albums).expect("failed to seed albums");
    }
}

// ---------------------------------------------------------------------------
// player-cli: help / version / device enumeration
// ---------------------------------------------------------------------------

#[test]
fn player_cli_help_shows_usage() {
    player_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Audio player"));
}

#[sotf_test::requires_hardware]
#[test]
fn player_cli_devices_lists_audio_devices() {
    player_cmd().arg("devices").assert().success().stdout(
        predicate::str::contains("Enumerating audio devices")
            .or(predicate::str::contains("Input Devices")),
    );
}

// ---------------------------------------------------------------------------
// player-cli: replay-gain analysis
// ---------------------------------------------------------------------------

#[test]
fn player_cli_replay_gain_analyzes_local_wav() {
    ensure_fixture_wav();
    let wav = fixture_wav();
    assert!(wav.is_file(), "fixture WAV should be generated at {}", wav.display());

    player_cmd()
        .args(["replay-gain", wav.to_str().unwrap()])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("ReplayGain")
                .and(predicate::str::contains("Gain"))
                .and(predicate::str::contains("Peak")),
        );
}

#[test]
fn player_cli_replay_gain_missing_file_fails() {
    player_cmd()
        .args(["replay-gain", "/nonexistent/no_such_file.wav"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error"));
}

// ---------------------------------------------------------------------------
// player-cli: play subcommand argument parsing / plugin wiring
//
// We do not require playback to succeed: CI runners may have no audio device.
// The important public behaviour is that arguments parse and the binary does
// not panic or emit clap "unexpected argument" errors.
// ---------------------------------------------------------------------------

fn assert_no_clap_panic(output: &std::process::Output) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument")
            && !stderr.contains("Found argument")
            && !stderr.contains("panicked")
            && !stderr.contains("assertion failed"),
        "command-line parsing or runtime panic: {stderr}"
    );
}

#[test]
fn player_cli_play_local_wav_parses() {
    let wav = fixture_wav();
    if !wav.is_file() {
        eprintln!("skipping: fixture WAV not found at {}", wav.display());
        return;
    }

    let output = player_cmd()
        .args(["play", wav.to_str().unwrap(), "--duration", "1"])
        .output()
        .expect("failed to spawn player-cli play");

    assert_no_clap_panic(&output);
    // If the file loaded, the binary prints a summary to stdout.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success() || stdout.contains("Loaded audio file"),
        "expected file load summary on success, got stdout: {stdout}"
    );
}

#[test]
fn player_cli_play_with_eq_filter_parses() {
    let wav = fixture_wav();
    if !wav.is_file() {
        eprintln!("skipping: fixture WAV not found at {}", wav.display());
        return;
    }

    let output = player_cmd()
        .args([
            "play",
            wav.to_str().unwrap(),
            "--duration",
            "1",
            "--filter",
            "1000:1.5:3.0",
        ])
        .output()
        .expect("failed to spawn player-cli play");

    assert_no_clap_panic(&output);
}

#[test]
fn player_cli_play_with_filter_type_parses() {
    let wav = fixture_wav();
    if !wav.is_file() {
        eprintln!("skipping: fixture WAV not found at {}", wav.display());
        return;
    }

    let output = player_cmd()
        .args([
            "play",
            wav.to_str().unwrap(),
            "--duration",
            "1",
            "--filter",
            "LS:100:0.7:-2.0",
            "--filter",
            "HP:80:0.707:0",
        ])
        .output()
        .expect("failed to spawn player-cli play");

    assert_no_clap_panic(&output);
}

#[test]
fn player_cli_play_invalid_filter_format_fails() {
    let wav = fixture_wav();
    if !wav.is_file() {
        eprintln!("skipping: fixture WAV not found at {}", wav.display());
        return;
    }

    player_cmd()
        .args([
            "play",
            wav.to_str().unwrap(),
            "--duration",
            "1",
            "--filter",
            "totally-invalid",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error parsing filters"));
}

#[test]
fn player_cli_play_with_lufs_flag_parses() {
    let wav = fixture_wav();
    if !wav.is_file() {
        eprintln!("skipping: fixture WAV not found at {}", wav.display());
        return;
    }

    let output = player_cmd()
        .args(["play", wav.to_str().unwrap(), "--duration", "1", "--lufs"])
        .output()
        .expect("failed to spawn player-cli play");

    assert_no_clap_panic(&output);
}

#[test]
fn player_cli_play_with_loudness_compensation_parses() {
    let wav = fixture_wav();
    if !wav.is_file() {
        eprintln!("skipping: fixture WAV not found at {}", wav.display());
        return;
    }

    let output = player_cmd()
        .args([
            "play",
            wav.to_str().unwrap(),
            "--duration",
            "1",
            "--loudness-compensation=-20,10",
        ])
        .output()
        .expect("failed to spawn player-cli play");

    assert_no_clap_panic(&output);
}

#[test]
fn player_cli_play_invalid_loudness_compensation_fails() {
    let wav = fixture_wav();
    if !wav.is_file() {
        eprintln!("skipping: fixture WAV not found at {}", wav.display());
        return;
    }

    player_cmd()
        .args([
            "play",
            wav.to_str().unwrap(),
            "--duration",
            "1",
            "--loudness-compensation",
            "1",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--loudness-compensation"));
}

#[test]
fn player_cli_play_with_rack_arg_parses() {
    let wav = fixture_wav();
    if !wav.is_file() {
        eprintln!("skipping: fixture WAV not found at {}", wav.display());
        return;
    }

    let output = player_cmd()
        .args([
            "play",
            wav.to_str().unwrap(),
            "--duration",
            "1",
            "--rack",
            "eq,lufs",
        ])
        .output()
        .expect("failed to spawn player-cli play");

    assert_no_clap_panic(&output);
}

#[test]
fn player_cli_play_with_upmixer_args_parses() {
    let wav = fixture_wav();
    if !wav.is_file() {
        eprintln!("skipping: fixture WAV not found at {}", wav.display());
        return;
    }

    let output = player_cmd()
        .args([
            "play",
            wav.to_str().unwrap(),
            "--duration",
            "1",
            "--upmixer",
            "--upmixer-config",
            "2.0",
            "--upmixer-stereo-width",
            "0.8",
        ])
        .output()
        .expect("failed to spawn player-cli play");

    assert_no_clap_panic(&output);
}

#[test]
fn player_cli_play_with_compressor_args_parses() {
    let wav = fixture_wav();
    if !wav.is_file() {
        eprintln!("skipping: fixture WAV not found at {}", wav.display());
        return;
    }

    let output = player_cmd()
        .args([
            "play",
            wav.to_str().unwrap(),
            "--duration",
            "1",
            "--compressor",
            "--compressor-threshold-db=-20.0",
            "--compressor-ratio",
            "4.0",
        ])
        .output()
        .expect("failed to spawn player-cli play");

    assert_no_clap_panic(&output);
}

#[test]
fn player_cli_play_with_crossfeed_args_parses() {
    let wav = fixture_wav();
    if !wav.is_file() {
        eprintln!("skipping: fixture WAV not found at {}", wav.display());
        return;
    }

    let output = player_cmd()
        .args([
            "play",
            wav.to_str().unwrap(),
            "--duration",
            "1",
            "--crossfeed",
            "--crossfeed-mode",
            "meier",
        ])
        .output()
        .expect("failed to spawn player-cli play");

    assert_no_clap_panic(&output);
}

#[test]
fn player_cli_status_runs_without_panicking() {
    let output = player_cmd()
        .arg("status")
        .output()
        .expect("failed to spawn player-cli status");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("panicked") && !stderr.contains("assertion failed"),
        "status subcommand panicked: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// player-cli: library / status commands (temp DB)
// ---------------------------------------------------------------------------

#[test]
fn player_cli_library_reports_empty_db() {
    let (_dir, db_path) = sotf_testkit::db::temp_sqlite_db();

    player_cmd()
        .args(["library", "--db", db_path.to_str().unwrap(), "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Tracks: 0"))
        .stdout(predicate::str::contains("Albums: 0"))
        .stdout(predicate::str::contains("Artists: 0"))
        .stdout(predicate::str::contains("Total duration: 0s"));
}

#[test]
fn player_cli_library_lists_seeded_db() {
    let (_dir, db_path) = sotf_testkit::db::temp_sqlite_db();
    seed_library_db(&db_path);

    player_cmd()
        .args(["library", "--db", db_path.to_str().unwrap(), "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Tracks: 3"))
        .stdout(predicate::str::contains("Albums: 2"))
        .stdout(predicate::str::contains("Artists: 2"))
        .stdout(predicate::str::contains("Total duration: 620s"));
}

#[test]
fn player_cli_library_search_finds_matches() {
    let (_dir, db_path) = sotf_testkit::db::temp_sqlite_db();
    seed_library_db(&db_path);

    player_cmd()
        .args(["library", "--db", db_path.to_str().unwrap(), "search", "Seeded Artist A"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Matching album IDs:"));
}

#[test]
fn player_cli_status_reports_summary() {
    let (_dir, db_path) = sotf_testkit::db::temp_sqlite_db();
    seed_library_db(&db_path);

    player_cmd()
        .args(["status", "--db", db_path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("3 tracks"))
        .stdout(predicate::str::contains("2 albums"))
        .stdout(predicate::str::contains("2 artists"))
        .stdout(predicate::str::contains("620s total"));
}

// ---------------------------------------------------------------------------
// sotf-recorder-cli: help / version / device enumeration
// ---------------------------------------------------------------------------

#[test]
fn recorder_cli_help_shows_usage() {
    recorder_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Generate and record test signals with analysis",
        ));
}

#[sotf_test::requires_hardware]
#[test]
fn recorder_cli_list_devices_shows_devices() {
    recorder_cmd()
        .arg("--list-devices")
        .assert()
        .success()
        .stdout(predicate::str::contains("Available Audio Devices"));
}

// ---------------------------------------------------------------------------
// sotf-recorder-cli: argument validation
// ---------------------------------------------------------------------------

#[test]
fn recorder_cli_missing_required_args_fails() {
    recorder_cmd()
        .args(["--signal", "tone"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn recorder_cli_invalid_signal_type_fails() {
    recorder_cmd()
        .args([
            "--signal",
            "not-a-signal",
            "--duration",
            "0.5",
            "--hwaudio-send-to",
            "0",
            "--hwaudio-record-from",
            "0",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error"));
}

#[test]
fn recorder_cli_invalid_channel_config_fails() {
    recorder_cmd()
        .args([
            "--signal",
            "tone",
            "--freq",
            "1000",
            "--duration",
            "1",
            "--hwaudio-send-to",
            "0,1",
            "--hwaudio-record-from",
            "0,1,2",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid channel configuration"));
}

#[test]
fn recorder_cli_channels_must_be_one() {
    recorder_cmd()
        .args([
            "--signal",
            "tone",
            "--freq",
            "1000",
            "--duration",
            "0.5",
            "--channels",
            "2",
            "--hwaudio-send-to",
            "0",
            "--hwaudio-record-from",
            "0",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Channels must be 1"));
}

#[test]
fn recorder_cli_invalid_mic_calibration_format_fails() {
    recorder_cmd()
        .args([
            "--signal",
            "tone",
            "--freq",
            "1000",
            "--duration",
            "0.5",
            "--hwaudio-send-to",
            "0",
            "--hwaudio-record-from",
            "0",
            "--mic-calibration",
            "bad-format",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("CHANNEL:PATH"));
}

// ---------------------------------------------------------------------------
// sotf-recorder-cli: signal dry runs
//
// Playback/recording may fail without real audio hardware, but the binary
// should still parse arguments and print the configuration header.
// ---------------------------------------------------------------------------

fn assert_recorder_config_header(args: &[&str]) {
    let output = recorder_cmd()
        .args(args)
        .output()
        .expect("failed to spawn sotf-recorder-cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Signal Recording and Analysis") || stdout.contains("Configuration:"),
        "expected config header in stdout, got: {stdout}"
    );
}

#[test]
fn recorder_cli_tone_signal_dry_run() {
    assert_recorder_config_header(&[
        "--signal",
        "tone",
        "--freq",
        "1000",
        "--duration",
        "0.5",
        "--hwaudio-send-to",
        "0",
        "--hwaudio-record-from",
        "0",
    ]);
}

#[test]
fn recorder_cli_two_tone_signal_dry_run() {
    assert_recorder_config_header(&[
        "--signal",
        "two-tone",
        "--freq1",
        "440",
        "--freq2",
        "880",
        "--duration",
        "0.5",
        "--hwaudio-send-to",
        "0",
        "--hwaudio-record-from",
        "0",
    ]);
}

#[test]
fn recorder_cli_sweep_signal_dry_run() {
    assert_recorder_config_header(&[
        "--signal",
        "sweep",
        "--start-freq",
        "20",
        "--end-freq",
        "20000",
        "--duration",
        "0.5",
        "--hwaudio-send-to",
        "0",
        "--hwaudio-record-from",
        "0",
    ]);
}

#[test]
fn recorder_cli_pink_noise_signal_dry_run() {
    assert_recorder_config_header(&[
        "--signal",
        "pink-noise",
        "--duration",
        "0.5",
        "--hwaudio-send-to",
        "0",
        "--hwaudio-record-from",
        "0",
    ]);
}

#[test]
fn recorder_cli_custom_sample_rate_parses() {
    assert_recorder_config_header(&[
        "--signal",
        "tone",
        "--freq",
        "1000",
        "--duration",
        "0.5",
        "--sample-rate",
        "44100",
        "--hwaudio-send-to",
        "0",
        "--hwaudio-record-from",
        "0",
    ]);
}

// ---------------------------------------------------------------------------
// WAV property assertions on a generated fixture
// ---------------------------------------------------------------------------

#[test]
fn recorder_output_wav_has_expected_properties() {
    use sotf_audio::signal_recorder::{SignalParams, SignalType, generate_signal, write_wav_file};

    let tmp = std::env::var("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("sotf-recorder-tests"));
    let _ = std::fs::create_dir_all(&tmp);
    let wav_path = tmp.join("phase3_2_generated_48000_float.wav");

    let duration_s = 0.5f32;
    let sample_rate = 48000u32;
    let channels = 1u16;
    let signal = generate_signal(
        SignalType::Tone,
        &SignalParams::Tone {
            freq: 1000.0,
            amp: 0.5,
        },
        duration_s,
        sample_rate,
    )
    .expect("failed to generate tone signal");

    write_wav_file(&wav_path, &signal, sample_rate, channels).expect("failed to write WAV file");

    let header = std::fs::read(&wav_path).expect("failed to read generated WAV");
    assert!(
        header.len() >= 44,
        "generated WAV is too short to contain a standard header"
    );

    // RIFF / WAVE magic
    assert_eq!(&header[0..4], b"RIFF");
    assert_eq!(&header[8..12], b"WAVE");
    assert_eq!(&header[12..16], b"fmt ");

    // Little-endian helpers
    let u16_le = |offset: usize| u16::from_le_bytes([header[offset], header[offset + 1]]);
    let u32_le = |offset: usize| {
        u32::from_le_bytes([
            header[offset],
            header[offset + 1],
            header[offset + 2],
            header[offset + 3],
        ])
    };

    let fmt_chunk_size = u32_le(16);
    let fmt_tag = u16_le(20);
    let channel_count = u16_le(22);
    let header_sample_rate = u32_le(24);
    let bits_per_sample = u16_le(34);

    // hound writes 32-bit float as either WAVE_FORMAT_IEEE_FLOAT (3) or
    // WAVE_FORMAT_EXTENSIBLE (0xFFFE) with a KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
    // sub-format GUID. Both mean 32-bit float samples.
    let is_float = fmt_tag == 0x0003
        || (fmt_tag == 0xFFFE
            && header.len() >= 46
            && u16_le(44) == 0x0003);
    assert!(is_float, "expected 32-bit float format (tag {fmt_tag})");
    assert_eq!(channel_count, channels, "channel count mismatch");
    assert_eq!(header_sample_rate, sample_rate, "sample rate mismatch");
    assert_eq!(bits_per_sample, 32, "expected 32-bit samples");

    // Find the "data" chunk after the fmt chunk (and any extension bytes).
    let data_offset = 12 + 8 + fmt_chunk_size as usize;
    assert!(
        header.len() >= data_offset + 8,
        "WAV header too short for data chunk"
    );
    assert_eq!(&header[data_offset..data_offset + 4], b"data");
    let data_chunk_size = u32_le(data_offset + 4);
    let expected_samples = (duration_s * sample_rate as f32) as u32;
    let expected_data_size = expected_samples * channel_count as u32 * 4;
    assert_eq!(
        data_chunk_size, expected_data_size,
        "data chunk size inconsistent with duration"
    );

    // RIFF chunk size should be file size minus the 8-byte RIFF header.
    let riff_chunk_size = u32_le(4);
    let expected_file_size = 8 + riff_chunk_size;
    let actual_file_size = std::fs::metadata(&wav_path)
        .expect("failed to stat WAV file")
        .len() as u32;
    assert_eq!(
        actual_file_size, expected_file_size,
        "RIFF chunk size inconsistent with file size"
    );
}
