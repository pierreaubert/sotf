//! Headless server mode for SOTF.
//!
//! When launched with `--server`, the app skips UI and runs MPD/DLNA servers
//! directly, allowing remote clients to browse the library and control playback.

use std::collections::HashSet;
use std::net::Ipv4Addr;
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sotf_dlna::{DlnaDevice, DlnaMediaServer, MediaServerAdapter};
use sotf_mpd::{
    FilterExpr, MpdAuthMode, MpdDirEntry, MpdPlayState, MpdServer, MpdServerConfig, MpdSongInfo,
    MpdStatus, PlayerAdapter,
};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::federation_config::{self, ServerConfig, SotfApiSettings};
use crate::lan_discovery::run_sotf_lan_discovery;
use crate::library::MusicLibrary;
use crate::player::Player;
use crate::queue::Queue;
use crate::sotf_server_event::{EventBroadcaster, SotfServerEvent};

const API_MAX_REQUEST_BYTES: usize = 64 * 1024;
const API_MAX_BODY_BYTES: usize = 32 * 1024;
const API_LIBRARY_DEFAULT_LIMIT: usize = 50;
const API_LIBRARY_MAX_LIMIT: usize = 250;

/// Shared state for the headless server adapters.
struct ServerState {
    player: Mutex<Player>,
    library: Mutex<MusicLibrary>,
    queue: Mutex<Queue>,
    /// Playlist version counter — incremented on every queue mutation.
    playlist_version: std::sync::atomic::AtomicU32,
    /// Library version counter for remote client cache invalidation.
    library_version: std::sync::atomic::AtomicU64,
    /// Broadcast channel for server-sent events.
    events: EventBroadcaster,
    /// Whether pairing mode is currently open.
    pairing_mode: std::sync::atomic::AtomicBool,
    /// Pairing nonce/short code — valid only while pairing_mode is true.
    pairing_nonce: parking_lot::Mutex<String>,
    /// Trusted client certificate store for mTLS.
    trusted_clients: parking_lot::Mutex<sotf_tls::TrustedClientStore>,
    /// Live trusted fingerprints used by the MPD mTLS verifier.
    trusted_client_fingerprints: Arc<std::sync::Mutex<HashSet<String>>>,
    /// Server TLS certificate fingerprint (for QR code / manual verification).
    server_fingerprint: String,
}

impl ServerState {
    /// Broadcast a server event to all connected clients.
    /// Silently ignored if there are no active subscribers.
    fn broadcast(&self, event: SotfServerEvent) {
        let _ = self.events.send(event);
    }

    /// Generate a fresh pairing nonce.
    fn refresh_pairing_nonce(&self) -> String {
        let nonce = generate_pairing_nonce();
        *self.pairing_nonce.lock() = nonce.clone();
        nonce
    }
}

