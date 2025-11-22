//! SOFA (Spatially Oriented Format for Acoustics) file writer
//!
//! This module provides functionality to export HRTF data to the SOFA format,
//! which is the standard for storing spatial audio data.
//!
//! # SOFA Format
//!
//! SOFA is an HDF5-based format with specific conventions for spatial audio.
//! The format includes:
//! - Listener position and orientation
//! - Source positions (measurement grid)
//! - Receiver positions (ear positions)
//! - Impulse responses (Data.IR)
//! - Metadata (sampling rate, conventions, etc.)
//!
//! # References
//! - SOFA Specification: https://www.sofaconventions.org/
//! - AES69-2015: AES standard for file exchange - Spatial acoustic data file format

use crate::acoustics::model::AcousticHeadModel;
use crate::error::{ScannerError, ScannerResult};
use nalgebra::Point3;

#[cfg(feature = "sofa")]
use hdf5::File;

#[cfg(feature = "sofa")]
use ndarray::{Array2, arr2};

/// SOFA file writer for HRTF data
pub struct SOFAWriter {
    file_path: String,
}

impl SOFAWriter {
    /// Create a new SOFA writer
    pub fn new<P: AsRef<str>>(file_path: P) -> Self {
        Self {
            file_path: file_path.as_ref().to_string(),
        }
    }

    /// Write SOFA file with HRTF data
    ///
    /// # Arguments
    /// * `model` - Acoustic head model with ear positions
    /// * `source_positions` - Array of source positions [M x 3]
    /// * `impulse_responses` - Array of IRs [M x R x N] where M=sources, R=ears (2), N=samples
    /// * `sampling_rate` - Sample rate in Hz (e.g., 44100)
    ///
    /// # SOFA Convention
    /// Uses SimpleFreeFieldHRIR convention (most common for HRTFs)
    #[cfg(feature = "sofa")]
    pub fn write_sofa(
        &self,
        model: &AcousticHeadModel,
        source_positions: &[Point3<f32>],
        impulse_responses: &[Vec<Vec<f32>>],
        sampling_rate: f32,
    ) -> ScannerResult<()> {
        use chrono::Utc;

        if source_positions.len() != impulse_responses.len() {
            return Err(ScannerError::InvalidConfig(
                "Mismatch between source positions and impulse responses".to_string(),
            ));
        }

        if impulse_responses.is_empty() {
            return Err(ScannerError::InsufficientData(
                "No impulse responses to write".to_string(),
            ));
        }

        // Validate IR structure
        let num_sources = impulse_responses.len();
        let num_receivers = impulse_responses[0].len();
        let num_samples = impulse_responses[0][0].len();

        if num_receivers != 2 {
            return Err(ScannerError::InvalidConfig(format!(
                "Expected 2 receivers (ears), got {}",
                num_receivers
            )));
        }

        log::info!("Writing SOFA file: {}", self.file_path);
        log::info!(
            "  Sources: {}, Receivers: {}, Samples: {}",
            num_sources,
            num_receivers,
            num_samples
        );

        // Create HDF5 file
        let file = File::create(&self.file_path)
            .map_err(|e| ScannerError::Io(format!("Failed to create SOFA file: {}", e)))?;

        // Write global attributes (required by SOFA)
        self.write_global_attributes(&file)?;

        // Write listener data
        self.write_listener_data(&file, model)?;

        // Write source positions
        self.write_source_positions(&file, source_positions)?;

        // Write receiver (ear) positions
        self.write_receiver_positions(&file, model)?;

        // Write impulse response data
        self.write_impulse_responses(&file, impulse_responses, sampling_rate)?;

        // Write timestamps
        let now = Utc::now();
        file.new_attr::<hdf5::types::VarLenUnicode>()
            .create("DateCreated")?
            .write_scalar(&now.format("%Y-%m-%d %H:%M:%S").to_string().into())?;

        file.new_attr::<hdf5::types::VarLenUnicode>()
            .create("DateModified")?
            .write_scalar(&now.format("%Y-%m-%d %H:%M:%S").to_string().into())?;

        log::info!("✓ SOFA file written successfully: {}", self.file_path);
        Ok(())
    }

