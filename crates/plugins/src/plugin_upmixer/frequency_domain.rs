// ============================================================================
// Frequency Domain Processing with ERB Bands
// ============================================================================

use super::UpmixerPlugin;
use crate::simd::{compute_covariance_simd, flush_denormals_complex_inplace};
use rustfft::num_complex::Complex;

fn base_ambient_gain_from_coherence(coherence: f32, ambient_boost: f32) -> f32 {
    let coherence_clamped = coherence.clamp(0.0, 1.0);
    let ambient_base = (1.0 - coherence_clamped).max(0.0);
    ambient_base.sqrt() * ambient_boost
}

impl UpmixerPlugin {
    /// Phase 2: Process frequency domain using ERB bands and Logic Steering
    ///
    /// This function performs:
    /// 1. LFO decorrelation update (if enabled)
    /// 2. ERB band-wise processing with:
    ///    - Covariance computation
    ///    - PCA decomposition
    ///    - Coherence calculation
    ///    - Logic steering
    /// 3. Three frequency ranges:
    ///    - LFE band (0 - lfe_cutoff_hz): Mono sum to LFE + highpass to mains
    ///    - Pass-through band (lfe_cutoff_hz - bandpass_hz): No upmixing
    ///    - Upmixing band (bandpass_hz - Nyquist): Direct/Ambient decomposition
    /// 4. Dialogue detection and adaptive decorrelation
    /// 5. Height channel masking
    pub(super) fn process_frequency_domain_erb_bands(&mut self) {
        // Guard against uninitialized state
        if self.sample_rate == 0 || self.fft_size == 0 {
            log::error!(
                "[UPMIXER] process_frequency_domain_erb_bands called with sample_rate={} fft_size={}",
                self.sample_rate,
                self.fft_size
            );
            return;
        }

        if self.decorrelation_mode == 1 {
            self.update_lfo_decorrelation();
        }

        let lfe_cutoff_bin =
            ((self.lfe_cutoff_hz * self.fft_size as f32) / self.sample_rate as f32) as usize;
        let bandpass_bin =
            ((self.bandpass_hz * self.fft_size as f32) / self.sample_rate as f32) as usize;
        let freq_per_bin = self.sample_rate as f32 / self.fft_size as f32;

        // Iterate over ERB bands
        for band_idx in 0..self.erb_bands.len() {
            let start_bin = self.erb_bands[band_idx];
            let end_bin = if band_idx + 1 < self.erb_bands.len() {
                self.erb_bands[band_idx + 1]
            } else {
                self.fft_size / 2 + 1
            };

            // Skip if band is empty or out of range
            if start_bin >= end_bin || start_bin > self.fft_size / 2 {
                continue;
            }

            // Calculate Band Statistics (Covariance) - SIMD accelerated
            let (cov_xx, cov_yy, cov_xy) = compute_covariance_simd(
                &self.freq_domain_left,
                &self.freq_domain_right,
                start_bin,
                end_bin,
            );

            // Logic Steering (Smoothing)
            let inst_energy = cov_xx + cov_yy;
            let smooth_energy = self.pca_cov_xx[band_idx] + self.pca_cov_yy[band_idx];

            // Variable Attack/Release
            // If energy rises (transient), attack fast. If falls, release slow.
            let center_bin = (start_bin + end_bin) / 2;
            let center_freq = center_bin as f32 * freq_per_bin;

            let norm = ((center_freq - 100.0) / (8000.0 - 100.0)).clamp(0.0, 1.0);
            let attack_alpha = 0.3 + 0.4 * norm;
            let release_alpha = 0.02 + 0.06 * norm;

            let alpha = if inst_energy > smooth_energy * 1.5 {
                attack_alpha
            } else {
                release_alpha
            };
            self.steering_alphas[band_idx] = alpha;

            // Update smoothed covariance
            self.pca_cov_xx[band_idx] = (1.0 - alpha) * self.pca_cov_xx[band_idx] + alpha * cov_xx;
            self.pca_cov_yy[band_idx] = (1.0 - alpha) * self.pca_cov_yy[band_idx] + alpha * cov_yy;
            self.pca_cov_xy[band_idx] = (1.0 - alpha) * self.pca_cov_xy[band_idx] + alpha * cov_xy;

            // PCA Decomposition
            let c_xx = self.pca_cov_xx[band_idx];
            let c_yy = self.pca_cov_yy[band_idx];
            let c_xy = self.pca_cov_xy[band_idx];

            // Eigenvalues of 2x2 Hermitian matrix
            let trace = c_xx + c_yy;
            let det = c_xx * c_yy - c_xy.norm_sqr();
            // Avoid sqrt of negative due to float errors
            let disc = ((trace / 2.0).powi(2) - det).max(0.0).sqrt();
            let lambda1 = trace / 2.0 + disc;
            let lambda2 = trace / 2.0 - disc;

            // Coherence (0 to 1)
            // High coherence = strong direct sound (lambda1 >> lambda2)
            // Low coherence = diffuse sound (lambda1 ~= lambda2)
            let mut coherence = if trace > 1e-9 {
                (lambda1 - lambda2) / (lambda1 + lambda2)
            } else {
                0.0
            };
            coherence = coherence.clamp(0.0, 1.0);

            self.coherence_instant[band_idx] = coherence;

            // 3A: Median-filtered coherence estimation
            // Write instant coherence to ring buffer, compute median of 5 values,
            // then apply gentle one-pole on median for robust outlier rejection.
            if band_idx < self.coherence_history.len() {
                let idx = self.coherence_history_idx % 5;
                self.coherence_history[band_idx][idx] = coherence;

                // Compute median of 5-element ring buffer via sorting a local copy
                let mut sorted = self.coherence_history[band_idx];
                sorted.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let median = sorted[2]; // Middle element of 5

                // Gentle one-pole on median (alpha=0.15 for smooth response)
                let prev = self.smoothed_coherence[band_idx];
                let smoothed = prev + 0.15 * (median - prev);
                self.smoothed_coherence[band_idx] = smoothed;
                coherence = smoothed;
            }

            // 1. LFE Band Logic
            // Determine intersection of current band [start_bin, end_bin) with LFE range [0, lfe_cutoff_bin]
            let lfe_end = (lfe_cutoff_bin + 1).min(end_bin);
            if start_bin < lfe_end {
                let loop_start = start_bin;
                let loop_end = lfe_end;

                for i in loop_start..loop_end {
                    let left = self.freq_domain_left[i];
                    let right = self.freq_domain_right[i];

                    // LFE band: use Linkwitz–Riley style crossover so that
                    // low frequencies are shared between LFE (low-pass
                    // mono sum) and mains (high-passed left/right).

                    let bin = i.min(self.lfe_low_gains.len() - 1);
                    let low_gain = self.lfe_low_gains[bin];
                    let high_gain = self.mains_high_gains[bin];

                    let mono = (left + right) * Complex::new(0.5 * low_gain, 0.0);
                    self.lfe[i] = mono;

                    let hp_scale = Complex::new(high_gain, 0.0);
                    self.direct_left[i] = left * hp_scale;
                    self.direct_right[i] = right * hp_scale;
                    self.ambient_left[i] = Complex::new(0.0, 0.0);
                    self.ambient_right[i] = Complex::new(0.0, 0.0);
                }
            }

            // 2. Pass-through Band Logic
            // Intersection of [start_bin, end_bin) with [lfe_cutoff_bin + 1, bandpass_bin)
            let pass_start = (lfe_cutoff_bin + 1).max(start_bin);
            let pass_end = bandpass_bin.min(end_bin);

            if pass_start < pass_end {
                for i in pass_start..pass_end {
                    let left = self.freq_domain_left[i];
                    let right = self.freq_domain_right[i];

                    self.direct_left[i] = left;
                    self.direct_right[i] = right;
                    self.lfe[i] = Complex::new(0.0, 0.0);
                    self.ambient_left[i] = Complex::new(0.0, 0.0);
                    self.ambient_right[i] = Complex::new(0.0, 0.0);
                }
            }

            // 3. Upmixing Band Logic
            // Intersection of [start_bin, end_bin) with [bandpass_bin, infinity)
            let upmix_start = bandpass_bin.max(start_bin);
            let upmix_end = end_bin;

            if upmix_start < upmix_end {
                // Perceptually-weighted ambient gain for better envelopment
                // Base: sqrt(1 - coherence) for energy preservation
                // Boost: configurable ambient_boost (default 1.2x) for enhanced spatial impression
                //
                // Dialogue detection: redistribute energy from ambient to center
                // while preserving total energy to avoid saturation
                let base_ambient_gain =
                    base_ambient_gain_from_coherence(coherence, self.ambient_boost);

                // Energy redistribution for dialogue: shift energy to center while maintaining total
                // dialogue_weight ranges from 0.0 (no dialogue) to configurable maximum
                let dialogue_weight = self.dialogue_probability * self.dialogue_weight;

                // Reduce ambient proportionally
                let ambient_gain = base_ambient_gain * (1.0 - dialogue_weight);

                // Increase direct/center coherence proportionally (not as a boost multiplier)
                let effective_coherence = coherence + (1.0 - coherence) * dialogue_weight;

                // 3B: Compute principal eigenvector from smoothed 2x2 covariance
                // for eigenvector projection direct/ambient split.
                // Eigenvector of [[c_xx, c_xy], [conj(c_xy), c_yy]] for lambda1:
                //   v = [c_xy, lambda1 - c_xx] (or fallback for degenerate case)
                let eigvec = if c_xy.norm_sqr() > 1e-18 {
                    let v = Complex::new(lambda1 - c_xx, 0.0);
                    let norm = (c_xy.norm_sqr() + v.norm_sqr()).sqrt();
                    if norm > 1e-9 {
                        (c_xy / norm, Complex::new(v.re / norm, 0.0))
                    } else {
                        (Complex::new(std::f32::consts::FRAC_1_SQRT_2, 0.0),
                         Complex::new(std::f32::consts::FRAC_1_SQRT_2, 0.0))
                    }
                } else {
                    // Degenerate case: equal power, use mid-side fallback
                    (Complex::new(std::f32::consts::FRAC_1_SQRT_2, 0.0),
                     Complex::new(std::f32::consts::FRAC_1_SQRT_2, 0.0))
                };
                let (ev_l, ev_r) = eigvec;

                for i in upmix_start..upmix_end {
                    let left = self.freq_domain_left[i];
                    let right = self.freq_domain_right[i];

                    // 3B: Project L/R onto principal eigenvector for direct component
                    // direct_projection = (L * conj(ev_l) + R * conj(ev_r))
                    let projection = left * ev_l.conj() + right * ev_r.conj();

                    // Direct = projection * eigenvector, scaled by effective_coherence
                    let direct_l = projection * ev_l * effective_coherence;
                    let direct_r = projection * ev_r * effective_coherence;
                    let direct_val = direct_l + direct_r; // mono direct for center
                    self.direct[i] = direct_val * 0.5;

                    // 3B: Ambient = residual after eigenvector projection
                    let residual_l = left - projection * ev_l;
                    let residual_r = right - projection * ev_r;
                    self.ambient_left[i] = residual_l * ambient_gain + (left - right) * ambient_gain * 0.3;
                    self.ambient_right[i] = residual_r * ambient_gain - (left - right) * ambient_gain * 0.3;

                    // Divergence for Fronts
                    self.direct_left[i] = left - self.direct[i] * self.stereo_width;
                    self.direct_right[i] = right - self.direct[i] * self.stereo_width;
                    self.lfe[i] = Complex::new(0.0, 0.0);

                    // 3F: Energy preservation correction
                    // Normalize so |direct|^2 + |ambient_L|^2 + |ambient_R|^2 ≈ |L|^2 + |R|^2
                    let input_energy =
                        left.norm_sqr() + right.norm_sqr();
                    let output_energy = self.direct[i].norm_sqr()
                        + self.direct_left[i].norm_sqr()
                        + self.direct_right[i].norm_sqr()
                        + self.ambient_left[i].norm_sqr()
                        + self.ambient_right[i].norm_sqr();
                    if output_energy > 1e-12 && input_energy > 1e-12 {
                        let correction = (input_energy / output_energy).sqrt();
                        // Gentle correction: limit to ±3dB to avoid artifacts
                        let correction = correction.clamp(0.707, 1.414);
                        self.direct[i] *= correction;
                        self.direct_left[i] *= correction;
                        self.direct_right[i] *= correction;
                        self.ambient_left[i] *= correction;
                        self.ambient_right[i] *= correction;
                    }

                    // Height mask: emphasize high-frequency, low-coherence (diffuse) content
                    // with reduced aggression to prevent "tizzy" artifacts
                    let nyquist = self.sample_rate as f32 / 2.0;
                    let freq = (i as f32 * self.sample_rate as f32) / self.fft_size as f32;
                    let hf_start = self.bandpass_hz.max(self.lfe_cutoff_hz);
                    let hf_end = self.height_hf_cap_hz.min(nyquist); // Configurable HF cap

                    let hf_ratio = if freq <= hf_start {
                        0.0
                    } else if freq >= hf_end {
                        1.0
                    } else {
                        (freq - hf_start) / (hf_end - hf_start)
                    };

                    // Reduced from sqrt() to linear^0.7 for gentler emphasis
                    let freq_weight = hf_ratio.powf(0.7);
                    let diffuse = (1.0 - coherence).max(0.0);

                    // Height suitability: additive blend allows direct HF content
                    // This prevents pure multiplicative gating (freq_weight * diffuse)
                    // which would block coherent high frequencies from reaching heights.
                    // 50/50 blend: some direct HF + some ambient = natural overhead sound
                    let height_suitability = (freq_weight * 0.5 + diffuse * 0.5).min(1.0);

                    // Transient-adaptive reduction: keep transients coherent
                    // During transients, reduce height channel emphasis by configurable amount
                    let transient_reduction = 1.0
                        - (self.hr_transient_env * self.height_transient_reduction)
                            .min(self.height_transient_reduction);

                    let height_mask = (height_suitability * transient_reduction).min(1.0);

                    let half_len = self.height_band_gains.len();
                    if i < half_len {
                        self.height_band_gains[i] = height_mask;
                    }
                }

                // Transient-adaptive decorrelation: reduce decorrelation during transients
                // to keep transients coherent and prevent "tizzy" artifacts.
                //
                // During transients (hr_transient_env approaching 1.0):
                // - decorrelation_strength approaches 0.0
                // - Filters approach identity (no decorrelation)
                //
                // During steady-state (hr_transient_env = 0.0):
                // - decorrelation_strength = 1.0
                // - Full decorrelation effect
                //
                // Dialogue-adaptive decorrelation: reduce decorrelation for dialogue
                // to keep vocals coherent and prevent metallic artifacts
                // Reduced transient impact (0.5 instead of 0.85) to prevent audible surround pumping
                let base_decorr_strength = (1.0 - self.hr_transient_env * 0.5).max(0.3);
                let dialogue_decorr_reduction = 1.0 - (self.dialogue_probability * 0.7); // Reduce by up to 70%
                let decorrelation_strength =
                    (base_decorr_strength * dialogue_decorr_reduction).max(0.05);

                // Apply transient-adaptive and dialogue-adaptive decorrelation
                self.apply_adaptive_decorrelation(upmix_start, upmix_end, decorrelation_strength);
            }
        }

        // Advance coherence history ring buffer index (once per frame)
        self.coherence_history_idx = self.coherence_history_idx.wrapping_add(1);

        // Flush denormals from frequency domain buffers to prevent CPU performance issues
        flush_denormals_complex_inplace(&mut self.direct);
        flush_denormals_complex_inplace(&mut self.direct_left);
        flush_denormals_complex_inplace(&mut self.direct_right);
        flush_denormals_complex_inplace(&mut self.ambient_left);
        flush_denormals_complex_inplace(&mut self.ambient_right);
        flush_denormals_complex_inplace(&mut self.lfe);

        // Apply spectral and temporal smoothing to height_band_gains
        self.smooth_height_gains();
    }
}

#[cfg(test)]
mod tests {
    use super::base_ambient_gain_from_coherence;

    #[test]
    fn base_ambient_gain_is_finite_for_out_of_range_coherence() {
        let values = [-10.0_f32, -1.0, 0.0, 0.5, 0.999_999, 1.0, 1.000_001, 10.0];
        let ambient_boost = 1.0; // default value
        for &c in &values {
            let g = base_ambient_gain_from_coherence(c, ambient_boost);
            assert!(
                g.is_finite(),
                "base_ambient_gain not finite for coherence={}",
                c
            );
            assert!(g >= 0.0);
        }
    }
}
