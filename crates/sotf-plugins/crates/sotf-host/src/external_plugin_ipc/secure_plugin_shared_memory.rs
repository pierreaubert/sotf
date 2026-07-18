use super::PluginIpcHeader;
#[cfg(unix)]
use super::clamp::clamp_file_permissions;
#[cfg(windows)]
use super::clamp::clamp_file_permissions;
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
use super::plugin_ipc_header::header_from_mmap;
use super::plugin_ipc_layout::PluginIpcLayout;
use super::plugin_ipc_layout::total_size;
use super::plugin_ipc_state::PluginIpcState;
use super::plugin_sandbox_backend_code::PluginSandboxBackendCode;
use super::plugin_sandbox_status_code::PluginSandboxStatusCode;
use super::types::PluginIpcRequest;
use super::types::PluginSandboxRuntimeStatus;
use crate::plugin::{Plugin, ProcessContext};
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
        self.validate_frame_count(frames)?;
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

        let header = self.header();
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
        input_scratch: &mut Vec<f32>,
        output_scratch: &mut Vec<f32>,
    ) -> io::Result<usize> {
        self.validate_frame_count(request.frames)?;
        if request.sequence != self.host_sequence() {
            return Err(invalid_data("stale external-plugin IPC request sequence"));
        }

        let input_samples = request.frames * self.layout.input_channels as usize;
        let output_samples = request.frames * self.layout.output_channels as usize;
        input_scratch.resize(input_samples, 0.0);
        output_scratch.resize(output_samples, 0.0);
        output_scratch.fill(0.0);
        input_scratch.copy_from_slice(&self.input_slice()[..input_samples]);

        let context = ProcessContext::new(self.layout.sample_rate, request.frames);
        self.suspend_access_for_plugin_call();
        let process_result = catch_unwind(AssertUnwindSafe(|| {
            plugin.process(&input_scratch[..], &mut output_scratch[..], &context)
        }));
        self.restore_access_after_plugin_call()?;

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

    pub(super) fn suspend_access_for_plugin_call(&mut self) {
        self.mmap.take();
        self.file.take();
    }

    pub(super) fn restore_access_after_plugin_call(&mut self) -> io::Result<()> {
        if self.file.is_some() && self.mmap.is_some() {
            return Ok(());
        }

        let file = open_existing_shared_memory_file(&self.path)?;
        let size = file.metadata()?.len() as usize;
        if size < audio_base_offset() {
            return Err(invalid_data("external-plugin IPC mapping is too small"));
        }

        // SAFETY: The descriptor is revalidated before exposing header or audio
        // slices after the unknown plugin call returns.
        let mmap = unsafe { MmapOptions::new().len(size).map_mut(&file)? };
        let header = header_from_mmap(&mmap)?;
        let layout = header.read_layout()?;
        if layout != self.layout {
            return Err(invalid_data("external-plugin IPC layout changed"));
        }
        if size < total_size(layout)? {
            return Err(invalid_data(
                "external-plugin IPC mapping has truncated body",
            ));
        }

        self.file = Some(file);
        self.mmap = Some(mmap);
        Ok(())
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
