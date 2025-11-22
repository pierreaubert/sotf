//! Core data types for Mesh2HRTF integration
//!
//! This module defines the fundamental data structures for representing
//! meshes, evaluation grids, sources, and HRTF data in the Mesh2HRTF pipeline.

use nalgebra::{Point3, Vector3};
use serde::{Deserialize, Serialize};

/// A 3D point in space
pub type Point = Point3<f64>;

/// A 3D vector
pub type Vec3 = Vector3<f64>;

/// A 3D node (vertex) in a mesh
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// Node ID (0-indexed)
    pub id: usize,
    /// 3D position
    pub position: Point,
}

impl Node {
    /// Create a new node
    pub fn new(id: usize, position: Point) -> Self {
        Self { id, position }
    }

    /// Create a node from coordinates
    pub fn from_coords(id: usize, x: f64, y: f64, z: f64) -> Self {
        Self {
            id,
            position: Point::new(x, y, z),
        }
    }
}

/// A triangular element in a mesh
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Element {
    /// Element ID (0-indexed)
    pub id: usize,
    /// Material ID
    pub material_id: usize,
    /// Vertex indices [v1, v2, v3]
    pub vertices: [usize; 3],
}

impl Element {
    /// Create a new element
    pub fn new(id: usize, material_id: usize, vertices: [usize; 3]) -> Self {
        Self {
            id,
            material_id,
            vertices,
        }
    }
}

/// A 3D mesh consisting of nodes and triangular elements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mesh {
    /// Mesh nodes (vertices)
    pub nodes: Vec<Node>,
    /// Triangular elements (faces)
    pub elements: Vec<Element>,
    /// Optional metadata
    pub metadata: MeshMetadata,
}

impl Mesh {
    /// Create a new empty mesh
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            elements: Vec::new(),
            metadata: MeshMetadata::default(),
        }
    }

    /// Get the number of nodes
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Get the number of elements
    pub fn num_elements(&self) -> usize {
        self.elements.len()
    }

    /// Get a node by ID
    pub fn get_node(&self, id: usize) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// Get an element by ID
    pub fn get_element(&self, id: usize) -> Option<&Element> {
        self.elements.get(id)
    }

    /// Validate mesh integrity
    ///
    /// Checks:
    /// - All vertex indices in elements exist
    /// - No degenerate triangles
    /// - Node IDs are unique (not necessarily sequential for evaluation grids)
    pub fn validate(&self) -> Result<(), String> {
        // Check node IDs are unique (but allow non-sequential for evaluation grids)
        // Build a map of node IDs to indices for element validation
        let mut node_id_to_index = std::collections::HashMap::new();
        for (i, node) in self.nodes.iter().enumerate() {
            if node_id_to_index.insert(node.id, i).is_some() {
                return Err(format!("Duplicate node ID: {}", node.id));
            }
        }

        // Check element references
        for element in &self.elements {
            for &vertex_id in &element.vertices {
                if !node_id_to_index.contains_key(&vertex_id) {
                    return Err(format!(
                        "Element {} references non-existent vertex {}",
                        element.id, vertex_id
                    ));
                }
            }

            // Check for degenerate triangles
            let [v0, v1, v2] = element.vertices;
            if v0 == v1 || v1 == v2 || v2 == v0 {
                return Err(format!("Element {} is degenerate", element.id));
            }
        }

        Ok(())
    }

    /// Compute mesh bounding box
    pub fn bounding_box(&self) -> (Point, Point) {
        if self.nodes.is_empty() {
            return (Point::origin(), Point::origin());
        }

        let mut min = self.nodes[0].position;
        let mut max = self.nodes[0].position;

        for node in &self.nodes {
            min.x = min.x.min(node.position.x);
            min.y = min.y.min(node.position.y);
            min.z = min.z.min(node.position.z);

            max.x = max.x.max(node.position.x);
            max.y = max.y.max(node.position.y);
            max.z = max.z.max(node.position.z);
        }

        (min, max)
    }

    /// Get mesh center (centroid of bounding box)
    pub fn center(&self) -> Point {
        let (min, max) = self.bounding_box();
        Point::new(
            (min.x + max.x) / 2.0,
            (min.y + max.y) / 2.0,
            (min.z + max.z) / 2.0,
        )
    }

    /// Get all elements with a specific material ID
    pub fn elements_with_material(&self, material_id: usize) -> Vec<&Element> {
        self.elements
            .iter()
            .filter(|e| e.material_id == material_id)
            .collect()
    }
}

