//! Source configuration for NumCalc simulations
//!
//! This module handles different source types for HRTF simulations:
//! - Ear sources (left, right, or both)
//! - Point sources (analytical)
//! - Plane waves (analytical)
//!
//! It also manages material assignment for boundary conditions.

use crate::mesh2hrtf::types::*;
use anyhow::{Context, Result};
use nalgebra::Point3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Material configuration for boundary conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    /// Material name (e.g., "Left ear", "Right ear", "Skull")
    pub name: String,

    /// Starting element index (0-based)
    pub index_start: usize,

    /// Ending element index (0-based, inclusive)
    pub index_end: usize,

    /// Boundary condition type ("VELO", "IMPE", "PRES")
    pub boundary_type: BoundaryCondition,

    /// Optional frequency-dependent material properties
    pub frequency_curve: Option<FrequencyCurve>,
}

/// Boundary condition types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoundaryCondition {
    /// Velocity boundary condition (vibrating surface)
    Velocity,

    /// Impedance boundary condition
    Impedance,

    /// Pressure boundary condition
    Pressure,
}

impl BoundaryCondition {
    pub fn to_string(&self) -> &'static str {
        match self {
            BoundaryCondition::Velocity => "VELO",
            BoundaryCondition::Impedance => "IMPE",
            BoundaryCondition::Pressure => "PRES",
        }
    }
}

/// Frequency-dependent material properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrequencyCurve {
    /// Frequencies (Hz)
    pub frequencies: Vec<f64>,

    /// Real component of impedance/admittance
    pub real_values: Vec<f64>,

    /// Imaginary component of impedance/admittance
    pub imag_values: Vec<f64>,
}

/// Source configuration for a NumCalc simulation
#[derive(Debug, Clone)]
pub struct SourceConfiguration {
    /// Source type
    pub source_type: SourceType,

    /// Number of sources (1 or 2)
    pub num_sources: usize,

    /// Materials for each source
    pub materials: Vec<HashMap<String, Material>>,

    /// Source positions (for point sources/plane waves)
    pub source_positions: Vec<Point3<f64>>,
}

impl SourceConfiguration {
    /// Create configuration for both ears
    pub fn both_ears(
        left_material_id: usize,
        right_material_id: usize,
        mesh: &Mesh,
    ) -> Result<Self> {
        let left_elements = Self::find_material_elements(mesh, left_material_id)?;
        let right_elements = Self::find_material_elements(mesh, right_material_id)?;

        // Create materials for left ear source
        let mut left_materials = HashMap::new();
        left_materials.insert(
            "Left ear".to_string(),
            Material {
                name: "Left ear".to_string(),
                index_start: left_elements.0,
                index_end: left_elements.1,
                boundary_type: BoundaryCondition::Velocity,
                frequency_curve: None,
            },
        );

        // Create materials for right ear source
        let mut right_materials = HashMap::new();
        right_materials.insert(
            "Right ear".to_string(),
            Material {
                name: "Right ear".to_string(),
                index_start: right_elements.0,
                index_end: right_elements.1,
                boundary_type: BoundaryCondition::Velocity,
                frequency_curve: None,
            },
        );

        Ok(Self {
            source_type: SourceType::BothEars {
                left_material: left_material_id,
                right_material: right_material_id,
            },
            num_sources: 2,
            materials: vec![left_materials, right_materials],
            source_positions: vec![],
        })
    }

    /// Create configuration for left ear only
    pub fn left_ear(material_id: usize, mesh: &Mesh) -> Result<Self> {
        let elements = Self::find_material_elements(mesh, material_id)?;

        let mut materials = HashMap::new();
        materials.insert(
            "Left ear".to_string(),
            Material {
                name: "Left ear".to_string(),
                index_start: elements.0,
                index_end: elements.1,
                boundary_type: BoundaryCondition::Velocity,
                frequency_curve: None,
            },
        );

        Ok(Self {
            source_type: SourceType::LeftEar {
                material: material_id,
            },
            num_sources: 1,
            materials: vec![materials],
            source_positions: vec![],
        })
    }