fn generate_pairing_nonce() -> String {
    let bytes: [u8; 16] = rand::random();
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn generate_api_auth_token() -> String {
    let bytes: [u8; 32] = rand::random();
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn ensure_server_mode_api_config(config: &mut ServerConfig) -> bool {
    let mut changed = false;
    if !config.api.enabled {
        config.api.enabled = true;
        changed = true;
    }
    if config
        .api
        .auth_token
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        config.api.auth_token = Some(generate_api_auth_token());
        changed = true;
    }
    changed
}

/// Adapter bridging MPD protocol commands to the SOTF player.
struct MpdPlayerAdapter {
    state: Arc<ServerState>,
}

impl PlayerAdapter for MpdPlayerAdapter {
    fn play(&self, pos: Option<u32>) -> Result<(), String> {
        let mut queue = self.state.queue.lock();
        let mut player = self.state.player.lock();

        let source = if let Some(pos) = pos {
            queue.jump_to(pos as usize)
        } else if queue.current_index.is_some() {
            // Resume current
            let result = player.resume().map_err(|e| e.to_string());
            drop(player);
            drop(queue);
            if result.is_ok() {
                self.state.broadcast(SotfServerEvent::PlaybackChanged);
            }
            return result;
        } else {
            queue.start()
        };

        let result = match source {
            Some(source) => player
                .load_and_play_source(source, vec![], 2, None)
                .map_err(|e| e.to_string()),
            None => Err("No track to play".to_string()),
        };
        drop(player);
        drop(queue);
        if result.is_ok() {
            self.state.broadcast(SotfServerEvent::PlaybackChanged);
        }
        result
    }

    fn pause(&self, state: Option<bool>) -> Result<(), String> {
        let player = self.state.player.lock();
        let result = match state {
            Some(true) | None => player.pause().map_err(|e| e.to_string()),
            Some(false) => player.resume().map_err(|e| e.to_string()),
        };
        drop(player);
        if result.is_ok() {
            self.state.broadcast(SotfServerEvent::PlaybackChanged);
        }
        result
    }

    fn stop(&self) -> Result<(), String> {
        let mut player = self.state.player.lock();
        let result = player.stop().map_err(|e| e.to_string());
        drop(player);
        if result.is_ok() {
            self.state.broadcast(SotfServerEvent::PlaybackChanged);
        }
        result
    }

    fn next(&self) -> Result<(), String> {
        let mut queue = self.state.queue.lock();
        let mut player = self.state.player.lock();

        let result = match queue.next_track() {
            Some(source) => player
                .load_and_play_source(source, vec![], 2, None)
                .map_err(|e| e.to_string()),
            None => {
                player.stop().map_err(|e| e.to_string())?;
                Ok(())
            }
        };
        drop(player);
        drop(queue);
        if result.is_ok() {
            self.state.broadcast(SotfServerEvent::PlaybackChanged);
        }
        result
    }

    fn previous(&self) -> Result<(), String> {
        let mut queue = self.state.queue.lock();
        let mut player = self.state.player.lock();

        let result = match queue.previous_track() {
            Some(source) => player
                .load_and_play_source(source, vec![], 2, None)
                .map_err(|e| e.to_string()),
            None => Err("No previous track".to_string()),
        };
        drop(player);
        drop(queue);
        if result.is_ok() {
            self.state.broadcast(SotfServerEvent::PlaybackChanged);
        }
        result
    }

    fn seek_pos(&self, _song_pos: u32, time: f64) -> Result<(), String> {
        let player = self.state.player.lock();
        let result = player.seek(time).map_err(|e| e.to_string());
        drop(player);
        if result.is_ok() {
            self.state.broadcast(SotfServerEvent::PlaybackChanged);
        }
        result
    }

    fn seek_cur(&self, time: f64) -> Result<(), String> {
        let player = self.state.player.lock();
        let current = player.get_position();
        let result = player.seek(current + time).map_err(|e| e.to_string());
        drop(player);
        if result.is_ok() {
            self.state.broadcast(SotfServerEvent::PlaybackChanged);
        }
        result
    }

    fn set_volume(&self, volume: u8) -> Result<(), String> {
        let player = self.state.player.lock();
        let vol_f32 = f32::from(volume) / 100.0;
        let result = player.set_volume(vol_f32).map_err(|e| e.to_string());
        drop(player);
        if result.is_ok() {
            self.state
                .broadcast(SotfServerEvent::VolumeChanged { volume });
        }
        result
    }

    fn volume_change(&self, delta: i8) -> Result<(), String> {
        let player = self.state.player.lock();
        let current = (player.get_volume() * 100.0) as i16;
        let new = (current + i16::from(delta)).clamp(0, 100);
        let result = player
            .set_volume(new as f32 / 100.0)
            .map_err(|e| e.to_string());
        drop(player);
        if result.is_ok() {
            self.state
                .broadcast(SotfServerEvent::VolumeChanged { volume: new as u8 });
        }
        result
    }

    fn status(&self) -> MpdStatus {
        let mut player = self.state.player.lock();
        let queue = self.state.queue.lock();
        let playback = player.get_playback_state();

        let state = if playback.is_playing {
            MpdPlayState::Play
        } else if queue.current_index.is_some() {
            MpdPlayState::Pause
        } else {
            MpdPlayState::Stop
        };

        let song = queue.current_index.map(|i| i as u32);

        // Compute total track count across all albums in queue
        let playlist_length: u32 = queue
            .items
            .iter()
            .map(|item| item.album.tracks.len() as u32)
            .sum();

        let duration = queue
            .current_track()
            .and_then(|t| t.duration_secs)
            .unwrap_or(0) as f64;

        MpdStatus {
            volume: (player.get_volume() * 100.0) as u8,
            repeat: false,
            random: false,
            single: false,
            consume: false,
            state,
            song,
            songid: song,
            elapsed: playback.position_secs,
            duration,
            audio: playback.sample_rate.map(|sr| format!("{}:16:2", sr)),
            playlist_length,
            playlist_version: self
                .state
                .playlist_version
                .load(std::sync::atomic::Ordering::Relaxed),
        }
    }

    fn current_song(&self) -> Option<MpdSongInfo> {
        let queue = self.state.queue.lock();
        let track = queue.current_track()?;
        let pos = queue.current_index.unwrap_or(0) as u32;
        Some(track_to_song_info(track, pos))
    }

    fn playlist_info(&self, range: Option<(u32, Option<u32>)>) -> Vec<MpdSongInfo> {
        let queue = self.state.queue.lock();
        let all_tracks = flatten_queue_tracks(&queue);

        let (start, end) = match range {
            Some((s, e)) => (s as usize, e.map_or(all_tracks.len(), |e| e as usize)),
            None => (0, all_tracks.len()),
        };

        all_tracks
            .into_iter()
            .skip(start)
            .take(end.saturating_sub(start))
            .collect()
    }

    fn playlist_song_by_id(&self, id: u32) -> Option<MpdSongInfo> {
        let queue = self.state.queue.lock();
        let all_tracks = flatten_queue_tracks(&queue);
        all_tracks.into_iter().find(|s| s.id == id)
    }

    fn add(&self, uri: &str) -> Result<(), String> {
        let library = self.state.library.lock();
        // Try to find an album matching the URI
        if let Some(album) = library.albums.iter().find(|a| a.title == uri) {
            let mut queue = self.state.queue.lock();
            queue.add(album.clone());
            let version = self
                .state
                .playlist_version
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                + 1;
            drop(queue);
            drop(library);
            self.state.broadcast(SotfServerEvent::QueueChanged {
                playlist_version: version,
            });
            Ok(())
        } else {
            Err(format!("Not found: {}", uri))
        }
    }

    fn add_id(&self, uri: &str, _pos: Option<u32>) -> Result<u32, String> {
        self.add(uri)?;
        let queue = self.state.queue.lock();
        Ok(queue.items.len().saturating_sub(1) as u32)
    }

    fn delete(&self, pos: u32) -> Result<(), String> {
        let mut queue = self.state.queue.lock();
        if queue.remove(pos as usize) {
            let version = self
                .state
                .playlist_version
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                + 1;
            drop(queue);
            self.state.broadcast(SotfServerEvent::QueueChanged {
                playlist_version: version,
            });
            Ok(())
        } else {
            Err(format!("Invalid position: {}", pos))
        }
    }

    fn clear(&self) -> Result<(), String> {
        let mut queue = self.state.queue.lock();
        queue.clear();
        let version = self
            .state
            .playlist_version
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        drop(queue);
        self.state.broadcast(SotfServerEvent::QueueChanged {
            playlist_version: version,
        });
        Ok(())
    }

    fn search(&self, filters: &[FilterExpr], _exact: bool) -> Vec<MpdSongInfo> {
        let library = self.state.library.lock();
        let mut results = Vec::new();
        let mut pos = 0u32;

        for album in &library.albums {
            for track in &album.tracks {
                let matches = filters.iter().all(|f| {
                    let tag = f.tag.to_lowercase();
                    let val = f.value.to_lowercase();
                    match tag.as_str() {
                        "artist" => track
                            .artist
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&val),
                        "album" => album.title.to_lowercase().contains(&val),
                        "title" => track
                            .title
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&val),
                        "genre" => track
                            .genre
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&val),
                        "any" => {
                            track
                                .title
                                .as_deref()
                                .unwrap_or("")
                                .to_lowercase()
                                .contains(&val)
                                || track
                                    .artist
                                    .as_deref()
                                    .unwrap_or("")
                                    .to_lowercase()
                                    .contains(&val)
                                || album.title.to_lowercase().contains(&val)
                        }
                        _ => true,
                    }
                });

                if matches {
                    let mut info = track_to_song_info(track, pos);
                    info.album = Some(album.title.clone());
                    results.push(info);
                }
                pos += 1;
            }
        }
        results
    }

    fn list_tag(&self, tag: &str, _filters: &[FilterExpr]) -> Vec<String> {
        let library = self.state.library.lock();
        let tag_lower = tag.to_lowercase();
        let mut values: Vec<String> = Vec::new();

        for album in &library.albums {
            match tag_lower.as_str() {
                "album" if !values.contains(&album.title) => {
                    values.push(album.title.clone());
                }
                "artist" | "albumartist" => {
                    let artist = album.artist();
                    if !values.contains(&artist) {
                        values.push(artist);
                    }
                }
                "genre" => {
                    for track in &album.tracks {
                        if let Some(ref g) = track.genre {
                            if !values.contains(g) {
                                values.push(g.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        values
    }

    fn lsinfo(&self, path: Option<&str>) -> Vec<MpdDirEntry> {
        let library = self.state.library.lock();
        match path {
            None | Some("") | Some("/") => library
                .albums
                .iter()
                .map(|a| MpdDirEntry {
                    is_directory: true,
                    path: a.title.clone(),
                })
                .collect(),
            Some(album_name) => {
                if let Some(album) = library.albums.iter().find(|a| a.title == album_name) {
                    album
                        .tracks
                        .iter()
                        .map(|t| MpdDirEntry {
                            is_directory: false,
                            path: t.path.display().to_string(),
                        })
                        .collect()
                } else {
                    vec![]
                }
            }
        }
    }
}

/// Adapter bridging DLNA ContentDirectory requests to the SOTF library.
struct DlnaLibraryAdapter {
    state: Arc<ServerState>,
}

impl MediaServerAdapter for DlnaLibraryAdapter {
    fn browse_albums(&self, start: u32, count: u32) -> (Vec<sotf_dlna::MediaAlbum>, u32) {
        let library = self.state.library.lock();
        let total = library.albums.len() as u32;
        let albums: Vec<sotf_dlna::MediaAlbum> = library
            .albums
            .iter()
            .skip(start as usize)
            .take(if count == 0 {
                library.albums.len()
            } else {
                count as usize
            })
            .map(|a| sotf_dlna::MediaAlbum {
                id: a.id.map_or_else(|| a.title.clone(), |id| id.to_string()),
                title: a.title.clone(),
                artist: a.artist(),
                year: a.year,
                track_count: a.tracks.len() as u32,
            })
            .collect();
        (albums, total)
    }

    fn browse_album_tracks(&self, album_id: &str) -> Vec<sotf_dlna::MediaTrack> {
        let library = self.state.library.lock();
        let album = library
            .albums
            .iter()
            .find(|a| a.id.is_some_and(|id| id.to_string() == album_id) || a.title == album_id);

        match album {
            Some(album) => album
                .tracks
                .iter()
                .enumerate()
                .map(|(i, t)| track_to_media_track(t, album, i))
                .collect(),
            None => vec![],
        }
    }

    fn search_tracks(
        &self,
        query: &str,
        start: u32,
        count: u32,
    ) -> (Vec<sotf_dlna::MediaTrack>, u32) {
        let library = self.state.library.lock();
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for album in &library.albums {
            for (i, track) in album.tracks.iter().enumerate() {
                let matches = track
                    .title
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&query_lower)
                    || track
                        .artist
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query_lower)
                    || album.title.to_lowercase().contains(&query_lower);

                if matches {
                    results.push(track_to_media_track(track, album, i));
                }
            }
        }

        let total = results.len() as u32;
        let page: Vec<_> = results
            .into_iter()
            .skip(start as usize)
            .take(if count == 0 {
                usize::MAX
            } else {
                count as usize
            })
            .collect();
        (page, total)
    }

    fn album_count(&self) -> u32 {
        let library = self.state.library.lock();
        library.albums.len() as u32
    }

    fn media_path(&self, track_id: &str) -> Option<sotf_dlna::MediaSource> {
        let library = self.state.library.lock();
        for album in &library.albums {
            for (i, track) in album.tracks.iter().enumerate() {
                if media_track_id(track, album, i) == track_id {
                    return Some(sotf_dlna::MediaSource {
                        path: track.path.clone(),
                        mime_type: mime_type_for_path(&track.path).to_string(),
                    });
                }
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// SOTF LAN control API
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct ApiRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

async fn run_sotf_api_server(
    settings: SotfApiSettings,
    state: Arc<ServerState>,
    listener: TcpListener,
    mut cancel: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    let auth_token = validate_sotf_api_token(&settings)?;

    loop {
        tokio::select! {
            changed = cancel.changed() => {
                if changed.is_err() || *cancel.borrow() {
                    break;
                }
            }
            accepted = listener.accept() => {
                let (stream, _) = accepted.map_err(|e| format!("accept: {e}"))?;
                let state = Arc::clone(&state);
                let settings = settings.clone();
                let auth_token = auth_token.clone();
                tokio::spawn(async move {
                    handle_sotf_api_connection(stream, state, settings, auth_token).await;
                });
            }
        }
    }

    Ok(())
}

async fn handle_sotf_api_connection(
    mut stream: TcpStream,
    state: Arc<ServerState>,
    settings: SotfApiSettings,
    auth_token: String,
) {
    match read_api_request(&mut stream).await {
        Ok(request) => {
            if let Err(err) =
                write_sotf_api_response(&mut stream, request, &state, &settings, &auth_token).await
            {
                let response = api_error_response(500, &err);
                let _ = stream.write_all(&response).await;
            }
        }
        Err(err) => {
            let response = api_error_response(400, &err);
            let _ = stream.write_all(&response).await;
        }
    }

    let _ = stream.shutdown().await;
}

async fn write_sotf_api_response(
    stream: &mut TcpStream,
    request: ApiRequest,
    state: &Arc<ServerState>,
    settings: &SotfApiSettings,
    auth_token: &str,
) -> Result<(), String> {
    let route = request.path.split('?').next().unwrap_or(&request.path);
    if route.starts_with("/api/v1/media/") {
        if request.method != "GET" && request.method != "HEAD" {
            let response = api_error_response(405, "method not allowed");
            return stream
                .write_all(&response)
                .await
                .map_err(|err| err.to_string());
        }
        if !api_auth_valid(&request.headers, auth_token) {
            let response = api_error_response(401, "missing or invalid bearer token");
            return stream
                .write_all(&response)
                .await
                .map_err(|err| err.to_string());
        }
        let range = api_header(&request.headers, "range");
        return stream_api_media(stream, &request.method, route, range, state).await;
    }

    // Long-lived SSE stream for /api/v1/events
    if route == "/api/v1/events" && request.method == "GET" {
        if !api_auth_valid(&request.headers, auth_token) {
            let response = api_error_response(401, "missing or invalid bearer token");
            return stream
                .write_all(&response)
                .await
                .map_err(|err| err.to_string());
        }
        return stream_api_events(stream, state).await;
    }

    let response = handle_sotf_api_request(request, state, settings, auth_token);
    stream
        .write_all(&response)
        .await
        .map_err(|err| err.to_string())
}

async fn read_api_request(stream: &mut TcpStream) -> Result<ApiRequest, String> {
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 2048];

    loop {
        if buf.len() > API_MAX_REQUEST_BYTES {
            return Err("request too large".to_string());
        }

        if let Some(header_end) = find_header_end(&buf) {
            let content_length = api_content_length_from_headers(&buf[..header_end])?;
            if content_length > API_MAX_BODY_BYTES {
                return Err("request body too large".to_string());
            }
            let required = header_end + content_length;
            while buf.len() < required {
                let read = stream
                    .read(&mut chunk)
                    .await
                    .map_err(|e| format!("read request body: {e}"))?;
                if read == 0 {
                    return Err("connection closed before request body completed".to_string());
                }
                buf.extend_from_slice(&chunk[..read]);
                if buf.len() > API_MAX_REQUEST_BYTES {
                    return Err("request too large".to_string());
                }
            }
            return parse_api_request(&buf[..required], header_end);
        }

        let read = stream
            .read(&mut chunk)
            .await
            .map_err(|e| format!("read request: {e}"))?;
        if read == 0 {
            return Err("connection closed before request headers completed".to_string());
        }
        buf.extend_from_slice(&chunk[..read]);
    }
}

fn parse_api_request(buf: &[u8], header_end: usize) -> Result<ApiRequest, String> {
    let header_text = std::str::from_utf8(&buf[..header_end])
        .map_err(|_| "request headers are not valid UTF-8".to_string())?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "missing request method".to_string())?;
    let path = parts
        .next()
        .ok_or_else(|| "missing request path".to_string())?;
    let version = parts
        .next()
        .ok_or_else(|| "missing HTTP version".to_string())?;

    if !version.starts_with("HTTP/1.") {
        return Err("unsupported HTTP version".to_string());
    }

    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| "malformed request header".to_string())?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
    }

    let content_length = api_content_length(&headers)?;
    let body_start = header_end;
    let body_end = body_start + content_length;
    Ok(ApiRequest {
        method: method.to_ascii_uppercase(),
        path: path.to_string(),
        headers,
        body: buf[body_start..body_end].to_vec(),
    })
}

fn handle_sotf_api_request(
    request: ApiRequest,
    state: &Arc<ServerState>,
    settings: &SotfApiSettings,
    auth_token: &str,
) -> Vec<u8> {
    let route = request.path.split('?').next().unwrap_or(&request.path);

    match (request.method.as_str(), route) {
        ("GET", "/api/v1/health") => {
            return api_json_response(
                200,
                json!({
                    "ok": true,
                    "service": "sotf",
                    "version": env!("CARGO_PKG_VERSION"),
                    "auth_required": true,
                }),
            );
        }
        ("GET", "/api/v1/discovery") => {
            return api_json_response(
                200,
                json!({
                    "service": "sotf",
                    "version": env!("CARGO_PKG_VERSION"),
                    "friendly_name": settings.friendly_name.clone(),
                    "api_version": 1,
                    "base_path": "/api/v1",
                    "auth": "bearer",
                    "auth_required": true,
                }),
            );
        }
        ("GET", "/api/v1/capabilities") => {
            return api_json_response(200, api_capabilities_json());
        }
        ("GET", "/api/v1/pairing/status") => {
            let pairing_enabled = state
                .pairing_mode
                .load(std::sync::atomic::Ordering::Relaxed);
            return api_json_response(
                200,
                json!({
                    "pairing_enabled": pairing_enabled,
                    "nonce": None::<String>,
                    "server_fingerprint": state.server_fingerprint.clone(),
                }),
            );
        }
        ("POST", "/api/v1/pairing/complete") => {
            return match api_pairing_complete(state, &request) {
                Ok(body) => api_json_response(200, body),
                Err(err) => api_error_response(400, &err),
            };
        }
        _ => {}
    }

    if !api_auth_valid(&request.headers, auth_token) {
        return api_error_response(401, "missing or invalid bearer token");
    }

    let adapter = MpdPlayerAdapter {
        state: Arc::clone(state),
    };

    match (request.method.as_str(), route) {
        ("POST", "/api/v1/pairing/enable") => {
            state
                .pairing_mode
                .store(true, std::sync::atomic::Ordering::Relaxed);
            let nonce = state.refresh_pairing_nonce();
            api_json_response(
                200,
                json!({ "ok": true, "pairing_enabled": true, "nonce": nonce }),
            )
        }
        ("POST", "/api/v1/pairing/disable") => {
            state
                .pairing_mode
                .store(false, std::sync::atomic::Ordering::Relaxed);
            *state.pairing_nonce.lock() = String::new();
            api_json_response(
                200,
                json!({ "ok": true, "pairing_enabled": false, "nonce": null }),
            )
        }
        ("GET", "/api/v1/pairing/clients") => {
            let clients: Vec<_> = state
                .trusted_clients
                .lock()
                .list()
                .into_iter()
                .map(|c| {
                    json!({
                        "fingerprint": c.fingerprint,
                        "name": c.name,
                        "paired_at": c.paired_at,
                    })
                })
                .collect();
            api_json_response(200, json!({ "clients": clients }))
        }
        ("DELETE", _) if route.starts_with("/api/v1/pairing/clients/") => {
            let fp = route.strip_prefix("/api/v1/pairing/clients/").unwrap_or("");
            if fp.is_empty() {
                return api_error_response(400, "fingerprint required");
            }
            let Ok(fingerprint) = normalize_certificate_fingerprint(fp) else {
                return api_error_response(400, "invalid fingerprint format");
            };
            match state.trusted_clients.lock().remove(&fingerprint) {
                Ok(true) => match remove_live_trusted_client(state, &fingerprint) {
                    Ok(()) => api_json_response(200, json!({ "ok": true })),
                    Err(err) => api_error_response(500, &err),
                },
                Ok(false) => api_error_response(404, "client not found"),
                Err(err) => api_error_response(500, &err),
            }
        }
        ("GET", "/api/v1/state") => api_json_response(200, api_state_json(state, &adapter)),
        ("GET", "/api/v1/queue") => api_json_response(200, api_queue_json(&adapter)),
        ("GET", "/api/v1/library/albums") => match api_library_albums_json(state, &request.path) {
            Ok(body) => api_json_response(200, body),
            Err(err) => api_error_response(400, &err),
        },
        ("POST", "/api/v1/play") => api_command_response("play", adapter.play(None)),
        ("POST", "/api/v1/pause") => api_command_response("pause", adapter.pause(Some(true))),
        ("POST", "/api/v1/resume") => api_command_response("resume", adapter.pause(Some(false))),
        ("POST", "/api/v1/stop") => api_command_response("stop", adapter.stop()),
        ("POST", "/api/v1/next") => api_command_response("next", adapter.next()),
        ("POST", "/api/v1/previous") => api_command_response("previous", adapter.previous()),
        ("POST", "/api/v1/queue/add-album") => match api_add_album_to_queue(state, &request) {
            Ok(body) => api_json_response(200, body),
            Err(err) => api_error_response(400, &err),
        },
        ("POST", "/api/v1/queue/clear") => match api_clear_queue(state) {
            Ok(body) => api_json_response(200, body),
            Err(err) => api_error_response(400, &err),
        },
        ("POST", "/api/v1/queue/delete") => match api_delete_queue_album(state, &request) {
            Ok(body) => api_json_response(200, body),
            Err(err) => api_error_response(400, &err),
        },
        ("POST", "/api/v1/queue/jump") => match api_jump_queue_album(state, &request) {
            Ok(body) => api_json_response(200, body),
            Err(err) => api_error_response(400, &err),
        },
        ("POST", "/api/v1/seek") => match api_json_body(&request).and_then(|body| {
            let position = body
                .get("position_secs")
                .and_then(Value::as_f64)
                .ok_or_else(|| "position_secs is required".to_string())?;
            if !position.is_finite() || position < 0.0 {
                return Err("position_secs must be a non-negative finite number".to_string());
            }
            let player = state.player.lock();
            player.seek(position).map_err(|e| e.to_string())
        }) {
            Ok(()) => {
                state.broadcast(SotfServerEvent::PlaybackChanged);
                api_json_response(200, json!({ "ok": true, "command": "seek" }))
            }
            Err(err) => api_error_response(400, &err),
        },
        ("POST", "/api/v1/volume") => match api_json_body(&request).and_then(|body| {
            let volume = body
                .get("volume")
                .and_then(Value::as_u64)
                .ok_or_else(|| "volume is required".to_string())?;
            if volume > 100 {
                return Err("volume must be between 0 and 100".to_string());
            }
            adapter.set_volume(volume as u8)
        }) {
            Ok(()) => api_json_response(200, json!({ "ok": true, "command": "volume" })),
            Err(err) => api_error_response(400, &err),
        },
        ("GET", _) if route.starts_with("/api/v1/library/albums/") => match route
            .strip_prefix("/api/v1/library/albums/")
            .and_then(|tail| tail.split('/').nth(1))
        {
            Some("artwork") => match api_library_album_artwork_response(state, route) {
                Ok(response) => response,
                Err(err) => api_error_response(404, &err),
            },
            _ => match api_library_album_tracks_json(state, route) {
                Ok(body) => api_json_response(200, body),
                Err(err) => api_error_response(404, &err),
            },
        },
        _ if route.starts_with("/api/v1/") => api_error_response(404, "unknown API route"),
        _ => api_error_response(404, "not found"),
    }
}

struct ApiMediaSource {
    path: std::path::PathBuf,
    mime_type: String,
}

async fn stream_api_media(
    stream: &mut TcpStream,
    method: &str,
    route: &str,
    range_header: Option<&str>,
    state: &Arc<ServerState>,
) -> Result<(), String> {
    let track_id = route
        .strip_prefix("/api/v1/media/")
        .ok_or_else(|| "invalid media route".to_string())?;
    if track_id.is_empty() || track_id.contains('/') || track_id.contains("..") {
        let response = api_error_response(400, "bad media id");
        return stream
            .write_all(&response)
            .await
            .map_err(|err| err.to_string());
    }

    let Some(source) = api_media_source(state, track_id) else {
        let response = api_error_response(404, "media track not found");
        return stream
            .write_all(&response)
            .await
            .map_err(|err| err.to_string());
    };

    let metadata = match tokio::fs::metadata(&source.path).await {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => {
            let response = api_error_response(404, "media track not found");
            return stream
                .write_all(&response)
                .await
                .map_err(|err| err.to_string());
        }
        Err(err) => {
            log::warn!(
                "[server] API media metadata error for {:?}: {}",
                source.path,
                err
            );
            let response = api_error_response(404, "media track not found");
            return stream
                .write_all(&response)
                .await
                .map_err(|err| err.to_string());
        }
    };

    let file_len = metadata.len();
    let range = match api_parse_range_header(range_header, file_len) {
        Ok(range) => range,
        Err(()) => {
            let header = format!(
                "HTTP/1.1 416 Range Not Satisfiable\r\n\
                 Content-Range: bytes */{}\r\n\
                 Content-Length: 0\r\n\
                 Accept-Ranges: bytes\r\n\
                 Cache-Control: no-store\r\n\
                 Connection: close\r\n\
                 \r\n",
                file_len
            );
            return stream
                .write_all(header.as_bytes())
                .await
                .map_err(|err| err.to_string());
        }
    };

    let (status, status_text, start, end) = match range {
        Some((start, end)) => (206, "Partial Content", start, end),
        None if file_len == 0 => (200, "OK", 0, 0),
        None => (200, "OK", 0, file_len - 1),
    };
    let body_len = if file_len == 0 { 0 } else { end - start + 1 };
    let content_range = if status == 206 {
        format!("Content-Range: bytes {}-{}/{}\r\n", start, end, file_len)
    } else {
        String::new()
    };
    let header = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Accept-Ranges: bytes\r\n\
         Cache-Control: no-store\r\n\
         {}\
         Connection: close\r\n\
         \r\n",
        status, status_text, source.mime_type, body_len, content_range
    );
    stream
        .write_all(header.as_bytes())
        .await
        .map_err(|err| err.to_string())?;

    if method == "GET" && body_len > 0 {
        stream_api_media_file(stream, &source.path, start, body_len).await?;
    }
    Ok(())
}

async fn stream_api_media_file(
    stream: &mut TcpStream,
    path: &std::path::Path,
    start: u64,
    len: u64,
) -> Result<(), String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|err| format!("open media file: {err}"))?;
    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(|err| format!("seek media file: {err}"))?;

    let mut remaining = len;
    let mut buf = vec![0u8; 64 * 1024];
    while remaining > 0 {
        let to_read = remaining.min(buf.len() as u64) as usize;
        let n = file
            .read(&mut buf[..to_read])
            .await
            .map_err(|err| format!("read media file: {err}"))?;
        if n == 0 {
            break;
        }
        stream
            .write_all(&buf[..n])
            .await
            .map_err(|err| err.to_string())?;
        remaining -= n as u64;
    }
    Ok(())
}

/// Stream server-sent events (SSE) for live playback and queue updates.
///
/// Sends an initial state snapshot, then subscribes to the broadcast channel
/// and forwards each event as an SSE frame until the client disconnects.
async fn stream_api_events(stream: &mut TcpStream, state: &Arc<ServerState>) -> Result<(), String> {
    let adapter = MpdPlayerAdapter {
        state: Arc::clone(state),
    };

    // Subscribe to events BEFORE sending the snapshot so we don't miss
    // any events that fire while the snapshot is being serialized.
    let mut rx = state.events.subscribe();

    let snapshot = api_state_json(state, &adapter);
    let snapshot_body = format!("event: state\ndata: {}\n\n", snapshot);
    let headers = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/event-stream\r\n\
         Cache-Control: no-cache\r\n\
         Connection: keep-alive\r\n\
         \r\n{}",
        snapshot_body
    );
    stream
        .write_all(headers.as_bytes())
        .await
        .map_err(|err| err.to_string())?;

    // Keep-alive ping interval
    let mut ping_interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

    loop {
        tokio::select! {
            _ = ping_interval.tick() => {
                if stream.write_all(b"event: ping\ndata: {}\n\n").await.is_err() {
                    break;
                }
            }
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        let payload = event.to_json();
                        let frame = format!("event: {}\ndata: {}\n\n",
                            sse_event_name(&event), payload);
                        if stream.write_all(frame.as_bytes()).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // Client fell behind; send a full state refresh so they
                        // can catch up without missing critical mutations.
                        let refresh = api_state_json(state, &adapter);
                        let frame = format!("event: state\ndata: {}\n\n", refresh);
                        if stream.write_all(frame.as_bytes()).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

fn sse_event_name(event: &SotfServerEvent) -> &'static str {
    match event {
        SotfServerEvent::PlaybackChanged => "playback_changed",
        SotfServerEvent::QueueChanged { .. } => "queue_changed",
        SotfServerEvent::VolumeChanged { .. } => "volume_changed",
        SotfServerEvent::StreamMetadataChanged { .. } => "stream_metadata_changed",
        SotfServerEvent::ScannerProgress { .. } => "scanner_progress",
        SotfServerEvent::LibraryChanged { .. } => "library_changed",
        SotfServerEvent::Error { .. } => "error",
    }
}

fn api_media_source(state: &Arc<ServerState>, track_id: &str) -> Option<ApiMediaSource> {
    let library = state.library.lock();
    for album in &library.albums {
        for (index, track) in album.tracks.iter().enumerate() {
            if api_track_id(track, album, index) == track_id
                || media_track_id(track, album, index) == track_id
            {
                return Some(ApiMediaSource {
                    path: track.path.clone(),
                    mime_type: mime_type_for_path(&track.path).to_string(),
                });
            }
        }
    }
    None
}

fn api_parse_range_header(
    range_header: Option<&str>,
    file_len: u64,
) -> Result<Option<(u64, u64)>, ()> {
    let Some(raw) = range_header else {
        return Ok(None);
    };
    let Some(spec) = raw.trim().strip_prefix("bytes=") else {
        return Err(());
    };
    if spec.contains(',') || file_len == 0 {
        return Err(());
    }
    let Some((start_raw, end_raw)) = spec.split_once('-') else {
        return Err(());
    };

    let (start, end) = if start_raw.is_empty() {
        let suffix_len = end_raw.parse::<u64>().map_err(|_| ())?;
        if suffix_len == 0 {
            return Err(());
        }
        (file_len.saturating_sub(suffix_len), file_len - 1)
    } else {
        let start = start_raw.parse::<u64>().map_err(|_| ())?;
        if start >= file_len {
            return Err(());
        }
        let end = if end_raw.is_empty() {
            file_len - 1
        } else {
            end_raw.parse::<u64>().map_err(|_| ())?.min(file_len - 1)
        };
        if end < start {
            return Err(());
        }
        (start, end)
    };

    Ok(Some((start, end)))
}

fn api_capabilities_json() -> Value {
    json!({
        "api_version": 1,
        "features": {
            "playback_control": true,
            "queue_editing": true,
            "library_browse": true,
            "library_search": true,
            "media_range": true,
            "events": true,
            "outputs": false,
            "plugin_presets": false,
            "room_eq": false,
            "headphone_eq": false,
            "pairing": false,
        },
        "endpoints": {
            "health": "/api/v1/health",
            "discovery": "/api/v1/discovery",
            "capabilities": "/api/v1/capabilities",
            "state": "/api/v1/state",
            "events": "/api/v1/events",
            "queue": "/api/v1/queue",
            "library_albums": "/api/v1/library/albums",
            "media": "/api/v1/media/{track_id}",
        },
    })
}

fn api_state_json(state: &Arc<ServerState>, adapter: &MpdPlayerAdapter) -> Value {
    let status = adapter.status();
    let current_song = adapter.current_song().map(|song| mpd_song_json(&song));
    let stream_metadata = {
        let player = state.player.lock();
        player.get_engine_state().stream_metadata
    };
    let (album_count, track_count) = {
        let library = state.library.lock();
        let track_count = library
            .albums
            .iter()
            .map(|album| album.tracks.len())
            .sum::<usize>();
        (library.albums.len(), track_count)
    };
    let library_version = state
        .library_version
        .load(std::sync::atomic::Ordering::Relaxed);

    json!({
        "playback": {
            "state": mpd_state_name(&status.state),
            "position_secs": status.elapsed,
            "duration_secs": status.duration,
            "volume": status.volume,
            "current_index": status.song,
            "playlist_length": status.playlist_length,
            "playlist_version": status.playlist_version,
            "audio": status.audio,
        },
        "current_song": current_song,
        "library": {
            "albums": album_count,
            "tracks": track_count,
            "library_version": library_version,
        },
        "stream_metadata": stream_metadata,
    })
}

fn api_queue_json(adapter: &MpdPlayerAdapter) -> Value {
    let songs: Vec<_> = adapter
        .playlist_info(None)
        .iter()
        .map(mpd_song_json)
        .collect();
    json!({ "items": songs })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ApiLibraryAlbumQuery {
    offset: usize,
    limit: usize,
    query: Option<String>,
    sort: ApiLibraryAlbumSort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApiLibraryAlbumSort {
    ArtistTitle,
    Title,
    Year,
}

impl Default for ApiLibraryAlbumQuery {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: API_LIBRARY_DEFAULT_LIMIT,
            query: None,
            sort: ApiLibraryAlbumSort::ArtistTitle,
        }
    }
}

fn api_library_albums_json(state: &Arc<ServerState>, path: &str) -> Result<Value, String> {
    let query = api_parse_library_album_query(path)?;
    let library = state.library.lock();
    let mut albums: Vec<_> = library.albums.iter().collect();

    if let Some(search) = query.query.as_deref() {
        let search = search.to_ascii_lowercase();
        albums.retain(|album| {
            album.title.to_ascii_lowercase().contains(&search)
                || album.artist().to_ascii_lowercase().contains(&search)
                || album
                    .edition
                    .as_deref()
                    .is_some_and(|edition| edition.to_ascii_lowercase().contains(&search))
        });
    }

    match query.sort {
        ApiLibraryAlbumSort::ArtistTitle => albums.sort_by(|a, b| {
            a.artist()
                .to_ascii_lowercase()
                .cmp(&b.artist().to_ascii_lowercase())
                .then_with(|| {
                    a.title
                        .to_ascii_lowercase()
                        .cmp(&b.title.to_ascii_lowercase())
                })
        }),
        ApiLibraryAlbumSort::Title => {
            albums.sort_by_key(|album| album.title.to_ascii_lowercase());
        }
        ApiLibraryAlbumSort::Year => {
            albums.sort_by_key(|album| {
                (
                    album.year.unwrap_or(u32::MAX),
                    album.title.to_ascii_lowercase(),
                )
            });
        }
    }

    let total = albums.len();
    let page: Vec<_> = albums
        .into_iter()
        .skip(query.offset)
        .take(query.limit)
        .map(api_album_json)
        .collect();
    let library_version = state
        .library_version
        .load(std::sync::atomic::Ordering::Relaxed);

    Ok(json!({
        "albums": page,
        "total": total,
        "offset": query.offset,
        "limit": query.limit,
        "library_version": library_version,
    }))
}

fn api_parse_library_album_query(path: &str) -> Result<ApiLibraryAlbumQuery, String> {
    let Some((_, raw_query)) = path.split_once('?') else {
        return Ok(ApiLibraryAlbumQuery::default());
    };

    let mut query = ApiLibraryAlbumQuery::default();
    for (key, value) in url::form_urlencoded::parse(raw_query.as_bytes()) {
        match key.as_ref() {
            "offset" => {
                query.offset = value
                    .parse::<usize>()
                    .map_err(|_| "offset must be a non-negative integer".to_string())?;
            }
            "limit" => {
                let limit = value
                    .parse::<usize>()
                    .map_err(|_| "limit must be a positive integer".to_string())?;
                if limit == 0 {
                    return Err("limit must be greater than zero".to_string());
                }
                query.limit = limit.min(API_LIBRARY_MAX_LIMIT);
            }
            "q" => {
                let value = value.trim();
                if !value.is_empty() {
                    query.query = Some(value.to_string());
                }
            }
            "sort" => {
                query.sort = match value.as_ref() {
                    "artist_title" | "artist" => ApiLibraryAlbumSort::ArtistTitle,
                    "title" => ApiLibraryAlbumSort::Title,
                    "year" => ApiLibraryAlbumSort::Year,
                    _ => return Err(format!("unsupported album sort: {value}")),
                };
            }
            _ => {}
        }
    }
    Ok(query)
}

fn api_library_album_tracks_json(state: &Arc<ServerState>, route: &str) -> Result<Value, String> {
    let album_id = route
        .strip_prefix("/api/v1/library/albums/")
        .and_then(|tail| tail.strip_suffix("/tracks"))
        .ok_or_else(|| "invalid album tracks route".to_string())?;
    let library = state.library.lock();
    let album = library
        .albums
        .iter()
        .find(|album| api_album_id(album) == album_id)
        .ok_or_else(|| "album not found".to_string())?;
    let tracks: Vec<_> = album
        .tracks
        .iter()
        .enumerate()
        .map(|(index, track)| api_track_json(track, album, index))
        .collect();
    Ok(json!({ "album": api_album_json(album), "tracks": tracks }))
}

fn api_library_album_artwork_response(
    state: &Arc<ServerState>,
    route: &str,
) -> Result<Vec<u8>, String> {
    let album_id = route
        .strip_prefix("/api/v1/library/albums/")
        .and_then(|tail| tail.strip_suffix("/artwork"))
        .ok_or_else(|| "invalid album artwork route".to_string())?;
    let library = state.library.lock();
    let album = library
        .albums
        .iter()
        .find(|album| api_album_id(album) == album_id)
        .ok_or_else(|| "album not found".to_string())?;
    let artwork = album
        .album_art_thumbnail
        .as_ref()
        .ok_or_else(|| "album artwork not found".to_string())?;
    Ok(api_binary_response(200, "image/png", artwork))
}

fn api_add_album_to_queue(state: &Arc<ServerState>, request: &ApiRequest) -> Result<Value, String> {
    let body = api_json_body(request)?;
    let album_id = body
        .get("album_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "album_id is required".to_string())?;
    let play_now = body
        .get("play_now")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let album = {
        let library = state.library.lock();
        library
            .albums
            .iter()
            .find(|album| api_album_id(album) == album_id)
            .cloned()
            .ok_or_else(|| "album not found".to_string())?
    };

    let mut queue = state.queue.lock();
    let index = queue.add(album);
    let source = if play_now { queue.jump_to(index) } else { None };
    let version = state
        .playlist_version
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        + 1;
    drop(queue);

    if let Some(source) = source {
        let mut player = state.player.lock();
        player
            .load_and_play_source(source, vec![], 2, None)
            .map_err(|e| e.to_string())?;
        drop(player);
        state.broadcast(SotfServerEvent::PlaybackChanged);
    }

    state.broadcast(SotfServerEvent::QueueChanged {
        playlist_version: version,
    });

    Ok(json!({
        "ok": true,
        "command": "queue.add-album",
        "index": index,
        "playlist_version": version,
    }))
}

fn api_clear_queue(state: &Arc<ServerState>) -> Result<Value, String> {
    let mut queue = state.queue.lock();
    queue.clear();
    let version = state
        .playlist_version
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        + 1;
    drop(queue);
    let mut player = state.player.lock();
    player.stop().map_err(|e| e.to_string())?;
    drop(player);
    state.broadcast(SotfServerEvent::PlaybackChanged);
    state.broadcast(SotfServerEvent::QueueChanged {
        playlist_version: version,
    });
    Ok(json!({
        "ok": true,
        "command": "queue.clear",
        "playlist_version": version,
    }))
}

fn api_delete_queue_album(state: &Arc<ServerState>, request: &ApiRequest) -> Result<Value, String> {
    let index = api_body_index(request)?;
    let mut queue = state.queue.lock();
    if index >= queue.items.len() {
        return Err("queue index out of range".to_string());
    }
    let was_current = queue.remove(index);
    let replacement = if was_current {
        queue.current_track_source()
    } else {
        None
    };
    let is_empty = queue.items.is_empty();
    let version = state
        .playlist_version
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        + 1;
    drop(queue);

    if was_current {
        let mut player = state.player.lock();
        if let Some(source) = replacement {
            player
                .load_and_play_source(source, vec![], 2, None)
                .map_err(|e| e.to_string())?;
        } else if is_empty {
            player.stop().map_err(|e| e.to_string())?;
        }
        drop(player);
        state.broadcast(SotfServerEvent::PlaybackChanged);
    }

    state.broadcast(SotfServerEvent::QueueChanged {
        playlist_version: version,
    });

    Ok(json!({
        "ok": true,
        "command": "queue.delete",
        "index": index,
        "was_current": was_current,
        "playlist_version": version,
    }))
}

fn api_jump_queue_album(state: &Arc<ServerState>, request: &ApiRequest) -> Result<Value, String> {
    let index = api_body_index(request)?;
    let mut queue = state.queue.lock();
    let source = queue
        .jump_to(index)
        .ok_or_else(|| "queue index out of range".to_string())?;
    drop(queue);
    let mut player = state.player.lock();
    player
        .load_and_play_source(source, vec![], 2, None)
        .map_err(|e| e.to_string())?;
    drop(player);
    state.broadcast(SotfServerEvent::PlaybackChanged);
    Ok(json!({
        "ok": true,
        "command": "queue.jump",
        "index": index,
    }))
}

fn api_body_index(request: &ApiRequest) -> Result<usize, String> {
    let body = api_json_body(request)?;
    let index = body
        .get("index")
        .and_then(Value::as_u64)
        .ok_or_else(|| "index is required".to_string())?;
    usize::try_from(index).map_err(|_| "index is too large".to_string())
}

fn api_command_response(command: &str, result: Result<(), String>) -> Vec<u8> {
    match result {
        Ok(()) => api_json_response(200, json!({ "ok": true, "command": command })),
        Err(err) => api_error_response(400, &err),
    }
}

fn api_json_body(request: &ApiRequest) -> Result<Value, String> {
    if request.body.is_empty() {
        return Err("JSON body is required".to_string());
    }
    serde_json::from_slice(&request.body).map_err(|e| format!("invalid JSON body: {e}"))
}

/// Handle a client completing the pairing ceremony.
/// Validates the nonce and adds the client's fingerprint to the trusted store.
fn api_pairing_complete(state: &Arc<ServerState>, request: &ApiRequest) -> Result<Value, String> {
    if !state
        .pairing_mode
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        return Err("pairing is not enabled".to_string());
    }

    let body = api_json_body(request)?;
    let nonce = body
        .get("nonce")
        .and_then(Value::as_str)
        .ok_or_else(|| "nonce is required".to_string())?;
    let fingerprint = body
        .get("fingerprint")
        .and_then(Value::as_str)
        .ok_or_else(|| "fingerprint is required".to_string())?;
    let name = body
        .get("name")
        .and_then(Value::as_str)
        .map(sanitize_pairing_client_name)
        .unwrap_or_else(|| "Unnamed Device".to_string());

    let mut expected_nonce = state.pairing_nonce.lock();
    if *expected_nonce != nonce {
        return Err("invalid nonce".to_string());
    }
    let fingerprint = normalize_certificate_fingerprint(fingerprint)?;

    state
        .trusted_clients
        .lock()
        .add(&fingerprint, &name)
        .map_err(|e| format!("failed to save trusted client: {e}"))?;
    insert_live_trusted_client(state, &fingerprint)?;

    state
        .pairing_mode
        .store(false, std::sync::atomic::Ordering::Relaxed);
    *expected_nonce = String::new();

    log::info!(
        "[pairing] Client '{}' added with fingerprint {}",
        name,
        fingerprint
    );
    Ok(json!({ "ok": true, "command": "pairing.complete" }))
}

pub fn normalize_certificate_fingerprint(fingerprint: &str) -> Result<String, String> {
    let compact: String = fingerprint
        .trim()
        .chars()
        .filter(|ch| *ch != ':' && *ch != '-' && !ch.is_ascii_whitespace())
        .collect();

    if compact.len() != 64 || !compact.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(
            "invalid fingerprint format; expected a SHA-256 certificate fingerprint as 64 hex characters, with optional ':' separators"
                .to_string(),
        );
    }

    let compact = compact.to_ascii_uppercase();
    let mut normalized = String::with_capacity(95);
    for (index, chunk) in compact.as_bytes().chunks(2).enumerate() {
        if index > 0 {
            normalized.push(':');
        }
        normalized.push(char::from(chunk[0]));
        normalized.push(char::from(chunk[1]));
    }

    Ok(normalized)
}

fn sanitize_pairing_client_name(name: &str) -> String {
    let name = name
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>();
    let name = name.trim();
    if name.is_empty() {
        "Unnamed Device".to_string()
    } else {
        name.chars().take(64).collect()
    }
}

fn insert_live_trusted_client(state: &ServerState, fingerprint: &str) -> Result<(), String> {
    let mut trusted = state
        .trusted_client_fingerprints
        .lock()
        .map_err(|e| format!("trusted fingerprint lock poisoned: {e}"))?;
    trusted.insert(fingerprint.to_string());
    Ok(())
}

fn remove_live_trusted_client(state: &ServerState, fingerprint: &str) -> Result<(), String> {
    let mut trusted = state
        .trusted_client_fingerprints
        .lock()
        .map_err(|e| format!("trusted fingerprint lock poisoned: {e}"))?;
    trusted.remove(fingerprint);
    Ok(())
}

fn mpd_song_json(song: &MpdSongInfo) -> Value {
    json!({
        "file": &song.file,
        "title": &song.title,
        "artist": &song.artist,
        "album": &song.album,
        "track": &song.track,
        "date": &song.date,
        "genre": &song.genre,
        "duration_secs": song.duration,
        "pos": song.pos,
        "id": song.id,
    })
}

fn api_album_json(album: &crate::library::Album) -> Value {
    json!({
        "id": api_album_id(album),
        "title": &album.title,
        "artist": album.artist(),
        "year": album.year,
        "track_count": album.tracks.len(),
        "edition": &album.edition,
        "dynamic_range": album.dynamic_range,
        "is_favorite": album.is_favorite,
        "play_count": album.play_count,
    })
}

fn api_track_json(
    track: &crate::library::Track,
    album: &crate::library::Album,
    index: usize,
) -> Value {
    json!({
        "id": api_track_id(track, album, index),
        "title": &track.title,
        "artist": &track.artist,
        "album": &album.title,
        "track": track.track_number,
        "duration_secs": track.duration_secs,
        "genre": &track.genre,
        "composer": &track.composer,
        "disc_number": track.disc_number,
        "conductor": &track.conductor,
        "performer": &track.performer,
        "ensemble": &track.ensemble,
        "channels": track.channels,
        "sample_rate": track.sample_rate,
        "bit_depth": track.bit_depth,
        "is_favorite": track.is_favorite,
        "play_count": track.play_count,
    })
}

fn api_album_id(album: &crate::library::Album) -> String {
    if let Some(id) = album.id {
        return format!("id:{id}");
    }
    if let Some(uuid) = album.uuid.as_deref()
        && is_safe_media_id(uuid)
    {
        return format!("uuid:{uuid}");
    }
    let mut hasher = Sha256::new();
    hasher.update(album.title.as_bytes());
    hasher.update([0]);
    hasher.update(album.artist().as_bytes());
    hasher.update([0]);
    hasher.update(album.edition.as_deref().unwrap_or_default().as_bytes());
    hasher.update([0]);
    if let Some(first_path) = album
        .tracks
        .first()
        .map(|track| track.path.to_string_lossy())
    {
        hasher.update(first_path.as_bytes());
    }
    let digest = hasher.finalize();
    format!("hash:{}", hex_prefix(&digest, 24))
}

fn api_track_id(
    track: &crate::library::Track,
    album: &crate::library::Album,
    index: usize,
) -> String {
    if let Some(uuid) = track.uuid.as_deref()
        && is_safe_media_id(uuid)
    {
        return format!("uuid:{uuid}");
    }
    media_track_id(track, album, index)
}

fn mpd_state_name(state: &MpdPlayState) -> &'static str {
    match state {
        MpdPlayState::Play => "play",
        MpdPlayState::Pause => "pause",
        MpdPlayState::Stop => "stop",
    }
}

