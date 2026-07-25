use rustfft::num_complex::Complex;

/// Pre-computed HRTF transfer functions for the XTC plant matrix.
///
/// When a SOFA/HRTF file is loaded, these frequency-domain transfer functions
/// replace the Woodworth analytical model for computing the crosstalk matrix C(f).
#[derive(Clone)]
pub(crate) struct HrtfTransferFunctions {
    /// Speaker L -> Left ear (ipsilateral)
    pub h_ll: Vec<Complex<f32>>,
    /// Speaker R -> Left ear (contralateral)
    pub h_lr: Vec<Complex<f32>>,
    /// Speaker L -> Right ear (contralateral)
    pub h_rl: Vec<Complex<f32>>,
    /// Speaker R -> Right ear (ipsilateral)
    pub h_rr: Vec<Complex<f32>>,
}

#[cfg(test)]
pub(crate) type XtcFilterSet = (
    Vec<Complex<f32>>,
    Vec<Complex<f32>>,
    Vec<Complex<f32>>,
    Vec<Complex<f32>>,
);

/// Cached geometry values to avoid repeated computation in the hot loop.
///
/// Optimization 3: Pre-compute all geometry-dependent values that don't change
/// per frequency bin, avoiding redundant sqrt/trig operations.
pub(crate) struct GeometryCache {
    pub freq_per_bin: f32,
    // Symmetric geometry (yaw ~= 0)
    pub symmetric: SymmetricGeometry,
    // Asymmetric geometry (yaw != 0), computed lazily
    pub asymmetric: Option<AsymmetricGeometry>,
}

/// Geometry values for symmetric XTC (yaw ~= 0).
pub(crate) struct SymmetricGeometry {
    pub a: f32,
    pub amplitude_ratio: f32,
    pub delay_ipsi: f32,
    pub delay_contra: f32,
    pub contra_angle: f32,
}

/// Geometry values for asymmetric XTC (yaw != 0).
pub(crate) struct AsymmetricGeometry {
    pub a: f32,
    pub theta_left: f32,
    pub theta_right: f32,
    pub amplitude_ratio_left: f32,
    pub delay_left_ipsi: f32,
    pub delay_left_contra: f32,
    pub angle_left_contra: f32,
    pub amplitude_ratio_right: f32,
    pub delay_right_ipsi: f32,
    pub delay_right_contra: f32,
    pub angle_right_contra: f32,
}
