//! Data types for HRTF processing

use ndarray::Array2;
use num_complex::Complex64;
use serde::{Deserialize, Serialize};

/// Sound pressure data on evaluation grid
#[derive(Debug, Clone)]
pub struct PressureData {
    /// Complex pressure values [num_points, num_frequencies]
    pub pressure: Array2<Complex64>,

    /// Node IDs (indices in evaluation grid)
    pub node_ids: Vec<usize>,

    /// Frequencies (Hz)
    pub frequencies: Vec<f64>,
}

/// Sound velocity data on evaluation grid
#[derive(Debug, Clone)]
pub struct VelocityData {
    /// Velocity magnitude values [num_points, num_frequencies]
    pub velocity: Array2<f64>,

    /// Node IDs (indices in evaluation grid)
    pub node_ids: Vec<usize>,

    /// Frequencies (Hz)
    pub frequencies: Vec<f64>,
}

/// Complete HRTF data from NumCalc simulation
#[derive(Debug, Clone)]
pub struct HrtfData {
    /// Pressure on evaluation grid
    pub eval_pressure: PressureData,

    /// Velocity on evaluation grid (optional)
    pub eval_velocity: Option<VelocityData>,

    /// Pressure on boundary (object mesh)
    pub boundary_pressure: Option<PressureData>,

    /// Velocity on boundary (object mesh)
    pub boundary_velocity: Option<VelocityData>,

    /// Source index (0-based)
    pub source_index: usize,

    /// Physical parameters
    pub speed_of_sound: f64,

    /// Density of medium (kg/m³)
    pub density: f64,
}

/// HRIR (Head-Related Impulse Response) data
#[derive(Debug, Clone)]
pub struct HrirData {
    /// Impulse response [num_points, num_samples]
    pub impulse_response: Array2<f64>,

    /// Sample rate (Hz)
    pub sample_rate: f64,

    /// Node IDs
    pub node_ids: Vec<usize>,
}

/// Data type for NumCalc be.out files
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    /// Pressure on evaluation grid (complex)
    PressureEvalGrid,

    /// Pressure on boundary/object mesh (complex)
    PressureBoundary,

    /// Velocity on evaluation grid (3D vector, complex)
    VelocityEvalGrid,

    /// Velocity on boundary/object mesh (scalar, complex)
    VelocityBoundary,
}

impl DataType {
    /// Get the filename for this data type
    pub fn filename(&self) -> &'static str {
        match self {
            DataType::PressureEvalGrid => "pEvalGrid",
            DataType::PressureBoundary => "pBoundary",
            DataType::VelocityEvalGrid => "vEvalGrid",
            DataType::VelocityBoundary => "vBoundary",
        }
    }

    /// Check if this data type is pressure (complex)
    pub fn is_pressure(&self) -> bool {
        matches!(
            self,
            DataType::PressureEvalGrid | DataType::PressureBoundary
        )
    }

    /// Check if this data type is velocity
    pub fn is_velocity(&self) -> bool {
        matches!(
            self,
            DataType::VelocityEvalGrid | DataType::VelocityBoundary
        )
    }
}

impl PressureData {
    /// Create new pressure data
    pub fn new(num_points: usize, num_frequencies: usize) -> Self {
        Self {
            pressure: Array2::zeros((num_points, num_frequencies)),
            node_ids: Vec::with_capacity(num_points),
            frequencies: Vec::with_capacity(num_frequencies),
        }
    }

    /// Get pressure magnitude (dB SPL)
    pub fn magnitude_db(&self, reference_pressure: f64) -> Array2<f64> {
        self.pressure
            .mapv(|p| 20.0 * (p.norm() / reference_pressure).log10())
    }

    /// Get phase (radians)
    pub fn phase(&self) -> Array2<f64> {
        self.pressure.mapv(|p| p.arg())
    }
}

impl VelocityData {
    /// Create new velocity data
    pub fn new(num_points: usize, num_frequencies: usize) -> Self {
        Self {
            velocity: Array2::zeros((num_points, num_frequencies)),
            node_ids: Vec::with_capacity(num_points),
            frequencies: Vec::with_capacity(num_frequencies),
        }
    }
}

impl HrtfData {
    /// Create new HRTF data structure
    pub fn new(num_eval_points: usize, num_frequencies: usize, source_index: usize) -> Self {
        Self {
            eval_pressure: PressureData::new(num_eval_points, num_frequencies),
            eval_velocity: None,
            boundary_pressure: None,
            boundary_velocity: None,
            source_index,
            speed_of_sound: 343.0,
            density: 1.1839,
        }
    }

    /// Get number of evaluation points
    pub fn num_points(&self) -> usize {
        self.eval_pressure.pressure.nrows()
    }

    /// Get number of frequencies
    pub fn num_frequencies(&self) -> usize {
        self.eval_pressure.pressure.ncols()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_type_filename() {
        assert_eq!(DataType::PressureEvalGrid.filename(), "pEvalGrid");
        assert_eq!(DataType::PressureBoundary.filename(), "pBoundary");
        assert_eq!(DataType::VelocityEvalGrid.filename(), "vEvalGrid");
        assert_eq!(DataType::VelocityBoundary.filename(), "vBoundary");
    }

    #[test]
    fn test_data_type_checks() {
        assert!(DataType::PressureEvalGrid.is_pressure());
        assert!(!DataType::PressureEvalGrid.is_velocity());
        assert!(DataType::VelocityEvalGrid.is_velocity());
        assert!(!DataType::VelocityEvalGrid.is_pressure());
    }

    #[test]
    fn test_pressure_data_creation() {
        let data = PressureData::new(100, 50);
        assert_eq!(data.pressure.nrows(), 100);
        assert_eq!(data.pressure.ncols(), 50);
        assert_eq!(data.node_ids.capacity(), 100);
        assert_eq!(data.frequencies.capacity(), 50);
    }

    #[test]
    fn test_hrtf_data_creation() {
        let data = HrtfData::new(100, 50, 0);
        assert_eq!(data.num_points(), 100);
        assert_eq!(data.num_frequencies(), 50);
        assert_eq!(data.source_index, 0);
    }
}
