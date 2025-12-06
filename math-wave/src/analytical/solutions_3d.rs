//! 3D Analytical Solutions
//!
//! Exact solutions for 3D acoustic scattering problems, primarily
//! sphere scattering using Mie theory (spherical Bessel/Hankel functions).

use super::{AnalyticalSolution, Point, SPEED_OF_SOUND};
use num_complex::Complex64;
use std::f64::consts::PI;

/// Sphere scattering: rigid sphere in a plane wave (Mie theory)
///
/// Analytical solution using spherical harmonics expansion:
///
/// ```text
/// p(r,θ) = Σₙ (2n+1) iⁿ [jₙ(kr) - aₙ hₙ⁽¹⁾(kr)] Pₙ(cos θ)
/// ```
///
/// For a **rigid sphere** (Neumann BC: ∂p/∂r = 0 on surface):
/// ```text
/// aₙ = jₙ'(ka) / hₙ⁽¹⁾'(ka)
/// ```
///
/// where:
/// - `jₙ` = spherical Bessel functions of the first kind
/// - `hₙ⁽¹⁾` = spherical Hankel functions of the first kind
/// - `Pₙ` = Legendre polynomials
///
/// # Arguments
///
/// * `wave_number` - k = 2πf/c
/// * `radius` - Sphere radius a
/// * `num_terms` - Number of terms (typically ka + 10)
/// * `r_points` - Radial evaluation points (must be >= radius)
/// * `theta_points` - Polar angle points (from z-axis, 0 to π)
///
/// # References
///
/// - Morse & Ingard, "Theoretical Acoustics", 1968, Section 8.3
/// - Bowman et al., "Electromagnetic and Acoustic Scattering", 1987
///
/// # Example
///
/// ```rust
/// use math_wave::analytical::sphere_scattering_3d;
/// use std::f64::consts::PI;
///
/// // ka = 1 (Mie regime)
/// let solution = sphere_scattering_3d(
///     1.0,  // wave_number
///     1.0,  // radius
///     20,   // num_terms
///     vec![1.5, 2.0, 3.0],  // r
///     vec![0.0, PI/4.0, PI/2.0, PI],  // theta
/// );
/// ```
pub fn sphere_scattering_3d(
    wave_number: f64,
    radius: f64,
    num_terms: usize,
    r_points: Vec<f64>,
    theta_points: Vec<f64>,
) -> AnalyticalSolution {
    let ka = wave_number * radius;

    // Compute scattering coefficients for rigid sphere
    let coefficients = compute_rigid_sphere_coefficients(ka, num_terms);

    // Generate grid (axisymmetric, so φ = 0)
    let mut positions = Vec::new();
    let mut pressure = Vec::new();

    for &r in &r_points {
        for &theta in &theta_points {
            positions.push(Point::from_spherical(r, theta, 0.0));

            let kr = wave_number * r;
            let cos_theta = theta.cos();

            // Total field: Σₙ (2n+1) iⁿ [jₙ(kr) - aₙ hₙ⁽¹⁾(kr)] Pₙ(cos θ)
            let mut total = Complex64::new(0.0, 0.0);

            for (n, coeff) in coefficients.iter().enumerate().take(num_terms) {
                let n_f64 = n as f64;

                // (2n+1)
                let prefactor = 2.0 * n_f64 + 1.0;

                // iⁿ = exp(i*n*π/2)
                let i_power_n = Complex64::new((n_f64 * PI / 2.0).cos(), (n_f64 * PI / 2.0).sin());

                // Spherical Bessel jₙ(kr)
                let jn = spherical_bessel_j(n, kr);

                // Spherical Hankel hₙ⁽¹⁾(kr) = jₙ(kr) + i*yₙ(kr)
                let yn = spherical_bessel_y(n, kr);
                let hn = Complex64::new(jn, yn);

                // Legendre polynomial Pₙ(cos θ)
                let pn = legendre_p(n, cos_theta);

                // Add term
                total += prefactor * i_power_n * (jn - coeff * hn) * pn;
            }

            pressure.push(total);
        }
    }

    let frequency = wave_number * SPEED_OF_SOUND / (2.0 * PI);

    AnalyticalSolution {
        name: format!("3D Sphere Scattering (ka={:.2})", ka),
        dimensions: 3,
        positions,
        pressure,
        wave_number,
        frequency,
        metadata: serde_json::json!({
            "radius": radius,
            "ka": ka,
            "num_terms": num_terms,
            "boundary_condition": "rigid",
            "r_points": r_points,
            "theta_range": [theta_points.first(), theta_points.last()],
            "regime": classify_regime(ka),
        }),
    }
}

