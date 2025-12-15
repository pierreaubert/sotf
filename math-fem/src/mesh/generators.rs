//! Mesh generators for common domains
//!
//! Provides functions to create structured meshes for rectangles, boxes, and circles.

use super::types::{BoundaryType, ElementType, Mesh, Point};
use std::collections::HashMap;

/// Generate a rectangular mesh with triangular elements
pub fn rectangular_mesh_triangles(
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
    nx: usize,
    ny: usize,
) -> Mesh {
    let mut mesh = Mesh::new(2);

    let dx = (x_max - x_min) / nx as f64;
    let dy = (y_max - y_min) / ny as f64;

    // Create nodes
    for j in 0..=ny {
        for i in 0..=nx {
            let x = x_min + i as f64 * dx;
            let y = y_min + j as f64 * dy;
            mesh.add_node(Point::new_2d(x, y));
        }
    }

    // Create triangular elements (2 triangles per cell)
    for j in 0..ny {
        for i in 0..nx {
            let n00 = j * (nx + 1) + i;
            let n10 = n00 + 1;
            let n01 = n00 + (nx + 1);
            let n11 = n01 + 1;

            // Two triangles per cell
            mesh.add_element(ElementType::Triangle, vec![n00, n10, n11]);
            mesh.add_element(ElementType::Triangle, vec![n00, n11, n01]);
        }
    }

    // Detect boundaries
    mesh.detect_boundaries();

    // Set boundary conditions based on position
    let tol = 1e-10;
    mesh.set_boundary_condition(BoundaryType::Dirichlet, 1, |points| {
        points.iter().all(|p| (p.x - x_min).abs() < tol)
    });
    mesh.set_boundary_condition(BoundaryType::Dirichlet, 2, |points| {
        points.iter().all(|p| (p.x - x_max).abs() < tol)
    });
    mesh.set_boundary_condition(BoundaryType::Neumann, 3, |points| {
        points.iter().all(|p| (p.y - y_min).abs() < tol)
    });
    mesh.set_boundary_condition(BoundaryType::Neumann, 4, |points| {
        points.iter().all(|p| (p.y - y_max).abs() < tol)
    });

    mesh
}

/// Generate a rectangular mesh with quadrilateral elements
pub fn rectangular_mesh_quads(
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
    nx: usize,
    ny: usize,
) -> Mesh {
    let mut mesh = Mesh::new(2);

    let dx = (x_max - x_min) / nx as f64;
    let dy = (y_max - y_min) / ny as f64;

    // Create nodes
    for j in 0..=ny {
        for i in 0..=nx {
            let x = x_min + i as f64 * dx;
            let y = y_min + j as f64 * dy;
            mesh.add_node(Point::new_2d(x, y));
        }
    }

    // Create quadrilateral elements
    for j in 0..ny {
        for i in 0..nx {
            let n00 = j * (nx + 1) + i;
            let n10 = n00 + 1;
            let n01 = n00 + (nx + 1);
            let n11 = n01 + 1;

            mesh.add_element(ElementType::Quadrilateral, vec![n00, n10, n11, n01]);
        }
    }

    mesh.detect_boundaries();
    mesh
}

/// Generate a box mesh with tetrahedral elements
pub fn box_mesh_tetrahedra(
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
    z_min: f64,
    z_max: f64,
    nx: usize,
    ny: usize,
    nz: usize,
) -> Mesh {
    let mut mesh = Mesh::new(3);

    let dx = (x_max - x_min) / nx as f64;
    let dy = (y_max - y_min) / ny as f64;
    let dz = (z_max - z_min) / nz as f64;

    // Create nodes
    for k in 0..=nz {
        for j in 0..=ny {
            for i in 0..=nx {
                let x = x_min + i as f64 * dx;
                let y = y_min + j as f64 * dy;
                let z = z_min + k as f64 * dz;
                mesh.add_node(Point::new_3d(x, y, z));
            }
        }
    }

    // Node indexing function
    let node_idx =
        |i: usize, j: usize, k: usize| -> usize { k * (ny + 1) * (nx + 1) + j * (nx + 1) + i };

    // Create tetrahedral elements (6 tetrahedra per cube)
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                // 8 corners of the cube
                let n000 = node_idx(i, j, k);
                let n100 = node_idx(i + 1, j, k);
                let n010 = node_idx(i, j + 1, k);
                let n110 = node_idx(i + 1, j + 1, k);
                let n001 = node_idx(i, j, k + 1);
                let n101 = node_idx(i + 1, j, k + 1);
                let n011 = node_idx(i, j + 1, k + 1);
                let n111 = node_idx(i + 1, j + 1, k + 1);

                // Divide cube into 6 tetrahedra (Kuhn triangulation)
                mesh.add_element(ElementType::Tetrahedron, vec![n000, n100, n110, n111]);
                mesh.add_element(ElementType::Tetrahedron, vec![n000, n110, n010, n111]);
                mesh.add_element(ElementType::Tetrahedron, vec![n000, n010, n011, n111]);
                mesh.add_element(ElementType::Tetrahedron, vec![n000, n011, n001, n111]);
                mesh.add_element(ElementType::Tetrahedron, vec![n000, n001, n101, n111]);
                mesh.add_element(ElementType::Tetrahedron, vec![n000, n101, n100, n111]);
            }
        }
    }

    mesh.detect_boundaries();
    mesh
}

