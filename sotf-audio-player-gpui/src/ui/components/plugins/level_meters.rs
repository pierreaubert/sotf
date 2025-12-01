//! Level meter UI components and logic.
//!
//! This module consolidates all level meter functionality:
//! - GPU-accelerated level meter element (`LevelMeterElement`)
//! - Level meter group rendering (with M/S/D buttons)
//! - Level meter panel rendering (for queue screen)
//! - App methods for level meter group management (mute/solo/dim)

use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::PluginSettings;
use sotf_plugins::ChannelState;
use std::panic;

use crate::app::{App as AppState, ChannelGroup, ChannelInfo};
use crate::theme::Theme;
use crate::ui::PlayerView;
use super::{MeterTheme, TickConfig, render_tick_row};

// ============================================================================
// dB Scale Utilities
// ============================================================================

/// dB scale positions: maps dB value to visual position (0.0 = bottom, 1.0 = top)
/// Using non-linear scale for better visual representation
pub fn db_to_position(db: f64) -> f32 {
    // -60dB = 0%, -30dB = 33%, -10dB = 66%, 0dB = 100%
    let normalized = if db <= -60.0 {
        0.0
    } else if db <= -30.0 {
        // -60 to -30: linear from 0 to 0.33
        ((db + 60.0) / 30.0) * 0.33
    } else if db <= -10.0 {
        // -30 to -10: linear from 0.33 to 0.66
        0.33 + ((db + 30.0) / 20.0) * 0.33
    } else {
        // -10 to 0: linear from 0.66 to 1.0
        0.66 + ((db + 10.0) / 10.0) * 0.34
    };
    normalized.clamp(0.0, 1.0) as f32
}

// ============================================================================
// GPU-Accelerated Level Meter Element
// ============================================================================

/// Colors used by the level meter
#[derive(Clone)]
pub struct MeterColors {
    /// Background color
    pub background: Rgba,
    /// Green (safe) level color
    pub green: Rgba,
    /// Yellow (caution) level color
    pub yellow: Rgba,
    /// Red (danger/clipping) level color
    pub red: Rgba,
    /// Peak indicator color
    pub peak: Rgba,
    /// Text color for channel name
    pub text: Rgba,
}

impl Default for MeterColors {
    fn default() -> Self {
        Self {
            background: rgb(0x1e1e1e),
            green: rgb(0x22c55e),
            yellow: rgb(0xf59e0b),
            red: rgb(0xdc2626),
            peak: rgb(0xffffff),
            text: rgb(0x999999),
        }
    }
}

/// GPU-accelerated level meter element
///
/// Renders a vertical level meter with gradient coloring (green → yellow → red)
/// using direct GPU quad rendering for maximum performance.
pub struct LevelMeterElement {
    /// Current level in dB
    level_db: f64,
    /// Peak hold level in dB
    peak_db: Option<f64>,
    /// Channel name to display
    #[allow(dead_code)]
    channel_name: SharedString,
    /// Whether this channel is clipping
    is_clipping: bool,
    /// Width of the meter bar
    bar_width: Pixels,
    /// Colors for the gradient
    colors: MeterColors,
}

impl LevelMeterElement {
    /// Create a new level meter element
    pub fn new(level_db: f64, channel_name: impl Into<SharedString>) -> Self {
        Self {
            level_db,
            peak_db: None,
            channel_name: channel_name.into(),
            is_clipping: level_db > -0.1,
            bar_width: px(16.0),
            colors: MeterColors::default(),
        }
    }

    /// Set the peak hold level
    pub fn peak(mut self, peak_db: f64) -> Self {
        self.peak_db = Some(peak_db);
        self
    }

    /// Set the bar width
    pub fn width(mut self, width: Pixels) -> Self {
        self.bar_width = width;
        self
    }

    /// Set custom colors
    pub fn colors(mut self, colors: MeterColors) -> Self {
        self.colors = colors;
        self
    }

    /// Get the fill ratio for the current level
    fn fill_ratio(&self) -> f32 {
        db_to_position(self.level_db)
    }

    /// Get the yellow threshold position
    fn yellow_threshold(&self) -> f32 {
        db_to_position(-6.0)
    }

    /// Get the red threshold position
    fn red_threshold(&self) -> f32 {
        db_to_position(-1.0)
    }
}

