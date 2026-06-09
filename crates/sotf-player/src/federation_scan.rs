//! Federation source scanning and connection diagnostics.
//!
//! Shared business logic used by both GPUI and TUI apps.

use crate::database::MusicDatabase;
use crate::federation_config::{
    ConnectionDiagnostic, ConnectionStatus, FederationSourceEntry, SourceConnectionConfig,
    StepResult,
};
use crate::sotf_api_client::{SotfApiAlbum, SotfApiAlbumTracks, SotfApiClient, SotfApiClientError};
use sotf_audio::decoder::AudioSource;
use sotf_federation::{
    DlnaProvider, DlnaProviderConfig, LibraryProvider, MpdProvider, MpdProviderConfig,
    ProviderAlbum, ProviderTrack, SourceId,
};
use std::sync::atomic::{AtomicBool, Ordering};

// ─── Scan result ─────────────────────────────────────────────────────────────

/// Result of a federation source scan.
#[derive(Debug)]
pub struct FederationScanResult {
    pub source_id: String,
    pub albums: usize,
    pub tracks: usize,
    pub error: Option<String>,
}

/// Progress callback invoked after each album is merged.
pub type ScanProgressFn = Box<dyn Fn(usize, usize) + Send>;

// ─── Scan pipeline ───────────────────────────────────────────────────────────

/// Fetch albums from a federation source provider.
/// Returns the provider albums or an error wrapped in `FederationScanResult`.
pub async fn fetch_source_albums(
    source: &FederationSourceEntry,
) -> Result<Vec<ProviderAlbum>, FederationScanResult> {
    let source_id_str = source.source_id.clone();

    match &source.connection {
        SourceConnectionConfig::Mpd {
            host,
            port,
            password,
            httpd_port,
            ..
        } => {
            let config = MpdProviderConfig {
                host: host.clone(),
                port: *port,
                password: password.clone(),
                httpd_port: *httpd_port,
            };
            let provider = MpdProvider::new(SourceId(source_id_str.clone()), config);
            provider
                .fetch_all_albums()
                .await
                .map_err(|e| FederationScanResult {
                    source_id: source_id_str,
                    albums: 0,
                    tracks: 0,
                    error: Some(format!("failed to fetch albums: {e}")),
                })
        }
        SourceConnectionConfig::Dlna {
            location_url,
            friendly_name,
        } => {
            let url = location_url.clone().ok_or_else(|| FederationScanResult {
                source_id: source_id_str.clone(),
                albums: 0,
                tracks: 0,
                error: Some("no DLNA location URL configured".to_string()),
            })?;
            let config = DlnaProviderConfig {
                location_url: url,
                friendly_name: friendly_name.clone().unwrap_or_default(),
            };
            let provider = DlnaProvider::new(SourceId(source_id_str.clone()), config);
            provider
                .fetch_all_albums()
                .await
                .map_err(|e| FederationScanResult {
                    source_id: source_id_str,
                    albums: 0,
                    tracks: 0,
                    error: Some(format!("failed to fetch albums: {e}")),
                })
        }
        SourceConnectionConfig::Peer {
            host,
            port,
            auth_token,
            ..
        } => {
            let token = auth_token
                .as_deref()
                .map(str::trim)
                .filter(|token| !token.is_empty())
                .ok_or_else(|| FederationScanResult {
                    source_id: source_id_str.clone(),
                    albums: 0,
                    tracks: 0,
                    error: Some("SOTF API token is required for Peer sources".to_string()),
                })?;
            let client =
                sotf_peer_client(host, *port, token).map_err(|e| FederationScanResult {
                    source_id: source_id_str.clone(),
                    albums: 0,
                    tracks: 0,
                    error: Some(format!("invalid SOTF API peer config: {e}")),
                })?;
            fetch_sotf_peer_albums(&client)
                .await
                .map_err(|e| FederationScanResult {
                    source_id: source_id_str,
                    albums: 0,
                    tracks: 0,
                    error: Some(format!("failed to fetch SOTF peer albums: {e}")),
                })
        }
        other => Err(FederationScanResult {
            source_id: source_id_str,
            albums: 0,
            tracks: 0,
            error: Some(format!(
                "{} provider not yet implemented",
                other.type_name()
            )),
        }),
    }
}

