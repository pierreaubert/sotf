//! Multi-Level Fast Multipole Method (MLFMM) assembly
//!
//! Direct port of NC_BuildSystemFMBEM (MLFMM mode) from NC_EquationSystem.cpp.
//!
//! The MLFMM extends SLFMM by using a hierarchical cluster tree:
//! ```text
//! Level 0:    Root cluster
//!             |
//! Level 1:    Child clusters
//!             |
//! Level 2:    Leaf clusters (elements)
//! ```
//!
//! The matrix-vector product proceeds:
//! 1. Upward pass: Aggregate sources from leaves to root using T-matrices
//! 2. Translation: Translate expansions between far clusters using D-matrices
//! 3. Downward pass: Disaggregate to leaves using S-matrices
//! 4. Near-field: Direct interactions at leaf level

use ndarray::{Array1, Array2};
use num_complex::Complex64;

use crate::core::integration::{regular_integration, singular_integration, unit_sphere_quadrature};
use crate::core::types::{Cluster, ClusterLevel, Element, PhysicsParams};
use math_wave::special::spherical_hankel_first_kind;

/// Result of MLFMM assembly
pub struct MlfmmSystem {
    /// Number of levels in the tree
    pub num_levels: usize,
    /// Near-field coefficient matrix at leaf level (sparse, stored as dense blocks)
    pub near_matrix: Vec<NearFieldBlock>,
    /// T-matrices at each level (child-to-parent aggregation)
    /// Level 0 has no T-matrices (root)
    pub t_matrices: Vec<LevelMatrices>,
    /// D-matrices for far-field translation at each level
    pub d_matrices: Vec<Vec<DMatrixEntry>>,
    /// S-matrices at each level (parent-to-child disaggregation)
    pub s_matrices: Vec<LevelMatrices>,
    /// RHS vector
    pub rhs: Array1<Complex64>,
    /// Number of DOFs
    pub num_dofs: usize,
    /// Cluster tree (indexed by level)
    cluster_levels: Vec<ClusterLevel>,
    /// DOF mappings at leaf level: for each leaf cluster, the global DOF indices
    pub leaf_dof_indices: Vec<Vec<usize>>,
    /// Number of sphere points at each level
    pub sphere_points_per_level: Vec<usize>,
}

/// Near-field block between two leaf clusters
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
/// The D-matrix for FMM is diagonal (same multipole index in source and field),
/// so we store only the diagonal entries to save memory.
#[derive(Debug, Clone)]
pub struct DMatrixEntry {
    /// Source cluster index
    pub source_cluster: usize,
    /// Field cluster index
    pub field_cluster: usize,
    /// Level in tree
    pub level: usize,
    /// Diagonal translation coefficients (length = num_sphere_points)
    pub diagonal: Array1<Complex64>,
}

/// Matrices at a single level of the tree
#[derive(Debug, Clone)]
pub struct LevelMatrices {
    /// Level index
    pub level: usize,
    /// Number of clusters at this level
    pub num_clusters: usize,
    /// Matrices indexed by cluster
    pub matrices: Vec<Array2<Complex64>>,
}

impl MlfmmSystem {
    /// Create a new empty MLFMM system
    pub fn new(num_dofs: usize, num_levels: usize, cluster_levels: Vec<ClusterLevel>) -> Self {
        let sphere_points_per_level: Vec<usize> = cluster_levels
            .iter()
            .map(|level| level.theta_points * level.phi_points)
            .collect();

        Self {
            num_levels,
            near_matrix: Vec::new(),
            t_matrices: Vec::with_capacity(num_levels),
            d_matrices: Vec::with_capacity(num_levels),
            s_matrices: Vec::with_capacity(num_levels),
            rhs: Array1::zeros(num_dofs),
            num_dofs,
            cluster_levels,
            leaf_dof_indices: Vec::new(),
            sphere_points_per_level,
        }
    }

