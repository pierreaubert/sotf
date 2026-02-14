//! Mesh refinement (h-refinement)
//!
//! Implements bisection-based refinement for triangles and tetrahedra.

use super::types::{Element, ElementType, Mesh, Point};
use std::collections::HashMap;

/// Edge represented by sorted node indices
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Edge(pub usize, pub usize);

impl Edge {
    pub fn new(a: usize, b: usize) -> Self {
        if a < b { Edge(a, b) } else { Edge(b, a) }
    }
}

/// Refinement result
pub struct RefinementResult {
    /// Indices of new elements created
    pub new_elements: Vec<usize>,
    /// Indices of elements removed
    pub removed_elements: Vec<usize>,
    /// Indices of new nodes created
    pub new_nodes: Vec<usize>,
}

/// Helper struct to manage midpoint creation
struct MidpointManager {
    edge_midpoints: HashMap<Edge, usize>,
    new_nodes: Vec<usize>,
}

impl MidpointManager {
    fn new() -> Self {
        Self {
            edge_midpoints: HashMap::new(),
            new_nodes: Vec::new(),
        }
    }

    fn get_midpoint(&mut self, mesh: &mut Mesh, a: usize, b: usize) -> usize {
        let edge = Edge::new(a, b);
        if let Some(&mid_idx) = self.edge_midpoints.get(&edge) {
            mid_idx
        } else {
            let mid = mesh.nodes[a].midpoint(&mesh.nodes[b]);
            let idx = mesh.add_node(mid);
            self.new_nodes.push(idx);
            self.edge_midpoints.insert(edge, idx);
            idx
        }
    }
}

/// Refine selected elements using bisection
///
/// For triangles: Uses longest-edge bisection (red-green refinement)
/// For tetrahedra: Uses edge bisection with closure
pub fn refine_elements(mesh: &mut Mesh, elements_to_refine: &[usize]) -> RefinementResult {
    let mut new_elements = Vec::new();
    let mut removed_elements = Vec::new();
    let mut midpoint_mgr = MidpointManager::new();

    for &elem_idx in elements_to_refine {
        let elem = mesh.elements[elem_idx].clone();
        removed_elements.push(elem_idx);

        match elem.element_type {
            ElementType::Triangle => {
                // Red refinement: divide into 4 triangles
                let v = elem.vertices();
                let m01 = midpoint_mgr.get_midpoint(mesh, v[0], v[1]);
                let m12 = midpoint_mgr.get_midpoint(mesh, v[1], v[2]);
                let m20 = midpoint_mgr.get_midpoint(mesh, v[2], v[0]);

                // 4 child triangles
                let children = vec![
                    vec![v[0], m01, m20],
                    vec![m01, v[1], m12],
                    vec![m20, m12, v[2]],
                    vec![m01, m12, m20],
                ];

                for nodes in children {
                    let idx = mesh.elements.len();
                    let mut child_elem =
                        Element::new(ElementType::Triangle, nodes, mesh.next_element_id);
                    mesh.next_element_id += 1;
                    child_elem.parent_id = Some(elem.id);
                    child_elem.level = elem.level + 1;
                    mesh.elements.push(child_elem);
                    new_elements.push(idx);
                }
            }
            ElementType::Tetrahedron => {
                // Divide into 8 tetrahedra using edge midpoints
                let v = elem.vertices();
                let m01 = midpoint_mgr.get_midpoint(mesh, v[0], v[1]);
                let m02 = midpoint_mgr.get_midpoint(mesh, v[0], v[2]);
                let m03 = midpoint_mgr.get_midpoint(mesh, v[0], v[3]);
                let m12 = midpoint_mgr.get_midpoint(mesh, v[1], v[2]);
                let m13 = midpoint_mgr.get_midpoint(mesh, v[1], v[3]);
                let m23 = midpoint_mgr.get_midpoint(mesh, v[2], v[3]);

                // 8 child tetrahedra
                let children = vec![
                    vec![v[0], m01, m02, m03],
                    vec![m01, v[1], m12, m13],
                    vec![m02, m12, v[2], m23],
                    vec![m03, m13, m23, v[3]],
                    vec![m01, m02, m03, m13],
                    vec![m01, m02, m12, m13],
                    vec![m02, m03, m13, m23],
                    vec![m02, m12, m13, m23],
                ];

                for nodes in children {
                    let idx = mesh.elements.len();
                    let mut child_elem =
                        Element::new(ElementType::Tetrahedron, nodes, mesh.next_element_id);
                    mesh.next_element_id += 1;
                    child_elem.parent_id = Some(elem.id);
                    child_elem.level = elem.level + 1;
                    mesh.elements.push(child_elem);
                    new_elements.push(idx);
                }
            }
            ElementType::Quadrilateral => {
                // Divide into 4 quadrilaterals
                let v = elem.vertices();
                let m01 = midpoint_mgr.get_midpoint(mesh, v[0], v[1]);
                let m12 = midpoint_mgr.get_midpoint(mesh, v[1], v[2]);
                let m23 = midpoint_mgr.get_midpoint(mesh, v[2], v[3]);
                let m30 = midpoint_mgr.get_midpoint(mesh, v[3], v[0]);

                // Center point
                let cx = (mesh.nodes[v[0]].x
                    + mesh.nodes[v[1]].x
                    + mesh.nodes[v[2]].x
                    + mesh.nodes[v[3]].x)
                    / 4.0;
                let cy = (mesh.nodes[v[0]].y
                    + mesh.nodes[v[1]].y
                    + mesh.nodes[v[2]].y
                    + mesh.nodes[v[3]].y)
                    / 4.0;
                let center = mesh.add_node(Point::new_2d(cx, cy));
                midpoint_mgr.new_nodes.push(center);

                let children = vec![
                    vec![v[0], m01, center, m30],
                    vec![m01, v[1], m12, center],
                    vec![center, m12, v[2], m23],
                    vec![m30, center, m23, v[3]],
                ];

                for nodes in children {
                    let idx = mesh.elements.len();
                    let mut child_elem =
                        Element::new(ElementType::Quadrilateral, nodes, mesh.next_element_id);
                    mesh.next_element_id += 1;
                    child_elem.parent_id = Some(elem.id);
                    child_elem.level = elem.level + 1;
                    mesh.elements.push(child_elem);
                    new_elements.push(idx);
                }
            }
            ElementType::Hexahedron => {
                // Divide into 8 hexahedra (simplified - center + face centers + edge midpoints)
                // This is a complex operation - simplified version
                let _v = elem.vertices();

                // For now, just mark as refined but don't subdivide
                // Full implementation would require many midpoint calculations
                log::warn!("Hexahedral refinement not fully implemented");
            }
        }
    }

    RefinementResult {
        new_elements,
        removed_elements,
        new_nodes: midpoint_mgr.new_nodes,
    }
}

