pub(super) fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// MPD's legacy time fields carry saturated nonnegative whole seconds.
pub(super) fn whole_seconds(seconds: f64) -> u64 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "MPD integer time fields intentionally discard fractional seconds and saturate negative values"
    )]
    let whole = seconds as u64;
    whole
}
