pub(super) const MAX_DELAY_MS: f32 = 5000.0;

/// Parse a per-channel delay parameter id of the form `delay_ms_{N}`.
/// Returns the channel index, or None if the id does not match.
pub(super) fn parse_channel_delay_id(id: &str) -> Option<usize> {
    id.strip_prefix("delay_ms_")
        .and_then(|tail| tail.parse::<usize>().ok())
}
