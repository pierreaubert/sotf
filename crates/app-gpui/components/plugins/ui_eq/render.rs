// intentional-file: fixed pixel values here are graph and plugin control geometry.
use super::super::common::{render_knob_sized, render_midi_badge, render_midi_page_indicator};
use super::calculate::calculate_band_response;
use super::calculate::calculate_dynamic_y_range;
use super::calculate::calculate_plot_width_without_legend;
use super::calculate::calculate_response_at_freq;
use super::consts::BAND_COLOR_FALLBACK;
use super::consts::CHART_BOTTOM_MARGIN;
use super::consts::CHART_HEIGHT;
use super::consts::CONTROL_POINT_RADIUS;
use super::consts::MAX_FREQ;
use super::consts::MIN_FREQ;
use super::consts::Q_BAR_HEIGHT;
use super::consts::Q_HANDLE_RADIUS;
use super::consts::freq_to_x;
use super::consts::gain_to_y_with_height;
use super::consts::q_to_bar_width;
use super::consts::x_to_freq;
use super::consts::y_to_gain_with_height;
use super::eq_chart_wrapper::EqChartWrapper;
use super::eq_control_point_drag::EqControlPointDrag;
use super::eq_qhandle_drag::EqQHandleDrag;
use super::get::get_channel_name;
use super::get::get_filter_type_index;
use super::misc::drag_delta_to_q_change;
use super::types::{EqCompactLayout, EqRenderState, EqViewMode};
use crate::app::AppState;
use crate::components::design::Ds;
use crate::components::graphs::common::rgba_to_u32;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_audio_kit::PotentiometerSize;
use gpui_px::{ChartTheme, ScaleType, line};
use math_audio_iir_fir::BiquadFilterType;
use sotf_audio_player::{EQFilter, PluginSettings};
use sotf_audio_player_midi::mapping::MidiOverlay;
use sotf_plugins::param_specs::{
    eq::BAND_TEMPLATE as EQ, eq::GLOBAL_PARAMS as EQ_GLOBAL, find_by_key as pk,
    fir_designer::PARAMS as FIR_PARAMS, linear_phase_eq::PARAMS as LP_PARAMS,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy)]
pub(crate) struct EqBandIndexing {
    pub(crate) stride: usize,
    pub(crate) frequency: usize,
    pub(crate) q: usize,
    pub(crate) gain: usize,
    pub(crate) filter_type: usize,
    pub(crate) active: Option<usize>,
}

impl EqBandIndexing {
    const STANDARD: Self = Self {
        stride: 4,
        frequency: 0,
        q: 1,
        gain: 2,
        filter_type: 3,
        active: None,
    };
    const FIR: Self = Self {
        stride: 5,
        frequency: 1,
        q: 2,
        gain: 3,
        filter_type: 0,
        active: Some(4),
    };

    fn param(self, band_idx: usize, local_idx: usize) -> usize {
        band_idx * self.stride + local_idx
    }
}

#[derive(Clone, Copy)]
pub(crate) enum EqGlobalControl {
    StandardMaxFilters,
    StandardTdf2,
    StandardTopology,
    LpNumFilters,
    LpFirLength,
    LpAutoGain,
    LpMix,
    FirNumFilters,
    FirLength,
    FirPhaseMode,
    FirAutoGain,
    FirMix,
}

fn format_eq_frequency_label(freq: f64) -> String {
    if freq >= 10_000.0 {
        format!("{:.0}k", freq / 1_000.0)
    } else if freq >= 1_000.0 {
        format!("{:.1}k", freq / 1_000.0)
    } else {
        format!("{freq:.0}")
    }
}

fn eq_frequency_points() -> &'static [f64] {
    static EQ_FREQUENCY_POINTS: OnceLock<Vec<f64>> = OnceLock::new();
    EQ_FREQUENCY_POINTS
        .get_or_init(|| {
            const NUM_POINTS: usize = 240;
            let min_freq = 20.0_f64;
            let max_freq = 20000.0_f64;
            let log_min = min_freq.ln();
            let log_max = max_freq.ln();

            (0..NUM_POINTS)
                .map(|i| {
                    let t = i as f64 / (NUM_POINTS - 1) as f64;
                    (log_min + t * (log_max - log_min)).exp()
                })
                .collect()
        })
        .as_slice()
}

#[derive(Clone)]
struct EqCurveRenderData {
    combined_response: Vec<f64>,
    band_responses: Vec<Vec<f64>>,
}

struct EqCurveRenderCache {
    signature: String,
    data: EqCurveRenderData,
}

impl EqCurveRenderCache {
    fn empty() -> Self {
        Self {
            signature: String::new(),
            data: EqCurveRenderData {
                combined_response: Vec::new(),
                band_responses: Vec::new(),
            },
        }
    }

    fn get_or_build(&mut self, filters: &[EQFilter], freq_points: &[f64]) -> EqCurveRenderData {
        let signature = eq_curve_signature(filters, freq_points.len());
        if self.signature != signature {
            self.data = EqCurveRenderData {
                combined_response: freq_points
                    .iter()
                    .map(|&freq| calculate_response_at_freq(filters, freq))
                    .collect(),
                band_responses: filters
                    .iter()
                    .map(|filter| {
                        freq_points
                            .iter()
                            .map(|&freq| calculate_band_response(filter, freq))
                            .collect()
                    })
                    .collect(),
            };
            self.signature = signature;
        }
        self.data.clone()
    }
}

fn eq_curve_cache() -> &'static Mutex<EqCurveRenderCache> {
    static EQ_CURVE_RENDER_CACHE: OnceLock<Mutex<EqCurveRenderCache>> = OnceLock::new();
    EQ_CURVE_RENDER_CACHE.get_or_init(|| Mutex::new(EqCurveRenderCache::empty()))
}

fn eq_curve_signature(filters: &[EQFilter], freq_count: usize) -> String {
    let mut signature = String::with_capacity(filters.len() * 64);
    signature.push_str(&freq_count.to_string());
    for filter in filters {
        use std::fmt::Write;
        let _ = write!(
            signature,
            "|{:?}:{:x}:{:x}:{:x}:{}:{}:{:?}:{}",
            filter.filter_type,
            filter.frequency.to_bits(),
            filter.q.to_bits(),
            filter.gain_db.to_bits(),
            filter.muted,
            filter.solo,
            filter.topology,
            filter
                .lambda
                .map(|lambda| lambda.to_bits().to_string())
                .unwrap_or_default()
        );
        for section in &filter.kautz_sections {
            let _ = write!(
                signature,
                ":k{:x},{:x},{:x}",
                section.pole_freq.to_bits(),
                section.q.to_bits(),
                section.gain.to_bits()
            );
        }
    }
    signature
}

