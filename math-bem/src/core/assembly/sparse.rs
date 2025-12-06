//! Sparse matrix structures (CSR format)
//!
//! This module provides Compressed Sparse Row (CSR) format for efficient
//! storage and matrix-vector operations with sparse matrices.
//!
//! CSR format stores:
//! - `values`: Non-zero entries in row-major order
//! - `col_indices`: Column index for each value
//! - `row_ptrs`: Index into values/col_indices where each row starts
//!
//! For BEM, the near-field matrix is sparse (only nearby element interactions),
//! while far-field is handled via FMM factorization.

use ndarray::Array1;
use num_complex::Complex64;
use std::ops::Range;

/// Compressed Sparse Row (CSR) matrix format
///
/// Memory-efficient storage for sparse matrices with O(nnz) space complexity.
/// Matrix-vector products are O(nnz) instead of O(n²) for dense matrices.
#[derive(Debug, Clone)]
pub struct CsrMatrix {
    /// Number of rows
    pub num_rows: usize,
    /// Number of columns
    pub num_cols: usize,
    /// Non-zero values in row-major order
    pub values: Vec<Complex64>,
    /// Column indices for each value
    pub col_indices: Vec<usize>,
    /// Row pointers: row_ptrs[i] is the start index in values/col_indices for row i
    /// row_ptrs[num_rows] = nnz (total number of non-zeros)
    pub row_ptrs: Vec<usize>,
}

impl CsrMatrix {
    /// Create a new empty CSR matrix
    pub fn new(num_rows: usize, num_cols: usize) -> Self {
        Self {
            num_rows,
            num_cols,
            values: Vec::new(),
            col_indices: Vec::new(),
            row_ptrs: vec![0; num_rows + 1],
        }
    }

    /// Create a CSR matrix with pre-allocated capacity
    pub fn with_capacity(num_rows: usize, num_cols: usize, nnz_estimate: usize) -> Self {
        Self {
            num_rows,
            num_cols,
            values: Vec::with_capacity(nnz_estimate),
            col_indices: Vec::with_capacity(nnz_estimate),
            row_ptrs: vec![0; num_rows + 1],
        }
    }

    /// Create a CSR matrix from a dense matrix
    ///
    /// Only stores entries with magnitude > threshold
    pub fn from_dense(dense: &ndarray::Array2<Complex64>, threshold: f64) -> Self {
        let num_rows = dense.nrows();
        let num_cols = dense.ncols();

        let mut values = Vec::new();
        let mut col_indices = Vec::new();
        let mut row_ptrs = vec![0usize; num_rows + 1];

        for i in 0..num_rows {
            for j in 0..num_cols {
                let val = dense[[i, j]];
                if val.norm() > threshold {
                    values.push(val);
                    col_indices.push(j);
                }
            }
            row_ptrs[i + 1] = values.len();
        }

        Self {
            num_rows,
            num_cols,
            values,
            col_indices,
            row_ptrs,
        }
    }

    /// Create a CSR matrix from COO (Coordinate) format triplets
    ///
    /// Triplets are (row, col, value). Duplicate entries are summed.
    pub fn from_triplets(
        num_rows: usize,
        num_cols: usize,
        mut triplets: Vec<(usize, usize, Complex64)>,
    ) -> Self {
        if triplets.is_empty() {
            return Self::new(num_rows, num_cols);
        }

        // Sort by row, then by column
        triplets.sort_by(|a, b| {
            if a.0 != b.0 {
                a.0.cmp(&b.0)
            } else {
                a.1.cmp(&b.1)
            }
        });

        let mut values = Vec::with_capacity(triplets.len());
        let mut col_indices = Vec::with_capacity(triplets.len());
        let mut row_ptrs = vec![0usize; num_rows + 1];

        let mut prev_row = usize::MAX;
        let mut prev_col = usize::MAX;

        for (row, col, val) in triplets {
            if row == prev_row && col == prev_col {
                // Same entry, accumulate
                if let Some(last) = values.last_mut() {
                    *last += val;
                }
            } else {
                // New entry - push it
                values.push(val);
                col_indices.push(col);

                // Update row pointers for any rows we skipped
                if row != prev_row {
                    let start = if prev_row == usize::MAX {
                        0
                    } else {
                        prev_row + 1
                    };
                    for item in row_ptrs.iter_mut().take(row + 1).skip(start) {
                        *item = values.len() - 1;
                    }
                }

                prev_row = row;
                prev_col = col;
            }
        }

        // Fill remaining row pointers
        let last_row = if prev_row == usize::MAX {
            0
        } else {
            prev_row + 1
        };
        for item in row_ptrs.iter_mut().take(num_rows + 1).skip(last_row) {
            *item = values.len();
        }

        Self {
            num_rows,
            num_cols,
            values,
            col_indices,
            row_ptrs,
        }
    }

