//! Lagrange basis functions for finite elements
//!
//! Provides P1, P2, P3 (simplex) and Q1, Q2 (tensor-product) basis functions.

use crate::mesh::ElementType;

/// Polynomial degree for Lagrange elements
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolynomialDegree {
    /// Linear (P1/Q1)
    P1,
    /// Quadratic (P2/Q2)
    P2,
    /// Cubic (P3/Q3)
    P3,
}

impl PolynomialDegree {
    pub fn degree(&self) -> usize {
        match self {
            PolynomialDegree::P1 => 1,
            PolynomialDegree::P2 => 2,
            PolynomialDegree::P3 => 3,
        }
    }
}

/// Number of nodes for a given element type and polynomial degree
pub fn num_nodes(element_type: ElementType, degree: PolynomialDegree) -> usize {
    match (element_type, degree) {
        // Triangles: (p+1)(p+2)/2
        (ElementType::Triangle, PolynomialDegree::P1) => 3,
        (ElementType::Triangle, PolynomialDegree::P2) => 6,
        (ElementType::Triangle, PolynomialDegree::P3) => 10,

        // Quadrilaterals: (p+1)^2
        (ElementType::Quadrilateral, PolynomialDegree::P1) => 4,
        (ElementType::Quadrilateral, PolynomialDegree::P2) => 9,
        (ElementType::Quadrilateral, PolynomialDegree::P3) => 16,

        // Tetrahedra: (p+1)(p+2)(p+3)/6
        (ElementType::Tetrahedron, PolynomialDegree::P1) => 4,
        (ElementType::Tetrahedron, PolynomialDegree::P2) => 10,
        (ElementType::Tetrahedron, PolynomialDegree::P3) => 20,

        // Hexahedra: (p+1)^3
        (ElementType::Hexahedron, PolynomialDegree::P1) => 8,
        (ElementType::Hexahedron, PolynomialDegree::P2) => 27,
        (ElementType::Hexahedron, PolynomialDegree::P3) => 64,
    }
}

/// Evaluate P1 triangle basis functions at (xi, eta)
/// Reference triangle: (0,0), (1,0), (0,1)
/// Returns [N0, N1, N2]
pub fn p1_triangle(xi: f64, eta: f64) -> [f64; 3] {
    [1.0 - xi - eta, xi, eta]
}

/// Evaluate P1 triangle basis function gradients (constant)
/// Returns [[dN0/dxi, dN0/deta], [dN1/dxi, dN1/deta], [dN2/dxi, dN2/deta]]
pub fn p1_triangle_grad() -> [[f64; 2]; 3] {
    [[-1.0, -1.0], [1.0, 0.0], [0.0, 1.0]]
}

/// Evaluate P2 triangle basis functions at (xi, eta)
/// Nodes: 3 vertices + 3 edge midpoints
/// Returns [N0, N1, N2, N01, N12, N20]
pub fn p2_triangle(xi: f64, eta: f64) -> [f64; 6] {
    let l0 = 1.0 - xi - eta;
    let l1 = xi;
    let l2 = eta;

    [
        l0 * (2.0 * l0 - 1.0), // vertex 0
        l1 * (2.0 * l1 - 1.0), // vertex 1
        l2 * (2.0 * l2 - 1.0), // vertex 2
        4.0 * l0 * l1,         // edge 0-1
        4.0 * l1 * l2,         // edge 1-2
        4.0 * l2 * l0,         // edge 2-0
    ]
}

/// Evaluate P2 triangle basis function gradients at (xi, eta)
pub fn p2_triangle_grad(xi: f64, eta: f64) -> [[f64; 2]; 6] {
    let l0 = 1.0 - xi - eta;
    let l1 = xi;
    let l2 = eta;

    // dl0/dxi = -1, dl0/deta = -1
    // dl1/dxi = 1,  dl1/deta = 0
    // dl2/dxi = 0,  dl2/deta = 1
    [
        // dN0/d(xi,eta) where N0 = l0*(2*l0-1)
        // dN0/dxi = (2*l0-1)*(-1) + l0*2*(-1) = 1 - 4*l0
        [1.0 - 4.0 * l0, 1.0 - 4.0 * l0],
        // dN1/d(xi,eta) where N1 = l1*(2*l1-1)
        [4.0 * l1 - 1.0, 0.0],
        // dN2/d(xi,eta) where N2 = l2*(2*l2-1)
        [0.0, 4.0 * l2 - 1.0],
        // dN01/d(xi,eta) where N01 = 4*l0*l1
        [4.0 * (l0 - l1), -4.0 * l1],
        // dN12/d(xi,eta) where N12 = 4*l1*l2
        [4.0 * l2, 4.0 * l1],
        // dN20/d(xi,eta) where N20 = 4*l2*l0
        [-4.0 * l2, 4.0 * (l0 - l2)],
    ]
}

/// Evaluate Q1 quadrilateral basis functions at (xi, eta)
/// Reference quad: [-1,1] x [-1,1]
/// Nodes: (−1,−1), (1,−1), (1,1), (−1,1)
pub fn q1_quadrilateral(xi: f64, eta: f64) -> [f64; 4] {
    [
        0.25 * (1.0 - xi) * (1.0 - eta),
        0.25 * (1.0 + xi) * (1.0 - eta),
        0.25 * (1.0 + xi) * (1.0 + eta),
        0.25 * (1.0 - xi) * (1.0 + eta),
    ]
}

