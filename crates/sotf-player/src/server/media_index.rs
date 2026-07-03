use super::misc::is_safe_media_id;
use super::misc::media_track_id;
use super::misc::mime_type_for_path;
use super::types::ApiMediaSource;
use crate::library::MusicLibrary;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct MediaSourceIndex {
    library_version: Option<u64>,
    sources: HashMap<String, ApiMediaSource>,
    #[cfg(test)]
    rebuilds: usize,
}

impl MediaSourceIndex {
    pub(super) fn lookup(
        &mut self,
        library: &MusicLibrary,
        library_version: u64,
        track_id: &str,
    ) -> Option<ApiMediaSource> {
        if self.library_version != Some(library_version) {
            self.rebuild(library, library_version);
        }
        self.sources.get(track_id).cloned()
    }

    fn rebuild(&mut self, library: &MusicLibrary, library_version: u64) {
        self.sources.clear();
        for album in &library.albums {
            for (index, track) in album.tracks.iter().enumerate() {
                let source = ApiMediaSource {
                    path: track.path.clone(),
                    mime_type: mime_type_for_path(&track.path).to_string(),
                };
                self.sources
                    .insert(public_track_id(track, album, index), source.clone());
                self.sources
                    .entry(media_track_id(track, album, index))
                    .or_insert(source);
            }
        }
        self.library_version = Some(library_version);
        #[cfg(test)]
        {
            self.rebuilds += 1;
        }
    }

    #[cfg(test)]
    pub(super) fn rebuilds(&self) -> usize {
        self.rebuilds
    }
}

fn public_track_id(
    track: &crate::library::Track,
    album: &crate::library::Album,
    index: usize,
) -> String {
    if let Some(uuid) = track.uuid.as_deref()
        && is_safe_media_id(uuid)
    {
        return format!("uuid:{uuid}");
    }
    media_track_id(track, album, index)
}
