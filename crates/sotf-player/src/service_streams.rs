use sotf_audio::decoder::{AudioSource, ServiceId};

#[derive(Debug, thiserror::Error)]
pub enum ServiceStreamResolveError {
    #[error("{0} streaming support is not compiled in")]
    UnsupportedService(ServiceId),
    #[error("{0}")]
    MissingCredentials(String),
    #[error("{0} returned PCM, but PCM service handoff is not wired into the engine yet")]
    PcmHandoffUnsupported(ServiceId),
    #[error("{0}")]
    Service(String),
}

pub fn resolve_service_stream_from_env(
    source: AudioSource,
) -> Result<AudioSource, ServiceStreamResolveError> {
    let AudioSource::ServiceStream { service, track_id } = source else {
        return Ok(source);
    };

    match service {
        ServiceId::Tidal => resolve_tidal_from_env(track_id),
        ServiceId::Spotify => resolve_spotify_from_env(track_id),
    }
}

#[cfg(feature = "tidal")]
fn resolve_tidal_from_env(track_id: String) -> Result<AudioSource, ServiceStreamResolveError> {
    use sotf_services::{AudioQuality, ServiceCredentials, ServiceStreamResult, StreamingService};

    let token = std::env::var("TIDAL_ACCESS_TOKEN")
        .ok()
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| {
            ServiceStreamResolveError::MissingCredentials(
                "Tidal playback requires TIDAL_ACCESS_TOKEN or a future UI credential store"
                    .to_string(),
            )
        })?;

    let mut service = sotf_services::tidal::TidalService::new();
    service
        .authenticate(ServiceCredentials::AccessToken(token))
        .map_err(|err| ServiceStreamResolveError::Service(err.to_string()))?;

    match service
        .start_stream(&track_id, AudioQuality::Lossless)
        .map_err(|err| ServiceStreamResolveError::Service(err.to_string()))?
    {
        ServiceStreamResult::Url { url, format_hint } => Ok(AudioSource::Url {
            url,
            format_hint,
            seekable: true,
        }),
        ServiceStreamResult::Pcm(_) => Err(ServiceStreamResolveError::PcmHandoffUnsupported(
            ServiceId::Tidal,
        )),
    }
}

#[cfg(not(feature = "tidal"))]
fn resolve_tidal_from_env(_track_id: String) -> Result<AudioSource, ServiceStreamResolveError> {
    Err(ServiceStreamResolveError::UnsupportedService(
        ServiceId::Tidal,
    ))
}

#[cfg(feature = "spotify")]
fn resolve_spotify_from_env(_track_id: String) -> Result<AudioSource, ServiceStreamResolveError> {
    Err(ServiceStreamResolveError::PcmHandoffUnsupported(
        ServiceId::Spotify,
    ))
}

#[cfg(not(feature = "spotify"))]
fn resolve_spotify_from_env(_track_id: String) -> Result<AudioSource, ServiceStreamResolveError> {
    Err(ServiceStreamResolveError::UnsupportedService(
        ServiceId::Spotify,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_service_source_passes_through() {
        let source = AudioSource::Url {
            url: "https://example.com/track.flac".to_string(),
            format_hint: Some("flac".to_string()),
            seekable: true,
        };

        assert_eq!(
            resolve_service_stream_from_env(source.clone()).unwrap(),
            source
        );
    }

    #[test]
    fn unsupported_service_reports_before_engine_decoder() {
        let source = AudioSource::ServiceStream {
            service: ServiceId::Tidal,
            track_id: "123".to_string(),
        };

        let err = resolve_service_stream_from_env(source).unwrap_err();
        #[cfg(not(feature = "tidal"))]
        assert!(err.to_string().contains("not compiled"));
        #[cfg(feature = "tidal")]
        assert!(err.to_string().contains("TIDAL_ACCESS_TOKEN"));
    }

    #[cfg(feature = "spotify")]
    #[test]
    fn spotify_reports_pcm_handoff_before_credentials() {
        let source = AudioSource::ServiceStream {
            service: ServiceId::Spotify,
            track_id: "spotify-track".to_string(),
        };

        let err = resolve_service_stream_from_env(source).unwrap_err();
        assert!(err.to_string().contains("PCM"));
    }
}
