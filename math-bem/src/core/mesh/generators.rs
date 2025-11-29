//! Mesh generators for analytical test geometries
//!
//! Provides functions to generate surface meshes for standard geometries
//! used in BEM validation: spheres, cylinders, etc.

use ndarray::{Array1, Array2};
use num_complex::Complex64;
use std::f64::consts::PI;

use crate::core::types::{BoundaryCondition, Element, ElementProperty, ElementType, Mesh};

/// Generate a spherical surface mesh using UV-sphere discretization
///
/// Creates a triangulated sphere with vertices distributed along
/// latitude/longitude lines.
///
/// # Arguments
/// * `radius` - Sphere radius
/// * `n_theta` - Number of divisions in polar direction (latitude)
/// * `n_phi` - Number of divisions in azimuthal direction (longitude)
///
/// # Returns
/// A `Mesh` with triangular elements covering the sphere surface
///
/// # Example
/// ```ignore
/// let mesh = generate_sphere_mesh(1.0, 16, 32);
/// ```
pub fn generate_sphere_mesh(radius: f64, n_theta: usize, n_phi: usize) -> Mesh {
    let mut nodes = Vec::new();
    let mut elements = Vec::new();

    // Generate nodes
    // North pole
    nodes.push([0.0, 0.0, radius]);

    // Interior latitude bands
    for i in 1..n_theta {
        let theta = PI * i as f64 / n_theta as f64;
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();

        for j in 0..n_phi {
            let phi = 2.0 * PI * j as f64 / n_phi as f64;
            let x = radius * sin_theta * phi.cos();
            let y = radius * sin_theta * phi.sin();
            let z = radius * cos_theta;
            nodes.push([x, y, z]);
        }
    }

    // South pole
    nodes.push([0.0, 0.0, -radius]);

    let n_nodes = nodes.len();
    let south_pole_idx = n_nodes - 1;

    // Generate elements
    // North polar cap (triangles connecting to north pole)
    for j in 0..n_phi {
        let j_next = (j + 1) % n_phi;
        elements.push(vec![0, 1 + j, 1 + j_next]);
    }

    // Middle bands (quad → 2 triangles)
    for i in 0..(n_theta - 2) {
        let row_start = 1 + i * n_phi;
        let next_row_start = 1 + (i + 1) * n_phi;

        for j in 0..n_phi {
            let j_next = (j + 1) % n_phi;

            // Current row nodes
            let n0 = row_start + j;
            let n1 = row_start + j_next;
            // Next row nodes
            let n2 = next_row_start + j;
            let n3 = next_row_start + j_next;

            // Two triangles per quad
            elements.push(vec![n0, n2, n1]);
            elements.push(vec![n1, n2, n3]);
        }
    }

    // South polar cap
    let last_row_start = 1 + (n_theta - 2) * n_phi;
    for j in 0..n_phi {
        let j_next = (j + 1) % n_phi;
        elements.push(vec![
            last_row_start + j,
            south_pole_idx,
            last_row_start + j_next,
        ]);
    }

    create_mesh_from_data(nodes, elements, radius)
}