/// Merge fetched albums into the local database.
///
/// Opens a secondary DB connection, clears old data for this source,
/// then merges all albums and tracks. Calls `on_progress(albums_merged, tracks_merged)`
/// after each album. Checks `cancel` between albums.
pub fn merge_albums_to_db(
    source_id: &str,
    albums: &[ProviderAlbum],
    cancel: &AtomicBool,
    on_progress: Option<&ScanProgressFn>,
) -> FederationScanResult {
    // Open a secondary DB connection on this background thread
    let db = match MusicDatabase::default_path() {
        Some(path) => match MusicDatabase::open_secondary(&path) {
            Ok(db) => db,
            Err(e) => {
                return FederationScanResult {
                    source_id: source_id.to_string(),
                    albums: 0,
                    tracks: 0,
                    error: Some(format!("failed to open database: {e}")),
                };
            }
        },
        None => {
            return FederationScanResult {
                source_id: source_id.to_string(),
                albums: 0,
                tracks: 0,
                error: Some("no database path configured".to_string()),
            };
        }
    };

    // Clear previous data for this source before full resync
    if let Err(e) = db.remove_exclusive_federation_tracks(source_id) {
        log::warn!("Failed to remove exclusive federation tracks: {e}");
    }
    if let Err(e) = db.clear_federation_source_data(source_id) {
        log::warn!("Failed to clear federation source data: {e}");
    }
    if let Err(e) = db.remove_orphaned_tracks() {
        log::warn!("Failed to remove orphaned tracks: {e}");
    }
    if let Err(e) = db.remove_orphaned_albums() {
        log::warn!("Failed to remove orphaned albums: {e}");
    }

    let mut album_count = 0;
    let mut track_count = 0;

    for album in albums {
        if cancel.load(Ordering::Relaxed) {
            return FederationScanResult {
                source_id: source_id.to_string(),
                albums: album_count,
                tracks: track_count,
                error: Some("cancelled".to_string()),
            };
        }

        match db.merge_federation_album(source_id, album) {
            Ok(album_id) => {
                album_count += 1;
                for track in &album.tracks {
                    match db.merge_federation_track(source_id, album_id, track) {
                        Ok(_) => track_count += 1,
                        Err(e) => log::warn!("Failed to merge track '{}': {e}", track.title),
                    }
                }
            }
            Err(e) => log::warn!("Failed to merge album '{}': {e}", album.title),
        }

        if let Some(cb) = on_progress {
            cb(album_count, track_count);
        }
    }

    log::info!(
        "Federation scan of '{source_id}' complete: {album_count} albums, {track_count} tracks merged"
    );

    FederationScanResult {
        source_id: source_id.to_string(),
        albums: album_count,
        tracks: track_count,
        error: None,
    }
}

// ─── Connection diagnostics ──────────────────────────────────────────────────

/// Run a structured diagnostic test against a federation source.
/// Tests each layer: DNS -> TCP -> TLS/Protocol.
/// Blocking — creates its own tokio runtime.
pub fn run_connection_diagnostic(source: &FederationSourceEntry) -> ConnectionStatus {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async {
        match &source.connection {
            SourceConnectionConfig::Mpd {
                host,
                port,
                password,
                ..
            } => {
                let diag = diagnose_mpd(host, *port, password.as_deref()).await;
                ConnectionStatus::Diagnostic(diag)
            }
            SourceConnectionConfig::Subsonic { url, .. } => {
                let diag = diagnose_tcp_from_url(url).await;
                ConnectionStatus::Diagnostic(diag)
            }
            SourceConnectionConfig::Dlna { location_url, .. } => {
                if let Some(url) = location_url {
                    let diag = diagnose_tcp_from_url(url).await;
                    ConnectionStatus::Diagnostic(diag)
                } else {
                    ConnectionStatus::Error("No DLNA location configured".to_string())
                }
            }
            SourceConnectionConfig::Peer {
                host,
                port,
                accepted_fingerprint,
                auth_token,
            } => {
                let diag = diagnose_sotf_peer(
                    host,
                    *port,
                    accepted_fingerprint.as_deref(),
                    auth_token.as_deref(),
                )
                .await;
                ConnectionStatus::Diagnostic(diag)
            }
            SourceConnectionConfig::Tidal { .. } => {
                let diag = diagnose_tcp_simple("api.tidal.com", 443).await;
                ConnectionStatus::Diagnostic(diag)
            }
            SourceConnectionConfig::Spotify { .. } => {
                let diag = diagnose_tcp_simple("ap.spotify.com", 443).await;
                ConnectionStatus::Diagnostic(diag)
            }
            SourceConnectionConfig::IcyRadio { url, .. } => {
                if url.is_empty() {
                    ConnectionStatus::Error("No stream URL configured".to_string())
                } else {
                    let diag = diagnose_tcp_from_url(url).await;
                    ConnectionStatus::Diagnostic(diag)
                }
            }
        }
    })
}

