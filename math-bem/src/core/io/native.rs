//! Native Rust JSON/TOML format for BEM solver configuration
//!
//! This module provides a clean, idiomatic Rust configuration format
//! that supports both JSON and TOML serialization via serde.
//!
//! ## Example JSON Configuration
//!
//! ```json
//! {
//!     "physics": {
//!         "frequency": 1000.0,
//!         "speed_of_sound": 343.0,
//!         "density": 1.21
//!     },
//!     "mesh": {
//!         "nodes_file": "nodes.json",
//!         "elements_file": "elements.json"
//!     },
//!     "solver": {
//!         "method": "BiCGSTAB",
//!         "tolerance": 1e-6,
//!         "max_iterations": 1000
//!     }
//! }
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use ndarray::{Array1, Array2};
use num_complex::Complex64;
use serde::{Deserialize, Serialize};

use crate::core::types::{
    BemMethod, BoundaryCondition, Element, ElementProperty, ElementType, PhysicsParams,
    SolverMethod,
};

/// Native BEM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BemConfig {
    /// Problem description
    #[serde(default)]
    pub description: String,

    /// Physics parameters
    pub physics: PhysicsConfig,

    /// Mesh configuration
    pub mesh: MeshConfig,

    /// Solver configuration
    #[serde(default)]
    pub solver: SolverConfig,

    /// BEM method configuration
    #[serde(default)]
    pub bem: BemMethodConfig,

    /// Boundary conditions
    #[serde(default)]
    pub boundary_conditions: Vec<BoundaryConditionConfig>,

    /// Excitation sources
    #[serde(default)]
    pub sources: SourceConfig,

    /// Output configuration
    #[serde(default)]
    pub output: OutputConfig,
}

/// Physics parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsConfig {
    /// Frequency in Hz
    pub frequency: f64,

    /// Speed of sound in m/s
    #[serde(default = "default_speed_of_sound")]
    pub speed_of_sound: f64,

    /// Medium density in kg/m³
    #[serde(default = "default_density")]
    pub density: f64,

    /// Reference pressure in Pa
    #[serde(default = "default_reference_pressure")]
    pub reference_pressure: f64,

    /// External problem (true) or internal (false)
    #[serde(default = "default_true")]
    pub external_problem: bool,
}

fn default_speed_of_sound() -> f64 {
    343.0
}
fn default_density() -> f64 {
    1.21
}
fn default_reference_pressure() -> f64 {
    1.0
}
fn default_true() -> bool {
    true
}

/// Mesh configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshConfig {
    /// Path to nodes file (JSON or CSV)
    #[serde(default)]
    pub nodes_file: Option<PathBuf>,

    /// Inline nodes data [[x, y, z], ...]
    #[serde(default)]
    pub nodes: Option<Vec<[f64; 3]>>,

    /// Path to elements file (JSON or CSV)
    #[serde(default)]
    pub elements_file: Option<PathBuf>,

    /// Inline elements data [[n1, n2, n3, ...], ...]
    #[serde(default)]
    pub elements: Option<Vec<Vec<usize>>>,

    /// Symmetry planes (x, y, z)
    #[serde(default)]
    pub symmetry: Option<[bool; 3]>,

    /// Symmetry origin
    #[serde(default)]
    pub symmetry_origin: Option<[f64; 3]>,
}

/// Solver configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SolverConfig {
    /// Iterative solver method
    #[serde(default)]
    pub method: SolverMethodConfig,

    /// Convergence tolerance
    #[serde(default = "default_tolerance")]
    pub tolerance: f64,

    /// Maximum iterations
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,

    /// Preconditioner type
    #[serde(default)]
    pub preconditioner: PreconditionerConfig,

    /// Print progress interval (0 = no output)
    #[serde(default)]
    pub print_interval: usize,
}

fn default_tolerance() -> f64 {
    1e-6
}
fn default_max_iterations() -> usize {
    1000
}

/// Iterative solver method
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SolverMethodConfig {
    /// Direct LU factorization
    Direct,
    /// Conjugate Gradient Squared
    #[default]
    Cgs,
    /// Bi-Conjugate Gradient Stabilized
    #[serde(alias = "bicgstab")]
    BiCgstab,
}

