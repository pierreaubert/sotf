//! Hardware-independent checks of the public capture-plan validation command.

use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

fn recorder() -> Command {
    let mut command = Command::cargo_bin("sotf-recorder-cli").unwrap();
    command.timeout(Duration::from_secs(15));
    command
}

#[test]
fn capture_plan_validates_without_legacy_signal_flags_or_hardware() {
    recorder()
        .arg("--validate-capture-session")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../sotf-player/tests/fixtures/capture-session.json"
        ))
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "2 microphones, 2 sources, 48000 Hz",
        ))
        .stdout(predicate::str::contains("have not been checked"));
}

#[test]
fn malformed_capture_plan_fails_without_recording() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), r#"{"version":1}"#).unwrap();
    recorder()
        .arg("--validate-capture-session")
        .arg(file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot parse capture session"));
}

#[test]
fn invalid_capture_geometry_exits_unsuccessfully() {
    let mut plan: serde_json::Value = serde_json::from_str(include_str!(
        "../../sotf-player/tests/fixtures/capture-session.json"
    ))
    .unwrap();
    plan["geometry"] = serde_json::json!("compact");
    plan["microphones"][0]["position_uncertainty_mm"] = serde_json::json!(5.0);
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), serde_json::to_vec(&plan).unwrap()).unwrap();
    recorder()
        .arg("--validate-capture-session")
        .arg(file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("at most 1 mm uncertainty"));
}

#[test]
fn device_listing_cannot_hide_capture_plan_validation() {
    recorder()
        .args([
            "--list-devices",
            "--validate-capture-session",
            "unused.json",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn capture_requires_an_explicit_output_directory() {
    recorder()
        .args(["--capture-session", "unused.json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--output-dir"));
}

#[test]
fn missing_calibration_stops_capture_before_device_access() {
    let output = tempfile::tempdir().unwrap();
    recorder()
        .arg("--capture-session")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../sotf-player/tests/fixtures/capture-session.json"
        ))
        .arg("--output-dir")
        .arg(output.path().join("new-recording"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot read calibration"));
    assert!(!output.path().join("new-recording").exists());
}

#[test]
fn dedicated_capture_binary_validates_the_same_session_contract() {
    Command::cargo_bin("roomeq-capture")
        .unwrap()
        .timeout(Duration::from_secs(15))
        .arg("validate")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../sotf-player/tests/fixtures/capture-session.json"
        ))
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "2 microphones, 2 sources, 48000 Hz",
        ));
}

#[test]
fn offline_processing_reports_missing_journal_without_opening_audio() {
    let root = tempfile::tempdir().unwrap();
    Command::cargo_bin("roomeq-capture")
        .unwrap()
        .timeout(Duration::from_secs(15))
        .arg("process")
        .arg(root.path())
        .arg("--output-dir")
        .arg(root.path().join("processed"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot resolve capture artifact"));
    assert!(!root.path().join("processed").exists());
}
