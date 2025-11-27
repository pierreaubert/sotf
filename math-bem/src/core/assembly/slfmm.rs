//! Single-Level Fast Multipole Method (SLFMM) assembly
//!
//! Direct port of NC_BuildSystemFMBEM (SLFMM mode) from NC_EquationSystem.cpp.
//!
//! The SLFMM decomposes the BEM interaction matrix as:
//! ```text
//! [A] = [N] + [S][D][T]
//! ```
//! where:
//! - [N] is the near-field matrix (direct interactions between nearby elements)
//! - [T] is the T-matrix (element-to-cluster multipole expansion)
//! - [D] is the D-matrix (cluster-to-cluster translation)
//! - [S] is the S-matrix (cluster-to-element local expansion)

use ndarray::{Array1, Array2};
use num_complex::Complex64;

use crate::core::greens::legendre::legendre_polynomials;
use crate::core::greens::spherical::spherical_hankel_first_kind;
use crate::core::integration::{regular_integration, singular_integration, unit_sphere_quadrature};
use crate::core::types::{BoundaryCondition, Cluster, Element, PhysicsParams};

/// Result of SLFMM assembly
pub struct SlfmmSystem {
    /// Near-field coefficient matrix (sparse, stored as dense blocks)
    pub near_matrix: Vec<NearFieldBlock>,
    /// T-matrix for each cluster (element DOFs → multipole expansion)
    pub t_matrices: Vec<Array2<Complex64>>,
    /// T-vector for RHS contribution
    pub t_vector: Array1<Complex64>,
    /// D-matrix entries for far cluster pairs
    pub d_matrices: Vec<DMatrixEntry>,
    /// S-matrix for each cluster (multipole expansion → field DOFs)
    pub s_matrices: Vec<Array2<Complex64>>,
    /// RHS vector
    pub rhs: Array1<Complex64>,
    /// Number of DOFs
    pub num_dofs: usize,
    /// Number of integration points on unit sphere
    pub num_sphere_points: usize,
    /// Number of expansion terms
    pub num_expansion_terms: usize,
    /// Number of clusters
    pub num_clusters: usize,
    /// Cluster DOF mappings: for each cluster, the global DOF indices
    pub cluster_dof_indices: Vec<Vec<usize>>,
}

/// Near-field block between two clusters
#[derive(Debug, Clone)]
pub struct NearFieldBlock {
    /// Source cluster index
    pub source_cluster: usize,
    /// Field cluster index
    pub field_cluster: usize,
    /// Dense coefficient matrix for this block
    pub coefficients: Array2<Complex64>,
}

/// D-matrix entry for a far cluster pair
#[derive(Debug, Clone)]
pub struct DMatrixEntry {
    /// Source cluster index
    pub source_cluster: usize,
    /// Field cluster index
    pub field_cluster: usize,
    /// Translation coefficients
    pub coefficients: Array2<Complex64>,
}

impl SlfmmSystem {
    /// Create a new empty SLFMM system
    pub fn new(
        num_dofs: usize,
        num_clusters: usize,
        num_sphere_points: usize,
        num_expansion_terms: usize,
    ) -> Self {
        Self {
            near_matrix: Vec::new(),
            t_matrices: Vec::with_capacity(num_clusters),
            t_vector: Array1::zeros(num_sphere_points * num_clusters),
            d_matrices: Vec::new(),
            s_matrices: Vec::with_capacity(num_clusters),
            rhs: Array1::zeros(num_dofs),
            num_dofs,
            num_sphere_points,
            num_expansion_terms,
            num_clusters,
            cluster_dof_indices: Vec::with_capacity(num_clusters),
        }
    }

