// ============================================================================
// Delay Plugin
// ============================================================================

use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use super::smoothing::Smoother;
use super::simd::{enable_ftz_daz, flush_denormals_inplace};
use serde::{Deserialize, Serialize};

const MAX_DELAY_MS: f32 = 5000.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelayPluginParams {
    #[serde(default = "default_delay_ms")]
    pub delay_ms: f32,
    #[serde(default = "default_feedback")]
    pub feedback: f32,
    #[serde(default = "default_mix")]
    pub mix: f32,
}

fn default_delay_ms() -> f32 { 100.0 }
fn default_feedback() -> f32 { 0.3 }
fn default_mix() -> f32 { 0.5 }

pub struct DelayPlugin {
    channels: usize,
    sample_rate: u32,
    param_delay_ms: ParameterId,
    delay_ms: f32,
    param_feedback: ParameterId,
    feedback: f32,
    param_mix: ParameterId,
    mix: f32,
    delay_smoother: Smoother,
    feedback_smoother: Smoother,
    mix_smoother: Smoother,
    buffer: Vec<f32>,
    write_pos: usize,
    max_samples: usize,
}

impl DelayPlugin {
    pub fn new(channels: usize, delay_ms: f32, feedback: f32, mix: f32) -> Self {
        let sr = 44100;
        let max_samples = (MAX_DELAY_MS * 0.001 * sr as f32) as usize + 2;
        Self {
            channels, sample_rate: sr,
            param_delay_ms: ParameterId::from("delay_ms"), delay_ms,
            param_feedback: ParameterId::from("feedback"), feedback,
            param_mix: ParameterId::from("mix"), mix,
            delay_smoother: Smoother::new(delay_ms * sr as f32 / 1000.0, 50.0, sr),
            feedback_smoother: Smoother::new(feedback, 5.0, sr),
            mix_smoother: Smoother::new(mix, 5.0, sr),
            buffer: vec![0.0; max_samples * channels],
            write_pos: 0,
            max_samples,
        }
    }

    pub fn from_params(channels: usize, params: DelayPluginParams) -> Self {
        Self::new(channels, params.delay_ms, params.feedback, params.mix)
    }
}

impl InPlacePlugin for DelayPlugin {
    fn info(&self) -> PluginInfo { PluginInfo::new("Delay", "1.1.0", "SotF") }
    fn channels(&self) -> usize { self.channels }
    fn parameters(&self) -> Vec<Parameter> {
        vec![Parameter::new_float("delay_ms", "Delay Time", 100.0, 0.1, MAX_DELAY_MS),
             Parameter::new_float("feedback", "Feedback", 0.3, 0.0, 0.95),
             Parameter::new_float("mix", "Mix", 0.5, 0.0, 1.0)]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_delay_ms { self.delay_ms = value.as_float().ok_or("val")?; self.delay_smoother.set_target(self.delay_ms * self.sample_rate as f32 / 1000.0); }
        else if id == self.param_feedback { self.feedback = value.as_float().ok_or("val")?; self.feedback_smoother.set_target(self.feedback); }
        else if id == self.param_mix { self.mix = value.as_float().ok_or("val")?; self.mix_smoother.set_target(self.mix); }
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_delay_ms { Some(ParameterValue::Float(self.delay_ms)) }
        else if id == &self.param_feedback { Some(ParameterValue::Float(self.feedback)) }
        else if id == &self.param_mix { Some(ParameterValue::Float(self.mix)) }
        else { None }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.max_samples = (MAX_DELAY_MS * 0.001 * sample_rate as f32) as usize + 2;
        self.buffer.resize(self.max_samples * self.channels, 0.0);
        self.delay_smoother = Smoother::new(self.delay_ms * sample_rate as f32 / 1000.0, 50.0, sample_rate);
        self.feedback_smoother.set_time(5.0, sample_rate);
        self.mix_smoother.set_time(5.0, sample_rate);
        Ok(())
    }

    fn reset(&mut self) { self.buffer.fill(0.0); self.write_pos = 0; }

    fn process_in_place(&mut self, buffer: &mut [f32], context: &ProcessContext) -> PluginResult<usize> {
        enable_ftz_daz();
        let num_frames = context.num_frames;
        for frame in 0..num_frames {
            let delay_samples = self.delay_smoother.next();
            let fb = self.feedback_smoother.next();
            let mix = self.mix_smoother.next();

            let int_delay = delay_samples.floor() as usize;
            let frac_delay = delay_samples - int_delay as f32;

            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                let input = buffer[idx];

                // Fractional delay read (linear interpolation)
                let r1 = (self.write_pos + self.max_samples - int_delay) % self.max_samples;
                let r2 = (r1 + self.max_samples - 1) % self.max_samples;
                
                let s1 = self.buffer[r1 * self.channels + ch];
                let s2 = self.buffer[r2 * self.channels + ch];
                let delayed = s1 + frac_delay * (s2 - s1);

                self.buffer[self.write_pos * self.channels + ch] = input + delayed * fb;
                buffer[idx] = input * (1.0 - mix) + delayed * mix;
            }
            self.write_pos = (self.write_pos + 1) % self.max_samples;
        }
        flush_denormals_inplace(buffer);
        Ok(num_frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_delay_basic() {
        let mut p = DelayPlugin::new(1, 10.0, 0.5, 0.5);
        p.initialize(48000).unwrap();
        let mut b = vec![1.0; 1000];
        p.process_in_place(&mut b, &ProcessContext { sample_rate: 48000, num_frames: 1000 }).unwrap();
        assert!(b[999] != 1.0);
    }
}
