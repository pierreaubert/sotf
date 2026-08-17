use qrcode::QrCode;
use qrcode::render::unicode;
use sotf_audio_player::Player;
use sotf_audio_player_tui::app::{App, InputMode};
#[cfg(feature = "dev-api")]
use sotf_audio_player_tui::dev_api::{DevCommand, DevQueryReply, DevReply};
use sotf_audio_player_tui::events::PlayerCommand;
use sotf_audio_player_tui::media_controls::TuiMediaControls;
use sotf_media_controls::{MediaPlayback, MediaPosition};
use std::time::Duration;

pub(super) fn print_sotf_api_connection_qr() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = sotf_audio_player::config::load_server_config()?;
    if sotf_audio_player::server::ensure_sotf_api_connection_config(&mut config) {
        sotf_audio_player::config::save_server_config(&config)?;
    }

    let payload = sotf_audio_player::server::sotf_api_connection_qr_payload(&config.api)?;
    let url = sotf_audio_player::server::sotf_api_server_url_for_settings(&config.api);
    let token = config.api.auth_token.as_deref().unwrap_or_default();
    let code = QrCode::new(payload.as_bytes())?;
    let qr = code.render::<unicode::Dense1x2>().quiet_zone(true).build();

    println!("SOTF API connection QR");
    println!("Name: {}", config.api.friendly_name);
    println!("URL: {url}");
    println!("Token: {token}");
    println!("Payload: {payload}");
    println!();
    println!("{qr}");

    Ok(())
}

/// Compute a cheap signature of loudness data for redraw gating.
/// Rounded to 0.1 dB so the meter "ticks" register as changes while
/// noise-floor jitter does not force a redraw every tick.
pub(super) fn loudness_redraw_signature(l: Option<&sotf_audio_player::LoudnessData>) -> u64 {
    let Some(l) = l else {
        return 0;
    };
    let q = |x: f64| -> u64 {
        if !x.is_finite() {
            return u64::MAX;
        }
        ((x * 10.0).round() as i64).wrapping_add(i64::MAX / 2) as u64
    };
    let mut sig = q(l.momentary_lufs)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(q(l.shortterm_lufs));
    sig = sig
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(q(l.integrated_lufs));
    for p in l.channel_peaks.iter() {
        let v = if *p > 0.0 {
            (20.0 * (*p).log10()).max(-120.0)
        } else {
            -120.0
        };
        sig = sig.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(q(v));
    }
    sig
}

