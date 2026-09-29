// intentional-file: fixed pixel values here are graph and plugin control geometry.
use super::super::MeterTheme;
use super::level_meter_manager::LevelMeterManager;
use super::misc::peak_spread_db;
use super::misc::should_use_peak_spread;
use crate::app::ChannelGroup;
#[cfg(feature = "dev-api")]
use crate::app::dev_api::{DevElementState, DevTrackExt};
use crate::app::i18n::LevelMeterTranslations;
use crate::components::design::Ds;
use crate::components::themed_tooltip;
use crate::level_meter_render::{
    ChannelMeterData, GroupMeterData, build_channel_meter_data, db_tick_label,
    format_width_percent, sample_peak_db,
};
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_audio_kit::{
    TickConfig, db_to_position, render_horizontal_meter_bar, render_horizontal_meter_bar_with,
    render_tick_row,
};
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant};

const FALLBACK_TRUE_PEAKS: [f64; 2] = [-60.0, -60.0];

fn render_no_meter_data(d: &Ds, text: LevelMeterTranslations, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .size_full()
        .min_w_0()
        .items_center()
        .justify_center()
        .gap(d.grid)
        .px(d.pad_y)
        .overflow_hidden()
        .child(
            div()
                .w_full()
                .text_align(TextAlign::Center)
                .text_size(d.text_sm)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_primary)
                .child(text.no_data),
        )
        .child(
            div()
                .w_full()
                .text_align(TextAlign::Center)
                .text_size(d.text_xs)
                .text_color(theme.text_muted)
                .child(text.no_data_hint),
        )
}

/// Keep unavailable analyzer values on the silent end of the meter scale.
///
/// Loudness snapshots can legitimately contain `NaN`/infinite values while
/// their gating window is warming up. Passing those values to a fractional
/// layout calculation can produce an invalid fill width, so sanitize them at
/// the UI boundary.
fn finite_meter_value(value: f64, fallback: f64) -> f64 {
    if value.is_finite() { value } else { fallback }
}

fn maximum_true_peak_value(
    loudness: &sotf_audio_player::LoudnessData,
    text: LevelMeterTranslations,
) -> String {
    match loudness.maximum_true_peak_dbtp {
        Some(value) if value.is_finite() => format!("{value:.1} dBTP"),
        Some(_) => text.true_peak_unavailable.to_string(),
        None if loudness.true_peak_is_compliant => "— dBTP".to_string(),
        None => text.true_peak_unavailable.to_string(),
    }
}

fn maximum_lufs_value(maximum: Option<f64>) -> String {
    match maximum {
        Some(value) if value.is_finite() => format!("{value:.1}"),
        _ => "—".to_string(),
    }
}

fn lra_display_value(
    loudness: Option<&sotf_audio_player::LoudnessData>,
    text: LevelMeterTranslations,
) -> (String, bool) {
    let Some(range) = loudness.and_then(|data| data.loudness_range) else {
        return (text.true_peak_unavailable.to_string(), false);
    };
    if range.status == sotf_plugins::analyzer::LoudnessRangeStatus::Valid
        && let Some(value) = range
            .range_lu
            .filter(|value| value.is_finite() && *value >= 0.0)
    {
        return (format!("{value:.1} LU"), !range.is_stable);
    }
    (text.true_peak_unavailable.to_string(), false)
}

fn render_lufs_maximum_summary(
    d: &Ds,
    label: &'static str,
    value: String,
    label_selector: &'static str,
    value_selector: &'static str,
    summary_selector: &'static str,
    theme: &Theme,
) -> impl IntoElement {
    #[cfg(not(feature = "dev-api"))]
    let _ = (label_selector, value_selector, summary_selector);
    let label_text = label.to_string();
    let label = div().w_full().min_w_0().child(label_text.clone());
    #[cfg(feature = "dev-api")]
    let label =
        label.dev_track_with_state(label_selector, DevElementState::default().text(label_text));

    let value_text = value;
    let value = div().w_full().min_w_0().child(value_text.clone());
    #[cfg(feature = "dev-api")]
    let value =
        value.dev_track_with_state(value_selector, DevElementState::default().text(value_text));

    let summary = div()
        .flex()
        .flex_col()
        // The label and its value form one compact summary. Keep this gap
        // below the section gap so the extra maximum rows fit in a short
        // Loudness Monitor panel without pushing the bars off-screen.
        .gap(d.half_grid)
        .w_full()
        .min_w_0()
        .text_size(d.text_xs)
        .text_color(theme.text_muted)
        .child(label)
        .child(value);
    #[cfg(feature = "dev-api")]
    let summary = summary.dev_track(summary_selector);
    summary
}

