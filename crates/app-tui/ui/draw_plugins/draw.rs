use super::super::*;
use super::misc::format_channel_config;
use super::types::ParamDisplayEntry;
use super::types::get_plugin_parameters;

pub(crate) fn draw_plugins_screen(f: &mut Frame, area: Rect, app: &App) {
    // Split vertically: help box on top, plugin panels below
    let vchunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Help box
            Constraint::Min(0),    // Plugin panels
        ])
        .split(area);

    // Draw help box with contextual text
    if app.input_mode == InputMode::AddPlugin {
        draw_help_box_with_text(f, vchunks[0], app, "↑/↓=navigate  Enter=add  Esc=cancel");
    } else if app.plugin_graph.is_empty() {
        draw_help_box_with_text(f, vchunks[0], app, "'a'=add plugins  's'=save  'l'=load");
    } else {
        draw_help_box(f, vchunks[0], app, Screen::Plugins);
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30), // Plugin chain
            Constraint::Percentage(70), // Available plugins
        ])
        .split(vchunks[1]);

    // Plugin chain list
    draw_plugin_list(f, chunks[0], app);

    // Available plugins list
    draw_available_plugins(f, chunks[1], app);
}

pub(crate) fn draw_plugin_list(f: &mut Frame, area: Rect, app: &App) {
    // Non-linear topology: `plugins()` (linear-only) returns empty, so the
    // user would see a misleading "0 plugins" rack. Show a banner instead
    // — same affordance as the GPUI "Open Graph View" card. The TUI has no
    // node-graph canvas, so editing has to happen in the GPUI app.
    if !app.plugin_graph.is_linear() {
        let node_count = app.plugin_graph.nodes.len();
        let conn_count = app.plugin_graph.connections.len();
        let msg = format!(
            "Graph mode: {} plugins, {} connections.\nNon-linear topology (parallel branches).\nUse the desktop app (sotf-desktop) to edit nodes and connections visually.",
            node_count, conn_count
        );
        let para = Paragraph::new(msg)
            .style(Style::default().fg(app.theme.fg_secondary))
            .wrap(ratatui::widgets::Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Plugin chain (graph)"),
            );
        f.render_widget(para, area);
        return;
    }

    let items: Vec<ListItem> = app
        .plugin_graph
        .plugins()
        .iter()
        .enumerate()
        .map(|(i, plugin)| {
            let enabled_marker = if plugin.enabled { "●" } else { "○" };
            let custom_name = plugin.name.clone();
            let fallback: &str = if app.plugin_graph.is_input_monitor(i) {
                "Loudness Monitor Input"
            } else if app.plugin_graph.is_output_monitor(i) {
                "Loudness Monitor Output"
            } else if plugin.permanent && matches!(plugin.plugin_type(), PluginType::Gain) {
                "Replay Gain"
            } else {
                plugin.plugin_type().name()
            };
            let display_name: String = custom_name
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| fallback.to_string());
            let content = format!("{} {} - {}", enabled_marker, i + 1, display_name);

            let style = if plugin.enabled {
                Style::default().fg(app.theme.accent_success)
            } else {
                Style::default().fg(app.theme.fg_muted)
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let title = if app.plugin_graph.is_empty() {
        "0 plugins".to_string()
    } else {
        format!(
            "{} plugins ({} ch)",
            app.plugin_graph.len(),
            app.plugin_graph.output_channels()
        )
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .fg(app.theme.fg_selected)
                .bg(app.theme.bg_selected)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    if !app.plugin_graph.is_empty() {
        state.select(Some(app.selected_plugin_index));
    }

    use ratatui::widgets::StatefulWidget;
    StatefulWidget::render(list, area, f.buffer_mut(), &mut state);
}

pub(crate) fn draw_available_plugins(f: &mut Frame, area: Rect, app: &App) {
    let is_selecting = app.input_mode == InputMode::AddPlugin;

    // Walk the shared category list. For each category emit a non-selectable
    // header row, then one row per plugin. Track the mapping from "selectable
    // index" (what `add_plugin_selected_index` points at) to "display index"
    // (the row in the rendered list) so highlighting lines up.
    let mut items: Vec<ListItem> = Vec::new();
    let mut display_for_selectable: Vec<usize> = Vec::new();

    for cat in sotf_audio_player::plugin_categories::CATEGORIES {
        items.push(
            ListItem::new(format!("── {} ──", cat.name)).style(
                Style::default()
                    .fg(app.theme.fg_secondary)
                    .add_modifier(Modifier::BOLD),
            ),
        );
        for plugin_type in cat.plugins {
            display_for_selectable.push(items.len());
            let content = format!("  {} - {}", plugin_type.name(), plugin_type.description());
            items.push(ListItem::new(content).style(Style::default().fg(app.theme.accent_primary)));
        }
    }

    let title = if is_selecting {
        "▶ Select Plugin (↑/↓, Enter=add, Esc=cancel)"
    } else {
        "Available Plugins"
    };

    let border_style = if is_selecting {
        Style::default().fg(app.theme.accent_primary)
    } else {
        Style::default().fg(app.theme.fg_secondary)
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title),
        )
        .highlight_style(
            Style::default()
                .fg(app.theme.fg_selected)
                .bg(app.theme.bg_selected)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    if is_selecting && let Some(&row) = display_for_selectable.get(app.add_plugin_selected_index) {
        state.select(Some(row));
    }

    use ratatui::widgets::StatefulWidget;
    StatefulWidget::render(list, area, f.buffer_mut(), &mut state);
}

pub(crate) fn draw_plugin_editor_modal(f: &mut Frame, app: &App) {
    if let Some(plugin) = app.get_editing_plugin() {
        // Check if we're editing a Matrix plugin - use specialized editor
        if matches!(plugin.settings, PluginSettings::Matrix { .. }) {
            draw_matrix_editor_modal(f, app);
            return;
        }

        // Create a centered modal (60% width, 80% height)
        let area = f.area();
        let modal_width = (area.width as f32 * 0.6) as u16;
        let modal_height = (area.height as f32 * 0.8) as u16;
        let modal_x = (area.width - modal_width) / 2;
        let modal_y = (area.height - modal_height) / 2;

        let modal_area = Rect {
            x: modal_x,
            y: modal_y,
            width: modal_width,
            height: modal_height,
        };

        // Clear the background
        f.render_widget(Clear, modal_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .style(
                Style::default()
                    .bg(app.theme.bg_primary)
                    .fg(app.theme.fg_primary),
            )
            .title(format!(
                "Edit {} Plugin (ESC to close)",
                plugin.plugin_type().name()
            ));

        f.render_widget(block, modal_area);

        // Inner area for parameters
        let inner = Rect {
            x: modal_area.x + 1,
            y: modal_area.y + 1,
            width: modal_area.width.saturating_sub(2),
            height: modal_area.height.saturating_sub(2),
        };

        let base_style = Style::default()
            .bg(app.theme.bg_primary)
            .fg(app.theme.fg_primary);

        // Build parameter list
        let mut lines = Vec::new();
        lines.push(Line::from(vec![
            Span::styled("Use ", base_style),
            Span::styled("↑/↓", base_style.fg(app.theme.accent_primary)),
            Span::styled(" to select parameter, ", base_style),
            Span::styled("←/→", base_style.fg(app.theme.accent_primary)),
            Span::styled(" to adjust value", base_style),
        ]));
        lines.push(Line::from(Span::styled("", base_style)));

        let entries = get_plugin_parameters(&plugin.settings, app.plugin_param_selection);

        // Compute the max label width for right-alignment (only from Param entries)
        let max_label_width = entries
            .iter()
            .filter_map(|e| match e {
                ParamDisplayEntry::Param(name, _) => Some(name.len()),
                ParamDisplayEntry::Separator(_) => None,
            })
            .max()
            .unwrap_or(0);

        let inner_width = inner.width as usize;
        let mut param_index = 0usize;
        let mut selected_line_index = 0usize; // display line of selected param
        for entry in &entries {
            match entry {
                ParamDisplayEntry::Separator(title) => {
                    // Render as a separator line: ── Title ──────
                    let prefix = "\u{2500}\u{2500} ";
                    let suffix_char = '\u{2500}';
                    let label = format!("{}{} ", prefix, title);
                    let remaining = inner_width.saturating_sub(label.len());
                    let separator_line = format!(
                        "{}{}",
                        label,
                        std::iter::repeat_n(suffix_char, remaining).collect::<String>()
                    );
                    lines.push(Line::from(Span::styled(
                        separator_line,
                        base_style.fg(app.theme.fg_secondary),
                    )));
                }
                ParamDisplayEntry::Param(name, value) => {
                    if param_index == app.plugin_param_selection {
                        selected_line_index = lines.len();
                    }
                    let style = if param_index == app.plugin_param_selection {
                        Style::default()
                            .fg(app.theme.fg_selected)
                            .bg(app.theme.bg_selected)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        base_style
                    };

                    // Right-align the label by padding on the left
                    let padded_name = format!("{:>width$}", name, width = max_label_width + 1);

                    lines.push(Line::from(vec![
                        Span::styled(format!("{} ", padded_name), style),
                        Span::styled(value.to_string(), style.fg(app.theme.title_color)),
                    ]));
                    param_index += 1;
                }
            }
        }

        // Auto-scroll to keep selected parameter visible
        let visible_height = inner.height as usize;
        let scroll_offset = if selected_line_index >= visible_height {
            (selected_line_index - visible_height + 2) as u16
        } else {
            0
        };

        let paragraph = Paragraph::new(lines)
            .style(Style::default().fg(app.theme.fg_primary))
            .block(Block::default())
            .style(base_style)
            .scroll((scroll_offset, 0));

        f.render_widget(paragraph, inner);
    }
}

/// Specialized matrix editor modal with visual grid display
pub(crate) fn draw_matrix_editor_modal(f: &mut Frame, app: &App) {
    let Some(plugin) = app.get_editing_plugin() else {
        return;
    };

    let PluginSettings::Matrix {
        input_channels,
        output_channels,
        matrix,
        ..
    } = &plugin.settings
    else {
        return;
    };

    // Create a centered modal (70% width, 85% height)
    let area = f.area();
    let modal_width = (area.width as f32 * 0.7).min(80.0) as u16;
    let modal_height = (area.height as f32 * 0.85).min(35.0) as u16;
    let modal_x = (area.width - modal_width) / 2;
    let modal_y = (area.height - modal_height) / 2;

    let modal_area = Rect {
        x: modal_x,
        y: modal_y,
        width: modal_width,
        height: modal_height,
    };

    // Clear background
    f.render_widget(Clear, modal_area);

    let preset_name = detect_matrix_preset(*input_channels, *output_channels, matrix);

    // Outer block
    let block = Block::default()
        .borders(Borders::ALL)
        .style(
            Style::default()
                .bg(app.theme.bg_primary)
                .fg(app.theme.fg_primary),
        )
        .title(format!(" Matrix Mixer - {} (ESC to close) ", preset_name));
    f.render_widget(block, modal_area);

    // Inner area
    let inner = Rect {
        x: modal_area.x + 1,
        y: modal_area.y + 1,
        width: modal_area.width.saturating_sub(2),
        height: modal_area.height.saturating_sub(2),
    };

    // Split into header (5 lines) and grid sections
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Header section
            Constraint::Min(3),    // Grid section
            Constraint::Length(2), // Help line
        ])
        .split(inner);

    // === Header Section ===
    draw_matrix_header(
        f,
        app,
        chunks[0],
        *input_channels,
        *output_channels,
        preset_name,
    );

    // === Grid Section ===
    draw_matrix_grid(f, app, chunks[1], *input_channels, *output_channels, matrix);

    // === Help Line ===
    let help_text = match app.matrix_edit_mode {
        MatrixEditMode::Header => "↑↓: Select | ←→: Adjust | Tab: Grid Mode | Esc: Exit",
        MatrixEditMode::Grid => {
            "↑↓←→: Navigate | -/+: Adjust ±0.5dB | 0: Zero | 1: Unity | Tab: Header Mode | Esc: Exit"
        }
    };
    let help = Paragraph::new(help_text)
        .style(Style::default().fg(app.theme.fg_secondary))
        .alignment(Alignment::Center);
    f.render_widget(help, chunks[2]);
}

/// Draw the header section of matrix editor (input/output channels, preset)
pub(crate) fn draw_matrix_header(
    f: &mut Frame,
    app: &App,
    area: Rect,
    input_channels: usize,
    output_channels: usize,
    preset_name: &str,
) {
    let in_header = app.matrix_edit_mode == MatrixEditMode::Header;

    let mut lines = Vec::new();

    let base_style = Style::default().fg(app.theme.fg_primary);

    // Input channels line
    let input_style = if in_header && app.matrix_header_selection == 0 {
        Style::default()
            .fg(app.theme.fg_selected)
            .bg(app.theme.bg_selected)
            .add_modifier(Modifier::BOLD)
    } else {
        base_style
    };
    lines.push(Line::from(vec![
        Span::styled("  Input Channels:  ", base_style),
        Span::styled(
            format!("[{}]", input_channels),
            input_style.fg(app.theme.accent_primary),
        ),
        Span::styled(
            format!("  ({})", format_channel_config(input_channels)),
            Style::default().fg(app.theme.fg_secondary),
        ),
    ]));

    // Output channels line
    let output_style = if in_header && app.matrix_header_selection == 1 {
        Style::default()
            .fg(app.theme.fg_selected)
            .bg(app.theme.bg_selected)
            .add_modifier(Modifier::BOLD)
    } else {
        base_style
    };
    lines.push(Line::from(vec![
        Span::styled("  Output Channels: ", base_style),
        Span::styled(
            format!("[{}]", output_channels),
            output_style.fg(app.theme.accent_primary),
        ),
        Span::styled(
            format!("  ({})", format_channel_config(output_channels)),
            Style::default().fg(app.theme.fg_secondary),
        ),
    ]));

    // Preset line
    let preset_style = if in_header && app.matrix_header_selection == 2 {
        Style::default()
            .fg(app.theme.fg_selected)
            .bg(app.theme.bg_selected)
            .add_modifier(Modifier::BOLD)
    } else {
        base_style
    };
    lines.push(Line::from(vec![
        Span::styled("  Preset:          ", base_style),
        Span::styled(
            format!("[{}]", preset_name),
            preset_style.fg(app.theme.title_color),
        ),
    ]));

    lines.push(Line::from(""));

    let paragraph = Paragraph::new(lines).style(Style::default().fg(app.theme.fg_primary));
    f.render_widget(paragraph, area);
}

/// Draw the matrix grid with channel labels and dB values
pub(crate) fn draw_matrix_grid(
    f: &mut Frame,
    app: &App,
    area: Rect,
    input_channels: usize,
    output_channels: usize,
    matrix: &[f32],
) {
    let in_grid = app.matrix_edit_mode == MatrixEditMode::Grid;

    // Calculate column widths: first column for row labels, then one per input
    let label_width = 5u16; // "Out" label column
    let cell_width = 7u16; // Each gain cell (e.g., "-12.5" or "-∞")

    // Build header row (empty corner + input channel labels)
    let mut header_cells =
        vec![Cell::from("Out\\In").style(Style::default().fg(app.theme.fg_secondary))];
    for inp in 0..input_channels {
        let label = get_channel_label(inp, input_channels);
        header_cells.push(
            Cell::from(label).style(
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
        );
    }
    let header = Row::new(header_cells).height(1);

    // Build data rows
    let mut rows = Vec::new();
    for out in 0..output_channels {
        let mut cells = Vec::new();

        // Row label (output channel)
        let row_label = get_channel_label(out, output_channels);
        cells.push(
            Cell::from(row_label).style(
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
        );

        // Gain cells
        for inp in 0..input_channels {
            let gain = matrix
                .get(out * input_channels + inp)
                .copied()
                .unwrap_or(0.0);
            let db_str = linear_to_db_string(gain);

            let is_selected = in_grid && out == app.matrix_grid_row && inp == app.matrix_grid_col;
            let style = if is_selected {
                Style::default()
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.bg_selected)
                    .add_modifier(Modifier::BOLD)
            } else if gain > 0.999 && gain < 1.001 {
                // Unity gain - highlight
                Style::default().fg(app.theme.title_color)
            } else if gain < 0.001 {
                // Silent - dim
                Style::default().fg(app.theme.fg_secondary)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };

            cells.push(Cell::from(db_str).style(style));
        }

        rows.push(Row::new(cells).height(1));
    }

    // Column widths
    let mut widths = vec![Constraint::Length(label_width)];
    for _ in 0..input_channels {
        widths.push(Constraint::Length(cell_width));
    }

    let table = Table::new(rows, widths).header(header).block(
        Block::default()
            .borders(Borders::TOP)
            .title(" Matrix Grid "),
    );

    f.render_widget(table, area);
}

pub(crate) fn draw_save_plugins_dialog(f: &mut Frame, app: &App) {
    let area = f.area();
    let dialog_width = (area.width as f32 * 0.7) as u16;
    let ac_height = autocomplete_dropdown_height(app);
    let base_height = (area.height as f32 * 0.6) as u16;
    let dialog_height = base_height + ac_height;
    let dialog_x = (area.width - dialog_width) / 2;
    let dialog_y = (area.height.saturating_sub(dialog_height)) / 2;

    let dialog_area = Rect {
        x: dialog_x,
        y: dialog_y,
        width: dialog_width,
        height: dialog_height,
    };

    // Clear background
    let block = Block::default()
        .borders(Borders::ALL)
        .style(
            Style::default()
                .bg(app.theme.bg_primary)
                .fg(app.theme.fg_primary),
        )
        .title("Save Plugin Preset");

    f.render_widget(Clear, dialog_area);
    f.render_widget(block, dialog_area);

    // Inner area for content
    let inner = Rect {
        x: dialog_area.x + 1,
        y: dialog_area.y + 1,
        width: dialog_area.width.saturating_sub(2),
        height: dialog_area.height.saturating_sub(2),
    };

    // If user is typing, show filename input; otherwise show preset list
    if !app.plugin_file_input.is_empty() {
        // Manual filename entry mode
        let lines = vec![
            Line::from("Enter preset name (without .json extension):"),
            Line::from(vec![
                Span::styled("  Saved to: ", Style::default().fg(app.theme.fg_muted)),
                Span::styled(
                    "plugin_presets/",
                    Style::default()
                        .fg(app.theme.fg_muted)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("> ", Style::default().fg(app.theme.accent_primary)),
                Span::raw(&app.plugin_file_input),
                Span::styled("_", Style::default().fg(app.theme.accent_success)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Note: ", Style::default().fg(app.theme.title_color)),
                Span::raw(".json extension will be added automatically"),
            ]),
            Line::from("Press Enter to save, ESC to cancel"),
        ];

        let paragraph = Paragraph::new(lines)
            .style(Style::default().fg(app.theme.fg_primary))
            .block(Block::default())
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, inner);
    } else if app.available_plugin_presets.is_empty() {
        // No presets available - show instructions
        let lines = vec![
            Line::from("No existing presets found in plugin_presets directory"),
            Line::from(""),
            Line::from("Type a preset name to save"),
            Line::from(""),
            Line::from(vec![
                Span::styled("Note: ", Style::default().fg(app.theme.title_color)),
                Span::raw(".json extension will be added automatically"),
            ]),
            Line::from(""),
            Line::from("Press ESC to cancel"),
        ];

        let paragraph = Paragraph::new(lines)
            .style(Style::default().fg(app.theme.title_color))
            .block(Block::default())
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, inner);
    } else {
        // Show preset list - user can select one to overwrite or type a new name
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Existing Presets ", Style::default()),
                Span::styled(
                    "(↑/↓ to select, Enter to overwrite, or type new name)",
                    Style::default().fg(app.theme.fg_muted),
                ),
            ]),
            Line::from(""),
        ];

        // Add each preset to the list
        for (i, preset) in app.available_plugin_presets.iter().enumerate() {
            let is_selected = i == app.selected_preset_index;
            let style = if is_selected {
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let marker = if is_selected { "► " } else { "  " };
            lines.push(Line::from(vec![
                Span::styled(marker, style),
                Span::styled(preset, style),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Hint: ", Style::default().fg(app.theme.title_color)),
            Span::raw("Select and press Enter to overwrite, or type to create new preset"),
        ]));

        let paragraph = Paragraph::new(lines)
            .style(Style::default().fg(app.theme.fg_primary))
            .block(Block::default())
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, inner);
    }

    // Autocomplete dropdown below dialog
    if ac_height > 0 {
        let ac_area = Rect {
            x: dialog_area.x,
            y: dialog_area.y + base_height,
            width: dialog_area.width,
            height: ac_height,
        };
        render_autocomplete_dropdown(f, ac_area, app);
    }
}

pub(crate) fn draw_load_plugins_dialog(f: &mut Frame, app: &App) {
    let area = f.area();
    let dialog_width = (area.width as f32 * 0.7) as u16;
    let ac_height = autocomplete_dropdown_height(app);
    let base_height = (area.height as f32 * 0.6) as u16;
    let dialog_height = base_height + ac_height;
    let dialog_x = (area.width - dialog_width) / 2;
    let dialog_y = (area.height.saturating_sub(dialog_height)) / 2;

    let dialog_area = Rect {
        x: dialog_x,
        y: dialog_y,
        width: dialog_width,
        height: dialog_height,
    };

    // Clear background
    let block = Block::default()
        .borders(Borders::ALL)
        .style(
            Style::default()
                .bg(app.theme.bg_primary)
                .fg(app.theme.fg_primary),
        )
        .title("Load Plugin Preset");

    f.render_widget(Clear, dialog_area);
    f.render_widget(block, dialog_area);

    // Inner area for content
    let inner = Rect {
        x: dialog_area.x + 1,
        y: dialog_area.y + 1,
        width: dialog_area.width.saturating_sub(2),
        height: dialog_area.height.saturating_sub(2),
    };

    // If user is typing, show filename input; otherwise show preset list
    if !app.plugin_file_input.is_empty() {
        // Manual filename entry mode
        let lines = vec![
            Line::from("Enter filename (without .json extension):"),
            Line::from(""),
            Line::from(vec![
                Span::styled("> ", Style::default().fg(app.theme.accent_primary)),
                Span::raw(&app.plugin_file_input),
                Span::styled("_", Style::default().fg(app.theme.accent_success)),
            ]),
            Line::from(""),
            Line::from("Press Enter to load, ESC to cancel"),
        ];

        let paragraph = Paragraph::new(lines)
            .style(Style::default().fg(app.theme.fg_primary))
            .block(Block::default())
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, inner);
    } else if app.available_plugin_presets.is_empty() {
        // No presets available
        let lines = vec![
            Line::from("No presets found in plugin_presets directory"),
            Line::from(""),
            Line::from("You can:"),
            Line::from("  • Type a filename to load a preset"),
            Line::from("  • Press ESC to cancel"),
            Line::from("  • Save your first preset with 's' from the Plugins screen"),
        ];

        let paragraph = Paragraph::new(lines)
            .style(Style::default().fg(app.theme.title_color))
            .block(Block::default())
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, inner);
    } else {
        // Show preset list
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Available Presets ", Style::default()),
                Span::styled(
                    "(↑/↓ to select, Enter to load)",
                    Style::default().fg(app.theme.fg_muted),
                ),
            ]),
            Line::from(""),
        ];

        // Add preset items
        for (i, preset) in app.available_plugin_presets.iter().enumerate() {
            let is_selected = i == app.selected_preset_index;
            let style = if is_selected {
                Style::default()
                    .fg(app.theme.fg_selected)
                    .bg(app.theme.bg_selected)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.accent_primary)
            };

            let marker = if is_selected { "► " } else { "  " };
            lines.push(Line::from(vec![
                Span::styled(marker, style),
                Span::styled(preset, style),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(
            "Or type a filename to load manually, ESC to cancel",
        ));

        let paragraph = Paragraph::new(lines)
            .style(Style::default().fg(app.theme.fg_primary))
            .block(Block::default())
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, inner);
    }

    // Autocomplete dropdown below dialog
    if ac_height > 0 {
        let ac_area = Rect {
            x: dialog_area.x,
            y: dialog_area.y + base_height,
            width: dialog_area.width,
            height: ac_height,
        };
        render_autocomplete_dropdown(f, ac_area, app);
    }
}

