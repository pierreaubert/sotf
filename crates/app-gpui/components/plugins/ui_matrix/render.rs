// intentional-file: fixed pixel values here are graph and plugin control geometry.
use super::consts::CELL_SIZE;
use super::consts::DB_STEP;
use super::consts::LABEL_WIDTH;
use super::consts::MAX_DB;
use super::consts::MIN_DB;
use super::consts::MSD_BTN_SIZE;
use super::consts::MSD_COL_WIDTH;
use super::misc::compute_output_groups;
use super::misc::format_gain_db;
use super::misc::{cell_index, checked_matrix_cell_index};
use super::types::MatrixRenderState;
use super::types::MsdAction;
use crate::app::AppState;
use crate::app::i18n::PluginCommonTranslations;
use crate::components::design::Ds;
use crate::components::themed_tooltip;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, ElementId, Entity, FontWeight, MouseButton, Rgba, ScrollHandle, SharedString,
    div, px, rems,
};
use gpui_ui_kit::{
    ButtonSet, ButtonSetOption, ButtonSetSize, NumberInput, NumberInputSize, Select, SelectOption,
    SelectSize, Slider, SliderSize, Text,
};
use sotf_audio_player::PluginUpdateEffect;
use sotf_audio_player::{db_to_linear, get_channel_label_from_config};

#[derive(Clone, Copy)]
struct MatrixGeometry {
    cell_size: f32,
    label_width: f32,
    msd_col_width: f32,
    msd_button_size: f32,
}

#[expect(
    clippy::cast_precision_loss,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
fn matrix_geometry(state: &MatrixRenderState<'_>) -> MatrixGeometry {
    let scale = state.layout_scale.max(0.01);
    let base_cell_size = CELL_SIZE * scale;
    let label_width = LABEL_WIDTH * scale;
    let msd_col_width = MSD_COL_WIDTH * scale;
    let channel_count = state.input_channels.max(state.output_channels).max(1) as f32;
    let available_for_cells =
        state.available_width.max(0.0) - label_width - msd_col_width - (8.0 * scale);
    let width_limited_cell_size = available_for_cells / channel_count;

    MatrixGeometry {
        // Preserve a readable minimum, but let dense 12–16 channel matrices
        // use the available width before the outer scroll view takes over.
        cell_size: base_cell_size
            .min(width_limited_cell_size.max(24.0))
            .clamp(24.0, 96.0),
        label_width,
        msd_col_width,
        msd_button_size: (MSD_BTN_SIZE * scale).clamp(18.0, 42.0),
    }
}

