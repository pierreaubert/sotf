use super::PluginIpcHeader;
use super::PluginIpcMidiEvent;
#[cfg(unix)]
use super::clamp::clamp_file_permissions;
#[cfg(windows)]
use super::clamp::clamp_file_permissions;
use super::consts::MAX_PLUGIN_IPC_MIDI_EVENTS;
use super::consts::MAX_PLUGIN_IPC_PARAMETER_EVENTS;
use super::consts::PLUGIN_IPC_CONTROL_BYTES;
use super::ensure::create_session_dir;
#[cfg(unix)]
use super::ensure::ensure_secure_parent_dir;
#[cfg(windows)]
use super::ensure::ensure_secure_parent_dir;
use super::invalid::invalid_data;
use super::invalid::invalid_input;
use super::misc::panic_payload_description;
use super::open::open_existing_shared_memory_file;
#[cfg(unix)]
use super::open::open_new_shared_memory_file;
#[cfg(windows)]
use super::open::open_new_shared_memory_file;
use super::plugin_ipc_header::audio_base_offset;
use super::plugin_ipc_header::control_base_offset;
use super::plugin_ipc_header::header_from_mmap;
use super::plugin_ipc_header::parameter_event_base_offset;
use super::plugin_ipc_layout::PluginIpcLayout;
use super::plugin_ipc_layout::total_size;
use super::plugin_ipc_state::PluginIpcState;
use super::plugin_sandbox_backend_code::PluginSandboxBackendCode;
use super::plugin_sandbox_status_code::PluginSandboxStatusCode;
use super::types::PluginIpcRequest;
use super::types::PluginSandboxRuntimeStatus;
use super::{PluginIpcControlRequest, PluginIpcControlResponse, PluginIpcParameterEvent};
use crate::parameters::{Parameter, ParameterValue};
use crate::plugin::{
    LoopRange, MidiEvent, MidiMessage, ParameterEvent, Plugin, ProcessContext, TransportInfo,
};
use memmap2::{MmapMut, MmapOptions};
use std::fs::File;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

pub struct SecurePluginSharedMemory {
    pub(super) path: PathBuf,
    pub(super) session_dir: Option<PathBuf>,
    pub(super) file: Option<File>,
    pub(super) mmap: Option<MmapMut>,
    pub(super) layout: PluginIpcLayout,
    pub(super) input_offset: usize,
    pub(super) output_offset: usize,
    pub(super) remove_on_drop: bool,
}

impl SecurePluginSharedMemory {
    pub fn publish_control_request(
        &mut self,
        sequence: u64,
        request: &PluginIpcControlRequest,
    ) -> io::Result<()> {
        let bytes = serde_json::to_vec(request)
            .map_err(|error| invalid_input(format!("invalid control request: {error}")))?;
        if bytes.len() > PLUGIN_IPC_CONTROL_BYTES {
            return Err(invalid_input(
                "external-plugin control request is too large",
            ));
        }
        let offset = control_base_offset();
        self.mmap.as_mut().expect("mapping is present")[offset..offset + bytes.len()]
            .copy_from_slice(&bytes);
        let header = self.header();
        header
            .control_request_len
            .store(bytes.len() as u32, Ordering::Release);
        header.control_response_len.store(0, Ordering::Release);
        header.control_status.store(0, Ordering::Release);
        header.control_sequence.store(sequence, Ordering::Release);
        header.control_state.store(1, Ordering::Release);
        Ok(())
    }

    pub fn take_control_request(&self) -> io::Result<Option<(u64, PluginIpcControlRequest)>> {
        let header = self.header();
        if header.control_state.load(Ordering::Acquire) != 1 {
            return Ok(None);
        }
        let sequence = header.control_sequence.load(Ordering::Acquire);
        if header.control_worker_sequence.load(Ordering::Acquire) == sequence {
            return Ok(None);
        }
        let len = header.control_request_len.load(Ordering::Acquire) as usize;
        if len > PLUGIN_IPC_CONTROL_BYTES {
            return Err(invalid_data(
                "invalid external-plugin control request length",
            ));
        }
        let offset = control_base_offset();
        let request = serde_json::from_slice(
            &self.mmap.as_ref().expect("mapping is present")[offset..offset + len],
        )
        .map_err(|error| invalid_data(format!("invalid control request JSON: {error}")))?;
        Ok(Some((sequence, request)))
    }