fn api_json_response(status: u16, body: Value) -> Vec<u8> {
    let body = serde_json::to_vec(&body).unwrap_or_else(|_| b"{\"error\":\"json\"}".to_vec());
    let headers = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        status,
        api_status_text(status),
        body.len()
    );
    let mut response = headers.into_bytes();
    response.extend_from_slice(&body);
    response
}

fn api_binary_response(status: u16, content_type: &str, body: &[u8]) -> Vec<u8> {
    let headers = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        status,
        api_status_text(status),
        content_type,
        body.len()
    );
    let mut response = headers.into_bytes();
    response.extend_from_slice(body);
    response
}

fn api_error_response(status: u16, error: &str) -> Vec<u8> {
    api_json_response(status, json!({ "ok": false, "error": error }))
}

fn api_status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        416 => "Range Not Satisfiable",
        500 => "Internal Server Error",
        _ => "Error",
    }
}

fn api_auth_valid(headers: &[(String, String)], auth_token: &str) -> bool {
    let Some(value) = api_header(headers, "authorization") else {
        return false;
    };
    value.trim() == format!("Bearer {auth_token}")
}

fn api_header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(header, _)| header == name)
        .map(|(_, value)| value.as_str())
}

fn api_content_length_from_headers(header_bytes: &[u8]) -> Result<usize, String> {
    let header_text = std::str::from_utf8(header_bytes)
        .map_err(|_| "request headers are not valid UTF-8".to_string())?;
    let headers: Vec<_> = header_text
        .split("\r\n")
        .skip(1)
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect();
    api_content_length(&headers)
}

