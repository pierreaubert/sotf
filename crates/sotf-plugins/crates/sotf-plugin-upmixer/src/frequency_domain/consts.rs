use super::super::UpmixerPlugin;
use super::diffuseness_and_doa::compute_diffuseness_and_doa;
use super::diffuseness_and_doa::update_diffuseness_state;
use super::misc::ambient_gain_with_controls;
use super::misc::median5;
use super::misc::normalize_decorrelation_blend;
use super::misc::principal_eigenvector;
use super::misc::transition_crossfade_weight;
use super::smooth::smooth_dialogue_spatial_control;
use math_audio_dsp::fast_math::{fast_atan2, fast_cos, fast_sin};
use rustfft::num_complex::Complex;
use sotf_host::simd::compute_covariance_simd;

/// Minimum height mask value -- prevents deep spectral notches that cause time-domain ringing
pub(in super::super) const HEIGHT_MASK_FLOOR: f32 = 0.10;

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
pub(super) const DIFFUSENESS_SMOOTHING_ALPHA: f32 = 0.18;

pub(super) const DIFFUSENESS_MAX_STEP: f32 = 0.08;

pub(super) const DIFFUSENESS_ENERGY_FLOOR: f32 = 1e-12;

/// Dialogue control smoothing for spatial decomposition. This is intentionally
/// slower than detector smoothing because it modulates center, ambience, and
/// decorrelation over the whole sound field.
pub(super) const DIALOGUE_SPATIAL_ATTACK_ALPHA: f32 = 0.06;

pub(super) const DIALOGUE_SPATIAL_RELEASE_ALPHA: f32 = 0.025;

pub(super) const DIALOGUE_SPATIAL_MAX_RISE: f32 = 0.035;

pub(super) const DIALOGUE_SPATIAL_MAX_FALL: f32 = 0.018;

pub(super) const DIALOGUE_SPATIAL_DEADBAND: f32 = 0.004;

#[inline(always)]
pub(super) fn bin_intensity_doa(left: Complex<f32>, right: Complex<f32>) -> Option<f32> {
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
    pub(in super::super) fn process_frequency_domain_erb_bands(&mut self) {
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