impl Default for Mesh {
    fn default() -> Self {
        Self::new()
    }
}

/// Mesh metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MeshMetadata {
    /// Mesh name/title
    pub name: Option<String>,
    /// Creation date
    pub created: Option<String>,
    /// Source file
    pub source: Option<String>,
    /// Additional properties
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// Evaluation grid types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GridType {
    /// Horizontal plane at a given height
    HorizontalPlane { z: f64, radius: f64, points: usize },
    /// Vertical plane at a specific azimuth angle
    VerticalPlane {
        angle: f64,
        radius: f64,
        points: usize,
    },
    /// Spherical grid around the head
    Sphere { radius: f64, points: usize },
    /// Custom grid from file
    Custom { path: std::path::PathBuf },
}

/// Evaluation grid
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationGrid {
    /// Grid name
    pub name: String,
    /// Grid type
    pub grid_type: GridType,
    /// Grid nodes
    pub nodes: Vec<Node>,
    /// Grid elements (optional, for surface grids)
    pub elements: Vec<Element>,
}

impl EvaluationGrid {
    /// Create a new evaluation grid
    pub fn new(name: String, grid_type: GridType) -> Self {
        Self {
            name,
            grid_type,
            nodes: Vec::new(),
            elements: Vec::new(),
        }
    }

    /// Get number of evaluation points
    pub fn num_points(&self) -> usize {
        self.nodes.len()
    }

    /// Convert evaluation grid to mesh format
    pub fn to_mesh(&self) -> Mesh {
        Mesh {
            nodes: self.nodes.clone(),
            elements: self.elements.clone(),
            metadata: MeshMetadata {
                name: Some(format!("Evaluation grid: {}", self.name)),
                created: None,
                source: None,
                extra: {
                    let mut map = std::collections::HashMap::new();
                    map.insert("units".to_string(), serde_json::json!("m"));
                    map
                },
            },
        }
    }
}

/// Source types for BEM simulation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SourceType {
    /// Both ears as velocity sources
    BothEars {
        /// Material ID for left ear elements
        left_material: usize,
        /// Material ID for right ear elements
        right_material: usize,
    },
    /// Left ear only
    LeftEar {
        /// Material ID for left ear elements
        material: usize,
    },
    /// Right ear only
    RightEar {
        /// Material ID for right ear elements
        material: usize,
    },
    /// Point source (analytical)
    PointSource {
        /// Source position
        position: Point,
    },
    /// Plane wave (analytical)
    PlaneWave {
        /// Direction of incidence
        direction: Vec3,
    },
}

/// BEM method selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BemMethod {
    /// Traditional BEM (O(n²))
    Bem,
    /// Single-level Fast Multipole Method
    SlFmmBem,
    /// Multi-level Fast Multipole Method (recommended)
    MlFmmBem,
}

impl Default for BemMethod {
    fn default() -> Self {
        Self::MlFmmBem
    }
}

impl std::fmt::Display for BemMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Bem => write!(f, "BEM"),
            Self::SlFmmBem => write!(f, "SL-FMM BEM"),
            Self::MlFmmBem => write!(f, "ML-FMM BEM"),
        }
    }
}

/// Physical parameters for simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicalParameters {
    /// Speed of sound (m/s)
    pub speed_of_sound: f64,
    /// Density of medium (kg/m³)
    pub density: f64,
}

impl Default for PhysicalParameters {
    fn default() -> Self {
        Self {
            speed_of_sound: 343.0, // Air at 20°C
            density: 1.1839,        // Air at 20°C
        }
    }
}

/// NumCalc project configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumCalcProject {
    /// Project title
    pub title: String,
    /// BEM method
    pub method: BemMethod,
    /// Source configuration
    pub source_type: SourceType,
    /// Object mesh (head)
    pub object_mesh: Mesh,
    /// Evaluation grids
    pub evaluation_grids: Vec<EvaluationGrid>,
    /// Simulation frequencies (Hz)
    pub frequencies: Vec<f64>,
    /// Physical parameters
    pub physical_params: PhysicalParameters,
    /// Additional configuration
    pub config: NumCalcConfig,
}

