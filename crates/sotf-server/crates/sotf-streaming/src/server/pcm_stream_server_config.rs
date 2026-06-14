use super::consts::DEFAULT_CLIENT_QUEUE_CAPACITY_CHUNKS;
use super::consts::DEFAULT_QUEUE_CAPACITY_CHUNKS;

#[derive(Clone, Debug)]
pub struct PcmStreamServerConfig {
    pub bind_addr: String,
    pub port: u16,
    pub initial_sample_rate: u32,
    pub initial_channels: u16,
    pub queue_capacity_chunks: usize,
    pub client_queue_capacity_chunks: usize,
}

impl Default for PcmStreamServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0".to_string(),
            port: 17_890,
            initial_sample_rate: 48_000,
            initial_channels: 2,
            queue_capacity_chunks: DEFAULT_QUEUE_CAPACITY_CHUNKS,
            client_queue_capacity_chunks: DEFAULT_CLIENT_QUEUE_CAPACITY_CHUNKS,
        }
    }
}
