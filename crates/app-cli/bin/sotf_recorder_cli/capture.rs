//! Multi-device session CLI; shared player logic owns capture orchestration.

use sotf_audio_player::capture_session::CaptureSessionPlan;
use sotf_audio_player::capture_session::ValidatedCaptureSession;
use sotf_audio_player::capture_session::record::record_capture_session;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

fn load_plan(plan_path: &Path) -> Result<ValidatedCaptureSession, String> {
    let json = std::fs::read_to_string(plan_path)
        .map_err(|e| format!("cannot read capture session: {e}"))?;
    let plan: CaptureSessionPlan =
        serde_json::from_str(&json).map_err(|e| format!("cannot parse capture session: {e}"))?;
    plan.validate().map_err(|e| e.to_string())
}

pub(super) fn validate(plan_path: &Path) -> Result<(), String> {
    let session = load_plan(plan_path)?;
    println!(
        "Session plan valid: {:?}, {} microphones, {} sources, {} Hz. Hardware, calibration files and take quality have not been checked.",
        session.plan().geometry,
        session.plan().microphones.len(),
        session.plan().sources.len(),
        session.plan().sample_rate_hz,
    );
    Ok(())
}

pub(super) fn record(plan_path: &Path, output: &Path) -> Result<(), String> {
    let session = load_plan(plan_path)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let handler_cancel = Arc::clone(&cancel);
    ctrlc::set_handler(move || handler_cancel.store(true, Ordering::Relaxed))
        .map_err(|e| format!("cannot install recording cancellation handler: {e}"))?;
    let plan_directory = plan_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let manifest = record_capture_session(&session, plan_directory, output, &cancel, |event| {
        println!(
            "Recording source {}/{}: {} (all microphones)",
            event.source_index + 1,
            event.source_count,
            event.source_id
        );
    })?;
    println!(
        "Saved {} raw takes. Clock correction, calibrated analysis and take QA remain pending.",
        manifest.takes.len()
    );
    Ok(())
}
