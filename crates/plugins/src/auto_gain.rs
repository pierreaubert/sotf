// ============================================================================
// Auto Gain Compensation
// ============================================================================

use crate::analyzer_loudness_monitor::LoudnessMonitor;
use crate::smoothing::Smoother;
use crate::simd::enable_ftz_daz;
use math_audio_dsp::fast_math::fast_pow10;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AutoGainLoudnessType {
    #[default] Momentary, ShortTerm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoGainParams {
    #[serde(default = "default_enabled")] pub enabled: bool,
    #[serde(default)] pub loudness_type: AutoGainLoudnessType,
    #[serde(default = "default_max_gain_db")] pub max_gain_db: f32,
    #[serde(default = "default_smoothing_ms")] pub smoothing_ms: f32,
}

fn default_enabled() -> bool { false }
fn default_max_gain_db() -> f32 { 12.0 }
fn default_smoothing_ms() -> f32 { 100.0 }

impl Default for AutoGainParams {
    fn default() -> Self {
        Self { enabled: false, loudness_type: AutoGainLoudnessType::Momentary, max_gain_db: 12.0, smoothing_ms: 100.0 }
    }
}

pub struct AutoGain {
    num_channels: usize, sample_rate: u32, input_monitor: LoudnessMonitor,
    output_monitor: LoudnessMonitor, gain_smoother: Smoother, current_gain_db: f32,
    last_input_lufs: f64, last_output_lufs: f64, last_input_peak: f64, last_output_peak: f64,
    enabled: bool, loudness_type: AutoGainLoudnessType, max_gain_db: f32, smoothing_ms: f32,
}

impl std::fmt::Debug for AutoGain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutoGain").field("enabled", &self.enabled).field("gain", &self.current_gain_db).finish_non_exhaustive()
    }
}

impl AutoGain {
    pub fn new(num_channels: usize, sample_rate: u32, params: AutoGainParams) -> Result<Self, String> {
        Ok(Self {
            num_channels, sample_rate,
            input_monitor: LoudnessMonitor::new(num_channels as u32, sample_rate)?,
            output_monitor: LoudnessMonitor::new(num_channels as u32, sample_rate)?,
            gain_smoother: Smoother::new(0.0, params.smoothing_ms, sample_rate),
            current_gain_db: 0.0, last_input_lufs: f64::NEG_INFINITY, last_output_lufs: f64::NEG_INFINITY,
            last_input_peak: 0.0, last_output_peak: 0.0,
            enabled: params.enabled, loudness_type: params.loudness_type,
            max_gain_db: params.max_gain_db, smoothing_ms: params.smoothing_ms,
        })
    }

    pub fn new_default(num_channels: usize, sample_rate: u32) -> Result<Self, String> { Self::new(num_channels, sample_rate, Default::default()) }

    pub fn set_sample_rate(&mut self, sr: u32) -> Result<(), String> {
        self.sample_rate = sr;
        self.input_monitor = LoudnessMonitor::new(self.num_channels as u32, sr)?;
        self.output_monitor = LoudnessMonitor::new(self.num_channels as u32, sr)?;
        self.gain_smoother.set_time(self.smoothing_ms, sr);
        Ok(())
    }

    pub fn reset(&mut self) {
        let _ = self.input_monitor.reset(); let _ = self.output_monitor.reset();
        self.gain_smoother.reset(0.0); self.current_gain_db = 0.0;
        self.last_input_lufs = f64::NEG_INFINITY; self.last_output_lufs = f64::NEG_INFINITY;
        self.last_input_peak = 0.0; self.last_output_peak = 0.0;
    }

    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; if !e { self.gain_smoother.set_target(0.0); } }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_max_gain_db(&mut self, m: f32) { self.max_gain_db = m.abs(); }
    pub fn set_smoothing_ms(&mut self, s: f32) { self.smoothing_ms = s; self.gain_smoother.set_time(s, self.sample_rate); }
    pub fn set_loudness_type(&mut self, t: AutoGainLoudnessType) { self.loudness_type = t; }

    pub fn measure_input(&mut self, input: &[f32]) -> Result<(), String> {
        if !self.enabled { return Ok(()); }
        self.input_monitor.add_frames(input)?;
        let info = self.input_monitor.get_loudness();
        self.last_input_lufs = if self.loudness_type == AutoGainLoudnessType::Momentary { info.momentary_lufs } else { info.shortterm_lufs };
        self.last_input_peak = info.peak;
        Ok(())
    }

    pub fn measure_output(&mut self, output: &[f32]) -> Result<(), String> {
        if !self.enabled { return Ok(()); }
        self.output_monitor.add_frames(output)?;
        let info = self.output_monitor.get_loudness();
        self.last_output_lufs = if self.loudness_type == AutoGainLoudnessType::Momentary { info.momentary_lufs } else { info.shortterm_lufs };
        self.last_output_peak = info.peak;
        if self.last_input_lufs.is_finite() && self.last_output_lufs.is_finite() {
            let target = (self.last_input_lufs - self.last_output_lufs) as f32;
            self.gain_smoother.set_target(target.clamp(-self.max_gain_db, self.max_gain_db));
        }
        Ok(())
    }

    #[inline]
    pub fn next_gain_linear(&mut self) -> f32 {
        if !self.enabled { return 1.0; }
        self.current_gain_db = self.gain_smoother.next();
        fast_pow10(self.current_gain_db / 20.0)
    }

    pub fn current_gain_db(&self) -> f32 { if !self.enabled { 0.0 } else { self.gain_smoother.current() } }
    pub fn last_input_lufs(&self) -> f64 { self.last_input_lufs }
    pub fn last_output_lufs(&self) -> f64 { self.last_output_lufs }
    pub fn last_input_peak(&self) -> f64 { self.last_input_peak }
    pub fn last_output_peak(&self) -> f64 { self.last_output_peak }

    pub fn apply_compensation(&mut self, output: &mut [f32], num_frames: usize) {
        if !self.enabled { return; }
        enable_ftz_daz();
        for frame in 0..num_frames {
            let gain = self.next_gain_linear();
            for ch in 0..self.num_channels {
                output[frame * self.num_channels + ch] *= gain;
            }
        }
    }

    pub fn get_data(&self) -> AutoGainData {
        AutoGainData {
            enabled: self.enabled, gain_db: self.current_gain_db,
            input_lufs: self.last_input_lufs, output_lufs: self.last_output_lufs,
            input_peak: self.last_input_peak, output_peak: self.last_output_peak,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoGainData {
    pub enabled: bool, pub gain_db: f32, pub input_lufs: f64,
    pub output_lufs: f64, pub input_peak: f64, pub output_peak: f64,
}