/// Preconditioner type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PreconditionerConfig {
    /// No preconditioning
    #[default]
    None,
    /// Diagonal (Jacobi) preconditioner
    Diagonal,
    /// Row scaling preconditioner
    RowScaling,
    /// Block diagonal preconditioner
    BlockDiagonal,
}

/// BEM method configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BemMethodConfig {
    /// BEM assembly method
    #[serde(default)]
    pub method: BemMethodType,

    /// Use Burton-Miller formulation
    #[serde(default)]
    pub burton_miller: bool,

    /// Burton-Miller coupling parameter
    #[serde(default = "default_coupling")]
    pub coupling_parameter: f64,

    /// Cluster size for FMM
    #[serde(default = "default_cluster_size")]
    pub cluster_size: usize,

    /// Expansion terms for FMM
    #[serde(default = "default_expansion_terms")]
    pub expansion_terms: usize,
}

fn default_coupling() -> f64 {
    1.0
}
fn default_cluster_size() -> usize {
    20
}
fn default_expansion_terms() -> usize {
    4
}

/// BEM assembly method type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BemMethodType {
    /// Traditional BEM (dense matrices)
    #[default]
    Traditional,
    /// Single-Level Fast Multipole Method
    #[serde(alias = "slfmm")]
    SingleLevelFmm,
    /// Multi-Level Fast Multipole Method
    #[serde(alias = "mlfmm")]
    MultiLevelFmm,
}

/// Boundary condition configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryConditionConfig {
    /// Element range (start, end) - inclusive
    pub elements: (usize, usize),

    /// Boundary condition type
    #[serde(rename = "type")]
    pub bc_type: BoundaryConditionType,

    /// Value (real part)
    pub value: f64,

    /// Value (imaginary part)
    #[serde(default)]
    pub value_imag: f64,
}

/// Boundary condition type
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BoundaryConditionType {
    /// Velocity boundary condition
    Velocity,
    /// Pressure boundary condition
    Pressure,
    /// Admittance boundary condition
    Admittance,
    /// Impedance boundary condition
    Impedance,
    /// Transfer admittance
    TransferAdmittance,
}

/// Source configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SourceConfig {
    /// Plane wave sources
    #[serde(default)]
    pub plane_waves: Vec<PlaneWaveConfig>,

    /// Point sources
    #[serde(default)]
    pub point_sources: Vec<PointSourceConfig>,
}

/// Plane wave source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaneWaveConfig {
    /// Direction vector [x, y, z] (will be normalized)
    pub direction: [f64; 3],

    /// Amplitude (complex)
    pub amplitude: f64,

    /// Phase in radians
    #[serde(default)]
    pub phase: f64,
}

/// Point source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointSourceConfig {
    /// Position [x, y, z]
    pub position: [f64; 3],

    /// Amplitude (complex)
    pub amplitude: f64,

    /// Phase in radians
    #[serde(default)]
    pub phase: f64,
}

/// Output configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OutputConfig {
    /// Output directory
    #[serde(default)]
    pub directory: Option<PathBuf>,

    /// Output pressure at evaluation points
    #[serde(default)]
    pub pressure: bool,

    /// Output velocity at evaluation points
    #[serde(default)]
    pub velocity: bool,

    /// Evaluation points file
    #[serde(default)]
    pub evaluation_points_file: Option<PathBuf>,

    /// Inline evaluation points
    #[serde(default)]
    pub evaluation_points: Option<Vec<[f64; 3]>>,
}

/// Configuration file format
#[derive(Debug, Clone, Copy)]
pub enum ConfigFormat {
    /// JSON format
    Json,
    /// TOML format
    Toml,
}

impl ConfigFormat {
    /// Detect format from file extension
    pub fn from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
        let ext = path.as_ref().extension()?.to_str()?;
        match ext.to_lowercase().as_str() {
            "json" => Some(ConfigFormat::Json),
            "toml" => Some(ConfigFormat::Toml),
            _ => None,
        }
    }
}

/// Load BEM configuration from a file
///
/// Format is auto-detected from file extension (.json or .toml)
pub fn load_config<P: AsRef<Path>>(path: P) -> Result<BemConfig, ConfigError> {
    let path = path.as_ref();
    let content = fs::read_to_string(path)?;

    let format = ConfigFormat::from_path(path)
        .ok_or_else(|| ConfigError::UnsupportedFormat(path.display().to_string()))?;

    parse_config(&content, format)
}

