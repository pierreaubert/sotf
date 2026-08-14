//! Bounded asynchronous adapter for expensive plugins hosted directly by a
//! physical audio callback.
//!
//! The callback only moves preallocated blocks through SPSC queues. The DSP
//! worker owns the wrapped plugin for its entire lifetime. `Drop` joins that
//! worker and therefore belongs on a control thread, never the audio thread.
//!
//! This adapter is intentionally scoped to direct-format, transport-agnostic
//! DSP such as Linear Phase EQ. A quantum assembled from several physical
//! callbacks preserves the first fragment's tempo/play-state metadata, and it
//! does not forward MIDI or note-expression events. Plugins whose signal
//! behavior depends on those streams need a dedicated timestamped adapter.

use crate::param_specs::UpdateMode;
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use crate::plugin::{
    Plugin, PluginCostClass, PluginInfo, PluginResult, ProcessContext, TransportInfo,
};
use rtrb::{Consumer, Producer, RingBuffer};
use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle, Thread};
use std::time::Duration;

const BLOCK_POOL_SIZE: usize = 8;
const EVENT_QUEUE_CAPACITY: usize = 4096;
const MAX_PREALLOCATED_SAMPLES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy)]
enum PrimitiveValue {
    Float(f32),
    Int(i32),
    Bool(bool),
}

impl PrimitiveValue {
    fn from_parameter(value: &ParameterValue) -> Option<Self> {
        match value {
            ParameterValue::Float(value) => Some(Self::Float(*value)),
            ParameterValue::Int(value) => Some(Self::Int(*value)),
            ParameterValue::Bool(value) => Some(Self::Bool(*value)),
            ParameterValue::String(_) => None,
        }
    }

    fn into_parameter(self) -> ParameterValue {
        match self {
            Self::Float(value) => ParameterValue::Float(value),
            Self::Int(value) => ParameterValue::Int(value),
            Self::Bool(value) => ParameterValue::Bool(value),
        }
    }
}

fn parameter_accepts_value(parameter: &Parameter, value: &ParameterValue) -> bool {
    match (&parameter.default_value, value) {
        (ParameterValue::Float(_), ParameterValue::Float(value)) => {
            value.is_finite()
                && !matches!(parameter.min_value, Some(ParameterValue::Float(min)) if *value < min)
                && !matches!(parameter.max_value, Some(ParameterValue::Float(max)) if *value > max)
        }
        (ParameterValue::Int(_), ParameterValue::Int(value)) => {
            !matches!(parameter.min_value, Some(ParameterValue::Int(min)) if *value < min)
                && !matches!(parameter.max_value, Some(ParameterValue::Int(max)) if *value > max)
        }
        (ParameterValue::Bool(_), ParameterValue::Bool(_)) => true,
        // Realtime string values are intentionally unsupported by the queue.
        _ => false,
    }
}

#[derive(Clone, Copy)]
struct TimelineEvent {
    epoch: u64,
    absolute_frame: u64,
    parameter_index: u16,
    value: PrimitiveValue,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum QueueEventError {
    Invalid,
    Saturated,
}

struct InputBlock {
    epoch: u64,
    start_frame: u64,
    filled_frames: usize,
    transport: TransportInfo,
    samples: Vec<f32>,
}

impl InputBlock {
    fn new(sample_count: usize) -> Self {
        Self {
            epoch: 0,
            start_frame: 0,
            filled_frames: 0,
            transport: TransportInfo::default(),
            samples: vec![0.0; sample_count],
        }
    }
}

struct OutputBlock {
    epoch: u64,
    start_frame: u64,
    samples: Vec<f32>,
}

impl OutputBlock {
    fn new(sample_count: usize) -> Self {
        Self {
            epoch: 0,
            start_frame: 0,
            samples: vec![0.0; sample_count],
        }
    }
}

struct AsyncMetadata {
    info: PluginInfo,
    parameters: Vec<Parameter>,
    values: Vec<ParameterValue>,
    input_channels: usize,
    output_channels: usize,
    sample_rate: u32,
    max_callback_frames: usize,
    quantum_frames: usize,
    inner_latency: usize,
    cost_class: PluginCostClass,
}

struct AudioQueues {
    jobs: Producer<InputBlock>,
    input_recycle: Consumer<InputBlock>,
    ready_output: Consumer<OutputBlock>,
    output_recycle: Producer<OutputBlock>,
    events: Producer<TimelineEvent>,
}

struct AudioTimeline {
    active_input: Option<InputBlock>,
    active_output: Option<OutputBlock>,
    deferred_output_recycle: Vec<OutputBlock>,
    absolute_input_frame: u64,
    absolute_emitted_frame: u64,
    epoch: u64,
    terminal_epoch: bool,
}

struct WorkerHandle {
    requested_epoch: Arc<AtomicU64>,
    next_needed_source_frame: Arc<AtomicU64>,
    completed_frame: Arc<AtomicU64>,
    completed_epoch: Arc<AtomicU64>,
    faulted: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    thread: Thread,
    join: Option<JoinHandle<Box<dyn Plugin>>>,
}

/// Adapts a fixed-rate/fixed-frame-count plugin to a physical realtime callback.
pub struct AsyncTimelinePlugin {
    metadata: AsyncMetadata,
    queues: AudioQueues,
    timeline: AudioTimeline,
    worker: WorkerHandle,
}

impl AsyncTimelinePlugin {
    pub fn new(
        mut inner: Box<dyn Plugin>,
        sample_rate: u32,
        max_callback_frames: usize,
    ) -> PluginResult<Self> {
        if sample_rate == 0 || max_callback_frames == 0 {
            return Err(
                "async adapter requires a non-zero sample rate and callback maximum".into(),
            );
        }
        // Initialization is exact-once and precedes every derived contract:
        // some plugins finalize channel/latency/quantum metadata here.
        inner.initialize(sample_rate)?;
        let input_channels = inner.input_channels();
        let output_channels = inner.output_channels();
        if input_channels == 0 || output_channels == 0 {
            return Err("async adapter requires non-zero channel counts".into());
        }
        if inner.output_sample_rate(sample_rate) != sample_rate
            || inner.output_frames_for_input(max_callback_frames) != max_callback_frames
        {
            return Err("async adapter requires fixed-rate, frame-preserving DSP".into());
        }
        let quantum_frames = inner
            .realtime_quantum_frames()
            .max(max_callback_frames)
            .max(1);
        let adapter_latency = quantum_frames
            .checked_mul(2)
            .ok_or_else(|| "async adapter latency overflow".to_string())?;
        let inner_latency = inner.latency_samples();
        inner_latency
            .checked_add(adapter_latency)
            .ok_or_else(|| "async adapter total latency overflow".to_string())?;
        let input_samples = quantum_frames
            .checked_mul(input_channels)
            .ok_or_else(|| "async adapter input allocation overflow".to_string())?;
        let output_samples = quantum_frames
            .checked_mul(output_channels)
            .ok_or_else(|| "async adapter output allocation overflow".to_string())?;
        let pool_samples = input_samples
            .checked_add(output_samples)
            .and_then(|samples| samples.checked_mul(BLOCK_POOL_SIZE))
            .ok_or_else(|| "async adapter pool allocation overflow".to_string())?;
        if pool_samples > MAX_PREALLOCATED_SAMPLES {
            return Err("async adapter negotiated buffer exceeds its preallocation bound".into());
        }

        let info = inner.info();
        let parameters = inner.parameters();
        if parameters.len() > u16::MAX as usize {
            return Err("async adapter supports at most 65535 parameters".into());
        }
        let values = parameters
            .iter()
            .map(|parameter| {
                inner
                    .get_parameter(&parameter.id)
                    .unwrap_or_else(|| parameter.default_value.clone())
            })
            .collect();
        let cost_class = inner.cost_class();
        let (jobs, worker_jobs) = RingBuffer::new(BLOCK_POOL_SIZE);
        let (mut worker_input_recycle, input_recycle) = RingBuffer::new(BLOCK_POOL_SIZE);
        let (worker_ready_output, ready_output) = RingBuffer::new(BLOCK_POOL_SIZE);
        let (mut output_recycle, worker_output_recycle) = RingBuffer::new(BLOCK_POOL_SIZE);
        let (events, worker_events) = RingBuffer::new(EVENT_QUEUE_CAPACITY);

        let active_input = InputBlock::new(input_samples);
        for _ in 1..BLOCK_POOL_SIZE {
            worker_input_recycle
                .push(InputBlock::new(input_samples))
                .map_err(|_| "async adapter input pool initialization failed".to_string())?;
        }
        for _ in 0..BLOCK_POOL_SIZE {
            output_recycle
                .push(OutputBlock::new(output_samples))
                .map_err(|_| "async adapter output pool initialization failed".to_string())?;
        }

        let requested_epoch = Arc::new(AtomicU64::new(0));
        let next_needed_source_frame = Arc::new(AtomicU64::new(0));
        let completed_frame = Arc::new(AtomicU64::new(0));
        let completed_epoch = Arc::new(AtomicU64::new(0));
        let faulted = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_epoch = Arc::clone(&requested_epoch);
        let worker_needed = Arc::clone(&next_needed_source_frame);
        let worker_completed = Arc::clone(&completed_frame);
        let worker_completed_epoch = Arc::clone(&completed_epoch);
        let worker_faulted = Arc::clone(&faulted);
        let worker_stop = Arc::clone(&stop);
        let parameter_ids: Vec<ParameterId> = parameters
            .iter()
            .map(|parameter| parameter.id.clone())
            .collect();
        let join = thread::Builder::new()
            .name("sotf-async-plugin".into())
            .spawn(move || {
                let mut inner = inner;
                let panic_faulted = Arc::clone(&worker_faulted);
                let result = catch_unwind(AssertUnwindSafe(|| {
                    run_worker(
                        inner.as_mut(),
                        sample_rate,
                        quantum_frames,
                        input_channels,
                        output_channels,
                        parameter_ids,
                        worker_jobs,
                        worker_input_recycle,
                        worker_ready_output,
                        worker_output_recycle,
                        worker_events,
                        worker_epoch,
                        worker_needed,
                        worker_completed,
                        worker_completed_epoch,
                        worker_faulted,
                        worker_stop,
                    );
                }));
                if result.is_err() {
                    panic_faulted.store(true, Ordering::Release);
                }
                inner
            })
            .map_err(|error| format!("failed to spawn async DSP worker: {error}"))?;
        let worker_thread = join.thread().clone();

        Ok(Self {
            metadata: AsyncMetadata {
                info,
                parameters,
                values,
                input_channels,
                output_channels,
                sample_rate,
                max_callback_frames,
                quantum_frames,
                inner_latency,
                cost_class,
            },
            queues: AudioQueues {
                jobs,
                input_recycle,
                ready_output,
                output_recycle,
                events,
            },
            timeline: AudioTimeline {
                active_input: Some(active_input),
                active_output: None,
                deferred_output_recycle: Vec::with_capacity(BLOCK_POOL_SIZE),
                absolute_input_frame: 0,
                absolute_emitted_frame: 0,
                epoch: 0,
                terminal_epoch: false,
            },
            worker: WorkerHandle {
                requested_epoch,
                next_needed_source_frame,
                completed_frame,
                completed_epoch,
                faulted,
                stop,
                thread: worker_thread,
                join: Some(join),
            },
        })
    }

