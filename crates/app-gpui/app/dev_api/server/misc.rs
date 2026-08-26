use super::super::registry;
use crate::app::types::{
    ChannelMapping, ChannelRecording, ChannelRecordingState, RecordingResult, RecordingState,
    RecordingStep,
};
use anyhow::{Result, anyhow};
use gpui::App;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(super) const REPLY_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) const POLL_INTERVAL: Duration = Duration::from_millis(20);

pub(super) fn default_room_eq_export_path() -> PathBuf {
    sotf_audio_player::config::get_app_config_dir()
        .unwrap_or_else(|| std::env::temp_dir().join("sotf-qa"))
        .join("qa-room-eq-export.json")
}

pub(super) fn room_eq_export_summary_for_path(path: &Path) -> Result<serde_json::Value> {
    let bytes = std::fs::metadata(path)?.len();
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let channel_count = json
        .get("channels")
        .and_then(|v| v.as_object())
        .map(|channels| channels.len())
        .unwrap_or(0);
    let global_plugin_count = json
        .get("global_plugins")
        .and_then(|v| v.as_array())
        .map(|plugins| plugins.len())
        .unwrap_or(0);
    let channel_plugins: Vec<&serde_json::Value> = json
        .get("channels")
        .and_then(|v| v.as_object())
        .map(|channels| {
            channels
                .values()
                .filter_map(|channel| channel.get("plugins").and_then(|v| v.as_array()))
                .flat_map(|plugins| plugins.iter())
                .collect()
        })
        .unwrap_or_default();
    let channel_plugin_count = channel_plugins.len();
    let filter_count = channel_plugins
        .iter()
        .filter_map(|plugin| plugin.get("parameters"))
        .filter_map(|params| params.get("filters"))
        .filter_map(|filters| filters.as_array())
        .map(|filters| filters.len())
        .sum::<usize>();

    Ok(serde_json::json!({
        "path": path,
        "exists": true,
        "bytes": bytes,
        "version": json.get("version").and_then(|v| v.as_str()),
        "channel_count": channel_count,
        "plugin_count": global_plugin_count + channel_plugin_count,
        "filter_count": filter_count,
    }))
}

pub(super) fn load_room_eq_recording_fixture(fixture_dir: &Path) -> Result<RecordingState> {
    let recordings_path = fixture_dir.join("recordings.json");
    if !recordings_path.is_file() {
        return Err(anyhow!(
            "RoomEQ fixture is missing recordings.json: {}",
            recordings_path.display()
        ));
    }
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&recordings_path)?)
        .map_err(|e| anyhow!("invalid recordings.json: {e}"))?;
    let speakers = json
        .get("speakers")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow!("recordings.json needs `speakers` object"))?;

    let mut names: Vec<String> = speakers.keys().cloned().collect();
    names.sort_by_key(|name| channel_sort_key(name));
    if names.is_empty() {
        return Err(anyhow!("recordings.json contains no speakers"));
    }

    let mut recording = RecordingState {
        model: sotf_audio_player::ui_models::recording::RecordingScreenModel {
            recording_directory: Some(fixture_dir.to_string_lossy().into_owned()),
            ..Default::default()
        },
        ..RecordingState::default()
    };
    recording.model.playback_config.num_channels = names.len();
    recording.model.playback_config.channel_mappings = names
        .iter()
        .enumerate()
        .map(|(idx, name)| ChannelMapping::single(idx + 1, name.clone()))
        .collect();
    recording.model.recording_config.num_channels = 1;
    recording.model.recording_config.channel_mappings = vec![0];

    recording.model.channel_recordings = names
        .iter()
        .enumerate()
        .map(|(idx, name)| -> Result<ChannelRecording> {
            let speaker = speakers
                .get(name)
                .ok_or_else(|| anyhow!("missing speaker `{name}`"))?;
            let frequencies = json_f32_array(speaker, "frequencies", name)?;
            let magnitude_db = json_f32_array(speaker, "magnitude_db", name)?;
            let phase_deg = json_f32_array(speaker, "phase_deg", name)?;
            if frequencies.len() != magnitude_db.len() || frequencies.len() != phase_deg.len() {
                return Err(anyhow!(
                    "speaker `{name}` has mismatched response lengths: frequencies={}, magnitude_db={}, phase_deg={}",
                    frequencies.len(),
                    magnitude_db.len(),
                    phase_deg.len()
                ));
            }

            let mut rec = ChannelRecording::new(idx, name.clone());
            rec.state = ChannelRecordingState::Done;
            rec.result = Some(RecordingResult {
                channel: idx,
                wav_path: fixture_child_path(fixture_dir, speaker.get("wav_path")),
                csv_path: fixture_child_path(fixture_dir, speaker.get("csv_path")),
                frequencies,
                magnitude_db,
                phase_deg,
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
            Ok(rec)
        })
        .collect::<Result<Vec<_>>>()?;

    recording.model.step = RecordingStep::Evaluating;
    recording.model.recording_progress = 1.0;
    recording.model.current_recording_channel = None;
    recording.model.status_message = format!("QA RoomEQ fixture loaded: {} channels", names.len());

    Ok(recording)
}

pub(super) fn json_f32_array(
    parent: &serde_json::Value,
    key: &str,
    speaker_name: &str,
) -> Result<Vec<f32>> {
    let array = parent
        .get(key)
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("speaker `{speaker_name}` needs `{key}` array"))?;
    array
        .iter()
        .enumerate()
        .map(|(idx, value)| {
            value
                .as_f64()
                .map(|v| v as f32)
                .ok_or_else(|| anyhow!("speaker `{speaker_name}` `{key}`[{idx}] is not numeric"))
        })
        .collect()
}

