//! Mesh types and generators for FEM
//!
//! This module provides mesh data structures and generators for 2D and 3D finite element analysis.

mod generators;
mod refinement;
mod types;

pub use generators::*;
pub use refinement::*;
pub use types::*;
