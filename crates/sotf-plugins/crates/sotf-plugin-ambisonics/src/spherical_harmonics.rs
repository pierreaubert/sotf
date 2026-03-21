// ============================================================================
// Spherical Harmonics for Ambisonics
// ============================================================================
//
// ACN (Ambisonic Channel Number) ordering with SN3D normalization.
// ACN index: n = l² + l + m  where l = degree (order), m = index (-l..=l)
// SN3D: Schmidt semi-normalized spherical harmonics
//
// Reference: Chapman (2009), Ambisonic principles

use std::f64::consts::PI;

/// Maximum supported Ambisonics order
pub const MAX_ORDER: usize = 3;

/// Number of Ambisonics channels for a given order: (order+1)²
pub fn channel_count(order: usize) -> usize {
    (order + 1) * (order + 1)
}

/// Extract (degree, index) from ACN channel number.
/// ACN = l² + l + m
pub fn acn_to_degree_index(acn: usize) -> (i32, i32) {
    let l = (acn as f64).sqrt() as i32;
    let m = acn as i32 - l * l - l;
    (l, m)
}

/// Compute a single real spherical harmonic Y_l^m(azimuth, elevation)
/// using ACN ordering and SN3D normalization.
///
/// - `azimuth`: horizontal angle in radians (0 = front, positive = left / counter-clockwise)
/// - `elevation`: vertical angle in radians (0 = horizon, positive = up)
/// - `l`: degree (order), 0..=MAX_ORDER
/// - `m`: index, -l..=l
pub fn spherical_harmonic(l: i32, m: i32, azimuth: f64, elevation: f64) -> f64 {
    let sn3d = sn3d_normalization(l, m);
    let alp = associated_legendre(l, m.unsigned_abs() as i32, elevation.sin());

    // AmbiX convention (ACN + SN3D):
    //   m >= 0: cos(m * azimuth)
    //   m <  0: sin(|m| * azimuth)
    if m > 0 {
        sn3d * alp * (m as f64 * azimuth).cos()
    } else if m < 0 {
        sn3d * alp * ((-m) as f64 * azimuth).sin()
    } else {
        sn3d * alp
    }
}

/// Compute all spherical harmonics up to `order` for a given direction.
/// Returns a vector of length `(order+1)²` in ACN order.
pub fn spherical_harmonics_vector(order: usize, azimuth: f64, elevation: f64) -> Vec<f64> {
    let n = channel_count(order);
    let mut result = Vec::with_capacity(n);
    for acn in 0..n {
        let (l, m) = acn_to_degree_index(acn);
        result.push(spherical_harmonic(l, m, azimuth, elevation));
    }
    result
}

/// SN3D normalization factor.
///
/// SN3D(l, m) = sqrt( (2 - delta(m,0)) * (l - |m|)! / (l + |m|)! )
///
/// where delta is the Kronecker delta.
fn sn3d_normalization(l: i32, m: i32) -> f64 {
    let abs_m = m.unsigned_abs() as i32;
    let delta = if m == 0 { 1.0 } else { 2.0 };
    let num = factorial(l - abs_m) as f64;
    let den = factorial(l + abs_m) as f64;
    (delta * num / den).sqrt()
}

/// Associated Legendre polynomial P_l^m(x) (unnormalized, without Condon-Shortley phase).
///
/// Uses stable recurrence relation for computation.
fn associated_legendre(l: i32, m: i32, x: f64) -> f64 {
    debug_assert!(m >= 0);
    debug_assert!(m <= l);

    // Start with P_m^m
    let mut pmm = 1.0;
    if m > 0 {
        let sqrt_1_minus_x2 = (1.0 - x * x).max(0.0).sqrt();
        let mut fact = 1.0;
        for _ in 1..=m {
            pmm *= fact * sqrt_1_minus_x2;
            fact += 2.0;
        }
    }

    if l == m {
        return pmm;
    }

    // P_{m+1}^m = x * (2m+1) * P_m^m
    let mut pmm1 = x * (2 * m + 1) as f64 * pmm;

    if l == m + 1 {
        return pmm1;
    }

    // Recurrence: (l-m) P_l^m = x(2l-1) P_{l-1}^m - (l+m-1) P_{l-2}^m
    let mut pll = 0.0;
    for ll in (m + 2)..=l {
        pll = (x * (2 * ll - 1) as f64 * pmm1 - (ll + m - 1) as f64 * pmm) / (ll - m) as f64;
        pmm = pmm1;
        pmm1 = pll;
    }
    pll
}

/// Factorial for small non-negative integers (sufficient for order <= 7).
fn factorial(n: i32) -> u64 {
    debug_assert!(n >= 0);
    match n {
        0 | 1 => 1,
        2 => 2,
        3 => 6,
        4 => 24,
        5 => 120,
        6 => 720,
        7 => 5040,
        8 => 40320,
        9 => 362_880,
        10 => 3_628_800,
        11 => 39_916_800,
        12 => 479_001_600,
        13 => 6_227_020_800,
        14 => 87_178_291_200,
        _ => (1..=n as u64).product(),
    }
}

