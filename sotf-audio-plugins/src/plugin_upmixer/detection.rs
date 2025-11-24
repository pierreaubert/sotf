// ============================================================================
// Dialogue Detection
// ============================================================================

use super::UpmixerPlugin;

impl UpmixerPlugin {
    /// Detect dialogue-like signals using spectral centroid and temporal envelope
    ///
    /// Dialogue characteristics:
    /// - Spectral centroid in 500-3000 Hz range (fundamental voice frequencies)
    /// - Low temporal envelope variance (relatively steady compared to music)
    /// - High coherence (mono/center content)
    ///
    /// Returns dialogue probability (0.0 to 1.0)
    #[inline]
    pub(super) fn detect_dialogue(&mut self) -> f32 {
        let spectrum_size = self.fft_size / 2 + 1;
        let freq_per_bin = self.sample_rate as f32 / self.fft_size as f32;

        // Voice frequency range: 500-3000 Hz (covers fundamental + formants)
        let voice_start_hz = 500.0;
        let voice_end_hz = 3000.0;
        let voice_start_bin = (voice_start_hz / freq_per_bin) as usize;
        let voice_end_bin = (voice_end_hz / freq_per_bin).min(spectrum_size as f32 - 1.0) as usize;

        // Calculate spectral centroid in voice range
        let mut weighted_sum = 0.0_f32;
        let mut magnitude_sum = 0.0_f32;

        for i in voice_start_bin..=voice_end_bin {
            let left_mag = self.freq_domain_left[i].norm();
            let right_mag = self.freq_domain_right[i].norm();
            let avg_mag = (left_mag + right_mag) * 0.5;

            let freq = i as f32 * freq_per_bin;
            weighted_sum += freq * avg_mag;
            magnitude_sum += avg_mag;
        }

        let spectral_centroid = if magnitude_sum > 1e-9 {
            weighted_sum / magnitude_sum
        } else {
            0.0
        };

        // Smooth spectral centroid with exponential averaging
        let centroid_alpha = 0.3;
        self.dialogue_spectral_centroid = centroid_alpha * spectral_centroid
            + (1.0 - centroid_alpha) * self.dialogue_spectral_centroid;

        // Calculate RMS energy for temporal envelope variance
        let mut energy_sum = 0.0_f32;
        for i in voice_start_bin..=voice_end_bin {
            let left_mag = self.freq_domain_left[i].norm_sqr();
            let right_mag = self.freq_domain_right[i].norm_sqr();
            energy_sum += left_mag + right_mag;
        }
        let rms = (energy_sum / ((voice_end_bin - voice_start_bin + 1) as f32 * 2.0)).sqrt();

        // Calculate envelope variance (difference from previous frame)
        let envelope_diff = if self.dialogue_prev_rms > 1e-9 {
            ((rms - self.dialogue_prev_rms) / self.dialogue_prev_rms).abs()
        } else {
            1.0 // High variance if previous was silence
        };
        self.dialogue_prev_rms = rms;

        // Smooth envelope variance
        let variance_alpha = 0.2;
        self.dialogue_envelope_variance = variance_alpha * envelope_diff
            + (1.0 - variance_alpha) * self.dialogue_envelope_variance;

        // Dialogue probability calculation
        // Voice has centroid in 800-2000 Hz (sweet spot), low variance (<0.3)
        let centroid_voice_min = 800.0;
        let centroid_voice_max = 2000.0;
        let centroid_score = if self.dialogue_spectral_centroid >= centroid_voice_min
            && self.dialogue_spectral_centroid <= centroid_voice_max
        {
            1.0
        } else if self.dialogue_spectral_centroid < centroid_voice_min {
            // Below range: fade from 500 to 800 Hz
            ((self.dialogue_spectral_centroid - voice_start_hz)
                / (centroid_voice_min - voice_start_hz))
                .clamp(0.0, 1.0)
        } else {
            // Above range: fade from 2000 to 3000 Hz
            (1.0 - ((self.dialogue_spectral_centroid - centroid_voice_max)
                / (voice_end_hz - centroid_voice_max)))
                .clamp(0.0, 1.0)
        };

        // Low variance indicates steady dialogue (vs. dynamic music)
        let variance_threshold = 0.4;
        let variance_score =
            (1.0 - (self.dialogue_envelope_variance / variance_threshold).min(1.0)).max(0.0);

        // Combined score with weighting
        let dialogue_prob = centroid_score * 0.6 + variance_score * 0.4;

        // Smooth dialogue probability with slow attack/release
        let prob_alpha = if dialogue_prob > self.dialogue_probability {
            0.1 // Slow attack: don't immediately assume dialogue
        } else {
            0.05 // Very slow release: maintain dialogue routing once detected
        };
        self.dialogue_probability =
            prob_alpha * dialogue_prob + (1.0 - prob_alpha) * self.dialogue_probability;

        self.dialogue_probability
    }
}
