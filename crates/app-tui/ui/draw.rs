use super::*;

pub fn draw(f: &mut Frame, app: &mut App) {
    // Paint the entire frame with theme colors so all widgets inherit them
    let bg_block = Block::default().style(
        Style::default()
            .bg(app.theme.bg_primary)
            .fg(app.theme.fg_primary),
    );
    f.render_widget(bg_block, f.area());

    // Loading screen: full-screen centered animation, bypass normal layout
    if app.current_screen == Screen::Loading {
        draw_loading_screen(f, app);
        return;
    }

    // Ensure filtered albums cache is updated
    app.filtered_albums();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title bar
            Constraint::Min(0),    // Main content
            Constraint::Length(1), // Status bar
        ])
        .split(f.area());

    // Title bar
    draw_title(f, chunks[0], app);

    // Calculate exact width needed for meters:
    // 2 (borders) + 1 (padding) + groups + 1 (padding)
    let mut meters_width = 2 + 1; // borders + left padding
    if !app.level_meter_groups.is_empty() {
        for group in &app.level_meter_groups {
            let num_channels_in_group = group.channels.len();
            // For stereo (2 channels), use 3 chars per meter + 2 chars spacing = 8 total
            // For other configs, use 1 char per channel with max(3) for controls
            let group_width = if num_channels_in_group == 2 {
                8 // 3 + 2 + 3 for stereo L-R layout
            } else {
                num_channels_in_group.max(3)
            };
            meters_width += group_width + 1; // group width + spacing
        }
    } else {
        meters_width += 1; // right padding even if no groups
    }

    // Use fixed width for right column to avoid extra space
    let right_col_width = meters_width.max(20) as u16; // Minimum 26 for LUFS/Volume boxes

    // Check window height for responsive layout
    let window_width = f.area().width;
    let window_height = f.area().height;
    let use_three_columns = window_height < 40;

    let main_chunks = if use_three_columns {
        // When height < 40, use 3 columns: main, loudness+volume, level meters
        if window_width > 100 {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Min(0),                  // Main content (takes remaining space)
                    Constraint::Length(26),              // Loudness + Volume column (fixed width)
                    Constraint::Length(right_col_width), // Level meters column (exact width)
                ])
                .split(chunks[1])
        } else {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Min(0),     // Main content (takes remaining space)
                    Constraint::Length(26), // Loudness + Volume column (fixed width)
                ])
                .split(chunks[1])
        }
    } else {
        // When height >= 40, use 2 columns: main, meters (with all components stacked)
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(0),                  // Main content (takes remaining space)
                Constraint::Length(right_col_width), // Right column (exact width)
            ])
            .split(chunks[1])
    };

    // Check if window is tall enough for dual view (library + queue)
    let show_dual_view = window_height >= DUAL_VIEW_HEIGHT_THRESHOLD
        && (app.current_screen == Screen::Library || app.current_screen == Screen::Queue);

    if show_dual_view {
        // Split main area vertically to show both library and queue
        let dual_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50), // Library
                Constraint::Percentage(50), // Queue
            ])
            .split(main_chunks[0]);

        // Draw both views with focus indication
        draw_library_screen(f, dual_chunks[0], app);
        draw_queue_screen(f, dual_chunks[1], app);
    } else {
        // Standard single-view mode
        match app.current_screen {
            Screen::Loading => unreachable!(), // Handled by early return above
            Screen::Library => draw_library_screen(f, main_chunks[0], app),
            Screen::Queue => draw_queue_screen(f, main_chunks[0], app),
            Screen::Playlists => draw_playlists_screen(f, main_chunks[0], app),
            Screen::Plugins => draw_plugins_screen(f, main_chunks[0], app),
            Screen::Devices => draw_devices_screen(f, main_chunks[0], app),
            Screen::Configure => draw_configure_screen(f, main_chunks[0], app),
        }
    }

    // Right column(s) with meters - layout depends on height
    if use_three_columns {
        // 3-column layout: loudness+volume in middle, level meters in right
        draw_loudness_and_volume_column(f, main_chunks[1], app);
        if window_width > 100 {
            draw_level_meter_box(f, main_chunks[2], app);
        }
    } else {
        // 2-column layout: all meters stacked vertically
        draw_meters_column(f, main_chunks[1], app);
    }

    // Status bar
    draw_status_bar(f, chunks[2], app);

    // Plugin parameter editor modal (if in edit mode)
    if app.input_mode == InputMode::EditPlugin {
        draw_plugin_editor_modal(f, app);
    }

    // Save/Load plugin input dialog
    if app.input_mode == InputMode::SavePlugins {
        draw_save_plugins_dialog(f, app);
    } else if app.input_mode == InputMode::LoadPlugins {
        draw_load_plugins_dialog(f, app);
    } else if app.input_mode == InputMode::LoadApoFile {
        draw_load_apo_file_dialog(f, app);
    } else if app.input_mode == InputMode::LoadSofaFile {
        draw_load_sofa_file_dialog(f, app);
    } else if app.input_mode == InputMode::FileExplorer {
        draw_file_explorer_modal(f, app);
    }

    // Scan progress popup
    if app.scan_in_progress {
        draw_scan_progress_dialog(f, app);
    }

    // Maintenance progress dialog
    if app.maintenance_in_progress {
        draw_maintenance_progress_dialog(f, app);
    }

    // ReplayGain progress dialog
    if app.replay_gain_manager.in_progress {
        draw_replay_gain_progress_dialog(f, app);
    }

    // Help modal
    if app.input_mode == InputMode::ShowHelp {
        draw_help_modal(f, app);
    }

    // Error modal
    if app.input_mode == InputMode::ShowError {
        draw_error_modal(f, app);
    }

    // Channel conflict modal
    if app.input_mode == InputMode::ChannelConflict {
        draw_channel_conflict_modal(f, app);
    }

    // Configure sub-screen modal (drawn when inside a sub-screen, not on the tab bar).
    // Skip when an overlay modal (file explorer, help, error, etc.) is active so the
    // configure wizard content doesn't paint over it.
    if app.current_screen == Screen::Configure && app.input_mode.is_configure_sub_screen() {
        draw_configure_modal(f, app);
    }
}
