#![allow(dead_code)]
use assert_cmd::Command;
use std::path::PathBuf;
use std::time::Duration;

/// Maximum time any CLI integration test is allowed to run before being killed.
///
/// The `play_*_parses` tests exercise argument parsing and plugin wiring only;
/// they do not require real audio hardware. A short timeout prevents CI runners
/// from hanging indefinitely when cpal cannot open the default device.
const TEST_TIMEOUT: Duration = Duration::from_secs(15);

fn player_cmd() -> Command {
    let mut cmd = Command::cargo_bin("player-cli").expect("player-cli binary available");
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

fn demo_audio_path(name: &str) -> PathBuf {
    if let Ok(root) = std::env::var("SOTF_TEST_DATA_ROOT") {
        PathBuf::from(root).join("audio").join(name)
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("data_tests")
            .join("audio")
            .join(name)
    }
}

const PLAY_PARSE_SMOKE_DURATION_SECS: &str = "1";

#[sotf_test::requires_hardware]
#[test]
fn player_cli_devices_lists_audio_devices() {
    let output = player_cmd()
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

    let output = player_cmd()
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
    let output = player_cmd()
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

    // We only verify argument parsing here. Use a finite duration so the
    // smoke test cannot enter the CLI's "play until stopped" mode.
    let output = player_cmd()
        .args([
            "play",
            demo.to_str().unwrap(),
            "--duration",
            PLAY_PARSE_SMOKE_DURATION_SECS,
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

    let output = player_cmd()
        .args([
            "play",
            demo.to_str().unwrap(),
            "--duration",
            PLAY_PARSE_SMOKE_DURATION_SECS,
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

    let output = player_cmd()
        .args([
            "play",
            demo.to_str().unwrap(),
            "--duration",
            PLAY_PARSE_SMOKE_DURATION_SECS,
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

    let output = player_cmd()
        .args([
            "play",
            demo.to_str().unwrap(),
            "--duration",
            PLAY_PARSE_SMOKE_DURATION_SECS,
            "--loudness-compensation=-20,10",
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
fn player_cli_status_runs_without_panicking() {
    // The status subcommand does not use the play subcommand's clap-assert
    // path, so it is safe to run in debug builds.
    let output = player_cmd()
        .args(["status"])
        .output()
        .expect("failed to spawn player-cli");

    // status may fail when nothing is playing, but it should not panic.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("panicked") && !stderr.contains("assertion failed"),
        "player-cli status panicked: {stderr}"
    );
}

#[test]
fn player_cli_replay_gain_invalid_file_fails() {
    let output = player_cmd()
        .args(["replay-gain", "/nonexistent/file.wav"])
        .output()
        .expect("failed to spawn player-cli");

    assert!(
        !output.status.success(),
        "expected player-cli replay-gain to fail for nonexistent file"
    );
}

#[test]
#[cfg(not(debug_assertions))]
fn player_cli_play_with_device_arg_parses() {
    let demo = demo_audio_path("classical.wav");
    if !demo.is_file() {
        eprintln!("skipping: demo audio not found at {}", demo.display());
        return;
    }

    let output = player_cmd()
        .args([
            "play",
            demo.to_str().unwrap(),
            "--duration",
            PLAY_PARSE_SMOKE_DURATION_SECS,
            "--device",
            "default",
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
fn player_cli_play_with_start_time_arg_parses() {
    let demo = demo_audio_path("classical.wav");
    if !demo.is_file() {
        eprintln!("skipping: demo audio not found at {}", demo.display());
        return;
    }

    let output = player_cmd()
        .args([
            "play",
            demo.to_str().unwrap(),
            "--duration",
            PLAY_PARSE_SMOKE_DURATION_SECS,
            "--start-time",
            "5.0",
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
fn player_cli_play_with_loudness_auto_gain_parses() {
    let demo = demo_audio_path("classical.wav");
    if !demo.is_file() {
        eprintln!("skipping: demo audio not found at {}", demo.display());
        return;
    }

    let output = player_cmd()
        .args([
            "play",
            demo.to_str().unwrap(),
            "--duration",
            PLAY_PARSE_SMOKE_DURATION_SECS,
            "--loudness-auto-gain",
            "--loudness-auto-gain-max-db",
            "12.0",
            "--loudness-auto-gain-smoothing-ms",
            "100.0",
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
fn player_cli_play_with_upmixer_args_parses() {
    let demo = demo_audio_path("classical.wav");
    if !demo.is_file() {
        eprintln!("skipping: demo audio not found at {}", demo.display());
        return;
    }

    let output = player_cmd()
        .args([
            "play",
            demo.to_str().unwrap(),
            "--duration",
            PLAY_PARSE_SMOKE_DURATION_SECS,
            "--upmixer",
            "--upmixer-gain-front-direct",
            "1.0",
            "--upmixer-stereo-width",
            "1.2",
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
fn player_cli_play_with_compressor_args_parses() {
    let demo = demo_audio_path("classical.wav");
    if !demo.is_file() {
        eprintln!("skipping: demo audio not found at {}", demo.display());
        return;
    }

    let output = player_cmd()
        .args([
            "play",
            demo.to_str().unwrap(),
            "--duration",
            PLAY_PARSE_SMOKE_DURATION_SECS,
            "--compressor",
            "--compressor-threshold-db=-20.0",
            "--compressor-ratio",
            "4.0",
            "--compressor-attack-ms",
            "10.0",
            "--compressor-release-ms",
            "100.0",
        ])
        .output()
        .expect("failed to spawn player-cli");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument") && !stderr.contains("error: Found argument"),
        "argument parsing failed: {stderr}"
    );
}

#[sotf_test::requires_hardware]
#[test]
fn recorder_cli_list_devices_shows_devices() {
    let output = recorder_cmd()
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
    let output = recorder_cmd()
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
    let output = recorder_cmd()
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
    let output = recorder_cmd()
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

#[test]
fn recorder_cli_sweep_signal_dry_run() {
    let output = recorder_cmd()
        .args([
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
        ])
        .output()
        .expect("failed to spawn sotf-recorder-cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Signal Recording and Analysis") || stdout.contains("Configuration:"),
        "expected config header in stdout, got: {stdout}"
    );
}

#[test]
fn recorder_cli_pink_noise_signal_dry_run() {
    let output = recorder_cmd()
        .args([
            "--signal",
            "pink-noise",
            "--duration",
            "0.5",
            "--hwaudio-send-to",
            "0",
            "--hwaudio-record-from",
            "0",
        ])
        .output()
        .expect("failed to spawn sotf-recorder-cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Signal Recording and Analysis") || stdout.contains("Configuration:"),
        "expected config header in stdout, got: {stdout}"
    );
}

#[test]
fn recorder_cli_custom_sample_rate_parses() {
    let output = recorder_cmd()
        .args([
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
        ])
        .output()
        .expect("failed to spawn sotf-recorder-cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Signal Recording and Analysis") || stdout.contains("Configuration:"),
        "expected config header in stdout, got: {stdout}"
    );
}

#[allow(dead_code)]
fn stdout_contains(output: &std::process::Output, needle: &str) -> bool {
    String::from_utf8_lossy(&output.stdout).contains(needle)
}
