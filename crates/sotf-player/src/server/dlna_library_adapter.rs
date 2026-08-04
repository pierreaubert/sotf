use super::misc::media_track_id;
use super::misc::mime_type_for_path;
use super::server_state::ServerState;
use super::track::track_to_media_track;
use sotf_dlna::MediaServerAdapter;
use std::sync::Arc;

/// Adapter bridging DLNA ContentDirectory requests to the SOTF library.
pub(super) struct DlnaLibraryAdapter {
    pub(super) state: Arc<ServerState>,
}

impl MediaServerAdapter for DlnaLibraryAdapter {
    fn browse_albums(&self, start: u32, count: u32) -> (Vec<sotf_dlna::MediaAlbum>, u32) {
        let library = self.state.library.lock();
        let total = library.albums.len() as u32;
        let albums: Vec<sotf_dlna::MediaAlbum> = library
            .albums
            .iter()
            .skip(start as usize)
            .take(if count == 0 {
                library.albums.len()
            } else {
                count as usize
            })
            .map(|a| sotf_dlna::MediaAlbum {
                id: a.id.map_or_else(|| a.title.clone(), |id| id.to_string()),
                title: a.title.clone(),
                artist: a.artist(),
                year: a.year,
                track_count: a.tracks.len() as u32,
            })
            .collect();
        (albums, total)
    }

    fn browse_album_tracks(&self, album_id: &str) -> Vec<sotf_dlna::MediaTrack> {
        let library = self.state.library.lock();
        let album = library
            .albums
            .iter()
            .find(|a| a.id.is_some_and(|id| id.to_string() == album_id) || a.title == album_id);

        match album {
            Some(album) => album
                .tracks
                .iter()
                .enumerate()
                .map(|(i, t)| track_to_media_track(t, album, i))
                .collect(),
            None => vec![],
        }
    }

    fn search_tracks(
        &self,
        query: &str,
        start: u32,
        count: u32,
    ) -> (Vec<sotf_dlna::MediaTrack>, u32) {
        let library = self.state.library.lock();
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for album in &library.albums {
            for (i, track) in album.tracks.iter().enumerate() {
                let matches = track
                    .title
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&query_lower)
                    || track
                        .artist
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query_lower)
                    || album.title.to_lowercase().contains(&query_lower);

                if matches {
                    results.push(track_to_media_track(track, album, i));
                }
            }
        }

        let total = results.len() as u32;
        let page: Vec<_> = results
            .into_iter()
            .skip(start as usize)
            .take(if count == 0 {
                usize::MAX
            } else {
                count as usize
            })
            .collect();
        (page, total)
    }

    fn album_count(&self) -> u32 {
        let library = self.state.library.lock();
        library.albums.len() as u32
    }

    fn content_directory_update_id(&self) -> u32 {
        self.state
            .library_version
            .load(std::sync::atomic::Ordering::Relaxed)
            .min(u32::MAX as u64) as u32
    }

    fn media_path(&self, track_id: &str) -> Option<sotf_dlna::MediaSource> {
        let library = self.state.library.lock();
        for album in &library.albums {
            for (i, track) in album.tracks.iter().enumerate() {
                if media_track_id(track, album, i) == track_id {
                    return Some(sotf_dlna::MediaSource {
                        path: track.path.clone(),
                        mime_type: mime_type_for_path(&track.path).to_string(),
                    });
                }
            }
        }
        None
    }
}