    /// Number of non-zero entries
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Sparsity ratio (fraction of non-zero entries)
    pub fn sparsity(&self) -> f64 {
        let total = self.num_rows * self.num_cols;
        if total == 0 {
            0.0
        } else {
            self.nnz() as f64 / total as f64
        }
    }

    /// Get the range of indices in values/col_indices for a given row
    pub fn row_range(&self, row: usize) -> Range<usize> {
        self.row_ptrs[row]..self.row_ptrs[row + 1]
    }

    /// Get the (col, value) pairs for a row
    pub fn row_entries(&self, row: usize) -> impl Iterator<Item = (usize, Complex64)> + '_ {
        let range = self.row_range(row);
        self.col_indices[range.clone()]
            .iter()
            .copied()
            .zip(self.values[range].iter().copied())
    }

    /// Matrix-vector product: y = A * x
    pub fn matvec(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        assert_eq!(x.len(), self.num_cols, "Input vector size mismatch");

        let mut y = Array1::zeros(self.num_rows);

        for i in 0..self.num_rows {
            let mut sum = Complex64::new(0.0, 0.0);
            for idx in self.row_range(i) {
                let j = self.col_indices[idx];
                sum += self.values[idx] * x[j];
            }
            y[i] = sum;
        }

        y
    }

    /// Matrix-vector product with accumulation: y += A * x
    pub fn matvec_add(&self, x: &Array1<Complex64>, y: &mut Array1<Complex64>) {
        assert_eq!(x.len(), self.num_cols, "Input vector size mismatch");
        assert_eq!(y.len(), self.num_rows, "Output vector size mismatch");

        for i in 0..self.num_rows {
            for idx in self.row_range(i) {
                let j = self.col_indices[idx];
                y[i] += self.values[idx] * x[j];
            }
        }
    }

    /// Transpose matrix-vector product: y = A^T * x
    pub fn matvec_transpose(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        assert_eq!(x.len(), self.num_rows, "Input vector size mismatch");

        let mut y = Array1::zeros(self.num_cols);

        for i in 0..self.num_rows {
            for idx in self.row_range(i) {
                let j = self.col_indices[idx];
                y[j] += self.values[idx] * x[i];
            }
        }

        y
    }

    /// Hermitian (conjugate transpose) matrix-vector product: y = A^H * x
    pub fn matvec_hermitian(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        assert_eq!(x.len(), self.num_rows, "Input vector size mismatch");

        let mut y = Array1::zeros(self.num_cols);

        for i in 0..self.num_rows {
            for idx in self.row_range(i) {
                let j = self.col_indices[idx];
                y[j] += self.values[idx].conj() * x[i];
            }
        }

        y
    }

    /// Get element at (i, j), returns 0 if not stored
    pub fn get(&self, i: usize, j: usize) -> Complex64 {
        for idx in self.row_range(i) {
            if self.col_indices[idx] == j {
                return self.values[idx];
            }
        }
        Complex64::new(0.0, 0.0)
    }

    /// Extract diagonal elements
    pub fn diagonal(&self) -> Array1<Complex64> {
        let n = self.num_rows.min(self.num_cols);
        let mut diag = Array1::zeros(n);

        for i in 0..n {
            diag[i] = self.get(i, i);
        }

        diag
    }

    /// Scale all values by a scalar
    pub fn scale(&mut self, scalar: Complex64) {
        for val in &mut self.values {
            *val *= scalar;
        }
    }

    /// Add a scalar to the diagonal
    pub fn add_diagonal(&mut self, scalar: Complex64) {
        let n = self.num_rows.min(self.num_cols);

        for i in 0..n {
            for idx in self.row_range(i) {
                if self.col_indices[idx] == i {
                    self.values[idx] += scalar;
                    break;
                }
            }
        }
    }

    /// Create identity matrix in CSR format
    pub fn identity(n: usize) -> Self {
        Self {
            num_rows: n,
            num_cols: n,
            values: vec![Complex64::new(1.0, 0.0); n],
            col_indices: (0..n).collect(),
            row_ptrs: (0..=n).collect(),
        }
    }

    /// Create diagonal matrix from vector
    pub fn from_diagonal(diag: &Array1<Complex64>) -> Self {
        let n = diag.len();
        Self {
            num_rows: n,
            num_cols: n,
            values: diag.to_vec(),
            col_indices: (0..n).collect(),
            row_ptrs: (0..=n).collect(),
        }
    }

    /// Convert to dense matrix (for debugging/small matrices)
    pub fn to_dense(&self) -> ndarray::Array2<Complex64> {
        let mut dense = ndarray::Array2::zeros((self.num_rows, self.num_cols));

        for i in 0..self.num_rows {
            for idx in self.row_range(i) {
                let j = self.col_indices[idx];
                dense[[i, j]] = self.values[idx];
            }
        }

        dense
    }
}