/// Compute a cheap signature of signal-path data for redraw gating.
/// Changes in sample rates, resampling state, or engine health drive
/// a redraw without allocating in the tick.
pub(super) fn signal_path_redraw_signature(p: Option<&sotf_audio_player::SignalPath>) -> u64 {
    let Some(p) = p else {
        return 0;
    };
    let source_rate = p.source.as_ref().map_or(0, |s| s.sample_rate_hz);
    let mut sig = (source_rate as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(p.output.sample_rate_hz);
    sig = sig
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(p.is_resampled() as u64);
    sig = sig
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(p.health.underruns);
    sig = sig
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(p.health.stream_errors);
    sig = sig
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(p.health.frames_dropped);
    sig = sig
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(p.health.clipping_detected.map_or(2, |c| c as u64));
    sig
}

/// Start playback for an audio source, handling matrix adaptation and channel clamping.
pub(super) fn start_playback(
    player: &mut Player,
    app: &mut App,
    source: sotf_audio::decoder::AudioSource,
    track_channels: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let track_sample_rate = app
        .current_track()
        .and_then(|t| t.sample_rate)
        .unwrap_or(48000);
    let sample_rate = app.get_target_sample_rate(track_sample_rate);

    log::info!(
        "[TUI] Starting playback: track={}Hz, target={}Hz, device_default={}Hz",
        track_sample_rate,
        sample_rate,
        app.get_current_sample_rate()
    );

    app.plugin_rack.graph.adapt_matrix_to_input(track_channels);

    // Apply ReplayGain correction to the permanent Gain plugin
    let rg_gain = app.get_replay_gain_for_current_track();
    app.plugin_rack.graph.set_replay_gain(rg_gain);

    // `load_and_play_source` only accepts a flat `Vec<PluginConfig>`,
    // which cannot express the DAG topology a non-linear plugin
    // graph (parallel branches, routed bass management) needs. If the
    // graph is non-linear, schedule a structural-flush plugin update
    // on the next tick so the real `PluginGraphConfig` is uploaded
    // via `update_plugin_graph` (the same path `run_app` uses for
    // in-place updates). Without this, pressing Play silently drops
    // routed RoomEQ topology until a parameter twiddle re-triggers a
    // structural flush.
    if !app.plugin_rack.graph.is_linear() {
        app.plugin_rack.needs_update = true;
        app.plugin_rack.update_retry_count = 0;
        app.plugin_rack.update_last_attempt = None;
    }

    let plugins = app.plugin_rack.graph.to_plugin_configs(sample_rate);
    let mut output_channels = app
        .plugin_rack
        .graph
        .output_channels_for_input(track_channels);

    let device_max = app.get_device_max_channels();
    log::info!(
        "[TUI] Plugin chain wants {} output channels, device max = {:?}",
        output_channels,
        device_max,
    );

    // Clamp output channels to device max — the playback thread will
    // downmix automatically when the processing chain outputs more
    // channels than the hardware supports.
    if let Some(max_channels) = device_max
        && output_channels > max_channels
    {
        log::info!(
            "[TUI] Clamping output from {} to {} channels (device limit)",
            output_channels,
            max_channels
        );
        output_channels = max_channels;
    }

    // Sync volume to the engine before playback starts
    player.set_volume(app.playback.volume)?;

    // Service streams (Tidal/Spotify) are resolved by the engine decoder
    // thread via the resolver hook installed at startup — no pre-resolution
    // here, failures surface as decode-time errors.
    let source_path = source.as_path().map(|p| p.to_path_buf());
    player.load_and_play_source(
        source,
        plugins,
        output_channels,
        app.audio_devices.current_output_name.clone(),
    )?;

    if let Some(path) = source_path {
        app.start_track_tracking(path);
    }
    Ok(())
}

pub(super) fn handle_player_command(
    player: &mut Player,
    app: &mut App,
    cmd: PlayerCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        PlayerCommand::Play(path) => {
            // Cancel any pending gapless queue before manual play
            let _ = player.cancel_next();
            // Stop tracking previous track if any
            app.stop_track_tracking();

            // Load album images when starting playback
            #[cfg(not(target_os = "windows"))]
            app.load_album_images();

            let track_channels = app.current_track().and_then(|t| t.channels).unwrap_or(2) as usize;

            // Clear suspensions from previous track
            app.plugin_rack.graph.clear_suspensions();
            app.plugin_rack.graph.update_channel_dependent_plugins();

            // Check for channel conflicts with all fixed-channel plugins
            let conflicts = app.plugin_rack.graph.find_channel_conflicts(track_channels);
            if !conflicts.is_empty() {
                log::info!(
                    "[TUI] Channel conflict: {}ch file with {} incompatible plugin(s)",
                    track_channels,
                    conflicts.len()
                );
                app.modal.channel_conflicts = conflicts;
                app.modal.channel_conflict_path = Some(path);
                app.modal.channel_conflict_selection = 0;
                app.modal.channel_conflict_track_channels = track_channels;
                app.enter_overlay_mode(InputMode::ChannelConflict);
                return Ok(());
            }

            start_playback(player, app, path, track_channels)?;
        }
        PlayerCommand::PlayResolved(path) => {
            // Play after channel conflict was resolved — skip clearing suspensions
            // and conflict re-check since the user already handled it.
            app.stop_track_tracking();
            #[cfg(not(target_os = "windows"))]
            app.load_album_images();
            let track_channels = app.current_track().and_then(|t| t.channels).unwrap_or(2) as usize;
            start_playback(player, app, path, track_channels)?;
        }
        PlayerCommand::Pause => {
            player.pause()?;
        }
        PlayerCommand::Resume => {
            player.resume()?;
        }
        PlayerCommand::Stop => {
            player.stop()?;
            // Stop tracking when playback stops
            app.stop_track_tracking();
        }
        PlayerCommand::SetVolume(volume) => {
            player.set_volume(volume)?;
        }
        PlayerCommand::SetOutputDevice(device_name) => {
            // Store the device name for future playback
            app.audio_devices.current_output_name = Some(device_name.clone());
            player.set_output_device(device_name.clone())?;
            app.ui.status_message = Some(format!(
                "Output device set to '{}'; will be used for next playback",
                device_name
            ));
            log::info!("Output device changed");
        }
        PlayerCommand::Seek(position) => {
            player.seek(position)?;
            log::info!("Seeked to {} seconds", position);
        }
        PlayerCommand::SeekRelative(offset) => {
            let current_pos = player.get_position();
            let new_pos = (current_pos + offset).max(0.0);
            player.seek(new_pos)?;
            log::info!(
                "Seeked {} seconds (from {} to {})",
                offset,
                current_pos,
                new_pos
            );
        }
        PlayerCommand::ToggleMute => {
            app.playback.muted = !app.playback.muted;
            player.set_mute(app.playback.muted)?;
            log::info!("Mute toggled: {}", app.playback.muted);
        }
    }
    Ok(())
}