    /// Apply the SLFMM operator: y = ([N] + [S][D][T]) * x
    ///
    /// This is used in iterative solvers (CGS, BiCGSTAB).
    ///
    /// The decomposition is:
    /// - Near-field: Direct element-to-element interactions for nearby clusters
    /// - Far-field: Multipole expansions with S*D*T factorization
    ///
    /// # Arguments
    /// * `x` - Input vector (length = num_dofs)
    ///
    /// # Returns
    /// * `y` - Output vector (length = num_dofs), y = A*x
    pub fn matvec(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        let mut y = Array1::zeros(self.num_dofs);

        // === Near-field contribution: y += [N] * x ===
        // Note: block.coefficients is (n_source, n_field) where:
        // - source = collocation points (rows, where we evaluate)
        // - field = integration elements (columns, source of influence)
        // For y = A*x: gather x from field DOFs, scatter y to source DOFs
        for block in &self.near_matrix {
            let src_dofs = &self.cluster_dof_indices[block.source_cluster];
            let fld_dofs = &self.cluster_dof_indices[block.field_cluster];

            // Gather x values from field cluster (columns of the matrix)
            let x_local: Array1<Complex64> =
                Array1::from_iter(fld_dofs.iter().map(|&i| x[i]));

            // Apply block matrix: y_local[i] = sum_j A[i,j] * x[j]
            let y_local = block.coefficients.dot(&x_local);

            // Scatter to source DOFs (rows of the matrix)
            for (local_i, &global_i) in src_dofs.iter().enumerate() {
                y[global_i] += y_local[local_i];
            }

            // Handle symmetric storage: if src != fld, also apply the (fld, src) block
            // which is the transpose of this block
            if block.source_cluster != block.field_cluster {
                // Gather x from source cluster DOFs
                let x_src: Array1<Complex64> =
                    Array1::from_iter(src_dofs.iter().map(|&i| x[i]));
                // Apply transpose: the (fld, src) block
                let y_fld = block.coefficients.t().dot(&x_src);
                // Scatter to field cluster DOFs
                for (local_j, &global_j) in fld_dofs.iter().enumerate() {
                    y[global_j] += y_fld[local_j];
                }
            }
        }

        // === Far-field contribution: y += [S][D][T] * x ===

        // Step 1: Compute multipole expansions for each cluster: t[c] = T[c] * x[c]
        let mut multipoles: Vec<Array1<Complex64>> = Vec::with_capacity(self.num_clusters);
        for (cluster_idx, t_mat) in self.t_matrices.iter().enumerate() {
            let cluster_dofs = &self.cluster_dof_indices[cluster_idx];
            if cluster_dofs.is_empty() || t_mat.is_empty() {
                multipoles.push(Array1::zeros(self.num_sphere_points));
                continue;
            }
            let x_local: Array1<Complex64> =
                Array1::from_iter(cluster_dofs.iter().map(|&i| x[i]));
            multipoles.push(t_mat.dot(&x_local));
        }

        // Step 2: Translate multipoles between far clusters: locals[fld] += D[src,fld] * multipoles[src]
        let mut locals: Vec<Array1<Complex64>> =
            (0..self.num_clusters)
                .map(|_| Array1::zeros(self.num_sphere_points))
                .collect();

        for d_entry in &self.d_matrices {
            let src_mult = &multipoles[d_entry.source_cluster];
            let translated = d_entry.coefficients.dot(src_mult);
            for i in 0..self.num_sphere_points {
                locals[d_entry.field_cluster][i] += translated[i];
            }
        }

        // Step 3: Evaluate locals at field points: y[c] += S[c] * locals[c]
        for (cluster_idx, s_mat) in self.s_matrices.iter().enumerate() {
            let cluster_dofs = &self.cluster_dof_indices[cluster_idx];
            if cluster_dofs.is_empty() || s_mat.is_empty() {
                continue;
            }
            let y_local = s_mat.dot(&locals[cluster_idx]);
            for (local_j, &global_j) in cluster_dofs.iter().enumerate() {
                y[global_j] += y_local[local_j];
            }
        }

        y
    }

