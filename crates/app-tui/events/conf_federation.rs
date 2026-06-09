use super::PlayerCommand;
use crate::app::{
    ADD_SOURCE_TYPE_IDX, App, FederationEditState, FederationMode, InputMode, SOURCE_TYPE_NAMES,
};
use crossterm::event::{KeyCode, KeyEvent};
use sotf_audio_player::federation_config::{
    ConnectionStatus, FederationSourceEntry, SourceConnectionConfig,
};

pub(super) fn handle_federation_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match app.federation_state.mode {
        FederationMode::List => handle_list_mode(app, key),
        FederationMode::EditSource => handle_edit_mode(app, key),
        FederationMode::AddSource => handle_add_mode(app, key),
    }
}

fn handle_list_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    let state = &mut app.federation_state;
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Configure;
        }
        KeyCode::Up if !state.sources.is_empty() && state.selected_idx > 0 => {
            state.selected_idx -= 1;
        }
        KeyCode::Down if state.selected_idx + 1 < state.sources.len() => {
            state.selected_idx += 1;
        }
        KeyCode::Char('a') => {
            ADD_SOURCE_TYPE_IDX.store(0, std::sync::atomic::Ordering::Relaxed);
            state.mode = FederationMode::AddSource;
        }
        KeyCode::Char('d') => {
            if let Some(source) = state.sources.get(state.selected_idx) {
                let source_id = source.source_id.clone();
                // Don't allow deleting the local source
                if source_id != "local" {
                    if let Some(db) = app.library.get_database() {
                        let _ = db.delete_federation_source(&source_id);
                    }
                    state.sources.retain(|s| s.source_id != source_id);
                    if state.selected_idx >= state.sources.len() && !state.sources.is_empty() {
                        state.selected_idx = state.sources.len() - 1;
                    }
                }
            }
        }
        KeyCode::Char('e') | KeyCode::Enter => {
            if let Some(source) = state.sources.get(state.selected_idx).cloned()
                && source.source_id != "local"
            {
                state.edit = Some(FederationEditState::new(source, false));
                state.mode = FederationMode::EditSource;
            }
        }
        KeyCode::Char(' ') => {
            if let Some(source) = state.sources.get_mut(state.selected_idx)
                && source.source_id != "local"
            {
                source.is_enabled = !source.is_enabled;
                if let Some(db) = app.library.get_database() {
                    let _ = db.toggle_federation_source(&source.source_id);
                }
            }
        }
        KeyCode::Char('t') => {
            test_federation_source(app);
        }
        KeyCode::Char('s') => {
            scan_federation_source(app);
        }
        _ => {}
    }
    None
}

fn handle_edit_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    let state = &mut app.federation_state;
    let Some(edit) = &mut state.edit else {
        state.mode = FederationMode::List;
        return None;
    };

    if edit.editing_value {
        match key.code {
            KeyCode::Enter => {
                let value = edit.edit_buffer.clone();
                edit.set_field_value(edit.selected_field, &value);
                edit.editing_value = false;
            }
            KeyCode::Esc => {
                edit.editing_value = false;
            }
            KeyCode::Backspace => {
                edit.edit_buffer.pop();
            }
            KeyCode::Char(c) => {
                edit.edit_buffer.push(c);
            }
            _ => {}
        }
        return None;
    }

    match key.code {
        KeyCode::Esc => {
            state.edit = None;
            state.mode = FederationMode::List;
        }
        KeyCode::Up if edit.selected_field > 0 => {
            edit.selected_field -= 1;
        }
        KeyCode::Down if edit.selected_field + 1 < edit.field_count() => {
            edit.selected_field += 1;
        }
        KeyCode::Enter => {
            edit.edit_buffer = edit.field_value(edit.selected_field);
            edit.editing_value = true;
        }
        KeyCode::Char('s') | KeyCode::Tab => {
            // Save
            let source = edit.source.clone();
            let is_new = edit.is_new;
            state.edit = None;
            state.mode = FederationMode::List;

            if let Some(db) = app.library.get_database() {
                let _ = db.save_federation_source(&source);
            }
            if is_new {
                state.sources.push(source);
            } else {
                for s in &mut state.sources {
                    if s.source_id == source.source_id {
                        *s = source.clone();
                        break;
                    }
                }
            }
        }
        _ => {}
    }
    None
}

fn handle_add_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    let state = &mut app.federation_state;
    let idx = ADD_SOURCE_TYPE_IDX.load(std::sync::atomic::Ordering::Relaxed);

    match key.code {
        KeyCode::Esc => {
            state.mode = FederationMode::List;
        }
        KeyCode::Up if idx > 0 => {
            ADD_SOURCE_TYPE_IDX.store(idx - 1, std::sync::atomic::Ordering::Relaxed);
        }
        KeyCode::Down if idx + 1 < SOURCE_TYPE_NAMES.len() => {
            ADD_SOURCE_TYPE_IDX.store(idx + 1, std::sync::atomic::Ordering::Relaxed);
        }
        KeyCode::Enter => {
            let type_name = SOURCE_TYPE_NAMES[idx].0;
            let display_name = SOURCE_TYPE_NAMES[idx].1.to_string();
            let connection = SourceConnectionConfig::default_for_type(type_name);
            let source_id = format!("{}:{}", type_name, uuid_short());
            let source = FederationSourceEntry {
                source_id,
                display_name,
                priority: 50,
                is_enabled: true,
                connection,
                is_available: None,
            };
            state.edit = Some(FederationEditState::new(source, true));
            state.mode = FederationMode::EditSource;
        }
        _ => {}
    }
    None
}

/// Generate a short unique-ish identifier
fn uuid_short() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    format!("{:x}", ts & 0xFFFF_FFFF)
}

