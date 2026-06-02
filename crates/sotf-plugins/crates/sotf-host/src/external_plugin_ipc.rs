//! Shared-memory transport primitives for out-of-process external plugins.
//!
//! This module intentionally keeps the realtime audio path free of RPC
//! dependencies. The host creates an owner-only mapping, validates it with the
//! same defensive checks used by the systemwide HAL path, and hands only this
//! descriptor to the trusted plugin worker process. The worker should copy
//! between shared memory and private plugin buffers; unknown plugins must never
//! receive direct pointers into this mapping.

use std::fs::{File, OpenOptions};
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::plugin::{Plugin, ProcessContext};
use memmap2::{MmapMut, MmapOptions};

const PLUGIN_IPC_MAGIC: u32 = 0x5350_4950; // 'SPIP'
const PLUGIN_IPC_VERSION: u32 = 1;
const MAX_PLUGIN_IPC_FRAMES: u32 = 8192;
const MAX_PLUGIN_IPC_CHANNELS: u32 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PluginSandboxStatusCode {
    Unknown = 0,
    Disabled = 1,
    Enforced = 2,
    Unsupported = 3,
}

impl PluginSandboxStatusCode {
    fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::Disabled,
            2 => Self::Enforced,
            3 => Self::Unsupported,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PluginSandboxBackendCode {
    Unknown = 0,
    LinuxLandlock = 1,
    MacosProcessIsolation = 2,
    WindowsProcessIsolation = 3,
    MacosAppSandboxHelper = 4,
}

impl PluginSandboxBackendCode {
    fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::LinuxLandlock,
            2 => Self::MacosProcessIsolation,
            3 => Self::WindowsProcessIsolation,
            4 => Self::MacosAppSandboxHelper,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginSandboxRuntimeStatus {
    pub status: PluginSandboxStatusCode,
    pub backend: PluginSandboxBackendCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginIpcLayout {
    pub sample_rate: u32,
    pub max_frames: u32,
    pub input_channels: u32,
    pub output_channels: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PluginIpcState {
    Idle = 0,
    HostReady = 1,
    WorkerProcessing = 2,
    WorkerReady = 3,
    WorkerFailed = 4,
}

impl PluginIpcState {
    fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::HostReady,
            2 => Self::WorkerProcessing,
            3 => Self::WorkerReady,
            4 => Self::WorkerFailed,
            _ => Self::Idle,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginIpcRequest {
    pub sequence: u64,
    pub frames: usize,
}

impl PluginIpcLayout {
    pub fn new(
        sample_rate: u32,
        max_frames: u32,
        input_channels: u32,
        output_channels: u32,
    ) -> io::Result<Self> {
        if sample_rate == 0 {
            return Err(invalid_input("sample_rate must be non-zero"));
        }
        if max_frames == 0 || max_frames > MAX_PLUGIN_IPC_FRAMES {
            return Err(invalid_input(format!(
                "max_frames must be in 1..={MAX_PLUGIN_IPC_FRAMES}, got {max_frames}"
            )));
        }
        if input_channels > MAX_PLUGIN_IPC_CHANNELS || output_channels > MAX_PLUGIN_IPC_CHANNELS {
            return Err(invalid_input(format!(
                "channel counts must be <= {MAX_PLUGIN_IPC_CHANNELS}, got in={input_channels} out={output_channels}"
            )));
        }
        if input_channels == 0 && output_channels == 0 {
            return Err(invalid_input(
                "at least one input or output channel is required",
            ));
        }

        Ok(Self {
            sample_rate,
            max_frames,
            input_channels,
            output_channels,
        })
    }

    fn input_samples(self) -> usize {
        self.max_frames as usize * self.input_channels as usize
    }

    fn output_samples(self) -> usize {
        self.max_frames as usize * self.output_channels as usize
    }
}

#[repr(C, align(64))]
struct PluginIpcHeader {
    magic: AtomicU32,
    version: AtomicU32,
    sample_rate: AtomicU32,
    max_frames: AtomicU32,
    input_channels: AtomicU32,
    output_channels: AtomicU32,
    block_frames: AtomicU32,
    processed_frames: AtomicU32,
    status_code: AtomicU32,
    _pad0: AtomicU32,
    host_sequence: AtomicU64,
    worker_sequence: AtomicU64,
    host_state: AtomicU32,
    worker_state: AtomicU32,
    reserved: [AtomicU32; 6],
}

impl PluginIpcHeader {
    fn initialize(&self, layout: PluginIpcLayout) {
        self.sample_rate
            .store(layout.sample_rate, Ordering::Release);
        self.max_frames.store(layout.max_frames, Ordering::Release);
        self.input_channels
            .store(layout.input_channels, Ordering::Release);
        self.output_channels
            .store(layout.output_channels, Ordering::Release);
        self.block_frames.store(0, Ordering::Release);
        self.processed_frames.store(0, Ordering::Release);
        self.status_code.store(0, Ordering::Release);
        self._pad0.store(0, Ordering::Release);
        self.host_sequence.store(0, Ordering::Release);
        self.worker_sequence.store(0, Ordering::Release);
        self.host_state
            .store(PluginIpcState::Idle as u32, Ordering::Release);
        self.worker_state
            .store(PluginIpcState::Idle as u32, Ordering::Release);
        self.reserved[0].store(PluginSandboxStatusCode::Unknown as u32, Ordering::Release);
        self.reserved[1].store(PluginSandboxBackendCode::Unknown as u32, Ordering::Release);
        self.version.store(PLUGIN_IPC_VERSION, Ordering::Release);
        self.magic.store(PLUGIN_IPC_MAGIC, Ordering::Release);
    }

    fn read_layout(&self) -> io::Result<PluginIpcLayout> {
        if self.magic.load(Ordering::Acquire) != PLUGIN_IPC_MAGIC {
            return Err(invalid_data("invalid external-plugin IPC magic"));
        }
        if self.version.load(Ordering::Acquire) != PLUGIN_IPC_VERSION {
            return Err(invalid_data("unsupported external-plugin IPC version"));
        }
        PluginIpcLayout::new(
            self.sample_rate.load(Ordering::Acquire),
            self.max_frames.load(Ordering::Acquire),
            self.input_channels.load(Ordering::Acquire),
            self.output_channels.load(Ordering::Acquire),
        )
    }
}

pub struct SecurePluginSharedMemory {
    path: PathBuf,
    session_dir: Option<PathBuf>,
    file: Option<File>,
    mmap: Option<MmapMut>,
    layout: PluginIpcLayout,
    input_offset: usize,
    output_offset: usize,
    remove_on_drop: bool,
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

    fn create_at_inner(
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

    fn from_parts(
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

    fn suspend_access_for_plugin_call(&mut self) {
        self.mmap.take();
        self.file.take();
    }

    fn restore_access_after_plugin_call(&mut self) -> io::Result<()> {
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

    fn header(&self) -> &PluginIpcHeader {
        header_from_mmap(self.mmap.as_ref().expect("mapping is present")).expect("valid header")
    }

    fn validate_frame_count(&self, frames: usize) -> io::Result<()> {
        if frames > self.layout.max_frames as usize {
            return Err(invalid_input(format!(
                "frame count {frames} exceeds max_frames {}",
                self.layout.max_frames
            )));
        }
        Ok(())
    }

    fn input_slice(&self) -> &[f32] {
        let len = self.layout.input_samples();
        let mmap = self.mmap.as_ref().expect("mapping is present");
        // SAFETY: layout validation guarantees the range is in-bounds.
        unsafe {
            std::slice::from_raw_parts(mmap.as_ptr().add(self.input_offset) as *const f32, len)
        }
    }

    fn output_slice(&self) -> &[f32] {
        let len = self.layout.output_samples();
        let mmap = self.mmap.as_ref().expect("mapping is present");
        // SAFETY: layout validation guarantees the range is in-bounds.
        unsafe {
            std::slice::from_raw_parts(mmap.as_ptr().add(self.output_offset) as *const f32, len)
        }
    }

    fn output_slice_mut(&mut self) -> &mut [f32] {
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

fn header_from_mmap(mmap: &MmapMut) -> io::Result<&PluginIpcHeader> {
    if mmap.len() < std::mem::size_of::<PluginIpcHeader>() {
        return Err(invalid_data("external-plugin IPC header is truncated"));
    }
    // SAFETY: mmap pages are at least pointer-aligned; PluginIpcHeader is at
    // offset zero and uses only atomic integer fields with C layout.
    Ok(unsafe { &*(mmap.as_ptr() as *const PluginIpcHeader) })
}

fn audio_base_offset() -> usize {
    align_up(
        std::mem::size_of::<PluginIpcHeader>(),
        std::mem::align_of::<f32>(),
    )
}

fn total_size(layout: PluginIpcLayout) -> io::Result<usize> {
    let input_bytes = layout
        .input_samples()
        .checked_mul(std::mem::size_of::<f32>())
        .ok_or_else(|| invalid_input("input buffer size overflow"))?;
    let output_bytes = layout
        .output_samples()
        .checked_mul(std::mem::size_of::<f32>())
        .ok_or_else(|| invalid_input("output buffer size overflow"))?;
    audio_base_offset()
        .checked_add(input_bytes)
        .and_then(|size| size.checked_add(output_bytes))
        .ok_or_else(|| invalid_input("shared-memory size overflow"))
}

fn align_up(value: usize, alignment: usize) -> usize {
    debug_assert!(alignment.is_power_of_two());
    (value + alignment - 1) & !(alignment - 1)
}

fn create_session_dir() -> io::Result<PathBuf> {
    let root = std::env::temp_dir().join(format!("sotf-plugin-ipc-{}", current_user_tag()));
    ensure_secure_parent_dir(&root)?;

    for _ in 0..128 {
        let token: u128 = rand::random();
        let session_dir = root.join(format!("session-{}-{token:032x}", std::process::id()));
        match std::fs::create_dir(&session_dir) {
            Ok(()) => {
                clamp_dir_permissions(&session_dir)?;
                return Ok(session_dir);
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate unique external-plugin IPC session directory",
    ))
}

#[cfg(unix)]
fn current_user_tag() -> String {
    // SAFETY: `getuid` has no preconditions and cannot fail.
    unsafe { libc::getuid().to_string() }
}

#[cfg(windows)]
fn current_user_tag() -> String {
    std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(unix)]
fn ensure_secure_parent_dir(parent: &Path) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    if !parent.exists() {
        std::fs::create_dir_all(parent)?;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    }

    let metadata = std::fs::symlink_metadata(parent)?;
    if !metadata.file_type().is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not a directory", parent.display()),
        ));
    }
    if metadata.uid() != unsafe { libc::getuid() } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not owned by the current user", parent.display()),
        ));
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(windows)]
fn ensure_secure_parent_dir(parent: &Path) -> io::Result<()> {
    if !parent.exists() {
        std::fs::create_dir_all(parent)?;
    }
    if !std::fs::metadata(parent)?.file_type().is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not a directory", parent.display()),
        ));
    }
    set_windows_owner_only_dacl(parent)?;
    validate_windows_owner_only_dacl(parent)?;
    Ok(())
}

#[cfg(unix)]
fn open_new_shared_memory_file(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW);
    let file = options.open(path)?;
    validate_shared_memory_file(&file, path)?;
    Ok(file)
}

#[cfg(windows)]
fn open_new_shared_memory_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    let file = options.read(true).write(true).create_new(true).open(path)?;
    if !file.metadata()?.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "external-plugin IPC path is not a regular file",
        ));
    }
    set_windows_owner_only_dacl(path)?;
    validate_shared_memory_file(&file, path)?;
    Ok(file)
}

