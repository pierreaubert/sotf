//! Host-side controller for out-of-process external plugin processing.
//!
//! This owns the host-created secure shared-memory segment, publishes audio
//! blocks into it, waits up to a caller-provided deadline for the worker, and
//! falls back to bypass-style audio when the worker fails or misses the block.

use std::path::Path;
use std::time::{Duration, Instant};

use crate::external_plugin_ipc::PluginIpcParameterEvent;
use crate::external_plugin_ipc::{
    PluginIpcControlRequest, PluginIpcControlResponse, PluginIpcLayout, PluginIpcState,
    PluginSandboxRuntimeStatus, SecurePluginSharedMemory,
};
use crate::parameters::{ParameterId, ParameterValue};
use crate::plugin::ProcessContext;

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
    next_control_sequence: u64,
    timeout_count: u64,
    worker_failure_count: u64,
    wrong_sequence_count: u64,
    fallback_delay: Vec<f32>,
    fallback_delay_frames: usize,
    fallback_delay_pos: usize,
    fallback_block: Vec<f32>,
    last_output: Vec<f32>,
    previous_status: Option<ExternalPluginHostBlockStatus>,
    parameter_ids: Vec<ParameterId>,
    parameter_event_scratch: Vec<PluginIpcParameterEvent>,
}

impl ExternalPluginHostProxy {
    pub fn new(layout: PluginIpcLayout, deadline: Duration) -> Result<Self, String> {
        let shared = SecurePluginSharedMemory::create(layout)
            .map_err(|err| format!("failed to create external-plugin shared memory: {err}"))?;
        Ok(Self::from_shared(shared, deadline))
    }

