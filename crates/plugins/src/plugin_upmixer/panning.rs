// ============================================================================
// VBAP Panning and Inverse FFT
// ============================================================================

use super::UpmixerPlugin;
use crate::simd::flush_denormals_inplace;

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
                let lfe_gain = self.lfe_gain.current();
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

                let gain_front_direct = self.gain_front_direct.current();
                let gain_front_ambient = self.gain_front_ambient.current();
                let gain_rear_ambient = self.gain_rear_ambient.current();

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
                    // Configurable direct bleed into heights for "air"
                    let height_direct_leak = self.height_direct_leak;

                    for i in 0..spectrum_size {
                        // 1. Direct component
                        let direct_component = (self.direct_left[i] * panning_gain_left
                            + self.direct_right[i] * panning_gain_right)
                            * height_direct_leak;

                        // 2. Ambient component (decorrelated)
                        // 3C: Use per-channel decorrelation filter if available,
                        // otherwise fall back to left/right pair
                        let decor_filter = if ch_idx < self.decorrelation_filters.len()
                            && i < self.decorrelation_filters[ch_idx].len()
                        {
                            self.decorrelation_filters[ch_idx][i]
                        } else {
                            // Fallback to legacy left/right filters
                            if speaker.azimuth > 0.0 {
                                self.decorrelation_filter_left[i]
                            } else {
                                self.decorrelation_filter_right[i]
                            }
                        };

                        let ambient_raw_l = self.ambient_left[i];
                        let ambient_raw_r = self.ambient_right[i];

                        // Apply per-channel decorrelation to ambient
                        let ambient_decor_l = ambient_raw_l * decor_filter;
                        let ambient_decor_r = ambient_raw_r * decor_filter;

                        let is_left = speaker.azimuth > 0.0;
                        let mut ambient_component = if is_left {
                            ambient_decor_l * panning_gain_left
                                + ambient_decor_r * panning_gain_right
                        } else {
                            ambient_decor_r * panning_gain_left
                                + ambient_decor_l * panning_gain_right
                        };

                        // For rear height channels, add late reflections (configurable)
                        // This ensures rear heights have energy even with mono/coherent content
                        if !is_front {
                            let late_reflection = (self.direct_left[i] + self.direct_right[i])
                                * self.rear_late_reflection;
                            ambient_component += late_reflection;
                        }

                        // Height mask with bounds check
                        let height_mask = self.height_band_gains.get(i).copied().unwrap_or(0.0);

                        self.temp_freq_out[i] = (direct_component * direct_gain
                            + ambient_component * ambient_gain)
                            * self.height_gain.current()
                            * height_mask;
                    }
                } else {
                    for i in 0..spectrum_size {
                        let mut direct_component = self.direct_left[i] * panning_gain_left
                            + self.direct_right[i] * panning_gain_right;

                        // Apply decorrelation for surround channels (non-front)
                        // 3C: Use per-channel decorrelation filter
                        let ambient_component = if !is_front {
                            let decor = if ch_idx < self.decorrelation_filters.len()
                                && i < self.decorrelation_filters[ch_idx].len()
                            {
                                self.decorrelation_filters[ch_idx][i]
                            } else if speaker.azimuth > 0.0 {
                                self.decorrelation_filter_left[i]
                            } else {
                                self.decorrelation_filter_right[i]
                            };

                            let amb_l = self.ambient_left[i] * decor;
                            let amb_r = self.ambient_right[i] * decor;

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

            // Guard against NaN/Inf in output (defensive)
            for sample in &mut self.time_out_channels[ch_idx] {
                if !sample.is_finite() {
                    *sample = 0.0;
                }
            }

            // Flush denormals to prevent CPU performance spikes and audio crackle
            flush_denormals_inplace(&mut self.time_out_channels[ch_idx]);
        }
    }
}
