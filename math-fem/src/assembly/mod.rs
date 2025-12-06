//! Finite element matrix assembly
//!
//! Assembles stiffness, mass, and combined Helmholtz matrices from mesh and basis functions.

mod helmholtz;
mod mass;
mod stiffness;

pub use helmholtz::*;
pub use mass::*;
pub use stiffness::*;