    #[cfg(feature = "sofa")]
    fn write_global_attributes(&self, file: &File) -> ScannerResult<()> {
        use hdf5::types::VarLenUnicode;

        // SOFA version and conventions
        file.new_attr::<VarLenUnicode>()
            .create("Conventions")?
            .write_scalar(&"SOFA".to_string().into())?;

        file.new_attr::<VarLenUnicode>()
            .create("Version")?
            .write_scalar(&"1.0".to_string().into())?;

        file.new_attr::<VarLenUnicode>()
            .create("SOFAConventions")?
            .write_scalar(&"SimpleFreeFieldHRIR".to_string().into())?;

        file.new_attr::<VarLenUnicode>()
            .create("SOFAConventionsVersion")?
            .write_scalar(&"1.0".to_string().into())?;

        file.new_attr::<VarLenUnicode>()
            .create("DataType")?
            .write_scalar(&"FIR".to_string().into())?;

        file.new_attr::<VarLenUnicode>()
            .create("RoomType")?
            .write_scalar(&"free field".to_string().into())?;

        file.new_attr::<VarLenUnicode>()
            .create("Title")?
            .write_scalar(&"HRTF generated from 3D head scan".to_string().into())?;

        file.new_attr::<VarLenUnicode>()
            .create("Organization")?
            .write_scalar(&"Head Scanner".to_string().into())?;

        file.new_attr::<VarLenUnicode>()
            .create("ApplicationName")?
            .write_scalar(&"head-scanner".to_string().into())?;

        file.new_attr::<VarLenUnicode>()
            .create("ApplicationVersion")?
            .write_scalar(env!("CARGO_PKG_VERSION"))?;

        file.new_attr::<VarLenUnicode>()
            .create("Comment")?
            .write_scalar(&"Generated using analytical Woodworth-Schlosberg model".to_string().into())?;

        Ok(())
    }

    #[cfg(feature = "sofa")]
    fn write_listener_data(&self, file: &File, model: &AcousticHeadModel) -> ScannerResult<()> {
        use hdf5::types::VarLenUnicode;

        // ListenerPosition [1 x 3]: position of head center
        let listener_position = file
            .new_dataset::<f32>()
            .shape([1, 3])
            .create("ListenerPosition")?;

        listener_position.write(&arr2![[
            model.head_center.x / 100.0, // Convert cm to m
            model.head_center.y / 100.0,
            model.head_center.z / 100.0,
        ]])?;

        listener_position
            .new_attr::<VarLenUnicode>()
            .create("Type")?
            .write_scalar(&"cartesian".to_string().into())?;

        listener_position
            .new_attr::<VarLenUnicode>()
            .create("Units")?
            .write_scalar(&"metre".to_string().into())?;

        // ListenerView [1 x 3]: listener's viewing direction (0, 0, 1) = looking forward
        let listener_view = file
            .new_dataset::<f32>()
            .shape([1, 3])
            .create("ListenerView")?;

        listener_view.write(&arr2![[0.0, 0.0, 1.0]])?;

        listener_view
            .new_attr::<VarLenUnicode>()
            .create("Type")?
            .write_scalar(&"cartesian".to_string().into())?;

        listener_view
            .new_attr::<VarLenUnicode>()
            .create("Units")?
            .write_scalar(&"metre".to_string().into())?;

        // ListenerUp [1 x 3]: listener's up direction (0, 1, 0)
        let listener_up = file
            .new_dataset::<f32>()
            .shape([1, 3])
            .create("ListenerUp")?;

        listener_up.write(&arr2![[0.0, 1.0, 0.0]])?;

        listener_up
            .new_attr::<VarLenUnicode>()
            .create("Type")?
            .write_scalar(&"cartesian".to_string().into())?;

        listener_up
            .new_attr::<VarLenUnicode>()
            .create("Units")?
            .write_scalar(&"metre".to_string().into())?;

        Ok(())
    }

    #[cfg(feature = "sofa")]
    fn write_source_positions(&self, file: &File, positions: &[Point3<f32>]) -> ScannerResult<()> {
        use hdf5::types::VarLenUnicode;

        let num_sources = positions.len();

        // Convert positions to meters and flatten to [M x 3] array
        let mut pos_array = Vec::with_capacity(num_sources * 3);
        for pos in positions {
            pos_array.push(pos.x / 100.0); // cm to m
            pos_array.push(pos.y / 100.0);
            pos_array.push(pos.z / 100.0);
        }

        let source_position = file
            .new_dataset::<f32>()
            .shape([num_sources, 3])
            .create("SourcePosition")?;

        source_position.write_raw(&pos_array)?;

        source_position
            .new_attr::<VarLenUnicode>()
            .create("Type")?
            .write_scalar(&"cartesian".to_string().into())?;

        source_position
            .new_attr::<VarLenUnicode>()
            .create("Units")?
            .write_scalar(&"metre".to_string().into())?;

        Ok(())
    }

