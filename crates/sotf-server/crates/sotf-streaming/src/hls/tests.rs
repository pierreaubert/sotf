use super::hls_byte_range::HlsByteRange;
use super::hls_segment::HlsSegment;
use super::hls_source::HlsSource;
use super::parse;
use super::parse::parse_master_playlist;
use super::parse::parse_media_playlist;
use super::resolve;
use super::types::PendingByteRange;
use std::io::{self, Read};
use std::time::Duration;
use url::Url;

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn base_url(path: &str) -> Url {
    Url::parse(&format!("http://example.test/{}", path)).unwrap()
}

#[test]
fn parses_media_playlist_with_relative_segments() {
    let playlist = "#EXTM3U\n#EXT-X-TARGETDURATION:6\n#EXTINF:6,\nseg0.aac\n#EXTINF:6,\nsub/seg1.aac\n#EXT-X-ENDLIST\n";
    let parsed = parse_media_playlist(&base_url("hls/live/index.m3u8"), playlist).unwrap();

    assert!(parsed.end_list);
    assert_eq!(parsed.target_duration, Duration::from_secs(6));
    assert_eq!(parsed.segments.len(), 2);
    assert_eq!(
        parsed.segments[0].url.as_str(),
        "http://example.test/hls/live/seg0.aac"
    );
    assert_eq!(
        parsed.segments[1].url.as_str(),
        "http://example.test/hls/live/sub/seg1.aac"
    );
}

#[test]
fn parses_media_playlist_with_init_map_and_byte_ranges() {
    let playlist = "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MAP:URI=\"init.mp4\",BYTERANGE=\"720@0\"\n#EXTINF:4,\n#EXT-X-BYTERANGE:1000@720\ntrack.m4s\n#EXTINF:4,\n#EXT-X-BYTERANGE:900\ntrack.m4s\n#EXT-X-ENDLIST\n";
    let parsed = parse_media_playlist(&base_url("hls/fmp4/index.m3u8"), playlist).unwrap();

    assert_eq!(parsed.segments.len(), 3);
    assert_eq!(
        parsed.segments[0].url.as_str(),
        "http://example.test/hls/fmp4/init.mp4"
    );
    assert_eq!(
        parsed.segments[0].byte_range,
        Some(HlsByteRange {
            offset: 0,
            length: 720
        })
    );
    assert_eq!(
        parsed.segments[1].byte_range,
        Some(HlsByteRange {
            offset: 720,
            length: 1000
        })
    );
    assert_eq!(
        parsed.segments[2].byte_range,
        Some(HlsByteRange {
            offset: 1720,
            length: 900
        })
    );
}

#[test]
fn rejects_encrypted_media_segments() {
    let playlist = "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\"\n#EXTINF:4,\nseg0.aac\n";
    let err = parse_media_playlist(&base_url("hls/live/index.m3u8"), playlist).unwrap_err();

    assert_eq!(err.kind(), io::ErrorKind::Unsupported);
}

#[test]
fn parses_master_playlist_by_highest_bandwidth() {
    let playlist = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=64000\nlo/index.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=192000\nhi/index.m3u8\n";
    let selected = parse_master_playlist(&base_url("root/master.m3u8"), playlist)
        .unwrap()
        .unwrap();

    assert_eq!(selected.as_str(), "http://example.test/root/hi/index.m3u8");
}

#[test]
fn hls_source_reads_playlist_segments_in_order() {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            eprintln!("skipping HLS loopback integration test: {err}");
            return;
        }
        Err(err) => panic!("failed to bind HLS test listener: {err}"),
    };
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let requests_for_thread = Arc::clone(&requests);

    let server = std::thread::spawn(move || {
        for _ in 0..3 {
            let (mut stream, _) = listener.accept().unwrap();
            handle_test_request(&mut stream);
            requests_for_thread.fetch_add(1, Ordering::SeqCst);
        }
    });

    let url = format!("http://{}/playlist.m3u8", addr);
    let mut source = HlsSource::open(&url).unwrap();
    let mut bytes = Vec::new();
    source.read_to_end(&mut bytes).unwrap();

    assert_eq!(bytes, b"helloworld");
    assert_eq!(source.format_hint(), Some("aac".to_string()));
    server.join().unwrap();
    assert_eq!(requests.load(Ordering::SeqCst), 3);
}

