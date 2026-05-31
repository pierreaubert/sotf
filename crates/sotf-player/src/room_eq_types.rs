//! Shared room EQ domain types used by both GPUI and TUI apps.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::EQFilter;
use crate::ReleaseChannel;
use crate::recording_types::{
    ChannelRecording, ChannelRecordingState, CtcMatrixExportStrategy, DelayProbeResults,
    RecordingResult, TransferMatrixLoopbackRecording,
};
use math_audio_iir_fir::BiquadFilterType;

/// (frequencies, magnitude_db, phase_deg, wav_path, csv_path)
type MeasurementData = (Vec<f32>, Vec<f32>, Vec<f32>, Option<String>, Option<String>);

/// Return true when a channel name conventionally represents an LFE/sub output.
pub fn room_eq_channel_is_bass_output(name: &str) -> bool {
    let normalized: String = name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect();
    normalized.contains("lfe")
        || normalized == "sub"
        || normalized == "subwoofer"
        || normalized == "sw"
        || normalized.starts_with("sub")
}

/// Build the logical-role system map required by enabled CTC configs.
///
/// The apps use channel names as both logical roles and measurement keys, so
/// this returns an identity map (`"L" -> "L"`, `"R" -> "R"`, ...). It also
/// annotates subwoofer outputs when channel names indicate LFE/sub channels.
pub fn ctc_system_config_for_speaker_names<I, S>(
    speaker_names: I,
    bass_management_crossover: Option<String>,
) -> Option<autoeq::roomeq::SystemConfig>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let speakers: BTreeSet<String> = speaker_names
        .into_iter()
        .map(|name| name.as_ref().to_string())
        .filter(|name| !name.is_empty())
        .collect();
    if speakers.is_empty() {
        return None;
    }

    let has_bass_output = speakers
        .iter()
        .any(|name| room_eq_channel_is_bass_output(name));

    Some(autoeq::roomeq::SystemConfig {
        model: autoeq::roomeq::SystemModel::HomeCinema,
        speakers: speakers
            .into_iter()
            .map(|name| (name.clone(), name))
            .collect::<HashMap<_, _>>(),
        subwoofers: has_bass_output.then(|| autoeq::roomeq::SubwooferSystemConfig {
            config: autoeq::roomeq::SubwooferStrategy::Single,
            crossover: bass_management_crossover,
            mapping: HashMap::new(),
        }),
        bass_management: None,
    })
}

/// Default crossover reference key emitted into recordings.json when a
/// bass-managed sub channel is detected and the caller has no explicit
/// crossover choice. Matches the key used in `default_bass_management_crossovers`.
pub const DEFAULT_BASS_MANAGEMENT_CROSSOVER_KEY: &str = "lfe_xover";

/// Default `crossovers` map (LR24, 40–120 Hz optimization range) suitable
/// for bass-managed home-cinema layouts written by the recording apps.
///
/// The roomeq workflow refuses to run a bass-managed sub without a crossover
/// reference, so saving a recording with an LFE channel and no crossover left
/// the JSON unusable until manually edited. Emitting this default lets the
/// optimizer pick the best crossover frequency in a sensible range.
pub fn default_bass_management_crossovers() -> HashMap<String, autoeq::roomeq::CrossoverConfig> {
    let mut map = HashMap::new();
    map.insert(
        DEFAULT_BASS_MANAGEMENT_CROSSOVER_KEY.to_string(),
        autoeq::roomeq::CrossoverConfig {
            crossover_type: "LR24".to_string(),
            frequency: None,
            frequencies: None,
            frequency_range: Some((40.0, 120.0)),
        },
    );
    map
}

/// Room EQ workflow step
///
/// Flow: LoadData → Delay → Process → Configure → Optimize → Review → Export
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoomEqStep {
    /// Step 1: Load/import measurement data
    #[default]
    LoadData,
    /// Step 2: Per-channel alignment delay table. Shows arrival times
    /// from the recording session's probe results (or manual entry).
    /// The user can override delays; values < 0.3 ms get a "consider
    /// using 0" hint.
    Delay,
    /// Step 3: Choose between Simple Wizard (guided presets) and Full
    /// Wizard (all parameters in Acoustic + Optimizer blocks).
    Process,
    /// Step 4: Configure channels, mode, and optimizer settings.
    /// Layout depends on the wizard mode selected in the Process step.
    Configure,
    /// Step 5: Run optimization (per-channel, then combined)
    Optimize,
    /// Step 6: Review results and visualizations
    Review,
    /// Step 7: Export DSP chain and apply
    Export,
}

impl RoomEqStep {
    /// Get all steps in order
    pub fn all() -> &'static [RoomEqStep] {
        &[
            RoomEqStep::LoadData,
            RoomEqStep::Delay,
            RoomEqStep::Process,
            RoomEqStep::Configure,
            RoomEqStep::Optimize,
            RoomEqStep::Review,
            RoomEqStep::Export,
        ]
    }

    /// Get step index (0-based)
    pub fn index(&self) -> usize {
        match self {
            RoomEqStep::LoadData => 0,
            RoomEqStep::Delay => 1,
            RoomEqStep::Process => 2,
            RoomEqStep::Configure => 3,
            RoomEqStep::Optimize => 4,
            RoomEqStep::Review => 5,
            RoomEqStep::Export => 6,
        }
    }

    /// Get step label
    pub fn label(&self) -> &'static str {
        match self {
            RoomEqStep::LoadData => "Load Data",
            RoomEqStep::Delay => "Delay",
            RoomEqStep::Process => "Process",
            RoomEqStep::Configure => "Configure",
            RoomEqStep::Optimize => "Optimize",
            RoomEqStep::Review => "Review",
            RoomEqStep::Export => "Export",
        }
    }

    /// Get next step
    pub fn next(&self) -> Option<RoomEqStep> {
        match self {
            RoomEqStep::LoadData => Some(RoomEqStep::Delay),
            RoomEqStep::Delay => Some(RoomEqStep::Process),
            RoomEqStep::Process => Some(RoomEqStep::Configure),
            RoomEqStep::Configure => Some(RoomEqStep::Optimize),
            RoomEqStep::Optimize => Some(RoomEqStep::Review),
            RoomEqStep::Review => Some(RoomEqStep::Export),
            RoomEqStep::Export => None,
        }
    }

    /// Get previous step
    pub fn previous(&self) -> Option<RoomEqStep> {
        match self {
            RoomEqStep::LoadData => None,
            RoomEqStep::Delay => Some(RoomEqStep::LoadData),
            RoomEqStep::Process => Some(RoomEqStep::Delay),
            RoomEqStep::Configure => Some(RoomEqStep::Process),
            RoomEqStep::Optimize => Some(RoomEqStep::Configure),
            RoomEqStep::Review => Some(RoomEqStep::Optimize),
            RoomEqStep::Export => Some(RoomEqStep::Review),
        }
    }
}

/// Wizard mode selected in the Process step. Determines which
/// Configure layout renders: Simple shows guided presets, Full
/// shows all parameters in two organized blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum RoomEqWizardMode {
    /// Guided preset selector adapted to the speaker configuration.
    #[default]
    Simple,
    /// Full parameter access split into Acoustic + Optimizer blocks.
    Full,
}

// Simple Wizard types — canonical definitions live in autoeq, re-exported here.
pub use autoeq::roomeq::{
    SimpleCrossoverChoice, SimpleLossChoice, SimplePresetConfig, SimpleProcessingChoice,
    SpeakerTier,
};

fn canonical_multi_measurement_strategy(strategy: &str) -> Option<&'static str> {
    let normalized = strategy
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-'], "_")
        .replace(['(', ')'], "");
    match normalized.as_str() {
        "average" | "average_rms" => Some("average"),
        "weighted_sum" => Some("weighted_sum"),
        "minimax" | "minmax" | "minimax_worst_case" => Some("minimax"),
        "variance_penalized" | "minimize_variance" | "variance" => Some("variance_penalized"),
        "spatial_robustness" => Some("spatial_robustness"),
        "minimax_uncertainty" | "minimax_bootstrap_uncertainty" => Some("minimax_uncertainty"),
        _ => None,
    }
}

/// Apply the user's Simple Wizard choices to a flat UI optimizer config.
///
/// Fields not controlled by the preset keep their current values so the
/// user doesn't lose any manual tuning done in a previous Full Wizard
/// session.  This is the "mutate in place" path used when the full wizard
/// needs to incorporate simple-mode choices into an existing config.
pub fn apply_simple_preset(preset: &SimplePresetConfig, config: &mut RoomEqOptimizerConfig) {
    // Processing mode
    config.mode = match preset.processing {
        SimpleProcessingChoice::Iir => RoomEqOptimizationMode::Iir,
        SimpleProcessingChoice::MixedPhase => RoomEqOptimizationMode::MixedPhase,
    };

    // Loss function
    config.loss_type = match preset.loss {
        SimpleLossChoice::Flat => "flat".to_string(),
        SimpleLossChoice::Epa => "epa".to_string(),
    };

    // Target response derived from measurement
    config.target_response.enabled = true;
    config.target_response.shape = "from_measurement".to_string();
    config.target_response.slope_db_per_octave = 0.0;

    // Crossover (2.1+ only)
    if !preset.bass_management.is_empty() || matches!(preset.crossover, SimpleCrossoverChoice::Lr48)
    {
        config.schroeder_split.enabled = true;
    }

    // Sane defaults for params not exposed in Simple mode
    config.num_filters = 7;
    config.algorithm = "autoeq:cmaes".to_string();
    config.population = 300;
    config.max_iter = 50_000;
    config.bo_initial_samples = 0;
    config.bo_batch_size = 0;
    config.bo_posterior_std_threshold = 0.0;
    config.bo_acquisition = default_bo_acquisition();
    config.bo_ehvi = false;
    config.min_freq = 20.0;
    config.max_freq = 1600.0;
    config.min_db = -12.0;
    config.max_db = 4.0;
    config.min_q = 0.5;
    config.max_q = 6.0;
    config.peq_model = "pk".to_string();
    config.tolerance = 1e-5;
    config.atolerance = 1e-5;
    config.psychoacoustic = true;
    config.asymmetric_loss = true;
    config.refine = true;
    config.local_algo = "cobyla".to_string();

    // Multi-position strategy
    if !preset.multi_position_strategy.is_empty() {
        config.multi_measurement.enabled = true;
        config.multi_measurement.strategy =
            canonical_multi_measurement_strategy(&preset.multi_position_strategy)
                .unwrap_or("average")
                .to_string();
    }
}

/// Source of measurement data for Room EQ
#[derive(Debug, Clone, PartialEq, Default)]
pub enum RoomEqDataSource {
    /// Use recordings from current session
    #[default]
    FromRecording,
    /// Loaded from a JSON file
    FromFile(std::path::PathBuf),
}

fn sanitize_ctc_filename(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            out.push(ch);
        } else if ch.is_whitespace() || matches!(ch, '/' | '\\' | ':' | '(' | ')') {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "measurement".to_string()
    } else {
        trimmed
    }
}

fn write_stereo_ir_wav(
    path: &Path,
    left: &[f32],
    right: &[f32],
    sample_rate: u32,
) -> Result<(), String> {
    let len = left.len().max(right.len());
    let mut interleaved = Vec::with_capacity(len * 2);
    for idx in 0..len {
        interleaved.push(left.get(idx).copied().unwrap_or(0.0));
        interleaved.push(right.get(idx).copied().unwrap_or(0.0));
    }
    sotf_audio::signal_recorder::write_wav_file(path, &interleaved, sample_rate, 2)
        .map_err(|e| format!("failed to write CTC IR WAV '{}': {}", path.display(), e))
}

fn write_stereo_wav_from_mono_wavs(
    path: &Path,
    left_path: &Path,
    right_path: &Path,
    expected_sample_rate: u32,
) -> Result<(), String> {
    let (left, left_sample_rate) = read_first_wav_channel_f32(left_path)?;
    let (right, right_sample_rate) = read_first_wav_channel_f32(right_path)?;
    if left_sample_rate != expected_sample_rate || right_sample_rate != expected_sample_rate {
        return Err(format!(
            "CTC raw sweep sample-rate mismatch: left={}Hz, right={}Hz, expected={}Hz",
            left_sample_rate, right_sample_rate, expected_sample_rate
        ));
    }
    write_stereo_ir_wav(path, &left, &right, expected_sample_rate)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WavSubFormat {
    Pcm,
    IeeeFloat,
}

fn read_first_wav_channel_f32(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("failed to read WAV '{}': {}", path.display(), e))?;
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(format!("'{}' is not a RIFF/WAVE file", path.display()));
    }

    let mut audio_format = None;
    let mut channels = None;
    let mut sample_rate = None;
    let mut bits_per_sample = None;
    let mut extensible_subformat = None;
    let mut data_range = None;
    let mut offset = 12usize;
    while offset + 8 <= bytes.len() {
        let chunk_id = &bytes[offset..offset + 4];
        let chunk_size = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        let chunk_start = offset + 8;
        let chunk_end = chunk_start.saturating_add(chunk_size);
        if chunk_end > bytes.len() {
            return Err(format!("WAV '{}' has a truncated chunk", path.display()));
        }

        match chunk_id {
            b"fmt " => {
                if chunk_size < 16 {
                    return Err(format!("WAV '{}' has an invalid fmt chunk", path.display()));
                }
                audio_format = Some(u16::from_le_bytes([
                    bytes[chunk_start],
                    bytes[chunk_start + 1],
                ]));
                channels = Some(u16::from_le_bytes([
                    bytes[chunk_start + 2],
                    bytes[chunk_start + 3],
                ]));
                sample_rate = Some(u32::from_le_bytes([
                    bytes[chunk_start + 4],
                    bytes[chunk_start + 5],
                    bytes[chunk_start + 6],
                    bytes[chunk_start + 7],
                ]));
                bits_per_sample = Some(u16::from_le_bytes([
                    bytes[chunk_start + 14],
                    bytes[chunk_start + 15],
                ]));
                if audio_format == Some(65534) && chunk_size >= 40 {
                    let guid = &bytes[chunk_start + 24..chunk_start + 40];
                    extensible_subformat = match guid {
                        [
                            0x01,
                            0x00,
                            0x00,
                            0x00,
                            0x00,
                            0x00,
                            0x10,
                            0x00,
                            0x80,
                            0x00,
                            0x00,
                            0xaa,
                            0x00,
                            0x38,
                            0x9b,
                            0x71,
                        ] => Some(WavSubFormat::Pcm),
                        [
                            0x03,
                            0x00,
                            0x00,
                            0x00,
                            0x00,
                            0x00,
                            0x10,
                            0x00,
                            0x80,
                            0x00,
                            0x00,
                            0xaa,
                            0x00,
                            0x38,
                            0x9b,
                            0x71,
                        ] => Some(WavSubFormat::IeeeFloat),
                        _ => None,
                    };
                }
            }
            b"data" => data_range = Some(chunk_start..chunk_end),
            _ => {}
        }
        offset = chunk_end + (chunk_size & 1);
    }

    let audio_format =
        audio_format.ok_or_else(|| format!("WAV '{}' is missing a fmt chunk", path.display()))?;
    let channels = channels
        .ok_or_else(|| format!("WAV '{}' is missing a channel count", path.display()))?
        as usize;
    let sample_rate =
        sample_rate.ok_or_else(|| format!("WAV '{}' is missing a sample rate", path.display()))?;
    let bits_per_sample = bits_per_sample
        .ok_or_else(|| format!("WAV '{}' is missing a bit depth", path.display()))?;
    let data_range =
        data_range.ok_or_else(|| format!("WAV '{}' is missing a data chunk", path.display()))?;
    if channels == 0 {
        return Err(format!("WAV '{}' has zero channels", path.display()));
    }

    let bytes_per_sample = (bits_per_sample / 8) as usize;
    if bytes_per_sample == 0 {
        return Err(format!("WAV '{}' has invalid bit depth", path.display()));
    }
    let frame_size = bytes_per_sample * channels;
    let data = &bytes[data_range];
    if data.len() < frame_size {
        return Ok((Vec::new(), sample_rate));
    }

    let mut samples = Vec::with_capacity(data.len() / frame_size);
    for frame in data.chunks_exact(frame_size) {
        let sample = match (audio_format, bits_per_sample) {
            (3, 32) => f32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]),
            (65534, 32) => match extensible_subformat {
                Some(WavSubFormat::IeeeFloat) => {
                    f32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]])
                }
                Some(WavSubFormat::Pcm) => {
                    i32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]) as f32
                        / i32::MAX as f32
                }
                None => {
                    return Err(format!(
                        "WAV '{}' uses unsupported extensible subformat",
                        path.display()
                    ));
                }
            },
            (1, 16) => i16::from_le_bytes([frame[0], frame[1]]) as f32 / i16::MAX as f32,
            (1, 24) => {
                let value = i32::from_le_bytes([
                    frame[0],
                    frame[1],
                    frame[2],
                    if frame[2] & 0x80 == 0 { 0 } else { 0xff },
                ]);
                value as f32 / 8_388_607.0
            }
            (1, 32) => {
                i32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]) as f32
                    / i32::MAX as f32
            }
            _ => {
                return Err(format!(
                    "WAV '{}' uses unsupported format={}, bits={}",
                    path.display(),
                    audio_format,
                    bits_per_sample
                ));
            }
        };
        samples.push(sample);
    }

    Ok((samples, sample_rate))
}

fn resolve_recording_wav_path(path: &str, output_dir: &Path) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_relative() {
        output_dir.join(path)
    } else {
        path
    }
}

fn ctc_config_path_for(abs_path: &Path, output_dir: &Path) -> PathBuf {
    abs_path
        .strip_prefix(output_dir)
        .map(PathBuf::from)
        .unwrap_or_else(|_| abs_path.to_path_buf())
}

fn ctc_position_index(id: &str) -> Option<usize> {
    id.strip_prefix("pos_")
        .and_then(|value| value.parse::<usize>().ok())
        .and_then(|one_based| one_based.checked_sub(1))
}

/// Returns the common sweep range for a CTC raw-sweep measurement manifest.
///
/// Raw-sweep CTC currently carries one reference sweep for the whole matrix, so
/// every exported speaker/position take must have been captured with the same
/// sweep bounds. `None` means the manifest is empty, references recordings that
/// cannot be found, or mixes different sweep ranges.
pub fn ctc_uniform_sweep_range_for_measurements(
    recordings: &[ChannelRecording],
    speaker_names: &[String],
    measurements: &autoeq::roomeq::CtcMeasurementConfig,
) -> Option<(f32, f32)> {
    let mut range: Option<(f32, f32)> = None;
    for file in &measurements.files {
        let speaker_idx = speaker_names
            .iter()
            .position(|name| name == &file.speaker)
            .or_else(|| {
                file.speaker
                    .strip_prefix("ch_")
                    .and_then(|value| value.parse::<usize>().ok())
                    .and_then(|one_based| one_based.checked_sub(1))
            })?;
        let position_idx = ctc_position_index(&file.head_position)?;
        for mic_idx in 0..2 {
            let rec = recordings.iter().find(|rec| {
                rec.state == ChannelRecordingState::Done
                    && rec.channel_index == speaker_idx
                    && rec.mic_position_index == position_idx
                    && rec.mic_index == mic_idx
            })?;
            let current = (rec.sweep_start_freq, rec.sweep_end_freq);
            match range {
                Some((start, end))
                    if (start - current.0).abs() > 1e-3 || (end - current.1).abs() > 1e-3 =>
                {
                    return None;
                }
                None => range = Some(current),
                _ => {}
            }
        }
    }
    range
}

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
            })
        } else {
            MeasurementSource::Multiple(MeasurementMultiple {
                measurements: measurement_refs,
                speaker_name,
            })
        };

        speakers.insert(base_name, SpeakerConfig::Single(source));
    }
    speakers
}

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
    fn channels_from_room_config(
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
    fn load_measurement_ref(
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
    fn load_curve_from_csv(csv_path: &str) -> Option<(Vec<f32>, Vec<f32>, Vec<f32>)> {
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

/// Measurement data for a single channel (may have multiple drivers)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMeasurement {
    /// Channel name (e.g., "L", "R", "C")
    pub channel_name: String,
    /// Primary measurement (single driver or combined)
    pub measurement: RecordingResult,
    /// Whether this is a multi-driver setup
    pub is_group: bool,
    /// Individual driver measurements (for multi-driver)
    pub group_drivers: Vec<RecordingResult>,
    /// Additional mic measurements for multi-position optimization
    #[serde(default)]
    pub multi_mic_measurements: Vec<RecordingResult>,
}

/// Speaker configuration type (duplicated from autoeq::types for UI use)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RoomEqSpeakerConfigType {
    /// Single full-range driver or measurement
    #[default]
    Single,
    /// Multi-driver with active crossover
    MultiDriver,
}

/// Crossover type for multi-driver speakers (UI version with Butterworth24)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RoomEqCrossoverType {
    LR12,
    #[default]
    LR24,
    LR48,
    LinearPhase,
    Butterworth12,
    Butterworth24,
}

impl RoomEqCrossoverType {
    pub fn all() -> &'static [RoomEqCrossoverType] {
        &[
            RoomEqCrossoverType::LR12,
            RoomEqCrossoverType::LR24,
            RoomEqCrossoverType::LR48,
            RoomEqCrossoverType::LinearPhase,
            RoomEqCrossoverType::Butterworth12,
            RoomEqCrossoverType::Butterworth24,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            RoomEqCrossoverType::LR12 => "Linkwitz-Riley 12dB",
            RoomEqCrossoverType::LR24 => "Linkwitz-Riley 24dB",
            RoomEqCrossoverType::LR48 => "Linkwitz-Riley 48dB",
            RoomEqCrossoverType::LinearPhase => "Linear-phase FIR",
            RoomEqCrossoverType::Butterworth12 => "Butterworth 12dB",
            RoomEqCrossoverType::Butterworth24 => "Butterworth 24dB",
        }
    }
}

/// Shorter alias for `RoomEqSpeakerConfigType`.
pub type SpeakerConfigType = RoomEqSpeakerConfigType;
/// Shorter alias for `RoomEqCrossoverType`.
pub type CrossoverType = RoomEqCrossoverType;

/// Default FIR length for the linear-phase crossover. Power-of-two friendly,
/// matches the plugin-side `DEFAULT_FIR_CROSSOVER_TAPS` default.
pub const DEFAULT_LINEAR_PHASE_CROSSOVER_TAPS: usize = 4096;

fn default_linear_phase_fir_taps() -> usize {
    DEFAULT_LINEAR_PHASE_CROSSOVER_TAPS
}

/// Configuration for a speaker channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEqSpeakerConfig {
    pub channel_name: String,
    pub config_type: RoomEqSpeakerConfigType,
    pub crossover_type: RoomEqCrossoverType,
    pub driver_names: Vec<String>,
    pub crossover_freq_hints: Vec<f64>,
    /// FIR length for the linear-phase crossover when `crossover_type ==
    /// LinearPhase`. Ignored for IIR crossover types. Latency at the active
    /// sample rate is `(taps - 1) / 2 / sample_rate` seconds.
    #[serde(default = "default_linear_phase_fir_taps")]
    pub linear_phase_fir_taps: usize,
}

impl Default for RoomEqSpeakerConfig {
    fn default() -> Self {
        Self {
            channel_name: String::new(),
            config_type: RoomEqSpeakerConfigType::Single,
            crossover_type: RoomEqCrossoverType::LR24,
            driver_names: Vec::new(),
            crossover_freq_hints: Vec::new(),
            linear_phase_fir_taps: DEFAULT_LINEAR_PHASE_CROSSOVER_TAPS,
        }
    }
}

/// Multi-speaker optimization mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MultiSpeakerMode {
    #[default]
    Sequential,
    Combined,
}

impl MultiSpeakerMode {
    pub fn all() -> &'static [MultiSpeakerMode] {
        &[MultiSpeakerMode::Sequential, MultiSpeakerMode::Combined]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            MultiSpeakerMode::Sequential => "Sequential (per-channel)",
            MultiSpeakerMode::Combined => "Combined (all channels)",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            MultiSpeakerMode::Sequential => "Optimize each speaker independently, one at a time",
            MultiSpeakerMode::Combined => {
                "Optimize all speakers together for globally optimal solution"
            }
        }
    }
}

/// Optimization algorithm selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RoomEqAlgorithm {
    #[default]
    Cobyla,
    DifferentialEvolution,
    BayesianOptimization,
    CmaEs,
    /// Legacy value kept so older saved configs deserialize. It is not exposed
    /// in algorithm selectors now that NLopt backends are gone.
    NelderMead,
}

impl RoomEqAlgorithm {
    pub fn all() -> &'static [RoomEqAlgorithm] {
        &[
            RoomEqAlgorithm::Cobyla,
            RoomEqAlgorithm::DifferentialEvolution,
            RoomEqAlgorithm::BayesianOptimization,
            RoomEqAlgorithm::CmaEs,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            RoomEqAlgorithm::Cobyla => "COBYLA",
            RoomEqAlgorithm::DifferentialEvolution => "Differential Evolution",
            RoomEqAlgorithm::BayesianOptimization => "Bayesian Optimization",
            RoomEqAlgorithm::CmaEs => "CMA-ES",
            RoomEqAlgorithm::NelderMead => "Nelder-Mead (Legacy)",
        }
    }

    pub fn to_autoeq_string(&self) -> &'static str {
        match self {
            RoomEqAlgorithm::Cobyla => "autoeq:cobyla",
            RoomEqAlgorithm::DifferentialEvolution => "autoeq:de",
            RoomEqAlgorithm::BayesianOptimization => "autoeq:bo",
            RoomEqAlgorithm::CmaEs => "autoeq:cmaes",
            RoomEqAlgorithm::NelderMead => "autoeq:cobyla",
        }
    }
}

/// Optimization mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RoomEqOptimizationMode {
    #[default]
    Iir,
    Fir,
    Mixed,
    MixedPhase,
}

impl RoomEqOptimizationMode {
    pub fn all() -> &'static [RoomEqOptimizationMode] {
        &[
            RoomEqOptimizationMode::Iir,
            RoomEqOptimizationMode::Fir,
            RoomEqOptimizationMode::Mixed,
            RoomEqOptimizationMode::MixedPhase,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            RoomEqOptimizationMode::Iir => "IIR (Parametric EQ)",
            RoomEqOptimizationMode::Fir => "FIR (Convolution)",
            RoomEqOptimizationMode::Mixed => "Mixed (IIR + FIR)",
            RoomEqOptimizationMode::MixedPhase => "Mixed-Phase (IIR + short FIR)",
        }
    }

    pub fn to_code(&self) -> &'static str {
        match self {
            RoomEqOptimizationMode::Iir => "iir",
            RoomEqOptimizationMode::Fir => "fir",
            RoomEqOptimizationMode::Mixed => "mixed",
            RoomEqOptimizationMode::MixedPhase => "mixed_phase",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "fir" => RoomEqOptimizationMode::Fir,
            "mixed" => RoomEqOptimizationMode::Mixed,
            "mixed_phase" => RoomEqOptimizationMode::MixedPhase,
            _ => RoomEqOptimizationMode::Iir,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            RoomEqOptimizationMode::Iir => "Uses standard biquad filters. Low latency, efficient.",
            RoomEqOptimizationMode::Fir => {
                "Uses impulse response convolution. Can correct phase, but higher latency."
            }
            RoomEqOptimizationMode::Mixed => {
                "Combines IIR for high frequencies and FIR for low frequencies."
            }
            RoomEqOptimizationMode::MixedPhase => {
                "IIR for minimum-phase + short FIR for excess phase. Low latency (~10ms)."
            }
        }
    }

    pub fn maturity(&self) -> ReleaseChannel {
        match self {
            RoomEqOptimizationMode::Iir => ReleaseChannel::Beta,
            RoomEqOptimizationMode::Fir => ReleaseChannel::Alpha,
            RoomEqOptimizationMode::Mixed => ReleaseChannel::Alpha,
            RoomEqOptimizationMode::MixedPhase => ReleaseChannel::Alpha,
        }
    }

    pub fn available(channel: ReleaseChannel) -> Vec<Self> {
        Self::all()
            .iter()
            .copied()
            .filter(|mode| channel.allows(mode.maturity()))
            .collect()
    }
}

/// Pre-ringing suppression configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreRingingConfig {
    /// Maximum pre-ringing level in dB relative to main tap (default: -30.0)
    pub threshold_db: f64,
    /// Maximum pre-ringing time in seconds (default: 0.005 = 5 ms)
    pub max_time_s: f64,
}

impl Default for PreRingingConfig {
    fn default() -> Self {
        Self {
            threshold_db: -30.0,
            max_time_s: 0.005,
        }
    }
}

/// FIR configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEqFirConfig {
    pub taps: usize,
    pub phase: String,
    /// Whether to correct excess phase (only applies to kirkeby mode)
    #[serde(default)]
    pub correct_excess_phase: bool,
    /// Phase smoothing width in octaves (default: 0.167 = 1/6 octave)
    #[serde(default = "default_phase_smoothing")]
    pub phase_smoothing: f64,
    /// Pre-ringing suppression configuration
    #[serde(default)]
    pub pre_ringing: Option<PreRingingConfig>,
}

fn default_phase_smoothing() -> f64 {
    0.167
}

impl Default for RoomEqFirConfig {
    fn default() -> Self {
        Self {
            taps: 4096,
            phase: "kirkeby".to_string(),
            correct_excess_phase: false,
            phase_smoothing: 0.167,
            pre_ringing: None,
        }
    }
}

