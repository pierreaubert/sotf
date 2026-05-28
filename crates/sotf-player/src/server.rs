//! Headless server mode for SOTF.
//!
//! When launched with `--server`, the app skips UI and runs MPD/DLNA servers
//! directly, allowing remote clients to browse the library and control playback.

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

const API_MAX_REQUEST_BYTES: usize = 64 * 1024;
const API_MAX_BODY_BYTES: usize = 32 * 1024;

/// Shared state for the headless server adapters.
struct ServerState {
    player: Mutex<Player>,
    library: Mutex<MusicLibrary>,
    queue: Mutex<Queue>,
    /// Playlist version counter — incremented on every queue mutation.
    playlist_version: std::sync::atomic::AtomicU32,
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
            return player.resume().map_err(|e| e.to_string());
        } else {
            queue.start()
        };

        match source {
            Some(source) => player
                .load_and_play_source(source, vec![], 2, None)
                .map_err(|e| e.to_string()),
            None => Err("No track to play".to_string()),
        }
    }

    fn pause(&self, state: Option<bool>) -> Result<(), String> {
        let player = self.state.player.lock();
        match state {
            Some(true) | None => player.pause().map_err(|e| e.to_string()),
            Some(false) => player.resume().map_err(|e| e.to_string()),
        }
    }

    fn stop(&self) -> Result<(), String> {
        let mut player = self.state.player.lock();
        player.stop().map_err(|e| e.to_string())
    }

    fn next(&self) -> Result<(), String> {
        let mut queue = self.state.queue.lock();
        let mut player = self.state.player.lock();

        match queue.next_track() {
            Some(source) => player
                .load_and_play_source(source, vec![], 2, None)
                .map_err(|e| e.to_string()),
            None => {
                player.stop().map_err(|e| e.to_string())?;
                Ok(())
            }
        }
    }

    fn previous(&self) -> Result<(), String> {
        let mut queue = self.state.queue.lock();
        let mut player = self.state.player.lock();

        match queue.previous_track() {
            Some(source) => player
                .load_and_play_source(source, vec![], 2, None)
                .map_err(|e| e.to_string()),
            None => Err("No previous track".to_string()),
        }
    }

    fn seek_pos(&self, _song_pos: u32, time: f64) -> Result<(), String> {
        let player = self.state.player.lock();
        player.seek(time).map_err(|e| e.to_string())
    }

    fn seek_cur(&self, time: f64) -> Result<(), String> {
        let player = self.state.player.lock();
        let current = player.get_position();
        player.seek(current + time).map_err(|e| e.to_string())
    }

    fn set_volume(&self, volume: u8) -> Result<(), String> {
        let player = self.state.player.lock();
        let vol_f32 = f32::from(volume) / 100.0;
        player.set_volume(vol_f32).map_err(|e| e.to_string())
    }

    fn volume_change(&self, delta: i8) -> Result<(), String> {
        let player = self.state.player.lock();
        let current = (player.get_volume() * 100.0) as i16;
        let new = (current + i16::from(delta)).clamp(0, 100);
        player
            .set_volume(new as f32 / 100.0)
            .map_err(|e| e.to_string())
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
            self.state
                .playlist_version
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
            self.state
                .playlist_version
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        } else {
            Err(format!("Invalid position: {}", pos))
        }
    }

    fn clear(&self) -> Result<(), String> {
        let mut queue = self.state.queue.lock();
        queue.clear();
        self.state
            .playlist_version
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
        _ => {}
    }

    if !api_auth_valid(&request.headers, auth_token) {
        return api_error_response(401, "missing or invalid bearer token");
    }

    let adapter = MpdPlayerAdapter {
        state: Arc::clone(state),
    };

    match (request.method.as_str(), route) {
        ("GET", "/api/v1/state") => api_json_response(200, api_state_json(state, &adapter)),
        ("GET", "/api/v1/events") => api_sse_snapshot_response(api_state_json(state, &adapter)),
        ("GET", "/api/v1/queue") => api_json_response(200, api_queue_json(&adapter)),
        ("GET", "/api/v1/library/albums") => api_json_response(200, api_library_albums_json(state)),
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
            Ok(()) => api_json_response(200, json!({ "ok": true, "command": "seek" })),
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
        ("GET", _) if route.starts_with("/api/v1/library/albums/") => {
            match api_library_album_tracks_json(state, route) {
                Ok(body) => api_json_response(200, body),
                Err(err) => api_error_response(404, &err),
            }
        }
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
            "library_search": false,
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
    let (album_count, track_count) = {
        let library = state.library.lock();
        let track_count = library
            .albums
            .iter()
            .map(|album| album.tracks.len())
            .sum::<usize>();
        (library.albums.len(), track_count)
    };

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
        },
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

fn api_library_albums_json(state: &Arc<ServerState>) -> Value {
    let library = state.library.lock();
    let albums: Vec<_> = library.albums.iter().map(api_album_json).collect();
    json!({ "albums": albums })
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
    state
        .playlist_version
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    drop(queue);

    if let Some(source) = source {
        let mut player = state.player.lock();
        player
            .load_and_play_source(source, vec![], 2, None)
            .map_err(|e| e.to_string())?;
    }

    Ok(json!({
        "ok": true,
        "command": "queue.add-album",
        "index": index,
        "playlist_version": state.playlist_version.load(std::sync::atomic::Ordering::Relaxed),
    }))
}

