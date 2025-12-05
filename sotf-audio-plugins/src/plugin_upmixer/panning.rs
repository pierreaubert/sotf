// ============================================================================
// VBAP Panning and Inverse FFT
// ============================================================================

use super::UpmixerPlugin;

impl UpmixerPlugin {
    /// Phase 3: Apply VBAP panning to distribute to output speakers and perform inverse FFT
    #[inline]
    pub(super) fn apply_vbap_panning_and_inverse_fft(&mut self) {
        let spectrum_size = self.fft_size / 2 + 1;
        let hr_mix_global = (self.hr_transient_env * self.hr_sharpen).clamp(0.0, 1.0);

        for ch_idx in 0..self.num_output_channels {
            let speaker = &self.speaker_config.speakers[ch_idx];

            if speaker.is_lfe {
                // LFE channel
                for i in 0..spectrum_size {
                    self.temp_freq_out[i] = self.lfe[i] * self.lfe_gain;
                }
            } else {
                // Regular speaker
                let panning_gain_left = self.panning_gains_left[ch_idx];
                let panning_gain_right = self.panning_gains_right[ch_idx];

                let is_front = speaker.azimuth.abs() < 80.0;
                let is_height = speaker.elevation > 10.0;
                let is_center = speaker.label == "C";

                // Front speakers use explicit front direct/ambient gains.
                let mut direct_gain = if is_front && !is_height {
                    self.gain_front_direct
                } else {
                    // Allow 20% direct bleed into surrounds for cohesion
                    // But respect zero gains for silence
                    if self.gain_front_direct == 0.0 && self.gain_rear_ambient == 0.0 {
                        0.0
                    } else {
                        0.20
                    }
                };

                let mut ambient_gain = if is_front && !is_height {
                    self.gain_front_ambient
                } else {
                    self.gain_rear_ambient
                };

                if is_front && !is_height && hr_mix_global > 0.0 {
                    let duck_direct = 0.25 * hr_mix_global;
                    let duck_ambient = 0.5 * hr_mix_global;

                    let min_scale = if self.safety_cap_db > 0.0 {
                        10.0_f32.powf(-self.safety_cap_db / 20.0)
                    } else {
                        0.0
                    };

                    let direct_scale = (1.0 - duck_direct).max(min_scale);
                    let ambient_scale = (1.0 - duck_ambient).max(min_scale);

                    direct_gain *= direct_scale;
                    ambient_gain *= ambient_scale;
                }

                if is_height {
                    // Allow 15% direct bleed into heights for "air"
                    let height_direct_leak = 0.15;

                    for i in 0..spectrum_size {
                        // 1. Direct component
                        let direct_component = (self.direct_left[i] * panning_gain_left
                            + self.direct_right[i] * panning_gain_right)
                            * height_direct_leak;

                        // 2. Ambient component (decorrelated)
                        // Use the static Velvet Noise filters (decorrelation_filter_left/right)
                        // instead of phase-shifted copies of ambient_left/right
                        let decor_l = self.decorrelation_filter_left[i];
                        let decor_r = self.decorrelation_filter_right[i];

                        let ambient_raw_l = self.ambient_left[i];
                        let ambient_raw_r = self.ambient_right[i];

                        // Apply decorrelation to ambient
                        let ambient_decor_l = ambient_raw_l * decor_l;
                        let ambient_decor_r = ambient_raw_r * decor_r;

                        let is_left = speaker.azimuth > 0.0;
                        let mut ambient_component = if is_left {
                            ambient_decor_l * panning_gain_left
                                + ambient_decor_r * panning_gain_right
                        } else {
                            ambient_decor_r * panning_gain_left
                                + ambient_decor_l * panning_gain_right
                        };

                        // For rear height channels, add late reflections (10% of direct signal)
                        // This ensures rear heights have energy even with mono/coherent content
                        if !is_front {
                            let late_reflection =
                                (self.direct_left[i] + self.direct_right[i]) * 0.10;
                            ambient_component += late_reflection;
                        }

                        // Height mask
                        let height_mask = self.height_band_gains[i];

                        self.temp_freq_out[i] = (direct_component * direct_gain
                            + ambient_component * ambient_gain)
                            * self.height_gain
                            * height_mask;
                    }
                } else {
                    for i in 0..spectrum_size {
                        let mut direct_component = self.direct_left[i] * panning_gain_left
                            + self.direct_right[i] * panning_gain_right;

                        // Apply decorrelation for surround channels (non-front)
                        let ambient_component = if !is_front {
                            let decor_l = self.decorrelation_filter_left[i];
                            let decor_r = self.decorrelation_filter_right[i];

                            let amb_l = self.ambient_left[i] * decor_l;
                            let amb_r = self.ambient_right[i] * decor_r;

                            amb_l * panning_gain_left + amb_r * panning_gain_right
                        } else {
                            self.ambient_left[i] * panning_gain_left
                                + self.ambient_right[i] * panning_gain_right
                        };

                        if is_front && !is_height && is_center {
                            let spread = self.center_spread.clamp(0.0, 1.0);
                            direct_component *= 1.0 - spread;
                        }
                        self.temp_freq_out[i] =
                            direct_component * direct_gain + ambient_component * ambient_gain;
                    }
                }
            }

            // Enforce RealFFT constraints: DC and Nyquist bins must be purely real
            if spectrum_size > 0 {
                self.temp_freq_out[0].im = 0.0;
                self.temp_freq_out[spectrum_size - 1].im = 0.0;
            }

            // Inverse FFT (Complex -> Real)
            self.fft_inverse
                .process(&mut self.temp_freq_out, &mut self.time_out_channels[ch_idx])
                .unwrap();
        }
    }
}