impl IntoElement for LevelMeterElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for LevelMeterElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        // Request a flexible layout that fills available space vertically
        let layout_id = window.request_layout(
            Style {
                size: size(self.bar_width.into(), relative(1.0).into()),
                min_size: size(self.bar_width.into(), px(60.0).into()),
                flex_grow: 1.0,
                flex_shrink: 0.0,
                ..Default::default()
            },
            [],
            cx,
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        let fill_ratio = self.fill_ratio();
        let yellow_threshold = self.yellow_threshold();
        let red_threshold = self.red_threshold();

        // Leave space at bottom for channel name
        let text_height = px(16.0);
        let meter_bounds = Bounds {
            origin: bounds.origin,
            size: size(bounds.size.width, bounds.size.height - text_height),
        };

        // Paint background
        window.paint_quad(PaintQuad {
            bounds: meter_bounds,
            corner_radii: Corners::all(px(2.0)),
            background: self.colors.background.into(),
            border_widths: Edges::default(),
            border_color: Hsla::transparent_black(),
            border_style: Default::default(),
        });

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

        let meter_height = meter_bounds.size.height;

        // Paint green segment (from bottom)
        if green_height > 0.001 {
            let segment_height = meter_height * green_height;
            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(
                        meter_bounds.origin.x,
                        meter_bounds.origin.y + meter_height - segment_height,
                    ),
                    size: size(meter_bounds.size.width, segment_height),
                },
                corner_radii: Corners::default(),
                background: self.colors.green.into(),
                border_widths: Edges::default(),
                border_color: Hsla::transparent_black(),
                border_style: Default::default(),
            });
        }

        // Paint yellow segment (above green)
        if yellow_height > 0.001 {
            let segment_height = meter_height * yellow_height;
            let segment_bottom = meter_height * yellow_threshold;
            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(
                        meter_bounds.origin.x,
                        meter_bounds.origin.y + meter_height - segment_bottom - segment_height,
                    ),
                    size: size(meter_bounds.size.width, segment_height),
                },
                corner_radii: Corners::default(),
                background: self.colors.yellow.into(),
                border_widths: Edges::default(),
                border_color: Hsla::transparent_black(),
                border_style: Default::default(),
            });
        }

        // Paint red segment (above yellow)
        if red_height > 0.001 {
            let segment_height = meter_height * red_height;
            let segment_bottom = meter_height * red_threshold;
            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(
                        meter_bounds.origin.x,
                        meter_bounds.origin.y + meter_height - segment_bottom - segment_height,
                    ),
                    size: size(meter_bounds.size.width, segment_height),
                },
                corner_radii: Corners::default(),
                background: self.colors.red.into(),
                border_widths: Edges::default(),
                border_color: Hsla::transparent_black(),
                border_style: Default::default(),
            });
        }

        // Paint peak indicator if present
        if let Some(peak_db) = self.peak_db {
            let peak_pos = db_to_position(peak_db);
            let peak_y = meter_bounds.origin.y + meter_height * (1.0 - peak_pos);
            let peak_color = if self.is_clipping {
                self.colors.red
            } else {
                self.colors.peak
            };

            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(meter_bounds.origin.x, peak_y - px(1.0)),
                    size: size(meter_bounds.size.width, px(2.0)),
                },
                corner_radii: Corners::default(),
                background: peak_color.into(),
                border_widths: Edges::default(),
                border_color: Hsla::transparent_black(),
                border_style: Default::default(),
            });
        }
    }
}

// ============================================================================
// Gradient Meter Rendering (div-based)
// ============================================================================

/// Render a meter with gradient coloring (green, yellow at top, red at clip)
pub fn render_gradient_meter(
    fill_ratio: f32,
    yellow_threshold: f32,
    red_threshold: f32,
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
    div()
        .flex()
        .flex_col()
        .items_center()
        .flex_1()
        // Meter bar container
        .child(
            div()
                .w(px(16.0))
                .flex_1()
                .bg(theme_c.background)
                .rounded(px(2.0))
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
                        .bg(theme_c.meter_normal),
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
                            .bg(theme_c.meter_warning),
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
                            .bg(theme_c.meter_clip),
                    )
                }),
        )
        // Channel name
        .child(
            div()
                .text_xs()
                .text_color(theme.text_muted)
                .mt_1()
                .child(channel_name),
        )
}

// ============================================================================
// PlayerView Level Meter UI Methods
// ============================================================================

