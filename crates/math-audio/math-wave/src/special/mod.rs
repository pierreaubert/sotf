//! Special mathematical functions for wave equations
//!
//! This module provides implementations of special functions commonly
//! used in wave equation solutions:
//!
//! - Spherical Bessel functions (jₙ, yₙ)
//! - Spherical Hankel functions (hₙ⁽¹⁾, hₙ⁽²⁾)
//! - Legendre polynomials (Pₙ, Pₙᵐ)
//!
//! These functions are critical for:
//! - Mie theory (sphere scattering)
//! - Spherical harmonic expansions
//! - FMM translation operators

pub mod helmholtz;
mod legendre;
pub mod spherical;

pub use helmholtz::*;
pub use legendre::*;
pub use spherical::*;
