// ============================================================================
// Dialogue Detection (Heuristic + ML dispatch)
// ============================================================================

use super::UpmixerPlugin;

impl UpmixerPlugin {
    /// Main dialogue detection entry point.
    ///
    /// If ML detection is enabled and the inference thread has produced a result,
    /// uses the ML V_prob (with the same smoothing). Otherwise falls back to the
    /// heuristic detector.
    ///
    /// Also computes MFCC features and sends them to the inference thread when active.
    #[inline]
    pub(super) fn detect_dialogue(&mut self) -> f32 {
        // If ML detection is active, compute features and send to inference thread
        let ml_v_prob = self.try_ml_inference();

        match ml_v_prob {
            Some(v_prob) => {
                // Use ML probability with the same smoothing as heuristic
                let p_alpha = if v_prob > self.dialogue.dialogue_probability {
                    0.1
                } else {
                    0.05
                };
                self.dialogue.dialogue_probability +=
                    p_alpha * (v_prob - self.dialogue.dialogue_probability);
                self.dialogue.dialogue_probability
            }
            None => {
                // Fall back to heuristic detection
                self.detect_dialogue_heuristic()
            }
        }
    }

    /// Heuristic dialogue detection using spectral centroid, envelope variance,
    /// and voice-band coherence.
    #[inline]
    fn detect_dialogue_heuristic(&mut self) -> f32 {
        let spectrum_size = self.core.fft_size / 2 + 1;
        let freq_per_bin = self.cache.cached_freq_per_bin;

        let voice_start_bin = self.cache.cached_voice_start_bin;
        let voice_end_bin = self.cache.cached_voice_end_bin;

        let mut weighted_sum = 0.0_f32;
        let mut power_sum = 0.0_f32;

        for i in voice_start_bin..=voice_end_bin {
            let p = (self.main_buffers.freq_domain_left[i].norm_sqr()
                + self.main_buffers.freq_domain_right[i].norm_sqr())
                * 0.5;
            weighted_sum += (i as f32 * freq_per_bin) * p;
            power_sum += p;
        }

        let centroid = if power_sum > 1e-9 {
            weighted_sum / power_sum
        } else {
            0.0
        };
        self.dialogue.dialogue_spectral_centroid +=
            0.3 * (centroid - self.dialogue.dialogue_spectral_centroid);

        let rms = (power_sum * 2.0 / ((voice_end_bin - voice_start_bin + 1) as f32 * 2.0)).sqrt();
        let env_diff = if self.dialogue.dialogue_prev_rms > 1e-9 {
            ((rms - self.dialogue.dialogue_prev_rms) / self.dialogue.dialogue_prev_rms).abs()
        } else {
            1.0
        };
        self.dialogue.dialogue_prev_rms = rms;
        self.dialogue.dialogue_envelope_variance +=
            0.2 * (env_diff - self.dialogue.dialogue_envelope_variance);

        let c_min = 800.0;
        let c_max = 2000.0;
        let c_score = if self.dialogue.dialogue_spectral_centroid >= c_min
            && self.dialogue.dialogue_spectral_centroid <= c_max
        {
            1.0
        } else if self.dialogue.dialogue_spectral_centroid < c_min {
            ((self.dialogue.dialogue_spectral_centroid - self.dialogue.voice_freq_min_hz)
                / (c_min - self.dialogue.voice_freq_min_hz))
                .clamp(0.0, 1.0)
        } else {
            (1.0 - ((self.dialogue.dialogue_spectral_centroid - c_max)
                / (self.dialogue.voice_freq_max_hz - c_max)))
                .clamp(0.0, 1.0)
        };

        /// Maximum envelope variance before dialogue is ruled out.
        /// Derived empirically: speech has low envelope variance (~0.1-0.2 RMS change
        /// between frames relative to level), while percussive or transient-heavy content
        /// exceeds 0.4. This threshold gives a good separation between sustained vocal
        /// content and dynamic non-speech material in typical music and film mixes.
        const DIALOGUE_ENVELOPE_VARIANCE_CEILING: f32 = 0.4;

        let v_score = (1.0
            - (self.dialogue.dialogue_envelope_variance / DIALOGUE_ENVELOPE_VARIANCE_CEILING)
                .min(1.0))
        .max(0.0);

        let mut coh_sum = 0.0f32;
        let mut coh_count = 0;
        for band_idx in 0..self.steering.erb_bands.len() {
            let start = self.steering.erb_bands[band_idx];
            let end = if band_idx + 1 < self.steering.erb_bands.len() {
                self.steering.erb_bands[band_idx + 1]
            } else {
                spectrum_size
            };
            let cf = ((start + end) / 2) as f32 * freq_per_bin;
            if cf >= self.dialogue.voice_freq_min_hz && cf <= self.dialogue.voice_freq_max_hz {
                coh_sum += self.steering.smoothed_coherence[band_idx];
                coh_count += 1;
            }
        }
        let voice_coh = if coh_count > 0 {
            coh_sum / coh_count as f32
        } else {
            0.0
        };

        let prob = c_score * self.dialogue.cached_dialogue_w_c
            + v_score * self.dialogue.cached_dialogue_w_v
            + voice_coh * self.dialogue.cached_dialogue_w_coh;
        let p_alpha = if prob > self.dialogue.dialogue_probability {
            0.1
        } else {
            0.05
        };
        self.dialogue.dialogue_probability += p_alpha * (prob - self.dialogue.dialogue_probability);

        self.dialogue.dialogue_probability
    }
}