/// Parse BEM configuration from a string
pub fn parse_config(content: &str, format: ConfigFormat) -> Result<BemConfig, ConfigError> {
    match format {
        ConfigFormat::Json => {
            serde_json::from_str(content).map_err(|e| ConfigError::ParseError(e.to_string()))
        }
        ConfigFormat::Toml => {
            toml::from_str(content).map_err(|e| ConfigError::ParseError(e.to_string()))
        }
    }
}

/// Save BEM configuration to a file
pub fn save_config<P: AsRef<Path>>(config: &BemConfig, path: P) -> Result<(), ConfigError> {
    let path = path.as_ref();
    let format = ConfigFormat::from_path(path)
        .ok_or_else(|| ConfigError::UnsupportedFormat(path.display().to_string()))?;

    let content = serialize_config(config, format)?;
    fs::write(path, content)?;
    Ok(())
}

/// Serialize BEM configuration to a string
pub fn serialize_config(config: &BemConfig, format: ConfigFormat) -> Result<String, ConfigError> {
    match format {
        ConfigFormat::Json => serde_json::to_string_pretty(config)
            .map_err(|e| ConfigError::SerializeError(e.to_string())),
        ConfigFormat::Toml => {
            toml::to_string_pretty(config).map_err(|e| ConfigError::SerializeError(e.to_string()))
        }
    }
}

/// Configuration error types
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Parse error
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Serialize error
    #[error("Serialize error: {0}")]
    SerializeError(String),

    /// Unsupported format
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    /// Missing required field
    #[error("Missing required field: {0}")]
    MissingField(String),
}

impl BemConfig {
    /// Create PhysicsParams from configuration
    pub fn to_physics_params(&self) -> PhysicsParams {
        PhysicsParams::new(
            self.physics.frequency,
            self.physics.speed_of_sound,
            self.physics.density,
            self.physics.external_problem,
        )
    }

    /// Get BEM method from configuration
    pub fn bem_method(&self) -> BemMethod {
        match self.bem.method {
            BemMethodType::Traditional => BemMethod::Traditional,
            BemMethodType::SingleLevelFmm => BemMethod::SingleLevelFmm,
            BemMethodType::MultiLevelFmm => BemMethod::MultiLevelFmm,
        }
    }

    /// Get solver method from configuration
    pub fn solver_method(&self) -> SolverMethod {
        match self.solver.method {
            SolverMethodConfig::Direct => SolverMethod::Direct,
            SolverMethodConfig::Cgs => SolverMethod::Cgs,
            SolverMethodConfig::BiCgstab => SolverMethod::BiCgstab,
        }
    }

    /// Load nodes from configuration
    pub fn load_nodes(&self, base_dir: &Path) -> Result<Array2<f64>, ConfigError> {
        if let Some(ref nodes) = self.mesh.nodes {
            // Inline nodes
            let n = nodes.len();
            let mut arr = Array2::zeros((n, 3));
            for (i, node) in nodes.iter().enumerate() {
                arr[[i, 0]] = node[0];
                arr[[i, 1]] = node[1];
                arr[[i, 2]] = node[2];
            }
            return Ok(arr);
        }

        if let Some(ref file) = self.mesh.nodes_file {
            let path = base_dir.join(file);
            let content = fs::read_to_string(&path)?;

            // Try JSON first
            if path.extension().is_some_and(|e| e == "json") {
                let nodes: Vec<[f64; 3]> = serde_json::from_str(&content)
                    .map_err(|e| ConfigError::ParseError(e.to_string()))?;
                let n = nodes.len();
                let mut arr = Array2::zeros((n, 3));
                for (i, node) in nodes.iter().enumerate() {
                    arr[[i, 0]] = node[0];
                    arr[[i, 1]] = node[1];
                    arr[[i, 2]] = node[2];
                }
                return Ok(arr);
            }

            // Try CSV
            let nodes = parse_csv_nodes(&content)?;
            return Ok(nodes);
        }

        Err(ConfigError::MissingField(
            "mesh.nodes or mesh.nodes_file".to_string(),
        ))
    }

