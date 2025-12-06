//! Incomplete LU (ILU) Preconditioner
//!
//! Ported from NumCalc's NC_IncompleteLUDecomposition.
//!
//! This preconditioner is critical for BEM systems which are typically
//! too ill-conditioned for simple Jacobi (diagonal) preconditioning.
//!
//! ## Algorithm
//!
//! 1. **Row scaling**: Scale matrix rows so that `sqrt(nnz_in_row / sum_of_squared_norms) = 1`
//! 2. **Threshold dropping**: Keep entries with `|a_ij| > threshold` or diagonal entries
//! 3. **Build sparse L and U**: L stored by rows (lower triangular), U by columns (upper triangular)
//! 4. **Incomplete factorization**: Compute L and U with fill-in restricted to original sparsity
//! 5. **Forward/backward substitution**: Apply (LU)⁻¹ = U⁻¹ L⁻¹
//!
//! ## Threshold Selection (from NumCalc)
//!
//! Different thresholds for different BEM methods:
//! - TBEM: 0.6 - 1.2 (dense, higher threshold)
//! - SLFMM: 0.01 - 0.9 (sparser, lower threshold)
//! - MLFMM: 0.005 - 0.65 (sparsest, lowest threshold)

// Allow needless_range_loop for matrix algorithm code ported from NumCalc
// These loops follow the original C indexing pattern for correctness verification
#![allow(clippy::needless_range_loop)]

use ndarray::{Array1, Array2};
use num_complex::Complex64;

use super::preconditioner::Preconditioner;

/// ILU Preconditioner method type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IluMethod {
    /// Traditional BEM (dense matrix)
    Tbem,
    /// Single-Level FMM (sparse near-field)
    Slfmm,
    /// Multi-Level FMM (very sparse near-field)
    Mlfmm,
}

/// Scanning degree for ILU threshold selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IluScanningDegree {
    /// Coarsest (highest threshold, fastest)
    Coarse = 0,
    /// Medium
    Medium = 1,
    /// Fine
    Fine = 2,
    /// Finest (lowest threshold, most accurate)
    Finest = 3,
}

/// Get threshold factor based on method and scanning degree
fn get_threshold_factor(method: IluMethod, degree: IluScanningDegree) -> f64 {
    match method {
        IluMethod::Tbem => match degree {
            IluScanningDegree::Coarse => 1.2,
            IluScanningDegree::Medium => 1.0,
            IluScanningDegree::Fine => 0.8,
            IluScanningDegree::Finest => 0.6,
        },
        IluMethod::Slfmm => match degree {
            IluScanningDegree::Coarse => 0.9,
            IluScanningDegree::Medium => 0.35,
            IluScanningDegree::Fine => 0.07,
            IluScanningDegree::Finest => 0.01,
        },
        IluMethod::Mlfmm => match degree {
            IluScanningDegree::Coarse => 0.65,
            IluScanningDegree::Medium => 0.15,
            IluScanningDegree::Fine => 0.05,
            IluScanningDegree::Finest => 0.005,
        },
    }
}

/// Result of ILU setup - includes scaled system for use with CGS
#[derive(Debug, Clone)]
pub struct IluSetup {
    /// The ILU preconditioner
    pub preconditioner: IluPreconditioner,
    /// Row-scaled matrix (A_scaled = D * A where D is row scaling)
    pub scaled_matrix: Array2<Complex64>,
    /// Row scaling factors (to scale RHS: b_scaled = D * b)
    pub row_scale: Array1<Complex64>,
}

/// Incomplete LU (ILU) Preconditioner
///
/// Based on NumCalc's NC_IncompleteLUDecomposition.
/// Uses threshold-based dropping with row scaling.
#[derive(Debug, Clone)]
pub struct IluPreconditioner {
    /// L matrix values (lower triangular, stored by rows)
    l_values: Vec<Complex64>,
    /// Column indices for L entries
    l_col_indices: Vec<usize>,
    /// Row start indices for L (length n+1)
    l_row_ptr: Vec<usize>,

