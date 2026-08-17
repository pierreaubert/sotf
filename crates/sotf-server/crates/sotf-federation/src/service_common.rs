//! Shared helpers for the streaming-service federation providers
//! (Tidal, Spotify). Compiled when either provider feature is enabled.

use crate::provider::{ProviderError, ProviderTrack};
use sotf_audio::decoder::{AudioSource, ServiceId};
use sotf_services::{ServiceError, ServiceTrack};

/// Hard cap on album art downloads — guards against a hostile or buggy
/// server streaming unbounded data.
const MAX_ALBUM_ART_BYTES: u64 = 8 * 1024 * 1024;

/// Map a streaming-service error onto the federation error type.
pub(crate) fn map_service_error(err: ServiceError) -> ProviderError {
    match err {
        ServiceError::AuthError(msg) => ProviderError::Auth(msg),
        ServiceError::NetworkError(msg) => ProviderError::Network(msg),
        ServiceError::NotFound(msg) => ProviderError::NotFound(msg),
        ServiceError::Other(msg) => ProviderError::Other(msg),
    }
}

/// Map a `ServiceTrack` onto a `ProviderTrack` whose audio source defers to
/// the engine's service-stream resolver (a fresh stream URL / PCM stream is
/// minted at decode time from the service + track id).
pub(crate) fn service_track_to_provider(service: ServiceId, track: ServiceTrack) -> ProviderTrack {
    let track_id = track.id.clone();
    ProviderTrack {
        external_id: track.id,
        title: track.title,
        artist: (!track.artist.is_empty()).then_some(track.artist),
        album_artist: None,
        track_number: track.track_number,
        disc_number: None,
        duration_secs: Some(track.duration_secs),
        genre: None,
        composer: None,
        channels: None,
        sample_rate: None,
        bit_depth: None,
        audio_source: AudioSource::ServiceStream { service, track_id },
    }
}

/// Download album art bytes. Returns `Ok(None)` when the URL does not answer
/// with an `image/*` payload (art is best-effort); network failures and
/// oversized payloads are errors.
pub(crate) async fn fetch_image(
    client: &reqwest::Client,
    url: &str,
) -> Result<Option<Vec<u8>>, ProviderError> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| ProviderError::Network(format!("album art request failed: {e}")))?;

    if !resp.status().is_success() {
        log::warn!("album art fetch for {url} returned HTTP {}", resp.status());
        return Ok(None);
    }

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    if !content_type.starts_with("image/") {
        log::warn!("album art fetch for {url} returned non-image content type {content_type}");
        return Ok(None);
    }

    if resp
        .content_length()
        .is_some_and(|len| len > MAX_ALBUM_ART_BYTES)
    {
        return Err(ProviderError::Other(format!(
            "album art exceeds {MAX_ALBUM_ART_BYTES} bytes (Content-Length)"
        )));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| ProviderError::Network(format!("album art download failed: {e}")))?;
    if bytes.len() as u64 > MAX_ALBUM_ART_BYTES {
        return Err(ProviderError::Other(format!(
            "album art exceeds {MAX_ALBUM_ART_BYTES} bytes"
        )));
    }

    Ok(Some(bytes.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_services::AudioQuality;

    #[test]
    fn map_service_error_variants() {
        assert!(matches!(
            map_service_error(ServiceError::AuthError("x".into())),
            ProviderError::Auth(_)
        ));
        assert!(matches!(
            map_service_error(ServiceError::NetworkError("x".into())),
            ProviderError::Network(_)
        ));
        assert!(matches!(
            map_service_error(ServiceError::NotFound("x".into())),
            ProviderError::NotFound(_)
        ));
        assert!(matches!(
            map_service_error(ServiceError::Other("x".into())),
            ProviderError::Other(_)
        ));
    }

    #[test]
    fn service_track_mapping_sets_service_stream_source() {
        let track = ServiceTrack {
            id: "12345".to_string(),
            title: "Time".to_string(),
            artist: "Pink Floyd".to_string(),
            album: "The Dark Side of the Moon".to_string(),
            duration_secs: 413.0,
            track_number: Some(4),
            album_art_url: None,
            available_qualities: vec![AudioQuality::Lossless],
        };
        let mapped = service_track_to_provider(ServiceId::Tidal, track);
        assert_eq!(mapped.external_id, "12345");
        assert_eq!(mapped.title, "Time");
        assert_eq!(mapped.artist.as_deref(), Some("Pink Floyd"));
        assert_eq!(mapped.track_number, Some(4));
        assert_eq!(mapped.duration_secs, Some(413.0));
        assert_eq!(
            mapped.audio_source,
            AudioSource::ServiceStream {
                service: ServiceId::Tidal,
                track_id: "12345".to_string(),
            }
        );
    }

    #[test]
    fn service_track_mapping_drops_empty_artist() {
        let track = ServiceTrack {
            id: "1".to_string(),
            title: "t".to_string(),
            artist: String::new(),
            album: "a".to_string(),
            duration_secs: 1.0,
            track_number: None,
            album_art_url: None,
            available_qualities: vec![],
        };
        let mapped = service_track_to_provider(ServiceId::Spotify, track);
        assert_eq!(mapped.artist, None);
    }
}
