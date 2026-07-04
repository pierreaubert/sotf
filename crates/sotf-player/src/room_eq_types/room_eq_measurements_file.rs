use super::ctc::ctc_config_path_for;
use super::misc::resolve_recording_wav_path;
use super::misc::sanitize_ctc_filename;
use super::types::ChannelMeasurement;
use super::types::DelayDetectionHints;
use super::types::MeasurementData;
use super::write::write_stereo_ir_wav;
use super::write::write_stereo_wav_from_mono_wavs;
use crate::recording_types::{
    ChannelRecording, ChannelRecordingState, CtcMatrixExportStrategy, DelayProbeResults,
    RecordingResult, TransferMatrixLoopbackRecording,
};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

/// Namespace for the room-EQ measurement helpers (CTC matrix export,
/// recordings.json loading, delay-detection hint extraction).
///
/// The historical struct used to also be the on-disk shape for the
/// `recordings.json` file, but every saver now writes
/// [`autoeq::RoomConfig`] directly via [`build_speakers_from_recordings`].
/// This type is kept as a unit struct so the long list of static helpers
/// retains a single namespace; nothing here is serialized.
#[derive(Debug, Clone, Copy, Default)]
pub struct RoomEqMeasurementsFile;

impl RoomEqMeasurementsFile {
    /// Build a CTC/N-by-M acoustic transfer-matrix manifest from completed
    /// multi-mic recordings. The current roomeq CTC solver consumes two-ear
    /// IR WAVs, so this exports mic 0 and mic 1 as a stereo impulse-response
    /// file for every completed `(head_position, speaker)` take.
    pub fn build_ctc_measurements_from_recordings(
        recordings: &[ChannelRecording],
        speaker_names: &[String],
        mic_names: &[String],
        sample_rate: u32,
        output_dir: &Path,
    ) -> Result<Option<autoeq::roomeq::CtcMeasurementConfig>, String> {
        Self::build_ctc_measurements_from_recordings_with_strategy(
            recordings,
            speaker_names,
            mic_names,
            sample_rate,
            output_dir,
            CtcMatrixExportStrategy::default(),
            None,
            &[],
        )
    }