/// Render the Matrix plugin
#[must_use]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
pub fn render_matrix_plugin(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    mut state: MatrixRenderState,
    theme: &Theme,
    route_labels: [&'static str; 2],
    scroll_handle: &ScrollHandle,
    text: PluginCommonTranslations,
) -> impl IntoElement {
    let geometry = matrix_geometry(&state);
    // Keep a route editable even before the user can reach the grid.
    if state.selected_cell.is_none() && state.input_channels > 0 && state.output_channels > 0 {
        state.selected_cell = Some((0, 0));
    }

    let grid = div()
        .id("matrix-grid-scroll")
        .w_full()
        .min_w_0()
        .overflow_x_scroll()
        .map(|mut content| {
            // Vertical wheel navigation belongs to the enclosing plugin editor.
            content.style().restrict_scroll_to_axis = Some(true);
            content
        })
        .track_scroll(scroll_handle)
        .child(render_matrix_grid(
            d,
            entity.clone(),
            plugin_idx,
            &state,
            geometry,
            theme,
        ));
    #[cfg(feature = "dev-api")]
    let grid = {
        use crate::app::dev_api::DevTrackExt;
        grid.dev_track("matrix.grid")
    };
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(
            div().flex().flex_wrap().items_center().gap(d.grid)
                .child(render_route_selectors(d, entity.clone(), &state, theme, route_labels))
                .children(render_selected_cell_editor(d, entity.clone(), &state, theme, text)),
        )
        .child(grid)
        .child(Text::caption(text.label("Off silences the route; On starts at 0 dB. Alt+wheel adjusts gain; double-click a cell to clear it.")))
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
fn render_route_selectors(
    d: &Ds,
    entity: Entity<AppState>,
    state: &MatrixRenderState,
    theme: &Theme,
    route_labels: [&'static str; 2],
) -> impl IntoElement {
    let instance_id = state.plugin_instance_id;
    let (input, output) = state.selected_cell.unwrap_or((0, 0));
    div()
        .flex()
        .flex_wrap()
        .gap(d.gap)
        .children([false, true].map(|is_output| {
            let count = if is_output {
                state.output_channels
            } else {
                state.input_channels
            };
            let selected = if is_output { output } else { input };
            // Select's retained blur subscription must not own the window state.
            let toggle = entity.downgrade();
            let change = entity.clone();
            let highlight_entity = entity.downgrade();
            let selector = Select::new(if is_output {
                "matrix-route-output"
            } else {
                "matrix-route-input"
            })
            .aria_label(route_labels[usize::from(is_output)])
            .size(SelectSize::Xs)
            .options(
                (0..count)
                    .map(|index| {
                        SelectOption::new(
                            index.to_string(),
                            get_channel_label_from_config(
                                index,
                                count,
                                state.speaker_config.as_deref(),
                            ),
                        )
                    })
                    .collect(),
            )
            .selected(selected.to_string())
            .disabled(count == 0)
            .is_open(state.route_menu == Some(is_output))
            .highlighted_index(state.route_highlight)
            .theme(theme.to_select_theme())
            .on_toggle(move |open, _, cx| {
                let _ = toggle.update(cx, |state, cx| {
                    let ui = &mut state.app.plugin_state.plugin_ui_state;
                    ui.matrix_route_menu = open.then_some((instance_id, is_output));
                    ui.matrix_choice_highlight = open.then_some(selected);
                    if open {
                        ui.matrix_preset_menu = None;
                    }
                    cx.notify();
                });
            })
            .on_highlight(move |index, _, cx| {
                let _ = highlight_entity.update(cx, |state, cx| {
                    state
                        .app
                        .plugin_state
                        .plugin_ui_state
                        .matrix_choice_highlight = index;
                    cx.notify();
                });
            })
            .on_change(move |value: &SharedString, _, cx| {
                let Ok(index) = value.parse::<usize>() else {
                    return;
                };
                if index >= count {
                    return;
                }
                change.update(cx, |state, cx| {
                    let (input, output) = if is_output {
                        (input, index)
                    } else {
                        (index, output)
                    };
                    state.app.plugin_state.matrix_selected_cell =
                        Some((instance_id, input, output));
                    state.app.plugin_state.plugin_ui_state.matrix_route_menu = None;
                    cx.notify();
                });
            });
            #[cfg(feature = "dev-api")]
            let selector = {
                use crate::app::dev_api::DevTrackExt;
                selector.dev_track(if is_output {
                    "matrix.route.output"
                } else {
                    "matrix.route.input"
                })
            };
            div()
                .key_context("PluginLayoutChoiceSelect")
                .flex()
                .items_center()
                .gap(d.gap)
                .child(Text::label(route_labels[usize::from(is_output)]))
                .child(selector)
        }))
}

#[expect(
    clippy::too_many_lines,
    reason = "Keep the declarative layout and its interaction handlers together"
)]
#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
fn render_selected_cell_editor(
    d: &Ds,
    entity: Entity<AppState>,
    state: &MatrixRenderState,
    theme: &Theme,
    text: PluginCommonTranslations,
) -> Option<AnyElement> {
    let plugin_instance_id = state.plugin_instance_id;
    let (input_idx, output_idx) = state.selected_cell?;
    let matrix_idx = checked_matrix_cell_index(
        input_idx,
        output_idx,
        state.input_channels,
        state.output_channels,
        state.matrix.len(),
    )?;
    let gain = *state.matrix.get(matrix_idx)?;
    let current_db = if gain.abs() < 0.001 {
        MIN_DB
    } else {
        20.0 * gain.abs().log10()
    };
    let polarity = if gain < 0.0 { -1.0 } else { 1.0 };
    let route_on = gain != 0.0;
    let input_label = get_channel_label_from_config(
        input_idx,
        state.input_channels,
        state.speaker_config.as_deref(),
    );
    let output_label = get_channel_label_from_config(
        output_idx,
        state.output_channels,
        state.speaker_config.as_deref(),
    );
    let label = format!("{output_label} ← {input_label} gain");
    let slider = Slider::new(("matrix-route-gain", plugin_instance_id))
        .value(current_db)
        .range(MIN_DB, MAX_DB)
        .step(0.1)
        .width(142.0)
        .size(SliderSize::Sm)
        .theme(theme.to_slider_theme())
        .disabled(!route_on)
        .aria_label(label.clone())
        .on_change({
            let entity = entity.clone();
            move |value, _, cx| {
                set_selected_gain(
                    &entity,
                    plugin_instance_id,
                    input_idx,
                    output_idx,
                    polarity * db_to_linear(value),
                    cx,
                );
            }
        })
        .on_reset({
            let entity = entity.clone();
            move |_, cx| {
                set_selected_gain(
                    &entity,
                    plugin_instance_id,
                    input_idx,
                    output_idx,
                    polarity,
                    cx,
                );
            }
        });
    let exact = NumberInput::new("matrix-selected-gain")
        .value(f64::from(current_db))
        .min(f64::from(MIN_DB))
        .max(f64::from(MAX_DB))
        .step(0.1)
        .decimals(1)
        .unit("dB")
        .size(NumberInputSize::Xs)
        .width(96.0)
        .theme(theme.to_number_input_theme())
        .disabled(!route_on)
        .aria_label(label)
        .on_change({
            let entity = entity.clone();
            move |value, _, cx| {
                set_selected_gain(
                    &entity,
                    plugin_instance_id,
                    input_idx,
                    output_idx,
                    polarity * db_to_linear(value as f32),
                    cx,
                );
            }
        });
    let route = ButtonSet::new(("matrix-route-enabled", plugin_instance_id))
        .options(vec![
            ButtonSetOption::new("off", text.label("Off")),
            ButtonSetOption::new("on", text.label("On")),
        ])
        .selected(if route_on { "on" } else { "off" })
        .size(ButtonSetSize::Xs)
        .theme(theme.to_button_set_theme())
        .on_change({
            let entity = entity.clone();
            move |value, _, cx| {
                set_selected_gain(
                    &entity,
                    plugin_instance_id,
                    input_idx,
                    output_idx,
                    if value.as_ref() == "on" {
                        if route_on { gain } else { 1.0 }
                    } else {
                        0.0
                    },
                    cx,
                );
            }
        });
    let phase = ButtonSet::new(("matrix-route-polarity", plugin_instance_id))
        .options(vec![
            ButtonSetOption::new("positive", "+"),
            ButtonSetOption::new("negative", "−"),
        ])
        .selected(if polarity < 0.0 {
            "negative"
        } else {
            "positive"
        })
        .disabled(!route_on)
        .size(ButtonSetSize::Xs)
        .theme(theme.to_button_set_theme())
        .on_change(move |value, _, cx| {
            set_selected_gain(
                &entity,
                plugin_instance_id,
                input_idx,
                output_idx,
                if value.as_ref() == "negative" {
                    -gain.abs()
                } else {
                    gain.abs()
                },
                cx,
            );
        });
    #[cfg(feature = "dev-api")]
    let (slider, exact, route, phase) = {
        use crate::app::dev_api::DevTrackExt;
        (
            slider.dev_track("matrix.gain.slider"),
            exact.dev_track("matrix.gain.exact"),
            route
                .option_wrapper(|option, button| {
                    button
                        .dev_track(format!("matrix.route.enabled.{}", ["off", "on"][option]))
                        .into_any_element()
                })
                .dev_track("matrix.route.enabled"),
            phase
                .option_wrapper(|option, button| {
                    button
                        .dev_track(format!(
                            "matrix.route.polarity.{}",
                            ["positive", "negative"][option]
                        ))
                        .into_any_element()
                })
                .dev_track("matrix.route.polarity"),
        )
    };
    Some(
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(d.grid)
            .child(Text::label(text.label("Route")))
            .child(route)
            .child(Text::label(text.label("Polarity")))
            .child(phase)
            .child(Text::label("Gain"))
            .child(slider)
            .child(exact)
            .into_any_element(),
    )
}

/// Apply an edit only to the same selected route and live matrix dimensions.
fn set_selected_gain(
    entity: &Entity<AppState>,
    instance_id: usize,
    input: usize,
    output: usize,
    gain: f32,
    cx: &mut App,
) {
    if !gain.is_finite() {
        return;
    }
    entity.update(cx, |state, cx| {
        if state
            .app
            .plugin_state
            .matrix_selected_cell
            .is_some_and(|selected| {
                selected.0 == instance_id && selected != (instance_id, input, output)
            })
        {
            return;
        }
        if let Some(sotf_audio_player::PluginSettings::Matrix {
            input_channels,
            output_channels,
            matrix,
            ..
        }) = state
            .app
            .plugin_state
            .editor_settings_mut_by_instance_id(instance_id)
            && let Some(index) = checked_matrix_cell_index(
                input,
                output,
                *input_channels,
                *output_channels,
                matrix.len(),
            )
        {
            matrix[index] = gain;
            state
                .app
                .plugin_state
                .record_editor_effect(PluginUpdateEffect::Structural);
            cx.notify();
        }
    });
}

/// Render the matrix grid with input columns and output rows, plus M/S/D sidebar
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
#[expect(
    clippy::cast_precision_loss,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
fn render_matrix_grid(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &MatrixRenderState,
    geometry: MatrixGeometry,
    theme: &Theme,
) -> impl IntoElement {
    let output_groups =
        compute_output_groups(state.output_channels, state.speaker_config.as_deref());

    // Include each cell's retained two-pixel horizontal margin so the scrollport
    // measures the full route grid instead of only its shrinkable flex wrapper.
    let content_width = geometry.label_width
        + state.input_channels as f32 * (geometry.cell_size + 2.0)
        + geometry.msd_col_width
        + 16.0 * state.layout_scale;
    div()
        .min_w(px(content_width))
        .flex_shrink_0()
        .flex()
        .gap(d.gap)
        // Left: the matrix grid
        .child(
            div()
                .flex()
                .flex_col()
                // Column headers (input labels)
                .child(
                    div()
                        .flex()
                        .child(
                            // Empty corner cell
                            div()
                                .w(px(geometry.label_width))
                                .flex_shrink_0()
                                .h(px(geometry.cell_size * 0.5))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(d.text_xs)
                                .text_color(theme.text_muted)
                                .child("OUT\\IN"),
                        )
                        .children((0..state.input_channels).map(|in_idx| {
                            let label = get_channel_label_from_config(
                                in_idx,
                                state.input_channels,
                                state.speaker_config.as_deref(),
                            );
                            div()
                                .w(px(geometry.cell_size))
                                .flex_shrink_0()
                                // Match route-cell spacing without increasing header height.
                                .mx_px()
                                .h(px(geometry.cell_size * 0.5))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(d.text_xs)
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.text_secondary)
                                .child(label)
                        })),
                )
                // Grid rows
                .children((0..state.output_channels).map(|out_idx| {
                    render_matrix_row(
                        d,
                        entity.clone(),
                        plugin_idx,
                        out_idx,
                        state,
                        geometry,
                        theme,
                    )
                })),
        )
        // Right: M/S/D sidebar
        .child(render_msd_sidebar(
            d,
            entity.clone(),
            plugin_idx,
            state,
            &output_groups,
            geometry,
            theme,
        ))
}

