// Integration tests for sotf-mpd.
//
// These tests exercise the crate's public API as a black box: command lines are
// parsed, dispatched to the in-memory adapter, and formatted as MPD responses.

use sotf_mpd::{
    FilterExpr, MpdCommand, MpdDirEntry, MpdPlayState, MpdSongInfo, MpdStatus, PlayerAdapter,
    handle_command, parse_command,
};
use std::sync::{Arc, Mutex};

// ============================================================================
// Test adapter: a fully functional in-memory player used only through the
// public `PlayerAdapter` trait.
// ============================================================================

struct MemoryAdapter {
    state: Mutex<MpdPlayState>,
    volume: Mutex<u8>,
    queue: Mutex<Vec<MpdSongInfo>>,
    current_pos: Mutex<Option<u32>>,
    elapsed: Mutex<f64>,
    duration: Mutex<f64>,
    audio: Mutex<Option<String>>,
    playlist_version: Mutex<u32>,
    next_id: Mutex<u32>,
}

impl MemoryAdapter {
    fn copy_song(song: &MpdSongInfo) -> MpdSongInfo {
        MpdSongInfo {
            file: song.file.clone(),
            title: song.title.clone(),
            artist: song.artist.clone(),
            album: song.album.clone(),
            track: song.track,
            date: song.date.clone(),
            genre: song.genre.clone(),
            duration: song.duration,
            pos: song.pos,
            id: song.id,
        }
    }

    fn new() -> Self {
        Self {
            state: Mutex::new(MpdPlayState::Stop),
            volume: Mutex::new(75),
            queue: Mutex::new(Vec::new()),
            current_pos: Mutex::new(None),
            elapsed: Mutex::new(0.0),
            duration: Mutex::new(240.0),
            audio: Mutex::new(Some("44100:16:2".into())),
            playlist_version: Mutex::new(1),
            next_id: Mutex::new(1),
        }
    }

    fn with_songs(songs: Vec<(&str, &str, &str, f64)>) -> Self {
        let adapter = Self::new();
        let mut queue = Vec::new();
        let mut next_id = 1;
        for (pos, (file, title, artist, duration)) in songs.into_iter().enumerate() {
            queue.push(MpdSongInfo {
                file: file.into(),
                title: Some(title.into()),
                artist: Some(artist.into()),
                album: None,
                track: None,
                date: None,
                genre: None,
                duration: Some(duration),
                pos: pos as u32,
                id: next_id,
            });
            next_id += 1;
        }
        *adapter.queue.lock().unwrap() = queue;
        *adapter.next_id.lock().unwrap() = next_id;
        *adapter.playlist_version.lock().unwrap() = 2;
        adapter
    }

    fn bump_version(&self) {
        let mut v = self.playlist_version.lock().unwrap();
        *v += 1;
    }

    fn song_at_pos(&self, pos: u32) -> Option<MpdSongInfo> {
        self.queue
            .lock()
            .unwrap()
            .get(pos as usize)
            .map(Self::copy_song)
    }

    fn renumber_queue(&self) {
        let mut q = self.queue.lock().unwrap();
        for (pos, song) in q.iter_mut().enumerate() {
            song.pos = pos as u32;
        }
    }
}

