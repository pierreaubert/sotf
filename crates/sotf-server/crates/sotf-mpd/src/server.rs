// ============================================================================
// MPD TCP Server
// ============================================================================
//
// Listens on a TCP port (default 6600) and handles MPD client sessions.
// Each client gets its own task. Supports command lists.
// Optional TLS via the `tls` feature.

use crate::handler::{PlayerAdapter, handle_command};
use crate::protocol::{self, MPD_VERSION, MpdCommand, MpdResponse};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

/// Maximum length, in bytes, of a single MPD protocol line. The reference MPD
/// server caps at ~32 KiB; we use a tighter 8 KiB which still comfortably
/// fits realistic commands while keeping the per-session memory bound small
/// enough that an unauthenticated peer cannot exhaust the host by streaming
/// gigabytes without a newline.
const MAX_LINE_BYTES: usize = 8 * 1024;

/// Authentication mode for the MPD server.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum MpdAuthMode {
    /// Mutual TLS — client cert fingerprint must be in the trusted set.
    #[default]
    Certificate,
    /// Legacy password-based authentication.
    Password,
}

/// MPD server configuration.
pub struct MpdServerConfig {
    /// Address to bind to (default "0.0.0.0").
    pub bind_address: String,
    /// Port to listen on (default 6600).
    pub port: u16,
    /// Enable TLS (requires `tls` feature). Default: true.
    pub tls_enabled: bool,
    /// Authentication mode (default: Certificate/mTLS).
    pub auth_mode: MpdAuthMode,
    /// Optional password for MPD authentication (only used when auth_mode == Password).
    pub password: Option<String>,
    /// Trusted client certificate fingerprints (only used when auth_mode == Certificate).
    #[cfg(feature = "tls")]
    pub trusted_client_fingerprints:
        std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
}

impl Default for MpdServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 6600,
            tls_enabled: true,
            auth_mode: MpdAuthMode::Certificate,
            password: None,
            #[cfg(feature = "tls")]
            trusted_client_fingerprints: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::HashSet::new(),
            )),
        }
    }
}

/// The MPD protocol server.
pub struct MpdServer {
    config: MpdServerConfig,
    adapter: Arc<dyn PlayerAdapter>,
    #[cfg(feature = "tls")]
    tls_acceptor: Option<tokio_rustls::TlsAcceptor>,
}

impl MpdServer {
    #[must_use]
    pub fn new(adapter: Arc<dyn PlayerAdapter>) -> Self {
        Self {
            config: MpdServerConfig::default(),
            adapter,
            #[cfg(feature = "tls")]
            tls_acceptor: None,
        }
    }

    #[must_use]
    pub fn with_config(config: MpdServerConfig, adapter: Arc<dyn PlayerAdapter>) -> Self {
        Self {
            config,
            adapter,
            #[cfg(feature = "tls")]
            tls_acceptor: None,
        }
    }

    /// Set the TLS acceptor for encrypted connections.
    ///
    /// When set and `config.tls_enabled` is true, all connections are TLS-wrapped.
    #[cfg(feature = "tls")]
    pub fn set_tls_acceptor(&mut self, acceptor: tokio_rustls::TlsAcceptor) {
        self.tls_acceptor = Some(acceptor);
    }

    /// Start the MPD server. This runs until the cancellation token is triggered.
    ///
    /// # Errors
    /// Returns an error if the server cannot bind to the configured address.
    pub async fn run(&self, cancel: tokio::sync::watch::Receiver<bool>) -> Result<(), String> {
        let addr = format!("{}:{}", self.config.bind_address, self.config.port);

        // Refuse to start in a misconfigured TLS state: if the integrator asked
        // for TLS (`tls_enabled = true`) but never installed an acceptor, every
        // connection would silently fall through to plaintext while the
        // surrounding configuration suggests otherwise. That is the worst
        // failure mode for a network-facing service, so bail out instead of
        // silently accepting cleartext on `0.0.0.0:6600`.
        #[cfg(feature = "tls")]
        {
            if self.config.tls_enabled && self.tls_acceptor.is_none() {
                return Err(format!(
                    "MPD configured with tls_enabled=true but no TLS acceptor installed; \
                     refusing to bind {addr} to avoid silently accepting plaintext. \
                     Call MpdServer::set_tls_acceptor() before run(), or set tls_enabled=false."
                ));
            }
        }

        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| format!("Failed to bind to {addr}: {e}"))?;