pub(super) fn update_media_controls(
    app: &mut App,
    player: &Player,
    media_controls: &mut Option<TuiMediaControls>,
) {
    let Some(mc) = media_controls.as_mut() else {
        return;
    };

    // Snapshot the desired metadata.
    let track = app.current_track();
    let album_title = app
        .playback
        .current_queue_index
        .and_then(|idx| app.queue.get(idx))
        .map(|entry| entry.item.album.title.clone())
        .filter(|s| !s.is_empty());

    let title = track
        .and_then(|t| t.title.clone())
        .filter(|s| !s.is_empty());
    let artist = track
        .and_then(|t| t.artist.clone())
        .filter(|s| !s.is_empty());
    let duration_secs = track.and_then(|t| t.duration_secs);

    let cover_url = app
        .playback
        .current_queue_index
        .and_then(|idx| app.queue.get(idx))
        .and_then(|entry| entry.item.album.album_art_path.as_ref())
        .filter(|path| path.exists())
        .map(|path| format!("file://{}", path.display()));

    // Only push metadata to the OS when something actually changed —
    // every call crosses an FFI boundary (e.g. macOS
    // MPNowPlayingInfoCenter) and we tick at ~10 Hz.
    let metadata_changed = app.media_control.last_queue_index != app.playback.current_queue_index
        || app.media_control.last_title != title
        || app.media_control.last_artist != artist
        || app.media_control.last_album != album_title
        || app.media_control.last_cover_url != cover_url
        || app.media_control.last_duration_secs != duration_secs;

    if metadata_changed {
        mc.set_metadata(
            title.as_deref(),
            artist.as_deref(),
            album_title.as_deref(),
            duration_secs.map(Duration::from_secs),
            cover_url.as_deref(),
        );
        app.media_control.last_queue_index = app.playback.current_queue_index;
        app.media_control.last_title = title;
        app.media_control.last_artist = artist;
        app.media_control.last_album = album_title;
        app.media_control.last_cover_url = cover_url;
        app.media_control.last_duration_secs = duration_secs;
    }

    let position_secs = player.get_position();
    let progress = Some(MediaPosition::from_secs_f64(position_secs));

    let playback = if app.playback.is_playing {
        MediaPlayback::Playing { progress }
    } else if app.playback.current_queue_index.is_some() {
        MediaPlayback::Paused { progress }
    } else {
        MediaPlayback::Stopped
    };

    mc.set_playback(playback);
}

