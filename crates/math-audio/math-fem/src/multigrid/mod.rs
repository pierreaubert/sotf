//! Geometric multigrid solver for finite element problems
//!
//! Provides V-cycle, W-cycle, and F-cycle multigrid methods with
//! Gauss-Seidel smoothing and linear interpolation transfer operators.

mod cycle;
mod hierarchy;
mod smoother;
mod transfer;

pub use cycle::*;
pub use hierarchy::*;
pub use smoother::*;
pub use transfer::*;
