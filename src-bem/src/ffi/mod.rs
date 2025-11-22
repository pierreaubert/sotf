//! FFI bindings to NumCalc C++ BEM solver
//!
//! This module provides a safe, high-level interface to the NumCalc
//! boundary element method solver via subprocess execution.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │  Rust Application                                       │
//! │  ┌─────────────────┐    ┌──────────────────┐          │
//! │  │ ParallelBemRunner│───>│ NumCalcRunner    │          │
//! │  │  (Rayon)        │    │  (Subprocess)    │          │
//! │  └─────────────────┘    └──────────────────┘          │
//! └─────────────────────────────────────────────────────────┘
//!                                   │
//!                                   ▼
//!                          ┌─────────────────┐
//!                          │  NumCalc.exe    │
//!                          │  (C++ BEM)      │
//!                          └─────────────────┘
//! ```
//!
//! ## Design Decisions
//!
//! **Subprocess vs Direct FFI**:
//! - ✅ No C++ ABI compatibility issues
//! - ✅ NumCalc already designed as standalone executable
//! - ✅ Easier cross-platform builds
//! - ✅ Process isolation (crashes don't take down Rust)
//! - ⚠️ Slightly higher overhead (acceptable for BEM)
//!
//! **Rayon vs Tokio**:
//! - ✅ Data parallelism (perfect for frequency sweeps)
//! - ✅ No async overhead
//! - ✅ Work stealing for load balancing
//! - ✅ CPU-bound workloads (BEM is pure compute)
//!
//! ## Example
//!
//! ```rust,no_run
//! use bem::ffi::{NumCalcRunner, NumCalcConfig};
//!
//! // Single frequency
//! let runner = NumCalcRunner::new("project_dir")?;
//! let config = NumCalcConfig {
//!     freq_start_idx: Some(0),
//!     freq_end_idx: Some(0),
//!     ..Default::default()
//! };
//! let output = runner.run(&config)?;
//!
//! // Parallel frequency sweep
//! use bem::ffi::ParallelBemRunner;
//! let parallel = ParallelBemRunner::new("project_dir")?;
//! let results = parallel.run_all_frequencies(100)?;
//! # Ok::<(), anyhow::Error>(())
//! ```

pub mod config;
pub mod parallel;
pub mod resources;
pub mod runner;

pub use config::{NumCalcConfig, NumCalcOutput};
pub use parallel::ParallelBemRunner;
pub use resources::{ResourceMonitor, SystemResources};
pub use runner::NumCalcRunner;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = NumCalcConfig::default();
        assert!(config.freq_start_idx.is_none());
        assert!(config.freq_end_idx.is_none());
        assert_eq!(config.max_iterations, 250);
        assert!(!config.estimate_ram);
    }
}
