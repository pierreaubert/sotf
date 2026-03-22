// ============================================================================
// MPD Command Handler
// ============================================================================
//
// Maps parsed MPD commands to PlayerAdapter trait calls.
// The trait is implemented by the daemon/app to bridge to the actual player.

use crate::protocol::*;

/// Playback state as seen by MPD clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MpdPlayState {
    Play,
    Pause,
    Stop,
}

impl std::fmt::Display for MpdPlayState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MpdPlayState::Play => write!(f, "play"),
            MpdPlayState::Pause => write!(f, "pause"),
            MpdPlayState::Stop => write!(f, "stop"),
        }
    }
}

/// Status snapshot returned by the adapter.
pub struct MpdStatus {
    pub volume: u8,        // 0-100
    pub repeat: bool,
    pub random: bool,
    pub single: bool,
    pub consume: bool,
    pub state: MpdPlayState,
    /// Current song position in playlist (0-indexed).
    pub song: Option<u32>,
    /// Current song ID.
    pub songid: Option<u32>,
    /// Elapsed time in seconds.
    pub elapsed: f64,
    /// Total duration in seconds.
    pub duration: f64,
    /// Audio format string: "samplerate:bits:channels"
    pub audio: Option<String>,
    /// Playlist length.
    pub playlist_length: u32,
    /// Playlist version (incremented on changes).
    pub playlist_version: u32,
}

/// Song metadata for MPD responses.
pub struct MpdSongInfo {
    pub file: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track: Option<u32>,
    pub date: Option<String>,
    pub genre: Option<String>,
    pub duration: Option<f64>,
    pub pos: u32,
    pub id: u32,
}

impl MpdSongInfo {
    pub fn to_kvs(&self) -> Vec<MpdKv> {
        let mut kvs = vec![kv("file", &self.file)];
        if let Some(ref t) = self.title {
            kvs.push(kv("Title", t));
        }
        if let Some(ref a) = self.artist {
            kvs.push(kv("Artist", a));
        }
        if let Some(ref a) = self.album {
            kvs.push(kv("Album", a));
        }
        if let Some(t) = self.track {
            kvs.push(kv("Track", t));
        }
        if let Some(ref d) = self.date {
            kvs.push(kv("Date", d));
        }
        if let Some(ref g) = self.genre {
            kvs.push(kv("Genre", g));
        }
        if let Some(d) = self.duration {
            kvs.push(kv("duration", format!("{:.3}", d)));
            // Also include Time for compatibility (integer seconds)
            kvs.push(kv("Time", d as u64));
        }
        kvs.push(kv("Pos", self.pos));
        kvs.push(kv("Id", self.id));
        kvs
    }
}

/// Directory entry for lsinfo.
pub struct MpdDirEntry {
    pub is_directory: bool,
    pub path: String,
}

/// Trait that bridges MPD commands to the actual SOTF player.
///
/// Implementors provide the glue between the MPD protocol and
/// the Player/QueueController/LibraryController APIs.
pub trait PlayerAdapter: Send + Sync + 'static {
    // Playback control
    fn play(&self, pos: Option<u32>) -> Result<(), String>;
    fn pause(&self, state: Option<bool>) -> Result<(), String>;
    fn stop(&self) -> Result<(), String>;
    fn next(&self) -> Result<(), String>;
    fn previous(&self) -> Result<(), String>;
    fn seek_pos(&self, song_pos: u32, time: f64) -> Result<(), String>;
    fn seek_cur(&self, time: f64) -> Result<(), String>;

    // Volume
    fn set_volume(&self, volume: u8) -> Result<(), String>;
    fn volume_change(&self, delta: i8) -> Result<(), String>;

    // Status
    fn status(&self) -> MpdStatus;
    fn current_song(&self) -> Option<MpdSongInfo>;

    // Queue
    fn playlist_info(&self, range: Option<(u32, Option<u32>)>) -> Vec<MpdSongInfo>;
    fn playlist_song_by_id(&self, id: u32) -> Option<MpdSongInfo>;
    fn add(&self, uri: &str) -> Result<(), String>;
    fn add_id(&self, uri: &str, pos: Option<u32>) -> Result<u32, String>;
    fn delete(&self, pos: u32) -> Result<(), String>;
    fn clear(&self) -> Result<(), String>;

    // Library
    fn search(&self, filters: &[FilterExpr], exact: bool) -> Vec<MpdSongInfo>;
    fn list_tag(&self, tag: &str, filters: &[FilterExpr]) -> Vec<String>;
    fn lsinfo(&self, path: Option<&str>) -> Vec<MpdDirEntry>;
}

