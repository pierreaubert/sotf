//! Shape function evaluation for finite elements
//!
//! Provides a unified interface for evaluating basis functions and their
//! gradients at arbitrary points within reference elements.

use super::lagrange::*;
use crate::mesh::ElementType;

/// Shape functions evaluated at a point
#[derive(Debug, Clone)]
pub struct ShapeValues {
    /// Basis function values [N_0, N_1, ..., N_n]
    pub values: Vec<f64>,
    /// Basis function gradients [[dN_0/dxi, dN_0/deta, ...], ...]
    pub gradients: Vec<Vec<f64>>,
}

/// Evaluate shape functions for an element at a reference point
pub fn evaluate_shape(
    element_type: ElementType,
    degree: PolynomialDegree,
    xi: f64,
    eta: f64,
    zeta: f64,
) -> ShapeValues {
    match (element_type, degree) {
        (ElementType::Triangle, PolynomialDegree::P1) => {
            let vals = p1_triangle(xi, eta);
            let grads = p1_triangle_grad();
            ShapeValues {
                values: vals.to_vec(),
                gradients: grads.iter().map(|g| g.to_vec()).collect(),
            }
        }
        (ElementType::Triangle, PolynomialDegree::P2) => {
            let vals = p2_triangle(xi, eta);
            let grads = p2_triangle_grad(xi, eta);
            ShapeValues {
                values: vals.to_vec(),
                gradients: grads.iter().map(|g| g.to_vec()).collect(),
            }
        }
        (ElementType::Quadrilateral, PolynomialDegree::P1) => {
            let vals = q1_quadrilateral(xi, eta);
            let grads = q1_quadrilateral_grad(xi, eta);
            ShapeValues {
                values: vals.to_vec(),
                gradients: grads.iter().map(|g| g.to_vec()).collect(),
            }
        }
        (ElementType::Tetrahedron, PolynomialDegree::P1) => {
            let vals = p1_tetrahedron(xi, eta, zeta);
            let grads = p1_tetrahedron_grad();
            ShapeValues {
                values: vals.to_vec(),
                gradients: grads.iter().map(|g| g.to_vec()).collect(),
            }
        }
        (ElementType::Hexahedron, PolynomialDegree::P1) => {
            let vals = q1_hexahedron(xi, eta, zeta);
            let grads = q1_hexahedron_grad(xi, eta, zeta);
            ShapeValues {
                values: vals.to_vec(),
                gradients: grads.iter().map(|g| g.to_vec()).collect(),
            }
        }
        _ => {
            // Higher-order elements not yet implemented
            log::warn!(
                "Shape functions for {:?} degree {:?} not implemented, using P1",
                element_type,
                degree
            );
            evaluate_shape(element_type, PolynomialDegree::P1, xi, eta, zeta)
        }
    }
}

/// Jacobian matrix for coordinate transformation
/// Maps reference element coordinates to physical coordinates
#[derive(Debug, Clone)]
pub struct Jacobian {
    /// Jacobian matrix entries (row-major)
    /// 2D: [[dx/dxi, dx/deta], [dy/dxi, dy/deta]]
    /// 3D: [[dx/dxi, dx/deta, dx/dzeta], ...]
    pub matrix: Vec<Vec<f64>>,
    /// Determinant of Jacobian
    pub det: f64,
    /// Inverse of Jacobian (for gradient transformation)
    pub inverse: Vec<Vec<f64>>,
}

impl Jacobian {
    /// Compute 2D Jacobian from shape function gradients and node coordinates
    pub fn from_2d(grad_ref: &[Vec<f64>], coords: &[[f64; 2]]) -> Self {
        // J = [ dx/dxi   dx/deta ]
        //     [ dy/dxi   dy/deta ]
        let mut j = [[0.0; 2]; 2];

        for (i, g) in grad_ref.iter().enumerate() {
            j[0][0] += g[0] * coords[i][0]; // dx/dxi
            j[0][1] += g[1] * coords[i][0]; // dx/deta
            j[1][0] += g[0] * coords[i][1]; // dy/dxi
            j[1][1] += g[1] * coords[i][1]; // dy/deta
        }

        let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
        let inv_det = 1.0 / det;

        let inverse = vec![
            vec![j[1][1] * inv_det, -j[0][1] * inv_det],
            vec![-j[1][0] * inv_det, j[0][0] * inv_det],
        ];

        Self {
            matrix: vec![vec![j[0][0], j[0][1]], vec![j[1][0], j[1][1]]],
            det,
            inverse,
        }
    }

