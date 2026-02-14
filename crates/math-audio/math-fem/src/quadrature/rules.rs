//! Quadrature rule selection based on element type and polynomial degree

use super::gauss::{
    QuadraturePoint, gauss_hexahedron, gauss_quadrilateral, gauss_tetrahedron, gauss_triangle,
};
use crate::mesh::ElementType;

/// Quadrature rule for a specific element type
#[derive(Debug, Clone)]
pub struct QuadratureRule {
    /// Element type this rule is for
    pub element_type: ElementType,
    /// Quadrature order (polynomial degree exactly integrated)
    pub order: usize,
    /// Quadrature points and weights
    pub points: Vec<QuadraturePoint>,
}

impl QuadratureRule {
    /// Create a quadrature rule for the given element type and order
    pub fn new(element_type: ElementType, order: usize) -> Self {
        let points = match element_type {
            ElementType::Triangle => gauss_triangle(order),
            ElementType::Quadrilateral => gauss_quadrilateral(order),
            ElementType::Tetrahedron => gauss_tetrahedron(order),
            ElementType::Hexahedron => gauss_hexahedron(order),
        };

        Self {
            element_type,
            order,
            points,
        }
    }

    /// Number of quadrature points
    pub fn num_points(&self) -> usize {
        self.points.len()
    }

    /// Iterator over quadrature points
    pub fn iter(&self) -> impl Iterator<Item = &QuadraturePoint> {
        self.points.iter()
    }
}

/// Determine minimum quadrature order needed for given polynomial degree
///
/// For FEM with polynomial degree p:
/// - Stiffness matrix: integrate grad(phi) * grad(phi) ~ degree 2(p-1)
/// - Mass matrix: integrate phi * phi ~ degree 2p
///
/// Gauss quadrature of order n integrates polynomials of degree 2n-1 exactly
pub fn required_order_for_stiffness(polynomial_degree: usize) -> usize {
    // Stiffness: derivatives reduce degree by 1, so integrate degree 2(p-1) = 2p - 2
    // Need n points where 2n - 1 >= 2p - 2, so n >= p - 0.5, so n = p
    polynomial_degree.max(1)
}

pub fn required_order_for_mass(polynomial_degree: usize) -> usize {
    // Mass: integrate degree 2p
    // Need n points where 2n - 1 >= 2p, so n >= p + 0.5, so n = p + 1
    polynomial_degree + 1
}

/// Create quadrature rule for stiffness matrix integration
pub fn for_stiffness(element_type: ElementType, polynomial_degree: usize) -> QuadratureRule {
    QuadratureRule::new(
        element_type,
        required_order_for_stiffness(polynomial_degree),
    )
}

/// Create quadrature rule for mass matrix integration
pub fn for_mass(element_type: ElementType, polynomial_degree: usize) -> QuadratureRule {
    QuadratureRule::new(element_type, required_order_for_mass(polynomial_degree))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quadrature_rule_creation() {
        let rule = QuadratureRule::new(ElementType::Triangle, 2);
        assert_eq!(rule.element_type, ElementType::Triangle);
        assert_eq!(rule.order, 2);
        assert!(!rule.points.is_empty());
    }

    #[test]
    fn test_required_orders() {
        // P1 elements
        assert_eq!(required_order_for_stiffness(1), 1);
        assert_eq!(required_order_for_mass(1), 2);

        // P2 elements
        assert_eq!(required_order_for_stiffness(2), 2);
        assert_eq!(required_order_for_mass(2), 3);

        // P3 elements
        assert_eq!(required_order_for_stiffness(3), 3);
        assert_eq!(required_order_for_mass(3), 4);
    }

    #[test]
    fn test_for_stiffness_and_mass() {
        let stiffness_rule = for_stiffness(ElementType::Triangle, 2);
        let mass_rule = for_mass(ElementType::Triangle, 2);

        // Mass matrix needs higher order
        assert!(mass_rule.num_points() >= stiffness_rule.num_points());
    }
}