fn render_band_frequency_guide(
    freq: f64,
    x: f32,
    color: Rgba,
    selected: bool,
    chart_height: f32,
    theme: &Theme,
) -> Vec<AnyElement> {
    const LABEL_WIDTH: f32 = 56.0;
    const LABEL_TOP: f32 = 2.0;
    const GUIDE_TOP: f32 = 24.0;
    const DASH_HEIGHT: f32 = 4.0;
    const DASH_GAP: f32 = 4.0;

    let label = format_eq_frequency_label(freq);
    let guide_height = chart_height - CHART_BOTTOM_MARGIN - GUIDE_TOP;
    let dash_count = (guide_height / (DASH_HEIGHT + DASH_GAP)).floor().max(1.0) as usize;
    let guide_color = Rgba {
        a: if selected { 0.6 } else { 0.35 },
        ..color
    };
    let label_bg = Rgba {
        a: if selected { 0.9 } else { 0.72 },
        ..theme.plugin_palette.eq_curve_colors.background
    };

    let label = div()
        .absolute()
        .left(px(x - LABEL_WIDTH / 2.0))
        .top(px(LABEL_TOP))
        .w(px(LABEL_WIDTH))
        .text_center()
        .px(px(3.0))
        .py(px(1.0))
        .rounded(px(4.0))
        .bg(label_bg)
        .text_size(px(10.0))
        .font_weight(if selected {
            FontWeight::BOLD
        } else {
            FontWeight::SEMIBOLD
        })
        .text_color(if selected { theme.text_primary } else { color })
        .child(label)
        .into_any_element();

    let guide = div()
        .absolute()
        .left(px(x))
        .top(px(GUIDE_TOP))
        .w(px(1.0))
        .h(px(guide_height))
        .flex()
        .flex_col()
        .gap(px(DASH_GAP))
        .children((0..dash_count).map(move |_| {
            div()
                .w(px(1.0))
                .h(px(DASH_HEIGHT))
                .bg(guide_color)
                .into_any_element()
        }))
        .into_any_element();

    vec![guide, label]
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_eq_channel_toolbar(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    channels: usize,
    selected_channel: usize,
    per_channel_mode: bool,
    theme: &Theme,
) -> AnyElement {
    let all_entity = entity.clone();
    let per_entity = entity.clone();

    div()
        .w_full()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_center()
        .gap(d.gap)
        .px(d.pad_x)
        .py(d.pad_y_half)
        .child(render_eq_mode_button(
            d,
            "All",
            !per_channel_mode,
            theme,
            move |_, _, cx| {
                all_entity.update(cx, |state, cx| {
                    state.app.set_eq_per_channel_mode(plugin_idx, false);
                    cx.notify();
                });
            },
        ))
        .child(render_eq_mode_button(
            d,
            "Per Ch",
            per_channel_mode,
            theme,
            move |_, _, cx| {
                per_entity.update(cx, |state, cx| {
                    state.app.set_eq_per_channel_mode(plugin_idx, true);
                    cx.notify();
                });
            },
        ))
        .when(per_channel_mode, |row| {
            row.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(d.grid)
                    .children((0..channels).map(|ch| {
                        let entity = entity.clone();
                        render_eq_mode_button(
                            d,
                            get_channel_name(ch, channels),
                            selected_channel == ch,
                            theme,
                            move |_, _, cx| {
                                entity.update(cx, |state, cx| {
                                    state.app.plugin_state.selected_eq_channel = ch;
                                    cx.notify();
                                });
                            },
                        )
                    })),
            )
        })
        .into_any_element()
}

fn render_eq_mode_button<F>(
    d: &Ds,
    label: impl Into<SharedString>,
    selected: bool,
    theme: &Theme,
    on_click: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    div()
        .px(d.pad_y)
        .py(d.pad_y_half)
        .text_size(d.text_xs)
        .font_weight(FontWeight::SEMIBOLD)
        .rounded(d.r_sm)
        .cursor_pointer()
        .when(selected, |el| {
            el.bg(theme.accent).text_color(theme.text_on_accent)
        })
        .when(!selected, |el| {
            el.bg(theme.background_secondary)
                .text_color(theme.text_secondary)
                .hover(|s| s.bg(theme.surface_hover))
        })
        .on_mouse_down(MouseButton::Left, on_click)
        .child(label.into())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_eq_property_strip(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    filter: Option<&EQFilter>,
    band_idx: usize,
    indexing: EqBandIndexing,
    state: &EqRenderState,
    is_lp_mode: bool,
    theme: &Theme,
) -> AnyElement {
    let Some(filter) = filter else {
        return div()
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .py(d.pad_y)
            .text_size(d.text_sm)
            .text_color(theme.text_muted)
            .child("No bands")
            .into_any_element();
    };

    let base_param_idx = band_idx * indexing.stride;
    let midi_overlay = state.midi_overlay;
    let mute_entity = entity.clone();
    let solo_entity = entity.clone();
    let add_entity = entity.clone();

    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap(d.gap)
        .px(d.pad_x)
        .py(d.pad_y)
        .bg(theme.surface)
        .rounded(d.r_md)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(d.grid)
                .min_w(rems(10.0))
                .child(
                    div()
                        .text_size(d.text_xs)
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.text_primary)
                        .child(format!(
                            "#{} {}",
                            band_idx + 1,
                            filter.filter_type.short_name()
                        )),
                )
                .child(
                    div()
                        .text_size(d.text_xs)
                        .text_color(theme.text_muted)
                        .child(format!(
                            "{:.0} Hz  {:+.1} dB  Q {:.2}",
                            filter.frequency, filter.gain_db, filter.q
                        )),
                ),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(d.grid)
                .child(render_filter_type_selector(
                    d,
                    entity.clone(),
                    plugin_idx,
                    &filter.filter_type,
                    band_idx,
                    base_param_idx + indexing.filter_type,
                    None,
                    theme,
                ))
                .when(!is_lp_mode, |row| {
                    row.child(render_standard_eq_algorithm_pill(d, state.topology, theme))
                }),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_center()
                .gap(d.gap)
                .child(render_eq_knob_with_midi(
                    d,
                    entity.clone(),
                    plugin_idx,
                    "Freq",
                    filter.frequency,
                    pk(EQ, "freq").min_f64(),
                    pk(EQ, "freq").max_f64(),
                    "Hz",
                    base_param_idx + indexing.frequency,
                    state.selected_param,
                    state.is_editing,
                    midi_overlay,
                    theme,
                ))
                .child(render_eq_knob_with_midi(
                    d,
                    entity.clone(),
                    plugin_idx,
                    "Gain",
                    filter.gain_db,
                    pk(EQ, "gain").min_f64(),
                    pk(EQ, "gain").max_f64(),
                    "dB",
                    base_param_idx + indexing.gain,
                    state.selected_param,
                    state.is_editing,
                    midi_overlay,
                    theme,
                ))
                .child(render_eq_knob_with_midi(
                    d,
                    entity.clone(),
                    plugin_idx,
                    "Q",
                    filter.q,
                    pk(EQ, "q").min_f64(),
                    pk(EQ, "q").max_f64(),
                    "",
                    base_param_idx + indexing.q,
                    state.selected_param,
                    state.is_editing,
                    midi_overlay,
                    theme,
                ))
                .children(indexing.active.map(|active_local_idx| {
                    render_eq_active_toggle(
                        d,
                        entity.clone(),
                        plugin_idx,
                        filter,
                        base_param_idx + active_local_idx,
                        state.selected_param,
                        state.is_editing,
                        theme,
                    )
                })),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(d.grid)
                .child(render_eq_action_button(
                    d,
                    "M",
                    filter.muted,
                    theme.error,
                    theme,
                    move |_, _, cx| {
                        mute_entity.update(cx, |state, cx| {
                            state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                            if let Err(e) = state.app.toggle_eq_band_mute(band_idx) {
                                log::warn!("Failed to toggle EQ band mute: {}", e);
                            }
                            cx.notify();
                        });
                    },
                ))
                .child(render_eq_action_button(
                    d,
                    "S",
                    filter.solo,
                    theme.success,
                    theme,
                    move |_, _, cx| {
                        solo_entity.update(cx, |state, cx| {
                            state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                            if let Err(e) = state.app.toggle_eq_band_solo(band_idx) {
                                log::warn!("Failed to toggle EQ band solo: {}", e);
                            }
                            cx.notify();
                        });
                    },
                ))
                .child(render_eq_add_button(d, add_entity, plugin_idx, theme)),
        )
        .into_any_element()
}

fn render_eq_action_button<F>(
    d: &Ds,
    label: &'static str,
    active: bool,
    active_color: Rgba,
    theme: &Theme,
    on_click: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    div()
        .px(d.pad_y)
        .py(d.pad_y_half)
        .text_size(d.text_xs)
        .font_weight(FontWeight::BOLD)
        .rounded(d.r_sm)
        .cursor_pointer()
        .bg(if active {
            active_color
        } else {
            theme.background_secondary
        })
        .text_color(if active {
            theme.text_on_accent
        } else {
            theme.text_secondary
        })
        .border_1()
        .border_color(if active { active_color } else { theme.border })
        .hover(|s| {
            s.bg(if active {
                active_color
            } else {
                theme.surface_hover
            })
        })
        .on_mouse_down(MouseButton::Left, on_click)
        .child(label)
}

fn render_eq_add_button(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .id("eq-add-band")
        .key_context("plugin-control")
        .px(d.pad_y)
        .py(d.pad_y_half)
        .text_size(d.text_xs)
        .font_weight(FontWeight::BOLD)
        .rounded(d.r_sm)
        .cursor_pointer()
        .bg(theme.success)
        .text_color(theme.text_on_accent)
        .hover(|s| s.opacity(0.8))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            entity.update(cx, |state, cx| {
                state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                if let Err(e) = state.app.add_eq_band() {
                    log::warn!("Failed to add EQ band: {}", e);
                }
                cx.notify();
            });
        })
        .child("+")
}