/// Generate a box mesh with hexahedral elements
pub fn box_mesh_hexahedra(
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
    z_min: f64,
    z_max: f64,
    nx: usize,
    ny: usize,
    nz: usize,
) -> Mesh {
    let mut mesh = Mesh::new(3);

    let dx = (x_max - x_min) / nx as f64;
    let dy = (y_max - y_min) / ny as f64;
    let dz = (z_max - z_min) / nz as f64;

    // Create nodes
    for k in 0..=nz {
        for j in 0..=ny {
            for i in 0..=nx {
                let x = x_min + i as f64 * dx;
                let y = y_min + j as f64 * dy;
                let z = z_min + k as f64 * dz;
                mesh.add_node(Point::new_3d(x, y, z));
            }
        }
    }

    // Node indexing function
    let node_idx =
        |i: usize, j: usize, k: usize| -> usize { k * (ny + 1) * (nx + 1) + j * (nx + 1) + i };

    // Create hexahedral elements
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let n000 = node_idx(i, j, k);
                let n100 = node_idx(i + 1, j, k);
                let n010 = node_idx(i, j + 1, k);
                let n110 = node_idx(i + 1, j + 1, k);
                let n001 = node_idx(i, j, k + 1);
                let n101 = node_idx(i + 1, j, k + 1);
                let n011 = node_idx(i, j + 1, k + 1);
                let n111 = node_idx(i + 1, j + 1, k + 1);

                mesh.add_element(
                    ElementType::Hexahedron,
                    vec![n000, n100, n110, n010, n001, n101, n111, n011],
                );
            }
        }
    }

    mesh.detect_boundaries();
    mesh
}

/// Generate a circular mesh with triangular elements
pub fn circular_mesh_triangles(
    center_x: f64,
    center_y: f64,
    radius: f64,
    n_radial: usize,
    n_angular: usize,
) -> Mesh {
    let mut mesh = Mesh::new(2);

    // Center node
    mesh.add_node(Point::new_2d(center_x, center_y));

    // Radial layers of nodes
    for r in 1..=n_radial {
        let rad = radius * (r as f64) / (n_radial as f64);
        for a in 0..n_angular {
            let theta = 2.0 * std::f64::consts::PI * (a as f64) / (n_angular as f64);
            let x = center_x + rad * theta.cos();
            let y = center_y + rad * theta.sin();
            mesh.add_node(Point::new_2d(x, y));
        }
    }

    // Inner ring (triangles from center)
    for a in 0..n_angular {
        let n1 = 1 + a;
        let n2 = 1 + (a + 1) % n_angular;
        mesh.add_element(ElementType::Triangle, vec![0, n1, n2]);
    }

    // Outer rings (quadrilaterals split into triangles)
    for r in 1..n_radial {
        let offset_inner = 1 + (r - 1) * n_angular;
        let offset_outer = 1 + r * n_angular;
        for a in 0..n_angular {
            let n00 = offset_inner + a;
            let n10 = offset_inner + (a + 1) % n_angular;
            let n01 = offset_outer + a;
            let n11 = offset_outer + (a + 1) % n_angular;

            mesh.add_element(ElementType::Triangle, vec![n00, n10, n11]);
            mesh.add_element(ElementType::Triangle, vec![n00, n11, n01]);
        }
    }

    mesh.detect_boundaries();

    // Set outer boundary
    let tol = radius * 0.001;
    mesh.set_boundary_condition(BoundaryType::Dirichlet, 1, |points| {
        let avg_r = points
            .iter()
            .map(|p| ((p.x - center_x).powi(2) + (p.y - center_y).powi(2)).sqrt())
            .sum::<f64>()
            / points.len() as f64;
        (avg_r - radius).abs() < tol
    });

    mesh
}

