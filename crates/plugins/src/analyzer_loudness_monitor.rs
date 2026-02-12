// ============================================================================
// Loudness Monitor Analyzer Plugin
// ============================================================================

use super::analyzer::LoudnessData;
use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use ebur128::{EbuR128, Mode};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::Arc;
use parking_lot::Mutex;
use rtrb::{RingBuffer, Consumer};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoudnessInfo {
    pub momentary_lufs: f64, pub shortterm_lufs: f64, pub integrated_lufs: f64, pub peak: f64,
}

pub struct LoudnessMonitor {
    ebur128: EbuR128,
}

impl LoudnessMonitor {
    pub fn new(channels: u32, sr: u32) -> Result<Self, String> {
        let ebur = EbuR128::new(channels, sr, Mode::M | Mode::S | Mode::I | Mode::SAMPLE_PEAK)
            .map_err(|e| format!("{:?}", e))?;
        Ok(Self { ebur128: ebur })
    }
    pub fn add_frames(&mut self, samples: &[f32]) -> Result<(), String> {
        self.ebur128.add_frames_f32(samples).map_err(|_| "EBU".into())
    }
    pub fn get_loudness(&self) -> LoudnessData {
        let mut d = LoudnessData::new(self.ebur128.channels() as usize);
        d.momentary_lufs = self.ebur128.loudness_momentary().unwrap_or(-120.0);
        d.shortterm_lufs = self.ebur128.loudness_shortterm().unwrap_or(-120.0);
        d.integrated_lufs = self.ebur128.loudness_global().unwrap_or(-120.0);
        for ch in 0..self.ebur128.channels() {
            d.channel_peaks[ch as usize] = self.ebur128.sample_peak(ch).unwrap_or(0.0);
        }
        d.peak = d.channel_peaks.iter().copied().fold(0.0, f64::max);
        d
    }
    pub fn reset(&mut self) -> Result<(), String> {
        self.ebur128.reset();
        Ok(())
    }
}

pub struct LoudnessMonitorPlugin {
    num_channels: usize, sample_rate: u32,
    producer: rtrb::Producer<f32>, consumer: Arc<Mutex<Consumer<f32>>>,
    shared_data: Arc<Mutex<LoudnessData>>,
    monitor: Arc<Mutex<LoudnessMonitor>>,
}

impl LoudnessMonitorPlugin {
    pub fn new(num_channels: usize) -> Result<Self, String> {
        let sr = 48000;
        let (p, c) = RingBuffer::new(sr as usize * 2);
        let monitor = LoudnessMonitor::new(num_channels as u32, sr)?;
        let shared_data = Arc::new(Mutex::new(LoudnessData::new(num_channels)));
        Ok(Self {
            num_channels, sample_rate: sr, producer: p, consumer: Arc::new(Mutex::new(c)),
            shared_data, monitor: Arc::new(Mutex::new(monitor)),
        })
    }
}

impl Plugin for LoudnessMonitorPlugin {
    fn info(&self) -> PluginInfo { PluginInfo::new("Loudness Monitor", "1.1.0", "Sotf") }
    fn input_channels(&self) -> usize { self.num_channels }
    fn output_channels(&self) -> usize { self.num_channels }
    fn parameters(&self) -> Vec<Parameter> { Vec::new() }
    fn set_parameter(&mut self, _: ParameterId, _: ParameterValue) -> PluginResult<()> { Ok(()) }
    fn get_parameter(&self, _: &ParameterId) -> Option<ParameterValue> { None }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        let mut m = self.monitor.lock();
        *m = LoudnessMonitor::new(self.num_channels as u32, sr)?;
        Ok(())
    }
    fn reset(&mut self) {
        let mut m = self.monitor.lock(); let _ = m.reset();
        let mut d = self.shared_data.lock(); *d = LoudnessData::new(self.num_channels);
    }
    fn process(&mut self, input: &[f32], output: &mut [f32], context: &ProcessContext) -> Result<usize, String> {
        output.copy_from_slice(input);
        for &s in input { let _ = self.producer.push(s); }
        let mut consumer = self.consumer.lock();
        let slots = consumer.slots();
        if let Ok(chunk) = consumer.read_chunk(slots) {
            let mut m = self.monitor.lock();
            let (s1, s2) = chunk.as_slices();
            let _ = m.add_frames(s1); let _ = m.add_frames(s2);
            chunk.commit_all();
            let mut d = self.shared_data.lock();
            *d = m.get_loudness();
        }
        Ok(context.num_frames)
    }
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        let d = self.shared_data.lock(); Some(Arc::new(d.clone()))
    }
}