/// Render EQ frequency response using gpui-px with draggable control points
///
/// Shows all filter bands overlaid on a single plot with log frequency axis
pub(crate) fn render_eq_visualization(
    entity: Entity<AppState>,
    plugin_idx: usize,
    filters: &[EQFilter],
    selected_band: Option<usize>,
    indexing: EqBandIndexing,
    theme: &Theme,
    width: f32,
) -> impl IntoElement {
    render_eq_visualization_sized(
        entity,
        plugin_idx,
        filters,
        selected_band,
        indexing,
        theme,
        width,
        CHART_HEIGHT,
    )
}

/// Render EQ frequency response using gpui-px with a caller-supplied height.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_eq_visualization_sized(
    entity: Entity<AppState>,
    plugin_idx: usize,
    filters: &[EQFilter],
    selected_band: Option<usize>,
    indexing: EqBandIndexing,
    theme: &Theme,
    width: f32,
    chart_height: f32,
) -> impl IntoElement {
    // Calculate dynamic y-axis range based on filter gains
    let (min_db, max_db) = calculate_dynamic_y_range(filters);

    let freq_points = eq_frequency_points();

    let curve_data = {
        let mut cache = eq_curve_cache()
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        cache.get_or_build(filters, freq_points)
    };

    // Create chart theme from app theme
    let chart_theme = ChartTheme {
        plot_background: theme.plugin_palette.eq_curve_colors.background,
        grid_color: theme.plugin_palette.eq_curve_colors.grid,
        axis_line_color: theme.plugin_palette.graph_colors.grid,
        axis_label_color: theme.text_secondary,
        title_color: theme.text_primary,
        legend_text_color: theme.text_secondary,
    };

    // Match gpui-px chart margins with the legend hidden. Band identity is
    // shown in the graph overlay instead of a right-side legend column.
    let plot_width = calculate_plot_width_without_legend(width);

    // Convert combined line color to u32
    let text_muted_u32 = {
        let c = theme.text_muted;
        ((c.r * 255.0) as u32) << 16 | ((c.g * 255.0) as u32) << 8 | (c.b * 255.0) as u32
    };
    let mut chart_builder = line(freq_points, &curve_data.combined_response)
        .x_scale(ScaleType::Log)
        .y_scale(ScaleType::Linear)
        .x_label("Frequency (Hz)")
        .y_label("dB (SPL)")
        .x_range(MIN_FREQ, MAX_FREQ)
        .y_range(min_db, max_db) // Dynamic Y range based on filter gains
        .size(width, chart_height)
        .color(text_muted_u32) // Combined response line
        .stroke_width(2.5)
        .theme(chart_theme);

    // Add each filter band as an additional series
    for (i, filter) in filters.iter().enumerate() {
        let color = theme
            .plugin_palette
            .band_colors
            .get(i)
            .map(|c| rgba_to_u32(*c))
            .unwrap_or(BAND_COLOR_FALLBACK);
        let is_selected = selected_band == Some(i);
        let is_muted = filter.muted;
        let is_soloed = filter.solo;
        let any_soloed = filters.iter().any(|f| f.solo);
        let effective_muted = is_muted || (any_soloed && !is_soloed);
        let opacity = if is_selected { 1.0 } else { 0.5 };
        let stroke = if is_selected { 2.0 } else { 1.5 };
        let opacity = if effective_muted { 0.2 } else { opacity };

        chart_builder = chart_builder.add_series(
            &curve_data.band_responses[i],
            Option::<String>::None,
            color,
            stroke,
            opacity,
        );
    }

    // Build the chart element
    let chart_element = match chart_builder.build() {
        Ok(chart) => chart.into_any_element(),
        Err(_) => div()
            .w(px(width))
            .h(px(chart_height))
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.plugin_palette.eq_curve_colors.background)
            .text_color(theme.text_secondary)
            .child("Unable to render chart")
            .into_any_element(),
    };

    // Create control points for each filter
    let mut control_points: Vec<AnyElement> = Vec::new();
    // Shared bounds reference for drag handlers
    let bounds_ref = Rc::new(RefCell::new(None::<Bounds<Pixels>>));

    for (i, filter) in filters.iter().enumerate() {
        let is_selected = selected_band == Some(i);
        let rgba_color = theme
            .plugin_palette
            .band_colors
            .get(i)
            .copied()
            .unwrap_or(gpui::rgba(BAND_COLOR_FALLBACK * 256 + 0xFF));
        let color = rgba_to_u32(rgba_color);

        // Calculate position
        let x = freq_to_x(filter.frequency, plot_width);
        let y = gain_to_y_with_height(filter.gain_db, min_db, max_db, chart_height);

        let band_idx = i;

        control_points.extend(render_band_frequency_guide(
            filter.frequency,
            x,
            rgba_color,
            is_selected,
            chart_height,
            theme,
        ));

        // Control point circle
        let border_color = if is_selected {
            theme.text_primary
        } else {
            Rgba {
                a: 0.5,
                ..theme.text_primary
            }
        };

        // Calculate Q bar width
        let bar_width = q_to_bar_width(filter.q);
        let bar_half_width = bar_width / 2.0;

        // Q bar (horizontal line through control point)
        let q_bar = div()
            .absolute()
            .left(px(x - bar_half_width))
            .top(px(y - Q_BAR_HEIGHT / 2.0))
            .w(px(bar_width))
            .h(px(Q_BAR_HEIGHT))
            .bg(rgba_color)
            .rounded(px(Q_BAR_HEIGHT / 2.0))
            .opacity(if is_selected { 0.8 } else { 0.5 })
            .into_any_element();

        control_points.push(q_bar);

        // Left Q handle (decrease Q when dragged left)
        let left_handle = {
            let entity_left = entity.clone();
            let current_q = filter.q;
            let bounds_ref = bounds_ref.clone();
            div()
                .id(("eq-q-left", i))
                .absolute()
                .left(px(x - bar_half_width - Q_HANDLE_RADIUS))
                .top(px(y - Q_HANDLE_RADIUS))
                .w(px(Q_HANDLE_RADIUS * 2.0))
                .h(px(Q_HANDLE_RADIUS * 2.0))
                .rounded_full()
                .bg(rgba_color)
                .border(px(1.0))
                .border_color(if is_selected {
                    theme.text_primary
                } else {
                    Rgba {
                        a: 0.4,
                        ..theme.text_primary
                    }
                })
                .cursor(gpui::CursorStyle::ResizeLeftRight)
                .hover(|s| s.size(px(Q_HANDLE_RADIUS * 2.5)))
                .on_drag(
                    EqQHandleDrag {
                        band_idx,
                        plugin_idx,
                        is_right_handle: false,
                        start_x: x - bar_half_width,
                        start_q: current_q,
                        color,
                    },
                    |drag, _, _, cx| {
                        cx.stop_propagation();
                        cx.new(|_| drag.clone())
                    },
                )
                .on_drag_move::<EqQHandleDrag>({
                    move |event, _window, cx| {
                        let bounds = if let Some(b) = *bounds_ref.borrow() {
                            b
                        } else {
                            return;
                        };
                        let drag_data = event.drag(cx);
                        let position = event.event.position;
                        // Convert global mouse X to local chart coordinate
                        let x_px: f32 = (position.x - bounds.origin.x).into();

                        // For left handle: moving left decreases Q, moving right increases Q
                        // drag_data.start_x is in local coordinates
                        let delta = drag_data.start_x - x_px;
                        let q_change = drag_delta_to_q_change(delta);
                        let new_q = (drag_data.start_q + q_change)
                            .clamp(pk(EQ, "q").min_f64(), pk(EQ, "q").max_f64());

                        let plugin_idx = drag_data.plugin_idx;
                        let band_idx = drag_data.band_idx;

                        entity_left.update(cx, |state, cx| {
                            state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                            state.app.set_plugin_param(
                                plugin_idx,
                                indexing.param(band_idx, indexing.q),
                                new_q,
                            );
                            cx.notify();
                        });
                        // window.refresh(); // Not needed with cx.notify()
                    }
                })
                .into_any_element()
        };

        control_points.push(left_handle);

        // Right Q handle (increase Q when dragged right)
        let right_handle = {
            let entity_right = entity.clone();
            let current_q = filter.q;
            let bounds_ref = bounds_ref.clone();
            div()
                .id(("eq-q-right", i))
                .absolute()
                .left(px(x + bar_half_width - Q_HANDLE_RADIUS))
                .top(px(y - Q_HANDLE_RADIUS))
                .w(px(Q_HANDLE_RADIUS * 2.0))
                .h(px(Q_HANDLE_RADIUS * 2.0))
                .rounded_full()
                .bg(rgba_color)
                .border(px(1.0))
                .border_color(if is_selected {
                    theme.text_primary
                } else {
                    Rgba {
                        a: 0.4,
                        ..theme.text_primary
                    }
                })
                .cursor(gpui::CursorStyle::ResizeLeftRight)
                .hover(|s| s.size(px(Q_HANDLE_RADIUS * 2.5)))
                .on_drag(
                    EqQHandleDrag {
                        band_idx,
                        plugin_idx,
                        is_right_handle: true,
                        start_x: x + bar_half_width,
                        start_q: current_q,
                        color,
                    },
                    |drag, _, _, cx| {
                        cx.stop_propagation();
                        cx.new(|_| drag.clone())
                    },
                )
                .on_drag_move::<EqQHandleDrag>({
                    move |event, _window, cx| {
                        let bounds = if let Some(b) = *bounds_ref.borrow() {
                            b
                        } else {
                            return;
                        };
                        let drag_data = event.drag(cx);
                        let position = event.event.position;
                        // Convert global mouse X to local chart coordinate
                        let x_px: f32 = (position.x - bounds.origin.x).into();

                        // For right handle: moving right increases Q, moving left decreases Q
                        let delta = x_px - drag_data.start_x;
                        let q_change = drag_delta_to_q_change(delta);
                        let new_q = (drag_data.start_q + q_change)
                            .clamp(pk(EQ, "q").min_f64(), pk(EQ, "q").max_f64());

                        let plugin_idx = drag_data.plugin_idx;
                        let band_idx = drag_data.band_idx;

                        entity_right.update(cx, |state, cx| {
                            state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                            state.app.set_plugin_param(
                                plugin_idx,
                                indexing.param(band_idx, indexing.q),
                                new_q,
                            );
                            cx.notify();
                        });
                        // window.refresh();
                    }
                })
                .into_any_element()
        };

        control_points.push(right_handle);

        // Main control point circle (rendered on top)
        let control_point = div()
            .id(("eq-control-point", i))
            .absolute()
            .left(px(x - CONTROL_POINT_RADIUS))
            .top(px(y - CONTROL_POINT_RADIUS))
            .w(px(CONTROL_POINT_RADIUS * 2.0))
            .h(px(CONTROL_POINT_RADIUS * 2.0))
            .rounded_full()
            .bg(rgba_color)
            .border(px(2.0))
            .border_color(border_color)
            .shadow_md()
            .cursor(gpui::CursorStyle::PointingHand)
            .hover(|s| s.size(px(CONTROL_POINT_RADIUS * 2.5)))
            .on_mouse_down(MouseButton::Left, {
                let entity_click = entity.clone();
                move |event, _window, cx| {
                    if event.click_count >= 2 {
                        // Double-click: reset band to default values
                        entity_click.update(cx, |state, cx| {
                            state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                            state.app.plugin_state.selected_eq_band = band_idx;
                            state.app.set_plugin_param(
                                plugin_idx,
                                indexing.param(band_idx, indexing.frequency),
                                pk(EQ, "freq").default_f64(),
                            );
                            state.app.set_plugin_param(
                                plugin_idx,
                                indexing.param(band_idx, indexing.q),
                                pk(EQ, "q").default_f64(),
                            );
                            state.app.set_plugin_param(
                                plugin_idx,
                                indexing.param(band_idx, indexing.gain),
                                pk(EQ, "gain").default_f64(),
                            );
                            cx.notify();
                        });
                    } else {
                        // Single click: select this band
                        entity_click.update(cx, |state, _| {
                            state.app.plugin_state.selected_eq_band = band_idx;
                        });
                    }
                }
            })
            .on_drag(
                EqControlPointDrag {
                    band_idx,
                    plugin_idx,
                    color,
                    start_freq: filter.frequency,
                    start_gain: filter.gain_db,
                    start_x: x,
                    start_y: y,
                },
                |drag, _, _, cx| {
                    cx.stop_propagation();
                    cx.new(|_| drag.clone())
                },
            )
            .into_any_element();

        control_points.push(control_point);
    }

    // Wrap chart and control points in a relative container
    // The on_drag_move handler is on the container so it receives events
    // even when the cursor moves away from the small control point circle
    let container = div()
        .id("eq-chart-container")
        .relative()
        .w(px(width))
        .h(px(chart_height))
        .child(chart_element)
        .children(control_points)
        .on_drag_move::<EqControlPointDrag>({
            let entity = entity.clone();
            let bounds_ref = bounds_ref.clone();
            move |event, _window, cx| {
                let bounds = if let Some(b) = *bounds_ref.borrow() {
                    b
                } else {
                    return;
                };
                let drag_data = event.drag(cx);
                // Position is relative to this container div, which IS the chart area
                let position = event.event.position;

                // Convert global mouse coordinates to local chart coordinates
                let x_px: f32 = (position.x - bounds.origin.x).into();
                let y_px: f32 = (position.y - bounds.origin.y).into();

                // Convert directly to freq/gain (no delta calculation needed)
                // Use wider range for dragging to allow extending beyond current view
                let new_freq = x_to_freq(x_px, plot_width).clamp(MIN_FREQ, MAX_FREQ);
                let new_gain =
                    y_to_gain_with_height(y_px, min_db, max_db, chart_height).clamp(-24.0, 24.0);

                let plugin_idx = drag_data.plugin_idx;
                let band_idx = drag_data.band_idx;

                entity.update(cx, |state, cx| {
                    state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                    state.app.set_plugin_param(
                        plugin_idx,
                        indexing.param(band_idx, indexing.frequency),
                        new_freq,
                    );
                    state.app.set_plugin_param(
                        plugin_idx,
                        indexing.param(band_idx, indexing.gain),
                        new_gain,
                    );
                    cx.notify();
                });
                // window.refresh();
            }
        });

    EqChartWrapper::new(container.into_any_element(), bounds_ref).into_any_element()
}

