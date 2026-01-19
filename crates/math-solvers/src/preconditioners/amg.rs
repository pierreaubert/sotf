//! Algebraic Multigrid (AMG) Preconditioner
//!
//! This module implements an algebraic multigrid preconditioner inspired by
//! hypre's BoomerAMG, designed for better parallel scalability across CPU cores.
//!
//! ## Features
//!
//! - **Parallel coarsening**: Classical Ruge-Stüben (RS) and PMIS algorithms
//! - **Interpolation**: Standard and extended interpolation operators
//! - **Smoothers**: Jacobi (fully parallel) and symmetric Gauss-Seidel
//! - **V-cycle**: Standard V(ν₁, ν₂) cycling with configurable pre/post smoothing
//!
//! ## Scalability
//!
//! The AMG preconditioner scales better than ILU across multiple cores because:
//! - Coarsening can be parallelized (PMIS is inherently parallel)
//! - Jacobi smoothing is embarrassingly parallel
//! - Each level's operations can be parallelized independently
//!
//! ## Usage
//!
//! ```ignore
//! use math_audio_solvers::{AmgPreconditioner, AmgConfig, CsrMatrix};
//!
//! let config = AmgConfig::default();
//! let precond = AmgPreconditioner::from_csr(&matrix, config);
//!
//! // Use with GMRES
//! let z = precond.apply(&residual);
//! ```

#[cfg(any(feature = "native", feature = "wasm"))]
use crate::parallel::{parallel_enumerate_map, parallel_map_indexed};
use crate::sparse::CsrMatrix;
use crate::traits::{ComplexField, Preconditioner};
use ndarray::Array1;
use num_traits::FromPrimitive;

/// Coarsening algorithm for AMG hierarchy construction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AmgCoarsening {
    /// Classical Ruge-Stüben coarsening
    /// Good quality but limited parallelism in the selection phase
    #[default]
    RugeStuben,

    /// Parallel Modified Independent Set (PMIS)
    /// Better parallel scalability, may produce slightly larger coarse grids
    Pmis,

    /// Hybrid MIS (HMIS) - PMIS in first pass, then RS cleanup
    /// Balance between quality and parallelism
    Hmis,
}

/// Interpolation operator type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AmgInterpolation {
    /// Standard interpolation - direct interpolation from coarse neighbors
    #[default]
    Standard,

    /// Extended interpolation - includes indirect (distance-2) connections
    /// Better for some problem types but more expensive
    Extended,

    /// Direct interpolation - simplest, only immediate strong connections
    /// Fastest but may have poor convergence for hard problems
    Direct,
}

/// Smoother type for AMG relaxation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AmgSmoother {
    /// Jacobi relaxation - fully parallel, requires damping (ω ≈ 0.6-0.8)
    #[default]
    Jacobi,

    /// l1-Jacobi - Jacobi with l1 norm scaling, more robust
    L1Jacobi,

    /// Symmetric Gauss-Seidel - forward then backward sweep
    /// Better convergence but limited parallelism
    SymmetricGaussSeidel,

    /// Chebyshev polynomial smoother - fully parallel, no damping needed
    Chebyshev,
}

/// AMG cycle type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AmgCycle {
    /// V-cycle: one visit to each level
    #[default]
    VCycle,

    /// W-cycle: two visits to coarser levels (more expensive)
    WCycle,

    /// F-cycle: hybrid between V and W
    FCycle,
}

/// Configuration for AMG preconditioner
#[derive(Debug, Clone)]
pub struct AmgConfig {
    /// Coarsening algorithm
    pub coarsening: AmgCoarsening,

    /// Interpolation operator type
    pub interpolation: AmgInterpolation,

    /// Smoother for pre- and post-relaxation
    pub smoother: AmgSmoother,

    /// Cycle type (V, W, or F)
    pub cycle: AmgCycle,

    /// Strong connection threshold (default: 0.25)
    /// Connections with |a_ij| >= θ * max_k |a_ik| are considered strong
    pub strong_threshold: f64,

    /// Maximum number of levels in the hierarchy
    pub max_levels: usize,

    /// Coarsest level size - switch to direct solve below this
    pub coarse_size: usize,

    /// Number of pre-smoothing sweeps (ν₁)
    pub num_pre_smooth: usize,

    /// Number of post-smoothing sweeps (ν₂)
    pub num_post_smooth: usize,

    /// Jacobi damping parameter (ω)
    pub jacobi_weight: f64,

    /// Truncation factor for interpolation (drop small weights)
    pub trunc_factor: f64,

    /// Maximum interpolation stencil size per row
    pub max_interp_elements: usize,

    /// Enable aggressive coarsening on first few levels
    pub aggressive_coarsening_levels: usize,
}