    /// Apply the SLFMM operator transpose: y = A^T * x
    ///
    /// Used by some iterative solvers (e.g., BiCGSTAB).
    pub fn matvec_transpose(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        let mut y = Array1::zeros(self.num_dofs);

        // Near-field contribution (transposed)
        // For A^T: if A maps (field -> source), then A^T maps (source -> field)
        for block in &self.near_matrix {
            let src_dofs = &self.cluster_dof_indices[block.source_cluster];
            let fld_dofs = &self.cluster_dof_indices[block.field_cluster];

            // For transpose: gather from source DOFs, scatter to field DOFs
            let x_local: Array1<Complex64> =
                Array1::from_iter(src_dofs.iter().map(|&i| x[i]));
            let y_local = block.coefficients.t().dot(&x_local);
            for (local_j, &global_j) in fld_dofs.iter().enumerate() {
                y[global_j] += y_local[local_j];
            }

            // Symmetric storage: also apply the (src, fld) -> (fld, src) transpose
            if block.source_cluster != block.field_cluster {
                let x_fld: Array1<Complex64> =
                    Array1::from_iter(fld_dofs.iter().map(|&i| x[i]));
                let y_src = block.coefficients.dot(&x_fld);
                for (local_i, &global_i) in src_dofs.iter().enumerate() {
                    y[global_i] += y_src[local_i];
                }
            }
        }

        // Far-field contribution (transposed): y += T^T * D^T * S^T * x
        // Step 1: S^T * x
        let mut locals: Vec<Array1<Complex64>> =
            (0..self.num_clusters)
                .map(|_| Array1::zeros(self.num_sphere_points))
                .collect();

        for (cluster_idx, s_mat) in self.s_matrices.iter().enumerate() {
            let cluster_dofs = &self.cluster_dof_indices[cluster_idx];
            if cluster_dofs.is_empty() || s_mat.is_empty() {
                continue;
            }
            let x_local: Array1<Complex64> =
                Array1::from_iter(cluster_dofs.iter().map(|&i| x[i]));
            locals[cluster_idx] = s_mat.t().dot(&x_local);
        }

        // Step 2: D^T translation (reversed direction)
        let mut multipoles: Vec<Array1<Complex64>> =
            (0..self.num_clusters)
                .map(|_| Array1::zeros(self.num_sphere_points))
                .collect();

        for d_entry in &self.d_matrices {
            // Transpose: fld -> src direction
            let fld_local = &locals[d_entry.field_cluster];
            let translated = d_entry.coefficients.t().dot(fld_local);
            for i in 0..self.num_sphere_points {
                multipoles[d_entry.source_cluster][i] += translated[i];
            }
        }

        // Step 3: T^T * multipoles
        for (cluster_idx, t_mat) in self.t_matrices.iter().enumerate() {
            let cluster_dofs = &self.cluster_dof_indices[cluster_idx];
            if cluster_dofs.is_empty() || t_mat.is_empty() {
                continue;
            }
            let y_local = t_mat.t().dot(&multipoles[cluster_idx]);
            for (local_i, &global_i) in cluster_dofs.iter().enumerate() {
                y[global_i] += y_local[local_i];
            }
        }

        y
    }
}

/// Build the SLFMM system matrices
///
/// # Arguments
/// * `elements` - Vector of mesh elements
/// * `nodes` - Node coordinates (num_nodes × 3)
/// * `clusters` - Vector of clusters
/// * `physics` - Physics parameters
/// * `n_theta` - Number of integration points in theta direction
/// * `n_phi` - Number of integration points in phi direction
/// * `n_terms` - Number of expansion terms
pub fn build_slfmm_system(
    elements: &[Element],
    nodes: &Array2<f64>,
    clusters: &[Cluster],
    physics: &PhysicsParams,
    n_theta: usize,
    n_phi: usize,
    n_terms: usize,
) -> SlfmmSystem {
    let num_dofs = count_dofs(elements);
    let num_clusters = clusters.len();
    let num_sphere_points = n_theta * n_phi;

    let mut system = SlfmmSystem::new(num_dofs, num_clusters, num_sphere_points, n_terms);

    // Build cluster DOF mappings: for each cluster, collect the global DOF indices
    // of elements that belong to this cluster
    build_cluster_dof_mappings(&mut system, elements, clusters);

    // Compute unit sphere quadrature points
    let (sphere_coords, sphere_weights) = unit_sphere_quadrature(n_theta, n_phi);

    // Build near-field matrix
    build_near_field(&mut system, elements, nodes, clusters, physics);

    // Build T-matrices (element-to-cluster expansion)
    build_t_matrices(
        &mut system,
        elements,
        clusters,
        physics,
        &sphere_coords,
        &sphere_weights,
    );

    // Build D-matrices (cluster-to-cluster translation)
    build_d_matrices(&mut system, clusters, physics, &sphere_coords, n_terms);

    // Build S-matrices (cluster-to-element evaluation)
    build_s_matrices(
        &mut system,
        elements,
        clusters,
        physics,
        &sphere_coords,
        &sphere_weights,
    );

    system
}

