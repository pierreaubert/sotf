//! Legacy `AudioSource`-returning service-stream resolution shim.
//!
//! This is a thin compatibility layer over [`crate::service_manager`] for the
//! apps (TUI/GPUI) that still pre-resolve `AudioSource::ServiceStream` before
//! handing the source to the engine. New code should instead install the
//! engine hook once at startup via
//! [`crate::service_manager::install_service_stream_resolver`] and let the
//! decoder thread resolve streams itself (including gapless preloads).
//!
//! The shim resolves through the *same* process-global [`ServiceManager`] as
//! the engine hook, so credentials, token refresh, and persist-back behave
//! identically on both paths. Because `AudioSource` has no PCM variant, a
//! service returning pre-decoded PCM (Spotify) maps to
//! [`ServiceStreamResolveError::PcmHandoffUnsupported`] here — playback of
//! such tracks requires the engine resolver hook.
//!
//! [`ServiceManager`]: crate::service_manager::ServiceManager

use crate::service_manager::{self, ServiceManagerError};
use sotf_audio::decoder::{AudioSource, ResolvedServiceStream, ServiceId};

#[derive(Debug, thiserror::Error)]
pub enum ServiceStreamResolveError {
    #[error("{0} streaming support is not compiled in")]
    UnsupportedService(ServiceId),
    #[error("{0}")]
    MissingCredentials(String),
    #[error(
        "{0} returned PCM, but this legacy resolution path cannot carry PCM streams; \
         install the engine service-stream resolver (ServiceManager) instead"
    )]
    PcmHandoffUnsupported(ServiceId),
    #[error("{0}")]
    Service(String),
}

/// Resolve a service-stream source into a directly decodable [`AudioSource`].
///
/// Non-service sources pass through unchanged. Delegates to the
/// process-global service manager (same code path as the engine resolver
/// hook); see the module docs for the PCM limitation.
pub fn resolve_service_stream_from_env(
    source: AudioSource,
) -> Result<AudioSource, ServiceStreamResolveError> {
    let AudioSource::ServiceStream { service, track_id } = source else {
        return Ok(source);
    };

    let resolved = service_manager::resolve_typed(service, &track_id).map_err(map_manager_error)?;
    resolved_to_audio_source(service, resolved)
}

fn map_manager_error(err: ServiceManagerError) -> ServiceStreamResolveError {
    match err {
        ServiceManagerError::Unsupported(service) => {
            ServiceStreamResolveError::UnsupportedService(service)
        }
        ServiceManagerError::MissingCredentials(msg) => {
            ServiceStreamResolveError::MissingCredentials(msg)
        }
        other => ServiceStreamResolveError::Service(other.to_string()),
    }
}

fn resolved_to_audio_source(
    service: ServiceId,
    resolved: ResolvedServiceStream,
) -> Result<AudioSource, ServiceStreamResolveError> {
    match resolved {
        ResolvedServiceStream::Url {
            url,
            format_hint,
            seekable,
        } => Ok(AudioSource::Url {
            url,
            format_hint,
            seekable,
        }),
        ResolvedServiceStream::Pcm { .. } => {
            Err(ServiceStreamResolveError::PcmHandoffUnsupported(service))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service_manager::{ServiceManager, install_manager_for_tests};

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::service_manager::SERVICE_STREAM_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

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
    fn tidal_failure_reports_before_engine_decoder() {
        // Holds the shared lock: this swaps out the process-global manager.
        let _lock = test_lock();

        // Deterministic manager: no DB sources, forced-missing env token.
        #[allow(unused_mut)]
        let mut manager = ServiceManager::new();
        #[cfg(feature = "tidal")]
        {
            manager = manager
                .with_test_source_loader(std::sync::Arc::new(|| Ok(Vec::new())))
                .with_test_tidal_env_token("");
        }
        install_manager_for_tests(manager);

        let source = AudioSource::ServiceStream {
            service: ServiceId::Tidal,
            track_id: "123".to_string(),
        };

        let err = resolve_service_stream_from_env(source).unwrap_err();
        #[cfg(not(feature = "tidal"))]
        assert!(err.to_string().contains("not compiled"), "got: {err}");
        #[cfg(feature = "tidal")]
        assert!(
            matches!(err, ServiceStreamResolveError::MissingCredentials(_))
                && err.to_string().contains("TIDAL_ACCESS_TOKEN"),
            "got: {err}"
        );
    }

    #[cfg(feature = "spotify")]
    #[test]
    fn spotify_without_cached_credentials_directs_to_settings() {
        let _lock = test_lock();

        // Point the cache dir at a guaranteed-empty temp directory.
        let dir = tempfile::tempdir().expect("temp cache dir");
        let manager = ServiceManager::new()
            .with_test_source_loader(std::sync::Arc::new(|| Ok(Vec::new())))
            .with_test_spotify_cache_dir(dir.path().to_path_buf());
        install_manager_for_tests(manager);

        let source = AudioSource::ServiceStream {
            service: ServiceId::Spotify,
            track_id: "spotify-track".to_string(),
        };

        let err = resolve_service_stream_from_env(source).unwrap_err();
        assert!(
            matches!(err, ServiceStreamResolveError::MissingCredentials(_)),
            "got: {err}"
        );
        assert!(err.to_string().contains("sign in"), "got: {err}");
    }

    #[cfg(not(feature = "spotify"))]
    #[test]
    fn spotify_reports_not_compiled_in() {
        let source = AudioSource::ServiceStream {
            service: ServiceId::Spotify,
            track_id: "spotify-track".to_string(),
        };

        let err = resolve_service_stream_from_env(source).unwrap_err();
        assert!(
            matches!(
                err,
                ServiceStreamResolveError::UnsupportedService(ServiceId::Spotify)
            ),
            "got: {err}"
        );
        assert!(err.to_string().contains("not compiled"), "got: {err}");
    }

    #[test]
    fn url_resolution_maps_to_audio_source() {
        let resolved = ResolvedServiceStream::Url {
            url: "https://example.com/track.flac".to_string(),
            format_hint: Some("flac".to_string()),
            seekable: true,
        };
        let source = resolved_to_audio_source(ServiceId::Tidal, resolved).expect("url maps");
        assert_eq!(
            source,
            AudioSource::Url {
                url: "https://example.com/track.flac".to_string(),
                format_hint: Some("flac".to_string()),
                seekable: true,
            }
        );
    }

    #[test]
    fn pcm_resolution_maps_to_pcm_handoff_error() {
        let resolved = ResolvedServiceStream::Pcm {
            sample_rate: 44_100,
            channels: 2,
            bits_per_sample: 32,
            total_frames: Some(10),
            reader: Box::new(std::io::Cursor::new(Vec::new())),
        };
        let err = resolved_to_audio_source(ServiceId::Spotify, resolved).unwrap_err();
        assert!(
            matches!(
                err,
                ServiceStreamResolveError::PcmHandoffUnsupported(ServiceId::Spotify)
            ),
            "got: {err}"
        );
        assert!(err.to_string().contains("PCM"), "got: {err}");
    }
}
