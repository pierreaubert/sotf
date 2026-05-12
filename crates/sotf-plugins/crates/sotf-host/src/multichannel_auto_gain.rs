// ============================================================================
// MultichannelAutoGain
// ============================================================================
//
// Wraps a stereo `AutoGain` with the fold-down meter buffer used by plugins
// that emit multichannel output from stereo input (Upmixer, AAE, etc.).
//
// The folded meter buffer is a stereo (L, R) sum of the multichannel output:
// each non-LFE speaker contributes to L when azimuth > +10°, to R when
// azimuth < -10°, and is split with -3 dB to both when |azimuth| <= 10°
// (front/back center). The stereo gain produced by the inner `AutoGain` is
// then applied uniformly to every output channel.
//
// When the output has 2 channels, the meter buffer is the output itself.
// For mono (1 channel), the single channel is duplicated to both meter
// channels. For 0 channels (degenerate), the call is a no-op.

use crate::auto_gain::{AutoGain, AutoGainData, AutoGainParams};
use crate::speaker_config::SpeakerConfig;

/// Stereo `AutoGain` with multichannel output support via stereo fold-down.
pub struct MultichannelAutoGain {
    inner: AutoGain,
    meter_buf: Vec<f32>,
}

impl MultichannelAutoGain {
    /// Create with given sample rate and parameters. The inner `AutoGain` is
    /// always 2-channel (stereo) — we fold multichannel output down to stereo
    /// for measurement.
    pub fn new(sample_rate: u32, params: AutoGainParams) -> Result<Self, String> {
        Ok(Self {
            inner: AutoGain::new(2, sample_rate, params)?,
            meter_buf: Vec::new(),
        })
    }

    pub fn set_sample_rate(&mut self, sr: u32) -> Result<(), String> {
        self.inner.set_sample_rate(sr)
    }

    pub fn reset(&mut self) {
        self.inner.reset();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.inner.set_enabled(enabled);
    }
    pub fn is_enabled(&self) -> bool {
        self.inner.is_enabled()
    }
    pub fn set_max_gain_db(&mut self, max_db: f32) {
        self.inner.set_max_gain_db(max_db);
    }
    pub fn set_smoothing_ms(&mut self, ms: f32) {
        self.inner.set_smoothing_ms(ms);
    }

    /// Measure the stereo input loudness. `input` is interleaved 2-ch.
    pub fn measure_input(&mut self, input: &[f32]) -> Result<(), String> {
        self.inner.measure_input(input)
    }

    /// Fold multichannel output to stereo, measure output loudness, and apply
    /// the resulting gain uniformly to all output channels.
    ///
    /// `output` is interleaved with `out_ch` channels per frame. `out_ch` may
    /// be less than `speaker_config.total_channels` (e.g. upmixer's binaural
    /// preview emits 2-ch even when configured for 5.1+); only speakers with
    /// `sp.channel < out_ch` contribute to the meter.
    pub fn measure_and_apply(
        &mut self,
        output: &mut [f32],
        num_frames: usize,
        out_ch: usize,
        speaker_config: &SpeakerConfig,
    ) -> Result<(), String> {
        if !self.inner.is_enabled() || num_frames == 0 || out_ch == 0 {
            return Ok(());
        }
        debug_assert_eq!(output.len(), num_frames * out_ch);

        self.fill_meter_buffer(output, num_frames, out_ch, speaker_config);
        self.inner.measure_output(&self.meter_buf)?;

        for frame in 0..num_frames {
            let gain = self.inner.next_gain_linear();
            let base = frame * out_ch;
            for sample in &mut output[base..base + out_ch] {
                *sample *= gain;
            }
        }
        Ok(())
    }

    /// Snapshot of the current AutoGain state (for `get_data()` UI exposure).
    pub fn data(&self) -> AutoGainData {
        self.inner.get_data()
    }

