// ============================================================================
// Stereo Imager Plugin - Multi-band M/S stereo width control
// ============================================================================

pub mod params;

use crate::params::PARAMS as SI;
use sotf_host::lr4_crossover::Lr4Crossover;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;

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

    cached_parameters: Vec<Parameter>,
}

const SMOOTHING_MS: f32 = 10.0;

impl StereoImagerPlugin {
    pub fn new(channels: usize, params: StereoImagerPluginParams) -> Self {
        let sr = 48000;
        let mut plugin = Self {
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

            cached_parameters: Vec::new(),
        };
        plugin.rebuild_cached_parameters();
        plugin
    }

    pub fn from_params(channels: usize, params: StereoImagerPluginParams) -> Self {
        Self::new(channels, params)
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
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
        ];
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
        self.cached_parameters.clone()
    }

    /// Override default to avoid cloning `cached_parameters` on every call.
    /// The trait default calls `self.parameters()` (a clone), which allocates.
    /// Instead, validate directly against the static PARAMS table — no heap
    /// allocation required.
    fn validate_parameter(&self, id: &ParameterId, value: &ParameterValue) -> PluginResult<()> {
        // Search the static PARAMS array; `find_by_key` panics so use .find() directly.
        let spec = SI.iter().find(|s| s.engine_key == id.as_str());
        if let Some(spec) = spec {
            if let ParameterValue::Float(v) = value {
                let min = spec.min_f64() as f32;
                let max = spec.max_f64() as f32;
                if *v < min || *v > max {
                    return Err(format!(
                        "{}: value {} out of range [{}, {}]",
                        id, v, min, max
                    ));
                }
            }
            Ok(())
        } else {
            Err(format!("Unknown parameter: {}", id))
        }
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;

        // Update the corresponding cached_parameter in-place (no Vec reallocation).
        // `Parameter::default_value` is the current value field used by the host.
        macro_rules! update_cached {
            ($id:expr, $val:expr) => {
                if let Some(p) = self.cached_parameters.iter_mut().find(|p| p.id == $id) {
                    p.default_value = $val;
                }
            };
        }

        match id.as_str() {
            "width" => {
                if let Some(v) = value.as_float() {
                    self.width = v;
                    self.width_smoother.set_target(v);
                    update_cached!(id, ParameterValue::Float(v));
                }
            }
            "low_mid_freq" => {
                if let Some(v) = value.as_float() {
                    self.low_mid_freq = v;
                    self.crossover_low.set_frequency(v);
                    update_cached!(id, ParameterValue::Float(v));
                }
            }
            "mid_high_freq" => {
                if let Some(v) = value.as_float() {
                    self.mid_high_freq = v;
                    self.crossover_high.set_frequency(v);
                    update_cached!(id, ParameterValue::Float(v));
                }
            }
            "low_width" => {
                if let Some(v) = value.as_float() {
                    self.low_width = v;
                    self.low_width_smoother.set_target(v);
                    update_cached!(id, ParameterValue::Float(v));
                }
            }
            "mid_width" => {
                if let Some(v) = value.as_float() {
                    self.mid_width = v;
                    self.mid_width_smoother.set_target(v);
                    update_cached!(id, ParameterValue::Float(v));
                }
            }
            "high_width" => {
                if let Some(v) = value.as_float() {
                    self.high_width = v;
                    self.high_width_smoother.set_target(v);
                    update_cached!(id, ParameterValue::Float(v));
                }
            }
            "mono_bass" => {
                if let Some(v) = value.as_bool() {
                    self.mono_bass = v;
                    update_cached!(id, ParameterValue::Bool(v));
                }
            }
            "mix" => {
                if let Some(v) = value.as_float() {
                    self.mix = v;
                    self.mix_smoother.set_target(v);
                    update_cached!(id, ParameterValue::Float(v));
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

        // Pre-allocate dry buffer for maximum expected frame size.
        // 65536 * 2 covers virtually all real-world buffer sizes (up to ~1.4s @ 48 kHz).
        self.dry_buf.resize(65536 * 2, 0.0);

        Ok(())
    }

    fn reset(&mut self) {
        self.crossover_low.reset();
        self.crossover_high.reset();
        // Snap all smoothers to their current target values so a reset
        // during a parameter transition does not resume the ramp.
        self.width_smoother.reset(self.width);
        self.low_width_smoother.reset(self.low_width);
        self.mid_width_smoother.reset(self.mid_width);
        self.high_width_smoother.reset(self.high_width);
        self.mix_smoother.reset(self.mix);
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

        // Ensure dry buffer is large enough.
        // initialize() pre-allocates 65536*2 samples covering virtually all
        // real-world buffer sizes; this resize is a last-resort fallback only.
        if self.dry_buf.len() < nf * 2 {
            self.dry_buf.resize(nf * 2, 0.0);
        }

        // Save dry signal for mix blending only when wet/dry blend is active.
        // At mix=1.0 (default) the copy is wasted; skip it to save memory bandwidth.
        let need_dry = !(self.mix_smoother.target() >= 1.0 - f32::EPSILON
            && self.mix_smoother.current() >= 1.0 - f32::EPSILON);
        if need_dry {
            self.dry_buf[..nf * 2].copy_from_slice(&buffer[..nf * 2]);
        }

        // Hoist mono_bass out of the per-sample loop — it changes extremely rarely
        // and checking a bool every sample wastes branch prediction budget.
        let mono_bass = self.mono_bass;

        for frame in 0..nf {
            let idx = frame * 2;
            let l = buffer[idx];
            let r = buffer[idx + 1];

            // M/S encode
            let mid = (l + r) * 0.5;
            let side = (l - r) * 0.5;

            // Split mid and side into bands via cascaded crossovers.
            // crossover_low: channel 0 = mid signal, channel 1 = side signal
            let (mid_low, mid_rest) = self.crossover_low.process(mid, 0);
            let (side_low, side_rest) = self.crossover_low.process(side, 1);
            // crossover_high: channel 0 = mid rest, channel 1 = side rest
            let (mid_mid, mid_high) = self.crossover_high.process(mid_rest, 0);
            let (side_mid, side_high) = self.crossover_high.process(side_rest, 1);

            // Advance smoothers (per-sample)
            let gw = self.width_smoother.advance();
            let lw = self.low_width_smoother.advance();
            let mw = self.mid_width_smoother.advance();
            let hw = self.high_width_smoother.advance();

            // Apply per-band width scaling to side signal.
            // mono_bass is hoisted above the loop — no per-sample branch.
            let low_side = if mono_bass { 0.0 } else { side_low * lw * gw };
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
            if need_dry {
                buffer[idx] = self.dry_buf[idx] * (1.0 - m) + wet_l * m;
                buffer[idx + 1] = self.dry_buf[idx + 1] * (1.0 - m) + wet_r * m;
            } else {
                buffer[idx] = wet_l;
                buffer[idx + 1] = wet_r;
            }
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

    /// reset() must snap all smoothers to their current target values.
    /// If a smoother is mid-transition when reset() is called, it must
    /// jump immediately to the target — not resume from where it left off.
    #[test]
    fn test_reset_snaps_smoothers() {
        let mut plugin = StereoImagerPlugin::new(2, StereoImagerPluginParams::default());
        plugin.initialize(48000).unwrap();

        // Drive the width smoother into a transition: set a new target
        plugin
            .set_parameter(ParameterId::from("width"), ParameterValue::Float(0.0))
            .unwrap();

        // reset() should snap the smoother to the new target (0.0)
        plugin.reset();

        // After reset, processing one frame: the smoother must produce 0.0,
        // meaning side is fully suppressed — L and R must be equal (both = mid).
        let num_frames = 512;
        let mut buffer = Vec::with_capacity(num_frames * 2);
        for i in 0..num_frames {
            let t = i as f32 * 0.01;
            buffer.push(t.sin() * 0.5); // L
            buffer.push(t.cos() * 0.3); // R
        }
        plugin
            .process_in_place(&mut buffer, &make_context(num_frames))
            .unwrap();

        // Every sample must have L == R (no smoother ramp-in from the old value)
        for frame in 0..num_frames {
            let idx = frame * 2;
            let diff = (buffer[idx] - buffer[idx + 1]).abs();
            assert!(
                diff < 0.01,
                "frame {frame}: L={} R={} — smoother was not snapped by reset()",
                buffer[idx],
                buffer[idx + 1]
            );
        }
    }

    /// mono_bass=false: the per-sample mono_bass branch must not change output
    /// when toggled rapidly mid-buffer (no crash, no NaN).
    #[test]
    fn test_rapid_mono_bass_toggle_no_nan() {
        let mut plugin = StereoImagerPlugin::new(2, StereoImagerPluginParams::default());
        plugin.initialize(48000).unwrap();

        let num_frames = 256;
        let mut buffer: Vec<f32> = (0..num_frames * 2)
            .map(|i| (i as f32 * 0.1).sin() * 0.5)
            .collect();

        // Toggle mono_bass multiple times before and during processing
        for _ in 0..5 {
            plugin
                .set_parameter(ParameterId::from("mono_bass"), ParameterValue::Bool(true))
                .unwrap();
            plugin
                .set_parameter(ParameterId::from("mono_bass"), ParameterValue::Bool(false))
                .unwrap();
        }

        plugin
            .process_in_place(&mut buffer, &make_context(num_frames))
            .unwrap();

        for (i, &s) in buffer.iter().enumerate() {
            assert!(
                !s.is_nan(),
                "NaN at sample {i} after rapid mono_bass toggle"
            );
            assert!(
                !s.is_infinite(),
                "Inf at sample {i} after rapid mono_bass toggle"
            );
        }
    }

    /// Buffer larger than 8192 frames: must not panic (dry_buf must accommodate).
    #[test]
    fn test_large_buffer_no_panic() {
        let mut plugin = StereoImagerPlugin::new(2, StereoImagerPluginParams::default());
        plugin.initialize(48000).unwrap();

        let num_frames = 16384;
        let mut buffer = vec![0.5_f32; num_frames * 2];
        // Should not panic
        plugin
            .process_in_place(&mut buffer, &make_context(num_frames))
            .unwrap();
    }

    /// Mix=0 (fully dry): output must be byte-for-byte identical to input.
    #[test]
    fn test_mix_zero_full_passthrough() {
        let params = StereoImagerPluginParams {
            mix: 0.0,
            width: 2.0, // Irrelevant — mix=0 means pure dry
            low_mid_freq: 250.0,
            mid_high_freq: 4000.0,
            low_width: 1.0,
            mid_width: 1.0,
            high_width: 1.0,
            mono_bass: false,
        };
        let mut plugin = StereoImagerPlugin::new(2, params);
        plugin.initialize(48000).unwrap();

        let num_frames = 512;
        let mut buffer: Vec<f32> = (0..num_frames * 2)
            .map(|i| (i as f32 * 0.05).sin() * 0.7)
            .collect();
        let original = buffer.clone();

        plugin
            .process_in_place(&mut buffer, &make_context(num_frames))
            .unwrap();

        for (i, (&orig, &out)) in original.iter().zip(buffer.iter()).enumerate() {
            assert_eq!(
                orig, out,
                "sample {i}: mix=0 changed output: expected {orig}, got {out}"
            );
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

        // Feed a DC offset (which is all "low" band): L=0.8, R=0.2
        // With mono_bass, the low band side is collapsed to zero.
        // So the low band becomes mid only: (0.8+0.2)*0.5 = 0.5 for both L and R.
        // But mid and high bands still carry the original stereo difference.
        let num_frames = 10000;
        let mut buffer = Vec::with_capacity(num_frames * 2);
        for _ in 0..num_frames {
            buffer.push(0.8); // L
            buffer.push(0.2); // R
        }

        plugin
            .process_in_place(&mut buffer, &make_context(num_frames))
            .unwrap();

        // After settling, DC should be in low band only, and mono_bass collapses
        // the side, so L and R converge for this DC signal.
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

        // With width=2.0 and all band widths=1.0, the side is scaled by 2.0.
        // For a constant signal: L=0.8, R=0.2:
        //   mid = 0.5, side = 0.3
        //   scaled_side = 0.3 * 2.0 = 0.6
        //   wet_L = 0.5 + 0.6 = 1.1
        //   wet_R = 0.5 - 0.6 = -0.1
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

        // L should be wider than original 0.8, R should be narrower than 0.2
        assert!(l > 0.9, "Wide L should be > 0.9, got {l}");
        assert!(r < 0.1, "Wide R should be < 0.1, got {r}");

        // The difference (L-R) should be roughly 4x the original side
        // Original side content: (0.8-0.2)/2 = 0.3 * 2 (width) = 0.6 each way
        // So L-R = 2*scaled_side = 1.2 vs original L-R = 0.6
        let diff = l - r;
        let original_diff = 0.6; // 0.8 - 0.2
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

        // Set width
        plugin
            .set_parameter(ParameterId::from("width"), ParameterValue::Float(1.5))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("width"));
        assert_eq!(val, Some(ParameterValue::Float(1.5)));

        // Set mono_bass
        plugin
            .set_parameter(ParameterId::from("mono_bass"), ParameterValue::Bool(true))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("mono_bass"));
        assert_eq!(val, Some(ParameterValue::Bool(true)));

        // Set mix
        plugin
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.75))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("mix"));
        assert_eq!(val, Some(ParameterValue::Float(0.75)));

        // Unknown parameter should fail
        let result =
            plugin.set_parameter(ParameterId::from("nonexistent"), ParameterValue::Float(1.0));
        assert!(result.is_err());

        // Unknown get should return None
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
}
