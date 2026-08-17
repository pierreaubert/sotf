use super::PlayerCommand;
#[cfg(any(feature = "tidal", feature = "spotify"))]
use crate::app::ServiceLoginState;
use crate::app::{
    ADD_SOURCE_TYPE_IDX, App, FederationEditState, FederationMode, InputMode, SOURCE_TYPE_NAMES,
    ServiceLoginEvent, ServiceLoginStatus,
};
use crossterm::event::{KeyCode, KeyEvent};
use sotf_audio_player::federation_config::{
    ConnectionStatus, FederationSourceEntry, SourceConnectionConfig,
};
#[cfg(any(feature = "tidal", feature = "spotify"))]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) fn handle_federation_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match app.federation.state.mode {
        FederationMode::List => handle_list_mode(app, key),
        FederationMode::EditSource => handle_edit_mode(app, key),
        FederationMode::AddSource => handle_add_mode(app, key),
    }
}

fn handle_list_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    let state = &mut app.federation.state;
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
        #[cfg(any(feature = "tidal", feature = "spotify"))]
        KeyCode::Char('l') => {
            toggle_service_login(app);
        }
        #[cfg(any(feature = "tidal", feature = "spotify"))]
        KeyCode::Char('L') => {
            service_logout(app);
        }
        _ => {}
    }
    None
}

fn handle_edit_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    let state = &mut app.federation.state;
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
    let state = &mut app.federation.state;
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
    let state = &mut app.federation.state;
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
    app.federation.receivers.test = Some(rx);

    std::thread::spawn(move || {
        let status = sotf_audio_player::federation_scan::run_connection_diagnostic(&source);
        let _ = tx.send((source_id, status));
    });
}