impl PlayerView {
    /// Render a single meter group with M/S/D buttons below the channels
    pub fn render_meter_group(
        &self,
        group: &ChannelGroup,
        group_idx: usize,
        is_selected: bool,
        loudness: Option<&sotf_audio_player::LoudnessData>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let muted = group.muted;
        let soloed = group.soloed;
        let dimmed = group.dimmed;

        // Pre-compute channel data to avoid closure issues with cx
        let channel_data: Vec<_> = group
            .channels
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

                let fill_ratio = db_to_position(peak_db);
                let yellow_threshold = db_to_position(-6.0);
                let red_threshold = db_to_position(-1.0);

                (
                    fill_ratio,
                    yellow_threshold,
                    red_threshold,
                    channel.name.clone(),
                )
            })
            .collect();

        let theme_c = theme.clone();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .p_2()
            .rounded_md()
            .when(is_selected, |d| d.bg(theme_c.surface_selected))
            .when(!is_selected, |d| d.bg(theme_c.background_secondary))
            // Group header (just the name)
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text_secondary)
                    .mb_1()
                    .child(group.name.clone()),
            )
            // Channel meters (3x taller for better visibility)
            .child(div().flex().gap_1().flex_1().min_h(px(240.0)).children(
                channel_data.into_iter().map(
                    |(fill_ratio, yellow_threshold, red_threshold, name)| {
                        render_gradient_meter(
                            fill_ratio,
                            yellow_threshold,
                            red_threshold,
                            name,
                            theme,
                        )
                    },
                ),
            ))
            // M/S/D buttons below channels (spans all channels in group)
            .child(
                div()
                    .flex()
                    .gap(px(2.0))
                    .mt_1()
                    .justify_center()
                    .child(self.render_msd_button(
                        "M",
                        muted,
                        theme.button_mute_active,
                        group_idx,
                        "mute",
                        theme,
                        cx,
                    ))
                    .child(self.render_msd_button(
                        "S",
                        soloed,
                        theme.button_solo_active,
                        group_idx,
                        "solo",
                        theme,
                        cx,
                    ))
                    .child(self.render_msd_button(
                        "D",
                        dimmed,
                        theme.button_dim_active,
                        group_idx,
                        "dim",
                        theme,
                        cx,
                    )),
            )
    }

    /// Render M/S/D button (interactive)
    pub fn render_msd_button(
        &self,
        label: &'static str,
        active: bool,
        active_color: gpui::Rgba,
        group_idx: usize,
        button_type: &'static str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme_c = theme.clone();
        div()
            .id(SharedString::from(format!(
                "msd-{}-{}",
                button_type, group_idx
            )))
            .px_2()
            .py(px(2.0))
            .rounded(px(2.0))
            .text_xs()
            .cursor_pointer()
            .when(active, |d| {
                d.bg(active_color).text_color(theme_c.text_primary)
            })
            .when(!active, |d| {
                d.bg(theme_c.surface)
                    .text_color(theme_c.text_muted)
                    .hover(|style| style.bg(theme_c.surface_hover))
            })
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        if group_idx < state.app.level_meter_groups.len() {
                            match button_type {
                                "mute" => {
                                    state.app.level_meter_groups[group_idx].muted =
                                        !state.app.level_meter_groups[group_idx].muted
                                }
                                "solo" => {
                                    state.app.level_meter_groups[group_idx].soloed =
                                        !state.app.level_meter_groups[group_idx].soloed
                                }
                                "dim" => {
                                    state.app.level_meter_groups[group_idx].dimmed =
                                        !state.app.level_meter_groups[group_idx].dimmed
                                }
                                _ => {}
                            }
                        }
                    });
                    cx.notify();
                }),
            )
            .child(label)
    }

    /// Render separate Meters panel (for queue screen)
    pub fn render_meters_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, loudness, groups, selected_group) = {
            let state = self.state.read(cx);
            (
                state.app.theme.clone(),
                state.app.loudness_info.clone(),
                state.app.level_meter_groups.clone(),
                state.app.selected_level_meter_group,
            )
        };

        let theme_c = theme.clone();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(350.0))
            .p_4()
            .bg(theme.background)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .mb_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Level Meters"),
                    )
                    .child(
                        div()
                            .px_2()
                            .py(px(2.0))
                            .rounded(px(3.0))
                            .text_xs()
                            .bg(theme.surface)
                            .text_color(theme.text_secondary)
                            .cursor_pointer()
                            .hover(|style| style.bg(theme_c.surface_hover))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        for group in &mut state.app.level_meter_groups {
                                            group.muted = false;
                                            group.soloed = false;
                                            group.dimmed = false;
                                        }
                                    });
                                    cx.notify();
                                }),
                            )
                            .child("Clear All"),
                    ),
            )
            .child({
                // Use a for loop to avoid FnMut closure escape issues with cx
                let mut meter_elements = Vec::new();
                for (idx, group) in groups.iter().enumerate() {
                    let is_selected = idx == selected_group;
                    meter_elements.push(
                        self.render_meter_group(
                            group,
                            idx,
                            is_selected,
                            loudness.as_ref(),
                            &theme,
                            cx,
                        )
                        .into_any_element(),
                    );
                }
                div()
                    .id("meter-groups-scroll")
                    .flex()
                    .gap_2()
                    .overflow_x_scroll()
                    .children(meter_elements)
            })
    }

    /// Render unified meter bar with consistent styling
    /// Uses the TickConfig's scale for bar fill to match tick mark positions
    pub fn render_meter_bar(
        label: &str,
        value: f64,
        tick_config: &TickConfig,
        meter_theme: &MeterTheme,
    ) -> impl IntoElement {
        // Use the same scale as the ticks for bar fill
        let ratio = tick_config.value_to_position(value);
        let bar_color = meter_theme.color_for_ratio(ratio);

        div()
            .flex()
            .items_center()
            .gap(px(4.0)) // Tighter gap for more bar space
            // Label
            .child(
                div()
                    .w(px(meter_theme.label_width))
                    .text_xs()
                    .text_color(meter_theme.color_text)
                    .child(label.to_string()),
            )
            // Bar with border
            .child(
                div()
                    .flex_1()
                    .h(px(meter_theme.bar_height))
                    .rounded(px(meter_theme.border_radius))
                    .border(px(meter_theme.border_width))
                    .border_color(meter_theme.color_border)
                    .bg(meter_theme.color_background)
                    .overflow_hidden()
                    .child(div().h_full().w(gpui::relative(ratio)).bg(bar_color)),
            )
            // Value display
            .child(
                div()
                    .w(px(meter_theme.value_width))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(meter_theme.color_text)
                    .text_align(TextAlign::Right)
                    .child(format!("{:.1}", value)),
            )
    }

    /// Render stereo width bar (0 = mono, 1 = wide)
    /// Uses the TickConfig's scale for bar fill to match tick mark positions
    pub fn render_width_bar(
        width: f64,
        tick_config: &TickConfig,
        meter_theme: &MeterTheme,
    ) -> impl IntoElement {
        // Use the same scale as the ticks for bar fill
        let ratio = tick_config.value_to_position(width);
        // Color: cyan/teal for width (uses info color from theme via meter_theme)
        let bar_color = meter_theme.color_info;

        div()
            .flex()
            .items_center()
            .gap(px(4.0))
            // Label
            .child(
                div()
                    .w(px(meter_theme.label_width))
                    .text_xs()
                    .text_color(meter_theme.color_text)
                    .child("W"),
            )
            // Bar with border
            .child(
                div()
                    .flex_1()
                    .h(px(meter_theme.bar_height))
                    .rounded(px(meter_theme.border_radius))
                    .border(px(meter_theme.border_width))
                    .border_color(meter_theme.color_border)
                    .bg(meter_theme.color_background)
                    .overflow_hidden()
                    .child(div().h_full().w(gpui::relative(ratio)).bg(bar_color)),
            )
            // Value display
            .child(
                div()
                    .w(px(meter_theme.value_width))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(meter_theme.color_text)
                    .text_align(TextAlign::Right)
                    .child(format!("{:.0}%", width * 100.0)),
            )
    }

    /// Render LUFS display with True Peak bars at top
    pub fn render_lufs_with_true_peak(
        &self,
        loudness: Option<&sotf_audio_player::LoudnessData>,
        theme: &Theme,
    ) -> impl IntoElement {
        let (
            integrated_lufs,
            shortterm_lufs,
            momentary_lufs,
            true_peak_left,
            true_peak_right,
            stereo_width,
        ) = if let Some(l) = loudness {
            let tp_left = l.true_peaks_dbtp.first().copied().unwrap_or(-60.0);
            let tp_right = l.true_peaks_dbtp.get(1).copied().unwrap_or(tp_left);
            // Stereo width derived from correlation: +1 = mono (0), 0 = uncorrelated (0.5), -1 = out of phase (1)
            let width = l
                .correlation_lr
                .map(|c| ((1.0 - c) / 2.0).clamp(0.0, 1.0))
                .unwrap_or(0.5);
            (
                l.integrated_lufs,
                l.shortterm_lufs,
                l.momentary_lufs,
                tp_left,
                tp_right,
                width,
            )
        } else {
            (-60.0, -60.0, -60.0, -60.0, -60.0, 0.5)
        };

        let meter_theme = MeterTheme::default();

        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_3()
            // True Peak section (on top)
            .child({
                // Use TickConfig preset for True Peak (quadratic scale from -60 to +6)
                let tick_config = TickConfig::true_peak();

                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .mb_1()
                            .child("True Peak"),
                    )
                    // Left channel bar (uses same scale as ticks)
                    .child(Self::render_meter_bar(
                        "L",
                        true_peak_left,
                        &tick_config,
                        &meter_theme,
                    ))
                    // Right channel bar (uses same scale as ticks)
                    .child(Self::render_meter_bar(
                        "R",
                        true_peak_right,
                        &tick_config,
                        &meter_theme,
                    ))
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
                            .gap(px(1.0))
                            // Label spacer
                            .child(div().w(px(meter_theme.label_width)))
                            // Legend area (flex-1, justify_between for labels)
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .justify_between()
                                    .text_xs()
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

                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .mb_1()
                            .child("LUFS"),
                    )
                    // Integrated LUFS (uses same scale as ticks)
                    .child(Self::render_meter_bar(
                        "I",
                        integrated_lufs,
                        &tick_config,
                        &meter_theme,
                    ))
                    // Short-term LUFS (uses same scale as ticks)
                    .child(Self::render_meter_bar(
                        "S",
                        shortterm_lufs,
                        &tick_config,
                        &meter_theme,
                    ))
                    // Momentary LUFS (uses same scale as ticks)
                    .child(Self::render_meter_bar(
                        "M",
                        momentary_lufs,
                        &tick_config,
                        &meter_theme,
                    ))
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
                            .gap(px(4.0))
                            // Label spacer
                            .child(div().w(px(meter_theme.label_width)))
                            // Legend area (flex-1, justify_between for labels)
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .justify_between()
                                    .text_xs()
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
            // Stereo Width section
            .child({
                // Use TickConfig preset for stereo width (linear scale 0 to 1)
                let tick_config = TickConfig::stereo_width();

                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .mb_1()
                            .child("Stereo Width"),
                    )
                    // Width bar (uses same scale as ticks)
                    .child(Self::render_width_bar(
                        stereo_width,
                        &tick_config,
                        &meter_theme,
                    ))
                    // Tick marks (aligned with bar using same flex layout)
                    .child(render_tick_row(
                        &tick_config,
                        meter_theme.label_width,
                        meter_theme.value_width,
                    ))
                    // Width legend (same flex layout as bar and ticks)
                    .child(
                        div()
                            .flex()
                            .gap(px(4.0))
                            // Label spacer
                            .child(div().w(px(meter_theme.label_width)))
                            // Legend area (flex-1, justify_between for labels)
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .justify_between()
                                    .text_xs()
                                    .text_color(meter_theme.color_text_muted)
                                    .child(div().child("Mono"))
                                    .child(div().child("50%"))
                                    .child(div().child("Wide")),
                            )
                            // Value spacer
                            .child(div().w(px(meter_theme.value_width))),
                    )
            })
    }

    /// Render separate LUFS panel
    pub fn render_lufs_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, loudness) = {
            let state = self.state.read(cx);
            (state.app.theme.clone(), state.app.loudness_info.clone())
        };
        div()
            .flex()
            .flex_col()
            .p_4()
            .bg(theme.background)
            .border_r_1()
            .border_color(theme.border)
            .child(self.render_lufs_with_true_peak(loudness.as_ref(), &theme))
    }
}

