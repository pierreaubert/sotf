//! Headphone EQ optimization
//!
//! Provides thin wrappers around the autoeq library for headphone equalization.
//! Most functionality is delegated to the library.

use std::path::PathBuf;

/// Validated measurement data for a file preview and its identity disclosure.
/// Loading belongs on a worker thread, never in a UI render method.
#[derive(Clone, Debug)]
pub struct HeadphoneMeasurementPreview {
    path: PathBuf,
    points: Vec<(f64, f64)>,
    bounds_hz: (f64, f64),
    provenance: Option<autoeq_measurements::MeasurementRecord>,
}

impl HeadphoneMeasurementPreview {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        path.to_str()
            .ok_or_else(|| "Measurement path must be valid UTF-8".to_string())?;
        let curve = autoeq::read::read_curve_from_csv(&path.to_path_buf())
            .map_err(|error| error.to_string())?;
        if curve.freq.len() != curve.spl.len() || curve.freq.len() < 2 {
            return Err("A measurement needs at least two frequency/level pairs".into());
        }
        let points: Vec<_> = curve
            .freq
            .iter()
            .copied()
            .zip(curve.spl.iter().copied())
            .collect();
        if points.iter().any(|(frequency, level)| {
            !frequency.is_finite() || *frequency <= 0.0 || !level.is_finite()
        }) {
            return Err("Measurement frequencies must be positive and all values finite".into());
        }
        let bounds_hz = points.iter().fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(low, high), (frequency, _)| (low.min(*frequency), high.max(*frequency)),
        );
        if bounds_hz.0 >= bounds_hz.1 {
            return Err("A measurement needs distinct frequency values".into());
        }
        let provenance = match autoeq_measurements::read_sidecar(path) {
            Ok(record) => {
                let validation = record.validate(autoeq_measurements::ValidationMode::Warn);
                if !validation.is_valid() {
                    return Err(format!(
                        "Invalid measurement provenance: {}",
                        validation.errors.join("; ")
                    ));
                }
                let hash = curve.content_hash().map_err(|error| error.to_string())?;
                if hash != record.provenance.content_hash {
                    return Err("Measurement provenance does not match the imported curve".into());
                }
                Some(record)
            }
            Err(autoeq_measurements::ProvenanceError::Io(error))
                if error.kind() == std::io::ErrorKind::NotFound =>
            {
                None
            }
            Err(error) => return Err(format!("Could not read measurement provenance: {error}")),
        };
        Ok(Self {
            path: path.to_path_buf(),
            points,
            bounds_hz,
            provenance,
        })
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
    pub fn points(&self) -> &[(f64, f64)] {
        &self.points
    }
    pub fn bounds_hz(&self) -> (f64, f64) {
        self.bounds_hz
    }

    /// Validated source evidence, retained independently of the selected target.
    pub fn provenance(&self) -> Option<&autoeq_measurements::MeasurementRecord> {
        self.provenance.as_ref()
    }

    /// Optional source-supplied identity fields in the standard acquisition
    /// extension map. Missing or non-text values remain unknown.
    pub fn identity_field(&self, name: &str) -> Option<&str> {
        self.provenance()?
            .provenance
            .acquisition
            .extensions
            .get(name)?
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }
}

// Re-export types from autoeq for convenience
pub use autoeq::{HeadphoneOptResult, VisualizationCurves};

/// Bundled target curve data
pub mod target_curves {
    pub const HARMAN_OVER_EAR_2018: &str =
        include_str!("../../../../data_tests/targets/harman-over-ear-2018.csv");
    pub const HARMAN_OVER_EAR_2015: &str =
        include_str!("../../../../data_tests/targets/harman-over-ear-2015.csv");
    pub const HARMAN_OVER_EAR_2013: &str =
        include_str!("../../../../data_tests/targets/harman-over-ear-2013.csv");
    pub const HARMAN_IN_EAR_2019: &str =
        include_str!("../../../../data_tests/targets/harman-in-ear-2019.csv");
}

/// Result of headphone EQ optimization with all curves for visualization
#[derive(Clone, Debug)]
pub struct HeadphoneOptimizationResult {
    /// Optimized biquad filters
    pub biquads: Vec<math_audio_iir_fir::Biquad>,
    /// Frequency points (Hz) - log-spaced
    pub frequencies: Vec<f64>,
    /// Input headphone measurement curve (dB)
    pub input_curve: Vec<f64>,
    /// Target curve (dB)
    pub target_curve: Vec<f64>,
    /// Deviation from target = target - input (dB)
    pub deviation_curve: Vec<f64>,
    /// Combined filter response (dB)
    pub filter_response: Vec<f64>,
    /// Error = deviation - filter_response (dB)
    pub error_curve: Vec<f64>,
    /// Corrected response = input + filter_response (dB)
    pub corrected_curve: Vec<f64>,
    /// Individual filter responses (each filter's dB response)
    pub individual_filter_responses: Vec<Vec<f64>>,
    /// Path where results were saved
    pub output_path: String,
    /// Optimization history (iteration, loss)
    pub optimization_history: Vec<(usize, f64)>,
    /// Initial loss value
    pub initial_loss: f64,
    /// Final loss value
    pub final_loss: f64,
}

