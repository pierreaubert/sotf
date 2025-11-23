//! NumCalc output parser
//!
//! Parses NumCalc BEM simulation output files from the be.out directory structure.
//!
//! # File Structure
//!
//! ```text
//! NumCalc/
//! └── source_N/
//!     └── be.out/
//!         ├── be.1/        # Frequency 1
//!         │   ├── pEvalGrid  # Pressure on evaluation grid (complex)
//!         │   ├── pBoundary  # Pressure on boundary (complex)
//!         │   ├── vEvalGrid  # Velocity on evaluation grid (3D vector, complex)
//!         │   └── vBoundary  # Velocity on boundary (scalar, complex)
//!         ├── be.2/        # Frequency 2
//!         │   └── ...
//!         └── ...
//! ```
//!
//! # File Format
//!
//! Each file contains space-separated values:
//! - Line 1: "Mesh2HRTF <version>"
//! - Line 2: Grid ID (integer)
//! - Line 3: "start_index  num_datalines"
//! - Following lines:
//!   - pEvalGrid/pBoundary: `node_id  real  imag`
//!   - vBoundary: `node_id  real  imag`
//!   - vEvalGrid: `node_id  real_x  imag_x  real_y  imag_y  real_z  imag_z`

use crate::hrtf::types::*;
use anyhow::{Context, Result};
use ndarray::Array2;
use num_complex::Complex64;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// NumCalc output parser
pub struct NumCalcParser {
    /// Project root directory
    project_dir: PathBuf,

    /// Number of frequencies
    num_frequencies: usize,
}

impl NumCalcParser {
    /// Create a new parser for a Mesh2HRTF project
    pub fn new<P: AsRef<Path>>(project_dir: P) -> Result<Self> {
        let project_path = project_dir.as_ref().to_path_buf();

        // Validate project directory
        if !project_path.exists() {
            anyhow::bail!("Project directory does not exist: {:?}", project_path);
        }

        Ok(Self {
            project_dir: project_path,
            num_frequencies: 0, // Will be determined from be.out contents
        })
    }

    /// Parse all data for a specific source
    pub fn parse_source(&mut self, source_index: usize) -> Result<HrtfData> {
        let source_dir = self.source_dir(source_index);

        // Determine number of frequencies
        self.detect_num_frequencies(&source_dir)?;

        // Parse pressure on evaluation grid
        let eval_pressure = self
            .parse_pressure(&source_dir, DataType::PressureEvalGrid)
            .context("Failed to parse evaluation grid pressure")?;

        let num_eval_points = eval_pressure.node_ids.len();

        let mut hrtf_data = HrtfData::new(num_eval_points, self.num_frequencies, source_index);
        hrtf_data.eval_pressure = eval_pressure;

        // Parse optional data (may not exist for all simulations)
        if let Ok(eval_velocity) = self.parse_velocity(&source_dir, DataType::VelocityEvalGrid) {
            hrtf_data.eval_velocity = Some(eval_velocity);
        }

        if let Ok(boundary_pressure) = self.parse_pressure(&source_dir, DataType::PressureBoundary)
        {
            hrtf_data.boundary_pressure = Some(boundary_pressure);
        }

        if let Ok(boundary_velocity) = self.parse_velocity(&source_dir, DataType::VelocityBoundary)
        {
            hrtf_data.boundary_velocity = Some(boundary_velocity);
        }

        Ok(hrtf_data)
    }

    /// Get source directory path
    fn source_dir(&self, source_index: usize) -> PathBuf {
        self.project_dir
            .join("NumCalc")
            .join(format!("source_{}", source_index + 1))
            .join("be.out")
    }

    /// Detect number of frequencies from be.out directory
    fn detect_num_frequencies(&mut self, source_dir: &Path) -> Result<()> {
        // Count be.N directories
        let mut max_freq_idx = 0;

        for entry in std::fs::read_dir(source_dir)
            .with_context(|| format!("Failed to read source directory: {:?}", source_dir))?
        {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if name_str.starts_with("be.") {
                if let Ok(freq_idx) = name_str[3..].parse::<usize>() {
                    max_freq_idx = max_freq_idx.max(freq_idx);
                }
            }
        }

        if max_freq_idx == 0 {
            anyhow::bail!("No frequency directories (be.N) found in {:?}", source_dir);
        }

        self.num_frequencies = max_freq_idx;
        Ok(())
    }

    /// Parse pressure data (complex)
    fn parse_pressure(&self, source_dir: &Path, data_type: DataType) -> Result<PressureData> {
        if !data_type.is_pressure() {
            anyhow::bail!("Data type {:?} is not pressure", data_type);
        }

        // Read first frequency to get metadata
        let first_file = source_dir.join("be.1").join(data_type.filename());
        let (num_points, start_index, frequencies) =
            self.read_file_metadata(&first_file, data_type)?;

        // Create data structure
        let mut pressure_data = PressureData::new(num_points, self.num_frequencies);
        pressure_data.frequencies = frequencies;

        // Read data for each frequency
        for freq_idx in 0..self.num_frequencies {
            let file_path = source_dir
                .join(format!("be.{}", freq_idx + 1))
                .join(data_type.filename());

            let (values, node_ids) = self.read_pressure_file(&file_path, data_type)?;

            // Store node IDs from first frequency
            if freq_idx == 0 {
                pressure_data.node_ids = node_ids;
            }

            // Store pressure values
            for (point_idx, &value) in values.iter().enumerate() {
                pressure_data.pressure[[point_idx, freq_idx]] = value;
            }
        }

        Ok(pressure_data)
    }