    /// Apply the MLFMM operator: y = ([N] + [S][D][T]) * x
    ///
    /// Multi-level version with upward/downward passes.
    ///
    /// The algorithm proceeds as follows:
    /// 1. **Near-field**: Direct interactions between nearby leaf clusters
    /// 2. **Upward pass**: Aggregate source multipoles from leaves to root (M2M translations)
    /// 3. **Translation**: Translate multipoles to locals between far clusters at each level (M2L)
    /// 4. **Downward pass**: Disaggregate local expansions from root to leaves (L2L translations)
    /// 5. **Evaluation**: Evaluate local expansions at leaf DOFs
    ///
    /// # Arguments
    /// * `x` - Input vector (length = num_dofs)
    ///
    /// # Returns
    /// * `y` - Output vector (length = num_dofs), y = A*x
    pub fn matvec(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        let mut y = Array1::zeros(self.num_dofs);

        if self.num_levels == 0 {
            return y;
        }

        let leaf_level = self.num_levels - 1;

        // === 1. Near-field contribution: y += [N] * x ===
        // Note: block.coefficients is (n_source, n_field) where:
        // - source = collocation points (rows, where we evaluate)
        // - field = integration elements (columns, source of influence)
        // For y = A*x: gather x from field DOFs, scatter y to source DOFs
        for block in &self.near_matrix {
            if block.source_cluster >= self.leaf_dof_indices.len()
                || block.field_cluster >= self.leaf_dof_indices.len()
            {
                continue;
            }

            let src_dofs = &self.leaf_dof_indices[block.source_cluster];
            let fld_dofs = &self.leaf_dof_indices[block.field_cluster];

            // Gather x values from field cluster (columns of the matrix)
            let x_local: Array1<Complex64> = Array1::from_iter(fld_dofs.iter().map(|&i| x[i]));

            // Apply block matrix: y_local[i] = sum_j A[i,j] * x[j]
            let y_local = block.coefficients.dot(&x_local);

            // Scatter to source DOFs (rows of the matrix)
            for (local_i, &global_i) in src_dofs.iter().enumerate() {
                if local_i < y_local.len() {
                    y[global_i] += y_local[local_i];
                }
            }

            // Handle symmetric storage: if src != fld, also apply the (fld, src) block
            // which is the transpose of this block
            if block.source_cluster != block.field_cluster {
                // Gather x from source cluster DOFs
                let x_src: Array1<Complex64> = Array1::from_iter(src_dofs.iter().map(|&i| x[i]));
                // Apply transpose: the (fld, src) block
                let y_fld = block.coefficients.t().dot(&x_src);
                // Scatter to field cluster DOFs
                for (local_j, &global_j) in fld_dofs.iter().enumerate() {
                    if local_j < y_fld.len() {
                        y[global_j] += y_fld[local_j];
                    }
                }
            }
        }

        // === 2. Far-field contribution using multi-level passes ===
        if self.num_levels > 1 && !self.t_matrices.is_empty() {
            // 2a. Upward pass: compute multipole expansions at each level
            let multipoles = self.upward_pass(x);

            // 2b. Translation at each level: M2L (multipole to local)
            let mut locals = self.translate_all_levels(&multipoles);

            // 2c. Downward pass: propagate locals from root to leaves
            self.downward_pass(&mut locals);

            // 2d. Evaluate locals at leaf DOFs
            self.evaluate_locals(&locals, leaf_level, &mut y);
        }

        y
    }

    /// Upward pass: aggregate sources using T-matrices (M2M translations)
    ///
    /// At leaf level: T_leaf * x → multipole expansion
    /// At non-leaf levels: Aggregate child multipoles → parent multipole
    fn upward_pass(&self, x: &Array1<Complex64>) -> Vec<Vec<Array1<Complex64>>> {
        let mut multipoles: Vec<Vec<Array1<Complex64>>> = Vec::with_capacity(self.num_levels);

        // Initialize storage for each level
        for level in 0..self.num_levels {
            let num_clusters = if level < self.cluster_levels.len() {
                self.cluster_levels[level].clusters.len()
            } else {
                0
            };
            let num_points = if level < self.sphere_points_per_level.len() {
                self.sphere_points_per_level[level]
            } else {
                0
            };
            multipoles.push(
                (0..num_clusters)
                    .map(|_| Array1::zeros(num_points))
                    .collect(),
            );
        }

        let leaf_level = self.num_levels - 1;

        // Step 1: Compute leaf-level multipoles from source DOFs
        if leaf_level < self.t_matrices.len() {
            let t_level = &self.t_matrices[leaf_level];
            for cluster_idx in 0..t_level.num_clusters {
                if cluster_idx >= self.leaf_dof_indices.len() {
                    continue;
                }
                let cluster_dofs = &self.leaf_dof_indices[cluster_idx];
                if cluster_dofs.is_empty() {
                    continue;
                }
                let t_mat = &t_level.matrices[cluster_idx];
                if t_mat.is_empty() {
                    continue;
                }
                let x_local: Array1<Complex64> =
                    Array1::from_iter(cluster_dofs.iter().map(|&i| x[i]));
                if x_local.len() == t_mat.ncols() {
                    multipoles[leaf_level][cluster_idx] = t_mat.dot(&x_local);
                }
            }
        }

        // Step 2: Propagate upward through tree (M2M translations)
        // Traverse from leaf-1 to root (level 0)
        for level in (0..leaf_level).rev() {
            if level >= self.t_matrices.len() || level >= self.cluster_levels.len() {
                continue;
            }

            let t_level = &self.t_matrices[level];
            let clusters = &self.cluster_levels[level].clusters;

            for (cluster_idx, cluster) in clusters.iter().enumerate() {
                if cluster_idx >= t_level.matrices.len() {
                    continue;
                }

                // Aggregate multipoles from all children
                let t_mat = &t_level.matrices[cluster_idx];
                if t_mat.is_empty() || cluster.sons.is_empty() {
                    continue;
                }

                // Concatenate child multipoles and apply T-matrix
                let child_level = level + 1;
                if child_level >= multipoles.len() {
                    continue;
                }

                let child_num_points = self
                    .sphere_points_per_level
                    .get(child_level)
                    .copied()
                    .unwrap_or(0);
                let mut child_multipoles = Array1::zeros(cluster.sons.len() * child_num_points);

                for (son_idx, &child_cluster_idx) in cluster.sons.iter().enumerate() {
                    if child_cluster_idx < multipoles[child_level].len() {
                        let child_mult = &multipoles[child_level][child_cluster_idx];
                        let offset = son_idx * child_num_points;
                        for (i, &val) in child_mult.iter().enumerate() {
                            if offset + i < child_multipoles.len() {
                                child_multipoles[offset + i] = val;
                            }
                        }
                    }
                }

                if child_multipoles.len() == t_mat.ncols() {
                    multipoles[level][cluster_idx] = t_mat.dot(&child_multipoles);
                }
            }
        }

        multipoles
    }

