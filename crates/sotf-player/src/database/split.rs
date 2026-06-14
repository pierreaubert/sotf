use super::misc::normalize_genre_name;

/// Split a metadata value by common delimiters (`,`, `/`, `;`)
/// Returns a vector of trimmed, non-empty values (preserves original capitalization)
pub(super) fn split_metadata_value(value: &str) -> Vec<String> {
    value
        .split([',', '/', ';'])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Split a genre value by common delimiters and normalize each
/// (dots/underscores to spaces, title case)
pub(super) fn split_and_normalize_genres(value: &str) -> Vec<String> {
    value
        .split([',', '/', ';'])
        .map(|s| normalize_genre_name(s.trim()))
        .filter(|s| !s.is_empty())
        .collect()
}