    pub const fn quantum_frames(&self) -> usize {
        self.metadata.quantum_frames
    }

    pub const fn max_callback_frames(&self) -> usize {
        self.metadata.max_callback_frames
    }

    pub const fn adapter_latency_frames(&self) -> usize {
        self.metadata.quantum_frames * 2
    }

    /// Highest exclusive source-frame boundary completed by the worker.
    /// This is diagnostic state; an audio callback must never wait on it.
    pub fn completed_frame(&self) -> u64 {
        self.worker.completed_frame.load(Ordering::Acquire)
    }

    /// Epoch paired with [`Self::completed_frame`]. Tests and diagnostics must
    /// compare both values across reset boundaries.
    pub fn completed_epoch(&self) -> u64 {
        self.worker.completed_epoch.load(Ordering::Acquire)
    }

    fn recycle_output(&mut self, block: OutputBlock) {
        match self.queues.output_recycle.push(block) {
            Ok(()) => {}
            Err(rtrb::PushError::Full(block)) => {
                // Pool conservation bounds this to BLOCK_POOL_SIZE. Capacity is
                // reserved during construction, so callback retention cannot
                // allocate or destroy the block.
                debug_assert!(self.timeline.deferred_output_recycle.len() < BLOCK_POOL_SIZE);
                self.timeline.deferred_output_recycle.push(block);
            }
        }
    }

    fn retry_deferred_recycle(&mut self) {
        while let Some(block) = self.timeline.deferred_output_recycle.pop() {
            if let Err(rtrb::PushError::Full(block)) = self.queues.output_recycle.push(block) {
                self.timeline.deferred_output_recycle.push(block);
                break;
            }
        }
    }

    fn emit_output(&mut self, output: &mut [f32], frames: usize) {
        output.fill(0.0);
        self.retry_deferred_recycle();
        let channels = self.metadata.output_channels;
        let latency = self.adapter_latency_frames() as u64;
        let mut output_frame = 0usize;
        while output_frame < frames {
            let emitted = self.timeline.absolute_emitted_frame + output_frame as u64;
            if emitted < latency {
                let silence = ((latency - emitted) as usize).min(frames - output_frame);
                output_frame += silence;
                continue;
            }
            let wanted = emitted - latency;
            if self.timeline.active_output.is_none() {
                while let Ok(block) = self.queues.ready_output.pop() {
                    let block_end = block.start_frame + self.metadata.quantum_frames as u64;
                    if block.epoch != self.timeline.epoch || block_end <= wanted {
                        self.recycle_output(block);
                        if !self.timeline.deferred_output_recycle.is_empty() {
                            break;
                        }
                    } else {
                        self.timeline.active_output = Some(block);
                        break;
                    }
                }
            }
            let Some(block) = self.timeline.active_output.as_ref() else {
                break;
            };
            if block.start_frame > wanted {
                let silence = ((block.start_frame - wanted) as usize).min(frames - output_frame);
                output_frame += silence;
                continue;
            }
            let offset = (wanted - block.start_frame) as usize;
            if offset >= self.metadata.quantum_frames {
                let block = self
                    .timeline
                    .active_output
                    .take()
                    .expect("active output exists");
                self.recycle_output(block);
                continue;
            }
            let copied_frames = (self.metadata.quantum_frames - offset).min(frames - output_frame);
            let source_start = offset * channels;
            let destination_start = output_frame * channels;
            let sample_count = copied_frames * channels;
            output[destination_start..destination_start + sample_count]
                .copy_from_slice(&block.samples[source_start..source_start + sample_count]);
            output_frame += copied_frames;
            if offset + copied_frames == self.metadata.quantum_frames {
                let block = self
                    .timeline
                    .active_output
                    .take()
                    .expect("active output exists");
                self.recycle_output(block);
            }
        }
        self.timeline.absolute_emitted_frame += frames as u64;
        let needed = self.timeline.absolute_emitted_frame.saturating_sub(latency);
        self.worker
            .next_needed_source_frame
            .store(needed, Ordering::Release);
    }

