use super::line_read::handle_session;
use super::line_read::read_line_bounded;
use super::misc::MAX_LINE_BYTES;
use super::misc::execute_command_list;
use super::misc::redact_for_log;
use super::types::LineRead;
use crate::handler::PlayerAdapter;
use crate::protocol::MpdCommand;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};

use crate::handler::*;
use crate::protocol::FilterExpr;

#[cfg(feature = "tls")]
use super::mpd_server::MpdServer;
#[cfg(feature = "tls")]
use super::mpd_server_config::MpdServerConfig;
#[cfg(feature = "tls")]
use super::types::MpdAuthMode;

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
            volume: 50,
            repeat: false,
            random: false,
            single: false,
            consume: false,
            state: MpdPlayState::Play,
            song: Some(0),
            songid: Some(0),
            elapsed: 30.5,
            duration: 240.0,
            audio: Some("44100:16:2".to_string()),
            playlist_length: 3,
            playlist_version: 5,
        }
    }
    fn current_song(&self) -> Option<MpdSongInfo> {
        Some(MpdSongInfo {
            file: "music/test.flac".to_string(),
            title: Some("Test Song".to_string()),
            artist: Some("Test Artist".to_string()),
            album: Some("Test Album".to_string()),
            track: Some(1),
            date: Some("2024".to_string()),
            genre: Some("Rock".to_string()),
            duration: Some(240.0),
            pos: 0,
            id: 0,
        })
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
        Ok(42)
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

struct IdleAdapter {
    volume: AtomicU8,
}

impl IdleAdapter {
    fn new(volume: u8) -> Self {
        Self {
            volume: AtomicU8::new(volume),
        }
    }
}

impl PlayerAdapter for IdleAdapter {
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
    fn set_volume(&self, vol: u8) -> Result<(), String> {
        self.volume.store(vol, Ordering::SeqCst);
        Ok(())
    }
    fn volume_change(&self, delta: i8) -> Result<(), String> {
        let current = self.volume.load(Ordering::SeqCst) as i16;
        self.volume.store(
            (current + delta as i16).clamp(0, 100) as u8,
            Ordering::SeqCst,
        );
        Ok(())
    }
    fn status(&self) -> MpdStatus {
        MpdStatus {
            volume: self.volume.load(Ordering::SeqCst),
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
        Ok(1)
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
fn test_command_list_execution() {
    let adapter: Arc<dyn PlayerAdapter> = Arc::new(DummyAdapter);
    let cmds = vec![MpdCommand::Status, MpdCommand::CurrentSong];

    let result = execute_command_list(&cmds, false, &adapter);
    assert!(result.contains("state: play"));
    assert!(result.contains("Title: Test Song"));
    assert!(result.ends_with("OK\n"));
}

#[test]
fn test_command_list_ok_execution() {
    let adapter: Arc<dyn PlayerAdapter> = Arc::new(DummyAdapter);
    let cmds = vec![MpdCommand::Ping, MpdCommand::Ping];

    let result = execute_command_list(&cmds, true, &adapter);
    assert_eq!(result.matches("list_OK").count(), 2);
    assert!(result.ends_with("OK\n"));
}

// ===== Regression: bounded line read rejects DoS payloads =====
//
// `tokio::io::AsyncBufReadExt::read_line` has no length cap, so an
// unauthenticated peer could stream gigabytes without a newline and OOM
// the host. `read_line_bounded` enforces a byte budget and signals
// `TooLong` instead of growing its buffer indefinitely.

#[tokio::test]
async fn test_read_line_bounded_accepts_short_line() {
    let input: &[u8] = b"ping\n";
    let mut reader = BufReader::new(input);
    match read_line_bounded(&mut reader, MAX_LINE_BYTES).await {
        LineRead::Line(s) => assert_eq!(s, "ping"),
        other => panic!("unexpected: {other:?}", other = format_outcome(&other)),
    }
}

#[tokio::test]
async fn test_read_line_bounded_rejects_oversized_payload() {
    // 16 KiB of `a` without any newline — twice the 8 KiB limit. The old
    // code would happily accept the entire payload (and would keep
    // accepting an unbounded continuation) before passing it to the
    // parser, allowing a trivial OOM-by-streaming.
    let oversized = vec![b'a'; MAX_LINE_BYTES * 2];
    let mut reader = BufReader::new(&oversized[..]);
    match read_line_bounded(&mut reader, MAX_LINE_BYTES).await {
        LineRead::TooLong => {}
        other => panic!(
            "expected TooLong, got {other:?}",
            other = format_outcome(&other)
        ),
    }
}

#[tokio::test]
async fn test_read_line_bounded_handles_crlf() {
    let input: &[u8] = b"status\r\n";
    let mut reader = BufReader::new(input);
    match read_line_bounded(&mut reader, MAX_LINE_BYTES).await {
        LineRead::Line(s) => assert_eq!(s, "status"),
        other => panic!("unexpected: {other:?}", other = format_outcome(&other)),
    }
}

#[tokio::test]
async fn test_read_line_bounded_eof_returns_eof() {
    let input: &[u8] = b"";
    let mut reader = BufReader::new(input);
    match read_line_bounded(&mut reader, MAX_LINE_BYTES).await {
        LineRead::Eof => {}
        other => panic!(
            "expected Eof, got {other:?}",
            other = format_outcome(&other)
        ),
    }
}

fn format_outcome(r: &LineRead) -> String {
    match r {
        LineRead::Line(s) => format!("Line({s:?})"),
        LineRead::Eof => "Eof".into(),
        LineRead::TooLong => "TooLong".into(),
        LineRead::InvalidUtf8 => "InvalidUtf8".into(),
    }
}

async fn read_client_chunk<R: tokio::io::AsyncRead + Unpin>(reader: &mut R) -> String {
    let mut buf = vec![0; 1024];
    let n = tokio::time::timeout(std::time::Duration::from_secs(1), reader.read(&mut buf))
        .await
        .unwrap()
        .unwrap();
    String::from_utf8_lossy(&buf[..n]).to_string()
}

#[tokio::test]
async fn test_idle_waits_until_noidle() {
    let (client, server) = tokio::io::duplex(1024);
    let (mut client_reader, mut client_writer) = tokio::io::split(client);
    let (server_reader, server_writer) = tokio::io::split(server);
    let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    let adapter: Arc<dyn PlayerAdapter> = Arc::new(IdleAdapter::new(50));

    let server_task = tokio::spawn(handle_session(
        server_reader,
        server_writer,
        adapter,
        cancel_rx,
        None,
    ));

    assert!(
        read_client_chunk(&mut client_reader)
            .await
            .starts_with("OK MPD ")
    );
    client_writer.write_all(b"idle player\n").await.unwrap();
    let pending = tokio::time::timeout(
        std::time::Duration::from_millis(50),
        read_client_chunk(&mut client_reader),
    )
    .await;
    assert!(
        pending.is_err(),
        "idle should wait until noidle or a change"
    );

    client_writer.write_all(b"noidle\n").await.unwrap();
    assert_eq!(read_client_chunk(&mut client_reader).await, "OK\n");
    client_writer.write_all(b"close\n").await.unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn test_idle_reports_mixer_change() {
    let (client, server) = tokio::io::duplex(1024);
    let (mut client_reader, mut client_writer) = tokio::io::split(client);
    let (server_reader, server_writer) = tokio::io::split(server);
    let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    let adapter = Arc::new(IdleAdapter::new(50));
    let server_adapter: Arc<dyn PlayerAdapter> = adapter.clone();

    let server_task = tokio::spawn(handle_session(
        server_reader,
        server_writer,
        server_adapter,
        cancel_rx,
        None,
    ));

    assert!(
        read_client_chunk(&mut client_reader)
            .await
            .starts_with("OK MPD ")
    );
    client_writer.write_all(b"idle mixer\n").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    adapter.volume.store(51, Ordering::SeqCst);

    let response = read_client_chunk(&mut client_reader).await;
    assert_eq!(response, "changed: mixer\nOK\n");
    client_writer.write_all(b"close\n").await.unwrap();
    server_task.await.unwrap().unwrap();
}

// ===== Regression: password lines are redacted before logging =====

#[test]
fn test_redact_for_log_strips_password_argument() {
    // Trace logging used to emit the raw command line, leaking plaintext
    // credentials whenever trace was enabled.
    assert_eq!(redact_for_log("password hunter2"), "password <redacted>");
    // Case-insensitive on the command name (MPD commands are ASCII).
    assert_eq!(redact_for_log("Password hunter2"), "password <redacted>");
    // Leading whitespace is preserved so the log columns still line up.
    assert_eq!(
        redact_for_log("  password hunter2"),
        "  password <redacted>"
    );
    // Non-password commands pass through unmodified.
    assert_eq!(redact_for_log("status"), "status");
    // A command that merely starts with the prefix `pass` is NOT a
    // password command and must not be redacted.
    assert_eq!(redact_for_log("passive 1"), "passive 1");
}

// ===== Regression: TLS misconfiguration refused at start =====

#[cfg(feature = "tls")]
#[tokio::test]
async fn test_run_refuses_tls_enabled_without_acceptor() {
    // tls_enabled=true with no acceptor installed used to silently fall
    // through to plaintext. Now `run()` must return an error before
    // binding, so a misconfigured deployment fails fast instead of
    // accepting cleartext on an apparently-encrypted port.
    let adapter: Arc<dyn PlayerAdapter> = Arc::new(DummyAdapter);
    let config = MpdServerConfig {
        // Bind to an ephemeral port and localhost; the test should bail
        // before bind() is reached anyway.
        bind_address: "127.0.0.1".into(),
        port: 0,
        tls_enabled: true,
        auth_mode: MpdAuthMode::Certificate,
        password: None,
        trusted_client_fingerprints: std::sync::Arc::new(std::sync::Mutex::new(
            std::collections::HashSet::new(),
        )),
    };
    let server = MpdServer::with_config(config, adapter);
    let (_tx, rx) = tokio::sync::watch::channel(false);
    let err = server
        .run(rx)
        .await
        .expect_err("expected misconfiguration error");
    assert!(
        err.contains("tls_enabled=true") || err.to_lowercase().contains("tls"),
        "error should mention TLS, got: {err}"
    );
}

// ----- Additional server helper coverage -----

#[test]
fn test_redact_for_log_edge_cases() {
    // Already-redacted line passes through unchanged
    assert_eq!(redact_for_log("password <redacted>"), "password <redacted>");
    // Bare command name is also redacted (no argument to leak)
    assert_eq!(redact_for_log("password"), "password <redacted>");
    // Tabs and multiple spaces between command and arg
    assert_eq!(redact_for_log("password\tsecret123"), "password <redacted>");
    // Case-insensitive match
    assert_eq!(redact_for_log("PASSWORD secret"), "password <redacted>");
}

#[tokio::test]
async fn test_read_line_bounded_only_lf() {
    let input: &[u8] = b"play\n";
    let mut reader = BufReader::new(input);
    match read_line_bounded(&mut reader, MAX_LINE_BYTES).await {
        LineRead::Line(s) => assert_eq!(s, "play"),
        other => panic!("unexpected: {other:?}", other = format_outcome(&other)),
    }
}

#[tokio::test]
async fn test_read_line_bounded_exactly_at_limit() {
    // max_bytes=3: three data bytes plus LF fits exactly.
    let input: &[u8] = b"abc\n";
    let mut reader = BufReader::new(input);
    match read_line_bounded(&mut reader, 3).await {
        LineRead::Line(s) => assert_eq!(s, "abc"),
        other => panic!("unexpected: {other:?}", other = format_outcome(&other)),
    }
}

#[tokio::test]
async fn test_read_line_bounded_oversize_by_one() {
    // max_bytes=3: four data bytes before LF must be rejected.
    let input: &[u8] = b"abcd\n";
    let mut reader = BufReader::new(input);
    match read_line_bounded(&mut reader, 3).await {
        LineRead::TooLong => {}
        other => panic!(
            "expected TooLong, got {other:?}",
            other = format_outcome(&other)
        ),
    }
}

#[tokio::test]
async fn test_read_line_bounded_times_out_when_reader_blocks() {
    // A reader that never produces data must block until the outer timeout
    // fires; this confirms the bounded read does not busy-loop or return early.
    let (reader, _writer) = tokio::io::duplex(64);
    let mut reader = BufReader::new(reader);
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(10),
        read_line_bounded(&mut reader, MAX_LINE_BYTES),
    )
    .await;
    assert!(result.is_err(), "expected timeout");
}

#[tokio::test]
async fn test_read_line_bounded_invalid_utf8() {
    let input: &[u8] = b"\xff\xfe\n";
    let mut reader = BufReader::new(input);
    match read_line_bounded(&mut reader, MAX_LINE_BYTES).await {
        LineRead::InvalidUtf8 => {}
        other => panic!(
            "expected InvalidUtf8, got {other:?}",
            other = format_outcome(&other)
        ),
    }
}

#[tokio::test]
async fn test_read_line_bounded_partial_line_at_eof() {
    let input: &[u8] = b"status";
    let mut reader = BufReader::new(input);
    match read_line_bounded(&mut reader, MAX_LINE_BYTES).await {
        LineRead::Line(s) => assert_eq!(s, "status"),
        other => panic!("unexpected: {other:?}", other = format_outcome(&other)),
    }
}
