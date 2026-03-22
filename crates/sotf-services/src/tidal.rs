// ============================================================================
// Tidal Integration
// ============================================================================
//
// Uses Tidal's HTTP API for authentication, search, and stream URL resolution.
// Audio is delivered as direct FLAC/AAC URLs that the engine's HTTP decoder handles.

use crate::service::*;
use serde::Deserialize;

/// Tidal API base URL.
const API_BASE: &str = "https://api.tidal.com/v1";

/// Tidal auth base URL.
const AUTH_BASE: &str = "https://auth.tidal.com/v1/oauth2";

/// Tidal client ID for device code flow.
/// In production this should be configurable, not hardcoded.
const DEFAULT_CLIENT_ID: &str = "";

pub struct TidalService {
    client: reqwest::blocking::Client,
    client_id: String,
    access_token: Option<String>,
    #[allow(dead_code)]
    refresh_token: Option<String>,
    country_code: String,
    quality: AudioQuality,
}

impl Default for TidalService {
    fn default() -> Self {
        Self::new()
    }
}

impl TidalService {
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            client_id: DEFAULT_CLIENT_ID.to_string(),
            access_token: None,
            refresh_token: None,
            country_code: "US".to_string(),
            quality: AudioQuality::Lossless,
        }
    }

    pub fn with_client_id(mut self, client_id: &str) -> Self {
        self.client_id = client_id.to_string();
        self
    }

    pub fn with_country_code(mut self, code: &str) -> Self {
        self.country_code = code.to_string();
        self
    }

    pub fn with_quality(mut self, quality: AudioQuality) -> Self {
        self.quality = quality;
        self
    }

    fn api_get(&self, path: &str) -> Result<reqwest::blocking::Response, ServiceError> {
        let token = self
            .access_token
            .as_ref()
            .ok_or_else(|| ServiceError::AuthError("Not authenticated".to_string()))?;

        let url = format!("{}{}", API_BASE, path);
        self.client
            .get(&url)
            .bearer_auth(token)
            .query(&[("countryCode", &self.country_code)])
            .send()
            .map_err(|e| ServiceError::NetworkError(e.to_string()))
    }

    fn quality_to_tidal_quality(&self) -> &str {
        match self.quality {
            AudioQuality::Low => "LOW",
            AudioQuality::Normal => "LOW",
            AudioQuality::High => "HIGH",
            AudioQuality::Lossless => "LOSSLESS",
            AudioQuality::HiRes => "HI_RES_LOSSLESS",
        }
    }
}

impl StreamingService for TidalService {
    fn authenticate(&mut self, credentials: ServiceCredentials) -> Result<(), ServiceError> {
        match credentials {
            ServiceCredentials::AccessToken(token) => {
                self.access_token = Some(token);
                // Verify the token works
                let resp = self.api_get("/sessions")?;
                if resp.status().is_success() {
                    let session: TidalSession = resp
                        .json()
                        .map_err(|e| ServiceError::AuthError(e.to_string()))?;
                    self.country_code = session.country_code;
                    log::info!(
                        "[Tidal] Authenticated as user {} (country: {})",
                        session.user_id,
                        self.country_code
                    );
                    Ok(())
                } else {
                    Err(ServiceError::AuthError(format!(
                        "Token validation failed: HTTP {}",
                        resp.status()
                    )))
                }
            }
            ServiceCredentials::DeviceCode => {
                if self.client_id.is_empty() {
                    return Err(ServiceError::AuthError(
                        "Tidal client_id not configured. Use with_client_id() or set TIDAL_CLIENT_ID env var.".to_string(),
                    ));
                }

                // Step 1: Request device code
                let resp = self
                    .client
                    .post(format!("{}/device_authorization", AUTH_BASE))
                    .form(&[
                        ("client_id", self.client_id.as_str()),
                        ("scope", "r_usr+w_usr+w_sub"),
                    ])
                    .send()
                    .map_err(|e| ServiceError::NetworkError(e.to_string()))?;

                if !resp.status().is_success() {
                    return Err(ServiceError::AuthError(format!(
                        "Device code request failed: HTTP {}",
                        resp.status()
                    )));
                }

                let device_auth: TidalDeviceAuth = resp
                    .json()
                    .map_err(|e| ServiceError::AuthError(e.to_string()))?;

                // Return the verification URL for the user to visit
                Err(ServiceError::AuthError(format!(
                    "Visit {} and enter code: {}",
                    device_auth.verification_uri_complete.as_deref()
                        .unwrap_or(&device_auth.verification_uri),
                    device_auth.user_code
                )))
            }
            _ => Err(ServiceError::AuthError(
                "Tidal supports AccessToken or DeviceCode credentials".to_string(),
            )),
        }
    }

