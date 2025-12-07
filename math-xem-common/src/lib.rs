//! Common types and utilities for BEM/FEM room acoustics simulators
//!
//! This crate provides shared functionality between BEM and FEM room acoustics
//! simulators, including:
//!
//! - Room geometry definitions (rectangular, L-shaped rooms)
//! - Sound source configuration (position, directivity, crossover)
//! - JSON configuration loading/saving
//! - Output JSON formatting
//! - Visualization utilities

mod config;
mod geometry;
mod output;
mod source;
mod types;

pub use config::*;
pub use geometry::*;
pub use output::*;
pub use source::*;
pub use types::*;

/// Library version
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!version().is_empty());
    }
}