/// Generate an annular (ring) mesh with triangular elements
pub fn annular_mesh_triangles(
    center_x: f64,
    center_y: f64,
    inner_radius: f64,
    outer_radius: f64,
    n_radial: usize,
    n_angular: usize,
) -> Mesh {
    let mut mesh = Mesh::new(2);

    // Create nodes in radial layers from inner to outer radius
    let dr = (outer_radius - inner_radius) / (n_radial as f64);

    for r in 0..=n_radial {
        let rad = inner_radius + r as f64 * dr;
        for a in 0..n_angular {
            let theta = 2.0 * std::f64::consts::PI * (a as f64) / (n_angular as f64);
            let x = center_x + rad * theta.cos();
            let y = center_y + rad * theta.sin();
            mesh.add_node(Point::new_2d(x, y));
        }
    }

    // Create triangular elements (2 per quad cell)
    for r in 0..n_radial {
        let offset_inner = r * n_angular;
        let offset_outer = (r + 1) * n_angular;
        for a in 0..n_angular {
            let n00 = offset_inner + a;
            let n10 = offset_inner + (a + 1) % n_angular;
            let n01 = offset_outer + a;
            let n11 = offset_outer + (a + 1) % n_angular;

            mesh.add_element(ElementType::Triangle, vec![n00, n10, n11]);
            mesh.add_element(ElementType::Triangle, vec![n00, n11, n01]);
        }
    }

    mesh.detect_boundaries();

    // Set inner boundary (obstacle) - tag 1
    let tol = inner_radius * 0.01;
    mesh.set_boundary_condition(BoundaryType::Dirichlet, 1, |points| {
        let avg_r = points
            .iter()
            .map(|p| ((p.x - center_x).powi(2) + (p.y - center_y).powi(2)).sqrt())
            .sum::<f64>()
            / points.len() as f64;
        (avg_r - inner_radius).abs() < tol
    });

    // Set outer boundary (far-field) - tag 2
    let tol_outer = outer_radius * 0.01;
    mesh.set_boundary_condition(BoundaryType::Dirichlet, 2, |points| {
        let avg_r = points
            .iter()
            .map(|p| ((p.x - center_x).powi(2) + (p.y - center_y).powi(2)).sqrt())
            .sum::<f64>()
            / points.len() as f64;
        (avg_r - outer_radius).abs() < tol_outer
    });

    mesh
}

/// Generate a spherical shell mesh using extruded icosphere (Tetrahedra)
///
/// Creates a 3D volume mesh between two concentric spheres.
/// Based on subdivisions of an icosahedron, extruded radially.
pub fn spherical_shell_mesh_tetrahedra(
    center_x: f64,
    center_y: f64,
    center_z: f64,
    inner_radius: f64,
    outer_radius: f64,
    subdivisions: usize,
    n_layers: usize,
) -> Mesh {
    let mut mesh = Mesh::new(3);

    // 1. Generate unit icosphere surface (nodes and triangles)
    let (base_nodes, base_tris) = generate_icosphere(subdivisions);
    let num_surface_nodes = base_nodes.len();

    // 2. Create nodes for all layers
    let dr = (outer_radius - inner_radius) / n_layers as f64;
    for l in 0..=n_layers {
        let r = inner_radius + l as f64 * dr;
        for p in &base_nodes {
            mesh.add_node(Point::new_3d(
                center_x + p.x * r,
                center_y + p.y * r,
                center_z + p.z * r,
            ));
        }
    }

    // 3. Create elements connecting layers
    for l in 0..n_layers {
        let offset_lower = l * num_surface_nodes;
        let offset_upper = (l + 1) * num_surface_nodes;

        for tri in &base_tris {
            let u0 = offset_lower + tri[0];
            let u1 = offset_lower + tri[1];
            let u2 = offset_lower + tri[2];

            let v0 = offset_upper + tri[0];
            let v1 = offset_upper + tri[1];
            let v2 = offset_upper + tri[2];

            // Split prism (u0,u1,u2)-(v0,v1,v2) into 3 tetrahedra
            // We use a consistent split based on sorting the base indices
            split_prism_consistent(&mut mesh, [u0, u1, u2], [v0, v1, v2]);
        }
    }

    mesh.detect_boundaries();

    // Set BCs
    let tol = inner_radius * 0.01;
    mesh.set_boundary_condition(BoundaryType::Dirichlet, 1, |pts| {
        // Inner
        pts[0].distance(&Point::new_3d(center_x, center_y, center_z)) < inner_radius + tol
    });
    mesh.set_boundary_condition(BoundaryType::Dirichlet, 2, |pts| {
        // Outer
        pts[0].distance(&Point::new_3d(center_x, center_y, center_z)) > outer_radius - tol
    });

    mesh
}

