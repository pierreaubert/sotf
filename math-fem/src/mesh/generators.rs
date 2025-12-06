//! Mesh generators for common domains
//!
//! Provides functions to create structured meshes for rectangles, boxes, and circles.

use super::types::{BoundaryType, ElementType, Mesh, Point};

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
///
/// Creates a mesh of the domain between two concentric circles.
/// Useful for scattering problems with circular obstacles.
///
/// # Arguments
///
/// * `center_x`, `center_y` - Center coordinates
/// * `inner_radius` - Inner circle radius (obstacle boundary)
/// * `outer_radius` - Outer circle radius (far-field boundary)
/// * `n_radial` - Number of layers in radial direction
/// * `n_angular` - Number of divisions in angular direction
///
/// # Boundary markers
///
/// * Tag 1: Inner boundary (obstacle surface)
/// * Tag 2: Outer boundary (far-field)
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
}