    /// Compute 3D Jacobian from shape function gradients and node coordinates
    pub fn from_3d(grad_ref: &[Vec<f64>], coords: &[[f64; 3]]) -> Self {
        let mut j = [[0.0; 3]; 3];

        for (i, g) in grad_ref.iter().enumerate() {
            for k in 0..3 {
                j[0][k] += g[k] * coords[i][0]; // dx/d(xi,eta,zeta)
                j[1][k] += g[k] * coords[i][1]; // dy/d(xi,eta,zeta)
                j[2][k] += g[k] * coords[i][2]; // dz/d(xi,eta,zeta)
            }
        }

        // Determinant of 3x3 matrix
        let det = j[0][0] * (j[1][1] * j[2][2] - j[1][2] * j[2][1])
            - j[0][1] * (j[1][0] * j[2][2] - j[1][2] * j[2][0])
            + j[0][2] * (j[1][0] * j[2][1] - j[1][1] * j[2][0]);

        let inv_det = 1.0 / det;

        // Inverse of 3x3 matrix
        let inverse = vec![
            vec![
                (j[1][1] * j[2][2] - j[1][2] * j[2][1]) * inv_det,
                (j[0][2] * j[2][1] - j[0][1] * j[2][2]) * inv_det,
                (j[0][1] * j[1][2] - j[0][2] * j[1][1]) * inv_det,
            ],
            vec![
                (j[1][2] * j[2][0] - j[1][0] * j[2][2]) * inv_det,
                (j[0][0] * j[2][2] - j[0][2] * j[2][0]) * inv_det,
                (j[0][2] * j[1][0] - j[0][0] * j[1][2]) * inv_det,
            ],
            vec![
                (j[1][0] * j[2][1] - j[1][1] * j[2][0]) * inv_det,
                (j[0][1] * j[2][0] - j[0][0] * j[2][1]) * inv_det,
                (j[0][0] * j[1][1] - j[0][1] * j[1][0]) * inv_det,
            ],
        ];

        Self {
            matrix: vec![
                vec![j[0][0], j[0][1], j[0][2]],
                vec![j[1][0], j[1][1], j[1][2]],
                vec![j[2][0], j[2][1], j[2][2]],
            ],
            det,
            inverse,
        }
    }

    /// Transform reference gradients to physical gradients
    /// grad_physical = J^{-T} * grad_ref
    pub fn transform_gradient(&self, grad_ref: &[f64]) -> Vec<f64> {
        let dim = self.inverse.len();
        let mut result = vec![0.0; dim];

        for i in 0..dim {
            for j in 0..dim {
                result[i] += self.inverse[j][i] * grad_ref[j];
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_shape_p1_triangle() {
        let shape = evaluate_shape(ElementType::Triangle, PolynomialDegree::P1, 0.25, 0.25, 0.0);
        assert_eq!(shape.values.len(), 3);
        assert_eq!(shape.gradients.len(), 3);

        // Partition of unity
        let sum: f64 = shape.values.iter().sum();
        assert!((sum - 1.0).abs() < 1e-14);
    }

    #[test]
    fn test_jacobian_2d_unit_triangle() {
        // Unit triangle with vertices (0,0), (1,0), (0,1)
        let coords = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
        let grads = vec![vec![-1.0, -1.0], vec![1.0, 0.0], vec![0.0, 1.0]];

        let jac = Jacobian::from_2d(&grads, &coords);

        // For unit triangle, J should be identity
        assert!((jac.matrix[0][0] - 1.0).abs() < 1e-14);
        assert!(jac.matrix[0][1].abs() < 1e-14);
        assert!(jac.matrix[1][0].abs() < 1e-14);
        assert!((jac.matrix[1][1] - 1.0).abs() < 1e-14);
        assert!((jac.det - 1.0).abs() < 1e-14);
    }

    #[test]
    fn test_jacobian_2d_scaled_triangle() {
        // Scaled triangle with vertices (0,0), (2,0), (0,2)
        let coords = [[0.0, 0.0], [2.0, 0.0], [0.0, 2.0]];
        let grads = vec![vec![-1.0, -1.0], vec![1.0, 0.0], vec![0.0, 1.0]];

        let jac = Jacobian::from_2d(&grads, &coords);

        // Jacobian should be 2*I
        assert!((jac.matrix[0][0] - 2.0).abs() < 1e-14);
        assert!((jac.matrix[1][1] - 2.0).abs() < 1e-14);
        assert!((jac.det - 4.0).abs() < 1e-14);
    }

    #[test]
    fn test_gradient_transformation() {
        // For unit triangle, gradient should be unchanged
        let coords = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
        let grads = vec![vec![-1.0, -1.0], vec![1.0, 0.0], vec![0.0, 1.0]];

        let jac = Jacobian::from_2d(&grads, &coords);
        let ref_grad = vec![1.0, 2.0];
        let phys_grad = jac.transform_gradient(&ref_grad);

        assert!((phys_grad[0] - 1.0).abs() < 1e-14);
        assert!((phys_grad[1] - 2.0).abs() < 1e-14);
    }
}
