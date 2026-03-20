// ============================================================================
// Band Merge Plugin
// ============================================================================

use serde::{Deserialize, Serialize};
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};

/// Maximum number of bands supported.
const MAX_BANDS: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandMergePluginParams {
    #[serde(default = "default_num_bands")]
    pub bands: usize,
    /// Per-band gain in dB. Defaults to 0.0 (unity) for each band.
    #[serde(default)]
    pub band_gains_db: Vec<f32>,
    /// Per-band mute flags. Defaults to false (unmuted) for each band.
    #[serde(default)]
    pub band_mutes: Vec<bool>,
}

fn default_num_bands() -> usize {
    2
}

pub struct BandMergePlugin {
    output_channels: usize,
    num_bands: usize,
    param_bands: ParameterId,
    /// Per-band gain in dB (up to MAX_BANDS).
    band_gains_db: [f32; MAX_BANDS],
    /// Per-band linear gain (precomputed from dB).
    band_gains_linear: [f32; MAX_BANDS],
    /// Per-band mute toggle.
    band_mutes: [bool; MAX_BANDS],
    cached_parameters: Vec<Parameter>,
}

impl BandMergePlugin {
    pub fn new(output_channels: usize, bands: usize) -> Result<Self, String> {
        if bands < 2 {
            return Err("Min 2 bands".into());
        }
        if bands > MAX_BANDS {
            return Err(format!("Max {} bands", MAX_BANDS));
        }
        let mut p = Self {
            output_channels,
            num_bands: bands,
            param_bands: ParameterId("bands".to_string()),
            band_gains_db: [0.0; MAX_BANDS],
            band_gains_linear: [1.0; MAX_BANDS],
            band_mutes: [false; MAX_BANDS],
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    pub fn from_params(
        output_channels: usize,
        params: &BandMergePluginParams,
    ) -> Result<Self, String> {
        let mut p = Self::new(output_channels, params.bands)?;
        for (i, &g) in params.band_gains_db.iter().enumerate().take(params.bands) {
            p.band_gains_db[i] = g;
            p.band_gains_linear[i] = db_to_linear(g);
        }
        for (i, &m) in params.band_mutes.iter().enumerate().take(params.bands) {
            p.band_mutes[i] = m;
        }
        p.rebuild_cached_parameters();
        Ok(p)
    }

    fn rebuild_cached_parameters(&mut self) {
        let mut params = vec![Parameter::new_int(
            "bands",
            "Bands",
            self.num_bands as i32,
            2,
            MAX_BANDS as i32,
        )];
        for i in 0..self.num_bands {
            let gain_id = format!("band_{}_gain_db", i);
            let gain_label = format!("Band {} Gain (dB)", i);
            params.push(Parameter::new_float(
                &gain_id,
                &gain_label,
                self.band_gains_db[i],
                -60.0,
                24.0,
            ));
            let mute_id = format!("band_{}_mute", i);
            let mute_label = format!("Band {} Mute", i);
            params.push(Parameter::new_bool(&mute_id, &mute_label, self.band_mutes[i]));
        }
        self.cached_parameters = params;
    }
}

#[inline]
fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

impl Plugin for BandMergePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("BandMerge", "1.1.0", "Sotf")
    }
    fn input_channels(&self) -> usize {
        self.output_channels * self.num_bands
    }
    fn output_channels(&self) -> usize {
        self.output_channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_bands {
            let v = value.as_int().ok_or("val")? as usize;
            if !(2..=MAX_BANDS).contains(&v) {
                return Err(format!("bands must be 2..={}", MAX_BANDS));
            }
            self.num_bands = v;
            self.rebuild_cached_parameters();
            return Ok(());
        }
        // Check per-band gain parameters
        for i in 0..self.num_bands {
            let gain_id = format!("band_{}_gain_db", i);
            if id.0 == gain_id {
                let v = value
                    .as_float()
                    .ok_or_else(|| format!("{} must be a float", gain_id))?;
                if v.is_finite() {
                    self.band_gains_db[i] = v;
                    self.band_gains_linear[i] = db_to_linear(v);
                    self.rebuild_cached_parameters();
                }
                return Ok(());
            }
            let mute_id = format!("band_{}_mute", i);
            if id.0 == mute_id {
                let v = value
                    .as_bool()
                    .ok_or_else(|| format!("{} must be a bool", mute_id))?;
                self.band_mutes[i] = v;
                self.rebuild_cached_parameters();
                return Ok(());
            }
        }
        Err(format!("Unknown parameter: {}", id))
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_bands {
            return Some(ParameterValue::Int(self.num_bands as i32));
        }
        for i in 0..self.num_bands {
            if id.0 == format!("band_{}_gain_db", i) {
                return Some(ParameterValue::Float(self.band_gains_db[i]));
            }
            if id.0 == format!("band_{}_mute", i) {
                return Some(ParameterValue::Bool(self.band_mutes[i]));
            }
        }
        None
    }
    fn initialize(&mut self, _sample_rate: u32) -> PluginResult<()> {
        Ok(())
    }
    fn reset(&mut self) {}

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
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
                    if !self.band_mutes[band] {
                        sum += input[in_off + band * out_ch + ch] * self.band_gains_linear[band];
                    }
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
    use crate::*;