/// Generate an icosphere mesh (subdivided icosahedron)
///
/// More uniform element sizes than UV-sphere, better for BEM.
///
/// # Arguments
/// * `radius` - Sphere radius
/// * `subdivisions` - Number of subdivision iterations (0=icosahedron, 1=42 verts, 2=162 verts)
///
/// # Returns
/// A `Mesh` with triangular elements
pub fn generate_icosphere_mesh(radius: f64, subdivisions: usize) -> Mesh {
    // Golden ratio
    let phi = (1.0 + 5.0_f64.sqrt()) / 2.0;
    let _scale = radius / (1.0 + phi * phi).sqrt();

    // Initial icosahedron vertices
    let mut vertices: Vec<[f64; 3]> = vec![
        [-1.0, phi, 0.0],
        [1.0, phi, 0.0],
        [-1.0, -phi, 0.0],
        [1.0, -phi, 0.0],
        [0.0, -1.0, phi],
        [0.0, 1.0, phi],
        [0.0, -1.0, -phi],
        [0.0, 1.0, -phi],
        [phi, 0.0, -1.0],
        [phi, 0.0, 1.0],
        [-phi, 0.0, -1.0],
        [-phi, 0.0, 1.0],
    ];

    // Scale to unit sphere
    for v in &mut vertices {
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        v[0] /= len;
        v[1] /= len;
        v[2] /= len;
    }

    // Initial icosahedron faces
    let mut faces: Vec<[usize; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];

    // Subdivide
    for _ in 0..subdivisions {
        let mut new_faces = Vec::new();
        let mut edge_midpoints: std::collections::HashMap<(usize, usize), usize> =
            std::collections::HashMap::new();

        for face in &faces {
            let v0 = face[0];
            let v1 = face[1];
            let v2 = face[2];

            // Get or create midpoints
            let m01 = get_midpoint(&mut vertices, &mut edge_midpoints, v0, v1);
            let m12 = get_midpoint(&mut vertices, &mut edge_midpoints, v1, v2);
            let m20 = get_midpoint(&mut vertices, &mut edge_midpoints, v2, v0);

            // Create 4 new faces
            new_faces.push([v0, m01, m20]);
            new_faces.push([v1, m12, m01]);
            new_faces.push([v2, m20, m12]);
            new_faces.push([m01, m12, m20]);
        }

        faces = new_faces;
    }

    // Scale to desired radius
    let nodes: Vec<[f64; 3]> = vertices
        .iter()
        .map(|v| [v[0] * radius, v[1] * radius, v[2] * radius])
        .collect();

    let elements: Vec<Vec<usize>> = faces.iter().map(|f| vec![f[0], f[1], f[2]]).collect();

    create_mesh_from_data(nodes, elements, radius)
}

/// Helper for icosphere: get or create edge midpoint
fn get_midpoint(
    vertices: &mut Vec<[f64; 3]>,
    cache: &mut std::collections::HashMap<(usize, usize), usize>,
    v0: usize,
    v1: usize,
) -> usize {
    let key = if v0 < v1 { (v0, v1) } else { (v1, v0) };

    if let Some(&idx) = cache.get(&key) {
        return idx;
    }

    // Create new vertex at midpoint, projected to unit sphere
    let mid = [
        (vertices[v0][0] + vertices[v1][0]) / 2.0,
        (vertices[v0][1] + vertices[v1][1]) / 2.0,
        (vertices[v0][2] + vertices[v1][2]) / 2.0,
    ];

    let len = (mid[0] * mid[0] + mid[1] * mid[1] + mid[2] * mid[2]).sqrt();
    let normalized = [mid[0] / len, mid[1] / len, mid[2] / len];

    let idx = vertices.len();
    vertices.push(normalized);
    cache.insert(key, idx);

    idx
}

/// Generate a cylindrical surface mesh
///
/// Creates a mesh for a finite cylinder (without end caps).
///
/// # Arguments
/// * `radius` - Cylinder radius
/// * `height` - Cylinder height (centered at z=0)
/// * `n_circumference` - Number of divisions around circumference
/// * `n_height` - Number of divisions along height
///
/// # Returns
/// A `Mesh` with quadrilateral elements
pub fn generate_cylinder_mesh(
    radius: f64,
    height: f64,
    n_circumference: usize,
    n_height: usize,
) -> Mesh {
    let mut nodes = Vec::new();
    let mut elements = Vec::new();

    let z_min = -height / 2.0;
    let dz = height / n_height as f64;

    // Generate nodes
    for i in 0..=n_height {
        let z = z_min + i as f64 * dz;

        for j in 0..n_circumference {
            let phi = 2.0 * PI * j as f64 / n_circumference as f64;
            let x = radius * phi.cos();
            let y = radius * phi.sin();
            nodes.push([x, y, z]);
        }
    }

    // Generate quad elements
    for i in 0..n_height {
        let row_start = i * n_circumference;
        let next_row_start = (i + 1) * n_circumference;

        for j in 0..n_circumference {
            let j_next = (j + 1) % n_circumference;

            let n0 = row_start + j;
            let n1 = row_start + j_next;
            let n2 = next_row_start + j_next;
            let n3 = next_row_start + j;

            elements.push(vec![n0, n1, n2, n3]);
        }
    }

    create_mesh_from_data(nodes, elements, radius)
}

