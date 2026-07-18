use super::mpd_play_state::MpdPlayState;
use super::mpd_play_state::handle_command;
use super::mpd_song_info::MpdSongInfo;
use super::player_adapter::PlayerAdapter;
use super::types::MpdDirEntry;
use super::types::MpdStatus;
use crate::protocol::*;

/// Minimal test adapter that does nothing.
struct DummyAdapter;

impl PlayerAdapter for DummyAdapter {
    fn play(&self, _pos: Option<u32>) -> Result<(), String> {
        Ok(())
    }
    fn pause(&self, _state: Option<bool>) -> Result<(), String> {
        Ok(())
    }
    fn stop(&self) -> Result<(), String> {
        Ok(())
    }
    fn next(&self) -> Result<(), String> {
        Ok(())
    }
    fn previous(&self) -> Result<(), String> {
        Ok(())
    }
    fn seek_pos(&self, _pos: u32, _time: f64) -> Result<(), String> {
        Ok(())
    }
    fn seek_cur(&self, _time: f64) -> Result<(), String> {
        Ok(())
    }
    fn set_volume(&self, _vol: u8) -> Result<(), String> {
        Ok(())
    }
    fn volume_change(&self, _delta: i8) -> Result<(), String> {
        Ok(())
    }
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
    fn current_song(&self) -> Option<MpdSongInfo> {
        None
    }
    fn playlist_info(&self, _range: Option<(u32, Option<u32>)>) -> Vec<MpdSongInfo> {
        vec![]
    }
    fn playlist_song_by_id(&self, _id: u32) -> Option<MpdSongInfo> {
        None
    }
    fn add(&self, _uri: &str) -> Result<(), String> {
        Ok(())
    }
    fn add_id(&self, _uri: &str, _pos: Option<u32>) -> Result<u32, String> {
        Ok(0)
    }
    fn delete(&self, _pos: u32) -> Result<(), String> {
        Ok(())
    }
    fn clear(&self) -> Result<(), String> {
        Ok(())
    }
    fn search(&self, _filters: &[FilterExpr], _exact: bool) -> Vec<MpdSongInfo> {
        vec![]
    }
    fn list_tag(&self, _tag: &str, _filters: &[FilterExpr]) -> Vec<String> {
        vec![]
    }
    fn lsinfo(&self, _path: Option<&str>) -> Vec<MpdDirEntry> {
        vec![]
    }
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

// ===== Regression: playid / seekid / deleteid route through ID, not pos =====
//
// MPD ids are explicitly stable across queue mutations; positions are not.
// The previous implementation passed the id straight into the position-based
// adapter calls, so `playid 17` on a reordered queue tried to play
// *position 17* instead of the song that ever had id 17. These tests
// install an adapter where pos and id are guaranteed *not* to coincide
// and verify each id-keyed command asks for the right position.

use std::sync::Mutex;

struct IdRoutingAdapter {
    /// Map id → pos. Deliberately chosen so pos and id are distinct.
    id_to_pos: std::collections::HashMap<u32, u32>,
    last_play_pos: Mutex<Option<u32>>,
    last_seek_pos: Mutex<Option<(u32, f64)>>,
    last_delete_pos: Mutex<Option<u32>>,
}

impl PlayerAdapter for IdRoutingAdapter {
    fn play(&self, pos: Option<u32>) -> Result<(), String> {
        *self.last_play_pos.lock().unwrap() = pos;
        Ok(())
    }
    fn pause(&self, _: Option<bool>) -> Result<(), String> {
        Ok(())
    }
    fn stop(&self) -> Result<(), String> {
        Ok(())
    }
    fn next(&self) -> Result<(), String> {
        Ok(())
    }
    fn previous(&self) -> Result<(), String> {
        Ok(())
    }
    fn seek_pos(&self, pos: u32, time: f64) -> Result<(), String> {
        *self.last_seek_pos.lock().unwrap() = Some((pos, time));
        Ok(())
    }
    fn seek_cur(&self, _: f64) -> Result<(), String> {
        Ok(())
    }
    fn set_volume(&self, _: u8) -> Result<(), String> {
        Ok(())
    }
    fn volume_change(&self, _: i8) -> Result<(), String> {
        Ok(())
    }
    fn status(&self) -> MpdStatus {
        MpdStatus {
            volume: 0,
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
            playlist_version: 0,
        }
    }
    fn current_song(&self) -> Option<MpdSongInfo> {
        None
    }
    fn playlist_info(&self, _: Option<(u32, Option<u32>)>) -> Vec<MpdSongInfo> {
        vec![]
    }
    fn playlist_song_by_id(&self, id: u32) -> Option<MpdSongInfo> {
        self.id_to_pos.get(&id).map(|&pos| MpdSongInfo {
            file: format!("track-{id}.flac"),
            title: None,
            artist: None,
            album: None,
            track: None,
            date: None,
            genre: None,
            duration: None,
            pos,
            id,
        })
    }
    fn add(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn add_id(&self, _: &str, _: Option<u32>) -> Result<u32, String> {
        Ok(0)
    }
    fn delete(&self, pos: u32) -> Result<(), String> {
        *self.last_delete_pos.lock().unwrap() = Some(pos);
        Ok(())
    }
    fn clear(&self) -> Result<(), String> {
        Ok(())
    }
    fn search(&self, _: &[FilterExpr], _: bool) -> Vec<MpdSongInfo> {
        vec![]
    }
    fn list_tag(&self, _: &str, _: &[FilterExpr]) -> Vec<String> {
        vec![]
    }
    fn lsinfo(&self, _: Option<&str>) -> Vec<MpdDirEntry> {
        vec![]
    }
}

fn id_routing_adapter() -> IdRoutingAdapter {
    // ID 17 lives at position 3, ID 42 lives at position 0 — the
    // canonical "id mismatches pos" arrangement after any queue reorder.
    let mut map = std::collections::HashMap::new();
    map.insert(17u32, 3u32);
    map.insert(42u32, 0u32);
    IdRoutingAdapter {
        id_to_pos: map,
        last_play_pos: Mutex::new(None),
        last_seek_pos: Mutex::new(None),
        last_delete_pos: Mutex::new(None),
    }
}

#[test]
fn test_playid_routes_by_id_not_position() {
    // `playid 17` must invoke play(Some(3)), NOT play(Some(17)).
    let adapter = id_routing_adapter();
    let resp = handle_command(&MpdCommand::PlayId(Some(17)), &adapter);
    assert!(resp.format().contains("OK"));
    assert_eq!(*adapter.last_play_pos.lock().unwrap(), Some(3));
}

#[test]
fn test_seekid_routes_by_id_not_position() {
    let adapter = id_routing_adapter();
    let resp = handle_command(&MpdCommand::SeekId(42, 12.5), &adapter);
    assert!(resp.format().contains("OK"));
    assert_eq!(*adapter.last_seek_pos.lock().unwrap(), Some((0, 12.5)));
}

#[test]
fn test_deleteid_routes_by_id_not_position() {
    let adapter = id_routing_adapter();
    let resp = handle_command(&MpdCommand::DeleteId(17), &adapter);
    assert!(resp.format().contains("OK"));
    assert_eq!(*adapter.last_delete_pos.lock().unwrap(), Some(3));
}

#[test]
fn test_playid_unknown_returns_no_exist() {
    // An unknown id must produce ACK [50] (NoExist), not corrupt state.
    let adapter = id_routing_adapter();
    let resp = handle_command(&MpdCommand::PlayId(Some(9999)), &adapter);
    let out = resp.format();
    assert!(
        out.starts_with("ACK [50@"),
        "expected ACK [50@...], got {out}"
    );
    assert!(adapter.last_play_pos.lock().unwrap().is_none());
}

#[test]
fn test_deleteid_unknown_returns_no_exist() {
    let adapter = id_routing_adapter();
    let resp = handle_command(&MpdCommand::DeleteId(9999), &adapter);
    assert!(resp.format().starts_with("ACK [50@"));
    assert!(adapter.last_delete_pos.lock().unwrap().is_none());
}

// ===== Regression: pause polarity matches the MPD spec =====

struct PausePolarityAdapter {
    last: Mutex<Option<Option<bool>>>,
}

impl PlayerAdapter for PausePolarityAdapter {
    fn play(&self, _: Option<u32>) -> Result<(), String> {
        Ok(())
    }
    fn pause(&self, state: Option<bool>) -> Result<(), String> {
        *self.last.lock().unwrap() = Some(state);
        Ok(())
    }
    fn stop(&self) -> Result<(), String> {
        Ok(())
    }
    fn next(&self) -> Result<(), String> {
        Ok(())
    }
    fn previous(&self) -> Result<(), String> {
        Ok(())
    }
    fn seek_pos(&self, _: u32, _: f64) -> Result<(), String> {
        Ok(())
    }
    fn seek_cur(&self, _: f64) -> Result<(), String> {
        Ok(())
    }
    fn set_volume(&self, _: u8) -> Result<(), String> {
        Ok(())
    }
    fn volume_change(&self, _: i8) -> Result<(), String> {
        Ok(())
    }
    fn status(&self) -> MpdStatus {
        MpdStatus {
            volume: 0,
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
            playlist_version: 0,
        }
    }
    fn current_song(&self) -> Option<MpdSongInfo> {
        None
    }
    fn playlist_info(&self, _: Option<(u32, Option<u32>)>) -> Vec<MpdSongInfo> {
        vec![]
    }
    fn playlist_song_by_id(&self, _: u32) -> Option<MpdSongInfo> {
        None
    }
    fn add(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn add_id(&self, _: &str, _: Option<u32>) -> Result<u32, String> {
        Ok(0)
    }
    fn delete(&self, _: u32) -> Result<(), String> {
        Ok(())
    }
    fn clear(&self) -> Result<(), String> {
        Ok(())
    }
    fn search(&self, _: &[FilterExpr], _: bool) -> Vec<MpdSongInfo> {
        vec![]
    }
    fn list_tag(&self, _: &str, _: &[FilterExpr]) -> Vec<String> {
        vec![]
    }
    fn lsinfo(&self, _: Option<&str>) -> Vec<MpdDirEntry> {
        vec![]
    }
}

#[test]
fn test_pause_polarity_dispatched_unchanged() {
    // The handler must forward the boolean it received from the parser
    // verbatim. Per spec: `pause 1` → pause, `pause 0` → resume,
    // `pause` (no arg) → toggle. Flipping the polarity in either layer
    // would invert the meaning for the adapter.
    let adapter = PausePolarityAdapter {
        last: Mutex::new(None),
    };

    let _ = handle_command(&MpdCommand::Pause(Some(true)), &adapter);
    assert_eq!(*adapter.last.lock().unwrap(), Some(Some(true)));

    let _ = handle_command(&MpdCommand::Pause(Some(false)), &adapter);
    assert_eq!(*adapter.last.lock().unwrap(), Some(Some(false)));

    let _ = handle_command(&MpdCommand::Pause(None), &adapter);
    assert_eq!(*adapter.last.lock().unwrap(), Some(None));
}

// ============================================================================
// Configurable adapter for broad handler coverage
// ============================================================================

struct ConfigurableAdapter {
    play_result: Result<(), String>,
    pause_result: Result<(), String>,
    stop_result: Result<(), String>,
    next_result: Result<(), String>,
    previous_result: Result<(), String>,
    seek_pos_result: Result<(), String>,
    seek_cur_result: Result<(), String>,
    set_volume_result: Result<(), String>,
    volume_change_result: Result<(), String>,
    add_result: Result<(), String>,
    add_id_result: Result<u32, String>,
    delete_result: Result<(), String>,
    clear_result: Result<(), String>,