pub(super) fn fixture_child_path(root: &Path, value: Option<&serde_json::Value>) -> Option<String> {
    let raw = value.and_then(|v| v.as_str())?;
    let path = PathBuf::from(raw);
    let path = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    Some(path.to_string_lossy().into_owned())
}

pub(super) fn channel_sort_key(name: &str) -> (usize, String) {
    let rank = match name {
        "L" => 0,
        "R" => 1,
        "C" => 2,
        "LFE" | "Sub" | "SW" => 3,
        "SL" => 4,
        "SR" => 5,
        "BL" => 6,
        "BR" => 7,
        _ => 100,
    };
    (rank, name.to_string())
}

pub(super) fn resolve_action_name(name: &str, cx: &App) -> Result<String> {
    if name.contains("::") {
        return Ok(name.to_string());
    }
    let suffix = format!("::{name}");
    let matches: Vec<&str> = cx
        .all_action_names()
        .iter()
        .copied()
        .filter(|n| n.ends_with(&suffix) || *n == name)
        .collect();
    match matches.as_slice() {
        [] => Err(anyhow!("no action registered for `{name}`")),
        [only] => Ok((*only).to_string()),
        many => Err(anyhow!(
            "ambiguous action `{name}`: matched {} ({})",
            many.len(),
            many.join(", ")
        )),
    }
}

pub(super) fn split_path_query(full: &str) -> (&str, &str) {
    match full.find('?') {
        Some(i) => (&full[..i], &full[i + 1..]),
        None => (full, ""),
    }
}

pub(super) fn percent_decode(s: &str) -> String {
    // Minimal: handle %XX and `+` → space. Fine for ASCII paths.
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = &s[i + 1..i + 3];
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub(super) fn list_elements_json() -> String {
    let entries = registry::snapshot();
    let mut items = Vec::with_capacity(entries.len());
    for (selector, element) in entries {
        let bounds = element.bounds;
        let centre = bounds.center();
        let mut item = serde_json::json!({
            "selector": selector,
            "x": f32::from(bounds.origin.x),
            "y": f32::from(bounds.origin.y),
            "w": f32::from(bounds.size.width),
            "h": f32::from(bounds.size.height),
            "cx": f32::from(centre.x),
            "cy": f32::from(centre.y),
        });
        let Some(state) = item.as_object_mut() else {
            continue;
        };
        if let Some(enabled) = element.state.enabled {
            state.insert("enabled".into(), serde_json::json!(enabled));
        }
        if let Some(selected) = element.state.selected {
            state.insert("selected".into(), serde_json::json!(selected));
        }
        if let Some(expanded) = element.state.expanded {
            state.insert("expanded".into(), serde_json::json!(expanded));
        }
        items.push(item);
    }
    serde_json::json!({ "ok": true, "elements": items }).to_string()
}

pub(super) fn http_response(status: u16, body: &str) -> String {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        500 => "Internal Server Error",
        _ => "Unknown",
    };
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body,
    )
}
