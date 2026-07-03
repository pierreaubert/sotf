use super::misc::default_room_eq_export_path;
use super::misc::load_room_eq_recording_fixture;
use super::misc::room_eq_export_summary_for_path;
use super::parse::parse_simple_crossover;
use super::parse::parse_simple_loss;
use super::parse::parse_simple_processing;
use super::parse::parse_speaker_tier;
use super::with::with_app_state;
use super::with::with_player_view;
use crate::app::types::RoomEqStep;
use anyhow::{Result, anyhow};
use gpui::{AnyWindowHandle, App};
use sotf_audio_player::room_eq_types::RoomEqWizardMode;
use std::path::PathBuf;

pub(super) fn qa_seed(
    payload: serde_json::Value,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<()> {
    let library_dirs = payload
        .get("library_dirs")
        .and_then(|value| value.as_array())
        .ok_or_else(|| anyhow!("seed payload needs `library_dirs` array"))?;

    let mut dirs = Vec::with_capacity(library_dirs.len());
    for value in library_dirs {
        let dir = value
            .as_str()
            .ok_or_else(|| anyhow!("library_dirs entries must be strings"))?;
        let dir = PathBuf::from(dir);
        if !dir.is_dir() {
            return Err(anyhow!(
                "seed library directory does not exist: {}",
                dir.display()
            ));
        }
        dirs.push(dir);
    }

    with_app_state(window, cx, |state| {
        for dir in dirs {
            state.app.add_directory_quiet(dir);
        }
        state
            .app
            .library_state
            .scan()
            .map_err(|err| anyhow!("scanning seeded library directories: {err}"))?;
        state.app.invalidate_library_stats();
        state.app.ui_state.current_screen = crate::app::Screen::Library;
        state.app.ui_state.input_mode = crate::app::InputMode::Normal;
        Ok(())
    })
}

pub(super) fn qa_room_eq(
    payload: serde_json::Value,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<()> {
    let fixture_dir = payload
        .get("fixture_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("room-eq payload needs `fixture_dir` string"))?;
    let fixture_dir = PathBuf::from(fixture_dir);
    if !fixture_dir.is_dir() {
        return Err(anyhow!(
            "RoomEQ fixture directory does not exist: {}",
            fixture_dir.display()
        ));
    }

    let start = payload
        .get("start")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let num_filters = payload
        .get("num_filters")
        .and_then(|v| v.as_u64())
        .map(|v| v.clamp(1, 32) as usize);
    let max_iter = payload
        .get("max_iter")
        .and_then(|v| v.as_u64())
        .map(|v| v.clamp(1, 50_000) as usize);
    let population = payload
        .get("population")
        .and_then(|v| v.as_u64())
        .map(|v| v.clamp(1, 10_000) as usize);
    let target = payload
        .get("target")
        .and_then(|v| v.as_str())
        .map(parse_speaker_tier)
        .transpose()?;
    let loss = payload
        .get("loss")
        .and_then(|v| v.as_str())
        .map(parse_simple_loss)
        .transpose()?;
    let processing = payload
        .get("processing")
        .and_then(|v| v.as_str())
        .map(parse_simple_processing)
        .transpose()?;
    let crossover = payload
        .get("crossover")
        .and_then(|v| v.as_str())
        .map(parse_simple_crossover)
        .transpose()?;

    let recording_state = load_room_eq_recording_fixture(&fixture_dir)?;

    with_player_view(window, cx, |view, cx| {
        view.state.update(cx, |state, cx| {
            state.app.measurement_state.recording_state = recording_state.clone();
            cx.notify();
        });

        view.load_room_eq_from_recording(cx);

        view.state.update(cx, |state, cx| {
            let room_eq = &mut state.app.measurement_state.room_eq_state;
            room_eq.wizard_mode = RoomEqWizardMode::Simple;
            if let Some(target) = target {
                room_eq.simple_preset.target = target;
            }
            if let Some(loss) = loss {
                room_eq.simple_preset.loss = loss;
            }
            if let Some(processing) = processing {
                room_eq.simple_preset.processing = processing;
            }
            if let Some(crossover) = crossover {
                room_eq.simple_preset.crossover = crossover;
            }
            let preset = room_eq.simple_preset.clone();
            sotf_audio_player::room_eq_types::apply_simple_preset(
                &preset,
                &mut room_eq.optimizer_config,
            );
            if let Some(num_filters) = num_filters {
                room_eq.optimizer_config.num_filters = num_filters;
            }
            if let Some(max_iter) = max_iter {
                room_eq.optimizer_config.max_iter = max_iter;
            }
            if let Some(population) = population {
                room_eq.optimizer_config.population = population;
            }
            room_eq.wizard_mode = RoomEqWizardMode::Full;
            room_eq.step = RoomEqStep::Optimize;
            room_eq.optimization_status = crate::app::types::OptimizationStatus::Idle;
            room_eq.channel_results.clear();
            room_eq.dsp_output = None;
            room_eq.overall_progress = 0.0;
            room_eq.progress_history.clear();
            room_eq.status_message =
                "QA RoomEQ fixture loaded with default wizard preset".to_string();
            room_eq.error_message = None;
            cx.notify();
        });

        if start {
            view.start_room_eq_optimization(cx);
        }

        Ok(())
    })
}

pub(super) fn qa_room_eq_export_json(
    payload: serde_json::Value,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<serde_json::Value> {
    let path = payload
        .get("path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(default_room_eq_export_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let dsp_output = with_app_state(window, cx, |state| {
        let room_eq = &state.app.measurement_state.room_eq_state;
        if room_eq.optimization_status != crate::app::types::OptimizationStatus::Completed {
            return Err(anyhow!(
                "RoomEQ optimization is not completed: {:?}",
                room_eq.optimization_status
            ));
        }
        room_eq
            .dsp_output
            .clone()
            .ok_or_else(|| anyhow!("RoomEQ has no DSP output to export"))
    })?;

    let json = serde_json::to_string_pretty(&dsp_output)?;
    std::fs::write(&path, json)?;
    let summary = room_eq_export_summary_for_path(&path)?;

    with_app_state(window, cx, |state| {
        let room_eq = &mut state.app.measurement_state.room_eq_state;
        room_eq.step = RoomEqStep::Export;
        room_eq.status_message = format!("QA RoomEQ JSON exported: {}", path.display());
        room_eq.error_message = None;
        Ok(())
    })?;

    Ok(summary)
}
