use super::sotf::sotf_api_album_to_provider_album;
use super::sotf::sotf_peer_client;
use super::types::FederationScanResult;
use crate::federation_config::{FederationSourceEntry, SourceConnectionConfig};
use crate::sotf_api_client::{SotfApiClient, SotfApiClientError};
use sotf_federation::{
    DlnaProvider, DlnaProviderConfig, LibraryProvider, MpdProvider, MpdProviderConfig,
    ProviderAlbum, SourceId,
};
#[cfg(feature = "spotify")]
use sotf_federation::{SpotifyProvider, SpotifyProviderConfig};
#[cfg(feature = "tidal")]
use sotf_federation::{TidalProvider, TidalProviderConfig};

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
            accepted_fingerprint,
            auth_token,
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
            let client = sotf_peer_client(host, *port, token, accepted_fingerprint.as_deref())
                .map_err(|e| FederationScanResult {
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
        #[cfg(feature = "tidal")]
        SourceConnectionConfig::Tidal {
            access_token,
            refresh_token,
            client_id,
            country_code,
            quality,
        } => {
            let config = TidalProviderConfig {
                access_token: access_token.clone(),
                refresh_token: refresh_token.clone(),
                client_id: client_id.clone(),
                country_code: country_code.clone(),
                quality: quality.clone(),
            };
            // Tidal refresh tokens are single-use: the connect below rotates
            // the stored token, so the rotated pair must be written back to
            // the source config or the next login silently fails.
            let provider = TidalProvider::new_with_token_persister(
                SourceId(source_id_str.clone()),
                config,
                tidal_token_persister(source),
            )
            .await
            .map_err(|e| FederationScanResult {
                source_id: source_id_str.clone(),
                albums: 0,
                tracks: 0,
                error: Some(format!("failed to connect to Tidal: {e}")),
            })?;
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
        #[cfg(not(feature = "tidal"))]
        SourceConnectionConfig::Tidal { .. } => Err(FederationScanResult {
            source_id: source_id_str,
            albums: 0,
            tracks: 0,
            error: Some("tidal support not compiled in".to_string()),
        }),
        #[cfg(feature = "spotify")]
        SourceConnectionConfig::Spotify { .. } => {
            let cache_dir =
                crate::service_login::spotify_cache_dir().ok_or_else(|| FederationScanResult {
                    source_id: source_id_str.clone(),
                    albums: 0,
                    tracks: 0,
                    error: Some(
                        "could not determine config directory for Spotify credential cache"
                            .to_string(),
                    ),
                })?;
            let provider = SpotifyProvider::new(
                SourceId(source_id_str.clone()),
                SpotifyProviderConfig { cache_dir },
            )
            .await
            .map_err(|e| FederationScanResult {
                source_id: source_id_str.clone(),
                albums: 0,
                tracks: 0,
                error: Some(format!("failed to connect to Spotify: {e}")),
            })?;
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
        #[cfg(not(feature = "spotify"))]
        SourceConnectionConfig::Spotify { .. } => Err(FederationScanResult {
            source_id: source_id_str,
            albums: 0,
            tracks: 0,
            error: Some("spotify support not compiled in".to_string()),
        }),
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

/// Build the scan-path persister for rotated Tidal tokens: writes the new
/// token pair back into the federation source config in the music database —
/// the same `save_federation_source` path the playback side uses
/// (`ServiceManager::persist_rotated_tidal_tokens`). Persistence failures are
/// logged and swallowed: the scan result is unaffected.
#[cfg(feature = "tidal")]
fn tidal_token_persister(source: &FederationSourceEntry) -> sotf_federation::TidalTokenPersister {
    tidal_token_persister_with(source, crate::service_login::persist_federation_source)
}

/// Test seam for [`tidal_token_persister`]: the `save` step is injectable so
/// tests do not touch the real music database.
#[cfg(feature = "tidal")]
fn tidal_token_persister_with(
    source: &FederationSourceEntry,
    save: impl Fn(&FederationSourceEntry) -> Result<(), String> + Send + Sync + 'static,
) -> sotf_federation::TidalTokenPersister {
    let entry = source.clone();
    std::sync::Arc::new(move |new_access: &str, new_refresh: Option<&str>| {
        let Some(new_refresh) = new_refresh else {
            // Access-token-only refresh: the rotated access token alone is
            // not enough to keep the source usable, so keep the stored
            // config untouched (Tidal always returns a new refresh token).
            log::warn!("[Tidal] scan refreshed without a rotated refresh token; not persisting");
            return;
        };
        let mut entry = entry.clone();
        if !crate::service_login::apply_tidal_device_tokens(&mut entry, new_access, new_refresh) {
            return; // not a Tidal source — nothing to persist
        }
        log::info!(
            "[Tidal] persisting rotated tokens for source {}",
            entry.source_id
        );
        if let Err(e) = save(&entry) {
            log::warn!(
                "[Tidal] failed to persist rotated tokens for source {}: {e}",
                entry.source_id
            );
        }
    })
}

#[cfg(all(test, feature = "tidal"))]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn tidal_source() -> FederationSourceEntry {
        FederationSourceEntry {
            source_id: "tidal:test".to_string(),
            display_name: "Tidal".to_string(),
            priority: 0,
            is_enabled: true,
            connection: SourceConnectionConfig::Tidal {
                access_token: "old-access".to_string(),
                refresh_token: "old-refresh".to_string(),
                client_id: "client".to_string(),
                country_code: "US".to_string(),
                quality: "LOSSLESS".to_string(),
            },
            is_available: None,
        }
    }

    type SavedEntries = Arc<Mutex<Vec<FederationSourceEntry>>>;

    fn recording_persister(
        source: &FederationSourceEntry,
    ) -> (sotf_federation::TidalTokenPersister, SavedEntries) {
        let saved: SavedEntries = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&saved);
        let persister = tidal_token_persister_with(source, move |entry: &FederationSourceEntry| {
            captured
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(entry.clone());
            Ok(())
        });
        (persister, saved)
    }

    #[test]
    fn persister_writes_rotated_tokens_into_source_entry() {
        let (persister, saved) = recording_persister(&tidal_source());

        persister("new-access", Some("new-refresh"));

        let saved = saved.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(saved.len(), 1);
        // The source id survives so `save_federation_source` upserts the
        // right row, and unrelated fields are untouched.
        assert_eq!(saved[0].source_id, "tidal:test");
        match &saved[0].connection {
            SourceConnectionConfig::Tidal {
                access_token,
                refresh_token,
                client_id,
                ..
            } => {
                assert_eq!(access_token, "new-access");
                assert_eq!(refresh_token, "new-refresh");
                assert_eq!(client_id, "client");
            }
            other => panic!("unexpected connection: {}", other.type_name()),
        }
    }

    #[test]
    fn persister_skips_when_refresh_token_missing() {
        let (persister, saved) = recording_persister(&tidal_source());

        persister("new-access", None);

        assert!(saved.lock().unwrap_or_else(|e| e.into_inner()).is_empty());
    }

    #[test]
    fn persister_swallows_save_errors() {
        let persister =
            tidal_token_persister_with(&tidal_source(), |_| Err("db is down".to_string()));

        // Must not panic or propagate — the scan result is unaffected.
        persister("new-access", Some("new-refresh"));
    }

    #[test]
    fn persister_ignores_non_tidal_source() {
        let mut source = tidal_source();
        source.connection = SourceConnectionConfig::Spotify {
            username: "user".to_string(),
            password: "pass".to_string(),
            quality: "High".to_string(),
        };
        let (persister, saved) = recording_persister(&source);

        persister("new-access", Some("new-refresh"));

        assert!(saved.lock().unwrap_or_else(|e| e.into_inner()).is_empty());
    }
}
