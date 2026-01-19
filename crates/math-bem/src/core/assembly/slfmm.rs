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
//!
//! **Note**: This module requires either the `native` or `wasm` feature for parallel processing.
//! Parallelism is provided via rayon (native) or wasm-bindgen-rayon (WASM).

#![cfg(any(feature = "native", feature = "wasm"))]

use ndarray::{Array1, Array2};
use num_complex::Complex64;

use crate::core::parallel::{parallel_enumerate_filter_map, parallel_flat_map, parallel_map};

use crate::core::integration::{regular_integration, singular_integration, unit_sphere_quadrature};
use crate::core::types::{BoundaryCondition, Cluster, Element, PhysicsParams};
use math_audio_wave::special::spherical_hankel_first_kind;

/// Result of SLFMM assembly
#[derive(Clone)]
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
///
/// The D-matrix for SLFMM is diagonal (same multipole index in source and field),
/// so we store only the diagonal entries to save memory.
/// This reduces storage from O(n²) to O(n) per cluster pair.
#[derive(Debug, Clone)]
pub struct DMatrixEntry {
    /// Source cluster index
    pub source_cluster: usize,
    /// Field cluster index
    pub field_cluster: usize,
    /// Diagonal translation coefficients (length = num_sphere_points)
    /// The full D-matrix would be: D[p,q] = diagonal[p] if p == q, else 0
    pub diagonal: Array1<Complex64>,
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

    /// Extract the near-field matrix as a dense matrix for preconditioning
    ///
    /// This assembles only the near-field blocks into a dense matrix,
    /// which is much smaller than the full matrix (O(N) vs O(N²) entries).
    pub fn extract_near_field_matrix(&self) -> Array2<Complex64> {
        let mut matrix = Array2::zeros((self.num_dofs, self.num_dofs));

        for block in &self.near_matrix {
            let src_dofs = &self.cluster_dof_indices[block.source_cluster];
            let fld_dofs = &self.cluster_dof_indices[block.field_cluster];

            // Copy block to global matrix
            for (local_i, &global_i) in src_dofs.iter().enumerate() {
                for (local_j, &global_j) in fld_dofs.iter().enumerate() {
                    matrix[[global_i, global_j]] += block.coefficients[[local_i, local_j]];
                }
            }

            // Handle symmetric storage: also fill the (fld, src) part
            if block.source_cluster != block.field_cluster {
                for (local_i, &global_i) in src_dofs.iter().enumerate() {
                    for (local_j, &global_j) in fld_dofs.iter().enumerate() {
                        matrix[[global_j, global_i]] += block.coefficients[[local_i, local_j]];
                    }
                }
            }
        }

        matrix
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
        // === Near-field contribution: y += [N] * x (parallelized) ===
        // Compute all near-field block contributions in parallel, then sum them
        let near_contributions: Vec<Vec<(usize, Complex64)>> =
            parallel_flat_map(&self.near_matrix, |block| {
                let src_dofs = &self.cluster_dof_indices[block.source_cluster];
                let fld_dofs = &self.cluster_dof_indices[block.field_cluster];

                let mut contributions = Vec::new();

                // Gather x values from field cluster
                let x_local: Array1<Complex64> = Array1::from_iter(fld_dofs.iter().map(|&i| x[i]));

                // Apply block matrix
                let y_local = block.coefficients.dot(&x_local);

                // Collect contributions for source DOFs
                for (local_i, &global_i) in src_dofs.iter().enumerate() {
                    contributions.push((global_i, y_local[local_i]));
                }

                // Handle symmetric storage
                if block.source_cluster != block.field_cluster {
                    let x_src: Array1<Complex64> =
                        Array1::from_iter(src_dofs.iter().map(|&i| x[i]));
                    let y_fld = block.coefficients.t().dot(&x_src);
                    for (local_j, &global_j) in fld_dofs.iter().enumerate() {
                        contributions.push((global_j, y_fld[local_j]));
                    }
                }

                vec![contributions]
            });

        // Accumulate near-field contributions
        let mut y = Array1::zeros(self.num_dofs);
        for contributions in near_contributions {
            for (idx, val) in contributions {
                y[idx] += val;
            }
        }

        // === Far-field contribution: y += [S][D][T] * x (parallelized) ===

        // Step 1: Compute multipole expansions for each cluster in parallel: t[c] = T[c] * x[c]
        let multipoles: Vec<Array1<Complex64>> = crate::core::parallel::parallel_enumerate_map(
            &self.t_matrices,
            |cluster_idx, t_mat| {
                let cluster_dofs = &self.cluster_dof_indices[cluster_idx];
                if cluster_dofs.is_empty() || t_mat.is_empty() {
                    Array1::zeros(self.num_sphere_points)
                } else {
                    let x_local: Array1<Complex64> =
                        Array1::from_iter(cluster_dofs.iter().map(|&i| x[i]));
                    t_mat.dot(&x_local)
                }
            },
        );

        // Step 2: Translate multipoles between far clusters (parallelized)
        // Compute all D*multipole products in parallel
        // D-matrix is diagonal, so D*x is just element-wise multiplication
        let d_contributions: Vec<(usize, Array1<Complex64>)> =
            parallel_map(&self.d_matrices, |d_entry| {
                let src_mult = &multipoles[d_entry.source_cluster];
                // Diagonal matrix-vector multiply: translated[i] = diagonal[i] * src_mult[i]
                let translated = &d_entry.diagonal * src_mult;
                (d_entry.field_cluster, translated)
            });

        // Accumulate D-matrix contributions into locals (sequential to avoid race)
        let mut locals: Vec<Array1<Complex64>> = (0..self.num_clusters)
            .map(|_| Array1::zeros(self.num_sphere_points))
            .collect();

        for (field_cluster, translated) in d_contributions {
            for i in 0..self.num_sphere_points {
                locals[field_cluster][i] += translated[i];
            }
        }

        // Step 3: Evaluate locals at field points in parallel: y[c] += S[c] * locals[c]
        let far_contributions: Vec<Vec<(usize, Complex64)>> =
            parallel_enumerate_filter_map(&self.s_matrices, |cluster_idx, s_mat| {
                let cluster_dofs = &self.cluster_dof_indices[cluster_idx];
                if cluster_dofs.is_empty() || s_mat.is_empty() {
                    return None;
                }
                let y_local = s_mat.dot(&locals[cluster_idx]);
                let contributions: Vec<(usize, Complex64)> = cluster_dofs
                    .iter()
                    .enumerate()
                    .map(|(local_j, &global_j)| (global_j, y_local[local_j]))
                    .collect();
                Some(contributions)
            });

        // Accumulate far-field contributions
        for contributions in far_contributions {
            for (idx, val) in contributions {
                y[idx] += val;
            }
        }

        y
    }

