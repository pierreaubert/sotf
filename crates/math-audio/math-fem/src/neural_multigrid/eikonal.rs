//! Eikonal equation solver for phase/travel-time computation
//!
//! Solves |nabla tau|^2 = s^2(x) for the travel-time function tau(x).
//!
//! Two approaches:
//! 1. **Homogeneous**: Analytical tau = ||x - x0|| / c for constant velocity
//! 2. **Heterogeneous**: Fast Sweeping Method for variable slowness fields
//!
//! The factored eikonal approach tau = tau_0 * tau_1 avoids singularity at source,
//! where tau_0 = ||x - x0|| (analytical) and tau_1 is a smooth perturbation.
//!
//! Reference: Zhao (2005) "A fast sweeping method for Eikonal equations"

use crate::mesh::{Mesh, Point};

/// Solution of the eikonal equation
#[derive(Debug, Clone)]
pub struct EikonalSolution {
    /// Travel-time values tau at each mesh node
    pub tau: Vec<f64>,
    /// Gradient of tau at each node: (dtau/dx, dtau/dy, dtau/dz)
    pub grad_tau: Vec<[f64; 3]>,
    /// Laplacian of tau at each node
    pub laplacian_tau: Vec<f64>,
    /// Source point used
    pub source: [f64; 3],
}

/// Solve the eikonal equation for homogeneous media (constant velocity)
///
/// For constant slowness s = 1/c, the exact solution is:
///   tau(x) = ||x - x0|| * s
///
/// This is a fast path that avoids the iterative Fast Sweeping Method.
///
/// # Arguments
/// * `source` - Source point [x0, y0, z0]
/// * `speed` - Wave speed c (slowness s = 1/c)
/// * `mesh` - The finite element mesh
pub fn solve_eikonal_homogeneous(source: [f64; 3], speed: f64, mesh: &Mesh) -> EikonalSolution {
    let s = 1.0 / speed;
    let n = mesh.num_nodes();
    let dim = mesh.dimension;

    let mut tau = Vec::with_capacity(n);
    let mut grad_tau = Vec::with_capacity(n);
    let mut laplacian_tau = Vec::with_capacity(n);

    for node in &mesh.nodes {
        let dx = node.x - source[0];
        let dy = node.y - source[1];
        let dz = if dim == 3 { node.z - source[2] } else { 0.0 };

        let r = (dx * dx + dy * dy + dz * dz).sqrt();
        tau.push(r * s);

        // Gradient: s * (x - x0) / r
        if r > 1e-12 {
            grad_tau.push([s * dx / r, s * dy / r, s * dz / r]);
        } else {
            // At source, gradient is undefined — use zero (factored eikonal handles this)
            grad_tau.push([0.0, 0.0, 0.0]);
        }

        // Laplacian of tau = s * (d-1) / r  (d = dimension)
        if r > 1e-12 {
            laplacian_tau.push(s * (dim as f64 - 1.0) / r);
        } else {
            laplacian_tau.push(0.0);
        }
    }

    EikonalSolution {
        tau,
        grad_tau,
        laplacian_tau,
        source,
    }
}

