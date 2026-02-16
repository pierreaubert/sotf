//! WaveHoltz solver for the Helmholtz equation
//!
//! Converts the indefinite Helmholtz problem `(K - ω²M)u = b` into positive-definite
//! wave equation time-steps, enabling O(N) solves via CG+AMG inner solves.
//!
//! # Algorithm
//!
//! 1. Time-step the wave equation `M w_tt + K w = b cos(ωt)` using implicit Newmark
//! 2. Apply cosine filter to extract the Helmholtz solution component
//! 3. Use GMRES to accelerate the fixed-point iteration `u = W(u)`
//!
//! # References
//!
//! - Appelo, Garcia, Runborg. "WaveHoltz: Iterative Solution of the Helmholtz Equation
//!   via the Wave Equation." SIAM J. Sci. Comput., 2020.
//! - Appelo, Banks, Henshaw, Schillinger. "A Multi-Frequency Helmholtz Solver Based
//!   on the WaveHoltz Algorithm." 2025.

mod config;
mod filter;
mod solver;
mod time_stepper;

pub use config::WaveHoltzConfig;
pub use filter::CosineFilter;
pub use solver::{solve_waveholtz, solve_waveholtz_multi_frequency};
pub(crate) use solver::solve_waveholtz_from_problem;
pub use time_stepper::WaveTimeStepper;
