use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct TidalSession {
    #[serde(rename = "userId")]
    pub(super) user_id: u64,
    #[serde(rename = "countryCode")]
    pub(super) country_code: String,
}

#[derive(Deserialize)]
pub(super) struct TidalDeviceAuth {
    #[serde(rename = "deviceCode")]
    pub(super) device_code: String,
    #[serde(rename = "userCode")]
    pub(super) user_code: String,
    #[serde(rename = "verificationUri")]
    pub(super) verification_uri: String,
    #[serde(rename = "verificationUriComplete")]
    pub(super) verification_uri_complete: Option<String>,
    #[serde(rename = "expiresIn", default)]
    pub(super) expires_in: u64,
}

#[derive(Deserialize)]
pub(super) struct TidalSearchResult<T> {
    pub(super) items: Vec<T>,
}

#[derive(Deserialize)]
pub(super) struct TidalTrack {
    pub(super) id: u64,
    pub(super) title: String,
    pub(super) duration: u32,
    #[serde(rename = "trackNumber")]
    pub(super) track_number: u32,
    pub(super) artist: TidalArtist,
    pub(super) album: TidalAlbumRef,
}

#[derive(Deserialize)]
pub(super) struct TidalAlbum {
    pub(super) id: u64,
    pub(super) title: String,
    pub(super) artist: TidalArtist,
    pub(super) cover: Option<String>,
    #[serde(rename = "numberOfTracks")]
    pub(super) number_of_tracks: u32,
    #[serde(rename = "releaseDate")]
    pub(super) release_date: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct TidalAlbumRef {
    pub(super) title: String,
    pub(super) cover: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct TidalArtist {
    pub(super) name: String,
}

#[derive(Deserialize)]
pub(super) struct TidalStreamInfo {
    pub(super) url: String,
    pub(super) codec: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct TidalTokenResponse {
    #[serde(rename = "access_token")]
    pub(super) access_token: String,
    #[serde(rename = "refresh_token")]
    pub(super) refresh_token: Option<String>,
}
