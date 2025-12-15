//! Neumann (natural) boundary conditions
//!
//! Implements ∂u/∂n = h on the boundary by adding surface integrals to the RHS.

use crate::assembly::HelmholtzProblem;
use crate::basis::PolynomialDegree;
use crate::mesh::{BoundaryFace, Mesh};
use crate::quadrature::gauss_legendre_1d;
use num_complex::Complex64;

/// Neumann boundary condition: ∂u/∂n = h on boundary
pub struct NeumannBC {
    /// Boundary tag from mesh
    pub tag: usize,
    /// Flux function h(x, y, z)
    flux_fn: Box<dyn Fn(f64, f64, f64) -> Complex64>,
}

impl std::fmt::Debug for NeumannBC {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NeumannBC").field("tag", &self.tag).finish()
    }
}

impl Clone for NeumannBC {
    fn clone(&self) -> Self {
        Self {
            tag: self.tag,
            flux_fn: Box::new(|_, _, _| Complex64::new(0.0, 0.0)),
        }
    }
}

impl NeumannBC {
    pub fn new<F>(tag: usize, flux_fn: F) -> Self
    where
        F: Fn(f64, f64, f64) -> Complex64 + 'static,
    {
        Self {
            tag,
            flux_fn: Box::new(flux_fn),
        }
    }

    /// Evaluate the flux at a point
    pub fn flux(&self, x: f64, y: f64, z: f64) -> Complex64 {
        (self.flux_fn)(x, y, z)
    }

    /// Get boundary edges/faces from mesh with this tag (marker)
    pub fn boundary_edges<'a>(&self, mesh: &'a Mesh) -> impl Iterator<Item = &'a BoundaryFace> {
        mesh.boundaries
            .iter()
            .filter(move |b| b.marker == self.tag as i32)
    }
}

/// Apply Neumann conditions to the Helmholtz system
///
/// Adds ∫_Γ h * φ_i dΓ to the RHS for boundary nodes
pub fn apply_neumann(
    problem: &mut HelmholtzProblem,
    mesh: &Mesh,
    neumann_bcs: &[NeumannBC],
    degree: PolynomialDegree,
) {
    let quad_order = degree.degree() + 1;
    let quad_1d = gauss_legendre_1d(quad_order);

    for bc in neumann_bcs {
        for boundary in bc.boundary_edges(mesh) {
            let nodes = &boundary.nodes;

            // For 2D: boundary is an edge with 2 nodes (P1) or 3 nodes (P2)
            // For 3D: boundary is a face (not handled here - simplified)
            if nodes.len() == 2 {
                // P1 edge: linear interpolation
                let p0 = &mesh.nodes[nodes[0]];
                let p1 = &mesh.nodes[nodes[1]];

                // Edge length
                let dx = p1.x - p0.x;
                let dy = p1.y - p0.y;
                let edge_len = (dx * dx + dy * dy).sqrt();

                // Integrate along edge
                let mut contrib = [Complex64::new(0.0, 0.0); 2];

                for qp in &quad_1d {
                    // Map from [-1, 1] to edge parameter [0, 1]
                    let t = 0.5 * (qp.xi() + 1.0);
                    let w = 0.5 * qp.weight * edge_len;

                    // Physical coordinates
                    let x = p0.x + t * dx;
                    let y = p0.y + t * dy;
                    let z = p0.z + t * (p1.z - p0.z);

                    // Flux value
                    let h = bc.flux(x, y, z);

                    // Shape functions on edge: N0 = 1-t, N1 = t
                    let n0 = 1.0 - t;
                    let n1 = t;

                    contrib[0] += h * Complex64::new(n0 * w, 0.0);
                    contrib[1] += h * Complex64::new(n1 * w, 0.0);
                }

                // Add to RHS
                problem.rhs[nodes[0]] += contrib[0];
                problem.rhs[nodes[1]] += contrib[1];
            }
            // Handle P2 edges (3 nodes) and 3D faces if needed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembly::HelmholtzProblem;
    use crate::mesh::unit_square_triangles;

    #[test]
    fn test_neumann_bc_creation() {
        let bc = NeumannBC::new(3, |x, _y, _z| Complex64::new(x, 0.0));
        assert_eq!(bc.tag, 3);
        assert_eq!(bc.flux(2.0, 0.0, 0.0), Complex64::new(2.0, 0.0));
    }

    #[test]
    fn test_apply_neumann() {
        let mesh = unit_square_triangles(4);
        let k = Complex64::new(1.0, 0.0);

        let mut problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(0.0, 0.0)
        });

        // Store original RHS sum
        let original_sum: Complex64 = problem.rhs.iter().sum();

        // Apply constant Neumann flux h = 1 on tag 3 (bottom boundary)
        let bcs = vec![NeumannBC::new(3, |_, _, _| Complex64::new(1.0, 0.0))];
        apply_neumann(&mut problem, &mesh, &bcs, PolynomialDegree::P1);

        // RHS should have changed
        let new_sum: Complex64 = problem.rhs.iter().sum();
        let diff = (new_sum - original_sum).norm();

        // The integral of 1 over a unit edge should be 1
        assert!(
            diff > 0.9 && diff < 1.1,
            "Neumann contribution should be ~1, got {}",
            diff
        );
    }
}