    pub fn publish_control_response(
        &mut self,
        sequence: u64,
        response: &PluginIpcControlResponse,
    ) -> io::Result<()> {
        let bytes = serde_json::to_vec(response)
            .map_err(|error| invalid_input(format!("invalid control response: {error}")))?;
        if bytes.len() > PLUGIN_IPC_CONTROL_BYTES {
            return Err(invalid_input(
                "external-plugin control response is too large",
            ));
        }
        let offset = control_base_offset();
        self.mmap.as_mut().expect("mapping is present")[offset..offset + bytes.len()]
            .copy_from_slice(&bytes);
        let header = self.header();
        header
            .control_response_len
            .store(bytes.len() as u32, Ordering::Release);
        header
            .control_worker_sequence
            .store(sequence, Ordering::Release);
        header.control_state.store(2, Ordering::Release);
        Ok(())
    }

    pub fn take_control_response(
        &self,
        sequence: u64,
    ) -> io::Result<Option<PluginIpcControlResponse>> {
        let header = self.header();
        if header.control_state.load(Ordering::Acquire) != 2
            || header.control_worker_sequence.load(Ordering::Acquire) != sequence
        {
            return Ok(None);
        }
        let len = header.control_response_len.load(Ordering::Acquire) as usize;
        if len > PLUGIN_IPC_CONTROL_BYTES {
            return Err(invalid_data(
                "invalid external-plugin control response length",
            ));
        }
        let offset = control_base_offset();
        let response = serde_json::from_slice(
            &self.mmap.as_ref().expect("mapping is present")[offset..offset + len],
        )
        .map_err(|error| invalid_data(format!("invalid control response JSON: {error}")))?;
        header.control_state.store(0, Ordering::Release);
        Ok(Some(response))
    }
    pub fn create(layout: PluginIpcLayout) -> io::Result<Self> {
        let session_dir = create_session_dir()?;
        let path = session_dir.join("audio-plugin-ipc.shm");
        Self::create_at_inner(path, Some(session_dir), layout)
    }

    pub fn create_at<P: AsRef<Path>>(path: P, layout: PluginIpcLayout) -> io::Result<Self> {
        Self::create_at_inner(path.as_ref().to_path_buf(), None, layout)
    }

    pub(super) fn create_at_inner(
        path: PathBuf,
        session_dir: Option<PathBuf>,
        layout: PluginIpcLayout,
    ) -> io::Result<Self> {
        let size = total_size(layout)?;
        if let Some(parent) = path.parent() {
            ensure_secure_parent_dir(parent)?;
        }

        let file = open_new_shared_memory_file(&path)?;
        file.set_len(size as u64)?;
        clamp_file_permissions(&file)?;

        // SAFETY: The file has just been created and sized to `size`; the
        // mapping length is validated before any typed access.
        let mmap = unsafe { MmapOptions::new().len(size).map_mut(&file)? };
        let region = Self::from_parts(path, session_dir, file, mmap, layout, true)?;
        region.header().initialize(layout);
        Ok(region)
    }

    pub fn open_existing<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = open_existing_shared_memory_file(&path)?;
        let size = file.metadata()?.len() as usize;
        if size < audio_base_offset() {
            return Err(invalid_data("external-plugin IPC mapping is too small"));
        }

        // SAFETY: The descriptor is validated before mapping, and the header is
        // checked before audio slices are exposed.
        let mmap = unsafe { MmapOptions::new().len(size).map_mut(&file)? };
        let header = header_from_mmap(&mmap)?;
        let layout = header.read_layout()?;
        if size < total_size(layout)? {
            return Err(invalid_data(
                "external-plugin IPC mapping has truncated body",
            ));
        }

