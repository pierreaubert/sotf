// ============================================================================
// Frequency Domain Processing with ERB Bands
// ============================================================================
//
// Uses intensity-vector-based direction-of-arrival (DOA) estimation per ERB band
// and diffuseness-based direct/ambient decomposition.
//
// For each ERB band:
// 1. Compute active intensity vector I = Re(P * V*) where P = (L+R)/2, V = (L-R)/2
// 2. DOA angle = atan2(I_y, I_x) gives per-band panning direction
// 3. Diffuseness psi = 1 - |I| / (P_energy * V_energy) measures how diffuse the field is
// 4. Base split gains are sqrt(1 - psi) and sqrt(psi); user boost controls are applied later.

use super::UpmixerPlugin;
use math_audio_dsp::fast_math::{fast_atan2, fast_cos, fast_sin};
use sotf_host::simd::compute_covariance_simd;

use rustfft::num_complex::Complex;

/// Minimum height mask value -- prevents deep spectral notches that cause time-domain ringing
pub(super) const HEIGHT_MASK_FLOOR: f32 = 0.10;

/// One-pole smoothing factor for median-filtered coherence (per-band)
const COHERENCE_SMOOTHING_ALPHA: f32 = 0.15;

/// Base steering attack alpha at lowest frequency (100 Hz)
const STEERING_ATTACK_BASE: f32 = 0.3;
/// Steering attack alpha range scaled by frequency norm
const STEERING_ATTACK_RANGE: f32 = 0.4;
/// Base steering release alpha at lowest frequency (100 Hz)
const STEERING_RELEASE_BASE: f32 = 0.02;
/// Steering release alpha range scaled by frequency norm
const STEERING_RELEASE_RANGE: f32 = 0.06;

/// Smoothing alpha for DOA angle tracking (one-pole filter)
const DOA_SMOOTHING_ALPHA: f32 = 0.2;
/// Smoothing alpha for diffuseness, which directly modulates ambient/height routing.
const DIFFUSENESS_SMOOTHING_ALPHA: f32 = 0.18;
const DIFFUSENESS_MAX_STEP: f32 = 0.08;
const DIFFUSENESS_ENERGY_FLOOR: f32 = 1e-12;
/// Dialogue control smoothing for spatial decomposition. This is intentionally
/// slower than detector smoothing because it modulates center, ambience, and
/// decorrelation over the whole sound field.
const DIALOGUE_SPATIAL_ATTACK_ALPHA: f32 = 0.06;
const DIALOGUE_SPATIAL_RELEASE_ALPHA: f32 = 0.025;
const DIALOGUE_SPATIAL_MAX_RISE: f32 = 0.035;
const DIALOGUE_SPATIAL_MAX_FALL: f32 = 0.018;
const DIALOGUE_SPATIAL_DEADBAND: f32 = 0.004;

/// 5-element median using 6 comparisons (optimal).
/// After eliminating the global minimum via 3 compare-swaps on pairs,
/// finds the 2nd-smallest of the remaining 4 elements (= median of 5).
#[inline(always)]
fn median5(arr: [f32; 5]) -> f32 {
    let [mut a, mut b, mut c, mut d, mut e] = arr;
    // Sort pairs: a <= b, c <= d                        (2 comparisons)
    if a > b {
        std::mem::swap(&mut a, &mut b);
    }
    if c > d {
        std::mem::swap(&mut c, &mut d);
    }
    // Order pairs so a <= c (thus a = min of {a,b,c,d}) (1 comparison)
    if a > c {
        std::mem::swap(&mut a, &mut c);
        std::mem::swap(&mut b, &mut d);
    }
    // Discard a (global minimum). Need 2nd-smallest of {b, c, d, e} where c <= d.
    // Sort b,e so b <= e                                (1 comparison)
    if b > e {
        std::mem::swap(&mut b, &mut e);
    }
    // Now b <= e, c <= d. 2nd-of-4 from two sorted pairs:
    // merge-pick index 1 = if b <= c then min(c, e) else min(b, d)
    if b <= c {
        // (1 comparison)
        if c <= e { c } else { e } // (1 comparison)
    } else if b <= d {
        b
    } else {
        d
    }
}

