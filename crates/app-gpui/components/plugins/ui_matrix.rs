//! Matrix Plugin UI Component
//!
//! Channel routing/mixing matrix with:
//! - Interactive grid visualization (inputs as columns, outputs as rows)
//! - dB display for gain values
//! - Click to toggle, scroll to adjust
//! - Preset buttons (Identity, Swap L/R, Mono Mix)

use super::common::{ParamSectionStyle, render_section_header};
use crate::app::AppState;
use crate::app::types::PluginUpdateType;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::{
    apply_matrix_preset, available_matrix_presets, db_to_linear, detect_matrix_preset,
    get_channel_label_from_config,
};

// Constants for matrix cell sizing
const CELL_SIZE: f32 = 48.0;
const LABEL_WIDTH: f32 = 36.0;
const MIN_DB: f32 = -60.0;
const MAX_DB: f32 = 6.0;
const DB_STEP: f32 = 1.0; // dB per scroll step

/// State for rendering the Matrix plugin
pub struct MatrixRenderState<'a> {
    pub input_channels: usize,
    pub output_channels: usize,
    pub matrix: &'a [f32],
    pub channel_states: &'a [sotf_plugins::ChannelState],
    pub speaker_config: Option<String>,
    pub is_editing: bool,
    pub selected_param: usize,
    /// Currently selected cell (input_idx, output_idx) for editing
    pub selected_cell: Option<(usize, usize)>,
}

/// Convert linear gain to dB string for display
/// Supports negative gains (for M/S encoding) by showing with minus sign prefix
fn format_gain_db(linear: f32) -> String {
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
fn cell_index(input_idx: usize, output_idx: usize, input_count: usize) -> usize {
    output_idx * input_count + input_idx
}

/// Render the Matrix plugin
pub fn render_matrix_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: MatrixRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let preset_name =
        detect_matrix_preset(state.input_channels, state.output_channels, state.matrix);

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Header with preset selector
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .param_section_style(theme)
                .child(render_section_header("MATRIX MIXER", theme))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child("Preset:"),
                        )
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(theme.surface)
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text_primary)
                                .child(preset_name),
                        ),
                ),
        )
        // Preset buttons
        .child(render_preset_buttons(
            entity.clone(),
            plugin_idx,
            state.input_channels,
            state.output_channels,
            preset_name,
            theme,
        ))
        // Matrix grid
        .child(render_matrix_grid(
            entity.clone(),
            plugin_idx,
            &state,
            theme,
        ))
        // Info footer
        .child(
            div()
                .flex()
                .justify_between()
                .text_xs()
                .text_color(theme.text_muted)
                .param_section_style(theme)
                .child(format!(
                    "{} inputs \u{2192} {} outputs",
                    state.input_channels, state.output_channels
                ))
                .child("Click: toggle | Scroll: adjust | Double-click: reset"),
        )
}

/// Render preset buttons
fn render_preset_buttons(
    entity: Entity<AppState>,
    plugin_idx: usize,
    input_channels: usize,
    output_channels: usize,
    current_preset: &str,
    theme: &Theme,
) -> impl IntoElement {
    let presets = available_matrix_presets(input_channels, output_channels);

    div()
        .flex()
        .gap_2()
        .children(presets.into_iter().map(|preset| {
            let is_active = current_preset == preset;
            let entity_clone = entity.clone();
            let preset_owned = preset.to_string();

            div()
                .px_3()
                .py_1()
                .rounded_lg()
                .cursor_pointer()
                .bg(if is_active {
                    theme.accent
                } else {
                    theme.surface
                })
                .border_1()
                .border_color(if is_active {
                    theme.accent
                } else {
                    theme.border
                })
                .text_sm()
                .text_color(if is_active {
                    theme.text_on_accent
                } else {
                    theme.text_secondary
                })
                .hover(|s| {
                    s.bg(if is_active {
                        theme.accent
                    } else {
                        theme.surface_hover
                    })
                })
                .child(preset)
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    entity_clone.update(cx, |state, _| {
                        // Get current matrix and apply preset
                        if let Some(plugin) =
                            state.app.plugin_state.chain.get_plugin_mut(plugin_idx)
                            && let sotf_audio_player::PluginSettings::Matrix {
                                input_channels: in_ch,
                                output_channels: out_ch,
                                ref mut matrix,
                                ..
                            } = plugin.settings
                            {
                                apply_matrix_preset(in_ch, out_ch, matrix, &preset_owned);
                                state.app.plugin_state.pending_plugin_update =
                                    Some(PluginUpdateType::Structural);
                            }
                    });
                })
        }))
}

