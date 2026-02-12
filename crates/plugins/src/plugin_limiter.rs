// ============================================================================
// Limiter Plugin
// ============================================================================

use super::param_specs::limiter::*;
use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use super::smoothing::Smoother;
use super::simd::{enable_ftz_daz, flush_denormals_inplace};
use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimiterPluginParams {
    #[serde(default = "default_threshold_db")]
    pub threshold_db: f32,
    #[serde(default = "default_release_ms")]
    pub release_ms: f32,
    #[serde(default = "default_lookahead_ms")]
    pub lookahead_ms: f32,
    #[serde(default = "default_soft")]
    pub soft: bool,
    #[serde(default = "default_mix")]
    pub mix: f32,
}

fn default_threshold_db() -> f32 { THRESHOLD_DEFAULT }
fn default_release_ms() -> f32 { RELEASE_DEFAULT }
fn default_lookahead_ms() -> f32 { LOOKAHEAD_DEFAULT }
fn default_soft() -> bool { SOFT_DEFAULT }
fn default_mix() -> f32 { MIX_DEFAULT }

pub struct LimiterPlugin {
    channels: usize,
    sample_rate: u32,
    param_threshold: ParameterId,
    threshold_db: f32,
    param_release: ParameterId,
    release_ms: f32,
    param_lookahead: ParameterId,
    lookahead_ms: f32,
    param_soft: ParameterId,
    soft: bool,
    param_mix: ParameterId,
    mix: f32,
    threshold_smoother: Smoother,
    mix_smoother: Smoother,
    envelope: f32,
    release_coeff: f32,
    lookahead_buffer: Vec<f32>,
    lookahead_pos: usize,
    lookahead_len: usize,
}

impl LimiterPlugin {
    pub fn new(channels: usize, threshold_db: f32, release_ms: f32, lookahead_ms: f32, soft: bool) -> Self {
        let sr = 44100;
        let lookahead_len = ((lookahead_ms * 0.001 * sr as f32) as usize).max(1);
        Self {
            channels,
            sample_rate: sr,
            param_threshold: ParameterId::from("threshold"),
            threshold_db,
            param_release: ParameterId::from("release"),
            release_ms,
            param_lookahead: ParameterId::from("lookahead"),
            lookahead_ms,
            param_soft: ParameterId::from("soft"),
            soft,
            param_mix: ParameterId::from("mix"),
            mix: 1.0,
            threshold_smoother: Smoother::new(fast_pow10(threshold_db / 20.0), 5.0, sr),
            mix_smoother: Smoother::new(1.0, 5.0, sr),
            envelope: 0.0,
            release_coeff: 0.0,
            lookahead_buffer: vec![0.0; lookahead_len * channels],
            lookahead_pos: 0,
            lookahead_len,
        }
    }

    pub fn from_params(channels: usize, params: LimiterPluginParams) -> Self {
        let mut p = Self::new(channels, params.threshold_db, params.release_ms, params.lookahead_ms, params.soft);
        p.mix = params.mix.clamp(0.0, 1.0);
        p
    }

    fn update_coefficients(&mut self) {
        self.release_coeff = (-1.0 / (self.release_ms * 0.001 * self.sample_rate as f32)).exp();
        let new_len = ((self.lookahead_ms * 0.001 * self.sample_rate as f32) as usize).max(1);
        if new_len != self.lookahead_len {
            self.lookahead_len = new_len;
            self.lookahead_buffer.resize(new_len * self.channels, 0.0);
            self.lookahead_pos = 0;
        }
    }
}

impl InPlacePlugin for LimiterPlugin {
    fn info(&self) -> PluginInfo { PluginInfo::new("Limiter", "1.1.0", "SotF") }
    fn channels(&self) -> usize { self.channels }
    fn parameters(&self) -> Vec<Parameter> {
        vec![Parameter::new_float("threshold", "Threshold", THRESHOLD_DEFAULT, THRESHOLD_MIN, THRESHOLD_MAX),
             Parameter::new_float("release", "Release", RELEASE_DEFAULT, RELEASE_MIN, RELEASE_MAX),
             Parameter::new_bool("soft", "Soft", SOFT_DEFAULT)]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_threshold { self.threshold_db = value.as_float().ok_or("val")?; self.threshold_smoother.set_target(fast_pow10(self.threshold_db / 20.0)); }
        else if id == self.param_release { self.release_ms = value.as_float().ok_or("val")?.max(1.0); self.update_coefficients(); }
        else if id == self.param_lookahead { self.lookahead_ms = value.as_float().ok_or("val")?.max(0.0); self.update_coefficients(); }
        else if id == self.param_soft { self.soft = value.as_bool().ok_or("val")?; }
        else if id == self.param_mix { self.mix = value.as_float().ok_or("val")?.clamp(0.0, 1.0); self.mix_smoother.set_target(self.mix); }
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_threshold { Some(ParameterValue::Float(self.threshold_db)) }
        else if id == &self.param_mix { Some(ParameterValue::Float(self.mix)) }
        else { None }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.update_coefficients();
        self.threshold_smoother.set_time(5.0, sample_rate);
        self.mix_smoother.set_time(5.0, sample_rate);
        Ok(())
    }

    fn reset(&mut self) {
        self.envelope = 0.0;
        self.lookahead_buffer.fill(0.0);
    }

    fn process_in_place(&mut self, buffer: &mut [f32], context: &ProcessContext) -> PluginResult<usize> {
        enable_ftz_daz();
        let num_frames = context.num_frames;
        let thresh = self.threshold_smoother.next();
        let mix = self.mix_smoother.next();

        for frame in 0..num_frames {
            let mut frame_peak = 0.0f32;
            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                frame_peak = frame_peak.max(buffer[idx].abs());
            }

            // Predictive peak from input
            let target_gr = if frame_peak > thresh { 20.0 * fast_log10(frame_peak / thresh) } else { 0.0 };
            
            // Lookahead limiting simplified: we want gain to drop FAST.
            // Attack is implicitly instantaneous due to lookahead peak detection.
            if target_gr > self.envelope {
                self.envelope = target_gr; // Instant attack on detection
            } else {
                self.envelope = target_gr + self.release_coeff * (self.envelope - target_gr);
            }

            let gain = fast_pow10(-self.envelope / 20.0);

            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                let input_sample = buffer[idx];
                
                // Write to circular buffer, read delayed
                let buf_idx = self.lookahead_pos * self.channels + ch;
                let delayed = self.lookahead_buffer[buf_idx];
                self.lookahead_buffer[buf_idx] = input_sample;

                let limited = if self.soft {
                    let norm = (delayed * gain) / thresh;
                    thresh * (norm * 0.75).tanh()
                } else {
                    (delayed * gain).clamp(-thresh, thresh)
                };

                buffer[idx] = (1.0 - mix) * delayed + mix * limited;
            }
            self.lookahead_pos = (self.lookahead_pos + 1) % self.lookahead_len;
        }
        flush_denormals_inplace(buffer);
        Ok(num_frames)
    }

    fn latency_samples(&self) -> usize { self.lookahead_len }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_limiter_basic() {
        let mut p = LimiterPlugin::new(1, -1.0, 50.0, 5.0, false);
        p.initialize(48000).unwrap();
        let mut b = vec![2.0; 1000];
        p.process_in_place(&mut b, &ProcessContext { sample_rate: 48000, num_frames: 1000 }).unwrap();
        let thresh_lin = fast_pow10(-1.0/20.0);
        for &s in &b[500..] { assert!(s.abs() <= thresh_lin * 1.05); }
    }
}
