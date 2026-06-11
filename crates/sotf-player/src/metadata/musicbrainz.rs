use super::{MetadataError, MetadataImportCandidate};
use serde::Deserialize;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use url::Url;

#[allow(async_fn_in_trait)]
pub trait MetadataProvider {
    async fn search_album(
        &self,
        artist: Option<&str>,
        album: &str,
    ) -> Result<Vec<MetadataImportCandidate>, MetadataError>;

    async fn search_track(
        &self,
        artist: Option<&str>,
        title: &str,
    ) -> Result<Vec<MetadataImportCandidate>, MetadataError>;

    async fn lookup_by_isrc(
        &self,
        isrc: &str,
    ) -> Result<Vec<MetadataImportCandidate>, MetadataError>;

    async fn fetch_release(
        &self,
        release_id: &str,
    ) -> Result<MetadataImportCandidate, MetadataError>;
}

pub struct MusicBrainzProvider {
    endpoint: String,
    client: reqwest::Client,
    last_request: Mutex<Option<Instant>>,
}

impl MusicBrainzProvider {
    pub fn new(user_agent: impl Into<String>) -> Result<Self, MetadataError> {
        Self::with_endpoint("https://musicbrainz.org/ws/2/", user_agent)
    }

    pub fn with_endpoint(
        endpoint: impl Into<String>,
        user_agent: impl Into<String>,
    ) -> Result<Self, MetadataError> {
        let client = reqwest::Client::builder()
            .user_agent(user_agent.into())
            .build()
            .map_err(|err| MetadataError::Provider(err.to_string()))?;
        Ok(Self {
            endpoint: endpoint.into().trim_end_matches('/').to_string(),
            client,
            last_request: Mutex::new(None),
        })
    }

    async fn throttle(&self) {
        let mut last = self.last_request.lock().await;
        if let Some(previous) = *last {
            let elapsed = previous.elapsed();
            if elapsed < Duration::from_secs(1) {
                tokio::time::sleep(Duration::from_secs(1) - elapsed).await;
            }
        }
        *last = Some(Instant::now());
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        resource: &str,
        query: &[(&str, String)],
    ) -> Result<T, MetadataError> {
        self.throttle().await;
        let mut url = Url::parse(&format!(
            "{}/{}",
            self.endpoint,
            resource.trim_start_matches('/')
        ))
        .map_err(|err| MetadataError::Provider(err.to_string()))?;
        {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in query {
                pairs.append_pair(key, value);
            }
        }
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|err| MetadataError::Provider(err.to_string()))?
            .error_for_status()
            .map_err(|err| MetadataError::Provider(err.to_string()))?;
        response
            .json()
            .await
            .map_err(|err| MetadataError::Provider(err.to_string()))
    }
}

#[derive(Debug, Deserialize)]
struct MbSearch<T> {
    #[serde(default)]
    releases: Vec<T>,
    #[serde(default)]
    recordings: Vec<T>,
}

#[derive(Debug, Default, Deserialize)]
struct MbRelease {
    id: String,
    title: Option<String>,
    date: Option<String>,
    score: Option<String>,
    #[serde(rename = "artist-credit", default)]
    artist_credit: Vec<MbArtistCredit>,
}

#[derive(Debug, Default, Deserialize)]
struct MbRecording {
    id: String,
    title: Option<String>,
    score: Option<String>,
    #[serde(default)]
    isrcs: Vec<String>,
    #[serde(rename = "artist-credit", default)]
    artist_credit: Vec<MbArtistCredit>,
    #[serde(default)]
    releases: Vec<MbRelease>,
}

#[derive(Debug, Deserialize)]
struct MbArtistCredit {
    name: Option<String>,
}

fn year_from_date(date: Option<&str>) -> Option<u32> {
    date.and_then(|date| date.split('-').next())
        .and_then(|year| year.parse().ok())
}

fn credit_name(credits: &[MbArtistCredit]) -> Option<String> {
    let name = credits
        .iter()
        .filter_map(|credit| credit.name.as_deref())
        .collect::<Vec<_>>()
        .join("");
    (!name.trim().is_empty()).then(|| name)
}

