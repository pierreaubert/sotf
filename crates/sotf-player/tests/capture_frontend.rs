//! Frontend capture workflow over a golden session.
//!
//! Moved from sotf-capture's clock tests: it exercises this crate's
//! `CaptureWorkflow` and `RoomEqScreenModel`, so it lives with its
//! subjects. The golden helpers mirror sotf-capture's clock fixtures;
//! the two JSON fixtures are intentionally duplicated into this crate's
//! tests/fixtures so each crate stays self-contained.

#![cfg(not(target_os = "ios"))]

use serde::Deserialize;
use sotf_capture::capture_session::protocol::prepare_capture_stimulus;
use sotf_capture::capture_session::record::RawCaptureManifest;
use sotf_capture::capture_session::record::RawCaptureStatus;
use sotf_capture::capture_session::record::RawCaptureTake;
use sotf_capture::capture_session::{
    CaptureGeometry, CaptureSessionPlan, CaptureTimingReference,
};

#[derive(Deserialize)]
struct Golden {
    sample_rate_hz: u32,
    offset_samples: [f64; 2],
    skew_ppm: [f64; 2],
    microphone_positions_m: [[f64; 3]; 2],
    timing_emitter_position_m: [f64; 3],
    sound_speed_m_s: f64,
}

fn fixture() -> (Golden, RawCaptureManifest, Vec<f32>, Vec<f32>) {
    let golden: Golden = serde_json::from_str(include_str!(
        "fixtures/capture-clock-golden.json"
    ))
    .unwrap();
    let mut plan: CaptureSessionPlan =
        serde_json::from_str(include_str!("fixtures/capture-session.json")).unwrap();
    plan.sample_rate_hz = golden.sample_rate_hz;
    plan.geometry = CaptureGeometry::Compact;
    plan.sweep.duration_secs = 0.05;
    for (mic, position) in plan
        .microphones
        .iter_mut()
        .zip(golden.microphone_positions_m)
    {
        mic.position_m = position;
        mic.position_uncertainty_mm = 0.1;
    }
    plan.timing_reference = Some(CaptureTimingReference {
        output_channel: 0,
        position_m: golden.timing_emitter_position_m,
        position_uncertainty_mm: 0.1,
        sound_speed_m_s: golden.sound_speed_m_s,
        sound_speed_uncertainty_m_s: 0.1,
    });
    let session = plan.validate().unwrap();
    let stimulus = prepare_capture_stimulus(&session).unwrap();
    let manifest = RawCaptureManifest {
        version: 1,
        plan: session.plan().clone(),
        stimulus: stimulus.layout,
        timing_reference_output_channel: Some(0),
        status: RawCaptureStatus::RawComplete,
        pending_processing: vec!["clock_correction".into()],
        calibrations: vec![],
        takes: vec![],
        error: None,
    };
    (golden, manifest, stimulus.samples, stimulus.timing_chirp)
}

// Independent first-order-hold sampling oracle. This intentionally does not use
// the production FIR resampler; the interpolation adds a small waveform error.
fn sample_clock(stimulus: &[f32], offset: f64, ppm: f64, propagation: f64) -> Vec<f32> {
    (0..stimulus.len() + 4096)
        .map(|n| {
            let time = (n as f64 - offset) / (1.0 + ppm / 1e6) - propagation;
            if time < 0.0 || time >= (stimulus.len() - 1) as f64 {
                return 0.0;
            }
            let index = time.floor() as usize;
            let frac = (time - index as f64) as f32;
            stimulus[index] * (1.0 - frac) + stimulus[index + 1] * frac
        })
        .collect()
}

fn take(manifest: &RawCaptureManifest, index: usize, samples: usize) -> RawCaptureTake {
    RawCaptureTake {
        take_id: String::new(), // The golden fixture uses the legacy single-take identity.
        repeat_index: 0,
        device_id: format!("usb-{index}"),
        output_device_id: "dac".into(),
        source_id: "left".into(),
        microphone_id: manifest.plan.microphones[index].id.clone(),
        wav_file: format!("mic-{index}.wav"),
        samples,
        input_sample_format: "F32".into(),
        output_sample_format: "F32".into(),
        peak_amplitude: 0.1,
        clipped_samples: 0,
    }
}