fn handle_test_request(stream: &mut TcpStream) {
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).unwrap();
    let request = String::from_utf8_lossy(&buf[..n]);
    let (status, content_type, body) = if request.contains("GET /playlist.m3u8") {
        (
                "200 OK",
                "application/vnd.apple.mpegurl",
                "#EXTM3U\n#EXT-X-TARGETDURATION:1\n#EXTINF:1,\nseg0.aac\n#EXTINF:1,\nseg1.aac\n#EXT-X-ENDLIST\n"
                    .as_bytes()
                    .to_vec(),
            )
    } else if request.contains("GET /seg0.aac") {
        ("200 OK", "audio/aac", b"hello".to_vec())
    } else if request.contains("GET /seg1.aac") {
        ("200 OK", "audio/aac", b"world".to_vec())
    } else {
        ("404 Not Found", "text/plain", b"missing".to_vec())
    };

    let header = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        content_type,
        body.len()
    );
    stream.write_all(header.as_bytes()).unwrap();
    stream.write_all(&body).unwrap();
}

// ----- Additional HLS parser coverage -----

#[test]
fn parse_attribute_basic() {
    let line = r#"#EXT-X-STREAM-INF:BANDWIDTH=128000,CODECS="mp4a.40.2""#;
    assert_eq!(parse::parse_attribute(line, "BANDWIDTH"), Some("128000"));
    assert_eq!(parse::parse_attribute(line, "CODECS"), Some("mp4a.40.2"));
    assert_eq!(parse::parse_attribute(line, "MISSING"), None);
}

#[test]
fn parse_attribute_quotes_with_commas() {
    let line = r#"#EXT-X-KEY:METHOD=AES-128,URI="http://example.com/key?a=1,b=2",IV=0x1234"#;
    assert_eq!(parse::parse_attribute(line, "METHOD"), Some("AES-128"));
    assert_eq!(
        parse::parse_attribute(line, "URI"),
        Some("http://example.com/key?a=1,b=2")
    );
    assert_eq!(parse::parse_attribute(line, "IV"), Some("0x1234"));
}

#[test]
fn parse_byte_range_with_offset() {
    let range = parse::parse_byte_range("1024@4096").unwrap();
    assert_eq!(range.length, 1024);
    assert_eq!(range.offset, Some(4096));
}

#[test]
fn parse_byte_range_without_offset() {
    let range = parse::parse_byte_range("2048").unwrap();
    assert_eq!(range.length, 2048);
    assert_eq!(range.offset, None);
}