    /// Load elements from configuration
    pub fn load_elements(&self, base_dir: &Path) -> Result<Vec<Element>, ConfigError> {
        if let Some(ref elements) = self.mesh.elements {
            return Ok(elements_from_connectivity(elements));
        }

        if let Some(ref file) = self.mesh.elements_file {
            let path = base_dir.join(file);
            let content = fs::read_to_string(&path)?;

            // Try JSON first
            if path.extension().is_some_and(|e| e == "json") {
                let connectivity: Vec<Vec<usize>> = serde_json::from_str(&content)
                    .map_err(|e| ConfigError::ParseError(e.to_string()))?;
                return Ok(elements_from_connectivity(&connectivity));
            }

            // Try CSV
            let elements = parse_csv_elements(&content)?;
            return Ok(elements);
        }

        Err(ConfigError::MissingField(
            "mesh.elements or mesh.elements_file".to_string(),
        ))
    }
}

/// Parse CSV nodes (x y z per line)
fn parse_csv_nodes(content: &str) -> Result<Array2<f64>, ConfigError> {
    let mut nodes = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let values: Vec<f64> = line
            .split([',', ' ', '\t'])
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        if values.len() >= 3 {
            nodes.push([values[0], values[1], values[2]]);
        }
    }

    let n = nodes.len();
    let mut arr = Array2::zeros((n, 3));
    for (i, node) in nodes.iter().enumerate() {
        arr[[i, 0]] = node[0];
        arr[[i, 1]] = node[1];
        arr[[i, 2]] = node[2];
    }

    Ok(arr)
}

/// Parse CSV elements (connectivity per line)
fn parse_csv_elements(content: &str) -> Result<Vec<Element>, ConfigError> {
    let mut connectivity = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let values: Vec<usize> = line
            .split([',', ' ', '\t'])
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        if values.len() >= 3 {
            connectivity.push(values);
        }
    }

    Ok(elements_from_connectivity(&connectivity))
}

