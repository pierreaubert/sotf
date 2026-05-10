//! CSV utility helpers (filename sanitization + extended-metrics loader)
//! used by the recording / room-EQ load paths. The historical
//! `perform_migration` / `write_room_config` functions that converted
//! the legacy `RoomEqMeasurementsFile` JSON into the autoeq RoomConfig
//! format have been removed — the only on-disk format is now
//! `autoeq::RoomConfig`.

use std::path::Path;

/// Stub kept so the load path can still ask "is migration needed?". The
/// legacy `RoomEqMeasurementsFile` schema is gone, so the answer is
/// always `false` and the caller should load the file with the
/// `autoeq::RoomConfig` parser directly.
pub fn check_needs_migration(_json: &str, _file_size: u64) -> bool {
    false
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

    let file = std::fs::File::open(csv_path).map_err(|e| format!("Failed to open CSV: {}", e))?;
    let reader = std::io::BufReader::new(file);

    let mut thd_percent = Vec::new();
    let mut rt60_ms = Vec::new();
    let mut clarity_c50_db = Vec::new();
    let mut clarity_c80_db = Vec::new();
    let mut excess_group_delay_ms = Vec::new();

    let mut lines = reader.lines();

    let header = lines
        .next()
        .ok_or("CSV file is empty")?
        .map_err(|e| format!("Failed to read header: {}", e))?;

    let has_extended_data = header.contains("thd_percent") || header.contains("c50_db");
    if !has_extended_data {
        return Err("CSV does not contain extended metrics".to_string());
    }

    for line_result in lines {
        let line = line_result.map_err(|e| format!("Failed to read line: {}", e))?;
        let parts: Vec<&str> = line.split(',').collect();

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

    let abs_path = Path::new(csv_path);
    if abs_path.exists()
        && let Ok(metrics) = read_extended_metrics_from_csv(abs_path)
    {
        log::info!("Loaded extended metrics from {:?}", abs_path);
        return Some(metrics);
    }

    if let Some(base) = base_dir {
        let rel_path = base.join(csv_path);
        if rel_path.exists()
            && let Ok(metrics) = read_extended_metrics_from_csv(&rel_path)
        {
            log::info!("Loaded extended metrics from {:?}", rel_path);
            return Some(metrics);
        }
    }

    log::debug!("No extended metrics found for CSV: {}", csv_path);
    None
}
