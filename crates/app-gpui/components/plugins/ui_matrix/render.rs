// intentional-file: fixed pixel values here are graph and plugin control geometry.
use super::super::common::ParamSectionStyle;
use super::consts::CELL_SIZE;
use super::consts::DB_STEP;
use super::consts::LABEL_WIDTH;
use super::consts::MAX_DB;
use super::consts::MIN_DB;
use super::consts::MSD_BTN_SIZE;
use super::consts::MSD_COL_WIDTH;
use super::misc::compute_output_groups;
use super::misc::format_gain_db;
use super::misc::{cell_index, checked_matrix_cell_index, matrix_settings_mut_by_instance_id};
use super::types::MatrixRenderState;
use super::types::MsdAction;
use crate::app::AppState;
use crate::app::types::PluginUpdateType;
use crate::components::design::Ds;
use crate::components::themed_tooltip;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{ButtonSet, ButtonSetOption, ButtonSetSize, NumberInput, NumberInputSize};
use sotf_audio_player::{
    apply_matrix_preset, available_matrix_presets, db_to_linear, detect_matrix_preset,
    get_channel_label_from_config,
};

#[derive(Clone, Copy)]
struct MatrixGeometry {
    cell_size: f32,
    label_width: f32,
    msd_col_width: f32,
    msd_button_size: f32,
}

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
pub fn render_matrix_plugin(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: MatrixRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let preset_name =
        detect_matrix_preset(state.input_channels, state.output_channels, state.matrix);
    let geometry = matrix_geometry(&state);

    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(d.section)
        // Preset buttons
        .child(render_preset_buttons(
            entity.clone(),
            state.plugin_instance_id,
            state.input_channels,
            state.output_channels,
            preset_name,
            theme,
        ))
        .children(render_selected_cell_editor(
            d,
            entity.clone(),
            &state,
            theme,
        ))
        // Matrix grid
        .child(
            div()
                .id("matrix-grid-scroll")
                .w_full()
                .min_w_0()
                .overflow_x_scroll()
                .child(render_matrix_grid(
                    d,
                    entity.clone(),
                    plugin_idx,
                    &state,
                    geometry,
                    theme,
                )),
        )
}

fn render_selected_cell_editor(
    d: &Ds,
    entity: Entity<AppState>,
    state: &MatrixRenderState,
    theme: &Theme,
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

    Some(
        div()
            .flex()
            .items_center()
            .gap(d.gap)
            .child(
                div()
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_secondary)
                    .child(format!("{output_label} ← {input_label}")),
            )
            .child(
                NumberInput::new("matrix-selected-gain")
                    .value(current_db as f64)
                    .min(MIN_DB as f64)
                    .max(MAX_DB as f64)
                    .step(0.1)
                    .decimals(1)
                    .unit("dB")
                    .size(NumberInputSize::Xs)
                    .width(96.0)
                    .aria_label(format!("{output_label} ← {input_label}"))
                    .on_change(move |value, _window, cx| {
                        entity.update(cx, |state, _| {
                            if state.app.plugin_state.matrix_selected_cell
                                != Some((plugin_instance_id, input_idx, output_idx))
                            {
                                return;
                            }
                            if let Some(settings) = matrix_settings_mut_by_instance_id(
                                &mut state.app.plugin_state.graph,
                                plugin_instance_id,
                            ) && let sotf_audio_player::PluginSettings::Matrix {
                                input_channels,
                                output_channels,
                                matrix,
                                ..
                            } = settings
                                && let Some(index) = checked_matrix_cell_index(
                                    input_idx,
                                    output_idx,
                                    *input_channels,
                                    *output_channels,
                                    matrix.len(),
                                )
                            {
                                matrix[index] = polarity * db_to_linear(value as f32);
                                state.app.plugin_state.update_state.pending_plugin_update =
                                    Some(PluginUpdateType::Structural);
                            }
                        });
                    }),
            )
            .into_any_element(),
    )
}

/// Render preset buttons using ButtonSet
fn render_preset_buttons(
    entity: Entity<AppState>,
    plugin_instance_id: usize,
    input_channels: usize,
    output_channels: usize,
    current_preset: &str,
    theme: &Theme,
) -> impl IntoElement {
    let presets = available_matrix_presets(input_channels, output_channels);
    let ms_disabled = input_channels > 2;
    let current = current_preset.to_string();

    let options: Vec<ButtonSetOption> = presets
        .into_iter()
        .map(|preset| {
            let is_ms = preset == "M/S Encode" || preset == "M/S Decode";
            ButtonSetOption::new(preset, preset).disabled(is_ms && ms_disabled)
        })
        .collect();

    ButtonSet::new("matrix-preset")
        .options(options)
        .selected(current)
        .size(ButtonSetSize::Sm)
        .theme(theme.to_button_set_theme())
        .on_change(move |value, _window, cx| {
            let preset_name = value.to_string();
            entity.update(cx, |state, _| {
                if let Some(settings) = matrix_settings_mut_by_instance_id(
                    &mut state.app.plugin_state.graph,
                    plugin_instance_id,
                ) && let sotf_audio_player::PluginSettings::Matrix {
                    input_channels: in_ch,
                    output_channels: out_ch,
                    matrix,
                    ..
                } = settings
                {
                    apply_matrix_preset(*in_ch, *out_ch, matrix, &preset_name);
                    state.app.plugin_state.update_state.pending_plugin_update =
                        Some(PluginUpdateType::Structural);
                }
            });
        })
}

/// Render the matrix grid with input columns and output rows, plus M/S/D sidebar
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

    div()
        .flex()
        .gap(d.gap)
        .param_section_style_lg(d, theme)
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

    div()
        .id(ElementId::Name(
            format!(
                "msd-{}-{}-{:?}",
                plugin_idx,
                channels.first().copied().unwrap_or(0),
                label
            )
            .into(),
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
                if let Some(settings) = matrix_settings_mut_by_instance_id(
                    &mut state.app.plugin_state.graph,
                    plugin_instance_id,
                ) && let sotf_audio_player::PluginSettings::Matrix {
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
                    state.app.plugin_state.update_state.pending_plugin_update =
                        Some(PluginUpdateType::Structural);
                }
            });
        })
}

/// Render a single row of the matrix grid
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

    div()
        .id(ElementId::Name(
            format!("matrix-cell-{}-{}-{}", plugin_idx, input_idx, output_idx).into(),
        ))
        .w(px(cell_size))
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
                    if let Some(settings) = matrix_settings_mut_by_instance_id(
                        &mut state.app.plugin_state.graph,
                        plugin_instance_id,
                    ) && let sotf_audio_player::PluginSettings::Matrix {
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
                        state.app.plugin_state.update_state.pending_plugin_update =
                            Some(PluginUpdateType::Structural);
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
        // Scroll to adjust value (preserving sign for negative gains)
        .on_scroll_wheel(move |event, _, cx| {
            entity_scroll.update(cx, |state, _| {
                if let Some(settings) = matrix_settings_mut_by_instance_id(
                    &mut state.app.plugin_state.graph,
                    plugin_instance_id,
                ) && let sotf_audio_player::PluginSettings::Matrix {
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
                        state.app.plugin_state.update_state.pending_plugin_update =
                            Some(PluginUpdateType::Structural);
                    }
                }
            });
        })
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