/// Evaluate Q1 quadrilateral basis function gradients at (xi, eta)
pub fn q1_quadrilateral_grad(xi: f64, eta: f64) -> [[f64; 2]; 4] {
    [
        [-0.25 * (1.0 - eta), -0.25 * (1.0 - xi)],
        [0.25 * (1.0 - eta), -0.25 * (1.0 + xi)],
        [0.25 * (1.0 + eta), 0.25 * (1.0 + xi)],
        [-0.25 * (1.0 + eta), 0.25 * (1.0 - xi)],
    ]
}

/// Evaluate P1 tetrahedron basis functions at (xi, eta, zeta)
/// Reference tet: (0,0,0), (1,0,0), (0,1,0), (0,0,1)
pub fn p1_tetrahedron(xi: f64, eta: f64, zeta: f64) -> [f64; 4] {
    [1.0 - xi - eta - zeta, xi, eta, zeta]
}

/// Evaluate P1 tetrahedron basis function gradients (constant)
pub fn p1_tetrahedron_grad() -> [[f64; 3]; 4] {
    [
        [-1.0, -1.0, -1.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ]
}

/// Evaluate Q1 hexahedron basis functions at (xi, eta, zeta)
/// Reference hex: [-1,1]^3
pub fn q1_hexahedron(xi: f64, eta: f64, zeta: f64) -> [f64; 8] {
    let xm = 1.0 - xi;
    let xp = 1.0 + xi;
    let em = 1.0 - eta;
    let ep = 1.0 + eta;
    let zm = 1.0 - zeta;
    let zp = 1.0 + zeta;

    [
        0.125 * xm * em * zm,
        0.125 * xp * em * zm,
        0.125 * xp * ep * zm,
        0.125 * xm * ep * zm,
        0.125 * xm * em * zp,
        0.125 * xp * em * zp,
        0.125 * xp * ep * zp,
        0.125 * xm * ep * zp,
    ]
}

/// Evaluate Q1 hexahedron basis function gradients at (xi, eta, zeta)
pub fn q1_hexahedron_grad(xi: f64, eta: f64, zeta: f64) -> [[f64; 3]; 8] {
    let xm = 1.0 - xi;
    let xp = 1.0 + xi;
    let em = 1.0 - eta;
    let ep = 1.0 + eta;
    let zm = 1.0 - zeta;
    let zp = 1.0 + zeta;

    [
        [-0.125 * em * zm, -0.125 * xm * zm, -0.125 * xm * em],
        [0.125 * em * zm, -0.125 * xp * zm, -0.125 * xp * em],
        [0.125 * ep * zm, 0.125 * xp * zm, -0.125 * xp * ep],
        [-0.125 * ep * zm, 0.125 * xm * zm, -0.125 * xm * ep],
        [-0.125 * em * zp, -0.125 * xm * zp, 0.125 * xm * em],
        [0.125 * em * zp, -0.125 * xp * zp, 0.125 * xp * em],
        [0.125 * ep * zp, 0.125 * xp * zp, 0.125 * xp * ep],
        [-0.125 * ep * zp, 0.125 * xm * zp, 0.125 * xm * ep],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_num_nodes() {
        assert_eq!(num_nodes(ElementType::Triangle, PolynomialDegree::P1), 3);
        assert_eq!(num_nodes(ElementType::Triangle, PolynomialDegree::P2), 6);
        assert_eq!(
            num_nodes(ElementType::Quadrilateral, PolynomialDegree::P1),
            4
        );
        assert_eq!(num_nodes(ElementType::Tetrahedron, PolynomialDegree::P1), 4);
        assert_eq!(num_nodes(ElementType::Hexahedron, PolynomialDegree::P1), 8);
    }

    #[test]
    fn test_p1_triangle_partition_of_unity() {
        // Shape functions should sum to 1 at any point
        let (xi, eta) = (0.3, 0.2);
        let n = p1_triangle(xi, eta);
        let sum: f64 = n.iter().sum();
        assert!((sum - 1.0).abs() < 1e-14);
    }

    #[test]
    fn test_p1_triangle_kronecker() {
        // N_i(vertex_j) = delta_ij
        let vertices = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)];
        for (i, &(xi, eta)) in vertices.iter().enumerate() {
            let n = p1_triangle(xi, eta);
            for (j, &nj) in n.iter().enumerate() {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!((nj - expected).abs() < 1e-14);
            }
        }
    }

    #[test]
    fn test_q1_quad_partition_of_unity() {
        let (xi, eta) = (0.5, -0.3);
        let n = q1_quadrilateral(xi, eta);
        let sum: f64 = n.iter().sum();
        assert!((sum - 1.0).abs() < 1e-14);
    }

    #[test]
    fn test_q1_quad_kronecker() {
        let vertices = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
        for (i, &(xi, eta)) in vertices.iter().enumerate() {
            let n = q1_quadrilateral(xi, eta);
            for (j, &nj) in n.iter().enumerate() {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!((nj - expected).abs() < 1e-14);
            }
        }
    }

    #[test]
    fn test_p1_tet_partition_of_unity() {
        let (xi, eta, zeta) = (0.1, 0.2, 0.3);
        let n = p1_tetrahedron(xi, eta, zeta);
        let sum: f64 = n.iter().sum();
        assert!((sum - 1.0).abs() < 1e-14);
    }

    #[test]
    fn test_q1_hex_partition_of_unity() {
        let (xi, eta, zeta) = (0.2, -0.4, 0.6);
        let n = q1_hexahedron(xi, eta, zeta);
        let sum: f64 = n.iter().sum();
        assert!((sum - 1.0).abs() < 1e-14);
    }
}
