#[cfg(test)]
use super::super::config::XtcPluginParams;
use super::misc::SPEED_OF_SOUND;
use rustfft::num_complex::Complex;
use std::f32::consts::PI;

/// Woodworth-Schlosberg head shadowing model
///
/// Provides frequency and angle dependent interaural level difference (ILD)
/// based on spherical head acoustics.
pub(crate) fn head_shadowing_woodworth(freq: f32, angle_rad: f32, head_radius: f32) -> f32 {
    if freq <= 0.0 {
        return 1.0;
    }

    // Wave number times head radius (ka)
    // This determines the diffraction regime
    let ka = 2.0 * PI * freq * head_radius / SPEED_OF_SOUND;
    let theta = angle_rad.abs();

    if ka < 0.5 {
        // Low frequency: sound diffracts fully around head
        // Minimal ILD, slight angle dependence
        1.0 - 0.05 * ka * theta.sin()
    } else if ka < 2.0 {
        // Transition region: gradual shadowing
        let t = (ka - 0.5) / 1.5; // 0 to 1 over transition
        let shadow_factor = (1.0 + theta.cos()) / 2.0;
        let low_freq = 1.0 - 0.05 * ka * theta.sin();
        let high_freq = shadow_factor.powf(0.5 + t);
        low_freq * (1.0 - t) + high_freq * t
    } else {
        // High frequency: significant head shadow
        // Shadow increases with angle from direct path
        let shadow_factor = (1.0 + theta.cos()) / 2.0; // 1 at 0°, 0 at 180°
        // Scaled exponent aligned with validation reference data
        let exponent = (ka / 4.0).min(3.0);
        shadow_factor.powf(exponent)
    }
}

/// Dispatch head shadowing as a complex gain.
/// head_model: 0 = Woodworth magnitude only, 1 = Brown-Duda magnitude + phase.
pub(crate) fn head_shadowing_complex(
    freq: f32,
    angle_rad: f32,
    head_radius: f32,
    head_model: usize,
) -> Complex<f32> {
    match head_model {
        1 => {
            let (magnitude, phase) = head_shadowing_brown_duda(freq, angle_rad, head_radius);
            Complex::new(magnitude * phase.cos(), magnitude * phase.sin())
        }
        _ => Complex::new(head_shadowing_woodworth(freq, angle_rad, head_radius), 0.0),
    }
}

/// Brown & Duda (1998) rigid-sphere head diffraction model.
///
/// Returns a Complex gain representing both ILD (magnitude) and ITD (phase)
/// for the contralateral path. This is more accurate than Woodworth above ~1.5kHz
/// because it models frequency-dependent diffraction around a rigid sphere.
///
/// Reference: Brown, C.P. & Duda, R.O. (1998). "A structural model for binaural
/// sound synthesis." IEEE Trans. Speech & Audio Processing, 6(5), 476-488.
pub(crate) fn head_shadowing_brown_duda(freq: f32, angle_rad: f32, head_radius: f32) -> (f32, f32) {
    if freq <= 0.0 {
        return (1.0, 0.0);
    }

    let theta = angle_rad.abs();
    let w = 2.0 * PI * freq;
    let a = head_radius;
    let c = SPEED_OF_SOUND;

    // Normalized frequency parameter
    let w0 = c / a; // characteristic frequency of the head

    // --- ILD: Head shadow magnitude (Brown & Duda Eq. 2) ---
    // The magnitude transfer function is approximated by:
    //   |H(w, theta)| = alpha_min + (1 - alpha_min) * cos(theta/2)
    // where alpha_min depends on frequency:
    //   alpha_min = 1.0 / (1.0 + (w / w0)^2 / 4.0)^0.5
    // This gives ~0dB at low frequencies and increasing attenuation at high frequencies.
    let mu = (w / w0).min(20.0); // normalized frequency, capped for stability
    // Brown-Duda magnitude model (rigid-sphere diffraction approximation):
    // At low freq (mu << 1): magnitude ≈ 1 (transparent)
    // At high freq (mu >> 1): magnitude ≈ cos(theta/2) (shadow)
    let alpha_min = (1.0 + mu * mu / 4.0).recip().sqrt(); // ~1 at low freq, ~0 at high freq
    let magnitude = alpha_min + (1.0 - alpha_min) * (theta / 2.0).cos();

    // --- ITD: Interaural time delay (Woodworth formula for ITD) ---
    // Brown & Duda use the Woodworth diffraction path for time delay:
    //   tau(theta) = (a/c) * (theta + sin(theta))  for theta < pi/2
    //   tau(theta) = (a/c) * (pi/2 + sin(theta))    extrapolated
    // This gives the additional path delay for the contralateral ear.
    let tau = if theta <= PI / 2.0 {
        (a / c) * (theta + theta.sin())
    } else {
        (a / c) * (PI / 2.0 + theta.sin())
    };
    let phase = -w * tau; // negative phase = delay

    (magnitude, phase)
}

/// Head shadowing filter: low-pass filter modeling high-frequency attenuation
/// as sound diffracts around the head
#[cfg(test)]
pub(crate) fn head_shadowing_filter(freq: f32, params: &XtcPluginParams) -> f32 {
    if freq <= 0.0 {
        return 1.0;
    }

    // Simple low-pass model: g(f) = 1 / (1 + (f / f_c)^n)
    // where n is determined by slope
    let f_c = params.head_shadow_cutoff_hz;
    let slope = params.head_shadow_slope_db_per_octave;

    // Convert slope to filter order (approximately)
    let n = slope / 6.0; // 6 dB/octave ≈ 1st order

    let ratio = freq / f_c;

    1.0 / (1.0 + ratio.powf(n))
}
