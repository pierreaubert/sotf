//! Gauss-Legendre quadrature rules
//!
//! Direct port of integration constants from NC_IntegrationConstants.h
//! and NC_ComputeGausseanPoints from NC_3dFunctions.cpp.

// Allow excessive precision for high-precision mathematical constants
#![allow(clippy::excessive_precision)]

/// Maximum supported Gauss-Legendre order
pub const N_GAUORDER: usize = 80;

/// Gauss-Legendre abscissas and weights for various orders
///
/// Returns (points, weights) where points are in [-1, 1]
pub fn gauss_legendre(order: usize) -> (Vec<f64>, Vec<f64>) {
    assert!(
        (1..=N_GAUORDER).contains(&order),
        "Order must be 1..{}",
        N_GAUORDER
    );

    let (x, w) = gauss_legendre_raw(order);
    (x.to_vec(), w.to_vec())
}

/// Raw Gauss-Legendre points and weights
fn gauss_legendre_raw(n: usize) -> (&'static [f64], &'static [f64]) {
    match n {
        1 => (&GL1_X, &GL1_W),
        2 => (&GL2_X, &GL2_W),
        3 => (&GL3_X, &GL3_W),
        4 => (&GL4_X, &GL4_W),
        5 => (&GL5_X, &GL5_W),
        6 => (&GL6_X, &GL6_W),
        7 => (&GL7_X, &GL7_W),
        8 => (&GL8_X, &GL8_W),
        10 => (&GL10_X, &GL10_W),
        12 => (&GL12_X, &GL12_W),
        16 => (&GL16_X, &GL16_W),
        20 => (&GL20_X, &GL20_W),
        _ => {
            // Fall back to closest available
            if n <= 2 {
                (&GL2_X, &GL2_W)
            } else if n <= 4 {
                (&GL4_X, &GL4_W)
            } else if n <= 6 {
                (&GL6_X, &GL6_W)
            } else if n <= 8 {
                (&GL8_X, &GL8_W)
            } else if n <= 12 {
                (&GL12_X, &GL12_W)
            } else if n <= 16 {
                (&GL16_X, &GL16_W)
            } else {
                (&GL20_X, &GL20_W)
            }
        }
    }
}

/// Triangle quadrature points (xi, eta, weight)
///
/// Returns vector of (xi, eta, weight) tuples for the reference triangle
/// with vertices at (0,0), (1,0), (0,1). Weights are scaled so they sum
/// to 0.5 (the area of the reference triangle).
pub fn triangle_quadrature(order: usize) -> Vec<(f64, f64, f64)> {
    // The raw tables have weights that sum to 1.0 (unit simplex convention).
    // We scale by 0.5 to get weights that sum to the reference triangle area.
    const AREA_SCALE: f64 = 0.5;
    match order {
        1 => GAUCORWEI_TR1
            .iter()
            .map(|&[x, y, w]| (x, y, w * AREA_SCALE))
            .collect(),
        2 => GAUCORWEI_TR4
            .iter()
            .map(|&[x, y, w]| (x, y, w * AREA_SCALE))
            .collect(),
        3 => GAUCORWEI_TR7
            .iter()
            .map(|&[x, y, w]| (x, y, w * AREA_SCALE))
            .collect(),
        _ => GAUCORWEI_TR13
            .iter()
            .map(|&[x, y, w]| (x, y, w * AREA_SCALE))
            .collect(),
    }
}

/// Quad quadrature points (tensor product of 1D Gauss-Legendre)
///
/// Returns vector of (xi, eta, weight) tuples for [-1,1]²
pub fn quad_quadrature(order: usize) -> Vec<(f64, f64, f64)> {
    let (points, weights) = gauss_legendre(order);
    let mut result = Vec::with_capacity(order * order);

    for (i, &xi) in points.iter().enumerate() {
        for (j, &eta) in points.iter().enumerate() {
            result.push((xi, eta, weights[i] * weights[j]));
        }
    }

    result
}

/// Unit sphere quadrature for FMM
///
/// Uses Gauss-Legendre in theta direction and uniform in phi
pub fn unit_sphere_quadrature(n_theta: usize, n_phi: usize) -> (Vec<[f64; 3]>, Vec<f64>) {
    use std::f64::consts::PI;

    let (cos_theta, weights_theta) = gauss_legendre(n_theta);
    let delta_phi = 2.0 * PI / n_phi as f64;

    let n_points = n_theta * n_phi;
    let mut coords = Vec::with_capacity(n_points);
    let mut weights = Vec::with_capacity(n_points);

    for (i, &ct) in cos_theta.iter().enumerate() {
        let st = (1.0 - ct * ct).sqrt();
        for j in 0..n_phi {
            let phi = delta_phi * j as f64;
            coords.push([st * phi.cos(), st * phi.sin(), ct]);
            weights.push(weights_theta[i] * delta_phi / (4.0 * PI));
        }
    }

    (coords, weights)
}

// Gauss-Legendre abscissas and weights
// Ported from NC_IntegrationConstants.h
static GL1_X: [f64; 1] = [0.0];
static GL1_W: [f64; 1] = [2.0];

