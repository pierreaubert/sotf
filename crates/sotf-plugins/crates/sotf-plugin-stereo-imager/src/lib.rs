// ============================================================================
// Stereo Imager Plugin - Multi-band M/S stereo width control
// ============================================================================

pub mod params;

use crate::params::PARAMS as SI;
use sotf_host::lr4_crossover::Lr4Crossover;
use sotf_host::param_specs::{ParamType, find_by_key as pk};
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::{LogSmoother, Smoother};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StereoImagerPluginParams {
    #[serde(default = "default_width")]
    pub width: f32,
    #[serde(default = "default_low_mid_freq")]
    pub low_mid_freq: f32,
    #[serde(default = "default_mid_high_freq")]
    pub mid_high_freq: f32,
    #[serde(default = "default_low_width")]
    pub low_width: f32,
    #[serde(default = "default_mid_width")]
    pub mid_width: f32,
    #[serde(default = "default_high_width")]
    pub high_width: f32,
    #[serde(default)]
    pub mono_bass: bool,
    #[serde(default = "default_mix")]
    pub mix: f32,
}

fn default_width() -> f32 {
    pk(SI, "width").default_f64() as f32
}
fn default_low_mid_freq() -> f32 {
    pk(SI, "low_mid_freq").default_f64() as f32
}
fn default_mid_high_freq() -> f32 {
    pk(SI, "mid_high_freq").default_f64() as f32
}
fn default_low_width() -> f32 {
    pk(SI, "low_width").default_f64() as f32
}
fn default_mid_width() -> f32 {
    pk(SI, "mid_width").default_f64() as f32
}
fn default_high_width() -> f32 {
    pk(SI, "high_width").default_f64() as f32
}
fn default_mix() -> f32 {
    pk(SI, "mix").default_f64() as f32
}

impl Default for StereoImagerPluginParams {
    fn default() -> Self {
        Self {
            width: default_width(),
            low_mid_freq: default_low_mid_freq(),
            mid_high_freq: default_mid_high_freq(),
            low_width: default_low_width(),
            mid_width: default_mid_width(),
            high_width: default_high_width(),
            mono_bass: pk(SI, "mono_bass").default_bool(),
            mix: default_mix(),
        }
    }
}

pub struct StereoImagerPlugin {
    channels: usize,
    sample_rate: u32,

    // Parameters
    width: f32,
    low_mid_freq: f32,
    mid_high_freq: f32,
    low_width: f32,
    mid_width: f32,
    high_width: f32,
    mono_bass: bool,
    mix: f32,

    // Crossovers: each crossover handles 2 channels (one for mid signal, one for side signal)
    crossover_low: Lr4Crossover<f32>,
    crossover_high: Lr4Crossover<f32>,

    // Pre-allocated dry buffer for mix blending
    dry_buf: Vec<f32>,

    // Smoothers for click-free parameter changes
    width_smoother: Smoother,
    low_width_smoother: Smoother,
    mid_width_smoother: Smoother,
    high_width_smoother: Smoother,
    mix_smoother: Smoother,
    low_mid_freq_smoother: LogSmoother,
    mid_high_freq_smoother: LogSmoother,
}

const SMOOTHING_MS: f32 = 10.0;
const FREQ_SMOOTHING_MS: f32 = 50.0;

impl StereoImagerPlugin {
    pub fn new(channels: usize, params: StereoImagerPluginParams) -> Self {
        let sr = 48000;
        Self {
            channels,
            sample_rate: sr,

            width: params.width,
            low_mid_freq: params.low_mid_freq,
            mid_high_freq: params.mid_high_freq,
            low_width: params.low_width,
            mid_width: params.mid_width,
            high_width: params.high_width,
            mono_bass: params.mono_bass,
            mix: params.mix,

            // 2 channels: channel 0 = mid, channel 1 = side
            crossover_low: Lr4Crossover::new(params.low_mid_freq, sr as f32, 2),
            crossover_high: Lr4Crossover::new(params.mid_high_freq, sr as f32, 2),

            dry_buf: Vec::new(),

            width_smoother: Smoother::new(params.width, SMOOTHING_MS, sr),
            low_width_smoother: Smoother::new(params.low_width, SMOOTHING_MS, sr),
            mid_width_smoother: Smoother::new(params.mid_width, SMOOTHING_MS, sr),
            high_width_smoother: Smoother::new(params.high_width, SMOOTHING_MS, sr),
            mix_smoother: Smoother::new(params.mix, SMOOTHING_MS, sr),
            low_mid_freq_smoother: LogSmoother::new(params.low_mid_freq, FREQ_SMOOTHING_MS, sr),
            mid_high_freq_smoother: LogSmoother::new(params.mid_high_freq, FREQ_SMOOTHING_MS, sr),
        }
    }

