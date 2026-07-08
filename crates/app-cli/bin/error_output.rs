//! User-facing error output helpers.
//!
//! CLI errors may originate from network or server operations that include URLs
//! containing authentication tokens, API keys, or other secrets. The helpers
//! here ensure that such values are redacted before they reach stderr or log
//! files.

/// Redact secret-bearing values from a string intended for user-facing output.
///
/// This replaces any token that looks like a URI/URL (`scheme://...`) with
/// `[URL REDACTED]` and any common secret query parameter
/// (`token=...`, `api_key=...`, etc.) with `[REDACTED]`. This prevents
/// authentication tokens, API keys, and other secrets from leaking in
/// network/server error messages.
pub fn redact_secrets(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut output = String::with_capacity(input.len());
    let mut i = 0;

    while i < chars.len() {
        if let Some(url_len) = detect_url_at(&chars[i..]) {
            output.push_str("[URL REDACTED]");
            i += url_len;
        } else if let Some(secret_len) = detect_secret_at(&chars[i..]) {
            output.push_str("[REDACTED]");
            i += secret_len;
        } else {
            output.push(chars[i]);
            i += 1;
        }
    }

    output
}

/// Detect a secret-bearing query parameter at the start of a character slice.
///
/// Matches case-insensitive keys such as `token`, `api_key`, `apikey`,
/// `secret`, `password`, `passwd`, `auth`, or `bearer` followed by `=` and a
/// value that runs until `&`, whitespace, or end of slice. Returns the total
/// length of the matched key/value pair.
fn detect_secret_at(slice: &[char]) -> Option<usize> {
    if slice.len() < 3 {
        return None;
    }

    let key_chars: Vec<char> = slice
        .iter()
        .take_while(|&&c| c != '=' && !c.is_whitespace() && c != '&')
        .copied()
        .collect();

    if key_chars.is_empty() || key_chars.len() >= slice.len() || slice[key_chars.len()] != '=' {
        return None;
    }

    let key: String = key_chars.iter().collect::<String>().to_lowercase();
    let secret_keys = [
        "token", "api_key", "apikey", "secret", "password", "passwd", "auth", "bearer",
    ];
    if !secret_keys.contains(&key.as_str()) {
        return None;
    }

    let mut value_len = 1; // the '=' itself
    while key_chars.len() + value_len < slice.len()
        && slice[key_chars.len() + value_len] != '&'
        && !slice[key_chars.len() + value_len].is_whitespace()
    {
        value_len += 1;
    }

    Some(key_chars.len() + value_len)
}

/// Detect a URL/URI at the start of a character slice.
///
/// A URL is recognized as an alphanumeric scheme (optionally containing `+`,
/// `-`, or `.`) followed by `://`. The URL runs until the next whitespace
/// character or the end of the slice. Returns the length of the URL in
/// characters, or `None` if no URL starts at this position.
fn detect_url_at(slice: &[char]) -> Option<usize> {
    // Minimum possible URL: "a://" (4 chars).
    if slice.len() < 4 {
        return None;
    }

    // Scheme must start with a letter.
    if !slice[0].is_ascii_alphabetic() {
        return None;
    }

    let mut scheme_len = 1;
    while scheme_len < slice.len()
        && (slice[scheme_len].is_ascii_alphanumeric()
            || slice[scheme_len] == '+'
            || slice[scheme_len] == '-'
            || slice[scheme_len] == '.')
    {
        scheme_len += 1;
    }

    // Require the scheme to be followed by "://".
    if scheme_len + 3 > slice.len()
        || slice[scheme_len] != ':'
        || slice[scheme_len + 1] != '/'
        || slice[scheme_len + 2] != '/'
    {
        return None;
    }

    // Consume until whitespace.
    let mut url_len = scheme_len + 3;
    while url_len < slice.len() && !slice[url_len].is_whitespace() {
        url_len += 1;
    }

    Some(url_len)
}

#[cfg(test)]
mod tests {
    use super::redact_secrets;

    #[test]
    fn redact_secrets_leaves_plain_text_unchanged() {
        let text = "Failed to load audio: file not found";
        assert_eq!(redact_secrets(text), text);
    }

    #[test]
    fn redact_secrets_redacts_http_url_with_token() {
        let input = "Network error: http://example.com/stream.mp3?token=SECRET123";
        let expected = "Network error: [URL REDACTED]";
        assert_eq!(redact_secrets(input), expected);
    }

    #[test]
    fn redact_secrets_redacts_https_url_with_api_key() {
        let input = "Failed: https://api.example.com/v1/play?api_key=AKIAIOSFODNN7EXAMPLE";
        let expected = "Failed: [URL REDACTED]";
        assert_eq!(redact_secrets(input), expected);
    }

    #[test]
    fn redact_secrets_redacts_multiple_urls() {
        let input = "Try http://a.com?x=1 or https://b.com?y=2";
        let expected = "Try [URL REDACTED] or [URL REDACTED]";
        assert_eq!(redact_secrets(input), expected);
    }

    #[test]
    fn redact_secrets_redacts_mpd_stream_url() {
        let input = "Stream failed: mpd-stream://localhost:6600?auth=TOKEN";
        let expected = "Stream failed: [URL REDACTED]";
        assert_eq!(redact_secrets(input), expected);
    }

    #[test]
    fn redact_secrets_preserves_file_paths() {
        let input = "File not found: /Users/alice/music/secret_song.wav";
        assert_eq!(redact_secrets(input), input);
    }

    #[test]
    fn redact_secrets_preserves_windows_file_paths() {
        let input = "File not found: C:\\Users\\alice\\music\\song.wav";
        assert_eq!(redact_secrets(input), input);
    }

    #[test]
    fn redact_secrets_url_at_end_of_line_has_no_trailing_whitespace() {
        let input = "error: https://example.com/?token=abc";
        let expected = "error: [URL REDACTED]";
        assert_eq!(redact_secrets(input), expected);
    }

    #[test]
    fn redact_secrets_redacts_loose_token_pair() {
        let input = "Unsupported file extension: mp3?token=SECRET123";
        let expected = "Unsupported file extension: mp3?[REDACTED]";
        assert_eq!(redact_secrets(input), expected);
    }

    #[test]
    fn redact_secrets_redacts_api_key_pair() {
        let input = "Network error: api_key=AKIAIOSFODNN7EXAMPLE&foo=bar";
        let expected = "Network error: [REDACTED]&foo=bar";
        assert_eq!(redact_secrets(input), expected);
    }

    #[test]
    fn redact_secrets_redacts_bearer_and_password_case_insensitive() {
        let input = "auth=foo Bearer=bar PASSWORD=baz";
        let expected = "[REDACTED] [REDACTED] [REDACTED]";
        assert_eq!(redact_secrets(input), expected);
    }

    #[test]
    fn redact_secrets_leaves_innocuous_key_unchanged() {
        let input = "license key=abc123";
        assert_eq!(redact_secrets(input), input);
    }
}
