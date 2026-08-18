use super::super::*;

pub(crate) fn draw_devices_screen(f: &mut Frame, area: Rect, app: &App) {
    let i18n = crate::i18n::TuiTranslations::for_language(app.ui.language);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),      // Help box
            Constraint::Percentage(60), // Output devices
            Constraint::Min(5),         // Cast devices
        ])
        .split(area);

    let help = i18n.dynamic("↑↓=Navigate  Enter=Select  R=Reload  Esc=Back".to_string());
    draw_help_box_with_text(f, chunks[0], app, &help);

    // --- Output devices block ----------------------------------------------
    let items: Vec<ListItem> = app
        .audio_devices
        .outputs
        .iter()
        .enumerate()
        .map(|(i, device)| {
            let is_selected = i == app.audio_devices.selected_output_index;

            let default_tag = if device.is_default {
                i18n.dynamic(" [DEFAULT]".to_string())
            } else {
                String::new()
            };
            let config_info = if let Some(ref config) = device.default_config {
                format!(" ({}ch, {}Hz)", config.channels, config.sample_rate)
            } else {
                String::new()
            };

            let selected_style = Style::default()
                .fg(app.theme.bg_primary)
                .bg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD);
            let normal_style = Style::default().fg(app.theme.fg_primary);

            let line = Line::from(vec![
                Span::styled(
                    format!(" {} ", if is_selected { "►" } else { " " }),
                    if is_selected {
                        selected_style
                    } else {
                        Style::default().fg(app.theme.accent_primary)
                    },
                ),
                Span::styled(
                    device.name.clone(),
                    if is_selected {
                        selected_style
                    } else {
                        normal_style
                    },
                ),
                Span::styled(
                    default_tag,
                    if is_selected {
                        selected_style
                    } else {
                        Style::default().fg(app.theme.accent_success)
                    },
                ),
                Span::styled(
                    config_info,
                    if is_selected {
                        selected_style
                    } else {
                        Style::default().fg(app.theme.fg_secondary)
                    },
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let output_title = i18n.dynamic(if app.audio_devices.outputs.is_empty() {
        " Output Devices (none found) ".to_string()
    } else {
        format!(" Output Devices ({}) ", app.audio_devices.outputs.len())
    });

    let mut list_state = ListState::default();
    list_state.select(Some(app.audio_devices.selected_output_index));

    let output_list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(app.theme.border_color))
                .title(output_title),
        )
        .highlight_style(
            Style::default()
                .fg(app.theme.bg_primary)
                .bg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(output_list, chunks[1], &mut list_state);

    // --- Cast devices block ------------------------------------------------
    let cast_items: Vec<ListItem> = app
        .audio_devices
        .cast
        .iter()
        .map(|device| {
            let line = Line::from(vec![
                Span::styled("   ", Style::default().fg(app.theme.accent_primary)),
                Span::styled(
                    device.name.clone(),
                    Style::default().fg(app.theme.fg_primary),
                ),
                Span::styled(
                    format!(" [{}]", device.device_type),
                    Style::default().fg(app.theme.accent_primary),
                ),
                Span::styled(
                    format!(" {}:{}", device.address, device.port),
                    Style::default().fg(app.theme.fg_secondary),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let cast_title = i18n.dynamic(if app.audio_devices.cast_discovery_running {
        " Cast Devices (scanning…) ".to_string()
    } else if app.audio_devices.cast.is_empty() {
        " Cast Devices (none found — press R to scan) ".to_string()
    } else {
        format!(" Cast Devices ({}) ", app.audio_devices.cast.len())
    });

    let cast_list = List::new(cast_items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(app.theme.border_color))
            .title(cast_title),
    );

    f.render_widget(cast_list, chunks[2]);
}

pub(crate) fn draw_configure_screen(f: &mut Frame, area: Rect, app: &App) {
    let i18n = crate::i18n::TuiTranslations::for_language(app.ui.language);
    use crate::app::ConfigureSubScreen;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Help box (with border)
            Constraint::Min(0),    // Select menu
        ])
        .split(area);

    let help = i18n.dynamic("↑↓=Navigate  Enter=Open  Esc=Back".to_string());
    draw_help_box_with_text(f, chunks[0], app, &help);

    let options: &[(ConfigureSubScreen, &str, &str)] = &[
        (
            ConfigureSubScreen::Directories,
            "1",
            "Directories        – Music library folders",
        ),
        (
            ConfigureSubScreen::Recording,
            "2",
            "Recording          – Measure impulse responses",
        ),
        (
            ConfigureSubScreen::RoomEq,
            "3",
            "Room EQ            – Optimize room correction filters",
        ),
        (
            ConfigureSubScreen::HeadphoneEq,
            "4",
            "Headphone EQ       – Target-curve EQ for headphones",
        ),
        (
            ConfigureSubScreen::SpinoramaEq,
            "5",
            "Spinorama EQ       – Speaker EQ from spinorama data",
        ),
        (
            ConfigureSubScreen::FederationSources,
            "6",
            "Library Sources    – Remote libraries (Subsonic, MPD, DLNA, Peer)",
        ),
        (
            ConfigureSubScreen::Servers,
            "7",
            "Servers            – SOTF API, MPD and DLNA settings",
        ),
        (
            ConfigureSubScreen::MetadataServices,
            "8",
            "Metadata Services  – MusicBrainz and tag provider settings",
        ),
    ];

    let items: Vec<ListItem> = options
        .iter()
        .map(|(sub, key, label)| {
            let is_selected = *sub == app.configure_sub_screen;
            let style = if is_selected {
                Style::default()
                    .fg(app.theme.bg_primary)
                    .bg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg_primary)
            };
            let line = Line::from(vec![
                Span::styled(
                    format!(" [{}] ", key),
                    if is_selected {
                        style.add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(app.theme.accent_primary)
                    },
                ),
                Span::styled(i18n.dynamic((*label).to_string()), style),
            ]);
            ListItem::new(line)
        })
        .collect();

    let selected_idx = options
        .iter()
        .position(|(sub, _, _)| *sub == app.configure_sub_screen)
        .unwrap_or(0);

    let mut list_state = ListState::default();
    list_state.select(Some(selected_idx));

    // Use Double border when the configure menu is focused
    let menu_border = if app.input_mode == InputMode::Configure {
        BorderType::Double
    } else {
        BorderType::Rounded
    };
    let menu_border_color = if app.input_mode == InputMode::Configure {
        app.theme.accent_primary
    } else {
        app.theme.border_color
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(menu_border)
                .border_style(Style::default().fg(menu_border_color))
                .title(i18n.ui(" Configure – select a workflow ")),
        )
        .highlight_style(
            Style::default()
                .fg(app.theme.bg_primary)
                .bg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(list, chunks[1], &mut list_state);
}

/// Draw the full-screen configure modal (covers everything below the 3-row title bar).
pub(crate) fn draw_configure_modal(f: &mut Frame, app: &App) {
    let i18n = crate::i18n::TuiTranslations::for_language(app.ui.language);
    use crate::app::ConfigureSubScreen;

    let area = below_title_bar(f);

    // Clear the area so the modal paints over everything underneath
    f.render_widget(Clear, area);

    let title = match app.configure_sub_screen {
        ConfigureSubScreen::Directories => " Directories ",
        ConfigureSubScreen::Recording => " Recording ",
        ConfigureSubScreen::RoomEq => " Room EQ ",
        ConfigureSubScreen::HeadphoneEq => " Headphone EQ ",
        ConfigureSubScreen::SpinoramaEq => " Spinorama EQ ",
        ConfigureSubScreen::FederationSources => " Library Sources ",
        ConfigureSubScreen::Servers => " Servers ",
        ConfigureSubScreen::MetadataServices => " Metadata Services ",
    };

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(
            Style::default()
                .fg(app.theme.accent_primary)
                .bg(app.theme.bg_primary),
        )
        .style(
            Style::default()
                .bg(app.theme.bg_primary)
                .fg(app.theme.fg_primary),
        )
        .title(i18n.dynamic(format!(
            "{} (Esc to close)",
            i18n.dynamic(title.to_string())
        )));

    f.render_widget(outer, area);

    // Inner content area (inside border)
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    match app.configure_sub_screen {
        ConfigureSubScreen::Directories => draw_directory_manager(f, inner, app),
        ConfigureSubScreen::Recording => draw_recording_screen(f, inner, app),
        ConfigureSubScreen::RoomEq => draw_room_eq_screen(f, inner, app),
        ConfigureSubScreen::HeadphoneEq => draw_headphone_eq_screen(f, inner, app),
        ConfigureSubScreen::SpinoramaEq => draw_spinorama_eq_screen(f, inner, app),
        ConfigureSubScreen::FederationSources => draw_federation_screen(f, inner, app),
        ConfigureSubScreen::Servers => draw_servers_screen(f, inner, app),
        ConfigureSubScreen::MetadataServices => draw_metadata_services_screen(f, inner, app),
    }
}

fn draw_metadata_services_screen(f: &mut Frame, area: Rect, app: &App) {
    let i18n = crate::i18n::TuiTranslations::for_language(app.ui.language);
    let config = sotf_audio_player::config::load_metadata_services_config()
        .unwrap_or_else(|_| sotf_audio_player::MetadataServicesConfig::default());
    let provider = config.providers.first().cloned().unwrap_or_default();
    let account = provider
        .username
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| i18n.dynamic("Anonymous".to_string()));
    let auth_status = if provider.has_stored_credentials {
        i18n.dynamic("Credentials saved".to_string())
    } else {
        i18n.dynamic("Anonymous search enabled".to_string())
    };
    let lines = vec![
        Line::from(vec![Span::styled(
            "MusicBrainz",
            Style::default()
                .fg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(i18n.dynamic(format!("Endpoint: {}", provider.endpoint))),
        Line::from(i18n.dynamic(format!("Account: {account}"))),
        Line::from(i18n.dynamic(format!("Status: {auth_status}"))),
        Line::from(i18n.dynamic(format!("User-Agent: {}", config.user_agent))),
        Line::from(""),
        Line::from(
            i18n.ui("Manual album/track metadata edits use the shared metadata controller."),
        ),
        Line::from(
            i18n.ui("MusicBrainz search/import is anonymous by default; login is optional."),
        ),
    ];
    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(app.theme.border_color))
                .title(i18n.ui(" Metadata Services ")),
        )
        .style(
            Style::default()
                .fg(app.theme.fg_primary)
                .bg(app.theme.bg_primary),
        );
    f.render_widget(paragraph, area);
}

pub(crate) fn draw_recording_screen(f: &mut Frame, area: Rect, app: &App) {
    let i18n = crate::i18n::TuiTranslations::for_language(app.ui.language);
    use ratatui::widgets::Tabs;
    use sotf_audio_player::recording_helpers::{
        position_guidance, take_quality_cell, take_verdict_text,
    };
    use sotf_audio_player::recording_types::{
        ChannelRecording, ChannelRecordingState, RecordingStep,
    };

    let s = &app.recording;

    // Layout: step tabs on top, content below
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(area);

    // Border types: Double for focused area, Plain for unfocused
    let step_tab_border = if s.step_tab_focused {
        BorderType::Double
    } else {
        BorderType::Plain
    };
    let content_border = if s.step_tab_focused {
        BorderType::Plain
    } else {
        BorderType::Double
    };
    let step_tab_border_color = if s.step_tab_focused {
        app.theme.accent_primary
    } else {
        app.theme.border_color
    };
    let content_border_color = if s.step_tab_focused {
        app.theme.border_color
    } else {
        app.theme.accent_primary
    };

    // Step tabs — built from `RecordingStep::all()` so new variants
    // (e.g. `Probe`) show up automatically. The Room EQ wizard had a
    // bug where a hand-rolled `Vec<WizardStep>` list silently dropped
    // newly-added variants; we don't repeat the pattern here.
    let steps = RecordingStep::all();
    let tab_titles: Vec<Line> = steps
        .iter()
        .map(|st| {
            let style = if *st == s.model.step {
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg_secondary)
            };
            Line::from(Span::styled(i18n.dynamic(st.label().to_string()), style))
        })
        .collect();
    let step_idx = steps.iter().position(|st| *st == s.model.step).unwrap_or(0);
    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(step_tab_border)
                .border_style(Style::default().fg(step_tab_border_color))
                .title(i18n.ui("Recording")),
        )
        .select(step_idx)
        .highlight_style(Style::default().fg(app.theme.accent_primary));
    f.render_widget(tabs, outer[0]);

    // Content wrapper with focus-aware border
    let content_wrapper = Block::default()
        .borders(Borders::ALL)
        .border_type(content_border)
        .border_style(Style::default().fg(content_border_color));
    f.render_widget(content_wrapper, outer[1]);
    let content = Rect {
        x: outer[1].x + 1,
        y: outer[1].y + 1,
        width: outer[1].width.saturating_sub(2),
        height: outer[1].height.saturating_sub(2),
    };

    match s.model.step {
        RecordingStep::Config => {
            // When editing a path field, reserve space for the autocomplete
            // dropdown at the bottom of the content area. Without this carve-
            // out the dropdown was placed *below* the already full-height
            // config block and clipped to zero rows — so suggestions never
            // appeared even though the rest of the autocomplete plumbing
            // worked.
            let editing_path = s.editing_output_dir || s.editing_mic_cal_channel.is_some();
            let ac_h = if editing_path {
                autocomplete_dropdown_height(app)
            } else {
                0
            };
            let (config_area, ac_area_opt) = if ac_h > 0 && content.height > ac_h + 2 {
                let split = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(3), Constraint::Length(ac_h)])
                    .split(content);
                (split[0], Some(split[1]))
            } else {
                (content, None)
            };

            let playback_name = if s.available_playback_devices.is_empty() {
                "(no devices)".to_string()
            } else if let Some(d) = s.available_playback_devices.get(s.selected_playback_idx) {
                d.1.clone()
            } else {
                "(select)".to_string()
            };

            let recording_name = if s.available_recording_devices.is_empty() {
                "(no devices)".to_string()
            } else if let Some(d) = s.available_recording_devices.get(s.selected_recording_idx) {
                d.1.clone()
            } else {
                "(select)".to_string()
            };

            // Build the row list dynamically. Each row is either a
            // non-selectable section header (`idx = None`) or one logical
            // field (`idx = Some(i)` where `i` matches `selected_field`'s
            // value via `recording_field_at`).
            use crate::app::RecordingField;
            let n_channels = s.model.recording_config.num_channels.max(1);
            let mic_cal_label = |ch: usize| {
                if n_channels > 1 {
                    format!("Mic Cal Ch{}", ch + 1)
                } else {
                    "Mic Calibration".to_string()
                }
            };
            let mic_cal_value = |ch: usize| {
                // Model-level vec: the wizard's working copy that the
                // editing accessors, capture and save all consult.
                s.model
                    .mic_calibration_paths
                    .get(ch)
                    .and_then(|o| o.clone())
                    .filter(|p| !p.is_empty())
                    .unwrap_or_else(|| "<none>".to_string())
            };
            let channel_input_label = |ch: usize| format!("Ch{} input", ch + 1);
            let channel_input_value = |ch: usize| {
                s.model
                    .recording_config
                    .channel_mappings
                    .get(ch)
                    .map(|c| (c + 1).to_string())
                    .unwrap_or_else(|| "1".to_string())
            };

            // Use owned strings throughout so the dynamic per-channel rows
            // can be formatted in place.
            let mut rows: Vec<(Option<usize>, String, String)> = Vec::new();
            rows.push((None, "── Devices ──".to_string(), String::new()));
            rows.push((Some(0), "Playback Device".to_string(), playback_name));
            rows.push((Some(1), "Recording Device".to_string(), recording_name));
            rows.push((
                Some(2),
                "Speaker Config".to_string(),
                s.model
                    .playback_config
                    .speaker_configuration
                    .as_str()
                    .to_string(),
            ));
            rows.push((None, "── Signal ──".to_string(), String::new()));
            rows.push((
                Some(3),
                "Signal Type".to_string(),
                s.model.signal_type.as_str().to_string(),
            ));
            rows.push((
                Some(4),
                "Duration (s)".to_string(),
                format!("{:.1}", s.model.signal_duration_secs),
            ));
            rows.push((
                Some(5),
                "Level (dB)".to_string(),
                format!("{:.1}", s.model.signal_level_db),
            ));
            rows.push((
                Some(6),
                "Sweep Start (Hz)".to_string(),
                format!("{:.0}", s.model.sweep_start_freq),
            ));
            rows.push((
                Some(7),
                "Sweep End (Hz)".to_string(),
                format!("{:.0}", s.model.sweep_end_freq),
            ));
            rows.push((None, "── Paths ──".to_string(), String::new()));
            rows.push((
                Some(8),
                "Output Directory".to_string(),
                if s.output_directory.is_empty() {
                    "<not set>".to_string()
                } else {
                    s.output_directory.clone()
                },
            ));
            rows.push((None, "── Recording Channels ──".to_string(), String::new()));
            rows.push((
                Some(9),
                "Num Channels".to_string(),
                s.model.recording_config.num_channels.to_string(),
            ));
            rows.push((
                Some(10),
                "CTC Matrix".to_string(),
                s.model
                    .recording_config
                    .ctc_matrix_strategy
                    .as_str()
                    .to_string(),
            ));
            rows.push((
                Some(11),
                "Loopback Input".to_string(),
                s.model
                    .recording_config
                    .ctc_loopback_input_channel
                    .map(|ch| (ch + 1).to_string())
                    .unwrap_or_else(|| "<none>".to_string()),
            ));
            rows.push((
                Some(12),
                "Measurement positions".to_string(),
                s.model.recording_config.num_positions.to_string(),
            ));
            rows.push((None, "── Measurement Quality ──".to_string(), String::new()));
            rows.push((
                Some(13),
                "Sweeps per channel".to_string(),
                s.model.num_sweeps.to_string(),
            ));
            for ch in 0..n_channels {
                rows.push((Some(14 + ch), mic_cal_label(ch), mic_cal_value(ch)));
            }
            for ch in 0..n_channels {
                rows.push((
                    Some(14 + n_channels + ch),
                    channel_input_label(ch),
                    channel_input_value(ch),
                ));
            }

            let mut lines: Vec<Line> = Vec::new();
            for (idx, label, value) in &rows {
                let is_selected = idx.is_some_and(|i| i == s.selected_field);
                let is_editing_numerical = is_selected && s.editing_value;
                // Resolve the field identity to know whether this row is
                // currently in path-edit mode.
                let field_kind =
                    idx.and_then(|i| crate::app::recording_field_at(&app.recording, i));
                let is_editing_path = is_selected
                    && match field_kind {
                        Some(RecordingField::OutputDir) => s.editing_output_dir,
                        Some(RecordingField::MicCal(ch)) => s.editing_mic_cal_channel == Some(ch),
                        _ => false,
                    };
                let is_editing = is_editing_numerical || is_editing_path;
                let display_value = if is_editing_numerical {
                    format!("{}▏", s.edit_buffer)
                } else if is_editing_path {
                    let path_val: String = match field_kind {
                        Some(RecordingField::OutputDir) => s.output_directory.clone(),
                        Some(RecordingField::MicCal(ch)) => s
                            .model
                            .mic_calibration_paths
                            .get(ch)
                            .and_then(|o| o.clone())
                            .unwrap_or_default(),
                        _ => value.clone(),
                    };
                    format!("{}▏", path_val)
                } else {
                    value.clone()
                };
                let style = if is_editing {
                    Style::default()
                        .fg(app.theme.fg_selected)
                        .bg(app.theme.bg_selected)
                        .add_modifier(Modifier::BOLD)
                } else if is_selected {
                    Style::default()
                        .fg(app.theme.accent_primary)
                        .add_modifier(Modifier::BOLD)
                } else if idx.is_none() {
                    Style::default()
                        .fg(app.theme.fg_secondary)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.theme.fg_primary)
                };
                let arrow = if is_editing {
                    "✎ "
                } else if is_selected {
                    "> "
                } else {
                    "  "
                };
                lines.push(Line::from(Span::styled(
                    format!(
                        "{}{:<22} {}",
                        arrow,
                        i18n.dynamic(label.clone()),
                        display_value
                    ),
                    style,
                )));
            }

            // Channel mapping display
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                i18n.dynamic("  Channels:".to_string()),
                Style::default()
                    .fg(app.theme.fg_secondary)
                    .add_modifier(Modifier::BOLD),
            )));
            for mapping in &s.model.playback_config.channel_mappings {
                lines.push(Line::from(Span::styled(
                    i18n.dynamic(format!(
                        "    {} → ch {}",
                        mapping.group_name,
                        mapping
                            .interface_channels
                            .iter()
                            .map(|c| c.to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    )),
                    Style::default().fg(app.theme.fg_primary),
                )));
            }

            lines.push(Line::from(""));
            let hint = if s.editing_value {
                " Type value, Enter=confirm  Esc=cancel"
            } else if s.editing_output_dir || s.editing_mic_cal_channel.is_some() {
                " Type path, Tab=complete  Enter=confirm  F2=browse  Esc=cancel"
            } else {
                " Up/Down=navigate  Left/Right=adjust  Enter=edit value/path  Tab=next field"
            };
            lines.push(Line::from(Span::styled(
                i18n.dynamic(hint.to_string()),
                Style::default().fg(app.theme.fg_secondary),
            )));

            let para = Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(i18n.ui("Configure")),
                )
                .wrap(Wrap { trim: false });
            f.render_widget(para, config_area);

            if let Some(ac_area) = ac_area_opt {
                f.render_widget(Clear, ac_area);
                render_autocomplete_dropdown(f, ac_area, app);
            }
        }

        RecordingStep::Capture => {
            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // status
                    Constraint::Min(5),    // channel list
                    Constraint::Length(1), // help
                ])
                .split(content);

            // Status
            let num_positions = s.model.recording_config.num_positions.max(1);
            let current_pos = s.model.current_position();
            let status_text = if !s.model.status_message.is_empty() {
                s.model.status_message.clone()
            } else if num_positions > 1 && current_pos < num_positions {
                // Multi-position workflow: tell the user where the mic(s)
                // go next (first position must be the main listening one).
                position_guidance(current_pos, num_positions)
            } else {
                "Ready to record. Select a channel and press Enter.".to_string()
            };
            let status = Paragraph::new(i18n.dynamic(status_text))
                .style(Style::default().fg(app.theme.accent_primary))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(i18n.ui("Status")),
                );
            f.render_widget(status, inner[0]);

            // Channel list
            let header = Row::new(vec![
                Cell::from("#"),
                Cell::from(i18n.ui("Channel")),
                Cell::from(i18n.ui("State")),
                Cell::from(i18n.ui("Quality")),
            ])
            .style(
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            );

            let rows: Vec<Row> = s
                .model
                .channel_recordings
                .iter()
                .enumerate()
                .map(|(i, ch)| {
                    let is_current = s.model.current_recording_channel == Some(i);
                    let state_str = match ch.state {
                        ChannelRecordingState::Empty => "[ ]",
                        ChannelRecordingState::Recording => "[REC]",
                        ChannelRecordingState::Done => "[OK]",
                        ChannelRecordingState::ReviewNeeded => "[!?]",
                        ChannelRecordingState::Error => "[ERR]",
                    };
                    let style = if is_current {
                        Style::default()
                            .fg(app.theme.accent_primary)
                            .add_modifier(Modifier::BOLD)
                    } else if ch.state == ChannelRecordingState::Done {
                        Style::default().fg(app.theme.accent_success)
                    } else if ch.state == ChannelRecordingState::ReviewNeeded {
                        Style::default().fg(app.theme.accent_warning)
                    } else {
                        Style::default().fg(app.theme.fg_primary)
                    };
                    Row::new(vec![
                        Cell::from(format!("{}", i + 1)),
                        Cell::from(ch.channel_name.clone()),
                        Cell::from(state_str),
                        Cell::from(take_quality_cell(ch.state, ch.result.as_ref())),
                    ])
                    .style(style)
                })
                .collect();

            let ch_table = Table::new(
                rows,
                [
                    Constraint::Length(3),
                    Constraint::Length(12),
                    Constraint::Length(8),
                    Constraint::Length(12),
                ],
            )
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(i18n.ui("Channels")),
            );
            f.render_widget(ch_table, inner[1]);

            let help = Paragraph::new(
                i18n.ui(" Up/Down=select  Enter=record  a=accept warned take  Tab=evaluate"),
            )
            .style(Style::default().fg(app.theme.fg_secondary));
            f.render_widget(help, inner[2]);
        }

        RecordingStep::Probe => {
            draw_recording_probe_step(f, content, app);
        }

        RecordingStep::BassAnchor => {
            draw_recording_bass_anchor_step(f, content, app);
        }

        RecordingStep::SplCalibration => {
            draw_recording_spl_calibration_step(f, content, app);
        }

        RecordingStep::Evaluating => {
            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(5), // channel summary
                    Constraint::Min(3),    // selected channel details
                    Constraint::Length(1), // help
                ])
                .split(content);

            // Channel summary
            let completed: Vec<&ChannelRecording> = s
                .model
                .channel_recordings
                .iter()
                .filter(|ch| ch.state == ChannelRecordingState::Done)
                .collect();

            if completed.is_empty() {
                let placeholder =
                    Paragraph::new(i18n.ui("No recordings completed yet. Go to Capture step."))
                        .style(Style::default().fg(app.theme.fg_secondary))
                        .alignment(Alignment::Center)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(i18n.ui("Evaluate")),
                        );
                f.render_widget(placeholder, content);
                return;
            }

            let header = Row::new(vec![
                Cell::from(i18n.ui("Channel")),
                Cell::from(i18n.ui("Points")),
                Cell::from(i18n.ui("Status")),
            ])
            .style(
                Style::default()
                    .fg(app.theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            );

            let rows: Vec<Row> = completed
                .iter()
                .enumerate()
                .map(|(i, ch)| {
                    let pts = ch
                        .result
                        .as_ref()
                        .map(|r| format!("{}", r.frequencies.len()))
                        .unwrap_or_else(|| "-".to_string());
                    let style = if i == s.selected_channel_view {
                        Style::default()
                            .fg(app.theme.accent_primary)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(app.theme.fg_primary)
                    };
                    Row::new(vec![
                        Cell::from(ch.channel_name.clone()),
                        Cell::from(pts),
                        Cell::from(take_quality_cell(ch.state, ch.result.as_ref())),
                    ])
                    .style(style)
                })
                .collect();

            let table = Table::new(
                rows,
                [
                    Constraint::Length(12),
                    Constraint::Length(10),
                    Constraint::Length(14),
                ],
            )
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(i18n.ui("Recorded Channels")),
            );
            f.render_widget(table, inner[0]);

            // Selected channel details
            if let Some(ch) = completed.get(s.selected_channel_view)
                && let Some(ref result) = ch.result
            {
                let mut details = vec![
                    Line::from(Span::styled(
                        i18n.dynamic(format!(" Channel: {}", ch.channel_name)),
                        Style::default().fg(app.theme.accent_primary),
                    )),
                    Line::from(
                        i18n.dynamic(format!(" Frequency points: {}", result.frequencies.len())),
                    ),
                ];
                if let Some(ref thd) = result.thd_percent {
                    let avg_thd = thd.iter().copied().sum::<f32>() / thd.len().max(1) as f32;
                    details.push(Line::from(
                        i18n.dynamic(format!(" Avg THD: {:.2}%", avg_thd)),
                    ));
                }
                if let Some(ref rt60) = result.rt60_ms {
                    let positive: Vec<f32> = rt60.iter().copied().filter(|v| *v > 0.0).collect();
                    if !positive.is_empty() {
                        let avg_rt60 = positive.iter().sum::<f32>() / positive.len() as f32;
                        details.push(Line::from(
                            i18n.dynamic(format!(" Avg RT60: {:.0} ms", avg_rt60)),
                        ));
                    }
                }
                // Per-take quality verdict (Task 9, §4 item 1): score +
                // warnings for the selected channel, so the user can see
                // which positions to re-measure before saving. The verdict
                // embeds engine-worded issue strings, hence the verbatim
                // boundary.
                if let Some(ref q) = result.quality {
                    let color = if q.trustworthy {
                        app.theme.accent_success
                    } else {
                        app.theme.accent_warning
                    };
                    details.push(Line::from(Span::styled(
                        format!(
                            " {}: {}",
                            i18n.ui("Quality"),
                            i18n.dynamic_or_verbatim(&take_verdict_text(q))
                        ),
                        Style::default().fg(color),
                    )));
                }
                let detail_para = Paragraph::new(details).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(i18n.ui("Details")),
                );
                f.render_widget(detail_para, inner[1]);
            }

            let help =
                Paragraph::new(i18n.ui(" Up/Down=select channel  Tab=save  BackTab=capture"))
                    .style(Style::default().fg(app.theme.fg_secondary));
            f.render_widget(help, inner[2]);
        }

        RecordingStep::Saving => {
            draw_recording_saving_step(f, content, app);
        }
    }
}

