// ============================================================================
// Band Merge Plugin
// ============================================================================

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::{enable_ftz_daz, flush_denormals_inplace};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandMergePluginParams {
    #[serde(default = "default_num_bands")]
    pub bands: usize,
}

fn default_num_bands() -> usize { 2 }

pub struct BandMergePlugin {
    output_channels: usize,
    num_bands: usize,
    param_bands: ParameterId,
}

impl BandMergePlugin {
    pub fn new(output_channels: usize, bands: usize) -> Result<Self, String> {
        if bands < 2 { return Err("Min 2 bands".into()); }
        Ok(Self { output_channels, num_bands: bands, param_bands: ParameterId("bands".to_string()) })
    }
    pub fn from_params(output_channels: usize, params: &BandMergePluginParams) -> Result<Self, String> {
        Self::new(output_channels, params.bands)
    }
}

impl Plugin for BandMergePlugin {
    fn info(&self) -> PluginInfo { PluginInfo::new("BandMerge", "1.1.0", "Sotf") }
    fn input_channels(&self) -> usize { self.output_channels * self.num_bands }
    fn output_channels(&self) -> usize { self.output_channels }
    fn parameters(&self) -> Vec<Parameter> { Vec::new() }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_bands { self.num_bands = value.as_int().ok_or("val")? as usize; Ok(()) }
        else { Err("unknown".into()) }
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_bands { Some(ParameterValue::Int(self.num_bands as i32)) } else { None }
    }
    fn initialize(&mut self, _sample_rate: u32) -> PluginResult<()> { Ok(()) }
    fn reset(&mut self) {}

    fn process(&mut self, input: &[f32], output: &mut [f32], context: &ProcessContext) -> Result<usize, String> {
        enable_ftz_daz();
        let num_frames = context.num_frames;
        let out_ch = self.output_channels;
        let in_ch = out_ch * self.num_bands;

        for frame in 0..num_frames {
            let in_off = frame * in_ch;
            let out_off = frame * out_ch;
            for ch in 0..out_ch {
                let mut sum = 0.0f32;
                for band in 0..self.num_bands {
                    sum += input[in_off + band * out_ch + ch];
                }
                output[out_off + ch] = sum;
            }
        }
        flush_denormals_inplace(output);
        Ok(num_frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_band_merge_basic() {
        let mut p = BandMergePlugin::new(2, 2).unwrap();
        let i = vec![1.0, 2.0, 3.0, 4.0]; let mut o = vec![0.0, 0.0];
        p.process(&i, &mut o, &ProcessContext { sample_rate: 48000, num_frames: 1 }).unwrap();
        assert_eq!(o, vec![4.0, 6.0]);
    }
}