impl PlayerAdapter for MemoryAdapter {
    fn play(&self, pos: Option<u32>) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        let queue = self.queue.lock().unwrap();
        let mut current_pos = self.current_pos.lock().unwrap();
        match pos {
            Some(p) if (p as usize) < queue.len() => {
                *current_pos = Some(p);
                *state = MpdPlayState::Play;
                Ok(())
            }
            None if !queue.is_empty() => {
                *current_pos = Some(0);
                *state = MpdPlayState::Play;
                Ok(())
            }
            None => {
                *state = MpdPlayState::Play;
                Ok(())
            }
            Some(p) => Err(format!("position {} out of range", p)),
        }
    }

    fn pause(&self, state: Option<bool>) -> Result<(), String> {
        let mut s = self.state.lock().unwrap();
        match state {
            Some(true) => *s = MpdPlayState::Pause,
            Some(false) => *s = MpdPlayState::Play,
            None => {
                *s = match *s {
                    MpdPlayState::Play => MpdPlayState::Pause,
                    MpdPlayState::Pause => MpdPlayState::Play,
                    MpdPlayState::Stop => MpdPlayState::Pause,
                };
            }
        }
        Ok(())
    }

    fn stop(&self) -> Result<(), String> {
        *self.state.lock().unwrap() = MpdPlayState::Stop;
        Ok(())
    }

    fn next(&self) -> Result<(), String> {
        let mut current_pos = self.current_pos.lock().unwrap();
        let queue_len = self.queue.lock().unwrap().len() as u32;
        if let Some(pos) = current_pos.as_mut() {
            *pos = (*pos + 1).min(queue_len.saturating_sub(1));
        }
        Ok(())
    }

    fn previous(&self) -> Result<(), String> {
        let mut current_pos = self.current_pos.lock().unwrap();
        if let Some(pos) = current_pos.as_mut() {
            *pos = pos.saturating_sub(1);
        }
        Ok(())
    }

    fn seek_pos(&self, song_pos: u32, time: f64) -> Result<(), String> {
        let queue = self.queue.lock().unwrap();
        if (song_pos as usize) >= queue.len() {
            return Err(format!("position {} out of range", song_pos));
        }
        drop(queue);
        *self.current_pos.lock().unwrap() = Some(song_pos);
        *self.elapsed.lock().unwrap() = time.max(0.0);
        Ok(())
    }

    fn seek_cur(&self, time: f64) -> Result<(), String> {
        *self.elapsed.lock().unwrap() = time.max(0.0);
        Ok(())
    }

    fn set_volume(&self, volume: u8) -> Result<(), String> {
        *self.volume.lock().unwrap() = volume;
        Ok(())
    }

    fn volume_change(&self, delta: i8) -> Result<(), String> {
        let mut v = self.volume.lock().unwrap();
        let new = (*v as i16 + delta as i16).clamp(0, 100) as u8;
        *v = new;
        Ok(())
    }

    fn status(&self) -> MpdStatus {
        let state = *self.state.lock().unwrap();
        let current_pos = *self.current_pos.lock().unwrap();
        let queue = self.queue.lock().unwrap();
        MpdStatus {
            volume: *self.volume.lock().unwrap(),
            repeat: false,
            random: false,
            single: false,
            consume: false,
            state,
            song: current_pos,
            songid: current_pos.and_then(|p| queue.get(p as usize).map(|s| s.id)),
            elapsed: *self.elapsed.lock().unwrap(),
            duration: *self.duration.lock().unwrap(),
            audio: self.audio.lock().unwrap().clone(),
            playlist_length: queue.len() as u32,
            playlist_version: *self.playlist_version.lock().unwrap(),
        }
    }

    fn current_song(&self) -> Option<MpdSongInfo> {
        self.current_pos
            .lock()
            .unwrap()
            .and_then(|p| self.song_at_pos(p))
    }

    fn playlist_info(&self, range: Option<(u32, Option<u32>)>) -> Vec<MpdSongInfo> {
        let q = self.queue.lock().unwrap();
        let start = range.map(|(s, _)| s as usize).unwrap_or(0);
        let end = range
            .and_then(|(_, e)| e)
            .map(|e| (e as usize).min(q.len()))
            .unwrap_or(q.len());
        q.get(start..end)
            .unwrap_or(&[])
            .iter()
            .map(Self::copy_song)
            .collect()
    }

    fn playlist_song_by_id(&self, id: u32) -> Option<MpdSongInfo> {
        self.queue
            .lock()
            .unwrap()
            .iter()
            .find(|s| s.id == id)
            .map(Self::copy_song)
    }

    fn add(&self, uri: &str) -> Result<(), String> {
        let mut q = self.queue.lock().unwrap();
        let mut next_id = self.next_id.lock().unwrap();
        let pos = q.len() as u32;
        q.push(MpdSongInfo {
            file: uri.into(),
            title: None,
            artist: None,
            album: None,
            track: None,
            date: None,
            genre: None,
            duration: None,
            pos,
            id: *next_id,
        });
        *next_id += 1;
        drop(q);
        self.bump_version();
        Ok(())
    }

    fn add_id(&self, uri: &str, pos: Option<u32>) -> Result<u32, String> {
        let mut q = self.queue.lock().unwrap();
        let mut next_id = self.next_id.lock().unwrap();
        let insert_pos = pos.map(|p| (p as usize).min(q.len())).unwrap_or(q.len());
        let id = *next_id;
        *next_id += 1;
        q.insert(
            insert_pos,
            MpdSongInfo {
                file: uri.into(),
                title: None,
                artist: None,
                album: None,
                track: None,
                date: None,
                genre: None,
                duration: None,
                pos: insert_pos as u32,
                id,
            },
        );
        drop(q);
        self.renumber_queue();
        self.bump_version();
        Ok(id)
    }

    fn delete(&self, pos: u32) -> Result<(), String> {
        let mut q = self.queue.lock().unwrap();
        if (pos as usize) >= q.len() {
            return Err(format!("position {} out of range", pos));
        }
        q.remove(pos as usize);
        drop(q);
        self.renumber_queue();
        self.bump_version();
        Ok(())
    }

    fn clear(&self) -> Result<(), String> {
        self.queue.lock().unwrap().clear();
        *self.current_pos.lock().unwrap() = None;
        self.bump_version();
        Ok(())
    }

    fn search(&self, filters: &[FilterExpr], _exact: bool) -> Vec<MpdSongInfo> {
        let q = self.queue.lock().unwrap();
        q.iter()
            .filter(|s| {
                filters.iter().all(|f| match f.tag.as_str() {
                    "artist" => s
                        .artist
                        .as_ref()
                        .map(|a| a.to_lowercase().contains(&f.value.to_lowercase()))
                        .unwrap_or(false),
                    "album" => s
                        .album
                        .as_ref()
                        .map(|a| a.to_lowercase().contains(&f.value.to_lowercase()))
                        .unwrap_or(false),
                    "title" => s
                        .title
                        .as_ref()
                        .map(|t| t.to_lowercase().contains(&f.value.to_lowercase()))
                        .unwrap_or(false),
                    _ => false,
                })
            })
            .map(Self::copy_song)
            .collect()
    }

    fn list_tag(&self, tag: &str, _filters: &[FilterExpr]) -> Vec<String> {
        let q = self.queue.lock().unwrap();
        let mut values: Vec<String> = q
            .iter()
            .filter_map(|s| match tag.to_lowercase().as_str() {
                "artist" => s.artist.clone(),
                "album" => s.album.clone(),
                "title" => s.title.clone(),
                _ => None,
            })
            .collect();
        values.sort();
        values.dedup();
        values
    }

    fn lsinfo(&self, _path: Option<&str>) -> Vec<MpdDirEntry> {
        vec![
            MpdDirEntry {
                is_directory: true,
                path: "Music".into(),
            },
            MpdDirEntry {
                is_directory: false,
                path: "Music/test.flac".into(),
            },
        ]
    }
}