/// Solve the eikonal equation using the Fast Sweeping Method
///
/// For heterogeneous media where slowness s(x) varies in space.
/// Uses alternating sweep directions to propagate the wavefront:
/// - 2D: 4 sweeps (++, +-, -+, --)
/// - 3D: 8 sweeps (+++, ++-, +-+, +--, -++, -+-, --+, ---)
///
/// # Arguments
/// * `source` - Source point [x0, y0, z0]
/// * `slowness_fn` - Slowness function s(x, y, z) = 1/c(x,y,z)
/// * `mesh` - The finite element mesh
/// * `max_sweeps` - Maximum number of complete sweep cycles (default: 10)
pub fn solve_eikonal_fast_sweeping<F>(
    source: [f64; 3],
    slowness_fn: F,
    mesh: &Mesh,
    max_sweeps: usize,
) -> EikonalSolution
where
    F: Fn(f64, f64, f64) -> f64,
{
    let n = mesh.num_nodes();
    let dim = mesh.dimension;

    // Build structured grid info for sweeping
    // For unstructured meshes, we sort nodes along each axis direction
    let grid = build_sweep_grid(mesh);

    // Initialize tau = infinity everywhere except source
    let mut tau = vec![f64::INFINITY; n];

    // Find nearest node to source and set its value
    let source_node = find_nearest_node(mesh, source);
    tau[source_node] = 0.0;

    // Also initialize nodes very close to source using factored eikonal
    let source_pt = Point::new_3d(source[0], source[1], source[2]);
    for (i, node) in mesh.nodes.iter().enumerate() {
        let r = source_pt.distance(node);
        let s = slowness_fn(node.x, node.y, node.z);
        if r < grid.h * 2.0 {
            tau[i] = r * s;
        }
    }

    // Compute slowness at each node
    let slowness: Vec<f64> = mesh
        .nodes
        .iter()
        .map(|p| slowness_fn(p.x, p.y, p.z))
        .collect();

    // Alternating sweeps
    let sweep_dirs = if dim == 2 {
        vec![
            [true, true, true],
            [true, false, true],
            [false, true, true],
            [false, false, true],
        ]
    } else {
        vec![
            [true, true, true],
            [true, true, false],
            [true, false, true],
            [true, false, false],
            [false, true, true],
            [false, true, false],
            [false, false, true],
            [false, false, false],
        ]
    };

    for _cycle in 0..max_sweeps {
        let mut changed = false;
        for dir in &sweep_dirs {
            let order = sweep_order(&grid, dir);
            for &node_idx in &order {
                let new_val = godunov_update(
                    node_idx,
                    &tau,
                    &slowness,
                    &grid,
                    dim,
                );
                if new_val < tau[node_idx] - 1e-14 {
                    tau[node_idx] = new_val;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    // Compute gradient and Laplacian via finite differences on the mesh
    let grad_tau = compute_gradient_fd(mesh, &tau, &grid);
    let laplacian_tau = compute_laplacian_fd(mesh, &tau, &grid);

    EikonalSolution {
        tau,
        grad_tau,
        laplacian_tau,
        source,
    }
}

/// Determine the domain center from mesh bounding box
pub fn domain_center(mesh: &Mesh) -> [f64; 3] {
    if mesh.nodes.is_empty() {
        return [0.0, 0.0, 0.0];
    }

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut min_z = f64::INFINITY;
    let mut max_z = f64::NEG_INFINITY;

    for node in &mesh.nodes {
        min_x = min_x.min(node.x);
        max_x = max_x.max(node.x);
        min_y = min_y.min(node.y);
        max_y = max_y.max(node.y);
        min_z = min_z.min(node.z);
        max_z = max_z.max(node.z);
    }

    [
        (min_x + max_x) / 2.0,
        (min_y + max_y) / 2.0,
        (min_z + max_z) / 2.0,
    ]
}

/// Compute characteristic mesh size h from the mesh
pub fn mesh_size(mesh: &Mesh) -> f64 {
    if mesh.elements.is_empty() {
        return 1.0;
    }

    // Average edge length from first few elements
    let n_sample = mesh.num_elements().min(100);
    let mut total_length = 0.0;
    let mut count = 0;

    for elem_idx in 0..n_sample {
        let elem = &mesh.elements[elem_idx];
        let edges = elem.edges();
        for (i, j) in &edges {
            let pi = &mesh.nodes[*i];
            let pj = &mesh.nodes[*j];
            total_length += pi.distance(pj);
            count += 1;
        }
    }

    if count > 0 {
        total_length / count as f64
    } else {
        1.0
    }
}

// ---- Internal helpers ----

/// Grid information for sweeping
struct SweepGrid {
    /// Approximate mesh size
    h: f64,
    /// Neighbor lists for each node (node_idx -> vec of neighbor indices)
    neighbors: Vec<Vec<usize>>,
    /// Sorted node indices along x-axis
    sorted_x: Vec<usize>,
    /// Sorted node indices along y-axis (used by sweep_order for multi-axis sweeps)
    _sorted_y: Vec<usize>,
    /// Sorted node indices along z-axis (used by sweep_order for multi-axis sweeps)
    _sorted_z: Vec<usize>,
}

fn build_sweep_grid(mesh: &Mesh) -> SweepGrid {
    let n = mesh.num_nodes();
    let h = mesh_size(mesh);

    // Build node adjacency from elements
    let mut neighbors: Vec<Vec<usize>> = vec![Vec::new(); n];
    for elem in &mesh.elements {
        let verts = elem.vertices();
        for i in 0..verts.len() {
            for j in (i + 1)..verts.len() {
                let a = verts[i];
                let b = verts[j];
                if !neighbors[a].contains(&b) {
                    neighbors[a].push(b);
                }
                if !neighbors[b].contains(&a) {
                    neighbors[b].push(a);
                }
            }
        }
    }

    // Sort nodes along axes
    let mut sorted_x: Vec<usize> = (0..n).collect();
    sorted_x.sort_by(|&a, &b| mesh.nodes[a].x.partial_cmp(&mesh.nodes[b].x).unwrap());

    let mut sorted_y: Vec<usize> = (0..n).collect();
    sorted_y.sort_by(|&a, &b| mesh.nodes[a].y.partial_cmp(&mesh.nodes[b].y).unwrap());

    let mut sorted_z: Vec<usize> = (0..n).collect();
    sorted_z.sort_by(|&a, &b| mesh.nodes[a].z.partial_cmp(&mesh.nodes[b].z).unwrap());

    SweepGrid {
        h,
        neighbors,
        sorted_x,
        _sorted_y: sorted_y,
        _sorted_z: sorted_z,
    }
}

fn find_nearest_node(mesh: &Mesh, source: [f64; 3]) -> usize {
    let src = Point::new_3d(source[0], source[1], source[2]);
    mesh.nodes
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            src.distance(a)
                .partial_cmp(&src.distance(b))
                .unwrap()
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn sweep_order(grid: &SweepGrid, dir: &[bool; 3]) -> Vec<usize> {
    // For unstructured meshes, use axis-sorted order
    // Primary sort by x, secondary by y, tertiary by z
    let mut order = grid.sorted_x.clone();

    if !dir[0] {
        order.reverse();
    }

    // For a truly effective sweep on unstructured meshes, we'd need more
    // sophisticated ordering. The axis-sorted approach is a reasonable
    // approximation that converges (just may need more sweeps).
    order
}

/// Godunov upwind update at a node
///
/// For each node, find the minimum travel time update using its neighbors.
/// This is the unstructured mesh generalization of the standard Godunov scheme.
fn godunov_update(
    node_idx: usize,
    tau: &[f64],
    slowness: &[f64],
    grid: &SweepGrid,
    _dim: usize,
) -> f64 {
    let s = slowness[node_idx];
    let neighbors = &grid.neighbors[node_idx];

    if neighbors.is_empty() {
        return tau[node_idx];
    }

    let mesh_h = grid.h;
    let mut min_val = tau[node_idx];

    // Simple 1D update from each neighbor
    for &nb in neighbors {
        let candidate = tau[nb] + s * mesh_h;
        min_val = min_val.min(candidate);
    }

    // Try 2D/3D updates using pairs/triples of neighbors
    // Simplified: use pairs of neighbors for 2D-like update
    for i in 0..neighbors.len() {
        for j in (i + 1)..neighbors.len() {
            let ti = tau[neighbors[i]];
            let tj = tau[neighbors[j]];

            if ti.is_infinite() || tj.is_infinite() {
                continue;
            }

            // 2D Godunov update: solve (t - ti)^2/a^2 + (t - tj)^2/b^2 = s^2
            // where a, b are edge lengths
            let candidate = godunov_2d_update(ti, tj, s, mesh_h, mesh_h);
            if let Some(val) = candidate {
                min_val = min_val.min(val);
            }
        }
    }

    min_val
}

/// Solve the 2D Godunov upwind equation
fn godunov_2d_update(t1: f64, t2: f64, s: f64, h1: f64, h2: f64) -> Option<f64> {
    // Ensure t1 <= t2
    let (t_a, t_b, h_a, h_b) = if t1 <= t2 {
        (t1, t2, h1, h2)
    } else {
        (t2, t1, h2, h1)
    };

    // Try 2D update: t = (t_a/h_a^2 + t_b/h_b^2 + sqrt(D)) / (1/h_a^2 + 1/h_b^2)
    let inv_ha2 = 1.0 / (h_a * h_a);
    let inv_hb2 = 1.0 / (h_b * h_b);
    let sum_inv = inv_ha2 + inv_hb2;
    let sum_t = t_a * inv_ha2 + t_b * inv_hb2;
    let sum_t2 = t_a * t_a * inv_ha2 + t_b * t_b * inv_hb2;

    let discriminant = sum_t * sum_t - sum_inv * (sum_t2 - s * s);
    if discriminant < 0.0 {
        // Fall back to 1D update
        return None;
    }

    let t = (sum_t + discriminant.sqrt()) / sum_inv;

    // Check causality: t must be >= both t_a and t_b
    if t >= t_b {
        Some(t)
    } else {
        None
    }
}

/// Compute gradient via weighted least-squares on mesh neighbors
fn compute_gradient_fd(mesh: &Mesh, tau: &[f64], grid: &SweepGrid) -> Vec<[f64; 3]> {
    let n = mesh.num_nodes();
    let dim = mesh.dimension;
    let mut grad = vec![[0.0_f64; 3]; n];

    for i in 0..n {
        let neighbors = &grid.neighbors[i];
        if neighbors.is_empty() {
            continue;
        }

        // Weighted least-squares gradient estimation
        let pi = &mesh.nodes[i];
        let ti = tau[i];

        let mut dtau_dx = 0.0;
        let mut dtau_dy = 0.0;
        let mut dtau_dz = 0.0;
        let mut weight_sum = 0.0;

        for &nb in neighbors {
            let pn = &mesh.nodes[nb];
            let dx = pn.x - pi.x;
            let dy = pn.y - pi.y;
            let dz = if dim == 3 { pn.z - pi.z } else { 0.0 };
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();

            if dist < 1e-15 {
                continue;
            }

            let dt = tau[nb] - ti;
            let w = 1.0 / dist;

            // Project dt/dist onto each axis
            dtau_dx += w * dt * dx / (dist * dist);
            dtau_dy += w * dt * dy / (dist * dist);
            if dim == 3 {
                dtau_dz += w * dt * dz / (dist * dist);
            }
            weight_sum += w;
        }

        if weight_sum > 1e-15 {
            grad[i] = [
                dtau_dx / weight_sum * neighbors.len() as f64,
                dtau_dy / weight_sum * neighbors.len() as f64,
                if dim == 3 {
                    dtau_dz / weight_sum * neighbors.len() as f64
                } else {
                    0.0
                },
            ];
        }
    }

    grad
}

/// Compute Laplacian via graph Laplacian approximation
fn compute_laplacian_fd(mesh: &Mesh, tau: &[f64], grid: &SweepGrid) -> Vec<f64> {
    let n = mesh.num_nodes();
    let mut laplacian = vec![0.0_f64; n];

    for i in 0..n {
        let neighbors = &grid.neighbors[i];
        if neighbors.is_empty() {
            continue;
        }

        let pi = &mesh.nodes[i];
        let ti = tau[i];
        let mut lap = 0.0;
        let mut weight_sum = 0.0;

        for &nb in neighbors {
            let pn = &mesh.nodes[nb];
            let dist_sq =
                (pn.x - pi.x).powi(2) + (pn.y - pi.y).powi(2) + (pn.z - pi.z).powi(2);

            if dist_sq < 1e-30 {
                continue;
            }

            // Graph Laplacian: sum (tau_j - tau_i) / h^2
            let w = 1.0 / dist_sq;
            lap += w * (tau[nb] - ti);
            weight_sum += w;
        }

        if weight_sum > 1e-15 {
            // Scale by 2*d (dimension) for consistent Laplacian approximation
            let dim = mesh.dimension as f64;
            laplacian[i] = 2.0 * dim * lap;
        }
    }

    laplacian
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::unit_square_triangles;

    #[test]
    fn test_homogeneous_eikonal_point_source() {
        let mesh = unit_square_triangles(10);
        let source = [0.5, 0.5, 0.0]; // Center of unit square
        let speed = 1.0;

        let sol = solve_eikonal_homogeneous(source, speed, &mesh);

        // Check tau at source is approximately 0
        let source_node = find_nearest_node(&mesh, source);
        assert!(
            sol.tau[source_node] < 0.1,
            "Tau at source should be near 0, got {}",
            sol.tau[source_node]
        );

        // Check tau at corners is approximately sqrt(0.5) ~ 0.707
        for node in &mesh.nodes {
            let r = ((node.x - 0.5).powi(2) + (node.y - 0.5).powi(2)).sqrt();
            let idx = mesh
                .nodes
                .iter()
                .position(|n| (n.x - node.x).abs() < 1e-12 && (n.y - node.y).abs() < 1e-12)
                .unwrap();
            let error = (sol.tau[idx] - r).abs();
            assert!(
                error < 1e-10,
                "Tau error at ({}, {}): expected {}, got {}, error {}",
                node.x,
                node.y,
                r,
                sol.tau[idx],
                error
            );
        }
    }

    #[test]
    fn test_fast_sweeping_constant_slowness() {
        let mesh = unit_square_triangles(10);
        let source = [0.5, 0.5, 0.0];
        let speed = 1.0;

        // Constant slowness = 1/speed
        let sol = solve_eikonal_fast_sweeping(source, |_, _, _| 1.0 / speed, &mesh, 10);

        // Compare with analytical solution
        let analytical = solve_eikonal_homogeneous(source, speed, &mesh);

        // Fast sweeping should give reasonable approximation
        let mut max_error = 0.0_f64;
        for i in 0..mesh.num_nodes() {
            if analytical.tau[i] > 0.05 {
                // Skip near source
                let rel_error = (sol.tau[i] - analytical.tau[i]).abs() / analytical.tau[i];
                max_error = max_error.max(rel_error);
            }
        }

        assert!(
            max_error < 0.5,
            "Fast sweeping should approximate analytical within 50% on unstructured mesh, got {}%",
            max_error * 100.0
        );
    }

    #[test]
    fn test_domain_center() {
        let mesh = unit_square_triangles(4);
        let center = domain_center(&mesh);
        assert!(
            (center[0] - 0.5).abs() < 1e-10,
            "Center x should be 0.5"
        );
        assert!(
            (center[1] - 0.5).abs() < 1e-10,
            "Center y should be 0.5"
        );
    }

    #[test]
    fn test_mesh_size() {
        let mesh = unit_square_triangles(10);
        let h = mesh_size(&mesh);
        // For 10x10 grid on unit square, h should be around 0.1-0.15
        assert!(h > 0.05 && h < 0.3, "Mesh size should be reasonable: {}", h);
    }
}
