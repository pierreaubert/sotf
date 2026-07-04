use crate::sotf_api_client::{SotfApiAlbum, SotfApiAlbumTracks, SotfApiClient, SotfApiClientError};
use sotf_audio::decoder::AudioSource;
use sotf_federation::{ProviderAlbum, ProviderTrack};

pub(super) fn sotf_api_album_to_provider_album(
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

pub(super) fn sotf_peer_client(
    host: &str,
    port: u16,
    token: &str,
    accepted_fingerprint: Option<&str>,
) -> Result<SotfApiClient, SotfApiClientError> {
    let base_url = sotf_peer_base_url(host, port);
    SotfApiClient::new_with_pinned_fingerprint(base_url, token, accepted_fingerprint)
}

pub(super) fn sotf_peer_base_url(host: &str, port: u16) -> String {
    let trimmed = host.trim().trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}:{port}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sotf_peer_base_url_defaults_to_https_for_bare_hosts() {
        assert_eq!(
            sotf_peer_base_url("studio.local", 8732),
            "https://studio.local:8732"
        );
    }

    #[test]
    fn sotf_peer_base_url_preserves_explicit_scheme() {
        assert_eq!(
            sotf_peer_base_url("http://studio.local:8732/", 9999),
            "http://studio.local:8732"
        );
    }
}
