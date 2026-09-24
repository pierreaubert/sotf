use super::*;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

fn setup() -> (tempfile::TempDir, ValidatedCaptureSession) {
    let root = tempfile::tempdir().unwrap();
    let mut plan: CaptureSessionPlan =
        serde_json::from_str(include_str!("../../../tests/fixtures/capture-session.json")).unwrap();
    plan.sweep.duration_secs = 0.05;
    for mic in &plan.microphones {
        std::fs::write(
            root.path().join(&mic.calibration_file),
            "20 0\n1000 1\n20000 0\n",
        )
        .unwrap();
    }
    (root, plan.validate().unwrap())
}

fn fake_capture(
    request: MultiCaptureRequest,
    _: &CancelFlag,
) -> Result<MultiCaptureResult, String> {
    assert_eq!(request.output_overrides.len(), 2);
    assert!(
        request
            .output_overrides
            .iter()
            .all(|region| region.channel == 0)
    );
    Ok(MultiCaptureResult {
        input_device_ids: request
            .inputs
            .iter()
            .map(|input| format!("id:{}", input.device))
            .collect(),
        output_device_id: "id:output".into(),
        sample_rate_hz: request.sample_rate_hz,
        recordings: request
            .inputs
            .iter()
            .map(|_| request.stimulus.clone())
            .collect(),
        input_sample_formats: vec!["F32".into(); request.inputs.len()],
        output_sample_format: "F32".into(),
    })
}

fn journal(directory: &Path) -> RawCaptureManifest {
    serde_json::from_slice(&std::fs::read(directory.join("capture-raw.json")).unwrap()).unwrap()
}

#[test]
fn all_mics_record_each_source_and_calibration_snapshots_are_frozen() {
    let (root, session) = setup();
    let output = root.path().join("recording");
    let cancel = Arc::new(AtomicBool::new(false));
    let mut sources = Vec::new();
    let manifest = record_with(
        &session,
        root.path(),
        &output,
        &cancel,
        |event| sources.push(event.source_id),
        |request, cancel| {
            // An external edit after acquisition starts must not change provenance.
            std::fs::write(
                root.path()
                    .join(&session.plan().microphones[0].calibration_file),
                "changed",
            )
            .unwrap();
            fake_capture(request, cancel)
        },
    )
    .unwrap();
    assert_eq!(sources, ["left", "right"]);
    assert_eq!(manifest.status, RawCaptureStatus::RawComplete);
    assert_eq!(manifest.takes.len(), 4);
    assert!(
        manifest
            .pending_processing
            .contains(&"clock_correction".to_owned())
    );
    for take in &manifest.takes {
        assert!(output.join(&take.wav_file).is_file());
        assert_eq!(take.clipped_samples, 0);
    }
    for calibration in &manifest.calibrations {
        let bytes = std::fs::read(output.join(&calibration.file)).unwrap();
        assert_eq!(calibration.sha256, format!("{:x}", Sha256::digest(&bytes)));
        assert_ne!(bytes, b"changed");
    }
    assert_eq!(journal(&output).takes.len(), 4);
    assert!(
        !output.join("recordings.json").exists(),
        "raw journal must not masquerade as corrected import"
    );
}

#[test]
fn cancellation_retains_previous_source_and_explicit_status() {
    let (root, session) = setup();
    let output = root.path().join("recording");
    let cancel = Arc::new(AtomicBool::new(false));
    let error = record_with(
        &session,
        root.path(),
        &output,
        &cancel,
        |_| {},
        |request, flag| {
            flag.store(true, Ordering::Relaxed);
            fake_capture(request, flag)
        },
    )
    .unwrap_err();
    assert_eq!(error, "cancelled");
    let saved = journal(&output);
    assert_eq!(saved.status, RawCaptureStatus::Cancelled);
    assert_eq!(saved.takes.len(), 2);
}

#[test]
fn wrong_rate_fails_without_saving_a_successful_take() {
    let (root, session) = setup();
    let output = root.path().join("recording");
    let cancel = Arc::new(AtomicBool::new(false));
    assert!(
        record_with(
            &session,
            root.path(),
            &output,
            &cancel,
            |_| {},
            |request, flag| {
                let mut result = fake_capture(request, flag)?;
                result.sample_rate_hz = 44_100;
                Ok(result)
            }
        )
        .is_err()
    );
    let saved = journal(&output);
    assert_eq!(saved.status, RawCaptureStatus::Failed);
    assert!(saved.takes.is_empty());
}

#[test]
fn calibration_failure_precedes_audio_and_directory_creation() {
    let (root, session) = setup();
    let output = root.path().join("recording");
    std::fs::remove_file(
        root.path()
            .join(&session.plan().microphones[1].calibration_file),
    )
    .unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    assert!(
        record_with(
            &session,
            root.path(),
            &output,
            &cancel,
            |_| {},
            |_, _| panic!("audio must not start")
        )
        .is_err()
    );
    assert!(!output.exists());
}

#[test]
fn existing_directory_is_never_overwritten() {
    let (root, session) = setup();
    let output = root.path().join("recording");
    std::fs::create_dir(&output).unwrap();
    std::fs::write(output.join("keep"), "existing recording").unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    assert!(
        record_with(
            &session,
            root.path(),
            &output,
            &cancel,
            |_| {},
            |_, _| panic!("audio must not start")
        )
        .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(output.join("keep")).unwrap(),
        "existing recording"
    );
}
