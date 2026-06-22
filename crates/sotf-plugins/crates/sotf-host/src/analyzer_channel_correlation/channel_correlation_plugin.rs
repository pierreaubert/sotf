use super::channel_correlation_monitor::ChannelCorrelationMonitor;
use crate::analyzer::{CorrelationData, RealTimeCache};
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use crate::plugin::{
    Plugin, PluginCompileMetadata, PluginCompiledOp, PluginCostClass, PluginInfo, PluginResult,
    ProcessContext,
};
use rtrb::{Consumer, RingBuffer};
use std::any::Any;
use std::sync::Arc;

/// Analyzer plugin wrapper. Mirrors `LoudnessMonitorPlugin` so it can be
/// dropped into the same host DAG slot.
///
/// **Status:** not currently registered with the engine's plugin factory.
/// The spatial-spider visualiser today reads correlation data from the
/// matrix embedded in `LoudnessData.correlation_matrix` (computed by the
/// permanent output LoudnessMonitor). This standalone plugin is kept for
/// the case where a future caller wants per-node correlation analysis
/// (e.g. inserted downstream of a specific plugin to capture *its* output
/// rather than the chain output). When that lands, add a `"channel_correlation"`
/// arm to `processing_thread::create_plugin_from_settings` and a
/// `PluginType::ChannelCorrelation` variant.
pub struct ChannelCorrelationPlugin {
    pub(super) num_channels: usize,
    pub(super) sample_rate: u32,
    pub(super) enabled: bool,
    pub(super) producer: rtrb::Producer<f32>,
    pub(super) consumer: Consumer<f32>,
    pub(super) cache: RealTimeCache<CorrelationData>,
    pub(super) monitor: ChannelCorrelationMonitor,
    pub(super) cached_parameters: Vec<Parameter>,
}

impl ChannelCorrelationPlugin {
    pub fn new(num_channels: usize) -> Result<Self, String> {
        let sr = 48000;
        let (p, c) = RingBuffer::new(sr as usize * 2);
        let monitor = ChannelCorrelationMonitor::new(num_channels, sr);
        let cache = RealTimeCache::new(CorrelationData::new(num_channels));
        let mut plugin = Self {
            num_channels,
            sample_rate: sr,
            enabled: true,
            producer: p,
            consumer: c,
            cache,
            monitor,
            cached_parameters: Vec::new(),
        };
        plugin.rebuild_cached_parameters();
        Ok(plugin)
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![Parameter::new_bool("enabled", "Enabled", self.enabled)];
    }

    /// Read-only handle to the cache, for hosts that want to wire the data
    /// into UI state outside of `Plugin::get_data`.
    pub fn cache(&self) -> &RealTimeCache<CorrelationData> {
        &self.cache
    }
}

impl Plugin for ChannelCorrelationPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Channel Correlation", "1.0.0", "Sotf")
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Analyzer
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::analyzer(Some(PluginCompiledOp::AnalyzerTap))
    }

    fn input_channels(&self) -> usize {
        self.num_channels
    }
    fn output_channels(&self) -> usize {
        self.num_channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;
        if id.as_str() == "enabled" {
            self.enabled = value.as_bool().unwrap_or(true);
            self.rebuild_cached_parameters();
        }
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id.as_str() == "enabled" {
            Some(ParameterValue::Bool(self.enabled))
        } else {
            None
        }
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        self.monitor = ChannelCorrelationMonitor::new(self.num_channels, sr);
        Ok(())
    }
    fn reset(&mut self) {
        self.monitor.reset();
        let nc = self.num_channels;
        self.cache.update(|d| {
            *d = CorrelationData::new(nc);
        });
    }
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        output.copy_from_slice(input);
        if !self.enabled {
            return Ok(context.num_frames);
        }
        let mut dropped = 0usize;
        for &s in input {
            if self.producer.push(s).is_err() {
                dropped += 1;
            }
        }
        if dropped > 0 {
            crate::rate_limited_log!(
                warn,
                5,
                "correlation ring buffer full, dropped {dropped} samples"
            );
        }
        let slots = self.consumer.slots();
        if let Ok(chunk) = self.consumer.read_chunk(slots) {
            let (s1, s2) = chunk.as_slices();
            self.monitor.add_frames(s1);
            self.monitor.add_frames(s2);
            chunk.commit_all();

            let monitor = &self.monitor;
            self.cache.update(|d| {
                monitor.update_correlation_data(d);
            });
        }
        Ok(context.num_frames)
    }
    fn process_compiled_f32(
        &mut self,
        op: PluginCompiledOp,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Option<Result<usize, String>> {
        if op != PluginCompiledOp::AnalyzerTap {
            return None;
        }
        Some(self.process(input, output, context))
    }
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
    fn take_cache_contention_stats(&mut self) -> (u64, u64) {
        self.cache.take_contention_stats()
    }
}
