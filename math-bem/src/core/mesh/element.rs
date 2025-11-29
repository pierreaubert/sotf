//! Element shape functions and geometry computations
//!
//! Direct port of shape function computations from NC_3dFunctions.cpp.

use ndarray::{Array1, Array2, array};

use crate::core::types::{Element, ElementType};

/// Shape functions for triangular and quadrilateral elements
impl Element {
    /// Compute shape functions at local coordinates (s, t)
    ///
    /// For triangles: Area coordinates with standard vertex mapping:
    /// - (0,0) -> node 0
    /// - (1,0) -> node 1
    /// - (0,1) -> node 2
    ///
    /// For quads: Bilinear functions on [-1,1]²
    pub fn shape_functions(&self, s: f64, t: f64) -> Array1<f64> {
        match self.element_type {
            ElementType::Tri3 => {
                // Triangle shape functions:
                // N0 = 1-s-t (vertex at (0,0))
                // N1 = s (vertex at (1,0))
                // N2 = t (vertex at (0,1))
                array![1.0 - s - t, s, t]
            }
            ElementType::Quad4 => {
                // Quad: bilinear on [-1,1]²
                let s1 = 0.25 * (s + 1.0);
                let s2 = 0.25 * (s - 1.0);
                let t1 = t + 1.0;
                let t2 = t - 1.0;
                array![s1 * t1, -s2 * t1, s2 * t2, -s1 * t2]
            }
        }
    }

    /// Compute shape function derivatives dN/ds and dN/dt
    pub fn shape_derivatives(&self, s: f64, t: f64) -> (Array1<f64>, Array1<f64>) {
        match self.element_type {
            ElementType::Tri3 => {
                // dN/ds = [-1, 1, 0], dN/dt = [-1, 0, 1]
                (array![-1.0, 1.0, 0.0], array![-1.0, 0.0, 1.0])
            }
            ElementType::Quad4 => {
                // dN/ds
                let dns = array![
                    0.25 * (t + 1.0),
                    -0.25 * (t + 1.0),
                    0.25 * (t - 1.0),
                    -0.25 * (t - 1.0)
                ];
                // dN/dt
                let dnt = array![
                    0.25 * (s + 1.0),
                    0.25 * (1.0 - s),
                    0.25 * (s - 1.0),
                    -0.25 * (s + 1.0)
                ];
                (dns, dnt)
            }
        }
    }

    /// Compute global coordinates from local coordinates (s, t)
    pub fn local_to_global(&self, s: f64, t: f64, nodes: &Array2<f64>) -> Array1<f64> {
        let n = self.shape_functions(s, t);
        let mut global = Array1::zeros(3);

        for (i, &node_idx) in self.connectivity.iter().enumerate() {
            for j in 0..3 {
                global[j] += n[i] * nodes[[node_idx, j]];
            }
        }

        global
    }

    /// Compute unit normal vector and Jacobian at local coordinates (s, t)
    ///
    /// Returns (normal, jacobian) where jacobian is the surface element dS.
    pub fn normal_and_jacobian(&self, s: f64, t: f64, nodes: &Array2<f64>) -> (Array1<f64>, f64) {
        let (dns, dnt) = self.shape_derivatives(s, t);

        // Compute dx/ds and dx/dt
        let mut dxds = Array1::zeros(3);
        let mut dxdt = Array1::zeros(3);

        for (i, &node_idx) in self.connectivity.iter().enumerate() {
            for j in 0..3 {
                dxds[j] += dns[i] * nodes[[node_idx, j]];
                dxdt[j] += dnt[i] * nodes[[node_idx, j]];
            }
        }

        // Normal = dxds × dxdt
        let normal = cross_product(&dxds, &dxdt);
        let length = normal.dot(&normal).sqrt();

        if length > 1e-15 {
            (normal / length, length)
        } else {
            (Array1::zeros(3), 0.0)
        }
    }
}

/// Cross product of two 3D vectors
pub fn cross_product(a: &Array1<f64>, b: &Array1<f64>) -> Array1<f64> {
    array![
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0]
    ]
}

/// Dot product of two 3D vectors
pub fn dot_product(a: &Array1<f64>, b: &Array1<f64>) -> f64 {
    a.dot(b)
}

/// Normalize a 3D vector
pub fn normalize(v: &Array1<f64>) -> (Array1<f64>, f64) {
    let len = v.dot(v).sqrt();
    if len > 1e-15 {
        (v / len, len)
    } else {
        (Array1::zeros(3), 0.0)
    }
}

/// Compute element center (centroid)
pub fn compute_element_center(nodes: &Array2<f64>, connectivity: &[usize]) -> Array1<f64> {
    let n = connectivity.len();
    let mut center = Array1::zeros(3);

    for &node_idx in connectivity {
        for j in 0..3 {
            center[j] += nodes[[node_idx, j]];
        }
    }

    center / n as f64
}

