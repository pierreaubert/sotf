use super::PluginIpcHeader;
use super::consts::MAX_PLUGIN_IPC_MIDI_EVENTS;
use super::consts::MAX_PLUGIN_IPC_PARAMETER_EVENTS;
use super::consts::PLUGIN_IPC_CONTROL_BYTES;
use super::consts::PLUGIN_IPC_MAGIC;
use super::consts::PLUGIN_IPC_VERSION;
use super::invalid::invalid_data;
use super::misc::align_up;
use super::plugin_ipc_layout::PluginIpcLayout;
use super::plugin_ipc_state::PluginIpcState;
use super::plugin_sandbox_backend_code::PluginSandboxBackendCode;
use super::plugin_sandbox_status_code::PluginSandboxStatusCode;
use memmap2::MmapMut;
use std::io;
use std::sync::atomic::Ordering;

impl PluginIpcHeader {
    pub(super) fn initialize(&self, layout: PluginIpcLayout) {
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
        self.midi_event_count.store(0, Ordering::Release);
        self.parameter_event_count.store(0, Ordering::Release);
        self.transport_flags.store(0, Ordering::Release);
        self.transport_sample_position.store(0, Ordering::Release);
        self.transport_bpm_bits
            .store(120.0_f64.to_bits(), Ordering::Release);
        self.transport_ppq_bits.store(0, Ordering::Release);
        self.transport_time_signature
            .store((4_u32 << 16) | 4, Ordering::Release);
        self._pad1.store(0, Ordering::Release);
        self.transport_loop_start.store(u64::MAX, Ordering::Release);
        self.transport_loop_end.store(u64::MAX, Ordering::Release);
        self.control_sequence.store(0, Ordering::Release);
        self.control_worker_sequence.store(0, Ordering::Release);
        self.control_state.store(0, Ordering::Release);
        self.control_request_len.store(0, Ordering::Release);
        self.control_response_len.store(0, Ordering::Release);
        self.control_status.store(0, Ordering::Release);
        self.reserved[0].store(PluginSandboxStatusCode::Unknown as u32, Ordering::Release);
        self.reserved[1].store(PluginSandboxBackendCode::Unknown as u32, Ordering::Release);
        self.reserved[2].store(0, Ordering::Release);
        self.reserved[3].store(0, Ordering::Release);
        self.version.store(PLUGIN_IPC_VERSION, Ordering::Release);
        self.magic.store(PLUGIN_IPC_MAGIC, Ordering::Release);
    }

    pub(super) fn read_layout(&self) -> io::Result<PluginIpcLayout> {
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

pub(super) fn header_from_mmap(mmap: &MmapMut) -> io::Result<&PluginIpcHeader> {
    if mmap.len() < std::mem::size_of::<PluginIpcHeader>() {
        return Err(invalid_data("external-plugin IPC header is truncated"));
    }
    // SAFETY: mmap pages are at least pointer-aligned; PluginIpcHeader is at
    // offset zero and uses only atomic integer fields with C layout.
    Ok(unsafe { &*(mmap.as_ptr() as *const PluginIpcHeader) })
}

pub(super) fn audio_base_offset() -> usize {
    align_up(
        std::mem::size_of::<PluginIpcHeader>()
            + MAX_PLUGIN_IPC_MIDI_EVENTS * std::mem::size_of::<super::PluginIpcMidiEvent>()
            + MAX_PLUGIN_IPC_PARAMETER_EVENTS
                * std::mem::size_of::<super::PluginIpcParameterEvent>()
            + PLUGIN_IPC_CONTROL_BYTES,
        std::mem::align_of::<f32>(),
    )
}

pub(super) fn control_base_offset() -> usize {
    std::mem::size_of::<PluginIpcHeader>()
        + MAX_PLUGIN_IPC_MIDI_EVENTS * std::mem::size_of::<super::PluginIpcMidiEvent>()
        + MAX_PLUGIN_IPC_PARAMETER_EVENTS * std::mem::size_of::<super::PluginIpcParameterEvent>()
}

pub(super) fn parameter_event_base_offset() -> usize {
    std::mem::size_of::<PluginIpcHeader>()
        + MAX_PLUGIN_IPC_MIDI_EVENTS * std::mem::size_of::<super::PluginIpcMidiEvent>()
}