/// Render a knob with an optional MIDI badge underneath
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_eq_knob_with_midi(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    label: &str,
    value: f64,
    min: f64,
    max: f64,
    unit: &str,
    param_idx: usize,
    selected_param: usize,
    is_editing: bool,
    midi_overlay: Option<&MidiOverlay>,
    theme: &Theme,
) -> impl IntoElement {
    let midi_assignment = midi_overlay.and_then(|o| o.assignments.get(&param_idx));

    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(d.grid)
        .child(render_knob_sized(
            entity,
            plugin_idx,
            label,
            value,
            min,
            max,
            unit,
            param_idx,
            selected_param,
            is_editing,
            None,
            PotentiometerSize::Xs,
            theme,
        ))
        .children(midi_assignment.map(|assignment| render_midi_badge(d, assignment, theme)))
}

/// Render the EQ plugin with graphical visualization
pub fn render_eq_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: EqRenderState,
    theme: &Theme,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    let ds = Ds::from_cx(cx);

    // Read selected channel from AppState
    let app_state = entity.read(cx);
    let selected_eq_channel = app_state.app.plugin_state.selected_eq_channel;
    let _ = app_state;

    // Determine which filters to display based on mode
    let display_filters: &[EQFilter] = if state.per_channel_mode {
        // Per-channel mode: get filters for selected channel
        if let Some(ch_filters) = state.channel_filters {
            let ch_idx = selected_eq_channel.min(ch_filters.len().saturating_sub(1));
            if ch_idx < ch_filters.len() {
                &ch_filters[ch_idx]
            } else {
                // Fallback to global filters
                state.filters
            }
        } else {
            // No channel filters available, fall back to global
            state.filters
        }
    } else {
        // Global mode: use the global filters
        state.filters
    };

    // Clamp selected band to valid range
    let selected_band_idx = state
        .selected_band_idx
        .min(display_filters.len().saturating_sub(1));
    let num_bands = display_filters.len();

    // Get the selected filter
    let selected_filter = if num_bands > 0 {
        Some(&display_filters[selected_band_idx])
    } else {
        None
    };
    let is_fir_mode = matches!(&state.mode, EqViewMode::FirDesigner { .. });
    let is_lp_mode = matches!(
        &state.mode,
        EqViewMode::LinearPhase { .. } | EqViewMode::FirDesigner { .. }
    );
    let indexing = if is_lp_mode {
        EqBandIndexing::FIR
    } else {
        EqBandIndexing::STANDARD
    };
    let layout = EqCompactLayout::from_width(state.available_width);

    // Compute selected param for editing mode
    let highlight_band_idx = if state.is_editing {
        Some(state.selected_param / indexing.stride)
    } else {
        Some(selected_band_idx)
    };

    // The graph is the primary control surface; band guides render on top of
    // it instead of reserving a legend column.
    let graph_width = state.available_width.max(800.0);

    // Build the UI - graph uses most of the horizontal space
    let graph_section = div()
        .flex()
        .flex_col()
        .flex_1()
        .child(render_eq_visualization(
            entity.clone(),
            plugin_idx,
            display_filters,
            highlight_band_idx,
            indexing,
            theme,
            graph_width,
        ));

    // Clone values needed for closures
    let channels = state.channels;
    // Linear-phase EQ is global-only; force the toggle off so the renderer's
    // downstream logic doesn't try to surface per-channel data we don't have.
    let per_channel_mode = if is_lp_mode {
        false
    } else {
        state.per_channel_mode
    };
    let channel_toolbar = (!is_lp_mode).then(|| {
        render_eq_channel_toolbar(
            &ds,
            entity.clone(),
            plugin_idx,
            channels,
            selected_eq_channel,
            per_channel_mode,
            theme,
        )
    });

    let _legacy_controls_section = if layout == EqCompactLayout::Current {
        div()
            .flex()
            .flex_col()
            .items_center() // Center band selector and knob box
            .gap(ds.section)
            .w_full()
            // Channel Mode Toggle and Channel Selector — hidden in linear-phase mode
            .when(!is_lp_mode, |container| {
                container.child({
                    let entity_clone = entity.clone();
                    let entity_clone2 = entity.clone();
                    let accent = theme.accent;
                    let text_on_accent = theme.text_on_accent;
                    let text_secondary = theme.text_secondary;
                    let bg_secondary = theme.background_secondary;
                    let surface_hover = theme.surface_hover;
                    let border = theme.border;

                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(ds.section)
                        .p(ds.pad_y)
                        .bg(theme.surface)
                        .rounded(ds.r_lg)
                        // Mode toggle buttons
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(ds.grid)
                                // All Channels button
                                .child({
                                    let is_selected = !per_channel_mode;
                                    div()
                                        .id("eq-mode-all")
                                        .px(ds.pad_x)
                                        .py(ds.pad_y_half)
                                        .text_size(ds.text_sm)
                                        .rounded(ds.r_md)
                                        .cursor_pointer()
                                        .when(is_selected, |d| {
                                            d.bg(accent)
                                                .text_color(text_on_accent)
                                                .font_weight(FontWeight::SEMIBOLD)
                                        })
                                        .when(!is_selected, |d| {
                                            d.bg(bg_secondary)
                                                .text_color(text_secondary)
                                                .hover(move |s| s.bg(surface_hover))
                                        })
                                        .on_mouse_down(MouseButton::Left, {
                                            let entity = entity_clone.clone();
                                            move |_event, _window, cx| {
                                                entity.update(cx, |state, cx| {
                                                    state
                                                        .app
                                                        .set_eq_per_channel_mode(plugin_idx, false);
                                                    cx.notify();
                                                });
                                            }
                                        })
                                        .child("All Channels")
                                })
                                // Per Channel button
                                .child({
                                    let is_selected = per_channel_mode;
                                    div()
                                        .id("eq-mode-per-channel")
                                        .px(ds.pad_x)
                                        .py(ds.pad_y_half)
                                        .text_size(ds.text_sm)
                                        .rounded(ds.r_md)
                                        .cursor_pointer()
                                        .when(is_selected, |d| {
                                            d.bg(accent)
                                                .text_color(text_on_accent)
                                                .font_weight(FontWeight::SEMIBOLD)
                                        })
                                        .when(!is_selected, |d| {
                                            d.bg(bg_secondary)
                                                .text_color(text_secondary)
                                                .hover(move |s| s.bg(surface_hover))
                                        })
                                        .on_mouse_down(MouseButton::Left, {
                                            let entity = entity_clone2.clone();
                                            move |_event, _window, cx| {
                                                entity.update(cx, |state, cx| {
                                                    state
                                                        .app
                                                        .set_eq_per_channel_mode(plugin_idx, true);
                                                    cx.notify();
                                                });
                                            }
                                        })
                                        .child("Per Channel")
                                }),
                        )
                        // Channel selector (only shown in per-channel mode)
                        .when(per_channel_mode, |d| {
                            d.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(ds.grid)
                                    .border(px(1.0))
                                    .border_color(border)
                                    .rounded(ds.r_md)
                                    .px(ds.pad_y)
                                    .children((0..channels).map(|ch| {
                                        let entity = entity.clone();
                                        let is_selected = ch == selected_eq_channel;
                                        let ch_name = get_channel_name(ch, channels);
                                        div()
                                            .id(("eq-channel", ch))
                                            .px(ds.pad_y)
                                            .py(ds.pad_y_half)
                                            .text_size(ds.text_sm)
                                            .rounded(ds.r_sm)
                                            .cursor_pointer()
                                            .when(is_selected, |d| {
                                                d.bg(accent)
                                                    .text_color(text_on_accent)
                                                    .font_weight(FontWeight::SEMIBOLD)
                                            })
                                            .when(!is_selected, |d| {
                                                d.bg(bg_secondary)
                                                    .text_color(text_secondary)
                                                    .hover(move |s| s.bg(surface_hover))
                                            })
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                move |_event, _window, cx| {
                                                    entity.update(cx, |state, _| {
                                                        state
                                                            .app
                                                            .plugin_state
                                                            .selected_eq_channel = ch;
                                                    });
                                                },
                                            )
                                            .child(ch_name)
                                    })),
                            )
                        })
                })
            })
            // Band selector tabs (custom rendering to avoid context issues)
            .child({
                let mut tabs_container = div()
                    .flex()
                    .items_center()
                    .justify_center() // Center tabs
                    .gap(ds.gap)
                    .p(ds.grid)
                    .bg(theme.surface)
                    .rounded(ds.r_lg);

                // Build each band tab manually
                for band_idx in 0..num_bands {
                    let is_selected = band_idx == selected_band_idx;
                    let filter = display_filters.get(band_idx);
                    let is_muted = filter.map(|f| f.muted).unwrap_or(false);
                    let is_soloed = filter.map(|f| f.solo).unwrap_or(false);
                    let filter_short_name =
                        filter.map(|f| f.filter_type.short_name()).unwrap_or("PK");
                    let entity_clone = entity.clone();
                    let accent = theme.accent;
                    let text_on_accent = theme.text_on_accent;
                    let text_secondary = theme.text_secondary;
                    let text_muted_color = theme.text_muted;
                    let text_primary = theme.text_primary;
                    let bg_secondary = theme.background_secondary;
                    let surface_hover = theme.surface_hover;
                    let error = theme.error;
                    let success = theme.success;
                    let border = theme.border;

                    let focus_handle = cx.focus_handle();

                    let tab = div()
                        .id(("eq-band", band_idx))
                        .track_focus(&focus_handle)
                        .key_context("plugin-control")
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(ds.grid)
                        .px(ds.pad_x)
                        .py(ds.pad_y)
                        .text_size(ds.text_sm)
                        .rounded(ds.r_md)
                        .cursor_pointer()
                        .when(is_selected, |d: Stateful<Div>| {
                            d.bg(accent)
                                .text_color(text_on_accent)
                                .font_weight(FontWeight::SEMIBOLD)
                        })
                        .when(!is_selected, |d: Stateful<Div>| {
                            d.bg(bg_secondary)
                                .text_color(text_secondary)
                                .hover(move |s: StyleRefinement| s.bg(surface_hover))
                        })
                        .when(is_muted, |d: Stateful<Div>| d.opacity(0.5))
                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                            entity_clone.update(cx, |state, _| {
                                state.app.plugin_state.selected_eq_band = band_idx;
                                // Also set editing plugin index so keybindings work
                                state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                            });
                        })
                        // Band number with filter type short code (e.g., "#1 PK")
                        .child(div().child(format!("#{} {}", band_idx + 1, filter_short_name)))
                        // Mute and Solo buttons row
                        .child(
                            div()
                                .flex()
                                .gap(ds.grid)
                                // Mute button (small circle)
                                .child({
                                    let entity_clone2 = entity.clone();
                                    div()
                                        .w(px(18.0))
                                        .h(px(18.0))
                                        .rounded_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(if is_muted { error } else { bg_secondary })
                                        .border(px(1.0))
                                        .border_color(if is_muted { error } else { border })
                                        .text_size(ds.text_xs)
                                        .font_weight(FontWeight::BOLD)
                                        .cursor_pointer()
                                        .when(is_muted, |d| d.text_color(text_primary))
                                        .when(!is_muted, |d| d.text_color(text_muted_color))
                                        .hover(move |s| {
                                            s.bg(if is_muted { error } else { surface_hover })
                                        })
                                        .on_mouse_down(MouseButton::Left, {
                                            move |_event, _window, cx| {
                                                cx.stop_propagation();
                                                entity_clone2.update(cx, |state, cx| {
                                                    state.app.plugin_state.editing_plugin_index =
                                                        Some(plugin_idx);
                                                    if let Err(e) =
                                                        state.app.toggle_eq_band_mute(band_idx)
                                                    {
                                                        log::warn!(
                                                            "Failed to toggle EQ band mute: {}",
                                                            e
                                                        );
                                                    }
                                                    cx.notify();
                                                });
                                            }
                                        })
                                        .child("M")
                                })
                                // Solo button (small circle)
                                .child({
                                    let entity_clone3 = entity.clone();
                                    div()
                                        .w(px(18.0))
                                        .h(px(18.0))
                                        .rounded_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(if is_soloed { success } else { bg_secondary })
                                        .border(px(1.0))
                                        .border_color(if is_soloed { success } else { border })
                                        .text_size(ds.text_xs)
                                        .font_weight(FontWeight::BOLD)
                                        .cursor_pointer()
                                        .when(is_soloed, |d| d.text_color(text_primary))
                                        .when(!is_soloed, |d| d.text_color(text_muted_color))
                                        .hover(move |s| {
                                            s.bg(if is_soloed { success } else { surface_hover })
                                        })
                                        .on_mouse_down(MouseButton::Left, {
                                            move |_event, _window, cx| {
                                                cx.stop_propagation();
                                                entity_clone3.update(cx, |state, cx| {
                                                    state.app.plugin_state.editing_plugin_index =
                                                        Some(plugin_idx);
                                                    if let Err(e) =
                                                        state.app.toggle_eq_band_solo(band_idx)
                                                    {
                                                        log::warn!(
                                                            "Failed to toggle EQ band solo: {}",
                                                            e
                                                        );
                                                    }
                                                    cx.notify();
                                                });
                                            }
                                        })
                                        .child("S")
                                }),
                        );

                    tabs_container = tabs_container.child(tab);
                }

                tabs_container
                    // Add band button
                    .child({
                        let entity_clone = entity.clone();
                        div()
                            .id("eq-add-band")
                            .focusable()
                            .key_context("plugin-control")
                            .px(ds.pad_x)
                            .py_1p5()
                            .text_size(ds.text_sm)
                            .font_weight(FontWeight::BOLD)
                            .rounded(ds.r_sm)
                            .cursor_pointer()
                            .bg(theme.success)
                            .text_color(theme.text_on_accent)
                            .hover(|s: StyleRefinement| s.opacity(0.8))
                            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                entity_clone.update(cx, |state, cx| {
                                    state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                                    if let Err(e) = state.app.add_eq_band() {
                                        log::warn!("Failed to add EQ band: {}", e);
                                    }
                                    cx.notify();
                                });
                            })
                            .child("+")
                    })
            })
            // MIDI page indicator (shown when controller connected)
            .when(
                state.midi_overlay.is_some_and(|o| o.has_controller()),
                |d| {
                    let Some(overlay) = state.midi_overlay else {
                        return d;
                    };
                    d.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(ds.gap)
                            .children(overlay.controller_name.as_ref().map(|name| {
                                div()
                                    .text_size(ds.text_xs)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text_secondary)
                                    .child(name.clone())
                            }))
                            .child(render_midi_page_indicator(
                                &ds,
                                overlay.current_page,
                                overlay.total_pages,
                                theme,
                            )),
                    )
                },
            )
            // Selected band controls
            .when(selected_filter.is_some(), |d| {
                let Some(filter) = selected_filter else {
                    return d;
                };
                let base_param_idx = selected_band_idx * indexing.stride;
                let midi_overlay = state.midi_overlay;

                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(ds.gap)
                        .p(ds.pad_x)
                        .bg(theme.background_secondary)
                        .rounded(ds.r_md)
                        // Filter type selector + topology controls
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(ds.grid)
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(ds.grid)
                                        .child(
                                            div()
                                                .text_size(ds.text_xs)
                                                .text_color(theme.text_muted)
                                                .child("Type"),
                                        )
                                        .when(!is_lp_mode, |row| {
                                            row.child(render_standard_eq_algorithm_pill(
                                                &ds,
                                                state.topology,
                                                theme,
                                            ))
                                        }),
                                )
                                .child(render_filter_type_selector(
                                    &ds,
                                    entity.clone(),
                                    plugin_idx,
                                    &filter.filter_type,
                                    selected_band_idx,
                                    base_param_idx + indexing.filter_type,
                                    None,
                                    theme,
                                )),
                        )
                        // Knobs row with MIDI badges
                        .child(
                            div()
                                .flex()
                                .gap(ds.section_lg)
                                .justify_center()
                                .child(render_eq_knob_with_midi(
                                    &ds,
                                    entity.clone(),
                                    plugin_idx,
                                    "Freq",
                                    filter.frequency,
                                    pk(EQ, "freq").min_f64(),
                                    pk(EQ, "freq").max_f64(),
                                    "Hz",
                                    base_param_idx + indexing.frequency,
                                    state.selected_param,
                                    state.is_editing,
                                    midi_overlay,
                                    theme,
                                ))
                                .child(render_eq_knob_with_midi(
                                    &ds,
                                    entity.clone(),
                                    plugin_idx,
                                    "Q",
                                    filter.q,
                                    pk(EQ, "q").min_f64(),
                                    pk(EQ, "q").max_f64(),
                                    "",
                                    base_param_idx + indexing.q,
                                    state.selected_param,
                                    state.is_editing,
                                    midi_overlay,
                                    theme,
                                ))
                                .child(render_eq_knob_with_midi(
                                    &ds,
                                    entity.clone(),
                                    plugin_idx,
                                    "Gain",
                                    filter.gain_db,
                                    pk(EQ, "gain").min_f64(),
                                    pk(EQ, "gain").max_f64(),
                                    "dB",
                                    base_param_idx + indexing.gain,
                                    state.selected_param,
                                    state.is_editing,
                                    midi_overlay,
                                    theme,
                                ))
                                .children(indexing.active.map(|active_local_idx| {
                                    render_eq_active_toggle(
                                        &ds,
                                        entity.clone(),
                                        plugin_idx,
                                        filter,
                                        base_param_idx + active_local_idx,
                                        state.selected_param,
                                        state.is_editing,
                                        theme,
                                    )
                                })),
                        ),
                )
            })
    } else {
        div()
    };
    let controls_section = if layout == EqCompactLayout::Current {
        render_eq_property_strip(
            &ds,
            entity.clone(),
            plugin_idx,
            selected_filter,
            selected_band_idx,
            indexing,
            &state,
            is_lp_mode,
            theme,
        )
    } else {
        div().into_any_element()
    };

    // Optional linear-phase info header — shown only for the LP variant.
    let fir_summary = match &state.mode {
        EqViewMode::LinearPhase {
            latency_samples,
            latency_ms,
            fir_length,
            auto_gain,
            mix,
        } => Some((
            *latency_samples,
            *latency_ms,
            *fir_length,
            "Linear",
            *auto_gain,
            *mix,
        )),
        EqViewMode::FirDesigner {
            latency_samples,
            latency_ms,
            fir_length,
            phase_mode,
            auto_gain,
            mix,
        } => Some((
            *latency_samples,
            *latency_ms,
            *fir_length,
            *phase_mode,
            *auto_gain,
            *mix,
        )),
        EqViewMode::Standard => None,
    };
    let lp_header = fir_summary.map(
        |(latency_samples, latency_ms, fir_length, phase_mode, auto_gain, mix)| {
            div()
                .flex()
                .items_center()
                .justify_center()
                .gap(ds.section)
                .px(ds.pad_x)
                .py(ds.pad_y_half)
                .bg(theme.surface)
                .rounded(ds.r_md)
                .text_size(ds.text_sm)
                .text_color(theme.text_secondary)
                .child(render_eq_global_stepper(
                    &ds,
                    entity.clone(),
                    plugin_idx,
                    if is_fir_mode {
                        EqGlobalControl::FirNumFilters
                    } else {
                        EqGlobalControl::LpNumFilters
                    },
                    "Filters",
                    state.num_filters.to_string(),
                    theme,
                ))
                .child(render_eq_global_stepper(
                    &ds,
                    entity.clone(),
                    plugin_idx,
                    if is_fir_mode {
                        EqGlobalControl::FirLength
                    } else {
                        EqGlobalControl::LpFirLength
                    },
                    "FIR length",
                    fir_length.to_string(),
                    theme,
                ))
                .child(if is_fir_mode {
                    render_eq_global_toggle(
                        &ds,
                        entity.clone(),
                        plugin_idx,
                        EqGlobalControl::FirPhaseMode,
                        "Phase",
                        phase_mode == "Minimum",
                        "Minimum",
                        "Linear",
                        theme,
                    )
                } else {
                    div()
                        .text_size(ds.text_sm)
                        .child(format!("Phase: {phase_mode}"))
                        .into_any_element()
                })
                .child(format!(
                    "Latency: {latency_samples} samples ({latency_ms:.2} ms)"
                ))
                .child(render_eq_global_toggle(
                    &ds,
                    entity.clone(),
                    plugin_idx,
                    if is_fir_mode {
                        EqGlobalControl::FirAutoGain
                    } else {
                        EqGlobalControl::LpAutoGain
                    },
                    "Auto-gain",
                    auto_gain,
                    "On",
                    "Off",
                    theme,
                ))
                .child(render_eq_global_stepper(
                    &ds,
                    entity.clone(),
                    plugin_idx,
                    if is_fir_mode {
                        EqGlobalControl::FirMix
                    } else {
                        EqGlobalControl::LpMix
                    },
                    "Mix",
                    format!("{:.0}%", mix * 100.0),
                    theme,
                ))
        },
    );
    let lp_analysis = fir_summary.map(
        |(latency_samples, latency_ms, fir_length, phase_mode, _, _)| {
            render_linear_phase_analysis(
                &ds,
                latency_samples,
                latency_ms,
                fir_length,
                phase_mode,
                theme,
            )
        },
    );

    // Algorithm info bar for Standard EQ
    let eq_header = if matches!(&state.mode, EqViewMode::Standard) {
        Some(
            div()
                .flex()
                .items_center()
                .justify_center()
                .gap(ds.section)
                .px(ds.pad_x)
                .py(ds.pad_y_half)
                .bg(theme.surface)
                .rounded(ds.r_md)
                .text_size(ds.text_sm)
                .text_color(theme.text_secondary)
                .child(render_eq_global_stepper(
                    &ds,
                    entity.clone(),
                    plugin_idx,
                    EqGlobalControl::StandardMaxFilters,
                    "Filters",
                    state.num_filters.to_string(),
                    theme,
                ))
                .child(render_eq_global_toggle(
                    &ds,
                    entity.clone(),
                    plugin_idx,
                    EqGlobalControl::StandardTopology,
                    "Topology",
                    state.topology > 0.5,
                    "SVF",
                    "Biquad",
                    theme,
                ))
                .child(render_eq_global_toggle(
                    &ds,
                    entity.clone(),
                    plugin_idx,
                    EqGlobalControl::StandardTdf2,
                    "TDF-II",
                    state.tdf2,
                    "On",
                    "Off",
                    theme,
                )),
        )
    } else {
        None
    };

    // Combine sections based on layout mode

    match layout {
        EqCompactLayout::Current => div()
            .flex()
            .flex_col()
            .items_center()
            .gap(ds.section_xl)
            .children(eq_header)
            .children(lp_header)
            .children(channel_toolbar)
            .child(graph_section)
            .children(lp_analysis)
            .child(controls_section)
            .into_any_element(),
        EqCompactLayout::BottomStrip => super::layout_compact::render_eq_bottom_strip(
            entity,
            plugin_idx,
            &state,
            display_filters,
            selected_band_idx,
            indexing,
            theme,
            cx,
        )
        .into_any_element(),
        EqCompactLayout::Inspector => super::layout_compact::render_eq_inspector(
            entity,
            plugin_idx,
            &state,
            display_filters,
            selected_band_idx,
            indexing,
            theme,
            cx,
        )
        .into_any_element(),
    }
}