/// Builder for constructing CSR matrices row by row
pub struct CsrBuilder {
    num_rows: usize,
    num_cols: usize,
    values: Vec<Complex64>,
    col_indices: Vec<usize>,
    row_ptrs: Vec<usize>,
    current_row: usize,
}

impl CsrBuilder {
    /// Create a new CSR builder
    pub fn new(num_rows: usize, num_cols: usize) -> Self {
        Self {
            num_rows,
            num_cols,
            values: Vec::new(),
            col_indices: Vec::new(),
            row_ptrs: vec![0],
            current_row: 0,
        }
    }

    /// Create a new CSR builder with estimated non-zeros
    pub fn with_capacity(num_rows: usize, num_cols: usize, nnz_estimate: usize) -> Self {
        Self {
            num_rows,
            num_cols,
            values: Vec::with_capacity(nnz_estimate),
            col_indices: Vec::with_capacity(nnz_estimate),
            row_ptrs: Vec::with_capacity(num_rows + 1),
            current_row: 0,
        }
    }

    /// Add entries for the current row (must be added in column order)
    pub fn add_row_entries(&mut self, entries: impl Iterator<Item = (usize, Complex64)>) {
        for (col, val) in entries {
            if val.norm() > 0.0 {
                self.values.push(val);
                self.col_indices.push(col);
            }
        }
        self.row_ptrs.push(self.values.len());
        self.current_row += 1;
    }

    /// Finish building and return the CSR matrix
    pub fn finish(mut self) -> CsrMatrix {
        // Fill remaining rows if not all rows were added
        while self.current_row < self.num_rows {
            self.row_ptrs.push(self.values.len());
            self.current_row += 1;
        }

        CsrMatrix {
            num_rows: self.num_rows,
            num_cols: self.num_cols,
            values: self.values,
            col_indices: self.col_indices,
            row_ptrs: self.row_ptrs,
        }
    }
}

/// Blocked CSR format for hierarchical matrices
///
/// Stores the matrix as a collection of dense blocks at the leaf level
/// of a hierarchical decomposition.
#[derive(Debug, Clone)]
pub struct BlockedCsr {
    /// Number of rows
    pub num_rows: usize,
    /// Number of columns
    pub num_cols: usize,
    /// Block size (rows and columns per block)
    pub block_size: usize,
    /// Number of block rows
    pub num_block_rows: usize,
    /// Number of block columns
    pub num_block_cols: usize,
    /// Dense blocks stored in CSR-like format
    /// Each block is a dense matrix
    pub blocks: Vec<ndarray::Array2<Complex64>>,
    /// Block column indices
    pub block_col_indices: Vec<usize>,
    /// Block row pointers
    pub block_row_ptrs: Vec<usize>,
}