/// Save-step renderer for the Recording wizard.
///
/// Lays out five boxes vertically: session name, room dimensions,
/// setup description, per-channel speakers, and a status/help strip.
/// The currently-focused field (`selected_save_field`) is highlighted
/// with the accent color; when `editing_save_value` is set, the field
/// shows a `>` marker and echoes `edit_buffer`.
///
/// Selected-field layout:
///   0      Session name
///   1..=3  Room width / depth / height
///   4      Unit toggle (Metric / Imperial)
///   5      Setup description
///   6..    Per-channel speaker entries (one index per playback mapping)
fn draw_recording_saving_step(f: &mut Frame, content: Rect, app: &App) {
    let i18n = crate::i18n::TuiTranslations::for_language(app.ui.language);
    use sotf_audio_player::recording_types::{ChannelRecordingState, RoomDimensionUnit};

    let s = &app.recording;
    let speaker_channel_count = s.model.playback_config.channel_mappings.len();
    // Results table (speakers per channel) grows with the channel
    // count; 1 row per channel + 2 lines borders + 1 line dropdown
    // overlay when editing.
    let speakers_rows = speaker_channel_count.max(1) as u16 + 2;
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),             // session name
            Constraint::Length(5),             // room dimensions
            Constraint::Length(3),             // setup description
            Constraint::Length(speakers_rows), // speakers per channel
            Constraint::Length(3),             // status
            Constraint::Length(1),             // help
        ])
        .split(content);

    let focused = |idx: usize| -> Style {
        if s.selected_save_field == idx {
            Style::default()
                .fg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.fg_primary)
        }
    };
    let is_editing_field = |idx: usize| s.editing_save_value && s.selected_save_field == idx;
    let field_text = |idx: usize, value: String, placeholder: &str| -> String {
        if is_editing_field(idx) {
            format!("> {}_", s.edit_buffer)
        } else if value.is_empty() {
            format!("<{}>", placeholder)
        } else {
            value
        }
    };

    // --- Session Name ------------------------------------------------
    let name_title = i18n.dynamic(
        if is_editing_field(0) {
            "Session Name (editing)"
        } else {
            "Session Name"
        }
        .to_string(),
    );
    let name_para = Paragraph::new(field_text(
        0,
        s.model.save_name.clone(),
        &i18n.dynamic("type session name".to_string()),
    ))
    .style(focused(0))
    .block(Block::default().borders(Borders::ALL).title(name_title));
    f.render_widget(name_para, inner[0]);

    // --- Room Dimensions ---------------------------------------------
    // Single line with four "cells" separated by spaces. Each cell is
    // rendered via a sub-paragraph so the accent-bold style only lands
    // on the focused one.
    let room_block = Block::default()
        .borders(Borders::ALL)
        .title(i18n.dynamic(format!(
            "Room Dimensions ({})",
            s.model.room_dimension_unit.label()
        )));
    let room_inner = room_block.inner(inner[1]);
    f.render_widget(room_block, inner[1]);

    let cells = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(room_inner);
    let dim_str = |idx: usize, v: f64| {
        field_text(
            idx,
            if v > 0.0 {
                format!("{:.2}", v)
            } else {
                String::new()
            },
            "0.00",
        )
    };
    f.render_widget(
        Paragraph::new(i18n.dynamic(format!(" W: {}", dim_str(1, s.model.room_width_input))))
            .style(focused(1)),
        cells[0],
    );
    f.render_widget(
        Paragraph::new(i18n.dynamic(format!(" D: {}", dim_str(2, s.model.room_depth_input))))
            .style(focused(2)),
        cells[1],
    );
    f.render_widget(
        Paragraph::new(i18n.dynamic(format!(" H: {}", dim_str(3, s.model.room_height_input))))
            .style(focused(3)),
        cells[2],
    );
    let unit_marker = i18n.dynamic(
        match s.model.room_dimension_unit {
            RoomDimensionUnit::Metric => "[Metric]",
            RoomDimensionUnit::Imperial => "[Imperial]",
        }
        .to_string(),
    );
    f.render_widget(
        Paragraph::new(format!(" {}", unit_marker)).style(focused(4)),
        cells[3],
    );

    // --- Setup Description -------------------------------------------
    let desc_title = i18n.dynamic(
        if is_editing_field(5) {
            "Setup Description (editing)"
        } else {
            "Setup Description"
        }
        .to_string(),
    );
    let desc_para = Paragraph::new(field_text(
        5,
        s.model.setup_description.clone(),
        &i18n.dynamic("describe treatment, seating, equipment".to_string()),
    ))
    .style(focused(5))
    .block(Block::default().borders(Borders::ALL).title(desc_title));
    f.render_widget(desc_para, inner[2]);

    // --- Speakers per Channel ----------------------------------------
    let catalog = &app.spinorama_eq.model.available_speakers;
    let spk_title = i18n.dynamic(
        if catalog.is_empty() {
            "Speakers per Channel  (catalog loading…)"
        } else {
            "Speakers per Channel"
        }
        .to_string(),
    );
    let spk_block = Block::default().borders(Borders::ALL).title(spk_title);
    let spk_inner = spk_block.inner(inner[3]);
    f.render_widget(spk_block, inner[3]);
    if speaker_channel_count == 0 {
        f.render_widget(
            Paragraph::new(i18n.ui(" No channels yet — record some first."))
                .style(Style::default().fg(app.theme.fg_secondary)),
            spk_inner,
        );
    } else {
        let rows: Vec<Row> = s
            .model
            .playback_config
            .channel_mappings
            .iter()
            .enumerate()
            .map(|(i, mapping)| {
                let field_idx = 6 + i;
                let current = s.model.channel_speakers.get(i).cloned().unwrap_or_default();
                let cell_value = if is_editing_field(field_idx) {
                    format!("> {}_", s.edit_buffer)
                } else if current.is_empty() {
                    i18n.dynamic("<empty>".to_string())
                } else {
                    current
                };
                let row_style = if s.selected_save_field == field_idx {
                    Style::default()
                        .fg(app.theme.accent_primary)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                Row::new(vec![
                    Cell::from(format!(" {}", mapping.group_name.clone())),
                    Cell::from(cell_value),
                ])
                .style(row_style)
            })
            .collect();
        let table = Table::new(rows, [Constraint::Length(8), Constraint::Percentage(90)]);
        f.render_widget(table, spk_inner);
    }

    // --- Status / suggestions ----------------------------------------
    // When editing a channel-speaker field, repurpose the status box
    // to show pipe-separated autocomplete matches from the spinorama
    // catalog. The user types freely in the input; this line is a
    // visual hint — they still commit with Enter.
    let editing_speaker = s.editing_save_value
        && s.selected_save_field >= 6
        && s.selected_save_field < 6 + speaker_channel_count;
    if editing_speaker {
        let q = s.edit_buffer.to_lowercase();
        let matches: Vec<String> = catalog
            .iter()
            .filter(|name| !q.is_empty() && name.to_lowercase().contains(&q))
            .take(5)
            .cloned()
            .collect();
        let hint = i18n.dynamic(if matches.is_empty() && catalog.is_empty() {
            " Loading catalog…".to_string()
        } else if matches.is_empty() {
            " No matches — free-form text is saved as-is".to_string()
        } else {
            format!(" ▸ {}", matches.join(" | "))
        });
        let suggestions = Paragraph::new(hint)
            .style(Style::default().fg(app.theme.accent_primary))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(i18n.ui("Suggestions (spinorama.org)")),
            );
        f.render_widget(suggestions, inner[4]);
    } else if let Some(ref err) = s.save.error {
        let err_para = Paragraph::new(i18n.dynamic_or_verbatim(err))
            .style(Style::default().fg(app.theme.accent_error))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(i18n.ui("Error")),
            );
        f.render_widget(err_para, inner[4]);
    } else if s.save.success {
        let ok = Paragraph::new(i18n.ui(" Recordings saved successfully!"))
            .style(Style::default().fg(app.theme.accent_success))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(i18n.ui("Status")),
            );
        f.render_widget(ok, inner[4]);
    } else {
        let completed = s
            .model
            .channel_recordings
            .iter()
            .filter(|ch| ch.state == ChannelRecordingState::Done)
            .count();
        let status = Paragraph::new(i18n.dynamic(format!(
            " {} channels ready. Output: {}",
            completed,
            if s.output_directory.is_empty() {
                "<default>"
            } else {
                &s.output_directory
            }
        )))
        .style(Style::default().fg(app.theme.fg_secondary))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(i18n.ui("Status")),
        );
        f.render_widget(status, inner[4]);
    }

    // --- Help --------------------------------------------------------
    let help_text = i18n.dynamic(
        if s.editing_save_value {
            " Type value | Enter=confirm | Esc=cancel"
        } else {
            " Tab=next field  ↑↓=nav  Enter=edit  u=unit  Ctrl+S=save"
        }
        .to_string(),
    );
    f.render_widget(
        Paragraph::new(help_text).style(Style::default().fg(app.theme.fg_secondary)),
        inner[5],
    );
}

