//! Host-driven engine for DAWs and other external-clock embedders.
//!
//! Unlike [`super::AudioEngine`], this type owns no device and spawns no audio
//! pipeline threads. The host calls [`EmbeddedAudioEngine::process_at`] from its
//! render callback and therefore remains the sole owner of timing and I/O.

use super::processing_thread::build_plugin_host_with_policy;
use crate::{EngineConfig, PluginBuildDiagnostic};
use sotf_plugins::{ParameterEventSender, ParameterValue, PluginHost};

/// Allocation-free-after-build, externally-clocked plugin engine.
pub struct EmbeddedAudioEngine {
    host: PluginHost,
    input_sample_rate: u32,
    max_block_frames: usize,
}

impl EmbeddedAudioEngine {
    /// Build the configured graph without opening an audio device.
    ///
    /// Non-fatal skipped-plugin diagnostics are returned alongside the engine.
    pub fn new(
        config: &EngineConfig,
    ) -> Result<(Self, Vec<PluginBuildDiagnostic>), PluginBuildDiagnostic> {
        config.validate().map_err(PluginBuildDiagnostic::host)?;
        let (host, diagnostics) = build_plugin_host_with_policy(
            &config.plugins,
            config.output_sample_rate,
            config.input_channels,
            config.oversampling_policy,
        )?;
        Ok((
            Self {
                host,
                input_sample_rate: config.output_sample_rate,
                max_block_frames: config.frame_size,
            },
            diagnostics,
        ))
    }

    /// Process one host-owned render block at an absolute sample position.
    ///
    /// `input` and `output` are interleaved. The caller must size `output` for
    /// `output_frames_for_input(input_frames) * output_channels()` samples.
    /// No device, sleeping, locks, or internal clock are involved.
    pub fn process_at(
        &mut self,
        sample_position: u64,
        input: &[f32],
        output: &mut [f32],
    ) -> Result<usize, String> {
        let input_channels = self.host.input_channels();
        if input_channels == 0 || !input.len().is_multiple_of(input_channels) {
            return Err(format!(
                "embedded input has {} samples, not a whole number of {}-channel frames",
                input.len(),
                input_channels
            ));
        }
        let input_frames = input.len() / input_channels;
        if input_frames > self.max_block_frames {
            return Err(format!(
                "embedded block has {input_frames} frames, exceeding configured maximum {}",
                self.max_block_frames
            ));
        }
        let required = self
            .host
            .output_frames_for_input(input_frames)
            .checked_mul(self.host.output_channels())
            .ok_or("embedded output size overflow")?;
        if output.len() < required {
            return Err(format!(
                "embedded output buffer too small: need {required} samples, got {}",
                output.len()
            ));
        }
        self.host.set_playback_position(sample_position);
        self.host.process(input, &mut output[..required])
    }

    /// Queue sample-accurate automation within the next render block.
    pub fn set_plugin_parameter_at(
        &mut self,
        plugin_index: usize,
        param_id: &str,
        value: ParameterValue,
        sample_offset: usize,
    ) -> Result<(), String> {
        self.host
            .validate_automatable_plugin_parameter(plugin_index, param_id, &value)?;
        self.host
            .set_plugin_parameter_at(plugin_index, param_id, value, sample_offset)
    }

    /// Move the bounded automation producer to a control thread.
    ///
    /// Once taken, automation should be queued through the returned sender;
    /// the render callback consumes it without locking.
    pub fn take_parameter_event_sender(&mut self) -> Option<ParameterEventSender> {
        self.host.take_parameter_event_sender()
    }

    pub fn input_channels(&self) -> usize {
        self.host.input_channels()
    }

    pub fn output_channels(&self) -> usize {
        self.host.output_channels()
    }

    pub fn output_sample_rate(&self) -> u32 {
        self.host.output_sample_rate(self.input_sample_rate)
    }

    pub fn output_frames_for_input(&self, input_frames: usize) -> usize {
        self.host.output_frames_for_input(input_frames)
    }

    pub fn latency_samples(&self) -> usize {
        self.host.total_latency_samples()
    }

    /// Reset plugin history after a transport discontinuity or loop jump.
    pub fn reset_transport(&mut self, sample_position: u64) {
        self.host.reset();
        self.host.set_playback_position(sample_position);
    }

    /// Maximum input block size accepted without growing host scratch storage.
    pub fn max_block_frames(&self) -> usize {
        self.max_block_frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PluginConfig;

    #[test]
    fn external_clock_and_sample_accurate_automation_drive_rendering() {
        let config = EngineConfig {
            frame_size: 8,
            plugins: vec![PluginConfig {
                plugin_type: "gain".to_string(),
                parameters: serde_json::json!({ "gain_db": 0.0 }),
            }],
            ..EngineConfig::default()
        };
        let (mut engine, diagnostics) = EmbeddedAudioEngine::new(&config).unwrap();
        assert!(diagnostics.is_empty());
        engine
            .set_plugin_parameter_at(0, "gain_db", ParameterValue::Float(-6.0), 4)
            .unwrap();

        let input = vec![1.0; 16];
        let mut output = vec![0.0; 16];
        assert_eq!(engine.process_at(96_000, &input, &mut output).unwrap(), 8);
        assert!(
            output[..8]
                .iter()
                .all(|sample| (*sample - 1.0).abs() < 1e-5)
        );
        // The gain plugin smooths realtime changes, so it intentionally does
        // not jump to the final -6 dB value. The externally scheduled boundary
        // still guarantees the first four stereo frames are untouched and the
        // following segment begins moving toward the new value.
        assert!(output[8..].iter().any(|sample| *sample < 0.9999));
    }
}
