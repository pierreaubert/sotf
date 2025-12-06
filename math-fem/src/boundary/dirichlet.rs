//! Dirichlet (essential) boundary conditions
//!
//! Implements u = g on the boundary by modifying the system matrix and RHS.

use crate::assembly::{HelmholtzMatrix, HelmholtzProblem};
use crate::mesh::Mesh;
use num_complex::Complex64;
use std::collections::HashSet;

/// Dirichlet boundary condition: u = g on boundary
pub struct DirichletBC {
    /// Boundary tag from mesh
    pub tag: usize,
    /// Value function g(x, y, z)
    value_fn: Box<dyn Fn(f64, f64, f64) -> Complex64>,
}

impl std::fmt::Debug for DirichletBC {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirichletBC")
            .field("tag", &self.tag)
            .finish()
    }
}

impl Clone for DirichletBC {
    fn clone(&self) -> Self {
        // Cannot clone the function, so create a dummy
        Self {
            tag: self.tag,
            value_fn: Box::new(|_, _, _| Complex64::new(0.0, 0.0)),
        }
    }
}

impl DirichletBC {
    pub fn new<F>(tag: usize, value_fn: F) -> Self
    where
        F: Fn(f64, f64, f64) -> Complex64 + 'static,
    {
        Self {
            tag,
            value_fn: Box::new(value_fn),
        }
    }

    /// Evaluate the boundary value at a point
    pub fn value(&self, x: f64, y: f64, z: f64) -> Complex64 {
        (self.value_fn)(x, y, z)
    }

    /// Get boundary nodes from mesh with this tag (marker)
    pub fn boundary_nodes(&self, mesh: &Mesh) -> HashSet<usize> {
        let mut nodes = HashSet::new();
        for boundary in &mesh.boundaries {
            if boundary.marker == self.tag as i32 {
                for &node in &boundary.nodes {
                    nodes.insert(node);
                }
            }
        }
        nodes
    }
}

/// Apply Dirichlet conditions to the Helmholtz system using row elimination
///
/// For each Dirichlet node i with value g_i:
/// - Set A[i, :] = 0
/// - Set A[i, i] = 1
/// - Set b[i] = g_i
/// - Modify b[j] -= A[j, i] * g_i for all j (to preserve symmetry in RHS)
pub fn apply_dirichlet(problem: &mut HelmholtzProblem, mesh: &Mesh, dirichlet_bcs: &[DirichletBC]) {
    use std::collections::HashMap;

    // Collect all Dirichlet nodes and their values, avoiding duplicates
    // If a node appears on multiple boundaries (e.g., corners), use the first value
    let mut dirichlet_map: HashMap<usize, Complex64> = HashMap::new();

    for bc in dirichlet_bcs {
        for &node in &bc.boundary_nodes(mesh) {
            // Only insert if not already present (first BC wins for shared nodes)
            dirichlet_map.entry(node).or_insert_with(|| {
                let point = &mesh.nodes[node];
                bc.value(point.x, point.y, point.z)
            });
        }
    }

    // Convert to vec for iteration
    let dirichlet_nodes: Vec<(usize, Complex64)> =
        dirichlet_map.iter().map(|(&n, &v)| (n, v)).collect();

    let dirichlet_set: HashSet<usize> = dirichlet_map.keys().copied().collect();

    // Create new matrix with modified entries
    let matrix = &problem.matrix;
    let n = matrix.dim;

    // First pass: accumulate column contributions to RHS
    // b[j] -= A[j, i] * g_i for Dirichlet nodes i
    let mut rhs_correction = vec![Complex64::new(0.0, 0.0); n];

    for k in 0..matrix.nnz() {
        let col = matrix.cols[k];
        if dirichlet_set.contains(&col) {
            let row = matrix.rows[k];
            if !dirichlet_set.contains(&row) {
                // Find the Dirichlet value for this column
                if let Some((_, g)) = dirichlet_nodes.iter().find(|(node, _)| *node == col) {
                    rhs_correction[row] += matrix.values[k] * g;
                }
            }
        }
    }

    // Apply RHS correction
    for i in 0..n {
        problem.rhs[i] -= rhs_correction[i];
    }

    // Set Dirichlet values in RHS
    for (node, value) in &dirichlet_nodes {
        problem.rhs[*node] = *value;
    }

    // Rebuild matrix with Dirichlet rows/columns zeroed
    let mut new_rows = Vec::new();
    let mut new_cols = Vec::new();
    let mut new_values = Vec::new();

    // Track which Dirichlet diagonals we've already added (to avoid duplicates)
    let mut dirichlet_diagonals_added: HashSet<usize> = HashSet::new();

    for k in 0..matrix.nnz() {
        let row = matrix.rows[k];
        let col = matrix.cols[k];

        if dirichlet_set.contains(&row) {
            // Zero out entire row except diagonal
            if row == col && !dirichlet_diagonals_added.contains(&row) {
                new_rows.push(row);
                new_cols.push(col);
                new_values.push(Complex64::new(1.0, 0.0));
                dirichlet_diagonals_added.insert(row);
            }
            // Skip all other entries in Dirichlet rows
        } else if dirichlet_set.contains(&col) {
            // Zero out column entries (already handled in RHS)
            continue;
        } else {
            // Keep original entry
            new_rows.push(row);
            new_cols.push(col);
            new_values.push(matrix.values[k]);
        }
    }

    // Ensure all Dirichlet diagonal entries exist (for nodes that had no diagonal in original)
    for (node, _) in &dirichlet_nodes {
        if !dirichlet_diagonals_added.contains(node) {
            new_rows.push(*node);
            new_cols.push(*node);
            new_values.push(Complex64::new(1.0, 0.0));
            dirichlet_diagonals_added.insert(*node);
        }
    }

    problem.matrix = HelmholtzMatrix {
        rows: new_rows,
        cols: new_cols,
        values: new_values,
        dim: n,
        wavenumber: matrix.wavenumber,
    };
}