/// Build the mapping from clusters to global DOF indices
fn build_cluster_dof_mappings(
    system: &mut SlfmmSystem,
    elements: &[Element],
    clusters: &[Cluster],
) {
    for cluster in clusters {
        let mut dof_indices = Vec::new();
        for &elem_idx in &cluster.element_indices {
            let elem = &elements[elem_idx];
            if elem.property.is_evaluation() {
                continue;
            }
            // Collect all DOF addresses for this element
            dof_indices.extend(elem.dof_addresses.iter().copied());
        }
        system.cluster_dof_indices.push(dof_indices);
    }
}

/// Count total number of DOFs
fn count_dofs(elements: &[Element]) -> usize {
    elements
        .iter()
        .filter(|e| !e.property.is_evaluation())
        .map(|e| e.dof_addresses.len())
        .sum()
}

/// Build near-field matrix blocks
fn build_near_field(
    system: &mut SlfmmSystem,
    elements: &[Element],
    nodes: &Array2<f64>,
    clusters: &[Cluster],
    physics: &PhysicsParams,
) {
    let gamma = Complex64::new(physics.gamma(), 0.0);
    let tau = Complex64::new(physics.tau, 0.0);
    let beta = physics.burton_miller_beta();

    // For each cluster pair in near-field
    for (i, cluster_i) in clusters.iter().enumerate() {
        // Self-interaction
        let mut block = compute_near_block(
            elements,
            nodes,
            &cluster_i.element_indices,
            &cluster_i.element_indices,
            physics,
            gamma,
            tau,
            beta,
            true, // is_self
        );

        // Add free terms to diagonal (jump conditions for Burton-Miller formulation)
        // For velocity BC: c = +1/2 for exterior problem, so diagonal += gamma * 0.5
        // This matches TBEM's add_free_terms() at tbem.rs:254-256
        for local_idx in 0..cluster_i.element_indices.len() {
            let elem_idx = cluster_i.element_indices[local_idx];
            let elem = &elements[elem_idx];
            if elem.property.is_evaluation() {
                continue;
            }
            // Check BC type to determine which free term to add
            match &elem.boundary_condition {
                BoundaryCondition::Velocity(_) | BoundaryCondition::VelocityWithAdmittance { .. } => {
                    // Velocity BC: diagonal term from CBIE jump
                    block[[local_idx, local_idx]] += gamma * 0.5;
                }
                BoundaryCondition::Pressure(_) => {
                    // Pressure BC: diagonal term from HBIE jump
                    block[[local_idx, local_idx]] += beta * tau * 0.5;
                }
                _ => {}
            }
        }

        system.near_matrix.push(NearFieldBlock {
            source_cluster: i,
            field_cluster: i,
            coefficients: block,
        });

        // Interaction with near clusters
        for &j in &cluster_i.near_clusters {
            if j > i {
                // Only compute upper triangle, lower is symmetric
                let cluster_j = &clusters[j];
                let block = compute_near_block(
                    elements,
                    nodes,
                    &cluster_i.element_indices,
                    &cluster_j.element_indices,
                    physics,
                    gamma,
                    tau,
                    beta,
                    false,
                );
                system.near_matrix.push(NearFieldBlock {
                    source_cluster: i,
                    field_cluster: j,
                    coefficients: block,
                });
            }
        }
    }
}

/// Compute a near-field block between two sets of elements
fn compute_near_block(
    elements: &[Element],
    nodes: &Array2<f64>,
    source_indices: &[usize],
    field_indices: &[usize],
    physics: &PhysicsParams,
    gamma: Complex64,
    tau: Complex64,
    beta: Complex64,
    is_self: bool,
) -> Array2<Complex64> {
    let n_source = source_indices.len();
    let n_field = field_indices.len();
    let mut block = Array2::zeros((n_source, n_field));

    for (i, &src_idx) in source_indices.iter().enumerate() {
        let source_elem = &elements[src_idx];
        if source_elem.property.is_evaluation() {
            continue;
        }

        for (j, &fld_idx) in field_indices.iter().enumerate() {
            let field_elem = &elements[fld_idx];
            if field_elem.property.is_evaluation() {
                continue;
            }

            let element_coords = get_element_coords(field_elem, nodes);

            let result = if is_self && src_idx == fld_idx {
                // Singular integration
                singular_integration(
                    &source_elem.center,
                    &source_elem.normal,
                    &element_coords,
                    field_elem.element_type,
                    physics,
                    None,
                    0,
                    false,
                )
            } else {
                // Regular integration
                regular_integration(
                    &source_elem.center,
                    &source_elem.normal,
                    &element_coords,
                    field_elem.element_type,
                    field_elem.area,
                    physics,
                    None,
                    0,
                    false,
                )
            };

            // Assemble using Burton-Miller formulation
            let coeff =
                result.dg_dn_integral * gamma * tau + result.d2g_dnxdny_integral * beta;
            block[[i, j]] = coeff;
        }
    }

    block
}

