use super::{ChannelRecording, ChannelRecordingState};
use crate::room_eq_types::{ChannelMeasurement, RoomEqMeasurementsFile};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Address of one source in the imported speaker/driver hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RecordingSourceIdentity {
    pub speaker: String,
    pub driver_index: Option<usize>,
    pub measurement_index: usize,
}

/// A complete import, retaining source topology alongside resolved responses.
/// Build this before replacing a wizard session; loading can fail partway through
/// a file and must never publish only the successfully loaded channels.
#[derive(Debug, Clone)]
pub struct RecordingImport {
    pub configuration: autoeq::RoomConfig,
    pub channels: Vec<ChannelMeasurement>,
}

impl RecordingImport {
    /// Preserve driver/position topology while handing current takes to RoomEQ.
    /// Validate the complete source set before publishing any measurements.
    pub fn channels_with_recordings(
        &self,
        recordings: &[ChannelRecording],
    ) -> Result<Vec<ChannelMeasurement>, String> {
        self.configuration_with_recordings(recordings)?;
        let mut channels = self.channels.clone();
        for recording in recordings {
            let address = recording
                .imported_source
                .as_ref()
                .ok_or("Missing source identity")?;
            let result = recording
                .result
                .as_ref()
                .ok_or("Missing recording result")?;
            let channel = channels
                .iter_mut()
                .find(|channel| channel.channel_name == address.speaker)
                .ok_or("Imported speaker unavailable")?;
            let (primary, additional) = if let Some(index) = address.driver_index {
                let driver = channel
                    .driver_measurement_sets
                    .get_mut(index)
                    .ok_or("Imported driver unavailable")?;
                (&mut driver.measurement, &mut driver.multi_mic_measurements)
            } else {
                (
                    &mut channel.measurement,
                    &mut channel.multi_mic_measurements,
                )
            };
            if address.measurement_index == 0 {
                *primary = result.clone();
            } else {
                *additional
                    .get_mut(address.measurement_index - 1)
                    .ok_or("Imported position unavailable")? = result.clone();
            }
        }
        for channel in &mut channels {
            if let Some(driver) = channel.driver_measurement_sets.first() {
                channel.measurement = driver.measurement.clone();
            }
        }
        Ok(channels)
    }

    /// Original capture identity, including unknown microphone/position indices.
    pub fn source_provenance(
        &self,
        source: &RecordingSourceIdentity,
    ) -> Option<&crate::room_eq_types::MeasurementProvenance> {
        let channel = self
            .channels
            .iter()
            .find(|channel| channel.channel_name == source.speaker)?;
        let provenance = match source.driver_index {
            Some(driver) => &channel.driver_measurement_sets.get(driver)?.provenance,
            None if !channel.is_group => &channel.provenance,
            None => return None,
        };
        provenance.get(source.measurement_index)
    }

    /// Replace source responses without flattening driver or speaker topology.
    /// All original source addresses must be present exactly once.
    pub fn configuration_with_recordings(
        &self,
        recordings: &[ChannelRecording],
    ) -> Result<autoeq::RoomConfig, String> {
        let expected: std::collections::HashSet<_> = self
            .recordings()
            .into_iter()
            .filter_map(|recording| recording.imported_source)
            .collect();
        let mut remaining = expected;
        let mut configuration = self.configuration.clone();
        for recording in recordings {
            let address = recording
                .imported_source
                .as_ref()
                .ok_or("Imported recording has no source identity")?;
            if !remaining.remove(address) {
                return Err(
                    "Imported recording source is duplicated or does not belong to this session"
                        .into(),
                );
            }
            let result = recording
                .result
                .as_ref()
                .filter(|result| !result.frequencies.is_empty())
                .ok_or("Imported recording has no response data")?;
            if result.frequencies.len() != result.magnitude_db.len()
                || (!result.phase_deg.is_empty()
                    && result.phase_deg.len() != result.frequencies.len())
                || result
                    .frequencies
                    .iter()
                    .chain(&result.magnitude_db)
                    .chain(&result.phase_deg)
                    .any(|value| !value.is_finite())
            {
                return Err("Imported recording response arrays are invalid".into());
            }
            let speaker = configuration
                .speakers
                .get_mut(&address.speaker)
                .ok_or("Imported speaker is unavailable")?;
            let source = match (speaker, address.driver_index) {
                (autoeq::SpeakerConfig::Single(source), None) => source,
                (autoeq::SpeakerConfig::Group(group), Some(index)) => group
                    .measurements
                    .get_mut(index)
                    .ok_or("Imported driver is unavailable")?,
                _ => return Err("Imported speaker topology changed".into()),
            };
            let measurement = match source {
                autoeq::MeasurementSource::Single(single) if address.measurement_index == 0 => {
                    &mut single.measurement
                }
                autoeq::MeasurementSource::Multiple(multiple) => multiple
                    .measurements
                    .get_mut(address.measurement_index)
                    .ok_or("Imported measurement position is unavailable")?,
                _ => return Err("Imported measurement topology changed".into()),
            };
            let name = measurement.name().map(str::to_owned);
            *measurement = autoeq::read::MeasurementRef::Inline(autoeq::read::InlineMeasurement {
                frequencies: result
                    .frequencies
                    .iter()
                    .map(|&value| f64::from(value))
                    .collect(),
                magnitude_db: result
                    .magnitude_db
                    .iter()
                    .map(|&value| f64::from(value))
                    .collect(),
                phase_deg: (!result.phase_deg.is_empty()).then(|| {
                    result
                        .phase_deg
                        .iter()
                        .map(|&value| f64::from(value))
                        .collect()
                }),
                name,
                wav_path: result.wav_path.clone(),
                csv_path: result.csv_path.clone(),
            });
        }
        if !remaining.is_empty() {
            return Err("One or more imported recording sources are missing".into());
        }
        Ok(configuration)
    }

