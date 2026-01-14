use crate::app::types::QueueItem;
use parking_lot::RwLock;
use sotf_audio::devices::AudioDevice;
use std::sync::Arc;

/// Shared state accessible by multiple managers
///
/// This struct holds state that needs to be effectively "global" or
/// shared across manager boundaries, but in a thread-safe way.
#[derive(Debug, Clone, Default)]
pub struct SharedState {
    /// Currently playing track info
    pub current_track: Arc<RwLock<Option<QueueItem>>>,

    /// Available output devices
    pub output_devices: Arc<RwLock<Vec<AudioDevice>>>,

    /// Available input devices
    pub input_devices: Arc<RwLock<Vec<AudioDevice>>>,
}

impl SharedState {
    pub fn new() -> Self {
        Self::default()
    }
}