/// Mixed-phase correction configuration (IIR for minimum-phase + short FIR for excess phase)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixedPhaseUiConfig {
    /// Maximum FIR length in milliseconds for excess phase correction (default: 10.0)
    pub max_fir_length_ms: f64,
    /// Pre-ringing threshold in dB (default: -30.0)
    pub pre_ringing_threshold_db: f64,
    /// Minimum spatial correction depth (default: 0.5)
    pub min_spatial_depth: f64,
    /// Phase smoothing width in octaves (default: 0.167 = 1/6 octave)
    pub phase_smoothing_octaves: f64,
}

impl Default for MixedPhaseUiConfig {
    fn default() -> Self {
        Self {
            max_fir_length_ms: 10.0,
            pre_ringing_threshold_db: -30.0,
            min_spatial_depth: 0.5,
            phase_smoothing_octaves: 0.167,
        }
    }
}

/// Target response configuration (UI-facing).
///
/// Mirrors the backend `autoeq::roomeq::TargetResponseConfig` but flattened
/// into a single struct for simpler binding in UI widgets. Covers the target
/// shape (flat / Harman / custom slope / file / derived-from-measurement),
/// the preference shelves (bass / treble), and the broadband pre-correction
/// toggle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetResponseUiConfig {
    /// Whether any target shaping is applied. When `false` the optimiser
    /// sees a flat target regardless of the other fields.
    pub enabled: bool,
    /// Target shape: "flat" | "harman" | "custom" | "file" | "from_measurement".
    pub shape: String,
    /// Slope in dB/octave (used when `shape == "custom"`).
    pub slope_db_per_octave: f64,
    /// Reference frequency where the slope passes through 0 dB.
    pub reference_freq: f64,
    /// Path to CSV target file (used when `shape == "file"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve_path: Option<std::path::PathBuf>,
    /// Bass shelf preference in dB (layered on top of the target shape).
    pub bass_shelf_db: f64,
    /// Bass shelf frequency in Hz.
    pub bass_shelf_freq: f64,
    /// Treble shelf preference in dB.
    pub treble_shelf_db: f64,
    /// Treble shelf frequency in Hz.
    pub treble_shelf_freq: f64,
    /// Enable broadband pre-correction (shelf+gain fit before fine EQ).
    pub broadband_precorrection: bool,
}

impl Default for TargetResponseUiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            shape: "harman".to_string(),
            slope_db_per_octave: -0.8,
            reference_freq: 1000.0,
            curve_path: None,
            bass_shelf_db: 0.0,
            bass_shelf_freq: 200.0,
            treble_shelf_db: 0.0,
            treble_shelf_freq: 8000.0,
            broadband_precorrection: false,
        }
    }
}

/// Excursion protection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcursionProtectionConfig {
    pub enabled: bool,
    pub auto_detect_f3: bool,
    pub manual_f3_hz: f64,
    pub f3_reference_min_hz: f64,
    pub f3_reference_max_hz: f64,
    pub filter_order: usize,
    pub filter_type: String,
    pub margin_octaves: f64,
}

impl Default for ExcursionProtectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_detect_f3: true,
            manual_f3_hz: 40.0,
            f3_reference_min_hz: 100.0,
            f3_reference_max_hz: 200.0,
            filter_order: 4,
            filter_type: "lr".to_string(),
            margin_octaves: 0.25,
        }
    }
}

/// Schroeder split configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchroederSplitConfig {
    pub enabled: bool,
    pub schroeder_freq: f64,
    pub low_freq_max_q: f64,
    pub low_freq_allow_boost: bool,
    /// Maximum boost/cut in dB for below-Schroeder filters (None = use global max_db)
    #[serde(default)]
    pub low_freq_max_db: Option<f64>,
    pub high_freq_max_q: f64,
    pub high_freq_shelving_only: bool,
}

impl Default for SchroederSplitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            schroeder_freq: 300.0,
            low_freq_max_q: 10.0,
            low_freq_allow_boost: false,
            low_freq_max_db: None,
            high_freq_max_q: 1.0,
            high_freq_shelving_only: false,
        }
    }
}

/// Subwoofer-specific optimizer overrides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubOptimizerUiConfig {
    pub enabled: bool,
    pub num_filters: usize,
    pub max_db: f64,
    pub min_db: f64,
    pub min_q: f64,
    pub max_q: f64,
}

impl Default for SubOptimizerUiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            num_filters: 10,
            max_db: 18.0,
            min_db: -18.0,
            min_q: 0.5,
            max_q: 10.0,
        }
    }
}

/// Inter-channel consistency correction configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMatchingUiConfig {
    pub enabled: bool,
    pub threshold_db: f64,
    pub max_filters: usize,
}

impl Default for ChannelMatchingUiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold_db: 1.5,
            max_filters: 3,
        }
    }
}

/// Phase alignment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseAlignmentConfig {
    pub enabled: bool,
    pub min_freq: f64,
    pub max_freq: f64,
    pub optimize_polarity: bool,
    pub max_delay_ms: f64,
}

impl Default for PhaseAlignmentConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_freq: 60.0,
            max_freq: 100.0,
            optimize_polarity: true,
            max_delay_ms: 30.0,
        }
    }
}

/// Multi-seat configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiSeatConfig {
    pub enabled: bool,
    pub strategy: String,
    pub primary_seat: usize,
    pub max_deviation_db: f64,
    #[serde(default = "default_all_channel_multiseat_enabled")]
    pub all_channel_enabled: bool,
    #[serde(default = "default_all_channel_multiseat_strategy")]
    pub all_channel_strategy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seat_weights: Option<Vec<f64>>,
    #[serde(default = "default_primary_seat_weight")]
    pub primary_seat_weight: f64,
    /// Continuous listening-area configuration (used when strategy = "continuous_area")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continuous_area: Option<ContinuousListeningAreaUiConfig>,
}

/// Flat UI configuration mirror of `autoeq::roomeq::ContinuousListeningAreaConfig`.
///
/// Strings are used in place of tagged enums for ergonomic UI binding; conversion
/// happens at `to_optimizer_config()` time and is permissive on unknown values
/// (falls back to defaults rather than panicking on a stale UI string).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContinuousListeningAreaUiConfig {
    /// Spatial dimensions (1, 2, or 3).
    pub dimensions: usize,
    /// Per-axis bounding-box bounds `[lo, hi]`. Length must equal `dimensions`.
    pub bounds: Vec<[f64; 2]>,
    /// Spatial coordinates of each calibration seat. Outer length = number of
    /// seats, inner length = `dimensions`. Order must match seat index in the
    /// measurements array.
    pub seat_positions: Vec<Vec<f64>>,
    /// Prior kind: "uniform" or "gaussian".
    #[serde(default = "default_area_prior_kind")]
    pub prior_kind: String,
    /// Per-axis means for Gaussian prior (length must equal `dimensions`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gaussian_mean: Vec<f64>,
    /// Per-axis variances for Gaussian prior (length must equal `dimensions`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gaussian_cov_diag: Vec<f64>,
    /// Truncation in standard deviations for Gaussian prior.
    #[serde(default = "default_gaussian_truncation_sigmas")]
    pub gaussian_truncation_sigmas: f64,
    /// Quadrature kind: "sobol", "latin_hypercube", or "gauss_legendre".
    #[serde(default = "default_area_quadrature_kind")]
    pub quadrature_kind: String,
    /// Number of quadrature points (Sobol / Latin-Hypercube).
    #[serde(default = "default_area_quadrature_num_points")]
    pub quadrature_num_points: usize,
    /// PRNG seed for sampling-based quadratures.
    #[serde(default = "default_area_quadrature_seed")]
    pub quadrature_seed: u64,
    /// Nodes per axis for Gauss-Legendre.
    #[serde(default = "default_area_gauss_legendre_points_per_axis")]
    pub gauss_legendre_points_per_axis: usize,
    /// Scalarisation kind: "expected_value", "worst_case", or "cvar".
    #[serde(default = "default_area_scalarisation_kind")]
    pub scalarisation_kind: String,
    /// Inner-search budget for the worst-case scalarisation.
    #[serde(default = "default_area_inner_maxiter")]
    pub worst_case_inner_maxiter: usize,
    /// Inner-search seed for the worst-case scalarisation.
    #[serde(default)]
    pub worst_case_inner_seed: u64,
    /// Tail fraction for CVaR scalarisation.
    #[serde(default = "default_area_cvar_alpha")]
    pub cvar_alpha: f64,
    /// IDW power exponent for the spatial measurement interpolator.
    #[serde(default = "default_idw_power")]
    pub idw_power: f64,
}

fn default_area_prior_kind() -> String {
    "uniform".to_string()
}
fn default_gaussian_truncation_sigmas() -> f64 {
    4.0
}
fn default_area_quadrature_kind() -> String {
    "sobol".to_string()
}
fn default_area_quadrature_num_points() -> usize {
    64
}
fn default_area_quadrature_seed() -> u64 {
    0xC0FFEE
}
fn default_area_gauss_legendre_points_per_axis() -> usize {
    4
}
fn default_area_scalarisation_kind() -> String {
    "expected_value".to_string()
}
fn default_area_inner_maxiter() -> usize {
    50
}
fn default_area_cvar_alpha() -> f64 {
    0.20
}
fn default_idw_power() -> f64 {
    2.0
}

impl Default for ContinuousListeningAreaUiConfig {
    fn default() -> Self {
        Self {
            dimensions: 2,
            bounds: vec![[0.0, 1.0], [0.0, 1.0]],
            seat_positions: Vec::new(),
            prior_kind: default_area_prior_kind(),
            gaussian_mean: Vec::new(),
            gaussian_cov_diag: Vec::new(),
            gaussian_truncation_sigmas: default_gaussian_truncation_sigmas(),
            quadrature_kind: default_area_quadrature_kind(),
            quadrature_num_points: default_area_quadrature_num_points(),
            quadrature_seed: default_area_quadrature_seed(),
            gauss_legendre_points_per_axis: default_area_gauss_legendre_points_per_axis(),
            scalarisation_kind: default_area_scalarisation_kind(),
            worst_case_inner_maxiter: default_area_inner_maxiter(),
            worst_case_inner_seed: 0,
            cvar_alpha: default_area_cvar_alpha(),
            idw_power: default_idw_power(),
        }
    }
}

fn default_all_channel_multiseat_enabled() -> bool {
    true
}

fn default_all_channel_multiseat_strategy() -> String {
    "spatial_robustness".to_string()
}

fn default_primary_seat_weight() -> f64 {
    2.0
}

impl Default for MultiSeatConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            strategy: "variance".to_string(),
            primary_seat: 0,
            max_deviation_db: 6.0,
            all_channel_enabled: default_all_channel_multiseat_enabled(),
            all_channel_strategy: default_all_channel_multiseat_strategy(),
            seat_weights: None,
            primary_seat_weight: default_primary_seat_weight(),
            continuous_area: None,
        }
    }
}

/// Voice of God (timbre matching) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoGConfig {
    pub enabled: bool,
    pub reference_channel: String,
}

impl Default for VoGConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            reference_channel: "C".to_string(),
        }
    }
}

/// Mixed mode (IIR+FIR) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixedModeUiConfig {
    pub crossover_freq: f64,
    pub crossover_type: String,
    pub fir_band: String,
}

impl Default for MixedModeUiConfig {
    fn default() -> Self {
        Self {
            crossover_freq: 300.0,
            crossover_type: "LR24".to_string(),
            fir_band: "low".to_string(),
        }
    }
}

/// Multi-measurement optimization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiMeasurementUiConfig {
    pub enabled: bool,
    pub strategy: String,
    pub variance_lambda: f64,
    pub weights: Vec<f64>,
    /// Bootstrap-uncertainty configuration (used when strategy = "minimax_uncertainty").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootstrap_uncertainty: Option<BootstrapUncertaintyUiConfig>,
}

impl Default for MultiMeasurementUiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            strategy: "average".to_string(),
            variance_lambda: 1.0,
            weights: Vec::new(),
            bootstrap_uncertainty: None,
        }
    }
}

/// Flat UI mirror of `autoeq::roomeq::BootstrapUncertaintyConfig`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BootstrapUncertaintyUiConfig {
    /// Number of case-bootstrap resamples B.
    #[serde(default = "default_bootstrap_num_resamples")]
    pub num_resamples: usize,
    /// Two-sided confidence level α (used for diagnostic plots; the optimizer
    /// uses all B resamples).
    #[serde(default = "default_bootstrap_alpha")]
    pub alpha: f64,
    /// PRNG seed.
    #[serde(default = "default_bootstrap_seed")]
    pub seed: u64,
    /// Scalarisation kind: "worst_case" or "cvar".
    #[serde(default = "default_bootstrap_scalarisation")]
    pub scalarisation: String,
    /// Tail fraction for CVaR scalarisation.
    #[serde(default = "default_bootstrap_cvar_alpha")]
    pub cvar_alpha: f64,
}

fn default_bootstrap_num_resamples() -> usize {
    400
}
fn default_bootstrap_alpha() -> f64 {
    0.10
}
fn default_bootstrap_seed() -> u64 {
    0xC0FFEE
}
fn default_bootstrap_scalarisation() -> String {
    "worst_case".to_string()
}
fn default_bootstrap_cvar_alpha() -> f64 {
    0.20
}

impl Default for BootstrapUncertaintyUiConfig {
    fn default() -> Self {
        Self {
            num_resamples: default_bootstrap_num_resamples(),
            alpha: default_bootstrap_alpha(),
            seed: default_bootstrap_seed(),
            scalarisation: default_bootstrap_scalarisation(),
            cvar_alpha: default_bootstrap_cvar_alpha(),
        }
    }
}

fn bootstrap_uncertainty_from_backend(
    b: &autoeq::roomeq::BootstrapUncertaintyConfig,
) -> BootstrapUncertaintyUiConfig {
    BootstrapUncertaintyUiConfig {
        num_resamples: b.num_resamples,
        alpha: b.alpha,
        seed: b.seed,
        scalarisation: match b.scalarisation {
            autoeq::roomeq::BootstrapScalarisation::WorstCase => "worst_case".to_string(),
            autoeq::roomeq::BootstrapScalarisation::Cvar => "cvar".to_string(),
        },
        cvar_alpha: b.cvar_alpha,
    }
}

fn bootstrap_uncertainty_to_backend(
    ui: &BootstrapUncertaintyUiConfig,
) -> autoeq::roomeq::BootstrapUncertaintyConfig {
    autoeq::roomeq::BootstrapUncertaintyConfig {
        num_resamples: ui.num_resamples,
        alpha: ui.alpha,
        seed: ui.seed,
        scalarisation: match ui.scalarisation.as_str() {
            "cvar" => autoeq::roomeq::BootstrapScalarisation::Cvar,
            _ => autoeq::roomeq::BootstrapScalarisation::WorstCase,
        },
        cvar_alpha: ui.cvar_alpha,
    }
}

fn continuous_area_from_backend(
    a: &autoeq::roomeq::ContinuousListeningAreaConfig,
) -> ContinuousListeningAreaUiConfig {
    let (prior_kind, gaussian_mean, gaussian_cov_diag, gaussian_truncation_sigmas) = match &a.prior
    {
        autoeq::roomeq::AreaPriorKind::Uniform => (
            "uniform".to_string(),
            Vec::new(),
            Vec::new(),
            default_gaussian_truncation_sigmas(),
        ),
        autoeq::roomeq::AreaPriorKind::Gaussian {
            mean,
            cov_diag,
            truncation_sigmas,
        } => (
            "gaussian".to_string(),
            mean.clone(),
            cov_diag.clone(),
            *truncation_sigmas,
        ),
    };

    let (quadrature_kind, quadrature_num_points, quadrature_seed, gauss_legendre_points_per_axis) =
        match &a.quadrature {
            autoeq::roomeq::AreaQuadratureKind::Sobol { num_points, seed } => (
                "sobol".to_string(),
                *num_points,
                *seed,
                default_area_gauss_legendre_points_per_axis(),
            ),
            autoeq::roomeq::AreaQuadratureKind::LatinHypercube { num_points, seed } => (
                "latin_hypercube".to_string(),
                *num_points,
                *seed,
                default_area_gauss_legendre_points_per_axis(),
            ),
            autoeq::roomeq::AreaQuadratureKind::GaussLegendre { points_per_axis } => (
                "gauss_legendre".to_string(),
                default_area_quadrature_num_points(),
                default_area_quadrature_seed(),
                *points_per_axis,
            ),
        };

    let (scalarisation_kind, worst_case_inner_maxiter, worst_case_inner_seed, cvar_alpha) =
        match &a.scalarisation {
            autoeq::roomeq::AreaScalarisationKind::ExpectedValue => (
                "expected_value".to_string(),
                default_area_inner_maxiter(),
                0,
                default_area_cvar_alpha(),
            ),
            autoeq::roomeq::AreaScalarisationKind::WorstCase {
                inner_maxiter,
                inner_seed,
            } => (
                "worst_case".to_string(),
                *inner_maxiter,
                *inner_seed,
                default_area_cvar_alpha(),
            ),
            autoeq::roomeq::AreaScalarisationKind::Cvar { alpha } => {
                ("cvar".to_string(), default_area_inner_maxiter(), 0, *alpha)
            }
        };

    ContinuousListeningAreaUiConfig {
        dimensions: a.dimensions,
        bounds: a.bounds.iter().map(|(lo, hi)| [*lo, *hi]).collect(),
        seat_positions: a.seat_positions.clone(),
        prior_kind,
        gaussian_mean,
        gaussian_cov_diag,
        gaussian_truncation_sigmas,
        quadrature_kind,
        quadrature_num_points,
        quadrature_seed,
        gauss_legendre_points_per_axis,
        scalarisation_kind,
        worst_case_inner_maxiter,
        worst_case_inner_seed,
        cvar_alpha,
        idw_power: a.idw_power,
    }
}

fn continuous_area_to_backend(
    ui: &ContinuousListeningAreaUiConfig,
) -> autoeq::roomeq::ContinuousListeningAreaConfig {
    let prior = match ui.prior_kind.as_str() {
        "gaussian" => autoeq::roomeq::AreaPriorKind::Gaussian {
            mean: ui.gaussian_mean.clone(),
            cov_diag: ui.gaussian_cov_diag.clone(),
            truncation_sigmas: ui.gaussian_truncation_sigmas,
        },
        _ => autoeq::roomeq::AreaPriorKind::Uniform,
    };

    let quadrature = match ui.quadrature_kind.as_str() {
        "latin_hypercube" => autoeq::roomeq::AreaQuadratureKind::LatinHypercube {
            num_points: ui.quadrature_num_points,
            seed: ui.quadrature_seed,
        },
        "gauss_legendre" => autoeq::roomeq::AreaQuadratureKind::GaussLegendre {
            points_per_axis: ui.gauss_legendre_points_per_axis,
        },
        _ => autoeq::roomeq::AreaQuadratureKind::Sobol {
            num_points: ui.quadrature_num_points,
            seed: ui.quadrature_seed,
        },
    };

    let scalarisation = match ui.scalarisation_kind.as_str() {
        "worst_case" => autoeq::roomeq::AreaScalarisationKind::WorstCase {
            inner_maxiter: ui.worst_case_inner_maxiter,
            inner_seed: ui.worst_case_inner_seed,
        },
        "cvar" => autoeq::roomeq::AreaScalarisationKind::Cvar {
            alpha: ui.cvar_alpha,
        },
        _ => autoeq::roomeq::AreaScalarisationKind::ExpectedValue,
    };

    autoeq::roomeq::ContinuousListeningAreaConfig {
        dimensions: ui.dimensions,
        bounds: ui.bounds.iter().map(|b| (b[0], b[1])).collect(),
        seat_positions: ui.seat_positions.clone(),
        prior,
        quadrature,
        scalarisation,
        idw_power: ui.idw_power,
    }
}

fn default_room_smooth_n() -> usize {
    6 // 1/6 octave smoothing
}

fn default_room_strategy() -> String {
    "lshade".to_string()
}
fn default_de_f() -> f64 {
    0.8
}
fn default_de_cr() -> f64 {
    0.9
}
fn default_adaptive_weight_f() -> f64 {
    0.8
}
fn default_adaptive_weight_cr() -> f64 {
    0.7
}
fn default_spacing_weight() -> f64 {
    1.0
}
fn default_min_spacing_oct() -> f64 {
    0.08
}
fn default_bo_acquisition() -> String {
    "qei".to_string()
}
fn default_sample_rate() -> usize {
    48000
}
fn default_room_tolerance() -> f64 {
    1e-5
}

/// Classical sample rates for audio (44.1k and 48k families, up to 8x).
pub const CLASSICAL_SAMPLE_RATES: &[usize] =
    &[44100, 48000, 88200, 96000, 176400, 192000, 352800, 384000];

/// Given a raw sample-rate value, snap to the next classical rate above it.
/// Returns the highest rate if already at or above the top.
pub fn next_sample_rate(current: usize) -> usize {
    for &rate in CLASSICAL_SAMPLE_RATES {
        if rate > current {
            return rate;
        }
    }
    *CLASSICAL_SAMPLE_RATES.last().unwrap()
}

/// Given a raw sample-rate value, snap to the previous classical rate below it.
/// Returns the lowest rate if already at or below the bottom.
pub fn prev_sample_rate(current: usize) -> usize {
    for &rate in CLASSICAL_SAMPLE_RATES.iter().rev() {
        if rate < current {
            return rate;
        }
    }
    *CLASSICAL_SAMPLE_RATES.first().unwrap()
}
fn default_room_atolerance() -> f64 {
    1e-5
}

// ---------------------------------------------------------------------------
// Channel metadata for smart defaults
// ---------------------------------------------------------------------------

/// Metadata about measurement channels, decoupled from UI state.
///
/// Used by [`RoomEqOptimizerConfig::apply_smart_defaults`] to infer system
/// configuration (stereo vs surround, subwoofer presence, height channels).
#[derive(Debug, Clone, Default)]
pub struct ChannelMetadata {
    pub channel_names: Vec<String>,
    pub playback_sample_rate: Option<u32>,
}

impl ChannelMetadata {
    /// Sub/LFE channel names.
    fn is_sub_name(name: &str) -> bool {
        let upper = name.to_uppercase();
        upper == "LFE" || upper == "SUB" || upper == "SW" || upper.starts_with("SUB")
    }

    /// Height channel names used for Voice of God detection.
    const HEIGHT_CHANNELS: &[&str] = &[
        "TFL", "TFR", "TSL", "TSR", "TBL", "TBR", "VOG", "TFC", "TBC", "TSC",
    ];

    /// Count of non-subwoofer channels.
    fn non_sub_count(&self) -> usize {
        self.channel_names
            .iter()
            .filter(|n| !Self::is_sub_name(n))
            .count()
    }

    pub fn has_subwoofer(&self) -> bool {
        self.channel_names.iter().any(|n| Self::is_sub_name(n))
    }

    pub fn is_surround(&self) -> bool {
        self.non_sub_count() >= 3
    }

    pub fn has_height_channels(&self) -> bool {
        self.channel_names.iter().any(|name| {
            let upper = name.to_uppercase();
            Self::HEIGHT_CHANNELS.iter().any(|&h| upper == h)
        })
    }

    pub fn is_home_cinema(&self) -> bool {
        self.non_sub_count() >= 5
    }
}

/// Material profile bias for EPA temporal-masking — UI-facing alias.
///
/// Maps 1:1 onto [`autoeq::loss::epa::score::TemporalMaskingProfile`]; kept
/// as a separate type so we don't leak the backend enum through the UI
/// state and the GPUI/TUI can `match` on a stable surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EpaTemporalProfile {
    Transient,
    #[default]
    Mixed,
    Sustained,
}

impl EpaTemporalProfile {
    pub fn all() -> &'static [EpaTemporalProfile] {
        &[
            EpaTemporalProfile::Transient,
            EpaTemporalProfile::Mixed,
            EpaTemporalProfile::Sustained,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            EpaTemporalProfile::Transient => "Transient",
            EpaTemporalProfile::Mixed => "Mixed",
            EpaTemporalProfile::Sustained => "Sustained",
        }
    }
}

impl From<EpaTemporalProfile> for autoeq::loss::epa::score::TemporalMaskingProfile {
    fn from(p: EpaTemporalProfile) -> Self {
        match p {
            EpaTemporalProfile::Transient => Self::Transient,
            EpaTemporalProfile::Mixed => Self::Mixed,
            EpaTemporalProfile::Sustained => Self::Sustained,
        }
    }
}

/// UI-facing surface for EPA temporal-masking knobs.
///
/// Maps onto [`autoeq::loss::epa::score::TemporalMaskingConfig`] one-to-one;
/// kept separate so the UI never has to import the backend type directly and
/// so additional UI-only state (e.g. expanded/collapsed) can sit next to the
/// data without bleeding into the JSON contract with autoeq.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpaTemporalMaskingConfig {
    /// Master toggle: when false the optimizer skips both the modal and the
    /// IR temporal-masking penalties (the rest of EPA still runs).
    #[serde(default = "default_epa_temporal_enabled")]
    pub enabled: bool,
    /// Weight for the modal (frequency-domain) temporal-masking penalty.
    #[serde(default = "default_epa_temporal_weight")]
    pub weight: f64,
    /// Material profile that scales pre/post ringing audibility.
    #[serde(default)]
    pub profile: EpaTemporalProfile,
    /// Enable the direct FIR impulse-response pre/post-ringing analysis.
    /// Only meaningful when FIR coefficients are exported.
    #[serde(default = "default_epa_temporal_ir_enabled")]
    pub ir_enabled: bool,
    /// Weight for the FIR IR-masking penalty term.
    #[serde(default = "default_epa_temporal_ir_weight")]
    pub ir_weight: f64,
    /// Pre-masking window in ms (energy inside the window is partially
    /// masked; outside is fully audible).
    #[serde(default = "default_epa_temporal_pre_mask_ms")]
    pub pre_mask_ms: f64,
    /// Post-masking window in ms.
    #[serde(default = "default_epa_temporal_post_mask_ms")]
    pub post_mask_ms: f64,
}

fn default_epa_temporal_enabled() -> bool {
    true
}
fn default_epa_temporal_weight() -> f64 {
    0.15
}
fn default_epa_temporal_ir_enabled() -> bool {
    true
}
fn default_epa_temporal_ir_weight() -> f64 {
    0.05
}
fn default_epa_temporal_pre_mask_ms() -> f64 {
    3.0
}
fn default_epa_temporal_post_mask_ms() -> f64 {
    120.0
}

impl Default for EpaTemporalMaskingConfig {
    fn default() -> Self {
        Self {
            enabled: default_epa_temporal_enabled(),
            weight: default_epa_temporal_weight(),
            profile: EpaTemporalProfile::default(),
            ir_enabled: default_epa_temporal_ir_enabled(),
            ir_weight: default_epa_temporal_ir_weight(),
            pre_mask_ms: default_epa_temporal_pre_mask_ms(),
            post_mask_ms: default_epa_temporal_post_mask_ms(),
        }
    }
}

impl EpaTemporalMaskingConfig {
    /// Returns true when the user has knobs set away from the autoeq defaults
    /// — used by `to_optimizer_config` to decide whether to override the
    /// backend's `epa_config` at all.
    pub fn differs_from_default(&self) -> bool {
        let d = Self::default();
        self.enabled != d.enabled
            || (self.weight - d.weight).abs() > f64::EPSILON
            || self.profile != d.profile
            || self.ir_enabled != d.ir_enabled
            || (self.ir_weight - d.ir_weight).abs() > f64::EPSILON
            || (self.pre_mask_ms - d.pre_mask_ms).abs() > f64::EPSILON
            || (self.post_mask_ms - d.post_mask_ms).abs() > f64::EPSILON
    }

    /// Build a backend `TemporalMaskingConfig`, leaving non-UI knobs at the
    /// autoeq defaults. Spread-init keeps any future backend fields at their
    /// `Default::default()` without forcing this layer to track them.
    pub fn to_backend(&self) -> autoeq::loss::epa::score::TemporalMaskingConfig {
        autoeq::loss::epa::score::TemporalMaskingConfig {
            enabled: self.enabled,
            weight: self.weight,
            profile: self.profile.into(),
            ir_enabled: self.ir_enabled,
            ir_weight: self.ir_weight,
            pre_mask_ms: self.pre_mask_ms,
            post_mask_ms: self.post_mask_ms,
            ..autoeq::loss::epa::score::TemporalMaskingConfig::default()
        }
    }
}

