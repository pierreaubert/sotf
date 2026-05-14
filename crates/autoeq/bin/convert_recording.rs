//! Utility to convert legacy recording.json files to the new RoomConfig format.
//!
//! Usage:
//!   convert_recording <input.json> [output.json]
//!
//! If output is not specified, the input file is overwritten and a .bak backup is created.

use autoeq::{
    DspChainOutput, InlineMeasurement, MeasurementRef, MeasurementSource, OptimizerConfig,
    RecordingConfiguration, RoomConfig, SpeakerConfig,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Legacy format: RoomEqMeasurementsFile (from app-gpui)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyMeasurementsFile {
    #[serde(default)]
    version: Option<u32>,
    channels: Vec<LegacyChannelMeasurement>,
    configuration: Option<LegacyRecordingConfiguration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyChannelMeasurement {
    channel_name: String,
    measurement: LegacyRecordingResult,
    #[serde(default)]
    is_group: bool,
    #[serde(default)]
    group_drivers: Vec<LegacyRecordingResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyRecordingResult {
    channel: usize,
    wav_path: Option<String>,
    csv_path: Option<String>,
    frequencies: Vec<f32>,
    magnitude_db: Vec<f32>,
    phase_deg: Vec<f32>,
    #[serde(default)]
    impulse_response: Option<Vec<f32>>,
    #[serde(default)]
    impulse_time_ms: Option<Vec<f32>>,
    #[serde(default)]
    excess_group_delay_ms: Option<Vec<f32>>,
    #[serde(default)]
    thd_percent: Option<Vec<f32>>,
    #[serde(default)]
    harmonic_distortion_db: Option<Vec<Vec<f32>>>,
    #[serde(default)]
    rt60_ms: Option<Vec<f32>>,
    #[serde(default)]
    clarity_c50_db: Option<Vec<f32>>,
    #[serde(default)]
    clarity_c80_db: Option<Vec<f32>>,
    #[serde(default)]
    spectrogram_db: Option<Vec<Vec<f32>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyRecordingConfiguration {
    playback_device_name: String,
    playback_device_id: String,
    playback_sample_rate: u32,
    playback_channels: u32,
    speaker_configuration: String,
    channel_names: Vec<String>,
    recording_device_name: String,
    recording_device_id: String,
    recording_sample_rate: u32,
    recording_channels: u32,
    mic_calibration_path: Option<String>,
    recording_directory: Option<String>,
    signal_type: String,
    signal_duration_secs: f32,
    signal_level_db: f32,
    /// Sweep start frequency in Hz (only applicable when signal_type is "Sweep")
    #[serde(default)]
    sweep_start_freq: Option<f32>,
    /// Sweep end frequency in Hz (only applicable when signal_type is "Sweep")
    #[serde(default)]
    sweep_end_freq: Option<f32>,
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn convert_legacy_to_room_config(legacy: &LegacyMeasurementsFile) -> RoomConfig {
    let mut speakers: HashMap<String, SpeakerConfig> = HashMap::new();

    for ch in &legacy.channels {
        let safe_channel_name = sanitize_filename(&ch.channel_name);
        let result = &ch.measurement;

        // Store only file references, not inline data
        let inline_measurement = InlineMeasurement {
            frequencies: Vec::new(),
            magnitude_db: Vec::new(),
            phase_deg: None,
            name: Some(ch.channel_name.clone()),
            wav_path: result.wav_path.clone(),
            csv_path: Some(format!("{}.csv", safe_channel_name)),
        };

        let measurement_ref = MeasurementRef::Inline(inline_measurement);
        let measurement_source = MeasurementSource::Single(autoeq::read::MeasurementSingle {
            measurement: measurement_ref,
            speaker_name: None,
        });
        let speaker_config = SpeakerConfig::Single(measurement_source);

        speakers.insert(ch.channel_name.clone(), speaker_config);
    }

    // Convert recording configuration if present
    let recording_config = legacy
        .configuration
        .as_ref()
        .map(|cfg| RecordingConfiguration {
            playback_device_name: Some(cfg.playback_device_name.clone()),
            playback_device_id: Some(cfg.playback_device_id.clone()),
            playback_sample_rate: Some(cfg.playback_sample_rate),
            playback_channels: Some(cfg.playback_channels as usize),
            speaker_configuration: Some(cfg.speaker_configuration.clone()),
            channel_names: Some(cfg.channel_names.clone()),
            recording_device_name: Some(cfg.recording_device_name.clone()),
            recording_device_id: Some(cfg.recording_device_id.clone()),
            recording_sample_rate: Some(cfg.recording_sample_rate),
            recording_channels: Some(cfg.recording_channels as usize),
            mic_calibration_path: cfg.mic_calibration_path.clone(),
            mic_calibration_paths: None,
            recording_directory: cfg.recording_directory.clone(),
            signal_type: Some(cfg.signal_type.clone()),
            signal_duration_secs: Some(cfg.signal_duration_secs),
            signal_level_db: Some(cfg.signal_level_db),
            // Sweep parameters for recomputing metrics from WAV
            sweep_start_freq: cfg.sweep_start_freq,
            sweep_end_freq: cfg.sweep_end_freq,
            // Legacy files predate room-info metadata; leave empty so
            // the new fields round-trip as absent.
            ..Default::default()
        });

    RoomConfig {
        version: autoeq::roomeq::default_config_version(),
        system: None,
        speakers,
        crossovers: None,
        target_curve: None,
        optimizer: OptimizerConfig::default(),
        recording_config,
        ctc: None,
        cea2034_cache: None,
    }
}

fn is_legacy_recording_format(json: &str) -> bool {
    // Check if JSON has "channels" array (legacy) vs "speakers" object (new)
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
        // New format has "speakers" as object, legacy has "channels" as array
        if value.get("speakers").is_some() {
            return false; // Already new format
        }
        if value.get("channels").is_some() {
            return true; // Legacy format
        }
    }
    false
}

fn backup_path_for(input_path: &std::path::Path) -> PathBuf {
    let mut backup = input_path.to_path_buf();
    let extension = backup
        .extension()
        .map(|e| format!("{}.bak", e.to_string_lossy()))
        .unwrap_or_else(|| "bak".to_string());
    backup.set_extension(extension);
    backup
}

fn write_output(
    input_path: &std::path::Path,
    output_path: &std::path::Path,
    output_json: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if output_path == input_path {
        let backup_path = backup_path_for(input_path);
        std::fs::copy(input_path, &backup_path)?;
        println!("Backup: {}", backup_path.display());
    }

    std::fs::write(output_path, output_json)?;
    println!(
        "Output: {} ({:.2} KB)",
        output_path.display(),
        output_json.len() as f64 / 1024.0
    );
    Ok(())
}

fn strip_deprecated_top_level_keys(value: &mut serde_json::Value) -> Vec<&'static str> {
    let deprecated_keys = ["group_delay"];
    let mut stripped = Vec::new();

    if let Some(obj) = value.as_object_mut() {
        for key in deprecated_keys {
            if obj.remove(key).is_some() {
                stripped.push(key);
            }
        }
    }

    stripped
}

fn normalize_room_config_for_latest_schema(config: &mut RoomConfig) {
    config.version = autoeq::roomeq::default_config_version();

    if config.optimizer.loss_type.eq_ignore_ascii_case("epa")
        && config.optimizer.epa_config.is_none()
    {
        config.optimizer.epa_config = Some(autoeq::loss::epa::score::EpaConfig::default());
    }
}

fn parse_room_config_with_cleanup(json: &str) -> Result<(RoomConfig, Vec<&'static str>), String> {
    let mut value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let stripped = strip_deprecated_top_level_keys(&mut value);
    let cleaned_json =
        serde_json::to_string(&value).map_err(|e| format!("failed to encode cleaned JSON: {e}"))?;
    let mut config: RoomConfig = serde_json::from_str(&cleaned_json).map_err(|e| format!("{e}"))?;
    normalize_room_config_for_latest_schema(&mut config);
    Ok((config, stripped))
}

fn parse_dsp_chain_output_with_latest_version(json: &str) -> Result<DspChainOutput, String> {
    let mut output: DspChainOutput = serde_json::from_str(json).map_err(|e| format!("{e}"))?;
    output.version = autoeq::roomeq::default_config_version();
    Ok(output)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <input.json> [output.json]", args[0]);
        eprintln!();
        eprintln!("Converts legacy recording.json files to the new RoomConfig format.");
        eprintln!();
        eprintln!("If output is not specified, the input file is overwritten");
        eprintln!("and a .bak backup is created.");
        std::process::exit(1);
    }

    let input_path = PathBuf::from(&args[1]);
    let output_path = if args.len() > 2 {
        PathBuf::from(&args[2])
    } else {
        input_path.clone()
    };

    // Read input file
    let json = std::fs::read_to_string(&input_path)?;
    let file_size = std::fs::metadata(&input_path)?.len();

    println!(
        "Input: {} ({:.2} KB)",
        input_path.display(),
        file_size as f64 / 1024.0
    );

    // Check if already new input or output format. These are still rewritten so
    // their schema version is upgraded to the latest canonical version.
    if !is_legacy_recording_format(&json) {
        match parse_room_config_with_cleanup(&json) {
            Ok((config, stripped_keys)) => {
                for key in stripped_keys {
                    println!("Stripped deprecated key: {}", key);
                }
                println!(
                    "File is in RoomConfig format; rewriting as version {}",
                    config.version
                );
                println!("  {} speaker(s)", config.speakers.len());
                let output_json = serde_json::to_string_pretty(&config)?;
                write_output(&input_path, &output_path, &output_json)?;
                return Ok(());
            }
            Err(room_err) => match parse_dsp_chain_output_with_latest_version(&json) {
                Ok(output) => {
                    println!(
                        "File is in DspChainOutput format; rewriting as version {}",
                        output.version
                    );
                    println!("  {} channel(s)", output.channels.len());
                    let output_json = serde_json::to_string_pretty(&output)?;
                    write_output(&input_path, &output_path, &output_json)?;
                    return Ok(());
                }
                Err(output_err) => {
                    eprintln!("Error: File doesn't appear to be a valid recording format");
                    eprintln!("  RoomConfig parse error: {room_err}");
                    eprintln!("  DspChainOutput parse error: {output_err}");
                    std::process::exit(1);
                }
            },
        }
    }

    // Parse legacy format
    let legacy: LegacyMeasurementsFile = serde_json::from_str(&json)?;
    println!("Detected legacy format (version {:?})", legacy.version);
    println!("  {} channel(s)", legacy.channels.len());

    // Convert to new format
    let room_config = convert_legacy_to_room_config(&legacy);

    // Write output
    let output_json = serde_json::to_string_pretty(&room_config)?;
    write_output(&input_path, &output_path, &output_json)?;
    println!("  {} speaker(s)", room_config.speakers.len());
    println!("Conversion complete!");

    Ok(())
}