/// Generate a cylinder mesh with end caps (closed cylinder)
pub fn generate_closed_cylinder_mesh(
    radius: f64,
    height: f64,
    n_circumference: usize,
    n_height: usize,
    n_cap_rings: usize,
) -> Mesh {
    let mut nodes = Vec::new();
    let mut elements = Vec::new();

    let z_min = -height / 2.0;
    let z_max = height / 2.0;
    let dz = height / n_height as f64;

    // Lateral surface nodes
    for i in 0..=n_height {
        let z = z_min + i as f64 * dz;
        for j in 0..n_circumference {
            let phi = 2.0 * PI * j as f64 / n_circumference as f64;
            nodes.push([radius * phi.cos(), radius * phi.sin(), z]);
        }
    }

    let _lateral_node_count = nodes.len();

    // Bottom cap center
    let bottom_center_idx = nodes.len();
    nodes.push([0.0, 0.0, z_min]);

    // Bottom cap rings
    for ring in 1..=n_cap_rings {
        let r = radius * ring as f64 / n_cap_rings as f64;
        for j in 0..n_circumference {
            let phi = 2.0 * PI * j as f64 / n_circumference as f64;
            nodes.push([r * phi.cos(), r * phi.sin(), z_min]);
        }
    }

    // Top cap center
    let top_center_idx = nodes.len();
    nodes.push([0.0, 0.0, z_max]);

    // Top cap rings
    for ring in 1..=n_cap_rings {
        let r = radius * ring as f64 / n_cap_rings as f64;
        for j in 0..n_circumference {
            let phi = 2.0 * PI * j as f64 / n_circumference as f64;
            nodes.push([r * phi.cos(), r * phi.sin(), z_max]);
        }
    }

    // Lateral surface elements (quads)
    for i in 0..n_height {
        let row_start = i * n_circumference;
        let next_row_start = (i + 1) * n_circumference;
        for j in 0..n_circumference {
            let j_next = (j + 1) % n_circumference;
            elements.push(vec![
                row_start + j,
                row_start + j_next,
                next_row_start + j_next,
                next_row_start + j,
            ]);
        }
    }

    // Bottom cap elements (triangles to center, then quads between rings)
    // Center triangles
    let bottom_ring1_start = bottom_center_idx + 1;
    for j in 0..n_circumference {
        let j_next = (j + 1) % n_circumference;
        elements.push(vec![
            bottom_center_idx,
            bottom_ring1_start + j_next,
            bottom_ring1_start + j,
        ]);
    }

    // Ring quads (bottom)
    for ring in 0..(n_cap_rings - 1) {
        let ring_start = bottom_center_idx + 1 + ring * n_circumference;
        let next_ring_start = ring_start + n_circumference;
        for j in 0..n_circumference {
            let j_next = (j + 1) % n_circumference;
            elements.push(vec![
                ring_start + j,
                ring_start + j_next,
                next_ring_start + j_next,
                next_ring_start + j,
            ]);
        }
    }

    // Connect outer bottom ring to lateral surface
    let outer_bottom_ring = bottom_center_idx + 1 + (n_cap_rings - 1) * n_circumference;
    for j in 0..n_circumference {
        let j_next = (j + 1) % n_circumference;
        elements.push(vec![
            outer_bottom_ring + j,
            outer_bottom_ring + j_next,
            j_next,
            j,
        ]);
    }

    // Top cap (similar structure, opposite winding)
    let top_ring1_start = top_center_idx + 1;
    for j in 0..n_circumference {
        let j_next = (j + 1) % n_circumference;
        elements.push(vec![
            top_center_idx,
            top_ring1_start + j,
            top_ring1_start + j_next,
        ]);
    }

    // Ring quads (top)
    for ring in 0..(n_cap_rings - 1) {
        let ring_start = top_center_idx + 1 + ring * n_circumference;
        let next_ring_start = ring_start + n_circumference;
        for j in 0..n_circumference {
            let j_next = (j + 1) % n_circumference;
            elements.push(vec![
                ring_start + j,
                next_ring_start + j,
                next_ring_start + j_next,
                ring_start + j_next,
            ]);
        }
    }

    // Connect outer top ring to lateral surface
    let outer_top_ring = top_center_idx + 1 + (n_cap_rings - 1) * n_circumference;
    let top_lateral_row = n_height * n_circumference;
    for j in 0..n_circumference {
        let j_next = (j + 1) % n_circumference;
        elements.push(vec![
            top_lateral_row + j,
            top_lateral_row + j_next,
            outer_top_ring + j_next,
            outer_top_ring + j,
        ]);
    }

    create_mesh_from_data(nodes, elements, radius)
}

