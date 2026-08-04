use super::handle::handle_content_directory;
use super::handle::handle_server_request;
use super::media_server_adapter::MediaServerAdapter;
use super::misc::parse_range_header;
use super::types::MediaAlbum;
use super::types::MediaSource;
use super::types::MediaTrack;
use crate::device::DlnaDevice;
use crate::gena::GenaRegistry;
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
    server_http_raw(adapter, request)
        .await
        .expect("request should succeed")
}

async fn server_http_raw(
    adapter: Arc<dyn MediaServerAdapter>,
    request: &str,
) -> Result<Vec<u8>, String> {
    server_http_raw_with_events(adapter, request, GenaRegistry::new()).await
}

async fn server_http_raw_with_events(
    adapter: Arc<dyn MediaServerAdapter>,
    request: &str,
    events: GenaRegistry,
) -> Result<Vec<u8>, String> {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let device = DlnaDevice::new_server("SOTF DLNA Test", addr.port());
    let base_url = format!("http://127.0.0.1:{}", addr.port());

    let task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        handle_server_request(stream, &device, &base_url, &adapter, &events).await
    });

    let mut client = TcpStream::connect(addr).await.unwrap();
    client.write_all(request.as_bytes()).await.unwrap();
    client.shutdown().await.unwrap();

    let mut response = Vec::new();
    client.read_to_end(&mut response).await.unwrap();
    match task.await.unwrap() {
        Ok(()) => Ok(response),
        Err(e) => Err(e),
    }
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

fn response_header(headers: &str, name: &str) -> Option<String> {
    headers.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name)
            .then(|| value.trim().to_string())
    })
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

