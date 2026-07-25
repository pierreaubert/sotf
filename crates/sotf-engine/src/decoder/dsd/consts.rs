pub(super) const DSF_HEADER_LEN: usize = 12;

pub(super) const DSF_ROOT_CHUNK_SIZE: usize = 28;

pub(super) const DSF_FMT_CHUNK_SIZE: usize = 52;

pub(super) const DFF_HEADER_LEN: usize = 12;

pub(super) const DSD_TO_PCM_DECIMATION: u64 = 64;

pub(super) const DSD_DECODE_CHUNK_FRAMES: u64 = 4096;

/// Defensive limit for decoded DSD channel state and scratch buffers.
pub(super) const MAX_DSD_CHANNELS: u16 = 64;