/// Create elements from connectivity data
fn elements_from_connectivity(connectivity: &[Vec<usize>]) -> Vec<Element> {
    connectivity
        .iter()
        .enumerate()
        .map(|(idx, conn)| {
            let element_type = if conn.len() == 3 {
                ElementType::Tri3
            } else {
                ElementType::Quad4
            };

            Element {
                connectivity: conn.clone(),
                element_type,
                property: ElementProperty::Surface,
                normal: Array1::zeros(3),
                node_normals: Array2::zeros((element_type.num_nodes(), 3)),
                center: Array1::zeros(3),
                area: 0.0,
                boundary_condition: BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]),
                group: 0,
                dof_addresses: vec![idx],
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_JSON: &str = r#"{
        "description": "Test BEM problem",
        "physics": {
            "frequency": 1000.0,
            "speed_of_sound": 343.0,
            "density": 1.21
        },
        "mesh": {
            "nodes": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            "elements": [[0, 1, 2]]
        },
        "solver": {
            "method": "bicgstab",
            "tolerance": 1e-8,
            "max_iterations": 500
        },
        "bem": {
            "method": "traditional",
            "burton_miller": true
        },
        "boundary_conditions": [
            {
                "elements": [0, 0],
                "type": "velocity",
                "value": 1.0
            }
        ],
        "sources": {
            "plane_waves": [
                {
                    "direction": [0.0, 0.0, -1.0],
                    "amplitude": 1.0,
                    "phase": 0.0
                }
            ]
        }
    }"#;

    const SAMPLE_TOML: &str = r#"
description = "Test BEM problem"

[physics]
frequency = 1000.0
speed_of_sound = 343.0
density = 1.21

[mesh]
nodes = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
elements = [[0, 1, 2]]

[solver]
method = "bicgstab"
tolerance = 1e-8
max_iterations = 500

[bem]
method = "traditional"
burton_miller = true

[[boundary_conditions]]
elements = [0, 0]
type = "velocity"
value = 1.0

[sources]
[[sources.plane_waves]]
direction = [0.0, 0.0, -1.0]
amplitude = 1.0
phase = 0.0
"#;

    #[test]
    fn test_parse_json() {
        let config = parse_config(SAMPLE_JSON, ConfigFormat::Json).unwrap();

        assert_eq!(config.description, "Test BEM problem");
        assert!((config.physics.frequency - 1000.0).abs() < 0.01);
        assert!((config.physics.speed_of_sound - 343.0).abs() < 0.01);
        assert_eq!(config.mesh.nodes.as_ref().unwrap().len(), 3);
        assert_eq!(config.mesh.elements.as_ref().unwrap().len(), 1);
        assert!(matches!(config.solver.method, SolverMethodConfig::BiCgstab));
        assert!(config.bem.burton_miller);
        assert_eq!(config.boundary_conditions.len(), 1);
        assert_eq!(config.sources.plane_waves.len(), 1);
    }

    #[test]
    fn test_parse_toml() {
        let config = parse_config(SAMPLE_TOML, ConfigFormat::Toml).unwrap();

        assert_eq!(config.description, "Test BEM problem");
        assert!((config.physics.frequency - 1000.0).abs() < 0.01);
        assert!((config.physics.speed_of_sound - 343.0).abs() < 0.01);
        assert_eq!(config.mesh.nodes.as_ref().unwrap().len(), 3);
    }

    #[test]
    fn test_to_physics_params() {
        let config = parse_config(SAMPLE_JSON, ConfigFormat::Json).unwrap();
        let physics = config.to_physics_params();

        assert!((physics.frequency - 1000.0).abs() < 0.01);
        assert!((physics.speed_of_sound - 343.0).abs() < 0.01);
        assert!((physics.density - 1.21).abs() < 0.01);
    }

    #[test]
    fn test_load_inline_nodes() {
        let config = parse_config(SAMPLE_JSON, ConfigFormat::Json).unwrap();
        let nodes = config.load_nodes(Path::new(".")).unwrap();

        assert_eq!(nodes.nrows(), 3);
        assert_eq!(nodes.ncols(), 3);
        assert!((nodes[[0, 0]] - 0.0).abs() < 1e-10);
        assert!((nodes[[1, 0]] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_load_inline_elements() {
        let config = parse_config(SAMPLE_JSON, ConfigFormat::Json).unwrap();
        let elements = config.load_elements(Path::new(".")).unwrap();

        assert_eq!(elements.len(), 1);
        assert!(matches!(elements[0].element_type, ElementType::Tri3));
    }

    #[test]
    fn test_serialize_json() {
        let config = parse_config(SAMPLE_JSON, ConfigFormat::Json).unwrap();
        let serialized = serialize_config(&config, ConfigFormat::Json).unwrap();

        // Verify we can parse it back
        let reparsed = parse_config(&serialized, ConfigFormat::Json).unwrap();
        assert_eq!(reparsed.description, config.description);
    }

    #[test]
    fn test_serialize_toml() {
        let config = parse_config(SAMPLE_JSON, ConfigFormat::Json).unwrap();
        let serialized = serialize_config(&config, ConfigFormat::Toml).unwrap();

        // Verify we can parse it back
        let reparsed = parse_config(&serialized, ConfigFormat::Toml).unwrap();
        assert_eq!(reparsed.description, config.description);
    }

    #[test]
    fn test_bem_method_conversion() {
        let config = parse_config(SAMPLE_JSON, ConfigFormat::Json).unwrap();
        assert!(matches!(config.bem_method(), BemMethod::Traditional));

        let json_fmm = r#"{
            "physics": {"frequency": 1000.0},
            "mesh": {"nodes": [], "elements": []},
            "bem": {"method": "slfmm"}
        }"#;
        let config_fmm = parse_config(json_fmm, ConfigFormat::Json).unwrap();
        assert!(matches!(config_fmm.bem_method(), BemMethod::SingleLevelFmm));
    }

    #[test]
    fn test_solver_method_conversion() {
        let config = parse_config(SAMPLE_JSON, ConfigFormat::Json).unwrap();
        assert!(matches!(config.solver_method(), SolverMethod::BiCgstab));
    }

    #[test]
    fn test_parse_csv_nodes() {
        let csv = "0.0 0.0 0.0\n1.0 0.0 0.0\n0.5 1.0 0.0";
        let nodes = parse_csv_nodes(csv).unwrap();

        assert_eq!(nodes.nrows(), 3);
        assert!((nodes[[1, 0]] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_parse_csv_elements() {
        let csv = "0, 1, 2\n1, 2, 3";
        let elements = parse_csv_elements(csv).unwrap();

        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].connectivity, vec![0, 1, 2]);
    }
}