// ============================================================================
// Test harness
// ============================================================================

struct TestSession {
    adapter: Arc<dyn PlayerAdapter>,
}

impl TestSession {
    fn new(adapter: Arc<dyn PlayerAdapter>) -> Self {
        Self { adapter }
    }

    fn exchange(&self, line: &str) -> String {
        match parse_command(line) {
            Ok(cmd) => handle_command(&cmd, self.adapter.as_ref()).format(),
            Err(err) => err.format(),
        }
    }

    fn exchange_command_list(&self, lines: &[&str], ok_between: bool) -> String {
        let mut commands = Vec::with_capacity(lines.len());
        for line in lines {
            match parse_command(line) {
                Ok(cmd) => commands.push(cmd),
                Err(err) => return err.format(),
            }
        }
        execute_command_list(&commands, ok_between, self.adapter.as_ref())
    }
}

fn execute_command_list(
    commands: &[MpdCommand],
    ok_between: bool,
    adapter: &dyn PlayerAdapter,
) -> String {
    let mut output = String::new();

    for (index, cmd) in commands.iter().enumerate() {
        match handle_command(cmd, adapter) {
            sotf_mpd::MpdResponse::Ok(kvs) => {
                for item in kvs {
                    output.push_str(&item.key);
                    output.push_str(": ");
                    output.push_str(&item.value);
                    output.push('\n');
                }
                if ok_between {
                    output.push_str("list_OK\n");
                }
            }
            sotf_mpd::MpdResponse::Error(mut err) => {
                err.command_index = index;
                output.push_str(&err.format());
                return output;
            }
            sotf_mpd::MpdResponse::ListOk => output.push_str("list_OK\n"),
        }
    }

    output.push_str("OK\n");
    output
}