/// Render the M/S/D sidebar column
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
#[expect(
    clippy::cast_precision_loss,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
fn render_msd_sidebar(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &MatrixRenderState,
    output_groups: &[(String, Vec<usize>)],
    geometry: MatrixGeometry,
    theme: &Theme,
) -> impl IntoElement {
    // Keep the sidebar aligned with matrix rows, including the cell margin.
    let row_height = geometry.cell_size + 2.0;

    div()
        .flex()
        .flex_col()
        .w(px(geometry.msd_col_width))
        // Header spacer to align with column header row
        .child(
            div()
                .h(px(geometry.cell_size * 0.5))
                .flex()
                .items_center()
                .justify_center()
                .text_size(d.text_xs)
                .font_weight(FontWeight::BOLD)
                .text_color(theme.text_muted)
                .child("M S D"),
        )
        // One group row per group, height = number of channels * row_height
        .children(output_groups.iter().map(|(group_name, channels)| {
            let group_height = channels.len() as f32 * row_height;
            let has_label = channels.len() > 1;

            // Check current states for this group
            let any_muted = channels
                .iter()
                .any(|&ch| state.channel_states.get(ch).is_some_and(|s| s.muted));
            let any_soloed = channels
                .iter()
                .any(|&ch| state.channel_states.get(ch).is_some_and(|s| s.soloed));
            let any_dimmed = channels
                .iter()
                .any(|&ch| state.channel_states.get(ch).is_some_and(|s| s.dimmed));

            let output_channels = state.output_channels;

            // Build the button row
            let buttons = div()
                .flex()
                .gap(d.grid)
                .child(render_msd_button(
                    d,
                    "M",
                    any_muted,
                    theme.error,
                    theme,
                    plugin_idx,
                    state.plugin_instance_id,
                    entity.clone(),
                    channels.clone(),
                    output_channels,
                    MsdAction::Mute,
                    geometry.msd_button_size,
                ))
                .child(render_msd_button(
                    d,
                    "S",
                    any_soloed,
                    theme.warning,
                    theme,
                    plugin_idx,
                    state.plugin_instance_id,
                    entity.clone(),
                    channels.clone(),
                    output_channels,
                    MsdAction::Solo,
                    geometry.msd_button_size,
                ))
                .child(render_msd_button(
                    d,
                    "D",
                    any_dimmed,
                    theme.info,
                    theme,
                    plugin_idx,
                    state.plugin_instance_id,
                    entity.clone(),
                    channels.clone(),
                    output_channels,
                    MsdAction::Dim,
                    geometry.msd_button_size,
                ));

            let mut container = div()
                .h(px(group_height))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(d.grid)
                .border_l_1()
                .border_color(theme.border)
                .pl(d.pad_y);

            if has_label {
                container = container.child(
                    div()
                        .text_size(d.text_xs)
                        .text_color(theme.text_muted)
                        .child(group_name.clone()),
                );
            }

            container.child(buttons)
        }))
}

