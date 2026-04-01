// ============================================================================
// VBAP Panning and Inverse FFT
// ============================================================================

use super::UpmixerPlugin;
use math_audio_dsp::fast_math::fast_cos;

impl UpmixerPlugin {
    #[inline]
    pub(super) fn apply_vbap_panning_and_inverse_fft(&mut self) {
        let spectrum_size = self.fft_size / 2 + 1;
        let hr_mix = (self.hr_transient_env * self.hr_sharpen.current() * self.hr_direct_envelope)
            .clamp(0.0, 1.0);

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
                } else if gfd == 0.0 && gra == 0.0 {
                    0.0
                } else {
                    // Reduce bleed for coherent signals to prevent voice leakage
                    let bleed_scale = (1.0 - self.dialogue_probability * 0.8).clamp(0.1, 1.0);
                    self.surround_direct_bleed.current() * bleed_scale
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

                    let sw = self.stereo_width.current();
                    let cs = self.center_spread.current();
                    let sw_cs = sw * cs;

                    if !is_f {
                        let rlr = self.rear_late_reflection.current() * gra; // Scale by rear gain
                        // For non-front height: use dominant ambient side to preserve separation
                        let (a_primary, a_secondary, g_primary, g_secondary) = if spk.azimuth >= 0.0
                        {
                            (&self.ambient_left, &self.ambient_right, pla, pra)
                        } else {
                            (&self.ambient_right, &self.ambient_left, pra, pla)
                        };

                        for i in 0..spectrum_size {
                            let d_val = self.direct[i];
                            let dl_val = self.direct_left[i] + d_val * sw_cs;
                            let dr_val = self.direct_right[i] + d_val * sw_cs;

                            let d_comp = dl_val * pld_dl + dr_val * prd_dl;
                            // Stereo-preserving ambient sum: mix primary and secondary sides
                            let a_stereo =
                                a_primary[i] * g_primary + a_secondary[i] * (g_secondary * 0.3);
                            let a_comp = a_stereo * dec[i]
                                + (self.direct_left[i] + self.direct_right[i]) * rlr;
                            self.temp_freq_out[i] =
                                (d_comp + a_comp) * (hg * self.height_band_gains[i]);
                        }
                    } else {
                        // For front height: use standard L/R mapping
                        let (a_primary, a_secondary, g_primary, g_secondary) = if spk.azimuth >= 0.0
                        {
                            (&self.ambient_left, &self.ambient_right, pla, pra)
                        } else {
                            (&self.ambient_right, &self.ambient_left, pra, pla)
                        };

                        for i in 0..spectrum_size {
                            let d_val = self.direct[i];
                            let dl_val = self.direct_left[i] + d_val * sw_cs;
                            let dr_val = self.direct_right[i] + d_val * sw_cs;

                            let d_comp = dl_val * pld_dl + dr_val * prd_dl;
                            let a_stereo =
                                a_primary[i] * g_primary + a_secondary[i] * (g_secondary * 0.3);
                            let a_comp = a_stereo * dec[i];
                            self.temp_freq_out[i] =
                                (d_comp + a_comp) * (hg * self.height_band_gains[i]);
                        }
                    }
                } else {
                    let sw = self.stereo_width.current();
                    let cs = self.center_spread.current();
                    let sw_cs = sw * cs;

                    // Correctly power-balanced gain for the extracted center signal in the physical center speaker.
                    // If stereo_w=1.0, we need sqrt(2) to match the power of the original L+R phantom center.
                    // If stereo_w=0.5, we only need sqrt(1.5)=1.225 because 0.5 of the energy is still in L/R.
                    //
                    // Edge case: when stereo_width=0, center_power_scale evaluates to sqrt(0)=0.
                    // This is intentional — width=0 means "keep all content in L/R with no center
                    // extraction", so the physical center speaker should receive zero direct signal.
                    // The audio is not lost; it remains in direct_left/direct_right via the L/R path.
                    let center_power_scale = (2.0 * (1.0 - (1.0 - sw).powi(2))).max(0.0).sqrt();
                    let p_direct_c = if is_f && is_c {
                        center_power_scale * (1.0 - cs) * dg
                    } else {
                        0.0
                    };

                    // Multiply direct_left/right by dg (which is gfd for front)
                    let plds = p_l * dg;
                    let prds = p_r * dg;

                    if !is_f {
                        let dec = &self.blended_decorrelation_filters[ch];
                        // Preserve stereo separation in surround ambient
                        let (a_primary, a_secondary, g_primary, g_secondary) = if spk.azimuth >= 0.0
                        {
                            (&self.ambient_left, &self.ambient_right, pla, pra)
                        } else {
                            (&self.ambient_right, &self.ambient_left, pra, pla)
                        };

                        // Pre-compute speaker azimuth in radians for DOA-based direct2 routing.
                        // Gain = cos(doa2 - spk_az), clamped to [0, 1]: maximum when DOA aligns
                        // with the speaker, zero when 90° away. This provides soft-knee VBAP for
                        // the secondary source without needing a full VBAP triplet solve.
                        let spk_az_rad = spk.azimuth * std::f32::consts::PI / 180.0;
                        let multi_source = self.multi_source_extraction;

                        for i in 0..spectrum_size {
                            let d_val = self.direct[i];
                            let dl_val = self.direct_left[i] + d_val * sw_cs;
                            let dr_val = self.direct_right[i] + d_val * sw_cs;

                            let a_stereo =
                                a_primary[i] * g_primary + a_secondary[i] * (g_secondary * 0.3);
                            // Surrounds get direct residues + decorrelated ambient
                            let mut out = (dl_val * plds + dr_val * prds) + a_stereo * dec[i];

                            // Add secondary source contribution steered by DOA.
                            // Gain = max(0, cos(doa2 - spk_az)) provides soft directional routing.
                            if multi_source {
                                let doa2 = self.direct2_doa_per_bin[i];
                                let d2_gain = fast_cos(doa2 - spk_az_rad).max(0.0);
                                out += self.direct2[i] * (d2_gain * dg);
                            }

                            self.temp_freq_out[i] = out;
                        }
                    } else {
                        // Front speakers: use standard L/R mix + extracted center
                        for i in 0..spectrum_size {
                            let d_val = self.direct[i];
                            let dl_val = self.direct_left[i] + d_val * sw_cs;
                            let dr_val = self.direct_right[i] + d_val * sw_cs;

                            self.temp_freq_out[i] = (dl_val * plds + dr_val * prds)
                                + (self.ambient_left[i] * pla + self.ambient_right[i] * pra)
                                + (d_val * p_direct_c);
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
