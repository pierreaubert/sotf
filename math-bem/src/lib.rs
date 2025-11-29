//! # BEM: Boundary Element Method Library
//!
//! High-performance, memory-efficient BEM solver for acoustic scattering problems.
//!
//! ## Features
//!
//! - FFI wrapper to NumCalc C++ BEM solver
//! - Parallel execution with Rayon (memory-efficient, no async overhead)
//! - Comprehensive analytical validation (1D, 2D, 3D)
//! - JSON output for visualization
//!
//! ## Example
//!
//! ```rust,no_run
//! use bem::{NumCalcRunner, NumCalcConfig};
//!
//! let runner = NumCalcRunner::new("project_dir")?;
//! let config = NumCalcConfig::default();
//! let output = runner.run(&config)?;
//! # Ok::<(), anyhow::Error>(())
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![allow(clippy::too_many_arguments)] // Scientific code often has many parameters

pub mod analytical;
pub mod testing;

#[cfg(feature = "ffi")]
pub mod ffi;

pub mod core;

// Re-exports
pub use analytical::*;
pub use testing::*;

#[cfg(feature = "ffi")]
pub use ffi::{NumCalcConfig, NumCalcOutput, NumCalcRunner, ParallelBemRunner};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Git commit hash (set during build)
pub const GIT_HASH: &str = env!("GIT_HASH");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
