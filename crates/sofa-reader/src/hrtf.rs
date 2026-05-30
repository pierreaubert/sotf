//! Higher-level HRTF helpers built on top of the low-level SOFA reader.
//!
//! This module covers the subset of SOFA used by SOTF's binaural and CTC
//! paths: SimpleFreeFieldHRIR-style files with two receiver channels and
//! nearest-position lookup for source azimuth/elevation/distance.

use crate::{Result, SofaError, SofaWriter};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Coordinate system types defined by the SOFA specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordinateSystem {
    /// Spherical coordinates (azimuth, elevation, radius).
    Spherical,
    /// Cartesian coordinates (x, y, z).
    Cartesian,
}

/// Source position in spherical coordinates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SourcePosition {
    /// Azimuth in degrees (-180 to 180).
    pub azimuth: f32,
    /// Elevation in degrees (-90 to 90).
    pub elevation: f32,
    /// Distance in meters.
    pub distance: f32,
}

impl SourcePosition {
    /// Create a new source position.
    pub fn new(azimuth: f32, elevation: f32, distance: f32) -> Self {
        Self {
            azimuth,
            elevation,
            distance,
        }
    }

    /// Calculate the great-circle (angular) distance between two positions, in degrees.
    ///
    /// Uses the haversine formula on the unit sphere, mapping (azimuth, elevation)
    /// to (longitude, latitude). The `distance` (radius) component is
    /// intentionally ignored — two positions in the same direction at different
    /// radii return zero. Callers that handle near-field measurements must
    /// branch on `distance` separately.
    pub fn angular_distance(&self, other: &SourcePosition) -> f32 {
        let az1 = self.azimuth.to_radians();
        let el1 = self.elevation.to_radians();
        let az2 = other.azimuth.to_radians();
        let el2 = other.elevation.to_radians();

        let dlat = el2 - el1;
        let dlon = az2 - az1;

        // Haversine: a = sin²(Δφ/2) + cos φ1 · cos φ2 · sin²(Δλ/2)
        let a = (dlat / 2.0).sin().powi(2) + el1.cos() * el2.cos() * (dlon / 2.0).sin().powi(2);
        // Clamp to [0, 1] to guard against tiny floating-point overshoot that
        // would make `sqrt(1 - a)` NaN at near-antipodes.
        let a = a.clamp(0.0, 1.0);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        c.to_degrees()
    }

    /// Convert to a Cartesian unit vector using x=front, y=left, z=up.
    pub fn to_cartesian_unit_vector(&self) -> [f32; 3] {
        let az = self.azimuth.to_radians();
        let el = self.elevation.to_radians();

        let x = el.cos() * az.cos();
        let y = el.cos() * az.sin();
        let z = el.sin();

        [x, y, z]
    }

    /// Build a spherical position from a Cartesian `[x, y, z]` triple in metres.
    ///
    /// Uses x=front, y=left, z=up. Returns `SourcePosition { 0, 0, 0 }` when
    /// the input is the origin (otherwise `atan2(0, 0) == 0` plus
    /// `asin(z/0) == NaN` would poison nearest-neighbour search).
    pub fn from_cartesian(x: f32, y: f32, z: f32) -> Self {
        let distance = (x * x + y * y + z * z).sqrt();
        if !distance.is_finite() || distance <= f32::EPSILON {
            return Self::new(0.0, 0.0, 0.0);
        }
        let azimuth = y.atan2(x).to_degrees();
        let elevation = (z / distance).clamp(-1.0, 1.0).asin().to_degrees();
        Self::new(azimuth, elevation, distance)
    }
}

/// HRTF data for a single source position.
#[derive(Debug, Clone)]
pub struct HrtfData {
    /// Source position.
    pub position: SourcePosition,
    /// Left-ear impulse response.
    pub ir_left: Vec<f32>,
    /// Right-ear impulse response.
    pub ir_right: Vec<f32>,
}

