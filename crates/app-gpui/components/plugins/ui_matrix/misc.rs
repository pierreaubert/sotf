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

#[doc(hidden)]
pub fn checked_matrix_cell_index(
    input_idx: usize,
    output_idx: usize,
    input_channels: usize,
    output_channels: usize,
    matrix_len: usize,
) -> Option<usize> {
    if input_idx >= input_channels || output_idx >= output_channels {
        return None;
    }
    let index = cell_index(input_idx, output_idx, input_channels);
    (index < matrix_len).then_some(index)
}

#[doc(hidden)]
pub fn matrix_settings_mut_by_instance_id(
    graph: &mut sotf_audio_player::PluginGraph,
    plugin_instance_id: usize,
) -> Option<&mut sotf_audio_player::PluginSettings> {
    graph
        .nodes
        .values_mut()
        .find(|node| node.plugin.id == plugin_instance_id)
        .map(|node| &mut node.plugin.settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_coordinates_cannot_alias_after_channel_count_shrinks() {
        assert_eq!(checked_matrix_cell_index(2, 0, 2, 2, 4), None);
        assert_eq!(checked_matrix_cell_index(0, 2, 2, 2, 4), None);
        assert_eq!(checked_matrix_cell_index(1, 1, 2, 2, 4), Some(3));
    }
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
