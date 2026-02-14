// ============================================================================
// VBAP Panning and Inverse FFT
// ============================================================================

use super::UpmixerPlugin;

impl UpmixerPlugin {
    #[inline]
    pub(super) fn apply_vbap_panning_and_inverse_fft(&mut self) {
        let spectrum_size = self.fft_size / 2 + 1;
        let hr_mix = (self.hr_transient_env * self.hr_sharpen).clamp(0.0, 1.0);

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
                        self.surround_direct_bleed
                    }
                };
                let mut ag = if is_f && !is_h {
                    gfa
                } else {
                    gra * self.rear_ambient_boost
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
                    let dl = self.height_direct_leak;
                    let dec = &self.blended_decorrelation_filters[ch];
                    for i in 0..spectrum_size {
                        let d_comp = (self.direct_left[i] * pld + self.direct_right[i] * prd) * dl;
                        let a_mono = if spk.azimuth > 0.0 {
                            self.ambient_left[i] * pla + self.ambient_right[i] * pra
                        } else {
                            self.ambient_right[i] * pla + self.ambient_left[i] * pra
                        };
                        let mut a_comp = a_mono * dec[i];
                        if !is_f {
                            a_comp += (self.direct_left[i] + self.direct_right[i])
                                * self.rear_late_reflection;
                        }
                        self.temp_freq_out[i] =
                            (d_comp + a_comp) * (hg * self.height_band_gains[i]);
                    }
                } else {
                    let ss = if is_f && is_c {
                        1.0 - self.center_spread.clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                    let plds = pld * ss;
                    let prds = prd * ss;
                    if !is_f {
                        let dec = &self.blended_decorrelation_filters[ch];
                        for i in 0..spectrum_size {
                            let a_mono = self.ambient_left[i] * pla + self.ambient_right[i] * pra;
                            self.temp_freq_out[i] = (self.direct_left[i] * plds
                                + self.direct_right[i] * prds)
                                + a_mono * dec[i];
                        }
                    } else {
                        for i in 0..spectrum_size {
                            self.temp_freq_out[i] = (self.direct_left[i] * plds
                                + self.direct_right[i] * prds)
                                + (self.ambient_left[i] * pla + self.ambient_right[i] * pra);
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
