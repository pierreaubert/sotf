//! Numerical quadrature rules for finite element integration
//!
//! Provides Gauss-Legendre and Gauss-Lobatto quadrature rules for
//! triangles, quadrilaterals, tetrahedra, and hexahedra.

mod gauss;
mod rules;

pub use gauss::*;
pub use rules::*;
