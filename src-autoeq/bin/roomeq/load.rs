//! Measurement loading utilities for room EQ

use super::types::MeasurementRef;
use autoeq::read::read_curve_from_csv;
use autoeq::Curve;
use std::error::Error;

/// Load a single measurement from a CSV file
pub fn load_measurement(measurement: &MeasurementRef) -> Result<Curve, Box<dyn Error>> {
    let path = measurement.path();
    read_curve_from_csv(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_load_measurement_path_ref() {
        let measurement = MeasurementRef::Path(PathBuf::from("tests/data/roomeq/test_speaker_left.csv"));
        let result = load_measurement(&measurement);
        assert!(result.is_ok(), "Should load test measurement");

        let curve = result.unwrap();
        assert!(!curve.freq.is_empty(), "Frequency data should not be empty");
        assert_eq!(curve.freq.len(), curve.spl.len(), "Frequency and SPL should have same length");
    }

    #[test]
    fn test_load_measurement_named_ref() {
        let measurement = MeasurementRef::Named {
            path: PathBuf::from("tests/data/roomeq/test_speaker_right.csv"),
            name: Some("Right Speaker".to_string()),
        };
        let result = load_measurement(&measurement);
        assert!(result.is_ok(), "Should load named measurement");
    }

    #[test]
    fn test_load_measurement_nonexistent() {
        let measurement = MeasurementRef::Path(PathBuf::from("nonexistent_file.csv"));
        let result = load_measurement(&measurement);
        assert!(result.is_err(), "Should fail for nonexistent file");
    }
}
