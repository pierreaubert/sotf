use std::time::Duration;

pub(super) const USER_AGENT: &str = "SOTF/1.0";

pub(super) const DEFAULT_TARGET_DURATION: Duration = Duration::from_secs(4);

pub(super) const MAX_PLAYLIST_BYTES: usize = 2 * 1024 * 1024;

pub(super) const MAX_SEGMENT_BYTES: usize = 64 * 1024 * 1024;

/// Maximum number of HLS segments allowed in a single media playlist.
///
/// This caps memory allocation when parsing a malicious or degenerate playlist
/// that declares an unbounded number of segments within the byte limit.
pub(super) const MAX_SEGMENTS: usize = 10_000;