        let tls_mode = self.tls_mode_label();
        log::info!("[MPD] Listening on {addr} ({tls_mode})");

        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, peer)) => {
                            log::debug!("[MPD] New connection from {peer}");
                            let adapter = Arc::clone(&self.adapter);
                            let cancel = cancel.clone();
                            // mTLS: client is authenticated by TLS handshake, no password needed.
                            let password = if self.config.auth_mode == MpdAuthMode::Certificate {
                                None
                            } else {
                                self.config.password.clone()
                            };

                            #[cfg(feature = "tls")]
                            let tls_acceptor = if self.config.tls_enabled {
                                self.tls_acceptor.clone()
                            } else {
                                None
                            };

                            tokio::spawn(async move {
                                let result = {
                                    #[cfg(feature = "tls")]
                                    {
                                        if let Some(acceptor) = tls_acceptor {
                                            match sotf_tls::tls_accept(&acceptor, stream).await {
                                                Ok(tls_stream) => {
                                                    let (reader, writer) = tokio::io::split(tls_stream);
                                                    handle_session(reader, writer, adapter, cancel, password).await
                                                }
                                                Err(e) => {
                                                    log::debug!("[MPD] TLS handshake failed from {peer}: {e}");
                                                    return;
                                                }
                                            }
                                        } else {
                                            let (reader, writer) = stream.into_split();
                                            handle_session(reader, writer, adapter, cancel, password).await
                                        }
                                    }
                                    #[cfg(not(feature = "tls"))]
                                    {
                                        let (reader, writer) = stream.into_split();
                                        handle_session(reader, writer, adapter, cancel, password).await
                                    }
                                };

                                if let Err(e) = result {
                                    log::debug!("[MPD] Session error from {peer}: {e}");
                                }
                                log::debug!("[MPD] Connection closed from {peer}");
                            });
                        }
                        Err(e) => {
                            log::warn!("[MPD] Accept error: {e}");
                        }
                    }
                }
                _ = wait_for_cancel(&cancel) => {
                    log::info!("[MPD] Server shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    fn tls_mode_label(&self) -> &'static str {
        #[cfg(feature = "tls")]
        {
            if self.config.tls_enabled && self.tls_acceptor.is_some() {
                return "TLS";
            }
        }
        "plain"
    }
}

/// Outcome of [`read_line_bounded`].
enum LineRead {
    /// A complete line (without trailing CR/LF).
    Line(String),
    /// EOF before any bytes were read.
    Eof,
    /// The line exceeded `max_bytes` without a newline. Connection should be
    /// terminated after an ACK is sent.
    TooLong,
    /// Bytes that did not form valid UTF-8 once the newline was reached.
    InvalidUtf8,
}

/// Read a single newline-terminated line, but cap the maximum length so an
/// unauthenticated client cannot exhaust memory by streaming an unbounded
/// stream without a `\n`. `tokio::io::AsyncBufReadExt::read_line` has no such
/// bound — the underlying buffer grows without limit — so this implements
/// the loop manually with a byte-count budget.
async fn read_line_bounded<R>(reader: &mut BufReader<R>, max_bytes: usize) -> LineRead
where
    R: AsyncRead + Unpin,
{
    let mut buf: Vec<u8> = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match reader.read(&mut byte).await {
            Ok(0) => {
                if buf.is_empty() {
                    return LineRead::Eof;
                }
                // Trailing partial line without newline at EOF — treat as a
                // complete line.
                break;
            }
            Ok(_) => {
                if byte[0] == b'\n' {
                    break;
                }
                if buf.len() >= max_bytes {
                    return LineRead::TooLong;
                }
                buf.push(byte[0]);
            }
            Err(_) => return LineRead::Eof,
        }
    }
    // Strip a trailing CR so CRLF and LF endings are both handled uniformly.
    if buf.last() == Some(&b'\r') {
        buf.pop();
    }
    match String::from_utf8(buf) {
        Ok(s) => LineRead::Line(s),
        Err(_) => LineRead::InvalidUtf8,
    }
}