/// Probe-step renderer for the Recording wizard.
///
/// Three panes: probe/silence/mic form, status/progress banner, and a
/// per-channel results table populated from
/// `ProbeCaptureState.results` after a successful capture. Mirrors the
/// Room EQ Delay Detection step layout so the two feel consistent —
/// differences are:
///   - channel list is seeded from `channel_recordings` (Capture step
///     already ran) rather than loaded measurements.
///   - on success, also shows the persisted WAV path under "Results".
fn draw_recording_probe_step(f: &mut Frame, content: Rect, app: &App) {
    let i18n = crate::i18n::TuiTranslations::for_language(app.ui.language);
    use sotf_audio_player::recording_types::ProbeCaptureStatus;
    use sotf_audio_player::room_eq_types::estimate_probe_sequence_ms;

    let s = &app.recording;
    let pc = &s.model.probe_capture;
    let channel_count = s.model.channel_recordings.len();

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // form
            Constraint::Length(3), // status
            Constraint::Min(5),    // results
            Constraint::Length(1), // help
        ])
        .split(content);

    let focused = |idx: usize| -> Style {
        if s.probe_selected_field == idx {
            Style::default()
                .fg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.fg_primary)
        }
    };
    let is_editing = |idx: usize| s.probe_editing_value && s.probe_selected_field == idx;
    let field_text = |idx: usize, val: String, placeholder: &str| -> String {
        if is_editing(idx) {
            format!("> {}_", s.edit_buffer)
        } else if val.is_empty() {
            format!("<{}>", placeholder)
        } else {
            val
        }
    };

    // --- Form ----------------------------------------------------------
    let form_rows = vec![
        Row::new(vec![
            Cell::from(i18n.ui("Probe duration (ms)")).style(focused(0)),
            Cell::from(field_text(
                0,
                format!("{:.0}", pc.probe_duration_ms),
                "1000",
            ))
            .style(focused(0)),
        ]),
        Row::new(vec![
            Cell::from(i18n.ui("Silence gap (ms)")).style(focused(1)),
            Cell::from(field_text(
                1,
                format!("{:.0}", pc.silence_duration_ms),
                "500",
            ))
            .style(focused(1)),
        ]),
        Row::new(vec![
            Cell::from(i18n.ui("Mic input channel")).style(focused(2)),
            Cell::from(field_text(2, format!("{}", pc.input_channel), "0")).style(focused(2)),
        ]),
        Row::new(vec![
            Cell::from(i18n.ui("[ Run Probe ]")).style(focused(3)),
            Cell::from(
                i18n.dynamic(
                    match pc.status {
                        ProbeCaptureStatus::Running { .. } => "running...",
                        ProbeCaptureStatus::Complete => "done",
                        ProbeCaptureStatus::Failed(_) => "failed",
                        ProbeCaptureStatus::Idle => "press r or Enter",
                    }
                    .to_string(),
                ),
            )
            .style(focused(3)),
        ]),
    ];
    let form = Table::new(
        form_rows,
        [Constraint::Length(24), Constraint::Percentage(60)],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(i18n.ui("Delay Probe Capture")),
    );
    f.render_widget(form, inner[0]);

    // --- Status banner -------------------------------------------------
    let estimated_total =
        estimate_probe_sequence_ms(channel_count, pc.probe_duration_ms, pc.silence_duration_ms);
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let (status_text, status_color) = match &pc.status {
        ProbeCaptureStatus::Idle => (
            "Idle — press `r` to capture tone-burst delays".to_string(),
            app.theme.fg_secondary,
        ),
        ProbeCaptureStatus::Running { .. } => {
            let pct = pc
                .status
                .progress(estimated_total, now_ms)
                .map(|p| format!("{:.0}%", p * 100.0))
                .unwrap_or_else(|| "…".to_string());
            (format!("Running... {}", pct), app.theme.accent_primary)
        }
        ProbeCaptureStatus::Complete => {
            let n = pc.results.as_ref().map(|r| r.channels.len()).unwrap_or(0);
            (
                format!("Complete — detected {} channel(s)", n),
                app.theme.accent_success,
            )
        }
        ProbeCaptureStatus::Failed(msg) => (format!("Failed: {}", msg), app.theme.accent_error),
    };
    let status = Paragraph::new(i18n.dynamic(status_text))
        .style(Style::default().fg(status_color))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(i18n.ui("Status")),
        );
    f.render_widget(status, inner[1]);

    // --- Results table -------------------------------------------------
    if let Some(results) = pc.results.as_ref() {
        let rows: Vec<Row> = results
            .channels
            .iter()
            .enumerate()
            .map(|(i, ch)| {
                let snr_color = if ch.snr_db > 10.0 {
                    app.theme.accent_success
                } else if ch.snr_db > 0.0 {
                    app.theme.accent_primary
                } else {
                    app.theme.accent_error
                };
                let align = results.alignment_delays_ms.get(i).copied().unwrap_or(0.0);
                Row::new(vec![
                    Cell::from(ch.channel_name.clone()),
                    Cell::from(format!("{:.2}", ch.arrival_ms)),
                    Cell::from(format!("{:+.1}", ch.gain_db)),
                    Cell::from(format!("{:+.1}", ch.snr_db)).style(Style::default().fg(snr_color)),
                    Cell::from(format!("{:.2}", align)),
                ])
            })
            .collect();
        let header = Row::new(vec![
            Cell::from(i18n.ui("Channel")),
            Cell::from(i18n.ui("Arrival ms")),
            Cell::from(i18n.ui("Gain dB")),
            Cell::from(i18n.ui("SNR dB")),
            Cell::from(i18n.ui("Align ms")),
        ])
        .style(
            Style::default()
                .fg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        );
        let table = Table::new(
            rows,
            [
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(12),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(i18n.ui("Results")),
        );
        f.render_widget(table, inner[2]);
    } else {
        let empty = Paragraph::new(i18n.ui(" No probe captured yet — press `r` to run"))
            .style(Style::default().fg(app.theme.fg_secondary))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(i18n.ui("Results")),
            );
        f.render_widget(empty, inner[2]);
    }

    // --- Help ----------------------------------------------------------
    let help = i18n.dynamic(
        if s.probe_editing_value {
            " Type value | Enter=confirm | Esc=cancel"
        } else {
            " Tab=next field  ←→=adjust  r=run  Tab=evaluate"
        }
        .to_string(),
    );
    f.render_widget(
        Paragraph::new(help).style(Style::default().fg(app.theme.fg_secondary)),
        inner[3],
    );
}