    /// Present every source as a reviewable take without inventing capture
    /// microphone/position metadata. The source address distinguishes unknowns.
    pub fn recordings(&self) -> Vec<ChannelRecording> {
        let mut recordings = Vec::with_capacity(self.take_count());
        for (channel_index, channel) in self.channels.iter().enumerate() {
            let mut append =
                |driver_index: Option<usize>,
                 driver_name: Option<&str>,
                 primary: &super::RecordingResult,
                 extras: &[super::RecordingResult],
                 provenance: &[crate::room_eq_types::MeasurementProvenance]| {
                    for (measurement_index, result) in
                        std::iter::once(primary).chain(extras).enumerate()
                    {
                        let identity = provenance.get(measurement_index);
                        let name = identity
                            .and_then(|identity| identity.name.clone())
                            .unwrap_or_else(|| {
                                format!("{} / {}", channel.channel_name, measurement_index + 1)
                            });
                        let name = driver_index.map_or_else(
                            || name.clone(),
                            |driver| {
                                format!(
                                    "{} / {} / {name}",
                                    channel.channel_name,
                                    driver_name
                                        .map(str::to_owned)
                                        .unwrap_or_else(|| (driver + 1).to_string())
                                )
                            },
                        );
                        let mut recording = ChannelRecording::with_mic_position(
                            channel_index,
                            name,
                            identity
                                .and_then(|identity| identity.mic_index)
                                .unwrap_or(0),
                            identity
                                .and_then(|identity| identity.mic_position_index)
                                .unwrap_or(0),
                        );
                        recording.imported_source = Some(RecordingSourceIdentity {
                            speaker: channel.channel_name.clone(),
                            driver_index,
                            measurement_index,
                        });
                        let mut result = result.clone();
                        result.channel = channel_index;
                        recording.result = Some(result);
                        recording.state = ChannelRecordingState::Done;
                        recordings.push(recording);
                    }
                };
            if channel.is_group {
                for (driver_index, driver) in channel.driver_measurement_sets.iter().enumerate() {
                    append(
                        Some(driver_index),
                        driver.name.as_deref(),
                        &driver.measurement,
                        &driver.multi_mic_measurements,
                        &driver.provenance,
                    );
                }
            } else {
                append(
                    None,
                    None,
                    &channel.measurement,
                    &channel.multi_mic_measurements,
                    &channel.provenance,
                );
            }
        }
        recordings
    }