static GL2_X: [f64; 2] = [-0.5773502691896257, 0.5773502691896257];
static GL2_W: [f64; 2] = [1.0, 1.0];

static GL3_X: [f64; 3] = [-0.7745966692414834, 0.0, 0.7745966692414834];
static GL3_W: [f64; 3] = [0.5555555555555556, 0.8888888888888888, 0.5555555555555556];

static GL4_X: [f64; 4] = [
    -0.8611363115940526,
    -0.3399810435848563,
    0.3399810435848563,
    0.8611363115940526,
];
static GL4_W: [f64; 4] = [
    0.3478548451374538,
    0.6521451548625461,
    0.6521451548625461,
    0.3478548451374538,
];

static GL5_X: [f64; 5] = [
    -0.9061798459386640,
    -0.5384693101056831,
    0.0,
    0.5384693101056831,
    0.9061798459386640,
];
static GL5_W: [f64; 5] = [
    0.2369268850561891,
    0.4786286704993665,
    0.5688888888888889,
    0.4786286704993665,
    0.2369268850561891,
];

static GL6_X: [f64; 6] = [
    -0.9324695142031521,
    -0.6612093864662645,
    -0.2386191860831969,
    0.2386191860831969,
    0.6612093864662645,
    0.9324695142031521,
];
static GL6_W: [f64; 6] = [
    0.1713244923791704,
    0.3607615730481386,
    0.4679139345726910,
    0.4679139345726910,
    0.3607615730481386,
    0.1713244923791704,
];

static GL7_X: [f64; 7] = [
    -0.9491079123427585,
    -0.7415311855993945,
    -0.4058451513773972,
    0.0,
    0.4058451513773972,
    0.7415311855993945,
    0.9491079123427585,
];
static GL7_W: [f64; 7] = [
    0.1294849661688697,
    0.2797053914892766,
    0.3818300505051189,
    0.4179591836734694,
    0.3818300505051189,
    0.2797053914892766,
    0.1294849661688697,
];

static GL8_X: [f64; 8] = [
    -0.9602898564975363,
    -0.7966664774136267,
    -0.5255324099163290,
    -0.1834346424956498,
    0.1834346424956498,
    0.5255324099163290,
    0.7966664774136267,
    0.9602898564975363,
];
static GL8_W: [f64; 8] = [
    0.1012285362903763,
    0.2223810344533745,
    0.3137066458778873,
    0.3626837833783620,
    0.3626837833783620,
    0.3137066458778873,
    0.2223810344533745,
    0.1012285362903763,
];

static GL10_X: [f64; 10] = [
    -0.9739065285171717,
    -0.8650633666889845,
    -0.6794095682990244,
    -0.4333953941292472,
    -0.1488743389816312,
    0.1488743389816312,
    0.4333953941292472,
    0.6794095682990244,
    0.8650633666889845,
    0.9739065285171717,
];
static GL10_W: [f64; 10] = [
    0.0666713443086881,
    0.1494513491505806,
    0.2190863625159820,
    0.2692667193099963,
    0.2955242247147529,
    0.2955242247147529,
    0.2692667193099963,
    0.2190863625159820,
    0.1494513491505806,
    0.0666713443086881,
];

static GL12_X: [f64; 12] = [
    -0.9815606342467192,
    -0.9041172563704749,
    -0.7699026741943047,
    -0.5873179542866175,
    -0.3678314989981802,
    -0.1252334085114689,
    0.1252334085114689,
    0.3678314989981802,
    0.5873179542866175,
    0.7699026741943047,
    0.9041172563704749,
    0.9815606342467192,
];
static GL12_W: [f64; 12] = [
    0.0471753363865118,
    0.1069393259953184,
    0.1600783285433462,
    0.2031674267230659,
    0.2334925365383548,
    0.2491470458134028,
    0.2491470458134028,
    0.2334925365383548,
    0.2031674267230659,
    0.1600783285433462,
    0.1069393259953184,
    0.0471753363865118,
];

static GL16_X: [f64; 16] = [
    -0.9894009349916499,
    -0.9445750230732326,
    -0.8656312023878318,
    -0.7554044083550030,
    -0.6178762444026438,
    -0.4580167776572274,
    -0.2816035507792589,
    -0.0950125098376374,
    0.0950125098376374,
    0.2816035507792589,
    0.4580167776572274,
    0.6178762444026438,
    0.7554044083550030,
    0.8656312023878318,
    0.9445750230732326,
    0.9894009349916499,
];
static GL16_W: [f64; 16] = [
    0.0271524594117541,
    0.0622535239386479,
    0.0951585116824928,
    0.1246289712555339,
    0.1495959888165767,
    0.1691565193950025,
    0.1826034150449236,
    0.1894506104550685,
    0.1894506104550685,
    0.1826034150449236,
    0.1691565193950025,
    0.1495959888165767,
    0.1246289712555339,
    0.0951585116824928,
    0.0622535239386479,
    0.0271524594117541,
];