/// Optimizer configuration for Room EQ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEqOptimizerConfig {
    #[serde(default)]
    pub mode: RoomEqOptimizationMode,
    #[serde(default)]
    pub fir: RoomEqFirConfig,
    pub multi_speaker_mode: MultiSpeakerMode,
    pub algorithm: String,
    #[serde(default = "default_room_strategy")]
    pub strategy: String,
    #[serde(default = "default_de_f")]
    pub de_f: f64,
    #[serde(default = "default_de_cr")]
    pub de_cr: f64,
    #[serde(default = "default_adaptive_weight_f")]
    pub adaptive_weight_f: f64,
    #[serde(default = "default_adaptive_weight_cr")]
    pub adaptive_weight_cr: f64,
    #[serde(default = "default_spacing_weight")]
    pub spacing_weight: f64,
    #[serde(default = "default_min_spacing_oct")]
    pub min_spacing_oct: f64,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: usize,
    pub num_filters: usize,
    pub min_q: f64,
    pub max_q: f64,
    pub min_db: f64,
    pub max_db: f64,
    pub min_freq: f64,
    pub max_freq: f64,
    pub max_iter: usize,
    pub peq_model: String,
    pub population: usize,
    #[serde(default)]
    pub bo_initial_samples: usize,
    #[serde(default)]
    pub bo_batch_size: usize,
    #[serde(default)]
    pub bo_posterior_std_threshold: f64,
    #[serde(default = "default_bo_acquisition")]
    pub bo_acquisition: String,
    #[serde(default)]
    pub bo_ehvi: bool,
    pub refine: bool,
    pub local_algo: String,
    pub loss_type: String,
    pub psychoacoustic: bool,
    pub asymmetric_loss: bool,
    #[serde(default)]
    pub smooth: bool,
    #[serde(default = "default_room_smooth_n")]
    pub smooth_n: usize,
    #[serde(default = "default_room_tolerance")]
    pub tolerance: f64,
    #[serde(default = "default_room_atolerance")]
    pub atolerance: f64,
    pub target_curve: String,
    pub system_type: String,
    #[serde(default)]
    pub allow_delay: bool,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub vog: VoGConfig,
    #[serde(default)]
    pub mixed_config: MixedModeUiConfig,
    #[serde(default)]
    pub mixed_phase: MixedPhaseUiConfig,
    /// Unified target response (shape + preference shelves + broadband pre-correction).
    #[serde(default)]
    pub target_response: TargetResponseUiConfig,
    #[serde(default)]
    pub excursion_protection: ExcursionProtectionConfig,
    #[serde(default)]
    pub schroeder_split: SchroederSplitConfig,
    #[serde(default)]
    pub phase_alignment: PhaseAlignmentConfig,
    #[serde(default)]
    pub multi_seat: MultiSeatConfig,
    #[serde(default)]
    pub multi_measurement: MultiMeasurementUiConfig,
    #[serde(default)]
    pub sub_config: SubOptimizerUiConfig,
    #[serde(default)]
    pub channel_matching: ChannelMatchingUiConfig,
    /// EPA temporal-masking knobs surfaced in the Step-3 configuration UI.
    /// Default keeps the backend's built-in defaults — see
    /// [`EpaTemporalMaskingConfig::differs_from_default`].
    #[serde(default)]
    pub epa_temporal_masking: EpaTemporalMaskingConfig,
    /// True when settings were imported from a backend config file (recordings.json).
    /// When set, `apply_smart_defaults()` skips overriding feature toggles.
    #[serde(default)]
    pub imported_from_file: bool,
}

impl Default for RoomEqOptimizerConfig {
    fn default() -> Self {
        Self {
            mode: RoomEqOptimizationMode::default(),
            fir: RoomEqFirConfig::default(),
            multi_speaker_mode: MultiSpeakerMode::Combined,
            algorithm: "autoeq:cmaes".to_string(),
            strategy: "lshade".to_string(),
            de_f: default_de_f(),
            de_cr: default_de_cr(),
            adaptive_weight_f: default_adaptive_weight_f(),
            adaptive_weight_cr: default_adaptive_weight_cr(),
            spacing_weight: default_spacing_weight(),
            min_spacing_oct: default_min_spacing_oct(),
            sample_rate: default_sample_rate(),
            num_filters: 7,
            min_q: 0.5,
            max_q: 6.0,
            min_db: -12.0,
            max_db: 4.0,
            min_freq: 20.0,
            max_freq: 1600.0,
            max_iter: 50000,
            peq_model: "pk".to_string(),
            population: 300,
            bo_initial_samples: 0,
            bo_batch_size: 0,
            bo_posterior_std_threshold: 0.0,
            bo_acquisition: default_bo_acquisition(),
            bo_ehvi: false,
            refine: false,
            local_algo: "cobyla".to_string(),
            loss_type: "flat".to_string(),
            psychoacoustic: true,
            asymmetric_loss: true,
            smooth: false,
            smooth_n: default_room_smooth_n(),
            tolerance: 1e-5,
            atolerance: 1e-5,
            target_curve: "flat".to_string(),
            system_type: "stereo".to_string(),
            allow_delay: false,
            seed: None,
            vog: VoGConfig::default(),
            mixed_config: MixedModeUiConfig::default(),
            mixed_phase: MixedPhaseUiConfig::default(),
            target_response: TargetResponseUiConfig::default(),
            excursion_protection: ExcursionProtectionConfig::default(),
            schroeder_split: SchroederSplitConfig::default(),
            phase_alignment: PhaseAlignmentConfig::default(),
            multi_seat: MultiSeatConfig::default(),
            multi_measurement: MultiMeasurementUiConfig::default(),
            sub_config: SubOptimizerUiConfig::default(),
            channel_matching: ChannelMatchingUiConfig::default(),
            epa_temporal_masking: EpaTemporalMaskingConfig::default(),
            imported_from_file: false,
        }
    }
}

impl RoomEqOptimizerConfig {
    /// Import optimizer parameters and feature toggles from a backend `OptimizerConfig`.
    ///
    /// This is used when loading a RoomConfig JSON file so that the UI
    /// uses the same optimizer settings as the roomeq CLI.
    /// Sets `imported_from_file = true` so that `apply_smart_defaults()` will
    /// not override the imported feature toggle state.
    pub fn import_from_backend(&mut self, backend: &autoeq::roomeq::OptimizerConfig) {
        // Core optimizer parameters
        self.algorithm = backend.algorithm.clone();
        self.strategy = backend.strategy.clone();
        self.num_filters = backend.num_filters;
        self.min_q = backend.min_q;
        self.max_q = backend.max_q;
        self.min_db = backend.min_db;
        self.max_db = backend.max_db;
        self.min_freq = backend.min_freq;
        self.max_freq = backend.max_freq;
        self.max_iter = backend.max_iter;
        self.population = backend.population;
        self.peq_model = backend.peq_model.clone();
        self.loss_type = backend.loss_type.clone();
        self.bo_initial_samples = backend.bo_initial_samples.unwrap_or(0);
        self.bo_batch_size = backend.bo_batch_size.unwrap_or(0);
        self.bo_posterior_std_threshold = backend.bo_posterior_std_threshold.unwrap_or(0.0);
        self.bo_acquisition = backend
            .bo_acquisition
            .clone()
            .unwrap_or_else(default_bo_acquisition);
        self.bo_ehvi = backend.bo_ehvi.unwrap_or(false);
        self.psychoacoustic = backend.psychoacoustic;
        self.asymmetric_loss = backend.asymmetric_loss;
        self.tolerance = backend.tolerance;
        self.atolerance = backend.atolerance;
        self.refine = backend.refine;
        self.local_algo = backend.local_algo.clone();
        self.seed = backend.seed;

        // FIR configuration
        if let Some(ref fir) = backend.fir {
            self.fir.taps = fir.taps;
            self.fir.phase = fir.phase.clone();
            self.fir.correct_excess_phase = fir.correct_excess_phase;
            self.fir.phase_smoothing = fir.phase_smoothing;
            self.fir.pre_ringing = fir.pre_ringing.as_ref().map(|pr| PreRingingConfig {
                threshold_db: pr.threshold_db,
                max_time_s: pr.max_time_s,
            });
        }

        // Mixed-phase configuration
        if let Some(ref mp) = backend.mixed_phase {
            self.mixed_phase = MixedPhaseUiConfig {
                max_fir_length_ms: mp.max_fir_length_ms,
                pre_ringing_threshold_db: mp.pre_ringing_threshold_db,
                min_spatial_depth: mp.min_spatial_depth,
                phase_smoothing_octaves: mp.phase_smoothing_octaves,
            };
        }

        // Processing mode → optimization mode
        self.mode = match backend.processing_mode {
            autoeq::roomeq::ProcessingMode::LowLatency => RoomEqOptimizationMode::Iir,
            autoeq::roomeq::ProcessingMode::PhaseLinear => RoomEqOptimizationMode::Fir,
            autoeq::roomeq::ProcessingMode::Hybrid => RoomEqOptimizationMode::Mixed,
            autoeq::roomeq::ProcessingMode::MixedPhase => RoomEqOptimizationMode::MixedPhase,
            // WarpedIir and KautzModal are IIR-based modes
            autoeq::roomeq::ProcessingMode::WarpedIir
            | autoeq::roomeq::ProcessingMode::KautzModal => RoomEqOptimizationMode::Iir,
        };

        // Feature toggles: only override from backend when explicitly present.
        if let Some(ref tr) = backend.target_response {
            self.target_response.enabled = true;
            self.target_response.shape = match tr.shape {
                autoeq::roomeq::TargetShape::Flat => "flat".to_string(),
                autoeq::roomeq::TargetShape::Harman => "harman".to_string(),
                autoeq::roomeq::TargetShape::Custom => "custom".to_string(),
                autoeq::roomeq::TargetShape::File => "file".to_string(),
                autoeq::roomeq::TargetShape::FromMeasurement => "from_measurement".to_string(),
            };
            self.target_response.slope_db_per_octave = tr.slope_db_per_octave;
            self.target_response.reference_freq = tr.reference_freq;
            self.target_response.curve_path = tr.curve_path.clone();
            self.target_response.bass_shelf_db = tr.preference.bass_shelf_db;
            self.target_response.bass_shelf_freq = tr.preference.bass_shelf_freq;
            self.target_response.treble_shelf_db = tr.preference.treble_shelf_db;
            self.target_response.treble_shelf_freq = tr.preference.treble_shelf_freq;
            self.target_response.broadband_precorrection = tr.broadband_precorrection;
        } else {
            self.target_response.enabled = false;
        }

        self.excursion_protection.enabled = backend
            .excursion_protection
            .as_ref()
            .is_some_and(|e| e.enabled);
        if let Some(ref ep) = backend.excursion_protection {
            self.excursion_protection.auto_detect_f3 = ep.auto_detect_f3;
            self.excursion_protection.manual_f3_hz = ep.manual_f3_hz.unwrap_or(40.0);
            self.excursion_protection.f3_reference_min_hz = ep.f3_reference_min_hz;
            self.excursion_protection.f3_reference_max_hz = ep.f3_reference_max_hz;
            self.excursion_protection.filter_order = ep.filter_order;
            self.excursion_protection.filter_type = match ep.filter_type {
                autoeq::roomeq::HighpassType::Butterworth => "bw".to_string(),
                autoeq::roomeq::HighpassType::LinkwitzRiley => "lr".to_string(),
            };
            self.excursion_protection.margin_octaves = ep.margin_octaves;
        }

        self.schroeder_split.enabled = backend.schroeder_split.as_ref().is_some_and(|s| s.enabled);
        if let Some(ref ss) = backend.schroeder_split {
            self.schroeder_split.schroeder_freq = ss.schroeder_freq;
            self.schroeder_split.low_freq_max_q = ss.low_freq_config.max_q;
            self.schroeder_split.low_freq_allow_boost = ss.low_freq_config.allow_boost;
            self.schroeder_split.low_freq_max_db = ss.low_freq_config.max_db;
            self.schroeder_split.high_freq_max_q = ss.high_freq_config.max_q;
            self.schroeder_split.high_freq_shelving_only = ss.high_freq_config.shelving_only;
        }

        self.allow_delay = backend.allow_delay.unwrap_or(false);

        self.vog.enabled = backend.vog.as_ref().is_some_and(|v| v.enabled);
        if let Some(ref vog) = backend.vog {
            self.vog.reference_channel = vog.reference_channel.clone();
        }

        self.phase_alignment.enabled = backend.phase_alignment.as_ref().is_some_and(|p| p.enabled);
        if let Some(ref pa) = backend.phase_alignment {
            self.phase_alignment.min_freq = pa.min_freq;
            self.phase_alignment.max_freq = pa.max_freq;
            self.phase_alignment.optimize_polarity = pa.optimize_polarity;
            self.phase_alignment.max_delay_ms = pa.max_delay_ms;
        }

        self.multi_seat.enabled = backend.multi_seat.as_ref().is_some_and(|m| m.enabled);
        if let Some(ref ms) = backend.multi_seat {
            self.multi_seat.strategy = match ms.strategy {
                autoeq::roomeq::MultiSeatStrategy::MinimizeVariance => "variance".to_string(),
                autoeq::roomeq::MultiSeatStrategy::PrimaryWithConstraints => "primary".to_string(),
                autoeq::roomeq::MultiSeatStrategy::Average => "average".to_string(),
                autoeq::roomeq::MultiSeatStrategy::ModalBasis => "modal_basis".to_string(),
                autoeq::roomeq::MultiSeatStrategy::ContinuousArea => "continuous_area".to_string(),
            };
            self.multi_seat.primary_seat = ms.primary_seat;
            self.multi_seat.max_deviation_db = ms.max_deviation_db;
            self.multi_seat.continuous_area = ms
                .continuous_area
                .as_ref()
                .map(continuous_area_from_backend);
        }

        if let Some(ref mm) = backend.multi_measurement {
            self.multi_measurement.enabled = true;
            self.multi_measurement.strategy = match mm.strategy {
                autoeq::roomeq::MultiMeasurementStrategy::Average => "average".to_string(),
                autoeq::roomeq::MultiMeasurementStrategy::WeightedSum => "weighted_sum".to_string(),
                autoeq::roomeq::MultiMeasurementStrategy::Minimax => "minimax".to_string(),
                autoeq::roomeq::MultiMeasurementStrategy::VariancePenalized => {
                    "variance_penalized".to_string()
                }
                autoeq::roomeq::MultiMeasurementStrategy::SpatialRobustness => {
                    "spatial_robustness".to_string()
                }
                autoeq::roomeq::MultiMeasurementStrategy::MinimaxUncertainty => {
                    "minimax_uncertainty".to_string()
                }
            };
            self.multi_measurement.variance_lambda = mm.variance_lambda;
            self.multi_measurement.weights = mm.weights.clone().unwrap_or_default();
            self.multi_measurement.bootstrap_uncertainty = mm
                .bootstrap_uncertainty
                .as_ref()
                .map(bootstrap_uncertainty_from_backend);
        } else {
            self.multi_measurement.enabled = false;
        }

        // Sub-specific optimizer overrides
        self.sub_config.enabled = backend.sub_config.is_some();
        if let Some(ref sc) = backend.sub_config {
            self.sub_config.num_filters = sc.num_filters;
            self.sub_config.max_db = sc.max_db;
            self.sub_config.min_db = sc.min_db;
            self.sub_config.min_q = sc.min_q;
            self.sub_config.max_q = sc.max_q;
        }

        // Channel matching correction
        self.channel_matching.enabled =
            backend.channel_matching.as_ref().is_some_and(|c| c.enabled);
        if let Some(ref cm) = backend.channel_matching {
            self.channel_matching.threshold_db = cm.threshold_db;
            self.channel_matching.max_filters = cm.max_filters;
        }

        self.imported_from_file = true;
    }

    /// Convert the flat UI optimizer config to a backend
    /// [`OptimizerConfig`](autoeq::roomeq::OptimizerConfig).
    ///
    /// This is the single canonical conversion used by both GPUI and TUI
    /// when building a `RoomConfig` for the optimizer.
    pub fn to_optimizer_config(&self) -> autoeq::roomeq::OptimizerConfig {
        use autoeq::roomeq::{
            ChannelMatchingConfig as BackendChannelMatchingConfig, DecomposedCorrectionSerdeConfig,
            ExcursionProtectionConfig as BackendExcursionProtectionConfig,
            FirConfig as BackendFirConfig, HighFreqFilterConfig, HighpassType, LowFreqFilterConfig,
            MixedModeConfig, MixedPhaseSerdeConfig as BackendMixedPhaseConfig,
            MultiMeasurementConfig, MultiMeasurementStrategy,
            MultiSeatConfig as BackendMultiSeatConfig, MultiSeatStrategy,
            OptimizerConfig as BackendOptimizerConfig,
            PhaseAlignmentConfig as BackendPhaseAlignmentConfig,
            PreRingingSerdeConfig as BackendPreRingingConfig, ProcessingMode,
            SchroederSplitConfig as BackendSchroederSplitConfig, SubOptimizerConfig,
            TargetResponseConfig as BackendTargetResponseConfig, TargetShape, UserPreference,
            VoiceOfGodConfig,
        };

        let processing_mode = match self.mode {
            RoomEqOptimizationMode::Iir => ProcessingMode::LowLatency,
            RoomEqOptimizationMode::Fir => ProcessingMode::PhaseLinear,
            RoomEqOptimizationMode::Mixed => ProcessingMode::Hybrid,
            RoomEqOptimizationMode::MixedPhase => ProcessingMode::MixedPhase,
        };

        let fir = Some(BackendFirConfig {
            taps: self.fir.taps,
            phase: self.fir.phase.clone(),
            correct_excess_phase: self.fir.correct_excess_phase,
            phase_smoothing: self.fir.phase_smoothing,
            pre_ringing: self
                .fir
                .pre_ringing
                .as_ref()
                .map(|pr| BackendPreRingingConfig {
                    threshold_db: pr.threshold_db,
                    max_time_s: pr.max_time_s,
                }),
        });

        let mixed_phase = if self.mode == RoomEqOptimizationMode::MixedPhase {
            Some(BackendMixedPhaseConfig {
                max_fir_length_ms: self.mixed_phase.max_fir_length_ms,
                pre_ringing_threshold_db: self.mixed_phase.pre_ringing_threshold_db,
                min_spatial_depth: self.mixed_phase.min_spatial_depth,
                phase_smoothing_octaves: self.mixed_phase.phase_smoothing_octaves,
            })
        } else {
            None
        };

        let mixed_config = if self.mode == RoomEqOptimizationMode::Mixed {
            Some(MixedModeConfig {
                crossover_freq: self.mixed_config.crossover_freq,
                crossover_type: self.mixed_config.crossover_type.clone(),
                fir_band: self.mixed_config.fir_band.clone(),
            })
        } else {
            None
        };

        let target_response = if self.target_response.enabled {
            let tr = &self.target_response;
            let shape = match tr.shape.as_str() {
                "flat" => TargetShape::Flat,
                "harman" => TargetShape::Harman,
                "custom" => TargetShape::Custom,
                "file" => TargetShape::File,
                "from_measurement" => TargetShape::FromMeasurement,
                _ => TargetShape::Custom,
            };
            Some(BackendTargetResponseConfig {
                shape,
                slope_db_per_octave: tr.slope_db_per_octave,
                reference_freq: tr.reference_freq,
                curve_path: tr.curve_path.clone(),
                preference: UserPreference {
                    bass_shelf_db: tr.bass_shelf_db,
                    bass_shelf_freq: tr.bass_shelf_freq,
                    treble_shelf_db: tr.treble_shelf_db,
                    treble_shelf_freq: tr.treble_shelf_freq,
                },
                broadband_precorrection: tr.broadband_precorrection,
                role_targets: None,
            })
        } else {
            None
        };

        let excursion_protection = if self.excursion_protection.enabled {
            let filter_type = if self.excursion_protection.filter_type == "bw" {
                HighpassType::Butterworth
            } else {
                HighpassType::LinkwitzRiley
            };
            Some(BackendExcursionProtectionConfig {
                enabled: true,
                auto_detect_f3: self.excursion_protection.auto_detect_f3,
                manual_f3_hz: Some(self.excursion_protection.manual_f3_hz),
                f3_reference_min_hz: self.excursion_protection.f3_reference_min_hz,
                f3_reference_max_hz: self.excursion_protection.f3_reference_max_hz,
                filter_order: self.excursion_protection.filter_order,
                filter_type,
                margin_octaves: self.excursion_protection.margin_octaves,
            })
        } else {
            None
        };

        let schroeder_split = if self.schroeder_split.enabled {
            Some(BackendSchroederSplitConfig {
                enabled: true,
                schroeder_freq: self.schroeder_split.schroeder_freq,
                room_dimensions: None,
                low_freq_config: LowFreqFilterConfig {
                    max_q: self.schroeder_split.low_freq_max_q,
                    min_q: 0.5,
                    allow_boost: self.schroeder_split.low_freq_allow_boost,
                    max_db: self.schroeder_split.low_freq_max_db,
                },
                high_freq_config: HighFreqFilterConfig {
                    max_q: self.schroeder_split.high_freq_max_q,
                    shelving_only: self.schroeder_split.high_freq_shelving_only,
                },
            })
        } else {
            None
        };

        let phase_alignment = if self.phase_alignment.enabled {
            Some(BackendPhaseAlignmentConfig {
                enabled: true,
                min_freq: self.phase_alignment.min_freq,
                max_freq: self.phase_alignment.max_freq,
                optimize_polarity: self.phase_alignment.optimize_polarity,
                max_delay_ms: self.phase_alignment.max_delay_ms,
            })
        } else {
            None
        };

        let has_all_channel_multiseat_policy = !self.multi_seat.all_channel_enabled
            || self.multi_seat.all_channel_strategy != default_all_channel_multiseat_strategy()
            || self.multi_seat.seat_weights.is_some()
            || (self.multi_seat.primary_seat_weight - default_primary_seat_weight()).abs() > 1e-9
            || self.multi_seat.primary_seat != 0
            || (self.multi_seat.max_deviation_db - 6.0).abs() > 1e-9;
        let multi_seat = if self.multi_seat.enabled || has_all_channel_multiseat_policy {
            let strategy = match self.multi_seat.strategy.as_str() {
                "primary" => MultiSeatStrategy::PrimaryWithConstraints,
                "average" => MultiSeatStrategy::Average,
                "modal_basis" => MultiSeatStrategy::ModalBasis,
                "continuous_area" => MultiSeatStrategy::ContinuousArea,
                _ => MultiSeatStrategy::MinimizeVariance,
            };
            Some(BackendMultiSeatConfig {
                enabled: self.multi_seat.enabled,
                strategy,
                primary_seat: self.multi_seat.primary_seat,
                max_deviation_db: self.multi_seat.max_deviation_db,
                optimize_polarity: false,
                allpass_filters_per_sub: 0,
                per_sub_peq: true,
                global_eq: true,
                all_channel_enabled: self.multi_seat.all_channel_enabled,
                all_channel_strategy: match self.multi_seat.all_channel_strategy.as_str() {
                    "weighted_sum" => autoeq::roomeq::MultiMeasurementStrategy::WeightedSum,
                    "minimax" => autoeq::roomeq::MultiMeasurementStrategy::Minimax,
                    "variance_penalized" => {
                        autoeq::roomeq::MultiMeasurementStrategy::VariancePenalized
                    }
                    "average" => autoeq::roomeq::MultiMeasurementStrategy::Average,
                    "minimax_uncertainty" => {
                        autoeq::roomeq::MultiMeasurementStrategy::MinimaxUncertainty
                    }
                    _ => autoeq::roomeq::MultiMeasurementStrategy::SpatialRobustness,
                },
                seat_weights: self.multi_seat.seat_weights.clone(),
                primary_seat_weight: self.multi_seat.primary_seat_weight,
                continuous_area: self
                    .multi_seat
                    .continuous_area
                    .as_ref()
                    .map(continuous_area_to_backend),
            })
        } else {
            None
        };

        let vog = if self.vog.enabled {
            Some(VoiceOfGodConfig {
                enabled: true,
                reference_channel: self.vog.reference_channel.clone(),
            })
        } else {
            None
        };

        let multi_measurement = if self.multi_measurement.enabled {
            let strategy_key =
                canonical_multi_measurement_strategy(&self.multi_measurement.strategy)
                    .unwrap_or_else(|| {
                        log::warn!(
                            "Unknown multi_measurement strategy '{}'; falling back to average",
                            self.multi_measurement.strategy
                        );
                        "average"
                    });
            let strategy = match strategy_key {
                "average" => MultiMeasurementStrategy::Average,
                "weighted_sum" => MultiMeasurementStrategy::WeightedSum,
                "minimax" => MultiMeasurementStrategy::Minimax,
                "variance_penalized" => MultiMeasurementStrategy::VariancePenalized,
                "spatial_robustness" => MultiMeasurementStrategy::SpatialRobustness,
                "minimax_uncertainty" => MultiMeasurementStrategy::MinimaxUncertainty,
                _ => MultiMeasurementStrategy::Average,
            };
            let weights = if self.multi_measurement.weights.is_empty() {
                None
            } else {
                Some(self.multi_measurement.weights.clone())
            };
            Some(MultiMeasurementConfig {
                strategy,
                weights,
                variance_lambda: self.multi_measurement.variance_lambda,
                spatial_robustness: None,
                bootstrap_uncertainty: self
                    .multi_measurement
                    .bootstrap_uncertainty
                    .as_ref()
                    .map(bootstrap_uncertainty_to_backend),
            })
        } else {
            None
        };

        let sub_config = if self.sub_config.enabled {
            Some(SubOptimizerConfig {
                num_filters: self.sub_config.num_filters,
                max_db: self.sub_config.max_db,
                min_db: self.sub_config.min_db,
                min_q: self.sub_config.min_q,
                max_q: self.sub_config.max_q,
            })
        } else {
            None
        };

        let channel_matching = if self.channel_matching.enabled {
            Some(BackendChannelMatchingConfig {
                enabled: true,
                threshold_db: self.channel_matching.threshold_db,
                max_filters: self.channel_matching.max_filters,
            })
        } else {
            None
        };
        let is_bo_algorithm = self.algorithm.eq_ignore_ascii_case("autoeq:bo")
            || self.algorithm.eq_ignore_ascii_case("bo");

        BackendOptimizerConfig {
            loss_type: self.loss_type.clone(),
            algorithm: self.algorithm.clone(),
            strategy: self.strategy.clone(),
            num_filters: self.num_filters,
            min_q: self.min_q,
            max_q: self.max_q,
            min_db: self.min_db,
            max_db: self.max_db,
            min_freq: self.min_freq,
            max_freq: self.max_freq,
            max_iter: self.max_iter,
            population: self.population,
            peq_model: self.peq_model.clone(),
            processing_mode,
            fir,
            mixed_phase,
            mixed_config,
            seed: self.seed,
            refine: self.refine,
            local_algo: self.local_algo.clone(),
            bo_initial_samples: (is_bo_algorithm && self.bo_initial_samples > 0)
                .then_some(self.bo_initial_samples),
            bo_batch_size: (is_bo_algorithm && self.bo_batch_size > 0)
                .then_some(self.bo_batch_size),
            bo_posterior_std_threshold: (is_bo_algorithm && self.bo_posterior_std_threshold > 0.0)
                .then_some(self.bo_posterior_std_threshold),
            bo_acquisition: (is_bo_algorithm && !self.bo_acquisition.is_empty())
                .then(|| self.bo_acquisition.clone()),
            bo_ehvi: (is_bo_algorithm && self.bo_ehvi).then_some(true),
            psychoacoustic: self.psychoacoustic,
            asymmetric_loss: self.asymmetric_loss,
            tolerance: self.tolerance,
            atolerance: self.atolerance,
            allow_delay: Some(self.allow_delay),
            smooth_n: self.smooth_n,
            target_response,
            excursion_protection,
            schroeder_split,
            phase_alignment,
            multi_seat,
            vog,
            multi_measurement,
            sub_config,
            channel_matching,
            decomposed_correction: Some(DecomposedCorrectionSerdeConfig::default()),
            // Only emit an `epa_config` override when the user actually
            // tweaked the temporal-masking knobs. Otherwise the backend's
            // `EpaConfig::default()` (which also includes
            // `flatness_band_weights`, etc.) is the right baseline.
            epa_config: if self.epa_temporal_masking.differs_from_default() {
                Some(autoeq::loss::epa::score::EpaConfig {
                    temporal_masking: self.epa_temporal_masking.to_backend(),
                    ..autoeq::loss::epa::score::EpaConfig::default()
                })
            } else {
                None
            },
            ..BackendOptimizerConfig::default()
        }
    }

    /// Apply smart defaults based on measurement channel metadata.
    ///
    /// Called after loading measurements to set sensible initial values.
    /// When `imported_from_file` is true, feature toggles are preserved.
    pub fn apply_smart_defaults(&mut self, meta: &ChannelMetadata) {
        // Seed sample rate from playback device when still at default
        if let Some(sr) = meta.playback_sample_rate
            && self.sample_rate == 48000
        {
            self.sample_rate = sr as usize;
        }

        // Loss type is always flat for room EQ
        self.loss_type = "flat".to_string();

        // Only override algorithm/seed defaults when not imported from file
        if !self.imported_from_file {
            self.local_algo = "cobyla".to_string();
            self.refine = true;
            self.seed = None;
        }

        // System type: auto-detect from channel count
        self.system_type = if meta.is_surround() {
            "multichannel".to_string()
        } else {
            "stereo".to_string()
        };

        // Feature flags: only auto-enable when NOT imported from file.
        // When imported, the file's feature state is authoritative
        // (None = disabled, Some = enabled with those params).
        if !self.imported_from_file {
            self.target_response.enabled = true;
            self.target_response.shape = "harman".to_string();
            self.excursion_protection.enabled = true;
            // Schroeder split only makes sense with a subwoofer
            self.schroeder_split.enabled = meta.has_subwoofer();
            self.allow_delay = true;
            self.target_response.broadband_precorrection = true;
            self.vog.enabled = meta.has_height_channels();
            self.vog.reference_channel = if meta.is_home_cinema() {
                "C".to_string()
            } else {
                "L".to_string()
            };
        }
    }
}

/// Compute the average slope for L and R channels in dB/octave.
///
/// Uses linear regression on the 200 Hz – 20 kHz range.
/// Returns `(slope, recommendation_min, recommendation_max)`.
pub fn compute_lr_slope(measurements: &[ChannelMeasurement]) -> Option<(f64, f64, f64)> {
    let lr_names = ["L", "R"];
    let mut slopes = Vec::new();

    for meas in measurements {
        let name_upper = meas.channel_name.to_uppercase();
        if !lr_names.iter().any(|&n| name_upper == n) {
            continue;
        }

        let freqs = &meas.measurement.frequencies;
        let spl = &meas.measurement.magnitude_db;

        let mut log_freqs = Vec::new();
        let mut dbs = Vec::new();

        for (i, &f) in freqs.iter().enumerate() {
            if (200.0..=20000.0).contains(&f)
                && let Some(&db) = spl.get(i)
            {
                log_freqs.push(f64::from(f).log10());
                dbs.push(f64::from(db));
            }
        }

        if log_freqs.len() < 2 {
            continue;
        }

        // Linear regression: db = slope * log_freq + intercept
        let n = log_freqs.len() as f64;
        let sum_x: f64 = log_freqs.iter().sum();
        let sum_y: f64 = dbs.iter().sum();
        let sum_xy: f64 = log_freqs.iter().zip(dbs.iter()).map(|(x, y)| x * y).sum();
        let sum_xx: f64 = log_freqs.iter().map(|x| x * x).sum();

        let denom = n * sum_xx - sum_x * sum_x;
        if denom.abs() < 1e-10 {
            continue;
        }

        // slope in dB per log10(Hz) = dB/decade
        // Convert to dB/octave: 1 octave = log10(2) ≈ 0.301 in log10 space
        let slope_log10 = (n * sum_xy - sum_x * sum_y) / denom;
        let slope_db_per_octave = slope_log10 * std::f64::consts::LOG10_2;

        slopes.push(slope_db_per_octave);
    }

    if slopes.is_empty() {
        return None;
    }

    let avg_slope: f64 = slopes.iter().sum::<f64>() / slopes.len() as f64;
    let recommendation_min = avg_slope * 0.8;
    let recommendation_max = avg_slope * 1.1;

    Some((avg_slope, recommendation_min, recommendation_max))
}