// ============================================================================
// Integration tests
// ============================================================================

#[test]
fn test_play_pause_stop_state_transitions() {
    let adapter: Arc<dyn PlayerAdapter> = Arc::new(MemoryAdapter::with_songs(vec![(
        "music/a.flac",
        "A",
        "Artist",
        180.0,
    )]));
    let session = TestSession::new(adapter);

    let resp = session.exchange("play");
    assert!(resp.contains("OK"));

    let resp = session.exchange("status");
    assert!(resp.contains("state: play"), "got: {}", resp);

    let resp = session.exchange("pause 1");
    assert!(resp.contains("OK"));

    let resp = session.exchange("status");
    assert!(resp.contains("state: pause"), "got: {}", resp);

    let resp = session.exchange("pause 0");
    assert!(resp.contains("OK"));

    let resp = session.exchange("status");
    assert!(resp.contains("state: play"), "got: {}", resp);

    let resp = session.exchange("stop");
    assert!(resp.contains("OK"));

    let resp = session.exchange("status");
    assert!(resp.contains("state: stop"), "got: {}", resp);
    assert!(
        !resp.contains("elapsed:"),
        "stopped state must omit elapsed"
    );
}

#[test]
fn test_status_outputs_and_stats() {
    let adapter: Arc<dyn PlayerAdapter> = Arc::new(MemoryAdapter::with_songs(vec![(
        "music/a.flac",
        "A",
        "Artist",
        240.0,
    )]));
    let session = TestSession::new(adapter);

    let resp = session.exchange("status");
    assert!(resp.contains("volume: 75"), "got: {}", resp);
    assert!(resp.contains("state: stop"), "got: {}", resp);
    assert!(resp.contains("playlistlength: 1"), "got: {}", resp);
    assert!(resp.contains("playlist: 2"), "got: {}", resp);
    assert!(resp.ends_with("OK\n"));

    let resp = session.exchange("outputs");
    assert!(
        resp.contains("outputname: SOTF Audio Output"),
        "got: {}",
        resp
    );
    assert!(resp.contains("outputenabled: 1"), "got: {}", resp);
    assert!(resp.ends_with("OK\n"));

    let resp = session.exchange("stats");
    assert!(resp.contains("songs: 0"), "got: {}", resp);
    assert!(resp.contains("uptime: 0"), "got: {}", resp);
    assert!(resp.ends_with("OK\n"));
}

#[test]
fn test_currentsong_roundtrip() {
    let adapter: Arc<dyn PlayerAdapter> = Arc::new(MemoryAdapter::with_songs(vec![
        ("music/a.flac", "Alpha", "Artist A", 180.0),
        ("music/b.flac", "Beta", "Artist B", 200.0),
    ]));
    let session = TestSession::new(adapter);

    let resp = session.exchange("currentsong");
    assert_eq!(resp, "OK\n", "no current song when stopped");

    let resp = session.exchange("play 1");
    assert!(resp.contains("OK"));

    let resp = session.exchange("currentsong");
    assert!(resp.contains("file: music/b.flac"), "got: {}", resp);
    assert!(resp.contains("Title: Beta"), "got: {}", resp);
    assert!(resp.contains("Artist: Artist B"), "got: {}", resp);
    assert!(resp.contains("Pos: 1"), "got: {}", resp);
    assert!(resp.contains("Id: 2"), "got: {}", resp);
    assert!(resp.contains("duration: 200.000"), "got: {}", resp);
    assert!(resp.ends_with("OK\n"));
}

