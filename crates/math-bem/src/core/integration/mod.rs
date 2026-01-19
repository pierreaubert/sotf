//! Numerical integration for BEM
//!
//! Provides Gauss-Legendre quadrature for regular and singular element integration.
//!
//! ## Module Organization
//!
//! - [`gauss`] - Gauss-Legendre quadrature rules for 1D, triangles, quads, and spheres
//! - [`singular`] - Singular integration for self-element (when source = field element)
//! - [`regular`] - Regular integration for non-self elements with adaptive subdivision

pub mod gauss;
pub mod regular;
pub mod singular;

pub use gauss::*;
pub use regular::{
    HIGH_ACCURACY_THRESHOLD, QUASI_SINGULAR_THRESHOLD, integrate_g_only, integrate_h_only,
    optimal_quadrature_order, quasi_singular_integration, regular_integration,
    regular_integration_fixed_order,
};
pub use singular::{
    MAX_SUBELEMENTS, QuadratureParams, Subelement, generate_subelements, singular_integration,
    singular_integration_with_params,
};