    status_val: MpdStatus,
    current_song_val: Mutex<Option<MpdSongInfo>>,
    playlist_songs_val: Mutex<Vec<MpdSongInfo>>,
    playlist_song_by_id_val: Mutex<std::collections::HashMap<u32, MpdSongInfo>>,
    search_results_val: Mutex<Vec<MpdSongInfo>>,
    list_tag_results_val: Vec<String>,
    lsinfo_results_val: Mutex<Vec<MpdDirEntry>>,

    last_play_pos: Mutex<Option<u32>>,
    last_pause_state: Mutex<Option<Option<bool>>>,
    last_seek_pos: Mutex<Option<(u32, f64)>>,
    last_seek_cur: Mutex<Option<f64>>,
    last_set_volume: Mutex<Option<u8>>,
    last_volume_delta: Mutex<Option<i8>>,
    last_add_uri: Mutex<Option<String>>,
    last_add_id_uri: Mutex<Option<String>>,
    last_add_id_pos: Mutex<Option<Option<u32>>>,
    last_delete_pos: Mutex<Option<u32>>,
}

impl ConfigurableAdapter {
    fn new_ok() -> Self {
        Self {
            play_result: Ok(()),
            pause_result: Ok(()),
            stop_result: Ok(()),
            next_result: Ok(()),
            previous_result: Ok(()),
            seek_pos_result: Ok(()),
            seek_cur_result: Ok(()),
            set_volume_result: Ok(()),
            volume_change_result: Ok(()),
            add_result: Ok(()),
            add_id_result: Ok(1),
            delete_result: Ok(()),
            clear_result: Ok(()),
            status_val: MpdStatus {
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
            },
            current_song_val: Mutex::new(None),
            playlist_songs_val: Mutex::new(vec![]),
            playlist_song_by_id_val: Mutex::new(std::collections::HashMap::new()),
            search_results_val: Mutex::new(vec![]),
            list_tag_results_val: vec![],
            lsinfo_results_val: Mutex::new(vec![]),
            last_play_pos: Mutex::new(None),
            last_pause_state: Mutex::new(None),
            last_seek_pos: Mutex::new(None),
            last_seek_cur: Mutex::new(None),
            last_set_volume: Mutex::new(None),
            last_volume_delta: Mutex::new(None),
            last_add_uri: Mutex::new(None),
            last_add_id_uri: Mutex::new(None),
            last_add_id_pos: Mutex::new(None),
            last_delete_pos: Mutex::new(None),
        }
    }

