//! Green's function computations for acoustic BEM
//!
//! This module provides:
//! - Helmholtz Green's function G = exp(ikr)/(4πr)
//! - Spherical Bessel and Hankel functions
//! - Legendre polynomials
//! - Derivatives for hypersingular kernels

pub mod helmholtz;
pub mod legendre;
pub mod spherical;

pub use helmholtz::*;
pub use legendre::*;
pub use spherical::*;