async fn fetch_sotf_peer_albums(
    client: &SotfApiClient,
) -> Result<Vec<ProviderAlbum>, SotfApiClientError> {
    const PAGE_LIMIT: usize = 250;
    let mut offset = 0;
    let mut albums = Vec::new();

    loop {
        let page = client
            .library_albums_page(offset, PAGE_LIMIT, None, Some("artist_title"))
            .await?;
        let page_len = page.albums.len();
        for album in page.albums {
            let tracks = client.album_tracks(&album.id).await?;
            albums.push(sotf_api_album_to_provider_album(client, album, tracks)?);
        }
        offset = offset.saturating_add(page_len);
        if page_len == 0 || offset >= page.total {
            break;
        }
    }

    Ok(albums)
}

fn sotf_api_album_to_provider_album(
    client: &SotfApiClient,
    album: SotfApiAlbum,
    tracks: SotfApiAlbumTracks,
) -> Result<ProviderAlbum, SotfApiClientError> {
    let provider_tracks = tracks
        .tracks
        .into_iter()
        .map(|track| {
            let media_url = client.authenticated_media_url(&track.id)?;
            Ok(ProviderTrack {
                external_id: track.id,
                title: track.title.unwrap_or_else(|| "Unknown Track".to_string()),
                artist: track.artist,
                album_artist: Some(album.artist.clone()),
                track_number: track.track,
                disc_number: track.disc_number,
                duration_secs: track.duration_secs.map(|duration| duration as f64),
                genre: track.genre,
                composer: track.composer,
                channels: track.channels,
                sample_rate: track.sample_rate,
                bit_depth: track.bit_depth,
                audio_source: AudioSource::Url {
                    url: media_url,
                    format_hint: None,
                    seekable: true,
                },
            })
        })
        .collect::<Result<Vec<_>, SotfApiClientError>>()?;

    Ok(ProviderAlbum {
        external_id: album.id,
        title: album.title,
        artist: album.artist,
        year: album.year,
        album_art_url: None,
        tracks: provider_tracks,
    })
}

fn sotf_peer_client(
    host: &str,
    port: u16,
    token: &str,
) -> Result<SotfApiClient, SotfApiClientError> {
    let base_url = sotf_peer_base_url(host, port);
    SotfApiClient::new(base_url, token)
}

fn sotf_peer_base_url(host: &str, port: u16) -> String {
    let trimmed = host.trim().trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}:{port}")
    }
}

async fn diagnose_sotf_peer(
    host: &str,
    port: u16,
    accepted_fingerprint: Option<&str>,
    auth_token: Option<&str>,
) -> ConnectionDiagnostic {
    let timeout = std::time::Duration::from_secs(5);

    let dns_resolve = match resolve_dns(host, port, timeout).await {
        Ok(r) => r,
        Err(diag) => return diag,
    };

    let tcp_connect = match tokio::time::timeout(
        timeout,
        tokio::net::TcpStream::connect(format!("{host}:{port}")),
    )
    .await
    {
        Ok(Ok(_)) => StepResult::Ok(format!("port {port} open")),
        Ok(Err(e)) => StepResult::Fail(format!("{e}")),
        Err(_) => StepResult::Fail("connection timed out".to_string()),
    };

    if !tcp_connect.is_ok() {
        return ConnectionDiagnostic {
            host: host.to_string(),
            port,
            dns_resolve,
            tcp_connect,
            tls_handshake: StepResult::Skipped("TCP failed".to_string()),
            protocol_hello: StepResult::Skipped("TCP failed".to_string()),
        };
    }

    let tls_handshake = StepResult::Skipped("SOTF API uses HTTP bearer auth".to_string());
    let protocol_hello =
        diagnose_sotf_peer_protocol(host, port, accepted_fingerprint, auth_token).await;

    ConnectionDiagnostic {
        host: host.to_string(),
        port,
        dns_resolve,
        tcp_connect,
        tls_handshake,
        protocol_hello,
    }
}

