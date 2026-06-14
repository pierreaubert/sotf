#[derive(Debug)]
pub(super) struct ApiRequest {
    pub(super) method: String,
    pub(super) path: String,
    pub(super) headers: Vec<(String, String)>,
    pub(super) body: Vec<u8>,
}

pub(super) struct ApiMediaSource {
    pub(super) path: std::path::PathBuf,
    pub(super) mime_type: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ApiLibraryAlbumSort {
    ArtistTitle,
    Title,
    Year,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SotfApiConnectionQrPayload {
    pub name: String,
    pub url: String,
    pub token: String,
}