    fn is_authenticated(&self) -> bool {
        self.access_token.is_some()
    }

    fn search_tracks(&self, query: &str, limit: u32) -> Result<Vec<ServiceTrack>, ServiceError> {
        let resp = self
            .client
            .get(format!("{}/search/tracks", API_BASE))
            .bearer_auth(self.access_token.as_ref().ok_or_else(|| {
                ServiceError::AuthError("Not authenticated".to_string())
            })?)
            .query(&[
                ("query", query),
                ("limit", &limit.to_string()),
                ("countryCode", &self.country_code),
            ])
            .send()
            .map_err(|e| ServiceError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ServiceError::NetworkError(format!(
                "Search failed: HTTP {}",
                resp.status()
            )));
        }

        let result: TidalSearchResult<TidalTrack> = resp
            .json()
            .map_err(|e| ServiceError::Other(e.to_string()))?;

        Ok(result
            .items
            .into_iter()
            .map(|t| ServiceTrack {
                id: t.id.to_string(),
                title: t.title,
                artist: t.artist.name,
                album: t.album.title,
                duration_secs: t.duration as f64,
                track_number: Some(t.track_number),
                album_art_url: t
                    .album
                    .cover
                    .map(|c| format!("https://resources.tidal.com/images/{}/640x640.jpg", c.replace('-', "/"))),
                available_qualities: vec![AudioQuality::High, AudioQuality::Lossless],
            })
            .collect())
    }

    fn search_albums(&self, query: &str, limit: u32) -> Result<Vec<ServiceAlbum>, ServiceError> {
        let resp = self
            .client
            .get(format!("{}/search/albums", API_BASE))
            .bearer_auth(self.access_token.as_ref().ok_or_else(|| {
                ServiceError::AuthError("Not authenticated".to_string())
            })?)
            .query(&[
                ("query", query),
                ("limit", &limit.to_string()),
                ("countryCode", &self.country_code),
            ])
            .send()
            .map_err(|e| ServiceError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ServiceError::NetworkError(format!(
                "Album search failed: HTTP {}",
                resp.status()
            )));
        }

        let result: TidalSearchResult<TidalAlbum> = resp
            .json()
            .map_err(|e| ServiceError::Other(e.to_string()))?;

        Ok(result
            .items
            .into_iter()
            .map(|a| ServiceAlbum {
                id: a.id.to_string(),
                title: a.title,
                artist: a.artist.name,
                year: a.release_date.and_then(|d| d[..4].parse().ok()),
                track_count: a.number_of_tracks,
                album_art_url: a
                    .cover
                    .map(|c| format!("https://resources.tidal.com/images/{}/640x640.jpg", c.replace('-', "/"))),
            })
            .collect())
    }

    fn album_tracks(&self, album_id: &str) -> Result<Vec<ServiceTrack>, ServiceError> {
        let resp = self.api_get(&format!("/albums/{}/tracks", album_id))?;

        if !resp.status().is_success() {
            return Err(ServiceError::NotFound(format!(
                "Album {} not found",
                album_id
            )));
        }

        let result: TidalSearchResult<TidalTrack> = resp
            .json()
            .map_err(|e| ServiceError::Other(e.to_string()))?;

        Ok(result
            .items
            .into_iter()
            .map(|t| ServiceTrack {
                id: t.id.to_string(),
                title: t.title,
                artist: t.artist.name,
                album: t.album.title,
                duration_secs: t.duration as f64,
                track_number: Some(t.track_number),
                album_art_url: None,
                available_qualities: vec![AudioQuality::High, AudioQuality::Lossless],
            })
            .collect())
    }

    fn start_stream(
        &mut self,
        track_id: &str,
        quality: AudioQuality,
    ) -> Result<ServiceStreamResult, ServiceError> {
        self.quality = quality;
        let quality_str = self.quality_to_tidal_quality();

        let resp = self
            .client
            .get(format!("{}/tracks/{}/urlpostpaywall", API_BASE, track_id))
            .bearer_auth(self.access_token.as_ref().ok_or_else(|| {
                ServiceError::AuthError("Not authenticated".to_string())
            })?)
            .query(&[
                ("audioquality", quality_str),
                ("urlusagemode", "STREAM"),
                ("assetpresentation", "FULL"),
                ("countryCode", &self.country_code),
            ])
            .send()
            .map_err(|e| ServiceError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ServiceError::NotFound(format!(
                "Track {} not available (HTTP {})",
                track_id,
                resp.status()
            )));
        }

        let stream_info: TidalStreamInfo = resp
            .json()
            .map_err(|e| ServiceError::Other(e.to_string()))?;

        // Tidal provides a direct URL — let the engine's HTTP decoder handle it
        let format_hint = match stream_info.codec.as_deref() {
            Some("FLAC") => Some("flac".to_string()),
            Some("AAC") => Some("aac".to_string()),
            Some("MQA") => Some("flac".to_string()), // MQA is FLAC-encapsulated
            _ => None,
        };

        log::info!(
            "[Tidal] Streaming track {} at {} quality, codec: {:?}",
            track_id,
            quality_str,
            stream_info.codec,
        );

        Ok(ServiceStreamResult::Url {
            url: stream_info.url,
            format_hint,
        })
    }

    fn stop_stream(&mut self) {
        // Tidal streams are HTTP URLs — nothing to clean up
    }

    fn service_name(&self) -> &str {
        "Tidal"
    }
}