/// Create a Mesh struct from raw node and element data
fn create_mesh_from_data(
    nodes: Vec<[f64; 3]>,
    connectivity: Vec<Vec<usize>>,
    _char_length: f64,
) -> Mesh {
    let n_nodes = nodes.len();
    let n_elements = connectivity.len();

    // Create node array
    let mut node_array = Array2::zeros((n_nodes, 3));
    for (i, node) in nodes.iter().enumerate() {
        node_array[[i, 0]] = node[0];
        node_array[[i, 1]] = node[1];
        node_array[[i, 2]] = node[2];
    }

    // Create elements
    let mut elements = Vec::with_capacity(n_elements);
    for (idx, conn) in connectivity.iter().enumerate() {
        let element_type = if conn.len() == 3 {
            ElementType::Tri3
        } else {
            ElementType::Quad4
        };

        let mut elem = Element {
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
        };

        // Compute geometry
        compute_element_geometry(&mut elem, &node_array);
        elements.push(elem);
    }

    // Compute node normals (average of adjacent element normals)
    let mut node_counts = vec![0usize; n_nodes];

    for elem in &elements {
        for &node_idx in &elem.connectivity {
            node_counts[node_idx] += 1;
        }
    }

    // Update element node normals
    for elem in &mut elements {
        for (local_idx, &_node_idx) in elem.connectivity.iter().enumerate() {
            // For sphere/cylinder, normal at node ≈ element normal (constant elements)
            for j in 0..3 {
                elem.node_normals[[local_idx, j]] = elem.normal[j];
            }
        }
    }

    Mesh {
        nodes: node_array,
        elements,
        external_node_numbers: (0..n_nodes as i32).collect(),
        external_element_numbers: (0..n_elements as i32).collect(),
        num_boundary_nodes: n_nodes,
        num_evaluation_nodes: 0,
        num_boundary_elements: n_elements,
        num_evaluation_elements: 0,
        symmetry_planes: [0, 0, 0],
        symmetry_coordinates: [0.0, 0.0, 0.0],
        num_reflections: 0,
    }
}

