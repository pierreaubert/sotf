use super::handle::handle_content_directory;
use super::handle::handle_server_request;
use super::media_server_adapter::MediaServerAdapter;
use super::misc::parse_range_header;
use super::types::MediaAlbum;
use super::types::MediaSource;
use super::types::MediaTrack;
use crate::device::DlnaDevice;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Adapter that misbehaves by ignoring the `count` argument — used to
/// prove the response-side truncation kicks in.
struct SearchStub {
    rows: Vec<MediaTrack>,
    total: u32,
}

impl MediaServerAdapter for SearchStub {
    fn browse_albums(&self, _start: u32, _count: u32) -> (Vec<MediaAlbum>, u32) {
        (Vec::new(), 0)
    }
    fn browse_album_tracks(&self, _album_id: &str) -> Vec<MediaTrack> {
        Vec::new()
    }
    fn search_tracks(&self, _query: &str, _start: u32, _count: u32) -> (Vec<MediaTrack>, u32) {
        (self.rows.iter().map(clone_track).collect(), self.total)
    }
    fn album_count(&self) -> u32 {
        0
    }
}

struct HttpStub {
    media_path: PathBuf,
}

impl MediaServerAdapter for HttpStub {
    fn browse_albums(&self, start: u32, count: u32) -> (Vec<MediaAlbum>, u32) {
        let albums = vec![MediaAlbum {
            id: "album-1".to_string(),
            title: "HTTP Album".to_string(),
            artist: "SOTF".to_string(),
            year: None,
            track_count: 1,
        }];
        let total = albums.len() as u32;
        let rows = albums
            .into_iter()
            .skip(start as usize)
            .take(count as usize)
            .collect();
        (rows, total)
    }

    fn browse_album_tracks(&self, album_id: &str) -> Vec<MediaTrack> {
        if album_id == "album-1" {
            vec![make_track("track-1")]
        } else {
            Vec::new()
        }
    }

    fn search_tracks(&self, _query: &str, start: u32, count: u32) -> (Vec<MediaTrack>, u32) {
        let tracks = vec![make_track("track-1")];
        let total = tracks.len() as u32;
        let rows = tracks
            .into_iter()
            .skip(start as usize)
            .take(count as usize)
            .collect();
        (rows, total)
    }

    fn album_count(&self) -> u32 {
        1
    }

    fn media_path(&self, track_id: &str) -> Option<MediaSource> {
        (track_id == "track-1").then(|| MediaSource {
            path: self.media_path.clone(),
            mime_type: "audio/flac".to_string(),
        })
    }
}

fn clone_track(t: &MediaTrack) -> MediaTrack {
    MediaTrack {
        id: t.id.clone(),
        album_id: t.album_id.clone(),
        title: t.title.clone(),
        artist: t.artist.clone(),
        album: t.album.clone(),
        genre: t.genre.clone(),
        track_number: t.track_number,
        duration_secs: t.duration_secs,
        file_path: t.file_path.clone(),
        mime_type: t.mime_type.clone(),
        sample_rate: t.sample_rate,
        channels: t.channels,
        bit_depth: t.bit_depth,
        file_size: t.file_size,
    }
}

fn make_track(id: &str) -> MediaTrack {
    MediaTrack {
        id: id.to_string(),
        album_id: "a".to_string(),
        title: id.to_string(),
        artist: "x".to_string(),
        album: "x".to_string(),
        genre: None,
        track_number: None,
        duration_secs: None,
        file_path: String::new(),
        mime_type: "audio/flac".to_string(),
        sample_rate: None,
        channels: None,
        bit_depth: None,
        file_size: None,
    }
}

async fn server_http_round_trip(adapter: Arc<dyn MediaServerAdapter>, request: &str) -> Vec<u8> {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let device = DlnaDevice::new_server("SOTF DLNA Test", addr.port());
    let base_url = format!("http://127.0.0.1:{}", addr.port());

    let task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        handle_server_request(stream, &device, &base_url, &adapter)
            .await
            .unwrap();
    });

    let mut client = TcpStream::connect(addr).await.unwrap();
    client.write_all(request.as_bytes()).await.unwrap();
    client.shutdown().await.unwrap();

    let mut response = Vec::new();
    client.read_to_end(&mut response).await.unwrap();
    task.await.unwrap();
    response
}

fn split_http_response(response: &[u8]) -> (String, Vec<u8>) {
    let separator = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap();
    let headers = String::from_utf8(response[..separator].to_vec()).unwrap();
    let body = response[(separator + 4)..].to_vec();
    (headers, body)
}

fn test_media_path() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "sotf-dlna-http-test-{}-{}.flac",
        std::process::id(),
        std::thread::current().name().unwrap_or("unnamed")
    ));
    path
}

