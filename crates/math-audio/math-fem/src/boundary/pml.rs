//! Perfectly Matched Layer (PML) for absorbing boundaries
//!
//! Implements coordinate stretching for absorbing outgoing waves.
//! The PML modifies the Helmholtz equation by introducing complex-valued
//! coordinate stretching functions.

use crate::mesh::Point;
use num_complex::Complex64;

/// PML stretching direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PmlDirection {
    X,
    Y,
    Z,
    XY,
    XZ,
    YZ,
    XYZ,
}

/// PML stretching profile
#[derive(Debug, Clone, Copy)]
pub enum PmlProfile {
    /// Polynomial profile: σ(s) = σ_max * (s/d)^n
    Polynomial { power: usize },
    /// Constant profile: σ(s) = σ_max
    Constant,
}

impl Default for PmlProfile {
    fn default() -> Self {
        PmlProfile::Polynomial { power: 2 }
    }
}

/// PML region definition
#[derive(Debug, Clone)]
pub struct PmlRegion {
    /// Direction of absorption
    pub direction: PmlDirection,
    /// Start of PML layer (inner boundary)
    pub start: [f64; 3],
    /// End of PML layer (outer boundary)
    pub end: [f64; 3],
    /// Maximum absorption coefficient σ_max
    pub sigma_max: f64,
    /// Profile type
    pub profile: PmlProfile,
    /// Wavenumber for scaling
    pub wavenumber: f64,
}

impl PmlRegion {
    /// Create a new PML region
    pub fn new(
        direction: PmlDirection,
        start: [f64; 3],
        end: [f64; 3],
        sigma_max: f64,
        wavenumber: f64,
    ) -> Self {
        Self {
            direction,
            start,
            end,
            sigma_max,
            profile: PmlProfile::default(),
            wavenumber,
        }
    }

    /// Create PML in +X direction starting at x = x_start
    pub fn x_positive(x_start: f64, thickness: f64, sigma_max: f64, wavenumber: f64) -> Self {
        Self::new(
            PmlDirection::X,
            [x_start, f64::NEG_INFINITY, f64::NEG_INFINITY],
            [x_start + thickness, f64::INFINITY, f64::INFINITY],
            sigma_max,
            wavenumber,
        )
    }

    /// Create PML in -X direction ending at x = x_end
    pub fn x_negative(x_end: f64, thickness: f64, sigma_max: f64, wavenumber: f64) -> Self {
        Self::new(
            PmlDirection::X,
            [x_end - thickness, f64::NEG_INFINITY, f64::NEG_INFINITY],
            [x_end, f64::INFINITY, f64::INFINITY],
            sigma_max,
            wavenumber,
        )
    }

    /// Create PML in +Y direction
    pub fn y_positive(y_start: f64, thickness: f64, sigma_max: f64, wavenumber: f64) -> Self {
        Self::new(
            PmlDirection::Y,
            [f64::NEG_INFINITY, y_start, f64::NEG_INFINITY],
            [f64::INFINITY, y_start + thickness, f64::INFINITY],
            sigma_max,
            wavenumber,
        )
    }

    /// Create PML in -Y direction
    pub fn y_negative(y_end: f64, thickness: f64, sigma_max: f64, wavenumber: f64) -> Self {
        Self::new(
            PmlDirection::Y,
            [f64::NEG_INFINITY, y_end - thickness, f64::NEG_INFINITY],
            [f64::INFINITY, y_end, f64::INFINITY],
            sigma_max,
            wavenumber,
        )
    }

    /// Check if a point is inside this PML region
    pub fn contains(&self, point: &Point) -> bool {
        let in_x = point.x >= self.start[0] && point.x <= self.end[0];
        let in_y = point.y >= self.start[1] && point.y <= self.end[1];
        let in_z = point.z >= self.start[2] && point.z <= self.end[2];

        match self.direction {
            PmlDirection::X => in_x,
            PmlDirection::Y => in_y,
            PmlDirection::Z => in_z,
            PmlDirection::XY => in_x || in_y,
            PmlDirection::XZ => in_x || in_z,
            PmlDirection::YZ => in_y || in_z,
            PmlDirection::XYZ => in_x || in_y || in_z,
        }
    }

    /// Compute stretching function s_x(x) = 1 + iσ(x)/(ωε₀)
    /// Simplified: s_x(x) = 1 + iσ(x)/k
    pub fn stretching_x(&self, x: f64) -> Complex64 {
        if !matches!(
            self.direction,
            PmlDirection::X | PmlDirection::XY | PmlDirection::XZ | PmlDirection::XYZ
        ) {
            return Complex64::new(1.0, 0.0);
        }

        let sigma = self.sigma_at_position(x, 0);
        Complex64::new(1.0, sigma / self.wavenumber)
    }

    /// Compute stretching function s_y(y)
    pub fn stretching_y(&self, y: f64) -> Complex64 {
        if !matches!(
            self.direction,
            PmlDirection::Y | PmlDirection::XY | PmlDirection::YZ | PmlDirection::XYZ
        ) {
            return Complex64::new(1.0, 0.0);
        }

        let sigma = self.sigma_at_position(y, 1);
        Complex64::new(1.0, sigma / self.wavenumber)
    }

