//! Zoom interpolation for smooth pan/zoom transitions
//!
//! Implements the zoom interpolation algorithm from d3-interpolate-zoom.

use std::f64::consts::E;

/// Zoom view state representing (center_x, center_y, size)
///
/// The view is defined by:
/// - center_x, center_y: The center point of the view
/// - size: The size/scale of the view (larger = more zoomed out)
#[derive(Debug, Clone, Copy, Default)]
pub struct ZoomView {
    /// Center X coordinate
    pub cx: f64,
    /// Center Y coordinate
    pub cy: f64,
    /// View size (scale factor)
    pub size: f64,
}

impl ZoomView {
    /// Create a new zoom view.
    pub fn new(cx: f64, cy: f64, size: f64) -> Self {
        Self { cx, cy, size }
    }

    /// Get the coordinates as a tuple (cx, cy, size).
    pub fn as_tuple(&self) -> (f64, f64, f64) {
        (self.cx, self.cy, self.size)
    }
}

/// Parameters for zoom interpolation.
#[derive(Debug, Clone, Copy)]
pub struct ZoomParams {
    /// Rho parameter controlling the curvature of the zoom trajectory.
    /// Higher values result in more pronounced zoom-out then zoom-in.
    /// Default is sqrt(2) as recommended by van Wijk and Nuij.
    pub rho: f64,
}

impl Default for ZoomParams {
    fn default() -> Self {
        Self { rho: 2.0_f64.sqrt() }
    }
}

/// Create a zoom interpolator between two views.
///
/// Implements the optimal pan-zoom trajectory from:
/// van Wijk, Jarke J., and Wim AA Nuij. "Smooth and efficient zooming and panning."
/// IEEE Symposium on Information Visualization, 2003.
///
/// # Example
///
/// ```
/// use d3rs::interpolate::zoom::{ZoomView, interpolate_zoom};
///
/// let view0 = ZoomView::new(0.0, 0.0, 100.0);  // Zoomed out, centered at origin
/// let view1 = ZoomView::new(50.0, 50.0, 10.0); // Zoomed in, centered at (50,50)
///
/// let interp = interpolate_zoom(view0, view1);
///
/// let mid = interp(0.5);
/// // Mid-point of the zoom transition
/// ```
pub fn interpolate_zoom(view0: ZoomView, view1: ZoomView) -> impl Fn(f64) -> ZoomView {
    interpolate_zoom_with_params(view0, view1, ZoomParams::default())
}

/// Create a zoom interpolator with custom parameters.
///
/// # Example
///
/// ```
/// use d3rs::interpolate::zoom::{ZoomView, ZoomParams, interpolate_zoom_with_params};
///
/// let view0 = ZoomView::new(0.0, 0.0, 100.0);
/// let view1 = ZoomView::new(50.0, 50.0, 10.0);
/// let params = ZoomParams { rho: 1.5 }; // Less pronounced zoom out
///
/// let interp = interpolate_zoom_with_params(view0, view1, params);
/// let mid = interp(0.5);
/// ```
pub fn interpolate_zoom_with_params(
    view0: ZoomView,
    view1: ZoomView,
    params: ZoomParams,
) -> impl Fn(f64) -> ZoomView {
    let ux0 = view0.cx;
    let uy0 = view0.cy;
    let w0 = view0.size;

    let ux1 = view1.cx;
    let uy1 = view1.cy;
    let w1 = view1.size;

    let dx = ux1 - ux0;
    let dy = uy1 - uy0;
    let d2 = dx * dx + dy * dy;
    let d1 = d2.sqrt();

    let rho = params.rho;
    let rho2 = rho * rho;
    let rho4 = rho2 * rho2;

    // Special case: same position, just scale
    if d2 < 1e-12 {
        let s = (w1 / w0).ln() / rho;
        let duration = s.abs();

        return Box::new(move |t: f64| {
            if duration < 1e-12 {
                return view1;
            }
            let w = w0 * (rho * t * s).exp();
            ZoomView::new(ux1, uy1, w)
        }) as Box<dyn Fn(f64) -> ZoomView>;
    }

    // General case: pan and zoom
    let b0 = (w1 * w1 - w0 * w0 + rho4 * d2) / (2.0 * w0 * rho2 * d1);
    let b1 = (w1 * w1 - w0 * w0 - rho4 * d2) / (2.0 * w1 * rho2 * d1);

    let r0 = (b0 * b0 + 1.0).sqrt().ln() - b0.asinh();
    let r1 = (b1 * b1 + 1.0).sqrt().ln() - b1.asinh();
    let s = (r1 - r0) / rho;
    let duration = s.abs();

    Box::new(move |t: f64| {
        if duration < 1e-12 {
            return view1;
        }

        let s_t = t * s;

        // Helper: cosh(x) = (e^x + e^-x) / 2
        let cosh = |x: f64| (E.powf(x) + E.powf(-x)) / 2.0;

        // Helper: sinh(x) = (e^x - e^-x) / 2
        let sinh = |x: f64| (E.powf(x) - E.powf(-x)) / 2.0;

        // u(s) = w0 / (rho^2 * d1) * (cosh(r0) * tanh(r0 + rho*s) - sinh(r0))
        let u = |s_val: f64| {
            let coshr_r0 = cosh(r0);
            w0 / (rho2 * d1) * (coshr_r0 * (r0 + rho * s_val).tanh() - sinh(r0))
        };

        let u_t = u(s_t);
        let w = w0 * cosh(r0) / cosh(r0 + rho * s_t);

        ZoomView::new(
            ux0 + u_t * dx,
            uy0 + u_t * dy,
            w,
        )
    }) as Box<dyn Fn(f64) -> ZoomView>
}