fn render_linear_phase_analysis(
    d: &Ds,
    latency_samples: usize,
    latency_ms: f32,
    fir_length: usize,
    phase_mode: &str,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_wrap()
        .justify_center()
        .gap(d.gap)
        .w_full()
        .child(render_lp_analysis_card(
            d,
            "Magnitude",
            "Editable paragraphic target".to_string(),
            theme.accent,
            theme,
        ))
        .child(render_lp_analysis_card(
            d,
            "Phase",
            if phase_mode == "Linear" {
                "Linear after latency compensation".to_string()
            } else {
                "Minimum phase, energy near start".to_string()
            },
            theme.success,
            theme,
        ))
        .child(render_lp_analysis_card(
            d,
            "Group Delay",
            format!("{latency_samples} samples / {latency_ms:.2} ms"),
            theme.warning,
            theme,
        ))
        .child(render_lp_analysis_card(
            d,
            "Impulse",
            if phase_mode == "Linear" {
                format!("{fir_length} taps, symmetric FIR")
            } else {
                format!("{fir_length} taps, minimum-phase FIR")
            },
            theme.text_secondary,
            theme,
        ))
}

fn render_lp_analysis_card(
    d: &Ds,
    label: &'static str,
    value: String,
    accent: Rgba,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(d.grid)
        .min_w(rems(10.0))
        .px(d.pad_x)
        .py(d.pad_y)
        .rounded(d.r_md)
        .bg(theme.surface)
        .border_l_4()
        .border_color(accent)
        .child(
            div()
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(accent)
                .child(label),
        )
        .child(
            div()
                .text_size(d.text_sm)
                .text_color(theme.text_secondary)
                .child(value),
        )
}