impl BlockedCsr {
    /// Create a new blocked CSR matrix
    pub fn new(num_rows: usize, num_cols: usize, block_size: usize) -> Self {
        let num_block_rows = num_rows.div_ceil(block_size);
        let num_block_cols = num_cols.div_ceil(block_size);

        Self {
            num_rows,
            num_cols,
            block_size,
            num_block_rows,
            num_block_cols,
            blocks: Vec::new(),
            block_col_indices: Vec::new(),
            block_row_ptrs: vec![0; num_block_rows + 1],
        }
    }

    /// Matrix-vector product using blocked structure
    pub fn matvec(&self, x: &Array1<Complex64>) -> Array1<Complex64> {
        assert_eq!(x.len(), self.num_cols, "Input vector size mismatch");

        let mut y = Array1::zeros(self.num_rows);

        for block_i in 0..self.num_block_rows {
            let row_start = block_i * self.block_size;
            let row_end = (row_start + self.block_size).min(self.num_rows);
            let local_rows = row_end - row_start;

            for idx in self.block_row_ptrs[block_i]..self.block_row_ptrs[block_i + 1] {
                let block_j = self.block_col_indices[idx];
                let block = &self.blocks[idx];

                let col_start = block_j * self.block_size;
                let col_end = (col_start + self.block_size).min(self.num_cols);
                let local_cols = col_end - col_start;

                // Extract local x
                let x_local: Array1<Complex64> =
                    Array1::from_iter((col_start..col_end).map(|j| x[j]));

                // Apply block
                for i in 0..local_rows {
                    let mut sum = Complex64::new(0.0, 0.0);
                    for j in 0..local_cols {
                        sum += block[[i, j]] * x_local[j];
                    }
                    y[row_start + i] += sum;
                }
            }
        }

        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn test_csr_from_dense() {
        let dense = array![
            [
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(2.0, 0.0)
            ],
            [
                Complex64::new(0.0, 0.0),
                Complex64::new(3.0, 0.0),
                Complex64::new(0.0, 0.0)
            ],
            [
                Complex64::new(4.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(5.0, 0.0)
            ],
        ];

        let csr = CsrMatrix::from_dense(&dense, 1e-15);

        assert_eq!(csr.num_rows, 3);
        assert_eq!(csr.num_cols, 3);
        assert_eq!(csr.nnz(), 5);

        // Check values
        assert_eq!(csr.get(0, 0), Complex64::new(1.0, 0.0));
        assert_eq!(csr.get(0, 2), Complex64::new(2.0, 0.0));
        assert_eq!(csr.get(1, 1), Complex64::new(3.0, 0.0));
        assert_eq!(csr.get(2, 0), Complex64::new(4.0, 0.0));
        assert_eq!(csr.get(2, 2), Complex64::new(5.0, 0.0));

        // Check zeros
        assert_eq!(csr.get(0, 1), Complex64::new(0.0, 0.0));
        assert_eq!(csr.get(1, 0), Complex64::new(0.0, 0.0));
    }

    #[test]
    fn test_csr_matvec() {
        let dense = array![
            [Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)],
            [Complex64::new(3.0, 0.0), Complex64::new(4.0, 0.0)],
        ];

        let csr = CsrMatrix::from_dense(&dense, 1e-15);
        let x = array![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)];

        let y = csr.matvec(&x);

        // [1 2] * [1]   [5]
        // [3 4]   [2] = [11]
        assert!((y[0] - Complex64::new(5.0, 0.0)).norm() < 1e-10);
        assert!((y[1] - Complex64::new(11.0, 0.0)).norm() < 1e-10);
    }