#[test]
fn test_playlistinfo_and_playlistid() {
    let adapter: Arc<dyn PlayerAdapter> = Arc::new(MemoryAdapter::with_songs(vec![
        ("music/a.flac", "Alpha", "Artist A", 180.0),
        ("music/b.flac", "Beta", "Artist B", 200.0),
        ("music/c.flac", "Gamma", "Artist C", 220.0),
    ]));
    let session = TestSession::new(adapter);

    let resp = session.exchange("playlistinfo");
    assert!(resp.contains("file: music/a.flac"));
    assert!(resp.contains("file: music/b.flac"));
    assert!(resp.contains("file: music/c.flac"));
    assert!(resp.contains("Pos: 0"));
    assert!(resp.contains("Pos: 1"));
    assert!(resp.contains("Pos: 2"));
    assert!(resp.ends_with("OK\n"));

    let resp = session.exchange("playlistinfo 1:3");
    assert!(!resp.contains("file: music/a.flac"));
    assert!(resp.contains("file: music/b.flac"));
    assert!(resp.contains("file: music/c.flac"));
    assert!(resp.ends_with("OK\n"));

    let resp = session.exchange("playlistid 2");
    assert!(resp.contains("file: music/b.flac"));
    assert!(resp.contains("Id: 2"));
    assert!(resp.ends_with("OK\n"));

    let resp = session.exchange("playlistid 999");
    assert!(
        resp.starts_with("ACK [50@"),
        "expected NoExist, got: {}",
        resp
    );
}

#[test]
fn test_seek_commands() {
    let adapter: Arc<dyn PlayerAdapter> = Arc::new(MemoryAdapter::with_songs(vec![
        ("music/a.flac", "Alpha", "Artist A", 180.0),
        ("music/b.flac", "Beta", "Artist B", 200.0),
    ]));
    let session = TestSession::new(adapter);

    session.exchange("play");
    session.exchange("seekcur 45.5");

    let resp = session.exchange("status");
    assert!(resp.contains("elapsed: 45.500"), "got: {}", resp);
    assert!(resp.contains("time: 45:240"), "got: {}", resp);

    let resp = session.exchange("seek 1 88.25");
    assert!(resp.contains("OK"));

    let resp = session.exchange("status");
    assert!(resp.contains("elapsed: 88.250"), "got: {}", resp);
    assert!(resp.contains("song: 1"), "got: {}", resp);
}

#[test]
fn test_queue_add_delete_clear() {
    let adapter: Arc<dyn PlayerAdapter> = Arc::new(MemoryAdapter::new());
    let session = TestSession::new(adapter);

    let resp = session.exchange("add \"music/first.flac\"");
    assert!(resp.contains("OK"));

    let resp = session.exchange("add \"music/second.flac\"");
    assert!(resp.contains("OK"));

    let resp = session.exchange("addid \"music/third.flac\" 1");
    assert!(resp.contains("Id: 3"), "got: {}", resp);

    let resp = session.exchange("playlistinfo");
    assert!(resp.contains("file: music/first.flac"));
    assert!(resp.contains("file: music/third.flac"));
    assert!(resp.contains("file: music/second.flac"));
    assert!(resp.contains("Pos: 0"));
    assert!(resp.contains("Pos: 1"));
    assert!(resp.contains("Pos: 2"));

    let resp = session.exchange("delete 1");
    assert!(resp.contains("OK"));

    let resp = session.exchange("playlistinfo");
    assert!(resp.contains("file: music/first.flac"));
    assert!(!resp.contains("file: music/third.flac"));
    assert!(resp.contains("file: music/second.flac"));

    let resp = session.exchange("clear");
    assert!(resp.contains("OK"));

    let resp = session.exchange("playlistinfo");
    assert_eq!(resp, "OK\n");
}

