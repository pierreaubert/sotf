use super::super::UpmixerPlugin;
use super::diffuseness_and_doa::compute_diffuseness_and_doa;
use super::diffuseness_and_doa::{stereo_spatial_cue, update_diffuseness_state};
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

const REFERENCE_HOP_SECONDS: f32 = 1024.0 / 48_000.0;

#[inline(always)]
pub(super) fn time_scaled_alpha(reference_alpha: f32, hop_samples: usize, sample_rate: u32) -> f32 {
    if sample_rate == 0 {
        return reference_alpha;
    }
    let intervals = hop_samples as f32 / sample_rate as f32 / REFERENCE_HOP_SECONDS;
    1.0 - (1.0 - reference_alpha).powf(intervals)
}

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
    stereo_spatial_cue(left.norm_sqr(), right.norm_sqr(), (left * right.conj()).re)
        .map(|(_, lateral_balance)| lateral_balance * std::f32::consts::FRAC_PI_2)
}

impl UpmixerPlugin {
    pub(in super::super) fn process_frequency_domain_erb_bands(&mut self) {
        if self.core.sample_rate == 0 || self.core.fft_size == 0 {
            return;
        }

        if self.decorrelation.decorrelation_mode == 1 {
            self.update_lfo_decorrelation();
        }

        // Zero direct2/direct2_doa_per_bin at frame start: ensures LFE/pass-through bins remain
        // silent, and handles the case when multi_source_extraction is toggled off mid-stream.
        let spec_size_pre = self.core.fft_size / 2 + 1;
        self.spectral.direct2[..spec_size_pre].fill(Complex::new(0.0, 0.0));
        self.spectral.direct2_doa_per_bin[..spec_size_pre].fill(0.0);

        let lfe_cutoff_bin = self.cache.cached_lfe_cutoff_bin;
        let bandpass_bin = self.cache.cached_bandpass_bin;
        let freq_per_bin = self.cache.cached_freq_per_bin;
        let analysis_smoothing_scale =
            self.core.fft_size as f32 * 0.5 / self.core.sample_rate as f32 / REFERENCE_HOP_SECONDS;
        let coherence_smoothing_alpha = time_scaled_alpha(
            COHERENCE_SMOOTHING_ALPHA,
            self.core.fft_size / 2,
            self.core.sample_rate,
        );
        let doa_smoothing_alpha = time_scaled_alpha(
            DOA_SMOOTHING_ALPHA,
            self.core.fft_size / 2,
            self.core.sample_rate,
        );
        self.dialogue.dialogue_spatial_control = smooth_dialogue_spatial_control(
            self.dialogue.dialogue_spatial_control,
            self.dialogue.dialogue_probability,
        );
        let dialogue_control =
            self.dialogue.dialogue_spatial_control * self.dialogue.dialogue_weight.current();

        for band_idx in 0..self.steering.erb_bands.len() {
            let start_bin = self.steering.erb_bands[band_idx];
            let end_bin = if band_idx + 1 < self.steering.erb_bands.len() {
                self.steering.erb_bands[band_idx + 1]
            } else {
                self.core.fft_size / 2 + 1
            };
            if start_bin >= end_bin || start_bin > self.core.fft_size / 2 {
                continue;
            }

            // Covariance for steering smoothing (attack/release detection)
            let (cov_xx, cov_yy, cov_xy) = compute_covariance_simd(
                &self.main_buffers.freq_domain_left,
                &self.main_buffers.freq_domain_right,
                start_bin,
                end_bin,
            );

            let inst_energy = cov_xx + cov_yy;
            let smooth_energy =
                self.spectral.pca_cov_xx[band_idx] + self.spectral.pca_cov_yy[band_idx];
            let center_bin = (start_bin + end_bin) / 2;
            let center_freq = center_bin as f32 * freq_per_bin;
            let norm = ((center_freq - 100.0) / (8000.0 - 100.0)).clamp(0.0, 1.0);
            let attack_alpha = time_scaled_alpha(
                STEERING_ATTACK_BASE + STEERING_ATTACK_RANGE * norm,
                self.core.fft_size / 2,
                self.core.sample_rate,
            );
            let release_alpha = time_scaled_alpha(
                STEERING_RELEASE_BASE + STEERING_RELEASE_RANGE * norm,
                self.core.fft_size / 2,
                self.core.sample_rate,
            );
            let alpha = if inst_energy > smooth_energy * 1.5 {
                attack_alpha
            } else {
                release_alpha
            };
            // Only written for test inspection; not read in the processing path.
            #[cfg(test)]
            {
                self.steering.steering_alphas[band_idx] = alpha;
            }

            self.spectral.pca_cov_xx[band_idx] =
                (1.0 - alpha) * self.spectral.pca_cov_xx[band_idx] + alpha * cov_xx;
            self.spectral.pca_cov_yy[band_idx] =
                (1.0 - alpha) * self.spectral.pca_cov_yy[band_idx] + alpha * cov_yy;
            self.spectral.pca_cov_xy[band_idx] =
                (1.0 - alpha) * self.spectral.pca_cov_xy[band_idx] + alpha * cov_xy;

            // Compute coherence from smoothed covariance (used for median filtering)
            let c_xx = self.spectral.pca_cov_xx[band_idx];
            let c_yy = self.spectral.pca_cov_yy[band_idx];
            let c_xy = self.spectral.pca_cov_xy[band_idx];
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
            self.steering.coherence_instant[band_idx] = coherence;

            if band_idx < self.steering.coherence_history.len() {
                let idx = self.steering.coherence_history_idx % 5;
                self.steering.coherence_history[band_idx][idx] = coherence;
                let median = median5(self.steering.coherence_history[band_idx]);
                let prev = self.steering.smoothed_coherence[band_idx];
                self.steering.smoothed_coherence[band_idx] =
                    prev + coherence_smoothing_alpha * (median - prev);
                coherence = self.steering.smoothed_coherence[band_idx];
            }

            // --- Intensity-vector DOA and diffuseness (Phase 1 & 2) ---
            let diffuseness_analysis = compute_diffuseness_and_doa(
                &self.main_buffers.freq_domain_left,
                &self.main_buffers.freq_domain_right,
                start_bin,
                end_bin,
            );
            let raw_diffuseness = diffuseness_analysis.diffuseness;
            let raw_doa = diffuseness_analysis.doa;
            let diffuseness = if band_idx < self.steering.smoothed_diffuseness.len()
                && band_idx < self.steering.diffuseness_initialized.len()
            {
                update_diffuseness_state(
                    &mut self.steering.smoothed_diffuseness[band_idx],
                    &mut self.steering.diffuseness_initialized[band_idx],
                    diffuseness_analysis,
                    analysis_smoothing_scale,
                )
            } else {
                raw_diffuseness
            };

            // Smooth DOA angle with one-pole filter (angle wrapping handled)
            if diffuseness_analysis.reliable && band_idx < self.steering.doa_angle.len() {
                let prev_doa = self.steering.doa_angle[band_idx];
                let mut delta = raw_doa - prev_doa;
                if delta > std::f32::consts::PI {
                    delta -= 2.0 * std::f32::consts::PI;
                } else if delta < -std::f32::consts::PI {
                    delta += 2.0 * std::f32::consts::PI;
                }
                self.steering.doa_angle[band_idx] = prev_doa + doa_smoothing_alpha * delta;
            }

            // LFE Band
            let lfe_end = (lfe_cutoff_bin + 1).min(end_bin);
            if start_bin < lfe_end {
                for i in start_bin..lfe_end {
                    let left = self.main_buffers.freq_domain_left[i];
                    let right = self.main_buffers.freq_domain_right[i];
                    let bin = i.min(self.spectral.lfe_low_gains.len() - 1);
                    self.main_buffers.lfe[i] =
                        (left + right) * self.spectral.lfe_low_gains[bin] * 0.5;
                    let hp = self.spectral.mains_high_gains[bin];
                    self.main_buffers.direct_left[i] = left * hp;
                    self.main_buffers.direct_right[i] = right * hp;
                    self.main_buffers.direct[i] = Complex::new(0.0, 0.0);
                    self.main_buffers.ambient_left[i] = Complex::new(0.0, 0.0);
                    self.main_buffers.ambient_right[i] = Complex::new(0.0, 0.0);
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
                    self.main_buffers.direct_left[i] = self.main_buffers.freq_domain_left[i];
                    self.main_buffers.direct_right[i] = self.main_buffers.freq_domain_right[i];
                    self.main_buffers.direct[i] = Complex::new(0.0, 0.0);
                    self.main_buffers.lfe[i] = Complex::new(0.0, 0.0);
                    self.main_buffers.ambient_left[i] = Complex::new(0.0, 0.0);
                    self.main_buffers.ambient_right[i] = Complex::new(0.0, 0.0);
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
                    self.surround.ambient_boost.current(),
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
                let extract_second = self.spectral.multi_source_extraction
                    && lambda1 > 1e-9
                    && (lambda2 / lambda1) > self.spectral.multi_source_threshold;

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

                let stereo_w = self.gains.stereo_width.current();
                let upmix_start = transition_end.max(start_bin);

                // Transition zone: cross-fade between pass-through and decomposed
                let xfade_start = transition_start.max(start_bin).max(lfe_cutoff_bin + 1);
                let xfade_end = transition_end.min(end_bin);
                for i in xfade_start..xfade_end {
                    let l = self.main_buffers.freq_domain_left[i];
                    let r = self.main_buffers.freq_domain_right[i];

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
                    self.main_buffers.direct[i] = decomp_center * t;
                    self.main_buffers.direct_left[i] = l * (1.0 - t) + decomp_dl * t;
                    self.main_buffers.direct_right[i] = r * (1.0 - t) + decomp_dr * t;
                    self.main_buffers.ambient_left[i] = decomp_amb_l * t;
                    self.main_buffers.ambient_right[i] = decomp_amb_r * t;
                    self.main_buffers.lfe[i] = Complex::new(0.0, 0.0);

                    // 2nd eigenvector projection for secondary source (multi-source extraction).
                    // proj2 captures the component perpendicular to the dominant source direction.
                    // Blended with t so it fades in across the transition zone.
                    if extract_second {
                        let proj2 = l * ev2_l.conj() + r * ev2_r.conj();
                        // Scalar projection amplitude; route as mono secondary source
                        self.spectral.direct2[i] = proj2 * t;
                        self.spectral.direct2_doa_per_bin[i] =
                            bin_intensity_doa(l, r).unwrap_or(doa2_band);
                    } else {
                        self.spectral.direct2[i] = Complex::new(0.0, 0.0);
                        self.spectral.direct2_doa_per_bin[i] = 0.0;
                    }
                }

                // Full upmix band (after transition zone)
                for i in upmix_start..end_bin {
                    let l = self.main_buffers.freq_domain_left[i];
                    let r = self.main_buffers.freq_domain_right[i];
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
                    self.main_buffers.direct[i] = (direct_l + aligned_r) * (eff_coh * 0.5);
                    self.main_buffers.ambient_left[i] = (l - direct_l) * ambient_gain;
                    self.main_buffers.ambient_right[i] = (r - direct_r) * ambient_gain;
                    self.main_buffers.direct_left[i] = l - self.main_buffers.direct[i] * stereo_w;
                    self.main_buffers.direct_right[i] =
                        r - self.main_buffers.direct[i] * phase_correction * stereo_w;
                    self.main_buffers.lfe[i] = Complex::new(0.0, 0.0);

                    // 2nd eigenvector projection for secondary source (full upmix zone).
                    if extract_second {
                        let proj2 = l * ev2_l.conj() + r * ev2_r.conj();
                        self.spectral.direct2[i] = proj2;
                        self.spectral.direct2_doa_per_bin[i] =
                            bin_intensity_doa(l, r).unwrap_or(doa2_band);
                    } else {
                        self.spectral.direct2[i] = Complex::new(0.0, 0.0);
                        self.spectral.direct2_doa_per_bin[i] = 0.0;
                    }
                }

                let tr_red = 1.0
                    - (self.height.height_transient_env_slow
                        * self.height.height_transient_reduction.current())
                    .min(self.height.height_transient_reduction.current());
                let corr_start = xfade_start.min(upmix_start);
                for i in corr_start..end_bin {
                    // Height suitability: blend frequency weight with diffuseness
                    // Diffuse content is better suited for height channels than coherent content
                    let h_suit =
                        (self.height.height_freq_weights[i] * 0.5 + diffuseness * 0.5).min(1.0);
                    self.height.height_band_gains[i] =
                        (h_suit * tr_red).clamp(HEIGHT_MASK_FLOOR, 1.0);
                }
            }
        }

        let strength = (1.0 - self.dialogue.dialogue_spatial_control * 0.7).clamp(0.05, 1.0);
        self.decorrelation.decorrelation_strength = strength;

        let spec_size = self.core.fft_size / 2 + 1;
        let num_ch = self.core.num_output_channels;

        // Only reblend when strength changed significantly, or in LFO mode
        // (LFO mode updates the underlying decorrelation filters every block)
        let needs_reblend = self.decorrelation.decorrelation_mode == 1
            || (strength - self.decorrelation.prev_decorrelation_strength).abs() > 0.02;

        if needs_reblend {
            self.decorrelation.prev_decorrelation_strength = strength;
            let id_w = 1.0 - strength;
            for ch in 0..num_ch {
                let s = &self.core.speaker_config.speakers[ch];
                if s.is_lfe || (s.azimuth.abs() < 80.0 && s.elevation.abs() < 10.0) {
                    self.decorrelation.blended_decorrelation_filters[ch]
                        .fill(Complex::new(1.0, 0.0));
                    continue;
                }
                let decor = if ch < self.decorrelation.decorrelation_filters.len() {
                    &self.decorrelation.decorrelation_filters[ch]
                } else if s.azimuth > 0.0 {
                    &self.decorrelation.decorrelation_filter_left
                } else {
                    &self.decorrelation.decorrelation_filter_right
                };
                if strength >= 0.99 {
                    self.decorrelation.blended_decorrelation_filters[ch].copy_from_slice(decor);
                } else {
                    for (i, d) in decor.iter().enumerate().take(spec_size) {
                        let blended = Complex::new(strength * d.re + id_w, strength * d.im);
                        self.decorrelation.blended_decorrelation_filters[ch][i] =
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
        if self.decorrelation.decorrelation_crossfade_remaining > 0 {
            let total = 25.0_f32;
            let t_linear =
                1.0 - (self.decorrelation.decorrelation_crossfade_remaining as f32 / total);
            // Equal-power crossfade shape
            let t = 0.5 - 0.5 * (std::f32::consts::PI * t_linear).cos();
            for ch in 0..num_ch {
                if ch < self.decorrelation.prev_blended_filters_for_crossfade.len() {
                    let prev = &self.decorrelation.prev_blended_filters_for_crossfade[ch];
                    let cur = &mut self.decorrelation.blended_decorrelation_filters[ch];
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
            self.decorrelation.decorrelation_crossfade_remaining -= 1;
        }

        self.steering.coherence_history_idx = self.steering.coherence_history_idx.wrapping_add(1);
        // Note: FTZ/DAZ CPU flags handle denormal flushing automatically

        let height_floor_end = self
            .cache
            .cached_bandpass_bin
            .min(self.height.height_band_gains.len());
        self.height.height_band_gains[..height_floor_end].fill(HEIGHT_MASK_FLOOR);

        // Compute height spectral flux gate before smoothing height gains
        self.compute_height_flux_gate();

        self.smooth_height_gains();
        self.height.height_band_gains[..height_floor_end].fill(HEIGHT_MASK_FLOOR);
    }
}
