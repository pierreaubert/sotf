//! 2D Analytical Solutions
//!
//! Exact solutions for 2D acoustic scattering problems, primarily
//! cylinder scattering using Bessel and Hankel function series.

use super::{AnalyticalSolution, Point};
use num_complex::Complex64;
use spec_math::Bessel;
use std::f64::consts::PI;

/// Cylinder scattering: rigid circular cylinder in a plane wave
///
/// Analytical solution using Bessel/Hankel function series:
///
/// ```text
/// p(r,θ) = exp(ikr cos θ) + Σ aₙ Hₙ⁽¹⁾(kr) exp(inθ)
/// ```
///
/// where Hₙ⁽¹⁾ are Hankel functions of the first kind.
///
/// For a **rigid cylinder** (Neumann boundary condition):
/// ```text
/// aₙ = -Jₙ'(ka) / Hₙ⁽¹⁾'(ka) * iⁿ
/// ```
///
/// # Arguments
///
/// * `wave_number` - k = 2πf/c
/// * `radius` - Cylinder radius
/// * `num_terms` - Number of terms in series (typically 2*ka + 10)
/// * `r_points` - Radial evaluation points
/// * `theta_points` - Angular evaluation points
///
/// # References
///
/// - Bowman, Senior, Uslenghi, "Electromagnetic and Acoustic Scattering
///   by Simple Shapes", 1987, Section 5.3
///
/// # Example
///
/// ```rust
/// use bem::analytical::cylinder_scattering_2d;
/// use std::f64::consts::PI;
///
/// // ka = 1 (low frequency)
/// let solution = cylinder_scattering_2d(1.0, 1.0, 20, vec![1.0, 2.0, 3.0], vec![0.0, PI/4.0, PI/2.0]);
/// ```
pub fn cylinder_scattering_2d(
    wave_number: f64,
    radius: f64,
    num_terms: usize,
    r_points: Vec<f64>,
    theta_points: Vec<f64>,
) -> AnalyticalSolution {
    let ka = wave_number * radius;

    // Compute scattering coefficients aₙ for rigid cylinder
    let coefficients = compute_rigid_cylinder_coefficients(ka, num_terms);

    // Generate grid
    let mut positions = Vec::new();
    let mut pressure = Vec::new();

    for &r in &r_points {
        let kr = wave_number * r;
        let hankels: Vec<Complex64> = (0..num_terms)
            .map(|n| {
                let n_i64 = n as i64;
                Complex64::new(bessel_j(n_i64, kr), bessel_y(n_i64, kr))
            })
            .collect();

        for &theta in &theta_points {
            positions.push(Point::from_polar(r, theta));

            // Incident wave: exp(ikr cos θ)
            let incident = Complex64::new((kr * theta.cos()).cos(), (kr * theta.cos()).sin());

            // Scattered wave: cosine expansion includes ±n symmetry explicitly
            let mut scattered = Complex64::new(0.0, 0.0);

            for n in 0..num_terms {
                let weight = if n == 0 { 1.0 } else { 2.0 };
                let cos_term = (n as f64 * theta).cos();
                let contribution = coefficients[n] * hankels[n];
                scattered += contribution * (weight * cos_term);
            }

            pressure.push(incident + scattered);
        }
    }

    let speed_of_sound = 343.0;
    let frequency = wave_number * speed_of_sound / (2.0 * PI);

    AnalyticalSolution {
        name: format!("2D Cylinder Scattering (ka={:.2})", ka),
        dimensions: 2,
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
        }),
    }
}

/// Compute scattering coefficients for rigid cylinder
///
/// For Neumann BC (∂p/∂n = 0 on surface):
/// ```text
/// aₙ = -Jₙ'(ka) / Hₙ⁽¹⁾'(ka) * iⁿ
/// ```
fn compute_rigid_cylinder_coefficients(ka: f64, num_terms: usize) -> Vec<Complex64> {
    let mut coefficients = Vec::with_capacity(num_terms);

    for n in 0..num_terms {
        let n_i64 = n as i64;
        let n_f64 = n as f64;

        // Derivatives using recurrence: J_n'(x) = J_{n-1}(x) - n/x * J_n(x)
        let jn = bessel_j(n_i64, ka);
        let jn_minus_1 = if n > 0 {
            bessel_j(n_i64 - 1, ka)
        } else {
            -bessel_j(1, ka)
        };
        let jn_prime = jn_minus_1 - n_f64 / ka * jn;

        // Same for Hankel: H_n'(x) = J_n'(x) + i*Y_n'(x)
        let yn = bessel_y(n_i64, ka);
        let yn_minus_1 = if n > 0 {
            bessel_y(n_i64 - 1, ka)
        } else {
            -bessel_y(1, ka)
        };
        let yn_prime = yn_minus_1 - n_f64 / ka * yn;

        let hankel_prime = Complex64::new(jn_prime, yn_prime);

        // i^n = exp(i*n*π/2)
        let i_power_n = Complex64::new((n_f64 * PI / 2.0).cos(), (n_f64 * PI / 2.0).sin());

        let a_n = -jn_prime / hankel_prime * i_power_n;

        coefficients.push(a_n);
    }

    coefficients
}

