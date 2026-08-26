use super::misc::default_room_eq_export_path;
use super::misc::load_room_eq_recording_fixture;
use super::misc::room_eq_export_summary_for_path;
use super::parse::parse_simple_crossover;
use super::parse::parse_simple_loss;
use super::parse::parse_simple_processing;
use super::parse::parse_speaker_tier;
use super::with::with_app_state;
use super::with::with_player_view;
use crate::app::types::headphone_eq::HeadphoneMeasurementSource;
#[cfg(feature = "dev-api")]
use crate::app::types::recording::{QaFakeCapture, QaFakeCaptureFault};
use crate::app::types::{
    ChannelMapping, ChannelRecording, ChannelRecordingState, RecordingState, RecordingStep,
    RoomEqStep,
};
use anyhow::{Result, anyhow};
use gpui::{AnyWindowHandle, App};
use sotf_audio_player::recording_types::RecordingResult;
use sotf_audio_player::room_eq_types::RoomEqWizardMode;
use std::collections::HashMap;
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

pub(super) fn qa_recording_fake_capture(
    payload: serde_json::Value,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<()> {
    let channels = payload
        .get("channels")
        .and_then(|value| value.as_u64())
        .ok_or_else(|| anyhow!("fake recording payload needs `channels` integer"))?;
    let channels = usize::try_from(channels)
        .ok()
        .filter(|channels| (1..=32).contains(channels))
        .ok_or_else(|| anyhow!("fake recording `channels` must be between 1 and 32"))?;
    let points = payload
        .get("points")
        .and_then(|value| value.as_u64())
        .ok_or_else(|| anyhow!("fake recording payload needs `points` integer"))?;
    let points = usize::try_from(points)
        .ok()
        .filter(|points| (2..=4096).contains(points))
        .ok_or_else(|| anyhow!("fake recording `points` must be between 2 and 4096"))?;

    let fault = match payload.get("fault").and_then(|value| value.as_str()) {
        None => None,
        Some(value) => Some(QaFakeCaptureFault::parse(value).ok_or_else(|| {
            anyhow!(
                "fake recording `fault` must be one of `device-loss`, `clipping`, or `io-failure`"
            )
        })?),
    };

    let channel_names = (0..channels)
        .map(|index| match index {
            0 => "L".to_string(),
            1 => "R".to_string(),
            2 => "C".to_string(),
            3 => "LFE".to_string(),
            4 => "SL".to_string(),
            5 => "SR".to_string(),
            6 => "BL".to_string(),
            7 => "BR".to_string(),
            _ => format!("CH{}", index + 1),
        })
        .collect::<Vec<_>>();

    let mut recording = RecordingState::default();
    recording.model.playback_config.num_channels = channels;
    recording.model.playback_config.channel_mappings = channel_names
        .iter()
        .enumerate()
        .map(|(index, name)| ChannelMapping::single(index + 1, name.clone()))
        .collect();
    recording.model.recording_config.num_channels = 1;
    recording.model.recording_config.channel_mappings = vec![0];
    recording.model.step = RecordingStep::Capture;
    recording.model.init_channel_recordings();
    recording.qa_fake_capture = Some(QaFakeCapture { points, fault });

    with_player_view(window, cx, |view, cx| {
        view.state.update(cx, |state, cx| {
            state.app.measurement_state.recording_state = recording;
            cx.notify();
        });
        Ok(())
    })
}

pub(super) fn qa_headphone_discovery_fixture(
    payload: serde_json::Value,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<()> {
    let catalog = payload
        .get("catalog")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow!("Headphone discovery fixture needs `catalog` array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| anyhow!("fixture catalog entries must be strings"))
        })
        .collect::<Result<Vec<_>>>()?;
    if catalog.is_empty() {
        return Err(anyhow!(
            "Headphone discovery fixture catalog must not be empty"
        ));
    }
    if catalog.iter().any(|name| name.trim().is_empty()) {
        return Err(anyhow!("fixture catalog entries must not be empty"));
    }

    let downloads = match payload.get("downloads") {
        None => HashMap::new(),
        Some(value) => value
            .as_object()
            .ok_or_else(|| anyhow!("fixture `downloads` must be an object keyed by headphone"))?
            .iter()
            .map(|(headphone, download)| {
                if !catalog.iter().any(|name| name == headphone) {
                    return Err(anyhow!(
                        "fixture download `{headphone}` is not present in `catalog`"
                    ));
                }
                let path = download
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .filter(|path| !path.trim().is_empty())
                    .ok_or_else(|| anyhow!("fixture download `{headphone}` needs non-empty `path`"))?
                    .to_owned();
                let points = download
                    .get("points")
                    .and_then(serde_json::Value::as_array)
                    .ok_or_else(|| anyhow!("fixture download `{headphone}` needs `points` array"))?
                    .iter()
                    .map(|point| {
                        let point = point.as_array().ok_or_else(|| {
                            anyhow!("fixture download `{headphone}` points must be [frequency, spl]")
                        })?;
                        if point.len() != 2 {
                            return Err(anyhow!(
                                "fixture download `{headphone}` points must have exactly two values"
                            ));
                        }
                        let frequency = point[0].as_f64().ok_or_else(|| {
                            anyhow!("fixture download `{headphone}` frequency must be numeric")
                        })?;
                        let spl = point[1].as_f64().ok_or_else(|| {
                            anyhow!("fixture download `{headphone}` SPL must be numeric")
                        })?;
                        if !frequency.is_finite() || frequency <= 0.0 || !spl.is_finite() {
                            return Err(anyhow!(
                                "fixture download `{headphone}` points must be finite with positive frequency"
                            ));
                        }
                        Ok((frequency, spl))
                    })
                    .collect::<Result<Vec<_>>>()?;
                if points.len() < 2 {
                    return Err(anyhow!(
                        "fixture download `{headphone}` needs at least two points"
                    ));
                }
                let delay_ms = download
                    .get("delay_ms")
                    .map(|value| {
                        value.as_u64().ok_or_else(|| {
                            anyhow!("fixture download `{headphone}` `delay_ms` must be a non-negative integer")
                        })
                    })
                    .transpose()?
                    .unwrap_or(0);
                let failures_remaining = download
                    .get("failures")
                    .map(|value| {
                        value.as_u64()
                            .and_then(|value| usize::try_from(value).ok())
                            .ok_or_else(|| {
                                anyhow!("fixture download `{headphone}` `failures` must be a non-negative integer")
                            })
                    })
                    .transpose()?
                    .unwrap_or(0);
                let failure_message = download
                    .get("failure_message")
                    .filter(|value| !value.is_null())
                    .map(|value| {
                        value
                            .as_str()
                            .filter(|value| !value.trim().is_empty())
                            .map(str::to_owned)
                            .ok_or_else(|| {
                                anyhow!("fixture download `{headphone}` `failure_message` must be a non-empty string")
                            })
                    })
                    .transpose()?
                    .unwrap_or_else(|| "Fixture measurement download failed".to_string());

                Ok((
                    headphone.clone(),
                    crate::app::types::headphone_eq::QaHeadphoneDownloadFixture {
                        path,
                        curve: points,
                        delay_ms,
                        failures_remaining,
                        failure_message,
                    },
                ))
            })
            .collect::<Result<HashMap<_, _>>>()?,
    };
    with_player_view(window, cx, |view, cx| {
        view.state.update(cx, |state, _cx| {
            state
                .app
                .measurement_state
                .headphone_eq_state
                .qa_discovery_fixture = Some(
                crate::app::types::headphone_eq::QaHeadphoneDiscoveryFixture { catalog, downloads },
            );
            state
                .app
                .measurement_state
                .headphone_eq_state
                .measurement_source = HeadphoneMeasurementSource::Spinorama;
            state.app.ui_state.current_screen = crate::app::Screen::HeadphoneEq;
            state.app.ui_state.input_mode = crate::app::InputMode::Normal;
        });
        cx.notify();
        Ok(())
    })
}

pub(super) fn qa_spinorama_discovery_fixture(
    payload: serde_json::Value,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<()> {
    let catalog = payload
        .get("catalog")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow!("Spinorama discovery fixture needs `catalog` array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|name| !name.trim().is_empty())
                .map(str::to_owned)
                .ok_or_else(|| anyhow!("fixture catalog entries must be non-empty strings"))
        })
        .collect::<Result<Vec<_>>>()?;
    if catalog.is_empty() {
        return Err(anyhow!(
            "Spinorama discovery fixture catalog must not be empty"
        ));
    }
    let catalog_delay_ms = payload
        .get("catalog_delay_ms")
        .map(|value| {
            value.as_u64().ok_or_else(|| {
                anyhow!("Spinorama fixture `catalog_delay_ms` must be a non-negative integer")
            })
        })
        .transpose()?
        .unwrap_or(0);
    let catalog_failures_remaining = payload
        .get("catalog_failures")
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| {
                    anyhow!("Spinorama fixture `catalog_failures` must be a non-negative integer")
                })
        })
        .transpose()?
        .unwrap_or(0);
    let catalog_failure_message = payload
        .get("catalog_failure_message")
        .filter(|value| !value.is_null())
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.trim().is_empty())
                .map(str::to_owned)
                .ok_or_else(|| {
                    anyhow!(
                        "Spinorama fixture `catalog_failure_message` must be a non-empty string"
                    )
                })
        })
        .transpose()?
        .unwrap_or_else(|| "Spinorama fixture catalog request failed".to_string());
    let speakers = payload
        .get("speakers")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| anyhow!("Spinorama discovery fixture needs `speakers` object"))?;
    let mut versions = HashMap::new();
    let mut measurements = HashMap::new();
    let mut responses = HashMap::new();
    for name in &catalog {
        let speaker = speakers
            .get(name)
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| anyhow!("fixture needs speaker data for `{name}`"))?;
        let fixture_versions = speaker
            .get("versions")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| anyhow!("fixture speaker `{name}` needs `versions` object"))?;
        if fixture_versions.is_empty() {
            return Err(anyhow!(
                "fixture speaker `{name}` needs at least one version"
            ));
        }
        let mut version_names = Vec::with_capacity(fixture_versions.len());
        for (version, values) in fixture_versions {
            if version.trim().is_empty() {
                return Err(anyhow!("fixture version names must not be empty"));
            }
            let values = values
                .as_array()
                .ok_or_else(|| {
                    anyhow!("fixture measurements for `{name}` / `{version}` must be array")
                })?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .filter(|measurement| !measurement.trim().is_empty())
                        .map(str::to_owned)
                        .ok_or_else(|| anyhow!("fixture measurements must be non-empty strings"))
                })
                .collect::<Result<Vec<_>>>()?;
            if values.is_empty() {
                return Err(anyhow!(
                    "fixture `{name}` / `{version}` needs at least one measurement"
                ));
            }
            version_names.push(version.clone());
            measurements.insert((name.clone(), version.clone()), values);
        }
        versions.insert(name.clone(), version_names);
    }
    if let Some(response_values) = payload.get("responses") {
        let response_values = response_values.as_object().ok_or_else(|| {
            anyhow!("Spinorama fixture `responses` must map speaker|version|measurement keys")
        })?;
        for (key, points) in response_values {
            let mut parts = key.splitn(3, '|');
            let (Some(speaker), Some(version), Some(measurement)) =
                (parts.next(), parts.next(), parts.next())
            else {
                return Err(anyhow!(
                    "Spinorama fixture response key must be speaker|version|measurement"
                ));
            };
            if !measurements
                .get(&(speaker.to_string(), version.to_string()))
                .is_some_and(|available| available.iter().any(|item| item == measurement))
            {
                return Err(anyhow!(
                    "Spinorama fixture response `{key}` has no matching measurement"
                ));
            }
            let points = points.as_array().ok_or_else(|| {
                anyhow!("Spinorama fixture response `{key}` must be an array of [Hz, dB]")
            })?;
            let (frequencies, spl): (Vec<_>, Vec<_>) = points
                .iter()
                .map(|point| {
                    let pair = point.as_array().ok_or_else(|| {
                        anyhow!("Spinorama fixture response `{key}` points must be [Hz, dB]")
                    })?;
                    if pair.len() != 2 {
                        return Err(anyhow!(
                            "Spinorama fixture response `{key}` points need two values"
                        ));
                    }
                    let frequency = pair[0]
                        .as_f64()
                        .filter(|value| value.is_finite() && *value > 0.0)
                        .ok_or_else(|| {
                            anyhow!("Spinorama fixture response `{key}` frequency must be positive")
                        })?;
                    let level = pair[1]
                        .as_f64()
                        .filter(|value| value.is_finite())
                        .ok_or_else(|| {
                            anyhow!("Spinorama fixture response `{key}` level must be finite")
                        })?;
                    Ok((frequency, level))
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .unzip();
            if frequencies.len() < 4 || frequencies.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(anyhow!(
                    "Spinorama fixture response `{key}` needs four increasing frequencies"
                ));
            }
            responses.insert(
                (
                    speaker.to_string(),
                    version.to_string(),
                    measurement.to_string(),
                ),
                crate::app::types::spinorama_eq::QaSpinoramaResponse { frequencies, spl },
            );
        }
    }
    with_player_view(window, cx, |view, cx| {
        view.state.update(cx, |state, _cx| {
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .qa_discovery_fixture = Some(
                crate::app::types::spinorama_eq::QaSpinoramaDiscoveryFixture {
                    catalog,
                    catalog_delay_ms,
                    catalog_failures_remaining,
                    catalog_failure_message,
                    versions,
                    measurements,
                    responses,
                },
            );
            state.app.ui_state.current_screen = crate::app::Screen::Spinorama;
            state.app.ui_state.input_mode = crate::app::InputMode::Normal;
            // The scenario must invoke the visible Refresh control. Avoid an
            // automatic render-time fetch consuming fixture responses first.
            state
                .app
                .measurement_state
                .spinorama_eq_state
                .speakers_cached_at = Some(std::time::Instant::now());
        });
        cx.notify();
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

/// Arrange a hermetic recording fixture for RoomEQ UI E2E without loading
/// measurements into the RoomEQ model. The subsequent visible Load from
/// recording action owns the production conversion and wizard transition.
pub(super) fn qa_room_eq_ui_fixture(
    payload: serde_json::Value,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<()> {
    let invalid = payload.get("invalid").and_then(|value| value.as_str());
    if let Some(invalid) = invalid {
        // The visible Load from recording handler treats this as a completed
        // session, then rejects it because the channel has no measurement
        // result. This gives the rendered test a genuine recovery path rather
        // than injecting a RoomEQ error after the fact.
        let mut recording_state = RecordingState::default();
        recording_state.model.channel_recordings = match invalid {
            "missing-channel" => {
                let mut channel = ChannelRecording::new(0, "L".to_string());
                channel.state = ChannelRecordingState::Done;
                vec![channel]
            }
            "mismatched-grid" => vec![
                qa_done_recording(0, "L", vec![20.0, 100.0, 1_000.0, 20_000.0]),
                qa_done_recording(1, "R", vec![20.0, 125.0, 1_000.0, 20_000.0]),
            ],
            _ => {
                return Err(anyhow!(
                    "RoomEQ UI invalid fixture must be `missing-channel` or `mismatched-grid`, got `{invalid}`"
                ));
            }
        };
        return with_player_view(window, cx, |view, cx| {
            view.state.update(cx, |state, cx| {
                state.app.measurement_state.recording_state = recording_state;
                state.app.measurement_state.room_eq_state = Default::default();
                state.app.ui_state.current_screen = crate::app::Screen::RoomEq;
                state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                cx.notify();
            });
            Ok(())
        });
    }

    let fixture_dir = payload
        .get("fixture_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("room-eq UI payload needs `fixture_dir` string"))?;
    let fixture_dir = PathBuf::from(fixture_dir);
    if !fixture_dir.is_dir() {
        return Err(anyhow!(
            "RoomEQ fixture directory does not exist: {}",
            fixture_dir.display()
        ));
    }
    let recording_state = load_room_eq_recording_fixture(&fixture_dir)?;
    with_player_view(window, cx, |view, cx| {
        view.state.update(cx, |state, cx| {
            state.app.measurement_state.recording_state = recording_state;
            state.app.measurement_state.room_eq_state = Default::default();
            state.app.ui_state.current_screen = crate::app::Screen::RoomEq;
            state.app.ui_state.input_mode = crate::app::InputMode::Normal;
            cx.notify();
        });
        Ok(())
    })
}

fn qa_done_recording(
    channel_index: usize,
    channel_name: &str,
    frequencies: Vec<f32>,
) -> ChannelRecording {
    let mut channel = ChannelRecording::new(channel_index, channel_name.to_string());
    channel.state = ChannelRecordingState::Done;
    channel.result = Some(RecordingResult {
        channel: channel_index,
        wav_path: None,
        csv_path: None,
        magnitude_db: vec![0.0; frequencies.len()],
        phase_deg: vec![0.0; frequencies.len()],
        frequencies,
        impulse_response: None,
        impulse_time_ms: None,
        thd_percent: None,
        harmonic_distortion_db: None,
        excess_group_delay_ms: None,
        rt60_ms: None,
        clarity_c50_db: None,
        clarity_c80_db: None,
        spectrogram_db: None,
        quality: None,
    });
    channel
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