    pub fn from_json(json: &str, base_directory: Option<&Path>) -> Result<Self, String> {
        let mut configuration: autoeq::RoomConfig =
            serde_json::from_str(json).map_err(|error| format!("Parse error: {error}"))?;
        if configuration.speakers.is_empty() {
            return Err("Recording file contains no speaker measurements".into());
        }
        if let Some(base_directory) = base_directory {
            configuration.resolve_paths(base_directory);
        }
        let resolved_json =
            serde_json::to_string(&configuration).map_err(|error| error.to_string())?;
        let mut channels = RoomEqMeasurementsFile::load_from_json(&resolved_json, None)?;
        if channels.len() != configuration.speakers.len() {
            return Err("One or more speaker source hierarchies could not be imported".into());
        }
        // RoomConfig uses a HashMap. Keep the review order stable across loads.
        channels.sort_by(|left, right| left.channel_name.cmp(&right.channel_name));
        Ok(Self {
            configuration,
            channels,
        })
    }

    pub fn take_count(&self) -> usize {
        self.channels
            .iter()
            .map(|channel| {
                if channel.is_group {
                    channel
                        .driver_measurement_sets
                        .iter()
                        .map(|driver| 1 + driver.multi_mic_measurements.len())
                        .sum()
                } else {
                    1 + channel.multi_mic_measurements.len()
                }
            })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_handoff_replaces_imported_system_and_crossovers() {
        let mut imported = RecordingImport::from_json(
            r#"{"version":"1.1.0","speakers":{"L":{"frequencies":[100,1000],"magnitude_db":[-3,0]}}}"#,
            None,
        )
        .unwrap();
        imported.configuration.system = Some(autoeq::roomeq::SystemConfig {
            model: autoeq::roomeq::SystemModel::HomeCinema,
            speakers: std::collections::HashMap::from([("L".into(), "L".into())]),
            subwoofers: None,
            bass_management: None,
            supporting_source_outputs: None,
        });
        imported.configuration.crossovers = Some(std::collections::HashMap::from([(
            "source-crossover".into(),
            autoeq::roomeq::CrossoverConfig {
                crossover_type: "LR48".into(),
                frequency: Some(55.0),
                frequencies: None,
                frequency_range: None,
            },
        )]));
        let mut recording = crate::recording_types::RecordingState::default();
        recording.channel_recordings = imported.recordings();
        let mut room = crate::ui_models::room_eq::RoomEqScreenModel::default();
        room.load_from_imported_recording(&recording, &imported)
            .unwrap();
        assert_eq!(
            serde_json::to_value(&room.imported_system).unwrap(),
            serde_json::to_value(&imported.configuration.system).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&room.imported_crossovers).unwrap(),
            serde_json::to_value(&imported.configuration.crossovers).unwrap()
        );
        let exported = room.to_room_config();
        assert_eq!(
            exported.crossovers.unwrap()["source-crossover"].frequency,
            Some(55.0)
        );

        // A rejected replacement must preserve the currently loaded metadata.
        recording.channel_recordings.clear();
        assert!(
            room.load_from_imported_recording(&recording, &imported)
                .is_err()
        );
        assert!(room.imported_system.is_some());
        assert!(room.imported_crossovers.is_some());

        // A valid source without these settings must not inherit the prior source.
        imported.configuration.system = None;
        imported.configuration.crossovers = None;
        recording.channel_recordings = imported.recordings();
        room.load_from_imported_recording(&recording, &imported)
            .unwrap();
        assert!(room.imported_system.is_none());
        assert!(room.imported_crossovers.is_none());
    }

