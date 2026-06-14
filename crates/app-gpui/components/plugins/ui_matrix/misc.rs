/// Convert linear gain to dB string for display
/// Supports negative gains (for M/S encoding) by showing with minus sign prefix
pub(super) fn format_gain_db(linear: f32) -> String {
    const SILENCE_THRESHOLD: f32 = 0.001; // -60 dB

    if linear.abs() < SILENCE_THRESHOLD {
        "-\u{221e}".to_string() // -infinity symbol
    } else {
        let sign = if linear < 0.0 { "-" } else { "" };
        let db = 20.0 * linear.abs().log10();
        if db.abs() < 0.05 {
            format!("{}0", sign)
        } else {
            format!("{}{:.1}", sign, db)
        }
    }
}

/// Get cell index in matrix from input/output indices
pub(super) fn cell_index(input_idx: usize, output_idx: usize, input_count: usize) -> usize {
    output_idx * input_count + input_idx
}

/// Compute output channel groups from MeterGroupSpec
pub(super) fn compute_output_groups(
    output_channels: usize,
    speaker_config: Option<&str>,
) -> Vec<(String, Vec<usize>)> {
    let groups = speaker_config
        .and_then(sotf_plugins::get_meter_groups)
        .or_else(|| sotf_plugins::get_meter_groups_by_channels(output_channels));
    if let Some(groups) = groups {
        groups
            .iter()
            .map(|g| {
                (
                    g.name.to_string(),
                    g.channels.iter().map(|c| c.index).collect(),
                )
            })
            .collect()
    } else {
        (0..output_channels)
            .map(|i| (format!("Ch{}", i), vec![i]))
            .collect()
    }
}