fn write_golden_directory() -> tempfile::TempDir {
    use sha2::{Digest, Sha256};
    use sotf_capture::capture_session::record::CaptureCalibration;
    use sotf_capture::signal_recorder::write_wav_file;
    let root = tempfile::tempdir().unwrap();
    let (golden, mut manifest, stimulus, chirp) = fixture();
    manifest.plan.sources.truncate(1);
    write_wav_file(
        &root.path().join("stimulus.wav"),
        &stimulus,
        golden.sample_rate_hz,
        1,
    )
    .unwrap();
    write_wav_file(
        &root.path().join("timing-chirp.wav"),
        &chirp,
        golden.sample_rate_hz,
        1,
    )
    .unwrap();
    for index in 0..2 {
        let propagation = (1.0 - golden.microphone_positions_m[index][0]) / golden.sound_speed_m_s
            * f64::from(golden.sample_rate_hz);
        let mut input = sample_clock(
            &stimulus,
            golden.offset_samples[index],
            golden.skew_ppm[index],
            propagation,
        );
        if index == 1 {
            let end_start = ((manifest.stimulus.end_chirp_offset as f64 + propagation)
                * (1.0 + golden.skew_ppm[index] / 1e6)
                + golden.offset_samples[index]) as usize;
            input[end_start.saturating_sub(32)..].fill(0.0);
        }
        let take = take(&manifest, index, input.len());
        write_wav_file(
            &root.path().join(&take.wav_file),
            &input,
            golden.sample_rate_hz,
            1,
        )
        .unwrap();
        manifest.takes.push(take);
        let bytes = b"20 0\n1000 0\n20000 0\n";
        let file = format!("calibration-{index}.txt");
        std::fs::write(root.path().join(&file), bytes).unwrap();
        manifest.calibrations.push(CaptureCalibration {
            microphone_id: manifest.plan.microphones[index].id.clone(),
            file,
            sha256: Sha256::digest(bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(""),
        });
    }
    std::fs::write(
        root.path().join("capture-raw.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    root
}

#[test]
fn capture_frontend_workflow_processes_and_imports_golden_session() {
    use sotf_audio_player::ui_models::capture::{CaptureStage, CaptureWorkflow};
    use sotf_audio_player::ui_models::room_eq::{RoomEqScreenModel, RoomEqViewEvent};

    let root = write_golden_directory();
    let output = root.path().join("frontend-processed");
    let mut workflow = CaptureWorkflow::default();
    workflow
        .process(root.path().to_path_buf(), output.clone())
        .unwrap();
    assert!(workflow.import_channels().is_none());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while workflow.is_busy() {
        workflow.poll();
        assert!(
            std::time::Instant::now() < deadline,
            "capture worker timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(
        workflow.stage,
        CaptureStage::Complete,
        "{}",
        workflow.message
    );
    assert_eq!(
        workflow.recording_manifest.as_ref(),
        Some(&output.join("recordings.json"))
    );
    assert!(
        workflow
            .qa_lines
            .iter()
            .any(|line| line.contains("Magnitude-only:"))
    );
    assert!(workflow.qa_lines.iter().any(|line| line.contains("SNR:")));
    let mut room = RoomEqScreenModel::default();
    room.apply(RoomEqViewEvent::LoadMeasurements(
        workflow.import_channels().unwrap(),
    ));
    let config = room.to_room_config();
    let autoeq::SpeakerConfig::Single(autoeq::MeasurementSource::Multiple(source)) =
        &config.speakers["left"]
    else {
        panic!("lost simultaneous microphone set")
    };
    let capture = source.provenance.capture.as_ref().unwrap();
    assert_eq!(capture.takes.len(), 2);
    assert!(capture.takes[1].residual_uncertainty_us.is_none());
    assert!(capture.coherent_reference(2).is_err());
}
