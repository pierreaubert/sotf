// ============================================================================
// MPD TCP Server
// ============================================================================
//
// Listens on a TCP port (default 6600) and handles MPD client sessions.
// Each client gets its own task. Supports command lists.

use crate::handler::{PlayerAdapter, handle_command};
use crate::protocol::{self, MpdCommand, MpdResponse, MPD_VERSION};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

/// MPD server configuration.
pub struct MpdServerConfig {
    /// Address to bind to (default "0.0.0.0").
    pub bind_address: String,
    /// Port to listen on (default 6600).
    pub port: u16,
}

impl Default for MpdServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 6600,
        }
    }
}

/// The MPD protocol server.
pub struct MpdServer {
    config: MpdServerConfig,
    adapter: Arc<dyn PlayerAdapter>,
}

impl MpdServer {
    pub fn new(adapter: Arc<dyn PlayerAdapter>) -> Self {
        Self {
            config: MpdServerConfig::default(),
            adapter,
        }
    }

    pub fn with_config(config: MpdServerConfig, adapter: Arc<dyn PlayerAdapter>) -> Self {
        Self { config, adapter }
    }

    /// Start the MPD server. This runs until the cancellation token is triggered.
    pub async fn run(&self, cancel: tokio::sync::watch::Receiver<bool>) -> Result<(), String> {
        let addr = format!("{}:{}", self.config.bind_address, self.config.port);
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| format!("Failed to bind to {}: {}", addr, e))?;

        log::info!("[MPD] Listening on {}", addr);

        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, peer)) => {
                            log::debug!("[MPD] New connection from {}", peer);
                            let adapter = Arc::clone(&self.adapter);
                            let cancel = cancel.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle_session(stream, adapter, cancel).await {
                                    log::debug!("[MPD] Session error from {}: {}", peer, e);
                                }
                                log::debug!("[MPD] Connection closed from {}", peer);
                            });
                        }
                        Err(e) => {
                            log::warn!("[MPD] Accept error: {}", e);
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

/// Handle a single client session.
async fn handle_session(
    stream: tokio::net::TcpStream,
    adapter: Arc<dyn PlayerAdapter>,
    cancel: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line_buf = String::new();

    // Send greeting
    let greeting = format!("OK MPD {}\n", MPD_VERSION);
    writer
        .write_all(greeting.as_bytes())
        .await
        .map_err(|e| e.to_string())?;

    // Command list state
    let mut command_list: Option<Vec<MpdCommand>> = None;
    let mut command_list_ok = false;

    loop {
        line_buf.clear();

        tokio::select! {
            result = reader.read_line(&mut line_buf) => {
                match result {
                    Ok(0) => break, // EOF
                    Ok(_) => {}
                    Err(e) => {
                        return Err(format!("Read error: {}", e));
                    }
                }
            }
            _ = wait_for_cancel(&cancel) => {
                break;
            }
        }

        let line = line_buf.trim_end_matches('\n').trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }

        log::trace!("[MPD] <- {}", line);

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
        fn playlist_info(&self, _range: Option<(u32, Option<u32>)>) -> Vec<MpdSongInfo> { vec![] }
        fn playlist_song_by_id(&self, _id: u32) -> Option<MpdSongInfo> { None }
        fn add(&self, _uri: &str) -> Result<(), String> { Ok(()) }
        fn add_id(&self, _uri: &str, _pos: Option<u32>) -> Result<u32, String> { Ok(42) }
        fn delete(&self, _pos: u32) -> Result<(), String> { Ok(()) }
        fn clear(&self) -> Result<(), String> { Ok(()) }
        fn search(&self, _filters: &[FilterExpr], _exact: bool) -> Vec<MpdSongInfo> { vec![] }
        fn list_tag(&self, _tag: &str, _filters: &[FilterExpr]) -> Vec<String> { vec![] }
        fn lsinfo(&self, _path: Option<&str>) -> Vec<MpdDirEntry> { vec![] }
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
}