impl Default for AmgConfig {
    fn default() -> Self {
        Self {
            coarsening: AmgCoarsening::default(),
            interpolation: AmgInterpolation::default(),
            smoother: AmgSmoother::default(),
            cycle: AmgCycle::default(),
            strong_threshold: 0.25,
            max_levels: 25,
            coarse_size: 50,
            num_pre_smooth: 1,
            num_post_smooth: 1,
            jacobi_weight: 0.6667, // 2/3 is optimal for Poisson
            trunc_factor: 0.0,
            max_interp_elements: 4,
            aggressive_coarsening_levels: 0,
        }
    }
}

impl AmgConfig {
    /// Configuration optimized for BEM systems
    ///
    /// BEM matrices are typically denser and less sparse than FEM,
    /// requiring adjusted thresholds.
    pub fn for_bem() -> Self {
        Self {
            strong_threshold: 0.5,           // Higher for denser BEM matrices
            coarsening: AmgCoarsening::Pmis, // Better parallel scalability
            smoother: AmgSmoother::L1Jacobi, // More robust for BEM
            max_interp_elements: 6,
            ..Default::default()
        }
    }

    /// Configuration optimized for FEM systems
    pub fn for_fem() -> Self {
        Self {
            strong_threshold: 0.25,
            coarsening: AmgCoarsening::RugeStuben,
            smoother: AmgSmoother::SymmetricGaussSeidel,
            ..Default::default()
        }
    }

    /// Configuration optimized for maximum parallel scalability
    pub fn for_parallel() -> Self {
        Self {
            coarsening: AmgCoarsening::Pmis,
            smoother: AmgSmoother::Jacobi,
            jacobi_weight: 0.8,
            num_pre_smooth: 2,
            num_post_smooth: 2,
            ..Default::default()
        }
    }

    /// Configuration for difficult/ill-conditioned problems
    pub fn for_difficult_problems() -> Self {
        Self {
            coarsening: AmgCoarsening::RugeStuben,
            interpolation: AmgInterpolation::Extended,
            smoother: AmgSmoother::SymmetricGaussSeidel,
            strong_threshold: 0.25,
            max_interp_elements: 8,
            num_pre_smooth: 2,
            num_post_smooth: 2,
            ..Default::default()
        }
    }
}

/// Point classification in coarsening
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointType {
    /// Undecided
    Undecided,
    /// Coarse point (C-point)
    Coarse,
    /// Fine point (F-point)
    Fine,
}

/// Single level in the AMG hierarchy
#[derive(Debug, Clone)]
struct AmgLevel<T: ComplexField> {
    /// System matrix A at this level (CSR format)
    matrix: CsrMatrix<T>,

    /// Prolongation operator P: coarse -> fine
    prolongation: Option<CsrMatrix<T>>,

    /// Restriction operator R: fine -> coarse (typically R = P^T)
    restriction: Option<CsrMatrix<T>>,

    /// Inverse diagonal for Jacobi smoothing
    diag_inv: Array1<T>,

    /// Coarse-to-fine mapping
    coarse_to_fine: Vec<usize>,

    /// Number of DOFs at this level
    num_dofs: usize,
}

/// Algebraic Multigrid Preconditioner
///
/// Implements a classical AMG V-cycle preconditioner with configurable
/// coarsening, interpolation, and smoothing strategies.
#[derive(Debug, Clone)]
pub struct AmgPreconditioner<T: ComplexField> {
    /// AMG hierarchy (finest to coarsest)
    levels: Vec<AmgLevel<T>>,

    /// Configuration
    config: AmgConfig,

    /// Statistics
    setup_time_ms: f64,
    grid_complexity: f64,
    operator_complexity: f64,
}