/// Compute output channel groups from MeterGroupSpec
fn compute_output_groups(
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

/// M/S/D button width
const MSD_BTN_SIZE: f32 = 20.0;
/// M/S/D column total width (3 buttons + gaps + padding)
const MSD_COL_WIDTH: f32 = 80.0;

/// Render the matrix grid with input columns and output rows, plus M/S/D sidebar
fn render_matrix_grid(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &MatrixRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let output_groups =
        compute_output_groups(state.output_channels, state.speaker_config.as_deref());

    div()
        .flex()
        .gap_2()
        .param_section_style_lg(theme)
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
                                .w(px(LABEL_WIDTH))
                                .h(px(24.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_xs()
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
                                .w(px(CELL_SIZE))
                                .h(px(24.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.text_secondary)
                                .child(label)
                        })),
                )
                // Grid rows
                .children((0..state.output_channels).map(|out_idx| {
                    render_matrix_row(entity.clone(), plugin_idx, out_idx, state, theme)
                })),
        )
        // Right: M/S/D sidebar
        .child(render_msd_sidebar(
            entity.clone(),
            plugin_idx,
            state,
            &output_groups,
            theme,
        ))
}

/// Render the M/S/D sidebar column
fn render_msd_sidebar(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &MatrixRenderState,
    output_groups: &[(String, Vec<usize>)],
    theme: &Theme,
) -> impl IntoElement {
    // Row height for each output channel: CELL_SIZE + 2px margin (m_px on each side)
    let row_height = CELL_SIZE + 2.0;

    div()
        .flex()
        .flex_col()
        .w(px(MSD_COL_WIDTH))
        // Header spacer to align with column header row
        .child(
            div()
                .h(px(24.0))
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
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
                .gap_1()
                .child(render_msd_button(
                    "M",
                    any_muted,
                    theme.error,
                    theme,
                    plugin_idx,
                    entity.clone(),
                    channels.clone(),
                    output_channels,
                    MsdAction::Mute,
                ))
                .child(render_msd_button(
                    "S",
                    any_soloed,
                    theme.warning,
                    theme,
                    plugin_idx,
                    entity.clone(),
                    channels.clone(),
                    output_channels,
                    MsdAction::Solo,
                ))
                .child(render_msd_button(
                    "D",
                    any_dimmed,
                    theme.info,
                    theme,
                    plugin_idx,
                    entity.clone(),
                    channels.clone(),
                    output_channels,
                    MsdAction::Dim,
                ));

            let mut container = div()
                .h(px(group_height))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_1()
                .border_l_1()
                .border_color(theme.border)
                .pl_2();

            if has_label {
                container = container.child(
                    div()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child(group_name.clone()),
                );
            }

            container.child(buttons)
        }))
}

#[derive(Clone, Copy)]
enum MsdAction {
    Mute,
    Solo,
    Dim,
}