    pub fn from_params(channels: usize, params: StereoImagerPluginParams) -> Self {
        Self::new(channels, params)
    }
}

impl InPlacePlugin for StereoImagerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("StereoImager", "1.0.0", "SotF")
            .with_description("Multi-band M/S stereo width control")
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float(
                "width",
                "Width",
                self.width,
                pk(SI, "width").min_f64() as f32,
                pk(SI, "width").max_f64() as f32,
            ),
            Parameter::new_float(
                "low_mid_freq",
                "Low-Mid Freq",
                self.low_mid_freq,
                pk(SI, "low_mid_freq").min_f64() as f32,
                pk(SI, "low_mid_freq").max_f64() as f32,
            ),
            Parameter::new_float(
                "mid_high_freq",
                "Mid-High Freq",
                self.mid_high_freq,
                pk(SI, "mid_high_freq").min_f64() as f32,
                pk(SI, "mid_high_freq").max_f64() as f32,
            ),
            Parameter::new_float(
                "low_width",
                "Low Width",
                self.low_width,
                pk(SI, "low_width").min_f64() as f32,
                pk(SI, "low_width").max_f64() as f32,
            ),
            Parameter::new_float(
                "mid_width",
                "Mid Width",
                self.mid_width,
                pk(SI, "mid_width").min_f64() as f32,
                pk(SI, "mid_width").max_f64() as f32,
            ),
            Parameter::new_float(
                "high_width",
                "High Width",
                self.high_width,
                pk(SI, "high_width").min_f64() as f32,
                pk(SI, "high_width").max_f64() as f32,
            ),
            Parameter::new_bool("mono_bass", "Mono Bass", self.mono_bass),
            Parameter::new_float(
                "mix",
                "Mix",
                self.mix,
                pk(SI, "mix").min_f64() as f32,
                pk(SI, "mix").max_f64() as f32,
            ),
        ]
    }

    fn validate_parameter(&self, id: &ParameterId, value: &ParameterValue) -> PluginResult<()> {
        let Some(spec) = SI.iter().find(|s| s.engine_key == id.as_str()) else {
            return Err(format!("Unknown parameter: {}", id));
        };
        match (&spec.param_type, value) {
            (ParamType::Float { min, max, .. }, ParameterValue::Float(v)) => {
                if v.is_nan() {
                    return Err(format!("{}: Value is NaN", id));
                }
                if v.is_infinite() {
                    return Err(format!("{}: Value is infinite", id));
                }
                if *v < *min as f32 {
                    return Err(format!(
                        "{}: Value {} is below minimum {}",
                        id, v, min
                    ));
                }
                if *v > *max as f32 {
                    return Err(format!(
                        "{}: Value {} is above maximum {}",
                        id, v, max
                    ));
                }
                Ok(())
            }
            (ParamType::Bool { .. }, ParameterValue::Bool(_)) => Ok(()),
            _ => Err(format!("{}: Parameter type mismatch", id)),
        }
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;

        match id.as_str() {
            "width" => {
                if let Some(v) = value.as_float() {
                    self.width = v;
                    self.width_smoother.set_target(v);
                }
            }
            "low_mid_freq" => {
                if let Some(v) = value.as_float() {
                    self.low_mid_freq = v;
                    self.low_mid_freq_smoother.set_target(v);
                }
            }
            "mid_high_freq" => {
                if let Some(v) = value.as_float() {
                    self.mid_high_freq = v;
                    self.mid_high_freq_smoother.set_target(v);
                }
            }
            "low_width" => {
                if let Some(v) = value.as_float() {
                    self.low_width = v;
                    self.low_width_smoother.set_target(v);
                }
            }
            "mid_width" => {
                if let Some(v) = value.as_float() {
                    self.mid_width = v;
                    self.mid_width_smoother.set_target(v);
                }
            }
            "high_width" => {
                if let Some(v) = value.as_float() {
                    self.high_width = v;
                    self.high_width_smoother.set_target(v);
                }
            }
            "mono_bass" => {
                if let Some(v) = value.as_bool() {
                    self.mono_bass = v;
                }
            }
            "mix" => {
                if let Some(v) = value.as_float() {
                    self.mix = v;
                    self.mix_smoother.set_target(v);
                }
            }
            _ => return Err(format!("Unknown parameter: {}", id)),
        }
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        match id.as_str() {
            "width" => Some(ParameterValue::Float(self.width)),
            "low_mid_freq" => Some(ParameterValue::Float(self.low_mid_freq)),
            "mid_high_freq" => Some(ParameterValue::Float(self.mid_high_freq)),
            "low_width" => Some(ParameterValue::Float(self.low_width)),
            "mid_width" => Some(ParameterValue::Float(self.mid_width)),
            "high_width" => Some(ParameterValue::Float(self.high_width)),
            "mono_bass" => Some(ParameterValue::Bool(self.mono_bass)),
            "mix" => Some(ParameterValue::Float(self.mix)),
            _ => None,
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;

        // Reinitialize crossovers at the correct sample rate
        self.crossover_low
            .reinit(self.low_mid_freq, sample_rate as f32, 2);
        self.crossover_high
            .reinit(self.mid_high_freq, sample_rate as f32, 2);

        // Reset smoothers at the new sample rate
        self.width_smoother = Smoother::new(self.width, SMOOTHING_MS, sample_rate);
        self.low_width_smoother = Smoother::new(self.low_width, SMOOTHING_MS, sample_rate);
        self.mid_width_smoother = Smoother::new(self.mid_width, SMOOTHING_MS, sample_rate);
        self.high_width_smoother = Smoother::new(self.high_width, SMOOTHING_MS, sample_rate);
        self.mix_smoother = Smoother::new(self.mix, SMOOTHING_MS, sample_rate);
        self.low_mid_freq_smoother =
            LogSmoother::new(self.low_mid_freq, FREQ_SMOOTHING_MS, sample_rate);
        self.mid_high_freq_smoother =
            LogSmoother::new(self.mid_high_freq, FREQ_SMOOTHING_MS, sample_rate);

        // Pre-allocate dry buffer for maximum expected frame size
        self.dry_buf.resize(8192 * 2, 0.0);

        Ok(())
    }

    fn reset(&mut self) {
        self.crossover_low.reset();
        self.crossover_high.reset();
        self.width_smoother.reset(self.width);
        self.low_width_smoother.reset(self.low_width);
        self.mid_width_smoother.reset(self.mid_width);
        self.high_width_smoother.reset(self.high_width);
        self.mix_smoother.reset(self.mix);
        self.low_mid_freq_smoother.reset(self.low_mid_freq);
        self.mid_high_freq_smoother.reset(self.mid_high_freq);
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();

        let nf = context.num_frames;

        // Stereo only -- pass through unchanged for non-stereo
        if self.channels != 2 {
            return Ok(nf);
        }

        // Ensure dry buffer is large enough (no allocation if already big enough)
        if self.dry_buf.len() < nf * 2 {
            self.dry_buf.resize(nf * 2, 0.0);
        }

        // Apply smoothed crossover frequencies before processing this buffer
        self.low_mid_freq_smoother.next_n(nf);
        let low_freq = self.low_mid_freq_smoother.current();
        if (low_freq - self.crossover_low.frequency()).abs() >= 0.1 {
            self.crossover_low.set_frequency(low_freq);
        }

        self.mid_high_freq_smoother.next_n(nf);
        let high_freq = self.mid_high_freq_smoother.current();
        if (high_freq - self.crossover_high.frequency()).abs() >= 0.1 {
            self.crossover_high.set_frequency(high_freq);
        }

        // Save dry signal for mix blending
        self.dry_buf[..nf * 2].copy_from_slice(&buffer[..nf * 2]);

        for frame in 0..nf {
            let idx = frame * 2;
            let l = buffer[idx];
            let r = buffer[idx + 1];

            // M/S encode
            let mid = (l + r) * 0.5;
            let side = (l - r) * 0.5;

            // Split mid and side into bands via cascaded crossovers.
            let (mid_low, mid_rest) = self.crossover_low.process(mid, 0);
            let (side_low, side_rest) = self.crossover_low.process(side, 1);
            let (mid_mid, mid_high) = self.crossover_high.process(mid_rest, 0);
            let (side_mid, side_high) = self.crossover_high.process(side_rest, 1);

            // Advance smoothers (per-sample)
            let gw = self.width_smoother.advance();
            let lw = self.low_width_smoother.advance();
            let mw = self.mid_width_smoother.advance();
            let hw = self.high_width_smoother.advance();

            // Apply per-band width scaling to side signal
            let low_side = if self.mono_bass {
                0.0
            } else {
                side_low * lw * gw
            };
            let mid_side_scaled = side_mid * mw * gw;
            let high_side_scaled = side_high * hw * gw;

            // Reconstruct total mid and side
            let total_mid = mid_low + mid_mid + mid_high;
            let total_side = low_side + mid_side_scaled + high_side_scaled;

            // M/S decode
            let wet_l = total_mid + total_side;
            let wet_r = total_mid - total_side;

            // Dry/wet mix
            let m = self.mix_smoother.advance();
            buffer[idx] = self.dry_buf[idx] * (1.0 - m) + wet_l * m;
            buffer[idx + 1] = self.dry_buf[idx + 1] * (1.0 - m) + wet_r * m;
        }

        flush_denormals_inplace(buffer);
        Ok(nf)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context(num_frames: usize) -> ProcessContext {
        ProcessContext {
            sample_rate: 48000,
            num_frames,
        }
    }

    /// Width=1.0, all band widths=1.0, mono_bass=false, mix=1.0 --> output equals input
    #[test]
    fn test_stereo_imager_passthrough() {
        let params = StereoImagerPluginParams {
            width: 1.0,
            low_mid_freq: 250.0,
            mid_high_freq: 4000.0,
            low_width: 1.0,
            mid_width: 1.0,
            high_width: 1.0,
            mono_bass: false,
            mix: 1.0,
        };
        let mut plugin = StereoImagerPlugin::new(2, params);
        plugin.initialize(48000).unwrap();

        // Feed a constant stereo signal for long enough to settle crossover transients
        let num_frames = 10000;
        let mut buffer = Vec::with_capacity(num_frames * 2);
        for _ in 0..num_frames {
            buffer.push(0.7); // L
            buffer.push(0.3); // R
        }
        let original = buffer.clone();

        plugin
            .process_in_place(&mut buffer, &make_context(num_frames))
            .unwrap();

        // Check the settled region (skip initial crossover transient)
        let settle = 2000;
        for frame in settle..num_frames {
            let idx = frame * 2;
            assert!(
                (buffer[idx] - original[idx]).abs() < 0.02,
                "frame {frame} L: expected {}, got {}",
                original[idx],
                buffer[idx]
            );
            assert!(
                (buffer[idx + 1] - original[idx + 1]).abs() < 0.02,
                "frame {frame} R: expected {}, got {}",
                original[idx + 1],
                buffer[idx + 1]
            );
        }
    }

    /// Width=0.0 --> L and R should be identical (mono)
    #[test]
    fn test_stereo_imager_mono() {
        let params = StereoImagerPluginParams {
            width: 0.0,
            low_mid_freq: 250.0,
            mid_high_freq: 4000.0,
            low_width: 1.0,
            mid_width: 1.0,
            high_width: 1.0,
            mono_bass: false,
            mix: 1.0,
        };
        let mut plugin = StereoImagerPlugin::new(2, params);
        plugin.initialize(48000).unwrap();

        let num_frames = 10000;
        let mut buffer = Vec::with_capacity(num_frames * 2);
        for i in 0..num_frames {
            buffer.push((i as f32 * 0.01).sin() * 0.5); // L
            buffer.push((i as f32 * 0.02).cos() * 0.3); // R
        }

        plugin
            .process_in_place(&mut buffer, &make_context(num_frames))
            .unwrap();

        // After settling, L and R should be identical (both = mid only)
        let settle = 2000;
        for frame in settle..num_frames {
            let idx = frame * 2;
            let diff = (buffer[idx] - buffer[idx + 1]).abs();
            assert!(
                diff < 0.01,
                "frame {frame}: L={} R={} diff={diff}",
                buffer[idx],
                buffer[idx + 1]
            );
        }
    }

    /// mono_bass=true --> low frequencies should be mono
    #[test]
    fn test_stereo_imager_mono_bass() {
        let params = StereoImagerPluginParams {
            width: 1.0,
            low_mid_freq: 250.0,
            mid_high_freq: 4000.0,
            low_width: 1.0,
            mid_width: 1.0,
            high_width: 1.0,
            mono_bass: true,
            mix: 1.0,
        };
        let mut plugin = StereoImagerPlugin::new(2, params);
        plugin.initialize(48000).unwrap();

        let num_frames = 10000;
        let mut buffer = Vec::with_capacity(num_frames * 2);
        for _ in 0..num_frames {
            buffer.push(0.8); // L
            buffer.push(0.2); // R
        }

        plugin
            .process_in_place(&mut buffer, &make_context(num_frames))
            .unwrap();

        let settle = 2000;
        for frame in settle..num_frames {
            let idx = frame * 2;
            let diff = (buffer[idx] - buffer[idx + 1]).abs();
            assert!(
                diff < 0.05,
                "frame {frame}: L={} R={} diff={diff} (expected mono bass)",
                buffer[idx],
                buffer[idx + 1]
            );
        }
    }

    /// Width=2.0 --> side component should be doubled
    #[test]
    fn test_stereo_imager_wide() {
        let params = StereoImagerPluginParams {
            width: 2.0,
            low_mid_freq: 250.0,
            mid_high_freq: 4000.0,
            low_width: 1.0,
            mid_width: 1.0,
            high_width: 1.0,
            mono_bass: false,
            mix: 1.0,
        };
        let mut plugin = StereoImagerPlugin::new(2, params);
        plugin.initialize(48000).unwrap();

        let num_frames = 10000;
        let mut buffer = Vec::with_capacity(num_frames * 2);
        for _ in 0..num_frames {
            buffer.push(0.8);
            buffer.push(0.2);
        }

        plugin
            .process_in_place(&mut buffer, &make_context(num_frames))
            .unwrap();

        let last = (num_frames - 1) * 2;
        let l = buffer[last];
        let r = buffer[last + 1];

        assert!(l > 0.9, "Wide L should be > 0.9, got {l}");
        assert!(r < 0.1, "Wide R should be < 0.1, got {r}");

        let diff = l - r;
        let original_diff = 0.6;
        assert!(
            diff > original_diff * 1.5,
            "L-R difference ({diff}) should be significantly larger than original ({original_diff})"
        );
    }

    /// Parameter set/get roundtrip
    #[test]
    fn test_parameter_roundtrip() {
        let mut plugin = StereoImagerPlugin::new(2, StereoImagerPluginParams::default());
        plugin.initialize(48000).unwrap();

        plugin
            .set_parameter(ParameterId::from("width"), ParameterValue::Float(1.5))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("width"));
        assert_eq!(val, Some(ParameterValue::Float(1.5)));

        plugin
            .set_parameter(ParameterId::from("mono_bass"), ParameterValue::Bool(true))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("mono_bass"));
        assert_eq!(val, Some(ParameterValue::Bool(true)));

        plugin
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.75))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("mix"));
        assert_eq!(val, Some(ParameterValue::Float(0.75)));

        let result =
            plugin.set_parameter(ParameterId::from("nonexistent"), ParameterValue::Float(1.0));
        assert!(result.is_err());

        assert_eq!(
            plugin.get_parameter(&ParameterId::from("nonexistent")),
            None
        );
    }

    /// Non-stereo channels should pass through unchanged
    #[test]
    fn test_non_stereo_passthrough() {
        let mut plugin = StereoImagerPlugin::new(1, StereoImagerPluginParams::default());
        plugin.initialize(48000).unwrap();

        let mut buffer = vec![0.5, 0.3, 0.7, 0.1];
        let original = buffer.clone();

        plugin
            .process_in_place(&mut buffer, &make_context(4))
            .unwrap();

        assert_eq!(buffer, original);
    }

    /// Changing crossover frequency should not update coefficients instantly.
    /// The frequency should smooth over time via LogSmoother.
    #[test]
    fn test_crossover_frequency_smoothing() {
        let mut plugin = StereoImagerPlugin::new(2, StereoImagerPluginParams::default());
        plugin.initialize(48000).unwrap();

        let initial_low_freq = plugin.crossover_low.frequency();
        assert!((initial_low_freq - 250.0).abs() < 0.1);

        // Change low-mid freq via set_parameter
        plugin
            .set_parameter(
                ParameterId::from("low_mid_freq"),
                ParameterValue::Float(1000.0),
            )
            .unwrap();

        // Crossover frequency should NOT have updated instantly
        assert!(
            (plugin.crossover_low.frequency() - initial_low_freq).abs() < 0.1,
            "crossover frequency updated instantly: got {}, expected ~{}",
            plugin.crossover_low.frequency(),
            initial_low_freq
        );

        // Process a buffer to let the smoother advance
        let mut buffer = vec![0.5_f32; 512 * 2];
        plugin.process_in_place(&mut buffer, &ProcessContext { num_frames: 512, sample_rate: 48000 }).unwrap();

        // After processing, the frequency should have moved toward the target
        let new_freq = plugin.crossover_low.frequency();
        assert!(
            new_freq > initial_low_freq,
            "frequency should have increased after processing: got {}, was {}",
            new_freq,
            initial_low_freq
        );
        assert!(
            new_freq < 1000.0,
            "frequency should not have reached target yet: got {}, target 1000",
            new_freq
        );

        // Process many more buffers until the frequency reaches the target
        for _ in 0..200 {
            plugin.process_in_place(&mut buffer, &ProcessContext { num_frames: 512, sample_rate: 48000 }).unwrap();
        }

        let final_freq = plugin.crossover_low.frequency();
        assert!(
            (final_freq - 1000.0).abs() < 1.0,
            "frequency should have reached target after many buffers: got {}, target 1000",
            final_freq
        );
    }

    /// set_parameter should not allocate (regression test for real-time safety).
    /// We verify functional correctness; the removal of cached_parameters/rebuild
    /// eliminates the Vec allocation path.
    #[test]
    fn test_set_parameter_no_allocation() {
        let mut plugin = StereoImagerPlugin::new(2, StereoImagerPluginParams::default());
        plugin.initialize(48000).unwrap();

        // Set every parameter repeatedly
        for i in 0..100 {
            let v = 0.5 + (i as f32 * 0.001);
            plugin
                .set_parameter(ParameterId::from("width"), ParameterValue::Float(v))
                .unwrap();
            plugin
                .set_parameter(ParameterId::from("low_mid_freq"), ParameterValue::Float(200.0 + v * 10.0))
                .unwrap();
            plugin
                .set_parameter(ParameterId::from("mid_high_freq"), ParameterValue::Float(3000.0 + v * 100.0))
                .unwrap();
            plugin
                .set_parameter(ParameterId::from("mix"), ParameterValue::Float(v.min(1.0)))
                .unwrap();
        }

        // parameters() should still return correct values
        let params = plugin.parameters();
        assert_eq!(params.len(), 8);

        // Verify the last-set values are reflected
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("width")),
            Some(ParameterValue::Float(0.5 + 99.0 * 0.001))
        );
    }
}

