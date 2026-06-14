use super::consts::DIFFUSENESS_ENERGY_FLOOR;
use super::smooth::smooth_diffuseness;
use math_audio_dsp::fast_math::fast_atan2;
use rustfft::num_complex::Complex;

/// Compute diffuseness-based gains from intensity vector analysis.
///
/// For a band of frequency bins, computes:
/// - Active intensity I = Re(P * V*) where P = pressure (mono), V = velocity (L-R)
/// - Diffuseness psi = 1 - |I| / sqrt(E_p * E_v), clamped to [0, 1]
/// - direct_gain = sqrt(1 - psi), ambient_gain = sqrt(psi)
///
/// The returned base gains satisfy direct^2 + ambient^2 = 1 before user boost controls.
#[inline]
pub(super) fn compute_diffuseness_and_doa(
    freq_left: &[Complex<f32>],
    freq_right: &[Complex<f32>],
    start_bin: usize,
    end_bin: usize,
) -> DiffusenessAndDoa {
    let mut intensity_re = 0.0_f32; // Re part of intensity (left-right axis)
    let mut intensity_im = 0.0_f32; // Im part of intensity (front-back axis)
    let mut pressure_energy = 0.0_f32;
    let mut velocity_energy = 0.0_f32;

    for i in start_bin..end_bin {
        let l = freq_left[i];
        let r = freq_right[i];

        // P = (L + R) / 2 (pressure / omnidirectional)
        let p = (l + r) * 0.5;
        // V = (L - R) / 2 (velocity / figure-of-eight)
        let v = (l - r) * 0.5;

        // Active intensity I = Re(P * conj(V))
        let pv_conj = p * v.conj();
        intensity_re += pv_conj.re;
        intensity_im += pv_conj.im;

        pressure_energy += p.norm_sqr();
        velocity_energy += v.norm_sqr();
    }

    let intensity_magnitude = (intensity_re * intensity_re + intensity_im * intensity_im).sqrt();

    // DOA angle from intensity vector
    let doa = fast_atan2(intensity_im, intensity_re);

    // Diffuseness: psi = 1 - |I| / sqrt(E_p * E_v)
    // When |I| is large relative to energies, the field is directional (psi -> 0)
    // When |I| is small, the field is diffuse (psi -> 1)
    let energy_product = (pressure_energy * velocity_energy).sqrt();
    let reliable = energy_product > DIFFUSENESS_ENERGY_FLOOR;
    let diffuseness = if reliable {
        (1.0 - intensity_magnitude / energy_product).clamp(0.0, 1.0)
    } else {
        0.0
    };

    DiffusenessAndDoa {
        diffuseness,
        doa,
        reliable,
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct DiffusenessAndDoa {
    pub(super) diffuseness: f32,
    pub(super) doa: f32,
    pub(super) reliable: bool,
}

#[cfg(test)]
impl DiffusenessAndDoa {
    #[inline(always)]
    pub(super) fn direct_gain(self) -> f32 {
        (1.0 - self.diffuseness).max(0.0).sqrt()
    }

    #[inline(always)]
    pub(super) fn ambient_gain(self) -> f32 {
        self.diffuseness.max(0.0).sqrt()
    }
}

#[inline(always)]
pub(super) fn update_diffuseness_state(
    smoothed_diffuseness: &mut f32,
    initialized: &mut bool,
    analysis: DiffusenessAndDoa,
    smoothing_scale: f32,
) -> f32 {
    if !analysis.reliable {
        return *smoothed_diffuseness;
    }

    if !*initialized {
        *smoothed_diffuseness = analysis.diffuseness;
        *initialized = true;
        analysis.diffuseness
    } else {
        let smoothed =
            smooth_diffuseness(*smoothed_diffuseness, analysis.diffuseness, smoothing_scale);
        *smoothed_diffuseness = smoothed;
        smoothed
    }
}