/// Redact sensitive arguments before emitting a trace log of a command line.
///
/// Currently strips the `password` argument so plaintext credentials never
/// reach the log sink, even at `trace` level.
fn redact_for_log(line: &str) -> String {
    let trimmed = line.trim_start();
    // Compare case-insensitively against the `password` command name. Only
    // ASCII matters here — MPD commands are ASCII — but `to_ascii_lowercase`
    // on the first few bytes avoids allocating a full lowercased copy.
    let first_word_end = trimmed
        .find(|c: char| c.is_whitespace())
        .unwrap_or(trimmed.len());
    if trimmed[..first_word_end].eq_ignore_ascii_case("password") {
        let lead_ws = &line[..line.len() - trimmed.len()];
        return format!("{lead_ws}password <redacted>");
    }
    line.to_string()
}

async fn wait_for_cancel(cancel: &tokio::sync::watch::Receiver<bool>) {
    let mut cancel = cancel.clone();
    loop {
        if *cancel.borrow() {
            return;
        }
        if cancel.changed().await.is_err() {
            return;
        }
    }
}

/// Handle a single client session over any async stream.
async fn handle_session<R, W>(
    reader: R,
    mut writer: W,
    adapter: Arc<dyn PlayerAdapter>,
    cancel: tokio::sync::watch::Receiver<bool>,
    password: Option<String>,
) -> Result<(), String>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut reader = BufReader::new(reader);
    let mut authenticated = password.is_none(); // no password = auto-authenticated

    // Send greeting
    let greeting = format!("OK MPD {MPD_VERSION}\n");
    writer
        .write_all(greeting.as_bytes())
        .await
        .map_err(|e| e.to_string())?;

    // Command list state
    let mut command_list: Option<Vec<MpdCommand>> = None;
    let mut command_list_ok = false;

    loop {
        let read_outcome = tokio::select! {
            outcome = read_line_bounded(&mut reader, MAX_LINE_BYTES) => outcome,
            _ = wait_for_cancel(&cancel) => break,
        };

        let line_buf = match read_outcome {
            LineRead::Eof => break,
            LineRead::TooLong => {
                // Refuse the line — an unauthenticated peer streaming a huge
                // pre-newline payload is a classic DoS vector. Send a generic
                // ACK and close the connection rather than re-syncing on
                // attacker-controlled bytes.
                let err =
                    protocol::MpdError::new(protocol::MpdErrorCode::Arg, "input", "line too long");
                let _ = writer.write_all(err.format().as_bytes()).await;
                break;
            }
            LineRead::InvalidUtf8 => {
                let err =
                    protocol::MpdError::new(protocol::MpdErrorCode::Arg, "input", "invalid UTF-8");
                writer
                    .write_all(err.format().as_bytes())
                    .await
                    .map_err(|e| e.to_string())?;
                continue;
            }
            LineRead::Line(s) => s,
        };

        let line = line_buf.as_str();
        if line.is_empty() {
            continue;
        }

        // Redact `password` commands before logging so plaintext credentials
        // never reach the trace sink even when trace logging is enabled.
        log::trace!("[MPD] <- {}", redact_for_log(line));

        // Parse the command
        let cmd = match protocol::parse_command(line) {
            Ok(cmd) => cmd,
            Err(err) => {
                let resp = err.format();
                log::trace!("[MPD] -> {}", resp.trim());
                writer
                    .write_all(resp.as_bytes())
                    .await
                    .map_err(|e| e.to_string())?;
                continue;
            }
        };

        // Handle password command before auth check
        if let MpdCommand::Password(ref pw) = cmd {
            if let Some(ref expected) = password {
                if pw == expected {
                    authenticated = true;
                    writer.write_all(b"OK\n").await.map_err(|e| e.to_string())?;
                } else {
                    let err = protocol::MpdError::new(
                        protocol::MpdErrorCode::Password,
                        "password",
                        "incorrect password",
                    );
                    writer
                        .write_all(err.format().as_bytes())
                        .await
                        .map_err(|e| e.to_string())?;
                }
            } else {
                // No password configured, accept anything
                writer.write_all(b"OK\n").await.map_err(|e| e.to_string())?;
            }
            continue;
        }

        // Require authentication for non-password commands (except ping/close)
        if !authenticated && !matches!(cmd, MpdCommand::Ping | MpdCommand::Close) {
            // Echo back at most 32 chars of the (possibly attacker-controlled)
            // command name so an unauthenticated peer cannot probe by reading
            // arbitrary bytes back through the ACK.
            let raw_cmd_name = line.split_whitespace().next().unwrap_or("unknown");
            let safe_cmd_name: String = raw_cmd_name.chars().take(32).collect();
            let err = protocol::MpdError::new(
                protocol::MpdErrorCode::Permission,
                &safe_cmd_name,
                "you don't have permission for this command",
            );
            writer
                .write_all(err.format().as_bytes())
                .await
                .map_err(|e| e.to_string())?;
            continue;
        }

        // Handle command list mode
        match &cmd {
            MpdCommand::CommandListBegin => {
                command_list = Some(Vec::new());
                command_list_ok = false;
                continue;
            }
            MpdCommand::CommandListOkBegin => {
                command_list = Some(Vec::new());
                command_list_ok = true;
                continue;
            }
            MpdCommand::CommandListEnd => {
                if let Some(cmds) = command_list.take() {
                    let response = execute_command_list(&cmds, command_list_ok, &adapter);
                    log::trace!("[MPD] -> (command list result, {} bytes)", response.len());
                    writer
                        .write_all(response.as_bytes())
                        .await
                        .map_err(|e| e.to_string())?;
                } else {
                    let err = protocol::MpdError::new(
                        protocol::MpdErrorCode::NotList,
                        "command_list_end",
                        "not in command list",
                    );
                    writer
                        .write_all(err.format().as_bytes())
                        .await
                        .map_err(|e| e.to_string())?;
                }
                continue;
            }
            _ => {}
        }

        // If we're in a command list, queue the command
        if let Some(ref mut list) = command_list {
            list.push(cmd);
            continue;
        }

        // Handle close
        if matches!(cmd, MpdCommand::Close) {
            break;
        }

        // Execute single command
        let response = handle_command(&cmd, adapter.as_ref());
        let formatted = response.format();
        log::trace!("[MPD] -> {}", formatted.trim());
        writer
            .write_all(formatted.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Execute a list of commands, producing a single response.
fn execute_command_list(
    cmds: &[MpdCommand],
    ok_between: bool,
    adapter: &Arc<dyn PlayerAdapter>,
) -> String {
    let mut output = String::new();

    for (i, cmd) in cmds.iter().enumerate() {
        let response = handle_command(cmd, adapter.as_ref());

        match response {
            MpdResponse::Ok(kvs) => {
                for kv_item in &kvs {
                    output.push_str(&kv_item.key);
                    output.push_str(": ");
                    output.push_str(&kv_item.value);
                    output.push('\n');
                }
                if ok_between {
                    output.push_str("list_OK\n");
                }
            }
            MpdResponse::Error(mut err) => {
                err.command_index = i;
                output.push_str(&err.format());
                return output; // Abort command list on error
            }
            MpdResponse::ListOk => {
                output.push_str("list_OK\n");
            }
        }
    }

    output.push_str("OK\n");
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::*;
    use crate::protocol::FilterExpr;

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
}
