/// Parse the leading four ASCII digits of an ISO-8601 release date
/// (`YYYY-MM-DD`, but `YYYY` and `YYYY-MM` also occur in Spotify data) into a
/// year. Returns `None` for malformed input instead of panicking.
pub(crate) fn parse_release_year(date: &str) -> Option<u32> {
    let prefix = date.get(..4)?;
    if !prefix.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    prefix.parse().ok()
}

/// Truncate a string for safe inclusion in log/error messages, replacing the
/// trailing portion with `…` when it would otherwise exceed `max` bytes.
pub(crate) fn truncate_for_log(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}