    /// Apply the SLFMM operator transpose: y = A^T * x (parallelized)
    ///
    /// Used by some iterative solvers (e.g., BiCGSTAB).
    pub fn matvec_transpose(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        // === Near-field contribution (transposed, parallelized) ===
        let near_contributions: Vec<Vec<(usize, Complex64)>> =
            parallel_flat_map(&self.near_matrix, |block| {
                let src_dofs = &self.cluster_dof_indices[block.source_cluster];
                let fld_dofs = &self.cluster_dof_indices[block.field_cluster];

                let mut contributions = Vec::new();

                // For transpose: gather from source DOFs, scatter to field DOFs
                let x_local: Array1<Complex64> = Array1::from_iter(src_dofs.iter().map(|&i| x[i]));
                let y_local = block.coefficients.t().dot(&x_local);
                for (local_j, &global_j) in fld_dofs.iter().enumerate() {
                    contributions.push((global_j, y_local[local_j]));
                }

                // Symmetric storage
                if block.source_cluster != block.field_cluster {
                    let x_fld: Array1<Complex64> =
                        Array1::from_iter(fld_dofs.iter().map(|&i| x[i]));
                    let y_src = block.coefficients.dot(&x_fld);
                    for (local_i, &global_i) in src_dofs.iter().enumerate() {
                        contributions.push((global_i, y_src[local_i]));
                    }
                }

                vec![contributions]
            });

        // Accumulate near-field contributions
        let mut y = Array1::zeros(self.num_dofs);
        for contributions in near_contributions {
            for (idx, val) in contributions {
                y[idx] += val;
            }
        }

        // === Far-field contribution (transposed, parallelized): y += T^T * D^T * S^T * x ===

        // Step 1: S^T * x (parallelized)
        let locals: Vec<Array1<Complex64>> = crate::core::parallel::parallel_enumerate_map(
            &self.s_matrices,
            |cluster_idx, s_mat| {
                let cluster_dofs = &self.cluster_dof_indices[cluster_idx];
                if cluster_dofs.is_empty() || s_mat.is_empty() {
                    Array1::zeros(self.num_sphere_points)
                } else {
                    let x_local: Array1<Complex64> =
                        Array1::from_iter(cluster_dofs.iter().map(|&i| x[i]));
                    s_mat.t().dot(&x_local)
                }
            },
        );

        // Step 2: D^T translation (parallelized)
        // D-matrix is diagonal, so D^T = D (diagonal matrices are symmetric)
        let d_contributions: Vec<(usize, Array1<Complex64>)> =
            parallel_map(&self.d_matrices, |d_entry| {
                let fld_local = &locals[d_entry.field_cluster];
                // Diagonal matrix-vector multiply: translated[i] = diagonal[i] * fld_local[i]
                let translated = &d_entry.diagonal * fld_local;
                (d_entry.source_cluster, translated)
            });

        // Accumulate D-matrix contributions
        let mut multipoles: Vec<Array1<Complex64>> = (0..self.num_clusters)
            .map(|_| Array1::zeros(self.num_sphere_points))
            .collect();

        for (source_cluster, translated) in d_contributions {
            for i in 0..self.num_sphere_points {
                multipoles[source_cluster][i] += translated[i];
            }
        }

        // Step 3: T^T * multipoles (parallelized)
        let far_contributions: Vec<Vec<(usize, Complex64)>> =
            parallel_enumerate_filter_map(&self.t_matrices, |cluster_idx, t_mat| {
                let cluster_dofs = &self.cluster_dof_indices[cluster_idx];
                if cluster_dofs.is_empty() || t_mat.is_empty() {
                    return None;
                }
                let y_local = t_mat.t().dot(&multipoles[cluster_idx]);
                let contributions: Vec<(usize, Complex64)> = cluster_dofs
                    .iter()
                    .enumerate()
                    .map(|(local_i, &global_i)| (global_i, y_local[local_i]))
                    .collect();
                Some(contributions)
            });

        // Accumulate far-field contributions
        for contributions in far_contributions {
            for (idx, val) in contributions {
                y[idx] += val;
            }
        }

        y
    }
}

