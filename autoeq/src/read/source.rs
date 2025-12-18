//! Measurement source handling (single file or averaging)

use crate::Curve;
use crate::read::{interpolate_log_space, read_curve_from_csv};
use ndarray::Array1;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::PathBuf;

/// Reference to a measurement file
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum MeasurementRef {
    /// Path to CSV file (freq, spl, phase columns)
    Path(PathBuf),

    /// Named measurement with optional metadata
    Named {
        /// Path to the CSV measurement file.
        path: PathBuf,
        /// Optional display name for the measurement.
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
}

impl MeasurementRef {
    /// Returns the path to the measurement file.
    pub fn path(&self) -> &PathBuf {
        match self {
            MeasurementRef::Path(p) => p,
            MeasurementRef::Named { path, .. } => path,
        }
    }

    /// Returns the optional display name, if provided.
    pub fn name(&self) -> Option<&str> {
        match self {
            MeasurementRef::Path(_) => None,
            MeasurementRef::Named { name, .. } => name.as_deref(),
        }
    }
}

/// Source of measurements (single file or multiple files for averaging)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum MeasurementSource {
    /// A single measurement file.
    Single(MeasurementRef),
    /// Multiple measurement files to be averaged.
    Multiple(Vec<MeasurementRef>),
}

/// Load a single measurement from a CSV file
pub fn load_measurement(measurement: &MeasurementRef) -> Result<Curve, Box<dyn Error>> {
    let path = measurement.path();
    read_curve_from_csv(path)
}

/// Load measurement(s) from a source and average if necessary
pub fn load_source(source: &MeasurementSource) -> Result<Curve, Box<dyn Error>> {
    match source {
        MeasurementSource::Single(m) => load_measurement(m),
        MeasurementSource::Multiple(measurements) => {
            if measurements.is_empty() {
                return Err("Measurement list is empty".into());
            }

            // Load all curves
            let mut curves = Vec::new();
            for m in measurements {
                match load_measurement(m) {
                    Ok(c) => curves.push(c),
                    Err(e) => {
                        eprintln!("Warning: failed to load measurement {:?}: {}", m.path(), e)
                    }
                }
            }

            if curves.is_empty() {
                return Err("No valid measurements loaded".into());
            }

            // Use first curve as reference grid
            let ref_curve = &curves[0];
            let freqs = ref_curve.freq.clone();

            // Interpolate all to reference grid and sum power
            let mut power_sum = Array1::<f64>::zeros(freqs.len());

            for curve in &curves {
                let interpolated = interpolate_log_space(&freqs, curve);
                // Convert SPL to power (proportional to pressure squared)
                // Power = 10^(SPL/10)
                let p = interpolated.spl.mapv(|spl| 10.0_f64.powf(spl / 10.0));
                power_sum = power_sum + p;
            }

            // Average power
            let avg_power = power_sum / (curves.len() as f64);

            // Convert back to SPL
            let avg_spl = avg_power.mapv(|p| 10.0 * p.log10());

            // Use phase from first measurement (primary position)
            let phase = ref_curve.phase.clone();

            Ok(Curve {
                freq: freqs,
                spl: avg_spl,
                phase,
            })
        }
    }
}
