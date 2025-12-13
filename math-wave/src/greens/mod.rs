//! Green's functions for wave equations
//!
//! This module provides Green's function implementations for the
//! Helmholtz equation and its derivatives.
//!
//! The 3D Helmholtz Green's function is:
//! ```text
//! G(x, y) = exp(ik|x-y|) / (4π|x-y|)
//! ```
//!
//! For 2D (cylindrical) problems:
//! ```text
//! G(x, y) = (i/4) H_0^(1)(k|x-y|)
//! ```

mod helmholtz;
mod legendre;
mod spherical;

pub use helmholtz::*;
pub use legendre::*;
pub use spherical::*;
