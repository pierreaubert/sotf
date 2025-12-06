//! 1D Analytical Solutions
//!
//! Exact solutions for 1D wave propagation problems.

use super::{AnalyticalSolution, Point};
use num_complex::Complex64;
use std::f64::consts::PI;

/// Default speed of sound (m/s)
pub const SPEED_OF_SOUND: f64 = 343.0;

/// 1D plane wave: p(x) = exp(ikx)
///
/// This is the simplest analytical solution, representing a wave
/// traveling in the positive x-direction.
///
/// # Arguments
///
/// * `wave_number` - k = 2πf/c = ω/c
/// * `x_min` - Start of domain
/// * `x_max` - End of domain
/// * `num_points` - Number of evaluation points
///
/// # Example
///
/// ```rust
/// use math_wave::analytical::plane_wave_1d;
/// use std::f64::consts::PI;
///
/// let solution = plane_wave_1d(1.0, 0.0, 2.0 * PI, 100);
/// // At x = 0: p = exp(0) = 1
/// assert!((solution.pressure[0].re - 1.0).abs() < 1e-10);
/// ```
pub fn plane_wave_1d(
    wave_number: f64,
    x_min: f64,
    x_max: f64,
    num_points: usize,
) -> AnalyticalSolution {
    assert!(num_points >= 2, "Need at least 2 points");

    let dx = (x_max - x_min) / (num_points - 1) as f64;

    let positions: Vec<Point> = (0..num_points)
        .map(|i| Point::new_1d(x_min + i as f64 * dx))
        .collect();

    let pressure: Vec<Complex64> = positions
        .iter()
        .map(|p| {
            let kx = wave_number * p.x;
            Complex64::new(kx.cos(), kx.sin())
        })
        .collect();

    let frequency = wave_number * SPEED_OF_SOUND / (2.0 * PI);

    AnalyticalSolution {
        name: format!("1D Plane Wave (k={})", wave_number),
        dimensions: 1,
        positions,
        pressure,
        wave_number,
        frequency,
        metadata: serde_json::json!({
            "x_min": x_min,
            "x_max": x_max,
            "num_points": num_points,
            "speed_of_sound": SPEED_OF_SOUND,
            "wavelength": 2.0 * PI / wave_number,
        }),
    }
}

/// 1D standing wave: p(x) = sin(kx)
///
/// Standing wave pattern with nodes at x = nπ/k.
/// This represents the superposition of two counter-propagating waves.
///
/// # Arguments
///
/// * `wave_number` - k = 2πf/c
/// * `x_min` - Start of domain
/// * `x_max` - End of domain
/// * `num_points` - Number of evaluation points
///
/// # Example
///
/// ```rust
/// use math_wave::analytical::standing_wave_1d;
/// use std::f64::consts::PI;
///
/// let solution = standing_wave_1d(1.0, 0.0, PI, 100);
/// // At x = 0: sin(0) = 0 (node)
/// assert!(solution.pressure[0].norm() < 1e-10);
/// ```
pub fn standing_wave_1d(
    wave_number: f64,
    x_min: f64,
    x_max: f64,
    num_points: usize,
) -> AnalyticalSolution {
    assert!(num_points >= 2, "Need at least 2 points");

    let dx = (x_max - x_min) / (num_points - 1) as f64;

    let positions: Vec<Point> = (0..num_points)
        .map(|i| Point::new_1d(x_min + i as f64 * dx))
        .collect();

    let pressure: Vec<Complex64> = positions
        .iter()
        .map(|p| {
            let kx = wave_number * p.x;
            // sin(kx) represented as imaginary for consistency with wave convention
            Complex64::new(0.0, kx.sin())
        })
        .collect();

    let frequency = wave_number * SPEED_OF_SOUND / (2.0 * PI);

    AnalyticalSolution {
        name: format!("1D Standing Wave (k={})", wave_number),
        dimensions: 1,
        positions,
        pressure,
        wave_number,
        frequency,
        metadata: serde_json::json!({
            "x_min": x_min,
            "x_max": x_max,
            "num_points": num_points,
            "wavelength": 2.0 * PI / wave_number,
            "node_spacing": PI / wave_number,
        }),
    }
}

