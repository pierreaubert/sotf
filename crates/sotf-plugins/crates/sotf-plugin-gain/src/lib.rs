// ============================================================================
// Gain Plugin - Simple gain control with per-channel support
// ============================================================================

pub mod params;

use sotf_host::param_specs::find_by_key as pk;
use crate::params::PARAMS as GN;
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
    param_smoothing_ms: ParameterId,
    cached_gains: Vec<f32>,
    smoothing_ms: f32,
    cached_parameters: Vec<Parameter>,
}

impl GainPlugin {
    pub fn new(channels: usize, gain_db: f32) -> Self {
        Self::with_smoothing(channels, gain_db, 20.0)
    }

    pub fn with_smoothing(channels: usize, gain_db: f32, smoothing_ms: f32) -> Self {
        // Placeholder rate; real rate is set in initialize()
        let sr = 48000;
        let gain_linear = Self::db_to_linear(gain_db);
        let mut p = Self {
            channels,
            sample_rate: sr,
            global_gain_db: gain_db,
            global_gain_smoother: Smoother::new(gain_linear, smoothing_ms, sr),
            channel_gains_db: Vec::with_capacity(channels),
            channel_gains_smoothers: Vec::with_capacity(channels),
            param_gain_db: ParameterId::from("gain_db"),
            param_smoothing_ms: ParameterId::from("smoothing_ms"),
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
        // Placeholder rate; real rate is set in initialize()
        let sr = 48000;
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
            param_smoothing_ms: ParameterId::from("smoothing_ms"),
            cached_gains: vec![0.0; channels],
            smoothing_ms: 20.0,
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    fn rebuild_cached_parameters(&mut self) {
        let mut params = vec![
            Parameter::new_float(
                "gain_db",
                "Gain",
                self.global_gain_db,
                pk(GN, "gain_db").min_f64() as f32,
                pk(GN, "gain_db").max_f64() as f32,
            ),
            Parameter::new_float("smoothing_ms", "Smoothing", self.smoothing_ms, 0.0, 200.0),
        ];

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

        if id == self.param_smoothing_ms {
            Parameter::new_float("smoothing_ms", "Smoothing", 20.0, 0.0, 200.0).validate(&val)?;
            if let Some(v) = val.as_float()
                && v.is_finite()
            {
                self.smoothing_ms = v.clamp(0.0, 200.0);
                self.global_gain_smoother
                    .set_time(self.smoothing_ms, self.sample_rate);
                for s in &mut self.channel_gains_smoothers {
                    s.set_time(self.smoothing_ms, self.sample_rate);
                }
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
        } else if id == &self.param_smoothing_ms {
            Some(ParameterValue::Float(self.smoothing_ms))
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
        } else if let Some(ramp) = context.ramps.iter().find(|r| r.param_index == 0) {
            // Sample-accurate automation: interpolate gain_db from ramp,
            // convert to linear per sample. Overrides the smoother.
            for frame in 0..nf {
                let t = frame as f32 / nf.max(1) as f32;
                let gain_db = ramp.start_value + (ramp.end_value - ramp.start_value) * t;
                let g = Self::db_to_linear(gain_db);
                let off = frame * self.channels;
                apply_gain_simd(&mut buffer[off..off + self.channels], g);
            }
            // Sync smoother to end value so it's correct if ramp stops
            let end_linear = Self::db_to_linear(ramp.end_value);
            self.global_gain_smoother.reset(end_linear);
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
            &ProcessContext::new(44100, 2,),
        )
        .unwrap();
        assert!((b[0] - 1.0).abs() < 1e-5);
    }

    /// Sample rate deferred initialization: create gain plugin, call
    /// initialize(96000), then verify smoothers respond correctly at the
    /// new rate (a gain change converges within expected time).
    #[test]
    fn test_sample_rate_deferred_initialization() {
        let mut p = GainPlugin::with_smoothing(1, 0.0, 20.0);
        // Initialize at 96000 Hz
        p.initialize(96000).unwrap();

        // Set a new gain target
        p.set_gain_db(-6.0);
        let target_linear = GainPlugin::db_to_linear(-6.0);

        // At 96000 Hz with 20ms smoothing, we need ~5*tau = ~100ms = 9600 samples
        // to converge. Process 200ms worth of samples to be safe.
        let num_frames = 19200; // 200ms at 96kHz
        let mut buf = vec![1.0f32; num_frames];
        p.process_in_place(
            &mut buf,
            &ProcessContext::new(96000, num_frames),
        )
        .unwrap();

        // After 200ms, the smoother should have converged to the target gain
        let last_sample = buf[num_frames - 1];
        assert!(
            (last_sample - target_linear).abs() < 0.01,
            "After 200ms at 96kHz, gain should converge to {target_linear:.4}, got {last_sample:.4}"
        );

        // Verify it didn't converge too fast (after only 1ms = 96 samples)
        // by checking the output wasn't already at target near the beginning
        let early_sample = buf[96]; // ~1ms
        let diff_from_target = (early_sample - target_linear).abs();
        assert!(
            diff_from_target > 0.01,
            "After only 1ms, gain should still be transitioning (diff={diff_from_target:.4})"
        );
    }
}