/// Build T-matrices (element to cluster multipole expansion)
fn build_t_matrices(
    system: &mut SlfmmSystem,
    elements: &[Element],
    clusters: &[Cluster],
    physics: &PhysicsParams,
    sphere_coords: &[[f64; 3]],
    sphere_weights: &[f64],
) {
    let k = physics.wave_number;
    let num_sphere_points = sphere_coords.len();

    for cluster in clusters {
        let num_elem = cluster.element_indices.len();
        let mut t_matrix = Array2::zeros((num_sphere_points, num_elem));

        for (j, &elem_idx) in cluster.element_indices.iter().enumerate() {
            let elem = &elements[elem_idx];
            if elem.property.is_evaluation() {
                continue;
            }

            // For each integration direction on the unit sphere
            for (p, coord) in sphere_coords.iter().enumerate() {
                // Compute exp(-ik * s · (y - cluster_center)) integrated over element
                // where s is the unit sphere direction

                // Simplified: use element center
                let diff: Vec<f64> = (0..3)
                    .map(|d| elem.center[d] - cluster.center[d])
                    .collect();
                let s_dot_diff: f64 = (0..3).map(|d| coord[d] * diff[d]).sum();

                let exp_factor =
                    Complex64::new((k * s_dot_diff).cos(), -(k * s_dot_diff).sin());

                t_matrix[[p, j]] = exp_factor * sphere_weights[p];
            }
        }

        system.t_matrices.push(t_matrix);
    }
}

/// Build D-matrices (cluster to cluster translation)
fn build_d_matrices(
    system: &mut SlfmmSystem,
    clusters: &[Cluster],
    physics: &PhysicsParams,
    sphere_coords: &[[f64; 3]],
    n_terms: usize,
) {
    let k = physics.wave_number;
    let num_sphere_points = sphere_coords.len();

    for (i, cluster_i) in clusters.iter().enumerate() {
        for &j in &cluster_i.far_clusters {
            let cluster_j = &clusters[j];

            // Distance vector between cluster centers
            let diff: Vec<f64> = (0..3)
                .map(|d| cluster_i.center[d] - cluster_j.center[d])
                .collect();
            let r = (diff[0] * diff[0] + diff[1] * diff[1] + diff[2] * diff[2]).sqrt();
            let kr = k * r;

            // Compute translation using spherical Hankel functions and Legendre polynomials
            let mut d_matrix = Array2::zeros((num_sphere_points, num_sphere_points));

            // Simplified: diagonal approximation using plane wave expansion
            // Full implementation would use multipole translation theorem
            let h_funcs = spherical_hankel_first_kind(n_terms.max(2), kr, 1.0);
            let _p_funcs = legendre_polynomials(n_terms.max(2), 0.0);

            for p in 0..num_sphere_points {
                // Simplified diagonal entry using monopole term
                d_matrix[[p, p]] = h_funcs[0] * Complex64::new(0.0, k);
            }

            system.d_matrices.push(DMatrixEntry {
                source_cluster: i,
                field_cluster: j,
                coefficients: d_matrix,
            });
        }
    }
}

/// Build S-matrices (cluster multipole to element evaluation)
fn build_s_matrices(
    system: &mut SlfmmSystem,
    elements: &[Element],
    clusters: &[Cluster],
    physics: &PhysicsParams,
    sphere_coords: &[[f64; 3]],
    sphere_weights: &[f64],
) {
    let k = physics.wave_number;
    let num_sphere_points = sphere_coords.len();

    for cluster in clusters {
        let num_elem = cluster.element_indices.len();
        let mut s_matrix = Array2::zeros((num_elem, num_sphere_points));

        for (j, &elem_idx) in cluster.element_indices.iter().enumerate() {
            let elem = &elements[elem_idx];
            if elem.property.is_evaluation() {
                continue;
            }

            // For each integration direction on the unit sphere
            for (p, coord) in sphere_coords.iter().enumerate() {
                // Compute exp(ik * s · (x - cluster_center))
                // where x is the field point and s is the unit sphere direction

                let diff: Vec<f64> = (0..3)
                    .map(|d| elem.center[d] - cluster.center[d])
                    .collect();
                let s_dot_diff: f64 = (0..3).map(|d| coord[d] * diff[d]).sum();

                let exp_factor =
                    Complex64::new((k * s_dot_diff).cos(), (k * s_dot_diff).sin());

                s_matrix[[j, p]] = exp_factor * sphere_weights[p];
            }
        }

        system.s_matrices.push(s_matrix);
    }
}

