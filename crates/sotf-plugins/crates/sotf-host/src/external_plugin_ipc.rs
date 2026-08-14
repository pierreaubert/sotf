//! Shared-memory transport primitives for out-of-process external plugins.
//!
//! This module intentionally keeps the realtime audio path free of RPC
//! dependencies. The host creates an owner-only mapping, validates it with the
//! same defensive checks used by the systemwide HAL path, and hands only this
//! descriptor to the trusted plugin worker process. The worker should copy
//! between shared memory and private plugin buffers; unknown plugins must never
//! receive direct pointers into this mapping.

use crate::parameters::{Parameter, ParameterId, ParameterValue};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, AtomicU64};

mod clamp;
mod configure;
mod consts;
mod current;
mod ensure;
mod invalid;
mod misc;
mod open;
mod plugin_ipc_header;
mod plugin_ipc_layout;
mod plugin_ipc_state;
mod plugin_sandbox_backend_code;
mod plugin_sandbox_status_code;
mod secure_plugin_shared_memory;
#[cfg(test)]
mod tests;
mod types;
mod validate;

pub use plugin_ipc_layout::*;
pub use plugin_ipc_state::*;
pub use plugin_sandbox_backend_code::*;
pub use plugin_sandbox_status_code::*;
pub use secure_plugin_shared_memory::*;
pub use types::*;
pub(crate) use validate::*;

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
    midi_event_count: AtomicU32,
    parameter_event_count: AtomicU32,
    transport_flags: AtomicU32,
    transport_sample_position: AtomicU64,
    transport_bpm_bits: AtomicU64,
    transport_ppq_bits: AtomicU64,
    transport_time_signature: AtomicU32,
    _pad1: AtomicU32,
    transport_loop_start: AtomicU64,
    transport_loop_end: AtomicU64,
    control: PluginIpcControlHeader,
    reserved: [AtomicU32; 6],
}

/// Control-plane atomics are grouped to keep the mapped top-level header
/// reviewable without changing the C layout or audio-plane ownership.
#[repr(C)]
struct PluginIpcControlHeader {
    sequence: AtomicU64,
    worker_sequence: AtomicU64,
    state: AtomicU32,
    request_len: AtomicU32,
    response_len: AtomicU32,
    status: AtomicU32,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PluginIpcControlRequest {
    Describe,
    Set {
        id: ParameterId,
        value: ParameterValue,
    },
    Get {
        id: ParameterId,
    },
    SaveState,
    LoadState {
        state: Vec<u8>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PluginIpcControlResponse {
    Description { parameters: Vec<Parameter> },
    Value(Option<ParameterValue>),
    State(Vec<u8>),
    Ack,
    Error(String),
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct PluginIpcMidiEvent {
    sample_offset: u32,
    data: [u8; 3],
    len: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct PluginIpcParameterEvent {
    pub(crate) sample_offset: u32,
    pub(crate) parameter_index: u32,
    pub(crate) value_tag: u32,
    pub(crate) value_bits: u32,
}
