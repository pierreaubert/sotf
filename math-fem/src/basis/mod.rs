//! Finite element basis functions
//!
//! Implements Lagrange polynomial basis functions for various element types
//! and polynomial degrees (P1, P2, P3 for simplices; Q1, Q2 for quads/hexes).

mod lagrange;
mod shape;

pub use lagrange::*;
pub use shape::*;