#[tokio::test]
async fn http_server_serves_description_content_directory_and_media_ranges() {
    let path = test_media_path();
    tokio::fs::write(&path, b"0123456789").await.unwrap();
    let adapter: Arc<dyn MediaServerAdapter> = Arc::new(HttpStub {
        media_path: path.clone(),
    });

    let description = server_http_round_trip(
        Arc::clone(&adapter),
        "GET /description.xml HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
    )
    .await;
    let (headers, body) = split_http_response(&description);
    assert!(headers.starts_with("HTTP/1.1 200 OK"), "got: {}", headers);
    let body = String::from_utf8(body).unwrap();
    assert!(body.contains("SOTF DLNA Test"), "got: {}", body);
    assert!(body.contains("ContentDirectory"), "got: {}", body);

    let browse = r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
  <s:Body>
    <u:Browse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
      <ObjectID>album-1</ObjectID>
      <BrowseFlag>BrowseDirectChildren</BrowseFlag>
      <Filter>*</Filter>
      <StartingIndex>0</StartingIndex>
      <RequestedCount>10</RequestedCount>
      <SortCriteria></SortCriteria>
    </u:Browse>
  </s:Body>
</s:Envelope>"#;
    let request = format!(
        "POST /ContentDirectory/control HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\n\r\n{}",
        browse.len(),
        browse
    );
    let content_directory = server_http_round_trip(Arc::clone(&adapter), &request).await;
    let (headers, body) = split_http_response(&content_directory);
    assert!(headers.starts_with("HTTP/1.1 200 OK"), "got: {}", headers);
    let body = String::from_utf8(body).unwrap();
    assert!(
        body.contains("<NumberReturned>1</NumberReturned>"),
        "got: {}",
        body
    );
    assert!(body.contains("/media/track-1"), "got: {}", body);

    let head = server_http_round_trip(
        Arc::clone(&adapter),
        "HEAD /media/track-1 HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
    )
    .await;
    let (headers, body) = split_http_response(&head);
    assert!(headers.starts_with("HTTP/1.1 200 OK"), "got: {}", headers);
    assert!(headers.contains("Content-Length: 10"), "got: {}", headers);
    assert!(body.is_empty(), "HEAD returned body: {:?}", body);

    let partial = server_http_round_trip(
        Arc::clone(&adapter),
        "GET /media/track-1 HTTP/1.1\r\nHost: 127.0.0.1\r\nRange: bytes=2-5\r\n\r\n",
    )
    .await;
    let (headers, body) = split_http_response(&partial);
    assert!(
        headers.starts_with("HTTP/1.1 206 Partial Content"),
        "got: {}",
        headers
    );
    assert!(
        headers.contains("Content-Range: bytes 2-5/10"),
        "got: {}",
        headers
    );
    assert_eq!(body, b"2345");

    let _ = tokio::fs::remove_file(path).await;
}

/// Review requirement: `Search` must honour `RequestedCount` on the
/// response side even when the adapter misbehaves.
#[test]
fn search_truncates_response_to_requested_count() {
    let stub: Arc<dyn MediaServerAdapter> = Arc::new(SearchStub {
        rows: (0..50).map(|i| make_track(&format!("t{}", i))).collect(),
        total: 1000,
    });
    let soap = r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
  <s:Body>
    <u:Search xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
      <ContainerID>0</ContainerID>
      <SearchCriteria>*</SearchCriteria>
      <Filter>*</Filter>
      <StartingIndex>0</StartingIndex>
      <RequestedCount>5</RequestedCount>
      <SortCriteria></SortCriteria>
    </u:Search>
  </s:Body>
</s:Envelope>"#;
    let resp = handle_content_directory(soap, &stub, "http://1.2.3.4:80");
    assert!(
        resp.contains("<NumberReturned>5</NumberReturned>"),
        "got: {}",
        resp
    );
    assert!(
        resp.contains("<TotalMatches>1000</TotalMatches>"),
        "got: {}",
        resp
    );
}

#[test]
fn parses_byte_ranges() {
    assert_eq!(parse_range_header(None, 100), Ok(None));
    assert_eq!(parse_range_header(Some("bytes=0-9"), 100), Ok(Some((0, 9))));
    assert_eq!(
        parse_range_header(Some("bytes=10-"), 100),
        Ok(Some((10, 99)))
    );
    assert_eq!(
        parse_range_header(Some("bytes=-10"), 100),
        Ok(Some((90, 99)))
    );
    assert_eq!(
        parse_range_header(Some("bytes=95-200"), 100),
        Ok(Some((95, 99)))
    );
}

#[test]
fn compute_body_len_u64_max_no_panic() {
    assert_eq!(super::handle::compute_body_len(100, 0, u64::MAX), u64::MAX);
}

#[test]
fn rejects_invalid_byte_ranges() {
    assert!(parse_range_header(Some("items=0-9"), 100).is_err());
    assert!(parse_range_header(Some("bytes=50-40"), 100).is_err());
    assert!(parse_range_header(Some("bytes=100-101"), 100).is_err());
    assert!(parse_range_header(Some("bytes=0-1,4-5"), 100).is_err());
    assert!(parse_range_header(Some("bytes=-0"), 100).is_err());
    assert!(parse_range_header(Some("bytes=0-1"), 0).is_err());
}
