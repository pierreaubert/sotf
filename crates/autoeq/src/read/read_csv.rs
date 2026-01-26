use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use crate::Curve;
use ndarray::Array1;

/// Load frequency response data from a CSV or text file
/// Expected formats:
/// - 2 columns: frequency, spl
/// - 4 columns: freq_left, spl_left, freq_right, spl_right (averaged)
pub fn load_frequency_response(
    path: &PathBuf,
) -> Result<(Array1<f64>, Array1<f64>), Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut frequencies = Vec::new();
    let mut spl_values = Vec::new();
    let mut detected_columns = 0;

    for (line_num, line) in reader.lines().enumerate() {
        let line = line?;
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }

        // Skip header if it contains text
        if line_num == 0 && (line.contains("freq") || line.contains("Freq") || line.contains("Hz"))
        {
            continue;
        }

        // Parse line (handle both comma and whitespace separation)
        let parts: Vec<&str> = if line.contains(',') {
            line.split(',').map(|s| s.trim()).collect()
        } else {
            line.split_whitespace().collect()
        };

        // Detect number of columns on first data line
        if detected_columns == 0 && parts.len() >= 2 {
            detected_columns = parts.len();
        }

        if detected_columns == 2 && parts.len() >= 2 {
            // 2-column format: freq, spl
            if let (Ok(freq), Ok(spl)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                frequencies.push(freq);
                spl_values.push(spl);
            }
        } else if detected_columns == 4 && parts.len() >= 4 {
            // 4-column format: freq_left, spl_left, freq_right, spl_right
            // Assume frequencies are the same for left and right, average the SPL
            if let (Ok(freq_l), Ok(spl_l), Ok(_freq_r), Ok(spl_r)) = (
                parts[0].parse::<f64>(),
                parts[1].parse::<f64>(),
                parts[2].parse::<f64>(),
                parts[3].parse::<f64>(),
            ) {
                frequencies.push(freq_l);
                spl_values.push((spl_l + spl_r) / 2.0); // Average left and right
            }
        }
    }

    if frequencies.is_empty() {
        return Err("No valid frequency response data found in file".into());
    }

    Ok((Array1::from_vec(frequencies), Array1::from_vec(spl_values)))
}

/// Read a frequency response curve from a CSV file
///
/// # Arguments
/// * `path` - Path to the CSV file
///
/// # Returns
/// * Result containing a Curve struct or an error
///
/// # CSV Format
/// The CSV file should have a header row with "frequency" and "spl" columns,
/// followed by rows of frequency (Hz) and SPL (dB) values.
pub fn read_curve_from_csv(path: &PathBuf) -> Result<Curve, Box<dyn Error>> {
    // Try to load as driver measurement (with optional phase) first
    match load_driver_measurement(path) {
        Ok((freq, spl, phase)) => Ok(crate::Curve { freq, spl, phase }),
        Err(_) => {
            // Fallback to load_frequency_response (handles 4-column stereo average)
            let result = load_frequency_response(path)?;
            Ok(crate::Curve {
                freq: Array1::from(result.0),
                spl: Array1::from(result.1),
                phase: None,
            })
        }
    }
}

/// Load driver measurement data from a CSV file with freq, spl, and optionally phase
///
/// # Arguments
/// * `path` - Path to the CSV file
///
/// # Returns
/// * Tuple of (frequencies, spl_values, optional phase_values)
///
/// # CSV Format
/// Expected formats:
/// - 2 columns: frequency, spl
/// - 3 columns: frequency, spl, phase
/// - Multi-column with headers: frequency_hz/freq, spl_db/spl, phase_deg/phase (extracts relevant columns)
#[allow(clippy::type_complexity)]
pub fn load_driver_measurement(
    path: &PathBuf,
) -> Result<(Array1<f64>, Array1<f64>, Option<Array1<f64>>), Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut frequencies = Vec::new();
    let mut spl_values = Vec::new();
    let mut phase_values = Vec::new();

    // Column indices (default to first 2-3 columns)
    let mut freq_col: Option<usize> = None;
    let mut spl_col: Option<usize> = None;
    let mut phase_col: Option<usize> = None;
    let mut header_parsed = false;

    for (line_num, line) in reader.lines().enumerate() {
        let line = line?;
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }

        // Parse line (handle both comma and whitespace separation)
        let parts: Vec<&str> = if line.contains(',') {
            line.split(',').map(|s| s.trim()).collect()
        } else {
            line.split_whitespace().collect()
        };

        // Try to parse header on first line
        if line_num == 0 && !header_parsed {
            let is_header = parts.iter().any(|p| {
                let lower = p.to_lowercase();
                lower.contains("freq")
                    || lower.contains("hz")
                    || lower.contains("spl")
                    || lower.contains("phase")
                    || lower.contains("db")
            });

            if is_header {
                // Parse header to find column indices
                for (idx, col_name) in parts.iter().enumerate() {
                    let lower = col_name.to_lowercase();
                    if freq_col.is_none()
                        && (lower.contains("freq") || lower == "hz" || lower == "frequency_hz")
                    {
                        freq_col = Some(idx);
                    } else if spl_col.is_none()
                        && (lower.contains("spl")
                            || lower.contains("magnitude")
                            || lower == "db"
                            || lower == "spl_db")
                    {
                        spl_col = Some(idx);
                    } else if phase_col.is_none()
                        && (lower.contains("phase") || lower == "phase_deg")
                    {
                        phase_col = Some(idx);
                    }
                }
                header_parsed = true;
                continue; // Skip header line
            }

            // No header found, use default column positions
            if parts.len() >= 2 {
                freq_col = Some(0);
                spl_col = Some(1);
                if parts.len() >= 3 {
                    phase_col = Some(2);
                }
            }
            header_parsed = true;
        }

        // Use default columns if not set
        let freq_idx = freq_col.unwrap_or(0);
        let spl_idx = spl_col.unwrap_or(1);

        // Parse data
        if parts.len() > freq_idx && parts.len() > spl_idx {
            if let (Ok(freq), Ok(spl)) =
                (parts[freq_idx].parse::<f64>(), parts[spl_idx].parse::<f64>())
            {
                frequencies.push(freq);
                spl_values.push(spl);

                // Parse phase if available
                if let Some(phase_idx) = phase_col {
                    if parts.len() > phase_idx {
                        if let Ok(phase) = parts[phase_idx].parse::<f64>() {
                            phase_values.push(phase);
                        }
                    }
                }
            }
        }
    }

    if frequencies.is_empty() {
        return Err("No valid driver measurement data found in file".into());
    }

    let phase = if !phase_values.is_empty() && phase_values.len() == frequencies.len() {
        Some(Array1::from_vec(phase_values))
    } else {
        None
    };

    Ok((
        Array1::from_vec(frequencies),
        Array1::from_vec(spl_values),
        phase,
    ))
}
