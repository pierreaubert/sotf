//! Sparse matrix structures (CSR format)
//!
//! This module provides Compressed Sparse Row (CSR) format for efficient
//! storage and matrix-vector operations with sparse matrices.

mod csr;

pub use csr::{BlockedCsr, CsrBuilder, CsrMatrix};
