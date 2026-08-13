pub(super) const PLUGIN_IPC_MAGIC: u32 = 0x5350_4950; // 'SPIP'

pub(super) const PLUGIN_IPC_VERSION: u32 = 2;

pub(super) const MAX_PLUGIN_IPC_MIDI_EVENTS: usize = 1024;
pub(super) const MAX_PLUGIN_IPC_PARAMETER_EVENTS: usize = 1024;
pub(super) const PLUGIN_IPC_CONTROL_BYTES: usize = 64 * 1024;

pub(super) const MAX_PLUGIN_IPC_FRAMES: u32 = 8192;

pub(super) const MAX_PLUGIN_IPC_CHANNELS: u32 = 128;
