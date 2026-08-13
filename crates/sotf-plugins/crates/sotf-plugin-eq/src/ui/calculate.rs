use super::consts::CHART_LEFT_MARGIN;
use super::consts::CHART_RIGHT_MARGIN;
use super::types::EqBandView;

fn band_response(filter: &EqBandView, freq: f64) -> f64 {
    crate::misc::create_band_stages(
        filter.filter_type,
        filter.frequency,
        filter.sample_rate,
        filter.q,
        filter.gain_db,
        filter.order,
    )
    .iter()
    .map(|stage| stage.log_result(freq))
    .sum()
}

/// Calculate the combined response in dB at a given frequency
pub fn calculate_response_at_freq(filters: &[EqBandView], freq: f64) -> f64 {
    if filters.is_empty() {
        return 0.0;
    }
    let any_soloed = filters.iter().any(|f| f.solo);
    filters
        .iter()
        .filter(|f| {
            if f.muted {
                return false;
            }
            if any_soloed && !f.solo {
                return false;
            }
            true
        })
        .map(|f| {
            band_response(f, freq)
        })
        .sum()
}

/// Calculate single band response at a frequency
pub fn calculate_band_response(filter: &EqBandView, freq: f64) -> f64 {
    if filter.muted {
        return 0.0;
    }
    band_response(filter, freq)
}

/// Calculate dynamic y-axis range based on filter gains.
/// Returns (min_db, max_db) for the chart y-axis.
///
/// Logic:
/// - If max absolute gain <= 0.5 dB, use -1.0 to +1.0 dB
/// - Otherwise, multiply by 1.2 and round to next integer (separately for upper/lower)
pub(super) fn calculate_dynamic_y_range(filters: &[EqBandView]) -> (f64, f64) {
    if filters.is_empty() {
        return (-1.0, 1.0);
    }

    // Find min and max gain across all filters
    let mut min_gain = 0.0_f64;
    let mut max_gain = 0.0_f64;

    for filter in filters {
        if filter.gain_db < min_gain {
            min_gain = filter.gain_db;
        }
        if filter.gain_db > max_gain {
            max_gain = filter.gain_db;
        }
    }

    // Calculate upper bound
    let upper_bound = if max_gain <= 0.5 {
        1.0
    } else {
        (max_gain * 1.2).ceil()
    };

    // Calculate lower bound
    let lower_bound = if min_gain.abs() <= 0.5 {
        -1.0
    } else {
        (min_gain * 1.2).floor()
    };

    (lower_bound, upper_bound)
}

/// Calculate the actual plot width based on chart width and legend configuration.
/// This must match gpui-px line chart legend calculation exactly.
///
/// # Arguments
/// * `chart_width` - Total width of the chart container
/// * `labels` - Iterator of label strings to calculate legend width from
pub fn calculate_plot_width<'a>(chart_width: f32, labels: impl Iterator<Item = &'a str>) -> f32 {
    // gpui-px constants
    const LEGEND_GAP: f32 = 20.0;
    const COLOR_INDICATOR_WIDTH: f32 = 16.0;
    const GAP_AFTER_COLOR: f32 = 8.0;
    const PADDING: f32 = 16.0; // 8px on each side
    const CHAR_WIDTH: f32 = 7.0; // Approximate width per character for text_xs

    // Find max label length
    let max_label_len = labels.map(|l| l.len()).max().unwrap_or(0);

    // Calculate legend width (vertical legend for Right position)
    let estimated_text_width = (max_label_len as f32) * CHAR_WIDTH;
    let legend_width = COLOR_INDICATOR_WIDTH + GAP_AFTER_COLOR + estimated_text_width + PADDING;
    let width_for_legend = legend_width + LEGEND_GAP;

    // Final plot width
    (chart_width - CHART_LEFT_MARGIN - CHART_RIGHT_MARGIN - width_for_legend).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use math_audio_iir_fir::BiquadFilterType;

    fn band(order: usize, sample_rate: f64) -> EqBandView {
        EqBandView {
            filter_type: BiquadFilterType::Notch,
            frequency: 2_000.0,
            q: 4.0,
            gain_db: 0.0,
            order,
            sample_rate,
            muted: false,
            solo: false,
        }
    }

    #[test]
    fn preview_uses_actual_order_and_sample_rate() {
        let second_order = calculate_band_response(&band(2, 48_000.0), 1_500.0);
        let eighth_order = calculate_band_response(&band(8, 48_000.0), 1_500.0);
        assert!((second_order - eighth_order).abs() > 0.05);

        let low_rate = calculate_band_response(&band(8, 44_100.0), 18_000.0);
        let high_rate = calculate_band_response(&band(8, 96_000.0), 18_000.0);
        assert!((low_rate - high_rate).abs() > 0.01);
    }
}