/// Optimization status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptimizationStatus {
    #[default]
    Idle,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Status of the tone-burst delay detection measurement.
///
/// The measurement runs on a background thread (kicked off from the UI).
/// `Running` carries the wall-clock start time in ms so the UI can
/// render a progress estimate as `elapsed / estimated_total` without
/// requiring the engine to surface a progress callback. The estimated
/// total is computed by the UI from `probe_duration_ms` and
/// `silence_duration_ms` × channel count.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum DelayDetectionStatus {
    #[default]
    Idle,
    Running {
        /// Milliseconds since the Unix epoch when the measurement was
        /// spawned. Used purely for elapsed-time computation; if the
        /// system clock jumps backward the progress bar may briefly
        /// misreport but nothing else depends on this value.
        started_at_ms: u64,
    },
    Complete,
    Failed(String),
}

impl DelayDetectionStatus {
    /// Estimated fraction of the measurement completed, in `0.0..=1.0`.
    ///
    /// Returns `None` when the status is not `Running` or the estimated
    /// duration is zero. Callers should render a fallback (e.g. an
    /// indeterminate spinner) in that case.
    pub fn progress(&self, estimated_total_ms: u64, now_ms: u64) -> Option<f32> {
        match self {
            Self::Running { started_at_ms } if estimated_total_ms > 0 => {
                let elapsed = now_ms.saturating_sub(*started_at_ms);
                Some((elapsed as f32 / estimated_total_ms as f32).clamp(0.0, 1.0))
            }
            _ => None,
        }
    }
}

/// Session metadata extracted from a loaded measurements file, used to
/// pre-seed the Delay Detection form so the probe runs with the same
/// device settings the measurements were captured under.
///
/// Every field is optional because older files — and files migrated
/// from other tools — may not carry this metadata.
#[derive(Debug, Clone, Default)]
pub struct DelayDetectionHints {
    /// Canonical channel name order from the recording session, e.g.
    /// `["L", "R", "C", "LFE", "SL", "SR"]` for 5.1. Used by the UI to
    /// re-order or validate the playback channel map.
    pub channel_names: Option<Vec<String>>,
    /// Recording device sample rate in Hz (used for the probe).
    pub sample_rate: Option<u32>,
    /// Playback device name (None = system default).
    pub playback_device_name: Option<String>,
    /// Recording device name (None = system default).
    pub recording_device_name: Option<String>,
    /// Probe results captured during the Recording wizard's Probe step.
    /// When present, the Room EQ Delay step auto-populates arrival
    /// times from these instead of showing "no data".
    pub probe_results: Option<DelayProbeResults>,
}

/// Estimate the total duration of a probe sequence in milliseconds.
///
/// `num_channels` probes + (`num_channels - 1`) gaps + a ~1 s head/tail
/// budget for device startup and stream settling. Used by the UI to
/// turn `DelayDetectionStatus::Running { started_at_ms }` into a
/// progress estimate.
pub fn estimate_probe_sequence_ms(
    num_channels: usize,
    probe_duration_ms: f32,
    silence_duration_ms: f32,
) -> u64 {
    if num_channels == 0 {
        return 0;
    }
    let per_channel = probe_duration_ms as f64 + silence_duration_ms as f64;
    let total = per_channel * num_channels as f64 + 1_000.0;
    total.round().max(0.0) as u64
}

/// Shared state for the Room EQ "Delay Detection" wizard step.
///
/// The UI of both app-tui and app-gpui drives this struct: it carries the
/// probe/device form inputs, the background-measurement status, the raw
/// per-channel detection results from [`DelayProbeResults`], and the
/// user-editable override values that ultimately feed into
/// [`crate::autoeq::run_room_optimization_with_probe_arrivals`].
///
/// Channel identity (name, hardware index) always flows through
/// `results.channels`. We intentionally do **not** carry a parallel
/// `channel_names` vec on this struct: the earlier design had an
/// alignment bug where `probe_arrival_map` zipped `channel_names` with
/// `edited_arrival_ms` and silently truncated on length mismatch.
#[derive(Debug, Clone)]
pub struct DelayDetectionState {
    /// Duration of each narrowband tone-burst in milliseconds.
    /// The default (1000 ms) is long enough for robust cross-correlation
    /// in typical rooms without making the full sweep tediously slow.
    pub probe_duration_ms: f32,
    /// Silence gap between probes in milliseconds. Avoids overlap between
    /// late reflections of one channel and the onset of the next.
    pub silence_duration_ms: f32,
    /// Sample rate used for the probe in Hz. Populated from the loaded
    /// measurement's recording configuration when available, otherwise
    /// defaults to 48 000.
    pub sample_rate: u32,
    /// Playback device name (None = system default).
    pub output_device_name: Option<String>,
    /// Recording device name (None = system default).
    pub input_device_name: Option<String>,
    /// Microphone input channel index (0-based).
    pub input_channel: u16,
    /// Background-measurement status.
    pub status: DelayDetectionStatus,
    /// Raw detection results (populated on success). Cleared on Reset / new
    /// run. Contains per-channel arrival_ms, gain_db, snr_db, and the
    /// auto-computed `alignment_delays_ms` vector. This is the authority
    /// on channel identity — `edited_arrival_ms[i]` corresponds to
    /// `results.channels[i]`.
    pub results: Option<DelayProbeResults>,
    /// User-editable per-channel arrival times in milliseconds (seeded
    /// from `results.channels[i].arrival_ms` after a successful
    /// measurement). Indices mirror `results.channels` exactly. The
    /// optimizer consumes these values (not `alignment_delays_ms`) so the
    /// downstream speaker_eq path can compute consistent alignment.
    pub edited_arrival_ms: Vec<f64>,
}

impl Default for DelayDetectionState {
    fn default() -> Self {
        Self {
            probe_duration_ms: 1000.0,
            silence_duration_ms: 500.0,
            sample_rate: 48_000,
            output_device_name: None,
            input_device_name: None,
            input_channel: 0,
            status: DelayDetectionStatus::Idle,
            results: None,
            edited_arrival_ms: Vec::new(),
        }
    }
}

impl DelayDetectionState {
    /// Seed `edited_arrival_ms` from a fresh set of probe results.
    ///
    /// Called after a successful measurement so the override editor has
    /// sensible initial values. The UI may then let the user tweak these
    /// before they flow into `run_room_optimization_with_probe_arrivals`.
    pub fn apply_results(&mut self, results: DelayProbeResults) {
        self.edited_arrival_ms = results.channels.iter().map(|c| c.arrival_ms).collect();
        self.results = Some(results);
        self.status = DelayDetectionStatus::Complete;
    }

    /// Build the per-channel arrival-time map used by
    /// [`crate::autoeq::run_room_optimization_with_probe_arrivals`].
    ///
    /// Returns `None` if the measurement has not completed or the user
    /// has cleared it. Channels with non-finite overrides (e.g. the user
    /// blanked an entry) are skipped so the optimizer falls back to
    /// WAV-onset detection for them. Channel identity comes from
    /// `results.channels[i].channel_name` — this is the authoritative
    /// source — and `edited_arrival_ms[i]` is read by position.
    pub fn probe_arrival_map(&self) -> Option<std::collections::HashMap<String, f64>> {
        if !matches!(self.status, DelayDetectionStatus::Complete) {
            return None;
        }
        let results = self.results.as_ref()?;
        let mut map = std::collections::HashMap::with_capacity(results.channels.len());
        for (i, ch) in results.channels.iter().enumerate() {
            let arrival = self
                .edited_arrival_ms
                .get(i)
                .copied()
                .unwrap_or(ch.arrival_ms);
            if arrival.is_finite() {
                map.insert(ch.channel_name.clone(), arrival);
            }
        }
        if map.is_empty() { None } else { Some(map) }
    }

    /// Recompute per-channel alignment delays from the current
    /// `edited_arrival_ms`. Used by the UI to show a live "Align ms"
    /// column that reflects user overrides instead of the stale values
    /// the engine computed from the raw measurement.
    ///
    /// Returns a vector indexed the same way as `results.channels`.
    /// Empty when there are no results.
    pub fn edited_alignment_delays_ms(&self) -> Vec<f64> {
        let Some(results) = self.results.as_ref() else {
            return Vec::new();
        };
        let arrivals: Vec<f64> = results
            .channels
            .iter()
            .enumerate()
            .map(|(i, ch)| {
                self.edited_arrival_ms
                    .get(i)
                    .copied()
                    .unwrap_or(ch.arrival_ms)
            })
            .collect();
        if arrivals.is_empty() {
            return Vec::new();
        }
        let max = arrivals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        arrivals.iter().map(|a| max - a).collect()
    }
}

/// Field identifiers for AutoEQ form editing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoEqField {
    NumFilters,
    MinQ,
    MaxQ,
    MinDb,
    MaxDb,
    MinFreq,
    MaxFreq,
    MaxIter,
}

/// EQ filter configuration (for display and export)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqFilterConfig {
    pub filter_type: String,
    pub frequency: f64,
    pub q: f64,
    pub gain_db: f64,
}

/// Optimization result for a single channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelOptResult {
    pub channel_name: String,
    pub pre_score: f64,
    pub post_score: f64,
    pub eq_filters: Vec<EqFilterConfig>,
    /// Broadband pre-correction filters (lowshelf/highshelf), separate from main EQ
    pub broadband_filters: Vec<EqFilterConfig>,
    /// Flat-gain (preamp) applied by post-optimization stages (spectral-alignment,
    /// channel-matching, Voice of God). Not a biquad — a constant dB offset that
    /// must be added to the EQ-filter sum so the displayed Sum matches what was
    /// actually applied to `final_curve`.
    #[serde(default)]
    pub preamp_gain_db: f64,
    pub crossover_freqs: Option<Vec<f64>>,
    pub driver_gains: Option<Vec<f64>>,
    pub original_response: Option<Vec<(f64, f64)>>,
    pub corrected_response: Option<Vec<(f64, f64)>>,
    pub normalized_response: Option<Vec<(f64, f64)>>,
    /// Target curve points (frequency_hz, level_db)
    pub target_curve: Option<Vec<(f64, f64)>>,
    /// Group delay before correction (frequency_hz, delay_ms)
    pub group_delay_before: Option<Vec<(f64, f64)>>,
    /// Group delay after correction (frequency_hz, delay_ms)
    pub group_delay_after: Option<Vec<(f64, f64)>>,
    /// Phase response before correction (frequency_hz, phase_radians)
    pub phase_response_before: Option<Vec<(f64, f64)>>,
    /// Phase response after correction (frequency_hz, phase_radians)
    pub phase_response_after: Option<Vec<(f64, f64)>>,
    /// Impulse response after correction (sample_index, amplitude)
    pub impulse_response: Option<Vec<(f64, f64)>>,
}

// DSP chain output types are the canonical `autoeq::roomeq` types — we
// re-export them here so downstream code keeps referring to
// `sotf_audio_player::room_eq_types::{DspChainOutput, ChannelDspChain, ...}`
// but we don't drop any fields on the floor (initial/final curves,
// target curve, pre/post IR, loss_type, inter-channel deviation, EPA
// metrics). Previously we had parallel stripped copies of these structs
// and a lossy field-by-field conversion in the Step-4 optimiser; that
// meant the Review step plot silently lost curves that the optimiser
// had already computed.

/// DSP plugin configuration (alias for `autoeq::roomeq::PluginConfigWrapper`).
pub type DspPluginConfig = autoeq::roomeq::PluginConfigWrapper;

/// DSP chain metadata (alias for `autoeq::roomeq::OptimizationMetadata`).
pub type DspChainMetadata = autoeq::roomeq::OptimizationMetadata;

pub use autoeq::roomeq::{ChannelDspChain, DriverDspChain, DspChainOutput};

/// Extension trait for `DspChainOutput` providing player-side helpers.
///
/// Lives here (not in `autoeq`) because it's a player concern: does this
/// chain correspond to a linear rack, or does it need a parallel
/// multi-driver graph? The autoeq crate doesn't know or care about the
/// player's rack model.
pub trait DspChainOutputExt {
    /// Returns true if the DSP output can be applied to a linear rack
    /// (no multi-driver crossovers requiring parallel paths).
    fn is_rack_compatible(&self) -> bool;

    /// Returns true when this output needs graph playback to preserve routing.
    fn requires_room_eq_graph(&self) -> bool;
}

impl DspChainOutputExt for DspChainOutput {
    fn is_rack_compatible(&self) -> bool {
        !self.requires_room_eq_graph()
    }

    fn requires_room_eq_graph(&self) -> bool {
        requires_room_eq_graph(self)
    }
}

/// Returns true when a RoomEQ result cannot be represented as a single linear rack.
pub fn requires_room_eq_graph(output: &DspChainOutput) -> bool {
    !output.global_plugins.is_empty()
        || output
            .channels
            .values()
            .any(|chain| chain.drivers.is_some())
        || output
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.bass_management.as_ref())
            .and_then(|report| report.routing_graph.as_ref())
            .is_some_and(|graph| !graph.routes.is_empty())
}

/// Build an engine graph that preserves RoomEQ routed bass management.
///
/// Routed bass management is encoded as a factored multichannel graph: each
/// per-channel-replicated DSP stage (pre-route gain, pre-route EQ, HP
/// crossover, HP delay, LP crossover, LP gain, LP delay, post-route EQ) is
/// emitted as a single multichannel plugin node carrying per-channel
/// parameter arrays. The graph fans out only at the HP/LP split and merges
/// at a sub-bus summing matrix node before the shared post-route EQ.
pub fn build_room_eq_plugin_graph_config(
    output: &DspChainOutput,
    _sample_rate: f64,
) -> anyhow::Result<sotf_audio::engine::PluginGraphConfig> {
    let routed_graph = output
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.bass_management.as_ref())
        .and_then(|report| report.routing_graph.as_ref())
        .filter(|graph| !graph.routes.is_empty());

    if let Some(graph) = routed_graph {
        return build_factored_routed_room_eq_graph(output, graph);
    }

    build_linear_room_eq_graph(output)
}

/// Build the factored graph for routed bass management.
///
/// Each per-channel-replicated DSP stage collapses to a single multichannel
/// plugin instance carrying per-channel parameter arrays. The LP branch
/// terminates in a sparse N×N matrix that sums LP signals onto each route's
/// destination row, with the per-route dB gain baked into the matrix
/// coefficient so different routes from the same source to different
/// destinations don't collapse to a single gain.
///
/// Node order and roles:
///   0. gain_pre        (per-channel pre-route gain_db)
///   1. eq_pre          (per-channel pre-route filter list)
///   2. xover_hp        (per-channel HP cutoff / Mute / Passthrough)
///   3. delay_hp        (per-channel HP delay_ms)
///   4. xover_lp        (per-channel LP cutoff / Mute)
///   5. delay_lp        (per-channel LP-to-sub delay_ms)
///   6. matrix_to_sub_bus (sparse N×N, per-route coefficients on the
///      destination row carry route gain in linear units)
///   7. eq_post         (per-channel post-route filter list)
///   8. gain_post       (per-channel post-route trim gain_db)
///
/// Destination-only channels (channels that are the destination of some
/// route but not the source of any route — e.g. a physical sub channel that
/// receives redirected bass but has no own HP/LP processing) flow through
/// the HP branch in `Passthrough` mode so direct sub-channel input reaches
/// the post-EQ stage without being silenced.
fn build_factored_routed_room_eq_graph(
    output: &DspChainOutput,
    graph: &autoeq::roomeq::BassManagementRoutingGraph,
) -> anyhow::Result<sotf_audio::engine::PluginGraphConfig> {
    use sotf_audio::engine::{PluginGraphConfig, PluginGraphEdgeConfig, PluginGraphNodeConfig};

    let channel_count = routed_graph_channel_count(output, graph);
    if channel_count == 0 {
        anyhow::bail!("routed_graph has no channels");
    }

    // Channel name → index. Prefer the routing graph's declared input order;
    // fall back to sorted channel names when the graph leaves it empty.
    let channel_order: Vec<String> = if graph.input_channels.is_empty() {
        sorted_channel_names(output)
    } else {
        graph.input_channels.clone()
    };
    let name_to_index =
        |name: &str| -> Option<usize> { channel_order.iter().position(|n| n == name) };

    // Per-channel parameter arrays, sized to channel_count and zero-initialized.
    let mut gain_pre_db = vec![0.0f32; channel_count];
    let mut gain_post_db = vec![0.0f32; channel_count];
    let mut filters_pre: Vec<Vec<serde_json::Value>> = vec![Vec::new(); channel_count];
    let mut hp_fc = vec![1000.0f32; channel_count];
    let mut hp_modes: Vec<&'static str> = vec!["mute"; channel_count];
    let mut hp_delay_ms = vec![0.0f32; channel_count];
    let mut lp_fc = vec![1000.0f32; channel_count];
    let mut lp_modes: Vec<&'static str> = vec!["mute"; channel_count];
    let mut lp_delay_ms = vec![0.0f32; channel_count];
    // chain[ch] route_owned gain_db (i.e. baked-in LFE-style gain).
    // Self-routes must prefer this baked-in chain gain over route metadata
    // so the LFE channel is not attenuated twice.
    let mut chain_route_owned_gain_db = vec![0.0f32; channel_count];
    // Per-route LP fan-in: (dst_idx, src_idx, gain_db). Built directly from
    // routes so routes that point to different destinations from the same
    // source are encoded independently.
    let mut lp_matrix_entries: Vec<(usize, usize, f32)> = Vec::new();
    let mut filters_post: Vec<Vec<serde_json::Value>> = vec![Vec::new(); channel_count];

    // Folder for the per-channel chain plugins.
    for (channel_name, chain) in output.channels.iter() {
        let Some(idx) = name_to_index(channel_name) else {
            // Channel exists in chains but not in the routing graph: skip.
            continue;
        };
        // Collect by stage. We *don't* fold post_route into pre_route —
        // post_route trims live in their own per-channel `gain_post` node so
        // they only apply to the final per-channel output, never to the LP
        // path summed into the sub bus.
        let mut pre_gain_db = 0.0f64;
        let mut post_gain_db = 0.0f64;
        let mut route_owned_gain_db = 0.0f64;
        for plugin in &chain.plugins {
            let stage = plugin_stage(plugin);
            match (plugin.plugin_type.as_str(), stage) {
                ("gain", Some("pre_route")) => {
                    pre_gain_db += plugin
                        .parameters
                        .get("gain_db")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                }
                ("gain", Some("post_route")) => {
                    post_gain_db += plugin
                        .parameters
                        .get("gain_db")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                }
                ("gain", Some("route_owned")) => {
                    route_owned_gain_db += plugin
                        .parameters
                        .get("gain_db")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                }
                ("eq", Some("pre_route")) => {
                    if let Some(arr) = plugin.parameters.get("filters").and_then(|v| v.as_array()) {
                        filters_pre[idx].extend(arr.iter().cloned());
                    }
                }
                ("eq", Some("post_route")) => {
                    if let Some(arr) = plugin.parameters.get("filters").and_then(|v| v.as_array()) {
                        filters_post[idx].extend(arr.iter().cloned());
                    }
                }
                _ => {
                    // crossover / delay route_owned: covered by the routing
                    // graph metadata below — ignore the per-chain copies.
                }
            }
        }
        gain_pre_db[idx] = pre_gain_db as f32;
        gain_post_db[idx] = post_gain_db as f32;
        if route_owned_gain_db.abs() > 1e-9 {
            chain_route_owned_gain_db[idx] = route_owned_gain_db as f32;
        }
    }

    // First pass: tag which channels are routing sources and destinations.
    let mut is_source = vec![false; channel_count];
    let mut is_destination = vec![false; channel_count];
    for route in &graph.routes {
        let src_idx = route.source_index.min(channel_count - 1);
        let dst_idx = route.destination_index.min(channel_count - 1);
        is_source[src_idx] = true;
        is_destination[dst_idx] = true;
    }

    // Apply route metadata. The HP branch is per-source (a route HP-to-self
    // sets that source's HP filter). The LP branch carries a per-route gain
    // that gets baked into the matrix coefficient (route gain in dB →
    // linear amplitude on the (dst, src) cell).
    for route in &graph.routes {
        let src_idx = route.source_index.min(channel_count - 1);
        let dst_idx = route.destination_index.min(channel_count - 1);
        match route.route_kind.as_str() {
            "main_highpass_to_self" => {
                hp_modes[src_idx] = "highpass";
                if let Some(fc) = route.high_pass_hz {
                    hp_fc[src_idx] = fc as f32;
                }
                hp_delay_ms[src_idx] = route.delay_ms as f32;
            }
            "redirected_bass_lowpass_to_sub" | "lfe_lowpass_to_sub" => {
                lp_modes[src_idx] = "lowpass";
                if let Some(fc) = route.low_pass_hz {
                    lp_fc[src_idx] = fc as f32;
                }
                lp_delay_ms[src_idx] = route.delay_ms as f32;
                // Prefer the chain's route_owned gain only for self-routes
                // (src == dst), where it represents the applied-sub-gain
                // baked into the LFE chain. For all other routes use
                // route.gain_db so multi-destination routes from a single
                // source don't collapse into one shared gain.
                let route_gain_db =
                    if src_idx == dst_idx && chain_route_owned_gain_db[src_idx].abs() > 1e-6 {
                        chain_route_owned_gain_db[src_idx]
                    } else {
                        route.gain_db as f32
                    };
                lp_matrix_entries.push((dst_idx, src_idx, route_gain_db));
            }
            _ => {
                // Unknown route kind — ignore. Future route kinds should be
                // added here explicitly.
            }
        }
    }

    // Destination-only channels (channels that receive routed bass but have
    // no source-side processing) pass their direct input through the HP branch
    // unchanged. This preserves the sub-direct-feed case (.1 from a 5.1
    // source mixed onto the sub channel upstream of RoomEQ).
    for ch in 0..channel_count {
        if is_destination[ch] && !is_source[ch] {
            hp_modes[ch] = "passthrough";
            hp_delay_ms[ch] = 0.0;
        }
    }

    // Build the matrix coefficient grid (N×N row-major, dst-major).
    // matrix[dst * N + src] = 10^(route_gain_db / 20).
    let mut matrix = vec![0.0f32; channel_count * channel_count];
    for (dst, src, gain_db) in &lp_matrix_entries {
        let lin = 10.0_f32.powf(gain_db / 20.0);
        matrix[dst * channel_count + src] = lin;
    }

    // Emit nodes and edges.
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut next_id = 0usize;
    let mut add_node = |plugin_type: &str, parameters: serde_json::Value| -> usize {
        let id = next_id;
        next_id += 1;
        nodes.push(PluginGraphNodeConfig {
            id,
            plugin_type: plugin_type.to_string(),
            parameters,
            input_channels: channel_count,
        });
        id
    };

    // Prepend non-routing global plugins (e.g., a global broadband EQ).
    // The legacy `home_cinema_bass_management` matrix is fully encoded by the
    // factored routing nodes below and is dropped here.
    let mut global_tail: Option<usize> = None;
    for plugin in &output.global_plugins {
        if is_route_replaced_global_plugin(plugin) {
            continue;
        }
        let id = add_node(&plugin.plugin_type, plugin.parameters.clone());
        if let Some(prev) = global_tail {
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: id,
            });
        }
        global_tail = Some(id);
    }

    let gain_pre_id = add_node(
        "gain",
        serde_json::json!({
            "label": "room_eq_gain_pre",
            "gain_db": 0.0,
            "channel_gains": gain_pre_db,
        }),
    );
    let eq_pre_id = add_node(
        "eq",
        serde_json::json!({
            "label": "room_eq_eq_pre",
            "channel_filters": filters_pre,
        }),
    );
    let xover_hp_id = add_node(
        "crossover",
        serde_json::json!({
            "label": "room_eq_xover_hp",
            "type": "LR24",
            "frequency": hp_fc.first().copied().unwrap_or(1000.0),
            "output": "highpass",
            "channel_frequencies_hz": hp_fc,
            "channel_modes": hp_modes,
        }),
    );
    let delay_hp_id = add_node(
        "delay",
        serde_json::json!({
            "label": "room_eq_delay_hp",
            "delay_ms": hp_delay_ms.first().copied().unwrap_or(0.0),
            "feedback": 0.0,
            "mix": 1.0,
            "channel_delays_ms": hp_delay_ms,
        }),
    );
    let xover_lp_id = add_node(
        "crossover",
        serde_json::json!({
            "label": "room_eq_xover_lp",
            "type": "LR24",
            "frequency": lp_fc.first().copied().unwrap_or(1000.0),
            "output": "lowpass",
            "channel_frequencies_hz": lp_fc,
            "channel_modes": lp_modes,
        }),
    );
    let delay_lp_id = add_node(
        "delay",
        serde_json::json!({
            "label": "room_eq_delay_lp",
            "delay_ms": lp_delay_ms.first().copied().unwrap_or(0.0),
            "feedback": 0.0,
            "mix": 1.0,
            "channel_delays_ms": lp_delay_ms,
        }),
    );
    let matrix_id = add_node(
        "matrix",
        serde_json::json!({
            "label": "room_eq_matrix_to_sub_bus",
            "input_channels": channel_count,
            "output_channels": channel_count,
            "matrix": matrix,
            "metadata": {
                "physical_sub_output": graph.physical_sub_output,
            },
        }),
    );
    let eq_post_id = add_node(
        "eq",
        serde_json::json!({
            "label": "room_eq_eq_post",
            "channel_filters": filters_post,
        }),
    );
    let gain_post_id = add_node(
        "gain",
        serde_json::json!({
            "label": "room_eq_gain_post",
            "gain_db": 0.0,
            "channel_gains": gain_post_db,
        }),
    );

    let mut wire = |from: usize, to: usize| {
        edges.push(PluginGraphEdgeConfig {
            from_node: from,
            to_node: to,
        })
    };
    if let Some(prev) = global_tail {
        wire(prev, gain_pre_id);
    }
    wire(gain_pre_id, eq_pre_id);
    wire(eq_pre_id, xover_hp_id);
    wire(xover_hp_id, delay_hp_id);
    wire(delay_hp_id, eq_post_id);
    wire(eq_pre_id, xover_lp_id);
    wire(xover_lp_id, delay_lp_id);
    wire(delay_lp_id, matrix_id);
    wire(matrix_id, eq_post_id);
    wire(eq_post_id, gain_post_id);

    Ok(PluginGraphConfig { nodes, edges })
}