    /// Parse velocity data (magnitude)
    fn parse_velocity(&self, source_dir: &Path, data_type: DataType) -> Result<VelocityData> {
        if !data_type.is_velocity() {
            anyhow::bail!("Data type {:?} is not velocity", data_type);
        }

        // Read first frequency to get metadata
        let first_file = source_dir.join("be.1").join(data_type.filename());
        let (num_points, start_index, frequencies) =
            self.read_file_metadata(&first_file, data_type)?;

        // Create data structure
        let mut velocity_data = VelocityData::new(num_points, self.num_frequencies);
        velocity_data.frequencies = frequencies;

        // Read data for each frequency
        for freq_idx in 0..self.num_frequencies {
            let file_path = source_dir
                .join(format!("be.{}", freq_idx + 1))
                .join(data_type.filename());

            let (values, node_ids) = self.read_velocity_file(&file_path, data_type)?;

            // Store node IDs from first frequency
            if freq_idx == 0 {
                velocity_data.node_ids = node_ids;
            }

            // Store velocity magnitudes
            for (point_idx, &value) in values.iter().enumerate() {
                velocity_data.velocity[[point_idx, freq_idx]] = value;
            }
        }

        Ok(velocity_data)
    }

    /// Read file metadata (number of points, start index)
    fn read_file_metadata(
        &self,
        file_path: &Path,
        data_type: DataType,
    ) -> Result<(usize, usize, Vec<f64>)> {
        let file = File::open(file_path)
            .with_context(|| format!("Failed to open file: {:?}", file_path))?;
        let reader = BufReader::new(file);

        let mut num_points = 0;
        let mut start_index = 0;

        for (line_idx, line) in reader.lines().enumerate() {
            let line = line?;
            let parts: Vec<&str> = line.split_whitespace().collect();

            // Skip Mesh2HRTF version line
            if parts.is_empty() || parts[0].starts_with("Mesh") {
                continue;
            }

            // Line with "start_index  num_points"
            if parts.len() == 2 {
                start_index = parts[0].parse::<usize>().unwrap_or(0);
                num_points = parts[1].parse::<usize>()?;
                break;
            }
        }

        if num_points == 0 {
            anyhow::bail!(
                "Could not determine number of points from file: {:?}",
                file_path
            );
        }

        // Frequencies will be filled later from NC.inp or parameters.json
        Ok((num_points, start_index, vec![]))
    }

    /// Read pressure file (complex values)
    fn read_pressure_file(
        &self,
        file_path: &Path,
        data_type: DataType,
    ) -> Result<(Vec<Complex64>, Vec<usize>)> {
        let file = File::open(file_path)
            .with_context(|| format!("Failed to open file: {:?}", file_path))?;
        let reader = BufReader::new(file);

        let mut values = Vec::new();
        let mut node_ids = Vec::new();

        for line in reader.lines() {
            let line = line?;
            let parts: Vec<&str> = line.split_whitespace().collect();

            // Skip header lines
            if parts.len() < 3 || parts[0].starts_with("Mesh") {
                continue;
            }

            // Parse: node_id  real  imag
            let node_id = parts[0].parse::<usize>()?;
            let real = parts[1].parse::<f64>()?;
            let imag = parts[2].parse::<f64>()?;

            node_ids.push(node_id);
            values.push(Complex64::new(real, imag));
        }

        Ok((values, node_ids))
    }

    /// Read velocity file (magnitude from components)
    fn read_velocity_file(
        &self,
        file_path: &Path,
        data_type: DataType,
    ) -> Result<(Vec<f64>, Vec<usize>)> {
        let file = File::open(file_path)
            .with_context(|| format!("Failed to open file: {:?}", file_path))?;
        let reader = BufReader::new(file);

        let mut values = Vec::new();
        let mut node_ids = Vec::new();

        for line in reader.lines() {
            let line = line?;
            let parts: Vec<&str> = line.split_whitespace().collect();

            // Skip header lines
            if parts.is_empty() || parts[0].starts_with("Mesh") {
                continue;
            }

            let node_id = parts[0].parse::<usize>()?;
            node_ids.push(node_id);

            match data_type {
                DataType::VelocityBoundary => {
                    // vBoundary: node_id  real  imag (scalar)
                    if parts.len() < 3 {
                        continue;
                    }
                    let real = parts[1].parse::<f64>()?;
                    let imag = parts[2].parse::<f64>()?;
                    let magnitude = Complex64::new(real, imag).norm();
                    values.push(magnitude);
                }
                DataType::VelocityEvalGrid => {
                    // vEvalGrid: node_id  real_x  imag_x  real_y  imag_y  real_z  imag_z
                    if parts.len() < 7 {
                        continue;
                    }
                    let vx = Complex64::new(parts[1].parse::<f64>()?, parts[2].parse::<f64>()?);
                    let vy = Complex64::new(parts[3].parse::<f64>()?, parts[4].parse::<f64>()?);
                    let vz = Complex64::new(parts[5].parse::<f64>()?, parts[6].parse::<f64>()?);

                    // Magnitude: sqrt(|vx|^2 + |vy|^2 + |vz|^2)
                    let magnitude =
                        (vx.norm().powi(2) + vy.norm().powi(2) + vz.norm().powi(2)).sqrt();
                    values.push(magnitude);
                }
                _ => anyhow::bail!("Invalid velocity data type: {:?}", data_type),
            }
        }

        Ok((values, node_ids))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_type_properties() {
        assert_eq!(DataType::PressureEvalGrid.filename(), "pEvalGrid");
        assert!(DataType::PressureEvalGrid.is_pressure());
        assert!(!DataType::PressureEvalGrid.is_velocity());

        assert_eq!(DataType::VelocityEvalGrid.filename(), "vEvalGrid");
        assert!(DataType::VelocityEvalGrid.is_velocity());
        assert!(!DataType::VelocityEvalGrid.is_pressure());
    }
}
