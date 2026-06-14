use super::consts::FEATURE_SIZE;
use super::consts::FRAME_FEATURE_SIZE;
use super::consts::NUM_AUX_FEATURES;
use super::consts::NUM_MEL_BANDS;
use super::consts::NUM_MFCCS;
use super::hz::hz_to_bin;
use super::hz::hz_to_mel;
use super::misc::mel_to_hz;
use super::types::MelFilter;
use rustfft::num_complex::Complex;

/// Zero-allocation feature extractor that reuses existing FFT data.
pub struct MfccExtractor {
    pub(super) sample_rate: u32,
    pub(super) fft_size: usize,
    pub(super) filter_weights: Vec<(usize, f32)>,
    pub(super) mel_filters: Vec<MelFilter>,
    pub(super) dct_matrix: Vec<f32>,
    pub(super) mel_energies: Vec<f32>,
    pub(super) log_mel_energies: Vec<f32>,
    pub(super) frame_features: Vec<f32>,
    pub(super) context_features: Vec<f32>,
    pub(super) mono_power: Vec<f32>,
    pub(super) prev_power: Vec<f32>,
    pub(super) prev_mfccs: Vec<f32>,
    pub(super) has_prev: bool,
}

impl MfccExtractor {
    /// Create a new feature extractor.
    pub fn new(sample_rate: u32, fft_size: usize) -> Self {
        let spectrum_size = fft_size / 2 + 1;
        let nyquist = sample_rate as f32 / 2.0;

        let mel_low = hz_to_mel(0.0);
        let mel_high = hz_to_mel(nyquist);
        let num_points = NUM_MEL_BANDS + 2;
        let mel_points: Vec<f32> = (0..num_points)
            .map(|i| mel_low + (mel_high - mel_low) * i as f32 / (num_points - 1) as f32)
            .collect();
        let hz_points: Vec<f32> = mel_points.iter().map(|&m| mel_to_hz(m)).collect();
        let bin_points: Vec<f32> = hz_points
            .iter()
            .map(|&f| f * fft_size as f32 / sample_rate as f32)
            .collect();

        let mut filter_weights = Vec::new();
        let mut mel_filters = Vec::with_capacity(NUM_MEL_BANDS);
        for band in 0..NUM_MEL_BANDS {
            let left = bin_points[band];
            let center = bin_points[band + 1];
            let right = bin_points[band + 2];
            let bin_start = left.floor() as usize;
            let bin_end = (right.ceil() as usize).min(spectrum_size - 1);
            let offset = filter_weights.len();
            let mut count = 0;

            for bin in bin_start..=bin_end {
                let bin_f = bin as f32;
                let weight = if bin_f <= left {
                    0.0
                } else if bin_f <= center {
                    (bin_f - left) / (center - left)
                } else if bin_f <= right {
                    (right - bin_f) / (right - center)
                } else {
                    0.0
                };
                if weight > 0.0 {
                    filter_weights.push((bin, weight));
                    count += 1;
                }
            }

            mel_filters.push(MelFilter { offset, len: count });
        }

        let mut dct_matrix = vec![0.0_f32; NUM_MFCCS * NUM_MEL_BANDS];
        for k in 0..NUM_MFCCS {
            for n in 0..NUM_MEL_BANDS {
                dct_matrix[k * NUM_MEL_BANDS + n] =
                    (std::f32::consts::PI * k as f32 * (n as f32 + 0.5) / NUM_MEL_BANDS as f32)
                        .cos();
            }
        }

        Self {
            sample_rate,
            fft_size,
            filter_weights,
            mel_filters,
            dct_matrix,
            mel_energies: vec![0.0; NUM_MEL_BANDS],
            log_mel_energies: vec![0.0; NUM_MEL_BANDS],
            frame_features: vec![0.0; FRAME_FEATURE_SIZE],
            context_features: vec![0.0; FEATURE_SIZE],
            mono_power: vec![0.0; spectrum_size],
            prev_power: vec![0.0; spectrum_size],
            prev_mfccs: vec![0.0; NUM_MFCCS],
            has_prev: false,
        }
    }

