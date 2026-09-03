//! User-facing error output helpers.
//!
//! CLI errors may originate from network or server operations that include URLs
//! containing authentication tokens, API keys, or other secrets. The helpers
//! here ensure that such values are redacted before they reach stderr or log
//! files.

use std::sync::atomic::{AtomicBool, Ordering};

/// Debug override for URL redaction, set once from `--show-urls` at startup.
///
/// Off by default so embedded secrets stay redacted; when enabled, full URLs
/// (including any secrets) pass through to ease stream-failure diagnosis.
static SHOW_URLS: AtomicBool = AtomicBool::new(false);

/// Set the `--show-urls` debug override. Call once from `main` after parsing
/// CLI arguments; both binaries share this module.
pub fn set_show_urls(show: bool) {
    SHOW_URLS.store(show, Ordering::Relaxed);
}

/// Query parameter keys treated as secret-bearing (case-insensitive).
const SECRET_KEYS: &[&str] = &[
    "token", "api_key", "apikey", "secret", "password", "passwd", "auth", "bearer",
];

/// Redact secret-bearing values from a string intended for user-facing output.
///
/// Only URLs that actually carry secrets are replaced with `[URL REDACTED]`:
/// those with userinfo (`scheme://user:pass@host...`) or with a secret query
/// parameter (`token=...`, `api_key=...`, etc.). Plain stream URLs pass
/// through unchanged so failures stay debuggable. Standalone secret pairs
/// (`token=...` outside a URL) become `[REDACTED]`. This prevents
/// authentication tokens, API keys, and other secrets from leaking in
/// network/server error messages.
pub fn redact_secrets(input: &str) -> String {
    redact_with(input, SHOW_URLS.load(Ordering::Relaxed))
}

fn redact_with(input: &str, show_urls: bool) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut output = String::with_capacity(input.len());
    let mut i = 0;

    while i < chars.len() {
        if let Some(url_len) = detect_url_at(&chars[i..]) {
            let url: String = chars[i..i + url_len].iter().collect();
            if !show_urls && url_has_secret(&url) {
                output.push_str("[URL REDACTED]");
            } else {
                output.push_str(&url);
            }
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

/// Report whether a URL string carries secret material: userinfo in the
/// authority (`scheme://user:pass@host...`) or a secret query parameter.
fn url_has_secret(url: &str) -> bool {
    let Some(scheme_end) = url.find("://") else {
        return false;
    };
    let rest = &url[scheme_end + 3..];
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    if rest[..authority_end].contains('@') {
        return true;
    }
    let Some(query_start) = rest.find('?') else {
        return false;
    };
    let query = &rest[query_start + 1..];
    let query = query.split('#').next().unwrap_or(query);
    query.split('&').any(|pair| {
        let key = pair.split('=').next().unwrap_or(pair).to_lowercase();
        SECRET_KEYS.contains(&key.as_str())
    })
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
    if !SECRET_KEYS.contains(&key.as_str()) {
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
    use super::{redact_secrets, redact_with};

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
    fn redact_secrets_preserves_plain_urls_without_secrets() {
        let input = "Try http://a.com/stream?x=1 or https://b.com/play?y=2";
        assert_eq!(redact_secrets(input), input);
    }

    #[test]
    fn redact_secrets_redacts_url_with_userinfo() {
        let input = "Failed: https://user:s3cret@example.com/stream";
        let expected = "Failed: [URL REDACTED]";
        assert_eq!(redact_secrets(input), expected);
    }

    #[test]
    fn redact_secrets_shows_secret_urls_when_show_urls_enabled() {
        let input = "Failed: https://example.com/play?token=SECRET123";
        assert_eq!(
            redact_with(input, true),
            input,
            "show_urls bypass must preserve the full URL"
        );
        assert_eq!(
            redact_with(input, false),
            "Failed: [URL REDACTED]",
            "default path must still redact secret-bearing URLs"
        );
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