/// Render a single M, S, or D button
fn render_msd_button(
    label: &'static str,
    is_active: bool,
    active_color: Rgba,
    theme: &Theme,
    plugin_idx: usize,
    entity: Entity<AppState>,
    channels: Vec<usize>,
    output_channels: usize,
    action: MsdAction,
) -> impl IntoElement {
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
        .w(px(MSD_BTN_SIZE))
        .h(px(18.0))
        .rounded_sm()
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
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .text_color(if is_active {
            theme.text_on_accent
        } else {
            theme.text_muted
        })
        .child(label)
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            entity.update(cx, |state, _| {
                if let Some(plugin) = state.app.plugin_state.chain.get_plugin_mut(plugin_idx)
                    && let sotf_audio_player::PluginSettings::Matrix {
                        ref mut channel_states,
                        output_channels: out_ch,
                        ..
                    } = plugin.settings
                    {
                        // Resize channel_states if needed
                        let target_len = out_ch.max(output_channels);
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
                        state.app.plugin_state.pending_plugin_update =
                            Some(PluginUpdateType::Structural);
                    }
            });
        })
}

/// Render a single row of the matrix grid
fn render_matrix_row(
    entity: Entity<AppState>,
    plugin_idx: usize,
    output_idx: usize,
    state: &MatrixRenderState,
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
                .w(px(LABEL_WIDTH))
                .h(px(CELL_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
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
                entity.clone(),
                plugin_idx,
                in_idx,
                output_idx,
                state.input_channels,
                gain,
                is_selected,
                theme,
            )
        }))
}

/// Render a single matrix cell
fn render_matrix_cell(
    entity: Entity<AppState>,
    plugin_idx: usize,
    input_idx: usize,
    output_idx: usize,
    input_count: usize,
    gain: f32,
    is_selected: bool,
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

    div()
        .id(ElementId::Name(
            format!("matrix-cell-{}-{}-{}", plugin_idx, input_idx, output_idx).into(),
        ))
        .w(px(CELL_SIZE))
        .h(px(CELL_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .m_px()
        .rounded_md()
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
        .text_xs()
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
                    if let Some(plugin) = state.app.plugin_state.chain.get_plugin_mut(plugin_idx)
                        && let sotf_audio_player::PluginSettings::Matrix {
                            input_channels,
                            ref mut matrix,
                            ref mut channel_states,
                            ..
                        } = plugin.settings
                        {
                            let idx = cell_index(input_idx, output_idx, input_channels);
                            if idx < matrix.len() {
                                matrix[idx] = 0.0;
                            }
                            if output_idx < channel_states.len() {
                                channel_states[output_idx] = sotf_plugins::ChannelState::default();
                            }
                            state.app.plugin_state.pending_plugin_update =
                                Some(PluginUpdateType::Structural);
                        }
                } else {
                    // Single click: select and toggle between 0 and 1
                    state.app.plugin_state.matrix_selected_cell = Some((input_idx, output_idx));
                    state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                    state.app.plugin_state.plugin_param_selection = param_idx;

                    // Toggle gain
                    if let Some(plugin) = state.app.plugin_state.chain.get_plugin_mut(plugin_idx)
                        && let sotf_audio_player::PluginSettings::Matrix {
                            input_channels,
                            ref mut matrix,
                            ..
                        } = plugin.settings
                        {
                            let idx = cell_index(input_idx, output_idx, input_channels);
                            if idx < matrix.len() {
                                // Toggle: if > 0.5, set to 0; otherwise set to 1
                                matrix[idx] = if matrix[idx] > 0.5 { 0.0 } else { 1.0 };
                                state.app.plugin_state.pending_plugin_update =
                                    Some(PluginUpdateType::Structural);
                            }
                        }
                }
            });
        })
        // Scroll to adjust value (preserving sign for negative gains)
        .on_scroll_wheel(move |event, _, cx| {
            entity_scroll.update(cx, |state, _| {
                if let Some(plugin) = state.app.plugin_state.chain.get_plugin_mut(plugin_idx)
                    && let sotf_audio_player::PluginSettings::Matrix {
                        input_channels,
                        ref mut matrix,
                        ..
                    } = plugin.settings
                    {
                        let idx = cell_index(input_idx, output_idx, input_channels);
                        if idx < matrix.len() {
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
                                state.app.plugin_state.pending_plugin_update =
                                    Some(PluginUpdateType::Structural);
                            }
                        }
                    }
            });
        })
}