    fn new_err(msg: &str) -> Self {
        let err = msg.to_string();
        Self {
            play_result: Err(err.clone()),
            pause_result: Err(err.clone()),
            stop_result: Err(err.clone()),
            next_result: Err(err.clone()),
            previous_result: Err(err.clone()),
            seek_pos_result: Err(err.clone()),
            seek_cur_result: Err(err.clone()),
            set_volume_result: Err(err.clone()),
            volume_change_result: Err(err.clone()),
            add_result: Err(err.clone()),
            add_id_result: Err(err.clone()),
            delete_result: Err(err.clone()),
            clear_result: Err(err.clone()),
            status_val: MpdStatus {
                volume: 0,
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
                playlist_version: 0,
            },
            current_song_val: Mutex::new(None),
            playlist_songs_val: Mutex::new(vec![]),
            playlist_song_by_id_val: Mutex::new(std::collections::HashMap::new()),
            search_results_val: Mutex::new(vec![]),
            list_tag_results_val: vec![],
            lsinfo_results_val: Mutex::new(vec![]),
            last_play_pos: Mutex::new(None),
            last_pause_state: Mutex::new(None),
            last_seek_pos: Mutex::new(None),
            last_seek_cur: Mutex::new(None),
            last_set_volume: Mutex::new(None),
            last_volume_delta: Mutex::new(None),
            last_add_uri: Mutex::new(None),
            last_add_id_uri: Mutex::new(None),
            last_add_id_pos: Mutex::new(None),
            last_delete_pos: Mutex::new(None),
        }
    }
}

impl PlayerAdapter for ConfigurableAdapter {
    fn play(&self, pos: Option<u32>) -> Result<(), String> {
        *self.last_play_pos.lock().unwrap() = pos;
        self.play_result.clone()
    }
    fn pause(&self, state: Option<bool>) -> Result<(), String> {
        *self.last_pause_state.lock().unwrap() = Some(state);
        self.pause_result.clone()
    }
    fn stop(&self) -> Result<(), String> {
        self.stop_result.clone()
    }
    fn next(&self) -> Result<(), String> {
        self.next_result.clone()
    }
    fn previous(&self) -> Result<(), String> {
        self.previous_result.clone()
    }
    fn seek_pos(&self, pos: u32, time: f64) -> Result<(), String> {
        *self.last_seek_pos.lock().unwrap() = Some((pos, time));
        self.seek_pos_result.clone()
    }
    fn seek_cur(&self, time: f64) -> Result<(), String> {
        *self.last_seek_cur.lock().unwrap() = Some(time);
        self.seek_cur_result.clone()
    }
    fn set_volume(&self, vol: u8) -> Result<(), String> {
        *self.last_set_volume.lock().unwrap() = Some(vol);
        self.set_volume_result.clone()
    }
    fn volume_change(&self, delta: i8) -> Result<(), String> {
        *self.last_volume_delta.lock().unwrap() = Some(delta);
        self.volume_change_result.clone()
    }
    fn status(&self) -> MpdStatus {
        MpdStatus {
            volume: self.status_val.volume,
            repeat: self.status_val.repeat,
            random: self.status_val.random,
            single: self.status_val.single,
            consume: self.status_val.consume,
            state: self.status_val.state,
            song: self.status_val.song,
            songid: self.status_val.songid,
            elapsed: self.status_val.elapsed,
            duration: self.status_val.duration,
            audio: self.status_val.audio.clone(),
            playlist_length: self.status_val.playlist_length,
            playlist_version: self.status_val.playlist_version,
        }
    }
    fn current_song(&self) -> Option<MpdSongInfo> {
        self.current_song_val.lock().unwrap().take()
    }
    fn playlist_info(&self, _range: Option<(u32, Option<u32>)>) -> Vec<MpdSongInfo> {
        std::mem::take(&mut *self.playlist_songs_val.lock().unwrap())
    }
    fn playlist_song_by_id(&self, id: u32) -> Option<MpdSongInfo> {
        self.playlist_song_by_id_val.lock().unwrap().remove(&id)
    }
    fn add(&self, uri: &str) -> Result<(), String> {
        *self.last_add_uri.lock().unwrap() = Some(uri.to_string());
        self.add_result.clone()
    }
    fn add_id(&self, uri: &str, pos: Option<u32>) -> Result<u32, String> {
        *self.last_add_id_uri.lock().unwrap() = Some(uri.to_string());
        *self.last_add_id_pos.lock().unwrap() = Some(pos);
        self.add_id_result.clone()
    }
    fn delete(&self, pos: u32) -> Result<(), String> {
        *self.last_delete_pos.lock().unwrap() = Some(pos);
        self.delete_result.clone()
    }
    fn clear(&self) -> Result<(), String> {
        self.clear_result.clone()
    }
    fn search(&self, _filters: &[FilterExpr], _exact: bool) -> Vec<MpdSongInfo> {
        std::mem::take(&mut *self.search_results_val.lock().unwrap())
    }
    fn list_tag(&self, _tag: &str, _filters: &[FilterExpr]) -> Vec<String> {
        self.list_tag_results_val.clone()
    }
    fn lsinfo(&self, _path: Option<&str>) -> Vec<MpdDirEntry> {
        std::mem::take(&mut *self.lsinfo_results_val.lock().unwrap())
    }
}

// ============================================================================
// Handler tests: simple ok commands
// ============================================================================

fn assert_ok_response(label: &str, resp: MpdResponse) -> String {
    let out = resp.format();
    assert!(out.contains("OK"), "{label}: expected OK, got {out}");
    out
}

#[test]
fn test_handle_stateless_ok_commands() {
    let cases = [
        ("close", MpdCommand::Close),
        ("password", MpdCommand::Password("secret".into())),
        ("stop", MpdCommand::Stop),
        ("next", MpdCommand::Next),
        ("previous", MpdCommand::Previous),
        ("random", MpdCommand::Random(true)),
        ("repeat", MpdCommand::Repeat(false)),
        ("single", MpdCommand::Single(SingleMode::OneShot)),
        ("consume", MpdCommand::Consume(true)),
        ("shuffle", MpdCommand::Shuffle),
        ("move", MpdCommand::Move(1, 2)),
        ("swap", MpdCommand::Swap(0, 1)),
        ("enableoutput", MpdCommand::EnableOutput(0)),
        ("disableoutput", MpdCommand::DisableOutput(0)),
        ("toggleoutput", MpdCommand::ToggleOutput(0)),
        ("command_list_begin", MpdCommand::CommandListBegin),
        ("command_list_ok_begin", MpdCommand::CommandListOkBegin),
        ("command_list_end", MpdCommand::CommandListEnd),
        ("idle", MpdCommand::Idle(vec!["player".into()])),
        ("noidle", MpdCommand::NoIdle),
        ("notcommands", MpdCommand::NotCommands),
    ];

    for (label, command) in cases {
        let adapter = ConfigurableAdapter::new_ok();
        assert_ok_response(label, handle_command(&command, &adapter));
    }
}

#[test]
fn test_handle_stateful_ok_commands_record_adapter_calls() {
    let adapter = ConfigurableAdapter::new_ok();
    let resp = handle_command(&MpdCommand::Play(Some(3)), &adapter);
    assert_ok_response("play with position", resp);
    assert_eq!(*adapter.last_play_pos.lock().unwrap(), Some(3));

    let adapter = ConfigurableAdapter::new_ok();
    let resp = handle_command(&MpdCommand::Play(None), &adapter);
    assert_ok_response("play without position", resp);
    assert_eq!(*adapter.last_play_pos.lock().unwrap(), None);

    let adapter = ConfigurableAdapter::new_ok();
    let resp = handle_command(&MpdCommand::Seek(2, 123.45), &adapter);
    assert_ok_response("seek", resp);
    assert_eq!(*adapter.last_seek_pos.lock().unwrap(), Some((2, 123.45)));

    let adapter = ConfigurableAdapter::new_ok();
    let resp = handle_command(&MpdCommand::SeekCur(30.0), &adapter);
    assert_ok_response("seekcur", resp);
    assert_eq!(*adapter.last_seek_cur.lock().unwrap(), Some(30.0));

    let adapter = ConfigurableAdapter::new_ok();
    let resp = handle_command(&MpdCommand::SetVol(80), &adapter);
    assert_ok_response("setvol", resp);
    assert_eq!(*adapter.last_set_volume.lock().unwrap(), Some(80));

    let adapter = ConfigurableAdapter::new_ok();
    let resp = handle_command(&MpdCommand::Volume(-5), &adapter);
    assert_ok_response("volume", resp);
    assert_eq!(*adapter.last_volume_delta.lock().unwrap(), Some(-5));
}

#[test]
fn test_handle_urlhandlers_ok_lists_file_handler() {
    let adapter = ConfigurableAdapter::new_ok();
    let resp = handle_command(&MpdCommand::UrlHandlers, &adapter);
    let out = assert_ok_response("urlhandlers", resp);
    assert!(out.contains("handler: file://"));
}

#[test]
fn test_handle_update_ok() {
    let adapter = ConfigurableAdapter::new_ok();
    let resp = handle_command(&MpdCommand::Update(Some("Music".into())), &adapter);
    let out = resp.format();
    assert!(out.contains("updating_db: 1"));
    assert!(out.contains("OK"));
}

// ============================================================================
// Handler tests: adapter error paths
// ============================================================================

#[test]
fn test_handle_adapter_error_mapping() {
    let cases = [
        ("play", MpdCommand::Play(Some(0)), "play failed", "ACK [52@"),
        (
            "pause",
            MpdCommand::Pause(Some(true)),
            "pause failed",
            "ACK [52@",
        ),
        ("stop", MpdCommand::Stop, "stop failed", "ACK [52@"),
        ("next", MpdCommand::Next, "next failed", "ACK [52@"),
        (
            "previous",
            MpdCommand::Previous,
            "previous failed",
            "ACK [52@",
        ),
        ("seek", MpdCommand::Seek(0, 10.0), "seek failed", "ACK [52@"),
        (
            "seekcur",
            MpdCommand::SeekCur(5.0),
            "seekcur failed",
            "ACK [52@",
        ),
        (
            "setvol",
            MpdCommand::SetVol(50),
            "setvol failed",
            "ACK [52@",
        ),
        (
            "volume",
            MpdCommand::Volume(10),
            "volume failed",
            "ACK [52@",
        ),
        (
            "add",
            MpdCommand::Add("file.flac".into()),
            "add failed",
            "ACK [50@",
        ),
        (
            "addid",
            MpdCommand::AddId("file.flac".into(), None),
            "addid failed",
            "ACK [50@",
        ),
        ("delete", MpdCommand::Delete(0), "delete failed", "ACK [2@"),
        ("clear", MpdCommand::Clear, "clear failed", "ACK [52@"),
    ];

    for (label, command, message, prefix) in cases {
        let adapter = ConfigurableAdapter::new_err(message);
        let out = handle_command(&command, &adapter).format();
        assert!(
            out.starts_with(prefix),
            "{label}: expected {prefix} error, got {out}"
        );
        assert!(
            out.contains(message),
            "{label}: expected adapter error message {message:?}, got {out}"
        );
    }
}

// ============================================================================
// Handler tests: status variations
// ============================================================================

#[test]
fn test_handle_status_playing_with_elapsed() {
    let mut adapter = ConfigurableAdapter::new_ok();
    adapter.status_val.state = MpdPlayState::Play;
    adapter.status_val.elapsed = 12.345;
    adapter.status_val.duration = 180.5;
    adapter.status_val.song = Some(2);
    adapter.status_val.songid = Some(42);
    adapter.status_val.audio = Some("44100:16:2".into());
    let resp = handle_command(&MpdCommand::Status, &adapter);
    let out = resp.format();
    assert!(out.contains("state: play"));
    assert!(out.contains("elapsed: 12.345"));
    assert!(out.contains("duration: 180.500"));
    assert!(out.contains("time: 12:180"));
    assert!(out.contains("song: 2"));
    assert!(out.contains("songid: 42"));
    assert!(out.contains("audio: 44100:16:2"));
    assert!(out.contains("OK"));
}

#[test]
fn test_handle_status_paused() {
    let mut adapter = ConfigurableAdapter::new_ok();
    adapter.status_val.state = MpdPlayState::Pause;
    adapter.status_val.elapsed = 5.0;
    adapter.status_val.duration = 200.0;
    let resp = handle_command(&MpdCommand::Status, &adapter);
    let out = resp.format();
    assert!(out.contains("state: pause"));
    assert!(out.contains("elapsed: 5.000"));
    assert!(out.contains("duration: 200.000"));
    assert!(out.contains("time: 5:200"));
}

#[test]
fn test_handle_status_stopped_no_elapsed() {
    let mut adapter = ConfigurableAdapter::new_ok();
    adapter.status_val.state = MpdPlayState::Stop;
    adapter.status_val.elapsed = 999.0;
    adapter.status_val.duration = 999.0;
    let resp = handle_command(&MpdCommand::Status, &adapter);
    let out = resp.format();
    assert!(out.contains("state: stop"));
    assert!(!out.contains("elapsed:"));
    assert!(!out.contains("duration:"));
    assert!(!out.contains("time:"));
}

#[test]
fn test_handle_status_nan_elapsed_clamped() {
    let mut adapter = ConfigurableAdapter::new_ok();
    adapter.status_val.state = MpdPlayState::Play;
    adapter.status_val.elapsed = f64::NAN;
    adapter.status_val.duration = f64::NAN;
    let resp = handle_command(&MpdCommand::Status, &adapter);
    let out = resp.format();
    assert!(out.contains("elapsed: 0.000"));
    assert!(out.contains("duration: 0.000"));
    assert!(out.contains("time: 0:0"));
}

#[test]
fn test_handle_status_negative_elapsed_clamped() {
    let mut adapter = ConfigurableAdapter::new_ok();
    adapter.status_val.state = MpdPlayState::Play;
    adapter.status_val.elapsed = -5.0;
    adapter.status_val.duration = -1.0;
    let resp = handle_command(&MpdCommand::Status, &adapter);
    let out = resp.format();
    assert!(out.contains("elapsed: 0.000"));
    assert!(out.contains("duration: 0.000"));
}

// ============================================================================
// Handler tests: current song
// ============================================================================

#[test]
fn test_handle_currentsong_none() {
    let adapter = ConfigurableAdapter::new_ok();
    let resp = handle_command(&MpdCommand::CurrentSong, &adapter);
    assert_eq!(resp.format(), "OK\n");
}

#[test]
fn test_handle_currentsong_some() {
    let adapter = ConfigurableAdapter::new_ok();
    *adapter.current_song_val.lock().unwrap() = Some(MpdSongInfo {
        file: "music/song.flac".into(),
        title: Some("Song Title".into()),
        artist: Some("Artist Name".into()),
        album: Some("Album Name".into()),
        track: Some(3),
        date: Some("2024".into()),
        genre: Some("Rock".into()),
        duration: Some(210.5),
        pos: 5,
        id: 99,
    });
    let resp = handle_command(&MpdCommand::CurrentSong, &adapter);
    let out = resp.format();
    assert!(out.contains("file: music/song.flac"));
    assert!(out.contains("Title: Song Title"));
    assert!(out.contains("Artist: Artist Name"));
    assert!(out.contains("Album: Album Name"));
    assert!(out.contains("Track: 3"));
    assert!(out.contains("Date: 2024"));
    assert!(out.contains("Genre: Rock"));
    assert!(out.contains("duration: 210.500"));
    assert!(out.contains("Time: 210"));
    assert!(out.contains("Pos: 5"));
    assert!(out.contains("Id: 99"));
    assert!(out.contains("OK"));
}

// ============================================================================
// Handler tests: playlist
// ============================================================================

#[test]
fn test_handle_playlistinfo_with_songs() {
    let adapter = ConfigurableAdapter::new_ok();
    *adapter.playlist_songs_val.lock().unwrap() = vec![
        MpdSongInfo {
            file: "a.flac".into(),
            title: Some("A".into()),
            artist: None,
            album: None,
            track: None,
            date: None,
            genre: None,
            duration: None,
            pos: 0,
            id: 1,
        },
        MpdSongInfo {
            file: "b.flac".into(),
            title: Some("B".into()),
            artist: None,
            album: None,
            track: None,
            date: None,
            genre: None,
            duration: None,
            pos: 1,
            id: 2,
        },
    ];
    let resp = handle_command(&MpdCommand::PlaylistInfo(None), &adapter);
    let out = resp.format();
    assert!(out.contains("file: a.flac"));
    assert!(out.contains("file: b.flac"));
    assert!(out.contains("OK"));
}

#[test]
fn test_handle_playlistid_none_lists_all() {
    let adapter = ConfigurableAdapter::new_ok();
    *adapter.playlist_songs_val.lock().unwrap() = vec![MpdSongInfo {
        file: "a.flac".into(),
        title: None,
        artist: None,
        album: None,
        track: None,
        date: None,
        genre: None,
        duration: None,
        pos: 0,
        id: 1,
    }];
    let resp = handle_command(&MpdCommand::PlaylistId(None), &adapter);
    let out = resp.format();
    assert!(out.contains("file: a.flac"));
    assert!(out.contains("OK"));
}

#[test]
fn test_handle_playlistid_some_found() {
    let adapter = ConfigurableAdapter::new_ok();
    adapter.playlist_song_by_id_val.lock().unwrap().insert(
        7,
        MpdSongInfo {
            file: "found.flac".into(),
            title: Some("Found".into()),
            artist: None,
            album: None,
            track: None,
            date: None,
            genre: None,
            duration: None,
            pos: 3,
            id: 7,
        },
    );
    let resp = handle_command(&MpdCommand::PlaylistId(Some(7)), &adapter);
    let out = resp.format();
    assert!(out.contains("file: found.flac"));
    assert!(out.contains("OK"));
}

#[test]
fn test_handle_playlistid_some_not_found() {
    let adapter = ConfigurableAdapter::new_ok();
    let resp = handle_command(&MpdCommand::PlaylistId(Some(999)), &adapter);
    let out = resp.format();
    assert!(out.starts_with("ACK [50@"), "expected NoExist, got {out}");
    assert!(out.contains("No such song with id: 999"));
}

// ============================================================================
// Handler tests: add / addid / delete / clear
// ============================================================================

#[test]
fn test_handle_add_ok() {
    let adapter = ConfigurableAdapter::new_ok();
    let resp = handle_command(&MpdCommand::Add("uri.flac".into()), &adapter);
    assert!(resp.format().contains("OK"));
    assert_eq!(
        *adapter.last_add_uri.lock().unwrap(),
        Some("uri.flac".into())
    );
}

#[test]
fn test_handle_addid_ok() {
    let adapter = ConfigurableAdapter::new_ok();
    let resp = handle_command(&MpdCommand::AddId("uri.flac".into(), Some(2)), &adapter);
    let out = resp.format();
    assert!(out.contains("Id: 1"));
    assert!(out.contains("OK"));
    assert_eq!(
        *adapter.last_add_id_uri.lock().unwrap(),
        Some("uri.flac".into())
    );
    assert_eq!(*adapter.last_add_id_pos.lock().unwrap(), Some(Some(2)));
}

#[test]
fn test_handle_delete_ok() {
    let adapter = ConfigurableAdapter::new_ok();
    let resp = handle_command(&MpdCommand::Delete(4), &adapter);
    assert!(resp.format().contains("OK"));
    assert_eq!(*adapter.last_delete_pos.lock().unwrap(), Some(4));
}

#[test]
fn test_handle_deleteid_ok() {
    let adapter = ConfigurableAdapter::new_ok();
    adapter.playlist_song_by_id_val.lock().unwrap().insert(
        10,
        MpdSongInfo {
            file: "x.flac".into(),
            title: None,
            artist: None,
            album: None,
            track: None,
            date: None,
            genre: None,
            duration: None,
            pos: 2,
            id: 10,
        },
    );
    let resp = handle_command(&MpdCommand::DeleteId(10), &adapter);
    assert!(resp.format().contains("OK"));
    assert_eq!(*adapter.last_delete_pos.lock().unwrap(), Some(2));
}

#[test]
fn test_handle_deleteid_unknown() {
    let adapter = ConfigurableAdapter::new_ok();
    let resp = handle_command(&MpdCommand::DeleteId(999), &adapter);
    let out = resp.format();
    assert!(out.starts_with("ACK [50@"), "expected NoExist, got {out}");
}

#[test]
fn test_handle_clear_ok() {
    let adapter = ConfigurableAdapter::new_ok();
    let resp = handle_command(&MpdCommand::Clear, &adapter);
    assert!(resp.format().contains("OK"));
}

// ============================================================================
// Handler tests: database / library commands
// ============================================================================

#[test]
fn test_handle_find_with_results() {
    let adapter = ConfigurableAdapter::new_ok();
    *adapter.search_results_val.lock().unwrap() = vec![MpdSongInfo {
        file: "r1.flac".into(),
        title: Some("Result 1".into()),
        artist: None,
        album: None,
        track: None,
        date: None,
        genre: None,
        duration: None,
        pos: 0,
        id: 1,
    }];
    let resp = handle_command(&MpdCommand::Find(vec![]), &adapter);
    let out = resp.format();
    assert!(out.contains("file: r1.flac"));
    assert!(out.contains("OK"));
}

#[test]
fn test_handle_search_with_results() {
    let adapter = ConfigurableAdapter::new_ok();
    *adapter.search_results_val.lock().unwrap() = vec![MpdSongInfo {
        file: "s1.flac".into(),
        title: Some("Search 1".into()),
        artist: None,
        album: None,
        track: None,
        date: None,
        genre: None,
        duration: None,
        pos: 0,
        id: 1,
    }];
    let resp = handle_command(&MpdCommand::Search(vec![]), &adapter);
    let out = resp.format();
    assert!(out.contains("file: s1.flac"));
    assert!(out.contains("OK"));
}

#[test]
fn test_handle_list_with_results() {
    let mut adapter = ConfigurableAdapter::new_ok();
    adapter.list_tag_results_val = vec!["Album A".into(), "Album B".into()];
    let resp = handle_command(&MpdCommand::List("album".into(), vec![]), &adapter);
    let out = resp.format();
    assert!(out.contains("Album: Album A"));
    assert!(out.contains("Album: Album B"));
    assert!(out.contains("OK"));
}

#[test]
fn test_handle_count_with_results() {
    let adapter = ConfigurableAdapter::new_ok();
    *adapter.search_results_val.lock().unwrap() = vec![
        MpdSongInfo {
            file: "a.flac".into(),
            title: None,
            artist: None,
            album: None,
            track: None,
            date: None,
            genre: None,
            duration: Some(120.0),
            pos: 0,
            id: 1,
        },
        MpdSongInfo {
            file: "b.flac".into(),
            title: None,
            artist: None,
            album: None,
            track: None,
            date: None,
            genre: None,
            duration: Some(180.0),
            pos: 1,
            id: 2,
        },
    ];
    let resp = handle_command(&MpdCommand::Count(vec![]), &adapter);
    let out = resp.format();
    assert!(out.contains("songs: 2"));
    assert!(out.contains("playtime: 300"));
    assert!(out.contains("OK"));
}

#[test]
fn test_handle_count_negative_playtime_clamped() {
    let adapter = ConfigurableAdapter::new_ok();
    *adapter.search_results_val.lock().unwrap() = vec![MpdSongInfo {
        file: "a.flac".into(),
        title: None,
        artist: None,
        album: None,
        track: None,
        date: None,
        genre: None,
        duration: None,
        pos: 0,
        id: 1,
    }];
    // Override count behavior by using default implementation which sums durations
    // Since duration is None, total is 0.0, so this just tests the path.
    let resp = handle_command(&MpdCommand::Count(vec![]), &adapter);
    let out = resp.format();
    assert!(out.contains("songs: 1"));
    assert!(out.contains("playtime: 0"));
}

#[test]
fn test_handle_listall_with_dirs_and_files() {
    let adapter = ConfigurableAdapter::new_ok();
    *adapter.lsinfo_results_val.lock().unwrap() = vec![
        MpdDirEntry {
            is_directory: true,
            path: "Music".into(),
        },
        MpdDirEntry {
            is_directory: false,
            path: "song.flac".into(),
        },
    ];
    let resp = handle_command(&MpdCommand::ListAll(None), &adapter);
    let out = resp.format();
    assert!(out.contains("directory: Music"));
    assert!(out.contains("file: song.flac"));
    assert!(out.contains("OK"));
}

#[test]
fn test_handle_lsinfo_with_dirs_and_files() {
    let adapter = ConfigurableAdapter::new_ok();
    *adapter.lsinfo_results_val.lock().unwrap() = vec![
        MpdDirEntry {
            is_directory: true,
            path: "Artists".into(),
        },
        MpdDirEntry {
            is_directory: false,
            path: "track.mp3".into(),
        },
    ];
    let resp = handle_command(&MpdCommand::LsInfo(None), &adapter);
    let out = resp.format();
    assert!(out.contains("directory: Artists"));
    assert!(out.contains("file: track.mp3"));
    assert!(out.contains("OK"));
}

// ============================================================================
// Handler tests: outputs / stats / decoders
// ============================================================================

#[test]
fn test_handle_outputs_response() {
    let adapter = ConfigurableAdapter::new_ok();
    let resp = handle_command(&MpdCommand::Outputs, &adapter);
    let out = resp.format();
    assert!(out.contains("outputid: 0"));
    assert!(out.contains("outputname: SOTF Audio Output"));
    assert!(out.contains("plugin: cpal"));
    assert!(out.contains("outputenabled: 1"));
    assert!(out.contains("OK"));
}

#[test]
fn test_handle_stats_response() {
    let adapter = ConfigurableAdapter::new_ok();
    let resp = handle_command(&MpdCommand::Stats, &adapter);
    let out = resp.format();
    assert!(out.contains("uptime: 0"));
    assert!(out.contains("playtime: 0"));
    assert!(out.contains("artists: 0"));
    assert!(out.contains("albums: 0"));
    assert!(out.contains("songs: 0"));
    assert!(out.contains("db_playtime: 0"));
    assert!(out.contains("db_update: 0"));
    assert!(out.contains("OK"));
}

#[test]
fn test_handle_decoders_response() {
    let adapter = ConfigurableAdapter::new_ok();
    let resp = handle_command(&MpdCommand::Decoders, &adapter);
    let out = resp.format();
    assert!(out.contains("plugin: flac"));
    assert!(out.contains("plugin: mp3"));
    assert!(out.contains("suffix: ogg"));
    assert!(out.contains("mime_type: audio/flac"));
    assert!(out.contains("OK"));
}

// ============================================================================
// Handler tests: seekid error path
// ============================================================================

#[test]
fn test_handle_seekid_ok() {
    let adapter = ConfigurableAdapter::new_ok();
    adapter.playlist_song_by_id_val.lock().unwrap().insert(
        5,
        MpdSongInfo {
            file: "s.flac".into(),
            title: None,
            artist: None,
            album: None,
            track: None,
            date: None,
            genre: None,
            duration: None,
            pos: 1,
            id: 5,
        },
    );
    let resp = handle_command(&MpdCommand::SeekId(5, 30.0), &adapter);
    assert!(resp.format().contains("OK"));
    assert_eq!(*adapter.last_seek_pos.lock().unwrap(), Some((1, 30.0)));
}

#[test]
fn test_handle_seekid_unknown() {
    let adapter = ConfigurableAdapter::new_ok();
    let resp = handle_command(&MpdCommand::SeekId(999, 10.0), &adapter);
    let out = resp.format();
    assert!(out.starts_with("ACK [50@"), "expected NoExist, got {out}");
}