    /// Compute stretching function s_z(z)
    pub fn stretching_z(&self, z: f64) -> Complex64 {
        if !matches!(
            self.direction,
            PmlDirection::Z | PmlDirection::XZ | PmlDirection::YZ | PmlDirection::XYZ
        ) {
            return Complex64::new(1.0, 0.0);
        }

        let sigma = self.sigma_at_position(z, 2);
        Complex64::new(1.0, sigma / self.wavenumber)
    }

    /// Compute σ at a given position in the specified dimension
    fn sigma_at_position(&self, pos: f64, dim: usize) -> f64 {
        let start = self.start[dim];
        let end = self.end[dim];
        let thickness = (end - start).abs();

        if thickness <= 0.0 {
            return 0.0;
        }

        // Distance from inner boundary
        let dist = if end > start {
            (pos - start).max(0.0).min(thickness)
        } else {
            (start - pos).max(0.0).min(thickness)
        };

        let normalized = dist / thickness;

        match self.profile {
            PmlProfile::Polynomial { power } => self.sigma_max * normalized.powi(power as i32),
            PmlProfile::Constant => {
                if normalized > 0.0 {
                    self.sigma_max
                } else {
                    0.0
                }
            }
        }
    }

    /// Compute the Jacobian-like factor for the PML transformation
    /// This modifies the material properties in the PML region
    pub fn pml_factor(&self, point: &Point) -> Complex64 {
        let sx = self.stretching_x(point.x);
        let sy = self.stretching_y(point.y);
        let sz = self.stretching_z(point.z);

        sx * sy * sz
    }

    /// Compute modified coefficient for stiffness term in x direction
    /// The Laplacian term ∂²/∂x² becomes (1/s_x) ∂/∂x (1/s_x ∂/∂x)
    pub fn stiffness_coefficient_x(&self, point: &Point) -> Complex64 {
        let sx = self.stretching_x(point.x);
        let sy = self.stretching_y(point.y);
        let sz = self.stretching_z(point.z);

        // For ∂/∂x terms: multiply by sy*sz/sx
        sy * sz / sx
    }

    /// Compute modified coefficient for stiffness term in y direction
    pub fn stiffness_coefficient_y(&self, point: &Point) -> Complex64 {
        let sx = self.stretching_x(point.x);
        let sy = self.stretching_y(point.y);
        let sz = self.stretching_z(point.z);

        sx * sz / sy
    }

    /// Compute modified coefficient for stiffness term in z direction
    pub fn stiffness_coefficient_z(&self, point: &Point) -> Complex64 {
        let sx = self.stretching_x(point.x);
        let sy = self.stretching_y(point.y);
        let sz = self.stretching_z(point.z);

        sx * sy / sz
    }

    /// Compute modified coefficient for mass term
    /// The mass term is multiplied by sx*sy*sz
    pub fn mass_coefficient(&self, point: &Point) -> Complex64 {
        self.pml_factor(point)
    }
}

/// Optimal σ_max for a given polynomial degree and elements per wavelength
///
/// σ_max ≈ (n+1) / (2 * d * √ε_r) * ln(1/R)
/// where n is polynomial power, d is PML thickness, R is desired reflection
pub fn optimal_sigma_max(
    polynomial_power: usize,
    thickness: f64,
    wavenumber: f64,
    target_reflection: f64,
) -> f64 {
    let ln_r = -target_reflection.ln();
    (polynomial_power + 1) as f64 * ln_r / (2.0 * thickness * wavenumber)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pml_region_creation() {
        let pml = PmlRegion::x_positive(1.0, 0.5, 10.0, 1.0);
        assert_eq!(pml.direction, PmlDirection::X);
        assert_eq!(pml.start[0], 1.0);
        assert_eq!(pml.end[0], 1.5);
    }

    #[test]
    fn test_pml_contains() {
        let pml = PmlRegion::x_positive(1.0, 0.5, 10.0, 1.0);

        assert!(!pml.contains(&Point::new_2d(0.5, 0.0)));
        assert!(pml.contains(&Point::new_2d(1.2, 0.0)));
        assert!(pml.contains(&Point::new_2d(1.5, 0.0)));
        assert!(!pml.contains(&Point::new_2d(1.6, 0.0)));
    }

    #[test]
    fn test_pml_stretching() {
        let pml = PmlRegion::x_positive(1.0, 0.5, 10.0, 1.0);

        // At inner boundary, stretching should be 1
        let s_inner = pml.stretching_x(1.0);
        assert!((s_inner.re - 1.0).abs() < 1e-10);
        assert!(s_inner.im.abs() < 1e-10);

        // At outer boundary, stretching should have maximum imaginary part
        let s_outer = pml.stretching_x(1.5);
        assert!((s_outer.re - 1.0).abs() < 1e-10);
        assert!(s_outer.im > 0.0);
    }

    #[test]
    fn test_pml_coefficient_unity_outside() {
        let pml = PmlRegion::x_positive(1.0, 0.5, 10.0, 1.0);
        let point = Point::new_2d(0.5, 0.0); // Outside PML

        let factor = pml.pml_factor(&point);
        assert!((factor.re - 1.0).abs() < 1e-10);
        assert!(factor.im.abs() < 1e-10);
    }

    #[test]
    fn test_optimal_sigma() {
        // For polynomial power 2, thickness 0.5, k=1, R=1e-6
        let sigma = optimal_sigma_max(2, 0.5, 1.0, 1e-6);
        assert!(sigma > 0.0);
        assert!(sigma < 100.0); // Reasonable bound
    }
}
