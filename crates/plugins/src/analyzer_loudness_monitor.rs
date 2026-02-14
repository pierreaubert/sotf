// ============================================================================
// Loudness Monitor Analyzer Plugin
// ============================================================================

use super::analyzer::LoudnessData;
use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use arc_swap::ArcSwap;
use ebur128::{EbuR128, Mode};
use rtrb::{Consumer, RingBuffer};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoudnessInfo {
    pub momentary_lufs: f64,
    pub shortterm_lufs: f64,
    pub integrated_lufs: f64,
    pub peak: f64,
}

pub struct LoudnessMonitor {
    ebur128: EbuR128,
    channels: u32,
    /// Running L/R correlation (Pearson) for stereo width.
    /// Smoothed with EMA to avoid jitter.
    correlation_lr: Option<f64>,
}

impl LoudnessMonitor {
    pub fn new(channels: u32, sr: u32) -> Result<Self, String> {
        let ebur = EbuR128::new(
            channels,
            sr,
            Mode::M | Mode::S | Mode::I | Mode::SAMPLE_PEAK | Mode::TRUE_PEAK,
        )
        .map_err(|e| format!("{:?}", e))?;
        Ok(Self {
            ebur128: ebur,
            channels,
            correlation_lr: None,
        })
    }

    pub fn add_frames(&mut self, samples: &[f32]) -> Result<(), String> {
        // Compute L/R correlation for stereo signals
        if self.channels == 2 {
            let frame_corr = compute_correlation_interleaved(samples, 2);
            if let Some(c) = frame_corr {
                // EMA smoothing: ~100ms at typical frame rates
                const ALPHA: f64 = 0.15;
                self.correlation_lr = Some(match self.correlation_lr {
                    Some(prev) => prev * (1.0 - ALPHA) + c * ALPHA,
                    None => c,
                });
            }
        }

        self.ebur128
            .add_frames_f32(samples)
            .map_err(|_| "EBU".into())
    }

    pub fn get_loudness(&self) -> LoudnessData {
        let nc = self.channels as usize;
        let mut d = LoudnessData::new(nc);
        d.momentary_lufs = self.ebur128.loudness_momentary().unwrap_or(-120.0);
        d.shortterm_lufs = self.ebur128.loudness_shortterm().unwrap_or(-120.0);
        d.integrated_lufs = self.ebur128.loudness_global().unwrap_or(-120.0);
        for ch in 0..self.channels {
            // prev_sample_peak: peak from last add_frames() call (real-time metering)
            d.channel_peaks[ch as usize] = self.ebur128.prev_sample_peak(ch).unwrap_or(0.0);
            // prev_true_peak: true peak from last add_frames() (accounts for inter-sample peaks)
            let tp_linear = self.ebur128.prev_true_peak(ch).unwrap_or(0.0);
            d.true_peaks_dbtp[ch as usize] = if tp_linear > 0.0 {
                20.0 * tp_linear.log10()
            } else {
                f64::NEG_INFINITY
            };
        }
        d.peak = d.channel_peaks.iter().copied().fold(0.0, f64::max);
        d.correlation_lr = self.correlation_lr;
        d
    }

    pub fn reset(&mut self) -> Result<(), String> {
        self.ebur128.reset();
        self.correlation_lr = None;
        Ok(())
    }
}

/// Compute Pearson correlation between channel 0 and channel 1 from interleaved samples.
/// Returns None if fewer than 2 frames or zero variance.
fn compute_correlation_interleaved(samples: &[f32], channels: usize) -> Option<f64> {
    if channels < 2 {
        return None;
    }
    let num_frames = samples.len() / channels;
    if num_frames < 2 {
        return None;
    }

    let mut sum_lr: f64 = 0.0;
    let mut sum_l2: f64 = 0.0;
    let mut sum_r2: f64 = 0.0;

    for i in 0..num_frames {
        let l = samples[i * channels] as f64;
        let r = samples[i * channels + 1] as f64;
        sum_lr += l * r;
        sum_l2 += l * l;
        sum_r2 += r * r;
    }

    let denom = (sum_l2 * sum_r2).sqrt();
    if denom < 1e-12 {
        return None; // silence or zero variance
    }
    Some((sum_lr / denom).clamp(-1.0, 1.0))
}

pub struct LoudnessMonitorPlugin {
    num_channels: usize,
    sample_rate: u32,
    producer: rtrb::Producer<f32>,
    consumer: Consumer<f32>,
    shared_data: Arc<ArcSwap<LoudnessData>>,
    monitor: LoudnessMonitor,
}

impl LoudnessMonitorPlugin {
    pub fn new(num_channels: usize) -> Result<Self, String> {
        let sr = 48000;
        let (p, c) = RingBuffer::new(sr as usize * 2);
        let monitor = LoudnessMonitor::new(num_channels as u32, sr)?;
        let shared_data = Arc::new(ArcSwap::from_pointee(LoudnessData::new(num_channels)));
        Ok(Self {
            num_channels,
            sample_rate: sr,
            producer: p,
            consumer: c,
            shared_data,
            monitor,
        })
    }
}

impl Plugin for LoudnessMonitorPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Loudness Monitor", "1.1.0", "Sotf")
    }
    fn input_channels(&self) -> usize {
        self.num_channels
    }
    fn output_channels(&self) -> usize {
        self.num_channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        Vec::new()
    }
    fn set_parameter(&mut self, _: ParameterId, _: ParameterValue) -> PluginResult<()> {
        Ok(())
    }
    fn get_parameter(&self, _: &ParameterId) -> Option<ParameterValue> {
        None
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        self.monitor = LoudnessMonitor::new(self.num_channels as u32, sr)?;
        Ok(())
    }
    fn reset(&mut self) {
        let _ = self.monitor.reset();
        self.shared_data
            .store(Arc::new(LoudnessData::new(self.num_channels)));
    }
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        output.copy_from_slice(input);
        for &s in input {
            let _ = self.producer.push(s);
        }
        let slots = self.consumer.slots();
        if let Ok(chunk) = self.consumer.read_chunk(slots) {
            let (s1, s2) = chunk.as_slices();
            let _ = self.monitor.add_frames(s1);
            let _ = self.monitor.add_frames(s2);
            chunk.commit_all();
            self.shared_data
                .store(Arc::new(self.monitor.get_loudness()));
        }
        Ok(context.num_frames)
    }
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        let data = self.shared_data.load_full();
        Some(data as Arc<dyn Any + Send + Sync>)
    }
}
