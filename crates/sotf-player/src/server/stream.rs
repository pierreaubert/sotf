use super::api::api_error_response;
use super::api::api_media_source;
use super::api::api_parse_range_header;
use super::api::api_state_json;
use super::misc::sse_event_name;
use super::mpd_player_adapter::MpdPlayerAdapter;
use super::server_state::ServerState;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub(super) async fn stream_api_media(
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

pub(super) async fn stream_api_media_file(
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
pub(super) async fn stream_api_events(
    stream: &mut TcpStream,
    state: &Arc<ServerState>,
) -> Result<(), String> {
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
