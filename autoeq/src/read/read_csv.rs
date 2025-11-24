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
    let result = load_frequency_response(path)?;
    Ok(crate::Curve {
        freq: Array1::from(result.0),
        spl: Array1::from(result.1),
    })
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
pub fn load_driver_measurement(
    path: &PathBuf,
) -> Result<(Array1<f64>, Array1<f64>, Option<Array1<f64>>), Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut frequencies = Vec::new();
    let mut spl_values = Vec::new();
    let mut phase_values = Vec::new();
    let mut detected_columns = 0;
    let mut has_phase = false;

    for (line_num, line) in reader.lines().enumerate() {
        let line = line?;
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }

        // Skip header if it contains text
        if line_num == 0
            && (line.contains("freq")
                || line.contains("Freq")
                || line.contains("Hz")
                || line.contains("spl")
                || line.contains("SPL")
                || line.contains("phase")
                || line.contains("Phase"))
        {
            // Check if header indicates phase column
            if line.contains("phase") || line.contains("Phase") {
                has_phase = true;
            }
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
            has_phase = detected_columns >= 3;
        }

        if detected_columns == 2 && parts.len() >= 2 {
            // 2-column format: freq, spl
            if let (Ok(freq), Ok(spl)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                frequencies.push(freq);
                spl_values.push(spl);
            }
        } else if detected_columns >= 3 && parts.len() >= 3 {
            // 3-column format: freq, spl, phase
            if let (Ok(freq), Ok(spl), Ok(phase)) = (
                parts[0].parse::<f64>(),
                parts[1].parse::<f64>(),
                parts[2].parse::<f64>(),
            ) {
                frequencies.push(freq);
                spl_values.push(spl);
                phase_values.push(phase);
            }
        }
    }

    if frequencies.is_empty() {
        return Err("No valid driver measurement data found in file".into());
    }

    let phase = if has_phase && !phase_values.is_empty() {
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