/// Classify scattering regime based on ka
pub fn classify_regime(ka: f64) -> &'static str {
    if ka < 0.3 {
        "Rayleigh (ka << 1)"
    } else if ka < 3.0 {
        "Mie (ka ~ 1)"
    } else {
        "Geometric (ka >> 1)"
    }
}

/// Compute scattering coefficients for rigid sphere
///
/// For Neumann BC (∂p/∂r = 0 on surface):
/// ```text
/// aₙ = jₙ'(ka) / hₙ⁽¹⁾'(ka)
/// ```
fn compute_rigid_sphere_coefficients(ka: f64, num_terms: usize) -> Vec<Complex64> {
    let mut coefficients = Vec::with_capacity(num_terms);

    for n in 0..num_terms {
        let n_f64 = n as f64;

        // Spherical Bessel and derivatives
        let jn = spherical_bessel_j(n, ka);
        let yn = spherical_bessel_y(n, ka);

        // Derivatives: jₙ'(x) = jₙ₋₁(x) - (n+1)/x * jₙ(x)
        let jn_minus_1 = if n > 0 {
            spherical_bessel_j(n - 1, ka)
        } else {
            // j₋₁(x) = cos(x)/x
            ka.cos() / ka
        };
        let jn_prime = jn_minus_1 - (n_f64 + 1.0) / ka * jn;

        // Same for yₙ
        let yn_minus_1 = if n > 0 {
            spherical_bessel_y(n - 1, ka)
        } else {
            // y₋₁(x) = sin(x)/x
            -ka.sin() / ka
        };
        let yn_prime = yn_minus_1 - (n_f64 + 1.0) / ka * yn;

        // Hankel derivative
        let hn_prime = Complex64::new(jn_prime, yn_prime);

        let a_n = Complex64::new(jn_prime, 0.0) / hn_prime;

        coefficients.push(a_n);
    }

    coefficients
}

/// Spherical Bessel function jₙ(x)
///
/// Relation to cylindrical Bessel: jₙ(x) = √(π/2x) * J_{n+1/2}(x)
///
/// Uses Miller's downward recurrence for numerical stability.
pub fn spherical_bessel_j(n: usize, x: f64) -> f64 {
    if x.abs() < 1e-10 {
        return if n == 0 { 1.0 } else { 0.0 };
    }

    // For small x or small n, direct formulas
    match n {
        0 => x.sin() / x,
        1 => x.sin() / (x * x) - x.cos() / x,
        _ => {
            // Miller's downward recurrence for stability when n > x
            let start_n = n + (x.abs() as usize) + 20;

            let mut j_next = 0.0;
            let mut j_curr = 1e-30;

            let mut values = vec![0.0; start_n + 1];
            values[start_n] = j_curr;

            for k in (0..start_n).rev() {
                let j_prev = (2 * k + 3) as f64 / x * j_curr - j_next;
                values[k] = j_prev;
                j_next = j_curr;
                j_curr = j_prev;
            }

            // Normalize using j₀(x) = sin(x)/x
            let true_j0 = x.sin() / x;
            let scale = true_j0 / values[0];

            values[n] * scale
        }
    }
}

/// Spherical Bessel function yₙ(x) (Neumann function)
///
/// Relation: yₙ(x) = √(π/2x) * Y_{n+1/2}(x)
pub fn spherical_bessel_y(n: usize, x: f64) -> f64 {
    if x.abs() < 1e-10 {
        return f64::NEG_INFINITY;
    }

    match n {
        0 => -x.cos() / x,
        1 => -x.cos() / (x * x) - x.sin() / x,
        _ => {
            // Upward recurrence (stable for y_n)
            let mut y_nm2 = -x.cos() / x;
            let mut y_nm1 = -x.cos() / (x * x) - x.sin() / x;

            for k in 2..=n {
                let y_n = (2 * k - 1) as f64 / x * y_nm1 - y_nm2;
                y_nm2 = y_nm1;
                y_nm1 = y_n;
            }

            y_nm1
        }
    }
}