#[test]
fn test_volume_and_playback_options() {
    let adapter: Arc<dyn PlayerAdapter> = Arc::new(MemoryAdapter::new());
    let session = TestSession::new(adapter);

    let resp = session.exchange("setvol 42");
    assert!(resp.contains("OK"));

    let resp = session.exchange("volume -10");
    assert!(resp.contains("OK"));

    let resp = session.exchange("status");
    assert!(resp.contains("volume: 32"), "got: {}", resp);

    for cmd in ["random 1", "repeat 0", "single oneshot", "consume 1"] {
        let resp = session.exchange(cmd);
        assert!(resp.contains("OK"), "{} failed: {}", cmd, resp);
    }
}

#[test]
fn test_error_paths_visible_to_clients() {
    let adapter: Arc<dyn PlayerAdapter> = Arc::new(MemoryAdapter::new());
    let session = TestSession::new(adapter);

    let resp = session.exchange("not_a_real_command");
    assert!(
        resp.starts_with("ACK [5@"),
        "expected UnknownCmd, got: {}",
        resp
    );

    let resp = session.exchange("setvol 101");
    assert!(resp.starts_with("ACK [2@"), "expected Arg, got: {}", resp);

    let resp = session.exchange("play 0 extra");
    assert!(
        resp.starts_with("ACK [2@"),
        "expected Arg for trailing tokens, got: {}",
        resp
    );

    let resp = session.exchange("seek 3 10.0");
    assert!(
        resp.starts_with("ACK [52@"),
        "expected System error for out-of-range seek, got: {}",
        resp
    );
}

#[test]
fn test_reflection_commands() {
    let adapter: Arc<dyn PlayerAdapter> = Arc::new(MemoryAdapter::new());
    let session = TestSession::new(adapter);

    let resp = session.exchange("commands");
    assert!(resp.contains("command: play"));
    assert!(resp.contains("command: status"));
    assert!(resp.contains("command: currentsong"));
    assert!(resp.ends_with("OK\n"));

    let resp = session.exchange("tagtypes");
    assert!(resp.contains("tagtype: Artist"));
    assert!(resp.contains("tagtype: Album"));
    assert!(resp.ends_with("OK\n"));

    let resp = session.exchange("urlhandlers");
    assert!(resp.contains("handler: file://"));
    assert!(resp.ends_with("OK\n"));
}

#[test]
fn test_parse_and_handle_database_commands() {
    let adapter: Arc<dyn PlayerAdapter> = Arc::new(MemoryAdapter::with_songs(vec![
        ("music/a.flac", "Alpha", "Pink Floyd", 180.0),
        ("music/b.flac", "Beta", "Radiohead", 200.0),
        ("music/c.flac", "Gamma", "Pink Floyd", 220.0),
    ]));
    let session = TestSession::new(adapter);

    let resp = session.exchange("find artist \"Pink Floyd\"");
    assert!(resp.contains("file: music/a.flac"));
    assert!(resp.contains("file: music/c.flac"));
    assert!(!resp.contains("file: music/b.flac"));
    assert!(resp.ends_with("OK\n"));

    let resp = session.exchange("list artist");
    assert!(resp.contains("Artist: Pink Floyd"));
    assert!(resp.contains("Artist: Radiohead"));
    assert!(resp.ends_with("OK\n"));

    let resp = session.exchange("lsinfo");
    assert!(resp.contains("directory: Music"));
    assert!(resp.contains("file: Music/test.flac"));
    assert!(resp.ends_with("OK\n"));

    let resp = session.exchange("count artist \"Pink Floyd\"");
    assert!(resp.contains("songs: 2"), "got: {}", resp);
    assert!(resp.contains("playtime: 400"), "got: {}", resp);
}

#[test]
fn test_command_list_roundtrip() {
    let adapter: Arc<dyn PlayerAdapter> = Arc::new(MemoryAdapter::with_songs(vec![(
        "music/a.flac",
        "Alpha",
        "Artist",
        180.0,
    )]));
    let session = TestSession::new(adapter);

    let response = session.exchange_command_list(&["play", "status", "currentsong"], true);

    assert!(response.contains("list_OK"), "got: {}", response);
    assert!(response.contains("state: play"), "got: {}", response);
    assert!(response.contains("file: music/a.flac"), "got: {}", response);
    assert!(response.ends_with("OK\n"), "got: {}", response);
}
