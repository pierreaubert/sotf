//! MPD streaming source.
//!
//! Plays a track from a remote MPD server by:
//! 1. Connecting to MPD's control port and telling it to play a specific file
//! 2. Connecting to MPD's httpd output to capture the audio stream
//! 3. Wrapping the HTTP stream as a Symphonia `MediaSource`
//!
//! URL format: `mpd-stream://host:control_port:httpd_port/path/to/file.flac`
//! Optional query: `?password=secret`

use crate::http_source::{HttpMediaSource, StreamMetadata};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::mpsc;
use std::time::Duration;
use symphonia_core::io::MediaSource;

/// Parse an `mpd-stream://` URL into its components.
///
/// Format: `mpd-stream://host:control_port:httpd_port/path/to/file.flac?password=secret`
#[derive(Debug, Clone)]
pub struct MpdStreamUrl {
    pub host: String,
    pub control_port: u16,
    pub httpd_port: u16,
    pub file_path: String,
    pub password: Option<String>,
}

impl MpdStreamUrl {
    pub fn parse(url: &str) -> Result<Self, String> {
        let rest = url
            .strip_prefix("mpd-stream://")
            .ok_or_else(|| format!("not an mpd-stream URL: {url}"))?;

        // Split off query string
        let (path_part, query) = if let Some(idx) = rest.find('?') {
            (&rest[..idx], Some(&rest[idx + 1..]))
        } else {
            (rest, None)
        };

        // Split host:control_port:httpd_port from /file/path
        let slash_idx = path_part
            .find('/')
            .ok_or_else(|| "missing file path in mpd-stream URL".to_string())?;
        let authority = &path_part[..slash_idx];
        let file_path = &path_part[slash_idx + 1..]; // strip leading /

        let parts: Vec<&str> = authority.split(':').collect();
        if parts.len() != 3 {
            return Err(format!(
                "expected host:control_port:httpd_port, got: {authority}"
            ));
        }
        let host = parts[0].to_string();
        let control_port: u16 = parts[1]
            .parse()
            .map_err(|_| format!("invalid control port: {}", parts[1]))?;
        let httpd_port: u16 = parts[2]
            .parse()
            .map_err(|_| format!("invalid httpd port: {}", parts[2]))?;

        let password = query.and_then(|q| {
            q.split('&').find_map(|kv| {
                let (k, v) = kv.split_once('=')?;
                if k == "password" {
                    Some(urlencoding_decode(v))
                } else {
                    None
                }
            })
        });

        let file_path = file_path.to_string();
        reject_mpd_control_chars(&file_path, "file_path")?;
        if let Some(ref pw) = password {
            reject_mpd_control_chars(pw, "password")?;
        }

        Ok(Self {
            host,
            control_port,
            httpd_port,
            file_path,
            password,
        })
    }
}

/// Simple percent-decoding (handles %XX sequences).
fn urlencoding_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
}

/// Reject any control char (0x00–0x1F, 0x7F) — MPD treats them as command
/// terminators or in-quote escapes and they let a hostile path/password
/// inject extra commands or break the connection.
fn reject_mpd_control_chars(s: &str, field: &str) -> Result<(), String> {
    for (i, b) in s.bytes().enumerate() {
        if b < 0x20 || b == 0x7F {
            return Err(format!(
                "{field} contains forbidden control char 0x{b:02X} at byte {i}"
            ));
        }
    }
    Ok(())
}

/// Quote a value for the MPD protocol: wrap in `"…"`, escaping `\` and `"`.
/// The caller MUST first run `reject_mpd_control_chars` so this can't be used
/// to inject newline-separated commands.
fn mpd_quote(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Tell MPD to play a specific track, then return the httpd stream URL.
///
/// This is a blocking operation (TCP connect + a few commands).
fn prepare_mpd_playback(parsed: &MpdStreamUrl) -> Result<(), String> {
    // Defence in depth: parsing may already have caught these (see
    // `MpdStreamUrl::parse`), but if any caller routes around the parser we
    // refuse to forward them to MPD here.
    reject_mpd_control_chars(&parsed.file_path, "file_path")?;
    if let Some(ref pw) = parsed.password {
        reject_mpd_control_chars(pw, "password")?;
    }
    let addr = format!("{}:{}", parsed.host, parsed.control_port);
    let stream = connect_mpd_control(&parsed.host, parsed.control_port, Duration::from_secs(5))
        .map_err(|e| format!("MPD connect failed ({addr}): {e}"))?;

    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

    let mut reader = BufReader::new(stream);

    // Read greeting
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| format!("MPD greeting read: {e}"))?;
    if !line.starts_with("OK MPD") {
        return Err(format!("unexpected MPD greeting: {}", line.trim()));
    }

    // Authenticate if needed
    if let Some(ref pw) = parsed.password {
        send_mpd(&mut reader, &format!("password {}", mpd_quote(pw)))?;
    }

    // Clear queue, add track, play
    send_mpd(&mut reader, "clear")?;
    send_mpd(
        &mut reader,
        &format!("add {}", mpd_quote(&parsed.file_path)),
    )?;
    send_mpd(&mut reader, "play")?;

    Ok(())
}

