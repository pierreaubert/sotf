//! Worker-side runner for isolated external plugins.
//!
//! This is the code shape the future subprocess will use: it opens the secure
//! shared-memory transport, copies each block into private buffers, invokes the
//! hosted plugin, and publishes the result back to the host.

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use crate::external_plugin_ipc::{
    PluginIpcControlRequest, PluginIpcControlResponse, PluginIpcRequest, SecurePluginSharedMemory,
};
use crate::parameters::Parameter;
use crate::plugin::{MidiEvent, ParameterEvent, Plugin, ProcessContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalPluginWorkerStep {
    NoRequest,
    Controlled,
    Processed { sequence: u64, frames: usize },
}

pub struct ExternalPluginWorker {
    shared: SecurePluginSharedMemory,
    plugin: Box<dyn Plugin>,
    input_scratch: Vec<f32>,
    output_scratch: Vec<f32>,
    midi_scratch: Vec<MidiEvent>,
    parameter_scratch: Vec<ParameterEvent>,
    parameters: Vec<Parameter>,
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

        let parameters = plugin.parameters();
        Ok(Self {
            shared,
            plugin,
            input_scratch: vec![0.0; max_input_samples],
            output_scratch: vec![0.0; max_output_samples],
            midi_scratch: Vec::with_capacity(1024),
            parameter_scratch: Vec::with_capacity(1024),
            parameters,
        })
    }

    pub fn process_one(&mut self) -> Result<ExternalPluginWorkerStep, String> {
        if let Some((sequence, request)) = self
            .shared
            .take_control_request()
            .map_err(|error| format!("failed to read external-plugin control request: {error}"))?
        {
            let response = match request {
                PluginIpcControlRequest::Describe => PluginIpcControlResponse::Description {
                    parameters: self.parameters.clone(),
                },
                PluginIpcControlRequest::Set { id, value } => self
                    .plugin
                    .set_parameter(id, value)
                    .map_or_else(PluginIpcControlResponse::Error, |_| {
                        PluginIpcControlResponse::Ack
                    }),
                PluginIpcControlRequest::Get { id } => {
                    PluginIpcControlResponse::Value(self.plugin.get_parameter(&id))
                }
                PluginIpcControlRequest::SaveState => self.plugin.save_opaque_state().map_or_else(
                    PluginIpcControlResponse::Error,
                    PluginIpcControlResponse::State,
                ),
                PluginIpcControlRequest::LoadState { state } => self
                    .plugin
                    .load_opaque_state(&state)
                    .map_or_else(PluginIpcControlResponse::Error, |_| {
                        PluginIpcControlResponse::Ack
                    }),
            };
            self.shared
                .publish_control_response(sequence, &response)
                .map_err(|error| format!("failed to publish control response: {error}"))?;
            return Ok(ExternalPluginWorkerStep::Controlled);
        }
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
            let transport = self
                .shared
                .read_host_context(request.frames, &mut self.midi_scratch)?;
            self.shared.read_parameter_events(
                request.frames,
                &self.parameters,
                &mut self.parameter_scratch,
            )?;
            let context = ProcessContext::new(self.shared.layout().sample_rate, request.frames)
                .with_transport(transport)
                .with_all_events(&self.midi_scratch, &[], &self.parameter_scratch);
            self.shared.process_worker_request(
                self.plugin.as_mut(),
                request,
                &mut self.input_scratch,
                &mut self.output_scratch,
                &context,
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
    use crate::plugin::{MidiMessage, ParameterEvent, TransportInfo};
    use crate::plugin::{PluginInfo, PluginResult, ProcessContext};
    use std::sync::{Arc, Mutex};

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

    struct ProtocolPlugin {
        value: f32,
        observed: Arc<Mutex<Option<ObservedContext>>>,
    }

    type ObservedContext = (usize, usize, u64, f64);

    impl Plugin for ProtocolPlugin {
        fn info(&self) -> PluginInfo {
            PluginInfo::new("Protocol", "0.1", "test")
        }
        fn input_channels(&self) -> usize {
            1
        }
        fn output_channels(&self) -> usize {
            1
        }
        fn parameters(&self) -> Vec<Parameter> {
            vec![Parameter::new_float("value", "Value", 1.0, 0.0, 4.0)]
        }
        fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
            if id.as_str() != "value" {
                return Err("unknown parameter".into());
            }
            self.value = value
                .as_float()
                .ok_or_else(|| "expected float".to_string())?;
            Ok(())
        }
        fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
            (id.as_str() == "value").then_some(ParameterValue::Float(self.value))
        }
        fn save_opaque_state(&self) -> PluginResult<Vec<u8>> {
            Ok(self.value.to_le_bytes().to_vec())
        }
        fn load_opaque_state(&mut self, state: &[u8]) -> PluginResult<()> {
            let bytes: [u8; 4] = state.try_into().map_err(|_| "invalid state".to_string())?;
            self.value = f32::from_le_bytes(bytes);
            Ok(())
        }
        fn process(
            &mut self,
            input: &[f32],
            output: &mut [f32],
            context: &ProcessContext,
        ) -> PluginResult<usize> {
            *self.observed.lock().unwrap() = Some((
                context
                    .midi_events
                    .first()
                    .map_or(usize::MAX, |event| event.sample_offset),
                context
                    .parameter_events
                    .first()
                    .map_or(usize::MAX, |event| event.sample_offset),
                context.transport.sample_position,
                context.transport.bpm,
            ));
            output[..context.num_frames].copy_from_slice(&input[..context.num_frames]);
            Ok(context.num_frames)
        }
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

    #[test]
    fn versioned_ipc_preserves_event_offsets_and_transport() {
        let layout = PluginIpcLayout::new(48_000, 64, 1, 1).unwrap();
        let mut host = SecurePluginSharedMemory::create(layout).unwrap();
        let worker_shared = SecurePluginSharedMemory::open_existing(host.path()).unwrap();
        let observed = Arc::new(Mutex::new(None));
        let mut worker = ExternalPluginWorker::new(
            worker_shared,
            Box::new(ProtocolPlugin {
                value: 1.0,
                observed: Arc::clone(&observed),
            }),
        )
        .unwrap();
        let midi = [crate::plugin::MidiEvent::new(
            11,
            MidiMessage::note_on(0, 60, 100),
        )];
        let automation = [ParameterEvent::new(
            23,
            ParameterId::from("value"),
            ParameterValue::Float(2.0),
        )];
        let context = ProcessContext::new(48_000, 64)
            .with_transport(TransportInfo::at_sample(12_345, 48_000).with_tempo(93.0, 48_000))
            .with_all_events(&midi, &[], &automation);
        let parameter_event = crate::external_plugin_ipc::PluginIpcParameterEvent {
            sample_offset: 23,
            parameter_index: 0,
            value_tag: 0,
            value_bits: 2.0_f32.to_bits(),
        };
        host.publish_host_block_with_events(1, 64, &[0.0; 64], &context, &[parameter_event])
            .unwrap();
        assert!(matches!(
            worker.process_one().unwrap(),
            ExternalPluginWorkerStep::Processed { .. }
        ));
        assert_eq!(*observed.lock().unwrap(), Some((11, 23, 12_345, 93.0)));
    }

    #[test]
    fn versioned_control_protocol_round_trips_parameters_and_state() {
        let layout = PluginIpcLayout::new(48_000, 64, 1, 1).unwrap();
        let mut host = SecurePluginSharedMemory::create(layout).unwrap();
        let worker_shared = SecurePluginSharedMemory::open_existing(host.path()).unwrap();
        let mut worker = ExternalPluginWorker::new(
            worker_shared,
            Box::new(ProtocolPlugin {
                value: 1.0,
                observed: Arc::new(Mutex::new(None)),
            }),
        )
        .unwrap();

        host.publish_control_request(1, &PluginIpcControlRequest::Describe)
            .unwrap();
        assert_eq!(
            worker.process_one().unwrap(),
            ExternalPluginWorkerStep::Controlled
        );
        match host.take_control_response(1).unwrap().unwrap() {
            PluginIpcControlResponse::Description { parameters } => {
                assert_eq!(parameters[0].id.as_str(), "value")
            }
            response => panic!("unexpected response: {response:?}"),
        }

        host.publish_control_request(
            2,
            &PluginIpcControlRequest::Set {
                id: ParameterId::from("value"),
                value: ParameterValue::Float(3.0),
            },
        )
        .unwrap();
        worker.process_one().unwrap();
        assert!(matches!(
            host.take_control_response(2).unwrap(),
            Some(PluginIpcControlResponse::Ack)
        ));
        host.publish_control_request(3, &PluginIpcControlRequest::SaveState)
            .unwrap();
        worker.process_one().unwrap();
        let state = match host.take_control_response(3).unwrap().unwrap() {
            PluginIpcControlResponse::State(state) => state,
            response => panic!("unexpected response: {response:?}"),
        };
        assert_eq!(f32::from_le_bytes(state.try_into().unwrap()), 3.0);

        host.publish_control_request(
            4,
            &PluginIpcControlRequest::LoadState {
                state: 1.5_f32.to_le_bytes().to_vec(),
            },
        )
        .unwrap();
        worker.process_one().unwrap();
        assert!(matches!(
            host.take_control_response(4).unwrap(),
            Some(PluginIpcControlResponse::Ack)
        ));
        host.publish_control_request(
            5,
            &PluginIpcControlRequest::Get {
                id: ParameterId::from("value"),
            },
        )
        .unwrap();
        worker.process_one().unwrap();
        assert!(matches!(
            host.take_control_response(5).unwrap(),
            Some(PluginIpcControlResponse::Value(Some(ParameterValue::Float(value)))) if value == 1.5
        ));
    }
}