    /// Translate multipoles to locals at each level (M2L translations)
    fn translate_all_levels(
        &self,
        multipoles: &[Vec<Array1<Complex64>>],
    ) -> Vec<Vec<Array1<Complex64>>> {
        let mut locals: Vec<Vec<Array1<Complex64>>> = Vec::with_capacity(self.num_levels);

        // Initialize storage
        for level in 0..self.num_levels {
            let num_clusters = if level < self.cluster_levels.len() {
                self.cluster_levels[level].clusters.len()
            } else {
                0
            };
            let num_points = self
                .sphere_points_per_level
                .get(level)
                .copied()
                .unwrap_or(0);
            locals.push(
                (0..num_clusters)
                    .map(|_| Array1::zeros(num_points))
                    .collect(),
            );
        }

        // Apply D-matrices at each level
        for (level, d_entries) in self.d_matrices.iter().enumerate() {
            if level >= multipoles.len() || level >= locals.len() {
                continue;
            }

            for d_entry in d_entries {
                if d_entry.source_cluster >= multipoles[level].len()
                    || d_entry.field_cluster >= locals[level].len()
                {
                    continue;
                }

                let src_mult = &multipoles[level][d_entry.source_cluster];
                if src_mult.len() != d_entry.diagonal.len() {
                    continue;
                }

                // D-matrix is diagonal, so D*x is element-wise multiplication
                for (i, (&d, &s)) in d_entry.diagonal.iter().zip(src_mult.iter()).enumerate() {
                    if i < locals[level][d_entry.field_cluster].len() {
                        locals[level][d_entry.field_cluster][i] += d * s;
                    }
                }
            }
        }

        locals
    }

    /// Downward pass: propagate locals from root to leaves (L2L translations)
    fn downward_pass(&self, locals: &mut [Vec<Array1<Complex64>>]) {
        // Traverse from root to leaves (level 0 to leaf_level)
        for level in 0..self.num_levels.saturating_sub(1) {
            if level >= self.s_matrices.len() || level >= self.cluster_levels.len() {
                continue;
            }

            let s_level = &self.s_matrices[level];
            let clusters = &self.cluster_levels[level].clusters;

            for (cluster_idx, cluster) in clusters.iter().enumerate() {
                if cluster_idx >= s_level.matrices.len() || cluster.sons.is_empty() {
                    continue;
                }

                let s_mat = &s_level.matrices[cluster_idx];
                if s_mat.is_empty() {
                    continue;
                }

                // Propagate parent local to children
                let parent_local = &locals[level][cluster_idx];
                if parent_local.len() != s_mat.ncols() {
                    continue;
                }

                let child_locals = s_mat.dot(parent_local);
                let child_level = level + 1;
                if child_level >= locals.len() {
                    continue;
                }

                let child_num_points = self
                    .sphere_points_per_level
                    .get(child_level)
                    .copied()
                    .unwrap_or(0);

                // Distribute to children
                for (son_idx, &child_cluster_idx) in cluster.sons.iter().enumerate() {
                    if child_cluster_idx >= locals[child_level].len() {
                        continue;
                    }

                    let offset = son_idx * child_num_points;
                    for i in 0..child_num_points {
                        if offset + i < child_locals.len()
                            && i < locals[child_level][child_cluster_idx].len()
                        {
                            locals[child_level][child_cluster_idx][i] += child_locals[offset + i];
                        }
                    }
                }
            }
        }
    }