fn scan_federation_source(app: &mut App) {
    if app.federation.receivers.scan.is_some() {
        app.ui.status_message = Some("A federation scan is already running.".to_string());
        return;
    }

    let state = &app.federation.state;
    let source = match state.sources.get(state.selected_idx) {
        Some(s) => s.clone(),
        None => return,
    };

    if source.source_id == "local" {
        return;
    }

    app.ui.status_message = Some(format!("Scanning {}...", source.display_name));

    let (tx, rx) = std::sync::mpsc::channel();
    app.federation.receivers.scan = Some(rx);

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

    let cancel = std::sync::atomic::AtomicBool::new(false);
    let result = federation_scan::sync_federation_source(source, &cancel, None, None).await;
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
    let result = match &app.federation.receivers.scan {
        Some(rx) => match rx.try_recv() {
            Ok(result) => Some(result),
            Err(std::sync::mpsc::TryRecvError::Empty) => return false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                app.federation.receivers.scan = None;
                return false;
            }
        },
        None => return false,
    };

    app.federation.receivers.scan = None;

    if let Some(result) = result {
        if let Some(ref err) = result.error {
            app.ui.status_message = Some(format!("Federation scan failed: {err}"));
            // Mark source as unavailable
            if let Some(source) = app
                .federation
                .state
                .sources
                .iter_mut()
                .find(|s| s.source_id == result.source_id)
            {
                source.is_available = Some(false);
            }
            if let Some(db) = app.library.get_database() {
                let _ = db.set_source_availability(&result.source_id, false);
            }
            app.federation
                .state
                .statuses
                .insert(result.source_id, ConnectionStatus::Error(err.clone()));
        } else {
            app.ui.status_message = Some(format!(
                "Scan complete: {} albums, {} tracks merged.",
                result.albums, result.tracks
            ));
            // Mark source as available
            if let Some(source) = app
                .federation
                .state
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
            app.federation.state.statuses.insert(
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
    let rx = match &app.federation.receivers.test {
        Some(rx) => rx,
        None => return false,
    };

    match rx.try_recv() {
        Ok((sid, status)) => {
            app.federation.receivers.test = None;

            let available = match &status {
                ConnectionStatus::Connected { .. } => true,
                ConnectionStatus::Diagnostic(d) => d.is_success(),
                _ => false,
            };

            if let Some(src) = app
                .federation
                .state
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
                && app.federation.receivers.scan.is_none()
                && app
                    .federation
                    .state
                    .sources
                    .get(app.federation.state.selected_idx)
                    .is_some_and(|source| {
                        source.source_id == sid
                            && matches!(source.connection, SourceConnectionConfig::Peer { .. })
                    });

            app.federation.state.statuses.insert(sid, status);
            if should_scan_peer {
                scan_federation_source(app);
            }
            true
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => false,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            app.federation.receivers.test = None;
            false
        }
    }
}

/// 'l' in the source list: start a login for the selected Tidal/Spotify
/// source, or cancel the in-progress one.
#[cfg(any(feature = "tidal", feature = "spotify"))]
fn toggle_service_login(app: &mut App) {
    if let Some(login) = &app.federation.state.login {
        login.cancel.store(true, Ordering::Relaxed);
        app.federation.state.login = None;
        app.federation.receivers.login = None;
        app.ui.status_message = Some("Login cancelled.".to_string());
        return;
    }

    let Some(source) = app
        .federation
        .state
        .sources
        .get(app.federation.state.selected_idx)
        .cloned()
    else {
        return;
    };
    if source.source_id == "local" {
        return;
    }

    match &source.connection {
        #[cfg(feature = "tidal")]
        SourceConnectionConfig::Tidal { client_id, .. } => {
            start_tidal_login(app, &source.source_id, client_id);
        }
        #[cfg(feature = "spotify")]
        SourceConnectionConfig::Spotify { .. } => {
            start_spotify_login(app, &source.source_id);
        }
        _ => {
            app.ui.status_message =
                Some("Login is only available for Tidal and Spotify sources.".to_string());
        }
    }
}

/// 'L' in the source list: clear the selected source's login state — Tidal
/// tokens on the source config, or the Spotify librespot credential cache.
#[cfg(any(feature = "tidal", feature = "spotify"))]
fn service_logout(app: &mut App) {
    let idx = app.federation.state.selected_idx;
    let Some(source) = app.federation.state.sources.get_mut(idx) else {
        return;
    };
    if source.source_id == "local" {
        return;
    }

    match &mut source.connection {
        #[cfg(feature = "tidal")]
        SourceConnectionConfig::Tidal { .. } => {
            sotf_audio_player::clear_tidal_tokens(source);
            if let Some(db) = app.library.get_database()
                && let Err(e) = db.save_federation_source(source)
            {
                log::error!(
                    "Failed to persist Tidal logout for {}: {e}",
                    source.source_id
                );
                app.ui.status_message = Some(format!("Failed to save source: {e}"));
                return;
            }
            app.ui.status_message = Some(format!(
                "Logged out of Tidal — tokens cleared for '{}'.",
                source.display_name
            ));
            sotf_audio_player::reset_service_sessions();
        }
        #[cfg(feature = "spotify")]
        SourceConnectionConfig::Spotify { .. } => {
            let Some(cache_dir) = sotf_audio_player::spotify_cache_dir() else {
                app.ui.status_message =
                    Some("Could not determine the Spotify credential cache directory.".to_string());
                return;
            };
            app.ui.status_message = Some(
                match sotf_audio_player::clear_spotify_cached_credentials(&cache_dir) {
                    Ok(true) => {
                        sotf_audio_player::reset_service_sessions();
                        "Logged out of Spotify (cached credentials removed).".to_string()
                    }
                    Ok(false) => "Spotify logout: no cached credentials to remove.".to_string(),
                    Err(e) => format!("Spotify logout failed: {e}"),
                },
            );
        }
        _ => {
            app.ui.status_message =
                Some("Logout is only available for Tidal and Spotify sources.".to_string());
        }
    }
}

/// Spawn the Tidal device-code flow: request the prompt, report it to the UI
/// for display, then poll every 5 s until completion, expiry, or cancel.
/// (Tidal's device-auth endpoint answers `slow_down` to aggressive polling.)
#[cfg(feature = "tidal")]
fn start_tidal_login(app: &mut App, source_id: &str, client_id: &str) {
    use sotf_service_tidal::DeviceAuthPoll;

    let (tx, rx) = std::sync::mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_thread = Arc::clone(&cancel);
    let client_id = client_id.trim().to_string();

    app.federation.receivers.login = Some(rx);
    app.federation.state.login = Some(ServiceLoginState {
        source_id: source_id.to_string(),
        status: ServiceLoginStatus::Starting,
        cancel,
    });

    if let Err(e) = std::thread::Builder::new()
        .name("tidal-login".into())
        .spawn(move || {
            let mut service = sotf_service_tidal::TidalService::new();
            if !client_id.is_empty() {
                service = service.with_client_id(&client_id);
            }
            let prompt = match service.begin_device_auth() {
                Ok(prompt) => prompt,
                Err(e) => {
                    let _ = tx.send(ServiceLoginEvent::Failed(e.to_string()));
                    return;
                }
            };
            if tx
                .send(ServiceLoginEvent::TidalPrompt {
                    verification_url: prompt.verification_url,
                    user_code: prompt.user_code,
                    expires_in_secs: prompt.expires_in_secs,
                })
                .is_err()
            {
                return; // UI went away
            }
            loop {
                if cancel_thread.load(Ordering::Relaxed) {
                    let _ = tx.send(ServiceLoginEvent::Cancelled);
                    return;
                }
                std::thread::sleep(std::time::Duration::from_secs(5));
                match service.poll_device_auth() {
                    Ok(DeviceAuthPoll::Pending) => {}
                    Ok(DeviceAuthPoll::Complete) => {
                        let _ = tx.send(ServiceLoginEvent::Complete {
                            tidal_tokens: Some((
                                service.access_token().unwrap_or_default().to_string(),
                                service.refresh_token().unwrap_or_default().to_string(),
                            )),
                        });
                        return;
                    }
                    Ok(DeviceAuthPoll::Expired) => {
                        let _ = tx.send(ServiceLoginEvent::Failed(
                            "device code expired — start the login again".to_string(),
                        ));
                        return;
                    }
                    Err(e) => {
                        let _ = tx.send(ServiceLoginEvent::Failed(e.to_string()));
                        return;
                    }
                }
            }
        })
    {
        app.federation.receivers.login = None;
        app.federation.state.login = None;
        app.ui.status_message = Some(format!("Failed to start Tidal login: {e}"));
    }
}

/// Spawn the Spotify OAuth flow: open the authorize URL in the system
/// browser (also reported to the UI as a fallback), then block on the
/// loopback callback. Credentials are written to the librespot cache by the
/// service itself, so completion carries no payload.
#[cfg(feature = "spotify")]
fn start_spotify_login(app: &mut App, source_id: &str) {
    let Some(cache_dir) = sotf_audio_player::spotify_cache_dir() else {
        app.ui.status_message =
            Some("Could not determine the Spotify credential cache directory.".to_string());
        return;
    };

    let (tx, rx) = std::sync::mpsc::channel();
    let tx_url = tx.clone();

    app.federation.receivers.login = Some(rx);
    app.federation.state.login = Some(ServiceLoginState {
        source_id: source_id.to_string(),
        status: ServiceLoginStatus::Starting,
        // Spotify's blocking loopback listener cannot be interrupted (it has
        // its own 180 s timeout); the flag only detaches the UI.
        cancel: Arc::new(AtomicBool::new(false)),
    });

    if let Err(e) = std::thread::Builder::new()
        .name("spotify-login".into())
        .spawn(move || {
            // `login_with_oauth` needs an ambient tokio runtime
            // (`librespot_core::Session::new` panics without one).
            let result = tokio::runtime::Runtime::new()
                .map_err(|e| e.to_string())
                .and_then(|rt| {
                    rt.block_on(async {
                        let mut service = sotf_service_spotify::SpotifyService::new();
                        service.login_with_oauth(&cache_dir, move |url| {
                            if let Err(e) = sotf_audio_player::open_url_in_browser(url) {
                                log::warn!("[Spotify] could not open the browser: {e}");
                            }
                            let _ = tx_url.send(ServiceLoginEvent::SpotifyUrl {
                                url: url.to_string(),
                            });
                        })
                    })
                    .map_err(|e| e.to_string())
                });
            let event = match result {
                Ok(()) => ServiceLoginEvent::Complete { tidal_tokens: None },
                Err(e) => ServiceLoginEvent::Failed(e),
            };
            let _ = tx.send(event);
        })
    {
        app.federation.receivers.login = None;
        app.federation.state.login = None;
        app.ui.status_message = Some(format!("Failed to start Spotify login: {e}"));
    }
}

/// Poll for streaming-service login progress. Call from the main tick loop.
/// Returns true if the UI needs a redraw.
pub fn poll_service_login(app: &mut App) -> bool {
    let mut events = Vec::new();
    let mut disconnected = false;
    match &app.federation.receivers.login {
        Some(rx) => loop {
            match rx.try_recv() {
                Ok(event) => events.push(event),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        },
        None => return false,
    }

    if events.is_empty() && !disconnected {
        return false;
    }

    for event in events {
        handle_service_login_event(app, event);
    }
    if disconnected {
        app.federation.receivers.login = None;
        // A sender that vanishes without a final event means the thread died
        // (e.g. panicked while building its HTTP client).
        if app.federation.state.login.take().is_some() {
            app.ui.status_message =
                Some("Login failed unexpectedly (background thread terminated).".to_string());
        }
    }
    true
}

fn handle_service_login_event(app: &mut App, event: ServiceLoginEvent) {
    match event {
        ServiceLoginEvent::TidalPrompt {
            verification_url,
            user_code,
            expires_in_secs,
        } => {
            if let Some(login) = &mut app.federation.state.login {
                login.status = ServiceLoginStatus::TidalDevicePrompt {
                    verification_url,
                    user_code,
                    expires_in_secs,
                    started: std::time::Instant::now(),
                };
            }
        }
        ServiceLoginEvent::SpotifyUrl { url } => {
            if let Some(login) = &mut app.federation.state.login {
                login.status = ServiceLoginStatus::SpotifyOAuth {
                    url,
                    started: std::time::Instant::now(),
                };
            }
        }
        ServiceLoginEvent::Complete { tidal_tokens } => {
            let Some(login) = app.federation.state.login.take() else {
                return;
            };
            app.federation.receivers.login = None;
            if let Some((access_token, refresh_token)) = tidal_tokens {
                finish_tidal_login(app, &login.source_id, &access_token, &refresh_token);
            } else {
                sotf_audio_player::reset_service_sessions();
                app.ui.status_message =
                    Some("Spotify login complete — credentials cached.".to_string());
            }
        }
        ServiceLoginEvent::Failed(err) => {
            app.federation.state.login = None;
            app.federation.receivers.login = None;
            app.ui.status_message = Some(format!("Login failed: {err}"));
        }
        ServiceLoginEvent::Cancelled => {
            // The UI already cleared the state and showed the message when
            // the user pressed 'l'; just drop the receiver.
            app.federation.state.login = None;
            app.federation.receivers.login = None;
        }
    }
}

/// Write freshly-issued Tidal tokens into the source and persist them via
/// the usual federation-source save path.
fn finish_tidal_login(app: &mut App, source_id: &str, access_token: &str, refresh_token: &str) {
    let Some(source) = app
        .federation
        .state
        .sources
        .iter_mut()
        .find(|s| s.source_id == source_id)
    else {
        app.ui.status_message = Some(format!("Login failed: source {source_id} no longer exists"));
        return;
    };
    if !sotf_audio_player::apply_tidal_device_tokens(source, access_token, refresh_token) {
        return;
    }
    // Keep an open edit form for the same source in sync, otherwise saving
    // it later would clobber the freshly-persisted tokens with stale ones.
    if let Some(edit) = &mut app.federation.state.edit
        && edit.source.source_id == source_id
    {
        sotf_audio_player::apply_tidal_device_tokens(&mut edit.source, access_token, refresh_token);
    }
    let display_name = source.display_name.clone();
    if let Some(db) = app.library.get_database()
        && let Err(e) = db.save_federation_source(source)
    {
        log::error!("Failed to persist Tidal tokens for {source_id}: {e}");
        app.ui.status_message = Some(format!("Failed to save source: {e}"));
        return;
    }
    sotf_audio_player::reset_service_sessions();
    app.ui.status_message = Some(format!(
        "Tidal login complete — tokens saved to '{display_name}'."
    ));
}
