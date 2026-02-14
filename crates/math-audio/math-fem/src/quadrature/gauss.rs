//! Gauss-Legendre quadrature points and weights
//!
//! Pre-computed points and weights for 1D, 2D (triangle, quad), and 3D (tet, hex) integration.

/// A single quadrature point with weight
#[derive(Debug, Clone, Copy)]
pub struct QuadraturePoint {
    /// Reference coordinates (xi, eta, zeta)
    pub coords: [f64; 3],
    /// Integration weight
    pub weight: f64,
}

impl QuadraturePoint {
    pub fn new_1d(xi: f64, weight: f64) -> Self {
        Self {
            coords: [xi, 0.0, 0.0],
            weight,
        }
    }

    pub fn new_2d(xi: f64, eta: f64, weight: f64) -> Self {
        Self {
            coords: [xi, eta, 0.0],
            weight,
        }
    }

    pub fn new_3d(xi: f64, eta: f64, zeta: f64, weight: f64) -> Self {
        Self {
            coords: [xi, eta, zeta],
            weight,
        }
    }

    #[inline]
    pub fn xi(&self) -> f64 {
        self.coords[0]
    }

    #[inline]
    pub fn eta(&self) -> f64 {
        self.coords[1]
    }

    #[inline]
    pub fn zeta(&self) -> f64 {
        self.coords[2]
    }
}

/// 1D Gauss-Legendre quadrature on [-1, 1]
pub fn gauss_legendre_1d(order: usize) -> Vec<QuadraturePoint> {
    match order {
        1 => vec![QuadraturePoint::new_1d(0.0, 2.0)],
        2 => {
            let x = 1.0 / 3.0_f64.sqrt();
            vec![
                QuadraturePoint::new_1d(-x, 1.0),
                QuadraturePoint::new_1d(x, 1.0),
            ]
        }
        3 => {
            let x = (3.0 / 5.0_f64).sqrt();
            vec![
                QuadraturePoint::new_1d(-x, 5.0 / 9.0),
                QuadraturePoint::new_1d(0.0, 8.0 / 9.0),
                QuadraturePoint::new_1d(x, 5.0 / 9.0),
            ]
        }
        4 => {
            let a = (3.0 / 7.0 - 2.0 / 7.0 * (6.0 / 5.0_f64).sqrt()).sqrt();
            let b = (3.0 / 7.0 + 2.0 / 7.0 * (6.0 / 5.0_f64).sqrt()).sqrt();
            let wa = (18.0 + 30.0_f64.sqrt()) / 36.0;
            let wb = (18.0 - 30.0_f64.sqrt()) / 36.0;
            vec![
                QuadraturePoint::new_1d(-b, wb),
                QuadraturePoint::new_1d(-a, wa),
                QuadraturePoint::new_1d(a, wa),
                QuadraturePoint::new_1d(b, wb),
            ]
        }
        5 => {
            let a = (5.0 - 2.0 * (10.0 / 7.0_f64).sqrt()).sqrt() / 3.0;
            let b = (5.0 + 2.0 * (10.0 / 7.0_f64).sqrt()).sqrt() / 3.0;
            let wa = (322.0 + 13.0 * 70.0_f64.sqrt()) / 900.0;
            let wb = (322.0 - 13.0 * 70.0_f64.sqrt()) / 900.0;
            vec![
                QuadraturePoint::new_1d(-b, wb),
                QuadraturePoint::new_1d(-a, wa),
                QuadraturePoint::new_1d(0.0, 128.0 / 225.0),
                QuadraturePoint::new_1d(a, wa),
                QuadraturePoint::new_1d(b, wb),
            ]
        }
        _ => {
            // For higher orders, use order 5
            gauss_legendre_1d(5)
        }
    }
}

