//! # BEM: Boundary Element Method Library
//!
//! High-performance, memory-efficient BEM solver for acoustic scattering problems.
//!
//! ## Features
//!
//! - Parallel execution with Rayon (memory-efficient, no async overhead)
//! - Comprehensive analytical validation (1D, 2D, 3D)
//! - JSON output for visualization
//!

#![warn(missing_docs)]
#![warn(clippy::all)]
#![allow(clippy::too_many_arguments)] // Scientific code often has many parameters

// pub mod analytical; // Removed - use math_audio_wave::analytical instead
pub mod core;
pub mod room_acoustics;
pub mod testing;

// Re-exports
pub use math_audio_wave::analytical;
pub use testing::*;

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
