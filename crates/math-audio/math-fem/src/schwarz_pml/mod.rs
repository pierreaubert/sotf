//! Optimized Schwarz Methods with PML transmission conditions
//!
//! Domain decomposition solver for the Helmholtz equation that achieves
//! bounded iteration counts independent of wavenumber k by using
//! Perfectly Matched Layer (PML) absorption at artificial subdomain boundaries.
//!
//! # Algorithm
//!
//! 1. Decompose domain into overlapping 1D strips along the x-axis
//! 2. Extend each strip with PML layers at artificial (interior) boundaries
//! 3. Assemble PML-modified Helmholtz systems on each subdomain
//! 4. Iterate: solve local problems, combine with partition of unity
//!
//! Supports both additive (parallel) and multiplicative (sequential) variants.
//!
//! # Reference
//!
//! Galkowski et al. (2024), "Optimised Schwarz methods with PML transmission
//! conditions for the Helmholtz equation", arXiv:2408.16580

mod config;
mod decomposition;
mod local_assembly;
mod solver;

pub use config::{SchwarzPmlConfig, SchwarzVariant};
pub use decomposition::{SubdomainInfo, compute_partition_of_unity, decompose_domain};
pub use solver::solve_schwarz_pml;