fn open_existing_shared_memory_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    configure_existing_open_options(&mut options);
    let file = options.open(path)?;
    validate_shared_memory_file(&file, path)?;
    Ok(file)
}

#[cfg(unix)]
fn configure_existing_open_options(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.custom_flags(libc::O_NOFOLLOW);
}

#[cfg(windows)]
fn configure_existing_open_options(_options: &mut OpenOptions) {}

#[cfg(unix)]
pub(crate) fn validate_shared_memory_file(file: &File, path: &Path) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let link_metadata = std::fs::symlink_metadata(path)?;
    if link_metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is a symlink", path.display()),
        ));
    }

    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not a regular file", path.display()),
        ));
    }
    if metadata.uid() != unsafe { libc::getuid() } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not owned by the current user", path.display()),
        ));
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not owner-only", path.display()),
        ));
    }
    Ok(())
}

#[cfg(windows)]
pub(crate) fn validate_shared_memory_file(file: &File, _path: &Path) -> io::Result<()> {
    if !file.metadata()?.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "external-plugin IPC path is not a regular file",
        ));
    }
    validate_windows_owner_only_dacl(_path)?;
    Ok(())
}

#[cfg(unix)]
fn clamp_file_permissions(file: &File) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
}

#[cfg(windows)]
fn clamp_file_permissions(file: &File) -> io::Result<()> {
    let _ = file;
    Ok(())
}