/// 1D wave with absorption: p(x) = exp(-(α + ik)x)
///
/// Includes damping term for validation of lossy media.
/// The wave decays exponentially with penetration depth 1/α.
///
/// # Arguments
///
/// * `wave_number` - k = 2πf/c
/// * `absorption` - α (damping coefficient, 1/m)
/// * `x_min` - Start of domain
/// * `x_max` - End of domain
/// * `num_points` - Number of evaluation points
///
/// # Example
///
/// ```rust
/// use math_wave::analytical::damped_wave_1d;
///
/// let solution = damped_wave_1d(1.0, 0.1, 0.0, 10.0, 100);
/// // Magnitude should decay exponentially
/// let ratio = solution.pressure[99].norm() / solution.pressure[0].norm();
/// assert!((ratio - (-0.1 * 10.0_f64).exp()).abs() < 1e-6);
/// ```
pub fn damped_wave_1d(
    wave_number: f64,
    absorption: f64,
    x_min: f64,
    x_max: f64,
    num_points: usize,
) -> AnalyticalSolution {
    assert!(num_points >= 2, "Need at least 2 points");
    assert!(absorption >= 0.0, "Absorption must be non-negative");

    let dx = (x_max - x_min) / (num_points - 1) as f64;

    let positions: Vec<Point> = (0..num_points)
        .map(|i| Point::new_1d(x_min + i as f64 * dx))
        .collect();

    let pressure: Vec<Complex64> = positions
        .iter()
        .map(|p| {
            // exp(-(α + ik)x) = exp(-αx) * exp(-ikx)
            let damping = (-absorption * p.x).exp();
            let wave = Complex64::new((wave_number * p.x).cos(), (wave_number * p.x).sin());
            damping * wave
        })
        .collect();

    let frequency = wave_number * SPEED_OF_SOUND / (2.0 * PI);

    AnalyticalSolution {
        name: format!("1D Damped Wave (k={}, α={})", wave_number, absorption),
        dimensions: 1,
        positions,
        pressure,
        wave_number,
        frequency,
        metadata: serde_json::json!({
            "x_min": x_min,
            "x_max": x_max,
            "absorption": absorption,
            "penetration_depth": if absorption > 0.0 { 1.0 / absorption } else { f64::INFINITY },
            "quality_factor": wave_number / (2.0 * absorption),
        }),
    }
}