fn test_federation_source(app: &mut App) {
    let state = &mut app.federation_state;
    let source_idx = state.selected_idx;
    let source = match state.sources.get(source_idx) {
        Some(s) => s.clone(),
        None => return,
    };

    if source.source_id == "local" {
        return;
    }

    let source_id = source.source_id.clone();
    state
        .statuses
        .insert(source_id.clone(), ConnectionStatus::Testing);

    let (tx, rx) = std::sync::mpsc::channel();
    app.federation_test_receiver = Some(rx);

    std::thread::spawn(move || {
        let status = sotf_audio_player::federation_scan::run_connection_diagnostic(&source);
        let _ = tx.send((source_id, status));
    });
}

fn scan_federation_source(app: &mut App) {
    if app.federation_scan_receiver.is_some() {
        app.status_message = Some("A federation scan is already running.".to_string());
        return;
    }

    let state = &app.federation_state;
    let source = match state.sources.get(state.selected_idx) {
        Some(s) => s.clone(),
        None => return,
    };

    if source.source_id == "local" {
        return;
    }

    app.status_message = Some(format!("Scanning {}...", source.display_name));

    let (tx, rx) = std::sync::mpsc::channel();
    app.federation_scan_receiver = Some(rx);

    std::thread::Builder::new()
        .name("federation-scan".into())
        .spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            let result = rt.block_on(do_federation_scan(&source));
            let _ = tx.send(result);
        })
        .expect("spawn federation scan thread");
}

async fn do_federation_scan(source: &FederationSourceEntry) -> crate::app::FederationScanResult {
    use sotf_audio_player::federation_scan;

    let source_id = source.source_id.clone();
    let cancel = std::sync::atomic::AtomicBool::new(false);

    let albums = match federation_scan::fetch_source_albums(source).await {
        Ok(albums) => albums,
        Err(result) => {
            return crate::app::FederationScanResult {
                source_id: result.source_id,
                albums: result.albums,
                tracks: result.tracks,
                error: result.error,
            };
        }
    };

    let result = federation_scan::merge_albums_to_db(&source_id, &albums, &cancel, None);
    crate::app::FederationScanResult {
        source_id: result.source_id,
        albums: result.albums,
        tracks: result.tracks,
        error: result.error,
    }
}

/// Poll for federation scan completion. Call from the main tick loop.
/// Returns true if the UI needs a redraw.
pub fn poll_federation_scan(app: &mut App) -> bool {
    let result = match &app.federation_scan_receiver {
        Some(rx) => match rx.try_recv() {
            Ok(result) => Some(result),
            Err(std::sync::mpsc::TryRecvError::Empty) => return false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                app.federation_scan_receiver = None;
                return false;
            }
        },
        None => return false,
    };

    app.federation_scan_receiver = None;

    if let Some(result) = result {
        if let Some(ref err) = result.error {
            app.status_message = Some(format!("Federation scan failed: {err}"));
            // Mark source as unavailable
            if let Some(source) = app
                .federation_state
                .sources
                .iter_mut()
                .find(|s| s.source_id == result.source_id)
            {
                source.is_available = Some(false);
            }
            if let Some(db) = app.library.get_database() {
                let _ = db.set_source_availability(&result.source_id, false);
            }
            app.federation_state
                .statuses
                .insert(result.source_id, ConnectionStatus::Error(err.clone()));
        } else {
            app.status_message = Some(format!(
                "Scan complete: {} albums, {} tracks merged.",
                result.albums, result.tracks
            ));
            // Mark source as available
            if let Some(source) = app
                .federation_state
                .sources
                .iter_mut()
                .find(|s| s.source_id == result.source_id)
            {
                source.is_available = Some(true);
            }
            if let Some(db) = app.library.get_database() {
                let _ = db.set_source_availability(&result.source_id, true);
                let _ = db.update_federation_source_sync_time(&result.source_id);
            }
            app.federation_state.statuses.insert(
                result.source_id,
                ConnectionStatus::Connected { version: None },
            );
            // Reload library to pick up newly merged albums
            if let Some(db) = app.library.get_database() {
                match db.load_library() {
                    Ok(albums) => {
                        app.library.albums = albums;
                        app.request_filter_update();
                        app.rebuild_artist_tree();
                    }
                    Err(e) => {
                        log::error!("Failed to reload library after federation scan: {e}");
                    }
                }
            }
        }
    }

    true
}

/// Poll for federation connection test completion. Call from the main tick loop.
/// Returns true if the UI needs a redraw.
pub fn poll_federation_test(app: &mut App) -> bool {
    let rx = match &app.federation_test_receiver {
        Some(rx) => rx,
        None => return false,
    };

    match rx.try_recv() {
        Ok((sid, status)) => {
            app.federation_test_receiver = None;

            let available = match &status {
                ConnectionStatus::Connected { .. } => true,
                ConnectionStatus::Diagnostic(d) => d.is_success(),
                _ => false,
            };

            if let Some(src) = app
                .federation_state
                .sources
                .iter_mut()
                .find(|s| s.source_id == sid)
            {
                src.is_available = Some(available);
            }
            if let Some(db) = app.library.get_database() {
                let _ = db.set_source_availability(&sid, available);
            }

            let should_scan_peer = available
                && app.federation_scan_receiver.is_none()
                && app
                    .federation_state
                    .sources
                    .get(app.federation_state.selected_idx)
                    .is_some_and(|source| {
                        source.source_id == sid
                            && matches!(source.connection, SourceConnectionConfig::Peer { .. })
                    });

            app.federation_state.statuses.insert(sid, status);
            if should_scan_peer {
                scan_federation_source(app);
            }
            true
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => false,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            app.federation_test_receiver = None;
            false
        }
    }
}
