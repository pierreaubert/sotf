use crate::federation_config::ServerConfig;
use crate::sotf_server_event::SotfServerEvent;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddr};

pub(super) fn sse_event_name(event: &SotfServerEvent) -> &'static str {
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

pub(super) fn sanitize_pairing_client_name(name: &str) -> String {
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

pub(super) fn log_sotf_api_request(
    method: &str,
    path: &str,
    peer_addr: SocketAddr,
    status: u16,
    elapsed: std::time::Duration,
) {
    let elapsed_ms = elapsed.as_secs_f64() * 1000.0;
    let path = redact_api_path_secrets(path);
    let line =
        format!("SOTF API {method} {path} -> {status} from {peer_addr} ({elapsed_ms:.1} ms)");
    log::info!("[server] {line}");
    eprintln!("{line}");
}

pub(super) fn redact_api_path_secrets(path: &str) -> String {
    let Some((route, query)) = path.split_once('?') else {
        return path.to_string();
    };
    let redacted = url::form_urlencoded::parse(query.as_bytes())
        .map(|(key, value)| {
            let value = if is_sensitive_query_key(&key) {
                "<redacted>".into()
            } else {
                value
            };
            (key, value)
        })
        .fold(
            url::form_urlencoded::Serializer::new(String::new()),
            |mut serializer, (key, value)| {
                serializer.append_pair(&key, &value);
                serializer
            },
        )
        .finish();
    format!("{route}?{redacted}")
}

fn is_sensitive_query_key(key: &str) -> bool {
    let key = key.trim().to_ascii_lowercase();
    key == "authorization"
        || key == "password"
        || key == "passwd"
        || key == "bearer"
        || key == "secret"
        || key == "api_key"
        || key == "apikey"
        || key == "client_secret"
        || key == "nonce"
        || key == "fingerprint"
        || key == "code"
        || key == "pin"
        || key.contains("api_key")
        || key.contains("api-key")
        || key.ends_with("_token")
        || key.ends_with("-token")
        || key.ends_with("_secret")
        || key.ends_with("-secret")
        || key.contains("token")
        || key.contains("nonce")
        || key.contains("fingerprint")
}

pub(super) fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|pos| pos + 4)
}

pub(super) fn mime_type_for_path(path: &std::path::Path) -> &'static str {
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

pub(super) fn media_track_id(
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

pub(super) fn is_safe_media_id(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

pub(super) fn hex_prefix(bytes: &[u8], chars: usize) -> String {
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

/// Detect a non-loopback local IPv4 address for DLNA announcements.
pub(super) fn get_local_ipv4() -> Ipv4Addr {
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

pub(super) fn invalid_configured_client_fingerprints(fingerprints: &[String]) -> Vec<String> {
    fingerprints
        .iter()
        .filter(|fingerprint| normalize_certificate_fingerprint(fingerprint).is_err())
        .cloned()
        .collect()
}

pub(super) fn initial_trusted_client_fingerprints(
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