/// Compute element center, area, and normal
fn compute_element_geometry(elem: &mut Element, nodes: &Array2<f64>) {
    let n = elem.connectivity.len();

    // Compute center
    elem.center = Array1::zeros(3);
    for &node_idx in &elem.connectivity {
        for j in 0..3 {
            elem.center[j] += nodes[[node_idx, j]];
        }
    }
    for j in 0..3 {
        elem.center[j] /= n as f64;
    }

    // Compute normal and area
    if n == 3 {
        // Triangle
        let n0 = elem.connectivity[0];
        let n1 = elem.connectivity[1];
        let n2 = elem.connectivity[2];

        let v1 = [
            nodes[[n1, 0]] - nodes[[n0, 0]],
            nodes[[n1, 1]] - nodes[[n0, 1]],
            nodes[[n1, 2]] - nodes[[n0, 2]],
        ];
        let v2 = [
            nodes[[n2, 0]] - nodes[[n0, 0]],
            nodes[[n2, 1]] - nodes[[n0, 1]],
            nodes[[n2, 2]] - nodes[[n0, 2]],
        ];

        // Cross product
        let cross = [
            v1[1] * v2[2] - v1[2] * v2[1],
            v1[2] * v2[0] - v1[0] * v2[2],
            v1[0] * v2[1] - v1[1] * v2[0],
        ];

        let len = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
        elem.area = len / 2.0;

        if len > 1e-15 {
            elem.normal = Array1::from_vec(vec![cross[0] / len, cross[1] / len, cross[2] / len]);
        }
    } else {
        // Quad - use diagonal cross product
        let n0 = elem.connectivity[0];
        let n1 = elem.connectivity[1];
        let n2 = elem.connectivity[2];
        let n3 = elem.connectivity[3];

        let d1 = [
            nodes[[n2, 0]] - nodes[[n0, 0]],
            nodes[[n2, 1]] - nodes[[n0, 1]],
            nodes[[n2, 2]] - nodes[[n0, 2]],
        ];
        let d2 = [
            nodes[[n3, 0]] - nodes[[n1, 0]],
            nodes[[n3, 1]] - nodes[[n1, 1]],
            nodes[[n3, 2]] - nodes[[n1, 2]],
        ];

        let cross = [
            d1[1] * d2[2] - d1[2] * d2[1],
            d1[2] * d2[0] - d1[0] * d2[2],
            d1[0] * d2[1] - d1[1] * d2[0],
        ];

        let len = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
        elem.area = len / 2.0; // Approximate area

        if len > 1e-15 {
            elem.normal = Array1::from_vec(vec![cross[0] / len, cross[1] / len, cross[2] / len]);
        }
    }

    // Ensure normal points outward (for convex shapes centered at origin)
    // Check: normal should point in same direction as center (n · c > 0)
    let n_dot_c = elem.normal[0] * elem.center[0]
        + elem.normal[1] * elem.center[1]
        + elem.normal[2] * elem.center[2];

    if n_dot_c < 0.0 {
        // Flip normal to point outward
        elem.normal[0] = -elem.normal[0];
        elem.normal[1] = -elem.normal[1];
        elem.normal[2] = -elem.normal[2];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sphere_mesh_generation() {
        let mesh = generate_sphere_mesh(1.0, 8, 16);

        assert!(mesh.nodes.nrows() > 0);
        assert!(!mesh.elements.is_empty());

        // Check all nodes are on the sphere
        for i in 0..mesh.nodes.nrows() {
            let r = (mesh.nodes[[i, 0]].powi(2)
                + mesh.nodes[[i, 1]].powi(2)
                + mesh.nodes[[i, 2]].powi(2))
            .sqrt();
            assert!((r - 1.0).abs() < 1e-10, "Node {} not on sphere: r={}", i, r);
        }

        // Check elements are valid triangles
        for elem in &mesh.elements {
            assert_eq!(elem.connectivity.len(), 3);
            assert!(elem.area > 0.0);
        }
    }

    #[test]
    fn test_icosphere_mesh_generation() {
        let mesh = generate_icosphere_mesh(1.0, 2);

        // Subdivision 2 should give 162 vertices
        assert_eq!(mesh.nodes.nrows(), 162);

        // Check all nodes are on unit sphere
        for i in 0..mesh.nodes.nrows() {
            let r = (mesh.nodes[[i, 0]].powi(2)
                + mesh.nodes[[i, 1]].powi(2)
                + mesh.nodes[[i, 2]].powi(2))
            .sqrt();
            assert!((r - 1.0).abs() < 1e-10);
        }
    }

    #[test]
    fn test_cylinder_mesh_generation() {
        let mesh = generate_cylinder_mesh(1.0, 2.0, 16, 8);

        assert!(mesh.nodes.nrows() > 0);
        assert!(!mesh.elements.is_empty());

        // Check all nodes are on cylinder surface
        for i in 0..mesh.nodes.nrows() {
            let r = (mesh.nodes[[i, 0]].powi(2) + mesh.nodes[[i, 1]].powi(2)).sqrt();
            assert!(
                (r - 1.0).abs() < 1e-10,
                "Node {} not on cylinder: r={}",
                i,
                r
            );
        }

        // Check elements are valid quads
        for elem in &mesh.elements {
            assert_eq!(elem.connectivity.len(), 4);
            assert!(elem.area > 0.0);
        }
    }

    #[test]
    fn test_sphere_normals_point_outward() {
        let mesh = generate_icosphere_mesh(1.0, 1);

        for elem in &mesh.elements {
            // Normal should point away from origin (outward)
            let dot = elem.normal[0] * elem.center[0]
                + elem.normal[1] * elem.center[1]
                + elem.normal[2] * elem.center[2];
            assert!(dot > 0.0, "Normal should point outward");
        }
    }

    #[test]
    fn test_sphere_surface_area() {
        // Higher resolution for accurate area
        let mesh = generate_icosphere_mesh(1.0, 4);

        let total_area: f64 = mesh.elements.iter().map(|e| e.area).sum();
        let expected_area = 4.0 * PI; // Surface area of unit sphere

        let error = (total_area - expected_area).abs() / expected_area;
        assert!(error < 0.01, "Surface area error: {:.2}%", error * 100.0);
    }
}
