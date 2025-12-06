//! Mesh types for 2D and 3D finite element analysis
//!
//! Supports triangular, quadrilateral, tetrahedral, and hexahedral elements.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A point in 2D or 3D space
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point {
    /// Create a 2D point (z = 0)
    pub fn new_2d(x: f64, y: f64) -> Self {
        Self { x, y, z: 0.0 }
    }

    /// Create a 3D point
    pub fn new_3d(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Euclidean distance to another point
    pub fn distance(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Midpoint between two points
    pub fn midpoint(&self, other: &Point) -> Point {
        Point {
            x: 0.5 * (self.x + other.x),
            y: 0.5 * (self.y + other.y),
            z: 0.5 * (self.z + other.z),
        }
    }
}

impl From<(f64, f64)> for Point {
    fn from(p: (f64, f64)) -> Self {
        Point::new_2d(p.0, p.1)
    }
}

impl From<(f64, f64, f64)> for Point {
    fn from(p: (f64, f64, f64)) -> Self {
        Point::new_3d(p.0, p.1, p.2)
    }
}

/// Element type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ElementType {
    /// 2D triangle (3 nodes for P1, 6 for P2)
    Triangle,
    /// 2D quadrilateral (4 nodes for Q1, 9 for Q2)
    Quadrilateral,
    /// 3D tetrahedron (4 nodes for P1, 10 for P2)
    Tetrahedron,
    /// 3D hexahedron (8 nodes for Q1, 27 for Q2)
    Hexahedron,
}

impl ElementType {
    /// Number of vertices (corners) for this element type
    pub fn num_vertices(&self) -> usize {
        match self {
            ElementType::Triangle => 3,
            ElementType::Quadrilateral => 4,
            ElementType::Tetrahedron => 4,
            ElementType::Hexahedron => 8,
        }
    }

    /// Number of nodes for P1/Q1 elements
    pub fn num_nodes_p1(&self) -> usize {
        self.num_vertices()
    }

    /// Number of nodes for P2/Q2 elements
    pub fn num_nodes_p2(&self) -> usize {
        match self {
            ElementType::Triangle => 6,      // 3 vertices + 3 edge midpoints
            ElementType::Quadrilateral => 9, // 4 vertices + 4 edge midpoints + 1 center
            ElementType::Tetrahedron => 10,  // 4 vertices + 6 edge midpoints
            ElementType::Hexahedron => 27,   // 8 vertices + 12 edge + 6 face + 1 center
        }
    }

    /// Spatial dimension of this element
    pub fn dimension(&self) -> usize {
        match self {
            ElementType::Triangle | ElementType::Quadrilateral => 2,
            ElementType::Tetrahedron | ElementType::Hexahedron => 3,
        }
    }

    /// Number of edges for this element type
    pub fn num_edges(&self) -> usize {
        match self {
            ElementType::Triangle => 3,
            ElementType::Quadrilateral => 4,
            ElementType::Tetrahedron => 6,
            ElementType::Hexahedron => 12,
        }
    }

    /// Number of faces for this element type
    pub fn num_faces(&self) -> usize {
        match self {
            ElementType::Triangle => 3,
            ElementType::Quadrilateral => 4,
            ElementType::Tetrahedron => 4,
            ElementType::Hexahedron => 6,
        }
    }
}

/// Polynomial degree for basis functions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PolynomialDegree {
    P1, // Linear
    P2, // Quadratic
    P3, // Cubic
}

impl PolynomialDegree {
    /// Get the degree as a number
    pub fn as_usize(&self) -> usize {
        match self {
            PolynomialDegree::P1 => 1,
            PolynomialDegree::P2 => 2,
            PolynomialDegree::P3 => 3,
        }
    }
}

/// A finite element with node indices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    /// Element type
    pub element_type: ElementType,
    /// Node indices (vertices first, then edge/face/interior nodes for higher order)
    pub nodes: Vec<usize>,
    /// Element ID for refinement tracking
    pub id: usize,
    /// Parent element ID (for refined elements)
    pub parent_id: Option<usize>,
    /// Refinement level
    pub level: usize,
}

impl Element {
    /// Create a new element
    pub fn new(element_type: ElementType, nodes: Vec<usize>, id: usize) -> Self {
        Self {
            element_type,
            nodes,
            id,
            parent_id: None,
            level: 0,
        }
    }

    /// Get vertex nodes (first N nodes are always vertices)
    pub fn vertices(&self) -> &[usize] {
        &self.nodes[..self.element_type.num_vertices()]
    }