/// Inputs for a SimpleFreeFieldHRTF SOFA file in the frequency domain.
#[derive(Debug, Clone)]
pub struct SimpleFreeFieldHrtf<'a> {
    /// Real part, row-major shape `[M, R, N frequencies]`.
    pub real: &'a [f64],
    /// Imaginary part, row-major shape `[M, R, N frequencies]`.
    pub imag: &'a [f64],
    /// Frequency bins in Hz, length `N`.
    pub frequencies: &'a [f64],
    /// Source positions, row-major shape `[M, 3]`.
    pub source_position: &'a [f64],
    /// Source coordinate system.
    pub source_coords: CoordinateSystem,
    /// Receiver positions, row-major shape `[R, 3]`.
    pub receiver_position: &'a [f64],
    /// Receiver coordinate system.
    pub receiver_coords: CoordinateSystem,
    /// Listener position `[x, y, z]` in metres. Defaults to origin.
    pub listener_position: Option<[f64; 3]>,
    /// Optional title attribute.
    pub title: Option<&'a str>,
    /// Number of source/evaluation positions (`M`).
    pub measurements: usize,
    /// Number of receivers (`R`).
    pub receivers: usize,
}

/// Write a SimpleFreeFieldHRTF SOFA file.
pub fn write_simple_free_field_hrtf<P: AsRef<Path>>(
    path: P,
    h: &SimpleFreeFieldHrtf<'_>,
) -> Result<()> {
    validate_simple_free_field_hrtf(h)?;

    let n = h.frequencies.len();
    let mut writer = SofaWriter::new();
    write_common_global_attributes(
        &mut writer,
        "SimpleFreeFieldHRTF",
        "TF",
        h.title.unwrap_or("HRTF"),
    );

    writer.add_dimension("M", h.measurements);
    writer.add_dimension("R", h.receivers);
    writer.add_dimension("N", n);
    writer.add_dimension("C", 3);
    writer.add_dimension("I", 1);

    writer.add_variable_f64("Data.Real", &["M", "R", "N"]);
    writer.write_f64("Data.Real", h.real)?;
    writer.add_variable_f64("Data.Imag", &["M", "R", "N"]);
    writer.write_f64("Data.Imag", h.imag)?;
    writer.add_variable_f64("N", &["N"]);
    writer.write_f64("N", h.frequencies)?;
    writer.add_variable_attribute_str("N", "Units", "hertz");

    writer.add_variable_f64("SourcePosition", &["M", "C"]);
    writer.write_f64("SourcePosition", h.source_position)?;
    add_coordinate_attributes(&mut writer, "SourcePosition", h.source_coords);

    let receiver_position = receiver_position_sofa_layout(h.receiver_position, h.receivers);
    writer.add_variable_f64("ReceiverPosition", &["R", "C", "I"]);
    writer.write_f64("ReceiverPosition", &receiver_position)?;
    add_coordinate_attributes(&mut writer, "ReceiverPosition", h.receiver_coords);

    let listener_position = h.listener_position.unwrap_or([0.0, 0.0, 0.0]);
    writer.add_variable_f64("ListenerPosition", &["I", "C"]);
    writer.write_f64("ListenerPosition", &listener_position)?;
    add_coordinate_attributes(&mut writer, "ListenerPosition", CoordinateSystem::Cartesian);

    writer.add_variable_f64("EmitterPosition", &["C", "I"]);
    writer.write_f64("EmitterPosition", &[0.0, 0.0, 0.0])?;
    add_coordinate_attributes(&mut writer, "EmitterPosition", CoordinateSystem::Cartesian);

    writer.finish(path)
}