/// Legendre polynomial Pₙ(x)
///
/// Uses recurrence: (n+1)Pₙ₊₁(x) = (2n+1)x Pₙ(x) - n Pₙ₋₁(x)
pub fn legendre_p(n: usize, x: f64) -> f64 {
    match n {
        0 => 1.0,
        1 => x,
        _ => {
            let mut p_nm2 = 1.0;
            let mut p_nm1 = x;

            for k in 2..=n {
                let p_n = ((2 * k - 1) as f64 * x * p_nm1 - (k - 1) as f64 * p_nm2) / k as f64;
                p_nm2 = p_nm1;
                p_nm1 = p_n;
            }

            p_nm1
        }
    }
}

/// Radar Cross Section (RCS) for sphere
///
/// RCS = σ = 4π/k² * Σₙ (2n+1) |aₙ|²
pub fn sphere_rcs_3d(wave_number: f64, radius: f64, num_terms: usize) -> f64 {
    let ka = wave_number * radius;
    let coefficients = compute_rigid_sphere_coefficients(ka, num_terms);

    let mut rcs = 0.0;
    for (n, a_n) in coefficients.iter().enumerate() {
        rcs += (2 * n + 1) as f64 * a_n.norm_sqr();
    }

    4.0 * PI * rcs / (wave_number * wave_number)
}

/// Scattering efficiency Q_scat = σ / (πa²)
///
/// Ratio of scattering cross-section to geometric cross-section.
pub fn sphere_scattering_efficiency_3d(wave_number: f64, radius: f64, num_terms: usize) -> f64 {
    let rcs = sphere_rcs_3d(wave_number, radius, num_terms);
    rcs / (PI * radius * radius)
}

/// 3D plane wave: p(x,y,z) = exp(ik·r)
///
/// A plane wave propagating in direction (θ, φ).
///
/// # Arguments
///
/// * `wave_number` - k = 2πf/c
/// * `theta` - Polar angle (from z-axis)
/// * `phi` - Azimuthal angle
/// * `points` - Evaluation points
pub fn plane_wave_3d(
    wave_number: f64,
    theta: f64,
    phi: f64,
    points: Vec<Point>,
) -> AnalyticalSolution {
    // Wave vector direction
    let kx = wave_number * theta.sin() * phi.cos();
    let ky = wave_number * theta.sin() * phi.sin();
    let kz = wave_number * theta.cos();

    let pressure: Vec<Complex64> = points
        .iter()
        .map(|p| {
            let phase = kx * p.x + ky * p.y + kz * p.z;
            Complex64::new(phase.cos(), phase.sin())
        })
        .collect();

    let frequency = wave_number * SPEED_OF_SOUND / (2.0 * PI);

    AnalyticalSolution {
        name: format!(
            "3D Plane Wave (k={}, θ={:.2}, φ={:.2})",
            wave_number, theta, phi
        ),
        dimensions: 3,
        positions: points,
        pressure,
        wave_number,
        frequency,
        metadata: serde_json::json!({
            "direction_theta": theta,
            "direction_phi": phi,
            "wave_vector": [kx, ky, kz],
            "wavelength": 2.0 * PI / wave_number,
        }),
    }
}