// ============================================================================
// App Level Meter Group Management
// ============================================================================

impl AppState {
    /// Update level meter groups based on current channel count from loudness info
    /// Creates a default stereo layout when no audio is playing
    pub fn update_level_meter_groups(&mut self) {
        self.level_meter_groups.clear();

        let num_channels = self
            .loudness_info
            .as_ref()
            .map(|l| l.channel_peaks.len())
            .unwrap_or(0);

        // Default to stereo (2 channels) when no audio is playing
        // This ensures meters are always visible with -60 dB default
        let num_channels = if num_channels == 0 { 2 } else { num_channels };

        // Standard channel layouts based on channel count
        match num_channels {
            1 => {
                // Mono
                self.level_meter_groups.push(ChannelGroup {
                    name: "Mono".to_string(),
                    channels: vec![ChannelInfo {
                        index: 0,
                        name: "M".to_string(),
                        display_name: vec!["M".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            2 => {
                // Stereo (2.0) - L and R are separate groups
                self.level_meter_groups.push(ChannelGroup {
                    name: "Left".to_string(),
                    channels: vec![ChannelInfo {
                        index: 0,
                        name: "L".to_string(),
                        display_name: vec!["L".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Right".to_string(),
                    channels: vec![ChannelInfo {
                        index: 1,
                        name: "R".to_string(),
                        display_name: vec!["R".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            3 => {
                // 2.1 - L, R, and LFE are separate groups
                self.level_meter_groups.push(ChannelGroup {
                    name: "Left".to_string(),
                    channels: vec![ChannelInfo {
                        index: 0,
                        name: "L".to_string(),
                        display_name: vec!["L".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Right".to_string(),
                    channels: vec![ChannelInfo {
                        index: 1,
                        name: "R".to_string(),
                        display_name: vec!["R".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "LFE".to_string(),
                    channels: vec![ChannelInfo {
                        index: 2,
                        name: "LFE".to_string(),
                        display_name: vec!["L".to_string(), "F".to_string(), "E".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            4 => {
                // Quad (FL, FR, SL, SR)
                self.level_meter_groups.push(ChannelGroup {
                    name: "Fronts".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "FL".to_string(),
                            display_name: vec!["F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "FR".to_string(),
                            display_name: vec!["F".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Surrounds".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 2,
                            name: "SL".to_string(),
                            display_name: vec!["S".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 3,
                            name: "SR".to_string(),
                            display_name: vec!["S".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            5 => {
                // 5.0 (FL, FR, FC, SL, SR) - Same as 5.1 without LFE
                self.level_meter_groups.push(ChannelGroup {
                    name: "Fronts".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "FL".to_string(),
                            display_name: vec!["F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "FR".to_string(),
                            display_name: vec!["F".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Center".to_string(),
                    channels: vec![ChannelInfo {
                        index: 2,
                        name: "C".to_string(),
                        display_name: vec!["C".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Surrounds".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 3,
                            name: "SL".to_string(),
                            display_name: vec!["S".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 4,
                            name: "SR".to_string(),
                            display_name: vec!["S".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            6 => {
                // 5.1 (FL, FR, FC, LFE, SL, SR)
                self.level_meter_groups.push(ChannelGroup {
                    name: "Fronts".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "FL".to_string(),
                            display_name: vec!["F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "FR".to_string(),
                            display_name: vec!["F".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Center".to_string(),
                    channels: vec![ChannelInfo {
                        index: 2,
                        name: "C".to_string(),
                        display_name: vec!["C".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "LFE".to_string(),
                    channels: vec![ChannelInfo {
                        index: 3,
                        name: "LFE".to_string(),
                        display_name: vec!["L".to_string(), "F".to_string(), "E".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Surrounds".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 4,
                            name: "SL".to_string(),
                            display_name: vec!["S".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 5,
                            name: "SR".to_string(),
                            display_name: vec!["S".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            8 => {
                // 7.1 (FL, FR, FC, LFE, SL, SR, BL, BR)
                self.level_meter_groups.push(ChannelGroup {
                    name: "Fronts".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "FL".to_string(),
                            display_name: vec!["F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "FR".to_string(),
                            display_name: vec!["F".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Center".to_string(),
                    channels: vec![ChannelInfo {
                        index: 2,
                        name: "C".to_string(),
                        display_name: vec!["C".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "LFE".to_string(),
                    channels: vec![ChannelInfo {
                        index: 3,
                        name: "LFE".to_string(),
                        display_name: vec!["L".to_string(), "F".to_string(), "E".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Surrounds".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 4,
                            name: "SL".to_string(),
                            display_name: vec!["S".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 5,
                            name: "SR".to_string(),
                            display_name: vec!["S".to_string(), "R".to_string()],
                        },
                        ChannelInfo {
                            index: 6,
                            name: "BL".to_string(),
                            display_name: vec!["B".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 7,
                            name: "BR".to_string(),
                            display_name: vec!["B".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            10 => {
                // 5.1.4 or 7.1.2 - Currently assuming 5.1.4
                // 5.1.4: FL, FR, FC, LFE, SL, SR, TFL, TFR, TBL, TBR
                // 7.1.2: FL, FR, FC, LFE, SL, SR, BL, BR, TML, TMR
                // TODO: Add configuration option to distinguish between these layouts
                self.level_meter_groups.push(ChannelGroup {
                    name: "Fronts".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "FL".to_string(),
                            display_name: vec!["F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "FR".to_string(),
                            display_name: vec!["F".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Center".to_string(),
                    channels: vec![ChannelInfo {
                        index: 2,
                        name: "C".to_string(),
                        display_name: vec!["C".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "LFE".to_string(),
                    channels: vec![ChannelInfo {
                        index: 3,
                        name: "LFE".to_string(),
                        display_name: vec!["L".to_string(), "F".to_string(), "E".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Surrounds".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 4,
                            name: "SL".to_string(),
                            display_name: vec!["S".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 5,
                            name: "SR".to_string(),
                            display_name: vec!["S".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Top".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 6,
                            name: "TFL".to_string(),
                            display_name: vec!["T".to_string(), "F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 7,
                            name: "TFR".to_string(),
                            display_name: vec!["T".to_string(), "F".to_string(), "R".to_string()],
                        },
                        ChannelInfo {
                            index: 8,
                            name: "TBL".to_string(),
                            display_name: vec!["T".to_string(), "B".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 9,
                            name: "TBR".to_string(),
                            display_name: vec!["T".to_string(), "B".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            14 => {
                // 9.1.4 (FL, FR, FC, LFE, SL, SR, BL, BR, FWL, FWR, TFL, TFR, TBL, TBR)
                self.level_meter_groups.push(ChannelGroup {
                    name: "Fronts".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "FL".to_string(),
                            display_name: vec!["F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "FR".to_string(),
                            display_name: vec!["F".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Center".to_string(),
                    channels: vec![ChannelInfo {
                        index: 2,
                        name: "C".to_string(),
                        display_name: vec!["C".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "LFE".to_string(),
                    channels: vec![ChannelInfo {
                        index: 3,
                        name: "LFE".to_string(),
                        display_name: vec!["L".to_string(), "F".to_string(), "E".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Sides".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 4,
                            name: "SL".to_string(),
                            display_name: vec!["S".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 5,
                            name: "SR".to_string(),
                            display_name: vec!["S".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Backs".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 6,
                            name: "BL".to_string(),
                            display_name: vec!["B".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 7,
                            name: "BR".to_string(),
                            display_name: vec!["B".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Wides".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 8,
                            name: "FWL".to_string(),
                            display_name: vec!["F".to_string(), "W".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 9,
                            name: "FWR".to_string(),
                            display_name: vec!["F".to_string(), "W".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Top".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 10,
                            name: "TFL".to_string(),
                            display_name: vec!["T".to_string(), "F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 11,
                            name: "TFR".to_string(),
                            display_name: vec!["T".to_string(), "F".to_string(), "R".to_string()],
                        },
                        ChannelInfo {
                            index: 12,
                            name: "TBL".to_string(),
                            display_name: vec!["T".to_string(), "B".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 13,
                            name: "TBR".to_string(),
                            display_name: vec!["T".to_string(), "B".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            16 => {
                // 9.1.6 (FL, FR, FC, LFE, SL, SR, BL, BR, FWL, FWR, TFL, TFR, TML, TMR, TBL, TBR)
                self.level_meter_groups.push(ChannelGroup {
                    name: "Fronts".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "FL".to_string(),
                            display_name: vec!["F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "FR".to_string(),
                            display_name: vec!["F".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Center".to_string(),
                    channels: vec![ChannelInfo {
                        index: 2,
                        name: "C".to_string(),
                        display_name: vec!["C".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "LFE".to_string(),
                    channels: vec![ChannelInfo {
                        index: 3,
                        name: "LFE".to_string(),
                        display_name: vec!["L".to_string(), "F".to_string(), "E".to_string()],
                    }],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Sides".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 4,
                            name: "SL".to_string(),
                            display_name: vec!["S".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 5,
                            name: "SR".to_string(),
                            display_name: vec!["S".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Backs".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 6,
                            name: "BL".to_string(),
                            display_name: vec!["B".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 7,
                            name: "BR".to_string(),
                            display_name: vec!["B".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Wides".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 8,
                            name: "FWL".to_string(),
                            display_name: vec!["F".to_string(), "W".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 9,
                            name: "FWR".to_string(),
                            display_name: vec!["F".to_string(), "W".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
                self.level_meter_groups.push(ChannelGroup {
                    name: "Top".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 10,
                            name: "TFL".to_string(),
                            display_name: vec!["T".to_string(), "F".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 11,
                            name: "TFR".to_string(),
                            display_name: vec!["T".to_string(), "F".to_string(), "R".to_string()],
                        },
                        ChannelInfo {
                            index: 12,
                            name: "TML".to_string(),
                            display_name: vec!["T".to_string(), "M".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 13,
                            name: "TMR".to_string(),
                            display_name: vec!["T".to_string(), "M".to_string(), "R".to_string()],
                        },
                        ChannelInfo {
                            index: 14,
                            name: "TBL".to_string(),
                            display_name: vec!["T".to_string(), "B".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 15,
                            name: "TBR".to_string(),
                            display_name: vec!["T".to_string(), "B".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
            _ => {
                // Generic fallback - treat all channels as one group
                let mut channels = Vec::new();
                for i in 0..num_channels {
                    channels.push(ChannelInfo {
                        index: i,
                        name: format!("CH{}", i + 1),
                        display_name: vec![format!("CH{}", i + 1)],
                    });
                }
                self.level_meter_groups.push(ChannelGroup {
                    name: "All Channels".to_string(),
                    channels,
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
        }

        // Update ChannelMuteSolo plugin to have correct number of channels
        self.update_channel_mute_solo_plugin();
    }

    /// Clear all mutes, solos, and dims in level meter groups
    pub fn clear_level_meter_mutes_and_solos(&mut self) {
        for group in &mut self.level_meter_groups {
            group.muted = false;
            group.soloed = false;
            group.dimmed = false;
        }
        self.update_channel_mute_solo_plugin();
    }

    /// Toggle mute for the selected level meter group
    pub fn toggle_level_meter_mute(&mut self) {
        if let Some(group) = self
            .level_meter_groups
            .get_mut(self.selected_level_meter_group)
        {
            group.muted = !group.muted;
            self.update_channel_mute_solo_plugin();
        }
    }

    /// Toggle solo for the selected level meter group
    pub fn toggle_level_meter_solo(&mut self) {
        if let Some(group) = self
            .level_meter_groups
            .get_mut(self.selected_level_meter_group)
        {
            let is_currently_soloed = group.soloed;

            // Solo behavior: only one group can be soloed at a time
            // When soloing, set soloed=true on selected group, soloed=false on all others
            // When un-soloing, set soloed=false on selected group
            for (idx, g) in self.level_meter_groups.iter_mut().enumerate() {
                if idx == self.selected_level_meter_group {
                    g.soloed = !is_currently_soloed;
                } else {
                    g.soloed = false;
                }
            }

            self.update_channel_mute_solo_plugin();
        }
    }

    /// Toggle dim for the selected level meter group
    pub fn toggle_level_meter_dim(&mut self) {
        if let Some(group) = self
            .level_meter_groups
            .get_mut(self.selected_level_meter_group)
        {
            group.dimmed = !group.dimmed;
            self.update_channel_mute_solo_plugin();
        }
    }

    /// Update the ChannelMuteSolo plugin based on current level meter group states
    fn update_channel_mute_solo_plugin(&mut self) {
        // Calculate total channel count
        let num_channels: usize = self
            .level_meter_groups
            .iter()
            .map(|g| g.channels.len())
            .sum();

        if num_channels == 0 {
            return;
        }

        // Build per-channel states from groups
        let mut channel_states = vec![
            ChannelState {
                muted: false,
                soloed: false,
                dimmed: false
            };
            num_channels
        ];

        for group in &self.level_meter_groups {
            for channel_info in &group.channels {
                if channel_info.index < num_channels {
                    channel_states[channel_info.index] = ChannelState {
                        muted: group.muted,
                        soloed: group.soloed,
                        dimmed: group.dimmed,
                    };
                }
            }
        }

        // Determine if any channel is muted, soloed, or dimmed
        let enabled = channel_states
            .iter()
            .any(|s| s.muted || s.soloed || s.dimmed);

        // Find and update the ChannelMuteSolo plugin
        for i in 0..self.plugin_chain.len() {
            if let Some(plugin) = self.plugin_chain.get_plugin_mut(i) {
                if matches!(&plugin.settings, PluginSettings::ChannelMuteSolo { .. }) {
                    // Update settings in memory
                    plugin.settings = PluginSettings::ChannelMuteSolo {
                        enabled,
                        channel_states: channel_states.clone(),
                    };
                    // Flag that plugins need updating
                    self.needs_plugin_update = true;
                    return;
                }
            }
        }
    }

    /// Navigate to next level meter group
    pub fn select_next_level_meter_group(&mut self) {
        if !self.level_meter_groups.is_empty() {
            self.selected_level_meter_group =
                (self.selected_level_meter_group + 1) % self.level_meter_groups.len();
        }
    }

    /// Navigate to previous level meter group
    pub fn select_previous_level_meter_group(&mut self) {
        if !self.level_meter_groups.is_empty() {
            if self.selected_level_meter_group == 0 {
                self.selected_level_meter_group = self.level_meter_groups.len() - 1;
            } else {
                self.selected_level_meter_group -= 1;
            }
        }
    }

    /// Navigate between mute, solo, and dim controls
    pub fn select_next_level_meter_control(&mut self) {
        self.level_meter_control_selection = (self.level_meter_control_selection + 1) % 3;
    }

    /// Navigate between mute, solo, and dim controls (previous)
    pub fn select_previous_level_meter_control(&mut self) {
        self.level_meter_control_selection = if self.level_meter_control_selection == 0 {
            2
        } else {
            self.level_meter_control_selection - 1
        };
    }

    /// Toggle the currently selected level meter control (mute/solo/dim)
    pub fn toggle_selected_level_meter_control(&mut self) {
        match self.level_meter_control_selection {
            0 => self.toggle_level_meter_mute(),
            1 => self.toggle_level_meter_solo(),
            2 => self.toggle_level_meter_dim(),
            _ => {}
        }
    }
}
