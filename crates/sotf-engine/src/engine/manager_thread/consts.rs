pub(super) const SPIN_MS_SLEEP_MANAGER: u64 = 10;

pub(super) const SPIN_MS_CHECK_MANAGER: u64 = 50;

/// Bound per-loop event work while still draining faster than audio threads can
/// publish position/stats updates. Processing only one event before the 50 ms
/// command wait lets the unbounded event queue grow during normal playback.
pub(super) const MAX_THREAD_EVENTS_PER_TICK: usize = 256;

pub(super) const PLUGIN_INIT_TIMEOUT_MS: u64 = 10000; // 10 seconds for plugin initialization (SOFA loading can be slow)

pub(super) const MAX_CONFIG_QUEUE_SIZE: usize = 5; // Maximum pending config updates

// The processing worker can legitimately spend one full expensive plugin
// block before observing a control command. Keep this aligned with the decoder
// command budget and rely on typed responses rather than accepting late ACKs.
pub(super) const PROCESSING_COMMAND_TIMEOUT_MS: u64 = 1000;

pub(super) const DECODER_COMMAND_TIMEOUT_MS: u64 = 1000;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(super) const EXTERNAL_PLUGIN_MAINTENANCE_INTERVAL_MS: u64 = 1000;