fn score(score: Option<&str>) -> u8 {
    score.and_then(|s| s.parse().ok()).unwrap_or(0)
}

impl MetadataProvider for MusicBrainzProvider {
    async fn search_album(
        &self,
        artist: Option<&str>,
        album: &str,
    ) -> Result<Vec<MetadataImportCandidate>, MetadataError> {
        let query = match artist {
            Some(artist) if !artist.trim().is_empty() => {
                format!("release:\"{album}\" AND artist:\"{artist}\"")
            }
            _ => format!("release:\"{album}\""),
        };
        let result: MbSearch<MbRelease> = self
            .get_json(
                "release",
                &[
                    ("query", query),
                    ("fmt", "json".to_string()),
                    ("limit", "10".to_string()),
                ],
            )
            .await?;
        Ok(result
            .releases
            .into_iter()
            .map(|release| MetadataImportCandidate {
                provider_id: "musicbrainz".to_string(),
                provider_entity_id: release.id,
                title: None,
                artist: None,
                album_artist: credit_name(&release.artist_credit),
                album_title: release.title,
                year: year_from_date(release.date.as_deref()),
                track_number: None,
                disc_number: None,
                isrc: None,
                score: score(release.score.as_deref()),
            })
            .collect())
    }

    async fn search_track(
        &self,
        artist: Option<&str>,
        title: &str,
    ) -> Result<Vec<MetadataImportCandidate>, MetadataError> {
        let query = match artist {
            Some(artist) if !artist.trim().is_empty() => {
                format!("recording:\"{title}\" AND artist:\"{artist}\"")
            }
            _ => format!("recording:\"{title}\""),
        };
        let result: MbSearch<MbRecording> = self
            .get_json(
                "recording",
                &[
                    ("query", query),
                    ("fmt", "json".to_string()),
                    ("limit", "10".to_string()),
                ],
            )
            .await?;
        Ok(recordings_to_candidates(result.recordings))
    }

    async fn lookup_by_isrc(
        &self,
        isrc: &str,
    ) -> Result<Vec<MetadataImportCandidate>, MetadataError> {
        let result: MbSearch<MbRecording> = self
            .get_json(
                &format!("isrc/{isrc}"),
                &[
                    ("fmt", "json".to_string()),
                    ("inc", "recordings+releases+artist-credits".to_string()),
                ],
            )
            .await?;
        Ok(recordings_to_candidates(result.recordings))
    }

    async fn fetch_release(
        &self,
        release_id: &str,
    ) -> Result<MetadataImportCandidate, MetadataError> {
        let release: MbRelease = self
            .get_json(
                &format!("release/{release_id}"),
                &[
                    ("fmt", "json".to_string()),
                    ("inc", "artist-credits".to_string()),
                ],
            )
            .await?;
        Ok(MetadataImportCandidate {
            provider_id: "musicbrainz".to_string(),
            provider_entity_id: release.id,
            title: None,
            artist: None,
            album_artist: credit_name(&release.artist_credit),
            album_title: release.title,
            year: year_from_date(release.date.as_deref()),
            track_number: None,
            disc_number: None,
            isrc: None,
            score: 100,
        })
    }
}

fn recordings_to_candidates(recordings: Vec<MbRecording>) -> Vec<MetadataImportCandidate> {
    recordings
        .into_iter()
        .map(|recording| {
            let release = recording.releases.first();
            MetadataImportCandidate {
                provider_id: "musicbrainz".to_string(),
                provider_entity_id: recording.id,
                title: recording.title,
                artist: credit_name(&recording.artist_credit),
                album_artist: release.and_then(|release| credit_name(&release.artist_credit)),
                album_title: release.and_then(|release| release.title.clone()),
                year: release.and_then(|release| year_from_date(release.date.as_deref())),
                track_number: None,
                disc_number: None,
                isrc: recording.isrcs.first().cloned(),
                score: score(recording.score.as_deref()),
            }
        })
        .collect()
}
