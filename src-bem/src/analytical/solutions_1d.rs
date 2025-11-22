//! 1D Analytical Solutions
//!
//! Exact solutions for 1D wave propagation problems.

use super::{AnalyticalSolution, Point};
use num_complex::Complex64;
use std::f64::consts::PI;

/// 1D plane wave: p(x) = exp(ikx)
///
/// This is the simplest analytical solution, useful for validating
/// basic BEM implementation.
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
/// use bem::analytical::plane_wave_1d;
///
/// let solution = plane_wave_1d(1.0, 0.0, 10.0, 100);
/// assert_eq!(solution.positions.len(), 100);
/// ```
pub fn plane_wave_1d(
    wave_number: f64,
    x_min: f64,
    x_max: f64,
    num_points: usize,
) -> AnalyticalSolution {
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

    // Assuming c = 343 m/s
    let speed_of_sound = 343.0;
    let frequency = wave_number * speed_of_sound / (2.0 * PI);

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
            "speed_of_sound": speed_of_sound,
        }),
    }
}

/// 1D standing wave: p(x) = sin(kx)
///
/// Standing wave pattern with nodes at x = nπ/k.
///
/// # Arguments
///
/// * `wave_number` - k = 2πf/c
/// * `x_min` - Start of domain
/// * `x_max` - End of domain
/// * `num_points` - Number of evaluation points
pub fn standing_wave_1d(
    wave_number: f64,
    x_min: f64,
    x_max: f64,
    num_points: usize,
) -> AnalyticalSolution {
    let dx = (x_max - x_min) / (num_points - 1) as f64;

    let positions: Vec<Point> = (0..num_points)
        .map(|i| Point::new_1d(x_min + i as f64 * dx))
        .collect();

    let pressure: Vec<Complex64> = positions
        .iter()
        .map(|p| {
            let kx = wave_number * p.x;
            // sin(kx) = (e^(ikx) - e^(-ikx)) / 2i
            Complex64::new(0.0, kx.sin())
        })
        .collect();

    let speed_of_sound = 343.0;
    let frequency = wave_number * speed_of_sound / (2.0 * PI);

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
        }),
    }
}

/// 1D wave with absorption: p(x) = exp(-(α + ik)x)
///
/// Includes damping term for validation of lossy media.
///
/// # Arguments
///
/// * `wave_number` - k = 2πf/c
/// * `absorption` - α (damping coefficient)
/// * `x_min` - Start of domain
/// * `x_max` - End of domain
/// * `num_points` - Number of evaluation points
pub fn damped_wave_1d(
    wave_number: f64,
    absorption: f64,
    x_min: f64,
    x_max: f64,
    num_points: usize,
) -> AnalyticalSolution {
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

    let speed_of_sound = 343.0;
    let frequency = wave_number * speed_of_sound / (2.0 * PI);

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
            "penetration_depth": 1.0 / absorption,
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
    fn test_standing_wave_nodes() {
        let k = 1.0;
        let solution = standing_wave_1d(k, 0.0, PI, 100);

        // At x = 0, sin(0) = 0
        assert_abs_diff_eq!(solution.pressure[0].im, 0.0, epsilon = 1e-10);

        // At x = π/2, sin(π/2) = 1. Pick the grid point closest to π/2.
        let dx = (PI - 0.0) / (100 - 1) as f64;
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
}