/// Render a single M, S, or D button
fn render_msd_button(
    d: &Ds,
    label: &'static str,
    is_active: bool,
    active_color: Rgba,
    theme: &Theme,
    plugin_idx: usize,
    plugin_instance_id: usize,
    entity: Entity<AppState>,
    channels: Vec<usize>,
    output_channels: usize,
    action: MsdAction,
    button_size: f32,
) -> impl IntoElement {
    let action_name = match &action {
        MsdAction::Mute => "Mute",
        MsdAction::Solo => "Solo",
        MsdAction::Dim => "Dim",
    };
    let tooltip = format!("{action_name} output group");
    let tooltip_theme = theme.clone();
    let group_first_channel = channels.first().copied().unwrap_or(0);

    let button = div()
        .id(ElementId::Name(
            format!("msd-{plugin_idx}-{group_first_channel}-{label:?}").into(),
        ))
        .min_w(rems(1.5))
        .min_h(rems(1.5))
        .w(px(button_size.max(24.0)))
        .rounded(d.r_sm)
        .cursor_pointer()
        .bg(if is_active {
            active_color
        } else {
            theme.background
        })
        .border_1()
        .border_color(if is_active {
            active_color
        } else {
            theme.border
        })
        .hover(|s| {
            s.bg(if is_active {
                active_color
            } else {
                theme.surface_hover
            })
        })
        .flex()
        .items_center()
        .justify_center()
        .text_size(d.text_xs)
        .font_weight(FontWeight::BOLD)
        .text_color(if is_active {
            theme.text_on_accent
        } else {
            theme.text_muted
        })
        .tooltip(move |_window, cx| themed_tooltip(tooltip.clone(), &tooltip_theme, cx))
        .child(label)
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            entity.update(cx, |state, _| {
                if let Some(settings) = state
                    .app
                    .plugin_state
                    .editor_settings_mut_by_instance_id(plugin_instance_id)
                    && let sotf_audio_player::PluginSettings::Matrix {
                        channel_states,
                        output_channels: out_ch,
                        ..
                    } = settings
                {
                    // Resize channel_states if needed
                    let target_len = (*out_ch).max(output_channels);
                    while channel_states.len() < target_len {
                        channel_states.push(sotf_plugins::ChannelState::default());
                    }
                    // Toggle: if any channel in group has the flag set, clear all; else set all
                    let new_value = !is_active;
                    for &ch in &channels {
                        if ch < channel_states.len() {
                            match action {
                                MsdAction::Mute => channel_states[ch].muted = new_value,
                                MsdAction::Solo => channel_states[ch].soloed = new_value,
                                MsdAction::Dim => channel_states[ch].dimmed = new_value,
                            }
                        }
                    }
                    state
                        .app
                        .plugin_state
                        .record_editor_effect(PluginUpdateEffect::Structural);
                }
            });
        });
    #[cfg(feature = "dev-api")]
    let button = {
        use crate::app::dev_api::{DevElementState, DevTrackExt};
        button.dev_track_with_state(
            format!("matrix.output.{plugin_idx}.{group_first_channel}.{label}"),
            DevElementState::default()
                .selected(is_active)
                .text(action_name),
        )
    };
    button
}

