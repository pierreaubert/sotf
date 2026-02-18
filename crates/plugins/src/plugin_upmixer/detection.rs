// ============================================================================
// Dialogue Detection
// ============================================================================

use super::UpmixerPlugin;

impl UpmixerPlugin {
    #[inline]
    pub(super) fn detect_dialogue(&mut self) -> f32 {
        let spectrum_size = self.fft_size / 2 + 1;
        let freq_per_bin = self.sample_rate as f32 / self.fft_size as f32;

        let voice_start_bin = (self.voice_freq_min_hz / freq_per_bin) as usize;
        let voice_end_bin =
            (self.voice_freq_max_hz / freq_per_bin).min(spectrum_size as f32 - 1.0) as usize;

        let mut weighted_sum = 0.0_f32;
        let mut power_sum = 0.0_f32;

        for i in voice_start_bin..=voice_end_bin {
            let p =
                (self.freq_domain_left[i].norm_sqr() + self.freq_domain_right[i].norm_sqr()) * 0.5;
            weighted_sum += (i as f32 * freq_per_bin) * p;
            power_sum += p;
        }

        let centroid = if power_sum > 1e-9 {
            weighted_sum / power_sum
        } else {
            0.0
        };
        self.dialogue_spectral_centroid += 0.3 * (centroid - self.dialogue_spectral_centroid);

        let rms = (power_sum * 2.0 / ((voice_end_bin - voice_start_bin + 1) as f32 * 2.0)).sqrt();
        let env_diff = if self.dialogue_prev_rms > 1e-9 {
            ((rms - self.dialogue_prev_rms) / self.dialogue_prev_rms).abs()
        } else {
            1.0
        };
        self.dialogue_prev_rms = rms;
        self.dialogue_envelope_variance += 0.2 * (env_diff - self.dialogue_envelope_variance);

        let c_min = 800.0;
        let c_max = 2000.0;
        let c_score = if self.dialogue_spectral_centroid >= c_min
            && self.dialogue_spectral_centroid <= c_max
        {
            1.0
        } else if self.dialogue_spectral_centroid < c_min {
            ((self.dialogue_spectral_centroid - self.voice_freq_min_hz)
                / (c_min - self.voice_freq_min_hz))
                .clamp(0.0, 1.0)
        } else {
            (1.0 - ((self.dialogue_spectral_centroid - c_max) / (self.voice_freq_max_hz - c_max)))
                .clamp(0.0, 1.0)
        };

        let v_score = (1.0 - (self.dialogue_envelope_variance / 0.4).min(1.0)).max(0.0);

        let mut coh_sum = 0.0f32;
        let mut coh_count = 0;
        for band_idx in 0..self.erb_bands.len() {
            let start = self.erb_bands[band_idx];
            let end = if band_idx + 1 < self.erb_bands.len() {
                self.erb_bands[band_idx + 1]
            } else {
                spectrum_size
            };
            let cf = ((start + end) / 2) as f32 * freq_per_bin;
            if cf >= self.voice_freq_min_hz && cf <= self.voice_freq_max_hz {
                coh_sum += self.smoothed_coherence[band_idx];
                coh_count += 1;
            }
        }
        let voice_coh = if coh_count > 0 {
            coh_sum / coh_count as f32
        } else {
            0.0
        };

        let w_sum = self.dialogue_centroid_weight + self.dialogue_variance_weight + self.dialogue_coherence_weight;
        let (w_c, w_v, w_coh) = if w_sum > 1e-9 {
            (
                self.dialogue_centroid_weight / w_sum,
                self.dialogue_variance_weight / w_sum,
                self.dialogue_coherence_weight / w_sum,
            )
        } else {
            (0.333, 0.333, 0.334)
        };
        let prob = c_score * w_c + v_score * w_v + voice_coh * w_coh;
        let p_alpha = if prob > self.dialogue_probability {
            0.1
        } else {
            0.05
        };
        self.dialogue_probability += p_alpha * (prob - self.dialogue_probability);

        self.dialogue_probability
    }
}
