//! Worker-side runner for isolated external plugins.
//!
//! This is the code shape the future subprocess will use: it opens the secure
//! shared-memory transport, copies each block into private buffers, invokes the
//! hosted plugin, and publishes the result back to the host.

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use crate::external_plugin_ipc::{PluginIpcRequest, SecurePluginSharedMemory};
use crate::plugin::Plugin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalPluginWorkerStep {
    NoRequest,
    Processed { sequence: u64, frames: usize },
}

pub struct ExternalPluginWorker {
    shared: SecurePluginSharedMemory,
    plugin: Box<dyn Plugin>,
    input_scratch: Vec<f32>,
    output_scratch: Vec<f32>,
}

impl ExternalPluginWorker {
    pub fn open<P: AsRef<Path>>(path: P, plugin: Box<dyn Plugin>) -> Result<Self, String> {
        let shared = SecurePluginSharedMemory::open_existing(path).map_err(|err| {
            format!("failed to open external-plugin shared memory as worker: {err}")
        })?;
        Self::new(shared, plugin)
    }

    pub fn new(shared: SecurePluginSharedMemory, plugin: Box<dyn Plugin>) -> Result<Self, String> {
        let layout = shared.layout();
        let plugin_inputs = plugin.input_channels();
        let plugin_outputs = plugin.output_channels();
        if plugin_inputs != layout.input_channels as usize {
            return Err(format!(
                "worker plugin input channel mismatch: plugin has {plugin_inputs}, shared layout has {}",
                layout.input_channels
            ));
        }
        if plugin_outputs != layout.output_channels as usize {
            return Err(format!(
                "worker plugin output channel mismatch: plugin has {plugin_outputs}, shared layout has {}",
                layout.output_channels
            ));
        }
        shared.publish_worker_latency_samples(plugin.latency_samples());

        let max_input_samples = layout.max_frames as usize * layout.input_channels as usize;
        let max_output_samples = layout.max_frames as usize * layout.output_channels as usize;

        Ok(Self {
            shared,
            plugin,
            input_scratch: Vec::with_capacity(max_input_samples),
            output_scratch: Vec::with_capacity(max_output_samples),
        })
    }

    pub fn process_one(&mut self) -> Result<ExternalPluginWorkerStep, String> {
        let Some(request) = self
            .shared
            .take_worker_request()
            .map_err(|err| format!("failed to read external-plugin worker request: {err}"))?
        else {
            return Ok(ExternalPluginWorkerStep::NoRequest);
        };

        self.process_request(request)
    }

    pub fn shared(&self) -> &SecurePluginSharedMemory {
        &self.shared
    }

    fn process_request(
        &mut self,
        request: PluginIpcRequest,
    ) -> Result<ExternalPluginWorkerStep, String> {
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.shared.process_worker_request(
                self.plugin.as_mut(),
                request,
                &mut self.input_scratch,
                &mut self.output_scratch,
            )
        }));

        match result {
            Ok(Ok(frames)) => Ok(ExternalPluginWorkerStep::Processed {
                sequence: request.sequence,
                frames,
            }),
            Ok(Err(err)) => Err(format!("external-plugin worker processing failed: {err}")),
            Err(payload) => {
                self.shared.publish_worker_failure(request.sequence, 3);
                Err(format!(
                    "external-plugin worker plugin panicked: {}",
                    panic_payload_description(payload.as_ref())
                ))
            }
        }
    }
}

fn panic_payload_description(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external_plugin_ipc::{PluginIpcLayout, PluginIpcState};
    use crate::parameters::{Parameter, ParameterId, ParameterValue};
    use crate::plugin::{PluginInfo, PluginResult, ProcessContext};

    struct ScalePlugin {
        channels: usize,
        factor: f32,
        latency_samples: usize,
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

        fn latency_samples(&self) -> usize {
            self.latency_samples
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
    fn test_worker_process_one_publishes_output() {
        let layout = PluginIpcLayout::new(48_000, 128, 2, 2).unwrap();
        let mut host_shared = SecurePluginSharedMemory::create(layout).unwrap();
        let worker_shared = SecurePluginSharedMemory::open_existing(host_shared.path()).unwrap();
        let mut worker = ExternalPluginWorker::new(
            worker_shared,
            Box::new(ScalePlugin {
                channels: 2,
                factor: 4.0,
                latency_samples: 96,
            }),
        )
        .unwrap();
        assert_eq!(host_shared.worker_latency_samples(), Some(96));

        let input = vec![0.25, -0.5, 1.0, -1.0];
        let mut output = vec![0.0; input.len()];
        host_shared.publish_host_block(9, 2, &input).unwrap();

        let step = worker.process_one().unwrap();
        assert_eq!(
            step,
            ExternalPluginWorkerStep::Processed {
                sequence: 9,
                frames: 2
            }
        );
        assert_eq!(host_shared.copy_worker_output(&mut output).unwrap(), 2);
        assert_eq!(output, vec![1.0, -2.0, 4.0, -4.0]);
        assert_eq!(
            worker.process_one().unwrap(),
            ExternalPluginWorkerStep::NoRequest
        );
    }

    #[test]
    fn test_worker_marks_failure_when_plugin_panics() {
        let layout = PluginIpcLayout::new(48_000, 128, 2, 2).unwrap();
        let mut host_shared = SecurePluginSharedMemory::create(layout).unwrap();
        let worker_shared = SecurePluginSharedMemory::open_existing(host_shared.path()).unwrap();
        let mut worker =
            ExternalPluginWorker::new(worker_shared, Box::new(PanickingPlugin { channels: 2 }))
                .unwrap();

        let input = vec![0.25, -0.5, 1.0, -1.0];
        host_shared.publish_host_block(10, 2, &input).unwrap();

        let err = worker.process_one().unwrap_err();
        assert!(err.contains("panicked"));
        assert_eq!(host_shared.worker_state(), PluginIpcState::WorkerFailed);
        assert_eq!(host_shared.worker_sequence(), 10);
    }
}
