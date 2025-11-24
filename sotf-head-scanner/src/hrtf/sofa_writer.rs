// src-head-scanner/src/hrtf/sofa_writer.rs
//
// SOFA (Spatially Oriented Format for Acoustics) file export
//
// SOFA is a netCDF-4 file format (which is based on HDF5) for storing spatial audio data
// Specification: AES69-2022 (SOFA Conventions 2.1)
//
// This module implements the SimpleFreeFieldHRIR convention for storing HRIRs measured
// with an omnidirectional source in free field conditions.
//
// File structure:
// - Data.IR [M, R, N]: Impulse responses (M measurements, R receivers, N samples)
// - Data.SamplingRate: Sampling rate in Hz
// - Data.Delay [M, R]: Delays in samples
// - SourcePosition [M, C]: Source positions (C=3 coordinates)
// - ReceiverPosition [R, C]: Receiver positions (typically 2 ears)
// - ListenerPosition [M, C]: Listener positions
// - ListenerView [M, C]: Listener view direction
// - ListenerUp [M, C]: Listener up direction
// - Global attributes: Version, Conventions, metadata

use crate::hrtf::HrirData;
use anyhow::{Context, Result};
use chrono::Utc;
use ndarray::{Array1, Array2, Array3};

/// Coordinate system type for positions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateSystem {
    /// Cartesian coordinates (x, y, z) in meters
    Cartesian,
    /// Spherical coordinates (azimuth, elevation, radius)
    /// - Azimuth: angle in horizontal plane, 0° = front, 90° = left (degrees)
    /// - Elevation: angle from horizontal plane, 0° = horizontal, 90° = up (degrees)
    /// - Radius: distance from origin (meters)
    Spherical,
}

impl CoordinateSystem {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Cartesian => "cartesian",
            Self::Spherical => "spherical",
        }
    }

    pub fn units(&self) -> &str {
        match self {
            Self::Cartesian => "metre, metre, metre",
            Self::Spherical => "degree, degree, metre",
        }
    }
}

/// Convert Cartesian coordinates (x, y, z) to Spherical (azimuth, elevation, radius)
///
/// # Arguments
/// * `x`, `y`, `z` - Cartesian coordinates in meters
///
/// # Returns
/// * `(azimuth, elevation, radius)` - Spherical coordinates
///   - Azimuth in degrees: 0° = front (+y), 90° = left (+x), 180° = back, -90° = right
///   - Elevation in degrees: 0° = horizontal plane, 90° = up (+z), -90° = down
///   - Radius in meters
pub fn cartesian_to_spherical(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    let radius = (x * x + y * y + z * z).sqrt();

    if radius < 1e-10 {
        // Origin point
        return (0.0, 0.0, 0.0);
    }

    // Azimuth: angle in horizontal plane from +y axis (front)
    // atan2(x, y) gives angle from +y axis, positive counterclockwise
    let azimuth = x.atan2(y).to_degrees();

    // Elevation: angle from horizontal plane
    // asin(z / r) gives elevation
    let elevation = (z / radius).asin().to_degrees();

    (azimuth, elevation, radius)
}

/// Convert Spherical coordinates (azimuth, elevation, radius) to Cartesian (x, y, z)
///
/// # Arguments
/// * `azimuth` - Azimuth angle in degrees (0° = front, 90° = left)
/// * `elevation` - Elevation angle in degrees (0° = horizontal, 90° = up)
/// * `radius` - Distance from origin in meters
///
/// # Returns
/// * `(x, y, z)` - Cartesian coordinates in meters
pub fn spherical_to_cartesian(azimuth: f64, elevation: f64, radius: f64) -> (f64, f64, f64) {
    let az_rad = azimuth.to_radians();
    let el_rad = elevation.to_radians();

    let cos_el = el_rad.cos();
    let x = radius * cos_el * az_rad.sin();
    let y = radius * cos_el * az_rad.cos();
    let z = radius * el_rad.sin();

    (x, y, z)
}

/// Metadata for SOFA file
#[derive(Debug, Clone)]
pub struct SofaMetadata {
    pub title: String,
    pub database_name: Option<String>,
    pub listener_short_name: Option<String>,
    pub author_contact: String,
    pub organization: String,
    pub license: String,
    pub application_name: String,
    pub application_version: String,
    pub comment: Option<String>,
}

impl Default for SofaMetadata {
    fn default() -> Self {
        Self {
            title: "HRTF Data".to_string(),
            database_name: None,
            listener_short_name: None,
            author_contact: "".to_string(),
            organization: "".to_string(),
            license: "No license specified".to_string(),
            application_name: "head-scanner".to_string(),
            application_version: env!("CARGO_PKG_VERSION").to_string(),
            comment: None,
        }
    }
}