/// 1D Helmholtz solution in a bounded domain [0, L]
///
/// Solves: d²u/dx² + k²u = f(x)
/// with Dirichlet BC: u(0) = u(L) = 0
///
/// For f(x) = sin(nπx/L), the solution is:
/// u(x) = sin(nπx/L) / (k² - (nπ/L)²)
///
/// # Arguments
///
/// * `wave_number` - k
/// * `length` - L, domain length
/// * `mode_number` - n, mode number (n >= 1)
/// * `num_points` - Number of evaluation points
pub fn helmholtz_1d_mode(
    wave_number: f64,
    length: f64,
    mode_number: usize,
    num_points: usize,
) -> AnalyticalSolution {
    assert!(num_points >= 2, "Need at least 2 points");
    assert!(mode_number >= 1, "Mode number must be >= 1");

    let n = mode_number as f64;
    let kn = n * PI / length;

    // Check for resonance
    let denom = wave_number * wave_number - kn * kn;
    assert!(
        denom.abs() > 1e-10,
        "Resonance condition: k ≈ nπ/L, solution unbounded"
    );

    let dx = length / (num_points - 1) as f64;

    let positions: Vec<Point> = (0..num_points)
        .map(|i| Point::new_1d(i as f64 * dx))
        .collect();

    let pressure: Vec<Complex64> = positions
        .iter()
        .map(|p| {
            let u = (n * PI * p.x / length).sin() / denom;
            Complex64::new(u, 0.0)
        })
        .collect();

    let frequency = wave_number * SPEED_OF_SOUND / (2.0 * PI);

    AnalyticalSolution {
        name: format!("1D Helmholtz Mode (k={}, n={})", wave_number, mode_number),
        dimensions: 1,
        positions,
        pressure,
        wave_number,
        frequency,
        metadata: serde_json::json!({
            "length": length,
            "mode_number": mode_number,
            "resonant_wavenumber": kn,
            "detuning": wave_number - kn,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn test_plane_wave_1d() {
        let k = 1.0;
        let solution = plane_wave_1d(k, 0.0, 2.0 * PI, 100);

        // Check boundary values
        assert_abs_diff_eq!(solution.pressure[0].re, 1.0, epsilon = 1e-10);
        assert_abs_diff_eq!(solution.pressure[0].im, 0.0, epsilon = 1e-10);

        // At x = 2π, should return to p = 1
        let last_idx = solution.pressure.len() - 1;
        assert_abs_diff_eq!(solution.pressure[last_idx].re, 1.0, epsilon = 1e-6);
        assert_abs_diff_eq!(solution.pressure[last_idx].im, 0.0, epsilon = 1e-6);
    }

    #[test]
    fn test_plane_wave_unit_magnitude() {
        let k = 2.5;
        let solution = plane_wave_1d(k, 0.0, 10.0, 50);

        // |exp(ikx)| = 1 for all x
        for p in &solution.pressure {
            assert_abs_diff_eq!(p.norm(), 1.0, epsilon = 1e-10);
        }
    }

    #[test]
    fn test_standing_wave_nodes() {
        let k = 1.0;
        let solution = standing_wave_1d(k, 0.0, PI, 200);

        // At x = 0, sin(0) = 0
        assert_abs_diff_eq!(solution.pressure[0].im, 0.0, epsilon = 1e-10);

        // At x = π/2, sin(π/2) = 1
        let dx = PI / 199.0;
        let target_idx = ((PI / 2.0) / dx).round() as usize;
        assert_abs_diff_eq!(solution.pressure[target_idx].im, 1.0, epsilon = 1e-4);
    }

    #[test]
    fn test_damped_wave_decay() {
        let k = 1.0;
        let alpha = 0.1;
        let solution = damped_wave_1d(k, alpha, 0.0, 10.0, 100);

        // Magnitude should decay exponentially
        let mag_start = solution.pressure[0].norm();
        let mag_end = solution.pressure[solution.pressure.len() - 1].norm();

        let expected_ratio = (-alpha * 10.0).exp();
        assert_abs_diff_eq!(mag_end / mag_start, expected_ratio, epsilon = 1e-6);
    }

    #[test]
    fn test_wavelength() {
        let k = 2.0;
        let wavelength = 2.0 * PI / k;

        let solution = plane_wave_1d(k, 0.0, wavelength, 100);

        // After one wavelength, phase should return to 0
        let p0 = solution.pressure[0];
        let p_end = solution.pressure[solution.pressure.len() - 1];

        assert_abs_diff_eq!(p0.re, p_end.re, epsilon = 1e-6);
        assert_abs_diff_eq!(p0.im, p_end.im, epsilon = 1e-6);
    }

    #[test]
    fn test_helmholtz_1d_mode() {
        let k = 1.0;
        let l = PI; // Length
        let n = 2; // Mode number

        let solution = helmholtz_1d_mode(k, l, n, 100);

        // Check boundary conditions: u(0) = u(L) = 0
        assert_abs_diff_eq!(solution.pressure[0].re, 0.0, epsilon = 1e-10);
        let last = solution.pressure.len() - 1;
        assert_abs_diff_eq!(solution.pressure[last].re, 0.0, epsilon = 1e-6);

        // Check that solution is non-zero in interior
        let mid = solution.pressure.len() / 4; // Not at a node
        assert!(solution.pressure[mid].norm() > 1e-10);
    }

    #[test]
    #[should_panic(expected = "Resonance")]
    fn test_helmholtz_resonance() {
        let l = PI;
        let k = 1.0; // k = π/L for n=1, resonance
        helmholtz_1d_mode(k, l, 1, 100);
    }
}