use math_audio_solvers::traits::LinearOperator;

impl LinearOperator<Complex64> for SlfmmSystem {
    fn num_rows(&self) -> usize {
        self.num_dofs
    }

    fn num_cols(&self) -> usize {
        self.num_dofs
    }

    fn apply(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        self.matvec(x)
    }

    fn apply_transpose(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        self.matvec_transpose(x)
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

/// Build near-field matrix blocks (parallelized)
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

    // Collect all cluster pairs that need processing
    let mut cluster_pairs: Vec<(usize, usize, bool)> = Vec::new();

    for (i, cluster_i) in clusters.iter().enumerate() {
        // Self-interaction
        cluster_pairs.push((i, i, true));

        // Interaction with near clusters (only upper triangle)
        for &j in &cluster_i.near_clusters {
            if j > i {
                cluster_pairs.push((i, j, false));
            }
        }
    }

    // Compute all near-field blocks in parallel
    let near_blocks: Vec<NearFieldBlock> = parallel_map(&cluster_pairs, |&(i, j, is_self)| {
        let cluster_i = &clusters[i];
        let cluster_j = &clusters[j];

        let mut block = compute_near_block(
            elements,
            nodes,
            &cluster_i.element_indices,
            &cluster_j.element_indices,
            physics,
            gamma,
            tau,
            beta,
            is_self,
        );

        // Add free terms to diagonal for self-interaction blocks
        if is_self {
            for local_idx in 0..cluster_i.element_indices.len() {
                let elem_idx = cluster_i.element_indices[local_idx];
                let elem = &elements[elem_idx];
                if elem.property.is_evaluation() {
                    continue;
                }
                match &elem.boundary_condition {
                    BoundaryCondition::Velocity(_)
                    | BoundaryCondition::VelocityWithAdmittance { .. } => {
                        block[[local_idx, local_idx]] += gamma * 0.5;
                    }
                    BoundaryCondition::Pressure(_) => {
                        block[[local_idx, local_idx]] += beta * tau * 0.5;
                    }
                    _ => {}
                }
            }
        }

        NearFieldBlock {
            source_cluster: i,
            field_cluster: j,
            coefficients: block,
        }
    });

    system.near_matrix = near_blocks;
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
            let coeff = result.dg_dn_integral * gamma * tau + result.d2g_dnxdny_integral * beta;
            block[[i, j]] = coeff;
        }
    }

    block
}