fn render_lra_summary(
    d: &Ds,
    label: &'static str,
    value: String,
    not_stable: bool,
    not_stable_label: &'static str,
    theme: &Theme,
) -> impl IntoElement {
    let label_text = label.to_string();
    let label = div().w_full().min_w_0().child(label_text.clone());
    #[cfg(feature = "dev-api")]
    let label = label.dev_track_with_state(
        "meters.lufs.lra-label",
        DevElementState::default().text(label_text),
    );

    let value_text = value;
    let value = div().w_full().min_w_0().child(value_text.clone());
    #[cfg(feature = "dev-api")]
    let value = value.dev_track_with_state(
        "meters.lufs.lra-value",
        DevElementState::default().text(value_text),
    );

    let mut summary = div()
        .flex()
        .flex_col()
        .gap(d.half_grid)
        .w_full()
        .min_w_0()
        .text_size(d.text_xs)
        .text_color(theme.text_muted)
        .child(label)
        .child(value);

    if not_stable {
        let status_text = not_stable_label;
        let status = div().w_full().min_w_0().child(status_text);
        #[cfg(feature = "dev-api")]
        let status = status.dev_track_with_state(
            "meters.lufs.lra-stability",
            DevElementState::default().text(status_text),
        );
        summary = summary.child(status);
    }

    #[cfg(feature = "dev-api")]
    let summary = summary.dev_track("meters.lufs.lra-summary");
    summary
}

/// Render a horizontal gain reduction meter
/// Uses render_gradient_meter for consistent styling
pub fn render_gr_meter(
    d: &Ds,
    gain_reduction_db: f64, // Should be negative or 0
    max_db: f64,            // e.g., -30.0 (max gain reduction to display)
    theme: &Theme,
) -> impl IntoElement {
    let gr_abs = gain_reduction_db.abs();
    let tick_config = TickConfig::gain_reduction(max_db);

    // Calculate fill ratio using tick config scale
    let fill_ratio = tick_config.value_to_position(gr_abs);

    // Color gradient: green -> yellow -> red based on amount
    let color = if gr_abs < 3.0 {
        theme.feedback.meter_normal // Green
    } else if gr_abs < 10.0 {
        theme.feedback.meter_warning // Yellow/Orange
    } else {
        theme.feedback.meter_clip // Red
    };

    div()
        .flex()
        .flex_col()
        .gap(d.grid)
        .w_full()
        // Header row with label and value
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_size(d.text_xs)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_secondary)
                        .child("GR"),
                )
                .child(
                    div()
                        .min_w(rems(4.375))
                        .px(d.pad_y)
                        .py(d.pad_y_half)
                        .rounded(d.r_md)
                        .bg(theme.surface)
                        .text_size(d.text_xs)
                        .font_weight(FontWeight::BOLD)
                        .text_color(color)
                        .text_align(TextAlign::Right)
                        .child(format!("-{:.1} dB", gr_abs)),
                ),
        )
        // Meter bar (full width)
        .child(
            div()
                .h(rems(0.75))
                .w_full()
                .bg(theme.background)
                .rounded(d.r_md)
                .border_1()
                .border_color(theme.border)
                .overflow_hidden()
                .child(
                    div()
                        .h_full()
                        .w(relative(fill_ratio))
                        .bg(color)
                        .rounded_l_md(),
                ),
        )
        // Tick marks (full width)
        .child(render_tick_row(&tick_config, 0.0, 0.0))
        // Legend (full width) - show as negative dB values (0, -10, -20, -30)
        .child(
            div()
                .flex()
                .justify_between()
                .text_size(d.text_xs)
                .text_color(theme.text_muted)
                .children(tick_config.major_values.iter().map(|v| {
                    let label = if *v == 0.0 {
                        "0".to_string()
                    } else {
                        format!("-{:.0}", v)
                    };
                    div().child(label)
                })),
        )
}

/// Render a meter with gradient coloring (green, yellow at top, red at clip)
/// Optional peak_ratio shows a peak hold indicator line
pub fn render_gradient_meter(
    d: &Ds,
    fill_ratio: f32,
    yellow_threshold: f32,
    red_threshold: f32,
    peak_ratio: Option<f32>,
    channel_name: String,
    theme: &Theme,
) -> impl IntoElement {
    // Calculate segment heights
    let green_height = fill_ratio.min(yellow_threshold);
    let yellow_height = if fill_ratio > yellow_threshold {
        (fill_ratio - yellow_threshold).min(red_threshold - yellow_threshold)
    } else {
        0.0
    };
    let red_height = if fill_ratio > red_threshold {
        fill_ratio - red_threshold
    } else {
        0.0
    };

    let theme_c = theme.clone();
    let peak_color = theme.plugin_palette.meter_colors.peak;
    div()
        .flex()
        .flex_col()
        .items_center()
        .flex_1()
        // Meter bar container
        .child(
            div()
                .w(rems(1.0))
                .flex_1()
                .min_h(rems(11.25))
                .bg(theme_c.background)
                .rounded(d.r_sm)
                .overflow_hidden()
                .relative()
                // Green segment (base)
                .child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                            green_height,
                        )))
                        .bg(theme_c.feedback.meter_normal),
                )
                // Yellow segment (above green)
                .when(yellow_height > 0.001, |el| {
                    el.child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                                yellow_threshold,
                            )))
                            .h(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                                yellow_height,
                            )))
                            .bg(theme_c.feedback.meter_warning),
                    )
                })
                // Red segment (above yellow)
                .when(red_height > 0.001, |el| {
                    el.child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                                red_threshold,
                            )))
                            .h(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                                red_height,
                            )))
                            .bg(theme_c.feedback.meter_clip),
                    )
                })
                // Peak hold indicator (horizontal line)
                .when(peak_ratio.is_some_and(|p| p > 0.001), |el| {
                    let peak_pos = peak_ratio.unwrap_or(0.0);
                    el.child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                                peak_pos,
                            )))
                            // intentional: 2px peak-hold indicator line — visual, not spacing
                            .h(px(2.0))
                            .bg(peak_color),
                    )
                }),
        )
        // Channel name
        .child(
            div()
                .text_size(d.text_xs)
                .text_color(theme.text_muted)
                .mt(d.grid)
                .child(channel_name),
        )
}