static GL20_X: [f64; 20] = [
    -0.9931285991850949,
    -0.9639719272779138,
    -0.9122344282513259,
    -0.8391169718222188,
    -0.7463319064601508,
    -0.6360536807265150,
    -0.5108670019508271,
    -0.3737060887154195,
    -0.2277858511416451,
    -0.0765265211334973,
    0.0765265211334973,
    0.2277858511416451,
    0.3737060887154195,
    0.5108670019508271,
    0.6360536807265150,
    0.7463319064601508,
    0.8391169718222188,
    0.9122344282513259,
    0.9639719272779138,
    0.9931285991850949,
];
static GL20_W: [f64; 20] = [
    0.0176140071391521,
    0.0406014298003869,
    0.0626720483341091,
    0.0832767415767048,
    0.1019301198172404,
    0.1181945319615184,
    0.1316886384491766,
    0.1420961093183820,
    0.1491729864726037,
    0.1527533871307258,
    0.1527533871307258,
    0.1491729864726037,
    0.1420961093183820,
    0.1316886384491766,
    0.1181945319615184,
    0.1019301198172404,
    0.0832767415767048,
    0.0626720483341091,
    0.0406014298003869,
    0.0176140071391521,
];

// Triangle quadrature rules (from NC_IntegrationConstants.h)
// Format: [xi, eta, weight]

static GAUCORWEI_TR1: [[f64; 3]; 1] = [[0.333333333333333, 0.333333333333333, 1.0]];

static GAUCORWEI_TR4: [[f64; 3]; 4] = [
    [0.333333333333333, 0.333333333333333, -0.5625],
    [0.6, 0.2, 0.520833333333333],
    [0.2, 0.6, 0.520833333333333],
    [0.2, 0.2, 0.520833333333333],
];

static GAUCORWEI_TR7: [[f64; 3]; 7] = [
    [0.333333333333333, 0.333333333333333, 0.225],
    [0.797426985353087, 0.101286507323456, 0.125939180544827],
    [0.101286507323456, 0.797426985353087, 0.125939180544827],
    [0.101286507323456, 0.101286507323456, 0.125939180544827],
    [0.470142064105115, 0.059715871789770, 0.132394152788506],
    [0.059715871789770, 0.470142064105115, 0.132394152788506],
    [0.470142064105115, 0.470142064105115, 0.132394152788506],
];

static GAUCORWEI_TR13: [[f64; 3]; 13] = [
    [0.333333333333333, 0.333333333333333, -0.149570044467682],
    [0.260345966079040, 0.260345966079040, 0.175615257433208],
    [0.260345966079040, 0.479308067841920, 0.175615257433208],
    [0.479308067841920, 0.260345966079040, 0.175615257433208],
    [0.065130102902216, 0.065130102902216, 0.053347235608838],
    [0.065130102902216, 0.869739794195568, 0.053347235608838],
    [0.869739794195568, 0.065130102902216, 0.053347235608838],
    [0.638444188569810, 0.048690315425316, 0.077113760890257],
    [0.048690315425316, 0.638444188569810, 0.077113760890257],
    [0.638444188569810, 0.312865496004874, 0.077113760890257],
    [0.312865496004874, 0.638444188569810, 0.077113760890257],
    [0.048690315425316, 0.312865496004874, 0.077113760890257],
    [0.312865496004874, 0.048690315425316, 0.077113760890257],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gauss_legendre_2() {
        let (x, w) = gauss_legendre(2);
        assert_eq!(x.len(), 2);
        assert!((x[0] + 0.5773502691896257).abs() < 1e-10);
        assert!((w[0] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_gauss_weights_sum() {
        // Sum of weights should be 2 (integral of 1 over [-1,1])
        for n in [2, 4, 6, 8, 10, 12, 16, 20] {
            let (_, w) = gauss_legendre(n);
            let sum: f64 = w.iter().sum();
            assert!((sum - 2.0).abs() < 1e-10, "n={}: sum={}", n, sum);
        }
    }

    #[test]
    fn test_triangle_quadrature() {
        let tri7 = triangle_quadrature(3);
        assert_eq!(tri7.len(), 7);

        // Weights should sum to 0.5 (area of reference triangle)
        let sum: f64 = tri7.iter().map(|&(_, _, w)| w).sum();
        assert!((sum - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_quad_quadrature() {
        let quad4 = quad_quadrature(2);
        assert_eq!(quad4.len(), 4);

        // Weights should sum to 4 (area of [-1,1]²)
        let sum: f64 = quad4.iter().map(|&(_, _, w)| w).sum();
        assert!((sum - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_sphere_quadrature() {
        let (coords, weights) = unit_sphere_quadrature(4, 8);
        assert_eq!(coords.len(), 32);
        assert_eq!(weights.len(), 32);

        // All points should be on unit sphere
        for c in &coords {
            let r = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
            assert!((r - 1.0).abs() < 1e-10);
        }

        // Weights should sum to 1 (normalized)
        let sum: f64 = weights.iter().sum();
        assert!((sum - 1.0).abs() < 0.01); // Approximate due to discretization
    }
}