/// Build T-matrices (element to cluster multipole expansion) - parallelized
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

    // Build all T-matrices in parallel
    let t_matrices: Vec<Array2<Complex64>> = parallel_map(clusters, |cluster| {
        let num_elem = cluster.element_indices.len();
        let mut t_matrix = Array2::zeros((num_sphere_points, num_elem));

        for (j, &elem_idx) in cluster.element_indices.iter().enumerate() {
            let elem = &elements[elem_idx];
            if elem.property.is_evaluation() {
                continue;
            }

            // Precompute difference vector
            let diff: [f64; 3] = [
                elem.center[0] - cluster.center[0],
                elem.center[1] - cluster.center[1],
                elem.center[2] - cluster.center[2],
            ];

            // For each integration direction on the unit sphere
            for (p, coord) in sphere_coords.iter().enumerate() {
                let s_dot_diff = coord[0] * diff[0] + coord[1] * diff[1] + coord[2] * diff[2];
                let exp_factor = Complex64::new((k * s_dot_diff).cos(), -(k * s_dot_diff).sin());
                t_matrix[[p, j]] = exp_factor * sphere_weights[p];
            }
        }

        t_matrix
    });

    system.t_matrices = t_matrices;
}

/// Build D-matrices (cluster to cluster translation) - parallelized
///
/// The D-matrix is diagonal in the single-level FMM, so we only store
/// the diagonal entries. This reduces memory from O(P²) to O(P) per pair
/// where P is the number of sphere integration points.
fn build_d_matrices(
    system: &mut SlfmmSystem,
    clusters: &[Cluster],
    physics: &PhysicsParams,
    _sphere_coords: &[[f64; 3]],
    n_terms: usize,
) {
    let k = physics.wave_number;
    let num_sphere_points = system.num_sphere_points;

    // Collect all far cluster pairs
    let mut far_pairs: Vec<(usize, usize)> = Vec::new();
    for (i, cluster_i) in clusters.iter().enumerate() {
        for &j in &cluster_i.far_clusters {
            far_pairs.push((i, j));
        }
    }

    // Log memory estimate
    let num_pairs = far_pairs.len();
    let mem_per_pair = num_sphere_points * std::mem::size_of::<Complex64>();
    let total_mem_mb = (num_pairs * mem_per_pair) as f64 / (1024.0 * 1024.0);
    if total_mem_mb > 100.0 {
        eprintln!(
            "    D-matrices: {} far pairs × {} points = {:.1} MB",
            num_pairs, num_sphere_points, total_mem_mb
        );
    }

    // Compute all D-matrices in parallel (diagonal storage only)
    let d_matrices: Vec<DMatrixEntry> = parallel_map(&far_pairs, |&(i, j)| {
        let cluster_i = &clusters[i];
        let cluster_j = &clusters[j];

        // Distance vector between cluster centers
        let diff = [
            cluster_i.center[0] - cluster_j.center[0],
            cluster_i.center[1] - cluster_j.center[1],
            cluster_i.center[2] - cluster_j.center[2],
        ];
        let r = (diff[0] * diff[0] + diff[1] * diff[1] + diff[2] * diff[2]).sqrt();
        let kr = k * r;

        // Compute translation using spherical Hankel functions
        let h_funcs = spherical_hankel_first_kind(n_terms.max(2), kr, 1.0);

        // D-matrix is diagonal: D[p,p] = h_0(kr) * ik
        // Store only the diagonal (all entries are the same in this simplified model)
        let d_value = h_funcs[0] * Complex64::new(0.0, k);
        let diagonal = Array1::from_elem(num_sphere_points, d_value);

        DMatrixEntry {
            source_cluster: i,
            field_cluster: j,
            diagonal,
        }
    });

    system.d_matrices = d_matrices;
}

/// Build S-matrices (cluster multipole to element evaluation) - parallelized
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

    // Build all S-matrices in parallel
    let s_matrices: Vec<Array2<Complex64>> = parallel_map(clusters, |cluster| {
        let num_elem = cluster.element_indices.len();
        let mut s_matrix = Array2::zeros((num_elem, num_sphere_points));

        for (j, &elem_idx) in cluster.element_indices.iter().enumerate() {
            let elem = &elements[elem_idx];
            if elem.property.is_evaluation() {
                continue;
            }

            // Precompute difference vector
            let diff: [f64; 3] = [
                elem.center[0] - cluster.center[0],
                elem.center[1] - cluster.center[1],
                elem.center[2] - cluster.center[2],
            ];

            // For each integration direction on the unit sphere
            for (p, coord) in sphere_coords.iter().enumerate() {
                let s_dot_diff = coord[0] * diff[0] + coord[1] * diff[1] + coord[2] * diff[2];
                let exp_factor = Complex64::new((k * s_dot_diff).cos(), (k * s_dot_diff).sin());
                s_matrix[[j, p]] = exp_factor * sphere_weights[p];
            }
        }

        s_matrix
    });

    system.s_matrices = s_matrices;
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