/// Render a single row of the matrix grid
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
fn render_matrix_row(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    output_idx: usize,
    state: &MatrixRenderState,
    geometry: MatrixGeometry,
    theme: &Theme,
) -> impl IntoElement {
    let output_label = get_channel_label_from_config(
        output_idx,
        state.output_channels,
        state.speaker_config.as_deref(),
    );

    div()
        .flex()
        // Row label (output channel)
        .child(
            div()
                .w(px(geometry.label_width))
                .flex_shrink_0()
                .h(px(geometry.cell_size))
                .flex()
                .items_center()
                .justify_center()
                .text_size(d.text_xs)
                .font_weight(FontWeight::BOLD)
                .text_color(theme.text_secondary)
                .child(output_label),
        )
        // Cells for each input
        .children((0..state.input_channels).map(|in_idx| {
            let idx = cell_index(in_idx, output_idx, state.input_channels);
            let gain = state.matrix.get(idx).copied().unwrap_or(0.0);
            let is_selected = state
                .selected_cell
                .is_some_and(|(sel_in, sel_out)| sel_in == in_idx && sel_out == output_idx);

            render_matrix_cell(
                d,
                entity.clone(),
                plugin_idx,
                state.plugin_instance_id,
                in_idx,
                output_idx,
                state.input_channels,
                state.output_channels,
                gain,
                is_selected,
                geometry.cell_size,
                theme,
            )
        }))
}

