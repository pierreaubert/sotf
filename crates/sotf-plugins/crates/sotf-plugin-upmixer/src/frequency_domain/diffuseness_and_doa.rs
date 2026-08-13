use super::consts::DIFFUSENESS_ENERGY_FLOOR;
use super::smooth::smooth_diffuseness;
use rustfft::num_complex::Complex;

/// Derive the spatial information that ordinary stereo actually contains.
///
/// `lateral_balance` is the signed L/R energy imbalance. `directness` combines
/// that cue with positive, in-phase correlation. Negative correlation and
/// quadrature correlation are deliberately not promoted to physical source
/// directions: two playback channels do not encode a front/back velocity axis.
#[inline(always)]
pub(super) fn stereo_spatial_cue(
    left_energy: f32,
    right_energy: f32,
    cross_real: f32,
) -> Option<(f32, f32)> {
    let total_energy = left_energy + right_energy;
    if !total_energy.is_finite() || total_energy <= DIFFUSENESS_ENERGY_FLOOR {
        return None;
    }

    let lateral_balance = ((left_energy - right_energy) / total_energy).clamp(-1.0, 1.0);
    let positive_correlation = (2.0 * cross_real / total_energy).clamp(0.0, 1.0);
    let directness = lateral_balance.abs().max(positive_correlation);
    Some((directness, lateral_balance))
}

/// Compute diffuseness-based gains from intensity vector analysis.
///
/// For a band of frequency bins, computes:
/// - directness = max(positive real L/R correlation, signed level-imbalance magnitude)
/// - Diffuseness psi = 1 - directness, clamped to [0, 1]
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
    let mut left_energy = 0.0_f32;
    let mut right_energy = 0.0_f32;
    let mut cross_real = 0.0_f32;

    for i in start_bin..end_bin {
        let l = freq_left[i];
        let r = freq_right[i];

        left_energy += l.norm_sqr();
        right_energy += r.norm_sqr();
        cross_real += (l * r.conj()).re;
    }

    let cue = stereo_spatial_cue(left_energy, right_energy, cross_real);
    let reliable = cue.is_some();
    let (diffuseness, doa) = cue.map_or((0.0, 0.0), |(directness, lateral_balance)| {
        // Preserve the existing panner's radians contract while limiting stereo
        // localization to the physically observable left/right hemisphere.
        (
            1.0 - directness,
            lateral_balance * std::f32::consts::FRAC_PI_2,
        )
    });

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