    fn enqueue_input(&mut self, input: &[f32], context: &ProcessContext<'_>) {
        let channels = self.metadata.input_channels;
        let mut source_frame = 0usize;
        while source_frame < context.num_frames {
            if self.timeline.active_input.is_none() {
                self.timeline.active_input = self.queues.input_recycle.pop().ok();
                if self.timeline.active_input.is_none() {
                    self.timeline.absolute_input_frame +=
                        (context.num_frames - source_frame) as u64;
                    return;
                }
            }
            let block = self
                .timeline
                .active_input
                .as_mut()
                .expect("active input exists");
            if block.filled_frames == 0 {
                block.epoch = self.timeline.epoch;
                block.start_frame = self.timeline.absolute_input_frame;
                block.transport = context.transport;
                block.transport.sample_position = block.start_frame;
            }
            let copied_frames = (self.metadata.quantum_frames - block.filled_frames)
                .min(context.num_frames - source_frame);
            let source_start = source_frame * channels;
            let destination_start = block.filled_frames * channels;
            let sample_count = copied_frames * channels;
            block.samples[destination_start..destination_start + sample_count]
                .copy_from_slice(&input[source_start..source_start + sample_count]);
            block.filled_frames += copied_frames;
            source_frame += copied_frames;
            self.timeline.absolute_input_frame += copied_frames as u64;
            if block.filled_frames == self.metadata.quantum_frames {
                let full = self
                    .timeline
                    .active_input
                    .take()
                    .expect("active input exists");
                match self.queues.jobs.push(full) {
                    Ok(()) => self.worker.thread.unpark(),
                    Err(rtrb::PushError::Full(mut full)) => {
                        full.filled_frames = 0;
                        self.timeline.active_input = Some(full);
                    }
                }
            }
        }
    }

    fn queue_context_events(
        &mut self,
        context: &ProcessContext<'_>,
    ) -> Result<(), QueueEventError> {
        if context.parameter_events.len() > self.queues.events.slots() {
            return Err(QueueEventError::Saturated);
        }
        let mut previous = 0usize;
        for (event_number, event) in context.parameter_events.iter().enumerate() {
            if event.sample_offset >= context.num_frames
                || (event_number != 0 && event.sample_offset < previous)
            {
                return Err(QueueEventError::Invalid);
            }
            previous = event.sample_offset;
            let Some(parameter_index) = self
                .metadata
                .parameters
                .iter()
                .position(|parameter| parameter.id == event.parameter_id)
            else {
                return Err(QueueEventError::Invalid);
            };
            if PrimitiveValue::from_parameter(&event.value).is_none()
                || self.metadata.parameters[parameter_index].update_mode != UpdateMode::Realtime
                || !parameter_accepts_value(
                    &self.metadata.parameters[parameter_index],
                    &event.value,
                )
            {
                return Err(QueueEventError::Invalid);
            }
        }
        for event in context.parameter_events {
            let parameter_index = self
                .metadata
                .parameters
                .iter()
                .position(|parameter| parameter.id == event.parameter_id)
                .expect("validated parameter event");
            let timeline_event = TimelineEvent {
                epoch: self.timeline.epoch,
                absolute_frame: self.timeline.absolute_input_frame + event.sample_offset as u64,
                parameter_index: parameter_index as u16,
                value: PrimitiveValue::from_parameter(&event.value)
                    .expect("validated primitive parameter event"),
            };
            if self.queues.events.push(timeline_event).is_err() {
                return Err(QueueEventError::Saturated);
            }
            self.metadata.values[parameter_index] = event.value.clone();
        }
        Ok(())
    }

    fn advance_silent_overload(&mut self, output: &mut [f32], frames: usize) {
        output.fill(0.0);
        self.timeline.absolute_emitted_frame += frames as u64;
        let latency = self.adapter_latency_frames() as u64;
        self.worker.next_needed_source_frame.store(
            self.timeline.absolute_emitted_frame.saturating_sub(latency),
            Ordering::Release,
        );
        // Do not join pre-overload samples to post-overload samples in one Q:
        // the worker must see an explicit absolute gap, never a shifted block.
        if let Some(block) = self.timeline.active_input.as_mut() {
            block.filled_frames = 0;
        }
        self.timeline.absolute_input_frame += frames as u64;
    }
}

impl Drop for AsyncTimelinePlugin {
    fn drop(&mut self) {
        self.worker.stop.store(true, Ordering::Release);
        self.worker.thread.unpark();
        if let Some(join) = self.worker.join.take()
            && let Ok(inner) = join.join()
        {
            // Plugin-owned native resources are destroyed on this control
            // thread, after the worker has stopped touching them.
            drop(inner);
        }
    }
}

impl Plugin for AsyncTimelinePlugin {
    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }

    fn info(&self) -> PluginInfo {
        self.metadata.info.clone()
    }

    fn input_channels(&self) -> usize {
        self.metadata.input_channels
    }

    fn output_channels(&self) -> usize {
        self.metadata.output_channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.metadata.parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let Some(parameter_index) = self
            .metadata
            .parameters
            .iter()
            .position(|parameter| parameter.id == id)
        else {
            return Err(String::new());
        };
        if self.worker.faulted.load(Ordering::Acquire) {
            return Err(String::new());
        }
        if self.metadata.parameters[parameter_index].update_mode != UpdateMode::Realtime {
            return Err(String::new());
        }
        if !parameter_accepts_value(&self.metadata.parameters[parameter_index], &value) {
            return Err(String::new());
        }
        let Some(primitive) = PrimitiveValue::from_parameter(&value) else {
            return Err(String::new());
        };
        self.queues
            .events
            .push(TimelineEvent {
                epoch: self.timeline.epoch,
                absolute_frame: self.timeline.absolute_input_frame,
                parameter_index: parameter_index as u16,
                value: primitive,
            })
            .map_err(|_| String::new())?;
        self.metadata.values[parameter_index] = value;
        self.worker.thread.unpark();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        self.metadata
            .parameters
            .iter()
            .position(|parameter| parameter.id == *id)
            .map(|index| self.metadata.values[index].clone())
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if sample_rate == self.metadata.sample_rate {
            Ok(())
        } else {
            Err("async adapter sample-rate changes require reconstruction".into())
        }
    }

