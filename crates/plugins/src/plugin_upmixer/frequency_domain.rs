// ============================================================================
// Frequency Domain Processing with ERB Bands
// ============================================================================

use super::UpmixerPlugin;
use crate::simd::{compute_covariance_simd, flush_denormals_complex_inplace, flush_denormals_inplace};
use rustfft::num_complex::Complex;

/// Minimum height mask value — prevents deep spectral notches that cause time-domain ringing
const HEIGHT_MASK_FLOOR: f32 = 0.02;

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
            let lfe_end = (lfe_cutoff_bin + 1).min(end_bin);
            if start_bin < lfe_end {
                let loop_start = start_bin;
                let loop_end = lfe_end;

                for i in loop_start..loop_end {
                    let left = self.freq_domain_left[i];
                    let right = self.freq_domain_right[i];

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
            let upmix_start = bandpass_bin.max(start_bin);
            let upmix_end = end_bin;

            if upmix_start < upmix_end {
                // Perceptually-weighted ambient gain
                let base_ambient_gain =
                    base_ambient_gain_from_coherence(coherence, self.ambient_boost);

                // Energy redistribution for dialogue
                let dialogue_weight = self.dialogue_probability * self.dialogue_weight;
                let ambient_gain = base_ambient_gain * (1.0 - dialogue_weight);
                let effective_coherence = coherence + (1.0 - coherence) * dialogue_weight;

                // 3B: Compute principal eigenvector
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
                    (Complex::new(std::f32::consts::FRAC_1_SQRT_2, 0.0),
                     Complex::new(std::f32::consts::FRAC_1_SQRT_2, 0.0))
                };
                let (ev_l, ev_r) = eigvec;

                // Optimization: Pre-calculate per-band energy sums
                let mut input_energy_band = 0.0_f32;
                let mut output_energy_band = 0.0_f32;

                for i in upmix_start..upmix_end {
                    let left = self.freq_domain_left[i];
                    let right = self.freq_domain_right[i];

                    let projection = left * ev_l.conj() + right * ev_r.conj();
                    let direct_l = projection * ev_l * effective_coherence;
                    let direct_r = projection * ev_r * effective_coherence;
                    let direct_val = direct_l + direct_r;
                    self.direct[i] = direct_val * 0.5;

                    let residual_l = left - projection * ev_l;
                    let residual_r = right - projection * ev_r;
                    let side = (left - right) * 0.3;
                    self.ambient_left[i] = residual_l * ambient_gain + side * ambient_gain;
                    self.ambient_right[i] = residual_r * ambient_gain - side * ambient_gain;

                    self.direct_left[i] = left - self.direct[i] * self.stereo_width;
                    self.direct_right[i] = right - self.direct[i] * self.stereo_width;
                    self.lfe[i] = Complex::new(0.0, 0.0);

                    input_energy_band += left.norm_sqr() + right.norm_sqr();
                    output_energy_band += self.direct[i].norm_sqr()
                        + self.direct_left[i].norm_sqr()
                        + self.direct_right[i].norm_sqr()
                        + self.ambient_left[i].norm_sqr()
                        + self.ambient_right[i].norm_sqr();
                }

                // 3F: Energy preservation correction (ONCE per band)
                if output_energy_band > 1e-12 && input_energy_band > 1e-12 {
                    let correction = (input_energy_band / output_energy_band).sqrt();
                    let correction = correction.clamp(0.707, 1.414);
                    
                    for i in upmix_start..upmix_end {
                        self.direct[i] *= correction;
                        self.direct_left[i] *= correction;
                        self.direct_right[i] *= correction;
                        self.ambient_left[i] *= correction;
                        self.ambient_right[i] *= correction;

                        let freq_weight = self.height_freq_weights[i];
                        let diffuse = (1.0 - coherence).max(0.0);
                        let height_suitability = (freq_weight * 0.5 + diffuse * 0.5).min(1.0);
                        let transient_reduction = 1.0 - (self.hr_transient_env * self.height_transient_reduction)
                            .min(self.height_transient_reduction);
                        
                        if i < self.height_band_gains.len() {
                            // Floor prevents deep spectral notches that cause time-domain ringing
                            self.height_band_gains[i] = (height_suitability * transient_reduction).clamp(HEIGHT_MASK_FLOOR, 1.0);
                        }
                    }
                } else {
                    let transient_reduction = 1.0 - (self.hr_transient_env * self.height_transient_reduction)
                        .min(self.height_transient_reduction);
                    for i in upmix_start..upmix_end {
                        let freq_weight = self.height_freq_weights[i];
                        let diffuse = (1.0 - coherence).max(0.0);
                        let height_suitability = (freq_weight * 0.5 + diffuse * 0.5).min(1.0);
                        if i < self.height_band_gains.len() {
                            self.height_band_gains[i] = (height_suitability * transient_reduction).clamp(HEIGHT_MASK_FLOOR, 1.0);
                        }
                    }
                }
            }
        }

        // Calculate adaptive decorrelation strength once per frame
        let base_decorr_strength = (1.0 - self.hr_transient_env * 0.5).max(0.3);
        let dialogue_decorr_reduction = 1.0 - (self.dialogue_probability * 0.7);
        let strength = (base_decorr_strength * dialogue_decorr_reduction).max(0.05);
        self.decorrelation_strength = strength;

        // Pre-calculate blended filters for all channels
        let spectrum_size = self.fft_size / 2 + 1;
        let num_ch = self.num_output_channels;
        
        if self.blended_decorrelation_filters.len() != num_ch {
            self.blended_decorrelation_filters = vec![vec![Complex::new(1.0, 0.0); spectrum_size]; num_ch];
        }

        let identity_weight = 1.0 - strength;
        let skip_blend = strength >= 0.99;

        for ch_idx in 0..num_ch {
            let speaker = &self.speaker_config.speakers[ch_idx];
            let is_front = speaker.azimuth.abs() < 80.0 && speaker.elevation.abs() < 10.0;
            
            if speaker.is_lfe || is_front {
                // Identity filter
                self.blended_decorrelation_filters[ch_idx].fill(Complex::new(1.0, 0.0));
                continue;
            }

            let decor = if ch_idx < self.decorrelation_filters.len() {
                &self.decorrelation_filters[ch_idx]
            } else if speaker.azimuth > 0.0 {
                &self.decorrelation_filter_left
            } else {
                &self.decorrelation_filter_right
            };

            if skip_blend {
                self.blended_decorrelation_filters[ch_idx].copy_from_slice(decor);
            } else {
                let target = &mut self.blended_decorrelation_filters[ch_idx];
                for i in 0..spectrum_size {
                    target[i] = Complex::new(
                        strength * decor[i].re + identity_weight,
                        strength * decor[i].im
                    );
                }
            }
        }

        // Advance coherence history ring buffer index (once per frame)
        self.coherence_history_idx = self.coherence_history_idx.wrapping_add(1);

        // Flush denormals from PCA covariance state arrays.
        flush_denormals_inplace(&mut self.pca_cov_xx);
        flush_denormals_inplace(&mut self.pca_cov_yy);
        flush_denormals_complex_inplace(&mut self.pca_cov_xy);

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