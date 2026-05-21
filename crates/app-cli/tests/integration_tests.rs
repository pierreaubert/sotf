//! Integration tests for app-cli binaries.
//!
//! These tests invoke the actual `player-cli` and `sotf-recorder-cli`
//! binaries and assert on their exit codes and stdout/stderr output.
//! They require the binaries to be built first (via `cargo build -p app-cli`).

use std::path::PathBuf;
use std::process::Command;

fn cargo_bin(name: &str) -> PathBuf {
    // cargo test sets CARGO_BIN_EXE_<name> when the binary is a target of the same workspace
    let env_var = format!("CARGO_BIN_EXE_{}", name.replace('-', "_").to_uppercase());
    std::env::var(&env_var)
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            // Fallback for running tests directly
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/debug")
                .join(name)
        })
}

fn demo_audio_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../app-gpui/assets/demo-audio")
        .join(name)
}

// ============================================================================
// player-cli
// ============================================================================

#[test]
fn player_cli_devices_lists_audio_devices() {
    let output = Command::new(cargo_bin("player-cli"))
        .args(["devices"])
        .output()
        .expect("failed to spawn player-cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "player-cli devices exited with {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("Enumerating audio devices") || stdout.contains("Available Audio Devices"),
        "expected device list header in stdout, got: {stdout}"
    );
}

#[test]
fn player_cli_replay_gain_analyzes_demo_file() {
    let demo = demo_audio_path("classical.wav");
    if !demo.is_file() {
        eprintln!("skipping: demo audio not found at {}", demo.display());
        return;
    }

    let output = Command::new(cargo_bin("player-cli"))
        .args(["replay-gain", demo.to_str().unwrap()])
        .output()
        .expect("failed to spawn player-cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "player-cli replay-gain exited with {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("ReplayGain") || stdout.contains("gain") || stdout.contains("dB"),
        "expected ReplayGain data in stdout, got: {stdout}"
    );
}

#[test]
#[cfg(not(debug_assertions))]
fn player_cli_play_invalid_file_fails() {
    let output = Command::new(cargo_bin("player-cli"))
        .args(["play", "/nonexistent/file.wav"])
        .output()
        .expect("failed to spawn player-cli");

    assert!(
        !output.status.success(),
        "expected player-cli play to fail for nonexistent file"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Error") || stdout_contains(&output, "Error"),
        "expected error message for invalid file, got stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
}

#[test]
#[cfg(not(debug_assertions))]
fn player_cli_play_with_filter_arg_parses() {
    let demo = demo_audio_path("classical.wav");
    if !demo.is_file() {
        eprintln!("skipping: demo audio not found at {}", demo.display());
        return;
    }

    // We only verify argument parsing here; actual playback would require
    // an audio device, so we use --duration 0 to exit immediately after setup.
    let output = Command::new(cargo_bin("player-cli"))
        .args([
            "play",
            demo.to_str().unwrap(),
            "--duration",
            "0",
            "--filter",
            "1000:1.5:3.0",
        ])
        .output()
        .expect("failed to spawn player-cli");

    // May succeed or fail depending on audio device availability;
    // the important thing is that it doesn't panic during argument parsing.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument") && !stderr.contains("error: Found argument"),
        "argument parsing failed: {stderr}"
    );
}

#[test]
#[cfg(not(debug_assertions))]
fn player_cli_play_with_rack_arg_parses() {
    let demo = demo_audio_path("classical.wav");
    if !demo.is_file() {
        eprintln!("skipping: demo audio not found at {}", demo.display());
        return;
    }

    let output = Command::new(cargo_bin("player-cli"))
        .args([
            "play",
            demo.to_str().unwrap(),
            "--duration",
            "0",
            "--rack",
            "eq,limiter",
        ])
        .output()
        .expect("failed to spawn player-cli");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument") && !stderr.contains("error: Found argument"),
        "argument parsing failed: {stderr}"
    );
}

#[test]
#[cfg(not(debug_assertions))]
fn player_cli_play_with_lufs_flag_parses() {
    let demo = demo_audio_path("classical.wav");
    if !demo.is_file() {
        eprintln!("skipping: demo audio not found at {}", demo.display());
        return;
    }

    let output = Command::new(cargo_bin("player-cli"))
        .args([
            "play",
            demo.to_str().unwrap(),
            "--duration",
            "0",
            "--lufs",
        ])
        .output()
        .expect("failed to spawn player-cli");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument") && !stderr.contains("error: Found argument"),
        "argument parsing failed: {stderr}"
    );
}

#[test]
#[cfg(not(debug_assertions))]
fn player_cli_play_with_loudness_compensation_parses() {
    let demo = demo_audio_path("classical.wav");
    if !demo.is_file() {
        eprintln!("skipping: demo audio not found at {}", demo.display());
        return;
    }

    let output = Command::new(cargo_bin("player-cli"))
        .args([
            "play",
            demo.to_str().unwrap(),
            "--duration",
            "0",
            "--loudness-compensation",
            "-20,10",
        ])
        .output()
        .expect("failed to spawn player-cli");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument") && !stderr.contains("error: Found argument"),
        "argument parsing failed: {stderr}"
    );
}

// ============================================================================
// sotf-recorder-cli
// ============================================================================

#[test]
fn recorder_cli_list_devices_shows_devices() {
    let output = Command::new(cargo_bin("sotf-recorder-cli"))
        .args(["--list-devices"])
        .output()
        .expect("failed to spawn sotf-recorder-cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "sotf-recorder-cli --list-devices exited with {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("Available Audio Devices"),
        "expected device list header in stdout, got: {stdout}"
    );
}

#[test]
fn recorder_cli_missing_required_args_fails() {
    let output = Command::new(cargo_bin("sotf-recorder-cli"))
        .args(["--signal", "tone"])
        .output()
        .expect("failed to spawn sotf-recorder-cli");

    assert!(
        !output.status.success(),
        "expected failure when missing required args"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Error") || stderr.contains("required"),
        "expected error about missing args, got: {stderr}"
    );
}

#[test]
fn recorder_cli_invalid_channel_config_fails() {
    // Single record channel with multiple send channels is valid (1:N case),
    // so let's test the invalid case directly.
    let output = Command::new(cargo_bin("sotf-recorder-cli"))
        .args([
            "--signal", "tone",
            "--freq", "1000",
            "--duration", "1",
            "--hwaudio-send-to", "0,1",
            "--hwaudio-record-from", "0,1,2",
        ])
        .output()
        .expect("failed to spawn sotf-recorder-cli");

    assert!(
        !output.status.success(),
        "expected failure for mismatched channel counts"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Invalid channel configuration"),
        "expected channel config error, got: {stderr}"
    );
}

#[test]
fn recorder_cli_tone_signal_dry_run() {
    // Without audio hardware this will fail at playback time, but we can
    // verify that argument parsing succeeds and the config is printed.
    let output = Command::new(cargo_bin("sotf-recorder-cli"))
        .args([
            "--signal", "tone",
            "--freq", "1000",
            "--duration", "0.5",
            "--hwaudio-send-to", "0",
            "--hwaudio-record-from", "0",
        ])
        .output()
        .expect("failed to spawn sotf-recorder-cli");

    // The binary may fail due to missing audio device, but it should at
    // least parse arguments and print the configuration header.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Signal Recording and Analysis") || stdout.contains("Configuration:"),
        "expected config header in stdout, got: {stdout}"
    );
}

// ============================================================================
// Helpers
// ============================================================================

fn stdout_contains(output: &std::process::Output, needle: &str) -> bool {
    String::from_utf8_lossy(&output.stdout).contains(needle)
}