/// Apply homogeneous Dirichlet conditions (u = 0)
pub fn apply_homogeneous_dirichlet(problem: &mut HelmholtzProblem, mesh: &Mesh, tags: &[usize]) {
    let bcs: Vec<DirichletBC> = tags
        .iter()
        .map(|&tag| DirichletBC::new(tag, |_, _, _| Complex64::new(0.0, 0.0)))
        .collect();

    apply_dirichlet(problem, mesh, &bcs);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembly::HelmholtzProblem;
    use crate::basis::PolynomialDegree;
    use crate::mesh::unit_square_triangles;

    #[test]
    fn test_dirichlet_bc_creation() {
        let bc = DirichletBC::new(1, |x, y, _z| Complex64::new(x + y, 0.0));
        assert_eq!(bc.tag, 1);
        assert_eq!(bc.value(1.0, 2.0, 0.0), Complex64::new(3.0, 0.0));
    }

    #[test]
    fn test_apply_homogeneous_dirichlet() {
        let mesh = unit_square_triangles(4);
        let k = Complex64::new(1.0, 0.0);

        let mut problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(1.0, 0.0)
        });

        // Apply homogeneous Dirichlet on tags 1 and 2 (left and right boundaries)
        apply_homogeneous_dirichlet(&mut problem, &mesh, &[1, 2]);

        // Check that Dirichlet nodes have RHS = 0
        let bc1 = DirichletBC::new(1, |_, _, _| Complex64::new(0.0, 0.0));
        let bc2 = DirichletBC::new(2, |_, _, _| Complex64::new(0.0, 0.0));

        for &node in &bc1.boundary_nodes(&mesh) {
            assert!(
                problem.rhs[node].norm() < 1e-10,
                "Node {} RHS should be 0",
                node
            );
        }
        for &node in &bc2.boundary_nodes(&mesh) {
            assert!(
                problem.rhs[node].norm() < 1e-10,
                "Node {} RHS should be 0",
                node
            );
        }
    }
}