fn render_standard_eq_algorithm_pill(d: &Ds, topology: f64, theme: &Theme) -> impl IntoElement {
    div()
        .px(d.pad_y)
        .py(d.pad_y_half)
        .text_size(d.text_xs)
        .font_weight(FontWeight::SEMIBOLD)
        .rounded(d.r_sm)
        .bg(theme.background_secondary)
        .text_color(theme.text_secondary)
        .child(if topology > 0.5 { "SVF" } else { "Biquad" })
}

pub(crate) fn render_eq_active_toggle(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    filter: &EQFilter,
    param_idx: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &Theme,
) -> AnyElement {
    let active = !filter.muted;
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(d.grid)
        .rounded(d.r_md)
        .when(selected_param == param_idx && is_editing, |el| {
            el.border_1().border_color(theme.accent)
        })
        .child(
            div()
                .text_size(d.text_xs)
                .text_color(theme.text_muted)
                .child("Active"),
        )
        .child(
            div()
                .px(d.pad_y)
                .py(d.pad_y_half)
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .rounded(d.r_sm)
                .cursor_pointer()
                .when(active, |el| {
                    el.bg(theme.accent).text_color(theme.text_on_accent)
                })
                .when(!active, |el| {
                    el.bg(theme.background_secondary)
                        .text_color(theme.text_secondary)
                        .hover(|s| s.bg(theme.surface_hover))
                })
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    entity.update(cx, |state, _| {
                        state.app.set_plugin_param(
                            plugin_idx,
                            param_idx,
                            if active { 0.0 } else { 1.0 },
                        );
                    });
                })
                .child(if active { "On" } else { "Off" }),
        )
        .into_any_element()
}