#[cfg(feature = "dev-api")]
pub(super) fn process_dev_command(
    app: &mut App,
    player: &mut Player,
    media_controls: &mut Option<TuiMediaControls>,
    cmd: DevCommand,
) {
    use sotf_audio_player_tui::events::handle_key_event;

    match cmd {
        DevCommand::Action {
            name,
            payload,
            reply,
        } => {
            let result = dispatch_tui_action(app, &name, payload);
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::Query { path, reply } => {
            let result = sotf_audio_player_tui::dev_api::queries::resolve(&path, app);
            let dev_reply = match result {
                Ok(value) => DevQueryReply::ok(value),
                Err(e) => DevQueryReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::Key { keystroke, reply } => {
            let result = parse_keystroke(&keystroke)
                .map(|key| {
                    if let Some(cmd) = handle_key_event(app, key) {
                        if let Err(e) = handle_player_command(player, app, cmd) {
                            log::error!("[dev-api] Player command error: {}", e);
                            app.ui.error_message = Some(e.to_string());
                            app.enter_overlay_mode(InputMode::ShowError);
                            app.playback.is_playing = false;
                        }
                        update_media_controls(app, player, media_controls);
                    }
                    Ok(())
                })
                .unwrap_or_else(Err);
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::Health { reply } => {
            let payload = serde_json::json!({
                "ok": true,
                "pid": std::process::id(),
                "screen": format!("{:?}", app.current_screen),
                "queue_length": app.queue.len(),
            });
            let _ = reply.send(DevQueryReply::ok(payload));
        }
        DevCommand::Quit { reply } => {
            app.should_quit = true;
            let _ = reply.send(DevReply::ok());
        }
        DevCommand::QaSeed { reply } => {
            let _ = reply.send(DevReply::err("qa seed not yet implemented for TUI"));
        }
    }
}

#[cfg(feature = "dev-api")]
pub(super) fn dispatch_tui_action(
    app: &mut App,
    name: &str,
    payload: Option<serde_json::Value>,
) -> anyhow::Result<()> {
    use sotf_audio_player::MetadataController;
    use sotf_audio_player_tui::app::{MetadataEditorState, Screen};

    match name {
        "PluginClear" => {
            app.clear_plugins();
            return Ok(());
        }
        "PluginAdd" => {
            let plugin_type = payload_str(&payload, "plugin_type")?;
            let ty = sotf_audio_player::PluginType::from_name(plugin_type)
                .ok_or_else(|| anyhow::anyhow!("unknown plugin type `{plugin_type}`"))?;
            app.add_plugin(&ty);
            return Ok(());
        }
        "PluginRemove" => {
            let idx = payload_u64(payload.as_ref(), "index")? as usize;
            app.remove_plugin(idx);
            return Ok(());
        }
        "PluginToggle" => {
            let idx = payload_u64(payload.as_ref(), "index")? as usize;
            app.toggle_plugin(idx);
            return Ok(());
        }
        "PluginMoveUp" => {
            let idx = payload_u64(payload.as_ref(), "index")? as usize;
            app.move_plugin_up(idx);
            return Ok(());
        }
        "PluginMoveDown" => {
            let idx = payload_u64(payload.as_ref(), "index")? as usize;
            app.move_plugin_down(idx);
            return Ok(());
        }
        "PluginSetParam" => {
            let idx = payload_u64(payload.as_ref(), "index")? as usize;
            let param_idx = payload_u64(payload.as_ref(), "param_index")? as usize;
            let value = payload_f64(payload.as_ref(), "value")?;
            app.set_plugin_param(idx, param_idx, value);
            return Ok(());
        }
        "PluginSetParamString" => {
            let idx = payload_u64(payload.as_ref(), "index")? as usize;
            let param_idx = payload_u64(payload.as_ref(), "param_index")? as usize;
            let value = payload_str(&payload, "value")?.to_string();
            let plugin = app
                .plugin_rack
                .graph
                .get_plugin_mut(idx)
                .ok_or_else(|| anyhow::anyhow!("plugin index {idx} out of range"))?;
            // Reuse logic from Task 2; consider exposing a shared helper
            sotf_audio_player::controllers::plugin::dev_api::actions::set_string_param(
                &mut plugin.settings,
                param_idx,
                value,
            )?;
            app.plugin_rack.graph.update_channel_dependent_plugins();
            app.request_plugin_update();
            return Ok(());
        }
        "PluginChainSave" => {
            let path = std::path::Path::new(payload_str(&payload, "path")?);
            app.save_plugins_to_path(path)
                .map_err(|e| anyhow::anyhow!(e))?;
            return Ok(());
        }
        "PluginChainLoad" => {
            let path = std::path::Path::new(payload_str(&payload, "path")?);
            app.load_plugins_from_path(path)
                .map_err(|e| anyhow::anyhow!(e))?;
            return Ok(());
        }
        _ => {}
    }

    match name {
        "PlayPause" => {
            app.playback.is_playing = !app.playback.is_playing;
        }
        "Stop" => {
            app.playback.is_playing = false;
        }
        "VolumeUp" => {
            app.playback.volume = (app.playback.volume + 0.05).min(1.0);
        }
        "VolumeDown" => {
            app.playback.volume = (app.playback.volume - 0.05).max(0.0);
        }
        "Mute" => {
            app.playback.muted = !app.playback.muted;
        }
        "SwitchToLibrary" => {
            app.current_screen = Screen::Library;
            app.input_mode = InputMode::Normal;
        }
        "SwitchToQueue" => {
            app.current_screen = Screen::Queue;
            app.input_mode = InputMode::Normal;
        }
        "SwitchToConfigure" => {
            app.current_screen = Screen::Configure;
            app.input_mode = InputMode::Configure;
        }
        "SwitchToPlugins" => {
            app.current_screen = Screen::Plugins;
            app.input_mode = InputMode::Normal;
        }
        "SwitchToDevices" => {
            app.current_screen = Screen::Devices;
            app.input_mode = InputMode::Normal;
        }
        "SwitchToPlaylists" => {
            app.current_screen = Screen::Playlists;
            app.input_mode = InputMode::Normal;
        }
        "MetadataSeedAlbum" => {
            app.library.albums =
                vec![sotf_audio_player::dev_api_fixtures::metadata_fixture_album()];
            app.library_view.selected_album_index = 0;
            app.request_filter_update();
            let _ = app.filtered_albums();
            app.current_screen = Screen::Library;
            app.input_mode = InputMode::Normal;
            app.modal.metadata_editor = None;
        }
        "MetadataOpenAlbumEditor" => {
            let index = app.library_view.selected_album_index;
            let album = app
                .filtered_albums()
                .get(index)
                .cloned()
                .or_else(|| app.library.albums.first().cloned())
                .ok_or_else(|| anyhow::anyhow!("no album available for metadata editor"))?;
            app.modal.metadata_editor = Some(
                MetadataEditorState::for_album(&album)
                    .map_err(|err| anyhow::anyhow!("metadata editor unavailable: {err}"))?,
            );
            app.input_mode = InputMode::MetadataEditor;
        }
        "MetadataSetField" => {
            let editor = app
                .modal
                .metadata_editor
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("metadata editor is not open"))?;
            let field = payload_str(&payload, "field")?;
            let value = payload_str(&payload, "value")?;
            let field_index = metadata_field_index(field)
                .ok_or_else(|| anyhow::anyhow!("unknown metadata field `{field}`"))?;
            editor.set_field_value(field_index, value.to_string());
        }
        "MetadataPreview" => {
            let (target, patch) = {
                let editor = app
                    .modal
                    .metadata_editor
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("metadata editor is not open"))?;
                (
                    editor.target.clone(),
                    editor
                        .patch()
                        .map_err(|err| anyhow::anyhow!("invalid metadata patch: {err}"))?,
                )
            };
            let result = MetadataController::preview_edit(&app.library, target, patch);
            let editor = app
                .modal
                .metadata_editor
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("metadata editor is not open"))?;
            match result {
                Ok(preview) => {
                    editor.preview = Some(preview);
                    editor.error = None;
                }
                Err(err) => {
                    editor.error = Some(err.to_string());
                    return Err(anyhow::anyhow!(err.to_string()));
                }
            }
        }
        "MetadataInjectCandidate" => {
            let editor = app
                .modal
                .metadata_editor
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("metadata editor is not open"))?;
            editor
                .search_results
                .push(metadata_candidate_from_payload(payload.as_ref()));
            editor.selected_result = editor.search_results.len().saturating_sub(1);
            editor.search_error = None;
        }
        "MetadataImportCandidate" => {
            let editor = app
                .modal
                .metadata_editor
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("metadata editor is not open"))?;
            let candidate = editor
                .search_results
                .get(editor.selected_result)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no metadata candidate selected"))?;
            editor.apply_candidate(candidate);
        }
        "MetadataClose" => {
            app.modal.metadata_editor = None;
            app.input_mode = InputMode::Normal;
        }
        _ => return Err(anyhow::anyhow!("unknown action: `{name}`")),
    }
    Ok(())
}