/// Gauss quadrature for triangles (area coordinates)
/// Reference triangle: (0,0), (1,0), (0,1)
pub fn gauss_triangle(order: usize) -> Vec<QuadraturePoint> {
    match order {
        1 => {
            // 1-point rule (degree 1)
            vec![QuadraturePoint::new_2d(1.0 / 3.0, 1.0 / 3.0, 0.5)]
        }
        2 | 3 => {
            // 3-point rule (degree 2)
            let a = 1.0 / 6.0;
            let b = 2.0 / 3.0;
            let w = 1.0 / 6.0;
            vec![
                QuadraturePoint::new_2d(a, a, w),
                QuadraturePoint::new_2d(b, a, w),
                QuadraturePoint::new_2d(a, b, w),
            ]
        }
        4 | 5 => {
            // 7-point rule (degree 5)
            let a1 = 1.0 / 3.0;
            let w1 = 9.0 / 80.0;

            let a2 = (6.0 - 15.0_f64.sqrt()) / 21.0;
            let b2 = (9.0 + 2.0 * 15.0_f64.sqrt()) / 21.0;
            let w2 = (155.0 - 15.0_f64.sqrt()) / 2400.0;

            let a3 = (6.0 + 15.0_f64.sqrt()) / 21.0;
            let b3 = (9.0 - 2.0 * 15.0_f64.sqrt()) / 21.0;
            let w3 = (155.0 + 15.0_f64.sqrt()) / 2400.0;

            vec![
                QuadraturePoint::new_2d(a1, a1, w1),
                QuadraturePoint::new_2d(a2, a2, w2),
                QuadraturePoint::new_2d(b2, a2, w2),
                QuadraturePoint::new_2d(a2, b2, w2),
                QuadraturePoint::new_2d(a3, a3, w3),
                QuadraturePoint::new_2d(b3, a3, w3),
                QuadraturePoint::new_2d(a3, b3, w3),
            ]
        }
        _ => {
            // 12-point rule (degree 6)
            let a1 = 0.063089014491502;
            let b1 = 0.873821971016996;
            let w1 = 0.025422453185103;

            let a2 = 0.249286745170910;
            let b2 = 0.501426509658179;
            let w2 = 0.058393137863189;

            let a3 = 0.310352451033785;
            let b3 = 0.053145049844816;
            let c3 = 0.636502499121399;
            let w3 = 0.041425537809187;

            vec![
                QuadraturePoint::new_2d(a1, a1, w1),
                QuadraturePoint::new_2d(b1, a1, w1),
                QuadraturePoint::new_2d(a1, b1, w1),
                QuadraturePoint::new_2d(a2, a2, w2),
                QuadraturePoint::new_2d(b2, a2, w2),
                QuadraturePoint::new_2d(a2, b2, w2),
                QuadraturePoint::new_2d(a3, b3, w3),
                QuadraturePoint::new_2d(b3, a3, w3),
                QuadraturePoint::new_2d(a3, c3, w3),
                QuadraturePoint::new_2d(c3, a3, w3),
                QuadraturePoint::new_2d(b3, c3, w3),
                QuadraturePoint::new_2d(c3, b3, w3),
            ]
        }
    }
}

/// Gauss quadrature for quadrilaterals
/// Reference quad: [-1,1] x [-1,1]
pub fn gauss_quadrilateral(order: usize) -> Vec<QuadraturePoint> {
    let pts_1d = gauss_legendre_1d(order);
    let mut result = Vec::with_capacity(pts_1d.len() * pts_1d.len());

    for pi in &pts_1d {
        for pj in &pts_1d {
            result.push(QuadraturePoint::new_2d(
                pi.xi(),
                pj.xi(),
                pi.weight * pj.weight,
            ));
        }
    }

    result
}