    /// U matrix values (upper triangular, stored by columns)
    u_values: Vec<Complex64>,
    /// Row indices for U entries
    u_row_indices: Vec<usize>,
    /// Column start indices for U (length n+1)
    u_col_ptr: Vec<usize>,

    /// Matrix dimension
    n: usize,
}

impl IluPreconditioner {
    /// Create ILU preconditioner from a dense matrix
    ///
    /// # Arguments
    /// * `a` - The coefficient matrix (will be scaled internally)
    /// * `method` - BEM method type (affects threshold selection)
    /// * `degree` - Scanning degree (affects accuracy vs speed tradeoff)
    pub fn from_matrix(
        a: &Array2<Complex64>,
        method: IluMethod,
        degree: IluScanningDegree,
    ) -> Self {
        let n = a.nrows();
        assert_eq!(n, a.ncols(), "Matrix must be square");

        let threshold = get_threshold_factor(method, degree);

        // Clone matrix for scaling
        let mut scaled = a.clone();
        let mut rhs_scale = vec![Complex64::new(1.0, 0.0); n];

        // Step 1: Row scaling
        // Scale each row so that sqrt(n_cols / sum_of_squared_norms) approaches 1
        for i in 0..n {
            let row_sum_sq: f64 = scaled.row(i).iter().map(|x| x.norm_sqr()).sum();
            if row_sum_sq > 1e-30 {
                let scale = (n as f64 / row_sum_sq).sqrt();
                for j in 0..n {
                    scaled[[i, j]] *= scale;
                }
                rhs_scale[i] = Complex64::new(scale, 0.0);
            }
        }

        // Step 2: Identify entries to keep (above threshold or diagonal)
        // Count "trues" (kept entries)
        let mut keep = vec![vec![false; n]; n];
        let mut n_trues = 0;

        for i in 0..n {
            for j in 0..n {
                if scaled[[i, j]].norm() > threshold || i == j {
                    keep[i][j] = true;
                    n_trues += 1;
                }
            }
        }

        // Step 3: Build column index array (for each row, which columns are kept)
        // jcol_tru[k] = column index of k-th kept entry
        // nrow_tru[i] = start index in jcol_tru for row i
        let mut jcol_tru = vec![0usize; n_trues];
        let mut nrow_tru = vec![0usize; n + 1];

        let mut kl = 0;
        for i in 0..n {
            nrow_tru[i] = kl;
            for j in 0..n {
                if keep[i][j] {
                    jcol_tru[kl] = j;
                    kl += 1;
                }
            }
        }
        nrow_tru[n] = kl;

        // Step 4: Build row index array (for each column, which rows are kept)
        // irow_tru[k] = row index of k-th kept entry (by column)
        // ncol_tru[j] = start index in irow_tru for column j
        let mut irow_tru = vec![0usize; n_trues];
        let mut ncol_tru = vec![0usize; n + 1];

        kl = 0;
        for j in 0..n {
            ncol_tru[j] = kl;
            for i in 0..n {
                // Check if (i, j) is in the kept set
                for k in nrow_tru[i]..nrow_tru[i + 1] {
                    if jcol_tru[k] == j {
                        irow_tru[kl] = i;
                        kl += 1;
                        break;
                    }
                }
            }
        }
        ncol_tru[n] = kl;

        // Step 5: Count L and U entries
        // L: lower triangular (including diagonal) - stored by rows
        // U: upper triangular (excluding diagonal) - stored by columns
        let mut nunz_l = 0;
        let mut nunz_u = 0;

        for i in 0..n {
            // Entries in row i with column <= i go to L
            for k in nrow_tru[i]..nrow_tru[i + 1] {
                if jcol_tru[k] <= i {
                    nunz_l += 1;
                }
            }
            // Entries in column i with row < i go to U
            for k in ncol_tru[i]..ncol_tru[i + 1] {
                if irow_tru[k] < i {
                    nunz_u += 1;
                }
            }
        }

        // Step 6: Allocate L and U structures
        let mut l_col_indices = vec![0usize; nunz_l];
        let mut l_row_ptr = vec![0usize; n + 1];
        let mut u_row_indices = vec![0usize; nunz_u];
        let mut u_col_ptr = vec![0usize; n + 1];
        let mut l_values = vec![Complex64::new(0.0, 0.0); nunz_l];
        let mut u_values = vec![Complex64::new(0.0, 0.0); nunz_u];

        // Fill L and U index structures
        kl = 0;
        let mut ku = 0;
        for i in 0..n {
            l_row_ptr[i] = kl;
            for k in nrow_tru[i]..nrow_tru[i + 1] {
                let j = jcol_tru[k];
                if j <= i {
                    l_col_indices[kl] = j;
                    l_values[kl] = scaled[[i, j]];
                    kl += 1;
                }
            }

            u_col_ptr[i] = ku;
            for k in ncol_tru[i]..ncol_tru[i + 1] {
                let row = irow_tru[k];
                if row < i {
                    u_row_indices[ku] = row;
                    u_values[ku] = scaled[[row, i]];
                    ku += 1;
                }
            }
        }
        l_row_ptr[n] = kl;
        u_col_ptr[n] = ku;

        // Step 7: Incomplete LU decomposition
        // This is the core algorithm that computes L and U factors
        Self::compute_ilu_factorization(
            n,
            &mut l_values,
            &l_col_indices,
            &l_row_ptr,
            &mut u_values,
            &u_row_indices,
            &u_col_ptr,
            &jcol_tru,
            &nrow_tru,
            &irow_tru,
            &ncol_tru,
        );

        IluPreconditioner {
            l_values,
            l_col_indices,
            l_row_ptr,
            u_values,
            u_row_indices,
            u_col_ptr,
            n,
        }
    }