    #[test]
    fn test_csr_from_triplets() {
        let triplets = vec![
            (0, 0, Complex64::new(1.0, 0.0)),
            (0, 2, Complex64::new(2.0, 0.0)),
            (1, 1, Complex64::new(3.0, 0.0)),
            (2, 0, Complex64::new(4.0, 0.0)),
            (2, 2, Complex64::new(5.0, 0.0)),
        ];

        let csr = CsrMatrix::from_triplets(3, 3, triplets);

        assert_eq!(csr.nnz(), 5);
        assert_eq!(csr.get(0, 0), Complex64::new(1.0, 0.0));
        assert_eq!(csr.get(1, 1), Complex64::new(3.0, 0.0));
    }

    #[test]
    fn test_csr_triplets_duplicate() {
        // Test that duplicate entries are summed
        let triplets = vec![
            (0, 0, Complex64::new(1.0, 0.0)),
            (0, 0, Complex64::new(2.0, 0.0)), // Duplicate!
            (1, 1, Complex64::new(3.0, 0.0)),
        ];

        let csr = CsrMatrix::from_triplets(2, 2, triplets);

        assert_eq!(csr.get(0, 0), Complex64::new(3.0, 0.0)); // 1 + 2 = 3
    }

    #[test]
    fn test_csr_identity() {
        let id = CsrMatrix::identity(3);

        assert_eq!(id.nnz(), 3);
        assert_eq!(id.get(0, 0), Complex64::new(1.0, 0.0));
        assert_eq!(id.get(1, 1), Complex64::new(1.0, 0.0));
        assert_eq!(id.get(2, 2), Complex64::new(1.0, 0.0));
        assert_eq!(id.get(0, 1), Complex64::new(0.0, 0.0));
    }

    #[test]
    fn test_csr_builder() {
        let mut builder = CsrBuilder::new(3, 3);

        // Row 0: entries at columns 0 and 2
        builder.add_row_entries(
            [(0, Complex64::new(1.0, 0.0)), (2, Complex64::new(2.0, 0.0))].into_iter(),
        );

        // Row 1: entry at column 1
        builder.add_row_entries([(1, Complex64::new(3.0, 0.0))].into_iter());

        // Row 2: entries at columns 0 and 2
        builder.add_row_entries(
            [(0, Complex64::new(4.0, 0.0)), (2, Complex64::new(5.0, 0.0))].into_iter(),
        );

        let csr = builder.finish();

        assert_eq!(csr.nnz(), 5);
        assert_eq!(csr.get(0, 0), Complex64::new(1.0, 0.0));
        assert_eq!(csr.get(1, 1), Complex64::new(3.0, 0.0));
    }

    #[test]
    fn test_csr_to_dense_roundtrip() {
        let original = array![
            [Complex64::new(1.0, 0.5), Complex64::new(0.0, 0.0)],
            [Complex64::new(2.0, -1.0), Complex64::new(3.0, 0.0)],
        ];

        let csr = CsrMatrix::from_dense(&original, 1e-15);
        let recovered = csr.to_dense();

        for i in 0..2 {
            for j in 0..2 {
                assert!((original[[i, j]] - recovered[[i, j]]).norm() < 1e-10);
            }
        }
    }

    #[test]
    fn test_csr_transpose_matvec() {
        let dense = array![
            [Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)],
            [Complex64::new(3.0, 0.0), Complex64::new(4.0, 0.0)],
            [Complex64::new(5.0, 0.0), Complex64::new(6.0, 0.0)],
        ];

        let csr = CsrMatrix::from_dense(&dense, 1e-15);
        let x = array![
            Complex64::new(1.0, 0.0),
            Complex64::new(2.0, 0.0),
            Complex64::new(3.0, 0.0)
        ];

        let y = csr.matvec_transpose(&x);

        // A^T * x = [1 3 5] * [1]   [22]
        //           [2 4 6]   [2] = [28]
        //                     [3]
        assert!((y[0] - Complex64::new(22.0, 0.0)).norm() < 1e-10);
        assert!((y[1] - Complex64::new(28.0, 0.0)).norm() < 1e-10);
    }
}