/// Compute element area
pub fn compute_element_area(
    nodes: &Array2<f64>,
    connectivity: &[usize],
    element_type: ElementType,
) -> f64 {
    match element_type {
        ElementType::Tri3 => {
            // Triangle area = 0.5 * |v1 × v2|
            let p0 = nodes.row(connectivity[0]);
            let p1 = nodes.row(connectivity[1]);
            let p2 = nodes.row(connectivity[2]);

            let v1 = array![p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
            let v2 = array![p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];

            let cross = cross_product(&v1, &v2);
            0.5 * cross.dot(&cross).sqrt()
        }
        ElementType::Quad4 => {
            // Quad area via two triangles
            let p0 = nodes.row(connectivity[0]);
            let p1 = nodes.row(connectivity[1]);
            let p2 = nodes.row(connectivity[2]);
            let p3 = nodes.row(connectivity[3]);

            // Triangle 0-1-2
            let v1 = array![p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
            let v2 = array![p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
            let cross1 = cross_product(&v1, &v2);
            let area1 = 0.5 * cross1.dot(&cross1).sqrt();

            // Triangle 0-2-3
            let v3 = array![p3[0] - p0[0], p3[1] - p0[1], p3[2] - p0[2]];
            let cross2 = cross_product(&v2, &v3);
            let area2 = 0.5 * cross2.dot(&cross2).sqrt();

            area1 + area2
        }
    }
}

/// Compute element normal at center
pub fn compute_element_normal(
    nodes: &Array2<f64>,
    connectivity: &[usize],
    element_type: ElementType,
) -> Array1<f64> {
    match element_type {
        ElementType::Tri3 => {
            let p0 = nodes.row(connectivity[0]);
            let p1 = nodes.row(connectivity[1]);
            let p2 = nodes.row(connectivity[2]);

            let v1 = array![p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
            let v2 = array![p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];

            let cross = cross_product(&v1, &v2);
            let (normal, _) = normalize(&cross);
            normal
        }
        ElementType::Quad4 => {
            // Use diagonals for quad
            let p0 = nodes.row(connectivity[0]);
            let p2 = nodes.row(connectivity[2]);
            let p1 = nodes.row(connectivity[1]);
            let p3 = nodes.row(connectivity[3]);

            let d1 = array![p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
            let d2 = array![p3[0] - p1[0], p3[1] - p1[1], p3[2] - p1[2]];

            let cross = cross_product(&d1, &d2);
            let (normal, _) = normalize(&cross);
            normal
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{BoundaryCondition, ElementProperty};
    use num_complex::Complex64;

    fn make_test_triangle() -> (Element, Array2<f64>) {
        let nodes =
            Array2::from_shape_vec((3, 3), vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0])
                .unwrap();

        let element = Element {
            connectivity: vec![0, 1, 2],
            element_type: ElementType::Tri3,
            property: ElementProperty::Surface,
            normal: array![0.0, 0.0, 1.0],
            node_normals: Array2::zeros((3, 3)),
            center: array![1.0 / 3.0, 1.0 / 3.0, 0.0],
            area: 0.5,
            boundary_condition: BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]),
            group: 0,
            dof_addresses: vec![0],
        };

        (element, nodes)
    }

    #[test]
    fn test_triangle_shape_functions() {
        let (element, _nodes) = make_test_triangle();

        // At vertex 0 (s=0, t=0) -> N = [1, 0, 0]
        let n = element.shape_functions(0.0, 0.0);
        assert!((n[0] - 1.0).abs() < 1e-10);
        assert!(n[1].abs() < 1e-10);
        assert!(n[2].abs() < 1e-10);

        // At vertex 1 (s=1, t=0) -> N = [0, 1, 0]
        let n = element.shape_functions(1.0, 0.0);
        assert!(n[0].abs() < 1e-10);
        assert!((n[1] - 1.0).abs() < 1e-10);
        assert!(n[2].abs() < 1e-10);

        // At center (s=1/3, t=1/3) -> N = [1/3, 1/3, 1/3]
        let n = element.shape_functions(1.0 / 3.0, 1.0 / 3.0);
        assert!((n[0] - 1.0 / 3.0).abs() < 1e-10);
        assert!((n[1] - 1.0 / 3.0).abs() < 1e-10);
        assert!((n[2] - 1.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_triangle_local_to_global() {
        let (element, nodes) = make_test_triangle();

        // At vertex 0 (s=0, t=0) -> node 0 at (0,0,0)
        let p = element.local_to_global(0.0, 0.0, &nodes);
        assert!((p[0] - 0.0).abs() < 1e-10);
        assert!((p[1] - 0.0).abs() < 1e-10);

        // At vertex 1 (s=1, t=0) -> node 1 at (1,0,0)
        let p = element.local_to_global(1.0, 0.0, &nodes);
        assert!((p[0] - 1.0).abs() < 1e-10);
        assert!((p[1] - 0.0).abs() < 1e-10);

        // At vertex 2 (s=0, t=1) -> node 2 at (0,1,0)
        let p = element.local_to_global(0.0, 1.0, &nodes);
        assert!((p[0] - 0.0).abs() < 1e-10);
        assert!((p[1] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_triangle_normal() {
        let (element, nodes) = make_test_triangle();

        let (normal, jacobian) = element.normal_and_jacobian(0.5, 0.25, &nodes);

        // Normal should be (0, 0, 1) for this XY-plane triangle
        assert!((normal[0]).abs() < 1e-10);
        assert!((normal[1]).abs() < 1e-10);
        assert!((normal[2] - 1.0).abs() < 1e-10);

        // Jacobian should be constant for linear triangle = 2 * area = 1
        assert!((jacobian - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_element_area() {
        let nodes =
            Array2::from_shape_vec((3, 3), vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0])
                .unwrap();

        let area = compute_element_area(&nodes, &[0, 1, 2], ElementType::Tri3);
        assert!((area - 0.5).abs() < 1e-10);
    }
}