    /// Setup ILU preconditioner with scaled system
    ///
    /// This is the recommended method for BEM systems. It returns:
    /// - The ILU preconditioner
    /// - The row-scaled matrix (for use in matvec)
    /// - The row scaling factors (to scale the RHS)
    ///
    /// # Usage
    /// ```ignore
    /// let setup = IluPreconditioner::setup_system(&matrix, IluMethod::Tbem, IluScanningDegree::Fine);
    /// let scaled_rhs = &setup.row_scale * &rhs;  // Scale RHS
    /// // Use setup.scaled_matrix for matvec in CGS
    /// // Use setup.preconditioner for preconditioning
    /// ```
    pub fn setup_system(
        a: &Array2<Complex64>,
        method: IluMethod,
        degree: IluScanningDegree,
    ) -> IluSetup {
        let threshold = get_threshold_factor(method, degree);
        Self::setup_system_with_threshold(a, threshold)
    }

    /// Setup ILU preconditioner with custom threshold
    ///
    /// For dense TBEM matrices, use low threshold (e.g., 0.05-0.2).
    /// For sparse FMM near-field, use higher threshold (e.g., 0.3-1.0).
    pub fn setup_system_with_threshold(a: &Array2<Complex64>, threshold: f64) -> IluSetup {
        let n = a.nrows();
        assert_eq!(n, a.ncols(), "Matrix must be square");

        // Clone matrix for scaling
        let mut scaled = a.clone();
        let mut row_scale = Array1::from_elem(n, Complex64::new(1.0, 0.0));

        // Step 1: Row scaling
        for i in 0..n {
            let row_sum_sq: f64 = scaled.row(i).iter().map(|x| x.norm_sqr()).sum();
            if row_sum_sq > 1e-30 {
                let scale = (n as f64 / row_sum_sq).sqrt();
                for j in 0..n {
                    scaled[[i, j]] *= scale;
                }
                row_scale[i] = Complex64::new(scale, 0.0);
            }
        }

        // Step 2-7: Build ILU (same as from_matrix but using pre-scaled matrix)
        let mut keep = vec![vec![false; n]; n];
        let mut n_trues = 0;

        for i in 0..n {
            for j in 0..n {
                if scaled[[i, j]].norm() > threshold || i == j {
                    keep[i][j] = true;
                    n_trues += 1;
                }
            }
        }

        let mut jcol_tru = vec![0usize; n_trues];
        let mut nrow_tru = vec![0usize; n + 1];

        let mut kl = 0;
        for i in 0..n {
            nrow_tru[i] = kl;
            for j in 0..n {
                if keep[i][j] {
                    jcol_tru[kl] = j;
                    kl += 1;
                }
            }
        }
        nrow_tru[n] = kl;

        let mut irow_tru = vec![0usize; n_trues];
        let mut ncol_tru = vec![0usize; n + 1];

        kl = 0;
        for j in 0..n {
            ncol_tru[j] = kl;
            for i in 0..n {
                for k in nrow_tru[i]..nrow_tru[i + 1] {
                    if jcol_tru[k] == j {
                        irow_tru[kl] = i;
                        kl += 1;
                        break;
                    }
                }
            }
        }
        ncol_tru[n] = kl;

        let mut nunz_l = 0;
        let mut nunz_u = 0;

        for i in 0..n {
            for k in nrow_tru[i]..nrow_tru[i + 1] {
                if jcol_tru[k] <= i {
                    nunz_l += 1;
                }
            }
            for k in ncol_tru[i]..ncol_tru[i + 1] {
                if irow_tru[k] < i {
                    nunz_u += 1;
                }
            }
        }

        let mut l_col_indices = vec![0usize; nunz_l];
        let mut l_row_ptr = vec![0usize; n + 1];
        let mut u_row_indices = vec![0usize; nunz_u];
        let mut u_col_ptr = vec![0usize; n + 1];
        let mut l_values = vec![Complex64::new(0.0, 0.0); nunz_l];
        let mut u_values = vec![Complex64::new(0.0, 0.0); nunz_u];

        kl = 0;
        let mut ku = 0;
        for i in 0..n {
            l_row_ptr[i] = kl;
            for k in nrow_tru[i]..nrow_tru[i + 1] {
                let j = jcol_tru[k];
                if j <= i {
                    l_col_indices[kl] = j;
                    l_values[kl] = scaled[[i, j]];
                    kl += 1;
                }
            }

            u_col_ptr[i] = ku;
            for k in ncol_tru[i]..ncol_tru[i + 1] {
                let row = irow_tru[k];
                if row < i {
                    u_row_indices[ku] = row;
                    u_values[ku] = scaled[[row, i]];
                    ku += 1;
                }
            }
        }
        l_row_ptr[n] = kl;
        u_col_ptr[n] = ku;

        Self::compute_ilu_factorization(
            n,
            &mut l_values,
            &l_col_indices,
            &l_row_ptr,
            &mut u_values,
            &u_row_indices,
            &u_col_ptr,
            &jcol_tru,
            &nrow_tru,
            &irow_tru,
            &ncol_tru,
        );

        let preconditioner = IluPreconditioner {
            l_values,
            l_col_indices,
            l_row_ptr,
            u_values,
            u_row_indices,
            u_col_ptr,
            n,
        };

        IluSetup {
            preconditioner,
            scaled_matrix: scaled,
            row_scale,
        }
    }