impl From<HeadphoneOptResult> for HeadphoneOptimizationResult {
    fn from(result: HeadphoneOptResult) -> Self {
        Self {
            biquads: result.biquads,
            frequencies: result.curves.frequencies,
            input_curve: result.curves.input_curve,
            target_curve: result.curves.target_curve,
            deviation_curve: result.curves.deviation_curve,
            filter_response: result.curves.filter_response,
            error_curve: result.curves.error_curve,
            corrected_curve: result.curves.corrected_curve,
            individual_filter_responses: result.curves.individual_filter_responses,
            output_path: String::new(),
            optimization_history: result.history,
            initial_loss: result.initial_loss,
            final_loss: result.final_loss,
        }
    }
}

/// Run headphone EQ optimization
///
/// # Arguments
/// * `curve_path` - Path to the headphone measurement CSV file
/// * `target` - Target curve identifier (e.g., "harman-over-ear-2018", "custom")
/// * `target_custom_path` - Path to custom target curve (only used if target is "custom")
/// * `args` - Optimization arguments (use Args::headphone_defaults() as base)
/// * `_export_format` - Export format for the resulting EQ file (unused, for compatibility)
///
/// # Returns
/// The optimization result with all curves for visualization
pub fn run_headphone_optimization(
    curve_path: &str,
    target: &str,
    target_custom_path: &str,
    args: &autoeq::Args,
    _export_format: &str,
) -> Result<HeadphoneOptimizationResult, String> {
    // Load headphone measurement
    let curve_path = PathBuf::from(curve_path);

    // Load target curve
    let target_curve = load_target_curve(target, target_custom_path)?;

    // Use library function (no progress callback, no config)
    let optim_params = autoeq::OptimParams::from(args);
    let result = autoeq::optimize_headphone(
        &curve_path,
        &target_curve,
        &optim_params,
        None, // No progress config
        None::<fn(&autoeq::ProgressUpdate) -> autoeq::de::CallbackAction>,
    )
    .map_err(|e| e.to_string())?;

    Ok(HeadphoneOptimizationResult::from(result))
}

/// Run headphone EQ optimization with a progress callback
pub fn run_headphone_optimization_with_callback<F>(
    curve_path: &str,
    target: &str,
    target_custom_path: &str,
    args: &autoeq::Args,
    progress_callback: Option<F>,
) -> Result<HeadphoneOptimizationResult, String>
where
    F: FnMut(&autoeq::ProgressUpdate) -> autoeq::de::CallbackAction + Send + 'static,
{
    let curve_path = PathBuf::from(curve_path);
    let target_curve = load_target_curve(target, target_custom_path)?;

    let progress_config = Some(autoeq::ProgressCallbackConfig {
        interval: 50,
        include_biquads: false,
        include_filter_response: false,
        frequencies: Vec::new(),
    });

    let optim_params = autoeq::OptimParams::from(args);
    let result = autoeq::optimize_headphone(
        &curve_path,
        &target_curve,
        &optim_params,
        progress_config,
        progress_callback,
    )
    .map_err(|e| e.to_string())?;

    Ok(HeadphoneOptimizationResult::from(result))
}

/// Load target curve from bundled data or custom file
pub fn load_target_curve(target: &str, custom_path: &str) -> Result<autoeq::Curve, String> {
    match target {
        "flat" => {
            let mut curve = parse_csv_curve(target_curves::HARMAN_OVER_EAR_2018)?;
            curve.spl.fill(0.0);
            Ok(curve)
        }
        "harman-over-ear-2018" => parse_csv_curve(target_curves::HARMAN_OVER_EAR_2018),
        "harman-over-ear-2015" => parse_csv_curve(target_curves::HARMAN_OVER_EAR_2015),
        "harman-over-ear-2013" => parse_csv_curve(target_curves::HARMAN_OVER_EAR_2013),
        "harman-in-ear-2019" => parse_csv_curve(target_curves::HARMAN_IN_EAR_2019),
        "custom" => autoeq::read::read_curve_from_csv(&PathBuf::from(custom_path))
            .map_err(|e| format!("Failed to read custom target curve: {}", e)),
        _ => autoeq::read::read_curve_from_csv(&PathBuf::from(custom_path))
            .map_err(|e| format!("A target curve is required for headphone: {}", e)),
    }
}

