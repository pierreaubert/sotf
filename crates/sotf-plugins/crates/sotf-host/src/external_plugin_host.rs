//! Host-side controller for out-of-process external plugin processing.
//!
//! This owns the host-created secure shared-memory segment, publishes audio
//! blocks into it, waits up to a caller-provided deadline for the worker, and
//! falls back to bypass-style audio when the worker fails or misses the block.

use std::path::Path;
use std::time::{Duration, Instant};

use crate::external_plugin_ipc::{
    PluginIpcLayout, PluginIpcState, PluginSandboxRuntimeStatus, SecurePluginSharedMemory,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalPluginHostBlockStatus {
    Processed,
    TimedOut,
    WorkerFailed,
    WrongSequence,
}

pub struct ExternalPluginHostProxy {
    shared: SecurePluginSharedMemory,
    deadline: Duration,
    next_sequence: u64,
    timeout_count: u64,
    worker_failure_count: u64,
    wrong_sequence_count: u64,
}

impl ExternalPluginHostProxy {
    pub fn new(layout: PluginIpcLayout, deadline: Duration) -> Result<Self, String> {
        let shared = SecurePluginSharedMemory::create(layout)
            .map_err(|err| format!("failed to create external-plugin shared memory: {err}"))?;
        Ok(Self::from_shared(shared, deadline))
    }

    pub fn from_shared(shared: SecurePluginSharedMemory, deadline: Duration) -> Self {
        Self {
            shared,
            deadline,
            next_sequence: 1,
            timeout_count: 0,
            worker_failure_count: 0,
            wrong_sequence_count: 0,
        }
    }

    pub fn shared_path(&self) -> &Path {
        self.shared.path()
    }

    pub fn timeout_count(&self) -> u64 {
        self.timeout_count
    }

    pub fn worker_failure_count(&self) -> u64 {
        self.worker_failure_count
    }

    pub fn wrong_sequence_count(&self) -> u64 {
        self.wrong_sequence_count
    }

    pub fn worker_sandbox_status(&self) -> PluginSandboxRuntimeStatus {
        self.shared.worker_sandbox_status()
    }

    pub fn process_block(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        frames: usize,
    ) -> Result<(usize, ExternalPluginHostBlockStatus), String> {
        self.process_block_with(input, output, frames, || {})
    }

    pub fn process_block_with(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        frames: usize,
        mut drive_worker: impl FnMut(),
    ) -> Result<(usize, ExternalPluginHostBlockStatus), String> {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1).max(1);
        self.shared
            .publish_host_block(sequence, frames, input)
            .map_err(|err| format!("failed to publish external-plugin block: {err}"))?;

        let start = Instant::now();
        loop {
            match self.shared.worker_state() {
                PluginIpcState::WorkerReady => {
                    if self.shared.worker_sequence() == sequence {
                        let processed = self.shared.copy_worker_output(output).map_err(|err| {
                            format!("failed to copy external-plugin output: {err}")
                        })?;
                        self.shared.clear_block();
                        return Ok((processed, ExternalPluginHostBlockStatus::Processed));
                    }
                    self.wrong_sequence_count = self.wrong_sequence_count.saturating_add(1);
                    let frames = self.write_fallback(input, output, frames);
                    self.shared.clear_block();
                    return Ok((frames, ExternalPluginHostBlockStatus::WrongSequence));
                }
                PluginIpcState::WorkerFailed => {
                    if self.shared.worker_sequence() == sequence {
                        self.worker_failure_count = self.worker_failure_count.saturating_add(1);
                        let frames = self.write_fallback(input, output, frames);
                        self.shared.clear_block();
                        return Ok((frames, ExternalPluginHostBlockStatus::WorkerFailed));
                    }
                }
                _ => {}
            }

            if start.elapsed() >= self.deadline {
                self.timeout_count = self.timeout_count.saturating_add(1);
                let frames = self.write_fallback(input, output, frames);
                self.shared.clear_block();
                return Ok((frames, ExternalPluginHostBlockStatus::TimedOut));
            }

            drive_worker();
            std::hint::spin_loop();
        }
    }

    fn write_fallback(&self, input: &[f32], output: &mut [f32], frames: usize) -> usize {
        let layout = self.shared.layout();
        let input_channels = layout.input_channels as usize;
        let output_channels = layout.output_channels as usize;
        let output_len = frames.saturating_mul(output_channels).min(output.len());
        output[..output_len].fill(0.0);

        if input_channels == 0 || output_channels == 0 {
            return frames;
        }

        let copy_channels = input_channels.min(output_channels);
        for frame in 0..frames {
            let src_base = frame * input_channels;
            let dst_base = frame * output_channels;
            if src_base >= input.len() || dst_base >= output_len {
                break;
            }

            let src_end = (src_base + copy_channels).min(input.len());
            let dst_end = (dst_base + copy_channels).min(output_len);
            let copied = (src_end - src_base).min(dst_end - dst_base);
            output[dst_base..dst_base + copied]
                .copy_from_slice(&input[src_base..src_base + copied]);
        }

        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external_plugin_worker::ExternalPluginWorker;
    use crate::parameters::{Parameter, ParameterId, ParameterValue};
    use crate::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};

    struct ScalePlugin {
        channels: usize,
        factor: f32,
    }

    impl Plugin for ScalePlugin {
        fn info(&self) -> PluginInfo {
            PluginInfo::new("Scale", "0.1", "test")
        }

        fn input_channels(&self) -> usize {
            self.channels
        }

        fn output_channels(&self) -> usize {
            self.channels
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

        fn process(
            &mut self,
            input: &[f32],
            output: &mut [f32],
            context: &ProcessContext,
        ) -> PluginResult<usize> {
            let samples = context.num_frames * self.channels;
            for idx in 0..samples {
                output[idx] = input[idx] * self.factor;
            }
            Ok(context.num_frames)
        }
    }

    struct PanickingPlugin {
        channels: usize,
    }

    impl Plugin for PanickingPlugin {
        fn info(&self) -> PluginInfo {
            PluginInfo::new("Panic", "0.1", "test")
        }

        fn input_channels(&self) -> usize {
            self.channels
        }

        fn output_channels(&self) -> usize {
            self.channels
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

        fn process(&mut self, _: &[f32], _: &mut [f32], _: &ProcessContext) -> PluginResult<usize> {
            panic!("worker plugin crash")
        }
    }

    #[test]
    fn test_host_proxy_processes_worker_output() {
        let layout = PluginIpcLayout::new(48_000, 128, 2, 2).unwrap();
        let mut proxy = ExternalPluginHostProxy::new(layout, Duration::from_millis(10)).unwrap();
        let worker_shared = SecurePluginSharedMemory::open_existing(proxy.shared_path()).unwrap();
        let mut worker = ExternalPluginWorker::new(
            worker_shared,
            Box::new(ScalePlugin {
                channels: 2,
                factor: 3.0,
            }),
        )
        .unwrap();

        let input = vec![0.25, -0.5, 1.0, -1.0];
        let mut output = vec![0.0; input.len()];
        let (frames, status) = proxy
            .process_block_with(&input, &mut output, 2, || {
                let _ = worker.process_one();
            })
            .unwrap();

        assert_eq!(frames, 2);
        assert_eq!(status, ExternalPluginHostBlockStatus::Processed);
        assert_eq!(output, vec![0.75, -1.5, 3.0, -3.0]);
    }

    #[test]
    fn test_host_proxy_times_out_to_passthrough() {
        let layout = PluginIpcLayout::new(48_000, 128, 2, 2).unwrap();
        let mut proxy = ExternalPluginHostProxy::new(layout, Duration::ZERO).unwrap();

        let input = vec![0.25, -0.5, 1.0, -1.0];
        let mut output = vec![0.0; input.len()];
        let (frames, status) = proxy.process_block(&input, &mut output, 2).unwrap();

        assert_eq!(frames, 2);
        assert_eq!(status, ExternalPluginHostBlockStatus::TimedOut);
        assert_eq!(output, input);
        assert_eq!(proxy.timeout_count(), 1);
    }

    #[test]
    fn test_host_proxy_worker_failure_falls_back() {
        let layout = PluginIpcLayout::new(48_000, 128, 2, 2).unwrap();
        let mut proxy = ExternalPluginHostProxy::new(layout, Duration::from_millis(10)).unwrap();
        let worker_shared = SecurePluginSharedMemory::open_existing(proxy.shared_path()).unwrap();
        let mut worker =
            ExternalPluginWorker::new(worker_shared, Box::new(PanickingPlugin { channels: 2 }))
                .unwrap();

        let input = vec![0.25, -0.5, 1.0, -1.0];
        let mut output = vec![0.0; input.len()];
        let (frames, status) = proxy
            .process_block_with(&input, &mut output, 2, || {
                let _ = worker.process_one();
            })
            .unwrap();

        assert_eq!(frames, 2);
        assert_eq!(status, ExternalPluginHostBlockStatus::WorkerFailed);
        assert_eq!(output, input);
        assert_eq!(proxy.worker_failure_count(), 1);
    }
}