    /// Create ILU preconditioner with custom threshold
    pub fn from_matrix_with_threshold(a: &Array2<Complex64>, threshold: f64) -> Self {
        let n = a.nrows();
        assert_eq!(n, a.ncols(), "Matrix must be square");

        // Clone matrix for scaling
        let mut scaled = a.clone();

        // Step 1: Row scaling
        for i in 0..n {
            let row_sum_sq: f64 = scaled.row(i).iter().map(|x| x.norm_sqr()).sum();
            if row_sum_sq > 1e-30 {
                let scale = (n as f64 / row_sum_sq).sqrt();
                for j in 0..n {
                    scaled[[i, j]] *= scale;
                }
            }
        }

        // Step 2: Identify entries to keep
        let mut keep = vec![vec![false; n]; n];
        let mut n_trues = 0;

        for i in 0..n {
            for j in 0..n {
                if scaled[[i, j]].norm() > threshold || i == j {
                    keep[i][j] = true;
                    n_trues += 1;
                }
            }
        }

        // Step 3-7: Same as from_matrix
        let mut jcol_tru = vec![0usize; n_trues];
        let mut nrow_tru = vec![0usize; n + 1];

        let mut kl = 0;
        for i in 0..n {
            nrow_tru[i] = kl;
            for j in 0..n {
                if keep[i][j] {
                    jcol_tru[kl] = j;
                    kl += 1;
                }
            }
        }
        nrow_tru[n] = kl;

        let mut irow_tru = vec![0usize; n_trues];
        let mut ncol_tru = vec![0usize; n + 1];

        kl = 0;
        for j in 0..n {
            ncol_tru[j] = kl;
            for i in 0..n {
                for k in nrow_tru[i]..nrow_tru[i + 1] {
                    if jcol_tru[k] == j {
                        irow_tru[kl] = i;
                        kl += 1;
                        break;
                    }
                }
            }
        }
        ncol_tru[n] = kl;

        let mut nunz_l = 0;
        let mut nunz_u = 0;

        for i in 0..n {
            for k in nrow_tru[i]..nrow_tru[i + 1] {
                if jcol_tru[k] <= i {
                    nunz_l += 1;
                }
            }
            for k in ncol_tru[i]..ncol_tru[i + 1] {
                if irow_tru[k] < i {
                    nunz_u += 1;
                }
            }
        }

        let mut l_col_indices = vec![0usize; nunz_l];
        let mut l_row_ptr = vec![0usize; n + 1];
        let mut u_row_indices = vec![0usize; nunz_u];
        let mut u_col_ptr = vec![0usize; n + 1];
        let mut l_values = vec![Complex64::new(0.0, 0.0); nunz_l];
        let mut u_values = vec![Complex64::new(0.0, 0.0); nunz_u];

        kl = 0;
        let mut ku = 0;
        for i in 0..n {
            l_row_ptr[i] = kl;
            for k in nrow_tru[i]..nrow_tru[i + 1] {
                let j = jcol_tru[k];
                if j <= i {
                    l_col_indices[kl] = j;
                    l_values[kl] = scaled[[i, j]];
                    kl += 1;
                }
            }

            u_col_ptr[i] = ku;
            for k in ncol_tru[i]..ncol_tru[i + 1] {
                let row = irow_tru[k];
                if row < i {
                    u_row_indices[ku] = row;
                    u_values[ku] = scaled[[row, i]];
                    ku += 1;
                }
            }
        }
        l_row_ptr[n] = kl;
        u_col_ptr[n] = ku;

        Self::compute_ilu_factorization(
            n,
            &mut l_values,
            &l_col_indices,
            &l_row_ptr,
            &mut u_values,
            &u_row_indices,
            &u_col_ptr,
            &jcol_tru,
            &nrow_tru,
            &irow_tru,
            &ncol_tru,
        );

        IluPreconditioner {
            l_values,
            l_col_indices,
            l_row_ptr,
            u_values,
            u_row_indices,
            u_col_ptr,
            n,
        }
    }