    fn fill_meter_buffer(
        &mut self,
        output: &[f32],
        num_frames: usize,
        out_ch: usize,
        speaker_config: &SpeakerConfig,
    ) {
        self.meter_buf.resize(num_frames * 2, 0.0);
        self.meter_buf.fill(0.0);

        // Stereo or mono passthrough: copy directly.
        if out_ch <= 2 {
            for frame in 0..num_frames {
                let out_base = frame * out_ch;
                let meter_base = frame * 2;
                self.meter_buf[meter_base] = output[out_base];
                self.meter_buf[meter_base + 1] = if out_ch == 2 {
                    output[out_base + 1]
                } else {
                    output[out_base]
                };
            }
            return;
        }

        for frame in 0..num_frames {
            let out_base = frame * out_ch;
            let meter_base = frame * 2;
            for sp in speaker_config.speakers {
                if sp.is_lfe || sp.channel >= out_ch {
                    continue;
                }
                let sample = output[out_base + sp.channel];
                if sp.azimuth > 10.0 {
                    self.meter_buf[meter_base] += sample;
                } else if sp.azimuth < -10.0 {
                    self.meter_buf[meter_base + 1] += sample;
                } else {
                    let split = sample * std::f32::consts::FRAC_1_SQRT_2;
                    self.meter_buf[meter_base] += split;
                    self.meter_buf[meter_base + 1] += split;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auto_gain::AutoGainLoudnessType;
    use crate::speaker_config::get_speaker_config;

    fn enabled_params() -> AutoGainParams {
        AutoGainParams {
            enabled: true,
            loudness_type: AutoGainLoudnessType::Momentary,
            max_gain_db: 12.0,
            smoothing_ms: 50.0,
        }
    }

    #[test]
    fn disabled_is_noop() {
        let mut mag = MultichannelAutoGain::new(48000, AutoGainParams::default()).unwrap();
        let cfg = get_speaker_config("5.1").unwrap();
        let mut output = vec![0.5_f32; 1024 * cfg.total_channels];
        let snapshot = output.clone();
        mag.measure_and_apply(&mut output, 1024, cfg.total_channels, cfg)
            .unwrap();
        assert_eq!(
            output, snapshot,
            "disabled MultichannelAutoGain must not modify output"
        );
    }

    #[test]
    fn passes_through_stereo() {
        let mut mag = MultichannelAutoGain::new(48000, enabled_params()).unwrap();
        let cfg = get_speaker_config("2.0").unwrap();
        let frames = 1024;
        let input: Vec<f32> = (0..frames * 2)
            .map(|i| (i as f32 * 0.01).sin() * 0.5)
            .collect();
        let mut output = input.clone();
        mag.measure_input(&input).unwrap();
        mag.measure_and_apply(&mut output, frames, cfg.total_channels, cfg)
            .unwrap();
        assert!(output.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn folds_5_1_output_for_metering() {
        let mut mag = MultichannelAutoGain::new(48000, enabled_params()).unwrap();
        let cfg = get_speaker_config("5.1").unwrap();
        let frames = 4096;
        // Build a 5.1 buffer with energy on FL/FR only, varying over time.
        let mut output = vec![0.0_f32; frames * cfg.total_channels];
        for frame in 0..frames {
            let s = (frame as f32 * 0.01).sin() * 0.4;
            // Find FL (azimuth +30) and FR (azimuth -30) channels.
            for sp in cfg.speakers {
                if sp.is_lfe {
                    continue;
                }
                if (sp.azimuth - 30.0).abs() < 1.0 {
                    output[frame * cfg.total_channels + sp.channel] = s;
                } else if (sp.azimuth + 30.0).abs() < 1.0 {
                    output[frame * cfg.total_channels + sp.channel] = -s;
                }
            }
        }
        // Stereo input that matches FL/FR content so AutoGain converges to ~0 dB.
        let input: Vec<f32> = (0..frames * 2)
            .flat_map(|f| {
                let s = (f as f32 * 0.01).sin() * 0.4;
                [s, -s]
            })
            .collect();
        mag.measure_input(&input).unwrap();
        mag.measure_and_apply(&mut output, frames, cfg.total_channels, cfg)
            .unwrap();
        assert!(output.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn out_ch_zero_does_not_panic() {
        // Defensive check: out_ch == 0 is unreachable from current callers,
        // but the helper is a public sotf-host API. Must not panic.
        let mut mag = MultichannelAutoGain::new(48000, enabled_params()).unwrap();
        let cfg = get_speaker_config("5.1").unwrap();
        let mut output: Vec<f32> = Vec::new();
        let res = mag.measure_and_apply(&mut output, 8, 0, cfg);
        assert!(
            res.is_ok(),
            "out_ch == 0 should be a graceful no-op, got {:?}",
            res
        );
    }

    #[test]
    fn out_ch_one_mono_passthrough() {
        // out_ch == 1: the single channel should map to both L and R of meter.
        let mut mag = MultichannelAutoGain::new(48000, enabled_params()).unwrap();
        let cfg = get_speaker_config("1.0").unwrap();
        let frames = 1024;
        let input: Vec<f32> = (0..frames * 2)
            .map(|i| (i as f32 * 0.01).sin() * 0.5)
            .collect();
        let mut output: Vec<f32> = (0..frames).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();
        mag.measure_input(&input).unwrap();
        let res = mag.measure_and_apply(&mut output, frames, 1, cfg);
        assert!(res.is_ok());
        assert!(output.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn binaural_preview_uses_actual_out_ch() {
        // 5.1 speaker_config but out_ch=2 (upmixer binaural_preview): the
        // helper must treat it as stereo passthrough, ignoring channels 2-5.
        let mut mag = MultichannelAutoGain::new(48000, enabled_params()).unwrap();
        let cfg = get_speaker_config("5.1").unwrap();
        let frames = 1024;
        let input: Vec<f32> = (0..frames * 2)
            .map(|i| (i as f32 * 0.01).sin() * 0.5)
            .collect();
        let mut output = input.clone();
        mag.measure_input(&input).unwrap();
        mag.measure_and_apply(&mut output, frames, 2, cfg).unwrap();
        assert!(output.iter().all(|v| v.is_finite()));
    }
}