fn mark_eq_global_update(state: &mut AppState) {
    state.app.plugin_state.update_state.pending_plugin_update =
        Some(crate::app::types::PluginUpdateType::Structural);
}

fn adjust_eq_global_control(
    entity: &Entity<AppState>,
    plugin_idx: usize,
    control: EqGlobalControl,
    delta: f64,
    cx: &mut App,
) {
    entity.update(cx, |state, cx| {
        let Some(plugin) = state.app.plugin_state.graph.get_plugin_mut(plugin_idx) else {
            return;
        };
        match (&mut plugin.settings, control) {
            (PluginSettings::EQ { max_filters, .. }, EqGlobalControl::StandardMaxFilters) => {
                *max_filters = ((*max_filters as i64) + delta as i64).clamp(
                    pk(EQ_GLOBAL, "max_filters").min_f64() as i64,
                    pk(EQ_GLOBAL, "max_filters").max_f64() as i64,
                ) as usize;
            }
            (PluginSettings::EQ { tdf2, .. }, EqGlobalControl::StandardTdf2) => *tdf2 = !*tdf2,
            (PluginSettings::EQ { topology, .. }, EqGlobalControl::StandardTopology) => {
                *topology = if *topology > 0.5 { 0.0 } else { 1.0 };
            }
            (PluginSettings::LinearPhaseEq { num_filters, .. }, EqGlobalControl::LpNumFilters) => {
                *num_filters = (*num_filters + delta).clamp(
                    pk(LP_PARAMS, "num_filters").min_f64(),
                    pk(LP_PARAMS, "num_filters").max_f64(),
                );
            }
            (PluginSettings::LinearPhaseEq { fir_length, .. }, EqGlobalControl::LpFirLength) => {
                *fir_length = (*fir_length + delta).clamp(
                    pk(LP_PARAMS, "fir_length").min_f64(),
                    pk(LP_PARAMS, "fir_length").max_f64(),
                );
            }
            (PluginSettings::LinearPhaseEq { auto_gain, .. }, EqGlobalControl::LpAutoGain) => {
                *auto_gain = !*auto_gain;
            }
            (PluginSettings::LinearPhaseEq { mix, .. }, EqGlobalControl::LpMix) => {
                *mix = (*mix + delta * 0.01).clamp(
                    pk(LP_PARAMS, "mix").min_f64(),
                    pk(LP_PARAMS, "mix").max_f64(),
                );
            }
            (PluginSettings::FirDesigner { num_filters, .. }, EqGlobalControl::FirNumFilters) => {
                *num_filters = (*num_filters + delta).clamp(
                    pk(FIR_PARAMS, "num_filters").min_f64(),
                    pk(FIR_PARAMS, "num_filters").max_f64(),
                );
            }
            (PluginSettings::FirDesigner { fir_length, .. }, EqGlobalControl::FirLength) => {
                *fir_length = (*fir_length + delta).clamp(
                    pk(FIR_PARAMS, "fir_length").min_f64(),
                    pk(FIR_PARAMS, "fir_length").max_f64(),
                );
            }
            (PluginSettings::FirDesigner { phase_mode, .. }, EqGlobalControl::FirPhaseMode) => {
                *phase_mode = if *phase_mode > 0.5 { 0.0 } else { 1.0 };
            }
            (PluginSettings::FirDesigner { auto_gain, .. }, EqGlobalControl::FirAutoGain) => {
                *auto_gain = !*auto_gain;
            }
            (PluginSettings::FirDesigner { mix, .. }, EqGlobalControl::FirMix) => {
                *mix = (*mix + delta * 0.01).clamp(
                    pk(FIR_PARAMS, "mix").min_f64(),
                    pk(FIR_PARAMS, "mix").max_f64(),
                );
            }
            _ => return,
        }
        mark_eq_global_update(state);
        cx.notify();
    });
}

