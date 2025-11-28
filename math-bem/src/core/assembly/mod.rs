//! BEM matrix assembly
//!
//! This module provides three methods for assembling BEM systems:
//!
//! - [`tbem`] - Traditional BEM with O(N²) dense matrix
//! - [`slfmm`] - Single-Level Fast Multipole Method
//! - [`mlfmm`] - Multi-Level Fast Multipole Method
//!
//! For small problems (N < 1000), TBEM is usually fastest.
//! For larger problems, FMM methods provide O(N log N) or O(N) scaling.

pub mod mlfmm;
pub mod slfmm;
pub mod sparse;
pub mod tbem;

pub use mlfmm::{build_cluster_tree, build_mlfmm_system, MlfmmSystem};
pub use slfmm::{build_slfmm_system, SlfmmSystem};
pub use sparse::{BlockedCsr, CsrBuilder, CsrMatrix};
pub use tbem::{
    apply_row_sum_correction, build_tbem_system, build_tbem_system_corrected,
    build_tbem_system_scaled, build_tbem_system_with_beta, TbemSystem,
};

#[cfg(feature = "parallel")]
pub use tbem::build_tbem_system_parallel;