    fn reset(&mut self) {
        if self.timeline.epoch == u64::MAX - 1 {
            // Reserve the terminal epoch rather than permitting ABA after a
            // wrap. This can only occur after 2^64-1 resets; fail silent.
            self.timeline.epoch = u64::MAX;
            self.timeline.terminal_epoch = true;
            self.worker.stop.store(true, Ordering::Release);
        } else if !self.timeline.terminal_epoch {
            self.timeline.epoch += 1;
        }
        self.timeline.absolute_input_frame = 0;
        self.timeline.absolute_emitted_frame = 0;
        if let Some(mut block) = self.timeline.active_input.take() {
            block.filled_frames = 0;
            block.epoch = self.timeline.epoch;
            self.timeline.active_input = Some(block);
        }
        if let Some(block) = self.timeline.active_output.take() {
            self.recycle_output(block);
        }
        self.worker
            .requested_epoch
            .store(self.timeline.epoch, Ordering::Release);
        self.worker
            .next_needed_source_frame
            .store(0, Ordering::Release);
        self.worker.completed_frame.store(0, Ordering::Release);
        self.worker
            .completed_epoch
            .store(self.timeline.epoch, Ordering::Release);
        self.worker.thread.unpark();
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext<'_>,
    ) -> PluginResult<usize> {
        let expected_input = context.num_frames.checked_mul(self.metadata.input_channels);
        let expected_output = context
            .num_frames
            .checked_mul(self.metadata.output_channels);
        if context.num_frames > self.metadata.max_callback_frames
            || context.sample_rate != self.metadata.sample_rate
            || expected_input != Some(input.len())
            || expected_output != Some(output.len())
            || input.iter().any(|sample| !sample.is_finite())
        {
            output.fill(0.0);
            return Err(String::new());
        }
        match self.queue_context_events(context) {
            Ok(()) => {}
            Err(QueueEventError::Invalid) => {
                output.fill(0.0);
                return Err(String::new());
            }
            Err(QueueEventError::Saturated) => {
                self.advance_silent_overload(output, context.num_frames);
                return Ok(context.num_frames);
            }
        }
        if context.num_frames == 0 {
            return Ok(0);
        }
        if self.worker.faulted.load(Ordering::Acquire) {
            self.advance_silent_overload(output, context.num_frames);
            return Ok(context.num_frames);
        }
        if self.timeline.terminal_epoch {
            output.fill(0.0);
            self.timeline.absolute_input_frame += context.num_frames as u64;
            self.timeline.absolute_emitted_frame += context.num_frames as u64;
            return Ok(context.num_frames);
        }
        self.emit_output(output, context.num_frames);
        self.enqueue_input(input, context);
        Ok(context.num_frames)
    }

    fn latency_samples(&self) -> usize {
        self.metadata.inner_latency + self.adapter_latency_frames()
    }

    fn realtime_quantum_frames(&self) -> usize {
        1
    }

    fn cost_class(&self) -> PluginCostClass {
        self.metadata.cost_class
    }
}

#[allow(clippy::too_many_arguments)]
fn run_worker(
    inner: &mut dyn Plugin,
    sample_rate: u32,
    quantum_frames: usize,
    input_channels: usize,
    output_channels: usize,
    parameter_ids: Vec<ParameterId>,
    mut jobs: Consumer<InputBlock>,
    mut input_recycle: Producer<InputBlock>,
    mut ready_output: Producer<OutputBlock>,
    mut output_recycle: Consumer<OutputBlock>,
    mut events: Consumer<TimelineEvent>,
    requested_epoch: Arc<AtomicU64>,
    next_needed_source_frame: Arc<AtomicU64>,
    completed_frame: Arc<AtomicU64>,
    completed_epoch: Arc<AtomicU64>,
    faulted: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
) {
    let mut epoch = 0u64;
    let mut expected_frame = 0u64;
    let mut inner_faulted = false;
    let mut future_event: Option<TimelineEvent> = None;
    let mut scratch_output = vec![0.0; quantum_frames * output_channels];
    let mut deferred_input_recycle: Option<InputBlock> = None;
    let mut spare_output: Option<OutputBlock> = None;

    while !stop.load(Ordering::Acquire) {
        let requested = requested_epoch.load(Ordering::Acquire);
        if requested != epoch {
            epoch = requested;
            expected_frame = 0;
            inner_faulted = !reset_worker_plugin(inner);
            // All old-epoch events were accepted before reset published the
            // new epoch. After a successful signal-state reset, apply them in
            // queue order so accepted parameter state survives reset. If an
            // inner call panics or fails, stop touching that instance until a
            // later explicit reset succeeds.
            if !inner_faulted
                && let Some(event) = future_event
                && event.epoch < requested
            {
                if apply_worker_event(inner, &parameter_ids, event) {
                    future_event = None;
                } else {
                    inner_faulted = true;
                }
            }
            while !inner_faulted && future_event.is_none() {
                let Ok(event) = events.pop() else {
                    break;
                };
                if event.epoch >= requested {
                    future_event = Some(event);
                } else if !apply_worker_event(inner, &parameter_ids, event) {
                    future_event = Some(event);
                    inner_faulted = true;
                }
            }
            faulted.store(inner_faulted, Ordering::Release);
        }
        if let Some(block) = deferred_input_recycle.take()
            && let Err(rtrb::PushError::Full(block)) = input_recycle.push(block)
        {
            deferred_input_recycle = Some(block);
            thread::park_timeout(Duration::from_millis(1));
            continue;
        }
        let Ok(mut job) = jobs.pop() else {
            thread::park_timeout(Duration::from_millis(1));
            continue;
        };
        // Reset and event publication can race the first epoch load above.
        // Recheck after acquiring a job and before consuming any events.
        if requested_epoch.load(Ordering::Acquire) != epoch || job.epoch != epoch || inner_faulted {
            job.filled_frames = 0;
            if let Err(rtrb::PushError::Full(job)) = input_recycle.push(job) {
                deferred_input_recycle = Some(job);
            }
            continue;
        }
        if expected_frame < job.start_frame {
            // An explicit source-timeline gap means the callback already
            // emitted bounded silence. Drain its bounded event prefix, reset
            // signal history, and jump directly to the next accepted block;
            // never perform unbounded zero-DSP catch-up proportional to the
            // duration of an overload.
            if !skip_worker_gap(
                inner,
                epoch,
                job.start_frame,
                &parameter_ids,
                &mut events,
                &mut future_event,
            ) {
                inner_faulted = true;
                faulted.store(true, Ordering::Release);
                job.filled_frames = 0;
                if let Err(rtrb::PushError::Full(job)) = input_recycle.push(job) {
                    deferred_input_recycle = Some(job);
                }
                continue;
            }
        }
        let late = job.start_frame + quantum_frames as u64
            <= next_needed_source_frame.load(Ordering::Acquire);
        let mut output_block = if late {
            None
        } else {
            spare_output.take().or_else(|| output_recycle.pop().ok())
        };
        let destination = output_block
            .as_mut()
            .map_or(&mut scratch_output[..], |block| &mut block.samples[..]);
        let processed = process_worker_span(
            inner,
            sample_rate,
            epoch,
            job.start_frame,
            quantum_frames,
            input_channels,
            output_channels,
            &parameter_ids,
            &mut events,
            &mut future_event,
            &job.samples,
            destination,
            job.transport,
        );
        if !processed {
            inner_faulted = true;
            faulted.store(true, Ordering::Release);
        }
        expected_frame = job.start_frame + quantum_frames as u64;
        completed_frame.store(expected_frame, Ordering::Release);
        completed_epoch.store(epoch, Ordering::Release);
        if let Some(mut block) = output_block {
            block.epoch = epoch;
            block.start_frame = job.start_frame;
            if processed
                && block.start_frame + quantum_frames as u64
                    > next_needed_source_frame.load(Ordering::Acquire)
            {
                if let Err(rtrb::PushError::Full(block)) = ready_output.push(block) {
                    spare_output = Some(block);
                }
            } else {
                spare_output = Some(block);
            }
        }
        job.filled_frames = 0;
        if let Err(rtrb::PushError::Full(job)) = input_recycle.push(job) {
            deferred_input_recycle = Some(job);
        }
    }
}

