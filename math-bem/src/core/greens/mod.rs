//! Green's function computations for acoustic BEM
//!
//! This module provides:
//! - Helmholtz Green's function G = exp(ikr)/(4πr)
//! - Spherical Bessel and Hankel functions
//! - Legendre polynomials
//! - Derivatives for hypersingular kernels

pub mod spherical;
pub mod legendre;
pub mod helmholtz;

pub use spherical::*;
pub use legendre::*;
pub use helmholtz::*;