/// Bass-anchor step renderer (GD-Opt v2 Phase GD-1e).
///
/// Display-only step that mirrors the GPUI wizard surface — config
/// summary, status banner, per-channel results table when present.
/// Optional in the wizard flow: skip with Tab if the system can't
/// reproduce sub-bass.
fn draw_recording_bass_anchor_step(f: &mut Frame, content: Rect, app: &App) {
    let i18n = crate::i18n::TuiTranslations::for_language(app.ui.language);
    use sotf_audio_player::recording_types::BassAnchorCaptureStatus;

    let s = &app.recording;
    let bac = &s.model.bass_anchor_capture;

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // explainer + config summary
            Constraint::Length(3), // status banner
            Constraint::Min(5),    // results
            Constraint::Length(1), // help
        ])
        .split(content);

    // --- Explainer + config summary ------------------------------------
    let tone_ms = 1000.0 * bac.bass_duration_s;
    let loopback_hint = match app
        .recording
        .model
        .recording_config
        .ctc_loopback_input_channel
    {
        Some(ch) => i18n.dynamic(format!(" • loopback ref ch {}", ch)),
        None => String::new(),
    };
    let explainer = vec![
        Line::from(Span::styled(
            i18n.dynamic(
                "Plays a steady-state bass tone per channel so GD-Opt v2 can lock-in the"
                    .to_string(),
            ),
            Style::default().fg(app.theme.fg_secondary),
        )),
        Line::from(Span::styled(
            i18n.dynamic(
                "first bass bin of the sweep-derived phase. Optional — skip with Tab.".to_string(),
            ),
            Style::default().fg(app.theme.fg_secondary),
        )),
        Line::from(Span::styled(
            i18n.dynamic(format!(
                " Tone: {:.1} Hz × {:.1} s ({} sub-windows) • silence {:.0} ms • mic ch {}{}",
                bac.bass_freq_hz,
                bac.bass_duration_s,
                bac.num_windows,
                bac.silence_duration_ms,
                bac.input_channel,
                loopback_hint,
            )),
            Style::default().fg(app.theme.fg_primary),
        )),
    ];
    let para = Paragraph::new(explainer).wrap(Wrap { trim: false }).block(
        Block::default()
            .borders(Borders::ALL)
            .title(i18n.ui("Bass Anchor")),
    );
    f.render_widget(para, inner[0]);

    // --- Status banner -------------------------------------------------
    let (status_text, status_color) = match &bac.status {
        BassAnchorCaptureStatus::Idle => (
            format!("Idle — optional step ({:.0} ms / channel).", tone_ms),
            app.theme.fg_secondary,
        ),
        BassAnchorCaptureStatus::Running { .. } => (
            "Capturing bass anchor…".to_string(),
            app.theme.accent_primary,
        ),
        BassAnchorCaptureStatus::Complete => (
            format!(
                "Complete — {} channel(s) analysed",
                bac.results.as_ref().map(|r| r.channels.len()).unwrap_or(0)
            ),
            app.theme.accent_success,
        ),
        BassAnchorCaptureStatus::Failed(e) => (format!("Failed: {e}"), app.theme.accent_error),
    };
    f.render_widget(
        Paragraph::new(i18n.dynamic(status_text))
            .style(Style::default().fg(status_color))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(i18n.ui("Status")),
            ),
        inner[1],
    );

    // --- Results table -------------------------------------------------
    if let Some(results) = bac.results.as_ref() {
        let header = Row::new(vec![
            Cell::from(i18n.ui("Channel")),
            Cell::from(i18n.ui("Phase °")),
            Cell::from(i18n.ui("|mag|")),
            Cell::from(i18n.ui("Stab °")),
            Cell::from(i18n.ui("Quality")),
        ])
        .style(
            Style::default()
                .fg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = results
            .channels
            .iter()
            .map(|ch| {
                // 20° advisory threshold (§2.8 of the GD-Opt v2 plan,
                // `docs/gd_opt_v2_plan.md` in the autoeq repo).
                let reliable = ch.bass_anchor_stability_deg < 20.0;
                let (quality, color) = if reliable {
                    ("OK", app.theme.accent_success)
                } else {
                    ("⚠ unreliable (>20°)", app.theme.accent_error)
                };
                Row::new(vec![
                    Cell::from(ch.channel_name.clone()),
                    Cell::from(format!("{:+.1}", ch.bass_anchor_phase_deg)),
                    Cell::from(format!("{:.3}", ch.bass_anchor_magnitude)),
                    Cell::from(format!("{:.1}", ch.bass_anchor_stability_deg)),
                    Cell::from(i18n.dynamic(quality.to_string())).style(Style::default().fg(color)),
                ])
            })
            .collect();
        let table = Table::new(
            rows,
            [
                Constraint::Length(14),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Min(10),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(i18n.dynamic(format!("Results (sample rate {} Hz)", results.sample_rate))),
        );
        f.render_widget(table, inner[2]);
    } else {
        let empty = Paragraph::new(i18n.ui(" No bass-anchor results yet — optional step."))
            .style(Style::default().fg(app.theme.fg_secondary))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(i18n.ui("Results")),
            );
        f.render_widget(empty, inner[2]);
    }

    // --- Help ----------------------------------------------------------
    let help = i18n.dynamic(" Tab/BackTab=switch step  (display-only — optional)".to_string());
    f.render_widget(
        Paragraph::new(help).style(Style::default().fg(app.theme.fg_secondary)),
        inner[3],
    );
}

