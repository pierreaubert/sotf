/// Helper function to convert cpal sample format to string
pub(super) fn format_to_string(format: cpal::SampleFormat) -> String {
    match format {
        cpal::SampleFormat::F32 => "f32".to_string(),
        cpal::SampleFormat::I16 => "i16".to_string(),
        cpal::SampleFormat::U16 => "u16".to_string(),
        _ => "unknown".to_string(),
    }
}

/// ASIO device prefix. Device identifiers starting with "ASIO:" will use the
/// ASIO host instead of the default (WASAPI) host on Windows.
///
/// Example: "ASIO:Focusrite USB ASIO" selects the Focusrite ASIO driver.
pub const ASIO_DEVICE_PREFIX: &str = "ASIO:";

pub(super) fn probe_channel_order(requested_channels: usize, default_channels: u16) -> Vec<u16> {
    let requested = u16::try_from(requested_channels).ok().filter(|&ch| ch > 0);
    let mut order = Vec::new();

    if let Some(ch) = requested {
        order.push(ch);
    }
    if order.first().copied() != Some(default_channels) {
        order.push(default_channels);
    }

    order
}

pub(super) fn build_sample_rate_candidates(
    requested_rate: u32,
    device_default: Option<u32>,
) -> Vec<u32> {
    let mut candidates = vec![requested_rate];
    for rate in [48_000, 44_100, 96_000, 192_000] {
        if !candidates.contains(&rate) {
            candidates.push(rate);
        }
    }
    if let Some(rate) = device_default
        && !candidates.contains(&rate)
    {
        candidates.push(rate);
    }
    candidates
}

/// Helper to match a device from a list of (id, name) tuples based on identifier
///
/// Priority:
/// 1. Exact ID match
/// 2. Exact Name match (case-insensitive)
/// 3. Starts With match (case-insensitive)
/// 4. Contains match (case-insensitive)
pub(super) fn match_device_priority(
    devices: &[(String, String)],
    identifier: &str,
) -> Option<usize> {
    let target = identifier.to_lowercase();

    // 1. Exact ID match
    if let Some(idx) = devices.iter().position(|(id, _)| id == identifier) {
        log::debug!("[find_device] Found device by ID match: {}", devices[idx].0);
        return Some(idx);
    }

    // 2. Exact Name match (case-insensitive)
    if let Some(idx) = devices
        .iter()
        .position(|(_, name)| name.to_lowercase() == target)
    {
        log::debug!(
            "[find_device] Found device by Exact Name match: {}",
            devices[idx].1
        );
        return Some(idx);
    }

    // 3. Starts With match (case-insensitive)
    if let Some(idx) = devices
        .iter()
        .position(|(_, name)| name.to_lowercase().starts_with(&target))
    {
        log::debug!(
            "[find_device] Found device by Starts With match: {}",
            devices[idx].1
        );
        return Some(idx);
    }

    // 4. Contains match (case-insensitive)
    if let Some(idx) = devices
        .iter()
        .position(|(_, name)| name.to_lowercase().contains(&target))
    {
        log::debug!(
            "[find_device] Found device by Contains match: {}",
            devices[idx].1
        );
        return Some(idx);
    }

    None
}

pub(super) fn summarize_available_device_names(
    device_info: &[(String, String)],
    limit: usize,
) -> String {
    let mut names: Vec<String> = device_info
        .iter()
        .filter_map(|(_, name)| {
            let trimmed = name.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
        .collect();
    names.sort();
    names.dedup();

    let shown: Vec<&str> = names.iter().take(limit).map(String::as_str).collect();
    if names.len() > limit {
        format!(
            "{} ... and {} more",
            shown.join(", "),
            names.len().saturating_sub(limit)
        )
    } else {
        shown.join(", ")
    }
}
