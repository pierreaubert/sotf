/// Maximum number of reconnection attempts on network error.
pub(super) const MAX_RECONNECT_ATTEMPTS: u32 = 5;

/// Initial backoff delay for reconnection (doubles each attempt).
pub(super) const INITIAL_BACKOFF_MS: u64 = 200;

/// Read-ahead buffer size in bytes (128KB).
pub(super) const READ_AHEAD_SIZE: usize = 128 * 1024;