/// Render a single matrix cell
#[expect(
    clippy::too_many_lines,
    reason = "Keep the declarative layout and its interaction handlers together"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
fn render_matrix_cell(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    plugin_instance_id: usize,
    input_idx: usize,
    output_idx: usize,
    input_count: usize,
    output_count: usize,
    gain: f32,
    is_selected: bool,
    cell_size: f32,
    theme: &Theme,
) -> impl IntoElement {
    let gain_db = format_gain_db(gain);
    let param_idx = cell_index(input_idx, output_idx, input_count);

    // Color intensity based on absolute gain (0 = dark, 1 = bright)
    let intensity = gain.abs().clamp(0.0, 1.0);
    let is_active = gain.abs() > 0.001;
    let is_negative = gain < -0.001;

    // Background color: interpolate from surface to accent/warning based on intensity
    // Negative gains (for M/S) use warning color
    let bg_color = if is_selected {
        theme.accent_muted
    } else if is_active {
        let target_color = if is_negative {
            theme.warning // Orange/yellow for negative (inverted polarity)
        } else {
            theme.accent
        };
        // Blend surface toward target based on intensity
        Rgba {
            r: theme.surface.r + (target_color.r - theme.surface.r) * intensity * 0.5,
            g: theme.surface.g + (target_color.g - theme.surface.g) * intensity * 0.5,
            b: theme.surface.b + (target_color.b - theme.surface.b) * intensity * 0.5,
            a: 1.0,
        }
    } else {
        theme.surface
    };

    let entity_click = entity.clone();
    let entity_scroll = entity.clone();
    let entity_keyboard = entity.clone();
    let focus_color = theme.border_focused;

    let cell = div()
        .id(ElementId::Name(
            format!("matrix-cell-{plugin_idx}-{input_idx}-{output_idx}").into(),
        ))
        .w(px(cell_size))
        .flex_shrink_0()
        .h(px(cell_size))
        .flex()
        .items_center()
        .justify_center()
        .m_px()
        .rounded(d.r_md)
        .cursor_pointer()
        .bg(bg_color)
        .border_1()
        .border_color(if is_selected {
            theme.accent
        } else if is_active {
            theme.border
        } else {
            theme.background_secondary
        })
        .hover(|s| s.border_color(theme.accent))
        .focusable()
        .focus_visible(move |s| s.border_1().border_color(focus_color))
        .text_size(d.text_xs)
        .font_weight(if is_active {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .text_color(if is_active {
            theme.text_primary
        } else {
            theme.text_muted
        })
        .child(gain_db)
        // Click handling
        .on_mouse_down(MouseButton::Left, move |event, _, cx| {
            entity_click.update(cx, |state, _| {
                if event.click_count >= 2 {
                    // Double-click to reset cell to 0 and clear M/S/D for that output channel
                    if let Some(settings) = state
                        .app
                        .plugin_state
                        .editor_settings_mut_by_instance_id(plugin_instance_id)
                        && let sotf_audio_player::PluginSettings::Matrix {
                            input_channels,
                            output_channels,
                            matrix,
                            channel_states,
                            ..
                        } = settings
                        && let Some(idx) = checked_matrix_cell_index(
                            input_idx,
                            output_idx,
                            *input_channels,
                            *output_channels,
                            matrix.len(),
                        )
                    {
                        matrix[idx] = 0.0;
                        if output_idx < channel_states.len() {
                            channel_states[output_idx] = sotf_plugins::ChannelState::default();
                        }
                        state
                            .app
                            .plugin_state
                            .record_editor_effect(PluginUpdateEffect::Structural);
                    }
                } else {
                    // Single click only selects the cell. Gain changes remain
                    // available through the keyboard parameter controls and
                    // the scroll wheel, so an accidental click cannot destroy
                    // a carefully trimmed value.
                    state.app.plugin_state.matrix_selected_cell =
                        Some((plugin_instance_id, input_idx, output_idx));
                    state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                    state.app.plugin_state.plugin_param_selection = param_idx;
                }
            });
        })
        .on_key_down(move |event, _, cx| {
            let key = event.keystroke.key.as_str();
            let (next_input, next_output) = if matches!(key, "enter" | "space") {
                (input_idx, output_idx)
            } else if let Some(next) =
                matrix_navigation(input_idx, output_idx, input_count, output_count, key)
            {
                next
            } else {
                return;
            };

            entity_keyboard.update(cx, |state, _| {
                state.app.plugin_state.matrix_selected_cell =
                    Some((plugin_instance_id, next_input, next_output));
                state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                state.app.plugin_state.plugin_param_selection =
                    cell_index(next_input, next_output, input_count);
            });
            cx.stop_propagation();
        })
        // Ordinary scrolling belongs to the editor; Alt-wheel adjusts gain.
        .on_scroll_wheel(move |event, _, cx| {
            if !event.modifiers.alt {
                return;
            }
            cx.stop_propagation();
            entity_scroll.update(cx, |state, _| {
                if let Some(settings) = state
                    .app
                    .plugin_state
                    .editor_settings_mut_by_instance_id(plugin_instance_id)
                    && let sotf_audio_player::PluginSettings::Matrix {
                        input_channels,
                        output_channels,
                        matrix,
                        ..
                    } = settings
                    && let Some(idx) = checked_matrix_cell_index(
                        input_idx,
                        output_idx,
                        *input_channels,
                        *output_channels,
                        matrix.len(),
                    )
                {
                    // Get scroll direction (up = increase, down = decrease)
                    let delta: f32 = match event.delta {
                        gpui::ScrollDelta::Lines(lines) => lines.y * DB_STEP,
                        gpui::ScrollDelta::Pixels(pixels) => {
                            let y_px: f32 = pixels.y.into();
                            y_px / 20.0 * DB_STEP // 20 pixels per dB step
                        }
                    };

                    if delta.abs() > 0.01 {
                        let current_val = matrix[idx];
                        let sign = if current_val < 0.0 { -1.0 } else { 1.0 };
                        let abs_val = current_val.abs();

                        let current_db = if abs_val < 0.001 {
                            MIN_DB
                        } else {
                            20.0 * abs_val.log10()
                        };
                        let new_db = (current_db + delta).clamp(MIN_DB, MAX_DB);
                        // Preserve sign, apply new magnitude
                        matrix[idx] = sign * db_to_linear(new_db);
                        state
                            .app
                            .plugin_state
                            .record_editor_effect(PluginUpdateEffect::Structural);
                    }
                }
            });
        });
    #[cfg(feature = "dev-api")]
    let cell = {
        use crate::app::dev_api::DevTrackExt;
        cell.dev_track(format!("matrix.cell.{plugin_idx}.{input_idx}.{output_idx}"))
    };
    cell
}

fn matrix_navigation(
    input_idx: usize,
    output_idx: usize,
    input_count: usize,
    output_count: usize,
    key: &str,
) -> Option<(usize, usize)> {
    let next = match key {
        "left" if input_idx > 0 => (input_idx - 1, output_idx),
        "right" if input_idx + 1 < input_count => (input_idx + 1, output_idx),
        "up" if output_idx > 0 => (input_idx, output_idx - 1),
        "down" if output_idx + 1 < output_count => (input_idx, output_idx + 1),
        _ => return None,
    };
    Some(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_geometry_compresses_dense_channel_layouts() {
        let matrix = [0.0_f32; 16];
        let channel_states = [sotf_plugins::ChannelState::default(); 4];
        let state = MatrixRenderState {
            route_menu: None,
            plugin_instance_id: 1,
            input_channels: 16,
            output_channels: 16,
            available_width: 640.0,
            layout_scale: 1.0,
            matrix: &matrix,
            channel_states: &channel_states,
            speaker_config: None,
            is_editing: false,
            selected_param: 0,
            selected_cell: None,
        };

        let geometry = matrix_geometry(&state);

        assert!(geometry.cell_size < CELL_SIZE);
        assert!(geometry.cell_size >= 24.0);
    }

    #[test]
    fn matrix_geometry_tracks_zoom_for_readable_controls() {
        let matrix = [0.0_f32; 4];
        let channel_states = [sotf_plugins::ChannelState::default(); 2];
        let state = MatrixRenderState {
            route_menu: None,
            plugin_instance_id: 1,
            input_channels: 2,
            output_channels: 2,
            available_width: 1200.0,
            layout_scale: 1.5,
            matrix: &matrix,
            channel_states: &channel_states,
            speaker_config: None,
            is_editing: false,
            selected_param: 0,
            selected_cell: None,
        };

        let geometry = matrix_geometry(&state);

        assert_eq!(geometry.cell_size, 72.0);
        assert_eq!(geometry.label_width, LABEL_WIDTH * 1.5);
    }

    #[test]
    fn matrix_navigation_moves_within_grid_without_wrapping() {
        assert_eq!(matrix_navigation(1, 1, 3, 3, "left"), Some((0, 1)));
        assert_eq!(matrix_navigation(1, 1, 3, 3, "right"), Some((2, 1)));
        assert_eq!(matrix_navigation(1, 1, 3, 3, "up"), Some((1, 0)));
        assert_eq!(matrix_navigation(1, 1, 3, 3, "down"), Some((1, 2)));
        assert_eq!(matrix_navigation(0, 0, 3, 3, "left"), None);
        assert_eq!(matrix_navigation(2, 2, 3, 3, "down"), None);
        assert_eq!(matrix_navigation(1, 1, 3, 3, "tab"), None);
    }
}