#[cfg(unix)]
fn clamp_dir_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

#[cfg(windows)]
fn clamp_dir_permissions(path: &Path) -> io::Result<()> {
    set_windows_owner_only_dacl(path)
}

#[cfg(windows)]
fn set_windows_owner_only_dacl(path: &Path) -> io::Result<()> {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, LocalFree};
    use windows_sys::Win32::Security::Authorization::{
        EXPLICIT_ACCESS_W, SE_FILE_OBJECT, SET_ACCESS, SetEntriesInAclW, SetNamedSecurityInfoW,
        TRUSTEE_W,
    };
    use windows_sys::Win32::Security::{
        DACL_SECURITY_INFORMATION, NO_INHERITANCE, PROTECTED_DACL_SECURITY_INFORMATION, PSID,
    };
    use windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS;

    let mut user_sid = current_windows_user_sid()?;
    let mut trustee = TRUSTEE_W::default();
    // SAFETY: `user_sid` contains a copied, valid SID for the current process
    // token and remains alive for the duration of this ACL construction.
    unsafe {
        windows_sys::Win32::Security::Authorization::BuildTrusteeWithSidW(
            &mut trustee,
            user_sid.as_mut_ptr() as PSID,
        );
    }

    let explicit = EXPLICIT_ACCESS_W {
        grfAccessPermissions: FILE_ALL_ACCESS,
        grfAccessMode: SET_ACCESS,
        grfInheritance: NO_INHERITANCE,
        Trustee: trustee,
    };

    let mut dacl = null_mut();
    // SAFETY: pointers reference valid inputs; `dacl` is released with LocalFree.
    let status = unsafe { SetEntriesInAclW(1, &explicit, null(), &mut dacl) };
    if status != ERROR_SUCCESS {
        return Err(win32_error(status));
    }

    let path_w = windows_path_wide(path);
    // SAFETY: `path_w` is NUL-terminated; `dacl` was allocated by
    // SetEntriesInAclW. We set a protected DACL so inheritable parent ACEs do
    // not grant access to unrelated principals.
    let status = unsafe {
        SetNamedSecurityInfoW(
            path_w.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            dacl,
            null(),
        )
    };
    // SAFETY: `dacl` was allocated by the Windows ACL APIs.
    unsafe {
        LocalFree(dacl as _);
    }

    if status != ERROR_SUCCESS {
        return Err(win32_error(status));
    }
    Ok(())
}

