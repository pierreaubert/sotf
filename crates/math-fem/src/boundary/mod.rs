//! Boundary condition handling for finite element problems
//!
//! Provides implementations for:
//! - Dirichlet (essential) boundary conditions
//! - Neumann (natural) boundary conditions
//! - Robin (mixed) boundary conditions
//! - PML (Perfectly Matched Layer) for absorbing boundaries

mod dirichlet;
mod neumann;
mod pml;
mod robin;

pub use dirichlet::*;
pub use neumann::*;
pub use pml::*;
pub use robin::*;

use crate::mesh::BoundaryType;
use num_complex::Complex64;

/// Boundary condition specification
#[derive(Debug, Clone)]
pub enum BoundaryCondition {
    /// Dirichlet: u = g on boundary
    Dirichlet(DirichletBC),
    /// Neumann: ∂u/∂n = h on boundary
    Neumann(NeumannBC),
    /// Robin: ∂u/∂n + αu = g on boundary
    Robin(RobinBC),
    /// PML: Absorbing layer with complex stretching
    Pml(PmlRegion),
}

impl BoundaryCondition {
    /// Get the boundary type for mesh marking
    pub fn boundary_type(&self) -> BoundaryType {
        match self {
            BoundaryCondition::Dirichlet(_) => BoundaryType::Dirichlet,
            BoundaryCondition::Neumann(_) => BoundaryType::Neumann,
            BoundaryCondition::Robin(_) => BoundaryType::Robin,
            BoundaryCondition::Pml(_) => BoundaryType::PML,
        }
    }
}

/// A collection of boundary conditions for a problem
#[derive(Debug, Clone, Default)]
pub struct BoundaryConditions {
    /// List of boundary conditions
    pub conditions: Vec<BoundaryCondition>,
}

impl BoundaryConditions {
    pub fn new() -> Self {
        Self {
            conditions: Vec::new(),
        }
    }

    /// Add a Dirichlet boundary condition
    pub fn add_dirichlet<F>(&mut self, tag: usize, value_fn: F)
    where
        F: Fn(f64, f64, f64) -> Complex64 + 'static,
    {
        self.conditions
            .push(BoundaryCondition::Dirichlet(DirichletBC::new(
                tag, value_fn,
            )));
    }

    /// Add a Neumann boundary condition
    pub fn add_neumann<F>(&mut self, tag: usize, flux_fn: F)
    where
        F: Fn(f64, f64, f64) -> Complex64 + 'static,
    {
        self.conditions
            .push(BoundaryCondition::Neumann(NeumannBC::new(tag, flux_fn)));
    }

    /// Add a Robin boundary condition
    pub fn add_robin<F, G>(&mut self, tag: usize, alpha_fn: F, g_fn: G)
    where
        F: Fn(f64, f64, f64) -> Complex64 + 'static,
        G: Fn(f64, f64, f64) -> Complex64 + 'static,
    {
        self.conditions
            .push(BoundaryCondition::Robin(RobinBC::new(tag, alpha_fn, g_fn)));
    }

    /// Add a PML region
    pub fn add_pml(&mut self, region: PmlRegion) {
        self.conditions.push(BoundaryCondition::Pml(region));
    }

    /// Get Dirichlet conditions
    pub fn dirichlet_conditions(&self) -> impl Iterator<Item = &DirichletBC> {
        self.conditions.iter().filter_map(|bc| match bc {
            BoundaryCondition::Dirichlet(d) => Some(d),
            _ => None,
        })
    }

    /// Get Neumann conditions
    pub fn neumann_conditions(&self) -> impl Iterator<Item = &NeumannBC> {
        self.conditions.iter().filter_map(|bc| match bc {
            BoundaryCondition::Neumann(n) => Some(n),
            _ => None,
        })
    }

    /// Get Robin conditions
    pub fn robin_conditions(&self) -> impl Iterator<Item = &RobinBC> {
        self.conditions.iter().filter_map(|bc| match bc {
            BoundaryCondition::Robin(r) => Some(r),
            _ => None,
        })
    }

    /// Get PML regions
    pub fn pml_regions(&self) -> impl Iterator<Item = &PmlRegion> {
        self.conditions.iter().filter_map(|bc| match bc {
            BoundaryCondition::Pml(p) => Some(p),
            _ => None,
        })
    }
}