fn reset_worker_plugin(inner: &mut dyn Plugin) -> bool {
    catch_unwind(AssertUnwindSafe(|| inner.reset())).is_ok()
}

fn skip_worker_gap(
    inner: &mut dyn Plugin,
    epoch: u64,
    target_frame: u64,
    parameter_ids: &[ParameterId],
    events: &mut Consumer<TimelineEvent>,
    future_event: &mut Option<TimelineEvent>,
) -> bool {
    loop {
        if future_event.is_none() {
            *future_event = events.pop().ok();
        }
        let Some(event) = *future_event else {
            break;
        };
        if event.epoch == epoch && event.absolute_frame >= target_frame {
            break;
        }
        if event.epoch > epoch {
            break;
        }
        if event.epoch == epoch && !apply_worker_event(inner, parameter_ids, event) {
            return false;
        }
        *future_event = events.pop().ok();
    }
    reset_worker_plugin(inner)
}

#[allow(clippy::too_many_arguments)]
fn process_worker_span(
    inner: &mut dyn Plugin,
    sample_rate: u32,
    epoch: u64,
    start_frame: u64,
    frames: usize,
    input_channels: usize,
    output_channels: usize,
    parameter_ids: &[ParameterId],
    events: &mut Consumer<TimelineEvent>,
    future_event: &mut Option<TimelineEvent>,
    input: &[f32],
    output: &mut [f32],
    transport: TransportInfo,
) -> bool {
    output.fill(0.0);
    let end_frame = start_frame + frames as u64;
    let mut cursor = start_frame;
    while cursor < end_frame {
        if future_event.is_none() {
            *future_event = events.pop().ok();
        }
        while let Some(event) = *future_event {
            if event.epoch > epoch {
                break;
            }
            if event.epoch < epoch {
                *future_event = events.pop().ok();
                continue;
            }
            if event.absolute_frame <= cursor {
                if !apply_worker_event(inner, parameter_ids, event) {
                    return false;
                }
                *future_event = events.pop().ok();
                continue;
            }
            break;
        }
        let next_boundary = future_event
            .filter(|event| event.epoch == epoch && event.absolute_frame < end_frame)
            .map_or(end_frame, |event| event.absolute_frame.max(cursor));
        let span_frames = (next_boundary - cursor) as usize;
        if span_frames == 0 {
            continue;
        }
        let frame_offset = (cursor - start_frame) as usize;
        let input_start = frame_offset * input_channels;
        let output_start = frame_offset * output_channels;
        let mut span_transport = transport;
        span_transport.sample_position = cursor;
        let context = ProcessContext::new(sample_rate, span_frames).with_transport(span_transport);
        let processed = catch_unwind(AssertUnwindSafe(|| {
            inner.process(
                &input[input_start..input_start + span_frames * input_channels],
                &mut output[output_start..output_start + span_frames * output_channels],
                &context,
            )
        }));
        match processed {
            Ok(Ok(written)) if written == span_frames => {}
            _ => return false,
        }
        cursor = next_boundary;
    }
    true
}

