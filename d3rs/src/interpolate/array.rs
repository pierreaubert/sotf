//! Array interpolation functions
//!
//! Provides interpolation between arrays of values.

use super::number::Interpolate;

/// Interpolate between two arrays of values.
///
/// Both arrays should have the same length. If they differ, the result
/// will have the length of the shorter array.
///
/// # Example
///
/// ```
/// use d3rs::interpolate::interpolate_array;
///
/// let a = vec![0.0, 0.0, 0.0];
/// let b = vec![100.0, 200.0, 300.0];
/// let interp = interpolate_array(&a, &b);
///
/// let mid = interp(0.5);
/// assert_eq!(mid, vec![50.0, 100.0, 150.0]);
/// ```
pub fn interpolate_array<'a, T: Interpolate>(a: &'a [T], b: &'a [T]) -> impl Fn(f64) -> Vec<T> + 'a {
    let len = a.len().min(b.len());
    move |t| (0..len).map(|i| a[i].interpolate(&b[i], t)).collect()
}

/// Interpolate between two arrays of f64 values (convenience function).
///
/// # Example
///
/// ```
/// use d3rs::interpolate::interpolate_number_array;
///
/// let a = vec![0.0, 10.0, 20.0];
/// let b = vec![100.0, 110.0, 120.0];
/// let interp = interpolate_number_array(a, b);
///
/// let result = interp(0.5);
/// assert_eq!(result, vec![50.0, 60.0, 70.0]);
/// ```
pub fn interpolate_number_array(a: Vec<f64>, b: Vec<f64>) -> impl Fn(f64) -> Vec<f64> {
    let len = a.len().min(b.len());
    move |t| (0..len).map(|i| a[i] + (b[i] - a[i]) * t).collect()
}

/// Interpolate arrays element-wise with different interpolators.
///
/// Each element can have its own interpolation function.
pub struct ArrayInterpolator<T> {
    interpolators: Vec<Box<dyn Fn(f64) -> T>>,
}

impl<T> ArrayInterpolator<T> {
    /// Create a new array interpolator from a list of interpolator functions.
    pub fn new(interpolators: Vec<Box<dyn Fn(f64) -> T>>) -> Self {
        Self { interpolators }
    }

    /// Interpolate at parameter t.
    pub fn interpolate(&self, t: f64) -> Vec<T> {
        self.interpolators.iter().map(|interp| interp(t)).collect()
    }
}

/// Interpolate a 2D array (matrix) of values.
///
/// # Example
///
/// ```
/// use d3rs::interpolate::interpolate_matrix;
///
/// let a = vec![vec![0.0, 1.0], vec![2.0, 3.0]];
/// let b = vec![vec![10.0, 11.0], vec![12.0, 13.0]];
/// let interp = interpolate_matrix(&a, &b);
///
/// let mid = interp(0.5);
/// assert_eq!(mid[0][0], 5.0);
/// assert_eq!(mid[1][1], 8.0);
/// ```
pub fn interpolate_matrix<'a>(a: &'a [Vec<f64>], b: &'a [Vec<f64>]) -> impl Fn(f64) -> Vec<Vec<f64>> + 'a {
    let rows = a.len().min(b.len());
    move |t| {
        (0..rows)
            .map(|i| {
                let cols = a[i].len().min(b[i].len());
                (0..cols).map(|j| a[i][j] + (b[i][j] - a[i][j]) * t).collect()
            })
            .collect()
    }
}

/// Zoom interpolation for 2D views [cx, cy, width].
///
/// This interpolates between two "zoom views" defined as [centerX, centerY, width].
/// Uses exponential interpolation for smooth zooming.
///
/// # Example
///
/// ```
/// use d3rs::interpolate::interpolate_zoom;
///
/// // Zoom from view at (0,0) with width 100 to view at (0,0) with width 10 (same position)
/// let interp = interpolate_zoom([0.0, 0.0, 100.0], [0.0, 0.0, 10.0]);
///
/// let start = interp(0.0);
/// assert!((start[0] - 0.0).abs() < 0.001);
/// assert!((start[2] - 100.0).abs() < 0.001);
/// ```
pub fn interpolate_zoom(p0: [f64; 3], p1: [f64; 3]) -> impl Fn(f64) -> [f64; 3] {
    let [x0, y0, w0] = p0;
    let [x1, y1, w1] = p1;

    let dx = x1 - x0;
    let dy = y1 - y0;
    let d = (dx * dx + dy * dy).sqrt();

    let rho = 1.4; // Zoom constant
    let rho2 = rho * rho;

    // Special case: no movement
    if d < 1e-6 {
        // Simple width interpolation
        let s = (w1 / w0).ln() / rho;
        return Box::new(move |t: f64| {
            let w = w0 * (rho * s * t).exp();
            [x0, y0, w]
        }) as Box<dyn Fn(f64) -> [f64; 3]>;
    }

    let b0 = (w1 * w1 - w0 * w0 + rho2 * rho2 * d * d) / (2.0 * w0 * rho2 * d);
    let b1 = (w1 * w1 - w0 * w0 - rho2 * rho2 * d * d) / (2.0 * w1 * rho2 * d);
    let r0 = (b0 * b0 + 1.0).sqrt().ln() - b0.asinh();
    let r1 = (b1 * b1 + 1.0).sqrt().ln() - b1.asinh();
    let s = (r1 - r0) / rho;

    Box::new(move |t: f64| {
        let s_t = r0 + s * rho * t;
        let u = w0 / (rho2 * d) * ((s_t.exp() - (-s_t).exp()) / 2.0 - b0.sinh());
        let w = w0 * (b0.cosh()) / ((s_t).cosh());
        [x0 + dx * u, y0 + dy * u, w]
    }) as Box<dyn Fn(f64) -> [f64; 3]>
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate_array() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![100.0, 200.0, 300.0];
        let interp = interpolate_array(&a, &b);

        assert_eq!(interp(0.0), vec![0.0, 0.0, 0.0]);
        assert_eq!(interp(0.5), vec![50.0, 100.0, 150.0]);
        assert_eq!(interp(1.0), vec![100.0, 200.0, 300.0]);
    }

    #[test]
    fn test_interpolate_matrix() {
        let a = vec![vec![0.0, 0.0], vec![0.0, 0.0]];
        let b = vec![vec![10.0, 20.0], vec![30.0, 40.0]];
        let interp = interpolate_matrix(&a, &b);

        let mid = interp(0.5);
        assert_eq!(mid[0][0], 5.0);
        assert_eq!(mid[0][1], 10.0);
        assert_eq!(mid[1][0], 15.0);
        assert_eq!(mid[1][1], 20.0);
    }

    #[test]
    fn test_interpolate_zoom() {
        let interp = interpolate_zoom([0.0, 0.0, 100.0], [100.0, 100.0, 10.0]);

        let start = interp(0.0);
        assert!((start[0] - 0.0).abs() < 0.01);
        assert!((start[1] - 0.0).abs() < 0.01);
        assert!((start[2] - 100.0).abs() < 0.01);

        let end = interp(1.0);
        assert!((end[0] - 100.0).abs() < 0.01);
        assert!((end[1] - 100.0).abs() < 0.01);
        assert!((end[2] - 10.0).abs() < 0.01);
    }
}