/// Compute diffuseness-based gains from intensity vector analysis.
///
/// For a band of frequency bins, computes:
/// - Active intensity I = Re(P * V*) where P = pressure (mono), V = velocity (L-R)
/// - Diffuseness psi = 1 - |I| / sqrt(E_p * E_v), clamped to [0, 1]
/// - direct_gain = sqrt(1 - psi), ambient_gain = sqrt(psi)
///
/// The returned base gains satisfy direct^2 + ambient^2 = 1 before user boost controls.
#[inline]
fn compute_diffuseness_and_doa(
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
struct DiffusenessAndDoa {
    diffuseness: f32,
    doa: f32,
    reliable: bool,
}

#[cfg(test)]
impl DiffusenessAndDoa {
    #[inline(always)]
    fn direct_gain(self) -> f32 {
        (1.0 - self.diffuseness).max(0.0).sqrt()
    }

    #[inline(always)]
    fn ambient_gain(self) -> f32 {
        self.diffuseness.max(0.0).sqrt()
    }
}

#[inline(always)]
fn smooth_dialogue_spatial_control(previous: f32, target: f32) -> f32 {
    let target = target.clamp(0.0, 1.0);
    let diff = target - previous;
    if diff.abs() <= DIALOGUE_SPATIAL_DEADBAND {
        return previous;
    }

    let alpha = if diff > 0.0 {
        DIALOGUE_SPATIAL_ATTACK_ALPHA
    } else {
        DIALOGUE_SPATIAL_RELEASE_ALPHA
    };
    let smoothed = previous + alpha * diff;
    let limited_diff =
        (smoothed - previous).clamp(-DIALOGUE_SPATIAL_MAX_FALL, DIALOGUE_SPATIAL_MAX_RISE);
    (previous + limited_diff).clamp(0.0, 1.0)
}

#[inline(always)]
fn smooth_diffuseness(previous: f32, target: f32, smoothing_scale: f32) -> f32 {
    let target = target.clamp(0.0, 1.0);
    let alpha = (DIFFUSENESS_SMOOTHING_ALPHA * smoothing_scale).max(0.035);
    let max_step = (DIFFUSENESS_MAX_STEP * smoothing_scale).max(0.025);
    let smoothed = previous + alpha * (target - previous);
    let limited_diff = (smoothed - previous).clamp(-max_step, max_step);
    (previous + limited_diff).clamp(0.0, 1.0)
}

#[inline(always)]
fn update_diffuseness_state(
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

#[inline(always)]
fn ambient_gain_with_controls(diffuseness: f32, ambient_boost: f32, dialogue_control: f32) -> f32 {
    diffuseness.max(0.0).sqrt() * ambient_boost * (1.0 - dialogue_control)
}

#[inline(always)]
fn principal_eigenvector(
    c_xx: f32,
    c_yy: f32,
    c_xy: Complex<f32>,
    lambda1: f32,
) -> (Complex<f32>, Complex<f32>) {
    if c_xy.norm_sqr() > 1e-18 {
        let v = lambda1 - c_xx;
        let norm = (c_xy.norm_sqr() + v * v).sqrt();
        if norm > 1e-9 {
            (c_xy / norm, Complex::new(v / norm, 0.0))
        } else if c_xx >= c_yy {
            (Complex::new(1.0, 0.0), Complex::new(0.0, 0.0))
        } else {
            (Complex::new(0.0, 0.0), Complex::new(1.0, 0.0))
        }
    } else if c_xx >= c_yy {
        (Complex::new(1.0, 0.0), Complex::new(0.0, 0.0))
    } else {
        (Complex::new(0.0, 0.0), Complex::new(1.0, 0.0))
    }
}

#[inline(always)]
fn transition_crossfade_weight(bin: usize, transition_start: usize, transition_width: f32) -> f32 {
    if transition_width <= 0.0 {
        return 1.0;
    }
    let linear_t = ((bin - transition_start) as f32 / transition_width).clamp(0.0, 1.0);
    0.5 - 0.5 * fast_cos(std::f32::consts::PI * linear_t)
}

#[inline(always)]
fn normalize_decorrelation_blend(blended: Complex<f32>) -> Complex<f32> {
    let mag_sq = blended.norm_sqr();
    if mag_sq > 1e-9 {
        blended * (1.0 / mag_sq.sqrt())
    } else {
        Complex::new(1.0, 0.0)
    }
}

#[inline(always)]
fn bin_intensity_doa(left: Complex<f32>, right: Complex<f32>) -> Option<f32> {
    let p = (left + right) * 0.5;
    let v = (left - right) * 0.5;
    let energy_product = (p.norm_sqr() * v.norm_sqr()).sqrt();
    if energy_product <= DIFFUSENESS_ENERGY_FLOOR {
        return None;
    }

    let intensity = p * v.conj();
    Some(fast_atan2(intensity.im, intensity.re))
}

impl UpmixerPlugin {
    pub(super) fn process_frequency_domain_erb_bands(&mut self) {
        if self.sample_rate == 0 || self.fft_size == 0 {
            return;
        }

        if self.decorrelation_mode == 1 {
            self.update_lfo_decorrelation();
        }

        // Zero direct2/direct2_doa_per_bin at frame start: ensures LFE/pass-through bins remain
        // silent, and handles the case when multi_source_extraction is toggled off mid-stream.
        let spec_size_pre = self.fft_size / 2 + 1;
        self.direct2[..spec_size_pre].fill(Complex::new(0.0, 0.0));
        self.direct2_doa_per_bin[..spec_size_pre].fill(0.0);

        let lfe_cutoff_bin = self.cached_lfe_cutoff_bin;
        let bandpass_bin = self.cached_bandpass_bin;
        let freq_per_bin = self.cached_freq_per_bin;
        let analysis_smoothing_scale = self.analysis_smoothing_scale();
        let coherence_smoothing_alpha =
            (COHERENCE_SMOOTHING_ALPHA * analysis_smoothing_scale).max(0.03);
        let doa_smoothing_alpha = (DOA_SMOOTHING_ALPHA * analysis_smoothing_scale).max(0.04);
        self.dialogue_spatial_control = smooth_dialogue_spatial_control(
            self.dialogue_spatial_control,
            self.dialogue_probability,
        );
        let dialogue_control = self.dialogue_spatial_control * self.dialogue_weight.current();

        for band_idx in 0..self.erb_bands.len() {
            let start_bin = self.erb_bands[band_idx];
            let end_bin = if band_idx + 1 < self.erb_bands.len() {
                self.erb_bands[band_idx + 1]
            } else {
                self.fft_size / 2 + 1
            };
            if start_bin >= end_bin || start_bin > self.fft_size / 2 {
                continue;
            }

            // Covariance for steering smoothing (attack/release detection)
            let (cov_xx, cov_yy, cov_xy) = compute_covariance_simd(
                &self.freq_domain_left,
                &self.freq_domain_right,
                start_bin,
                end_bin,
            );

            let inst_energy = cov_xx + cov_yy;
            let smooth_energy = self.pca_cov_xx[band_idx] + self.pca_cov_yy[band_idx];
            let center_bin = (start_bin + end_bin) / 2;
            let center_freq = center_bin as f32 * freq_per_bin;
            let norm = ((center_freq - 100.0) / (8000.0 - 100.0)).clamp(0.0, 1.0);
            let attack_alpha = ((STEERING_ATTACK_BASE + STEERING_ATTACK_RANGE * norm)
                * analysis_smoothing_scale)
                .max(0.05);
            let release_alpha = ((STEERING_RELEASE_BASE + STEERING_RELEASE_RANGE * norm)
                * analysis_smoothing_scale)
                .max(0.005);
            let alpha = if inst_energy > smooth_energy * 1.5 {
                attack_alpha
            } else {
                release_alpha
            };
            // Only written for test inspection; not read in the processing path.
            #[cfg(test)]
            {
                self.steering_alphas[band_idx] = alpha;
            }

            self.pca_cov_xx[band_idx] = (1.0 - alpha) * self.pca_cov_xx[band_idx] + alpha * cov_xx;
            self.pca_cov_yy[band_idx] = (1.0 - alpha) * self.pca_cov_yy[band_idx] + alpha * cov_yy;
            self.pca_cov_xy[band_idx] = (1.0 - alpha) * self.pca_cov_xy[band_idx] + alpha * cov_xy;

            // Compute coherence from smoothed covariance (used for median filtering)
            let c_xx = self.pca_cov_xx[band_idx];
            let c_yy = self.pca_cov_yy[band_idx];
            let c_xy = self.pca_cov_xy[band_idx];
            let trace = c_xx + c_yy;
            let det = c_xx * c_yy - c_xy.norm_sqr();
            let disc = ((trace / 2.0).powi(2) - det).max(0.0).sqrt();
            let lambda1 = trace / 2.0 + disc;
            let lambda2 = trace / 2.0 - disc;

            let mut coherence = if trace > 1e-9 {
                (lambda1 - lambda2) / (lambda1 + lambda2)
            } else {
                0.0
            };
            coherence = coherence.clamp(0.0, 1.0);
            self.coherence_instant[band_idx] = coherence;

            if band_idx < self.coherence_history.len() {
                let idx = self.coherence_history_idx % 5;
                self.coherence_history[band_idx][idx] = coherence;
                let median = median5(self.coherence_history[band_idx]);
                let prev = self.smoothed_coherence[band_idx];
                self.smoothed_coherence[band_idx] =
                    prev + coherence_smoothing_alpha * (median - prev);
                coherence = self.smoothed_coherence[band_idx];
            }

            // --- Intensity-vector DOA and diffuseness (Phase 1 & 2) ---
            let diffuseness_analysis = compute_diffuseness_and_doa(
                &self.freq_domain_left,
                &self.freq_domain_right,
                start_bin,
                end_bin,
            );
            let raw_diffuseness = diffuseness_analysis.diffuseness;
            let raw_doa = diffuseness_analysis.doa;
            let diffuseness = if band_idx < self.smoothed_diffuseness.len()
                && band_idx < self.diffuseness_initialized.len()
            {
                update_diffuseness_state(
                    &mut self.smoothed_diffuseness[band_idx],
                    &mut self.diffuseness_initialized[band_idx],
                    diffuseness_analysis,
                    analysis_smoothing_scale,
                )
            } else {
                raw_diffuseness
            };

            // Smooth DOA angle with one-pole filter (angle wrapping handled)
            if diffuseness_analysis.reliable && band_idx < self.doa_angle.len() {
                let prev_doa = self.doa_angle[band_idx];
                let mut delta = raw_doa - prev_doa;
                if delta > std::f32::consts::PI {
                    delta -= 2.0 * std::f32::consts::PI;
                } else if delta < -std::f32::consts::PI {
                    delta += 2.0 * std::f32::consts::PI;
                }
                self.doa_angle[band_idx] = prev_doa + doa_smoothing_alpha * delta;
            }

            // LFE Band
            let lfe_end = (lfe_cutoff_bin + 1).min(end_bin);
            if start_bin < lfe_end {
                for i in start_bin..lfe_end {
                    let left = self.freq_domain_left[i];
                    let right = self.freq_domain_right[i];
                    let bin = i.min(self.lfe_low_gains.len() - 1);
                    self.lfe[i] = (left + right) * self.lfe_low_gains[bin] * 0.5;
                    let hp = self.mains_high_gains[bin];
                    self.direct_left[i] = left * hp;
                    self.direct_right[i] = right * hp;
                    self.direct[i] = Complex::new(0.0, 0.0);
                    self.ambient_left[i] = Complex::new(0.0, 0.0);
                    self.ambient_right[i] = Complex::new(0.0, 0.0);
                }
            }

            // Pass-through Band (with cross-fade transition near bandpass boundary)
            let transition_half = 4usize;
            let transition_start = bandpass_bin.saturating_sub(transition_half);
            let transition_end = bandpass_bin + transition_half;
            let transition_width = (transition_end - transition_start) as f32;

            let pass_start = (lfe_cutoff_bin + 1).max(start_bin);
            let pass_end = transition_start.min(end_bin);
            if pass_start < pass_end {
                for i in pass_start..pass_end {
                    self.direct_left[i] = self.freq_domain_left[i];
                    self.direct_right[i] = self.freq_domain_right[i];
                    self.direct[i] = Complex::new(0.0, 0.0);
                    self.lfe[i] = Complex::new(0.0, 0.0);
                    self.ambient_left[i] = Complex::new(0.0, 0.0);
                    self.ambient_right[i] = Complex::new(0.0, 0.0);
                }
            }

            // Transition zone + Upmixing Band
            // Uses intensity-vector DOA for direction and diffuseness for decomposition
            let needs_upmix = transition_start.max(start_bin) < end_bin;
            if needs_upmix {
                // Diffuseness-based ambient gain. The base sqrt(psi) split is energy preserving;
                // ambient_boost and dialogue control are user/scene controls applied on top.
                let ambient_gain = ambient_gain_with_controls(
                    diffuseness,
                    self.ambient_boost.current(),
                    dialogue_control,
                );

                // Effective coherence for center extraction: blend coherence with dialogue
                let eff_coh = coherence + (1.0 - coherence) * dialogue_control;

                // Use PCA eigenvector for projection (still needed for directional decomposition).
                // c_xy is complex; retaining it in ev_l preserves inter-channel phase.
                let (ev_l, ev_r) = principal_eigenvector(c_xx, c_yy, c_xy, lambda1);

                // --- 2nd eigenvector (multi-source extraction) ---
                // lambda2 = trace - lambda1 (already have trace and lambda1 from coherence computation)
                // eigvec2 is perpendicular to eigvec1: rotate 90° → (-ev_r, ev_l) in the unitary sense.
                // We gate on lambda2/lambda1 > threshold to avoid extracting noise as a 2nd source.
                let extract_second = self.multi_source_extraction
                    && lambda1 > 1e-9
                    && (lambda2 / lambda1) > self.multi_source_threshold;

                // eigvec2 components: perpendicular rotation of eigvec1.
                // eigvec1 = (ev_l, ev_r) where both may have imaginary parts from c_xy being complex.
                // For the perpendicular in the 2-channel (L,R) space: ev2_l = -conj(ev_r), ev2_r = conj(ev_l).
                // This preserves the unitary property: <ev1, ev2> = 0.
                let ev2_l = -ev_r.conj();
                let ev2_r = ev_l.conj();

                // DOA estimate for the secondary source: derive from the eigvec2 L/R imbalance.
                // |ev2_l| = |ev_r| and |ev2_r| = |ev_l|.
                // When dominant source is left-biased (|ev_l| large), secondary is right-biased.
                // DOA angle: positive = right of center, negative = left of center.
                // Use atan2 of (|ev2_l| - |ev2_r|, 1) to get a directional bias in [-π/2, π/2].
                let ev2_l_mag = ev2_l.norm();
                let ev2_r_mag = ev2_r.norm();
                let doa2_band = fast_atan2(ev2_l_mag - ev2_r_mag, 1.0);

                let stereo_w = self.stereo_width.current();
                let upmix_start = transition_end.max(start_bin);

                // Transition zone: cross-fade between pass-through and decomposed
                let xfade_start = transition_start.max(start_bin).max(lfe_cutoff_bin + 1);
                let xfade_end = transition_end.min(end_bin);
                for i in xfade_start..xfade_end {
                    let l = self.freq_domain_left[i];
                    let r = self.freq_domain_right[i];

                    // Smooth raised-cosine blend: 0.0 = pass-through, 1.0 = upmix.
                    let t = transition_crossfade_weight(i, transition_start, transition_width);

                    // Project onto principal component for directional extraction
                    let proj = l * ev_l.conj() + r * ev_r.conj();
                    let direct_l = proj * ev_l;
                    let direct_r = proj * ev_r;
                    let phase_product = direct_r * direct_l.conj();
                    let phase_correction = if phase_product.norm_sqr() > 1e-18 {
                        phase_product / phase_product.norm()
                    } else {
                        Complex::new(1.0, 0.0)
                    };
                    let aligned_r = direct_r * phase_correction.conj();
                    let decomp_center = (direct_l + aligned_r) * (eff_coh * 0.5);
                    let decomp_amb_l = (l - direct_l) * ambient_gain;
                    let decomp_amb_r = (r - direct_r) * ambient_gain;
                    let decomp_dl = l - decomp_center * stereo_w;
                    let decomp_dr = r - decomp_center * phase_correction * stereo_w;

                    // Blend: pass-through has center=0, ambient=0, direct=original
                    self.direct[i] = decomp_center * t;
                    self.direct_left[i] = l * (1.0 - t) + decomp_dl * t;
                    self.direct_right[i] = r * (1.0 - t) + decomp_dr * t;
                    self.ambient_left[i] = decomp_amb_l * t;
                    self.ambient_right[i] = decomp_amb_r * t;
                    self.lfe[i] = Complex::new(0.0, 0.0);

                    // 2nd eigenvector projection for secondary source (multi-source extraction).
                    // proj2 captures the component perpendicular to the dominant source direction.
                    // Blended with t so it fades in across the transition zone.
                    if extract_second {
                        let proj2 = l * ev2_l.conj() + r * ev2_r.conj();
                        // Scalar projection amplitude; route as mono secondary source
                        self.direct2[i] = proj2 * t;
                        self.direct2_doa_per_bin[i] = bin_intensity_doa(l, r).unwrap_or(doa2_band);
                    } else {
                        self.direct2[i] = Complex::new(0.0, 0.0);
                        self.direct2_doa_per_bin[i] = 0.0;
                    }
                }

                // Full upmix band (after transition zone)
                for i in upmix_start..end_bin {
                    let l = self.freq_domain_left[i];
                    let r = self.freq_domain_right[i];
                    let proj = l * ev_l.conj() + r * ev_r.conj();
                    let direct_l = proj * ev_l;
                    let direct_r = proj * ev_r;
                    let phase_product = direct_r * direct_l.conj();
                    let phase_correction = if phase_product.norm_sqr() > 1e-18 {
                        phase_product / phase_product.norm()
                    } else {
                        Complex::new(1.0, 0.0)
                    };
                    let aligned_r = direct_r * phase_correction.conj();
                    self.direct[i] = (direct_l + aligned_r) * (eff_coh * 0.5);
                    self.ambient_left[i] = (l - direct_l) * ambient_gain;
                    self.ambient_right[i] = (r - direct_r) * ambient_gain;
                    self.direct_left[i] = l - self.direct[i] * stereo_w;
                    self.direct_right[i] = r - self.direct[i] * phase_correction * stereo_w;
                    self.lfe[i] = Complex::new(0.0, 0.0);

                    // 2nd eigenvector projection for secondary source (full upmix zone).
                    if extract_second {
                        let proj2 = l * ev2_l.conj() + r * ev2_r.conj();
                        self.direct2[i] = proj2;
                        self.direct2_doa_per_bin[i] = bin_intensity_doa(l, r).unwrap_or(doa2_band);
                    } else {
                        self.direct2[i] = Complex::new(0.0, 0.0);
                        self.direct2_doa_per_bin[i] = 0.0;
                    }
                }

                let tr_red = 1.0
                    - (self.height_transient_env_slow * self.height_transient_reduction.current())
                        .min(self.height_transient_reduction.current());
                let corr_start = xfade_start.min(upmix_start);
                for i in corr_start..end_bin {
                    // Height suitability: blend frequency weight with diffuseness
                    // Diffuse content is better suited for height channels than coherent content
                    let h_suit = (self.height_freq_weights[i] * 0.5 + diffuseness * 0.5).min(1.0);
                    self.height_band_gains[i] = (h_suit * tr_red).clamp(HEIGHT_MASK_FLOOR, 1.0);
                }
            }
        }
        let strength = (1.0 - self.dialogue_spatial_control * 0.7).clamp(0.05, 1.0);
        self.decorrelation_strength = strength;

        let spec_size = self.fft_size / 2 + 1;
        let num_ch = self.num_output_channels;

        // Only reblend when strength changed significantly, or in LFO mode
        // (LFO mode updates the underlying decorrelation filters every block)
        let needs_reblend = self.decorrelation_mode == 1
            || (strength - self.prev_decorrelation_strength).abs() > 0.02;

        if needs_reblend {
            self.prev_decorrelation_strength = strength;
            let id_w = 1.0 - strength;
            for ch in 0..num_ch {
                let s = &self.speaker_config.speakers[ch];
                if s.is_lfe || (s.azimuth.abs() < 80.0 && s.elevation.abs() < 10.0) {
                    self.blended_decorrelation_filters[ch].fill(Complex::new(1.0, 0.0));
                    continue;
                }
                let decor = if ch < self.decorrelation_filters.len() {
                    &self.decorrelation_filters[ch]
                } else if s.azimuth > 0.0 {
                    &self.decorrelation_filter_left
                } else {
                    &self.decorrelation_filter_right
                };
                if strength >= 0.99 {
                    self.blended_decorrelation_filters[ch].copy_from_slice(decor);
                } else {
                    for (i, d) in decor.iter().enumerate().take(spec_size) {
                        let blended = Complex::new(strength * d.re + id_w, strength * d.im);
                        self.blended_decorrelation_filters[ch][i] =
                            normalize_decorrelation_blend(blended);
                    }
                }
            }
        }

        // Cross-fade decorrelation filters during mode/bypass transitions.
        // 25 blocks at 48kHz/2048/50%-overlap ≈ 535ms, which is long enough to
        // avoid audible swish/phase artifacts from the abrupt all-pass phase change.
        // A cosine crossfade shape (equal-power) avoids the click at t=0 that a
        // linear ramp produces when the two phase responses differ widely.
        if self.decorrelation_crossfade_remaining > 0 {
            let total = 25.0_f32;
            let t_linear = 1.0 - (self.decorrelation_crossfade_remaining as f32 / total);
            // Equal-power crossfade shape
            let t = 0.5 - 0.5 * (std::f32::consts::PI * t_linear).cos();
            for ch in 0..num_ch {
                if ch < self.prev_blended_filters_for_crossfade.len() {
                    let prev = &self.prev_blended_filters_for_crossfade[ch];
                    let cur = &mut self.blended_decorrelation_filters[ch];
                    for i in 0..spec_size {
                        // Phase-angle interpolation preserves the all-pass property
                        // during mode transitions (linear complex interp does not).
                        // Use fast math approximations for real-time efficiency.
                        let prev_phase = fast_atan2(prev[i].im, prev[i].re);
                        let cur_phase = fast_atan2(cur[i].im, cur[i].re);
                        let mut delta = cur_phase - prev_phase;
                        if delta > std::f32::consts::PI {
                            delta -= 2.0 * std::f32::consts::PI;
                        } else if delta < -std::f32::consts::PI {
                            delta += 2.0 * std::f32::consts::PI;
                        }
                        let blended_phase = prev_phase + t * delta;
                        cur[i] = Complex::new(fast_cos(blended_phase), fast_sin(blended_phase));
                    }
                }
            }
            self.decorrelation_crossfade_remaining -= 1;
        }

        self.coherence_history_idx = self.coherence_history_idx.wrapping_add(1);
        // Note: FTZ/DAZ CPU flags handle denormal flushing automatically

        let height_floor_end = self.cached_bandpass_bin.min(self.height_band_gains.len());
        self.height_band_gains[..height_floor_end].fill(HEIGHT_MASK_FLOOR);

        // Compute height spectral flux gate before smoothing height gains
        self.compute_height_flux_gate();

        self.smooth_height_gains();
        self.height_band_gains[..height_floor_end].fill(HEIGHT_MASK_FLOOR);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quadrature_intensity_counts_as_directional_energy() {
        let mut left = vec![Complex::new(0.0, 0.0); 4];
        let mut right = vec![Complex::new(0.0, 0.0); 4];

        left[1] = Complex::new(1.0, 0.0);
        right[1] = Complex::new(0.0, 1.0);

        let analysis = compute_diffuseness_and_doa(&left, &right, 1, 2);

        assert!(
            analysis.diffuseness < 0.01,
            "quadrature intensity should remain directional, got diffuseness {}",
            analysis.diffuseness
        );
        assert!(
            analysis.doa.abs() > 1.0,
            "DOA should preserve the imaginary intensity axis, got {}",
            analysis.doa
        );
        assert!(analysis.reliable);
        assert!(analysis.direct_gain() > 0.99);
        assert!(analysis.ambient_gain() < 0.1);
    }

    #[test]
    fn near_silence_diffuseness_is_unreliable_not_ambient() {
        let left = vec![Complex::new(1e-10, 0.0); 4];
        let right = vec![Complex::new(-1e-10, 0.0); 4];

        let analysis = compute_diffuseness_and_doa(&left, &right, 1, 3);

        assert!(!analysis.reliable);
        assert_eq!(analysis.diffuseness, 0.0);
        assert_eq!(analysis.ambient_gain(), 0.0);
        assert_eq!(analysis.direct_gain(), 1.0);
    }

    #[test]
    fn silence_carries_previous_diffuseness_in_processing_state() {
        let mut previous = 0.35;
        let mut initialized = true;
        let unreliable = DiffusenessAndDoa {
            diffuseness: 0.0,
            doa: 0.0,
            reliable: false,
        };

        let selected = update_diffuseness_state(&mut previous, &mut initialized, unreliable, 1.0);

        assert_eq!(selected, 0.35);
        assert_eq!(previous, 0.35);
        assert!(initialized);
    }

    #[test]
    fn complex_principal_eigenvector_preserves_covariance_phase() {
        let c_xx = 1.0;
        let c_yy = 1.0;
        let c_xy = Complex::new(0.0, 0.8);
        let lambda1 = 1.8;

        let (ev_l, ev_r) = principal_eigenvector(c_xx, c_yy, c_xy, lambda1);

        assert!(
            ev_l.im.abs() > 0.7,
            "principal eigenvector should retain complex covariance phase: {ev_l:?}"
        );

        let lhs_l = ev_l * c_xx + ev_r * c_xy;
        let lhs_r = ev_l * c_xy.conj() + ev_r * c_yy;
        let rhs_l = ev_l * lambda1;
        let rhs_r = ev_r * lambda1;

        assert!((lhs_l - rhs_l).norm() < 1e-5, "left residual too large");
        assert!((lhs_r - rhs_r).norm() < 1e-5, "right residual too large");
    }

    #[test]
    fn ambient_gain_is_energy_preserving_only_before_user_boosts() {
        let diffuseness: f32 = 0.64;
        let base_direct = (1.0 - diffuseness).sqrt();
        let base_ambient = ambient_gain_with_controls(diffuseness, 1.0, 0.0);

        assert!((base_direct * base_direct + base_ambient * base_ambient - 1.0).abs() < 1e-6);

        let boosted_ambient = ambient_gain_with_controls(diffuseness, 1.5, 0.0);
        assert!(boosted_ambient > base_ambient);
    }

    #[test]
    fn transition_crossfade_uses_raised_cosine_shape() {
        let start = 10;
        let width = 8.0;

        let first = transition_crossfade_weight(start, start, width);
        let quarter = transition_crossfade_weight(start + 2, start, width);
        let middle = transition_crossfade_weight(start + 4, start, width);
        let three_quarter = transition_crossfade_weight(start + 6, start, width);
        let last = transition_crossfade_weight(start + 8, start, width);

        assert!(first.abs() < 1e-6);
        assert!((middle - 0.5).abs() < 0.002);
        assert!((last - 1.0).abs() < 0.002);
        assert!(quarter < 0.25, "raised cosine should ease in below linear");
        assert!(
            three_quarter > 0.75,
            "raised cosine should ease out above linear"
        );
    }

    #[test]
    fn decorrelation_blend_normalization_is_unit_magnitude() {
        let blended = Complex::new(0.37, -0.61);
        let normalized = normalize_decorrelation_blend(blended);

        assert!((normalized.norm() - 1.0).abs() < 1e-6);
        assert_eq!(
            normalize_decorrelation_blend(Complex::new(0.0, 0.0)),
            Complex::new(1.0, 0.0)
        );
    }

    #[test]
    fn bin_intensity_doa_varies_per_frequency_bin() {
        let left_a = Complex::new(1.0, 0.0);
        let right_a = Complex::new(0.2, 0.0);
        let left_b = Complex::new(0.2, 0.0);
        let right_b = Complex::new(1.0, 0.0);

        let doa_a = bin_intensity_doa(left_a, right_a).unwrap();
        let doa_b = bin_intensity_doa(left_b, right_b).unwrap();

        assert!(
            (doa_a - doa_b).abs() > 1.0,
            "per-bin secondary DOA should react to per-bin L/R direction"
        );
    }

    #[test]
    fn bin_intensity_doa_ignores_silent_bins() {
        assert!(bin_intensity_doa(Complex::new(0.0, 0.0), Complex::new(0.0, 0.0)).is_none());
    }

    #[test]
    fn dialogue_spatial_control_slew_limits_jitter() {
        let up = smooth_dialogue_spatial_control(0.0, 1.0);
        assert!(
            up <= DIALOGUE_SPATIAL_MAX_RISE + 1e-6,
            "dialogue control rose too quickly: {up}"
        );

        let down = smooth_dialogue_spatial_control(1.0, 0.0);
        assert!(
            1.0 - down <= DIALOGUE_SPATIAL_MAX_FALL + 1e-6,
            "dialogue control fell too quickly: {}",
            1.0 - down
        );

        let stable = smooth_dialogue_spatial_control(0.5, 0.5 + DIALOGUE_SPATIAL_DEADBAND * 0.5);
        assert_eq!(stable, 0.5);
    }

    #[test]
    fn diffuseness_smoothing_limits_block_steps() {
        let up = smooth_diffuseness(0.0, 1.0, 1.0);
        assert!(
            up <= DIFFUSENESS_MAX_STEP + 1e-6,
            "diffuseness rose too quickly: {up}"
        );

        let down = smooth_diffuseness(1.0, 0.0, 1.0);
        assert!(
            1.0 - down <= DIFFUSENESS_MAX_STEP + 1e-6,
            "diffuseness fell too quickly: {}",
            1.0 - down
        );

        let narrow_band = smooth_diffuseness(0.0, 1.0, 0.35);
        assert!(
            narrow_band <= up,
            "narrow-band analysis should not smooth faster than ERB mode"
        );
    }
}