fn apply_worker_event(
    inner: &mut dyn Plugin,
    parameter_ids: &[ParameterId],
    event: TimelineEvent,
) -> bool {
    if let Some(id) = parameter_ids.get(event.parameter_index as usize) {
        matches!(
            catch_unwind(AssertUnwindSafe(|| {
                inner.set_parameter(id.clone(), event.value.into_parameter())
            })),
            Ok(Ok(()))
        )
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_no_allocs;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicUsize};
    use std::time::{Duration, Instant};

    #[derive(Default)]
    struct OracleState {
        initialize_calls: AtomicUsize,
        process_calls: AtomicUsize,
        reset_calls: AtomicUsize,
        block_worker: AtomicBool,
        fail_process: AtomicBool,
        panic_process: AtomicBool,
        panic_parameter: AtomicBool,
        panic_reset: AtomicBool,
        applied_gains: Mutex<Vec<f32>>,
        drop_thread: Mutex<Option<thread::ThreadId>>,
    }

    struct OraclePlugin {
        state: Arc<OracleState>,
        initialized: bool,
        gain: f32,
        quantum: usize,
        latency: usize,
    }

    impl OraclePlugin {
        fn new(state: Arc<OracleState>, quantum: usize, latency: usize) -> Self {
            Self {
                state,
                initialized: false,
                gain: 1.0,
                quantum,
                latency,
            }
        }
    }

    impl Drop for OraclePlugin {
        fn drop(&mut self) {
            *self.state.drop_thread.lock().unwrap() = Some(thread::current().id());
        }
    }

    impl Plugin for OraclePlugin {
        fn info(&self) -> PluginInfo {
            PluginInfo::new("async-oracle", "0.1", "test")
        }
        fn input_channels(&self) -> usize {
            1
        }
        fn output_channels(&self) -> usize {
            1
        }
        fn parameters(&self) -> Vec<Parameter> {
            vec![
                Parameter::new_float("gain", "Gain", self.gain, 0.0, 8.0),
                Parameter::new_int("shape", "Shape", 0, 0, 3)
                    .with_update_mode(UpdateMode::Structural),
            ]
        }
        fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
            assert!(
                !self.state.panic_parameter.load(Ordering::Acquire),
                "injected parameter panic"
            );
            match (id.as_str(), value) {
                ("gain", ParameterValue::Float(value)) if (0.0..=8.0).contains(&value) => {
                    self.gain = value;
                    self.state.applied_gains.lock().unwrap().push(value);
                    Ok(())
                }
                ("shape", _) => Err("shape is structural".into()),
                _ => Err("invalid oracle parameter".into()),
            }
        }
        fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
            match id.as_str() {
                "gain" => Some(ParameterValue::Float(self.gain)),
                "shape" => Some(ParameterValue::Int(0)),
                _ => None,
            }
        }
        fn initialize(&mut self, _sample_rate: u32) -> PluginResult<()> {
            let prior = self.state.initialize_calls.fetch_add(1, Ordering::SeqCst);
            if prior != 0 {
                return Err("initialized twice".into());
            }
            self.initialized = true;
            Ok(())
        }
        fn process(
            &mut self,
            input: &[f32],
            output: &mut [f32],
            context: &ProcessContext<'_>,
        ) -> PluginResult<usize> {
            self.state.process_calls.fetch_add(1, Ordering::SeqCst);
            while self.state.block_worker.load(Ordering::Acquire) {
                thread::yield_now();
            }
            if self.state.fail_process.load(Ordering::Acquire) {
                output.fill(1234.0);
                return Err("injected worker failure".into());
            }
            assert!(
                !self.state.panic_process.load(Ordering::Acquire),
                "injected process panic"
            );
            for (destination, source) in output.iter_mut().zip(input) {
                *destination = *source * self.gain;
            }
            Ok(context.num_frames)
        }
        fn reset(&mut self) {
            self.state.reset_calls.fetch_add(1, Ordering::SeqCst);
            assert!(
                !self.state.panic_reset.load(Ordering::Acquire),
                "injected reset panic"
            );
        }
        fn latency_samples(&self) -> usize {
            if self.initialized { self.latency } else { 0 }
        }
        fn realtime_quantum_frames(&self) -> usize {
            if self.initialized { self.quantum } else { 1 }
        }
        fn cost_class(&self) -> PluginCostClass {
            PluginCostClass::Convolution
        }
    }

    fn adapter(
        max_callback: usize,
        quantum: usize,
        latency: usize,
    ) -> (AsyncTimelinePlugin, Arc<OracleState>) {
        let state = Arc::new(OracleState::default());
        let inner = OraclePlugin::new(Arc::clone(&state), quantum, latency);
        (
            AsyncTimelinePlugin::new(Box::new(inner), 48_000, max_callback).unwrap(),
            state,
        )
    }

    fn wait_for(adapter: &AsyncTimelinePlugin, target: u64) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while adapter.completed_frame() < target {
            assert!(
                Instant::now() < deadline,
                "worker did not complete frame {target}"
            );
            thread::yield_now();
        }
    }

    fn wait_until(mut predicate: impl FnMut() -> bool, message: &str) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !predicate() {
            assert!(Instant::now() < deadline, "{message}");
            thread::yield_now();
        }
    }

    fn process(
        adapter: &mut AsyncTimelinePlugin,
        input: &[f32],
        events: &[crate::plugin::ParameterEvent],
    ) -> Vec<f32> {
        let mut output = vec![f32::NAN; input.len()];
        let context = ProcessContext::new(48_000, input.len()).with_parameter_events(events);
        assert_eq!(
            adapter.process(input, &mut output, &context).unwrap(),
            input.len()
        );
        assert!(output.iter().all(|sample| sample.is_finite()));
        output
    }

    #[test]
    fn initializes_once_before_deriving_quantum_and_latency() {
        let (mut adapter, state) = adapter(17, 32, 7);
        assert_eq!(state.initialize_calls.load(Ordering::SeqCst), 1);
        assert_eq!(adapter.quantum_frames(), 32);
        assert_eq!(adapter.adapter_latency_frames(), 64);
        assert_eq!(adapter.latency_samples(), 71);
        adapter.initialize(48_000).unwrap();
        assert_eq!(state.initialize_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn callbacks_1_17_and_max_match_exact_two_quantum_delay() {
        const Q: usize = 32;
        let (mut adapter, _) = adapter(Q, 8, 0);
        let source: Vec<f32> = (0..192).map(|frame| frame as f32 * 0.01 - 0.5).collect();
        let mut driven = source.clone();
        driven.extend(std::iter::repeat_n(0.0, 2 * Q));
        let partitions = [1usize, 17, Q];
        let mut cursor = 0;
        let mut output = Vec::new();
        let mut partition = 0;
        while cursor < driven.len() {
            let frames = partitions[partition % partitions.len()].min(driven.len() - cursor);
            output.extend(process(&mut adapter, &driven[cursor..cursor + frames], &[]));
            cursor += frames;
            wait_for(&adapter, (cursor / Q * Q) as u64);
            partition += 1;
        }
        for (frame, actual) in output.iter().enumerate() {
            let expected = frame
                .checked_sub(2 * Q)
                .and_then(|source_frame| source.get(source_frame))
                .copied()
                .unwrap_or(0.0);
            assert_eq!(*actual, expected, "absolute output frame {frame}");
        }
    }

    #[test]
    fn automation_orders_quantum_minus_one_boundary_plus_one_and_ties() {
        const Q: usize = 8;
        let (mut adapter, state) = adapter(Q, Q, 0);
        let mut rendered = process(&mut adapter, &[1.0; Q - 1], &[]);
        let event = |offset, value| {
            crate::plugin::ParameterEvent::new(
                offset,
                ParameterId::from("gain"),
                ParameterValue::Float(value),
            )
        };
        let events = [event(0, 2.0), event(1, 3.0), event(1, 4.0), event(2, 5.0)];
        rendered.extend(process(&mut adapter, &[1.0; Q], &events));
        wait_for(&adapter, Q as u64);
        rendered.extend(process(&mut adapter, &[1.0], &[]));
        wait_for(&adapter, (2 * Q) as u64);
        rendered.extend(process(&mut adapter, &[0.0; Q], &[]));
        rendered.extend(process(&mut adapter, &[0.0; Q], &[]));
        let expected = [
            1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 4.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0,
        ];
        assert_eq!(&rendered[2 * Q..4 * Q], &expected);
        assert_eq!(&*state.applied_gains.lock().unwrap(), &[2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn event_queue_saturation_advances_timeline_and_never_replays_shifted_audio() {
        const Q: usize = 8;
        let (mut adapter, _) = adapter(Q, Q, 0);
        for _ in 0..EVENT_QUEUE_CAPACITY {
            adapter
                .set_parameter(ParameterId::from("gain"), ParameterValue::Float(1.0))
                .unwrap();
        }

        let event = crate::plugin::ParameterEvent::new(
            0,
            ParameterId::from("gain"),
            ParameterValue::Float(2.0),
        );
        let input = [9.0; Q];
        let mut overloaded = [f32::NAN; Q];
        let events = [event];
        let context = ProcessContext::new(48_000, Q).with_parameter_events(&events);
        assert_eq!(
            adapter.process(&input, &mut overloaded, &context).unwrap(),
            Q
        );
        assert_eq!(overloaded, [0.0; Q]);
        assert_eq!(adapter.timeline.absolute_input_frame, Q as u64);
        assert_eq!(adapter.timeline.absolute_emitted_frame, Q as u64);

        let post_overload = process(&mut adapter, &[2.0; Q], &[]);
        assert_eq!(post_overload, [0.0; Q]);
        wait_for(&adapter, (2 * Q) as u64);
        let gap = process(&mut adapter, &[0.0; Q], &[]);
        let recovered = process(&mut adapter, &[0.0; Q], &[]);
        assert_eq!(gap, [0.0; Q]);
        assert_eq!(recovered, [2.0; Q]);
    }

    #[test]
    fn input_pool_saturation_preserves_absolute_gaps_and_recovers_all_blocks() {
        const Q: usize = 8;
        let (mut adapter, state) = adapter(Q, Q, 0);
        state.block_worker.store(true, Ordering::Release);
        assert_eq!(process(&mut adapter, &[1.0; Q], &[]), [0.0; Q]);
        wait_until(
            || state.process_calls.load(Ordering::Acquire) != 0,
            "worker did not enter the blocked process call",
        );

        for _ in 0..BLOCK_POOL_SIZE {
            let _ = process(&mut adapter, &[1.0; Q], &[]);
        }
        assert!(adapter.timeline.active_input.is_none());
        assert_eq!(
            adapter.timeline.absolute_input_frame,
            ((BLOCK_POOL_SIZE + 1) * Q) as u64
        );

        state.block_worker.store(false, Ordering::Release);
        adapter.worker.thread.unpark();
        wait_for(&adapter, (BLOCK_POOL_SIZE * Q) as u64);
        wait_until(
            || adapter.queues.input_recycle.slots() == BLOCK_POOL_SIZE,
            "input blocks were not all returned after saturation",
        );

        let marker = process(&mut adapter, &[3.0; Q], &[]);
        assert_eq!(marker, [1.0; Q]);
        wait_for(&adapter, ((BLOCK_POOL_SIZE + 2) * Q) as u64);
        let explicit_gap = process(&mut adapter, &[0.0; Q], &[]);
        let recovered = process(&mut adapter, &[0.0; Q], &[]);
        assert_eq!(explicit_gap, [0.0; Q]);
        assert_eq!(recovered, [3.0; Q]);
    }

    #[test]
    fn output_pool_saturation_discards_only_its_absolute_slots_and_recovers_conservation() {
        const Q: usize = 8;
        let (mut adapter, _) = adapter(Q, Q, 0);
        let context = ProcessContext::new(48_000, Q);
        for block_number in 0..BLOCK_POOL_SIZE + 2 {
            adapter.enqueue_input(&[block_number as f32 + 1.0; Q], &context);
            wait_for(&adapter, ((block_number + 1) * Q) as u64);
            wait_until(
                || adapter.queues.input_recycle.slots() != 0,
                "worker did not recycle an input block",
            );
        }
        assert_eq!(adapter.queues.ready_output.slots(), BLOCK_POOL_SIZE);
        assert_eq!(adapter.queues.output_recycle.slots(), BLOCK_POOL_SIZE);

        adapter.timeline.absolute_emitted_frame = (2 * Q) as u64;
        let mut rendered = Vec::new();
        for _ in 0..BLOCK_POOL_SIZE + 2 {
            let mut block = [f32::NAN; Q];
            adapter.emit_output(&mut block, Q);
            rendered.extend(block);
        }
        for block_number in 0..BLOCK_POOL_SIZE {
            assert_eq!(
                &rendered[block_number * Q..(block_number + 1) * Q],
                &[block_number as f32 + 1.0; Q]
            );
        }
        assert_eq!(&rendered[BLOCK_POOL_SIZE * Q..], &[0.0; 2 * Q]);

        adapter.enqueue_input(&[11.0; Q], &context);
        wait_for(&adapter, ((BLOCK_POOL_SIZE + 3) * Q) as u64);
        let mut recovered = [f32::NAN; Q];
        adapter.emit_output(&mut recovered, Q);
        assert_eq!(recovered, [11.0; Q]);
        wait_until(
            || adapter.queues.input_recycle.slots() == BLOCK_POOL_SIZE,
            "input pool was not conserved",
        );
        assert_eq!(adapter.queues.output_recycle.slots(), 0);
        assert!(adapter.timeline.deferred_output_recycle.is_empty());
    }

    #[test]
    fn negotiated_max_callback_is_causal_and_every_output_path_overwrites() {
        const MAX: usize = 17;
        let (mut adapter, _) = adapter(MAX, 8, 5);
        assert_eq!(adapter.quantum_frames(), MAX);
        assert_eq!(adapter.adapter_latency_frames(), 2 * MAX);
        assert_eq!(adapter.latency_samples(), 2 * MAX + 5);

        let first = process(&mut adapter, &[1.0; MAX], &[]);
        wait_for(&adapter, MAX as u64);
        let second = process(&mut adapter, &[0.0; MAX], &[]);
        wait_for(&adapter, (2 * MAX) as u64);
        let third = process(&mut adapter, &[0.0; MAX], &[]);
        assert_eq!(first, [0.0; MAX]);
        assert_eq!(second, [0.0; MAX]);
        assert_eq!(third, [1.0; MAX]);

        let mut oversized = [f32::NAN; MAX + 1];
        let context = ProcessContext::new(48_000, MAX + 1);
        assert!(
            adapter
                .process(&[1.0; MAX + 1], &mut oversized, &context)
                .is_err()
        );
        assert_eq!(oversized, [0.0; MAX + 1]);
    }

    #[test]
    fn reset_rejects_stale_output_but_preserves_an_accepted_parameter_update() {
        const Q: usize = 8;
        let (mut adapter, state) = adapter(Q, Q, 0);
        assert_eq!(process(&mut adapter, &[9.0; Q], &[]), [0.0; Q]);
        wait_for(&adapter, Q as u64);
        wait_until(
            || adapter.queues.ready_output.slots() != 0,
            "pre-reset output was not published",
        );
        adapter
            .set_parameter(ParameterId::from("gain"), ParameterValue::Float(2.0))
            .unwrap();
        adapter.reset();
        assert_eq!(
            adapter.get_parameter(&ParameterId::from("gain")),
            Some(ParameterValue::Float(2.0))
        );
        let first = process(&mut adapter, &[1.0; Q], &[]);
        wait_until(
            || state.applied_gains.lock().unwrap().contains(&2.0),
            "accepted pre-reset parameter was not applied",
        );
        wait_until(
            || adapter.queues.ready_output.slots() != 0,
            "post-reset output was not published",
        );
        let second = process(&mut adapter, &[1.0; Q], &[]);
        let third = process(&mut adapter, &[0.0; Q], &[]);
        assert!(first.iter().chain(&second).all(|sample| *sample == 0.0));
        assert!(third.iter().all(|sample| *sample == 2.0));
        assert!(third.iter().all(|sample| *sample != 9.0));
    }

    #[test]
    fn warmed_valid_recycle_and_stale_rejection_paths_never_allocate_or_destroy() {
        const Q: usize = 8;
        let (mut warmed, _) = adapter(Q, Q, 0);
        let _ = process(&mut warmed, &[1.0; Q], &[]);
        wait_for(&warmed, Q as u64);
        let _ = process(&mut warmed, &[0.0; Q], &[]);
        wait_for(&warmed, (2 * Q) as u64);
        let input = [0.0; Q];
        let mut output = [f32::NAN; Q];
        let context = ProcessContext::new(48_000, Q);
        assert_no_allocs("async timeline warmed recycle callback", || {
            warmed.process(&input, &mut output, &context).unwrap();
        });

        let (mut stale, _) = adapter(Q, Q, 0);
        let _ = process(&mut stale, &[9.0; Q], &[]);
        wait_for(&stale, Q as u64);
        wait_until(
            || stale.queues.ready_output.slots() != 0,
            "stale candidate was not published",
        );
        stale.reset();
        stale.timeline.absolute_emitted_frame = (2 * Q) as u64;
        let mut stale_output = [f32::NAN; Q];
        assert_no_allocs("async timeline stale recycle callback", || {
            stale.process(&input, &mut stale_output, &context).unwrap();
        });
        assert_eq!(stale_output, [0.0; Q]);
    }

    #[test]
    fn control_thread_drop_joins_worker_and_destroys_inner_on_control_thread() {
        let control_thread = thread::current().id();
        let (adapter, state) = adapter(8, 8, 0);
        drop(adapter);
        let dropped_on = *state.drop_thread.lock().unwrap();
        assert!(dropped_on.is_some());
        assert_eq!(dropped_on, Some(control_thread));
    }

    #[test]
    fn worker_panics_are_contained_and_inner_destruction_returns_to_control_thread() {
        const Q: usize = 8;
        let control_thread = thread::current().id();

        let assert_control_drop = |adapter: AsyncTimelinePlugin, state: &Arc<OracleState>| {
            drop(adapter);
            assert_eq!(*state.drop_thread.lock().unwrap(), Some(control_thread));
        };

        let (mut process_adapter, process_state) = adapter(Q, Q, 0);
        process_state.panic_process.store(true, Ordering::Release);
        assert_eq!(process(&mut process_adapter, &[1.0; Q], &[]), [0.0; Q]);
        wait_until(
            || process_adapter.worker.faulted.load(Ordering::Acquire),
            "process panic was not latched",
        );
        assert_control_drop(process_adapter, &process_state);

        let (mut parameter_adapter, parameter_state) = adapter(Q, Q, 0);
        parameter_state
            .panic_parameter
            .store(true, Ordering::Release);
        parameter_adapter
            .set_parameter(ParameterId::from("gain"), ParameterValue::Float(2.0))
            .unwrap();
        assert_eq!(process(&mut parameter_adapter, &[1.0; Q], &[]), [0.0; Q]);
        wait_until(
            || parameter_adapter.worker.faulted.load(Ordering::Acquire),
            "parameter panic was not latched",
        );
        assert_control_drop(parameter_adapter, &parameter_state);

        let (mut reset_adapter, reset_state) = adapter(Q, Q, 0);
        reset_state.panic_reset.store(true, Ordering::Release);
        reset_adapter.reset();
        wait_until(
            || reset_adapter.worker.faulted.load(Ordering::Acquire),
            "reset panic was not latched",
        );
        assert_control_drop(reset_adapter, &reset_state);
    }

    #[test]
    fn fault_latch_recycles_queued_jobs_without_reentering_inner() {
        const Q: usize = 8;
        let (mut adapter, state) = adapter(Q, Q, 0);
        state.block_worker.store(true, Ordering::Release);
        state.panic_process.store(true, Ordering::Release);
        assert_eq!(process(&mut adapter, &[1.0; Q], &[]), [0.0; Q]);
        wait_until(
            || state.process_calls.load(Ordering::Acquire) == 1,
            "worker did not enter the first process call",
        );
        for _ in 0..BLOCK_POOL_SIZE {
            assert_eq!(process(&mut adapter, &[2.0; Q], &[]), [0.0; Q]);
        }

        state.block_worker.store(false, Ordering::Release);
        adapter.worker.thread.unpark();
        wait_until(
            || adapter.worker.faulted.load(Ordering::Acquire),
            "process panic was not latched",
        );
        wait_until(
            || adapter.queues.input_recycle.slots() == BLOCK_POOL_SIZE,
            "queued jobs were not recycled after the fault",
        );
        assert_eq!(
            state.process_calls.load(Ordering::Acquire),
            1,
            "faulted inner must not be called again before reset"
        );
    }

    #[test]
    fn future_epoch_event_is_preserved_until_worker_observes_reset() {
        const Q: usize = 8;
        let state = Arc::new(OracleState::default());
        let mut inner = OraclePlugin::new(Arc::clone(&state), Q, 0);
        let (mut event_producer, mut event_consumer) = RingBuffer::new(4);
        assert!(
            event_producer
                .push(TimelineEvent {
                    epoch: 1,
                    absolute_frame: 0,
                    parameter_index: 0,
                    value: PrimitiveValue::Float(2.0),
                })
                .is_ok()
        );
        let parameter_ids = [ParameterId::from("gain")];
        let input = [1.0; Q];
        let mut old_epoch_output = [f32::NAN; Q];
        let mut future_event = None;

        assert!(process_worker_span(
            &mut inner,
            48_000,
            0,
            0,
            Q,
            1,
            1,
            &parameter_ids,
            &mut event_consumer,
            &mut future_event,
            &input,
            &mut old_epoch_output,
            TransportInfo::default(),
        ));
        assert_eq!(old_epoch_output, [1.0; Q]);
        assert_eq!(future_event.map(|event| event.epoch), Some(1));
        assert!(state.applied_gains.lock().unwrap().is_empty());

        assert!(reset_worker_plugin(&mut inner));
        let mut new_epoch_output = [f32::NAN; Q];
        assert!(process_worker_span(
            &mut inner,
            48_000,
            1,
            0,
            Q,
            1,
            1,
            &parameter_ids,
            &mut event_consumer,
            &mut future_event,
            &input,
            &mut new_epoch_output,
            TransportInfo::default(),
        ));
        assert_eq!(new_epoch_output, [2.0; Q]);
        assert_eq!(&*state.applied_gains.lock().unwrap(), &[2.0]);
    }

    #[test]
    fn long_overload_gap_resets_and_jumps_without_zero_dsp_catch_up() {
        const Q: usize = 8;
        const GAP_FRAMES: usize = 1_000_000 * Q;
        let (mut adapter, state) = adapter(Q, Q, 0);
        assert_eq!(process(&mut adapter, &[1.0; Q], &[]), [0.0; Q]);
        wait_for(&adapter, Q as u64);
        let calls_before_gap = state.process_calls.load(Ordering::Acquire);

        adapter
            .set_parameter(ParameterId::from("gain"), ParameterValue::Float(2.0))
            .unwrap();
        let mut silence = [f32::NAN; Q];
        adapter.advance_silent_overload(&mut silence, GAP_FRAMES);
        assert_eq!(silence, [0.0; Q]);

        assert_eq!(process(&mut adapter, &[3.0; Q], &[]), [0.0; Q]);
        let target = (Q + GAP_FRAMES + Q) as u64;
        wait_for(&adapter, target);
        assert_eq!(
            state.process_calls.load(Ordering::Acquire),
            calls_before_gap + 1,
            "gap duration must not create proportional DSP work"
        );
        assert_eq!(state.reset_calls.load(Ordering::Acquire), 1);
        assert_eq!(&*state.applied_gains.lock().unwrap(), &[2.0]);
    }

    #[test]
    fn inner_process_failure_latches_full_silence_until_reset() {
        const Q: usize = 8;
        let (mut adapter, state) = adapter(Q, Q, 0);
        state.fail_process.store(true, Ordering::Release);
        assert_eq!(process(&mut adapter, &[1.0; Q], &[]), [0.0; Q]);
        wait_for(&adapter, Q as u64);
        wait_until(
            || adapter.worker.faulted.load(Ordering::Acquire),
            "worker failure was not latched",
        );

        assert_eq!(process(&mut adapter, &[2.0; Q], &[]), [0.0; Q]);
        assert_eq!(process(&mut adapter, &[3.0; Q], &[]), [0.0; Q]);
        assert_eq!(adapter.timeline.absolute_input_frame, (3 * Q) as u64);
        assert_eq!(adapter.timeline.absolute_emitted_frame, (3 * Q) as u64);

        state.fail_process.store(false, Ordering::Release);
        adapter.reset();
        wait_until(
            || !adapter.worker.faulted.load(Ordering::Acquire),
            "reset did not clear the worker fault",
        );
        assert_eq!(process(&mut adapter, &[4.0; Q], &[]), [0.0; Q]);
        wait_until(
            || {
                adapter.completed_epoch() == adapter.timeline.epoch
                    && adapter.completed_frame() >= Q as u64
            },
            "post-reset block did not complete in the new epoch",
        );
        assert_eq!(process(&mut adapter, &[0.0; Q], &[]), [0.0; Q]);
        assert_eq!(process(&mut adapter, &[0.0; Q], &[]), [4.0; Q]);
    }
}