/// NumCalc configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumCalcConfig {
    /// Maximum iterations
    pub max_iterations: usize,
    /// Convergence tolerance
    pub tolerance: f64,
    /// Enable reference to head center
    pub reference: bool,
    /// Compute HRIRs
    pub compute_hrirs: bool,
    /// Sampling rate for HRIRs (Hz)
    pub sample_rate: Option<f64>,
}

impl Default for NumCalcConfig {
    fn default() -> Self {
        Self {
            max_iterations: 250,
            tolerance: 1e-3,
            reference: false,
            compute_hrirs: false,
            sample_rate: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_creation() {
        let node = Node::from_coords(0, 1.0, 2.0, 3.0);
        assert_eq!(node.id, 0);
        assert_eq!(node.position.x, 1.0);
        assert_eq!(node.position.y, 2.0);
        assert_eq!(node.position.z, 3.0);
    }

    #[test]
    fn test_element_creation() {
        let element = Element::new(0, 1, [0, 1, 2]);
        assert_eq!(element.id, 0);
        assert_eq!(element.material_id, 1);
        assert_eq!(element.vertices, [0, 1, 2]);
    }

    #[test]
    fn test_mesh_validation() {
        let mut mesh = Mesh::new();
        mesh.nodes = vec![
            Node::from_coords(0, 0.0, 0.0, 0.0),
            Node::from_coords(1, 1.0, 0.0, 0.0),
            Node::from_coords(2, 0.0, 1.0, 0.0),
        ];
        mesh.elements = vec![Element::new(0, 0, [0, 1, 2])];

        assert!(mesh.validate().is_ok());
    }

    #[test]
    fn test_mesh_validation_missing_vertex() {
        let mut mesh = Mesh::new();
        mesh.nodes = vec![
            Node::from_coords(0, 0.0, 0.0, 0.0),
            Node::from_coords(1, 1.0, 0.0, 0.0),
        ];
        mesh.elements = vec![Element::new(0, 0, [0, 1, 999])]; // Vertex 999 doesn't exist

        assert!(mesh.validate().is_err());
    }

    #[test]
    fn test_mesh_validation_non_sequential_ids() {
        // Evaluation grids can have non-sequential node IDs (e.g., starting at 300000)
        let mut mesh = Mesh::new();
        mesh.nodes = vec![
            Node::from_coords(300000, 0.0, 0.0, 0.0),
            Node::from_coords(300001, 1.0, 0.0, 0.0),
            Node::from_coords(300002, 0.0, 1.0, 0.0),
        ];
        mesh.elements = vec![Element::new(300000, 0, [300000, 300001, 300002])];

        assert!(mesh.validate().is_ok());
    }

    #[test]
    fn test_mesh_validation_duplicate_node_id() {
        let mut mesh = Mesh::new();
        mesh.nodes = vec![
            Node::from_coords(0, 0.0, 0.0, 0.0),
            Node::from_coords(0, 1.0, 0.0, 0.0), // Duplicate ID
        ];

        assert!(mesh.validate().is_err());
    }

    #[test]
    fn test_mesh_validation_degenerate() {
        let mut mesh = Mesh::new();
        mesh.nodes = vec![
            Node::from_coords(0, 0.0, 0.0, 0.0),
            Node::from_coords(1, 1.0, 0.0, 0.0),
            Node::from_coords(2, 0.0, 1.0, 0.0),
        ];
        mesh.elements = vec![Element::new(0, 0, [0, 0, 1])]; // Degenerate

        assert!(mesh.validate().is_err());
    }

    #[test]
    fn test_mesh_bounding_box() {
        let mut mesh = Mesh::new();
        mesh.nodes = vec![
            Node::from_coords(0, 0.0, 0.0, 0.0),
            Node::from_coords(1, 1.0, 2.0, 3.0),
            Node::from_coords(2, -1.0, -1.0, -1.0),
        ];

        let (min, max) = mesh.bounding_box();
        assert_eq!(min, Point::new(-1.0, -1.0, -1.0));
        assert_eq!(max, Point::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_bem_method_display() {
        assert_eq!(BemMethod::Bem.to_string(), "BEM");
        assert_eq!(BemMethod::SlFmmBem.to_string(), "SL-FMM BEM");
        assert_eq!(BemMethod::MlFmmBem.to_string(), "ML-FMM BEM");
    }
}