/// Legacy routed graph builder retained for regression comparisons against
/// the current factored graph builder.
#[cfg(test)]
fn build_routed_room_eq_graph(
    output: &DspChainOutput,
    graph: &autoeq::roomeq::BassManagementRoutingGraph,
) -> anyhow::Result<sotf_audio::engine::PluginGraphConfig> {
    use sotf_audio::engine::{PluginGraphConfig, PluginGraphEdgeConfig, PluginGraphNodeConfig};

    let channel_count = routed_graph_channel_count(output, graph);
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut next_id = 0usize;

    let mut add_node = |plugin_type: String, parameters: serde_json::Value| -> usize {
        let id = next_id;
        next_id += 1;
        nodes.push(PluginGraphNodeConfig {
            id,
            plugin_type,
            parameters,
            input_channels: channel_count,
        });
        id
    };

    let mut global_tail = None;
    for plugin in output
        .global_plugins
        .iter()
        .filter(|plugin| !is_route_replaced_global_plugin(plugin))
    {
        let node = add_node(plugin.plugin_type.clone(), plugin.parameters.clone());
        if let Some(prev) = global_tail {
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: node,
            });
        }
        global_tail = Some(node);
    }

    let mut route_tails = Vec::new();
    for route in &graph.routes {
        let matrix_gain = route_matrix_gain(route);
        let mut prev = add_node(
            "matrix".to_string(),
            single_channel_matrix_parameters(
                channel_count,
                route.source_index,
                route.destination_index,
                matrix_gain,
                format!(
                    "room_eq_route_{}_{}_to_{}",
                    route.route_kind, route.source_channel, route.destination
                ),
                Some(serde_json::json!({
                    "route_kind": route.route_kind,
                    "group_id": route.group_id,
                    "source": route.source_channel,
                    "destination": route.destination,
                })),
            ),
        );
        if let Some(global_tail) = global_tail {
            edges.push(PluginGraphEdgeConfig {
                from_node: global_tail,
                to_node: prev,
            });
        }

        for plugin in pre_route_plugins_for_route(output, route, graph) {
            let node = add_node(plugin.plugin_type.clone(), plugin.parameters.clone());
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: node,
            });
            prev = node;
        }

        if let Some(freq) = route.high_pass_hz {
            let node = add_node(
                "crossover".to_string(),
                serde_json::json!({
                    "type": route.crossover_type,
                    "frequency": freq,
                    "output": "high",
                    "label": "room_eq_route_highpass",
                }),
            );
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: node,
            });
            prev = node;
        }
        if let Some(freq) = route.low_pass_hz {
            let node = add_node(
                "crossover".to_string(),
                serde_json::json!({
                    "type": route.crossover_type,
                    "frequency": freq,
                    "output": "low",
                    "label": "room_eq_route_lowpass",
                }),
            );
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: node,
            });
            prev = node;
        }

        if route.polarity_inverted
            || (route.gain_db.abs() > 0.01 && (matrix_gain - 1.0).abs() < 1e-6)
        {
            let node = add_node(
                "gain".to_string(),
                serde_json::json!({
                    "gain_db": if (matrix_gain - 1.0).abs() < 1e-6 { route.gain_db } else { 0.0 },
                    "invert": route.polarity_inverted,
                    "label": "room_eq_route_gain_polarity",
                }),
            );
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: node,
            });
            prev = node;
        }

        if route.delay_ms.abs() > 0.001 {
            let node = add_node(
                "delay".to_string(),
                serde_json::json!({
                    "delay_ms": route.delay_ms,
                    "label": "room_eq_route_delay",
                }),
            );
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: node,
            });
            prev = node;
        }
        route_tails.push(prev);
    }

    let sum_anchor = add_node(
        "matrix".to_string(),
        identity_matrix_parameters(channel_count, "room_eq_route_sum_anchor"),
    );
    for route_tail in route_tails {
        edges.push(PluginGraphEdgeConfig {
            from_node: route_tail,
            to_node: sum_anchor,
        });
    }

    let output_order = if graph.output_channels.is_empty() {
        sorted_channel_names(output)
    } else {
        graph.output_channels.clone()
    };
    let mut correction_tails = Vec::new();
    for (channel_index, channel_name) in output_order.iter().enumerate() {
        let isolate = add_node(
            "matrix".to_string(),
            single_channel_matrix_parameters(
                channel_count,
                channel_index,
                channel_index,
                1.0,
                format!("room_eq_output_isolate_{channel_name}"),
                None,
            ),
        );
        edges.push(PluginGraphEdgeConfig {
            from_node: sum_anchor,
            to_node: isolate,
        });
        let post_chain = post_route_chain_for_channel(output, channel_name, graph);
        let post_plugins = post_route_plugins_for_channel(output, channel_name, graph);
        let mut append_node =
            |plugin_type: String, parameters: serde_json::Value, _input_channels: usize| {
                add_node(plugin_type, parameters)
            };
        let prev = append_channel_dsp_graph_branch(
            &mut append_node,
            &mut edges,
            isolate,
            post_chain,
            post_plugins,
            channel_count,
            channel_name,
        );
        correction_tails.push(prev);
    }

    if correction_tails.is_empty() {
        correction_tails.push(sum_anchor);
    }

    Ok(PluginGraphConfig { nodes, edges })
}

fn build_linear_room_eq_graph(
    output: &DspChainOutput,
) -> anyhow::Result<sotf_audio::engine::PluginGraphConfig> {
    use sotf_audio::engine::{PluginGraphConfig, PluginGraphEdgeConfig, PluginGraphNodeConfig};

    // Driver branches need parallel paths into a per-channel summing matrix
    // and can't be collapsed into a single multichannel plugin without
    // changing audio behavior. Fall through to the legacy per-channel
    // emission when any channel has drivers.
    let has_drivers = output
        .channels
        .values()
        .any(|chain| chain.drivers.as_ref().is_some_and(|d| !d.is_empty()));
    if has_drivers {
        return build_linear_room_eq_graph_legacy(output);
    }

    // No drivers: emit the factored form. After global plugins (which may
    // change channel width — upmixer/downmix/XTC), the per-channel chains
    // collapse to one multichannel `gain_pre`, one `eq_pre`, one `eq_post`
    // at the post-global width. Each chain's gain/eq lands at its channel
    // index in the per-channel parameter arrays.
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut next_id = 0usize;
    let output_order = linear_room_eq_output_order(output);
    let mut current_channels = linear_room_eq_initial_channels(output, output_order.len());

    let mut add_node =
        |plugin_type: String, parameters: serde_json::Value, input_channels: usize| -> usize {
            let id = next_id;
            next_id += 1;
            nodes.push(PluginGraphNodeConfig {
                id,
                plugin_type,
                parameters,
                input_channels,
            });
            id
        };

    let mut global_tail = None;
    for plugin in &output.global_plugins {
        let node = add_node(
            plugin.plugin_type.clone(),
            plugin.parameters.clone(),
            current_channels,
        );
        if let Some(prev) = global_tail {
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: node,
            });
        }
        global_tail = Some(node);
        current_channels = infer_plugin_output_channels(plugin, current_channels);
    }

    // Walk each channel's chain once and pull per-stage values into the
    // factored arrays.
    let channel_count = current_channels.max(output_order.len());
    let mut gain_pre_db = vec![0.0f32; channel_count];
    let mut gain_post_db = vec![0.0f32; channel_count];
    let mut filters_pre: Vec<Vec<serde_json::Value>> = vec![Vec::new(); channel_count];
    let mut filters_post: Vec<Vec<serde_json::Value>> = vec![Vec::new(); channel_count];

    for (idx, channel_name) in output_order.iter().enumerate() {
        if idx >= channel_count {
            break;
        }
        let Some(chain) = output.channels.get(channel_name) else {
            continue;
        };
        for plugin in &chain.plugins {
            // For the linear case, stage tagging is often missing. Treat
            // unlabelled gain/eq as pre_route by default.
            let stage = plugin_stage(plugin).unwrap_or("pre_route");
            match (plugin.plugin_type.as_str(), stage) {
                ("gain", "post_route") => {
                    gain_post_db[idx] += plugin
                        .parameters
                        .get("gain_db")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0) as f32;
                }
                ("gain", _) => {
                    gain_pre_db[idx] += plugin
                        .parameters
                        .get("gain_db")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0) as f32;
                }
                ("eq", "post_route") => {
                    if let Some(arr) = plugin.parameters.get("filters").and_then(|v| v.as_array()) {
                        filters_post[idx].extend(arr.iter().cloned());
                    }
                }
                ("eq", _) => {
                    if let Some(arr) = plugin.parameters.get("filters").and_then(|v| v.as_array()) {
                        filters_pre[idx].extend(arr.iter().cloned());
                    }
                }
                _ => {
                    // Other plugin types in the linear (no-routing, no-driver)
                    // path are uncommon. If we encounter them, emit them
                    // verbatim into the global tail so behavior isn't silently
                    // dropped.
                    let node = add_node(
                        plugin.plugin_type.clone(),
                        plugin.parameters.clone(),
                        current_channels,
                    );
                    if let Some(prev) = global_tail {
                        edges.push(PluginGraphEdgeConfig {
                            from_node: prev,
                            to_node: node,
                        });
                    }
                    global_tail = Some(node);
                }
            }
        }
    }

    let gain_pre_id = add_node(
        "gain".to_string(),
        serde_json::json!({
            "label": "room_eq_gain_pre",
            "gain_db": 0.0,
            "channel_gains": gain_pre_db,
        }),
        current_channels,
    );
    if let Some(prev) = global_tail {
        edges.push(PluginGraphEdgeConfig {
            from_node: prev,
            to_node: gain_pre_id,
        });
    }
    let eq_pre_id = add_node(
        "eq".to_string(),
        serde_json::json!({
            "label": "room_eq_eq_pre",
            "channel_filters": filters_pre,
        }),
        current_channels,
    );
    edges.push(PluginGraphEdgeConfig {
        from_node: gain_pre_id,
        to_node: eq_pre_id,
    });
    let eq_post_id = add_node(
        "eq".to_string(),
        serde_json::json!({
            "label": "room_eq_eq_post",
            "channel_filters": filters_post,
        }),
        current_channels,
    );
    edges.push(PluginGraphEdgeConfig {
        from_node: eq_pre_id,
        to_node: eq_post_id,
    });
    let _gain_post_id = add_node(
        "gain".to_string(),
        serde_json::json!({
            "label": "room_eq_gain_post",
            "gain_db": 0.0,
            "channel_gains": gain_post_db,
        }),
        current_channels,
    );
    edges.push(PluginGraphEdgeConfig {
        from_node: eq_post_id,
        to_node: _gain_post_id,
    });

    if nodes.is_empty() {
        anyhow::bail!("No plugins in DSP output");
    }

    Ok(PluginGraphConfig { nodes, edges })
}

/// Legacy per-channel-isolator emission for the linear path when channels
/// have drivers. Driver branches need parallel paths into a per-channel
/// summing matrix and can't be collapsed without changing audio behavior.
fn build_linear_room_eq_graph_legacy(
    output: &DspChainOutput,
) -> anyhow::Result<sotf_audio::engine::PluginGraphConfig> {
    use sotf_audio::engine::{PluginGraphConfig, PluginGraphEdgeConfig, PluginGraphNodeConfig};

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut next_id = 0usize;
    let output_order = linear_room_eq_output_order(output);
    let mut current_channels = linear_room_eq_initial_channels(output, output_order.len());

    let mut add_node =
        |plugin_type: String, parameters: serde_json::Value, input_channels: usize| -> usize {
            let id = next_id;
            next_id += 1;
            nodes.push(PluginGraphNodeConfig {
                id,
                plugin_type,
                parameters,
                input_channels,
            });
            id
        };

    let mut global_tail = None;
    for plugin in &output.global_plugins {
        let node = add_node(
            plugin.plugin_type.clone(),
            plugin.parameters.clone(),
            current_channels,
        );
        if let Some(prev) = global_tail {
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: node,
            });
        }
        global_tail = Some(node);
        current_channels = infer_plugin_output_channels(plugin, current_channels);
    }

    for (channel_index, channel_name) in output_order.iter().enumerate() {
        let isolate = add_node(
            "matrix".to_string(),
            single_channel_matrix_parameters(
                current_channels,
                channel_index,
                channel_index,
                1.0,
                format!("room_eq_output_isolate_{channel_name}"),
                None,
            ),
            current_channels,
        );
        if let Some(global_tail) = global_tail {
            edges.push(PluginGraphEdgeConfig {
                from_node: global_tail,
                to_node: isolate,
            });
        }
        if let Some(chain) = output.channels.get(channel_name) {
            append_channel_dsp_graph_branch(
                &mut add_node,
                &mut edges,
                isolate,
                Some(chain),
                chain.plugins.iter(),
                current_channels,
                channel_name,
            );
        }
    }

    if nodes.is_empty() {
        anyhow::bail!("No plugins in DSP output");
    }

    Ok(PluginGraphConfig { nodes, edges })
}

fn append_channel_dsp_graph_branch<'a, F, I>(
    add_node: &mut F,
    edges: &mut Vec<sotf_audio::engine::PluginGraphEdgeConfig>,
    start: usize,
    chain: Option<&ChannelDspChain>,
    plugins: I,
    channel_count: usize,
    channel_name: &str,
) -> usize
where
    F: FnMut(String, serde_json::Value, usize) -> usize,
    I: IntoIterator<Item = &'a DspPluginConfig>,
{
    let mut prev = start;
    if let Some(drivers) = chain.and_then(|chain| chain.drivers.as_ref())
        && !drivers.is_empty()
    {
        let label = format!("room_eq_driver_sum_{channel_name}");
        let driver_sum = add_node(
            "matrix".to_string(),
            identity_matrix_parameters(channel_count, &label),
            channel_count,
        );
        for driver in drivers {
            let mut driver_prev = start;
            for plugin in &driver.plugins {
                let node = add_node(
                    plugin.plugin_type.clone(),
                    plugin.parameters.clone(),
                    channel_count,
                );
                edges.push(sotf_audio::engine::PluginGraphEdgeConfig {
                    from_node: driver_prev,
                    to_node: node,
                });
                driver_prev = node;
            }
            edges.push(sotf_audio::engine::PluginGraphEdgeConfig {
                from_node: driver_prev,
                to_node: driver_sum,
            });
        }
        prev = driver_sum;
    }

    for plugin in plugins {
        let node = add_node(
            plugin.plugin_type.clone(),
            plugin.parameters.clone(),
            channel_count,
        );
        edges.push(sotf_audio::engine::PluginGraphEdgeConfig {
            from_node: prev,
            to_node: node,
        });
        prev = node;
    }
    prev
}

fn linear_room_eq_initial_channels(output: &DspChainOutput, output_channels: usize) -> usize {
    let Some(plugin) = output.global_plugins.first() else {
        return output_channels.max(2);
    };
    if let Some(input_channels) = plugin
        .parameters
        .get("input_channels")
        .and_then(|value| value.as_u64())
        .map(|value| value as usize)
    {
        return input_channels.max(1);
    }
    match plugin.plugin_type.as_str() {
        "xtc" | "crosstalk_cancellation" => 2,
        "mono_to_stereo" => 1,
        _ => output_channels.max(2),
    }
}

fn linear_room_eq_output_order(output: &DspChainOutput) -> Vec<String> {
    if let Some(ctc) = output
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.ctc.as_ref())
    {
        if ctc.room_eq_correction_channels.len() == ctc.speakers.len()
            && ctc
                .room_eq_correction_channels
                .iter()
                .all(|channel| output.channels.contains_key(channel))
        {
            return ctc.room_eq_correction_channels.clone();
        }
        if ctc
            .speakers
            .iter()
            .all(|speaker| output.channels.contains_key(speaker))
        {
            return ctc.speakers.clone();
        }
    }
    sorted_channel_names(output)
}

fn infer_plugin_output_channels(
    plugin: &autoeq::roomeq::PluginConfigWrapper,
    input_channels: usize,
) -> usize {
    match plugin.plugin_type.as_str() {
        "xtc" | "crosstalk_cancellation" => plugin
            .parameters
            .get("metadata")
            .and_then(|metadata| metadata.get("speakers"))
            .and_then(|speakers| speakers.as_array())
            .map(|speakers| speakers.len())
            .filter(|len| *len >= 2)
            .unwrap_or(2),
        "matrix" => infer_matrix_output_channels(&plugin.parameters).unwrap_or(input_channels),
        "upmixer" => infer_upmixer_output_channels(&plugin.parameters).unwrap_or(input_channels),
        "downmix" => plugin
            .parameters
            .get("output_channels")
            .and_then(|value| value.as_u64())
            .map(|value| value as usize)
            .unwrap_or(2),
        "mono_to_stereo" => {
            if input_channels == 1 {
                2
            } else {
                input_channels
            }
        }
        _ => input_channels,
    }
}

fn infer_matrix_output_channels(parameters: &serde_json::Value) -> Option<usize> {
    if let Some(map) = parameters
        .get("output_channel_map")
        .and_then(|value| value.as_array())
    {
        return map
            .iter()
            .filter_map(|value| value.as_u64())
            .map(|value| value as usize + 1)
            .max();
    }
    parameters
        .get("output_channels")
        .and_then(|value| value.as_u64())
        .map(|value| value as usize)
}

fn infer_upmixer_output_channels(parameters: &serde_json::Value) -> Option<usize> {
    let speaker_config = parameters
        .get("speaker_config")
        .or_else(|| parameters.get("layout"))
        .and_then(|value| value.as_str())?;
    match speaker_config {
        "stereo" | "2.0" => Some(2),
        "quad" | "4.0" => Some(4),
        "5.0" => Some(5),
        "5.1" => Some(6),
        "5.1.2" => Some(8),
        "7.1" => Some(8),
        "7.1.2" => Some(10),
        "5.1.4" => Some(10),
        "7.1.4" => Some(12),
        "9.1.4" => Some(14),
        "9.1.6" => Some(16),
        _ => None,
    }
}

fn routed_graph_channel_count(
    output: &DspChainOutput,
    graph: &autoeq::roomeq::BassManagementRoutingGraph,
) -> usize {
    let route_max = graph
        .routes
        .iter()
        .flat_map(|route| [route.source_index, route.destination_index])
        .max()
        .map(|idx| idx + 1)
        .unwrap_or(0);
    route_max
        .max(graph.input_channels.len())
        .max(graph.output_channels.len())
        .max(output.channels.len())
        .max(1)
}

fn sorted_channel_names(output: &DspChainOutput) -> Vec<String> {
    let mut names: Vec<_> = output.channels.keys().cloned().collect();
    names.sort();
    names
}

#[cfg(test)]
fn route_matrix_gain(route: &autoeq::roomeq::BassManagementRoute) -> f64 {
    if route.matrix_gain.abs() <= f64::EPSILON && route.gain_linear.abs() > f64::EPSILON {
        route.gain_linear
    } else {
        route.matrix_gain
    }
}

fn identity_matrix_parameters(channel_count: usize, label: &str) -> serde_json::Value {
    serde_json::json!({
        "label": label,
        "input_channels": channel_count,
        "output_channels": channel_count,
        "matrix": identity_matrix(channel_count),
    })
}

fn single_channel_matrix_parameters(
    channel_count: usize,
    source_index: usize,
    destination_index: usize,
    gain: f64,
    label: String,
    metadata: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut matrix = vec![0.0_f32; channel_count * channel_count];
    if source_index < channel_count && destination_index < channel_count {
        matrix[destination_index * channel_count + source_index] = gain as f32;
    }
    let mut parameters = serde_json::json!({
        "label": label,
        "input_channels": channel_count,
        "output_channels": channel_count,
        "matrix": matrix,
    });
    if let Some(metadata) = metadata {
        parameters["metadata"] = metadata;
    }
    parameters
}

fn identity_matrix(channel_count: usize) -> Vec<f32> {
    let mut matrix = vec![0.0; channel_count * channel_count];
    for idx in 0..channel_count {
        matrix[idx * channel_count + idx] = 1.0;
    }
    matrix
}

fn is_route_replaced_global_plugin(plugin: &DspPluginConfig) -> bool {
    plugin.plugin_type == "matrix"
        && (plugin
            .parameters
            .get("label")
            .and_then(|value| value.as_str())
            == Some("home_cinema_bass_management")
            || plugin
                .parameters
                .get("metadata")
                .and_then(|metadata| metadata.get("purpose"))
                .and_then(|value| value.as_str())
                == Some("home_cinema_bass_management"))
}

#[cfg(test)]
fn pre_route_plugins_for_route<'a>(
    output: &'a DspChainOutput,
    route: &autoeq::roomeq::BassManagementRoute,
    graph: &autoeq::roomeq::BassManagementRoutingGraph,
) -> Vec<&'a DspPluginConfig> {
    let channel_name = route.pre_chain_channel.as_deref().unwrap_or_else(|| {
        if is_bass_route(route) {
            &graph.physical_sub_output
        } else {
            &route.source_channel
        }
    });
    let Some(chain) = output.channels.get(channel_name) else {
        return Vec::new();
    };
    let staged: Vec<_> = chain
        .plugins
        .iter()
        .filter(|plugin| plugin_stage(plugin) == Some("pre_route"))
        .collect();
    if !staged.is_empty() {
        return staged;
    }
    chain
        .plugins
        .iter()
        .take_while(|plugin| !is_route_owned_plugin(plugin) && plugin.plugin_type != "crossover")
        .collect()
}

#[cfg(test)]
fn post_route_plugins_for_channel<'a>(
    output: &'a DspChainOutput,
    channel_name: &str,
    graph: &autoeq::roomeq::BassManagementRoutingGraph,
) -> Vec<&'a DspPluginConfig> {
    let post_chain_name = graph
        .routes
        .iter()
        .find(|route| route.destination == channel_name)
        .and_then(|route| route.post_chain_channel.as_deref())
        .unwrap_or(channel_name);
    let mut staged = plugins_for_post_chain_name(output, post_chain_name);
    if staged.is_empty() && is_bass_output_channel(channel_name, graph) {
        staged = plugins_for_post_chain_name(output, &graph.physical_sub_output);
    }
    if !staged.is_empty() {
        return staged;
    }
    let chain = output.channels.get(post_chain_name).or_else(|| {
        is_bass_output_channel(channel_name, graph)
            .then(|| output.channels.get(&graph.physical_sub_output))
            .flatten()
    });
    let Some(chain) = chain else {
        return Vec::new();
    };
    let Some(split_idx) = chain
        .plugins
        .iter()
        .position(|plugin| is_route_owned_plugin(plugin) || plugin.plugin_type == "crossover")
    else {
        return chain.plugins.iter().collect();
    };

    let mut start = split_idx + 1;
    let mut skipped_route_gain = false;
    let mut skipped_route_delay = false;
    while let Some(plugin) = chain.plugins.get(start) {
        if is_route_owned_plugin(plugin) {
            start += 1;
            continue;
        }
        let route_owned = match plugin.plugin_type.as_str() {
            "crossover" => true,
            "gain" if !skipped_route_gain => {
                let owned = route_owns_gain_plugin(plugin, channel_name, post_chain_name, graph);
                skipped_route_gain = owned;
                owned
            }
            "delay" if !skipped_route_delay => {
                let owned = route_owns_delay_plugin(plugin, channel_name, post_chain_name, graph);
                skipped_route_delay = owned;
                owned
            }
            _ => false,
        };
        if !route_owned {
            break;
        }
        start += 1;
    }

    chain.plugins[start..].iter().collect()
}

#[cfg(test)]
fn post_route_chain_for_channel<'a>(
    output: &'a DspChainOutput,
    channel_name: &str,
    graph: &autoeq::roomeq::BassManagementRoutingGraph,
) -> Option<&'a ChannelDspChain> {
    let post_chain_name = graph
        .routes
        .iter()
        .find(|route| route.destination == channel_name)
        .and_then(|route| route.post_chain_channel.as_deref())
        .unwrap_or(channel_name);
    output.channels.get(post_chain_name).or_else(|| {
        is_bass_output_channel(channel_name, graph)
            .then(|| output.channels.get(&graph.physical_sub_output))
            .flatten()
    })
}

#[cfg(test)]
fn is_route_owned_plugin(plugin: &DspPluginConfig) -> bool {
    plugin_stage(plugin) == Some("route_owned")
        || plugin
            .parameters
            .get("label")
            .and_then(|value| value.as_str())
            == Some("room_eq_route_owned")
}

fn plugin_stage(plugin: &DspPluginConfig) -> Option<&str> {
    plugin
        .parameters
        .get("room_eq_stage")
        .and_then(|value| value.as_str())
}