#[cfg(feature = "dev-api")]
fn payload_str<'a>(payload: &'a Option<serde_json::Value>, key: &str) -> anyhow::Result<&'a str> {
    payload
        .as_ref()
        .and_then(|value| value.get(key))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("metadata action payload needs string `{key}`"))
}

#[cfg(feature = "dev-api")]
fn payload_u32(payload: Option<&serde_json::Value>, key: &str, default: u32) -> u32 {
    payload
        .and_then(|value| value.get(key))
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(default)
}

#[cfg(feature = "dev-api")]
fn payload_u8(payload: Option<&serde_json::Value>, key: &str, default: u8) -> u8 {
    payload
        .and_then(|value| value.get(key))
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(default)
}

#[cfg(feature = "dev-api")]
fn payload_string(payload: Option<&serde_json::Value>, key: &str, default: &str) -> Option<String> {
    payload
        .and_then(|value| value.get(key))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| Some(default.to_string()))
}

#[cfg(feature = "dev-api")]
fn payload_u64(payload: Option<&serde_json::Value>, key: &str) -> anyhow::Result<u64> {
    payload
        .and_then(|value| value.get(key))
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| anyhow::anyhow!("payload needs u64 `{key}`"))
}

#[cfg(feature = "dev-api")]
fn payload_f64(payload: Option<&serde_json::Value>, key: &str) -> anyhow::Result<f64> {
    payload
        .and_then(|value| value.get(key))
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| anyhow::anyhow!("payload needs f64 `{key}`"))
}

