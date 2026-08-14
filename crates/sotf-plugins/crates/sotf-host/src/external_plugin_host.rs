//! Host-side controller for out-of-process external plugin processing.
//!
//! This owns the host-created secure shared-memory segment, publishes audio
//! blocks into it, and consumes the result on the following callback. The
//! realtime thread never waits for the worker; late work falls back to
//! latency-matched dry audio.

use std::path::Path;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::net::UnixDatagram;

use crate::external_plugin_ipc::PluginIpcParameterEvent;
use crate::external_plugin_ipc::{
    PluginIpcControlRequest, PluginIpcControlResponse, PluginIpcLayout, PluginIpcState,
    PluginSandboxRuntimeStatus, SecurePluginSharedMemory,
};
use crate::parameters::{ParameterId, ParameterValue};
use crate::plugin::{MidiEvent, ProcessContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalPluginHostBlockStatus {
    /// The fixed-latency timeline has not reached the first scheduled output.
    Priming,
    Processed,
    TimedOut,
    WorkerFailed,
    WrongSequence,
}

#[derive(Debug, Clone, Copy)]
struct PendingTimelineBlock {
    sequence: u64,
    frames: usize,
    output_start: u64,
    timeout_counted: bool,
}

pub struct ExternalPluginHostProxy {
    shared: SecurePluginSharedMemory,
    next_sequence: u64,
    next_control_sequence: u64,
    timeout_count: u64,
    worker_failure_count: u64,
    wrong_sequence_count: u64,
    fallback_delay: Vec<f32>,
    fallback_delay_frames: usize,
    fallback_delay_pos: usize,
    fallback_block: Vec<f32>,
    last_output: Vec<f32>,
    transition_from: Vec<f32>,
    transition_remaining: usize,
    previous_status: Option<ExternalPluginHostBlockStatus>,
    parameter_ids: Vec<ParameterId>,
    parameter_event_scratch: Vec<PluginIpcParameterEvent>,
    /// Last automation value for each parameter from callbacks that could not
    /// be submitted while the single IPC slot was occupied. These values are
    /// applied at offset zero of the next submitted block so worker state does
    /// not silently diverge after a deadline miss.
    deferred_parameter_events: Vec<Option<PluginIpcParameterEvent>>,
    midi_event_scratch: Vec<MidiEvent>,
    deferred_midi_events: Vec<MidiEvent>,
    pending: Option<PendingTimelineBlock>,
    worker_output_scratch: Vec<f32>,
    timeline_audio: Vec<f32>,
    timeline_status: Vec<ExternalPluginHostBlockStatus>,
    timeline_frame: u64,
    #[cfg(unix)]
    notifier: Option<UnixDatagram>,
    #[cfg(unix)]
    wake_path: std::path::PathBuf,
}

impl ExternalPluginHostProxy {
    pub fn new(layout: PluginIpcLayout, deadline: Duration) -> Result<Self, String> {
        let shared = SecurePluginSharedMemory::create(layout)
            .map_err(|err| format!("failed to create external-plugin shared memory: {err}"))?;
        Ok(Self::from_shared(shared, deadline))
    }

    pub fn from_shared(shared: SecurePluginSharedMemory, _deadline: Duration) -> Self {
        let layout = shared.layout();
        #[cfg(unix)]
        let wake_path = shared.path().with_extension("wake");
        #[cfg(unix)]
        let notifier = UnixDatagram::unbound().ok().and_then(|socket| {
            socket.set_nonblocking(true).ok()?;
            Some(socket)
        });
        Self {
            shared,
            next_sequence: 1,
            next_control_sequence: 1,
            timeout_count: 0,
            worker_failure_count: 0,
            wrong_sequence_count: 0,
            fallback_delay: Vec::new(),
            fallback_delay_frames: 0,
            fallback_delay_pos: 0,
            fallback_block: vec![0.0; layout.max_frames as usize * layout.output_channels as usize],
            last_output: vec![0.0; layout.output_channels as usize],
            transition_from: vec![0.0; layout.output_channels as usize],
            transition_remaining: 0,
            previous_status: None,
            parameter_ids: Vec::new(),
            parameter_event_scratch: Vec::with_capacity(1024),
            deferred_parameter_events: Vec::new(),
            midi_event_scratch: Vec::with_capacity(1024),
            deferred_midi_events: Vec::with_capacity(1024),
            pending: None,
            worker_output_scratch: vec![
                0.0;
                layout.max_frames as usize * layout.output_channels as usize
            ],
            timeline_audio: vec![
                0.0;
                layout.max_frames as usize * 2 * layout.output_channels as usize
            ],
            timeline_status: vec![
                ExternalPluginHostBlockStatus::Priming;
                layout.max_frames as usize * 2
            ],
            timeline_frame: 0,
            #[cfg(unix)]
            notifier,
            #[cfg(unix)]
            wake_path,
        }
    }

    /// Configure latency-matched dry fallback on the control thread after the
    /// worker metadata handshake. This may allocate and must not be called from
    /// the audio callback.
    pub fn configure_fallback_latency(&mut self, latency_samples: usize) -> Result<(), String> {
        let channels = self.shared.layout().output_channels as usize;
        let samples = latency_samples
            .checked_mul(channels)
            .ok_or_else(|| "external-plugin fallback delay size overflow".to_string())?;
        let mut fallback_delay = Vec::new();
        fallback_delay.try_reserve_exact(samples).map_err(|error| {
            format!("could not allocate external-plugin fallback delay: {error}")
        })?;
        fallback_delay.resize(samples, 0.0);
        self.fallback_delay = fallback_delay;
        self.fallback_delay_frames = latency_samples;
        self.fallback_delay_pos = 0;
        Ok(())
    }

    pub fn configure_parameters(&mut self, ids: Vec<ParameterId>) {
        self.deferred_parameter_events.clear();
        self.deferred_parameter_events.resize(ids.len(), None);
        self.parameter_ids = ids;
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

    pub fn worker_latency_samples(&self) -> Option<usize> {
        self.shared.worker_latency_samples()
    }

    /// Fixed transport latency reserved by the isolated graph contract.
    pub fn pipeline_latency_samples(&self) -> usize {
        self.shared.layout().max_frames as usize
    }

    pub fn request_control(
        &mut self,
        request: &PluginIpcControlRequest,
        timeout: Duration,
    ) -> Result<PluginIpcControlResponse, String> {
        let sequence = self.next_control_sequence;
        self.next_control_sequence = self.next_control_sequence.wrapping_add(1).max(1);
        self.shared
            .publish_control_request(sequence, request)
            .map_err(|error| {
                format!("failed to publish external-plugin control request: {error}")
            })?;
        self.notify_worker();
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| "external-plugin control timeout is too large".to_string())?;
        loop {
            if let Some(response) = self
                .shared
                .take_control_response(sequence)
                .map_err(|error| format!("failed to read control response: {error}"))?
            {
                return Ok(response);
            }
            if Instant::now() >= deadline {
                return Err("external-plugin control request timed out".to_string());
            }
            std::thread::yield_now();
        }
    }

    pub fn process_block(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        frames: usize,
    ) -> Result<(usize, ExternalPluginHostBlockStatus), String> {
        self.process_block_with(input, output, frames, || {})
    }

    pub fn process_block_with_context(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<(usize, ExternalPluginHostBlockStatus), String> {
        self.process_block_with_context_and_worker(input, output, context, || {})
    }

    /// Produce the same latency-compensated fallback used for IPC failures
    /// without publishing a request. This keeps quarantined and launch-failed
    /// instances on the compiled graph timeline.
    pub fn process_fallback(&mut self, input: &[f32], output: &mut [f32], frames: usize) -> usize {
        debug_assert!(frames <= self.pipeline_latency_samples());
        self.prepare_fallback(input, frames);
        self.write_fallback_to_timeline(
            self.timeline_frame + self.pipeline_latency_samples() as u64,
            frames,
            ExternalPluginHostBlockStatus::TimedOut,
        );
        self.emit_timeline(output, frames);
        self.timeline_frame = self.timeline_frame.saturating_add(frames as u64);
        frames
    }

    pub fn process_block_with(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        frames: usize,
        drive_worker: impl FnMut(),
    ) -> Result<(usize, ExternalPluginHostBlockStatus), String> {
        let context = ProcessContext::new(self.shared.layout().sample_rate, frames);
        self.process_block_with_context_and_worker(input, output, &context, drive_worker)
    }

    pub fn process_block_with_context_and_worker(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
        mut drive_worker: impl FnMut(),
    ) -> Result<(usize, ExternalPluginHostBlockStatus), String> {
        let frames = context.num_frames;
        let layout = self.shared.layout();
        let max_frames = layout.max_frames as usize;
        if frames > max_frames {
            return Err(format!(
                "external-plugin callback has {frames} frames, maximum is {max_frames}"
            ));
        }
        if context.sample_rate != layout.sample_rate {
            return Err(format!(
                "external-plugin callback sample rate is {}, expected {}",
                context.sample_rate, layout.sample_rate
            ));
        }
        let input_samples = frames
            .checked_mul(layout.input_channels as usize)
            .ok_or_else(|| "external-plugin input sample count overflow".to_string())?;
        let output_samples = frames
            .checked_mul(layout.output_channels as usize)
            .ok_or_else(|| "external-plugin output sample count overflow".to_string())?;
        if input.len() < input_samples {
            return Err(format!(
                "external-plugin input has {} samples, expected at least {input_samples}",
                input.len()
            ));
        }
        if output.len() < output_samples {
            return Err(format!(
                "external-plugin output has {} samples, expected at least {output_samples}",
                output.len()
            ));
        }
        if context.midi_events.len() > self.midi_event_scratch.capacity() {
            return Err("external-plugin MIDI event capacity exceeded".to_string());
        }
        if context
            .midi_events
            .iter()
            .any(|event| event.sample_offset >= frames)
        {
            return Err("external-plugin MIDI event offset is outside block".to_string());
        }
        self.parameter_event_scratch.clear();
        if context.parameter_events.len() > self.parameter_event_scratch.capacity() {
            return Err("external-plugin automation event capacity exceeded".to_string());
        }
        for event in context.parameter_events {
            let parameter_index = self
                .parameter_ids
                .iter()
                .position(|id| id == &event.parameter_id)
                .ok_or_else(|| {
                    format!("unknown external-plugin parameter '{}'", event.parameter_id)
                })?;
            let (value_tag, value_bits) = match &event.value {
                ParameterValue::Float(value) => (0, value.to_bits()),
                ParameterValue::Int(value) => (1, *value as u32),
                ParameterValue::Bool(value) => (2, u32::from(*value)),
                ParameterValue::String(_) => {
                    return Err("string parameters cannot be sample-accurately automated".into());
                }
            };
            self.parameter_event_scratch.push(PluginIpcParameterEvent {
                sample_offset: event.sample_offset as u32,
                parameter_index: parameter_index as u32,
                value_tag,
                value_bits,
            });
        }
        // Poll once at callback entry. Completed work overwrites its future
        // fallback region on the absolute timeline; late work never changes
        // samples that have already left the callback.
        self.resolve_pending(self.timeline_frame.saturating_add(frames as u64))?;

        if self.pending.is_none() {
            self.prepend_deferred_parameter_events()?;
            let midi_count = self
                .deferred_midi_events
                .len()
                .checked_add(context.midi_events.len())
                .ok_or_else(|| "external-plugin MIDI event count overflow".to_string())?;
            if midi_count > self.midi_event_scratch.capacity() {
                return Err(format!(
                    "external-plugin deferred MIDI requires {midi_count} events, capacity is {}",
                    self.midi_event_scratch.capacity()
                ));
            }
            self.midi_event_scratch.clear();
            self.midi_event_scratch
                .extend_from_slice(&self.deferred_midi_events);
            self.midi_event_scratch
                .extend_from_slice(context.midi_events);
            let mut ipc_context = *context;
            ipc_context.midi_events = &self.midi_event_scratch;
            let sequence = self.next_sequence;
            self.next_sequence = self.next_sequence.wrapping_add(1).max(1);
            self.shared
                .publish_host_block_with_events(
                    sequence,
                    frames,
                    input,
                    &ipc_context,
                    &self.parameter_event_scratch,
                )
                .map_err(|err| format!("failed to publish external-plugin block: {err}"))?;
            self.deferred_parameter_events.fill(None);
            self.deferred_midi_events.clear();
            self.pending = Some(PendingTimelineBlock {
                sequence,
                frames,
                output_start: self.timeline_frame + max_frames as u64,
                timeout_counted: false,
            });
            self.notify_worker();
            drive_worker();
        } else {
            // A single shared-memory slot is intentionally bounded. If its
            // worker misses a callback, this callback is already scheduled as
            // constant-latency fallback rather than overwriting in-flight IO.
            let deferred_count = self
                .deferred_midi_events
                .len()
                .checked_add(context.midi_events.len())
                .ok_or_else(|| "external-plugin deferred MIDI event count overflow".to_string())?;
            if deferred_count > self.deferred_midi_events.capacity() {
                return Err(format!(
                    "external-plugin deferred MIDI requires {deferred_count} events, capacity is {}",
                    self.deferred_midi_events.capacity()
                ));
            }
            for event in &self.parameter_event_scratch {
                let mut deferred = *event;
                deferred.sample_offset = 0;
                self.deferred_parameter_events[event.parameter_index as usize] = Some(deferred);
            }
            self.deferred_midi_events.extend(
                context
                    .midi_events
                    .iter()
                    .map(|event| MidiEvent::new(0, event.message)),
            );
        }

        // Publication and all fallible validation precede timeline mutation.
        // Therefore an error cannot emit a block without advancing the
        // absolute timeline and replay it on the next callback.
        self.prepare_fallback(input, frames);
        self.write_fallback_to_timeline(
            self.timeline_frame + max_frames as u64,
            frames,
            ExternalPluginHostBlockStatus::TimedOut,
        );
        let status = self.emit_timeline(output, frames);

        self.timeline_frame = self.timeline_frame.saturating_add(frames as u64);
        Ok((frames, status))
    }

    fn prepend_deferred_parameter_events(&mut self) -> Result<(), String> {
        let deferred_count = self
            .deferred_parameter_events
            .iter()
            .filter(|event| event.is_some())
            .count();
        if deferred_count == 0 {
            return Ok(());
        }
        let current_count = self.parameter_event_scratch.len();
        let total = current_count
            .checked_add(deferred_count)
            .ok_or_else(|| "external-plugin automation event count overflow".to_string())?;
        if total > self.parameter_event_scratch.capacity() {
            return Err(format!(
                "external-plugin deferred automation requires {total} events, capacity is {}",
                self.parameter_event_scratch.capacity()
            ));
        }
        self.parameter_event_scratch.resize(
            total,
            PluginIpcParameterEvent {
                sample_offset: 0,
                parameter_index: 0,
                value_tag: 0,
                value_bits: 0,
            },
        );
        self.parameter_event_scratch
            .copy_within(0..current_count, deferred_count);
        for (destination, event) in self.deferred_parameter_events.iter().flatten().enumerate() {
            self.parameter_event_scratch[destination] = *event;
        }
        Ok(())
    }

    fn resolve_pending(&mut self, callback_end: u64) -> Result<(), String> {
        let Some(mut pending) = self.pending else {
            return Ok(());
        };
        let output_end = pending.output_start.saturating_add(pending.frames as u64);
        match self.shared.worker_state() {
            PluginIpcState::WorkerReady if self.shared.worker_sequence() == pending.sequence => {
                let processed = self
                    .shared
                    .copy_worker_output(&mut self.worker_output_scratch)
                    .map_err(|err| format!("failed to copy external-plugin output: {err}"))?;
                self.shared.clear_block();
                self.pending = None;
                if processed != pending.frames {
                    return Err(format!(
                        "external-plugin worker returned {processed} frames for a {}-frame request",
                        pending.frames
                    ));
                }
                let write_start = self.timeline_frame.max(pending.output_start);
                if write_start < output_end {
                    let offset = (write_start - pending.output_start) as usize;
                    let frames = (output_end - write_start) as usize;
                    let channels = self.shared.layout().output_channels as usize;
                    let sample_offset = offset * channels;
                    self.write_worker_output_to_timeline(
                        write_start,
                        sample_offset,
                        frames,
                        ExternalPluginHostBlockStatus::Processed,
                    );
                }
            }
            PluginIpcState::WorkerReady => {
                self.wrong_sequence_count = self.wrong_sequence_count.saturating_add(1);
                self.mark_timeline_status(
                    self.timeline_frame.max(pending.output_start),
                    output_end,
                    ExternalPluginHostBlockStatus::WrongSequence,
                );
                self.shared.clear_block();
                self.pending = None;
            }
            PluginIpcState::WorkerFailed => {
                let status = if self.shared.worker_sequence() == pending.sequence {
                    self.worker_failure_count = self.worker_failure_count.saturating_add(1);
                    ExternalPluginHostBlockStatus::WorkerFailed
                } else {
                    self.wrong_sequence_count = self.wrong_sequence_count.saturating_add(1);
                    ExternalPluginHostBlockStatus::WrongSequence
                };
                self.mark_timeline_status(
                    self.timeline_frame.max(pending.output_start),
                    output_end,
                    status,
                );
                self.shared.clear_block();
                self.pending = None;
            }
            _ => {
                if !pending.timeout_counted && callback_end > pending.output_start {
                    self.timeout_count = self.timeout_count.saturating_add(1);
                    pending.timeout_counted = true;
                }
                self.pending = Some(pending);
            }
        }
        Ok(())
    }

    fn notify_worker(&self) {
        #[cfg(unix)]
        if let Some(socket) = self.notifier.as_ref() {
            let _ = socket.send_to(&[1], &self.wake_path);
        }
    }

    fn prepare_fallback(&mut self, input: &[f32], frames: usize) {
        let layout = self.shared.layout();
        let input_channels = layout.input_channels as usize;
        let output_channels = layout.output_channels as usize;
        let output_len = frames
            .saturating_mul(output_channels)
            .min(self.fallback_block.len());
        self.fallback_block[..output_len].fill(0.0);

        if output_channels == 0 {
            return;
        }
        let copy_channels = input_channels.min(output_channels);
        for frame in 0..frames {
            let src_base = frame * input_channels;
            let dst_base = frame * output_channels;
            if dst_base >= output_len {
                break;
            }
            for ch in 0..output_channels {
                let dry = if ch < copy_channels && src_base + ch < input.len() {
                    input[src_base + ch]
                } else {
                    0.0
                };
                if self.fallback_delay_frames == 0 {
                    self.fallback_block[dst_base + ch] = dry;
                } else {
                    let delay_index = self.fallback_delay_pos * output_channels + ch;
                    self.fallback_block[dst_base + ch] = self.fallback_delay[delay_index];
                    self.fallback_delay[delay_index] = dry;
                }
            }
            if self.fallback_delay_frames > 0 {
                self.fallback_delay_pos += 1;
                if self.fallback_delay_pos == self.fallback_delay_frames {
                    self.fallback_delay_pos = 0;
                }
            }
        }
    }

    fn write_fallback_to_timeline(
        &mut self,
        absolute_start: u64,
        frames: usize,
        status: ExternalPluginHostBlockStatus,
    ) {
        let channels = self.shared.layout().output_channels as usize;
        let capacity = self.timeline_status.len();
        for frame in 0..frames {
            let ring_frame = (absolute_start as usize + frame) % capacity;
            let dst = ring_frame * channels;
            let src = frame * channels;
            self.timeline_audio[dst..dst + channels]
                .copy_from_slice(&self.fallback_block[src..src + channels]);
            self.timeline_status[ring_frame] = status;
        }
    }

    fn write_worker_output_to_timeline(
        &mut self,
        absolute_start: u64,
        sample_offset: usize,
        frames: usize,
        status: ExternalPluginHostBlockStatus,
    ) {
        let channels = self.shared.layout().output_channels as usize;
        let capacity = self.timeline_status.len();
        for frame in 0..frames {
            let ring_frame = (absolute_start as usize + frame) % capacity;
            let dst = ring_frame * channels;
            let src = sample_offset + frame * channels;
            self.timeline_audio[dst..dst + channels]
                .copy_from_slice(&self.worker_output_scratch[src..src + channels]);
            self.timeline_status[ring_frame] = status;
        }
    }

    fn mark_timeline_status(
        &mut self,
        absolute_start: u64,
        absolute_end: u64,
        status: ExternalPluginHostBlockStatus,
    ) {
        let capacity = self.timeline_status.len();
        for frame in absolute_start..absolute_end {
            self.timeline_status[frame as usize % capacity] = status;
        }
    }

    fn emit_timeline(
        &mut self,
        output: &mut [f32],
        frames: usize,
    ) -> ExternalPluginHostBlockStatus {
        let channels = self.shared.layout().output_channels as usize;
        let capacity = self.timeline_status.len();
        let transition_total = self.pipeline_latency_samples().clamp(1, 64);
        let mut final_status = ExternalPluginHostBlockStatus::Priming;
        for frame in 0..frames {
            let ring_frame = (self.timeline_frame as usize + frame) % capacity;
            let status = self.timeline_status[ring_frame];
            let processed = status == ExternalPluginHostBlockStatus::Processed;
            if self.previous_status != Some(status) {
                if self.previous_status.is_some_and(|previous| {
                    previous != ExternalPluginHostBlockStatus::Priming
                        && (previous == ExternalPluginHostBlockStatus::Processed) != processed
                }) {
                    self.transition_from.copy_from_slice(&self.last_output);
                    self.transition_remaining = transition_total;
                }
                self.previous_status = Some(status);
            }
            let fade = if self.transition_remaining > 0 {
                let progressed = transition_total - self.transition_remaining + 1;
                self.transition_remaining -= 1;
                progressed as f32 / transition_total as f32
            } else {
                1.0
            };
            let src = ring_frame * channels;
            let dst = frame * channels;
            for ch in 0..channels {
                let sample = self.timeline_audio[src + ch];
                let sample = if fade < 1.0 {
                    self.transition_from[ch] * (1.0 - fade) + sample * fade
                } else {
                    sample
                };
                output[dst + ch] = sample;
                self.last_output[ch] = sample;
                self.timeline_audio[src + ch] = 0.0;
            }
            self.timeline_status[ring_frame] = ExternalPluginHostBlockStatus::Priming;
            final_status = status;
        }
        final_status
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_no_allocs;
    use crate::external_plugin_worker::ExternalPluginWorker;
    use crate::parameters::{Parameter, ParameterId, ParameterValue};
    use crate::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};

    static DEADLINE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

    struct AutomatedScalePlugin {
        factor: f32,
    }

    impl Plugin for AutomatedScalePlugin {
        fn info(&self) -> PluginInfo {
            PluginInfo::new("Automated Scale", "0.1", "test")
        }

        fn input_channels(&self) -> usize {
            1
        }

        fn output_channels(&self) -> usize {
            1
        }

        fn parameters(&self) -> Vec<Parameter> {
            vec![Parameter::new_float("factor", "Factor", 1.0, 0.0, 4.0)]
        }

        fn set_parameter(&mut self, _: ParameterId, value: ParameterValue) -> PluginResult<()> {
            if let ParameterValue::Float(value) = value {
                self.factor = value;
            }
            Ok(())
        }

        fn get_parameter(&self, _: &ParameterId) -> Option<ParameterValue> {
            Some(ParameterValue::Float(self.factor))
        }

        fn process(
            &mut self,
            input: &[f32],
            output: &mut [f32],
            context: &ProcessContext,
        ) -> PluginResult<usize> {
            let mut event_index = 0;
            let mut midi_index = 0;
            for frame in 0..context.num_frames {
                while let Some(event) = context.midi_events.get(midi_index)
                    && event.sample_offset == frame
                {
                    self.factor = 4.0;
                    midi_index += 1;
                }
                while let Some(event) = context.parameter_events.get(event_index)
                    && event.sample_offset == frame
                {
                    if let ParameterValue::Float(value) = &event.value {
                        self.factor = *value;
                    }
                    event_index += 1;
                }
                output[frame] = input[frame] * self.factor;
            }
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
    fn test_host_proxy_processes_worker_output() {
        let layout = PluginIpcLayout::new(48_000, 2, 2, 2).unwrap();
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
        let (_, status) = proxy
            .process_block_with(&input, &mut output, 2, || {
                let _ = worker.process_one();
            })
            .unwrap();
        assert_eq!(status, ExternalPluginHostBlockStatus::Priming);
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
    fn warmed_success_and_timeout_paths_allocate_nothing() {
        let layout = PluginIpcLayout::new(48_000, 1024, 2, 2).unwrap();
        let mut proxy = ExternalPluginHostProxy::new(layout, Duration::from_millis(10)).unwrap();
        let worker_shared = SecurePluginSharedMemory::open_existing(proxy.shared_path()).unwrap();
        let mut worker = ExternalPluginWorker::new(
            worker_shared,
            Box::new(ScalePlugin {
                channels: 2,
                factor: 2.0,
            }),
        )
        .unwrap();
        let input = [0.25_f32; 2048];
        let mut output = [0.0_f32; 2048];

        proxy
            .process_block_with(&input, &mut output, 128, || {
                worker.process_one().unwrap();
            })
            .unwrap();
        assert_no_allocs("external IPC warmed success", || {
            for frames in [128, 256, 511, 512, 1024].into_iter().cycle().take(40) {
                proxy
                    .process_block_with(&input, &mut output, frames, || {
                        worker.process_one().unwrap();
                    })
                    .unwrap();
            }
        });

        let timeout_shared = SecurePluginSharedMemory::create(layout).unwrap();
        let mut timeout_proxy =
            ExternalPluginHostProxy::from_shared(timeout_shared, Duration::ZERO);
        timeout_proxy
            .process_block(&input, &mut output, 128)
            .unwrap();
        assert_no_allocs("external IPC warmed timeout", || {
            for frames in [128, 256, 511, 512, 1024].into_iter().cycle().take(40) {
                timeout_proxy
                    .process_block(&input, &mut output, frames)
                    .unwrap();
            }
        });
    }

    #[test]
    fn max_frames_is_a_capacity_not_an_exact_callback_size() {
        let layout = PluginIpcLayout::new(48_000, 2, 1, 1).unwrap();
        let mut proxy = ExternalPluginHostProxy::new(layout, Duration::ZERO).unwrap();

        let mut short_output = [0.0_f32; 1];
        proxy.process_block(&[0.25], &mut short_output, 1).unwrap();

        let mut oversized_output = [0.0_f32; 3];
        let error = proxy
            .process_block(&[0.25; 3], &mut oversized_output, 3)
            .unwrap_err();
        assert_eq!(error, "external-plugin callback has 3 frames, maximum is 2");
    }

    #[test]
    fn invalid_public_buffers_do_not_advance_or_replay_the_timeline() {
        let layout = PluginIpcLayout::new(48_000, 2, 1, 1).unwrap();
        let mut proxy = ExternalPluginHostProxy::new(layout, Duration::ZERO).unwrap();
        let error = proxy
            .process_block(&[0.25, 0.5], &mut [0.0; 1], 2)
            .unwrap_err();
        assert!(error.contains("output has 1 samples"), "{error}");

        let mut output = [9.0; 2];
        assert_eq!(
            proxy.process_block(&[0.25, 0.5], &mut output, 2).unwrap().1,
            ExternalPluginHostBlockStatus::Priming
        );
        assert_eq!(output, [0.0, 0.0]);
        proxy.process_block(&[0.75, 1.0], &mut output, 2).unwrap();
        assert_eq!(output, [0.25, 0.5]);
    }

    #[test]
    fn automation_from_a_skipped_callback_is_applied_on_worker_recovery() {
        let layout = PluginIpcLayout::new(48_000, 2, 1, 1).unwrap();
        let mut proxy = ExternalPluginHostProxy::new(layout, Duration::ZERO).unwrap();
        proxy.configure_parameters(vec![ParameterId::from("factor")]);
        let worker_shared = SecurePluginSharedMemory::open_existing(proxy.shared_path()).unwrap();
        let mut worker = ExternalPluginWorker::new(
            worker_shared,
            Box::new(AutomatedScalePlugin { factor: 1.0 }),
        )
        .unwrap();
        let input = [0.25_f32; 2];
        let mut output = [0.0_f32; 2];

        let event = crate::plugin::ParameterEvent::new(
            1,
            ParameterId::from("factor"),
            ParameterValue::Float(3.0),
        );
        let events = [event];
        let context = ProcessContext::new(48_000, 2).with_parameter_events(&events);
        assert_no_allocs("external IPC deferred automation recovery", || {
            // Submit block one but leave it in flight.
            proxy.process_block(&input, &mut output, 2).unwrap();

            // This callback cannot use the occupied slot. Its final automation
            // state must not disappear just because its audio uses fallback.
            proxy
                .process_block_with_context(&input, &mut output, &context)
                .unwrap();

            // Finish block one, then submit and finish the recovery block. The
            // deferred value is injected at offset zero of that recovery block.
            worker.process_one().unwrap();
            proxy
                .process_block_with(&input, &mut output, 2, || {
                    worker.process_one().unwrap();
                })
                .unwrap();
            proxy.process_block(&input, &mut output, 2).unwrap();
        });
        assert_eq!(output, [0.5, 0.75]);
    }

    #[test]
    fn midi_from_a_skipped_callback_is_applied_on_worker_recovery() {
        let layout = PluginIpcLayout::new(48_000, 2, 1, 1).unwrap();
        let mut proxy = ExternalPluginHostProxy::new(layout, Duration::ZERO).unwrap();
        let worker_shared = SecurePluginSharedMemory::open_existing(proxy.shared_path()).unwrap();
        let mut worker = ExternalPluginWorker::new(
            worker_shared,
            Box::new(AutomatedScalePlugin { factor: 1.0 }),
        )
        .unwrap();
        let input = [0.25_f32; 2];
        let mut output = [0.0_f32; 2];
        let midi_events = [MidiEvent::new(
            1,
            crate::plugin::MidiMessage::note_on(0, 60, 127),
        )];
        let context = ProcessContext::new(48_000, 2).with_midi_events(&midi_events);

        assert_no_allocs("external IPC deferred MIDI recovery", || {
            proxy.process_block(&input, &mut output, 2).unwrap();
            proxy
                .process_block_with_context(&input, &mut output, &context)
                .unwrap();
            worker.process_one().unwrap();
            proxy
                .process_block_with(&input, &mut output, 2, || {
                    worker.process_one().unwrap();
                })
                .unwrap();
            proxy.process_block(&input, &mut output, 2).unwrap();
        });
        assert_eq!(output, [0.625, 1.0]);
    }

    #[test]
    fn variable_callbacks_keep_constant_latency_across_ring_wraps() {
        const MAX_FRAMES: usize = 1024;
        let layout = PluginIpcLayout::new(48_000, MAX_FRAMES as u32, 1, 1).unwrap();
        let mut proxy = ExternalPluginHostProxy::new(layout, Duration::ZERO).unwrap();
        let worker_shared = SecurePluginSharedMemory::open_existing(proxy.shared_path()).unwrap();
        let mut worker = ExternalPluginWorker::new(
            worker_shared,
            Box::new(ScalePlugin {
                channels: 1,
                factor: 2.0,
            }),
        )
        .unwrap();
        assert_eq!(proxy.pipeline_latency_samples(), MAX_FRAMES);

        let partitions = [128, 256, 511, 512, 1024, 511, 128, 1024, 256, 512];
        let mut absolute_frame = 0_usize;
        for frames in partitions.into_iter().cycle().take(30) {
            let input = (0..frames)
                .map(|offset| ((absolute_frame + offset) % 97) as f32 / 97.0)
                .collect::<Vec<_>>();
            let mut output = vec![0.0_f32; frames];
            proxy
                .process_block_with(&input, &mut output, frames, || {
                    worker.process_one().unwrap();
                })
                .unwrap();
            for (offset, sample) in output.into_iter().enumerate() {
                let timeline_frame = absolute_frame + offset;
                let expected = if timeline_frame < MAX_FRAMES {
                    0.0
                } else {
                    2.0 * ((timeline_frame - MAX_FRAMES) % 97) as f32 / 97.0
                };
                assert!(
                    (sample - expected).abs() < 1.0e-6,
                    "frame={timeline_frame}, frames={frames}, sample={sample}, expected={expected}"
                );
            }
            absolute_frame += frames;
        }
    }

    #[test]
    fn variable_timeout_fallback_has_the_same_constant_latency() {
        const MAX_FRAMES: usize = 1024;
        let layout = PluginIpcLayout::new(48_000, MAX_FRAMES as u32, 1, 1).unwrap();
        let mut proxy = ExternalPluginHostProxy::new(layout, Duration::ZERO).unwrap();
        let partitions = [128, 256, 511, 512, 1024, 256, 128, 511];
        let mut absolute_frame = 0_usize;
        for frames in partitions.into_iter().cycle().take(24) {
            let input = (0..frames)
                .map(|offset| ((absolute_frame + offset) % 89) as f32 / 89.0)
                .collect::<Vec<_>>();
            let mut output = vec![0.0_f32; frames];
            proxy.process_block(&input, &mut output, frames).unwrap();
            for (offset, sample) in output.into_iter().enumerate() {
                let timeline_frame = absolute_frame + offset;
                let expected = if timeline_frame < MAX_FRAMES {
                    0.0
                } else {
                    ((timeline_frame - MAX_FRAMES) % 89) as f32 / 89.0
                };
                assert!((sample - expected).abs() < 1.0e-6);
            }
            absolute_frame += frames;
        }
    }

    #[test]
    fn test_host_proxy_times_out_to_passthrough() {
        let layout = PluginIpcLayout::new(48_000, 2, 2, 2).unwrap();
        let mut proxy = ExternalPluginHostProxy::new(layout, Duration::ZERO).unwrap();

        let input = vec![0.25, -0.5, 1.0, -1.0];
        let mut output = vec![0.0; input.len()];
        let (_, first) = proxy.process_block(&input, &mut output, 2).unwrap();
        assert_eq!(first, ExternalPluginHostBlockStatus::Priming);
        let (frames, status) = proxy.process_block(&input, &mut output, 2).unwrap();

        assert_eq!(frames, 2);
        assert_eq!(status, ExternalPluginHostBlockStatus::TimedOut);
        assert_eq!(output, input);
        assert_eq!(proxy.timeout_count(), 1);
    }

    #[test]
    fn test_host_proxy_worker_failure_falls_back() {
        let layout = PluginIpcLayout::new(48_000, 2, 2, 2).unwrap();
        let mut proxy = ExternalPluginHostProxy::new(layout, Duration::from_millis(10)).unwrap();
        let worker_shared = SecurePluginSharedMemory::open_existing(proxy.shared_path()).unwrap();
        let mut worker =
            ExternalPluginWorker::new(worker_shared, Box::new(PanickingPlugin { channels: 2 }))
                .unwrap();

        let input = vec![0.25, -0.5, 1.0, -1.0];
        let mut output = vec![0.0; input.len()];
        let (_, first) = proxy
            .process_block_with(&input, &mut output, 2, || {
                let _ = worker.process_one();
            })
            .unwrap();
        assert_eq!(first, ExternalPluginHostBlockStatus::Priming);
        let (frames, status) = proxy.process_block(&input, &mut output, 2).unwrap();

        assert_eq!(frames, 2);
        assert_eq!(status, ExternalPluginHostBlockStatus::WorkerFailed);
        assert_eq!(output, input);
        assert_eq!(proxy.worker_failure_count(), 1);
    }

    #[test]
    fn test_timeout_fallback_preserves_reported_latency() {
        let layout = PluginIpcLayout::new(48_000, 8, 1, 1).unwrap();
        let mut proxy = ExternalPluginHostProxy::new(layout, Duration::ZERO).unwrap();
        proxy.configure_fallback_latency(3).unwrap();

        let input = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mut output = [0.0; 8];
        let (_, first) = proxy.process_block(&input, &mut output, 8).unwrap();
        assert_eq!(first, ExternalPluginHostBlockStatus::Priming);
        let (_, status) = proxy.process_block(&input, &mut output, 8).unwrap();

        assert_eq!(status, ExternalPluginHostBlockStatus::TimedOut);
        assert_eq!(output, [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_processed_to_fallback_transition_has_bounded_discontinuity() {
        let layout = PluginIpcLayout::new(48_000, 64, 1, 1).unwrap();
        let mut proxy = ExternalPluginHostProxy::new(layout, Duration::from_millis(10)).unwrap();
        let worker_shared = SecurePluginSharedMemory::open_existing(proxy.shared_path()).unwrap();
        let mut worker = ExternalPluginWorker::new(
            worker_shared,
            Box::new(ScalePlugin {
                channels: 1,
                factor: 1.0,
            }),
        )
        .unwrap();
        let input = [1.0; 64];
        let mut output = [0.0; 64];
        proxy
            .process_block_with(&input, &mut output, 64, || {
                let _ = worker.process_one();
            })
            .unwrap();

        let fallback_input = [-1.0; 64];
        proxy
            .process_block(&fallback_input, &mut output, 64)
            .unwrap();
        // The second callback returns the genuinely processed first block and
        // submits the fallback block. The third callback observes that missed
        // request and transitions to its latency-matched fallback.
        assert_eq!(output, [1.0; 64]);
        proxy
            .process_block(&fallback_input, &mut output, 64)
            .unwrap();

        let max_step = std::iter::once((output[0] - 1.0).abs())
            .chain(output.windows(2).map(|pair| (pair[1] - pair[0]).abs()))
            .fold(0.0_f32, f32::max);
        assert!(max_step <= 2.0 / 64.0 + f32::EPSILON, "{max_step}");
        assert_eq!(output[63], -1.0);
    }

    #[test]
    fn callback_submits_once_and_never_polls_in_a_loop() {
        let _guard = DEADLINE_TEST_LOCK.lock().unwrap();
        let layout = PluginIpcLayout::new(48_000, 16, 1, 1).unwrap();
        let mut proxy = ExternalPluginHostProxy::new(layout, Duration::from_millis(2)).unwrap();
        let mut output = [0.0; 16];
        let mut worker_steps = 0;
        let (_, status) = proxy
            .process_block_with(&[0.0; 16], &mut output, 16, || worker_steps += 1)
            .unwrap();
        assert_eq!(status, ExternalPluginHostBlockStatus::Priming);
        assert_eq!(worker_steps, 1);
    }

    #[test]
    fn late_worker_recovery_discards_stale_result_and_resumes_pipeline() {
        let _guard = DEADLINE_TEST_LOCK.lock().unwrap();
        let layout = PluginIpcLayout::new(48_000, 16, 1, 1).unwrap();
        let mut proxy = ExternalPluginHostProxy::new(layout, Duration::ZERO).unwrap();
        let worker_shared = SecurePluginSharedMemory::open_existing(proxy.shared_path()).unwrap();
        let mut worker = ExternalPluginWorker::new(
            worker_shared,
            Box::new(ScalePlugin {
                channels: 1,
                factor: 2.0,
            }),
        )
        .unwrap();
        let input = [0.25_f32; 16];
        let mut output = [0.0_f32; 16];
        assert_eq!(
            proxy.process_block(&input, &mut output, 16).unwrap().1,
            ExternalPluginHostBlockStatus::Priming
        );
        assert_eq!(
            proxy.process_block(&input, &mut output, 16).unwrap().1,
            ExternalPluginHostBlockStatus::TimedOut
        );
        worker.process_one().unwrap();
        assert_eq!(
            proxy.process_block(&input, &mut output, 16).unwrap().1,
            ExternalPluginHostBlockStatus::TimedOut
        );
        worker.process_one().unwrap();
        assert_eq!(
            proxy.process_block(&input, &mut output, 16).unwrap().1,
            ExternalPluginHostBlockStatus::Processed
        );
        assert!((output[15] - 0.5).abs() < 1e-6);
        assert!(
            output
                .windows(2)
                .all(|pair| (pair[1] - pair[0]).abs() <= 0.5 / 16.0 + 1e-6)
        );
    }
}
