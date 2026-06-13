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
    cmd
}

fn fixture_wav() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tone_send0_rec0_48000.wav")
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
    let wav = fixture_wav();
    if !wav.is_file() {
        eprintln!("skipping: fixture WAV not found at {}", wav.display());
        return;
    }

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
