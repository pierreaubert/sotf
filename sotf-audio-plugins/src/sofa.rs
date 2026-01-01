// ============================================================================
// SOFA File Reader - Spatially Oriented Format for Acoustics
// ============================================================================
//
// This module provides functionality to read SOFA files containing HRTF data.
// SOFA is a file format for storing spatially oriented acoustic data, based
// on NetCDF.
// Since loading a SOFA file implies linking with NetCDF, we also support loading
// from a sqlite database. There is a binary doing sofa_2_sqlite.
//
// Primary use case: Reading Head-Related Transfer Functions (HRTFs) for
// binaural audio rendering.
//
// Supported conventions:
// - SimpleFreeFieldHRIR (most common for HRTF datasets like SADIE, KEMAR)
//
// References:
// - https://www.sofaconventions.org/
// - AES69-2015: AES standard for file exchange - Spatial acoustic data file format

use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

// ============================================================================
// Types and Structures
// ============================================================================

/// Coordinate system types defined by SOFA specification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordinateSystem {
    /// Spherical coordinates (azimuth, elevation, radius)
    Spherical,
    /// Cartesian coordinates (x, y, z)
    Cartesian,
}

/// Source position in spherical coordinates
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SourcePosition {
    /// Azimuth in degrees (-180 to 180)
    pub azimuth: f32,
    /// Elevation in degrees (-90 to 90)
    pub elevation: f32,
    /// Distance in meters
    pub distance: f32,
}

impl SourcePosition {
    /// Create a new source position
    pub fn new(azimuth: f32, elevation: f32, distance: f32) -> Self {
        Self {
            azimuth,
            elevation,
            distance,
        }
    }

    /// Calculate angular distance between two positions (in degrees)
    /// Uses great circle distance for azimuth/elevation
    pub fn angular_distance(&self, other: &SourcePosition) -> f32 {
        let az1 = self.azimuth.to_radians();
        let el1 = self.elevation.to_radians();
        let az2 = other.azimuth.to_radians();
        let el2 = other.elevation.to_radians();

        // Haversine formula for great circle distance
        let dlat = el2 - el1;
        let dlon = az2 - az1;

        let a = (dlat / 2.0).sin().powi(2) + el1.cos() * el2.cos() * (dlon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        c.to_degrees()
    }

    /// Convert to Cartesian unit vector (x, y, z)
    /// Coordinate system: x=front, y=left, z=up
    pub fn to_cartesian_unit_vector(&self) -> [f32; 3] {
        let az = self.azimuth.to_radians();
        let el = self.elevation.to_radians();

        // Standard conversion
        // x = cos(el) * cos(az)
        // y = cos(el) * sin(az)
        // z = sin(el)

        let x = el.cos() * az.cos();
        let y = el.cos() * az.sin();
        let z = el.sin();

        [x, y, z]
    }
}

/// HRTF data for a single source position
#[derive(Debug, Clone)]
pub struct HrtfData {
    /// Source position
    pub position: SourcePosition,
    /// Left ear impulse response
    pub ir_left: Vec<f32>,
    /// Right ear impulse response
    pub ir_right: Vec<f32>,
}

/// SOFA file reader for HRTF data
#[derive(Clone)]
pub struct SofaFile {
    /// Sample rate in Hz
    pub sample_rate: f32,
    /// Number of source positions (measurements)
    pub num_measurements: usize,
    /// Length of each impulse response in samples
    pub ir_length: usize,
    /// Source positions
    pub positions: Vec<SourcePosition>,
    /// All HRTF impulse responses [M × 2 × N]
    /// M = measurements, 2 = left/right ears, N = ir_length
    pub impulse_responses: Vec<f32>,
    /// Convention used in SOFA file
    pub convention: String,
    /// Data sampling rate (from SOFA file)
    pub data_sample_rate: Option<f32>,
}

impl SofaFile {
    /// Load HRTF data from a .hrtfdb (SQLite) file
    pub fn load_sqlite<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let path_ref = path.as_ref();
        let conn = Connection::open(path_ref).map_err(|e| {
            format!(
                "Failed to open SQLite HRTF database '{}': {}",
                path_ref.display(),
                e
            )
        })?;

        let mut stmt = conn
            .prepare("SELECT key, value FROM metadata")
            .map_err(|e| e.to_string())?;
        let metadata_iter = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;

