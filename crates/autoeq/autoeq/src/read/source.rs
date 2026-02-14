//! Measurement source handling (single file or averaging)

use crate::Curve;
use crate::read::{interpolate_log_space, read_curve_from_csv};
use ndarray::Array1;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::{Path, PathBuf};

/// Inline measurement data (frequencies, SPL, phase)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InlineMeasurement {
    /// Frequency points in Hz
    pub frequencies: Vec<f64>,
    /// Sound Pressure Level in dB
    pub magnitude_db: Vec<f64>,
    /// Phase in degrees (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase_deg: Option<Vec<f64>>,
    /// Optional display name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Optional path to associated WAV file
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wav_path: Option<String>,
    /// Optional path to associated CSV file
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csv_path: Option<String>,
}

impl InlineMeasurement {
    /// Resolve relative paths in this measurement against a base directory.
    /// If csv_path or wav_path is relative, prepend the base directory.
    pub fn resolve_paths(&mut self, base_dir: &Path) {
        if let Some(ref csv_path) = self.csv_path {
            let path = PathBuf::from(csv_path);
            if path.is_relative() {
                self.csv_path = Some(base_dir.join(&path).to_string_lossy().to_string());
            }
        }
        if let Some(ref wav_path) = self.wav_path {
            let path = PathBuf::from(wav_path);
            if path.is_relative() {
                self.wav_path = Some(base_dir.join(&path).to_string_lossy().to_string());
            }
        }
    }
}

/// Reference to a measurement file
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum MeasurementRef {
    /// Inline measurement data (stored directly in JSON)
    Inline(InlineMeasurement),

    /// Named measurement with optional metadata
    Named {
        /// Path to the CSV measurement file.
        path: PathBuf,
        /// Optional display name for the measurement.
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },

    /// Path to CSV file (freq, spl, phase columns)
    Path(PathBuf),
}

impl MeasurementRef {
    /// Returns the path to the measurement file, if this is a file-based reference.
    /// Returns None for inline measurements.
    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            MeasurementRef::Path(p) => Some(p),
            MeasurementRef::Named { path, .. } => Some(path),
            MeasurementRef::Inline(_) => None,
        }
    }

    /// Returns the optional display name, if provided.
    pub fn name(&self) -> Option<&str> {
        match self {
            MeasurementRef::Path(_) => None,
            MeasurementRef::Named { name, .. } => name.as_deref(),
            MeasurementRef::Inline(inline) => inline.name.as_deref(),
        }
    }

    /// Returns true if this is an inline measurement (data stored in JSON)
    pub fn is_inline(&self) -> bool {
        matches!(self, MeasurementRef::Inline(_))
    }

    /// Returns the inline measurement data, if this is an inline reference.
    pub fn inline_data(&self) -> Option<&InlineMeasurement> {
        match self {
            MeasurementRef::Inline(data) => Some(data),
            _ => None,
        }
    }

    /// Resolve relative paths in this measurement reference against a base directory.
    pub fn resolve_paths(&mut self, base_dir: &Path) {
        match self {
            MeasurementRef::Path(p) => {
                if p.is_relative() {
                    *p = base_dir.join(&*p);
                }
            }
            MeasurementRef::Named { path, .. } => {
                if path.is_relative() {
                    *path = base_dir.join(&*path);
                }
            }
            MeasurementRef::Inline(inline) => {
                inline.resolve_paths(base_dir);
            }
        }
    }
}

/// Source of measurements (single file, multiple files for averaging, or in-memory curve)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum MeasurementSource {
    /// A single measurement file with optional speaker name
    Single(MeasurementSingle),
    /// Multiple measurement files to be averaged with optional speaker name
    Multiple(MeasurementMultiple),
    /// In-memory curve data (not serializable to JSON config files).
    /// Use this when curves are already loaded in memory.
    #[serde(skip)]
    InMemory(Curve),
}

/// Single measurement with metadata
/// 
/// Custom implementation to support both string path and object with speaker_name
#[derive(Debug, Clone, JsonSchema)]
pub struct MeasurementSingle {
    pub measurement: MeasurementRef,
    pub speaker_name: Option<String>,
}

impl Serialize for MeasurementSingle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.speaker_name.is_none() {
            // No metadata, serialize as the inner ref (might be string or object)
            self.measurement.serialize(serializer)
        } else {
            // Has metadata, must serialize as object
            use serde::ser::SerializeMap;
            let mut map = serializer.serialize_map(None)?;
            match &self.measurement {
                MeasurementRef::Path(p) => {
                    map.serialize_entry("path", p)?;
                }
                MeasurementRef::Named { path, name } => {
                    map.serialize_entry("path", path)?;
                    if let Some(n) = name {
                        map.serialize_entry("name", n)?;
                    }
                }
                MeasurementRef::Inline(inline) => {
                    // Inline is already an object, but we can't easily merge without duplicating fields
                    // or using a temporary value.
                    map.serialize_entry("inline", inline)?;
                }
            }
            map.serialize_entry("speaker_name", &self.speaker_name)?;
            map.end()
        }
    }
}