fn resolve_mpd_control_addrs(host: &str, port: u16) -> Result<Vec<SocketAddr>, String> {
    let addrs = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("resolve {host}:{port}: {e}"))?
        .collect::<Vec<_>>();

    if addrs.is_empty() {
        Err(format!("resolve {host}:{port}: no addresses found"))
    } else {
        Ok(addrs)
    }
}

fn connect_mpd_control(host: &str, port: u16, timeout: Duration) -> Result<TcpStream, String> {
    let addrs = resolve_mpd_control_addrs(host, port)?;
    let mut last_error = None;

    for addr in addrs {
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(stream) => return Ok(stream),
            Err(err) => last_error = Some(format!("{addr}: {err}")),
        }
    }

    Err(last_error.unwrap_or_else(|| format!("resolve {host}:{port}: no addresses found")))
}

/// Send a command and read until OK or ACK.
fn send_mpd(reader: &mut BufReader<TcpStream>, cmd: &str) -> Result<(), String> {
    let stream = reader.get_mut();
    stream
        .write_all(format!("{cmd}\n").as_bytes())
        .map_err(|e| format!("MPD send '{cmd}': {e}"))?;
    stream
        .flush()
        .map_err(|e| format!("MPD flush '{cmd}': {e}"))?;

    let mut response = String::new();
    loop {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| format!("MPD read '{cmd}': {e}"))?;
        if line.starts_with("OK") {
            return Ok(());
        }
        if line.starts_with("ACK") {
            return Err(format!("MPD error for '{cmd}': {}", line.trim()));
        }
        response.push_str(&line);
    }
}

/// A MediaSource that streams audio from an MPD server's httpd output.
///
/// On creation, it tells MPD to play the requested track, then connects
/// to the httpd stream and forwards audio data to Symphonia.
pub struct MpdStreamSource {
    inner: HttpMediaSource,
}

impl MpdStreamSource {
    /// Open an MPD stream source from an `mpd-stream://` URL.
    ///
    /// 1. Parses the URL to extract host, ports, file path, password
    /// 2. Connects to MPD control port and starts playback of the track
    /// 3. Connects to the httpd stream and wraps it as a MediaSource
    pub fn open(url: &str) -> Result<(Self, mpsc::Receiver<StreamMetadata>), String> {
        let parsed = MpdStreamUrl::parse(url)?;

        // Tell MPD to play the track
        prepare_mpd_playback(&parsed)?;

        // Small delay to let MPD start producing audio to the httpd output
        std::thread::sleep(Duration::from_millis(200));

        // Connect to the httpd stream
        let stream_url = format!("http://{}:{}/", parsed.host, parsed.httpd_port);
        let (http_source, metadata_rx) =
            HttpMediaSource::open(&stream_url).map_err(|e| format!("httpd stream: {e}"))?;

        Ok((Self { inner: http_source }, metadata_rx))
    }

    /// Hint for Symphonia format probing.
    pub fn format_hint(&self) -> Option<String> {
        self.inner.format_hint()
    }
}

impl Read for MpdStreamSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Seek for MpdStreamSource {
    fn seek(&mut self, _pos: SeekFrom) -> io::Result<u64> {
        // httpd streams are not seekable
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "MPD httpd stream is not seekable",
        ))
    }
}

impl MediaSource for MpdStreamSource {
    fn is_seekable(&self) -> bool {
        false
    }

    fn byte_len(&self) -> Option<u64> {
        None // continuous stream, unknown length
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mpd_stream_url() {
        let url = "mpd-stream://192.168.1.5:6600:6601/Music/Artist/Album/track.flac";
        let parsed = MpdStreamUrl::parse(url).unwrap();
        assert_eq!(parsed.host, "192.168.1.5");
        assert_eq!(parsed.control_port, 6600);
        assert_eq!(parsed.httpd_port, 6601);
        assert_eq!(parsed.file_path, "Music/Artist/Album/track.flac");
        assert_eq!(parsed.password, None);
    }

    #[test]
    fn test_parse_mpd_stream_url_with_password() {
        let url = "mpd-stream://myserver:6600:6601/path/file.mp3?password=s3cret";
        let parsed = MpdStreamUrl::parse(url).unwrap();
        assert_eq!(parsed.host, "myserver");
        assert_eq!(parsed.password, Some("s3cret".to_string()));
    }

    #[test]
    fn test_resolve_mpd_control_addrs_accepts_localhost() {
        let addrs = resolve_mpd_control_addrs("localhost", 6600).unwrap();

        assert!(addrs.iter().any(|addr| addr.port() == 6600));
    }

    #[test]
    fn test_parse_invalid_url() {
        assert!(MpdStreamUrl::parse("http://example.com").is_err());
        assert!(MpdStreamUrl::parse("mpd-stream://host:6600/file").is_err()); // missing httpd port
    }
}