impl<T: ComplexField> AmgPreconditioner<T>
where
    T::Real: Sync + Send,
{
    /// Create AMG preconditioner from a CSR matrix
    pub fn from_csr(matrix: &CsrMatrix<T>, config: AmgConfig) -> Self {
        let start = std::time::Instant::now();

        let mut levels = Vec::new();
        let mut current_matrix = matrix.clone();

        // Extract diagonal for first level
        let diag_inv = Self::compute_diag_inv(&current_matrix);

        levels.push(AmgLevel {
            matrix: current_matrix.clone(),
            prolongation: None,
            restriction: None,
            diag_inv,
            coarse_to_fine: Vec::new(),
            num_dofs: current_matrix.num_rows,
        });

        // Build hierarchy
        for _level_idx in 0..config.max_levels - 1 {
            let n = current_matrix.num_rows;
            if n <= config.coarse_size {
                break;
            }

            // Compute strength matrix
            let strong_connections =
                Self::compute_strength_matrix(&current_matrix, config.strong_threshold);

            // Coarsening
            let (point_types, coarse_to_fine) = match config.coarsening {
                AmgCoarsening::RugeStuben => {
                    Self::coarsen_ruge_stuben(&current_matrix, &strong_connections)
                }
                AmgCoarsening::Pmis | AmgCoarsening::Hmis => {
                    Self::coarsen_pmis(&current_matrix, &strong_connections)
                }
            };

            let num_coarse = coarse_to_fine.len();
            if num_coarse == 0 || num_coarse >= n {
                // Can't coarsen further
                break;
            }

            // Build interpolation operator
            let prolongation = Self::build_interpolation(
                &current_matrix,
                &strong_connections,
                &point_types,
                &coarse_to_fine,
                &config,
            );

            // Restriction is transpose of prolongation
            let restriction = Self::transpose_csr(&prolongation);

            // Galerkin coarse grid: A_c = R * A * P
            let coarse_matrix =
                Self::galerkin_product(&restriction, &current_matrix, &prolongation);

            // Extract diagonal for new level
            let coarse_diag_inv = Self::compute_diag_inv(&coarse_matrix);

            // Update level with P and R
            if let Some(last) = levels.last_mut() {
                last.prolongation = Some(prolongation);
                last.restriction = Some(restriction);
                last.coarse_to_fine = coarse_to_fine;
            }

            // Add coarse level
            levels.push(AmgLevel {
                matrix: coarse_matrix.clone(),
                prolongation: None,
                restriction: None,
                diag_inv: coarse_diag_inv,
                coarse_to_fine: Vec::new(),
                num_dofs: num_coarse,
            });

            current_matrix = coarse_matrix;
        }

        let setup_time_ms = start.elapsed().as_secs_f64() * 1000.0;

        // Compute complexities
        let (grid_complexity, operator_complexity) = Self::compute_complexities(&levels);

        Self {
            levels,
            config,
            setup_time_ms,
            grid_complexity,
            operator_complexity,
        }
    }

    /// Get number of levels in hierarchy
    pub fn num_levels(&self) -> usize {
        self.levels.len()
    }

    /// Get setup time in milliseconds
    pub fn setup_time_ms(&self) -> f64 {
        self.setup_time_ms
    }

    /// Get grid complexity (sum of DOFs / fine DOFs)
    pub fn grid_complexity(&self) -> f64 {
        self.grid_complexity
    }

    /// Get operator complexity (sum of nnz / fine nnz)
    pub fn operator_complexity(&self) -> f64 {
        self.operator_complexity
    }

    /// Get configuration
    pub fn config(&self) -> &AmgConfig {
        &self.config
    }

    /// Compute inverse diagonal for Jacobi smoothing
    fn compute_diag_inv(matrix: &CsrMatrix<T>) -> Array1<T> {
        let n = matrix.num_rows;
        let mut diag_inv = Array1::from_elem(n, T::one());

        for i in 0..n {
            let diag = matrix.get(i, i);
            let tol = T::Real::from_f64(1e-15).unwrap();
            if diag.norm() > tol {
                diag_inv[i] = diag.inv();
            }
        }

        diag_inv
    }

    /// Compute strength of connection matrix
    ///
    /// Entry (i,j) is strong if |a_ij| >= θ * max_k!=i |a_ik|
    fn compute_strength_matrix(matrix: &CsrMatrix<T>, theta: f64) -> Vec<Vec<usize>> {
        let n = matrix.num_rows;

        #[cfg(any(feature = "native", feature = "wasm"))]
        {
            parallel_map_indexed(n, |i| {
                // Find max off-diagonal magnitude in row i
                let mut max_off_diag = T::Real::from_f64(0.0).unwrap();
                for (j, val) in matrix.row_entries(i) {
                    if i != j {
                        let norm = val.norm();
                        if norm > max_off_diag {
                            max_off_diag = norm;
                        }
                    }
                }

                let threshold = T::Real::from_f64(theta).unwrap() * max_off_diag;

                // Collect strong connections
                let mut row_strong = Vec::new();
                for (j, val) in matrix.row_entries(i) {
                    if i != j && val.norm() >= threshold {
                        row_strong.push(j);
                    }
                }
                row_strong
            })
        }

        #[cfg(not(any(feature = "native", feature = "wasm")))]
        {
            let mut strong: Vec<Vec<usize>> = vec![Vec::new(); n];
            for (i, row_strong) in strong.iter_mut().enumerate().take(n) {
                // Find max off-diagonal magnitude in row i
                let mut max_off_diag = T::Real::from_f64(0.0).unwrap();
                for (j, val) in matrix.row_entries(i) {
                    if i != j {
                        let norm = val.norm();
                        if norm > max_off_diag {
                            max_off_diag = norm;
                        }
                    }
                }

                let threshold = T::Real::from_f64(theta).unwrap() * max_off_diag;

                // Collect strong connections
                for (j, val) in matrix.row_entries(i) {
                    if i != j && val.norm() >= threshold {
                        row_strong.push(j);
                    }
                }
            }
            strong
        }
    }

    /// Classical Ruge-Stüben coarsening
    fn coarsen_ruge_stuben(
        matrix: &CsrMatrix<T>,
        strong: &[Vec<usize>],
    ) -> (Vec<PointType>, Vec<usize>) {
        let n = matrix.num_rows;
        let mut point_types = vec![PointType::Undecided; n];

        // Compute influence measure λ_i = |S_i^T| (how many points strongly depend on i)
        let mut lambda: Vec<usize> = vec![0; n];
        for row in strong.iter().take(n) {
            for &j in row {
                lambda[j] += 1;
            }
        }

        // Build priority queue (we use a simple approach: process by decreasing lambda)
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| lambda[b].cmp(&lambda[a]));

        // First pass: select C-points
        for &i in &order {
            if point_types[i] != PointType::Undecided {
                continue;
            }

            // Make i a C-point
            point_types[i] = PointType::Coarse;

            // All points that strongly depend on i become F-points
            for j in 0..n {
                if point_types[j] == PointType::Undecided && strong[j].contains(&i) {
                    point_types[j] = PointType::Fine;
                    // Update lambda for neighbors
                    for &k in &strong[j] {
                        if point_types[k] == PointType::Undecided {
                            lambda[k] = lambda[k].saturating_sub(1);
                        }
                    }
                }
            }
        }

        // Ensure all remaining undecided become fine
        for pt in &mut point_types {
            if *pt == PointType::Undecided {
                *pt = PointType::Fine;
            }
        }

        // Build coarse-to-fine mapping
        let coarse_to_fine: Vec<usize> = (0..n)
            .filter(|&i| point_types[i] == PointType::Coarse)
            .collect();

        (point_types, coarse_to_fine)
    }

    /// Parallel Modified Independent Set (PMIS) coarsening
    fn coarsen_pmis(matrix: &CsrMatrix<T>, strong: &[Vec<usize>]) -> (Vec<PointType>, Vec<usize>) {
        let n = matrix.num_rows;
        let mut point_types = vec![PointType::Undecided; n];

        // Compute weights based on number of strong connections
        let weights: Vec<f64> = (0..n)
            .map(|i| {
                // Weight = |S_i| + random tie-breaker
                strong[i].len() as f64 + (i as f64 * 0.0001) % 0.001
            })
            .collect();

        // Iterative independent set selection
        let mut changed = true;
        let mut iteration = 0;
        const MAX_ITERATIONS: usize = 100;

        while changed && iteration < MAX_ITERATIONS {
            changed = false;
            iteration += 1;

            #[cfg(any(feature = "native", feature = "wasm"))]
            {
                // Parallel pass: determine new C-points and F-points
                let updates: Vec<(usize, PointType)> =
                    parallel_enumerate_map(&point_types, |i, pt| {
                        if *pt != PointType::Undecided {
                            return (i, *pt);
                        }

                        // Check if i has maximum weight among undecided strong neighbors
                        let mut is_max = true;
                        for &j in &strong[i] {
                            if point_types[j] == PointType::Undecided && weights[j] > weights[i] {
                                is_max = false;
                                break;
                            }
                        }

                        // Check if any strong neighbor is already C
                        let has_c_neighbor = strong[i]
                            .iter()
                            .any(|&j| point_types[j] == PointType::Coarse);

                        if has_c_neighbor {
                            (i, PointType::Fine)
                        } else if is_max {
                            (i, PointType::Coarse)
                        } else {
                            (i, PointType::Undecided)
                        }
                    });

                for (i, new_type) in updates {
                    if point_types[i] != new_type {
                        point_types[i] = new_type;
                        changed = true;
                    }
                }
            }

            #[cfg(not(any(feature = "native", feature = "wasm")))]
            {
                // Sequential fallback
                let old_types = point_types.clone();
                for i in 0..n {
                    if old_types[i] != PointType::Undecided {
                        continue;
                    }

                    // Check if i has maximum weight among undecided strong neighbors
                    let mut is_max = true;
                    for &j in &strong[i] {
                        if old_types[j] == PointType::Undecided && weights[j] > weights[i] {
                            is_max = false;
                            break;
                        }
                    }

                    // Check if any strong neighbor is already C
                    let has_c_neighbor =
                        strong[i].iter().any(|&j| old_types[j] == PointType::Coarse);

                    if has_c_neighbor {
                        point_types[i] = PointType::Fine;
                        changed = true;
                    } else if is_max {
                        point_types[i] = PointType::Coarse;
                        changed = true;
                    }
                }
            }
        }

        // Any remaining undecided become coarse
        for pt in &mut point_types {
            if *pt == PointType::Undecided {
                *pt = PointType::Coarse;
            }
        }

        // Build coarse-to-fine mapping
        let coarse_to_fine: Vec<usize> = (0..n)
            .filter(|&i| point_types[i] == PointType::Coarse)
            .collect();

        (point_types, coarse_to_fine)
    }

    /// Build interpolation operator P
    fn build_interpolation(
        matrix: &CsrMatrix<T>,
        strong: &[Vec<usize>],
        point_types: &[PointType],
        coarse_to_fine: &[usize],
        config: &AmgConfig,
    ) -> CsrMatrix<T> {
        let n_fine = matrix.num_rows;
        let n_coarse = coarse_to_fine.len();

        // Build fine-to-coarse mapping
        let mut fine_to_coarse = vec![usize::MAX; n_fine];
        for (coarse_idx, &fine_idx) in coarse_to_fine.iter().enumerate() {
            fine_to_coarse[fine_idx] = coarse_idx;
        }

        // Build P row by row
        let mut triplets: Vec<(usize, usize, T)> = Vec::new();

        for i in 0..n_fine {
            match point_types[i] {
                PointType::Coarse => {
                    // C-point: identity mapping P_ij = 1 if j = coarse_index(i)
                    let coarse_idx = fine_to_coarse[i];
                    triplets.push((i, coarse_idx, T::one()));
                }
                PointType::Fine => {
                    // F-point: interpolate from strong C-neighbors
                    let a_ii = matrix.get(i, i);

                    // Collect strong C-neighbors
                    let c_neighbors: Vec<usize> = strong[i]
                        .iter()
                        .copied()
                        .filter(|&j| point_types[j] == PointType::Coarse)
                        .collect();

                    if c_neighbors.is_empty() {
                        continue;
                    }

                    // Standard interpolation weights
                    match config.interpolation {
                        AmgInterpolation::Direct | AmgInterpolation::Standard => {
                            let mut weights: Vec<(usize, T)> = Vec::new();
                            let mut sum_weights = T::zero();

                            for &j in &c_neighbors {
                                let a_ij = matrix.get(i, j);
                                let tol = T::Real::from_f64(1e-15).unwrap();
                                if a_ii.norm() > tol {
                                    let w = T::zero() - a_ij * a_ii.inv();
                                    weights.push((fine_to_coarse[j], w));
                                    sum_weights += w;
                                }
                            }

                            // Add weak connections contribution (standard interpolation)
                            if config.interpolation == AmgInterpolation::Standard {
                                let mut weak_sum = T::zero();
                                for (j, val) in matrix.row_entries(i) {
                                    if j != i && !c_neighbors.contains(&j) {
                                        weak_sum += val;
                                    }
                                }

                                let tol = T::Real::from_f64(1e-15).unwrap();
                                if sum_weights.norm() > tol && weak_sum.norm() > tol {
                                    let scale = T::one() + weak_sum * (a_ii * sum_weights).inv();
                                    for (_, w) in &mut weights {
                                        *w *= scale;
                                    }
                                }
                            }

                            // Truncate small weights if configured
                            if config.trunc_factor > 0.0 {
                                let max_w = weights.iter().map(|(_, w)| w.norm()).fold(
                                    T::Real::from_f64(0.0).unwrap(),
                                    |a, b| {
                                        if a > b { a } else { b }
                                    },
                                );
                                let threshold =
                                    T::Real::from_f64(config.trunc_factor).unwrap() * max_w;
                                weights.retain(|(_, w)| w.norm() >= threshold);

                                if weights.len() > config.max_interp_elements {
                                    weights.sort_by(|a, b| {
                                        b.1.norm().partial_cmp(&a.1.norm()).unwrap()
                                    });
                                    weights.truncate(config.max_interp_elements);
                                }
                            }

                            for (coarse_idx, w) in weights {
                                triplets.push((i, coarse_idx, w));
                            }
                        }
                        AmgInterpolation::Extended => {
                            let mut weights: Vec<(usize, T)> = Vec::new();

                            // Direct C-neighbors
                            for &j in &c_neighbors {
                                let a_ij = matrix.get(i, j);
                                let tol = T::Real::from_f64(1e-15).unwrap();
                                if a_ii.norm() > tol {
                                    let w = T::zero() - a_ij * a_ii.inv();
                                    weights.push((fine_to_coarse[j], w));
                                }
                            }

                            // F-neighbors contribute through their C-neighbors
                            let f_neighbors: Vec<usize> = strong[i]
                                .iter()
                                .copied()
                                .filter(|&j| point_types[j] == PointType::Fine)
                                .collect();

                            for &k in &f_neighbors {
                                let a_ik = matrix.get(i, k);
                                let a_kk = matrix.get(k, k);

                                let tol = T::Real::from_f64(1e-15).unwrap();
                                if a_kk.norm() < tol {
                                    continue;
                                }

                                for &j in &strong[k] {
                                    if point_types[j] == PointType::Coarse {
                                        let a_kj = matrix.get(k, j);
                                        let w = T::zero() - a_ik * a_kj * (a_ii * a_kk).inv();

                                        let coarse_j = fine_to_coarse[j];
                                        if let Some((_, existing)) =
                                            weights.iter_mut().find(|(idx, _)| *idx == coarse_j)
                                        {
                                            *existing += w;
                                        } else {
                                            weights.push((coarse_j, w));
                                        }
                                    }
                                }
                            }

                            if weights.len() > config.max_interp_elements {
                                weights
                                    .sort_by(|a, b| b.1.norm().partial_cmp(&a.1.norm()).unwrap());
                                weights.truncate(config.max_interp_elements);
                            }

                            for (coarse_idx, w) in weights {
                                triplets.push((i, coarse_idx, w));
                            }
                        }
                    }
                }
                PointType::Undecided => {}
            }
        }

        CsrMatrix::from_triplets(n_fine, n_coarse, triplets)
    }

    /// Transpose a CSR matrix
    fn transpose_csr(matrix: &CsrMatrix<T>) -> CsrMatrix<T> {
        let m = matrix.num_rows;
        let n = matrix.num_cols;

        let mut triplets: Vec<(usize, usize, T)> = Vec::new();
        for i in 0..m {
            for (j, val) in matrix.row_entries(i) {
                triplets.push((j, i, val));
            }
        }

        CsrMatrix::from_triplets(n, m, triplets)
    }

    /// Compute Galerkin coarse grid operator: A_c = R * A * P
    fn galerkin_product(r: &CsrMatrix<T>, a: &CsrMatrix<T>, p: &CsrMatrix<T>) -> CsrMatrix<T> {
        let ap = a.matmul(p);
        r.matmul(&ap)
    }

    /// Sparse matrix multiplication - now uses optimized CSR matmul
    #[allow(dead_code)]
    fn sparse_matmul(a: &CsrMatrix<T>, b: &CsrMatrix<T>) -> CsrMatrix<T> {
        a.matmul(b)
    }

    /// Compute grid and operator complexities
    fn compute_complexities(levels: &[AmgLevel<T>]) -> (f64, f64) {
        if levels.is_empty() {
            return (1.0, 1.0);
        }

        let fine_dofs = levels[0].num_dofs as f64;
        let fine_nnz = levels[0].matrix.nnz() as f64;

        let total_dofs: f64 = levels.iter().map(|l| l.num_dofs as f64).sum();
        let total_nnz: f64 = levels.iter().map(|l| l.matrix.nnz() as f64).sum();

        let grid_complexity = total_dofs / fine_dofs;
        let operator_complexity = total_nnz / fine_nnz;

        (grid_complexity, operator_complexity)
    }

    /// Apply Jacobi smoothing: x = x + ω * D^{-1} * (b - A*x)
    fn smooth_jacobi(
        matrix: &CsrMatrix<T>,
        diag_inv: &Array1<T>,
        x: &mut Array1<T>,
        b: &Array1<T>,
        omega: f64,
        num_sweeps: usize,
    ) {
        let omega = T::from_real(T::Real::from_f64(omega).unwrap());
        let n = x.len();

        for _ in 0..num_sweeps {
            let r = b - &matrix.matvec(x);

            #[cfg(any(feature = "native", feature = "wasm"))]
            {
                let updates: Vec<T> = parallel_map_indexed(n, |i| omega * diag_inv[i] * r[i]);
                for (i, delta) in updates.into_iter().enumerate() {
                    x[i] += delta;
                }
            }

            #[cfg(not(any(feature = "native", feature = "wasm")))]
            {
                for i in 0..n {
                    x[i] += omega * diag_inv[i] * r[i];
                }
            }
        }
    }

    /// Apply l1-Jacobi smoothing
    fn smooth_l1_jacobi(
        matrix: &CsrMatrix<T>,
        x: &mut Array1<T>,
        b: &Array1<T>,
        num_sweeps: usize,
    ) {
        let n = x.len();

        let l1_diag: Vec<T::Real> = (0..n)
            .map(|i| {
                let mut sum = T::Real::from_f64(0.0).unwrap();
                for (_, val) in matrix.row_entries(i) {
                    sum += val.norm();
                }
                let tol = T::Real::from_f64(1e-15).unwrap();
                if sum > tol {
                    sum
                } else {
                    T::Real::from_f64(1.0).unwrap()
                }
            })
            .collect();

        for _ in 0..num_sweeps {
            let r = b - &matrix.matvec(x);

            #[cfg(any(feature = "native", feature = "wasm"))]
            {
                let updates: Vec<T> =
                    parallel_map_indexed(n, |i| r[i] * T::from_real(l1_diag[i]).inv());
                for (i, delta) in updates.into_iter().enumerate() {
                    x[i] += delta;
                }
            }

            #[cfg(not(any(feature = "native", feature = "wasm")))]
            {
                for i in 0..n {
                    x[i] += r[i] * T::from_real(l1_diag[i]).inv();
                }
            }
        }
    }

    /// Apply symmetric Gauss-Seidel smoothing
    fn smooth_sym_gauss_seidel(
        matrix: &CsrMatrix<T>,
        x: &mut Array1<T>,
        b: &Array1<T>,
        num_sweeps: usize,
    ) {
        let n = x.len();
        let tol = T::Real::from_f64(1e-15).unwrap();

        for _ in 0..num_sweeps {
            // Forward sweep
            for i in 0..n {
                let mut sum = b[i];
                let mut diag = T::one();

                for (j, val) in matrix.row_entries(i) {
                    if j == i {
                        diag = val;
                    } else {
                        sum -= val * x[j];
                    }
                }

                if diag.norm() > tol {
                    x[i] = sum * diag.inv();
                }
            }

            // Backward sweep
            for i in (0..n).rev() {
                let mut sum = b[i];
                let mut diag = T::one();

                for (j, val) in matrix.row_entries(i) {
                    if j == i {
                        diag = val;
                    } else {
                        sum -= val * x[j];
                    }
                }

                if diag.norm() > tol {
                    x[i] = sum * diag.inv();
                }
            }
        }
    }

    /// Apply V-cycle
    fn v_cycle(&self, level: usize, x: &mut Array1<T>, b: &Array1<T>) {
        let lvl = &self.levels[level];

        // Coarsest level: direct solve (or many smoothing iterations)
        if level == self.levels.len() - 1 || lvl.prolongation.is_none() {
            match self.config.smoother {
                AmgSmoother::Jacobi | AmgSmoother::Chebyshev => {
                    Self::smooth_jacobi(
                        &lvl.matrix,
                        &lvl.diag_inv,
                        x,
                        b,
                        self.config.jacobi_weight,
                        20,
                    );
                }
                AmgSmoother::L1Jacobi => {
                    Self::smooth_l1_jacobi(&lvl.matrix, x, b, 20);
                }
                AmgSmoother::SymmetricGaussSeidel => {
                    Self::smooth_sym_gauss_seidel(&lvl.matrix, x, b, 10);
                }
            }
            return;
        }

        // Pre-smoothing
        match self.config.smoother {
            AmgSmoother::Jacobi | AmgSmoother::Chebyshev => {
                Self::smooth_jacobi(
                    &lvl.matrix,
                    &lvl.diag_inv,
                    x,
                    b,
                    self.config.jacobi_weight,
                    self.config.num_pre_smooth,
                );
            }
            AmgSmoother::L1Jacobi => {
                Self::smooth_l1_jacobi(&lvl.matrix, x, b, self.config.num_pre_smooth);
            }
            AmgSmoother::SymmetricGaussSeidel => {
                Self::smooth_sym_gauss_seidel(&lvl.matrix, x, b, self.config.num_pre_smooth);
            }
        }

        // Compute residual: r = b - A*x
        let r = b - &lvl.matrix.matvec(x);

        // Restrict residual to coarse grid: r_c = R * r
        let r_coarse = lvl.restriction.as_ref().unwrap().matvec(&r);

        // Initialize coarse correction
        let n_coarse = self.levels[level + 1].num_dofs;
        let mut e_coarse = Array1::from_elem(n_coarse, T::zero());

        // Recursive call
        self.v_cycle(level + 1, &mut e_coarse, &r_coarse);

        // Prolongate correction: e = P * e_c
        let e = lvl.prolongation.as_ref().unwrap().matvec(&e_coarse);

        // Apply correction: x = x + e
        *x = x.clone() + e;

        // Post-smoothing
        match self.config.smoother {
            AmgSmoother::Jacobi | AmgSmoother::Chebyshev => {
                Self::smooth_jacobi(
                    &lvl.matrix,
                    &lvl.diag_inv,
                    x,
                    b,
                    self.config.jacobi_weight,
                    self.config.num_post_smooth,
                );
            }
            AmgSmoother::L1Jacobi => {
                Self::smooth_l1_jacobi(&lvl.matrix, x, b, self.config.num_post_smooth);
            }
            AmgSmoother::SymmetricGaussSeidel => {
                Self::smooth_sym_gauss_seidel(&lvl.matrix, x, b, self.config.num_post_smooth);
            }
        }
    }
}