        let mut metadata = std::collections::HashMap::new();
        for item in metadata_iter {
            let (key, value) = item.map_err(|e| e.to_string())?;
            metadata.insert(key, value);
        }

        let convention = metadata
            .get("convention")
            .ok_or("Missing 'convention' in metadata")?
            .clone();
        let sample_rate = metadata
            .get("sample_rate")
            .ok_or("Missing 'sample_rate'")?
            .parse::<f32>()
            .map_err(|e| e.to_string())?;
        let ir_length = metadata
            .get("ir_length")
            .ok_or("Missing 'ir_length'")?
            .parse::<usize>()
            .map_err(|e| e.to_string())?;
        let num_measurements = metadata
            .get("num_measurements")
            .ok_or("Missing 'num_measurements'")?
            .parse::<usize>()
            .map_err(|e| e.to_string())?;
        let data_sample_rate = metadata
            .get("data_sample_rate")
            .and_then(|s| s.parse::<f32>().ok());

        let positions: Vec<SourcePosition> = {
            let blob: Vec<u8> = conn
                .query_row(
                    "SELECT value FROM data WHERE key = 'positions'",
                    [],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            bincode::deserialize(&blob)
                .map_err(|e| format!("Failed to deserialize positions: {}", e))?
        };

        let impulse_responses: Vec<f32> = {
            let blob: Vec<u8> = conn
                .query_row(
                    "SELECT value FROM data WHERE key = 'impulse_responses'",
                    [],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            blob.chunks_exact(4)
                .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
                .collect()
        };

        Ok(Self {
            sample_rate,
            num_measurements,
            ir_length,
            positions,
            impulse_responses,
            convention,
            data_sample_rate,
        })
    }

    /// Load a SOFA file from disk
    ///
    /// # Arguments
    /// * `path` - Path to the SOFA file
    ///
    /// # Returns
    /// * `Ok(SofaFile)` - Successfully loaded SOFA data
    /// * `Err(String)` - Error message if loading failed
    #[cfg(feature = "sofa_support")]
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let path_ref = path.as_ref();
        let file = netcdf::open(path_ref)
            .map_err(|e| format!("Failed to open SOFA file '{}': {}", path_ref.display(), e))?;

        // Read global attributes
        let convention = Self::read_string_attr(&file, "SOFAConventions")?;
        log::debug!("[SOFA] Convention: {}", convention);

        // Read dimensions
        let m_dim = file
            .dimension("M")
            .ok_or("Missing dimension 'M' (measurements)")?;
        let n_dim = file
            .dimension("N")
            .ok_or("Missing dimension 'N' (samples)")?;
        let r_dim = file
            .dimension("R")
            .ok_or("Missing dimension 'R' (receivers)")?;

        let num_measurements = m_dim.len();
        let ir_length = n_dim.len();
        let num_receivers = r_dim.len();

        log::info!(
            "[SOFA] Dimensions: M={}, R={}, N={}",
            num_measurements,
            num_receivers,
            ir_length
        );

        if num_receivers != 2 {
            return Err(format!(
                "Expected 2 receivers (left/right ears), got {}",
                num_receivers
            ));
        }

        // Read sample rate
        let sample_rate = Self::read_sample_rate(&file)?;
        log::debug!("[SOFA] Sample rate: {} Hz", sample_rate);

        // Read source positions
        let positions = Self::read_source_positions(&file, num_measurements)?;
        log::debug!("[SOFA] Loaded {} source positions", positions.len());

        // Read impulse responses
        // Data.IR has shape [M, R, N] = [measurements, receivers, samples]
        let ir_var = file
            .variable("Data.IR")
            .ok_or("Missing variable 'Data.IR'")?;

        let ir_data: Vec<f32> = ir_var
            .get_values(..)
            .map_err(|e| format!("Failed to read IR data: {}", e))?;

        log::info!(
            "[SOFA] Loaded {} IR samples ({}×{}×{})",
            ir_data.len(),
            num_measurements,
            num_receivers,
            ir_length
        );

        Ok(Self {
            sample_rate,
            num_measurements,
            ir_length,
            positions,
            impulse_responses: ir_data,
            convention,
            data_sample_rate: Some(sample_rate),
        })
    }

