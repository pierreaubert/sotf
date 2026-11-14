use sotf_audio_player::format_channel_count;

/// Format sample rate and bit depth for display (e.g., "24/44.1k", "16/48k")
#[must_use]
pub fn format_sample_info(bit_depth: Option<u32>, sample_rate: Option<u32>) -> Option<String> {
    match (bit_depth, sample_rate) {
        (Some(bits), Some(rate)) => {
            let rate_str = if rate % 1000 == 0 {
                format!("{}k", rate / 1000)
            } else {
                format!("{:.1}k", f64::from(rate) / 1000.0)
            };
            Some(format!("{bits}/{rate_str}"))
        }
        (Some(bits), None) => Some(format!("{bits}bit")),
        (None, Some(rate)) => {
            let rate_str = if rate % 1000 == 0 {
                format!("{}k", rate / 1000)
            } else {
                format!("{:.1}k", f64::from(rate) / 1000.0)
            };
            Some(rate_str)
        }
        (None, None) => None,
    }
}

/// Format non-stereo channel counts for compact album metadata display.
#[must_use]
pub fn format_channel_info(channels: Option<u32>) -> Option<String> {
    channels.and_then(|count| (count != 2).then(|| format_channel_count(count)))
}

/// Format the dynamic range for display
#[must_use]
pub fn format_dr(dr: Option<f64>) -> Option<String> {
    dr.map(|d| format!("{d:.0}"))
}