/// SPL Calibration step renderer (GD-Opt v2 Phase GD-1e.5).
///
/// Mirrors the GPUI wizard surface (`spl_calibration.rs`):
///   - Form fields for tone parameters (freq / amp / duration / output ch / input ch).
///   - Run/Cancel pseudo-button row.
///   - Engine result + meter-reading input + derived `spl_offset_db`.
fn draw_recording_spl_calibration_step(f: &mut Frame, content: Rect, app: &App) {
    let i18n = crate::i18n::TuiTranslations::for_language(app.ui.language);
    use sotf_audio_player::recording_types::SplCalibrationCaptureStatus;

    let s = &app.recording;
    let cal = &s.model.spl_calibration_capture;

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // form (5 fields + run + meter reading)
            Constraint::Length(3),  // status
            Constraint::Min(4),     // result / derived offset
            Constraint::Length(1),  // help
        ])
        .split(content);

    let focused = |idx: usize| -> Style {
        if s.spl_selected_field == idx {
            Style::default()
                .fg(app.theme.accent_primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.fg_primary)
        }
    };
    let is_editing = |idx: usize| s.spl_editing_value && s.spl_selected_field == idx;
    let field_text = |idx: usize, val: String, placeholder: &str| -> String {
        if is_editing(idx) {
            format!("> {}_", s.edit_buffer)
        } else if val.is_empty() {
            format!("<{}>", placeholder)
        } else {
            val
        }
    };

    let running = matches!(cal.status, SplCalibrationCaptureStatus::Running { .. });
    let run_label = i18n.dynamic(
        if running {
            "[ Cancel tone ]"
        } else {
            match cal.status {
                SplCalibrationCaptureStatus::Idle | SplCalibrationCaptureStatus::Failed(_) => {
                    "[ Play calibration tone ]"
                }
                SplCalibrationCaptureStatus::Running { .. } => "[ Playing… ]",
                SplCalibrationCaptureStatus::Complete => "[ Re-play tone ]",
            }
        }
        .to_string(),
    );
    let run_state = i18n.dynamic(
        match &cal.status {
            SplCalibrationCaptureStatus::Idle => "press r or Enter",
            SplCalibrationCaptureStatus::Running { .. } => "running…",
            SplCalibrationCaptureStatus::Complete => "done",
            SplCalibrationCaptureStatus::Failed(_) => "failed",
        }
        .to_string(),
    );

    let reported_str = match cal.reported_db_spl {
        Some(v) => format!("{:.1}", v),
        None => String::new(),
    };

    let form_rows = vec![
        Row::new(vec![
            Cell::from(i18n.ui("Reference freq (Hz)")).style(focused(0)),
            Cell::from(field_text(
                0,
                format!("{:.0}", cal.reference_freq_hz),
                "1000",
            ))
            .style(focused(0)),
        ]),
        Row::new(vec![
            Cell::from(i18n.ui("Tone amplitude (0-1)")).style(focused(1)),
            Cell::from(field_text(1, format!("{:.3}", cal.tone_amp), "0.250")).style(focused(1)),
        ]),
        Row::new(vec![
            Cell::from(i18n.ui("Duration (s)")).style(focused(2)),
            Cell::from(field_text(2, format!("{:.1}", cal.duration_s), "3.0")).style(focused(2)),
        ]),
        Row::new(vec![
            Cell::from(i18n.ui("Output channel")).style(focused(3)),
            Cell::from(field_text(3, format!("{}", cal.output_channel), "0")).style(focused(3)),
        ]),
        Row::new(vec![
            Cell::from(i18n.ui("Mic input channel")).style(focused(4)),
            Cell::from(field_text(4, format!("{}", cal.input_channel), "0")).style(focused(4)),
        ]),
        Row::new(vec![
            Cell::from(run_label).style(focused(5)),
            Cell::from(run_state).style(focused(5)),
        ]),
        Row::new(vec![
            Cell::from(i18n.ui("Reported dBSPL")).style(focused(6)),
            Cell::from(field_text(
                6,
                reported_str,
                &i18n.dynamic("type meter reading".to_string()),
            ))
            .style(focused(6)),
        ]),
    ];
    let form = Table::new(
        form_rows,
        [Constraint::Length(24), Constraint::Percentage(60)],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(i18n.ui("SPL Calibration")),
    );
    f.render_widget(form, inner[0]);

    // --- Status banner ------------------------------------------------
    let (status_text, status_color) = match &cal.status {
        SplCalibrationCaptureStatus::Idle => (
            format!(
                "Ready — {:.0} Hz @ amp {:.3} for {:.1}s on ch {}",
                cal.reference_freq_hz, cal.tone_amp, cal.duration_s, cal.output_channel
            ),
            app.theme.fg_secondary,
        ),
        SplCalibrationCaptureStatus::Running { .. } => (
            "Tone playing — read your SPL meter now…".to_string(),
            app.theme.accent_primary,
        ),
        SplCalibrationCaptureStatus::Complete => (
            match cal.engine_result.as_ref() {
                Some(r) => format!(
                    "Tone captured — peak {:.4}, RMS {:.4}. Enter the dBSPL your meter showed.",
                    r.peak_sample_level, r.rms_sample_level
                ),
                None => "Complete".to_string(),
            },
            app.theme.accent_success,
        ),
        SplCalibrationCaptureStatus::Failed(e) => (format!("Failed: {e}"), app.theme.accent_error),
    };
    f.render_widget(
        Paragraph::new(i18n.dynamic(status_text))
            .style(Style::default().fg(status_color))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(i18n.ui("Status")),
            ),
        inner[1],
    );

    // --- Engine result + derived offset -------------------------------
    let mut detail_lines: Vec<Line> = Vec::new();
    if let Some(r) = cal.engine_result.as_ref() {
        detail_lines.push(Line::from(Span::styled(
            i18n.dynamic(format!(
                " Sample rate {} Hz  •  peak {:.4}  •  RMS {:.4}  •  ref {:.0} Hz  •  out ch {}",
                r.sample_rate,
                r.peak_sample_level,
                r.rms_sample_level,
                r.reference_freq_hz,
                r.output_channel
            )),
            Style::default().fg(app.theme.fg_primary),
        )));
        match cal.reported_db_spl {
            Some(v) => detail_lines.push(Line::from(Span::styled(
                i18n.dynamic(format!(" Reported dBSPL: {:.1}", v)),
                Style::default().fg(app.theme.fg_primary),
            ))),
            None => detail_lines.push(Line::from(Span::styled(
                i18n.dynamic(
                    " Reported dBSPL: (move to the field below and type your meter reading)"
                        .to_string(),
                ),
                Style::default().fg(app.theme.fg_secondary),
            ))),
        }
        if let Some(out) = cal.to_spl_calibration() {
            detail_lines.push(Line::from(Span::styled(
                i18n.dynamic(format!(
                    " → spl_offset_db = {:.2}  (will be stored on Save)",
                    out.spl_offset_db
                )),
                Style::default().fg(app.theme.accent_success),
            )));
        }
    } else {
        detail_lines.push(Line::from(Span::styled(
            i18n.dynamic(
                " No tone captured yet — press r (or Enter on the Run row) to play the reference tone."
                    .to_string(),
            ),
            Style::default().fg(app.theme.fg_secondary),
        )));
    }
    f.render_widget(
        Paragraph::new(detail_lines)
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(i18n.ui("Result")),
            ),
        inner[2],
    );

    // --- Help ---------------------------------------------------------
    let help = i18n.dynamic(
        if s.spl_editing_value {
            " Type value | Enter=confirm | Esc=cancel"
        } else if running {
            " r/Enter=cancel  Tab/BackTab=field  ←→=adjust"
        } else {
            " Tab=next field  ←→=adjust  Enter=edit/run  r=run  Tab=Capture"
        }
        .to_string(),
    );
    f.render_widget(
        Paragraph::new(help).style(Style::default().fg(app.theme.fg_secondary)),
        inner[3],
    );
}