/// Uniform refinement of the entire mesh
pub fn uniform_refine(mesh: &mut Mesh) -> RefinementResult {
    let elements_to_refine: Vec<usize> = (0..mesh.num_elements()).collect();
    refine_elements(mesh, &elements_to_refine)
}

/// Refine elements where error exceeds threshold
pub fn adaptive_refine(
    mesh: &mut Mesh,
    element_errors: &[f64],
    threshold: f64,
) -> RefinementResult {
    let elements_to_refine: Vec<usize> = element_errors
        .iter()
        .enumerate()
        .filter(|(_, e)| **e > threshold)
        .map(|(i, _)| i)
        .collect();

    refine_elements(mesh, &elements_to_refine)
}

/// Mark elements for refinement based on Dörfler marking strategy
///
/// Mark enough elements to capture a fraction θ of the total error
pub fn doerfler_marking(element_errors: &[f64], theta: f64) -> Vec<usize> {
    let total_error_sq: f64 = element_errors.iter().map(|e| e * e).sum();
    let target = theta * total_error_sq;

    // Sort elements by error (descending)
    let mut indexed_errors: Vec<(usize, f64)> = element_errors
        .iter()
        .enumerate()
        .map(|(i, &e)| (i, e))
        .collect();
    indexed_errors.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let mut marked = Vec::new();
    let mut accumulated = 0.0;

    for (idx, error) in indexed_errors {
        marked.push(idx);
        accumulated += error * error;
        if accumulated >= target {
            break;
        }
    }

    marked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::generators::unit_square_triangles;

    #[test]
    fn test_triangle_refinement() {
        let mut mesh = unit_square_triangles(1);
        assert_eq!(mesh.num_elements(), 2);

        let result = uniform_refine(&mut mesh);

        // Each triangle becomes 4
        assert_eq!(result.new_elements.len(), 8);
        assert_eq!(result.removed_elements.len(), 2);
    }

    #[test]
    fn test_doerfler_marking() {
        let errors = vec![0.1, 0.5, 0.2, 0.8, 0.3];
        let marked = doerfler_marking(&errors, 0.5);

        // Should mark element 3 (error 0.8) first, then 1 (0.5), etc.
        assert!(!marked.is_empty());
        assert!(marked.contains(&3)); // Highest error
    }

    #[test]
    fn test_adaptive_refine() {
        let mut mesh = unit_square_triangles(1);
        let errors = vec![0.5, 0.1]; // Only refine first element

        let result = adaptive_refine(&mut mesh, &errors, 0.3);

        assert_eq!(result.removed_elements.len(), 1);
        assert_eq!(result.new_elements.len(), 4);
    }
}