/// Calculate the duration of a zoom transition.
///
/// Returns a normalized duration value that represents the "work" required
/// to perform the zoom transition. Useful for timing animations.
///
/// # Example
///
/// ```
/// use d3rs::interpolate::zoom::{ZoomView, zoom_duration};
///
/// let view0 = ZoomView::new(0.0, 0.0, 100.0);
/// let view1 = ZoomView::new(50.0, 50.0, 10.0);
///
/// let duration = zoom_duration(view0, view1);
/// // Duration in normalized units
/// ```
pub fn zoom_duration(view0: ZoomView, view1: ZoomView) -> f64 {
    zoom_duration_with_rho(view0, view1, 2.0_f64.sqrt())
}

/// Calculate zoom duration with custom rho parameter.
pub fn zoom_duration_with_rho(view0: ZoomView, view1: ZoomView, rho: f64) -> f64 {
    let dx = view1.cx - view0.cx;
    let dy = view1.cy - view0.cy;
    let d2 = dx * dx + dy * dy;
    let d1 = d2.sqrt();

    let w0 = view0.size;
    let w1 = view1.size;

    let rho2 = rho * rho;
    let rho4 = rho2 * rho2;

    if d2 < 1e-12 {
        // Same position, just scale
        return ((w1 / w0).ln() / rho).abs();
    }

    let b0 = (w1 * w1 - w0 * w0 + rho4 * d2) / (2.0 * w0 * rho2 * d1);
    let b1 = (w1 * w1 - w0 * w0 - rho4 * d2) / (2.0 * w1 * rho2 * d1);

    let r0 = (b0 * b0 + 1.0).sqrt().ln() - b0.asinh();
    let r1 = (b1 * b1 + 1.0).sqrt().ln() - b1.asinh();

    ((r1 - r0) / rho).abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zoom_same_position() {
        let view0 = ZoomView::new(50.0, 50.0, 100.0);
        let view1 = ZoomView::new(50.0, 50.0, 10.0);

        let interp = interpolate_zoom(view0, view1);

        let start = interp(0.0);
        assert!((start.cx - 50.0).abs() < 0.001);
        assert!((start.size - 100.0).abs() < 0.001);

        let end = interp(1.0);
        assert!((end.cx - 50.0).abs() < 0.001);
        assert!((end.size - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_zoom_pan_only() {
        let view0 = ZoomView::new(0.0, 0.0, 100.0);
        let view1 = ZoomView::new(100.0, 0.0, 100.0);

        let interp = interpolate_zoom(view0, view1);

        let start = interp(0.0);
        assert!((start.cx - 0.0).abs() < 0.001);
        assert!((start.size - 100.0).abs() < 0.5);

        let end = interp(1.0);
        assert!((end.cx - 100.0).abs() < 0.001);
        assert!((end.size - 100.0).abs() < 0.5);
    }

    #[test]
    fn test_zoom_pan_and_scale() {
        let view0 = ZoomView::new(0.0, 0.0, 100.0);
        let view1 = ZoomView::new(50.0, 50.0, 10.0);

        let interp = interpolate_zoom(view0, view1);

        let mid = interp(0.5);
        // Mid-point should be between start and end values
        assert!(mid.cx >= 0.0 && mid.cx <= 50.0);
        assert!(mid.cy >= 0.0 && mid.cy <= 50.0);
    }

    #[test]
    fn test_zoom_duration() {
        let view0 = ZoomView::new(0.0, 0.0, 100.0);
        let view1 = ZoomView::new(50.0, 50.0, 10.0);

        let duration = zoom_duration(view0, view1);
        assert!(duration > 0.0);

        // Same position, different scale
        let view2 = ZoomView::new(0.0, 0.0, 10.0);
        let duration2 = zoom_duration(view0, view2);
        assert!(duration2 > 0.0);
    }
}