    /// Compute the incomplete LU factorization
    ///
    /// This is the core algorithm from NumCalc.
    /// Modifies l_values and u_values in place.
    #[allow(clippy::too_many_arguments)]
    fn compute_ilu_factorization(
        n: usize,
        l_values: &mut [Complex64],
        l_col_indices: &[usize],
        l_row_ptr: &[usize],
        u_values: &mut [Complex64],
        u_row_indices: &[usize],
        u_col_ptr: &[usize],
        jcol_tru: &[usize],
        nrow_tru: &[usize],
        irow_tru: &[usize],
        ncol_tru: &[usize],
    ) {
        // Boolean vectors for marking row/column membership
        let mut mi_row = vec![false; n];
        let mut mi_col = vec![false; n];
        let mut mk_vct = vec![false; n];

        for i in 0..n {
            // Reset all markers (critical: must reset ALL, not just 0..=i)
            for j in 0..n {
                mi_row[j] = false;
                mi_col[j] = false;
            }
            // Mark kept entries in row i (from original sparsity)
            for k in nrow_tru[i]..nrow_tru[i + 1] {
                mi_row[jcol_tru[k]] = true;
            }
            // Mark kept entries in column i
            for k in ncol_tru[i]..ncol_tru[i + 1] {
                mi_col[irow_tru[k]] = true;
            }

            // Update L column i (rows >= i)
            for k in i..n {
                if !mi_col[k] {
                    continue;
                }

                // Find position of L[k, i]
                let mut j1 = 0;
                for j in l_row_ptr[k]..l_row_ptr[k + 1] {
                    if l_col_indices[j] == i {
                        j1 = j;
                        break;
                    }
                }

                // Mark kept entries in row k
                for j in 0..n {
                    mk_vct[j] = false;
                }
                for j in nrow_tru[k]..nrow_tru[k + 1] {
                    mk_vct[jcol_tru[j]] = true;
                }

                // L[k,i] -= sum_{m<i} L[k,m] * U[m,i]
                let mut ml = 0;
                let mut mu = 0;
                for m in 0..i {
                    if mk_vct[m] && mi_col[m] {
                        l_values[j1] -= l_values[l_row_ptr[k] + ml] * u_values[u_col_ptr[i] + mu];
                    }
                    if mk_vct[m] {
                        ml += 1;
                    }
                    if mi_col[m] {
                        mu += 1;
                    }
                }
            }

            // Update U row i (columns > i)
            for k in (i + 1)..n {
                if !mi_row[k] {
                    continue;
                }

                // Find position of U[i, k]
                let mut j1 = 0;
                for j in u_col_ptr[k]..u_col_ptr[k + 1] {
                    if u_row_indices[j] == i {
                        j1 = j;
                        break;
                    }
                }

                // Mark kept entries in column k
                for j in 0..n {
                    mk_vct[j] = false;
                }
                for j in ncol_tru[k]..ncol_tru[k + 1] {
                    mk_vct[irow_tru[j]] = true;
                }

                // U[i,k] -= sum_{m<i} L[i,m] * U[m,k]
                let mut ml = 0;
                let mut mu = 0;
                for m in 0..i {
                    if mi_row[m] && mk_vct[m] {
                        u_values[j1] -= l_values[l_row_ptr[i] + ml] * u_values[u_col_ptr[k] + mu];
                    }
                    if mi_row[m] {
                        ml += 1;
                    }
                    if mk_vct[m] {
                        mu += 1;
                    }
                }

                // U[i,k] /= L[i,i]
                let l_diag_idx = l_row_ptr[i + 1] - 1;
                if l_values[l_diag_idx].norm() > 1e-30 {
                    u_values[j1] /= l_values[l_diag_idx];
                }
            }
        }
    }