        Self::from_parts(path, None, file, mmap, layout, false)
    }

    pub(super) fn from_parts(
        path: PathBuf,
        session_dir: Option<PathBuf>,
        file: File,
        mmap: MmapMut,
        layout: PluginIpcLayout,
        remove_on_drop: bool,
    ) -> io::Result<Self> {
        let input_offset = audio_base_offset();
        let output_offset = input_offset + layout.input_samples() * std::mem::size_of::<f32>();
        if mmap.len() < total_size(layout)? {
            return Err(invalid_data("external-plugin IPC mapping is undersized"));
        }

        Ok(Self {
            path,
            session_dir,
            file: Some(file),
            mmap: Some(mmap),
            layout,
            input_offset,
            output_offset,
            remove_on_drop,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn layout(&self) -> PluginIpcLayout {
        self.layout
    }

    pub fn host_sequence(&self) -> u64 {
        self.header().host_sequence.load(Ordering::Acquire)
    }

    pub fn publish_host_sequence(&self, sequence: u64) {
        self.header()
            .host_sequence
            .store(sequence, Ordering::Release);
    }

    pub fn worker_sequence(&self) -> u64 {
        self.header().worker_sequence.load(Ordering::Acquire)
    }

    pub fn publish_worker_sequence(&self, sequence: u64) {
        self.header()
            .worker_sequence
            .store(sequence, Ordering::Release);
    }

    pub fn host_state(&self) -> PluginIpcState {
        PluginIpcState::from_raw(self.header().host_state.load(Ordering::Acquire))
    }

    pub fn worker_state(&self) -> PluginIpcState {
        PluginIpcState::from_raw(self.header().worker_state.load(Ordering::Acquire))
    }

    pub fn block_frames(&self) -> usize {
        self.header().block_frames.load(Ordering::Acquire) as usize
    }

    pub fn processed_frames(&self) -> usize {
        self.header().processed_frames.load(Ordering::Acquire) as usize
    }

    /// Publish one host-owned audio block into shared memory.
    ///
    /// The worker-side API copies this block into private buffers before
    /// invoking unknown plugin code.
    pub fn publish_host_block(
        &mut self,
        sequence: u64,
        frames: usize,
        input: &[f32],
    ) -> io::Result<()> {
        self.publish_host_block_with_context(
            sequence,
            frames,
            input,
            &ProcessContext::new(self.layout.sample_rate, frames),
        )
    }

    pub fn publish_host_block_with_context(
        &mut self,
        sequence: u64,
        frames: usize,
        input: &[f32],
        context: &ProcessContext,
    ) -> io::Result<()> {
        self.publish_host_block_with_events(sequence, frames, input, context, &[])
    }

    pub(crate) fn publish_host_block_with_events(
        &mut self,
        sequence: u64,
        frames: usize,
        input: &[f32],
        context: &ProcessContext,
        parameter_events: &[PluginIpcParameterEvent],
    ) -> io::Result<()> {
        self.validate_frame_count(frames)?;
        if context.sample_rate != self.layout.sample_rate || context.num_frames != frames {
            return Err(invalid_input(
                "external-plugin process context does not match IPC block layout",
            ));
        }
        if context.midi_events.len() > MAX_PLUGIN_IPC_MIDI_EVENTS {
            return Err(invalid_input(format!(
                "external-plugin block has {} MIDI events, maximum is {MAX_PLUGIN_IPC_MIDI_EVENTS}",
                context.midi_events.len()
            )));
        }
        if parameter_events.len() > MAX_PLUGIN_IPC_PARAMETER_EVENTS {
            return Err(invalid_input("too many external-plugin parameter events"));
        }
        let expected_input = frames
            .checked_mul(self.layout.input_channels as usize)
            .ok_or_else(|| invalid_input("input frame count overflow"))?;
        if input.len() < expected_input {
            return Err(invalid_input(format!(
                "input has {} samples, expected at least {expected_input}",
                input.len()
            )));
        }

        let output_samples = frames
            .checked_mul(self.layout.output_channels as usize)
            .ok_or_else(|| invalid_input("output frame count overflow"))?;

        let (shared_input, shared_output) = self.audio_slices_mut();
        shared_input[..expected_input].copy_from_slice(&input[..expected_input]);
        shared_output[..output_samples].fill(0.0);

        let midi_ptr = unsafe {
            self.mmap
                .as_mut()
                .expect("mapping is present")
                .as_mut_ptr()
                .add(std::mem::size_of::<PluginIpcHeader>())
                .cast::<PluginIpcMidiEvent>()
        };
        for (index, event) in context.midi_events.iter().enumerate() {
            if event.sample_offset >= frames {
                return Err(invalid_input(format!(
                    "MIDI event offset {} is outside {frames}-frame block",
                    event.sample_offset
                )));
            }
            unsafe {
                *midi_ptr.add(index) = PluginIpcMidiEvent {
                    sample_offset: event.sample_offset as u32,
                    data: event.message.data,
                    len: event.message.len,
                };
            }
        }
        let parameter_ptr = unsafe {
            self.mmap
                .as_mut()
                .expect("mapping is present")
                .as_mut_ptr()
                .add(parameter_event_base_offset())
                .cast::<PluginIpcParameterEvent>()
        };
        for (index, event) in parameter_events.iter().enumerate() {
            if event.sample_offset as usize >= frames {
                return Err(invalid_input("parameter event offset is outside block"));
            }
            unsafe { *parameter_ptr.add(index) = *event };
        }

        let header = self.header();
        let transport = context.transport;
        let mut transport_flags = u32::from(transport.playing);
        transport_flags |= u32::from(transport.recording) << 1;
        transport_flags |= u32::from(transport.looping) << 2;
        header
            .midi_event_count
            .store(context.midi_events.len() as u32, Ordering::Release);
        header
            .parameter_event_count
            .store(parameter_events.len() as u32, Ordering::Release);
        header
            .transport_flags
            .store(transport_flags, Ordering::Release);
        header
            .transport_sample_position
            .store(transport.sample_position, Ordering::Release);
        header
            .transport_bpm_bits
            .store(transport.bpm.to_bits(), Ordering::Release);
        header
            .transport_ppq_bits
            .store(transport.ppq_position.to_bits(), Ordering::Release);
        header.transport_time_signature.store(
            (u32::from(transport.time_signature.numerator) << 16)
                | u32::from(transport.time_signature.denominator),
            Ordering::Release,
        );
        let (loop_start, loop_end) = transport.loop_range.map_or((u64::MAX, u64::MAX), |range| {
            (range.start_sample, range.end_sample)
        });
        header
            .transport_loop_start
            .store(loop_start, Ordering::Release);
        header.transport_loop_end.store(loop_end, Ordering::Release);
        header.processed_frames.store(0, Ordering::Release);
        header.status_code.store(0, Ordering::Release);
        header.block_frames.store(frames as u32, Ordering::Release);
        header.host_sequence.store(sequence, Ordering::Release);
        header
            .worker_state
            .store(PluginIpcState::Idle as u32, Ordering::Release);
        header
            .host_state
            .store(PluginIpcState::HostReady as u32, Ordering::Release);
        Ok(())
    }

    pub fn read_host_context(
        &self,
        frames: usize,
        midi_scratch: &mut Vec<MidiEvent>,
    ) -> io::Result<TransportInfo> {
        let header = self.header();
        let count = header.midi_event_count.load(Ordering::Acquire) as usize;
        if count > MAX_PLUGIN_IPC_MIDI_EVENTS || count > midi_scratch.capacity() {
            return Err(invalid_data("invalid external-plugin MIDI event count"));
        }
        midi_scratch.clear();
        let midi_ptr = unsafe {
            self.mmap
                .as_ref()
                .expect("mapping is present")
                .as_ptr()
                .add(std::mem::size_of::<PluginIpcHeader>())
                .cast::<PluginIpcMidiEvent>()
        };
        for index in 0..count {
            let event = unsafe { *midi_ptr.add(index) };
            if event.sample_offset as usize >= frames || event.len > 3 {
                return Err(invalid_data("invalid external-plugin MIDI event"));
            }
            midi_scratch.push(MidiEvent::new(
                event.sample_offset as usize,
                MidiMessage::new(event.data, event.len),
            ));
        }
        let flags = header.transport_flags.load(Ordering::Acquire);
        let signature = header.transport_time_signature.load(Ordering::Acquire);
        let loop_start = header.transport_loop_start.load(Ordering::Acquire);
        let loop_end = header.transport_loop_end.load(Ordering::Acquire);
        let loop_range = if loop_start == u64::MAX || loop_end == u64::MAX {
            None
        } else {
            LoopRange::new(loop_start, loop_end)
        };
        Ok(TransportInfo {
            playing: flags & 1 != 0,
            recording: flags & 2 != 0,
            looping: flags & 4 != 0,
            sample_position: header.transport_sample_position.load(Ordering::Acquire),
            bpm: f64::from_bits(header.transport_bpm_bits.load(Ordering::Acquire)),
            time_signature: crate::plugin::TimeSignature {
                numerator: (signature >> 16) as u8,
                denominator: signature as u8,
            },
            ppq_position: f64::from_bits(header.transport_ppq_bits.load(Ordering::Acquire)),
            loop_range,
        })
    }

    pub fn read_parameter_events(
        &self,
        frames: usize,
        parameters: &[Parameter],
        scratch: &mut Vec<ParameterEvent>,
    ) -> io::Result<()> {
        let count = self.header().parameter_event_count.load(Ordering::Acquire) as usize;
        if count > MAX_PLUGIN_IPC_PARAMETER_EVENTS || count > scratch.capacity() {
            return Err(invalid_data(
                "invalid external-plugin parameter event count",
            ));
        }
        scratch.clear();
        let ptr = unsafe {
            self.mmap
                .as_ref()
                .expect("mapping is present")
                .as_ptr()
                .add(parameter_event_base_offset())
                .cast::<PluginIpcParameterEvent>()
        };
        for index in 0..count {
            let event = unsafe { *ptr.add(index) };
            let parameter = parameters
                .get(event.parameter_index as usize)
                .ok_or_else(|| invalid_data("invalid external-plugin parameter index"))?;
            if event.sample_offset as usize >= frames {
                return Err(invalid_data("invalid external-plugin parameter offset"));
            }
            let value = match event.value_tag {
                0 => ParameterValue::Float(f32::from_bits(event.value_bits)),
                1 => ParameterValue::Int(event.value_bits as i32),
                2 => ParameterValue::Bool(event.value_bits != 0),
                _ => return Err(invalid_data("invalid external-plugin parameter value tag")),
            };
            scratch.push(ParameterEvent::new(
                event.sample_offset as usize,
                parameter.id.clone(),
                value,
            ));
        }
        Ok(())
    }

    pub fn copy_worker_output(&self, output: &mut [f32]) -> io::Result<usize> {
        if self.worker_state() != PluginIpcState::WorkerReady {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "worker output is not ready",
            ));
        }

        let frames = self.processed_frames();
        self.validate_frame_count(frames)?;
        let output_samples = frames
            .checked_mul(self.layout.output_channels as usize)
            .ok_or_else(|| invalid_data("output frame count overflow"))?;
        if output.len() < output_samples {
            return Err(invalid_input(format!(
                "output has {} samples, expected at least {output_samples}",
                output.len()
            )));
        }

        let shared_output = self.output_slice();
        output[..output_samples].copy_from_slice(&shared_output[..output_samples]);
        Ok(frames)
    }

    pub fn take_worker_request(&self) -> io::Result<Option<PluginIpcRequest>> {
        if self.host_state() != PluginIpcState::HostReady {
            return Ok(None);
        }

        let sequence = self.host_sequence();
        if self.worker_sequence() == sequence {
            return Ok(None);
        }

        let frames = self.block_frames();
        self.validate_frame_count(frames)?;
        self.header()
            .worker_state
            .store(PluginIpcState::WorkerProcessing as u32, Ordering::Release);
        Ok(Some(PluginIpcRequest { sequence, frames }))
    }

    pub fn process_worker_request(
        &mut self,
        plugin: &mut dyn Plugin,
        request: PluginIpcRequest,
        input_scratch: &mut [f32],
        output_scratch: &mut [f32],
        context: &ProcessContext,
    ) -> io::Result<usize> {
        self.validate_frame_count(request.frames)?;
        if request.sequence != self.host_sequence() {
            return Err(invalid_data("stale external-plugin IPC request sequence"));
        }

        let input_samples = request.frames * self.layout.input_channels as usize;
        let output_samples = request.frames * self.layout.output_channels as usize;
        if input_scratch.len() < input_samples || output_scratch.len() < output_samples {
            return Err(invalid_input(
                "external-plugin worker scratch is smaller than the negotiated layout",
            ));
        }
        output_scratch[..output_samples].fill(0.0);
        input_scratch[..input_samples].copy_from_slice(&self.input_slice()[..input_samples]);

        let process_result = catch_unwind(AssertUnwindSafe(|| {
            plugin.process(
                &input_scratch[..input_samples],
                &mut output_scratch[..output_samples],
                context,
            )
        }));

        let frames = match process_result {
            Ok(Ok(frames)) => {
                if frames > request.frames {
                    self.publish_worker_failure(request.sequence, 2);
                    return Err(invalid_data(format!(
                        "external plugin returned {frames} frames for {}-frame request",
                        request.frames
                    )));
                }
                frames
            }
            Ok(Err(err)) => {
                self.publish_worker_failure(request.sequence, 1);
                return Err(invalid_data(format!(
                    "external plugin process failed: {err}"
                )));
            }
            Err(payload) => {
                self.publish_worker_failure(request.sequence, 3);
                return Err(invalid_data(format!(
                    "external plugin process panicked: {}",
                    panic_payload_description(payload.as_ref())
                )));
            }
        };

        let output_samples = frames * self.layout.output_channels as usize;
        self.output_slice_mut()[..output_samples]
            .copy_from_slice(&output_scratch[..output_samples]);
        self.publish_worker_ready(request.sequence, frames)?;
        Ok(frames)
    }

    pub fn publish_worker_ready(&self, sequence: u64, frames: usize) -> io::Result<()> {
        self.validate_frame_count(frames)?;
        let header = self.header();
        header
            .processed_frames
            .store(frames as u32, Ordering::Release);
        header.status_code.store(0, Ordering::Release);
        header.worker_sequence.store(sequence, Ordering::Release);
        header
            .worker_state
            .store(PluginIpcState::WorkerReady as u32, Ordering::Release);
        Ok(())
    }

    pub fn publish_worker_failure(&self, sequence: u64, status_code: u32) {
        let header = self.header();
        header
            .status_code
            .store(status_code.max(1), Ordering::Release);
        header.worker_sequence.store(sequence, Ordering::Release);
        header
            .worker_state
            .store(PluginIpcState::WorkerFailed as u32, Ordering::Release);
    }

    pub fn clear_block(&self) {
        let header = self.header();
        header
            .host_state
            .store(PluginIpcState::Idle as u32, Ordering::Release);
        header
            .worker_state
            .store(PluginIpcState::Idle as u32, Ordering::Release);
    }

    pub fn publish_worker_sandbox_status(
        &self,
        status: PluginSandboxStatusCode,
        backend: PluginSandboxBackendCode,
    ) {
        let header = self.header();
        header.reserved[0].store(status as u32, Ordering::Release);
        header.reserved[1].store(backend as u32, Ordering::Release);
    }

    pub fn worker_sandbox_status(&self) -> PluginSandboxRuntimeStatus {
        let header = self.header();
        PluginSandboxRuntimeStatus {
            status: PluginSandboxStatusCode::from_raw(header.reserved[0].load(Ordering::Acquire)),
            backend: PluginSandboxBackendCode::from_raw(header.reserved[1].load(Ordering::Acquire)),
        }
    }

    /// Publish immutable worker metadata after the hosted plugin is loaded.
    pub fn publish_worker_latency_samples(&self, latency_samples: usize) {
        let header = self.header();
        let latency = u32::try_from(latency_samples).unwrap_or(u32::MAX);
        header.reserved[2].store(latency, Ordering::Relaxed);
        header.reserved[3].store(1, Ordering::Release);
    }

    /// Return the hosted plugin's reported latency once worker metadata is ready.
    pub fn worker_latency_samples(&self) -> Option<usize> {
        let header = self.header();
        (header.reserved[3].load(Ordering::Acquire) != 0)
            .then(|| header.reserved[2].load(Ordering::Relaxed) as usize)
    }

    pub fn audio_slices_mut(&mut self) -> (&mut [f32], &mut [f32]) {
        let input_len = self.layout.input_samples();
        let output_len = self.layout.output_samples();
        let mmap = self.mmap.as_mut().expect("mapping is present");

        // SAFETY: `input_offset` and `output_offset` are derived from a
        // validated layout, are f32-aligned, and the backing mmap has already
        // been checked to cover the full range.
        unsafe {
            let base = mmap.as_mut_ptr();
            let input =
                std::slice::from_raw_parts_mut(base.add(self.input_offset) as *mut f32, input_len);
            let output = std::slice::from_raw_parts_mut(
                base.add(self.output_offset) as *mut f32,
                output_len,
            );
            (input, output)
        }
    }

    pub(super) fn header(&self) -> &PluginIpcHeader {
        header_from_mmap(self.mmap.as_ref().expect("mapping is present")).expect("valid header")
    }

    pub(super) fn validate_frame_count(&self, frames: usize) -> io::Result<()> {
        if frames > self.layout.max_frames as usize {
            return Err(invalid_input(format!(
                "frame count {frames} exceeds max_frames {}",
                self.layout.max_frames
            )));
        }
        Ok(())
    }

    pub(super) fn input_slice(&self) -> &[f32] {
        let len = self.layout.input_samples();
        let mmap = self.mmap.as_ref().expect("mapping is present");
        // SAFETY: layout validation guarantees the range is in-bounds.
        unsafe {
            std::slice::from_raw_parts(mmap.as_ptr().add(self.input_offset) as *const f32, len)
        }
    }

    pub(super) fn output_slice(&self) -> &[f32] {
        let len = self.layout.output_samples();
        let mmap = self.mmap.as_ref().expect("mapping is present");
        // SAFETY: layout validation guarantees the range is in-bounds.
        unsafe {
            std::slice::from_raw_parts(mmap.as_ptr().add(self.output_offset) as *const f32, len)
        }
    }

    pub(super) fn output_slice_mut(&mut self) -> &mut [f32] {
        let len = self.layout.output_samples();
        let mmap = self.mmap.as_mut().expect("mapping is present");
        // SAFETY: layout validation guarantees the range is in-bounds.
        unsafe {
            std::slice::from_raw_parts_mut(
                mmap.as_mut_ptr().add(self.output_offset) as *mut f32,
                len,
            )
        }
    }
}

impl Drop for SecurePluginSharedMemory {
    fn drop(&mut self) {
        self.mmap.take();
        self.file.take();
        if self.remove_on_drop {
            let _ = std::fs::remove_file(&self.path);
            if let Some(session_dir) = &self.session_dir {
                let _ = std::fs::remove_dir(session_dir);
            }
        }
    }
}
