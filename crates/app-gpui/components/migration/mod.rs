//! Shared migration utilities for converting legacy measurement files to new RoomConfig format.
//!
//! This module provides functions for:
//! - Detecting if a JSON file needs migration (legacy format with large inline data)
//! - Converting legacy RoomEqMeasurementsFile to autoeq::RoomConfig format
//! - Writing CSV files with measurement data
//!
//! Used by both the Recording and Room EQ components.

use crate::app::types::RoomEqMeasurementsFile;
use std::path::Path;

/// Check if a JSON file needs migration (legacy format with large inline data)
///
/// Returns true if:
/// - File is larger than 1MB
/// - Contains "channels" array with inline frequency data (>100 points)
pub fn check_needs_migration(json: &str, file_size: u64) -> bool {
    // If file is small (<1MB), don't bother with migration
    if file_size < 1_000_000 {
        return false;
    }

    // Check if the JSON contains large inline frequency data
    // Look for "frequencies" or "magnitude_db" arrays with data
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
        // Check channels array for inline data
        if let Some(channels) = value.get("channels").and_then(|c| c.as_array()) {
            for channel in channels {
                // Check measurement.frequencies array
                if let Some(measurement) = channel.get("measurement") {
                    if let Some(freqs) = measurement.get("frequencies").and_then(|f| f.as_array()) {
                        if freqs.len() > 100 {
                            return true;
                        }
                    }
                }
                // Check result.frequencies for older format
                if let Some(result) = channel.get("result") {
                    if let Some(freqs) = result.get("frequencies").and_then(|f| f.as_array()) {
                        if freqs.len() > 100 {
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
}

/// Result of a migration operation
#[derive(Debug)]
pub struct MigrationResult {
    /// Number of channels successfully migrated
    pub channel_count: usize,
    /// Path to the backup file
    pub backup_path: std::path::PathBuf,
    /// Paths to the generated CSV files
    pub csv_paths: Vec<std::path::PathBuf>,
}

/// Perform migration from legacy RoomEqMeasurementsFile format to new RoomConfig format.
///
/// This will:
/// 1. Back up the original JSON file with a `.bak` extension
/// 2. Write CSV files with measurement data for each channel
/// 3. Write a new RoomConfig JSON file to the original location
///
/// Returns the number of channels migrated on success.
pub fn perform_migration(
    json: &str,
    original_path: &Path,
    session_dir: &Path,
) -> Result<MigrationResult, String> {
    // Parse the legacy format
    let measurements_file = RoomEqMeasurementsFile::from_json_str(json)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;

    // Step 1: Back up the original JSON file
    let backup_path = {
        let mut backup = original_path.to_path_buf();
        let extension = backup
            .extension()
            .map(|e| format!("{}.bak", e.to_string_lossy()))
            .unwrap_or_else(|| "bak".to_string());
        backup.set_extension(extension);
        backup
    };

    std::fs::copy(original_path, &backup_path)
        .map_err(|e| format!("Failed to back up original file: {}", e))?;

    log::info!("Backed up original JSON to {:?}", backup_path);

    // Step 2: Write CSV files for each channel
    let mut csv_paths = Vec::new();

    for channel in &measurements_file.channels {
        let safe_channel_name = sanitize_filename(&channel.channel_name);
        let csv_filename = format!("{}.csv", safe_channel_name);
        let csv_path = session_dir.join(&csv_filename);

        let result = &channel.measurement;
        write_migration_csv(result, &csv_path)?;

        log::info!(
            "Wrote migrated CSV ({} points): {:?}",
            result.frequencies.len(),
            csv_path
        );
        csv_paths.push(csv_path);
    }

    // Step 3: Write new RoomConfig JSON to the original file location
    write_room_config(&measurements_file, original_path)?;

    log::info!("Wrote new RoomConfig JSON to {:?}", original_path);

    Ok(MigrationResult {
        channel_count: measurements_file.channels.len(),
        backup_path,
        csv_paths,
    })
}

/// Convert legacy RoomEqMeasurementsFile to new RoomConfig format and write to file
pub fn write_room_config(
    measurements: &RoomEqMeasurementsFile,
    path: &Path,
) -> Result<(), String> {
    use autoeq::{
        InlineMeasurement, MeasurementRef, MeasurementSource, OptimizerConfig,
        RecordingConfiguration, RoomConfig, SpeakerConfig,
    };
    use std::collections::HashMap;

    // Convert channels to speakers HashMap with inline measurements
    let mut speakers: HashMap<String, SpeakerConfig> = HashMap::new();

    for ch in measurements.channels.iter() {
        let safe_channel_name = sanitize_filename(&ch.channel_name);
        let result = &ch.measurement;

        // Create inline measurement with frequency response data
        let inline_measurement = InlineMeasurement {
            frequencies: result.frequencies.iter().map(|&f| f as f64).collect(),
            magnitude_db: result.magnitude_db.iter().map(|&m| m as f64).collect(),
            phase_deg: Some(result.phase_deg.iter().map(|&p| p as f64).collect()),
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
    let recording_config = measurements.configuration.as_ref().map(|cfg| {
        RecordingConfiguration {
            playback_device_name: Some(cfg.playback_device_name.clone()),
            playback_device_id: Some(cfg.playback_device_id.clone()),
            playback_sample_rate: Some(cfg.playback_sample_rate),
            playback_channels: Some(cfg.playback_channels),
            speaker_configuration: Some(cfg.speaker_configuration.clone()),
            channel_names: Some(cfg.channel_names.clone()),
            recording_device_name: Some(cfg.recording_device_name.clone()),
            recording_device_id: Some(cfg.recording_device_id.clone()),
            recording_sample_rate: Some(cfg.recording_sample_rate),
            recording_channels: Some(cfg.recording_channels),
            mic_calibration_path: cfg.mic_calibration_path.clone(),
            recording_directory: cfg.recording_directory.clone(),
            signal_type: Some(cfg.signal_type.clone()),
            signal_duration_secs: Some(cfg.signal_duration_secs),
            signal_level_db: Some(cfg.signal_level_db),
            // Sweep parameters for recomputing metrics from WAV
            sweep_start_freq: cfg.sweep_start_freq,
            sweep_end_freq: cfg.sweep_end_freq,
        }
    });

    // Build RoomConfig
    let room_config = RoomConfig {
        version: "1.1.0".to_string(),
        system: None,
        speakers,
        crossovers: None,
        target_curve: None,
        group_delay: None,
        optimizer: OptimizerConfig::default(),
        recording_config,
    };

    let file =
        std::fs::File::create(path).map_err(|e| format!("Failed to create file: {}", e))?;
    serde_json::to_writer_pretty(file, &room_config)
        .map_err(|e| format!("Failed to serialize: {}", e))?;

    Ok(())
}

/// Write measurement data to CSV with extended format
pub fn write_migration_csv(
    result: &crate::app::types::RecordingResult,
    csv_path: &Path,
) -> Result<(), String> {
    use std::io::Write;

    let mut file =
        std::fs::File::create(csv_path).map_err(|e| format!("Failed to create CSV: {}", e))?;

    // Header
    writeln!(
        file,
        "frequency_hz,spl_db,phase_deg,thd_percent,rt60_ms,c50_db,c80_db,group_delay_ms"
    )
    .map_err(|e| format!("Failed to write header: {}", e))?;

    // Data
    for i in 0..result.frequencies.len() {
        let freq = result.frequencies[i];
        let spl = result.magnitude_db.get(i).copied().unwrap_or(0.0);
        let phase = result.phase_deg.get(i).copied().unwrap_or(0.0);

        // Get optional extended data
        let thd = result
            .thd_percent
            .as_ref()
            .and_then(|v| v.get(i))
            .copied()
            .unwrap_or(0.0);
        let rt60 = result
            .rt60_ms
            .as_ref()
            .and_then(|v| v.get(i))
            .copied()
            .unwrap_or(0.0);
        let c50 = result
            .clarity_c50_db
            .as_ref()
            .and_then(|v| v.get(i))
            .copied()
            .unwrap_or(0.0);
        let c80 = result
            .clarity_c80_db
            .as_ref()
            .and_then(|v| v.get(i))
            .copied()
            .unwrap_or(0.0);
        let gd = result
            .excess_group_delay_ms
            .as_ref()
            .and_then(|v| v.get(i))
            .copied()
            .unwrap_or(0.0);

        writeln!(
            file,
            "{:.2},{:.4},{:.4},{:.6},{:.4},{:.4},{:.4},{:.6}",
            freq, spl, phase, thd, rt60, c50, c80, gd
        )
        .map_err(|e| format!("Failed to write data: {}", e))?;
    }

    Ok(())
}

/// Sanitize a string for use as a filename
pub fn sanitize_filename(name: &str) -> String {
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

/// Extended metrics that can be read from CSV or computed from WAV
#[derive(Debug, Default)]
pub struct ExtendedMetrics {
    pub thd_percent: Option<Vec<f32>>,
    pub rt60_ms: Option<Vec<f32>>,
    pub clarity_c50_db: Option<Vec<f32>>,
    pub clarity_c80_db: Option<Vec<f32>>,
    pub excess_group_delay_ms: Option<Vec<f32>>,
}

/// Read extended metrics from CSV file
///
/// CSV format expected:
/// frequency_hz,spl_db,phase_deg,thd_percent,rt60_ms,c50_db,c80_db,group_delay_ms
pub fn read_extended_metrics_from_csv(csv_path: &Path) -> Result<ExtendedMetrics, String> {
    use std::io::BufRead;

    let file = std::fs::File::open(csv_path)
        .map_err(|e| format!("Failed to open CSV: {}", e))?;
    let reader = std::io::BufReader::new(file);

    let mut thd_percent = Vec::new();
    let mut rt60_ms = Vec::new();
    let mut clarity_c50_db = Vec::new();
    let mut clarity_c80_db = Vec::new();
    let mut excess_group_delay_ms = Vec::new();

    let mut lines = reader.lines();

    // Skip header
    let header = lines.next()
        .ok_or("CSV file is empty")?
        .map_err(|e| format!("Failed to read header: {}", e))?;

    // Check if this is an extended CSV (has more than 3 columns)
    let has_extended_data = header.contains("thd_percent") || header.contains("c50_db");
    if !has_extended_data {
        return Err("CSV does not contain extended metrics".to_string());
    }

    for line_result in lines {
        let line = line_result.map_err(|e| format!("Failed to read line: {}", e))?;
        let parts: Vec<&str> = line.split(',').collect();

        // Expected columns: freq, spl, phase, thd, rt60, c50, c80, gd
        if parts.len() >= 8 {
            thd_percent.push(parts[3].parse().unwrap_or(0.0));
            rt60_ms.push(parts[4].parse().unwrap_or(0.0));
            clarity_c50_db.push(parts[5].parse().unwrap_or(0.0));
            clarity_c80_db.push(parts[6].parse().unwrap_or(0.0));
            excess_group_delay_ms.push(parts[7].parse().unwrap_or(0.0));
        }
    }

    if thd_percent.is_empty() {
        return Err("No data rows found in CSV".to_string());
    }

    Ok(ExtendedMetrics {
        thd_percent: Some(thd_percent),
        rt60_ms: Some(rt60_ms),
        clarity_c50_db: Some(clarity_c50_db),
        clarity_c80_db: Some(clarity_c80_db),
        excess_group_delay_ms: Some(excess_group_delay_ms),
    })
}

/// Try to load extended metrics for a measurement
///
/// Attempts to read from CSV file first, as that contains pre-computed metrics
/// from the original recording session.
pub fn load_extended_metrics(
    csv_path: Option<&str>,
    base_dir: Option<&Path>,
) -> Option<ExtendedMetrics> {
    let csv_path = csv_path?;

    // Try absolute path first
    let abs_path = Path::new(csv_path);
    if abs_path.exists() {
        if let Ok(metrics) = read_extended_metrics_from_csv(abs_path) {
            log::info!("Loaded extended metrics from {:?}", abs_path);
            return Some(metrics);
        }
    }

    // Try relative to base_dir
    if let Some(base) = base_dir {
        let rel_path = base.join(csv_path);
        if rel_path.exists() {
            if let Ok(metrics) = read_extended_metrics_from_csv(&rel_path) {
                log::info!("Loaded extended metrics from {:?}", rel_path);
                return Some(metrics);
            }
        }
    }

    log::debug!("No extended metrics found for CSV: {}", csv_path);
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_needs_migration_small_file() {
        let json = r#"{"channels": []}"#;
        assert!(!check_needs_migration(json, 100)); // Small file
    }

    #[test]
    fn test_check_needs_migration_large_file_no_data() {
        let json = r#"{"channels": []}"#;
        assert!(!check_needs_migration(json, 2_000_000)); // Large but no inline data
    }

    #[test]
    fn test_check_needs_migration_large_file_with_data() {
        // Create JSON with >100 frequency points
        let frequencies: Vec<f32> = (0..200).map(|i| i as f32 * 100.0).collect();
        let json = format!(
            r#"{{"channels": [{{"measurement": {{"frequencies": {:?}}}}}]}}"#,
            frequencies
        );
        assert!(check_needs_migration(&json, 2_000_000)); // Large with inline data
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("L"), "L");
        assert_eq!(sanitize_filename("Front Left"), "Front_Left");
        assert_eq!(sanitize_filename("Ch/1"), "Ch_1");
        assert_eq!(sanitize_filename("test-123_abc"), "test-123_abc");
    }
}