async fn diagnose_sotf_peer_protocol(
    host: &str,
    port: u16,
    accepted_fingerprint: Option<&str>,
    auth_token: Option<&str>,
) -> StepResult {
    let token = auth_token.unwrap_or("").trim();
    let client = match sotf_peer_client(
        host,
        port,
        if token.is_empty() {
            "diagnostic"
        } else {
            token
        },
    ) {
        Ok(client) => client,
        Err(e) => return StepResult::Fail(format!("invalid SOTF API URL: {e}")),
    };

    let discovery = match client.discovery().await {
        Ok(discovery) if discovery.service == "sotf" => discovery,
        Ok(discovery) => {
            return StepResult::Fail(format!(
                "unexpected service '{}' at SOTF API endpoint",
                discovery.service
            ));
        }
        Err(e) => return StepResult::Fail(format!("SOTF API discovery failed: {e}")),
    };

    if let Some(expected) = accepted_fingerprint
        .map(str::trim)
        .filter(|fingerprint| !fingerprint.is_empty())
    {
        match client.pairing_status().await {
            Ok(status) if peer_fingerprints_match(expected, &status.server_fingerprint) => {}
            Ok(status) => {
                return StepResult::Fail(format!(
                    "server fingerprint mismatch: expected {expected}, got {}",
                    status.server_fingerprint
                ));
            }
            Err(e) => return StepResult::Fail(format!("fingerprint check failed: {e}")),
        }
    }

    if token.is_empty() {
        return StepResult::Fail(format!(
            "SOTF API {} reachable; API token required",
            discovery.version
        ));
    }

    match client.state().await {
        Ok(state) => StepResult::Ok(format!(
            "SOTF API {} — {} albums, {} tracks",
            discovery.version, state.library.albums, state.library.tracks
        )),
        Err(e) => StepResult::Fail(format!("SOTF API auth failed: {e}")),
    }
}

fn peer_fingerprints_match(expected: &str, actual: &str) -> bool {
    fn compact(value: &str) -> String {
        value
            .chars()
            .filter(|ch| ch.is_ascii_hexdigit())
            .flat_map(char::to_lowercase)
            .collect()
    }
    let expected = compact(expected);
    let actual = compact(actual);
    !expected.is_empty() && expected == actual
}

/// Diagnose an MPD connection: DNS -> TCP -> MPD greeting + optional auth.
async fn diagnose_mpd(host: &str, port: u16, password: Option<&str>) -> ConnectionDiagnostic {
    let timeout = std::time::Duration::from_secs(5);

    let dns_resolve = match resolve_dns(host, port, timeout).await {
        Ok(r) => r,
        Err(diag) => return diag,
    };

    let tcp_connect = match tokio::time::timeout(
        timeout,
        tokio::net::TcpStream::connect(format!("{host}:{port}")),
    )
    .await
    {
        Ok(Ok(stream)) => {
            let tcp_result = StepResult::Ok(format!("port {port} open"));
            let tls_handshake = StepResult::Skipped("MPD uses plain TCP".to_string());

            use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
            let mut reader = BufReader::new(stream);
            let mut greeting = String::new();
            let protocol_hello =
                match tokio::time::timeout(timeout, reader.read_line(&mut greeting)).await {
                    Ok(Ok(_)) if greeting.starts_with("OK MPD") => {
                        let version = greeting.trim().trim_start_matches("OK MPD ").to_string();
                        if let Some(pw) = password {
                            let escaped = pw.replace('\\', "\\\\").replace('"', "\\\"");
                            let cmd = format!("password \"{escaped}\"\n");
                            let inner = reader.into_inner();
                            let mut writer = tokio::io::BufWriter::new(inner);
                            if writer.write_all(cmd.as_bytes()).await.is_err()
                                || writer.flush().await.is_err()
                            {
                                StepResult::Fail(format!("MPD {version} — auth send failed"))
                            } else {
                                let mut r2 = tokio::io::BufReader::new(writer.into_inner());
                                let mut resp = String::new();
                                match tokio::time::timeout(timeout, r2.read_line(&mut resp)).await {
                                    Ok(Ok(_)) if resp.starts_with("OK") => {
                                        StepResult::Ok(format!("MPD {version} — authenticated"))
                                    }
                                    Ok(Ok(_)) => StepResult::Fail(format!(
                                        "MPD {version} — auth rejected: {}",
                                        resp.trim()
                                    )),
                                    _ => StepResult::Fail(format!(
                                        "MPD {version} — auth response failed"
                                    )),
                                }
                            }
                        } else {
                            StepResult::Ok(format!("MPD {version}"))
                        }
                    }
                    Ok(Ok(_)) => {
                        StepResult::Fail(format!("unexpected greeting: {}", greeting.trim()))
                    }
                    Ok(Err(e)) => StepResult::Fail(format!("read error: {e}")),
                    Err(_) => StepResult::Fail("greeting timed out".to_string()),
                };

            return ConnectionDiagnostic {
                host: host.to_string(),
                port,
                dns_resolve,
                tcp_connect: tcp_result,
                tls_handshake,
                protocol_hello,
            };
        }
        Ok(Err(e)) => StepResult::Fail(format!("{e}")),
        Err(_) => StepResult::Fail("connection timed out".to_string()),
    };

    ConnectionDiagnostic {
        host: host.to_string(),
        port,
        dns_resolve,
        tcp_connect,
        tls_handshake: StepResult::Skipped("TCP failed".to_string()),
        protocol_hello: StepResult::Skipped("TCP failed".to_string()),
    }
}

