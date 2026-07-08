pub(super) const API_MAX_REQUEST_BYTES: usize = 64 * 1024;

pub(super) const API_MAX_BODY_BYTES: usize = 32 * 1024;

pub(super) const API_LIBRARY_DEFAULT_LIMIT: usize = 50;

pub(super) const API_LIBRARY_MAX_LIMIT: usize = 250;

pub(super) const API_REQUEST_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

pub(super) const API_MAX_CONCURRENT_CONNECTIONS: usize = 128;

/// Maximum lifetime of a pairing window once enabled. A client must complete the
/// pairing ceremony within this duration; afterwards the nonce is treated as
/// expired and pairing must be re-enabled.
pub(super) const PAIRING_MAX_LIFETIME: std::time::Duration = std::time::Duration::from_secs(600);