fn validate_simple_free_field_hrtf(h: &SimpleFreeFieldHrtf<'_>) -> Result<()> {
    let expected_data = h.measurements * h.receivers * h.frequencies.len();
    if h.real.len() != expected_data {
        return Err(SofaError::InvalidStructure(format!(
            "Data.Real length {} does not match M*R*N {}",
            h.real.len(),
            expected_data
        )));
    }
    if h.imag.len() != expected_data {
        return Err(SofaError::InvalidStructure(format!(
            "Data.Imag length {} does not match M*R*N {}",
            h.imag.len(),
            expected_data
        )));
    }
    let expected_sources = h.measurements * 3;
    if h.source_position.len() != expected_sources {
        return Err(SofaError::InvalidStructure(format!(
            "SourcePosition length {} does not match M*C {}",
            h.source_position.len(),
            expected_sources
        )));
    }
    let expected_receivers = h.receivers * 3;
    if h.receiver_position.len() != expected_receivers {
        return Err(SofaError::InvalidStructure(format!(
            "ReceiverPosition length {} does not match R*C {}",
            h.receiver_position.len(),
            expected_receivers
        )));
    }
    Ok(())
}

fn write_common_global_attributes(
    writer: &mut SofaWriter,
    convention: &str,
    data_type: &str,
    title: &str,
) {
    writer.add_attribute_str("Conventions", "SOFA");
    writer.add_attribute_str("Version", "2.1");
    writer.add_attribute_str("SOFAConventions", convention);
    writer.add_attribute_str("SOFAConventionsVersion", "1.0");
    writer.add_attribute_str("APIName", "sofa-reader");
    writer.add_attribute_str("APIVersion", env!("CARGO_PKG_VERSION"));
    writer.add_attribute_str("AuthorContact", "");
    writer.add_attribute_str("Comment", "");
    writer.add_attribute_str("DataType", data_type);
    writer.add_attribute_str("History", "Created by sofa-reader");
    writer.add_attribute_str("License", "No license specified, use as is");
    writer.add_attribute_str("Organization", "");
    writer.add_attribute_str("References", "");
    writer.add_attribute_str("RoomType", "free field");
    writer.add_attribute_str("Origin", "SOFA export");
    writer.add_attribute_str("Title", title);
}

fn add_coordinate_attributes(writer: &mut SofaWriter, variable: &str, coords: CoordinateSystem) {
    writer.add_variable_attribute_str(variable, "Type", coordinate_type(coords));
    writer.add_variable_attribute_str(variable, "Units", coordinate_units(coords));
}

fn coordinate_type(coords: CoordinateSystem) -> &'static str {
    match coords {
        CoordinateSystem::Cartesian => "cartesian",
        CoordinateSystem::Spherical => "spherical",
    }
}

fn coordinate_units(coords: CoordinateSystem) -> &'static str {
    match coords {
        CoordinateSystem::Cartesian => "metre",
        CoordinateSystem::Spherical => "degree, degree, metre",
    }
}

fn receiver_position_sofa_layout(receiver_position: &[f64], receivers: usize) -> Vec<f64> {
    debug_assert_eq!(receiver_position.len(), receivers * 3);
    receiver_position.to_vec()
}

/// SOFA/HRTF file data loaded into memory.
#[derive(Clone)]
pub struct SofaFile {
    /// Sample rate in Hz.
    pub sample_rate: f32,
    /// Number of source positions.
    pub num_measurements: usize,
    /// Length of each impulse response in samples.
    pub ir_length: usize,
    /// Source positions.
    pub positions: Vec<SourcePosition>,
    /// All HRTF impulse responses [M x 2 x N].
    pub impulse_responses: Vec<f32>,
    /// SOFA convention used by the file.
    pub convention: String,
    /// Data sampling rate from the SOFA file, when available.
    pub data_sample_rate: Option<f32>,
}

impl SofaFile {
    /// Load HRTF data from a SOFA file or `.hrtfdb` SQLite cache.
    pub fn load<P: AsRef<Path>>(path: P) -> std::result::Result<Self, String> {
        let path_ref = path.as_ref();
        let ext = path_ref.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext {
            "hrtfdb" | "sqlite" | "db" => Self::load_sqlite(path_ref),
            _ => Self::load_sofa(path_ref),
        }
    }