    /// Get HRTF data for a specific measurement index
    ///
    /// # Arguments
    /// * `index` - Measurement index (0..num_measurements)
    ///
    /// # Returns
    /// * `Some(HrtfData)` - HRTF data if index is valid
    /// * `None` - If index is out of bounds
    pub fn get_hrtf(&self, index: usize) -> Option<HrtfData> {
        if index >= self.num_measurements {
            return None;
        }

        let position = self.positions[index];

        // Extract IR for this measurement
        // Data layout: [M, R, N] where M=measurement, R=receiver (0=left, 1=right), N=samples
        let offset = index * 2 * self.ir_length;

        let ir_left = self.impulse_responses[offset..offset + self.ir_length].to_vec();
        let ir_right =
            self.impulse_responses[offset + self.ir_length..offset + 2 * self.ir_length].to_vec();

        Some(HrtfData {
            position,
            ir_left,
            ir_right,
        })
    }

    /// Find the nearest HRTF measurement for a given source position
    ///
    /// # Arguments
    /// * `target` - Target source position
    ///
    /// # Returns
    /// * Tuple of (index, distance) for the nearest measurement
    pub fn find_nearest(&self, target: &SourcePosition) -> (usize, f32) {
        let mut min_dist = f32::MAX;
        let mut min_idx = 0;

        for (i, pos) in self.positions.iter().enumerate() {
            let dist = pos.angular_distance(target);
            if dist < min_dist {
                min_dist = dist;
                min_idx = i;
            }
        }

        (min_idx, min_dist)
    }

    /// Find the 3 nearest HRTF measurements for a given source position
    ///
    /// # Arguments
    /// * `target` - Target source position
    ///
    /// # Returns
    /// * Array of 3 tuples (index, distance) sorted by distance
    pub fn find_three_nearest(&self, target: &SourcePosition) -> [(usize, f32); 3] {
        let mut candidates: Vec<(usize, f32)> = self
            .positions
            .iter()
            .enumerate()
            .map(|(i, pos)| (i, pos.angular_distance(target)))
            .collect();

        // Sort by distance
        // Note: partial_cmp can fail for NaN, but distances shouldn't be NaN
        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Return top 3
        [
            candidates[0],
            candidates.get(1).cloned().unwrap_or(candidates[0]),
            candidates.get(2).cloned().unwrap_or(candidates[0]),
        ]
    }

    /// Get HRTF for a specific position (uses nearest neighbor for now)
    ///
    /// # Arguments
    /// * `position` - Desired source position
    ///
    /// # Returns
    /// * `Some(HrtfData)` - HRTF data for nearest measurement
    /// * `None` - If no measurements available
    pub fn get_hrtf_at_position(&self, position: &SourcePosition) -> Option<HrtfData> {
        if self.num_measurements == 0 {
            return None;
        }

        let (index, _dist) = self.find_nearest(position);
        self.get_hrtf(index)
    }

    /// Get all available source positions
    pub fn get_positions(&self) -> &[SourcePosition] {
        &self.positions
    }

    // ========================================================================
    // Private helper methods
    // ========================================================================

    /// Read a string attribute from NetCDF file
    #[cfg(feature = "sofa_support")]
    fn read_string_attr(file: &netcdf::File, name: &str) -> Result<String, String> {
        match file.attribute(name) {
            Some(attr) => {
                let value = attr
                    .value()
                    .map_err(|e| format!("Failed to read attribute '{}': {}", name, e))?;

                // Handle different attribute value types
                match value {
                    netcdf::AttributeValue::Str(s) => Ok(s),
                    netcdf::AttributeValue::Uchars(v) => {
                        // Sometimes strings are stored as byte arrays
                        String::from_utf8(v)
                            .map_err(|e| format!("Invalid UTF-8 in attribute '{}': {}", name, e))
                    }
                    _ => Err(format!("Attribute '{}' is not a string", name)),
                }
            }
            None => Err(format!("Missing attribute '{}'", name)),
        }
    }