#[test]
fn parse_byte_range_rejects_zero_length() {
    let err = parse::parse_byte_range("0").unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn parse_byte_range_rejects_invalid_length() {
    let err = parse::parse_byte_range("abc").unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn parse_byte_range_rejects_invalid_offset() {
    let err = parse::parse_byte_range("100@abc").unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn parse_byte_range_quoted() {
    let range = parse::parse_byte_range("\"512@128\"").unwrap();
    assert_eq!(range.length, 512);
    assert_eq!(range.offset, Some(128));
}

#[test]
fn hls_byte_range_end_exclusive() {
    let range = HlsByteRange {
        offset: 100,
        length: 50,
    };
    assert_eq!(range.end_exclusive().unwrap(), 150);
    assert_eq!(range.header_value().unwrap(), "bytes=100-149");
}

#[test]
fn hls_byte_range_overflow_rejected() {
    let range = HlsByteRange {
        offset: u64::MAX,
        length: 1,
    };
    assert!(range.end_exclusive().is_err());
    assert!(range.header_value().is_err());
}

#[test]
fn parse_master_playlist_fallback_parses_stream_inf() {
    // Not a valid MasterPlaylist per m3u8_rs, so falls back to manual parsing.
    let playlist = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=256000\nmid/index.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=512000\nhi/index.m3u8\n";
    let selected = parse::parse_master_playlist(&base_url("root/master.m3u8"), playlist)
        .unwrap()
        .unwrap();
    assert_eq!(selected.as_str(), "http://example.test/root/hi/index.m3u8");
}

#[test]
fn parse_master_playlist_skips_i_frame_variants() {
    let playlist = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,VIDEO=\"main\"\nhi/video.m3u8\n#EXT-X-I-FRAME-STREAM-INF:BANDWIDTH=50000,URI=\"iframe.m3u8\"\n";
    let selected = parse::parse_master_playlist(&base_url("root/master.m3u8"), playlist)
        .unwrap()
        .unwrap();
    assert_eq!(selected.as_str(), "http://example.test/root/hi/video.m3u8");
}

#[test]
fn parse_master_playlist_empty_returns_none() {
    let playlist = "#EXTM3U\n";
    assert!(
        parse::parse_master_playlist(&base_url("root/master.m3u8"), playlist)
            .unwrap()
            .is_none()
    );
}

#[test]
fn parse_media_playlist_empty_segments_rejected() {
    let playlist = "#EXTM3U\n#EXT-X-TARGETDURATION:6\n";
    let err = parse::parse_media_playlist(&base_url("hls/live/index.m3u8"), playlist).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn parse_media_playlist_default_target_duration() {
    let playlist = "#EXTM3U\n#EXTINF:6,\nseg0.aac\n";
    let parsed = parse::parse_media_playlist(&base_url("hls/live/index.m3u8"), playlist).unwrap();
    assert_eq!(parsed.target_duration, Duration::from_secs(4));
}

#[test]
fn parse_media_playlist_unencrypted_key_is_ok() {
    let playlist =
        "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-KEY:METHOD=NONE\n#EXTINF:4,\nseg0.aac\n";
    let parsed = parse::parse_media_playlist(&base_url("hls/live/index.m3u8"), playlist).unwrap();
    assert_eq!(parsed.segments.len(), 1);
}

#[test]
fn segment_key_distinguishes_byte_ranges() {
    let url = base_url("seg.aac");
    let seg1 = HlsSegment::new(
        url.clone(),
        Some(HlsByteRange {
            offset: 0,
            length: 100,
        }),
    );
    let seg2 = HlsSegment::new(
        url.clone(),
        Some(HlsByteRange {
            offset: 100,
            length: 100,
        }),
    );
    let seg3 = HlsSegment::new(url.clone(), None);
    assert_ne!(seg1.key(), seg2.key());
    assert_ne!(seg1.key(), seg3.key());
    assert_eq!(seg1.key(), seg1.key());
}

#[test]
fn resolve_byte_range_uses_last_end_when_offset_missing() {
    let mut last_end: Option<u64> = Some(1000);
    let pending = PendingByteRange {
        length: 500,
        offset: None,
    };
    let range = resolve::resolve_byte_range(pending, &mut last_end).unwrap();
    assert_eq!(range.offset, 1000);
    assert_eq!(range.length, 500);
    assert_eq!(last_end, Some(1500));
}

#[test]
fn resolve_byte_range_zero_start_when_no_last_end() {
    let mut last_end: Option<u64> = None;
    let pending = PendingByteRange {
        length: 100,
        offset: None,
    };
    let range = resolve::resolve_byte_range(pending, &mut last_end).unwrap();
    assert_eq!(range.offset, 0);
    assert_eq!(last_end, Some(100));
}

#[test]
fn parse_media_playlist_rejects_segment_count_bomb() {
    // A malicious playlist could still be within the byte cap while declaring
    // thousands of tiny segments. The parser must reject playlists that exceed
    // MAX_SEGMENTS to avoid unbounded Vec allocation.
    let mut playlist = String::from("#EXTM3U\n#EXT-X-TARGETDURATION:1\n");
    for i in 0..(super::consts::MAX_SEGMENTS + 1) {
        playlist.push_str("#EXTINF:1,\n");
        playlist.push_str(&format!("seg{i}.aac\n"));
    }
    let err = parse::parse_media_playlist(&base_url("hls/live/index.m3u8"), &playlist).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("maximum segment count"),
        "got: {}",
        err
    );
}

#[test]
fn parse_media_playlist_rejects_byte_range_overflow() {
    // offset + length must not overflow u64.
    let playlist = "#EXTM3U\n#EXT-X-TARGETDURATION:1\n#EXT-X-BYTERANGE:1@18446744073709551615\n#EXTINF:1,\nseg.aac\n";
    let err = parse::parse_media_playlist(&base_url("hls/live/index.m3u8"), playlist).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("overflow"),
        "expected overflow error, got: {}",
        err
    );
}
