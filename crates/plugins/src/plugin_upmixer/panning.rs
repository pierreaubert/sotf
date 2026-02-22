// ============================================================================
// VBAP Panning and Inverse FFT
// ============================================================================

use super::UpmixerPlugin;

impl UpmixerPlugin {
    #[inline]
    pub(super) fn apply_vbap_panning_and_inverse_fft(&mut self) {
        let spectrum_size = self.fft_size / 2 + 1;
        let hr_mix = (self.hr_transient_env * self.hr_sharpen.current() * self.hr_direct_envelope).clamp(0.0, 1.0);

        let gfd = self.gain_front_direct.current();
        let gfa = self.gain_front_ambient.current();
        let gra = self.gain_rear_ambient.current();
        let lfg = self.lfe_gain.current();
        let hg = self.height_gain.current();

        for ch in 0..self.num_output_channels {
            let spk = &self.speaker_config.speakers[ch];
            if spk.is_lfe {
                for i in 0..spectrum_size {
                    self.temp_freq_out[i] = self.lfe[i] * lfg;
                }
            } else {
                let p_l = self.panning_gains_left[ch];
                let p_r = self.panning_gains_right[ch];
                let is_f = self.cached_is_front[ch];
                let is_h = self.cached_is_height[ch];
                let is_c = self.cached_is_center[ch];

                let mut dg = if is_f && !is_h {
                    gfd
                } else {
                    if gfd == 0.0 && gra == 0.0 {
                        0.0
                    } else {
                        // Reduce bleed for coherent signals to prevent voice leakage
                        let bleed_scale = (1.0 - self.dialogue_probability * 0.8).clamp(0.1, 1.0);
                        self.surround_direct_bleed.current() * bleed_scale
                    }
                };
                let mut ag = if is_f && !is_h {
                    gfa
                } else {
                    gra * self.rear_ambient_boost.current()
                };

                if is_f && !is_h && hr_mix > 0.0 {
                    dg *= (1.0 - 0.25 * hr_mix).max(self.safety_cap_min_scale);
                    ag *= (1.0 - 0.5 * hr_mix).max(self.safety_cap_min_scale);
                }

                let pld = p_l * dg;
                let prd = p_r * dg;
                let pla = p_l * ag;
                let pra = p_r * ag;

                if is_h {
                    // Pre-compute scalar products outside inner loop
                    // Reduce height direct leak for coherent signals
                    let h_leak_scale = (1.0 - self.dialogue_probability * 0.9).clamp(0.05, 1.0);
                    let pld_dl = pld * self.height_direct_leak.current() * h_leak_scale;
                    let prd_dl = prd * self.height_direct_leak.current() * h_leak_scale;
                    let dec = &self.blended_decorrelation_filters[ch];

                    if !is_f {
                        let rlr = self.rear_late_reflection.current() * gra; // Scale by rear gain
                        // For non-front height: use dominant ambient side to preserve separation
                        let (a_primary, a_secondary, g_primary, g_secondary) = if spk.azimuth >= 0.0 {
                            (&self.ambient_left, &self.ambient_right, pla, pra)
                        } else {
                            (&self.ambient_right, &self.ambient_left, pra, pla)
                        };

                        for i in 0..spectrum_size {
                            let d_comp =
                                self.direct_left[i] * pld_dl + self.direct_right[i] * prd_dl;
                            // Stereo-preserving ambient sum: mix primary and secondary sides
                            let a_stereo = a_primary[i] * g_primary + a_secondary[i] * (g_secondary * 0.3);
                            let a_comp = a_stereo * dec[i]
                                + (self.direct_left[i] + self.direct_right[i]) * rlr;
                            self.temp_freq_out[i] =
                                (d_comp + a_comp) * (hg * self.height_band_gains[i]);
                        }
                    } else {
                        // For front height: use standard L/R mapping
                        let (a_primary, a_secondary, g_primary, g_secondary) = if spk.azimuth >= 0.0 {
                            (&self.ambient_left, &self.ambient_right, pla, pra)
                        } else {
                            (&self.ambient_right, &self.ambient_left, pra, pla)
                        };

                        for i in 0..spectrum_size {
                            let d_comp =
                                self.direct_left[i] * pld_dl + self.direct_right[i] * prd_dl;
                            let a_stereo = a_primary[i] * g_primary + a_secondary[i] * (g_secondary * 0.3);
                            let a_comp = a_stereo * dec[i];
                            self.temp_freq_out[i] =
                                (d_comp + a_comp) * (hg * self.height_band_gains[i]);
                        }
                    }
                } else {
                    let ss = if is_f && is_c {
                        1.0 - self.center_spread.current()
                    } else {
                        1.0
                    };
                    let plds = p_l * ss;
                    let prds = p_r * ss;

                    // Extra gain for the extracted center signal in the center speaker
                    // MUST be scaled by gain_front_direct (dg for is_f speakers)
                    let p_direct = if is_f && is_c { 1.414 * gfd } else { 0.0 };

                    if !is_f {
                        let dec = &self.blended_decorrelation_filters[ch];
                        // Preserve stereo separation in surround ambient
                        let (a_primary, a_secondary, g_primary, g_secondary) = if spk.azimuth >= 0.0 {
                            (&self.ambient_left, &self.ambient_right, pla, pra)
                        } else {
                            (&self.ambient_right, &self.ambient_left, pra, pla)
                        };

                        for i in 0..spectrum_size {
                            let a_stereo = a_primary[i] * g_primary + a_secondary[i] * (g_secondary * 0.3);
                            // Surrounds get direct residues + decorrelated ambient
                            self.temp_freq_out[i] = (self.direct_left[i] * plds
                                + self.direct_right[i] * prds)
                                + a_stereo * dec[i];
                        }
                    } else {
                        // Front speakers: use standard L/R mix + extracted center
                        for i in 0..spectrum_size {
                            self.temp_freq_out[i] = (self.direct_left[i] * plds
                                + self.direct_right[i] * prds)
                                + (self.ambient_left[i] * pla + self.ambient_right[i] * pra)
                                + (self.direct[i] * p_direct);
                        }
                    }
                }
            }
            if spectrum_size > 0 {
                self.temp_freq_out[0].im = 0.0;
                self.temp_freq_out[spectrum_size - 1].im = 0.0;
            }
            self.fft_inverse
                .process(&mut self.temp_freq_out, &mut self.time_out_channels[ch])
                .unwrap();
        }
    }
}