    /// Read sample rate from SOFA file
    #[cfg(feature = "sofa_support")]
    fn read_sample_rate(file: &netcdf::File) -> Result<f32, String> {
        // Try Data.SamplingRate variable first
        if let Some(var) = file.variable("Data.SamplingRate") {
            let values: Vec<f32> = var
                .get_values(..)
                .map_err(|e| format!("Failed to read Data.SamplingRate: {}", e))?;
            if !values.is_empty() {
                return Ok(values[0]);
            }
        }

        // Fallback to attribute
        match file.attribute("Data.SamplingRate") {
            Some(attr) => {
                let value = attr
                    .value()
                    .map_err(|e| format!("Failed to read Data.SamplingRate attribute: {}", e))?;

                match value {
                    netcdf::AttributeValue::Doubles(v) if !v.is_empty() => Ok(v[0] as f32),
                    netcdf::AttributeValue::Double(v) => Ok(v as f32),
                    netcdf::AttributeValue::Floats(v) if !v.is_empty() => Ok(v[0]),
                    netcdf::AttributeValue::Float(v) => Ok(v),
                    netcdf::AttributeValue::Ints(v) if !v.is_empty() => Ok(v[0] as f32),
                    netcdf::AttributeValue::Int(v) => Ok(v as f32),
                    _ => Err("Data.SamplingRate attribute has unexpected type".to_string()),
                }
            }
            None => Err("Missing Data.SamplingRate".to_string()),
        }
    }

    /// Read source positions from SOFA file
    #[cfg(feature = "sofa_support")]
    fn read_source_positions(
        file: &netcdf::File,
        num_measurements: usize,
    ) -> Result<Vec<SourcePosition>, String> {
        // Read SourcePosition variable [M, C] where C=3 (coordinates)
        let pos_var = file
            .variable("SourcePosition")
            .ok_or("Missing variable 'SourcePosition'")?;

        let pos_data: Vec<f32> = pos_var
            .get_values(..)
            .map_err(|e| format!("Failed to read SourcePosition: {}", e))?;

        if pos_data.len() != num_measurements * 3 {
            return Err(format!(
                "SourcePosition data size mismatch: expected {}, got {}",
                num_measurements * 3,
                pos_data.len()
            ));
        }

        // Determine coordinate system
        let coord_system = match file.attribute("SourcePosition:Type") {
            Some(attr) => {
                let value = attr.value().ok();
                match value {
                    Some(netcdf::AttributeValue::Str(s)) if s.contains("spherical") => {
                        CoordinateSystem::Spherical
                    }
                    Some(netcdf::AttributeValue::Str(s)) if s.contains("cartesian") => {
                        CoordinateSystem::Cartesian
                    }
                    _ => CoordinateSystem::Spherical, // Default assumption
                }
            }
            None => CoordinateSystem::Spherical, // Default assumption
        };

        log::debug!("[SOFA] Coordinate system: {:?}", coord_system);

        // Parse positions
        let mut positions = Vec::with_capacity(num_measurements);
        for i in 0..num_measurements {
            let idx = i * 3;
            let pos = match coord_system {
                CoordinateSystem::Spherical => {
                    // Coordinates are [azimuth, elevation, distance]
                    SourcePosition::new(pos_data[idx], pos_data[idx + 1], pos_data[idx + 2])
                }
                CoordinateSystem::Cartesian => {
                    // Convert Cartesian (x, y, z) to spherical
                    let (x, y, z) = (pos_data[idx], pos_data[idx + 1], pos_data[idx + 2]);
                    let distance = (x * x + y * y + z * z).sqrt();
                    let azimuth = y.atan2(x).to_degrees();
                    let elevation = (z / distance).asin().to_degrees();
                    SourcePosition::new(azimuth, elevation, distance)
                }
            };
            positions.push(pos);
        }

        Ok(positions)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_position_distance() {
        let pos1 = SourcePosition::new(0.0, 0.0, 1.0);
        let pos2 = SourcePosition::new(90.0, 0.0, 1.0);

        let dist = pos1.angular_distance(&pos2);
        assert!(
            (dist - 90.0).abs() < 0.1,
            "Expected ~90 degrees, got {}",
            dist
        );
    }

    #[test]
    fn test_source_position_same() {
        let pos1 = SourcePosition::new(45.0, 30.0, 1.5);
        let pos2 = SourcePosition::new(45.0, 30.0, 1.5);

        let dist = pos1.angular_distance(&pos2);
        assert!(dist < 0.01, "Expected ~0 degrees, got {}", dist);
    }
}