fn api_content_length(headers: &[(String, String)]) -> Result<usize, String> {
    match api_header(headers, "content-length") {
        Some(value) => value
            .parse::<usize>()
            .map_err(|_| "invalid Content-Length".to_string()),
        None => Ok(0),
    }
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|pos| pos + 4)
}

fn validate_sotf_api_token(settings: &SotfApiSettings) -> Result<String, String> {
    let token = settings.auth_token.as_deref().unwrap_or_default().trim();
    if token.is_empty() {
        return Err("SOTF API requires a non-empty auth_token when enabled".to_string());
    }
    Ok(token.to_string())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn track_to_song_info(track: &crate::library::Track, pos: u32) -> MpdSongInfo {
    MpdSongInfo {
        file: track.path.display().to_string(),
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: None,
        track: track.track_number,
        date: None,
        genre: track.genre.clone(),
        duration: track.duration_secs.map(|d| d as f64),
        pos,
        id: pos,
    }
}

fn track_to_media_track(
    track: &crate::library::Track,
    album: &crate::library::Album,
    index: usize,
) -> sotf_dlna::MediaTrack {
    let album_id = album
        .id
        .map_or_else(|| album.title.clone(), |id| id.to_string());

    sotf_dlna::MediaTrack {
        id: media_track_id(track, album, index),
        album_id,
        title: track.title.clone().unwrap_or_default(),
        artist: track.artist.clone().unwrap_or_default(),
        album: album.title.clone(),
        genre: track.genre.clone(),
        track_number: track.track_number,
        duration_secs: track.duration_secs.map(|d| d as f64),
        file_path: track.path.display().to_string(),
        mime_type: mime_type_for_path(&track.path).to_string(),
        sample_rate: track.sample_rate,
        channels: track.channels,
        bit_depth: track.bit_depth,
        file_size: std::fs::metadata(&track.path).ok().map(|m| m.len()),
    }
}

fn mime_type_for_path(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "flac" => "audio/flac",
        "mp3" => "audio/mpeg",
        "m4a" | "aac" => "audio/mp4",
        "ogg" | "oga" => "audio/ogg",
        "wav" => "audio/wav",
        "aif" | "aiff" => "audio/aiff",
        _ => "application/octet-stream",
    }
}