/// SOFA file writer for SimpleFreeFieldHRIR convention
pub struct SofaWriter {
    metadata: SofaMetadata,
    coordinate_system: CoordinateSystem,
    room_type: String,
}

impl SofaWriter {
    /// Create a new SOFA writer with default settings
    pub fn new() -> Self {
        Self {
            metadata: SofaMetadata::default(),
            coordinate_system: CoordinateSystem::Spherical,
            room_type: "free field".to_string(),
        }
    }

    /// Set custom metadata
    pub fn with_metadata(mut self, metadata: SofaMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Set coordinate system (default: Spherical)
    pub fn with_coordinate_system(mut self, coord_sys: CoordinateSystem) -> Self {
        self.coordinate_system = coord_sys;
        self
    }

    /// Set room type (default: "free field")
    pub fn with_room_type(mut self, room_type: String) -> Self {
        self.room_type = room_type;
        self
    }

    /// Write HrirData to SOFA file
    ///
    /// # Arguments
    /// * `hrir_data` - HRIR data to export
    /// * `source_positions` - Source positions [M, 3] in meters (cartesian) or [M, 3] (azimuth, elevation, radius)
    /// * `output_path` - Path to output .sofa file
    pub fn write_hrir(
        &self,
        hrir_data: &HrirData,
        source_positions: &Array2<f64>,
        output_path: &str,
    ) -> Result<()> {
        let num_measurements = hrir_data.num_points();
        let num_samples = hrir_data.num_samples();
        let num_receivers = 2; // Stereo (left and right ear)

        // Validate dimensions
        if source_positions.nrows() != num_measurements {
            anyhow::bail!(
                "Source positions ({}) must match number of measurements ({})",
                source_positions.nrows(),
                num_measurements
            );
        }
        if source_positions.ncols() != 3 {
            anyhow::bail!("Source positions must have 3 coordinates");
        }

        // Create netCDF file
        let mut file = netcdf::create(output_path).context("Failed to create SOFA file")?;

        // Write global attributes
        self.write_global_attributes(&mut file)?;

        // Define dimensions
        file.add_dimension("M", num_measurements)?;
        file.add_dimension("R", num_receivers)?;
        file.add_dimension("N", num_samples)?;
        file.add_dimension("C", 3)?;
        file.add_dimension("E", 1)?;

        // Write Data group variables (dimensions will be looked up by name)
        self.write_data_vars_hrir(&mut file, hrir_data)?;

        // Write position variables
        self.write_position_vars(&mut file, source_positions, num_measurements, num_receivers)?;

        Ok(())
    }

    /// Write global SOFA attributes
    fn write_global_attributes(&self, file: &mut netcdf::FileMut) -> Result<()> {
        // SOFA version and convention
        file.add_attribute("Conventions", "SOFA")?;
        file.add_attribute("Version", "2.1")?;
        file.add_attribute("SOFAConventions", "SimpleFreeFieldHRIR")?;
        file.add_attribute("SOFAConventionsVersion", "1.0")?;

        // API information
        file.add_attribute("APIName", self.metadata.application_name.as_str())?;
        file.add_attribute("APIVersion", self.metadata.application_version.as_str())?;

        // Data type
        file.add_attribute("DataType", "FIR")?;

        // Room type
        file.add_attribute("RoomType", self.room_type.as_str())?;

        // Dates
        let now = Utc::now().to_rfc3339();
        file.add_attribute("DateCreated", now.as_str())?;
        file.add_attribute("DateModified", now.as_str())?;

        // User metadata
        file.add_attribute("Title", self.metadata.title.as_str())?;

        if let Some(ref db_name) = self.metadata.database_name {
            file.add_attribute("DatabaseName", db_name.as_str())?;
        }

        if let Some(ref listener_name) = self.metadata.listener_short_name {
            file.add_attribute("ListenerShortName", listener_name.as_str())?;
        }

        file.add_attribute("AuthorContact", self.metadata.author_contact.as_str())?;
        file.add_attribute("Organization", self.metadata.organization.as_str())?;
        file.add_attribute("License", self.metadata.license.as_str())?;

        if let Some(ref comment) = self.metadata.comment {
            file.add_attribute("Comment", comment.as_str())?;
        }

        Ok(())
    }

    /// Write Data.* variables with HRIR data
    fn write_data_vars_hrir(&self, file: &mut netcdf::FileMut, hrir_data: &HrirData) -> Result<()> {
        let num_points = hrir_data.num_points();
        let num_samples = hrir_data.num_samples();

        // Prepare Data.IR array [M, R, N]
        let mut ir_data = Array3::<f64>::zeros((num_points, 2, num_samples));

        // Copy impulse responses
        // HrirData has shape [points, samples], we need to split into left/right
        for i in 0..num_points {
            for n in 0..num_samples {
                let value = hrir_data.impulse_response[[i, n]];
                // For now, duplicate to both ears (mono to stereo)
                // TODO: Handle stereo data properly when we have separate left/right ears
                ir_data[[i, 0, n]] = value;
                ir_data[[i, 1, n]] = value;
            }
        }

        // Write Data.IR
        let mut ir_var = file.add_variable::<f64>("Data.IR", &["M", "R", "N"])?;
        ir_var.put_values(&ir_data.into_raw_vec(), ..)?;
        ir_var.put_attribute("Units", "")?; // Dimensionless

        // Write Data.SamplingRate
        let mut sr_var = file.add_variable::<f64>("Data.SamplingRate", &[])?;
        sr_var.put_values(&[hrir_data.sample_rate], ..)?;
        sr_var.put_attribute("Units", "hertz")?;

        // Write Data.Delay [M, R] (all zeros for now)
        let delay_data = Array2::<f64>::zeros((num_points, 2));
        let mut delay_var = file.add_variable::<f64>("Data.Delay", &["M", "R"])?;
        delay_var.put_values(&delay_data.into_raw_vec(), ..)?;
        delay_var.put_attribute("Units", "samples")?;

        Ok(())
    }

    /// Write position variables (source, receiver, listener)
    fn write_position_vars(
        &self,
        file: &mut netcdf::FileMut,
        source_positions: &Array2<f64>,
        num_measurements: usize,
        num_receivers: usize,
    ) -> Result<()> {
        let coord_type = self.coordinate_system.as_str();
        let coord_units = self.coordinate_system.units();

        // Convert source positions if needed
        let source_pos = if self.coordinate_system == CoordinateSystem::Cartesian {
            source_positions.clone()
        } else {
            // Convert from Cartesian to Spherical
            let mut spherical = Array2::<f64>::zeros((num_measurements, 3));
            for i in 0..num_measurements {
                let (az, el, r) = cartesian_to_spherical(
                    source_positions[[i, 0]],
                    source_positions[[i, 1]],
                    source_positions[[i, 2]],
                );
                spherical[[i, 0]] = az;
                spherical[[i, 1]] = el;
                spherical[[i, 2]] = r;
            }
            spherical
        };

        // Write SourcePosition
        let mut src_var = file.add_variable::<f64>("SourcePosition", &["M", "C"])?;
        src_var.put_values(&source_pos.into_raw_vec(), ..)?;
        src_var.put_attribute("Type", coord_type)?;
        src_var.put_attribute("Units", coord_units)?;

        // Write ReceiverPosition (ears relative to listener)
        // Standard HRTF convention: left ear = (-0.09, 0, 0), right ear = (0.09, 0, 0)
        let receiver_pos = if self.coordinate_system == CoordinateSystem::Cartesian {
            Array2::from_shape_vec((2, 3), vec![-0.09, 0.0, 0.0, 0.09, 0.0, 0.0])?
        } else {
            // Convert to spherical
            let (az1, el1, r1) = cartesian_to_spherical(-0.09, 0.0, 0.0);
            let (az2, el2, r2) = cartesian_to_spherical(0.09, 0.0, 0.0);
            Array2::from_shape_vec((2, 3), vec![az1, el1, r1, az2, el2, r2])?
        };

        let mut rcv_var = file.add_variable::<f64>("ReceiverPosition", &["R", "C"])?;
        rcv_var.put_values(&receiver_pos.into_raw_vec(), ..)?;
        rcv_var.put_attribute("Type", coord_type)?;
        rcv_var.put_attribute("Units", coord_units)?;

        // Write ListenerPosition (all at origin)
        let listener_pos = Array2::<f64>::zeros((num_measurements, 3));
        let mut lst_var = file.add_variable::<f64>("ListenerPosition", &["M", "C"])?;
        lst_var.put_values(&listener_pos.into_raw_vec(), ..)?;
        lst_var.put_attribute("Type", coord_type)?;
        lst_var.put_attribute("Units", coord_units)?;

        // Write ListenerView (facing forward: +y direction in Cartesian, or 0° azimuth)
        let view_dir = if self.coordinate_system == CoordinateSystem::Cartesian {
            vec![0.0, 1.0, 0.0] // +y direction (forward)
        } else {
            vec![0.0, 0.0, 1.0] // 0° azimuth, 0° elevation, unit distance
        };
        let listener_view = Array2::from_shape_fn((num_measurements, 3), |(_, j)| view_dir[j]);

        let mut view_var = file.add_variable::<f64>("ListenerView", &["M", "C"])?;
        view_var.put_values(&listener_view.into_raw_vec(), ..)?;
        view_var.put_attribute("Type", coord_type)?;
        view_var.put_attribute("Units", coord_units)?;

        // Write ListenerUp (up direction: +z in Cartesian, or 90° elevation)
        let up_dir = if self.coordinate_system == CoordinateSystem::Cartesian {
            vec![0.0, 0.0, 1.0] // +z direction (up)
        } else {
            vec![0.0, 90.0, 1.0] // Any azimuth, 90° elevation, unit distance
        };
        let listener_up = Array2::from_shape_fn((num_measurements, 3), |(_, j)| up_dir[j]);

        let mut up_var = file.add_variable::<f64>("ListenerUp", &["M", "C"])?;
        up_var.put_values(&listener_up.into_raw_vec(), ..)?;
        up_var.put_attribute("Type", coord_type)?;
        up_var.put_attribute("Units", coord_units)?;

        Ok(())
    }
}

impl Default for SofaWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn test_cartesian_to_spherical() {
        // Test origin
        let (az, el, r) = cartesian_to_spherical(0.0, 0.0, 0.0);
        assert_abs_diff_eq!(az, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(el, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(r, 0.0, epsilon = 1e-10);

        // Test front (0, 1, 0) → azimuth = 0°
        let (az, el, r) = cartesian_to_spherical(0.0, 1.0, 0.0);
        assert_abs_diff_eq!(az, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(el, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(r, 1.0, epsilon = 1e-10);

        // Test left (1, 0, 0) → azimuth = 90°
        let (az, el, r) = cartesian_to_spherical(1.0, 0.0, 0.0);
        assert_abs_diff_eq!(az, 90.0, epsilon = 1e-10);
        assert_abs_diff_eq!(el, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(r, 1.0, epsilon = 1e-10);

        // Test right (-1, 0, 0) → azimuth = -90°
        let (az, el, r) = cartesian_to_spherical(-1.0, 0.0, 0.0);
        assert_abs_diff_eq!(az, -90.0, epsilon = 1e-10);
        assert_abs_diff_eq!(el, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(r, 1.0, epsilon = 1e-10);

        // Test up (0, 0, 1) → elevation = 90°
        let (az, el, r) = cartesian_to_spherical(0.0, 0.0, 1.0);
        assert_abs_diff_eq!(el, 90.0, epsilon = 1e-10);
        assert_abs_diff_eq!(r, 1.0, epsilon = 1e-10);

        // Test down (0, 0, -1) → elevation = -90°
        let (az, el, r) = cartesian_to_spherical(0.0, 0.0, -1.0);
        assert_abs_diff_eq!(el, -90.0, epsilon = 1e-10);
        assert_abs_diff_eq!(r, 1.0, epsilon = 1e-10);
    }

    #[test]
    fn test_spherical_to_cartesian() {
        // Test front: azimuth=0°, elevation=0°, radius=1
        let (x, y, z) = spherical_to_cartesian(0.0, 0.0, 1.0);
        assert_abs_diff_eq!(x, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(y, 1.0, epsilon = 1e-10);
        assert_abs_diff_eq!(z, 0.0, epsilon = 1e-10);

        // Test left: azimuth=90°, elevation=0°, radius=1
        let (x, y, z) = spherical_to_cartesian(90.0, 0.0, 1.0);
        assert_abs_diff_eq!(x, 1.0, epsilon = 1e-10);
        assert_abs_diff_eq!(y, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(z, 0.0, epsilon = 1e-10);

        // Test up: elevation=90°, radius=1
        let (x, y, z) = spherical_to_cartesian(0.0, 90.0, 1.0);
        assert_abs_diff_eq!(x, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(y, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(z, 1.0, epsilon = 1e-10);
    }

    #[test]
    fn test_coordinate_round_trip() {
        let test_points = vec![
            (1.0, 0.0, 0.0),
            (0.0, 1.0, 0.0),
            (0.0, 0.0, 1.0),
            (1.0, 1.0, 0.0),
            (1.0, 1.0, 1.0),
            (-1.0, 0.5, 0.3),
        ];

        for (x, y, z) in test_points {
            let (az, el, r) = cartesian_to_spherical(x, y, z);
            let (x2, y2, z2) = spherical_to_cartesian(az, el, r);
            assert_abs_diff_eq!(x, x2, epsilon = 1e-10);
            assert_abs_diff_eq!(y, y2, epsilon = 1e-10);
            assert_abs_diff_eq!(z, z2, epsilon = 1e-10);
        }
    }
}
