//! BEM matrix assembly
//!
//! This module provides three methods for assembling BEM systems:
//!
//! - [`tbem`] - Traditional BEM with O(N²) dense matrix (always available)
//! - [`slfmm`] - Single-Level Fast Multipole Method (requires `native` or `wasm` feature)
//! - [`mlfmm`] - Multi-Level Fast Multipole Method
//!
//! For small problems (N < 1000), TBEM is usually fastest.
//! For larger problems, FMM methods provide O(N log N) or O(N) scaling.
//!
//! ## WASM Compatibility
//!
//! With the `wasm` feature, all assembly methods are available including SLFMM.
//! Parallel processing is provided via wasm-bindgen-rayon (Web Workers).

// Allow needless_range_loop for complex FMM algorithm code ported from NumCalc
#[allow(clippy::needless_range_loop)]
pub mod mlfmm;
#[cfg(any(feature = "native", feature = "wasm"))]
pub mod slfmm;
pub mod sparse;
pub mod tbem;

pub use mlfmm::{MlfmmSystem, build_cluster_tree, build_mlfmm_system};
#[cfg(any(feature = "native", feature = "wasm"))]
pub use slfmm::{SlfmmSystem, build_slfmm_system};
pub use sparse::{BlockedCsr, CsrBuilder, CsrMatrix};
pub use tbem::{
    TbemSystem, apply_row_sum_correction, build_tbem_system, build_tbem_system_corrected,
    build_tbem_system_scaled, build_tbem_system_with_beta,
};

#[cfg(feature = "parallel")]
pub use tbem::build_tbem_system_parallel;
