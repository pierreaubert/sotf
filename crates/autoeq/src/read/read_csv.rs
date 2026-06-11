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

/// Read a frequency response curve from a CSV file.
///
/// After loading the raw columns, calls [`crate::Curve::decompose_into_cache`]
/// to populate `min_phase`, `excess_phase`, and `excess_delay_ms` when phase
/// data is present. These derived fields are **not** persisted to CSV — they
/// are always recomputed at load time so the decomposition algorithm can
/// evolve without requiring a re-export (§2.4 and §2.11 Q3 of
/// `docs/gd_opt_v2_plan.md`). When phase is absent the cache fields stay
/// `None`.
///
/// # Arguments
/// * `path` - Path to the CSV file
///
/// # CSV Format
/// The CSV file should have a header row with "frequency" and "spl" columns,
/// followed by rows of frequency (Hz) and SPL (dB) values.
pub fn read_curve_from_csv(path: &PathBuf) -> Result<Curve, Box<dyn Error>> {
    // Try to load as driver measurement (with optional phase / coherence /
    // noise_floor_db) first. GD-Opt v2 adds `coherence` and
    // `noise_floor_db` columns — see §2.4 of `docs/gd_opt_v2_plan.md`.
    let mut curve = match load_driver_measurement(path) {
        Ok((freq, spl, phase, coherence, noise_floor_db)) => crate::Curve {
            freq,
            spl,
            phase,
            coherence,
            noise_floor_db,
            ..Default::default()
        },
        Err(_) => {
            // Fallback to load_frequency_response (handles 4-column stereo average)
            let result = load_frequency_response(path)?;
            crate::Curve {
                freq: Array1::from(result.0),
                spl: Array1::from(result.1),
                phase: None,
                ..Default::default()
            }
        }
    };
    // GD-1d: populate min-phase / excess-phase cache at the disk-load boundary.
    // No-op when phase is absent or arrays disagree in length.
    curve.decompose_into_cache();
    Ok(curve)
}

/// Load driver measurement data from a CSV file.
///
/// # Arguments
/// * `path` - Path to the CSV file
///
/// # Returns
/// * `(frequencies, spl_values, phase?, coherence?, noise_floor_db?)`.
///   The three optional columns are populated when the matching
///   header is present and every row parses cleanly; otherwise `None`.
///
/// # CSV Format
/// Column discovery is header-name driven, so column order doesn't matter.
/// Recognised column names (case-insensitive):
/// - **freq**: `frequency_hz`, `frequency`, `freq`, or `hz`.
/// - **spl**: `spl`, `spl_db`, `magnitude`, or `db`.
/// - **phase**: `phase_deg` or any column name containing `phase`.
/// - **coherence**: `coherence` (γ² from the multi-sweep average,
///   added by GD-Opt v2 — see `docs/gd_opt_v2_plan.md` §2.4).
/// - **noise_floor_db**: `noise_floor_db` (per-bin noise-floor
///   estimate in dB, added by GD-Opt v2 §2.4).
///
/// Headerless CSVs default to positional columns: col 0 = freq,
/// col 1 = spl, col 2 = phase (if present). Coherence and noise-floor
/// columns require explicit headers.
#[allow(clippy::type_complexity)]
pub fn load_driver_measurement(
    path: &PathBuf,
) -> Result<
    (
        Array1<f64>,
        Array1<f64>,
        Option<Array1<f64>>,
        Option<Array1<f64>>,
        Option<Array1<f64>>,
    ),
    Box<dyn std::error::Error>,
> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut frequencies = Vec::new();
    let mut spl_values = Vec::new();
    let mut phase_values = Vec::new();
    let mut coherence_values = Vec::new();
    let mut noise_floor_values = Vec::new();

    // Column indices (default to first 2-3 columns)
    let mut freq_col: Option<usize> = None;
    let mut spl_col: Option<usize> = None;
    let mut phase_col: Option<usize> = None;
    let mut coherence_col: Option<usize> = None;
    let mut noise_floor_col: Option<usize> = None;
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
                    || lower.contains("coherence")
                    || lower.contains("noise_floor")
            });

            if is_header {
                // Parse header to find column indices
                for (idx, col_name) in parts.iter().enumerate() {
                    let lower = col_name.to_lowercase();
                    // Order matters — check the most specific names first.
                    // `noise_floor_db` contains `db`, so it must be checked
                    // before `spl_col`'s `db` fallback, otherwise `spl`
                    // would hijack it. Same for `coherence` vs any
                    // generic check.
                    if noise_floor_col.is_none() && lower.contains("noise_floor") {
                        noise_floor_col = Some(idx);
                    } else if coherence_col.is_none() && lower == "coherence" {
                        coherence_col = Some(idx);
                    } else if freq_col.is_none()
                        && (lower.contains("freq") || lower == "hz" || lower == "frequency_hz")
                    {
                        freq_col = Some(idx);
                    } else if phase_col.is_none()
                        && (lower.contains("phase") || lower == "phase_deg")
                    {
                        phase_col = Some(idx);
                    } else if spl_col.is_none()
                        && (lower.contains("spl")
                            || lower.contains("magnitude")
                            || lower == "db"
                            || lower == "spl_db")
                    {
                        spl_col = Some(idx);
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
        if parts.len() > freq_idx
            && parts.len() > spl_idx
            && let (Ok(freq), Ok(spl)) = (
                parts[freq_idx].parse::<f64>(),
                parts[spl_idx].parse::<f64>(),
            )
        {
            frequencies.push(freq);
            spl_values.push(spl);

            // Parse phase if available
            if let Some(phase_idx) = phase_col
                && parts.len() > phase_idx
                && let Ok(phase) = parts[phase_idx].parse::<f64>()
            {
                phase_values.push(phase);
            }
            // Parse coherence if available
            if let Some(coh_idx) = coherence_col
                && parts.len() > coh_idx
                && let Ok(coh) = parts[coh_idx].parse::<f64>()
            {
                coherence_values.push(coh);
            }
            // Parse noise_floor_db if available
            if let Some(nf_idx) = noise_floor_col
                && parts.len() > nf_idx
                && let Ok(nf) = parts[nf_idx].parse::<f64>()
            {
                noise_floor_values.push(nf);
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
    let coherence = if !coherence_values.is_empty() && coherence_values.len() == frequencies.len() {
        Some(Array1::from_vec(coherence_values))
    } else {
        None
    };
    let noise_floor_db =
        if !noise_floor_values.is_empty() && noise_floor_values.len() == frequencies.len() {
            Some(Array1::from_vec(noise_floor_values))
        } else {
            None
        };

    Ok((
        Array1::from_vec(frequencies),
        Array1::from_vec(spl_values),
        phase,
        coherence,
        noise_floor_db,
    ))
}

#[cfg(test)]
mod gd_v2_tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_tmp(csv: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(csv.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn legacy_three_column_csv_still_loads() {
        let csv = "frequency,spl,phase\n20,0.0,10\n200,1.0,20\n2000,2.0,30\n";
        let f = write_tmp(csv);
        let curve = read_curve_from_csv(&f.path().to_path_buf()).unwrap();
        assert_eq!(curve.freq.len(), 3);
        assert_eq!(curve.spl.len(), 3);
        assert!(curve.phase.is_some());
        assert_eq!(curve.phase.as_ref().unwrap().len(), 3);
        assert!(curve.coherence.is_none());
        assert!(curve.noise_floor_db.is_none());
    }

    #[test]
    fn gd_v2_extended_csv_populates_coherence_and_noise_floor() {
        let csv = "\
frequency,spl,phase,coherence,noise_floor_db
20,0.0,10,0.95,-45
200,1.0,20,0.98,-50
2000,2.0,30,0.99,-55
";
        let f = write_tmp(csv);
        let curve = read_curve_from_csv(&f.path().to_path_buf()).unwrap();
        assert_eq!(curve.freq.len(), 3);
        assert_eq!(curve.spl.len(), 3);
        assert_eq!(curve.phase.as_ref().unwrap().len(), 3);
        let coh = curve.coherence.expect("coherence populated");
        assert_eq!(coh.len(), 3);
        assert!((coh[0] - 0.95).abs() < 1e-9);
        let nf = curve.noise_floor_db.expect("noise_floor_db populated");
        assert_eq!(nf.len(), 3);
        assert!((nf[2] + 55.0).abs() < 1e-9);
        // GD-1d: derived fields are now populated at load time when phase is present.
        assert!(curve.min_phase.is_some(), "min_phase populated by GD-1d");
        assert!(
            curve.excess_phase.is_some(),
            "excess_phase populated by GD-1d"
        );
        assert!(
            curve.excess_delay_ms.is_some(),
            "excess_delay_ms populated by GD-1d"
        );
    }

    #[test]
    fn column_order_is_header_driven() {
        // Coherence before freq; noise_floor_db before phase. The parser
        // must key off names, not positions.
        let csv = "\
coherence,frequency,noise_floor_db,phase,spl
0.9,20,-45,10,0.0
0.95,200,-50,20,1.0
";
        let f = write_tmp(csv);
        let curve = read_curve_from_csv(&f.path().to_path_buf()).unwrap();
        assert_eq!(curve.freq.len(), 2);
        assert!((curve.freq[0] - 20.0).abs() < 1e-9);
        assert!((curve.freq[1] - 200.0).abs() < 1e-9);
        assert!((curve.spl[0]).abs() < 1e-9);
        assert!((curve.spl[1] - 1.0).abs() < 1e-9);
        let coh = curve.coherence.expect("coherence populated");
        assert!((coh[0] - 0.9).abs() < 1e-9);
        let nf = curve.noise_floor_db.expect("noise_floor_db populated");
        assert!((nf[0] + 45.0).abs() < 1e-9);
    }

    #[test]
    fn mismatched_extended_row_count_drops_column() {
        // noise_floor_db column has one unparseable row ("nan-ish"); the
        // parser must keep the other columns but drop noise_floor_db.
        let csv = "\
frequency,spl,noise_floor_db
20,0.0,-45
200,1.0,not-a-number
2000,2.0,-55
";
        let f = write_tmp(csv);
        let curve = read_curve_from_csv(&f.path().to_path_buf()).unwrap();
        assert_eq!(curve.freq.len(), 3);
        assert_eq!(curve.spl.len(), 3);
        assert!(curve.noise_floor_db.is_none());
    }

    #[test]
    fn headerless_two_column_csv_loads() {
        let csv = "20 0.0\n200 1.0\n2000 2.0\n";
        let f = write_tmp(csv);
        let result = load_driver_measurement(&f.path().to_path_buf()).unwrap();
        assert_eq!(result.0.len(), 3);
        assert_eq!(result.1.len(), 3);
        assert!(result.2.is_none());
    }

    #[test]
    fn empty_file_errors() {
        let csv = "";
        let f = write_tmp(csv);
        let result = load_driver_measurement(&f.path().to_path_buf());
        assert!(result.is_err());
    }

    #[test]
    fn comments_only_file_errors() {
        let csv = "# comment\n// another comment\n";
        let f = write_tmp(csv);
        let result = load_driver_measurement(&f.path().to_path_buf());
        assert!(result.is_err());
    }

    #[test]
    fn headerless_with_phase_loads() {
        let csv = "20 0.0 10\n200 1.0 20\n2000 2.0 30\n";
        let f = write_tmp(csv);
        let result = load_driver_measurement(&f.path().to_path_buf()).unwrap();
        assert_eq!(result.0.len(), 3);
        assert!(result.2.is_some());
        assert_eq!(result.2.as_ref().unwrap().len(), 3);
    }

    #[test]
    fn comma_separated_headerless_loads() {
        let csv = "20,0.0\n200,1.0\n2000,2.0\n";
        let f = write_tmp(csv);
        let result = load_driver_measurement(&f.path().to_path_buf()).unwrap();
        assert_eq!(result.0.len(), 3);
        assert_eq!(result.1.len(), 3);
    }

    #[test]
    fn header_with_coherence_and_noise_floor() {
        let csv = "\
freq,spl,phase,coherence,noise_floor_db
20,0.0,10,0.95,-45
200,1.0,20,0.98,-50
2000,2.0,30,0.99,-55
";
        let f = write_tmp(csv);
        let result = load_driver_measurement(&f.path().to_path_buf()).unwrap();
        assert_eq!(result.0.len(), 3);
        assert!(result.2.is_some());
        assert!(result.3.is_some());
        assert!(result.4.is_some());
        let coh = result.3.unwrap();
        assert!((coh[0] - 0.95).abs() < 1e-9);
        let nf = result.4.unwrap();
        assert!((nf[1] + 50.0).abs() < 1e-9);
    }
}
