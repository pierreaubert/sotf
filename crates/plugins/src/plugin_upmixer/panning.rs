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

        // Pre-calculate global gains to avoid smoothing overhead inside channel loop
        let gain_front_direct = self.gain_front_direct.current();
        let gain_front_ambient = self.gain_front_ambient.current();
        let gain_rear_ambient = self.gain_rear_ambient.current();
        let lfe_gain = self.lfe_gain.current();
        let height_gain_val = self.height_gain.current();

        for ch_idx in 0..self.num_output_channels {
            let speaker = &self.speaker_config.speakers[ch_idx];

            if speaker.is_lfe {
                // LFE channel
                for i in 0..spectrum_size {
                    self.temp_freq_out[i] = self.lfe[i] * lfe_gain;
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
                    gain_front_direct
                } else {
                    // Configurable direct bleed into surrounds/heights for cohesion
                    if gain_front_direct == 0.0 && gain_rear_ambient == 0.0 {
                        0.0
                    } else {
                        self.surround_direct_bleed
                    }
                };

                let mut ambient_gain = if is_front && !is_height {
                    gain_front_ambient
                } else {
                    // Configurable ambient gain boost for rears
                    gain_rear_ambient * self.rear_ambient_boost
                };

                if is_front && !is_height && hr_mix_global > 0.0 {
                    let duck_direct = 0.25 * hr_mix_global;
                    let duck_ambient = 0.5 * hr_mix_global;
                    let min_scale = self.safety_cap_min_scale;
                    direct_gain *= (1.0 - duck_direct).max(min_scale);
                    ambient_gain *= (1.0 - duck_ambient).max(min_scale);
                }

                // Optimization: Incorporate gains into panning gains where possible
                let p_l_direct = panning_gain_left * direct_gain;
                let p_r_direct = panning_gain_right * direct_gain;
                let p_l_ambient = panning_gain_left * ambient_gain;
                let p_r_ambient = panning_gain_right * ambient_gain;

                if is_height {
                    let direct_leak = self.height_direct_leak;
                    let h_scale = height_gain_val;
                    let blended_decor = &self.blended_decorrelation_filters[ch_idx];
                    
                    for i in 0..spectrum_size {
                        let direct_component = (self.direct_left[i] * p_l_direct
                            + self.direct_right[i] * p_r_direct) * direct_leak;

                        // Optimization: Combine ambient signals before complex multiplication
                        let is_left = speaker.azimuth > 0.0;
                        let amb_mono = if is_left {
                            self.ambient_left[i] * p_l_ambient + self.ambient_right[i] * p_r_ambient
                        } else {
                            self.ambient_right[i] * p_l_ambient + self.ambient_left[i] * p_r_ambient
                        };

                        let mut ambient_component = amb_mono * blended_decor[i];

                        if !is_front {
                            ambient_component += (self.direct_left[i] + self.direct_right[i])
                                * self.rear_late_reflection;
                        }

                        let height_mask = self.height_band_gains.get(i).copied().unwrap_or(0.0);
                        self.temp_freq_out[i] = (direct_component + ambient_component) * (h_scale * height_mask);
                    }
                } else {
                    let spread_scale = if is_front && is_center {
                        1.0 - self.center_spread.clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                    let p_l_dir_scaled = p_l_direct * spread_scale;
                    let p_r_dir_scaled = p_r_direct * spread_scale;

                    if !is_front {
                        let blended_decor = &self.blended_decorrelation_filters[ch_idx];

                        // Surround channels: apply decorrelation
                        for i in 0..spectrum_size {
                            let amb_mono = self.ambient_left[i] * p_l_ambient + self.ambient_right[i] * p_r_ambient;
                            let ambient_component = amb_mono * blended_decor[i];

                            self.temp_freq_out[i] = (self.direct_left[i] * p_l_dir_scaled + self.direct_right[i] * p_r_dir_scaled)
                                + ambient_component;
                        }
                    } else {
                        // Front channels: no decorrelation
                        for i in 0..spectrum_size {
                            self.temp_freq_out[i] = (self.direct_left[i] * p_l_dir_scaled + self.direct_right[i] * p_r_dir_scaled)
                                + (self.ambient_left[i] * p_l_ambient + self.ambient_right[i] * p_r_ambient);
                        }
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