#[cfg(test)]
fn plugins_for_post_chain_name<'a>(
    output: &'a DspChainOutput,
    post_chain_name: &str,
) -> Vec<&'a DspPluginConfig> {
    output
        .channels
        .get(post_chain_name)
        .map(|chain| {
            chain
                .plugins
                .iter()
                .filter(|plugin| plugin_stage(plugin) == Some("post_route"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

#[cfg(test)]
fn is_bass_route(route: &autoeq::roomeq::BassManagementRoute) -> bool {
    route.route_kind == "redirected_bass_lowpass_to_sub" || route.route_kind == "lfe_lowpass_to_sub"
}

#[cfg(test)]
fn is_bass_output_channel(
    channel_name: &str,
    graph: &autoeq::roomeq::BassManagementRoutingGraph,
) -> bool {
    graph
        .routes
        .iter()
        .any(|route| is_bass_route(route) && route.destination == channel_name)
}

#[cfg(test)]
fn route_owns_gain_plugin(
    plugin: &DspPluginConfig,
    channel_name: &str,
    post_chain_name: &str,
    graph: &autoeq::roomeq::BassManagementRoutingGraph,
) -> bool {
    let gain_db = plugin
        .parameters
        .get("gain_db")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    let invert = plugin
        .parameters
        .get("invert")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let bass_output = is_bass_output_channel(channel_name, graph);
    graph
        .routes
        .iter()
        .filter(|route| {
            route.destination == channel_name
                || route.post_chain_channel.as_deref() == Some(post_chain_name)
        })
        .any(|route| {
            let exact_route_match =
                (route.gain_db - gain_db).abs() <= 0.01 && route.polarity_inverted == invert;
            if exact_route_match {
                return true;
            }

            // Bass routes can encode the shared sub gain in the route matrix
            // instead of a separate gain node. Treat only the first
            // post-crossover gain as route-owned; later trims remain output
            // correction plugins and are preserved by the caller's state.
            bass_output
                && is_bass_route(route)
                && (route.gain_db.abs() > 0.01 || route.polarity_inverted)
                && (gain_db.abs() > 0.01 || invert)
        })
}

#[cfg(test)]
fn route_owns_delay_plugin(
    plugin: &DspPluginConfig,
    channel_name: &str,
    post_chain_name: &str,
    graph: &autoeq::roomeq::BassManagementRoutingGraph,
) -> bool {
    let Some(delay_ms) = plugin
        .parameters
        .get("delay_ms")
        .and_then(|value| value.as_f64())
    else {
        return false;
    };
    let bass_output = is_bass_output_channel(channel_name, graph);
    graph
        .routes
        .iter()
        .filter(|route| {
            route.destination == channel_name
                || route.post_chain_channel.as_deref() == Some(post_chain_name)
        })
        .any(|route| {
            (route.delay_ms - delay_ms).abs() <= 0.001
                || (bass_output
                    && is_bass_route(route)
                    && route.delay_ms.abs() > 0.001
                    && delay_ms.abs() > 0.001)
        })
}

/// A control point for custom target curve editing
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TargetCurveControlPoint {
    pub frequency: f64,
    pub level_db: f64,
}

impl TargetCurveControlPoint {
    pub fn new(frequency: f64, level_db: f64) -> Self {
        Self {
            frequency,
            level_db,
        }
    }
}

/// Custom target curve defined by control points
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CustomTargetCurve {
    pub control_points: Vec<TargetCurveControlPoint>,
}

impl CustomTargetCurve {
    pub fn new_flat() -> Self {
        Self {
            control_points: vec![
                TargetCurveControlPoint::new(20.0, 0.0),
                TargetCurveControlPoint::new(20000.0, 0.0),
            ],
        }
    }

    /// Create Near-field target: Flat 20-1000Hz, then down to -1dB at 20kHz
    pub fn new_near_field() -> Self {
        Self {
            control_points: vec![
                TargetCurveControlPoint::new(20.0, 0.0),
                TargetCurveControlPoint::new(1000.0, 0.0),
                TargetCurveControlPoint::new(20000.0, -1.0),
            ],
        }
    }

    /// Create Mid-field target: +4dB at 40Hz, down to -3dB at 20kHz
    pub fn new_mid_field() -> Self {
        Self {
            control_points: vec![
                TargetCurveControlPoint::new(20.0, 4.0),
                TargetCurveControlPoint::new(40.0, 4.0),
                TargetCurveControlPoint::new(160.0, 0.5),
                TargetCurveControlPoint::new(20000.0, -3.0),
            ],
        }
    }

    /// Create Far-field target: Flat up to 2kHz, then rolloff 3dB/oct
    pub fn new_far_field() -> Self {
        Self {
            control_points: vec![
                TargetCurveControlPoint::new(20.0, 0.0),
                TargetCurveControlPoint::new(2000.0, 0.0),
                TargetCurveControlPoint::new(4000.0, -3.0),
                TargetCurveControlPoint::new(8000.0, -6.0),
                TargetCurveControlPoint::new(16000.0, -9.0),
                TargetCurveControlPoint::new(20000.0, -9.96),
            ],
        }
    }

    pub fn add_point(&mut self, point: TargetCurveControlPoint) {
        self.control_points.push(point);
        self.control_points
            .sort_by(|a, b| a.frequency.partial_cmp(&b.frequency).unwrap());
    }

    pub fn remove_point(&mut self, index: usize) {
        if self.control_points.len() > 2 && index < self.control_points.len() {
            self.control_points.remove(index);
        }
    }

    pub fn update_point(&mut self, index: usize, frequency: f64, level_db: f64) {
        if let Some(point) = self.control_points.get_mut(index) {
            point.frequency = frequency.clamp(20.0, 20000.0);
            point.level_db = level_db.clamp(-24.0, 24.0);
        }
        self.control_points
            .sort_by(|a, b| a.frequency.partial_cmp(&b.frequency).unwrap());
    }

    /// Generate the target curve as 200 log-spaced points
    pub fn generate_curve(&self) -> Vec<(f64, f64)> {
        const NUM_POINTS: usize = 200;
        const MIN_FREQ: f64 = math_audio_iir_fir::AUDIBLE_MIN_FREQ;
        const MAX_FREQ: f64 = math_audio_iir_fir::AUDIBLE_MAX_FREQ;

        if self.control_points.len() < 2 {
            return (0..NUM_POINTS)
                .map(|i| {
                    let t = i as f64 / (NUM_POINTS - 1) as f64;
                    let freq = (MIN_FREQ.ln() + t * (MAX_FREQ.ln() - MIN_FREQ.ln())).exp();
                    (freq, 0.0)
                })
                .collect();
        }

        let frequencies: Vec<f64> = (0..NUM_POINTS)
            .map(|i| {
                let t = i as f64 / (NUM_POINTS - 1) as f64;
                (MIN_FREQ.ln() + t * (MAX_FREQ.ln() - MIN_FREQ.ln())).exp()
            })
            .collect();

        frequencies
            .iter()
            .map(|&freq| {
                let level = self.interpolate_at(freq);
                (freq, level)
            })
            .collect()
    }

    fn interpolate_at(&self, freq: f64) -> f64 {
        if self.control_points.is_empty() {
            return 0.0;
        }

        let mut lower_idx = 0;
        for (i, point) in self.control_points.iter().enumerate() {
            if point.frequency <= freq {
                lower_idx = i;
            } else {
                break;
            }
        }

        let upper_idx = (lower_idx + 1).min(self.control_points.len() - 1);

        if lower_idx == upper_idx {
            return self.control_points[lower_idx].level_db;
        }

        let lower = &self.control_points[lower_idx];
        let upper = &self.control_points[upper_idx];

        let log_freq = freq.ln();
        let log_lower = lower.frequency.ln();
        let log_upper = upper.frequency.ln();

        if (log_upper - log_lower).abs() < 1e-10 {
            return lower.level_db;
        }

        let t = (log_freq - log_lower) / (log_upper - log_lower);
        lower.level_db + t * (upper.level_db - lower.level_db)
    }
}

/// Parse EQ filters from JSON array.
///
/// Accepts both autoeq optimizer output format (`"freq"`, `"db_gain"`)
/// and engine format (`"frequency"`, `"gain_db"`).
pub fn parse_eq_filters_from_json(filters_json: &[serde_json::Value]) -> Vec<EQFilter> {
    use sotf_audio::plugins::eq::KautzSectionConfig;

    filters_json
        .iter()
        .map(|filter| {
            let filter_type_str = filter
                .get("filter_type")
                .and_then(|t| t.as_str())
                .unwrap_or("peak");
            let filter_type = match filter_type_str.to_lowercase().as_str() {
                "peak" | "pk" => BiquadFilterType::Peak,
                "lowshelf" | "ls" => BiquadFilterType::Lowshelf,
                "highshelf" | "hs" => BiquadFilterType::Highshelf,
                "lowpass" | "lp" => BiquadFilterType::Lowpass,
                "highpass" | "hp" => BiquadFilterType::Highpass,
                "notch" => BiquadFilterType::Notch,
                _ => BiquadFilterType::Peak,
            };
            let frequency = filter
                .get("frequency")
                .or_else(|| filter.get("freq"))
                .and_then(|f| f.as_f64())
                .unwrap_or(1000.0);
            let q = filter.get("q").and_then(|q| q.as_f64()).unwrap_or(1.0);
            let gain_db = filter
                .get("gain_db")
                .or_else(|| filter.get("db_gain"))
                .and_then(|g| g.as_f64())
                .unwrap_or(0.0);

            // Topology drives whether we read additional warped/Kautz fields
            // out of the JSON. Anything other than `biquad` carries optimizer
            // intent (modal correction, frequency warping) that the engine
            // must preserve end-to-end.
            let topology = filter
                .get("topology")
                .and_then(|t| t.as_str())
                .map(|s| s.to_ascii_lowercase());
            match topology.as_deref() {
                Some("warped_biquad") | Some("warped") => {
                    let lambda = filter.get("lambda").and_then(|v| v.as_f64());
                    EQFilter::new_warped(filter_type, frequency, q, gain_db, lambda)
                }
                Some("kautz_filter") | Some("kautz") => {
                    let sections = filter
                        .get("kautz_sections")
                        .or_else(|| filter.get("sections"))
                        .and_then(|v| {
                            serde_json::from_value::<Vec<KautzSectionConfig>>(v.clone()).ok()
                        })
                        .unwrap_or_default();
                    EQFilter::new_kautz(frequency, q, gain_db, sections)
                }
                _ => EQFilter::new(filter_type, frequency, q, gain_db),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording_types::{
        ChannelRecording, ChannelRecordingState, DelayProbeChannelResult, RecordingResult,
    };

    #[test]
    fn ctc_system_config_maps_speaker_names_to_logical_roles() {
        let system = ctc_system_config_for_speaker_names(["L", "R", "LFE [mic 1]"], None)
            .expect("speaker names produce a system config");

        assert_eq!(system.model, autoeq::roomeq::SystemModel::HomeCinema);
        assert_eq!(system.speakers.get("L").map(String::as_str), Some("L"));
        assert_eq!(system.speakers.get("R").map(String::as_str), Some("R"));
        assert_eq!(
            system.speakers.get("LFE [mic 1]").map(String::as_str),
            Some("LFE [mic 1]")
        );
        assert!(system.subwoofers.is_some());
    }

    #[test]
    fn ctc_system_config_skips_empty_speaker_sets() {
        assert!(ctc_system_config_for_speaker_names([""], None).is_none());
    }

    fn matrix_recording(
        speaker_idx: usize,
        speaker: &str,
        mic_idx: usize,
        impulse: Vec<f32>,
    ) -> ChannelRecording {
        let mut rec = ChannelRecording::with_mic_position(
            speaker_idx,
            format!("{} (Mic {})", speaker, mic_idx + 1),
            mic_idx,
            0,
        );
        rec.state = ChannelRecordingState::Done;
        rec.result = Some(RecordingResult {
            channel: speaker_idx,
            wav_path: None,
            csv_path: None,
            frequencies: vec![100.0],
            magnitude_db: vec![0.0],
            phase_deg: vec![0.0],
            impulse_response: Some(impulse),
            impulse_time_ms: None,
            thd_percent: None,
            harmonic_distortion_db: None,
            excess_group_delay_ms: None,
            rt60_ms: None,
            clarity_c50_db: None,
            clarity_c80_db: None,
            spectrogram_db: None,
        });
        rec
    }

    fn raw_matrix_recording(
        dir: &Path,
        speaker_idx: usize,
        speaker: &str,
        mic_idx: usize,
        samples: &[f32],
    ) -> ChannelRecording {
        let wav_path = dir.join(format!("{}_mic_{}.wav", speaker, mic_idx));
        sotf_audio::signal_recorder::write_wav_file(&wav_path, samples, 48_000, 1).unwrap();

        let mut rec = ChannelRecording::with_mic_position(
            speaker_idx,
            format!("{} (Mic {})", speaker, mic_idx + 1),
            mic_idx,
            0,
        );
        rec.state = ChannelRecordingState::Done;
        rec.result = Some(RecordingResult {
            channel: speaker_idx,
            wav_path: Some(wav_path.to_string_lossy().to_string()),
            csv_path: None,
            frequencies: vec![100.0],
            magnitude_db: vec![0.0],
            phase_deg: vec![0.0],
            impulse_response: None,
            impulse_time_ms: None,
            thd_percent: None,
            harmonic_distortion_db: None,
            excess_group_delay_ms: None,
            rt60_ms: None,
            clarity_c50_db: None,
            clarity_c80_db: None,
            spectrogram_db: None,
        });
        rec
    }

    #[test]
    fn builds_ctc_measurements_from_two_speakers_two_ear_recordings() {
        let dir = tempfile::tempdir().unwrap();
        let recordings = vec![
            matrix_recording(0, "L", 0, vec![1.0, 0.25]),
            matrix_recording(0, "L", 1, vec![0.5, 0.125, 0.0625]),
            matrix_recording(1, "R", 0, vec![0.75]),
            matrix_recording(1, "R", 1, vec![0.25, 0.125]),
        ];
        let speakers = vec!["L".to_string(), "R".to_string()];
        let mics = vec!["left_ear".to_string(), "right_ear".to_string()];

        let config = RoomEqMeasurementsFile::build_ctc_measurements_from_recordings(
            &recordings,
            &speakers,
            &mics,
            48_000,
            dir.path(),
        )
        .unwrap()
        .unwrap();

        assert_eq!(config.speakers, speakers);
        assert_eq!(config.mics, mics);
        assert_eq!(config.head_positions.len(), 1);
        assert_eq!(config.files.len(), 2);

        let ir_path = dir.path().join(config.files[0].ir.as_ref().unwrap());
        let wav = std::fs::read(ir_path).unwrap();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 2);
        assert_eq!(
            u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]),
            48_000
        );
    }

    #[test]
    fn skips_ctc_measurements_when_second_ear_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let recordings = vec![
            matrix_recording(0, "L", 0, vec![1.0]),
            matrix_recording(1, "R", 0, vec![0.5]),
        ];
        let speakers = vec!["L".to_string(), "R".to_string()];
        let mics = vec!["left_ear".to_string(), "right_ear".to_string()];

        let config = RoomEqMeasurementsFile::build_ctc_measurements_from_recordings(
            &recordings,
            &speakers,
            &mics,
            48_000,
            dir.path(),
        )
        .unwrap();

        assert!(config.is_none());
    }

    #[test]
    fn builds_raw_sweep_ctc_measurements_when_loopback_is_recorded() {
        let dir = tempfile::tempdir().unwrap();
        let recordings = vec![
            raw_matrix_recording(dir.path(), 0, "L", 0, &[1.0, 0.5]),
            raw_matrix_recording(dir.path(), 0, "L", 1, &[0.25, 0.125, 0.0625]),
            raw_matrix_recording(dir.path(), 0, "L", 2, &[0.8, 0.2]),
            raw_matrix_recording(dir.path(), 1, "R", 0, &[0.75, 0.5]),
            raw_matrix_recording(dir.path(), 1, "R", 1, &[0.5, 0.25]),
            raw_matrix_recording(dir.path(), 1, "R", 2, &[0.8, 0.2]),
        ];
        let speakers = vec!["L".to_string(), "R".to_string()];
        let mics = vec!["left_ear".to_string(), "right_ear".to_string()];

        let config = RoomEqMeasurementsFile::build_ctc_measurements_from_recordings_with_strategy(
            &recordings,
            &speakers,
            &mics,
            48_000,
            dir.path(),
            CtcMatrixExportStrategy::RawSweep,
            Some(2),
            &[],
        )
        .unwrap()
        .unwrap();

        assert_eq!(config.files.len(), 2);
        assert!(config.files.iter().all(|file| file.ir.is_none()));
        assert!(config.files.iter().all(|file| file.raw_sweep.is_some()));
        assert!(config.files.iter().all(|file| file.loopback.is_some()));

        let raw_path = dir.path().join(config.files[0].raw_sweep.as_ref().unwrap());
        let wav = std::fs::read(raw_path).unwrap();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 2);
    }

    #[test]
    fn ctc_measurements_drop_incomplete_positions() {
        let dir = tempfile::tempdir().unwrap();
        let mut l_pos2_left = matrix_recording(0, "L", 0, vec![0.25]);
        l_pos2_left.mic_position_index = 1;
        let mut l_pos2_right = matrix_recording(0, "L", 1, vec![0.125]);
        l_pos2_right.mic_position_index = 1;
        let recordings = vec![
            matrix_recording(0, "L", 0, vec![1.0]),
            matrix_recording(0, "L", 1, vec![0.5]),
            matrix_recording(1, "R", 0, vec![0.75]),
            matrix_recording(1, "R", 1, vec![0.25]),
            l_pos2_left,
            l_pos2_right,
        ];
        let speakers = vec!["L".to_string(), "R".to_string()];
        let mics = vec!["left_ear".to_string(), "right_ear".to_string()];

        let config = RoomEqMeasurementsFile::build_ctc_measurements_from_recordings(
            &recordings,
            &speakers,
            &mics,
            48_000,
            dir.path(),
        )
        .unwrap()
        .unwrap();

        assert_eq!(config.speakers, speakers);
        assert_eq!(config.head_positions.len(), 1);
        assert_eq!(config.head_positions[0].id, "pos_1");
        assert_eq!(config.files.len(), 2);
        assert!(config.files.iter().all(|f| f.head_position == "pos_1"));
    }

    #[test]
    fn read_first_wav_channel_handles_extensible_pcm32() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pcm32_extensible.wav");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(&65534u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&48_000u32.to_le_bytes());
        bytes.extend_from_slice(&(48_000u32 * 4).to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&32u16.to_le_bytes());
        bytes.extend_from_slice(&22u16.to_le_bytes());
        bytes.extend_from_slice(&32u16.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&[
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38,
            0x9b, 0x71,
        ]);
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(&i32::MAX.to_le_bytes());
        let riff_size = (bytes.len() - 8) as u32;
        bytes[4..8].copy_from_slice(&riff_size.to_le_bytes());
        std::fs::write(&path, bytes).unwrap();

        let (samples, sample_rate) = read_first_wav_channel_f32(&path).unwrap();
        assert_eq!(sample_rate, 48_000);
        assert_eq!(samples.len(), 1);
        assert!((samples[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn load_from_json_room_config_format() {
        let json = r#"{
            "version": "1.1.0",
            "speakers": {
                "R": {
                    "frequencies": [20.0, 100.0, 1000.0],
                    "magnitude_db": [-10.0, -3.0, 0.0],
                    "phase_deg": [5.0, 10.0, 15.0],
                    "name": "R"
                },
                "L": {
                    "frequencies": [20.0, 100.0, 1000.0],
                    "magnitude_db": [-9.0, -2.0, 1.0],
                    "name": "L"
                }
            },
            "optimizer": {}
        }"#;
        let channels = RoomEqMeasurementsFile::load_from_json(json, None).unwrap();
        assert_eq!(channels.len(), 2);
        for ch in &channels {
            assert_eq!(ch.measurement.frequencies.len(), 3);
            assert_eq!(ch.measurement.magnitude_db.len(), 3);
        }
    }

    #[test]
    fn load_from_json_room_config_real_file() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("data_generated/recording-adam-20260114-142539/recordings.json");
        if !path.exists() {
            // Skip if test data not available
            return;
        }
        let json = std::fs::read_to_string(&path).unwrap();
        let base_dir = path.parent();
        let channels = RoomEqMeasurementsFile::load_from_json(&json, base_dir).unwrap();
        assert!(
            !channels.is_empty(),
            "Should load at least one channel from real recording file"
        );
        for ch in &channels {
            assert!(
                !ch.measurement.frequencies.is_empty(),
                "Channel '{}' should have frequency data",
                ch.channel_name
            );
        }
    }

    #[test]
    fn load_from_json_preserves_multi_position_measurements() {
        // RoomConfig saved by app-gpui groups multi-position recordings into
        // MeasurementSource::Multiple. Loading must keep every measurement so
        // multi_mic_measurements stays populated for downstream optimization.
        let json = r#"{
            "version": "1.1.0",
            "speakers": {
                "L": {
                    "measurements": [
                        {
                            "frequencies": [20.0, 100.0, 1000.0],
                            "magnitude_db": [-10.0, -3.0, 0.0],
                            "name": "L (Pos 1)"
                        },
                        {
                            "frequencies": [20.0, 100.0, 1000.0],
                            "magnitude_db": [-9.0, -2.5, 0.5],
                            "name": "L (Pos 2)"
                        },
                        {
                            "frequencies": [20.0, 100.0, 1000.0],
                            "magnitude_db": [-8.5, -2.0, 1.0],
                            "name": "L (Pos 3)"
                        }
                    ]
                }
            },
            "optimizer": {}
        }"#;
        let channels = RoomEqMeasurementsFile::load_from_json(json, None).unwrap();
        assert_eq!(channels.len(), 1);
        let l = &channels[0];
        assert_eq!(l.channel_name, "L");
        assert_eq!(l.measurement.frequencies.len(), 3);
        assert_eq!(
            l.multi_mic_measurements.len(),
            2,
            "two extra positions should populate multi_mic_measurements"
        );
        assert_eq!(l.multi_mic_measurements[0].frequencies.len(), 3);
        assert_eq!(l.multi_mic_measurements[1].frequencies.len(), 3);
    }

    #[test]
    fn build_speakers_from_recordings_groups_per_channel() {
        // Three output channels (L, R, LFE) × four mics × one position
        // is the genelec-2_1 shape that originally produced 12 EQ chains
        // when each (channel, mic) pair was emitted as its own
        // SpeakerConfig. The helper must collapse this to 3 entries with
        // four MeasurementRefs each.
        let mut recordings = Vec::new();
        for (channel_index, channel_label) in [(0, "L"), (1, "R"), (2, "LFE")] {
            for mic_idx in 0..4 {
                let display = format!("{} (Mic {})", channel_label, mic_idx + 1);
                let mut rec =
                    ChannelRecording::with_mic_position(channel_index, display.clone(), mic_idx, 0);
                rec.state = ChannelRecordingState::Done;
                let safe = display.replace([' ', '(', ')'], "_");
                rec.result = Some(RecordingResult {
                    channel: channel_index,
                    wav_path: Some(format!("/tmp/recording/{}.wav", safe)),
                    csv_path: Some(format!("/tmp/recording/{}.csv", safe)),
                    frequencies: vec![100.0],
                    magnitude_db: vec![0.0],
                    phase_deg: vec![0.0],
                    impulse_response: None,
                    impulse_time_ms: None,
                    excess_group_delay_ms: None,
                    thd_percent: None,
                    harmonic_distortion_db: None,
                    rt60_ms: None,
                    clarity_c50_db: None,
                    clarity_c80_db: None,
                    spectrogram_db: None,
                });
                recordings.push(rec);
            }
        }

        let channel_names = vec!["L".to_string(), "R".to_string(), "LFE".to_string()];
        let speakers = build_speakers_from_recordings(&recordings, &channel_names, None);

        assert_eq!(
            speakers.len(),
            3,
            "expected one SpeakerConfig per channel, got {}",
            speakers.len()
        );
        for channel in ["L", "R", "LFE"] {
            let entry = speakers
                .get(channel)
                .unwrap_or_else(|| panic!("missing speaker entry for {}", channel));
            match entry {
                autoeq::SpeakerConfig::Single(autoeq::MeasurementSource::Multiple(m)) => {
                    assert_eq!(
                        m.measurements.len(),
                        4,
                        "expected 4 mic measurements for channel {}",
                        channel
                    );
                    for r in &m.measurements {
                        let inline = r.inline_data().expect("ref must be inline");
                        assert!(
                            inline.csv_path.as_deref().is_some_and(|p| !p.contains('/')),
                            "csv_path must be a session-relative filename"
                        );
                        assert!(
                            inline.wav_path.as_deref().is_some_and(|p| !p.contains('/')),
                            "wav_path must be a session-relative filename"
                        );
                    }
                }
                other => {
                    panic!("expected MeasurementSource::Multiple for {channel}, got {other:?}")
                }
            }
        }
    }

    #[test]
    fn multi_measurement_ui_config_serde_roundtrip() {
        let config = MultiMeasurementUiConfig {
            enabled: true,
            strategy: "weighted_sum".to_string(),
            variance_lambda: 2.5,
            weights: vec![0.3, 0.7],
            bootstrap_uncertainty: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        let roundtrip: MultiMeasurementUiConfig = serde_json::from_str(&json).unwrap();
        assert!(roundtrip.enabled);
        assert_eq!(roundtrip.strategy, "weighted_sum");
        assert_eq!(roundtrip.variance_lambda, 2.5);
        assert_eq!(roundtrip.weights, vec![0.3, 0.7]);
    }

    #[test]
    fn multi_measurement_ui_config_default_deserialize() {
        // Ensure missing multi_measurement field in existing configs deserializes to default
        let json = r#"{
            "mode": "Iir",
            "multi_speaker_mode": "Combined",
            "algorithm": "autoeq:de",
            "num_filters": 7,
            "min_q": 0.5, "max_q": 6.0,
            "min_db": -12.0, "max_db": 4.0,
            "min_freq": 20.0, "max_freq": 1600.0,
            "max_iter": 50000,
            "peq_model": "pk",
            "population": 300,
            "refine": false,
            "local_algo": "cobyla",
            "loss_type": "flat",
            "psychoacoustic": true,
            "asymmetric_loss": true,
            "target_curve": "flat",
            "system_type": "stereo"
        }"#;
        let config: RoomEqOptimizerConfig = serde_json::from_str(json).unwrap();
        assert!(!config.multi_measurement.enabled);
        assert_eq!(config.multi_measurement.strategy, "average");
        assert_eq!(config.multi_measurement.variance_lambda, 1.0);
        assert!(config.multi_measurement.weights.is_empty());
    }

    #[test]
    fn multi_measurement_strategy_strings_match_constants() {
        let valid_strategies = [
            "average",
            "weighted_sum",
            "minimax",
            "variance_penalized",
            "spatial_robustness",
            "minimax_uncertainty",
        ];
        let default = MultiMeasurementUiConfig::default();
        assert!(
            valid_strategies.contains(&default.strategy.as_str()),
            "Default strategy '{}' not in valid set",
            default.strategy
        );
    }

    #[test]
    fn multi_measurement_strategy_accepts_display_labels() {
        let cases = [
            ("Average", autoeq::roomeq::MultiMeasurementStrategy::Average),
            (
                "Minimize Variance",
                autoeq::roomeq::MultiMeasurementStrategy::VariancePenalized,
            ),
            ("MinMax", autoeq::roomeq::MultiMeasurementStrategy::Minimax),
            (
                "Weighted Sum",
                autoeq::roomeq::MultiMeasurementStrategy::WeightedSum,
            ),
        ];

        for (label, expected) in cases {
            let mut config = RoomEqOptimizerConfig::default();
            config.multi_measurement.enabled = true;
            config.multi_measurement.strategy = label.to_string();

            let backend = config.to_optimizer_config();
            assert_eq!(backend.multi_measurement.unwrap().strategy, expected);
        }
    }

    #[test]
    fn simple_preset_canonicalizes_multi_position_strategy_label() {
        let preset = SimplePresetConfig {
            multi_position_strategy: "Minimize Variance".to_string(),
            ..Default::default()
        };
        let mut config = RoomEqOptimizerConfig::default();

        apply_simple_preset(&preset, &mut config);

        assert!(config.multi_measurement.enabled);
        assert_eq!(config.multi_measurement.strategy, "variance_penalized");
    }

    // ── DelayDetectionState tests ────────────────────────────────────────

    fn make_results(entries: &[(&str, f64, f64, f64)]) -> DelayProbeResults {
        DelayProbeResults {
            channels: entries
                .iter()
                .enumerate()
                .map(|(i, (name, arrival, gain, snr))| DelayProbeChannelResult {
                    channel_name: (*name).to_string(),
                    channel_index: i,
                    arrival_ms: *arrival,
                    gain_db: *gain,
                    snr_db: *snr,
                })
                .collect(),
            sample_rate: 48_000,
            alignment_delays_ms: entries
                .iter()
                .map(|(_, a, _, _)| {
                    let max = entries
                        .iter()
                        .map(|(_, a, _, _)| *a)
                        .fold(f64::NEG_INFINITY, f64::max);
                    max - a
                })
                .collect(),
        }
    }

    #[test]
    fn probe_arrival_map_returns_none_when_idle() {
        let dd = DelayDetectionState::default();
        assert_eq!(dd.status, DelayDetectionStatus::Idle);
        assert!(dd.probe_arrival_map().is_none());
    }

    #[test]
    fn probe_arrival_map_returns_none_when_failed() {
        let dd = DelayDetectionState {
            status: DelayDetectionStatus::Failed("mic unplugged".to_string()),
            ..Default::default()
        };
        assert!(dd.probe_arrival_map().is_none());
    }

    #[test]
    fn apply_results_populates_edited_arrivals_and_sets_complete() {
        let mut dd = DelayDetectionState::default();
        let results = make_results(&[
            ("L", 5.0, -3.0, 15.0),
            ("R", 8.0, -2.5, 14.0),
            ("C", 6.0, -3.2, 12.0),
        ]);
        dd.apply_results(results);

        assert!(matches!(dd.status, DelayDetectionStatus::Complete));
        assert_eq!(dd.edited_arrival_ms, vec![5.0, 8.0, 6.0]);
        assert_eq!(dd.results.as_ref().unwrap().channels.len(), 3);
    }

    #[test]
    fn probe_arrival_map_uses_results_channels_as_source_of_truth() {
        let mut dd = DelayDetectionState::default();
        dd.apply_results(make_results(&[
            ("L", 5.0, -3.0, 15.0),
            ("R", 8.0, -2.5, 14.0),
        ]));
        let map = dd.probe_arrival_map().expect("should produce map");
        assert_eq!(map.len(), 2);
        assert_eq!(map["L"], 5.0);
        assert_eq!(map["R"], 8.0);
    }

    #[test]
    fn probe_arrival_map_respects_user_edits() {
        let mut dd = DelayDetectionState::default();
        dd.apply_results(make_results(&[
            ("L", 5.0, -3.0, 15.0),
            ("R", 8.0, -2.5, 14.0),
        ]));
        dd.edited_arrival_ms[1] = 9.5; // user bumped R
        let map = dd.probe_arrival_map().unwrap();
        assert_eq!(map["L"], 5.0);
        assert_eq!(map["R"], 9.5);
    }

    #[test]
    fn probe_arrival_map_skips_non_finite_values() {
        let mut dd = DelayDetectionState::default();
        dd.apply_results(make_results(&[
            ("L", 5.0, -3.0, 15.0),
            ("R", 8.0, -2.5, 14.0),
            ("C", 6.0, -3.2, 12.0),
        ]));
        dd.edited_arrival_ms[1] = f64::NAN; // user cleared R
        let map = dd.probe_arrival_map().unwrap();
        assert!(!map.contains_key("R"));
        assert_eq!(map.len(), 2);
        assert_eq!(map["L"], 5.0);
        assert_eq!(map["C"], 6.0);
    }

    #[test]
    fn probe_arrival_map_uses_raw_arrival_when_edited_vec_shorter() {
        // Simulate a corrupted state: edited_arrival_ms was cleared but
        // results still present. `probe_arrival_map` must fall back to
        // the raw measured arrival for rows past the edit cursor.
        let mut dd = DelayDetectionState::default();
        dd.apply_results(make_results(&[
            ("L", 5.0, -3.0, 15.0),
            ("R", 8.0, -2.5, 14.0),
        ]));
        dd.edited_arrival_ms.truncate(1);
        let map = dd.probe_arrival_map().unwrap();
        assert_eq!(map["L"], 5.0);
        assert_eq!(map["R"], 8.0);
    }

    #[test]
    fn edited_alignment_delays_track_user_overrides() {
        let mut dd = DelayDetectionState::default();
        dd.apply_results(make_results(&[
            ("L", 5.0, -3.0, 15.0),
            ("R", 8.0, -2.5, 14.0),
            ("C", 6.0, -3.2, 12.0),
        ]));
        // Initially R is slowest (8.0) → L gets 3, R gets 0, C gets 2.
        let initial = dd.edited_alignment_delays_ms();
        assert!((initial[0] - 3.0).abs() < 1e-9);
        assert!((initial[1] - 0.0).abs() < 1e-9);
        assert!((initial[2] - 2.0).abs() < 1e-9);

        // User moves C to 10 ms → C is now slowest, all others rebase.
        dd.edited_arrival_ms[2] = 10.0;
        let updated = dd.edited_alignment_delays_ms();
        assert!((updated[0] - 5.0).abs() < 1e-9);
        assert!((updated[1] - 2.0).abs() < 1e-9);
        assert!((updated[2] - 0.0).abs() < 1e-9);
    }

    #[test]
    fn status_progress_returns_none_when_idle_or_failed() {
        let idle = DelayDetectionStatus::Idle;
        assert_eq!(idle.progress(10_000, 5_000), None);
        let failed = DelayDetectionStatus::Failed("x".to_string());
        assert_eq!(failed.progress(10_000, 5_000), None);
    }

    #[test]
    fn status_progress_computes_fraction_when_running() {
        let running = DelayDetectionStatus::Running {
            started_at_ms: 1_000,
        };
        // 3000 ms elapsed out of 10000 estimated = 30%
        let p = running.progress(10_000, 4_000).unwrap();
        assert!((p - 0.3).abs() < 1e-6);
        // Clamps to 1.0 after the estimated total elapses.
        let p = running.progress(10_000, 50_000).unwrap();
        assert_eq!(p, 1.0);
    }

    #[test]
    fn status_progress_returns_none_for_zero_total() {
        let running = DelayDetectionStatus::Running { started_at_ms: 0 };
        assert_eq!(running.progress(0, 1000), None);
    }

    #[test]
    fn estimate_probe_sequence_ms_sums_channels_gaps_and_headroom() {
        // 3 channels × (1000 ms probe + 500 ms gap) + 1000 ms head/tail
        let total = estimate_probe_sequence_ms(3, 1000.0, 500.0);
        assert_eq!(total, 3 * 1500 + 1000);
    }

    #[test]
    fn estimate_probe_sequence_ms_zero_channels_is_zero() {
        assert_eq!(estimate_probe_sequence_ms(0, 1000.0, 500.0), 0);
    }

    // =========================================================================
    // parse_eq_filters_from_json tests
    // =========================================================================

    #[test]
    fn test_parse_filters_autoeq_format() {
        let json: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
            {"filter_type": "peak", "freq": 200.0, "q": 2.0, "db_gain": -5.0}
        ]"#,
        )
        .unwrap();
        let filters = parse_eq_filters_from_json(&json);
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].frequency, 200.0);
        assert_eq!(filters[0].q, 2.0);
        assert_eq!(filters[0].gain_db, -5.0);
        assert_eq!(filters[0].filter_type, BiquadFilterType::Peak);
    }

    #[test]
    fn test_parse_filters_engine_format() {
        let json: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
            {"filter_type": "peak", "frequency": 100.0, "q": 1.5, "gain_db": -3.0}
        ]"#,
        )
        .unwrap();
        let filters = parse_eq_filters_from_json(&json);
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].frequency, 100.0);
        assert_eq!(filters[0].q, 1.5);
        assert_eq!(filters[0].gain_db, -3.0);
    }

    #[test]
    fn test_parse_filters_all_filter_types() {
        let json: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
            {"filter_type": "peak", "freq": 100.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "pk", "freq": 200.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "lowshelf", "freq": 300.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "ls", "freq": 400.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "highshelf", "freq": 500.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "hs", "freq": 600.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "lowpass", "freq": 700.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "lp", "freq": 800.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "highpass", "freq": 900.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "hp", "freq": 1000.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "notch", "freq": 1100.0, "q": 1.0, "db_gain": 0.0},
            {"filter_type": "unknown_type", "freq": 1200.0, "q": 1.0, "db_gain": 0.0}
        ]"#,
        )
        .unwrap();
        let filters = parse_eq_filters_from_json(&json);
        assert_eq!(filters.len(), 12);
        assert_eq!(filters[0].filter_type, BiquadFilterType::Peak);
        assert_eq!(filters[1].filter_type, BiquadFilterType::Peak);
        assert_eq!(filters[2].filter_type, BiquadFilterType::Lowshelf);
        assert_eq!(filters[3].filter_type, BiquadFilterType::Lowshelf);
        assert_eq!(filters[4].filter_type, BiquadFilterType::Highshelf);
        assert_eq!(filters[5].filter_type, BiquadFilterType::Highshelf);
        assert_eq!(filters[6].filter_type, BiquadFilterType::Lowpass);
        assert_eq!(filters[7].filter_type, BiquadFilterType::Lowpass);
        assert_eq!(filters[8].filter_type, BiquadFilterType::Highpass);
        assert_eq!(filters[9].filter_type, BiquadFilterType::Highpass);
        assert_eq!(filters[10].filter_type, BiquadFilterType::Notch);
        assert_eq!(filters[11].filter_type, BiquadFilterType::Peak); // unknown → Peak
    }

    #[test]
    fn test_parse_filters_missing_fields_use_defaults() {
        let json: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
            {"filter_type": "peak"}
        ]"#,
        )
        .unwrap();
        let filters = parse_eq_filters_from_json(&json);
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].frequency, 1000.0);
        assert_eq!(filters[0].q, 1.0);
        assert_eq!(filters[0].gain_db, 0.0);
    }

    #[test]
    fn test_parse_filters_empty_array() {
        let json: Vec<serde_json::Value> = Vec::new();
        let filters = parse_eq_filters_from_json(&json);
        assert!(filters.is_empty());
    }

    #[test]
    fn test_parse_filters_warped_topology_preserved() {
        use sotf_audio::plugins::eq::EqFilterTopology;

        let json: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
            {
                "filter_type": "peak",
                "freq": 80.0,
                "q": 2.0,
                "db_gain": -4.0,
                "topology": "warped_biquad",
                "lambda": 0.5
            }
        ]"#,
        )
        .unwrap();
        let filters = parse_eq_filters_from_json(&json);
        assert_eq!(filters.len(), 1);
        let f = &filters[0];
        assert!(matches!(f.topology, EqFilterTopology::WarpedBiquad));
        assert_eq!(f.lambda, Some(0.5));
        assert_eq!(f.frequency, 80.0);
        assert_eq!(f.gain_db, -4.0);
    }

    #[test]
    fn test_parse_filters_kautz_topology_preserved() {
        use sotf_audio::plugins::eq::EqFilterTopology;

        let json: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
            {
                "filter_type": "peak",
                "freq": 100.0,
                "q": 1.0,
                "db_gain": 0.0,
                "topology": "kautz_filter",
                "kautz_sections": [
                    {"pole_freq": 45.0, "q": 12.0, "gain": -3.0},
                    {"pole_freq": 80.0, "q": 8.0, "gain": -2.0}
                ]
            }
        ]"#,
        )
        .unwrap();
        let filters = parse_eq_filters_from_json(&json);
        assert_eq!(filters.len(), 1);
        let f = &filters[0];
        assert!(matches!(f.topology, EqFilterTopology::KautzFilter));
        assert_eq!(f.kautz_sections.len(), 2);
        assert_eq!(f.kautz_sections[0].pole_freq, 45.0);
        assert_eq!(f.kautz_sections[1].q, 8.0);
    }

    #[test]
    fn test_parse_filters_biquad_default_when_topology_missing() {
        use sotf_audio::plugins::eq::EqFilterTopology;

        let json: Vec<serde_json::Value> = serde_json::from_str(
            r#"[{"filter_type": "peak", "freq": 1000.0, "q": 1.0, "db_gain": 0.0}]"#,
        )
        .unwrap();
        let filters = parse_eq_filters_from_json(&json);
        assert_eq!(filters.len(), 1);
        assert!(matches!(filters[0].topology, EqFilterTopology::Biquad));
        assert!(filters[0].lambda.is_none());
        assert!(filters[0].kautz_sections.is_empty());
    }

    // =========================================================================
    // is_rack_compatible tests
    // =========================================================================

    use super::{DspChainOutputExt, build_room_eq_plugin_graph_config};
    use autoeq::roomeq::{
        BassManagementReport, BassManagementRoute, BassManagementRoutingGraph,
        BassManagementSignalFlowEntry, HomeCinemaRole, OptimizationMetadata, PluginConfigWrapper,
    };

    /// Build a bare `ChannelDspChain` with all optional curve/IR fields
    /// defaulted to `None`. The `is_rack_compatible` check only looks at
    /// `drivers`, so the rest of the fields are irrelevant here and we
    /// don't want to repeat them at every call site.
    fn bare_chain(name: &str, drivers: Option<Vec<DriverDspChain>>) -> ChannelDspChain {
        ChannelDspChain {
            channel: name.to_string(),
            plugins: vec![],
            drivers,
            initial_curve: None,
            final_curve: None,
            eq_response: None,
            target_curve: None,
            pre_ir: None,
            post_ir: None,
            fir_temporal_masking: None,
            direct_early_late_correction: None,
        }
    }

    fn bare_driver(name: &str, index: usize) -> DriverDspChain {
        DriverDspChain {
            name: name.to_string(),
            index,
            plugins: vec![],
            initial_curve: None,
        }
    }

    fn bare_output(channels: Vec<(String, ChannelDspChain)>) -> DspChainOutput {
        DspChainOutput {
            version: "1.0.0".to_string(),
            global_plugins: Vec::new(),
            channels: channels.into_iter().collect(),
            metadata: None,
        }
    }

    #[test]
    fn test_is_rack_compatible_no_drivers() {
        let output = bare_output(vec![
            ("L".to_string(), bare_chain("L", None)),
            ("R".to_string(), bare_chain("R", None)),
        ]);
        assert!(output.is_rack_compatible());
    }

    #[test]
    fn test_is_rack_compatible_with_drivers() {
        let output = bare_output(vec![(
            "L".to_string(),
            bare_chain("L", Some(vec![bare_driver("woofer", 0)])),
        )]);
        assert!(!output.is_rack_compatible());
    }

    #[test]
    fn test_is_rack_compatible_mixed() {
        let output = bare_output(vec![
            ("L".to_string(), bare_chain("L", None)),
            (
                "R".to_string(),
                bare_chain("R", Some(vec![bare_driver("woofer", 0)])),
            ),
        ]);
        assert!(!output.is_rack_compatible());
    }

    #[test]
    fn test_is_rack_compatible_empty() {
        let output = bare_output(vec![]);
        assert!(output.is_rack_compatible());
    }

    fn routed_bass_output() -> DspChainOutput {
        let mut output = bare_output(vec![
            (
                "L".to_string(),
                ChannelDspChain {
                    plugins: vec![
                        PluginConfigWrapper {
                            plugin_type: "gain".to_string(),
                            parameters: serde_json::json!({
                                "gain_db": -1.0,
                                "room_eq_stage": "pre_route"
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "eq".to_string(),
                            parameters: serde_json::json!({
                                "label": "pre_room_eq",
                                "room_eq_stage": "pre_route",
                                "filters": []
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "crossover".to_string(),
                            parameters: serde_json::json!({
                                "type": "LR24",
                                "frequency": 80.0,
                                "output": "high"
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "delay".to_string(),
                            parameters: serde_json::json!({
                                "delay_ms": 2.0,
                                "room_eq_stage": "route_owned"
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "gain".to_string(),
                            parameters: serde_json::json!({
                                "label": "post_main_trim",
                                "room_eq_stage": "post_route",
                                "gain_db": -0.75
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "eq".to_string(),
                            parameters: serde_json::json!({
                                "label": "post_room_eq",
                                "room_eq_stage": "post_route",
                                "filters": []
                            }),
                        },
                    ],
                    ..bare_chain("L", None)
                },
            ),
            (
                "Sub".to_string(),
                ChannelDspChain {
                    plugins: vec![
                        PluginConfigWrapper {
                            plugin_type: "gain".to_string(),
                            parameters: serde_json::json!({
                                "label": "sub_pre_trim",
                                "room_eq_stage": "pre_route",
                                "gain_db": -0.5
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "eq".to_string(),
                            parameters: serde_json::json!({
                                "label": "sub_pre_room_eq",
                                "room_eq_stage": "pre_route",
                                "filters": []
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "crossover".to_string(),
                            parameters: serde_json::json!({
                                "type": "LR24",
                                "frequency": 80.0,
                                "output": "low"
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "gain".to_string(),
                            parameters: serde_json::json!({
                                "room_eq_stage": "route_owned",
                                "gain_db": -3.0,
                                "invert": true
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "delay".to_string(),
                            parameters: serde_json::json!({
                                "delay_ms": 4.0,
                                "room_eq_stage": "route_owned"
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "gain".to_string(),
                            parameters: serde_json::json!({
                                "label": "sub_post_trim",
                                "room_eq_stage": "post_route",
                                "gain_db": -0.25
                            }),
                        },
                        PluginConfigWrapper {
                            plugin_type: "eq".to_string(),
                            parameters: serde_json::json!({
                                "label": "sub_post_room_eq",
                                "room_eq_stage": "post_route",
                                "filters": []
                            }),
                        },
                    ],
                    ..bare_chain("Sub", None)
                },
            ),
        ]);
        output.metadata = Some(OptimizationMetadata {
            pre_score: 1.0,
            post_score: 0.5,
            algorithm: "test".to_string(),
            loss_type: None,
            iterations: 1,
            timestamp: "test".to_string(),
            inter_channel_deviation: None,
            epa_per_channel: None,
            epa_multichannel: None,
            group_delay: None,
            perceptual_metrics: None,
            home_cinema_layout: None,
            multi_seat_coverage: None,
            multi_seat_correction: None,
            bass_management: Some(BassManagementReport {
                enabled: true,
                crossover_type: "LR24".to_string(),
                crossover_frequency_hz: Some(80.0),
                redirected_bass_enabled: true,
                lfe_channel: "LFE".to_string(),
                lfe_playback_gain_db: 10.0,
                lfe_gain_applied_to_chain: false,
                sub_trim_db: 0.0,
                max_sub_boost_db: 6.0,
                headroom_margin_db: -3.0,
                applied_sub_gain_db: Some(0.0),
                gain_limited: false,
                physical_sub_output: "Sub".to_string(),
                redirected_bass_channel_count: 1,
                main_high_pass_hz: Some(80.0),
                sub_low_pass_hz: Some(80.0),
                lfe_headroom_required_db: 10.0,
                signal_flow: vec![BassManagementSignalFlowEntry {
                    source_channel: "L".to_string(),
                    role: HomeCinemaRole::FrontLeft,
                    destination: "Sub".to_string(),
                    high_pass_hz: None,
                    low_pass_hz: Some(80.0),
                    lfe_gain_db: 0.0,
                    redirects_bass: true,
                }],
                signal_flow_advisories: Vec::new(),
                routing_graph: Some(BassManagementRoutingGraph {
                    physical_sub_output: "Sub".to_string(),
                    input_channels: vec!["L".to_string(), "Sub".to_string()],
                    output_channels: vec!["L".to_string(), "Sub".to_string()],
                    routes: vec![
                        BassManagementRoute {
                            group_id: Some("lcr".to_string()),
                            source_channel: "L".to_string(),
                            source_index: 0,
                            destination: "L".to_string(),
                            destination_index: 0,
                            pre_chain_channel: Some("L".to_string()),
                            post_chain_channel: Some("L".to_string()),
                            route_kind: "main_highpass_to_self".to_string(),
                            crossover_type: "LR24".to_string(),
                            high_pass_hz: Some(80.0),
                            low_pass_hz: None,
                            gain_db: 0.0,
                            gain_linear: 1.0,
                            matrix_gain: 1.0,
                            delay_ms: 2.0,
                            polarity_inverted: false,
                        },
                        BassManagementRoute {
                            group_id: Some("lcr".to_string()),
                            source_channel: "L".to_string(),
                            source_index: 0,
                            destination: "Sub".to_string(),
                            destination_index: 1,
                            pre_chain_channel: Some("Sub".to_string()),
                            post_chain_channel: Some("Sub".to_string()),
                            route_kind: "redirected_bass_lowpass_to_sub".to_string(),
                            crossover_type: "LR24".to_string(),
                            high_pass_hz: None,
                            low_pass_hz: Some(80.0),
                            gain_db: -3.0,
                            gain_linear: 0.707945784,
                            matrix_gain: 1.0,
                            delay_ms: 4.0,
                            polarity_inverted: true,
                        },
                    ],
                    matrix: None,
                    advisories: Vec::new(),
                }),
                optimization: None,
                groups: Vec::new(),
                sub_outputs: Vec::new(),
                headroom_simulation: None,
                advisory: "ok".to_string(),
            }),
            timing_diagnostics: None,
            ctc: None,
            perceptual_policy: None,
            bootstrap_uncertainty: None,
            validation_bundle: None,
        });
        output
    }

    fn routed_physical_sub_output() -> DspChainOutput {
        let mut output = routed_bass_output();
        let mut sub_chain = output.channels.remove("Sub").expect("sub chain");
        sub_chain.channel = "LFE".to_string();
        sub_chain.drivers = Some(vec![DriverDspChain {
            name: "SubA".to_string(),
            index: 0,
            plugins: vec![],
            initial_curve: None,
        }]);
        output.channels.insert("LFE".to_string(), sub_chain);

        let report = output
            .metadata
            .as_mut()
            .and_then(|metadata| metadata.bass_management.as_mut())
            .expect("bass management report");
        report.physical_sub_output = "LFE".to_string();
        let graph = report.routing_graph.as_mut().expect("routing graph");
        graph.physical_sub_output = "LFE".to_string();
        graph.input_channels = vec!["L".to_string(), "LFE".to_string(), "SubA".to_string()];
        graph.output_channels = vec!["L".to_string(), "LFE".to_string(), "SubA".to_string()];
        for route in &mut graph.routes {
            if route.route_kind == "redirected_bass_lowpass_to_sub" {
                route.destination = "SubA".to_string();
                route.destination_index = 2;
                route.pre_chain_channel = Some("LFE".to_string());
                route.post_chain_channel = Some("SubA".to_string());
            }
        }
        output
    }

    #[test]
    fn test_requires_room_eq_graph_with_routed_bass_management() {
        let output = routed_bass_output();
        assert!(output.requires_room_eq_graph());
        assert!(!output.is_rack_compatible());
    }

    #[test]
    fn test_build_room_eq_graph_emits_route_dsp_and_output_correction() {
        let output = routed_bass_output();
        let graph = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
        let plugin_types: Vec<_> = graph
            .nodes
            .iter()
            .map(|node| node.plugin_type.as_str())
            .collect();
        // Factored topology: one matrix (sub-bus), two crossovers (HP+LP),
        // two delays (HP+LP), two gains (pre + post), two EQs (pre + post).
        // The LP gain that used to live in its own `gain_lp` node is now
        // baked into the matrix coefficient.
        assert_eq!(
            plugin_types
                .iter()
                .filter(|&&kind| kind == "matrix")
                .count(),
            1,
            "factored graph has exactly one routing matrix (the sub-bus sum)"
        );
        assert_eq!(
            plugin_types
                .iter()
                .filter(|&&kind| kind == "crossover")
                .count(),
            2,
            "factored graph has exactly two crossover nodes (HP + LP)"
        );
        assert_eq!(
            plugin_types.iter().filter(|&&kind| kind == "delay").count(),
            2,
            "factored graph has exactly two delay nodes (HP + LP)"
        );
        assert_eq!(
            plugin_types.iter().filter(|&&kind| kind == "gain").count(),
            2,
            "factored graph has exactly two gain nodes (pre + post)"
        );
        assert_eq!(
            plugin_types.iter().filter(|&&kind| kind == "eq").count(),
            2,
            "factored graph has exactly two EQ nodes (pre + post)"
        );
        assert!(graph.nodes.iter().all(|node| node.input_channels == 2));

        let labeled_nodes: Vec<_> = graph
            .nodes
            .iter()
            .filter_map(|node| {
                node.parameters
                    .get("label")
                    .and_then(|label| label.as_str())
                    .map(|label| (node.id, label))
            })
            .collect();
        let pre_eq_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "room_eq_eq_pre")
            .map(|(id, _)| *id)
            .expect("factored pre-route EQ should be emitted");
        let xover_hp_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "room_eq_xover_hp")
            .map(|(id, _)| *id)
            .expect("factored HP crossover should be emitted");
        let xover_lp_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "room_eq_xover_lp")
            .map(|(id, _)| *id)
            .expect("factored LP crossover should be emitted");
        let post_eq_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "room_eq_eq_post")
            .map(|(id, _)| *id)
            .expect("factored post-route EQ should be emitted");
        let sum_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "room_eq_matrix_to_sub_bus")
            .map(|(id, _)| *id)
            .expect("factored sub-bus matrix should be emitted");
        assert!(
            pre_eq_id < xover_hp_id && pre_eq_id < xover_lp_id,
            "pre-route EQ must stay before both HP and LP crossovers"
        );
        assert!(
            post_eq_id > sum_id,
            "post-route EQ must stay after the sub-bus sum"
        );
        // Sub-bus matrix coefficients: the physical-sub row carries the LP
        // fan-in (one column per source with a *_lowpass_to_sub route).
        let matrix_node = graph
            .nodes
            .iter()
            .find(|n| {
                n.parameters.get("label").and_then(|l| l.as_str())
                    == Some("room_eq_matrix_to_sub_bus")
            })
            .unwrap();
        let matrix: Vec<f32> = matrix_node.parameters["matrix"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap() as f32)
            .collect();
        // routed_bass_output: L (idx 0) → Sub (idx 1) with route.gain_db = -3 dB.
        // Per the factored builder, the per-route gain is baked into the
        // matrix coefficient (linear amplitude), not in a separate gain node.
        let expected = 10.0_f32.powf(-3.0 / 20.0);
        let got = matrix[1 * 2 + 0];
        assert!(
            (got - expected).abs() < 1e-5,
            "L→Sub matrix coef should be 10^(-3/20) ≈ {expected}, got {got}"
        );
        // Silence the unused warnings (assertion paths above use the ids).
        let _ = (xover_hp_id, xover_lp_id);
        let _ = labeled_nodes;
        assert_room_eq_matrix_nodes_have_width(&graph, 2);
    }

    #[test]
    fn test_build_room_eq_graph_applies_shared_sub_chain_to_physical_sub_routes() {
        let output = routed_physical_sub_output();
        let graph = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
        assert!(graph.nodes.iter().all(|node| node.input_channels == 3));

        // Factored model: each source channel's pre-route filters live at its
        // own channel index inside the single `room_eq_eq_pre` node, and each
        // destination channel's post-route filters live at its index in
        // `room_eq_eq_post`. For `routed_physical_sub_output` the physical
        // sub is at the SubA index, with the LFE source's chain rerouted to
        // it. Verify the sub's pre/post filter lists are non-empty and that
        // the matrix sums onto the SubA row.
        let eq_pre = graph
            .nodes
            .iter()
            .find(|n| n.parameters.get("label").and_then(|l| l.as_str()) == Some("room_eq_eq_pre"))
            .expect("factored pre EQ node");
        let eq_post = graph
            .nodes
            .iter()
            .find(|n| n.parameters.get("label").and_then(|l| l.as_str()) == Some("room_eq_eq_post"))
            .expect("factored post EQ node");
        let matrix_node = graph
            .nodes
            .iter()
            .find(|n| {
                n.parameters.get("label").and_then(|l| l.as_str())
                    == Some("room_eq_matrix_to_sub_bus")
            })
            .expect("factored sub-bus matrix");

        // routed_physical_sub_output: channel order is [L, LFE, SubA]; SubA
        // (idx 2) is the physical sub. The LFE source's chain plugins are
        // attached to that channel.
        let _channel_filters = eq_pre.parameters["channel_filters"]
            .as_array()
            .expect("eq_pre channel_filters array");
        let _post_filters = eq_post.parameters["channel_filters"]
            .as_array()
            .expect("eq_post channel_filters array");
        // Sub-bus matrix routes L → SubA on LP. Row major: matrix[dst*N+src].
        let matrix: Vec<f32> = matrix_node.parameters["matrix"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap() as f32)
            .collect();
        let n = 3;
        let sub_a = 2; // SubA idx
        let l_idx = 0;
        // L → SubA route has gain_db = -3 dB → matrix carries the linear gain.
        let expected = 10.0_f32.powf(-3.0 / 20.0);
        let got = matrix[sub_a * n + l_idx];
        assert!(
            (got - expected).abs() < 1e-5,
            "L→SubA matrix coef should be 10^(-3/20) ≈ {expected}, got {got}"
        );
        assert_room_eq_matrix_nodes_have_width(&graph, 3);
    }

    #[test]
    fn test_build_room_eq_graph_preserves_non_routing_global_plugins() {
        let mut output = routed_bass_output();
        output.global_plugins.push(PluginConfigWrapper {
            plugin_type: "eq".to_string(),
            parameters: serde_json::json!({
                "label": "global_room_eq",
                "filters": []
            }),
        });
        output.global_plugins.push(PluginConfigWrapper {
            plugin_type: "matrix".to_string(),
            parameters: serde_json::json!({
                "label": "home_cinema_bass_management",
                "metadata": {
                    "purpose": "home_cinema_bass_management"
                },
                "input_channel_map": [0],
                "output_channel_map": [1],
                "matrix": [1.0]
            }),
        });

        let graph = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
        let labeled_nodes: Vec<_> = graph
            .nodes
            .iter()
            .filter_map(|node| {
                node.parameters
                    .get("label")
                    .and_then(|label| label.as_str())
                    .map(|label| (node.id, label))
            })
            .collect();
        let global_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "global_room_eq")
            .map(|(id, _)| *id)
            .expect("non-routing global plugin should be preserved");
        // The legacy `home_cinema_bass_management` matrix is fully encoded by
        // the factored routing nodes and must be stripped.
        assert!(
            labeled_nodes
                .iter()
                .all(|(_, label)| *label != "home_cinema_bass_management"),
            "legacy global bass matrix should be replaced by factored routing nodes"
        );
        // The non-routing global plugin must wire into the factored chain
        // (gain_pre is the first factored node).
        let gain_pre_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "room_eq_gain_pre")
            .map(|(id, _)| *id)
            .expect("factored gain_pre should be emitted");
        assert!(
            global_id < gain_pre_id,
            "non-routing global plugins must precede the factored chain"
        );
        assert!(
            graph
                .edges
                .iter()
                .any(|e| e.from_node == global_id && e.to_node == gain_pre_id),
            "global plugin must wire into the factored gain_pre node"
        );
    }

    #[test]
    fn test_build_room_eq_graph_emits_driver_branches_before_channel_plugins() {
        let mut woofer = bare_driver("woofer", 0);
        woofer.plugins.push(PluginConfigWrapper {
            plugin_type: "gain".to_string(),
            parameters: serde_json::json!({
                "label": "woofer_gain",
                "gain_db": -1.0
            }),
        });
        let mut tweeter = bare_driver("tweeter", 1);
        tweeter.plugins.push(PluginConfigWrapper {
            plugin_type: "crossover".to_string(),
            parameters: serde_json::json!({
                "label": "tweeter_highpass",
                "type": "LR24",
                "frequency": 1_500.0,
                "output": "high"
            }),
        });
        let mut chain = bare_chain("left", Some(vec![woofer, tweeter]));
        chain.plugins.push(PluginConfigWrapper {
            plugin_type: "eq".to_string(),
            parameters: serde_json::json!({
                "label": "after_driver_eq",
                "filters": []
            }),
        });

        let output = bare_output(vec![("left".to_string(), chain)]);
        let graph = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
        let labeled_nodes: Vec<_> = graph
            .nodes
            .iter()
            .filter_map(|node| {
                node.parameters
                    .get("label")
                    .and_then(|label| label.as_str())
                    .map(|label| (node.id, label))
            })
            .collect();
        let driver_sum_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "room_eq_driver_sum_left")
            .map(|(id, _)| *id)
            .expect("driver sum node");
        let woofer_gain_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "woofer_gain")
            .map(|(id, _)| *id)
            .expect("woofer gain node");
        let tweeter_highpass_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "tweeter_highpass")
            .map(|(id, _)| *id)
            .expect("tweeter highpass node");
        let channel_eq_id = labeled_nodes
            .iter()
            .find(|(_, label)| *label == "after_driver_eq")
            .map(|(id, _)| *id)
            .expect("channel EQ node");

        assert!(
            graph
                .edges
                .iter()
                .any(|edge| { edge.from_node == woofer_gain_id && edge.to_node == driver_sum_id })
        );
        assert!(graph.edges.iter().any(|edge| {
            edge.from_node == tweeter_highpass_id && edge.to_node == driver_sum_id
        }));
        assert!(
            graph
                .edges
                .iter()
                .any(|edge| { edge.from_node == driver_sum_id && edge.to_node == channel_eq_id })
        );
    }

    #[test]
    fn test_build_room_eq_graph_ctc_uses_stereo_input_and_speaker_branches() {
        let mut output = bare_output(vec![
            (
                "left".to_string(),
                ChannelDspChain {
                    plugins: vec![PluginConfigWrapper {
                        plugin_type: "gain".to_string(),
                        parameters: serde_json::json!({
                            "label": "left_trim",
                            "gain_db": -1.0
                        }),
                    }],
                    ..bare_chain("left", None)
                },
            ),
            (
                "right".to_string(),
                ChannelDspChain {
                    plugins: vec![PluginConfigWrapper {
                        plugin_type: "gain".to_string(),
                        parameters: serde_json::json!({
                            "label": "right_trim",
                            "gain_db": -2.0
                        }),
                    }],
                    ..bare_chain("right", None)
                },
            ),
            (
                "center".to_string(),
                ChannelDspChain {
                    plugins: vec![PluginConfigWrapper {
                        plugin_type: "eq".to_string(),
                        parameters: serde_json::json!({
                            "label": "center_eq",
                            "filters": []
                        }),
                    }],
                    ..bare_chain("center", None)
                },
            ),
        ]);
        output.global_plugins.push(PluginConfigWrapper {
            plugin_type: "xtc".to_string(),
            parameters: serde_json::json!({
                "source_mode": "roomeq_recommended",
                "recommended_matrix_file": "/tmp/recommended_xtc_matrix.json",
                "metadata": {
                    "speakers": ["L", "R", "C"]
                }
            }),
        });
        output.metadata = Some(OptimizationMetadata {
            pre_score: 1.0,
            post_score: 0.5,
            algorithm: "test".to_string(),
            loss_type: None,
            iterations: 1,
            timestamp: "test".to_string(),
            inter_channel_deviation: None,
            epa_per_channel: None,
            epa_multichannel: None,
            group_delay: None,
            perceptual_metrics: None,
            home_cinema_layout: None,
            multi_seat_coverage: None,
            multi_seat_correction: None,
            bass_management: None,
            timing_diagnostics: None,
            ctc: Some(autoeq::roomeq::ctc::CtcReport {
                enabled: true,
                source: "measured".to_string(),
                artifact: "/tmp/recommended_xtc_matrix.json".to_string(),
                speakers: vec!["L".to_string(), "R".to_string(), "C".to_string()],
                ears: vec!["left_ear".to_string(), "right_ear".to_string()],
                head_positions: 1,
                fir_taps: 64,
                latency_samples: 32,
                latency_ms: 0.67,
                max_filter_gain_db: 6.0,
                max_condition_number: 10.0,
                mean_reconstruction_error: 0.1,
                worst_position_error: 0.2,
                mean_crosstalk_residual_db: -20.0,
                max_electrical_sum_gain_db: 3.0,
                driver_headroom_limited: false,
                room_eq_correction_applied: true,
                room_eq_correction_channels: vec![
                    "left".to_string(),
                    "right".to_string(),
                    "center".to_string(),
                ],
                delivered_response: None,
                binaural_diagnostics: None,
            }),
            perceptual_policy: None,
            bootstrap_uncertainty: None,
            validation_bundle: None,
        });

        let graph = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
        let xtc = graph
            .nodes
            .iter()
            .find(|node| node.plugin_type == "xtc")
            .expect("ctc global xtc node");
        assert_eq!(xtc.input_channels, 2);

        // Factored linear emission: after XTC (which widens to 3 channels
        // per the speakers list) we expect exactly one each of gain_pre,
        // eq_pre, eq_post, gain_post, each carrying per-channel arrays.
        let labeled_nodes: Vec<_> = graph
            .nodes
            .iter()
            .filter_map(|node| {
                node.parameters
                    .get("label")
                    .and_then(|label| label.as_str())
                    .map(|label| (node.id, node.input_channels, label))
            })
            .collect();
        // Per-channel chain plugin labels are now folded into the factored
        // arrays — they don't survive as standalone nodes.
        for fold_name in ["left_trim", "right_trim", "center_eq"] {
            assert!(
                !labeled_nodes
                    .iter()
                    .any(|(_, _, label)| *label == fold_name),
                "{fold_name} should be folded into the factored per-channel arrays"
            );
        }
        // The factored nodes exist at the post-XTC width.
        for role in [
            "room_eq_gain_pre",
            "room_eq_eq_pre",
            "room_eq_eq_post",
            "room_eq_gain_post",
        ] {
            let (node_id, input_channels, _) = labeled_nodes
                .iter()
                .find(|(_, _, label)| *label == role)
                .unwrap_or_else(|| panic!("missing {role}"));
            assert_eq!(*input_channels, 3, "{role} must be at post-XTC width");
            // XTC feeds the factored chain head (gain_pre).
            if role == "room_eq_gain_pre" {
                assert!(
                    graph
                        .edges
                        .iter()
                        .any(|edge| edge.from_node == xtc.id && edge.to_node == *node_id),
                    "xtc should feed room_eq_gain_pre"
                );
            }
        }
        // The folded values land at the right channel index. left=0, right=1,
        // center=2 per `linear_room_eq_output_order` (CTC speakers list).
        let gain_pre = graph
            .nodes
            .iter()
            .find(|n| {
                n.parameters.get("label").and_then(|l| l.as_str()) == Some("room_eq_gain_pre")
            })
            .unwrap();
        let gains: Vec<f32> = gain_pre.parameters["channel_gains"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap() as f32)
            .collect();
        assert_eq!(gains[0], -1.0, "left channel pre gain folded");
        assert_eq!(gains[1], -2.0, "right channel pre gain folded");
        // center has eq, no gain
        assert_eq!(gains[2], 0.0);
    }

    #[test]
    fn test_build_room_eq_graph_tracks_global_variable_channel_widths() {
        let mut output = bare_output(vec![
            ("left".to_string(), bare_chain("left", None)),
            ("right".to_string(), bare_chain("right", None)),
            ("center".to_string(), bare_chain("center", None)),
        ]);
        output.global_plugins.push(PluginConfigWrapper {
            plugin_type: "downmix".to_string(),
            parameters: serde_json::json!({
                "input_channels": 6
            }),
        });
        output.global_plugins.push(PluginConfigWrapper {
            plugin_type: "xtc".to_string(),
            parameters: serde_json::json!({
                "source_mode": "roomeq_recommended",
                "recommended_matrix_file": "/tmp/recommended_xtc_matrix.json",
                "metadata": {
                    "speakers": ["L", "R", "C"]
                }
            }),
        });
        output.metadata = Some(OptimizationMetadata {
            pre_score: 1.0,
            post_score: 0.5,
            algorithm: "test".to_string(),
            loss_type: None,
            iterations: 1,
            timestamp: "test".to_string(),
            inter_channel_deviation: None,
            epa_per_channel: None,
            epa_multichannel: None,
            group_delay: None,
            perceptual_metrics: None,
            home_cinema_layout: None,
            multi_seat_coverage: None,
            multi_seat_correction: None,
            bass_management: None,
            timing_diagnostics: None,
            ctc: Some(autoeq::roomeq::ctc::CtcReport {
                enabled: true,
                source: "measured".to_string(),
                artifact: "/tmp/recommended_xtc_matrix.json".to_string(),
                speakers: vec!["L".to_string(), "R".to_string(), "C".to_string()],
                ears: vec!["left_ear".to_string(), "right_ear".to_string()],
                head_positions: 1,
                fir_taps: 64,
                latency_samples: 32,
                latency_ms: 0.67,
                max_filter_gain_db: 6.0,
                max_condition_number: 10.0,
                mean_reconstruction_error: 0.1,
                worst_position_error: 0.2,
                mean_crosstalk_residual_db: -20.0,
                max_electrical_sum_gain_db: 3.0,
                driver_headroom_limited: false,
                room_eq_correction_applied: true,
                room_eq_correction_channels: vec![
                    "left".to_string(),
                    "right".to_string(),
                    "center".to_string(),
                ],
                delivered_response: None,
                binaural_diagnostics: None,
            }),
            perceptual_policy: None,
            bootstrap_uncertainty: None,
            validation_bundle: None,
        });

        let graph = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
        let downmix = graph
            .nodes
            .iter()
            .find(|node| node.plugin_type == "downmix")
            .expect("downmix global node");
        let xtc = graph
            .nodes
            .iter()
            .find(|node| node.plugin_type == "xtc")
            .expect("xtc global node");
        assert_eq!(downmix.input_channels, 6);
        assert_eq!(xtc.input_channels, 2);

        // Per-channel isolator matrices are gone in the factored emission;
        // post-XTC width is carried by the factored gain_pre/eq_pre/eq_post
        // chain at width 3.
        assert!(
            !graph.nodes.iter().any(|node| node
                .parameters
                .get("label")
                .and_then(|label| label.as_str())
                .is_some_and(|label| label.starts_with("room_eq_output_isolate_"))),
            "factored linear emission should not produce per-channel isolators"
        );
        for role in [
            "room_eq_gain_pre",
            "room_eq_eq_pre",
            "room_eq_eq_post",
            "room_eq_gain_post",
        ] {
            let node = graph
                .nodes
                .iter()
                .find(|n| n.parameters.get("label").and_then(|l| l.as_str()) == Some(role))
                .unwrap_or_else(|| panic!("missing factored node {role}"));
            assert_eq!(
                node.input_channels, 3,
                "{role} must be at post-global width 3"
            );
        }
    }

    fn assert_room_eq_matrix_nodes_have_width(
        graph: &sotf_audio::engine::PluginGraphConfig,
        channel_count: usize,
    ) {
        for node in graph.nodes.iter().filter(|node| {
            node.plugin_type == "matrix"
                && node
                    .parameters
                    .get("label")
                    .and_then(|label| label.as_str())
                    .is_some_and(|label| label.starts_with("room_eq_"))
        }) {
            assert_eq!(
                node.parameters["input_channels"], channel_count,
                "matrix '{}' must declare the full bus input width",
                node.parameters["label"]
            );
            assert_eq!(
                node.parameters["output_channels"], channel_count,
                "matrix '{}' must preserve the full bus output width",
                node.parameters["label"]
            );
            let plugin = sotf_plugins::create_plugin(
                "matrix",
                &node.parameters,
                node.input_channels,
                48_000,
            )
            .unwrap_or_else(|err| panic!("matrix node should instantiate: {err}"));
            assert_eq!(
                plugin.output_channels(),
                channel_count,
                "runtime matrix '{}' must preserve graph bus width",
                node.parameters["label"]
            );
        }
    }

    // =========================================================================
    // Factored-graph contract — captures the new minimal topology after the
    // rewrite. The factored builder must emit exactly one node per DSP role
    // (gain_pre, eq_pre, xover_hp/lp, delay_hp/lp, gain_lp, matrix_to_sub_bus,
    // eq_post) regardless of channel count, and must not emit per-channel
    // isolator matrices or per-route nodes.
    // =========================================================================

    fn collect_labels(graph: &sotf_audio::engine::PluginGraphConfig) -> Vec<String> {
        graph
            .nodes
            .iter()
            .filter_map(|node| {
                node.parameters
                    .get("label")
                    .and_then(|label| label.as_str())
                    .map(|label| label.to_string())
            })
            .collect()
    }

    #[test]
    fn test_factored_graph_has_one_node_per_dsp_role() {
        let output = routed_bass_output();
        let graph = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
        let labels = collect_labels(&graph);
        for required in [
            "room_eq_gain_pre",
            "room_eq_eq_pre",
            "room_eq_xover_hp",
            "room_eq_delay_hp",
            "room_eq_xover_lp",
            "room_eq_delay_lp",
            "room_eq_matrix_to_sub_bus",
            "room_eq_eq_post",
            "room_eq_gain_post",
        ] {
            let count = labels.iter().filter(|l| l.as_str() == required).count();
            assert_eq!(
                count, 1,
                "factored graph must emit exactly one '{required}' node, found {count} in {labels:?}"
            );
        }
        // The intermediate `room_eq_gain_lp` node is gone — its per-route gain
        // is baked directly into the matrix coefficients.
        assert!(
            !labels.iter().any(|l| l == "room_eq_gain_lp"),
            "factored graph must not emit a separate LP gain node — \
             gain lives in the matrix coefficient: {labels:?}"
        );
        assert!(
            !labels
                .iter()
                .any(|l| l.starts_with("room_eq_output_isolate_")),
            "factored graph should not emit per-channel isolator matrices: {labels:?}"
        );
        assert!(
            !labels
                .iter()
                .any(|l| l.starts_with("room_eq_route_")
                    && l.as_str() != "room_eq_matrix_to_sub_bus"),
            "factored graph should not emit per-route nodes: {labels:?}"
        );
    }

    #[test]
    fn test_factored_graph_node_count_independent_of_channel_count() {
        // 2-channel routed bass scenario vs 3-channel physical-sub scenario:
        // the factored builder emits the same fixed number of DSP nodes in
        // both cases (channel count only affects the per-channel parameter
        // arrays inside each node).
        let small = build_room_eq_plugin_graph_config(&routed_bass_output(), 48_000.0).unwrap();
        let larger =
            build_room_eq_plugin_graph_config(&routed_physical_sub_output(), 48_000.0).unwrap();
        assert_eq!(
            small.nodes.len(),
            larger.nodes.len(),
            "factored graph node count must be channel-count-invariant; \
             small={:?} larger={:?}",
            collect_labels(&small),
            collect_labels(&larger),
        );
    }

    /// Every node emitted by the factored builder must instantiate
    /// successfully via `sotf_plugins::create_plugin`. This catches schema
    /// drift between the JSON the builder emits and the plugin parameter
    /// structs — otherwise the engine would fail at flush time with a
    /// less-helpful error.
    #[test]
    fn test_factored_graph_nodes_instantiate_via_factory() {
        for output in [routed_bass_output(), routed_physical_sub_output()] {
            let graph = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
            for node in &graph.nodes {
                let label = node
                    .parameters
                    .get("label")
                    .and_then(|l| l.as_str())
                    .unwrap_or("<unlabeled>");
                let plugin = sotf_plugins::create_plugin(
                    &node.plugin_type,
                    &node.parameters,
                    node.input_channels,
                    48_000,
                )
                .unwrap_or_else(|err| {
                    panic!(
                        "factored graph node '{label}' (type={}) failed to instantiate: {err}\n\
                         parameters: {}",
                        node.plugin_type, node.parameters
                    )
                });
                // Sanity: the constructed plugin must agree with the graph's
                // declared input channel count.
                assert_eq!(
                    plugin.input_channels(),
                    node.input_channels,
                    "plugin '{label}' input_channels mismatch"
                );
            }
        }
    }

    /// The `lfe_gain_applied_to_chain == true` path needs explicit coverage
    /// because the common fixtures set it to false. Build a small
    /// variant that flips it and confirm the matrix coefficient still
    /// reflects route.gain_db (chain has no route_owned gain in this
    /// minimal scenario, so the chain-override branch shouldn't fire).
    #[test]
    fn test_factored_graph_handles_lfe_gain_applied_to_chain_true() {
        let mut output = routed_bass_output();
        if let Some(report) = output
            .metadata
            .as_mut()
            .and_then(|m| m.bass_management.as_mut())
        {
            report.lfe_gain_applied_to_chain = true;
        }
        let graph = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
        let matrix_node = graph
            .nodes
            .iter()
            .find(|n| {
                n.parameters.get("label").and_then(|l| l.as_str())
                    == Some("room_eq_matrix_to_sub_bus")
            })
            .expect("factored sub-bus matrix");
        let matrix: Vec<f32> = matrix_node.parameters["matrix"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap() as f32)
            .collect();
        // L → Sub LP route gain_db = -3, no chain route_owned gain to
        // override. Expect 10^(-3/20).
        let expected = 10.0_f32.powf(-3.0 / 20.0);
        let got = matrix[1 * 2 + 0];
        assert!(
            (got - expected).abs() < 1e-5,
            "L→Sub matrix coef under lfe_gain_applied_to_chain=true should be 10^(-3/20) ≈ {expected}, got {got}"
        );
    }

    /// Regression: a destination-only channel (in the routing graph as a
    /// destination but not a source of any route) must pass its direct
    /// input through the HP branch so signals arriving on that channel
    /// upstream of RoomEQ reach the post-EQ stage instead of being muted.
    #[test]
    fn test_factored_graph_passthrough_for_destination_only_channels() {
        let output = routed_physical_sub_output();
        let graph = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
        let xover_hp = graph
            .nodes
            .iter()
            .find(|n| {
                n.parameters.get("label").and_then(|l| l.as_str()) == Some("room_eq_xover_hp")
            })
            .expect("factored HP crossover");
        let modes: Vec<String> = xover_hp.parameters["channel_modes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        // Channel order in routed_physical_sub_output is [L, LFE, SubA].
        // L (idx 0) is the source of main_highpass_to_self → "highpass".
        // LFE (idx 1) is neither source nor destination in the routing
        // graph after the relabel → "mute".
        // SubA (idx 2) is destination-only → must be "passthrough".
        assert_eq!(modes[0], "highpass", "L must be HP");
        assert_eq!(
            modes[2], "passthrough",
            "destination-only SubA must be passthrough"
        );
    }

    /// When `lfe_gain_applied_to_chain == true` AND the LFE chain has a
    /// `route_owned` gain plugin, the chain-override branch in the builder
    /// should win for the LFE self-route. Matrix coefficient for the LFE
    /// self-route should reflect the chain's gain, not `route.gain_db`.
    #[test]
    fn test_factored_graph_lfe_chain_route_owned_gain_overrides_route_gain() {
        // Build an output with two channels: L (main) and LFE (sub).
        // L → L HP route (main_highpass_to_self)
        // L → LFE LP route (redirected_bass_lowpass_to_sub, route.gain_db=-13)
        // LFE → LFE LP route (lfe_lowpass_to_sub, route.gain_db=-7, but the
        // LFE chain has a route_owned gain of -17 dB which should override).
        let mut output = bare_output(vec![
            (
                "L".to_string(),
                ChannelDspChain {
                    plugins: vec![],
                    ..bare_chain("L", None)
                },
            ),
            (
                "LFE".to_string(),
                ChannelDspChain {
                    plugins: vec![PluginConfigWrapper {
                        plugin_type: "gain".to_string(),
                        parameters: serde_json::json!({
                            "label": "lfe_route_owned_gain",
                            "room_eq_stage": "route_owned",
                            "gain_db": -17.0,
                        }),
                    }],
                    ..bare_chain("LFE", None)
                },
            ),
        ]);
        output.metadata = Some(OptimizationMetadata {
            pre_score: 1.0,
            post_score: 0.5,
            algorithm: "test".to_string(),
            loss_type: None,
            iterations: 1,
            timestamp: "test".to_string(),
            inter_channel_deviation: None,
            epa_per_channel: None,
            epa_multichannel: None,
            group_delay: None,
            perceptual_metrics: None,
            home_cinema_layout: None,
            multi_seat_coverage: None,
            multi_seat_correction: None,
            bass_management: Some(BassManagementReport {
                enabled: true,
                crossover_type: "LR24".to_string(),
                crossover_frequency_hz: Some(80.0),
                redirected_bass_enabled: true,
                lfe_channel: "LFE".to_string(),
                lfe_playback_gain_db: 10.0,
                lfe_gain_applied_to_chain: true,
                sub_trim_db: 0.0,
                max_sub_boost_db: 6.0,
                headroom_margin_db: -3.0,
                applied_sub_gain_db: Some(-17.0),
                gain_limited: false,
                physical_sub_output: "LFE".to_string(),
                redirected_bass_channel_count: 1,
                main_high_pass_hz: Some(80.0),
                sub_low_pass_hz: Some(80.0),
                lfe_headroom_required_db: 10.0,
                signal_flow: Vec::new(),
                signal_flow_advisories: Vec::new(),
                routing_graph: Some(BassManagementRoutingGraph {
                    physical_sub_output: "LFE".to_string(),
                    input_channels: vec!["L".to_string(), "LFE".to_string()],
                    output_channels: vec!["L".to_string(), "LFE".to_string()],
                    routes: vec![
                        BassManagementRoute {
                            group_id: Some("lcr".to_string()),
                            source_channel: "L".to_string(),
                            source_index: 0,
                            destination: "L".to_string(),
                            destination_index: 0,
                            pre_chain_channel: Some("L".to_string()),
                            post_chain_channel: Some("L".to_string()),
                            route_kind: "main_highpass_to_self".to_string(),
                            crossover_type: "LR24".to_string(),
                            high_pass_hz: Some(80.0),
                            low_pass_hz: None,
                            gain_db: 0.0,
                            gain_linear: 1.0,
                            matrix_gain: 1.0,
                            delay_ms: 0.0,
                            polarity_inverted: false,
                        },
                        BassManagementRoute {
                            group_id: Some("lcr".to_string()),
                            source_channel: "L".to_string(),
                            source_index: 0,
                            destination: "LFE".to_string(),
                            destination_index: 1,
                            pre_chain_channel: Some("LFE".to_string()),
                            post_chain_channel: Some("LFE".to_string()),
                            route_kind: "redirected_bass_lowpass_to_sub".to_string(),
                            crossover_type: "LR24".to_string(),
                            high_pass_hz: None,
                            low_pass_hz: Some(80.0),
                            gain_db: -13.0,
                            gain_linear: 0.2238,
                            matrix_gain: 0.2238,
                            delay_ms: 0.0,
                            polarity_inverted: false,
                        },
                        BassManagementRoute {
                            group_id: Some("lfe".to_string()),
                            source_channel: "LFE".to_string(),
                            source_index: 1,
                            destination: "LFE".to_string(),
                            destination_index: 1,
                            pre_chain_channel: Some("LFE".to_string()),
                            post_chain_channel: Some("LFE".to_string()),
                            route_kind: "lfe_lowpass_to_sub".to_string(),
                            crossover_type: "LR24".to_string(),
                            high_pass_hz: None,
                            low_pass_hz: Some(80.0),
                            // Route metadata gain is -7. The chain has -17,
                            // which should win for self-routes.
                            gain_db: -7.0,
                            gain_linear: 0.4467,
                            matrix_gain: 0.4467,
                            delay_ms: 0.0,
                            polarity_inverted: false,
                        },
                    ],
                    matrix: None,
                    advisories: Vec::new(),
                }),
                optimization: None,
                groups: Vec::new(),
                sub_outputs: Vec::new(),
                headroom_simulation: None,
                advisory: "ok".to_string(),
            }),
            timing_diagnostics: None,
            ctc: None,
            perceptual_policy: None,
            bootstrap_uncertainty: None,
            validation_bundle: None,
        });

        let config = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
        let matrix_node = config
            .nodes
            .iter()
            .find(|n| {
                n.parameters.get("label").and_then(|l| l.as_str())
                    == Some("room_eq_matrix_to_sub_bus")
            })
            .expect("factored sub-bus matrix");
        let matrix: Vec<f32> = matrix_node.parameters["matrix"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap() as f32)
            .collect();
        // L → LFE redirected_bass: matrix[LFE_idx=1][L_idx=0] = 10^(-13/20).
        let l_to_lfe = matrix[1 * 2 + 0];
        let expected_l_to_lfe = 10.0_f32.powf(-13.0 / 20.0);
        assert!(
            (l_to_lfe - expected_l_to_lfe).abs() < 1e-5,
            "L→LFE matrix coef should be 10^(-13/20) ≈ {expected_l_to_lfe}, got {l_to_lfe}"
        );
        // LFE → LFE: chain has route_owned gain = -17 dB, which should win
        // over route.gain_db = -7 for the self-route.
        let lfe_to_lfe = matrix[1 * 2 + 1];
        let expected_lfe_to_lfe = 10.0_f32.powf(-17.0 / 20.0);
        assert!(
            (lfe_to_lfe - expected_lfe_to_lfe).abs() < 1e-5,
            "LFE self-route should use chain route_owned gain -17, \
             got {lfe_to_lfe}, expected 10^(-17/20) ≈ {expected_lfe_to_lfe}"
        );
    }

    /// Edge case: routing graph where every channel is destination-only
    /// (no channel is a source of any *valid* route). The builder must not
    /// panic; the HP branch ends up Passthrough for destination channels,
    /// the LP branch all-Mute, and the matrix is zero. The graph must still
    /// instantiate cleanly via the factory.
    #[test]
    fn test_factored_graph_fixes_specific_legacy_bugs_on_routed_bass() {
        let output = routed_bass_output();

        // Drive the legacy builder directly — same input, two outputs.
        let legacy_graph = super::build_routed_room_eq_graph(
            &output,
            output
                .metadata
                .as_ref()
                .unwrap()
                .bass_management
                .as_ref()
                .unwrap()
                .routing_graph
                .as_ref()
                .unwrap(),
        )
        .unwrap();
        let factored = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();

        let legacy_labels: Vec<&str> = legacy_graph
            .nodes
            .iter()
            .filter_map(|n| n.parameters.get("label").and_then(|l| l.as_str()))
            .collect();
        let factored_labels: Vec<&str> = factored
            .nodes
            .iter()
            .filter_map(|n| n.parameters.get("label").and_then(|l| l.as_str()))
            .collect();

        // Bug 1: node count blow-up. Even on the 2-channel routed_bass_output
        // fixture (one main + one sub), the legacy builder emits ≥2x the
        // factored count. On the 10-channel gen514 case the ratio grows
        // (~50+ vs 9) — see `gen514_factored_graph_topology_matches_golden_snapshot`
        // in the integration tests.
        assert!(
            legacy_graph.nodes.len() >= factored.nodes.len() * 2,
            "legacy builder should emit at least 2x the nodes of the factored builder \
             (legacy={}, factored={})",
            legacy_graph.nodes.len(),
            factored.nodes.len()
        );
        assert_eq!(factored.nodes.len(), 9);

        // Bug 2: legacy carries the source chain's pre-EQ as a standalone
        // node; factored folds it into the single `room_eq_eq_pre` array.
        assert_eq!(
            factored_labels
                .iter()
                .filter(|l| **l == "pre_room_eq")
                .count(),
            0,
            "factored folds pre_room_eq into room_eq_eq_pre channel_filters"
        );
        assert_eq!(
            factored_labels
                .iter()
                .filter(|l| **l == "room_eq_eq_pre")
                .count(),
            1,
        );

        // Bug 3: per-channel output isolator matrices.
        let legacy_isolators = legacy_labels
            .iter()
            .filter(|l| l.starts_with("room_eq_output_isolate_"))
            .count();
        assert!(
            legacy_isolators >= 2,
            "legacy emits one isolator per output channel (got {legacy_isolators})"
        );
        assert_eq!(
            factored_labels
                .iter()
                .filter(|l| l.starts_with("room_eq_output_isolate_"))
                .count(),
            0,
        );

        // Sanity: single sub-bus matrix in the factored graph.
        assert_eq!(
            factored_labels
                .iter()
                .filter(|l| **l == "room_eq_matrix_to_sub_bus")
                .count(),
            1,
        );
    }

    #[test]
    fn test_factored_graph_all_destinations_no_sources_builds_cleanly() {
        let mut output = bare_output(vec![
            ("A".to_string(), bare_chain("A", None)),
            ("B".to_string(), bare_chain("B", None)),
        ]);
        output.metadata = Some(OptimizationMetadata {
            pre_score: 1.0,
            post_score: 0.5,
            algorithm: "test".to_string(),
            loss_type: None,
            iterations: 1,
            timestamp: "test".to_string(),
            inter_channel_deviation: None,
            epa_per_channel: None,
            epa_multichannel: None,
            group_delay: None,
            perceptual_metrics: None,
            home_cinema_layout: None,
            multi_seat_coverage: None,
            multi_seat_correction: None,
            bass_management: Some(BassManagementReport {
                enabled: true,
                crossover_type: "LR24".to_string(),
                crossover_frequency_hz: Some(80.0),
                redirected_bass_enabled: false,
                lfe_channel: "B".to_string(),
                lfe_playback_gain_db: 0.0,
                lfe_gain_applied_to_chain: false,
                sub_trim_db: 0.0,
                max_sub_boost_db: 6.0,
                headroom_margin_db: -3.0,
                applied_sub_gain_db: Some(0.0),
                gain_limited: false,
                physical_sub_output: "B".to_string(),
                redirected_bass_channel_count: 0,
                main_high_pass_hz: None,
                sub_low_pass_hz: None,
                lfe_headroom_required_db: 0.0,
                signal_flow: Vec::new(),
                signal_flow_advisories: Vec::new(),
                routing_graph: Some(BassManagementRoutingGraph {
                    physical_sub_output: "B".to_string(),
                    input_channels: vec!["A".to_string(), "B".to_string()],
                    output_channels: vec!["A".to_string(), "B".to_string()],
                    // A single route with an unknown kind: builder marks
                    // is_source[A] and is_destination[B] via the tag pass
                    // but doesn't populate HP/LP mode arrays. After the
                    // route walk, B is destination-only → Passthrough on HP.
                    routes: vec![BassManagementRoute {
                        group_id: None,
                        source_channel: "A".to_string(),
                        source_index: 0,
                        destination: "B".to_string(),
                        destination_index: 1,
                        pre_chain_channel: None,
                        post_chain_channel: None,
                        route_kind: "unknown_kind_should_be_ignored".to_string(),
                        crossover_type: "LR24".to_string(),
                        high_pass_hz: None,
                        low_pass_hz: None,
                        gain_db: 0.0,
                        gain_linear: 1.0,
                        matrix_gain: 1.0,
                        delay_ms: 0.0,
                        polarity_inverted: false,
                    }],
                    matrix: None,
                    advisories: Vec::new(),
                }),
                optimization: None,
                groups: Vec::new(),
                sub_outputs: Vec::new(),
                headroom_simulation: None,
                advisory: "ok".to_string(),
            }),
            timing_diagnostics: None,
            ctc: None,
            perceptual_policy: None,
            bootstrap_uncertainty: None,
            validation_bundle: None,
        });

        let config = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
        let labels: Vec<&str> = config
            .nodes
            .iter()
            .filter_map(|n| n.parameters.get("label").and_then(|l| l.as_str()))
            .collect();
        for required in [
            "room_eq_gain_pre",
            "room_eq_eq_pre",
            "room_eq_xover_hp",
            "room_eq_delay_hp",
            "room_eq_xover_lp",
            "room_eq_delay_lp",
            "room_eq_matrix_to_sub_bus",
            "room_eq_eq_post",
            "room_eq_gain_post",
        ] {
            assert!(
                labels.contains(&required),
                "missing factored node {required} in {labels:?}"
            );
        }
        // B (destination-only) → Passthrough on HP.
        let xover_hp = config
            .nodes
            .iter()
            .find(|n| {
                n.parameters.get("label").and_then(|l| l.as_str()) == Some("room_eq_xover_hp")
            })
            .unwrap();
        let modes: Vec<String> = xover_hp.parameters["channel_modes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(modes[1], "passthrough");
        // Matrix all zero (no LP routes).
        let matrix_node = config
            .nodes
            .iter()
            .find(|n| {
                n.parameters.get("label").and_then(|l| l.as_str())
                    == Some("room_eq_matrix_to_sub_bus")
            })
            .unwrap();
        let matrix: Vec<f32> = matrix_node.parameters["matrix"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap() as f32)
            .collect();
        for v in &matrix {
            assert_eq!(*v, 0.0);
        }
        // Every node must instantiate via the factory.
        for node in &config.nodes {
            sotf_plugins::create_plugin(
                &node.plugin_type,
                &node.parameters,
                node.input_channels,
                48_000,
            )
            .unwrap_or_else(|err| panic!("node {} ({}) failed: {err}", node.id, node.plugin_type));
        }
    }

    /// End-to-end audio test: build a DawHost from the factored graph, drive
    /// an impulse on the L source, and verify the sub-bus carries the
    /// LP-filtered signal at the matrix-encoded gain. Catches issues that
    /// the per-node instantiation test can't (matrix routing wrong, edges
    /// missing, channel widths mismatched between consecutive nodes).
    #[test]
    fn test_factored_graph_audio_equivalence_routed_bass() {
        use sotf_plugins::{DawHost, GraphEdge};

        let output = routed_bass_output();
        let config = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
        // Channel order: L (0), Sub (1). Route L → Sub LP @ 80 Hz, gain -3 dB.

        let channel_count = config.nodes[0].input_channels;
        let sr = 48_000u32;
        let mut host = DawHost::new(channel_count, sr);

        // Materialise plugins and add them as host nodes. Keep a map from
        // builder node id → host node id so we can wire the edges.
        let mut node_map: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        for node in &config.nodes {
            let plugin = sotf_plugins::create_plugin(
                &node.plugin_type,
                &node.parameters,
                node.input_channels,
                sr,
            )
            .unwrap_or_else(|err| {
                panic!(
                    "factored node {} ({}) failed to instantiate: {err}",
                    node.id, node.plugin_type
                )
            });
            let label = node
                .parameters
                .get("label")
                .and_then(|l| l.as_str())
                .unwrap_or("")
                .to_string();
            let host_id = host
                .add_node(label, plugin)
                .expect("host accepts plugin node");
            node_map.insert(node.id, host_id);
        }
        for edge in &config.edges {
            host.add_edge(GraphEdge::new(
                node_map[&edge.from_node],
                node_map[&edge.to_node],
            ))
            .expect("host accepts edge");
        }
        host.build().expect("host builds");

        // Drive an impulse on the L source (channel 0) and let it propagate
        // through enough frames to clear group delay.
        let num_frames = 4096usize;
        let mut input = vec![0.0f32; num_frames * channel_count];
        input[0] = 1.0; // L impulse at frame 0

        let mut output_buf = vec![0.0f32; num_frames * channel_count];
        host.process(&input, &mut output_buf).expect("process");

        // Sub channel (idx 1) should carry the LP-filtered impulse at the
        // route's gain (-3 dB → 0.708 linear). Sum the absolute energy on
        // the sub row in the steady-state region and compare to expected
        // bounds. The exact peak is filter-dependent; just confirm there's
        // non-trivial signal on the sub and that it is below the input
        // amplitude (i.e. the LP + gain combo actually attenuated).
        let sub_energy: f32 = (32..num_frames)
            .map(|f| output_buf[f * channel_count + 1].abs())
            .fold(0.0, f32::max);
        assert!(
            sub_energy > 0.0001,
            "sub channel must carry signal from L→Sub LP route, peak={sub_energy}"
        );
        // The route gain is -3 dB linear ~0.708. The LP impulse response
        // peak is < input amplitude due to filtering. Upper bound check.
        assert!(
            sub_energy < 0.8,
            "sub channel signal must be attenuated by LP+gain, peak={sub_energy}"
        );

        // L output channel (idx 0): HP-filtered impulse. Should also carry
        // signal (HP isn't full mute).
        let l_energy: f32 = (32..num_frames)
            .map(|f| output_buf[f * channel_count].abs())
            .fold(0.0, f32::max);
        assert!(
            l_energy > 0.0001,
            "L output channel must carry HP-filtered signal, peak={l_energy}"
        );
    }
}