#[cfg(windows)]
fn validate_windows_owner_only_dacl(path: &Path) -> io::Result<()> {
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, LocalFree};
    use windows_sys::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        ACCESS_ALLOWED_ACE, ACL, DACL_SECURITY_INFORMATION, EqualSid, GetAce,
        OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
    };
    use windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS;
    use windows_sys::Win32::System::SystemServices::ACCESS_ALLOWED_ACE_TYPE;

    let mut user_sid = current_windows_user_sid()?;
    let path_w = windows_path_wide(path);
    let mut owner: PSID = null_mut();
    let mut dacl: *mut ACL = null_mut();
    let mut security_descriptor: PSECURITY_DESCRIPTOR = null_mut();

    // SAFETY: output pointers are valid and the returned security descriptor is
    // released with LocalFree below.
    let status = unsafe {
        GetNamedSecurityInfoW(
            path_w.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut security_descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(win32_error(status));
    }

    let result = (|| {
        if owner.is_null() || unsafe { EqualSid(owner, user_sid.as_mut_ptr() as PSID) } == 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} is not owned by the current user", path.display()),
            ));
        }
        if dacl.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} has no owner-only DACL", path.display()),
            ));
        }

        // SAFETY: `dacl` comes from GetNamedSecurityInfoW and is valid until
        // `security_descriptor` is freed.
        if unsafe { (*dacl).AceCount } != 1 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} DACL is not owner-only", path.display()),
            ));
        }

        let mut ace = null_mut();
        // SAFETY: index 0 is valid because AceCount == 1.
        if unsafe { GetAce(dacl, 0, &mut ace) } == 0 || ace.is_null() {
            return Err(io::Error::last_os_error());
        }
        let allowed = ace as *const ACCESS_ALLOWED_ACE;
        // SAFETY: GetAce returned a valid ACE pointer.
        let header = unsafe { (*allowed).Header };
        if header.AceType != ACCESS_ALLOWED_ACE_TYPE as u8 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} DACL contains a non-allow ACE", path.display()),
            ));
        }
        // SAFETY: ACCESS_ALLOWED_ACE stores the SID immediately at SidStart.
        let ace_sid = unsafe { &(*allowed).SidStart as *const u32 as PSID };
        if unsafe { EqualSid(ace_sid, user_sid.as_mut_ptr() as PSID) } == 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} DACL grants a non-owner principal", path.display()),
            ));
        }
        // SAFETY: GetAce returned a valid ACCESS_ALLOWED_ACE.
        if unsafe { (*allowed).Mask } & FILE_ALL_ACCESS != FILE_ALL_ACCESS {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} DACL does not grant owner full access", path.display()),
            ));
        }
        Ok(())
    })();

    if !security_descriptor.is_null() {
        // SAFETY: security_descriptor was allocated by GetNamedSecurityInfoW.
        unsafe {
            LocalFree(security_descriptor as _);
        }
    }
    result
}

