//! Headless server mode for SOTF.
//!
//! When launched with `--server`, the app skips UI and runs MPD/DLNA servers
//! directly, allowing remote clients to browse the library and control playback.

use std::net::Ipv4Addr;
use std::sync::Arc;

use parking_lot::Mutex;
use sotf_dlna::{DlnaDevice, DlnaMediaServer, MediaServerAdapter};
use sotf_mpd::{
    FilterExpr, MpdAuthMode, MpdDirEntry, MpdPlayState, MpdServer, MpdServerConfig, MpdSongInfo,
    MpdStatus, PlayerAdapter,
};

use crate::federation_config::{self, ServerConfig};
use crate::library::MusicLibrary;
use crate::player::Player;
use crate::queue::Queue;

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
            Some(source) => {
                player
                    .load_and_play_source(source, vec![], 2, None)
                    .map_err(|e| e.to_string())
            }
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
                "album" => {
                    if !values.contains(&album.title) {
                        values.push(album.title.clone());
                    }
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
        let album = library.albums.iter().find(|a| {
            a.id.map_or(false, |id| id.to_string() == album_id) || a.title == album_id
        });

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

    fn search_tracks(&self, query: &str, start: u32, count: u32) -> (Vec<sotf_dlna::MediaTrack>, u32) {
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
            .take(if count == 0 { usize::MAX } else { count as usize })
            .collect();
        (page, total)
    }

    fn album_count(&self) -> u32 {
        let library = self.state.library.lock();
        library.albums.len() as u32
    }
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

    // Guess MIME type from extension
    let mime = match track
        .path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "flac" => "audio/flac",
        "mp3" => "audio/mpeg",
        "m4a" | "aac" => "audio/mp4",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        _ => "audio/unknown",
    };

    sotf_dlna::MediaTrack {
        id: format!("{}-{}", album_id, index),
        album_id,
        title: track.title.clone().unwrap_or_default(),
        artist: track.artist.clone().unwrap_or_default(),
        album: album.title.clone(),
        genre: track.genre.clone(),
        track_number: track.track_number,
        duration_secs: track.duration_secs.map(|d| d as f64),
        file_path: track.path.display().to_string(),
        mime_type: mime.to_string(),
        sample_rate: track.sample_rate,
        channels: track.channels,
        bit_depth: track.bit_depth,
        file_size: std::fs::metadata(&track.path).ok().map(|m| m.len()),
    }
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

    if !config.mpd.enabled && !config.dlna.enabled {
        eprintln!(
            "error: No servers are enabled in the configuration.\n\
             \n\
             Configure servers in ~/.config/sotf/servers.json or use the\n\
             Configure > Servers screen in the UI to enable MPD and/or DLNA,\n\
             then re-run with --server."
        );
        std::process::exit(1);
    }

    // Load library from database
    let mut library = MusicLibrary::with_database()?;
    library.load_from_database()?;
    let album_count = library.albums.len();
    log::info!(
        "[server] Library loaded: {} albums",
        album_count
    );
    eprintln!("Library loaded: {} albums", album_count);

    let player = Player::new();

    let state = Arc::new(ServerState {
        player: Mutex::new(player),
        library: Mutex::new(library),
        queue: Mutex::new(Queue::new()),
        playlist_version: std::sync::atomic::AtomicU32::new(1),
    });

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
            let adapter: Arc<dyn PlayerAdapter> =
                Arc::new(MpdPlayerAdapter {
                    state: Arc::clone(&state),
                });
            let server = MpdServer::with_config(mpd_config, adapter);
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
            let device =
                DlnaDevice::new_server(&config.dlna.friendly_name, config.dlna.port);
            let adapter: Arc<dyn MediaServerAdapter> =
                Arc::new(DlnaLibraryAdapter {
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
    MpdServerConfig {
        bind_address: settings.bind_address.clone(),
        port: settings.port,
        tls_enabled: settings.tls_enabled,
        auth_mode: match settings.auth_mode {
            federation_config::MpdAuthMode::Certificate => MpdAuthMode::Certificate,
            federation_config::MpdAuthMode::Password => MpdAuthMode::Password,
        },
        password: settings.password.clone(),
    }
}