    /// Forward-backward substitution: solve (LU)z = r
    ///
    /// 1. Forward: L * y = r
    /// 2. Backward: U * z = y
    fn forward_backward(&self, r: &Array1<Complex64>) -> Array1<Complex64> {
        let mut z = r.clone();

        // Forward elimination: L * y = r
        // L is lower triangular stored by rows
        if self.l_values.is_empty() {
            return z;
        }

        // z[0] /= L[0,0]
        let l_diag_0 = self.l_values[0];
        if l_diag_0.norm() > 1e-30 {
            z[0] /= l_diag_0;
        }

        for i in 1..self.n {
            // z[i] -= sum_{j<i} L[i,j] * z[j]
            // The last entry in row i is the diagonal
            let row_end = self.l_row_ptr[i + 1];
            let diag_idx = row_end - 1;

            for k in self.l_row_ptr[i]..diag_idx {
                let j = self.l_col_indices[k];
                let z_j = z[j];
                z[i] -= self.l_values[k] * z_j;
            }

            // z[i] /= L[i,i]
            if self.l_values[diag_idx].norm() > 1e-30 {
                z[i] /= self.l_values[diag_idx];
            }
        }

        // Backward substitution: U * result = z
        // U is upper triangular stored by columns
        // Process columns from right to left
        for i in (1..self.n).rev() {
            // For each entry U[row, i] where row < i:
            // z[row] -= U[row, i] * z[i]
            let z_i = z[i];
            for k in self.u_col_ptr[i]..self.u_col_ptr[i + 1] {
                let row = self.u_row_indices[k];
                z[row] -= self.u_values[k] * z_i;
            }
        }

        z
    }