// ============================================================================
// Tidal API response types
// ============================================================================

#[derive(Deserialize)]
struct TidalSession {
    #[serde(rename = "userId")]
    user_id: u64,
    #[serde(rename = "countryCode")]
    country_code: String,
}

#[derive(Deserialize)]
struct TidalDeviceAuth {
    #[serde(rename = "userCode")]
    user_code: String,
    #[serde(rename = "verificationUri")]
    verification_uri: String,
    #[serde(rename = "verificationUriComplete")]
    verification_uri_complete: Option<String>,
}

#[derive(Deserialize)]
struct TidalSearchResult<T> {
    items: Vec<T>,
}

#[derive(Deserialize)]
struct TidalTrack {
    id: u64,
    title: String,
    duration: u32,
    #[serde(rename = "trackNumber")]
    track_number: u32,
    artist: TidalArtist,
    album: TidalAlbumRef,
}

#[derive(Deserialize)]
struct TidalAlbum {
    id: u64,
    title: String,
    artist: TidalArtist,
    cover: Option<String>,
    #[serde(rename = "numberOfTracks")]
    number_of_tracks: u32,
    #[serde(rename = "releaseDate")]
    release_date: Option<String>,
}

#[derive(Deserialize)]
struct TidalAlbumRef {
    title: String,
    cover: Option<String>,
}

#[derive(Deserialize)]
struct TidalArtist {
    name: String,
}

#[derive(Deserialize)]
struct TidalStreamInfo {
    url: String,
    codec: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tidal_service_not_authenticated() {
        let service = TidalService::new();
        assert!(!service.is_authenticated());
    }

    #[test]
    fn test_tidal_quality_mapping() {
        let mut service = TidalService::new();
        service.quality = AudioQuality::Lossless;
        assert_eq!(service.quality_to_tidal_quality(), "LOSSLESS");

        service.quality = AudioQuality::High;
        assert_eq!(service.quality_to_tidal_quality(), "HIGH");

        service.quality = AudioQuality::HiRes;
        assert_eq!(service.quality_to_tidal_quality(), "HI_RES_LOSSLESS");
    }

    #[test]
    fn test_tidal_device_code_requires_client_id() {
        let mut service = TidalService::new();
        let result = service.authenticate(ServiceCredentials::DeviceCode);
        assert!(result.is_err());
        match result {
            Err(ServiceError::AuthError(msg)) => {
                assert!(msg.contains("client_id"));
            }
            _ => panic!("Expected AuthError"),
        }
    }
}
