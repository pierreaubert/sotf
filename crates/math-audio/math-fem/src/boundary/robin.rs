//! Robin (mixed) boundary conditions
//!
//! Implements ∂u/∂n + αu = g on the boundary by modifying both matrix and RHS.
//! Robin BC adds:
//! - α * ∫_Γ φ_i * φ_j dΓ to the matrix (mass-like boundary term)
//! - ∫_Γ g * φ_i dΓ to the RHS

use crate::assembly::HelmholtzProblem;
use crate::mesh::Mesh;
use crate::quadrature::gauss_legendre_1d;
use num_complex::Complex64;

/// Robin boundary condition: ∂u/∂n + αu = g on boundary
pub struct RobinBC {
    /// Boundary tag from mesh
    pub tag: usize,
    /// Alpha coefficient function α(x, y, z)
    alpha_fn: Box<dyn Fn(f64, f64, f64) -> Complex64>,
    /// RHS function g(x, y, z)
    g_fn: Box<dyn Fn(f64, f64, f64) -> Complex64>,
}

impl std::fmt::Debug for RobinBC {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RobinBC").field("tag", &self.tag).finish()
    }
}

impl Clone for RobinBC {
    fn clone(&self) -> Self {
        Self {
            tag: self.tag,
            alpha_fn: Box::new(|_, _, _| Complex64::new(0.0, 0.0)),
            g_fn: Box::new(|_, _, _| Complex64::new(0.0, 0.0)),
        }
    }
}

impl RobinBC {
    pub fn new<F, G>(tag: usize, alpha_fn: F, g_fn: G) -> Self
    where
        F: Fn(f64, f64, f64) -> Complex64 + 'static,
        G: Fn(f64, f64, f64) -> Complex64 + 'static,
    {
        Self {
            tag,
            alpha_fn: Box::new(alpha_fn),
            g_fn: Box::new(g_fn),
        }
    }

    /// Create a first-order absorbing boundary condition (Sommerfeld)
    /// ∂u/∂n + iku = 0
    pub fn absorbing(tag: usize, wavenumber: Complex64) -> Self {
        let ik = Complex64::new(0.0, 1.0) * wavenumber;
        Self::new(tag, move |_, _, _| ik, |_, _, _| Complex64::new(0.0, 0.0))
    }

    /// Evaluate alpha at a point
    pub fn alpha(&self, x: f64, y: f64, z: f64) -> Complex64 {
        (self.alpha_fn)(x, y, z)
    }

    /// Evaluate g at a point
    pub fn g(&self, x: f64, y: f64, z: f64) -> Complex64 {
        (self.g_fn)(x, y, z)
    }
}

/// Apply Robin conditions to the Helmholtz system
///
/// Modifies both matrix (adds boundary mass-like term) and RHS
pub fn apply_robin(
    problem: &mut HelmholtzProblem,
    mesh: &Mesh,
    robin_bcs: &[RobinBC],
    quad_order: usize,
) {
    let quad_1d = gauss_legendre_1d(quad_order);

    for bc in robin_bcs {
        for boundary in mesh.boundaries.iter().filter(|b| b.marker == bc.tag as i32) {
            let nodes = &boundary.nodes;

            if nodes.len() == 2 {
                // P1 edge
                let p0 = &mesh.nodes[nodes[0]];
                let p1 = &mesh.nodes[nodes[1]];

                let dx = p1.x - p0.x;
                let dy = p1.y - p0.y;
                let edge_len = (dx * dx + dy * dy).sqrt();

                // Local contributions
                let mut m_local = [[Complex64::new(0.0, 0.0); 2]; 2];
                let mut f_local = [Complex64::new(0.0, 0.0); 2];

                for qp in &quad_1d {
                    let t = 0.5 * (qp.xi() + 1.0);
                    let w = 0.5 * qp.weight * edge_len;

                    let x = p0.x + t * dx;
                    let y = p0.y + t * dy;
                    let z = p0.z + t * (p1.z - p0.z);

                    let alpha = bc.alpha(x, y, z);
                    let g = bc.g(x, y, z);

                    let n0 = 1.0 - t;
                    let n1 = t;
                    let shapes = [n0, n1];

                    // Matrix: α * ∫ N_i * N_j dΓ
                    for i in 0..2 {
                        for j in 0..2 {
                            m_local[i][j] += alpha * Complex64::new(shapes[i] * shapes[j] * w, 0.0);
                        }
                        // RHS: ∫ g * N_i dΓ
                        f_local[i] += g * Complex64::new(shapes[i] * w, 0.0);
                    }
                }

                // Add to global matrix
                for i in 0..2 {
                    for j in 0..2 {
                        if m_local[i][j].norm() > 1e-15 {
                            problem.matrix.rows.push(nodes[i]);
                            problem.matrix.cols.push(nodes[j]);
                            problem.matrix.values.push(m_local[i][j]);
                        }
                    }
                    // Add to RHS
                    problem.rhs[nodes[i]] += f_local[i];
                }
            }
        }
    }
}

/// Create an absorbing (Sommerfeld) boundary condition
///
/// First-order ABC: ∂u/∂n + iku = 0
/// This is exact for outgoing plane waves in 1D.
pub fn create_absorbing_bc(tag: usize, wavenumber: Complex64) -> RobinBC {
    RobinBC::absorbing(tag, wavenumber)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembly::HelmholtzProblem;
    use crate::basis::PolynomialDegree;
    use crate::mesh::unit_square_triangles;

    #[test]
    fn test_robin_bc_creation() {
        let bc = RobinBC::new(
            1,
            |_, _, _| Complex64::new(1.0, 0.0),
            |x, y, _z| Complex64::new(x + y, 0.0),
        );
        assert_eq!(bc.tag, 1);
        assert_eq!(bc.alpha(0.0, 0.0, 0.0), Complex64::new(1.0, 0.0));
        assert_eq!(bc.g(1.0, 2.0, 0.0), Complex64::new(3.0, 0.0));
    }

    #[test]
    fn test_absorbing_bc() {
        let k = Complex64::new(1.0, 0.0);
        let bc = RobinBC::absorbing(1, k);

        // Alpha should be ik
        let alpha = bc.alpha(0.0, 0.0, 0.0);
        assert!((alpha.re).abs() < 1e-10);
        assert!((alpha.im - 1.0).abs() < 1e-10);

        // g should be 0
        let g = bc.g(0.0, 0.0, 0.0);
        assert!(g.norm() < 1e-10);
    }

    #[test]
    fn test_apply_robin() {
        let mesh = unit_square_triangles(4);
        let k = Complex64::new(1.0, 0.0);

        let mut problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(0.0, 0.0)
        });

        let original_nnz = problem.matrix.nnz();

        // Apply Robin BC with α = 1, g = 0 on boundary tag 3
        let bcs = vec![RobinBC::new(
            3,
            |_, _, _| Complex64::new(1.0, 0.0),
            |_, _, _| Complex64::new(0.0, 0.0),
        )];
        apply_robin(&mut problem, &mesh, &bcs, 2);

        // Matrix should have more entries (boundary mass term)
        assert!(problem.matrix.nnz() > original_nnz);
    }
}
