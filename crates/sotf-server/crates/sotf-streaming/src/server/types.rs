use super::pcm_stream_chunk::PcmStreamChunk;
use super::pcm_stream_format::PcmStreamFormat;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::mpsc::SyncSender;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcmStreamStats {
    pub local_addr: SocketAddr,
    pub client_count: u32,
    pub published_chunks: u64,
    pub dropped_chunks: u64,
    pub published_frames: u64,
    pub published_bytes: u64,
    pub current_format: PcmStreamFormat,
}

#[derive(Debug)]
pub(super) enum ClientMessage {
    Chunk(Arc<PcmStreamChunk>),
    FormatChanged,
}

#[derive(Debug)]
pub(super) struct ClientRegistration {
    pub(super) tx: SyncSender<ClientMessage>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StreamKind {
    Wav,
    RawF32,
}