    /// Compute flattened temporal-context features from existing FFT spectra.
    #[inline]
    pub fn compute(
        &mut self,
        freq_left: &[Complex<f32>],
        freq_right: &[Complex<f32>],
    ) -> &[f32; FEATURE_SIZE] {
        for i in 0..self.mono_power.len() {
            self.mono_power[i] = (freq_left[i].norm_sqr() + freq_right[i].norm_sqr()) * 0.5;
        }

        for (band_idx, filter) in self.mel_filters.iter().enumerate() {
            let mut energy = 0.0_f32;
            let pairs = &self.filter_weights[filter.offset..filter.offset + filter.len];
            for &(bin, weight) in pairs {
                energy += self.mono_power[bin] * weight;
            }
            self.mel_energies[band_idx] = energy;
        }

        const LOG_FLOOR: f32 = 1e-10;
        for i in 0..NUM_MEL_BANDS {
            self.log_mel_energies[i] = (self.mel_energies[i] + LOG_FLOOR).ln();
        }

        for k in 0..NUM_MFCCS {
            let row = &self.dct_matrix[k * NUM_MEL_BANDS..(k + 1) * NUM_MEL_BANDS];
            let mut sum = 0.0_f32;
            for (n, &dct_coeff) in row.iter().enumerate() {
                sum += dct_coeff * self.log_mel_energies[n];
            }
            self.frame_features[k] = sum;
        }

        if self.has_prev {
            for k in 0..NUM_MFCCS {
                self.frame_features[NUM_MFCCS + k] = self.frame_features[k] - self.prev_mfccs[k];
            }
        } else {
            self.frame_features[NUM_MFCCS..NUM_MFCCS * 2].fill(0.0);
        }
        self.prev_mfccs
            .copy_from_slice(&self.frame_features[..NUM_MFCCS]);

        self.compute_aux_features(freq_left, freq_right);
        self.has_prev = true;

        self.context_features.copy_within(FRAME_FEATURE_SIZE.., 0);
        self.context_features[FEATURE_SIZE - FRAME_FEATURE_SIZE..FEATURE_SIZE]
            .copy_from_slice(&self.frame_features);

        self.context_features[..FEATURE_SIZE].try_into().unwrap()
    }