/// Point source (monopole): G(r) = exp(ikr)/(4πr)
///
/// Green's function for the Helmholtz equation in 3D.
///
/// # Arguments
///
/// * `wave_number` - k
/// * `source` - Source location
/// * `points` - Field points
pub fn point_source_3d(wave_number: f64, source: Point, points: Vec<Point>) -> AnalyticalSolution {
    let pressure: Vec<Complex64> = points
        .iter()
        .map(|p| {
            let r = p.distance_to(&source);
            if r < 1e-15 {
                Complex64::new(f64::INFINITY, 0.0)
            } else {
                let kr = wave_number * r;
                Complex64::new(kr.cos(), kr.sin()) / (4.0 * PI * r)
            }
        })
        .collect();

    let frequency = wave_number * SPEED_OF_SOUND / (2.0 * PI);

    AnalyticalSolution {
        name: format!("3D Point Source (k={})", wave_number),
        dimensions: 3,
        positions: points,
        pressure,
        wave_number,
        frequency,
        metadata: serde_json::json!({
            "source": [source.x, source.y, source.z],
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn test_spherical_bessel_j0() {
        // j₀(x) = sin(x)/x
        let x = 1.0;
        assert_abs_diff_eq!(spherical_bessel_j(0, x), x.sin() / x, epsilon = 1e-10);

        let x = PI;
        assert_abs_diff_eq!(spherical_bessel_j(0, x), 0.0, epsilon = 1e-10);
    }

    #[test]
    fn test_spherical_bessel_j1() {
        // j₁(x) = sin(x)/x² - cos(x)/x
        let x: f64 = 2.0;
        let expected = x.sin() / (x * x) - x.cos() / x;
        assert_abs_diff_eq!(spherical_bessel_j(1, x), expected, epsilon = 1e-10);
    }

    #[test]
    fn test_spherical_bessel_y0() {
        // y₀(x) = -cos(x)/x
        let x = 1.0;
        assert_abs_diff_eq!(spherical_bessel_y(0, x), -x.cos() / x, epsilon = 1e-10);
    }

    #[test]
    fn test_legendre_polynomials() {
        // P₀(x) = 1
        assert_abs_diff_eq!(legendre_p(0, 0.5), 1.0, epsilon = 1e-10);

        // P₁(x) = x
        assert_abs_diff_eq!(legendre_p(1, 0.5), 0.5, epsilon = 1e-10);

        // P₂(x) = (3x² - 1)/2
        let x = 0.5;
        let expected = (3.0 * x * x - 1.0) / 2.0;
        assert_abs_diff_eq!(legendre_p(2, x), expected, epsilon = 1e-10);
    }

    #[test]
    fn test_sphere_rayleigh_scattering() {
        // Rayleigh regime: ka << 1
        let k = 0.1;
        let a = 1.0;
        let ka = k * a;

        let rcs = sphere_rcs_3d(k, a, 10);

        // RCS should be positive and finite
        assert!(rcs > 0.0);
        assert!(rcs.is_finite());

        // In Rayleigh regime, RCS ~ (ka)⁴
        let rayleigh_scaling = ka.powi(4);
        assert!(rcs < rayleigh_scaling * 1000.0); // Rough check
    }

    #[test]
    fn test_sphere_geometric_limit() {
        // Geometric regime: ka >> 1
        // RCS → 2πa² (twice geometric cross section)
        let k = 20.0;
        let a = 1.0;

        let rcs = sphere_rcs_3d(k, a, 50);
        let geometric = 2.0 * PI * a * a;

        // Should approach geometric limit
        assert!((rcs / geometric - 1.0).abs() < 0.2); // Within 20%
    }

    #[test]
    fn test_sphere_scattering_3d() {
        // Basic sanity test
        let k = 1.0;
        let a = 1.0;

        let solution = sphere_scattering_3d(k, a, 20, vec![2.0], vec![0.0, PI / 2.0, PI]);

        assert_eq!(solution.pressure.len(), 3);

        // All pressures should be finite
        for p in &solution.pressure {
            assert!(p.re.is_finite());
            assert!(p.im.is_finite());
        }
    }

    #[test]
    fn test_scattering_efficiency() {
        let k = 1.0;
        let a = 1.0;

        let q_scat = sphere_scattering_efficiency_3d(k, a, 30);

        // Efficiency should be positive and reasonable
        assert!(q_scat > 0.0);
        assert!(q_scat < 10.0);
    }

    #[test]
    fn test_regime_classification() {
        assert_eq!(classify_regime(0.1), "Rayleigh (ka << 1)");
        assert_eq!(classify_regime(1.0), "Mie (ka ~ 1)");
        assert_eq!(classify_regime(10.0), "Geometric (ka >> 1)");
    }

    #[test]
    fn test_point_source_3d() {
        let k = 1.0;
        let source = Point::new_3d(0.0, 0.0, 0.0);
        let points = vec![Point::new_3d(1.0, 0.0, 0.0), Point::new_3d(2.0, 0.0, 0.0)];

        let solution = point_source_3d(k, source, points);

        // Magnitude should decay as 1/r
        let ratio = solution.pressure[1].norm() / solution.pressure[0].norm();
        assert_abs_diff_eq!(ratio, 0.5, epsilon = 1e-10);
    }

    #[test]
    fn test_plane_wave_3d() {
        let k = 1.0;
        let theta = 0.0; // Along z-axis
        let phi = 0.0;

        let points = vec![Point::new_3d(0.0, 0.0, 0.0), Point::new_3d(0.0, 0.0, 1.0)];

        let solution = plane_wave_3d(k, theta, phi, points);

        // |exp(ikz)| = 1
        for p in &solution.pressure {
            assert_abs_diff_eq!(p.norm(), 1.0, epsilon = 1e-10);
        }
    }
}