/// Helper: split a prism into 3 tetrahedra consistently
fn split_prism_consistent(mesh: &mut Mesh, lower: [usize; 3], upper: [usize; 3]) {
    // Sort base indices to define a local canonical order
    let mut perm = [0, 1, 2];
    perm.sort_by_key(|&i| lower[i]);

    let p0 = perm[0];
    let p1 = perm[1];
    let p2 = perm[2];

    // Canonical vertices
    let v0 = lower[p0];
    let v1 = lower[p1];
    let v2 = lower[p2];
    let v3 = upper[p0];
    let v4 = upper[p1];
    let v5 = upper[p2];

    // Standard split for 0<1<2
    // T1: 0-1-2-5 (base tri + top-max)
    mesh.add_element(ElementType::Tetrahedron, vec![v0, v1, v2, v5]);
    // T2: 0-1-5-4
    mesh.add_element(ElementType::Tetrahedron, vec![v0, v1, v5, v4]);
    // T3: 0-4-5-3
    mesh.add_element(ElementType::Tetrahedron, vec![v0, v4, v5, v3]);
}

/// Helper: generate unit icosphere
fn generate_icosphere(subdivisions: usize) -> (Vec<Point>, Vec<[usize; 3]>) {
    // Basic Icosahedron
    let t = (1.0 + 5.0f64.sqrt()) / 2.0;
    let mut nodes = vec![
        Point::new_3d(-1.0, t, 0.0),
        Point::new_3d(1.0, t, 0.0),
        Point::new_3d(-1.0, -t, 0.0),
        Point::new_3d(1.0, -t, 0.0),
        Point::new_3d(0.0, -1.0, t),
        Point::new_3d(0.0, 1.0, t),
        Point::new_3d(0.0, -1.0, -t),
        Point::new_3d(0.0, 1.0, -t),
        Point::new_3d(t, 0.0, -1.0),
        Point::new_3d(t, 0.0, 1.0),
        Point::new_3d(-t, 0.0, -1.0),
        Point::new_3d(-t, 0.0, 1.0),
    ];

    // Normalize
    for p in &mut nodes {
        let len = (p.x * p.x + p.y * p.y + p.z * p.z).sqrt();
        p.x /= len;
        p.y /= len;
        p.z /= len;
    }

    let mut tris = vec![
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
        let mut new_tris = Vec::new();
        let mut mid_cache = HashMap::new();

        for tri in tris {
            let v0 = tri[0];
            let v1 = tri[1];
            let v2 = tri[2];

            // Get midpoints (and add to nodes if new)
            // Note: can't use closure easily with mutable borrows, using helper function pattern
            // But we need to define helper inside or outside.
            // Since this is a standalone function, we can put logic in loop.

            let a = get_middle_point(v0, v1, &mut nodes, &mut mid_cache);
            let b = get_middle_point(v1, v2, &mut nodes, &mut mid_cache);
            let c = get_middle_point(v2, v0, &mut nodes, &mut mid_cache);

            new_tris.push([v0, a, c]);
            new_tris.push([v1, b, a]);
            new_tris.push([v2, c, b]);
            new_tris.push([a, b, c]);
        }
        tris = new_tris;
    }

    (nodes, tris)
}

fn get_middle_point(
    p1: usize,
    p2: usize,
    nodes: &mut Vec<Point>,
    cache: &mut HashMap<(usize, usize), usize>,
) -> usize {
    let key = if p1 < p2 { (p1, p2) } else { (p2, p1) };
    if let Some(&idx) = cache.get(&key) {
        return idx;
    }

    let pt1 = nodes[p1];
    let pt2 = nodes[p2];
    let mut middle = Point::new_3d(
        (pt1.x + pt2.x) / 2.0,
        (pt1.y + pt2.y) / 2.0,
        (pt1.z + pt2.z) / 2.0,
    );

    let len = (middle.x * middle.x + middle.y * middle.y + middle.z * middle.z).sqrt();
    middle.x /= len;
    middle.y /= len;
    middle.z /= len;

    let idx = nodes.len();
    nodes.push(middle);
    cache.insert(key, idx);
    idx
}