    /// Load HRTF data from a `.hrtfdb` SQLite cache.
    pub fn load_sqlite<P: AsRef<Path>>(path: P) -> std::result::Result<Self, String> {
        let path_ref = path.as_ref();
        let conn = rusqlite::Connection::open(path_ref).map_err(|e| {
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
            bincode::serde::decode_from_slice(&blob, bincode::config::standard())
                .map(|(value, _)| value)
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

    fn load_sofa(path: &Path) -> std::result::Result<Self, String> {
        let reader = crate::SofaReader::open(path)
            .map_err(|e| format!("Failed to open SOFA file '{}': {}", path.display(), e))?;

        let convention = reader
            .attribute_string("SOFAConventions")
            .map_err(|e| format!("Missing attribute 'SOFAConventions': {}", e))?;
        log::debug!("[SOFA] Convention: {}", convention);

        let num_measurements = reader
            .dimension("M")
            .map_err(|_| "Missing dimension 'M' (measurements)".to_string())?;
        let ir_length = reader
            .dimension("N")
            .map_err(|_| "Missing dimension 'N' (samples)".to_string())?;
        let num_receivers = reader
            .dimension("R")
            .map_err(|_| "Missing dimension 'R' (receivers)".to_string())?;

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

        let sample_rate = reader
            .read_scalar_f32("Data.SamplingRate")
            .or_else(|_| reader.attribute_f64("Data.SamplingRate").map(|v| v as f32))
            .map_err(|e| format!("Failed to read Data.SamplingRate: {}", e))?;
        log::debug!("[SOFA] Sample rate: {} Hz", sample_rate);

        let pos_data = reader
            .read_f32("SourcePosition")
            .map_err(|e| format!("Failed to read SourcePosition: {}", e))?;

        if pos_data.len() != num_measurements * 3 {
            return Err(format!(
                "SourcePosition data size mismatch: expected {}, got {}",
                num_measurements * 3,
                pos_data.len()
            ));
        }

        let coord_system = match reader.attribute("SourcePosition:Type") {
            Some(crate::AttrValue::String(s)) if s.contains("cartesian") => {
                CoordinateSystem::Cartesian
            }
            _ => CoordinateSystem::Spherical,
        };
        log::debug!("[SOFA] Coordinate system: {:?}", coord_system);

        let mut positions = Vec::with_capacity(num_measurements);
        for i in 0..num_measurements {
            let idx = i * 3;
            let pos = match coord_system {
                CoordinateSystem::Spherical => {
                    SourcePosition::new(pos_data[idx], pos_data[idx + 1], pos_data[idx + 2])
                }
                CoordinateSystem::Cartesian => {
                    let (x, y, z) = (pos_data[idx], pos_data[idx + 1], pos_data[idx + 2]);
                    SourcePosition::from_cartesian(x, y, z)
                }
            };
            positions.push(pos);
        }
        log::debug!("[SOFA] Loaded {} source positions", positions.len());

        let ir_data = reader
            .read_f32("Data.IR")
            .map_err(|e| format!("Failed to read IR data: {}", e))?;

        let expected_ir_len = num_measurements
            .checked_mul(num_receivers)
            .and_then(|v| v.checked_mul(ir_length))
            .ok_or_else(|| {
                format!(
                    "SOFA dimensions overflow: M={}, R={}, N={}",
                    num_measurements, num_receivers, ir_length
                )
            })?;
        if ir_data.len() != expected_ir_len {
            return Err(format!(
                "Data.IR size mismatch: expected M*R*N = {}, got {}",
                expected_ir_len,
                ir_data.len()
            ));
        }

        log::info!(
            "[SOFA] Loaded {} IR samples ({}x{}x{})",
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

    /// Get HRTF data for a measurement index.
    ///
    /// Allocates two new `Vec<f32>` for the impulse responses. Use
    /// [`Self::get_hrtf_slices`] or [`Self::get_hrtf_into`] in audio-thread
    /// paths where allocation is forbidden.
    pub fn get_hrtf(&self, index: usize) -> Option<HrtfData> {
        let (position, left, right) = self.get_hrtf_slices(index)?;
        Some(HrtfData {
            position,
            ir_left: left.to_vec(),
            ir_right: right.to_vec(),
        })
    }

    /// Borrow the left/right impulse responses for a measurement index.
    /// Returns `(position, ir_left, ir_right)` borrowed directly from the
    /// internal `impulse_responses` buffer — no allocation, audio-thread safe.
    pub fn get_hrtf_slices(&self, index: usize) -> Option<(SourcePosition, &[f32], &[f32])> {
        if index >= self.num_measurements {
            return None;
        }
        let position = self.positions[index];
        let offset = index * 2 * self.ir_length;
        let left = &self.impulse_responses[offset..offset + self.ir_length];
        let right = &self.impulse_responses[offset + self.ir_length..offset + 2 * self.ir_length];
        Some((position, left, right))
    }

    /// Copy the left/right impulse responses for a measurement index into
    /// caller-supplied buffers. Returns the position on success. Each output
    /// buffer must have at least `ir_length()` elements; only that prefix is
    /// overwritten.
    pub fn get_hrtf_into(
        &self,
        index: usize,
        left_out: &mut [f32],
        right_out: &mut [f32],
    ) -> Option<SourcePosition> {
        let (position, left, right) = self.get_hrtf_slices(index)?;
        if left_out.len() < left.len() || right_out.len() < right.len() {
            return None;
        }
        left_out[..left.len()].copy_from_slice(left);
        right_out[..right.len()].copy_from_slice(right);
        Some(position)
    }

    /// Find the nearest HRTF measurement for a source position.
    pub fn find_nearest(&self, target: &SourcePosition) -> (usize, f32) {
        if self.positions.is_empty() {
            return (0, f32::INFINITY);
        }
        let mut min_dist = f32::MAX;
        let mut min_idx = 0;

        for (i, pos) in self.positions.iter().enumerate() {
            let dist = Self::lookup_distance(pos, target);
            if dist < min_dist {
                min_dist = dist;
                min_idx = i;
            }
        }

        (min_idx, min_dist)
    }

    /// Find the three nearest HRTF measurements for a source position.
    pub fn find_three_nearest(&self, target: &SourcePosition) -> [(usize, f32); 3] {
        if self.positions.is_empty() {
            return [(0, f32::INFINITY); 3];
        }
        let mut best = [(0usize, f32::INFINITY); 3];
        for (i, pos) in self.positions.iter().enumerate() {
            let dist = Self::lookup_distance(pos, target);
            if dist < best[0].1 {
                best[2] = best[1];
                best[1] = best[0];
                best[0] = (i, dist);
            } else if dist < best[1].1 {
                best[2] = best[1];
                best[1] = (i, dist);
            } else if dist < best[2].1 {
                best[2] = (i, dist);
            }
        }
        if self.positions.len() == 1 {
            best[1] = best[0];
            best[2] = best[0];
        } else if self.positions.len() == 2 {
            best[2] = best[1];
        }
        best
    }

    fn lookup_distance(a: &SourcePosition, b: &SourcePosition) -> f32 {
        let angular_deg = a.angular_distance(b);
        if !angular_deg.is_finite() {
            return f32::INFINITY;
        }
        let radial = (a.distance - b.distance).abs();
        if !radial.is_finite() {
            return f32::INFINITY;
        }
        let mean_r = ((a.distance.abs() + b.distance.abs()) * 0.5).max(1e-3);
        let tangential = mean_r * angular_deg.to_radians();
        (tangential * tangential + radial * radial).sqrt()
    }

    /// Get HRTF data for the nearest available measurement to a position.
    pub fn get_hrtf_at_position(&self, position: &SourcePosition) -> Option<HrtfData> {
        if self.num_measurements == 0 {
            return None;
        }

        let (index, _dist) = self.find_nearest(position);
        self.get_hrtf(index)
    }

    /// Get an HRTF interpolated across the three nearest measurements using
    /// inverse-distance weights, eliminating the audible "snap" of nearest-
    /// neighbor lookup when sources move continuously.
    ///
    /// Falls back to the nearest measurement if any of the three lookups
    /// collapse onto the same index. Returns `None` if the database is empty.
    pub fn get_hrtf_interpolated(&self, position: &SourcePosition) -> Option<HrtfData> {
        if self.num_measurements == 0 {
            return None;
        }
        let nbrs = self.find_three_nearest(position);
        // Degenerate case: only one unique neighbor — return it directly.
        if nbrs[0].0 == nbrs[1].0 && nbrs[1].0 == nbrs[2].0 {
            return self.get_hrtf(nbrs[0].0);
        }

        // Inverse-distance weights with epsilon to avoid div-by-zero when a
        // neighbour lies exactly on the query point. Normalise so weights sum
        // to 1.
        const EPS: f32 = 1e-4;
        let mut weights = [0.0f32; 3];
        let mut total = 0.0f32;
        for (slot, (_idx, dist)) in weights.iter_mut().zip(nbrs.iter()) {
            let w = 1.0 / (*dist + EPS);
            *slot = w;
            total += w;
        }
        if total <= 0.0 {
            return self.get_hrtf(nbrs[0].0);
        }
        for w in weights.iter_mut() {
            *w /= total;
        }

        let position_out = self.positions[nbrs[0].0];
        let mut ir_left = vec![0.0f32; self.ir_length];
        let mut ir_right = vec![0.0f32; self.ir_length];
        for ((idx, _dist), w) in nbrs.iter().zip(weights.iter()) {
            let (_pos, left, right) = self.get_hrtf_slices(*idx)?;
            for (dst, src) in ir_left.iter_mut().zip(left.iter()) {
                *dst += src * (*w);
            }
            for (dst, src) in ir_right.iter_mut().zip(right.iter()) {
                *dst += src * (*w);
            }
        }
        Some(HrtfData {
            position: position_out,
            ir_left,
            ir_right,
        })
    }

    /// Get all available source positions.
    pub fn get_positions(&self) -> &[SourcePosition] {
        &self.positions
    }
}

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

    #[test]
    fn test_angular_distance_antipodes() {
        let pos1 = SourcePosition::new(0.0, 0.0, 1.0);
        let pos2 = SourcePosition::new(180.0, 0.0, 1.0);
        let dist = pos1.angular_distance(&pos2);
        assert!(
            (dist - 180.0).abs() < 0.01,
            "Antipodal points should be 180°, got {}",
            dist
        );
        assert!(dist.is_finite(), "Antipodal distance must not be NaN");
    }

    #[test]
    fn test_angular_distance_ninety_degrees() {
        let pos1 = SourcePosition::new(0.0, 90.0, 1.0);
        let pos2 = SourcePosition::new(0.0, 0.0, 1.0);
        let dist = pos1.angular_distance(&pos2);
        assert!(
            (dist - 90.0).abs() < 0.01,
            "Pole-to-equator should be 90°, got {}",
            dist
        );
    }

    #[test]
    fn test_angular_distance_ignores_radius() {
        let pos1 = SourcePosition::new(30.0, 15.0, 0.2);
        let pos2 = SourcePosition::new(30.0, 15.0, 2.0);
        let dist = pos1.angular_distance(&pos2);
        assert!(
            dist < 0.01,
            "Angular distance should ignore radius, got {}",
            dist
        );
    }

    #[test]
    fn test_angular_distance_wrap_around() {
        let pos1 = SourcePosition::new(-179.0, 0.0, 1.0);
        let pos2 = SourcePosition::new(179.0, 0.0, 1.0);
        let dist = pos1.angular_distance(&pos2);
        assert!(
            (dist - 2.0).abs() < 0.01,
            "Azimuth wrap-around should give 2°, got {}",
            dist
        );
    }

    #[test]
    fn test_cartesian_to_spherical_origin() {
        let pos = SourcePosition::from_cartesian(0.0, 0.0, 0.0);
        assert_eq!(pos.azimuth, 0.0);
        assert_eq!(pos.elevation, 0.0);
        assert_eq!(pos.distance, 0.0);
        assert!(pos.azimuth.is_finite());
        assert!(pos.elevation.is_finite());
        assert!(pos.distance.is_finite());
    }

    #[test]
    fn test_cartesian_to_spherical_known_points() {
        let pos = SourcePosition::from_cartesian(1.0, 0.0, 0.0);
        assert!(pos.azimuth.abs() < 0.01);
        assert!(pos.elevation.abs() < 0.01);
        assert!((pos.distance - 1.0).abs() < 0.01);

        let pos = SourcePosition::from_cartesian(0.0, 0.0, 1.0);
        assert!((pos.elevation - 90.0).abs() < 0.01);
        assert!((pos.distance - 1.0).abs() < 0.01);

        let pos = SourcePosition::from_cartesian(0.0, 1.0, 0.0);
        assert!((pos.azimuth - 90.0).abs() < 0.01);
        assert!((pos.distance - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_find_nearest_never_returns_nan() {
        let sf = SofaFile {
            sample_rate: 48000.0,
            num_measurements: 1,
            ir_length: 1,
            positions: vec![SourcePosition::from_cartesian(0.0, 0.0, 0.0)],
            impulse_responses: vec![0.0, 0.0],
            convention: "test".to_string(),
            data_sample_rate: Some(48000.0),
        };
        let (idx, dist) = sf.find_nearest(&SourcePosition::new(45.0, 30.0, 1.0));
        assert_eq!(idx, 0);
        assert!(dist.is_finite(), "find_nearest should never return NaN");
    }

    #[test]
    fn test_find_nearest_considers_radius_when_direction_matches() {
        let sf = SofaFile {
            sample_rate: 48000.0,
            num_measurements: 2,
            ir_length: 1,
            positions: vec![
                SourcePosition::new(30.0, 15.0, 0.25),
                SourcePosition::new(30.0, 15.0, 1.5),
            ],
            impulse_responses: vec![0.0, 0.0, 1.0, 1.0],
            convention: "test".to_string(),
            data_sample_rate: Some(48000.0),
        };
        let (idx, dist) = sf.find_nearest(&SourcePosition::new(30.0, 15.0, 1.45));
        assert_eq!(idx, 1, "Nearest match should favor radius proximity");
        assert!(dist.is_finite());
    }

    #[test]
    fn test_find_three_nearest_returns_sorted_neighbors() {
        let sf = SofaFile {
            sample_rate: 48000.0,
            num_measurements: 4,
            ir_length: 1,
            positions: vec![
                SourcePosition::new(0.0, 0.0, 1.0),
                SourcePosition::new(20.0, 0.0, 1.0),
                SourcePosition::new(40.0, 0.0, 1.0),
                SourcePosition::new(60.0, 0.0, 1.0),
            ],
            impulse_responses: vec![0.0; 8],
            convention: "test".to_string(),
            data_sample_rate: Some(48000.0),
        };
        let nearest = sf.find_three_nearest(&SourcePosition::new(18.0, 0.0, 1.0));
        assert_eq!(nearest[0].0, 1);
        assert!(nearest[0].1 <= nearest[1].1);
        assert!(nearest[1].1 <= nearest[2].1);
    }
}