/// Gauss quadrature for tetrahedra
/// Reference tet: (0,0,0), (1,0,0), (0,1,0), (0,0,1)
pub fn gauss_tetrahedron(order: usize) -> Vec<QuadraturePoint> {
    match order {
        1 => {
            // 1-point rule
            vec![QuadraturePoint::new_3d(0.25, 0.25, 0.25, 1.0 / 6.0)]
        }
        2 => {
            // 4-point rule (degree 2)
            let a = (5.0 - 5.0_f64.sqrt()) / 20.0;
            let b = (5.0 + 3.0 * 5.0_f64.sqrt()) / 20.0;
            let w = 1.0 / 24.0;
            vec![
                QuadraturePoint::new_3d(a, a, a, w),
                QuadraturePoint::new_3d(b, a, a, w),
                QuadraturePoint::new_3d(a, b, a, w),
                QuadraturePoint::new_3d(a, a, b, w),
            ]
        }
        3 | 4 => {
            // 5-point rule (degree 3)
            let w1 = -4.0 / 30.0;
            let w2 = 9.0 / 120.0;
            vec![
                QuadraturePoint::new_3d(0.25, 0.25, 0.25, w1),
                QuadraturePoint::new_3d(1.0 / 6.0, 1.0 / 6.0, 1.0 / 6.0, w2),
                QuadraturePoint::new_3d(0.5, 1.0 / 6.0, 1.0 / 6.0, w2),
                QuadraturePoint::new_3d(1.0 / 6.0, 0.5, 1.0 / 6.0, w2),
                QuadraturePoint::new_3d(1.0 / 6.0, 1.0 / 6.0, 0.5, w2),
            ]
        }
        _ => {
            // 15-point rule (degree 5)
            let a1 = 0.25;
            let w1 = 8.0 / 405.0;

            let a2 = 1.0 / 3.0;
            let b2 = 0.0;
            let w2 = 5.0 / 567.0;

            let a3 = (1.0 + 15.0_f64.sqrt()) / 8.0;
            let b3 = (1.0 - 15.0_f64.sqrt()) / 8.0;
            let w3 = 20.0 / 2187.0;

            vec![
                QuadraturePoint::new_3d(a1, a1, a1, w1),
                // Permutations of (a2, a2, a2, b2)
                QuadraturePoint::new_3d(a2, a2, a2, w2),
                QuadraturePoint::new_3d(b2, a2, a2, w2),
                QuadraturePoint::new_3d(a2, b2, a2, w2),
                QuadraturePoint::new_3d(a2, a2, b2, w2),
                // Permutations of (a3, a3, a3, b3)
                QuadraturePoint::new_3d(a3, a3, a3, w3),
                QuadraturePoint::new_3d(b3, a3, a3, w3),
                QuadraturePoint::new_3d(a3, b3, a3, w3),
                QuadraturePoint::new_3d(a3, a3, b3, w3),
                // Permutations of (a3, a3, b3, b3)
                QuadraturePoint::new_3d(a3, a3, b3, w3),
                QuadraturePoint::new_3d(a3, b3, a3, w3),
                QuadraturePoint::new_3d(b3, a3, a3, w3),
                QuadraturePoint::new_3d(a3, b3, b3, w3),
                QuadraturePoint::new_3d(b3, a3, b3, w3),
                QuadraturePoint::new_3d(b3, b3, a3, w3),
            ]
        }
    }
}

/// Gauss quadrature for hexahedra
/// Reference hex: [-1,1] x [-1,1] x [-1,1]
pub fn gauss_hexahedron(order: usize) -> Vec<QuadraturePoint> {
    let pts_1d = gauss_legendre_1d(order);
    let mut result = Vec::with_capacity(pts_1d.len().pow(3));

    for pi in &pts_1d {
        for pj in &pts_1d {
            for pk in &pts_1d {
                result.push(QuadraturePoint::new_3d(
                    pi.xi(),
                    pj.xi(),
                    pk.xi(),
                    pi.weight * pj.weight * pk.weight,
                ));
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gauss_1d_integrates_polynomials() {
        // 2-point rule should exactly integrate up to degree 3
        let pts = gauss_legendre_1d(2);

        // Integrate x^2 from -1 to 1 = 2/3
        let integral: f64 = pts.iter().map(|p| p.xi().powi(2) * p.weight).sum();
        assert!((integral - 2.0 / 3.0).abs() < 1e-14);

        // Integrate x^3 from -1 to 1 = 0
        let integral: f64 = pts.iter().map(|p| p.xi().powi(3) * p.weight).sum();
        assert!(integral.abs() < 1e-14);
    }

    #[test]
    fn test_triangle_centroid_rule() {
        let pts = gauss_triangle(1);
        assert_eq!(pts.len(), 1);
        assert!((pts[0].xi() - 1.0 / 3.0).abs() < 1e-14);
        assert!((pts[0].eta() - 1.0 / 3.0).abs() < 1e-14);
        assert!((pts[0].weight - 0.5).abs() < 1e-14); // Area of reference triangle
    }

    #[test]
    fn test_quad_weights_sum() {
        for order in 1..=5 {
            let pts = gauss_quadrilateral(order);
            let sum: f64 = pts.iter().map(|p| p.weight).sum();
            // Weights should sum to 4 (area of [-1,1]^2)
            assert!(
                (sum - 4.0).abs() < 1e-14,
                "Order {} failed: sum = {}",
                order,
                sum
            );
        }
    }

    #[test]
    fn test_tet_weights_sum() {
        let pts = gauss_tetrahedron(1);
        let sum: f64 = pts.iter().map(|p| p.weight).sum();
        // Weights should sum to 1/6 (volume of reference tet)
        assert!((sum - 1.0 / 6.0).abs() < 1e-14);
    }

    #[test]
    fn test_hex_weights_sum() {
        for order in 1..=3 {
            let pts = gauss_hexahedron(order);
            let sum: f64 = pts.iter().map(|p| p.weight).sum();
            // Weights should sum to 8 (volume of [-1,1]^3)
            assert!(
                (sum - 8.0).abs() < 1e-14,
                "Order {} failed: sum = {}",
                order,
                sum
            );
        }
    }
}