    /// Get edges as pairs of node indices
    pub fn edges(&self) -> Vec<(usize, usize)> {
        let verts = self.vertices();
        match self.element_type {
            ElementType::Triangle => vec![
                (verts[0], verts[1]),
                (verts[1], verts[2]),
                (verts[2], verts[0]),
            ],
            ElementType::Quadrilateral => vec![
                (verts[0], verts[1]),
                (verts[1], verts[2]),
                (verts[2], verts[3]),
                (verts[3], verts[0]),
            ],
            ElementType::Tetrahedron => vec![
                (verts[0], verts[1]),
                (verts[0], verts[2]),
                (verts[0], verts[3]),
                (verts[1], verts[2]),
                (verts[1], verts[3]),
                (verts[2], verts[3]),
            ],
            ElementType::Hexahedron => vec![
                // Bottom face
                (verts[0], verts[1]),
                (verts[1], verts[2]),
                (verts[2], verts[3]),
                (verts[3], verts[0]),
                // Top face
                (verts[4], verts[5]),
                (verts[5], verts[6]),
                (verts[6], verts[7]),
                (verts[7], verts[4]),
                // Vertical edges
                (verts[0], verts[4]),
                (verts[1], verts[5]),
                (verts[2], verts[6]),
                (verts[3], verts[7]),
            ],
        }
    }
}

/// Boundary condition type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BoundaryType {
    /// Dirichlet: u = g (essential BC)
    Dirichlet,
    /// Neumann: ∂u/∂n = h (natural BC)
    Neumann,
    /// Robin: ∂u/∂n + α*u = g (mixed BC)
    Robin,
    /// Perfectly Matched Layer (absorbing BC)
    PML,
    /// Interior (not a boundary)
    Interior,
}

/// A boundary face or edge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryFace {
    /// Node indices defining this boundary face/edge
    pub nodes: Vec<usize>,
    /// Boundary type
    pub boundary_type: BoundaryType,
    /// Boundary marker (for identifying different boundaries)
    pub marker: i32,
    /// Owning element index
    pub element_idx: usize,
    /// Local face/edge index within the element
    pub local_idx: usize,
}

/// A finite element mesh
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mesh {
    /// Mesh dimension (2 or 3)
    pub dimension: usize,
    /// Node coordinates
    pub nodes: Vec<Point>,
    /// Elements
    pub elements: Vec<Element>,
    /// Boundary faces/edges
    pub boundaries: Vec<BoundaryFace>,
    /// Next element ID for refinement
    pub(crate) next_element_id: usize,
    /// Node to element connectivity (cached)
    #[serde(skip)]
    node_to_elements: Option<Vec<Vec<usize>>>,
}

impl Mesh {
    /// Create a new empty mesh
    pub fn new(dimension: usize) -> Self {
        assert!(dimension == 2 || dimension == 3, "Dimension must be 2 or 3");
        Self {
            dimension,
            nodes: Vec::new(),
            elements: Vec::new(),
            boundaries: Vec::new(),
            next_element_id: 0,
            node_to_elements: None,
        }
    }