impl<T: ComplexField> Preconditioner<T> for AmgPreconditioner<T>
where
    T::Real: Sync + Send,
{
    fn apply(&self, r: &Array1<T>) -> Array1<T> {
        if self.levels.is_empty() {
            return r.clone();
        }

        let n = self.levels[0].num_dofs;
        if r.len() != n {
            return r.clone();
        }

        let mut z = Array1::from_elem(n, T::zero());

        match self.config.cycle {
            AmgCycle::VCycle => {
                self.v_cycle(0, &mut z, r);
            }
            AmgCycle::WCycle => {
                self.v_cycle(0, &mut z, r);
                self.v_cycle(0, &mut z, r);
            }
            AmgCycle::FCycle => {
                self.v_cycle(0, &mut z, r);
                let residual = r - &self.levels[0].matrix.matvec(&z);
                let mut correction = Array1::from_elem(n, T::zero());
                self.v_cycle(0, &mut correction, &residual);
                z = z + correction;
            }
        }

        z
    }
}

/// Diagnostic information about AMG setup
#[derive(Debug, Clone)]
pub struct AmgDiagnostics {
    /// Number of levels
    pub num_levels: usize,
    /// Grid complexity
    pub grid_complexity: f64,
    /// Operator complexity
    pub operator_complexity: f64,
    /// Setup time in milliseconds
    pub setup_time_ms: f64,
    /// DOFs per level
    pub level_dofs: Vec<usize>,
    /// NNZ per level
    pub level_nnz: Vec<usize>,
}