    /// Same as [`Self::build_ctc_measurements_from_recordings`], but lets
    /// callers opt into exporting raw two-ear sweeps with a loopback channel.
    ///
    /// `loopback_mic_index` is only used by
    /// [`CtcMatrixExportStrategy::RawSweep`]. The raw-sweep strategy returns
    /// `Ok(None)` when the loopback channel or raw WAV paths are incomplete,
    /// preserving the default measured-IR behaviour for normal app saves.
    #[allow(
        clippy::too_many_arguments,
        reason = "export builder: one argument per input data source"
    )]
    pub fn build_ctc_measurements_from_recordings_with_strategy(
        recordings: &[ChannelRecording],
        speaker_names: &[String],
        mic_names: &[String],
        sample_rate: u32,
        output_dir: &Path,
        strategy: CtcMatrixExportStrategy,
        loopback_mic_index: Option<usize>,
        loopback_recordings: &[TransferMatrixLoopbackRecording],
    ) -> Result<Option<autoeq::roomeq::CtcMeasurementConfig>, String> {
        if mic_names.len() < 2 {
            return Ok(None);
        }

        let matrix_dir = output_dir.join("ctc_matrix");
        let mut by_take: BTreeMap<(usize, usize, usize), &RecordingResult> = BTreeMap::new();
        let max_mic_index = loopback_mic_index.unwrap_or(1).max(1);

        for rec in recordings {
            if rec.state != ChannelRecordingState::Done {
                continue;
            }
            let Some(result) = rec.result.as_ref() else {
                continue;
            };
            if rec.mic_index > max_mic_index {
                continue;
            }
            let usable = match strategy {
                CtcMatrixExportStrategy::ImpulseResponse => {
                    rec.mic_index <= 1
                        && result
                            .impulse_response
                            .as_ref()
                            .is_some_and(|ir| !ir.is_empty())
                }
                CtcMatrixExportStrategy::RawSweep => {
                    result.wav_path.as_ref().is_some_and(|p| !p.is_empty())
                        && (rec.mic_index <= 1 || Some(rec.mic_index) == loopback_mic_index)
                }
            };
            if !usable {
                continue;
            }
            by_take.insert(
                (rec.mic_position_index, rec.channel_index, rec.mic_index),
                result,
            );
        }

        if by_take.is_empty() {
            return Ok(None);
        }

        let mut candidate_speakers = BTreeSet::new();
        let mut candidate_positions = BTreeSet::new();
        for (position_idx, speaker_idx, mic_idx) in by_take.keys().copied() {
            if mic_idx <= 1 {
                candidate_speakers.insert(speaker_idx);
                candidate_positions.insert(position_idx);
            }
        }

        let has_complete_take = |position_idx: usize, speaker_idx: usize| -> bool {
            if !by_take.contains_key(&(position_idx, speaker_idx, 0))
                || !by_take.contains_key(&(position_idx, speaker_idx, 1))
            {
                return false;
            }
            match strategy {
                CtcMatrixExportStrategy::ImpulseResponse => true,
                CtcMatrixExportStrategy::RawSweep => {
                    loopback_recordings.iter().any(|r| {
                        r.speaker_index == speaker_idx
                            && r.mic_position_index == position_idx
                            && !r.wav_path.is_empty()
                    }) || loopback_mic_index.is_some_and(|loopback_idx| {
                        by_take
                            .get(&(position_idx, speaker_idx, loopback_idx))
                            .and_then(|r| r.wav_path.as_ref())
                            .is_some_and(|p| !p.is_empty())
                    })
                }
            }
        };

        let mut complete_pairs: HashSet<(usize, usize)> = HashSet::new();
        for position_idx in &candidate_positions {
            for speaker_idx in &candidate_speakers {
                if has_complete_take(*position_idx, *speaker_idx) {
                    complete_pairs.insert((*position_idx, *speaker_idx));
                }
            }
        }

        let mut speaker_indices: BTreeSet<usize> =
            complete_pairs.iter().map(|(_, speaker)| *speaker).collect();
        let mut position_indices: BTreeSet<usize> = complete_pairs
            .iter()
            .map(|(position, _)| *position)
            .collect();
        loop {
            let before = (speaker_indices.len(), position_indices.len());
            position_indices.retain(|position_idx| {
                speaker_indices
                    .iter()
                    .all(|speaker_idx| complete_pairs.contains(&(*position_idx, *speaker_idx)))
            });
            speaker_indices.retain(|speaker_idx| {
                position_indices
                    .iter()
                    .all(|position_idx| complete_pairs.contains(&(*position_idx, *speaker_idx)))
            });
            if before == (speaker_indices.len(), position_indices.len()) {
                break;
            }
        }

        let speakers: Vec<String> = speaker_indices
            .iter()
            .map(|idx| {
                speaker_names
                    .get(*idx)
                    .cloned()
                    .unwrap_or_else(|| format!("ch_{}", idx + 1))
            })
            .collect();
        if speakers.len() < 2 {
            return Ok(None);
        }
        if position_indices.is_empty() {
            return Ok(None);
        }

        std::fs::create_dir_all(&matrix_dir)
            .map_err(|e| format!("failed to create CTC matrix directory: {}", e))?;

        let mut used_positions = BTreeSet::new();
        let mut files = Vec::new();

        for position_idx in &position_indices {
            for speaker_idx in &speaker_indices {
                let left = by_take.get(&(*position_idx, *speaker_idx, 0));
                let right = by_take.get(&(*position_idx, *speaker_idx, 1));
                let (Some(left), Some(right)) = (left, right) else {
                    continue;
                };

                let speaker = speaker_names
                    .get(*speaker_idx)
                    .cloned()
                    .unwrap_or_else(|| format!("ch_{}", speaker_idx + 1));
                let head_position = format!("pos_{}", position_idx + 1);
                let safe_head = sanitize_ctc_filename(&head_position);
                let safe_speaker = sanitize_ctc_filename(&speaker);

                match strategy {
                    CtcMatrixExportStrategy::ImpulseResponse => {
                        let left_ir = left
                            .impulse_response
                            .as_ref()
                            .filter(|ir| !ir.is_empty())
                            .ok_or_else(|| {
                                "left-ear recording has no impulse response".to_string()
                            })?;
                        let right_ir = right
                            .impulse_response
                            .as_ref()
                            .filter(|ir| !ir.is_empty())
                            .ok_or_else(|| {
                                "right-ear recording has no impulse response".to_string()
                            })?;
                        let file_name = format!("{}_{}_ears_ir.wav", safe_head, safe_speaker);
                        let abs_path = matrix_dir.join(&file_name);
                        write_stereo_ir_wav(&abs_path, left_ir, right_ir, sample_rate)?;

                        used_positions.insert(*position_idx);
                        files.push(autoeq::roomeq::CtcMeasurementFileConfig {
                            head_position,
                            speaker,
                            ir: Some(PathBuf::from("ctc_matrix").join(file_name)),
                            raw_sweep: None,
                            loopback: None,
                        });
                    }
                    CtcMatrixExportStrategy::RawSweep => {
                        let left_wav = left.wav_path.as_ref().filter(|p| !p.is_empty());
                        let right_wav = right.wav_path.as_ref().filter(|p| !p.is_empty());
                        let loopback_wav = loopback_recordings
                            .iter()
                            .find(|r| {
                                r.speaker_index == *speaker_idx
                                    && r.mic_position_index == *position_idx
                            })
                            .map(|r| &r.wav_path)
                            .or_else(|| {
                                let loopback_idx = loopback_mic_index?;
                                by_take
                                    .get(&(*position_idx, *speaker_idx, loopback_idx))
                                    .and_then(|r| r.wav_path.as_ref())
                            })
                            .filter(|p| !p.is_empty());
                        let (Some(left_wav), Some(right_wav), Some(loopback_wav)) =
                            (left_wav, right_wav, loopback_wav)
                        else {
                            continue;
                        };
                        let left_wav = resolve_recording_wav_path(left_wav, output_dir);
                        let right_wav = resolve_recording_wav_path(right_wav, output_dir);
                        let loopback_wav = resolve_recording_wav_path(loopback_wav, output_dir);
                        let file_name =
                            format!("{}_{}_ears_raw_sweep.wav", safe_head, safe_speaker);
                        let abs_path = matrix_dir.join(&file_name);
                        write_stereo_wav_from_mono_wavs(
                            &abs_path,
                            &left_wav,
                            &right_wav,
                            sample_rate,
                        )?;

                        used_positions.insert(*position_idx);
                        files.push(autoeq::roomeq::CtcMeasurementFileConfig {
                            head_position,
                            speaker,
                            ir: None,
                            raw_sweep: Some(PathBuf::from("ctc_matrix").join(file_name)),
                            loopback: Some(ctc_config_path_for(&loopback_wav, output_dir)),
                        });
                    }
                }
            }
        }

        if files.len() < 2 {
            return Ok(None);
        }

        let head_positions = used_positions
            .into_iter()
            .map(|idx| autoeq::roomeq::CtcHeadPositionConfig {
                id: format!("pos_{}", idx + 1),
                x: 0.0,
                y: 0.0,
                z: 0.0,
                yaw_deg: 0.0,
            })
            .collect();

        Ok(Some(autoeq::roomeq::CtcMeasurementConfig {
            speakers,
            mics: mic_names.iter().take(2).cloned().collect(),
            head_positions,
            files,
        }))
    }

    /// Try to extract the delay-detection hints from an
    /// `autoeq::RoomConfig` JSON blob. Returns the canonical channel
    /// name order and sample rate the recording session was captured
    /// at, so the Delay Detection step can align its probe with the
    /// same device settings the user measured with.
    ///
    /// Returns `None` when the file does not carry the config — that
    /// means the file was recorded without session metadata and the
    /// caller should fall back to defaults (0..N indices, 48 000 Hz).
    pub fn extract_delay_detection_hints(json: &str) -> Option<DelayDetectionHints> {
        let room_config = serde_json::from_str::<autoeq::RoomConfig>(json).ok()?;
        let rc = room_config.recording_config?;
        // The autoeq crate stores probe results as `ProbeResultsLegacy`
        // which is shape-compatible with the engine's `ProbeDelayResults`
        // (re-exported as `DelayProbeResults`). Translate via serde
        // round-trip so the player-layer type is what DelayDetectionState
        // expects.
        let probe_results = rc.probe_results.as_ref().and_then(|pr| {
            serde_json::to_string(pr)
                .ok()
                .and_then(|j| serde_json::from_str::<DelayProbeResults>(&j).ok())
        });
        Some(DelayDetectionHints {
            channel_names: rc.channel_names,
            sample_rate: rc.recording_sample_rate,
            playback_device_name: rc.playback_device_name,
            recording_device_name: rc.recording_device_name,
            probe_results,
        })
    }

    /// Load measurements from an `autoeq::RoomConfig` JSON file.
    ///
    /// `base_dir` is used to resolve relative wav/csv paths.  Pass the
    /// parent directory of the JSON file.
    pub fn load_from_json(
        json: &str,
        base_dir: Option<&std::path::Path>,
    ) -> Result<Vec<ChannelMeasurement>, String> {
        let room_config: autoeq::RoomConfig =
            serde_json::from_str(json).map_err(|e| format!("Parse error: {}", e))?;
        log::info!(
            "Loaded {} speakers (RoomConfig format)",
            room_config.speakers.len()
        );
        Ok(Self::channels_from_room_config(room_config, base_dir))
    }

    /// Convert an `autoeq::RoomConfig` into `Vec<ChannelMeasurement>`.
    pub(super) fn channels_from_room_config(
        room_config: autoeq::RoomConfig,
        base_dir: Option<&std::path::Path>,
    ) -> Vec<ChannelMeasurement> {
        let resolve_path = |rel: &str| -> String {
            match base_dir {
                Some(dir) => {
                    let abs = dir.join(rel);
                    if abs.exists() {
                        abs.to_string_lossy().to_string()
                    } else {
                        rel.to_string()
                    }
                }
                None => rel.to_string(),
            }
        };

        room_config
            .speakers
            .into_iter()
            .enumerate()
            .filter_map(|(idx, (channel_name, speaker_config))| {
                // Collect every MeasurementRef from the speaker config so
                // multi-position recordings (saved as
                // MeasurementSource::Multiple) round-trip with all takes
                // preserved. The first ref becomes the primary measurement;
                // any extras are stored as multi_mic_measurements.
                let measurement_refs: Vec<autoeq::read::MeasurementRef> = match speaker_config {
                    autoeq::SpeakerConfig::Single(source) => match source {
                        autoeq::MeasurementSource::Single(s) => vec![s.measurement],
                        autoeq::MeasurementSource::Multiple(m) => m.measurements,
                        autoeq::MeasurementSource::InMemory(_)
                        | autoeq::MeasurementSource::InMemoryMultiple(_) => Vec::new(),
                    },
                    _ => Vec::new(), // Groups not yet supported
                };

                let mut iter = measurement_refs.into_iter();
                let primary_ref = iter.next()?;

                // Build ChannelMeasurement from the primary MeasurementRef
                let (frequencies, magnitude_db, phase_deg, wav_path, csv_path) =
                    Self::load_measurement_ref(&primary_ref, &resolve_path);

                let multi_mic_measurements = iter
                    .map(|extra_ref| {
                        let (frequencies, magnitude_db, phase_deg, wav_path, csv_path) =
                            Self::load_measurement_ref(&extra_ref, &resolve_path);
                        RecordingResult {
                            channel: idx,
                            wav_path,
                            csv_path,
                            frequencies,
                            magnitude_db,
                            phase_deg,
                            impulse_response: None,
                            impulse_time_ms: None,
                            excess_group_delay_ms: None,
                            thd_percent: None,
                            harmonic_distortion_db: None,
                            rt60_ms: None,
                            clarity_c50_db: None,
                            clarity_c80_db: None,
                            spectrogram_db: None,
                        }
                    })
                    .filter(|res| !res.frequencies.is_empty())
                    .collect();

                Some(ChannelMeasurement {
                    channel_name,
                    measurement: RecordingResult {
                        channel: idx,
                        wav_path,
                        csv_path,
                        frequencies,
                        magnitude_db,
                        phase_deg,
                        impulse_response: None,
                        impulse_time_ms: None,
                        excess_group_delay_ms: None,
                        thd_percent: None,
                        harmonic_distortion_db: None,
                        rt60_ms: None,
                        clarity_c50_db: None,
                        clarity_c80_db: None,
                        spectrogram_db: None,
                    },
                    is_group: false,
                    group_drivers: Vec::new(),
                    multi_mic_measurements,
                })
            })
            .filter(|ch| !ch.measurement.frequencies.is_empty())
            .collect()
    }

    /// Load measurement data from any MeasurementRef variant (inline, named path, or bare path).
    /// Returns (frequencies, magnitude_db, phase_deg, wav_path, csv_path).
    pub(super) fn load_measurement_ref(
        measurement_ref: &autoeq::read::MeasurementRef,
        resolve_path: &dyn Fn(&str) -> String,
    ) -> MeasurementData {
        match measurement_ref {
            autoeq::read::MeasurementRef::Inline(data) => {
                let wav_path = data.wav_path.as_deref().map(resolve_path);
                let csv_path = data.csv_path.as_deref().map(resolve_path);

                if data.frequencies.is_empty() {
                    // Inline has no data — try loading from referenced CSV
                    if let Some(ref csv) = csv_path {
                        if let Some(loaded) = Self::load_curve_from_csv(csv) {
                            return (loaded.0, loaded.1, loaded.2, wav_path, csv_path);
                        }
                    }
                    (Vec::new(), Vec::new(), Vec::new(), wav_path, csv_path)
                } else {
                    (
                        data.frequencies.iter().map(|&f| f as f32).collect(),
                        data.magnitude_db.iter().map(|&m| m as f32).collect(),
                        data.phase_deg
                            .as_ref()
                            .map(|p| p.iter().map(|&v| v as f32).collect())
                            .unwrap_or_default(),
                        wav_path,
                        csv_path,
                    )
                }
            }
            autoeq::read::MeasurementRef::Named { path, .. } => {
                let csv_str = resolve_path(&path.to_string_lossy());
                let loaded = Self::load_curve_from_csv(&csv_str);
                let (freq, mag, phase) = loaded.unwrap_or_default();
                (freq, mag, phase, None, Some(csv_str))
            }
            autoeq::read::MeasurementRef::Path(path) => {
                let csv_str = resolve_path(&path.to_string_lossy());
                let loaded = Self::load_curve_from_csv(&csv_str);
                let (freq, mag, phase) = loaded.unwrap_or_default();
                (freq, mag, phase, None, Some(csv_str))
            }
        }
    }

    /// Load a curve from a CSV file, returning (frequencies, magnitude_db, phase_deg).
    pub(super) fn load_curve_from_csv(csv_path: &str) -> Option<(Vec<f32>, Vec<f32>, Vec<f32>)> {
        let path = std::path::PathBuf::from(csv_path);
        match autoeq::read::read_curve_from_csv(&path) {
            Ok(curve) => {
                log::info!(
                    "Loaded {} frequency points from CSV: {}",
                    curve.freq.len(),
                    csv_path
                );
                Some((
                    curve.freq.iter().map(|&f| f as f32).collect(),
                    curve.spl.iter().map(|&s| s as f32).collect(),
                    curve
                        .phase
                        .map(|p| p.iter().map(|&v| v as f32).collect())
                        .unwrap_or_default(),
                ))
            }
            Err(e) => {
                log::warn!("Failed to load CSV '{}': {}", csv_path, e);
                None
            }
        }
    }
}