    /// Get number of nonzeros in L
    pub fn nnz_l(&self) -> usize {
        self.l_values.len()
    }

    /// Get number of nonzeros in U
    pub fn nnz_u(&self) -> usize {
        self.u_values.len()
    }

    /// Get fill ratio (nnz(L+U) / n^2)
    pub fn fill_ratio(&self) -> f64 {
        (self.l_values.len() + self.u_values.len()) as f64 / (self.n * self.n) as f64
    }
}

impl Preconditioner for IluPreconditioner {
    fn apply(&self, r: &Array1<Complex64>) -> Array1<Complex64> {
        self.forward_backward(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ilu_simple() {
        // Simple 3x3 diagonally dominant matrix
        let a = Array2::from_shape_vec(
            (3, 3),
            vec![
                Complex64::new(4.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(5.0, 0.0),
                Complex64::new(2.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(2.0, 0.0),
                Complex64::new(6.0, 0.0),
            ],
        )
        .unwrap();

        let precond =
            IluPreconditioner::from_matrix(&a, IluMethod::Tbem, IluScanningDegree::Coarse);

        // Check that ILU was created
        assert!(precond.nnz_l() > 0);

        // Apply to a test vector
        let r = Array1::from_vec(vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(1.0, 0.0),
        ]);
        let z = precond.apply(&r);

        // Just check that it produces a result
        assert_eq!(z.len(), 3);
        assert!(z.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn test_ilu_complex() {
        // Test with complex entries (like BEM matrices)
        let a = Array2::from_shape_vec(
            (3, 3),
            vec![
                Complex64::new(4.0, 1.0),
                Complex64::new(1.0, -0.5),
                Complex64::new(0.5, 0.0),
                Complex64::new(1.0, 0.5),
                Complex64::new(5.0, -1.0),
                Complex64::new(2.0, 0.3),
                Complex64::new(0.5, 0.0),
                Complex64::new(2.0, -0.3),
                Complex64::new(6.0, 2.0),
            ],
        )
        .unwrap();

        let precond = IluPreconditioner::from_matrix(&a, IluMethod::Tbem, IluScanningDegree::Fine);

        let r = Array1::from_vec(vec![
            Complex64::new(1.0, 0.5),
            Complex64::new(2.0, -0.5),
            Complex64::new(0.5, 1.0),
        ]);
        let z = precond.apply(&r);

        assert_eq!(z.len(), 3);
        assert!(z.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn test_ilu_fill_ratio() {
        // For a dense matrix with low threshold, fill ratio should be high
        let n = 10;
        let mut a = Array2::zeros((n, n));
        for i in 0..n {
            for j in 0..n {
                a[[i, j]] = Complex64::new((i + j) as f64 + 1.0, 0.0);
            }
            // Strong diagonal
            a[[i, i]] = Complex64::new((n * 2) as f64, 0.0);
        }

        let precond_coarse =
            IluPreconditioner::from_matrix(&a, IluMethod::Tbem, IluScanningDegree::Coarse);
        let precond_finest =
            IluPreconditioner::from_matrix(&a, IluMethod::Tbem, IluScanningDegree::Finest);

        // Finest should have higher fill ratio (lower threshold)
        assert!(precond_finest.fill_ratio() >= precond_coarse.fill_ratio());
    }
}