/// Convert degrees to radians
pub fn deg_to_rad(degrees: f64) -> f64 {
    degrees * PI / 180.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOLERANCE: f64 = 1e-10;

    #[test]
    fn test_channel_count() {
        assert_eq!(channel_count(0), 1); // W only
        assert_eq!(channel_count(1), 4); // FOA: W, Y, Z, X
        assert_eq!(channel_count(2), 9); // SOA
        assert_eq!(channel_count(3), 16); // TOA
    }

    #[test]
    fn test_acn_ordering() {
        // ACN 0: l=0, m=0  (W - omnidirectional)
        assert_eq!(acn_to_degree_index(0), (0, 0));
        // ACN 1: l=1, m=-1 (Y - left-right)
        assert_eq!(acn_to_degree_index(1), (1, -1));
        // ACN 2: l=1, m=0  (Z - up-down)
        assert_eq!(acn_to_degree_index(2), (1, 0));
        // ACN 3: l=1, m=1  (X - front-back)
        assert_eq!(acn_to_degree_index(3), (1, 1));
        // ACN 4: l=2, m=-2
        assert_eq!(acn_to_degree_index(4), (2, -2));
    }

    #[test]
    fn test_zeroth_order_omnidirectional() {
        // Y_0^0 should be 1.0 everywhere (with SN3D normalization)
        let y00_front = spherical_harmonic(0, 0, 0.0, 0.0);
        let y00_left = spherical_harmonic(0, 0, PI / 2.0, 0.0);
        let y00_up = spherical_harmonic(0, 0, 0.0, PI / 2.0);
        let y00_back = spherical_harmonic(0, 0, PI, 0.0);

        assert!((y00_front - 1.0).abs() < TOLERANCE);
        assert!((y00_left - 1.0).abs() < TOLERANCE);
        assert!((y00_up - 1.0).abs() < TOLERANCE);
        assert!((y00_back - 1.0).abs() < TOLERANCE);
    }

    #[test]
    fn test_first_order_cardinal_directions() {
        // AmbiX convention: ACN 1 = Y (sin(az)), ACN 2 = Z (sin(el)), ACN 3 = X (cos(az))
        // FOA at front (az=0, el=0): W=1, Y=0, Z=0, X=1
        let front = spherical_harmonics_vector(1, 0.0, 0.0);
        assert!((front[0] - 1.0).abs() < TOLERANCE); // W
        assert!(front[1].abs() < TOLERANCE); // Y: sin(0) = 0
        assert!(front[2].abs() < TOLERANCE); // Z: sin(0) = 0
        assert!((front[3] - 1.0).abs() < TOLERANCE); // X: cos(0) = 1

        // FOA at left (az=pi/2, el=0): W=1, Y=1, Z=0, X=0
        let left = spherical_harmonics_vector(1, PI / 2.0, 0.0);
        assert!((left[0] - 1.0).abs() < TOLERANCE); // W
        assert!((left[1] - 1.0).abs() < TOLERANCE); // Y: sin(pi/2) = 1
        assert!(left[2].abs() < TOLERANCE); // Z: sin(0) = 0
        assert!(left[3].abs() < TOLERANCE); // X: cos(pi/2) = 0

        // FOA at top (az=0, el=pi/2): W=1, Y=0, Z=1, X=0
        let top = spherical_harmonics_vector(1, 0.0, PI / 2.0);
        assert!((top[0] - 1.0).abs() < TOLERANCE); // W
        assert!(top[1].abs() < TOLERANCE); // Y
        assert!((top[2] - 1.0).abs() < TOLERANCE); // Z: sin(pi/2) = 1
        assert!(top[3].abs() < TOLERANCE); // X: cos(el)*cos(az) = 0
    }

    #[test]
    fn test_second_order_values() {
        // Verify that second-order harmonics produce the expected channel count
        let sh = spherical_harmonics_vector(2, 0.0, 0.0);
        assert_eq!(sh.len(), 9);
        // W channel should still be 1.0
        assert!((sh[0] - 1.0).abs() < TOLERANCE);
    }

    #[test]
    fn test_orthogonality_foa() {
        // Numerical integration on the sphere: integral of Y_l1^m1 * Y_l2^m2 should be
        // approximately delta(l1,l2)*delta(m1,m2) * 4π / (2l+1) for SN3D
        // We approximate with a grid of points
        let n_az = 72;
        let n_el = 36;
        let daz = 2.0 * PI / n_az as f64;
        let del = PI / n_el as f64;

        // Test that ACN 0 and ACN 3 (W and X) are orthogonal
        let mut integral = 0.0;
        for i in 0..n_az {
            let az = (i as f64 + 0.5) * daz - PI;
            for j in 0..n_el {
                let el = (j as f64 + 0.5) * del - PI / 2.0;
                let w = el.cos() * daz * del; // solid angle weight
                let y0 = spherical_harmonic(0, 0, az, el);
                let y3 = spherical_harmonic(1, 1, az, el);
                integral += y0 * y3 * w;
            }
        }
        assert!(
            integral.abs() < 0.01,
            "W and X should be orthogonal, got {}",
            integral
        );
    }

    #[test]
    fn test_sn3d_normalization_factors() {
        // SN3D(0,0) = 1.0
        assert!((sn3d_normalization(0, 0) - 1.0).abs() < TOLERANCE);
        // SN3D(1,0) = 1.0
        assert!((sn3d_normalization(1, 0) - 1.0).abs() < TOLERANCE);
        // SN3D(1,1) = 1.0
        assert!((sn3d_normalization(1, 1) - 1.0).abs() < TOLERANCE);
        // SN3D(1,-1) = 1.0
        assert!((sn3d_normalization(1, -1) - 1.0).abs() < TOLERANCE);
    }

    #[test]
    fn test_deg_to_rad() {
        assert!((deg_to_rad(0.0)).abs() < TOLERANCE);
        assert!((deg_to_rad(90.0) - PI / 2.0).abs() < TOLERANCE);
        assert!((deg_to_rad(180.0) - PI).abs() < TOLERANCE);
        assert!((deg_to_rad(360.0) - 2.0 * PI).abs() < TOLERANCE);
    }
}
