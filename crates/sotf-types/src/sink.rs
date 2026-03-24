//! Audio output sink types.

/// Configuration for opening an audio sink.
#[derive(Debug, Clone)]
pub struct SinkConfig {
    pub sample_rate: u32,
    pub channels: usize,
    pub buffer_ms: u32,
    /// Optional device name/identifier. Meaning is sink-specific.
    pub device: Option<String>,
    /// Allow virtual output devices (for loopback testing).
    pub allow_virtual_output: bool,
}

/// Result of opening a sink — the actual hardware parameters may differ from requested.
#[derive(Debug, Clone)]
pub struct SinkOpenResult {
    /// Actual channel count (may be less than requested if hardware doesn't support it).
    pub channels: usize,
    /// Buffer capacity in samples (interleaved).
    pub buffer_capacity: usize,
}

/// Output sink type selector.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SinkType {
    /// Local hardware output via cpal (default).
    #[default]
    Cpal,
}