impl<'de> Deserialize<'de> for MeasurementSingle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            path: Option<PathBuf>,
            name: Option<String>,
            inline: Option<InlineMeasurement>,
            speaker_name: Option<String>,
            // Also support direct string for 'path' if not in object
            #[serde(flatten)]
            extra: serde_json::Value,
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        
        if value.is_string() {
            // Case: "path/to/file.csv"
            let path = value.as_str().unwrap().into();
            return Ok(MeasurementSingle {
                measurement: MeasurementRef::Path(path),
                speaker_name: None,
            });
        }

        if let Ok(helper) = serde_json::from_value::<Helper>(value.clone()) {
            let speaker_name = helper.speaker_name;
            
            if let Some(inline) = helper.inline {
                return Ok(MeasurementSingle {
                    measurement: MeasurementRef::Inline(inline),
                    speaker_name,
                });
            }
            
            if let Some(path) = helper.path {
                if let Some(name) = helper.name {
                    return Ok(MeasurementSingle {
                        measurement: MeasurementRef::Named { path, name: Some(name) },
                        speaker_name,
                    });
                } else {
                    return Ok(MeasurementSingle {
                        measurement: MeasurementRef::Path(path),
                        speaker_name,
                    });
                }
            }
        }

        // Fallback to trying to parse as MeasurementRef if it doesn't match our helper
        let measurement = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(MeasurementSingle {
            measurement,
            speaker_name: None,
        })
    }
}

/// Multiple measurements with metadata
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MeasurementMultiple {
    pub measurements: Vec<MeasurementRef>,
    /// Optional speaker name (e.g., "Genelec 8361A")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_name: Option<String>,
}

impl MeasurementSource {
    /// Returns the optional speaker name, if provided.
    pub fn speaker_name(&self) -> Option<&str> {
        match self {
            MeasurementSource::Single(s) => s.speaker_name.as_deref(),
            MeasurementSource::Multiple(m) => m.speaker_name.as_deref(),
            MeasurementSource::InMemory(_) => None,
        }
    }

    /// Resolve relative paths in this measurement source against a base directory.
    pub fn resolve_paths(&mut self, base_dir: &Path) {
        match self {
            MeasurementSource::Single(s) => s.measurement.resolve_paths(base_dir),
            MeasurementSource::Multiple(m) => {
                for r in &mut m.measurements {
                    r.resolve_paths(base_dir);
                }
            }
            MeasurementSource::InMemory(_) => {} // No paths to resolve
        }
    }
}

/// Load a single measurement from a file or inline data
pub fn load_measurement(measurement: &MeasurementRef) -> Result<Curve, Box<dyn Error>> {
    match measurement {
        MeasurementRef::Path(path) => read_curve_from_csv(path),
        MeasurementRef::Named { path, .. } => read_curve_from_csv(path),
        MeasurementRef::Inline(inline) => {
            // If inline data is empty but csv_path is provided, load from CSV
            if inline.frequencies.is_empty() || inline.magnitude_db.is_empty() {
                if let Some(ref csv_path) = inline.csv_path {
                    return read_curve_from_csv(&PathBuf::from(csv_path));
                }
                return Err(format!(
                    "Inline measurement has empty data and no csv_path to fall back to (name: {:?})",
                    inline.name
                )
                .into());
            }

            if inline.frequencies.len() != inline.magnitude_db.len() {
                return Err(format!(
                    "Inline measurement has mismatched lengths: {} frequencies, {} magnitude values",
                    inline.frequencies.len(),
                    inline.magnitude_db.len()
                )
                .into());
            }

            let phase = inline.phase_deg.as_ref().and_then(|p| {
                if p.len() != inline.frequencies.len() {
                    eprintln!(
                        "Warning: phase array length ({}) doesn't match frequencies ({}), ignoring phase",
                        p.len(),
                        inline.frequencies.len()
                    );
                    None
                } else {
                    Some(Array1::from(p.clone()))
                }
            });

            Ok(Curve {
                freq: Array1::from(inline.frequencies.clone()),
                spl: Array1::from(inline.magnitude_db.clone()),
                phase,
            })
        }
    }
}

/// Load measurement(s) from a source and average if necessary
pub fn load_source(source: &MeasurementSource) -> Result<Curve, Box<dyn Error>> {
    match source {
        MeasurementSource::Single(s) => load_measurement(&s.measurement),
        MeasurementSource::InMemory(curve) => Ok(curve.clone()),
        MeasurementSource::Multiple(m) => {
            if m.measurements.is_empty() {
                return Err("Measurement list is empty".into());
            }

            // Load all curves
            let mut curves = Vec::new();
            for r in &m.measurements {
                match load_measurement(r) {
                    Ok(c) => curves.push(c),
                    Err(e) => {
                        let name = r.path().map(|p| p.display().to_string())
                            .or_else(|| r.name().map(String::from))
                            .unwrap_or_else(|| "inline".to_string());
                        eprintln!("Warning: failed to load measurement {}: {}", name, e)
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