pub(crate) fn draw_load_apo_file_dialog(f: &mut Frame, app: &App) {
    let area = f.area();
    let dialog_width = (area.width as f32 * 0.6) as u16;
    let ac_height = autocomplete_dropdown_height(app);
    let base_height: u16 = 10;
    let dialog_height = base_height + ac_height;
    let dialog_x = (area.width - dialog_width) / 2;
    let dialog_y = (area.height.saturating_sub(dialog_height)) / 2;

    let dialog_area = Rect {
        x: dialog_x,
        y: dialog_y,
        width: dialog_width,
        height: dialog_height,
    };

    // Clear background
    let block = Block::default()
        .borders(Borders::ALL)
        .style(
            Style::default()
                .bg(app.theme.bg_primary)
                .fg(app.theme.fg_primary),
        )
        .title("Load APO EQ File");

    f.render_widget(Clear, dialog_area);
    f.render_widget(block, dialog_area);

    // Inner area for text
    let inner = Rect {
        x: dialog_area.x + 1,
        y: dialog_area.y + 1,
        width: dialog_area.width.saturating_sub(2),
        height: dialog_area.height.saturating_sub(2),
    };

    let lines = vec![
        Line::from("Enter path to APO file:"),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(app.theme.accent_primary)),
            Span::raw(&app.apo_file_input),
            Span::styled("_", Style::default().fg(app.theme.accent_success)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Supported format:",
            Style::default().fg(app.theme.title_color),
        )]),
        Line::from("  Filter 1: ON PK Fc 100 Hz Gain -2.0 dB Q 1.41"),
        Line::from("  Filter 2: ON LSC Fc 105 Hz Gain 4.1 dB Q 0.71"),
        Line::from(""),
        Line::from("Press Enter to load, ESC to cancel"),
    ];

    let paragraph = Paragraph::new(lines)
        .style(Style::default().fg(app.theme.fg_primary))
        .block(Block::default())
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, inner);

    // Autocomplete dropdown below dialog
    if ac_height > 0 {
        let ac_area = Rect {
            x: dialog_area.x,
            y: dialog_area.y + base_height,
            width: dialog_area.width,
            height: ac_height,
        };
        render_autocomplete_dropdown(f, ac_area, app);
    }
}