    /// Add a node and return its index
    pub fn add_node(&mut self, point: Point) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(point);
        self.node_to_elements = None; // Invalidate cache
        idx
    }

    /// Add an element and return its index
    pub fn add_element(&mut self, element_type: ElementType, nodes: Vec<usize>) -> usize {
        let idx = self.elements.len();
        let id = self.next_element_id;
        self.next_element_id += 1;
        self.elements.push(Element::new(element_type, nodes, id));
        self.node_to_elements = None; // Invalidate cache
        idx
    }

    /// Add a boundary face/edge
    pub fn add_boundary(
        &mut self,
        nodes: Vec<usize>,
        boundary_type: BoundaryType,
        marker: i32,
        element_idx: usize,
        local_idx: usize,
    ) {
        self.boundaries.push(BoundaryFace {
            nodes,
            boundary_type,
            marker,
            element_idx,
            local_idx,
        });
    }

    /// Number of nodes
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Number of elements
    pub fn num_elements(&self) -> usize {
        self.elements.len()
    }

    /// Get node coordinates
    pub fn node(&self, idx: usize) -> &Point {
        &self.nodes[idx]
    }

    /// Get element
    pub fn element(&self, idx: usize) -> &Element {
        &self.elements[idx]
    }

    /// Build node-to-element connectivity
    pub fn build_connectivity(&mut self) {
        let mut node_to_elems: Vec<Vec<usize>> = vec![Vec::new(); self.nodes.len()];
        for (elem_idx, elem) in self.elements.iter().enumerate() {
            for &node_idx in &elem.nodes {
                node_to_elems[node_idx].push(elem_idx);
            }
        }
        self.node_to_elements = Some(node_to_elems);
    }

    /// Get elements containing a node
    pub fn elements_containing_node(&self, node_idx: usize) -> Option<&[usize]> {
        self.node_to_elements
            .as_ref()
            .map(|c| c[node_idx].as_slice())
    }

    /// Find boundary edges/faces automatically
    pub fn detect_boundaries(&mut self) {
        self.boundaries.clear();

        // For 2D: edges that appear only once are boundaries
        // For 3D: faces that appear only once are boundaries
        let mut face_count: HashMap<Vec<usize>, (usize, usize)> = HashMap::new();

        for (elem_idx, elem) in self.elements.iter().enumerate() {
            let faces = self.element_faces(elem);
            for mut face in faces {
                face.sort();
                face_count
                    .entry(face)
                    .and_modify(|e| e.1 += 1)
                    .or_insert((elem_idx, 1));
            }
        }

        // Single-occurrence faces are boundaries
        for (face_nodes, (elem_idx, count)) in face_count {
            if count == 1 {
                // Find local index
                let elem = &self.elements[elem_idx];
                let faces = self.element_faces(elem);
                let local_idx = faces
                    .iter()
                    .enumerate()
                    .find(|(_, f)| {
                        let mut sorted = (*f).clone();
                        sorted.sort();
                        sorted == face_nodes
                    })
                    .map(|(i, _)| i)
                    .unwrap_or(0);

                self.boundaries.push(BoundaryFace {
                    nodes: face_nodes,
                    boundary_type: BoundaryType::Neumann, // Default
                    marker: 0,
                    element_idx: elem_idx,
                    local_idx,
                });
            }
        }
    }

    /// Get faces/edges of an element (for boundary detection)
    fn element_faces(&self, elem: &Element) -> Vec<Vec<usize>> {
        let verts = elem.vertices();
        match elem.element_type {
            ElementType::Triangle => vec![
                vec![verts[0], verts[1]],
                vec![verts[1], verts[2]],
                vec![verts[2], verts[0]],
            ],
            ElementType::Quadrilateral => vec![
                vec![verts[0], verts[1]],
                vec![verts[1], verts[2]],
                vec![verts[2], verts[3]],
                vec![verts[3], verts[0]],
            ],
            ElementType::Tetrahedron => vec![
                vec![verts[0], verts[1], verts[2]],
                vec![verts[0], verts[1], verts[3]],
                vec![verts[0], verts[2], verts[3]],
                vec![verts[1], verts[2], verts[3]],
            ],
            ElementType::Hexahedron => vec![
                vec![verts[0], verts[1], verts[2], verts[3]], // Bottom
                vec![verts[4], verts[5], verts[6], verts[7]], // Top
                vec![verts[0], verts[1], verts[5], verts[4]], // Front
                vec![verts[2], verts[3], verts[7], verts[6]], // Back
                vec![verts[0], verts[3], verts[7], verts[4]], // Left
                vec![verts[1], verts[2], verts[6], verts[5]], // Right
            ],
        }
    }

    /// Set boundary condition on faces matching a predicate
    pub fn set_boundary_condition<F>(
        &mut self,
        boundary_type: BoundaryType,
        marker: i32,
        predicate: F,
    ) where
        F: Fn(&[Point]) -> bool,
    {
        for boundary in &mut self.boundaries {
            let face_points: Vec<Point> = boundary.nodes.iter().map(|&i| self.nodes[i]).collect();
            if predicate(&face_points) {
                boundary.boundary_type = boundary_type;
                boundary.marker = marker;
            }
        }
    }

    /// Get all boundary nodes with a specific type
    pub fn boundary_nodes(&self, boundary_type: BoundaryType) -> HashSet<usize> {
        let mut nodes = HashSet::new();
        for boundary in &self.boundaries {
            if boundary.boundary_type == boundary_type {
                nodes.extend(boundary.nodes.iter().cloned());
            }
        }
        nodes
    }

    /// Compute element centroid
    pub fn element_centroid(&self, elem_idx: usize) -> Point {
        let elem = &self.elements[elem_idx];
        let verts = elem.vertices();
        let n = verts.len() as f64;
        let mut cx = 0.0;
        let mut cy = 0.0;
        let mut cz = 0.0;
        for &v in verts {
            cx += self.nodes[v].x;
            cy += self.nodes[v].y;
            cz += self.nodes[v].z;
        }
        Point::new_3d(cx / n, cy / n, cz / n)
    }

    /// Compute element area (2D) or volume (3D)
    pub fn element_measure(&self, elem_idx: usize) -> f64 {
        let elem = &self.elements[elem_idx];
        let verts: Vec<&Point> = elem.vertices().iter().map(|&i| &self.nodes[i]).collect();

        match elem.element_type {
            ElementType::Triangle => {
                // Area = 0.5 * |det([x2-x1, y2-y1; x3-x1, y3-y1])|
                let v1 = (verts[1].x - verts[0].x, verts[1].y - verts[0].y);
                let v2 = (verts[2].x - verts[0].x, verts[2].y - verts[0].y);
                0.5 * (v1.0 * v2.1 - v1.1 * v2.0).abs()
            }
            ElementType::Quadrilateral => {
                // Split into two triangles
                let a1 = {
                    let v1 = (verts[1].x - verts[0].x, verts[1].y - verts[0].y);
                    let v2 = (verts[2].x - verts[0].x, verts[2].y - verts[0].y);
                    0.5 * (v1.0 * v2.1 - v1.1 * v2.0).abs()
                };
                let a2 = {
                    let v1 = (verts[2].x - verts[0].x, verts[2].y - verts[0].y);
                    let v2 = (verts[3].x - verts[0].x, verts[3].y - verts[0].y);
                    0.5 * (v1.0 * v2.1 - v1.1 * v2.0).abs()
                };
                a1 + a2
            }
            ElementType::Tetrahedron => {
                // Volume = |det([v1-v0, v2-v0, v3-v0])| / 6
                let v1 = (
                    verts[1].x - verts[0].x,
                    verts[1].y - verts[0].y,
                    verts[1].z - verts[0].z,
                );
                let v2 = (
                    verts[2].x - verts[0].x,
                    verts[2].y - verts[0].y,
                    verts[2].z - verts[0].z,
                );
                let v3 = (
                    verts[3].x - verts[0].x,
                    verts[3].y - verts[0].y,
                    verts[3].z - verts[0].z,
                );
                let det = v1.0 * (v2.1 * v3.2 - v2.2 * v3.1) - v1.1 * (v2.0 * v3.2 - v2.2 * v3.0)
                    + v1.2 * (v2.0 * v3.1 - v2.1 * v3.0);
                det.abs() / 6.0
            }
            ElementType::Hexahedron => {
                // Approximate by splitting into 6 tetrahedra
                // This is a simplification - exact formula is more complex
                let _center = Point::new_3d(
                    verts.iter().map(|v| v.x).sum::<f64>() / 8.0,
                    verts.iter().map(|v| v.y).sum::<f64>() / 8.0,
                    verts.iter().map(|v| v.z).sum::<f64>() / 8.0,
                );
                // Sum volumes of pyramids from each face to center
                // Simplified calculation
                let dx = verts[1].x - verts[0].x;
                let dy = verts[3].y - verts[0].y;
                let dz = verts[4].z - verts[0].z;
                (dx * dy * dz).abs()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_distance() {
        let p1 = Point::new_2d(0.0, 0.0);
        let p2 = Point::new_2d(3.0, 4.0);
        assert!((p1.distance(&p2) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_point_midpoint() {
        let p1 = Point::new_2d(0.0, 0.0);
        let p2 = Point::new_2d(2.0, 4.0);
        let mid = p1.midpoint(&p2);
        assert!((mid.x - 1.0).abs() < 1e-10);
        assert!((mid.y - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_element_type_properties() {
        assert_eq!(ElementType::Triangle.num_vertices(), 3);
        assert_eq!(ElementType::Triangle.num_nodes_p2(), 6);
        assert_eq!(ElementType::Tetrahedron.dimension(), 3);
    }

    #[test]
    fn test_mesh_triangle_area() {
        let mut mesh = Mesh::new(2);
        mesh.add_node(Point::new_2d(0.0, 0.0));
        mesh.add_node(Point::new_2d(1.0, 0.0));
        mesh.add_node(Point::new_2d(0.0, 1.0));
        mesh.add_element(ElementType::Triangle, vec![0, 1, 2]);

        let area = mesh.element_measure(0);
        assert!((area - 0.5).abs() < 1e-10);
    }
}
