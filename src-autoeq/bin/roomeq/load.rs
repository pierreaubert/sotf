//! Measurement loading utilities for room EQ

use super::types::{MeasurementRef, SpeakerConfig};
use autoeq::read::read_curve_from_csv;
use autoeq::Curve;
use std::error::Error;

/// Load a single measurement from a CSV file
pub fn load_measurement(measurement: &MeasurementRef) -> Result<Curve, Box<dyn Error>> {
    let path = measurement.path();
    read_curve_from_csv(path)
}

/// Load all measurements for a speaker configuration
pub fn load_speaker_measurements(
    speaker_config: &SpeakerConfig,
) -> Result<Vec<Curve>, Box<dyn Error>> {
    match speaker_config {
        SpeakerConfig::Single(measurement) => {
            let curve = load_measurement(measurement)?;
            Ok(vec![curve])
        }
        SpeakerConfig::Group(group) => {
            let mut curves = Vec::new();
            for measurement in &group.measurements {
                let curve = load_measurement(measurement)?;
                curves.push(curve);
            }
            Ok(curves)
        }
    }
}

/// Check if a speaker config is a group (multi-driver)
pub fn is_group(speaker_config: &SpeakerConfig) -> bool {
    matches!(speaker_config, SpeakerConfig::Group(_))
}

/// Get the crossover reference for a speaker group
pub fn get_crossover_ref(speaker_config: &SpeakerConfig) -> Option<&str> {
    match speaker_config {
        SpeakerConfig::Group(group) => group.crossover.as_deref(),
        _ => None,
    }
}