pub(crate) fn draw_load_sofa_file_dialog(f: &mut Frame, app: &App) {
    let area = f.area();
    let dialog_width = (area.width as f32 * 0.6) as u16;
    let ac_height = autocomplete_dropdown_height(app);
    let base_height: u16 = 8;
    let dialog_height = base_height + ac_height;
    let dialog_x = (area.width - dialog_width) / 2;
    let dialog_y = (area.height.saturating_sub(dialog_height)) / 2;

    let dialog_area = Rect {
        x: dialog_x,
        y: dialog_y,
        width: dialog_width,
        height: dialog_height,
    };

    // Clear background
    let block = Block::default()
        .borders(Borders::ALL)
        .style(
            Style::default()
                .bg(app.theme.bg_primary)
                .fg(app.theme.fg_primary),
        )
        .title("Load SOFA HRTF File");

    f.render_widget(Clear, dialog_area);
    f.render_widget(block, dialog_area);

    // Inner area for text
    let inner = Rect {
        x: dialog_area.x + 1,
        y: dialog_area.y + 1,
        width: dialog_area.width.saturating_sub(2),
        height: dialog_area.height.saturating_sub(2),
    };

    let lines = vec![
        Line::from("Enter path to SOFA file containing HRTFs:"),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(app.theme.accent_primary)),
            Span::raw(&app.sofa_file_input),
            Span::styled("_", Style::default().fg(app.theme.accent_success)),
        ]),
        Line::from(""),
        Line::from("SOFA format contains Head-Related Transfer Functions"),
        Line::from("Press Enter to set path, ESC to cancel"),
    ];

    let paragraph = Paragraph::new(lines)
        .style(Style::default().fg(app.theme.fg_primary))
        .block(Block::default())
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, inner);

    // Autocomplete dropdown below dialog
    if ac_height > 0 {
        let ac_area = Rect {
            x: dialog_area.x,
            y: dialog_area.y + base_height,
            width: dialog_area.width,
            height: ac_height,
        };
        render_autocomplete_dropdown(f, ac_area, app);
    }
}