/// Render LUFS display with True Peak bars at top (standalone function)
pub fn render_lufs_with_true_peak(
    d: &Ds,
    loudness: Option<&sotf_audio_player::LoudnessData>,
    layout_scale: f32,
    text: LevelMeterTranslations,
    theme: &Theme,
) -> impl IntoElement {
    if loudness.is_none() {
        let empty = div().w_full().child(render_no_meter_data(d, text, theme));
        #[cfg(feature = "dev-api")]
        let empty = empty.dev_track("plugin.loudness.no-data");
        return div().child(empty);
    }
    let maximum_true_peak_value = loudness
        .map(|data| maximum_true_peak_value(data, text))
        .unwrap_or_default();
    let maximum_momentary_value = loudness
        .map(|data| maximum_lufs_value(data.maximum_momentary_lufs))
        .unwrap_or_else(|| "—".to_string());
    let maximum_shortterm_value = loudness
        .map(|data| maximum_lufs_value(data.maximum_shortterm_lufs))
        .unwrap_or_else(|| "—".to_string());
    let (lra_value, lra_not_stable) = lra_display_value(loudness, text);
    // Older snapshots and analyzers that do not expose oversampled peaks can
    // still provide per-channel sample peaks. Keep that fallback channel
    // aware instead of silently substituting a fake stereo L/R pair. The
    // section heading also says what is actually being displayed.
    let (
        integrated_lufs,
        shortterm_lufs,
        momentary_lufs,
        true_peaks,
        peak_label,
        stereo_width,
        peak_spread,
        channel_count,
    ) = if let Some(l) = loudness {
        let (true_peaks, peak_label): (Vec<f64>, &'static str) =
            if l.true_peak_valid && !l.true_peaks_dbtp.is_empty() {
                (
                    l.true_peaks_dbtp
                        .iter()
                        .map(|peak| finite_meter_value(*peak, -60.0))
                        .collect(),
                    text.true_peak,
                )
            } else if !l.channel_peaks.is_empty() {
                (
                    l.channel_peaks
                        .iter()
                        .map(|peak| sample_peak_db(*peak))
                        .collect(),
                    text.peak,
                )
            } else {
                (Vec::new(), text.true_peak)
            };
        let width = l
            .correlation_lr
            .filter(|correlation| correlation.is_finite())
            .map(|correlation| ((1.0 - correlation) / 2.0).clamp(0.0, 1.0));
        let channel_count = true_peaks.len();
        let peak_spread = peak_spread_db(&true_peaks);
        (
            finite_meter_value(l.integrated_lufs, -60.0),
            finite_meter_value(l.shortterm_lufs, -60.0),
            finite_meter_value(l.momentary_lufs, -60.0),
            true_peaks,
            peak_label,
            width,
            peak_spread,
            channel_count,
        )
    } else {
        (
            -60.0,
            -60.0,
            -60.0,
            FALLBACK_TRUE_PEAKS.to_vec(),
            text.true_peak,
            None,
            0.0,
            0,
        )
    };

    let meter_theme = MeterTheme::from_theme(theme, layout_scale);

    div()
        .flex()
        .flex_col()
        // Fill the available width so the bars (each `flex_1` inside
        // render_meter_bar) actually grow when the meters panel is wide.
        // Without this, the outer flex_col sizes to content and the bars
        // collapse to whatever flex_1 of the intrinsic-width parent works
        // out to — leaving a wide empty band on the right of the panel.
        .w_full()
        // These three meter groups share one compact panel; medium spacing
        // keeps the full stack inside the mounted plugin's available height.
        .gap(d.gap_md)
        .p(d.pad_x)
        // True Peak section (on top)
        .child({
            // Use TickConfig preset for True Peak (quadratic scale from -60 to +6)
            let tick_config = TickConfig::true_peak();

            div()
                .flex()
                .flex_col()
                .gap(d.gap)
                .child(
                    div()
                        .text_size(d.text_sm)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .mb(d.grid)
                        .child(peak_label),
                )
                .child({
                    let label_text = text.max_true_peak.to_string();
                    let value_text = maximum_true_peak_value.clone();
                    let summary = div()
                        .flex()
                        .flex_col()
                        .gap(d.gap)
                        .w_full()
                        .min_w_0()
                        .text_size(d.text_xs)
                        .text_color(theme.text_muted)
                        .child({
                            let label = div().w_full().min_w_0().child(label_text.clone());
                            #[cfg(feature = "dev-api")]
                            let label = label.dev_track_with_state(
                                "meters.lufs.maximum-true-peak-label",
                                DevElementState::default().text(label_text),
                            );
                            label
                        })
                        .child({
                            let value = div().w_full().min_w_0().child(value_text.clone());
                            #[cfg(feature = "dev-api")]
                            let value = value.dev_track_with_state(
                                "meters.lufs.maximum-true-peak-value",
                                DevElementState::default().text(value_text),
                            );
                            value
                        });
                    #[cfg(feature = "dev-api")]
                    let summary = summary.dev_track("meters.lufs.maximum-true-peak-summary");
                    summary
                })
                .children(true_peaks.iter().enumerate().map(|(index, true_peak)| {
                    let bar = PlayerView::render_meter_bar(
                        d,
                        sotf_audio_player::get_channel_label(index, true_peaks.len()),
                        *true_peak,
                        &tick_config,
                        &meter_theme,
                    );
                    #[cfg(feature = "dev-api")]
                    let bar = bar.dev_track(format!("meters.lufs.true-peak-bar.{index}"));
                    bar
                }))
                // Tick marks (aligned with bar using same flex layout)
                .child(render_tick_row(
                    &tick_config,
                    meter_theme.label_width,
                    meter_theme.value_width,
                ))
                // True Peak legend (same flex layout as bar and ticks)
                .child(
                    div()
                        .flex()
                        // intentional: pixel-exact meter divider — do not scale
                        .gap(px(1.0))
                        // Label spacer
                        .child(div().w(px(meter_theme.label_width)))
                        // Legend area (flex-1, justify_between for labels)
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .justify_between()
                                .text_size(d.text_xs)
                                .text_color(meter_theme.color_text_muted)
                                .children(tick_config.major_values.iter().map(|db| {
                                    let label = if *db > 0.0 {
                                        format!("+{}", *db as i32)
                                    } else {
                                        format!("{}", *db as i32)
                                    };
                                    div().child(label)
                                })),
                        )
                        // Value spacer
                        .child(div().w(px(meter_theme.value_width))),
                )
        })
        // LUFS section (below)
        .child({
            // Use TickConfig preset for LUFS (quadratic scale from -60 to 0)
            let tick_config = TickConfig::lufs();
            let integrated_bar = PlayerView::render_meter_bar(
                d,
                "I".to_string(),
                integrated_lufs,
                &tick_config,
                &meter_theme,
            );
            #[cfg(feature = "dev-api")]
            let integrated_bar = integrated_bar.dev_track("meters.lufs.integrated-bar");
            let shortterm_bar = PlayerView::render_meter_bar(
                d,
                "S".to_string(),
                shortterm_lufs,
                &tick_config,
                &meter_theme,
            );
            #[cfg(feature = "dev-api")]
            let shortterm_bar = shortterm_bar.dev_track("meters.lufs.shortterm-bar");
            let momentary_bar = PlayerView::render_meter_bar(
                d,
                "M".to_string(),
                momentary_lufs,
                &tick_config,
                &meter_theme,
            );
            #[cfg(feature = "dev-api")]
            let momentary_bar = momentary_bar.dev_track("meters.lufs.momentary-bar");

            div()
                .flex()
                .flex_col()
                .gap(d.gap)
                .child(
                    div()
                        .text_size(d.text_sm)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .mb(d.grid)
                        .child(text.lufs),
                )
                // These finite-only programme latches are independent of the
                // current-window validity flags and reset with the epoch.
                .child(render_lufs_maximum_summary(
                    d,
                    text.max_momentary,
                    maximum_momentary_value,
                    "meters.lufs.maximum-momentary-label",
                    "meters.lufs.maximum-momentary-value",
                    "meters.lufs.maximum-momentary-summary",
                    theme,
                ))
                .child(render_lra_summary(
                    d,
                    text.lra,
                    lra_value,
                    lra_not_stable,
                    text.lra_not_stable,
                    theme,
                ))
                .child(render_lufs_maximum_summary(
                    d,
                    text.max_shortterm,
                    maximum_shortterm_value,
                    "meters.lufs.maximum-shortterm-label",
                    "meters.lufs.maximum-shortterm-value",
                    "meters.lufs.maximum-shortterm-summary",
                    theme,
                ))
                // Integrated LUFS (uses same scale as ticks)
                .child(integrated_bar)
                // Short-term LUFS (uses same scale as ticks)
                .child(shortterm_bar)
                // Momentary LUFS (uses same scale as ticks)
                .child(momentary_bar)
                // Tick marks (aligned with bar using same flex layout)
                .child(render_tick_row(
                    &tick_config,
                    meter_theme.label_width,
                    meter_theme.value_width,
                ))
                // LUFS legend (same flex layout as bar and ticks)
                .child(
                    div()
                        .flex()
                        .gap(d.grid)
                        // Label spacer
                        .child(div().w(px(meter_theme.label_width)))
                        // Legend area (flex-1, justify_between for labels)
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .justify_between()
                                .text_size(d.text_xs)
                                .text_color(meter_theme.color_text_muted)
                                .child(div().child("-60"))
                                .child(div().child("-30"))
                                .child(div().child("-10"))
                                .child(div().child("0")),
                        )
                        // Value spacer
                        .child(div().w(px(meter_theme.value_width))),
                )
        })
        // Stereo Width / Peak Spread section
        .child({
            if should_use_peak_spread(channel_count) {
                let tick_config = TickConfig::peak_spread();

                div()
                    .flex()
                    .flex_col()
                    .gap(d.gap)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .mb(d.grid)
                            .child(text.peak_spread),
                    )
                    .child(PlayerView::render_peak_spread_bar(
                        d,
                        peak_spread,
                        &tick_config,
                        &meter_theme,
                    ))
                    .child(render_tick_row(
                        &tick_config,
                        meter_theme.label_width,
                        meter_theme.value_width,
                    ))
                    .child(
                        div()
                            .flex()
                            .gap(d.grid)
                            .child(div().w(px(meter_theme.label_width)))
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .justify_between()
                                    .text_size(d.text_xs)
                                    .text_color(meter_theme.color_text_muted)
                                    .child(div().child(text.even))
                                    .child(div().child("6 dB"))
                                    .child(div().child("12 dB"))
                                    .child(div().child("24+")),
                            )
                            .child(div().w(px(meter_theme.value_width))),
                    )
                    .into_any_element()
            } else if channel_count >= 2 && stereo_width.is_some() {
                let tick_config = TickConfig::stereo_width();

                div()
                    .flex()
                    .flex_col()
                    .gap(d.gap)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .mb(d.grid)
                            .child(text.stereo_width),
                    )
                    .child(PlayerView::render_width_bar(
                        d,
                        stereo_width.unwrap_or(0.0),
                        &tick_config,
                        &meter_theme,
                    ))
                    .child(render_tick_row(
                        &tick_config,
                        meter_theme.label_width,
                        meter_theme.value_width,
                    ))
                    .child(
                        div()
                            .flex()
                            .gap(d.grid)
                            .child(div().w(px(meter_theme.label_width)))
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .justify_between()
                                    .text_size(d.text_xs)
                                    .text_color(meter_theme.color_text_muted)
                                    .child(div().child(text.mono))
                                    .child(div().child("50%"))
                                    .child(div().child(text.wide)),
                            )
                            .child(div().w(px(meter_theme.value_width))),
                    )
                    .into_any_element()
            } else {
                div()
                    .text_size(d.text_xs)
                    .text_color(theme.text_muted)
                    .child(text.stereo_width_unavailable)
                    .into_any_element()
            }
        })
}