#[cfg(feature = "dev-api")]
fn metadata_field_index(field: &str) -> Option<usize> {
    match field {
        "title" => Some(0),
        "artist" => Some(1),
        "album_artist" => Some(2),
        "year" => Some(3),
        "genre" => Some(4),
        "composer" => Some(5),
        "disc" | "disc_number" => Some(6),
        "track" | "track_number" => Some(7),
        "conductor" => Some(8),
        "performer" => Some(9),
        "isrc" => Some(10),
        "ensemble" => Some(11),
        "edition" => Some(12),
        _ => None,
    }
}

#[cfg(feature = "dev-api")]
fn metadata_candidate_from_payload(
    payload: Option<&serde_json::Value>,
) -> sotf_audio_player::MetadataImportCandidate {
    sotf_audio_player::MetadataImportCandidate {
        provider_id: "musicbrainz".to_string(),
        provider_entity_id: payload
            .and_then(|value| value.get("provider_entity_id"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("scenario-release")
            .to_string(),
        title: payload_string(payload, "title", "Imported Track"),
        artist: payload_string(payload, "artist", "Imported Artist"),
        album_artist: payload_string(payload, "album_artist", "Imported Artist"),
        album_title: payload_string(payload, "album_title", "Imported Album"),
        year: Some(payload_u32(payload, "year", 2024)),
        track_number: Some(payload_u32(payload, "track_number", 1)),
        disc_number: Some(payload_u32(payload, "disc_number", 1)),
        isrc: payload
            .and_then(|value| value.get("isrc"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        score: payload_u8(payload, "score", 96),
    }
}

#[cfg(feature = "dev-api")]
pub(super) fn parse_keystroke(s: &str) -> anyhow::Result<crossterm::event::KeyEvent> {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut modifiers = KeyModifiers::empty();
    let mut code_str = s;

    // Parse modifier prefixes (e.g. "ctrl-a", "shift-up")
    if let Some(rest) = s.strip_prefix("ctrl-") {
        modifiers |= KeyModifiers::CONTROL;
        code_str = rest;
    } else if let Some(rest) = s.strip_prefix("shift-") {
        modifiers |= KeyModifiers::SHIFT;
        code_str = rest;
    } else if let Some(rest) = s.strip_prefix("alt-") {
        modifiers |= KeyModifiers::ALT;
        code_str = rest;
    }

    let code = match code_str {
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
        "space" => KeyCode::Char(' '),
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "delete" | "del" => KeyCode::Delete,
        "backspace" | "bs" => KeyCode::Backspace,
        "insert" => KeyCode::Insert,
        "f1" => KeyCode::F(1),
        "f2" => KeyCode::F(2),
        "f3" => KeyCode::F(3),
        "f4" => KeyCode::F(4),
        "f5" => KeyCode::F(5),
        "f6" => KeyCode::F(6),
        "f7" => KeyCode::F(7),
        "f8" => KeyCode::F(8),
        "f9" => KeyCode::F(9),
        "f10" => KeyCode::F(10),
        "f11" => KeyCode::F(11),
        "f12" => KeyCode::F(12),
        c if c.len() == 1 => KeyCode::Char(c.chars().next().unwrap()),
        _ => return Err(anyhow::anyhow!("unknown keystroke: `{s}`")),
    };

    Ok(KeyEvent::new(code, modifiers))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_audio_player::SignalPath;

    #[test]
    fn signal_path_signature_changes_with_resampling_and_health() {
        let idle_path = SignalPath {
            source: None,
            plugin_chain: Vec::new(),
            processing: sotf_audio_player::SignalPathProcessing {
                resampling: None,
                latency_samples: 0,
                bypassed: false,
            },
            output: sotf_audio_player::SignalPathOutput {
                device: None,
                sample_rate_hz: 48_000,
                channels: 2,
                access_mode: "Shared".to_string(),
                exclusive_active: false,
            },
            health: sotf_audio_player::SignalPathHealth {
                underruns: 0,
                stream_errors: 0,
                frames_dropped: 0,
                clipping_detected: None,
                headroom_db: None,
            },
        };
        let sig_idle = signal_path_redraw_signature(Some(&idle_path));

        let mut resampled_path = idle_path.clone();
        resampled_path.source = Some(sotf_audio_player::SignalPathSource {
            format: "FLAC".to_string(),
            sample_rate_hz: 44_100,
            channels: 2,
            bits_per_sample: 16,
            lossless: true,
        });
        resampled_path.output.sample_rate_hz = 48_000;
        let sig_resampled = signal_path_redraw_signature(Some(&resampled_path));
        assert_ne!(sig_idle, sig_resampled);

        let mut unhealthy_path = resampled_path.clone();
        unhealthy_path.health.underruns = 1;
        let sig_unhealthy = signal_path_redraw_signature(Some(&unhealthy_path));
        assert_ne!(sig_resampled, sig_unhealthy);
    }
}