    #[test]
    fn test_band_merge_basic() {
        let mut p = BandMergePlugin::new(2, 2).unwrap();
        let i = vec![1.0, 2.0, 3.0, 4.0];
        let mut o = vec![0.0, 0.0];
        p.process(
            &i,
            &mut o,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1,
            },
        )
        .unwrap();
        assert_eq!(o, vec![4.0, 6.0]);
    }

    #[test]
    fn test_band_merge_with_gain() {
        let mut p = BandMergePlugin::new(1, 2).unwrap();
        // Set band 0 gain to +6 dB (~2x), band 1 gain stays at 0 dB (1x)
        p.set_parameter(
            ParameterId::from("band_0_gain_db"),
            ParameterValue::Float(6.0206),
        )
        .unwrap();

        // Band 0: 1.0, Band 1: 1.0
        let i = vec![1.0, 1.0];
        let mut o = vec![0.0];
        p.process(
            &i,
            &mut o,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1,
            },
        )
        .unwrap();
        // Band 0 * 2.0 + Band 1 * 1.0 = 3.0
        assert!((o[0] - 3.0).abs() < 0.01, "got {}", o[0]);
    }

    #[test]
    fn test_band_merge_with_mute() {
        let mut p = BandMergePlugin::new(2, 2).unwrap();
        // Mute band 1
        p.set_parameter(
            ParameterId::from("band_1_mute"),
            ParameterValue::Bool(true),
        )
        .unwrap();

        let i = vec![1.0, 2.0, 3.0, 4.0];
        let mut o = vec![0.0, 0.0];
        p.process(
            &i,
            &mut o,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1,
            },
        )
        .unwrap();
        // Only band 0 contributes: [1.0, 2.0]
        assert_eq!(o, vec![1.0, 2.0]);
    }

    #[test]
    fn test_band_merge_mute_and_gain_combined() {
        let mut p = BandMergePlugin::new(1, 3).unwrap();
        // Mute band 0
        p.set_parameter(
            ParameterId::from("band_0_mute"),
            ParameterValue::Bool(true),
        )
        .unwrap();
        // Set band 2 gain to +6 dB (~2x)
        p.set_parameter(
            ParameterId::from("band_2_gain_db"),
            ParameterValue::Float(6.0206),
        )
        .unwrap();

        // 3 bands, 1 channel: band0=10.0, band1=1.0, band2=1.0
        let i = vec![10.0, 1.0, 1.0];
        let mut o = vec![0.0];
        p.process(
            &i,
            &mut o,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1,
            },
        )
        .unwrap();
        // band0 muted, band1 * 1.0 + band2 * 2.0 = 3.0
        assert!((o[0] - 3.0).abs() < 0.01, "got {}", o[0]);
    }

    #[test]
    fn test_band_merge_get_set_parameters() {
        let mut p = BandMergePlugin::new(2, 3).unwrap();

        // Default gain is 0.0
        assert_eq!(
            p.get_parameter(&ParameterId::from("band_0_gain_db")),
            Some(ParameterValue::Float(0.0))
        );
        // Default mute is false
        assert_eq!(
            p.get_parameter(&ParameterId::from("band_1_mute")),
            Some(ParameterValue::Bool(false))
        );

        // Set and retrieve
        p.set_parameter(
            ParameterId::from("band_2_gain_db"),
            ParameterValue::Float(-3.0),
        )
        .unwrap();
        assert_eq!(
            p.get_parameter(&ParameterId::from("band_2_gain_db")),
            Some(ParameterValue::Float(-3.0))
        );

        p.set_parameter(
            ParameterId::from("band_0_mute"),
            ParameterValue::Bool(true),
        )
        .unwrap();
        assert_eq!(
            p.get_parameter(&ParameterId::from("band_0_mute")),
            Some(ParameterValue::Bool(true))
        );
    }

    #[test]
    fn test_band_merge_from_params() {
        let params = BandMergePluginParams {
            bands: 3,
            band_gains_db: vec![6.0206, 0.0, -60.0],
            band_mutes: vec![false, true, false],
        };
        let mut p = BandMergePlugin::from_params(1, &params).unwrap();

        // band0=1.0 * ~2.0, band1=1.0 muted, band2=1.0 * ~0.001
        let i = vec![1.0, 1.0, 1.0];
        let mut o = vec![0.0];
        p.process(
            &i,
            &mut o,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1,
            },
        )
        .unwrap();
        // ~2.0 + 0 + ~0.001 ≈ 2.001
        assert!((o[0] - 2.0).abs() < 0.05, "got {}", o[0]);
    }

    #[test]
    fn test_band_merge_parameters_list() {
        let p = BandMergePlugin::new(2, 3).unwrap();
        let params = p.parameters();
        // 1 (bands) + 3 * 2 (gain + mute per band) = 7
        assert_eq!(params.len(), 7);
    }
}