/// Handle a single MPD command and produce a response.
pub fn handle_command(cmd: &MpdCommand, adapter: &dyn PlayerAdapter) -> MpdResponse {
    match cmd {
        // Connection
        MpdCommand::Ping => MpdResponse::ok(),
        MpdCommand::Close => MpdResponse::ok(), // handled at session level
        MpdCommand::Password(_) => MpdResponse::ok(), // no auth for now

        // Playback control
        MpdCommand::Play(pos) => match adapter.play(*pos) {
            Ok(()) => MpdResponse::ok(),
            Err(e) => MpdResponse::Error(MpdError::new(MpdErrorCode::System, "play", &e)),
        },
        MpdCommand::PlayId(id) => match adapter.play(*id) {
            Ok(()) => MpdResponse::ok(),
            Err(e) => MpdResponse::Error(MpdError::new(MpdErrorCode::System, "playid", &e)),
        },
        MpdCommand::Pause(state) => match adapter.pause(*state) {
            Ok(()) => MpdResponse::ok(),
            Err(e) => MpdResponse::Error(MpdError::new(MpdErrorCode::System, "pause", &e)),
        },
        MpdCommand::Stop => match adapter.stop() {
            Ok(()) => MpdResponse::ok(),
            Err(e) => MpdResponse::Error(MpdError::new(MpdErrorCode::System, "stop", &e)),
        },
        MpdCommand::Next => match adapter.next() {
            Ok(()) => MpdResponse::ok(),
            Err(e) => MpdResponse::Error(MpdError::new(MpdErrorCode::System, "next", &e)),
        },
        MpdCommand::Previous => match adapter.previous() {
            Ok(()) => MpdResponse::ok(),
            Err(e) => MpdResponse::Error(MpdError::new(MpdErrorCode::System, "previous", &e)),
        },
        MpdCommand::Seek(pos, time) => match adapter.seek_pos(*pos, *time) {
            Ok(()) => MpdResponse::ok(),
            Err(e) => MpdResponse::Error(MpdError::new(MpdErrorCode::System, "seek", &e)),
        },
        MpdCommand::SeekId(id, time) => match adapter.seek_pos(*id, *time) {
            Ok(()) => MpdResponse::ok(),
            Err(e) => MpdResponse::Error(MpdError::new(MpdErrorCode::System, "seekid", &e)),
        },
        MpdCommand::SeekCur(time) => match adapter.seek_cur(*time) {
            Ok(()) => MpdResponse::ok(),
            Err(e) => MpdResponse::Error(MpdError::new(MpdErrorCode::System, "seekcur", &e)),
        },

        // Volume
        MpdCommand::SetVol(vol) => match adapter.set_volume(*vol) {
            Ok(()) => MpdResponse::ok(),
            Err(e) => MpdResponse::Error(MpdError::new(MpdErrorCode::System, "setvol", &e)),
        },
        MpdCommand::Volume(delta) => match adapter.volume_change(*delta) {
            Ok(()) => MpdResponse::ok(),
            Err(e) => MpdResponse::Error(MpdError::new(MpdErrorCode::System, "volume", &e)),
        },

        // Playback options (accept but ignore for now)
        MpdCommand::Random(_)
        | MpdCommand::Repeat(_)
        | MpdCommand::Single(_)
        | MpdCommand::Consume(_) => MpdResponse::ok(),

        // Status
        MpdCommand::Status => {
            let s = adapter.status();
            let mut kvs = vec![
                kv("volume", s.volume),
                kv("repeat", if s.repeat { 1 } else { 0 }),
                kv("random", if s.random { 1 } else { 0 }),
                kv("single", if s.single { 1 } else { 0 }),
                kv("consume", if s.consume { 1 } else { 0 }),
                kv("playlist", s.playlist_version),
                kv("playlistlength", s.playlist_length),
                kv("state", s.state),
                kv("xfade", 0),
                kv("mixrampdb", "0.000000"),
                kv("mixrampdelay", "nan"),
            ];
            if let Some(song) = s.song {
                kvs.push(kv("song", song));
            }
            if let Some(songid) = s.songid {
                kvs.push(kv("songid", songid));
            }
            if s.state != MpdPlayState::Stop {
                kvs.push(kv("elapsed", format!("{:.3}", s.elapsed)));
                kvs.push(kv("duration", format!("{:.3}", s.duration)));
                // Legacy time field: "elapsed:total" as integers
                kvs.push(kv(
                    "time",
                    format!("{}:{}", s.elapsed as u64, s.duration as u64),
                ));
            }
            if let Some(ref audio) = s.audio {
                kvs.push(kv("audio", audio));
            }
            MpdResponse::ok_with(kvs)
        }
        MpdCommand::Stats => {
            // Minimal stats — real values would come from database
            MpdResponse::ok_with(vec![
                kv("uptime", 0),
                kv("playtime", 0),
                kv("artists", 0),
                kv("albums", 0),
                kv("songs", 0),
                kv("db_playtime", 0),
                kv("db_update", 0),
            ])
        }
        MpdCommand::CurrentSong => match adapter.current_song() {
            Some(song) => MpdResponse::ok_with(song.to_kvs()),
            None => MpdResponse::ok(),
        },

        // Queue
        MpdCommand::PlaylistInfo(range) => {
            let songs = adapter.playlist_info(*range);
            let mut kvs = Vec::new();
            for song in songs {
                kvs.extend(song.to_kvs());
            }
            MpdResponse::ok_with(kvs)
        }
        MpdCommand::PlaylistId(id) => {
            if let Some(id) = id {
                match adapter.playlist_song_by_id(*id) {
                    Some(song) => MpdResponse::ok_with(song.to_kvs()),
                    None => MpdResponse::Error(MpdError::new(
                        MpdErrorCode::NoExist,
                        "playlistid",
                        &format!("No such song with id: {}", id),
                    )),
                }
            } else {
                let songs = adapter.playlist_info(None);
                let mut kvs = Vec::new();
                for song in songs {
                    kvs.extend(song.to_kvs());
                }
                MpdResponse::ok_with(kvs)
            }
        }
        MpdCommand::Add(uri) => match adapter.add(uri) {
            Ok(()) => MpdResponse::ok(),
            Err(e) => MpdResponse::Error(MpdError::new(MpdErrorCode::NoExist, "add", &e)),
        },
        MpdCommand::AddId(uri, pos) => match adapter.add_id(uri, *pos) {
            Ok(id) => MpdResponse::ok_with(vec![kv("Id", id)]),
            Err(e) => MpdResponse::Error(MpdError::new(MpdErrorCode::NoExist, "addid", &e)),
        },
        MpdCommand::Delete(pos) => match adapter.delete(*pos) {
            Ok(()) => MpdResponse::ok(),
            Err(e) => MpdResponse::Error(MpdError::new(MpdErrorCode::Arg, "delete", &e)),
        },
        MpdCommand::DeleteId(id) => match adapter.delete(*id) {
            Ok(()) => MpdResponse::ok(),
            Err(e) => MpdResponse::Error(MpdError::new(MpdErrorCode::Arg, "deleteid", &e)),
        },
        MpdCommand::Clear => match adapter.clear() {
            Ok(()) => MpdResponse::ok(),
            Err(e) => MpdResponse::Error(MpdError::new(MpdErrorCode::System, "clear", &e)),
        },
        MpdCommand::Shuffle | MpdCommand::Move(_, _) | MpdCommand::Swap(_, _) => {
            // Accept but don't implement shuffle/move/swap yet
            MpdResponse::ok()
        }

        // Database
        MpdCommand::Find(filters) => {
            let songs = adapter.search(filters, true);
            let mut kvs = Vec::new();
            for song in songs {
                kvs.extend(song.to_kvs());
            }
            MpdResponse::ok_with(kvs)
        }
        MpdCommand::Search(filters) => {
            let songs = adapter.search(filters, false);
            let mut kvs = Vec::new();
            for song in songs {
                kvs.extend(song.to_kvs());
            }
            MpdResponse::ok_with(kvs)
        }
        MpdCommand::List(tag, filters) => {
            let values = adapter.list_tag(tag, filters);
            let mut kvs = Vec::new();
            // Capitalize the tag name for the response
            let tag_cap = capitalize_first(tag);
            for v in values {
                kvs.push(kv(&tag_cap, &v));
            }
            MpdResponse::ok_with(kvs)
        }
        MpdCommand::Count(filters) => {
            let songs = adapter.search(filters, false);
            let total_time: f64 = songs
                .iter()
                .filter_map(|s| s.duration)
                .sum();
            MpdResponse::ok_with(vec![
                kv("songs", songs.len()),
                kv("playtime", total_time as u64),
            ])
        }
        MpdCommand::ListAll(path) => {
            let entries = adapter.lsinfo(path.as_deref());
            let mut kvs = Vec::new();
            for entry in entries {
                if entry.is_directory {
                    kvs.push(kv("directory", &entry.path));
                } else {
                    kvs.push(kv("file", &entry.path));
                }
            }
            MpdResponse::ok_with(kvs)
        }
        MpdCommand::LsInfo(path) => {
            let entries = adapter.lsinfo(path.as_deref());
            let mut kvs = Vec::new();
            for entry in entries {
                if entry.is_directory {
                    kvs.push(kv("directory", &entry.path));
                } else {
                    kvs.push(kv("file", &entry.path));
                }
            }
            MpdResponse::ok_with(kvs)
        }
        MpdCommand::Update(_) => {
            // Accept but library scanning is separate
            MpdResponse::ok_with(vec![kv("updating_db", 1)])
        }

        // Outputs
        MpdCommand::Outputs => MpdResponse::ok_with(vec![
            kv("outputid", 0),
            kv("outputname", "SOTF Audio Output"),
            kv("plugin", "cpal"),
            kv("outputenabled", 1),
        ]),
        MpdCommand::EnableOutput(_)
        | MpdCommand::DisableOutput(_)
        | MpdCommand::ToggleOutput(_) => MpdResponse::ok(),

        // Reflection
        MpdCommand::Commands => {
            let cmds = [
                "add", "addid", "clear", "close", "commands", "consume",
                "count", "currentsong", "delete", "deleteid", "decoders",
                "disableoutput", "enableoutput", "find", "idle", "list",
                "listall", "lsinfo", "next", "noidle", "notcommands",
                "outputs", "pause", "ping", "play", "playid", "playlistid",
                "playlistinfo", "previous", "random", "repeat", "search",
                "seek", "seekcur", "seekid", "setvol", "shuffle", "single",
                "stats", "status", "stop", "swap", "tagtypes",
                "toggleoutput", "update", "urlhandlers", "volume",
            ];
            let kvs = cmds.iter().map(|c| kv("command", c)).collect();
            MpdResponse::ok_with(kvs)
        }
        MpdCommand::NotCommands => MpdResponse::ok(),
        MpdCommand::TagTypes => {
            let tags = [
                "Artist", "Album", "Title", "Track", "Genre", "Date",
                "Composer", "Disc", "AlbumArtist",
            ];
            let kvs = tags.iter().map(|t| kv("tagtype", t)).collect();
            MpdResponse::ok_with(kvs)
        }
        MpdCommand::UrlHandlers => {
            MpdResponse::ok_with(vec![kv("handler", "file://")])
        }
        MpdCommand::Decoders => {
            let mut kvs = Vec::new();
            for (name, suffixes, mime) in [
                ("flac", "flac", "audio/flac"),
                ("mp3", "mp3", "audio/mpeg"),
                ("aac", "aac m4a mp4", "audio/aac"),
                ("vorbis", "ogg oga", "audio/ogg"),
                ("wav", "wav", "audio/wav"),
                ("aiff", "aiff aif", "audio/aiff"),
            ] {
                kvs.push(kv("plugin", name));
                for s in suffixes.split(' ') {
                    kvs.push(kv("suffix", s));
                }
                kvs.push(kv("mime_type", mime));
            }
            MpdResponse::ok_with(kvs)
        }

        // Command lists — handled at session level, not here
        MpdCommand::CommandListBegin
        | MpdCommand::CommandListOkBegin
        | MpdCommand::CommandListEnd => MpdResponse::ok(),

        // Idle — handled at session level
        MpdCommand::Idle(_) | MpdCommand::NoIdle => MpdResponse::ok(),
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal test adapter that does nothing.
    struct DummyAdapter;

    impl PlayerAdapter for DummyAdapter {
        fn play(&self, _pos: Option<u32>) -> Result<(), String> { Ok(()) }
        fn pause(&self, _state: Option<bool>) -> Result<(), String> { Ok(()) }
        fn stop(&self) -> Result<(), String> { Ok(()) }
        fn next(&self) -> Result<(), String> { Ok(()) }
        fn previous(&self) -> Result<(), String> { Ok(()) }
        fn seek_pos(&self, _pos: u32, _time: f64) -> Result<(), String> { Ok(()) }
        fn seek_cur(&self, _time: f64) -> Result<(), String> { Ok(()) }
        fn set_volume(&self, _vol: u8) -> Result<(), String> { Ok(()) }
        fn volume_change(&self, _delta: i8) -> Result<(), String> { Ok(()) }
        fn status(&self) -> MpdStatus {
            MpdStatus {
                volume: 75,
                repeat: false,
                random: false,
                single: false,
                consume: false,
                state: MpdPlayState::Stop,
                song: None,
                songid: None,
                elapsed: 0.0,
                duration: 0.0,
                audio: None,
                playlist_length: 0,
                playlist_version: 1,
            }
        }
        fn current_song(&self) -> Option<MpdSongInfo> { None }
        fn playlist_info(&self, _range: Option<(u32, Option<u32>)>) -> Vec<MpdSongInfo> { vec![] }
        fn playlist_song_by_id(&self, _id: u32) -> Option<MpdSongInfo> { None }
        fn add(&self, _uri: &str) -> Result<(), String> { Ok(()) }
        fn add_id(&self, _uri: &str, _pos: Option<u32>) -> Result<u32, String> { Ok(0) }
        fn delete(&self, _pos: u32) -> Result<(), String> { Ok(()) }
        fn clear(&self) -> Result<(), String> { Ok(()) }
        fn search(&self, _filters: &[FilterExpr], _exact: bool) -> Vec<MpdSongInfo> { vec![] }
        fn list_tag(&self, _tag: &str, _filters: &[FilterExpr]) -> Vec<String> { vec![] }
        fn lsinfo(&self, _path: Option<&str>) -> Vec<MpdDirEntry> { vec![] }
    }

    #[test]
    fn test_handle_ping() {
        let adapter = DummyAdapter;
        let resp = handle_command(&MpdCommand::Ping, &adapter);
        assert!(resp.format().contains("OK"));
    }

    #[test]
    fn test_handle_status() {
        let adapter = DummyAdapter;
        let resp = handle_command(&MpdCommand::Status, &adapter);
        let out = resp.format();
        assert!(out.contains("volume: 75"));
        assert!(out.contains("state: stop"));
        assert!(out.contains("playlistlength: 0"));
        assert!(out.ends_with("OK\n"));
    }

    #[test]
    fn test_handle_commands() {
        let adapter = DummyAdapter;
        let resp = handle_command(&MpdCommand::Commands, &adapter);
        let out = resp.format();
        assert!(out.contains("command: play"));
        assert!(out.contains("command: status"));
        assert!(out.contains("command: search"));
    }

    #[test]
    fn test_handle_tagtypes() {
        let adapter = DummyAdapter;
        let resp = handle_command(&MpdCommand::TagTypes, &adapter);
        let out = resp.format();
        assert!(out.contains("tagtype: Artist"));
        assert!(out.contains("tagtype: Album"));
    }

    #[test]
    fn test_handle_outputs() {
        let adapter = DummyAdapter;
        let resp = handle_command(&MpdCommand::Outputs, &adapter);
        let out = resp.format();
        assert!(out.contains("outputname: SOTF Audio Output"));
        assert!(out.contains("outputenabled: 1"));
    }

    #[test]
    fn test_handle_decoders() {
        let adapter = DummyAdapter;
        let resp = handle_command(&MpdCommand::Decoders, &adapter);
        let out = resp.format();
        assert!(out.contains("plugin: flac"));
        assert!(out.contains("plugin: mp3"));
        assert!(out.contains("suffix: ogg"));
    }
}