#[cfg(windows)]
fn current_windows_user_sid() -> io::Result<Vec<u8>> {
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_INSUFFICIENT_BUFFER, GetLastError, HANDLE,
    };
    use windows_sys::Win32::Security::{
        CopySid, GetLengthSid, GetTokenInformation, PSID, TOKEN_QUERY, TOKEN_USER, TokenUser,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    struct HandleGuard(HANDLE);
    impl Drop for HandleGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: handle was returned by OpenProcessToken.
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
    }

    let mut token = null_mut();
    // SAFETY: GetCurrentProcess returns a pseudo-handle valid for this call.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let _token = HandleGuard(token);

    let mut needed = 0;
    // SAFETY: first call probes the required buffer size.
    let ok = unsafe { GetTokenInformation(token, TokenUser, null_mut(), 0, &mut needed) };
    if ok != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "GetTokenInformation unexpectedly succeeded without a buffer",
        ));
    }
    if unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER || needed == 0 {
        return Err(io::Error::last_os_error());
    }

    let mut token_user = vec![0u8; needed as usize];
    // SAFETY: `token_user` is sized from the API-provided length.
    if unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            token_user.as_mut_ptr() as *mut _,
            needed,
            &mut needed,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }

    // SAFETY: successful TokenUser query returns a TOKEN_USER at the buffer
    // start, whose SID remains valid while `token_user` is alive.
    let source_sid = unsafe { (*(token_user.as_ptr() as *const TOKEN_USER)).User.Sid };
    let sid_len = unsafe { GetLengthSid(source_sid) };
    if sid_len == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut sid = vec![0u8; sid_len as usize];
    // SAFETY: destination buffer is exactly GetLengthSid bytes.
    if unsafe { CopySid(sid_len, sid.as_mut_ptr() as PSID, source_sid) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(sid)
}

#[cfg(windows)]
fn windows_path_wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

#[cfg(windows)]
fn win32_error(code: u32) -> io::Error {
    io::Error::from_raw_os_error(code as i32)
}

fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn panic_payload_description(payload: &(dyn std::any::Any + Send)) -> String {
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
    use crate::parameters::{Parameter, ParameterId, ParameterValue};
    use crate::plugin::{PluginInfo, PluginResult};

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

    #[test]
    fn test_secure_plugin_shared_memory_roundtrip() {
        let layout = PluginIpcLayout::new(48_000, 128, 2, 2).unwrap();
        let mut shared = SecurePluginSharedMemory::create(layout).unwrap();
        assert_eq!(shared.layout(), layout);
        assert!(shared.path().exists());

        shared.publish_host_sequence(7);
        shared.publish_worker_sequence(6);
        assert_eq!(shared.host_sequence(), 7);
        assert_eq!(shared.worker_sequence(), 6);

        let (input, output) = shared.audio_slices_mut();
        assert_eq!(input.len(), 256);
        assert_eq!(output.len(), 256);
        input[0] = 0.25;
        output[0] = -0.5;
        assert_eq!(input[0], 0.25);
        assert_eq!(output[0], -0.5);
    }

    #[test]
    fn test_publish_host_block_clears_only_current_output_block() {
        let layout = PluginIpcLayout::new(48_000, 128, 2, 2).unwrap();
        let mut shared = SecurePluginSharedMemory::create(layout).unwrap();
        let input = vec![0.25, -0.5, 1.0, -1.0];

        {
            let (_input, output) = shared.audio_slices_mut();
            output.fill(9.0);
        }

        shared.publish_host_block(42, 2, &input).unwrap();

        let (_input, output) = shared.audio_slices_mut();
        assert_eq!(&output[..4], &[0.0, 0.0, 0.0, 0.0]);
        assert_eq!(output[4], 9.0);
        assert_eq!(output[output.len() - 1], 9.0);
    }

    #[test]
    fn test_worker_processes_request_through_private_buffers() {
        let layout = PluginIpcLayout::new(48_000, 128, 2, 2).unwrap();
        let mut host_shared = SecurePluginSharedMemory::create(layout).unwrap();
        let mut worker_shared =
            SecurePluginSharedMemory::open_existing(host_shared.path()).unwrap();
        let input = vec![0.25, -0.5, 1.0, -1.0];
        let mut output = vec![0.0; input.len()];

        host_shared.publish_host_block(42, 2, &input).unwrap();
        assert_eq!(worker_shared.host_state(), PluginIpcState::HostReady);
        let request = worker_shared
            .take_worker_request()
            .unwrap()
            .expect("request should be ready");

        let mut plugin = ScalePlugin {
            channels: 2,
            factor: 2.0,
        };
        let mut input_scratch = Vec::new();
        let mut output_scratch = Vec::new();
        let frames = worker_shared
            .process_worker_request(
                &mut plugin,
                request,
                &mut input_scratch,
                &mut output_scratch,
            )
            .unwrap();

        assert_eq!(frames, 2);
        assert_eq!(worker_shared.worker_state(), PluginIpcState::WorkerReady);
        assert_eq!(host_shared.copy_worker_output(&mut output).unwrap(), 2);
        assert_eq!(output, vec![0.5, -1.0, 2.0, -2.0]);
        assert_eq!(host_shared.worker_sequence(), 42);
    }

    #[test]
    fn test_open_existing_validates_header() {
        let layout = PluginIpcLayout::new(48_000, 64, 1, 2).unwrap();
        let shared = SecurePluginSharedMemory::create(layout).unwrap();
        let reopened = SecurePluginSharedMemory::open_existing(shared.path()).unwrap();
        assert_eq!(reopened.layout(), layout);
    }

    #[cfg(unix)]
    #[test]
    fn test_secure_plugin_shared_memory_uses_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let layout = PluginIpcLayout::new(48_000, 64, 1, 1).unwrap();
        let shared = SecurePluginSharedMemory::create(layout).unwrap();

        let file_mode = std::fs::metadata(shared.path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600);

        let parent_mode = std::fs::metadata(shared.path().parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(parent_mode, 0o700);
    }

    #[cfg(unix)]
    #[test]
    fn test_open_existing_rejects_symlink() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "sotf-plugin-ipc-symlink-test-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();

        let target = root.join("target.shm");
        std::fs::write(&target, b"not a valid mapping").unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();
        let link = root.join("link.shm");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        assert!(SecurePluginSharedMemory::open_existing(&link).is_err());

        let _ = std::fs::remove_file(link);
        let _ = std::fs::remove_file(target);
        let _ = std::fs::remove_dir(root);
    }
}