fn media_track_id(
    track: &crate::library::Track,
    album: &crate::library::Album,
    index: usize,
) -> String {
    if let Some(uuid) = track.uuid.as_deref()
        && is_safe_media_id(uuid)
    {
        return format!("track-{uuid}");
    }

    let mut hasher = Sha256::new();
    hasher.update(album.id.map(|id| id.to_le_bytes()).unwrap_or_default());
    hasher.update(album.title.as_bytes());
    hasher.update([0]);
    hasher.update(index.to_le_bytes());
    hasher.update([0]);
    hasher.update(track.path.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    format!("track-{}", hex_prefix(&digest, 24))
}

fn is_safe_media_id(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

fn hex_prefix(bytes: &[u8], chars: usize) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let byte_count = chars.div_ceil(2).min(bytes.len());
    let mut out = String::with_capacity(byte_count * 2);
    for &b in &bytes[..byte_count] {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out.truncate(chars);
    out
}

/// Flatten the queue into a flat list of MpdSongInfo (one entry per track).
fn flatten_queue_tracks(queue: &Queue) -> Vec<MpdSongInfo> {
    let mut result = Vec::new();
    let mut pos = 0u32;
    for item in &queue.items {
        for track in &item.album.tracks {
            let mut info = track_to_song_info(track, pos);
            info.album = Some(item.album.title.clone());
            result.push(info);
            pos += 1;
        }
    }
    result
}

/// URL that local-network DLNA clients can use to reach the media server.
#[must_use]
pub fn dlna_server_url(port: u16) -> String {
    dlna_server_url_for_bind("0.0.0.0", port)
}

/// URL that DLNA clients can use for a configured bind address.
#[must_use]
pub fn dlna_server_url_for_bind(bind_address: &str, port: u16) -> String {
    let host = dlna_advertised_ipv4(bind_address);
    format!("http://{host}:{port}/")
}

/// URL that SOTF remote clients can use for a configured API bind address.
#[must_use]
pub fn sotf_api_server_url_for_bind(bind_address: &str, port: u16) -> String {
    let host = dlna_advertised_ipv4(bind_address);
    format!("http://{host}:{port}/api/v1")
}

/// IPv4 address to advertise in DLNA URLs for a configured bind address.
#[must_use]
pub fn dlna_advertised_ipv4(bind_address: &str) -> Ipv4Addr {
    bind_address
        .trim()
        .parse::<Ipv4Addr>()
        .ok()
        .filter(|ip| !ip.is_unspecified())
        .unwrap_or_else(get_local_ipv4)
}

/// Detect a non-loopback local IPv4 address for DLNA announcements.
fn get_local_ipv4() -> Ipv4Addr {
    // Try to find a non-loopback IPv4 address by connecting to a public DNS
    // (no actual traffic is sent — the OS just picks the right interface).
    if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:53").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                if let std::net::IpAddr::V4(v4) = addr.ip() {
                    return v4;
                }
            }
        }
    }
    Ipv4Addr::LOCALHOST
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Run the app in headless server mode.
///
/// Loads the music library from the database, ensures the SOTF API is enabled,
/// starts enabled servers (SOTF API, MPD, DLNA), and blocks until a shutdown
/// signal (SIGINT/SIGTERM) is received.
pub fn run_server_mode() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = crate::config::load_server_config()?;
    if ensure_server_mode_api_config(&mut config) {
        match crate::config::save_server_config(&config) {
            Ok(()) => {
                log::info!("[server] Enabled SOTF API defaults in server config");
                eprintln!(
                    "SOTF API enabled on {}:{}",
                    config.api.bind_address, config.api.port
                );
            }
            Err(err) => {
                log::warn!("[server] Failed to persist SOTF API defaults: {}", err);
                eprintln!("Warning: could not save SOTF API defaults: {err}");
            }
        }
    }
    let config_dir =
        crate::config::get_app_config_dir().ok_or("Could not determine config directory")?;
    let trusted_clients = sotf_tls::TrustedClientStore::load(&config_dir)?;

    validate_server_mode_config(&config, &trusted_clients)?;

    // Load library from database
    let mut library = MusicLibrary::with_database()?;
    library.load_from_database()?;
    let album_count = library.albums.len();
    log::info!("[server] Library loaded: {} albums", album_count);
    eprintln!("Library loaded: {} albums", album_count);

    let player = Player::new();
    let event_broadcaster = crate::sotf_server_event::new_event_broadcaster(64);

    // Load or generate server certificate
    let cert_store = sotf_tls::CertStore::load_or_generate(&config_dir)?;
    let server_fingerprint = cert_store.server_fingerprint();

    log::info!("[server] Trusted clients loaded: {}", trusted_clients.len());
    let initial_mpd_trusted_client_fingerprints =
        initial_trusted_client_fingerprints(&config, &trusted_clients);
    if config.mpd.enabled
        && config.mpd.tls_enabled
        && config.mpd.auth_mode == federation_config::MpdAuthMode::Certificate
        && initial_mpd_trusted_client_fingerprints.is_empty()
    {
        eprintln!(
            "MPD certificate auth has no trusted clients yet. MPD will listen, but clients must pair through the SOTF API before they can connect."
        );
        log::warn!("[server] MPD mTLS starting with no trusted client fingerprints");
    }
    let trusted_client_fingerprints = Arc::new(std::sync::Mutex::new(
        initial_mpd_trusted_client_fingerprints,
    ));

    let state = Arc::new(ServerState {
        player: Mutex::new(player),
        library: Mutex::new(library),
        queue: Mutex::new(Queue::new()),
        playlist_version: std::sync::atomic::AtomicU32::new(1),
        library_version: std::sync::atomic::AtomicU64::new(1),
        events: event_broadcaster,
        pairing_mode: std::sync::atomic::AtomicBool::new(false),
        pairing_nonce: parking_lot::Mutex::new(String::new()),
        trusted_clients: parking_lot::Mutex::new(trusted_clients),
        trusted_client_fingerprints,
        server_fingerprint: server_fingerprint.clone(),
    });

    let mpd_tls_acceptor = if config.mpd.enabled && config.mpd.tls_enabled {
        Some(build_mpd_tls_acceptor(&config, &cert_store, &state)?)
    } else {
        None
    };

    if config.api.enabled {
        validate_sotf_api_token(&config.api)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    }

    // Build a tokio runtime for the async servers
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        // Register signal handler
        let tx = shutdown_tx.clone();
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            log::info!("[server] Shutdown signal received");
            eprintln!("\nShutting down...");
            let _ = tx.send(true);
        });

        let mut handles = Vec::new();

        // Start MPD server
        if config.mpd.enabled {
            let mpd_config = mpd_settings_to_config(&config, &state);
            let adapter: Arc<dyn PlayerAdapter> = Arc::new(MpdPlayerAdapter {
                state: Arc::clone(&state),
            });
            let mut server = MpdServer::with_config(mpd_config, adapter);
            if let Some(acceptor) = mpd_tls_acceptor.clone() {
                server.set_tls_acceptor(acceptor);
            }
            let cancel = shutdown_rx.clone();

            eprintln!(
                "MPD server listening on {}:{}",
                config.mpd.bind_address, config.mpd.port
            );

            handles.push(tokio::spawn(async move {
                if let Err(e) = server.run(cancel).await {
                    log::error!("[server] MPD server error: {}", e);
                    eprintln!("MPD server error: {}", e);
                }
            }));
        }

        // Start DLNA server
        if config.dlna.enabled {
            let device = DlnaDevice::new_server(&config.dlna.friendly_name, config.dlna.port);
            let adapter: Arc<dyn MediaServerAdapter> = Arc::new(DlnaLibraryAdapter {
                state: Arc::clone(&state),
            });
            let server = DlnaMediaServer::new(device, adapter);
            let cancel = shutdown_rx.clone();
            let bind_address = config.dlna.bind_address.clone();
            let local_ip = dlna_advertised_ipv4(&bind_address);
            let dlna_url = dlna_server_url_for_bind(&bind_address, config.dlna.port);

            eprintln!(
                "DLNA server '{}' listening on {}:{} (URL: {})",
                config.dlna.friendly_name, bind_address, config.dlna.port, dlna_url
            );

            handles.push(tokio::spawn(async move {
                if let Err(e) = server.run(&bind_address, local_ip, cancel).await {
                    log::error!("[server] DLNA server error: {}", e);
                    eprintln!("DLNA server error: {}", e);
                }
            }));
        }

        // Start SOTF LAN control API
        if config.api.enabled {
            let api_config = config.api.clone();
            let cancel = shutdown_rx.clone();
            let api_state = Arc::clone(&state);
            let api_bind_addr = format!("{}:{}", api_config.bind_address, api_config.port);
            if let Some(api_listener) = match TcpListener::bind(&api_bind_addr).await {
                Ok(listener) => Some(listener),
                Err(e) => {
                    log::error!("[server] SOTF API bind error: {}", e);
                    eprintln!("SOTF API bind error on {api_bind_addr}: {e}");
                    let _ = shutdown_tx.send(true);
                    None
                }
            } {
                eprintln!(
                    "SOTF API '{}' listening on {}:{}",
                    api_config.friendly_name, api_config.bind_address, api_config.port
                );

                let (api_discovery_tx, api_discovery_rx) = tokio::sync::watch::channel(false);
                let global_cancel = shutdown_rx.clone();
                let discovery_cancel_tx = api_discovery_tx.clone();
                tokio::spawn(async move {
                    let mut global_cancel = global_cancel;
                    let _ = global_cancel.changed().await;
                    let _ = discovery_cancel_tx.send(true);
                });

                let api_discovery_tx_on_exit = api_discovery_tx.clone();
                handles.push(tokio::spawn(async move {
                    if let Err(e) =
                        run_sotf_api_server(api_config, api_state, api_listener, cancel).await
                    {
                        log::error!("[server] SOTF API server error: {}", e);
                        eprintln!("SOTF API server error: {}", e);
                    }
                    let _ = api_discovery_tx_on_exit.send(true);
                }));

                let discovery_config = config.api.clone();
                let discovery_ip = get_local_ipv4();
                let pairing_enabled = state
                    .pairing_mode
                    .load(std::sync::atomic::Ordering::Relaxed);
                eprintln!(
                    "SOTF API discovery advertising _sotf._tcp for {}:{}",
                    discovery_ip, discovery_config.port
                );
                handles.push(tokio::spawn(async move {
                    if let Err(e) = run_sotf_lan_discovery(
                        discovery_config,
                        discovery_ip,
                        pairing_enabled,
                        api_discovery_rx,
                    )
                    .await
                    {
                        log::warn!("[server] SOTF API discovery error: {}", e);
                        eprintln!("SOTF API discovery warning: {}", e);
                    }
                }));
            }
        }

        eprintln!("Server mode running. Press Ctrl-C to stop.");

        // Wait for all server tasks to finish (they exit on shutdown signal)
        for handle in handles {
            let _ = handle.await;
        }
    });

    Ok(())
}

