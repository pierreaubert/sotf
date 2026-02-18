// ============================================================================
// Frequency Domain Processing with ERB Bands
// ============================================================================

use super::UpmixerPlugin;
use crate::simd::compute_covariance_simd;

use rustfft::num_complex::Complex;

/// Minimum height mask value — prevents deep spectral notches that cause time-domain ringing
const HEIGHT_MASK_FLOOR: f32 = 0.02;

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

/// Minimum energy correction ratio (prevents over-attenuation)
const ENERGY_CORRECTION_MIN: f32 = 0.85;
/// Maximum energy correction ratio (prevents over-boost)
const ENERGY_CORRECTION_MAX: f32 = 1.15;

/// 5-element median using 6 comparisons (optimal).
/// After eliminating the global minimum via 3 compare-swaps on pairs,
/// finds the 2nd-smallest of the remaining 4 elements (= median of 5).
#[inline(always)]
fn median5(arr: [f32; 5]) -> f32 {
    let [mut a, mut b, mut c, mut d, mut e] = arr;
    // Sort pairs: a ≤ b, c ≤ d                        (2 comparisons)
    if a > b {
        std::mem::swap(&mut a, &mut b);
    }
    if c > d {
        std::mem::swap(&mut c, &mut d);
    }
    // Order pairs so a ≤ c (thus a = min of {a,b,c,d}) (1 comparison)
    if a > c {
        std::mem::swap(&mut a, &mut c);
        std::mem::swap(&mut b, &mut d);
    }
    // Discard a (global minimum). Need 2nd-smallest of {b, c, d, e} where c ≤ d.
    // Sort b,e so b ≤ e                                (1 comparison)
    if b > e {
        std::mem::swap(&mut b, &mut e);
    }
    // Now b ≤ e, c ≤ d. 2nd-of-4 from two sorted pairs:
    // merge-pick index 1 = if b ≤ c then min(c, e) else min(b, d)
    if b <= c {
        // (1 comparison)
        if c <= e { c } else { e } // (1 comparison)
    } else {
        if b <= d { b } else { d }
    }
}

fn base_ambient_gain_from_coherence(coherence: f32, ambient_boost: f32) -> f32 {
    let coherence_clamped = coherence.clamp(0.0, 1.0);
    let ambient_base = (1.0 - coherence_clamped).max(0.0);
    // Standard sqrt is okay here as it's once per ERB band, but we could use an approximation if needed.
    ambient_base.sqrt() * ambient_boost
}

impl UpmixerPlugin {
    pub(super) fn process_frequency_domain_erb_bands(&mut self) {
        if self.sample_rate == 0 || self.fft_size == 0 {
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
            let attack_alpha = STEERING_ATTACK_BASE + STEERING_ATTACK_RANGE * norm;
            let release_alpha = STEERING_RELEASE_BASE + STEERING_RELEASE_RANGE * norm;
            let alpha = if inst_energy > smooth_energy * 1.5 {
                attack_alpha
            } else {
                release_alpha
            };
            self.steering_alphas[band_idx] = alpha;

            self.pca_cov_xx[band_idx] = (1.0 - alpha) * self.pca_cov_xx[band_idx] + alpha * cov_xx;
            self.pca_cov_yy[band_idx] = (1.0 - alpha) * self.pca_cov_yy[band_idx] + alpha * cov_yy;
            self.pca_cov_xy[band_idx] = (1.0 - alpha) * self.pca_cov_xy[band_idx] + alpha * cov_xy;

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
                self.smoothed_coherence[band_idx] = prev + COHERENCE_SMOOTHING_ALPHA * (median - prev);
                coherence = self.smoothed_coherence[band_idx];
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
                    self.ambient_left[i] = Complex::new(0.0, 0.0);
                    self.ambient_right[i] = Complex::new(0.0, 0.0);
                }
            }

            // Pass-through Band
            let pass_start = (lfe_cutoff_bin + 1).max(start_bin);
            let pass_end = bandpass_bin.min(end_bin);
            if pass_start < pass_end {
                for i in pass_start..pass_end {
                    self.direct_left[i] = self.freq_domain_left[i];
                    self.direct_right[i] = self.freq_domain_right[i];
                    self.lfe[i] = Complex::new(0.0, 0.0);
                    self.ambient_left[i] = Complex::new(0.0, 0.0);
                    self.ambient_right[i] = Complex::new(0.0, 0.0);
                }
            }