    /// Create configuration for right ear only
    pub fn right_ear(material_id: usize, mesh: &Mesh) -> Result<Self> {
        let elements = Self::find_material_elements(mesh, material_id)?;

        let mut materials = HashMap::new();
        materials.insert(
            "Right ear".to_string(),
            Material {
                name: "Right ear".to_string(),
                index_start: elements.0,
                index_end: elements.1,
                boundary_type: BoundaryCondition::Velocity,
                frequency_curve: None,
            },
        );

        Ok(Self {
            source_type: SourceType::RightEar {
                material: material_id,
            },
            num_sources: 1,
            materials: vec![materials],
            source_positions: vec![],
        })
    }

    /// Create configuration for point source
    pub fn point_source(position: Point3<f64>) -> Self {
        Self {
            source_type: SourceType::PointSource { position },
            num_sources: 1,
            materials: vec![HashMap::new()],
            source_positions: vec![position],
        }
    }

    /// Create configuration for plane wave
    pub fn plane_wave(direction: Vec3) -> Self {
        Self {
            source_type: SourceType::PlaneWave { direction },
            num_sources: 1,
            materials: vec![HashMap::new()],
            source_positions: vec![Point3::new(direction.x, direction.y, direction.z)],
        }
    }

    /// Find elements with a specific material ID
    fn find_material_elements(mesh: &Mesh, material_id: usize) -> Result<(usize, usize)> {
        let elements: Vec<_> = mesh
            .elements
            .iter()
            .filter(|e| e.material_id == material_id)
            .collect();

        if elements.is_empty() {
            anyhow::bail!("No elements found with material ID {}", material_id);
        }

        // Find min and max element IDs for this material
        let min_id = elements.iter().map(|e| e.id).min().unwrap();
        let max_id = elements.iter().map(|e| e.id).max().unwrap();

        Ok((min_id, max_id))
    }

    /// Check if this is an ear source (not analytical)
    pub fn is_ear_source(&self) -> bool {
        matches!(
            self.source_type,
            SourceType::BothEars { .. }
                | SourceType::LeftEar { .. }
                | SourceType::RightEar { .. }
        )
    }

    /// Check if this is a point source
    pub fn is_point_source(&self) -> bool {
        matches!(self.source_type, SourceType::PointSource { .. })
    }

    /// Check if this is a plane wave
    pub fn is_plane_wave(&self) -> bool {
        matches!(self.source_type, SourceType::PlaneWave { .. })
    }

    /// Get source name for display
    pub fn source_name(&self, source_index: usize) -> String {
        match &self.source_type {
            SourceType::BothEars { .. } => {
                if source_index == 0 {
                    "Left ear".to_string()
                } else {
                    "Right ear".to_string()
                }
            }
            SourceType::LeftEar { .. } => "Left ear".to_string(),
            SourceType::RightEar { .. } => "Right ear".to_string(),
            SourceType::PointSource { .. } => "Point source".to_string(),
            SourceType::PlaneWave { .. } => "Plane wave".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boundary_condition_to_string() {
        assert_eq!(BoundaryCondition::Velocity.to_string(), "VELO");
        assert_eq!(BoundaryCondition::Impedance.to_string(), "IMPE");
        assert_eq!(BoundaryCondition::Pressure.to_string(), "PRES");
    }

    #[test]
    fn test_point_source_configuration() {
        let pos = Point3::new(1.0, 2.0, 3.0);
        let config = SourceConfiguration::point_source(pos);

        assert_eq!(config.num_sources, 1);
        assert!(config.is_point_source());
        assert!(!config.is_ear_source());
        assert!(!config.is_plane_wave());
        assert_eq!(config.source_positions.len(), 1);
        assert_eq!(config.source_positions[0], pos);
    }

    #[test]
    fn test_plane_wave_configuration() {
        let dir = Vec3::new(0.0, -1.0, 0.0);
        let config = SourceConfiguration::plane_wave(dir);

        assert_eq!(config.num_sources, 1);
        assert!(config.is_plane_wave());
        assert!(!config.is_ear_source());
        assert!(!config.is_point_source());
    }

    #[test]
    fn test_source_names() {
        let pos = Point3::new(0.0, 0.0, 0.0);

        let point_config = SourceConfiguration::point_source(pos);
        assert_eq!(point_config.source_name(0), "Point source");

        let plane_config = SourceConfiguration::plane_wave(Vec3::new(0.0, -1.0, 0.0));
        assert_eq!(plane_config.source_name(0), "Plane wave");
    }
}