    /// Evaluate local expansions at leaf-level DOFs
    fn evaluate_locals(
        &self,
        locals: &[Vec<Array1<Complex64>>],
        leaf_level: usize,
        y: &mut Array1<Complex64>,
    ) {
        if leaf_level >= self.s_matrices.len() || leaf_level >= locals.len() {
            return;
        }

        let s_level = &self.s_matrices[leaf_level];

        for cluster_idx in 0..s_level.num_clusters {
            if cluster_idx >= self.leaf_dof_indices.len() || cluster_idx >= locals[leaf_level].len()
            {
                continue;
            }

            let cluster_dofs = &self.leaf_dof_indices[cluster_idx];
            if cluster_dofs.is_empty() {
                continue;
            }

            let s_mat = &s_level.matrices[cluster_idx];
            if s_mat.is_empty() {
                continue;
            }

            let local_exp = &locals[leaf_level][cluster_idx];
            if local_exp.len() != s_mat.ncols() {
                continue;
            }

            let y_local = s_mat.dot(local_exp);
            for (local_j, &global_j) in cluster_dofs.iter().enumerate() {
                if local_j < y_local.len() && global_j < y.len() {
                    y[global_j] += y_local[local_j];
                }
            }
        }
    }

    /// Get cluster at given level and index
    pub fn get_cluster(&self, level: usize, index: usize) -> Option<&Cluster> {
        self.cluster_levels.get(level)?.clusters.get(index)
    }

    /// Get number of clusters at a given level
    pub fn num_clusters_at_level(&self, level: usize) -> usize {
        self.cluster_levels
            .get(level)
            .map(|l| l.clusters.len())
            .unwrap_or(0)
    }
}

/// Build the MLFMM system matrices
///
/// # Arguments
/// * `elements` - Vector of mesh elements
/// * `nodes` - Node coordinates (num_nodes × 3)
/// * `cluster_levels` - Hierarchical cluster tree
/// * `physics` - Physics parameters
pub fn build_mlfmm_system(
    elements: &[Element],
    nodes: &Array2<f64>,
    cluster_levels: Vec<ClusterLevel>,
    physics: &PhysicsParams,
) -> MlfmmSystem {
    let num_dofs = count_dofs(elements);
    let num_levels = cluster_levels.len();

    let mut system = MlfmmSystem::new(num_dofs, num_levels, cluster_levels.clone());

    // Build leaf DOF mappings (needed for matvec)
    if let Some(leaf_level) = cluster_levels.last() {
        build_leaf_dof_mappings(&mut system, elements, &leaf_level.clusters);
    }

    // Build near-field matrix at leaf level
    if let Some(leaf_level) = cluster_levels.last() {
        build_near_field(&mut system, elements, nodes, &leaf_level.clusters, physics);
    }

    // Build T-matrices at each level (leaves to root)
    for (level_idx, level) in cluster_levels.iter().enumerate().rev() {
        let (sphere_coords, sphere_weights) =
            unit_sphere_quadrature(level.theta_points, level.phi_points);

        let t_matrices = build_t_matrices_level(
            elements,
            &level.clusters,
            physics,
            &sphere_coords,
            &sphere_weights,
            level_idx,
            &cluster_levels,
        );

        system.t_matrices.push(t_matrices);
    }
    system.t_matrices.reverse(); // Order from root to leaves

    // Build D-matrices at each level
    for (level_idx, level) in cluster_levels.iter().enumerate() {
        let num_sphere_points = level.theta_points * level.phi_points;

        let d_matrices = build_d_matrices_level(
            &level.clusters,
            physics,
            &[], // sphere_coords not needed for diagonal D-matrices
            level.expansion_terms,
            level_idx,
            num_sphere_points,
        );

        system.d_matrices.push(d_matrices);
    }

    // Build S-matrices at each level (root to leaves)
    for (level_idx, level) in cluster_levels.iter().enumerate() {
        let (sphere_coords, sphere_weights) =
            unit_sphere_quadrature(level.theta_points, level.phi_points);

        let s_matrices = build_s_matrices_level(
            elements,
            &level.clusters,
            physics,
            &sphere_coords,
            &sphere_weights,
            level_idx,
            &cluster_levels,
        );

        system.s_matrices.push(s_matrices);
    }

    system
}