fn validate_server_mode_config(
    config: &ServerConfig,
    _trusted_clients: &sotf_tls::TrustedClientStore,
) -> Result<(), Box<dyn std::error::Error>> {
    if !config.mpd.enabled && !config.dlna.enabled && !config.api.enabled {
        return Err(
            "No servers are enabled in the configuration. Enable MPD, DLNA, or the SOTF API in Configure > Servers or ~/.config/sotf/servers.json, then re-run with --server."
                .into(),
        );
    }

    if config.api.enabled {
        validate_sotf_api_token(&config.api)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    }

    if config.mpd.enabled && config.mpd.tls_enabled {
        match config.mpd.auth_mode {
            federation_config::MpdAuthMode::Certificate => {
                let invalid_fingerprints =
                    invalid_configured_client_fingerprints(&config.mpd.trusted_client_fingerprints);
                if !invalid_fingerprints.is_empty() {
                    let shown = invalid_fingerprints
                        .iter()
                        .take(3)
                        .map(|fp| format!("'{fp}'"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let suffix = if invalid_fingerprints.len() > 3 {
                        format!(" and {} more", invalid_fingerprints.len() - 3)
                    } else {
                        String::new()
                    };
                    return Err(format!(
                        "MPD trusted client fingerprint configuration contains invalid fingerprint(s): {shown}{suffix}. Expected client certificate SHA-256 fingerprints as 64 hex characters, with optional ':' separators."
                    )
                    .into());
                }
            }
            federation_config::MpdAuthMode::Password => {
                if config
                    .mpd
                    .password
                    .as_deref()
                    .unwrap_or_default()
                    .is_empty()
                {
                    return Err(
                        "MPD is enabled with password authentication, but no password is configured. Set an MPD password in Configure > Servers or disable MPD."
                            .into(),
                    );
                }
            }
        }
    }

    Ok(())
}

fn invalid_configured_client_fingerprints(fingerprints: &[String]) -> Vec<String> {
    fingerprints
        .iter()
        .filter(|fingerprint| normalize_certificate_fingerprint(fingerprint).is_err())
        .cloned()
        .collect()
}

/// Convert the persisted `MpdSettings` into the `MpdServerConfig` used by the server.
fn mpd_settings_to_config(config: &ServerConfig, state: &Arc<ServerState>) -> MpdServerConfig {
    let settings = &config.mpd;
    MpdServerConfig {
        bind_address: settings.bind_address.clone(),
        port: settings.port,
        tls_enabled: settings.tls_enabled,
        auth_mode: match settings.auth_mode {
            federation_config::MpdAuthMode::Certificate => MpdAuthMode::Certificate,
            federation_config::MpdAuthMode::Password => MpdAuthMode::Password,
        },
        password: settings.password.clone(),
        trusted_client_fingerprints: Arc::clone(&state.trusted_client_fingerprints),
    }
}

fn initial_trusted_client_fingerprints(
    config: &ServerConfig,
    trusted_clients: &sotf_tls::TrustedClientStore,
) -> HashSet<String> {
    let mut fingerprints: HashSet<String> = config
        .mpd
        .trusted_client_fingerprints
        .iter()
        .filter_map(|fp| normalize_certificate_fingerprint(fp).ok())
        .collect();
    for client in trusted_clients.list() {
        if let Ok(fp) = normalize_certificate_fingerprint(&client.fingerprint) {
            fingerprints.insert(fp);
        }
    }
    fingerprints
}

fn build_mpd_tls_acceptor(
    config: &ServerConfig,
    cert_store: &sotf_tls::CertStore,
    state: &Arc<ServerState>,
) -> Result<tokio_rustls::TlsAcceptor, Box<dyn std::error::Error>> {
    let tls_config = match config.mpd.auth_mode {
        federation_config::MpdAuthMode::Certificate => {
            let trusted = Arc::clone(&state.trusted_client_fingerprints);
            sotf_tls::build_server_tls_config_mtls(
                cert_store.cert_clone(),
                cert_store.key_clone(),
                trusted,
            )?
        }
        federation_config::MpdAuthMode::Password => {
            if config
                .mpd
                .password
                .as_deref()
                .unwrap_or_default()
                .is_empty()
            {
                return Err("MPD password authentication requires a non-empty password".into());
            }
            sotf_tls::build_server_tls_config(cert_store.cert_clone(), cert_store.key_clone())?
        }
    };

    eprintln!(
        "MPD TLS certificate fingerprint: {}",
        cert_store.server_fingerprint()
    );
    Ok(tokio_rustls::TlsAcceptor::from(tls_config))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api_settings(token: Option<&str>) -> SotfApiSettings {
        SotfApiSettings {
            enabled: true,
            bind_address: "127.0.0.1".to_string(),
            port: 8732,
            friendly_name: "Test SOTF".to_string(),
            auth_token: token.map(str::to_string),
        }
    }

    #[test]
    fn sotf_api_requires_non_empty_token() {
        assert!(validate_sotf_api_token(&api_settings(None)).is_err());
        assert!(validate_sotf_api_token(&api_settings(Some("   "))).is_err());
        assert_eq!(
            validate_sotf_api_token(&api_settings(Some("secret"))).unwrap(),
            "secret"
        );
    }

    #[test]
    fn server_mode_api_defaults_enable_api_and_generate_token() {
        let mut config = ServerConfig::default();

        assert!(ensure_server_mode_api_config(&mut config));

        assert!(config.api.enabled);
        let token = config.api.auth_token.as_deref().unwrap();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|ch| ch.is_ascii_hexdigit()));
    }

    #[test]
    fn server_mode_api_defaults_preserve_existing_enabled_token() {
        let mut config = ServerConfig::default();
        config.api.enabled = true;
        config.api.auth_token = Some("existing-token".to_string());

        assert!(!ensure_server_mode_api_config(&mut config));

        assert!(config.api.enabled);
        assert_eq!(config.api.auth_token.as_deref(), Some("existing-token"));
    }

    #[test]
    fn dlna_server_url_includes_configured_port() {
        let url = dlna_server_url(8200);

        assert!(url.starts_with("http://"));
        assert!(url.ends_with(":8200/"));
    }

    #[test]
    fn dlna_server_url_uses_specific_bind_address() {
        let url = dlna_server_url_for_bind("192.168.1.42", 8200);

        assert_eq!(url, "http://192.168.1.42:8200/");
    }

    #[test]
    fn sotf_api_server_url_includes_api_path() {
        let url = sotf_api_server_url_for_bind("192.168.1.42", 8732);

        assert_eq!(url, "http://192.168.1.42:8732/api/v1");
    }

    #[test]
    fn dlna_advertised_ipv4_uses_specific_bind_address() {
        assert_eq!(
            dlna_advertised_ipv4("192.168.1.42"),
            Ipv4Addr::new(192, 168, 1, 42)
        );
    }

    fn empty_trusted_client_store() -> (tempfile::TempDir, sotf_tls::TrustedClientStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = sotf_tls::TrustedClientStore::load(dir.path()).unwrap();
        (dir, store)
    }

    fn valid_fingerprint() -> String {
        (0..32).map(|_| "AA").collect::<Vec<_>>().join(":")
    }

    #[test]
    fn certificate_fingerprint_normalization_accepts_common_hex_formats() {
        let compact = "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899";
        let colon = "aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99";
        let expected = "AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99";

        assert_eq!(
            normalize_certificate_fingerprint(compact).unwrap(),
            expected
        );
        assert_eq!(normalize_certificate_fingerprint(colon).unwrap(), expected);
    }

    #[test]
    fn server_mode_preflight_accepts_mpd_certificate_auth_without_trusted_clients() {
        let (_dir, trusted_clients) = empty_trusted_client_store();
        let mut config = ServerConfig::default();
        config.mpd.enabled = true;

        validate_server_mode_config(&config, &trusted_clients).unwrap();
    }

    #[test]
    fn mpd_tls_acceptor_allows_empty_certificate_trust_for_pairing_bootstrap() {
        let dir = tempfile::tempdir().unwrap();
        let cert_store = sotf_tls::CertStore::load_or_generate(dir.path()).unwrap();
        let state = test_state();
        let mut config = ServerConfig::default();
        config.mpd.enabled = true;

        build_mpd_tls_acceptor(&config, &cert_store, &state).unwrap();
    }

    #[test]
    fn server_mode_preflight_reports_invalid_configured_client_fingerprints() {
        let (_dir, trusted_clients) = empty_trusted_client_store();
        let mut config = ServerConfig::default();
        config.mpd.enabled = true;
        config.mpd.trusted_client_fingerprints = vec!["AA:BB:CC".to_string()];

        let err = validate_server_mode_config(&config, &trusted_clients)
            .expect_err("invalid configured fingerprint should be reported")
            .to_string();

        assert!(err.contains("invalid fingerprint"));
        assert!(err.contains("AA:BB:CC"));
        assert!(!err.contains("no trusted client fingerprints"));
    }

    #[test]
    fn server_mode_preflight_accepts_mpd_certificate_auth_with_configured_fingerprint() {
        let (_dir, trusted_clients) = empty_trusted_client_store();
        let mut config = ServerConfig::default();
        config.mpd.enabled = true;
        config.mpd.trusted_client_fingerprints = vec![valid_fingerprint()];

        validate_server_mode_config(&config, &trusted_clients).unwrap();
    }

    #[test]
    fn server_mode_preflight_accepts_mpd_certificate_auth_with_paired_client() {
        let (_dir, mut trusted_clients) = empty_trusted_client_store();
        trusted_clients
            .add(&valid_fingerprint(), "Test Client")
            .unwrap();
        let mut config = ServerConfig::default();
        config.mpd.enabled = true;

        validate_server_mode_config(&config, &trusted_clients).unwrap();
    }

    #[test]
    fn server_mode_preflight_rejects_mpd_password_auth_without_password() {
        let (_dir, trusted_clients) = empty_trusted_client_store();
        let mut config = ServerConfig::default();
        config.mpd.enabled = true;
        config.mpd.auth_mode = federation_config::MpdAuthMode::Password;

        let err = validate_server_mode_config(&config, &trusted_clients)
            .expect_err("password auth without password should be rejected")
            .to_string();

        assert!(err.contains("no password is configured"));
    }

    #[test]
    fn sotf_api_auth_accepts_only_bearer_token() {
        let headers = vec![("authorization".to_string(), "Bearer secret".to_string())];
        assert!(api_auth_valid(&headers, "secret"));
        assert!(!api_auth_valid(&headers, "other"));

        let headers = vec![("authorization".to_string(), "Basic secret".to_string())];
        assert!(!api_auth_valid(&headers, "secret"));
    }

    #[test]
    fn sotf_api_parses_request_with_body() {
        let raw =
            b"POST /api/v1/volume HTTP/1.1\r\nHost: localhost\r\nContent-Length: 13\r\n\r\n{\"volume\":42}";
        let header_end = find_header_end(raw).unwrap();
        let request = parse_api_request(raw, header_end).unwrap();
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/v1/volume");
        assert_eq!(api_header(&request.headers, "host"), Some("localhost"));
        assert_eq!(request.body, br#"{"volume":42}"#);
    }

    #[test]
    fn sotf_api_capabilities_are_public_and_media_aware() {
        let request = ApiRequest {
            method: "GET".to_string(),
            path: "/api/v1/capabilities".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
        };
        let state = Arc::new(ServerState {
            player: Mutex::new(Player::new()),
            library: Mutex::new(MusicLibrary::default()),
            queue: Mutex::new(Queue::new()),
            playlist_version: std::sync::atomic::AtomicU32::new(1),
            library_version: std::sync::atomic::AtomicU64::new(1),
            events: crate::sotf_server_event::new_event_broadcaster(64),
            pairing_mode: std::sync::atomic::AtomicBool::new(false),
            pairing_nonce: parking_lot::Mutex::new(String::new()),
            trusted_clients: parking_lot::Mutex::new(
                sotf_tls::TrustedClientStore::load(std::env::temp_dir().as_path()).unwrap(),
            ),
            trusted_client_fingerprints: Arc::new(std::sync::Mutex::new(HashSet::new())),
            server_fingerprint: "AA:BB:CC".to_string(),
        });
        let response =
            handle_sotf_api_request(request, &state, &api_settings(Some("secret")), "secret");
        let response = String::from_utf8(response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("\"media_range\":true"));
        assert!(response.contains("\"events\":true"));
    }

    #[test]
    fn sotf_api_media_range_parser_handles_common_forms() {
        assert_eq!(api_parse_range_header(None, 10).unwrap(), None);
        assert_eq!(
            api_parse_range_header(Some("bytes=2-5"), 10).unwrap(),
            Some((2, 5))
        );
        assert_eq!(
            api_parse_range_header(Some("bytes=6-"), 10).unwrap(),
            Some((6, 9))
        );
        assert_eq!(
            api_parse_range_header(Some("bytes=-4"), 10).unwrap(),
            Some((6, 9))
        );
        assert!(api_parse_range_header(Some("items=0-1"), 10).is_err());
        assert!(api_parse_range_header(Some("bytes=10-12"), 10).is_err());
        assert!(api_parse_range_header(Some("bytes=1-0"), 10).is_err());
        assert!(api_parse_range_header(Some("bytes=0-1,3-4"), 10).is_err());
    }

    #[test]
    fn broadcast_events_on_volume_change() {
        let state = Arc::new(ServerState {
            player: Mutex::new(Player::new()),
            library: Mutex::new(MusicLibrary::default()),
            queue: Mutex::new(Queue::new()),
            playlist_version: std::sync::atomic::AtomicU32::new(1),
            library_version: std::sync::atomic::AtomicU64::new(1),
            events: crate::sotf_server_event::new_event_broadcaster(64),
            pairing_mode: std::sync::atomic::AtomicBool::new(false),
            pairing_nonce: parking_lot::Mutex::new(String::new()),
            trusted_clients: parking_lot::Mutex::new(
                sotf_tls::TrustedClientStore::load(std::env::temp_dir().as_path()).unwrap(),
            ),
            trusted_client_fingerprints: Arc::new(std::sync::Mutex::new(HashSet::new())),
            server_fingerprint: "AA:BB:CC".to_string(),
        });
        let mut rx = state.events.subscribe();
        let adapter = MpdPlayerAdapter {
            state: Arc::clone(&state),
        };

        // Set volume to 50 first, then change by +10
        adapter.set_volume(50).unwrap();
        let _ = rx.try_recv(); // consume VolumeChanged from set_volume
        adapter.volume_change(10).unwrap();
        let event = rx.try_recv().expect("expected an event");
        assert_eq!(event, SotfServerEvent::VolumeChanged { volume: 60 });
    }

    #[test]
    fn broadcast_events_on_queue_clear() {
        let state = Arc::new(ServerState {
            player: Mutex::new(Player::new()),
            library: Mutex::new(MusicLibrary::default()),
            queue: Mutex::new(Queue::new()),
            playlist_version: std::sync::atomic::AtomicU32::new(1),
            library_version: std::sync::atomic::AtomicU64::new(1),
            events: crate::sotf_server_event::new_event_broadcaster(64),
            pairing_mode: std::sync::atomic::AtomicBool::new(false),
            pairing_nonce: parking_lot::Mutex::new(String::new()),
            trusted_clients: parking_lot::Mutex::new(
                sotf_tls::TrustedClientStore::load(std::env::temp_dir().as_path()).unwrap(),
            ),
            trusted_client_fingerprints: Arc::new(std::sync::Mutex::new(HashSet::new())),
            server_fingerprint: "AA:BB:CC".to_string(),
        });
        let mut rx = state.events.subscribe();

        api_clear_queue(&state).unwrap();
        // api_clear_queue broadcasts PlaybackChanged then QueueChanged
        let event1 = rx.try_recv().expect("expected first event after clear");
        assert!(matches!(event1, SotfServerEvent::PlaybackChanged));
        let event2 = rx.try_recv().expect("expected second event after clear");
        assert!(matches!(event2, SotfServerEvent::QueueChanged { .. }));
    }

    fn test_state() -> Arc<ServerState> {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let tmp = std::env::temp_dir().join(format!("sotf_test_tls_{n}"));
        std::fs::create_dir_all(&tmp).ok();
        Arc::new(ServerState {
            player: Mutex::new(Player::new()),
            library: Mutex::new(MusicLibrary::default()),
            queue: Mutex::new(Queue::new()),
            playlist_version: std::sync::atomic::AtomicU32::new(1),
            library_version: std::sync::atomic::AtomicU64::new(1),
            events: crate::sotf_server_event::new_event_broadcaster(64),
            pairing_mode: std::sync::atomic::AtomicBool::new(false),
            pairing_nonce: parking_lot::Mutex::new(String::new()),
            trusted_clients: parking_lot::Mutex::new(
                sotf_tls::TrustedClientStore::load(&tmp).unwrap()
            ),
            trusted_client_fingerprints: Arc::new(std::sync::Mutex::new(HashSet::new())),
            server_fingerprint: "AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99".to_string(),
        })
    }

    fn auth_header() -> Vec<(String, String)> {
        vec![("authorization".to_string(), "Bearer secret".to_string())]
    }

    fn auth_get(path: &str) -> String {
        format!(
            "GET {path} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer secret\r\nConnection: close\r\n\r\n"
        )
    }

    async fn read_http_response(addr: std::net::SocketAddr, request: String) -> String {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut response = Vec::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            stream.read_to_end(&mut response),
        )
        .await
        .unwrap()
        .unwrap();
        String::from_utf8(response).unwrap()
    }

    async fn read_until(stream: &mut TcpStream, needle: &str) -> String {
        let mut response = Vec::new();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            let mut buf = [0_u8; 1024];
            loop {
                let n = stream.read(&mut buf).await.unwrap();
                if n == 0 {
                    break;
                }
                response.extend_from_slice(&buf[..n]);
                if String::from_utf8_lossy(&response).contains(needle) {
                    break;
                }
            }
        })
        .await
        .unwrap();
        String::from_utf8(response).unwrap()
    }

    async fn connect_sse_client(addr: std::net::SocketAddr) -> TcpStream {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(auth_get("/api/v1/events").as_bytes())
            .await
            .unwrap();
        let response = read_until(&mut stream, "event: state").await;
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Content-Type: text/event-stream"));
        stream
    }

    async fn spawn_test_api_server(
        state: Arc<ServerState>,
    ) -> (
        std::net::SocketAddr,
        tokio::sync::watch::Sender<bool>,
        tokio::task::JoinHandle<Result<(), String>>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let handle = tokio::spawn(run_sotf_api_server(
            api_settings(Some("secret")),
            state,
            listener,
            shutdown_rx,
        ));
        (addr, shutdown_tx, handle)
    }

    async fn stop_test_api_server(
        shutdown_tx: tokio::sync::watch::Sender<bool>,
        handle: tokio::task::JoinHandle<Result<(), String>>,
    ) {
        let _ = shutdown_tx.send(true);
        tokio::time::timeout(std::time::Duration::from_secs(2), handle)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }

    fn album(title: &str, artist: &str, year: u32) -> crate::library::Album {
        crate::library::Album {
            title: title.to_string(),
            year: Some(year),
            tracks: vec![crate::library::Track {
                title: Some(format!("{title} track")),
                artist: Some(artist.to_string()),
                album_artist: Some(artist.to_string()),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn album_list_body(state: &Arc<ServerState>, path: &str) -> Value {
        api_library_albums_json(state, path).unwrap()
    }

    #[test]
    fn api_library_albums_pages_empty_library() {
        let state = test_state();
        let body = album_list_body(&state, "/api/v1/library/albums?offset=0&limit=10");
        assert_eq!(body["albums"].as_array().unwrap().len(), 0);
        assert_eq!(body["total"], 0);
        assert_eq!(body["offset"], 0);
        assert_eq!(body["limit"], 10);
        assert_eq!(body["library_version"], 1);
    }

    #[test]
    fn api_library_albums_pages_small_library() {
        let state = test_state();
        state.library.lock().albums = vec![
            album("Zebra", "Beta", 2020),
            album("Alpha", "Alpha", 2010),
            album("Moon", "Alpha", 2005),
        ];

        let body = album_list_body(&state, "/api/v1/library/albums?offset=1&limit=1");
        let albums = body["albums"].as_array().unwrap();
        assert_eq!(body["total"], 3);
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0]["title"], "Moon");
    }

    #[test]
    fn api_library_albums_pages_large_library_and_clamps_limit() {
        let state = test_state();
        state.library.lock().albums = (0..300)
            .map(|i| album(&format!("Album {i:03}"), "Artist", 2000 + (i % 20)))
            .collect();

        let body = album_list_body(&state, "/api/v1/library/albums?offset=250&limit=999");
        assert_eq!(body["total"], 300);
        assert_eq!(body["offset"], 250);
        assert_eq!(body["limit"], API_LIBRARY_MAX_LIMIT);
        assert_eq!(body["albums"].as_array().unwrap().len(), 50);
    }

    #[test]
    fn api_library_albums_query_filters_and_sorts() {
        let state = test_state();
        state.library.lock().albums = vec![
            album("Late", "Beta", 2020),
            album("Early", "Beta", 1990),
            album("Other", "Alpha", 2015),
        ];

        let body = album_list_body(&state, "/api/v1/library/albums?q=beta&sort=year&limit=10");
        let albums = body["albums"].as_array().unwrap();
        assert_eq!(body["total"], 2);
        assert_eq!(albums[0]["title"], "Early");
        assert_eq!(albums[1]["title"], "Late");
    }

    #[test]
    fn api_library_albums_rejects_invalid_bounds_and_sort() {
        assert!(api_parse_library_album_query("/api/v1/library/albums?limit=0").is_err());
        assert!(api_parse_library_album_query("/api/v1/library/albums?offset=-1").is_err());
        assert!(api_parse_library_album_query("/api/v1/library/albums?sort=random").is_err());
    }

    #[test]
    fn api_library_album_artwork_returns_png_bytes() {
        let state = test_state();
        let mut album = album("Art", "Artist", 2024);
        album.id = Some(42);
        album.album_art_thumbnail = Some(vec![137, 80, 78, 71]);
        state.library.lock().albums = vec![album];

        let response =
            api_library_album_artwork_response(&state, "/api/v1/library/albums/id:42/artwork")
                .unwrap();
        let response = String::from_utf8_lossy(&response);
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Content-Type: image/png"));
    }

    #[test]
    fn sotf_api_serves_parallel_clients_while_sse_client_is_connected() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let state = test_state();
            let (addr, shutdown_tx, server_handle) =
                spawn_test_api_server(Arc::clone(&state)).await;

            let sse_client = connect_sse_client(addr).await;
            let mut handles = Vec::new();
            for idx in 0..8 {
                let path = if idx % 2 == 0 {
                    "/api/v1/state"
                } else {
                    "/api/v1/queue"
                };
                handles.push(tokio::spawn(read_http_response(addr, auth_get(path))));
            }

            for handle in handles {
                let response = handle.await.unwrap();
                assert!(response.starts_with("HTTP/1.1 200 OK"));
            }

            drop(sse_client);
            stop_test_api_server(shutdown_tx, server_handle).await;
        });
    }

    #[test]
    fn sotf_api_broadcasts_events_to_multiple_sse_clients() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let state = test_state();
            let (addr, shutdown_tx, server_handle) =
                spawn_test_api_server(Arc::clone(&state)).await;

            let mut first = connect_sse_client(addr).await;
            let mut second = connect_sse_client(addr).await;

            state.broadcast(SotfServerEvent::VolumeChanged { volume: 77 });

            let first_event = read_until(&mut first, "event: volume_changed").await;
            let second_event = read_until(&mut second, "event: volume_changed").await;
            assert!(first_event.contains("\"volume\":77"));
            assert!(second_event.contains("\"volume\":77"));

            drop(first);
            drop(second);
            stop_test_api_server(shutdown_tx, server_handle).await;
        });
    }

    #[test]
    fn pairing_status_is_public_and_reflects_mode() {
        let state = test_state();

        // Pairing disabled
        let req = ApiRequest {
            method: "GET".to_string(),
            path: "/api/v1/pairing/status".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
        };
        let resp = handle_sotf_api_request(req, &state, &api_settings(Some("secret")), "secret");
        let resp_str = String::from_utf8(resp).unwrap();
        assert!(resp_str.starts_with("HTTP/1.1 200 OK"));
        assert!(resp_str.contains("\"pairing_enabled\":false"));
        assert!(resp_str.contains("\"server_fingerprint\""));

        // Enable pairing
        state
            .pairing_mode
            .store(true, std::sync::atomic::Ordering::Relaxed);
        *state.pairing_nonce.lock() = "ABC123".to_string();

        let req = ApiRequest {
            method: "GET".to_string(),
            path: "/api/v1/pairing/status".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
        };
        let resp = handle_sotf_api_request(req, &state, &api_settings(Some("secret")), "secret");
        let resp_str = String::from_utf8(resp).unwrap();
        assert!(resp_str.starts_with("HTTP/1.1 200 OK"));
        assert!(resp_str.contains("\"pairing_enabled\":true"));
        assert!(resp_str.contains("\"nonce\":null"));
    }

    #[test]
    fn pairing_complete_requires_enabled_mode() {
        let state = test_state();

        let req = ApiRequest {
            method: "POST".to_string(),
            path: "/api/v1/pairing/complete".to_string(),
            headers: Vec::new(),
            body: br#"{"nonce":"abc","fingerprint":"AA:BB:CC","name":"Test"}"#.to_vec(),
        };
        let resp = handle_sotf_api_request(req, &state, &api_settings(Some("secret")), "secret");
        let resp_str = String::from_utf8(resp).unwrap();
        assert!(resp_str.starts_with("HTTP/1.1 400"));
        assert!(resp_str.contains("pairing is not enabled"));
    }

    #[test]
    fn pairing_complete_validates_nonce() {
        let state = test_state();
        state
            .pairing_mode
            .store(true, std::sync::atomic::Ordering::Relaxed);
        *state.pairing_nonce.lock() = "GOOD01".to_string();

        let req = ApiRequest {
            method: "POST".to_string(),
            path: "/api/v1/pairing/complete".to_string(),
            headers: Vec::new(),
            body: br#"{"nonce":"BAD001","fingerprint":"AA:BB:CC","name":"Test"}"#.to_vec(),
        };
        let resp = handle_sotf_api_request(req, &state, &api_settings(Some("secret")), "secret");
        let resp_str = String::from_utf8(resp).unwrap();
        assert!(resp_str.starts_with("HTTP/1.1 400"));
        assert!(resp_str.contains("invalid nonce"));
    }

    #[test]
    fn pairing_complete_rejects_malformed_fingerprint() {
        let state = test_state();
        state
            .pairing_mode
            .store(true, std::sync::atomic::Ordering::Relaxed);
        *state.pairing_nonce.lock() = "PAIR01".to_string();

        let body = r#"{"nonce":"PAIR01","fingerprint":"ZZ:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99","name":"Bad"}"#;
        let req = ApiRequest {
            method: "POST".to_string(),
            path: "/api/v1/pairing/complete".to_string(),
            headers: Vec::new(),
            body: body.as_bytes().to_vec(),
        };
        let resp = handle_sotf_api_request(req, &state, &api_settings(Some("secret")), "secret");
        let resp_str = String::from_utf8(resp).unwrap();
        assert!(resp_str.starts_with("HTTP/1.1 400"));
        assert!(resp_str.contains("invalid fingerprint format"));
    }

    #[test]
    fn pairing_complete_adds_trusted_client() {
        let state = test_state();
        state
            .pairing_mode
            .store(true, std::sync::atomic::Ordering::Relaxed);
        *state.pairing_nonce.lock() = "PAIR01".to_string();

        let valid_fp = "AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99";
        let body = format!(
            "{{\"nonce\":\"PAIR01\",\"fingerprint\":\"{}\",\"name\":\"iPhone\"}}",
            valid_fp
        );
        let req = ApiRequest {
            method: "POST".to_string(),
            path: "/api/v1/pairing/complete".to_string(),
            headers: Vec::new(),
            body: body.into_bytes(),
        };
        let resp = handle_sotf_api_request(req, &state, &api_settings(Some("secret")), "secret");
        let resp_str = String::from_utf8(resp).unwrap();
        assert!(resp_str.starts_with("HTTP/1.1 200 OK"));
        assert!(resp_str.contains("\"command\":\"pairing.complete\""));

        assert!(state.trusted_clients.lock().contains(valid_fp));
        assert!(
            state
                .trusted_client_fingerprints
                .lock()
                .unwrap()
                .contains(valid_fp)
        );
        assert!(
            !state
                .pairing_mode
                .load(std::sync::atomic::Ordering::Relaxed)
        );
        assert!(state.pairing_nonce.lock().is_empty());
    }

    #[test]
    fn pairing_admin_requires_auth() {
        let state = test_state();

        let req = ApiRequest {
            method: "POST".to_string(),
            path: "/api/v1/pairing/enable".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
        };
        let resp = handle_sotf_api_request(req, &state, &api_settings(Some("secret")), "wrong");
        let resp_str = String::from_utf8(resp).unwrap();
        assert!(resp_str.starts_with("HTTP/1.1 401"));
    }

    #[test]
    fn pairing_enable_disable_cycle() {
        let state = test_state();

        // Enable
        let req = ApiRequest {
            method: "POST".to_string(),
            path: "/api/v1/pairing/enable".to_string(),
            headers: auth_header(),
            body: Vec::new(),
        };
        let resp = handle_sotf_api_request(req, &state, &api_settings(Some("secret")), "secret");
        let resp_str = String::from_utf8(resp).unwrap();
        assert!(resp_str.starts_with("HTTP/1.1 200 OK"));
        assert!(resp_str.contains("\"pairing_enabled\":true"));
        assert!(
            state
                .pairing_mode
                .load(std::sync::atomic::Ordering::Relaxed)
        );

        // Disable
        let req = ApiRequest {
            method: "POST".to_string(),
            path: "/api/v1/pairing/disable".to_string(),
            headers: auth_header(),
            body: Vec::new(),
        };
        let resp = handle_sotf_api_request(req, &state, &api_settings(Some("secret")), "secret");
        let resp_str = String::from_utf8(resp).unwrap();
        assert!(resp_str.starts_with("HTTP/1.1 200 OK"));
        assert!(resp_str.contains("\"pairing_enabled\":false"));
        assert!(
            !state
                .pairing_mode
                .load(std::sync::atomic::Ordering::Relaxed)
        );
    }

    #[test]
    fn pairing_revoke_client() {
        let state = test_state();
        let fp = "AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99";
        state.trusted_clients.lock().add(fp, "Test").unwrap();
        state
            .trusted_client_fingerprints
            .lock()
            .unwrap()
            .insert(fp.to_string());
        assert!(state.trusted_clients.lock().contains(fp));

        let req = ApiRequest {
            method: "DELETE".to_string(),
            path: format!("/api/v1/pairing/clients/{}", fp),
            headers: auth_header(),
            body: Vec::new(),
        };
        let resp = handle_sotf_api_request(req, &state, &api_settings(Some("secret")), "secret");
        let resp_str = String::from_utf8(resp).unwrap();
        assert!(resp_str.starts_with("HTTP/1.1 200 OK"));
        assert!(!state.trusted_clients.lock().contains(fp));
        assert!(
            !state
                .trusted_client_fingerprints
                .lock()
                .unwrap()
                .contains(fp)
        );
    }
}