    #[cfg(feature = "sofa")]
    fn write_receiver_positions(
        &self,
        file: &File,
        model: &AcousticHeadModel,
    ) -> ScannerResult<()> {
        use hdf5::types::VarLenUnicode;

        // ReceiverPosition [2 x 3 x 1]: positions of left and right ears
        // Dims: [R x C x M] where R=2 (ears), C=3 (x,y,z), M=1 (single measurement setup)

        // Note: Positions are relative to listener position (head center)
        let left_rel = model.left_ear - model.head_center;
        let right_rel = model.right_ear - model.head_center;

        let receiver_positions = file
            .new_dataset::<f32>()
            .shape([2, 3, 1])
            .create("ReceiverPosition")?;

        let pos_data = [
            // Left ear [3 x 1]
            left_rel.x / 100.0,
            left_rel.y / 100.0,
            left_rel.z / 100.0,
            // Right ear [3 x 1]
            right_rel.x / 100.0,
            right_rel.y / 100.0,
            right_rel.z / 100.0,
        ];

        receiver_positions.write_raw(&pos_data)?;

        receiver_positions
            .new_attr::<VarLenUnicode>()
            .create("Type")?
            .write_scalar(&"cartesian".to_string().into())?;

        receiver_positions
            .new_attr::<VarLenUnicode>()
            .create("Units")?
            .write_scalar(&"metre".to_string().into())?;

        Ok(())
    }

    #[cfg(feature = "sofa")]
    fn write_impulse_responses(
        &self,
        file: &File,
        impulse_responses: &[Vec<Vec<f32>>],
        sampling_rate: f32,
    ) -> ScannerResult<()> {
        let num_sources = impulse_responses.len();
        let num_receivers = impulse_responses[0].len();
        let num_samples = impulse_responses[0][0].len();

        // Create Data group
        let data_group = file.create_group("Data")?;

        // Data.IR [M x R x N]: M sources, R receivers (2), N samples
        let ir_dataset = data_group
            .new_dataset::<f32>()
            .shape([num_sources, num_receivers, num_samples])
            .create("IR")?;

        // Flatten impulse responses to 1D array in row-major order
        let mut ir_flat = Vec::with_capacity(num_sources * num_receivers * num_samples);
        for source_irs in impulse_responses {
            for ear_ir in source_irs {
                ir_flat.extend_from_slice(ear_ir);
            }
        }

        ir_dataset.write_raw(&ir_flat)?;

        // Data.SamplingRate
        data_group
            .new_attr::<f32>()
            .create("SamplingRate")?
            .write_scalar(&sampling_rate)?;

        data_group
            .new_attr::<hdf5::types::VarLenUnicode>()
            .create("SamplingRate:Units")?
            .write_scalar(&"hertz".to_string().into())?;

        // Data.Delay [M x R]: per-source, per-receiver delay (all zeros for us)
        let delay_dataset = data_group
            .new_dataset::<f32>()
            .shape([num_sources, num_receivers])
            .create("Delay")?;

        let delays = vec![0.0f32; num_sources * num_receivers];
        delay_dataset.write_raw(&delays)?;

        Ok(())
    }

    /// Write SOFA file (stub when feature is disabled)
    #[cfg(not(feature = "sofa"))]
    pub fn write_sofa(
        &self,
        _model: &AcousticHeadModel,
        _source_positions: &[Point3<f32>],
        _impulse_responses: &[Vec<Vec<f32>>],
        _sampling_rate: f32,
    ) -> ScannerResult<()> {
        Err(ScannerError::InvalidConfig(
            "SOFA support not enabled. Rebuild with --features sofa".to_string(),
        ))
    }
}

#[cfg(all(test, feature = "sofa"))]
mod tests {
    use super::*;
    use crate::mesh::{Mesh, Triangle, Vertex};

    fn create_test_model() -> AcousticHeadModel {
        let vertices = vec![
            Vertex::new(0.0, 0.0, 0.0),
            Vertex::new(10.0, 0.0, 0.0),
            Vertex::new(0.0, 10.0, 0.0),
        ];
        let triangles = vec![Triangle::new(0, 1, 2)];
        let mesh = Mesh::from_parts(vertices, triangles);

        AcousticHeadModel {
            mesh,
            left_ear: Point3::new(-7.0, 0.0, 0.0),
            right_ear: Point3::new(7.0, 0.0, 0.0),
            head_center: Point3::origin(),
            head_radius: 9.0,
            dimensions: (18.0, 18.0, 18.0),
        }
    }

    #[test]
    fn test_sofa_write() {
        let model = create_test_model();

        // Create test data
        let source_positions = vec![
            Point3::new(0.0, 0.0, 100.0),   // Front
            Point3::new(100.0, 0.0, 0.0),   // Left
            Point3::new(-100.0, 0.0, 0.0),  // Right
        ];

        let impulse_responses = vec![
            vec![vec![0.1, 0.2, 0.3], vec![0.1, 0.2, 0.3]], // Source 1: [left_ir, right_ir]
            vec![vec![0.4, 0.5, 0.6], vec![0.4, 0.5, 0.6]], // Source 2
            vec![vec![0.7, 0.8, 0.9], vec![0.7, 0.8, 0.9]], // Source 3
        ];

        let temp_file = "/tmp/test.sofa";
        let writer = SOFAWriter::new(temp_file);

        let result = writer.write_sofa(&model, &source_positions, &impulse_responses, 44100.0);

        assert!(result.is_ok());

        // Clean up
        std::fs::remove_file(temp_file).ok();
    }
}