/// Diagnose a plain TCP connection (DNS + TCP only).
async fn diagnose_tcp_simple(host: &str, port: u16) -> ConnectionDiagnostic {
    let timeout = std::time::Duration::from_secs(5);

    let dns_resolve = match resolve_dns(host, port, timeout).await {
        Ok(r) => r,
        Err(diag) => return diag,
    };

    let tcp_connect = match tokio::time::timeout(
        timeout,
        tokio::net::TcpStream::connect(format!("{host}:{port}")),
    )
    .await
    {
        Ok(Ok(_)) => StepResult::Ok(format!("port {port} open")),
        Ok(Err(e)) => StepResult::Fail(format!("{e}")),
        Err(_) => StepResult::Fail("connection timed out".to_string()),
    };

    let protocol_hello = if tcp_connect.is_ok() {
        StepResult::Ok("TCP reachable".to_string())
    } else {
        StepResult::Skipped("TCP failed".to_string())
    };

    ConnectionDiagnostic {
        host: host.to_string(),
        port,
        dns_resolve,
        tcp_connect,
        tls_handshake: StepResult::Skipped("not tested".to_string()),
        protocol_hello,
    }
}

/// Diagnose connectivity to a URL by extracting host:port and doing DNS + TCP.
async fn diagnose_tcp_from_url(url: &str) -> ConnectionDiagnostic {
    let stripped = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host_port = stripped.split('/').next().unwrap_or(stripped);
    let (host, port) = if let Some((h, p)) = host_port.rsplit_once(':') {
        (h.to_string(), p.parse().unwrap_or(80))
    } else {
        (
            host_port.to_string(),
            if url.starts_with("https") { 443 } else { 80 },
        )
    };
    diagnose_tcp_simple(&host, port).await
}

/// Shared DNS resolution step.
async fn resolve_dns(
    host: &str,
    port: u16,
    timeout: std::time::Duration,
) -> Result<StepResult, ConnectionDiagnostic> {
    match tokio::time::timeout(timeout, tokio::net::lookup_host(format!("{host}:{port}"))).await {
        Ok(Ok(mut addrs)) => {
            if let Some(addr) = addrs.next() {
                Ok(StepResult::Ok(format!("resolved to {}", addr.ip())))
            } else {
                Err(fail_at_dns(host, port, "no addresses returned"))
            }
        }
        Ok(Err(e)) => Err(fail_at_dns(host, port, &format!("{e}"))),
        Err(_) => Err(fail_at_dns(host, port, "DNS lookup timed out")),
    }
}

fn fail_at_dns(host: &str, port: u16, reason: &str) -> ConnectionDiagnostic {
    ConnectionDiagnostic {
        host: host.to_string(),
        port,
        dns_resolve: StepResult::Fail(reason.to_string()),
        tcp_connect: StepResult::Skipped("DNS failed".to_string()),
        tls_handshake: StepResult::Skipped("DNS failed".to_string()),
        protocol_hello: StepResult::Skipped("DNS failed".to_string()),
    }
}