            // Upmixing Band
            let upmix_start = bandpass_bin.max(start_bin);
            if upmix_start < end_bin {
                let ambient_gain = base_ambient_gain_from_coherence(coherence, self.ambient_boost.current())
                    * (1.0 - self.dialogue_probability * self.dialogue_weight.current());
                let eff_coh = coherence
                    + (1.0 - coherence) * (self.dialogue_probability * self.dialogue_weight.current());

                let (ev_l, ev_r) = if c_xy.norm_sqr() > 1e-18 {
                    let v = lambda1 - c_xx;
                    let norm = (c_xy.norm_sqr() + v * v).sqrt();
                    if norm > 1e-9 {
                        (c_xy / norm, Complex::new(v / norm, 0.0))
                    } else {
                        (
                            Complex::new(std::f32::consts::FRAC_1_SQRT_2, 0.0),
                            Complex::new(std::f32::consts::FRAC_1_SQRT_2, 0.0),
                        )
                    }
                } else {
                    (
                        Complex::new(std::f32::consts::FRAC_1_SQRT_2, 0.0),
                        Complex::new(std::f32::consts::FRAC_1_SQRT_2, 0.0),
                    )
                };

                let mut in_e = 0.0f32;
                let mut out_e = 0.0f32;
                for i in upmix_start..end_bin {
                    let l = self.freq_domain_left[i];
                    let r = self.freq_domain_right[i];
                    let proj = l * ev_l.conj() + r * ev_r.conj();
                    let direct_l = proj * ev_l;
                    let direct_r = proj * ev_r;
                    // Phase-align left and right projections before summing to center
                    let phase_product = direct_r * direct_l.conj();
                    let phase_correction = if phase_product.norm_sqr() > 1e-18 {
                        phase_product / phase_product.norm()
                    } else {
                        Complex::new(1.0, 0.0)
                    };
                    let aligned_r = direct_r * phase_correction.conj();
                    self.direct[i] = (direct_l + aligned_r) * (eff_coh * 0.25);
                    self.ambient_left[i] =
                        (l - direct_l) * ambient_gain + (l - r) * (0.3 * ambient_gain);
                    self.ambient_right[i] =
                        (r - direct_r) * ambient_gain - (l - r) * (0.3 * ambient_gain);
                    self.direct_left[i] = l - self.direct[i] * self.stereo_width.current();
                    self.direct_right[i] = r - self.direct[i] * self.stereo_width.current();
                    self.lfe[i] = Complex::new(0.0, 0.0);
                    in_e += l.norm_sqr() + r.norm_sqr();
                    out_e += self.direct[i].norm_sqr()
                        + self.direct_left[i].norm_sqr()
                        + self.direct_right[i].norm_sqr()
                        + self.ambient_left[i].norm_sqr()
                        + self.ambient_right[i].norm_sqr();
                }

                let corr = if out_e > 1e-12 && in_e > 1e-12 {
                    (in_e / out_e).sqrt().clamp(ENERGY_CORRECTION_MIN, ENERGY_CORRECTION_MAX)
                } else {
                    1.0
                };
                let tr_red = 1.0
                    - (self.hr_transient_env * self.height_transient_reduction.current())
                        .min(self.height_transient_reduction.current());
                for i in upmix_start..end_bin {
                    self.energy_correction_per_bin[i] = corr;
                    let h_suit = (self.height_freq_weights[i] * 0.5
                        + (1.0 - coherence).max(0.0) * 0.5)
                        .min(1.0);
                    self.height_band_gains[i] = (h_suit * tr_red).clamp(HEIGHT_MASK_FLOOR, 1.0);
                }
            }
        }

        self.smooth_and_apply_energy_correction();
        let strength = ((1.0 - self.hr_transient_env * 0.5).max(0.3)
            * (1.0 - self.dialogue_probability * 0.7))
            .max(0.05);
        self.decorrelation_strength = strength;

        let spec_size = self.fft_size / 2 + 1;
        let num_ch = self.num_output_channels;
        if self.blended_decorrelation_filters.len() != num_ch {
            self.blended_decorrelation_filters =
                vec![vec![Complex::new(1.0, 0.0); spec_size]; num_ch];
            self.prev_decorrelation_strength = -1.0; // Force recompute
        }

        // Only reblend when strength changed significantly, or in LFO mode
        // (LFO mode updates the underlying decorrelation filters every block)
        let needs_reblend = self.decorrelation_mode == 1
            || (strength - self.prev_decorrelation_strength).abs() > 0.01;

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
                    for i in 0..spec_size {
                        self.blended_decorrelation_filters[ch][i] =
                            Complex::new(strength * decor[i].re + id_w, strength * decor[i].im);
                    }
                }
            }
        }

        self.coherence_history_idx = self.coherence_history_idx.wrapping_add(1);
        // Note: FTZ/DAZ CPU flags handle denormal flushing automatically
        self.smooth_height_gains();
    }

    #[inline]
    fn smooth_and_apply_energy_correction(&mut self) {
        let spec_size = self.fft_size / 2 + 1;
        let mut smoothed = std::mem::take(&mut self.energy_correction_temp);
        for i in 0..spec_size {
            let start = i.saturating_sub(1);
            let end = (i + 2).min(spec_size);
            let mut sum = 0.0f32;
            let mut count = 0;
            for j in start..end {
                sum += self.energy_correction_per_bin[j];
                count += 1;
            }
            smoothed[i] = sum / count as f32;
        }

        for i in 0..spec_size {
            let prev = self.energy_correction_prev[i];
            let alpha = if smoothed[i] < prev { 0.3 } else { 0.1 };
            let blended = alpha * smoothed[i] + (1.0 - alpha) * prev;
            self.energy_correction_prev[i] = blended;
            self.direct[i] *= blended;
            self.direct_left[i] *= blended;
            self.direct_right[i] *= blended;
            self.ambient_left[i] *= blended;
            self.ambient_right[i] *= blended;
        }
        self.energy_correction_temp = smoothed;
    }
}
