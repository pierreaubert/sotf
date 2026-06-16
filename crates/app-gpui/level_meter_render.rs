//! Pure helpers for rendering GPUI level meters.
//!
//! Kept outside the visual component tree so the data-summarization logic
//! can be unit-tested even though `components` is excluded from `cfg(test)`
//! builds.

use gpui_audio_kit::db_to_position;
use sotf_audio_player::{ChannelInfo, LoudnessData};
use std::sync::OnceLock;

/// Pre-computed per-channel meter data. Extracting this while the state
/// read lock is held lets `render_meters_panel` avoid cloning the entire
/// `groups` and `peak_hold` vectors every frame.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelMeterData {
    pub fill_ratio: f32,
    pub yellow_threshold: f32,
    pub red_threshold: f32,
    pub peak_hold_ratio: Option<f32>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupMeterData {
    pub group_idx: usize,
    pub muted: bool,
    pub soloed: bool,
    pub dimmed: bool,
    pub channels: Vec<ChannelMeterData>,
}

/// Compute per-channel meter fill ratios and peak-hold markers from the
/// loudness data and peak-hold vector.
pub fn build_channel_meter_data(
    channels: &[ChannelInfo],
    loudness: Option<&LoudnessData>,
    peak_hold: &[f64],
) -> Vec<ChannelMeterData> {
    channels
        .iter()
        .map(|channel| {
            let peak = loudness
                .and_then(|l| l.channel_peaks.get(channel.index))
                .copied()
                .unwrap_or(0.0);

            let peak_db = if peak > 0.0001 {
                20.0 * peak.log10()
            } else {
                -60.0
            };

            let peak_hold_value = peak_hold.get(channel.index).copied().unwrap_or(0.0);
            let peak_hold_db = if peak_hold_value > 0.0001 {
                20.0 * peak_hold_value.log10()
            } else {
                -60.0
            };
            let peak_hold_ratio = if peak_hold_value > 0.0001 {
                Some(db_to_position(peak_hold_db))
            } else {
                None
            };

            ChannelMeterData {
                fill_ratio: db_to_position(peak_db),
                yellow_threshold: db_to_position(-6.0),
                red_threshold: db_to_position(-1.0),
                peak_hold_ratio,
                name: channel.name.clone(),
            }
        })
        .collect()
}

/// Static labels for the vertical dB legend ticks.
///
/// Returns `None` for values outside the standard legend set so callers can
/// choose a fallback while the common path stays allocation-free.
pub fn db_tick_label(db: i32) -> Option<&'static str> {
    Some(match db {
        0 => "0",
        -6 => "-6",
        -12 => "-12",
        -18 => "-18",
        -24 => "-24",
        -30 => "-30",
        -40 => "-40",
        -50 => "-50",
        -60 => "-60",
        _ => return None,
    })
}

static WIDTH_PERCENT_LABELS: OnceLock<Vec<&'static str>> = OnceLock::new();

fn width_percent_labels() -> &'static [&'static str] {
    WIDTH_PERCENT_LABELS.get_or_init(|| {
        (0..=100)
            .map(|i| {
                let label = format!("{}%", i);
                // Leak a bounded set of 101 small strings once; this removes
                // per-frame heap allocation from the width-bar label.
                Box::leak(label.into_boxed_str()) as &'static str
            })
            .collect()
    })
}

/// Format a stereo-width ratio (0.0 = mono, 1.0 = wide) as a pre-cached,
/// heap-allocation-free percentage label.
pub fn format_width_percent(width: f64) -> &'static str {
    let pct = (width * 100.0).round().clamp(0.0, 100.0) as usize;
    width_percent_labels()[pct]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_channel(index: usize, name: &str) -> ChannelInfo {
        ChannelInfo {
            index,
            name: name.to_string(),
            display_name: vec![name.to_string()],
        }
    }

    fn make_loudness(peaks: Vec<f64>) -> LoudnessData {
        let len = peaks.len();
        LoudnessData {
            channel_peaks: Arc::new(peaks),
            ..LoudnessData::new(len)
        }
    }

    #[test]
    fn build_channel_meter_data_maps_peaks_and_peak_hold() {
        let channels = vec![make_channel(0, "L"), make_channel(1, "R")];
        let loudness = make_loudness(vec![1.0, 0.01]);
        let peak_hold = vec![0.5, 0.0];

        let data = build_channel_meter_data(&channels, Some(&loudness), &peak_hold);

        assert_eq!(data.len(), 2);
        assert_eq!(data[0].name, "L");
        assert!(
            data[0].fill_ratio > data[1].fill_ratio,
            "L should be higher than R"
        );
        assert!(
            data[0].peak_hold_ratio.is_some(),
            "L peak hold should be visible"
        );
        assert!(
            data[1].peak_hold_ratio.is_none(),
            "R peak hold should be hidden"
        );
        assert_eq!(data[0].yellow_threshold, data[1].yellow_threshold);
    }

    #[test]
    fn build_channel_meter_data_uses_zero_for_missing_loudness() {
        let channels = vec![make_channel(0, "L")];
        let data = build_channel_meter_data(&channels, None, &[]);
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].fill_ratio, db_to_position(-60.0));
        assert!(data[0].peak_hold_ratio.is_none());
    }

    #[test]
    fn db_tick_label_returns_static_strings_for_legend_ticks() {
        assert_eq!(db_tick_label(0), Some("0"));
        assert_eq!(db_tick_label(-6), Some("-6"));
        assert_eq!(db_tick_label(-60), Some("-60"));
        assert_eq!(db_tick_label(3), None);
    }

    #[test]
    fn format_width_percent_uses_cached_labels_and_clamps() {
        assert_eq!(format_width_percent(0.0), "0%");
        assert_eq!(format_width_percent(0.5), "50%");
        assert_eq!(format_width_percent(1.0), "100%");
        assert_eq!(format_width_percent(0.123), "12%");
        assert_eq!(format_width_percent(-0.5), "0%");
        assert_eq!(format_width_percent(1.5), "100%");
    }
}
