//! GPUI capture commands delegate to the shared worker without opening hardware.
#![cfg(not(target_os = "ios"))]

use sotf_audio_player::ui_models::capture::CaptureStage;
use sotf_audio_player_gpui::app::state::measurement::{
    CaptureAction, CapturePath, MultiCaptureState,
};
use std::time::{Duration, Instant};

fn finish(panel: &mut MultiCaptureState) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while panel.workflow.is_busy() {
        panel.workflow.poll();
        assert!(Instant::now() < deadline, "capture worker did not finish");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn capture_panel_cannot_overlap_legacy_recording_or_import_raw_data() {
    let mut panel = MultiCaptureState::default();
    assert!(!panel.open(true));
    assert!(!panel.active);
    assert!(panel.open(false));
    assert!(panel.active);
    assert!(panel.execute(CaptureAction::Record).is_err());
    assert!(panel.execute(CaptureAction::Import).is_err());
    assert!(!panel.workflow.is_busy());
    assert!(panel.close());
    assert!(!panel.active);
}

#[test]
fn capture_plan_inputs_are_frozen_during_load_and_revalidated_after_edit() {
    let directory = tempfile::tempdir().unwrap();
    let plan = directory.path().join("session.json");
    std::fs::write(
        &plan,
        include_str!("../../sotf-player/tests/fixtures/capture-session.json"),
    )
    .unwrap();
    let mut panel = MultiCaptureState::default();
    panel.open(false);
    panel
        .edit_path(CapturePath::Plan, plan.display().to_string())
        .unwrap();
    panel.execute(CaptureAction::LoadPlan).unwrap();
    assert!(!panel.close());
    assert!(panel.edit_path(CapturePath::Raw, "new-raw".into()).is_err());
    assert!(panel.execute(CaptureAction::LoadPlan).is_err());
    finish(&mut panel);
    assert_eq!(
        panel.workflow.stage,
        CaptureStage::Ready,
        "{}",
        panel.workflow.message
    );
    assert!(panel.workflow.can_record());
    assert!(!panel.workflow.can_import());
    panel
        .edit_path(
            CapturePath::Raw,
            directory.path().join("raw").display().to_string(),
        )
        .unwrap();
    assert!(panel.workflow.can_record());
    panel
        .edit_path(CapturePath::Plan, "different-plan.json".into())
        .unwrap();
    assert!(!panel.workflow.can_record());
    assert!(panel.execute(CaptureAction::Record).is_err());
    assert!(!directory.path().join("raw").exists());
}

#[test]
fn failed_saved_session_processing_recovers_controls_without_stale_import() {
    let directory = tempfile::tempdir().unwrap();
    let mut panel = MultiCaptureState::default();
    panel.open(false);
    panel
        .edit_path(
            CapturePath::Raw,
            directory.path().join("missing").display().to_string(),
        )
        .unwrap();
    panel
        .edit_path(
            CapturePath::Processed,
            directory.path().join("processed").display().to_string(),
        )
        .unwrap();
    panel.execute(CaptureAction::Process).unwrap();
    assert!(!panel.close());
    finish(&mut panel);
    assert_eq!(panel.workflow.stage, CaptureStage::Failed);
    assert!(!panel.workflow.message.is_empty());
    assert!(panel.execute(CaptureAction::Import).is_err());
    assert!(panel.close());
}