#[tokio::test]
async fn http_server_exposes_scpd_and_gena_lifecycle() {
    let adapter: Arc<dyn MediaServerAdapter> = Arc::new(HttpStub {
        media_path: test_media_path(),
    });

    for (path, expected_action) in [
        ("/ContentDirectory/scpd.xml", "Browse"),
        ("/ConnectionManager/scpd.xml", "GetProtocolInfo"),
    ] {
        let response = server_http_round_trip(
            Arc::clone(&adapter),
            &format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n"),
        )
        .await;
        let (headers, body) = split_http_response(&response);
        assert!(headers.starts_with("HTTP/1.1 200 OK"), "got: {headers}");
        let body = String::from_utf8(body).unwrap();
        assert!(body.contains("<scpd"), "got: {body}");
        assert!(
            body.contains(&format!("<name>{expected_action}</name>")),
            "got: {body}"
        );
    }

    let callback_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let callback_addr = callback_listener.local_addr().unwrap();
    let notifications = tokio::spawn(async move {
        let mut requests = Vec::new();
        for _ in 0..2 {
            let (mut stream, _) = callback_listener.accept().await.unwrap();
            let mut request = Vec::new();
            stream.read_to_end(&mut request).await.unwrap();
            requests.push(request);
        }
        requests
    });

    let events = GenaRegistry::new();
    let subscribe = format!(
        "SUBSCRIBE /ContentDirectory/event HTTP/1.1\r\n\
         HOST: 127.0.0.1\r\n\
         CALLBACK: <http://{callback_addr}/events>\r\n\
         NT: upnp:event\r\n\
         TIMEOUT: Second-60\r\n\
         \r\n"
    );
    let response = server_http_raw_with_events(Arc::clone(&adapter), &subscribe, events.clone())
        .await
        .unwrap();
    let (headers, body) = split_http_response(&response);
    assert!(headers.starts_with("HTTP/1.1 200 OK"), "got: {headers}");
    assert!(body.is_empty());
    assert_eq!(
        response_header(&headers, "TIMEOUT").as_deref(),
        Some("Second-60")
    );
    let sid = response_header(&headers, "SID").expect("new subscription must return SID");
    // The registry is shared with the request handler, so a library update
    // reaches the same subscriber without another HTTP control request.
    events.notify(
        "/ContentDirectory/event",
        super::handle::content_directory_event_body(2),
    );
    let notifications = notifications.await.unwrap();
    assert_eq!(notifications.len(), 2);
    let first_notify = String::from_utf8(notifications[0].clone()).unwrap();
    assert!(
        first_notify.starts_with("NOTIFY /events HTTP/1.1"),
        "{first_notify}"
    );
    assert!(
        first_notify.contains(&format!("SID: {sid}")),
        "{first_notify}"
    );
    assert!(first_notify.contains("SEQ: 0"), "{first_notify}");
    assert!(
        first_notify.contains("<SystemUpdateID>1</SystemUpdateID>"),
        "{first_notify}"
    );
    let second_notify = String::from_utf8(notifications[1].clone()).unwrap();
    assert!(second_notify.contains("SEQ: 1"), "{second_notify}");
    assert!(
        second_notify.contains("<SystemUpdateID>2</SystemUpdateID>"),
        "{second_notify}"
    );

    let wrong_service_renewal = format!(
        "SUBSCRIBE /ConnectionManager/event HTTP/1.1\r\n\
         HOST: 127.0.0.1\r\n\
         SID: {sid}\r\n\
         TIMEOUT: Second-90\r\n\
         \r\n"
    );
    let response =
        server_http_raw_with_events(Arc::clone(&adapter), &wrong_service_renewal, events.clone())
            .await
            .unwrap();
    let (headers, _) = split_http_response(&response);
    assert!(
        headers.starts_with("HTTP/1.1 412 Precondition Failed"),
        "got: {headers}"
    );

    let renewal = format!(
        "SUBSCRIBE /ContentDirectory/event HTTP/1.1\r\n\
         HOST: 127.0.0.1\r\n\
         SID: {sid}\r\n\
         TIMEOUT: Second-90\r\n\
         \r\n"
    );
    let response = server_http_raw_with_events(Arc::clone(&adapter), &renewal, events.clone())
        .await
        .unwrap();
    let (headers, _) = split_http_response(&response);
    assert!(headers.starts_with("HTTP/1.1 200 OK"), "got: {headers}");
    assert_eq!(
        response_header(&headers, "SID").as_deref(),
        Some(sid.as_str())
    );
    assert_eq!(
        response_header(&headers, "TIMEOUT").as_deref(),
        Some("Second-90")
    );

    let unsubscribe = format!(
        "UNSUBSCRIBE /ContentDirectory/event HTTP/1.1\r\n\
         HOST: 127.0.0.1\r\n\
         SID: {sid}\r\n\
         \r\n"
    );
    let response = server_http_raw_with_events(adapter, &unsubscribe, events)
        .await
        .unwrap();
    let (headers, _) = split_http_response(&response);
    assert!(headers.starts_with("HTTP/1.1 200 OK"), "got: {headers}");
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

// ===== QA-SEC-006: request-line / header / URL abuse =====

#[tokio::test]
async fn rejects_single_oversize_header_line() {
    let adapter: Arc<dyn MediaServerAdapter> = Arc::new(SearchStub {
        rows: Vec::new(),
        total: 0,
    });
    let mut request = String::from("GET /description.xml HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Huge: ");
    request.extend(std::iter::repeat_n(
        'A',
        crate::http_io::MAX_HEADER_LINE + 16,
    ));
    request.push_str("\r\n\r\n");

    let err = server_http_raw(adapter, &request).await.unwrap_err();
    assert!(
        err.contains("header line exceeds cap"),
        "expected cap error, got: {}",
        err
    );
}

#[tokio::test]
async fn rejects_path_traversal_in_media_url() {
    let adapter: Arc<dyn MediaServerAdapter> = Arc::new(HttpStub {
        media_path: test_media_path(),
    });
    let response = server_http_round_trip(
        adapter,
        "GET /media/../../etc/passwd HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
    )
    .await;
    let (headers, body) = split_http_response(&response);
    assert!(
        headers.starts_with("HTTP/1.1 400 Bad Request"),
        "got: {}",
        headers
    );
    assert!(
        body == b"Bad media id",
        "response body must not reveal filesystem path, got: {:?}",
        String::from_utf8_lossy(&body)
    );
}

#[tokio::test]
async fn media_404_does_not_leak_filesystem_path() {
    let adapter: Arc<dyn MediaServerAdapter> = Arc::new(HttpStub {
        media_path: std::env::temp_dir().join("sotf-dlna-secret-path.flac"),
    });
    let response = server_http_round_trip(
        adapter,
        "GET /media/track-1 HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
    )
    .await;
    let (headers, body) = split_http_response(&response);
    assert!(
        headers.starts_with("HTTP/1.1 404 Not Found"),
        "got: {}",
        headers
    );
    let body_text = String::from_utf8_lossy(&body);
    assert!(
        !body_text.contains("secret-path"),
        "error body must not leak path, got: {}",
        body_text
    );
}