/// Build the mapping from leaf clusters to global DOF indices
fn build_leaf_dof_mappings(
    system: &mut MlfmmSystem,
    elements: &[Element],
    leaf_clusters: &[Cluster],
) {
    for cluster in leaf_clusters {
        let mut dof_indices = Vec::new();
        for &elem_idx in &cluster.element_indices {
            let elem = &elements[elem_idx];
            if elem.property.is_evaluation() {
                continue;
            }
            // Collect all DOF addresses for this element
            dof_indices.extend(elem.dof_addresses.iter().copied());
        }
        system.leaf_dof_indices.push(dof_indices);
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

/// Build near-field matrix blocks at leaf level
fn build_near_field(
    system: &mut MlfmmSystem,
    elements: &[Element],
    nodes: &Array2<f64>,
    leaf_clusters: &[Cluster],
    physics: &PhysicsParams,
) {
    let gamma = Complex64::new(physics.gamma(), 0.0);
    let tau = Complex64::new(physics.tau, 0.0);
    let beta = physics.burton_miller_beta();

    // For each leaf cluster pair in near-field
    for (i, cluster_i) in leaf_clusters.iter().enumerate() {
        // Self-interaction
        let block = compute_near_block(
            elements,
            nodes,
            &cluster_i.element_indices,
            &cluster_i.element_indices,
            physics,
            gamma,
            tau,
            beta,
            true,
        );
        system.near_matrix.push(NearFieldBlock {
            source_cluster: i,
            field_cluster: i,
            coefficients: block,
        });

        // Interaction with near clusters
        for &j in &cluster_i.near_clusters {
            if j > i {
                let cluster_j = &leaf_clusters[j];
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
            let coeff = result.dg_dn_integral * gamma * tau + result.d2g_dnxdny_integral * beta;
            block[[i, j]] = coeff;
        }
    }

    block
}

/// Build T-matrices at a single level
fn build_t_matrices_level(
    elements: &[Element],
    clusters: &[Cluster],
    physics: &PhysicsParams,
    sphere_coords: &[[f64; 3]],
    sphere_weights: &[f64],
    level_idx: usize,
    cluster_levels: &[ClusterLevel],
) -> LevelMatrices {
    let k = physics.wave_number;
    let num_sphere_points = sphere_coords.len();
    let num_clusters = clusters.len();
    let mut matrices = Vec::with_capacity(num_clusters);

    let is_leaf_level = level_idx == cluster_levels.len() - 1;

    for cluster in clusters {
        if is_leaf_level {
            // Leaf level: T-matrix maps element DOFs to multipole expansion
            let num_elem = cluster.element_indices.len();
            let mut t_matrix = Array2::zeros((num_sphere_points, num_elem));

            for (j, &elem_idx) in cluster.element_indices.iter().enumerate() {
                let elem = &elements[elem_idx];
                if elem.property.is_evaluation() {
                    continue;
                }

                for (p, coord) in sphere_coords.iter().enumerate() {
                    let diff: Vec<f64> =
                        (0..3).map(|d| elem.center[d] - cluster.center[d]).collect();
                    let s_dot_diff: f64 = (0..3).map(|d| coord[d] * diff[d]).sum();

                    let exp_factor =
                        Complex64::new((k * s_dot_diff).cos(), -(k * s_dot_diff).sin());

                    t_matrix[[p, j]] = exp_factor * sphere_weights[p];
                }
            }

            matrices.push(t_matrix);
        } else {
            // Non-leaf: T-matrix aggregates child multipoles
            let num_children = cluster.sons.len();
            let child_num_points = if level_idx + 1 < cluster_levels.len() {
                cluster_levels[level_idx + 1].theta_points
                    * cluster_levels[level_idx + 1].phi_points
            } else {
                num_sphere_points
            };

            let mut t_matrix = Array2::zeros((num_sphere_points, num_children * child_num_points));

            // Build aggregation operator for each child
            for (child_idx, &child_cluster_idx) in cluster.sons.iter().enumerate() {
                if level_idx + 1 < cluster_levels.len() {
                    let child_cluster = &cluster_levels[level_idx + 1].clusters[child_cluster_idx];

                    // Translation from child center to parent center
                    for (p, coord) in sphere_coords.iter().enumerate() {
                        let diff: Vec<f64> = (0..3)
                            .map(|d| child_cluster.center[d] - cluster.center[d])
                            .collect();
                        let s_dot_diff: f64 = (0..3).map(|d| coord[d] * diff[d]).sum();

                        let exp_factor =
                            Complex64::new((k * s_dot_diff).cos(), -(k * s_dot_diff).sin());

                        // Map child sphere points to parent
                        for cp in 0..child_num_points {
                            t_matrix[[p, child_idx * child_num_points + cp]] =
                                exp_factor * sphere_weights[p] / child_num_points as f64;
                        }
                    }
                }
            }

            matrices.push(t_matrix);
        }
    }

    LevelMatrices {
        level: level_idx,
        num_clusters,
        matrices,
    }
}

/// Build D-matrices at a single level
///
/// The D-matrix is diagonal in the FMM, so we only store the diagonal entries.
/// This reduces memory from O(P²) to O(P) per cluster pair.
fn build_d_matrices_level(
    clusters: &[Cluster],
    physics: &PhysicsParams,
    _sphere_coords: &[[f64; 3]],
    n_terms: usize,
    level_idx: usize,
    num_sphere_points: usize,
) -> Vec<DMatrixEntry> {
    let k = physics.wave_number;
    let mut d_entries = Vec::new();

    for (i, cluster_i) in clusters.iter().enumerate() {
        for &j in &cluster_i.far_clusters {
            let cluster_j = &clusters[j];

            // Distance vector between cluster centers
            let diff: Vec<f64> = (0..3)
                .map(|d| cluster_i.center[d] - cluster_j.center[d])
                .collect();
            let r = (diff[0] * diff[0] + diff[1] * diff[1] + diff[2] * diff[2]).sqrt();

            if r < 1e-15 {
                continue; // Skip coincident clusters
            }

            let kr = k * r;

            // Compute translation operator using multipole expansion
            let h_funcs = spherical_hankel_first_kind(n_terms.max(2), kr, 1.0);

            // D-matrix is diagonal: D[p,p] = h_0(kr) * ik
            // Store only the diagonal (all entries are the same in this simplified model)
            let d_value = h_funcs[0] * Complex64::new(0.0, k);
            let diagonal = Array1::from_elem(num_sphere_points, d_value);

            d_entries.push(DMatrixEntry {
                source_cluster: i,
                field_cluster: j,
                level: level_idx,
                diagonal,
            });
        }
    }

    d_entries
}

/// Build S-matrices at a single level
fn build_s_matrices_level(
    elements: &[Element],
    clusters: &[Cluster],
    physics: &PhysicsParams,
    sphere_coords: &[[f64; 3]],
    sphere_weights: &[f64],
    level_idx: usize,
    cluster_levels: &[ClusterLevel],
) -> LevelMatrices {
    let k = physics.wave_number;
    let num_sphere_points = sphere_coords.len();
    let num_clusters = clusters.len();
    let mut matrices = Vec::with_capacity(num_clusters);

    let is_leaf_level = level_idx == cluster_levels.len() - 1;

    for cluster in clusters {
        if is_leaf_level {
            // Leaf level: S-matrix maps local expansion to element DOFs
            let num_elem = cluster.element_indices.len();
            let mut s_matrix = Array2::zeros((num_elem, num_sphere_points));

            for (j, &elem_idx) in cluster.element_indices.iter().enumerate() {
                let elem = &elements[elem_idx];
                if elem.property.is_evaluation() {
                    continue;
                }

                for (p, coord) in sphere_coords.iter().enumerate() {
                    let diff: Vec<f64> =
                        (0..3).map(|d| elem.center[d] - cluster.center[d]).collect();
                    let s_dot_diff: f64 = (0..3).map(|d| coord[d] * diff[d]).sum();

                    let exp_factor = Complex64::new((k * s_dot_diff).cos(), (k * s_dot_diff).sin());

                    s_matrix[[j, p]] = exp_factor * sphere_weights[p];
                }
            }

            matrices.push(s_matrix);
        } else {
            // Non-leaf: S-matrix disaggregates to children
            let num_children = cluster.sons.len();
            let child_num_points = if level_idx + 1 < cluster_levels.len() {
                cluster_levels[level_idx + 1].theta_points
                    * cluster_levels[level_idx + 1].phi_points
            } else {
                num_sphere_points
            };

            let mut s_matrix = Array2::zeros((num_children * child_num_points, num_sphere_points));

            for (child_idx, &child_cluster_idx) in cluster.sons.iter().enumerate() {
                if level_idx + 1 < cluster_levels.len() {
                    let child_cluster = &cluster_levels[level_idx + 1].clusters[child_cluster_idx];

                    for (p, coord) in sphere_coords.iter().enumerate() {
                        let diff: Vec<f64> = (0..3)
                            .map(|d| child_cluster.center[d] - cluster.center[d])
                            .collect();
                        let s_dot_diff: f64 = (0..3).map(|d| coord[d] * diff[d]).sum();

                        let exp_factor =
                            Complex64::new((k * s_dot_diff).cos(), (k * s_dot_diff).sin());

                        for cp in 0..child_num_points {
                            s_matrix[[child_idx * child_num_points + cp, p]] =
                                exp_factor * sphere_weights[p] / child_num_points as f64;
                        }
                    }
                }
            }

            matrices.push(s_matrix);
        }
    }

    LevelMatrices {
        level: level_idx,
        num_clusters,
        matrices,
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

/// Estimate the optimal number of levels for the cluster tree
///
/// Based on NC_EstimateNumLevelsMLFMM from the C++ code.
pub fn estimate_num_levels(
    num_elements: usize,
    elements_per_leaf: usize,
    min_levels: usize,
    max_levels: usize,
) -> usize {
    if num_elements == 0 {
        return min_levels;
    }

    // Estimate: each level divides by ~8 (octree)
    let mut levels = 1;
    let mut n = num_elements;

    while n > elements_per_leaf && levels < max_levels {
        n /= 8;
        levels += 1;
    }

    levels.max(min_levels).min(max_levels)
}

/// Build cluster tree from elements
///
/// Creates a hierarchical octree-like structure.
pub fn build_cluster_tree(
    elements: &[Element],
    target_elements_per_leaf: usize,
    physics: &PhysicsParams,
) -> Vec<ClusterLevel> {
    let num_elements = elements.len();
    let num_levels = estimate_num_levels(num_elements, target_elements_per_leaf, 1, 8);

    let mut levels = Vec::with_capacity(num_levels);

    // Compute bounding box
    let (min_coords, max_coords) = compute_bounding_box(elements);
    let root_center = [
        (min_coords[0] + max_coords[0]) / 2.0,
        (min_coords[1] + max_coords[1]) / 2.0,
        (min_coords[2] + max_coords[2]) / 2.0,
    ];
    let root_radius = ((max_coords[0] - min_coords[0]).powi(2)
        + (max_coords[1] - min_coords[1]).powi(2)
        + (max_coords[2] - min_coords[2]).powi(2))
    .sqrt()
        / 2.0;

    // Create root cluster
    let root_cluster = Cluster::new(Array1::from_vec(root_center.to_vec()));
    let all_indices: Vec<usize> = (0..num_elements).collect();

    // Build tree recursively
    let mut root_level = ClusterLevel::new(1);
    let mut root = root_cluster;
    root.element_indices = all_indices;
    root.radius = root_radius;
    root.level = 0;

    // Set expansion parameters based on wave number
    let kr = physics.wave_number * root_radius;
    root_level.expansion_terms = ((kr + 6.0 * kr.ln().max(1.0)) as usize).clamp(4, 30);
    root_level.theta_points = root_level.expansion_terms;
    root_level.phi_points = 2 * root_level.expansion_terms;

    root_level.clusters.push(root);
    root_level.num_original = 1;
    root_level.max_radius = root_radius;
    root_level.avg_radius = root_radius;
    root_level.min_radius = root_radius;

    levels.push(root_level);

    // Subdivide recursively if needed
    if num_levels > 1 {
        subdivide_level(&mut levels, elements, 0, target_elements_per_leaf, physics);
    }

    // Determine near/far interactions at each level
    for level in &mut levels {
        compute_near_far_lists(&mut level.clusters, physics.wave_number);
    }

    levels
}

/// Compute bounding box of all elements
fn compute_bounding_box(elements: &[Element]) -> ([f64; 3], [f64; 3]) {
    let mut min_coords = [f64::MAX, f64::MAX, f64::MAX];
    let mut max_coords = [f64::MIN, f64::MIN, f64::MIN];

    for elem in elements {
        for d in 0..3 {
            min_coords[d] = min_coords[d].min(elem.center[d]);
            max_coords[d] = max_coords[d].max(elem.center[d]);
        }
    }

    (min_coords, max_coords)
}

/// Recursively subdivide clusters
fn subdivide_level(
    levels: &mut Vec<ClusterLevel>,
    elements: &[Element],
    parent_level: usize,
    target_elements_per_leaf: usize,
    physics: &PhysicsParams,
) {
    let parent_clusters = levels[parent_level].clusters.clone();
    let mut child_level = ClusterLevel::new(parent_clusters.len() * 8);

    let mut max_radius = 0.0_f64;
    let mut min_radius = f64::MAX;
    let mut sum_radius = 0.0_f64;

    for (parent_idx, parent) in parent_clusters.iter().enumerate() {
        if parent.element_indices.len() <= target_elements_per_leaf {
            // This cluster becomes a leaf - copy to child level
            let mut leaf = parent.clone();
            leaf.level = parent_level + 1;
            leaf.father = Some(parent_idx);

            max_radius = max_radius.max(leaf.radius);
            min_radius = min_radius.min(leaf.radius);
            sum_radius += leaf.radius;

            let child_idx = child_level.clusters.len();
            levels[parent_level].clusters[parent_idx]
                .sons
                .push(child_idx);
            child_level.clusters.push(leaf);
            continue;
        }

        // Subdivide into 8 octants
        let half_size = parent.radius / 2.0;
        let offsets = [
            [-1.0, -1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, 1.0, 1.0],
            [1.0, -1.0, -1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, -1.0],
            [1.0, 1.0, 1.0],
        ];

        for offset in &offsets {
            let child_center = [
                parent.center[0] + offset[0] * half_size * 0.5,
                parent.center[1] + offset[1] * half_size * 0.5,
                parent.center[2] + offset[2] * half_size * 0.5,
            ];

            // Find elements in this octant
            let child_elements: Vec<usize> = parent
                .element_indices
                .iter()
                .filter(|&&idx| {
                    let elem = &elements[idx];
                    let dx = elem.center[0] - parent.center[0];
                    let dy = elem.center[1] - parent.center[1];
                    let dz = elem.center[2] - parent.center[2];

                    (dx * offset[0] >= 0.0) && (dy * offset[1] >= 0.0) && (dz * offset[2] >= 0.0)
                })
                .copied()
                .collect();

            if child_elements.is_empty() {
                continue;
            }

            let mut child = Cluster::new(Array1::from_vec(child_center.to_vec()));
            child.element_indices = child_elements;
            child.radius = half_size;
            child.level = parent_level + 1;
            child.father = Some(parent_idx);

            max_radius = max_radius.max(child.radius);
            min_radius = min_radius.min(child.radius);
            sum_radius += child.radius;

            let child_idx = child_level.clusters.len();
            levels[parent_level].clusters[parent_idx]
                .sons
                .push(child_idx);
            child_level.clusters.push(child);
        }
    }

    let num_clusters = child_level.clusters.len();
    child_level.num_original = num_clusters;
    child_level.max_radius = max_radius;
    child_level.min_radius = min_radius;
    child_level.avg_radius = if num_clusters > 0 {
        sum_radius / num_clusters as f64
    } else {
        0.0
    };

    // Set expansion parameters
    let kr = physics.wave_number * child_level.avg_radius;
    child_level.expansion_terms = ((kr + 6.0 * kr.ln().max(1.0)) as usize).clamp(4, 30);
    child_level.theta_points = child_level.expansion_terms;
    child_level.phi_points = 2 * child_level.expansion_terms;

    levels.push(child_level);

    // Continue subdividing if needed
    let current_level = levels.len() - 1;
    let should_continue = levels[current_level]
        .clusters
        .iter()
        .any(|c| c.element_indices.len() > target_elements_per_leaf);

    if should_continue && current_level < 7 {
        subdivide_level(
            levels,
            elements,
            current_level,
            target_elements_per_leaf,
            physics,
        );
    }
}

/// Compute near/far cluster lists based on separation criterion
fn compute_near_far_lists(clusters: &mut [Cluster], wave_number: f64) {
    let n = clusters.len();
    let min_separation = 2.0; // Clusters farther than 2*radius are "far"

    for i in 0..n {
        let center_i = clusters[i].center.clone();
        let radius_i = clusters[i].radius;

        let mut near = Vec::new();
        let mut far = Vec::new();

        for j in 0..n {
            if i == j {
                continue;
            }

            let center_j = &clusters[j].center;
            let radius_j = clusters[j].radius;

            let dist = ((center_i[0] - center_j[0]).powi(2)
                + (center_i[1] - center_j[1]).powi(2)
                + (center_i[2] - center_j[2]).powi(2))
            .sqrt();

            let separation = dist / (radius_i + radius_j).max(1e-15);

            // Also check if clusters are in the far-field acoustically
            let kr = wave_number * dist;
            let is_acoustic_far = kr > 2.0;

            if separation > min_separation && is_acoustic_far {
                far.push(j);
            } else {
                near.push(j);
            }
        }

        clusters[i].near_clusters = near;
        clusters[i].far_clusters = far;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{BoundaryCondition, ElementProperty, ElementType};
    use ndarray::array;

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
    fn test_estimate_num_levels() {
        assert_eq!(estimate_num_levels(10, 10, 1, 8), 1);
        // 100 elements, ~12 per octant after first split -> still need to subdivide
        assert_eq!(estimate_num_levels(100, 10, 1, 8), 3);
        // 1000 elements -> need multiple levels
        assert!(estimate_num_levels(1000, 10, 1, 8) >= 3);
    }

    #[test]
    fn test_build_cluster_tree() {
        let (elements, _nodes) = make_test_elements();
        let physics = PhysicsParams::new(100.0, 343.0, 1.21, false);

        let tree = build_cluster_tree(&elements, 10, &physics);

        assert!(!tree.is_empty());
        assert!(!tree[0].clusters.is_empty());
        assert_eq!(tree[0].clusters[0].element_indices.len(), elements.len());
    }

    #[test]
    fn test_build_mlfmm_system() {
        let (elements, nodes) = make_test_elements();
        let physics = PhysicsParams::new(100.0, 343.0, 1.21, false);

        let cluster_tree = build_cluster_tree(&elements, 10, &physics);
        let system = build_mlfmm_system(&elements, &nodes, cluster_tree, &physics);

        assert_eq!(system.num_dofs, 2);
        assert!(system.num_levels >= 1);
    }

    #[test]
    fn test_mlfmm_matvec() {
        let (elements, nodes) = make_test_elements();
        let physics = PhysicsParams::new(100.0, 343.0, 1.21, false);

        let cluster_tree = build_cluster_tree(&elements, 10, &physics);
        let system = build_mlfmm_system(&elements, &nodes, cluster_tree, &physics);

        let x = Array1::from_vec(vec![Complex64::new(1.0, 0.0), Complex64::new(0.0, 1.0)]);
        let y = system.matvec(&x);

        assert_eq!(y.len(), 2);
    }
}