    pub fn from_shared(shared: SecurePluginSharedMemory, deadline: Duration) -> Self {
        let layout = shared.layout();
        Self {
            shared,
            deadline,
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
            previous_status: None,
            parameter_ids: Vec::new(),
            parameter_event_scratch: Vec::with_capacity(1024),
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
        self.prepare_fallback(input, frames);
        let frames = self.write_fallback(output, frames);
        self.finish_block(output, frames, ExternalPluginHostBlockStatus::TimedOut);
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
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1).max(1);
        self.shared
            .publish_host_block_with_events(
                sequence,
                frames,
                input,
                context,
                &self.parameter_event_scratch,
            )
            .map_err(|err| format!("failed to publish external-plugin block: {err}"))?;
        self.prepare_fallback(input, frames);

        let start = Instant::now();
        let block_period =
            Duration::from_secs_f64(frames as f64 / self.shared.layout().sample_rate as f64);
        let effective_deadline = self.deadline.min(block_period.mul_f64(0.75));
        loop {
            match self.shared.worker_state() {
                PluginIpcState::WorkerReady => {
                    if self.shared.worker_sequence() == sequence {
                        let processed = self.shared.copy_worker_output(output).map_err(|err| {
                            format!("failed to copy external-plugin output: {err}")
                        })?;
                        self.shared.clear_block();
                        self.finish_block(
                            output,
                            processed,
                            ExternalPluginHostBlockStatus::Processed,
                        );
                        return Ok((processed, ExternalPluginHostBlockStatus::Processed));
                    }
                    self.wrong_sequence_count = self.wrong_sequence_count.saturating_add(1);
                    let frames = self.write_fallback(output, frames);
                    self.shared.clear_block();
                    self.finish_block(output, frames, ExternalPluginHostBlockStatus::WrongSequence);
                    return Ok((frames, ExternalPluginHostBlockStatus::WrongSequence));
                }
                PluginIpcState::WorkerFailed if self.shared.worker_sequence() == sequence => {
                    self.worker_failure_count = self.worker_failure_count.saturating_add(1);
                    let frames = self.write_fallback(output, frames);
                    self.shared.clear_block();
                    self.finish_block(output, frames, ExternalPluginHostBlockStatus::WorkerFailed);
                    return Ok((frames, ExternalPluginHostBlockStatus::WorkerFailed));
                }
                _ => {}
            }

            if start.elapsed() >= effective_deadline {
                self.timeout_count = self.timeout_count.saturating_add(1);
                let frames = self.write_fallback(output, frames);
                self.shared.clear_block();
                self.finish_block(output, frames, ExternalPluginHostBlockStatus::TimedOut);
                return Ok((frames, ExternalPluginHostBlockStatus::TimedOut));
            }

            drive_worker();
            std::hint::spin_loop();
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

    fn write_fallback(&self, output: &mut [f32], frames: usize) -> usize {
        let output_len = frames
            .saturating_mul(self.shared.layout().output_channels as usize)
            .min(output.len())
            .min(self.fallback_block.len());
        output[..output_len].copy_from_slice(&self.fallback_block[..output_len]);
        frames
    }

    fn finish_block(
        &mut self,
        output: &mut [f32],
        frames: usize,
        status: ExternalPluginHostBlockStatus,
    ) {
        let channels = self.shared.layout().output_channels as usize;
        if channels == 0 {
            return;
        }
        let processed = status == ExternalPluginHostBlockStatus::Processed;
        if self.previous_status.is_some_and(|previous| {
            (previous == ExternalPluginHostBlockStatus::Processed) != processed
        }) {
            let transition_frames = frames.min(64);
            for frame in 0..transition_frames {
                let fade = (frame + 1) as f32 / transition_frames as f32;
                for ch in 0..channels {
                    let index = frame * channels + ch;
                    if index < output.len() {
                        output[index] = self.last_output[ch] * (1.0 - fade) + output[index] * fade;
                    }
                }
            }
        }
        if frames > 0 {
            let base = (frames - 1) * channels;
            for ch in 0..channels {
                if base + ch < output.len() {
                    self.last_output[ch] = output[base + ch];
                }
            }
        }
        self.previous_status = Some(status);
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
    fn warmed_success_and_timeout_paths_allocate_nothing() {
        let layout = PluginIpcLayout::new(48_000, 128, 2, 2).unwrap();
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
        let input = [0.25_f32; 256];
        let mut output = [0.0_f32; 256];

        proxy
            .process_block_with(&input, &mut output, 128, || {
                worker.process_one().unwrap();
            })
            .unwrap();
        assert_no_allocs("external IPC warmed success", || {
            for _ in 0..32 {
                let (_, status) = proxy
                    .process_block_with(&input, &mut output, 128, || {
                        worker.process_one().unwrap();
                    })
                    .unwrap();
                assert_eq!(status, ExternalPluginHostBlockStatus::Processed);
            }
        });

        let timeout_shared = SecurePluginSharedMemory::create(layout).unwrap();
        let mut timeout_proxy =
            ExternalPluginHostProxy::from_shared(timeout_shared, Duration::ZERO);
        timeout_proxy
            .process_block(&input, &mut output, 128)
            .unwrap();
        assert_no_allocs("external IPC warmed timeout", || {
            for _ in 0..32 {
                let (_, status) = timeout_proxy
                    .process_block(&input, &mut output, 128)
                    .unwrap();
                assert_eq!(status, ExternalPluginHostBlockStatus::TimedOut);
            }
        });
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

    #[test]
    fn test_timeout_fallback_preserves_reported_latency() {
        let layout = PluginIpcLayout::new(48_000, 128, 1, 1).unwrap();
        let mut proxy = ExternalPluginHostProxy::new(layout, Duration::ZERO).unwrap();
        proxy.configure_fallback_latency(3).unwrap();

        let input = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mut output = [0.0; 8];
        let (_, status) = proxy.process_block(&input, &mut output, 8).unwrap();

        assert_eq!(status, ExternalPluginHostBlockStatus::TimedOut);
        assert_eq!(output, [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_processed_to_fallback_transition_has_bounded_discontinuity() {
        let layout = PluginIpcLayout::new(48_000, 128, 1, 1).unwrap();
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
        proxy = ExternalPluginHostProxy::from_shared(
            SecurePluginSharedMemory::open_existing(proxy.shared_path()).unwrap(),
            Duration::ZERO,
        );
        // Seed a processed endpoint through the public path; replacing the
        // worker is unnecessary because this proxy only exercises fallback.
        proxy.previous_status = Some(ExternalPluginHostBlockStatus::Processed);
        proxy.last_output[0] = 1.0;
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
    fn test_small_block_deadline_is_capped_below_period() {
        let _guard = DEADLINE_TEST_LOCK.lock().unwrap();
        let layout = PluginIpcLayout::new(48_000, 16, 1, 1).unwrap();
        let mut proxy = ExternalPluginHostProxy::new(layout, Duration::from_millis(2)).unwrap();
        let mut output = [0.0; 16];
        let started = Instant::now();
        let (_, status) = proxy.process_block(&[0.0; 16], &mut output, 16).unwrap();
        let elapsed = started.elapsed();

        assert_eq!(status, ExternalPluginHostBlockStatus::TimedOut);
        assert!(elapsed < Duration::from_millis(1), "elapsed {elapsed:?}");
    }

    #[test]
    fn deadline_percentiles_remain_bounded_under_cpu_contention() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        let _guard = DEADLINE_TEST_LOCK.lock().unwrap();
        let running = Arc::new(AtomicBool::new(true));
        let contender_running = Arc::clone(&running);
        let contender = std::thread::spawn(move || {
            let mut value = 1_u64;
            while contender_running.load(Ordering::Relaxed) {
                value =
                    std::hint::black_box(value.wrapping_mul(6364136223846793005).wrapping_add(1));
            }
            value
        });

        let layout = PluginIpcLayout::new(48_000, 16, 1, 1).unwrap();
        let mut proxy = ExternalPluginHostProxy::new(layout, Duration::from_millis(2)).unwrap();
        let input = [0.0_f32; 16];
        let mut output = [0.0_f32; 16];
        let mut samples = Vec::with_capacity(256);
        for _ in 0..256 {
            let start = Instant::now();
            let (_, status) = proxy.process_block(&input, &mut output, 16).unwrap();
            assert_eq!(status, ExternalPluginHostBlockStatus::TimedOut);
            samples.push(start.elapsed());
        }
        running.store(false, Ordering::Relaxed);
        contender.join().unwrap();
        samples.sort_unstable();
        let p95 = samples[samples.len() * 95 / 100];
        let p99 = samples[samples.len() * 99 / 100];
        assert!(p95 < Duration::from_millis(1), "p95={p95:?}, p99={p99:?}");
        assert!(p99 < Duration::from_millis(4), "p95={p95:?}, p99={p99:?}");
    }
}