pub(crate) fn render_eq_global_stepper(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    control: EqGlobalControl,
    label: &'static str,
    value: String,
    theme: &Theme,
) -> AnyElement {
    let minus_entity = entity.clone();
    let plus_entity = entity;
    div()
        .flex()
        .items_center()
        .gap(d.grid)
        .px(d.pad_y)
        .py(d.pad_y_half)
        .rounded(d.r_sm)
        .bg(theme.background_secondary)
        .child(
            div()
                .text_size(d.text_xs)
                .text_color(theme.text_muted)
                .child(label),
        )
        .child(
            div()
                .px(d.grid)
                .cursor_pointer()
                .text_color(theme.text_secondary)
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    adjust_eq_global_control(&minus_entity, plugin_idx, control, -1.0, cx);
                })
                .child("-"),
        )
        .child(
            div()
                .min_w(px(42.0))
                .text_center()
                .text_size(d.text_sm)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_primary)
                .child(value),
        )
        .child(
            div()
                .px(d.grid)
                .cursor_pointer()
                .text_color(theme.text_secondary)
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    adjust_eq_global_control(&plus_entity, plugin_idx, control, 1.0, cx);
                })
                .child("+"),
        )
        .into_any_element()
}

pub(crate) fn render_eq_global_toggle(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    control: EqGlobalControl,
    label: &'static str,
    value: bool,
    on_label: &'static str,
    off_label: &'static str,
    theme: &Theme,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(d.grid)
        .px(d.pad_y)
        .py(d.pad_y_half)
        .rounded(d.r_sm)
        .cursor_pointer()
        .bg(if value {
            theme.accent
        } else {
            theme.background_secondary
        })
        .text_color(if value {
            theme.text_on_accent
        } else {
            theme.text_secondary
        })
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            adjust_eq_global_control(&entity, plugin_idx, control, 1.0, cx);
        })
        .child(div().text_size(d.text_xs).child(label))
        .child(
            div()
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .child(if value { on_label } else { off_label }),
        )
        .into_any_element()
}

/// Render a filter type selector using exclusive buttons
pub(crate) fn render_filter_type_selector(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    current_type: &BiquadFilterType,
    _band_idx: usize,
    param_idx: usize,
    _select_open: Option<(usize, usize)>,
    theme: &Theme,
) -> impl IntoElement {
    // Define all filter types with 2-letter abbreviations
    let filter_types: Vec<(usize, &'static str)> = vec![
        (0, "PK"), // Peak
        (1, "LS"), // Low Shelf
        (2, "HS"), // High Shelf
        (3, "LP"), // Low Pass
        (4, "HP"), // High Pass
        (5, "BP"), // Band Pass
        (6, "NO"), // Notch
        (7, "AP"), // All Pass
    ];

    let current_index = get_filter_type_index(current_type);
    let d = *d;

    div()
        .flex()
        .flex_wrap()
        .gap(d.grid)
        .children(filter_types.into_iter().map(move |(idx, abbrev)| {
            let is_active = idx == current_index;
            let entity_clone = entity.clone();

            div()
                .px(d.pad_y)
                .py(d.pad_y_half)
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .rounded(d.r_sm)
                .cursor_pointer()
                .when(is_active, |el| {
                    el.bg(theme.accent).text_color(theme.text_on_accent)
                })
                .when(!is_active, |el| {
                    el.bg(theme.background_secondary)
                        .text_color(theme.text_secondary)
                        .hover(|s| s.bg(theme.surface_hover))
                })
                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                    entity_clone.update(cx, |state, _| {
                        state
                            .app
                            .set_plugin_param(plugin_idx, param_idx, idx as f64);
                    });
                })
                .child(abbrev)
        }))
}