    #[test]
    fn recording_import_preserves_each_position_and_source_configuration() {
        let json = r#"{
            "version": "1.1.0",
            "speakers": {
                "L": { "speaker_name": "Example speaker", "measurements": [
                    { "name": "Left seat", "frequencies": [100.0, 1000.0], "magnitude_db": [-3.0, 0.0] },
                    { "name": "Right seat", "frequencies": [100.0, 1000.0], "magnitude_db": [-6.0, -1.0] }
                ] }
            }
        }"#;
        let imported = RecordingImport::from_json(json, None).unwrap();
        assert_eq!(imported.take_count(), 2);
        let channel = &imported.channels[0];
        assert_eq!(channel.provenance[0].name.as_deref(), Some("Left seat"));
        assert_eq!(channel.provenance[1].name.as_deref(), Some("Right seat"));
        assert_eq!(channel.multi_mic_measurements[0].magnitude_db, [-6.0, -1.0]);
        let autoeq::SpeakerConfig::Single(autoeq::MeasurementSource::Multiple(source)) =
            &imported.configuration.speakers["L"]
        else {
            panic!("source hierarchy lost")
        };
        assert_eq!(source.speaker_name.as_deref(), Some("Example speaker"));
        assert_eq!(source.measurements.len(), 2);
        let mut recordings = imported.recordings();
        assert_eq!(recordings.len(), 2);
        assert_eq!(recordings[0].channel_name, "Left seat");
        assert_eq!(recordings[1].channel_name, "Right seat");
        // Both sources have unknown capture indices. Their imported addresses
        // must still keep explicit take acceptance independent.
        assert_eq!(recordings[0].mic_index, recordings[1].mic_index);
        assert_eq!(
            recordings[0].mic_position_index,
            recordings[1].mic_position_index
        );
        let mut review = crate::ui_models::take_review::TakeReviewState::default();
        assert!(review.accept(&mut recordings[0]));
        assert!(review.is_accepted(&recordings[0]));
        assert!(!review.is_accepted(&recordings[1]));
        assert!(!review.all_accepted(&recordings));
        assert!(review.accept(&mut recordings[1]));
        assert!(review.all_accepted(&recordings));
        recordings[1].result.as_mut().unwrap().magnitude_db[0] = -8.0;
        let saved = imported.configuration_with_recordings(&recordings).unwrap();
        let reloaded =
            RecordingImport::from_json(&serde_json::to_string(&saved).unwrap(), None).unwrap();
        assert_eq!(reloaded.take_count(), 2);
        assert_eq!(reloaded.channels[0].measurement.magnitude_db[0], -3.0);
        assert_eq!(
            reloaded.channels[0].multi_mic_measurements[0].magnitude_db[0],
            -8.0
        );
        assert_eq!(
            reloaded.channels[0].provenance[1].name.as_deref(),
            Some("Right seat")
        );
        assert!(
            imported
                .configuration_with_recordings(&recordings[..1])
                .is_err()
        );
        let duplicated = vec![recordings[0].clone(), recordings[0].clone()];
        assert!(imported.configuration_with_recordings(&duplicated).is_err());
    }

    #[test]
    fn recording_import_rejects_empty_session() {
        let error =
            RecordingImport::from_json(r#"{"version":"1.1.0","speakers":{}}"#, None).unwrap_err();
        assert_eq!(error, "Recording file contains no speaker measurements");
    }

    #[test]
    fn recording_import_anchors_measurement_and_ctc_paths_before_resave() {
        let source_directory = tempfile::tempdir().unwrap();
        let json = r#"{
            "version": "1.1.0",
            "speakers": { "L": {
                "frequencies": [100.0, 1000.0], "magnitude_db": [-3.0, 0.0],
                "wav_path": "capture.wav"
            } },
            "ctc": { "reference_sweep": "sweep.wav" }
        }"#;
        let imported = RecordingImport::from_json(json, Some(source_directory.path())).unwrap();
        let recording = crate::recording_types::RecordingState {
            channel_recordings: imported.recordings(),
            recording_directory: Some(source_directory.path().to_string_lossy().into_owned()),
            ..Default::default()
        };
        let mut room = crate::ui_models::room_eq::RoomEqScreenModel::default();
        room.load_from_imported_recording(&recording, &imported)
            .unwrap();
        assert_eq!(
            room.ctc_config.as_ref().unwrap().reference_sweep,
            Some(source_directory.path().join("sweep.wav"))
        );
        assert_eq!(
            std::fs::read_dir(source_directory.path()).unwrap().count(),
            0
        );
        let saved = imported
            .configuration_with_recordings(&imported.recordings())
            .unwrap();
        assert_eq!(
            saved.ctc.as_ref().unwrap().reference_sweep.as_deref(),
            Some(source_directory.path().join("sweep.wav").as_path())
        );
        let destination = tempfile::tempdir().unwrap();
        let reloaded = RecordingImport::from_json(
            &serde_json::to_string(&saved).unwrap(),
            Some(destination.path()),
        )
        .unwrap();
        assert_eq!(
            reloaded.recordings()[0]
                .result
                .as_ref()
                .unwrap()
                .wav_path
                .as_deref(),
            source_directory.path().join("capture.wav").to_str()
        );
        assert_eq!(
            reloaded.configuration.ctc.as_ref().unwrap().reference_sweep,
            saved.ctc.as_ref().unwrap().reference_sweep
        );
    }

    #[test]
    fn recording_import_resaves_each_group_driver_position() {
        let json = r#"{
            "version": "1.1.0",
            "speakers": { "L": {
                "name": "Left speaker", "crossover": "main",
                "measurements": [
                    { "speaker_name": "Woofer", "measurements": [
                        { "name": "Seat 1", "frequencies": [100.0, 1000.0], "magnitude_db": [-3.0, 0.0] },
                        { "name": "Seat 2", "frequencies": [100.0, 1000.0], "magnitude_db": [-4.0, -1.0] }
                    ] },
                    { "speaker_name": "Tweeter", "measurements": [
                        { "name": "Seat 1", "frequencies": [100.0, 1000.0], "magnitude_db": [-5.0, -2.0] },
                        { "name": "Seat 2", "frequencies": [100.0, 1000.0], "magnitude_db": [-6.0, -3.0] }
                    ] }
                ]
            } }
        }"#;
        let mut configuration: autoeq::RoomConfig = serde_json::from_str(json).unwrap();
        configuration.recording_config = Some(autoeq::roomeq::RecordingConfiguration {
            recording_sample_rate: Some(96_000),
            ..Default::default()
        });
        let imported =
            RecordingImport::from_json(&serde_json::to_string(&configuration).unwrap(), None)
                .unwrap();
        assert_eq!(imported.take_count(), 4);
        let mut recordings = imported.recordings();
        assert!(recordings.iter().all(|recording| {
            recording
                .result
                .as_ref()
                .is_some_and(|result| result.sample_rate_hz == Some(96_000))
        }));
        assert_eq!(
            recordings[2].imported_source.as_ref().unwrap().driver_index,
            Some(1)
        );
        let mut review = crate::ui_models::take_review::TakeReviewState::default();
        review.accept(&mut recordings[0]);
        assert!(!review.is_accepted(&recordings[2]));
        recordings[3].result.as_mut().unwrap().magnitude_db[0] = -9.0;
        recordings[3].result.as_mut().unwrap().impulse_time_ms = Some(vec![0.0, 1.0]);
        recordings[3].result.as_mut().unwrap().impulse_response = Some(vec![1.0, 0.25]);
        let recording_state = crate::recording_types::RecordingState {
            channel_recordings: recordings.clone(),
            ..Default::default()
        };
        let mut room = crate::ui_models::room_eq::RoomEqScreenModel::default();
        room.load_from_imported_recording(&recording_state, &imported)
            .unwrap();
        assert_eq!(room.channel_measurements.len(), 1);
        assert!(room.has_multiple_measurements());
        assert_eq!(room.multi_position_counts, vec![("L".to_string(), 2)]);
        let channel = &room.channel_measurements[0];
        assert!(channel.is_group);
        assert_eq!(channel.driver_measurement_sets.len(), 2);
        let second_seat = &channel.driver_measurement_sets[1].multi_mic_measurements[0];
        assert_eq!(second_seat.magnitude_db[0], -9.0);
        assert_eq!(second_seat.sample_rate_hz, Some(96_000));
        assert_eq!(
            second_seat.impulse_response.as_deref(),
            Some([1.0, 0.25].as_slice())
        );
        let incomplete = crate::recording_types::RecordingState {
            channel_recordings: recordings[..recordings.len() - 1].to_vec(),
            ..Default::default()
        };
        assert!(
            room.load_from_imported_recording(&incomplete, &imported)
                .is_err()
        );
        assert_eq!(
            room.channel_measurements[0].driver_measurement_sets.len(),
            2
        );
        let saved = imported.configuration_with_recordings(&recordings).unwrap();
        let autoeq::SpeakerConfig::Group(group) = &saved.speakers["L"] else {
            panic!("driver grouping lost")
        };
        assert_eq!(group.name, "Left speaker");
        assert_eq!(group.crossover.as_deref(), Some("main"));
        let loaded =
            RecordingImport::from_json(&serde_json::to_string(&saved).unwrap(), None).unwrap();
        let takes = loaded.recordings();
        assert_eq!(takes.len(), 4);
        assert!(takes.iter().all(|recording| {
            recording
                .result
                .as_ref()
                .is_some_and(|result| result.sample_rate_hz == Some(96_000))
        }));
        assert_eq!(takes[0].result.as_ref().unwrap().magnitude_db[0], -3.0);
        assert_eq!(takes[2].result.as_ref().unwrap().magnitude_db[0], -5.0);
        assert_eq!(takes[3].result.as_ref().unwrap().magnitude_db[0], -9.0);
        assert_eq!(takes[3].imported_source, recordings[3].imported_source);
    }
}