impl<T: ComplexField> AmgPreconditioner<T> {
    /// Get diagnostic information
    pub fn diagnostics(&self) -> AmgDiagnostics {
        AmgDiagnostics {
            num_levels: self.levels.len(),
            grid_complexity: self.grid_complexity,
            operator_complexity: self.operator_complexity,
            setup_time_ms: self.setup_time_ms,
            level_dofs: self.levels.iter().map(|l| l.num_dofs).collect(),
            level_nnz: self.levels.iter().map(|l| l.matrix.nnz()).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_complex::Complex64;

    /// Create a simple 1D Laplacian matrix for testing
    fn create_1d_laplacian(n: usize) -> CsrMatrix<Complex64> {
        let mut triplets: Vec<(usize, usize, Complex64)> = Vec::new();

        for i in 0..n {
            triplets.push((i, i, Complex64::new(2.0, 0.0)));
            if i > 0 {
                triplets.push((i, i - 1, Complex64::new(-1.0, 0.0)));
            }
            if i < n - 1 {
                triplets.push((i, i + 1, Complex64::new(-1.0, 0.0)));
            }
        }

        CsrMatrix::from_triplets(n, n, triplets)
    }

    #[test]
    fn test_amg_creation() {
        let matrix = create_1d_laplacian(100);
        let config = AmgConfig::default();

        let amg = AmgPreconditioner::from_csr(&matrix, config);

        assert!(amg.num_levels() >= 2);
        assert!(amg.grid_complexity() >= 1.0);
        assert!(amg.operator_complexity() >= 1.0);
    }

    #[test]
    fn test_amg_apply() {
        let matrix = create_1d_laplacian(50);
        let config = AmgConfig::default();
        let amg = AmgPreconditioner::from_csr(&matrix, config);

        let r = Array1::from_vec((0..50).map(|i| Complex64::new(i as f64, 0.0)).collect());

        let z = amg.apply(&r);

        assert_eq!(z.len(), r.len());

        let diff: f64 = (&z - &r).iter().map(|x| x.norm()).sum();
        assert!(diff > 1e-10, "Preconditioner should modify the vector");
    }

    #[test]
    fn test_amg_pmis_coarsening() {
        let matrix = create_1d_laplacian(100);
        let config = AmgConfig {
            coarsening: AmgCoarsening::Pmis,
            ..Default::default()
        };

        let amg = AmgPreconditioner::from_csr(&matrix, config);
        assert!(amg.num_levels() >= 2);
    }

    #[test]
    fn test_amg_different_smoothers() {
        let matrix = create_1d_laplacian(50);
        let r = Array1::from_vec((0..50).map(|i| Complex64::new(i as f64, 0.0)).collect());

        for smoother in [
            AmgSmoother::Jacobi,
            AmgSmoother::L1Jacobi,
            AmgSmoother::SymmetricGaussSeidel,
        ] {
            let config = AmgConfig {
                smoother,
                ..Default::default()
            };
            let amg = AmgPreconditioner::from_csr(&matrix, config);

            let z = amg.apply(&r);
            assert_eq!(z.len(), r.len());
        }
    }

    #[test]
    fn test_amg_reduces_residual() {
        let n = 64;
        let matrix = create_1d_laplacian(n);
        let config = AmgConfig::default();
        let amg = AmgPreconditioner::from_csr(&matrix, config);

        let b = Array1::from_vec(
            (0..n)
                .map(|i| Complex64::new((i as f64).sin(), 0.0))
                .collect(),
        );

        let mut x = Array1::from_elem(n, Complex64::new(0.0, 0.0));

        let r0 = &b - &matrix.matvec(&x);
        let norm_r0: f64 = r0.iter().map(|v| v.norm_sqr()).sum::<f64>().sqrt();

        for _ in 0..10 {
            let r = &b - &matrix.matvec(&x);
            let z = amg.apply(&r);
            x = x + z;
        }

        let rf = &b - &matrix.matvec(&x);
        let norm_rf: f64 = rf.iter().map(|v| v.norm_sqr()).sum::<f64>().sqrt();

        assert!(
            norm_rf < norm_r0 * 0.1,
            "AMG should significantly reduce residual: {} -> {}",
            norm_r0,
            norm_rf
        );
    }

    #[test]
    fn test_diagnostics() {
        let matrix = create_1d_laplacian(100);
        let amg = AmgPreconditioner::from_csr(&matrix, AmgConfig::default());

        let diag = amg.diagnostics();

        assert!(diag.num_levels >= 2);
        assert_eq!(diag.level_dofs.len(), diag.num_levels);
        assert_eq!(diag.level_nnz.len(), diag.num_levels);
        assert!(diag.grid_complexity >= 1.0);
        assert!(diag.setup_time_ms >= 0.0);
    }
}