impl PlayerView {
    /// Render vertical dB legend
    pub fn render_vertical_legend(
        &self,
        d: &Ds,
        theme: &Theme,
        align_right: bool,
    ) -> impl IntoElement {
        let ticks = [0, -6, -12, -18, -24, -30, -40, -50, -60];
        let text_muted = theme.text_muted;
        let border = theme.border;
        let text_xs = d.text_xs;
        let grid = d.grid;

        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .h_full()
            .min_h(rems(17.5))
            .p_0p5() // Match meter group padding
            // Outer container (matches meters_row flex_1 h_full)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(rems(12.5))
                    // Ticks area (matches meter_bar flex_1).
                    // Width is wide enough to fit a 3-char dB label
                    // ("-60") + tick + a small gap; both legends use the
                    // same width so the meter group between them sits at
                    // the panel's true horizontal centre. Narrower
                    // widths cause labels to overflow on the left
                    // legend (clipped or pushed out of the panel) while
                    // the right legend's labels overflow into adjacent
                    // empty space — visible asymmetry.
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_h(rems(11.25))
                            .w(rems(2.25))
                            .overflow_hidden()
                            .children(ticks.into_iter().map(move |db| {
                                let pos = db_to_position(db as f64);
                                // Use top positioning: top = (1 - pos), then offset by half line height
                                let top_fraction = 1.0 - pos;

                                // Adjust label offset for edge labels to keep them visible:
                                // - Top label (0 dB): move label down
                                // - Bottom label (-60 dB): move label up
                                // - Other labels: no additional offset
                                let label_offset = if db == 0 {
                                    px(6.0) // Top: move label down
                                } else if db == -60 {
                                    px(-6.0) // Bottom: move label up
                                } else {
                                    px(0.0) // No additional offset
                                };

                                let label_text = db_tick_label(db)
                                    .map(|s| s.into())
                                    .unwrap_or_else(|| format!("{}", db));
                                let label = div()
                                    .text_size(d.text_xs)
                                    .text_color(text_muted)
                                    .mt(label_offset)
                                    .child(label_text);

                                let tick = div().w(px(4.0)).h(px(1.0)).bg(border);

                                let container = div()
                                    .absolute()
                                    .left_0()
                                    .right_0()
                                    .top(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                                        top_fraction,
                                    )))
                                    // Offset by half line height (~6px for 9px text) to center tick on position
                                    .mt(px(-6.0))
                                    .flex()
                                    .items_center()
                                    .justify_between();

                                if align_right {
                                    // Legend on right: tick → label (tick points toward meter on left)
                                    container.child(tick).child(label)
                                } else {
                                    // Legend on left: label → tick (tick points toward meter on right)
                                    container.child(label).child(tick)
                                }
                            })),
                    )
                    // Spacer for Channel Name (matches render_gradient_meter channel name)
                    .child(
                        div().text_size(text_xs).mt(grid).opacity(0.0).child("X"), // Dummy text to match height
                    ),
            )
            // Spacer (matches MSD buttons height and margin)
            .child(
                div()
                    .flex()
                    .flex_col()
                    // intentional: 1px divider mirrors real MSD column below — do not scale
                    .gap(px(1.0))
                    .mt(grid)
                    .items_center()
                    .justify_center()
                    .opacity(0.0) // Invisible, just for spacing
                    .child(
                        div()
                            .px(d.half_grid)
                            .py(d.half_grid)
                            .text_size(text_xs)
                            .child("M"),
                    )
                    .child(
                        div()
                            .px(d.half_grid)
                            .py(d.half_grid)
                            .text_size(text_xs)
                            .child("S"),
                    )
                    .child(
                        div()
                            .px(d.half_grid)
                            .py(d.half_grid)
                            .text_size(text_xs)
                            .child("D"),
                    ),
            )
    }

    /// Render a single meter group with M/S/D buttons below the channels
    /// Render a single meter group with M/S/D buttons below the channels
    pub fn render_meter_group(
        &self,
        group: &ChannelGroup,
        group_idx: usize,
        is_selected: bool,
        loudness: Option<&sotf_audio_player::LoudnessData>,
        peak_hold: &[f64],
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let channel_data = build_channel_meter_data(&group.channels, loudness, peak_hold);
        let text = LevelMeterTranslations::for_language(self.state.read(cx).app.ui_state.language);
        self.render_meter_group_data(
            group_idx,
            group.muted,
            group.soloed,
            group.dimmed,
            is_selected,
            channel_data,
            false,
            text,
            theme,
            cx,
        )
    }

    /// Render a meter group from pre-computed channel data. This avoids
    /// cloning the full `ChannelGroup` / `peak_hold` vectors in callers
    /// such as `render_meters_panel`.
    fn render_meter_group_data(
        &self,
        group_idx: usize,
        muted: bool,
        soloed: bool,
        dimmed: bool,
        is_selected: bool,
        channel_data: Vec<ChannelMeterData>,
        show_group_status: bool,
        text: LevelMeterTranslations,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let background_secondary = theme.background_secondary;
        let group_status = div()
            .w_full()
            .px(d.half_grid)
            .py(d.half_grid)
            .rounded(d.r_sm)
            .text_size(d.text_xs)
            .font_weight(if is_selected {
                FontWeight::BOLD
            } else {
                FontWeight::NORMAL
            })
            .text_color(if is_selected {
                theme.accent
            } else {
                theme.text_muted
            })
            .child(if is_selected {
                format!("{} {} · {}", text.group, group_idx + 1, text.selected)
            } else {
                format!("{} {}", text.group, group_idx + 1)
            });
        #[cfg(feature = "dev-api")]
        let group_status = group_status.dev_track(format!("meters.group.{group_idx}.status"));

        let group_element = div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .h_full()
            .min_h(rems(17.5))
            .p_0p5()
            .rounded(d.r_sm)
            .border_1()
            .border_color(if is_selected {
                theme.accent
            } else {
                background_secondary
            })
            .bg(background_secondary)
            .when(show_group_status, |group| group.child(group_status))
            // Channel meters
            .child(div().flex().gap_px().flex_1().min_h(rems(12.5)).children(
                channel_data.into_iter().map(|data| {
                    render_gradient_meter(
                        &d,
                        data.fill_ratio,
                        data.yellow_threshold,
                        data.red_threshold,
                        data.peak_hold_ratio,
                        data.name,
                        theme,
                    )
                }),
            ))
            // M/S/D buttons vertical column below meters, centered
            .child(
                div()
                    .flex()
                    .flex_col()
                    // intentional: pixel-exact MSD divider — do not scale
                    .gap(px(1.0))
                    .mt(d.grid)
                    .items_center()
                    .justify_center()
                    .child(self.render_msd_button(
                        "M",
                        text.mute,
                        text.group,
                        muted,
                        theme.button_mute_active,
                        group_idx,
                        "mute",
                        theme,
                        cx,
                    ))
                    .child(self.render_msd_button(
                        "S",
                        text.solo,
                        text.group,
                        soloed,
                        theme.button_solo_active,
                        group_idx,
                        "solo",
                        theme,
                        cx,
                    ))
                    .child(self.render_msd_button(
                        "D",
                        text.dim,
                        text.group,
                        dimmed,
                        theme.button_dim_active,
                        group_idx,
                        "dim",
                        theme,
                        cx,
                    )),
            );
        #[cfg(feature = "dev-api")]
        let group_element = group_element.dev_track_with_state(
            format!("meters.group.{group_idx}"),
            DevElementState::default().selected(is_selected),
        );
        group_element
    }

    /// Render M/S/D button (interactive)
    pub fn render_msd_button(
        &self,
        label: &'static str,
        action_name: &'static str,
        group_name: &'static str,
        active: bool,
        active_color: gpui::Rgba,
        group_idx: usize,
        button_type: &'static str,
        theme: &Theme,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let shortcut = match button_type {
            "mute" => "Alt+M",
            "solo" => "Alt+Shift+M",
            "dim" => "Ctrl+Alt+M",
            _ => "",
        };
        let tooltip = if shortcut.is_empty() {
            format!("{action_name} {group_name} {}", group_idx + 1)
        } else {
            format!("{action_name} {group_name} {} ({shortcut})", group_idx + 1)
        };
        let tooltip_theme = theme.clone();
        let mut button_theme = theme.to_button_theme();
        button_theme.accent = active_color;
        button_theme.accent_hover = active_color;
        let state_handle = self.state.clone();
        let button = Button::new(format!("meter-{button_type}-{group_idx}"), label)
            .size(ButtonSize::Xs)
            .variant(ButtonVariant::Ghost)
            .selected(active)
            .theme(button_theme)
            .aria_label(format!("{action_name} {group_name} {}", group_idx + 1))
            .on_click(move |_window, cx| {
                state_handle.update(cx, |state, cx| {
                    if group_idx < state.app.level_meters.groups.len() {
                        match button_type {
                            "mute" => {
                                let new_state = !state.app.level_meters.groups[group_idx].muted;
                                state.app.set_level_meter_mute(group_idx, new_state);
                            }
                            "solo" => {
                                let new_state = !state.app.level_meters.groups[group_idx].soloed;
                                state.app.set_level_meter_solo(group_idx, new_state);
                            }
                            "dim" => {
                                let new_state = !state.app.level_meters.groups[group_idx].dimmed;
                                state.app.set_level_meter_dim(group_idx, new_state);
                            }
                            _ => {}
                        }
                    }
                    cx.notify();
                });
            });
        #[cfg(feature = "dev-api")]
        let button = button.dev_track_with_state(
            format!("meters.group.{group_idx}.{button_type}"),
            DevElementState::default().enabled(true).selected(active),
        );
        div()
            .id(SharedString::from(format!(
                "meter-tooltip-{button_type}-{group_idx}"
            )))
            .tooltip(move |_window, cx| themed_tooltip(tooltip.clone(), &tooltip_theme, cx))
            .child(button)
    }

    /// Render separate Meters panel (for queue screen)
    pub fn render_meters_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let text = LevelMeterTranslations::for_language(self.state.read(cx).app.ui_state.language);
        // Pre-compute only the render data while the state read lock is held.
        // Previously this cloned the full `groups`, `peak_hold`, `loudness`
        // and `theme` values on every frame.
        let (theme, groups_data, selected_group, has_data) = {
            let state = self.state.read(cx);
            let theme = state.app.ui_state.theme.clone();
            let loudness = state.app.playback.loudness_info.as_deref();
            let peak_hold = &state.app.level_meters.peak_hold;
            let groups = &state.app.level_meters.groups;

            let groups_data: Vec<GroupMeterData> = groups
                .iter()
                .enumerate()
                .map(|(group_idx, group)| GroupMeterData {
                    group_idx,
                    is_selected: group_idx == state.app.level_meters.selected_group,
                    muted: group.muted,
                    soloed: group.soloed,
                    dimmed: group.dimmed,
                    channels: build_channel_meter_data(&group.channels, loudness, peak_hold),
                })
                .collect();

            (
                theme,
                groups_data,
                state.app.level_meters.selected_group,
                loudness.is_some(),
            )
        };
        let center_meter_groups = groups_data.len() <= 2;
        let selected_status = div()
            .text_size(d.text_xs)
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.accent)
            .child(format!(
                "{}: {} {}",
                text.selected,
                text.group,
                selected_group + 1
            ));
        #[cfg(feature = "dev-api")]
        let selected_status = selected_status.dev_track("meters.selected-group");
        let panel = div()
            .flex()
            .flex_col()
            .items_center()
            .size_full()
            .p(d.pad_x)
            .bg(theme.background)
            .child(
                div().w_full().mb(d.gap).child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(d.half_grid)
                        .text_size(d.text_sm)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_align(TextAlign::Center)
                        .child(text.level_meters)
                        .when(has_data, |header| header.child(selected_status)),
                ),
            )
            .when(has_data, |panel| {
                panel.child({
                    // Use a for loop to avoid FnMut closure escape issues with cx
                    let mut meter_elements = Vec::new();

                    // Left Legend
                    meter_elements.push(
                        self.render_vertical_legend(&d, &theme, false)
                            .into_any_element(),
                    );

                    for group_data in groups_data {
                        meter_elements.push(
                            self.render_meter_group_data(
                                group_data.group_idx,
                                group_data.muted,
                                group_data.soloed,
                                group_data.dimmed,
                                group_data.is_selected,
                                group_data.channels,
                                true,
                                text,
                                &theme,
                                cx,
                            )
                            .into_any_element(),
                        );
                    }

                    // Right Legend
                    meter_elements.push(
                        self.render_vertical_legend(&d, &theme, true)
                            .into_any_element(),
                    );

                    div()
                        .id("meter-groups-scroll")
                        .flex()
                        .flex_1()
                        .when(center_meter_groups, |groups| groups.justify_center())
                        .when(!center_meter_groups, |groups| groups.justify_start())
                        .gap_0()
                        .overflow_x_scroll()
                        .min_h(rems(18.75))
                        .children(meter_elements)
                })
            })
            .when(!has_data, |panel| {
                let empty_state = render_no_meter_data(&d, text, &theme);
                #[cfg(feature = "dev-api")]
                let empty_state = empty_state.dev_track("meters.no-data");
                panel.child(div().flex().flex_1().size_full().child(empty_state))
            });
        #[cfg(feature = "dev-api")]
        let panel = panel
            .dev_track_with_state("meters.panel", DevElementState::default().enabled(has_data));
        panel
    }

    /// Render unified meter bar with consistent styling
    /// Uses the TickConfig's scale for bar fill to match tick mark positions
    pub fn render_meter_bar(
        d: &Ds,
        label: String,
        value: f64,
        tick_config: &TickConfig,
        meter_theme: &MeterTheme,
    ) -> impl IntoElement {
        let horizontal_theme =
            meter_theme.to_horizontal_meter_theme(d.text_xs, px(meter_theme.horizontal_gap));
        render_horizontal_meter_bar(label, value, tick_config, horizontal_theme)
    }

    /// Render stereo width bar (0 = mono, 1 = wide)
    /// Uses the TickConfig's scale for bar fill to match tick mark positions
    pub fn render_width_bar(
        d: &Ds,
        width: f64,
        tick_config: &TickConfig,
        meter_theme: &MeterTheme,
    ) -> impl IntoElement {
        let horizontal_theme =
            meter_theme.to_horizontal_meter_theme(d.text_xs, px(meter_theme.horizontal_gap));
        let ratio = tick_config.value_to_position(width);
        render_horizontal_meter_bar_with(
            "W",
            ratio,
            meter_theme.color_info,
            format_width_percent(width),
            horizontal_theme,
        )
    }

    /// Render multichannel peak spread bar (0 = even, higher = wider channel imbalance)
    pub fn render_peak_spread_bar(
        d: &Ds,
        spread_db: f64,
        tick_config: &TickConfig,
        meter_theme: &MeterTheme,
    ) -> impl IntoElement {
        let horizontal_theme =
            meter_theme.to_horizontal_meter_theme(d.text_xs, px(meter_theme.horizontal_gap));
        let ratio = tick_config.value_to_position(spread_db);
        render_horizontal_meter_bar_with(
            "D",
            ratio,
            meter_theme.color_info,
            format!("{spread_db:.1} dB"),
            horizontal_theme,
        )
    }

    /// Render LUFS display with True Peak bars at top (wrapper method)
    pub fn render_lufs_with_true_peak(
        &self,
        d: &Ds,
        loudness: Option<&sotf_audio_player::LoudnessData>,
        layout_scale: f32,
        text: LevelMeterTranslations,
        theme: &Theme,
    ) -> impl IntoElement {
        // Call the standalone function
        render_lufs_with_true_peak(d, loudness, layout_scale, text, theme)
    }

    /// Render separate LUFS panel
    pub fn render_lufs_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let text = LevelMeterTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let (theme, loudness, layout_scale) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.playback.loudness_info.clone(),
                crate::ui::compute_combined_scale(
                    state.app.ui_state.window_width,
                    state.app.ui_state.window_height,
                    state.app.ui_state.font_scale,
                    state.app.ui_state.min_font_size_px,
                    state.app.ui_state.max_font_size_px,
                ),
            )
        };
        // No `.items_center()` here: cross-axis alignment defaults to
        // stretch so the inner content fills the panel width. With
        // items_center, gpui's flex implementation would size children
        // to their intrinsic content width and `w_full()` on the inner
        // wrapper would not propagate — bars would collapse to ~150px
        // even on a 700px panel.
        let has_data = loudness.is_some();
        let panel = div()
            .flex()
            .flex_col()
            .w_full()
            .p(d.card)
            .bg(theme.background)
            .when_some(loudness.as_deref(), |panel, loudness| {
                panel.child(self.render_lufs_with_true_peak(
                    &d,
                    Some(loudness),
                    layout_scale,
                    text,
                    &theme,
                ))
            })
            .when(!has_data, |panel| {
                let empty_state = render_no_meter_data(&d, text, &theme);
                #[cfg(feature = "dev-api")]
                let empty_state = empty_state.dev_track("meters.lufs-no-data");
                panel.child(empty_state)
            });
        #[cfg(feature = "dev-api")]
        let panel = panel.dev_track_with_state(
            "meters.lufs-panel",
            DevElementState::default().enabled(has_data),
        );
        panel
    }
}