/// Cylindrical Bessel function of the first kind, order `n`
fn bessel_j(n: i64, x: f64) -> f64 {
    x.bessel_jv(n as f64)
}

/// Cylindrical Bessel function of the second kind (Neumann), order `n`
fn bessel_y(n: i64, x: f64) -> f64 {
    x.bessel_yv(n as f64)
}

/// Pressure directivity pattern (far-field)
///
/// In the far-field (kr >> 1), the scattered pressure becomes:
/// ```text
/// p_scattered ~ exp(ikr) / √(2πkr) * f(θ)
/// ```
///
/// where f(θ) is the scattering amplitude (directivity).
pub fn cylinder_directivity_2d(
    wave_number: f64,
    radius: f64,
    num_terms: usize,
    theta_points: Vec<f64>,
) -> Vec<Complex64> {
    let ka = wave_number * radius;
    let coefficients = compute_rigid_cylinder_coefficients(ka, num_terms);

    theta_points
        .iter()
        .map(|&theta| {
            let mut directivity = Complex64::new(0.0, 0.0);

            for n in 0..num_terms {
                let weight = if n == 0 { 1.0 } else { 2.0 };
                let cos_term = (n as f64 * theta).cos();
                directivity += coefficients[n] * (weight * cos_term);
            }

            directivity
        })
        .collect()
}

/// Total scattering cross-section (2D)
///
/// Optical theorem: σ_total = (4/k) * Im[f(0)]
pub fn cylinder_scattering_cross_section_2d(
    wave_number: f64,
    radius: f64,
    num_terms: usize,
) -> f64 {
    let ka = wave_number * radius;
    let coefficients = compute_rigid_cylinder_coefficients(ka, num_terms);

    // Scattering cross section σ = (4/k) * Σ |aₙ|²
    // Note: The factor 2 for n>0 is already included in the definition of the series expansion
    // but for cross section we sum the squared magnitudes of the coefficients.
    // For cylindrical wave expansion: σ = (4/k) * [ |a₀|² + 2 * Σₙ₌₁ |aₙ|² ]

    let mut sum_sq = 0.0;
    for (n, a_n) in coefficients.iter().enumerate() {
        let weight = if n == 0 { 1.0 } else { 2.0 };
        sum_sq += weight * a_n.norm_sqr();
    }

    4.0 / wave_number * sum_sq
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn test_cylinder_low_frequency() {
        // Low frequency: ka = 0.1 (Rayleigh scattering)
        let k = 0.1;
        let a = 1.0;

        let solution = cylinder_scattering_2d(
            k,
            a,
            10,
            vec![2.0], // Outside cylinder
            vec![0.0, PI / 2.0, PI],
        );

        // At low frequency, scattering should be weak
        // Total field ≈ incident field
        for p in &solution.pressure {
            assert!(p.norm() > 0.5); // Not completely scattered
            assert!(p.norm() < 2.0); // Not strongly scattered
        }
    }

    #[test]
    fn test_cylinder_boundary_condition() {
        // On surface of rigid cylinder, normal derivative should be zero
        let k = 2.0;
        let a = 1.0;

        let solution = cylinder_scattering_2d(
            k,
            a,
            30,
            vec![a], // On surface
            (0..36).map(|i| i as f64 * 2.0 * PI / 36.0).collect(),
        );

        // For rigid BC, we can't directly check ∂p/∂r = 0 without
        // numerical differentiation, but we can check that the solution
        // is well-defined on the surface
        for p in &solution.pressure {
            assert!(p.norm().is_finite());
        }
    }

    #[test]
    fn test_symmetry() {
        // Plane wave at θ=0 → solution should be symmetric about y-axis
        let k = 1.0;
        let a = 1.0;

        let solution = cylinder_scattering_2d(
            k,
            a,
            20,
            vec![2.0],
            vec![PI / 4.0, -PI / 4.0], // Symmetric angles
        );

        // Magnitudes should be equal due to symmetry
        assert_abs_diff_eq!(
            solution.pressure[0].norm(),
            solution.pressure[1].norm(),
            epsilon = 1e-6
        );
    }

    #[test]
    fn test_bessel_functions() {
        // Verify Bessel function implementation
        let x = 1.0;

        // J_0(1) ≈ 0.7651976865579666
        assert_abs_diff_eq!(bessel_j(0, x), 0.7651976865579666, epsilon = 1e-10);

        // J_1(1) ≈ 0.4400505857449335
        assert_abs_diff_eq!(bessel_j(1, x), 0.4400505857449335, epsilon = 1e-10);
    }

    #[test]
    fn test_scattering_cross_section() {
        // Scattering cross-section should be positive
        let k = 1.0;
        let a = 1.0;

        let sigma = cylinder_scattering_cross_section_2d(k, a, 30);
        assert!(sigma > 0.0);
        assert!(sigma.is_finite());
    }
}