fn api_clear_queue(state: &Arc<ServerState>) -> Result<Value, String> {
    let mut queue = state.queue.lock();
    queue.clear();
    state
        .playlist_version
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    drop(queue);
    let mut player = state.player.lock();
    player.stop().map_err(|e| e.to_string())?;
    Ok(json!({
        "ok": true,
        "command": "queue.clear",
        "playlist_version": state.playlist_version.load(std::sync::atomic::Ordering::Relaxed),
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
    state
        .playlist_version
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
    }

    Ok(json!({
        "ok": true,
        "command": "queue.delete",
        "index": index,
        "was_current": was_current,
        "playlist_version": state.playlist_version.load(std::sync::atomic::Ordering::Relaxed),
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

fn api_sse_snapshot_response(body: Value) -> Vec<u8> {
    let body = format!("event: state\ndata: {}\n\n", body);
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut response = headers.into_bytes();
    response.extend_from_slice(body.as_bytes());
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
/// Loads the music library from the database, starts any enabled servers
/// (MPD, DLNA), and blocks until a shutdown signal (SIGINT/SIGTERM) is received.
///
/// Returns an error if no servers are enabled in the configuration.
pub fn run_server_mode() -> Result<(), Box<dyn std::error::Error>> {
    let config = crate::config::load_server_config()?;

    if !config.mpd.enabled && !config.dlna.enabled && !config.api.enabled {
        eprintln!(
            "error: No servers are enabled in the configuration.\n\
             \n\
             Configure servers in ~/.config/sotf/servers.json or use the\n\
             Configure > Servers screen in the UI to enable MPD, DLNA, and/or the SOTF API,\n\
             then re-run with --server."
        );
        std::process::exit(1);
    }

    // Load library from database
    let mut library = MusicLibrary::with_database()?;
    library.load_from_database()?;
    let album_count = library.albums.len();
    log::info!("[server] Library loaded: {} albums", album_count);
    eprintln!("Library loaded: {} albums", album_count);

    let player = Player::new();

    let state = Arc::new(ServerState {
        player: Mutex::new(player),
        library: Mutex::new(library),
        queue: Mutex::new(Queue::new()),
        playlist_version: std::sync::atomic::AtomicU32::new(1),
    });

    let mpd_tls_acceptor = if config.mpd.enabled && config.mpd.tls_enabled {
        Some(build_mpd_tls_acceptor(&config)?)
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
            let mpd_config = mpd_settings_to_config(&config);
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
            let local_ip = get_local_ipv4();

            eprintln!(
                "DLNA server '{}' on port {} (IP: {})",
                config.dlna.friendly_name, config.dlna.port, local_ip
            );

            handles.push(tokio::spawn(async move {
                if let Err(e) = server.run(local_ip, cancel).await {
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
                eprintln!(
                    "SOTF API discovery advertising _sotf._tcp for {}:{}",
                    discovery_ip, discovery_config.port
                );
                handles.push(tokio::spawn(async move {
                    if let Err(e) =
                        run_sotf_lan_discovery(discovery_config, discovery_ip, api_discovery_rx)
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

/// Convert the persisted `MpdSettings` into the `MpdServerConfig` used by the server.
fn mpd_settings_to_config(config: &ServerConfig) -> MpdServerConfig {
    let settings = &config.mpd;
    let trusted_client_fingerprints = std::sync::Arc::new(std::sync::Mutex::new(
        settings
            .trusted_client_fingerprints
            .iter()
            .cloned()
            .collect(),
    ));
    MpdServerConfig {
        bind_address: settings.bind_address.clone(),
        port: settings.port,
        tls_enabled: settings.tls_enabled,
        auth_mode: match settings.auth_mode {
            federation_config::MpdAuthMode::Certificate => MpdAuthMode::Certificate,
            federation_config::MpdAuthMode::Password => MpdAuthMode::Password,
        },
        password: settings.password.clone(),
        trusted_client_fingerprints,
    }
}

fn build_mpd_tls_acceptor(
    config: &ServerConfig,
) -> Result<tokio_rustls::TlsAcceptor, Box<dyn std::error::Error>> {
    let cert_store = sotf_tls::CertStore::load_or_generate(
        &crate::config::get_app_config_dir().ok_or("Could not determine config directory")?,
    )?;

    let tls_config = match config.mpd.auth_mode {
        federation_config::MpdAuthMode::Certificate => {
            if config.mpd.trusted_client_fingerprints.is_empty() {
                return Err(
                    "MPD certificate authentication requires at least one trusted client fingerprint"
                        .into(),
                );
            }
            let trusted = std::sync::Arc::new(std::sync::Mutex::new(
                config
                    .mpd
                    .trusted_client_fingerprints
                    .iter()
                    .cloned()
                    .collect(),
            ));
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
}