#[cfg(test)]
mod programme_true_peak_tests {
    use super::*;
    use crate::app::i18n::Language;
    use sotf_audio_player::LoudnessData;

    #[test]
    fn programme_maximum_is_independent_of_status_flags_and_later_intervals() {
        let text = LevelMeterTranslations::for_language(Language::English);
        assert_eq!(maximum_lufs_value(Some(-9.36)), "-9.4");
        assert_eq!(maximum_lufs_value(None), "—");
        assert_eq!(maximum_lufs_value(Some(f64::INFINITY)), "—");
        let mut loudness = LoudnessData::new(2);
        loudness.maximum_true_peak_dbtp = Some(-1.2);
        loudness.true_peak_is_compliant = false;
        loudness.true_peak_valid = false;
        loudness.update_true_peaks(&[-3.0, -4.0]);
        let label = maximum_true_peak_value(&loudness, text);
        assert_eq!(label, "-1.2 dBTP");

        loudness.update_true_peaks(&[-9.0, -10.0]);
        assert_eq!(maximum_true_peak_value(&loudness, text), label);

        loudness.maximum_true_peak_dbtp = None;
        loudness.true_peak_is_compliant = true;
        assert_eq!(maximum_true_peak_value(&loudness, text), "— dBTP");

        loudness.true_peak_is_compliant = false;
        assert_eq!(maximum_true_peak_value(&loudness, text), "Unavailable");

        loudness.maximum_true_peak_dbtp = Some(f64::INFINITY);
        assert_eq!(maximum_true_peak_value(&loudness, text), "Unavailable");
    }
}