#[cfg(test)]
mod localization_render_tests {
    use super::*;
    use crate::{
        app::{FederationMode, HeadphoneEqStep, InputMode, PlaylistMode, SpinoramaStep},
        i18n::Language,
        theme::Theme,
    };
    use ratatui::{Terminal, backend::TestBackend};
    use sotf_audio_player::{recording_types::RecordingStep, room_eq_types::RoomEqStep};

    fn render(terminal: &mut Terminal<TestBackend>, app: &App, screen: fn(&mut Frame, Rect, &App)) {
        terminal
            .draw(|frame| screen(frame, frame.area(), app))
            .unwrap();
    }

    fn render_mut(
        terminal: &mut Terminal<TestBackend>,
        app: &mut App,
        screen: fn(&mut Frame, Rect, &mut App),
    ) {
        terminal
            .draw(|frame| screen(frame, frame.area(), app))
            .unwrap();
    }

    fn render_dialog(
        terminal: &mut Terminal<TestBackend>,
        app: &App,
        dialog: fn(&mut Frame, &App),
    ) {
        terminal.draw(|frame| dialog(frame, app)).unwrap();
    }

    #[test]
    fn domain_workflows_render_in_every_locale() {
        let mut terminal = Terminal::new(TestBackend::new(160, 60)).unwrap();
        let mut app = App::new(Theme::default(), false);

        for language in Language::ALL {
            app.ui.language = language;
            render_dialog(&mut terminal, &app, draw_loading_screen);
            render(&mut terminal, &app, draw_library_screen);
            render_mut(&mut terminal, &mut app, draw_queue_screen);
            render(&mut terminal, &app, draw_playlists_screen);
            for mode in [
                PlaylistMode::Create,
                PlaylistMode::Rename,
                PlaylistMode::ConfirmDelete,
                PlaylistMode::Tracks,
            ] {
                app.playlists.mode = mode;
                render(&mut terminal, &app, draw_playlists_screen);
            }
            app.playlists.mode = PlaylistMode::List;

            render(&mut terminal, &app, draw_plugins_screen);
            app.input_mode = InputMode::AddPlugin;
            render(&mut terminal, &app, draw_plugins_screen);
            app.input_mode = InputMode::Normal;
            render(&mut terminal, &app, draw_devices_screen);
            render(&mut terminal, &app, draw_configure_screen);

            app.library_view.editing_directory = true;
            app.library_view.directory_input = "/music".to_string();
            app.autocomplete.menu_active = true;
            app.autocomplete.suggestions = vec!["/music/example".to_string()];
            render(&mut terminal, &app, draw_directory_manager);
            app.library_view.editing_directory = false;
            app.autocomplete.menu_active = false;

            render(&mut terminal, &app, draw_federation_screen);
            app.federation.state.mode = FederationMode::AddSource;
            render(&mut terminal, &app, draw_federation_screen);
            app.federation.state.mode = FederationMode::List;

            // Service-login panel (Tidal prompt, Spotify OAuth, starting) in
            // every locale — exercises the dynamic catalog entries.
            let login_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            for status in [
                crate::app::ServiceLoginStatus::Starting,
                crate::app::ServiceLoginStatus::TidalDevicePrompt {
                    verification_url: "https://link.tidal.com/ABCDE".to_string(),
                    user_code: "ABCD-EFGH".to_string(),
                    expires_in_secs: 300,
                    started: std::time::Instant::now(),
                },
                crate::app::ServiceLoginStatus::SpotifyOAuth {
                    url: "https://accounts.spotify.com/authorize?...".to_string(),
                    started: std::time::Instant::now(),
                },
            ] {
                app.federation.state.login = Some(crate::app::ServiceLoginState {
                    source_id: "tidal:test".to_string(),
                    status,
                    cancel: std::sync::Arc::clone(&login_cancel),
                });
                render(&mut terminal, &app, draw_federation_screen);
            }
            app.federation.state.login = None;
            render(&mut terminal, &app, draw_servers_screen);
            app.server_state.editing_value = true;
            render(&mut terminal, &app, draw_servers_screen);
            app.server_state.editing_value = false;

            app.ui.status_message = Some("No directories to scan".to_string());
            terminal
                .draw(|frame| draw_status_bar(frame, frame.area(), &app))
                .unwrap();
            app.ui.status_message = Some("opaque external service detail".to_string());
            terminal
                .draw(|frame| draw_status_bar(frame, frame.area(), &app))
                .unwrap();
            app.ui.status_message = None;

            terminal
                .draw(|frame| draw_meters_column(frame, frame.area(), &mut app))
                .unwrap();

            render_dialog(&mut terminal, &app, draw_scan_progress_dialog);
            render_dialog(&mut terminal, &app, draw_maintenance_progress_dialog);
            render_dialog(&mut terminal, &app, draw_replay_gain_progress_dialog);
            render_dialog(&mut terminal, &app, draw_save_plugins_dialog);
            render_dialog(&mut terminal, &app, draw_load_plugins_dialog);
            app.plugin_rack.file_input = "example".to_string();
            render_dialog(&mut terminal, &app, draw_save_plugins_dialog);
            render_dialog(&mut terminal, &app, draw_load_plugins_dialog);
            app.plugin_rack.file_input.clear();
            render_dialog(&mut terminal, &app, draw_load_apo_file_dialog);
            render_dialog(&mut terminal, &app, draw_load_sofa_file_dialog);

            app.file_explorer.picker_title = "Select Music Directory".to_string();
            render_dialog(&mut terminal, &app, draw_file_explorer_modal);

            app.ui.error_message = Some("opaque external audio error".to_string());
            render_dialog(&mut terminal, &app, draw_error_modal);
            app.ui.error_message = None;
            render_dialog(&mut terminal, &app, draw_channel_conflict_modal);

            for screen in [
                Screen::Loading,
                Screen::Library,
                Screen::Queue,
                Screen::Playlists,
                Screen::Plugins,
                Screen::Devices,
                Screen::Tools,
                Screen::EarTraining,
                Screen::AbTesting,
                Screen::Configure,
            ] {
                app.current_screen = screen;
                render_dialog(&mut terminal, &app, draw_help_modal);
            }

            terminal
                .draw(|frame| {
                    let area = frame.area();
                    draw_freq_response_chart(frame, area, &app, &[], &[], &[], &[]);
                })
                .unwrap();
            terminal
                .draw(|frame| {
                    let area = frame.area();
                    draw_freq_response_chart(
                        frame,
                        area,
                        &app,
                        &[20.0, 20_000.0],
                        &[0.0, 1.0],
                        &[0.0, 0.0],
                        &[0.0, -1.0],
                    );
                })
                .unwrap();

            for step in RecordingStep::all() {
                app.recording.model.step = *step;
                render(&mut terminal, &app, draw_recording_screen);
            }
            for step in [
                HeadphoneEqStep::SelectFile,
                HeadphoneEqStep::Configure,
                HeadphoneEqStep::Optimize,
                HeadphoneEqStep::Results,
                HeadphoneEqStep::UpdatePlugin,
            ] {
                app.headphone_eq.step = step;
                render(&mut terminal, &app, draw_headphone_eq_screen);
            }
            for step in RoomEqStep::all() {
                app.room_eq.model.step = *step;
                render(&mut terminal, &app, draw_room_eq_screen);
            }
            for step in [
                SpinoramaStep::Select,
                SpinoramaStep::Configure,
                SpinoramaStep::Optimize,
                SpinoramaStep::Results,
                SpinoramaStep::UpdatePlugin,
            ] {
                app.spinorama_eq.step = step;
                render(&mut terminal, &app, draw_spinorama_eq_screen);
            }
        }
    }
}