/// Generate a unit square mesh with triangles
pub fn unit_square_triangles(n: usize) -> Mesh {
    rectangular_mesh_triangles(0.0, 1.0, 0.0, 1.0, n, n)
}

/// Generate a unit square mesh with quadrilaterals
pub fn unit_square_quads(n: usize) -> Mesh {
    rectangular_mesh_quads(0.0, 1.0, 0.0, 1.0, n, n)
}

/// Generate a unit cube mesh with tetrahedra
pub fn unit_cube_tetrahedra(n: usize) -> Mesh {
    box_mesh_tetrahedra(0.0, 1.0, 0.0, 1.0, 0.0, 1.0, n, n, n)
}

/// Generate a unit cube mesh with hexahedra
pub fn unit_cube_hexahedra(n: usize) -> Mesh {
    box_mesh_hexahedra(0.0, 1.0, 0.0, 1.0, 0.0, 1.0, n, n, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rectangular_mesh_triangles() {
        let mesh = rectangular_mesh_triangles(0.0, 1.0, 0.0, 1.0, 2, 2);

        // 3x3 = 9 nodes
        assert_eq!(mesh.num_nodes(), 9);
        // 2x2 cells x 2 triangles = 8 triangles
        assert_eq!(mesh.num_elements(), 8);
    }

    #[test]
    fn test_rectangular_mesh_quads() {
        let mesh = rectangular_mesh_quads(0.0, 1.0, 0.0, 1.0, 3, 3);

        assert_eq!(mesh.num_nodes(), 16);
        assert_eq!(mesh.num_elements(), 9);
    }

    #[test]
    fn test_box_mesh_tetrahedra() {
        let mesh = box_mesh_tetrahedra(0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 2, 2, 2);

        // 3x3x3 = 27 nodes
        assert_eq!(mesh.num_nodes(), 27);
        // 2x2x2 cubes x 6 tetrahedra = 48 tetrahedra
        assert_eq!(mesh.num_elements(), 48);
    }

    #[test]
    fn test_circular_mesh() {
        let mesh = circular_mesh_triangles(0.0, 0.0, 1.0, 3, 8);

        // Center + 3 rings of 8 nodes = 1 + 24 = 25 nodes
        assert_eq!(mesh.num_nodes(), 25);
    }

    #[test]
    fn test_annular_mesh() {
        let mesh = annular_mesh_triangles(0.0, 0.0, 0.5, 2.0, 4, 16);

        // (n_radial + 1) rings of n_angular nodes = 5 * 16 = 80 nodes
        assert_eq!(mesh.num_nodes(), 80);

        // n_radial * n_angular * 2 triangles = 4 * 16 * 2 = 128 elements
        assert_eq!(mesh.num_elements(), 128);

        // Should have boundaries on both inner and outer circles
        assert!(!mesh.boundaries.is_empty());
    }

    #[test]
    fn test_unit_square() {
        let mesh = unit_square_triangles(4);
        assert_eq!(mesh.num_nodes(), 25);
        assert_eq!(mesh.num_elements(), 32);
    }

    #[test]
    fn test_boundary_detection() {
        let mesh = rectangular_mesh_triangles(0.0, 1.0, 0.0, 1.0, 2, 2);

        // Should have 8 boundary edges (2 per side of square)
        assert_eq!(mesh.boundaries.len(), 8);
    }

    #[test]
    fn test_spherical_shell_mesh() {
        // Icosphere subdiv 0 = 12 nodes, 20 tris
        // 2 layers
        // Total nodes = 12 * 3 = 36 nodes
        // Total elems = 2 layers * 20 tris * 3 tets = 120 tets
        let mesh = spherical_shell_mesh_tetrahedra(0.0, 0.0, 0.0, 1.0, 2.0, 0, 2);

        assert_eq!(mesh.num_nodes(), 36);
        assert_eq!(mesh.num_elements(), 120);
        assert_eq!(mesh.dimension, 3);

        // Check boundaries
        // Outer + Inner = 20 + 20 = 40 boundary faces
        assert_eq!(mesh.boundaries.len(), 40);
    }
}
