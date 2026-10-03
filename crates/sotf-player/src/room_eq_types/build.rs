use crate::recording_types::{ChannelRecording, ChannelRecordingState};
pub use autoeq::roomeq::DspChainOutput;
use std::collections::{BTreeMap, HashMap};

/// Build the per-channel speaker map for an `autoeq::RoomConfig` from a
/// finished recording session. All completed takes for the same
/// `channel_index` (every microphone, every measurement position) are
/// folded into a single `SpeakerConfig`, so roomeq emits one EQ chain per
/// real output channel instead of one per (channel × mic × position) take.
///
/// * `channel_names` is indexed by `channel_index` and supplies the bare
///   output name (e.g. `"L"`, `"R"`, `"LFE"`) used as the map key.
/// * `channel_speakers` (optional) maps a channel name to the speaker
///   model string (e.g. `"Genelec 8361A"`); when present it is recorded
///   as `speaker_name` in the resulting source so the optimizer / UI can
///   reference the catalog entry.
///
/// Each take is exported as `MeasurementRef::Inline` carrying the
/// session-relative `wav_path` / `csv_path` so the autoeq optimizer can
/// pick up the WAV for FDW analysis even though the SPL data lives in
/// the CSV file.
pub fn build_speakers_from_recordings(
    recordings: &[ChannelRecording],
    channel_names: &[String],
    channel_speakers: Option<&std::collections::HashMap<String, String>>,
) -> HashMap<String, autoeq::SpeakerConfig> {
    use autoeq::read::{InlineMeasurement, MeasurementMultiple, MeasurementRef, MeasurementSingle};
    use autoeq::{MeasurementSource, SpeakerConfig};

    let mut grouped: BTreeMap<usize, Vec<&ChannelRecording>> = BTreeMap::new();
    for rec in recordings {
        if rec.state != ChannelRecordingState::Done {
            continue;
        }
        if rec.result.is_none() {
            continue;
        }
        grouped.entry(rec.channel_index).or_default().push(rec);
    }

    let mut speakers: HashMap<String, SpeakerConfig> = HashMap::new();
    for (channel_index, mut group) in grouped {
        group.sort_by_key(|r| (r.mic_position_index, r.mic_index));

        let primary = group[0];
        let base_name = channel_names
            .get(channel_index)
            .cloned()
            .unwrap_or_else(|| {
                primary
                    .channel_name
                    .find(" (")
                    .map_or(primary.channel_name.as_str(), |pos| {
                        &primary.channel_name[..pos]
                    })
                    .to_string()
            });

        let measurement_refs: Vec<MeasurementRef> = group
            .iter()
            .filter_map(|rec| {
                let result = rec.result.as_ref()?;
                let relative_wav = result
                    .wav_path
                    .as_ref()
                    .and_then(|p| std::path::Path::new(p).file_name())
                    .map(|f| f.to_string_lossy().to_string());
                let relative_csv = result
                    .csv_path
                    .as_ref()
                    .and_then(|p| std::path::Path::new(p).file_name())
                    .map(|f| f.to_string_lossy().to_string());
                if relative_wav.is_none() && relative_csv.is_none() {
                    return None;
                }
                Some(MeasurementRef::Inline(InlineMeasurement {
                    frequencies: Vec::new(),
                    magnitude_db: Vec::new(),
                    phase_deg: None,
                    name: Some(rec.channel_name.clone()),
                    wav_path: relative_wav,
                    csv_path: relative_csv,
                }))
            })
            .collect();

        if measurement_refs.is_empty() {
            continue;
        }

        let speaker_name = channel_speakers
            .and_then(|m| m.get(&base_name))
            .map(|s| s.to_string());

        let source = if measurement_refs.len() == 1 {
            MeasurementSource::Single(MeasurementSingle {
                measurement: measurement_refs.into_iter().next().unwrap(),
                speaker_name,
                provenance: Default::default(),
            })
        } else {
            MeasurementSource::Multiple(MeasurementMultiple {
                measurements: measurement_refs,
                speaker_name,
                provenance: Default::default(),
            })
        };

        speakers.insert(base_name, SpeakerConfig::Single(source));
    }
    speakers
}

pub use sotf_room_eq_graph::build_room_eq_plugin_graph_config;