/// Get element node coordinates as Array2
fn get_element_coords(element: &Element, nodes: &Array2<f64>) -> Array2<f64> {
    let num_nodes = element.connectivity.len();
    let mut coords = Array2::zeros((num_nodes, 3));

    for (i, &node_idx) in element.connectivity.iter().enumerate() {
        for j in 0..3 {
            coords[[i, j]] = nodes[[node_idx, j]];
        }
    }

    coords
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{BoundaryCondition, ElementProperty, ElementType};
    use ndarray::array;

    fn make_test_cluster() -> Cluster {
        let mut cluster = Cluster::new(array![0.5, 0.5, 0.0]);
        cluster.element_indices = vec![0, 1];
        cluster.near_clusters = vec![];
        cluster.far_clusters = vec![];
        cluster
    }

    fn make_test_elements() -> (Vec<Element>, Array2<f64>) {
        let nodes = Array2::from_shape_vec(
            (4, 3),
            vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.5, 1.0, 0.0, 1.5, 1.0, 0.0],
        )
        .unwrap();

        let elem0 = Element {
            connectivity: vec![0, 1, 2],
            element_type: ElementType::Tri3,
            property: ElementProperty::Surface,
            normal: array![0.0, 0.0, 1.0],
            node_normals: Array2::zeros((3, 3)),
            center: array![0.5, 1.0 / 3.0, 0.0],
            area: 0.5,
            boundary_condition: BoundaryCondition::Velocity(vec![Complex64::new(1.0, 0.0)]),
            group: 0,
            dof_addresses: vec![0],
        };

        let elem1 = Element {
            connectivity: vec![1, 3, 2],
            element_type: ElementType::Tri3,
            property: ElementProperty::Surface,
            normal: array![0.0, 0.0, 1.0],
            node_normals: Array2::zeros((3, 3)),
            center: array![1.0, 2.0 / 3.0, 0.0],
            area: 0.5,
            boundary_condition: BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]),
            group: 0,
            dof_addresses: vec![1],
        };

        (vec![elem0, elem1], nodes)
    }

    #[test]
    fn test_slfmm_system_creation() {
        let system = SlfmmSystem::new(10, 2, 32, 5);
        assert_eq!(system.num_dofs, 10);
        assert_eq!(system.num_sphere_points, 32);
        assert_eq!(system.num_expansion_terms, 5);
    }

    #[test]
    fn test_build_slfmm_system() {
        let (elements, nodes) = make_test_elements();
        let cluster = make_test_cluster();
        let clusters = vec![cluster];
        let physics = PhysicsParams::new(100.0, 343.0, 1.21, false);

        let system = build_slfmm_system(&elements, &nodes, &clusters, &physics, 4, 8, 5);

        assert_eq!(system.num_dofs, 2);
        assert_eq!(system.t_matrices.len(), 1);
        assert_eq!(system.s_matrices.len(), 1);
    }

    #[test]
    fn test_near_field_block() {
        let (elements, nodes) = make_test_elements();
        let physics = PhysicsParams::new(100.0, 343.0, 1.21, false);
        let gamma = Complex64::new(physics.gamma(), 0.0);
        let tau = Complex64::new(physics.tau, 0.0);
        let beta = physics.burton_miller_beta();

        let block = compute_near_block(
            &elements,
            &nodes,
            &[0, 1],
            &[0, 1],
            &physics,
            gamma,
            tau,
            beta,
            true,
        );

        assert_eq!(block.shape(), &[2, 2]);
        // Diagonal entries should be non-zero
        assert!(block[[0, 0]].norm() > 0.0);
        assert!(block[[1, 1]].norm() > 0.0);
    }
}
