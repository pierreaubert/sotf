// ============================================================================
// Gain Plugin - Simple gain control with per-channel support
// ============================================================================

use sotf_host::param_specs::{find_by_key as pk, gain::PARAMS as GN};
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{apply_gain_simd, apply_per_channel_gain_simd};
use sotf_host::smoothing::Smoother;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GainPluginParams {
    #[serde(default = "default_gain_db")]
    pub gain_db: f32,
    #[serde(default)]
    pub channel_gains: Vec<f32>,
}

fn default_gain_db() -> f32 {
    pk(GN, "gain_db").default_f64() as f32
}

pub struct GainPlugin {
    channels: usize,
    sample_rate: u32,
    global_gain_db: f32,
    global_gain_smoother: Smoother,
    channel_gains_db: Vec<f32>,
    channel_gains_smoothers: Vec<Smoother>,
    param_gain_db: ParameterId,
    cached_gains: Vec<f32>,
    smoothing_ms: f32,
    cached_parameters: Vec<Parameter>,
}

impl GainPlugin {
    pub fn new(channels: usize, gain_db: f32) -> Self {
        Self::with_smoothing(channels, gain_db, 20.0)
    }

    pub fn with_smoothing(channels: usize, gain_db: f32, smoothing_ms: f32) -> Self {
        let sr = 44100;
        let gain_linear = Self::db_to_linear(gain_db);
        let mut p = Self {
            channels,
            sample_rate: sr,
            global_gain_db: gain_db,
            global_gain_smoother: Smoother::new(gain_linear, smoothing_ms, sr),
            channel_gains_db: Vec::with_capacity(channels),
            channel_gains_smoothers: Vec::with_capacity(channels),
            param_gain_db: ParameterId::from("gain_db"),
            cached_gains: vec![0.0; channels],
            smoothing_ms,
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        p
    }

    pub fn new_per_channel(channel_gains: Vec<f32>) -> Result<Self, String> {
        if channel_gains.is_empty() {
            return Err("Empty".into());
        }
        let channels = channel_gains.len();
        let sr = 44100;
        let cgs: Vec<Smoother> = channel_gains
            .iter()
            .map(|&db| Smoother::new(Self::db_to_linear(db), 20.0, sr))
            .collect();
        let mut p = Self {
            channels,
            sample_rate: sr,
            global_gain_db: 0.0,
            global_gain_smoother: Smoother::new(1.0, 20.0, sr),
            channel_gains_db: channel_gains,
            channel_gains_smoothers: cgs,
            param_gain_db: ParameterId::from("gain_db"),
            cached_gains: vec![0.0; channels],
            smoothing_ms: 20.0,
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    fn rebuild_cached_parameters(&mut self) {
        let mut params = vec![Parameter::new_float(
            "gain_db",
            "Gain",
            self.global_gain_db,
            pk(GN, "gain_db").min_f64() as f32,
            pk(GN, "gain_db").max_f64() as f32,
        )];

        if self.is_per_channel() {
            for ch in 0..self.channels {
                params.push(Parameter::new_float(
                    &format!("gain_db_{}", ch),
                    &format!("Gain Ch {}", ch + 1),
                    self.channel_gains_db[ch],
                    pk(GN, "gain_db").min_f64() as f32,
                    pk(GN, "gain_db").max_f64() as f32,
                ));
            }
        }

        self.cached_parameters = params;
    }

    pub fn from_params(channels: usize, params: GainPluginParams) -> Result<Self, String> {
        if params.channel_gains.is_empty() {
            Ok(Self::new(channels, params.gain_db))
        } else {
            if params.channel_gains.len() != channels {
                return Err("Mismatch".into());
            }
            Self::new_per_channel(params.channel_gains)
        }
    }

    pub fn is_per_channel(&self) -> bool {
        !self.channel_gains_db.is_empty()
    }
    pub fn set_gain_db(&mut self, db: f32) {
        self.global_gain_db = db;
        self.global_gain_smoother.set_target(Self::db_to_linear(db));
        self.channel_gains_db.clear();
        self.channel_gains_smoothers.clear();
    }
    pub fn set_gain_linear(&mut self, g: f32) {
        self.global_gain_smoother.set_target(g);
        self.global_gain_db = Self::linear_to_db(g);
        self.channel_gains_db.clear();
        self.channel_gains_smoothers.clear();
    }
    pub fn set_channel_gains(&mut self, dbs: Vec<f32>) -> Result<(), String> {
        if dbs.len() != self.channels {
            return Err("Mismatch".into());
        }
        self.channel_gains_smoothers = dbs
            .iter()
            .map(|&db| Smoother::new(Self::db_to_linear(db), self.smoothing_ms, self.sample_rate))
            .collect();
        self.channel_gains_db = dbs;
        Ok(())
    }
    pub fn set_channel_gain_db(&mut self, ch: usize, db: f32) -> Result<(), String> {
        if ch >= self.channels {
            return Err("OOB".into());
        }
        if self.channel_gains_db.is_empty() {
            self.channel_gains_db = vec![self.global_gain_db; self.channels];
            self.channel_gains_smoothers = vec![
                Smoother::new(
                    self.global_gain_smoother.current(),
                    self.smoothing_ms,
                    self.sample_rate
                );
                self.channels
            ];
        }
        self.channel_gains_db[ch] = db;
        self.channel_gains_smoothers[ch].set_target(Self::db_to_linear(db));
        Ok(())
    }
    pub fn gain_db(&self) -> f32 {
        self.global_gain_db
    }
    pub fn gain_linear(&self) -> f32 {
        self.global_gain_smoother.current()
    }
    pub fn channel_gain_db(&self, ch: usize) -> Option<f32> {
        if self.is_per_channel() {
            self.channel_gains_db.get(ch).copied()
        } else if ch < self.channels {
            Some(self.global_gain_db)
        } else {
            None
        }
    }
    #[inline]
    fn db_to_linear(db: f32) -> f32 {
        10.0_f32.powf(db / 20.0)
    }
    fn linear_to_db(l: f32) -> f32 {
        20.0 * l.log10()
    }
}

impl InPlacePlugin for GainPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Gain", "1.2.0", "Sotf")
    }
    fn channels(&self) -> usize {
        self.channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, val: ParameterValue) -> PluginResult<()> {
        // Validate against parameter definitions
        // For per-channel gains, we might need a template since they are dynamic
        if id == self.param_gain_db {
            Parameter::new_float("gain_db", "Gain", 0.0, -100.0, 24.0).validate(&val)?;
            if let Some(v) = val.as_float()
                && v.is_finite()
            {
                self.set_gain_db(v);
                self.rebuild_cached_parameters();
                return Ok(());
            }
        }

        if let Some(s) = id.as_str().strip_prefix("gain_db_")
            && let Ok(ch) = s.parse::<usize>()
        {
            Parameter::new_float("gain_db_ch", "Gain Ch", 0.0, -100.0, 24.0).validate(&val)?;
            if let Some(v) = val.as_float()
                && v.is_finite()
            {
                self.set_channel_gain_db(ch, v)?;
                self.rebuild_cached_parameters();
                return Ok(());
            }
        }
        Err(format!("Invalid or unknown parameter: {}", id))
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_gain_db {
            Some(ParameterValue::Float(self.global_gain_db))
        } else {
            id.as_str()
                .strip_prefix("gain_db_")
                .and_then(|s| s.parse::<usize>().ok())
                .and_then(|ch| self.channel_gain_db(ch))
                .map(ParameterValue::Float)
        }
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        self.global_gain_smoother.set_time(self.smoothing_ms, sr);
        for s in &mut self.channel_gains_smoothers {
            s.set_time(self.smoothing_ms, sr);
        }
        Ok(())
    }
    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        let nf = context.num_frames;
        if self.is_per_channel() {
            for frame in 0..nf {
                for ch in 0..self.channels {
                    self.cached_gains[ch] = self.channel_gains_smoothers[ch].advance();
                }
                let off = frame * self.channels;
                apply_per_channel_gain_simd(
                    &mut buffer[off..off + self.channels],
                    self.channels,
                    &self.cached_gains,
                );
            }
        } else {
            for frame in 0..nf {
                let g = self.global_gain_smoother.advance();
                let off = frame * self.channels;
                apply_gain_simd(&mut buffer[off..off + self.channels], g);
            }
        }
        Ok(nf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_unity_gain() {
        let mut p = GainPlugin::new(2, 0.0);
        let mut b = vec![1.0, 2.0, 3.0, 4.0];
        p.process_in_place(
            &mut b,
            &ProcessContext {
                sample_rate: 44100,
                num_frames: 2,
            },
        )
        .unwrap();
        assert!((b[0] - 1.0).abs() < 1e-5);
    }
}