/// Parse a CSV string into a Curve
pub fn parse_csv_curve(csv_data: &str) -> Result<autoeq::Curve, String> {
    use ndarray::Array1;

    let mut freq = Vec::new();
    let mut spl = Vec::new();

    for (i, line) in csv_data.lines().enumerate() {
        // Skip header line
        if i == 0 && line.contains("frequency") {
            continue;
        }

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 2 {
            if let (Ok(f), Ok(s)) = (
                parts[0].trim().parse::<f64>(),
                parts[1].trim().parse::<f64>(),
            ) {
                freq.push(f);
                spl.push(s);
            }
        }
    }

    if freq.is_empty() {
        return Err("No valid data found in CSV".to_string());
    }

    Ok(autoeq::Curve {
        freq: Array1::from(freq),
        spl: Array1::from(spl),
        phase: None,
        ..Default::default()
    })
}

#[cfg(test)]
mod measurement_preview_tests {
    use super::HeadphoneMeasurementPreview;
    use std::io::Write;

    #[test]
    fn flat_target_loads_without_a_custom_file() {
        let flat = super::load_target_curve("flat", "").unwrap();
        let reference = super::load_target_curve("harman-over-ear-2018", "").unwrap();
        assert_eq!(flat.freq, reference.freq);
        assert!(flat.freq.len() > 2);
        assert!(flat.spl.iter().all(|level| *level == 0.0));
    }

    #[test]
    fn file_preview_retains_source_points_and_bounds() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "frequency,spl\n20,-4\n1000,0\n20000,-3").unwrap();
        let preview = HeadphoneMeasurementPreview::load(file.path()).unwrap();
        assert_eq!(preview.path(), file.path());
        assert_eq!(preview.bounds_hz(), (20.0, 20000.0));
        assert_eq!(
            preview.points(),
            &[(20.0, -4.0), (1000.0, 0.0), (20000.0, -3.0)]
        );
    }

    #[test]
    fn file_preview_rejects_invalid_or_degenerate_measurements() {
        for contents in [
            "frequency,spl\n0,0\n100,1",
            "frequency,spl\n20,NaN\n100,1",
            "frequency,spl\n20,0",
            "frequency,spl\n20,0\n20,1",
        ] {
            let mut file = tempfile::NamedTempFile::new().unwrap();
            file.write_all(contents.as_bytes()).unwrap();
            assert!(
                HeadphoneMeasurementPreview::load(file.path()).is_err(),
                "{contents}"
            );
        }
    }

    #[test]
    fn file_preview_retains_validated_provenance_and_rejects_stale_sidecars() {
        use autoeq_measurements::{MeasurementOrigin, MeasurementRecord, write_sidecar};
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("headphone.csv");
        std::fs::write(&path, "frequency,spl\n20,-4\n1000,0\n20000,-3\n").unwrap();
        let curve = autoeq::read::read_curve_from_csv(&path).unwrap();
        let mut record =
            MeasurementRecord::from_source_path(curve, MeasurementOrigin::Csv, &path).unwrap();
        for (key, value) in [
            ("model", "Example variant"),
            ("rig", "IEC fixture"),
            ("sample", "Unit 2, left ear"),
            ("compensation", "Uncompensated"),
        ] {
            record
                .provenance
                .acquisition
                .extensions
                .insert(key.into(), value.into());
        }
        write_sidecar(&path, &record).unwrap();
        let preview = HeadphoneMeasurementPreview::load(&path).unwrap();
        assert_eq!(preview.identity_field("model"), Some("Example variant"));
        assert_eq!(preview.identity_field("rig"), Some("IEC fixture"));
        assert_eq!(preview.identity_field("sample"), Some("Unit 2, left ear"));
        assert_eq!(
            preview.identity_field("compensation"),
            Some("Uncompensated")
        );
        assert_eq!(preview.provenance().unwrap().id, record.id);
        std::fs::write(&path, "frequency,spl\n20,-1\n1000,0\n20000,-3\n").unwrap();
        assert!(
            HeadphoneMeasurementPreview::load(&path)
                .unwrap_err()
                .contains("does not match")
        );
        std::fs::write(autoeq_measurements::sidecar_path(&path), "invalid json").unwrap();
        assert!(
            HeadphoneMeasurementPreview::load(&path)
                .unwrap_err()
                .contains("provenance")
        );
    }
}