    #[inline]
    pub(super) fn compute_aux_features(
        &mut self,
        freq_left: &[Complex<f32>],
        freq_right: &[Complex<f32>],
    ) {
        let spectrum_size = self.mono_power.len();
        let freq_per_bin = self.sample_rate as f32 / self.fft_size as f32;
        let nyquist = self.sample_rate as f32 * 0.5;
        let voice_start = hz_to_bin(200.0, freq_per_bin, spectrum_size);
        let voice_end = hz_to_bin(5000.0, freq_per_bin, spectrum_size);
        let low_end = hz_to_bin(250.0, freq_per_bin, spectrum_size);
        let low_mid_end = hz_to_bin(500.0, freq_per_bin, spectrum_size);
        let mid_end = hz_to_bin(2000.0, freq_per_bin, spectrum_size);
        let high_mid_end = hz_to_bin(5000.0, freq_per_bin, spectrum_size);
        let eps = 1e-12_f32;

        let mut left_energy = 0.0;
        let mut right_energy = 0.0;
        let mut mid_energy = 0.0;
        let mut side_energy = 0.0;
        let mut voice_energy = 0.0;
        let mut voice_mid_energy = 0.0;
        let mut voice_side_energy = 0.0;
        let mut voice_left_energy = 0.0;
        let mut voice_right_energy = 0.0;
        let mut cross = Complex::new(0.0, 0.0);
        let mut voice_cross = Complex::new(0.0, 0.0);
        let mut centroid_num = 0.0;
        let mut flux = 0.0;
        let mut band_low = 0.0;
        let mut band_low_mid = 0.0;
        let mut band_mid = 0.0;
        let mut band_high_mid = 0.0;
        let mut band_high = 0.0;

        for i in 0..spectrum_size {
            let l = freq_left[i];
            let r = freq_right[i];
            let l_pow = l.norm_sqr();
            let r_pow = r.norm_sqr();
            let mono = self.mono_power[i];
            let mid = (l + r) * 0.5;
            let side = (l - r) * 0.5;
            let freq = i as f32 * freq_per_bin;

            left_energy += l_pow;
            right_energy += r_pow;
            mid_energy += mid.norm_sqr();
            side_energy += side.norm_sqr();
            cross += l * r.conj();
            centroid_num += freq * mono;

            let diff = mono - self.prev_power[i];
            if self.has_prev && diff > 0.0 {
                flux += diff;
            }
            self.prev_power[i] = mono;

            if i <= low_end {
                band_low += mono;
            } else if i <= low_mid_end {
                band_low_mid += mono;
            } else if i <= mid_end {
                band_mid += mono;
            } else if i <= high_mid_end {
                band_high_mid += mono;
            } else {
                band_high += mono;
            }

            if i >= voice_start && i <= voice_end {
                voice_energy += mono;
                voice_mid_energy += mid.norm_sqr();
                voice_side_energy += side.norm_sqr();
                voice_left_energy += l_pow;
                voice_right_energy += r_pow;
                voice_cross += l * r.conj();
            }
        }

        let total_energy = left_energy + right_energy;
        let mono_total = self.mono_power.iter().sum::<f32>();
        let centroid = if mono_total > eps {
            centroid_num / mono_total
        } else {
            0.0
        };
        let mut spread_num = 0.0;
        for (i, &power) in self.mono_power.iter().enumerate() {
            let freq = i as f32 * freq_per_bin;
            let d = freq - centroid;
            spread_num += d * d * power;
        }
        let spread = if mono_total > eps {
            (spread_num / mono_total).sqrt()
        } else {
            0.0
        };

        let energy_root = (left_energy * right_energy).sqrt();
        let voice_energy_root = (voice_left_energy * voice_right_energy).sqrt();
        let correlation = if energy_root > eps {
            cross.re / energy_root
        } else {
            0.0
        };
        let phase_coherence = if energy_root > eps {
            cross.norm() / energy_root
        } else {
            0.0
        };
        let voice_correlation = if voice_energy_root > eps {
            voice_cross.re / voice_energy_root
        } else {
            0.0
        };
        let voice_phase_coherence = if voice_energy_root > eps {
            voice_cross.norm() / voice_energy_root
        } else {
            0.0
        };

        let offset = NUM_MFCCS * 2;
        let aux = &mut self.frame_features[offset..offset + NUM_AUX_FEATURES];
        aux[0] = (mono_total + eps).ln();
        aux[1] = (mid_energy + eps).ln();
        aux[2] = (side_energy + eps).ln();
        aux[3] = mid_energy / (mid_energy + side_energy + eps);
        aux[4] = side_energy / (mid_energy + side_energy + eps);
        aux[5] = (left_energy - right_energy) / (total_energy + eps);
        aux[6] = 1.0 - (left_energy - right_energy).abs() / (total_energy + eps);
        aux[7] = correlation.clamp(-1.0, 1.0);
        aux[8] = phase_coherence.clamp(0.0, 1.0);
        aux[9] = voice_energy / (mono_total + eps);
        aux[10] = voice_mid_energy / (voice_mid_energy + voice_side_energy + eps);
        aux[11] = voice_side_energy / (voice_mid_energy + voice_side_energy + eps);
        aux[12] = (voice_left_energy - voice_right_energy)
            / (voice_left_energy + voice_right_energy + eps);
        aux[13] = 1.0
            - (voice_left_energy - voice_right_energy).abs()
                / (voice_left_energy + voice_right_energy + eps);
        aux[14] = voice_correlation.clamp(-1.0, 1.0);
        aux[15] = voice_phase_coherence.clamp(0.0, 1.0);
        aux[16] = centroid / nyquist.max(1.0);
        aux[17] = spread / nyquist.max(1.0);
        aux[18] = if self.has_prev {
            flux / (mono_total + eps)
        } else {
            0.0
        };
        aux[19] = band_low / (mono_total + eps);
        aux[20] = band_low_mid / (mono_total + eps);
        aux[21] = band_mid / (mono_total + eps);
        aux[22] = band_high_mid / (mono_total + eps);
        aux[23] = band_high / (mono_total + eps);
    }

    /// Reset state (e.g., on stream discontinuity).
    pub fn reset(&mut self) {
        self.has_prev = false;
        self.prev_mfccs.fill(0.0);
        self.prev_power.fill(0.0);
        self.frame_features.fill(0.0);
        self.context_features.fill(0.0);
    }
}
